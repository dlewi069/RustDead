use super::stats::SurvivalStats;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionType {
    Healthy,
    Starving,
    Dehydrated,
    Bleeding,
    Infected,
    RadiationSickness,
    Exhausted,
    WellNourished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveCondition {
    pub condition_type: ConditionType,
    pub description: String,
    pub severity: u8, // 1 to 5
}

impl SurvivalStats {
    pub fn get_active_conditions(&self) -> Vec<ActiveCondition> {
        let mut conditions = Vec::new();

        if self.bleeding {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::Bleeding,
                description: "Severe hemorrhage. Losing health rapidly. Apply a bandage!".into(),
                severity: 4,
            });
        }

        if self.hunger <= 0.0 {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::Starving,
                description: "Starving to death. Vital organs shutting down.".into(),
                severity: 5,
            });
        } else if self.hunger < 25.0 {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::Starving,
                description: "Hungry. Stamina recovery significantly reduced.".into(),
                severity: 2,
            });
        }

        if self.thirst <= 0.0 {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::Dehydrated,
                description: "Critically dehydrated. Taking continuous damage.".into(),
                severity: 5,
            });
        } else if self.thirst < 25.0 {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::Dehydrated,
                description: "Thirsty. Speed and endurance impaired.".into(),
                severity: 2,
            });
        }

        if self.infection >= 80.0 {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::Infected,
                description: "Advanced stage infection. Sepsis imminent. Use antibiotics!".into(),
                severity: 5,
            });
        } else if self.infection > 0.0 {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::Infected,
                description: format!("Infected ({:.0}%). Virus multiplying.", self.infection),
                severity: (self.infection / 20.0).ceil() as u8,
            });
        }

        if self.radiation > 50.0 {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::RadiationSickness,
                description: "Acute radiation poisoning. Cellular degradation.".into(),
                severity: 4,
            });
        }

        if self.stamina <= 5.0 {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::Exhausted,
                description: "Exhausted. Cannot sprint or jump.".into(),
                severity: 2,
            });
        }

        if self.hunger > 80.0 && self.thirst > 80.0 && !self.bleeding && self.infection == 0.0 {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::WellNourished,
                description: "Well nourished. Passive health regeneration active.".into(),
                severity: 1,
            });
        }

        if conditions.is_empty() {
            conditions.push(ActiveCondition {
                condition_type: ConditionType::Healthy,
                description: "Stable condition. All vitals nominal.".into(),
                severity: 1,
            });
        }

        conditions
    }
}
