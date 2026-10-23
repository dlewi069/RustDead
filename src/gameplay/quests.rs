//! Quests: the Narrative plugin's quest graphs (`assets/data/quests.json`, `extractor/gen_quests.py`).
//!
//! - A quest is a graph of states and branches. Entering a state runs its events (in order) and
//!   arms its branches; a branch whose tasks are all done (optional ones aside) leads to its
//!   destination state. A Success or Failure state ends the quest. `QuestLog` (saved as
//!   `GameState::quest_log`) is NarrativeComponent: `begin` (BeginQuest, a no-op once a quest is
//!   started or finished: IsQuestStartedOrFinished), `data_task` (CompleteNarrativeDataTask: a
//!   BPT_CompleteDataTask matches its DataTask and its Argument, case-insensitively, or any argument
//!   when the call's is empty), `reach` (BPT_GoToLocation's TickTask every TickInterval 0.2 s:
//!   the pawn within DistanceTolerance of GoalLocation, unless Invert?). NE_ForgetQuest takes the
//!   quest out of the log. The engine is pure and unit-tested.
//! - Starters: BP_PlayerCharacter's ShowStartingQuests? (set when SD_GameInstance spawns the
//!   survivor) begins Quest_TheSafeZone; BP_StartQuestArea (the lab entrance, its Box overlap and a
//!   0.2 s timer) begins Quest_Dungeon_LaboratoryBunker; the Military Intel Laptop's SCAN LAPTOP
//!   (JigContextMenuComp.ContextScanLaptop -> BP_LaptopEventManager.CallRandomEvent) begins a random
//!   quest of the manager's Quests that isn't started or finished, retrying with a CheckTimes counter
//!   that's never reset (the BP's), and once it has counted every quest: "All Laptop Quests
//!   Completed. Come Back Later...". The laptop is used up either way: the BP's drop to make up for
//!   it has no item (EX_NoObject).
//! - Events: Event_SpawnMarker (BP_QuestMarker: a W_QuestMarker on the map's marker overlay at
//!   size 4, and a BP_CompassMarker on the compass), Event_RemoveMarker / Event_RemoveActor (by
//!   tag), Event_SpawnActor (BP_GuaranteedSpawner_Classified, with its QuestArgument),
//!   Event_RewardPlayerXP (LevellingComponent.AddXP(Amount, false) = Amount x XP MULTIPLIER).
//!   PowerOnGenerator comes from BP_LaboratoryGenerator turning on (its QuestArgument:
//!   "LaboratoryBunker", the second generator "LaboratoryBunker2").
//! - BP_GuaranteedSpawner (the "Classified Military Crate"): searched like a container
//!   (DurationToHold 4 s, ChanceToProduceNoise 10 %); once open it completes InteractWithActor with
//!   its argument, its lid turns -65° over the 1.5 s Timeline and after a 1.3 s Delay one of its
//!   Items (named weapons, Array_Random) is spawned at its Scene point (here: lies on the ground).
//! - The HUD's TaskBox (BP_CustomNarrativeDefaultUI's quest delegates): "QUEST STARTED" / "QUEST
//!   UPDATED" / "QUEST SUCCEEDED", the quest name in capitals and the branches' tasks
//!   (BP_CustomQuestBranch / BP_CustomQuestTask: "- {Description} (Optional)", "(n/N)" when more
//!   than one is needed, done tasks at 0.15 opacity with a strike at 0.5) under FadeInTasks.
//!
//! - The laptop's Eliminate Bandits / Military quests: Event_SpawnPrefab (the quest camp,
//!   prefabs.json, as entities with solid meshes and its containers, tagged), Event_SpawnAIHumanSpawner
//!   (BP_AISpawningVolume_HumanQuest: its Spawning AI in its box, within 60 m, no respawns,
//!   remembered with its QuestArgument), the AI's deaths completing KillNPC with that argument
//!   (`human_kills`; other humans' empty argument counts for any KillNPC task, as Narrative
//!   matches it), Event_RewardGiveItem (one of its ItemSets into the inventory; the laptop quests'
//!   sets name no item, so only their 400 XP is paid). Forget removes the camp and the volume.
//!
//! Not ported: quest givers, the grab montage on opening the crate, the crate's outline and
//! magnifier, quest givers, the Underground? arrow. On loading a save the events of the states a
//! quest went through are run again (markers, the crate); which events Narrative refires on load
//! is a guess. GetTaskProgressText's format "(n/N)" is a guess, as are the TaskBox's fonts (the
//! RichTextBlocks' style table isn't read).

use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::interact::Containers;
use super::items::{ContainerDef, ItemDb};
use super::map::{WorldMarker, WorldMarkers};
use crate::level::Level;
use crate::menus::settings::SettingsStore;
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{CharacterModel, model};

/// BP_PlayerCharacter's starting quest.
pub const SAFE_ZONE: &str = "Quest_TheSafeZone_C";
/// BPT_GoToLocation's TickInterval and BP_StartQuestArea's overlap timer.
const TICK: f32 = 0.2;
/// The pawn's location is its capsule's centre (half of 164 cm above the feet).
const PAWN_CENTRE: f32 = 0.82;
/// BP_QuestMarker's S_MarkerData size.
const MARKER_SIZE: f32 = 4.0;
/// Keys: quest map markers, and the crates' dynamic collision.
const MARKER_KEYS: usize = 1 << 40;
const COLLIDE_KEY: usize = 1 << 42;
/// Quest camps' container keys (and, under COLLIDE_KEY, their meshes').
const QUEST_PREFAB_KEYS: usize = 5 << 32;
/// Quest spawning volumes' indices in `Spawners`.
const QUEST_SPAWNERS: usize = 1 << 26;
/// BP_AISpawningVolume_HumanQuest's Player Activation Range.
const QUEST_VOLUME_RANGE: f32 = 60.0;
/// BP_GuaranteedSpawner: the lid's Lerp(0, -65) roll and the Delay before the weapon appears.
const LID_ROLL: f32 = -65.0;
const DROP_AFTER: f32 = 1.3;
const ALL_DONE: &str = "All Laptop Quests Completed. Come Back Later...";

// ---------------------------------------------------------------- data

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StateKind {
    #[default]
    Regular,
    Success,
    Failure,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct TaskDef {
    /// "GoToLocation", "DataTask" or an unported task class.
    pub kind: String,
    pub description: Option<String>,
    pub optional: bool,
    pub hidden: bool,
    pub at: [f32; 3],
    pub tolerance: f32,
    /// FriendlyLocationName.
    pub location: String,
    pub invert: bool,
    /// The NarrativeDataTask's asset name ("InteractWithActor", "PowerOnGenerator"...).
    pub task: String,
    pub argument: String,
    pub quantity: u32,
}

impl TaskDef {
    pub fn required(&self) -> u32 {
        if self.kind == "DataTask" { self.quantity.max(1) } else { 1 }
    }

    /// GetTaskDescription unless DescriptionOverride is set. BPT_GoToLocation: "{verb} {location}";
    /// BPT_CompleteDataTask's own format isn't fully readable: its task and argument (a guess).
    pub fn text(&self) -> String {
        if let Some(d) = self.description.as_ref().filter(|d| !d.is_empty()) {
            return d.clone();
        }
        match self.kind.as_str() {
            "GoToLocation" => format!("{} {}", if self.invert { "Get away from" } else { "Go to" }, self.location),
            _ => format!("{} {}", self.task, self.argument).trim().to_string(),
        }
    }

    fn supported(&self) -> bool {
        matches!(self.kind.as_str(), "GoToLocation" | "DataTask")
    }
}

#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct EventDef {
    pub kind: String,
    pub supported: Option<bool>,
    pub at: [f32; 3],
    pub rotation: [f32; 4],
    pub texture: Option<String>,
    pub text: String,
    pub tag: String,
    pub colour: [f32; 4],
    pub underground: bool,
    pub class: Option<String>,
    pub argument: String,
    pub amount: f32,
    pub quest: Option<String>,
    /// Event_SpawnAIHumanSpawner: Spawning AI (class, count) and the box's half extents (m, Bevy axes).
    pub spawning: Vec<(String, u32)>,
    pub extent: [f32; 3],
    /// Event_RewardGiveItem: RandomSetOfItems? and ItemSet1-3 (item, amount).
    pub random: bool,
    pub sets: Vec<Vec<(String, u32)>>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct StateDef {
    pub kind: StateKind,
    pub events: Vec<EventDef>,
    pub branches: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct BranchDef {
    pub to: Option<String>,
    pub tasks: Vec<TaskDef>,
    #[serde(default)]
    pub events: Vec<EventDef>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct QuestDef {
    pub name: String,
    pub description: String,
    pub start: String,
    pub states: HashMap<String, StateDef>,
    pub branches: HashMap<String, BranchDef>,
}

impl QuestDef {
    /// Every task and event of the quest is one the port runs.
    pub fn playable(&self) -> bool {
        self.branches.values().all(|b| b.tasks.iter().all(TaskDef::supported) && b.events.iter().all(|e| e.supported != Some(false)))
            && self.states.values().all(|s| s.events.iter().all(|e| e.supported != Some(false)))
    }
}

/// BP_GuaranteedSpawner's values.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct CrateDef {
    pub class: String,
    pub name: String,
    pub hold: f32,
    pub noise: f32,
    pub items: Vec<String>,
    pub mesh: String,
    pub lid: String,
    pub lid_at: [f32; 3],
    pub drop_at: [f32; 3],
    pub lid_seconds: f32,
}

#[derive(Resource, Deserialize, Default)]
pub struct QuestDb {
    pub quests: HashMap<String, QuestDef>,
    #[serde(rename = "crate")]
    pub crate_: CrateDef,
}

impl QuestDb {
    pub fn load() -> Self {
        let text = std::fs::read_to_string("assets/data/quests.json").expect("assets/data/quests.json missing (extractor/export_assets.sh --quests)");
        serde_json::from_str(&text).expect("assets/data/quests.json malformed")
    }
}

// ---------------------------------------------------------------- the log

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Succeeded,
    Failed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct QuestProgress {
    pub class: String,
    pub state: String,
    /// The states entered so far (their events run again when a save is loaded).
    pub path: Vec<String>,
    /// The current state's branches: progress of each task.
    pub tasks: BTreeMap<String, Vec<u32>>,
    pub outcome: Option<Outcome>,
}

/// NarrativeComponent's quests (`GameState::quest_log`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct QuestLog {
    pub quests: Vec<QuestProgress>,
}

/// A branch as BP_CustomQuestBranch shows it: each visible task's line and whether it's done.
#[derive(Clone, Debug, PartialEq)]
pub struct BranchView(pub Vec<(String, bool)>);

/// What a delegate of BP_CustomNarrativeDefaultUI does to the TaskBox.
#[derive(Clone, Debug, PartialEq)]
pub struct UiOp {
    pub quest: String,
    /// QuestUpdatedSubtext (Some("") clears it; None leaves it).
    pub subtext: Option<&'static str>,
    /// Show Quest Update / On Quest New State: the branches shown, and FadeInTasks played.
    pub branches: Option<Vec<BranchView>>,
}

// Outputs are short-lived (a frame's worth); boxing the event isn't worth the churn.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// A quest event to run in the world.
    Event { quest: String, event: EventDef },
    Ui(UiOp),
}

impl QuestLog {
    pub fn get(&self, class: &str) -> Option<&QuestProgress> {
        self.quests.iter().find(|q| q.class == class)
    }

    /// IsQuestStartedOrFinished.
    pub fn started_or_finished(&self, class: &str) -> bool {
        self.get(class).is_some()
    }

    pub fn active(&self) -> impl Iterator<Item = &QuestProgress> {
        self.quests.iter().filter(|q| q.outcome.is_none())
    }

    /// BeginQuest.
    pub fn begin(&mut self, db: &QuestDb, class: &str) -> Vec<Out> {
        let mut out = Vec::new();
        let Some(def) = db.quests.get(class) else {
            warn!("no quest {class}");
            return out;
        };
        if self.started_or_finished(class) {
            return out;
        }
        self.quests.push(QuestProgress { class: class.into(), state: String::new(), path: Vec::new(), tasks: BTreeMap::new(), outcome: None });
        info!("quest started: {class}");
        self.enter(db, class, &def.start, &mut out);
        // OnQuestStarted after the start state's OnQuestNewState.
        out.push(Out::Ui(UiOp { quest: def.name.clone(), subtext: Some("QUEST STARTED"), branches: None }));
        out
    }

    /// The journal's FORGET (and NE_ForgetQuest): the quest leaves the log and can start again.
    pub fn forget(&mut self, class: &str) {
        self.quests.retain(|q| q.class != class);
    }

    fn enter(&mut self, db: &QuestDb, class: &str, state_id: &str, out: &mut Vec<Out>) {
        let (Some(def), Some(state)) = (db.quests.get(class), db.quests.get(class).and_then(|d| d.states.get(state_id))) else { return };
        let Some(q) = self.quests.iter_mut().find(|q| q.class == class) else { return };
        q.state = state_id.into();
        q.path.push(state_id.into());
        q.tasks = state.branches.iter().filter_map(|b| Some((b.clone(), vec![0; def.branches.get(b)?.tasks.len()]))).collect();
        q.outcome = match state.kind {
            StateKind::Regular => None,
            StateKind::Success => Some(Outcome::Succeeded),
            StateKind::Failure => Some(Outcome::Failed),
        };
        if q.outcome.is_some() {
            info!("quest {class}: {:?}", q.outcome);
        }
        let views = (!state.branches.is_empty()).then(|| state.branches.iter().map(|b| view(def, b, q.tasks.get(b))).collect());
        let mut forget = false;
        for e in &state.events {
            forget |= e.kind == "Forget";
            out.push(Out::Event { quest: class.into(), event: e.clone() });
        }
        out.push(Out::Ui(match views {
            Some(v) => UiOp { quest: def.name.clone(), subtext: Some("QUEST UPDATED"), branches: Some(v) },
            None => UiOp {
                quest: def.name.clone(),
                subtext: Some(match state.kind {
                    StateKind::Regular => "QUEST UPDATED",
                    StateKind::Success => "QUEST SUCCEEDED",
                    StateKind::Failure => "QUEST FAILED",
                }),
                branches: None,
            },
        }));
        if forget {
            self.forget(class);
        }
    }

    /// Progress `n` on tasks of the active quests matching `pick`, then take finished branches.
    fn progress(&mut self, db: &QuestDb, n: u32, pick: impl Fn(&TaskDef) -> bool) -> Vec<Out> {
        let mut out = Vec::new();
        let classes: Vec<String> = self.active().map(|q| q.class.clone()).collect();
        for class in classes {
            let Some(def) = db.quests.get(&class) else { continue };
            let Some(q) = self.quests.iter_mut().find(|q| q.class == class) else { continue };
            let mut taken = None;
            for (bid, prog) in q.tasks.iter_mut() {
                let Some(branch) = def.branches.get(bid) else { continue };
                let mut changed = false;
                for (task, p) in branch.tasks.iter().zip(prog.iter_mut()) {
                    let need = task.required();
                    if *p >= need || !pick(task) {
                        continue;
                    }
                    *p = (*p + n).min(need);
                    changed = true;
                    // On Quest Task Progress Made (short of done) / On Quest Task Completed.
                    out.push(Out::Ui(UiOp { quest: def.name.clone(), subtext: (*p < need).then_some(""), branches: None }));
                }
                if changed {
                    let v = view(def, bid, Some(prog));
                    if let Some(Out::Ui(op)) = out.last_mut() {
                        op.branches = Some(vec![v.clone()]);
                    }
                    let done = branch.tasks.iter().zip(prog.iter()).all(|(t, p)| t.optional || *p >= t.required());
                    if done && taken.is_none() {
                        taken = Some((bid.clone(), v));
                    }
                }
            }
            if let Some((bid, v)) = taken {
                info!("quest {class}: branch {bid} done");
                out.push(Out::Ui(UiOp { quest: def.name.clone(), subtext: Some(""), branches: Some(vec![v]) }));
                let branch = &def.branches[&bid];
                for e in &branch.events {
                    out.push(Out::Event { quest: class.clone(), event: e.clone() });
                }
                if let Some(to) = branch.to.clone() {
                    self.enter(db, &class, &to, &mut out);
                }
            }
        }
        out
    }

    /// CompleteNarrativeDataTask(task, argument, quantity).
    pub fn data_task(&mut self, db: &QuestDb, task: &str, argument: &str, quantity: u32) -> Vec<Out> {
        self.progress(db, quantity, |t| t.kind == "DataTask" && t.task == task && (argument.is_empty() || t.argument.eq_ignore_ascii_case(argument)))
    }

    /// BPT_GoToLocation's tick with the pawn at `pawn`.
    pub fn reach(&mut self, db: &QuestDb, pawn: Vec3) -> Vec<Out> {
        self.progress(db, 1, |t| t.kind == "GoToLocation" && (pawn.distance(Vec3::from_array(t.at)) < t.tolerance) != t.invert)
    }

    /// The world events of the states each unfinished quest went through, in order (to rebuild
    /// its markers and actors after loading; rewards aren't paid twice).
    pub fn replay(&self, db: &QuestDb) -> Vec<Out> {
        let mut out = Vec::new();
        for q in self.active() {
            let Some(def) = db.quests.get(&q.class) else { continue };
            for s in q.path.iter().filter_map(|s| def.states.get(s)) {
                for e in s.events.iter().filter(|e| matches!(e.kind.as_str(), "SpawnMarker" | "RemoveMarker" | "SpawnActor" | "RemoveActor")) {
                    out.push(Out::Event { quest: q.class.clone(), event: e.clone() });
                }
            }
        }
        out
    }

    /// Journal_QuestsUI.Show Quest: the branches of every state the quest reached, newest state
    /// first, with whether they're behind it (not the current state: drawn at 0.5). In a past
    /// state the branch that led on shows done.
    pub fn journal_branches(&self, db: &QuestDb, class: &str) -> Vec<(BranchView, bool)> {
        let (Some(q), Some(def)) = (self.get(class), db.quests.get(class)) else { return Vec::new() };
        let mut out = Vec::new();
        for (i, sid) in q.path.iter().enumerate().rev() {
            let Some(state) = def.states.get(sid) else { continue };
            let current = i + 1 == q.path.len();
            for b in &state.branches {
                let Some(branch) = def.branches.get(b) else { continue };
                let progress = if current {
                    q.tasks.get(b).cloned()
                } else {
                    let taken = q.path.get(i + 1).is_some_and(|next| branch.to.as_ref() == Some(next));
                    Some(branch.tasks.iter().map(|t| if taken { t.required() } else { 0 }).collect())
                };
                out.push((view(def, b, progress.as_ref()), !current));
            }
        }
        out
    }

    /// The journal's FORGET: NarrativeComp.ForgetQuest, then the quest's 'Forget' state is
    /// entered (its events take its marker and crate away).
    pub fn forget_from_journal(&mut self, db: &QuestDb, class: &str) -> Vec<Out> {
        self.forget(class);
        let Some(state) = db.quests.get(class).and_then(|d| d.states.get("Forget")) else { return Vec::new() };
        state.events.iter().map(|e| Out::Event { quest: class.into(), event: e.clone() }).collect()
    }

    /// The current state's branches as the journal and TaskBox show them.
    pub fn views(&self, db: &QuestDb, class: &str) -> Vec<BranchView> {
        let (Some(q), Some(def)) = (self.get(class), db.quests.get(class)) else { return Vec::new() };
        q.tasks.iter().map(|(b, p)| view(def, b, Some(p))).collect()
    }
}

fn view(def: &QuestDef, branch: &str, progress: Option<&Vec<u32>>) -> BranchView {
    let Some(b) = def.branches.get(branch) else { return BranchView(Vec::new()) };
    BranchView(
        b.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.hidden)
            .map(|(i, t)| {
                let p = progress.and_then(|p| p.get(i).copied()).unwrap_or(0);
                (task_line(t, p), p >= t.required())
            })
            .collect(),
    )
}

/// BP_CustomQuestTask: "- {Desc} {Prog}" with Desc = "{Description} {Optional}".
pub fn task_line(t: &TaskDef, progress: u32) -> String {
    let desc = format!("{} {}", t.text(), if t.optional { " (Optional)" } else { "" });
    let prog = if t.required() > 1 { format!("({progress}/{})", t.required()) } else { String::new() };
    format!("- {desc} {prog}").trim_end().to_string()
}

/// BP_LaptopEventManager.CallRandomEvent: a random pool quest not started or finished; each
/// miss counts against `check_times`, and when it reaches the pool's size: none (all done).
pub fn pick_laptop_quest<'a>(pool: &'a [String], log: &QuestLog, check_times: &mut usize, rng: &mut impl Rng) -> Option<&'a String> {
    if pool.is_empty() {
        return None;
    }
    // The BP loops until it finds one; past the count with everything done it would never
    // stop, so this gives up after a generous number of draws.
    for _ in 0..pool.len() * 64 {
        let q = pool.choose(rng)?;
        if !log.started_or_finished(q) {
            return Some(q);
        }
        *check_times += 1;
        if *check_times == pool.len() {
            return None;
        }
    }
    None
}

/// FadeInTasks (24000 ticks/s, cubic keys with flat tangents): RenderOpacity 0 -> 2 by 1.25 s,
/// down to 1 by 34 s and 0 by 36.5 s.
pub const FADE_TASKS: [(f32, f32); 4] = [(0.0, 0.0), (30000.0 / 24000.0, 2.0), (816000.0 / 24000.0, 1.0), (876000.0 / 24000.0, 0.0)];
const FADE_TASKS_END: f32 = 924000.0 / 24000.0;

pub fn cubic_track(keys: &[(f32, f32)], t: f32) -> f32 {
    let Some(i) = keys.iter().position(|k| k.0 > t) else { return keys.last().map_or(0.0, |k| k.1) };
    if i == 0 {
        return keys[0].1;
    }
    let ((t0, a), (t1, b)) = (keys[i - 1], keys[i]);
    let s = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
    a + (b - a) * s * s * (3.0 - 2.0 * s)
}

// ---------------------------------------------------------------- the world

/// CompleteNarrativeDataTask from the world (the lab generator, crates).
#[derive(Message, Clone, Debug)]
pub struct DataTask {
    pub task: String,
    pub argument: String,
    pub quantity: u32,
}

/// Begin a named quest (BeginQuest: the screenshot tour's way to pick a laptop quest).
#[derive(Message, Clone, Debug)]
pub struct BeginQuest(pub String);

/// SCAN LAPTOP from the inventory's context menu (the laptop is already used up).
#[derive(Message, Clone, Copy, Debug)]
pub struct ScanLaptop;

/// The journal's FORGET, confirmed (YES).
#[derive(Message, Clone, Debug)]
pub struct ForgetQuest(pub String);

/// A BP_GuaranteedSpawner: its container key, argument, and the open animation's clock.
#[derive(Component)]
pub struct QuestCrate {
    pub key: usize,
    pub tag: String,
    pub argument: String,
    pub opened: Option<f32>,
    lid: Entity,
    dropped: bool,
}

#[derive(Resource, Default)]
pub struct QuestWorld {
    /// Quest markers on the map (tag, WorldMarkers key).
    markers: Vec<(String, usize)>,
    next: usize,
    /// The laptop manager's CheckTimes (an actor variable: per session).
    check_times: usize,
    laptop_pool: Vec<String>,
    /// BP_StartQuestArea: box centre, half extents, quest.
    areas: Vec<(Vec3, Vec3, String)>,
    tick: f32,
    started: bool,
    /// Event_SpawnPrefab's quest camps.
    prefabs: Vec<QuestCamp>,
    /// Event_SpawnAIHumanSpawner's volumes: spawner index -> (tag, QuestArgument).
    pub human_spawners: HashMap<usize, (String, String)>,
}

/// A quest camp (Event_SpawnPrefab): its tag, entities, container keys and collision keys.
type QuestCamp = (String, Vec<Entity>, Vec<usize>, Vec<usize>);

/// The TaskBox's contents and FadeInTasks' clock.
#[derive(Resource, Default)]
pub struct TaskBox {
    pub quest: String,
    pub subtext: String,
    pub branches: Vec<BranchView>,
    pub age: Option<f32>,
    rev: u32,
}

impl TaskBox {
    fn apply(&mut self, op: &UiOp) {
        if let Some(s) = op.subtext {
            self.subtext = s.into();
        }
        if let Some(b) = &op.branches {
            self.quest = op.quest.to_uppercase();
            self.branches = b.clone();
            // Show Quest Update: StartAtTime 0.5 while FadeInTasks is still playing.
            self.age = Some(if self.age.is_some_and(|a| a < FADE_TASKS_END) { 0.5 } else { 0.0 });
        }
        self.rev += 1;
    }
}

#[derive(Component)]
struct TaskBoxUi;
#[derive(Component)]
struct BaseAlpha(f32);
/// A task's Cross (every Node has a BackgroundColor: only these are faded).
#[derive(Component)]
struct Strike;
#[derive(Component)]
struct QuestCompassIcon(usize);

pub fn plugin(app: &mut App) {
    app.insert_resource(QuestDb::load())
        .init_resource::<QuestWorld>()
        .init_resource::<TaskBox>()
        .add_message::<DataTask>()
        .add_message::<ScanLaptop>()
        .add_message::<BeginQuest>()
        .add_message::<ForgetQuest>()
        .add_systems(Startup, register_crate)
        .add_systems(OnEnter(AppState::InGame), (reset, spawn_task_box))
        .add_systems(
            Update,
            ((start, tick, scan_laptops, begin_quests, human_kills, data_tasks, forget_quests, open_crates).chain(), (draw_task_box, compass_icons))
                .chain()
                .run_if(in_state(AppState::InGame))
                .run_if(in_state(MenuScreen::None)),
        );
}

/// The crate class searches like a container (`interact::container_def`).
fn register_crate(qdb: Res<QuestDb>, mut db: ResMut<ItemDb>) {
    let c = &qdb.crate_;
    db.containers.insert(c.class.clone(), ContainerDef { name: c.name.clone(), table: None, hold: c.hold, lockpick: false, noise: c.noise, mesh: Some(c.mesh.clone()), airdrop: None });
}

fn reset(mut world: ResMut<QuestWorld>, mut task_box: ResMut<TaskBox>) {
    *world = QuestWorld::default();
    *task_box = TaskBox::default();
}

/// The level's quest starters, ShowStartingQuests? and the replay of a loaded game's quests.
#[allow(clippy::too_many_arguments)]
fn start(
    mut commands: Commands,
    level: Option<Res<Level>>,
    qdb: Res<QuestDb>,
    mut game: ResMut<CurrentGame>,
    mut world: ResMut<QuestWorld>,
    mut ctx: ApplyCtx,
) {
    let Some(level) = level else { return };
    if world.started || game.state.is_none() {
        return;
    }
    world.started = true;
    for a in &level.actors {
        let props = a.props.as_ref();
        match a.class.as_str() {
            "BP_LaptopEventManager_C" => {
                let quests: Vec<serde_json::Value> = props.and_then(|p| p.get("Quests")).and_then(|q| serde_json::from_str(q).ok()).unwrap_or_default();
                let pool: Vec<String> = quests.iter().filter_map(|q| q.get("ObjectName")?.as_str()?.split('\'').nth(1).map(str::to_string)).collect();
                let (playable, skipped): (Vec<_>, Vec<_>) = pool.into_iter().partition(|q| qdb.quests.get(q).is_some_and(QuestDef::playable));
                if !skipped.is_empty() {
                    info!("laptop quests not ported (they need human NPCs): {}", skipped.join(", "));
                }
                world.laptop_pool = playable;
            }
            "BP_StartQuestArea_C" => {
                let Some(quest) = props.and_then(|p| p.get("Quest")).cloned() else { continue };
                let ext: serde_json::Value = props.and_then(|p| p.get("Box.BoxExtent")).and_then(|e| serde_json::from_str(e).ok()).unwrap_or_default();
                let f = |k: &str| ext.get(k).and_then(|v| v.as_f64()).unwrap_or(500.0) as f32 / 100.0;
                world.areas.push((a.position(), Vec3::new(f("X"), f("Z"), f("Y")), quest));
            }
            _ => {}
        }
    }
    let Some(state) = game.state.as_mut() else { return };
    let outs = state.quest_log.replay(&qdb);
    let mut outs = outs;
    outs.extend(state.quest_log.begin(&qdb, SAFE_ZONE));
    apply(&mut commands, &mut ctx, &mut world, &mut game, outs);
}

/// Everything quest events touch in the world.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ApplyCtx<'w, 's> {
    qdb: Res<'w, QuestDb>,
    store: Res<'w, SettingsStore>,
    assets: Res<'w, AssetServer>,
    markers: ResMut<'w, WorldMarkers>,
    containers: ResMut<'w, Containers>,
    task_box: ResMut<'w, TaskBox>,
    crates: Query<'w, 's, (Entity, &'static QuestCrate)>,
    rays: crate::level::collision::PropRays<'w>,
    spawners: Option<ResMut<'w, crate::level::spawners::Spawners>>,
    db: Res<'w, ItemDb>,
}

fn apply(commands: &mut Commands, ctx: &mut ApplyCtx, world: &mut QuestWorld, game: &mut CurrentGame, outs: Vec<Out>) {
    for o in outs {
        match o {
            Out::Ui(op) => ctx.task_box.apply(&op),
            Out::Event { quest, event: e } => match e.kind.as_str() {
                "SpawnMarker" => {
                    let key = MARKER_KEYS + world.next;
                    world.next += 1;
                    let [r, g, b, a] = e.colour;
                    ctx.markers.0.push(WorldMarker {
                        key,
                        pos: Vec3::from_array(e.at),
                        texture: e.texture.clone().unwrap_or_default(),
                        size: MARKER_SIZE,
                        colour: Color::linear_rgba(r, g, b, a),
                        title: e.text.clone(),
                        description: String::new(),
                    });
                    world.markers.push((e.tag.clone(), key));
                }
                "RemoveMarker" => {
                    let keys: Vec<usize> = world.markers.iter().filter(|(t, _)| *t == e.tag).map(|(_, k)| *k).collect();
                    world.markers.retain(|(t, _)| *t != e.tag);
                    ctx.markers.0.retain(|m| !keys.contains(&m.key));
                }
                "SpawnActor" if e.class.as_deref() == Some(ctx.qdb.crate_.class.as_str()) => {
                    spawn_crate(commands, ctx, &e, world.next);
                    world.next += 1;
                }
                "RemoveActor" => {
                    for (ent, c) in ctx.crates.iter().filter(|(_, c)| c.tag == e.tag) {
                        commands.entity(ent).despawn();
                        ctx.containers.list.retain(|x| x.0 != c.key);
                        ctx.rays.set_dynamic(COLLIDE_KEY + c.key, Vec::new());
                    }
                    // The quest camp and its spawning volume go too (both carry the event's Tag).
                    let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut world.prefabs).into_iter().partition(|p| p.0 == e.tag);
                    world.prefabs = kept;
                    for (_, ents, boxes, collide) in gone {
                        for ent in ents {
                            commands.entity(ent).despawn();
                        }
                        ctx.containers.list.retain(|x| !boxes.contains(&x.0));
                        for k in collide {
                            ctx.rays.set_dynamic(k, Vec::new());
                        }
                    }
                    let volumes: Vec<usize> = world.human_spawners.iter().filter(|(_, (t, _))| *t == e.tag).map(|(i, _)| *i).collect();
                    for i in volumes {
                        world.human_spawners.remove(&i);
                        if let Some(sp) = ctx.spawners.as_mut() {
                            sp.remove(i);
                        }
                    }
                }
                "SpawnPrefab" => spawn_quest_prefab(commands, ctx, world, &e),
                "SpawnAIHumanSpawner" => {
                    // BP_AISpawningVolume_HumanQuest: no respawns, active within its Player
                    // Activation Range (60 m), InitialSpawnDelay 0.5 s.
                    let t = Transform::from_translation(Vec3::from_array(e.at)).with_rotation(Quat::from_array(e.rotation).normalize());
                    for (class, count) in &e.spawning {
                        let Some(kind) = super::zombies::ZombieKind::from_class(class) else { continue };
                        let i = QUEST_SPAWNERS + world.next;
                        world.next += 1;
                        let mut def = crate::level::spawners::SpawnerDef::human_volume(kind, &t, *count);
                        def.extent = Vec3::from_array(e.extent);
                        def.radius = QUEST_VOLUME_RANGE;
                        def.initial_delay = 0.5;
                        def.respawn = crate::level::spawners::RespawnMethod::None;
                        if let Some(sp) = ctx.spawners.as_mut() {
                            sp.add(i, def);
                        }
                        world.human_spawners.insert(i, (e.tag.clone(), e.argument.clone()));
                    }
                    info!("quest {quest}: {} AI spawning for {}", e.spawning.iter().map(|s| s.1).sum::<u32>(), e.argument);
                }
                "RewardGiveItem" => {
                    // One of ItemSet1-3 (with RandomSetOfItems?), each item JigTryAddItemSomewhere.
                    let set = if e.random { rand::thread_rng().gen_range(0..e.sets.len().max(1)) } else { 0 };
                    let items = e.sets.get(set).cloned().unwrap_or_default();
                    for (item, n) in &items {
                        let stack = ctx.db.stack(item, *n);
                        let _ = super::interact::player_inv(game).insert(&ctx.db, stack);
                    }
                    info!("quest {quest}: item reward {items:?}");
                }
                "RewardPlayerXP" => {
                    let xp = e.amount * super::survival::difficulty(&ctx.store, "General", "XP MULTIPLIER");
                    if let Some(st) = game.state.as_mut() {
                        st.progression.add_xp(xp.round() as u64);
                    }
                    info!("quest {quest}: {xp} XP");
                }
                "Forget" => {}
                other => warn!("quest {quest}: event {other} not ported"),
            },
        }
    }
}

/// Event_SpawnPrefab: the prefab (prefabs.json) at the event's Transform, tagged: its meshes
/// (solid), its containers.
fn spawn_quest_prefab(commands: &mut Commands, ctx: &mut ApplyCtx, world: &mut QuestWorld, e: &EventDef) {
    let Some(def) = e.class.as_deref().and_then(|c| crate::level::prefabs::data().prefabs.get(c)) else {
        warn!("quest prefab {:?} not in prefabs.json", e.class);
        return;
    };
    let base = Transform::from_translation(Vec3::from_array(e.at)).with_rotation(Quat::from_array(e.rotation).normalize()).to_matrix();
    let place = |m: &[f32; 12]| Transform::from_matrix(base * crate::level::affine(m).to_matrix());
    let (mut ents, mut boxes, mut collide) = (Vec::new(), Vec::new(), Vec::new());
    for (mesh, m) in &def.meshes {
        let t = place(m);
        let path = format!("game/{mesh}.glb");
        let key = COLLIDE_KEY + QUEST_PREFAB_KEYS + world.next;
        world.next += 1;
        let handle: Handle<Mesh> = ctx.assets.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(path.clone()));
        ents.push(commands.spawn((model(&ctx.assets, path), t, CrateCollision(handle, t, key), DespawnOnExit(AppState::InGame))).id());
        collide.push(key);
    }
    for a in &def.actors {
        if ctx.db.containers.contains_key(&a.class) {
            let key = QUEST_PREFAB_KEYS + world.next;
            world.next += 1;
            ctx.containers.add_at(key, place(&a.transform).translation, &a.class, Vec::new());
            boxes.push(key);
        }
    }
    info!("quest prefab {} at {:.0}: {} meshes, {} containers", e.class.as_deref().unwrap_or("?"), Vec3::from_array(e.at), ents.len(), boxes.len());
    world.prefabs.push((e.tag.clone(), ents, boxes, collide));
}

fn spawn_crate(commands: &mut Commands, ctx: &mut ApplyCtx, e: &EventDef, n: usize) {
    let c = &ctx.qdb.crate_;
    let at = Vec3::from_array(e.at);
    let rotation = Quat::from_array(e.rotation).normalize();
    let t = Transform::from_translation(at).with_rotation(rotation);
    let key = ctx.containers.add_dropped(at, &c.class, None);
    ctx.containers.quest_crates.insert(key);
    let root = commands.spawn((t, Visibility::default(), DespawnOnExit(AppState::InGame))).id();
    commands.spawn((model(&ctx.assets, format!("game/{}.glb", c.mesh)), Transform::default(), ChildOf(root)));
    let lid = commands.spawn((model(&ctx.assets, format!("game/{}.glb", c.lid)), Transform::from_translation(Vec3::from_array(c.lid_at)), ChildOf(root))).id();
    commands.entity(root).insert(QuestCrate { key, tag: e.tag.clone(), argument: e.argument.clone(), opened: None, lid, dropped: false });
    // Solid like its StaticMesh (blocks the Pawn channel).
    let mesh: Handle<Mesh> = ctx.assets.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(format!("game/{}.glb", c.mesh)));
    commands.entity(root).insert(CrateCollision(mesh, t, COLLIDE_KEY + key));
    info!("quest crate {} ({}) at {at:.1}, #{n}", e.tag, e.argument);
}

/// Waiting for the crate's mesh before it can collide.
#[derive(Component)]
struct CrateCollision(Handle<Mesh>, Transform, usize);

/// GoToLocation ticks, BP_StartQuestArea's overlap timer, the crates' collision.
#[allow(clippy::too_many_arguments)]
fn tick(
    mut commands: Commands,
    time: Res<Time>,
    mut game: ResMut<CurrentGame>,
    mut world: ResMut<QuestWorld>,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
    pending: Query<(Entity, &CrateCollision)>,
    mut ctx: ApplyCtx,
) {
    for (e, c) in &pending {
        if ctx.rays.set_dynamic_from(c.2, &c.0, &c.1) {
            commands.entity(e).remove::<CrateCollision>();
        }
    }
    world.tick += time.delta_secs();
    if world.tick < TICK {
        return;
    }
    world.tick = 0.0;
    let Ok(p) = player.single() else { return };
    let pawn = p.translation + Vec3::Y * PAWN_CENTRE;
    let Some(state) = game.state.as_mut() else { return };
    let mut outs = state.quest_log.reach(&ctx.qdb, pawn);
    for (centre, half, quest) in &world.areas {
        if ((pawn - *centre).abs().cmple(*half + Vec3::splat(0.34))).all() {
            outs.extend(state.quest_log.begin(&ctx.qdb, quest));
        }
    }
    if !outs.is_empty() {
        apply(&mut commands, &mut ctx, &mut world, &mut game, outs);
    }
}

fn scan_laptops(mut commands: Commands, mut scans: MessageReader<ScanLaptop>, mut game: ResMut<CurrentGame>, mut world: ResMut<QuestWorld>, mut notify: MessageWriter<super::notify::Notify>, mut ctx: ApplyCtx) {
    for _ in scans.read() {
        let Some(state) = game.state.as_mut() else { return };
        let pool = world.laptop_pool.clone();
        let mut checks = world.check_times;
        let picked = pick_laptop_quest(&pool, &state.quest_log, &mut checks, &mut rand::thread_rng()).cloned();
        world.check_times = checks;
        match picked {
            Some(q) => {
                info!("Started Laptop Event: {q}");
                let outs = state.quest_log.begin(&ctx.qdb, &q);
                apply(&mut commands, &mut ctx, &mut world, &mut game, outs);
            }
            None => {
                notify.write(super::notify::Notify { text: ALL_DONE.into(), secs: 5.0, color: Color::WHITE });
            }
        }
    }
}

fn begin_quests(mut commands: Commands, mut begins: MessageReader<BeginQuest>, mut game: ResMut<CurrentGame>, mut world: ResMut<QuestWorld>, mut ctx: ApplyCtx) {
    for b in begins.read() {
        let Some(state) = game.state.as_mut() else { return };
        info!("quest begun: {}", b.0);
        let outs = state.quest_log.begin(&ctx.qdb, &b.0);
        apply(&mut commands, &mut ctx, &mut world, &mut game, outs);
    }
}

/// BP_MasterBandit / Military / Scavenger's death: CompleteNarrativeDataTask(KillNPC,
/// ArgumentForQuest, 1). A quest volume's AI carries its QuestArgument; the others' is empty,
/// which any KillNPC task takes.
fn human_kills(world: Res<QuestWorld>, killed: Query<&crate::level::LevelZombie, Added<super::combat::Dead>>, mut tasks: MessageWriter<DataTask>) {
    for z in &killed {
        if z.kind.human().is_none() {
            continue;
        }
        let argument = z.spawner.and_then(|i| world.human_spawners.get(&i)).map(|(_, a)| a.clone()).unwrap_or_default();
        tasks.write(DataTask { task: "KillNPC".into(), argument, quantity: 1 });
    }
}

fn data_tasks(mut commands: Commands, mut tasks: MessageReader<DataTask>, mut game: ResMut<CurrentGame>, mut world: ResMut<QuestWorld>, mut ctx: ApplyCtx) {
    for t in tasks.read() {
        let Some(state) = game.state.as_mut() else { return };
        let outs = state.quest_log.data_task(&ctx.qdb, &t.task, &t.argument, t.quantity);
        apply(&mut commands, &mut ctx, &mut world, &mut game, outs);
    }
}

fn forget_quests(mut commands: Commands, mut forgets: MessageReader<ForgetQuest>, mut game: ResMut<CurrentGame>, mut world: ResMut<QuestWorld>, mut ctx: ApplyCtx) {
    for f in forgets.read() {
        let Some(state) = game.state.as_mut() else { return };
        info!("quest forgotten: {}", f.0);
        let outs = state.quest_log.forget_from_journal(&ctx.qdb, &f.0);
        apply(&mut commands, &mut ctx, &mut world, &mut game, outs);
    }
}

/// A searched crate: InteractWithActor, the lid's Timeline and, 1.3 s on, the weapon.
#[allow(clippy::too_many_arguments)]
fn open_crates(
    time: Res<Time>,
    qdb: Res<QuestDb>,
    db: Res<ItemDb>,
    mut containers: ResMut<Containers>,
    mut crates: Query<(&GlobalTransform, &mut QuestCrate)>,
    mut lids: Query<&mut Transform>,
    mut tasks: MessageWriter<DataTask>,
    mut ground: ResMut<super::inventory_ui::Ground>,
) {
    let opened = std::mem::take(&mut containers.quest_opened);
    for (gt, mut c) in &mut crates {
        if opened.contains(&c.key) && c.opened.is_none() {
            c.opened = Some(0.0);
            tasks.write(DataTask { task: "InteractWithActor".into(), argument: c.argument.clone(), quantity: 1 });
            info!("quest crate {} opened", c.tag);
        }
        let Some(age) = c.opened.as_mut() else { continue };
        *age += time.delta_secs();
        let age = *age;
        let k = (age / qdb.crate_.lid_seconds.max(0.01)).min(1.0);
        if let Ok(mut t) = lids.get_mut(c.lid) {
            // Roll is about UE X, our X.
            t.rotation = Quat::from_rotation_x((LID_ROLL * k).to_radians());
        }
        if age >= DROP_AFTER && !c.dropped {
            c.dropped = true;
            let mut rng = rand::thread_rng();
            if let Some(item) = qdb.crate_.items.choose(&mut rng) {
                let at = gt.transform_point(Vec3::from_array(qdb.crate_.drop_at));
                ground.drop_at(at, db.spawn(item, 1, &mut rng));
                info!("quest crate {} dropped {item}", c.tag);
            }
        }
    }
}

fn spawn_task_box(mut commands: Commands) {
    // TaskBox: the canvas's top left, 670 x 800; Overlay_1's 15 px padding.
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(670.0), padding: UiRect::all(Val::Px(15.0)), flex_direction: FlexDirection::Column, ..default() },
        Visibility::Hidden,
        TaskBoxUi,
        DespawnOnExit(AppState::InGame),
    ));
}

#[allow(clippy::too_many_arguments)]
fn draw_task_box(
    mut commands: Commands,
    time: Res<Time>,
    font: Res<MenuFont>,
    mut task_box: ResMut<TaskBox>,
    mut root: Query<(Entity, &mut Visibility), With<TaskBoxUi>>,
    children: Query<&Children>,
    mut texts: Query<(&mut TextColor, &BaseAlpha)>,
    mut bars: Query<(&mut BackgroundColor, &BaseAlpha), With<Strike>>,
    mut built: Local<u32>,
) {
    let Ok((root, mut vis)) = root.single_mut() else { return };
    if *built != task_box.rev {
        *built = task_box.rev;
        commands.entity(root).despawn_related::<Children>();
        let text = |s: String, size: f32, alpha: f32| (label_colored(&font, s, size, Color::WHITE), BaseAlpha(alpha));
        commands.entity(root).with_children(|r| {
            r.spawn(text(task_box.subtext.clone(), 10.0, 1.0));
            r.spawn((text(task_box.quest.clone(), 14.0, 1.0), Node { margin: UiRect::top(Val::Px(5.0)), ..default() }));
            for b in &task_box.branches {
                r.spawn(Node { flex_direction: FlexDirection::Column, margin: UiRect { left: Val::Px(-5.0), top: Val::Px(15.0), ..default() }, ..default() }).with_children(|tasks| {
                    for (line, done) in &b.0 {
                        tasks.spawn(Node { margin: UiRect::left(Val::Px(15.0)), align_self: AlignSelf::FlexStart, ..default() }).with_children(|o| {
                            o.spawn(text(line.clone(), 12.0, if *done { 0.15 } else { 1.0 }));
                            // Cross: 2 px across the line, 0.5 when done.
                            o.spawn((
                                Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), top: Val::Percent(50.0), height: Val::Px(2.0), ..default() },
                                BackgroundColor(Color::WHITE),
                                BaseAlpha(if *done { 0.5 } else { 0.0 }),
                                Strike,
                            ));
                        });
                    }
                });
            }
        });
    }
    let Some(age) = task_box.age.as_mut() else {
        *vis = Visibility::Hidden;
        return;
    };
    *age += time.delta_secs();
    let a = cubic_track(&FADE_TASKS, *age).clamp(0.0, 1.0);
    vis.set_if_neq(if *age < FADE_TASKS_END { Visibility::Inherited } else { Visibility::Hidden });
    for k in children.iter_descendants(root) {
        if let Ok((mut c, base)) = texts.get_mut(k) {
            c.0.set_alpha(base.0 * a);
        }
        if let Ok((mut c, base)) = bars.get_mut(k) {
            c.0.set_alpha(base.0 * a);
        }
    }
}

/// BP_QuestMarker's BP_CompassMarker: the marker's icon in its colour on the compass.
#[allow(clippy::type_complexity)]
fn compass_icons(
    mut commands: Commands,
    world: Res<QuestWorld>,
    markers: Res<WorldMarkers>,
    assets: Res<AssetServer>,
    strip: Query<Entity, With<super::hud::CompassStrip>>,
    cams: Query<&GlobalTransform, With<super::PlayerCamera>>,
    mut icons: Query<(Entity, &QuestCompassIcon, &mut Node, &mut Visibility)>,
) {
    let Ok(strip) = strip.single() else { return };
    let quest: Vec<&WorldMarker> = markers.0.iter().filter(|m| world.markers.iter().any(|(_, k)| *k == m.key)).collect();
    let have: Vec<usize> = icons.iter().map(|(_, i, ..)| i.0).collect();
    for (e, i, ..) in &icons {
        if !quest.iter().any(|m| m.key == i.0) {
            commands.entity(e).despawn();
        }
    }
    for m in quest.iter().filter(|m| !have.contains(&m.key)) {
        let icon = commands
            .spawn((
                Node { position_type: PositionType::Absolute, top: Val::Px(5.0), width: Val::Px(25.0), height: Val::Px(25.0), ..default() },
                ImageNode::new(assets.load(m.texture.clone())).with_color(m.colour),
                QuestCompassIcon(m.key),
                Visibility::Hidden,
            ))
            .id();
        commands.entity(strip).add_child(icon);
    }
    let Some(cam) = cams.iter().next() else { return };
    for (_, icon, mut node, mut vis) in &mut icons {
        let x = quest.iter().find(|m| m.key == icon.0).and_then(|m| super::markers::compass_x(m.pos, cam.translation(), *cam.forward(), *cam.right()));
        vis.set_if_neq(if x.is_some() { Visibility::Inherited } else { Visibility::Hidden });
        if let Some(x) = x {
            node.left = Val::Px(x);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Option<QuestDb> {
        let text = std::fs::read_to_string("assets/data/quests.json").ok()?;
        Some(serde_json::from_str(&text).unwrap())
    }

    fn events(outs: &[Out]) -> Vec<String> {
        outs.iter().filter_map(|o| if let Out::Event { event, .. } = o { Some(event.kind.clone()) } else { None }).collect()
    }

    #[test]
    fn classified_container_goes_to_the_spot_then_opens_the_crate() {
        let Some(db) = db() else { return };
        let q = "Quest_LaptopEvent_ClassifiedContainer001_C";
        let mut log = QuestLog::default();
        let outs = log.begin(&db, q);
        assert_eq!(events(&outs), ["SpawnMarker", "SpawnActor"]);
        assert!(matches!(outs.last(), Some(Out::Ui(UiOp { subtext: Some("QUEST STARTED"), .. }))));
        assert!(log.begin(&db, q).is_empty(), "started already");
        let goal = Vec3::new(1547.1188, 14.620056, 2598.8914);
        // The crate can't be opened before getting there.
        assert!(events(&log.data_task(&db, "InteractWithActor", "ClassifiedContainer1", 1)).is_empty());
        assert!(log.reach(&db, goal + Vec3::X * 10.5).is_empty(), "10 m tolerance");
        let outs = log.reach(&db, goal + Vec3::X * 9.0);
        assert_eq!(events(&outs), ["RemoveMarker"]);
        assert_eq!(log.get(q).unwrap().state, "QuestState_1");
        assert!(events(&log.data_task(&db, "InteractWithActor", "ClassifiedContainer2", 1)).is_empty(), "another crate");
        let outs = log.data_task(&db, "interactwithactor", "classifiedcontainer1", 1);
        assert!(events(&outs).is_empty(), "the task name must match exactly");
        let outs = log.data_task(&db, "InteractWithActor", "classifiedcontainer1", 1);
        assert_eq!(events(&outs), ["RewardPlayerXP"]);
        assert!(matches!(outs.iter().find_map(|o| if let Out::Event { event, .. } = o { Some(event.amount) } else { None }), Some(a) if a == 400.0));
        assert_eq!(log.get(q).unwrap().outcome, Some(Outcome::Succeeded));
        assert!(matches!(outs.last(), Some(Out::Ui(UiOp { subtext: Some("QUEST SUCCEEDED"), .. }))));
        assert!(log.begin(&db, q).is_empty(), "finished");
    }

    #[test]
    fn the_lab_quest_needs_its_own_generator_and_not_the_optional_task() {
        let Some(db) = db() else { return };
        let q = "Quest_Dungeon_LaboratoryBunker_C";
        let mut log = QuestLog::default();
        log.begin(&db, q);
        let lines = &log.views(&db, q)[0].0;
        assert_eq!(lines[0], ("- Power on the laboratories generator".to_string(), false));
        assert_eq!(lines[1], ("- Eliminate infected  (Optional)".to_string(), false));
        assert!(log.data_task(&db, "PowerOnGenerator", "LaboratoryBunker2", 1).is_empty());
        let outs = log.data_task(&db, "PowerOnGenerator", "LaboratoryBunker", 1);
        assert_eq!(events(&outs), ["RewardPlayerXP"]);
        assert_eq!(log.get(q).unwrap().outcome, Some(Outcome::Succeeded));
    }

    #[test]
    fn replay_rebuilds_markers_and_crates_without_rewards() {
        let Some(db) = db() else { return };
        let q = "Quest_LaptopEvent_ClassifiedContainer003_C";
        let mut log = QuestLog::default();
        log.begin(&db, q);
        assert_eq!(events(&log.replay(&db)), ["SpawnMarker", "SpawnActor"]);
        let goal = Vec3::from_array(db.quests[q].branches.values().flat_map(|b| &b.tasks).find(|t| t.kind == "GoToLocation").unwrap().at);
        log.reach(&db, goal);
        assert_eq!(events(&log.replay(&db)), ["SpawnMarker", "SpawnActor", "RemoveMarker"]);
        let saved: QuestLog = serde_json::from_str(&serde_json::to_string(&log).unwrap()).unwrap();
        assert_eq!(saved, log);
    }

    #[test]
    fn the_journal_lists_reached_states_newest_first_and_forget_clears_the_world() {
        let Some(db) = db() else { return };
        let q = "Quest_LaptopEvent_ClassifiedContainer001_C";
        let mut log = QuestLog::default();
        log.begin(&db, q);
        log.reach(&db, Vec3::new(1547.1188, 14.620056, 2598.8914));
        let b = log.journal_branches(&db, q);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0], (BranchView(vec![("- Find and open the container in the vicinity".into(), false)]), false));
        assert_eq!(b[1], (BranchView(vec![("- Go to the specified location".into(), true)]), true));
        assert_eq!(events(&log.forget_from_journal(&db, q)), ["Forget", "RemoveMarker", "RemoveActor"]);
        assert!(!log.started_or_finished(q));
    }

    #[test]
    fn forgetting_lets_a_quest_start_again() {
        let Some(db) = db() else { return };
        let mut log = QuestLog::default();
        log.begin(&db, SAFE_ZONE);
        assert!(log.started_or_finished(SAFE_ZONE));
        log.forget(SAFE_ZONE);
        assert!(!log.started_or_finished(SAFE_ZONE));
        assert!(!log.begin(&db, SAFE_ZONE).is_empty());
        assert_eq!(log.views(&db, SAFE_ZONE)[0].0[0].0, "- Go to The Valley Safe Zone");
    }

    #[test]
    fn every_laptop_and_story_quest_is_playable() {
        let Some(db) = db() else { return };
        let mut playable: Vec<&String> = db.quests.iter().filter(|(_, d)| d.playable()).map(|(k, _)| k).collect();
        playable.sort();
        assert_eq!(playable.len(), 18, "{playable:?}");
        assert_eq!(playable.iter().filter(|q| q.contains("Eliminate")).count(), 6);
        assert!(db.crate_.items.iter().all(|i| i.starts_with("DA_")) && db.crate_.items.len() == 14);
        assert_eq!((db.crate_.name.as_str(), db.crate_.hold, db.crate_.noise), ("Classified Military Crate", 4.0, 10.0));
    }

    #[test]
    fn elimination_spawns_the_camp_then_counts_the_kills() {
        let Some(db) = db() else { return };
        let q = "Quest_LaptopEvent_EliminateBandits001_C";
        let mut log = QuestLog::default();
        let outs = log.begin(&db, q);
        assert_eq!(events(&outs), ["SpawnMarker", "SpawnPrefab"]);
        let camp = Vec3::new(1745.08, 12.39, 717.88);
        let outs = log.reach(&db, camp);
        assert_eq!(events(&outs), ["RemoveMarker", "SpawnAIHumanSpawner"]);
        let spawner = outs.iter().find_map(|o| if let Out::Event { event, .. } = o { (event.kind == "SpawnAIHumanSpawner").then(|| event.clone()) } else { None }).unwrap();
        assert_eq!(spawner.spawning, [("BP_Bandit_Ranged_Boss_C".to_string(), 2), ("BP_Bandit_Ranged_AssaultRifle_C".to_string(), 6)]);
        assert_eq!((spawner.argument.as_str(), spawner.extent), ("BanditCamp1", [12.5, 2.0, 12.5]));
        // Eight kills (another quest's camp doesn't count; an argument-less kill does).
        assert!(events(&log.data_task(&db, "KillNPC", "MilitaryCamp1", 1)).is_empty());
        for _ in 0..6 {
            assert!(events(&log.data_task(&db, "KillNPC", "BanditCamp1", 1)).is_empty());
        }
        assert!(events(&log.data_task(&db, "KillNPC", "", 1)).is_empty());
        assert_eq!(events(&log.data_task(&db, "KillNPC", "BanditCamp1", 1)), ["RewardPlayerXP", "RewardGiveItem"]);
    }

    #[test]
    fn the_laptop_counts_misses_and_gives_up_at_the_pool_size() {
        let Some(db) = db() else { return };
        let pool: Vec<String> = vec!["Quest_LaptopEvent_ClassifiedContainer001_C".into(), "Quest_LaptopEvent_ClassifiedContainer002_C".into()];
        let mut log = QuestLog::default();
        let (mut checks, mut rng) = (0, rand::thread_rng());
        // Nothing started: the first draw is taken and nothing is counted.
        assert!(pick_laptop_quest(&pool, &log, &mut checks, &mut rng).is_some());
        assert_eq!(checks, 0);
        for q in &pool {
            log.begin(&db, q);
        }
        // Every quest done: misses count up to the pool's size, then "all completed".
        assert!(pick_laptop_quest(&pool, &log, &mut checks, &mut rng).is_none());
        assert_eq!(checks, 2);
        // The counter is never reset (the BP's): one more miss would pass it, so the next free
        // quest is found again however many draws it takes.
        log.forget(&pool[1]);
        assert_eq!(pick_laptop_quest(&pool, &log, &mut checks, &mut rng), Some(&pool[1]));
    }

    #[test]
    fn task_lines_and_fade() {
        let t = TaskDef { kind: "DataTask".into(), task: "KillNPC".into(), argument: "BanditCamp1".into(), quantity: 8, description: Some("Find & eliminate the bandits".into()), ..default() };
        assert_eq!(task_line(&t, 3), "- Find & eliminate the bandits  (3/8)");
        let g = TaskDef { kind: "GoToLocation".into(), location: "The Valley Safe Zone".into(), ..default() };
        assert_eq!(task_line(&g, 0), "- Go to The Valley Safe Zone");
        assert_eq!(cubic_track(&FADE_TASKS, 0.0), 0.0);
        assert!((cubic_track(&FADE_TASKS, 1.25) - 2.0).abs() < 1e-5);
        assert!(cubic_track(&FADE_TASKS, 20.0) > 1.0);
        assert_eq!(cubic_track(&FADE_TASKS, 40.0), 0.0);
    }
}
