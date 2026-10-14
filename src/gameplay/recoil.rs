//! Weapon recoil: BP_FirearmPickup's StartRecoil, WeaponRecoilValues and its two timelines
//! (decompiled).
//!
//! - Every shot (FireBullet) stops both timelines and rolls a kick: pitch = RecoilVertical
//!   ± VerticalRandomDeviation, yaw = ±(RecoilHorizontal ± HorizontalRandomDeviation), each sign a
//!   RandomBool. Hip fire doubles it, crouching takes 0.75 of the aimed kick (1 from the hip).
//!   GetVertical/HorizontalRecoilStrength (1 + the attachments' RecoilControl stat / 100) aren't
//!   ported with the attachments, so they're 1.
//! - RecoilTL (0→1 linear over 1 s, played at RecoilRate × 5) eases the view through the kick,
//!   EaseOut exponent 2, as AddControllerPitch/YawInput deltas.
//! - When it finishes, a 0.1 s Delay; if no new kick is playing by then, ResetRecoilTL (rate
//!   RecoilRate) eases back kick / ResetRecoilDivider (EaseInOut exponent 2). Only the last shot's
//!   kick is taken back, so a burst climbs.
//! - The game's `bEnableLegacyInputScales=True` (DefaultInput.ini) turns one unit of pitch input
//!   into -2.5° (BaseGame.ini InputPitchScale: negative input looks up) and of yaw into +2.5°
//!   (to the right).

use bevy::prelude::*;
use rand::Rng;

use super::items::FirearmData;

/// UE's legacy InputPitchScale / InputYawScale (degrees per unit of input).
const PITCH_SCALE: f32 = -2.5;
const YAW_SCALE: f32 = 2.5;
/// The Delay between RecoilTL finishing and the reset.
const RESET_DELAY: f32 = 0.1;

/// Recoil under way. The camera takes `pending` (view degrees: up, right) each frame.
#[derive(Resource, Default, Debug)]
pub struct Recoil {
    /// The last kick, in input units (pitch, yaw).
    kick: Vec2,
    /// Rates of the kick and the reset (timeline play rates; both are 1 s long).
    rate: f32,
    reset_rate: f32,
    divider: f32,
    /// RecoilTL's and ResetRecoilTL's positions, while playing.
    kicking: Option<f32>,
    resetting: Option<f32>,
    /// The Delay node's countdown.
    delay: Option<f32>,
    /// TempPitch/TempYaw: what the playing timeline has applied so far (input units).
    applied: Vec2,
    /// View change not yet given to the camera: (degrees up, degrees right).
    pub pending: Vec2,
}

/// The kick for one shot (pitch, yaw input units).
pub fn kick(fa: &FirearmData, hipfire: bool, crouching: bool, rng: &mut impl Rng) -> Vec2 {
    let mult = match (crouching, hipfire) {
        (true, true) => 1.0,
        (true, false) => 0.75,
        (false, true) => 2.0,
        (false, false) => 1.0,
    };
    let sign = |rng: &mut dyn rand::RngCore| if rng.gen_bool(0.5) { 1.0 } else { -1.0 };
    let pitch = fa.recoil_vertical + sign(rng) * fa.recoil_vertical_deviation;
    let yaw = sign(rng) * (fa.recoil_horizontal + sign(rng) * fa.recoil_horizontal_deviation);
    Vec2::new(pitch, yaw) * mult
}

/// KismetMathLibrary.Ease's EaseOut with exponent 2.
pub fn ease_out(a: f32) -> f32 {
    1.0 - (1.0 - a).powi(2)
}

/// KismetMathLibrary.Ease's EaseInOut with exponent 2.
pub fn ease_in_out(a: f32) -> f32 {
    if a < 0.5 { 0.5 * (2.0 * a).powi(2) } else { 1.0 - 0.5 * (2.0 * (1.0 - a)).powi(2) }
}

/// Input units (pitch, yaw) to view degrees (up, right).
fn view(input: Vec2) -> Vec2 {
    Vec2::new(input.x * PITCH_SCALE, input.y * YAW_SCALE)
}

impl Recoil {
    /// StartRecoil: a shot from `fa` with this kick.
    pub fn start(&mut self, fa: &FirearmData, kick: Vec2) {
        self.kick = kick;
        self.rate = fa.recoil_rate.max(0.01) * 5.0;
        self.reset_rate = fa.recoil_rate.max(0.01);
        self.divider = if fa.reset_recoil_divider == 0.0 { 1.0 } else { fa.reset_recoil_divider };
        self.resetting = None;
        self.kicking = Some(0.0);
        self.applied = Vec2::ZERO;
    }

    /// Is a kick or its reset still moving the view?
    pub fn busy(&self) -> bool {
        self.kicking.is_some() || self.resetting.is_some() || self.delay.is_some()
    }

    pub fn tick(&mut self, dt: f32) {
        if let Some(t) = self.kicking.as_mut() {
            *t = (*t + dt * self.rate).min(1.0);
            let at = self.kick * ease_out(*t);
            self.pending += view(at - self.applied);
            self.applied = at;
            if *t >= 1.0 {
                self.kicking = None;
                // A Delay already counting down isn't restarted.
                self.delay.get_or_insert(RESET_DELAY);
            }
        } else if let Some(t) = self.resetting.as_mut() {
            *t = (*t + dt * self.reset_rate).min(1.0);
            let at = self.kick / self.divider * ease_in_out(*t);
            self.pending -= view(at - self.applied);
            self.applied = at;
            if *t >= 1.0 {
                self.resetting = None;
            }
        }
        if let Some(d) = self.delay.as_mut() {
            *d -= dt;
            if *d <= 0.0 {
                self.delay = None;
                if self.kicking.is_none() {
                    self.applied = Vec2::ZERO;
                    self.resetting = Some(0.0);
                }
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Recoil>()
        .add_systems(OnEnter(crate::menus::AppState::InGame), |mut r: ResMut<Recoil>| *r = Recoil::default())
        .add_systems(Update, (|time: Res<Time>, mut r: ResMut<Recoil>| r.tick(time.delta_secs())).run_if(in_state(crate::menus::AppState::InGame)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ak74() -> FirearmData {
        FirearmData {
            recoil_vertical: -0.5,
            recoil_horizontal: 0.0,
            recoil_vertical_deviation: 0.0,
            recoil_horizontal_deviation: 0.15,
            recoil_rate: 2.0,
            reset_recoil_divider: 1.5,
            ..default()
        }
    }

    #[test]
    fn kicks_follow_weapon_recoil_values() {
        let mut rng = rand::thread_rng();
        let fa = ak74();
        for _ in 0..50 {
            let aimed = kick(&fa, false, false, &mut rng);
            assert_eq!(aimed.x, -0.5);
            assert!((aimed.y.abs() - 0.15).abs() < 1e-6);
            assert_eq!(kick(&fa, true, false, &mut rng).x, -1.0, "hip fire doubles it");
            assert_eq!(kick(&fa, false, true, &mut rng).x, -0.375, "crouched and aimed: 0.75");
            assert_eq!(kick(&fa, true, true, &mut rng).x, -0.5, "crouched from the hip: 1");
        }
    }

    #[test]
    fn eases_match_ue() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(0.5), 0.75);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_in_out(0.25), 0.125);
        assert_eq!(ease_in_out(0.5), 0.5);
        assert_eq!(ease_in_out(1.0), 1.0);
    }

    #[test]
    fn a_shot_kicks_up_then_two_thirds_come_back() {
        let fa = ak74();
        let mut r = Recoil::default();
        r.start(&fa, Vec2::new(-0.5, 0.2));
        // RecoilTL at 2 x 5 = 10x: the whole kick in 0.1 s.
        for _ in 0..10 {
            r.tick(0.01);
        }
        assert!((r.pending - Vec2::new(1.25, 0.5)).length() < 1e-4, "{}", r.pending);
        // 0.1 s Delay, then the reset over 1 / 2 = 0.5 s.
        r.pending = Vec2::ZERO;
        for _ in 0..100 {
            r.tick(0.01);
        }
        assert!(!r.busy());
        assert!((r.pending + Vec2::new(1.25, 0.5) / 1.5).length() < 1e-4, "{}", r.pending);
    }

    #[test]
    fn a_new_shot_cancels_the_reset_so_bursts_climb() {
        let fa = ak74();
        let mut r = Recoil::default();
        for _ in 0..5 {
            r.start(&fa, Vec2::new(-0.5, 0.0));
            // 0.12 s apart: each kick (0.1 s) finishes, but its Delay runs out while the next kick
            // is playing, so there's no reset in between.
            for _ in 0..12 {
                r.tick(0.01);
            }
        }
        for _ in 0..100 {
            r.tick(0.01);
        }
        // Five kicks of 1.25°, only the last one's 2/3 taken back.
        assert!((r.pending.x - (5.0 * 1.25 - 1.25 / 1.5)).abs() < 1e-3, "{}", r.pending);
    }
}
