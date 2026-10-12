use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::engine::GameState;
use crate::engine::difficulty::DifficultyLevel;

/// `assets/data/menus.json`, generated from the menu blueprints by `extractor/gen_menu_data.py`.
#[derive(Resource, Deserialize, Debug, Clone)]
pub struct MenuData {
    pub settings: Vec<Page>,
    pub difficulty: Vec<Page>,
    pub creator: CreatorData,
    pub new_game_rows: Vec<Row>,
    /// `DT_Difficulty` rows (Peaceful..Nightmare) as the difficulty rows `SetDifficulty` writes.
    #[serde(default)]
    pub difficulty_presets: Vec<DifficultyPreset>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DifficultyPreset {
    pub name: String,
    pub values: Vec<PresetValue>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PresetValue {
    pub page: String,
    pub label: String,
    pub value: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Page {
    pub name: String,
    pub rows: Vec<Row>,
}

/// One line of a settings page: either a section header or a labelled control.
#[derive(Deserialize, Debug, Clone)]
pub struct Row {
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: RowKind,
    #[serde(default)]
    pub min: f32,
    #[serde(default)]
    pub max: f32,
    #[serde(default)]
    pub digits: usize,
    #[serde(default = "one")]
    pub multiplier: f32,
    #[serde(default)]
    pub cvar: Option<String>,
    /// The game's starting value (difficulty sliders: `Save_Difficulties` defaults).
    #[serde(default)]
    pub default: Option<f32>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub text: Option<String>,
}

fn one() -> f32 {
    1.0
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RowKind {
    #[default]
    Check,
    Slider,
    Choice,
    Key,
    Button,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreatorData {
    pub default_forename: String,
    pub default_surname: String,
    pub occupations: Vec<String>,
    pub body: Vec<CreatorRow>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreatorRow {
    pub label: String,
    pub options: Vec<String>,
    pub default: String,
}

impl MenuData {
    pub fn load() -> Self {
        let text = std::fs::read_to_string("assets/data/menus.json")
            .expect("assets/data/menus.json missing (run extractor/export_assets.sh --menu-data)");
        serde_json::from_str(&text).expect("assets/data/menus.json is malformed")
    }
}

/// The survivor built in the character creator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterProfile {
    pub forename: String,
    pub surname: String,
    pub age: String,
    pub occupation: usize,
    /// Index into each `CreatorData::body` row's options, same order.
    pub body: Vec<usize>,
}

impl CharacterProfile {
    pub fn new(data: &CreatorData) -> Self {
        Self {
            forename: data.default_forename.clone(),
            surname: data.default_surname.clone(),
            age: "30".into(),
            occupation: 0,
            body: data
                .body
                .iter()
                .map(|r| r.options.iter().position(|o| *o == r.default).unwrap_or(0))
                .collect(),
        }
    }
}

/// The session being played (or about to start): simulation state plus front-end choices.
#[derive(Resource, Default)]
pub struct CurrentGame {
    pub state: Option<GameState>,
    pub profile: Option<CharacterProfile>,
    pub difficulty: DifficultyLevel,
    pub permadeath: bool,
    /// Save slot this session writes to on Save / Quick Save.
    pub slot: Option<String>,
    /// What the survivor carries (saved beside the slot as `<slot>.inventory.json`).
    pub inventory: Option<crate::gameplay::equipment::PlayerInventory>,
}
