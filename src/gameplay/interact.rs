//! Searching containers and the inventory screens.
//!
//! Every `Container_*` / `PicklockContainer_*` actor in the level is lootable: hold Interact (F,
//! `IA_Interact` in IMC_General) for the class's `DurationToHold`, then its loot table is rolled
//! once per session and the loot screen opens. Locked `PicklockContainer_*`s take the
//! lockpicking minigame instead (`lockpick.rs`) and open when picked. Tab (`IA_Inventory`) opens
//! the inventory alone.
//! The screens themselves are `inventory_ui.rs`.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use super::equipment::PlayerInventory;
use super::inventory_ui::Dirty;
use super::items::{Grid, ItemDb};
use crate::level::Level;
use crate::menus::widgets::{MenuFont, label};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::CharacterModel;

const REACH: f32 = 2.4;
/// Loot grids are this wide (the loot column fits 13 cells); they grow downward as needed.
pub const LOOT_COLUMNS: u8 = 10;

/// Which gameplay screen is open over the world (gameplay input is paused while one is).
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameOverlay {
    #[default]
    None,
    Inventory,
    Loot(usize),
    /// Picking a container's lock.
    Lockpick(usize),
}

impl GameOverlay {
    pub fn is_open(&self) -> bool {
        *self != GameOverlay::None
    }

    /// The inventory screen (alone or beside a container's loot).
    pub fn shows_inventory(&self) -> bool {
        matches!(self, GameOverlay::Inventory | GameOverlay::Loot(_))
    }
}

/// The survivor's inventory lives in the session (`CurrentGame::inventory`) so saves carry it.
pub fn player_inv(game: &mut CurrentGame) -> &mut PlayerInventory {
    game.inventory.get_or_insert_with(PlayerInventory::default)
}

/// Level containers (actor index, position, class) and what's been found in them this session.
#[derive(Resource, Default)]
pub struct Containers {
    list: Vec<(usize, Vec3, String)>,
    pub searched: HashMap<usize, Grid>,
    /// Picklock containers' locks (rolled when first tried) and the ones picked open.
    pub locks: HashMap<usize, super::lockpick::Lock>,
    pub unlocked: HashSet<usize>,
    /// A container to open right away (just picked).
    pub open_now: Option<usize>,
    /// Dead zombies are containers too (BP_MasterZombie's BP_JigMultiplayer): their keys in
    /// `list`, from [`CORPSE_KEYS`] up, and the corpse entities.
    corpses: HashMap<usize, Entity>,
    next_corpse: usize,
    collected: bool,
}

/// Container keys for corpses start above any level actor index.
pub const CORPSE_KEYS: usize = 1 << 32;

/// A container class: a level container's (items.json) or a dead zombie class's, whose grid is
/// filled from its AI loot table and opens without a hold (OnExecuteInteract).
pub fn container_def(db: &ItemDb, class: &str) -> Option<super::items::ContainerDef> {
    if let Some(d) = db.containers.get(class) {
        return Some(d.clone());
    }
    let c = super::zombies::ZombieKind::from_class(class)?.class();
    Some(super::items::ContainerDef { name: c.container.clone().unwrap_or_else(|| "Dead Zombie".into()), table: c.loot_table.clone(), hold: 0.0, lockpick: false })
}

#[derive(Resource, Default)]
struct Search {
    target: Option<usize>,
    progress: f32,
}

#[derive(Component)]
pub(crate) struct Prompt;
#[derive(Component)]
pub(crate) struct PromptText;
#[derive(Component)]
pub(crate) struct PromptBar;

pub fn plugin(app: &mut App) {
    app.init_resource::<GameOverlay>()
        .init_resource::<Containers>()
        .init_resource::<Search>()
        .add_systems(OnEnter(AppState::InGame), (spawn_prompt, reset_session))
        .add_systems(
            Update,
            (collect_containers, track_corpses, search_containers, toggle_inventory, close_overlay.after(crate::menus::escape_goes_back), dress_player)
                .chain()
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(OnExit(MenuScreen::None), hide_prompt);
}

fn reset_session(mut overlay: ResMut<GameOverlay>, mut containers: ResMut<Containers>, mut search: ResMut<Search>) {
    *overlay = GameOverlay::None;
    *containers = Containers::default();
    *search = Search::default();
}

fn collect_containers(level: Option<Res<Level>>, db: Res<ItemDb>, mut containers: ResMut<Containers>) {
    let Some(level) = level else { return };
    if containers.collected {
        return;
    }
    containers.collected = true;
    let found: Vec<_> = level
        .actors
        .iter()
        .enumerate()
        .filter(|(_, a)| db.containers.contains_key(&a.class))
        .map(|(i, a)| (i, a.position(), a.class.clone()))
        .collect();
    containers.list.extend(found);
}

/// Register zombies as they die and drop corpses that rotted away.
#[allow(clippy::type_complexity)]
fn track_corpses(
    mut containers: ResMut<Containers>,
    died: Query<(Entity, &Transform, &crate::level::LevelZombie), Added<super::combat::Dead>>,
    alive: Query<(), With<super::combat::Dead>>,
    mut overlay: ResMut<GameOverlay>,
) {
    for (e, t, z) in &died {
        let key = CORPSE_KEYS + containers.next_corpse;
        containers.next_corpse += 1;
        containers.list.push((key, t.translation, z.kind.class_name().to_string()));
        containers.corpses.insert(key, e);
    }
    let gone: Vec<usize> = containers.corpses.iter().filter(|(_, e)| !alive.contains(**e)).map(|(k, _)| *k).collect();
    for k in gone {
        containers.corpses.remove(&k);
        containers.list.retain(|c| c.0 != k);
        containers.searched.remove(&k);
        if *overlay == GameOverlay::Loot(k) {
            *overlay = GameOverlay::None;
        }
    }
}

fn spawn_prompt(mut commands: Commands, font: Res<MenuFont>) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(160.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(6.0),
            ..default()
        },
        Visibility::Hidden,
        Prompt,
        DespawnOnExit(AppState::InGame),
        children![
            (label(&font, "", 16.0), PromptText),
            (
                Node { width: Val::Px(220.0), height: Val::Px(6.0), ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                children![(Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::WHITE), PromptBar)],
            ),
        ],
    ));
}

fn hide_prompt(mut prompts: Query<&mut Visibility, With<Prompt>>) {
    for mut v in &mut prompts {
        *v = Visibility::Hidden;
    }
}

#[allow(clippy::too_many_arguments)]
fn search_containers(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<State<MenuScreen>>,
    db: Res<ItemDb>,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
    mut containers: ResMut<Containers>,
    mut search: ResMut<Search>,
    mut overlay: ResMut<GameOverlay>,
    mut dirty: ResMut<Dirty>,
    mut prompt: Query<&mut Visibility, With<Prompt>>,
    mut text: Query<&mut Text, With<PromptText>>,
    mut bar: Query<&mut Node, With<PromptBar>>,
    mut game: ResMut<CurrentGame>,
    mut notify: MessageWriter<super::notify::Notify>,
) {
    if let Some(actor) = containers.open_now.take()
        && let Some((_, _, class)) = containers.list.iter().find(|c| c.0 == actor).cloned()
    {
        open_loot(&db, &mut containers, actor, &class);
        *overlay = GameOverlay::Loot(actor);
        dirty.0 = true;
    }
    let (Ok(p), Ok(mut vis)) = (player.single(), prompt.single_mut()) else { return };
    if *screen.get() != MenuScreen::None || overlay.is_open() {
        *vis = Visibility::Hidden;
        return;
    }
    let p = p.translation;
    let nearest = containers
        .list
        .iter()
        .filter(|(_, c, _)| (c.y - p.y).abs() < 2.5 && c.xz().distance(p.xz()) < REACH)
        .min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p)))
        .cloned();
    let Some((actor, _, class)) = nearest else {
        *vis = Visibility::Hidden;
        *search = Search::default();
        return;
    };
    let Some(def) = container_def(&db, &class) else { return };
    *vis = Visibility::Inherited;
    let searched = containers.searched.contains_key(&actor);

    if search.target != Some(actor) {
        *search = Search { target: Some(actor), progress: 0.0 };
    }
    // A picklock container still locked: F starts the lockpicking minigame.
    if def.lockpick && !searched && !containers.unlocked.contains(&actor) {
        if let Ok(mut t) = text.single_mut() {
            t.0 = format!("Press [F] to pick the lock of {}", def.name);
        }
        if let Ok(mut n) = bar.single_mut() {
            n.width = Val::Percent(0.0);
        }
        if keys.just_pressed(KeyCode::KeyF) {
            let lock = *containers.locks.entry(actor).or_insert_with(|| super::lockpick::Lock::roll(&mut rand::thread_rng()));
            match super::lockpick::can_try(&mut game, &lock) {
                Ok(()) => *overlay = GameOverlay::Lockpick(actor),
                Err(why) => {
                    notify.write(super::notify::Notify::new(why));
                }
            }
        }
        return;
    }
    let hold = if searched { 0.0 } else { def.hold };
    if keys.pressed(KeyCode::KeyF) {
        search.progress += time.delta_secs();
    } else {
        search.progress = 0.0;
    }
    let verb = if def.lockpick && !searched { "pick the lock of" } else if searched { "open" } else { "search" };
    if let Ok(mut t) = text.single_mut() {
        t.0 = format!("Hold [F] to {verb} {}", def.name);
    }
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(if hold > 0.0 { (search.progress / hold).min(1.0) * 100.0 } else { 0.0 });
    }
    if keys.pressed(KeyCode::KeyF) && search.progress >= hold {
        open_loot(&db, &mut containers, actor, &class);
        *overlay = GameOverlay::Loot(actor);
        dirty.0 = true;
        search.progress = 0.0;
    }
}

/// Roll a container's loot the first time it's opened.
fn open_loot(db: &ItemDb, containers: &mut Containers, actor: usize, class: &str) {
    let Some(def) = container_def(db, class) else { return };
    containers.searched.entry(actor).or_insert_with(|| {
        let mut g = Grid::new(def.name.clone(), LOOT_COLUMNS, 6);
        let mut rng = rand::thread_rng();
        for s in def.table.as_deref().map(|t| db.roll(t, &mut rng)).unwrap_or_default() {
            insert_growing(db, &mut g, s);
        }
        g
    });
}

fn toggle_inventory(keys: Res<ButtonInput<KeyCode>>, screen: Res<State<MenuScreen>>, mut overlay: ResMut<GameOverlay>, mut dirty: ResMut<Dirty>) {
    if *screen.get() == MenuScreen::None && keys.just_pressed(KeyCode::Tab) {
        *overlay = if overlay.is_open() { GameOverlay::None } else { GameOverlay::Inventory };
        dirty.0 = true;
    }
}

fn close_overlay(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<GameOverlay>, mut dirty: ResMut<Dirty>) {
    if overlay.is_open() && keys.just_pressed(KeyCode::Escape) {
        *overlay = GameOverlay::None;
        dirty.0 = true;
    }
}

/// Insert into a loot grid, adding rows until it fits (loot is never lost to a full grid).
pub fn insert_growing(db: &ItemDb, g: &mut Grid, mut stack: super::items::Stack) {
    for _ in 0..40 {
        match g.insert(db, stack) {
            None => return,
            Some(rest) => {
                stack = rest;
                g.height += 1;
            }
        }
    }
}

/// Keep the survivor dressed in whatever clothing is equipped.
fn dress_player(
    mut commands: Commands,
    mut game: ResMut<CurrentGame>,
    db: Res<ItemDb>,
    assets: Res<AssetServer>,
    data: Res<crate::menus::MenuData>,
    mut players: Query<(Entity, Option<&mut crate::models::WornClothing>, Option<&Children>), (With<CharacterModel>, With<crate::level::KinematicBody>)>,
    parts: Query<(), With<crate::models::ClothingPart>>,
) {
    let Ok((player, worn, kids)) = players.single_mut() else { return };
    if !game.is_changed() && worn.is_some() {
        return;
    }
    let female = game.profile.as_ref().is_some_and(|p| data.creator.body[0].options.get(p.body[0]).is_some_and(|s| s == "Female"));
    let want = crate::models::WornClothing(super::equipment::worn_meshes(player_inv(&mut game), &db, female));
    if worn.as_deref() == Some(&want) {
        return;
    }
    let old: Vec<Entity> = kids.into_iter().flat_map(|c| c.iter()).filter(|e| parts.contains(*e)).collect();
    let meshes: Vec<String> = want.0.iter().map(|(_, m)| m.clone()).collect();
    crate::models::dress(&mut commands, &assets, player, &old, &meshes);
    commands.entity(player).insert(want);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_zombies_are_loot_containers() {
        let Ok(text) = std::fs::read_to_string("assets/data/items.json") else { return };
        if super::super::zombies::zombie_classes().is_empty() {
            return;
        }
        let db: ItemDb = serde_json::from_str(&text).unwrap();
        let roamer = container_def(&db, "BP_Zombie_Roamer_C").unwrap();
        assert_eq!((roamer.name.as_str(), roamer.table.as_deref(), roamer.hold), ("Dead Zombie", Some("Zombie"), 0.0));
        let hound = container_def(&db, "BP_Zombie_Dog_C").unwrap();
        assert_eq!((hound.name.as_str(), hound.table.as_deref()), ("Dead Zombie Hound", Some("ZombieHound")));
        assert_eq!(container_def(&db, "BP_Zombie_Radiated_C").unwrap().table.as_deref(), Some("ZombieRadiated"));
        // Their AI loot tables exist and roll real items.
        for t in ["Zombie", "ZombieHound", "ZombieRadiated", "ZombieInfected"] {
            assert!(db.loot_tables.get(t).is_some_and(|rows| rows.iter().all(|r| db.items.contains_key(&r.item))), "{t}");
        }
        // Level containers still come from items.json.
        assert!(container_def(&db, "Nonexistent_C").is_none());
    }
}
