//! Landscape from `SdExport level`: the 16-bit heightmap becomes chunked meshes streamed around
//! the camera with distance LOD, drawn with a splat material that blends the Terrain_Material
//! paint layers (`extractor/gen_terrain.py` packs weights and albedos into texture arrays).

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageArrayLayout, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use serde::Deserialize;

use crate::menus::AppState;

/// Quads per chunk edge (1 m each at full detail).
const CHUNK: i32 = 128;
/// How far under the surface still counts as being on (or in) the landscape rather than in a
/// space below it, metres.
const UNDERGROUND: f32 = 1.5;

/// Chunks further than this (in metres, chunk centre to camera) aren't drawn.
const VIEW: f32 = 1800.0;

#[derive(Deserialize)]
struct LandscapeInfo {
    location: [f32; 3],
    scale: [f32; 3],
    section_min: [i32; 2],
    heightmap_size: [usize; 2],
}

#[derive(Deserialize)]
struct TerrainLayers {
    tiling: Vec<f32>,
}

/// Heights in Bevy space: `height(x, z)` for any world position over the landscape.
#[derive(Resource, Clone)]
pub struct Terrain {
    heights: std::sync::Arc<Vec<f32>>,
    /// The landscape's visibility (hole) layer, `DataLayer`: 0..255 per sample; empty if absent.
    holes: std::sync::Arc<Vec<u8>>,
    pub size: [usize; 2],
    /// World (Bevy) x/z of sample (0, 0) and spacing between samples, metres.
    pub origin: Vec2,
    pub spacing: f32,
}

impl Terrain {
    pub fn load(dir: &str) -> Option<Self> {
        let info: LandscapeInfo = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/landscape.json")).ok()?).ok()?;
        let img = image::open(format!("{dir}/heightmap.png")).ok()?.into_luma16();
        let [w, h] = info.heightmap_size;
        // UE: z = loc.z + (h - 32768) / 128 * scale.z  (cm)
        let heights = img.pixels().map(|p| (info.location[2] + (p.0[0] as f32 - 32768.0) / 128.0 * info.scale[2]) / 100.0).collect();
        let holes = image::open(format!("{dir}/weight_DataLayer.png")).ok().map(|i| i.into_luma8().into_raw()).filter(|v| v.len() == w * h).unwrap_or_default();
        Some(Self {
            heights: std::sync::Arc::new(heights),
            holes: std::sync::Arc::new(holes),
            size: [w, h],
            origin: Vec2::new(
                (info.location[0] + info.section_min[0] as f32 * info.scale[0]) / 100.0,
                (info.location[1] + info.section_min[1] as f32 * info.scale[1]) / 100.0,
            ),
            spacing: info.scale[0] / 100.0,
        })
    }

    fn sample(&self, i: i32, j: i32) -> f32 {
        let i = i.clamp(0, self.size[0] as i32 - 1) as usize;
        let j = j.clamp(0, self.size[1] as i32 - 1) as usize;
        self.heights[j * self.size[0] + i]
    }

    fn hole_weight(&self, i: i32, j: i32) -> f32 {
        if self.holes.is_empty() {
            return 0.0;
        }
        let i = i.clamp(0, self.size[0] as i32 - 1) as usize;
        let j = j.clamp(0, self.size[1] as i32 - 1) as usize;
        self.holes[j * self.size[0] + i] as f32 / 255.0
    }

    /// Is the landscape cut away at world x/z? (The visibility layer's opacity mask: the
    /// interpolated weight at 0.5 or more.)
    pub fn hole(&self, x: f32, z: f32) -> bool {
        if self.holes.is_empty() {
            return false;
        }
        let gx = (x - self.origin.x) / self.spacing;
        let gz = (z - self.origin.y) / self.spacing;
        let (i, j) = (gx.floor() as i32, gz.floor() as i32);
        let (fx, fz) = (gx - i as f32, gz - j as f32);
        let a = self.hole_weight(i, j) * (1.0 - fx) + self.hole_weight(i + 1, j) * fx;
        let b = self.hole_weight(i, j + 1) * (1.0 - fx) + self.hole_weight(i + 1, j + 1) * fx;
        a * (1.0 - fz) + b * fz >= 0.5
    }

    /// The landscape as a floor for something at `p`: its height, unless there's a hole here or
    /// `p` is already underground (through a hole: labs, bunkers, cellars).
    pub fn ground_under(&self, p: Vec3) -> Option<f32> {
        let h = self.height(p.x, p.z);
        (!self.hole(p.x, p.z) && p.y > h - UNDERGROUND).then_some(h)
    }

    /// Is `p` inside the landscape's surface (blocks sight and shots)? Underground spaces below
    /// it aren't.
    pub fn blocks(&self, p: Vec3) -> bool {
        let h = self.height(p.x, p.z);
        p.y < h && p.y > h - UNDERGROUND && !self.hole(p.x, p.z)
    }

    /// Bilinear ground height at world x/z.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let gx = (x - self.origin.x) / self.spacing;
        let gz = (z - self.origin.y) / self.spacing;
        let (i, j) = (gx.floor() as i32, gz.floor() as i32);
        let (fx, fz) = (gx - i as f32, gz - j as f32);
        let a = self.sample(i, j) * (1.0 - fx) + self.sample(i + 1, j) * fx;
        let b = self.sample(i, j + 1) * (1.0 - fx) + self.sample(i + 1, j + 1) * fx;
        a * (1.0 - fz) + b * fz
    }

    /// Surface normal at world x/z (central differences over one grid step).
    pub fn normal(&self, x: f32, z: f32) -> Vec3 {
        let s = self.spacing;
        let dx = self.height(x + s, z) - self.height(x - s, z);
        let dz = self.height(x, z + s) - self.height(x, z - s);
        Vec3::new(-dx, 2.0 * s, -dz).normalize()
    }

    pub fn extent(&self) -> Vec2 {
        Vec2::new((self.size[0] - 1) as f32, (self.size[1] - 1) as f32) * self.spacing
    }

    fn chunks(&self) -> IVec2 {
        IVec2::new((self.size[0] as i32 - 1 + CHUNK - 1) / CHUNK, (self.size[1] as i32 - 1 + CHUNK - 1) / CHUNK)
    }

    /// Mesh for chunk `c` sampling every `step` heights, with a skirt that hides cracks
    /// against neighbours at a different LOD.
    fn chunk_mesh(&self, c: IVec2, step: i32) -> Mesh {
        let n = CHUNK / step + 1;
        let (i0, j0) = (c.x * CHUNK, c.y * CHUNK);
        let world = |i: i32, j: i32| Vec3::new(self.origin.x + i as f32 * self.spacing, self.sample(i, j), self.origin.y + j as f32 * self.spacing);
        let mut pos = Vec::with_capacity((n * n + 4 * n) as usize);
        let mut nrm = Vec::with_capacity(pos.capacity());
        let mut uv = Vec::with_capacity(pos.capacity());
        for gj in 0..n {
            for gi in 0..n {
                let (i, j) = (i0 + gi * step, j0 + gj * step);
                let p = world(i, j);
                let dx = self.sample(i + step, j) - self.sample(i - step, j);
                let dz = self.sample(i, j + step) - self.sample(i, j - step);
                pos.push(p.to_array());
                nrm.push(Vec3::new(-dx, 2.0 * step as f32 * self.spacing, -dz).normalize().to_array());
                uv.push([p.x, p.z]);
            }
        }
        let mut idx: Vec<u32> = Vec::with_capacity((n * n * 6) as usize);
        let at = |gi: i32, gj: i32| (gj * n + gi) as u32;
        for gj in 0..n - 1 {
            for gi in 0..n - 1 {
                // Holes: quads whose centre is masked out by the visibility layer.
                let (ci, cj) = (i0 + gi * step + step / 2, j0 + gj * step + step / 2);
                let centre = (self.hole_weight(ci, cj) + self.hole_weight(ci + 1, cj) + self.hole_weight(ci, cj + 1) + self.hole_weight(ci + 1, cj + 1)) / 4.0;
                if step == 1 && centre >= 0.5 || step > 1 && self.hole_weight(ci, cj) >= 0.5 {
                    continue;
                }
                let (a, b, c, d) = (at(gi, gj), at(gi + 1, gj), at(gi, gj + 1), at(gi + 1, gj + 1));
                idx.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
        // Skirt: duplicate each edge ring lowered by a few metres; both windings so it shows
        // from either side.
        let drop = 2.0 + step as f32 * 2.0;
        let edges: [Vec<(i32, i32)>; 4] = [
            (0..n).map(|g| (g, 0)).collect(),
            (0..n).map(|g| (g, n - 1)).collect(),
            (0..n).map(|g| (0, g)).collect(),
            (0..n).map(|g| (n - 1, g)).collect(),
        ];
        for edge in edges {
            let base = pos.len() as u32;
            for &(gi, gj) in &edge {
                let k = at(gi, gj) as usize;
                let mut p = pos[k];
                p[1] -= drop;
                pos.push(p);
                nrm.push(nrm[k]);
                uv.push(uv[k]);
            }
            for e in 0..edge.len() as u32 - 1 {
                let (t0, t1) = (at(edge[e as usize].0, edge[e as usize].1), at(edge[e as usize + 1].0, edge[e as usize + 1].1));
                let (b0, b1) = (base + e, base + e + 1);
                idx.extend_from_slice(&[t0, b0, t1, t1, b0, b1, t0, t1, b0, t1, b1, b0]);
            }
        }
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
            .with_inserted_indices(Indices::U32(idx))
    }
}

#[derive(Clone, Copy, ShaderType, Debug, Reflect)]
pub struct TerrainParams {
    /// xy: world x/z of the weight map's (0,0) corner; zw: world size covered.
    pub bounds: Vec4,
    /// UV repeats per metre for each of the 12 layers.
    pub tiling: [Vec4; 3],
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct TerrainExtension {
    #[uniform(100)]
    pub params: TerrainParams,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    pub weights: Handle<Image>,
    #[texture(103, dimension = "2d_array")]
    #[sampler(104)]
    pub layers: Handle<Image>,
}

impl MaterialExtension for TerrainExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain.wesl".into()
    }
}

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainExtension>;

#[derive(Resource)]
pub struct TerrainAssets {
    material: Handle<TerrainMaterial>,
    loaded: HashMap<IVec2, (Entity, i32)>,
}

#[derive(Component)]
pub struct TerrainChunk;

pub fn setup_terrain_material(dir: &str, terrain: &Terrain, assets: &AssetServer, materials: &mut Assets<TerrainMaterial>) -> TerrainAssets {
    let layers: TerrainLayers = std::fs::read_to_string(format!("{dir}/terrain.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(TerrainLayers { tiling: vec![0.2; 12] });
    let mut t = [0.2f32; 12];
    for (i, v) in layers.tiling.iter().take(12).enumerate() {
        t[i] = *v;
    }
    let repeat = ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    };
    let weights = assets
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| {
            s.array_layout = Some(ImageArrayLayout::RowCount { rows: 3 });
            s.is_srgb = false;
            s.sampler = ImageSampler::linear();
        })
        .load(format!("{dir}/terrain_weights.png"));
    let layer_tex = assets
        .load_builder()
        .with_settings(move |s: &mut ImageLoaderSettings| {
            s.array_layout = Some(ImageArrayLayout::RowCount { rows: 12 });
            s.sampler = ImageSampler::Descriptor(repeat.clone());
        })
        .load(format!("{dir}/terrain_layers.png"));
    let ext = terrain.extent();
    let material = materials.add(ExtendedMaterial {
        base: StandardMaterial { perceptual_roughness: 0.95, reflectance: 0.2, ..default() },
        extension: TerrainExtension {
            params: TerrainParams {
                bounds: Vec4::new(terrain.origin.x, terrain.origin.y, ext.x, ext.y),
                tiling: [Vec4::from_slice(&t[0..4]), Vec4::from_slice(&t[4..8]), Vec4::from_slice(&t[8..12])],
            },
            weights,
            layers: layer_tex,
        },
    });
    TerrainAssets { material, loaded: HashMap::new() }
}

fn lod_step(distance: f32) -> i32 {
    match distance {
        d if d < 200.0 => 1,
        d if d < 450.0 => 2,
        d if d < 900.0 => 4,
        d if d < 1400.0 => 8,
        _ => 16,
    }
}

/// Keep chunks around the camera at the right LOD; rebuilds a few per frame to avoid hitches.
pub fn stream_terrain(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    state: Option<ResMut<TerrainAssets>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Some(terrain), Some(mut state)) = (terrain, state) else { return };
    let Some(cam) = cameras.iter().next() else { return };
    let cam = cam.translation();
    let chunks = terrain.chunks();
    let size = CHUNK as f32 * terrain.spacing;

    let mut want: Vec<(f32, IVec2, i32)> = Vec::new();
    for cj in 0..chunks.y {
        for ci in 0..chunks.x {
            let centre = terrain.origin + (Vec2::new(ci as f32, cj as f32) + 0.5) * size;
            let d = (centre - cam.xz()).length() - size * 0.5;
            if d < VIEW {
                want.push((d, IVec2::new(ci, cj), lod_step(d.max(0.0))));
            }
        }
    }
    want.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut budget = 6;
    for &(_, c, step) in &want {
        if state.loaded.get(&c).is_some_and(|(_, s)| *s == step) {
            continue;
        }
        if budget == 0 {
            break;
        }
        budget -= 1;
        let mesh = meshes.add(terrain.chunk_mesh(c, step));
        let e = commands
            .spawn((Mesh3d(mesh), MeshMaterial3d(state.material.clone()), Transform::default(), TerrainChunk, DespawnOnExit(AppState::InGame)))
            .id();
        if let Some((old, _)) = state.loaded.insert(c, (e, step)) {
            commands.entity(old).despawn();
        }
    }
    let keep: std::collections::HashSet<IVec2> = want.iter().map(|w| w.1).collect();
    let drop: Vec<IVec2> = state.loaded.keys().filter(|c| !keep.contains(c)).copied().collect();
    for c in drop {
        if let Some((e, _)) = state.loaded.remove(&c) {
            commands.entity(e).despawn();
        }
    }
}

/// Forget chunk entities when the world is torn down (they're despawned by DespawnOnExit).
pub fn reset_terrain(mut state: Option<ResMut<TerrainAssets>>) {
    if let Some(s) = state.as_mut() {
        s.loaded.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landscape_holes_open_the_lab_entrance() {
        let Some(t) = Terrain::load(crate::level::LEVEL_DIR) else { return };
        // The laboratory's entrance (a 4 m hole in the visibility layer) and ground beside it.
        assert!(t.hole(1754.5, 166.5));
        assert!(!t.hole(1745.0, 166.5));
        let h = t.height(1745.0, 166.5);
        assert_eq!(t.ground_under(Vec3::new(1745.0, h + 0.2, 166.5)), Some(h));
        assert_eq!(t.ground_under(Vec3::new(1754.5, h + 0.2, 166.5)), None, "no floor over the hole");
        // Underground (the lab, ~8 m down): the surface overhead is neither floor nor wall.
        let below = Vec3::new(1745.0, h - 8.0, 166.5);
        assert_eq!(t.ground_under(below), None);
        assert!(!t.blocks(below) && t.blocks(Vec3::new(1745.0, h - 0.5, 166.5)));
        // The normal is unit length, points up, and leans downhill.
        let n = t.normal(1745.0, 166.5);
        assert!((n.length() - 1.0).abs() < 1e-4 && n.y > 0.0);
        let downhill = t.height(1745.0 + n.x, 166.5 + n.z);
        assert!(downhill <= h + 1e-3, "{downhill} vs {h}");
    }
}
