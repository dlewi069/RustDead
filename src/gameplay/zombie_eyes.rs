//! The zombies' glowing eyes: BP_MasterZombie's and BP_ZombieBoss's EyeLeftMesh / EyeRightMesh
//! (decompiled, `zombies.json` `eyes`, `extractor/gen_ai.py`):
//!
//! - Two 1M_Cube StaticMeshComponents on the mesh's `eyesleft` / `eyesright` sockets (both on
//!   the `eyes` bone), at each class's RelativeLocation / Rotation / Scale3D (about 3 x 2 x 1 cm),
//!   no collision, no shadow, in the class's M_GlowingEyes instance: the Roamer's (and the
//!   crawler's) orange x 10, Infected red x 10, Radiated green x 10, the bosses' grey x 5, the
//!   horde bosses' Infected red.
//! - BeginPlay: with Game.DisableZombieEyes (DISABLE ZOMBIE EYES?, Gameplay page) both meshes
//!   are set to none, for the zombie's life.
//! - Event_BloodMoon (BP_MasterZombie's children only): Start? puts M_GlowingEyes_BloodMoon on
//!   both (red x 150), its end the class's EyeMaterial back. It's bound at BeginPlay, so a zombie
//!   that spawns during a blood moon keeps its own eyes (`world_events::BloodMoonBoost`).
//! - Death destroys both components.
//!
//! Ours: M_GlowingEyes's graph isn't readable; an eye is an unlit box of Colour x
//! EmissiveStrength (no opacity mask). The AI optimizer's layers that hide the eyes farther off
//! aren't ported: they show at any distance. The hound's skeleton has no eye sockets (UE would
//! hang its eyes at the mesh's origin, inside the body), so it has none.

use std::collections::HashMap;

use bevy::prelude::*;

use super::combat::Dead;
use super::world_events::BloodMoonBoost;
use super::zombies::{EyeLook, EyesDef, blood_moon_eyes};
use crate::level::LevelZombie;
use crate::menus::AppState;
use crate::menus::settings::{SettingsStore, Value, row_key};

pub const DISABLE: &str = "DISABLE ZOMBIE EYES?";

/// A zombie's eyes (none: disabled, or its class has none) and whether they're the blood moon's.
#[derive(Component, Debug)]
pub struct Eyes {
    pub parts: Vec<Entity>,
    pub blood_moon: bool,
}

/// The eye box and a material per look.
#[derive(Resource, Default)]
struct EyeAssets {
    cube: Option<Handle<Mesh>>,
    looks: HashMap<[u32; 4], Handle<StandardMaterial>>,
}

impl EyeAssets {
    fn material(&mut self, materials: &mut Assets<StandardMaterial>, look: EyeLook) -> Handle<StandardMaterial> {
        let key = [look.colour[0].to_bits(), look.colour[1].to_bits(), look.colour[2].to_bits(), look.strength.to_bits()];
        self.looks.entry(key).or_insert_with(|| materials.add(StandardMaterial { base_color: emissive(look), unlit: true, ..default() })).clone()
    }
}

/// Colour x EmissiveStrength.
pub fn emissive(look: EyeLook) -> Color {
    let [r, g, b] = look.colour.map(|c| c * look.strength);
    Color::linear_rgb(r, g, b)
}

/// The look a class's eyes have: the blood moon's while it's up for this zombie.
pub fn look(def: &EyesDef, blood_moon: bool) -> EyeLook {
    if blood_moon && def.blood_moon { blood_moon_eyes().unwrap_or(def.look) } else { def.look }
}

/// An eye under its bone: the socket's offset, then the component's transform.
pub fn eye_transform(part: &super::zombies::EyePart) -> Transform {
    part.socket.transform.to_transform() * part.local.to_transform()
}

/// Whether DISABLE ZOMBIE EYES? is ticked.
pub fn disabled(store: &SettingsStore) -> bool {
    store.find(&row_key("settings", "Gameplay", DISABLE)) == Some(Value::Bool(true))
}

pub fn plugin(app: &mut App) {
    app.init_resource::<EyeAssets>().add_systems(Update, (attach, blood_moon, on_death).chain().run_if(in_state(AppState::InGame)));
}

/// BeginPlay: the eyes on the `eyes` bone once the model is in (every body part has its own copy
/// of the skeleton, animated alike; any copy will do).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn attach(
    mut commands: Commands,
    store: Res<SettingsStore>,
    mut res: ResMut<EyeAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    zombies: Query<(Entity, &LevelZombie, Has<BloodMoonBoost>), (Without<Eyes>, Without<Dead>)>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    let off = disabled(&store);
    for (e, z, boost) in &zombies {
        let Some(def) = z.kind.class().eyes.as_ref().filter(|_| !off) else {
            commands.entity(e).insert(Eyes { parts: Vec::new(), blood_moon: false });
            continue;
        };
        let bone_named = |bone: &str| {
            children.iter_descendants(e).find(|&c| names.get(c).is_ok_and(|n| n.as_str() == bone || n.as_str().strip_prefix(bone).and_then(|r| r.strip_prefix('_')).is_some_and(|r| r.chars().all(|c| c.is_ascii_digit()))))
        };
        let Some(bones) = def.parts.iter().map(|p| bone_named(&p.socket.bone)).collect::<Option<Vec<_>>>() else { continue };
        let cube = res.cube.get_or_insert_with(|| meshes.add(Cuboid::from_length(1.0))).clone();
        let blood = boost && def.blood_moon;
        let material = res.material(&mut materials, look(def, blood));
        let parts = def
            .parts
            .iter()
            .zip(bones)
            .map(|(p, bone)| commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(material.clone()), eye_transform(p), bevy::light::NotShadowCaster, ChildOf(bone))).id())
            .collect();
        commands.entity(e).insert(Eyes { parts, blood_moon: blood });
    }
}

/// Event_BloodMoon: the blood moon's eyes on, then the class's back.
fn blood_moon(mut res: ResMut<EyeAssets>, mut materials: ResMut<Assets<StandardMaterial>>, mut zombies: Query<(&LevelZombie, &mut Eyes, Has<BloodMoonBoost>)>, mut mats: Query<&mut MeshMaterial3d<StandardMaterial>>) {
    for (z, mut eyes, boost) in &mut zombies {
        let Some(def) = z.kind.class().eyes.as_ref().filter(|d| d.blood_moon) else { continue };
        if eyes.parts.is_empty() || eyes.blood_moon == boost {
            continue;
        }
        eyes.blood_moon = boost;
        let material = res.material(&mut materials, look(def, boost));
        for &p in &eyes.parts {
            if let Ok(mut m) = mats.get_mut(p) {
                m.0 = material.clone();
            }
        }
    }
}

/// Death: K2_DestroyComponent on both.
fn on_death(mut commands: Commands, mut dead: Query<&mut Eyes, Added<Dead>>) {
    for mut eyes in &mut dead {
        for p in eyes.parts.drain(..) {
            commands.entity(p).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::zombies::{ZombieKind, zombie_classes};
    use super::*;

    #[test]
    fn every_zombie_but_the_hound_has_two_eyes_in_its_material() {
        let eyes = |c: &str| ZombieKind::from_class(c).and_then(|k| k.class().eyes.clone());
        for (name, c) in zombie_classes() {
            if c.animal.is_some() || c.human.is_some() || name == "BP_Zombie_Dog_C" {
                assert!(c.eyes.is_none(), "{name}");
                continue;
            }
            let e = c.eyes.as_ref().unwrap_or_else(|| panic!("{name} has no eyes"));
            assert_eq!(e.parts.len(), 2, "{name}");
            assert!(e.parts.iter().all(|p| p.socket.bone == "eyes"), "{name}");
            // The bosses aren't BP_MasterZombie's children: no Event_BloodMoon.
            assert_eq!(e.blood_moon, c.boss.is_none(), "{name}");
        }
        let roamer = eyes("BP_Zombie_Roamer_C").unwrap();
        assert_eq!((roamer.material.as_str(), roamer.look.strength), ("M_GlowingEyes_Roamer", 10.0));
        assert_eq!(eyes("BP_Zombie_Crawler_C").unwrap().material, "M_GlowingEyes_Roamer");
        assert_eq!(eyes("BP_Zombie_Infected_Horde_C").unwrap().material, "M_GlowingEyes_Infected");
        assert_eq!(eyes("BP_Zombie_Radiated_C").unwrap().material, "M_GlowingEyes_Radiated");
        assert_eq!(eyes("BP_ZombieBoss_Military_C").unwrap().material, "M_GlowingEyes_Boss");
        assert_eq!(eyes("BP_ZombieBossHorde_Military_C").unwrap().material, "M_GlowingEyes_Infected");
        assert!(eyes("BP_Zombie_Dog_C").is_none());
    }

    #[test]
    fn the_eyes_sit_either_side_of_the_bone_and_are_a_few_cm() {
        let roamer = ZombieKind::from_class("BP_Zombie_Roamer_C").unwrap().class().eyes.clone().unwrap();
        let [l, r] = [&roamer.parts[0], &roamer.parts[1]].map(eye_transform);
        // eyesleft / eyesright: 4.2 cm either way along the bone's X.
        assert!((l.translation.x - 0.042).abs() < 0.002 && (r.translation.x + 0.041).abs() < 0.002, "{l:?} {r:?}");
        assert!(l.translation.distance(r.translation) < 0.1);
        // The Roamer's own Scale3D (3 x 2.5 x 1 cm) on the 1 m cube.
        assert!(l.scale.abs_diff_eq(Vec3::new(0.03, 0.01, 0.025), 1e-5), "{:?}", l.scale);
        let boss = ZombieKind::from_class("BP_ZombieBoss_Military_C").unwrap().class().eyes.clone().unwrap();
        assert!((eye_transform(&boss.parts[0]).translation.x - 0.051).abs() < 0.002);
    }

    #[test]
    fn the_blood_moon_turns_the_zombies_eyes_red_but_not_the_bosses() {
        let roamer = ZombieKind::from_class("BP_Zombie_Roamer_C").unwrap().class().eyes.clone().unwrap();
        let boss = ZombieKind::from_class("BP_ZombieBoss_Military_C").unwrap().class().eyes.clone().unwrap();
        let red = blood_moon_eyes().expect("M_GlowingEyes_BloodMoon");
        assert_eq!(red.strength, 150.0);
        assert_eq!(look(&roamer, true), red);
        assert_eq!(look(&roamer, false), roamer.look);
        assert_eq!(look(&boss, true), boss.look);
        // Colour x EmissiveStrength: the roamer's orange.
        let c = emissive(roamer.look).to_linear();
        assert!((c.red - 10.0).abs() < 1e-4 && (c.green - 5.02887).abs() < 1e-3 && c.blue == 0.0);
    }

    #[test]
    fn the_setting_takes_them_away() {
        let mut store = SettingsStore::default();
        assert!(!disabled(&store), "DISABLE ZOMBIE EYES? defaults to off");
        store.values.insert(row_key("settings", "Gameplay", DISABLE), Value::Bool(true));
        assert!(disabled(&store));
    }
}
