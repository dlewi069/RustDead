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
    /// Reading a BP_StandardNote (its index in `Level::notes`).
    Letter(usize),
    /// The journal (J), on its Skills page.
    Journal,
    /// The map (M).
    Map,
    /// Talking to a trader (its container key; `traders.rs`).
    Dialogue(usize),
    /// "Are you sure you want to pick up this container?" (a placed buildable's id; `storage.rs`).
    ConfirmPickup(usize),
    /// Renaming a placed container (its id; `storage.rs`).
    Rename(usize),
    /// SleepingUI (`sleep.rs`).
    Sleep,
    /// RespawnScreen, while dead (`respawn.rs`): Esc doesn't close it.
    Respawn,
    /// ScreenPermadeath, dead with PERMADEATH on (`respawn.rs`): Esc doesn't close it.
    Permadeath,
    /// RadioUI for a radio (`radio.rs`).
    Radio(super::radio::RadioOwner),
    /// CharacterBarberMenu at a barber (its index; `barber.rs`, which handles Esc: a focused text
    /// box lets go first).
    Barber(usize),
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
    /// Refilled while their loot screen was up (`loot_refill.rs`): the grid stays, but they're
    /// unsearched (ContainerLooted false) and the next search adds a new roll to it.
    pub refilled: HashSet<usize>,
    /// Marked As Storage Container (`loot_refill::mark`): never refilled.
    pub marked: HashSet<usize>,
}

impl Containers {
    /// ContainerLooted: searched, and not refilled since.
    pub fn looted(&self, key: usize) -> bool {
        self.searched.contains_key(&key) && !self.refilled.contains(&key)
    }

    /// A corpse's entity, by its key.
    pub fn corpse(&self, key: usize) -> Option<Entity> {
        self.corpses.get(&key).copied()
    }

    /// The level's containers (and the item spawners') are listed.
    pub fn collected(&self) -> bool {
        self.collected
    }
}

/// A container's HoldToInteract finished (or a picked lock opened it): BP_JigMultiplayer's
/// StartRefillTimer (`loot_refill.rs`).
#[derive(Message, Clone, Debug)]
pub struct Searched {
    pub key: usize,
    pub class: String,
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
    // A destroyed infestation's pile (JigCanInteract = IsDestroyed?), opened on a press.
    if let Some(d) = super::infestations::container(class) {
        return Some(d);
    }
    // Container_DeadPlayerLoot: OnExecuteInteract opens it on a press.
    if class == super::death_loot::CLASS {
        return Some(super::items::ContainerDef { name: super::death_loot::PROMPT_NAME.into(), table: None, hold: 0.0, lockpick: false, noise: 0.0, mesh: None, airdrop: None, tool: None, refills: false, magnifier: [0.0; 3] });
    }
    let c = super::zombies::ZombieKind::from_class(class)?.class();
    Some(super::items::ContainerDef { name: c.container.clone().unwrap_or_else(|| "Dead Zombie".into()), table: c.loot_table.clone(), hold: 0.0, lockpick: false, noise: 0.0, mesh: None, airdrop: None, tool: None, refills: false, magnifier: [0.0; 3] })
}

#[derive(Resource, Default)]
pub(crate) struct Search {
    target: Option<usize>,
    progress: f32,
    /// Quick Search's NoiseEvent timer (0.075 s) and whether this press already made its noise.
    noise_tick: f32,
    noise_done: bool,
    /// The selected interact option (the wheel; SetupWidget selects the first).
    option: usize,
}

#[derive(Component)]
pub(crate) struct Prompt;
#[derive(Component)]
pub(crate) struct PromptText;
#[derive(Component)]
pub(crate) struct PromptBar;

/// BPI_Player.PlayMontage on the survivor (Svr_Montage -> MC_Montage: played at once, over
/// whatever montage was playing): the interactions' Loot_* montages.
#[derive(Message, Clone, Copy, Debug)]
pub struct PlayerMontage(pub crate::models::ActionKind, pub Option<f32>);

impl PlayerMontage {
    /// At rate 1 (the montage's own).
    pub fn new(kind: crate::models::ActionKind) -> Self {
        Self(kind, None)
    }
}

fn play_montages(mut msgs: MessageReader<PlayerMontage>, mut player: Query<&mut crate::models::UpperAction, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>) {
    for m in msgs.read() {
        if let Ok(mut a) = player.single_mut() {
            a.start_at(m.0, m.1);
            info!("montage: {:?} at {:?}", m.0, m.1);
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_message::<PlayerMontage>()
        .add_message::<Searched>()
        .add_systems(Update, play_montages.run_if(in_state(AppState::InGame)))
        .init_resource::<GameOverlay>()
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
    mut searched_msg: MessageWriter<Searched>,
    (level, power, store, mut noise, tdb, traders, driving, mut tasks, mut view, mut montage, scroll, mut highlight): (
        Option<Res<crate::level::Level>>,
        Option<Res<crate::level::power::Power>>,
        Res<crate::menus::settings::SettingsStore>,
        MessageWriter<super::zombies::Noise>,
        Res<super::traders::TraderDb>,
        Res<super::traders::Traders>,
        Res<super::vehicles::Driving>,
        MessageWriter<super::quests::DataTask>,
        ResMut<super::search_widget::SearchView>,
        MessageWriter<PlayerMontage>,
        Res<bevy::input::mouse::AccumulatedMouseScroll>,
        ResMut<super::outline::Highlight>,
    ),
) {
    // BP_LootContainerWidget is only up while a loot container below fills it in (`show`).
    view.bypass_change_detection().shown = false;
    if let Some(actor) = containers.open_now.take()
        && let Some((_, _, class)) = containers.list.iter().find(|c| c.0 == actor).cloned()
    {
        // BP_MASTER_PicklockContainer, picked: SetInspected(true), StartRefillTimer.
        searched_msg.write(Searched { key: actor, class: class.clone() });
        open_loot(&db, &mut containers, actor, &class);
        *overlay = GameOverlay::Loot(actor);
        dirty.0 = true;
        montage.write_batch(open_montage(&db, &class).map(PlayerMontage::new));
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
        .filter(|c| level.as_ref().and_then(|l| crate::level::doors::reach_distance(&l.doors, &l.notes, power.as_deref(), p)).is_none_or(|d| c.1.distance(p) <= d));
    let Some((actor, _, class)) = nearest else {
        *vis = Visibility::Hidden;
        *search = Search::default();
        return;
    };
    // A trader: F begins its dialogue (BP_TraderMaster.OnServerExecuteInteract).
    if super::traders::is_trader(actor) {
        let line = traders.get(&tdb, actor).map(|(_, c)| super::traders::prompt(c)).unwrap_or_default();
        *vis = Visibility::Inherited;
        highlight.claim(super::outline::Highlit::Key(actor));
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
        highlight.claim(super::outline::Highlit::Key(actor));
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
    highlight.claim(super::outline::Highlit::Key(actor));
    let searched = containers.looted(actor);

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
    // A tool-required container (BP_MASTER_ToolRequiredContainer) not yet looted: no hold
    // without its tool (FindItem_Pure: "{A} not found").
    let instance_tool = level.as_ref().and_then(|l| l.actors.get(actor)).and_then(|a| a.props.as_ref()?.get("ToolNeeded").cloned());
    let tool = tool_needed(&def, instance_tool);
    // Loot containers (BP_LootContainer and its kin) show BP_LootContainerWidget; corpses and
    // crafting stations keep the plain prompt.
    let loot_widget = class != super::inventory_ui::STATION && super::zombies::ZombieKind::from_class(&class).is_none();
    let marked = containers.marked.contains(&actor);
    let storage = db.containers.contains_key(&class) && !containers.quest_crates.contains(&actor);
    let options = super::search_widget::options(&def, storage, marked);
    if scroll.delta.y != 0.0 && options.len() > 1 {
        search.option = (search.option + 1) % options.len();
    }
    let option = search.option.min(options.len() - 1);
    let mut show = |vis: &mut Visibility, progress: f32, held: bool| {
        if !loot_widget {
            return;
        }
        *vis = Visibility::Hidden;
        let next = super::search_widget::SearchView {
            shown: true,
            key: Some(actor),
            name: def.name.clone(),
            looted: searched,
            marked,
            loot_type: super::search_widget::loot_type_name(def.table.as_deref()).into(),
            held,
            progress,
            options: options.iter().map(|o| o.to_string()).collect(),
            selected: option,
        };
        if *view != next {
            *view = next;
        } else {
            view.bypass_change_detection().shown = true;
        }
    };
    if let Some(tool) = &tool
        && !searched
        && !tool.as_ref().is_some_and(|t| player_inv(&mut game).count(t) > 0)
    {
        if let Ok(mut t) = text.single_mut() {
            t.0 = format!("Hold [F] to search {}", def.name);
        }
        if let Ok(mut n) = bar.single_mut() {
            n.width = Val::Percent(0.0);
        }
        show(&mut vis, 0.0, false);
        search.progress = 0.0;
        if keys.just_pressed(KeyCode::KeyF) {
            let name = tool.as_ref().and_then(|t| db.items.get(t)).map_or("", |d| d.name.as_str());
            notify.write(super::notify::Notify::new(format!("{name} not found")));
        }
        return;
    }
    // Option2 (BP_LootContainer's Mark As Storage / Loot Container) is a press, never a search.
    if option == 1 {
        if let Ok(mut n) = bar.single_mut() {
            n.width = Val::Percent(0.0);
        }
        search.progress = 0.0;
        show(&mut vis, 0.0, false);
        if keys.just_pressed(KeyCode::KeyF) {
            match super::loot_refill::mark(&mut containers, actor) {
                Ok(on) => info!("loot: {actor} marked as {} container", if on { "storage" } else { "loot" }),
                Err(why) => {
                    notify.write(super::notify::Notify::new(why));
                }
            }
        }
        return;
    }
    let passive = game.state.as_ref().map(|s| s.passive.clone()).unwrap_or_default();
    let scavenging = passive.multiplier(Passive::Scavenging);
    // The tool-required master's MaxTime is DurationToHold / ScavengingMultiplier alone.
    let setting = if def.tool.is_some() { 1.0 } else { super::survival::difficulty(&store, "Loot / Environment", "SEARCH TIME MULTIPLIER") };
    let hold = if searched { 0.0 } else { search_time(def.hold, scavenging, setting) };
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
    }
    let progress = if hold > 0.0 { (search.progress / hold).min(1.0) } else { 0.0 };
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(progress * 100.0);
    }
    // The widget's QuickSearch line shows while the hold is on (IsHeld?).
    show(&mut vis, progress, holding && hold > 0.0);
    if holding && search.progress >= hold {
        // OpenContainer after a hold: AddXP_Scavenging(4-6).
        if hold > 0.0
            && let Some(st) = game.state.as_mut()
        {
            st.passive.add_xp(Passive::Scavenging, rand::thread_rng().gen_range(4.0..=6.0));
        }
        search.progress = 0.0;
        if def.tool.is_some() && !searched {
            let minus = 10.0 * super::survival::difficulty(&store, "General", "OTHER ITEMS DURABILITY MULTIPLIER");
            for line in tool_looted(&db, &mut game, tool.flatten().as_deref(), minus, &mut rand::thread_rng()) {
                notify.write(super::notify::Notify::new(line));
            }
        }
        montage.write_batch(open_montage(&db, &class).map(PlayerMontage::new));
        // BP_GuaranteedSpawner.OpenContainer: Opened, so it can't be searched again (its
        // PlayMontage(Loot_MidShelf_GrabItem_Montage) is `open_montage`'s: quests.rs registers it).
        if containers.quest_crates.remove(&actor) {
            containers.list.retain(|c| c.0 != actor);
            containers.quest_opened.push(actor);
            return;
        }
        // HoldToInteract done: StartRefillTimer.
        if hold > 0.0 {
            searched_msg.write(Searched { key: actor, class: class.clone() });
        }
        open_loot(&db, &mut containers, actor, &class);
        *overlay = GameOverlay::Loot(actor);
        dirty.0 = true;
    }
}

/// The montage opening a container plays: Loot_MidShelf_GrabItem for BP_LootContainer and its
/// children (the level's, picklock and tool-required ones, airdrops, the briefcases), the quest
/// crate (BP_GuaranteedSpawner), the death crate and the infestation piles; none for corpses, the crafting / cooking stations, traders.
pub fn open_montage(db: &ItemDb, class: &str) -> Option<crate::models::ActionKind> {
    let grabs = db.containers.contains_key(class) || class == super::death_loot::CLASS || super::infestations::db().classes.contains_key(class);
    grabs.then_some(crate::models::ActionKind::Grab)
}

/// The tool a tool-required container's search needs: `None` when it needs none, `Some(None)`
/// when ToolNeeded? is set with no tool named (FindItem_Pure then never finds it).
pub fn tool_needed(def: &super::items::ContainerDef, instance: Option<String>) -> Option<Option<String>> {
    def.tool.as_ref().filter(|t| t.required).map(|t| instance.or_else(|| t.item.clone()))
}

/// BP_MASTER_ToolRequiredContainer's HoldToInteract, done: the tool loses 10 x OTHER ITEMS
/// DURABILITY MULTIPLIER (ReduceDurabilityOfFoundItem: only while it has more; "No durability - {name}"
/// when it has no more than that, the container opening anyway), AddXP_Thief(40-60) (Scavenging's
/// 4-6 is every search's), and the first time Level x 100 XP: "Container Looted, XP Given: {XP}".
/// Returns the toasts.
pub fn tool_looted(db: &ItemDb, game: &mut CurrentGame, tool: Option<&str>, minus: f32, rng: &mut impl Rng) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(t) = tool {
        let max = db.items.get(t).map_or(0.0, |d| d.durability);
        if let Some(s) = player_inv(game).find_mut(t)
            && max != 0.0
        {
            let now = s.durability.unwrap_or(max);
            if now > minus {
                s.durability = Some(now - minus);
            } else {
                out.push(format!("No durability - {}", db.items.get(t).map_or(t, |d| d.name.as_str())));
            }
        }
    }
    if let Some(st) = game.state.as_mut() {
        st.passive.add_xp(Passive::Thief, rng.gen_range(40.0..=60.0));
        let xp = st.progression.level.max(1) as u64 * 100;
        st.progression.add_xp(xp);
        out.push(format!("Container Looted, XP Given: {xp}"));
    }
    out
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

/// Roll a container's loot the first time it's opened, and again after a refill (into what a
/// refill left in it while its loot screen was up: ours, see `loot_refill.rs`).
pub(crate) fn open_loot(db: &ItemDb, containers: &mut Containers, actor: usize, class: &str) {
    let Some(def) = container_def(db, class) else { return };
    let refilled = containers.refilled.remove(&actor);
    if containers.searched.contains_key(&actor) && !refilled {
        return;
    }
    let (name, unique) = containers.spawned.get(&actor).cloned().unwrap_or_default();
    let mut rng = rand::thread_rng();
    let table = def.table.as_deref().map(|t| db.roll(t, &mut rng)).unwrap_or_default();
    // AddUniqueLoot: the spawner's sets join the container's starting items.
    let sets = unique.iter().flat_map(|u| db.roll_unique(u, &mut rng)).collect::<Vec<_>>();
    let g = containers.searched.entry(actor).or_insert_with(|| Grid::new(name.unwrap_or(def.name), LOOT_COLUMNS, 6));
    for s in table.into_iter().chain(sets) {
        insert_growing(db, g, s);
    }
}

fn toggle_inventory(keys: Res<ButtonInput<KeyCode>>, screen: Res<State<MenuScreen>>, mut overlay: ResMut<GameOverlay>, mut dirty: ResMut<Dirty>) {
    // Tab is a key like any other while typing a name.
    if *screen.get() == MenuScreen::None && keys.just_pressed(KeyCode::Tab) && !matches!(*overlay, GameOverlay::Rename(_) | GameOverlay::Barber(_) | GameOverlay::Respawn | GameOverlay::Permadeath) {
        *overlay = if overlay.is_open() { GameOverlay::None } else { GameOverlay::Inventory };
        dirty.0 = true;
    }
}

fn close_overlay(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<GameOverlay>, mut dirty: ResMut<Dirty>) {
    if overlay.is_open() && !matches!(*overlay, GameOverlay::Respawn | GameOverlay::Permadeath | GameOverlay::Barber(_)) && keys.just_pressed(KeyCode::Escape) {
        *overlay = GameOverlay::None;
        dirty.0 = true;
    }
}

/// Insert into a loot grid, adding rows until it fits (loot is never lost to a full grid).
pub fn insert_growing(db: &ItemDb, g: &mut Grid, mut stack: super::items::Stack) {
    if !g.accepts(&stack.item) {
        return;
    }
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
pub(crate) fn dress_player(
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
    fn opening_containers_grabs_but_corpses_and_stations_dont() {
        let Ok(text) = std::fs::read_to_string("assets/data/items.json") else { return };
        let db: ItemDb = serde_json::from_str(&text).unwrap();
        let grab = Some(crate::models::ActionKind::Grab);
        for c in ["Container_ResidentialFoodLoot_Sink_C", "AirdropContainer_Military_C", "PicklockContainer_Safe_C", "BP_ToolRequired_GunLocker_C", super::super::death_loot::CLASS] {
            assert_eq!(open_montage(&db, c), grab, "{c}");
        }
        for c in ["BP_Zombie_Roamer_C", "BP_CraftingStation_C", "BP_MilitaryTrader_C"] {
            assert_eq!(open_montage(&db, c), None, "{c}");
        }
    }

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
    fn tool_containers_need_their_tool() {
        let Ok(text) = std::fs::read_to_string("assets/data/items.json") else { return };
        let db: ItemDb = serde_json::from_str(&text).unwrap();
        let def = |c: &str| container_def(&db, c).unwrap();
        // The axe cabinet needs nothing; the gun locker its Bolt Cutters; the second gun locker's
        // instances name Wire Cutters (none named: never found).
        assert_eq!(tool_needed(&def("BP_ToolRequired_Axe_C"), None), None);
        assert_eq!(tool_needed(&def("BP_ToolRequired_GunLocker_C"), None), Some(Some("DA_BoltCutters".into())));
        assert_eq!(tool_needed(&def("BP_ToolRequired_GunLocker2_C"), Some("DA_WireCutters".into())), Some(Some("DA_WireCutters".into())));
        assert_eq!(tool_needed(&def("BP_ToolRequired_GunLocker2_C"), None), Some(None));
        assert_eq!((def("BP_ToolRequired_Axe_C").name.as_str(), def("BP_ToolRequired_Axe_C").hold), ("Emergency Axe", 12.0));
        assert!(db.loot_tables["Axe"].iter().any(|r| r.item == "DA_FiremansAxe"));
        // Looted: the cutters lose 10, the XP is Level x 100.
        let mut game = CurrentGame { state: Some(crate::engine::game::GameState::new()), ..default() };
        player_inv(&mut game).insert(&db, db.stack("DA_BoltCutters", 1));
        let toasts = tool_looted(&db, &mut game, Some("DA_BoltCutters"), 10.0, &mut rand::thread_rng());
        assert_eq!(player_inv(&mut game).find_mut("DA_BoltCutters").unwrap().durability, Some(90.0));
        let level = game.state.as_ref().unwrap().progression.level.max(1);
        assert_eq!(toasts, [format!("Container Looted, XP Given: {}", level * 100)]);
        // Worn down to 10 or less: no wear, a message, the container opens anyway.
        player_inv(&mut game).find_mut("DA_BoltCutters").unwrap().durability = Some(10.0);
        let toasts = tool_looted(&db, &mut game, Some("DA_BoltCutters"), 10.0, &mut rand::thread_rng());
        assert_eq!(toasts[0], "No durability - Bolt Cutters");
        assert_eq!(player_inv(&mut game).find_mut("DA_BoltCutters").unwrap().durability, Some(10.0));
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
