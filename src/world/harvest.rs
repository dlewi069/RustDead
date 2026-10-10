use crate::items::{create_stock_item, Item};
use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarvestNodeType {
    PineTree,
    RockBoulder,
}

impl HarvestNodeType {
    pub fn name(&self) -> &'static str {
        match self {
            HarvestNodeType::PineTree => "Pine Tree",
            HarvestNodeType::RockBoulder => "Granite Boulder",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestNode {
    pub id: u32,
    pub node_type: HarvestNodeType,
    pub position: Position3D,
    pub health: f32,
    pub max_health: f32,
    pub is_depleted: bool,
    pub respawn_timer_s: f32,
}

impl HarvestNode {
    pub fn new(id: u32, node_type: HarvestNodeType, position: Position3D) -> Self {
        let max_hp = match node_type {
            HarvestNodeType::PineTree => 75.0,
            HarvestNodeType::RockBoulder => 100.0,
        };
        Self {
            id,
            node_type,
            position,
            health: max_hp,
            max_health: max_hp,
            is_depleted: false,
            respawn_timer_s: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if self.is_depleted {
            self.respawn_timer_s -= dt;
            if self.respawn_timer_s <= 0.0 {
                self.is_depleted = false;
                self.health = self.max_health;
            }
        }
    }

    /// Strike the node with tool, weapon or bare hands
    pub fn strike(&mut self, damage: f32) -> Option<(Item, u32, bool)> {
        if self.is_depleted {
            return None;
        }

        self.health -= damage;
        let was_felled = if self.health <= 0.0 {
            self.health = 0.0;
            self.is_depleted = true;
            self.respawn_timer_s = 90.0;
            true
        } else {
            false
        };

        let (item_id, count, xp) = match self.node_type {
            HarvestNodeType::PineTree => {
                if was_felled {
                    ("wood_log", 2, 25)
                } else {
                    ("wood_plank", 1, 10)
                }
            }
            HarvestNodeType::RockBoulder => {
                if was_felled {
                    ("scrap_metal", 2, 30)
                } else {
                    ("stone", 2, 12)
                }
            }
        };

        if let Some(mut item) = create_stock_item(item_id) {
            item = item.with_stack(count);
            Some((item, xp, was_felled))
        } else {
            None
        }
    }

    /// Generate initial distribution of forest pine trees and rock outcrops
    pub fn generate_valley_nodes() -> Vec<Self> {
        let mut nodes = Vec::new();
        let mut id = 1;

        // Pine grove around player spawn and valley roads
        let tree_coords = [
            (-12.0, 16.0), (-16.0, 22.0), (-8.0, 28.0), (18.0, 12.0), (22.0, 18.0),
            (26.0, 32.0), (-25.0, 50.0), (-32.0, 62.0), (14.0, 65.0), (20.0, 80.0),
            (-18.0, -22.0), (-28.0, -35.0), (15.0, -25.0), (24.0, -42.0), (-35.0, 15.0),
            (35.0, -15.0), (-40.0, -60.0), (42.0, 55.0),
        ];
        for (x, z) in tree_coords {
            nodes.push(Self::new(id, HarvestNodeType::PineTree, Position3D::new(x, 0.0, z)));
            id += 1;
        }

        // Rock boulders around hillsides and mountain borders
        let rock_coords = [
            (-22.0, 30.0), (25.0, 28.0), (-14.0, 48.0), (32.0, 45.0), (-30.0, -18.0),
            (28.0, -28.0), (-45.0, 40.0), (45.0, -45.0), (-10.0, 75.0), (12.0, 92.0),
        ];
        for (x, z) in rock_coords {
            nodes.push(Self::new(id, HarvestNodeType::RockBoulder, Position3D::new(x, 0.0, z)));
            id += 1;
        }

        nodes
    }
}
