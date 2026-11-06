//! `BP_LaboratoryCorpse` (28 in the lab, decompiled): a skeletal mesh posed by one frame of
//! Lab_CientistCorpse_Anim. UserConstructionScript: AnimationSingleNode with SavedPosition =
//! the instance's `AnimFloat` (seconds into the 0.28 s clip, a pose library: past the end holds
//! the last frame), not playing; the mesh is Lab_CientistCorpse, or Lab_HuardCorpse when
//! `Scientist?` is off; `EnableCollision?` (on by default, off on 23) switches QueryAndPhysics.
//! Its AIOSubject (layers 25 / 40 m) hides the mesh beyond the last layer.
//!
//! The pose is sampled onto the bones by hand (`models::anim::PoseTrack`), once the scene is in.
//! Ours: no collision (the game's is the physics asset; ours would need one), the hiding is by
//! distance to the camera.

use bevy::prelude::*;

use crate::menus::AppState;
use crate::models::anim::PoseTrack;

const DIR: &str = "SurrounDead/Content/Blueprints/Laboratory/Corpses";
/// Beyond the AIOSubject's last OptimizationLayer (4000 cm) the mesh is hidden.
const HIDE_PAST: f32 = 40.0;

#[derive(Clone, Debug, PartialEq)]
pub struct LabCorpse {
    pub actor: usize,
    pub transform: Transform,
    /// `Scientist?`: the scientist's mesh, else the guard's.
    pub scientist: bool,
    /// `AnimFloat`: the pose's time in the clip.
    pub pose: f32,
}

impl LabCorpse {
    pub fn from_actors(actors: &[super::ActorInfo]) -> Vec<Self> {
        actors
            .iter()
            .enumerate()
            .filter(|(_, a)| a.class == "BP_LaboratoryCorpse_C")
            .map(|(i, a)| {
                let prop = |k: &str| a.props.as_ref().and_then(|m| m.get(k));
                LabCorpse {
                    actor: i,
                    transform: super::affine(&a.transform),
                    // The class defaults: Scientist? on, AnimFloat 0.
                    scientist: prop("Scientist?").is_none_or(|v| v != "false"),
                    pose: prop("AnimFloat").and_then(|v| v.parse().ok()).unwrap_or(0.0),
                }
            })
            .collect()
    }

    pub fn mesh(&self) -> String {
        format!("game/{DIR}/{}.glb", if self.scientist { "Lab_CientistCorpse" } else { "Lab_HuardCorpse" })
    }
}

/// A corpse's model; `posed` once its bones took the pose.
#[derive(Component)]
pub(super) struct CorpseModel {
    pose: f32,
    posed: bool,
}

#[derive(Resource, Default)]
pub(super) struct CorpseTrack(Option<PoseTrack>);

pub(super) fn setup_lab_corpses(mut commands: Commands, level: Option<Res<super::Level>>, assets: Res<AssetServer>) {
    let corpses = level.map(|l| LabCorpse::from_actors(&l.actors)).unwrap_or_default();
    commands.insert_resource(CorpseTrack(PoseTrack::load(&format!("assets/game/{DIR}/Lab_CientistCorpse_Anim.anim.json"))));
    for c in &corpses {
        commands.spawn((crate::models::model(&assets, c.mesh()), c.transform, Visibility::Hidden, CorpseModel { pose: c.pose, posed: false }, DespawnOnExit(AppState::InGame)));
    }
    info!("lab corpses: {} ({} guards)", corpses.len(), corpses.iter().filter(|c| !c.scientist).count());
}

/// Pose each corpse once its bones exist; show it within 40 m of the camera.
pub(super) fn pose_lab_corpses(
    track: Res<CorpseTrack>,
    camera: Query<&GlobalTransform, With<crate::gameplay::PlayerCamera>>,
    mut models: Query<(Entity, &mut CorpseModel, &Transform, &mut Visibility)>,
    children: Query<&Children>,
    mut bones: Query<(&Name, &mut Transform), Without<CorpseModel>>,
) {
    let eye = camera.iter().next().map(|c| c.translation());
    for (e, mut c, t, mut vis) in &mut models {
        let near = eye.is_some_and(|eye| eye.distance(t.translation) <= HIDE_PAST);
        vis.set_if_neq(if near { Visibility::Inherited } else { Visibility::Hidden });
        if c.posed || !near {
            continue;
        }
        let Some(track) = &track.0 else { continue };
        let pose: Vec<_> = track.sample(c.pose).collect();
        let mut hit = 0;
        for d in children.iter_descendants(e) {
            if let Ok((name, mut bt)) = bones.get_mut(d)
                && let Some((_, p)) = pose.iter().find(|(b, _)| *b == name.as_str())
            {
                p.apply(&mut bt);
                hit += 1;
            }
        }
        if hit > 0 {
            c.posed = true;
            info!("lab corpse posed at {:.1}: {hit} bones, pose {}", t.translation, c.pose);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpses_read_their_mesh_and_pose() {
        let actor = |props: &[(&str, &str)]| super::super::ActorInfo {
            class: "BP_LaboratoryCorpse_C".into(),
            name: "C".into(),
            transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 3.0],
            props: Some(props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()),
        };
        let c = LabCorpse::from_actors(&[actor(&[]), actor(&[("Scientist?", "false"), ("AnimFloat", "0.16128")])]);
        assert_eq!((c[0].scientist, c[0].pose), (true, 0.0));
        assert_eq!((c[1].scientist, c[1].pose, c[1].actor), (false, 0.16128, 1));
        assert!(c[1].mesh().ends_with("Lab_HuardCorpse.glb"));
        assert_eq!(c[0].transform.translation, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn the_level_has_its_corpses_and_their_clip() {
        let Ok(a) = std::fs::read_to_string(format!("{}/actors.json", crate::level::LEVEL_DIR)) else { return };
        let actors: Vec<super::super::ActorInfo> = serde_json::from_str(&a).unwrap();
        let c = LabCorpse::from_actors(&actors);
        assert_eq!(c.len(), 28);
        assert!(c.iter().any(|c| !c.scientist) && c.iter().any(|c| c.pose > 0.0));
        if let Some(track) = PoseTrack::load(&format!("assets/game/{DIR}/Lab_CientistCorpse_Anim.anim.json")) {
            // Different AnimFloats are different poses.
            let a: Vec<_> = track.sample(0.04).map(|(_, p)| p.rotation).collect();
            let b: Vec<_> = track.sample(0.16).map(|(_, p)| p.rotation).collect();
            assert_ne!(a, b);
        }
    }
}
