use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZombieType {
    Walker,
    Runner,
    Bloater,
    Juggernaut,
    Screamer,
    Spitter,
    Hound,
}

impl ZombieType {
    pub fn xp_reward(&self) -> u64 {
        match self {
            ZombieType::Walker => 20,
            ZombieType::Runner => 35,
            ZombieType::Hound => 30,
            ZombieType::Screamer => 45,
            ZombieType::Spitter => 55,
            ZombieType::Bloater => 75,
            ZombieType::Juggernaut => 200,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiState {
    Idle,
    Wander,
    Investigate,
    Chase,
    Attack,
    Staggered,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zombie {
    pub id: u32,
    pub zombie_type: ZombieType,
    pub position: Position3D,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub speed_walk: f32,
    pub speed_run: f32,
    pub attack_damage: f32,
    pub attack_range: f32,
    pub attack_cooldown_s: f32,
    pub attack_timer: f32,
    pub state: AiState,
    pub state_timer: f32,
    pub target_position: Option<Position3D>,
    pub target_entity_id: Option<u32>,
    pub stagger_timer: f32,
}

impl Zombie {
    pub fn new(id: u32, zombie_type: ZombieType, position: Position3D) -> Self {
        let (max_hp, walk, run, dmg, range, cd) = match zombie_type {
            ZombieType::Walker => (70.0, 1.2, 2.5, 18.0, 1.8, 1.4),
            ZombieType::Runner => (45.0, 2.0, 5.5, 14.0, 1.7, 1.0),
            ZombieType::Bloater => (160.0, 0.9, 2.0, 45.0, 2.5, 2.0),
            ZombieType::Juggernaut => (450.0, 1.5, 4.2, 55.0, 2.2, 1.8),
            ZombieType::Screamer => (55.0, 1.3, 4.0, 12.0, 1.6, 1.2),
            ZombieType::Spitter => (85.0, 1.0, 3.0, 25.0, 14.0, 2.5),
            ZombieType::Hound => (40.0, 2.5, 6.5, 22.0, 1.5, 0.9),
        };

        Self {
            id,
            zombie_type,
            position,
            yaw: 0.0,
            health: max_hp,
            max_health: max_hp,
            speed_walk: walk,
            speed_run: run,
            attack_damage: dmg,
            attack_range: range,
            attack_cooldown_s: cd,
            attack_timer: 0.0,
            state: AiState::Idle,
            state_timer: 2.0,
            target_position: None,
            target_entity_id: None,
            stagger_timer: 0.0,
        }
    }

    pub fn take_damage(&mut self, amount: f32) -> bool {
        if self.state == AiState::Dead {
            return false;
        }

        self.health = (self.health - amount).max(0.0);

        if self.health <= 0.0 {
            self.state = AiState::Dead;
            true // Died
        } else {
            // High damage hit staggers the zombie
            if amount >= 35.0 && self.zombie_type != ZombieType::Juggernaut {
                self.state = AiState::Staggered;
                self.stagger_timer = 0.45;
            }
            false
        }
    }

    pub fn is_alive(&self) -> bool {
        self.state != AiState::Dead && self.health > 0.0
    }
}
