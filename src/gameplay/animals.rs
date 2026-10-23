//! Wildlife: `BP_Animal`'s children (deer, rabbits, wolves, bears), spawned by
//! `BP_AISpawner_Animals` and driven by their AI controllers and behaviour trees.
//!
//! Their classes sit in `assets/data/zombies.json` beside the zombies (`extractor/gen_ai.py`), so
//! spawning, shooting, corpses, loot and the optimiser are the zombies'; this module is their
//! perception and behaviour (`zombies::AnimalTraits` holds the per-species values):
//!
//! - **Sight** (AISenseConfig_Sight): `sight` m, kept to `lose_sight` once seen, `peripheral`°
//!   either side of the facing, blocked by terrain and props; the dead aren't seen.
//! - **Hearing**: a [`Noise`] within its own range and the animal's `hearing`; the stimulus
//!   lasts `hearing_age` s (MaxAge), and its expiry is a perception update too.
//! - **Deer and rabbits** (`flees`): seeing the survivor (tag CanScareDeer), or hearing anything,
//!   sets CanSeePlayer. Run Away then picks a point within `flee_radius` of one `away` m back from
//!   the nearest actor it *sees*, runs there at `alert_speed`, waits (`alert_waits`) and repeats.
//!   Heard but not seen, Run Away finds nobody and never finishes: the animal stands frozen until
//!   CanSeePlayer clears (sight lost, or the noise's MaxAge running out, which clears it even while
//!   it still sees you: the controller's ClearValue doesn't check). Otherwise it roams: a random
//!   point within `roam_radius`, walked to at `roam_speed`, then a wait.
//! - **Wolves and bears**: seeing the survivor (ZombieCanAttack, not Dead) snarls and sets
//!   CanSeePlayer: chase at `alert_speed`, attacking (`survival::zombie_attacks`: DamageToDo per
//!   AttackMontage, 15 % bleed) within MeleeAttackDistance of capsule centres. Hearing a noise
//!   sends them to look (`investigate_speed`, then wait). Otherwise they roam: a short wait, a
//!   point within `roam_radius`, a longer wait.
//! - Being hurt doesn't tell them where you are (no damage sense); deer and rabbits stop for the
//!   BP's 1 s (DisableMovement, Delay 1) and every species plays its hit montage.
//!
//! BTTask_Wait: `WaitTime ± RandomDeviation`, never below 0. Turning is CharacterMovement's
//! default RotationRate (360°/s, bOrientRotationToMovement).
//!
//! Not ported: wolves and bears scaring deer (CanScareDeer on them) and hunting deer and rabbits
//! (ZombieCanAttack); navigation (points are anywhere, not "reachable"), AutoSuccessRange,
//! the deer's antlers (socket S_Antlers), the meshes' offset from the capsule for collisions.

use bevy::prelude::*;
use rand::Rng;

use super::combat::{Dead, Health};
use super::zombies::{AnimalTraits, Noise, ZombieKind};
use crate::level::{KinematicBody, LevelZombie, Terrain};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{CharacterModel, Locomotion};

/// CharacterMovement's default RotationRate yaw.
const TURN_RATE: f32 = 360.0;
/// The deer's and rabbit's DisableMovement on a hit, until the BP's Delay(1.0).
const HURT_STOP: f32 = 1.0;
/// The survivor's capsule centre over the feet (sight and reach aim at it).
const PLAYER_CENTRE: f32 = 0.82;
/// `survival::zombie_attacks` still bites this far beyond MeleeAttackDistance.
pub const REACH_SLACK: f32 = 0.3;

/// What an animal is doing (the running branch of its behaviour tree).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Brain {
    /// The roam sequence: `phase` 0 = the hunters' first wait, 1 = walking to `goal`, 2 = the
    /// wait after; `wait` is the current wait's time left.
    Roam { phase: u8, wait: f32, goal: Vec3 },
    /// Running Away to `goal`, then waiting there (`wait`).
    Flee { goal: Vec3, wait: Option<f32> },
    /// Run Away found nobody in sight: stuck until CanSeePlayer clears.
    Frozen,
    /// Wolves and bears: after the survivor.
    Chase,
    /// Wolves and bears: to the last heard noise, then waiting there.
    Investigate { at: Vec3, wait: Option<f32> },
}

/// One animal's mind: its blackboard and its tree's running branch.
#[derive(Component, Debug)]
pub struct Animal {
    pub brain: Brain,
    /// Sight currently senses the survivor.
    pub seen: bool,
    /// The CanSeePlayer blackboard key.
    pub can_see_player: bool,
    /// The hunters' IsInvestigating and LastHeardLocation.
    pub investigating: Option<Vec3>,
    /// Seconds left of the last hearing stimulus.
    pub heard: Option<f32>,
    /// Movement disabled after a hit.
    pub stopped: f32,
    /// Close enough to attack: set inside MeleeAttackDistance, cleared beyond it plus the bite
    /// check's 0.3 m slack (ours: without it the chase flickers between running and biting).
    pub in_reach: bool,
}

impl Animal {
    pub fn new(rng: &mut impl Rng) -> Self {
        Self { brain: Brain::Roam { phase: 0, wait: rng.gen_range(0.0..5.0), goal: Vec3::ZERO }, seen: false, can_see_player: false, investigating: None, heard: None, stopped: 0.0, in_reach: false }
    }
}

/// BTTask_Wait's duration: `FRandRange(max(0, WaitTime - RandomDeviation), WaitTime + RandomDeviation)`.
pub fn wait_time([time, dev]: [f32; 2], rng: &mut impl Rng) -> f32 {
    let (lo, hi) = ((time - dev).max(0.0), time + dev);
    if hi > lo { rng.gen_range(lo..hi) } else { lo }
}

/// A random point on the ground plane within `radius` of `at` (GetRandomReachablePointInRadius,
/// without the navmesh).
pub fn point_within(at: Vec3, radius: f32, rng: &mut impl Rng) -> Vec3 {
    let a = rng.gen_range(0.0..std::f32::consts::TAU);
    let r = radius * rng.gen_range(0.0f32..1.0).sqrt();
    at + Vec3::new(a.cos() * r, 0.0, a.sin() * r)
}

/// Run Away: `away` m back from the threat, then a random point within a radius rolled in
/// `flee_radius` of there (the height is the animal's own).
pub fn flee_goal(pos: Vec3, threat: Vec3, away: f32, [lo, hi]: [f32; 2], rng: &mut impl Rng) -> Vec3 {
    let dir = (threat - pos).with_y(0.0).normalize_or(Vec3::Z);
    let back = pos - dir * away;
    point_within(back, rng.gen_range(lo..=hi), rng).with_y(pos.y)
}

/// AISenseConfig_Sight in plan: within `radius` and `peripheral`° of the facing. (Line of
/// sight is checked separately.)
pub fn in_sight(pos: Vec3, forward: Vec3, target: Vec3, radius: f32, peripheral: f32) -> bool {
    let to = (target - pos).with_y(0.0);
    let d = to.length();
    d <= radius && (d < 0.5 || forward.with_y(0.0).angle_between(to).to_degrees() <= peripheral)
}

/// A perception update: sight gained or lost, a noise heard, the noise's MaxAge running out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sensed {
    SightGained,
    SightLost,
    Heard(Vec3),
    HearingExpired,
}

/// The AI controller's PerceptionUpdate: the blackboard keys a stimulus sets or clears.
pub fn perceive(a: &mut Animal, traits: &AnimalTraits, what: Sensed) {
    match (traits.flees, what) {
        // BP_AnimalDeer/Rabbit_AIController: sight (CanScareDeer) and hearing both set it;
        // either one being lost clears it.
        (true, Sensed::SightGained | Sensed::Heard(_)) => a.can_see_player = true,
        (true, Sensed::SightLost | Sensed::HearingExpired) => a.can_see_player = false,
        // BP_AnimalWolf/Bear_AIController: sight sets/clears CanSeePlayer, hearing
        // IsInvestigating + LastHeardLocation.
        (false, Sensed::SightGained) => a.can_see_player = true,
        (false, Sensed::SightLost) => a.can_see_player = false,
        (false, Sensed::Heard(at)) => a.investigating = Some(at),
        (false, Sensed::HearingExpired) => a.investigating = None,
    }
}

/// The behaviour tree's root selector: which branch runs now, restarting it when it changes.
/// `threat` is the survivor while sight senses them.
pub fn choose(a: &mut Animal, traits: &AnimalTraits, pos: Vec3, threat: Option<Vec3>, rng: &mut impl Rng) {
    // The roam sequence from the top: the hunters' first wait, else straight to a new spot.
    let mut roam = || {
        let first = traits.roam_waits.first().copied().filter(|_| traits.roam_waits.len() > 1);
        Brain::Roam { phase: if first.is_some() { 0 } else { 1 }, wait: first.map_or(0.0, |w| wait_time(w, rng)), goal: point_within(pos, traits.roam_radius, rng) }
    };
    let want_roam = |b: &Brain| !matches!(b, Brain::Roam { .. });
    if traits.flees {
        match (a.can_see_player, a.brain) {
            // Stop Anim, then Run Away: from whoever it sees, or stuck.
            (true, Brain::Roam { .. }) => {
                a.brain = match threat {
                    Some(t) => Brain::Flee { goal: flee_goal(pos, t, traits.away, traits.flee_radius.unwrap_or([30.0, 40.0]), rng), wait: None },
                    None => Brain::Frozen,
                }
            }
            (false, b) if want_roam(&b) => a.brain = roam(),
            _ => {}
        }
        return;
    }
    // Investigate Noise, then Chase, then roam (Selector_1's children in order).
    let next = if a.investigating.is_some() && !a.can_see_player {
        match a.brain {
            Brain::Investigate { .. } => None,
            _ => a.investigating.map(|at| Brain::Investigate { at, wait: None }),
        }
    } else if a.can_see_player {
        // StopInvestigating, Wolf_FindPlayerLocation, then chase and attack in parallel.
        a.investigating = None;
        (a.brain != Brain::Chase).then_some(Brain::Chase)
    } else {
        want_roam(&a.brain).then(roam)
    };
    if let Some(b) = next {
        a.brain = b;
    }
}

/// Where the animal wants to go this frame and how fast; `None` stands. Advances waits and
/// loops sequences. `centre`: its capsule centre; `player`: the survivor's.
pub fn step(a: &mut Animal, traits: &AnimalTraits, kind: ZombieKind, centre: Vec3, player: Option<Vec3>, dt: f32, rng: &mut impl Rng) -> Option<(Vec3, f32)> {
    let arrived = |goal: Vec3| (goal - centre).xz().length() <= traits.arrive;
    match a.brain {
        Brain::Roam { phase: 0, wait, goal } => {
            a.brain = if wait > dt { Brain::Roam { phase: 0, wait: wait - dt, goal } } else { Brain::Roam { phase: 1, wait: 0.0, goal } };
            None
        }
        Brain::Roam { phase: 1, goal, .. } if !arrived(goal) => Some((goal, kind.roam_speed())),
        Brain::Roam { phase: 1, goal, .. } => {
            let w = traits.roam_waits.last().copied().unwrap_or([0.0, 0.0]);
            a.brain = Brain::Roam { phase: 2, wait: wait_time(w, rng), goal };
            None
        }
        Brain::Roam { wait, goal, .. } => {
            a.brain = if wait > dt {
                Brain::Roam { phase: 2, wait: wait - dt, goal }
            } else {
                // The sequence again from the top.
                let first = traits.roam_waits.first().copied().filter(|_| traits.roam_waits.len() > 1);
                Brain::Roam { phase: if first.is_some() { 0 } else { 1 }, wait: first.map_or(0.0, |w| wait_time(w, rng)), goal: point_within(centre, traits.roam_radius, rng) }
            };
            None
        }
        Brain::Flee { goal, wait: None } if !arrived(goal) => Some((goal, kind.alert_speed())),
        Brain::Flee { goal, wait: None } => {
            a.brain = Brain::Flee { goal, wait: Some(traits.alert_waits.first().map_or(0.0, |w| wait_time(*w, rng))) };
            None
        }
        Brain::Flee { goal, wait: Some(w) } => {
            a.brain = if w > dt {
                Brain::Flee { goal, wait: Some(w - dt) }
            } else {
                // Run Away again: from whoever it sees now, or stuck.
                match player.filter(|_| a.seen) {
                    Some(t) => Brain::Flee { goal: flee_goal(centre, t, traits.away, traits.flee_radius.unwrap_or([30.0, 40.0]), rng), wait: None },
                    None => Brain::Frozen,
                }
            };
            None
        }
        Brain::Frozen => None,
        Brain::Chase => {
            let d = player.map_or(f32::MAX, |p| p.distance(centre));
            a.in_reach = if a.in_reach { d <= traits.reach + REACH_SLACK } else { d <= traits.reach - 0.1 };
            player.filter(|_| !a.in_reach).map(|p| (p, kind.alert_speed()))
        }
        Brain::Investigate { at, wait: None } if !arrived(at) => Some((at, traits.investigate_speed.unwrap_or(kind.roam_speed()))),
        Brain::Investigate { at, wait: None } => {
            a.brain = Brain::Investigate { at, wait: Some(traits.investigate_waits.first().map_or(0.0, |w| wait_time(*w, rng))) };
            None
        }
        Brain::Investigate { at, wait: Some(w) } => {
            if w > dt {
                a.brain = Brain::Investigate { at, wait: Some(w - dt) };
            } else {
                // StopInvestigating: back to roaming on the next choice.
                a.investigating = None;
                a.brain = Brain::Roam { phase: 0, wait: 0.0, goal: centre };
            }
            None
        }
    }
}

/// The animals' noise AudioComponents (Deer_Noises, Wolf_Noises, Bear_Noises): AttenuationSettings
/// GunshotsRifleSuppressed with overrides AttenuationShapeExtents 300 cm, FalloffDistance 4000 cm.
pub const VOICE_ATTENUATION: &str = "GunshotsRifleSuppressed (animal noises)";

fn voice_attenuation(db: Option<ResMut<crate::audio::SoundDb>>) {
    let Some(mut db) = db else { return };
    if let Some(base) = db.attenuation.get("GunshotsRifleSuppressed").copied() {
        db.attenuation.insert(VOICE_ATTENUATION.into(), crate::audio::Attenuation { inner: 3.0, falloff: 40.0, ..base });
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, voice_attenuation).add_systems(Update, (adopt, (sense, act).chain().run_if(in_state(MenuScreen::None)), butcher).chain().run_if(in_state(AppState::InGame)));
}

/// Animals the spawners place: their mind and the mesh's scale.
fn adopt(mut commands: Commands, mut added: Query<(Entity, &LevelZombie, &mut Transform), Added<LevelZombie>>) {
    let mut rng = rand::thread_rng();
    for (e, lz, mut t) in &mut added {
        let Some(a) = lz.kind.animal() else { continue };
        t.scale = Vec3::splat(a.scale);
        commands.entity(e).insert(Animal::new(&mut rng));
    }
}

/// The AI controller's perception: sight and hearing updates into the blackboard, then the
/// tree's choice of branch. A hit stops deer and rabbits (DisableMovement for the 1 s Delay).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sense(
    time: Res<Time>,
    mut noises: MessageReader<Noise>,
    terrain: Option<Res<Terrain>>,
    mut props: crate::level::collision::PropRays,
    vitals: Res<super::PlayerVitals>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
    mut animals: Query<(&Transform, &LevelZombie, &mut Animal, Option<Ref<Health>>), Without<Dead>>,
    mut sound: MessageWriter<crate::audio::Sound>,
) {
    let dt = time.delta_secs();
    let noises: Vec<Noise> = noises.read().copied().collect();
    let me = player.single().ok().filter(|_| !vitals.is_dead()).map(|p| p.translation + Vec3::Y * PLAYER_CENTRE);
    let mut rng = rand::thread_rng();
    for (t, lz, mut a, health) in &mut animals {
        let Some(traits) = lz.kind.animal() else { continue };
        let centre = lz.kind.centre(t);
        let forward = t.rotation * Vec3::Z;
        let sensed = me.is_some_and(|p| {
            in_sight(centre, forward, p, if a.seen { traits.lose_sight } else { traits.sight }, traits.peripheral)
                && terrain.as_deref().is_none_or(|tr| super::zombies::clear_line(Some(tr), centre, p))
                && props.cast(centre, (p - centre).normalize_or_zero(), p.distance(centre)).is_none()
        });
        if sensed != a.seen {
            a.seen = sensed;
            perceive(&mut a, traits, if sensed { Sensed::SightGained } else { Sensed::SightLost });
            // The hunters' controllers snarl on seeing their prey.
            if sensed && let Some(cue) = &traits.snarl {
                sound.write(crate::audio::Sound::at(cue, centre));
            }
        }
        if let Some(n) = noises.iter().find(|n| n.at.distance(centre) <= n.range.min(traits.hearing)) {
            a.heard = Some(traits.hearing_age);
            perceive(&mut a, traits, Sensed::Heard(n.at));
        } else if let Some(left) = a.heard {
            if left > dt {
                a.heard = Some(left - dt);
            } else {
                a.heard = None;
                perceive(&mut a, traits, Sensed::HearingExpired);
            }
        }
        if traits.flees && health.is_some_and(|h| h.is_changed() && !h.is_added() && h.hp < h.max) {
            a.stopped = HURT_STOP;
        }
        let threat = me.filter(|_| a.seen);
        choose(&mut a, traits, centre, threat, &mut rng);
    }
}

/// The running branch's movement: walking/running toward its goal at the CharacterMovement
/// rotation rate, attacking in reach, standing otherwise.
#[allow(clippy::type_complexity)]
fn act(
    time: Res<Time>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>, Without<Animal>)>,
    vitals: Res<super::PlayerVitals>,
    mut animals: Query<(&mut Transform, &LevelZombie, &mut Animal, &mut Locomotion), Without<Dead>>,
) {
    let dt = time.delta_secs();
    let me = player.single().ok().filter(|_| !vitals.is_dead()).map(|p| p.translation + Vec3::Y * PLAYER_CENTRE);
    let mut rng = rand::thread_rng();
    for (mut t, lz, mut a, mut loco) in &mut animals {
        let Some(traits) = lz.kind.animal() else { continue };
        if a.stopped > 0.0 {
            a.stopped -= dt;
            loco.set_if_neq(Locomotion::Idle);
            continue;
        }
        let centre = lz.kind.centre(&t);
        let walk = step(&mut a, traits, lz.kind, centre, me, dt, &mut rng);
        match walk {
            Some((to, speed)) => {
                let flat = (to - centre).with_y(0.0);
                let d = flat.length();
                if d > 1e-3 {
                    face(&mut t, flat, dt);
                    let fwd = t.rotation * Vec3::Z;
                    t.translation += fwd * (speed * dt).min(d);
                }
                loco.set_if_neq(gait(lz.kind, speed));
            }
            None if a.brain == Brain::Chase => {
                // In reach: turn on the survivor and bite / claw (`survival::zombie_attacks`).
                if let Some(p) = me {
                    face(&mut t, (p - centre).with_y(0.0), dt);
                }
                loco.set_if_neq(Locomotion::Attack);
            }
            None => {
                loco.set_if_neq(Locomotion::Idle);
            }
        }
    }
}

/// The Locomotion1D sample a speed lands nearest: the walk or the run.
fn gait(kind: ZombieKind, speed: f32) -> Locomotion {
    let samples = &kind.class().locomotion;
    let nearest = samples.iter().min_by(|a, b| (a.1 - speed).abs().total_cmp(&(b.1 - speed).abs())).map_or(0.0, |s| s.1);
    let top = samples.iter().map(|s| s.1).fold(0.0, f32::max);
    if nearest >= top && top > 0.0 { Locomotion::Run } else { Locomotion::Walk }
}

fn face(t: &mut Transform, dir: Vec3, dt: f32) {
    if dir.length_squared() < 1e-6 {
        return;
    }
    let want = Quat::from_rotation_y(dir.x.atan2(dir.z));
    let angle = t.rotation.angle_between(want);
    let max = TURN_RATE.to_radians() * dt;
    t.rotation = if angle <= max { want } else { t.rotation.slerp(want, max / angle) };
}

/// Skill_Huntsman: a kill also leaves a Container_AnimalMeat ("Animal Meat", its loot table
/// AnimalMeat) at the animal (BeginDeferredActorSpawnFromClass at its transform).
#[allow(clippy::type_complexity)]
fn butcher(
    mut commands: Commands,
    assets: Res<AssetServer>,
    db: Res<super::ItemDb>,
    game: Res<CurrentGame>,
    mut containers: ResMut<super::interact::Containers>,
    died: Query<(&Transform, &LevelZombie), Added<Dead>>,
) {
    for (t, lz) in &died {
        let Some(traits) = lz.kind.animal() else { continue };
        let huntsman = game.state.as_ref().is_some_and(|s| s.tech.has(crate::progression::tech::HUNTSMAN));
        if !traits.huntsman_meat || !huntsman {
            continue;
        }
        const CLASS: &str = "Container_AnimalMeat_C";
        let key = containers.add_dropped(t.translation, CLASS, Some("Animal Meat".into()));
        if let Some(mesh) = db.containers.get(CLASS).and_then(|c| c.mesh.as_deref()) {
            commands.spawn((crate::models::model(&assets, format!("game/{mesh}.glb")), Transform::from_translation(t.translation).with_rotation(t.rotation), DespawnOnExit(AppState::InGame)));
        }
        info!("huntsman: Animal Meat at {:.1} (container {key})", t.translation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn class(name: &str) -> Option<(ZombieKind, &'static AnimalTraits)> {
        let k = ZombieKind::from_class(name)?;
        Some((k, k.animal()?))
    }

    #[test]
    fn species_carry_their_blueprint_values() {
        let (Some((deer, d)), Some((wolf, w)), Some((bear, b)), Some((rabbit, r))) = (class("BP_AnimalDeer_C"), class("BP_AnimalWolf_C"), class("BP_AnimalBear_C"), class("BP_AnimalRabbit_C")) else { return };
        // Deer: 100 HP, graze at 0.5 m/s, flee at 8 within 40-80 m of a point 5 m back, sight 40 m.
        assert_eq!((deer.health().0, deer.roam_speed(), deer.alert_speed()), (100.0, 0.5, 8.0));
        assert!(d.flees && d.flee_radius == Some([40.0, 80.0]) && d.away == 5.0 && d.sight == 40.0 && d.peripheral == 150.0);
        assert_eq!(deer.class().loot_table.as_deref(), Some("Deer"));
        assert_eq!(d.killed, "Deer");
        // Rabbits: 15 HP, flee within 30-40 m, scaled 1.2.
        assert_eq!(rabbit.health().0, 15.0);
        assert!(r.flees && r.flee_radius == Some([30.0, 40.0]) && (r.scale - 1.2).abs() < 1e-5 && r.death.is_some());
        // Wolves: 150 HP, bite 20 with a 15 % bleed, chase 8 / investigate 6 / roam 0.8 within 7.5 m.
        assert!(!w.flees);
        assert_eq!((wolf.health().0, wolf.class().damage, wolf.alert_speed(), w.investigate_speed, w.roam_radius), (150.0, 20.0, 8.0, Some(6.0), 7.5));
        assert!((wolf.class().bleed - 0.15).abs() < 1e-5 && w.snarl.as_deref() == Some("WolfSnarl_Cue"));
        // Bears: 500 HP, claw 31, chase 6, sight 25 m.
        assert_eq!((bear.health().0, bear.class().damage, bear.alert_speed(), b.sight), (500.0, 31.0, 6.0, 25.0));
        for (_, t) in [(deer, d), (wolf, w), (bear, b), (rabbit, r)] {
            assert_eq!(t.corpse_secs, 600.0);
            assert!(t.huntsman_meat && t.bounds.is_some() && t.reach == 2.0);
        }
    }

    fn traits(flees: bool) -> AnimalTraits {
        AnimalTraits {
            flees,
            sight: 40.0,
            lose_sight: 60.0,
            peripheral: 120.0,
            hearing: 160.0,
            hearing_age: 30.0,
            investigate_speed: Some(6.0),
            roam_radius: 25.0,
            roam_waits: if flees { vec![[15.0, 30.0]] } else { vec![[1.0, 3.0], [5.0, 30.0]] },
            alert_waits: vec![[10.0, 5.0]],
            investigate_waits: vec![[30.0, 45.0]],
            away: 5.0,
            flee_radius: Some([40.0, 80.0]),
            arrive: 0.7,
            reach: 2.0,
            scale: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn waits_follow_bt_task_wait() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        for _ in 0..200 {
            let w = wait_time([15.0, 30.0], &mut rng);
            assert!((0.0..45.0).contains(&w));
            let w = wait_time([10.0, 5.0], &mut rng);
            assert!((5.0..15.0).contains(&w));
        }
        assert_eq!(wait_time([5.0, 0.0], &mut rng), 5.0);
    }

    #[test]
    fn flight_goes_away_from_the_threat_on_average() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(2);
        let (pos, threat) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 20.0));
        for _ in 0..100 {
            let g = flee_goal(pos, threat, 5.0, [40.0, 80.0], &mut rng);
            // Within 80 m of the point 5 m back.
            assert!(g.distance(Vec3::new(0.0, 0.0, -5.0)) <= 80.0 + 1e-3 && g.y == 0.0);
        }
    }

    #[test]
    fn sight_needs_range_and_the_arc() {
        assert!(in_sight(Vec3::ZERO, Vec3::Z, Vec3::new(0.0, 0.0, 39.0), 40.0, 150.0));
        assert!(!in_sight(Vec3::ZERO, Vec3::Z, Vec3::new(0.0, 0.0, 41.0), 40.0, 150.0));
        // 150° either side: only straight behind is blind; 120° (wolves) misses 135°.
        assert!(in_sight(Vec3::ZERO, Vec3::Z, Vec3::new(10.0, 0.0, -10.0), 40.0, 150.0));
        assert!(!in_sight(Vec3::ZERO, Vec3::Z, Vec3::new(10.0, 0.0, -10.0), 40.0, 120.0));
    }

    #[test]
    fn deer_flee_what_they_see_and_freeze_at_what_they_hear() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        let t = traits(true);
        let mut a = Animal::new(&mut rng);
        // Heard, not seen: Run Away finds nobody: frozen.
        perceive(&mut a, &t, Sensed::Heard(Vec3::new(50.0, 0.0, 0.0)));
        choose(&mut a, &t, Vec3::ZERO, None, &mut rng);
        assert_eq!(a.brain, Brain::Frozen);
        // Seeing the survivor now changes nothing (CanSeePlayer was already set; the task hangs).
        perceive(&mut a, &t, Sensed::SightGained);
        choose(&mut a, &t, Vec3::ZERO, Some(Vec3::new(0.0, 0.0, 10.0)), &mut rng);
        assert_eq!(a.brain, Brain::Frozen);
        // The noise's MaxAge runs out: CanSeePlayer clears and it roams again...
        perceive(&mut a, &t, Sensed::HearingExpired);
        choose(&mut a, &t, Vec3::ZERO, Some(Vec3::new(0.0, 0.0, 10.0)), &mut rng);
        assert!(matches!(a.brain, Brain::Roam { .. }));
        // ...until a sight update: then it runs from what it sees.
        perceive(&mut a, &t, Sensed::SightLost);
        perceive(&mut a, &t, Sensed::SightGained);
        choose(&mut a, &t, Vec3::ZERO, Some(Vec3::new(0.0, 0.0, 10.0)), &mut rng);
        assert!(matches!(a.brain, Brain::Flee { wait: None, .. }));
        perceive(&mut a, &t, Sensed::SightLost);
        choose(&mut a, &t, Vec3::ZERO, None, &mut rng);
        assert!(matches!(a.brain, Brain::Roam { phase: 1, .. }), "deer roam with no first wait");
    }

    #[test]
    fn a_chase_stops_inside_reach_and_holds_there() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(5);
        let Some((wolf, _)) = class("BP_AnimalWolf_C") else { return };
        let t = traits(false);
        let mut a = Animal::new(&mut rng);
        a.brain = Brain::Chase;
        let at = |d: f32| Some(Vec3::new(0.0, 0.0, d));
        assert!(step(&mut a, &t, wolf, Vec3::ZERO, at(5.0), 0.1, &mut rng).is_some(), "runs in");
        assert!(step(&mut a, &t, wolf, Vec3::ZERO, at(1.85), 0.1, &mut rng).is_none(), "in reach: bites");
        // Wobbling around 2 m doesn't send it running again; leaving does.
        assert!(step(&mut a, &t, wolf, Vec3::ZERO, at(2.2), 0.1, &mut rng).is_none());
        assert!(step(&mut a, &t, wolf, Vec3::ZERO, at(2.4), 0.1, &mut rng).is_some());
    }

    #[test]
    fn wolves_investigate_noises_and_chase_what_they_see() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(4);
        let t = traits(false);
        let mut a = Animal::new(&mut rng);
        let shot = Vec3::new(30.0, 0.0, 0.0);
        perceive(&mut a, &t, Sensed::Heard(shot));
        choose(&mut a, &t, Vec3::ZERO, None, &mut rng);
        assert_eq!(a.brain, Brain::Investigate { at: shot, wait: None });
        perceive(&mut a, &t, Sensed::SightGained);
        choose(&mut a, &t, Vec3::ZERO, Some(Vec3::new(0.0, 0.0, 10.0)), &mut rng);
        assert_eq!((a.brain, a.investigating), (Brain::Chase, None));
        perceive(&mut a, &t, Sensed::SightLost);
        choose(&mut a, &t, Vec3::ZERO, None, &mut rng);
        assert!(matches!(a.brain, Brain::Roam { phase: 0, .. }), "wolves wait before roaming");
    }
}
