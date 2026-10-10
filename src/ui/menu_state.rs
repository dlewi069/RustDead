use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveMenu {
    None,
    MainMenu,
    PauseMenu,
    CharacterCreator,
    Settings,
    DifficultySettings,
    Inventory,
    Crafting,
    SkillTree,
    Trader,
    MedicalTrauma,
    Lockpicking,
    WorldMap,
    EmergencyRadio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterCustomization {
    pub face_index: usize,
    pub facial_detail: String,
    pub facial_hair: String,
    pub hair_color: String,
    pub gender: String,
}

impl Default for CharacterCustomization {
    fn default() -> Self {
        Self {
            face_index: 0,
            facial_detail: "None".to_string(),
            facial_hair: "None".to_string(),
            hair_color: "Brown".to_string(),
            gender: "Male".to_string(),
        }
    }
}

pub const FACIAL_DETAILS: &[&str] = &[
    "None", "Eyebags", "Freckles", "Nose Bandage", "Nose Piercing",
    "Scar 1", "Scar 2", "Scar 3", "Tattoo 1", "Tattoo 2", "Tattoo 3",
    "War Paint 1", "War Paint 2", "War Paint 3",
];

pub const FACIAL_HAIR: &[&str] = &[
    "None", "Beard 1", "Beard 2", "Beard 3", "Beard 4",
    "Goatee 1", "Goatee 2", "Moustache 1", "Moustache 2", "Moustache 3",
];

pub const HAIR_COLORS: &[&str] = &[
    "Brown", "Light Brown", "Dark Brown", "Blonde", "Light Blonde",
    "Black", "Gray", "White", "Pink", "Purple", "Blue", "Ginger",
];
