use super::stats::SurvivalStats;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraumaType {
    Fracture,         // Leg / Arm fracture slowing movement
    HeavyBleed,       // Arterial bleeding draining HP quickly
    FoodPoisoning,    // Nausea draining thirst and stamina
    RadiationPoison,  // Decreases max health
    PainShock,        // Shaky aim / slow stamina
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicalTrauma {
    pub trauma: TraumaType,
    pub duration_remaining_s: f32,
    pub severity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicalSystem {
    pub active_traumas: Vec<MedicalTrauma>,
    pub pain_level: f32, // 0.0 to 100.0
}

impl Default for MedicalSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl MedicalSystem {
    pub fn new() -> Self {
        Self {
            active_traumas: Vec::new(),
            pain_level: 0.0,
        }
    }

    pub fn add_trauma(&mut self, trauma: TraumaType, severity: f32, duration: f32) {
        if !self.has_trauma(trauma) {
            self.active_traumas.push(MedicalTrauma {
                trauma,
                duration_remaining_s: duration,
                severity,
            });
        }
    }

    pub fn has_trauma(&self, trauma: TraumaType) -> bool {
        self.active_traumas.iter().any(|t| t.trauma == trauma)
    }

    pub fn remove_trauma(&mut self, trauma: TraumaType) -> bool {
        let initial_len = self.active_traumas.len();
        self.active_traumas.retain(|t| t.trauma != trauma);
        self.active_traumas.len() < initial_len
    }

    pub fn tick(&mut self, stats: &mut SurvivalStats, dt: f32) {
        let mut to_remove = Vec::new();

        for (idx, trauma) in self.active_traumas.iter_mut().enumerate() {
            trauma.duration_remaining_s -= dt;
            if trauma.duration_remaining_s <= 0.0 {
                to_remove.push(idx);
                continue;
            }

            match trauma.trauma {
                TraumaType::Fracture => {
                    // Fractured limb restricts sprinting
                    stats.stamina = (stats.stamina - dt * 2.0).max(0.0);
                }
                TraumaType::HeavyBleed => {
                    // Heavy bleed drains 3.5 HP per second
                    stats.take_damage(3.5 * dt, false, 0.0);
                }
                TraumaType::FoodPoisoning => {
                    // Poisoning drains water and hunger
                    stats.thirst = (stats.thirst - dt * 1.5).max(0.0);
                    stats.hunger = (stats.hunger - dt * 0.8).max(0.0);
                }
                TraumaType::RadiationPoison => {
                    stats.radiation = (stats.radiation + dt * 0.5).min(100.0);
                }
                TraumaType::PainShock => {
                    self.pain_level = (self.pain_level + dt * 2.0).min(100.0);
                }
            }
        }

        // Remove expired traumas in reverse order
        for idx in to_remove.into_iter().rev() {
            self.active_traumas.remove(idx);
        }

        // Pain naturally subsides slowly
        self.pain_level = (self.pain_level - dt * 0.5).max(0.0);
    }

    /// Speed multiplier affected by fractures (e.g. 0.55 if fractured leg)
    pub fn movement_speed_multiplier(&self) -> f32 {
        if self.has_trauma(TraumaType::Fracture) {
            0.55 // Limping
        } else {
            1.0
        }
    }

    /// Apply specific medical treatment item
    pub fn apply_medical_item(&mut self, stats: &mut SurvivalStats, item_id: &str) -> Option<&'static str> {
        match item_id {
            "splint" => {
                if self.remove_trauma(TraumaType::Fracture) {
                    Some("Bone set and splinted. Leg mobility restored.")
                } else {
                    None
                }
            }
            "tourniquet" => {
                let stopped_heavy = self.remove_trauma(TraumaType::HeavyBleed);
                let stopped_light = stats.bleeding;
                stats.bleeding = false;
                if stopped_heavy || stopped_light {
                    Some("Tourniquet applied tightly. Arterial bleeding arrested.")
                } else {
                    None
                }
            }
            "bandage" => {
                let stopped_light = stats.bleeding;
                let stopped_heavy = self.remove_trauma(TraumaType::HeavyBleed);
                stats.bleeding = false;
                stats.heal(20.0);
                if stopped_light || stopped_heavy {
                    Some("Bandage applied. Bleeding arrested and wound dressed (+20 HP).")
                } else {
                    Some("Bandage applied. Minor lacerations dressed (+20 HP).")
                }
            }
            "medkit" | "large_medkit" => {
                let _ = self.remove_trauma(TraumaType::HeavyBleed);
                stats.bleeding = false;
                stats.heal(65.0);
                Some("Trauma kit deployed. Major injuries stabilized and sterile field applied (+65 HP).")
            }
            "antibiotics" => {
                if stats.infection > 0.0 {
                    stats.infection = (stats.infection - 50.0).max(0.0);
                    Some("Antibiotics taken. Bacterial infection reduced by 50%.")
                } else {
                    None
                }
            }
            "antiviral_injector" => {
                stats.infection = 0.0;
                Some("Antiviral injected. Virus eliminated completely from bloodstream.")
            }
            "radaway" | "radiation_pills" | "anti_rad_pills" => {
                if stats.radiation > 0.0 {
                    stats.radiation = (stats.radiation - 60.0).max(0.0);
                    Some("Potassium iodide/RadAway administered. Flushed 60 mSv of radiation toxins.")
                } else {
                    None
                }
            }
            "painkillers" | "morphine" => {
                self.pain_level = 0.0;
                self.remove_trauma(TraumaType::PainShock);
                Some("Painkillers taken. Pain numbed, aim and stamina stabilized.")
            }
            "charcoal_tablets" => {
                if self.remove_trauma(TraumaType::FoodPoisoning) {
                    Some("Activated charcoal consumed. Stomach toxins neutralized.")
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
