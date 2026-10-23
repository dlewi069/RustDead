//! Weather, from LongdownValley's `Ultra_Dynamic_Weather` and the UDS weather presets
//! (`assets/data/weather.json`, `extractor/gen_weather.py`):
//!
//! - **Which weather**: a random preset when the game begins (Begin Play Weather is Random), then
//!   a new one each day at the change hour (Random Weather Variation: Daily, hour 8), drawn from
//!   the current season's Weather Type Probabilities and faded in over Transition Length (0.25
//!   game hours). The `Weather_Override_Volume` (the map's southern corner) forces its preset
//!   (Foggy) inside, fading over its Transition Width (75 m).
//! - **What it does** (UDS's own shading isn't ported; the mapping from its 0-10 values is ours):
//!   cloud coverage dims the sun and greys the sky, fog and dust pull the fog in (dust tints it
//!   the level's Dust Particle Color), rain and snow fall around the camera except under a roof
//!   (UDS_Occlusion_Volume's job), thunder flashes the sky in Lightning Flash colour.
//! - **Lightning** (UDW_Lightning_Spawn_Manager's Timed Lightning Spawn): every Lightning Flash
//!   Period (60 / Lightning Flash Frequency 14 x (1 ± 0.5 x Timing Randomization 1) s) a
//!   threshold is rolled in 5-10, and a flash happens when Thunder/Lightning is above it, at a
//!   random distance in Lightning Flash Distance Range (1.55-6.5 km; uniformly, a guess). Its
//!   brightness is Lightning Distance Range's 1 -> 0 over that range, clamped to 0.15-1.
//!   [`LightningFlash`] tells `weather_sounds` (the close thunder).

use std::sync::OnceLock;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::menus::{AppState, CurrentGame, MenuScreen};

/// One UDS_Weather_Settings preset (0-10 scales; material values 0-1).
#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct WeatherValues {
    pub cloud: f32,
    pub rain: f32,
    pub snow: f32,
    pub thunder: f32,
    pub wind: f32,
    pub fog: f32,
    pub dust: f32,
    pub wetness: f32,
    pub snow_cover: f32,
    pub dust_cover: f32,
}

impl WeatherValues {
    pub fn lerp(&self, o: &Self, t: f32) -> Self {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Self {
            cloud: l(self.cloud, o.cloud),
            rain: l(self.rain, o.rain),
            snow: l(self.snow, o.snow),
            thunder: l(self.thunder, o.thunder),
            wind: l(self.wind, o.wind),
            fog: l(self.fog, o.fog),
            dust: l(self.dust, o.dust),
            wetness: l(self.wetness, o.wetness),
            snow_cover: l(self.snow_cover, o.snow_cover),
            dust_cover: l(self.dust_cover, o.dust_cover),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
struct Preset {
    name: String,
    #[serde(flatten)]
    values: WeatherValues,
}

#[derive(Deserialize, Clone, Debug)]
struct Override {
    weather: Option<String>,
    transition: f32,
    triangles: Vec<[f32; 2]>,
}

#[derive(Deserialize, Debug)]
struct WeatherData {
    presets: std::collections::HashMap<String, Preset>,
    seasons: std::collections::HashMap<String, Vec<(String, f32)>>,
    timing: String,
    change_hour: f32,
    transition_hours: f32,
    random_start: bool,
    overrides: Vec<Override>,
}

fn data() -> Option<&'static WeatherData> {
    static DATA: OnceLock<Option<WeatherData>> = OnceLock::new();
    DATA.get_or_init(|| std::fs::read_to_string("assets/data/weather.json").ok().and_then(|t| serde_json::from_str(&t).ok())).as_ref()
}

/// The weather saved with the game (Ultra_Dynamic_Weather's current/previous preset).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WeatherSave {
    pub current: String,
    pub previous: String,
    /// 0..1 through the transition from `previous` to `current`.
    pub blend: f32,
    /// The day the last daily change happened.
    pub changed_on: Option<u32>,
}

/// Draw a preset from a season's probabilities.
pub fn pick(season: &str, roll: f32) -> Option<String> {
    let table = data()?.seasons.get(season)?;
    let total: f32 = table.iter().map(|e| e.1).sum();
    let mut x = roll * total;
    for (name, p) in table {
        if x < *p {
            return Some(name.clone());
        }
        x -= p;
    }
    table.last().map(|e| e.0.clone())
}

fn values(name: &str) -> WeatherValues {
    data().and_then(|d| d.presets.get(name)).map(|p| p.values).unwrap_or_default()
}

pub fn display_name(name: &str) -> &str {
    data().and_then(|d| d.presets.get(name)).map_or(name, |p| p.name.as_str())
}

/// How far inside the override volumes `p` (x, z) is: 1 inside, fading to 0 over the transition.
fn override_weight(o: &Override, p: Vec2) -> f32 {
    let mut nearest = f32::MAX;
    for [a, b, c] in o.triangles.as_chunks::<3>().0 {
        let (a, b, c) = (Vec2::from(*a), Vec2::from(*b), Vec2::from(*c));
        let d = |p: Vec2, a: Vec2, b: Vec2| (b - a).perp_dot(p - a);
        let (d1, d2, d3) = (d(p, a, b), d(p, b, c), d(p, c, a));
        let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        if !(neg && pos) {
            return 1.0;
        }
        for (s, e) in [(a, b), (b, c), (c, a)] {
            let t = ((p - s).dot(e - s) / (e - s).length_squared().max(1e-6)).clamp(0.0, 1.0);
            nearest = nearest.min(p.distance(s + (e - s) * t));
        }
    }
    if o.transition <= 0.0 { 0.0 } else { (1.0 - nearest / o.transition).max(0.0) }
}

/// The weather right now, after transitions and override volumes.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Weather(pub WeatherValues);

#[derive(Component)]
struct Drop {
    snow: bool,
}

#[derive(Resource)]
struct Precipitation {
    rain_mesh: Handle<Mesh>,
    snow_mesh: Handle<Mesh>,
    rain_mat: Handle<StandardMaterial>,
    snow_mat: Handle<StandardMaterial>,
    roof_check: f32,
    flash: f32,
    /// Its brightness (Current Lightning Intensity).
    flash_intensity: f32,
    /// Seconds to the next Timed Lightning Spawn.
    next_lightning: f32,
}

/// The camera has a roof over it (UDS's occlusion: no rain falls, weather sounds are muffled).
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct Sheltered(pub bool);

/// A lightning flash `distance` metres away (Start Lightning Flash).
#[derive(Message, Clone, Copy, Debug)]
pub struct LightningFlash {
    pub distance: f32,
}

/// Lightning Flash Frequency (flashes a minute) and Timing Randomization.
const FLASH_FREQUENCY: f32 = 14.0;
const FLASH_RANDOMIZATION: f32 = 1.0;
/// Lightning Flash Distance Range (UE 155000-650000 cm).
pub const FLASH_DISTANCE: [f32; 2] = [1550.0, 6500.0];

/// Lightning Flash Period: 60 / frequency x (1 + randomization x rand(-0.5, 0.5)).
pub fn flash_period(rng: &mut impl Rng) -> f32 {
    60.0 / FLASH_FREQUENCY * (1.0 + FLASH_RANDOMIZATION * rng.gen_range(-0.5..0.5))
}

/// Timed Lightning Spawn's check: a flash when Thunder/Lightning beats a threshold rolled in 5-10.
pub fn flash_strikes(thunder: f32, rng: &mut impl Rng) -> bool {
    thunder > rng.gen_range(5.0..10.0)
}

/// Lightning Distance Range, clamped to 0.15-1: a flash's brightness at `distance` metres.
pub fn flash_intensity(distance: f32) -> f32 {
    (1.0 - (distance - FLASH_DISTANCE[0]) / (FLASH_DISTANCE[1] - FLASH_DISTANCE[0])).clamp(0.15, 1.0)
}

const MAX_DROPS: usize = 2500;
/// Precipitation fills a box this far around the camera.
const BOX: f32 = 14.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<Weather>()
        .init_resource::<Sheltered>()
        .add_message::<LightningFlash>()
        .add_systems(OnEnter(AppState::InGame), setup_precipitation)
        .add_systems(Update, update_weather.run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)))
        .add_systems(
            Update,
            (apply_weather.after(super::daynight::LightTheWorld), fall).run_if(in_state(AppState::InGame)),
        );
}

fn setup_precipitation(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let drop = |c: Color| StandardMaterial { base_color: c, unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, ..default() };
    commands.insert_resource(Precipitation {
        rain_mesh: meshes.add(Rectangle::new(0.012, 0.55)),
        snow_mesh: meshes.add(Rectangle::new(0.04, 0.04)),
        rain_mat: materials.add(drop(Color::srgba(0.75, 0.8, 0.88, 0.35))),
        snow_mat: materials.add(drop(Color::srgba(0.95, 0.95, 1.0, 0.85))),
        roof_check: 0.0,
        flash: 0.0,
        flash_intensity: 1.0,
        next_lightning: flash_period(&mut rand::thread_rng()),
    });
}

/// Daily changes and the transition between presets, on the game clock.
fn update_weather(mut game: ResMut<CurrentGame>, mut last_hour: Local<Option<f32>>) {
    let Some(d) = data() else { return };
    let Some(state) = game.state.as_mut() else { return };
    let env = &state.environment;
    let (hour, day) = (env.time_of_day_hours, env.days);
    let season = super::daynight::season(super::daynight::date(day).1);
    let w = &mut state.weather;
    let mut rng = rand::thread_rng();
    if w.current.is_empty() {
        w.current = if d.random_start { pick(season, rng.gen_range(0.0..1.0)).unwrap_or_else(|| "Clear_Skies".into()) } else { "Clear_Skies".into() };
        w.previous = w.current.clone();
        w.blend = 1.0;
        w.changed_on = Some(day);
    }
    if d.timing == "Daily" && hour >= d.change_hour && w.changed_on != Some(day) {
        w.changed_on = Some(day);
        if let Some(next) = pick(season, rng.gen_range(0.0..1.0)) {
            w.previous = std::mem::replace(&mut w.current, next);
            w.blend = 0.0;
        }
    }
    // Game hours passed this frame (the clock wraps at midnight).
    let dh = last_hour.map_or(0.0, |h| (hour - h).rem_euclid(24.0));
    *last_hour = Some(hour);
    if w.blend < 1.0 {
        w.blend = (w.blend + dh.min(1.0) / d.transition_hours.max(1e-3)).min(1.0);
    }
}

/// Fog, clouds and lightning on top of the day/night light.
#[allow(clippy::too_many_arguments)]
fn apply_weather(
    time: Res<Time>,
    game: Res<CurrentGame>,
    mut weather: ResMut<Weather>,
    mut sun: Query<&mut DirectionalLight, With<super::daynight::Sun>>,
    mut fog: Query<&mut DistanceFog>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    players: Query<&Transform, With<crate::models::CharacterModel>>,
    precip: Option<ResMut<Precipitation>>,
    mut flashes: MessageWriter<LightningFlash>,
) {
    let (Some(d), Some(state)) = (data(), &game.state) else { return };
    let w = &state.weather;
    if w.current.is_empty() {
        return;
    }
    let mut v = values(&w.previous).lerp(&values(&w.current), w.blend);
    if let Some(p) = players.iter().next() {
        for o in &d.overrides {
            let k = override_weight(o, p.translation.xz());
            if k > 0.0 {
                v = v.lerp(&values(o.weather.as_deref().unwrap_or("Foggy")), k);
            }
        }
    }
    weather.0 = v;
    let (cloud, fog_k, dust) = (v.cloud / 10.0, v.fog / 10.0, v.dust / 10.0);
    for mut s in &mut sun {
        s.illuminance *= 1.0 - 0.8 * cloud.powf(1.5);
    }
    let grey = |c: Color, k: f32| {
        let l = c.to_linear();
        let lum = 0.2126 * l.red + 0.7152 * l.green + 0.0722 * l.blue;
        Color::LinearRgba(l.mix(&LinearRgba::rgb(lum, lum, lum * 1.05), k))
    };
    // The level's Dust Particle Color.
    let dust_colour = |c: Color| {
        let l = c.to_linear();
        let lum = (0.2126 * l.red + 0.7152 * l.green + 0.0722 * l.blue).max(0.02);
        Color::LinearRgba(l.mix(&(LinearRgba::rgb(0.510417, 0.498645, 0.31901) * (lum / 0.5)), dust))
    };
    ambient.brightness *= 1.0 - 0.3 * cloud;
    let sky = dust_colour(grey(clear.0, 0.8 * cloud));
    clear.0 = sky;
    // Fog and dust pull the fog distance in from the clear-day 250-520 m.
    let vis = 1.0 / (1.0 + 4.0 * fog_k + 12.0 * dust);
    for mut f in &mut fog {
        f.color = sky;
        f.falloff = FogFalloff::Linear { start: 250.0 * vis * vis, end: 520.0 * vis };
    }
    // Thunder: brief flashes in Lightning Flash Light Source Color (CCD9FF).
    if let Some(mut p) = precip {
        p.flash = (p.flash - time.delta_secs()).max(0.0);
        p.next_lightning -= time.delta_secs();
        if p.next_lightning <= 0.0 {
            let mut rng = rand::thread_rng();
            p.next_lightning = flash_period(&mut rng);
            if flash_strikes(v.thunder, &mut rng) {
                let distance = rng.gen_range(FLASH_DISTANCE[0]..FLASH_DISTANCE[1]);
                p.flash = 0.18;
                p.flash_intensity = flash_intensity(distance);
                flashes.write(LightningFlash { distance });
                debug!("lightning {distance:.0} m away");
            }
        }
        if p.flash > 0.0 {
            ambient.brightness += 4000.0 * p.flash_intensity * (p.flash / 0.18);
            ambient.color = Color::linear_rgb(0.598958, 0.692534, 1.0);
        }
    }
}

/// Rain and snow around the camera: a pool of drops recycled inside a box that follows it.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn fall(
    mut commands: Commands,
    time: Res<Time>,
    weather: Res<Weather>,
    precip: Option<ResMut<Precipitation>>,
    mut sheltered: ResMut<Sheltered>,
    camera: Query<&GlobalTransform, With<super::PlayerCamera>>,
    mut drops: Query<(&Drop, &mut Transform, &mut Visibility)>,
    mut props: crate::level::collision::PropRays,
) {
    let (Some(mut p), Ok(cam)) = (precip, camera.single()) else { return };
    let eye = cam.translation();
    // Under a roof: no rain (UDS_Occlusion_Volume covers interiors in the game).
    p.roof_check -= time.delta_secs();
    if p.roof_check <= 0.0 {
        p.roof_check = 0.25;
        sheltered.set_if_neq(Sheltered(props.cast(eye, Vec3::Y, 30.0).is_some()));
    }
    let v = weather.0;
    let want_rain = if sheltered.0 { 0 } else { (v.rain / 10.0 * MAX_DROPS as f32) as usize };
    let want_snow = if sheltered.0 { 0 } else { (v.snow / 10.0 * MAX_DROPS as f32) as usize };
    let snow_have = drops.iter().filter(|d| d.0.snow).count();
    let rain_have = drops.iter().count() - snow_have;
    let mut rng = rand::thread_rng();
    // Grow each kind's pool to what's falling now (extra drops are hidden, not despawned).
    let new = std::iter::repeat_n(false, want_rain.saturating_sub(rain_have)).chain(std::iter::repeat_n(true, want_snow.saturating_sub(snow_have)));
    for snow in new {
        let pos = eye + Vec3::new(rng.gen_range(-BOX..BOX), rng.gen_range(-BOX..BOX), rng.gen_range(-BOX..BOX));
        commands.spawn((
            Mesh3d(if snow { p.snow_mesh.clone() } else { p.rain_mesh.clone() }),
            MeshMaterial3d(if snow { p.snow_mat.clone() } else { p.rain_mat.clone() }),
            Transform::from_translation(pos),
            NotShadowCaster,
            Drop { snow },
            DespawnOnExit(AppState::InGame),
        ));
    }
    let dt = time.delta_secs();
    let wind = Vec3::new(1.0, 0.0, 0.4).normalize() * v.wind * 0.35;
    let (mut rain_n, mut snow_n) = (0usize, 0usize);
    for (d, mut t, mut vis) in &mut drops {
        let n = if d.snow { &mut snow_n } else { &mut rain_n };
        *n += 1;
        let active = *n <= if d.snow { want_snow } else { want_rain };
        if !active {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        vis.set_if_neq(Visibility::Inherited);
        let speed = if d.snow { Vec3::new(0.0, -1.2, 0.0) + wind * 1.5 } else { Vec3::new(0.0, -9.0, 0.0) + wind };
        t.translation += speed * dt;
        // Wrap inside the box around the camera.
        let rel = t.translation - eye;
        let wrap = |x: f32| if x < -BOX { x + 2.0 * BOX } else if x > BOX { x - 2.0 * BOX } else { x };
        t.translation = eye + Vec3::new(wrap(rel.x), wrap(rel.y), wrap(rel.z));
        // Face the camera around the vertical (streaks stay upright, tilted by the wind).
        let to_cam = (eye - t.translation) * Vec3::new(1.0, 0.0, 1.0);
        let yaw = to_cam.x.atan2(to_cam.z);
        t.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(if d.snow { 0.0 } else { (wind.x / 9.0).atan() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasons_draw_from_the_levels_probabilities() {
        let Some(d) = data() else { return };
        // Summer: Clear Skies 2 of 9.07 total weight comes first.
        assert_eq!(pick("Summer", 0.0).as_deref(), Some("Clear_Skies"));
        assert_eq!(pick("Summer", 0.999).as_deref(), Some("Radiation"));
        // No snow in summer beyond Light Snow's 0.01; blizzards only in autumn and winter.
        assert!(d.seasons["Summer"].iter().all(|(n, _)| n != "Snow_Blizzard"));
        assert!(d.seasons["Winter"].iter().any(|(n, _)| n == "Snow_Blizzard"));
        let rain = values("Rain_Thunderstorm");
        assert_eq!((rain.rain, rain.thunder, rain.wind), (10.0, 10.0, 10.0));
        assert_eq!(display_name("Rain_Thunderstorm"), "Thunderstorm");
        assert_eq!((d.change_hour, d.transition_hours, d.timing.as_str()), (8.0, 0.25, "Daily"));
    }

    #[test]
    fn override_volume_fades_over_its_transition() {
        let o = Override { weather: Some("Foggy".into()), transition: 75.0, triangles: vec![[0.0, 0.0], [100.0, 0.0], [0.0, 100.0]] };
        assert_eq!(override_weight(&o, Vec2::new(10.0, 10.0)), 1.0);
        assert!((override_weight(&o, Vec2::new(-37.5, 50.0)) - 0.5).abs() < 1e-4);
        assert_eq!(override_weight(&o, Vec2::new(-100.0, 50.0)), 0.0);
    }

    #[test]
    fn transitions_blend_every_value() {
        let a = values("Clear_Skies");
        let b = values("Rain");
        let m = a.lerp(&b, 0.5);
        assert_eq!(m.rain, b.rain / 2.0);
        assert_eq!(m.cloud, (a.cloud + b.cloud) / 2.0);
    }

    #[test]
    fn lightning_follows_the_spawn_manager() {
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let p = flash_period(&mut rng);
            assert!((60.0 / 14.0 * 0.5..=60.0 / 14.0 * 1.5).contains(&p), "{p}");
        }
        // The threshold is rolled in 5-10: a full thunderstorm always strikes, 5 never does.
        assert!((0..100).all(|_| flash_strikes(10.0, &mut rng)));
        assert!((0..100).all(|_| !flash_strikes(5.0, &mut rng)));
        assert_eq!(flash_intensity(1550.0), 1.0);
        assert_eq!(flash_intensity(6500.0), 0.15);
        assert!((flash_intensity(4025.0) - 0.5).abs() < 1e-4);
    }
}
