//! The level's sirens, decompiled from `BP_Siren` (7 placed, an SM_Prop_Speaker_03 pole):
//!
//! - BeginPlay stops its Audio (Siren_Cue, AttenuationSettings GunshotsRifle) and calls Event_Siren:
//!   a RandomSirenTimer of RandomTimeMin-RandomTimeMax (600-7200 s). When it fires
//!   (SirenPlayTimer) the siren activates, and a Delay of 138 s later deactivates it and calls
//!   Event_Siren again.
//! - ActivateSiren (if not Playing): Audio.FadeIn(5 s), Playing, and a SirenNoise timer every
//!   2.5 s (looping): AISense_Hearing.ReportNoiseEvent at the speaker, MaxRange 6000 (60 m).
//!   DeactivateSiren (if Playing): the noise timer cleared, Audio.FadeOut(5 s), not Playing.
//! - Interact ("Siren", option "Interact") is a FlipFlop: A clears the random timer and
//!   activates; B calls Event_Siren and deactivates. A random activation's 138 s Delay still runs
//!   after a press.
//!
//! Ours: the sirens' state lives in `power::Power::sirens` so the F prompt competes with the
//! other level gadgets'. Interact plays Loot_MidShelf_GrabItem_Montage (`doors::interact_doors`).
//! The outline while its prompt is up is `gameplay::outline`'s. Not ported: hiding the mesh beyond
//! 300 m (AIOSubject).

use bevy::prelude::*;
use rand::Rng;

/// BP_Siren's RandomTimeMin / RandomTimeMax.
const RANDOM_TIME: (f32, f32) = (600.0, 7200.0);
/// SirenPlayTimer's Delay before the siren goes quiet.
const PLAY_TIME: f32 = 138.0;
/// The SirenNoise timer.
const NOISE_EVERY: f32 = 2.5;
/// SirenNoise's ReportNoiseEvent MaxRange (6000 cm).
pub const NOISE_RANGE: f32 = 60.0;
/// Audio.FadeIn / FadeOut duration.
const FADE: f32 = 5.0;
/// The Audio component above the actor (RelativeLocation Z 746 cm).
const AUDIO_HEIGHT: f32 = 7.46;

#[derive(Clone, Debug, PartialEq)]
pub struct Siren {
    pub pos: Vec3,
    /// Playing.
    pub playing: bool,
    /// The interact FlipFlop's state (true after an A).
    flip: bool,
    /// RandomSirenTimer: seconds left, None when cleared.
    timer: Option<f32>,
    /// The 138 s Delay after a random start.
    stop_in: Option<f32>,
    /// The SirenNoise timer while playing.
    noise_in: Option<f32>,
    /// The Audio's fade volume (0-1).
    pub gain: f32,
}

/// What a tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SirenTick {
    pub started: bool,
    pub stopped: bool,
    /// SirenNoise fired.
    pub noise: bool,
}

pub fn random_time(rng: &mut impl Rng) -> f32 {
    rng.gen_range(RANDOM_TIME.0..RANDOM_TIME.1)
}

impl Siren {
    /// BeginPlay: quiet, Event_Siren.
    pub fn new(pos: Vec3, rng: &mut impl Rng) -> Self {
        Self { pos, playing: false, flip: false, timer: Some(random_time(rng)), stop_in: None, noise_in: None, gain: 0.0 }
    }

    /// Seconds until the random timer fires, if it's set.
    pub fn timer(&self) -> Option<f32> {
        self.timer
    }

    fn activate(&mut self) -> bool {
        if self.playing {
            return false;
        }
        self.playing = true;
        self.noise_in = Some(NOISE_EVERY);
        true
    }

    fn deactivate(&mut self) -> bool {
        if !self.playing {
            return false;
        }
        self.playing = false;
        self.noise_in = None;
        true
    }

    /// The interact FlipFlop. `again` is the next random time (Event_Siren on B).
    pub fn interact(&mut self, again: f32) -> SirenTick {
        self.flip = !self.flip;
        if self.flip {
            self.timer = None;
            SirenTick { started: self.activate(), ..default() }
        } else {
            self.timer = Some(again);
            SirenTick { stopped: self.deactivate(), ..default() }
        }
    }

    /// Advance the timers and the fade by `dt`; `again` rolls a new random time. What starts or
    /// stops in this tick counts its time from the tick's end.
    pub fn tick(&mut self, dt: f32, mut again: impl FnMut() -> f32) -> SirenTick {
        let mut out = SirenTick::default();
        if let Some(t) = self.noise_in.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                *t += NOISE_EVERY;
                out.noise = true;
            }
        }
        let to = if self.playing { 1.0 } else { 0.0 };
        self.gain += (to - self.gain).clamp(-dt / FADE, dt / FADE);
        // Both timers run first, so one set in this tick starts counting from its end.
        let (timer, stop_in) = (self.timer.map(|t| t - dt), self.stop_in.map(|t| t - dt));
        self.timer = timer;
        self.stop_in = stop_in;
        if stop_in.is_some_and(|t| t <= 0.0) {
            self.stop_in = None;
            out.stopped |= self.deactivate();
            self.timer = Some(again());
        }
        if timer.is_some_and(|t| t <= 0.0) {
            self.timer = None;
            out.started |= self.activate();
            self.stop_in = Some(PLAY_TIME);
        }
        out
    }
}

/// A siren's Audio (index into `Power::sirens`).
#[derive(Component)]
pub(super) struct SirenAudio(usize);

pub(super) fn setup_sirens(mut commands: Commands, power: Option<Res<super::power::Power>>) {
    let Some(power) = power else { return };
    for (i, s) in power.sirens.iter().enumerate() {
        commands.spawn((
            Transform::from_translation(s.pos + Vec3::Y * AUDIO_HEIGHT),
            crate::audio::AmbientLoop { cue: "Siren_Cue".into(), volume: 1.0, active: false },
            crate::audio::LoopAttenuation("GunshotsRifle".into()),
            crate::audio::LoopGain(0.0),
            SirenAudio(i),
            DespawnOnExit(crate::menus::AppState::InGame),
        ));
    }
}

/// Run the sirens' timers: SirenNoise draws zombies, the Audio fades with Playing.
pub(super) fn run_sirens(
    time: Res<Time>,
    power: Option<ResMut<super::power::Power>>,
    mut noise: MessageWriter<crate::gameplay::zombies::Noise>,
    mut audio: Query<(&SirenAudio, &mut crate::audio::AmbientLoop, &mut crate::audio::LoopGain)>,
) {
    let Some(mut power) = power else { return };
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    // Ticking mustn't mark Power changed every frame (the lamps re-pick on a change).
    let power = power.bypass_change_detection();
    for (i, s) in power.sirens.iter_mut().enumerate() {
        let t = s.tick(dt, || random_time(&mut rng));
        if t.started {
            info!("siren {i} activated");
        }
        if t.stopped {
            info!("siren {i} deactivated");
        }
        if t.noise {
            noise.write(crate::gameplay::zombies::Noise { at: s.pos, range: NOISE_RANGE });
        }
    }
    for (a, mut l, mut g) in &mut audio {
        let Some(s) = power.sirens.get(a.0) else { continue };
        let active = s.gain > 0.0;
        if l.active != active {
            l.active = active;
        }
        if g.0 != s.gain {
            g.0 = s.gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn siren() -> Siren {
        Siren { pos: Vec3::ZERO, playing: false, flip: false, timer: Some(10.0), stop_in: None, noise_in: None, gain: 0.0 }
    }

    #[test]
    fn random_siren_plays_138_s_then_rearms() {
        let mut s = siren();
        assert_eq!(s.tick(5.0, || 900.0), SirenTick::default());
        let t = s.tick(5.0, || 900.0);
        assert!(t.started && s.playing && s.timer().is_none());
        // Fades in over 5 s; noise every 2.5 s.
        assert!(s.tick(2.5, || 900.0).noise);
        assert!((s.gain - 0.5).abs() < 1e-4);
        assert!(s.tick(2.5, || 900.0).noise);
        assert_eq!(s.gain, 1.0);
        let t = s.tick(133.0, || 900.0);
        assert!(t.stopped && !s.playing);
        assert_eq!(s.timer(), Some(900.0), "Event_Siren again");
        // Fades out over 5 s, no more noise.
        assert!(!s.tick(2.5, || 900.0).noise);
        assert!((s.gain - 0.5).abs() < 1e-4);
        s.tick(2.5, || 900.0);
        assert_eq!(s.gain, 0.0);
    }

    #[test]
    fn interact_flip_flops() {
        let mut s = siren();
        assert!(s.interact(1000.0).started && s.playing && s.timer().is_none());
        // Pressed on: plays until pressed again.
        assert!(!s.tick(500.0, || 1.0).stopped && s.playing);
        assert!(s.interact(1000.0).stopped && !s.playing);
        assert_eq!(s.timer(), Some(1000.0));
        assert!(random_time(&mut rand::thread_rng()) >= 600.0);
    }
}
