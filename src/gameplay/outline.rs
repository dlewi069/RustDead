//! The outline on whatever the interact prompt names, and the containers' search icons
//! (BP_LootContainer, BP_MASTER_PicklockContainer, BP_MASTER_ToolRequiredContainer and the rest;
//! decompiled):
//!
//! - The outline: while a container's ShowLootContainerWidget, or any other interactable's
//!   ShowInteractWidget, is up (OnBeginInteract), the actor calls SetRenderCustomDepth(Video.Outline
//!   = OUTLINE OBJECTS ENABLED?, on) on its own components, off again when it goes
//!   (OnEndInteract). Which components: `assets/data/outlines.json` (gen_outlines.py: the meshes of
//!   the components each Blueprint calls it on; a well's StaticMesh, a lever's Base and Lever, a
//!   lab sliding door's Frame (not the door), a bandit turret's StaticMesh and Turret Base, a
//!   trader's or the barber's SkeletalMesh, a corpse's mesh...). The level's post process draws
//!   custom depth with M_Highlight_Inst (M_Highlight, a before-tonemapping post-process material:
//!   HighlightColour white, LineWidth 1, OutlineIntensity 1).
//! - The search icon: Event_Outline at BeginPlay starts DistanceOutline every 0.5 s. With
//!   Video.OutlineDistance (CONTAINER SEARCH ICON WHEN CLOSE TO?, on), the survivor within 15 m
//!   (GetDistanceTo: actor origins), the container not looted (ContainerLooted) and TraceToPlayer
//!   clear (a Visibility line from the actor + 15 cm up to the survivor's capsule centre + 65 cm,
//!   ignoring the container and the survivor), Event_MagnifierUI(true) adds a Screen-space
//!   WidgetComponent at MagnifierTransform (items.json `magnifier`, per class): DrawSize 25 x 25,
//!   MagnifierContainer_UI (the magnifier icon) or, on the tool-required ones,
//!   MagnifierContainer_Locked (with a 10 x 10 padlock at its bottom left). Otherwise it's removed.
//! - The airdrops (Master_AirdropContainer), the death crate (Container_DeadPlayerLoot) and the
//!   quest crate (BP_GuaranteedSpawner) have their own Magnifier component (25 x 25,
//!   MagnifierContainer_UI; the airdrop's 75 cm up, the others' at the root): DistanceOutline
//!   every 0.5 s shows it within 750 cm with Video.OutlineDistance (the death crate's also needs
//!   Video.Outline), with no trace and whether looted or not; OnBeginInteract hides it,
//!   OnEndInteract shows it again.
//!
//! Ours: M_Highlight's graph isn't readable, so the line is a 1-pixel white edge just outside the
//! silhouette, drawn over everything after tonemapping: a second camera renders the outlined
//! meshes (render layer 29) into a mask, and a full-screen UI shader (`assets/shaders/outline.wesl`)
//! lights the pixels next to it. A static prop is drawn there as a copy; a spawned entity's meshes
//! join layer 29 themselves while outlined (a character's skinned meshes only: its SkeletalMesh).
//! The prompt systems name their target in [`Highlight`] each frame. TraceToPlayer meets the
//! static props (not the container's own mesh) and the landscape; buildables and vehicles don't
//! block it. A vehicle outlines every mesh of its model (BP_VehicleMaster's is one skeletal mesh).
//! Not ported: the outline on what has no prompt of ours yet or isn't a container to us (the
//! barrier gate, the circuit box, lamps, the level's sleeping bags, the infestations, world
//! pickups), the military trader's APC; the search icon on the containers of spawned camps and
//! quest prefabs (their keys are past `CORPSE_KEYS`).

use std::collections::HashMap;

use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, TextureFormat};
use bevy::asset::RenderAssetUsages;
use bevy::shader::ShaderRef;
use bevy::ui_render::UiMaterialPlugin;
use bevy::ui_render::ui_material::{MaterialNode, UiMaterial};
use bevy::window::PrimaryWindow;

use super::PlayerCamera;
use super::interact::{CORPSE_KEYS, Containers};
use super::items::{ContainerDef, ItemDb};
use super::search_widget::SearchView;
use crate::level::collision::PropRays;
use crate::level::{KinematicBody, Level, PropKey, Terrain};
use crate::menus::AppState;
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::models::CharacterModel;

/// The outline mask's render layer.
pub const LAYER: usize = 29;
/// DistanceOutline's timer and reach (1500 cm).
pub const EVERY: f32 = 0.5;
pub const REACH: f32 = 15.0;
/// The widget component's DrawSize, and the padlock in MagnifierContainer_Locked.
pub const ICON_PX: f32 = 25.0;
pub const PADLOCK_PX: f32 = 10.0;
/// TraceToPlayer: from the actor 15 cm up to the capsule centre (0.82 m over the feet) + 65 cm.
const FROM_UP: f32 = 0.15;
const TO_UP: f32 = 0.82 + 0.65;
const ICONS: &str = "game/SurrounDead/Content/UI/Textures/over_250_survival_flat_icons";

pub fn plugin(app: &mut App) {
    app.add_plugins(UiMaterialPlugin::<OutlineMaterial>::default())
        .init_resource::<Icons>()
        .init_resource::<Highlight>()
        .insert_resource(OutlineDb::load())
        .add_systems(OnEnter(AppState::InGame), setup)
        .add_systems(Update, (fit_mask, search_icons.after(super::interact::search_containers), place_icons).chain().run_if(in_state(AppState::InGame)))
        // After every prompt system (all in Update) has named its target this frame.
        .add_systems(PostUpdate, (sync_target, tag_entities).chain().before(bevy::transform::TransformSystems::Propagate).run_if(in_state(AppState::InGame)))
        .add_observer(layer_proxy);
}

/// Draws the 1-pixel edge around the mask's shapes: `params` = colour (rgb), line width (w).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct OutlineMaterial {
    #[uniform(0)]
    pub params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub mask: Handle<Image>,
}

impl UiMaterial for OutlineMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/outline.wesl".into()
    }
}

/// What the interact prompt shows this frame, named by the system that claimed it: the actor whose
/// OnBeginInteract turns its custom depth on. Taken (back to `None`) every frame.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct Highlight(pub Option<Highlit>);

#[derive(Clone, Debug, PartialEq)]
pub enum Highlit {
    /// A container (`interact::Containers` key): a level container's StaticMesh, a spawned one's
    /// mesh, a corpse, a trader, an airdrop, the death crate, a quest crate, a placed storage.
    Key(usize),
    /// A level Blueprint by class and actor origin: its outlined components (outlines.json), found
    /// among the static props at its origin.
    Actor { class: String, at: Vec3 },
    /// Door `i` of `Level::doors`: its moving parts, plus its outlined components still static
    /// (a lever's Base, a lab sliding door's Frame instead of its door).
    Door(usize),
    /// A placed buildable (`building::Placed` id): its StaticMesh.
    Buildable(usize),
    /// Spawned entities; `skinned`: only their skinned meshes (a SkeletalMesh component).
    Entities { roots: Vec<Entity>, skinned: bool },
    /// Static props (a burnt-out vehicle's StaticMesh).
    Props(Vec<PropKey>),
}

impl Highlight {
    /// Name the prompt's target, unless a system before this one claimed the prompt this frame
    /// (`interact::search_containers` runs first, and its loot widget leaves the plain prompt
    /// hidden for the others).
    pub fn claim(&mut self, target: Highlit) {
        self.0.get_or_insert(target);
    }
}

impl Highlit {
    /// The container key, for a container.
    pub fn key(&self) -> Option<usize> {
        match self {
            Highlit::Key(k) => Some(*k),
            _ => None,
        }
    }
}

/// outlines.json: each Blueprint class's outlined components' mesh stems.
#[derive(Resource, Default)]
pub struct OutlineDb(pub HashMap<String, Vec<String>>);

impl OutlineDb {
    pub fn load() -> Self {
        Self(std::fs::read_to_string("assets/data/outlines.json").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default())
    }
}

/// How far from the actor's origin its outlined components are looked for (they sit at it: 0.0-0.04 m
/// on every level actor with one; a bandit turret's generator is 0.63 m off).
const ACTOR_REACH: f32 = 1.0;

/// The props drawing `class`'s outlined components at `at`: the nearest of each stem (two for a
/// stem listed twice), skipping `except` (stems drawn some other way).
fn actor_props(level: &Level, db: &OutlineDb, class: &str, at: Vec3, except: &[String]) -> Vec<PropKey> {
    let Some(stems) = db.0.get(class) else { return Vec::new() };
    let mut out = Vec::new();
    let mut seen: Vec<&String> = Vec::new();
    for stem in stems.iter().filter(|s| !except.contains(s)) {
        if seen.contains(&stem) {
            continue;
        }
        seen.push(stem);
        let n = stems.iter().filter(|s| *s == stem).count();
        out.extend(level.props_near(at, stem, ACTOR_REACH).into_iter().take(n).map(|(k, ..)| k));
    }
    out
}

/// The outline's pieces and what it shows.
#[derive(Resource)]
pub struct Outline {
    /// What's outlined.
    pub target: Option<Highlit>,
    /// The copies of outlined static props drawn into the mask.
    proxies: Vec<Entity>,
    /// Spawned entities outlined in place (their meshes join the mask's layer) and whether only
    /// skinned meshes count.
    roots: Vec<Entity>,
    skinned: bool,
    /// Meshes put on the mask's layer, with the layers they had.
    tagged: Vec<(Entity, Option<RenderLayers>)>,
    /// A prop to copy wasn't loaded yet: try again next frame.
    pending: bool,
    camera: Entity,
    node: Entity,
    material: Handle<OutlineMaterial>,
    size: UVec2,
}

impl Outline {
    pub fn key(&self) -> Option<usize> {
        self.target.as_ref().and_then(Highlit::key)
    }

    /// The copies drawn into the mask and the spawned meshes put on its layer.
    pub fn drawn(&self) -> (usize, usize) {
        (self.proxies.len(), self.tagged.len())
    }
}

/// The copy of an outlined mesh (its scene's parts go on the mask's layer once spawned).
#[derive(Component)]
struct OutlineProxy;

#[derive(Component)]
struct MaskCamera;

#[derive(Component)]
struct OutlineNode;

/// Each container's icon point, origin and own props (found once), and the icons shown.
#[derive(Resource, Default)]
pub struct Icons {
    spots: HashMap<usize, Option<Spot>>,
    /// The icons up, by container key (Event_MagnifierUI(true) adds, (false) removes).
    pub shown: HashMap<usize, Entity>,
    tick: f32,
}

#[derive(Clone, Debug)]
struct Spot {
    /// MagnifierTransform's point in the world.
    icon: Vec3,
    /// The actor's location.
    origin: Vec3,
    /// The container's own StaticMesh (TraceToPlayer ignores self).
    own: Vec<PropKey>,
    locked: bool,
}

#[derive(Component)]
struct SearchIcon(usize);

fn mask_image(size: UVec2) -> Image {
    let mut img = Image::new_target_texture(size.x.max(1), size.y.max(1), TextureFormat::Rgba8Unorm, None);
    img.asset_usage = RenderAssetUsages::RENDER_WORLD;
    img
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<OutlineMaterial>>, window: Query<&Window, With<PrimaryWindow>>, mut icons: ResMut<Icons>, mut highlight: ResMut<Highlight>) {
    *icons = Icons::default();
    highlight.0 = None;
    let size = window.single().map(|w| UVec2::new(w.physical_width(), w.physical_height())).unwrap_or(UVec2::new(1920, 1080));
    let mask = images.add(mask_image(size));
    let camera = commands
        .spawn((
            Camera3d::default(),
            Camera { order: -2, is_active: false, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
            RenderTarget::Image(mask.clone().into()),
            Msaa::Off,
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            RenderLayers::layer(LAYER),
            MaskCamera,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let material = materials.add(OutlineMaterial { params: Vec4::new(1.0, 1.0, 1.0, 1.0), mask });
    let node = commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() },
            MaterialNode(material.clone()),
            GlobalZIndex(i32::MIN + 2),
            bevy::picking::Pickable::IGNORE,
            OutlineNode,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    commands.insert_resource(Outline { target: None, proxies: Vec::new(), roots: Vec::new(), skinned: false, tagged: Vec::new(), pending: false, camera, node, material, size });
}

/// Keep the mask at the window's size and the mask camera on the player camera's view.
#[allow(clippy::type_complexity)]
fn fit_mask(
    mut commands: Commands,
    outline: Option<ResMut<Outline>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<OutlineMaterial>>,
    main: Query<(Entity, &Projection), (With<PlayerCamera>, Without<MaskCamera>)>,
    mut mask_cam: Query<(Option<&ChildOf>, &mut Projection), With<MaskCamera>>,
) {
    let Some(mut outline) = outline else { return };
    if let Ok(w) = window.single() {
        let size = UVec2::new(w.physical_width(), w.physical_height());
        if size != outline.size && size.min_element() > 0 {
            outline.size = size;
            let mask = images.add(mask_image(size));
            commands.entity(outline.camera).insert(RenderTarget::Image(mask.clone().into()));
            if let Some(mut m) = materials.get_mut(&outline.material) {
                m.mask = mask;
            }
        }
    }
    let (Ok((cam, proj)), Ok((parent, mut p))) = (main.single(), mask_cam.single_mut()) else { return };
    if parent.is_none_or(|c| c.parent() != cam) {
        commands.entity(outline.camera).insert((ChildOf(cam), Transform::IDENTITY));
    }
    *p = proj.clone();
}

/// The container's StaticMesh: the class's mesh nearest the actor among the level's props.
fn own_props(level: &Level, def: &ContainerDef, origin: Vec3) -> Vec<PropKey> {
    let Some(stem) = def.mesh.as_deref().and_then(|m| m.rsplit('/').next()) else { return Vec::new() };
    level.prop_near(origin, stem, 2.0).map(|(k, ..)| vec![k]).unwrap_or_default()
}

/// A container's actor transform: a level actor's, or the spawned container's.
fn actor_transform(level: &Level, spawns: Option<&crate::level::item_spawners::ItemSpawns>, key: usize, class: &str, at: Vec3) -> Transform {
    if let Some(s) = spawns.and_then(|s| s.0.iter().find(|s| s.key == key && s.class == class)) {
        return s.transform;
    }
    match level.actors.get(key).filter(|a| a.class == class) {
        Some(a) => crate::level::affine(&a.transform),
        None => Transform::from_translation(at),
    }
}

/// The spawned entities a container key can be.
#[derive(bevy::ecs::system::SystemParam)]
struct KeyBodies<'w, 's> {
    containers: Res<'w, Containers>,
    traders: Query<'w, 's, (Entity, &'static super::traders::TraderBody)>,
    airdrops: Query<'w, 's, (Entity, &'static super::airdrops::AirCrate)>,
    quest: Query<'w, 's, (Entity, &'static super::quests::QuestCrate)>,
    death: Query<'w, 's, (Entity, &'static super::death_loot::Crate)>,
    placed: Query<'w, 's, (Entity, &'static super::building::Placed)>,
}

impl KeyBodies<'_, '_> {
    /// The entity drawing container `key` and whether only its skinned meshes count.
    fn find(&self, key: usize) -> Option<(Entity, bool)> {
        if let Some(e) = self.containers.corpse(key) {
            return Some((e, true));
        }
        if let Some((e, _)) = self.traders.iter().find(|(_, t)| super::traders::TRADER_KEYS + t.0 == key) {
            return Some((e, true));
        }
        let crate_of = self
            .airdrops
            .iter()
            .find(|(_, c)| c.key == Some(key))
            .map(|(e, _)| e)
            .or_else(|| self.quest.iter().find(|(_, c)| c.key == key).map(|(e, _)| e))
            .or_else(|| self.death.iter().find(|(_, c)| super::death_loot::DEATH_KEYS + c.0 == key).map(|(e, _)| e))
            .or_else(|| {
                // A placed storage's or bench's key.
                let ids = [key.checked_sub(super::storage::STORAGE_KEYS), key.checked_sub(super::building::BENCH_KEYS)];
                self.placed.iter().find(|(_, p)| ids.contains(&Some(p.0))).map(|(e, _)| e)
            });
        crate_of.map(|e| (e, false))
    }
}

/// OnBeginInteract / OnEndInteract: swap what's outlined when the prompt's target changes (with
/// OUTLINE OBJECTS ENABLED?).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_target(
    mut commands: Commands,
    mut highlight: ResMut<Highlight>,
    store: Res<SettingsStore>,
    outline: Option<ResMut<Outline>>,
    level: Option<Res<Level>>,
    (spawns, db, outlines, doors): (Option<Res<crate::level::item_spawners::ItemSpawns>>, Res<ItemDb>, Res<OutlineDb>, Option<Res<crate::level::doors::Doors>>),
    bodies: KeyBodies,
    assets: Res<AssetServer>,
    mut cameras: Query<&mut Camera, With<MaskCamera>>,
    mut nodes: Query<&mut Node, With<OutlineNode>>,
) {
    let now = highlight.0.take();
    let (Some(mut outline), Some(level)) = (outline, level) else { return };
    let on = store.find(&row_key("settings", "Gameplay", "OUTLINE OBJECTS ENABLED?")) != Some(Value::Bool(false));
    let now = now.filter(|_| on);
    if now == outline.target && !outline.pending {
        return;
    }
    for p in outline.proxies.drain(..) {
        commands.entity(p).try_despawn();
    }
    for (e, prev) in outline.tagged.drain(..) {
        match prev {
            Some(l) => commands.entity(e).try_insert(l),
            None => commands.entity(e).try_remove::<RenderLayers>(),
        };
    }
    outline.target = now.clone();
    let (mut props, mut roots, mut skinned, mut glb) = (Vec::new(), Vec::new(), false, None);
    match now {
        None => {}
        Some(Highlit::Key(key)) => {
            if let Some((e, s)) = bodies.find(key) {
                roots.push(e);
                skinned = s;
            } else if let Some((_, at, class)) = bodies.containers.list.iter().find(|c| c.0 == key)
                && let Some(def) = db.containers.get(class)
                && let Some(mesh) = def.mesh.as_deref()
            {
                // A level container's StaticMesh is a static prop; a spawned one (the hidden
                // caches, briefcases) draws its class's mesh at its transform.
                props = own_props(&level, def, *at);
                if props.is_empty() {
                    glb = Some((format!("game/{mesh}.glb"), actor_transform(&level, spawns.as_deref(), key, class, *at)));
                }
            }
        }
        Some(Highlit::Actor { class, at }) => props = actor_props(&level, &outlines, &class, at, &[]),
        Some(Highlit::Door(i)) => {
            if let Some(d) = level.doors.get(i) {
                let class = level.actors.get(d.actor).map_or("", |a| a.class.as_str());
                // The lab sliding door outlines its Frame, not the door that slides.
                if class != "BP_LaboratorySlidingDoor_C" {
                    roots.extend(doors.as_deref().map(|ds| ds.part_entities(i).to_vec()).unwrap_or_default());
                }
                let moving: Vec<String> = d.parts.iter().map(|p| level.mesh_stem(p.mesh).to_string()).collect();
                let at = level.actors.get(d.actor).map_or(d.pos, |a| a.position());
                props = actor_props(&level, &outlines, class, at, &moving);
            }
        }
        Some(Highlit::Buildable(id)) => roots.extend(bodies.placed.iter().find(|(_, p)| p.0 == id).map(|(e, _)| e)),
        Some(Highlit::Entities { roots: r, skinned: s }) => {
            roots = r;
            skinned = s;
        }
        Some(Highlit::Props(p)) => props = p,
    }
    let layer = RenderLayers::layer(LAYER);
    outline.pending = false;
    for key in props {
        let Some((parts, t)) = level.prop_parts(key) else {
            outline.pending = true;
            continue;
        };
        {
            let root = commands.spawn((t, Visibility::default(), OutlineProxy, DespawnOnExit(AppState::InGame))).id();
            for (m, mat) in parts {
                commands.spawn((Mesh3d(m), MeshMaterial3d(mat), Transform::IDENTITY, layer.clone(), ChildOf(root)));
            }
            outline.proxies.push(root);
        }
    }
    if let Some((path, t)) = glb {
        let e = commands.spawn((crate::models::model(&assets, path), t, OutlineProxy, layer, DespawnOnExit(AppState::InGame))).id();
        outline.proxies.push(e);
    }
    outline.roots = roots;
    outline.skinned = skinned;
    let any = !outline.proxies.is_empty() || !outline.roots.is_empty();
    if let Ok(mut c) = cameras.get_mut(outline.camera) {
        c.is_active = any;
    }
    if let Ok(mut n) = nodes.get_mut(outline.node) {
        n.display = if any { Display::Flex } else { Display::None };
    }
    if !outline.pending {
        info!("outline: {:?} ({} copies, {} entities)", outline.target, outline.proxies.len(), outline.roots.len());
    }
}

/// SetRenderCustomDepth on a spawned entity: its meshes (those loaded so far, and later ones as
/// they come) join the mask's layer.
fn tag_entities(
    mut commands: Commands,
    outline: Option<ResMut<Outline>>,
    children: Query<&Children>,
    meshes: Query<(Option<&RenderLayers>, Has<bevy::mesh::skinning::SkinnedMesh>), With<Mesh3d>>,
) {
    let Some(mut outline) = outline else { return };
    let mask = RenderLayers::layer(LAYER);
    for root in outline.roots.clone() {
        for e in std::iter::once(root).chain(children.iter_descendants(root)) {
            let Ok((layers, skinned)) = meshes.get(e) else { continue };
            if (outline.skinned && !skinned) || layers.is_some_and(|l| l.intersects(&mask)) || outline.tagged.iter().any(|t| t.0 == e) {
                continue;
            }
            let prev = layers.cloned();
            commands.entity(e).try_insert(prev.clone().unwrap_or_default().with(LAYER));
            outline.tagged.push((e, prev));
        }
    }
}

/// Scene children don't inherit render layers: put the copy's parts on the mask's.
fn layer_proxy(ev: On<bevy::world_serialization::WorldInstanceReady>, proxies: Query<(), With<OutlineProxy>>, children: Query<&Children>, mut commands: Commands) {
    if proxies.contains(ev.entity) {
        for e in children.iter_descendants(ev.entity) {
            commands.entity(e).insert(RenderLayers::layer(LAYER));
        }
    }
}

/// DistanceOutline's rule for one container.
pub fn icon_shown(enabled: bool, looted: bool, distance: f32, clear: bool) -> bool {
    enabled && !looted && distance <= REACH && clear
}

/// The airdrops', the death crate's and the quest crate's DistanceOutline (750 cm).
pub const CLOSE_REACH: f32 = 7.5;
/// The airdrop's Magnifier component: 75 cm up.
const AIRDROP_ICON_UP: f32 = 0.75;

/// Their rule: within 7.5 m with the setting on (the death crate's also needs OUTLINE OBJECTS
/// ENABLED?), hidden while their prompt is up (OnBeginInteract), looted or not.
pub fn close_icon_shown(enabled: bool, distance: f32, prompted: bool) -> bool {
    enabled && distance <= CLOSE_REACH && !prompted
}

/// DistanceOutline every 0.5 s over the loot containers in reach.
#[allow(clippy::too_many_arguments)]
fn search_icons(
    mut commands: Commands,
    time: Res<Time>,
    store: Res<SettingsStore>,
    db: Res<ItemDb>,
    containers: Res<Containers>,
    level: Option<Res<Level>>,
    spawns: Option<Res<crate::level::item_spawners::ItemSpawns>>,
    terrain: Option<Res<Terrain>>,
    assets: Res<AssetServer>,
    view: Res<SearchView>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
    mut rays: PropRays,
    mut icons: ResMut<Icons>,
) {
    icons.tick += time.delta_secs();
    if icons.tick < EVERY {
        return;
    }
    icons.tick = 0.0;
    let (Some(level), Ok(p)) = (level, player.single()) else { return };
    let enabled = store.find(&row_key("settings", "Gameplay", "CONTAINER SEARCH ICON WHEN CLOSE TO?")) != Some(Value::Bool(false));
    let outline_on = store.find(&row_key("settings", "Gameplay", "OUTLINE OBJECTS ENABLED?")) != Some(Value::Bool(false));
    let centre = p.translation + Vec3::Y * 0.82;
    let mut up: Vec<usize> = Vec::new();
    let death = |class: &str| class == super::death_loot::CLASS;
    for (key, at, class) in containers.list.iter().filter(|c| c.1.distance(centre) <= REACH + 2.0) {
        // The airdrops, the death crate and the quest crate: their own Magnifier component, moved
        // with them (an airdrop lands, a death crate falls to the floor).
        let airdrop = db.containers.get(class).is_some_and(|d| d.airdrop.is_some());
        if airdrop || death(class) || containers.quest_crates.contains(key) {
            let lift = if airdrop { AIRDROP_ICON_UP } else { 0.0 };
            icons.spots.insert(*key, Some(Spot { icon: *at + Vec3::Y * lift, origin: *at, own: Vec::new(), locked: false }));
            let prompted = view.shown && view.key == Some(*key);
            if close_icon_shown(enabled && (outline_on || !death(class)), at.distance(centre), prompted) {
                up.push(*key);
            }
            continue;
        }
        // The three masters' children: not corpses, stations or traders.
        let Some(def) = db.containers.get(class).filter(|d| d.hold > 0.0) else { continue };
        if *key >= CORPSE_KEYS || super::traders::is_trader(*key) {
            continue;
        }
        let spot = icons
            .spots
            .entry(*key)
            .or_insert_with(|| {
                let t = actor_transform(&level, spawns.as_deref(), *key, class, *at);
                let [x, y, z] = def.magnifier;
                Some(Spot { icon: t.transform_point(Vec3::new(x, y, z)), origin: t.translation, own: own_props(&level, def, t.translation), locked: def.tool.is_some() })
            })
            .clone();
        let Some(spot) = spot else { continue };
        let distance = spot.origin.distance(centre);
        let looted = containers.looted(*key);
        let mut clear = || {
            let (a, b) = (spot.origin + Vec3::Y * FROM_UP, p.translation + Vec3::Y * TO_UP);
            let (dir, len) = ((b - a).normalize_or_zero(), a.distance(b));
            super::zombies::clear_line(terrain.as_deref(), a, b) && rays.cast_except(a, dir, len, &spot.own).is_none()
        };
        if icon_shown(enabled, looted, distance, !looted && distance <= REACH && clear()) {
            up.push(*key);
        }
    }
    let gone: Vec<usize> = icons.shown.keys().filter(|k| !up.contains(k)).copied().collect();
    for k in gone {
        if let Some(e) = icons.shown.remove(&k) {
            commands.entity(e).try_despawn();
        }
    }
    for k in up {
        if icons.shown.contains_key(&k) {
            continue;
        }
        let locked = icons.spots.get(&k).and_then(|s| s.as_ref()).is_some_and(|s| s.locked);
        let e = commands
            .spawn((
                Node { position_type: PositionType::Absolute, width: Val::Px(ICON_PX), height: Val::Px(ICON_PX), ..default() },
                ImageNode::new(assets.load(format!("{ICONS}/magnifier.png"))),
                GlobalZIndex(-3),
                bevy::picking::Pickable::IGNORE,
                Visibility::Hidden,
                SearchIcon(k),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        if locked {
            commands.spawn((
                Node { position_type: PositionType::Absolute, left: Val::Px(0.0), bottom: Val::Px(0.0), width: Val::Px(PADLOCK_PX), height: Val::Px(PADLOCK_PX), ..default() },
                ImageNode::new(assets.load(format!("{ICONS}/padlock.png"))),
                bevy::picking::Pickable::IGNORE,
                ChildOf(e),
            ));
        }
        icons.shown.insert(k, e);
    }
}

/// A Screen-space widget component: centred on its point, drawn over the world.
fn place_icons(
    icons: Res<Icons>,
    ui_scale: Res<UiScale>,
    camera: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    mut nodes: Query<(&SearchIcon, &mut Node, &mut Visibility)>,
) {
    let Ok((cam, cam_t)) = camera.single() else { return };
    for (icon, mut node, mut vis) in &mut nodes {
        let Some(Some(spot)) = icons.spots.get(&icon.0) else { continue };
        match cam.world_to_viewport(cam_t, spot.icon) {
            Ok(px) => {
                let px = px / ui_scale.0;
                node.left = Val::Px(px.x - ICON_PX / 2.0);
                node.top = Val::Px(px.y - ICON_PX / 2.0);
                vis.set_if_neq(Visibility::Inherited);
            }
            Err(_) => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_shows_within_15_m_of_an_unlooted_container_in_sight() {
        assert!(icon_shown(true, false, 14.9, true));
        assert!(!icon_shown(true, false, 15.1, true));
        assert!(!icon_shown(true, true, 3.0, true), "ContainerLooted");
        assert!(!icon_shown(true, false, 3.0, false), "TraceToPlayer hit something");
        assert!(!icon_shown(false, false, 3.0, true), "CONTAINER SEARCH ICON WHEN CLOSE TO? off");
    }

    #[test]
    fn the_crates_own_magnifiers_show_within_7_5_m_until_their_prompt_is_up() {
        assert!(close_icon_shown(true, 7.4, false));
        assert!(!close_icon_shown(true, 7.6, false), "DistanceOutline's 750 cm");
        assert!(!close_icon_shown(true, 3.0, true), "OnBeginInteract hides it");
        assert!(!close_icon_shown(false, 3.0, false), "Video.OutlineDistance off");
    }

    #[test]
    fn the_first_prompt_system_keeps_the_outline() {
        let mut h = Highlight::default();
        h.claim(Highlit::Key(7));
        h.claim(Highlit::Door(3));
        assert_eq!(h.0, Some(Highlit::Key(7)), "the loot widget's container, not a door claimed after it");
        assert_eq!(h.0.as_ref().and_then(Highlit::key), Some(7));
        assert_eq!(Highlit::Door(3).key(), None);
    }

    #[test]
    fn each_interactable_outlines_its_own_components() {
        let db = OutlineDb::load();
        let o = |c: &str| db.0.get(c).cloned().unwrap_or_default();
        assert_eq!(o("BP_WaterWell_C"), ["Mesh_WaterWell"]);
        // BP_Lever: Base and Lever; the lab sliding door: its Frame, not the Door that slides.
        assert_eq!(o("BP_Lever_C"), ["LeverBase", "Lever"]);
        assert_eq!(o("BP_LaboratorySlidingDoor_C"), ["LabSmallDoorframe"]);
        // A child inherits the calls, with its own mesh.
        assert_eq!(o("BP_Radio_Text_Military_C"), ["SM_Prop_Radio_01"]);
        assert_eq!(o("BP_BanditTurret_AI_C"), ["SM_Prop_Generator_Small_01", "SM_Prop_Computer_Camera_Tripod_01"]);
        assert_eq!(o("BP_ContainerDoors_C").len(), 2, "Door1 and Door2");
    }

    #[test]
    fn every_loot_container_class_has_its_icon_point() {
        let db = ItemDb::load();
        let c = &db.containers;
        // MagnifierTransform: (0, 0, 50) cm on the hidden cache, (0, 0, 90) on the gun lockers.
        assert_eq!(c["Container_HiddenCache_C"].magnifier, [0.0, 0.5, 0.0]);
        assert_eq!(c["BP_ToolRequired_GunLocker_C"].magnifier, [0.0, 0.9, 0.0]);
        // The vending machine's is 25 cm out along X.
        assert_eq!(c["Container_ShopLoot_VendingMachine_C"].magnifier, [0.25, 1.0, 0.0]);
        assert!(c["BP_ToolRequired_GunLocker_C"].tool.is_some(), "MagnifierContainer_Locked");
    }

    #[test]
    fn refills_follow_each_class_template() {
        let db = ItemDb::load();
        let c = &db.containers;
        assert!(c["Container_HiddenCache_C"].refills);
        assert!(c["BP_ToolRequired_GunLocker_C"].refills);
        for no in ["Container_AnimalMeat_C", "Container_POICrate_C", "Container_CompoundCrate_C", "Container_SpecialItemSpawner_C"] {
            assert!(!c[no].refills, "{no}: DoesContainerRefillLoot? false on its BP_JigMultiplayer");
        }
    }
}
