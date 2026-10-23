//! Containers spawned at BeginPlay by item spawners (decompiled):
//!
//! - `BP_SpecialItemContainerSpawner` (the police station): 2 s after BeginPlay, a
//!   `Container_SpecialItemSpawner` ("Briefcase", 4x3) at one of `Spawn1`..`Spawn4` (picked at
//!   random), turned to a random yaw, named after the instance's `Container Name` ("Police
//!   Briefcase"), with the instance's `UniqueLootSets` (DT_UniqueLoot rows: "PoliceStationKeys" =
//!   the Police Generator Key) added to its loot by `GameFunctionLibrary.AddUniqueLoot`.
//! - `BP_Keycard_Spawner`: a `BP_KeycardBriefcaseContainer` (loot table Keycards) at `Spawn0` or
//!   `Spawn1` with that component's rotation; tagged `EMS_SkipSave`, so it's rolled again each load.
//!
//! The spawn points are the instances' component overrides (`SdExport levelactors` writes them as
//! "<Component>.RelativeLocation"/"RelativeRotation", UE cm/degrees); unset ones keep the BP
//! templates' (the special spawner's points sit 75 cm out along ±X/±Y, the keycard spawner's at
//! its root). The briefcases are keyed in `Containers` by the spawner's actor index.

use bevy::prelude::*;
use rand::Rng;

use super::ActorInfo;
use crate::menus::AppState;

#[derive(Clone, Debug, PartialEq)]
pub struct SpawnedContainer {
    /// The spawner's actor index (the container's key in `gameplay::interact::Containers`).
    pub key: usize,
    pub class: &'static str,
    pub transform: Transform,
    /// The instance's `Container Name`, replacing the class's.
    pub name: Option<String>,
    /// DT_UniqueLoot rows added to its loot.
    pub unique: Vec<String>,
}

/// This session's spawned containers.
#[derive(Resource, Default, Debug)]
pub struct ItemSpawns(pub Vec<SpawnedContainer>);

/// A UE (X, Y, Z) JSON vector or rotator field set.
fn ue_vec(json: &str, keys: [&str; 3]) -> Option<Vec3> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    Some(Vec3::new(f(keys[0]), f(keys[1]), f(keys[2])))
}

/// A spawner component's world transform: `rel` (UE cm) and `yaw` (UE degrees) on the actor.
pub(crate) fn component_world(a: &ActorInfo, rel: Vec3, yaw: f32) -> Transform {
    let t = &a.transform;
    // The actor's UE X, Z, Y axes in Bevy space (see `doors::find`).
    let (x, z, y) = (Vec3::new(t[0], t[1], t[2]).normalize_or(Vec3::X), Vec3::new(t[3], t[4], t[5]).normalize_or(Vec3::Y), Vec3::new(t[6], t[7], t[8]).normalize_or(Vec3::Z));
    let rot = Quat::from_mat3(&Mat3::from_cols(x, z, y));
    // UE yaw turns X towards Y; with our axes that is a turn by -yaw about +Y (`doors::part_transform`).
    Transform::from_translation(a.position() + (x * rel.x + y * rel.y + z * rel.z) / 100.0).with_rotation(rot * Quat::from_rotation_y(-yaw.to_radians()))
}

/// A spawn point component's (relative location, relative yaw), or the template's.
pub(crate) fn point(a: &ActorInfo, name: &str, template: Vec3) -> (Vec3, f32) {
    let prop = |k: &str| a.props.as_ref().and_then(|m| m.get(&format!("{name}.{k}")));
    let rel = prop("RelativeLocation").and_then(|j| ue_vec(j, ["X", "Y", "Z"])).unwrap_or(template);
    let yaw = prop("RelativeRotation").and_then(|j| ue_vec(j, ["Pitch", "Yaw", "Roll"])).map_or(0.0, |r| r.y);
    (rel, yaw)
}

/// Roll every spawner's container.
pub fn roll(actors: &[ActorInfo], rng: &mut impl Rng) -> Vec<SpawnedContainer> {
    let mut out = Vec::new();
    for (i, a) in actors.iter().enumerate() {
        let prop = |k: &str| a.props.as_ref().and_then(|m| m.get(k));
        match a.class.as_str() {
            "BP_SpecialItemContainerSpawner_C" => {
                // Spawn1..4's template offsets.
                let points = [("Spawn1", Vec3::new(75.0, 0.0, 0.0)), ("Spawn2", Vec3::new(0.0, 75.0, 0.0)), ("Spawn3", Vec3::new(-75.0, 0.0, 0.0)), ("Spawn4", Vec3::new(0.0, -75.0, 0.0))];
                let (name, template) = points[rng.gen_range(0..points.len())];
                let (rel, _) = point(a, name, template);
                // MakeRotator(0, 0, RandomFloatInRange(-360, 360)): a world yaw, not the point's.
                let mut transform = component_world(a, rel, 0.0);
                transform.rotation = Quat::from_rotation_y(-rng.gen_range(-360.0f32..360.0).to_radians());
                let name = prop("Container Name")
                    .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
                    .and_then(|v| v.get("SourceString").and_then(|s| s.as_str()).map(str::to_string));
                let unique = prop("UniqueLootSets").and_then(|j| serde_json::from_str::<Vec<String>>(j).ok()).unwrap_or_default();
                out.push(SpawnedContainer { key: i, class: "Container_SpecialItemSpawner_C", transform, name, unique });
            }
            "BP_Keycard_Spawner_C" => {
                let name = ["Spawn0", "Spawn1"][rng.gen_range(0..2)];
                let (rel, yaw) = point(a, name, Vec3::ZERO);
                out.push(SpawnedContainer { key: i, class: "BP_KeycardBriefcaseContainer_C", transform: component_world(a, rel, yaw), name: None, unique: Vec::new() });
            }
            _ => {}
        }
    }
    out
}

pub(super) fn setup_item_spawns(mut commands: Commands, level: Option<Res<super::Level>>) {
    let spawns = level.map(|l| roll(&l.actors, &mut rand::thread_rng())).unwrap_or_default();
    commands.insert_resource(ItemSpawns(spawns));
}

/// Draw the spawned containers (their class's StaticMesh).
pub(super) fn spawn_item_containers(mut commands: Commands, spawns: Res<ItemSpawns>, db: Res<crate::gameplay::items::ItemDb>, assets: Res<AssetServer>) {
    for s in &spawns.0 {
        let Some(mesh) = db.containers.get(s.class).and_then(|c| c.mesh.as_deref()) else { continue };
        commands.spawn((crate::models::model(&assets, format!("game/{mesh}.glb")), s.transform, DespawnOnExit(AppState::InGame)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn actor(class: &str, props: &[(&str, &str)]) -> ActorInfo {
        ActorInfo {
            class: class.into(),
            name: "A".into(),
            // Identity rotation at (10, 0, 20).
            transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 20.0],
            props: Some(props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()),
        }
    }

    #[test]
    fn special_spawner_uses_its_points_name_and_sets() {
        let a = actor(
            "BP_SpecialItemContainerSpawner_C",
            &[
                ("Spawn1.RelativeLocation", r#"{"X":100.0,"Y":0.0,"Z":400.0}"#),
                ("Spawn2.RelativeLocation", r#"{"X":100.0,"Y":0.0,"Z":400.0}"#),
                ("Spawn3.RelativeLocation", r#"{"X":100.0,"Y":0.0,"Z":400.0}"#),
                ("Spawn4.RelativeLocation", r#"{"X":100.0,"Y":0.0,"Z":400.0}"#),
                ("Container Name", r#"{"SourceString":"Police Briefcase"}"#),
                ("UniqueLootSets", r#"["PoliceStationKeys"]"#),
            ],
        );
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        let s = roll(&[actor("Other", &[]), a], &mut rng);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].key, s[0].class), (1, "Container_SpecialItemSpawner_C"));
        // UE (X, Y, Z) cm -> (X, Z, Y) m on the actor.
        assert!(s[0].transform.translation.distance(Vec3::new(11.0, 4.0, 20.0)) < 1e-4);
        assert_eq!(s[0].name.as_deref(), Some("Police Briefcase"));
        assert_eq!(s[0].unique, ["PoliceStationKeys"]);
    }

    #[test]
    fn keycard_spawner_picks_one_of_two_points() {
        let a = actor("BP_Keycard_Spawner_C", &[("Spawn0.RelativeLocation", r#"{"X":0.0,"Y":1000.0,"Z":0.0}"#), ("Spawn1.RelativeLocation", r#"{"X":-500.0,"Y":0.0,"Z":0.0}"#), ("Spawn1.RelativeRotation", r#"{"Pitch":0.0,"Yaw":90.0,"Roll":0.0}"#)]);
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..40 {
            let s = roll(std::slice::from_ref(&a), &mut rng);
            let t = s[0].transform;
            seen.insert((t.translation.x.round() as i32, t.translation.z.round() as i32));
            if t.translation.x.round() == 5.0 {
                // Spawn1's yaw 90: UE X (our +X) turns to UE Y (our +Z).
                assert!((t.rotation * Vec3::X).distance(Vec3::Z) < 1e-4);
            }
        }
        assert_eq!(seen, [(10, 30), (5, 20)].into_iter().collect());
    }

    #[test]
    fn level_spawners_resolve() {
        let Ok(a) = std::fs::read_to_string(format!("{}/actors.json", crate::level::LEVEL_DIR)) else { return };
        let actors: Vec<ActorInfo> = serde_json::from_str(&a).unwrap();
        let s = roll(&actors, &mut rand::thread_rng());
        // The police station's briefcase and three keycard briefcases.
        assert_eq!(s.iter().filter(|c| c.class == "Container_SpecialItemSpawner_C").count(), 1);
        assert_eq!(s.iter().filter(|c| c.class == "BP_KeycardBriefcaseContainer_C").count(), 3);
        let police = s.iter().find(|c| c.name.is_some()).unwrap();
        assert_eq!((police.name.as_deref(), police.unique.as_slice()), (Some("Police Briefcase"), &["PoliceStationKeys".to_string()][..]));
        // Within the station: the points are up to ~16 m from the spawner.
        let at = actors[police.key].position();
        assert!(police.transform.translation.distance(at) < 20.0);
    }
}
