//! Placed traps ('PlayerTrap' buildables), decompiled: Buildable_Spikes, _RazorWire, _LogWire,
//! _BearTrap and _LandMine. Each reacts to a character beginning to overlap its Box_0 (data:
//! buildables.json `trigger`); the survivor ('Player' tag) never sets one off.
//!
//! - **Spikes / razor wire / log wire**: the character is added to AIArray, DisableMovement, takes
//!   ApplyDamage 25 with a white floating number; a Delay (5 s for the spikes, 3 s for the wires;
//!   a running Delay isn't restarted) then sets every character in AIArray walking again.
//! - **Bear trap**: armed (DoOnce open, Box collision on): the first character in is caught
//!   (DisableMovement, moved onto the box's X/Y), the box's collision goes off, 25 damage and the
//!   number, and BearTrapTimeline closes the jaws (0.2 s, a flat-tangent cubic: each jaw's pitch
//!   0 -> 60°, roll 90, yaw 0 / 180). When it finishes, 10 s later Event_OpenBearTrap lets the
//!   character walk, reopens the DoOnce and reverses the timeline; 5 s after that the box
//!   collides again.
//! - **Landmine**: a character stepping on it, or Damage_Object (rounds, swings, blasts: any
//!   damage), sets off Event_Explode once: FX_Explosion_New (random yaw, 1-2 a side),
//!   explosion_large_06_Cue, the burn decal, the camera shake within 15 m; every character in its
//!   4 m ExplosionSphere that isn't a dead AI (the survivor included) takes 200-225 with a white
//!   number; a 50 m noise; 0.1 s later it's gone.
//!
//! Trap damage has no instigator, so its kills give no XP (`turrets::NoCredit`, as the turrets').
//! Ours: overlap = the character's capsule axis point nearest the box centre inside the box
//! grown by the capsule radius; held AIs stand idle (`zombies::Trapped`), so they don't swing
//! either; the decal and the camera shake aren't drawn; a landmine's remains aren't saved.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use rand::Rng;

use super::building::{BuildableDb, Buildables};
use super::combat::{Dead, Health};
use super::damage_numbers::{DamageNumber, body};
use super::particles::Fx;
use super::zombies::{Noise, Trapped};
use crate::level::{KinematicBody, LevelZombie};
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, CurrentGame};
use crate::models::CharacterModel;

/// ApplyDamage of the spikes, wires and bear trap.
pub const TRAP_DAMAGE: f32 = 25.0;
/// The landmine's ApplyDamage roll.
pub const MINE_DAMAGE: (f32, f32) = (200.0, 225.0);
/// BearTrapTimeline's length; the Delays after it closes and after it reopens.
pub const JAWS_SECS: f32 = 0.2;
pub const BEAR_HOLD: f32 = 10.0;
pub const BEAR_REARM: f32 = 5.0;
/// The jaws' pitch when shut.
const JAWS_SHUT: f32 = 60.0;
/// ReportNoiseEvent's MaxRange (m).
const MINE_NOISE: f32 = 50.0;
/// The Delay before K2_DestroyActor.
const MINE_GONE: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapKind {
    /// Spikes and the wires: their release Delay (s).
    Snare,
    Bear,
    Mine,
}

impl TrapKind {
    pub fn of(actor: &str) -> Option<(Self, f32)> {
        Some(match actor {
            "Buildable_Spikes_C" => (Self::Snare, 5.0),
            "Buildable_RazorWire_C" | "Buildable_LogWire_C" => (Self::Snare, 3.0),
            "Buildable_BearTrap_C" => (Self::Bear, 0.0),
            "Buildable_LandMine_C" => (Self::Mine, 0.0),
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Jaws {
    /// Open and armed.
    Armed,
    /// Closing (seconds in).
    Closing(f32),
    /// Shut on its catch, waiting out the 10 s.
    Shut(f32),
    /// Opening again (seconds in), then the 5 s before the box collides.
    Opening(f32),
    Rearming(f32),
}

impl Jaws {
    /// The timeline's alpha (0 open, 1 shut).
    pub fn alpha(self) -> f32 {
        let cubic = |t: f32| {
            let t = (t / JAWS_SECS).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        match self {
            Jaws::Armed | Jaws::Rearming(_) => 0.0,
            Jaws::Closing(t) => cubic(t),
            Jaws::Shut(_) => 1.0,
            Jaws::Opening(t) => 1.0 - cubic(t),
        }
    }

    /// Advance `dt`; true when the catch is let go (Event_OpenBearTrap).
    pub fn step(&mut self, dt: f32) -> bool {
        match self {
            Jaws::Armed => {}
            Jaws::Closing(t) => {
                *t += dt;
                if *t >= JAWS_SECS {
                    *self = Jaws::Shut(0.0);
                }
            }
            Jaws::Shut(t) => {
                *t += dt;
                if *t >= BEAR_HOLD {
                    *self = Jaws::Opening(0.0);
                    return true;
                }
            }
            Jaws::Opening(t) => {
                *t += dt;
                if *t >= JAWS_SECS {
                    *self = Jaws::Rearming(0.0);
                }
            }
            Jaws::Rearming(t) => {
                *t += dt;
                if *t >= BEAR_REARM {
                    *self = Jaws::Armed;
                }
            }
        }
        false
    }
}

/// A placed trap's state (by buildable id).
#[derive(Debug, Clone)]
pub struct TrapState {
    /// Who overlapped last frame (for begin-overlap).
    inside: HashSet<Entity>,
    /// AIArray.
    held: Vec<Entity>,
    /// The release Delay running.
    release_in: Option<f32>,
    pub jaws: Jaws,
    caught: Option<Entity>,
    /// The landmine went off: seconds since.
    pub exploded: Option<f32>,
}

impl Default for TrapState {
    fn default() -> Self {
        Self { inside: HashSet::new(), held: Vec::new(), release_in: None, jaws: Jaws::Armed, caught: None, exploded: None }
    }
}

#[derive(Resource, Default)]
pub struct Traps(pub HashMap<usize, TrapState>);

/// The traps' systems (`level_traps` runs before them).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrapSet;

/// A landmine's Event_Explode at a point with its ExplosionSphere radius (m): the placed ones and
/// the level's BP_Landmine (`level_traps.rs`) share it.
#[derive(Message, Clone, Copy, Debug)]
pub struct MineBlast {
    pub at: Vec3,
    pub radius: f32,
}

/// Damage_Object reaching an object at `c`: a round passing within 40 cm, a swing within 2 m in
/// front, a blast's radius (ours: the BPI's sources, as the barrels take them).
pub fn object_hit(c: Vec3, shots: &[super::turrets::Shot], swings: &[super::harvest::Swing], blasts: &[super::barrels::ObjectBlast]) -> bool {
    shots.iter().any(|s| {
        let along = (c - s.origin).dot(s.dir);
        along > 0.0 && along <= s.reach + 0.4 && (s.origin + s.dir * along).distance(c) <= 0.4
    }) || swings.iter().any(|s| {
        let flat = (c - s.feet).with_y(0.0);
        flat.length() <= 2.0 && flat.normalize_or_zero().dot(s.facing) > 0.5
    }) || blasts.iter().any(|b| b.at.distance(c) <= b.radius)
}

/// The bear trap's jaw parts.
#[derive(Component)]
pub struct Jaw(pub f32);

pub fn plugin(app: &mut App) {
    app.init_resource::<Traps>()
        .add_message::<MineBlast>()
        .add_systems(OnEnter(AppState::InGame), |mut t: ResMut<Traps>| t.0.clear())
        .add_systems(Update, (spring, close_jaws).chain().in_set(TrapSet).after(super::building::BuildSet).run_if(in_state(AppState::InGame)));
}

/// Whether a capsule (feet, radius, height) overlaps the box at `t` with half extents `half`.
pub fn capsule_in_box(t: &Transform, half: Vec3, feet: Vec3, radius: f32, height: f32) -> bool {
    let local = |p: Vec3| t.rotation.inverse() * (p - t.translation);
    let (a, b) = (local(feet + Vec3::Y * radius), local(feet + Vec3::Y * (height - radius).max(radius)));
    // The axis point nearest the box centre, tested against the box grown by the radius.
    let ab = b - a;
    let s = if ab.length_squared() > 1e-8 { (-a.dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    let p = a + ab * s;
    let q = p.abs() - half;
    q.max(Vec3::ZERO).length() <= radius
}

/// UE FRotator (degrees) as a glb-space quat (the extractor's rotator_quat + axis swap).
pub fn ue_rotator(pitch: f32, yaw: f32, roll: f32) -> Quat {
    let (p, y, r) = (pitch.to_radians() / 2.0, yaw.to_radians() / 2.0, roll.to_radians() / 2.0);
    let (sp, cp, sy, cy, sr, cr) = (p.sin(), p.cos(), y.sin(), y.cos(), r.sin(), r.cos());
    let (x, yy, z, w) = (cr * sp * sy - sr * cp * cy, -cr * sp * cy - sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy);
    Quat::from_xyzw(x, z, yy, -w)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn spring(
    mut commands: Commands,
    time: Res<Time>,
    db: Res<BuildableDb>,
    mut placed: ResMut<Buildables>,
    mut traps: ResMut<Traps>,
    (item_db, store): (Res<super::ItemDb>, Res<SettingsStore>),
    mut game: ResMut<CurrentGame>,
    mut zombies: Query<(Entity, &mut Transform, &KinematicBody, &mut Health), (With<LevelZombie>, Without<Dead>)>,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    (mut shots, mut swings, mut blasts): (MessageReader<super::turrets::Shot>, MessageReader<super::harvest::Swing>, MessageReader<super::barrels::ObjectBlast>),
    (mut numbers, mut fx, mut noise, mut sound): (MessageWriter<DamageNumber>, MessageWriter<Fx>, MessageWriter<Noise>, MessageWriter<crate::audio::Sound>),
    mut mine_blasts: MessageReader<MineBlast>,
    mut no_credit: ResMut<super::turrets::NoCredit>,
    mut rays: crate::level::collision::PropRays,
) {
    let dt = time.delta_secs();
    let shots: Vec<super::turrets::Shot> = shots.read().copied().collect();
    let swings: Vec<super::harvest::Swing> = swings.read().cloned().collect();
    let blasts: Vec<super::barrels::ObjectBlast> = blasts.read().copied().collect();
    let mut rng = rand::thread_rng();
    let mut gone = Vec::new();
    let mut booms: Vec<MineBlast> = mine_blasts.read().copied().collect();
    for l in &placed.list {
        let Some((kind, release)) = TrapKind::of(&l.actor) else { continue };
        let Some(trig) = db.actors.get(&l.actor).and_then(|d| d.trigger.as_ref()) else { continue };
        let st = traps.0.entry(l.id).or_default();
        let box_t = l.transform * Transform::from_translation(Vec3::from_array(trig.t)).with_rotation(Quat::from_array(trig.r).normalize());
        let half = Vec3::from_array(trig.half);
        // Begin overlap: AI characters now in the box that weren't last frame.
        let now: HashSet<Entity> = zombies.iter().filter(|(_, t, b, _)| capsule_in_box(&box_t, half, t.translation, b.radius, b.height)).map(|(e, ..)| e).collect();
        let entered: Vec<Entity> = now.difference(&st.inside).copied().collect();
        st.inside = now;
        match kind {
            TrapKind::Snare => {
                for e in entered {
                    let Ok((_, t, b, mut h)) = zombies.get_mut(e) else { continue };
                    st.held.push(e);
                    commands.entity(e).insert(Trapped);
                    h.hp -= TRAP_DAMAGE;
                    no_credit.0.insert(e);
                    numbers.write(DamageNumber::blast(TRAP_DAMAGE, body(t.translation, b.height)));
                    st.release_in.get_or_insert(release);
                    info!("traps: {} caught a character", l.item);
                }
                if let Some(r) = st.release_in.as_mut() {
                    *r -= dt;
                    if *r <= 0.0 {
                        st.release_in = None;
                        // SetMovementMode(Walking) on all of AIArray (never emptied).
                        for &e in &st.held {
                            if let Ok(mut c) = commands.get_entity(e) {
                                c.remove::<Trapped>();
                            }
                        }
                    }
                }
            }
            TrapKind::Bear => {
                if st.jaws == Jaws::Armed
                    && let Some(&e) = entered.first()
                    && let Ok((_, mut t, b, mut h)) = zombies.get_mut(e)
                {
                    // Event_CatchCharacter: held, moved onto the box's X/Y (its own height kept).
                    t.translation = box_t.translation.with_y(t.translation.y);
                    commands.entity(e).insert(Trapped);
                    h.hp -= TRAP_DAMAGE;
                    no_credit.0.insert(e);
                    numbers.write(DamageNumber::blast(TRAP_DAMAGE, body(t.translation, b.height)));
                    st.caught = Some(e);
                    st.jaws = Jaws::Closing(0.0);
                    info!("traps: bear trap caught a character");
                }
                if st.jaws.step(dt)
                    && let Some(e) = st.caught.take()
                    && let Ok(mut c) = commands.get_entity(e)
                {
                    c.remove::<Trapped>();
                }
            }
            TrapKind::Mine => {
                if let Some(s) = st.exploded.as_mut() {
                    *s += dt;
                    if *s >= MINE_GONE {
                        gone.push(l.id);
                    }
                    continue;
                }
                let c = l.transform.translation + Vec3::Y * 0.1;
                if entered.is_empty() && !object_hit(c, &shots, &swings, &blasts) {
                    continue;
                }
                st.exploded = Some(0.0);
                let radius = db.actors.get(&l.actor).and_then(|d| d.blast_radius).unwrap_or(4.0);
                booms.push(MineBlast { at: c, radius });
            }
        }
    }
    // Event_Explode.
    for MineBlast { at: c, radius: r } in booms {
        fx.write(Fx::at("FX_Explosion_New", c).yawed(rng.gen_range(0.0..std::f32::consts::TAU)).scaled(super::throwables::random_scale(1.0, 2.0, &mut rng)));
        sound.write(crate::audio::Sound::at("explosion_large_06_Cue", c));
        noise.write(Noise { at: c, range: MINE_NOISE });
        commands.spawn((
            Transform::from_translation(c + Vec3::Y),
            PointLight { color: Color::srgb(1.0, 0.75, 0.4), intensity: 4_000_000.0, range: r * 2.5, ..default() },
            super::throwables::Flash(0.35),
            DespawnOnExit(AppState::InGame),
        ));
        let mut hits = 0;
        for (e, t, b, mut h) in zombies.iter_mut() {
            if super::throwables::reaches(c, r, t.translation, b) {
                let dmg = rng.gen_range(MINE_DAMAGE.0..=MINE_DAMAGE.1);
                h.hp -= dmg;
                no_credit.0.insert(e);
                numbers.write(DamageNumber::blast(dmg, body(t.translation, b.height)));
                hits += 1;
            }
        }
        let mut me = false;
        if let Ok((t, b)) = player.single()
            && super::throwables::reaches(c, r, t.translation, b)
        {
            let dmg = rng.gen_range(MINE_DAMAGE.0..=MINE_DAMAGE.1);
            let taken = super::survival::take_hit(&mut game, &item_db, &store, dmg);
            if let Some(s) = game.state.as_mut() {
                s.survival.health = (s.survival.health - taken).max(0.0);
            }
            numbers.write(DamageNumber::blast(dmg, body(t.translation, b.height)));
            me = true;
        }
        info!("traps: landmine exploded at {c:.1}, {hits} AI hurt, survivor hurt {me}");
    }
    // K2_DestroyActor: the mine's gone for good.
    if !gone.is_empty() {
        for id in gone {
            if let Some(i) = placed.list.iter().position(|l| l.id == id) {
                let l = placed.list.remove(i);
                commands.entity(l.entity).despawn();
                rays.set_dynamic(super::building::COLLIDE_KEY + l.id, Vec::new());
                traps.0.remove(&id);
            }
        }
        super::building::save_placed(&mut game, &placed);
    }
}

/// BearTrapTimeline's update: each jaw's pitch from its alpha.
fn close_jaws(traps: Res<Traps>, placed: Res<Buildables>, parents: Query<&ChildOf>, mut jaws: Query<(Entity, &Jaw, &mut Transform)>) {
    for (e, Jaw(yaw), mut t) in &mut jaws {
        // The jaw hangs off the trap's StaticMesh part, which hangs off the root.
        let root = parents.get(e).ok().and_then(|p| parents.get(p.parent()).ok()).map(|p| p.parent());
        let Some(l) = root.and_then(|r| placed.list.iter().find(|l| l.entity == r)) else { continue };
        let alpha = traps.0.get(&l.id).map_or(0.0, |s| s.jaws.alpha());
        t.rotation = ue_rotator(alpha * JAWS_SHUT, *yaw, 90.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_catch_capsules() {
        let t = Transform::from_translation(Vec3::new(0.0, 0.3, 0.0));
        let half = Vec3::new(0.4, 0.32, 0.4);
        assert!(capsule_in_box(&t, half, Vec3::ZERO, 0.35, 1.8));
        assert!(capsule_in_box(&t, half, Vec3::new(0.7, 0.0, 0.0), 0.35, 1.8), "the capsule's side reaches in");
        assert!(!capsule_in_box(&t, half, Vec3::new(0.8, 0.0, 0.0), 0.35, 1.8));
        assert!(!capsule_in_box(&t, half, Vec3::new(0.0, 1.5, 0.0), 0.35, 1.8), "above it");
    }

    #[test]
    fn the_bear_trap_closes_holds_and_rearms() {
        let mut j = Jaws::Closing(0.0);
        assert_eq!(j.alpha(), 0.0);
        j.step(0.1);
        assert!((j.alpha() - 0.5).abs() < 1e-5);
        j.step(0.1);
        assert_eq!(j, Jaws::Shut(0.0));
        assert!(!j.step(9.9));
        assert!(j.step(0.2), "let go 10 s after it shut");
        j.step(JAWS_SECS);
        assert!(matches!(j, Jaws::Rearming(_)) && j.alpha() == 0.0);
        j.step(BEAR_REARM);
        assert_eq!(j, Jaws::Armed);
    }

    #[test]
    fn jaws_match_the_parts() {
        let db = BuildableDb::load();
        let a = &db.actors["Buildable_BearTrap_C"];
        for (name, yaw) in [("SM_Prop_Bear_Trap_Jaw_01", 0.0), ("SM_Prop_Bear_Trap_Jaw_02", 180.0)] {
            let p = a.parts.iter().find(|p| p.name == name).unwrap();
            // Open (pitch 0) is how the construction script has them.
            assert!(ue_rotator(0.0, yaw, 90.0).angle_between(Quat::from_array(p.r).normalize()) < 1e-3, "{name}");
        }
        assert_eq!(TrapKind::of("Buildable_RazorWire_C"), Some((TrapKind::Snare, 3.0)));
        assert_eq!(a.trigger.as_ref().map(|t| t.half), Some([0.4, 0.32, 0.4]));
        assert_eq!(db.actors["Buildable_LandMine_C"].blast_radius, Some(4.0));
    }
}
