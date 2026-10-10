use super::lockpick::{LockDifficulty, LockMechanism};
use super::loot_table::{LootTable, LootTier};
use crate::items::GridContainer;
use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootContainer {
    pub id: u32,
    pub name: String,
    pub position: Position3D,
    pub interaction_radius_m: f32,
    pub storage: GridContainer,
    pub is_searched: bool,
    pub lock: Option<LockMechanism>,
}

impl LootContainer {
    pub fn new(id: u32, name: impl Into<String>, position: Position3D, tier: LootTier, w: u8, h: u8) -> Self {
        let name_str = name.into();
        let mut storage = GridContainer::new(format!("container_{}", id), &name_str, w, h, 80.0);

        // Populate with randomized tier loot
        let loot_count = (rand::random::<u32>() % 3 + 2) as usize;
        let loot = LootTable::roll_loot(tier, loot_count);
        for item in loot {
            let _ = storage.auto_insert(item);
        }

        Self {
            id,
            name: name_str,
            position,
            interaction_radius_m: 2.5,
            storage,
            is_searched: false,
            lock: None,
        }
    }

    pub fn with_lock(mut self, difficulty: LockDifficulty) -> Self {
        self.lock = Some(LockMechanism::new(difficulty));
        self
    }

    pub fn is_locked(&self) -> bool {
        self.lock.as_ref().map(|l| l.is_locked).unwrap_or(false)
    }

    pub fn can_interact(&self, player_pos: &Position3D) -> bool {
        self.position.distance_to(player_pos) <= self.interaction_radius_m
    }
}
