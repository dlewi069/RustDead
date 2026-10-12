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
}

impl KinematicBody {
    pub fn new(radius: f32, height: f32) -> Self {
        Self { radius, height, vertical_velocity: 0.0, grounded: true, jump: None }
    }
}

const STEP: f32 = 0.45;
const GRAVITY: f32 = -26.0;
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
    fn item(&mut self, level: &Level, streaming: &PropStreaming, meshes: &Assets<Mesh>, cell: IVec2, index: u32) -> Option<ItemTris> {
        if let Some(c) = self.items.get(&(cell, index)) {
            return c.clone();
        }
        let item = level.cells.get(&cell)?.get(index as usize)?;
        let mut tris = Vec::new();
        let ready = match *item {
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
        let built = (ready && !tris.is_empty()).then(|| {
            let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
            for t in &tris {
                for v in t {
                    min = min.min(*v);
                    max = max.max(*v);
                }
            }
            ItemTris { min, max, tris: Arc::new(tris) }
        });
        self.items.insert((cell, index), built.clone());
        built
    }

    /// Triangles of items whose bounds overlap the box around `p`.
    fn near(&mut self, level: &Level, streaming: &PropStreaming, meshes: &Assets<Mesh>, p: Vec3, reach: Vec3) -> Vec<Arc<Vec<[Vec3; 3]>>> {
        let (lo, hi) = (p - reach, p + reach);
        let mut out = Vec::new();
        let c0 = cell_of(lo);
        let c1 = cell_of(hi);
        for cz in c0.y..=c1.y {
            for cx in c0.x..=c1.x {
                let cell = IVec2::new(cx, cz);
                let Some(items) = level.cells.get(&cell) else { continue };
                if !streaming.loaded.contains_key(&cell) {
                    continue;
                }
                for i in 0..items.len() as u32 {
                    if let Some(it) = self.item(level, streaming, meshes, cell, i)
                        && it.min.cmple(hi).all()
                        && it.max.cmpge(lo).all()
                    {
                        out.push(it.tris.clone());
                    }
                }
            }
        }
        out
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
        self.items.retain(|(cell, _), _| {
            pinned.contains_key(cell)
                || keep.iter().any(|p| {
                let c = cell_of(*p);
                (cell.x - c.x).abs() <= 1 && (cell.y - c.y).abs() <= 1
            })
        });
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
        let tris = match (&level, &streaming) {
            (Some(l), Some(s)) => cache.near(l, s, &meshes, p + Vec3::Y * body.height * 0.5, Vec3::new(body.radius + 1.0, body.height, body.radius + 1.0)),
            _ => Vec::new(),
        };

        p = push_out_of_walls(p, body.radius, body.height, &tris);

        // Ground: terrain or the highest floor within a step of the feet.
        let terrain_y = terrain.as_ref().map(|t| t.height(p.x, p.z));
        let floor_y = floor_under(&tris, p.x, p.z, p.y - 50.0, p.y + STEP);
        let ground = match (terrain_y, floor_y) {
            (Some(a), Some(b)) => a.max(b),
            (a, b) => a.or(b).unwrap_or(0.0),
        };

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
        if !body.grounded {
            body.vertical_velocity += GRAVITY * dt;
            p.y += body.vertical_velocity * dt;
            if p.y <= ground {
                p.y = ground;
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

/// Nearest hit among triangle sets, within `max`.
fn first_hit(sets: &[Arc<Vec<[Vec3; 3]>>], origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
    sets.iter().flat_map(|s| s.iter()).filter_map(|t| ray_triangle(origin, dir, *t)).filter(|&d| d <= max).min_by(f32::total_cmp)
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
    /// Distance to the first prop surface along `dir` (unit) within `max`.
    pub fn cast(&mut self, origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
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
            if let Some(d) = first_hit(&sets, origin, dir, e + 0.05) {
                return Some(d);
            }
            s = e;
        }
        None
    }
}

#[allow(dead_code)]
const _: f32 = CELL;

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!((first_hit(&walls, Vec3::new(0.0, 1.0, 0.0), Vec3::X, 100.0).unwrap() - 3.0).abs() < 1e-4);
        // Back faces count too (the inside of a building).
        assert!((first_hit(&walls, Vec3::new(10.0, 1.0, 0.0), -Vec3::X, 100.0).unwrap() - 5.0).abs() < 1e-4);
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
