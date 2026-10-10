use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum KeycardTier {
    Level0,    // Maintenance & Checkpoint Gates
    Level1,    // Research Lab Outer Security
    Level2,    // Military Armory Blast Doors
    Level3,    // Black Site Executive Command Vault
    AllAccess, // Master Access Card
}

impl KeycardTier {
    pub fn required_item_id(&self) -> &'static str {
        match self {
            KeycardTier::Level0 => "keycard_lvl0",
            KeycardTier::Level1 => "keycard_lvl1",
            KeycardTier::Level2 => "keycard_lvl2",
            KeycardTier::Level3 => "keycard_lvl3",
            KeycardTier::AllAccess => "keycard_all_access",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            KeycardTier::Level0 => "Keycard [Level 0 - Maintenance]",
            KeycardTier::Level1 => "Keycard [Level 1 - Research Facility]",
            KeycardTier::Level2 => "Keycard [Level 2 - Military Armory]",
            KeycardTier::Level3 => "Keycard [Level 3 - Black Site Terminal]",
            KeycardTier::AllAccess => "Master Keycard [All-Access Clearance]",
        }
    }

    pub fn from_item_id(id: &str) -> Option<Self> {
        match id {
            "keycard_lvl0" => Some(KeycardTier::Level0),
            "keycard_lvl1" => Some(KeycardTier::Level1),
            "keycard_lvl2" => Some(KeycardTier::Level2),
            "keycard_lvl3" => Some(KeycardTier::Level3),
            "keycard_all_access" | "all_access_keycard" => Some(KeycardTier::AllAccess),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BunkerSecurityDoor {
    pub id: u32,
    pub name: String,
    pub position: Position3D,
    pub required_tier: KeycardTier,
    pub is_unlocked: bool,
    pub is_open: bool,
    pub interaction_radius_m: f32,
    pub card_reader_pos: Position3D,
}

impl BunkerSecurityDoor {
    pub fn new(
        id: u32,
        name: impl Into<String>,
        position: Position3D,
        required_tier: KeycardTier,
    ) -> Self {
        let name_str = name.into();
        let card_reader_pos = Position3D::new(position.x + 2.0, position.y, position.z);
        Self {
            id,
            name: name_str,
            position,
            required_tier,
            is_unlocked: false,
            is_open: false,
            interaction_radius_m: 3.5,
            card_reader_pos,
        }
    }

    pub fn can_interact(&self, player_pos: &Position3D) -> bool {
        self.position.distance_2d(player_pos) <= self.interaction_radius_m
    }

    pub fn try_toggle(&mut self, player_highest_keycard: Option<KeycardTier>) -> Result<bool, String> {
        if self.is_unlocked {
            self.is_open = !self.is_open;
            return Ok(self.is_open);
        }

        if let Some(tier) = player_highest_keycard {
            if tier >= self.required_tier || tier == KeycardTier::AllAccess {
                self.is_unlocked = true;
                self.is_open = true;
                return Ok(true);
            }
        }

        Err(format!(
            "ACCESS DENIED: Electronic mag-lock requires clearance [{}].",
            self.required_tier.display_name()
        ))
    }
}

pub fn generate_valley_bunker_doors() -> Vec<BunkerSecurityDoor> {
    vec![
        BunkerSecurityDoor::new(
            1,
            "Valley Research Lab Alpha — Air Lock Bulkhead",
            Position3D::new(-115.0, 0.0, 115.0),
            KeycardTier::Level1,
        ),
        BunkerSecurityDoor::new(
            2,
            "FOB Bravo Underground Armory — Vault Blast Door",
            Position3D::new(128.0, 0.0, -92.0),
            KeycardTier::Level2,
        ),
        BunkerSecurityDoor::new(
            3,
            "Black Site Omega — Classified Command Chamber",
            Position3D::new(185.0, 0.0, 155.0),
            KeycardTier::Level3,
        ),
    ]
}
