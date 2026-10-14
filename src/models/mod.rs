//! Meshes exported from the game (`extractor/export_assets.sh --models`) and the modular
//! survivor assembled from them.
//!
//! Exported `.glb` files carry only material *names*; `assets/game/materials.json`
//! (`extractor/gen_materials.py`) maps each name to the atlas texture or tint the UE material
//! draws with, and [`remap_materials`] swaps those in once a model's scene has spawned.

use std::collections::HashMap;

use bevy::gltf::GltfMaterialName;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use serde::Deserialize;

use crate::menus::data::{CharacterProfile, CreatorData};

pub mod anim;

pub use anim::{ActionKind, AnimLibrary, AnimSet, Animated, Locomotion, Stance, UpperAction};

pub const PLAYER_MODEL: &str = "game/SurrounDead/Content/PlayerModel";

pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameMaterials::load())
            .init_resource::<anim::AnimLibrary>()
            .add_observer(remap_materials)
            .add_observer(anim::wire_skeleton)
            .add_systems(Update, (attach_to_bones, anim::wire_late, anim::drive_animation, swap_parts).chain());
    }
}

#[derive(Deserialize, Clone, Default)]
struct MaterialDef {
    texture: Option<String>,
    color: Option<[f32; 4]>,
    /// Synty tree materials colour zero-vertex-alpha triangles (the trunk) with this instead.
    trunk_color: Option<[f32; 4]>,
}

/// Material name -> how to draw it, plus the Bevy materials built so far.
#[derive(Resource, Default)]
pub struct GameMaterials {
    defs: HashMap<String, MaterialDef>,
    built: HashMap<String, Handle<StandardMaterial>>,
}

impl GameMaterials {
    fn load() -> Self {
        let defs = std::fs::read_to_string("assets/game/materials.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| {
                warn!("assets/game/materials.json missing; run extractor/export_assets.sh --models");
                HashMap::new()
            });
        Self { defs, built: HashMap::new() }
    }

    /// Trunk material for tree materials (see [`MaterialDef::trunk_color`]).
    pub fn trunk(&mut self, name: &str, materials: &mut Assets<StandardMaterial>) -> Option<Handle<StandardMaterial>> {
        let key = format!("{name}#trunk");
        if let Some(h) = self.built.get(&key) {
            return Some(h.clone());
        }
        let [r, g, b, a] = self.defs.get(name)?.trunk_color?;
        let h = materials.add(StandardMaterial { base_color: Color::linear_rgba(r, g, b, a), perceptual_roughness: 0.9, ..default() });
        self.built.insert(key, h.clone());
        Some(h)
    }

    /// `Mat_Clear_Water`: a translucent material whose graph isn't readable; the colour,
    /// opacity and glossiness here are chosen to read as clear lake/river water (a guess).
    pub fn water(&mut self, materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
        self.built
            .entry("Mat_Clear_Water".into())
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgba(0.16, 0.30, 0.34, 0.72),
                    perceptual_roughness: 0.06,
                    reflectance: 0.6,
                    alpha_mode: AlphaMode::Blend,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .clone()
    }

    pub fn get(&mut self, name: &str, assets: &AssetServer, materials: &mut Assets<StandardMaterial>) -> Option<Handle<StandardMaterial>> {
        if let Some(h) = self.built.get(name) {
            return Some(h.clone());
        }
        let def = self.defs.get(name)?.clone();
        let glass = name.contains("Glass");
        let base_color = match def.color {
            // A tint parameter's alpha isn't opacity (several are 0: bears, deer, wolves, swamp
            // reeds, tree branches), and with the alpha cut-off it would hide the mesh.
            Some([r, g, b, _]) => Color::linear_rgb(r, g, b),
            // Glass materials are untextured translucent shaders in UE.
            None if glass => Color::srgba(0.25, 0.3, 0.35, 0.45),
            None => Color::WHITE,
        };
        let handle = materials.add(StandardMaterial {
            base_color,
            base_color_texture: def.texture.map(|t| assets.load(t)),
            perceptual_roughness: if glass { 0.1 } else { 0.85 },
            // Synty atlases are opaque; feature decals (scars, paint) cut out at UE's 0.3333 clip.
            alpha_mode: if glass { AlphaMode::Blend } else { AlphaMode::Mask(0.3333) },
            ..default()
        });
        self.built.insert(name.to_string(), handle.clone());
        Some(handle)
    }
}

/// A glTF scene whose materials should be swapped for the game's.
#[derive(Component)]
pub struct GameModel;

/// Draw every mesh in this model with one material (skin colour, hair colour) instead of the
/// names baked into the file.
#[derive(Component, Clone)]
pub struct MaterialOverride(pub String);

/// Spawnable bundle for an exported mesh, `path` relative to `assets/` (e.g. "game/.../X.glb").
pub fn model(assets: &AssetServer, path: impl Into<String>) -> impl Bundle {
    (WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.into()))), GameModel)
}

fn remap_materials(
    ev: On<WorldInstanceReady>,
    roots: Query<Option<&MaterialOverride>, With<GameModel>>,
    children: Query<&Children>,
    mut meshes: Query<(&GltfMaterialName, &Mesh3d, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    mut lib: ResMut<GameMaterials>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(override_mat) = roots.get(ev.entity) else { return };
    for e in children.iter_descendants(ev.entity) {
        let Ok((name, mesh, mut mat, mut vis)) = meshes.get_mut(e) else { continue };
        // UE's materials here ignore vertex colour, but Bevy multiplies COLOR_0 into the base
        // colour; hair meshes carry zero vertex alpha, which the alpha cut-off then discards.
        if let Some(mut m) = mesh_assets.get_mut(&mesh.0)
            && m.contains_attribute(Mesh::ATTRIBUTE_COLOR)
        {
            m.remove_attribute(Mesh::ATTRIBUTE_COLOR);
        }
        if name.0 == "Invisible" {
            *vis = Visibility::Hidden;
            continue;
        }
        let want = override_mat.map_or(name.0.as_str(), |o| o.0.as_str());
        if let Some(h) = lib.get(want, &assets, &mut materials) {
            mat.0 = h;
        }
    }
}

/// A survivor part and how it hangs off the body.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub path: String,
    pub material: Option<String>,
    /// Static meshes `BP_CharacterCreator` attaches to a bone of the Head mesh, with the
    /// component's relative transform (glTF space); skinned parts share the body's bind space.
    pub bone: Option<(&'static str, Transform)>,
    /// Which swappable body part it is (`BODY_PARTS`).
    pub body: Option<&'static str>,
}

/// The body-part slots BP_PlayerCharacter swaps for clothing (BodyPartVisibility).
pub const BODY_PARTS: [&str; 8] = ["Torso", "Arms", "Biceps", "Hands", "Legs", "LowerThighs", "LowerLegs", "Feet"];

/// The bare body (`SetSexMesh` / `UpdateBodyParts`): package paths per part; no Biceps or
/// LowerThighs (the underwear pieces only some clothing brings back).
pub fn default_body(sex: &str) -> Vec<(&'static str, Option<String>)> {
    let bp = "SurrounDead/Content/PlayerModel/BodyParts";
    BODY_PARTS
        .iter()
        .map(|&b| {
            let mesh = match b {
                "Biceps" | "LowerThighs" => None,
                "Hands" => Some(format!("{bp}/Hands/SK_Chr_Hands")),
                _ => Some(format!("{bp}/{b}/{sex}/SK_Chr_{sex}_{b}")),
            };
            (b, mesh)
        })
        .collect()
}

/// The skin material for a profile's skin tone.
pub fn skin_material(profile: &CharacterProfile) -> String {
    format!("M_Chr_Texture_1{}", ["A", "B", "C"][profile.body[1].min(2)])
}

/// A UE relative transform (location in cm, rotator in degrees) in glTF space, where positions
/// are (X, Z, Y) / 100 and rotations conjugate by the same axis swap: quat (x, z, y, -w).
pub fn ue_relative(loc_cm: [f32; 3], pitch: f32, yaw: f32, roll: f32) -> Transform {
    // FRotator::Quaternion.
    let (sp, cp) = (pitch.to_radians() / 2.0).sin_cos();
    let (sy, cy) = (yaw.to_radians() / 2.0).sin_cos();
    let (sr, cr) = (roll.to_radians() / 2.0).sin_cos();
    let (x, y, z, w) = (cr * sp * sy - sr * cp * cy, -cr * sp * cy - sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy);
    Transform::from_translation(Vec3::new(loc_cm[0], loc_cm[2], loc_cm[1]) / 100.0).with_rotation(Quat::from_xyzw(x, z, y, -w).normalize())
}

/// `BP_CharacterCreator`'s attachments: HairMesh / BeardMesh on `head` (P90 R-90), EyebrowsMesh on
/// `eyebrows` (0.5 cm along Y, R-90), and Mouth / Accessory1-3 on `head` at (0, 156.9, 0.5) R-90,
/// which cancels the bone's bind pose (those meshes are modelled in character space).
fn hair_attach() -> (&'static str, Transform) {
    ("head", ue_relative([0.0; 3], 90.0, 0.0, -90.0))
}
fn eyebrows_attach() -> (&'static str, Transform) {
    ("eyebrows", ue_relative([0.0, 0.5, 0.0], 0.0, 0.0, -90.0))
}
fn feature_attach() -> (&'static str, Transform) {
    ("head", ue_relative([0.0, 156.9, 0.5], 0.0, 0.0, -90.0))
}

/// The survivor's mesh parts (as `BP_CharacterCreator` assembles them) for a creator profile.
pub fn character_parts(profile: &CharacterProfile, data: &CreatorData) -> Vec<Part> {
    let pick = |i: usize| data.body[i].options[profile.body[i]].as_str();
    let sex = pick(0);
    let skin = skin_material(profile);
    let hair_mat = |i: usize| format!("MI_CharacterHair_{}", pick(i).replace(' ', "").replace("Gray", "Grey"));

    let part = |path: String, material: Option<String>, bone: Option<(&'static str, Transform)>| Part { path, material, bone, body: None };
    let mut parts = vec![part(format!("{PLAYER_MODEL}/BodyParts/Head/{sex}/SK_Chr_{sex}_Head.glb"), Some(skin.clone()), None)];
    for (b, mesh) in default_body(sex) {
        if let Some(m) = mesh {
            parts.push(Part { body: Some(b), ..part(format!("game/{m}.glb"), Some(skin.clone()), None) });
        }
    }

    // "Male Hair 3" -> Chr_MaleHair3, "Beard 2" -> Chr_Beard2, "Eyebrows 1" -> Chr_Eyebrows1.
    let mesh_name = |option: &str| option.replace(' ', "");
    let hair = pick(2);
    if hair != "Bald" {
        parts.push(part(format!("{PLAYER_MODEL}/Hair/HairMesh/Chr_{}.glb", mesh_name(hair)), Some(hair_mat(3)), Some(hair_attach())));
    }
    let beard = pick(5);
    if beard != "None" {
        parts.push(part(format!("{PLAYER_MODEL}/Hair/Beard/Chr_{}.glb", mesh_name(beard)), Some(hair_mat(6)), Some(hair_attach())));
    }
    let brows = pick(7);
    if brows != "None" {
        parts.push(part(format!("{PLAYER_MODEL}/Hair/Eyebrows/Chr_{}.glb", mesh_name(brows)), Some(hair_mat(3)), Some(eyebrows_attach())));
    }
    // Mouth and the three accessory slots are head decals: Features/<Sex>/<Sex>_<Name>.
    for i in [4, 8, 9, 10] {
        let option = pick(i);
        if option != "None" {
            parts.push(part(format!("{PLAYER_MODEL}/BodyParts/Head/Features/{sex}/{sex}_{}.glb", mesh_name(option)), None, Some(feature_attach())));
        }
    }
    parts
}

/// Marks the root of an assembled survivor; parts are its children.
#[derive(Component)]
pub struct CharacterModel;

/// The Head part, whose skeleton carries the `head` / `eyebrows` bones attachments hang from.
#[derive(Component)]
struct HeadPart;

/// A part waiting for its bone to exist (the Head scene spawns asynchronously).
#[derive(Component)]
struct AttachToBone(&'static str, Transform);

pub fn spawn_character(commands: &mut Commands, assets: &AssetServer, profile: &CharacterProfile, data: &CreatorData, root: impl Bundle) -> Entity {
    let id = commands.spawn((root, CharacterModel, Visibility::default(), Animated(AnimSet::Survivor), Locomotion::Idle, Stance::Unarmed, UpperAction::default())).id();
    for (i, p) in character_parts(profile, data).into_iter().enumerate() {
        let mut part = commands.spawn((model(assets, p.path), Transform::default()));
        if let Some(m) = p.material {
            part.insert(MaterialOverride(m));
        }
        if i == 0 {
            part.insert(HeadPart);
        }
        if let Some(b) = p.body {
            part.insert(BodyPart(b));
        }
        if let Some((bone, rel)) = p.bone {
            part.insert((AttachToBone(bone, rel), Visibility::Hidden));
        }
        let part = part.id();
        commands.entity(id).add_child(part);
    }
    id
}

/// Hang hair, beard, eyebrows and face features off their Head bone once its skeleton has
/// spawned, at the blueprint component's relative transform.
fn attach_to_bones(
    mut commands: Commands,
    pending: Query<(Entity, &AttachToBone, &ChildOf)>,
    heads: Query<(Entity, &ChildOf), With<HeadPart>>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (part, attach, part_parent) in &pending {
        let Some((head, _)) = heads.iter().find(|(_, p)| p.parent() == part_parent.parent()) else { continue };
        let Some(bone) = children.iter_descendants(head).find(|e| names.get(*e).is_ok_and(|n| n.as_str() == attach.0)) else { continue };
        commands.entity(part).remove::<AttachToBone>().insert((attach.1, Visibility::Inherited));
        commands.entity(bone).add_child(part);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_clothes_show_before_old_ones_go() {
        let mut app = App::new();
        app.init_resource::<Time>().add_systems(Update, swap_parts);
        let body = app.world_mut().spawn_empty().id();
        let old = app.world_mut().spawn((Outgoing, ChildOf(body))).id();
        let new = app.world_mut().spawn((Incoming::default(), Visibility::Hidden, ChildOf(body))).id();
        // Its scene hasn't spawned: the old part stays, the new one hidden.
        for _ in 0..5 {
            app.update();
        }
        assert!(app.world().get_entity(old).is_ok());
        assert_eq!(app.world().get::<Visibility>(new), Some(&Visibility::Hidden));
        app.world_mut().spawn(ChildOf(new));
        for _ in 0..REVEAL_AFTER {
            app.update();
            assert!(app.world().get_entity(old).is_ok());
        }
        app.update();
        app.update();
        assert_eq!(app.world().get::<Visibility>(new), Some(&Visibility::Inherited));
        assert!(app.world().get_entity(old).is_err());
    }
    use crate::menus::data::MenuData;

    /// Bind-pose world matrix of a node in a glb (its parent chain composed).
    fn glb_node_world(path: &str, name: &str) -> Option<Mat4> {
        let bytes = std::fs::read(path).ok()?;
        let len = u32::from_le_bytes(bytes[12..16].try_into().ok()?) as usize;
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + len]).ok()?;
        let nodes = json["nodes"].as_array()?;
        let mut parent = std::collections::HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            for c in n["children"].as_array().into_iter().flatten() {
                parent.insert(c.as_u64()? as usize, i);
            }
        }
        let v = |n: &serde_json::Value, k: &str, d: &[f32]| -> Vec<f32> { n[k].as_array().map_or(d.to_vec(), |a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()) };
        let local = |i: usize| {
            let (t, r, s) = (v(&nodes[i], "translation", &[0.0; 3]), v(&nodes[i], "rotation", &[0.0, 0.0, 0.0, 1.0]), v(&nodes[i], "scale", &[1.0; 3]));
            Mat4::from_scale_rotation_translation(Vec3::from_slice(&s), Quat::from_slice(&r), Vec3::from_slice(&t))
        };
        let mut i = nodes.iter().position(|n| n["name"] == name)?;
        let mut m = local(i);
        while let Some(&p) = parent.get(&i) {
            i = p;
            m = local(i) * m;
        }
        Some(m)
    }

    #[test]
    fn attachments_sit_where_the_blueprint_puts_them() {
        let head = "assets/game/SurrounDead/Content/PlayerModel/BodyParts/Head/Male/SK_Chr_Male_Head.glb";
        if !std::path::Path::new(head).exists() {
            return;
        }
        let world = |(bone, rel): (&str, Transform)| glb_node_world(head, bone).unwrap() * rel.to_matrix();
        // Hair hangs off `head` turned to face +Z (what the old hand-tuned rotation did).
        let hair = world(hair_attach());
        assert!(hair.transform_vector3(Vec3::X).abs_diff_eq(Vec3::Z, 1e-3), "{hair}");
        // Face features cancel the head's bind pose: identity in character space.
        assert!(world(feature_attach()).abs_diff_eq(Mat4::IDENTITY, 1e-3));
        // Eyebrows are modelled in their bone's frame upright and facing forward, above the eyes.
        let brows = world(eyebrows_attach());
        assert!(brows.transform_vector3(Vec3::Y).abs_diff_eq(Vec3::Y, 1e-3) && brows.transform_vector3(Vec3::Z).abs_diff_eq(Vec3::Z, 1e-3), "{brows}");
        assert!((1.65..1.75).contains(&brows.w_axis.y) && brows.w_axis.z > 0.1, "{brows}");
    }

    #[test]
    fn every_default_and_variant_part_exists() {
        let data: MenuData = serde_json::from_str(&std::fs::read_to_string("assets/data/menus.json").unwrap()).unwrap();
        if !std::path::Path::new("assets/game/SurrounDead/Content/PlayerModel").exists() {
            return; // assets not exported on this machine
        }
        let mut profile = CharacterProfile::new(&data.creator);
        // Walk every option of every body row once.
        for row in 0..data.creator.body.len() {
            for opt in 0..data.creator.body[row].options.len() {
                profile.body[row] = opt;
                for part in character_parts(&profile, &data.creator) {
                    assert!(std::path::Path::new("assets").join(&part.path).exists(), "missing {}", part.path);
                }
            }
            profile = CharacterProfile::new(&data.creator);
        }
    }
}


/// Split a triangle mesh into (leaves, trunk) by vertex alpha: Synty trees leave trunk vertices
/// at zero alpha (leaves carry wind weights there). `None` if the mesh has no vertex colours or
/// either side would be empty.
pub fn split_by_vertex_alpha(mesh: &Mesh) -> Option<(Mesh, Mesh)> {
    use bevy::mesh::{Indices, VertexAttributeValues};
    let alpha: Vec<f32> = match mesh.attribute(Mesh::ATTRIBUTE_COLOR)? {
        VertexAttributeValues::Float32x4(c) => c.iter().map(|c| c[3]).collect(),
        VertexAttributeValues::Unorm8x4(c) => c.iter().map(|c| c[3] as f32 / 255.0).collect(),
        VertexAttributeValues::Uint16x4(c) | VertexAttributeValues::Unorm16x4(c) => c.iter().map(|c| c[3] as f32 / 65535.0).collect(),
        _ => return None,
    };
    let idx: Vec<u32> = match mesh.indices()? {
        Indices::U16(i) => i.iter().map(|&x| x as u32).collect(),
        Indices::U32(i) => i.clone(),
    };
    let (mut leaves, mut trunk) = (Vec::new(), Vec::new());
    for tri in idx.chunks_exact(3) {
        let is_trunk = tri.iter().all(|&v| alpha[v as usize] <= 0.0);
        if is_trunk { &mut trunk } else { &mut leaves }.extend_from_slice(tri);
    }
    if leaves.is_empty() || trunk.is_empty() {
        return None;
    }
    let with = |i: Vec<u32>| {
        let mut m = mesh.clone();
        m.insert_indices(Indices::U32(i));
        m.remove_attribute(Mesh::ATTRIBUTE_COLOR);
        m
    };
    Some((with(leaves), with(trunk)))
}

/// Clothing currently drawn on a survivor (slot name, mesh path), so it's only respawned on change.
#[derive(Component, Default, PartialEq)]
pub struct WornClothing(pub Vec<(String, String)>);

/// A swappable body part (one of `BODY_PARTS`) under a [`CharacterModel`].
#[derive(Component)]
pub struct BodyPart(pub &'static str);

/// The body-part meshes currently on a survivor (part, package path or hidden).
#[derive(Component, Default, PartialEq, Debug, Clone)]
pub struct BodyMeshes(pub Vec<(&'static str, Option<String>)>);

/// Swap a survivor's body parts from `have` to `want` (BodyPartVisibility): changed parts are
/// despawned and respawned with the skin, hidden ones just go.
pub fn set_body(commands: &mut Commands, assets: &AssetServer, character: Entity, parts: &[(Entity, &'static str)], have: &BodyMeshes, want: &BodyMeshes, skin: &str) {
    for (b, mesh) in &want.0 {
        if have.0.iter().any(|(h, m)| h == b && m == mesh) {
            continue;
        }
        for (e, _) in parts.iter().filter(|(_, p)| p == b) {
            commands.entity(*e).insert(Outgoing).remove::<BodyPart>();
        }
        if let Some(m) = mesh {
            let part = commands.spawn((model(assets, format!("game/{m}.glb")), Transform::default(), MaterialOverride(skin.to_string()), BodyPart(b), Incoming::default(), Visibility::Hidden)).id();
            commands.entity(character).add_child(part);
        }
    }
}

/// A clothing part spawned under a [`CharacterModel`].
#[derive(Component)]
pub struct ClothingPart;

/// Spawn clothing meshes (skinned to the shared body skeleton, so they animate with it) as
/// parts of a survivor. `meshes` are package paths without extension.
pub fn dress(commands: &mut Commands, assets: &AssetServer, character: Entity, old: &[Entity], meshes: &[String]) {
    for e in old {
        commands.entity(*e).insert(Outgoing).remove::<ClothingPart>();
    }
    for m in meshes {
        let part = commands.spawn((model(assets, format!("game/{m}.glb")), Transform::default(), ClothingPart, Incoming::default(), Visibility::Hidden)).id();
        commands.entity(character).add_child(part);
    }
}

/// A part being put on: hidden until its scene is in place and has been animated, so a change
/// of clothes never shows a part in its bind (T) pose.
#[derive(Component, Default)]
pub struct Incoming {
    /// Frames since its scene spawned.
    frames: u8,
    /// Seconds waited (a file that never loads gives up and shows nothing new).
    waited: f32,
}

/// A part being taken off: it stays until the character's incoming parts are shown.
#[derive(Component)]
pub struct Outgoing;

/// Frames an incoming part waits after its scene spawns: one for the animation wiring, one for
/// the first posed frame.
const REVEAL_AFTER: u8 = 2;

/// Show incoming parts once ready, then drop the parts they replace.
fn swap_parts(
    mut commands: Commands,
    time: Res<Time>,
    mut incoming: Query<(Entity, &mut Incoming, &mut Visibility, Option<&Children>, &ChildOf)>,
    outgoing: Query<(Entity, &ChildOf), With<Outgoing>>,
) {
    let mut waiting = std::collections::HashSet::new();
    for (e, mut inc, mut vis, kids, parent) in &mut incoming {
        inc.waited += time.delta_secs();
        if kids.is_some_and(|k| !k.is_empty()) {
            inc.frames = inc.frames.saturating_add(1);
        }
        if inc.frames > REVEAL_AFTER || inc.waited > 3.0 {
            *vis = Visibility::Inherited;
            commands.entity(e).remove::<Incoming>();
        } else {
            waiting.insert(parent.parent());
        }
    }
    for (e, parent) in &outgoing {
        if !waiting.contains(&parent.parent()) {
            commands.entity(e).despawn();
        }
    }
}
