//! The survivor's footsteps as zombies hear them. Player_AnimBP's `AnimNotify_Footstep` (the
//! `Footstep` notifies on the locomotion sequences) calls BP_PlayerCharacter's `Footstep Sound`,
//! which reports a hearing noise of `MaxRange / SneakingMultiplier`:
//!
//! - sprinting (`Sprinting?`): 1500 cm;
//! - crouched: 50 cm, or 150 cm crouch-sprinting;
//! - otherwise 750 cm.
//!
//! Each crouched step also gives 0.2-0.4 Sneaking XP (AddXP_Sneaking). The surface sounds
//! themselves aren't played.

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
    fn crouching_is_quieter() {
        assert_eq!(footstep_range(Locomotion::Sprint), 15.0);
        assert_eq!(footstep_range(Locomotion::Run), 7.5);
        assert_eq!(footstep_range(Locomotion::AimFwd), 7.5);
        assert_eq!(footstep_range(Locomotion::CrouchWalk), 0.5);
        assert_eq!(footstep_range(Locomotion::CrouchAimLeft), 0.5);
        assert_eq!(footstep_range(Locomotion::CrouchSprint), 1.5);
    }
}
