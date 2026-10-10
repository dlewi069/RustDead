use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DifficultyLevel {
    Peaceful,
    Easy,
    Standard,
    Hardened,
    Nightmare,
}

impl Default for DifficultyLevel {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DifficultyModifiers {
    pub level: DifficultyLevel,
    pub player_damage_taken_mult: f32,
    pub zombie_health_mult: f32,
    pub zombie_damage_mult: f32,
    pub zombie_sprint_chance: f32,
    pub hunger_thirst_drain_mult: f32,
    pub infection_rate_mult: f32,
    pub loot_quantity_mult: f32,
    pub hostile_bandits: bool,
}

impl DifficultyModifiers {
    pub fn for_level(level: DifficultyLevel) -> Self {
        match level {
            DifficultyLevel::Peaceful => Self {
                level,
                player_damage_taken_mult: 0.0,
                zombie_health_mult: 0.5,
                zombie_damage_mult: 0.0,
                zombie_sprint_chance: 0.0,
                hunger_thirst_drain_mult: 0.5,
                infection_rate_mult: 0.0,
                loot_quantity_mult: 1.5,
                hostile_bandits: false,
            },
            DifficultyLevel::Easy => Self {
                level,
                player_damage_taken_mult: 0.7,
                zombie_health_mult: 0.75,
                zombie_damage_mult: 0.7,
                zombie_sprint_chance: 0.1,
                hunger_thirst_drain_mult: 0.75,
                infection_rate_mult: 0.6,
                loot_quantity_mult: 1.25,
                hostile_bandits: true,
            },
            DifficultyLevel::Standard => Self {
                level,
                player_damage_taken_mult: 1.0,
                zombie_health_mult: 1.0,
                zombie_damage_mult: 1.0,
                zombie_sprint_chance: 0.25,
                hunger_thirst_drain_mult: 1.0,
                infection_rate_mult: 1.0,
                loot_quantity_mult: 1.0,
                hostile_bandits: true,
            },
            DifficultyLevel::Hardened => Self {
                level,
                player_damage_taken_mult: 1.4,
                zombie_health_mult: 1.35,
                zombie_damage_mult: 1.4,
                zombie_sprint_chance: 0.45,
                hunger_thirst_drain_mult: 1.3,
                infection_rate_mult: 1.4,
                loot_quantity_mult: 0.8,
                hostile_bandits: true,
            },
            DifficultyLevel::Nightmare => Self {
                level,
                player_damage_taken_mult: 2.0,
                zombie_health_mult: 1.8,
                zombie_damage_mult: 2.0,
                zombie_sprint_chance: 0.75,
                hunger_thirst_drain_mult: 1.75,
                infection_rate_mult: 2.0,
                loot_quantity_mult: 0.5,
                hostile_bandits: true,
            },
        }
    }
}
