//! The survivor's death loot: BP_PlayerCharacter.SpawnDeadPlayerLoot and
//! `Inventory/Containers/Container_DeadPlayerLoot` (decompiled):
//!
//! - On death (both of Client_Died's paths), unless Difficulty.KeepInventory ("KEEP INVENTORY ON
//!   DEATH?": then the controller keeps the containers for the new pawn), a
//!   Container_DeadPlayerLoot is spawned at the body's spine_02 socket, unrotated, and its
//!   BP_JigMultiplayer takes the pawn's MainJigContainers: everything worn and carried.
//! - The crate (SM_Prop_Crate_04, tagged NoVault) settles: TraceForFloor every 0.01 s traces 1 cm
//!   down (WorldStatic / WorldDynamic) and moves to the hit, else 1 cm lower (1 m/s).
//! - BeginPlay spawns a BP_Marker at it: T_HumanSkull_256x256, "Death", "Location of your death.",
//!   Texture Size 2, on the compass, life span 1200 s. With Game.DeathWidget off ("IN GAME MARKER
//!   WHEN NEAR TO DEATH LOCATION?") its DeathWidget component goes; else ContainerDeathWidget (the
//!   skull tinted EEEEEE over "Death Location", 14 pt, screen space at its desired size) shows
//!   while the crate is in its AIOSubject's first layer (50 m). The mesh is hidden past 50 m too.
//! - Interact: "Dead Body Loot"; OnExecuteInteract (a press, no hold) opens its containers beside
//!   the inventory (W_DeadPlayerLootUI), plays Loot_MidShelf_GrabItem_Montage, removes the marker
//!   and the DeathWidget and sets "[Looted]". Nothing removes the crate itself, emptied or not.
//!
//! - The marker's AddMarkerToCompass? reaches W_WorldMarker's Marker Data, which never reads it:
//!   the death marker doesn't show on the compass in the game either.
//!
//! Ours: the contents are one loot grid (the worn items carry their own grids, as when dragged)
//! instead of W_DeadPlayerLootUI's equipment-shaped layout (a copy of the equipment panel: every
//! slot a JSIContainer, then the worn grids); the spine is 1 m above the feet; the crates and their
//! markers are saved (`GameState::death_bags`; the game's EMS saves its actors). The grab montage
//! is `interact::open_montage`'s; the magnifier and outline `outline.rs`'s.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::interact::{Containers, GameOverlay, LOOT_COLUMNS, insert_growing, player_inv};
use super::items::{Grid, ItemDb};
use super::map::{WorldMarker, WorldMarkers};
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::{AppState, CurrentGame};

/// The crates' container keys: past the placed benches'.
pub const DEATH_KEYS: usize = super::building::BENCH_KEYS + (1 << 26);
pub const CLASS: &str = "Container_DeadPlayerLoot_C";
/// The interact widget's name and the grid's title (ContainerName).
pub const PROMPT_NAME: &str = "Dead Body Loot";
const TITLE: &str = "Dead Player Container";
const MESH: &str = "game/SurrounDead/Content/PolygonFiles/PolygonApocalypse/Meshes/Props/SM_Prop_Crate_04.glb";
const SKULL: &str = "game/SurrounDead/Content/UI/Textures/Survivors_Icon_Pack/256x256/Hunting_For_Food/T_HumanSkull_256x256.png";
/// The BP_Marker's SetLifeSpan.
pub const MARKER_LIFE: f32 = 1200.0;
/// TraceForFloor: 1 cm per 0.01 s.
const SETTLE_SPEED: f32 = 1.0;
/// The AIOSubject's first optimisation layer (5000 cm): the mesh and the DeathWidget show inside it.
const NEAR: f32 = 50.0;
/// Ours: spine_02's height above the feet.
const SPINE: f32 = 1.0;

/// A Container_DeadPlayerLoot in the save.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeathBag {
    pub at: [f32; 3],
    pub grid: Grid,
    /// The death marker's life left (s); `None` once it's gone (looted or expired).
    pub marker: Option<f32>,
    /// Settled on the floor.
    #[serde(default)]
    pub settled: bool,
}

/// The survivor just died (Client_Died), at their feet.
#[derive(Message, Clone, Copy, Debug)]
pub struct Died {
    pub at: Vec3,
}

/// A death crate, by its bag's index (container key `DEATH_KEYS` + it).
#[derive(Component)]
pub struct Crate(pub usize);
#[derive(Component)]
struct DeathWidget(usize);

/// Crates spawned for this session's saved bags.
#[derive(Resource, Default)]
struct Spawned(usize);

pub fn plugin(app: &mut App) {
    app.add_message::<Died>()
        .init_resource::<Spawned>()
        .add_systems(OnEnter(AppState::InGame), |mut s: ResMut<Spawned>| s.0 = 0)
        .add_systems(Update, (drop_bag, spawn_crates, settle, opened, sync, death_widget).chain().run_if(in_state(AppState::InGame)));
}

/// SpawnDeadPlayerLoot's contents: every worn item (with its grids) and everything in the
/// pockets, in one grid; the inventory is left empty.
pub fn bag_grid(db: &ItemDb, inv: &mut super::equipment::PlayerInventory) -> Grid {
    let mut g = Grid::new(TITLE, LOOT_COLUMNS, 6);
    let inv = std::mem::take(inv);
    for stack in inv.equipped.into_values().chain(inv.pockets.items.into_iter().map(|p| p.stack)) {
        insert_growing(db, &mut g, stack);
    }
    g
}

/// Difficulty.KeepInventory.
pub fn keeps_inventory(store: &SettingsStore) -> bool {
    store.find(&row_key("difficulty", "Player", "KEEP INVENTORY ON DEATH?")) == Some(Value::Bool(true))
}

fn widget_on(store: &SettingsStore) -> bool {
    store.find(&row_key("settings", "Gameplay", "IN GAME MARKER WHEN NEAR TO DEATH LOCATION? (RESTART REQUIRED)")) != Some(Value::Bool(false))
}

fn drop_bag(mut died: MessageReader<Died>, db: Res<ItemDb>, store: Res<SettingsStore>, mut game: ResMut<CurrentGame>) {
    for d in died.read() {
        if keeps_inventory(&store) || game.state.is_none() {
            continue;
        }
        let grid = bag_grid(&db, player_inv(&mut game));
        let n = grid.items.len();
        let Some(state) = game.state.as_mut() else { continue };
        state.death_bags.push(DeathBag { at: (d.at + Vec3::Y * SPINE).to_array(), grid, marker: Some(MARKER_LIFE), settled: false });
        info!("death loot: a crate with {n} items at {:.1}", d.at);
    }
}

/// The saved / new bags into the world: the crate, its container and its marker.
fn spawn_crates(mut commands: Commands, assets: Res<AssetServer>, game: Res<CurrentGame>, mut spawned: ResMut<Spawned>, mut containers: ResMut<Containers>, mut markers: ResMut<WorldMarkers>) {
    let Some(state) = game.state.as_ref() else { return };
    while spawned.0 < state.death_bags.len() {
        let i = spawned.0;
        spawned.0 += 1;
        let b = &state.death_bags[i];
        let key = DEATH_KEYS + i;
        let at = Vec3::from_array(b.at);
        containers.add_at(key, at, CLASS, Vec::new());
        containers.searched.insert(key, b.grid.clone());
        commands.spawn((crate::models::model(&assets, MESH), Transform::from_translation(at), Crate(i), DespawnOnExit(AppState::InGame)));
        if b.marker.is_some() {
            markers.0.push(death_marker(key, at));
        }
    }
}

pub fn death_marker(key: usize, pos: Vec3) -> WorldMarker {
    WorldMarker {
        key,
        pos,
        texture: SKULL.into(),
        size: 2.0,
        colour: Color::WHITE,
        title: "Death".into(),
        description: "Location of your death.".into(),
    }
}

/// TraceForFloor: down at 1 m/s until the floor; the marker's life runs out.
#[allow(clippy::too_many_arguments)]
fn settle(
    time: Res<Time>,
    terrain: Option<Res<crate::level::terrain::Terrain>>,
    mut rays: crate::level::collision::PropRays,
    mut game: ResMut<CurrentGame>,
    mut containers: ResMut<Containers>,
    mut markers: ResMut<WorldMarkers>,
    mut crates: Query<(&mut Transform, &Crate)>,
) {
    let dt = time.delta_secs();
    let Some(state) = game.bypass_change_detection().state.as_mut() else { return };
    for (i, b) in state.death_bags.iter_mut().enumerate() {
        if let Some(life) = b.marker.as_mut() {
            *life -= dt;
            if *life <= 0.0 {
                b.marker = None;
                markers.0.retain(|m| m.key != DEATH_KEYS + i);
            }
        }
    }
    for (mut t, c) in &mut crates {
        let Some(b) = state.death_bags.get_mut(c.0) else { continue };
        t.translation = Vec3::from_array(b.at);
        if b.settled {
            continue;
        }
        let p = t.translation;
        let step = SETTLE_SPEED * dt;
        let prop = rays.cast(p + Vec3::Y * 0.01, Vec3::NEG_Y, 0.01 + step).map(|d| p.y + 0.01 - d);
        let ground = terrain.as_ref().and_then(|tr| tr.ground_under(p + Vec3::Y * 0.01)).filter(|g| *g >= p.y - step);
        match prop.into_iter().chain(ground).reduce(f32::max) {
            Some(floor) => {
                t.translation.y = floor;
                b.settled = true;
                info!("death loot: crate {} settled at {:.1}", c.0, t.translation);
            }
            None => t.translation.y -= step,
        }
        b.at = t.translation.to_array();
        if let Some(entry) = containers.list.iter_mut().find(|e| e.0 == DEATH_KEYS + c.0) {
            entry.1 = t.translation;
        }
    }
}

/// OnExecuteInteract: the marker and the DeathWidget go.
fn opened(overlay: Res<GameOverlay>, mut game: ResMut<CurrentGame>, mut markers: ResMut<WorldMarkers>) {
    let GameOverlay::Loot(key) = *overlay else { return };
    let Some(i) = key.checked_sub(DEATH_KEYS).filter(|i| *i < 1 << 26) else { return };
    let Some(b) = game.bypass_change_detection().state.as_mut().and_then(|s| s.death_bags.get_mut(i)) else { return };
    if b.marker.take().is_some() {
        markers.0.retain(|m| m.key != key);
        info!("death loot: crate {i} opened, marker removed");
    }
}

/// Keep the save's bags in step with what's left in them.
fn sync(containers: Res<Containers>, mut game: ResMut<CurrentGame>) {
    let Some(state) = game.bypass_change_detection().state.as_mut() else { return };
    for (i, b) in state.death_bags.iter_mut().enumerate() {
        if let Some(g) = containers.searched.get(&(DEATH_KEYS + i))
            && *g != b.grid
        {
            b.grid = g.clone();
        }
    }
}

/// ContainerDeathWidget over each unlooted crate within 50 m.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn death_widget(
    mut commands: Commands,
    assets: Res<AssetServer>,
    font: Res<crate::menus::widgets::MenuFont>,
    store: Res<SettingsStore>,
    game: Res<CurrentGame>,
    camera: Query<(&Camera, &GlobalTransform), With<super::PlayerCamera>>,
    mut crates: Query<(&Transform, &Crate, &mut Visibility)>,
    mut widgets: Query<(Entity, &DeathWidget, &mut Node)>,
) {
    let (Some(state), Ok((cam, cam_t))) = (game.state.as_ref(), camera.single()) else { return };
    let on = widget_on(&store);
    let mut shown = Vec::new();
    for (t, c, mut vis) in &mut crates {
        let near = t.translation.distance(cam_t.translation()) < NEAR;
        vis.set_if_neq(if near { Visibility::Inherited } else { Visibility::Hidden });
        let Some(b) = state.death_bags.get(c.0) else { continue };
        if !(on && near && b.marker.is_some()) {
            continue;
        }
        if let Ok(px) = cam.world_to_viewport(cam_t, t.translation) {
            shown.push((c.0, px));
        }
    }
    for (e, w, mut n) in &mut widgets {
        match shown.iter().position(|s| s.0 == w.0) {
            Some(k) => {
                let (_, px) = shown.swap_remove(k);
                n.left = Val::Px(px.x);
                n.top = Val::Px(px.y);
            }
            None => commands.entity(e).despawn(),
        }
    }
    for (i, px) in shown {
        // VerticalBox_32: the skull (its brush's default 32 px) over the text, both centred.
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(px.x), top: Val::Px(px.y), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
            UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
            DeathWidget(i),
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
            children![
                (Node { width: Val::Px(32.0), height: Val::Px(32.0), ..default() }, ImageNode::new(assets.load(SKULL)).with_color(Color::linear_rgb(0.85, 0.85, 0.85))),
                crate::menus::widgets::label_colored(&font, "Death Location".to_string(), 14.0, Color::WHITE),
            ],
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::equipment::{PlayerInventory, Slot};

    #[test]
    fn the_bag_takes_everything_worn_and_carried() {
        let db = ItemDb::load();
        if db.items.is_empty() {
            return;
        }
        let (pack, food) = ("DA_HikingBackpack".to_string(), "DA_CannedBakedBeans".to_string());
        let mut inv = PlayerInventory::default();
        let mut bag = db.stack(&pack, 1);
        assert!(!bag.contents.is_empty(), "{pack} has grids");
        bag.contents[0].insert(&db, db.stack(&food, 1));
        inv.equipped.insert(Slot::Backpack, bag);
        inv.pockets.insert(&db, db.stack(&food, 2));
        let g = bag_grid(&db, &mut inv);
        assert_eq!(g.name, TITLE);
        assert_eq!(g.items.len(), 2);
        // The backpack keeps what was in it.
        let b = g.items.iter().find(|p| p.stack.item == pack).expect("backpack");
        assert_eq!(b.stack.contents[0].items.len(), 1);
        assert!(inv.equipped.is_empty() && inv.pockets.items.is_empty());
        assert_eq!(death_marker(DEATH_KEYS, Vec3::ZERO).title, "Death");
    }
}
