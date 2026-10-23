//! Swimming: SwimmingComponent (WaterDetection, WaterTrace, OxygenTimer, DealDamage), the water
//! bodies (BP_WaterBox / BP_WaterSpline) and BP_PlayerCharacter's swim input.
//!
//! - WaterTrace (every 0.01 s): a line from 164 cm above the feet down to the feet against the
//!   water surfaces; Deepness = 1 - distance / 164 (a start already under the surface: 1). At
//!   0.65 or more the survivor swims (OnRep_IsSwimming?: Swimming movement, burning stops, ADS
//!   ends, the weapon goes away); below it they walk again. On a ladder they never swim.
//! - Afloat, WaterDetection lowers the body towards the surface minus 30 cm (capsule centre: the
//!   feet 1.12 m under) at 250 cm/s; it never lifts it (FClamp to the current height).
//! - Moving: W/S along the camera's whole forward vector (looking down dives), A/D along its
//!   right, 300 cm/s; Shift = SwimFast, 500 x FitnessSwimmingMultiplier cm/s, a sprint for stamina.
//!   Under water Space (IA_FloatUpwards) rises along the camera's up vector.
//! - UnderWater? when Deepness is 1. OxygenTimer (0.1 s): under water oxygen drops 0.375 /
//!   FitnessSwimmingMultiplier, otherwise it refills 0.25 x the multiplier up to 100; DealDamage
//!   (every 0.8 s) does 4-8 while it's 0. The HUD's OxygenStat shows it (hud.rs).
//!
//! Ours: UE's swimming physics (BrakingDecelerationSwimming 100, buoyancy) isn't simulated: the
//! body moves at the input speed and stops without input; under water it stays put unless
//! Space or the camera's pitch moves it (neutral buoyancy, as CharMoveComp's default Buoyancy 1).
//! Not ported: radiated water (BP_WaterBox Radiated?: UpdateRadiation 0.15), the water sounds
//! (WaterAudioCheck), the camera's underwater effect, Fitness XP from swimming.

use bevy::prelude::*;
use rand::Rng;

use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::CharacterModel;

/// WaterTrace's length above the feet (m).
pub const TRACE: f32 = 1.64;
/// Deepness at which the survivor swims.
pub const SWIM_AT: f32 = 0.65;
/// Afloat: the feet this far under the surface (30 cm + the 82 cm capsule half-height).
pub const FLOAT: f32 = 0.30 + 0.82;
/// FInterpTo_Constant's 250 cm/s.
pub const SINK: f32 = 2.5;
pub const SWIM_SPEED: f32 = 3.0;
pub const SWIM_FAST: f32 = 5.0;
pub const MAX_OXYGEN: f32 = 100.0;

/// WaterTrace's Deepness for feet at `feet` and the water surface over them.
pub fn deepness(feet: f32, surface: Option<f32>) -> f32 {
    let Some(s) = surface.filter(|s| *s >= feet) else { return 0.0 };
    let start = feet + TRACE;
    if s >= start { 1.0 } else { 1.0 - (start - s) / TRACE }
}

/// OxygenTimer's 0.1 s step.
pub fn oxygen_step(oxygen: f32, under: bool, fitness_swimming: f32) -> f32 {
    let m = fitness_swimming.max(0.01);
    let next = if under { oxygen - 0.375 / m } else { oxygen + 0.25 * m };
    next.clamp(0.0, MAX_OXYGEN)
}

/// SwimmingComponent's state.
#[derive(Resource, Debug)]
pub struct Swim {
    pub swimming: bool,
    pub underwater: bool,
    pub deepness: f32,
    /// The water surface over the survivor, if any.
    pub surface: Option<f32>,
    pub oxygen: f32,
    oxygen_tick: f32,
    damage_tick: f32,
}

impl Default for Swim {
    fn default() -> Self {
        Self { swimming: false, underwater: false, deepness: 0.0, surface: None, oxygen: MAX_OXYGEN, oxygen_tick: 0.0, damage_tick: 0.0 }
    }
}

/// Run condition: not swimming (the BP's IsSwimming checks on shoves, throws, the roll...).
pub fn not_swimming(swim: Res<Swim>) -> bool {
    !swim.swimming
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Swim>()
        .add_systems(OnEnter(AppState::InGame), |mut s: ResMut<Swim>| *s = Swim::default())
        .add_systems(Update, (detect.before(crate::level::collision::move_bodies), oxygen).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

/// WaterDetection: the trace, swimming on and off, and the afloat height.
#[allow(clippy::type_complexity)]
fn detect(
    time: Res<Time>,
    mut swim: ResMut<Swim>,
    mut rays: crate::level::collision::PropRays,
    mut burning: ResMut<super::burning::PlayerBurning>,
    mut armed: ResMut<super::Armed>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<CharacterModel>>,
) {
    let Ok((mut t, mut body)) = player.single_mut() else { return };
    let feet = t.translation;
    if body.climbing {
        if swim.swimming {
            *swim = Swim { oxygen: swim.oxygen, ..default() };
            body.swimming = false;
        }
        return;
    }
    swim.surface = rays.water_surface(feet.x, feet.z, feet.y - 0.01, feet.y + TRACE + 30.0);
    swim.deepness = deepness(feet.y, swim.surface);
    let now = swim.deepness >= SWIM_AT;
    if now != swim.swimming {
        swim.swimming = now;
        body.swimming = now;
        info!("swimming {now} (deepness {:.2})", swim.deepness);
        if now {
            // OnRep_IsSwimming?: SetBurning(false), stop ADS, put the weapon away.
            burning.0 = None;
            armed.drawn = None;
            armed.aiming = false;
            body.vertical_velocity = 0.0;
        } else {
            body.vertical_velocity = 0.0;
        }
    }
    swim.underwater = swim.swimming && swim.deepness >= 1.0;
    // Afloat: lowered to the surface - 30 cm, never lifted.
    if swim.swimming
        && let Some(s) = swim.surface
    {
        let target = s - FLOAT;
        if t.translation.y > target {
            t.translation.y = (t.translation.y - SINK * time.delta_secs()).max(target);
        }
    }
}

/// OxygenTimer and DealDamage.
fn oxygen(
    time: Res<Time>,
    mut swim: ResMut<Swim>,
    mut game: ResMut<CurrentGame>,
    (db, store): (Res<super::ItemDb>, Res<crate::menus::settings::SettingsStore>),
    vitals: Res<super::PlayerVitals>,
) {
    let dt = time.delta_secs();
    let fitness = game.state.as_ref().map_or(1.0, |s| s.passive.fitness_swimming);
    swim.oxygen_tick += dt;
    while swim.oxygen_tick >= 0.1 {
        swim.oxygen_tick -= 0.1;
        swim.oxygen = oxygen_step(swim.oxygen, swim.underwater, fitness);
    }
    swim.damage_tick += dt;
    if swim.damage_tick < 0.8 {
        return;
    }
    swim.damage_tick -= 0.8;
    if swim.oxygen > 0.0 || vitals.is_dead() || game.state.is_none() {
        return;
    }
    let damage = rand::thread_rng().gen_range(4.0..=8.0);
    let taken = super::survival::take_hit(&mut game, &db, &store, damage);
    if let Some(s) = game.state.as_mut() {
        s.survival.health = (s.survival.health - taken).max(0.0);
    }
    info!("drowning: {taken:.1}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepness_is_the_traces() {
        assert_eq!(deepness(0.0, None), 0.0);
        // Water below the feet doesn't count.
        assert_eq!(deepness(1.0, Some(0.5)), 0.0);
        assert!((deepness(0.0, Some(0.82)) - 0.5).abs() < 1e-6);
        // Head under the surface: the trace starts inside the water.
        assert_eq!(deepness(0.0, Some(2.0)), 1.0);
        // Afloat (feet 1.12 m under) is deep enough to keep swimming.
        let afloat = deepness(-FLOAT, Some(0.0));
        assert!(afloat >= SWIM_AT && afloat < 1.0, "{afloat}");
        // Waist-deep (0.9 m) wades.
        assert!(deepness(0.0, Some(0.9)) < SWIM_AT);
    }

    #[test]
    fn oxygen_drains_under_water_and_refills_above() {
        let mut o = MAX_OXYGEN;
        let mut steps = 0;
        while o > 0.0 {
            o = oxygen_step(o, true, 1.0);
            steps += 1;
        }
        // 100 / 0.375 per 0.1 s: 26.7 s of air.
        assert_eq!(steps, 267);
        let mut up = 0;
        while o < MAX_OXYGEN {
            o = oxygen_step(o, false, 1.0);
            up += 1;
        }
        assert_eq!(up, 400);
        // Fitness's swimming multiplier stretches the air.
        assert!(oxygen_step(50.0, true, 1.5) > oxygen_step(50.0, true, 1.0));
    }
}
