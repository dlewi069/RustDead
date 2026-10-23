//! LongdownValley, imported from the game's `.umap` (`extractor/export_assets.sh --level`).
//!
//! `assets/game/levels/LongdownValley/` holds:
//! - `landscape.json`, `heightmap.png`, `terrain_*.png`: the landscape ([`terrain`]);
//! - `placements.json`: every static mesh / foliage instance as Bevy-space transforms;
//! - `splines.json`: spline meshes (roads, fences, cables), deformed at load ([`spline`]);
//! - `actors.json`: every actor's class and transform (PlayerStarts, spawners, containers...);
//!   spawners also carry their property overrides ([`spawners`]);
//! - `harvest.json`: the foliage instances whose surface is Tree or Stone, and the TrashObjects'
//!   meshes (`gameplay::harvest`).
//!
//! Props and splines are bucketed into [`CELL`]-metre cells and streamed around the camera,
//! except door meshes, which [`doors`] spawns itself so they can move.

pub mod collision;
pub mod doors;
pub mod garage;
pub mod ladders;
pub mod power;
pub mod prefabs;
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
            .init_resource::<prefabs::Prefabs>()
            .add_systems(OnEnter(AppState::InGame), prefabs::reset)
            .add_systems(
                Update,
                prefabs::run_prefabs
                    .after(spawners::run_spawners)
                    .run_if(resource_exists::<Level>)
                    .run_if(resource_exists::<PropStreaming>)
                    .run_if(in_state(AppState::InGame))
                    .run_if(in_state(crate::menus::MenuScreen::None)),
            )
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
                    (doors::load_doors, doors::spawn_doors, doors::move_doors, garage::move_garages, power::status_lamps, power::generator_hum).chain().after(resolve_props),
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
    /// Placed items (cell, index in it) a melee swing can harvest.
    pub harvest: HashMap<PropKey, Harvestable>,
    /// The prefab spawners, their prefabs' meshes placed in `cells` (hidden until spawned).
    pub prefabs: Vec<prefabs::PrefabSite>,
}

/// A prop's drawn mesh and material.
pub type DrawPart = (Handle<Mesh>, Handle<StandardMaterial>);

/// A placed prop: its cell and index in the cell's list.
pub type PropKey = (IVec2, u32);

/// What a melee swing can break: a foliage instance's physical surface, as BP_MeleePickup's hit
/// switch reads it, or a TrashObject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Harvestable {
    /// PhysMat_Tree: becomes BP_HarvestableObject_Tree when hit.
    Tree,
    /// PhysMat_Stone (the Resource_Rock foliage): becomes BP_HarvestableObject_Rocks.
    Stone,
    /// A TrashObject's mesh (its WorldObjectComponent takes melee damage).
    Trash,
}

/// Marks a streamed-in prop's draw entities that can be harvested away.
#[derive(Component)]
pub struct PropItem(pub PropKey);

struct PropMesh {
    path: String,
    /// The mesh's simple collision (`simple_collision.json`: the trees' BodySetup capsules and
    /// hulls), used instead of its drawn triangles.
    simple: Option<std::sync::Arc<Vec<[Vec3; 3]>>>,
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
    /// A BP_WaterBox / BP_WaterSpline surface: SwimmingComponent's water trace hits it.
    water: bool,
}

/// A prop mesh that's a water body's surface (BP_WaterBox's cube, BP_WaterSpline's plane).
fn is_water(path: &str) -> bool {
    path.contains("/Terrain/Water/")
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
        // { "tree": { mesh: [placement indices] }, "stone": {...}, "trash": {...} }
        let surfaces: HashMap<String, HashMap<String, Vec<usize>>> = read("harvest.json").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let mut surface_of: HashMap<(&str, usize), Harvestable> = HashMap::new();
        for (kind, h) in [("tree", Harvestable::Tree), ("stone", Harvestable::Stone), ("trash", Harvestable::Trash)] {
            for (mesh, list) in surfaces.get(kind).into_iter().flatten() {
                surface_of.extend(list.iter().map(|&k| ((mesh.as_str(), k), h)));
            }
        }
        let mut harvest = HashMap::new();

        let simple: HashMap<String, Vec<[f32; 9]>> = std::fs::read_to_string("assets/data/simple_collision.json").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let mut meshes = Vec::new();
        let mut index = HashMap::new();
        let mut mesh_id = |path: &str, meshes: &mut Vec<PropMesh>| -> u32 {
            *index.entry(path.to_string()).or_insert_with(|| {
                let simple = simple.get(path).map(|t| std::sync::Arc::new(t.iter().map(|v| [Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5]), Vec3::new(v[6], v[7], v[8])]).collect()));
                meshes.push(PropMesh { path: format!("game/{path}.glb"), simple, gltf: None, parts: None });
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
                let cell = cell_of(transform.translation);
                let list = cells.entry(cell).or_default();
                if let Some(&h) = surface_of.get(&(path.as_str(), k)) {
                    harvest.insert((cell, list.len() as u32), h);
                }
                list.push(Item::Static { mesh: id, transform });
            }
        }
        for (i, s) in splines.iter().enumerate() {
            let id = mesh_id(&s.mesh, &mut meshes);
            cells.entry(cell_of(affine(&s.transform).translation)).or_default().push(Item::Spline { index: i as u32, mesh: id });
        }
        // Each prefab spawner's prefab, its meshes in the cells like the level's own.
        let mut prefab_sites = Vec::new();
        for (i, a) in actors.iter().enumerate() {
            let Some(mut site) = prefabs::PrefabSite::from_actor(i, a) else { continue };
            let Some(def) = site.def() else { continue };
            let base = site.transform.to_matrix();
            for (mesh, m) in &def.meshes {
                let id = mesh_id(mesh, &mut meshes);
                let transform = Transform::from_matrix(base * affine(m).to_matrix());
                let cell = cell_of(transform.translation);
                let list = cells.entry(cell).or_default();
                site.keys.push((cell, list.len() as u32));
                list.push(Item::Static { mesh: id, transform });
            }
            prefab_sites.push(site);
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
        info!("level: {} meshes, {} cells, {} actors, {} doors, {} harvestable, {} prefab sites", meshes.len(), cells.len(), actors.len(), doors.len(), harvest.len(), prefab_sites.len());
        Some(Self { actors, meshes, splines, cells, doors, harvest, prefabs: prefab_sites })
    }

    /// Harvestable props of `kind` within `r` of `p` (horizontally): key and position, nearest first.
    pub fn harvestables_near(&self, p: Vec3, r: f32, kind: Harvestable) -> Vec<(PropKey, Vec3)> {
        let mut out: Vec<(PropKey, Vec3)> = self
            .harvest
            .iter()
            .filter(|(_, k)| **k == kind)
            .filter_map(|(&(cell, i), _)| match self.cells.get(&cell)?.get(i as usize)? {
                Item::Static { transform, .. } => Some(((cell, i), transform.translation)),
                Item::Spline { .. } => None,
            })
            .filter(|(_, at)| at.xz().distance(p.xz()) < r)
            .collect();
        out.sort_by(|a, b| a.1.xz().distance(p.xz()).total_cmp(&b.1.xz().distance(p.xz())));
        out
    }

    /// The placed static prop nearest `at` (within `within` m) whose mesh file is `stem`: its key,
    /// transform and glb path.
    pub fn prop_near(&self, at: Vec3, stem: &str, within: f32) -> Option<(PropKey, Transform, String)> {
        let c = cell_of(at);
        let suffix = format!("/{stem}.glb");
        (-1..=1)
            .flat_map(|dx| (-1..=1).map(move |dz| c + IVec2::new(dx, dz)))
            .filter_map(|cell| self.cells.get(&cell).map(|items| (cell, items)))
            .flat_map(|(cell, items)| items.iter().enumerate().map(move |(i, it)| (cell, i, it)))
            .filter_map(|(cell, i, it)| match *it {
                Item::Static { mesh, transform } if self.meshes[mesh as usize].path.ends_with(&suffix) => Some(((cell, i as u32), transform, self.meshes[mesh as usize].path.clone())),
                _ => None,
            })
            .filter(|(_, t, _)| t.translation.distance(at) <= within)
            .min_by(|a, b| a.1.translation.distance(at).total_cmp(&b.1.translation.distance(at)))
    }

    /// A placed static prop's draw parts (once loaded) and transform.
    pub fn prop_parts(&self, (cell, index): PropKey) -> Option<(Vec<DrawPart>, Transform)> {
        let Item::Static { mesh, transform } = *self.cells.get(&cell)?.get(index as usize)? else { return None };
        let parts = self.meshes[mesh as usize].parts.as_ref()?;
        Some((parts.iter().map(|p| (p.mesh.clone(), p.material.clone())).collect(), transform))
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
    /// Props gone for the session (felled trees, broken rocks): not drawn, no collision.
    removed: HashSet<PropKey>,
}

impl PropStreaming {
    /// Take a prop out of the world for the rest of the session. The caller despawns its
    /// [`PropItem`] entities and drops its collision ([`collision::CollisionCache::forget`]).
    pub fn remove(&mut self, key: PropKey) {
        self.removed.insert(key);
    }

    /// Take a prop out of the static cells for good (gameplay draws it itself): its cell, if
    /// loaded, comes back for the caller to despawn so it reloads without it.
    pub fn hide(&mut self, key: PropKey) -> Option<Entity> {
        self.removed.insert(key);
        self.loaded.remove(&key.0)
    }

    pub fn is_removed(&self, key: PropKey) -> bool {
        self.removed.contains(&key)
    }

    /// Bring props back (a prefab spawning). Their loaded cells are dropped so they stream in
    /// again with them; the caller despawns the returned cell entities and forgets the props'
    /// collision.
    pub fn restore(&mut self, keys: &[PropKey]) -> Vec<Entity> {
        for k in keys {
            self.removed.remove(k);
        }
        let cells: HashSet<IVec2> = keys.iter().map(|k| k.0).collect();
        cells.into_iter().filter_map(|c| self.loaded.remove(&c)).collect()
    }
}

fn enter_level(
    mut commands: Commands,
    level: Option<Res<Level>>,
    terrain: Option<Res<Terrain>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    // Prefab meshes stay hidden until their prefab spawns this session (`prefabs::run_prefabs`).
    let hidden = |l: &Level| l.prefabs.iter().flat_map(|s| s.keys.iter().copied()).collect::<HashSet<_>>();
    let removed = match &level {
        Some(l) => hidden(l),
        None => match Level::load() {
            Some(l) => {
                let h = hidden(&l);
                commands.insert_resource(l);
                h
            }
            None => {
                warn!("{LEVEL_DIR} missing; run extractor/export_assets.sh --level");
                return;
            }
        },
    };
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
    commands.insert_resource(PropStreaming { removed, ..default() });
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
        let water = is_water(&m.path);
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
                    parts.push(Part { mesh: p.mesh.clone(), material: lib.water(&mut materials), collide: false, lamp: None, water: true });
                    continue;
                }
                let mat = lib.get(name, &assets, &mut materials).unwrap_or_else(|| materials.add(StandardMaterial::default()));
                // Trees: trunk triangles get the material's trunk colour.
                if let Some(trunk_mat) = lib.trunk(name, &mut materials)
                    && let Some((leaves, trunk)) = meshes.get(&p.mesh).and_then(|m| crate::models::split_by_vertex_alpha(&m))
                {
                    parts.push(Part { mesh: meshes.add(leaves), material: mat, collide: false, lamp: None, water: false });
                    parts.push(Part { mesh: meshes.add(trunk), material: trunk_mat, collide: true, lamp: None, water: false });
                    continue;
                }
                // UE materials ignore vertex colour; Bevy would multiply it in (see models).
                if let Some(mut mesh) = meshes.get_mut(&p.mesh)
                    && mesh.contains_attribute(Mesh::ATTRIBUTE_COLOR)
                {
                    mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR);
                }
                parts.push(Part { mesh: p.mesh.clone(), material: mat, collide: !walk_through(name), lamp: power::LampKind::of(&m.path, name), water: false });
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
        for (i, item) in items.iter().enumerate() {
            let key = (c, i as u32);
            if state.removed.contains(&key) {
                continue;
            }
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
                        if level.harvest.contains_key(&key) {
                            child.insert(PropItem(key));
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
