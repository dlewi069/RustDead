use crate::math::{Position3D, Ray};
use super::weapon::WeaponStats;
use glam::Vec3;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitZone {
    Head,
    Torso,
    Stomach,
    Limbs,
}

impl HitZone {
    pub fn damage_multiplier(&self, headshot_mult: f32) -> f32 {
        match self {
            HitZone::Head => headshot_mult,
            HitZone::Torso => 1.0,
            HitZone::Stomach => 0.85,
            HitZone::Limbs => 0.60,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BallisticShot {
    pub ray: Ray,
    pub initial_damage: f32,
    pub max_range: f32,
    pub headshot_mult: f32,
    pub noise_radius: f32,
}

impl BallisticShot {
    pub fn from_weapon(origin: Position3D, forward: Vec3, weapon: &WeaponStats) -> Self {
        let mut rng = rand::thread_rng();
        // Apply weapon spread cone
        let spread_angle = weapon.spread_rad;
        let rx = rng.gen_range(-spread_angle..spread_angle);
        let ry = rng.gen_range(-spread_angle..spread_angle);

        let spread_dir = Vec3::new(
            forward.x + rx,
            forward.y + ry,
            forward.z,
        ).normalize_or_zero();

        Self {
            ray: Ray::new(origin, spread_dir),
            initial_damage: weapon.base_damage,
            max_range: weapon.effective_range_m,
            headshot_mult: weapon.headshot_multiplier,
            noise_radius: weapon.effective_noise_radius(),
        }
    }

    pub fn calculate_damage_at_distance(&self, distance: f32) -> f32 {
        if distance > self.max_range * 1.5 {
            return 0.0;
        }

        if distance <= self.max_range {
            self.initial_damage
        } else {
            // Gradual falloff past effective range
            let t = (distance - self.max_range) / (self.max_range * 0.5);
            self.initial_damage * (1.0 - (t * 0.5)).max(0.2)
        }
    }
}
