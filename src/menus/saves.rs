//! `SaveMenu`: slot list on the left, selected save's details on the right, Save / Load / Set
//! Current / Delete along the bottom. Opened from the main menu (Load) and the pause screen (Save).
//!
//! Slots are `SaveSlotManager` files in `saves/`; the creator profile sits beside each one as
//! `<slot>.profile.json` so the save format itself is unchanged.

use std::path::Path;

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;

use super::data::CharacterProfile;
use super::main_menu::SAVE_DIR;
use super::widgets::*;
use super::{AppState, CurrentGame, GotoScreen, MenuScreen};
use crate::persistence::save_slots::{SaveSlotManager, SaveSlotMetadata};

pub struct SavesPlugin;

impl Plugin for SavesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedSlot>()
            .add_systems(OnEnter(MenuScreen::Saves), spawn_saves)
            .add_systems(Update, rebuild_on_change.run_if(in_state(MenuScreen::Saves)))
            .add_observer(on_save_action);
    }
}

#[derive(Resource, Default)]
struct SelectedSlot(Option<String>);

#[derive(Component, Clone)]
enum SaveAction {
    Select(String),
    Save,
    Load,
    SetCurrent,
    Delete,
}

/// Marks the root so the page can be rebuilt after a save/delete.
#[derive(Component)]
struct SavesRoot;

#[derive(Resource)]
struct RebuildSaves;

fn profile_path(slot: &str) -> std::path::PathBuf {
    Path::new(SAVE_DIR).join(format!("{slot}.profile.json"))
}

fn inventory_path(slot: &str) -> std::path::PathBuf {
    Path::new(SAVE_DIR).join(format!("{slot}.inventory.json"))
}

/// Read a slot into the session. Returns false if the file is missing or unreadable.
pub fn load_into(game: &mut CurrentGame, slot: &str) -> bool {
    let Ok(container) = SaveSlotManager::load_slot(slot, Path::new(SAVE_DIR)) else { return false };
    game.difficulty = container.game_state.difficulty.level;
    game.state = Some(container.game_state);
    game.profile = std::fs::read_to_string(profile_path(slot)).ok().and_then(|s| serde_json::from_str::<CharacterProfile>(&s).ok());
    game.inventory = std::fs::read_to_string(inventory_path(slot)).ok().and_then(|s| serde_json::from_str(&s).ok());
    game.slot = Some(slot.to_string());
    true
}

/// Write the session to `slot` (and make it the current save).
pub fn write_save(game: &mut CurrentGame, slot: &str, display_name: &str) -> std::io::Result<()> {
    let Some(state) = &game.state else { return Ok(()) };
    SaveSlotManager::save_slot(state, slot, display_name, Path::new(SAVE_DIR))?;
    if let Some(profile) = &game.profile {
        std::fs::write(profile_path(slot), serde_json::to_string_pretty(profile).unwrap_or_default())?;
    }
    if let Some(inv) = &game.inventory {
        std::fs::write(inventory_path(slot), serde_json::to_string(inv).unwrap_or_default())?;
    }
    game.slot = Some(slot.to_string());
    Ok(())
}

/// Unix seconds -> "YYYY-MM-DD HH:MM" (UTC).
fn format_date(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let (h, m) = ((secs % 86_400) / 3600, (secs % 3600) / 60);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}")
}

fn spawn_saves(
    mut commands: Commands,
    font: Res<MenuFont>,
    app: Res<State<AppState>>,
    game: Res<CurrentGame>,
    mut selected: ResMut<SelectedSlot>,
) {
    let in_game = *app.get() == AppState::InGame;
    let slots = SaveSlotManager::list_slots(Path::new(SAVE_DIR));
    if selected.0.as_ref().is_none_or(|s| !slots.iter().any(|m| &m.slot_id == s)) {
        selected.0 = game.slot.clone().filter(|s| slots.iter().any(|m| &m.slot_id == s)).or_else(|| slots.first().map(|m| m.slot_id.clone()));
    }
    let font = &*font;
    let back = if in_game { MenuScreen::Pause } else { MenuScreen::Main };

    let root = commands.spawn((screen(MenuScreen::Saves), GlobalZIndex(4), SavesRoot, children![backdrop(0.55)])).id();
    let window = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                ..anchored(Vec2::splat(0.5), Vec2::ZERO, Vec2::new(1800.0, 950.0), Vec2::splat(0.5))
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
        ))
        .id();
    commands.entity(root).add_child(window);
    commands.entity(root).with_child(back_button(font, GotoScreen(back)));

    let title = commands.spawn((label(font, "Save Games", 24.0), Node { margin: UiRect::new(Val::Px(20.0), Val::Px(0.0), Val::Px(0.0), Val::Px(10.0)), ..default() })).id();
    let body = commands.spawn(Node { flex_grow: 1.0, column_gap: Val::Px(10.0), min_height: Val::Px(0.0), ..default() }).id();
    let buttons = commands.spawn(Node { padding: UiRect::axes(Val::Px(30.0), Val::Px(10.0)), column_gap: Val::Px(25.0), align_items: AlignItems::Center, ..default() }).id();
    commands.entity(window).add_children(&[title, body, buttons]);

    // Slot list.
    let list = commands
        .spawn((
            Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), padding: UiRect::all(Val::Px(20.0)), overflow: Overflow::scroll_y(), ..default() },
            ScrollPosition::default(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
        ))
        .id();
    commands.entity(body).add_child(list);
    if slots.is_empty() {
        let none = commands.spawn(label_colored(font, "No save selected or present", 16.0, TEXT_DIM)).id();
        commands.entity(list).add_child(none);
    }
    for meta in &slots {
        let current = game.slot.as_deref() == Some(meta.slot_id.as_str());
        let mut row = commands.spawn((
            button(
                Node { min_height: Val::Px(90.0), padding: UiRect::horizontal(Val::Px(20.0)), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, flex_shrink: 0.0, ..default() },
                ButtonPalette { normal: Color::srgba(0.0, 0.0, 0.0, 0.5), ..ButtonPalette::MENU },
                SaveAction::Select(meta.slot_id.clone()),
            ),
            children![
                label(font, meta.display_name.clone(), 20.0),
                label_colored(font, if current { "[Current]" } else { "" }, 14.0, TEXT_DIM),
            ],
        ));
        if selected.0.as_deref() == Some(meta.slot_id.as_str()) {
            row.insert(Chosen);
        }
        let id = row.id();
        commands.entity(list).add_child(id);
    }

    // Details of the selected slot.
    let info = commands
        .spawn((
            Node { flex_grow: 0.5, flex_basis: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), padding: UiRect::all(Val::Px(20.0)), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
        ))
        .id();
    commands.entity(body).add_child(info);
    if let Some(meta) = selected.0.as_ref().and_then(|s| slots.iter().find(|m| &m.slot_id == s)) {
        for line in describe(meta) {
            let l = commands.spawn(label_colored(font, line, 16.0, Color::srgb(0.75, 0.75, 0.75))).id();
            commands.entity(info).add_child(l);
        }
    }

    let action = |commands: &mut Commands, text: &str, action: SaveAction, enabled: bool| {
        let mut b = commands.spawn(button_widget(font, text, 18.0, Node::default(), action));
        if !enabled {
            b.insert(InteractionDisabled);
        }
        b.id()
    };
    let has = selected.0.is_some();
    let save = action(&mut commands, "Save Game", SaveAction::Save, in_game);
    let load = action(&mut commands, "Load Game", SaveAction::Load, has);
    let current = action(&mut commands, "Set Current Save", SaveAction::SetCurrent, has && in_game);
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let delete = action(&mut commands, "Delete Save", SaveAction::Delete, has);
    commands.entity(buttons).add_children(&[save, load, current, spacer, delete]);
}

fn describe(meta: &SaveSlotMetadata) -> Vec<String> {
    let profile = std::fs::read_to_string(profile_path(&meta.slot_id)).ok().and_then(|s| serde_json::from_str::<CharacterProfile>(&s).ok());
    let mut lines = vec![meta.display_name.clone()];
    if let Some(p) = profile {
        lines.push(format!("CHARACTER NAME : {} {}", p.forename, p.surname));
    }
    lines.push(format!("DATE CREATED : {}", format_date(meta.timestamp_epoch_secs)));
    lines.push(format!("LEVEL {}  -  {:?}", meta.character_level, meta.difficulty));
    lines.push(meta.location_desc.clone());
    lines
}

fn on_save_action(
    ev: On<Activate>,
    mut commands: Commands,
    actions: Query<&SaveAction>,
    mut selected: ResMut<SelectedSlot>,
    mut game: ResMut<CurrentGame>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_screen: ResMut<NextState<MenuScreen>>,
) {
    let Ok(action) = actions.get(ev.entity) else { return };
    match action {
        SaveAction::Select(slot) => selected.0 = Some(slot.clone()),
        SaveAction::Save => {
            let slot = game.slot.clone().unwrap_or_else(|| "Survivor".into());
            if let Err(e) = write_save(&mut game, &slot, &slot) {
                error!("save failed: {e}");
            }
            selected.0 = Some(slot);
        }
        SaveAction::Load => {
            let Some(slot) = selected.0.clone() else { return };
            if load_into(&mut game, &slot) {
                // Re-entering InGame (even from InGame) rebuilds the world from the loaded state.
                next_app.set(AppState::InGame);
                next_screen.set(MenuScreen::None);
                return;
            }
        }
        SaveAction::SetCurrent => game.slot = selected.0.clone(),
        SaveAction::Delete => {
            if let Some(slot) = selected.0.take() {
                let _ = std::fs::remove_file(Path::new(SAVE_DIR).join(format!("{slot}.sav.json")));
                let _ = std::fs::remove_file(profile_path(&slot));
                let _ = std::fs::remove_file(inventory_path(&slot));
                if game.slot.as_deref() == Some(slot.as_str()) {
                    game.slot = None;
                }
            }
        }
    }
    commands.insert_resource(RebuildSaves);
}

fn rebuild_on_change(
    mut commands: Commands,
    flag: Option<Res<RebuildSaves>>,
    roots: Query<Entity, With<SavesRoot>>,
    font: Res<MenuFont>,
    app: Res<State<AppState>>,
    game: Res<CurrentGame>,
    selected: ResMut<SelectedSlot>,
) {
    if flag.is_none() && !selected.is_changed() {
        return;
    }
    commands.remove_resource::<RebuildSaves>();
    for e in &roots {
        commands.entity(e).despawn();
    }
    // Reuse the spawn path; it reads everything fresh from disk.
    spawn_saves(commands, font, app, game, selected);
}

#[cfg(test)]
mod tests {
    use super::format_date;

    #[test]
    fn formats_unix_time_as_utc_date() {
        assert_eq!(format_date(0), "1970-01-01 00:00");
        assert_eq!(format_date(1_791_676_800 + 3_660), "2026-10-11 01:01");
        assert_eq!(format_date(951_782_400), "2000-02-29 00:00");
    }
}
