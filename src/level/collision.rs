//! Kinematic character collision against the level's props: walls push characters out, floors
//! and stairs carry them, terrain is the fallback ground. No physics engine (none supports
//! Bevy 0.20 yet); characters are vertical capsules tested against nearby triangles.
//!
//! Triangles come from the same meshes the props draw with, transformed to world space lazily
//! for items near a body and dropped when the body moves away.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

use super::{CELL, Item, Level, PropStreaming, Terrain, affine, cell_of};

/// A character the level should keep out of walls and on the ground.
#[derive(Component)]
pub struct KinematicBody {
    pub radius: f32,
    pub height: f32,
    pub vertical_velocity: f32,
    pub grounded: bool,
    /// Set by gameplay for one frame to jump.
    pub jump: Option<f32>,
    /// On a ladder (`ladders`): no gravity; gameplay moves it up and down, the floor stops it.
    pub climbing: bool,
    /// Swimming (`gameplay::swim`): no gravity either; the floor (river bed) still stops it.
    pub swimming: bool,
    /// Moved by a montage's root motion (`gameplay::vault`, the BP's Flying movement mode): no
    /// gravity, floors or walls; gameplay sets the position.
    pub scripted: bool,
    /// Seconds spent held in the air waiting for the props below to load (see `move_bodies`).
    held: f32,
    /// Set for one frame on touching down from a fall: the downward speed, m/s (OnLanded's
    /// `GetVelocity().Z * -1`).
    pub landed: Option<f32>,
}

impl KinematicBody {
    pub fn new(radius: f32, height: f32) -> Self {
        Self { radius, height, vertical_velocity: 0.0, grounded: true, jump: None, climbing: false, swimming: false, scripted: false, landed: None, held: 0.0 }
    }
}

/// The highest kerb or stair the body walks up.
pub const STEP: f32 = 0.45;
/// Longest a falling body waits for the props under it to load.
const HOLD_FOR: f32 = 5.0;
/// UE gravity (-980 cm/s^2) x BP_PlayerCharacter CharMoveComp GravityScale 1.75.
const GRAVITY: f32 = -9.8 * 1.75;
/// Surfaces steeper than this (normal.y below) are walls.
const WALKABLE: f32 = 0.6;

#[derive(Clone)]
struct ItemTris {
    min: Vec3,
    max: Vec3,
    tris: Arc<Vec<[Vec3; 3]>>,
}

#[derive(Resource, Default)]
pub struct CollisionCache {
    items: HashMap<(IVec2, u32), Option<ItemTris>>,
    /// Cells recently raycast through (seconds left), kept built between shots.
    pinned: HashMap<IVec2, f32>,
    /// Moving props (doors), keyed by their owner's index; rebuilt by the owner when they move.
    dynamic: HashMap<usize, ItemTris>,
    /// Water surfaces (BP_WaterBox / BP_WaterSpline), like `items`.
    water: HashMap<(IVec2, u32), Option<ItemTris>>,
    /// Every water item in the level, found once. They're searched all together: a BP_WaterBox
    /// is bucketed by its origin but can reach hundreds of metres past its cell (the sea's does
    /// ~680 m), so looking only in the cells around a point missed water near a lake's shore.
    water_keys: Option<Arc<Vec<(IVec2, u32)>>>,
}

/// World-space triangles of `mesh` placed at `t`.
pub fn tris_of(mesh: &Mesh, t: &Transform) -> Vec<[Vec3; 3]> {
    let mut out = Vec::new();
    mesh_tris(mesh, t, &mut out);
    out
}

fn bounded(tris: Vec<[Vec3; 3]>) -> Option<ItemTris> {
    if tris.is_empty() {
        return None;
    }
    let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
    for t in &tris {
        for v in t {
            min = min.min(*v);
            max = max.max(*v);
        }
    }
    Some(ItemTris { min, max, tris: Arc::new(tris) })
}

fn mesh_tris(mesh: &Mesh, t: &Transform, out: &mut Vec<[Vec3; 3]>) {
    let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return };
    let m = t.to_matrix();
    let w: Vec<Vec3> = p.iter().map(|v| m.transform_point3(Vec3::from_array(*v))).collect();
    let mut push = |a: usize, b: usize, c: usize| {
        if a < w.len() && b < w.len() && c < w.len() {
            out.push([w[a], w[b], w[c]]);
        }
    };
    match mesh.indices() {
        Some(Indices::U16(i)) => i.chunks_exact(3).for_each(|c| push(c[0] as usize, c[1] as usize, c[2] as usize)),
        Some(Indices::U32(i)) => i.chunks_exact(3).for_each(|c| push(c[0] as usize, c[1] as usize, c[2] as usize)),
        None => (0..w.len() / 3).for_each(|k| push(k * 3, k * 3 + 1, k * 3 + 2)),
    }
}

impl CollisionCache {
    pub(super) fn has_dynamic(&self, key: usize) -> bool {
        self.dynamic.contains_key(&key)
    }

    /// Replace a moving prop's triangles (an empty list still marks it as built).
    pub fn set_dynamic(&mut self, key: usize, tris: Vec<[Vec3; 3]>) {
        let item = bounded(tris).unwrap_or(ItemTris { min: Vec3::MAX, max: Vec3::MIN, tris: Arc::default() });
        self.dynamic.insert(key, item);
    }

    /// Triangles currently held for moving prop `key`.
    pub fn dynamic_tris(&self, key: usize) -> usize {
        self.dynamic.get(&key).map_or(0, |d| d.tris.len())
    }

    pub(super) fn clear_dynamic(&mut self) {
        self.dynamic.clear();
    }

    fn item(&mut self, level: &Level, streaming: &PropStreaming, meshes: &Assets<Mesh>, cell: IVec2, index: u32) -> Option<ItemTris> {
        if let Some(c) = self.items.get(&(cell, index)) {
            return c.clone();
        }
        if streaming.is_removed((cell, index)) {
            self.items.insert((cell, index), None);
            return None;
        }
        let item = level.cells.get(&cell)?.get(index as usize)?;
        let mut tris = Vec::new();
        let ready = match *item {
            Item::Static { mesh, transform } if let Some(simple) = &level.meshes[mesh as usize].simple => {
                let m = transform.to_matrix();
                tris.extend(simple.iter().map(|t| t.map(|v| m.transform_point3(v))));
                true
            }
            Item::Static { mesh, transform } => {
                let Some(parts) = &level.meshes[mesh as usize].parts else { return None };
                for p in parts.iter().filter(|p| p.collide) {
                    let Some(m) = meshes.get(&p.mesh) else { return None };
                    mesh_tris(&m, &transform, &mut tris);
                }
                true
            }
            Item::Spline { index: s, .. } => {
                let Some(parts) = streaming.spline_meshes.get(&s) else { return None };
                let t = affine(&level.splines[s as usize].transform);
                for (h, _, _) in parts.iter().filter(|p| p.2) {
                    let Some(m) = meshes.get(h) else { return None };
                    mesh_tris(&m, &t, &mut tris);
                }
                true
            }
        };
        let built = if ready { bounded(tris) } else { None };
        self.items.insert((cell, index), built.clone());
        built
    }

    /// A placed item's water-surface triangles (None: not water, or not loaded yet).
    fn water_item(&mut self, level: &Level, streaming: &PropStreaming, meshes: &Assets<Mesh>, cell: IVec2, index: u32) -> Option<ItemTris> {
        if let Some(c) = self.water.get(&(cell, index)) {
            return c.clone();
        }
        let item = level.cells.get(&cell)?.get(index as usize)?;
        let mut tris = Vec::new();
        match *item {
            Item::Static { mesh, transform } => {
                let parts = level.meshes[mesh as usize].parts.as_ref()?;
                for p in parts.iter().filter(|p| p.water) {
                    let m = meshes.get(&p.mesh)?;
                    mesh_tris(m, &transform, &mut tris);
                }
            }
            Item::Spline { index: s, mesh } => {
                if super::is_water(&level.meshes[mesh as usize].path) {
                    let parts = streaming.spline_meshes.get(&s)?;
                    let t = affine(&level.splines[s as usize].transform);
                    for (h, _, _) in parts {
                        let m = meshes.get(h)?;
                        mesh_tris(m, &t, &mut tris);
                    }
                }
            }
        }
        let built = bounded(tris);
        self.water.insert((cell, index), built.clone());
        built
    }

    /// Water triangles of items overlapping the box around `p` (those whose meshes are in).
    fn near_water(&mut self, level: &Level, streaming: &PropStreaming, meshes: &Assets<Mesh>, p: Vec3, reach: Vec3) -> Vec<Arc<Vec<[Vec3; 3]>>> {
        let (lo, hi) = (p - reach, p + reach);
        let keys = self
            .water_keys
            .get_or_insert_with(|| {
                let mut keys: Vec<(IVec2, u32)> = level
                    .cells
                    .iter()
                    .flat_map(|(cell, items)| {
                        items.iter().enumerate().filter(|(_, it)| matches!(it, Item::Static { mesh, .. } | Item::Spline { mesh, .. } if super::is_water(&level.meshes[*mesh as usize].path))).map(move |(i, _)| (*cell, i as u32))
                    })
                    .collect();
                keys.sort_by_key(|(c, i)| (c.x, c.y, *i));
                Arc::new(keys)
            })
            .clone();
        let mut out = Vec::new();
        for &(cell, i) in keys.iter() {
            if let Some(it) = self.water_item(level, streaming, meshes, cell, i)
                && it.min.cmple(hi).all()
                && it.max.cmpge(lo).all()
            {
                out.push(it.tris.clone());
            }
        }
        out
    }

    /// Triangles of items whose bounds overlap the box around `p`.
    fn near(&mut self, level: &Level, streaming: &PropStreaming, meshes: &Assets<Mesh>, p: Vec3, reach: Vec3) -> Vec<Arc<Vec<[Vec3; 3]>>> {
        self.near_pending(level, streaming, meshes, p, reach).0
    }

    /// [`Self::near`], and whether the cells under the box (the whole column) still have props
    /// whose collision isn't built (cell not streamed in, or meshes loading).
    fn near_pending(&mut self, level: &Level, streaming: &PropStreaming, meshes: &Assets<Mesh>, p: Vec3, reach: Vec3) -> (Vec<Arc<Vec<[Vec3; 3]>>>, bool) {
        let mut pending = false;
        let (lo, hi) = (p - reach, p + reach);
        let mut out: Vec<_> = self.dynamic.values().filter(|it| it.min.cmple(hi).all() && it.max.cmpge(lo).all()).map(|it| it.tris.clone()).collect();
        let c0 = cell_of(lo);
        let c1 = cell_of(hi);
        for cz in c0.y..=c1.y {
            for cx in c0.x..=c1.x {
                let cell = IVec2::new(cx, cz);
                let Some(items) = level.cells.get(&cell) else { continue };
                if !streaming.loaded.contains_key(&cell) {
                    pending |= !items.is_empty();
                    continue;
                }
                for i in 0..items.len() as u32 {
                    let it = self.item(level, streaming, meshes, cell, i);
                    // Built items are cached (bounds or None); uncached ones are still loading.
                    pending |= !self.items.contains_key(&(cell, i));
                    if let Some(it) = it
                        && it.min.cmple(hi).all()
                        && it.max.cmpge(lo).all()
                    {
                        out.push(it.tris.clone());
                    }
                }
            }
        }
        (out, pending)
    }

    /// Drop a prop's cached triangles (it was taken out of the world).
    pub fn forget(&mut self, key: super::PropKey) {
        self.items.remove(&key);
    }

    /// The first streamed prop along `dir` (unit) within `max` (a short ray: a melee swing),
    /// with its distance.
    pub fn cast_item(&mut self, level: &Level, streaming: &PropStreaming, meshes: &Assets<Mesh>, origin: Vec3, dir: Vec3, max: f32) -> Option<(f32, super::PropKey)> {
        let end = origin + dir * max;
        let (lo, hi) = (origin.min(end) - Vec3::splat(0.05), origin.max(end) + Vec3::splat(0.05));
        let mut best: Option<(f32, super::PropKey)> = None;
        let (c0, c1) = (cell_of(lo), cell_of(hi));
        for cz in c0.y..=c1.y {
            for cx in c0.x..=c1.x {
                let cell = IVec2::new(cx, cz);
                let Some(items) = level.cells.get(&cell) else { continue };
                if !streaming.loaded.contains_key(&cell) {
                    continue;
                }
                for i in 0..items.len() as u32 {
                    let Some(it) = self.item(level, streaming, meshes, cell, i) else { continue };
                    if it.min.cmpgt(hi).any() || it.max.cmplt(lo).any() {
                        continue;
                    }
                    if let Some((d, _)) = first_hit(std::slice::from_ref(&it.tris), origin, dir, max)
                        && best.is_none_or(|b| d < b.0)
                    {
                        best = Some((d, (cell, i)));
                    }
                }
            }
        }
        best
    }

    /// Highest walkable prop surface under `xz` between heights `below..=above` (props must be
    /// streamed in; spawners use it to tell landscape from rooftops).
    pub fn prop_floor(&mut self, level: &Level, streaming: &PropStreaming, meshes: &Assets<Mesh>, xz: Vec2, (below, above): (f32, f32)) -> Option<f32> {
        let (x, z) = (xz.x, xz.y);
        let mid = (below + above) * 0.5;
        let tris = self.near(level, streaming, meshes, Vec3::new(x, mid, z), Vec3::new(0.5, (above - below) * 0.5 + 0.1, 0.5));
        floor_under(&tris, x, z, below, above)
    }

    fn forget_far(&mut self, keep: &[Vec3], dt: f32) {
        self.pinned.retain(|_, t| {
            *t -= dt;
            *t > 0.0
        });
        let pinned = &self.pinned;
        let wanted = |cell: &IVec2| {
            pinned.contains_key(cell)
                || keep.iter().any(|p| {
                    let c = cell_of(*p);
                    (cell.x - c.x).abs() <= 1 && (cell.y - c.y).abs() <= 1
                })
        };
        self.items.retain(|(cell, _), _| wanted(cell));
        self.water.retain(|(cell, _), _| wanted(cell));
    }
}

fn closest_on_triangle(p: Vec3, [a, b, c]: [Vec3; 3]) -> Vec3 {
    // Ericson, Real-Time Collision Detection 5.1.5
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// Height of the highest walkable triangle under (x, z) between `below` and `above`.
fn floor_under(tris: &[Arc<Vec<[Vec3; 3]>>], x: f32, z: f32, below: f32, above: f32) -> Option<f32> {
    let mut best: Option<f32> = None;
    for set in tris {
        for t in set.iter() {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            let len = n.length();
            if len < 1e-6 || (n.y / len).abs() < WALKABLE {
                continue;
            }
            // Barycentric test in XZ.
            let (a, b, c) = (t[0].xz(), t[1].xz(), t[2].xz());
            let p = Vec2::new(x, z);
            let d = (b - a).perp_dot(c - a);
            if d.abs() < 1e-9 {
                continue;
            }
            let u = (p - a).perp_dot(c - a) / d;
            let v = (b - a).perp_dot(p - a) / d;
            if u < 0.0 || v < 0.0 || u + v > 1.0 {
                continue;
            }
            let y = t[0].y + u * (t[1].y - t[0].y) + v * (t[2].y - t[0].y);
            if y >= below && y <= above && best.is_none_or(|b| y > b) {
                best = Some(y);
            }
        }
    }
    best
}

/// Push a capsule (feet at `p`) sideways out of wall triangles. Spheres stacked from step height
/// up to the head, so kerbs and stairs below the step are climbed rather than blocked.
fn push_out_of_walls(mut p: Vec3, radius: f32, height: f32, tris: &[Arc<Vec<[Vec3; 3]>>]) -> Vec3 {
    for _ in 0..2 {
        for k in 0..3 {
            let h = STEP + radius + (height - STEP - 2.0 * radius).max(0.0) * (k as f32 / 2.0);
            for set in tris {
                for tri in set.iter() {
                    let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
                    if n.y.abs() >= WALKABLE {
                        continue;
                    }
                    let centre = p + Vec3::Y * h;
                    let q = closest_on_triangle(centre, *tri);
                    // Overlap in 3D, but resolve sideways only (ground is handled separately).
                    let dist = centre.distance(q);
                    let mut side = centre - q;
                    side.y = 0.0;
                    let side_len = side.length();
                    if dist < radius && side_len > 1e-5 {
                        p += side / side_len * (radius - dist);
                    }
                }
            }
        }
    }
    p
}

#[allow(clippy::too_many_arguments)]
pub fn move_bodies(
    time: Res<Time>,
    level: Option<Res<Level>>,
    streaming: Option<Res<PropStreaming>>,
    terrain: Option<Res<Terrain>>,
    meshes: Res<Assets<Mesh>>,
    mut cache: ResMut<CollisionCache>,
    mut bodies: Query<(&mut KinematicBody, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.05);
    let mut positions = Vec::new();
    for (mut body, mut t) in &mut bodies {
        let mut p = t.translation;
        body.landed = None;
        if body.scripted {
            body.jump = None;
            body.grounded = false;
            body.vertical_velocity = 0.0;
            positions.push(p);
            continue;
        }
        let (tris, pending) = match (&level, &streaming) {
            (Some(l), Some(s)) => cache.near_pending(l, s, &meshes, p + Vec3::Y * body.height * 0.5, Vec3::new(body.radius + 1.0, body.height, body.radius + 1.0)),
            _ => (Vec::new(), false),
        };

        p = push_out_of_walls(p, body.radius, body.height, &tris);

        // Ground: terrain or the highest floor within a step of the feet.
        let terrain_y = terrain.as_ref().and_then(|t| t.ground_under(p));
        let floor_y = floor_under(&tris, p.x, p.z, p.y - 50.0, p.y + STEP);
        let ground = match (terrain_y, floor_y) {
            (Some(a), Some(b)) => a.max(b),
            (a, b) => a.or(b).unwrap_or(0.0),
        };

        if body.climbing || body.swimming {
            body.jump = None;
            body.grounded = false;
            p.y = (p.y + body.vertical_velocity * dt).max(ground);
            t.translation = p;
            positions.push(p);
            continue;
        }
        if let Some(v) = body.jump.take()
            && body.grounded
        {
            body.vertical_velocity = v;
            body.grounded = false;
        }
        if body.grounded {
            if p.y - ground > STEP {
                body.grounded = false; // walked off an edge
            } else {
                p.y = ground;
            }
        }
        // Not ours: UE has every floor loaded. A body placed (spawn, save load, teleport) above
        // props that are still streaming in hangs there instead of dropping through them, for up
        // to HOLD_FOR seconds.
        if !body.grounded && body.vertical_velocity <= 0.0 && pending && body.held < HOLD_FOR {
            if body.held == 0.0 {
                info!("body held at {p}: props below still loading");
            }
            body.held += dt;
            body.vertical_velocity = 0.0;
            t.translation = p;
            positions.push(p);
            continue;
        }
        if !pending {
            body.held = 0.0;
        }
        if !body.grounded {
            body.vertical_velocity += GRAVITY * dt;
            p.y += body.vertical_velocity * dt;
            if p.y <= ground {
                p.y = ground;
                body.landed = Some(-body.vertical_velocity);
                body.vertical_velocity = 0.0;
                body.grounded = true;
            }
        }
        t.translation = p;
        positions.push(p);
    }
    cache.forget_far(&positions, dt);
}

/// Distance along `dir` (unit) to a triangle, either side facing (Möller-Trumbore).
fn ray_triangle(origin: Vec3, dir: Vec3, [a, b, c]: [Vec3; 3]) -> Option<f32> {
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 0.0).then_some(t)
}

/// Nearest hit among triangle sets, within `max`, and the triangle hit.
fn first_hit(sets: &[Arc<Vec<[Vec3; 3]>>], origin: Vec3, dir: Vec3, max: f32) -> Option<(f32, [Vec3; 3])> {
    sets.iter().flat_map(|s| s.iter()).filter_map(|t| ray_triangle(origin, dir, *t).map(|d| (d, *t))).filter(|&(d, _)| d <= max).min_by(|a, b| a.0.total_cmp(&b.0))
}

/// Whether a triangle (in the box's frame) touches the box centred at the origin with half
/// extents `half`: the separating axis test over the box's faces, the triangle's plane and the
/// nine edge cross products.
pub fn tri_box(tri: [Vec3; 3], half: Vec3) -> bool {
    let edges = [tri[1] - tri[0], tri[2] - tri[1], tri[0] - tri[2]];
    let separated = |axis: Vec3| {
        if axis.length_squared() < 1e-12 {
            return false;
        }
        let p = tri.map(|v| v.dot(axis));
        let r = half.x * axis.x.abs() + half.y * axis.y.abs() + half.z * axis.z.abs();
        p[0].min(p[1]).min(p[2]) > r || p[0].max(p[1]).max(p[2]) < -r
    };
    let box_axes = [Vec3::X, Vec3::Y, Vec3::Z];
    if box_axes.iter().any(|&a| separated(a)) || separated(edges[0].cross(edges[1])) {
        return false;
    }
    !box_axes.iter().any(|&a| edges.iter().any(|&e| separated(a.cross(e))))
}

/// A triangle's unit normal, turned to face against `dir`.
pub fn facing_normal([a, b, c]: [Vec3; 3], dir: Vec3) -> Vec3 {
    let n = (b - a).cross(c - a).normalize_or_zero();
    if n.dot(dir) > 0.0 { -n } else { n }
}

/// Rays against the level's streamed props (bullets). Props that aren't streamed in or whose
/// meshes haven't loaded are transparent.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PropRays<'w> {
    level: Option<Res<'w, Level>>,
    streaming: Option<Res<'w, PropStreaming>>,
    meshes: Res<'w, Assets<Mesh>>,
    cache: ResMut<'w, CollisionCache>,
}

impl PropRays<'_> {
    /// Set a moving prop's triangles (`CollisionCache::set_dynamic`) where this param already
    /// holds the cache, with the meshes to read them from.
    /// Replace moving prop `key`'s triangles (empty: none).
    pub fn set_dynamic(&mut self, key: usize, tris: Vec<[Vec3; 3]>) {
        self.cache.set_dynamic(key, tris);
    }

    pub fn set_dynamic_from(&mut self, key: usize, mesh: &Handle<Mesh>, t: &Transform) -> bool {
        let Some(m) = self.meshes.get(mesh) else { return false };
        let tris = tris_of(m, t);
        self.cache.set_dynamic(key, tris);
        true
    }

    /// The highest water surface over (x, z) between heights `below..=above`.
    pub fn water_surface(&mut self, x: f32, z: f32, below: f32, above: f32) -> Option<f32> {
        let (Some(level), Some(streaming)) = (self.level.as_deref(), self.streaming.as_deref()) else { return None };
        let mid = (below + above) * 0.5;
        let tris = self.cache.near_water(level, streaming, &self.meshes, Vec3::new(x, mid, z), Vec3::new(0.5, (above - below) * 0.5 + 0.1, 0.5));
        floor_under(&tris, x, z, below, above)
    }

    /// Whether a streamed or moving prop's triangle crosses the box at `centre` with half extents
    /// `half` turned by `rot` (a BoxTraceSingle whose start and end are the same point).
    pub fn overlaps_box(&mut self, centre: Vec3, half: Vec3, rot: Quat) -> bool {
        let (Some(level), Some(streaming)) = (self.level.as_deref(), self.streaming.as_deref()) else { return false };
        let reach = (rot * Vec3::X * half.x).abs() + (rot * Vec3::Y * half.y).abs() + (rot * Vec3::Z * half.z).abs();
        let sets = self.cache.near(level, streaming, &self.meshes, centre, reach);
        let inv = rot.inverse();
        sets.iter().flat_map(|s| s.iter()).any(|t| tri_box(t.map(|v| inv * (v - centre)), half))
    }

    /// Distance to the first prop surface along `dir` (unit) within `max`.
    pub fn cast(&mut self, origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
        self.cast_hit(origin, dir, max).map(|(d, _)| d)
    }

    /// Like `cast`, with the surface normal (facing the ray) at the hit.
    pub fn cast_hit(&mut self, origin: Vec3, dir: Vec3, max: f32) -> Option<(f32, Vec3)> {
        const SEGMENT: f32 = 16.0;
        let (Some(level), Some(streaming)) = (self.level.as_deref(), self.streaming.as_deref()) else { return None };
        let cache = &mut *self.cache;
        let mut s = 0.0;
        while s < max {
            let e = (s + SEGMENT).min(max);
            let (a, b) = (origin + dir * s, origin + dir * e);
            let (lo, hi) = (cell_of(a.min(b)), cell_of(a.max(b)));
            for cz in lo.y..=hi.y {
                for cx in lo.x..=hi.x {
                    cache.pinned.insert(IVec2::new(cx, cz), 2.0);
                }
            }
            let sets = cache.near(level, streaming, &self.meshes, (a + b) * 0.5, (b - a).abs() * 0.5 + Vec3::splat(0.05));
            if let Some((d, tri)) = first_hit(&sets, origin, dir, e + 0.05) {
                return Some((d, facing_normal(tri, dir)));
            }
            s = e;
        }
        None
    }
}

/// USpringArmComponent's collision test (on by default; ProbeSize 12 cm): the camera comes in to
/// just short of the first thing between the pivot and where the arm wants it.
pub const CAMERA_PROBE: f32 = 0.12;

/// The arm length to use given the wanted `arm` and the first prop hit along it.
pub fn spring_arm(arm: f32, hit: Option<f32>) -> f32 {
    hit.map_or(arm, |d| (d - CAMERA_PROBE).clamp(0.1, arm))
}

#[allow(dead_code)]
const _: f32 = CELL;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangles_against_a_box() {
        let half = Vec3::new(1.0, 0.5, 1.0);
        // Ground just under the box, then crossing its bottom.
        let ground = |y: f32| [Vec3::new(-5.0, y, -5.0), Vec3::new(5.0, y, -5.0), Vec3::new(0.0, y, 5.0)];
        assert!(!tri_box(ground(-0.6), half));
        assert!(tri_box(ground(-0.4), half));
        // A triangle wholly inside, and one off to the side whose bounds overlap the box's.
        assert!(tri_box([Vec3::ZERO, Vec3::X * 0.1, Vec3::Z * 0.1], half));
        assert!(!tri_box([Vec3::new(0.5, 0.0, 1.6), Vec3::new(1.6, 0.0, 0.5), Vec3::new(1.6, 0.0, 1.6)], half));
        assert!(tri_box([Vec3::new(0.4, 0.0, 1.5), Vec3::new(1.5, 0.0, 0.4), Vec3::new(1.6, 0.0, 1.6)], half));
    }

    #[test]
    fn spring_arm_stops_short_of_walls() {
        assert_eq!(spring_arm(3.0, None), 3.0);
        assert!((spring_arm(3.0, Some(1.0)) - 0.88).abs() < 1e-6);
        // A hit beyond the arm or right at the pivot.
        assert_eq!(spring_arm(3.0, Some(5.0)), 3.0);
        assert_eq!(spring_arm(3.0, Some(0.05)), 0.1);
    }

    #[test]
    fn closest_point_inside_and_on_edge() {
        let t = [Vec3::ZERO, Vec3::X, Vec3::Z];
        assert!((closest_on_triangle(Vec3::new(0.2, 1.0, 0.2), t) - Vec3::new(0.2, 0.0, 0.2)).length() < 1e-5);
        assert!((closest_on_triangle(Vec3::new(-1.0, 0.0, 0.5), t) - Vec3::new(0.0, 0.0, 0.5)).length() < 1e-5);
    }

    fn wall_at_x(x: f32) -> Arc<Vec<[Vec3; 3]>> {
        // A 4 m tall wall in the YZ plane at `x`.
        Arc::new(vec![
            [Vec3::new(x, 0.0, -5.0), Vec3::new(x, 4.0, -5.0), Vec3::new(x, 0.0, 5.0)],
            [Vec3::new(x, 4.0, -5.0), Vec3::new(x, 4.0, 5.0), Vec3::new(x, 0.0, 5.0)],
        ])
    }

    #[test]
    fn walls_push_out_but_low_kerbs_do_not() {
        let p = push_out_of_walls(Vec3::new(0.9, 0.0, 0.0), 0.35, 1.8, &[wall_at_x(1.0)]);
        assert!((p.x - 0.65).abs() < 0.02, "pushed to {p}");
        // A 0.3 m kerb face is under the step height: no push.
        let kerb = Arc::new(vec![[Vec3::new(1.0, 0.0, -5.0), Vec3::new(1.0, 0.3, -5.0), Vec3::new(1.0, 0.0, 5.0)]]);
        let p = push_out_of_walls(Vec3::new(0.9, 0.0, 0.0), 0.35, 1.8, &[kerb]);
        assert_eq!(p, Vec3::new(0.9, 0.0, 0.0));
    }

    #[test]
    fn rays_stop_at_the_nearest_wall_from_either_side() {
        let walls = [wall_at_x(5.0), wall_at_x(3.0)];
        let (d, tri) = first_hit(&walls, Vec3::new(0.0, 1.0, 0.0), Vec3::X, 100.0).unwrap();
        assert!((d - 3.0).abs() < 1e-4);
        assert!((facing_normal(tri, Vec3::X) - -Vec3::X).length() < 1e-4);
        // Back faces count too (the inside of a building); the normal still faces the ray.
        let (d, tri) = first_hit(&walls, Vec3::new(10.0, 1.0, 0.0), -Vec3::X, 100.0).unwrap();
        assert!((d - 5.0).abs() < 1e-4);
        assert!((facing_normal(tri, -Vec3::X) - Vec3::X).length() < 1e-4);
        // Over the top, out of range, or pointing away: nothing.
        assert!(first_hit(&walls, Vec3::new(0.0, 5.0, 0.0), Vec3::X, 100.0).is_none());
        assert!(first_hit(&walls, Vec3::new(0.0, 1.0, 0.0), Vec3::X, 2.5).is_none());
        assert!(first_hit(&walls, Vec3::new(0.0, 1.0, 0.0), -Vec3::X, 100.0).is_none());
    }

    #[test]
    fn floor_picks_highest_walkable_within_range() {
        let floor = Arc::new(vec![[Vec3::new(-1.0, 2.0, -1.0), Vec3::new(-1.0, 2.0, 1.0), Vec3::new(1.0, 2.0, 0.0)]]);
        assert_eq!(floor_under(&[floor.clone()], 0.0, 0.0, 0.0, 2.2), Some(2.0));
        assert_eq!(floor_under(&[floor], 0.0, 0.0, 0.0, 1.5), None);
    }
}
