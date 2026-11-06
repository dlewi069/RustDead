//! The buildable doors and gates (Blueprints/BuildingSystem/Actors/Doors, decompiled).
//!
//! - **Buildable_WoodenDoor / _WoodenDoor2 / _BarricadedMetalDoor / _BunkerDoor** (one graph,
//!   four classes): Interact (Option1) -> MC_OnInteractBuildable -> Event_OpenDoor. Closed
//!   (IsClosed): IsClosed = false, DotProduct = Normal(StaticMesh location - the player's) .
//!   StaticMesh forward, Timeline_OpenDoor.Play; open: Reverse. The update sets StaticMesh's
//!   relative rotation to yaw Lerp(0, DotProduct >= 0 ? 90 : -90, Alpha), so the door swings
//!   away from whoever opened it. The timeline is 0.5 s and its Alpha curve (0 -> 1, auto
//!   tangents 2) is a straight line. It finishing backwards sets IsClosed = true.
//! - **Buildable_WoodenGate**: the same with two leaves on 2 s timelines (Timeline_0: Gate_DoorLeft
//!   to yaw 90, Timeline_1: Gate_DoorRight to -90; zero end tangents, an ease in and out); the
//!   DotProduct is worked out but unused, so the gate always opens the same way.
//! - **Buildable_GarageDoor** (Requires Power?): a skeletal mesh. Interact without power:
//!   "Requires power" (2 s); while DoorOpening? nothing; else GarageDoor_AnimFix_Montage forwards
//!   (closed) or backwards from 1.83 s (Opened?), DoorOpening? = true, and after a 1.83 s Delay
//!   Opened? flips and DoorOpening? = false. Its Power stat reads On / Off (the delegates).
//! - None of them saves IsClosed / Opened? (no SaveGame flag): placed doors come back closed.
//!
//! Ours: collision follows the leaves when a swing ends (the game sweeps the moving mesh); the
//! garage door blocks with its closed bind-pose triangles unless open and still (the game uses
//! its physics asset), like `level::garage`. The doors have no working health in the game
//! either (Buildable_MASTER's, see `building.rs`).

use bevy::prelude::*;

use super::building::{Buildables, Live};
use crate::models::anim::PoseTrack;

/// Buildable_GarageDoor's SkeletalMesh and its montage's clip.
pub const GARAGE_MESH: &str = "SurrounDead/Content/Meshes/Buildables/GarageDoor/GarageDoor";
const GARAGE_ANIM: &str = "assets/game/SurrounDead/Content/Meshes/Buildables/GarageDoor/GarageDoor_AnimFix.anim.json";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// One leaf (StaticMesh) on a 0.5 s timeline, away from the player.
    Swing,
    /// Two leaves on 2 s timelines.
    Gate,
    /// The powered skeletal door on its 1.83 s montage.
    Garage,
}

impl Kind {
    pub fn of(actor: &str) -> Option<Kind> {
        Some(match actor {
            "Buildable_WoodenDoor_C" | "Buildable_WoodenDoor2_C" | "Buildable_BarricadedMetalDoor_C" | "Buildable_BunkerDoor_C" => Kind::Swing,
            "Buildable_WoodenGate_C" => Kind::Gate,
            "Buildable_GarageDoor_C" => Kind::Garage,
            _ => return None,
        })
    }

    /// TimelineLength, or the montage's Delay.
    pub fn length(self) -> f32 {
        match self {
            Kind::Swing => 0.5,
            Kind::Gate => 2.0,
            Kind::Garage => 1.83,
        }
    }

    /// The Alpha track at `t` seconds.
    pub fn alpha(self, t: f32) -> f32 {
        let s = (t / self.length()).clamp(0.0, 1.0);
        match self {
            // Cubic keys with zero leave/arrive tangents: Hermite = smoothstep.
            Kind::Gate => s * s * (3.0 - 2.0 * s),
            _ => s,
        }
    }
}

/// A placed door's IsClosed (Opened? inverted) and its timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct DoorState {
    pub closed: bool,
    /// The timeline's (or montage's) position, seconds.
    pub t: f32,
    /// +1 playing, -1 reversing, 0 still.
    pub dir: f32,
    /// DotProduct when it last opened.
    pub dot: f32,
}

impl Default for DoorState {
    fn default() -> Self {
        Self { closed: true, t: 0.0, dir: 0.0, dot: 1.0 }
    }
}

impl DoorState {
    /// Event_OpenDoor (Swing / Gate) or the garage's interact once powered. `dot` = DotProduct.
    /// Whether anything started.
    pub fn interact(&mut self, kind: Kind, dot: f32) -> bool {
        if kind == Kind::Garage {
            // DoorOpening? blocks; Opened? picks the direction.
            if self.dir != 0.0 {
                return false;
            }
            self.dir = if self.closed { 1.0 } else { -1.0 };
            return true;
        }
        if self.closed {
            self.closed = false;
            self.dot = dot;
            self.dir = 1.0;
        } else {
            self.dir = -1.0;
        }
        true
    }

    /// Advance; true when the motion ended this step.
    pub fn step(&mut self, kind: Kind, dt: f32) -> bool {
        if self.dir == 0.0 {
            return false;
        }
        self.t += self.dir * dt;
        let len = kind.length();
        if self.t >= len {
            self.t = len;
            self.dir = 0.0;
            // The garage's Opened? flips after its Delay.
            self.closed = false;
            true
        } else if self.t <= 0.0 {
            self.t = 0.0;
            self.dir = 0.0;
            // Finished going backwards: IsClosed.
            self.closed = true;
            true
        } else {
            false
        }
    }

    /// The leaves' yaws (UE degrees) for (StaticMesh or Gate_DoorLeft, Gate_DoorRight).
    pub fn yaws(&self, kind: Kind) -> (f32, f32) {
        let a = kind.alpha(self.t);
        match kind {
            Kind::Swing => (a * if self.dot >= 0.0 { 90.0 } else { -90.0 }, 0.0),
            Kind::Gate => (a * 90.0, a * -90.0),
            Kind::Garage => (0.0, 0.0),
        }
    }

    /// The garage door blocks unless open and still.
    pub fn blocks(&self) -> bool {
        self.closed || self.dir != 0.0
    }
}

/// A moving leaf: the placed buildable's id and which yaw it takes (0 the first, 1 the second).
#[derive(Component)]
pub struct Leaf(pub usize, pub u8);

/// The garage door's skeletal model.
#[derive(Component)]
pub struct GarageModel(pub usize);

/// Which leaf a part of a door actor is.
pub fn leaf(kind: Option<Kind>, part: &str) -> Option<u8> {
    match (kind?, part) {
        (Kind::Swing, "StaticMesh") | (Kind::Gate, "Gate_DoorLeft") => Some(0),
        (Kind::Gate, "Gate_DoorRight") => Some(1),
        _ => None,
    }
}

/// UE yaw (degrees) as a glb-space rotation.
pub fn yaw_quat(yaw: f32) -> Quat {
    super::traps::ue_rotator(0.0, yaw, 0.0)
}

/// Part `i`'s transform relative to the actor with the door's leaves turned.
pub fn part_pose(l: &Live, def: &super::building::ActorDef, i: usize) -> Transform {
    let t = def.part_transform(i);
    let Some(kind) = Kind::of(&l.actor) else { return t };
    match leaf(Some(kind), &def.parts[i].name) {
        Some(n) => {
            let (a, b) = l.door.yaws(kind);
            // SetRelativeRotation replaces the part's own (identity in all four) rotation.
            t * Transform::from_rotation(yaw_quat(if n == 0 { a } else { b }))
        }
        None => t,
    }
}

/// DotProduct: the player-to-door direction against StaticMesh's forward (UE X, ours +X).
pub fn dot_product(door: &Transform, player: Vec3) -> f32 {
    let to = (door.translation - player).normalize_or_zero();
    to.dot(door.rotation * Vec3::X)
}

#[derive(Resource, Default)]
struct GarageClip(Option<PoseTrack>);

pub fn plugin(app: &mut App) {
    app.init_resource::<GarageClip>()
        .add_systems(OnEnter(crate::menus::AppState::InGame), |mut c: ResMut<GarageClip>| {
            if c.0.is_none() {
                c.0 = PoseTrack::load(GARAGE_ANIM);
            }
        })
        .add_systems(Update, swing.after(super::building::BuildSet).run_if(in_state(crate::menus::AppState::InGame)));
}

/// Run the timelines, turn the leaves, pose the garage doors; collision is redone when a motion ends.
#[allow(clippy::type_complexity)]
fn swing(
    time: Res<Time>,
    clip: Res<GarageClip>,
    mut placed: ResMut<Buildables>,
    mut leaves: Query<(&Leaf, &mut Transform)>,
    models: Query<(Entity, &GarageModel)>,
    children: Query<&Children>,
    mut bones: Query<(&Name, &mut Transform), Without<Leaf>>,
) {
    let dt = time.delta_secs();
    for l in placed.list.iter_mut() {
        let Some(kind) = Kind::of(&l.actor) else { continue };
        if l.door.step(kind, dt) {
            info!("build doors: {} {}", l.item, if l.door.closed { "closed" } else { "open" });
            l.unsolid();
        }
    }
    for (Leaf(id, n), mut t) in &mut leaves {
        let Some(l) = placed.list.iter().find(|l| l.id == *id) else { continue };
        let Some(kind) = Kind::of(&l.actor) else { continue };
        let (a, b) = l.door.yaws(kind);
        let q = yaw_quat(if *n == 0 { a } else { b });
        if t.rotation != q {
            t.rotation = q;
        }
    }
    let Some(track) = &clip.0 else { return };
    for (e, GarageModel(id)) in &models {
        let Some(l) = placed.list.iter().find(|l| l.id == *id) else { continue };
        let pose: Vec<_> = track.sample(l.door.t).collect();
        for d in children.iter_descendants(e) {
            if let Ok((name, mut t)) = bones.get_mut(d)
                && let Some((_, p)) = pose.iter().find(|(b, _)| *b == name.as_str())
            {
                let mut want = *t;
                p.apply(&mut want);
                if want != *t {
                    *t = want;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::building::BuildableDb;

    #[test]
    fn swing_door_opens_away_and_closes() {
        let mut d = DoorState::default();
        assert!(d.interact(Kind::Swing, -0.4));
        // IsClosed drops at once; the yaw goes towards -90 for a negative dot.
        assert!(!d.closed);
        d.step(Kind::Swing, 0.25);
        assert!((d.yaws(Kind::Swing).0 + 45.0).abs() < 1e-3);
        assert!(d.step(Kind::Swing, 0.3));
        assert_eq!(d.yaws(Kind::Swing).0, -90.0);
        // Interact again: Reverse; IsClosed only once back at 0.
        assert!(d.interact(Kind::Swing, 0.9));
        d.step(Kind::Swing, 0.4);
        assert!(!d.closed && d.dot == -0.4);
        assert!(d.step(Kind::Swing, 0.2));
        assert!(d.closed && d.yaws(Kind::Swing).0 == 0.0);
        // Opened from the other side it swings the other way.
        d.interact(Kind::Swing, 0.2);
        d.step(Kind::Swing, 1.0);
        assert_eq!(d.yaws(Kind::Swing).0, 90.0);
    }

    #[test]
    fn swing_door_reverses_mid_way() {
        let mut d = DoorState::default();
        d.interact(Kind::Swing, 1.0);
        d.step(Kind::Swing, 0.2);
        d.interact(Kind::Swing, 1.0);
        assert!(d.step(Kind::Swing, 0.25));
        assert!(d.closed);
    }

    #[test]
    fn gate_eases_both_leaves() {
        let mut d = DoorState::default();
        d.interact(Kind::Gate, -1.0);
        d.step(Kind::Gate, 1.0);
        assert_eq!(d.yaws(Kind::Gate), (45.0, -45.0));
        d.step(Kind::Gate, 0.5);
        // smoothstep(0.75) = 0.84375
        assert!((d.yaws(Kind::Gate).0 - 75.9375).abs() < 1e-3);
        assert!(d.step(Kind::Gate, 0.5));
        assert_eq!(d.yaws(Kind::Gate), (90.0, -90.0));
    }

    #[test]
    fn garage_ignores_presses_while_moving() {
        let mut d = DoorState::default();
        assert!(d.blocks());
        assert!(d.interact(Kind::Garage, 0.0));
        d.step(Kind::Garage, 1.0);
        assert!(!d.interact(Kind::Garage, 0.0));
        // Opened? flips only after the Delay.
        assert!(d.closed && d.blocks());
        assert!(d.step(Kind::Garage, 0.9));
        assert!(!d.closed && !d.blocks() && d.t == 1.83);
        assert!(d.interact(Kind::Garage, 0.0));
        assert!(d.blocks());
        d.step(Kind::Garage, 2.0);
        assert!(d.closed && d.t == 0.0);
    }

    #[test]
    fn leaves_and_dot() {
        assert_eq!(leaf(Kind::of("Buildable_BunkerDoor_C"), "StaticMesh"), Some(0));
        assert_eq!(leaf(Kind::of("Buildable_WoodenGate_C"), "StaticMesh"), None);
        assert_eq!(leaf(Kind::of("Buildable_WoodenGate_C"), "Gate_DoorRight"), Some(1));
        assert_eq!(leaf(Kind::of("Buildable_Box_C"), "StaticMesh"), None);
        // Standing on the door's -X side (behind its forward): dot > 0.
        let door = Transform::from_xyz(5.0, 0.0, 0.0);
        assert!(dot_product(&door, Vec3::ZERO) > 0.99);
        assert!(dot_product(&door, Vec3::new(10.0, 0.0, 0.0)) < -0.99);
        // UE yaw 90 turns X to UE Y = our +Z.
        assert!((yaw_quat(90.0) * Vec3::X - Vec3::Z).length() < 1e-5);
    }

    #[test]
    fn buildables_have_the_door_parts() {
        let db = BuildableDb::load();
        for (actor, def) in &db.actors {
            match Kind::of(actor) {
                Some(Kind::Swing) => assert!(def.parts.iter().any(|p| p.name == "StaticMesh"), "{actor}"),
                Some(Kind::Gate) => assert!(["Gate_DoorLeft", "Gate_DoorRight"].iter().all(|n| def.parts.iter().any(|p| &p.name == n))),
                Some(Kind::Garage) => assert!(def.requires_power),
                None => {}
            }
            if Kind::of(actor).is_some() {
                assert!(def.options.first().map(String::as_str) == Some("Interact"), "{actor}");
            }
        }
        assert_eq!(db.actors.keys().filter(|a| Kind::of(a).is_some()).count(), 6);
    }
}
