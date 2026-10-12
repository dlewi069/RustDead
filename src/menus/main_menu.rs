//! `MenuWidget`: title, the bottom-left button column, version box, the Modes overlay and the
//! quit-to-desktop confirmation.
//!
//! The original renders `MainMenuLevelSequence` (a 3D flythrough) behind the widget. Until the
//! level is ported, the backdrop cycles the game's `UI/Textures/Backgrounds` screenshots.

use std::path::Path;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;

use super::widgets::*;
use super::{AppState, CurrentGame, GAME_UPDATE, GAME_VERSION, GotoScreen, MenuScreen};
use crate::persistence::save_slots::SaveSlotManager;

pub const SAVE_DIR: &str = "saves";
const UI_TEX: &str = "game/SurrounDead/Content/UI/Textures";
const SLIDE_COUNT: usize = 9;
const SLIDE_HOLD: f32 = 8.0;
const SLIDE_FADE: f32 = 1.5;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), spawn_backdrop)
            .add_systems(OnEnter(MenuScreen::Main), spawn_main)
            .add_systems(OnEnter(MenuScreen::Modes), spawn_modes)
            .add_systems(OnEnter(MenuScreen::QuitToDesktop), spawn_quit_confirm)
            .add_systems(OnEnter(MenuScreen::Creator), |mut q: Query<&mut Visibility, With<Slideshow>>| q.iter_mut().for_each(|mut v| *v = Visibility::Hidden))
            .add_systems(OnExit(MenuScreen::Creator), |mut q: Query<&mut Visibility, With<Slideshow>>| q.iter_mut().for_each(|mut v| *v = Visibility::Inherited))
            .add_systems(Update, (slideshow, mode_previews).run_if(in_state(AppState::MainMenu)))
            .add_observer(on_action);
    }
}

#[derive(Component, Clone, Copy)]
enum MainAction {
    Continue,
    QuitToDesktop,
}

#[derive(Component)]
struct Slideshow {
    current: usize,
    t: f32,
}

#[derive(Component)]
struct SlideLayer(bool);

/// Mode entry index whose preview image fades in while its button is hovered.
#[derive(Component)]
struct ModeButton(usize);
#[derive(Component)]
struct ModePreview(usize);

fn slide(assets: &AssetServer, i: usize) -> Handle<Image> {
    assets.load(format!("{UI_TEX}/Backgrounds/Screenshot{}.png", i % SLIDE_COUNT + 1))
}

fn spawn_backdrop(mut commands: Commands, assets: Res<AssetServer>) {
    // Doesn't clear, so the character creator's 3D preview (camera order -1) shows through.
    commands.spawn((
        Camera2d,
        Camera { clear_color: ClearColorConfig::None, ..default() },
        IsDefaultUiCamera,
        DespawnOnExit(AppState::MainMenu),
    ));
    commands.spawn((
        AudioPlayer::new(assets.load("audio/theme_menu.wav")),
        PlaybackSettings::LOOP,
        DespawnOnExit(AppState::MainMenu),
    ));
    let full = || Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    commands.spawn((
        full(),
        GlobalZIndex(-10),
        BackgroundColor(Color::BLACK),
        Slideshow { current: 0, t: 0.0 },
        DespawnOnExit(AppState::MainMenu),
        children![
            (full(), ImageNode::new(slide(&assets, 0)).with_mode(NodeImageMode::Stretch), SlideLayer(false)),
            (full(), ImageNode::new(slide(&assets, 1)).with_mode(NodeImageMode::Stretch).with_color(Color::NONE), SlideLayer(true)),
        ],
    ));
}

fn slideshow(
    time: Res<Time>,
    assets: Res<AssetServer>,
    mut shows: Query<&mut Slideshow>,
    mut layers: Query<(&SlideLayer, &mut ImageNode)>,
) {
    let Ok(mut show) = shows.single_mut() else { return };
    show.t += time.delta_secs();
    let fade = ((show.t - SLIDE_HOLD) / SLIDE_FADE).clamp(0.0, 1.0);
    if show.t >= SLIDE_HOLD + SLIDE_FADE {
        show.current += 1;
        show.t = 0.0;
        for (layer, mut img) in &mut layers {
            img.image = slide(&assets, show.current + layer.0 as usize);
            img.color = if layer.0 { Color::NONE } else { Color::WHITE };
        }
        return;
    }
    for (layer, mut img) in &mut layers {
        if layer.0 {
            img.color = Color::srgba(1.0, 1.0, 1.0, fade);
        }
    }
}

fn spawn_main(mut commands: Commands, font: Res<MenuFont>, assets: Res<AssetServer>) {
    let has_saves = !SaveSlotManager::list_slots(Path::new(SAVE_DIR)).is_empty();
    let font = &*font;

    let root = commands
        .spawn((
            screen(MenuScreen::Main),
            children![
                (
                    anchored(Vec2::new(0.5, 0.0), Vec2::ZERO, Vec2::new(1133.0, 267.0), Vec2::new(0.5, -0.15)),
                    ImageNode::new(assets.load(format!("{UI_TEX}/TitleTransparent1.png"))),
                ),
                (
                    Node {
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(20.0)),
                        ..anchored(Vec2::ONE, Vec2::new(-15.0, -15.0), Vec2::new(400.0, 165.0), Vec2::ONE)
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                    children![label(font, GAME_UPDATE, 30.0), label_colored(font, GAME_VERSION, 22.0, TEXT_DIM)],
                ),
            ],
        ))
        .id();

    let column = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(5.0),
            ..anchored(Vec2::new(0.0, 1.0), Vec2::ZERO, Vec2::new(500.0, 260.0), Vec2::new(-0.075, 1.4))
        })
        .id();
    commands.entity(root).add_child(column);

    let entries: [(&str, Option<MenuScreen>); 6] = [
        ("Continue", None),
        ("New", Some(MenuScreen::NewGame)),
        ("Load", Some(MenuScreen::Saves)),
        ("Modes", Some(MenuScreen::Modes)),
        ("Settings", Some(MenuScreen::Settings)),
        ("Quit", Some(MenuScreen::QuitToDesktop)),
    ];
    for (text, goto) in entries {
        let enabled = goto.is_some() || has_saves;
        let color = if enabled { TEXT } else { Color::srgba(1.0, 1.0, 1.0, 0.35) };
        let mut e = commands.spawn((
            button(
                Node { flex_grow: 1.0, align_items: AlignItems::Center, padding: UiRect::left(Val::Px(10.0)), ..default() },
                ButtonPalette::MENU,
                (),
            ),
            children![label_colored(font, text, 18.0, color)],
        ));
        match goto {
            Some(to) => e.insert(GotoScreen(to)),
            None => e.insert(MainAction::Continue),
        };
        if !enabled {
            e.insert(InteractionDisabled);
        }
        let id = e.id();
        commands.entity(column).add_child(id);
    }
}

const MODES: [(&str, &str, Option<&str>); 4] = [
    ("Secret Game Mode", "Currently In Development - More information soon.", None),
    ("Shooting Range", "A place to sharpen your skills or just a place to relax.", Some("ShootingRangeImage")),
    ("Tutorial", "Get use to basic game controls and mechanics.", Some("TutorialWorldImage")),
    ("Vehicle Playground", "Play around with vehicles.", Some("VehicleWorldImage")),
];

fn spawn_modes(mut commands: Commands, font: Res<MenuFont>, assets: Res<AssetServer>) {
    let font = &*font;
    let root = commands.spawn((screen(MenuScreen::Modes), children![backdrop(0.55)])).id();

    // Four 1000x400 entries in a ScaleBox fitting a 500px column: everything renders at half size.
    let column = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(62.5),
                width: Val::Px(500.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(2),
        ))
        .id();
    commands.entity(root).add_child(column);

    for (i, (title, desc, image)) in MODES.iter().enumerate() {
        let entry = commands
            .spawn((
                Node {
                    height: Val::Px(200.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![
                    (
                        button(Node { padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)), ..default() }, ButtonPalette::MENU, ModeButton(i)),
                        // None of these levels are ported yet.
                        InteractionDisabled,
                        children![label(font, *title, 32.0)],
                    ),
                    (label(font, *desc, 13.0), Node { margin: UiRect::top(Val::Px(25.0)), ..default() }),
                ],
            ))
            .id();
        commands.entity(column).add_child(entry);

        if let Some(image) = image {
            let preview = commands
                .spawn((
                    anchored(Vec2::splat(0.5), Vec2::ZERO, Vec2::new(1280.0, 720.0), Vec2::new(0.28, 0.5)),
                    ImageNode::new(assets.load(format!("{UI_TEX}/{image}.png"))).with_color(Color::NONE),
                    ModePreview(i),
                    GlobalZIndex(1),
                ))
                .id();
            commands.entity(root).add_child(preview);
        }
    }
    commands.entity(root).with_child(back_button(font, GotoScreen(MenuScreen::Main)));
}

/// `ShootingRangeFade` / `TutorialFade` / `VehicleFade`: preview opacity follows hover.
fn mode_previews(
    time: Res<Time>,
    buttons: Query<(&ModeButton, &Hovered)>,
    mut previews: Query<(&ModePreview, &mut ImageNode)>,
) {
    for (preview, mut img) in &mut previews {
        let hovered = buttons.iter().any(|(b, h)| b.0 == preview.0 && h.get());
        let a = img.color.alpha();
        let target = if hovered { 1.0 } else { 0.0 };
        let step = time.delta_secs() * 4.0;
        let next = if a < target { (a + step).min(target) } else { (a - step).max(target) };
        if next != a {
            img.color = Color::srgba(1.0, 1.0, 1.0, next);
        }
    }
}

/// Centered yes/no dialog used by the main menu and pause screen.
pub fn confirm_dialog<S: States>(
    commands: &mut Commands,
    font: &MenuFont,
    state: S,
    question: &str,
    yes: impl Bundle,
    no: impl Bundle,
) {
    let yes_node = anchored(Vec2::splat(0.5), Vec2::ZERO, Vec2::new(125.0, 50.0), Vec2::new(1.5, -1.0));
    let no_node = anchored(Vec2::splat(0.5), Vec2::ZERO, Vec2::new(125.0, 50.0), Vec2::new(-0.5, -1.0));
    commands.spawn((
        screen(state),
        GlobalZIndex(10),
        children![
            backdrop(0.6),
            (
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![(label(font, question, 24.0), TextLayout::justify(Justify::Center))],
            ),
            button_widget(font, "YES", 24.0, yes_node, yes),
            button_widget(font, "NO", 24.0, no_node, no),
        ],
    ));
}

fn spawn_quit_confirm(mut commands: Commands, font: Res<MenuFont>, app: Res<State<AppState>>) {
    let (question, back) = match app.get() {
        AppState::MainMenu => ("Are you sure you want to quit to desktop?", MenuScreen::Main),
        AppState::InGame => ("Are you sure you want to quit to desktop?\nAll unsaved progress will be lost", MenuScreen::Pause),
    };
    confirm_dialog(&mut commands, &font, MenuScreen::QuitToDesktop, question, MainAction::QuitToDesktop, GotoScreen(back));
    commands.spawn((screen(MenuScreen::QuitToDesktop), GlobalZIndex(11), children![back_button(&font, GotoScreen(back))]));
}

fn on_action(
    ev: On<Activate>,
    actions: Query<&MainAction>,
    mut game: ResMut<CurrentGame>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_screen: ResMut<NextState<MenuScreen>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(action) = actions.get(ev.entity) else { return };
    match action {
        MainAction::QuitToDesktop => {
            exit.write(AppExit::Success);
        }
        MainAction::Continue => {
            let Some(latest) = SaveSlotManager::list_slots(Path::new(SAVE_DIR)).into_iter().next() else { return };
            if super::saves::load_into(&mut game, &latest.slot_id) {
                next_app.set(AppState::InGame);
                next_screen.set(MenuScreen::None);
            }
        }
    }
}
