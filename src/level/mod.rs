//! LongdownValley, imported from the game's `.umap` (`extractor/export_assets.sh --level`).
//!
//! `assets/game/levels/LongdownValley/` holds:
//! - `landscape.json`, `heightmap.png`, `terrain_*.png`: the landscape ([`terrain`]);
//! - `placements.json`: every static mesh / foliage instance as Bevy-space transforms;
//! - `splines.json`: spline meshes (roads, fences, cables), deformed at load ([`spline`]);
//! - `actors.json`: every actor's class and transform (PlayerStarts, spawners, containers...);
//!   spawners also carry their property overrides ([`spawners`]).
//!
//! Props and splines are bucketed into [`CELL`]-metre cells and streamed around the camera,
//! except door meshes, which [`doors`] spawns itself so they can move.

pub mod collision;
pub mod doors;
pub mod garage;
pub mod ladders;
pub mod power;
pub mod grass;
pub mod item_spawners;
pub mod lights;
pub mod spawners;
pub mod spline;
pub mod terrain;

use std::collections::{HashMap, HashSet};

use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::*;
use serde::Deserialize;

use crate::menus::AppState;
use crate::models::GameMaterials;
use spline::SplineMesh;
pub use collision::KinematicBody;
pub use spawners::LevelZombie;
pub use terrain::Terrain;
use terrain::{TerrainMaterial, reset_terrain, setup_terrain_material, stream_terrain};

pub const LEVEL_DIR: &str = "assets/game/levels/LongdownValley";
const ASSET_DIR: &str = "game/levels/LongdownValley";
pub(crate) const CELL: f32 = 64.0;
/// Props are kept within this radius of the camera (fog hides the edge).
const PROP_RADIUS: f32 = 420.0;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default())
            .add_systems(OnEnter(AppState::InGame), (enter_level, grass::setup_grass, lights::setup_lights, ladders::setup_ladders, power::setup_power, item_spawners::setup_item_spawns, item_spawners::spawn_item_containers, garage::setup_garages).chain())
            .init_resource::<collision::CollisionCache>()
            .init_resource::<doors::Doors>()
            .init_resource::<garage::Garages>()
            .add_systems(OnExit(AppState::InGame), (reset_terrain, reset_props, grass::reset_grass, lights::reset_lights, doors::reset_doors))
            .add_systems(
                PostUpdate,
                (ladders::climb_ladders, collision::move_bodies)
                    .chain()
                    .before(bevy::transform::TransformSystems::Propagate)
                    .run_if(in_state(AppState::InGame))
                    .run_if(in_state(crate::menus::MenuScreen::None)),
            )
            .add_systems(
                Update,
                (stream_terrain, request_props, resolve_props, spawn_cells, spawners::run_spawners, spawners::optimize_subjects, grass::resolve_grass, grass::stream_grass, lights::update_lights, spin_turbines)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                (
                    (doors::load_doors, doors::spawn_doors, doors::move_doors, garage::move_garages, power::status_lamps).chain().after(resolve_props),
                    doors::interact_doors.after(crate::gameplay::interact::search_containers),
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Deserialize, Clone)]
pub struct ActorInfo {
    pub class: String,
    pub name: String,
    pub transform: [f32; 12],
    /// Per-instance property overrides (spawners only), values as JSON text.
    #[serde(default)]
    pub props: Option<HashMap<String, String>>,
}

impl ActorInfo {
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.transform[9], self.transform[10], self.transform[11])
    }
    pub fn yaw(&self) -> f32 {
        // Column 0 is the actor's forward (UE X) in Bevy space.
        let fwd = Vec3::new(self.transform[0], self.transform[1], self.transform[2]);
        fwd.x.atan2(fwd.z)
    }
}

pub fn affine(m: &[f32; 12]) -> Transform {
    let a = Mat4::from_cols(
        Vec4::new(m[0], m[1], m[2], 0.0),
        Vec4::new(m[3], m[4], m[5], 0.0),
        Vec4::new(m[6], m[7], m[8], 0.0),
        Vec4::new(m[9], m[10], m[11], 1.0),
    );
    Transform::from_matrix(a)
}

/// Level data that doesn't change between sessions; loaded once.
#[derive(Resource)]
pub struct Level {
    pub actors: Vec<ActorInfo>,
    meshes: Vec<PropMesh>,
    splines: Vec<SplineMesh>,
    cells: HashMap<IVec2, Vec<Item>>,
    /// Interactive doors (their meshes are not in `cells`).
    pub doors: Vec<doors::DoorDef>,
}

struct PropMesh {
    path: String,
    gltf: Option<Handle<Gltf>>,
    /// Resolved draw parts. `Some(vec![])` if the file failed to load.
    parts: Option<Vec<Part>>,
}

struct Part {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    /// Characters collide with it (not leaves, grass, plants, rubbish decals).
    collide: bool,
    /// A status lamp the Blueprint recolours (material slot 1 of a lab switch or door frame).
    lamp: Option<power::LampKind>,
}

/// Materials characters walk through: foliage cards and ground clutter.
fn walk_through(material: &str) -> bool {
    const SOFT: [&str; 12] = ["Leaves", "Leaf", "Plant", "Grass", "Bush", "Seaweed", "Fern", "Flower", "Ivy", "Vine", "Rubbish", "Decal"];
    SOFT.iter().any(|k| material.contains(k)) || (material.contains("Tree") && !material.contains("Trunk"))
}

#[derive(Clone, Copy)]
enum Item {
    Static { mesh: u32, transform: Transform },
    Spline { index: u32, mesh: u32 },
}

fn cell_of(p: Vec3) -> IVec2 {
    IVec2::new((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32)
}

impl Level {
    fn load() -> Option<Self> {
        let read = |f: &str| std::fs::read_to_string(format!("{LEVEL_DIR}/{f}")).ok();
        let placements: HashMap<String, Vec<[f32; 12]>> = serde_json::from_str(&read("placements.json")?).ok()?;
        let splines: Vec<SplineMesh> = serde_json::from_str(&read("splines.json").unwrap_or_else(|| "[]".into())).ok()?;
        let actors: Vec<ActorInfo> = serde_json::from_str(&read("actors.json")?).ok()?;

        let mut meshes = Vec::new();
        let mut index = HashMap::new();
        let mut mesh_id = |path: &str, meshes: &mut Vec<PropMesh>| -> u32 {
            *index.entry(path.to_string()).or_insert_with(|| {
                meshes.push(PropMesh { path: format!("game/{path}.glb"), gltf: None, parts: None });
                meshes.len() as u32 - 1
            })
        };
        let (found, taken) = doors::find(&actors, &placements);
        let mut cells: HashMap<IVec2, Vec<Item>> = HashMap::new();
        for (path, list) in &placements {
            let id = mesh_id(path, &mut meshes);
            for (k, m) in list.iter().enumerate() {
                if taken.contains(&(path.clone(), k)) {
                    continue;
                }
                let transform = affine(m);
                cells.entry(cell_of(transform.translation)).or_default().push(Item::Static { mesh: id, transform });
            }
        }
        for (i, s) in splines.iter().enumerate() {
            let id = mesh_id(&s.mesh, &mut meshes);
            cells.entry(cell_of(affine(&s.transform).translation)).or_default().push(Item::Spline { index: i as u32, mesh: id });
        }
        let doors = found
            .into_iter()
            .map(|(mut d, paths)| {
                for (part, path) in d.parts.iter_mut().zip(&paths) {
                    part.mesh = mesh_id(path, &mut meshes);
                }
                d
            })
            .collect::<Vec<_>>();
        info!("level: {} meshes, {} cells, {} actors, {} doors", meshes.len(), cells.len(), actors.len(), doors.len());
        Some(Self { actors, meshes, splines, cells, doors })
    }

    /// PlayerStarts straight from disk, for callers that run before [`Level`] is inserted.
    pub fn peek_player_starts() -> Option<Vec<Vec3>> {
        let actors: Vec<ActorInfo> = serde_json::from_str(&std::fs::read_to_string(format!("{LEVEL_DIR}/actors.json")).ok()?).ok()?;
        Some(actors.iter().filter(|a| a.class == "PlayerStart").map(|a| a.position()).collect())
    }

    /// PlayerStart actors (where a new character can begin).
    pub fn player_starts(&self) -> Vec<Vec3> {
        self.actors.iter().filter(|a| a.class == "PlayerStart").map(|a| a.position()).collect()
    }
}

/// Per-session streaming state.
#[derive(Resource, Default)]
pub struct PropStreaming {
    loaded: HashMap<IVec2, Entity>,
    /// Deformed spline meshes, kept so re-entering a cell doesn't redo the work.
    /// Deformed spline meshes: (mesh, material, characters collide with it).
    spline_meshes: HashMap<u32, Vec<(Handle<Mesh>, Handle<StandardMaterial>, bool)>>,
    wanted: Vec<IVec2>,
}

fn enter_level(
    mut commands: Commands,
    level: Option<Res<Level>>,
    terrain: Option<Res<Terrain>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    if level.is_none() {
        match Level::load() {
            Some(l) => commands.insert_resource(l),
            None => {
                warn!("{LEVEL_DIR} missing; run extractor/export_assets.sh --level");
                return;
            }
        }
    }
    let terrain = match terrain {
        Some(t) => t.clone(),
        None => match Terrain::load(LEVEL_DIR) {
            Some(t) => {
                commands.insert_resource(t.clone());
                t
            }
            None => return,
        },
    };
    commands.insert_resource(setup_terrain_material(ASSET_DIR, &terrain, &assets, &mut materials));
    commands.insert_resource(PropStreaming::default());
    commands.insert_resource(spawners::Spawners::default());
}

fn reset_props(mut commands: Commands) {
    commands.remove_resource::<PropStreaming>();
}

/// Work out which cells should be loaded and start loading their meshes.
fn request_props(
    level: Option<ResMut<Level>>,
    state: Option<ResMut<PropStreaming>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    assets: Res<AssetServer>,
) {
    let (Some(mut level), Some(mut state)) = (level, state) else { return };
    let Some(cam) = cameras.iter().next() else { return };
    let cam = cam.translation();
    let r = (PROP_RADIUS / CELL).ceil() as i32;
    let c0 = cell_of(cam);
    let mut wanted = Vec::new();
    for dz in -r..=r {
        for dx in -r..=r {
            let c = c0 + IVec2::new(dx, dz);
            let centre = (c.as_vec2() + 0.5) * CELL;
            if (centre - cam.xz()).length() <= PROP_RADIUS + CELL && level.cells.contains_key(&c) {
                wanted.push(c);
            }
        }
    }
    wanted.sort_by_key(|c| (*c - c0).length_squared());
    for c in &wanted {
        let ids: Vec<u32> = level.cells[c]
            .iter()
            .map(|i| match i {
                Item::Static { mesh, .. } | Item::Spline { mesh, .. } => *mesh,
            })
            .collect();
        for id in ids {
            let m = &mut level.meshes[id as usize];
            if m.gltf.is_none() {
                m.gltf = Some(assets.load(m.path.clone()));
            }
        }
    }
    state.wanted = wanted;
}

/// Turn loaded glTFs into (mesh, material) pairs with the game's materials applied.
fn resolve_props(
    level: Option<ResMut<Level>>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut lib: ResMut<GameMaterials>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut level) = level else { return };
    for m in level.meshes.iter_mut().filter(|m| m.parts.is_none()) {
        let Some(handle) = &m.gltf else { continue };
        if assets.load_state(handle).is_failed() {
            m.parts = Some(Vec::new());
            continue;
        }
        let Some(gltf) = gltfs.get(handle) else { continue };
        let names: HashMap<AssetId<_>, &str> = gltf.named_materials.iter().map(|(n, h)| (h.id(), &**n)).collect();
        // BP_WaterBox / BP_WaterSpline surfaces (SplineWaterTEST, PlaneSplineMesh): the BPs draw
        // them with Mat_Clear_Water and they're water volumes, not floors.
        let water = m.path.contains("/Terrain/Water/");
        let mut parts = Vec::new();
        let mut ready = true;
        for gm in &gltf.meshes {
            let Some(gm) = gltf_meshes.get(gm) else {
                ready = false;
                break;
            };
            for p in &gm.primitives {
                let name = p.material.as_ref().and_then(|h| names.get(&h.id()).copied()).unwrap_or("");
                if name == "Invisible" {
                    continue;
                }
                if water {
                    parts.push(Part { mesh: p.mesh.clone(), material: lib.water(&mut materials), collide: false, lamp: None });
                    continue;
                }
                let mat = lib.get(name, &assets, &mut materials).unwrap_or_else(|| materials.add(StandardMaterial::default()));
                // Trees: trunk triangles get the material's trunk colour.
                if let Some(trunk_mat) = lib.trunk(name, &mut materials)
                    && let Some((leaves, trunk)) = meshes.get(&p.mesh).and_then(|m| crate::models::split_by_vertex_alpha(&m))
                {
                    parts.push(Part { mesh: meshes.add(leaves), material: mat, collide: false, lamp: None });
                    parts.push(Part { mesh: meshes.add(trunk), material: trunk_mat, collide: true, lamp: None });
                    continue;
                }
                // UE materials ignore vertex colour; Bevy would multiply it in (see models).
                if let Some(mut mesh) = meshes.get_mut(&p.mesh)
                    && mesh.contains_attribute(Mesh::ATTRIBUTE_COLOR)
                {
                    mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR);
                }
                parts.push(Part { mesh: p.mesh.clone(), material: mat, collide: !walk_through(name), lamp: power::LampKind::of(&m.path, name) });
            }
        }
        if ready {
            m.parts = Some(parts);
        }
    }
}

/// BP_WindTurbine's blades (its `SM_Prop_Powerpole_Single_01` component, despite the name).
#[derive(Component)]
pub struct TurbineBlades;

/// `Event_Spin`: every 0.01 s the blades pitch by Actor_Weather_Status.Wind x 2 degrees (UDS
/// wind 0-1, our weather's Wind Intensity / 10), about their local pitch axis (UE Y = our Z).
fn spin_turbines(time: Res<Time>, weather: Res<crate::gameplay::weather::Weather>, mut blades: Query<&mut Transform, With<TurbineBlades>>) {
    let rate = (weather.0.wind / 10.0 * 2.0 / 0.01).to_radians();
    for mut t in &mut blades {
        t.rotate_local_z(rate * time.delta_secs());
    }
}

/// Spawn cells whose meshes are ready (a few per frame) and drop cells out of range.
fn spawn_cells(
    mut commands: Commands,
    level: Option<Res<Level>>,
    state: Option<ResMut<PropStreaming>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Some(level), Some(mut state)) = (level, state) else { return };
    let wanted: HashSet<IVec2> = state.wanted.iter().copied().collect();
    let gone: Vec<IVec2> = state.loaded.keys().filter(|c| !wanted.contains(c)).copied().collect();
    for c in gone {
        if let Some(e) = state.loaded.remove(&c) {
            commands.entity(e).despawn();
        }
    }

    let mut budget = 4;
    let order = state.wanted.clone();
    for c in order {
        if budget == 0 {
            break;
        }
        if state.loaded.contains_key(&c) {
            continue;
        }
        let items = &level.cells[&c];
        let ready = items.iter().all(|i| match i {
            Item::Static { mesh, .. } | Item::Spline { mesh, .. } => level.meshes[*mesh as usize].parts.is_some(),
        });
        if !ready {
            continue;
        }
        budget -= 1;
        let cell = commands.spawn((Transform::default(), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        for item in items {
            match *item {
                Item::Static { mesh, transform } => {
                    let spins = level.meshes[mesh as usize].path.ends_with("/SM_Prop_Windfarm_Blades_01.glb");
                    for p in level.meshes[mesh as usize].parts.as_deref().unwrap_or_default() {
                        let mut child = commands.spawn((Mesh3d(p.mesh.clone()), MeshMaterial3d(p.material.clone()), transform));
                        if spins {
                            child.insert(TurbineBlades);
                        }
                        if let Some(kind) = p.lamp {
                            child.insert(power::StatusLamp::new(kind, transform.translation));
                        }
                        let child = child.id();
                        commands.entity(cell).add_child(child);
                    }
                }
                Item::Spline { index, mesh } => {
                    let spline = &level.splines[index as usize];
                    let parts = state
                        .spline_meshes
                        .entry(index)
                        .or_insert_with(|| {
                            level.meshes[mesh as usize]
                                .parts
                                .as_deref()
                                .unwrap_or_default()
                                .iter()
                                .filter_map(|p| {
                                    let bent = spline.deform(&*meshes.get(&p.mesh)?)?;
                                    Some((meshes.add(bent), p.material.clone(), p.collide))
                                })
                                .collect()
                        })
                        .clone();
                    for (m, mat, _) in parts {
                        let child = commands.spawn((Mesh3d(m), MeshMaterial3d(mat), affine(&spline.transform))).id();
                        commands.entity(cell).add_child(child);
                    }
                }
            }
        }
        state.loaded.insert(c, cell);
    }
}
