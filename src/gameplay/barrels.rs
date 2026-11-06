//! Explosive barrels: BP_ExplosiveBarrelMaster (the level's 11 BP_ExplosiveBarrel_Child),
//! decompiled.
//!
//! - Health: the construction script's 25-75, then BeginPlay's 25-50.
//! - Damage_Object takes it down: the survivor's rounds (their damage) and swings, and the frag,
//!   pipe bomb and Molotov fire (their DemolitionExpert multiplier, 1 or 1.5, to every object in
//!   10 / 8 / 5 m). Damage_Shoved does nothing.
//! - At 0, Event_Explode (once): FX_Explosion_New at a random yaw scaled 1-2 a side,
//!   explosion_large_06_Cue, a 50 m noise, the M_Burn decal (30 s), the camera shake; every
//!   character in its 8 m ExplosionSphere that's a living AI or the survivor ('Player' tag) takes
//!   75-125 with a white floating number; then the barrel is gone.
//!
//! Ours: the barrel's SM_Prop_Barrel_02 stays in the static cells until it blows (then it's
//! removed and forgotten by the collision); hits are a 0.5 m sphere at its middle; the decal,
//! the camera shake and RadialForce aren't drawn; the round's damage is the gun's FirearmDamage
//! (as the turrets take it), without fall-off or crits; destroyed barrels come back next session.

use bevy::prelude::*;
use rand::Rng;

use super::combat::{Dead, Health};
use super::damage_numbers::DamageNumber;
use super::particles::Fx;
use super::zombies::Noise;
use crate::level::{KinematicBody, LevelZombie, PropKey};
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, CurrentGame};
use crate::models::{CharacterModel, UpperAction};

/// ExplosionSphere's radius (m).
pub const RADIUS: f32 = 8.0;
/// ApplyDamage's roll.
pub const DAMAGE: (f32, f32) = (75.0, 125.0);
/// ReportNoiseEvent's MaxRange (m).
const NOISE: f32 = 50.0;
/// Ours: where a round or a swing counts as on the barrel (the barrel's middle, 0.5 m round).
const HIT_RADIUS: f32 = 0.5;
const MIDDLE: f32 = 0.5;

/// Damage_Object on every object within `radius` of `at` (the throwables).
#[derive(Message, Clone, Copy, Debug)]
pub struct ObjectBlast {
    pub at: Vec3,
    pub radius: f32,
    pub damage: f32,
}

#[derive(Debug, Clone)]
pub struct Barrel {
    pub at: Vec3,
    pub key: Option<PropKey>,
    pub health: f32,
    pub exploded: bool,
}

impl Barrel {
    /// Damage_Object: true when this hit sets it off.
    pub fn damage(&mut self, dmg: f32) -> bool {
        if self.exploded {
            return false;
        }
        self.health -= dmg;
        if self.health <= 0.0 {
            self.exploded = true;
            return true;
        }
        false
    }
}

#[derive(Resource, Default)]
pub struct Barrels {
    pub list: Vec<Barrel>,
    collected: bool,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Barrels>()
        .add_message::<ObjectBlast>()
        .add_systems(OnEnter(AppState::InGame), |mut b: ResMut<Barrels>| *b = Barrels::default())
        .add_systems(Update, (collect, take_damage).chain().run_if(in_state(AppState::InGame)));
}

fn collect(level: Option<Res<crate::level::Level>>, mut barrels: ResMut<Barrels>) {
    let Some(level) = level else { return };
    if barrels.collected {
        return;
    }
    barrels.collected = true;
    let mut rng = rand::thread_rng();
    for a in level.actors.iter().filter(|a| matches!(a.class.as_str(), "BP_ExplosiveBarrel_Child_C" | "BP_ExplosiveBarrelMaster_C")) {
        let at = a.position();
        let key = level.prop_near(at, "SM_Prop_Barrel_02", 0.5).map(|(k, _, _)| k);
        barrels.list.push(Barrel { at, key, health: rng.gen_range(25.0..=50.0), exploded: false });
    }
    info!("barrels: {}", barrels.list.len());
}

/// Whether a round from `origin` along `dir`, stopping `reach` m out, meets the barrel.
pub fn shot_hits(b: &Barrel, origin: Vec3, dir: Vec3, reach: f32) -> bool {
    let c = b.at + Vec3::Y * MIDDLE;
    let along = (c - origin).dot(dir);
    along > 0.0 && along <= reach + HIT_RADIUS && (origin + dir * along).distance(c) <= HIT_RADIUS
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn take_damage(
    mut commands: Commands,
    mut barrels: ResMut<Barrels>,
    (mut shots, mut swings, mut blasts): (MessageReader<super::turrets::Shot>, MessageReader<super::harvest::Swing>, MessageReader<ObjectBlast>),
    (db, store): (Res<super::ItemDb>, Res<SettingsStore>),
    mut game: ResMut<CurrentGame>,
    mut streaming: Option<ResMut<crate::level::PropStreaming>>,
    mut cache: ResMut<crate::level::collision::CollisionCache>,
    items: Query<(Entity, &crate::level::PropItem)>,
    mut zombies: Query<(&Transform, &KinematicBody, &mut Health, Option<&mut UpperAction>), (With<LevelZombie>, Without<Dead>)>,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    (mut fx, mut noise, mut numbers, mut sound): (MessageWriter<Fx>, MessageWriter<Noise>, MessageWriter<DamageNumber>, MessageWriter<crate::audio::Sound>),
) {
    let mut hits: Vec<(usize, f32)> = Vec::new();
    for s in shots.read() {
        hits.extend(barrels.list.iter().enumerate().filter(|(_, b)| !b.exploded && shot_hits(b, s.origin, s.dir, s.reach)).map(|(i, _)| (i, s.damage)));
    }
    for s in swings.read() {
        for (i, b) in barrels.list.iter().enumerate().filter(|(_, b)| !b.exploded) {
            let flat = (b.at - s.feet).with_y(0.0);
            if flat.length() <= 2.0 && flat.normalize_or_zero().dot(s.facing) > 0.5 {
                hits.push((i, s.damage));
            }
        }
    }
    for x in blasts.read() {
        hits.extend(barrels.list.iter().enumerate().filter(|(_, b)| !b.exploded && b.at.distance(x.at) <= x.radius).map(|(i, _)| (i, x.damage)));
    }
    let mut rng = rand::thread_rng();
    for (i, dmg) in hits {
        if !barrels.list[i].damage(dmg) {
            info!("barrel {i} hit for {dmg:.1}: {:.1} left", barrels.list[i].health);
            continue;
        }
        // Event_Explode.
        let b = barrels.list[i].clone();
        let at = b.at;
        fx.write(Fx::at("FX_Explosion_New", at).yawed(rng.gen_range(0.0..std::f32::consts::TAU)).scaled(super::throwables::random_scale(1.0, 2.0, &mut rng)));
        sound.write(crate::audio::Sound::at("explosion_large_06_Cue", at));
        noise.write(Noise { at, range: NOISE });
        commands.spawn((
            Transform::from_translation(at + Vec3::Y),
            PointLight { color: Color::srgb(1.0, 0.75, 0.4), intensity: 4_000_000.0, range: RADIUS * 2.5, ..default() },
            super::throwables::Flash(0.35),
            DespawnOnExit(AppState::InGame),
        ));
        let mut hit = super::throwables::hurt_zombies(at, RADIUS, DAMAGE, &mut zombies);
        if let Ok((t, body)) = player.single()
            && super::throwables::reaches(at, RADIUS, t.translation, body)
        {
            let dmg = rng.gen_range(DAMAGE.0..=DAMAGE.1);
            let taken = super::survival::take_hit(&mut game, &db, &store, dmg);
            if let Some(s) = game.state.as_mut() {
                s.survival.health = (s.survival.health - taken).max(0.0);
            }
            hit.push((dmg, super::damage_numbers::body(t.translation, body.height)));
        }
        // SpawnFloatingDamageText for each one damaged, white.
        numbers.write_batch(hit.iter().map(|&(damage, at)| DamageNumber::blast(damage, at)));
        if let Some(key) = b.key {
            if let Some(s) = streaming.as_mut() {
                s.remove(key);
            }
            cache.forget(key);
            for (e, p) in &items {
                if p.0 == key {
                    commands.entity(e).despawn();
                }
            }
        }
        info!("barrel {i} exploded at {at:.1}: {} hurt", hit.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barrels_take_hits_then_blow_once() {
        let mut b = Barrel { at: Vec3::ZERO, key: None, health: 30.0, exploded: false };
        assert!(!b.damage(1.5));
        assert!((b.health - 28.5).abs() < 1e-5);
        assert!(b.damage(30.0));
        assert!(!b.damage(30.0), "only once");
        // A round along X at the barrel's middle height, stopped at the barrel.
        let b = Barrel { at: Vec3::new(5.0, 0.0, 0.0), key: None, health: 30.0, exploded: false };
        assert!(shot_hits(&b, Vec3::new(0.0, 0.5, 0.0), Vec3::X, 4.6));
        assert!(!shot_hits(&b, Vec3::new(0.0, 0.5, 0.0), Vec3::X, 2.0), "stopped short");
        assert!(!shot_hits(&b, Vec3::new(0.0, 2.0, 0.0), Vec3::X, 10.0), "over it");
    }
}
