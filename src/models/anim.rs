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
}

/// Ask for an upper-body action: bump `serial` with a new `kind` to (re)start it.
#[derive(Component, Default, Clone, Copy)]
pub struct UpperAction {
    pub kind: Option<ActionKind>,
    pub serial: u32,
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
            ],
            AnimSet::Zombie(_) => vec![((Unarmed, Hit), "Zombie/Regular/Other/Other/Anim_Zombie_hit")],
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
struct AnimFile {
    fps: f32,
    frames: usize,
    duration: f32,
    bones: HashMap<String, BoneKeys>,
}

/// A clip, its length and the bones it keys.
fn load_clip(path: &str) -> Option<(AnimationClip, f32, Vec<String>)> {
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
    Some((clip, duration, file.bones.into_keys().collect()))
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
            let mut loaded: HashMap<&str, (Handle<AnimationClip>, f32)> = HashMap::new();
            let mut bones = std::collections::HashSet::new();
            let mut load = |path: &'static str, clips: &mut Assets<AnimationClip>| {
                if let Some(c) = loaded.get(path) {
                    return Some(c.clone());
                }
                let (clip, duration, keyed) = load_clip(path).or_else(|| {
                    warn!("animation {path} missing (extractor/export_assets.sh --anims)");
                    None
                })?;
                bones.extend(keyed);
                let h = (clips.add(clip), duration);
                loaded.insert(path, h.clone());
                Some(h)
            };
            // Node weights stay 1: Bevy multiplies them into the player's per-animation weights
            // (which `drive_animation` fades), and two overlapping clips with zero effective
            // weight blend as 0/0 = NaN, which made characters vanish during every crossfade.
            for (key, path, looped) in set.clips() {
                let Some((clip, _)) = load(path, clips) else { continue };
                let lower = if layered { graph.add_clip_with_mask(clip.clone(), 1 << UPPER, 1.0, graph.root) } else { graph.add_clip(clip.clone(), 1.0, graph.root) };
                let upper = layered.then(|| graph.add_clip_with_mask(clip, 1 << LOWER, 1.0, graph.root));
                nodes.insert(key, LocoNodes { lower, upper, looped });
            }
            for (key, path) in set.actions() {
                let Some((clip, duration)) = load(path, clips) else { continue };
                actions.insert(key, (graph.add_clip_with_mask(clip, 1 << LOWER, 1.0, graph.root), duration));
            }
            if layered {
                for b in bones {
                    graph.add_target_to_mask_group(AnimationTargetId::from_name(&Name::new(b.clone())), if is_upper(&b) { UPPER } else { LOWER });
                }
            }
            SetGraph { graph: graphs.add(graph), nodes, actions }
        })
    }

    /// How long an action's clip runs (e.g. the reload), once the set has been built.
    pub fn action_secs(&self, set: AnimSet, stance: Stance, kind: ActionKind) -> Option<f32> {
        self.sets.get(&set)?.actions.get(&(stance, kind)).map(|a| a.1)
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

struct PlayingAction {
    node: AnimationNodeIndex,
    elapsed: f32,
    duration: f32,
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
        let targets = std::iter::once(root).chain(kids.into_iter().flat_map(|c| c.iter()));
        for e in targets {
            let Ok((mut pp, mut player)) = parts.get_mut(e) else { continue };
            let pp = &mut *pp;
            // A new locomotion clip starts from its beginning and fades in; the rest fade out.
            if pp.current != Some(wanted) {
                pp.current = Some(wanted);
                if !pp.fading.iter().any(|f| f.0 == wanted) {
                    pp.fading.push((wanted, 0.0));
                    let n = &g.nodes[&wanted];
                    for node in std::iter::once(n.lower).chain(n.upper) {
                        let a = player.start(node);
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
                if let Some(&(node, duration)) = req.kind.and_then(|k| g.actions.get(&(stance, k))) {
                    player.start(node);
                    pp.action = Some(PlayingAction { node, elapsed: 0.0, duration });
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
