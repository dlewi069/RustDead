//! Airdrops: BP_EventManager's airdrop timer, AirdropAircraft and Master_AirdropContainer.
//!
//! - BP_EventManager (BeginPlay, then after every drop): with AIRDROPS ENABLED? (Difficulty.Airdrop)
//!   a timer of 300-3600 s; when it fires (and the setting is still on) an AirdropAircraft spawns
//!   at a random point of BP_Event_AirdropGoToLocation's region bounds times the level's
//!   DistanceToSpawnAirdrop (5), 450-550 m up, yawed at the region's centre, with RegionRadius
//!   and 1-3 crates to drop.
//! - AirdropAircraft: the AirDropPlane mesh (x2) flying straight at Speed 75 m/s (VInterpTo_Constant
//!   along its forward axis, height kept) for FlightDuration 300 s, AirCraft_Cue looping through the
//!   "Aircraft" attenuation, its three navigation lights blinking (MC_Lights, 1 s). Every
//!   RegionRadius / Speed s (CheckForRegionCrossing) it traces straight down; once over the region's
//!   sphere it waits 0.1-30 s, then drops its crates 1-4 s apart, each a random class of Military,
//!   Medical and Supplies at its DirectionOfLineTracer (0.8 m under it), scaled 1.35.
//! - Master_AirdropContainer: the AirdropCrate (x1.35, 2.4 m tall; simulated: UE gravity and
//!   LinearDamping 1, so it sinks at about 9.8 m/s) under its parachute, which goes when it hits
//!   something (ReceiveHit). 3 s after it appears: "Airdrop incoming..." (only with AIRDROP
//!   NOTIFICATION ENABLED?, off by default). Its BP_Marker puts the
//!   Drop icon (x2, in the class's colour) on the map with "Military Airdrop" etc. and "Location of
//!   airdrop dropped by a mysterious military force.". Landed, it's a container: hold F 6 s
//!   (DurationToHold, Quick Search noise 10 %), looted as "Airdrop" from its class's table
//!   (AirdropMilitary / AirdropMedical / AirdropSupplies), and it blocks the way.
//!
//! - The crate's ParticleSystem (62 cm up, under the actor's scale): its class's coloured smoke
//!   ([`smoke`]) from the moment it appears, Deactivate()d once it's opened.
//!
//! Not ported: the outline and magnifier, saving crates, the event manager's jets, explosions, blood moon and dynamic random
//! events. Crates land on terrain and on props streamed in near the survivor; one far away lands on
//! the terrain under it.

use bevy::prelude::*;
use rand::Rng;

use super::interact::Containers;
use super::items::ItemDb;
use super::map::{WorldMarker, WorldMarkers};
use crate::level::{Level, Terrain};
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::{AppState, MenuScreen};
use crate::models::{MaterialRename, model};

/// AirdropAircraft.Speed (7500 cm/s).
pub const SPEED: f32 = 75.0;
/// FlightDuration: the aircraft's life span.
const FLIGHT: f32 = 300.0;
/// BP_EventManager's AirdropTimeToSpawn range (s).
pub const EVERY: (f32, f32) = (300.0, 3600.0);
/// The aircraft's height (45000-55000 cm).
pub const ALTITUDE: (f32, f32) = (450.0, 550.0);
/// After crossing into the region: 0.1-30 s, then 1-4 s between crates.
const FIRST_DROP: (f32, f32) = (0.1, 30.0);
const BETWEEN: (f32, f32) = (1.0, 4.0);
/// DirectionOfLineTracer's RelativeLocation Z (-80 cm).
const TRACER: f32 = 0.8;
/// The crate's scale: the aircraft spawns it at 1.35 (its StaticMesh's RelativeScale3D is 1.35
/// too; counted once, as the Parachute's RelativeLocation then sits on the crate's top, 2.4 m).
const CRATE_SCALE: f32 = 1.35;
/// UE's default gravity (the crate isn't a character: no GravityScale) and its LinearDamping.
pub const GRAVITY: f32 = 9.8;
const DAMPING: f32 = 1.0;
/// "Airdrop incoming..." after BeginPlay's 3 s Delay.
const NOTICE_AFTER: f32 = 3.0;
/// AirdropAircraft's crate classes (its Airdrops array).
pub const CLASSES: [&str; 3] = ["AirdropContainer_Military_C", "AirdropContainer_Medical_C", "AirdropContainer_Supplies_C"];
const PLANE: &str = "game/SurrounDead/Content/Meshes/World/AirDropPlane.glb";
/// The StaticMesh component's OverrideMaterials (slot 0; the glass keeps its own).
const PLANE_MATERIAL: (&str, &str) = ("Mat_PolygonMilitary_AircraftLarge_01", "Mat_PolygonMilitary_Aircraft_01");
const CRATE: &str = "game/SurrounDead/Content/Meshes/World/AirdropCrate.glb";
const PARACHUTE: &str = "game/SurrounDead/Content/PolygonFiles/PolygonBattleRoyale/Meshes/Props/SM_Prop_EmergencyDrop_01_Parachute.glb";
const DROP_ICON: &str = "game/SurrounDead/Content/UI/Textures/Survival-UI-Bundle/Rhombus/512x512/Drop.png";
const DESCRIPTION: &str = "Location of airdrop dropped by a mysterious military force.";
/// Dynamic collision keys for landed crates.
const COLLIDE_KEY: usize = 1 << 41;

/// BP_Event_AirdropGoToLocation's sphere and BP_EventManager's DistanceToSpawnAirdrop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub centre: Vec3,
    pub radius: f32,
    pub spawn_distance: f32,
}

impl Region {
    fn from_level(level: &Level) -> Option<Self> {
        let prop = |a: &crate::level::ActorInfo, k: &str| a.props.as_ref().and_then(|p| p.get(k)).and_then(|v| v.parse::<f32>().ok());
        let region = level.actors.iter().find(|a| a.class == "BP_Event_AirdropGoToLocation_C")?;
        let manager = level.actors.iter().find(|a| a.class == "BP_EventManager_C");
        Some(Self {
            centre: region.position(),
            // RegionRadius (cm; the class default 5000, at least 64 by its construction script).
            radius: prop(region, "RegionRadius").unwrap_or(5000.0).max(64.0) / 100.0,
            spawn_distance: manager.and_then(|m| prop(m, "DistanceToSpawnAirdrop")).unwrap_or(0.0),
        })
    }

    /// Where an aircraft starts: a random point of the sphere's bounds, its X and Y (UE) times
    /// DistanceToSpawnAirdrop, at 450-550 m; and its heading (flat, at the centre).
    pub fn aircraft_start(&self, rng: &mut impl Rng) -> (Vec3, Vec3) {
        let r = self.radius;
        let p = self.centre + Vec3::new(rng.gen_range(-r..=r), 0.0, rng.gen_range(-r..=r));
        let start = Vec3::new(p.x * self.spawn_distance, rng.gen_range(ALTITUDE.0..=ALTITUDE.1), p.z * self.spawn_distance);
        let dir = (self.centre - start).with_y(0.0).normalize_or(Vec3::X);
        (start, dir)
    }

    /// CheckForRegionCrossing's trace straight down hits the sphere.
    pub fn below(&self, at: Vec3) -> bool {
        at.xz().distance(self.centre.xz()) < self.radius && at.y > self.centre.y - self.radius
    }

    /// The check's timer: RegionRadius / Speed.
    pub fn check_every(&self) -> f32 {
        self.radius / SPEED
    }
}

/// One step of a simulated body with UE's linear damping (PhysX: v / (1 + dt c)).
pub fn fall(vel: f32, dt: f32) -> f32 {
    (vel - GRAVITY * dt) / (1.0 + dt * DAMPING)
}

#[derive(Component)]
pub struct Aircraft {
    dir: Vec3,
    age: f32,
    next_check: f32,
    /// Over the region: seconds to the next crate and how many are left.
    dropping: Option<(f32, u32)>,
    crates: u32,
    lights: Vec<Entity>,
}

#[derive(Component)]
pub struct AirCrate {
    pub class: &'static str,
    pub vel: f32,
    pub age: f32,
    /// Its container key once landed.
    pub key: Option<usize>,
    parachute: Entity,
    /// Its smoke (an `Emitter`).
    smoke: Entity,
    /// Its number this session (its collision key).
    id: usize,
}

/// Each crate class's ParticleSystem template (Master_AirdropContainer's FX_Smoke_Tall, which the
/// children swap for their colour).
pub fn smoke(class: &str) -> &'static str {
    match class {
        "AirdropContainer_Military_C" => "FX_Smoke_Tall_Military",
        "AirdropContainer_Medical_C" => "FX_Smoke_Tall_Medical",
        "AirdropContainer_Supplies_C" => "FX_Smoke_Tall_Food",
        _ => "FX_Smoke_Tall",
    }
}

/// The ParticleSystem's RelativeLocation Z (62 cm).
const SMOKE_HEIGHT: f32 = 0.62;

#[derive(Resource, Default)]
pub struct Airdrops {
    pub region: Option<Region>,
    /// Seconds to the event manager's timer (None: not armed).
    pub timer: Option<f32>,
    /// BeginPlay's 1 s Delay before the first arming.
    started: f32,
    pub planes: u32,
    pub crates: u32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Airdrops>()
        .add_systems(OnEnter(AppState::InGame), |mut a: ResMut<Airdrops>| *a = Airdrops::default())
        .add_systems(Update, (schedule, fly, (sink, land).chain(), engine_sound, smoke_out).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

/// Opening the crate (its loot widget shown) deactivates the smoke; what's out drifts off.
fn smoke_out(containers: Res<Containers>, crates: Query<&AirCrate>, mut emitters: Query<&mut super::particles::Emitter>) {
    for c in &crates {
        if c.key.is_some_and(|k| containers.searched.contains_key(&k))
            && let Ok(mut e) = emitters.get_mut(c.smoke)
            && e.active
        {
            e.active = false;
            info!("airdrop crate {} opened: smoke off", c.class);
        }
    }
}

/// AIRDROP NOTIFICATION ENABLED? (CVar Game.AirDrop, off by default): the crate's
/// AirdropNotification only toasts while it's on.
pub fn notification_on(store: &SettingsStore) -> bool {
    store.find(&crate::menus::settings::row_key("settings", "Gameplay", "AIRDROP NOTIFICATION ENABLED?")) == Some(crate::menus::settings::Value::Bool(true))
}

pub fn enabled(store: &SettingsStore) -> bool {
    store.find(&row_key("difficulty", "General", "AIRDROPS ENABLED?")) != Some(Value::Bool(false))
}

/// The event manager: arm the timer a second in (AirdropSpawn's Delay), and on firing spawn an
/// aircraft and arm it again, while the setting is on.
fn schedule(mut commands: Commands, time: Res<Time>, level: Option<Res<Level>>, store: Res<SettingsStore>, mut drops: ResMut<Airdrops>, assets: Res<AssetServer>) {
    let Some(level) = level else { return };
    if drops.region.is_none() {
        drops.region = Region::from_level(&level);
        drops.started = 0.0;
    }
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    if drops.started < 1.0 {
        drops.started += dt;
        if drops.started >= 1.0 && enabled(&store) {
            drops.timer = Some(rng.gen_range(EVERY.0..=EVERY.1));
        }
        return;
    }
    let Some(t) = drops.timer.as_mut() else { return };
    *t -= dt;
    if *t > 0.0 {
        return;
    }
    drops.timer = None;
    if !enabled(&store) {
        return;
    }
    spawn_aircraft(&mut commands, &assets, &mut drops, &mut rng);
    drops.started = 0.0;
}

pub fn spawn_aircraft(commands: &mut Commands, assets: &AssetServer, drops: &mut Airdrops, rng: &mut impl Rng) -> Option<Entity> {
    let region = drops.region?;
    let (start, dir) = region.aircraft_start(rng);
    Some(spawn_aircraft_at(commands, assets, &region, start, dir, rng.gen_range(1..=3)))
}

/// An aircraft at `start` heading `dir` with `crates` to drop (also used by the tour).
pub fn spawn_aircraft_at(commands: &mut Commands, assets: &AssetServer, region: &Region, start: Vec3, dir: Vec3, crates: u32) -> Entity {
    // The mesh's nose is UE +X, our local +X.
    let rotation = Quat::from_rotation_y((-dir.z).atan2(dir.x));
    let plane = commands
        .spawn((Transform::from_translation(start).with_rotation(rotation), Visibility::default(), DespawnOnExit(AppState::InGame)))
        .id();
    commands.spawn((model(assets, PLANE), MaterialRename(vec![(PLANE_MATERIAL.0.into(), PLANE_MATERIAL.1.into())]), Transform::from_scale(Vec3::splat(2.0)), ChildOf(plane)));
    // LightLeftRed / LightRightGreen / LightBackWhite: 50000 cd, 163.84 m AttenuationRadius, at
    // their RelativeLocations (UE (X, Y, Z) cm -> our (X, Z, Y) m).
    let lights = [(Vec3::new(-29.35, 14.53, -47.35), Color::srgb(1.0, 0.0, 0.0)), (Vec3::new(-29.35, 14.53, 47.24), Color::srgb(0.325, 1.0, 0.0)), (Vec3::new(-70.66, 29.27, 0.0), Color::WHITE)]
        .map(|(at, color)| {
            commands
                .spawn((
                    PointLight { color, intensity: crate::level::lights::lumens(50000.0), range: 163.84, shadow_maps_enabled: false, ..default() },
                    Transform::from_translation(at),
                    ChildOf(plane),
                ))
                .id()
        })
        .to_vec();
    commands.entity(plane).insert(Aircraft { dir, age: 0.0, next_check: region.check_every(), dropping: None, crates, lights });
    info!("airdrop aircraft from {start:.0} heading {dir:.2}, {crates} crates");
    plane
}

fn fly(mut commands: Commands, time: Res<Time>, assets: Res<AssetServer>, mut drops: ResMut<Airdrops>, mut planes: Query<(Entity, &mut Transform, &mut Aircraft)>, mut lights: Query<&mut Visibility>) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let Some(region) = drops.region else { return };
    for (e, mut t, mut a) in &mut planes {
        a.age += dt;
        if a.age >= FLIGHT {
            commands.entity(e).despawn();
            continue;
        }
        t.translation += a.dir * SPEED * dt;
        // MC_Lights: the lights toggle every second.
        let on = (a.age as u32).is_multiple_of(2);
        for l in &a.lights {
            if let Ok(mut v) = lights.get_mut(*l) {
                v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
            }
        }
        let tracer = t.translation - Vec3::Y * TRACER;
        match a.dropping {
            None => {
                a.next_check -= dt;
                if a.next_check <= 0.0 {
                    a.next_check += region.check_every();
                    if region.below(tracer) {
                        a.dropping = Some((rng.gen_range(FIRST_DROP.0..=FIRST_DROP.1), a.crates));
                        info!("airdrop aircraft over the region at {:.0}", t.translation);
                    }
                }
            }
            Some((wait, left)) if left > 0 => {
                let wait = wait - dt;
                if wait > 0.0 {
                    a.dropping = Some((wait, left));
                    continue;
                }
                let class = CLASSES[rng.gen_range(0..CLASSES.len())];
                spawn_crate(&mut commands, &assets, &mut drops, class, tracer);
                a.dropping = Some((rng.gen_range(BETWEEN.0..=BETWEEN.1), left - 1));
            }
            Some(_) => {}
        }
    }
}

/// A crate under its parachute at `at` (also used by the tour).
pub fn spawn_crate(commands: &mut Commands, assets: &AssetServer, drops: &mut Airdrops, class: &'static str, at: Vec3) -> Entity {
    let crate_ = commands.spawn((Transform::from_translation(at), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
    commands.spawn((model(assets, CRATE), Transform::from_scale(Vec3::splat(CRATE_SCALE)), ChildOf(crate_)));
    // Parachute: RelativeLocation (-39.6, 0, 120) cm under the actor's 1.35 scale.
    let parachute = commands.spawn((model(assets, PARACHUTE), Transform::from_translation(Vec3::new(-0.396, 1.2, 0.0) * 1.35).with_scale(Vec3::splat(1.35)), ChildOf(crate_))).id();
    let smoke = commands
        .spawn((Transform::from_xyz(0.0, SMOKE_HEIGHT * CRATE_SCALE, 0.0).with_scale(Vec3::splat(CRATE_SCALE)), Visibility::default(), super::particles::Emitter::new(self::smoke(class)), ChildOf(crate_)))
        .id();
    commands.entity(crate_).insert(AirCrate { class, vel: 0.0, age: 0.0, key: None, parachute, smoke, id: drops.crates as usize });
    drops.crates += 1;
    info!("airdrop crate {class} at {at:.0}");
    crate_
}

/// The crate sinks under its parachute; 3 s in it announces itself and goes on the map.
#[allow(clippy::too_many_arguments)]
fn sink(
    time: Res<Time>,
    db: Res<ItemDb>,
    store: Res<SettingsStore>,
    mut crates: Query<(&mut Transform, &mut AirCrate)>,
    mut notify: MessageWriter<super::notify::Notify>,
    mut markers: ResMut<WorldMarkers>,
) {
    let dt = time.delta_secs();
    for (mut t, mut c) in &mut crates {
        let before = c.age;
        c.age += dt;
        if before < NOTICE_AFTER && c.age >= NOTICE_AFTER && notification_on(&store) {
            notify.write(super::notify::Notify::new("Airdrop incoming..."));
        }
        if before == 0.0
            && let Some(info) = db.containers.get(c.class).and_then(|d| d.airdrop.clone())
        {
            markers.0.push(WorldMarker {
                key: c.id,
                pos: t.translation,
                texture: DROP_ICON.into(),
                size: 2.0,
                colour: Color::linear_rgb(info.color[0], info.color[1], info.color[2]),
                title: info.title,
                description: DESCRIPTION.into(),
            });
        }
        if c.key.is_none() {
            c.vel = fall(c.vel, dt);
            t.translation.y += c.vel * dt;
        }
    }
}

/// ReceiveHit: on the ground (terrain or a streamed prop), the parachute goes and the crate opens
/// for searching, solid.
#[allow(clippy::too_many_arguments)]
fn land(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    mut rays: crate::level::collision::PropRays,
    mut containers: ResMut<Containers>,
    assets: Res<AssetServer>,
    mut crates: Query<(&mut Transform, &mut AirCrate)>,
    mut mesh: Local<Option<Handle<Mesh>>>,
) {
    let handle = mesh.get_or_insert_with(|| assets.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(CRATE))).clone();
    for (mut t, mut c) in &mut crates {
        if c.key.is_some() {
            continue;
        }
        let p = t.translation;
        let floor = rays.cast(p + Vec3::Y * 0.5, Vec3::NEG_Y, 0.5 - c.vel.min(0.0) * 0.1 + 0.5).map(|d| p.y + 0.5 - d);
        let ground = terrain.as_ref().and_then(|tr| tr.ground_under(p + Vec3::Y * 0.5)).into_iter().chain(floor).reduce(f32::max);
        let Some(g) = ground else { continue };
        if p.y > g {
            continue;
        }
        t.translation.y = g;
        info!("airdrop crate {} landed at {:.1} speed {:.1}", c.class, t.translation, -c.vel);
        c.vel = 0.0;
        c.key = Some(containers.add_dropped(t.translation, c.class, None));
        commands.entity(c.parachute).despawn();
        rays.set_dynamic_from(COLLIDE_KEY + c.id, &handle, &t.with_scale(Vec3::splat(CRATE_SCALE)));
    }
}

/// The engine's AudioComponent: AirCraft_Cue through the "Aircraft" attenuation (1500 m inner,
/// 1750 m falloff), restarted whenever it comes back into earshot.
#[allow(clippy::type_complexity)]
fn engine_sound(mut commands: Commands, camera: Query<&GlobalTransform, With<super::PlayerCamera>>, planes: Query<(Entity, &Transform), (With<Aircraft>, Without<crate::audio::Voiced>)>, db: Option<Res<crate::audio::SoundDb>>, mut sound: MessageWriter<crate::audio::Sound>) {
    let (Ok(ear), Some(db)) = (camera.single(), db) else { return };
    let reach = db.attenuation.get("Aircraft").map_or(3250.0, |a| a.inner + a.falloff);
    for (e, t) in &planes {
        if t.translation.distance(ear.translation()) < reach {
            commands.entity(e).insert(crate::audio::Voiced);
            sound.write(crate::audio::Sound { cue: "AirCraft_Cue".into(), at: Some(t.translation), volume: 1.0, attenuation: Some("Aircraft".into()), follow: Some(e) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region() -> Region {
        Region { centre: Vec3::new(865.41, 0.0, 891.36), radius: 1750.0, spawn_distance: 5.0 }
    }

    #[test]
    fn aircraft_start_far_out_high_up_and_head_for_the_region() {
        let r = region();
        let mut rng = rand::thread_rng();
        for _ in 0..200 {
            let (start, dir) = r.aircraft_start(&mut rng);
            assert!((ALTITUDE.0..=ALTITUDE.1).contains(&start.y));
            // X and Y within (centre ± radius) x 5.
            assert!(start.x >= (r.centre.x - r.radius) * 5.0 - 1e-2 && start.x <= (r.centre.x + r.radius) * 5.0 + 1e-2);
            assert!((dir.length() - 1.0).abs() < 1e-4 && dir.y == 0.0);
            let to_centre = (r.centre - start).with_y(0.0).normalize();
            assert!(dir.dot(to_centre) > 0.999);
        }
        assert!((r.check_every() - 23.333).abs() < 0.01);
        assert!(r.below(Vec3::new(900.0, 500.0, 900.0)));
        assert!(!r.below(Vec3::new(865.0 + 1800.0, 500.0, 891.0)));
    }

    #[test]
    fn crates_sink_at_about_ten_metres_a_second() {
        let (mut v, dt) = (0.0, 1.0 / 60.0);
        for _ in 0..(60 * 20) {
            v = fall(v, dt);
        }
        assert!((v + GRAVITY / DAMPING).abs() < 0.2, "{v}");
    }

    #[test]
    fn airdrop_containers_are_in_the_data() {
        let Ok(t) = std::fs::read_to_string("assets/data/items.json") else { return };
        let db: ItemDb = serde_json::from_str(&t).unwrap();
        for c in CLASSES {
            let d = &db.containers[c];
            assert_eq!((d.name.as_str(), d.hold), ("Airdrop", 6.0), "{c}");
            assert!(d.table.as_ref().is_some_and(|t| db.loot_tables.contains_key(t)), "{c}");
            assert!(d.airdrop.is_some(), "{c}");
        }
        assert_eq!(db.containers["AirdropContainer_Medical_C"].airdrop.as_ref().unwrap().title, "Medical Airdrop");
    }

    #[test]
    fn each_crate_smokes_in_its_colour() {
        let fx = super::super::particles::fx_data();
        if fx.systems.is_empty() {
            return;
        }
        for c in CLASSES {
            assert!(fx.systems.contains_key(smoke(c)), "{c}: {}", smoke(c));
            assert!(fx.systems[smoke(c)].emitters.iter().all(|e| e.loops == 0), "{c}: smokes until opened");
        }
        assert_ne!(smoke("AirdropContainer_Military_C"), smoke("AirdropContainer_Medical_C"));
    }
}
