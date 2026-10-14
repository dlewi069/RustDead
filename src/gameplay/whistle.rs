//! The Distraction whistle: IA_Distract (O) in BP_PlayerCharacter (decompiled).
//!
//! - Without the tech tree's Skill_Distraction: "Distraction Skill Not Unlocked".
//! - With it, unless one is still playing (DistractNoisePlaying?): the Whistle_Cue (a quarter as
//!   loud crouched), then after a 0.5 s Delay a noise zombies hear 40 m away (15 m crouched);
//!   another 2 s and it can be used again. (Which ReportNoiseEvent the Delay resumes into is read
//!   from context: the 1500 / 4000 cm one next to the crouch check; a guess.)

use bevy::prelude::*;

use super::zombies::Noise;
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{CharacterModel, Locomotion};

const NOISE_AFTER: f32 = 0.5;
const READY_AFTER: f32 = NOISE_AFTER + 2.0;

/// Seconds since the whistle, while one is under way.
#[derive(Resource, Default)]
pub struct Whistle(pub Option<f32>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Whistle>()
        .add_systems(OnEnter(AppState::InGame), |mut w: ResMut<Whistle>| w.0 = None)
        .add_systems(Update, whistle.run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)).run_if(super::player_in_control));
}

/// The noise's range: 40 m, 15 m crouched.
pub fn whistle_range(crouched: bool) -> f32 {
    if crouched { 15.0 } else { 40.0 }
}

#[allow(clippy::too_many_arguments)]
fn whistle(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<CurrentGame>,
    mut state: ResMut<Whistle>,
    player: Query<(&Transform, &Locomotion), With<CharacterModel>>,
    mut noise: MessageWriter<Noise>,
    mut notify: MessageWriter<super::notify::Notify>,
    mut sound: MessageWriter<crate::audio::Sound>,
) {
    let Ok((t, loco)) = player.single() else { return };
    if let Some(age) = state.0.as_mut() {
        let before = *age;
        *age += time.delta_secs();
        if before < NOISE_AFTER && *age >= NOISE_AFTER {
            let range = whistle_range(loco.crouched());
            noise.write(Noise { at: t.translation, range });
            info!("whistle heard {range} m away");
        }
        if *age >= READY_AFTER {
            state.0 = None;
        }
        return;
    }
    if !keys.just_pressed(KeyCode::KeyO) {
        return;
    }
    if !game.state.as_ref().is_some_and(|s| s.tech.has(crate::progression::tech::DISTRACTION)) {
        notify.write(super::notify::Notify::new("Distraction Skill Not Unlocked"));
        return;
    }
    state.0 = Some(0.0);
    // Whistle_Cue, a quarter as loud crouched.
    let mut s = crate::audio::Sound::at("Whistle_Cue", t.translation + Vec3::Y * 1.6);
    s.volume = if loco.crouched() { 0.25 } else { 1.0 };
    sound.write(s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crouching_whistles_quieter() {
        assert_eq!(whistle_range(false), 40.0);
        assert_eq!(whistle_range(true), 15.0);
    }
}
