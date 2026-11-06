//! Power for placed buildables: Buildable_GeneratorMaster (Small / Large / Industrial) and the
//! lights (Buildable_CeilingLight(2), _WallLight, _Lamp, _ConstructionLight, _CampingLamp), with
//! Buildable_MASTER's Power On / Power Off, decompiled. The options are in `building::interact`.
//!
//! - Generator: **Turn On** (Option1, "Turn Off" while running) needs fuel, else "Empty";
//!   **Refuel** with less than MaxFuel - 1 takes a Gas Can and fills it ("No gas can", "Gas
//!   full"); **Show Generator Area** toggles its range sphere. Running: FuelCost every 0.2 s takes
//!   0.01 (at 0 it stops), Shake every 0.02 s jitters the mesh ±0.5 cm, engine_generator_loop_01_Cue
//!   plays (2 s fade out), and every 1 s GetObjects powers on each buildable with Requires Power?
//!   inside its Collision sphere (10 / 25 / 35 m); turning off (or running dry) powers them off.
//!   A placed generator starts empty (CurrentFuel 0). The interact widget's 'Fuel' stat: "N %".
//!   **Pickup** with fuel asks first: "Removing will resort in generator losing all gas.\r\nPress
//!   again to remove." (a second press within 10 s); picking up powers its range off.
//! - Power On sets PoweredOn? (only for Requires Power? actors) and fires the On delegate; the
//!   ceiling and wall lights then light (SetIntensity 4); the lamp and the construction light
//!   light only with their own switch on too (Interact toggles LightOn; 1.5 / 4); the camping lamp
//!   needs no power, its switch alone. The 'Power' stat shows On / Off.
//!
//! Ours: "in range" is the buildable's origin within the sphere (the game's sphere trace meets
//! any of its collision); the lights' exponent falloff has no candela equivalent, so their
//! intensity is taken as candelas (a guess, like the lab lights'); the area sphere's material is
//! ours; the 1 s pickup hold isn't ported (no buildable has it here yet).

use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

use super::building::{BuildableDb, Buildables};
use crate::menus::AppState;

/// FuelCost: 0.01 every 0.2 s.
pub const FUEL_STEP: f64 = 0.01;
pub const FUEL_EVERY: f32 = 0.2;
/// GetObjects' timer while running.
pub const SCAN_EVERY: f32 = 1.0;
/// The second Pickup press's window (the Delay that resets PlayerClicked?).
pub const PICKUP_WINDOW: f32 = 10.0;
/// Shake: every 0.02 s, ±0.5 cm.
const SHAKE_EVERY: f32 = 0.02;
const SHAKE: f32 = 0.005;

/// A placed buildable's power: a generator's fuel and switch, a light's switch (saved), whether
/// a generator powers it now.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PowerState {
    /// CurrentFuel (a double, as in the game: 20000 f32 steps drift).
    #[serde(default)]
    pub fuel: f64,
    /// TurnedOn?.
    #[serde(default)]
    pub on: bool,
    /// LightOn (the lamps' own switch).
    #[serde(default)]
    pub light_on: bool,
    /// PoweredOn?.
    #[serde(skip)]
    pub powered: bool,
    /// The range sphere shown (Show Generator Area).
    #[serde(skip)]
    pub area: bool,
    /// Seconds left of the second-press window after the fuel warning.
    #[serde(skip)]
    pub warned: f32,
    #[serde(skip)]
    fuel_timer: f32,
    #[serde(skip)]
    scan_timer: f32,
}

impl PowerState {
    pub fn is_default(&self) -> bool {
        self.fuel == 0.0 && !self.on && !self.light_on
    }

    /// Turn On / Turn Off: `Err("Empty")` without fuel.
    pub fn toggle(&mut self) -> Result<bool, &'static str> {
        if self.on {
            self.on = false;
        } else if self.fuel != 0.0 {
            self.on = true;
            // K2_SetTimer(GetObjects, 1.0, looping): the first scan a second later.
            self.scan_timer = SCAN_EVERY;
            self.fuel_timer = 0.0;
        } else {
            return Err("Empty");
        }
        Ok(self.on)
    }

    /// Refuel: whether a Gas Can is wanted (CurrentFuel <= MaxFuel - 1).
    pub fn wants_fuel(&self, max: f32) -> bool {
        self.fuel <= max as f64 - 1.0
    }

    /// FuelCost and GetObjects' timers over `dt`: (ran dry, scan now).
    pub fn run(&mut self, dt: f32) -> (bool, bool) {
        if !self.on {
            return (false, false);
        }
        self.fuel_timer += dt;
        let mut dry = false;
        while self.fuel_timer >= FUEL_EVERY && self.on {
            self.fuel_timer -= FUEL_EVERY;
            self.fuel -= FUEL_STEP;
            if self.fuel <= 0.0 {
                self.fuel = 0.0;
                self.on = false;
                dry = true;
            }
        }
        self.scan_timer -= dt;
        let scan = self.on && self.scan_timer <= 0.0;
        if scan {
            self.scan_timer += SCAN_EVERY;
        }
        (dry, scan)
    }
}

/// Whether a light's lights are lit: powered (or not needing power) and switched on (or having
/// no switch).
pub fn lit(requires_power: bool, switch: bool, p: &PowerState) -> bool {
    (!requires_power || p.powered) && (!switch || p.light_on)
}

/// "N %" (Conv_DoubleToText, no fractional digits).
pub fn fuel_text(fuel: f64, max: f32) -> String {
    format!("{} %", (fuel / max.max(1.0) as f64 * 100.0).round() as i64)
}

/// A light of a placed buildable (its index in `ActorDef::lights`).
#[derive(Component)]
pub struct BuildLight(pub usize);

/// Buildable_MASTER's StaticMesh part (the generator shakes it).
#[derive(Component)]
pub struct MainMesh(pub Vec3);

/// A generator's range sphere.
#[derive(Component)]
pub struct AreaSphere;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (run_generators, light_up, shake).chain().after(super::building::BuildSet).run_if(in_state(AppState::InGame)));
}

/// Power On / Power Off on the buildables needing power within `radius` of `at`.
pub fn power_range(placed: &mut Buildables, db: &BuildableDb, at: Vec3, radius: f32, on: bool) {
    for l in placed.list.iter_mut() {
        if db.actors.get(&l.actor).is_some_and(|d| d.requires_power) && l.transform.translation.distance(at) <= radius {
            l.power.powered = on;
        }
    }
}

/// Fuel, the 1 s power scan, dry generators powering off; the hum and the area sphere.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn run_generators(
    mut commands: Commands,
    time: Res<Time>,
    db: Res<BuildableDb>,
    mut placed: ResMut<Buildables>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut hums: Query<&mut crate::audio::AmbientLoop>,
    spheres: Query<(Entity, &ChildOf), With<AreaSphere>>,
) {
    let dt = time.delta_secs();
    let mut changes = Vec::new();
    for l in placed.list.iter_mut() {
        let Some(g) = db.actors.get(&l.actor).and_then(|d| d.generator.as_ref()) else { continue };
        l.power.warned = (l.power.warned - dt).max(0.0);
        let (dry, scan) = l.power.run(dt);
        if dry {
            info!("generators: {} ran dry", l.item);
            changes.push((l.transform.translation, g.radius, false));
        } else if scan {
            changes.push((l.transform.translation, g.radius, true));
        }
        // The AudioComponent, on while running.
        match hums.get_mut(l.entity) {
            Ok(mut h) => {
                if h.active != l.power.on {
                    h.active = l.power.on;
                }
            }
            Err(_) => {
                commands.entity(l.entity).insert(crate::audio::AmbientLoop { cue: "engine_generator_loop_01_Cue".into(), volume: 1.0, active: l.power.on });
            }
        }
        // Show Generator Area: the Sphere component (engine Sphere x radius / 50).
        let shown = spheres.iter().find(|(_, p)| p.parent() == l.entity).map(|(e, _)| e);
        match (l.power.area, shown) {
            (true, None) => {
                let mesh = meshes.add(Sphere::new(g.radius).mesh().uv(32, 16));
                let mat = materials.add(StandardMaterial { base_color: Color::srgba(0.3, 0.7, 1.0, 0.12), alpha_mode: AlphaMode::Blend, unlit: true, cull_mode: None, double_sided: true, ..default() });
                commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::default(), AreaSphere, ChildOf(l.entity)));
            }
            (false, Some(e)) => commands.entity(e).despawn(),
            _ => {}
        }
    }
    for (at, r, on) in changes {
        power_range(&mut placed, &db, at, r, on);
    }
}

/// The lights: on as `lit` says, at their intensity (taken as candelas) and radius.
fn light_up(db: Res<BuildableDb>, placed: Res<Buildables>, parents: Query<&ChildOf>, mut points: Query<(Entity, &BuildLight, Option<&mut PointLight>, Option<&mut SpotLight>)>) {
    for (e, BuildLight(i), point, spot) in &mut points {
        let Ok(root) = parents.get(e).map(|p| p.parent()) else { continue };
        let Some(l) = placed.list.iter().find(|l| l.entity == root) else { continue };
        let Some(d) = db.actors.get(&l.actor) else { continue };
        let Some(def) = d.lights.get(*i) else { continue };
        let want = if lit(d.requires_power, d.switch, &l.power) { crate::level::lights::lumens(def.intensity) } else { 0.0 };
        if let Some(mut p) = point
            && p.intensity != want
        {
            p.intensity = want;
        }
        if let Some(mut s) = spot
            && s.intensity != want
        {
            s.intensity = want;
        }
    }
}

/// Shake while running: the StaticMesh part at its place ± 0.5 cm, every 0.02 s.
fn shake(time: Res<Time>, placed: Res<Buildables>, parents: Query<&ChildOf>, mut parts: Query<(Entity, &MainMesh, &mut Transform)>, mut every: Local<f32>) {
    *every -= time.delta_secs();
    let tick = *every <= 0.0;
    if tick {
        *every = SHAKE_EVERY;
    }
    let mut rng = rand::thread_rng();
    for (e, MainMesh(rest), mut t) in &mut parts {
        let Ok(root) = parents.get(e).map(|p| p.parent()) else { continue };
        let running = placed.list.iter().any(|l| l.entity == root && l.power.on);
        if !running {
            if t.translation != *rest {
                t.translation = *rest;
            }
        } else if tick {
            // UE X / Y are glb X / Z.
            t.translation = *rest + Vec3::new(rng.gen_range(-SHAKE..=SHAKE), 0.0, rng.gen_range(-SHAKE..=SHAKE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuel_burns_and_runs_dry() {
        let mut p = PowerState::default();
        assert_eq!(p.toggle(), Err("Empty"));
        p.fuel = 0.1;
        assert_eq!(p.toggle(), Ok(true));
        // The first scan a second in.
        assert_eq!(p.run(0.5), (false, false));
        assert_eq!(p.run(0.5), (false, true));
        // 0.01 per 0.2 s: 0.1 lasts 2 s (a tick more when rounding leaves a sliver).
        let mut dry = false;
        for _ in 0..13 {
            dry |= p.run(0.1).0;
        }
        assert!(dry && !p.on && p.fuel == 0.0, "{p:?}");
        assert_eq!(p.run(1.0), (false, false));
        // 200 fuel at 0.05 / s: 4000 s.
        let mut p = PowerState { fuel: 200.0, ..default() };
        p.toggle().unwrap();
        let mut secs = 0;
        while p.on && secs < 5000 {
            p.run(1.0);
            secs += 1;
        }
        assert!((3999..=4001).contains(&secs), "{secs}");
        assert!(p.wants_fuel(200.0) && !PowerState { fuel: 199.5, ..default() }.wants_fuel(200.0));
    }

    #[test]
    fn lights_need_power_and_switch() {
        let mut p = PowerState::default();
        assert!(!lit(true, false, &p) && !lit(false, true, &p) && lit(false, false, &p));
        p.powered = true;
        assert!(lit(true, false, &p) && !lit(true, true, &p));
        p.light_on = true;
        assert!(lit(true, true, &p) && lit(false, true, &p));
        assert_eq!(fuel_text(100.0, 200.0), "50 %");
        assert_eq!(fuel_text(0.0, 200.0), "0 %");
    }

    #[test]
    fn data() {
        let db = BuildableDb::load();
        let g = |a: &str| db.actors[a].generator.as_ref().map(|g| (g.max_fuel, g.radius));
        assert_eq!(g("Buildable_SmallGenerator_C"), Some((200.0, 10.0)));
        assert_eq!(g("Buildable_LargeGenerator_C"), Some((400.0, 25.0)));
        assert_eq!(g("Buildable_IndustrialGenerator_C"), Some((800.0, 35.0)));
        let a = &db.actors["Buildable_CeilingLight_C"];
        assert!(a.requires_power && !a.switch && a.lights.len() == 1 && a.lights[0].intensity == 4.0);
        let a = &db.actors["Buildable_Lamp_C"];
        assert!(a.requires_power && a.switch && a.lights[0].intensity == 1.5);
        let a = &db.actors["Buildable_CampingLamp_C"];
        assert!(!a.requires_power && a.switch);
        let a = &db.actors["Buildable_ConstructionLight_C"];
        assert!(a.lights.len() == 2 && a.lights.iter().all(|l| l.spot && (l.cone - 50.0).abs() < 1e-3));
    }
}
