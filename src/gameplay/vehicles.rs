//! Vehicles: BP_VehicleSpawner / BP_VehicleUniqueSpawner, BP_VehicleMaster and its 17 children
//! (`assets/data/vehicles.json`, `extractor/gen_vehicles.py`), VehicleDrivingComponent,
//! VehicleFuelComponent, VehicleHealthComponent and VehicleUI.
//!
//! - **Spawning** (BP_VehicleSpawner.Svr_Spawn, once a session here; the game saves SpawnerUsed?
//!   and the vehicles): RandomChance = RandomFloatInRange(0, 100) <= Difficulty.VehicleChance (30 %
//!   by default), and only while there are at most 30 vehicles; then a random class from the
//!   spawner's DT_VehicleInfo row (VehicleSelection: Civilian, Military, EmergencyServices,
//!   Industrial, Apocalypse) at the spawner's transform. BP_VehicleUniqueSpawner always spawns its
//!   `Vehicle` class. BeginPlay puts a random one of the class's Colors on material slot 0.
//! - **Getting in** (F, Interact_GetInVehicle): the survivor sits at PlayerLocation (30 cm lower),
//!   A_Car_IdleShortened looping, weapons away; VehicleUI shows; fuel starts burning
//!   (StartFuelSpend). **Getting out** (F, ExitVehicle): not at 10 mph or more
//!   (CheckCanExitCarDriverSide); at ExitPoint_DriverSide, or the passenger side when a 25 cm
//!   sphere there is blocked.
//! - **Driving** (VehicleDrivingComponent): W throttle, S brake (reverse once stopped: Chaos's
//!   automatic reverse), A/D steer, Space handbrake, H horn (Cue_Horn). No throttle without fuel,
//!   with the engine off or the vehicle wrecked (health 0). Every 2 s a 25 m noise (Event_Noise).
//! - **The physics** stand in for UE's ChaosWheeledVehicleMovementComponent with the class's own
//!   values: engine torque = MaxTorque × TorqueCurve(RPM) / the curve's peak (Chaos normalises the
//!   curve), RPM from the driven wheels through the gear and FinalRatio, clamped to idle..MaxRPM,
//!   no drive at MaxRPM; automatic gears up at ChangeUpRPM, down at ChangeDownRPM, GearChangeTime
//!   without drive (Chaos's 4500 / 2000 / 0.4 s defaults: a class whose MaxRPM is under 4500 never
//!   leaves first gear, as in the game); brakes from the wheels' MaxBrakeTorque /
//!   MaxHandBrakeTorque; drag 0.5 × 1.225 × DragCoefficient × 1.8 × 1.4 m² (Chaos's default
//!   chassis) × v². Ours: no tyre slip (a bicycle model turning at the front wheels' MaxSteerAngle),
//!   rolling resistance 0.015 g, the engine braking guess below, input rates 5 /s, and the body
//!   follows the ground under its wheels (pitch and roll) instead of simulating suspension.
//! - **Hits**: the mesh hitting the world plays VehicleCollision_Cue at 5 mph or more (VehicleHitAudio,
//!   0.25 s apart); it does no damage (ApplyDamageToVehicleBasedOnSpeed is never called). Ours: a
//!   hit stops the vehicle. The HitBoxCollision overlapping a living zombie or animal at 5 mph or
//!   more: ApplyDamageToAIBasedOnSpeed, 300-350 to it and DmgOnVehicleCollision (5-8) to the
//!   vehicle, which ReceiveAnyDamage takes as damage × DAMAGE DEPLETION MULTIPLIER ÷ 2.5.
//! - **Fuel** (FuelSpend every WasteTime): WasteOfFuelPerTime × FUEL DEPLETION MULTIPLIER, a tenth of
//!   it under 2 mph; empty stops the throttle. A Gas Can at the FuelBoxCollision (UseGasCan) adds
//!   half the tank ("No gas found" without one); ours on F, the game's after its interact timer.
//! - Parked vehicles are solid (their hit box joins the collision's dynamic sets).
//!
//! Not ported yet: storage, push, tow, flip, lights, radio, the engine sound, repairing, water,
//! smoke/fire, the driver's damage when it's wrecked, saving vehicles.

use std::collections::HashMap;

use bevy::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::interact::{GameOverlay, Prompt, PromptBar, PromptText, player_inv};
use super::notify::Notify;
use crate::level::{KinematicBody, Level, LevelZombie, Terrain};
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{CharacterModel, Locomotion, MaterialRename, model};

/// m/s to mph (GetForwardSpeedMPH).
pub const MPH: f32 = 2.236_94;
/// Spawners stop once there are more than this many vehicles (LessEqual_IntInt(count, 30)).
const MAX_VEHICLES: usize = 30;
/// Dynamic collision keys for parked vehicles.
const COLLIDE_KEY: usize = 1 << 42;
/// Chaos's default chassis (ChassisWidth 180, ChassisHeight 140 cm) for the drag area, and air.
const DRAG_AREA: f32 = 1.8 * 1.4;
const AIR: f32 = 1.225;
/// Ours: rolling resistance as a fraction of the weight.
const ROLLING: f32 = 0.015;
/// Ours: how fast throttle, brake and steering follow the keys (per second).
const INPUT_RATE: f32 = 5.0;
/// Ours: how near the hit box F reaches.
const REACH: f32 = 1.2;
/// The camera's arm while driving (ours: the vehicle's view isn't read).
pub const VEHICLE_ARM: f32 = 9.0;
/// Vehicle storages' container keys: under the traders' range, past any corpse.
pub const STORAGE_KEYS: usize = super::interact::CORPSE_KEYS + (1 << 31);
/// TurnOnOffLights: SetIntensity(50000) (unitless: candela x 625, as `level::lights`).
const LIGHT_INTENSITY: f32 = 50000.0;
/// Event_FlipVehicle: up 100 cm, then 1 s before another flip.
const FLIP_LIFT: f32 = 1.0;
const FLIP_WAIT: f32 = 1.0;
/// The engine's AudioComponent sound.
const ENGINE_CUE: &str = "MasterVehicle_Audio";
/// Event_Noise: every 2 s, 2500 cm.
const NOISE_EVERY: f32 = 2.0;
const NOISE_RANGE: f32 = 25.0;
/// A step the wheels climb (higher ground ahead is a wall); ours.
const CLIMB: f32 = 0.6;

#[derive(Deserialize, Clone, Debug)]
pub struct Engine {
    pub max_rpm: f32,
    pub max_torque: f32,
    pub idle_rpm: f32,
    pub brake_effect: f32,
    pub torque_curve: Vec<[f32; 2]>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Transmission {
    #[serde(rename = "final")]
    pub final_ratio: f32,
    pub forward: Vec<f32>,
    pub reverse: Vec<f32>,
    pub change_up: f32,
    pub change_down: f32,
    pub change_time: f32,
    pub efficiency: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Wheel {
    pub bone: String,
    /// The wheel's centre in the mesh (glb space, m).
    pub at: Option<[f32; 3]>,
    pub front: bool,
    /// cm.
    pub radius: f32,
    pub max_steer: f32,
    pub steering: bool,
    pub engine: bool,
    pub handbrake: bool,
    pub brake_torque: f32,
    pub handbrake_torque: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct HitBox {
    pub at: [f32; 3],
    pub half: [f32; 3],
}

/// A headlight (SpotLightComponent): UE offset, cone half-angles (degrees), AttenuationRadius (cm).
#[derive(Deserialize, Clone, Debug)]
pub struct LightDef {
    pub at: [f32; 3],
    pub inner: f32,
    pub outer: f32,
    pub radius: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Fuel {
    pub current: f32,
    pub max: f32,
    pub waste_time: f32,
    pub waste: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct HealthDef {
    pub current: f32,
    pub max: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VehicleClass {
    pub name: String,
    pub mesh: String,
    #[serde(default)]
    pub extra_meshes: Vec<String>,
    #[serde(default)]
    pub colors: Vec<String>,
    pub slot0: Option<String>,
    pub mass: f32,
    pub drag: f32,
    pub engine: Engine,
    pub transmission: Transmission,
    pub wheels: Vec<Wheel>,
    /// UE units (cm) relative to the actor.
    pub player: [f32; 3],
    pub exit_driver: [f32; 3],
    pub exit_passenger: [f32; 3],
    pub fuel_box: [f32; 3],
    pub hit_box: HitBox,
    #[serde(default)]
    pub lights: Vec<LightDef>,
    /// The storage grid (BP_JigMultiplayer's container): columns, rows.
    pub storage: [u8; 2],
    pub fuel: Fuel,
    pub health: HealthDef,
    pub collision_damage: [f32; 2],
}

#[derive(Deserialize, Clone, Debug)]
pub struct SpawnRow {
    pub row: String,
    pub classes: Vec<String>,
}

#[derive(Resource, Deserialize, Default)]
pub struct VehicleDb {
    pub classes: HashMap<String, VehicleClass>,
    /// Enum_VehicleSpawner value -> its DT_VehicleInfo row.
    pub spawners: HashMap<String, SpawnRow>,
}

impl VehicleDb {
    pub fn load() -> Self {
        std::fs::read_to_string("assets/data/vehicles.json").ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
}

/// A UE offset (cm, X forward, Y right, Z up) in the vehicle's frame (m, +X forward, +Y up, +Z right).
pub fn ue(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[2], v[1]) / 100.0
}

/// Enum_VehicleSpawner's entry names (as the spawners' VehicleSelection) -> value.
fn selection(name: &str) -> u8 {
    match name.trim_matches('"').rsplit("::").next() {
        Some("NewEnumerator1") => 1,
        Some("NewEnumerator2") => 2,
        Some("NewEnumerator6") => 3,
        Some("NewEnumerator3") => 4,
        _ => 0,
    }
}

impl VehicleClass {
    /// The driven wheels' radius (m).
    pub fn drive_radius(&self) -> f32 {
        self.wheels.iter().find(|w| w.engine).or(self.wheels.first()).map_or(0.35, |w| w.radius / 100.0)
    }

    /// Front axle to rear axle (m).
    pub fn wheelbase(&self) -> f32 {
        let xs: Vec<(bool, f32)> = self.wheels.iter().filter_map(|w| Some((w.front, w.at?[0]))).collect();
        let avg = |front: bool| {
            let v: Vec<f32> = xs.iter().filter(|x| x.0 == front).map(|x| x.1).collect();
            if v.is_empty() { 0.0 } else { v.iter().sum::<f32>() / v.len() as f32 }
        };
        (avg(true) - avg(false)).abs().max(1.0)
    }

    /// The front steering wheels' MaxSteerAngle (degrees).
    pub fn max_steer(&self) -> f32 {
        self.wheels.iter().find(|w| w.steering).map_or(0.0, |w| w.max_steer)
    }
}

/// The engine's torque at `rpm` (Nm): MaxTorque × the curve there ÷ the curve's peak (Chaos's
/// FVehicleEngineConfig normalises the curve); linear between keys, flat past the ends. A class
/// without a curve has its MaxTorque everywhere.
pub fn torque_at(e: &Engine, rpm: f32) -> f32 {
    let c = &e.torque_curve;
    let peak = c.iter().map(|k| k[1]).fold(0.0f32, f32::max);
    if c.is_empty() || peak <= 0.0 {
        return e.max_torque;
    }
    let v = if rpm <= c[0][0] {
        c[0][1]
    } else if rpm >= c[c.len() - 1][0] {
        c[c.len() - 1][1]
    } else {
        let i = c.windows(2).position(|w| rpm >= w[0][0] && rpm <= w[1][0]).unwrap_or(0);
        let (a, b) = (c[i], c[i + 1]);
        a[1] + (b[1] - a[1]) * (rpm - a[0]) / (b[0] - a[0]).max(1e-6)
    };
    e.max_torque * (v / peak).max(0.0)
}

/// The drivetrain's state between frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    /// Forward speed, m/s (negative: backwards).
    pub speed: f32,
    pub rpm: f32,
    /// -1 reverse, 1.. forward gears.
    pub gear: i32,
    /// Seconds left of a gear change (no drive).
    pub shifting: f32,
}

impl Default for Drive {
    fn default() -> Self {
        Self { speed: 0.0, rpm: 0.0, gear: 1, shifting: 0.0 }
    }
}

/// One step of the drivetrain: `forward` / `back` are the W / S inputs (0..1), `power` false
/// without fuel or engine (no throttle).
pub fn step_drive(c: &VehicleClass, d: &mut Drive, forward: f32, back: f32, handbrake: bool, power: bool, dt: f32) {
    // Chaos's automatic reverse: braking to a stop then holding S engages reverse; W out of it.
    if d.gear < 0 {
        if forward > 0.0 && d.speed > -0.5 {
            d.gear = 1;
        }
    } else if back > 0.0 && d.speed < 0.5 {
        d.gear = -1;
    }
    let (throttle, brake) = if d.gear < 0 { (back, forward) } else { (forward, back) };
    let throttle = if power { throttle } else { 0.0 };
    let t = &c.transmission;
    let ratio = if d.gear < 0 { t.reverse.first().copied().unwrap_or(3.0) } else { t.forward.get((d.gear - 1) as usize).copied().unwrap_or(1.0) };
    let total = ratio * t.final_ratio;
    let r = c.drive_radius();
    let raw = d.speed.abs() / r * total * 60.0 / std::f32::consts::TAU;
    d.rpm = raw.clamp(c.engine.idle_rpm, c.engine.max_rpm);
    d.shifting = (d.shifting - dt).max(0.0);
    let limiter = raw >= c.engine.max_rpm - 1.0;
    let drive = if d.shifting > 0.0 || limiter { 0.0 } else { throttle * torque_at(&c.engine, d.rpm) * total * t.efficiency / r };
    let dir = if d.gear < 0 { -1.0 } else { 1.0 };
    d.speed += dir * drive / c.mass * dt;
    // Everything that slows it: brakes, the handbrake, drag, rolling, and (ours) engine braking
    // off the throttle: EngineBrakeEffect × MaxTorque through the gears.
    let brakes: f32 = c.wheels.iter().map(|w| brake * w.brake_torque / (w.radius / 100.0)).sum::<f32>()
        + if handbrake { c.wheels.iter().filter(|w| w.handbrake).map(|w| w.handbrake_torque / (w.radius / 100.0)).sum::<f32>() } else { 0.0 };
    let engine_brake = (1.0 - throttle) * c.engine.brake_effect * c.engine.max_torque * total / r * 0.1;
    let resist = brakes + 0.5 * AIR * c.drag * DRAG_AREA * d.speed * d.speed + ROLLING * c.mass * 9.81 + engine_brake;
    let dv = resist / c.mass * dt;
    d.speed = if d.speed.abs() <= dv { if drive > 0.0 { d.speed } else { 0.0 } } else { d.speed - d.speed.signum() * dv };
    // The automatic gearbox.
    if d.gear > 0 && d.shifting <= 0.0 {
        if d.rpm >= t.change_up && (d.gear as usize) < t.forward.len() {
            d.gear += 1;
            d.shifting = t.change_time;
        } else if d.rpm <= t.change_down && d.gear > 1 {
            d.gear -= 1;
            d.shifting = t.change_time;
        }
    }
}

/// FuelSpend's waste per second: WasteOfFuelPerTime per WasteTime × the multiplier, a tenth under 2 mph.
pub fn fuel_rate(f: &Fuel, multiplier: f32, mph: f32) -> f32 {
    f.waste / f.waste_time.max(1e-4) * multiplier * if mph.abs() >= 2.0 { 1.0 } else { 0.1 }
}

/// ApplyDamageToVehicleBasedOnSpeed's tiers are dead code; VehicleHitAudio needs 5 mph.
pub fn hit_sounds(mph: f32) -> bool {
    mph.abs() >= 5.0
}

/// CheckCanExitCarDriverSide: under 10 mph (FTrunc(|mph|) < 10).
pub fn can_exit(mph: f32) -> bool {
    (mph.abs().trunc() as i32) < 10
}

/// UseGasCan: half the tank, clamped.
pub fn refuel(fuel: f32, max: f32) -> f32 {
    (fuel + max / 2.0).clamp(0.0, max)
}

/// A vehicle in the world.
#[derive(Component)]
pub struct Vehicle {
    pub class: String,
    pub drive: Drive,
    /// Smoothed inputs.
    pub steer: f32,
    forward: f32,
    back: f32,
    pub fuel: f32,
    pub health: f32,
    pub engine_on: bool,
    /// Wheel roll (radians).
    spin: f32,
    /// Falling speed when off the ground.
    vy: f32,
    pub id: usize,
    hit_cool: f32,
    noise: f32,
    /// The Colors entry on slot 0 (saved).
    pub color: Option<String>,
    /// LightsOn?.
    pub lights_on: bool,
    /// Seconds left before another flip (Flipped).
    flipped: f32,
    /// The wheel bones once found: entity, bind local transform, parent's bind rotation in the
    /// vehicle's frame, steers.
    wheels: Option<Vec<(Entity, Transform, Quat, bool)>>,
}

impl Vehicle {
    pub fn mph(&self) -> f32 {
        self.drive.speed * MPH
    }

    /// Its storage's container key.
    pub fn storage_key(&self) -> usize {
        STORAGE_KEYS + self.id
    }
}

/// The world's vehicles in the save (the game saves the spawned vehicles and its spawners'
/// SpawnerUsed?; ours: `None` until the spawners have rolled once for this game).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct VehicleSave {
    pub list: Vec<SavedVehicle>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SavedVehicle {
    pub class: String,
    pub at: [f32; 3],
    pub yaw: f32,
    pub fuel: f32,
    pub health: f32,
    pub color: Option<String>,
    #[serde(default)]
    pub lights: bool,
    #[serde(default)]
    pub storage: Option<super::items::Grid>,
}

/// A headlight spotlight.
#[derive(Component)]
struct HeadLight;

/// One of the engine sound's crossfade inputs (its index), on a child of the vehicle.
#[derive(Component)]
struct EngineLoop(usize);

/// The survivor's vehicle (VehicleDrivingComponent.Vehicle) and per-session spawning.
#[derive(Resource, Default)]
pub struct Driving {
    pub vehicle: Option<Entity>,
    spawned: bool,
    /// Vehicles' hit boxes (for the F reach), refreshed each frame.
    pub count: usize,
}

impl Driving {
    pub fn active(&self) -> bool {
        self.vehicle.is_some()
    }
}

/// Run condition: on foot (not driving).
pub fn on_foot(driving: Res<Driving>) -> bool {
    !driving.active()
}

/// The vehicle systems (zombies run over die after them).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct VehicleSet;

#[derive(Component)]
struct VehicleUi;
#[derive(Component)]
struct VehicleName;
#[derive(Component)]
struct HealthBar;
#[derive(Component)]
struct FuelBar;
#[derive(Component)]
struct SpeedText;

pub fn plugin(app: &mut App) {
    app.insert_resource(VehicleDb::load())
        .init_resource::<Driving>()
        .add_systems(OnEnter(AppState::InGame), (|mut d: ResMut<Driving>| *d = Driving::default(), spawn_ui))
        .add_systems(
            Update,
            (spawn_vehicles, interact.after(super::harvest::salvage), drive.before(crate::level::collision::move_bodies), seat, find_wheels, turn_wheels, hit_zombies, update_ui, engine_sound, headlights, save_vehicles)
                .chain()
                .in_set(VehicleSet)
                .run_if(in_state(AppState::InGame)),
        );
}

/// The vehicle's hit box as triangles in the world (the parked vehicle's collision).
fn box_tris(c: &VehicleClass, t: &Transform) -> Vec<[Vec3; 3]> {
    let centre = ue(c.hit_box.at);
    let h = ue(c.hit_box.half).abs();
    let corner = |i: usize| t.transform_point(centre + Vec3::new(if i & 1 == 0 { -h.x } else { h.x }, if i & 2 == 0 { -h.y } else { h.y }, if i & 4 == 0 { -h.z } else { h.z }));
    let faces = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]];
    faces.iter().flat_map(|f| [[corner(f[0]), corner(f[1]), corner(f[2])], [corner(f[0]), corner(f[2]), corner(f[3])]]).collect()
}

/// Whether `p` (world) is within `margin` of the hit box.
pub fn in_hit_box(c: &VehicleClass, t: &Transform, p: Vec3, margin: f32) -> bool {
    let local = t.compute_affine().inverse().transform_point3(p) - ue(c.hit_box.at);
    let h = ue(c.hit_box.half).abs() + Vec3::splat(margin);
    local.x.abs() <= h.x && local.y.abs() <= h.y && local.z.abs() <= h.z
}

/// A vehicle as spawned or loaded.
struct Spawn {
    class: String,
    t: Transform,
    color: Option<String>,
    fuel: f32,
    health: f32,
    lights: bool,
}

/// BP_VehicleSpawner / BP_VehicleUniqueSpawner once the level is in, or the saved vehicles.
#[allow(clippy::too_many_arguments)]
fn spawn_vehicles(
    mut commands: Commands,
    assets: Res<AssetServer>,
    db: Res<VehicleDb>,
    sounds: Res<crate::audio::SoundDb>,
    store: Res<SettingsStore>,
    level: Option<Res<Level>>,
    terrain: Option<Res<Terrain>>,
    game: Res<CurrentGame>,
    mut containers: ResMut<super::interact::Containers>,
    mut driving: ResMut<Driving>,
    mut rays: crate::level::collision::PropRays,
) {
    let (Some(level), Some(terrain)) = (level, terrain) else { return };
    if driving.spawned || db.classes.is_empty() || game.state.is_none() {
        return;
    }
    driving.spawned = true;
    let mut rng = rand::thread_rng();
    let saved = game.state.as_ref().and_then(|s| s.world_vehicles.clone());
    let spawns: Vec<(Spawn, Option<super::items::Grid>)> = if let Some(save) = saved {
        save.list
            .into_iter()
            .map(|v| {
                let t = Transform::from_translation(Vec3::from(v.at)).with_rotation(Quat::from_rotation_y(v.yaw));
                (Spawn { class: v.class, t, color: v.color, fuel: v.fuel, health: v.health, lights: v.lights }, v.storage)
            })
            .collect()
    } else {
        let chance = super::survival::difficulty(&store, "Vehicles", "VEHICLE SPAWN CHANCE %");
        let mut out = Vec::new();
        for a in &level.actors {
            let class = match a.class.as_str() {
                "BP_VehicleSpawner_C" => {
                    // RandomChance, then GetAllActorsOfClass(BP_VehicleMaster) <= 30.
                    if rng.gen_range(0.0..100.0) > chance || out.len() > MAX_VEHICLES {
                        continue;
                    }
                    let sel = a.props.as_ref().and_then(|p| p.get("VehicleSelection")).map_or(0, |s| selection(s));
                    let Some(row) = db.spawners.get(&sel.to_string()) else { continue };
                    let Some(c) = row.classes.choose(&mut rng) else { continue };
                    c.clone()
                }
                "BP_VehicleUniqueSpawner_C" => match a.props.as_ref().and_then(|p| p.get("Vehicle")) {
                    Some(c) => c.trim_matches('"').to_string(),
                    None => continue,
                },
                _ => continue,
            };
            let Some(def) = db.classes.get(&class) else { continue };
            // The Billboard's transform, scale 1; only its yaw (ours: the body then rests on the ground).
            let fwd = crate::level::affine(&a.transform).rotation * Vec3::X;
            let yaw = (-fwd.z).atan2(fwd.x);
            let mut at = a.position();
            at.y = terrain.ground_under(at + Vec3::Y * 2.0).unwrap_or(at.y).max(at.y - 1.0);
            let color = def.colors.choose(&mut rng).and_then(|c| c.rsplit('/').next()).map(str::to_string);
            out.push((Spawn { class, t: Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)), color, fuel: def.fuel.current, health: def.health.current, lights: false }, None));
        }
        info!("vehicles: {} spawned at {chance:.0}% chance", out.len());
        out
    };
    let mut n = 0;
    for (sp, storage) in spawns {
        let Some(def) = db.classes.get(&sp.class) else { continue };
        let mut body = commands.spawn((model(&assets, format!("game/{}.glb", def.mesh)), sp.t, DespawnOnExit(AppState::InGame)));
        if let (Some(slot0), Some(color)) = (&def.slot0, &sp.color) {
            body.insert(MaterialRename(vec![(slot0.clone(), color.clone())]));
        }
        let e = body.id();
        for m in &def.extra_meshes {
            commands.spawn((model(&assets, format!("game/{m}.glb")), Transform::IDENTITY, ChildOf(e)));
        }
        // The headlights, pointing ahead (+X).
        for l in &def.lights {
            commands.spawn((
                SpotLight { intensity: 0.0, range: l.radius / 100.0, inner_angle: l.inner.to_radians(), outer_angle: l.outer.to_radians(), shadow_maps_enabled: false, ..default() },
                Transform::from_translation(ue(l.at)).looking_to(Vec3::X, Vec3::Y),
                HeadLight,
                ChildOf(e),
            ));
        }
        // EngineSound: its crossfade's inputs, each a loop faded by the speed.
        if let Some(x) = sounds.crossfades.get(ENGINE_CUE) {
            for (i, input) in x.inputs.iter().enumerate() {
                commands.spawn((
                    Transform::from_translation(Vec3::new(1.5, 0.9, 0.0)),
                    crate::audio::AmbientLoop { cue: input.cue.clone(), volume: 1.0, active: false },
                    crate::audio::LoopGain(0.0),
                    EngineLoop(i),
                    ChildOf(e),
                ));
            }
        }
        let v = Vehicle {
            class: sp.class.clone(),
            drive: Drive::default(),
            steer: 0.0,
            forward: 0.0,
            back: 0.0,
            fuel: sp.fuel,
            health: sp.health,
            engine_on: sp.health > 0.0,
            spin: 0.0,
            vy: 0.0,
            id: n,
            hit_cool: 0.0,
            noise: 0.0,
            color: sp.color,
            lights_on: sp.lights,
            flipped: 0.0,
            wheels: None,
        };
        if let Some(g) = storage {
            containers.searched.insert(v.storage_key(), g);
        }
        commands.entity(e).insert(v);
        rays.set_dynamic(COLLIDE_KEY + n, box_tris(def, &sp.t));
        n += 1;
    }
    driving.count = n;
}

/// The vehicles into the save (`GameState::world_vehicles`), every frame like the survivor's place.
fn save_vehicles(driving: Res<Driving>, containers: Res<super::interact::Containers>, mut game: ResMut<CurrentGame>, vehicles: Query<(&Transform, &Vehicle)>) {
    if !driving.spawned {
        return;
    }
    let Some(state) = game.state.as_mut() else { return };
    let mut list: Vec<(usize, SavedVehicle)> = vehicles
        .iter()
        .map(|(t, v)| {
            let fwd = t.rotation * Vec3::X;
            (
                v.id,
                SavedVehicle {
                    class: v.class.clone(),
                    at: t.translation.into(),
                    yaw: (-fwd.z).atan2(fwd.x),
                    fuel: v.fuel,
                    health: v.health,
                    color: v.color.clone(),
                    lights: v.lights_on,
                    storage: containers.searched.get(&v.storage_key()).filter(|g| !g.items.is_empty()).cloned(),
                },
            )
        })
        .collect();
    // (Before their entities exist, a fresh roll has nothing to save yet.)
    if list.is_empty() && driving.count > 0 {
        return;
    }
    list.sort_by_key(|(id, _)| *id);
    state.world_vehicles = Some(VehicleSave { list: list.into_iter().map(|(_, v)| v).collect() });
}

/// EngineSound while the survivor drives it; VehicleSound's 'RPM' = |mph| fades the inputs.
fn engine_sound(driving: Res<Driving>, sounds: Res<crate::audio::SoundDb>, vehicles: Query<&Vehicle>, mut loops: Query<(&ChildOf, &EngineLoop, &mut crate::audio::AmbientLoop, &mut crate::audio::LoopGain)>) {
    let Some(x) = sounds.crossfades.get(ENGINE_CUE) else { return };
    for (parent, i, mut l, mut gain) in &mut loops {
        let Ok(v) = vehicles.get(parent.parent()) else { continue };
        let on = driving.vehicle == Some(parent.parent()) && v.engine_on && v.health > 0.0;
        if l.active != on {
            l.active = on;
            if i.0 == 0 {
                info!("vehicles: engine sound {}", if on { "on" } else { "off" });
            }
        }
        gain.0 = x.inputs.get(i.0).map_or(0.0, |input| input.gain(v.mph().abs()));
    }
}

/// TurnOnOffLights' intensity on each headlight.
fn headlights(vehicles: Query<&Vehicle>, mut lights: Query<(&ChildOf, &mut SpotLight), With<HeadLight>>) {
    for (parent, mut spot) in &mut lights {
        let Ok(v) = vehicles.get(parent.parent()) else { continue };
        let want = if v.lights_on { crate::level::lights::lumens(LIGHT_INTENSITY / 625.0) } else { 0.0 };
        if spot.intensity != want {
            spot.intensity = want;
        }
    }
}

/// F at a vehicle: Get In Vehicle, or the Gas Can at its fuel cap; F in it: get out.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn interact(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    screen: Res<State<MenuScreen>>,
    mut overlay: ResMut<GameOverlay>,
    (mut containers, mut dirty, mut option): (ResMut<super::interact::Containers>, ResMut<super::inventory_ui::Dirty>, Local<usize>),
    db: Res<VehicleDb>,
    mut driving: ResMut<Driving>,
    mut game: ResMut<CurrentGame>,
    mut armed: ResMut<super::Armed>,
    mut vehicles: Query<(Entity, &Transform, &mut Vehicle), Without<CharacterModel>>,
    mut player: Query<(&mut Transform, &mut KinematicBody, &mut Locomotion), With<CharacterModel>>,
    mut prompt: Query<&mut Visibility, With<Prompt>>,
    mut text: Query<&mut Text, With<PromptText>>,
    mut bar: Query<&mut Node, With<PromptBar>>,
    mut notify: MessageWriter<Notify>,
    mut rays: crate::level::collision::PropRays,
) {
    let (Ok((mut pt, mut body, mut loco)), Ok(mut vis)) = (player.single_mut(), prompt.single_mut()) else { return };
    if *screen.get() != MenuScreen::None || overlay.is_open() {
        return;
    }
    // In a vehicle: F gets out (ExitVehicle).
    if let Some(e) = driving.vehicle {
        *vis = Visibility::Hidden;
        let Ok((_, t, v)) = vehicles.get(e) else {
            driving.vehicle = None;
            body.scripted = false;
            return;
        };
        if !keys.just_pressed(KeyCode::KeyF) || !can_exit(v.mph()) {
            return;
        }
        let Some(def) = db.classes.get(&v.class) else { return };
        // CheckCanExitCarDriverSide: a 25 cm sphere at the exit point; blocked, the passenger side.
        let free = |rays: &mut crate::level::collision::PropRays, p: Vec3| [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z].iter().all(|d| rays.cast(p, *d, 0.25).is_none());
        let driver = t.transform_point(ue(def.exit_driver));
        let passenger = t.transform_point(ue(def.exit_passenger));
        let Some(out) = [driver, passenger].into_iter().find(|p| free(&mut rays, *p)) else {
            info!("vehicles: no room to get out");
            return;
        };
        let tf = *t;
        driving.vehicle = None;
        pt.translation = out - Vec3::Y * 0.7;
        body.scripted = false;
        body.vertical_velocity = 0.0;
        *loco = Locomotion::Idle;
        if let Ok((_, _, mut v)) = vehicles.get_mut(e) {
            v.forward = 0.0;
            v.back = 0.0;
            rays.set_dynamic(COLLIDE_KEY + v.id, box_tris(def, &tf));
        }
        info!("vehicles: got out of the {} at {:.1}", def.name, out);
        return;
    }
    if *vis != Visibility::Hidden {
        return;
    }
    let p = pt.translation + Vec3::Y * 0.9;
    let near = vehicles
        .iter()
        .filter_map(|(e, t, v)| Some((e, *t, db.classes.get(&v.class)?)))
        .filter(|(_, t, c)| in_hit_box(c, t, p, REACH))
        .min_by(|a, b| a.1.translation.distance(p).total_cmp(&b.1.translation.distance(p)));
    let Some((e, t, def)) = near else { return };
    *vis = Visibility::Inherited;
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(0.0);
    }
    // InRefuellingArea: the FuelBoxCollision (75 x 50 x 50 cm) around the tank's cap.
    let local = t.compute_affine().inverse().transform_point3(p) - ue(def.fuel_box);
    let at_tank = local.x.abs() <= 0.75 + 0.3 && local.z.abs() <= 0.5 + 0.3 && local.y.abs() <= 1.5;
    // The interact widget's options (InteractOptions): Get In Vehicle, Open Storage; the wheel
    // picks (Push / Tow Vehicle aren't ported).
    const OPTIONS: [&str; 2] = ["Get In Vehicle", "Open Storage"];
    if scroll.delta.y != 0.0 {
        *option = (*option + 1) % OPTIONS.len();
    }
    if let Ok(mut tx) = text.single_mut() {
        tx.0 = if at_tank { format!("{}: [F] Refuel", def.name) } else { format!("{}: [F] {}  [Wheel] change", def.name, OPTIONS[*option]) };
    }
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Ok((_, _, mut v)) = vehicles.get_mut(e) else { return };
    if !at_tank && *option == 1 {
        // Interact_OpenStorage: the BP_JigMultiplayer container as "Vehicle Storage".
        let [cols, rows] = def.storage;
        containers.searched.entry(v.storage_key()).or_insert_with(|| super::items::Grid::new("Vehicle Storage", cols, rows));
        *overlay = GameOverlay::Loot(v.storage_key());
        dirty.0 = true;
        info!("vehicles: opened the {}'s storage ({cols} x {rows})", def.name);
        return;
    }
    if at_tank {
        // UseGasCan.
        if player_inv(&mut game).count("DA_GasCan") > 0 {
            player_inv(&mut game).remove("DA_GasCan", 1);
            v.fuel = refuel(v.fuel, def.fuel.max);
            info!("vehicles: refuelled the {} to {:.0}", def.name, v.fuel);
        } else {
            notify.write(Notify::new("No gas found"));
        }
        return;
    }
    // Interact_GetInVehicle.
    driving.vehicle = Some(e);
    body.scripted = true;
    body.vertical_velocity = 0.0;
    armed.drawn = None;
    armed.aiming = false;
    *loco = Locomotion::Drive;
    rays.set_dynamic(COLLIDE_KEY + v.id, Vec::new());
    *vis = Visibility::Hidden;
    info!("vehicles: got in the {} (fuel {:.0}, health {:.0})", def.name, v.fuel, v.health);
}

/// The survivor sits at PlayerLocation (30 cm lower), facing ahead.
fn seat(driving: Res<Driving>, db: Res<VehicleDb>, vehicles: Query<(&Transform, &Vehicle), Without<CharacterModel>>, mut player: Query<(&mut Transform, &mut Locomotion), With<CharacterModel>>) {
    let Some(e) = driving.vehicle else { return };
    let (Ok((t, v)), Ok((mut pt, mut loco))) = (vehicles.get(e), player.single_mut()) else { return };
    let Some(def) = db.classes.get(&v.class) else { return };
    pt.translation = t.transform_point(ue(def.player) - Vec3::Y * 0.3) - Vec3::Y * 0.82;
    // The survivor's model faces +Z; the vehicle's forward is +X.
    pt.rotation = t.rotation * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    if *loco != Locomotion::Drive {
        *loco = Locomotion::Drive;
    }
}

/// The ground under a point: the landscape or a prop's top (roads, bridges), whichever is higher.
/// Looked for from a climbable step above `p` down, so a roof or ceiling overhead isn't taken for it.
fn ground(terrain: &Terrain, rays: &mut crate::level::collision::PropRays, p: Vec3) -> f32 {
    let land = terrain.ground_under(p + Vec3::Y * CLIMB).unwrap_or(terrain.height(p.x, p.z));
    let prop = rays.cast(p + Vec3::Y * CLIMB, Vec3::NEG_Y, CLIMB + 2.0).map(|d| p.y + CLIMB - d);
    prop.map_or(land, |h| h.max(land))
}

/// VehicleDrivingComponent's inputs, the drivetrain, steering, ground, hits, fuel and noise.
#[allow(clippy::too_many_arguments)]
fn drive(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<State<MenuScreen>>,
    overlay: Res<GameOverlay>,
    db: Res<VehicleDb>,
    store: Res<SettingsStore>,
    driving: Res<Driving>,
    terrain: Option<Res<Terrain>>,
    mut rays: crate::level::collision::PropRays,
    mut vehicles: Query<(&mut Transform, &mut Vehicle)>,
    mut sound: MessageWriter<crate::audio::Sound>,
    mut noise: MessageWriter<super::zombies::Noise>,
    mut notify: MessageWriter<Notify>,
) {
    let (Some(e), Some(terrain)) = (driving.vehicle, terrain) else { return };
    let Ok((mut t, mut v)) = vehicles.get_mut(e) else { return };
    let Some(def) = db.classes.get(&v.class) else { return };
    let dt = time.delta_secs().min(0.05);
    let input = *screen.get() == MenuScreen::None && !overlay.is_open();
    let key = |k: KeyCode| if input && keys.pressed(k) { 1.0 } else { 0.0 };
    let approach = |x: f32, to: f32| x + (to - x).clamp(-INPUT_RATE * dt, INPUT_RATE * dt);
    v.forward = approach(v.forward, key(KeyCode::KeyW));
    v.back = approach(v.back, key(KeyCode::KeyS));
    v.steer = approach(v.steer, key(KeyCode::KeyD) - key(KeyCode::KeyA));
    let handbrake = key(KeyCode::Space) > 0.0;
    if input && keys.just_pressed(KeyCode::KeyH) {
        sound.write(crate::audio::Sound::at("Cue_Horn", t.translation));
    }
    // IA_Flashlight (L): TurnOnOffLights, not with an empty tank or the engine off; a wreck's
    // lights only go off.
    if input && keys.just_pressed(KeyCode::KeyL) && v.engine_on && v.fuel > 0.0 && (v.lights_on || v.health > 0.0) {
        v.lights_on = !v.lights_on;
        info!("vehicles: lights {}", if v.lights_on { "on" } else { "off" });
    }
    // IA_VehicleFlip (K): Event_FlipVehicle.
    v.flipped = (v.flipped - dt).max(0.0);
    if input && keys.just_pressed(KeyCode::KeyK) {
        if can_exit(v.mph()) && v.flipped <= 0.0 {
            t.translation.y += FLIP_LIFT;
            let fwd = (t.rotation * Vec3::X).with_y(0.0).normalize_or(Vec3::X);
            t.rotation = Quat::from_rotation_y((-fwd.z).atan2(fwd.x));
            v.flipped = FLIP_WAIT;
            info!("vehicles: flipped upright");
        } else {
            notify.write(Notify::new("Cannot flip vehicle"));
        }
    }
    let power = v.engine_on && v.fuel > 0.0 && v.health > 0.0;
    let (fwd, back) = (v.forward, v.back);
    let mut d = v.drive;
    step_drive(def, &mut d, fwd, back, handbrake, power, dt);
    // FuelSpend.
    if v.engine_on && v.fuel > 0.0 {
        let mult = super::survival::difficulty(&store, "Vehicles", "FUEL DEPLETION MULTIPLIER");
        v.fuel = (v.fuel - fuel_rate(&def.fuel, mult, d.speed * MPH) * dt).max(0.0);
    }
    // Event_Noise.
    v.noise += dt;
    if v.noise >= NOISE_EVERY {
        v.noise -= NOISE_EVERY;
        noise.write(super::zombies::Noise { at: t.translation, range: NOISE_RANGE });
    }
    // Steering: a bicycle model on the front wheels' angle.
    let steer = (v.steer * def.max_steer()).to_radians();
    let yaw_rate = d.speed * steer.tan() / def.wheelbase();
    let heading = (t.rotation * Vec3::X).with_y(0.0).normalize_or(Vec3::X);
    let yaw = (-heading.z).atan2(heading.x) - yaw_rate * dt;
    let flat = Quat::from_rotation_y(yaw);
    let dir = flat * Vec3::X;
    // Hits ahead (or behind): the bumper's corners and middle at two heights, and ground too high
    // to drive up.
    v.hit_cool = (v.hit_cool - dt).max(0.0);
    let step = d.speed * dt;
    if step.abs() > 1e-4 {
        let h = ue(def.hit_box.half).abs();
        let c = ue(def.hit_box.at);
        let end = c.x + h.x * step.signum();
        let along = dir * step.signum();
        let base = Transform::from_translation(t.translation).with_rotation(flat);
        let mut blocked = None;
        for z in [-h.z * 0.9, 0.0, h.z * 0.9] {
            for y in [0.5, 1.0] {
                let o = base.transform_point(Vec3::new(end, y, z));
                if let Some(hit) = rays.cast(o - along * 0.05, along, step.abs() + 0.1) {
                    blocked = Some(format!("a prop {hit:.2} m ahead of {o:.1}"));
                }
            }
            let front = base.transform_point(Vec3::new(end + step, 0.0, z));
            let g = ground(&terrain, &mut rays, front);
            if g > t.translation.y + CLIMB {
                blocked = Some(format!("ground {g:.2} at {front:.1} over {:.2}", t.translation.y));
            }
        }
        if let Some(why) = blocked {
            debug!("vehicles: blocked by {why}");
            let mph = d.speed * MPH;
            if hit_sounds(mph) && v.hit_cool <= 0.0 {
                sound.write(crate::audio::Sound::at("VehicleCollision_Cue", t.translation + dir * end));
                v.hit_cool = 0.25;
                info!("vehicles: hit at {:.0} mph", mph.abs());
            }
            d.speed = 0.0;
        } else {
            t.translation += dir * step;
        }
    }
    // The body rests on the ground under its wheels.
    let base = Transform::from_translation(t.translation).with_rotation(flat);
    let mut heights = Vec::new();
    for w in &def.wheels {
        let Some(at) = w.at else { continue };
        let at = Vec3::from(at);
        let contact = base.transform_point(Vec3::new(at.x, 0.0, at.z));
        heights.push((at, ground(&terrain, &mut rays, contact) - (at.y - w.radius / 100.0)));
    }
    if !heights.is_empty() {
        let mean = heights.iter().map(|h| h.1).sum::<f32>() / heights.len() as f32;
        let side = |pick: &dyn Fn(Vec3) -> bool| {
            let v: Vec<f32> = heights.iter().filter(|h| pick(h.0)).map(|h| h.1).collect();
            if v.is_empty() { mean } else { v.iter().sum::<f32>() / v.len() as f32 }
        };
        let (front, rear) = (side(&|a| a.x > 0.0), side(&|a| a.x <= 0.0));
        let (right, left) = (side(&|a| a.z > 0.0), side(&|a| a.z <= 0.0));
        let track = heights.iter().map(|h| h.0.z.abs()).fold(0.0f32, f32::max).max(0.5) * 2.0;
        let pitch = ((front - rear) / def.wheelbase()).atan().clamp(-0.6, 0.6);
        let roll = ((right - left) / track).atan().clamp(-0.6, 0.6);
        if t.translation.y > mean + 0.05 {
            v.vy -= 9.81 * dt;
            t.translation.y = (t.translation.y + v.vy * dt).max(mean);
        } else {
            v.vy = 0.0;
            t.translation.y = mean;
        }
        // Pitch about the vehicle's right (+Z), roll about its forward (+X).
        t.rotation = flat * Quat::from_rotation_z(pitch) * Quat::from_rotation_x(-roll);
    } else {
        t.rotation = flat;
    }
    v.spin += d.speed / def.drive_radius() * dt;
    v.drive = d;
}

/// The wheel bones, once the vehicle's scene is in.
fn find_wheels(
    db: Res<VehicleDb>,
    mut vehicles: Query<(Entity, &GlobalTransform, &mut Vehicle)>,
    children: Query<&Children>,
    names: Query<(&Name, &Transform, Option<&ChildOf>)>,
    globals: Query<&GlobalTransform>,
) {
    for (e, g, mut v) in &mut vehicles {
        if v.wheels.is_some() {
            continue;
        }
        let Some(def) = db.classes.get(&v.class) else { continue };
        let mut found = Vec::new();
        for w in &def.wheels {
            let Some(bone) = children.iter_descendants(e).find(|c| names.get(*c).is_ok_and(|n| n.0.as_str() == w.bone)) else { continue };
            let Ok((_, local, parent)) = names.get(bone) else { continue };
            let parent_rot = parent.and_then(|p| globals.get(p.parent()).ok()).map_or(Quat::IDENTITY, |pg| g.rotation().inverse() * pg.rotation());
            found.push((bone, *local, parent_rot, w.steering));
        }
        if found.len() == def.wheels.len() && !found.is_empty() {
            debug!("vehicles: {} wheel bones of the {} found", found.len(), def.name);
            v.wheels = Some(found);
        }
    }
}

/// Wheels roll with the speed and the front ones turn with the steering.
fn turn_wheels(db: Res<VehicleDb>, vehicles: Query<&Vehicle>, mut bones: Query<&mut Transform>) {
    for v in &vehicles {
        let (Some(ws), Some(def)) = (&v.wheels, db.classes.get(&v.class)) else { continue };
        let steer = (v.steer * def.max_steer()).to_radians();
        for (bone, base, parent, steers) in ws {
            let Ok(mut t) = bones.get_mut(*bone) else { continue };
            // In the vehicle's frame: steer about up, roll about the axle (+Z).
            let r = Quat::from_rotation_y(if *steers { -steer } else { 0.0 }) * Quat::from_rotation_z(-v.spin);
            t.rotation = parent.inverse() * r * *parent * base.rotation;
        }
    }
}

/// ApplyDamageToAIBasedOnSpeed: living zombies and animals in the hit box at 5 mph or more.
#[allow(clippy::type_complexity)]
fn hit_zombies(
    driving: Res<Driving>,
    db: Res<VehicleDb>,
    store: Res<SettingsStore>,
    mut vehicles: Query<(&Transform, &mut Vehicle)>,
    mut zombies: Query<(&Transform, &mut super::combat::Health), (With<LevelZombie>, Without<super::Dead>, Without<Vehicle>)>,
) {
    let Some(e) = driving.vehicle else { return };
    let Ok((t, mut v)) = vehicles.get_mut(e) else { return };
    let Some(def) = db.classes.get(&v.class) else { return };
    if v.mph().abs() < 5.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    for (zt, mut h) in &mut zombies {
        if h.hp <= 0.0 || !in_hit_box(def, t, zt.translation + Vec3::Y * 0.9, 0.3) {
            continue;
        }
        h.hp -= rng.gen_range(300..=350) as f32;
        // DmgOnVehicleCollision, through ReceiveAnyDamage.
        let dmg = rng.gen_range(def.collision_damage[0].trunc() as i32..=def.collision_damage[1].trunc() as i32) as f32;
        v.health = (v.health - dmg * super::survival::difficulty(&store, "Vehicles", "DAMAGE DEPLETION MULTIPLIER") / 2.5).max(0.0);
        if v.health <= 0.0 {
            v.engine_on = false;
        }
        info!("vehicles: ran over a zombie at {:.0} mph (vehicle health {:.1})", v.mph().abs(), v.health);
    }
}

/// VehicleUI: bottom right, name, HealthBar (wrench), FuelBar (gas can), speed.
fn spawn_ui(mut commands: Commands, font: Res<MenuFont>, assets: Res<AssetServer>) {
    const ICONS: &str = "game/SurrounDead/Content/UI/Textures/Survivors_Icon_Pack/256x256";
    let font = &*font;
    let row = || Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, padding: UiRect { left: Val::Px(5.0), top: Val::Px(5.0), right: Val::Px(5.0), ..default() }, ..default() };
    let fill = |c: Color| (Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(c));
    let track = || (Node { flex_grow: 1.0, height: Val::Px(12.0), margin: UiRect::left(Val::Px(5.0)), padding: UiRect::all(Val::Px(1.5)), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.1)));
    let icon = |path: &str| (Node { width: Val::Px(24.0), height: Val::Px(24.0), margin: UiRect::left(Val::Px(5.0)), ..default() }, ImageNode::new(assets.load(format!("{ICONS}/{path}.png"))));
    commands.spawn((
        // Border_0: anchored bottom right, (-10, -10) offset, 310 x 130, alignment (1, 1).
        Node { position_type: PositionType::Absolute, right: Val::Px(10.0), bottom: Val::Px(10.0), width: Val::Px(310.0), padding: UiRect::all(Val::Px(10.0)), flex_direction: FlexDirection::Column, ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        Visibility::Hidden,
        VehicleUi,
        DespawnOnExit(AppState::InGame),
        children![
            (label_colored(font, "", 16.0, Color::WHITE), Node { margin: UiRect::left(Val::Px(5.0)), ..default() }, VehicleName),
            (row(), children![(track(), children![(fill(Color::srgb_u8(0x7F, 0, 0)), HealthBar)]), icon("Weapons_For_The_Apocalypse/T_Pipe_wrench_256x256")]),
            (row(), children![icon("A_Scavengers_Prey/T_Gas_256x256"), (track(), children![(fill(Color::srgb_u8(0xBD, 0x98, 0x53)), FuelBar)])]),
            (label_colored(font, "0 MPH", 14.0, Color::WHITE), Node { margin: UiRect { left: Val::Px(5.0), top: Val::Px(5.0), ..default() }, ..default() }, SpeedText),
        ],
    ));
}

/// VehicleUI's Tick (speed, in KPH with VEHICLE SPEED IN KPH? = Game.Metric) and its UpdateFuel /
/// UpdateHealth.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_ui(
    driving: Res<Driving>,
    db: Res<VehicleDb>,
    store: Res<SettingsStore>,
    vehicles: Query<&Vehicle>,
    mut ui: Query<&mut Visibility, With<VehicleUi>>,
    mut name: Query<&mut Text, (With<VehicleName>, Without<SpeedText>)>,
    mut speed: Query<&mut Text, (With<SpeedText>, Without<VehicleName>)>,
    mut health: Query<&mut Node, (With<HealthBar>, Without<FuelBar>)>,
    mut fuel: Query<&mut Node, (With<FuelBar>, Without<HealthBar>)>,
) {
    let Ok(mut vis) = ui.single_mut() else { return };
    let Some(v) = driving.vehicle.and_then(|e| vehicles.get(e).ok()) else {
        *vis = Visibility::Hidden;
        return;
    };
    let Some(def) = db.classes.get(&v.class) else { return };
    *vis = Visibility::Inherited;
    let metric = store.find(&row_key("settings", "Gameplay", "VEHICLE SPEED IN KPH?")) == Some(Value::Bool(true));
    if let Ok(mut t) = name.single_mut() {
        t.0.clone_from(&def.name);
    }
    if let Ok(mut t) = speed.single_mut() {
        let s = (v.mph() * if metric { 1.60934 } else { 1.0 }).abs();
        t.0 = format!("{s:.0} {}", if metric { "KPH" } else { "MPH" });
    }
    if let Ok(mut n) = health.single_mut() {
        n.width = Val::Percent(100.0 * v.health / def.health.max.max(1.0));
    }
    if let Ok(mut n) = fuel.single_mut() {
        n.width = Val::Percent(100.0 * v.fuel / def.fuel.max.max(1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suv() -> VehicleClass {
        VehicleDb::load().classes.get("Vehicle_SUV_C").cloned().expect("vehicles.json has the SUV")
    }

    #[test]
    fn torque_curves_are_normalised_to_max_torque() {
        let c = suv();
        // Torque_SUV: 150 at 1000, 250 at 3500 (its peak), 0 at 6500; MaxTorque 400.
        assert!((torque_at(&c.engine, 3500.0) - 400.0).abs() < 1e-3);
        assert!((torque_at(&c.engine, 1000.0) - 240.0).abs() < 1e-3);
        assert!((torque_at(&c.engine, 500.0) - 240.0).abs() < 1e-3);
        // BP_VehicleMaster's own curve runs over 0..2 "RPM": flat at its peak above that.
        let e = Engine { max_rpm: 4000.0, max_torque: 400.0, idle_rpm: 900.0, brake_effect: 0.2, torque_curve: vec![[0.0, 0.0], [2.0, 300.0]] };
        assert_eq!(torque_at(&e, 3000.0), 400.0);
    }

    #[test]
    fn the_suv_accelerates_and_tops_out_in_first_gear() {
        let c = suv();
        let mut d = Drive::default();
        for _ in 0..(60 * 30) {
            step_drive(&c, &mut d, 1.0, 0.0, false, true, 1.0 / 60.0);
        }
        // MaxRPM 3750 is under ChangeUpRPM 4500: it never shifts, and the limiter holds it at
        // 3750 rpm through 4 × 3 on 40 cm wheels: ~13 m/s (29 mph).
        assert_eq!(d.gear, 1);
        let top = 3750.0 * std::f32::consts::TAU / 60.0 / 12.0 * 0.4;
        assert!(d.speed > top * 0.9 && d.speed <= top * 1.01, "{} vs {top}", d.speed);
        // Off the throttle it slows; braking stops it, then S reverses.
        for _ in 0..(60 * 10) {
            step_drive(&c, &mut d, 0.0, 1.0, false, true, 1.0 / 60.0);
        }
        assert_eq!(d.gear, -1);
        assert!(d.speed < 0.0, "{}", d.speed);
    }

    #[test]
    fn no_power_no_drive() {
        let c = suv();
        let mut d = Drive::default();
        for _ in 0..120 {
            step_drive(&c, &mut d, 1.0, 0.0, false, false, 1.0 / 60.0);
        }
        assert_eq!(d.speed, 0.0);
    }

    #[test]
    fn a_high_revving_class_shifts_up() {
        let mut c = suv();
        c.engine.max_rpm = 6500.0;
        let mut d = Drive::default();
        let mut top_gear = 1;
        for _ in 0..(60 * 40) {
            step_drive(&c, &mut d, 1.0, 0.0, false, true, 1.0 / 60.0);
            top_gear = top_gear.max(d.gear);
        }
        assert!(top_gear >= 2, "stayed in gear {top_gear}");
    }

    #[test]
    fn fuel_exit_and_refuel_rules() {
        let f = Fuel { current: 1000.0, max: 1000.0, waste_time: 0.01, waste: 0.01 };
        assert!((fuel_rate(&f, 1.0, 30.0) - 1.0).abs() < 1e-5);
        assert!((fuel_rate(&f, 1.0, 1.0) - 0.1).abs() < 1e-5);
        assert!((fuel_rate(&f, 2.0, -30.0) - 2.0).abs() < 1e-5);
        assert!(can_exit(9.9) && !can_exit(10.0) && !can_exit(-12.0));
        assert_eq!(refuel(800.0, 1000.0), 1000.0);
        assert_eq!(refuel(100.0, 1000.0), 600.0);
        assert!(!hit_sounds(4.9) && hit_sounds(5.0));
    }

    #[test]
    fn every_class_has_wheels_and_spawner_rows_resolve() {
        let db = VehicleDb::load();
        assert_eq!(db.classes.len(), 17);
        for (k, c) in &db.classes {
            assert!(c.wheels.len() >= 4 && c.wheels.iter().all(|w| w.at.is_some()), "{k}");
            assert!(c.wheels.iter().any(|w| w.steering) && c.wheels.iter().any(|w| w.engine), "{k}");
            assert!(c.wheelbase() > 1.0, "{k}");
        }
        for r in db.spawners.values() {
            assert!(r.classes.iter().all(|c| db.classes.contains_key(c)), "{}", r.row);
        }
        assert_eq!(selection("\"Enum_VehicleSpawner::NewEnumerator6\""), 3);
        assert_eq!(db.spawners["3"].row, "Industrial");
    }

    #[test]
    fn saves_round_trip_and_classes_have_lights_and_storage() {
        let mut g = super::super::items::Grid::new("Vehicle Storage", 8, 8);
        g.items.clear();
        let save = VehicleSave {
            list: vec![SavedVehicle { class: "Vehicle_SUV_C".into(), at: [1.0, 2.0, 3.0], yaw: 0.5, fuel: 640.0, health: 97.5, color: Some("Mat_PolygonMilitary_Vehicles_03".into()), lights: true, storage: Some(g) }],
        };
        let back: VehicleSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();
        assert_eq!(back, save);
        // An older save without lights or storage still loads.
        let old: SavedVehicle = serde_json::from_str(r#"{"class":"Vehicle_Van_C","at":[0,0,0],"yaw":0,"fuel":1,"health":1,"color":null}"#).unwrap();
        assert!(!old.lights && old.storage.is_none());
        let db = VehicleDb::load();
        for (k, c) in &db.classes {
            assert!(c.lights.len() >= 2, "{k}: {} lights", c.lights.len());
            // The loot column fits the widest storage.
            assert!(c.storage[0] >= 1 && c.storage[0] <= super::super::interact::LOOT_COLUMNS && c.storage[1] >= 1, "{k}: {:?}", c.storage);
        }
        // Storage keys don't read as traders' or level containers'.
        assert!(!super::super::traders::is_trader(STORAGE_KEYS + 100) && STORAGE_KEYS > super::super::interact::CORPSE_KEYS);
    }

    #[test]
    fn the_engine_sound_crossfades_by_speed() {
        let sounds = crate::audio::SoundDb::load();
        let Some(x) = sounds.crossfades.get(ENGINE_CUE) else { return };
        assert_eq!(x.inputs.len(), 3);
        let loudest = |mph: f32| (0..3).max_by(|a, b| x.inputs[*a].gain(mph).total_cmp(&x.inputs[*b].gain(mph))).unwrap();
        // Idle when stopped, the low loop around 30 mph, the high one past 60.
        assert_eq!(loudest(0.0), 0);
        assert_eq!(loudest(30.0), 1);
        assert_eq!(loudest(70.0), 2);
    }

    #[test]
    fn hit_box_contains_its_centre() {
        let c = suv();
        let t = Transform::from_xyz(10.0, 0.0, 5.0).with_rotation(Quat::from_rotation_y(0.7));
        assert!(in_hit_box(&c, &t, t.transform_point(ue(c.hit_box.at)), 0.0));
        assert!(!in_hit_box(&c, &t, t.translation + Vec3::new(0.0, 0.0, 30.0), 0.5));
        assert_eq!(box_tris(&c, &t).len(), 12);
        // UE (X fwd, Y right, Z up) cm -> (X, Z, Y) m.
        assert_eq!(ue([100.0, 200.0, 300.0]), Vec3::new(1.0, 3.0, 2.0));
    }
}
