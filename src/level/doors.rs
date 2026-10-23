//! Interactive doors, from the door Blueprints (decompiled; see each `Kind`):
//!
//! - `BP_LockedDoor`: locked until picked (the lockpicking minigame, XP = level x 100, "Door
//!   Unlocked, XP Given") or forced with a crowbar (10 x OTHER ITEMS DURABILITY MULTIPLIER of its
//!   durability, a noise heard 75 m away). Then F swings it 90°. From behind it opens for 5 s.
//! - `BP_KeycardDoor(_Child)`, `BP_HangarDoor`, `BP_LaboratorySecureDoors`: every opening from the
//!   front takes the door's keycard (consumed) unless the survivor carries the All Access keycard
//!   (kept); the first gives level x 200 (secure doors: 400) XP. Closing is free, and so is
//!   opening from behind (secure doors only with `CanOpenDoorFromBehind?`; the keycard door closes
//!   again after 10 s). Opened doors close themselves after 1800 s.
//! - `BP_LaboratorySlidingDoor`: `Locked` ones take their keycard (or All Access) once, give
//!   level x 400 XP and are unlocked from then on; `Warning` ones open for 5 s; the rest toggle.
//!   `CanOpenDoorFromBehind?` ones open for 5 s from behind. (`Unlocked` is lab power, not ported.)
//! - `BP_LaboratoryEntranceDoors` (wings roll 75°), `BP_WarehouseDoors` (halves slide 250 cm
//!   apart), `BP_ContainerDoors` (wings swing 90°): free.
//! - `BP_KeypadDoor`: a code (rolled 1111-9999 each session and written on its `BP_KeypadNote`)
//!   entered on KeypadUI (`gameplay::keypad`) opens it, gives level x 200 XP once and leaves it
//!   unlocked for the session (`OpenedInThisInstance?`); F then toggles it. A wrong code is a noise
//!   heard 80 m away. From behind it opens for 10 s. Opened doors close themselves after 1800 s.
//! - `BP_SubwayTravelDoor`: travel: the clock jumps TimeToTravel (UDS units, 100 = 1 h) and the
//!   player arrives at TravelToLoc.
//!
//! - `BP_Lever` (the police station): "Interact" with no power says "No Power To Lever";
//!   powered (by `BP_PowerGenerator`, `power.rs`) it calls CallInteractOnObject on its `Actors`
//!   (the garage door, `garage.rs`) and swings its handle: the 0.6 s LeverMovement timeline sets
//!   its pitch Lerp(75, -75), played from the start or reversed on alternate pulls.
//!
//! Not here: `BP_PoliceStationGarageDoor` (a skeletal mesh, `garage.rs`),
//! `BP_ZombieDoor` (only a "zombies banging" sound emitter).
//!
//! The door meshes are the Blueprints' components in placements.json; `find` takes them out of
//! the static cells so they can move. Durations are the BPs' TimelineLengths; the secure doors'
//! wheel (2 s) and wings (2.5 s) run as one 2.5 s motion here. "Behind" is the actor's -X side
//! (GetBehindObject? isn't decompiled; a guess).

use std::collections::HashSet;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

use super::collision::CollisionCache;
use super::{ActorInfo, Level, affine};
use crate::gameplay::interact::{Containers, GameOverlay, Prompt, PromptBar, PromptText, player_inv};
use crate::gameplay::notify::Notify;
use crate::menus::{AppState, CurrentGame, MenuScreen};

const REACH: f32 = 2.6;
const ALL_ACCESS: &str = "DA_AllAccessKeycard";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    /// Yaw about the part's origin (UE degrees).
    Yaw(f32),
    /// Roll about the part's local X (UE degrees).
    Roll(f32),
    /// Slide by a local offset (UE cm, UE axes).
    Slide(Vec3),
    /// Pitch about the part's local UE Y (our Z) by this many UE degrees (the lever).
    Pitch(f32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Locked,
    /// Lab sliding door: `keycard` while `Locked`; `warning`: placed in the Warning state (no
    /// power). Lab generators change both at runtime (`Doors::set_power`).
    Sliding { keycard: Option<String>, warning: bool, from_behind: bool },
    /// A keycard per opening from the front; `xp` per survivor level, once.
    Keycard { card: String, xp: u64, from_behind: bool, behind_close: Option<f32> },
    /// `note`: where its BP_KeypadNote lies.
    Keypad { note: Option<Vec3> },
    Free,
    Travel { to: Vec3, hours: f32, name: String },
    /// `BP_Lever`: the garage doors (actor indices) it works. Locked = no power.
    Lever { gates: Vec<usize> },
}

#[derive(Clone, Debug)]
pub struct DoorPart {
    pub mesh: u32,
    pub base: Transform,
    pub motion: Motion,
}

#[derive(Clone, Debug)]
pub struct DoorDef {
    pub actor: usize,
    pub kind: Kind,
    pub parts: Vec<DoorPart>,
    pub pos: Vec3,
    /// The actor's +X (front) in world space.
    pub front: Vec3,
    /// Closes itself this long after opening.
    pub auto_close: Option<f32>,
    /// Seconds to open or close (the Timeline's length).
    pub secs: f32,
    /// What the prompt calls it (the BP's ShowInteractWidget name).
    pub name: String,
}

struct ClassSpec {
    /// (mesh file stem, part's relative location in the actor (UE cm), motion).
    parts: Vec<(&'static str, Vec3, Motion)>,
    auto_close: Option<f32>,
    secs: f32,
    name: &'static str,
}

fn spec(parts: Vec<(&'static str, Vec3, Motion)>, auto_close: Option<f32>, secs: f32, name: &'static str) -> ClassSpec {
    ClassSpec { parts, auto_close, secs, name }
}

fn class_spec(a: &ActorInfo) -> Option<ClassSpec> {
    let slide = |x: f32, y: f32| Motion::Slide(Vec3::new(x, y, 0.0));
    Some(match a.class.as_str() {
        // `find` applies an instance's Mesh override.
        "BP_LockedDoor_C" => spec(vec![("SM_Bld_Door_01", Vec3::ZERO, Motion::Yaw(90.0))], None, 0.5, "Door"),
        "BP_LaboratorySlidingDoor_C" => spec(vec![("LabSmallDoor", Vec3::ZERO, slide(125.0, 0.0))], None, 1.5, "Sliding Door"),
        "BP_KeycardDoor_C" | "BP_KeycardDoor_Child_C" => {
            spec(vec![("SM_Bld_Bunker_Wall_Caution_Door_Opening_01", Vec3::new(11.0, -77.0, 29.0), Motion::Yaw(90.0))], Some(1800.0), 0.5, "Keycard Door")
        }
        // The keycard door's Door component and timelines (0.5 s); its DoorName is per instance.
        "BP_KeypadDoor_C" => {
            spec(vec![("SM_Bld_Bunker_Wall_Caution_Door_Opening_01", Vec3::new(11.0, -77.0, 29.0), Motion::Yaw(90.0))], Some(1800.0), 0.5, "Keypad Door")
        }
        "BP_HangarDoor_C" => spec(
            vec![("SM_Bld_Hangar_Door_01", Vec3::new(-7.65, 0.0, 0.0), slide(0.0, 300.0)), ("SM_Bld_Hangar_Door_01", Vec3::new(7.65, 0.0, 0.0), slide(0.0, -300.0))],
            Some(1800.0),
            4.0,
            "Hangar Door",
        ),
        "BP_LaboratorySecureDoors_C" => spec(
            vec![
                ("LabDoorLeft", Vec3::ZERO, slide(0.0, -100.0)),
                ("LabDoorRight", Vec3::ZERO, slide(0.0, 145.0)),
                ("LabDoorCircle", Vec3::new(0.0, 0.0, 141.652), Motion::Roll(-90.0)),
            ],
            None,
            2.5,
            "Secure Doors",
        ),
        "BP_LaboratoryEntranceDoors_C" => spec(
            vec![("LabEntranceDoorL", Vec3::new(0.0, -115.0, 0.0), Motion::Roll(-75.0)), ("LabEntranceDoorR", Vec3::new(0.0, 115.0, 0.0), Motion::Roll(75.0))],
            None,
            2.5,
            "Laboratory Doors",
        ),
        "BP_WarehouseDoors_C" => spec(vec![("WarehouseDoor_Left", Vec3::ZERO, slide(-250.0, 0.0)), ("WarehouseDoor_Right", Vec3::ZERO, slide(250.0, 0.0))], None, 4.0, "Warehouse Doors"),
        // Door1 (-107 Y) turns -180 -> -270, Door2 (+107 Y) -180 -> -90.
        "BP_ContainerDoors_C" => spec(
            vec![
                ("SM_Prop_Shipping_Container_Door_01", Vec3::new(0.0, -107.19, 0.0), Motion::Yaw(-90.0)),
                ("SM_Prop_Shipping_Container_Door_01", Vec3::new(0.0, 107.19, 0.0), Motion::Yaw(90.0)),
            ],
            None,
            1.0,
            "Container Doors",
        ),
        "BP_SubwayTravelDoor_C" => spec(vec![("BunkerDoor", Vec3::ZERO, Motion::Yaw(15.0))], None, 0.5, "Subway"),
        // Placed at the template's pitch 75; pulled to -75 (the timeline's curve eases; ours is linear).
        "BP_Lever_C" => spec(vec![("Lever", Vec3::ZERO, Motion::Pitch(-150.0))], None, 0.6, "Lever"),
        _ => return None,
    })
}

fn kind_of(a: &ActorInfo, actors: &[ActorInfo]) -> Kind {
    let p = |k: &str| a.props.as_ref().and_then(|m| m.get(k)).map(|v| v.trim_matches('"').to_string());
    let flag = |k: &str| p(k).as_deref() == Some("true");
    // BP_KeycardDoor / BP_LaboratorySecureDoors / BP_HangarDoor class default: DA_Level0Keycard.
    let card = || p("Keycard").unwrap_or_else(|| "DA_Level0Keycard".into());
    match a.class.as_str() {
        "BP_LockedDoor_C" => Kind::Locked,
        "BP_LaboratorySlidingDoor_C" => Kind::Sliding {
            // The sliding door's own default is DA_Level2Keycard.
            keycard: flag("Locked").then(|| p("Keycard").unwrap_or_else(|| "DA_Level2Keycard".into())),
            warning: flag("Warning"),
            from_behind: flag("CanOpenDoorFromBehind?"),
        },
        "BP_KeycardDoor_C" | "BP_KeycardDoor_Child_C" => Kind::Keycard { card: card(), xp: 200, from_behind: true, behind_close: Some(10.0) },
        "BP_HangarDoor_C" => Kind::Keycard { card: card(), xp: 200, from_behind: true, behind_close: None },
        // AutoDoor's close delay isn't in the dump; the sliding door's 5 s.
        "BP_LaboratorySecureDoors_C" => Kind::Keycard { card: card(), xp: 400, from_behind: flag("CanOpenDoorFromBehind?"), behind_close: Some(5.0) },
        "BP_KeypadDoor_C" => Kind::Keypad { note: p("NoteRef").and_then(|n| actors.iter().find(|x| x.name == n)).map(ActorInfo::position) },
        "BP_SubwayTravelDoor_C" => {
            let to = p("TravelToLoc").and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok()).map_or(Vec3::ZERO, |v| {
                let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                Vec3::new(f("X"), f("Z"), f("Y")) / 100.0
            });
            let name = p("TravelToName")
                .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
                .and_then(|v| v.get("SourceString").and_then(|s| s.as_str()).map(str::to_string))
                .unwrap_or_else(|| "Subway".into());
            Kind::Travel { to, hours: p("TimeToTravel").and_then(|v| v.parse().ok()).unwrap_or(300.0) / 100.0, name }
        }
        "BP_Lever_C" => Kind::Lever {
            gates: a.props.as_ref().and_then(|m| m.get("Actors")).map(|j| super::power::refs(j)).unwrap_or_default().iter().filter_map(|n| actors.iter().position(|x| &x.name == n)).collect(),
        },
        _ => Kind::Free,
    }
}

/// Each door with the mesh path of each of its parts.
pub type Found = Vec<(DoorDef, Vec<String>)>;

/// Doors in the level and the placement instances (mesh path, index) they take over.
pub fn find(actors: &[ActorInfo], placements: &std::collections::HashMap<String, Vec<[f32; 12]>>) -> (Found, HashSet<(String, usize)>) {
    let mut taken = HashSet::new();
    let mut doors = Vec::new();
    for (i, a) in actors.iter().enumerate() {
        let Some(spec) = class_spec(a) else { continue };
        let mesh_override = a.props.as_ref().and_then(|m| m.get("Mesh")).map(|v| v.trim_matches('"').to_string()).filter(|_| a.class == "BP_LockedDoor_C");
        let t = &a.transform;
        let (c0, c1, c2) = (Vec3::new(t[0], t[1], t[2]).normalize_or(Vec3::X), Vec3::new(t[3], t[4], t[5]).normalize_or(Vec3::Y), Vec3::new(t[6], t[7], t[8]).normalize_or(Vec3::Z));
        let pos = a.position();
        let mut parts = Vec::new();
        let mut paths = Vec::new();
        for &(stem, rel, motion) in &spec.parts {
            let stem = mesh_override.as_deref().unwrap_or(stem);
            let want = pos + (c0 * rel.x + c2 * rel.y + c1 * rel.z) / 100.0;
            let best = placements
                .iter()
                .filter(|(path, _)| path.rsplit('/').next() == Some(stem))
                .flat_map(|(path, list)| list.iter().enumerate().map(move |(k, m)| (path, k, m)))
                .filter(|(path, k, _)| !taken.contains(&((*path).clone(), *k)))
                .map(|(path, k, m)| (path, k, affine(m), Vec3::new(m[9], m[10], m[11]).distance(want)))
                .filter(|c| c.3 < 3.0)
                .min_by(|a, b| a.3.total_cmp(&b.3));
            if let Some((path, k, base, _)) = best {
                taken.insert((path.clone(), k));
                paths.push(path.clone());
                parts.push(DoorPart { mesh: 0, base, motion });
            }
        }
        if !parts.is_empty() {
            // The keypad door's per-instance DoorName ("Oil Rig Security Door - Testing Facility").
            let name = a
                .props
                .as_ref()
                .and_then(|m| m.get("DoorName"))
                .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
                .and_then(|v| v.get("SourceString").and_then(|s| s.as_str()).map(str::to_string))
                .unwrap_or_else(|| spec.name.to_string());
            doors.push((DoorDef { actor: i, kind: kind_of(a, actors), parts, pos, front: c0, auto_close: spec.auto_close, secs: spec.secs, name }, paths));
        }
    }
    (doors, taken)
}

/// A part's transform at `open` (0 closed .. 1 open).
pub fn part_transform(p: &DoorPart, open: f32) -> Transform {
    let mut t = p.base;
    match p.motion {
        // UE yaw is clockwise seen from above in a left-handed frame; our axes mirror it.
        Motion::Yaw(deg) => t.rotate_local_y(-(deg * open).to_radians()),
        // UE X (roll axis) is our X; the handedness flip turns the angle round like yaw's.
        Motion::Roll(deg) => t.rotate_local_x(-(deg * open).to_radians()),
        // UE (X, Y, Z) cm -> our (X, Z, Y) m.
        Motion::Slide(cm) => t.translation += t.rotation * Vec3::new(cm.x, cm.z, cm.y) * (open / 100.0),
        // UE pitch turns X up towards Z; with our axes that's a positive turn about local Z.
        Motion::Pitch(deg) => t.rotate_local_z((deg * open).to_radians()),
    }
    t
}

#[derive(Default, Clone, Debug)]
struct DoorState {
    open: f32,
    target: bool,
    /// The direction its sound last played for (`door_sounds`).
    heard: bool,
    locked: bool,
    since_open: f32,
    /// Opened for a while only (from behind, `Warning` doors): closes after this long.
    close_after: Option<f32>,
    /// The keycard / keypad XP was given.
    xp_given: bool,
    /// The keypad's code (BeginPlay's roll).
    code: u32,
    /// A lab sliding door's Warning state: it only opens for 5 s at a time.
    warning: bool,
}

impl DoorState {
    fn new(kind: &Kind) -> Self {
        // A keypad door is "locked" until its code was entered this session.
        Self {
            locked: matches!(kind, Kind::Locked | Kind::Sliding { keycard: Some(_), .. } | Kind::Keypad { .. } | Kind::Lever { .. }),
            warning: matches!(kind, Kind::Sliding { warning: true, .. }),
            ..default()
        }
    }

    fn open_now(&mut self, close_after: Option<f32>) {
        self.target = true;
        self.since_open = 0.0;
        self.close_after = close_after;
    }
}

/// What pressing F at a door does.
#[derive(Debug, PartialEq)]
enum Press {
    Toggle,
    /// Open, closing again after this long.
    OpenFor(f32),
    Lockpick,
    Crowbar,
    /// A keycard opens it (`consume`: the card used up; None with All Access).
    Card { consume: Option<String> },
    NoCard,
    /// The code pad opens.
    Keypad,
    Travel,
    /// "No Power To Lever".
    NoPower,
    /// Pull the lever: swing it and work its garage doors.
    Lever,
    Nothing,
}

fn prompt_options(d: &DoorDef, s: &DoorState, behind: bool) -> Vec<&'static str> {
    match (&d.kind, s.locked) {
        (Kind::Locked, true) if !behind => vec!["Picklock", "Crowbar"],
        (Kind::Travel { .. }, _) => vec!["Travel"],
        (Kind::Lever { .. }, _) => vec!["Interact"],
        _ => vec![if s.target { "Close" } else { "Open" }],
    }
}

/// The door BPs' interact branches. `has` tells whether the survivor carries an item.
fn press(d: &DoorDef, s: &DoorState, option: &str, behind: bool, has: impl Fn(&str) -> bool) -> Press {
    let card = |card: &str| {
        if has(ALL_ACCESS) {
            Press::Card { consume: None }
        } else if has(card) {
            Press::Card { consume: Some(card.to_string()) }
        } else {
            Press::NoCard
        }
    };
    match &d.kind {
        Kind::Travel { .. } => Press::Travel,
        Kind::Lever { .. } if s.locked => Press::NoPower,
        Kind::Lever { .. } => Press::Lever,
        Kind::Locked if s.locked && behind => Press::OpenFor(5.0),
        Kind::Locked if s.locked => match option {
            "Crowbar" => Press::Crowbar,
            _ => Press::Lockpick,
        },
        Kind::Sliding { keycard, from_behind, .. } => {
            let warning = &s.warning;
            if s.target {
                // Warning doors close by themselves (Event_AutoDoor ignores an open door).
                if *warning && s.close_after.is_some() { Press::Nothing } else { Press::Toggle }
            } else if *from_behind && behind {
                Press::OpenFor(5.0)
            } else if s.locked {
                card(keycard.as_deref().unwrap_or("DA_Level2Keycard"))
            } else if *warning {
                Press::OpenFor(5.0)
            } else {
                Press::Toggle
            }
        }
        Kind::Keycard { card: c, from_behind, behind_close, .. } => {
            if s.target {
                Press::Toggle
            } else if *from_behind && behind {
                behind_close.map_or(Press::Toggle, Press::OpenFor)
            } else {
                card(c)
            }
        }
        // Behind: Event_OpenDoorBehind (nothing while open). In front: the pad until the code
        // was entered, then MC_OpenDoor toggles.
        Kind::Keypad { .. } if behind => if s.target { Press::Nothing } else { Press::OpenFor(10.0) },
        Kind::Keypad { .. } if s.locked => Press::Keypad,
        Kind::Locked | Kind::Free | Kind::Keypad { .. } => Press::Toggle,
    }
}

/// XP per survivor level for the first keycard opening.
fn card_xp(kind: &Kind) -> u64 {
    match kind {
        Kind::Keycard { xp, .. } => *xp,
        Kind::Keypad { .. } => 200,
        Kind::Sliding { .. } => 400,
        _ => 0,
    }
}

#[derive(Resource, Default)]
pub struct Doors {
    states: Vec<DoorState>,
    parts: Vec<Vec<Entity>>,
    spawned: bool,
    option: usize,
}

impl Doors {
    /// How far door `i` is open (0 closed .. 1 open).
    pub fn open_fraction(&self, i: usize) -> f32 {
        self.states.get(i).map_or(0.0, |s| s.open)
    }

    /// Whether door `i` is (still) locked.
    pub fn locked(&self, i: usize) -> bool {
        self.states.get(i).is_some_and(|s| s.locked)
    }

    /// A lab generator's SetPower_TurnedOn (on: Unlocked) / SetPower_Warning (off) on door `i`;
    /// each state clears the others (OnRep_Unlocked / OnRep_Warning).
    pub fn set_power(&mut self, i: usize, on: bool) {
        if let Some(s) = self.states.get_mut(i) {
            s.locked = false;
            s.warning = !on;
        }
    }

    /// SetPower_TurnedOn on lever `i` (`PoweredOn`).
    pub fn set_lever_power(&mut self, i: usize, on: bool) {
        if let Some(s) = self.states.get_mut(i) {
            s.locked = !on;
        }
    }

    /// Door `i`'s status lamp (BP_LaboratorySlidingDoor's OnRep_Locked/Unlocked/Warning; with
    /// none set the frame keeps its mesh's M_LaboratoryStatus, the unlocked panel).
    pub fn lamp(&self, i: usize) -> super::power::Lamp {
        use super::power::Lamp;
        match self.states.get(i) {
            Some(s) if s.warning => Lamp::Warning,
            Some(s) if s.locked => Lamp::Locked,
            _ => Lamp::Unlocked,
        }
    }

    /// Door `i`'s keypad code (0 if it has none).
    pub fn keypad_code(&self, i: usize) -> u32 {
        self.states.get(i).map_or(0, |s| s.code)
    }
}

impl DoorDef {
    /// F opens it with nothing in hand, from the front.
    pub fn opens_freely(&self) -> bool {
        matches!(self.kind, Kind::Free | Kind::Sliding { keycard: None, warning: false, .. })
    }
}

pub(super) fn reset_doors(mut commands: Commands, mut cache: ResMut<CollisionCache>) {
    commands.insert_resource(Doors::default());
    cache.clear_dynamic();
}

/// Door meshes aren't in any streamed cell: request them up front (a handful of files).
pub(super) fn load_doors(level: Option<ResMut<Level>>, assets: Res<AssetServer>, doors: Res<Doors>) {
    let Some(mut level) = level else { return };
    if doors.spawned {
        return;
    }
    let ids: Vec<u32> = level.doors.iter().flat_map(|d| d.parts.iter().map(|p| p.mesh)).collect();
    for id in ids {
        let m = &mut level.meshes[id as usize];
        if m.gltf.is_none() {
            m.gltf = Some(assets.load(m.path.clone()));
        }
    }
}

#[derive(Component)]
pub(super) struct DoorPartOf(usize, usize);

/// Spawn every door's parts once their meshes are ready.
pub(super) fn spawn_doors(mut commands: Commands, level: Option<Res<Level>>, mut doors: ResMut<Doors>) {
    let Some(level) = level else { return };
    if doors.spawned || level.doors.iter().any(|d| d.parts.iter().any(|p| level.meshes[p.mesh as usize].parts.is_none())) {
        return;
    }
    doors.spawned = true;
    for (di, d) in level.doors.iter().enumerate() {
        let mut s = DoorState::new(&d.kind);
        if matches!(d.kind, Kind::Keypad { .. }) {
            s.code = rand::Rng::gen_range(&mut rand::thread_rng(), 1111..=9999);
        }
        doors.states.push(s);
        let mut ents = Vec::new();
        for (pi, p) in d.parts.iter().enumerate() {
            let root = commands.spawn((part_transform(p, 0.0), Visibility::default(), DoorPartOf(di, pi), DespawnOnExit(AppState::InGame))).id();
            for m in level.meshes[p.mesh as usize].parts.as_deref().unwrap_or_default() {
                commands.spawn((Mesh3d(m.mesh.clone()), MeshMaterial3d(m.material.clone()), Transform::IDENTITY, ChildOf(root)));
            }
            ents.push(root);
        }
        doors.parts.push(ents);
    }
}

/// Advance a door's timeline by `dt` (its own auto-close included).
fn step(s: &mut DoorState, d: &DoorDef, dt: f32) {
    let goal = if s.target { 1.0 } else { 0.0 };
    let rate = dt / d.secs.max(0.01);
    s.open = if s.open < goal { (s.open + rate).min(1.0) } else { (s.open - rate).max(0.0) };
    if s.target {
        s.since_open += dt;
        if s.close_after.or(d.auto_close).is_some_and(|c| s.since_open >= c) {
            s.target = false;
            s.close_after = None;
        }
    }
}

/// Animate doors and keep their collision where the door is.
pub(super) fn move_doors(
    time: Res<Time>,
    level: Option<Res<Level>>,
    mut doors: ResMut<Doors>,
    mut parts: Query<(&DoorPartOf, &mut Transform)>,
    mut cache: ResMut<CollisionCache>,
    meshes: Res<Assets<Mesh>>,
    mut sound: MessageWriter<crate::audio::Sound>,
) {
    let Some(level) = level else { return };
    let dt = time.delta_secs();
    for (i, s) in doors.states.iter_mut().enumerate() {
        if s.target != s.heard {
            s.heard = s.target;
            let d = &level.doors[i];
            for cue in door_sounds(&level.actors[d.actor].class, s.target) {
                sound.write(crate::audio::Sound::at(*cue, d.pos + Vec3::Y));
            }
        }
        let before = s.open;
        step(s, &level.doors[i], dt);
        if before != s.open || !cache.has_dynamic(i) {
            let mut tris = Vec::new();
            for p in &level.doors[i].parts {
                let t = part_transform(p, s.open);
                for m in level.meshes[p.mesh as usize].parts.as_deref().unwrap_or_default().iter().filter(|m| m.collide) {
                    if let Some(mesh) = meshes.get(&m.mesh) {
                        tris.extend(super::collision::tris_of(mesh, &t));
                    }
                }
            }
            cache.set_dynamic(i, tris);
        }
    }
    for (DoorPartOf(d, p), mut t) in &mut parts {
        if let Some(s) = doors.states.get(*d) {
            *t = part_transform(&level.doors[*d].parts[*p], s.open);
        }
    }
}

/// What a door class plays starting to open or close (the door BPs' PlaySoundAtLocation).
/// Wooden doors and the lever play nothing.
pub fn door_sounds(class: &str, opening: bool) -> &'static [&'static str] {
    match (class, opening) {
        ("BP_KeycardDoor_C" | "BP_KeycardDoor_Child_C" | "BP_KeypadDoor_C", true) => &["door_lock_open_02_Cue"],
        ("BP_KeycardDoor_C" | "BP_KeycardDoor_Child_C" | "BP_KeypadDoor_C", false) => &["door_lock_close_01_Cue"],
        ("BP_LaboratorySlidingDoor_C" | "BP_LaboratorySecureDoors_C", true) => &["door_lock_open_02_Cue"],
        ("BP_HangarDoor_C", true) => &["door_lock_open_02_Cue", "metal_gate_automatic_door_squeak_05_Cue"],
        ("BP_HangarDoor_C", false) => &["metal_gate_automatic_door_squeak_05_Cue"],
        ("BP_WarehouseDoors_C", _) => &["metal_gate_automatic_door_squeak_05_Cue"],
        _ => &[],
    }
}

/// The first opening's XP (survivor level x `card_xp`), once per door.
fn give_open_xp(s: &mut DoorState, kind: &Kind, game: &mut CurrentGame, notify: &mut MessageWriter<Notify>) {
    if s.xp_given {
        return;
    }
    s.xp_given = true;
    if let Some(state) = game.state.as_mut() {
        let xp = state.progression.level.max(1) as u64 * card_xp(kind);
        state.progression.add_xp(xp);
        notify.write(Notify::new(format!("Door Unlocked, XP Given: {xp}")));
    }
}

/// A code from the keypad: right, MC_OpenDoor (opening unlocks it for the session; an open door
/// just closes). Returns whether it opened (the XP is due); a wrong code changes nothing here.
fn keypad_code(s: &mut DoorState, correct: bool) -> bool {
    if !correct {
        return false;
    }
    if s.target {
        s.target = false;
        return false;
    }
    s.locked = false;
    s.open_now(None);
    true
}

/// A keypad note within reach of `p` (its door's index).
fn note_near(doors: &[DoorDef], p: Vec3) -> Option<usize> {
    doors.iter().position(|d| matches!(d.kind, Kind::Keypad { note: Some(n) } if (n.y - p.y).abs() < 3.0 && n.xz().distance(p.xz()) < REACH))
}

/// How far the nearest door, note, lab generator or light switch within reach is, so a farther
/// container leaves F to it.
pub fn reach_distance(doors: &[DoorDef], power: Option<&super::power::Power>, p: Vec3) -> Option<f32> {
    let note = note_near(doors, p).and_then(|i| match doors[i].kind {
        Kind::Keypad { note: Some(n) } => Some(n.distance(p)),
        _ => None,
    });
    let door = nearest(doors, p).map(|i| doors[i].pos.distance(p));
    let power = power.and_then(|w| w.target(p)).map(|t| t.1);
    note.into_iter().chain(door).chain(power).reduce(f32::min)
}

/// The nearest door within reach of `p`.
fn nearest(doors: &[DoorDef], p: Vec3) -> Option<usize> {
    doors
        .iter()
        .enumerate()
        .filter(|(_, d)| (d.pos.y - p.y).abs() < 3.0 && d.pos.xz().distance(p.xz()) < REACH)
        .min_by(|a, b| a.1.pos.distance(p).total_cmp(&b.1.pos.distance(p)))
        .map(|(i, _)| i)
}

/// The door prompt and F, when no container claimed the prompt this frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn interact_doors(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    screen: Res<State<MenuScreen>>,
    level: Option<Res<Level>>,
    store: Res<crate::menus::settings::SettingsStore>,
    mut doors: ResMut<Doors>,
    mut game: ResMut<CurrentGame>,
    mut containers: ResMut<Containers>,
    mut overlay: ResMut<GameOverlay>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<super::KinematicBody>)>,
    mut prompt: Query<&mut Visibility, With<Prompt>>,
    mut text: Query<&mut Text, With<PromptText>>,
    mut bar: Query<&mut Node, With<PromptBar>>,
    mut notify: MessageWriter<Notify>,
    mut noise: MessageWriter<crate::gameplay::zombies::Noise>,
    // (Bevy systems take at most 16 parameters.)
    (mut codes, mut power, mut garages, mut sound, driving, mut quest_tasks): (
        MessageReader<crate::gameplay::keypad::KeypadCode>,
        Option<ResMut<super::power::Power>>,
        ResMut<super::garage::Garages>,
        MessageWriter<crate::audio::Sound>,
        Res<crate::gameplay::vehicles::Driving>,
        MessageWriter<crate::gameplay::quests::DataTask>,
    ),
) {
    let Some(level) = level else { return };
    for c in codes.read() {
        let (Some(d), Some(s)) = (level.doors.get(c.door), doors.states.get_mut(c.door)) else { continue };
        if !c.correct {
            // AISense_Hearing.ReportNoiseEvent, MaxRange 8000 cm.
            noise.write(crate::gameplay::zombies::Noise { at: d.pos, range: 80.0 });
        } else if keypad_code(s, true) {
            give_open_xp(s, &d.kind, &mut game, &mut notify);
        }
    }
    // Doors picked open in the lockpicking minigame.
    for (i, d) in level.doors.iter().enumerate() {
        if let Some(s) = doors.states.get_mut(i)
            && s.locked
            && containers.unlocked.contains(&d.actor)
        {
            s.locked = false;
            s.open_now(None);
            if let Some(state) = game.state.as_mut() {
                let xp = state.progression.level.max(1) as u64 * 100;
                state.progression.add_xp(xp);
                notify.write(Notify::new(format!("Door Unlocked, XP Given: {xp}")));
            }
        }
    }
    let (Ok(mut me), Ok(mut vis)) = (player.single_mut(), prompt.single_mut()) else { return };
    if *screen.get() != MenuScreen::None || overlay.is_open() || *vis != Visibility::Hidden || driving.active() {
        return;
    }
    let p = me.translation;
    // A keypad door's note (BP_KeypadNote: "Keypad Note", read with F).
    if let Some(i) = note_near(&level.doors, p) {
        *vis = Visibility::Inherited;
        if let Ok(mut n) = bar.single_mut() {
            n.width = Val::Percent(0.0);
        }
        if let Ok(mut t) = text.single_mut() {
            // The note's interact option isn't in the dump (GetInteractOptions builds it); "Read" is ours.
            t.0 = "Keypad Note: [F] Read".into();
        }
        if keys.just_pressed(KeyCode::KeyF) {
            *overlay = GameOverlay::Note(i);
        }
        return;
    }
    // A lab generator or light switch nearer than any door.
    let target = power.as_ref().and_then(|w| w.target(p));
    let door = nearest(&level.doors, p);
    if let (Some((t, dist)), Some(power)) = (target, power.as_mut())
        && door.is_none_or(|i| dist < level.doors[i].pos.distance(p))
    {
        use super::power::{SwitchState, Target};
        *vis = Visibility::Inherited;
        if let Ok(mut n) = bar.single_mut() {
            n.width = Val::Percent(0.0);
        }
        // BP_PowerGenerator's options: Option1 Turn On/Off, Option2 Add Gasoline.
        let gen_opts = |g: usize| [if power.power_generators[g].on { "Turn Off" } else { "Turn On" }, "Add Gasoline"];
        if matches!(t, Target::PowerGenerator(_)) && scroll.delta.y != 0.0 {
            doors.option = (doors.option + 1) % 2;
        }
        let label = match t {
            Target::Generator(g) => format!("Generator: [F] {}", if power.generators[g].on { "Turn Off" } else { "Turn On" }),
            Target::PowerGenerator(g) => format!("Generator: [F] {}  [Wheel] change", gen_opts(g)[doors.option.min(1)]),
            // Its single option isn't in the dump (GetInteractOptions builds it); "Use" is ours.
            Target::Switch(k) if power.switches[k].state == SwitchState::Warning => "Light Switch: no power".to_string(),
            Target::Switch(_) => "Light Switch: [F] Use".to_string(),
        };
        if let Ok(mut t) = text.single_mut() {
            t.0 = label;
        }
        if keys.just_pressed(KeyCode::KeyF) {
            match t {
                Target::Generator(g) => {
                    if let Some(f) = power.toggle_generator(g) {
                        for actor in f.doors {
                            if let Some(di) = level.doors.iter().position(|d| d.actor == actor) {
                                doors.set_power(di, f.on);
                            }
                        }
                        let msg = if f.on { "Laboratory generator powered on..." } else { "Laboratory generator powered off..." };
                        notify.write(Notify::new(msg));
                        if f.on {
                            let argument = power.generators[g].quest_argument.clone();
                            quest_tasks.write(crate::gameplay::quests::DataTask { task: "PowerOnGenerator".into(), argument, quantity: 1 });
                        }
                    }
                }
                Target::Switch(k) => {
                    power.press_switch(k);
                }
                Target::PowerGenerator(g) if doors.option.min(1) == 1 => {
                    let has_can = player_inv(&mut game).count("DA_GasCan") > 0;
                    let (used, msg) = power.power_generators[g].add_gas(has_can);
                    if used {
                        player_inv(&mut game).remove("DA_GasCan", 1);
                    }
                    notify.write(Notify::new(msg));
                }
                Target::PowerGenerator(g) => {
                    let result = {
                        let inv = player_inv(&mut game);
                        power.power_generators[g].turn_on(|item| inv.count(item) > 0)
                    };
                    if let super::power::TurnOn::Switched { on, actors, consume } = &result {
                        if let Some(item) = consume {
                            player_inv(&mut game).remove(item, 1);
                        }
                        for actor in actors {
                            if let Some(di) = level.doors.iter().position(|d| d.actor == *actor) {
                                doors.set_lever_power(di, *on);
                            }
                        }
                    }
                    notify.write(Notify::new(result.message()));
                }
            }
        }
        return;
    }
    let Some(i) = door else { return };
    let d = &level.doors[i];
    let Some(s) = doors.states.get(i).cloned() else { return };
    let s = &s;
    let behind = (p - d.pos).dot(d.front) < 0.0;
    let opts = prompt_options(d, s, behind);
    if scroll.delta.y != 0.0 {
        doors.option = (doors.option + 1) % opts.len().max(1);
    }
    let pick = opts[doors.option.min(opts.len() - 1)];
    *vis = Visibility::Inherited;
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(0.0);
    }
    let name = match &d.kind {
        Kind::Locked if s.locked => "Locked Door",
        Kind::Travel { name, .. } => name,
        _ => &d.name,
    };
    if let Ok(mut t) = text.single_mut() {
        let more = if opts.len() > 1 { "  [Wheel] change" } else { "" };
        t.0 = format!("{name}: [F] {pick}{more}");
    }
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let action = {
        let inv = player_inv(&mut game);
        press(d, s, pick, behind, |item| inv.count(item) > 0)
    };
    let s = &mut doors.states[i];
    match action {
        Press::Nothing => {}
        Press::Toggle if s.target => s.target = false,
        Press::Toggle => s.open_now(None),
        Press::OpenFor(secs) => s.open_now(Some(secs)),
        Press::Lockpick => {
            let lock = *containers.locks.entry(d.actor).or_insert_with(|| crate::gameplay::lockpick::Lock::roll(&mut rand::thread_rng()));
            match crate::gameplay::lockpick::can_try(&mut game, &lock) {
                Ok(()) => *overlay = GameOverlay::Lockpick(d.actor),
                Err(why) => {
                    notify.write(Notify::new(why));
                }
            }
        }
        Press::Crowbar => {
            let need = 10.0 * crate::gameplay::survival::difficulty(&store, "General", "OTHER ITEMS DURABILITY MULTIPLIER");
            match player_inv(&mut game).find_mut("DA_Crowbar") {
                None => {
                    notify.write(Notify::new("No crowbar found"));
                }
                Some(c) if c.durability.unwrap_or(100.0) <= need => {
                    notify.write(Notify::new("Crowbar durability too low"));
                }
                Some(c) => {
                    c.durability = Some(c.durability.unwrap_or(100.0) - need);
                    noise.write(crate::gameplay::zombies::Noise { at: p, range: 75.0 });
                    // BP_LockedDoor's NoiseAudio: WoodBreak_cue.
                    sound.write(crate::audio::Sound::at("WoodBreak_cue", d.pos + Vec3::Y));
                    s.locked = false;
                    s.open_now(None);
                }
            }
        }
        Press::Card { consume } => {
            if let Some(card) = consume {
                player_inv(&mut game).remove(&card, 1);
            }
            s.locked = false;
            s.open_now(None);
            give_open_xp(s, &d.kind, &mut game, &mut notify);
        }
        Press::Keypad => *overlay = GameOverlay::Keypad(i),
        Press::NoCard => {
            notify.write(Notify::new("No Keycard Available"));
        }
        Press::NoPower => {
            notify.write(Notify::new("No Power To Lever"));
        }
        Press::Lever => {
            // LeverToggled? (true at first): PlayFromStart, else Reverse.
            s.target = !s.target;
            if let Kind::Lever { gates } = &d.kind {
                for g in gates {
                    garages.toggle(*g);
                }
            }
        }
        Press::Travel => {
            if let Kind::Travel { to, hours, .. } = &d.kind {
                if let Some(state) = game.state.as_mut() {
                    let env = &mut state.environment;
                    let t = env.time_of_day_hours + hours;
                    env.days += (t / 24.0) as u32;
                    env.time_of_day_hours = t.rem_euclid(24.0);
                }
                me.translation = *to;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doors_sound_like_their_blueprints() {
        assert_eq!(door_sounds("BP_KeycardDoor_C", true), ["door_lock_open_02_Cue"]);
        assert_eq!(door_sounds("BP_KeypadDoor_C", false), ["door_lock_close_01_Cue"]);
        assert!(door_sounds("BP_HangarDoor_C", true).contains(&"metal_gate_automatic_door_squeak_05_Cue"));
        assert!(door_sounds("BP_LockedDoor_C", true).is_empty(), "wooden doors are silent");
    }

    fn part(motion: Motion) -> DoorPart {
        DoorPart { mesh: 0, base: Transform::default(), motion }
    }

    fn door(kind: Kind) -> DoorDef {
        DoorDef { actor: 0, kind, parts: vec![], pos: Vec3::ZERO, front: Vec3::X, auto_close: None, secs: 0.5, name: "Door".into() }
    }

    fn keycard(from_behind: bool, behind_close: Option<f32>) -> Kind {
        Kind::Keycard { card: "DA_Level1Keycard".into(), xp: 200, from_behind, behind_close }
    }

    #[test]
    fn doors_swing_roll_and_slide() {
        let t = part_transform(&part(Motion::Yaw(90.0)), 1.0);
        assert!((t.rotation * Vec3::X).abs_diff_eq(Vec3::Z, 1e-5), "{:?}", t.rotation * Vec3::X);
        assert_eq!(part_transform(&part(Motion::Yaw(90.0)), 0.0), Transform::default());
        let s = part_transform(&part(Motion::Slide(Vec3::new(125.0, 0.0, 0.0))), 1.0);
        assert!((s.translation - Vec3::new(1.25, 0.0, 0.0)).length() < 1e-5);
        // UE Y is our Z.
        let s = part_transform(&part(Motion::Slide(Vec3::new(0.0, -100.0, 0.0))), 0.5);
        assert!((s.translation - Vec3::new(0.0, 0.0, -0.5)).length() < 1e-5);
        // Roll turns about X: the part's up tilts towards -Z for a positive UE roll.
        let r = part_transform(&part(Motion::Roll(90.0)), 1.0);
        assert!((r.rotation * Vec3::X).abs_diff_eq(Vec3::X, 1e-5));
        assert!((r.rotation * Vec3::Y).abs_diff_eq(-Vec3::Z, 1e-5), "{:?}", r.rotation * Vec3::Y);
    }

    #[test]
    fn locked_doors_pick_force_or_open_from_behind() {
        let d = door(Kind::Locked);
        let s = DoorState::new(&d.kind);
        assert!(s.locked);
        assert_eq!(prompt_options(&d, &s, false), ["Picklock", "Crowbar"]);
        assert_eq!(press(&d, &s, "Picklock", false, |_| true), Press::Lockpick);
        assert_eq!(press(&d, &s, "Crowbar", false, |_| true), Press::Crowbar);
        assert_eq!(prompt_options(&d, &s, true), ["Open"]);
        assert_eq!(press(&d, &s, "Open", true, |_| false), Press::OpenFor(5.0));
        let open = DoorState { locked: false, ..s };
        assert_eq!(press(&d, &open, "Open", false, |_| false), Press::Toggle);
    }

    #[test]
    fn keycard_doors_take_a_card_per_opening_unless_all_access() {
        let d = door(keycard(true, Some(10.0)));
        let closed = DoorState::new(&d.kind);
        assert!(!closed.locked);
        assert_eq!(press(&d, &closed, "Open", false, |_| false), Press::NoCard);
        assert_eq!(press(&d, &closed, "Open", false, |i| i == "DA_Level1Keycard"), Press::Card { consume: Some("DA_Level1Keycard".into()) });
        assert_eq!(press(&d, &closed, "Open", false, |i| i == ALL_ACCESS || i == "DA_Level1Keycard"), Press::Card { consume: None });
        // Closing is free, and so is opening from behind (for 10 s here).
        let open = DoorState { target: true, ..closed.clone() };
        assert_eq!(press(&d, &open, "Close", false, |_| false), Press::Toggle);
        assert_eq!(press(&d, &closed, "Open", true, |_| false), Press::OpenFor(10.0));
        // The hangar opens from behind for good; secure doors without the flag want the card.
        assert_eq!(press(&door(keycard(true, None)), &closed, "Open", true, |_| false), Press::Toggle);
        assert_eq!(press(&door(keycard(false, Some(5.0))), &closed, "Open", true, |_| false), Press::NoCard);
    }

    #[test]
    fn lab_sliding_doors_follow_locked_and_warning() {
        let locked = door(Kind::Sliding { keycard: Some("DA_Level3Keycard".into()), warning: false, from_behind: true });
        let s = DoorState::new(&locked.kind);
        assert!(s.locked);
        assert_eq!(press(&locked, &s, "Open", false, |_| false), Press::NoCard);
        assert_eq!(press(&locked, &s, "Open", false, |i| i == "DA_Level3Keycard"), Press::Card { consume: Some("DA_Level3Keycard".into()) });
        assert_eq!(press(&locked, &s, "Open", true, |_| false), Press::OpenFor(5.0));
        let warning = door(Kind::Sliding { keycard: None, warning: true, from_behind: false });
        let s = DoorState::new(&warning.kind);
        assert!(!s.locked);
        assert_eq!(press(&warning, &s, "Open", false, |_| false), Press::OpenFor(5.0));
        let plain = door(Kind::Sliding { keycard: None, warning: false, from_behind: false });
        assert_eq!(press(&plain, &DoorState::new(&plain.kind), "Open", false, |_| false), Press::Toggle);
        assert_eq!(card_xp(&plain.kind), 400);
    }

    #[test]
    fn keypad_doors_want_the_code_once_per_session() {
        let d = door(Kind::Keypad { note: None });
        let s = DoorState::new(&d.kind);
        assert!(s.locked);
        assert_eq!(press(&d, &s, "Open", false, |_| true), Press::Keypad);
        // From behind: open for 10 s, nothing while open.
        assert_eq!(press(&d, &s, "Open", true, |_| false), Press::OpenFor(10.0));
        assert_eq!(press(&d, &DoorState { target: true, ..s.clone() }, "Close", true, |_| false), Press::Nothing);
        let mut s = s;
        assert!(!keypad_code(&mut s, false));
        assert!(s.locked && !s.target);
        assert!(keypad_code(&mut s, true));
        // Unlocked and opening; F now toggles it. A right code at an open door closes it.
        assert!(!s.locked && s.target);
        assert_eq!(press(&d, &s, "Close", false, |_| false), Press::Toggle);
        assert!(!keypad_code(&mut s, true));
        assert!(!s.target && !s.locked);
        assert_eq!(card_xp(&d.kind), 200);
    }

    #[test]
    fn doors_and_notes_in_reach_give_their_distance() {
        let mut d = door(Kind::Keypad { note: Some(Vec3::new(10.0, 0.0, 0.0)) });
        d.pos = Vec3::new(0.0, 0.0, 2.0);
        let doors = [d];
        assert_eq!(reach_distance(&doors, None, Vec3::ZERO), Some(2.0));
        assert_eq!(reach_distance(&doors, None, Vec3::new(9.0, 0.0, 0.0)), Some(1.0));
        assert_eq!(note_near(&doors, Vec3::new(9.0, 0.0, 0.0)), Some(0));
        assert_eq!(reach_distance(&doors, None, Vec3::new(5.0, 0.0, 0.0)), None);
    }

    #[test]
    fn timeline_runs_for_its_length_and_doors_close_themselves() {
        let mut d = door(Kind::Free);
        d.secs = 4.0;
        let mut s = DoorState::default();
        s.open_now(Some(5.0));
        step(&mut s, &d, 2.0);
        assert!((s.open - 0.5).abs() < 1e-5);
        step(&mut s, &d, 2.0);
        assert_eq!(s.open, 1.0);
        step(&mut s, &d, 1.5);
        assert!(!s.target, "closes after 5 s");
        // A door's own auto-close (keycard doors: 1800 s) applies to any opening.
        d.auto_close = Some(1800.0);
        let mut s = DoorState::default();
        s.open_now(None);
        step(&mut s, &d, 1799.0);
        assert!(s.target);
        step(&mut s, &d, 2.0);
        assert!(!s.target);
    }

    #[test]
    fn lever_needs_power() {
        let d = DoorDef { actor: 0, kind: Kind::Lever { gates: vec![5] }, parts: vec![], pos: Vec3::ZERO, front: Vec3::X, auto_close: None, secs: 0.6, name: "Lever".into() };
        let mut s = DoorState::new(&d.kind);
        assert_eq!(prompt_options(&d, &s, false), ["Interact"]);
        assert_eq!(press(&d, &s, "Interact", false, |_| true), Press::NoPower);
        s.locked = false;
        assert_eq!(press(&d, &s, "Interact", true, |_| false), Press::Lever);
    }

    #[test]
    fn level_doors_find_their_meshes() {
        let dir = crate::level::LEVEL_DIR;
        let (Ok(a), Ok(p)) = (std::fs::read_to_string(format!("{dir}/actors.json")), std::fs::read_to_string(format!("{dir}/placements.json"))) else { return };
        let actors: Vec<ActorInfo> = serde_json::from_str(&a).unwrap();
        let placements = serde_json::from_str(&p).unwrap();
        let (doors, taken) = find(&actors, &placements);
        let count = |k: &str| doors.iter().filter(|(d, _)| actors[d.actor].class == k).count();
        assert_eq!(count("BP_LockedDoor_C"), 20);
        assert!(count("BP_LaboratorySlidingDoor_C") >= 35, "{}", count("BP_LaboratorySlidingDoor_C"));
        assert_eq!(count("BP_SubwayTravelDoor_C"), 6);
        assert_eq!(count("BP_KeycardDoor_C") + count("BP_KeycardDoor_Child_C"), 11);
        assert_eq!(count("BP_HangarDoor_C"), 1);
        assert_eq!(count("BP_LaboratoryEntranceDoors_C"), 2);
        assert_eq!(count("BP_LaboratorySecureDoors_C"), 2);
        // The oil rig's keypad door, named per instance, with its note ~40 m away.
        let (kp, _) = doors.iter().find(|(d, _)| actors[d.actor].class == "BP_KeypadDoor_C").expect("keypad door");
        assert_eq!(kp.name, "Oil Rig Security Door - Testing Facility");
        assert!(matches!(kp.kind, Kind::Keypad { note: Some(n) } if n.distance(kp.pos) < 60.0), "{:?}", kp.kind);
        // Every part of the multi-part doors resolves to a mesh.
        let full = |k: &str, n: usize| doors.iter().filter(|(d, _)| actors[d.actor].class == k).all(|(d, _)| d.parts.len() == n);
        assert!(full("BP_ContainerDoors_C", 2) && full("BP_HangarDoor_C", 2) && full("BP_LaboratoryEntranceDoors_C", 2) && full("BP_LaboratorySecureDoors_C", 3));
        assert_eq!(taken.len(), doors.iter().map(|(d, _)| d.parts.len()).sum::<usize>());
        // 5 Locked lab doors want a keycard; 12 Warning ones open for a while.
        let lab = |f: &dyn Fn(&Kind) -> bool| doors.iter().filter(|(d, _)| f(&d.kind)).count();
        assert_eq!(lab(&|k| matches!(k, Kind::Sliding { keycard: Some(_), .. })), 5);
        assert_eq!(lab(&|k| matches!(k, Kind::Sliding { warning: true, .. })), 12);
        assert!(doors.iter().any(|(d, _)| matches!(&d.kind, Kind::Keycard { card, .. } if card == "DA_Level3Keycard")));        // The police lever: its handle, working the garage door.
        let (lever, paths) = doors.iter().find(|(d, _)| actors[d.actor].class == "BP_Lever_C").expect("lever");
        assert!(paths[0].ends_with("/Lever"));
        assert!(matches!(&lever.kind, Kind::Lever { gates } if gates.iter().map(|&g| actors[g].class.as_str()).eq(["BP_PoliceStationGarageDoor_C"])));
        // Placed at pitch 75: pulled, it points 75° down instead.
        let fwd = |open: f32| part_transform(&lever.parts[0], open).rotation * Vec3::X;
        assert!(fwd(0.0).y > 0.95 && fwd(1.0).y < -0.95, "{:?} {:?}", fwd(0.0), fwd(1.0));
    }
}
