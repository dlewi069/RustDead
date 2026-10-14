//! The tech tree: SkillTree / TechTreeComponent (decompiled; the Tech Tree plugin's
//! TechnologyTree). `assets/data/techtree.json` (`gen_items.py`) holds the 11 skills: name,
//! tooltip, cost, icon, editor position and parents.
//!
//! - Every character level-up adds a skill point (LevellingComponent.LevelUp -> Add Skill Points;
//!   ours: `ProgressionSystem::unspent_points`).
//! - TryToUnlockTechnology: "Cannot unlock - Skill is already unlocked!", "Cannot unlock - Parent
//!   skill is locked!", "Cannot unlock - Not enough skill points!"; otherwise the cost is paid.

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct TechNode {
    pub id: String,
    pub name: String,
    pub tooltip: String,
    pub cost: u32,
    /// Enum_SkillType: the icon's tint (0 red, 1 green, 2 blue, 3 grey).
    #[serde(default)]
    pub kind: u8,
    pub icon: String,
    /// Position in the tree's editor canvas (TreeSize units).
    pub pos: [f32; 2],
    pub parents: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct TechDb {
    pub size: [f32; 2],
    pub slot: [f32; 2],
    pub nodes: Vec<TechNode>,
}

impl TechDb {
    pub fn load() -> Self {
        std::fs::read_to_string("assets/data/techtree.json").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn node(&self, id: &str) -> Option<&TechNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// The skills unlocked so far (saved with the game).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct TechTree {
    pub unlocked: Vec<String>,
    /// Skill_Deathbringer's DeathbringerSkillUsed? (once each life).
    #[serde(default)]
    pub deathbringer_used: bool,
}

pub const HEALTH_REGEN: &str = "Skill_HealthRegen";
pub const LOCKPICK: &str = "Skill_Lockpick";
pub const CRITICAL_HITTER: &str = "Skill_CriticalHitter";
pub const DEMOLITION_EXPERT: &str = "Skill_DemolitionExpert";
pub const NIGHT_CREATURE: &str = "Skill_NightCreature";
pub const DEATHBRINGER: &str = "Skill_Deathbringer";
pub const DISTRACTION: &str = "Skill_Distraction";
pub const COMBAT_ROLL: &str = "Skill_CombatRoll";

impl TechTree {
    pub fn has(&self, id: &str) -> bool {
        self.unlocked.iter().any(|u| u == id)
    }

    /// TryToUnlockTechnology, paying from `points`.
    pub fn try_unlock(&mut self, db: &TechDb, id: &str, points: &mut u32) -> Result<(), &'static str> {
        let Some(node) = db.node(id) else { return Err("Cannot unlock - Skill is already unlocked!") };
        if self.has(id) {
            return Err("Cannot unlock - Skill is already unlocked!");
        }
        if !node.parents.iter().all(|p| self.has(p)) {
            return Err("Cannot unlock - Parent skill is locked!");
        }
        if *points < node.cost {
            return Err("Cannot unlock - Not enough skill points!");
        }
        *points -= node.cost;
        self.unlocked.push(id.to_string());
        Ok(())
    }

    /// ResetTree: every unlocked skill's cost back as points.
    pub fn reset(&mut self, db: &TechDb, points: &mut u32) {
        for id in self.unlocked.drain(..) {
            *points += db.node(&id).map_or(0, |n| n.cost);
        }
    }

    /// Skill_Deathbringer at 0 health: once each life, health is set to 25. Whether it saved them.
    pub fn deathbringer(&mut self) -> bool {
        if self.deathbringer_used || !self.has(DEATHBRINGER) {
            return false;
        }
        self.deathbringer_used = true;
        true
    }
}

/// Deathbringer's health (MedicalComponent.Health = 25; the tooltip says 15).
pub const DEATHBRINGER_HEALTH: f32 = 25.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> TechDb {
        TechDb::load()
    }

    #[test]
    fn the_tree_is_the_games() {
        let db = db();
        if db.nodes.is_empty() {
            return;
        }
        assert_eq!(db.nodes.len(), 11);
        let regen = db.node(HEALTH_REGEN).unwrap();
        assert_eq!((regen.cost, regen.parents.len()), (1, 0), "the root");
        assert_eq!(db.node(LOCKPICK).unwrap().parents, vec!["Skill_Distraction".to_string()]);
        assert_eq!(db.node(DEATHBRINGER).unwrap().cost, 5);
        assert!(db.nodes.iter().all(|n| n.parents.iter().all(|p| db.node(p).is_some())));
    }

    #[test]
    fn unlocking_needs_parents_and_points() {
        let db = db();
        if db.nodes.is_empty() {
            return;
        }
        let mut t = TechTree::default();
        let mut points = 2;
        assert_eq!(t.try_unlock(&db, LOCKPICK, &mut points), Err("Cannot unlock - Parent skill is locked!"));
        assert_eq!(t.try_unlock(&db, HEALTH_REGEN, &mut points), Ok(()));
        assert_eq!(points, 1);
        assert_eq!(t.try_unlock(&db, HEALTH_REGEN, &mut points), Err("Cannot unlock - Skill is already unlocked!"));
        assert_eq!(t.try_unlock(&db, DISTRACTION, &mut points), Err("Cannot unlock - Not enough skill points!"));
        points += 1;
        assert_eq!(t.try_unlock(&db, DISTRACTION, &mut points), Ok(()));
        assert!(t.has(DISTRACTION) && points == 0);
        t.reset(&db, &mut points);
        assert!(t.unlocked.is_empty() && points == 3, "ResetTree refunds every cost");
    }

    #[test]
    fn deathbringer_saves_once_a_life() {
        let mut t = TechTree::default();
        assert!(!t.deathbringer(), "not unlocked");
        t.unlocked.push(DEATHBRINGER.into());
        assert!(t.deathbringer());
        assert!(!t.deathbringer(), "used up until the next life");
        t.deathbringer_used = false;
        assert!(t.deathbringer());
    }
}
