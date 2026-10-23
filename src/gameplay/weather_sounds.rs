//! Weather sounds: Ultra_Dynamic_Weather's two sound components and their MetaSounds (read with
//! `extractor/metasound.py`; waves in sounds.json's `weather`, gen_sounds.py).
//!
//! - **Rain** (UDS_Rain_Loop, four of them in UDS_Directional_WeatherSounds' two components, wave
//!   indices 0-3): a light loop at MapRange(|Rain Intensity - 5|, 0-4.99 -> 0.3-0) and a heavy
//!   one at MapRange(intensity, 3-10 -> 0-0.5), each started at a random 1-5 s into its wave
//!   once its volume passes 0.001 and stopped below; in very light rain, RainHit drips every
//!   0.2-1.5 s at 0.3-1 x MapRange(0.001-0.5 -> 0-1) x MapRange(1-3 -> 1-0) x 0.07, pitched ±2
//!   semitones. The graph's output is x 0.75.
//! - **Wind** (UDS_Wind_Loop, four likewise): a Brownian noise loop at 0.5 x 0.8 x (Wind
//!   Intensity / 10)^2.8, its amplitude wandering to a new 0.7-1 every Amplitude Period and its
//!   pitch to a new ±3 semitones every Pitch Period (1.1 / 1.0 s and 0.75 / 0.55 s in the
//!   directional graph), each glide taking that period.
//! - **Global** (UDS_Global_WeatherSounds): Wind_Whistling at (MapRange(wind, 6-10 -> 0-1))^2 x
//!   0.7, pitch wandering -5..+2 semitones every 0.8 s, started 0-15 s in; distant thunder
//!   (UDS_Distant_Thunder_Loop): while Thunder/Lightning > 0.01, two players each take a random
//!   DistantThunder (not one of the last two) every 17 s, the second starting 8 s after the
//!   first, at (Thunder / 10)^1.6; close thunder (UDS_Close_Thunder, from each lightning flash):
//!   a random CloseThunder (not one of the last five) after Close Thunder Sound Delay (0.15 s per
//!   km), at Close Thunder Volume x MapRange(distance, 1.55-6.5 km -> 1.3-0.4).
//! - Volumes: every weather volume setting is 1 on the level's Ultra_Dynamic_Weather; the
//!   MetaSounds are AmbientAudio (the AMBIENT slider). Under a roof ([`super::weather::Sheltered`])
//!   everything is x Max Attenuation for UDS Outdoor Sound (0.1), the occluded end of the graphs'
//!   attenuation.
//!
//! Not ported: stereo panning by wind direction ("Wind Lean"), the occlusion's low-pass filter
//! (Min LPF Frequency 700 Hz) and the wind's 3-8 kHz one, per-direction occlusion, the
//! InterpTo smoothing of the inputs (the weather already fades between presets), puddle splashes.

use std::collections::VecDeque;

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use rand::Rng;

use super::weather::{FLASH_DISTANCE, LightningFlash, Sheltered, Weather};
use crate::audio::{Played, SoundDb, class_volume};
use crate::menus::AppState;
use crate::menus::settings::SettingsStore;

/// Max Attenuation for UDS Outdoor Sound: the occluded volume.
pub const SHELTERED: f32 = 0.1;
/// Close Thunder Delay Per KM.
const THUNDER_DELAY_PER_KM: f32 = 0.15;
/// UDS_Distant_Thunder_Loop's TriggerRepeat period and the second player's Trigger Delay.
const DISTANT_PERIOD: f32 = 17.0;
const DISTANT_OFFSET: f32 = 8.0;

/// MetaSound MapRange (clamped): `x` from `a0..a1` onto `b0..b1`.
pub fn map_range(x: f32, [a0, a1]: [f32; 2], [b0, b1]: [f32; 2]) -> f32 {
    let t = if a1 == a0 { 0.0 } else { ((x - a0) / (a1 - a0)).clamp(0.0, 1.0) };
    b0 + (b1 - b0) * t
}

/// UDS_Rain_Loop's volumes at a rain intensity (0-10): (light loop, heavy loop, drips), before
/// the graph's x 0.75.
pub fn rain_volumes(intensity: f32) -> (f32, f32, f32) {
    let light = map_range((intensity - 5.0).abs(), [0.0, 4.99], [0.3, 0.0]);
    let heavy = map_range(intensity, [3.0, 10.0], [0.0, 0.5]);
    let hits = map_range(intensity, [0.001, 0.5], [0.0, 1.0]) * map_range(intensity, [1.0, 3.0], [1.0, 0.0]) * 0.07;
    (light, heavy, hits)
}

/// UDS_Wind_Loop's gain at a wind intensity, for an amplitude wander of `amp` (0.7-1).
pub fn wind_volume(intensity: f32, amp: f32) -> f32 {
    0.5 * amp * 0.8 * (intensity / 10.0).max(0.0).powf(2.8)
}

/// UDS_Global_WeatherSounds' Wind_Whistling volume.
pub fn whistle_volume(intensity: f32) -> f32 {
    map_range(intensity, [6.0, 10.0], [0.0, 1.0]).powi(2) * 0.7
}

/// UDS_Distant_Thunder_Loop's gain at a Thunder/Lightning value.
pub fn distant_thunder_volume(thunder: f32) -> f32 {
    (thunder / 10.0).max(0.0).powf(1.6)
}

/// Close thunder for a flash `distance` metres away: (delay, volume).
pub fn close_thunder(distance: f32) -> (f32, f32) {
    (distance / 1000.0 * THUNDER_DELAY_PER_KM, map_range(distance, FLASH_DISTANCE, [1.3, 0.4]))
}

/// A semitone shift as a playback speed.
fn semitones(st: f32) -> f32 {
    2f32.powf(st / 12.0)
}

/// RandomFloat on a TriggerRepeat into InterpTo: a value gliding to a new random target in
/// `range` every `period`, over `glide` seconds.
#[derive(Clone, Copy, Debug)]
pub struct Wander {
    pub value: f32,
    from: f32,
    target: f32,
    t: f32,
    period: f32,
    glide: f32,
    range: [f32; 2],
}

impl Wander {
    pub fn new(range: [f32; 2], period: f32, glide: f32, start: f32) -> Self {
        Self { value: start, from: start, target: start, t: period, period, glide, range }
    }

    pub fn step(&mut self, dt: f32, rng: &mut impl Rng) -> f32 {
        self.t += dt;
        if self.t >= self.period {
            self.t -= self.period;
            self.from = self.value;
            self.target = rng.gen_range(self.range[0]..=self.range[1]);
        }
        let k = (self.t / self.glide.max(1e-3)).min(1.0);
        self.value = self.from + (self.target - self.from) * k;
        self.value
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoopKind {
    LightRain,
    HeavyRain,
    Wind,
    Whistle,
}

impl LoopKind {
    /// Its name in `Played` (the tour's sound log: no spaces).
    fn id(self) -> &'static str {
        match self {
            LoopKind::LightRain => "UDS_Rain_Loop.Light",
            LoopKind::HeavyRain => "UDS_Rain_Loop.Heavy",
            LoopKind::Wind => "UDS_Wind_Loop",
            LoopKind::Whistle => "Wind_Whistling",
        }
    }

    fn waves(self) -> &'static str {
        match self {
            LoopKind::LightRain => "UDS_Rain_Loop.Light Rain Sounds",
            LoopKind::HeavyRain => "UDS_Rain_Loop.Heavy Rain Sounds",
            LoopKind::Wind => "UDS_Wind_Loop.Wind Sounds",
            LoopKind::Whistle => "UDS_Global_WeatherSounds.Wave Player",
        }
    }
}

/// A playing weather loop.
#[derive(Component)]
struct WeatherLoop {
    kind: LoopKind,
    index: usize,
}

/// A playing thunder (its volume follows the thunder value while it plays).
#[derive(Component)]
struct DistantThunder;

/// The weather sounds' state: wanders, timers, recent picks.
#[derive(Resource)]
struct WeatherSounds {
    /// Per wind loop: (amplitude, pitch).
    wind: [(Wander, Wander); 4],
    whistle_pitch: Wander,
    /// The two distant-thunder players' countdowns (None: stopped).
    distant: [Option<f32>; 2],
    recent_distant: VecDeque<usize>,
    recent_close: VecDeque<usize>,
    /// Close thunders waiting out their delay: (seconds left, volume).
    close: Vec<(f32, f32)>,
    /// Per rain loop, seconds to its next drip.
    drips: [f32; 4],
}

impl Default for WeatherSounds {
    fn default() -> Self {
        // The directional graph's two wind loops per component: Amplitude Period 1.1 / 1.0,
        // Pitch Period 0.75 / 0.55.
        let wind = |amp: f32, pitch: f32| (Wander::new([0.7, 1.0], amp, amp, 0.85), Wander::new([-3.0, 3.0], pitch, pitch, 0.0));
        Self {
            wind: [wind(1.1, 0.75), wind(1.0, 0.55), wind(1.1, 0.75), wind(1.0, 0.55)],
            whistle_pitch: Wander::new([-5.0, 2.0], 0.8, 0.8, 0.0),
            distant: [None, None],
            recent_distant: VecDeque::new(),
            recent_close: VecDeque::new(),
            close: Vec::new(),
            drips: [0.0; 4],
        }
    }
}

/// Array Random Get with No Repeats: an index not among the last `no_repeats` picks.
fn pick_fresh(n: usize, recent: &mut VecDeque<usize>, no_repeats: usize, rng: &mut impl Rng) -> usize {
    let keep = no_repeats.min(n.saturating_sub(1));
    let choices: Vec<usize> = (0..n).filter(|i| !recent.iter().take(keep).any(|r| r == i)).collect();
    let i = choices.get(rng.gen_range(0..choices.len().max(1))).copied().unwrap_or(0);
    recent.push_front(i);
    recent.truncate(keep.max(1));
    i
}

pub fn plugin(app: &mut App) {
    app.init_resource::<WeatherSounds>()
        .add_systems(OnEnter(AppState::InGame), |mut s: ResMut<WeatherSounds>| *s = WeatherSounds::default())
        .add_systems(Update, (loops, thunder).run_if(in_state(AppState::InGame)));
}

fn waves<'a>(db: &'a SoundDb, key: &str) -> &'a [String] {
    db.weather.waves.get(key).map_or(&[], Vec::as_slice)
}

/// The AMBIENT slider (and MASTER), x 0.1 under a roof.
fn ambient(db: &SoundDb, store: &SettingsStore, sheltered: bool) -> f32 {
    class_volume(db, store, db.weather.class.as_deref()) * if sheltered { SHELTERED } else { 1.0 }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn loops(
    mut commands: Commands,
    time: Res<Time>,
    db: Res<SoundDb>,
    store: Res<SettingsStore>,
    assets: Res<AssetServer>,
    weather: Res<Weather>,
    sheltered: Res<Sheltered>,
    mut state: ResMut<WeatherSounds>,
    mut playing: Query<(Entity, &WeatherLoop, Option<&mut AudioSink>)>,
    mut played: ResMut<Played>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let v = weather.0;
    let factor = ambient(&db, &store, sheltered.0);
    let (light, heavy, drips) = rain_volumes(v.rain);
    let mut want: Vec<(LoopKind, usize, f32, f32)> = Vec::new();
    for i in 0..4 {
        want.push((LoopKind::LightRain, i, light * 0.75, 1.0));
        want.push((LoopKind::HeavyRain, i, heavy * 0.75, 1.0));
        let (amp, pitch) = &mut state.wind[i];
        let a = amp.step(dt, &mut rng);
        let p = pitch.step(dt, &mut rng);
        want.push((LoopKind::Wind, i, wind_volume(v.wind, a), semitones(p)));
    }
    let wp = state.whistle_pitch.step(dt, &mut rng);
    want.push((LoopKind::Whistle, 0, whistle_volume(v.wind), semitones(wp)));
    for (kind, index, gain, speed) in want {
        // The rain loops start and stop at 0.001 (TriggerOnThreshold), the whistle at 0.01; the
        // wind plays from the start (ours: once it's audible at all).
        let on = gain > if kind == LoopKind::Whistle { 0.01 } else { 0.001 };
        let found = playing.iter_mut().find(|(_, l, _)| l.kind == kind && l.index == index);
        match (on, found) {
            (true, Some((_, _, Some(mut sink)))) => {
                sink.set_volume(Volume::Linear(gain * factor));
                sink.set_speed(speed);
            }
            (true, None) => {
                let list = waves(&db, kind.waves());
                let Some(wave) = list.get(index % list.len().max(1)) else { continue };
                // RandomTime into the Wave Player's Start Time: 1-5 s (the whistle 0-15 s).
                let start = if kind == LoopKind::Whistle { rng.gen_range(0.0..15.0) } else { rng.gen_range(1.0..5.0) };
                commands.spawn((
                    AudioPlayer::new(assets.load::<AudioSource>(wave.clone())),
                    PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(gain * factor), speed, start_position: Some(std::time::Duration::from_secs_f32(start)), ..default() },
                    WeatherLoop { kind, index },
                    DespawnOnExit(AppState::InGame),
                ));
                *played.0.entry(kind.id().to_string()).or_insert(0) += 1;
                if index == 0 {
                    info!("weather sound: {} on", kind.id());
                }
            }
            (false, Some((e, _, _))) => {
                commands.entity(e).despawn();
            }
            _ => {}
        }
    }
    // Very light rain: RainHit drips, each rain loop on its own 0.2-1.5 s clock.
    if drips > 0.0 && !sheltered.0 {
        let hits = waves(&db, "UDS_Rain_Loop.Rain Hit Sounds");
        for d in &mut state.drips {
            *d -= dt;
            if *d <= 0.0 {
                *d = rng.gen_range(0.2..1.5);
                let Some(w) = hits.get(rng.gen_range(0..hits.len().max(1))) else { continue };
                commands.spawn((
                    AudioPlayer::new(assets.load::<AudioSource>(w.clone())),
                    PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(rng.gen_range(0.3..1.0) * drips * 0.75 * factor), speed: semitones(rng.gen_range(-2.0..2.0)), ..default() },
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn thunder(
    mut commands: Commands,
    time: Res<Time>,
    db: Res<SoundDb>,
    store: Res<SettingsStore>,
    assets: Res<AssetServer>,
    weather: Res<Weather>,
    sheltered: Res<Sheltered>,
    mut state: ResMut<WeatherSounds>,
    mut flashes: MessageReader<LightningFlash>,
    mut distant: Query<&mut AudioSink, With<DistantThunder>>,
    mut played: ResMut<Played>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let factor = ambient(&db, &store, sheltered.0);
    let t = weather.0.thunder;
    let gain = distant_thunder_volume(t);
    for mut s in &mut distant {
        s.set_volume(Volume::Linear(gain * factor));
    }
    // TriggerOnThreshold at 0.01 starts the two TriggerRepeats (the first after its 8 s delay)
    // and stops them below.
    if t > 0.01 {
        if state.distant == [None, None] {
            state.distant = [Some(DISTANT_OFFSET), Some(0.0)];
        }
    } else {
        state.distant = [None, None];
    }
    let list = waves(&db, "UDS_Distant_Thunder_Loop.Thunder Sounds");
    for k in 0..2 {
        let Some(left) = state.distant[k].as_mut() else { continue };
        *left -= dt;
        if *left > 0.0 || list.is_empty() {
            continue;
        }
        *left += DISTANT_PERIOD;
        let i = pick_fresh(list.len(), &mut state.recent_distant, 2, &mut rng);
        commands.spawn((
            AudioPlayer::new(assets.load::<AudioSource>(list[i].clone())),
            PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(gain * factor), ..default() },
            DistantThunder,
            DespawnOnExit(AppState::InGame),
        ));
        *played.0.entry("UDS_Distant_Thunder_Loop".into()).or_insert(0) += 1;
    }
    for f in flashes.read() {
        let (delay, volume) = close_thunder(f.distance);
        state.close.push((delay, volume));
    }
    let close = waves(&db, "UDS_Close_Thunder.Thunder Sounds");
    let mut due = Vec::new();
    state.close.retain_mut(|(left, volume)| {
        *left -= dt;
        if *left <= 0.0 {
            due.push(*volume);
        }
        *left > 0.0
    });
    for volume in due {
        if close.is_empty() {
            break;
        }
        let i = pick_fresh(close.len(), &mut state.recent_close, 5, &mut rng);
        commands.spawn((
            AudioPlayer::new(assets.load::<AudioSource>(close[i].clone())),
            PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(volume * factor), ..default() },
            DespawnOnExit(AppState::InGame),
        ));
        *played.0.entry("UDS_Close_Thunder".into()).or_insert(0) += 1;
        info!("weather sound: close thunder at {volume:.2}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_crossfades_light_to_heavy() {
        assert_eq!(rain_volumes(0.0), (0.0, 0.0, 0.0));
        let (l, h, _) = rain_volumes(5.0);
        assert!((l - 0.3).abs() < 1e-6 && (h - 0.5 * 2.0 / 7.0).abs() < 1e-5, "light peaks at 5");
        let (l, h, d) = rain_volumes(10.0);
        assert!(l.abs() < 1e-3 && (h - 0.5).abs() < 1e-6 && d == 0.0, "heavy rain only at 10");
        let (_, h, d) = rain_volumes(0.5);
        assert!(h == 0.0 && (d - 0.07).abs() < 1e-6, "drips in a drizzle");
    }

    #[test]
    fn wind_and_whistling_follow_the_graphs() {
        assert_eq!(wind_volume(0.0, 1.0), 0.0);
        assert!((wind_volume(10.0, 1.0) - 0.4).abs() < 1e-6);
        assert!((wind_volume(5.0, 1.0) - 0.4 * 0.5f32.powf(2.8)).abs() < 1e-6);
        assert_eq!(whistle_volume(6.0), 0.0);
        assert!((whistle_volume(10.0) - 0.7).abs() < 1e-6);
        assert!((whistle_volume(8.0) - 0.7 * 0.25).abs() < 1e-6);
    }

    #[test]
    fn thunder_by_value_and_distance() {
        assert_eq!(distant_thunder_volume(0.0), 0.0);
        assert!((distant_thunder_volume(10.0) - 1.0).abs() < 1e-6);
        // Close Thunder Sound Delay: 0.15 s per km; MapRangeClamped(distance, range, 1.3, 0.4).
        let (d, v) = close_thunder(1550.0);
        assert!((d - 0.2325).abs() < 1e-4 && (v - 1.3).abs() < 1e-6);
        let (d, v) = close_thunder(6500.0);
        assert!((d - 0.975).abs() < 1e-4 && (v - 0.4).abs() < 1e-6);
    }

    #[test]
    fn wanders_glide_within_their_range() {
        let mut rng = rand::thread_rng();
        let mut w = Wander::new([0.7, 1.0], 1.0, 1.0, 0.85);
        for _ in 0..300 {
            let v = w.step(1.0 / 30.0, &mut rng);
            assert!((0.7..=1.0).contains(&v));
        }
    }

    #[test]
    fn no_repeats_skips_the_last_picks() {
        let mut rng = rand::thread_rng();
        let mut recent = VecDeque::new();
        let mut last: Vec<usize> = Vec::new();
        for _ in 0..200 {
            let i = pick_fresh(6, &mut recent, 5, &mut rng);
            assert!(!last.iter().rev().take(5).any(|&j| j == i), "{i} repeated within 5 ({last:?})");
            last.push(i);
        }
    }

    #[test]
    fn the_weather_waves_are_in_the_data() {
        let db = SoundDb::load();
        if db.cues.is_empty() {
            return;
        }
        for k in [LoopKind::LightRain, LoopKind::HeavyRain, LoopKind::Wind] {
            assert_eq!(waves(&db, k.waves()).len(), 4, "{}", k.waves());
        }
        assert_eq!(waves(&db, LoopKind::Whistle.waves()).len(), 1);
        assert_eq!(waves(&db, "UDS_Distant_Thunder_Loop.Thunder Sounds").len(), 11);
        assert_eq!(waves(&db, "UDS_Close_Thunder.Thunder Sounds").len(), 6);
        assert_eq!(db.weather.class.as_deref(), Some("AmbientAudio"));
    }
}
