//! `NewGameDifficultySelectionPage` (main menu → New) and the in-game `DifficultySettingsMenu`.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::data::MenuData;
use super::settings::{Group, SettingsStore, Value, row_key, spawn_panel, spawn_row};
use super::widgets::*;
use super::{CurrentGame, GotoScreen, MenuScreen};
use crate::engine::difficulty::DifficultyLevel;

pub struct NewGamePlugin;

impl Plugin for NewGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuScreen::NewGame), spawn_new_game)
            .add_systems(OnEnter(MenuScreen::DifficultySettings), spawn_in_game_difficulty)
            .add_observer(on_difficulty)
            .add_observer(on_start);
    }
}

const LEVELS: [(DifficultyLevel, &str); 5] = [
    (DifficultyLevel::Peaceful, "Peaceful"),
    (DifficultyLevel::Easy, "Easy"),
    (DifficultyLevel::Standard, "Standard"),
    (DifficultyLevel::Hardened, "Hardened"),
    (DifficultyLevel::Nightmare, "Nightmare"),
];

#[derive(Component, Clone, Copy)]
struct DifficultyChoice(DifficultyLevel);

#[derive(Component)]
struct StartButton;

fn spawn_new_game(mut commands: Commands, font: Res<MenuFont>, data: Res<MenuData>, store: Res<SettingsStore>, game: Res<CurrentGame>) {
    let font = &*font;
    let root = commands.spawn((screen(MenuScreen::NewGame), GlobalZIndex(4), children![backdrop(0.55)])).id();

    let header = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        })
        .id();
    commands.entity(root).add_child(header);
    let title = commands.spawn((label(font, "DIFFICULTY", 28.0), Node { margin: UiRect::vertical(Val::Px(20.0)), ..default() })).id();
    commands.entity(header).add_child(title);

    // SettingRowBig: PERMADEATH? above the difficulty buttons, the in-game toggle below.
    let big = |commands: &mut Commands, i: usize| {
        let row = &data.new_game_rows[i];
        spawn_row(commands, font, &store, row_key("newgame", "General", &row.label), row, false, 22.0)
    };
    let permadeath = big(&mut commands, 0);
    commands.entity(header).add_child(permadeath);

    let choices = commands
        .spawn(Node { width: Val::Percent(100.0), padding: UiRect::horizontal(Val::Px(250.0)), justify_content: JustifyContent::SpaceEvenly, ..default() })
        .id();
    commands.entity(header).add_child(choices);
    for (level, name) in LEVELS {
        let mut b = commands.spawn((
            button(Node { padding: UiRect::axes(Val::Px(18.0), Val::Px(5.0)), ..default() }, ButtonPalette::MENU, DifficultyChoice(level)),
            children![label(font, name, 18.0)],
        ));
        if level == game.difficulty {
            b.insert(Chosen);
        }
        let id = b.id();
        commands.entity(choices).add_child(id);
    }
    let allow_changes = big(&mut commands, 1);
    commands.entity(header).add_child(allow_changes);

    // Category panel: 1750x675 centred (alignment 0.5, 0.385), rows start 360px in.
    let node = Node {
        padding: UiRect::all(Val::Px(20.0)),
        ..anchored(Vec2::splat(0.5), Vec2::ZERO, Vec2::new(1750.0, 675.0), Vec2::new(0.5, 0.385))
    };
    let panel = spawn_panel(&mut commands, font, &data, &store, Group::Difficulty, node, 360.0);
    commands.entity(root).add_child(panel);

    let start_node = Node { position_type: PositionType::Absolute, bottom: Val::Px(20.0), left: Val::Percent(50.0), margin: UiRect::left(Val::Px(-80.0)), ..default() };
    let back_node = Node { position_type: PositionType::Absolute, bottom: Val::Px(20.0), right: Val::Px(40.0), ..default() };
    commands.entity(root).with_children(|r| {
        r.spawn((
            button(Node { padding: UiRect::axes(Val::Px(16.0), Val::Px(4.0)), ..start_node }, ButtonPalette::BARE, StartButton),
            children![label(font, "Start", 48.0)],
        ));
        r.spawn((
            button(Node { padding: UiRect::axes(Val::Px(16.0), Val::Px(4.0)), ..back_node }, ButtonPalette::BARE, GotoScreen(MenuScreen::Main)),
            children![label(font, "Back", 48.0)],
        ));
    });
}

/// Pause → Difficulty Settings: the same category panel, editable mid-game.
fn spawn_in_game_difficulty(mut commands: Commands, font: Res<MenuFont>, data: Res<MenuData>, store: Res<SettingsStore>) {
    let root = commands.spawn((screen(MenuScreen::DifficultySettings), GlobalZIndex(4), children![backdrop(0.6)])).id();
    let panel = spawn_panel(
        &mut commands,
        &font,
        &data,
        &store,
        Group::Difficulty,
        anchored(Vec2::splat(0.5), Vec2::ZERO, Vec2::new(1710.0, 800.0), Vec2::splat(0.5)),
        360.0,
    );
    commands.entity(root).add_child(panel);
    commands.entity(root).with_child(back_button(&font, GotoScreen(MenuScreen::Pause)));
}

fn on_difficulty(
    ev: On<Activate>,
    mut commands: Commands,
    choices: Query<(Entity, &DifficultyChoice)>,
    mut game: ResMut<CurrentGame>,
    data: Res<MenuData>,
    mut store: ResMut<SettingsStore>,
) {
    let Ok((_, choice)) = choices.get(ev.entity) else { return };
    game.difficulty = choice.0;
    // ChangeDifficulty: the button's DT_Difficulty row overwrites the difficulty settings.
    if let Some((_, name)) = LEVELS.iter().find(|(l, _)| *l == choice.0) {
        store.apply_preset(&data, name);
    }
    for (e, c) in &choices {
        if c.0 == choice.0 {
            commands.entity(e).insert(Chosen);
        } else {
            commands.entity(e).remove::<Chosen>();
        }
    }
}

fn on_start(
    ev: On<Activate>,
    starts: Query<(), With<StartButton>>,
    store: Res<SettingsStore>,
    data: Res<MenuData>,
    mut game: ResMut<CurrentGame>,
    mut next: ResMut<NextState<MenuScreen>>,
) {
    if starts.get(ev.entity).is_err() {
        return;
    }
    let permadeath = &data.new_game_rows[0];
    game.permadeath = store.get(&row_key("newgame", "General", &permadeath.label), permadeath) == Value::Bool(true);
    next.set(MenuScreen::Creator);
}
