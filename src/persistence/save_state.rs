use crate::engine::GameState;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

pub struct SaveSystem;

impl SaveSystem {
    pub fn save_to_file(state: &GameState, path: impl AsRef<Path>) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> std::io::Result<GameState> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let state: GameState = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(state)
    }
}
