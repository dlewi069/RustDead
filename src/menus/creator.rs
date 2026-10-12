//! `CharacterCreatorMenu`: Details (forename, surname, age, prior occupation) and Body (sex,
//! skin, hair, mouth, beard, eyebrows, accessories), then Continue into the world.
//!
//! The original shows `BP_CharacterCreator` (the modular player mesh) on the right with a
//! "Rotate Character" slider; that arrives with the player model import.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::data::{CharacterProfile, MenuData};
use crate::models::{CharacterModel, spawn_character};
use bevy::ui::Pressed;
use super::widgets::*;
use super::{AppState, CurrentGame, GotoScreen, MenuScreen};
use crate::engine::GameState;

pub struct CreatorPlugin;

impl Plugin for CreatorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TextFocus>()
            .init_resource::<PreviewLocomotion>()
            .add_systems(OnEnter(MenuScreen::Creator), (spawn_creator, spawn_preview_stage))
            .add_systems(OnExit(MenuScreen::Creator), |mut f: ResMut<TextFocus>| f.0 = None)
            .add_systems(
                Update,
                (type_into_field, refresh_creator, rebuild_preview, rotate_preview, preview_locomotion).run_if(in_state(MenuScreen::Creator)),
            )
            .add_observer(on_creator);
    }
}

/// The text box currently receiving keystrokes (Esc/Enter releases it instead of leaving the page).
#[derive(Resource, Default)]
pub struct TextFocus(pub Option<TextField>);

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextField {
    Forename,
    Surname,
    Age,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cycler {
    Occupation,
    Body(usize),
}

#[derive(Component, Clone, Copy)]
enum CreatorAction {
    Focus(TextField),
    Step(Cycler, i32),
    Continue,
}

/// Text showing a field's current value.
#[derive(Component, Clone, Copy)]
enum CreatorValue {
    Text(TextField),
    Cycler(Cycler),
}

fn spawn_creator(mut commands: Commands, font: Res<MenuFont>, data: Res<MenuData>, mut game: ResMut<CurrentGame>) {
    let font = &*font;
    let profile = game.profile.get_or_insert_with(|| CharacterProfile::new(&data.creator)).clone();
    let root = commands.spawn((screen(MenuScreen::Creator), GlobalZIndex(4))).id();

    commands.entity(root).with_children(|r| {
        r.spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), top: Val::Px(16.0), justify_content: JustifyContent::Center, ..default() },
            children![label(font, "Customize Character", 24.0)],
        ));
        let corner = |right: f32| Node { position_type: PositionType::Absolute, right: Val::Px(right), bottom: Val::Px(24.0), padding: UiRect::axes(Val::Px(14.0), Val::Px(4.0)), ..default() };
        r.spawn((button(corner(40.0), ButtonPalette::BARE, GotoScreen(MenuScreen::NewGame)), children![label(font, "Back", 36.0)]));
        r.spawn((button(corner(220.0), ButtonPalette::BARE, CreatorAction::Continue), children![label(font, "Continue", 36.0)]));
        // "Rotate Character" slider (bottom-right, 500 wide); held arrows spin the preview.
        r.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..anchored(Vec2::ONE, Vec2::new(-115.0, -125.0), Vec2::new(500.0, 70.0), Vec2::ONE)
            },
            children![
                label(font, "Rotate Character", 14.0),
                (
                    Node { column_gap: Val::Px(12.0), ..default() },
                    children![
                        (button(Node { width: Val::Px(60.0), height: Val::Px(32.0), justify_content: JustifyContent::Center, ..default() }, ButtonPalette::MENU, RotateButton(-1.0)), children![label(font, "<", 16.0)]),
                        (button(Node { width: Val::Px(60.0), height: Val::Px(32.0), justify_content: JustifyContent::Center, ..default() }, ButtonPalette::MENU, RotateButton(1.0)), children![label(font, ">", 16.0)]),
                    ],
                ),
            ],
        ));
    });

    // Left column: 1200 wide from y=60, a 50% black border inset 25px with 20px padding.
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(25.0),
                top: Val::Px(85.0),
                bottom: Val::Px(25.0),
                width: Val::Px(1150.0),
                padding: UiRect::all(Val::Px(20.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        ))
        .id();
    commands.entity(root).add_child(panel);

    let header = |commands: &mut Commands, text: &str| {
        let h = commands.spawn((label(font, text, 22.0), Node { margin: UiRect::vertical(Val::Px(8.0)), ..default() })).id();
        commands.entity(panel).add_child(h);
    };
    let line = |commands: &mut Commands, text: &str| {
        let l = commands
            .spawn((
                Node { align_items: AlignItems::Center, column_gap: Val::Px(12.0), min_height: Val::Px(40.0), ..default() },
                children![(label(font, text, 16.0), Node { width: Val::Px(220.0), ..default() })],
            ))
            .id();
        commands.entity(panel).add_child(l);
        l
    };

    header(&mut commands, "Details");
    for (text, field, value) in [
        ("Forename:", TextField::Forename, &profile.forename),
        ("Surname:", TextField::Surname, &profile.surname),
        ("Age:", TextField::Age, &profile.age),
    ] {
        let l = line(&mut commands, text);
        let b = commands
            .spawn((
                button(
                    Node { width: Val::Px(420.0), height: Val::Px(36.0), padding: UiRect::horizontal(Val::Px(10.0)), align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.0)), ..default() },
                    ButtonPalette::MENU,
                    CreatorAction::Focus(field),
                ),
                BorderColor::all(Color::WHITE.with_alpha(0.4)),
                field,
                children![(label(font, value.clone(), 16.0), CreatorValue::Text(field))],
            ))
            .id();
        commands.entity(l).add_child(b);
    }
    let occupation = line(&mut commands, "Prior Occupation:");
    spawn_cycler(&mut commands, font, occupation, Cycler::Occupation, data.creator.occupations[profile.occupation].clone());

    header(&mut commands, "Body");
    for (i, row) in data.creator.body.iter().enumerate() {
        let l = line(&mut commands, &row.label);
        spawn_cycler(&mut commands, font, l, Cycler::Body(i), row.options[profile.body[i]].clone());
    }
    let note = commands
        .spawn((label_colored(font, "If changing sex, you will need to change accessory type again.", 12.0, TEXT_DIM), Node { margin: UiRect::top(Val::Px(10.0)), ..default() }))
        .id();
    commands.entity(panel).add_child(note);
}

fn spawn_cycler(commands: &mut Commands, font: &MenuFont, parent: Entity, which: Cycler, value: String) {
    let arrow = |text: &'static str, delta: i32| {
        (
            button(
                Node { width: Val::Px(36.0), height: Val::Px(36.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                ButtonPalette::MENU,
                CreatorAction::Step(which, delta),
            ),
            children![label(font, text, 16.0)],
        )
    };
    let l = commands.spawn(arrow("<", -1)).id();
    let v = commands
        .spawn((Node { width: Val::Px(340.0), justify_content: JustifyContent::Center, ..default() }, children![(label(font, value, 16.0), CreatorValue::Cycler(which))]))
        .id();
    let r = commands.spawn(arrow(">", 1)).id();
    commands.entity(parent).add_children(&[l, v, r]);
}

fn on_creator(
    ev: On<Activate>,
    actions: Query<&CreatorAction>,
    data: Res<MenuData>,
    mut focus: ResMut<TextFocus>,
    mut game: ResMut<CurrentGame>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_screen: ResMut<NextState<MenuScreen>>,
) {
    let Ok(action) = actions.get(ev.entity) else { return };
    match *action {
        CreatorAction::Focus(field) => focus.0 = Some(field),
        CreatorAction::Step(which, delta) => {
            focus.0 = None;
            let Some(profile) = game.profile.as_mut() else { return };
            let (index, len) = match which {
                Cycler::Occupation => (&mut profile.occupation, data.creator.occupations.len()),
                Cycler::Body(i) => (&mut profile.body[i], data.creator.body[i].options.len()),
            };
            *index = (*index as i32 + delta).rem_euclid(len as i32) as usize;
        }
        CreatorAction::Continue => {
            let Some(profile) = game.profile.clone() else { return };
            if profile.age.parse::<u32>().is_err() {
                // The original rejects this with "Has to be numeric".
                focus.0 = Some(TextField::Age);
                return;
            }
            let mut state = GameState::new();
            state.difficulty = crate::engine::difficulty::DifficultyModifiers::for_level(game.difficulty);
            game.state = Some(state);
            game.slot = Some(format!("{}{}", profile.forename, profile.surname).replace(' ', ""));
            next_app.set(AppState::InGame);
            next_screen.set(MenuScreen::None);
        }
    }
}

fn type_into_field(
    mut keys: MessageReader<KeyboardInput>,
    mut focus: ResMut<TextFocus>,
    mut game: ResMut<CurrentGame>,
) {
    let Some(field) = focus.0 else {
        keys.clear();
        return;
    };
    let Some(profile) = game.profile.as_mut() else { return };
    let text = match field {
        TextField::Forename => &mut profile.forename,
        TextField::Surname => &mut profile.surname,
        TextField::Age => &mut profile.age,
    };
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Enter | Key::Escape | Key::Tab => focus.0 = None,
            Key::Backspace => {
                text.pop();
            }
            Key::Character(c) => {
                let ok = match field {
                    TextField::Age => c.chars().all(|ch| ch.is_ascii_digit()) && text.len() < 3,
                    _ => text.chars().count() < 24,
                };
                if ok {
                    text.push_str(c);
                }
            }
            Key::Space if field != TextField::Age => text.push(' '),
            _ => {}
        }
        if focus.0.is_none() {
            break;
        }
    }
}

fn refresh_creator(
    game: Res<CurrentGame>,
    focus: Res<TextFocus>,
    data: Res<MenuData>,
    time: Res<Time>,
    mut values: Query<(&CreatorValue, &mut Text)>,
) {
    let Some(profile) = &game.profile else { return };
    let caret = if (time.elapsed_secs() * 2.0) as u32 % 2 == 0 { "|" } else { " " };
    for (value, mut text) in &mut values {
        let s = match *value {
            CreatorValue::Text(field) => {
                let v = match field {
                    TextField::Forename => &profile.forename,
                    TextField::Surname => &profile.surname,
                    TextField::Age => &profile.age,
                };
                if focus.0 == Some(field) { format!("{v}{caret}") } else { v.clone() }
            }
            CreatorValue::Cycler(Cycler::Occupation) => data.creator.occupations[profile.occupation].clone(),
            CreatorValue::Cycler(Cycler::Body(i)) => data.creator.body[i].options[profile.body[i]].clone(),
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

/// Hold to spin the preview survivor.
#[derive(Component)]
struct RotateButton(f32);

/// The survivor being customised, plus the profile it was built from.
#[derive(Component)]
struct Preview {
    built_from: CharacterProfile,
}

/// Camera + light for the 3D preview, rendered under the UI (the menu camera doesn't clear).
fn spawn_preview_stage(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera { order: -1, ..default() },
        Transform::from_xyz(0.0, 1.05, 3.4).looking_at(Vec3::new(0.0, 0.95, 0.0), Vec3::Y),
        DespawnOnExit(MenuScreen::Creator),
    ));
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        DespawnOnExit(MenuScreen::Creator),
    ));
    commands.spawn((
        AmbientLight { brightness: 400.0, ..default() },
        DespawnOnExit(MenuScreen::Creator),
    ));
}

/// Respawn the preview whenever the profile changes (parts are cached assets, so this is cheap).
fn rebuild_preview(
    mut commands: Commands,
    game: Res<CurrentGame>,
    data: Res<MenuData>,
    assets: Res<AssetServer>,
    previews: Query<(Entity, &Preview, &Transform)>,
) {
    let Some(profile) = &game.profile else { return };
    let mut yaw = Quat::from_rotation_y(-0.35);
    for (e, preview, t) in &previews {
        if preview.built_from == *profile {
            return;
        }
        yaw = t.rotation;
        commands.entity(e).despawn();
    }
    // Right of the details panel, where BP_CharacterCreator stands in the original.
    let root = (
        Transform::from_xyz(1.35, 0.0, 0.0).with_rotation(yaw),
        Preview { built_from: profile.clone() },
        DespawnOnExit(MenuScreen::Creator),
    );
    spawn_character(&mut commands, &assets, profile, &data.creator, root);
}

fn rotate_preview(
    time: Res<Time<Real>>,
    held: Query<&RotateButton, With<Pressed>>,
    mut previews: Query<&mut Transform, (With<Preview>, With<CharacterModel>)>,
) {
    let dir: f32 = held.iter().map(|b| b.0).sum();
    if dir == 0.0 {
        return;
    }
    for mut t in &mut previews {
        t.rotate_y(dir * 2.0 * time.delta_secs());
    }
}

/// What the preview survivor plays (idle normally; the screenshot tour cycles others).
#[derive(Resource, Default)]
pub struct PreviewLocomotion(pub crate::models::Locomotion);

fn preview_locomotion(want: Res<PreviewLocomotion>, mut previews: Query<&mut crate::models::Locomotion, With<Preview>>) {
    for mut l in &mut previews {
        l.set_if_neq(want.0);
    }
}
