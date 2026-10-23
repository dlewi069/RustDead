//! Combat Roll (IA_Roll = C): BP_PlayerCharacter.Svr_PlayerRoll, Client_RollRotation.
//!
//! - Needs the Skill_CombatRoll tech, 8 stamina (Svr_ReduceStamina 8), and not: falling, on a
//!   ladder, seated, in the inventory (any overlay here), already rolling, swimming, fishing
//!   (CurrentlyFishing?). (Also vehicles, binoculars and photo mode, which don't exist yet.)
//! - Client_RollRotation turns the survivor to the last movement input (the camera-relative WASD
//!   direction); with none they keep their facing (Character.RollFollowsPlayer is off: the actor's
//!   rotation, not the control rotation).
//! - Roll_Montage (Anims/Parkour/Roll at RateScale 1.5, the whole body) with
//!   foley_jump_movement_throw_02_Cue; its root motion carries the body forward (5.4 m over the
//!   clip; ours moves the KinematicBody along the facing, walls still stop it). IsPlayerRolling?
//!   holds movement for the BP's 1 s Delay, then clears and puts out a burning survivor
//!   (StopBurning).

use bevy::prelude::*;

use super::interact::GameOverlay;
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{ActionKind, CharacterModel, Locomotion, UpperAction};

/// Svr_PlayerRoll's stamina check and cost.
pub const STAMINA: f32 = 8.0;
/// The BP's Delay before IsPlayerRolling? clears.
pub const ROLLING: f32 = 1.0;

/// IsPlayerRolling? and the root motion's progress.
#[derive(Resource, Default)]
pub struct Roll {
    /// Seconds since the roll began.
    pub elapsed: Option<f32>,
    /// Root travel already applied (m).
    moved: f32,
    track: Option<(f32, Vec<f32>)>,
}

impl Roll {
    pub fn active(&self) -> bool {
        self.elapsed.is_some()
    }
}

/// Why a roll can't start (the BP just does nothing).
#[derive(Debug, PartialEq)]
pub enum Refused {
    NoTech,
    Tired,
    Busy,
}

/// Svr_PlayerRoll's checks.
pub fn can_roll(unlocked: bool, stamina: f32, grounded: bool, climbing: bool, sitting: bool, overlay_open: bool, rolling: bool) -> Result<(), Refused> {
    if !unlocked {
        return Err(Refused::NoTech);
    }
    if !grounded || climbing || sitting || overlay_open || rolling {
        return Err(Refused::Busy);
    }
    if stamina < STAMINA {
        return Err(Refused::Tired);
    }
    Ok(())
}

/// The roll's systems (the tour presses C before them).
#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct RollSet;

pub fn plugin(app: &mut App) {
    app.init_resource::<Roll>()
        .add_systems(OnEnter(AppState::InGame), |mut r: ResMut<Roll>| *r = Roll::default())
        .add_systems(
            Update,
            (start, roll_motion.before(crate::level::collision::move_bodies)).chain().in_set(RollSet).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)),
        );
}

/// The camera-relative WASD direction (main.rs's movement: W = where the camera looks).
fn input_direction(keys: &ButtonInput<KeyCode>, camera: Option<&GlobalTransform>) -> Option<Vec3> {
    let cam = camera?;
    let look = (cam.rotation() * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    let right = look.cross(Vec3::Y);
    let axis = |pos: &[KeyCode], neg: &[KeyCode]| keys.any_pressed(pos.iter().copied()) as i32 as f32 - keys.any_pressed(neg.iter().copied()) as i32 as f32;
    let fwd = axis(&[KeyCode::KeyW, KeyCode::ArrowUp], &[KeyCode::KeyS, KeyCode::ArrowDown]);
    let side = axis(&[KeyCode::KeyD, KeyCode::ArrowRight], &[KeyCode::KeyA, KeyCode::ArrowLeft]);
    let d = look * fwd + right * side;
    (d.length_squared() > 0.0).then(|| d.normalize())
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn start(
    keys: Res<ButtonInput<KeyCode>>,
    overlay: Res<GameOverlay>,
    mut roll: ResMut<Roll>,
    mut game: ResMut<CurrentGame>,
    camera: Query<&GlobalTransform, With<super::PlayerCamera>>,
    mut player: Query<(&mut Transform, &crate::level::KinematicBody, &Locomotion, &mut UpperAction), (With<CharacterModel>, With<crate::level::KinematicBody>)>,
    mut sound: MessageWriter<crate::audio::Sound>,
    fishing: Res<super::fishing::Fishing>,
) {
    if !keys.just_pressed(KeyCode::KeyC) {
        return;
    }
    let Ok((mut t, body, loco, mut action)) = player.single_mut() else { return };
    if body.swimming || fishing.active {
        return;
    }
    let Some(state) = game.state.as_mut() else { return };
    let unlocked = state.tech.has(crate::progression::tech::COMBAT_ROLL);
    if let Err(why) = can_roll(unlocked, state.survival.stamina, body.grounded, body.climbing, *loco == Locomotion::Sit, overlay.is_open(), roll.active()) {
        debug!("no roll: {why:?}");
        return;
    }
    state.survival.stamina -= STAMINA;
    if let Some(dir) = input_direction(&keys, camera.iter().next()) {
        t.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
    }
    action.start(ActionKind::Roll);
    sound.write(crate::audio::Sound::at("foley_jump_movement_throw_02_Cue", t.translation + Vec3::Y));
    if roll.track.is_none() {
        roll.track = crate::models::anim::root_forward(crate::models::anim::ROLL_CLIP);
    }
    roll.elapsed = Some(0.0);
    roll.moved = 0.0;
    info!("roll");
}

/// The montage's root motion along the facing, and the 1 s IsPlayerRolling? (then StopBurning).
fn roll_motion(time: Res<Time>, mut roll: ResMut<Roll>, mut burning: ResMut<super::burning::PlayerBurning>, mut player: Query<&mut Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>) {
    let Some(elapsed) = roll.elapsed else { return };
    let elapsed = elapsed + time.delta_secs();
    if let (Some((fps, track)), Ok(mut t)) = (roll.track.as_ref(), player.single_mut()) {
        let at = crate::models::anim::sample_track(*fps, track, elapsed * ActionKind::Roll.rate());
        let step = at - roll.moved;
        let forward = t.rotation * Vec3::Z;
        t.translation += forward.with_y(0.0).normalize_or_zero() * step;
        roll.moved = at;
    }
    if elapsed >= ROLLING {
        roll.elapsed = None;
        if burning.0.take().is_some() {
            info!("roll put out the fire");
        }
    } else {
        roll.elapsed = Some(elapsed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roll_needs_the_tech_stamina_and_footing() {
        assert_eq!(can_roll(false, 100.0, true, false, false, false, false), Err(Refused::NoTech));
        assert_eq!(can_roll(true, 7.9, true, false, false, false, false), Err(Refused::Tired));
        assert_eq!(can_roll(true, 8.0, true, false, false, false, false), Ok(()));
        for (grounded, climbing, sitting, overlay, rolling) in [(false, false, false, false, false), (true, true, false, false, false), (true, false, true, false, false), (true, false, false, true, false), (true, false, false, false, true)] {
            assert_eq!(can_roll(true, 100.0, grounded, climbing, sitting, overlay, rolling), Err(Refused::Busy));
        }
    }

    #[test]
    fn root_motion_carries_the_roll() {
        let Some((fps, track)) = crate::models::anim::root_forward(crate::models::anim::ROLL_CLIP) else { return };
        // 5.4 m over the 1.4 s clip, at RateScale 1.5: done by ~0.93 s, inside the 1 s hold.
        let end = crate::models::anim::sample_track(fps, &track, ROLLING * ActionKind::Roll.rate());
        assert!((end - 5.38).abs() < 0.05, "{end}");
        let half = crate::models::anim::sample_track(fps, &track, 0.3 * ActionKind::Roll.rate());
        assert!(half > 0.5 && half < end, "{half}");
        assert_eq!(crate::models::anim::sample_track(fps, &track, 0.0), 0.0);
    }
}
