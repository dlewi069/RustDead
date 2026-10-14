//! Character animation from the game's AnimSequences (`SdExport animjson`): clips are rebuilt as
//! Bevy `AnimationClip`s targeting bones by name, and every skinned part of a character (each
//! modular body part carries its own copy of the skeleton, UE's LeaderPose setup) plays the same
//! clip in lock-step.
//!
//! The survivor follows `Player_AnimBP`: a locomotion clip per [`Stance`] (BS_Jog unarmed,
//! BS_RifleJog2D, BS_PistolJog, BS_MeleeJog; the `*_Ironsights` / melee aiming blend spaces while
//! aiming), and one-shot [`UpperAction`]s (fire, reload, melee swings) layered over the upper
//! body the way the montages play in the `UpperBody` slot. Each locomotion clip is in the graph
//! twice, a lower-body and an upper-body node, so an action can take the arms and spine while
//! the legs keep walking.

use std::collections::HashMap;

use bevy::animation::{AnimatedBy, AnimationTargetId, animated_field};
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use serde::Deserialize;

pub const ANIM_DIR: &str = "assets/game/SurrounDead/Content/Animations";

/// What a character is doing, as far as animation cares.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Locomotion {
    #[default]
    Idle,
    Walk,
    Run,
    Sprint,
    Air,
    Attack,
    Dead,
    /// Aiming (RMB): standing, or walking relative to where the survivor faces.
    AimIdle,
    AimFwd,
    AimBwd,
    AimLeft,
    AimRight,
    /// On a ladder (BP_Ladder): moving up or down, or holding still.
    Climb,
    ClimbIdle,
    /// Crouched (BP_PlayerCharacter `Crouch`): still, walking, crouch-sprinting, and aiming.
    CrouchIdle,
    CrouchWalk,
    CrouchSprint,
    CrouchAimIdle,
    CrouchAimFwd,
    CrouchAimBwd,
    CrouchAimLeft,
    CrouchAimRight,
    /// Sitting on the ground (IA_SitDown: SittingPlayer_Anim_Montage, looping).
    Sit,
}

impl Locomotion {
    pub fn crouched(self) -> bool {
        use Locomotion::*;
        matches!(self, CrouchIdle | CrouchWalk | CrouchSprint | CrouchAimIdle | CrouchAimFwd | CrouchAimBwd | CrouchAimLeft | CrouchAimRight)
    }
}

/// What the survivor holds, choosing the locomotion set (`Player_AnimBP`'s weapon states).
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Stance {
    #[default]
    Unarmed,
    Rifle,
    Pistol,
    Melee,
}

/// One-shot upper-body animations (montages).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ActionKind {
    Fire,
    Reload,
    /// Melee swing n (the weapon cycles through its attack montages).
    Swing(u8),
    /// Flinch on being hit (BP_MasterZombie `HitAnimation`).
    Hit,
    /// The survivor's shove (V): the drawn weapon's `ShoveMontage`, else the bare-hand push.
    Shove,
    /// Shove while looking down: Attack_Stomp_Montage_RM.
    Stomp,
    /// A zombie shoved back (`ShovedAnimation`: Zombie_Pushed_Montage).
    Shoved,
    /// Throwing the Throwable slot's item (G): Throw_Montage.
    Throw,
}

impl ActionKind {
    /// Svr_Montage's play rate (the shoves run at 1.15).
    pub fn rate(self) -> f32 {
        if self == ActionKind::Shove { 1.15 } else { 1.0 }
    }
}

/// Ask for an upper-body action: bump `serial` with a new `kind` to (re)start it.
#[derive(Component, Default, Clone, Copy)]
pub struct UpperAction {
    pub kind: Option<ActionKind>,
    pub serial: u32,
    /// The montage play rate when it isn't the kind's own (`ActionKind::rate`): reloads at
    /// ReloadBaseSpeed × ReloadingMultiplier, swings at SwingSpeed × StrengthSpeedMultiplier.
    pub speed: Option<f32>,
}

impl UpperAction {
    /// (Re)start `kind` at its own rate.
    pub fn start(&mut self, kind: ActionKind) {
        self.start_at(kind, None);
    }

    /// (Re)start `kind`, at `speed` if given.
    pub fn start_at(&mut self, kind: ActionKind, speed: Option<f32>) {
        self.kind = Some(kind);
        self.speed = speed.filter(|s| *s > 0.0);
        self.serial = self.serial.wrapping_add(1);
    }

    /// The playing action's rate.
    pub fn rate(&self) -> f32 {
        self.speed.or(self.kind.map(ActionKind::rate)).unwrap_or(1.0)
    }
}

/// Which clip set a character uses.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum AnimSet {
    Survivor,
    /// `AnimBP_Zombie` with the class's `SD_AIComponent.MovementBlendspace` samples.
    Zombie(crate::gameplay::zombies::ZombieKind),
    /// `BP_Zombie_Dog` (`AnimBP_ZombieDog`, PolygonDog clips).
    Hound,
}

/// Animations, keyed by stance; sets without stances use `Unarmed`.
type Key = (Stance, Locomotion);

impl AnimSet {
    /// ((stance, state), clip path under Content/Animations, loops)
    fn clips(self) -> Vec<(Key, &'static str, bool)> {
        use Locomotion::*;
        use Stance::*;
        match self {
            AnimSet::Survivor => vec![
                // BS_Jog (+ jump and death).
                ((Unarmed, Idle), "Anims/No_Rifle/ThirdPersonIdle", true),
                ((Unarmed, Walk), "Anims/No_Rifle/ThirdPersonWalk", true),
                ((Unarmed, Run), "Anims/No_Rifle/ThirdPersonRun", true),
                ((Unarmed, Sprint), "Anims/No_Rifle/ThirdPersonSprint", true),
                ((Unarmed, Air), "Anims/No_Rifle/A_JumpAir", true),
                ((Unarmed, Dead), "Player/Anims/Random/Anim_Regular_death1", false),
                // Player_AnimBP's Ladder state: the up clip above 1 cm/s (also going down), else idle.
                ((Unarmed, Climb), "Anims/Other/Anim_Regular_ladder_up-inplace", true),
                ((Unarmed, ClimbIdle), "Anims/Other/Anim_Regular_ladder_idle", true),
                // SittingPlayer_Anim_Montage: its one section loops on itself (every stance).
                ((Unarmed, Sit), "Player/Anims/Actions/SittingPlayer_Anim", true),
                // BS_Crouch2D: the walk at every speed up to the crouch sprint's 420 cm/s.
                ((Unarmed, CrouchIdle), "Anims/No_Rifle/A_Crouch_Idle", true),
                ((Unarmed, CrouchWalk), "Anims/No_Rifle/A_Crouch_Walk", true),
                ((Unarmed, CrouchSprint), "Anims/No_Rifle/A_Crouch_Walk", true),
                // AR_CrouchMovement_2D, BS_Crouch_Rifle_Ironsights.
                ((Rifle, CrouchIdle), "Anims/Rifle_Crouch/A_AR_Crouch_Idle", true),
                ((Rifle, CrouchWalk), "Anims/Rifle_Crouch/A_AR_Crouch_Walk", true),
                ((Rifle, CrouchSprint), "Anims/Rifle_Crouch/A_AR_Crouch_Walk", true),
                ((Rifle, CrouchAimIdle), "Anims/Rifle_Crouch/Crouch_Idle_Rifle_Ironsights", true),
                ((Rifle, CrouchAimFwd), "Anims/Rifle_Crouch/Crouch_Walk_Fwd_Rifle_Ironsights", true),
                ((Rifle, CrouchAimBwd), "Anims/Rifle_Crouch/Crouch_Walk_BwdRifle_Ironsights", true),
                ((Rifle, CrouchAimLeft), "Anims/Rifle_Crouch/Crouch_Walk_Lt_Rifle_Ironsights", true),
                ((Rifle, CrouchAimRight), "Anims/Rifle_Crouch/Crouch_Walk_Rt_Rifle_Ironsights", true),
                // BS_PistolCrouch, BS_Crouch_Pistol_Ironsights.
                ((Pistol, CrouchIdle), "Anims/Pistol_Crouch/A_HG_Crouch_Idle", true),
                ((Pistol, CrouchWalk), "Anims/Pistol_Crouch/A_HG_Crouch_Walk", true),
                ((Pistol, CrouchSprint), "Anims/Pistol_Crouch/A_HG_Crouch_Walk", true),
                ((Pistol, CrouchAimIdle), "Anims/Pistol_Crouch/W1_Crouch_Aim_Idle_IPC", true),
                ((Pistol, CrouchAimFwd), "Anims/Pistol_Crouch/W1_CrouchWalk_Aim_F_Loop_IPC", true),
                ((Pistol, CrouchAimBwd), "Anims/Pistol_Crouch/W1_CrouchWalk_Aim_B_Loop_IPC", true),
                ((Pistol, CrouchAimLeft), "Anims/Pistol_Crouch/W1_CrouchWalk_Aim_L_Loop_IPC", true),
                ((Pistol, CrouchAimRight), "Anims/Pistol_Crouch/W1_CrouchWalk_Aim_R_Loop_IPC", true),
                // BS_RifleJog2D, BS_Rifle_Ironsights.
                ((Rifle, Idle), "Anims/Rifle_Standing/Rifle_Idle", true),
                ((Rifle, Walk), "Anims/Rifle_Standing/A_AR_Walk", true),
                ((Rifle, Run), "Anims/Rifle_Standing/A_AR_Run", true),
                ((Rifle, Sprint), "Anims/Rifle_Standing/A_AR_Run", true),
                ((Rifle, AimIdle), "Anims/Rifle_Standing/Idle_Rifle_Ironsights", true),
                ((Rifle, AimFwd), "Anims/Rifle_Standing/Walk_Fwd_Rifle_Ironsights", true),
                ((Rifle, AimBwd), "Anims/Rifle_Standing/Walk_Bwd_Rifle_Ironsights", true),
                ((Rifle, AimLeft), "Anims/Rifle_Standing/Walk_Lt_Rifle_Ironsights", true),
                ((Rifle, AimRight), "Anims/Rifle_Standing/Walk_Rt_Rifle_Ironsights", true),
                // BS_PistolJog, BS_Pistol_Ironsights.
                ((Pistol, Idle), "Anims/Pistol_Standing/A_HG_Idle", true),
                ((Pistol, Walk), "Anims/Pistol_Standing/A_HG_Walk", true),
                ((Pistol, Run), "Anims/Pistol_Standing/A_HG_Run", true),
                ((Pistol, Sprint), "Anims/Pistol_Standing/A_HG_Run", true),
                ((Pistol, AimIdle), "Anims/Pistol_Standing/A_HG_Aim_Stand", true),
                ((Pistol, AimFwd), "Anims/Pistol_Standing/A_AimWalk_0", true),
                ((Pistol, AimBwd), "Anims/Pistol_Standing/A_AimWalk_180", true),
                ((Pistol, AimLeft), "Anims/Pistol_Standing/A_AimWalk_-90", true),
                ((Pistol, AimRight), "Anims/Pistol_Standing/A_AimWalk_90", true),
                // BS_MeleeJog (run/sprint are the unarmed ones), BS_Melee_Aiming.
                ((Melee, Idle), "Anims/No_Rifle/ThirdPersonIdleMelee", true),
                ((Melee, AimIdle), "Anims/No_Rifle/ThirdPersonIdleMelee", true),
                ((Melee, AimFwd), "Player/1Handed/Walk_F_Seq", true),
                ((Melee, AimBwd), "Player/1Handed/Walk_B_Seq", true),
                ((Melee, AimLeft), "Player/1Handed/Walk_L_Seq", true),
                ((Melee, AimRight), "Player/1Handed/Walk_R_Seq", true),
                // BS_MeleeCrouch (by direction; unaimed walking faces the way it goes).
                ((Melee, CrouchIdle), "Anims/MeleeCrouch/Crouch_Idle_Axe", true),
                ((Melee, CrouchWalk), "Anims/MeleeCrouch/Crouch_Fwd_Axe", true),
                ((Melee, CrouchSprint), "Anims/MeleeCrouch/Crouch_Fwd_Axe", true),
                ((Melee, CrouchAimIdle), "Anims/MeleeCrouch/Crouch_Idle_Axe", true),
                ((Melee, CrouchAimFwd), "Anims/MeleeCrouch/Crouch_Fwd_Axe", true),
                ((Melee, CrouchAimBwd), "Anims/MeleeCrouch/Crouch_Bwd_Axe", true),
                ((Melee, CrouchAimLeft), "Anims/MeleeCrouch/Crouch_Lt_Axe", true),
                ((Melee, CrouchAimRight), "Anims/MeleeCrouch/Crouch_Rt_Axe", true),
            ],
            AnimSet::Zombie(kind) => vec![
                ((Unarmed, Idle), zombie_clip(kind, 0.0), true),
                ((Unarmed, Walk), zombie_clip(kind, kind.roam_speed()), true),
                ((Unarmed, Run), zombie_clip(kind, kind.alert_speed()), true),
                // BP_MasterZombie AttackMontage (every child, crawlers included).
                ((Unarmed, Attack), "Zombie/Regular/Attacks/Anim_Zombie_attack1", true),
                // A two-frame held pose: repeated, or it ends inside the blend and the corpse
                // drops back to the bind T-pose.
                ((Unarmed, Dead), "Zombie/Regular/Other/Other/Zombie_DeathPosition", true),
            ],
            // ZombieDogAILocomotion (0 / 100 / 600 cm/s), AS_Dog_Attack_Bite_Montage, death.
            AnimSet::Hound => vec![
                ((Unarmed, Idle), "../PolygonFiles/PolygonDog/Animation/Locomotion/AS_Dog_Locomotion_Standing", true),
                ((Unarmed, Walk), "../PolygonFiles/PolygonDog/Animation/Locomotion/AS_Dog_Locomotion_Walking", true),
                ((Unarmed, Run), "../PolygonFiles/PolygonDog/Animation/Locomotion/AS_Dog_Locomotion_Running", true),
                ((Unarmed, Attack), "../PolygonFiles/PolygonDog/Animation/Attack/AS_Dog_Attack_Bite", true),
                ((Unarmed, Dead), "../PolygonFiles/PolygonDog/Animation/Death/AS_Dog_Death_Standing", false),
            ],
        }
    }

    /// Upper-body montages' sequences per stance.
    fn actions(self) -> Vec<((Stance, ActionKind), &'static str)> {
        use ActionKind::*;
        use Stance::*;
        match self {
            AnimSet::Survivor => vec![
                ((Rifle, Fire), "Anims/Other/Fire_Rifle_Ironsights"),
                ((Rifle, Reload), "Anims/Other/Reload_Rifle_Hip"),
                ((Pistol, Reload), "Anims/Other/Reload_Pistol"),
                // 1HMeleeAttack1-4_Montage.
                ((Melee, Swing(0)), "Player/1Handed/Attack_01_Seq"),
                ((Melee, Swing(1)), "Player/1Handed/Attack_02_Seq"),
                ((Melee, Swing(2)), "Player/1Handed/Attack_04_Seq"),
                ((Melee, Swing(3)), "Player/1Handed/Attack_06_Seq"),
                // Each pickup's BP_WeaponsPickupComponent.ShoveMontage: rifles keep the
                // component's RiflePush, pistols PistolPush, melee weapons (and no weapon)
                // SmallMeleePush. (One melee weapon uses LargeMeleePush; not told apart here.)
                ((Unarmed, Shove), "Player/Anims/Actions/Push/Anim_SmallMeleePush"),
                ((Melee, Shove), "Player/Anims/Actions/Push/Anim_SmallMeleePush"),
                ((Rifle, Shove), "Player/Anims/Actions/Push/Anim_RiflePush"),
                ((Pistol, Shove), "Player/Anims/Actions/Push/Anim_PistolPush"),
                // Attack_Stomp_Montage_RM's sequence.
                ((Unarmed, Stomp), "Player/1Handed/Attack_06_Seq"),
                ((Melee, Stomp), "Player/1Handed/Attack_06_Seq"),
                ((Rifle, Stomp), "Player/1Handed/Attack_06_Seq"),
                ((Pistol, Stomp), "Player/1Handed/Attack_06_Seq"),
                ((Unarmed, Throw), "Player/Anims/Actions/Throw"),
                ((Melee, Throw), "Player/Anims/Actions/Throw"),
                ((Rifle, Throw), "Player/Anims/Actions/Throw"),
                ((Pistol, Throw), "Player/Anims/Actions/Throw"),
            ],
            AnimSet::Zombie(_) => vec![((Unarmed, Hit), "Zombie/Regular/Other/Other/Anim_Zombie_hit"), ((Unarmed, Shoved), "Zombie/ZombieAnims/Zombie_HitReact_Head")],
            // HitAnimation: AS_Dog_Attack_Hit_Montage.
            AnimSet::Hound => vec![((Unarmed, Hit), "../PolygonFiles/PolygonDog/Animation/Attack/AS_Dog_Attack_Hit")],
        }
    }
}

/// The blendspace sample nearest `speed` (the blend between samples isn't reproduced), as a path
/// under Content/Animations. Classes without samples use the Roamer's ZombieAILocomotion clips.
fn zombie_clip(kind: crate::gameplay::zombies::ZombieKind, speed: f32) -> &'static str {
    let samples = &kind.class().locomotion;
    let fallback = if speed <= 0.0 { "Zombie/Regular/A_Enemy_Zombie_Idle" } else if speed < 2.0 { "Zombie/Regular/Locomotion/Anim_Zombie_walk1_inplace" } else { "Zombie/Regular/Locomotion/Anim_Zombie_run1_inplace" };
    samples
        .iter()
        .min_by(|a, b| (a.1 - speed).abs().total_cmp(&(b.1 - speed).abs()))
        .and_then(|(clip, _)| clip.strip_prefix("Animations/"))
        .unwrap_or(fallback)
}

/// Every clip path an [`AnimSet`] uses (for the exporter list and tests).
pub fn clip_paths(set: AnimSet) -> Vec<&'static str> {
    set.clips().into_iter().map(|c| c.1).chain(set.actions().into_iter().map(|a| a.1)).collect()
}

#[derive(Deserialize)]
struct BoneKeys {
    t: Vec<[f32; 3]>,
    r: Vec<[f32; 4]>,
    s: Vec<[f32; 3]>,
}

#[derive(Deserialize)]
struct Notify {
    name: String,
    time: f32,
}

#[derive(Deserialize)]
struct AnimFile {
    fps: f32,
    frames: usize,
    duration: f32,
    /// The sequence's RateScale.
    #[serde(default = "one")]
    rate: f32,
    #[serde(default)]
    notifies: Vec<Notify>,
    bones: HashMap<String, BoneKeys>,
}

fn one() -> f32 {
    1.0
}

/// A loaded clip's timing: length, RateScale and its `Footstep` notify times.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipTiming {
    pub duration: f32,
    pub rate: f32,
    pub footsteps: Vec<f32>,
}

/// A clip sampled by hand onto named bones, for props whose pose follows game state rather than
/// an animation graph (the police garage gate's montage, played forwards and backwards).
pub struct PoseTrack {
    fps: f32,
    pub duration: f32,
    bones: Vec<(String, BoneKeys)>,
}

impl PoseTrack {
    /// `path` is the `.anim.json` file.
    pub fn load(path: &str) -> Option<Self> {
        let file: AnimFile = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        // Tracks that hold still (within float noise) count as constant.
        fn settle<const N: usize>(k: &mut Vec<[f32; N]>) {
            if k.iter().all(|v| v.iter().zip(&k[0]).all(|(a, b)| (a - b).abs() < 1e-4)) {
                k.truncate(1);
            }
        }
        let mut bones: Vec<(String, BoneKeys)> = file.bones.into_iter().collect();
        for (_, k) in &mut bones {
            settle(&mut k.t);
            // A rotation and its negation are the same turn.
            if k.r.iter().all(|q| Quat::from_array(*q).normalize().dot(Quat::from_array(k.r[0]).normalize()).abs() > 1.0 - 1e-6) {
                k.r.truncate(1);
            }
            settle(&mut k.s);
        }
        Some(Self { fps: file.fps, duration: file.duration, bones })
    }

    /// The animated parts of each bone's local transform `t` seconds in (clamped to the clip):
    /// translation / rotation / scale, each `None` when the clip holds it constant. Constant
    /// tracks are the UE reference pose in UE's units (the gate's Root carries a scale of 100);
    /// the exported mesh already has its own rest pose for them.
    pub fn sample(&self, t: f32) -> impl Iterator<Item = (&str, BonePose)> {
        let f = (t.clamp(0.0, self.duration) * self.fps).max(0.0);
        let (i, a) = (f.floor() as usize, f.fract());
        let lerp3 = move |k: &[[f32; 3]]| match k.len() {
            0 | 1 => None,
            n => Some(Vec3::from_array(k[i.min(n - 1)]).lerp(Vec3::from_array(k[(i + 1).min(n - 1)]), a)),
        };
        self.bones.iter().map(move |(name, k)| {
            let rotation = match k.r.len() {
                0 | 1 => None,
                n => Some(Quat::from_array(k.r[i.min(n - 1)]).normalize().slerp(Quat::from_array(k.r[(i + 1).min(n - 1)]).normalize(), a)),
            };
            (name.as_str(), BonePose { translation: lerp3(&k.t), rotation, scale: lerp3(&k.s) })
        })
    }
}

/// A sampled bone: the components the clip animates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BonePose {
    pub translation: Option<Vec3>,
    pub rotation: Option<Quat>,
    pub scale: Option<Vec3>,
}

impl BonePose {
    /// Apply the animated components onto `t`.
    pub fn apply(&self, t: &mut Transform) {
        if let Some(v) = self.translation {
            t.translation = v;
        }
        if let Some(q) = self.rotation {
            t.rotation = q;
        }
        if let Some(v) = self.scale {
            t.scale = v;
        }
    }
}

/// A clip, its timing and the bones it keys.
fn load_clip(path: &str) -> Option<(AnimationClip, ClipTiming, Vec<String>)> {
    let file: AnimFile = serde_json::from_str(&std::fs::read_to_string(format!("{ANIM_DIR}/{path}.anim.json")).ok()?).ok()?;
    let mut clip = AnimationClip::default();
    let times = |n: usize| -> Vec<f32> { (0..n).map(|i| if n > 1 { i as f32 / file.fps } else { 0.0 }).collect() };
    for (bone, keys) in &file.bones {
        let target = AnimationTargetId::from_name(&Name::new(bone.clone()));
        // A single key still needs two points for a curve; hold it for the whole clip.
        let span = |n: usize| if n == 1 { vec![0.0, file.duration.max(1.0 / file.fps)] } else { times(n) };
        let dup = |n: usize| if n == 1 { 2 } else { n };
        let t: Vec<Vec3> = keys.t.iter().map(|v| Vec3::from_array(*v)).cycle().take(dup(keys.t.len())).collect();
        let r: Vec<Quat> = keys.r.iter().map(|v| Quat::from_array(*v).normalize()).cycle().take(dup(keys.r.len())).collect();
        let s: Vec<Vec3> = keys.s.iter().map(|v| Vec3::from_array(*v)).cycle().take(dup(keys.s.len())).collect();
        if let Ok(c) = AnimatableKeyframeCurve::new(span(keys.t.len()).into_iter().zip(t)) {
            clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::translation), c));
        }
        if let Ok(c) = AnimatableKeyframeCurve::new(span(keys.r.len()).into_iter().zip(r)) {
            clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::rotation), c));
        }
        if let Ok(c) = AnimatableKeyframeCurve::new(span(keys.s.len()).into_iter().zip(s)) {
            clip.add_curve_to_target(target, AnimatableCurve::new(animated_field!(Transform::scale), c));
        }
    }
    let duration = file.duration.max(1.0 / file.fps);
    clip.set_duration(duration);
    let _ = file.frames;
    let footsteps = file.notifies.iter().filter(|n| n.name == "Footstep").map(|n| n.time).collect();
    Some((clip, ClipTiming { duration, rate: file.rate, footsteps }, file.bones.into_keys().collect()))
}

/// Mask groups: the upper body (spine up, arms, head) and everything else.
const UPPER: u32 = 0;
const LOWER: u32 = 1;

fn is_upper(bone: &str) -> bool {
    let b = bone.to_ascii_lowercase();
    ["spine", "neck", "head", "clavicle", "upperarm", "lowerarm", "hand", "finger", "thumb", "index", "eye", "jaw", "ik_hand"].iter().any(|k| b.contains(k))
}

struct LocoNodes {
    /// Whole body for sets without actions, else the lower body.
    lower: AnimationNodeIndex,
    /// The same clip on the upper body only (survivor), faded out under an action.
    upper: Option<AnimationNodeIndex>,
    looped: bool,
    timing: ClipTiming,
}

struct SetGraph {
    graph: Handle<AnimationGraph>,
    nodes: HashMap<Key, LocoNodes>,
    actions: HashMap<(Stance, ActionKind), (AnimationNodeIndex, f32)>,
}

impl SetGraph {
    /// The clip for a state, falling back to the unarmed one, then the stance's idle.
    fn find(&self, stance: Stance, state: Locomotion) -> Option<(Key, &LocoNodes)> {
        [(stance, state), (Stance::Unarmed, state), (stance, Locomotion::Idle), (Stance::Unarmed, Locomotion::Idle)]
            .into_iter()
            .find_map(|k| self.nodes.get(&k).map(|n| (k, n)))
    }
}

/// Clip graphs per [`AnimSet`], built on first use.
#[derive(Resource, Default)]
pub struct AnimLibrary {
    sets: HashMap<AnimSet, SetGraph>,
}

impl AnimLibrary {
    fn get(&mut self, set: AnimSet, clips: &mut Assets<AnimationClip>, graphs: &mut Assets<AnimationGraph>) -> &SetGraph {
        self.sets.entry(set).or_insert_with(|| {
            let mut graph = AnimationGraph::new();
            let layered = !set.actions().is_empty();
            let mut nodes = HashMap::new();
            let mut actions = HashMap::new();
            let mut loaded: HashMap<&str, (Handle<AnimationClip>, ClipTiming)> = HashMap::new();
            let mut bones = std::collections::HashSet::new();
            let mut load = |path: &'static str, clips: &mut Assets<AnimationClip>| {
                if let Some(c) = loaded.get(path) {
                    return Some(c.clone());
                }
                let (clip, timing, keyed) = load_clip(path).or_else(|| {
                    warn!("animation {path} missing (extractor/export_assets.sh --anims)");
                    None
                })?;
                bones.extend(keyed);
                let h = (clips.add(clip), timing);
                loaded.insert(path, h.clone());
                Some(h)
            };
            // Node weights stay 1: Bevy multiplies them into the player's per-animation weights
            // (which `drive_animation` fades), and two overlapping clips with zero effective
            // weight blend as 0/0 = NaN, which made characters vanish during every crossfade.
            for (key, path, looped) in set.clips() {
                let Some((clip, timing)) = load(path, clips) else { continue };
                let lower = if layered { graph.add_clip_with_mask(clip.clone(), 1 << UPPER, 1.0, graph.root) } else { graph.add_clip(clip.clone(), 1.0, graph.root) };
                let upper = layered.then(|| graph.add_clip_with_mask(clip, 1 << LOWER, 1.0, graph.root));
                nodes.insert(key, LocoNodes { lower, upper, looped, timing });
            }
            for (key, path) in set.actions() {
                let Some((clip, timing)) = load(path, clips) else { continue };
                actions.insert(key, (graph.add_clip_with_mask(clip, 1 << LOWER, 1.0, graph.root), timing.duration));
            }
            if layered {
                for b in bones {
                    graph.add_target_to_mask_group(AnimationTargetId::from_name(&Name::new(b.clone())), if is_upper(&b) { UPPER } else { LOWER });
                }
            }
            SetGraph { graph: graphs.add(graph), nodes, actions }
        })
    }

    /// The locomotion clip a state plays (after the stance fallbacks) and its timing, once the
    /// set has been built.
    pub fn timing(&self, set: AnimSet, stance: Stance, state: Locomotion) -> Option<(&'static str, &ClipTiming)> {
        let (key, n) = self.sets.get(&set)?.find(stance, state)?;
        let path = set.clips().into_iter().find(|c| c.0 == key)?.1;
        Some((path, &n.timing))
    }

    /// How long an action's clip runs (e.g. the reload), once the set has been built.
    pub fn action_secs(&self, set: AnimSet, stance: Stance, kind: ActionKind) -> Option<f32> {
        self.sets.get(&set)?.actions.get(&(stance, kind)).map(|a| a.1 / kind.rate())
    }
}

/// On a character root: animate the skinned parts under it with this set.
#[derive(Component)]
pub struct Animated(pub AnimSet);

/// A part whose skeleton is wired to its own AnimationPlayer: the locomotion clips fading in
/// or out (weight 0..1) and the action playing over the upper body.
#[derive(Component, Default)]
pub struct PartPlayer {
    current: Option<Key>,
    fading: Vec<(Key, f32)>,
    action: Option<PlayingAction>,
    action_serial: u32,
}

#[derive(Clone, Copy)]
struct PlayingAction {
    node: AnimationNodeIndex,
    elapsed: f32,
    duration: f32,
}

/// Where a character's parts are in their clips, copied onto a part that joins later (a new
/// piece of clothing or body part) so it moves in step with the rest: (node, seek time) pairs.
struct Snapshot {
    current: Option<Key>,
    fading: Vec<(Key, f32)>,
    action: Option<(PlayingAction, f32)>,
    action_serial: u32,
    seeks: Vec<(AnimationNodeIndex, f32)>,
}

impl Snapshot {
    fn of(pp: &PartPlayer, player: &AnimationPlayer, g: &SetGraph) -> Self {
        let nodes = pp.fading.iter().flat_map(|(k, _)| {
            let n = &g.nodes[k];
            std::iter::once(n.lower).chain(n.upper)
        });
        let seek = |node| player.animation(node).map(|a| (node, a.seek_time()));
        Self {
            current: pp.current,
            fading: pp.fading.clone(),
            action: pp.action.and_then(|a| seek(a.node).map(|(_, t)| (a, t))),
            action_serial: pp.action_serial,
            seeks: nodes.filter_map(seek).collect(),
        }
    }

    /// Put a fresh part where the others are.
    fn apply(&self, pp: &mut PartPlayer, player: &mut AnimationPlayer, g: &SetGraph, rate: f32) {
        pp.current = self.current;
        pp.fading = self.fading.clone();
        pp.action_serial = self.action_serial;
        for (key, _) in &self.fading {
            let n = &g.nodes[key];
            for node in std::iter::once(n.lower).chain(n.upper) {
                let a = player.start(node);
                a.set_speed(n.timing.rate);
                if n.looped {
                    a.repeat();
                }
                if let Some(&(_, t)) = self.seeks.iter().find(|s| s.0 == node) {
                    a.seek_to(t);
                }
            }
        }
        pp.action = self.action.map(|(a, t)| {
            player.start(a.node).set_speed(rate).seek_to(t);
            a
        });
    }
}

/// Crossfade time between locomotion clips, and the action blend in/out.
const FADE: f32 = 0.18;
const ACTION_IN: f32 = 0.1;
const ACTION_OUT: f32 = 0.2;

/// The glTF exporter de-duplicates node names, so most body parts call their root bone
/// `Root_001` (the Head part keeps `Root`). Clips key the skeleton's real names.
fn bone_name(node: &str) -> &str {
    match node.rsplit_once('_') {
        Some((base, n)) if n.len() == 3 && n.chars().all(|c| c.is_ascii_digit()) => base,
        _ => node,
    }
}

/// What wiring a part to its clips needs.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Wiring<'w, 's> {
    commands: Commands<'w, 's>,
    children: Query<'w, 's, &'static Children>,
    names: Query<'w, 's, &'static Name>,
    skinned: Query<'w, 's, (), With<bevy::mesh::skinning::SkinnedMesh>>,
    wired: Query<'w, 's, (), With<PartPlayer>>,
    lib: ResMut<'w, AnimLibrary>,
    clips: ResMut<'w, Assets<AnimationClip>>,
    graphs: ResMut<'w, Assets<AnimationGraph>>,
}

impl Wiring<'_, '_> {
    /// Tag a loaded part's bones as animation targets and give it a player with the set's
    /// graph. False if its scene has no skinned mesh (yet).
    fn wire(&mut self, part: Entity, set: AnimSet) -> bool {
        if self.wired.contains(part) {
            return true;
        }
        if !self.children.iter_descendants(part).any(|e| self.skinned.contains(e)) {
            return false; // static attachment (hair, beard) or not spawned yet
        }
        for e in self.children.iter_descendants(part) {
            if let Ok(n) = self.names.get(e) {
                let bone = Name::new(bone_name(n.as_str()).to_string());
                self.commands.entity(e).insert((AnimationTargetId::from_name(&bone), AnimatedBy(part)));
            }
        }
        let g = self.lib.get(set, &mut self.clips, &mut self.graphs);
        self.commands.entity(part).insert((AnimationPlayer::default(), AnimationGraphHandle(g.graph.clone()), PartPlayer::default()));
        true
    }
}

/// When a skinned part's scene is ready, wire it to its character's clips.
pub fn wire_skeleton(ev: On<WorldInstanceReady>, parents: Query<&ChildOf>, animated: Query<&Animated>, mut wiring: Wiring) {
    let part = ev.entity;
    // The character root is the part itself (zombies) or its parent (survivor parts).
    let set = animated.get(part).or_else(|_| parents.get(part).and_then(|p| animated.get(p.parent())));
    if let Ok(Animated(set)) = set {
        wiring.wire(part, *set);
    }
}

/// Characters that became `Animated` after their scene was already in place (a cached model
/// spawns its scene before the next frame's systems tag it): wire them now.
pub fn wire_late(added: Query<(Entity, &Animated, Option<&Children>), Added<Animated>>, mut wiring: Wiring) {
    for (root, Animated(set), kids) in &added {
        let parts: Vec<Entity> = std::iter::once(root).chain(kids.into_iter().flat_map(|c| c.iter())).collect();
        for p in parts {
            wiring.wire(p, *set);
        }
    }
}

/// How far apart (seconds) a character's parts are in their current locomotion clip; 0 when
/// they move in step. None until two parts play.
pub fn phase_spread<'a>(lib: &AnimLibrary, set: AnimSet, parts: impl Iterator<Item = (&'a PartPlayer, &'a AnimationPlayer)>) -> Option<f32> {
    let g = lib.sets.get(&set)?;
    let times: Vec<f32> = parts.filter_map(|(pp, player)| player.animation(g.nodes.get(&pp.current?)?.lower).map(|a| a.seek_time())).collect();
    (times.len() >= 2).then(|| times.iter().copied().fold(f32::MIN, f32::max) - times.iter().copied().fold(f32::MAX, f32::min))
}

/// Play each character's [`Locomotion`] (in its [`Stance`]) and [`UpperAction`] on all its parts.
#[allow(clippy::type_complexity)]
pub fn drive_animation(
    time: Res<Time>,
    roots: Query<(Entity, &Animated, &Locomotion, Option<&Stance>, Option<&UpperAction>, Option<&Children>)>,
    mut parts: Query<(&mut PartPlayer, &mut AnimationPlayer)>,
    mut lib: ResMut<AnimLibrary>,
    mut clips: ResMut<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let dt = time.delta_secs();
    for (root, Animated(set), state, stance, action, kids) in &roots {
        let g = lib.get(*set, &mut clips, &mut graphs);
        let stance = stance.copied().unwrap_or_default();
        let Some((wanted, _)) = g.find(stance, *state) else { continue };
        let targets: Vec<Entity> = std::iter::once(root).chain(kids.into_iter().flat_map(|c| c.iter())).collect();
        // A part that's already playing, for any that just joined to copy.
        let fresh = |pp: &PartPlayer| pp.current.is_none();
        let leader = targets.iter().find_map(|&e| parts.get(e).ok().filter(|(pp, _)| !fresh(pp)).map(|(pp, player)| Snapshot::of(pp, player, g)));
        for &e in &targets {
            let Ok((mut pp, mut player)) = parts.get_mut(e) else { continue };
            let pp = &mut *pp;
            if fresh(pp)
                && let Some(snap) = &leader
            {
                snap.apply(pp, &mut player, g, action.map_or(1.0, |a| a.rate()));
            }
            // A new locomotion clip starts from its beginning and fades in; the rest fade out.
            if pp.current != Some(wanted) {
                pp.current = Some(wanted);
                if !pp.fading.iter().any(|f| f.0 == wanted) {
                    pp.fading.push((wanted, 0.0));
                    let n = &g.nodes[&wanted];
                    for node in std::iter::once(n.lower).chain(n.upper) {
                        let a = player.start(node);
                        a.set_speed(n.timing.rate);
                        if n.looped {
                            a.repeat();
                        }
                    }
                }
            }
            // New action request: (re)start its clip over the upper body.
            if let Some(req) = action
                && req.serial != pp.action_serial
            {
                pp.action_serial = req.serial;
                if let Some(old) = pp.action.take() {
                    player.stop(old.node);
                }
                if let Some(kind) = req.kind
                    && let Some(&(node, duration)) = g.actions.get(&(stance, kind))
                {
                    let rate = req.rate();
                    player.start(node).set_speed(rate);
                    pp.action = Some(PlayingAction { node, elapsed: 0.0, duration: duration / rate });
                }
            }
            let mut over = 0.0;
            if let Some(a) = pp.action.as_mut() {
                a.elapsed += dt;
                over = (a.elapsed / ACTION_IN).min(((a.duration - a.elapsed) / ACTION_OUT).max(0.0)).clamp(0.0, 1.0);
                if a.elapsed >= a.duration {
                    player.stop(a.node);
                    pp.action = None;
                    over = 0.0;
                } else if let Some(p) = player.animation_mut(a.node) {
                    p.set_weight(over.max(1e-3));
                }
            }
            for (key, w) in pp.fading.iter_mut() {
                let target = if Some(*key) == pp.current { 1.0 } else { 0.0 };
                *w = if target > *w { (*w + dt / FADE).min(1.0) } else { (*w - dt / FADE).max(0.0) };
                let n = &g.nodes[key];
                if let Some(p) = player.animation_mut(n.lower) {
                    p.set_weight(*w);
                }
                if let Some(p) = n.upper.and_then(|u| player.animation_mut(u)) {
                    p.set_weight(*w * (1.0 - over));
                }
            }
            pp.fading.retain(|(key, w)| {
                let keep = *w > 0.0 || Some(*key) == pp.current;
                if !keep {
                    let n = &g.nodes[key];
                    player.stop(n.lower);
                    if let Some(u) = n.upper {
                        player.stop(u);
                    }
                }
                keep
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_clip_is_exported_and_masks_split_the_body() {
        if !std::path::Path::new(ANIM_DIR).exists() {
            return;
        }
        let zombies = crate::gameplay::zombies::zombie_classes().iter().map(|(n, _)| AnimSet::Zombie(crate::gameplay::zombies::ZombieKind::from_class(n).unwrap()));
        for set in [AnimSet::Survivor, AnimSet::Hound].into_iter().chain(zombies) {
            for path in clip_paths(set) {
                assert!(load_clip(path).is_some(), "{path} not exported");
            }
        }
        assert!(is_upper("spine_01") && is_upper("Hand_R") && is_upper("lowerarm_l") && is_upper("head"));
        assert!(!is_upper("Pelvis") && !is_upper("Thigh_L") && !is_upper("Root") && !is_upper("ik_foot_l"));
    }

    #[test]
    fn parts_that_join_late_move_in_step() {
        if !std::path::Path::new(ANIM_DIR).exists() {
            return;
        }
        let (mut lib, mut clips, mut graphs) = (AnimLibrary::default(), Assets::<AnimationClip>::default(), Assets::<AnimationGraph>::default());
        let g = lib.get(AnimSet::Survivor, &mut clips, &mut graphs);
        let (key, n) = g.find(Stance::Unarmed, Locomotion::Idle).unwrap();
        // The body: 0.7 s into idle, its last action (serial 3) long finished.
        let mut body = (PartPlayer { current: Some(key), fading: vec![(key, 1.0)], action: None, action_serial: 3 }, AnimationPlayer::default());
        for node in std::iter::once(n.lower).chain(n.upper) {
            body.1.start(node).repeat().seek_to(0.7);
        }
        let snap = Snapshot::of(&body.0, &body.1, g);
        // New gloves (a hands part) arrive: they join at 0.7 s and don't replay the action.
        let (mut pp, mut player) = (PartPlayer::default(), AnimationPlayer::default());
        snap.apply(&mut pp, &mut player, g, 1.0);
        assert_eq!((pp.current, pp.action_serial, pp.action.is_none()), (Some(key), 3, true));
        assert!((player.animation(n.lower).unwrap().seek_time() - 0.7).abs() < 1e-5);
        assert!(player.animation(n.upper.unwrap()).is_some());
    }

    #[test]
    fn graph_nodes_carry_full_weight() {
        if !std::path::Path::new(ANIM_DIR).exists() {
            return;
        }
        let (mut clips, mut graphs) = (Assets::<AnimationClip>::default(), Assets::<AnimationGraph>::default());
        let mut lib = AnimLibrary::default();
        for set in [AnimSet::Survivor, AnimSet::Zombie(Default::default()), AnimSet::Hound] {
            let h = lib.get(set, &mut clips, &mut graphs).graph.clone();
            let g = graphs.get(&h).unwrap();
            for (i, n) in g.nodes().map(|i| (i, &g[i])) {
                // A zero node weight zeroes every blend through it (NaN bones in crossfades).
                assert!(n.weight > 0.0, "{set:?} node {i:?} has weight {}", n.weight);
            }
        }
    }

    #[test]
    fn strips_exporter_dedup_suffix_only() {
        assert_eq!(bone_name("Root_001"), "Root");
        assert_eq!(bone_name("Root"), "Root");
        assert_eq!(bone_name("thumb_01_l"), "thumb_01_l");
        assert_eq!(bone_name("spine_03"), "spine_03");
    }
}
