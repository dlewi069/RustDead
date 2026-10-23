//! Music and area ambience: BP_Audio_Manager + BP_Audio_Area (OpenWorldAudio), the survivor's
//! CheckCombat, BP_AudioPoint, and the MainMenu level's AmbientSound.
//!
//! - BP_Audio_Area boxes (actors.json: AreaName, Priority (default 1), ExplorationMusic /
//!   CombatMusic / Ambient_Sounds as (cue, volume) lists, DelayBetweenMusicMin/Max, BoxExtent
//!   (template 500 cm) under the actor's scale and yaw). The most relevant area is the highest
//!   priority box the survivor is in (an equal one entered later takes over; leaving it rescans
//!   the others, first wins on ties). A new area with the same AreaName changes nothing.
//! - Music (every area has UseDelayedMusic and DelayBeforeStart): TimeBeforeNewSong waits a
//!   random DelayBetweenMusicMin..Max (Main 40-120 s, the others 5-10 s), then a random
//!   ExplorationMusic cue plays (CreateSound2D at the entry's volume, DelayedFadeDuration 0); at
//!   95 % of its length (`RetriggerableDelay((Duration - DelayedFadeDuration) x 0.95)`) it's cut and
//!   the wait starts again. Changing area (ChangeAudio): if the new area's pick is the cue
//!   already playing it carries on; otherwise it fades out over Exploration_SwitchFadeDelay (4 s,
//!   E_Switch_Method 1) and the new area's wait begins.
//! - Combat: BP_PlayerCharacter.CheckCombat (every 2 s, COMBAT MUSIC ENABLED? = Character.
//!   CombatMusic) sets EnemiesClose when a live BP_MainEnemy within 30 m is InCombat? (ours:
//!   chasing). The manager checks every 0.15 s: entering combat, if the area has CombatMusic
//!   (only Main), the music fades out over Combat_SwitchFadeDelay (2 s), and after
//!   Combat_SwitchDelay (1 s) a combat track fades in over 2 s; it plays once. Leaving combat fades
//!   it out (2 s) and the exploration wait starts again. Each switch blocks the next check for
//!   2.3 s (CanCheckAgain). Areas without combat music keep their exploration music.
//! - Ambient_Sounds (the Subway's Underground_Cue): a 2D bed that cross-fades over
//!   AmbientCrossFadeDuration (4 s) between areas (1 s at the start).
//! - BP_AudioPoint (4 at the shipwreck): a looping cue at its place (`AmbientLoop`).
//! - The main menu: MainMenuMusic at VolumeMultiplier 0.5 while the menus are up.
//!
//! Volumes are the cue's VolumeMultiplier x the entry's volume x MUSIC (or AMBIENT) x MASTER,
//! applied every frame so the sliders act at once. Not ported: switch stingers (no area sets
//! one), RandomAmbientSoundCue one-shots (none set), SetInside's indoor ambient multiplier and
//! reverb, the TimeOfDay / Combat cue parameters (no music cue reads them), PlayMusicOnGameStart.

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use rand::Rng;
use serde::Deserialize;

use super::{SoundDb, class_volume, pick};
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::AppState;

/// E_Switch_Method 1's delays (SetAudioVariables' selects).
const EXPLORATION_FADE: f32 = 4.0;
const COMBAT_FADE: f32 = 2.0;
const COMBAT_DELAY: f32 = 1.0;
/// StartCombatMusic's Delay before CanCheckAgain.
const RECHECK: f32 = 2.3;
const AMBIENT_FADE: f32 = 4.0;
/// CheckCombat's SphereOverlapActors radius (m) and timer.
pub const COMBAT_RADIUS: f32 = 30.0;
const COMBAT_EVERY: f32 = 2.0;

/// An ST_Audio entry: a cue and its volume.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub cue: String,
    pub volume: f32,
}

/// A BP_Audio_Area.
#[derive(Clone, Debug)]
pub struct Area {
    pub name: String,
    pub priority: i32,
    pub music: Vec<Track>,
    pub combat: Vec<Track>,
    pub ambient: Vec<Track>,
    pub delay: [f32; 2],
    /// World -> the box's unscaled local space (metres), and its half extent there.
    to_local: Mat4,
    half: Vec3,
}

impl Voice {
    pub fn layer(&self) -> Layer {
        self.layer
    }

    /// Fading out to go.
    pub fn dying(&self) -> bool {
        self.dying
    }
}

impl Area {
    pub fn contains(&self, p: Vec3) -> bool {
        let l = self.to_local.transform_point3(p);
        l.x.abs() <= self.half.x && l.y.abs() <= self.half.y && l.z.abs() <= self.half.z
    }

    fn from_actor(a: &crate::level::ActorInfo) -> Option<Self> {
        let props = a.props.as_ref()?;
        #[derive(Deserialize)]
        struct V3 {
            #[serde(rename = "X")]
            x: f32,
            #[serde(rename = "Y")]
            y: f32,
            #[serde(rename = "Z")]
            z: f32,
        }
        // UE (X, Y, Z) cm -> Bevy local (X, Z, Y) m.
        let ext = props.get("BoxExtent").and_then(|v| serde_json::from_str::<V3>(v).ok()).map_or(Vec3::splat(5.0), |e| Vec3::new(e.x, e.z, e.y) / 100.0);
        let tracks = |k: &str| -> Vec<Track> {
            let Some(list) = props.get(k).and_then(|v| serde_json::from_str::<Vec<serde_json::Value>>(v).ok()) else { return Vec::new() };
            list.iter()
                .filter_map(|e| {
                    let o = e.as_object()?;
                    let get = |name: &str| o.iter().find(|(k, _)| k.split('_').next() == Some(name)).map(|(_, v)| v);
                    let cue = get("Sound")?.get("ObjectName")?.as_str()?.split('\'').nth(1)?.to_string();
                    let volume = get("Volume").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    Some(Track { cue, volume })
                })
                .collect()
        };
        let num = |k: &str, d: f32| props.get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
        let m = Mat4::from_cols(
            Vec4::new(a.transform[0], a.transform[1], a.transform[2], 0.0),
            Vec4::new(a.transform[3], a.transform[4], a.transform[5], 0.0),
            Vec4::new(a.transform[6], a.transform[7], a.transform[8], 0.0),
            Vec4::new(a.transform[9], a.transform[10], a.transform[11], 1.0),
        );
        Some(Self {
            name: props.get("AreaName").map(|s| s.trim_matches('"').to_string()).unwrap_or_default(),
            priority: num("Priority", 1.0) as i32,
            music: tracks("ExplorationMusic"),
            combat: tracks("CombatMusic"),
            ambient: tracks("Ambient_Sounds"),
            // BP_Audio_Area has no class default for these (0); every instance sets them.
            delay: [num("DelayBetweenMusicMin", 0.0), num("DelayBetweenMusicMax", 0.0)],
            to_local: m.inverse(),
            half: ext,
        })
    }
}

/// The level's audio areas and points.
#[derive(Resource, Default)]
pub struct AudioAreas {
    pub areas: Vec<Area>,
    /// BP_AudioPoint: (cue, where).
    pub points: Vec<(String, Vec3)>,
}

impl AudioAreas {
    pub fn from_actors(actors: &[crate::level::ActorInfo]) -> Self {
        let areas = actors.iter().filter(|a| a.class == "BP_Audio_Area_C").filter_map(Area::from_actor).collect();
        let points = actors
            .iter()
            .filter(|a| a.class == "BP_AudioPoint_C")
            .filter_map(|a| Some((a.props.as_ref()?.get("Sound")?.clone(), a.position())))
            .collect();
        Self { areas, points }
    }

    fn load() -> Self {
        let path = format!("{}/actors.json", crate::level::LEVEL_DIR);
        let actors: Vec<crate::level::ActorInfo> = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self::from_actors(&actors)
    }

    /// The areas `p` is in, in level order.
    pub fn overlapping(&self, p: Vec3) -> Vec<usize> {
        (0..self.areas.len()).filter(|&i| self.areas[i].contains(p)).collect()
    }
}

/// What the director wants the speakers to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    /// Start a music track, fading in over `fade` s.
    Play { track: Track, fade: f32 },
    /// Fade the music out over `secs` (0 = cut).
    FadeOut(f32),
    /// Cross-fade the ambient bed to this (None: silence) over `fade` s.
    Ambient(Option<Track>, f32),
}

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    /// No area, or combat with its track done.
    Idle,
    /// TimeBeforeNewSong.
    Waiting(f32),
    /// A track playing; cut when `left` runs out.
    Playing { track: Track, left: Option<f32>, combat: bool },
    /// Fading out / StartCombatMusic's delays, then `next`.
    Switching { left: f32, next: Next },
}

#[derive(Clone, Debug, PartialEq)]
enum Next {
    Wait,
    Combat(Track),
}

/// BP_Audio_Manager's state.
#[derive(Clone, Debug)]
pub struct Director {
    /// MostRelevantAudioArea and the overlapping ones.
    pub area: Option<usize>,
    overlapping: Vec<usize>,
    pub combat: bool,
    phase: Phase,
    /// CanCheckAgain's lockout.
    recheck: f32,
    started: bool,
}

impl Default for Director {
    fn default() -> Self {
        Self { area: None, overlapping: Vec::new(), combat: false, phase: Phase::Idle, recheck: 0.0, started: false }
    }
}

impl Director {
    /// What's playing: (cue, combat).
    pub fn playing(&self) -> Option<(&str, bool)> {
        match &self.phase {
            Phase::Playing { track, combat, .. } => Some((track.cue.as_str(), *combat)),
            _ => None,
        }
    }

    /// The phase, for logs: idle, waiting, playing, switching.
    pub fn phase_name(&self) -> &'static str {
        match self.phase {
            Phase::Idle => "idle",
            Phase::Waiting(_) => "waiting",
            Phase::Playing { .. } => "playing",
            Phase::Switching { .. } => "switching",
        }
    }

    /// End the wait now (the tour; it would take 40-120 s in Main).
    pub fn skip_wait(&mut self) {
        if let Phase::Waiting(t) = &mut self.phase {
            *t = 0.0;
        }
    }

    pub fn waiting(&self) -> Option<f32> {
        if let Phase::Waiting(t) = self.phase { Some(t) } else { None }
    }

    fn wait(&mut self, areas: &AudioAreas, rng: &mut impl Rng) {
        let d = self.area.map_or([0.0, 0.0], |a| areas.areas[a].delay);
        self.phase = Phase::Waiting(rng.gen_range(d[0]..=d[1].max(d[0])));
    }

    fn random(list: &[Track], rng: &mut impl Rng) -> Option<Track> {
        (!list.is_empty()).then(|| list[rng.gen_range(0..list.len())].clone())
    }

    /// The overlap events for the survivor now at a set of areas (BP_Audio_Area's Begin/End
    /// overlap; at the start, ReceiveBeginPlay's scan).
    pub fn update_areas(&mut self, areas: &AudioAreas, now: &[usize], rng: &mut impl Rng, out: &mut Vec<Cmd>) {
        if !self.started {
            self.started = true;
            self.overlapping = now.to_vec();
            // BeginPlay: the last of the highest priority (ties: a later one replaces).
            let mut best: Option<usize> = None;
            for &i in now {
                if best.is_none_or(|b| areas.areas[b].priority <= areas.areas[i].priority) {
                    best = Some(i);
                }
            }
            self.area = best;
            if let Some(a) = best {
                out.push(Cmd::Ambient(Self::random(&areas.areas[a].ambient, rng), 1.0));
                self.wait(areas, rng);
            }
            return;
        }
        let entered: Vec<usize> = now.iter().copied().filter(|i| !self.overlapping.contains(i)).collect();
        let left: Vec<usize> = self.overlapping.iter().copied().filter(|i| !now.contains(i)).collect();
        for i in left {
            self.overlapping.retain(|&o| o != i);
            if self.area == Some(i) {
                // RemoveFromArray: rescan; the first of the highest priority.
                let mut best: Option<usize> = None;
                for &o in &self.overlapping {
                    if best.is_none_or(|b| areas.areas[o].priority > areas.areas[b].priority) {
                        best = Some(o);
                    }
                }
                let old = self.area.map(|a| areas.areas[a].name.clone());
                self.area = best;
                if let Some(b) = best
                    && old.as_deref() != Some(areas.areas[b].name.as_str())
                {
                    self.change(areas, rng, out);
                }
            }
        }
        for i in entered {
            self.overlapping.push(i);
            if let Some(cur) = self.area
                && (areas.areas[cur].name == areas.areas[i].name || areas.areas[i].priority < areas.areas[cur].priority)
            {
                continue;
            }
            self.area = Some(i);
            self.change(areas, rng, out);
        }
    }

    /// ChangeAudio.
    fn change(&mut self, areas: &AudioAreas, rng: &mut impl Rng, out: &mut Vec<Cmd>) {
        let Some(a) = self.area.map(|a| &areas.areas[a]) else { return };
        out.push(Cmd::Ambient(Self::random(&a.ambient, rng), AMBIENT_FADE));
        if self.combat {
            return;
        }
        let pick = Self::random(&a.music, rng);
        if let (Phase::Playing { track, .. }, Some(p)) = (&self.phase, &pick)
            && track.cue == p.cue
        {
            return;
        }
        match self.phase {
            Phase::Playing { .. } => {
                out.push(Cmd::FadeOut(EXPLORATION_FADE));
                self.phase = Phase::Switching { left: EXPLORATION_FADE, next: Next::Wait };
            }
            Phase::Switching { .. } => {}
            _ => self.wait(areas, rng),
        }
    }

    /// The music's clock and the 0.15 s combat check (`enemies_close`: the survivor's EnemiesClose).
    pub fn tick(&mut self, dt: f32, areas: &AudioAreas, enemies_close: bool, rng: &mut impl Rng, out: &mut Vec<Cmd>) {
        self.recheck = (self.recheck - dt).max(0.0);
        if self.recheck <= 0.0 && enemies_close != self.combat && self.area.is_some() {
            self.recheck = RECHECK;
            self.combat = enemies_close;
            let a = &areas.areas[self.area.unwrap()];
            if self.combat {
                if let Some(t) = Self::random(&a.combat, rng) {
                    out.push(Cmd::FadeOut(COMBAT_FADE));
                    self.phase = Phase::Switching { left: COMBAT_FADE + COMBAT_DELAY, next: Next::Combat(t) };
                }
            } else if matches!(self.phase, Phase::Playing { combat: true, .. } | Phase::Switching { next: Next::Combat(_), .. }) {
                out.push(Cmd::FadeOut(COMBAT_FADE));
                self.phase = Phase::Switching { left: COMBAT_FADE, next: Next::Wait };
            } else if self.phase == Phase::Idle {
                self.wait(areas, rng);
            }
        }
        match &mut self.phase {
            Phase::Waiting(t) => {
                *t -= dt;
                if *t <= 0.0 {
                    // During combat the timeline is stopped.
                    if self.combat {
                        self.phase = Phase::Idle;
                    } else if let Some(track) = self.area.and_then(|a| Self::random(&areas.areas[a].music, rng)) {
                        out.push(Cmd::Play { track: track.clone(), fade: 0.0 });
                        self.phase = Phase::Playing { track, left: None, combat: false };
                    } else {
                        self.phase = Phase::Idle;
                    }
                }
            }
            Phase::Playing { left: Some(l), combat, .. } => {
                *l -= dt;
                if *l <= 0.0 {
                    let was_combat = *combat;
                    out.push(Cmd::FadeOut(0.0));
                    if was_combat || self.combat {
                        self.phase = Phase::Idle;
                    } else {
                        self.wait(areas, rng);
                    }
                }
            }
            Phase::Switching { left, next } => {
                *left -= dt;
                if *left <= 0.0 {
                    match next.clone() {
                        Next::Wait => self.wait(areas, rng),
                        Next::Combat(track) => {
                            out.push(Cmd::Play { track: track.clone(), fade: COMBAT_FADE });
                            self.phase = Phase::Playing { track, left: None, combat: true };
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The playing track's length is known: cut it at 95 % (DelayedFadeDuration 0).
    pub fn started(&mut self, duration: f32) {
        if let Phase::Playing { left, .. } = &mut self.phase {
            *left = Some(duration * 0.95);
        }
    }
}

/// A WAV file's length in seconds, from its header.
pub fn wav_seconds(path: &str) -> Option<f32> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 4096];
    let n = f.read(&mut head).ok()?;
    let head = &head[..n];
    if head.get(0..4)? != b"RIFF" || head.get(8..12)? != b"WAVE" {
        return None;
    }
    let total = std::fs::metadata(path).ok()?.len();
    let (mut at, mut byte_rate) = (12usize, None);
    while at + 8 <= head.len() {
        let id = &head[at..at + 4];
        let size = u32::from_le_bytes(head[at + 4..at + 8].try_into().ok()?) as u64;
        if id == b"fmt " {
            byte_rate = Some(u32::from_le_bytes(head.get(at + 16..at + 20)?.try_into().ok()?) as f32);
        } else if id == b"data" {
            // Some writers put 0 or -1 here; fall back to the rest of the file.
            let data = if size == 0 || size == u32::MAX as u64 { total - at as u64 - 8 } else { size };
            return Some(data as f32 / byte_rate?);
        }
        at += 8 + size as usize + (size as usize & 1);
    }
    None
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Layer {
    Music,
    Ambient,
    Menu,
}

/// A playing 2D voice with its fade.
#[derive(Component)]
pub struct Voice {
    pub cue: String,
    layer: Layer,
    base: f32,
    class: Option<String>,
    level: f32,
    /// Fade target and rate per second; reaching 0 while dying despawns it.
    target: f32,
    rate: f32,
    dying: bool,
}

#[derive(Resource, Default)]
pub struct Music {
    pub director: Director,
    check: f32,
    pub enemies_close: bool,
}

pub fn plugin(app: &mut App) {
    app.insert_resource(AudioAreas::load())
        .init_resource::<Music>()
        .add_systems(OnEnter(AppState::MainMenu), menu_music)
        .add_systems(OnEnter(AppState::InGame), (start, spawn_points))
        .add_systems(Update, (check_combat, direct).chain().run_if(in_state(AppState::InGame)))
        .add_systems(Update, fade);
}

/// A new game (or a load): the manager starts over; the menu music fades out over 1 s (ours) and
/// the last session's voices stop.
fn start(mut music: ResMut<Music>, mut voices: Query<&mut Voice>) {
    *music = Music::default();
    for layer in [Layer::Music, Layer::Ambient] {
        fade_out(&mut voices, layer, 0.0);
    }
    fade_out(&mut voices, Layer::Menu, 1.0);
}

fn spawn_points(mut commands: Commands, areas: Res<AudioAreas>) {
    for (cue, at) in &areas.points {
        commands.spawn((Transform::from_translation(*at), super::AmbientLoop { cue: cue.clone(), volume: 1.0, active: true }, DespawnOnExit(AppState::InGame)));
    }
}

fn menu_music(mut commands: Commands, db: Res<SoundDb>, assets: Res<AssetServer>, voices: Query<&Voice>) {
    if voices.iter().any(|v| v.layer == Layer::Menu && !v.dying) {
        return;
    }
    // The MainMenu level's AmbientSound: VolumeMultiplier 0.5.
    spawn_voice(&mut commands, &db, &assets, Layer::Menu, &Track { cue: "MainMenuMusic".into(), volume: 0.5 }, 0.0);
}

/// CheckCombat every 2 s: a live zombie within 30 m chasing the survivor.
#[allow(clippy::type_complexity)]
fn check_combat(
    time: Res<Time>,
    store: Res<SettingsStore>,
    mut music: ResMut<Music>,
    player: Query<&Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    zombies: Query<(&Transform, &crate::gameplay::zombies::Zombie), Without<crate::gameplay::Dead>>,
) {
    music.check -= time.delta_secs();
    if music.check > 0.0 {
        return;
    }
    music.check = COMBAT_EVERY;
    let enabled = store.find(&row_key("settings", "Audio", "COMBAT MUSIC ENABLED?")) != Some(Value::Bool(false));
    let Ok(me) = player.single() else { return };
    let close = enabled && zombies.iter().any(|(t, z)| matches!(z.brain, crate::gameplay::zombies::Brain::Chase) && t.translation.distance(me.translation) <= COMBAT_RADIUS);
    music.enemies_close = close;
}

#[allow(clippy::too_many_arguments)]
fn direct(
    mut commands: Commands,
    time: Res<Time>,
    areas: Res<AudioAreas>,
    db: Res<SoundDb>,
    assets: Res<AssetServer>,
    mut music: ResMut<Music>,
    player: Query<&Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut voices: Query<&mut Voice>,
) {
    let Ok(me) = player.single() else { return };
    let mut rng = rand::thread_rng();
    let mut out = Vec::new();
    let close = music.enemies_close;
    let now = areas.overlapping(me.translation);
    music.director.update_areas(&areas, &now, &mut rng, &mut out);
    music.director.tick(time.delta_secs(), &areas, close, &mut rng, &mut out);
    for cmd in out {
        match cmd {
            Cmd::Play { track, fade } => {
                if let Some(d) = spawn_voice(&mut commands, &db, &assets, Layer::Music, &track, fade) {
                    music.director.started(d);
                }
                info!("music: {} ({})", track.cue, areas.areas.get(music.director.area.unwrap_or(usize::MAX)).map_or("", |a| a.name.as_str()));
            }
            Cmd::FadeOut(secs) => fade_out(&mut voices, Layer::Music, secs),
            Cmd::Ambient(track, secs) => {
                fade_out(&mut voices, Layer::Ambient, secs);
                if let Some(t) = track {
                    spawn_voice(&mut commands, &db, &assets, Layer::Ambient, &t, secs);
                }
            }
        }
    }
}

fn fade_out(voices: &mut Query<&mut Voice>, layer: Layer, secs: f32) {
    for mut v in voices.iter_mut().filter(|v| v.layer == layer && !v.dying) {
        v.dying = true;
        v.target = 0.0;
        v.rate = if secs <= 0.0 { f32::INFINITY } else { v.level.max(0.01) / secs };
    }
}

/// Start a cue as a 2D voice; returns the chosen wave's length.
fn spawn_voice(commands: &mut Commands, db: &SoundDb, assets: &AssetServer, layer: Layer, track: &Track, fade: f32) -> Option<f32> {
    let cue = db.cues.get(&track.cue)?;
    let mut rng = rand::thread_rng();
    let v = pick(cue, rng.gen_range(0.0..1.0))?;
    let wave = v.waves.first()?;
    let base = cue.volume * track.volume * rng.gen_range(v.volume[0]..=v.volume[1].max(v.volume[0]));
    let speed = cue.pitch * rng.gen_range(v.pitch[0]..=v.pitch[1].max(v.pitch[0]));
    let looping = v.looping || layer != Layer::Music;
    commands.spawn((
        AudioPlayer::new(assets.load::<AudioSource>(wave.clone())),
        PlaybackSettings { mode: if looping { PlaybackMode::Loop } else { PlaybackMode::Despawn }, volume: Volume::Linear(0.0), speed, ..default() },
        Voice { cue: track.cue.clone(), layer, base, class: cue.class.clone(), level: if fade > 0.0 { 0.0 } else { 1.0 }, target: 1.0, rate: if fade > 0.0 { 1.0 / fade } else { f32::INFINITY }, dying: false },
    ));
    wav_seconds(&format!("assets/{wave}")).map(|d| d / speed.max(0.01))
}

/// Fades, and the sliders applied live.
fn fade(mut commands: Commands, time: Res<Time<Real>>, db: Res<SoundDb>, store: Res<SettingsStore>, mut voices: Query<(Entity, &mut Voice, Option<&mut AudioSink>)>) {
    let dt = time.delta_secs();
    for (e, mut v, sink) in &mut voices {
        let step = v.rate * dt;
        v.level = if v.level < v.target { (v.level + step).min(v.target) } else { (v.level - step).max(v.target) };
        if v.dying && v.level <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(v.base * v.level * class_volume(&db, &store, v.class.as_deref())));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn area(name: &str, priority: i32, centre: Vec3, half: f32, music: &[&str], combat: &[&str], delay: [f32; 2]) -> Area {
        let t = |c: &[&str]| c.iter().map(|c| Track { cue: c.to_string(), volume: 0.25 }).collect();
        Area { name: name.into(), priority, music: t(music), combat: t(combat), ambient: Vec::new(), delay, to_local: Mat4::from_translation(-centre), half: Vec3::splat(half) }
    }

    fn world() -> AudioAreas {
        AudioAreas {
            areas: vec![
                area("Main", 1, Vec3::ZERO, 1000.0, &["Main_ExplorationMusic"], &["CombatMusic_Cue"], [40.0, 120.0]),
                area("SafeZone", 2, Vec3::new(100.0, 0.0, 0.0), 10.0, &["SafeZoneWAV_Cue"], &[], [5.0, 10.0]),
                area("Bunker", 2, Vec3::new(-100.0, 0.0, 0.0), 10.0, &["AmbBunker1WAV_Cue"], &[], [5.0, 10.0]),
                area("Prison", 2, Vec3::new(-110.0, 0.0, 0.0), 10.0, &["AmbBunker1WAV_Cue"], &[], [5.0, 10.0]),
            ],
            points: Vec::new(),
        }
    }

    fn run(d: &mut Director, w: &AudioAreas, at: Vec3, secs: f32, close: bool, rng: &mut rand::rngs::StdRng) -> Vec<Cmd> {
        let mut out = Vec::new();
        let mut t = 0.0;
        while t < secs {
            d.update_areas(w, &w.overlapping(at), rng, &mut out);
            d.tick(0.05, w, close, rng, &mut out);
            if matches!(out.last(), Some(Cmd::Play { .. })) && d.playing().is_some() {
                d.started(100.0);
            }
            t += 0.05;
        }
        out
    }

    #[test]
    fn music_waits_then_plays_then_waits() {
        let w = world();
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let mut d = Director::default();
        let out = run(&mut d, &w, Vec3::ZERO, 1.0, false, &mut rng);
        assert_eq!(d.area, Some(0));
        let wait = d.waiting().unwrap();
        assert!((39.0..=120.0).contains(&wait), "{wait}");
        assert!(!out.iter().any(|c| matches!(c, Cmd::Play { .. })));
        let out = run(&mut d, &w, Vec3::ZERO, 121.0, false, &mut rng);
        assert!(out.iter().any(|c| matches!(c, Cmd::Play { track, .. } if track.cue == "Main_ExplorationMusic")));
        // Cut at 95 % of its 100 s, then the wait again.
        let out = run(&mut d, &w, Vec3::ZERO, 96.0, false, &mut rng);
        assert!(out.contains(&Cmd::FadeOut(0.0)), "{out:?}");
        assert!(d.waiting().is_some() || d.playing().is_some());
    }

    #[test]
    fn areas_switch_by_priority_and_name() {
        let w = world();
        let mut rng = rand::rngs::StdRng::seed_from_u64(2);
        let mut d = Director::default();
        run(&mut d, &w, Vec3::ZERO, 130.0, false, &mut rng);
        assert_eq!(d.playing(), Some(("Main_ExplorationMusic", false)));
        // Into the safe zone (priority 2): the music fades over 4 s, then the zone's 5-10 s wait.
        let out = run(&mut d, &w, Vec3::new(100.0, 0.0, 0.0), 1.0, false, &mut rng);
        assert_eq!(d.area, Some(1));
        assert!(out.contains(&Cmd::FadeOut(4.0)), "{out:?}");
        run(&mut d, &w, Vec3::new(100.0, 0.0, 0.0), 15.0, false, &mut rng);
        assert_eq!(d.playing(), Some(("SafeZoneWAV_Cue", false)));
        // Back out: Main again (rescan).
        run(&mut d, &w, Vec3::ZERO, 1.0, false, &mut rng);
        assert_eq!(d.area, Some(0));
        // Bunker -> the overlapping Prison: the same cue carries on.
        run(&mut d, &w, Vec3::new(-100.0, 0.0, 0.0), 15.0, false, &mut rng);
        assert_eq!(d.playing(), Some(("AmbBunker1WAV_Cue", false)));
        let out = run(&mut d, &w, Vec3::new(-105.0, 0.0, 0.0), 1.0, false, &mut rng);
        assert_eq!(d.area, Some(3));
        assert!(!out.iter().any(|c| matches!(c, Cmd::FadeOut(_))), "{out:?}");
        assert_eq!(d.playing(), Some(("AmbBunker1WAV_Cue", false)));
    }

    #[test]
    fn combat_music_comes_and_goes() {
        let w = world();
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        let mut d = Director::default();
        run(&mut d, &w, Vec3::ZERO, 130.0, false, &mut rng);
        let out = run(&mut d, &w, Vec3::ZERO, 0.1, true, &mut rng);
        assert!(d.combat && out.contains(&Cmd::FadeOut(2.0)));
        // Fade 2 s + delay 1 s, then the fight track fades in over 2 s.
        let out = run(&mut d, &w, Vec3::ZERO, 3.1, true, &mut rng);
        assert!(out.iter().any(|c| matches!(c, Cmd::Play { track, fade } if track.cue == "CombatMusic_Cue" && *fade == 2.0)), "{out:?}");
        assert_eq!(d.playing(), Some(("CombatMusic_Cue", true)));
        // Enemies gone: fade out (after the 2.3 s lockout), then the exploration wait.
        let out = run(&mut d, &w, Vec3::ZERO, 3.0, false, &mut rng);
        assert!(!d.combat && out.contains(&Cmd::FadeOut(2.0)), "{out:?}");
        run(&mut d, &w, Vec3::ZERO, 2.1, false, &mut rng);
        assert!(d.waiting().is_some());
        // An area without combat music keeps its music through a fight.
        let mut d = Director::default();
        run(&mut d, &w, Vec3::new(100.0, 0.0, 0.0), 15.0, false, &mut rng);
        let out = run(&mut d, &w, Vec3::new(100.0, 0.0, 0.0), 5.0, true, &mut rng);
        assert!(d.combat && !out.iter().any(|c| matches!(c, Cmd::FadeOut(_) | Cmd::Play { .. })), "{out:?}");
        assert_eq!(d.playing(), Some(("SafeZoneWAV_Cue", false)));
    }

    #[test]
    fn level_areas_load() {
        let Ok(a) = std::fs::read_to_string(format!("{}/actors.json", crate::level::LEVEL_DIR)) else { return };
        let actors: Vec<crate::level::ActorInfo> = serde_json::from_str(&a).unwrap();
        let w = AudioAreas::from_actors(&actors);
        assert_eq!(w.areas.len(), 11);
        assert_eq!(w.points.len(), 4);
        let main = w.areas.iter().find(|a| a.name == "Main").unwrap();
        assert_eq!((main.priority, main.delay), (1, [40.0, 120.0]));
        assert_eq!(main.music, vec![Track { cue: "Main_ExplorationMusic".into(), volume: 0.25 }]);
        assert_eq!(main.combat[0].cue, "CombatMusic_Cue");
        // Main covers the valley: 475 x 5 m = 2375 m each way from (759, 763).
        assert!(main.contains(Vec3::new(759.0, 100.0, 763.0)) && main.contains(Vec3::new(-1500.0, 50.0, 2900.0)));
        let subway = w.areas.iter().find(|a| a.name == "Subway").unwrap();
        assert_eq!(subway.ambient[0].cue, "Underground_Cue");
        // BUNKER_0: 175 x 100 m half extents, turned 180°, at (49.4, 5.5, 911.2).
        assert!(subway.contains(Vec3::new(49.4 + 170.0, 5.5, 911.2 + 95.0)) && !subway.contains(Vec3::new(49.4, 5.5, 911.2 + 105.0)));
        // A turned safe zone (yaw 20°, scale 5): 25 m half extents.
        let sz = &w.areas.iter().find(|a| a.name == "SafeZone").unwrap();
        assert!(sz.contains(Vec3::new(1155.13, 15.42, 1315.61)));
        let db = SoundDb::load();
        if !db.cues.is_empty() {
            for a in &w.areas {
                for t in a.music.iter().chain(&a.combat).chain(&a.ambient) {
                    assert!(db.cues.contains_key(&t.cue), "{}", t.cue);
                }
            }
            assert!(db.cues.contains_key("MainMenuMusic") && db.cues.contains_key("ShipWreck_Background_Cue"));
        }
    }

    #[test]
    fn wav_length_from_its_header() {
        let dir = std::env::temp_dir().join(format!("sd_wav_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.wav");
        // 1 s of 16-bit stereo 48 kHz.
        let rate = 48000u32 * 4;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + rate).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&48000u32.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&rate.to_le_bytes());
        b.resize(b.len() + rate as usize, 0);
        std::fs::write(&path, b).unwrap();
        assert!((wav_seconds(path.to_str().unwrap()).unwrap() - 1.0).abs() < 1e-4);
        std::fs::remove_dir_all(dir).ok();
    }
}
