//! Landscape grass: Terrain_Material's grass output, scattered at runtime like UE's
//! `ALandscapeProxy` grass. `assets/data/grass.json` (`extractor/gen_grass.py`) lists each paint
//! layer's `LandscapeGrassType` varieties; the layer's painted weight (minus `NoFoliage`) is the
//! chance an instance appears at a spot.
//!
//! - Grid varieties sit on a world grid `10 / sqrt(GrassDensity)` m apart (density is per
//!   10 x 10 m), jittered by `PlacementJitter`; the rest scatter at random at the same density.
//! - Random yaw, aligned to the slope, scaled per `ScaleX/Y/Z` (`Uniform` uses X for all).
//! - Streamed in 16 m cells; a cell holds the varieties whose `EndCullDistance` reaches it and is
//!   rebuilt when that set changes. UE fades between Start/EndCullDistance; here they pop.

use std::collections::HashMap;

use bevy::gltf::{Gltf, GltfMesh};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use serde::Deserialize;

use super::Terrain;
use crate::menus::AppState;
use crate::models::GameMaterials;

const CELL: f32 = 16.0;
/// Cells spawned per frame (each is a few hundred instances at most).
const BUDGET: usize = 6;

#[derive(Deserialize, Clone, Debug)]
pub struct Variety {
    pub mesh: String,
    pub material: Option<String>,
    /// Instances per 10 x 10 m at full weight.
    pub density: f32,
    pub cull: [f32; 2],
    pub scale: [[f32; 2]; 3],
    pub uniform: bool,
    pub grid: bool,
    pub jitter: f32,
    pub random_rotation: bool,
    pub align: bool,
    pub shadows: bool,
}

struct LoadedVariety {
    def: Variety,
    /// Weight channel of its paint layer.
    channel: usize,
    gltf: Handle<Gltf>,
    parts: Option<Vec<(Handle<Mesh>, Handle<StandardMaterial>)>>,
}

/// The painted layer weights, CPU side (terrain_weights.png: 3 RGBA tiles = 12 channels, the
/// eleven paint layers then NoFoliage), over the terrain's extent.
pub struct Weights {
    pub size: [usize; 2],
    pub data: Vec<u8>,
    pub origin: Vec2,
    pub extent: Vec2,
}

pub const NO_FOLIAGE: usize = 11;

impl Weights {
    pub fn load(dir: &str, terrain: &Terrain) -> Option<Self> {
        let img = image::open(format!("{dir}/terrain_weights.png")).ok()?.into_rgba8();
        let (w, h3) = img.dimensions();
        Some(Self { size: [w as usize, h3 as usize / 3], data: img.into_raw(), origin: terrain.origin, extent: terrain.extent() })
    }

    /// Weight 0-1 of `channel` at world x/z (nearest texel, ~2 m).
    pub fn get(&self, channel: usize, x: f32, z: f32) -> f32 {
        let u = ((x - self.origin.x) / self.extent.x * (self.size[0] - 1) as f32).round();
        let v = ((z - self.origin.y) / self.extent.y * (self.size[1] - 1) as f32).round();
        if u < 0.0 || v < 0.0 || u >= self.size[0] as f32 || v >= self.size[1] as f32 {
            return 0.0;
        }
        let (tile, c) = (channel / 4, channel % 4);
        let row = tile * self.size[1] + v as usize;
        self.data[(row * self.size[0] + u as usize) * 4 + c] as f32 / 255.0
    }
}

#[derive(Resource)]
pub struct Grass {
    varieties: Vec<LoadedVariety>,
    weights: Weights,
    /// Spawned cells: their entity and which varieties they hold (bit per variety).
    cells: HashMap<IVec2, (Entity, u64)>,
}

#[derive(Deserialize)]
struct GrassFile(HashMap<String, Vec<Variety>>);

/// Deterministic per-spot randomness (splitmix64).
fn hash(a: i64, b: i64, c: u64) -> u64 {
    let mut z = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ c.wrapping_mul(0x1656_67B1_9E37_79F9);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit(h: u64, k: u32) -> f32 {
    ((h.rotate_left(k * 13) >> 40) as f32) / (1u64 << 24) as f32
}

/// Spots for one variety inside a cell: (x, z, per-spot hash). Grid varieties use a world grid
/// so neighbouring cells line up; random ones draw `density * area / 100` points.
pub fn spots(v: &Variety, id: u64, cell: IVec2) -> Vec<(f32, f32, u64)> {
    let (x0, z0) = (cell.x as f32 * CELL, cell.y as f32 * CELL);
    let mut out = Vec::new();
    if v.density <= 0.0 {
        return out;
    }
    if v.grid {
        let step = 10.0 / v.density.sqrt();
        let (i0, i1) = ((x0 / step).ceil() as i64, ((x0 + CELL) / step).ceil() as i64);
        let (j0, j1) = ((z0 / step).ceil() as i64, ((z0 + CELL) / step).ceil() as i64);
        for i in i0..i1 {
            for j in j0..j1 {
                let h = hash(i, j, id);
                let jx = (unit(h, 1) - 0.5) * step * v.jitter;
                let jz = (unit(h, 2) - 0.5) * step * v.jitter;
                out.push((i as f32 * step + jx, j as f32 * step + jz, h));
            }
        }
    } else {
        let expected = v.density * CELL * CELL / 100.0;
        let base = hash(cell.x as i64, cell.y as i64, id);
        let n = expected.floor() as u64 + u64::from(unit(base, 3) < expected.fract());
        for k in 0..n {
            let h = hash(cell.x as i64, cell.y as i64, id ^ (k + 1).wrapping_mul(0x51_7CC1_B727_220A));
            out.push((x0 + unit(h, 1) * CELL, z0 + unit(h, 2) * CELL, h));
        }
    }
    out
}

/// Placement for a kept spot: random yaw, slope alignment and the variety's scale.
fn place(v: &Variety, terrain: &Terrain, x: f32, z: f32, h: u64) -> Transform {
    let y = terrain.height(x, z);
    let mut rot = if v.random_rotation { Quat::from_rotation_y(unit(h, 4) * std::f32::consts::TAU) } else { Quat::IDENTITY };
    if v.align {
        let d = 0.5;
        let n = Vec3::new(terrain.height(x - d, z) - terrain.height(x + d, z), 2.0 * d, terrain.height(x, z - d) - terrain.height(x, z + d)).normalize();
        rot = Quat::from_rotation_arc(Vec3::Y, n) * rot;
    }
    let lerp = |[a, b]: [f32; 2], t: f32| a + (b - a) * t;
    let sx = lerp(v.scale[0], unit(h, 5));
    // UE X, Y, Z -> Bevy x, z, y.
    let scale = if v.uniform { Vec3::splat(sx) } else { Vec3::new(sx, lerp(v.scale[2], unit(h, 7)), lerp(v.scale[1], unit(h, 6))) };
    Transform { translation: Vec3::new(x, y, z), rotation: rot, scale }
}

#[derive(Deserialize)]
struct TerrainLayers {
    layers: Vec<String>,
}

pub(super) fn setup_grass(mut commands: Commands, terrain: Option<Res<Terrain>>, assets: Res<AssetServer>) {
    let Some(terrain) = terrain else { return };
    // terrain.json's layer order is the weight channel order.
    let Some(layers) = std::fs::read_to_string(format!("{}/terrain.json", super::LEVEL_DIR)).ok().and_then(|t| serde_json::from_str::<TerrainLayers>(&t).ok()) else { return };
    let Some(GrassFile(file)) = std::fs::read_to_string("assets/data/grass.json").ok().and_then(|t| serde_json::from_str(&t).ok()) else { return };
    let Some(weights) = Weights::load(super::LEVEL_DIR, &terrain) else { return };
    let mut varieties = Vec::new();
    for (channel, layer) in layers.layers.iter().enumerate() {
        for def in file.get(layer).into_iter().flatten() {
            let gltf = assets.load(format!("game/{}.glb", def.mesh));
            varieties.push(LoadedVariety { def: def.clone(), channel, gltf, parts: None });
        }
    }
    commands.insert_resource(Grass { varieties, weights, cells: HashMap::new() });
}

pub(super) fn reset_grass(mut commands: Commands) {
    commands.remove_resource::<Grass>();
}

/// Resolve each variety's glb into drawable parts (its own materials, or the override).
pub(super) fn resolve_grass(
    grass: Option<ResMut<Grass>>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut lib: ResMut<GameMaterials>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut grass) = grass else { return };
    for v in grass.varieties.iter_mut().filter(|v| v.parts.is_none()) {
        if assets.load_state(&v.gltf).is_failed() {
            v.parts = Some(Vec::new());
            continue;
        }
        let Some(gltf) = gltfs.get(&v.gltf) else { continue };
        let names: HashMap<AssetId<_>, &str> = gltf.named_materials.iter().map(|(n, h)| (h.id(), &**n)).collect();
        let mut parts = Vec::new();
        let mut ready = true;
        for gm in &gltf.meshes {
            let Some(gm) = gltf_meshes.get(gm) else {
                ready = false;
                break;
            };
            for p in &gm.primitives {
                let own = p.material.as_ref().and_then(|h| names.get(&h.id()).copied()).unwrap_or("");
                let mat = v.def.material.as_deref().and_then(|m| lib.get(m, &assets, &mut materials)).or_else(|| lib.get(own, &assets, &mut materials));
                if let Some(mut mesh) = meshes.get_mut(&p.mesh)
                    && mesh.contains_attribute(Mesh::ATTRIBUTE_COLOR)
                {
                    mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR);
                }
                parts.push((p.mesh.clone(), mat.unwrap_or_else(|| materials.add(StandardMaterial::default()))));
            }
        }
        if ready {
            v.parts = Some(parts);
        }
    }
}

/// Which varieties reach a cell from `eye`: bit per variety whose EndCullDistance covers it.
fn reach(varieties: &[LoadedVariety], cell: IVec2, eye: Vec3) -> u64 {
    let centre = Vec2::new((cell.x as f32 + 0.5) * CELL, (cell.y as f32 + 0.5) * CELL);
    let d = (centre.distance(eye.xz()) - CELL * 0.7).max(0.0);
    varieties.iter().enumerate().filter(|(_, v)| d <= v.def.cull[1]).fold(0, |m, (i, _)| m | (1u64 << i.min(63)))
}

pub(super) fn stream_grass(
    mut commands: Commands,
    grass: Option<ResMut<Grass>>,
    terrain: Option<Res<Terrain>>,
    players: Query<&Transform, With<crate::models::CharacterModel>>,
) {
    let (Some(mut grass), Some(terrain)) = (grass, terrain) else { return };
    let Some(eye) = players.iter().next().map(|t| t.translation) else { return };
    if grass.varieties.iter().any(|v| v.parts.is_none()) {
        return;
    }
    let far = grass.varieties.iter().map(|v| v.def.cull[1]).fold(0.0, f32::max) + CELL;
    let (c0, c1) = (((eye.xz() - Vec2::splat(far)) / CELL).floor().as_ivec2(), ((eye.xz() + Vec2::splat(far)) / CELL).floor().as_ivec2());
    // Drop cells out of reach or whose variety set changed.
    let stale: Vec<IVec2> = grass.cells.iter().filter(|(c, (_, m))| reach(&grass.varieties, **c, eye) != *m).map(|(c, _)| *c).collect();
    for c in stale {
        if let Some((e, _)) = grass.cells.remove(&c) {
            commands.entity(e).despawn();
        }
    }
    let mut todo: Vec<(IVec2, u64)> = Vec::new();
    for cz in c0.y..=c1.y {
        for cx in c0.x..=c1.x {
            let c = IVec2::new(cx, cz);
            let mask = reach(&grass.varieties, c, eye);
            if mask != 0 && !grass.cells.contains_key(&c) {
                todo.push((c, mask));
            }
        }
    }
    todo.sort_by_key(|(c, _)| ((c.as_vec2() + 0.5) * CELL).distance_squared(eye.xz()) as i64);
    for (c, mask) in todo.into_iter().take(BUDGET) {
        let cell = commands.spawn((Transform::default(), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        let g = &*grass;
        for (i, v) in g.varieties.iter().enumerate().filter(|(i, _)| mask & (1u64 << (*i).min(63)) != 0) {
            let parts = v.parts.as_deref().unwrap_or_default();
            for (x, z, h) in spots(&v.def, i as u64, c) {
                let w = (g.weights.get(v.channel, x, z) - g.weights.get(NO_FOLIAGE, x, z)).max(0.0);
                if unit(h, 8) >= w {
                    continue;
                }
                let t = place(&v.def, &terrain, x, z, h);
                for (mesh, mat) in parts {
                    let mut e = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), t, ChildOf(cell)));
                    if !v.def.shadows {
                        e.insert(NotShadowCaster);
                    }
                }
            }
        }
        grass.cells.insert(c, (cell, mask));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variety(density: f32, grid: bool) -> Variety {
        Variety {
            mesh: String::new(),
            material: None,
            density,
            cull: [70.0, 80.0],
            scale: [[1.0, 1.5]; 3],
            uniform: false,
            grid,
            jitter: 1.0,
            random_rotation: true,
            align: true,
            shadows: true,
        }
    }

    #[test]
    fn density_is_per_ten_by_ten_metres() {
        // 40 per 100 m2 on a 16 m cell: ~102-121 grid spots (10-11 per row); 1 per 100 m2 random: ~2.56.
        let n = spots(&variety(40.0, true), 0, IVec2::ZERO).len();
        assert!((90..=130).contains(&n), "{n}");
        let total: usize = (0..200).map(|i| spots(&variety(1.0, false), 0, IVec2::new(i, 0)).len()).sum();
        assert!((450..=575).contains(&total), "{total}");
    }

    #[test]
    fn grid_cells_tile_without_seams_or_overlap() {
        let v = variety(10.0, true);
        let a = spots(&v, 3, IVec2::new(0, 0));
        let b = spots(&v, 3, IVec2::new(1, 0));
        assert!(a.iter().all(|s| s.0 < CELL + 2.0) && b.iter().all(|s| s.0 >= CELL - 2.0));
        // Same spots every time (deterministic per world position).
        assert_eq!(spots(&v, 3, IVec2::new(5, -2)), spots(&v, 3, IVec2::new(5, -2)));
    }

    #[test]
    fn exported_layers_resolve_to_terrain_channels() {
        let Ok(text) = std::fs::read_to_string("assets/data/grass.json") else { return };
        let GrassFile(file) = serde_json::from_str(&text).unwrap();
        assert_eq!(file.len(), 11);
        let grass = &file["Grass_LayerInfo"];
        let patch = grass.iter().find(|v| v.mesh.ends_with("SM_GrassMeshPatch_03")).unwrap();
        assert_eq!((patch.density, patch.cull), (40.0, [70.0, 80.0]));
        assert!(file["Rock_LayerInfo"].is_empty(), "bare rock grows nothing");
    }
}
