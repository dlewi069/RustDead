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
use rand::Rng;

use crate::progression::passive::Passive;

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
    /// A keypad door's code pad (door index).
    Keypad(usize),
    /// Reading a keypad door's note (door index).
    Note(usize),
    /// The journal (J), on its Skills page.
    Journal,
    /// The map (M).
    Map,
    /// Talking to a trader (its container key; `traders.rs`).
    Dialogue(usize),
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
    pub(crate) list: Vec<(usize, Vec3, String)>,
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
    /// Containers item spawners placed (`level::item_spawners`), by key: their instance name
    /// and the DT_UniqueLoot sets added to their loot.
    pub spawned: HashMap<usize, (Option<String>, Vec<String>)>,
    /// Quest crates (BP_GuaranteedSpawner, `quests.rs`): searching one opens it instead of a
    /// loot screen, and its key lands in `quest_opened`.
    pub quest_crates: HashSet<usize>,
    pub quest_opened: Vec<usize>,
    /// Dead wandering traders already searched (their DoOnce "All of the trader's loot is destroyed...").
    pub told: HashSet<usize>,
}

impl Containers {
    /// A container something left in the world this session (the Huntsman's Animal Meat bag):
    /// its key, from the corpse range; `name` overrides the class's.
    pub fn add_dropped(&mut self, at: Vec3, class: &str, name: Option<String>) -> usize {
        let key = CORPSE_KEYS + self.next_corpse;
        self.next_corpse += 1;
        self.list.push((key, at, class.to_string()));
        self.spawned.insert(key, (name, Vec::new()));
        key
    }
}

impl Containers {
    /// A container that came with a prefab (`level::prefabs::CONTAINER_KEYS` and up), with the
    /// DT_UniqueLoot sets its child actor template adds.
    pub fn add_at(&mut self, key: usize, at: Vec3, class: &str, unique: Vec<String>) {
        if self.list.iter().any(|c| c.0 == key) {
            return;
        }
        self.list.push((key, at, class.to_string()));
        self.spawned.insert(key, (None, unique));
    }
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
    Some(super::items::ContainerDef { name: c.container.clone().unwrap_or_else(|| "Dead Zombie".into()), table: c.loot_table.clone(), hold: 0.0, lockpick: false, noise: 0.0, mesh: None, airdrop: None })
}

#[derive(Resource, Default)]
pub(crate) struct Search {
    target: Option<usize>,
    progress: f32,
    /// Quick Search's NoiseEvent timer (0.075 s) and whether this press already made its noise.
    noise_tick: f32,
    noise_done: bool,
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

fn collect_containers(level: Option<Res<Level>>, spawns: Option<Res<crate::level::item_spawners::ItemSpawns>>, db: Res<ItemDb>, mut containers: ResMut<Containers>) {
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
    for s in spawns.iter().flat_map(|s| &s.0) {
        containers.list.push((s.key, s.transform.translation, s.class.to_string()));
        containers.spawned.insert(s.key, (s.name.clone(), s.unique.clone()));
    }
}

/// Register zombies as they die, follow the corpses (one killed mid-fall or on a step keeps
/// moving after it dies) and drop corpses that rotted away.
#[allow(clippy::type_complexity)]
fn track_corpses(
    mut containers: ResMut<Containers>,
    died: Query<(Entity, &Transform, &crate::level::LevelZombie), Added<super::combat::Dead>>,
    alive: Query<&Transform, With<super::combat::Dead>>,
    mut overlay: ResMut<GameOverlay>,
    spawners: Option<Res<crate::level::spawners::Spawners>>,
) {
    for (e, t, z) in &died {
        let key = CORPSE_KEYS + containers.next_corpse;
        containers.next_corpse += 1;
        containers.list.push((key, t.translation, z.kind.class_name().to_string()));
        containers.corpses.insert(key, e);
        // A boss's UserConstructionScript: AddUniqueLoot(Unqiue Items Set) joins its loot table.
        if let Some(b) = z.kind.boss() {
            containers.spawned.insert(key, (None, b.unique_sets.clone()));
        }
        // A spawning volume's UniqueLoot: AddUniqueLoot on what it spawned.
        if let (Some(i), Some(s)) = (z.spawner, spawners.as_ref())
            && !s.unique(i).is_empty()
        {
            containers.spawned.insert(key, (None, s.unique(i).to_vec()));
        }
    }
    let Containers { list, corpses, .. } = &mut *containers;
    for (key, at, _) in list.iter_mut().filter(|c| c.0 >= CORPSE_KEYS) {
        if let Some(t) = corpses.get(key).and_then(|e| alive.get(*e).ok()) {
            *at = t.translation;
        }
    }
    let gone: Vec<usize> = containers.corpses.iter().filter(|(_, e)| !alive.contains(**e)).map(|(k, _)| *k).collect();
    for k in gone {
        containers.corpses.remove(&k);
        containers.spawned.remove(&k);
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn search_containers(
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
    (level, power, store, mut noise, tdb, traders, driving, mut tasks): (
        Option<Res<crate::level::Level>>,
        Option<Res<crate::level::power::Power>>,
        Res<crate::menus::settings::SettingsStore>,
        MessageWriter<super::zombies::Noise>,
        Res<super::traders::TraderDb>,
        Res<super::traders::Traders>,
        Res<super::vehicles::Driving>,
        MessageWriter<super::quests::DataTask>,
    ),
) {
    if let Some(actor) = containers.open_now.take()
        && let Some((_, _, class)) = containers.list.iter().find(|c| c.0 == actor).cloned()
    {
        open_loot(&db, &mut containers, actor, &class);
        *overlay = GameOverlay::Loot(actor);
        dirty.0 = true;
    }
    let (Ok(p), Ok(mut vis)) = (player.single(), prompt.single_mut()) else { return };
    // In a vehicle nothing around is in reach (`vehicles::interact` owns F).
    if *screen.get() != MenuScreen::None || overlay.is_open() || driving.active() {
        *vis = Visibility::Hidden;
        return;
    }
    let p = p.translation;
    let nearest = containers
        .list
        .iter()
        .filter(|(_, c, _)| (c.y - p.y).abs() < 2.5 && c.xz().distance(p.xz()) < REACH)
        .min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p)))
        .cloned()
        // A nearer door or keypad note takes F (`level::doors::interact_doors`).
        .filter(|c| level.as_ref().and_then(|l| crate::level::doors::reach_distance(&l.doors, power.as_deref(), p)).is_none_or(|d| c.1.distance(p) <= d));
    let Some((actor, _, class)) = nearest else {
        *vis = Visibility::Hidden;
        *search = Search::default();
        return;
    };
    // A trader: F begins its dialogue (BP_TraderMaster.OnServerExecuteInteract).
    if super::traders::is_trader(actor) {
        let line = traders.get(&tdb, actor).map(|(_, c)| super::traders::prompt(c)).unwrap_or_default();
        *vis = Visibility::Inherited;
        if let Ok(mut t) = text.single_mut() {
            t.0 = line;
        }
        if let Ok(mut n) = bar.single_mut() {
            n.width = Val::Percent(0.0);
        }
        if keys.just_pressed(KeyCode::KeyF) {
            *overlay = super::traders::talk(&tdb, &traders, actor);
            dirty.0 = true;
            // BP_QuestGiver: BeginDialogue, then CompleteNarrativeDataTask(InteractWithActor, InteractionArgument).
            if let Some((_, c)) = traders.get(&tdb, actor).filter(|(_, c)| c.quest_giver) {
                tasks.write(super::quests::DataTask { task: "InteractWithActor".into(), argument: c.argument.clone(), quantity: 1 });
            }
        }
        return;
    }
    // A dead wandering trader: JigCanInteract is off; Interact tells once that its loot is gone.
    if actor >= CORPSE_KEYS && super::zombies::ZombieKind::from_class(&class).and_then(|k| k.human()).is_some_and(|h| h.trader) {
        *vis = Visibility::Inherited;
        if let Ok(mut t) = text.single_mut() {
            t.0 = "Press [F] to Interact".into();
        }
        if let Ok(mut n) = bar.single_mut() {
            n.width = Val::Percent(0.0);
        }
        if keys.just_pressed(KeyCode::KeyF) && containers.told.insert(actor) {
            notify.write(super::notify::Notify { text: super::traders::LOOT_DESTROYED.into(), secs: 6.0, color: Color::WHITE });
        }
        return;
    }
    let Some(mut def) = container_def(&db, &class) else { return };
    if let Some((Some(name), _)) = containers.spawned.get(&actor) {
        def.name = name.clone();
    }
    *vis = Visibility::Inherited;
    let searched = containers.searched.contains_key(&actor);

    if search.target != Some(actor) {
        *search = Search { target: Some(actor), ..default() };
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
    let passive = game.state.as_ref().map(|s| s.passive.clone()).unwrap_or_default();
    let scavenging = passive.multiplier(Passive::Scavenging);
    let hold = if searched { 0.0 } else { search_time(def.hold, scavenging, super::survival::difficulty(&store, "Loot / Environment", "SEARCH TIME MULTIPLIER")) };
    let holding = keys.pressed(KeyCode::KeyF);
    // IA_QuickSearch (Left Shift) while holding: the bar fills 2.85 x ScavengingMultiplier as fast,
    // and every 0.075 s there's a ChanceToProduceNoise / ThiefMultiplier % chance of a 40 m noise
    // (once per press).
    let quick = holding && hold > 0.0 && keys.pressed(KeyCode::ShiftLeft);
    if !quick {
        search.noise_tick = 0.0;
        search.noise_done = false;
    }
    if holding {
        search.progress += time.delta_secs() * if quick { QUICK_SEARCH * scavenging } else { 1.0 };
    } else {
        search.progress = 0.0;
    }
    if quick && !search.noise_done && def.noise > 0.0 {
        search.noise_tick += time.delta_secs();
        let mut rng = rand::thread_rng();
        while search.noise_tick >= NOISE_EVERY && !search.noise_done {
            search.noise_tick -= NOISE_EVERY;
            if rng.gen_range(0.0..100.0) <= def.noise / passive.multiplier(Passive::Thief).max(0.01) {
                search.noise_done = true;
                noise.write(super::zombies::Noise { at: p, range: QUICK_SEARCH_NOISE });
                info!("quick search made a noise ({})", def.name);
            }
        }
    }
    let verb = if def.lockpick && !searched { "pick the lock of" } else if searched { "open" } else { "search" };
    if let Ok(mut t) = text.single_mut() {
        t.0 = format!("Hold [F] to {verb} {}", def.name);
        // A crafting station opens on the press (ours: ShowInteractWidget's options aren't read).
        if class == super::inventory_ui::STATION {
            t.0 = format!("Press [F] to use {}", def.name);
        }
        // The widget's QuickSearch line, shown while the key is held.
        if holding && hold > 0.0 {
            t.0.push_str("\n[LeftShift] Quick Search");
        }
    }
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(if hold > 0.0 { (search.progress / hold).min(1.0) * 100.0 } else { 0.0 });
    }
    if holding && search.progress >= hold {
        // OpenContainer after a hold: AddXP_Scavenging(4-6).
        if hold > 0.0
            && let Some(st) = game.state.as_mut()
        {
            st.passive.add_xp(Passive::Scavenging, rand::thread_rng().gen_range(4.0..=6.0));
        }
        search.progress = 0.0;
        // BP_GuaranteedSpawner.OpenContainer: Opened, so it can't be searched again.
        if containers.quest_crates.remove(&actor) {
            containers.list.retain(|c| c.0 != actor);
            containers.quest_opened.push(actor);
            return;
        }
        open_loot(&db, &mut containers, actor, &class);
        *overlay = GameOverlay::Loot(actor);
        dirty.0 = true;
    }
}

/// Quick Search's bar speed (BP_LootContainerWidget: 2.85 x ScavengingMultiplier).
const QUICK_SEARCH: f32 = 2.85;
/// NoiseEvent's timer and ReportNoiseEvent range (4000 cm).
const NOISE_EVERY: f32 = 0.075;
const QUICK_SEARCH_NOISE: f32 = 40.0;

/// BP_LootContainer: DurationToHold / ScavengingMultiplier x SEARCH TIME MULTIPLIER.
pub fn search_time(hold: f32, scavenging: f32, setting: f32) -> f32 {
    hold / scavenging.max(0.01) * setting
}

/// Roll a container's loot the first time it's opened.
fn open_loot(db: &ItemDb, containers: &mut Containers, actor: usize, class: &str) {
    let Some(def) = container_def(db, class) else { return };
    let (name, unique) = containers.spawned.get(&actor).cloned().unwrap_or_default();
    containers.searched.entry(actor).or_insert_with(|| {
        let mut g = Grid::new(name.unwrap_or(def.name), LOOT_COLUMNS, 6);
        let mut rng = rand::thread_rng();
        let table = def.table.as_deref().map(|t| db.roll(t, &mut rng)).unwrap_or_default();
        // AddUniqueLoot: the spawner's sets join the container's starting items.
        let sets = unique.iter().flat_map(|u| db.roll_unique(u, &mut rng)).collect::<Vec<_>>();
        for s in table.into_iter().chain(sets) {
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
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn dress_player(
    mut commands: Commands,
    mut game: ResMut<CurrentGame>,
    db: Res<ItemDb>,
    assets: Res<AssetServer>,
    data: Res<crate::menus::MenuData>,
    mut players: Query<(Entity, Option<&mut crate::models::WornClothing>, Option<&crate::models::BodyMeshes>, Option<&Children>), (With<CharacterModel>, With<crate::level::KinematicBody>)>,
    parts: Query<(), With<crate::models::ClothingPart>>,
    body_parts: Query<&crate::models::BodyPart>,
) {
    let Ok((player, worn, body, kids)) = players.single_mut() else { return };
    if !game.is_changed() && worn.is_some() {
        return;
    }
    let female = game.profile.as_ref().is_some_and(|p| data.creator.body[0].options.get(p.body[0]).is_some_and(|s| s == "Female"));
    // Body parts under the clothing (skin poked through coats and trousers without this).
    let want_body = super::equipment::body_meshes(player_inv(&mut game), &db, female);
    if body != Some(&want_body)
        && let Some(profile) = game.profile.as_ref()
    {
        let have = body.cloned().unwrap_or_else(|| crate::models::BodyMeshes(crate::models::default_body(if female { "Female" } else { "Male" })));
        let current: Vec<(Entity, &'static str)> = kids.into_iter().flat_map(|c| c.iter()).filter_map(|e| body_parts.get(e).ok().map(|b| (e, b.0))).collect();
        crate::models::set_body(&mut commands, &assets, player, &current, &have, &want_body, &crate::models::skin_material(profile));
        let shown: Vec<&str> = want_body.0.iter().filter(|(_, m)| m.is_some()).map(|(b, _)| *b).collect();
        info!("body parts shown: {}", shown.join(" "));
        commands.entity(player).insert(want_body);
    }
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
    fn scavenging_and_the_setting_scale_search_time() {
        assert_eq!(search_time(4.0, 1.0, 1.0), 4.0);
        assert!((search_time(4.0, 1.025, 1.0) - 4.0 / 1.025).abs() < 1e-6);
        assert_eq!(search_time(4.0, 1.0, 0.5), 2.0, "SEARCH TIME MULTIPLIER");
    }

    #[test]
    fn corpses_are_looted_where_they_lie() {
        let mut app = App::new();
        app.init_resource::<Containers>().init_resource::<GameOverlay>().add_systems(Update, track_corpses);
        let t = Transform::from_xyz(1.0, 5.0, 2.0);
        let zombie = crate::level::LevelZombie { home: Vec3::ZERO, spawner: None, model: String::new(), material: None, kind: Default::default() };
        let e = app.world_mut().spawn((t, zombie, super::super::combat::Dead::at(&t))).id();
        app.update();
        // It settles 3 m lower after dying: its loot follows.
        app.world_mut().get_mut::<Transform>(e).unwrap().translation.y = 2.0;
        app.update();
        let c = app.world().resource::<Containers>();
        assert_eq!(c.list.iter().filter(|c| c.0 >= CORPSE_KEYS).map(|c| c.1).collect::<Vec<_>>(), [Vec3::new(1.0, 2.0, 2.0)]);
        // Gone: no longer a container.
        app.world_mut().despawn(e);
        app.update();
        assert!(app.world().resource::<Containers>().list.iter().all(|c| c.0 < CORPSE_KEYS));
    }

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

    #[test]
    fn spawned_briefcase_holds_its_unique_set() {
        let Ok(text) = std::fs::read_to_string("assets/data/items.json") else { return };
        let db: ItemDb = serde_json::from_str(&text).unwrap();
        let mut c = Containers::default();
        c.list.push((7, Vec3::ZERO, "Container_SpecialItemSpawner_C".into()));
        c.spawned.insert(7, (Some("Police Briefcase".into()), vec!["PoliceStationKeys".into()]));
        open_loot(&db, &mut c, 7, "Container_SpecialItemSpawner_C");
        let g = &c.searched[&7];
        assert_eq!(g.name, "Police Briefcase");
        assert!(g.items.iter().any(|p| p.stack.item == "DA_PoliceGeneratorKey"));
    }
}
