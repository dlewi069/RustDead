//! `BP_PoliceStationGarageDoor` (decompiled): a FoldingGate skeletal mesh (its Door component,
//! 200 cm up and turned -90° yaw) that the police lever opens and closes. CallInteractOnObject ->
//! Event_OpenDoor: nothing while `DoorOpening?`; closed, it plays FoldingGate_Anim_Montage
//! forwards; open, backwards from 2.76 s; either way it flips `Open?` after a 2.76 s Delay. It
//! doesn't check power itself: the lever does.
//!
//! The clip is sampled onto the gate's bones by hand (`models::anim::PoseTrack`). Collision is
//! ours: the closed gate's bind-pose triangles while closed or moving, nothing once open (the
//! game collides with its physics asset).

use bevy::prelude::*;

use super::collision::CollisionCache;
use crate::menus::AppState;
use crate::models::anim::PoseTrack;

const MESH: &str = "game/SurrounDead/Content/Meshes/Gate/FoldingGate.glb";
const ANIM: &str = "assets/game/SurrounDead/Content/Meshes/Gate/FoldingGate_Anim.anim.json";
/// The Delay after each montage (its length).
const MONTAGE: f32 = 2.76;
/// Dynamic collision keys for gates, above any door index.
const COLLIDE_KEY: usize = 1 << 40;

#[derive(Clone, Debug, PartialEq)]
pub struct Gate {
    pub actor: usize,
    pub transform: Transform,
    /// Open?
    pub open: bool,
    /// DoorOpening?: seconds into the current montage.
    pub moving: Option<f32>,
}

impl Gate {
    /// Event_OpenDoor. Whether it started moving.
    pub fn toggle(&mut self) -> bool {
        if self.moving.is_some() {
            return false;
        }
        self.moving = Some(0.0);
        true
    }

    pub fn step(&mut self, dt: f32) {
        if let Some(e) = self.moving.as_mut() {
            *e += dt;
            if *e >= MONTAGE {
                self.open = !self.open;
                self.moving = None;
            }
        }
    }

    /// Where in the clip the gate is: opening plays it forwards, closing backwards from the end.
    pub fn clip_time(&self) -> f32 {
        match (self.moving, self.open) {
            (Some(e), false) => e,
            (Some(e), true) => MONTAGE - e,
            (None, true) => MONTAGE,
            (None, false) => 0.0,
        }
    }

    /// It blocks the way unless it's open and still.
    pub fn blocks(&self) -> bool {
        !self.open || self.moving.is_some()
    }
}

#[derive(Component)]
pub(super) struct GateModel(usize);

#[derive(Resource, Default)]
pub struct Garages {
    pub gates: Vec<Gate>,
    track: Option<PoseTrack>,
    mesh: Option<Handle<Mesh>>,
    /// The collision last set per gate (blocking or not).
    collision: Vec<Option<bool>>,
}

impl Garages {
    /// CallInteractOnObject on the gate actor `actor`.
    pub fn toggle(&mut self, actor: usize) -> bool {
        self.gates.iter_mut().find(|g| g.actor == actor).is_some_and(Gate::toggle)
    }

    pub fn from_actors(actors: &[super::ActorInfo]) -> Self {
        let gates: Vec<Gate> = actors
            .iter()
            .enumerate()
            .filter(|(_, a)| a.class == "BP_PoliceStationGarageDoor_C")
            .map(|(i, a)| {
                // The Door component: (0, 0, 200) cm, yaw -90 (a turn of +90° about our Y).
                let transform = super::affine(&a.transform) * Transform::from_xyz(0.0, 2.0, 0.0).with_rotation(Quat::from_rotation_y(90f32.to_radians()));
                Gate { actor: i, transform, open: false, moving: None }
            })
            .collect();
        let n = gates.len();
        Self { gates, collision: vec![None; n], ..default() }
    }
}

pub(super) fn setup_garages(mut commands: Commands, level: Option<Res<super::Level>>, assets: Res<AssetServer>) {
    let mut g = level.map(|l| Garages::from_actors(&l.actors)).unwrap_or_default();
    if !g.gates.is_empty() {
        g.track = PoseTrack::load(ANIM);
        g.mesh = Some(assets.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(MESH)));
    }
    for (i, gate) in g.gates.iter().enumerate() {
        commands.spawn((crate::models::model(&assets, MESH), gate.transform, GateModel(i), DespawnOnExit(AppState::InGame)));
    }
    commands.insert_resource(g);
}

/// Run the montages, pose the gates' bones and keep their collision current.
pub(super) fn move_garages(
    time: Res<Time>,
    mut garages: ResMut<Garages>,
    models: Query<(Entity, &GateModel)>,
    children: Query<&Children>,
    mut bones: Query<(&Name, &mut Transform)>,
    mut cache: ResMut<CollisionCache>,
    meshes: Res<Assets<Mesh>>,
) {
    let dt = time.delta_secs();
    for g in &mut garages.gates {
        g.step(dt);
    }
    let g = &mut *garages;
    if let Some(track) = &g.track {
        for (e, GateModel(i)) in &models {
            let Some(gate) = g.gates.get(*i) else { continue };
            let pose: Vec<_> = track.sample(gate.clip_time()).collect();
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
    let Some(mesh) = g.mesh.as_ref().and_then(|h| meshes.get(h)) else { return };
    for (i, gate) in g.gates.iter().enumerate() {
        let blocks = gate.blocks();
        if g.collision[i] != Some(blocks) || !cache.has_dynamic(COLLIDE_KEY + i) {
            cache.set_dynamic(COLLIDE_KEY + i, if blocks { super::collision::tris_of(mesh, &gate.transform) } else { Vec::new() });
            g.collision[i] = Some(blocks);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_opens_and_closes_over_the_montage() {
        let mut g = Gate { actor: 3, transform: Transform::IDENTITY, open: false, moving: None };
        assert!(g.blocks() && g.clip_time() == 0.0);
        assert!(g.toggle());
        // Pulling again mid-way does nothing (DoorOpening?).
        g.step(1.0);
        assert!(!g.toggle());
        assert_eq!(g.clip_time(), 1.0);
        g.step(1.8);
        assert!(g.open && g.moving.is_none() && !g.blocks());
        assert_eq!(g.clip_time(), MONTAGE);
        // Closing runs the clip backwards and blocks at once.
        assert!(g.toggle());
        g.step(0.76);
        assert!(g.blocks() && (g.clip_time() - 2.0).abs() < 1e-5);
        g.step(2.0);
        assert!(!g.open && g.clip_time() == 0.0);
    }

    #[test]
    fn level_gate_and_clip() {
        let Ok(a) = std::fs::read_to_string(format!("{}/actors.json", crate::level::LEVEL_DIR)) else { return };
        let actors: Vec<super::super::ActorInfo> = serde_json::from_str(&a).unwrap();
        let mut g = Garages::from_actors(&actors);
        assert_eq!(g.gates.len(), 1);
        let actor = g.gates[0].actor;
        assert!(g.toggle(actor) && !g.toggle(actor + 1));
        // 2 m above the actor.
        assert!((g.gates[0].transform.translation.y - actors[actor].position().y - 2.0).abs() < 1e-3);
        let Some(track) = PoseTrack::load(ANIM) else { return };
        // g1 carries the gate: it ends 4.7 m along its local -Z.
        let end: Vec<_> = track.sample(MONTAGE).collect();
        let g1 = end.iter().find(|(b, _)| *b == "g1").unwrap().1;
        assert!((g1.translation.unwrap().z + 4.747).abs() < 0.01);
        let start = track.sample(0.0).find(|(b, _)| *b == "g1").unwrap().1;
        assert!(start.translation.unwrap().length() < 1e-4);
        // Root's constant reference pose (scale 100, UE units) is left to the mesh.
        let root = end.iter().find(|(b, _)| *b == "Root").unwrap().1;
        assert_eq!(root, crate::models::anim::BonePose::default());
    }
}
