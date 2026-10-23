//! `PauseScreen`: Resume / Settings / Difficulty Settings / Quick Save / Save / Suicide /
//! Quit To Main Menu / Quit, plus the suicide and quit-to-menu confirmations.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::main_menu::confirm_dialog;
use super::widgets::*;
use super::{AppState, CurrentGame, GAME_UPDATE, GAME_VERSION, GotoScreen, MenuScreen};

pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuScreen::Pause), spawn_pause)
            .add_systems(OnEnter(MenuScreen::Suicide), spawn_suicide)
            .add_systems(OnEnter(MenuScreen::QuitToMainMenu), spawn_quit_to_menu)
            .add_systems(OnEnter(MenuScreen::None), |mut t: ResMut<Time<Virtual>>| t.unpause())
            .add_systems(OnExit(MenuScreen::None), |mut t: ResMut<Time<Virtual>>| t.pause())
            .add_systems(OnEnter(AppState::MainMenu), |mut t: ResMut<Time<Virtual>>| t.unpause())
            .add_systems(Update, quick_save_key.run_if(in_state(MenuScreen::None)))
            .add_observer(on_pause_action);
    }
}

/// Set when the player confirms Suicide; the world moves them to a fresh spawn point.
#[derive(Resource)]
pub struct RespawnPlayer;

#[derive(Component, Clone, Copy)]
enum PauseAction {
    QuickSave,
    Respawn,
    QuitToMainMenu,
}

pub const QUICK_SAVE_SLOT: &str = "QuickSave";

fn spawn_pause(mut commands: Commands, font: Res<MenuFont>) {
    let font = &*font;
    let root = commands.spawn((screen(MenuScreen::Pause), GlobalZIndex(3), children![backdrop(0.45)])).id();
    let column = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            ..anchored(Vec2::splat(0.5), Vec2::ZERO, Vec2::new(500.0, 395.0), Vec2::splat(0.5))
        })
        .id();
    commands.entity(root).add_child(column);

    let entries: [(&str, Option<MenuScreen>, Option<PauseAction>); 8] = [
        ("Resume", Some(MenuScreen::None), None),
        ("Settings", Some(MenuScreen::Settings), None),
        ("Difficulty Settings", Some(MenuScreen::DifficultySettings), None),
        ("Quick Save", None, Some(PauseAction::QuickSave)),
        ("Save", Some(MenuScreen::Saves), None),
        ("Suicide", Some(MenuScreen::Suicide), None),
        ("Quit To Main Menu", Some(MenuScreen::QuitToMainMenu), None),
        ("Quit", Some(MenuScreen::QuitToDesktop), None),
    ];
    for (text, goto, action) in entries {
        let mut e = commands.spawn((
            button(
                Node { flex_grow: 1.0, margin: UiRect::vertical(Val::Px(5.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                ButtonPalette::MENU,
                (),
            ),
            children![label(font, text, 18.0)],
        ));
        if let Some(to) = goto {
            e.insert(GotoScreen(to));
        }
        if let Some(a) = action {
            e.insert(a);
        }
        let id = e.id();
        commands.entity(column).add_child(id);
    }
    let version = commands
        .spawn((label_colored(font, format!("{GAME_UPDATE} - {GAME_VERSION}"), 12.0, TEXT_DIM), Node { align_self: AlignSelf::Center, ..default() }))
        .id();
    commands.entity(column).add_child(version);
}

fn spawn_suicide(mut commands: Commands, font: Res<MenuFont>) {
    confirm_dialog(&mut commands, &font, MenuScreen::Suicide, "Respawn somewhere else?", PauseAction::Respawn, GotoScreen(MenuScreen::Pause));
    commands.spawn((screen(MenuScreen::Suicide), GlobalZIndex(11), children![back_button(&font, GotoScreen(MenuScreen::Pause))]));
}

fn spawn_quit_to_menu(mut commands: Commands, font: Res<MenuFont>) {
    confirm_dialog(
        &mut commands,
        &font,
        MenuScreen::QuitToMainMenu,
        "Are you sure you want to quit?\nAll unsaved progress will be lost",
        PauseAction::QuitToMainMenu,
        GotoScreen(MenuScreen::Pause),
    );
    commands.spawn((screen(MenuScreen::QuitToMainMenu), GlobalZIndex(11), children![back_button(&font, GotoScreen(MenuScreen::Pause))]));
}

fn quick_save(game: &mut CurrentGame) {
    let keep = game.slot.clone();
    if let Err(e) = super::saves::write_save(game, QUICK_SAVE_SLOT, "Quick Save") {
        error!("quick save failed: {e}");
    }
    // Quick saves don't become the current save (the game's "QUICK SAVE CREATES NEW SLOT?" off).
    game.slot = keep;
}

/// F5 is bound to IA_QuickSave in `IMC_General`.
fn quick_save_key(keys: Res<ButtonInput<KeyCode>>, mut game: ResMut<CurrentGame>) {
    if keys.just_pressed(KeyCode::F5) {
        quick_save(&mut game);
    }
}

fn on_pause_action(
    ev: On<Activate>,
    mut commands: Commands,
    actions: Query<&PauseAction>,
    mut game: ResMut<CurrentGame>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_screen: ResMut<NextState<MenuScreen>>,
) {
    let Ok(action) = actions.get(ev.entity) else { return };
    match action {
        PauseAction::QuickSave => {
            quick_save(&mut game);
            next_screen.set(MenuScreen::None);
        }
        PauseAction::Respawn => {
            // Client_SUICIDE: LevellingComponent.XPDeath first.
            if let Some(st) = game.state.as_mut() {
                st.progression.xp_death();
                st.tech.deathbringer_used = false;
            }
            commands.insert_resource(RespawnPlayer);
            next_screen.set(MenuScreen::None);
        }
        PauseAction::QuitToMainMenu => {
            *game = CurrentGame::default();
            next_app.set(AppState::MainMenu);
            next_screen.set(MenuScreen::Main);
        }
    }
}
