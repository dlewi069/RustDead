//! The level's own traps (decompiled): 79 BP_Landmine and 75 BP_BarbedWire (47 of the base
//! class, 22 BP_BarbedWire_Child1, 6 BP_BarbedWire_Child). They're separate Blueprints from the
//! buildable traps (`traps.rs`).
//!
//! - **BP_Landmine**: ReceiveBeginPlay rolls RandomFloatInRange(0.1, 100) <= 50 and destroys
//!   itself, so about half of them exist in a session. Box (12 x 12 x 15 cm extent; overlaps Pawn,
//!   PhysicsBody and Vehicle; 14.3 cm up) beginning to overlap anything, or Damage_Object, runs
//!   Event_Explode once: the buildable landmine's own graph (FX_Explosion_New, explosion_large_06_Cue,
//!   the burn decal, the 15 m camera shake, 200-225 with a white number to every Character in the
//!   4 m ExplosionSphere that isn't a dead AI, the survivor included, a 50 m noise), then it's
//!   gone 0.1 s later. Unlike the buildable one it has no 'Player' tag check: the survivor sets
//!   it off too. Its StaticMesh ignores pawns: you walk onto it.
//! - **BP_BarbedWire**: Box (base 225 x 40 x 32 cm at (0, 8.4, 32), Child1 225 x 55 x 39.3 at 33 cm
//!   up, Child 50 x 70 x 45 at (8.5, 3, 43.8)) beginning to overlap a pawn: skipped if its
//!   VehicleDrivingComponent has a Vehicle (the survivor driving), else ApplyDamage 8-12 and a
//!   0.5 s looping timer (DamageTimer) of ApplyDamage 10 on the last actor that began
//!   overlapping; any end overlap clears the timer. No damage numbers, no bleeding. Its mesh
//!   has NoCollision: it's walked through, not over.
//! - Neither saves anything: a session's exploded mines come back after a reload.
//!
//! Ours: overlap = the capsule test of `traps::capsule_in_box`, per frame (begin = in now, not
//! last frame); the mine's vehicle overlap = the box centre inside the driven vehicle's hit
//! box; the component boxes are the Blueprints' templates (checked against the dump by a test).
//! Not ported: the decal, the camera shake, RadialForce's impulse.

use std::collections::HashSet;

use bevy::prelude::*;
use rand::Rng;

use super::combat::{Dead, Health};
use super::traps::{MineBlast, capsule_in_box};
use crate::level::{KinematicBody, LevelZombie};
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, CurrentGame};
use crate::models::CharacterModel;

/// BP_Landmine's ExplosionSphere (cm 400).
pub const MINE_RADIUS: f32 = 4.0;
/// ReceiveBeginPlay: destroyed when RandomFloatInRange(0.1, 100) <= this.
const MINE_REMOVED_AT: f32 = 50.0;
/// The Delay before K2_DestroyActor.
const MINE_GONE: f32 = 0.1;
/// BP_BarbedWire: the first ApplyDamage roll, the timer's damage and period.
pub const WIRE_FIRST: (f32, f32) = (8.0, 12.0);
pub const WIRE_TICK: f32 = 10.0;
pub const WIRE_PERIOD: f32 = 0.5;

/// A class's Box component: relative location and BoxExtent, UE cm.
pub fn trigger_box(class: &str) -> Option<([f32; 3], [f32; 3])> {
    Some(match class {
        "BP_Landmine_C" => ([1.5686027e-8, 0.0, 14.315797], [12.0, 12.0, 15.0]),
        "BP_BarbedWire_C" => ([9.429989e-9, 8.44373, 32.0], [225.0, 40.0, 32.0]),
        "BP_BarbedWire_Child1_C" => ([0.0, 0.0, 33.23923], [225.0, 55.0, 39.28391]),
        "BP_BarbedWire_Child_C" => ([8.5032425, 3.0, 43.777256], [50.0, 70.0, 45.0]),
        _ => return None,
    })
}

/// The box in the world: its transform (no scale) and half extents (m, the actor's scale in).
pub fn world_box(actor: &Transform, rel: [f32; 3], extent: [f32; 3]) -> (Transform, Vec3) {
    let ue = |v: [f32; 3]| Vec3::new(v[0], v[2], v[1]) / 100.0;
    let at = actor.transform_point(ue(rel));
    (Transform::from_translation(at).with_rotation(actor.rotation), ue(extent) * actor.scale.abs())
}

#[derive(Clone, Debug)]
pub struct Mine {
    pub box_t: Transform,
    pub half: Vec3,
    pub key: Option<crate::level::PropKey>,
    inside: HashSet<Entity>,
    /// Seconds since Event_Explode.
    pub exploded: Option<f32>,
}

/// One wire's DamageTimer and who was in its box last frame.
#[derive(Clone, Debug, Default)]
pub struct WireState {
    inside: HashSet<Entity>,
    /// The actor the timer hurts and the seconds towards its next tick.
    pub timer: Option<(Entity, f32)>,
}

impl WireState {
    /// One frame: `now` is who's in the box, `driving` whether an entity is in a vehicle.
    /// Returns the ApplyDamage calls.
    pub fn step(&mut self, now: HashSet<Entity>, driving: impl Fn(Entity) -> bool, dt: f32, rng: &mut impl Rng) -> Vec<(Entity, f32)> {
        let mut out = Vec::new();
        // The timer's ticks before anyone leaves this frame.
        if let Some((e, t)) = self.timer.as_mut() {
            *t += dt;
            while *t >= WIRE_PERIOD {
                *t -= WIRE_PERIOD;
                out.push((*e, WIRE_TICK));
            }
        }
        // Any end overlap clears the timer, whoever it was hurting.
        if self.inside.difference(&now).next().is_some() {
            self.timer = None;
        }
        let mut entered: Vec<Entity> = now.difference(&self.inside).copied().collect();
        entered.sort();
        for e in entered {
            if driving(e) {
                continue;
            }
            out.push((e, rng.gen_range(WIRE_FIRST.0..=WIRE_FIRST.1)));
            // K2_SetTimerDelegate on the same delegate restarts it on the newcomer.
            self.timer = Some((e, 0.0));
        }
        self.inside = now;
        out
    }
}

#[derive(Clone, Debug)]
pub struct Wire {
    pub box_t: Transform,
    pub half: Vec3,
    pub state: WireState,
}

#[derive(Resource, Default)]
pub struct LevelTraps {
    pub mines: Vec<Mine>,
    pub wires: Vec<Wire>,
    /// The wires' ApplyDamage on the survivor (before armor), this session.
    pub survivor_hits: Vec<f32>,
    collected: bool,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<LevelTraps>()
        .add_systems(OnEnter(AppState::InGame), |mut t: ResMut<LevelTraps>| *t = LevelTraps::default())
        .add_systems(Update, (collect, run_traps).chain().before(super::traps::TrapSet).run_if(in_state(AppState::InGame)));
}

/// BeginPlay: the mines that survive their roll (the rest leave the level) and every wire.
fn collect(
    mut commands: Commands,
    level: Option<Res<crate::level::Level>>,
    mut traps: ResMut<LevelTraps>,
    mut streaming: Option<ResMut<crate::level::PropStreaming>>,
    mut cache: ResMut<crate::level::collision::CollisionCache>,
    items: Query<(Entity, &crate::level::PropItem)>,
) {
    let Some(level) = level else { return };
    if traps.collected {
        return;
    }
    traps.collected = true;
    let mut rng = rand::thread_rng();
    let mut removed = 0;
    for a in &level.actors {
        let Some((rel, extent)) = trigger_box(&a.class) else { continue };
        let actor = crate::level::affine(&a.transform);
        let (box_t, half) = world_box(&actor, rel, extent);
        if a.class == "BP_Landmine_C" {
            let key = level.prop_near(a.position(), "SM_Prop_Mine_01", 0.5).map(|(k, _, _)| k);
            if rng.gen_range(0.1..=100.0) <= MINE_REMOVED_AT {
                if let Some(k) = key {
                    remove_prop(&mut commands, streaming.as_deref_mut(), &mut cache, &items, k);
                }
                removed += 1;
                continue;
            }
            traps.mines.push(Mine { box_t, half, key, inside: HashSet::new(), exploded: None });
        } else {
            traps.wires.push(Wire { box_t, half, state: WireState::default() });
        }
    }
    info!("level traps: {} mines ({removed} removed by their roll), {} wires", traps.mines.len(), traps.wires.len());
}

fn remove_prop(
    commands: &mut Commands,
    streaming: Option<&mut crate::level::PropStreaming>,
    cache: &mut crate::level::collision::CollisionCache,
    items: &Query<(Entity, &crate::level::PropItem)>,
    key: crate::level::PropKey,
) {
    if let Some(s) = streaming {
        s.remove(key);
    }
    cache.forget(key);
    for (e, p) in items {
        if p.0 == key {
            commands.entity(e).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run_traps(
    mut commands: Commands,
    time: Res<Time>,
    mut traps: ResMut<LevelTraps>,
    (item_db, store, vehicle_db, driving): (Res<super::ItemDb>, Res<SettingsStore>, Res<super::vehicles::VehicleDb>, Res<super::vehicles::Driving>),
    mut game: ResMut<CurrentGame>,
    mut zombies: Query<(Entity, &Transform, &KinematicBody, &mut Health), (With<LevelZombie>, Without<Dead>)>,
    player: Query<(Entity, &Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    vehicles: Query<(&Transform, &super::vehicles::Vehicle), Without<CharacterModel>>,
    (mut shots, mut swings, mut blasts): (MessageReader<super::turrets::Shot>, MessageReader<super::harvest::Swing>, MessageReader<super::barrels::ObjectBlast>),
    mut mine_blasts: MessageWriter<MineBlast>,
    mut no_credit: ResMut<super::turrets::NoCredit>,
    mut streaming: Option<ResMut<crate::level::PropStreaming>>,
    mut cache: ResMut<crate::level::collision::CollisionCache>,
    items: Query<(Entity, &crate::level::PropItem)>,
) {
    let dt = time.delta_secs();
    let shots: Vec<_> = shots.read().copied().collect();
    let swings: Vec<_> = swings.read().cloned().collect();
    let blasts: Vec<_> = blasts.read().copied().collect();
    let me = player.single().ok();
    let in_car = driving.vehicle.is_some();
    // Who's in a box: the AIs and the survivor (pawns).
    let pawns: Vec<(Entity, Vec3, f32, f32)> = zombies
        .iter()
        .map(|(e, t, b, _)| (e, t.translation, b.radius, b.height))
        .chain(me.map(|(e, t, b)| (e, t.translation, b.radius, b.height)))
        .collect();
    let in_box = |t: &Transform, half: Vec3| -> HashSet<Entity> { pawns.iter().filter(|(_, f, r, h)| capsule_in_box(t, half, *f, *r, *h)).map(|p| p.0).collect() };
    let car = driving.vehicle.and_then(|v| vehicles.get(v).ok()).and_then(|(t, v)| vehicle_db.classes.get(&v.class).map(|c| (t, c)));
    let mut rng = rand::thread_rng();
    let mut gone = Vec::new();
    for (i, m) in traps.mines.iter_mut().enumerate() {
        if let Some(s) = m.exploded.as_mut() {
            *s += dt;
            if *s >= MINE_GONE {
                gone.push(i);
            }
            continue;
        }
        let now = in_box(&m.box_t, m.half);
        let entered = now.difference(&m.inside).next().is_some();
        m.inside = now;
        let c = m.box_t.translation;
        let run_over = car.is_some_and(|(t, def)| super::vehicles::in_hit_box(def, t, c, m.half.max_element()));
        if entered || run_over || super::traps::object_hit(c, &shots, &swings, &blasts) {
            m.exploded = Some(0.0);
            mine_blasts.write(MineBlast { at: c, radius: MINE_RADIUS });
            info!("level traps: mine at {c:.1} set off (stepped on {entered}, run over {run_over})");
        }
    }
    for i in gone.into_iter().rev() {
        let m = traps.mines.remove(i);
        if let Some(k) = m.key {
            remove_prop(&mut commands, streaming.as_deref_mut(), &mut cache, &items, k);
        }
    }
    let me_e = me.map(|p| p.0);
    let mut hits = Vec::new();
    for w in &mut traps.wires {
        let now = in_box(&w.box_t, w.half);
        if now.is_empty() && w.state.inside.is_empty() && w.state.timer.is_none() {
            continue;
        }
        for (e, dmg) in w.state.step(now, |e| in_car && Some(e) == me_e, dt, &mut rng) {
            if Some(e) == me_e {
                // AnyDamage on the survivor (armor and Toughness through take_hit).
                let taken = super::survival::take_hit(&mut game, &item_db, &store, dmg);
                if let Some(s) = game.state.as_mut() {
                    s.survival.health = (s.survival.health - taken).max(0.0);
                }
                info!("level traps: barbed wire hurt the survivor {dmg:.1} ({taken:.1} taken)");
                hits.push(dmg);
            } else if let Ok((_, _, _, mut h)) = zombies.get_mut(e) {
                h.hp -= dmg;
                no_credit.0.insert(e);
            }
        }
    }
    traps.survivor_hits.extend(hits);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn wire_hurts_on_entry_then_every_half_second_until_someone_leaves() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        let (a, b) = (Entity::from_raw_u32(1).unwrap(), Entity::from_raw_u32(2).unwrap());
        let mut w = WireState::default();
        let first = w.step(HashSet::from([a]), |_| false, 0.0, &mut rng);
        assert_eq!(first.len(), 1);
        assert!(first[0].0 == a && (WIRE_FIRST.0..=WIRE_FIRST.1).contains(&first[0].1));
        assert!(w.step(HashSet::from([a]), |_| false, 0.4, &mut rng).is_empty());
        assert_eq!(w.step(HashSet::from([a]), |_| false, 0.2, &mut rng), vec![(a, WIRE_TICK)]);
        // B walks in: hurt, and the timer moves to B.
        let hits = w.step(HashSet::from([a, b]), |_| false, 0.0, &mut rng);
        assert!(hits.len() == 1 && hits[0].0 == b);
        assert_eq!(w.step(HashSet::from([a, b]), |_| false, 0.5, &mut rng), vec![(b, WIRE_TICK)]);
        // A leaves: the timer stops for B too.
        assert!(w.step(HashSet::from([b]), |_| false, 0.0, &mut rng).is_empty());
        assert!(w.timer.is_none() && w.step(HashSet::from([b]), |_| false, 2.0, &mut rng).is_empty());
        // Driving in does nothing.
        let mut w = WireState::default();
        assert!(w.step(HashSet::from([a]), |_| true, 0.0, &mut rng).is_empty() && w.timer.is_none());
    }

    #[test]
    fn boxes_sit_on_the_actor() {
        // An actor turned 90° (UE yaw), scaled 2: the Child's box offset turns and the extent grows.
        let actor = Transform::from_xyz(10.0, 0.0, 0.0).with_rotation(super::super::traps::ue_rotator(0.0, 90.0, 0.0)).with_scale(Vec3::splat(2.0));
        let (rel, ext) = trigger_box("BP_BarbedWire_Child_C").unwrap();
        let (t, half) = world_box(&actor, rel, ext);
        // UE X 8.5 cm * 2 -> along our +Z after the turn; 43.8 cm * 2 up.
        assert!((t.translation - Vec3::new(10.0 - 0.06, 0.8755, 0.17)).length() < 0.01, "{}", t.translation);
        assert!((half - Vec3::new(1.0, 0.9, 1.4)).length() < 1e-4);
        assert!(trigger_box("BP_Landmine_C").is_some() && trigger_box("BP_Zombie_C").is_none());
    }

    #[test]
    fn boxes_match_the_blueprints() {
        // target/bpdump (scripts/dump_bp.sh); skipped without it.
        let read = |p: &str| std::fs::read_to_string(format!("target/bpdump/{p}.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        for (class, path) in [
            ("BP_Landmine_C", "Blueprints/Other/Objects/BP_Landmine"),
            ("BP_BarbedWire_C", "Blueprints/Other/BarbedWire/BP_BarbedWire"),
            ("BP_BarbedWire_Child1_C", "Blueprints/Other/BarbedWire/BP_BarbedWire_Child1"),
            ("BP_BarbedWire_Child_C", "Blueprints/Other/BarbedWire/BP_BarbedWire_Child"),
        ] {
            let Some(d) = read(path) else { return };
            let b = d.as_array().unwrap().iter().find(|e| e["Name"] == "Box_GEN_VARIABLE").unwrap();
            let v = |k: &str, f: &str| b["Properties"][k][f].as_f64().unwrap_or(0.0) as f32;
            let (rel, ext) = trigger_box(class).unwrap();
            let near = |a: [f32; 3], k: &str| a.iter().zip(["X", "Y", "Z"]).all(|(x, f)| (x - v(k, f)).abs() < 1e-3);
            assert!(near(ext, "BoxExtent") && near(rel, "RelativeLocation"), "{class}: {rel:?} {ext:?}");
        }
    }

    #[test]
    fn level_has_the_traps() {
        let Ok(a) = std::fs::read_to_string(format!("{}/actors.json", crate::level::LEVEL_DIR)) else { return };
        let actors: Vec<crate::level::ActorInfo> = serde_json::from_str(&a).unwrap();
        let n = |c: &str| actors.iter().filter(|a| a.class == c).count();
        assert_eq!(n("BP_Landmine_C"), 79);
        assert_eq!(n("BP_BarbedWire_C") + n("BP_BarbedWire_Child_C") + n("BP_BarbedWire_Child1_C"), 75);
    }
}
