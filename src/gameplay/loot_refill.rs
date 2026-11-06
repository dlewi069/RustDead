//! LOOT REGENERATION: BP_JigMultiplayer's refill timer as BP_LootContainer,
//! BP_MASTER_PicklockContainer and BP_MASTER_ToolRequiredContainer drive it (decompiled):
//!
//! - The setting: LOOT REGENERATION DISABLED ON LOAD? is a DefaultCheckBoxSetting on the
//!   Difficulty.Refill cvar (registered true): ticked = Refill true = the timers run and a load
//!   keeps what was searched; unticked = every load resets the loot and nothing refills. (Until
//!   2026-11-01 ours read it the other way round.)
//! - BeginPlay: RefillTimer = Difficulty.RefillTime (LOOT REFILL TIMER (MINUTES), 30) x 60. Without
//!   Difficulty.Refill, ContainerLooted, Looted? and DoesContainerRefillLoot? are set false, so
//!   nothing ever refills. With it, the component's DoesContainerRefillLoot? stands: true, but
//!   false on Container_AnimalMeat, _CompoundCrate, _POICrate, _SpecialItemSpawner (the
//!   briefcases) and _LaptopEventCrate (items.json `refills`); Bind_Refill is bound and 1.5-5 s
//!   later CheckContainerRefill starts the timer.
//! - HoldToInteract done, or a lock picked: StartRefillTimer restarts it (IsLootContainer and
//!   DoesContainerRefillLoot?), `interact::Searched`.
//! - ProcessRefillContainer, when it fires, only if DoesContainerRefillLoot?: OnRefillContainer
//!   (Bind_Refill: unless MarkedAsStorageContainer, ContainerLooted false and SetInspected: no
//!   "[Looted]"), ClearContainer unless the container's inventory is up, Looted? false (the next
//!   open rolls new loot), and the timer again (RefillTime x 60). Its "Refilled" PrintString only
//!   shows in development builds.
//! - Mark As Storage Container (BP_LootContainer's Option2; `interact.rs` presses it): on an
//!   unsearched container "Container Not Searched"; else MarkedAsStorageContainer flips, the
//!   widget's AdditionalTxt reads "[Storage]" / "[Looted]" and the option "Mark As Loot
//!   Container" / "Mark As Storage Container". Marking sets DoesContainerRefillLoot? false (a
//!   timer still running then does nothing when it fires, and isn't re-armed); unmarking sets it
//!   true again only with Difficulty.Refill, and starts no timer.
//! - ActorPreSave keeps RefillTimer = the timer's remaining time; EasyMultiSave keeps the
//!   container's items, ContainerLooted and MarkedAsStorageContainer. ActorLoaded: a marked one
//!   stays looted and never refills, whatever the setting; else with Difficulty.Refill Looted? =
//!   ContainerLooted and StartRefillTimer from the remaining time (all of it when none ran);
//!   without it ContainerLooted false and ClearContainer.
//! - Both settings are read at BeginPlay, so a change applies from the next load.
//!
//! Ours: only searched containers keep a timer (an unsearched one's refill changes nothing). One
//! refilled while its loot screen is up keeps its items, and its next search adds a new roll to
//! them (where the game's new roll goes isn't readable). Saved (`GameState::loot`): the level's
//! containers and the item spawners' briefcases and caches (keys below `CORPSE_KEYS`); the
//! prefab sites' containers are rolled again each session, so theirs aren't. Unmarking keeps the
//! class's own DoesContainerRefillLoot? (the game's sets it true even on a briefcase, to no
//! effect without a timer).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::interact::{CORPSE_KEYS, Containers, GameOverlay, Searched, container_def};
use super::items::{Grid, ItemDb};
use crate::menus::AppState;
use crate::menus::data::CurrentGame;
use crate::menus::settings::{SettingsStore, Value, row_key};

const PAGE: &str = "Loot / Environment";
pub const DISABLED_ON_LOAD: &str = "LOOT REGENERATION DISABLED ON LOAD?";
pub const REFILL_MINUTES: &str = "LOOT REFILL TIMER (MINUTES)";
/// OnExecuteInteract(Option2) on a container not yet searched.
pub const NOT_SEARCHED: &str = "Container Not Searched";

/// The refill timers, by container key (game seconds when each fires).
#[derive(Resource, Default, Debug)]
pub struct Refill {
    /// Difficulty.Refill when this session began: the timers run, and the save keeps the loot.
    pub enabled: bool,
    pub due: HashMap<usize, f64>,
    /// The save's containers are back (ActorLoaded), and these keys are saved.
    pub loaded: bool,
    pub saved: HashSet<usize>,
}

/// The loot containers' saved state (`GameState::loot`): ActorPreSave's RefillTimer and what
/// EasyMultiSave keeps of each, by container key.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LootSave {
    /// Searched containers' items (ContainerLooted, unless in `refilled`).
    pub searched: BTreeMap<usize, Grid>,
    /// Refilled while their loot screen was up: the items stay, unsearched.
    #[serde(default)]
    pub refilled: BTreeSet<usize>,
    /// Seconds left on each running refill timer.
    #[serde(default)]
    pub timers: BTreeMap<usize, f64>,
    /// MarkedAsStorageContainer.
    #[serde(default)]
    pub marked: BTreeSet<usize>,
}

/// Mark As Storage Container on `key`: the new MarkedAsStorageContainer, or the notice when it
/// isn't searched (ContainerLooted false; a marked one always counts as looted).
pub fn mark(containers: &mut Containers, key: usize) -> Result<bool, &'static str> {
    if !containers.looted(key) {
        return Err(NOT_SEARCHED);
    }
    if containers.marked.remove(&key) {
        Ok(false)
    } else {
        containers.marked.insert(key);
        Ok(true)
    }
}

/// ActorPreSave for the saved keys: what's searched, refilled, marked and the timers' time left.
pub fn save_into(save: &mut LootSave, containers: &Containers, refill: &Refill, now: f64) -> bool {
    let keep = |k: &usize| refill.saved.contains(k);
    let mut changed = false;
    for (k, g) in containers.searched.iter().filter(|(k, _)| keep(k)) {
        if save.searched.get(k) != Some(g) {
            save.searched.insert(*k, g.clone());
            changed = true;
        }
    }
    let before = save.searched.len();
    save.searched.retain(|k, _| containers.searched.contains_key(k));
    changed |= save.searched.len() != before;
    let refilled: BTreeSet<usize> = containers.refilled.iter().copied().filter(keep).collect();
    let marked: BTreeSet<usize> = containers.marked.iter().copied().filter(keep).collect();
    if save.refilled != refilled || save.marked != marked {
        save.refilled = refilled;
        save.marked = marked;
        changed = true;
    }
    // The time left changes every frame; it isn't a change worth telling anyone about.
    save.timers = refill.due.iter().filter(|(k, _)| keep(k)).map(|(k, t)| (*k, (t - now).max(0.0))).collect();
    changed
}

/// ActorLoaded on the saved containers: marked ones come back looted, without a timer; with
/// Difficulty.Refill the rest come back as they were, timers running on from what was left (or
/// a whole `period`); without it they stay as BeginPlay leaves them, unsearched and empty.
pub fn restore(save: &LootSave, containers: &mut Containers, refill: &mut Refill, refills: impl Fn(usize) -> bool, now: f64, period: f64) {
    for (k, g) in &save.searched {
        let marked = save.marked.contains(k);
        if !marked && !refill.enabled {
            continue;
        }
        containers.searched.insert(*k, g.clone());
        if marked {
            containers.marked.insert(*k);
            continue;
        }
        if save.refilled.contains(k) {
            containers.refilled.insert(*k);
        }
        if refills(*k) {
            refill.due.insert(*k, now + save.timers.get(k).copied().unwrap_or(period));
        }
    }
}

impl Refill {
    /// StartRefillTimer: (re)arm `key` to fire `period` s from `now`, if it refills.
    pub fn start(&mut self, key: usize, refills: bool, now: f64, period: f64) {
        if self.enabled && refills {
            self.due.insert(key, now + period);
        }
    }

    /// The timers that have fired by `now`, earliest first (taken out).
    pub fn take_due(&mut self, now: f64) -> Vec<usize> {
        let mut out: Vec<(f64, usize)> = self.due.iter().filter(|(_, t)| **t <= now).map(|(k, t)| (*t, *k)).collect();
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, k) in &out {
            self.due.remove(k);
        }
        out.into_iter().map(|(_, k)| k).collect()
    }
}

/// What a refill did to a container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refilled {
    /// ClearContainer: its items are gone, and the next search rolls new loot.
    Cleared,
    /// Its loot screen was up: the items stay, but it's unsearched (and the timer runs again).
    Kept,
}

/// ProcessRefillContainer on `key`; `open`: its loot screen is up (IsInventoryVisible?).
pub fn refill(containers: &mut Containers, key: usize, open: bool) -> Refilled {
    if open && containers.searched.contains_key(&key) {
        containers.refilled.insert(key);
        Refilled::Kept
    } else {
        containers.searched.remove(&key);
        containers.refilled.remove(&key);
        Refilled::Cleared
    }
}

/// RefillTime x 60.
pub fn period(store: &SettingsStore) -> f64 {
    super::survival::difficulty(store, PAGE, REFILL_MINUTES) as f64 * 60.0
}

/// Difficulty.Refill: LOOT REGENERATION DISABLED ON LOAD? ticked (the default).
pub fn regeneration_on(store: &SettingsStore) -> bool {
    store.find(&row_key("difficulty", PAGE, DISABLED_ON_LOAD)) != Some(Value::Bool(false))
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Refill>()
        .add_systems(OnEnter(AppState::InGame), begin_play)
        .add_systems(Update, (load, run_refills, save).chain().after(super::interact::search_containers).run_if(in_state(AppState::InGame)));
}

/// BeginPlay: the setting as the session starts.
fn begin_play(store: Res<SettingsStore>, mut refill: ResMut<Refill>) {
    *refill = Refill { enabled: regeneration_on(&store), ..default() };
    info!("loot refill: regeneration {}", if refill.enabled { "on" } else { "off" });
}

/// ActorLoaded, once the level's containers are listed: the save's loot back.
fn load(time: Res<Time>, db: Res<ItemDb>, store: Res<SettingsStore>, game: Res<CurrentGame>, mut containers: ResMut<Containers>, mut refill: ResMut<Refill>) {
    if refill.loaded || !containers.collected() {
        return;
    }
    refill.loaded = true;
    refill.saved = containers.list.iter().filter(|c| c.0 < CORPSE_KEYS && db.containers.contains_key(&c.2)).map(|c| c.0).collect();
    let Some(save) = game.state.as_ref().map(|s| s.loot.clone()) else { return };
    let classes: HashMap<usize, String> = containers.list.iter().filter(|c| save.searched.contains_key(&c.0)).map(|c| (c.0, c.2.clone())).collect();
    let refills = |k: usize| classes.get(&k).and_then(|c| container_def(&db, c)).is_some_and(|d| d.refills);
    let saved = refill.saved.clone();
    let save = LootSave { searched: save.searched.into_iter().filter(|(k, _)| saved.contains(k)).collect(), ..save };
    restore(&save, &mut containers, &mut refill, refills, time.elapsed_secs_f64(), period(&store));
    info!("loot refill: loaded {} searched, {} marked, {} timers", save.searched.len(), save.marked.len(), refill.due.len());
}

/// ActorPreSave, kept current like the other savers.
fn save(time: Res<Time>, containers: Res<Containers>, refill: Res<Refill>, mut game: ResMut<CurrentGame>) {
    if !refill.loaded {
        return;
    }
    let Some(state) = game.bypass_change_detection().state.as_mut() else { return };
    if save_into(&mut state.loot, &containers, &refill, time.elapsed_secs_f64()) {
        game.set_changed();
    }
}

pub fn run_refills(
    time: Res<Time>,
    db: Res<ItemDb>,
    store: Res<SettingsStore>,
    overlay: Res<GameOverlay>,
    mut msgs: MessageReader<Searched>,
    mut containers: ResMut<Containers>,
    mut timers: ResMut<Refill>,
) {
    let now = time.elapsed_secs_f64();
    let period = period(&store);
    for m in msgs.read() {
        let refills = container_def(&db, &m.class).is_some_and(|d| d.refills);
        timers.start(m.key, refills, now, period);
        if timers.due.contains_key(&m.key) {
            info!("loot refill: {} armed for {period:.0} s", m.key);
        }
    }
    for key in timers.take_due(now) {
        // DoesContainerRefillLoot? is off while it's marked: the timer just ends.
        if containers.marked.contains(&key) {
            info!("loot refill: {key} marked, no refill");
            continue;
        }
        let r = refill(&mut containers, key, *overlay == GameOverlay::Loot(key));
        info!("loot refill: {key} {r:?}");
        if r == Refilled::Kept {
            timers.start(key, true, now, period);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::items::Grid;

    fn on() -> Refill {
        Refill { enabled: true, ..default() }
    }

    #[test]
    fn a_search_arms_the_timer_only_with_regeneration_on_and_a_refilling_class() {
        let mut r = Refill::default();
        r.start(1, true, 0.0, 1800.0);
        assert!(r.due.is_empty(), "without Difficulty.Refill DoesContainerRefillLoot? is false");
        let mut r = on();
        r.start(1, false, 0.0, 1800.0);
        assert!(r.due.is_empty(), "the briefcases, POI crates... never refill");
        r.start(1, true, 10.0, 1800.0);
        assert_eq!(r.due[&1], 1810.0);
        // A second search restarts it (ClearTimer, then CheckContainerRefill).
        r.start(1, true, 100.0, 1800.0);
        assert_eq!(r.due[&1], 1900.0);
    }

    #[test]
    fn timers_fire_once_in_order() {
        let mut r = on();
        r.start(1, true, 0.0, 20.0);
        r.start(2, true, 0.0, 10.0);
        r.start(3, true, 0.0, 30.0);
        assert!(r.take_due(9.9).is_empty());
        assert_eq!(r.take_due(25.0), vec![2, 1]);
        assert_eq!(r.take_due(25.0), Vec::<usize>::new());
        assert_eq!(r.due.len(), 1);
    }

    #[test]
    fn a_refill_clears_a_closed_container_and_unsearches_an_open_one() {
        let mut c = Containers::default();
        c.searched.insert(5, Grid::new("Cupboard", 6, 6));
        c.searched.insert(6, Grid::new("Desk", 6, 6));
        assert!(c.looted(5) && c.looted(6));
        assert_eq!(refill(&mut c, 5, false), Refilled::Cleared);
        assert!(!c.looted(5) && !c.searched.contains_key(&5));
        assert_eq!(refill(&mut c, 6, true), Refilled::Kept);
        assert!(!c.looted(6) && c.searched.contains_key(&6), "IsInventoryVisible?: no ClearContainer");
        // The next refill with the screen closed clears it.
        assert_eq!(refill(&mut c, 6, false), Refilled::Cleared);
        assert!(!c.searched.contains_key(&6) && !c.refilled.contains(&6));
    }

    #[test]
    fn regeneration_follows_the_setting_and_the_timer_its_minutes() {
        let mut store = SettingsStore::default();
        assert!(regeneration_on(&store), "Difficulty.Refill is registered true");
        store.values.insert(row_key("difficulty", PAGE, DISABLED_ON_LOAD), Value::Bool(false));
        assert!(!regeneration_on(&store), "unticked: the loot resets on load and nothing refills");
        store.values.insert(row_key("difficulty", PAGE, DISABLED_ON_LOAD), Value::Bool(true));
        assert!(regeneration_on(&store));
        store.values.insert(row_key("difficulty", PAGE, REFILL_MINUTES), Value::Num(2.0));
        assert_eq!(period(&store), 120.0);
    }

    #[test]
    fn only_a_searched_container_can_be_marked_and_unmarked() {
        let mut c = Containers::default();
        assert_eq!(mark(&mut c, 3), Err(NOT_SEARCHED));
        c.searched.insert(3, Grid::new("Fridge", 6, 6));
        assert_eq!(mark(&mut c, 3), Ok(true));
        assert!(c.marked.contains(&3) && c.looted(3));
        assert_eq!(mark(&mut c, 3), Ok(false));
        assert!(c.marked.is_empty());
        // Refilled with its screen up: ContainerLooted is false again.
        c.refilled.insert(3);
        assert_eq!(mark(&mut c, 3), Err(NOT_SEARCHED));
    }

    fn searched() -> (Containers, Refill) {
        let mut c = Containers::default();
        let mut r = on();
        r.saved = [1, 2, 3, 4].into();
        for k in [1, 2, 3, CORPSE_KEYS + 1] {
            c.searched.insert(k, Grid::new("Box", 6, 6));
        }
        c.marked.insert(2);
        c.refilled.insert(3);
        r.due.insert(1, 100.0);
        r.due.insert(CORPSE_KEYS + 1, 50.0);
        (c, r)
    }

    #[test]
    fn the_save_keeps_the_saved_keys_and_the_time_left() {
        let (c, r) = searched();
        let mut save = LootSave::default();
        assert!(save_into(&mut save, &c, &r, 40.0));
        assert_eq!(save.searched.keys().copied().collect::<Vec<_>>(), vec![1, 2, 3], "corpses aren't saved");
        assert_eq!((save.marked.iter().copied().collect::<Vec<_>>(), save.refilled.iter().copied().collect::<Vec<_>>()), (vec![2], vec![3]));
        assert_eq!(save.timers.get(&1), Some(&60.0));
        assert_eq!(save.timers.len(), 1);
        // Nothing new: no change; the timers still count down.
        assert!(!save_into(&mut save, &c, &r, 50.0));
        assert_eq!(save.timers.get(&1), Some(&50.0));
        // A refill cleared one.
        let mut c = c;
        c.searched.remove(&1);
        assert!(save_into(&mut save, &c, &r, 50.0));
        assert!(!save.searched.contains_key(&1));
    }

    #[test]
    fn a_load_keeps_the_loot_with_regeneration_and_only_the_marked_without() {
        let (c, r) = searched();
        let mut save = LootSave::default();
        save_into(&mut save, &c, &r, 40.0);
        // Difficulty.Refill: all back, the timer from what was left, a whole period for one
        // that had none; never for a marked one.
        let (mut c2, mut r2) = (Containers::default(), on());
        restore(&save, &mut c2, &mut r2, |_| true, 1000.0, 1800.0);
        assert!(c2.looted(1) && c2.looted(2) && c2.searched.contains_key(&3) && !c2.looted(3));
        assert!(c2.marked.contains(&2));
        assert_eq!(r2.due.get(&1), Some(&1060.0));
        assert_eq!(r2.due.get(&3), Some(&2800.0));
        assert!(!r2.due.contains_key(&2), "DoesContainerRefillLoot? false");
        // A class that never refills gets no timer.
        let (mut c3, mut r3) = (Containers::default(), on());
        restore(&save, &mut c3, &mut r3, |k| k != 1, 0.0, 1800.0);
        assert!(c3.looted(1) && !r3.due.contains_key(&1));
        // Without it: ClearContainer on all but the marked one.
        let (mut c4, mut r4) = (Containers::default(), Refill::default());
        restore(&save, &mut c4, &mut r4, |_| true, 0.0, 1800.0);
        assert_eq!(c4.searched.keys().copied().collect::<Vec<_>>(), vec![2]);
        assert!(c4.marked.contains(&2) && c4.looted(2) && r4.due.is_empty());
    }

    #[test]
    fn the_save_round_trips_through_json() {
        let (c, r) = searched();
        let mut save = LootSave::default();
        save_into(&mut save, &c, &r, 40.0);
        let back: LootSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();
        assert_eq!(back, save);
        // An older save has no `loot` at all.
        let old: LootSave = serde_json::from_str(r#"{"searched": {}}"#).unwrap();
        assert_eq!(old, LootSave::default());
    }
}
