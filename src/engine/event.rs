use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    GunshotFired {
        weapon_name: String,
        position: Position3D,
        noise_radius: f32,
    },
    ZombieHit {
        zombie_id: u32,
        damage: f32,
        hit_zone: String,
        killed: bool,
    },
    PlayerHurt {
        damage: f32,
        source: String,
        caused_bleeding: bool,
    },
    ItemConsumed {
        item_name: String,
    },
    ContainerLooted {
        container_name: String,
        items_count: usize,
    },
    StructurePlaced {
        structure_name: String,
        position: Position3D,
    },
    VehicleBoarded {
        vehicle_name: String,
    },
    VehicleExited {
        vehicle_name: String,
    },
}
