//! Vaulting and climbing: IA_Jump's other input event in BP_PlayerCharacter (the one that isn't
//! Event_Jump). Both fire on the press; a vault puts the character in the Flying movement mode,
//! so the jump does nothing then.
//!
//! - Not while swimming, flying (a vault already running), on a ladder, in a vehicle, seated or
//!   aiming. Without the Parkour tech not while falling either (its tooltip: "Player can parkour
//!   while falling/jumping & increased parkour speed"). Needs 5 stamina, but takes none.
//! - A 10 cm sphere from ForwardArrow (10 cm under the capsule's centre) 1 m along the facing
//!   finds the wall (actors tagged NoClimb refuse: no such tags here). A 5 cm sphere from 1.75 m
//!   over a point 10 cm into it, down to that point, finds its top; another from 2.5 m over a
//!   point 50 cm into it finds the far side ("Back Wall Height").
//! - Top more than 80 cm over the hit: **climb** (MQ_Climb_RM1), not with a broken bone, and only
//!   with 2.5 m clear over the capsule and nothing 1.2 m ahead at 2 m up. Capsule 15 x 46, moved
//!   25 cm off the wall at top - 65 cm, facing it; Delay 1 s; then, if the wall is thin, it jumps
//!   down the far side (MQ_JumpingDownFromWall_RM1, Delay 1.2 s). Both at 1.35 with Parkour.
//! - Lower: capsule 15 x 30, facing the wall. The far side within 30 cm of the top ("Wall Thick"):
//!   **get up** onto it (MQ_GettingUp_RM1) from 50 cm off the wall at the top's height; else
//!   **vault** over (MQ_Vault_RM1) where they stand, at top - 20 cm. Delay (1.35 with Parkour) ×
//!   (0.5 with a broken bone) / 0.95 or / 1.13: the Parkour factor lengthens it, as the BP has it
//!   (its montages play at 1.0 here).
//! - After the Delay: capsule 30 x 82, Walking. The montage plays on to its end; UE's root motion
//!   in Walking still drives the horizontal motion, so ours does too.
//!
//! Ours: each sphere sweep is five parallel rays (centre and four at the radius) against the
//! streamed props (not the landscape), the hit moved back by the radius; one ray slipped between
//! a picket fence's boards onto its rail. In UE the capsule sweeps along the root motion; here the
//! body only stops at floors (props and terrain), and settles onto the top when it lands.

use std::collections::HashMap;

use bevy::prelude::*;

use super::Armed;
use crate::level::KinematicBody;
use crate::level::collision::PropRays;
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{ActionKind, CharacterModel, Locomotion, UpperAction};

/// The stamina the vault checks for.
pub const STAMINA: f32 = 5.0;
/// SelectFloat(1.35, 1.0, IsTechnologyUnlocked(Skill_Parkour)).
pub const PARKOUR_RATE: f32 = 1.35;
pub const PARKOUR: &str = "Skill_Parkour";
/// CapsuleHalfHeight: the actor's location over the feet.
const HALF_HEIGHT: f32 = 0.82;
/// CharacterMesh0's RelativeLocation Z (-84 cm): the mesh's feet under the actor.
const MESH: f32 = 0.84;
/// ForwardArrow's RelativeLocation Z.
const ARROW: f32 = -0.10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Up a wall more than 80 cm over the hit; `thick`: stays on top, else jumps down beyond.
    Climb { thick: bool },
    /// Onto a low, deep ledge.
    GetUp,
    /// Over a low, thin one.
    Vault,
}

/// Where a vault starts.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub kind: Kind,
    /// The mesh's feet when the montage starts (the BP's SetActorLocation).
    pub feet: Vec3,
    /// Facing the wall.
    pub yaw: f32,
    /// The wall's top (the 5 cm sphere's centre on it).
    pub top: f32,
    /// Where the forward sphere touched the wall (its centre: 10 cm off the face).
    pub wall: Vec3,
}

/// A sphere sweep of `radius` along unit `dir`, as five parallel rays (the centre and four at the
/// radius around it): the nearest hit's distance along `dir` and its normal.
fn sweep(cast: &mut dyn FnMut(Vec3, Vec3, f32) -> Option<(f32, Vec3)>, origin: Vec3, dir: Vec3, radius: f32, max: f32) -> Option<(f32, Vec3)> {
    let side = if dir.y.abs() > 0.9 { Vec3::X } else { dir.cross(Vec3::Y).normalize() };
    let up = side.cross(dir);
    [Vec3::ZERO, side, -side, up, -up].into_iter().filter_map(|o| cast(origin + o * radius, dir, max + radius)).min_by(|a, b| a.0.total_cmp(&b.0))
}

/// The BP's traces from feet `feet` facing `forward`. `cast(origin, dir, max)` is the first
/// surface along a unit `dir` within `max`: its distance and normal, facing the ray.
pub fn plan(mut cast: impl FnMut(Vec3, Vec3, f32) -> Option<(f32, Vec3)>, feet: Vec3, forward: Vec3, broken_bone: bool) -> Option<Plan> {
    let fwd = forward.with_y(0.0).normalize_or_zero();
    if fwd == Vec3::ZERO {
        return None;
    }
    let cast: &mut dyn FnMut(Vec3, Vec3, f32) -> Option<(f32, Vec3)> = &mut cast;
    let centre = feet + Vec3::Y * HALF_HEIGHT;
    let arrow = centre + Vec3::Y * ARROW;
    // The sphere's centre where it touches: `radius` short of the surface.
    let (d, n) = sweep(cast, arrow, fwd, 0.10, 1.0)?;
    let wall = arrow + fwd * (d - 0.10).max(0.0);
    let mut down = |at: Vec3, from: f32| sweep(cast, at + Vec3::Y * from, Vec3::NEG_Y, 0.05, from).map(|(d, _)| at.y + from - d + 0.05);
    let top = down(wall - n * 0.10, 1.75)?;
    let climb = top - wall.y > 0.80;
    let thick = down(wall - n * 0.50, 2.5).is_some_and(|back| top - back < 0.30);
    if climb && (broken_bone || sweep(cast, centre, Vec3::Y, 0.05, 2.5).is_some() || cast(centre + Vec3::Y * 2.0, fwd, 1.2).is_some()) {
        return None;
    }
    let into = (-n).with_y(0.0).normalize_or_zero();
    let into = if into == Vec3::ZERO { fwd } else { into };
    let (kind, at) = match (climb, thick) {
        (true, _) => (Kind::Climb { thick }, (wall + n * 0.25).with_y(top - 0.65)),
        (false, true) => (Kind::GetUp, (wall + n * 0.50).with_y(top)),
        (false, false) => (Kind::Vault, centre.with_y(top - 0.20)),
    };
    Some(Plan { kind, feet: at - Vec3::Y * MESH, yaw: into.x.atan2(into.z), top, wall })
}

/// One montage of a vault: its action, play rate and the Delay before the next step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Phase {
    pub action: ActionKind,
    pub rate: f32,
    pub hold: f32,
}

/// The montages a vault plays, with the BP's rates and Delays.
pub fn phases(kind: Kind, parkour: bool, broken_bone: bool) -> Vec<Phase> {
    let p = if parkour { PARKOUR_RATE } else { 1.0 };
    let b = if broken_bone { 0.5 } else { 1.0 };
    match kind {
        Kind::Climb { thick } => {
            let mut v = vec![Phase { action: ActionKind::Climb, rate: p, hold: 1.0 / p }];
            if !thick {
                v.push(Phase { action: ActionKind::JumpDown, rate: p, hold: 1.2 / p });
            }
            v
        }
        Kind::GetUp => vec![Phase { action: ActionKind::GetUp, rate: 1.0, hold: p * b / 0.95 }],
        Kind::Vault => vec![Phase { action: ActionKind::VaultOver, rate: 1.0, hold: p * b / 1.13 }],
    }
}

/// A clip's root motion: fps, RateScale, offsets.
type Track = (f32, f32, Vec<Vec3>);

struct Active {
    kind: Kind,
    phases: Vec<Phase>,
    index: usize,
    /// Seconds into the current phase.
    elapsed: f32,
    /// The feet when the phase began.
    origin: Vec3,
    rotation: Quat,
    /// The root offset already applied after the hold.
    applied: Vec3,
}

/// The vault in progress (the BP's Is Climbing?, and the montage's root motion).
#[derive(Resource, Default)]
pub struct Vault {
    active: Option<Active>,
    tracks: HashMap<&'static str, Option<Track>>,
}

impl Vault {
    /// A montage is moving the survivor (input doesn't).
    pub fn active(&self) -> bool {
        self.active.is_some()
    }

    /// What's playing: the vault and its current montage.
    pub fn current(&self) -> Option<(Kind, ActionKind)> {
        self.active.as_ref().map(|a| (a.kind, a.phases[a.index].action))
    }

    fn track(&mut self, action: ActionKind) -> Option<&Track> {
        let path = action.parkour_clip()?;
        self.tracks.entry(path).or_insert_with(|| crate::models::anim::root_motion(path)).as_ref()
    }
}

/// Starting a vault, before main.rs's movement (whose jump it suppresses).
#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct VaultSet;

pub fn plugin(app: &mut App) {
    app.init_resource::<Vault>()
        .add_systems(OnEnter(AppState::InGame), |mut v: ResMut<Vault>| v.active = None)
        .add_systems(
            Update,
            (start.in_set(VaultSet).run_if(super::on_foot), root_motion.after(VaultSet).before(crate::level::collision::move_bodies))
                .run_if(in_state(AppState::InGame))
                .run_if(in_state(MenuScreen::None)),
        );
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn start(
    keys: Res<ButtonInput<KeyCode>>,
    armed: Res<Armed>,
    game: Res<CurrentGame>,
    mut vault: ResMut<Vault>,
    mut rays: PropRays,
    mut player: Query<(&mut Transform, &mut KinematicBody, &Locomotion, &mut UpperAction), With<CharacterModel>>,
) {
    if !keys.just_pressed(KeyCode::Space) || vault.active() {
        return;
    }
    let Ok((mut t, mut body, loco, mut action)) = player.single_mut() else { return };
    let Some(state) = game.state.as_ref() else { return };
    if body.swimming || body.climbing || *loco == Locomotion::Sit || armed.aiming {
        return;
    }
    let parkour = state.tech.has(PARKOUR);
    if (!parkour && !body.grounded) || state.survival.stamina < STAMINA {
        return;
    }
    let broken = state.effects.broken_bone.is_some();
    let Some(plan) = plan(|o, d, max| rays.cast_hit(o, d, max), t.translation, t.rotation * Vec3::Z, broken) else { return };
    let phases = phases(plan.kind, parkour, broken);
    if phases.iter().any(|p| vault.track(p.action).is_none()) {
        warn!("vault clips missing (extractor/export_assets.sh --anims)");
        return;
    }
    let first = phases[0];
    let speed = first.rate * vault.track(first.action).map_or(1.0, |t| t.1);
    action.start_at(first.action, Some(speed));
    t.translation = plan.feet;
    t.rotation = Quat::from_rotation_y(plan.yaw);
    body.scripted = true;
    info!("vault {:?} top {:.2} from {:.2}", plan.kind, plan.top, plan.feet);
    vault.active = Some(Active { kind: plan.kind, phases, index: 0, elapsed: 0.0, origin: plan.feet, rotation: t.rotation, applied: Vec3::ZERO });
}

/// The montages' root motion: all of it during each Delay (floors still stop the feet), then
/// only the horizontal part with the body walking again, until the last clip ends.
#[allow(clippy::type_complexity)]
fn root_motion(
    time: Res<Time>,
    mut vault: ResMut<Vault>,
    mut rays: PropRays,
    terrain: Option<Res<crate::level::Terrain>>,
    mut player: Query<(&mut Transform, &mut KinematicBody, &mut UpperAction), With<CharacterModel>>,
) {
    let Some(mut a) = vault.active.take() else { return };
    let Ok((mut t, mut body, mut action)) = player.single_mut() else { return };
    a.elapsed += time.delta_secs();
    let phase = a.phases[a.index];
    let Some((fps, scale, track)) = vault.track(phase.action).cloned() else { return };
    let speed = phase.rate * scale;
    let offset = a.rotation * crate::models::anim::sample_motion(fps, &track, a.elapsed * speed);
    if a.elapsed < phase.hold {
        let mut p = a.origin + offset;
        let floor = rays.cast(p + Vec3::Y * 0.5, Vec3::NEG_Y, 0.5).map(|d| p.y + 0.5 - d);
        let ground = terrain.as_ref().and_then(|tr| tr.ground_under(p)).into_iter().chain(floor).reduce(f32::max);
        if let Some(g) = ground {
            p.y = p.y.max(g);
        }
        t.translation = p;
        a.applied = offset;
    } else if a.index + 1 < a.phases.len() {
        // The next montage takes over the slot from where this one left the body.
        a.index += 1;
        a.elapsed = 0.0;
        a.origin = t.translation;
        a.applied = Vec3::ZERO;
        let next = a.phases[a.index];
        let scale = vault.track(next.action).map_or(1.0, |t| t.1);
        action.start_at(next.action, Some(next.rate * scale));
    } else {
        // Walking again (capsule 30 x 82); the montage's root motion still moves them sideways.
        if body.scripted {
            body.scripted = false;
            body.vertical_velocity = 0.0;
        }
        t.translation += (offset - a.applied).with_y(0.0);
        a.applied = offset;
        let clip_end = track.len().saturating_sub(1) as f32 / fps / speed;
        if a.elapsed >= clip_end {
            info!("vault done at {:.2}", t.translation);
            return;
        }
    }
    vault.active = Some(a);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ray against axis-aligned boxes (min, max): distance and the face's normal.
    fn boxes(list: &[(Vec3, Vec3)]) -> impl FnMut(Vec3, Vec3, f32) -> Option<(f32, Vec3)> + '_ {
        move |o, d, max| {
            let mut best: Option<(f32, Vec3)> = None;
            for &(lo, hi) in list {
                let (mut t0, mut t1, mut n) = (f32::MIN, f32::MAX, Vec3::ZERO);
                let mut hit = true;
                for k in 0..3 {
                    if d[k].abs() < 1e-9 {
                        hit &= o[k] >= lo[k] && o[k] <= hi[k];
                        continue;
                    }
                    let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
                    let (near, far) = (a.min(b), a.max(b));
                    if near > t0 {
                        t0 = near;
                        n = Vec3::ZERO;
                        n[k] = -d[k].signum();
                    }
                    t1 = t1.min(far);
                }
                if hit && t0 <= t1 && t0 > 0.0 && t0 <= max && best.is_none_or(|b| t0 < b.0) {
                    best = Some((t0, n));
                }
            }
            best
        }
    }

    /// A wall across +Z, its near face 0.5 m ahead of the feet at the origin, `h` tall and
    /// `depth` deep.
    fn wall(h: f32, depth: f32) -> (Vec3, Vec3) {
        (Vec3::new(-5.0, 0.0, 0.5), Vec3::new(5.0, h, 0.5 + depth))
    }

    #[test]
    fn the_wall_decides_the_move() {
        let ground = (Vec3::new(-20.0, -1.0, -20.0), Vec3::new(20.0, 0.0, 20.0));
        let go = |w: (Vec3, Vec3)| plan(boxes(&[ground, w]), Vec3::ZERO, Vec3::Z, false);
        // Below ForwardArrow (0.72 m): nothing to hit.
        assert_eq!(go(wall(0.6, 0.2)), None);
        // A 1 m fence: vault where they stand, from top - 20 cm (mesh 84 cm under the actor).
        let p = go(wall(1.0, 0.1)).expect("fence");
        assert_eq!(p.kind, Kind::Vault);
        assert!((p.top - 1.05).abs() < 1e-3, "{}", p.top);
        assert!((p.feet.y - (1.05 - 0.2 - 0.84)).abs() < 1e-3 && p.feet.z.abs() < 1e-4, "{}", p.feet);
        // A 1 m crate: get up onto it, 50 cm off the wall's hit point.
        let p = go(wall(1.0, 2.0)).expect("crate");
        assert_eq!(p.kind, Kind::GetUp);
        assert!((p.feet.z - (0.4 - 0.5)).abs() < 1e-3 && (p.feet.y - (1.05 - 0.84)).abs() < 1e-3, "{}", p.feet);
        // 1.8 m: a climb; thick stays up, thin jumps down beyond.
        assert_eq!(go(wall(1.8, 2.0)).map(|p| p.kind), Some(Kind::Climb { thick: true }));
        let p = go(wall(1.8, 0.2)).expect("thin wall");
        assert_eq!(p.kind, Kind::Climb { thick: false });
        assert!((p.feet.y - (1.85 - 0.65 - 0.84)).abs() < 1e-3 && (p.feet.z - (0.4 - 0.25)).abs() < 1e-3, "{}", p.feet);
        // Facing the wall (+Z: yaw 0).
        assert!(p.yaw.abs() < 1e-4);
        // Too tall to find the top from 1.75 m over the hit.
        assert_eq!(go(wall(3.0, 2.0)), None);
        // No headroom, or a broken bone: no climb (a vault is still fine).
        let ceiling = (Vec3::new(-5.0, 2.5, -5.0), Vec3::new(5.0, 2.6, 0.4));
        assert_eq!(plan(boxes(&[ground, wall(1.8, 2.0), ceiling]), Vec3::ZERO, Vec3::Z, false), None);
        assert_eq!(plan(boxes(&[ground, wall(1.8, 2.0)]), Vec3::ZERO, Vec3::Z, true), None);
        assert_eq!(plan(boxes(&[ground, wall(1.0, 0.1)]), Vec3::ZERO, Vec3::Z, true).map(|p| p.kind), Some(Kind::Vault));
    }

    #[test]
    fn picket_tops_count_not_the_rail_between_them() {
        let ground = (Vec3::new(-20.0, -1.0, -20.0), Vec3::new(20.0, 0.0, 20.0));
        // 1.2 m boards 8 cm wide every 14 cm, on a rail 0.9 m up behind them; the survivor
        // faces a gap between two boards.
        let mut parts = vec![ground, (Vec3::new(-5.0, 0.8, 0.52), Vec3::new(5.0, 0.9, 0.6))];
        parts.extend((-30..30).map(|i| {
            let x = i as f32 * 0.14 + 0.03;
            (Vec3::new(x, 0.0, 0.5), Vec3::new(x + 0.08, 1.2, 0.52))
        }));
        let p = plan(boxes(&parts), Vec3::ZERO, Vec3::Z, false).expect("fence");
        assert_eq!(p.kind, Kind::Vault);
        assert!((p.top - 1.25).abs() < 1e-3, "{}", p.top);
    }

    #[test]
    fn a_wall_at_an_angle_turns_them_to_face_it() {
        let ground = (Vec3::new(-20.0, -1.0, -20.0), Vec3::new(20.0, 0.0, 20.0));
        // Walking 30° off +Z at a wall facing -Z: they turn square to it.
        let p = plan(boxes(&[ground, wall(1.0, 0.1)]), Vec3::ZERO, Vec3::new(0.5, 0.0, 0.866), false).expect("fence");
        assert!(p.yaw.abs() < 1e-4, "{}", p.yaw);
    }

    #[test]
    fn delays_follow_the_blueprint() {
        assert_eq!(phases(Kind::Climb { thick: true }, false, false), vec![Phase { action: ActionKind::Climb, rate: 1.0, hold: 1.0 }]);
        let thin = phases(Kind::Climb { thick: false }, true, false);
        assert_eq!(thin.len(), 2);
        assert!((thin[0].hold - 1.0 / 1.35).abs() < 1e-5 && (thin[1].hold - 1.2 / 1.35).abs() < 1e-5 && thin[1].rate == 1.35);
        assert!((phases(Kind::Vault, false, false)[0].hold - 1.0 / 1.13).abs() < 1e-5);
        assert!((phases(Kind::GetUp, true, true)[0].hold - 1.35 * 0.5 / 0.95).abs() < 1e-5);
    }

    #[test]
    fn climb_root_motion_lifts_them_onto_the_top() {
        let Some((fps, _, track)) = crate::models::anim::root_motion("Anims/Parkour/MQ_Climb_RM1") else { return };
        // Over the 1 s Delay the root rises ~1.23 m and moves ~0.5 m on: from top - 1.49 m that
        // leaves the feet a quarter of a metre under the top, inside the step the body climbs.
        let end = crate::models::anim::sample_motion(fps, &track, 1.0);
        assert!(end.y > 1.15 && end.y < 1.3 && end.z > 0.4, "{end}");
        assert!(-0.65 - MESH + end.y > -crate::level::collision::STEP, "{}", end.y);
    }
}
