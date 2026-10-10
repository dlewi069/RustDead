use crate::engine::difficulty::DifficultyLevel;
use crate::engine::GameState;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSlotMetadata {
    pub slot_id: String,
    pub display_name: String,
    pub timestamp_epoch_secs: u64,
    pub playtime_seconds: f32,
    pub character_level: u32,
    pub difficulty: DifficultyLevel,
    pub location_desc: String,
    pub player_hp: f32,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveContainer {
    pub metadata: SaveSlotMetadata,
    pub game_state: GameState,
}

pub struct SaveSlotManager;

impl SaveSlotManager {
    fn slot_path(base_dir: &Path, slot_id: &str) -> PathBuf {
        base_dir.join(format!("{}.sav.json", slot_id))
    }

    pub fn save_slot(
        state: &GameState,
        slot_id: &str,
        display_name: &str,
        base_dir: &Path,
    ) -> std::io::Result<SaveSlotMetadata> {
        fs::create_dir_all(base_dir)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let metadata = SaveSlotMetadata {
            slot_id: slot_id.to_string(),
            display_name: display_name.to_string(),
            timestamp_epoch_secs: current_time,
            playtime_seconds: (state.tick_count as f32) / 60.0,
            character_level: state.progression.level,
            difficulty: state.difficulty.level,
            location_desc: format!(
                "X: {:.1}, Z: {:.1} (Sector {})",
                state.player_pos.x,
                state.player_pos.z,
                if state.active_bunker.is_some() { "Bunker Subterranean" } else { "Valley Surface" }
            ),
            player_hp: state.survival.health,
            file_name: format!("{}.sav.json", slot_id),
        };

        let container = SaveContainer {
            metadata: metadata.clone(),
            game_state: state.clone(),
        };

        let json = serde_json::to_string_pretty(&container)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let file_path = Self::slot_path(base_dir, slot_id);
        let mut file = File::create(file_path)?;
        file.write_all(json.as_bytes())?;

        Ok(metadata)
    }

    pub fn load_slot(slot_id: &str, base_dir: &Path) -> std::io::Result<SaveContainer> {
        let file_path = Self::slot_path(base_dir, slot_id);
        let mut file = File::open(file_path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        let container: SaveContainer = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(container)
    }

    pub fn list_slots(base_dir: &Path) -> Vec<SaveSlotMetadata> {
        let mut list = Vec::new();
        if let Ok(entries) = fs::read_dir(base_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json")
                    && path.to_string_lossy().contains(".sav.")
                {
                    if let Ok(mut f) = File::open(&path) {
                        let mut buf = String::new();
                        if f.read_to_string(&mut buf).is_ok() {
                            if let Ok(cont) = serde_json::from_str::<SaveContainer>(&buf) {
                                list.push(cont.metadata);
                            }
                        }
                    }
                }
            }
        }
        list.sort_by(|a, b| b.timestamp_epoch_secs.cmp(&a.timestamp_epoch_secs));
        list
    }

    pub fn quick_save(state: &GameState, base_dir: &Path) -> std::io::Result<SaveSlotMetadata> {
        Self::save_slot(state, "quicksave", "Quick Save [F5]", base_dir)
    }

    pub fn quick_load(base_dir: &Path) -> std::io::Result<SaveContainer> {
        Self::load_slot("quicksave", base_dir)
    }
}
