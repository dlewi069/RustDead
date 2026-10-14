//! The survivor's footsteps as zombies hear them. Player_AnimBP's `AnimNotify_Footstep` (the
//! `Footstep` notifies on the locomotion sequences) calls BP_PlayerCharacter's `Footstep Sound`,
//! which reports a hearing noise of `MaxRange / SneakingMultiplier`:
//!
//! - sprinting (`Sprinting?`): 1500 cm;
//! - crouched: 50 cm, or 150 cm crouch-sprinting;
//! - otherwise 750 cm.
//!
//! Each crouched step also gives 0.2-0.4 Sneaking XP (AddXP_Sneaking).
//!
//! The step's sound (Player_AnimBP): a 5 cm sphere trace 125 cm down, its surface type picks
//! `<Surface>Sprint_Cue` sprinting or `<Surface>_Cue` otherwise, at volume 0.05 (0.025 crouched).
//! Ours: on the landscape the surface is the dominant paint layer's PhysMaterial (sounds.json
//! `terrain_surfaces`); on props it's the default, Concrete (props' physical materials aren't
//! exported).

use bevy::prelude::*;

use rand::Rng;

use super::zombies::Noise;
use crate::progression::passive::Passive;
use crate::level::{KinematicBody, LevelZombie};
use crate::models::{AnimLibrary, Animated, CharacterModel, Locomotion, Stance};

/// The locomotion clip's play head, mirrored from `drive_animation` (a new state starts its
/// clip from the beginning), and the last step's range for the tour.
#[derive(Resource, Default, Debug)]
pub struct Footsteps {
    playing: Option<(Stance, Locomotion)>,
    t: f32,
    pub last_range: Option<f32>,
    pub count: u32,
}

/// `Footstep Sound`'s MaxRange in metres.
pub fn footstep_range(state: Locomotion) -> f32 {
    match state {
        Locomotion::Sprint => 15.0,
        Locomotion::CrouchSprint => 1.5,
        s if s.crouched() => 0.5,
        _ => 7.5,
    }
}

/// The footstep cue for a surface and gait.
pub fn step_cue(surface: &str, state: Locomotion) -> String {
    if state == Locomotion::Sprint { format!("{surface}Sprint_Cue") } else { format!("{surface}_Cue") }
}

/// The landscape's paint layers (terrain.json order = terrain_weights.png channels).
pub struct Surfaces {
    layers: Vec<String>,
    weights: Option<crate::level::grass::Weights>,
}

impl Surfaces {
    fn load(terrain: &crate::level::Terrain) -> Self {
        #[derive(serde::Deserialize)]
        struct Layers {
            layers: Vec<String>,
        }
        let dir = crate::level::LEVEL_DIR;
        let layers = std::fs::read_to_string(format!("{dir}/terrain.json")).ok().and_then(|t| serde_json::from_str::<Layers>(&t).ok()).map(|l| l.layers).unwrap_or_default();
        Self { layers, weights: crate::level::grass::Weights::load(dir, terrain) }
    }

    /// The surface of the most-painted layer at x/z (unpainted: the first layer's, a guess).
    pub fn at<'a>(&self, map: &'a std::collections::HashMap<String, String>, x: f32, z: f32) -> &'a str {
        let Some(w) = &self.weights else { return "Concrete" };
        let best = (0..self.layers.len()).max_by(|&a, &b| w.get(a, x, z).total_cmp(&w.get(b, x, z))).unwrap_or(0);
        self.layers.get(best).and_then(|l| map.get(l)).map_or("Concrete", String::as_str)
    }
}

/// How many notify `times` a looping clip of length `duration` passes going from play time
/// `from` to `to` (unwrapped seconds, `from` excluded).
pub fn notifies_passed(times: &[f32], duration: f32, from: f32, to: f32) -> usize {
    if duration <= 0.0 || to <= from {
        return 0;
    }
    times
        .iter()
        .map(|&t| {
            // Occurrences k*duration + t in (from, to].
            let count = |x: f32| ((x - t) / duration).floor() as i64;
            (count(to) - count(from)).max(0) as usize
        })
        .sum()
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Footsteps>().add_systems(
        Update,
        footsteps.after(crate::models::anim::drive_animation).run_if(in_state(crate::menus::AppState::InGame)),
    );
}

#[allow(clippy::type_complexity)]
fn footsteps(
    time: Res<Time>,
    lib: Res<AnimLibrary>,
    mut steps: ResMut<Footsteps>,
    mut noise: MessageWriter<Noise>,
    mut game: ResMut<crate::menus::CurrentGame>,
    (sounds, terrain, mut surfaces, mut sound): (Res<crate::audio::SoundDb>, Option<Res<crate::level::Terrain>>, Local<Option<Surfaces>>, MessageWriter<crate::audio::Sound>),
    player: Query<(&Transform, &Animated, &Locomotion, Option<&Stance>, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
) {
    let Ok((t, Animated(set), &state, stance, body)) = player.single() else { return };
    let stance = stance.copied().unwrap_or_default();
    let Some((_, timing)) = lib.timing(*set, stance, state) else { return };
    if steps.playing != Some((stance, state)) {
        steps.playing = Some((stance, state));
        steps.t = 0.0;
    }
    let from = steps.t;
    steps.t += time.delta_secs() * timing.rate;
    if !body.grounded || timing.footsteps.is_empty() {
        return;
    }
    let n = notifies_passed(&timing.footsteps, timing.duration, from, steps.t);
    if n > 0 {
        let passive = game.state.as_mut().map(|s| &mut s.passive);
        let sneaking = passive.as_ref().map_or(1.0, |p| p.multiplier(Passive::Sneaking));
        let range = footstep_range(state) / sneaking.max(0.01);
        if state.crouched()
            && let Some(p) = passive
        {
            let mut rng = rand::thread_rng();
            for _ in 0..n {
                p.add_xp(Passive::Sneaking, rng.gen_range(0.2..=0.4));
            }
        }
        noise.write(Noise { at: t.translation, range });
        if let Some(terrain) = terrain.as_deref() {
            let surfaces = surfaces.get_or_insert_with(|| Surfaces::load(terrain));
            let on_landscape = terrain.ground_under(t.translation).is_some_and(|h| (t.translation.y - h).abs() < 0.3);
            let surface = if on_landscape { surfaces.at(&sounds.terrain_surfaces, t.translation.x, t.translation.z) } else { "Concrete" };
            let mut s = crate::audio::Sound::at(step_cue(surface, state), t.translation);
            s.volume = if state.crouched() { 0.025 } else { 0.05 };
            sound.write(s);
        }
        steps.last_range = Some(range);
        steps.count += n as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_fire_as_the_clip_passes_its_notifies() {
        // ThirdPersonRun: 0.80 s, steps at 0.29 and 0.68.
        let times = [0.29, 0.68];
        assert_eq!(notifies_passed(&times, 0.8, 0.0, 0.2), 0);
        assert_eq!(notifies_passed(&times, 0.8, 0.2, 0.3), 1);
        assert_eq!(notifies_passed(&times, 0.8, 0.3, 0.7), 1);
        // Across the loop: 0.68 in this lap, 0.29 in the next.
        assert_eq!(notifies_passed(&times, 0.8, 0.6, 1.1), 2);
        assert_eq!(notifies_passed(&times, 0.8, 0.0, 8.0), 20);
    }

    #[test]
    fn step_cues_follow_gait_and_surface() {
        assert_eq!(step_cue("Grass", Locomotion::Sprint), "GrassSprint_Cue");
        assert_eq!(step_cue("Concrete", Locomotion::Run), "Concrete_Cue");
        assert_eq!(step_cue("Dirt", Locomotion::CrouchWalk), "Dirt_Cue");
    }

    #[test]
    fn crouching_is_quieter() {
        assert_eq!(footstep_range(Locomotion::Sprint), 15.0);
        assert_eq!(footstep_range(Locomotion::Run), 7.5);
        assert_eq!(footstep_range(Locomotion::AimFwd), 7.5);
        assert_eq!(footstep_range(Locomotion::CrouchWalk), 0.5);
        assert_eq!(footstep_range(Locomotion::CrouchAimLeft), 0.5);
        assert_eq!(footstep_range(Locomotion::CrouchSprint), 1.5);
    }
}
