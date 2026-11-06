//! The barber: `Blueprints/Other/Objects/BP_Barber` (2 in the level) and
//! `UI/Widgets/Player/CharacterBarberMenu` (decompiled).
//!
//! - BeginPlay: RandomBool picks the look: SK_Chr_Musician_Female_01 with Chr_FemaleHair5 in
//!   MI_CharacterHair_Blonde and SM_Chr_Attach_Glasses_01, or SK_Chr_Hunter_Male_01 with
//!   Chr_MaleHair3 in MI_CharacterHair_Black and no glasses. The SkeletalMesh plays A_Idle
//!   (AnimationSingleNode); Hair and Glasses hang on `head`, Object (SM_Prop_Barber_Clippers_01)
//!   on `Hand_R`, at their components' relative transforms. AIOSubject: hidden past its last
//!   layer (7500 cm).
//! - ShowInteractWidget(true, "Barber", GetInteractOptions: Option1 "Interact").
//! - OnExecuteInteract: SetViewTargetWithBlend(the barber, 0.5 s, linear), so its Camera
//!   component ((0, 40, 150) cm, yaw 90) becomes the view; SetPlayerGearVisibility(true) hides the
//!   headwear and facewear; InGameUI hidden; CharacterBarberMenu (Chr, SkipAnim?) added with UI
//!   input and the cursor; the survivor is put at the Debug billboard ((-65, 170, 0), yaw -75).
//! - CharacterBarberMenu: the creator's Details / Body column (title "Barber", an Exit button at
//!   the bottom right); every change goes straight onto the character and its save (SkinColor
//!   re-materials the body parts, Sex's UpdateClothing swaps the head and body meshes, the hair /
//!   beard / eyebrows / mouth / accessories their meshes); a non-numeric age says "Has to be
//!   numeric" and empties the box.
//! - Exit (ExitMenu): SetViewTargetWithBlend(the survivor, 0.5 s), the gear and HUD back, game
//!   input.
//!
//! Ours: the dropdowns are the creator's < > cyclers (as on our creator page); Esc leaves too
//! (a focused text box lets go first); the profile change rebuilds the survivor through
//! `models::restyle` (new parts shown once ready); SetPlayerGearVisibility is `head_gear`'s (the
//! open Barber overlay hides the hat and mask); the outline on its SkeletalMesh is `outline.rs`'s.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use rand::Rng;

use super::interact::{GameOverlay, Prompt, PromptBar, PromptText};
use crate::level::KinematicBody;
use crate::level::item_spawners::component_world;
use crate::menus::creator::{TextField, TextFocus, refresh_creator, spawn_profile_panel, type_into_field};
use crate::menus::widgets::{ButtonPalette, MenuFont, button, label};
use crate::menus::{AppState, CurrentGame, MenuData, MenuScreen};
use crate::models::{AnimSet, Animated, CharacterModel, Locomotion, MaterialOverride, model, ue_relative};

pub const CLASS: &str = "BP_Barber_C";
/// AIOSubject's last OptimizationLayer (7500 cm).
const SHOWN_WITHIN: f32 = 75.0;
/// The interaction reach (ours, as the radios').
const REACH: f32 = 2.4;
/// SetViewTargetWithBlend's BlendTime.
pub const BLEND: f32 = 0.5;
/// The Camera and Debug components (UE cm, yaw).
const CAMERA: (Vec3, f32) = (Vec3::new(0.0, 40.0, 150.0), 90.0);
const SEAT: (Vec3, f32) = (Vec3::new(-65.0, 170.0, 0.0), -75.0);
const PROMPT: &str = "Barber: [F] Interact";

/// A barber in the level.
#[derive(Clone, Debug)]
pub struct Site {
    pub actor: usize,
    pub root: Transform,
    /// Its Camera component as a Bevy camera transform.
    pub camera: Transform,
    /// The Debug point: where the survivor is put (feet, facing).
    pub seat: Transform,
    pub female: bool,
    entity: Option<Entity>,
}

/// The view's blend into (or out of) a barber's camera.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub site: usize,
    from: Transform,
    pub t: f32,
    pub closing: bool,
}

#[derive(Resource, Default)]
pub struct Barbers {
    pub list: Vec<Site>,
    collected: bool,
    pub view: Option<View>,
}

/// The menu's root.
#[derive(Component)]
pub struct BarberUi;
#[derive(Component)]
struct ExitButton;
/// A barber's root (visibility by distance).
#[derive(Component)]
struct BarberBody;
/// A static mesh waiting for a bone of the barber's body.
#[derive(Component)]
struct OnBone(Entity, &'static str, Transform);

/// The barber's systems (the tour's step runs before them).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BarberSet;

pub fn plugin(app: &mut App) {
    app.init_resource::<Barbers>()
        .add_systems(OnEnter(AppState::InGame), |mut b: ResMut<Barbers>| *b = Barbers::default())
        .add_systems(Update, (collect, attach, show_near).chain().run_if(in_state(AppState::InGame)))
        .add_systems(
            Update,
            interact.after(crate::level::doors::interact_doors).in_set(BarberSet).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)),
        )
        // Esc after escape_goes_back, which leaves an open overlay alone (else the same press would
        // also open the pause menu).
        .add_systems(
            Update,
            (open_and_close, escape.after(crate::menus::escape_goes_back), type_into_field, commit_age, refresh_creator).chain().in_set(BarberSet).run_if(in_state(AppState::InGame)),
        )
        .add_systems(Update, restyle_survivor.before(super::interact::dress_player).run_if(in_state(AppState::InGame)))
        .add_systems(PostUpdate, blend_camera.before(bevy::transform::TransformSystems::Propagate).run_if(in_state(AppState::InGame)))
        .add_observer(exit_pressed);
}

/// The Camera component's world transform, looking along its X.
pub fn camera_transform(a: &crate::level::ActorInfo) -> Transform {
    let t = component_world(a, CAMERA.0, CAMERA.1);
    Transform::from_translation(t.translation).looking_to(t.rotation * Vec3::X, Vec3::Y)
}

/// The Debug point: the survivor's feet there, facing its X (our survivor model faces +Z).
pub fn seat_transform(a: &crate::level::ActorInfo) -> Transform {
    let t = component_world(a, SEAT.0, SEAT.1);
    let f = t.rotation * Vec3::X;
    Transform::from_translation(t.translation).with_rotation(Quat::from_rotation_y(f.x.atan2(f.z)))
}

/// A linear view blend (VTBlend_Linear).
pub fn blend(from: &Transform, to: &Transform, alpha: f32) -> Transform {
    let a = alpha.clamp(0.0, 1.0);
    Transform::from_translation(from.translation.lerp(to.translation, a)).with_rotation(from.rotation.slerp(to.rotation, a))
}

fn collect(mut commands: Commands, assets: Res<AssetServer>, level: Option<Res<crate::level::Level>>, mut barbers: ResMut<Barbers>) {
    let Some(level) = level else { return };
    if barbers.collected {
        return;
    }
    barbers.collected = true;
    let mut rng = rand::thread_rng();
    for (i, a) in level.actors.iter().enumerate().filter(|(_, a)| a.class == CLASS) {
        let female = rng.gen_bool(0.5);
        let root = crate::level::affine(&a.transform);
        let e = commands.spawn((root, Visibility::Hidden, BarberBody, DespawnOnExit(AppState::InGame))).id();
        let (mesh, hair, hair_mat) = if female {
            ("SurrounDead/Content/Meshes/PlayerModels/NotInUseATM/SK_Chr_Musician_Female_01", "Chr_FemaleHair5", "MI_CharacterHair_Blonde")
        } else {
            ("SurrounDead/Content/Meshes/PlayerModels/SK_Chr_Hunter_Male_01", "Chr_MaleHair3", "MI_CharacterHair_Black")
        };
        let body = commands.spawn((model(&assets, format!("game/{mesh}.glb")), Transform::default(), Animated(AnimSet::Trader), Locomotion::Idle, ChildOf(e))).id();
        // Hair: P90 Y345.96 R255.96 = the creator's P90 R-90 (pitch 90 leaves only yaw - roll).
        commands.spawn((
            model(&assets, format!("game/SurrounDead/Content/PlayerModel/Hair/HairMesh/{hair}.glb")),
            MaterialOverride(hair_mat.into()),
            Visibility::Hidden,
            OnBone(body, "head", ue_relative([0.0; 3], 90.0, 0.0, -90.0)),
            ChildOf(e),
        ));
        if female {
            commands.spawn((
                model(&assets, "game/SurrounDead/Content/PolygonFiles/PolygonShops/Meshes/Characters/Character_Attachments/SM_Chr_Attach_Glasses_01.glb"),
                Visibility::Hidden,
                OnBone(body, "head", ue_relative([0.0, 0.0, -1.0], 0.0, 0.0, -90.0)),
                ChildOf(e),
            ));
        }
        commands.spawn((
            model(&assets, "game/SurrounDead/Content/PolygonFiles/PolygonShops/Meshes/Props/SM_Prop_Barber_Clippers_01.glb"),
            Visibility::Hidden,
            OnBone(body, "Hand_R", ue_relative([-13.372741, -3.7821085, -8.173441], -1.7279409, 9.851076, 99.851074)),
            ChildOf(e),
        ));
        barbers.list.push(Site { actor: i, root, camera: camera_transform(a), seat: seat_transform(a), female, entity: Some(e) });
    }
    info!("barbers: {}", barbers.list.len());
}

fn attach(mut commands: Commands, pending: Query<(Entity, &OnBone)>, children: Query<&Children>, names: Query<&Name>) {
    for (e, OnBone(body, bone, at)) in &pending {
        let Some(b) = children.iter_descendants(*body).find(|c| names.get(*c).is_ok_and(|n| n.as_str().eq_ignore_ascii_case(bone))) else { continue };
        commands.entity(e).remove::<OnBone>().insert((*at, Visibility::Inherited));
        commands.entity(b).add_child(e);
    }
}

fn show_near(barbers: Res<Barbers>, player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>, mut vis: Query<&mut Visibility, With<BarberBody>>) {
    let Ok(p) = player.single() else { return };
    for s in &barbers.list {
        if let Some(mut v) = s.entity.and_then(|e| vis.get_mut(e).ok()) {
            v.set_if_neq(if s.root.translation.distance(p.translation) <= SHOWN_WITHIN { Visibility::Inherited } else { Visibility::Hidden });
        }
    }
}

/// "Barber: [F] Interact" within reach; F sits the survivor in the chair and opens the menu.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn interact(
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<GameOverlay>,
    driving: Res<super::vehicles::Driving>,
    mut barbers: ResMut<Barbers>,
    mut player: Query<(&mut Transform, &mut KinematicBody, &mut Locomotion), (With<CharacterModel>, Without<super::PlayerCamera>)>,
    camera: Query<&Transform, With<super::PlayerCamera>>,
    mut prompt: Query<&mut Visibility, With<Prompt>>,
    mut text: Query<&mut Text, With<PromptText>>,
    mut bar: Query<&mut Node, With<PromptBar>>,
    mut highlight: ResMut<super::outline::Highlight>,
) {
    let (Ok((mut me, mut body, mut loco)), Ok(mut vis)) = (player.single_mut(), prompt.single_mut()) else { return };
    if overlay.is_open() || *vis != Visibility::Hidden || driving.active() {
        return;
    }
    let p = me.translation;
    let Some(i) = barbers.list.iter().enumerate().filter(|(_, s)| s.root.translation.distance(p) <= REACH).min_by(|a, b| a.1.root.translation.distance(p).total_cmp(&b.1.root.translation.distance(p))).map(|(i, _)| i) else {
        return;
    };
    *vis = Visibility::Inherited;
    // Its SkeletalMesh (not the hair, glasses or clippers).
    if let Some(e) = barbers.list[i].entity {
        highlight.claim(super::outline::Highlit::Entities { roots: vec![e], skinned: true });
    }
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(0.0);
    }
    if let Ok(mut t) = text.single_mut() {
        t.0 = PROMPT.into();
    }
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let seat = barbers.list[i].seat;
    *me = seat;
    body.vertical_velocity = 0.0;
    // Movement doesn't run under the menu: the survivor idles there (not in whatever it was doing).
    loco.set_if_neq(Locomotion::Idle);
    let from = camera.single().copied().unwrap_or(barbers.list[i].camera);
    barbers.view = Some(View { site: i, from, t: 0.0, closing: false });
    *overlay = GameOverlay::Barber(i);
    *vis = Visibility::Hidden;
    info!("barber {i}: sat at {:.1}", seat.translation);
}

/// The menu, the HUD and the gear follow the overlay; leaving starts the blend back.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn open_and_close(
    mut commands: Commands,
    overlay: Res<GameOverlay>,
    font: Res<MenuFont>,
    data: Res<MenuData>,
    mut game: ResMut<CurrentGame>,
    mut barbers: ResMut<Barbers>,
    mut focus: ResMut<TextFocus>,
    uis: Query<Entity, With<BarberUi>>,
    mut hud: Query<&mut Visibility, With<super::hud::HudRoot>>,
    mut last: Local<Option<usize>>,
) {
    let now = match *overlay {
        GameOverlay::Barber(i) => Some(i),
        _ => None,
    };
    if now == *last {
        return;
    }
    *last = now;
    for mut v in &mut hud {
        *v = if now.is_some() { Visibility::Hidden } else { Visibility::Inherited };
    }
    focus.0 = None;
    match now {
        Some(_) => {
            let profile = game.profile.get_or_insert_with(|| crate::menus::data::CharacterProfile::new(&data.creator)).clone();
            let root = commands
                .spawn((
                    Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                    GlobalZIndex(4),
                    BarberUi,
                    DespawnOnExit(AppState::InGame),
                ))
                .id();
            commands.entity(root).with_children(|r| {
                // TextBlock_44 "Barber": top centre, alignment (0.5, -0.5), the default 24.
                r.spawn((
                    Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), top: Val::Px(14.0), justify_content: JustifyContent::Center, ..default() },
                    children![label(&font, "Barber", 24.0)],
                ));
                // ExitButton: bottom right, alignment (1.25, 1.5); BackText "Exit" at 36.
                r.spawn((
                    button(
                        Node { position_type: PositionType::Absolute, right: Val::Px(40.0), bottom: Val::Px(24.0), padding: UiRect::axes(Val::Px(14.0), Val::Px(4.0)), ..default() },
                        ButtonPalette::BARE,
                        ExitButton,
                    ),
                    children![label(&font, "Exit", 36.0)],
                ));
            });
            spawn_profile_panel(&mut commands, &font, &data, &profile, root, false);
        }
        None => {
            uis.iter().for_each(|e| commands.entity(e).despawn());
            // The blend back starts from the barber's camera (`blend_camera`).
            if let Some(v) = barbers.view.as_mut() {
                v.t = 0.0;
                v.closing = true;
            }
            info!("barber: exit");
        }
    }
}

fn exit_pressed(ev: On<Activate>, exits: Query<(), With<ExitButton>>, mut overlay: ResMut<GameOverlay>) {
    if exits.contains(ev.entity) {
        *overlay = GameOverlay::None;
    }
}

/// Esc leaves, unless a text box has the keys (then it only lets go of them).
fn escape(keys: Res<ButtonInput<KeyCode>>, focus: Res<TextFocus>, mut overlay: ResMut<GameOverlay>) {
    if matches!(*overlay, GameOverlay::Barber(_)) && focus.0.is_none() && keys.just_pressed(KeyCode::Escape) {
        *overlay = GameOverlay::None;
    }
}

/// EditableTextBox_Age's OnTextCommitted: not numeric -> "Has to be numeric" and an empty box.
fn commit_age(overlay: Res<GameOverlay>, focus: Res<TextFocus>, mut game: ResMut<CurrentGame>, mut notify: MessageWriter<super::notify::Notify>, mut last: Local<Option<TextField>>) {
    let was = std::mem::replace(&mut *last, focus.0);
    if !matches!(*overlay, GameOverlay::Barber(_)) || was != Some(TextField::Age) || focus.0 == Some(TextField::Age) {
        return;
    }
    if let Some(p) = game.profile.as_mut()
        && !is_numeric(&p.age)
    {
        p.age.clear();
        notify.write(super::notify::Notify::new("Has to be numeric"));
    }
}

/// UE's IsNumeric: an optional sign, digits with at most one point; empty is not.
pub fn is_numeric(s: &str) -> bool {
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    !s.is_empty() && s.chars().filter(|&c| c == '.').count() <= 1 && s.chars().all(|c| c.is_ascii_digit() || c == '.') && s.chars().any(|c| c.is_ascii_digit())
}

/// The survivor's look follows the profile (the barber's live changes): rebuilt when it differs
/// from the one it was built from.
#[allow(clippy::type_complexity)]
fn restyle_survivor(
    mut commands: Commands,
    game: Res<CurrentGame>,
    data: Res<MenuData>,
    assets: Res<AssetServer>,
    player: Query<(Entity, Option<&BuiltFrom>, &Children), (With<CharacterModel>, With<KinematicBody>)>,
    heads: Query<(), With<crate::models::HeadPart>>,
    bodies: Query<(), With<crate::models::BodyPart>>,
) {
    let (Ok((e, built, kids)), Some(profile)) = (player.single(), game.profile.as_ref()) else { return };
    match built {
        None => {
            commands.entity(e).insert(BuiltFrom(profile.clone()));
        }
        Some(b) if b.0 != *profile => {
            let head = kids.iter().find(|k| heads.contains(*k));
            let body: Vec<Entity> = kids.iter().filter(|k| bodies.contains(*k)).collect();
            crate::models::restyle(&mut commands, &assets, e, head, &body, profile, &data.creator);
            commands.entity(e).insert(BuiltFrom(profile.clone()));
            let pick = |i: usize| data.creator.body[i].options.get(profile.body[i]).cloned().unwrap_or_default();
            info!("restyle: {} / {} / {}", pick(0), pick(1), pick(2));
        }
        _ => {}
    }
}

/// The profile the survivor's parts were built from.
#[derive(Component)]
pub struct BuiltFrom(pub crate::menus::data::CharacterProfile);

/// SetViewTargetWithBlend: 0.5 s linear into the barber's Camera, and back to the spring arm.
fn blend_camera(time: Res<Time>, mut barbers: ResMut<Barbers>, mut camera: Query<&mut Transform, With<super::PlayerCamera>>) {
    let Barbers { list, view, .. } = &mut *barbers;
    let Some(v) = view.as_mut() else { return };
    let (Ok(mut cam), Some(site)) = (camera.single_mut(), list.get(v.site)) else { return };
    v.t += time.delta_secs();
    let a = v.t / BLEND;
    if v.closing {
        // Towards the spring arm `camera_controls` has just placed.
        *cam = blend(&site.camera, &cam, a);
        if a >= 1.0 {
            *view = None;
        }
    } else {
        *cam = blend(&v.from, &site.camera, a);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(yaw_deg: f32, at: Vec3) -> crate::level::ActorInfo {
        // Bevy-space columns: UE X, up, UE Y (a UE yaw turns X towards Y: -yaw about +Y here).
        let r = Quat::from_rotation_y(-yaw_deg.to_radians());
        let (x, y) = (r * Vec3::X, r * Vec3::Z);
        crate::level::ActorInfo { class: CLASS.into(), name: "b".into(), transform: [x.x, x.y, x.z, 0.0, 1.0, 0.0, y.x, y.y, y.z, at.x, at.y, at.z], props: None }
    }

    #[test]
    fn the_camera_and_seat_sit_on_the_components() {
        let a = actor(0.0, Vec3::new(10.0, 2.0, 5.0));
        // Camera (0, 40, 150) yaw 90: 0.4 m along UE Y (Bevy +Z), 1.5 m up, looking along UE Y.
        let c = camera_transform(&a);
        assert!(c.translation.abs_diff_eq(Vec3::new(10.0, 3.5, 5.4), 1e-4), "{}", c.translation);
        assert!(c.forward().as_vec3().abs_diff_eq(Vec3::Z, 1e-4), "{:?}", c.forward());
        // Debug (-65, 170, 0) yaw -75: the survivor faces 75 degrees from UE X towards -Y.
        let s = seat_transform(&a);
        assert!(s.translation.abs_diff_eq(Vec3::new(9.35, 2.0, 6.7), 1e-4), "{}", s.translation);
        let f = s.rotation * Vec3::Z;
        let want = Quat::from_rotation_y(75f32.to_radians()) * Vec3::X;
        assert!(f.abs_diff_eq(want, 1e-4), "{f} vs {want}");
        // A turned barber turns both.
        let b = actor(90.0, Vec3::ZERO);
        assert!(camera_transform(&b).forward().as_vec3().abs_diff_eq(-Vec3::X, 1e-4));
    }

    #[test]
    fn the_view_blends_linearly() {
        let a = Transform::from_xyz(0.0, 0.0, 0.0);
        let b = Transform::from_xyz(2.0, 4.0, 0.0).with_rotation(Quat::from_rotation_y(1.0));
        assert_eq!(blend(&a, &b, 0.0).translation, a.translation);
        assert!(blend(&a, &b, 0.5).translation.abs_diff_eq(Vec3::new(1.0, 2.0, 0.0), 1e-5));
        assert!(blend(&a, &b, 2.0).rotation.abs_diff_eq(b.rotation, 1e-5), "clamped at the end");
    }

    #[test]
    fn age_must_be_numeric() {
        assert!(is_numeric("34") && is_numeric("-2") && is_numeric("1.5"));
        assert!(!is_numeric("") && !is_numeric("abc") && !is_numeric("1.2.3") && !is_numeric("+"));
    }
}
