//! `SettingsMenu` (Video / Audio / Controls / Gameplay) and the reusable category panel that the
//! difficulty pages also use. Rows are data from `menus.json`; values live in [`SettingsStore`]
//! and persist to `saves/settings.json`.

use std::collections::HashMap;

use bevy::audio::Volume;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode};
use serde::{Deserialize, Serialize};

use super::data::{MenuData, Page, Row, RowKind};
use super::widgets::*;
use super::{AppState, MenuScreen};

const SETTINGS_FILE: &str = "saves/settings.json";

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SettingsStore::load())
            .add_systems(Startup, |data: Res<super::MenuData>, mut store: ResMut<SettingsStore>| store.fill_defaults(&data))
            .add_systems(OnEnter(MenuScreen::Settings), spawn_settings)
            .add_systems(OnExit(MenuScreen::Settings), save_settings)
            .add_systems(Update, (refresh_values, apply_settings).run_if(resource_changed::<SettingsStore>))
            .add_observer(on_category)
            .add_observer(on_row);
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Num(f32),
    Choice(usize),
}

/// Every settings / difficulty value, keyed `"<group>/<page>/<row label>"`.
#[derive(Resource, Serialize, Deserialize, Default)]
pub struct SettingsStore {
    pub values: HashMap<String, Value>,
    /// The file's format: 2 saves only values that differ from their row's default, so a
    /// default read from the game later (SD_GameInstance's 30 % vehicle chance) reaches existing
    /// files. Files without it held every row, defaults and guessed fallbacks included.
    #[serde(default)]
    pub version: u32,
}

const VERSION: u32 = 2;

impl SettingsStore {
    fn load() -> Self {
        std::fs::read_to_string(SETTINGS_FILE)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn get(&self, key: &str, row: &Row) -> Value {
        self.values.get(key).copied().unwrap_or_else(|| default_value(row))
    }

    /// Give every row without a stored value its default, so [`Self::find`] sees the game's
    /// starting values (a 30 min day, not 1). An old file's value that is just the guessed
    /// fallback of a row that now has the game's own default gives way to it.
    pub fn fill_defaults(&mut self, data: &super::MenuData) {
        let old = self.version < VERSION;
        for (group, pages) in [("settings", &data.settings), ("difficulty", &data.difficulty)] {
            for page in pages {
                for row in page.rows.iter().filter(|r| !r.label.is_empty()) {
                    let key = row_key(group, &page.name, &row.label);
                    if old && row.default.is_some() && self.values.get(&key) == Some(&fallback_value(row)) {
                        self.values.remove(&key);
                    }
                    self.values.entry(key).or_insert_with(|| default_value(row));
                }
            }
        }
        self.version = VERSION;
    }

    /// What's saved: only values that differ from their row's default.
    pub fn changed(&self, data: &super::MenuData) -> SettingsStore {
        let mut values = self.values.clone();
        for (group, pages) in [("settings", &data.settings), ("difficulty", &data.difficulty)] {
            for page in pages {
                for row in page.rows.iter().filter(|r| !r.label.is_empty()) {
                    let key = row_key(group, &page.name, &row.label);
                    if values.get(&key) == Some(&default_value(row)) {
                        values.remove(&key);
                    }
                }
            }
        }
        SettingsStore { values, version: VERSION }
    }

    /// `GameFunctionLibrary.ChangeDifficulty` -> `SetDifficulty`: write a `DT_Difficulty` preset
    /// into the difficulty rows (check rows take 0/1 as false/true). False if there's no such preset.
    pub fn apply_preset(&mut self, data: &super::MenuData, name: &str) -> bool {
        let Some(preset) = data.difficulty_presets.iter().find(|p| p.name == name) else { return false };
        for v in &preset.values {
            let row = data.difficulty.iter().find(|p| p.name == v.page).and_then(|p| p.rows.iter().find(|r| r.label == v.label));
            let value = match row.map(|r| &r.kind) {
                Some(super::data::RowKind::Check) => Value::Bool(v.value > 0.5),
                _ => Value::Num(v.value),
            };
            self.values.insert(row_key("difficulty", &v.page, &v.label), value);
        }
        true
    }

    /// Look up a value by group/page/label without the row definition (falls back to `None`).
    pub fn find(&self, key: &str) -> Option<Value> {
        self.values.get(key).copied()
    }
}

/// A row's starting value: the game's (menus.json `default`), else [`fallback_value`].
fn default_value(row: &Row) -> Value {
    match (row.kind, row.default) {
        (RowKind::Check, Some(d)) => Value::Bool(d > 0.5),
        (RowKind::Slider, Some(d)) => Value::Num(d),
        _ => fallback_value(row),
    }
}

/// The blueprints don't carry every default (some live in config/CVars), so pick sensible ones.
fn fallback_value(row: &Row) -> Value {
    match row.kind {
        RowKind::Check => Value::Bool(row.label.contains("ENABLED?")),
        RowKind::Slider => {
            let v = if row.cvar.as_deref().is_some_and(|c| c.starts_with("GameAudio")) {
                row.max
            } else if row.label.contains("FIELD OF VIEW") {
                90.0
            } else if row.label == "MAX FPS" {
                120.0
            } else if row.label == "RESOLUTION SCALE" {
                100.0
            } else if row.label == "GAMMA" {
                2.2
            } else if row.min <= 1.0 && 1.0 <= row.max {
                1.0
            } else {
                (row.min + row.max) * 0.5
            };
            Value::Num(v)
        }
        RowKind::Choice => {
            let pick = ["Normal", "HIGH", "WINDOWED", "1920x1080", "TAA", "Off"];
            Value::Choice(row.options.iter().position(|o| pick.contains(&o.as_str())).unwrap_or(0))
        }
        RowKind::Key | RowKind::Button => Value::Bool(false),
    }
}

fn step_of(row: &Row) -> f32 {
    let raw = (row.max - row.min) / 20.0;
    let unit = 10f32.powi(-(row.digits as i32));
    (raw / unit).round().max(1.0) * unit
}

fn format_value(row: &Row, v: Value) -> String {
    match v {
        Value::Bool(b) => if b { "X".into() } else { String::new() },
        Value::Num(n) => format!("{:.*}", row.digits, n * row.multiplier),
        Value::Choice(i) => row.options.get(i).cloned().unwrap_or_default(),
    }
}

fn save_settings(store: Res<SettingsStore>, data: Res<super::MenuData>) {
    write(&store, &data);
}

/// Write `saves/settings.json` (what differs from the defaults, plus values outside the menus
/// such as the map picker's Compass Marker?).
pub fn write(store: &SettingsStore, data: &super::MenuData) {
    if let Ok(json) = serde_json::to_string_pretty(&store.changed(data)) {
        let _ = std::fs::create_dir_all("saves");
        let _ = std::fs::write(SETTINGS_FILE, json);
    }
}

/// Which page list a panel shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Settings,
    Difficulty,
}

impl Group {
    fn key(self) -> &'static str {
        match self {
            Group::Settings => "settings",
            Group::Difficulty => "difficulty",
        }
    }
    fn pages(self, data: &MenuData) -> &[Page] {
        match self {
            Group::Settings => &data.settings,
            Group::Difficulty => &data.difficulty,
        }
    }
}

pub fn row_key(group: &str, page: &str, label: &str) -> String {
    format!("{group}/{page}/{label}")
}

#[derive(Component, Clone, Copy)]
struct CategoryButton {
    group: Group,
    index: usize,
}

/// The white selection bar on a `CategoryButton`.
#[derive(Component)]
struct CategoryBar(Group, usize);

#[derive(Component)]
struct PageContent(Group);

/// Clickable part of a row: toggles a check, or steps a slider/choice by `delta`.
#[derive(Component, Clone)]
struct RowInput {
    key: String,
    row: Row,
    delta: i32,
}

/// Text (or check mark) showing a row's current value.
#[derive(Component, Clone)]
struct RowValue {
    key: String,
    row: Row,
}

/// Slider fill bar.
#[derive(Component, Clone)]
struct RowFill {
    key: String,
    row: Row,
}

/// Left category column + scrolling row list, the layout `SettingsMenu` and the difficulty pages
/// share. `content_left` is where the row list starts (245 in settings, 360 in difficulty).
pub fn spawn_panel(commands: &mut Commands, font: &MenuFont, data: &MenuData, store: &SettingsStore, group: Group, node: Node, content_left: f32) -> Entity {
    let pages = group.pages(data);
    let panel = commands.spawn(node).id();

    let categories = commands
        .spawn(Node { position_type: PositionType::Absolute, flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id();
    commands.entity(panel).add_child(categories);
    for (index, page) in pages.iter().enumerate() {
        let mut e = commands.spawn((
            button(Node { align_items: AlignItems::Center, ..default() }, ButtonPalette::CATEGORY, CategoryButton { group, index }),
            children![
                (Node { width: Val::Px(5.0), height: Val::Px(32.0), ..default() }, BackgroundColor(Color::WHITE.with_alpha(if index == 0 { 1.0 } else { 0.0 })), CategoryBar(group, index)),
                (label(font, page.name.clone(), 18.0), Node { margin: UiRect::new(Val::Px(20.0), Val::Px(80.0), Val::Px(10.0), Val::Px(10.0)), ..default() }),
            ],
        ));
        if index == 0 {
            e.insert(Chosen);
        }
        let id = e.id();
        commands.entity(categories).add_child(id);
    }

    let content = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(content_left),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            PageContent(group),
        ))
        .id();
    commands.entity(panel).add_child(content);
    spawn_rows(commands, font, store, content, group.key(), &pages[0]);
    panel
}

pub fn spawn_rows(commands: &mut Commands, font: &MenuFont, store: &SettingsStore, parent: Entity, group: &str, page: &Page) {
    let mut stripe = false;
    for row in &page.rows {
        if let Some(section) = &row.section {
            let header = commands
                .spawn((label(font, section.clone(), 28.0), Node { margin: UiRect::new(Val::Px(20.0), Val::Px(0.0), Val::Px(20.0), Val::Px(10.0)), ..default() }))
                .id();
            commands.entity(parent).add_child(header);
            continue;
        }
        stripe = !stripe;
        let key = row_key(group, &page.name, &row.label);
        let line = spawn_row(commands, font, store, key, row, stripe, 14.0);
        commands.entity(parent).add_child(line);
    }
}

/// `SettingRow`: alternating border, 14pt label on the left, control right-aligned.
pub fn spawn_row(commands: &mut Commands, font: &MenuFont, store: &SettingsStore, key: String, row: &Row, stripe: bool, label_size: f32) -> Entity {
    let line = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                min_height: Val::Px(46.0),
                padding: UiRect::horizontal(Val::Px(20.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, if stripe { 0.05 } else { 0.0 })),
            children![(label(font, row.label.clone(), label_size), Node { margin: UiRect::all(Val::Px(10.0)), ..default() })],
        ))
        .id();
    let value = store.get(&key, row);
    let controls = commands.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    commands.entity(line).add_child(controls);

    let small = |text: &'static str, delta: i32| {
        (
            button(
                Node { width: Val::Px(32.0), height: Val::Px(32.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                ButtonPalette::MENU,
                RowInput { key: key.clone(), row: row.clone(), delta },
            ),
            children![label(font, text, 14.0)],
        )
    };
    let value_text = |width: f32| {
        (
            Node { width: Val::Px(width), justify_content: JustifyContent::Center, ..default() },
            children![(label(font, format_value(row, value), 14.0), RowValue { key: key.clone(), row: row.clone() })],
        )
    };

    match row.kind {
        RowKind::Check => {
            let b = commands
                .spawn((
                    button(
                        Node {
                            width: Val::Px(32.0),
                            height: Val::Px(32.0),
                            border: UiRect::all(Val::Px(2.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        ButtonPalette::MENU,
                        RowInput { key: key.clone(), row: row.clone(), delta: 0 },
                    ),
                    BorderColor::all(Color::WHITE.with_alpha(0.7)),
                    children![(label(font, format_value(row, value), 16.0), RowValue { key: key.clone(), row: row.clone() })],
                ))
                .id();
            commands.entity(controls).add_child(b);
        }
        RowKind::Slider => {
            let frac = match value {
                Value::Num(n) => ((n - row.min) / (row.max - row.min)).clamp(0.0, 1.0),
                _ => 0.0,
            };
            let left = commands.spawn(small("<", -1)).id();
            let bar = commands
                .spawn((
                    Node { width: Val::Px(220.0), height: Val::Px(8.0), ..default() },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15)),
                    children![(
                        Node { width: Val::Percent(frac * 100.0), height: Val::Percent(100.0), ..default() },
                        BackgroundColor(Color::WHITE),
                        RowFill { key: key.clone(), row: row.clone() },
                    )],
                ))
                .id();
            let text = commands.spawn(value_text(80.0)).id();
            let right = commands.spawn(small(">", 1)).id();
            commands.entity(controls).add_children(&[left, bar, text, right]);
        }
        RowKind::Choice => {
            let left = commands.spawn(small("<", -1)).id();
            let text = commands.spawn(value_text(260.0)).id();
            let right = commands.spawn(small(">", 1)).id();
            commands.entity(controls).add_children(&[left, text, right]);
        }
        RowKind::Key => {
            let keys = if row.keys.is_empty() { "None".to_string() } else { row.keys.iter().map(|k| key_name(k)).collect::<Vec<_>>().join("  /  ") };
            let t = commands
                .spawn((
                    Node { min_width: Val::Px(200.0), padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)), justify_content: JustifyContent::Center, ..default() },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
                    children![label(font, keys, 14.0)],
                ))
                .id();
            commands.entity(controls).add_child(t);
        }
        RowKind::Button => {
            let text = row.text.clone().unwrap_or_else(|| row.label.clone());
            let b = commands.spawn(button_widget(font, &text, 14.0, Node::default(), ())).id();
            commands.entity(controls).add_child(b);
        }
    }
    line
}

/// UE key names -> what the game's key-binding widget prints.
fn key_name(k: &str) -> String {
    match k {
        "SpaceBar" => "Space Bar".into(),
        "LeftMouseButton" => "Left Mouse Button".into(),
        "RightMouseButton" => "Right Mouse Button".into(),
        "MiddleMouseButton" => "Middle Mouse Button".into(),
        "LeftShift" => "Left Shift".into(),
        "LeftControl" => "Left Ctrl".into(),
        "LeftAlt" => "Left Alt".into(),
        "One" => "1".into(),
        "Two" => "2".into(),
        "Three" => "3".into(),
        "Four" => "4".into(),
        "Six" => "6".into(),
        "Seven" => "7".into(),
        "Eight" => "8".into(),
        "Nine" => "9".into(),
        "Hyphen" => "-".into(),
        "Equals" => "=".into(),
        "Semicolon" => ";".into(),
        "Backslash" => "\\".into(),
        other => other.to_string(),
    }
}

fn spawn_settings(mut commands: Commands, font: Res<MenuFont>, data: Res<MenuData>, store: Res<SettingsStore>, app: Res<State<AppState>>) {
    let back = if *app.get() == AppState::InGame { MenuScreen::Pause } else { MenuScreen::Main };
    let root = commands.spawn((screen(MenuScreen::Settings), GlobalZIndex(4), children![backdrop(0.55)])).id();
    let panel = spawn_panel(
        &mut commands,
        &font,
        &data,
        &store,
        Group::Settings,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(20.0),
            top: Val::Px(20.0),
            right: Val::Px(20.0),
            bottom: Val::Px(70.0),
            ..default()
        },
        245.0,
    );
    commands.entity(root).add_child(panel);
    commands.entity(root).with_child(back_button(&font, super::GotoScreen(back)));
}

fn on_category(
    ev: On<Activate>,
    mut commands: Commands,
    cats: Query<(Entity, &CategoryButton)>,
    mut bars: Query<(&CategoryBar, &mut BackgroundColor)>,
    contents: Query<(Entity, &PageContent)>,
    font: Res<MenuFont>,
    data: Res<MenuData>,
    store: Res<SettingsStore>,
) {
    let Ok((_, clicked)) = cats.get(ev.entity) else { return };
    let clicked = *clicked;
    for (e, c) in &cats {
        if c.group == clicked.group {
            if c.index == clicked.index {
                commands.entity(e).insert(Chosen);
            } else {
                commands.entity(e).remove::<Chosen>();
            }
        }
    }
    for (bar, mut bg) in &mut bars {
        if bar.0 == clicked.group {
            bg.0 = Color::WHITE.with_alpha(if bar.1 == clicked.index { 1.0 } else { 0.0 });
        }
    }
    let page = &clicked.group.pages(&data)[clicked.index];
    for (e, content) in &contents {
        if content.0 == clicked.group {
            commands.entity(e).despawn_related::<Children>().insert(ScrollPosition::default());
            spawn_rows(&mut commands, &font, &store, e, clicked.group.key(), page);
        }
    }
}

fn on_row(ev: On<Activate>, inputs: Query<&RowInput>, mut store: ResMut<SettingsStore>) {
    let Ok(input) = inputs.get(ev.entity) else { return };
    let row = &input.row;
    let next = match store.get(&input.key, row) {
        Value::Bool(b) => Value::Bool(!b),
        Value::Num(n) => {
            let step = step_of(row);
            Value::Num((n + step * input.delta as f32).clamp(row.min, row.max))
        }
        Value::Choice(i) => {
            let n = row.options.len().max(1) as i32;
            Value::Choice((i as i32 + input.delta).rem_euclid(n) as usize)
        }
    };
    store.values.insert(input.key.clone(), next);
}

fn refresh_values(
    store: Res<SettingsStore>,
    mut texts: Query<(&RowValue, &mut Text)>,
    mut fills: Query<(&RowFill, &mut Node)>,
) {
    for (rv, mut text) in &mut texts {
        let s = format_value(&rv.row, store.get(&rv.key, &rv.row));
        if text.0 != s {
            text.0 = s;
        }
    }
    for (rf, mut node) in &mut fills {
        if let Value::Num(n) = store.get(&rf.key, &rf.row) {
            node.width = Val::Percent(((n - rf.row.min) / (rf.row.max - rf.row.min)).clamp(0.0, 1.0) * 100.0);
        }
    }
}

/// Push the handful of settings Bevy can act on today into the engine.
fn apply_settings(
    store: Res<SettingsStore>,
    mut volume: ResMut<GlobalVolume>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let get = |page: &str, label: &str| store.find(&row_key("settings", page, label));
    if let Some(Value::Num(v)) = get("Audio", "MASTER") {
        volume.volume = Volume::Linear(v);
    }
    let Ok(mut window) = windows.single_mut() else { return };
    if let Some(Value::Choice(i)) = get("Video", "WINDOW MODE") {
        let mode = match i {
            1 => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            2 => WindowMode::Fullscreen(MonitorSelection::Current, bevy::window::VideoModeSelection::Current),
            _ => WindowMode::Windowed,
        };
        if window.mode != mode {
            window.mode = mode;
        }
    }
    if let Some(Value::Bool(vsync)) = get("Video", "VERTICAL SYNC") {
        let mode = if vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
        if window.present_mode != mode {
            window.present_mode = mode;
        }
    }
    if let Some(Value::Choice(i)) = get("Video", "RESOLUTION") {
        let res = ["1280x720", "1600x900", "1920x1080", "2560x1440", "3840x2160"];
        if let Some((w, h)) = res.get(i).and_then(|r| r.split_once('x')) {
            let (w, h): (u32, u32) = (w.parse().unwrap_or(1280), h.parse().unwrap_or(720));
            if window.mode == WindowMode::Windowed && (window.resolution.physical_width(), window.resolution.physical_height()) != (w, h) {
                window.resolution.set_physical_resolution(w, h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> super::super::MenuData {
        serde_json::from_str(&std::fs::read_to_string("assets/data/menus.json").unwrap()).unwrap()
    }

    #[test]
    fn old_files_take_the_games_defaults_and_saves_keep_only_changes() {
        let data = data();
        let key = row_key("difficulty", "Vehicles", "VEHICLE SPAWN CHANCE %");
        let day = row_key("difficulty", "General", "DAY LENGTH");
        // An old file: every row stored, the vehicle chance at the old 1.0 guess.
        let mut store = SettingsStore { values: HashMap::from([(key.clone(), Value::Num(1.0)), (day.clone(), Value::Num(45.0))]), version: 0 };
        store.fill_defaults(&data);
        assert_eq!(store.find(&key), Some(Value::Num(30.0)));
        // A real choice survives.
        assert_eq!(store.find(&day), Some(Value::Num(45.0)));
        assert_eq!(store.version, VERSION);
        // A new file's 1 % is the player's own and stays.
        let mut store = SettingsStore { values: HashMap::from([(key.clone(), Value::Num(1.0))]), version: VERSION };
        store.fill_defaults(&data);
        assert_eq!(store.find(&key), Some(Value::Num(1.0)));
        // Saving drops what equals the default.
        let saved = store.changed(&data);
        assert_eq!(saved.values.len(), 1);
        assert_eq!(saved.values.get(&key), Some(&Value::Num(1.0)));
    }

    #[test]
    fn difficulty_presets_set_the_dt_difficulty_rows() {
        let data = data();
        assert_eq!(data.difficulty_presets.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Peaceful", "Easy", "Standard", "Hardened", "Nightmare"]);
        let mut store = SettingsStore::default();
        store.fill_defaults(&data);
        let num = |s: &SettingsStore, page: &str, label: &str| match s.find(&row_key("difficulty", page, label)) {
            Some(Value::Num(v)) => v,
            other => panic!("{label}: {other:?}"),
        };
        assert!(store.apply_preset(&data, "Nightmare"));
        assert_eq!(num(&store, "AI", "ZOMBIE HEALTH MULTIPLIER"), 1.75);
        assert_eq!(num(&store, "AI", "ZOMBIE SPEED MULTIPLIER"), 2.0);
        assert_eq!(num(&store, "Player", "HUNGER DEPLETION MULTIPLIER"), 1.25);
        assert_eq!(store.find(&row_key("difficulty", "Player", "RESET PASSIVE SKILLS ON DEATH?")), Some(Value::Bool(true)));
        assert!(store.apply_preset(&data, "Peaceful"));
        assert_eq!(num(&store, "AI", "ZOMBIE SPAWN AMOUNT MULTIPLIER"), 0.0);
        assert_eq!(store.find(&row_key("difficulty", "Player", "KEEP INVENTORY ON DEATH?")), Some(Value::Bool(true)));
        // Loot rarity isn't part of SetDifficulty; unknown presets change nothing.
        assert!(!store.apply_preset(&data, "Impossible"));
    }
}
