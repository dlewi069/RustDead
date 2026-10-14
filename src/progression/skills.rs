use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillType {
    // Combat
    Marksmanship,
    MeleeMastery,
    FastHands,

    // Survival
    IronStomach,
    Metabolism,
    Immunity,

    // Athletics
    Cardio,
    Sprinter,
    PackMule,

    // Scavenging & Engineering
    LuckyLooter,
    Lockpicking,
    Scrapper,
}

impl SkillType {
    pub const ALL: [SkillType; 12] = [
        SkillType::Marksmanship,
        SkillType::MeleeMastery,
        SkillType::FastHands,
        SkillType::IronStomach,
        SkillType::Metabolism,
        SkillType::Immunity,
        SkillType::Cardio,
        SkillType::Sprinter,
        SkillType::PackMule,
        SkillType::LuckyLooter,
        SkillType::Lockpicking,
        SkillType::Scrapper,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            SkillType::Marksmanship => "Marksmanship",
            SkillType::MeleeMastery => "Melee Mastery",
            SkillType::FastHands => "Fast Hands (Reload Speed)",
            SkillType::IronStomach => "Iron Stomach",
            SkillType::Metabolism => "Metabolism (Hydration)",
            SkillType::Immunity => "Immunity & Resistance",
            SkillType::Cardio => "Cardio (Stamina)",
            SkillType::Sprinter => "Sprinter (Athletics)",
            SkillType::PackMule => "Pack Mule (Carry Capacity)",
            SkillType::LuckyLooter => "Lucky Looter",
            SkillType::Lockpicking => "Lockpicking Mastery",
            SkillType::Scrapper => "Scrapper (Salvaging)",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            SkillType::Marksmanship | SkillType::MeleeMastery | SkillType::FastHands => "Combat",
            SkillType::IronStomach | SkillType::Metabolism | SkillType::Immunity => "Survival",
            SkillType::Cardio | SkillType::Sprinter | SkillType::PackMule => "Athletics",
            SkillType::LuckyLooter | SkillType::Lockpicking | SkillType::Scrapper => "Scavenging & Engineering",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SkillType::Marksmanship => "-6% Weapon Recoil and -5% Bullet Spread per level",
            SkillType::MeleeMastery => "+10% Melee Strike Damage per level",
            SkillType::FastHands => "+8% Weapon Reload Speed per level",
            SkillType::IronStomach => "-7% Hunger Drain rate and resistance to food poisoning",
            SkillType::Metabolism => "-7% Thirst Drain rate per level",
            SkillType::Immunity => "-10% Infection buildup and +8% Radiation resistance",
            SkillType::Cardio => "+15 Max Stamina and +10% Stamina recovery speed",
            SkillType::Sprinter => "+5% Sprint movement speed per level",
            SkillType::PackMule => "+8.0 kg carry weight limit per level",
            SkillType::LuckyLooter => "+10% chance to roll rare/military loot from containers",
            SkillType::Lockpicking => "+15% lockpick sweet-spot tolerance and slower pick breaking",
            SkillType::Scrapper => "+25% bonus materials yield when disassembling items",
        }
    }

    pub fn max_level(&self) -> u32 {
        5
    }
}

/// LevellingComponent's defaults.
pub const FIRST_MAX_XP: u64 = 500;
pub const LEVEL_CAP: u32 = 50;

fn first_max_xp() -> u64 {
    FIRST_MAX_XP
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressionSystem {
    pub level: u32,
    pub current_xp: u64,
    /// LevellingComponent's CurrentMaxXP: 500 at level 1, x 1.325 (rounded up) per level.
    #[serde(default = "first_max_xp")]
    pub max_xp: u64,
    /// Level-ups not yet announced ("Level up! You're now level {lvl}!", "+1 skill point").
    #[serde(skip)]
    pub level_ups: Vec<u32>,
    pub unspent_points: u32,
    pub total_earned_xp: u64,
    pub skills: Vec<(SkillType, u32)>, // skill and current level (0..=5)
}

impl Default for ProgressionSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressionSystem {
    pub fn new() -> Self {
        let skills = SkillType::ALL.iter().map(|&s| (s, 0)).collect();
        Self {
            level: 1,
            current_xp: 0,
            max_xp: FIRST_MAX_XP,
            level_ups: Vec::new(),
            unspent_points: 0,
            total_earned_xp: 0,
            skills,
        }
    }

    /// XP required to advance from current level to level + 1 (CurrentMaxXP).
    pub fn xp_for_next_level(&self) -> u64 {
        self.max_xp.max(1)
    }

    /// LevellingComponent.AddXP + LevelUp: the remainder carries over; each level gives a skill
    /// point (TechTreeComponent's Add Skill Points); at LevelCap 50 the bar stays full.
    /// Returns Some(new_level) if it levelled up.
    pub fn add_xp(&mut self, amount: u64) -> Option<u32> {
        self.current_xp += amount;
        self.total_earned_xp += amount;

        let mut leveled_up = false;
        while self.current_xp >= self.xp_for_next_level() {
            if self.level >= LEVEL_CAP {
                self.current_xp = self.max_xp;
                break;
            }
            let needed = self.xp_for_next_level();
            self.current_xp -= needed;
            self.level += 1;
            self.unspent_points += 1;
            // CalculateMaxXP: ceil(x 1.325), at most 200000.
            self.max_xp = ((self.max_xp as f64 * 1.325).ceil() as u64).min(200_000);
            self.level_ups.push(self.level);
            leveled_up = true;
        }

        if leveled_up {
            Some(self.level)
        } else {
            None
        }
    }

    /// LevellingComponent.XPDeath (Pause > Suicide): a level lower, the bar empty, and
    /// CurrentMaxXP = round(CurrentMaxXP / 2.65 x 2).
    pub fn xp_death(&mut self) {
        if self.level > 1 {
            self.level -= 1;
            self.current_xp = 0;
            self.max_xp = (self.max_xp as f64 / 2.65 * 2.0).round() as u64;
        }
    }

    pub fn get_skill_level(&self, skill: SkillType) -> u32 {
        self.skills
            .iter()
            .find(|(s, _)| *s == skill)
            .map(|(_, lvl)| *lvl)
            .unwrap_or(0)
    }

    /// Allocates an unspent skill point into the requested skill
    pub fn upgrade_skill(&mut self, skill: SkillType) -> Result<u32, &'static str> {
        if self.unspent_points == 0 {
            return Err("No skill points available!");
        }

        for (s, lvl) in &mut self.skills {
            if *s == skill {
                if *lvl >= skill.max_level() {
                    return Err("Skill already maxed out!");
                }
                *lvl += 1;
                self.unspent_points -= 1;
                return Ok(*lvl);
            }
        }

        Err("Skill not found")
    }

    // --- Calculated stat bonuses ---

    /// Multiplier for weapon recoil (1.0 down to 0.70)
    pub fn recoil_multiplier(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::Marksmanship);
        (1.0 - (lvl as f32 * 0.06)).max(0.60)
    }

    /// Multiplier for melee damage (1.0 up to 1.50)
    pub fn melee_damage_multiplier(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::MeleeMastery);
        1.0 + (lvl as f32 * 0.10)
    }

    /// Multiplier for reload speed (1.0 down to 0.60 duration)
    pub fn reload_duration_multiplier(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::FastHands);
        (1.0 - (lvl as f32 * 0.08)).max(0.50)
    }

    /// Multiplier for hunger decay (1.0 down to 0.65)
    pub fn hunger_decay_multiplier(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::IronStomach);
        (1.0 - (lvl as f32 * 0.07)).max(0.50)
    }

    /// Multiplier for thirst decay (1.0 down to 0.65)
    pub fn thirst_decay_multiplier(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::Metabolism);
        (1.0 - (lvl as f32 * 0.07)).max(0.50)
    }

    /// Resistance to infection buildup (0.0 up to 0.50)
    pub fn infection_resistance(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::Immunity);
        (lvl as f32 * 0.10).min(0.50)
    }

    /// Resistance to radiation buildup (0.0 up to 0.40)
    pub fn radiation_resistance(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::Immunity);
        (lvl as f32 * 0.08).min(0.40)
    }

    /// Bonus maximum stamina (+0 to +75)
    pub fn bonus_max_stamina(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::Cardio);
        lvl as f32 * 15.0
    }

    /// Bonus stamina recovery rate multiplier (1.0 to 1.50)
    pub fn stamina_recovery_multiplier(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::Cardio);
        1.0 + (lvl as f32 * 0.10)
    }

    /// Sprint speed multiplier (1.0 to 1.25)
    pub fn sprint_speed_multiplier(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::Sprinter);
        1.0 + (lvl as f32 * 0.05)
    }

    /// Bonus carry weight capacity in kilograms (+0 to +40 kg)
    pub fn bonus_carry_capacity_kg(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::PackMule);
        lvl as f32 * 8.0
    }

    /// Bonus lucky looter chance (0.0 to 0.50)
    pub fn lucky_loot_bonus(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::LuckyLooter);
        lvl as f32 * 0.10
    }

    /// Lockpick sweet spot angular tolerance multiplier (1.0 to 1.75)
    pub fn lockpick_tolerance_multiplier(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::Lockpicking);
        1.0 + (lvl as f32 * 0.15)
    }

    /// Scrapper material bonus multiplier (1.0 to 2.25)
    pub fn scrap_yield_multiplier(&self) -> f32 {
        let lvl = self.get_skill_level(SkillType::Scrapper);
        1.0 + (lvl as f32 * 0.25)
    }
}

#[cfg(test)]
mod levelling_tests {
    use super::*;

    #[test]
    fn levels_follow_the_levelling_component() {
        let mut p = ProgressionSystem::new();
        assert_eq!(p.xp_for_next_level(), 500);
        assert_eq!(p.add_xp(499), None);
        assert_eq!(p.add_xp(10), Some(2));
        assert_eq!((p.level, p.current_xp, p.max_xp, p.unspent_points), (2, 9, 663, 1), "ceil(500 x 1.325)");
        // Two levels at once carry the remainder.
        p.add_xp(663 + 879);
        assert_eq!((p.level, p.current_xp, p.max_xp), (4, 9, 1165));
        assert_eq!(p.level_ups, vec![2, 3, 4]);
        p.xp_death();
        assert_eq!((p.level, p.current_xp, p.max_xp), (3, 0, 879), "round(1165 / 2.65 x 2)");
    }

    #[test]
    fn level_50_is_the_cap() {
        let mut p = ProgressionSystem::new();
        for _ in 0..80 {
            p.add_xp(p.max_xp);
        }
        assert_eq!(p.level, LEVEL_CAP);
        assert_eq!(p.current_xp, p.max_xp);
        assert!(p.max_xp <= 200_000);
        assert_eq!(p.unspent_points, 49);
    }
}
