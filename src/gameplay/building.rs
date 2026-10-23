//! Building: the survivor's BuildingComponent (build mode), the placed Buildable_* actors
//! (Buildable_MASTER) and Buildable_HordeBeacon's horde, decompiled. Data:
//! `assets/data/buildables.json` (`extractor/gen_buildables.py`).
//!
//! - **Build mode** (JigContextMenuComp.ContextPlaceItem: an inventory item's "Place" option):
//!   Event_LaunchBuildMode with the item's data asset, then the inventory closes. Launching again
//!   while building stops instead. BuildCycle (every 0.01 s): a line from the camera along its
//!   view, ThirdPersonBuildingTraceDistance (850 cm) long; BuildTransform = the hit (else the end)
//!   plus AddLocation, its yaw RotationZ (its roll/pitch kept: R tilts the build onto the hit's
//!   normal, Shift+R levels it). The ghost (the item's BuildMesh, M_BuildingOutline_Green/Red)
//!   is green when it can be placed: BoundsCheck is a box at the mesh's bounds (÷
//!   BuildBoundsCheckMultiplier 1.5 while SnappingEnabled?, the default) + BuildBoundsOverlapOffset;
//!   with the line on something, an overlap only blocks while snapping, unless it's the landscape
//!   and the item's SnapType sits on it (Fence, Stairs, the Foundations); with the line in the air
//!   the build needs an overlap (the landscape one only, while snapping). Unless the item has
//!   BuildAllowBuildingInNoBuildZones, a line meeting a NoBuild-tagged actor (BP_NoBuildArea's
//!   40 m boxes, present while BLOCK BUILDING AT MAJOR POIs? is on; the beacon) turns it red and
//!   says "No build zone". Keys (BP_PlayerCharacter): LMB places (Svr_SpawnBuild), RMB stops,
//!   the wheel turns ±2°, Shift+wheel moves it ±5 cm along its up, Shift+E/Q ±90°, Shift+F
//!   toggles snapping; BP_Ingame's BuildBox lists them and BuildingDegrees shows RotationZ.
//! - **SpawnBuild**: the BuildActorClass at BuildTransform, S_Build_Cue, one of the item used up;
//!   with more of it left build mode carries on (FindJigItemByDA), else it stops. Moving a placed
//!   buildable (ReplacingObject: build mode with ReplacingActor, its collision off) sets its
//!   transform instead.
//! - **Buildable_MASTER**: the actor's static mesh parts (its construction script, collision
//!   where the component has it), its InteractOptions: "Move" and "Pickup" (back into the
//!   inventory, JigTryAddItemSomewhere). Placed buildables are saved (`GameState::buildables`).
//! - **Buildable_HordeBeacon** ("Interact"): TraceForBuildables (another buildable within 10 m:
//!   "Buildable blocking beacon signal"), else Event_StartHorde: "Horde Beacon Activated...",
//!   the dish turning (its 12 s Rotate Alpha timeline, ±45°, RInterpTo 3), 5 s, then "Horde
//!   Beginning in 30/20/10/5 Seconds..." (10, 10, 5, 5 s; Interact again meanwhile stops it:
//!   "Stopping Horde Beacon"). Each wave (DT_HordeSystem's HordeBeacon row: 4, 6, 8, 12, 15
//!   BP_Zombie_Infected_Horde, a Military_Small boss, two, then the Military boss): "Wave: N
//!   Begin!", 2 s, then one AI every 0.5-1 s at FindLocation (a point within 8.5 m, 2 m further
//!   out at a random yaw, projected down from 5 m up), each sent to the survivor's position
//!   (SetBB_LockOnTarget: MoveToSpecificLoc within 5 m of it, at walking pace). 5 s later and
//!   every 4 s: all dead? "Wave Completed!\nNext Wave in 15 Seconds...", or after the last
//!   "Final wave defeated!", 1 s, SpawnRewards (the Rewards list shuffled, each at its
//!   SpawnChance, up to MaxItemsToSpawn 8, at RewardSpawnLoc, ±1 m apart after the second),
//!   FX_Spark_Impact, metal_med_impact_01_Cue, and the beacon is gone. Once the waves start it
//!   can't be stopped, moved or picked up ("Horde Already Started!"). The survivor's death turns
//!   every beacon back into its pickup (Event_PlayerDead -> Event_SpawnBeacon).
//!
//! Ours: interaction is by distance (`REACH`), not the look trace; the ghost's materials, the
//! light ray's pulsing red and the dish curve (linear between its keys) are stand-ins (the
//! material graphs aren't readable); an AI despawned by the optimiser counts as dead (in the
//! game an invalid enemy never does); FindLocation has no navmesh (any point in the disc). Not
//! ported: snapping to other buildables' sockets (modular walls, floors), the buildables'
//! own functions (storage, lights, benches, beds, doors), their health, the horde music
//! (MusicLayeringComponent) and pausing the area music, the grab montages, ACH_Builder.

use std::collections::HashMap;

use bevy::gltf::{Gltf, GltfMesh};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::combat::Dead;
use super::interact::{GameOverlay, player_inv};
use super::notify::Notify;
use crate::level::collision::PropRays;
use crate::level::{KinematicBody, LevelZombie, Terrain};
use crate::menus::settings::SettingsStore;
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::CharacterModel;

/// BP_PlayerCharacter's ThirdPersonBuildingTraceDistance (the port has no first person).
const TRACE: f32 = 8.5;
/// ChangeRotationZ steps: the wheel, Shift+E/Q.
const WHEEL_DEGREES: f32 = 2.0;
const QUARTER: f32 = 90.0;
/// Shift+wheel: AddLocation along the ghost's up.
const OFFSET_STEP: f32 = 0.05;
/// Event_NoBuildZone's Delay before its "No build zone".
const NO_BUILD_TOAST: f32 = 2.0;
/// How near the survivor must be to use a placed buildable (ours; the game traces the look).
const REACH: f32 = 2.5;
/// TraceForBuildables' sphere.
const BEACON_CLEARANCE: f32 = 10.0;
/// FindLocation: GetRandomLocationInNavigableRadius 850 cm, then (200, 0, 500) cm turned by a
/// random yaw; ProjectLocation traces 4000 x 2.5 cm down.
const SPAWN_RADIUS: f32 = 8.5;
const SPAWN_OUT: f32 = 2.0;
const SPAWN_UP: f32 = 5.0;
const PROJECT_DOWN: f32 = 100.0;
/// AI_MoveToSpecificLoc.GetTargetLoc: a reachable point within 500 cm of the target.
const LOCK_ON_SPREAD: f32 = 5.0;
/// Collision keys of placed buildables (`CollisionCache::set_dynamic`).
const COLLIDE_KEY: usize = 1 << 44;
const BEACON: &str = "Buildable_HordeBeacon_C";
/// CreateNotificationUI's LinearColor(1, 0, 0) on the beacon's messages.
const RED: Color = Color::linear_rgb(1.0, 0.0, 0.0);

// --- data ---------------------------------------------------------------------------------

/// What BuildingComponent reads off a buildable item's data asset.
#[derive(Deserialize, Clone, Debug)]
pub struct BuildItem {
    /// BuildMesh: the ghost.
    pub mesh: Option<String>,
    /// BuildActorClass.
    pub actor: String,
    /// SnapType (Enum_BuildingSnapTypes' display name).
    pub snap: String,
    pub bounds_multiplier: f32,
    /// BuildBoundsOverlapOffset (UE cm, the mesh's frame).
    pub overlap_offset: [f32; 3],
    pub allow_no_build: bool,
}

impl BuildItem {
    /// IsPlaceableOnLandscape's snap types.
    pub fn sits_on_landscape(&self) -> bool {
        matches!(self.snap.as_str(), "Fence" | "Stairs" | "StairsHalf" | "Foundation" | "FoundationHalf" | "FoundationQuarter" | "FoundationTriangle")
    }

    /// The overlap offset in glb space (metres).
    fn offset(&self) -> Vec3 {
        let [x, y, z] = self.overlap_offset;
        Vec3::new(x, z, y) / 100.0
    }
}

/// A static mesh part of a buildable actor, relative to its parent part (or the actor).
#[derive(Deserialize, Clone, Debug)]
pub struct PartDef {
    pub name: String,
    pub parent: Option<String>,
    pub mesh: String,
    #[serde(default)]
    pub materials: Vec<Option<String>>,
    pub collides: bool,
    pub t: [f32; 3],
    pub r: [f32; 4],
    pub s: [f32; 3],
}

impl PartDef {
    pub fn local(&self) -> Transform {
        Transform { translation: Vec3::from_array(self.t), rotation: Quat::from_array(self.r).normalize(), scale: Vec3::from_array(self.s) }
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct ActorDef {
    /// InteractOptions' labels, in order.
    pub options: Vec<String>,
    pub tags: Vec<String>,
    /// PickupDataAsset: the item it goes back to.
    pub pickup: Option<String>,
    pub parts: Vec<PartDef>,
}

impl ActorDef {
    /// A part's transform relative to the actor (its parents' applied).
    pub fn part_transform(&self, i: usize) -> Transform {
        let mut t = self.parts[i].local();
        let mut up = self.parts[i].parent.as_deref();
        let mut guard = 0;
        while let Some(name) = up
            && guard < 8
        {
            let Some(p) = self.parts.iter().find(|p| p.name == name) else { break };
            t = p.local() * t;
            up = p.parent.as_deref();
            guard += 1;
        }
        t
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Wave {
    /// (AI class, Amount).
    pub groups: Vec<(String, u32)>,
    #[serde(rename = "final")]
    pub last: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Reward {
    pub item: String,
    pub chance: f32,
    pub min: u32,
    pub max: u32,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct HordeDef {
    pub waves: Vec<Wave>,
    pub rewards: Vec<Reward>,
    pub max_rewards: u32,
    /// RewardSpawnLoc (glb space, metres).
    pub reward_at: [f32; 3],
    pub dish_secs: f32,
    /// The dish timeline's Rotate Alpha keys (s, alpha).
    pub dish_alpha: Vec<[f32; 2]>,
}

impl HordeDef {
    /// Rotate Alpha at `t` seconds into the loop (linear between the keys: ours).
    pub fn dish_alpha(&self, t: f32) -> f32 {
        let k = &self.dish_alpha;
        let Some(first) = k.first() else { return 0.0 };
        if t <= first[0] {
            return first[1];
        }
        for w in k.windows(2) {
            if t <= w[1][0] {
                let f = (t - w[0][0]) / (w[1][0] - w[0][0]).max(1e-6);
                return w[0][1] + (w[1][1] - w[0][1]) * f;
            }
        }
        k.last().map_or(0.0, |l| l[1])
    }
}

#[derive(Resource, Deserialize, Default)]
pub struct BuildableDb {
    pub items: HashMap<String, BuildItem>,
    pub actors: HashMap<String, ActorDef>,
    pub horde: HordeDef,
    /// BP_NoBuildArea's Box extent (UE cm).
    pub no_build: [f32; 3],
}

impl BuildableDb {
    pub fn load() -> Self {
        std::fs::read_to_string("assets/data/buildables.json").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }
}

// --- placement rules ----------------------------------------------------------------------

/// What the ghost's bounds box overlaps (BoundsCheck's BoxTraceSingle).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Overlap {
    None,
    /// Only the landscape.
    Landscape,
    /// Anything else (props, other buildables).
    Other,
}

/// BuildCycle's colour: can the build go here? `on_something`: the view line hit something.
pub fn can_build(on_something: bool, snapping: bool, overlap: Overlap, sits_on_landscape: bool) -> bool {
    // BoundsCheck(InvertBool): hit nothing -> false; hit with snapping off -> true; on the
    // landscape with a landscape snap type -> InvertBool; anything else -> !InvertBool.
    let bounds_hit = |invert: bool| match overlap {
        Overlap::None => false,
        _ if !snapping => true,
        Overlap::Landscape if sits_on_landscape => invert,
        _ => !invert,
    };
    if on_something { !bounds_hit(false) || !snapping } else { bounds_hit(true) }
}

/// ChangeRotationZ: RotationZ ± `by`, wrapped once into 0..=360.
pub fn change_rotation(z: f32, increase: bool, by: f32) -> f32 {
    let z = if increase { z + by } else { z - by };
    if z < 0.0 {
        z + 360.0
    } else if z > 360.0 {
        z - 360.0
    } else {
        z
    }
}

/// Where a BP_NoBuildArea box keeps builds out: a line from `from` to `to` entering it (a line
/// trace starting inside a box doesn't report it). `axes` are its unit axes, `half` its extents.
pub fn line_enters_box(from: Vec3, to: Vec3, centre: Vec3, axes: [Vec3; 3], half: Vec3) -> bool {
    let local = |p: Vec3| Vec3::new((p - centre).dot(axes[0]), (p - centre).dot(axes[1]), (p - centre).dot(axes[2]));
    let (a, b) = (local(from), local(to));
    if a.abs().cmple(half).all() {
        return false;
    }
    // Slab test.
    let d = b - a;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if a[i].abs() > half[i] {
                return false;
            }
            continue;
        }
        let (t0, t1) = ((-half[i] - a[i]) / d[i], (half[i] - a[i]) / d[i]);
        lo = lo.max(t0.min(t1));
        hi = hi.min(t0.max(t1));
        if lo > hi {
            return false;
        }
    }
    true
}

// --- the horde ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Idle,
    /// "Horde Beacon Activated...", 5 s.
    Activated,
    /// After "Horde Beginning in {30, 20, 10, 5} Seconds..." (stage 0-3).
    Countdown(u8),
    /// "Wave: N Begin!", 2 s.
    WaveIntro,
    /// Spawning group `group`'s `k`-th AI (from 1) next.
    Spawning { group: usize, k: u32 },
    /// The 5 s after the last spawn, then the 4 s between EnemyDefeatedChecks.
    Checking,
    /// "Wave Completed!", 15 s.
    NextWave,
    /// "Final wave defeated!", 1 s.
    Final,
    /// Rewards out, the beacon goes.
    Done,
}

/// Buildable_HordeBeacon's state.
#[derive(Clone, Debug)]
pub struct Horde {
    pub started: bool,
    pub can_stop: bool,
    pub wave: usize,
    pub phase: Phase,
    /// Seconds until the phase's next step.
    pub timer: f32,
    pub enemies: Vec<Entity>,
}

impl Default for Horde {
    fn default() -> Self {
        Self { started: false, can_stop: false, wave: 0, phase: Phase::Idle, timer: 0.0, enemies: Vec::new() }
    }
}

/// What the horde asks of the world.
#[derive(Clone, Debug, PartialEq)]
pub enum HordeOut {
    Toast(String),
    /// Spawn one AI of this class (FindLocation may fail).
    Spawn(String),
    Dish(bool),
    /// SpawnRewards, the spark and the sound, then the beacon is destroyed.
    Finish,
}

const COUNTDOWN: [(&str, f32); 4] = [
    ("Horde Beginning in 30 Seconds...", 10.0),
    ("Horde Beginning in 20 Seconds...", 10.0),
    ("Horde Beginning in 10 Seconds...", 5.0),
    ("Horde Beginning in 5 Seconds...", 5.0),
];

impl Horde {
    /// Interact (Event_StartHorde): start it, or stop it during the countdown.
    pub fn interact(&mut self, out: &mut Vec<HordeOut>) {
        if self.started {
            if self.can_stop {
                self.stop(out);
            }
            return;
        }
        self.started = true;
        self.can_stop = true;
        self.phase = Phase::Activated;
        self.timer = 5.0;
        out.push(HordeOut::Dish(true));
        out.push(HordeOut::Toast("Horde Beacon Activated...".into()));
    }

    /// Event_StopHorde.
    fn stop(&mut self, out: &mut Vec<HordeOut>) {
        out.push(HordeOut::Toast("Stopping Horde Beacon".into()));
        self.started = false;
        self.phase = Phase::Idle;
        out.push(HordeOut::Dish(false));
    }

    /// Pickup (OnDestroy): whether it may go back into the inventory.
    pub fn pickup(&mut self, out: &mut Vec<HordeOut>) -> bool {
        if !self.started {
            return true;
        }
        if self.can_stop {
            self.stop(out);
        } else {
            out.push(HordeOut::Toast("Horde Already Started!".into()));
        }
        false
    }

    /// Event_NextWave.
    fn next_wave(&mut self, out: &mut Vec<HordeOut>) {
        if !self.started {
            self.phase = Phase::Idle;
            return;
        }
        out.push(HordeOut::Toast(format!("Wave: {} Begin!", self.wave + 1)));
        self.phase = Phase::WaveIntro;
        self.timer = 2.0;
    }

    /// Advance by `dt`. `all_dead`: EnemyDefeatedCheck over the enemies spawned so far.
    pub fn tick(&mut self, dt: f32, def: &HordeDef, all_dead: impl Fn(&[Entity]) -> bool, rng: &mut impl Rng) -> Vec<HordeOut> {
        let mut out = Vec::new();
        if matches!(self.phase, Phase::Idle | Phase::Done) {
            return out;
        }
        self.timer -= dt;
        if self.timer > 0.0 {
            return out;
        }
        match self.phase {
            Phase::Activated | Phase::Countdown(_) => {
                let stage = match self.phase {
                    Phase::Countdown(s) => s + 1,
                    _ => 0,
                };
                match COUNTDOWN.get(stage as usize) {
                    Some(&(text, wait)) if self.started => {
                        out.push(HordeOut::Toast(text.into()));
                        self.phase = Phase::Countdown(stage);
                        self.timer = wait;
                    }
                    _ => {
                        self.can_stop = false;
                        self.next_wave(&mut out);
                    }
                }
            }
            Phase::WaveIntro => {
                self.phase = Phase::Spawning { group: 0, k: 1 };
                self.timer = 0.0;
                return [out, self.tick(0.0, def, all_dead, rng)].concat();
            }
            Phase::Spawning { group, k } => {
                let groups = def.waves.get(self.wave).map(|w| w.groups.as_slice()).unwrap_or_default();
                match groups.get(group) {
                    Some((class, n)) if k <= *n => {
                        out.push(HordeOut::Spawn(class.clone()));
                        self.phase = Phase::Spawning { group, k: k + 1 };
                        self.timer = rng.gen_range(0.5..=1.0);
                    }
                    Some(_) => {
                        self.phase = Phase::Spawning { group: group + 1, k: 1 };
                        self.timer = 0.0;
                        return [out, self.tick(0.0, def, all_dead, rng)].concat();
                    }
                    None => {
                        self.phase = Phase::Checking;
                        self.timer = 5.0;
                    }
                }
            }
            Phase::Checking => {
                if !all_dead(&self.enemies) {
                    self.timer = 4.0;
                } else if def.waves.get(self.wave).is_none_or(|w| w.last) {
                    // FinalWaveDefeated?
                    out.push(HordeOut::Toast("Final wave defeated!".into()));
                    self.started = false;
                    out.push(HordeOut::Dish(false));
                    self.phase = Phase::Final;
                    self.timer = 1.0;
                } else {
                    out.push(HordeOut::Toast("Wave Completed!\r\nNext Wave in 15 Seconds...".into()));
                    self.phase = Phase::NextWave;
                    self.timer = 15.0;
                }
            }
            Phase::NextWave => {
                self.wave += 1;
                self.next_wave(&mut out);
            }
            Phase::Final => {
                self.phase = Phase::Done;
                out.push(HordeOut::Finish);
            }
            Phase::Idle | Phase::Done => {}
        }
        out
    }
}

/// SpawnRewards: the list shuffled, each at its SpawnChance (rand(0, 100) <= chance), up to
/// MaxItemsToSpawn; (item, count, offset in metres: none for the first two).
pub fn roll_rewards(def: &HordeDef, rng: &mut impl Rng) -> Vec<(String, u32, Vec2)> {
    let mut list: Vec<&Reward> = def.rewards.iter().collect();
    list.shuffle(rng);
    let mut out = Vec::new();
    for r in list {
        if rng.gen_range(0.0..=100.0) > r.chance {
            continue;
        }
        let spread = if out.len() > 1 { Vec2::new(rng.gen_range(-1.0..=1.0), rng.gen_range(-1.0..=1.0)) } else { Vec2::ZERO };
        out.push((r.item.clone(), rng.gen_range(r.min..=r.max.max(r.min)), spread));
        if out.len() as u32 >= def.max_rewards {
            break;
        }
    }
    out
}

// --- the world ----------------------------------------------------------------------------

/// A placed buildable as saved.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlacedBuildable {
    /// The item's data asset (its BuildActorClass is looked up again on load).
    pub item: String,
    pub pos: [f32; 3],
    pub rot: [f32; 4],
}

/// The inventory's "Place".
#[derive(Message, Clone)]
pub struct LaunchBuild {
    pub item: String,
}

/// A placed buildable in this session.
pub struct Live {
    pub id: usize,
    pub item: String,
    pub actor: String,
    pub transform: Transform,
    pub entity: Entity,
    /// The beacon's dish part and its yaw (degrees), and the timeline's time.
    dish: Option<(Entity, f32, f32)>,
    pub horde: Option<Horde>,
    /// Its collision is in the cache.
    solid: bool,
}

#[derive(Resource, Default)]
pub struct Buildables {
    pub list: Vec<Live>,
    next_id: usize,
    restored: bool,
    /// The option the wheel picked on the buildable in reach.
    option: usize,
}

/// BuildingComponent's state.
#[derive(Resource)]
pub struct BuildMode {
    /// DARef: the item being placed (`None` = BuildModeOn? off).
    pub item: Option<String>,
    /// ReplacingActor: the placed buildable being moved (by id).
    pub replacing: Option<usize>,
    /// RotationZ (degrees).
    pub rotation_z: f32,
    /// AddLocation (metres).
    pub add_location: Vec3,
    /// SnappingEnabled? (on by default).
    pub snapping: bool,
    /// The kept roll/pitch of BuildTransform (R tilts it onto the surface).
    tilt: Quat,
    pub transform: Transform,
    /// The view line's hit normal.
    normal: Vec3,
    pub can_build: bool,
    ghost: Option<Entity>,
    /// Event_NoBuildZone's pending Delay.
    no_build_in: Option<f32>,
    /// R / Shift+R are DoOnce until released.
    r_held: bool,
}

impl Default for BuildMode {
    fn default() -> Self {
        Self {
            item: None,
            replacing: None,
            rotation_z: 0.0,
            add_location: Vec3::ZERO,
            snapping: true,
            tilt: Quat::IDENTITY,
            transform: Transform::default(),
            normal: Vec3::Y,
            can_build: false,
            ghost: None,
            no_build_in: None,
            r_held: false,
        }
    }
}

impl BuildMode {
    pub fn on(&self) -> bool {
        self.item.is_some()
    }

    fn rotation(&self) -> Quat {
        // UE yaw turns X towards Y: about +Y the other way here.
        self.tilt * Quat::from_rotation_y(-self.rotation_z.to_radians())
    }
}

/// Run condition: not in build mode (its keys are the weapons').
pub fn not_building(build: Res<BuildMode>) -> bool {
    !build.on()
}

/// A placed buildable's root.
#[derive(Component)]
pub struct Placed(pub usize);

#[derive(Component)]
struct Ghost;

/// Every mesh under it is drawn with this material.
#[derive(Component)]
struct Recolor(Handle<StandardMaterial>);

#[derive(Component)]
struct BuildBox;

#[derive(Component)]
struct SnapText;

#[derive(Component)]
struct DegreesText;

/// A horde AI sent to a place (SetBB_LockOnTarget).
#[derive(Component)]
pub struct LockOnTarget(pub Vec3);

/// Materials and mesh bounds.
#[derive(Resource, Default)]
struct BuildAssets {
    green: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
    pulse: Handle<StandardMaterial>,
    gltfs: HashMap<String, Handle<Gltf>>,
}

/// The build mode and buildable systems (the tour drives them from before).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildSet;

pub fn plugin(app: &mut App) {
    app.insert_resource(BuildableDb::load())
        .init_resource::<BuildMode>()
        .init_resource::<Buildables>()
        .init_resource::<BuildAssets>()
        .add_message::<LaunchBuild>()
        .add_systems(OnEnter(AppState::InGame), reset)
        .add_systems(
            Update,
            (restore, launch, build_input, build_cycle, spawn_build, make_solid, recolor, pulse, build_hud)
                .chain()
                .in_set(BuildSet)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (interact.after(crate::level::doors::interact_doors).after(super::turrets::disarm), run_hordes, lock_on, beacons_on_death)
                .chain()
                .in_set(BuildSet)
                .run_if(in_state(AppState::InGame))
                .run_if(in_state(MenuScreen::None)),
        );
}

fn reset(mut build: ResMut<BuildMode>, mut placed: ResMut<Buildables>, mut materials: ResMut<Assets<StandardMaterial>>, mut assets: ResMut<BuildAssets>) {
    *build = BuildMode { rotation_z: build.rotation_z, ..default() };
    *placed = Buildables::default();
    // M_BuildingOutline_Green / _Red and M_PulsingLight_Camera (graphs unreadable: ours).
    let glow = |c: Color| StandardMaterial { base_color: c, emissive: c.to_linear() * 2.0, unlit: true, alpha_mode: AlphaMode::Blend, ..default() };
    assets.green = materials.add(glow(Color::srgba(0.1, 1.0, 0.2, 0.45)));
    assets.red = materials.add(glow(Color::srgba(1.0, 0.1, 0.1, 0.45)));
    assets.pulse = materials.add(StandardMaterial { alpha_mode: AlphaMode::Add, ..glow(Color::srgba(1.0, 0.05, 0.02, 0.5)) });
}

fn glb(path: &str) -> String {
    format!("game/{path}.glb")
}

/// Spawn a buildable actor's parts under a root at `t`.
fn spawn_actor(commands: &mut Commands, assets: &AssetServer, build_assets: &BuildAssets, def: &ActorDef, id: usize, t: Transform) -> (Entity, Option<Entity>) {
    let root = commands.spawn((t, Visibility::default(), Placed(id), DespawnOnExit(AppState::InGame))).id();
    let mut ents: HashMap<&str, Entity> = HashMap::new();
    let mut dish = None;
    // Parents come before children in the construction script.
    for p in &def.parts {
        let mut e = commands.spawn((crate::models::model(assets, glb(&p.mesh)), p.local()));
        if let Some(Some(m)) = p.materials.first() {
            if m == "M_PulsingLight_Camera" {
                e.insert(Recolor(build_assets.pulse.clone()));
            } else {
                e.insert(crate::models::MaterialOverride(m.clone()));
            }
        }
        let e = e.id();
        let parent = p.parent.as_deref().and_then(|n| ents.get(n).copied()).unwrap_or(root);
        commands.entity(parent).add_child(e);
        ents.insert(&p.name, e);
        if p.name == "Dish" {
            dish = Some(e);
        }
    }
    (root, dish)
}

fn save(game: &mut CurrentGame, placed: &Buildables) {
    if let Some(state) = game.state.as_mut() {
        state.buildables = placed
            .list
            .iter()
            .map(|l| PlacedBuildable { item: l.item.clone(), pos: l.transform.translation.to_array(), rot: l.transform.rotation.to_array() })
            .collect();
    }
}

fn add_live(commands: &mut Commands, assets: &AssetServer, build_assets: &BuildAssets, db: &BuildableDb, placed: &mut Buildables, item: &str, t: Transform) -> Option<usize> {
    let bi = db.items.get(item)?;
    let def = db.actors.get(&bi.actor)?;
    let id = placed.next_id;
    placed.next_id += 1;
    let (entity, dish) = spawn_actor(commands, assets, build_assets, def, id, t);
    let horde = (bi.actor == BEACON).then(Horde::default);
    placed.list.push(Live { id, item: item.to_string(), actor: bi.actor.clone(), transform: t, entity, dish: dish.map(|d| (d, 0.0, 0.0)), horde, solid: false });
    Some(id)
}

/// The saved buildables come back once the world is up.
fn restore(mut commands: Commands, game: Res<CurrentGame>, db: Res<BuildableDb>, assets: Res<AssetServer>, build_assets: Res<BuildAssets>, mut placed: ResMut<Buildables>) {
    if placed.restored {
        return;
    }
    let Some(state) = game.state.as_ref() else { return };
    placed.restored = true;
    for b in &state.buildables {
        let t = Transform::from_translation(Vec3::from_array(b.pos)).with_rotation(Quat::from_array(b.rot).normalize());
        if add_live(&mut commands, &assets, &build_assets, &db, &mut placed, &b.item, t).is_none() {
            warn!("buildables: unknown saved buildable {}", b.item);
        }
    }
    if !state.buildables.is_empty() {
        info!("buildables: restored {}", state.buildables.len());
    }
}

/// Event_StopBuildMode.
fn stop_build(commands: &mut Commands, build: &mut BuildMode, placed: &mut Buildables) {
    if let Some(id) = build.replacing.take()
        && let Some(l) = placed.list.iter_mut().find(|l| l.id == id)
    {
        // SetActorEnableCollision(true): `make_solid` puts it back.
        l.solid = false;
    }
    if let Some(g) = build.ghost.take() {
        commands.entity(g).despawn();
    }
    build.item = None;
    build.add_location = Vec3::ZERO;
    build.no_build_in = None;
}

/// Event_LaunchBuildMode (from the inventory or a buildable's Move).
fn start_build(build: &mut BuildMode, item: String, replacing: Option<usize>) {
    build.item = Some(item);
    build.replacing = replacing;
    build.can_build = false;
}

#[allow(clippy::too_many_arguments)]
fn launch(
    mut commands: Commands,
    mut msgs: MessageReader<LaunchBuild>,
    mut build: ResMut<BuildMode>,
    mut overlay: ResMut<GameOverlay>,
    mut placed: ResMut<Buildables>,
    db: Res<BuildableDb>,
) {
    for m in msgs.read() {
        if build.on() {
            stop_build(&mut commands, &mut build, &mut placed);
            continue;
        }
        if !db.items.contains_key(&m.item) {
            warn!("buildables: {} has no build data", m.item);
            continue;
        }
        info!("build mode: {}", m.item);
        start_build(&mut build, m.item.clone(), None);
        // ContextPlaceItem's ToggleInventory.
        if overlay.shows_inventory() {
            *overlay = GameOverlay::None;
        }
    }
}

/// BP_PlayerCharacter's build-mode keys.
#[allow(clippy::too_many_arguments)]
fn build_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    overlay: Res<GameOverlay>,
    vitals: Res<super::PlayerVitals>,
    mut build: ResMut<BuildMode>,
    mut placed: ResMut<Buildables>,
    menu: Res<State<MenuScreen>>,
) {
    if !build.on() {
        return;
    }
    if vitals.is_dead() {
        stop_build(&mut commands, &mut build, &mut placed);
        return;
    }
    if overlay.is_open() || *menu.get() != MenuScreen::None {
        return;
    }
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    // IA_Aim: Event_StopBuildMode.
    if mouse.just_pressed(MouseButton::Right) {
        stop_build(&mut commands, &mut build, &mut placed);
        return;
    }
    let wheel = scroll.delta.y;
    if wheel != 0.0 {
        if shift {
            let up = build.transform.rotation * Vec3::Y;
            build.add_location += up * OFFSET_STEP * wheel.signum();
        } else {
            build.rotation_z = change_rotation(build.rotation_z, wheel > 0.0, WHEEL_DEGREES);
        }
    }
    if shift && keys.just_pressed(KeyCode::KeyE) {
        build.rotation_z = change_rotation(build.rotation_z, true, QUARTER);
    }
    if shift && keys.just_pressed(KeyCode::KeyQ) {
        build.rotation_z = change_rotation(build.rotation_z, false, QUARTER);
    }
    if shift && keys.just_pressed(KeyCode::KeyF) {
        build.snapping = !build.snapping;
    }
    // R: MakeRotFromZX(Normal, the ghost's forward); Shift+R: level again. Each once per press.
    if keys.just_released(KeyCode::KeyR) {
        build.r_held = false;
    }
    if keys.just_pressed(KeyCode::KeyR) && !build.r_held {
        build.r_held = true;
        build.tilt = if shift { Quat::IDENTITY } else { Quat::from_rotation_arc(Vec3::Y, build.normal.normalize_or(Vec3::Y)) };
    }
}

/// A line against the terrain: the first point under it within `max`, refined.
fn terrain_ray(terrain: &Terrain, origin: Vec3, dir: Vec3, max: f32) -> Option<(f32, Vec3)> {
    let mut prev = 0.0;
    let mut d = 0.1;
    while d <= max {
        if terrain.blocks(origin + dir * d) {
            let (mut lo, mut hi) = (prev, d);
            for _ in 0..12 {
                let mid = (lo + hi) * 0.5;
                if terrain.blocks(origin + dir * mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let p = origin + dir * hi;
            return Some((hi, terrain.normal(p.x, p.z)));
        }
        prev = d;
        d += 0.25;
    }
    None
}

/// The mesh-space bounds (centre, half extents) of a loaded glb.
fn glb_bounds(handle: &Handle<Gltf>, gltfs: &Assets<Gltf>, gltf_meshes: &Assets<GltfMesh>, meshes: &Assets<Mesh>) -> Option<(Vec3, Vec3)> {
    let gltf = gltfs.get(handle)?;
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    for gm in &gltf.meshes {
        for p in &gltf_meshes.get(gm)?.primitives {
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(ps)) = meshes.get(&p.mesh)?.attribute(Mesh::ATTRIBUTE_POSITION) else { continue };
            for v in ps {
                lo = lo.min(Vec3::from_array(*v));
                hi = hi.max(Vec3::from_array(*v));
            }
        }
    }
    (lo.x <= hi.x).then(|| ((lo + hi) * 0.5, (hi - lo) * 0.5))
}

/// The triangles of a loaded glb at `t`.
fn glb_tris(handle: &Handle<Gltf>, t: &Transform, gltfs: &Assets<Gltf>, gltf_meshes: &Assets<GltfMesh>, meshes: &Assets<Mesh>) -> Option<Vec<[Vec3; 3]>> {
    let gltf = gltfs.get(handle)?;
    let mut tris = Vec::new();
    for gm in &gltf.meshes {
        for p in &gltf_meshes.get(gm)?.primitives {
            tris.extend(crate::level::collision::tris_of(meshes.get(&p.mesh)?, t));
        }
    }
    Some(tris)
}

/// What the box overlaps: the landscape (its surface between the box's bottom and top at
/// points over the footprint) and the props.
fn overlap(terrain: Option<&Terrain>, rays: &mut PropRays, centre: Vec3, half: Vec3, rot: Quat) -> Overlap {
    if rays.overlaps_box(centre, half, rot) {
        return Overlap::Other;
    }
    let Some(terrain) = terrain else { return Overlap::None };
    const N: i32 = 4;
    for i in 0..=N {
        for j in 0..=N {
            let (u, v) = (i as f32 / N as f32 * 2.0 - 1.0, j as f32 / N as f32 * 2.0 - 1.0);
            let bottom = centre + rot * Vec3::new(u * half.x, -half.y, v * half.z);
            let top = centre + rot * Vec3::new(u * half.x, half.y, v * half.z);
            let h = terrain.height(bottom.x, bottom.z);
            if !terrain.hole(bottom.x, bottom.z) && h > bottom.y.min(top.y) && h < bottom.y.max(top.y) {
                return Overlap::Landscape;
            }
        }
    }
    Overlap::None
}

/// BuildCycle: where the build would go and whether it can.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build_cycle(
    mut commands: Commands,
    time: Res<Time>,
    mut build: ResMut<BuildMode>,
    db: Res<BuildableDb>,
    camera: Query<&GlobalTransform, With<super::PlayerCamera>>,
    terrain: Option<Res<Terrain>>,
    mut rays: PropRays,
    store: Res<SettingsStore>,
    level: Option<Res<crate::level::Level>>,
    placed: Res<Buildables>,
    asset_server: Res<AssetServer>,
    mut assets: ResMut<BuildAssets>,
    (gltfs, gltf_meshes, meshes): (Res<Assets<Gltf>>, Res<Assets<GltfMesh>>, Res<Assets<Mesh>>),
    mut notify: MessageWriter<Notify>,
    mut ghosts: Query<(&mut Transform, &mut Recolor), With<Ghost>>,
) {
    let Some(item) = build.item.clone() else { return };
    let (Some(bi), Ok(cam)) = (db.items.get(&item), camera.single()) else { return };
    let origin = cam.translation();
    let dir = cam.forward().as_vec3();
    let ground = terrain.as_deref().and_then(|t| terrain_ray(t, origin, dir, TRACE));
    let prop = rays.cast_hit(origin, dir, TRACE);
    let hit = match (ground, prop) {
        (Some(g), Some(p)) => Some(if g.0 <= p.0 { g } else { p }),
        (g, p) => g.or(p),
    };
    if let Some((_, n)) = hit {
        build.normal = n;
    }
    let at = origin + dir * hit.map_or(TRACE, |h| h.0) + build.add_location;
    build.transform = Transform::from_translation(at).with_rotation(build.rotation());
    // BP_NoBuildArea (only while BLOCK BUILDING AT MAJOR POIs? keeps them) and NoBuild-tagged
    // buildables along the view line.
    let end = origin + dir * hit.map_or(TRACE, |h| h.0);
    let no_build = !bi.allow_no_build && {
        let areas = super::combat::store_bool(&store, "General", "BLOCK BUILDING AT MAJOR POIs?");
        let half = Vec3::new(db.no_build[0], db.no_build[2], db.no_build[1]) / 100.0;
        let in_area = areas
            && level.as_ref().is_some_and(|l| {
                l.actors.iter().filter(|a| a.class == "BP_NoBuildArea_C").any(|a| {
                    let t = &a.transform;
                    let axes = [Vec3::new(t[0], t[1], t[2]), Vec3::new(t[3], t[4], t[5]), Vec3::new(t[6], t[7], t[8])];
                    let scale = Vec3::new(axes[0].length(), axes[1].length(), axes[2].length());
                    line_enters_box(origin, end + dir * 0.01, a.position(), axes.map(|v| v.normalize_or(Vec3::X)), Vec3::new(half.x * scale.x, half.y * scale.y, half.z * scale.z))
                })
            });
        let at_beacon = placed.list.iter().filter(|l| Some(l.id) != build.replacing).any(|l| {
            db.actors.get(&l.actor).is_some_and(|d| d.tags.iter().any(|t| t == "NoBuild")) && end.distance(l.transform.translation) < 1.5
        });
        in_area || at_beacon
    };
    // The ghost: the BuildMesh's bounds, once loaded.
    let Some(mesh) = bi.mesh.as_deref() else { return };
    let handle = assets.gltfs.entry(mesh.to_string()).or_insert_with(|| asset_server.load(glb(mesh))).clone();
    if build.ghost.is_none() {
        let g = commands.spawn((crate::models::model(&asset_server, glb(mesh)), build.transform, Ghost, Recolor(assets.red.clone()), DespawnOnExit(AppState::InGame))).id();
        build.ghost = Some(g);
    }
    let can = if no_build {
        let left = build.no_build_in.get_or_insert(NO_BUILD_TOAST);
        *left -= time.delta_secs();
        if *left <= 0.0 {
            notify.write(Notify::new("No build zone"));
            build.no_build_in = Some(NO_BUILD_TOAST);
        }
        false
    } else {
        build.no_build_in = None;
        match glb_bounds(&handle, &gltfs, &gltf_meshes, &meshes) {
            Some((centre, half)) => {
                let half = if build.snapping { half / bi.bounds_multiplier.max(0.01) } else { half };
                let rot = build.transform.rotation;
                let c = build.transform.translation + rot * (centre + bi.offset());
                can_build(hit.is_some(), build.snapping, overlap(terrain.as_deref(), &mut rays, c, half, rot), bi.sits_on_landscape())
            }
            None => false,
        }
    };
    build.can_build = can;
    if let Some(g) = build.ghost
        && let Ok((mut t, mut rc)) = ghosts.get_mut(g)
    {
        *t = build.transform;
        rc.0 = if can { assets.green.clone() } else { assets.red.clone() };
    }
}

/// IA_PrimaryAction while building: Svr_SpawnBuild.
#[allow(clippy::too_many_arguments)]
fn spawn_build(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    overlay: Res<GameOverlay>,
    menu: Res<State<MenuScreen>>,
    driving: Res<super::vehicles::Driving>,
    mut build: ResMut<BuildMode>,
    db: Res<BuildableDb>,
    mut placed: ResMut<Buildables>,
    mut game: ResMut<CurrentGame>,
    asset_server: Res<AssetServer>,
    assets: Res<BuildAssets>,
    mut sound: MessageWriter<crate::audio::Sound>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
    mut roots: Query<&mut Transform, (With<Placed>, Without<CharacterModel>)>,
) {
    if !build.on() || !mouse.just_pressed(MouseButton::Left) || overlay.is_open() || *menu.get() != MenuScreen::None || driving.active() || !build.can_build {
        return;
    }
    let Some(item) = build.item.clone() else { return };
    let t = build.transform;
    if let Some(id) = build.replacing {
        // ReplacingObject: K2_SetActorTransform, collision back on.
        if let Some(l) = placed.list.iter_mut().find(|l| l.id == id) {
            l.transform = t;
            l.solid = false;
            if let Ok(mut rt) = roots.get_mut(l.entity) {
                *rt = t;
            }
            info!("buildables: moved {} to {:.1}", l.item, t.translation);
        }
        stop_build(&mut commands, &mut build, &mut placed);
        save(&mut game, &placed);
        return;
    }
    if player_inv(&mut game).count(&item) == 0 {
        stop_build(&mut commands, &mut build, &mut placed);
        return;
    }
    if add_live(&mut commands, &asset_server, &assets, &db, &mut placed, &item, t).is_none() {
        return;
    }
    player_inv(&mut game).remove(&item, 1);
    info!("buildables: placed {} at {:.1}", item, t.translation);
    if let Ok(p) = player.single() {
        sound.write(crate::audio::Sound::at("S_Build_Cue", p.translation));
    }
    // FindJigItemByDA: more of it carries on, else Event_StopBuildMode.
    if player_inv(&mut game).count(&item) == 0 {
        stop_build(&mut commands, &mut build, &mut placed);
    }
    save(&mut game, &placed);
}

/// Placed buildables' collision once their meshes are in (off while one is being moved).
#[allow(clippy::too_many_arguments)]
fn make_solid(
    mut placed: ResMut<Buildables>,
    build: Res<BuildMode>,
    db: Res<BuildableDb>,
    asset_server: Res<AssetServer>,
    mut assets: ResMut<BuildAssets>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
    mut rays: PropRays,
) {
    for l in placed.list.iter_mut() {
        if Some(l.id) == build.replacing {
            if l.solid {
                rays.set_dynamic(COLLIDE_KEY + l.id, Vec::new());
                l.solid = false;
            }
            continue;
        }
        if l.solid {
            continue;
        }
        let Some(def) = db.actors.get(&l.actor) else { continue };
        let mut tris = Vec::new();
        let mut ready = true;
        for (i, p) in def.parts.iter().enumerate().filter(|(_, p)| p.collides) {
            let handle = assets.gltfs.entry(p.mesh.clone()).or_insert_with(|| asset_server.load(glb(&p.mesh))).clone();
            match glb_tris(&handle, &(l.transform * def.part_transform(i)), &gltfs, &gltf_meshes, &meshes) {
                Some(t) => tris.extend(t),
                // A mesh that failed to load is left out.
                None if asset_server.load_state(&handle).is_failed() => {}
                None => ready = false,
            }
        }
        if ready {
            rays.set_dynamic(COLLIDE_KEY + l.id, tris);
            l.solid = true;
        }
    }
}

/// Apply `Recolor` materials to the meshes under them.
fn recolor(roots: Query<(Entity, &Recolor)>, children: Query<&Children>, mut mats: Query<&mut MeshMaterial3d<StandardMaterial>>) {
    for (root, rc) in &roots {
        for e in children.iter_descendants(root) {
            if let Ok(mut m) = mats.get_mut(e)
                && m.0 != rc.0
            {
                m.0 = rc.0.clone();
            }
        }
    }
}

/// M_PulsingLight's Frequency 0.75.
fn pulse(time: Res<Time>, assets: Res<BuildAssets>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let a = 0.5 + 0.5 * (time.elapsed_secs() * 0.75 * std::f32::consts::TAU).sin();
    if let Some(mut m) = materials.get_mut(&assets.pulse) {
        m.base_color = Color::srgba(1.0, 0.05, 0.02, 0.15 + 0.45 * a);
    }
}

/// BP_Ingame's BuildBox and BuildingDegrees.
#[allow(clippy::too_many_arguments)]
fn build_hud(
    mut commands: Commands,
    build: Res<BuildMode>,
    font: Res<MenuFont>,
    boxes: Query<Entity, With<BuildBox>>,
    mut snap: Query<&mut Text, (With<SnapText>, Without<DegreesText>)>,
    mut degrees: Query<&mut Text, (With<DegreesText>, Without<SnapText>)>,
    mut was_snapping: Local<Option<bool>>,
) {
    if !build.on() {
        for e in &boxes {
            commands.entity(e).despawn();
        }
        *was_snapping = None;
        return;
    }
    if boxes.is_empty() {
        let lines = [
            "[LMB] Place Build",
            "[RMB] Stop Build",
            "[Mouse Wheel Axis] Rotate Left/Right",
            "[Shift + Mouse Wheel Axis] Offset Forwards/Backwards",
            "[R] Snap Build To Ground",
            "[Shift + R] Reset Snap Build To Ground",
            "[Shift + Q] Rotate -90°",
            "[Shift + E] Rotate +90°",
            "[Shift + F] Disable Snapping / More Freedom",
        ];
        // Anchored at the left edge's middle, Alignment (0, 0.5), auto-sized.
        let wrap = commands
            .spawn((
                Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), bottom: Val::Px(0.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, ..default() },
                Pickable::IGNORE,
                BuildBox,
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let panel = commands.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)))).id();
        commands.entity(wrap).add_child(panel);
        for (i, l) in lines.iter().enumerate() {
            let mut e = commands.spawn((Node { margin: UiRect::all(Val::Px(5.0)), ..default() }, label_colored(&font, *l, 14.0, Color::WHITE)));
            if i == lines.len() - 1 {
                e.insert(SnapText);
            }
            let e = e.id();
            commands.entity(panel).add_child(e);
        }
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Percent(50.0), margin: UiRect { left: Val::Px(30.0), top: Val::Px(30.0), ..default() }, ..default() },
            label_colored(&font, degrees_text(build.rotation_z), 12.0, Color::WHITE),
            DegreesText,
            BuildBox,
            DespawnOnExit(AppState::InGame),
        ));
        *was_snapping = Some(build.snapping);
        return;
    }
    // ChangeSnappingModeText (only once it's been toggled).
    if *was_snapping != Some(build.snapping) {
        *was_snapping = Some(build.snapping);
        if let Ok(mut t) = snap.single_mut() {
            t.0 = if build.snapping { "[Shift + F] Disable Snapping".into() } else { "[Shift + F] Enable Snapping".into() };
        }
    }
    if let Ok(mut t) = degrees.single_mut() {
        let s = degrees_text(build.rotation_z);
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// UpdateBuildingDegree: Conv_DoubleToText with up to 3 fraction digits.
pub fn degrees_text(d: f32) -> String {
    let s = format!("{d:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// F on a placed buildable in reach: its options (the wheel picks).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn interact(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    overlay: Res<GameOverlay>,
    item_db: Res<super::ItemDb>,
    db: Res<BuildableDb>,
    mut build: ResMut<BuildMode>,
    mut placed: ResMut<Buildables>,
    mut game: ResMut<CurrentGame>,
    mut ground: ResMut<super::inventory_ui::Ground>,
    mut notify: MessageWriter<Notify>,
    mut rays: PropRays,
    driving: Res<super::vehicles::Driving>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
    (mut prompt, mut text, mut bar): (
        Query<&mut Visibility, With<super::interact::Prompt>>,
        Query<&mut Text, With<super::interact::PromptText>>,
        Query<&mut Node, With<super::interact::PromptBar>>,
    ),
) {
    let (Ok(p), Ok(mut vis)) = (player.single(), prompt.single_mut()) else { return };
    if overlay.is_open() || *vis != Visibility::Hidden || build.on() || driving.active() {
        return;
    }
    let p = p.translation;
    let near = placed
        .list
        .iter()
        .enumerate()
        .filter(|(_, l)| l.transform.translation.with_y(p.y).distance(p) <= REACH && (l.transform.translation.y - p.y).abs() < 2.5)
        .min_by(|a, b| a.1.transform.translation.distance(p).total_cmp(&b.1.transform.translation.distance(p)))
        .map(|(i, _)| i);
    let Some(i) = near else { return };
    let l = &placed.list[i];
    let Some(def) = db.actors.get(&l.actor) else { return };
    // The options ported: the beacon's Interact, Buildable_MASTER's Move and Pickup.
    let options: Vec<&str> = def.options.iter().map(String::as_str).filter(|o| matches!(*o, "Move" | "Pickup") || (*o == "Interact" && l.horde.is_some())).collect();
    if options.is_empty() {
        return;
    }
    if scroll.delta.y != 0.0 {
        placed.option = (placed.option + 1) % options.len();
    }
    let pick = options[placed.option.min(options.len() - 1)];
    *vis = Visibility::Inherited;
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(0.0);
    }
    let name = def.pickup.as_ref().and_then(|d| item_db.items.get(d)).map_or("Buildable", |d| d.name.as_str()).to_string();
    if let Ok(mut t) = text.single_mut() {
        let more = if options.len() > 1 { "  [Wheel] change" } else { "" };
        t.0 = format!("{name}: [F] {pick}{more}");
    }
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let mut out = Vec::new();
    match pick {
        "Interact" => {
            // OnInteractBuildable: TraceForBuildables first.
            let me = placed.list[i].transform.translation;
            let blocked = placed.list.iter().enumerate().any(|(k, o)| k != i && o.transform.translation.distance(me) <= BEACON_CLEARANCE);
            if blocked {
                notify.write(Notify::new("Buildable blocking beacon signal"));
            } else if let Some(h) = placed.list[i].horde.as_mut() {
                h.interact(&mut out);
                info!("horde beacon: interact (started {}, can stop {})", h.started, h.can_stop);
            }
        }
        "Move" => {
            if placed.list[i].horde.as_ref().is_none_or(|h| !h.started) {
                // ReplacingObject: build mode with this actor, RotationZ = its yaw.
                let (yaw, _, _) = placed.list[i].transform.rotation.to_euler(EulerRot::YXZ);
                build.rotation_z = change_rotation(-yaw.to_degrees(), true, 0.0);
                let item = placed.list[i].item.clone();
                let id = placed.list[i].id;
                start_build(&mut build, item, Some(id));
            }
        }
        _ => {
            // Pickup: Svr_Spawn (JigTryAddItemSomewhere, then K2_DestroyActor).
            let ok = match placed.list[i].horde.as_mut() {
                Some(h) => h.pickup(&mut out),
                None => true,
            };
            if ok {
                let l = placed.list.remove(i);
                commands.entity(l.entity).despawn();
                rays.set_dynamic(COLLIDE_KEY + l.id, Vec::new());
                let item = def.pickup.clone().unwrap_or(l.item.clone());
                let stack = item_db.stack(&item, 1);
                // Ours: what doesn't fit lands at the survivor's feet (the game drops it).
                if let Some(rest) = player_inv(&mut game).insert(&item_db, stack) {
                    ground.drop_at(p, rest);
                }
                info!("buildables: picked up {}", l.item);
                save(&mut game, &placed);
            }
        }
    }
    toasts(out, &mut notify);
}

/// The horde's messages (the dish follows `Horde::started`: Timeline.Stop leaves it where it is).
fn toasts(out: Vec<HordeOut>, notify: &mut MessageWriter<Notify>) {
    for o in out {
        if let HordeOut::Toast(t) = o {
            info!("horde beacon: {}", t.replace("\r\n", " "));
            notify.write(Notify::new(t.replace("\r\n", "\n")).colored(RED));
        }
    }
}

/// The beacons' waves and dishes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run_hordes(
    mut commands: Commands,
    time: Res<Time>,
    db: Res<BuildableDb>,
    item_db: Res<super::ItemDb>,
    mut placed: ResMut<Buildables>,
    mut game: ResMut<CurrentGame>,
    mut ground: ResMut<super::inventory_ui::Ground>,
    mut notify: MessageWriter<Notify>,
    mut sound: MessageWriter<crate::audio::Sound>,
    mut fx: MessageWriter<super::particles::Fx>,
    terrain: Option<Res<Terrain>>,
    mut rays: PropRays,
    asset_server: Res<AssetServer>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
    zombies: Query<Has<Dead>, With<LevelZombie>>,
    mut parts: Query<&mut Transform, (Without<CharacterModel>, Without<LevelZombie>)>,
) {
    let dt = time.delta_secs();
    let me = player.single().ok().map(|t| t.translation);
    let mut rng = rand::thread_rng();
    let all_dead = |list: &[Entity]| list.iter().all(|&e| zombies.get(e).unwrap_or(true));
    let mut finished = Vec::new();
    for i in 0..placed.list.len() {
        // The dish: its Rotate Alpha timeline while the horde runs, RInterpTo at 3.
        let running = placed.list[i].horde.as_ref().is_some_and(|h| h.started);
        if let Some((dish, yaw, t)) = placed.list[i].dish.as_mut() {
            if running {
                *t = (*t + dt) % db.horde.dish_secs.max(0.1);
                let target = 45.0 + (-45.0 - 45.0) * db.horde.dish_alpha(*t);
                *yaw += (target - *yaw) * (dt * 3.0).clamp(0.0, 1.0);
            }
            if let Ok(mut pt) = parts.get_mut(*dish) {
                pt.rotation = Quat::from_rotation_y(-yaw.to_radians());
            }
        }
        let Some(h) = placed.list[i].horde.as_mut() else { continue };
        let out = h.tick(dt, &db.horde, all_dead, &mut rng);
        let at = placed.list[i].transform.translation;
        for o in out {
            match o {
                HordeOut::Spawn(class) => {
                    let Some(kind) = super::zombies::ZombieKind::from_class(&class) else {
                        warn!("horde beacon: no zombie class {class}");
                        continue;
                    };
                    let Some(spot) = find_location(at, terrain.as_deref(), &mut rays, &mut rng) else { continue };
                    let (model, material) = kind.pick_look(&mut rng);
                    let lz = LevelZombie { home: spot, spawner: None, model, material, kind };
                    let e = crate::level::spawners::spawn_zombie(&mut commands, &asset_server, Transform::from_translation(spot).with_rotation(Quat::from_rotation_y(rng.gen_range(0.0..std::f32::consts::TAU))), lz);
                    if let Some(me) = me {
                        commands.entity(e).insert(LockOnTarget(me));
                    }
                    if let Some(h) = placed.list[i].horde.as_mut() {
                        h.enemies.push(e);
                        info!("horde beacon: spawned {class} ({} so far)", h.enemies.len());
                    }
                }
                HordeOut::Finish => finished.push(i),
                other => toasts(vec![other], &mut notify),
            }
        }
    }
    // SpawnRewards, the spark and sound, K2_DestroyActor.
    for &i in finished.iter().rev() {
        let l = placed.list.remove(i);
        rays.set_dynamic(COLLIDE_KEY + l.id, Vec::new());
        commands.entity(l.entity).despawn();
        let base = l.transform.transform_point(Vec3::from_array(db.horde.reward_at));
        fx.write(super::particles::Fx::at("FX_Spark_Impact", l.transform.translation));
        sound.write(crate::audio::Sound::at("metal_med_impact_01_Cue", l.transform.translation));
        for (item, count, spread) in roll_rewards(&db.horde, &mut rng) {
            let p = base + Vec3::new(spread.x, 0.0, spread.y);
            // They drop from RewardSpawnLoc; ours lie where they'd land.
            let floor = floor_under(terrain.as_deref(), &mut rays, p).unwrap_or(l.transform.translation.y);
            ground.drop_at(p.with_y(floor + 0.05), item_db.spawn(&item, count, &mut rng));
            info!("horde beacon: reward {item} x{count}");
        }
        save(&mut game, &placed);
    }
}

/// The first floor (terrain or prop) under `p`.
fn floor_under(terrain: Option<&Terrain>, rays: &mut PropRays, p: Vec3) -> Option<f32> {
    let prop = rays.cast(p, Vec3::NEG_Y, PROJECT_DOWN).map(|d| p.y - d);
    let land = terrain.and_then(|t| t.ground_under(p));
    match (prop, land) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

/// FindLocation + ProjectLocation (no navmesh here: any point in the disc).
fn find_location(at: Vec3, terrain: Option<&Terrain>, rays: &mut PropRays, rng: &mut impl Rng) -> Option<Vec3> {
    let a = rng.gen_range(0.0..std::f32::consts::TAU);
    let r = SPAWN_RADIUS * rng.gen_range(0.0f32..=1.0).sqrt();
    let yaw = rng.gen_range(0.0..std::f32::consts::TAU);
    let top = at + Vec3::new(a.cos() * r, 0.0, a.sin() * r) + Vec3::new(yaw.cos() * SPAWN_OUT, SPAWN_UP, yaw.sin() * SPAWN_OUT);
    floor_under(terrain, rays, top).map(|y| top.with_y(y))
}

/// SetBB_LockOnTarget: walk to a point near the target, then back to their own business.
fn lock_on(mut commands: Commands, mut zombies: Query<(Entity, &LockOnTarget, &mut super::zombies::Zombie)>) {
    let mut rng = rand::thread_rng();
    for (e, target, mut z) in &mut zombies {
        let a = rng.gen_range(0.0..std::f32::consts::TAU);
        let r = rng.gen_range(0.0..LOCK_ON_SPREAD);
        z.brain = super::zombies::Brain::Roam { goal: Some(target.0 + Vec3::new(a.cos() * r, 0.0, a.sin() * r)), wait: 0.0 };
        commands.entity(e).remove::<LockOnTarget>();
    }
}

/// Event_PlayerDead -> Event_SpawnBeacon: each beacon back as its pickup at RewardSpawnLoc.
#[allow(clippy::too_many_arguments)]
fn beacons_on_death(
    mut commands: Commands,
    vitals: Res<super::PlayerVitals>,
    db: Res<BuildableDb>,
    item_db: Res<super::ItemDb>,
    mut placed: ResMut<Buildables>,
    mut game: ResMut<CurrentGame>,
    mut ground: ResMut<super::inventory_ui::Ground>,
    terrain: Option<Res<Terrain>>,
    mut rays: PropRays,
    mut was_dead: Local<bool>,
) {
    let dead = vitals.is_dead();
    if dead == *was_dead {
        return;
    }
    *was_dead = dead;
    if !dead {
        return;
    }
    let mut i = 0;
    let mut changed = false;
    while i < placed.list.len() {
        if placed.list[i].horde.is_none() {
            i += 1;
            continue;
        }
        let l = placed.list.remove(i);
        rays.set_dynamic(COLLIDE_KEY + l.id, Vec::new());
        commands.entity(l.entity).despawn();
        let p = l.transform.transform_point(Vec3::from_array(db.horde.reward_at));
        let floor = floor_under(terrain.as_deref(), &mut rays, p).unwrap_or(l.transform.translation.y);
        ground.drop_at(p.with_y(floor + 0.05), item_db.stack(&l.item, 1));
        info!("horde beacon: back to a pickup (the survivor died)");
        changed = true;
    }
    if changed {
        save(&mut game, &placed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn db() -> BuildableDb {
        BuildableDb::load()
    }

    #[test]
    fn green_and_red() {
        // Snapping (the default): on something, any overlap but the landscape blocks.
        assert!(can_build(true, true, Overlap::None, false));
        assert!(!can_build(true, true, Overlap::Other, false));
        assert!(!can_build(true, true, Overlap::Landscape, false));
        assert!(can_build(true, true, Overlap::Landscape, true));
        // In the air it needs something to rest in.
        assert!(!can_build(false, true, Overlap::None, false));
        assert!(!can_build(false, true, Overlap::Other, false));
        assert!(can_build(false, true, Overlap::Landscape, true));
        // Snapping off: anything on something; in the air only overlapping.
        assert!(can_build(true, false, Overlap::Other, false));
        assert!(can_build(false, false, Overlap::Other, false));
        assert!(!can_build(false, false, Overlap::None, false));
    }

    #[test]
    fn rotation_wraps_once() {
        assert_eq!(change_rotation(0.0, false, 2.0), 358.0);
        assert_eq!(change_rotation(358.0, true, 2.0), 360.0);
        assert_eq!(change_rotation(359.0, true, 90.0), 89.0);
        assert_eq!(degrees_text(358.0), "358");
        assert_eq!(degrees_text(12.5), "12.5");
    }

    #[test]
    fn no_build_boxes_stop_lines_entering_them() {
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        let half = Vec3::splat(20.0);
        assert!(line_enters_box(Vec3::new(-25.0, 0.0, 0.0), Vec3::new(-15.0, 0.0, 0.0), Vec3::ZERO, axes, half));
        assert!(!line_enters_box(Vec3::new(-30.0, 0.0, 0.0), Vec3::new(-25.0, 0.0, 0.0), Vec3::ZERO, axes, half));
        // From inside, a line trace doesn't see it.
        assert!(!line_enters_box(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO, axes, half));
    }

    #[test]
    fn beacon_data() {
        let db = db();
        let Some(item) = db.items.get("DA_HordeBeacon") else { return };
        assert_eq!(item.actor, BEACON);
        assert_eq!(item.bounds_multiplier, 1.5);
        let actor = &db.actors[BEACON];
        assert_eq!(actor.options, ["Interact", "Move", "Pickup"]);
        // The light ray hangs off the dish, 1.15 m up it.
        let cube = actor.parts.iter().position(|p| p.name == "Cube").unwrap();
        assert_eq!(actor.parts[cube].parent.as_deref(), Some("Dish"));
        assert!((actor.part_transform(cube).translation.y - (0.77175 + 1.148773 * 0.9)).abs() < 0.01);
        let waves: Vec<u32> = db.horde.waves.iter().map(|w| w.groups.iter().map(|g| g.1).sum()).collect();
        assert_eq!(waves, [4, 6, 1, 8, 12, 2, 15, 1]);
        assert!(db.horde.waves.last().unwrap().last);
        assert_eq!(db.horde.max_rewards, 8);
        assert_eq!(db.horde.dish_alpha(3.0), 0.5);
        assert!(db.items.values().all(|i| db.actors.contains_key(&i.actor)));
    }

    /// Run a horde with every spawn dying at once; returns the toasts and the spawns.
    fn run(def: &HordeDef, until: f32, stop_at: Option<f32>) -> (Vec<String>, Vec<String>, bool) {
        let mut h = Horde::default();
        let mut out = Vec::new();
        h.interact(&mut out);
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let (mut toasts, mut spawns, mut finished) = (Vec::new(), Vec::new(), false);
        let mut t = 0.0;
        let mut stopped = false;
        while t < until {
            if let Some(s) = stop_at
                && t >= s
                && !stopped
            {
                stopped = true;
                h.interact(&mut out);
            }
            out.extend(h.tick(0.05, def, |_| true, &mut rng));
            for o in out.drain(..) {
                match o {
                    HordeOut::Toast(s) => toasts.push(s),
                    HordeOut::Spawn(c) => spawns.push(c),
                    HordeOut::Finish => finished = true,
                    HordeOut::Dish(_) => {}
                }
            }
            t += 0.05;
        }
        (toasts, spawns, finished)
    }

    #[test]
    fn a_horde_runs_its_waves_to_the_end() {
        let db = db();
        if db.horde.waves.is_empty() {
            return;
        }
        let (toasts, spawns, finished) = run(&db.horde, 600.0, None);
        assert_eq!(toasts[0], "Horde Beacon Activated...");
        assert_eq!(toasts[1], "Horde Beginning in 30 Seconds...");
        assert_eq!(toasts[5], "Wave: 1 Begin!");
        assert!(toasts.iter().any(|t| t == "Wave: 8 Begin!"));
        assert_eq!(toasts.last().unwrap(), "Final wave defeated!");
        assert_eq!(spawns.len(), 49);
        assert_eq!(spawns.last().unwrap(), "BP_ZombieBossHorde_Military_C");
        assert!(finished);
    }

    #[test]
    fn the_countdown_can_be_stopped_the_waves_cannot() {
        let db = db();
        if db.horde.waves.is_empty() {
            return;
        }
        let (toasts, spawns, _) = run(&db.horde, 60.0, Some(12.0));
        assert_eq!(toasts.last().unwrap(), "Stopping Horde Beacon");
        assert!(spawns.is_empty());
        let (toasts, spawns, _) = run(&db.horde, 45.0, Some(40.0));
        assert!(!toasts.iter().any(|t| t == "Stopping Horde Beacon"));
        assert!(!spawns.is_empty());
        let mut h = Horde { started: true, can_stop: false, ..default() };
        let mut out = Vec::new();
        assert!(!h.pickup(&mut out));
        assert_eq!(out, [HordeOut::Toast("Horde Already Started!".into())]);
    }

    #[test]
    fn rewards_stop_at_the_maximum() {
        let def = HordeDef { rewards: (0..20).map(|i| Reward { item: format!("DA_{i}"), chance: 100.0, min: 1, max: 1 }).collect(), max_rewards: 8, ..default() };
        let mut rng = rand::rngs::StdRng::seed_from_u64(2);
        let r = roll_rewards(&def, &mut rng);
        assert_eq!(r.len(), 8);
        assert_eq!((r[0].2, r[1].2), (Vec2::ZERO, Vec2::ZERO));
        assert_ne!(r[5].2, Vec2::ZERO);
    }
}
