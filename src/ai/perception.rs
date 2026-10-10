use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseEvent {
    pub position: Position3D,
    pub radius_m: f32,
    pub intensity: f32,
    pub source_entity: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ZombiePerception {
    pub view_angle_deg: f32,
    pub day_sight_range_m: f32,
    pub night_sight_range_m: f32,
}

impl Default for ZombiePerception {
    fn default() -> Self {
        Self {
            view_angle_deg: 120.0,
            day_sight_range_m: 35.0,
            night_sight_range_m: 14.0,
        }
    }
}

impl ZombiePerception {
    pub fn can_see_player(
        &self,
        zombie_pos: &Position3D,
        zombie_yaw: f32,
        player_pos: &Position3D,
        is_daytime: bool,
        player_flashlight_on: bool,
    ) -> bool {
        let dist = zombie_pos.distance_to(player_pos);
        let max_sight = if player_flashlight_on {
            // Flashlight makes player highly visible at night!
            self.day_sight_range_m * 1.25
        } else if is_daytime {
            self.day_sight_range_m
        } else {
            self.night_sight_range_m
        };

        if dist > max_sight {
            return false;
        }

        // Close proximity detection regardless of facing
        if dist < 3.0 {
            return true;
        }

        // Check horizontal angle
        let dx = player_pos.x - zombie_pos.x;
        let dz = player_pos.z - zombie_pos.z;
        let angle_to_player = dx.atan2(dz).to_degrees();

        let mut diff = (angle_to_player - zombie_yaw).abs();
        if diff > 180.0 {
            diff = 360.0 - diff;
        }

        diff <= (self.view_angle_deg / 2.0)
    }

    pub fn can_hear_noise(&self, zombie_pos: &Position3D, noise: &NoiseEvent) -> bool {
        let dist = zombie_pos.distance_to(&noise.position);
        dist <= noise.radius_m
    }
}
