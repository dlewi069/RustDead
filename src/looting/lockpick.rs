use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockDifficulty {
    Novice,
    Adept,
    Expert,
    Master,
}

impl LockDifficulty {
    pub fn base_sweet_spot_degrees(&self) -> f32 {
        match self {
            LockDifficulty::Novice => 36.0,
            LockDifficulty::Adept => 22.0,
            LockDifficulty::Expert => 12.0,
            LockDifficulty::Master => 6.0,
        }
    }

    pub fn unlock_xp(&self) -> u64 {
        match self {
            LockDifficulty::Novice => 25,
            LockDifficulty::Adept => 50,
            LockDifficulty::Expert => 100,
            LockDifficulty::Master => 250,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockMechanism {
    pub difficulty: LockDifficulty,
    pub is_locked: bool,
    pub target_angle_deg: f32, // Secret angle (0.0 to 180.0)
    pub current_pick_angle: f32,
    pub current_cylinder_rotation: f32, // 0.0 to 90.0 (90 = unlocked)
    pub pick_durability: f32,          // 0.0 to 100.0 (breaks at 0)
}

#[derive(Debug, Clone, PartialEq)]
pub enum LockpickResult {
    SuccessUnlocked { xp: u64 },
    Turning { cylinder_angle: f32 },
    Jammed { pick_damaged: bool, durability_left: f32 },
    PickBroke,
}

impl LockMechanism {
    pub fn new(difficulty: LockDifficulty) -> Self {
        let target = rand::random::<f32>() * 180.0;
        Self {
            difficulty,
            is_locked: true,
            target_angle_deg: target,
            current_pick_angle: 90.0,
            current_cylinder_rotation: 0.0,
            pick_durability: 100.0,
        }
    }

    pub fn set_pick_angle(&mut self, angle: f32) {
        self.current_pick_angle = angle.clamp(0.0, 180.0);
    }

    /// Try to turn the cylinder with tension.
    /// `skill_tolerance_mult` from player's Lockpicking skill
    pub fn turn_cylinder(&mut self, tension: f32, skill_tolerance_mult: f32) -> LockpickResult {
        if !self.is_locked {
            return LockpickResult::SuccessUnlocked { xp: 0 };
        }

        let half_sweet_spot = (self.difficulty.base_sweet_spot_degrees() * skill_tolerance_mult) / 2.0;
        let angle_diff = (self.current_pick_angle - self.target_angle_deg).abs();

        if angle_diff <= half_sweet_spot {
            // Inside sweet spot: Cylinder turns smoothly towards 90 deg
            self.current_cylinder_rotation = (self.current_cylinder_rotation + tension * 45.0).min(90.0);

            if self.current_cylinder_rotation >= 90.0 {
                self.is_locked = false;
                LockpickResult::SuccessUnlocked {
                    xp: self.difficulty.unlock_xp(),
                }
            } else {
                LockpickResult::Turning {
                    cylinder_angle: self.current_cylinder_rotation,
                }
            }
        } else {
            // Off sweet spot: Cylinder gets stuck, tension bends and damages the pick
            let stress_factor = (angle_diff / 180.0).clamp(0.2, 1.0);
            let damage = tension * 35.0 * stress_factor / skill_tolerance_mult;
            self.pick_durability -= damage;

            // Reset cylinder rotation back to 0
            self.current_cylinder_rotation = (self.current_cylinder_rotation - 20.0).max(0.0);

            if self.pick_durability <= 0.0 {
                self.pick_durability = 0.0;
                LockpickResult::PickBroke
            } else {
                LockpickResult::Jammed {
                    pick_damaged: true,
                    durability_left: self.pick_durability,
                }
            }
        }
    }

    /// Insert a new pick reset durability
    pub fn replace_pick(&mut self) {
        self.pick_durability = 100.0;
        self.current_cylinder_rotation = 0.0;
    }
}
