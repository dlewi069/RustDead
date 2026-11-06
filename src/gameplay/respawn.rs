//! Death and the respawn screen: BP_PlayerController.Client_Died (LevellingComponent.XPDeath,
//! then `UI/Widgets/Player/RespawnScreen`) and SD_GameInstance.Survival_Respawn, decompiled:
//!
//! - RespawnScreen: "YOU ARE DEAD" (48, E1E1E1) over Respawn - Random / Respawn - Spawn Point /
//!   Load A Previous Save / Quit To Main Menu / Quit (32 pt, the menu buttons' tints), and at the
//!   bottom "You survived for N days.", "You travelled for X.XXkm.", "You killed N zombies."
//!   (bosses included), "You killed N humans." While it's up the world runs at 0.1x
//!   (SetGlobalTimeDilation, standalone only).
//! - Respawn - Random: a new pawn at a random 'Survival' PlayerStart. Respawn - Spawn Point: at
//!   the controller's RespawnLoc if RespawnPointEnabled (a bed's Set Spawn Point, saved), else
//!   "No Respawn Point Set" and the screen stays. Either way Death_PlayerStats runs after the
//!   respawn (the screen shows the stats before the reset).
//! - Load A Previous Save shows the save menu; Quit To Main Menu loads MainMenu; Quit asks
//!   (QuitWidgetRespawn).
//!
//! - ScreenPermadeath (Client_Died with Difficulty.Permadeath, saved per game as
//!   `GameState::permadeath` from the new game page's PERMADEATH?): "PERMADEATH MODE" (28) at the
//!   top, "YOU ARE DEAD" (48) over Quit To Main Menu / Quit (32), and Statistics (48): Days
//!   Survived, Zombies Killed (ZombieKills only), Humans Killed, Distance Travelled ("1.23km"),
//!   each value 50 px after its label. Time at 0.1x. Its Construct (DeleteSave? true) deletes the
//!   save slot and toasts "{Name} Deleted"; Quit asks (QuitWidgetRespawn with Permadeath?: "no"
//!   comes back with DeleteSave? false). No respawn.
//!
//! Ours: the PlayerStarts' tags aren't exported, so the random pick is from all 22; the new pawn
//! is the same survivor moved and refreshed (`survival::respawn`); Load / Quit open our Saves
//! and QuitToDesktop pages (Esc there goes to the pause menu, then back here); the Fade
//! animation and BackgroundBlur are a dark overlay.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::interact::GameOverlay;
use super::notify::Notify;
use crate::menus::data::CurrentGame;
use crate::menus::widgets::{ButtonPalette, MenuFont, button, label, label_colored};
use crate::menus::{AppState, MenuScreen};

/// SetGlobalTimeDilation while the screen is up.
pub const DILATION: f32 = 0.1;

/// Respawn: at a random PlayerStart (`random`) or the spawn point.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Respawn {
    pub random: bool,
}

#[derive(Component)]
struct RespawnUi;

#[derive(Component)]
struct PermadeathUi;

/// ScreenPermadeath's buttons.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum PermadeathButton {
    QuitToMainMenu,
    Quit,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum RespawnButton {
    Random,
    SpawnPoint,
    LoadSave,
    QuitToMainMenu,
    Quit,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Respawn>()
        .add_systems(Update, (open_close, open_close_permadeath, dilate).run_if(in_state(AppState::InGame)))
        .add_systems(OnExit(AppState::InGame), |mut t: ResMut<Time<Virtual>>| t.set_relative_speed(1.0))
        .add_observer(on_button)
        .add_observer(on_permadeath_button);
}

/// ScreenPermadeath's Get_DistanceTravelled: DistanceTravelled / 100000 (0-2 decimals) + "km".
pub fn permadeath_km(distance_cm: f64) -> String {
    km_text(distance_cm).trim_end_matches('.').to_string()
}

/// Construct's DeleteSave?: the slot's files go, "{Name} Deleted" (once a death).
fn open_close_permadeath(
    mut commands: Commands,
    overlay: Res<GameOverlay>,
    font: Res<MenuFont>,
    mut game: ResMut<CurrentGame>,
    mut notify: MessageWriter<Notify>,
    uis: Query<Entity, With<PermadeathUi>>,
    mut deleted: Local<bool>,
) {
    if *overlay != GameOverlay::Permadeath {
        uis.iter().for_each(|e| commands.entity(e).despawn());
        *deleted = false;
        return;
    }
    if !*deleted {
        *deleted = true;
        if let Some(slot) = game.slot.take() {
            crate::menus::saves::delete_save(&slot);
            notify.write(Notify::new(format!("{slot} Deleted")));
            info!("permadeath: deleted save {slot}");
        }
    }
    if uis.is_empty() {
        let stats = game.state.as_ref().map(|s| s.stats.clone()).unwrap_or_default();
        spawn_permadeath(&mut commands, &font, &stats);
    }
}

/// ScreenPermadeath (widget_tree.py on UI/Widgets/Player/ScreenPermadeath): the title, UIOptions
/// (900 wide, its bottom 15 % above the middle) and Stats (900 x 400, from 15 % below it).
fn spawn_permadeath(commands: &mut Commands, font: &MenuFont, stats: &super::stats::JournalStats) {
    let root = commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            GlobalZIndex(40),
            DespawnOnExit(AppState::InGame),
            PermadeathUi,
        ))
        .id();
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), top: Val::Px(8.0), justify_content: JustifyContent::Center, ..default() },
        ChildOf(root),
        children![label(font, "PERMADEATH MODE", 28.0)],
    ));
    let options = commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), bottom: Val::Percent(50.0), margin: UiRect::bottom(Val::Px(350.0 * 0.15)), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(20.0), ..default() },
            ChildOf(root),
        ))
        .id();
    commands.spawn((label(font, "YOU ARE DEAD", 48.0), ChildOf(options)));
    for (b, text) in [(PermadeathButton::QuitToMainMenu, "Quit To Main Menu"), (PermadeathButton::Quit, "Quit")] {
        let node = Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
        commands.spawn((button(node, ButtonPalette::MENU, b), ChildOf(options), children![label(font, text, 32.0)]));
    }
    let table = commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), top: Val::Percent(50.0), margin: UiRect::top(Val::Px(400.0 * 0.15)), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(20.0), ..default() },
            ChildOf(root),
        ))
        .id();
    commands.spawn((label(font, "Statistics", 48.0), ChildOf(table)));
    for (name, value) in [
        ("Days Survived:", grouped(stats.days_survived)),
        ("Zombies Killed:", grouped(stats.zombie_kills)),
        ("Humans Killed:", grouped(stats.human_kills)),
        ("Distance Travelled:", permadeath_km(stats.distance)),
    ] {
        commands.spawn((
            Node { column_gap: Val::Px(50.0), ..default() },
            ChildOf(table),
            children![label(font, name, 32.0), label(font, value, 32.0)],
        ));
    }
}

fn on_permadeath_button(ev: On<Activate>, buttons: Query<&PermadeathButton>, mut game: ResMut<CurrentGame>, mut next_app: ResMut<NextState<AppState>>, mut next_screen: ResMut<NextState<MenuScreen>>) {
    let Ok(b) = buttons.get(ev.entity) else { return };
    match b {
        // SetGlobalTimeDilation(1), ResetPlayerStats, DestroyActors, Event_LoadLevel('MainMenu').
        PermadeathButton::QuitToMainMenu => {
            *game = CurrentGame::default();
            next_app.set(AppState::MainMenu);
            next_screen.set(MenuScreen::Main);
        }
        // QuitWidgetRespawn with Permadeath?.
        PermadeathButton::Quit => next_screen.set(MenuScreen::QuitToDesktop),
    }
}

/// "You travelled for {km}km.": DistanceTravelled / 100000, up to 2 decimals.
pub fn km_text(distance_cm: f64) -> String {
    let km = format!("{:.2}", distance_cm / 100_000.0);
    let km = km.trim_end_matches('0').trim_end_matches('.');
    format!("{km}km.")
}

/// Conv_IntToText with grouping: 1,234.
fn grouped(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn open_close(mut commands: Commands, overlay: Res<GameOverlay>, font: Res<MenuFont>, game: Res<CurrentGame>, uis: Query<Entity, With<RespawnUi>>) {
    let open = *overlay == GameOverlay::Respawn;
    if !open {
        uis.iter().for_each(|e| commands.entity(e).despawn());
    } else if uis.is_empty() {
        let stats = game.state.as_ref().map(|s| s.stats.clone()).unwrap_or_default();
        spawn(&mut commands, &font, &stats);
    }
}

fn spawn(commands: &mut Commands, font: &MenuFont, stats: &super::stats::JournalStats) {
    let root = commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            GlobalZIndex(40),
            DespawnOnExit(AppState::InGame),
            RespawnUi,
        ))
        .id();
    // DeadSizeBox: centred along the top (an Overlay slot naming only HAlign_Center).
    let column = commands
        .spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(20.0), ..default() }, ChildOf(root)))
        .id();
    commands.spawn((label_colored(font, "YOU ARE DEAD", 48.0, Color::srgb(0.75, 0.75, 0.75)), ChildOf(column)));
    for (b, text) in [
        (RespawnButton::Random, "Respawn - Random"),
        (RespawnButton::SpawnPoint, "Respawn - Spawn Point"),
        (RespawnButton::LoadSave, "Load A Previous Save"),
        (RespawnButton::QuitToMainMenu, "Quit To Main Menu"),
        (RespawnButton::Quit, "Quit"),
    ] {
        let node = Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
        commands.spawn((button(node, ButtonPalette::MENU, b), ChildOf(column), children![label(font, text, 32.0)]));
    }
    // StatsSizeBox: 400 px high at the bottom centre, a line each with 20 px between.
    let lines = [
        format!("You survived for {} days.", grouped(stats.days_survived)),
        format!("You travelled for {}", km_text(stats.distance)),
        format!("You killed {} zombies.", grouped(stats.zombie_kills + stats.boss_zombie_kills)),
        format!("You killed {} humans.", grouped(stats.human_kills)),
    ];
    let bottom = commands
        .spawn((
            Node { position_type: PositionType::Absolute, bottom: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Px(400.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(20.0), ..default() },
            ChildOf(root),
        ))
        .id();
    for l in lines {
        commands.spawn((label(font, l, 32.0), ChildOf(bottom)));
    }
}

/// SetGlobalTimeDilation(0.1) while the screen is up, 1 otherwise.
fn dilate(overlay: Res<GameOverlay>, mut time: ResMut<Time<Virtual>>) {
    let want = if matches!(*overlay, GameOverlay::Respawn | GameOverlay::Permadeath) { DILATION } else { 1.0 };
    if time.relative_speed() != want {
        time.set_relative_speed(want);
    }
}

#[allow(clippy::too_many_arguments)]
fn on_button(
    ev: On<Activate>,
    buttons: Query<&RespawnButton>,
    mut game: ResMut<CurrentGame>,
    mut respawn: MessageWriter<Respawn>,
    mut notify: MessageWriter<Notify>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_screen: ResMut<NextState<MenuScreen>>,
) {
    let Ok(b) = buttons.get(ev.entity) else { return };
    match b {
        RespawnButton::Random => {
            respawn.write(Respawn { random: true });
        }
        RespawnButton::SpawnPoint => {
            if game.state.as_ref().is_some_and(|s| s.respawn_point.is_some()) {
                respawn.write(Respawn { random: false });
            } else {
                notify.write(Notify::new("No Respawn Point Set"));
            }
        }
        RespawnButton::LoadSave => next_screen.set(MenuScreen::Saves),
        RespawnButton::QuitToMainMenu => {
            *game = CurrentGame::default();
            next_app.set(AppState::MainMenu);
            next_screen.set(MenuScreen::Main);
        }
        RespawnButton::Quit => next_screen.set(MenuScreen::QuitToDesktop),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_text() {
        assert_eq!(km_text(0.0), "0km.");
        assert_eq!(km_text(123_456.0), "1.23km.");
        assert_eq!(km_text(150_000.0), "1.5km.");
        assert_eq!(grouped(7), "7");
        assert_eq!(grouped(1234), "1,234");
        assert_eq!(grouped(1_234_567), "1,234,567");
        assert_eq!(permadeath_km(123_456.0), "1.23km");
        assert_eq!(permadeath_km(0.0), "0km");
    }
}
