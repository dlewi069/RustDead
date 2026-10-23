//! Sound playback for the game's SoundCues (`assets/data/sounds.json`, `extractor/gen_sounds.py`).
//!
//! - A [`Sound`] message names a cue and where it plays (`None`: 2D, like the UI). A variant is
//!   picked by weight (SoundNodeRandom), its volume and pitch rolled in the modulators' ranges, and
//!   it plays at cue VolumeMultiplier x variant volume x the SoundClass's slider (Audio settings:
//!   AMBIENT / EFFECTS / MUSIC / UI / RADIO) x MASTER x the attenuation at the listener's distance.
//! - Attenuation is the cue's SoundAttenuation asset: full volume within the sphere's radius, then
//!   down to silence over FalloffDistance, linearly or with UE's logarithmic curve. Ours is
//!   computed once for one-shots and every frame for loops ([`Emitter`]); there's no stereo
//!   panning, occlusion or low-pass filtering.

pub mod music;

use std::collections::HashMap;

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use rand::Rng;
use serde::Deserialize;

use crate::menus::settings::{SettingsStore, Value, row_key};

#[derive(Deserialize, Clone, Debug)]
pub struct Variant {
    pub waves: Vec<String>,
    pub weight: f32,
    pub volume: [f32; 2],
    pub pitch: [f32; 2],
    #[serde(rename = "loop", default)]
    pub looping: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Cue {
    pub class: Option<String>,
    pub attenuation: Option<String>,
    pub volume: f32,
    pub pitch: f32,
    pub variants: Vec<Variant>,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Attenuation {
    pub inner: f32,
    pub falloff: f32,
    pub model: Falloff,
    #[serde(default = "yes")]
    pub spatialize: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum Falloff {
    Linear,
    Logarithmic,
    Inverse,
    LogReverse,
    NaturalSound,
    Custom,
}

#[derive(Deserialize, Resource, Default, Debug)]
pub struct SoundDb {
    pub cues: HashMap<String, Cue>,
    pub attenuation: HashMap<String, Attenuation>,
    /// SoundClass -> the Audio settings slider.
    pub classes: HashMap<String, String>,
    /// Landscape paint layer (LayerInfo) -> its PhysMaterial's surface ("Grass", "Dirt"...).
    #[serde(default)]
    pub terrain_surfaces: HashMap<String, String>,
    /// A sequence's AnimNotify_PlaySound (its montage's, in sequence time): reloads' mag out/in.
    #[serde(default)]
    pub action_sounds: HashMap<String, Vec<NotifySound>>,
    /// SoundNodeParamCrossFade cues: each input is its own looping cue, faded by the parameter.
    #[serde(default)]
    pub crossfades: HashMap<String, Crossfade>,
    /// UDS's weather MetaSounds' waves (`gameplay::weather_sounds`).
    #[serde(default)]
    pub weather: WeatherSoundData,
}

/// Each weather MetaSound's wave arrays, as "<MetaSound>.<variable or node>", and their class.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct WeatherSoundData {
    pub class: Option<String>,
    #[serde(default)]
    pub waves: HashMap<String, Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Crossfade {
    pub param: Option<String>,
    pub inputs: Vec<CrossfadeInput>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CrossfadeInput {
    pub cue: String,
    pub fade_in: [f32; 2],
    pub fade_out: [f32; 2],
    pub volume: f32,
}

impl CrossfadeInput {
    /// SoundNodeParamCrossFade: 0 -> Volume over FadeInDistanceStart..End, Volume -> 0 over
    /// FadeOutDistanceStart..End (an empty fade-in range is full from its start).
    pub fn gain(&self, param: f32) -> f32 {
        let ramp = |x: f32, [a, b]: [f32; 2]| if b <= a { if x >= a { 1.0 } else { 0.0 } } else { ((x - a) / (b - a)).clamp(0.0, 1.0) };
        self.volume * ramp(param, self.fade_in) * (1.0 - ramp(param, self.fade_out))
    }
}

/// A live volume factor for the loops following this entity (a crossfade input's gain).
#[derive(Component, Clone, Copy, Debug)]
pub struct LoopGain(pub f32);

#[derive(Deserialize, Clone, Debug)]
pub struct NotifySound {
    pub time: f32,
    pub cue: String,
    pub volume: f32,
    pub pitch: f32,
}

impl SoundDb {
    pub fn load() -> Self {
        std::fs::read_to_string("assets/data/sounds.json").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }
}

/// Play a cue (`at`: where; None for 2D).
#[derive(Message, Clone, Debug)]
pub struct Sound {
    pub cue: String,
    pub at: Option<Vec3>,
    /// The AudioComponent's VolumeMultiplier, if it's played through one.
    pub volume: f32,
    /// Overrides the cue's attenuation (an AudioComponent's AttenuationSettings).
    pub attenuation: Option<String>,
    /// Follow this entity (loops on a zombie) instead of staying at `at`.
    pub follow: Option<Entity>,
}

impl Sound {
    pub fn at(cue: impl Into<String>, at: Vec3) -> Self {
        Self { cue: cue.into(), at: Some(at), volume: 1.0, attenuation: None, follow: None }
    }

    pub fn ui(cue: impl Into<String>) -> Self {
        Self { cue: cue.into(), at: None, volume: 1.0, attenuation: None, follow: None }
    }
}

/// A playing positioned sound whose volume follows the listener (loops, and one-shots on movers).
#[derive(Component)]
pub struct Emitter {
    base: f32,
    att: Attenuation,
    at: Vec3,
    follow: Option<Entity>,
}

/// An entity with a looping voice playing (an AudioComponent like the zombies' Zombie_Cue).
/// Removing it stops the loop; the loop also stops beyond its attenuation's reach.
#[derive(Component)]
pub struct Voiced;

/// A looping AudioComponent at a place (a campfire's SC_CampFire, a running generator's hum):
/// its loop plays while `active` and the listener is within the cue's attenuation reach.
#[derive(Component, Clone, Debug)]
pub struct AmbientLoop {
    pub cue: String,
    /// The AudioComponent's VolumeMultiplier.
    pub volume: f32,
    pub active: bool,
}

/// How many times each cue has started (for the tour's checks).
#[derive(Resource, Default, Debug)]
pub struct Played(pub std::collections::BTreeMap<String, u32>);

/// The listener: the gameplay camera.
#[derive(Component)]
pub struct Listener;

pub fn plugin(app: &mut App) {
    app.insert_resource(SoundDb::load())
        .init_resource::<Played>()
        .add_message::<Sound>()
        .add_observer(click)
        .add_systems(Update, (action_sounds, ambient_loops, play, follow).chain());
    music::plugin(app);
}

/// ButtonWidget's click: UI_click_sound_Cue on every button press.
fn click(_ev: On<bevy::ui_widgets::Activate>, mut sound: MessageWriter<Sound>) {
    sound.write(Sound::ui("UI_click_sound_Cue"));
}

/// UE's distance attenuation (FAttenuationSettings): 1 inside `inner`, then over `falloff`.
/// Logarithmic is 0.5 x -ln(alpha) clamped to 0..1 (alpha = how far through the falloff), from UE's
/// AttenuationEval as we remember it (a guess); the other models fall back to linear.
pub fn attenuate(distance: f32, att: &Attenuation) -> f32 {
    if distance <= att.inner {
        return 1.0;
    }
    let alpha = ((distance - att.inner) / att.falloff.max(0.01)).clamp(0.0, 1.0);
    if alpha >= 1.0 {
        return 0.0;
    }
    match att.model {
        Falloff::Logarithmic => (-0.5 * alpha.max(1e-4).ln()).clamp(0.0, 1.0),
        _ => 1.0 - alpha,
    }
}

/// The SoundClass's slider x MASTER (sliders default to 1).
pub fn class_volume(db: &SoundDb, store: &SettingsStore, class: Option<&str>) -> f32 {
    let slider = |label: &str| match store.find(&row_key("settings", "Audio", label)) {
        Some(Value::Num(v)) => v,
        _ => 1.0,
    };
    let own = class.and_then(|c| db.classes.get(c)).map_or(1.0, |label| slider(label));
    own * slider("MASTER")
}

/// Pick a variant by weight.
pub fn pick(cue: &Cue, roll: f32) -> Option<&Variant> {
    let total: f32 = cue.variants.iter().map(|v| v.weight.max(0.0)).sum();
    let mut left = roll * total;
    for v in &cue.variants {
        left -= v.weight.max(0.0);
        if left < 0.0 {
            return Some(v);
        }
    }
    cue.variants.last()
}

#[allow(clippy::too_many_arguments)]
fn play(
    mut commands: Commands,
    mut msgs: MessageReader<Sound>,
    db: Res<SoundDb>,
    store: Res<SettingsStore>,
    assets: Res<AssetServer>,
    listener: Query<&GlobalTransform, With<Listener>>,
    fallback: Query<&GlobalTransform, With<Camera3d>>,
    mut played: ResMut<Played>,
) {
    if msgs.is_empty() {
        return;
    }
    let ear = listener.iter().next().or(fallback.iter().next()).map(|t| t.translation());
    let mut rng = rand::thread_rng();
    for s in msgs.read() {
        let Some(cue) = db.cues.get(&s.cue) else {
            warn!("no sound {}", s.cue);
            continue;
        };
        let Some(v) = pick(cue, rng.gen_range(0.0..1.0)) else { continue };
        let att = s.attenuation.as_ref().or(cue.attenuation.as_ref()).and_then(|a| db.attenuation.get(a)).copied().filter(|a| a.spatialize);
        let base = cue.volume * s.volume * rng.gen_range(v.volume[0]..=v.volume[1].max(v.volume[0])) * class_volume(&db, &store, cue.class.as_deref());
        let pitch = cue.pitch * rng.gen_range(v.pitch[0]..=v.pitch[1].max(v.pitch[0]));
        let gain = match (s.at, att, ear) {
            (Some(at), Some(a), Some(ear)) => attenuate(at.distance(ear), &a),
            _ => 1.0,
        };
        // Beyond the falloff a one-shot isn't worth starting.
        if gain <= 0.0 && !v.looping {
            continue;
        }
        *played.0.entry(s.cue.clone()).or_insert(0) += 1;
        for w in &v.waves {
            let mut e = commands.spawn((
                AudioPlayer::new(assets.load::<AudioSource>(w.clone())),
                PlaybackSettings { mode: if v.looping { PlaybackMode::Loop } else { PlaybackMode::Despawn }, volume: Volume::Linear(base * gain), speed: pitch, ..default() },
                Transform::from_translation(s.at.unwrap_or_default()),
            ));
            if let (Some(at), Some(a)) = (s.at, att) {
                e.insert(Emitter { base, att: a, at, follow: s.follow });
            }
        }
        debug!("sound {} gain {gain:.2}", s.cue);
    }
}

/// The survivor's action montage's PlaySound notifies, as its clip passes them (at the action's
/// play rate; a new action starts over).
#[allow(clippy::type_complexity)]
fn action_sounds(
    time: Res<Time>,
    db: Res<SoundDb>,
    player: Query<(&Transform, &crate::models::UpperAction, &crate::models::Stance), (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut state: Local<(u32, f32, usize)>,
    mut sound: MessageWriter<Sound>,
) {
    let Ok((t, action, stance)) = player.single() else { return };
    let (serial, elapsed, next) = &mut *state;
    if action.serial != *serial {
        *serial = action.serial;
        *elapsed = 0.0;
        *next = 0;
    }
    let Some(kind) = action.kind else { return };
    let Some(list) = crate::models::AnimSet::Survivor.action_clip(*stance, kind).and_then(|clip| db.action_sounds.get(clip)) else { return };
    *elapsed += time.delta_secs() * action.rate();
    while let Some(n) = list.get(*next).filter(|n| n.time <= *elapsed) {
        let mut s = Sound::at(n.cue.clone(), t.translation + Vec3::Y * 1.3);
        s.volume = n.volume;
        sound.write(s);
        *next += 1;
    }
}

/// Start the loops in reach that should play, stop the ones switched off.
#[allow(clippy::type_complexity)]
fn ambient_loops(
    mut commands: Commands,
    db: Res<SoundDb>,
    listener: Query<&GlobalTransform, With<Listener>>,
    loops: Query<(Entity, &GlobalTransform, &AmbientLoop, Has<Voiced>)>,
    mut sound: MessageWriter<Sound>,
) {
    let Some(ear) = listener.iter().next().map(|t| t.translation()) else { return };
    for (e, t, l, voiced) in &loops {
        if !l.active {
            if voiced {
                commands.entity(e).remove::<Voiced>();
            }
            continue;
        }
        if voiced {
            continue;
        }
        let reach = db.cues.get(&l.cue).and_then(|c| c.attenuation.as_ref()).and_then(|a| db.attenuation.get(a)).map_or(30.0, |a| a.inner + a.falloff);
        if t.translation().distance(ear) < reach {
            commands.entity(e).insert(Voiced);
            sound.write(Sound { cue: l.cue.clone(), at: Some(t.translation()), volume: l.volume, attenuation: None, follow: Some(e) });
        }
    }
}

/// Keep positioned sounds' volume on the listener's distance; loops on despawned entities stop.
fn follow(
    mut commands: Commands,
    mut emitters: Query<(Entity, &mut Emitter, Option<&mut AudioSink>)>,
    targets: Query<(&GlobalTransform, Has<Voiced>, Option<&LoopGain>)>,
    listener: Query<&GlobalTransform, With<Listener>>,
    fallback: Query<&GlobalTransform, With<Camera3d>>,
) {
    let Some(ear) = listener.iter().next().or(fallback.iter().next()).map(|t| t.translation()) else { return };
    for (e, mut em, sink) in &mut emitters {
        let mut gain = 1.0;
        if let Some(f) = em.follow {
            match targets.get(f) {
                Ok((t, true, g)) => {
                    em.at = t.translation();
                    gain = g.map_or(1.0, |g| g.0);
                }
                _ => {
                    commands.entity(e).despawn();
                    continue;
                }
            }
            // Out of earshot: the loop stops (and starts again when the owner's system re-adds it).
            if em.at.distance(ear) > em.att.inner + em.att.falloff + 5.0 {
                commands.entity(e).despawn();
                commands.entity(f).try_remove::<Voiced>();
                continue;
            }
        }
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(em.base * gain * attenuate(em.at.distance(ear), &em.att)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attenuation_follows_the_asset() {
        let lin = Attenuation { inner: 0.5, falloff: 20.0, model: Falloff::Linear, spatialize: true };
        assert_eq!(attenuate(0.3, &lin), 1.0);
        assert!((attenuate(10.5, &lin) - 0.5).abs() < 1e-6);
        assert_eq!(attenuate(25.0, &lin), 0.0);
        let log = Attenuation { inner: 20.0, falloff: 250.0, model: Falloff::Logarithmic, spatialize: true };
        assert_eq!(attenuate(10.0, &log), 1.0);
        let mid = attenuate(145.0, &log);
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
        assert_eq!(attenuate(300.0, &log), 0.0);
    }

    #[test]
    fn crossfades_ramp_in_and_out() {
        let idle = CrossfadeInput { cue: String::new(), fade_in: [0.0, 0.0], fade_out: [15.0, 25.0], volume: 1.0 };
        let low = CrossfadeInput { cue: String::new(), fade_in: [0.5, 20.0], fade_out: [40.0, 50.0], volume: 1.0 };
        assert_eq!(idle.gain(0.0), 1.0);
        assert!((idle.gain(20.0) - 0.5).abs() < 1e-6);
        assert_eq!(idle.gain(30.0), 0.0);
        assert_eq!(low.gain(0.0), 0.0);
        assert_eq!(low.gain(30.0), 1.0);
        assert!((low.gain(45.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn variants_are_picked_by_weight() {
        let v = |w: f32, name: &str| Variant { waves: vec![name.into()], weight: w, volume: [1.0, 1.0], pitch: [1.0, 1.0], looping: false };
        let cue = Cue { class: None, attenuation: None, volume: 1.0, pitch: 1.0, variants: vec![v(1.0, "a"), v(3.0, "b")] };
        assert_eq!(pick(&cue, 0.1).unwrap().waves[0], "a");
        assert_eq!(pick(&cue, 0.3).unwrap().waves[0], "b");
        assert_eq!(pick(&cue, 0.99).unwrap().waves[0], "b");
    }

    #[test]
    fn the_games_cues_are_there() {
        let db = SoundDb::load();
        if db.cues.is_empty() {
            return;
        }
        for c in ["S_EmptyFire_Cue", "GunJam_Cue", "Head_shot_1_Cue", "ZombieBreathLoopCue", "UI_click_sound_Cue", "explosion_large_06_Cue", "FishReelIN_Cue"] {
            assert!(db.cues.get(c).is_some_and(|c| !c.variants.is_empty()), "{c}");
        }
        assert_eq!(db.classes.get("EffectsAudio").map(String::as_str), Some("EFFECTS"));
        assert!(db.attenuation.contains_key("GunshotsRifle"));
        // The reload montages' mag out / mag in notifies, in clip order.
        let reload = &db.action_sounds["Anims/Other/Reload_Rifle_Hip"];
        assert_eq!(reload.iter().map(|n| n.cue.as_str()).collect::<Vec<_>>(), ["AR_Mag_Out_Cue", "AR_Mag_IN_2_Cue"]);
        assert!(reload[0].time < reload[1].time);
        assert_eq!(crate::models::AnimSet::Survivor.action_clip(crate::models::Stance::Rifle, crate::models::ActionKind::Reload), Some("Anims/Other/Reload_Rifle_Hip"));
    }
}
