//! Zombie behaviour, from `BT_Zombie` / `BB_Zombie` and `BP_ZombieAIController`'s perception:
//!
//! - **Chase** while it can see the survivor (`CanSeePlayer?`): alert speed (`Alert Speed` 425),
//!   attack in melee range (`Zombie_Attack`, swings timed in `survival.rs`).
//! - **Investigate** the last heard or seen location at alert speed (`MovementType` 1), then
//!   wait there 30 ± 5 s (`BTTask_Wait_4`) and calm down (`IsAlert?` cleared).
//! - **Roam** otherwise: a random point within 10 m (`AI_FindRandomLocation` Radius 1000), walk
//!   there at `Roaming Speed` 100 (0.7 m acceptance), idle 30 ± 15 s (`BTTask_Wait_3`).
//!
//! Sight (`AISenseConfig_Sight`): 25 m, kept up to 40 m once seen, 70° either side of facing,
//! blocked by terrain. Hearing (`AISenseConfig_Hearing`): 150 m for a loud noise such as a
//! gunshot ([`Noise`]). Being hurt makes a zombie chase whoever hurt it. Speeds scale with
//! ZOMBIE SPEED MULTIPLIER; turning is `RotationRate` 275°/s.
//!
//! The bosses (`BT_ZombieBoss`, the same tree) use this brain with their own speeds, 110° sight
//! cone and waits; they chase to contact and swing on the way (`bosses.rs`).

use bevy::prelude::*;
use rand::Rng;
use serde::Deserialize;

use super::combat::{Dead, Health};
use super::survival::difficulty;
use crate::level::{KinematicBody, LevelZombie, Terrain};
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, MenuScreen};
use crate::models::{AnimSet, Animated, CharacterModel, Locomotion, UpperAction};

const SIGHT: f32 = 25.0;
const LOSE_SIGHT: f32 = 40.0;
const PERIPHERAL: f32 = 70.0;
pub const HEARING: f32 = 150.0;
const ROAM_RADIUS: f32 = 10.0;
const ROAM_SPEED: f32 = 1.0;
const ALERT_SPEED: f32 = 4.25;
const TURN_RATE: f32 = 275.0;
/// Close enough to swing (`Zombie_EnemyInRange`; the range itself isn't readable, a guess).
pub const MELEE_RANGE: f32 = 1.5;
const ARRIVE: f32 = 0.7;
/// A chasing boss stops this far beyond its own capsule radius: the survivor's (MoveToActor's
/// bStopOnOverlap).
const BOSS_STOP: f32 = 0.35;
const EYE: f32 = 1.6;

/// Something loud happened (`ReportNoiseEvent`): zombies within `range` go to look.
#[derive(Message, Clone, Copy)]
pub struct Noise {
    pub at: Vec3,
    pub range: f32,
}

/// Damage_Shoved: Zombie_Pushed_Montage (2.33 s) plays, movement stops and `CanAttack?` is off
/// until it completes. Seconds left.
#[derive(Component, Clone, Copy, Debug)]
pub struct Shoved(pub f32);

/// Standing still for someone (the wandering trader's DisableMovement while it talks or trades).
#[derive(Component, Clone, Copy, Debug)]
pub struct HeldStill;

/// Zombie_Pushed_Montage's length.
pub const SHOVED_SECS: f32 = 2.3333333;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Brain {
    Roam { goal: Option<Vec3>, wait: f32 },
    Investigate { at: Vec3, wait: Option<f32> },
    Chase,
}

#[derive(Component)]
pub struct Zombie {
    pub brain: Brain,
    /// Where it last saw the survivor (investigated after losing sight).
    last_seen: Option<Vec3>,
    kind: ZombieKind,
    /// The survivor is in view this frame (a ranged human holds and shoots).
    pub in_sight: bool,
}

/// A `BP_MasterZombie` child's class defaults, from `assets/data/zombies.json`
/// (`extractor/gen_ai.py`). Speeds m/s, lengths m, chances 0-1.
#[derive(Deserialize, Clone, Debug)]
pub struct ZombieClass {
    pub name: Option<String>,
    /// `Meshes` / `Materials`: one of each is picked at random (`SetMeshAndMaterial`).
    pub meshes: Vec<String>,
    pub materials: Vec<String>,
    /// `DamageToDo` per hit.
    pub damage: f32,
    pub roam_speed: f32,
    pub alert_speed: f32,
    /// DamageComponent `StandardHealthValue` and the class's `HealthDeviation`.
    pub health: f32,
    pub health_deviation: f32,
    /// `TimeBetweenAttacksMin/Max`.
    pub attack_interval: [f32; 2],
    pub bleed: f32,
    pub heavy_bleed: f32,
    pub broken_bone: f32,
    /// `Chance for Rad Damage` and `Radiation Damage` per hit.
    pub rad_chance: f32,
    pub rad_damage: f32,
    /// `XPMultiplier_Min/Max`.
    pub xp: [f32; 2],
    pub loot_table: Option<String>,
    /// It has a `ShovedAnimation` (crawlers and hounds don't, and shrug off shoves).
    #[serde(default)]
    pub shovable: bool,
    /// BP_JigMultiplayer's container name ("Dead Zombie", "Dead Zombie Hound").
    #[serde(default)]
    pub container: Option<String>,
    pub capsule_half_height: f32,
    pub capsule_radius: f32,
    /// `SD_AIComponent.MovementBlendspace` samples: (clip, speed). Clip paths are relative to
    /// Content/Animations once loaded ([`zombie_classes`] rewrites them).
    pub locomotion: Vec<(String, f32)>,
    /// A `BP_Animal` child's own traits (`gameplay::animals`); None for zombies.
    #[serde(default)]
    pub animal: Option<AnimalTraits>,
    /// A `BP_ZombieBoss` child's own traits (`gameplay::bosses`).
    #[serde(default)]
    pub boss: Option<BossTraits>,
    /// A human AI class's own traits (`gameplay::humans`).
    #[serde(default)]
    pub human: Option<HumanTraits>,
}

/// What a human AI class (SmartAI's BP_MasterAICharacter through BP_MasterBandit / Military /
/// Scavenger) adds. Lengths m, speeds m/s.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct HumanTraits {
    /// Bandit, Military or Scavenger (the friendly scavengers only fight back).
    pub faction: String,
    /// The kill toast's name: "Bandit Killed, XP Given: {XP}".
    pub killed: String,
    /// `Boss?`: the kill's 25 % skill point.
    pub boss: bool,
    /// AI_Health's difficulty row (Difficulty.BanditHealth...).
    pub health_row: Option<String>,
    pub hair: Vec<String>,
    pub beard: Vec<String>,
    /// `RangedWeapon` / `MeleeWeapon`: one of each is picked (SetWeapons), keys of [`ai_weapons`].
    pub ranged: Vec<String>,
    pub melee: Vec<String>,
    pub start_ranged: bool,
    /// The SmartAI component's Accuracy: a trace shot's cone is Accuracy x Spread (Trace).
    pub accuracy: f32,
    pub melee_distance: f32,
    /// E_AIBehaviour Melee Attack's movement speed.
    pub melee_speed: f32,
    pub sight: f32,
    pub lose_sight: f32,
    pub peripheral: f32,
    pub hearing: f32,
    pub roam: [f32; 2],
    pub roam_wait: [f32; 2],
    pub corpse_secs: f32,
    pub death: Vec<String>,
    /// BP_MasterTrader: the wandering trader (`traders.rs`).
    #[serde(default)]
    pub trader: bool,
    #[serde(default)]
    pub backpack: Option<Backpack>,
}

/// BackpackMeshes' pick: a static mesh on the body's Backpack socket, its transform in glb space.
#[derive(Deserialize, Clone, Debug)]
pub struct Backpack {
    pub mesh: String,
    pub t: [f32; 3],
    pub r: [f32; 4],
}

/// A BP_AIWeapon_* class's defaults (what SmartAI's Range Attack and melee read).
#[derive(Deserialize, Clone, Debug, Default)]
pub struct AiWeapon {
    pub ranged: bool,
    /// Damage by E_HitboxTypeAI: Head, Chest, Arms, Legs, Hands, Feet, Other, None.
    pub damage: [f32; 8],
    pub range: f32,
    /// Spread (Trace), degrees.
    pub spread: f32,
    /// Range Attack Frequency Min / Max (s between shots).
    pub interval: [f32; 2],
    pub magazine: u32,
    pub sound: Option<String>,
    pub muzzle_flash: Option<String>,
    pub reload: Option<String>,
    pub melee: Vec<String>,
    pub melee_interval: [f32; 2],
    pub mesh: Option<String>,
    /// RangeStartPoint in the weapon's space (UE cm).
    pub muzzle: [f32; 3],
    pub socket: Option<String>,
}

/// The human AI's weapons, by class.
pub fn ai_weapons() -> &'static std::collections::BTreeMap<String, AiWeapon> {
    static W: std::sync::OnceLock<std::collections::BTreeMap<String, AiWeapon>> = std::sync::OnceLock::new();
    W.get_or_init(|| {
        std::fs::read_to_string("assets/data/zombies.json")
            .ok()
            .and_then(|t| serde_json::from_str::<ZombieData>(&t).ok())
            .map(|d| d.weapons)
            .unwrap_or_default()
    })
}

/// What a `BP_ZombieBoss` child adds (its own Blueprint, not BP_MasterZombie's). Lengths m.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct BossTraits {
    /// The AttackMontage's sequence (relative to Content/Animations once loaded), on the
    /// UpperBody slot at `rate` (its RateScale).
    pub attack: Option<String>,
    pub rate: f32,
    /// AnimNotify_ZombieBossAttackRightArm: RightArmTrace runs in this window (montage seconds).
    pub hit_window: [f32; 2],
    /// SD_AIComponent.MeleeAttackDistance (actor centres): InRangeForMelee? starts the swings.
    pub reach: f32,
    /// RightArmTrace's sphere at Hand_R.
    pub hand_radius: f32,
    /// The death's "Add Skill Points" chance (RandomFloatInRange(0, 100) <= 25).
    pub skill_point_chance: f32,
    pub corpse_secs: f32,
    /// AddUniqueLoot's `Unqiue Items Set`: DT_UniqueLoot sets added to the corpse's loot.
    pub unique_sets: Vec<String>,
    /// The kill toast's name: "Zombie Boss Killed, XP Given: {XP}".
    pub killed: String,
    /// CharacterMesh0's RelativeScale3D (Bevy axes).
    pub scale: [f32; 3],
    /// The AI controller's sight PeripheralVisionAngleDegrees (the rest is BP_ZombieAIController's).
    pub peripheral: f32,
    pub voice: Option<String>,
    /// BT_ZombieBoss's waits: after investigating, then before roaming ([WaitTime, RandomDeviation]).
    pub waits: Vec<[f32; 2]>,
}

/// What a `BP_Animal` child adds: its AI controller's senses, its behaviour tree's speeds,
/// waits and radii, and its montages. Lengths m, speeds m/s, waits `[WaitTime, RandomDeviation]`.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct AnimalTraits {
    /// Runs from what it sees (deer, rabbits: Run Away); else hunts it (wolves, bears).
    pub flees: bool,
    /// The kill toast's name: "Deer Killed, XP Given: {XP}".
    pub killed: String,
    pub sight: f32,
    pub lose_sight: f32,
    /// AISenseConfig_Sight's PeripheralVisionAngleDegrees: either side of the facing.
    pub peripheral: f32,
    pub hearing: f32,
    /// The hearing stimulus' MaxAge (s).
    pub hearing_age: f32,
    /// The Investigate Noise branch's speed (hunters only).
    pub investigate_speed: Option<f32>,
    pub roam_radius: f32,
    /// The roam sequence's waits in order (hunters: one before choosing a spot, one after).
    pub roam_waits: Vec<[f32; 2]>,
    pub alert_waits: Vec<[f32; 2]>,
    pub investigate_waits: Vec<[f32; 2]>,
    /// Run Away: a point within `flee_radius` of one `away` m back from the threat.
    pub away: f32,
    pub flee_radius: Option<[f32; 2]>,
    /// BTTask_MoveTo AcceptableRadius.
    pub arrive: f32,
    /// AttackMontage / HitAnimation / death montage sequences (relative to Content/Animations
    /// once loaded).
    pub attacks: Vec<String>,
    pub hits: Vec<String>,
    pub death: Option<String>,
    /// The Delay before a corpse is destroyed.
    pub corpse_secs: f32,
    /// With Skill_Huntsman a Container_AnimalMeat ("Animal Meat") spawns at the kill.
    pub huntsman_meat: bool,
    pub voice: Option<String>,
    /// Played when it first sees its prey (WolfSnarl_Cue, BearSnarl_cue).
    pub snarl: Option<String>,
    /// The first mesh's bind-pose bounds (glb space: +Z forward, +Y up), for hit tests.
    pub bounds: Option<[[f32; 3]; 2]>,
    /// CharacterMesh0's RelativeScale3D.
    pub scale: f32,
    /// How far the mesh origin (the entity) sits behind the capsule's centre.
    pub mesh_back: f32,
    /// MeleeAttackDistance (capsule centres).
    pub reach: f32,
}

#[derive(Deserialize)]
struct ZombieData {
    classes: std::collections::BTreeMap<String, ZombieClass>,
    #[serde(default)]
    weapons: std::collections::BTreeMap<String, AiWeapon>,
}

/// Every zombie class, by Blueprint class name (sorted, so indices are stable).
pub fn zombie_classes() -> &'static [(String, ZombieClass)] {
    static CLASSES: std::sync::OnceLock<Vec<(String, ZombieClass)>> = std::sync::OnceLock::new();
    CLASSES.get_or_init(|| {
        std::fs::read_to_string("assets/data/zombies.json")
            .ok()
            .and_then(|t| serde_json::from_str::<ZombieData>(&t).ok())
            .map(|d| d.classes.into_iter().map(|(n, mut c)| {
                // Clip paths under Content/Animations, the folder the animation loader reads.
                let rel = |p: &mut String| *p = p.strip_prefix("Animations/").map_or_else(|| format!("../{p}"), str::to_string);
                c.locomotion.iter_mut().for_each(|l| rel(&mut l.0));
                if let Some(a) = c.animal.as_mut() {
                    a.attacks.iter_mut().chain(a.hits.iter_mut()).chain(a.death.iter_mut()).for_each(rel);
                }
                if let Some(b) = c.boss.as_mut() {
                    b.attack.iter_mut().for_each(rel);
                }
                (n, c)
            }).collect())
            .unwrap_or_default()
    })
}

/// How a class's body lies for hit tests and capsules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Upright,
    /// `BP_Zombie_Crawler`: dragging itself along the ground.
    Prone,
    Hound,
    /// A `BP_Animal`: a quadruped sized by its mesh's bounds.
    Animal,
}

/// Which zombie class: an index into [`zombie_classes`]. The default is the common
/// `BP_Zombie_Roamer_C` ("Undead Shambler").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ZombieKind(u8);

impl Default for ZombieKind {
    fn default() -> Self {
        Self::from_class("BP_Zombie_Roamer_C").unwrap_or(ZombieKind(0))
    }
}

/// Used when zombies.json is missing (tests on a machine without the data).
static FALLBACK: std::sync::LazyLock<ZombieClass> = std::sync::LazyLock::new(|| ZombieClass {
    name: None,
    meshes: Vec::new(),
    materials: Vec::new(),
    damage: 12.0,
    roam_speed: ROAM_SPEED,
    alert_speed: ALERT_SPEED,
    health: 100.0,
    health_deviation: 10.0,
    attack_interval: [1.45, 2.05],
    bleed: 0.0,
    heavy_bleed: 0.0,
    broken_bone: 0.0,
    rad_chance: 0.0,
    rad_damage: 0.0,
    xp: [5.0, 10.0],
    loot_table: None,
        shovable: true,
    container: None,
    capsule_half_height: 0.9,
    capsule_radius: 0.35,
    locomotion: Vec::new(),
    animal: None,
    boss: None,
    human: None,
});

impl ZombieKind {
    pub fn from_class(class: &str) -> Option<Self> {
        zombie_classes().iter().position(|(n, _)| n == class).map(|i| ZombieKind(i as u8))
    }
    pub fn hound() -> Self {
        Self::from_class("BP_Zombie_Dog_C").unwrap_or_default()
    }
    pub fn class_name(self) -> &'static str {
        zombie_classes().get(self.0 as usize).map_or("BP_Zombie_Roamer_C", |c| c.0.as_str())
    }
    pub fn class(self) -> &'static ZombieClass {
        zombie_classes().get(self.0 as usize).map_or(&*FALLBACK, |c| &c.1)
    }
    /// The class's animal traits, if it's a `BP_Animal`.
    pub fn animal(self) -> Option<&'static AnimalTraits> {
        self.class().animal.as_ref()
    }
    /// The class's boss traits, if it's a `BP_ZombieBoss`.
    pub fn boss(self) -> Option<&'static BossTraits> {
        self.class().boss.as_ref()
    }
    /// The class's human traits, if it's one of SmartAI's characters.
    pub fn human(self) -> Option<&'static HumanTraits> {
        self.class().human.as_ref()
    }
    /// The sight cone's half-angle (BP_ZombieBoss_AIController widens it to 110°).
    pub fn peripheral(self) -> f32 {
        self.human().map(|h| h.peripheral).or(self.boss().map(|b| b.peripheral)).unwrap_or(PERIPHERAL)
    }
    /// Sight and lose-sight radii (the human AI controllers' AISenseConfig_Sight).
    pub fn sight(self) -> (f32, f32) {
        self.human().map_or((SIGHT, LOSE_SIGHT), |h| (h.sight, h.lose_sight))
    }
    /// The wait (s) after reaching an investigated spot and before roaming on: BTTask_Wait_4 /
    /// _3 (30 ± 5, 30 ± 15), the boss tree's _14 / _15 (30 ± 15 both).
    pub fn wait(self, after_investigating: bool, rng: &mut impl Rng) -> f32 {
        // SmartAI: Roam Wait Time ± its deviation.
        let w = self.human().map(|h| h.roam_wait).or_else(|| self.boss().and_then(|b| b.waits.get(if after_investigating { 0 } else { 1 }).copied()));
        let [t, dev] = w.unwrap_or(if after_investigating { [30.0, 5.0] } else { [30.0, 15.0] });
        if dev > 0.0 { rng.gen_range(t - dev..t + dev) } else { t }
    }
    /// The mesh's RelativeScale3D (bosses are drawn up to 1.5x).
    pub fn mesh_scale(self) -> Vec3 {
        match (self.boss(), self.animal()) {
            (Some(b), _) => Vec3::from_array(b.scale),
            (_, Some(a)) => Vec3::splat(a.scale),
            _ => Vec3::ONE,
        }
    }

    /// The capsule's centre for a body drawn at `t` (an animal's mesh sits behind and below it).
    pub fn centre(self, t: &Transform) -> Vec3 {
        let c = self.class();
        let back = c.animal.as_ref().map_or(0.0, |a| a.mesh_back);
        t.translation + t.rotation * Vec3::Z * back + Vec3::Y * c.capsule_half_height
    }

    /// AttackSound (BP_MasterZombie: ZombieGrowlShort; BP_Zombie_Dog: HoundAttackCue; the
    /// animals' AttackPlayer plays none).
    pub fn attack_sound(self) -> Option<&'static str> {
        if self.human().is_some() {
            return None;
        }
        match self.shape() {
            Shape::Hound => Some("HoundAttackCue"),
            Shape::Animal => None,
            _ => Some("ZombieGrowlShort"),
        }
    }

    /// DeathSound (the hound, the animals and the humans have none).
    pub fn death_sound(self) -> Option<&'static str> {
        (self.human().is_none() && matches!(self.shape(), Shape::Upright | Shape::Prone)).then_some("ZombieDeathShort")
    }

    /// The Zombie_Cue AudioComponent's loop (BP_Zombie_Dog: HoundCue; Deer_Noises, WolfCue,
    /// BearCue; rabbits are silent).
    pub fn voice(self) -> Option<&'static str> {
        if self.human().is_some() {
            return None;
        }
        match self.shape() {
            Shape::Hound => Some("HoundCue"),
            Shape::Animal => self.animal().and_then(|a| a.voice.as_deref()),
            _ if self.boss().is_some() => self.boss().and_then(|b| b.voice.as_deref()),
            _ => Some("ZombieBreathLoopCue"),
        }
    }

    pub fn shape(self) -> Shape {
        if self.class().animal.is_some() {
            return Shape::Animal;
        }
        match self.class_name() {
            "BP_Zombie_Dog_C" => Shape::Hound,
            "BP_Zombie_Crawler_C" => Shape::Prone,
            _ => Shape::Upright,
        }
    }
    pub fn alert_speed(self) -> f32 {
        self.class().alert_speed
    }
    pub fn roam_speed(self) -> f32 {
        self.class().roam_speed
    }
    pub fn attack_interval(self) -> (f32, f32) {
        let [a, b] = self.class().attack_interval;
        (a, b.max(a + 1e-3))
    }
    pub fn shovable(self) -> bool {
        self.class().shovable
    }
    pub fn health(self) -> (f32, f32) {
        (self.class().health, self.class().health_deviation)
    }
    /// The capsule: radius and full height (`CapsuleHalfHeight` x 2).
    pub fn body(self) -> KinematicBody {
        let c = self.class();
        KinematicBody::new(c.capsule_radius, c.capsule_half_height * 2.0)
    }
    pub fn anim_set(self) -> AnimSet {
        // SmartAI's characters share the survivor's skeleton and stance animations.
        if self.human().is_some() {
            return AnimSet::Survivor;
        }
        match self.shape() {
            Shape::Hound => AnimSet::Hound,
            _ => AnimSet::Zombie(self),
        }
    }
    /// The difficulty row scaling its spawners (`GetCVarType` by `SpawnType`).
    pub fn spawn_multiplier(self) -> &'static str {
        match self.shape() {
            Shape::Hound => "ZOMBIE HOUND SPAWN AMOUNT MULTIPLIER",
            Shape::Animal => "ANIMAL SPAWN AMOUNT MULTIPLIER",
            _ if self.boss().is_some() => "ZOMBIE BOSS SPAWN AMOUNT MULTIPLIER",
            _ => "ZOMBIE SPAWN AMOUNT MULTIPLIER",
        }
    }
    /// A random look: (glb path under assets/, material override).
    pub fn pick_look(self, rng: &mut impl Rng) -> (String, Option<String>) {
        let c = self.class();
        let mesh = if c.meshes.is_empty() { "PolygonFiles/PolygonCityZombies/Meshes/Characters/Normal/SK_Zombie_Hoodie_Male_01".to_string() } else { c.meshes[rng.gen_range(0..c.meshes.len())].clone() };
        let mat = (!c.materials.is_empty()).then(|| c.materials[rng.gen_range(0..c.materials.len())].clone());
        (format!("game/SurrounDead/Content/{mesh}.glb"), mat)
    }
}

pub fn plugin(app: &mut App) {
    app.add_message::<Noise>().add_systems(
        Update,
        (adopt, (perceive, act).chain().run_if(in_state(MenuScreen::None)), zombie_voices)
            .chain()
            .run_if(in_state(AppState::InGame)),
    );
}

/// Zombies spawned by the level's spawners (or anything else tagged `LevelZombie`).
fn adopt(mut commands: Commands, mut added: Query<(Entity, &LevelZombie, &mut Transform), Added<LevelZombie>>) {
    let mut rng = rand::thread_rng();
    for (e, lz, mut t) in &mut added {
        // A boss's CharacterMesh0 is scaled up (the capsule isn't).
        if lz.kind.boss().is_some() {
            t.scale = lz.kind.mesh_scale();
        }
        // Animals think for themselves (`gameplay::animals`).
        if lz.kind.animal().is_none() {
            commands.entity(e).insert(Zombie { brain: Brain::Roam { goal: None, wait: rng.gen_range(0.0..10.0) }, last_seen: None, kind: lz.kind, in_sight: false });
        }
        commands.entity(e).insert((
            lz.kind.body(),
            Animated(lz.kind.anim_set()),
            Locomotion::Idle,
            UpperAction::default(),
        ));
    }
}

/// Is the line between two points clear of terrain?
pub(crate) fn clear_line(terrain: Option<&Terrain>, a: Vec3, b: Vec3) -> bool {
    let Some(t) = terrain else { return true };
    let d = b - a;
    let steps = (d.length() / 1.5).ceil().max(1.0) as usize;
    (1..steps).all(|i| {
        let p = a + d * (i as f32 / steps as f32);
        !t.blocks(p)
    })
}

/// Can a zombie at `pos` facing `forward` see `target`? `alert` keeps it in view out to the
/// lose-sight radius and all around.
pub fn sees(terrain: Option<&Terrain>, pos: Vec3, forward: Vec3, target: Vec3, alert: bool) -> bool {
    sees_within(terrain, pos, forward, target, alert, PERIPHERAL, (SIGHT, LOSE_SIGHT))
}

/// [`sees`] with the sight cone's half-angle (degrees) and the sight / lose-sight radii.
pub fn sees_within(terrain: Option<&Terrain>, pos: Vec3, forward: Vec3, target: Vec3, alert: bool, peripheral: f32, (sight, lose): (f32, f32)) -> bool {
    let to = (target - pos) * Vec3::new(1.0, 0.0, 1.0);
    let dist = to.length();
    if dist > if alert { lose } else { sight } {
        return false;
    }
    if !alert && dist > 1.0 && forward.angle_between(to).to_degrees() > peripheral {
        return false;
    }
    clear_line(terrain, pos + Vec3::Y * EYE, target + Vec3::Y * EYE)
}

#[allow(clippy::type_complexity)]
fn perceive(
    mut noises: MessageReader<Noise>,
    terrain: Option<Res<Terrain>>,
    mut props: crate::level::collision::PropRays,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
    mut zombies: Query<(&Transform, &mut Zombie, Option<Ref<Health>>), Without<Dead>>,
    vitals: Res<super::PlayerVitals>,
) {
    let noises: Vec<Noise> = noises.read().copied().collect();
    let Ok(me) = player.single() else { return };
    let mut rng = rand::thread_rng();
    for (t, mut z, health) in &mut zombies {
        let chasing = z.brain == Brain::Chase;
        let forward = t.rotation * Vec3::Z;
        let kind = z.kind;
        // Hurt: it knows where you are.
        let hurt = health.is_some_and(|h| h.is_changed() && !h.is_added() && h.hp < h.max);
        // AIPerception sight traces the visibility channel: walls and props block it too.
        let mut in_view = || {
            sees_within(terrain.as_deref(), t.translation, forward, me.translation, chasing, kind.peripheral(), kind.sight()) && {
                let (eye, at) = (t.translation + Vec3::Y * EYE, me.translation + Vec3::Y * EYE);
                let d = at - eye;
                props.cast(eye, d.normalize_or_zero(), d.length()).is_none()
            }
        };
        let seen = !vitals.is_dead() && in_view();
        z.in_sight = seen;
        // The friendly scavengers only turn on you once hurt (and while they keep you in view).
        let friendly = kind.human().is_some_and(|h| h.faction == "Scavenger") && !chasing;
        if !vitals.is_dead() && (hurt || (seen && !friendly)) {
            z.brain = Brain::Chase;
            z.last_seen = Some(me.translation);
            continue;
        }
        if chasing {
            // Lost sight: go to where it was last seen.
            let at = z.last_seen.unwrap_or(t.translation);
            z.brain = Brain::Investigate { at, wait: None };
        }
        // The loudest nearby noise wins over roaming (or a stale investigation).
        if let Some(n) = noises.iter().filter(|n| n.at.distance(t.translation) <= n.range).min_by(|a, b| a.at.distance(t.translation).total_cmp(&b.at.distance(t.translation))) {
            // Not exactly on the spot: they converge loosely.
            let jitter = Vec3::new(rng.gen_range(-2.0..2.0), 0.0, rng.gen_range(-2.0..2.0));
            z.brain = Brain::Investigate { at: n.at + jitter, wait: None };
        }
    }
}

#[allow(clippy::type_complexity)]
fn act(
    time: Res<Time>,
    store: Res<SettingsStore>,
    vitals: Res<super::PlayerVitals>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>, Without<Zombie>)>,
    mut zombies: Query<(&mut Transform, &mut Zombie, &mut Locomotion, Option<&mut Shoved>, Option<&super::burning::Burning>, Option<&super::humans::HumanArms>, (Has<HeldStill>, Has<super::world_events::BloodMoonBoost>)), Without<Dead>>,
) {
    let dt = time.delta_secs();
    let speed_mult = difficulty(&store, "AI", "ZOMBIE SPEED MULTIPLIER");
    let me = player.single().ok().map(|p| p.translation);
    let mut rng = rand::thread_rng();
    for (mut t, mut z, mut loco, shoved, burning, arms, (held, roused)) in &mut zombies {
        // The Blood Moon's Alert Speed x1.5.
        let alert = z.kind.alert_speed() * if roused { super::world_events::BloodMoonBoost::ALERT_SPEED } else { 1.0 };
        if held {
            loco.set_if_neq(Locomotion::Idle);
            continue;
        }
        // Reeling from a shove: standing still, not attacking.
        if let Some(mut s) = shoved
            && s.0 > 0.0
        {
            s.0 -= dt;
            loco.set_if_neq(Locomotion::Idle);
            continue;
        }
        let pos = t.translation;
        // (where to go, how fast, animation)
        let mut walk: Option<(Vec3, f32, Locomotion)> = None;
        match z.brain {
            Brain::Chase => match me {
                Some(p) if !vitals.is_dead() => {
                    let flat = (p - pos) * Vec3::new(1.0, 0.0, 1.0);
                    if let Some(h) = z.kind.human() {
                        // SmartAI: a ranged weapon holds and fires (`humans.rs`) once the target
                        // is in view within Range Attack Distance; melee closes to Melee Distance.
                        let (hold, speed) = match arms.and_then(|a| a.ranged()) {
                            Some(w) => (z.in_sight && flat.length() <= w.range, alert),
                            None => (flat.length() <= h.melee_distance, h.melee_speed),
                        };
                        if hold {
                            face(&mut t, flat, dt);
                            loco.set_if_neq(Locomotion::Idle);
                        } else {
                            walk = Some((p, speed, Locomotion::Run));
                        }
                    } else if let Some(stop) = z.kind.boss().map(|_| z.kind.class().capsule_radius + BOSS_STOP) {
                        // ZombieBoss_Chase: MoveToActor (stop on overlap) runs alongside the
                        // swings (`bosses.rs`), which play on the upper body.
                        if flat.length() > stop {
                            walk = Some((p, alert, Locomotion::Run));
                        } else {
                            face(&mut t, flat, dt);
                            loco.set_if_neq(Locomotion::Idle);
                        }
                    } else if flat.length() > MELEE_RANGE {
                        walk = Some((p, alert, Locomotion::Run));
                    } else {
                        face(&mut t, flat, dt);
                        loco.set_if_neq(Locomotion::Attack);
                    }
                }
                _ => z.brain = Brain::Roam { goal: None, wait: z.kind.wait(false, &mut rng) },
            },
            Brain::Investigate { at, wait } => match wait {
                None if (at - pos).xz().length() > ARRIVE * 2.0 => walk = Some((at, alert, Locomotion::Run)),
                None => z.brain = Brain::Investigate { at, wait: Some(z.kind.wait(true, &mut rng)) },
                Some(w) if w > 0.0 => z.brain = Brain::Investigate { at, wait: Some(w - dt) },
                Some(_) => z.brain = Brain::Roam { goal: None, wait: 0.0 },
            },
            Brain::Roam { goal, wait } => match goal {
                Some(g) if (g - pos).xz().length() > ARRIVE => walk = Some((g, z.kind.roam_speed(), Locomotion::Walk)),
                Some(_) => z.brain = Brain::Roam { goal: None, wait: z.kind.wait(false, &mut rng) },
                None if wait > 0.0 => z.brain = Brain::Roam { goal: None, wait: wait - dt },
                None => {
                    let a = rng.gen_range(0.0..std::f32::consts::TAU);
                    // SmartAI: Min / Max Roam Distance.
                    let [lo, hi] = z.kind.human().map_or([2.0, ROAM_RADIUS], |h| h.roam);
                    let r = rng.gen_range(lo..hi.max(lo + 0.1));
                    z.brain = Brain::Roam { goal: Some(pos + Vec3::new(a.cos() * r, 0.0, a.sin() * r)), wait: 0.0 };
                }
            },
        }
        match walk {
            Some((to, speed, anim)) => {
                let flat = (to - pos) * Vec3::new(1.0, 0.0, 1.0);
                let d = flat.length();
                if d > 1e-3 {
                    face(&mut t, flat, dt);
                    let step = (speed * speed_mult * super::burning::speed_factor(burning) * dt).min(d);
                    // Move the way it faces (it turns at 275°/s, so wide turns take a moment).
                    let fwd = t.rotation * Vec3::Z;
                    t.translation += fwd * step;
                }
                loco.set_if_neq(anim);
            }
            None if !matches!(z.brain, Brain::Chase) => {
                loco.set_if_neq(Locomotion::Idle);
            }
            None => {}
        }
    }
}

/// Turn toward a horizontal direction at the CharacterMovement rotation rate.
fn face(t: &mut Transform, dir: Vec3, dt: f32) {
    if dir.length_squared() < 1e-6 {
        return;
    }
    let want = Quat::from_rotation_y(dir.x.atan2(dir.z));
    let angle = t.rotation.angle_between(want);
    let max = TURN_RATE.to_radians() * dt;
    t.rotation = if angle <= max { want } else { t.rotation.slerp(want, max / angle) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_carry_their_blueprint_values() {
        if zombie_classes().is_empty() {
            return;
        }
        let k = |n: &str| ZombieKind::from_class(n).unwrap();
        // The common Roamer walks at 1 m/s and chases at 2.5 (not the master's 4.25).
        let roamer = ZombieKind::default();
        assert_eq!(roamer.class_name(), "BP_Zombie_Roamer_C");
        assert_eq!((roamer.roam_speed(), roamer.alert_speed(), roamer.health()), (1.0, 2.5, (100.0, 10.0)));
        assert_eq!(roamer.class().loot_table.as_deref(), Some("Zombie"));
        // Crawlers drag themselves at 1 m/s with crawl clips; Radiated / Infected are tougher.
        let crawler = k("BP_Zombie_Crawler_C");
        assert_eq!((crawler.shape(), crawler.alert_speed(), crawler.health().0), (Shape::Prone, 1.0, 75.0));
        assert!(crate::models::anim::clip_paths(crawler.anim_set()).iter().any(|c| c.contains("ground-move")));
        let radiated = k("BP_Zombie_Radiated_C");
        assert_eq!((radiated.health().0, radiated.class().damage, radiated.class().rad_chance), (200.0, 18.0, 1.0));
        assert_eq!(k("BP_Zombie_Infected_C").class().loot_table.as_deref(), Some("ZombieInfected"));
        assert_eq!(ZombieKind::hound().shape(), Shape::Hound);
        // Every class has meshes and a look to pick.
        for (name, c) in zombie_classes() {
            assert!(!c.meshes.is_empty(), "{name}");
        }
    }

    #[test]
    fn sight_needs_range_and_the_front_arc_until_alert() {
        let z = Vec3::ZERO;
        let fwd = Vec3::Z;
        assert!(sees(None, z, fwd, Vec3::new(0.0, 0.0, 20.0), false));
        assert!(!sees(None, z, fwd, Vec3::new(0.0, 0.0, 30.0), false), "beyond 25 m");
        assert!(sees(None, z, fwd, Vec3::new(0.0, 0.0, 30.0), true), "kept to 40 m once seen");
        assert!(!sees(None, z, fwd, Vec3::new(0.0, 0.0, -10.0), false), "behind");
        assert!(sees(None, z, fwd, Vec3::new(0.0, 0.0, -10.0), true), "alert: all round");
        assert!(sees(None, z, fwd, Vec3::new(8.0, 0.0, 5.0), false), "58° off: within 70°");
    }

    #[test]
    fn turning_is_rate_limited() {
        let mut t = Transform::default();
        face(&mut t, Vec3::NEG_Z, 0.1);
        let turned = Transform::default().rotation.angle_between(t.rotation).to_degrees();
        assert!((turned - 27.5).abs() < 0.5, "{turned}");
    }
}

/// Zombie_Cue: each living zombie in earshot breathes (VolumeMultiplier 0.5, the "Zombies"
/// attenuation); `audio` stops the loop out of earshot or when `Voiced` goes (death).
#[allow(clippy::type_complexity)]
pub fn zombie_voices(
    mut commands: Commands,
    camera: Query<&GlobalTransform, With<super::PlayerCamera>>,
    zombies: Query<(Entity, &Transform, &LevelZombie), (Without<super::Dead>, Without<crate::audio::Voiced>)>,
    mut sound: MessageWriter<crate::audio::Sound>,
) {
    let Ok(ear) = camera.single() else { return };
    let ear = ear.translation();
    for (e, t, z) in &zombies {
        // The Zombies attenuation reaches 2 + 30 m, the animals' 3 + 40 m.
        let reach = if z.kind.animal().is_some() { 43.0 } else { 30.0 };
        if t.translation.distance(ear) < reach {
            commands.entity(e).insert(crate::audio::Voiced);
            let Some(cue) = z.kind.voice() else { continue };
            // The animals' AudioComponents override their attenuation: 3 m inner, 40 m falloff.
            // BP_ZombieBoss's Zombie_Cue: VolumeMultiplier 0.2 over the same 2 + 30 m shape.
            let (volume, attenuation) = match (z.kind.animal(), z.kind.boss()) {
                (Some(_), _) => (1.0, super::animals::VOICE_ATTENUATION),
                (_, Some(_)) => (0.2, "Zombies"),
                _ => (0.5, "Zombies"),
            };
            let attenuation = Some(attenuation.to_string());
            sound.write(crate::audio::Sound { cue: cue.into(), at: Some(t.translation), volume, attenuation, follow: Some(e) });
        }
    }
}
