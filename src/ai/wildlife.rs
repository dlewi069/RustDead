use crate::ai::perception::NoiseEvent;
use crate::items::{Item, ItemType, Rarity};
use crate::math::Position3D;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WildlifeType {
    WhiteTailedDeer,
    ForestRabbit,
    WildBoar,
}

impl WildlifeType {
    pub fn display_name(&self) -> &'static str {
        match self {
            WildlifeType::WhiteTailedDeer => "White-Tailed Deer",
            WildlifeType::ForestRabbit => "Forest Hare",
            WildlifeType::WildBoar => "Wild Boar",
        }
    }

    pub fn max_health(&self) -> f32 {
        match self {
            WildlifeType::WhiteTailedDeer => 45.0,
            WildlifeType::ForestRabbit => 15.0,
            WildlifeType::WildBoar => 65.0,
        }
    }

    pub fn flee_speed(&self) -> f32 {
        match self {
            WildlifeType::WhiteTailedDeer => 7.5,
            WildlifeType::ForestRabbit => 8.2,
            WildlifeType::WildBoar => 6.0,
        }
    }

    pub fn detection_radius(&self) -> f32 {
        match self {
            WildlifeType::WhiteTailedDeer => 28.0,
            WildlifeType::ForestRabbit => 22.0,
            WildlifeType::WildBoar => 18.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wildlife {
    pub id: u32,
    pub wildlife_type: WildlifeType,
    pub name: String,
    pub position: Position3D,
    pub home_origin: Position3D,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub is_fleeing: bool,
    pub flee_timer: f32,
    pub wander_timer: f32,
}

impl Wildlife {
    pub fn new(id: u32, wildlife_type: WildlifeType, position: Position3D) -> Self {
        let max_hp = wildlife_type.max_health();
        let name = wildlife_type.display_name().to_string();
        Self {
            id,
            wildlife_type,
            name,
            position,
            home_origin: position,
            yaw: 0.0,
            health: max_hp,
            max_health: max_hp,
            is_fleeing: false,
            flee_timer: 0.0,
            wander_timer: 3.0,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.health > 0.0
    }

    pub fn take_damage(&mut self, damage: f32) -> bool {
        if !self.is_alive() {
            return false;
        }

        self.health = (self.health - damage).max(0.0);
        if self.health <= 0.0 {
            true // killed
        } else {
            // Panic and sprint away when wounded!
            self.is_fleeing = true;
            self.flee_timer = 6.0;
            false
        }
    }

    pub fn update(&mut self, player_pos: &Position3D, noises: &[NoiseEvent], dt: f32) {
        if !self.is_alive() {
            return;
        }

        // 1. Noise perception: Spooked by nearby gunshots or vehicle engines
        for noise in noises {
            if noise.position.distance_2d(&self.position) <= noise.radius_m * 0.75 {
                self.is_fleeing = true;
                self.flee_timer = 5.0;
                // Run directly away from noise source
                let dx = self.position.x - noise.position.x;
                let dz = self.position.z - noise.position.z;
                self.yaw = dx.atan2(dz).to_degrees();
                break;
            }
        }

        // 2. Proximity perception: Spooked if survivor approaches within detection radius
        let player_dist = self.position.distance_2d(player_pos);
        if player_dist < self.wildlife_type.detection_radius() {
            self.is_fleeing = true;
            self.flee_timer = 5.0;
            // Face away from player
            let dx = self.position.x - player_pos.x;
            let dz = self.position.z - player_pos.z;
            self.yaw = dx.atan2(dz).to_degrees();
        }

        // 3. Movement logic
        if self.is_fleeing {
            self.flee_timer -= dt;
            if self.flee_timer <= 0.0 {
                self.is_fleeing = false;
                self.wander_timer = 4.0;
            } else {
                let speed = self.wildlife_type.flee_speed();
                let yaw_rad = self.yaw.to_radians();
                self.position.x += yaw_rad.sin() * speed * dt;
                self.position.z += yaw_rad.cos() * speed * dt;
            }
        } else {
            // Peaceful wandering/grazing around home origin
            self.wander_timer -= dt;
            if self.wander_timer <= 0.0 {
                let mut rng = rand::thread_rng();
                self.wander_timer = rng.gen_range(4.0..9.0);
                let ox = rng.gen_range(-12.0..12.0);
                let oz = rng.gen_range(-12.0..12.0);
                let target_x = self.home_origin.x + ox;
                let target_z = self.home_origin.z + oz;
                let dx = target_x - self.position.x;
                let dz = target_z - self.position.z;
                self.yaw = dx.atan2(dz).to_degrees();
            }
        }
    }

    /// Generates harvested meat, pelt, and hides upon animal being hunted
    pub fn generate_drops(&self) -> Vec<Item> {
        let mut drops = Vec::new();
        match self.wildlife_type {
            WildlifeType::WhiteTailedDeer => {
                let venison = Item::new(
                    "raw_venison",
                    "Raw Venison (Meat)",
                    ItemType::Food,
                    Rarity::Common,
                    0.5,
                    1,
                    1,
                    5,
                )
                .with_stack(2);
                drops.push(venison);

                let hide = Item::new(
                    "animal_hide",
                    "Cured Deer Hide",
                    ItemType::Material,
                    Rarity::Common,
                    0.8,
                    1,
                    2,
                    10,
                )
                .with_stack(2);
                drops.push(hide);
            }
            WildlifeType::ForestRabbit => {
                let venison = Item::new(
                    "raw_venison",
                    "Raw Hare Meat",
                    ItemType::Food,
                    Rarity::Common,
                    0.3,
                    1,
                    1,
                    5,
                )
                .with_stack(1);
                drops.push(venison);
            }
            WildlifeType::WildBoar => {
                let venison = Item::new(
                    "raw_venison",
                    "Raw Boar Pork",
                    ItemType::Food,
                    Rarity::Common,
                    0.6,
                    1,
                    1,
                    5,
                )
                .with_stack(3);
                drops.push(venison);

                let hide = Item::new(
                    "animal_hide",
                    "Thick Boar Hide",
                    ItemType::Material,
                    Rarity::Uncommon,
                    1.2,
                    1,
                    2,
                    10,
                )
                .with_stack(3);
                drops.push(hide);
            }
        }
        drops
    }
}
