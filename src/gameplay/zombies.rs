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
const EYE: f32 = 1.6;

/// Something loud happened (`ReportNoiseEvent`): zombies within `range` go to look.
#[derive(Message, Clone, Copy)]
pub struct Noise {
    pub at: Vec3,
    pub range: f32,
}

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
    /// BP_JigMultiplayer's container name ("Dead Zombie", "Dead Zombie Hound").
    #[serde(default)]
    pub container: Option<String>,
    pub capsule_half_height: f32,
    pub capsule_radius: f32,
    /// `SD_AIComponent.MovementBlendspace` samples: (clip under Content/, speed).
    pub locomotion: Vec<(String, f32)>,
}

#[derive(Deserialize)]
struct ZombieData {
    classes: std::collections::BTreeMap<String, ZombieClass>,
}

/// Every zombie class, by Blueprint class name (sorted, so indices are stable).
pub fn zombie_classes() -> &'static [(String, ZombieClass)] {
    static CLASSES: std::sync::OnceLock<Vec<(String, ZombieClass)>> = std::sync::OnceLock::new();
    CLASSES.get_or_init(|| {
        std::fs::read_to_string("assets/data/zombies.json")
            .ok()
            .and_then(|t| serde_json::from_str::<ZombieData>(&t).ok())
            .map(|d| d.classes.into_iter().collect())
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
    container: None,
    capsule_half_height: 0.9,
    capsule_radius: 0.35,
    locomotion: Vec::new(),
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
    pub fn shape(self) -> Shape {
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
    pub fn health(self) -> (f32, f32) {
        (self.class().health, self.class().health_deviation)
    }
    /// The capsule: radius and full height (`CapsuleHalfHeight` x 2).
    pub fn body(self) -> KinematicBody {
        let c = self.class();
        KinematicBody::new(c.capsule_radius, c.capsule_half_height * 2.0)
    }
    pub fn anim_set(self) -> AnimSet {
        match self.shape() {
            Shape::Hound => AnimSet::Hound,
            _ => AnimSet::Zombie(self),
        }
    }
    /// The difficulty row scaling its spawners (`GetCVarType` by `SpawnType`).
    pub fn spawn_multiplier(self) -> &'static str {
        match self.shape() {
            Shape::Hound => "ZOMBIE HOUND SPAWN AMOUNT MULTIPLIER",
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
        (adopt, (perceive, act).chain().run_if(in_state(MenuScreen::None)))
            .chain()
            .run_if(in_state(AppState::InGame)),
    );
}

/// Zombies spawned by the level's spawners (or anything else tagged `LevelZombie`).
fn adopt(mut commands: Commands, added: Query<(Entity, &LevelZombie), Added<LevelZombie>>) {
    let mut rng = rand::thread_rng();
    for (e, lz) in &added {
        commands.entity(e).insert((
            Zombie { brain: Brain::Roam { goal: None, wait: rng.gen_range(0.0..10.0) }, last_seen: None, kind: lz.kind },
            lz.kind.body(),
            Animated(lz.kind.anim_set()),
            Locomotion::Idle,
            UpperAction::default(),
        ));
    }
}

/// Is the line between two points clear of terrain?
fn clear_line(terrain: Option<&Terrain>, a: Vec3, b: Vec3) -> bool {
    let Some(t) = terrain else { return true };
    let d = b - a;
    let steps = (d.length() / 1.5).ceil().max(1.0) as usize;
    (1..steps).all(|i| {
        let p = a + d * (i as f32 / steps as f32);
        p.y > t.height(p.x, p.z)
    })
}

/// Can a zombie at `pos` facing `forward` see `target`? `alert` keeps it in view out to the
/// lose-sight radius and all around.
pub fn sees(terrain: Option<&Terrain>, pos: Vec3, forward: Vec3, target: Vec3, alert: bool) -> bool {
    let to = (target - pos) * Vec3::new(1.0, 0.0, 1.0);
    let dist = to.length();
    if dist > if alert { LOSE_SIGHT } else { SIGHT } {
        return false;
    }
    if !alert && dist > 1.0 && forward.angle_between(to).to_degrees() > PERIPHERAL {
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
        // Hurt: it knows where you are.
        let hurt = health.is_some_and(|h| h.is_changed() && !h.is_added() && h.hp < h.max);
        // AIPerception sight traces the visibility channel: walls and props block it too.
        let mut in_view = || {
            sees(terrain.as_deref(), t.translation, forward, me.translation, chasing) && {
                let (eye, at) = (t.translation + Vec3::Y * EYE, me.translation + Vec3::Y * EYE);
                let d = at - eye;
                props.cast(eye, d.normalize_or_zero(), d.length()).is_none()
            }
        };
        if !vitals.is_dead() && (hurt || in_view()) {
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
    mut zombies: Query<(&mut Transform, &mut Zombie, &mut Locomotion), Without<Dead>>,
) {
    let dt = time.delta_secs();
    let speed_mult = difficulty(&store, "AI", "ZOMBIE SPEED MULTIPLIER");
    let me = player.single().ok().map(|p| p.translation);
    let mut rng = rand::thread_rng();
    for (mut t, mut z, mut loco) in &mut zombies {
        let pos = t.translation;
        // (where to go, how fast, animation)
        let mut walk: Option<(Vec3, f32, Locomotion)> = None;
        match z.brain {
            Brain::Chase => match me {
                Some(p) if !vitals.is_dead() => {
                    let flat = (p - pos) * Vec3::new(1.0, 0.0, 1.0);
                    if flat.length() > MELEE_RANGE {
                        walk = Some((p, z.kind.alert_speed(), Locomotion::Run));
                    } else {
                        face(&mut t, flat, dt);
                        loco.set_if_neq(Locomotion::Attack);
                    }
                }
                _ => z.brain = Brain::Roam { goal: None, wait: rng.gen_range(15.0..45.0) },
            },
            Brain::Investigate { at, wait } => match wait {
                None if (at - pos).xz().length() > ARRIVE * 2.0 => walk = Some((at, z.kind.alert_speed(), Locomotion::Run)),
                None => z.brain = Brain::Investigate { at, wait: Some(rng.gen_range(25.0..35.0)) },
                Some(w) if w > 0.0 => z.brain = Brain::Investigate { at, wait: Some(w - dt) },
                Some(_) => z.brain = Brain::Roam { goal: None, wait: 0.0 },
            },
            Brain::Roam { goal, wait } => match goal {
                Some(g) if (g - pos).xz().length() > ARRIVE => walk = Some((g, z.kind.roam_speed(), Locomotion::Walk)),
                Some(_) => z.brain = Brain::Roam { goal: None, wait: rng.gen_range(15.0..45.0) },
                None if wait > 0.0 => z.brain = Brain::Roam { goal: None, wait: wait - dt },
                None => {
                    let a = rng.gen_range(0.0..std::f32::consts::TAU);
                    let r = rng.gen_range(2.0..ROAM_RADIUS);
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
                    let step = (speed * speed_mult * dt).min(d);
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
