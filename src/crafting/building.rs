use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructureType {
    WoodenWall,
    WoodenDoor,
    Barricade,
    Campfire,
    SleepingBag,
    WaterWell,
    Workbench,
    StorageChest,
    HordeBeacon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedStructure {
    pub id: u32,
    pub structure_type: StructureType,
    pub position: Position3D,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub is_open: bool, // for doors
    pub is_lit: bool,  // for campfire
}

impl PlacedStructure {
    pub fn new(id: u32, structure_type: StructureType, position: Position3D, yaw: f32) -> Self {
        let max_hp = match structure_type {
            StructureType::WoodenWall => 350.0,
            StructureType::WoodenDoor => 250.0,
            StructureType::Barricade => 200.0,
            StructureType::Campfire => 50.0,
            StructureType::SleepingBag => 40.0,
            StructureType::WaterWell => 500.0,
            StructureType::Workbench => 300.0,
            StructureType::StorageChest => 200.0,
            StructureType::HordeBeacon => 500.0,
        };

        Self {
            id,
            structure_type,
            position,
            yaw,
            health: max_hp,
            max_health: max_hp,
            is_open: false,
            is_lit: false,
        }
    }

    pub fn take_damage(&mut self, amount: f32) -> bool {
        self.health = (self.health - amount).max(0.0);
        self.health <= 0.0 // True if destroyed
    }

    pub fn repair(&mut self, amount: f32) {
        self.health = (self.health + amount).min(self.max_health);
    }

    pub fn toggle_door(&mut self) -> bool {
        if self.structure_type == StructureType::WoodenDoor {
            self.is_open = !self.is_open;
            true
        } else {
            false
        }
    }

    pub fn toggle_campfire(&mut self) -> bool {
        if self.structure_type == StructureType::Campfire {
            self.is_lit = !self.is_lit;
            true
        } else {
            false
        }
    }
}
