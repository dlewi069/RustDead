//! Shove and stomp (V, `IA_Shove` -> `Svr_WeaponShove`, decompiled from BP_PlayerCharacter):
//!
//! - Nothing while rolling, swimming, falling, in the inventory or while a montage plays.
//! - Camera pitched down 45° or more: a stomp (Attack_Stomp_Montage_RM, 1.67 s). Its
//!   AnimNotify_Stomp window (0.50-0.90 s) runs `PlayerStompTrace` every 0.05 s: a 50 cm sphere
//!   at the right foot; the first pawn in it takes 20-40 damage, a noise is heard 5 m away and
//!   the trace stops.
//! - Otherwise, with 4 stamina (used up): a shove, the drawn weapon's `ShoveMontage` (or
//!   Anim_SmallMeleePush_Montage) at rate 1.15. Its AnimNotify_Shove window runs
//!   `WeaponShoveTrace` every 0.05 s: a 25 cm sphere swept from the left hand to the right; the
//!   first pawn hit gets Damage_Shoved (no damage) and the trace stops.
//! - A shoved zombie (BP_MasterZombie.Damage_Shoved) with a `ShovedAnimation` plays it, stops
//!   and can't attack until it ends (`zombies::Shoved`); crawlers and hounds have none and shrug
//!   it off.
//!
//! Ours: the hands and foot are fixed points ahead of the survivor (the melee swing uses the
//! same kind of reach test), and the sphere is tested every frame of the window.

use bevy::prelude::*;
use rand::Rng;

use super::combat::{Armed, Dead, Health, PlayerCamera};
use super::zombies::{Noise, SHOVED_SECS, Shoved};
use crate::level::{KinematicBody, LevelZombie};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{ActionKind, CharacterModel, Stance, UpperAction};

/// Svr_ReduceStamina before a shove (and the stamina it needs).
pub const SHOVE_STAMINA: f32 = 4.0;
/// The stomp when the camera's pitch is at or below -45°.
const STOMP_PITCH: f32 = -45.0;

/// A shove or stomp under way.
#[derive(Resource, Default, Debug)]
pub struct Shoving(pub Option<Move>);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub kind: ActionKind,
    /// Seconds since it started.
    pub t: f32,
    /// The hit window and the whole montage, in seconds at its play rate.
    pub window: (f32, f32),
    pub length: f32,
    /// The trace found someone (it stops at the first).
    pub hit: bool,
}

impl Move {
    /// The montage for `kind` in `stance`: (notify start, notify length, montage length), in
    /// montage time (from the montages' AnimNotify_Shove / AnimNotify_Stomp).
    fn montage(kind: ActionKind, stance: Stance) -> (f32, f32, f32) {
        match (kind, stance) {
            (ActionKind::Stomp, _) => (0.5015, 0.4002, 1.6667),
            (_, Stance::Rifle) => (0.3002, 0.4848, 1.4667),
            (_, Stance::Pistol) => (0.2846, 0.4100, 1.4667),
            _ => (0.5002, 0.3996, 1.8),
        }
    }

    pub fn new(kind: ActionKind, stance: Stance) -> Self {
        let (start, len, total) = Self::montage(kind, stance);
        let r = kind.rate();
        Self { kind, t: 0.0, window: (start / r, (start + len) / r), length: total / r, hit: false }
    }

    pub fn tracing(&self) -> bool {
        !self.hit && self.t >= self.window.0 && self.t <= self.window.1
    }
}

/// Stomp, shove or nothing, by the camera's pitch (UE degrees, down negative) and stamina.
pub fn choose(pitch: f32, stamina: f32) -> Option<ActionKind> {
    if pitch <= STOMP_PITCH {
        Some(ActionKind::Stomp)
    } else if stamina >= SHOVE_STAMINA {
        Some(ActionKind::Shove)
    } else {
        None
    }
}

/// The trace sphere's centre and radius for a survivor standing at `feet` facing `facing`.
pub fn trace_sphere(kind: ActionKind, feet: Vec3, facing: Vec3) -> (Vec3, f32) {
    match kind {
        // Foot_R, a step ahead at the ground; 50 cm.
        ActionKind::Stomp => (feet + facing * 0.35 + Vec3::Y * 0.15, 0.5),
        // Between the hands, arms out at chest height; 25 cm.
        _ => (feet + facing * 0.55 + Vec3::Y * 1.3, 0.25),
    }
}

/// Does a sphere touch an upright capsule standing at `feet`?
pub fn touches(centre: Vec3, r: f32, feet: Vec3, radius: f32, height: f32) -> bool {
    let a = feet + Vec3::Y * radius;
    let b = feet + Vec3::Y * (height - radius).max(radius);
    let ab = b - a;
    let s = ((centre - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * s).distance(centre) <= r + radius
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Shoving>().add_systems(OnEnter(AppState::InGame), |mut s: ResMut<Shoving>| s.0 = None).add_systems(
        Update,
        shove
            .after(super::combat::CombatSet)
            .run_if(in_state(AppState::InGame))
            .run_if(in_state(MenuScreen::None))
            .run_if(super::on_foot)
            .run_if(super::swim::not_swimming),
    );
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn shove(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    armed: Res<Armed>,
    mut shoving: ResMut<Shoving>,
    mut game: ResMut<CurrentGame>,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
    mut player: Query<(&Transform, &KinematicBody, &Stance, &mut UpperAction), With<CharacterModel>>,
    mut zombies: Query<(Entity, &Transform, &LevelZombie, &KinematicBody, &mut Health, Option<&mut UpperAction>), (Without<Dead>, Without<CharacterModel>)>,
    mut noise: MessageWriter<Noise>,
) {
    let Ok((me, body, stance, mut action)) = player.single_mut() else { return };
    if let Some(m) = shoving.0.as_mut() {
        m.t += time.delta_secs();
        if m.t >= m.length {
            shoving.0 = None;
        }
    }
    if keys.just_pressed(KeyCode::KeyV) && shoving.0.is_none() && body.grounded && !body.climbing && !armed.busy() {
        let pitch = camera.single().map_or(0.0, |c| c.forward().y.clamp(-1.0, 1.0).asin().to_degrees());
        let stamina = game.state.as_ref().map_or(0.0, |s| s.survival.stamina);
        if let Some(kind) = choose(pitch, stamina) {
            if kind == ActionKind::Shove
                && let Some(s) = game.state.as_mut()
            {
                s.survival.stamina -= SHOVE_STAMINA;
            }
            shoving.0 = Some(Move::new(kind, *stance));
            action.start(kind);
        }
    }
    let Some(m) = shoving.0.as_mut().filter(|m| m.tracing()) else { return };
    let facing = me.rotation * Vec3::Z;
    let (centre, r) = trace_sphere(m.kind, me.translation, facing);
    // BP_ZombieBoss's CanStompAI? is false (and it can't be shoved): the trace passes it by.
    let stomp = m.kind == ActionKind::Stomp;
    let Some((e, t, z, _, mut h, flinch)) = zombies.iter_mut().find(|(_, t, z, b, _, _)| !(stomp && z.kind.boss().is_some()) && touches(centre, r, t.translation, b.radius, b.height)) else { return };
    m.hit = true;
    if m.kind == ActionKind::Stomp {
        h.hp -= rand::thread_rng().gen_range(20.0..40.0);
        noise.write(Noise { at: me.translation, range: 5.0 });
        if let Some(mut a) = flinch {
            a.start(ActionKind::Hit);
        }
        info!("stomp hit {} at {}", z.kind.class_name(), t.translation);
    } else if z.kind.shovable() {
        commands.entity(e).insert(Shoved(SHOVED_SECS));
        if let Some(mut a) = flinch {
            a.start(ActionKind::Shoved);
        }
        info!("shoved {} at {}", z.kind.class_name(), t.translation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looking_down_stomps_else_shoves_with_stamina() {
        assert_eq!(choose(-60.0, 0.0), Some(ActionKind::Stomp));
        assert_eq!(choose(-10.0, 4.0), Some(ActionKind::Shove));
        assert_eq!(choose(-10.0, 3.9), None);
    }

    #[test]
    fn windows_follow_the_montages_and_rate() {
        // The bare-hand push: notify 0.50 + 0.40 s of a 1.8 s montage, played at 1.15.
        let m = Move::new(ActionKind::Shove, Stance::Unarmed);
        assert!((m.window.0 - 0.5002 / 1.15).abs() < 1e-4 && (m.length - 1.8 / 1.15).abs() < 1e-4);
        let rifle = Move::new(ActionKind::Shove, Stance::Rifle);
        assert!(rifle.window.0 < m.window.0);
        let stomp = Move::new(ActionKind::Stomp, Stance::Rifle);
        assert!((stomp.window.0 - 0.5015).abs() < 1e-4 && (stomp.length - 1.6667).abs() < 1e-4);
        let mut m = m;
        assert!(!m.tracing());
        m.t = 0.6;
        assert!(m.tracing());
        m.hit = true;
        assert!(!m.tracing());
    }

    #[test]
    fn shove_reaches_a_zombie_in_front_only() {
        let (c, r) = trace_sphere(ActionKind::Shove, Vec3::ZERO, Vec3::Z);
        // A 35 cm wide, 1.8 m tall zombie a metre ahead: touched; 1.5 m ahead or behind: not.
        assert!(touches(c, r, Vec3::new(0.0, 0.0, 1.0), 0.35, 1.8));
        assert!(!touches(c, r, Vec3::new(0.0, 0.0, 1.5), 0.35, 1.8));
        assert!(!touches(c, r, Vec3::new(0.0, 0.0, -1.0), 0.35, 1.8));
        // The stomp reaches a crawler on the ground in front.
        let (c, r) = trace_sphere(ActionKind::Stomp, Vec3::ZERO, Vec3::Z);
        assert!(touches(c, r, Vec3::new(0.0, 0.0, 1.0), 0.3, 0.6));
    }
}
