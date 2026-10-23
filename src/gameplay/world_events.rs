//! BP_EventManager's world events besides the airdrops (`airdrops.rs`) and the dynamic prefab
//! events (`level::prefabs`), decompiled:
//!
//! - **Jets** (Jet_Spawn): every 30-3600 s, MaxNumberOfJetsToSpawn (the level's 2) BP_Event_JetActor
//!   1-3 s apart, each at (the survivor's X x 750, Y x 750, 300-600 m) turned (yaw only) toward
//!   the survivor, flying its DirectionOfFlight at Speed 400 m/s (VInterpTo_Constant toward a
//!   point 100 km ahead, height kept) for its InitialLifeSpan of 60 s, with RandomJet_Cue (the
//!   Aircraft attenuation). As the bytecode reads, the spawn point scales the survivor's world
//!   position by 750: unless they stand within ~30 m of the world origin, the jet starts hundreds
//!   of km away and never comes near.
//! - **Explosions** (SpawnExplosion): every 300-3600 s, explosions_Cue at (X x r, Y x r, 20-30 m)
//!   with r = RandomFloatInRange(-20, 20): likewise far off unless near the origin.
//! - **The Blood Moon** (BloodMoonEvent, 2 s after BeginPlay, binds UDS Sunset and Sunrise): at
//!   each sunset a RandomFloatInRange(0, 100) <= BloodMoonChance (5) starts it: UDS Moon Light
//!   Color (1, 0.35, 0.35) and Moon Material Color, "The Blood Moon is upon us..." (Moon_Icon, red,
//!   4 s) and the BloodMoon delegate (true): every BP_MasterZombie alive (they bind at BeginPlay)
//!   gets Alert Speed x1.5, Attack Speed x1.75, DamageToDo x2, XP 15-25 and the blood moon eyes.
//!   The sunrise binding is added then and never removed, so every later sunrise sets the moon
//!   back, says "The Blood Moon is Settling down..." and resets the zombies (BloodMoon false), as
//!   the BP does.
//!
//! Ours: Attack Speed is taken as the swings' rate (their interval / 1.75); the eyes aren't drawn
//! (the zombies' glowing eyes aren't ported).

use bevy::prelude::*;
use rand::Rng;

use crate::level::LevelZombie;
use crate::menus::{AppState, CurrentGame};
use crate::models::CharacterModel;

/// The jets' and explosions' timers, the jets' 1-3 s apart.
const JET_EVERY: [f32; 2] = [30.0, 3600.0];
const JET_APART: [f32; 2] = [1.0, 3.0];
const EXPLOSION_EVERY: [f32; 2] = [300.0, 3600.0];
/// BP_Event_JetActor's Speed and InitialLifeSpan.
const JET_SPEED: f32 = 400.0;
const JET_LIFE: f32 = 60.0;
/// The spawn points' scaling of the survivor's position.
const JET_SCALE: f32 = 750.0;
const EXPLOSION_SCALE: f32 = 20.0;
/// BloodMoonChance (%) and the moon's colours.
pub const BLOOD_MOON_CHANCE: f32 = 5.0;
pub const BLOOD_MOON_LIGHT: Color = Color::linear_rgb(1.0, 0.348958, 0.348958);
/// BP_EventManager's own MaxNumberOfJetsToSpawn (the level instance sets 2).
const JETS_DEFAULT: u32 = 1;

/// Whether the Blood Moon is up (SetBloodMoon), for the moonlight and the zombies.
#[derive(Resource, Default, Debug)]
pub struct BloodMoon {
    pub on: bool,
    /// Event_StopBloodMoonSunrise is bound to Sunrise (after the first blood moon, for good).
    pub sunrise_bound: bool,
}

/// A zombie the BloodMoon delegate reached while it was alive.
#[derive(Component, Clone, Copy, Debug)]
pub struct BloodMoonBoost;

impl BloodMoonBoost {
    pub const ALERT_SPEED: f32 = 1.5;
    pub const ATTACK_SPEED: f32 = 1.75;
    pub const DAMAGE: f32 = 2.0;
    pub const XP: [f32; 2] = [15.0, 25.0];
}

#[derive(Resource)]
struct Timers {
    jet_in: f32,
    explosion_in: f32,
    /// Jets still to come in this flight, and the wait to the next.
    jets_left: u32,
    next_jet: f32,
    jets: u32,
    /// The sun was up last frame (for UDS Sunset / Sunrise).
    sun_up: Option<bool>,
}

impl Default for Timers {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        Self { jet_in: rng.gen_range(JET_EVERY[0]..JET_EVERY[1]), explosion_in: rng.gen_range(EXPLOSION_EVERY[0]..EXPLOSION_EVERY[1]), jets_left: 0, next_jet: 0.0, jets: JETS_DEFAULT, sun_up: None }
    }
}

#[derive(Component)]
pub struct Jet {
    pub age: f32,
    pub dir: Vec3,
}

/// Where Jet_Spawn puts a jet for a survivor at `p`: (X x 750, 300-600 m, Y x 750).
pub fn jet_spawn(p: Vec3, height: f32) -> Vec3 {
    Vec3::new(p.x * JET_SCALE, height, p.z * JET_SCALE)
}

/// SpawnExplosion's point: the survivor's X and Y times r (-20..20), 20-30 m up.
pub fn explosion_at(p: Vec3, r: f32, height: f32) -> Vec3 {
    Vec3::new(p.x * r, height, p.z * r)
}

/// The sun is over the horizon (`daynight::sun_pitch` runs 0..360 through the day and night).
pub fn sun_up(hour: f32) -> bool {
    super::daynight::sun_pitch(hour).to_radians().sin() > 0.0
}

/// UDS Sunset / Sunrise from the sun crossing the horizon: Some(true) at sunset, Some(false) at
/// sunrise.
pub fn sun_crossing(was_up: bool, up: bool) -> Option<bool> {
    match (was_up, up) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<BloodMoon>()
        .add_systems(OnEnter(AppState::InGame), |mut commands: Commands, mut b: ResMut<BloodMoon>| {
            commands.insert_resource(Timers::default());
            *b = BloodMoon::default();
        })
        .add_systems(Update, (read_instance, jets_and_explosions, fly_jets, blood_moon).chain().run_if(in_state(AppState::InGame)));
}

/// The level's BP_EventManager's MaxNumberOfJetsToSpawn.
fn read_instance(level: Option<Res<crate::level::Level>>, timers: Option<ResMut<Timers>>, mut done: Local<bool>) {
    let (Some(level), Some(mut timers)) = (level, timers) else { return };
    if *done {
        return;
    }
    *done = true;
    if let Some(n) = level.actors.iter().find(|a| a.class == "BP_EventManager_C").and_then(|a| a.props.as_ref()?.get("MaxNumberOfJetsToSpawn")?.parse().ok()) {
        timers.jets = n;
    }
}

fn jets_and_explosions(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    timers: Option<ResMut<Timers>>,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
    mut sound: MessageWriter<crate::audio::Sound>,
) {
    let (Some(mut timers), Ok(me)) = (timers, player.single()) else { return };
    let p = me.translation;
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    timers.jet_in -= dt;
    if timers.jet_in <= 0.0 {
        timers.jet_in = rng.gen_range(JET_EVERY[0]..JET_EVERY[1]);
        timers.jets_left = timers.jets;
        timers.next_jet = 0.0;
    }
    if timers.jets_left > 0 {
        timers.next_jet -= dt;
        if timers.next_jet <= 0.0 {
            timers.jets_left -= 1;
            timers.next_jet = rng.gen_range(JET_APART[0]..JET_APART[1]);
            let at = jet_spawn(p, rng.gen_range(300.0..600.0));
            let dir = (p - at).with_y(0.0).normalize_or(Vec3::X);
            // The JetMesh faces UE +X: its local +X along the flight.
            let rot = Quat::from_rotation_arc(Vec3::X, dir);
            let e = commands
                .spawn((crate::models::model(&assets, "game/SurrounDead/Content/Meshes/World/JetMesh.glb"), Transform::from_translation(at).with_rotation(rot).with_scale(Vec3::splat(2.0)), Jet { age: 0.0, dir }, DespawnOnExit(AppState::InGame)))
                .id();
            // Its AudioComponent: RandomJet_Cue through the Aircraft attenuation, following it.
            sound.write(crate::audio::Sound { cue: "RandomJet_Cue".into(), at: Some(at), volume: 1.0, attenuation: Some("Aircraft".into()), follow: Some(e) });
            info!("jet spawned at {at:.0} ({:.0} km off), flying {dir:.2}", at.distance(p) / 1000.0);
        }
    }
    timers.explosion_in -= dt;
    if timers.explosion_in <= 0.0 {
        timers.explosion_in = rng.gen_range(EXPLOSION_EVERY[0]..EXPLOSION_EVERY[1]);
        let at = explosion_at(p, rng.gen_range(-EXPLOSION_SCALE..EXPLOSION_SCALE), rng.gen_range(20.0..30.0));
        sound.write(crate::audio::Sound::at("explosions_Cue", at));
        info!("distant explosion at {at:.0} ({:.1} km off)", at.distance(p) / 1000.0);
    }
}

fn fly_jets(mut commands: Commands, time: Res<Time>, mut jets: Query<(Entity, &mut Jet, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut j, mut t) in &mut jets {
        j.age += dt;
        if j.age >= JET_LIFE {
            commands.entity(e).despawn();
            continue;
        }
        t.translation += j.dir * JET_SPEED * dt;
    }
}

#[allow(clippy::too_many_arguments)]
fn blood_moon(
    mut commands: Commands,
    game: Res<CurrentGame>,
    timers: Option<ResMut<Timers>>,
    mut moon: ResMut<BloodMoon>,
    mut notify: MessageWriter<super::notify::Notify>,
    zombies: Query<(Entity, &LevelZombie), Without<super::combat::Dead>>,
    boosted: Query<Entity, With<BloodMoonBoost>>,
) {
    let (Some(mut timers), Some(state)) = (timers, game.state.as_ref()) else { return };
    let up = sun_up(state.environment.time_of_day_hours);
    let was = timers.sun_up.replace(up);
    let Some(crossing) = was.and_then(|w| sun_crossing(w, up)) else { return };
    if crossing {
        // Event_StartBloodMoon.
        if rand::thread_rng().gen_range(0.0..100.0) <= BLOOD_MOON_CHANCE {
            moon.on = true;
            moon.sunrise_bound = true;
            notify.write(super::notify::Notify { text: "The Blood Moon is upon us...".into(), secs: 4.0, color: Color::WHITE });
            let mut n = 0;
            for (e, z) in &zombies {
                if z.kind.animal().is_none() && z.kind.human().is_none() {
                    commands.entity(e).insert(BloodMoonBoost);
                    n += 1;
                }
            }
            info!("blood moon: {n} zombies roused");
        }
    } else if moon.sunrise_bound {
        // Event_StopBloodMoonSunrise (bound for good once a blood moon has come).
        moon.on = false;
        notify.write(super::notify::Notify { text: "The Blood Moon is Settling down...".into(), secs: 4.0, color: Color::WHITE });
        for e in &boosted {
            commands.entity(e).remove::<BloodMoonBoost>();
        }
        info!("blood moon over");
    }
}

/// Start the blood moon now (the screenshot tour), as a sunset's roll coming up would.
pub fn force_blood_moon(world: &mut World) {
    let zombies: Vec<Entity> = world
        .query_filtered::<(Entity, &LevelZombie), Without<super::combat::Dead>>()
        .iter(world)
        .filter(|(_, z)| z.kind.animal().is_none() && z.kind.human().is_none())
        .map(|(e, _)| e)
        .collect();
    for e in &zombies {
        world.entity_mut(*e).insert(BloodMoonBoost);
    }
    let mut moon = world.resource_mut::<BloodMoon>();
    moon.on = true;
    moon.sunrise_bound = true;
    world.write_message(super::notify::Notify { text: "The Blood Moon is upon us...".into(), secs: 4.0, color: Color::WHITE });
    info!("blood moon (forced): {} zombies roused", zombies.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jets_and_explosions_start_where_the_bp_puts_them() {
        // The survivor's position scaled by 750: near the origin the jet starts close by.
        assert_eq!(jet_spawn(Vec3::new(0.02, 10.0, -0.04), 450.0), Vec3::new(15.0, 450.0, -30.0));
        let far = jet_spawn(Vec3::new(782.0, 12.0, 752.0), 450.0);
        assert!(far.xz().length() > 500_000.0, "{far}");
        // 60 s at 400 m/s: 24 km of flight.
        assert_eq!(JET_SPEED * JET_LIFE, 24_000.0);
        assert_eq!(explosion_at(Vec3::new(10.0, 0.0, 20.0), -2.0, 25.0), Vec3::new(-20.0, 25.0, -40.0));
    }

    #[test]
    fn sunset_and_sunrise_are_the_sun_crossing_the_horizon() {
        assert_eq!(sun_crossing(true, false), Some(true));
        assert_eq!(sun_crossing(false, true), Some(false));
        assert_eq!(sun_crossing(true, true), None);
        // The sun is down at night, up at noon.
        assert!(sun_up(12.0) && !sun_up(2.0) && !sun_up(22.0));
    }
}
