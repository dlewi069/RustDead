//! The passive skills: PassiveSkillsComponent (decompiled).
//!
//! - Ten skills, each level 1-25 with XP towards `MaxXP` (1000 at level 1). `AddXP_<Skill>` adds
//!   the amount (×2, ×3 for Fitness, while that skill's XP book is in effect: `<Skill>MultiplierUsed?`)
//!   and clamps it to `MaxXP`; reaching it runs `<Skill>_LevelUp`: the level goes up, XP restarts
//!   at 0, `MaxXP` becomes ceil(MaxXP × 1.1), the book's effect ends, the skill's multipliers grow
//!   and "Your '{Skill}' skill is now level {Lvl}!" shows for 6 s. At level 25, XP stays full.
//! - What each multiplier does lives in the systems that read it: Fitness (StaminaComponent:
//!   sprint drain ÷ FitnessStaminaMultiplier, recovery × FitnessMultiplier, recovery delay
//!   1 / FitnessStaminaRegainMultiplier s, sprint speed × FitnessMultiplier), Toughness (AnyDamage
//!   ÷ ToughnessMultiplier), Sneaking (footstep noise range ÷ SneakingMultiplier).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Passive {
    Fitness,
    Strength,
    Toughness,
    Sneaking,
    FirstAid,
    Marksmanship,
    Reloading,
    Thief,
    Fishing,
    Scavenging,
}

impl Passive {
    pub const ALL: [Passive; 10] = [
        Passive::Fitness,
        Passive::Strength,
        Passive::Toughness,
        Passive::Sneaking,
        Passive::FirstAid,
        Passive::Marksmanship,
        Passive::Reloading,
        Passive::Thief,
        Passive::Fishing,
        Passive::Scavenging,
    ];

    /// The name LevelUpNotification uses.
    pub fn name(self) -> &'static str {
        match self {
            Passive::Fitness => "Fitness",
            Passive::Strength => "Strength",
            Passive::Toughness => "Toughness",
            Passive::Sneaking => "Sneaking",
            Passive::FirstAid => "First Aid",
            Passive::Marksmanship => "Marksmanship",
            Passive::Reloading => "Reloading",
            Passive::Thief => "Thief",
            Passive::Fishing => "Fishing",
            Passive::Scavenging => "Scavenging",
        }
    }

    /// The XP factor while the skill's XP book is in effect.
    fn book_factor(self) -> f32 {
        if self == Passive::Fitness { 3.0 } else { 2.0 }
    }

    /// The main multiplier's growth per level (`<Skill>_LevelUp`).
    fn growth(self) -> f32 {
        match self {
            Passive::Fitness => 1.01,
            Passive::Strength => 1.025,
            Passive::Toughness => 1.0525,
            Passive::Sneaking => 1.035,
            Passive::FirstAid => 1.0325,
            Passive::Marksmanship => 1.04,
            Passive::Reloading => 1.0325,
            Passive::Thief => 1.0625,
            Passive::Fishing => 1.05,
            Passive::Scavenging => 1.025,
        }
    }
}

pub const MAX_LEVEL: u32 = 25;
const FIRST_MAX_XP: f32 = 1000.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillProgress {
    pub level: u32,
    pub xp: f32,
    pub max_xp: f32,
    /// `<Skill>Multiplier`.
    pub multiplier: f32,
    /// `<Skill>MultiplierUsed?`: an XP book is in effect until the next level.
    pub book: bool,
}

impl Default for SkillProgress {
    fn default() -> Self {
        Self { level: 1, xp: 0.0, max_xp: FIRST_MAX_XP, multiplier: 1.0, book: false }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PassiveSkills {
    pub skills: Vec<(Passive, SkillProgress)>,
    /// Fitness_LevelUp's extra multipliers (× 1.05, × 1.15, × 1.0425 per level).
    pub fitness_stamina: f32,
    pub fitness_stamina_regain: f32,
    pub fitness_swimming: f32,
    /// Strength_LevelUp's StrengthSpeedMultiplier (× 1.0125): melee swing speed.
    pub strength_speed: f32,
    /// Level-ups not yet announced (LevelUpNotification), drained by the game.
    #[serde(skip)]
    pub level_ups: Vec<(Passive, u32)>,
}

impl Default for PassiveSkills {
    fn default() -> Self {
        Self {
            skills: Passive::ALL.iter().map(|&p| (p, SkillProgress::default())).collect(),
            fitness_stamina: 1.0,
            fitness_stamina_regain: 1.0,
            fitness_swimming: 1.0,
            strength_speed: 1.0,
            level_ups: Vec::new(),
        }
    }
}

impl PassiveSkills {
    pub fn get(&self, skill: Passive) -> SkillProgress {
        self.skills.iter().find(|(p, _)| *p == skill).map(|(_, s)| s.clone()).unwrap_or_default()
    }

    fn get_mut(&mut self, skill: Passive) -> &mut SkillProgress {
        if let Some(i) = self.skills.iter().position(|(p, _)| *p == skill) {
            return &mut self.skills[i].1;
        }
        self.skills.push((skill, SkillProgress::default()));
        &mut self.skills.last_mut().expect("just pushed").1
    }

    pub fn multiplier(&self, skill: Passive) -> f32 {
        self.get(skill).multiplier
    }

    /// `AddXP_<Skill>`: the new level if it went up.
    pub fn add_xp(&mut self, skill: Passive, amount: f32) -> Option<u32> {
        let s = self.get_mut(skill);
        let amount = if s.book { amount * skill.book_factor() } else { amount };
        s.xp = (s.xp + amount).clamp(0.0, s.max_xp);
        if s.xp < s.max_xp {
            return None;
        }
        s.book = false;
        if s.level >= MAX_LEVEL {
            s.xp = s.max_xp;
            return None;
        }
        s.level += 1;
        // RemainingXP = XP - MaxXP, which the clamp above leaves at 0.
        s.xp = 0.0;
        s.max_xp = (s.max_xp * 1.1).ceil();
        s.multiplier *= skill.growth();
        let level = s.level;
        match skill {
            Passive::Fitness => {
                self.fitness_stamina *= 1.05;
                self.fitness_stamina_regain *= 1.15;
                self.fitness_swimming *= 1.0425;
            }
            Passive::Strength => self.strength_speed *= 1.0125,
            _ => {}
        }
        self.level_ups.push((skill, level));
        Some(level)
    }

    /// `XPMultiplier_<Skill>` (reading its XP book): false if one is already in effect.
    pub fn use_book(&mut self, skill: Passive) -> bool {
        let s = self.get_mut(skill);
        !std::mem::replace(&mut s.book, true)
    }

    /// LevelUpNotification's text.
    pub fn level_up_message(skill: Passive, level: u32) -> String {
        format!("Your '{}' skill is now level {level}!", skill.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_take_ten_percent_more_xp_each() {
        let mut p = PassiveSkills::default();
        assert_eq!(p.add_xp(Passive::Toughness, 999.0), None);
        assert_eq!(p.add_xp(Passive::Toughness, 5.0), Some(2), "XP is clamped to MaxXP, which levels up");
        let t = p.get(Passive::Toughness);
        assert_eq!((t.xp, t.max_xp), (0.0, 1100.0), "the extra 4 XP is lost to the clamp");
        assert!((t.multiplier - 1.0525).abs() < 1e-6);
        p.add_xp(Passive::Toughness, 1100.0);
        assert_eq!(p.get(Passive::Toughness).max_xp, 1210.0);
        p.add_xp(Passive::Toughness, 1210.0);
        assert_eq!(p.get(Passive::Toughness).max_xp, 1331.0);
        p.add_xp(Passive::Toughness, 1331.0);
        assert_eq!(p.get(Passive::Toughness).max_xp, 1465.0, "ceil(1464.1)");
    }

    #[test]
    fn fitness_grows_its_stamina_multipliers() {
        let mut p = PassiveSkills::default();
        p.add_xp(Passive::Fitness, 1000.0);
        assert!((p.multiplier(Passive::Fitness) - 1.01).abs() < 1e-6);
        assert!((p.fitness_stamina - 1.05).abs() < 1e-6 && (p.fitness_stamina_regain - 1.15).abs() < 1e-6);
    }

    #[test]
    fn books_multiply_xp_until_the_next_level() {
        let mut p = PassiveSkills::default();
        assert!(p.use_book(Passive::Sneaking));
        assert!(!p.use_book(Passive::Sneaking), "one at a time");
        p.add_xp(Passive::Sneaking, 10.0);
        assert_eq!(p.get(Passive::Sneaking).xp, 20.0);
        p.add_xp(Passive::Sneaking, 500.0);
        assert_eq!(p.get(Passive::Sneaking).level, 2);
        p.add_xp(Passive::Sneaking, 10.0);
        assert_eq!(p.get(Passive::Sneaking).xp, 10.0, "the book ended with the level");
        assert!(p.use_book(Passive::Fitness));
        p.add_xp(Passive::Fitness, 10.0);
        assert_eq!(p.get(Passive::Fitness).xp, 30.0, "Fitness books triple");
    }

    #[test]
    fn level_25_is_the_cap() {
        let mut p = PassiveSkills::default();
        for _ in 0..40 {
            let max = p.get(Passive::Thief).max_xp;
            p.add_xp(Passive::Thief, max);
        }
        let t = p.get(Passive::Thief);
        assert_eq!(t.level, MAX_LEVEL);
        assert_eq!(t.xp, t.max_xp);
        assert_eq!(PassiveSkills::level_up_message(Passive::FirstAid, 3), "Your 'First Aid' skill is now level 3!");
    }
}
