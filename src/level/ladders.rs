//! `BP_Ladder` (decompiled): a climb box from the ladder's foot (`MeshBottom`, per instance) to
//! `OffsetBox` (55 cm) above the actor, 40 cm deep and 80 cm wide, and a top box (80x90x60 cm,
//! 55..115 cm up) to step onto it from above. Only overlap *begins* and *ends* do anything:
//!
//! - entering the climb box (with `CanClimb?`, not already climbing): ClimbingLadder? on, the
//!   survivor snapped to `PlayerPostition` (40 cm in front, height kept) facing the ladder;
//! - entering the top box: the same, 75 cm lower;
//! - leaving the climb box: ClimbingLadder? off and `CanClimb?` off for 0.2 s.
//!
//! While climbing (`BP_PlayerCharacter`: MOVE_Flying, MaxFlySpeed 100) W/S move along the up
//! axis at 1 m/s, strafing does nothing and Jump lets go. Leaving the box at the top drops back
//! to walking on whatever is there; at the foot the floor stops the descent.

use bevy::prelude::*;

use super::{ActorInfo, KinematicBody};

/// MaxFlySpeed, m/s.
pub const CLIMB_SPEED: f32 = 1.0;
const SNAP_BACK: f32 = 40.0;
const TOP_DROP: f32 = 75.0;
const COOLDOWN: f32 = 0.2;

#[derive(Clone, Debug)]
pub struct LadderDef {
    pub pos: Vec3,
    /// The actor's +X (towards the ladder from where the survivor stands) and +Y, horizontal.
    pub x: Vec3,
    pub y: Vec3,
    /// |MeshBottom| and OffsetBox, UE cm.
    pub bottom: f32,
    pub offset: f32,
    /// `Mesh?`: it draws its own LadderPiece stack (false: a climb volume on a building's ladder).
    pub visible: bool,
}

/// An axis-aligned box in the ladder's local UE cm.
#[derive(Clone, Copy, Debug)]
struct LocalBox {
    min: Vec3,
    max: Vec3,
}

impl LadderDef {
    pub fn from_actor(a: &ActorInfo) -> Option<Self> {
        if a.class != "BP_Ladder_C" {
            return None;
        }
        let prop = |k: &str| a.props.as_ref().and_then(|m| m.get(k)).cloned();
        let bottom = prop("MeshBottom")
            .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
            .map(|v| {
                let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                Vec3::new(f("X"), f("Y"), f("Z")).length()
            })
            .unwrap_or(200.0);
        let offset = prop("OffsetBox").and_then(|v| v.parse().ok()).unwrap_or(55.0);
        let t = &a.transform;
        // The actor's axes (see doors::find): UE X is column 0, UE Y column 2.
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z).normalize_or(Vec3::X);
        let visible = prop("Mesh?").as_deref() != Some("false");
        Some(Self { pos: a.position(), x: flat(Vec3::new(t[0], t[1], t[2])), y: flat(Vec3::new(t[6], t[7], t[8])), bottom, offset, visible })
    }

    /// World point -> ladder-local UE cm.
    fn local(&self, p: Vec3) -> Vec3 {
        let d = p - self.pos;
        Vec3::new(d.dot(self.x), d.dot(self.y), d.y) * 100.0
    }

    /// Ladder-local UE cm -> world.
    fn world(&self, l: Vec3) -> Vec3 {
        self.pos + (self.x * l.x + self.y * l.y + Vec3::Y * l.z) / 100.0
    }

    /// LadderCollision: extent (20, 40, (bottom + offset) / 2) at (-15, 0, offset - half).
    fn climb_box(&self) -> LocalBox {
        LocalBox { min: Vec3::new(-35.0, -40.0, -self.bottom), max: Vec3::new(5.0, 40.0, self.offset) }
    }

    /// LadderTopCollision: extent (40, 45, 30) at (-35, 0, 85).
    fn top_box(&self) -> LocalBox {
        LocalBox { min: Vec3::new(-75.0, -45.0, 55.0), max: Vec3::new(5.0, 45.0, 115.0) }
    }

    /// Whether a standing capsule (feet at `p`) overlaps `b` (the capsule as its bounding box).
    fn overlaps(&self, b: LocalBox, p: Vec3, radius: f32, height: f32) -> bool {
        let l = self.local(p);
        let (r, h) = (radius * 100.0, height * 100.0);
        l.x > b.min.x - r && l.x < b.max.x + r && l.y > b.min.y - r && l.y < b.max.y + r && l.z + h > b.min.z && l.z < b.max.z
    }

    /// Where the survivor hangs on it at height `y` (PlayerPostition, X/Y only), and the
    /// facing (towards the ladder).
    pub fn grip(&self, y: f32) -> (Vec3, Quat) {
        let mut p = self.world(Vec3::new(-SNAP_BACK, 0.0, 0.0));
        p.y = y;
        // Survivor models face +Z.
        (p, Quat::from_rotation_y(self.x.x.atan2(self.x.z)))
    }
}

/// Per-ladder overlap memory (BP_Ladder's CanClimb? and the begin/end events).
#[derive(Resource, Default)]
pub struct Ladders {
    pub defs: Vec<LadderDef>,
    in_climb: Vec<bool>,
    in_top: Vec<bool>,
    cooldown: Vec<f32>,
}

/// What a ladder does to the survivor this frame.
#[derive(Debug, PartialEq)]
pub enum Event {
    /// Start climbing, snapped to `grip` at this height.
    Grab(f32),
    LetGo,
}

impl Ladders {
    pub fn new(defs: Vec<LadderDef>) -> Self {
        let n = defs.len();
        Self { defs, in_climb: vec![false; n], in_top: vec![false; n], cooldown: vec![0.0; n] }
    }

    /// Advance ladder `i`'s overlap state for a capsule with feet at `p`.
    pub fn update(&mut self, i: usize, p: Vec3, radius: f32, height: f32, climbing: bool, dt: f32) -> Option<Event> {
        let d = &self.defs[i];
        let (climb, top) = (d.overlaps(d.climb_box(), p, radius, height), d.overlaps(d.top_box(), p, radius, height));
        self.cooldown[i] = (self.cooldown[i] - dt).max(0.0);
        let can = self.cooldown[i] <= 0.0;
        // Stepping back off the top enters both boxes at once; the top one is for that (75 cm down).
        let ev = if self.in_climb[i] && !climb {
            self.cooldown[i] = COOLDOWN;
            Some(Event::LetGo)
        } else if top && !self.in_top[i] && can && !climbing {
            Some(Event::Grab(p.y - TOP_DROP / 100.0))
        } else if climb && !self.in_climb[i] && can && !climbing {
            Some(Event::Grab(p.y))
        } else {
            None
        };
        self.in_climb[i] = climb;
        self.in_top[i] = top;
        ev
    }
}

pub(super) fn setup_ladders(mut commands: Commands, level: Option<Res<super::Level>>) {
    let defs = level.map(|l| l.actors.iter().filter_map(LadderDef::from_actor).collect()).unwrap_or_default();
    commands.insert_resource(Ladders::new(defs));
}

/// Grab and let go of ladders (before the bodies move).
pub(super) fn climb_ladders(time: Res<Time>, ladders: Option<ResMut<Ladders>>, mut bodies: Query<(&mut KinematicBody, &mut Transform), With<crate::models::CharacterModel>>) {
    let Some(mut ladders) = ladders else { return };
    let dt = time.delta_secs();
    for (mut body, mut t) in &mut bodies {
        let p = t.translation;
        for i in 0..ladders.defs.len() {
            if ladders.defs[i].pos.distance_squared(p) > 30.0 * 30.0 {
                continue;
            }
            match ladders.update(i, p, body.radius, body.height, body.climbing, dt) {
                Some(Event::Grab(y)) => {
                    let (at, facing) = ladders.defs[i].grip(y);
                    t.translation = at;
                    t.rotation = facing;
                    body.climbing = true;
                    body.vertical_velocity = 0.0;
                    body.grounded = false;
                }
                Some(Event::LetGo) => body.climbing = false,
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ladder() -> LadderDef {
        // At the origin facing +X, 4 m tall: the foot 4 m below the actor.
        LadderDef { pos: Vec3::new(0.0, 4.0, 0.0), x: Vec3::X, y: Vec3::Z, bottom: 400.0, offset: 55.0, visible: true }
    }

    #[test]
    fn walking_into_the_foot_grabs_and_climbing_out_the_top_lets_go() {
        let mut l = Ladders::new(vec![ladder()]);
        let (r, h) = (0.35, 1.8);
        // Standing back from it: nothing.
        assert_eq!(l.update(0, Vec3::new(-1.5, 0.0, 0.0), r, h, false, 0.1), None);
        // Walked up to it: grab at this height, snapped 40 cm in front facing +X.
        assert_eq!(l.update(0, Vec3::new(-0.6, 0.0, 0.0), r, h, false, 0.1), Some(Event::Grab(0.0)));
        let (at, facing) = l.defs[0].grip(0.0);
        assert!((at - Vec3::new(-0.4, 0.0, 0.0)).length() < 1e-5);
        assert!((facing * Vec3::Z).abs_diff_eq(Vec3::X, 1e-5));
        // Climbing: inside, nothing new (passing through the top box too).
        assert_eq!(l.update(0, Vec3::new(-0.4, 3.9, 0.0), r, h, true, 0.1), None);
        // Feet above OffsetBox (4.55 m): let go, and 0.2 s before grabbing again.
        assert_eq!(l.update(0, Vec3::new(-0.4, 4.6, 0.0), r, h, true, 0.1), Some(Event::LetGo));
        assert_eq!(l.update(0, Vec3::new(-0.4, 4.5, 0.0), r, h, false, 0.1), None);
    }

    #[test]
    fn stepping_onto_the_top_climbs_down_75_cm() {
        let mut l = Ladders::new(vec![ladder()]);
        // On the roof beyond the ladder (+X), feet at the actor's height: outside both boxes.
        assert_eq!(l.update(0, Vec3::new(1.5, 4.0, 0.0), 0.35, 1.8, false, 0.1), None);
        // Backing to the edge enters both; the top box puts them 75 cm down the ladder.
        assert_eq!(l.update(0, Vec3::new(0.3, 4.0, 0.0), 0.35, 1.8, false, 0.1), Some(Event::Grab(3.25)));
    }

    #[test]
    fn level_ladders_have_their_heights() {
        let dir = crate::level::LEVEL_DIR;
        let Ok(a) = std::fs::read_to_string(format!("{dir}/actors.json")) else { return };
        let actors: Vec<ActorInfo> = serde_json::from_str(&a).unwrap();
        let defs: Vec<LadderDef> = actors.iter().filter_map(LadderDef::from_actor).collect();
        assert_eq!(defs.len(), 48);
        // Per-instance MeshBottom (3.6 m to 14 m ladders), not all the BP default.
        assert!(defs.iter().any(|d| (d.bottom - 360.0).abs() < 1.0) && defs.iter().any(|d| d.bottom > 1400.0), "{:?}", defs.iter().map(|d| d.bottom).collect::<Vec<_>>());
    }
}
