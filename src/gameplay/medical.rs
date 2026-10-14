//! Medical effects, from `Blueprints/Components/MedicalComponent` and `RadiationComponent`:
//!
//! - **Bleed**: 2-4 damage every 1-2 s (the interval is rolled once), stopping on its own after
//!   10-12 s. Bleeding again while bleeding turns it into a heavy bleed.
//! - **Heavy Bleed**: 4-6 damage every 1-1.5 s, for 14-16 s.
//! - **Broken Bone**: jump velocity 400 instead of 600 for 180-360 s.
//! - **Radiation sickness**: the game's radiation meter (100 = clean) at 0 deals 15 damage every
//!   5 s (`ApplyRadiationDamage`); anything that raises the meter clears it. Our
//!   `SurvivalStats::radiation` counts the other way (0 = clean), so sickness is at 100.
//! - Consumables: `Medical` heals (`IncreaseHealth`: × the passive FirstAidMultiplier, and 5-10
//!   First Aid XP), `Radiation` raises the
//!   meter (lowers ours), and `RemoveMedicalEffects` ends the named effects.

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::survival::SurvivalStats;

/// A damage-over-time effect: its tick interval, time to the next tick and time left.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dot {
    pub every: f32,
    pub next: f32,
    pub left: f32,
}

/// The player's active medical effects (saved with the game, like `ComponentPreSave`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MedicalEffects {
    pub bleed: Option<Dot>,
    pub heavy_bleed: Option<Dot>,
    /// Seconds of broken bone left.
    pub broken_bone: Option<f32>,
    pub radiation_sickness: bool,
    /// Time to the next radiation-sickness tick (the 5 s looping timer).
    pub sickness_tick: f32,
}

pub const JUMP_VELOCITY: f32 = 600.0;
pub const BROKEN_JUMP_VELOCITY: f32 = 400.0;
const SICKNESS_EVERY: f32 = 5.0;
const SICKNESS_DAMAGE: f32 = 15.0;

impl MedicalEffects {
    /// `Bleed? = true` (OnRep_Bleed?).
    pub fn add_bleed(&mut self, rng: &mut impl Rng) {
        if self.bleed.is_some() {
            self.bleed = None;
            self.add_heavy_bleed(rng);
            return;
        }
        let every = rng.gen_range(1.0..2.0);
        self.bleed = Some(Dot { every, next: every, left: rng.gen_range(10.0..12.0) });
    }

    /// `HeavyBleed? = true`: a running heavy bleed isn't restarted.
    pub fn add_heavy_bleed(&mut self, rng: &mut impl Rng) {
        if self.heavy_bleed.is_none() {
            let every = rng.gen_range(1.0..1.5);
            self.heavy_bleed = Some(Dot { every, next: every, left: rng.gen_range(14.0..16.0) });
        }
    }

    pub fn add_broken_bone(&mut self, rng: &mut impl Rng) {
        if self.broken_bone.is_none() {
            self.broken_bone = Some(rng.gen_range(180.0..360.0));
        }
    }

    /// `Client_RemoveEffect` with an item's `RemoveMedicalEffects`.
    pub fn remove(&mut self, names: &[String]) {
        for n in names {
            match n.as_str() {
                "Bleed" => self.bleed = None,
                "Heavy Bleed" => self.heavy_bleed = None,
                "Broken Bone" => self.broken_bone = None,
                _ => {}
            }
        }
    }

    pub fn jump_velocity(&self) -> f32 {
        if self.broken_bone.is_some() { BROKEN_JUMP_VELOCITY } else { JUMP_VELOCITY }
    }

    /// Advance the timers by `dt` and return the damage dealt (`Svr_Damage` ticks).
    pub fn tick(&mut self, dt: f32, radiation: f32, rng: &mut impl Rng) -> f32 {
        let mut damage = 0.0;
        for (dot, lo, hi) in [(&mut self.bleed, 2.0, 4.0), (&mut self.heavy_bleed, 4.0, 6.0)] {
            if let Some(d) = dot {
                d.next -= dt;
                while d.next <= 0.0 {
                    damage += rng.gen_range(lo..hi);
                    d.next += d.every;
                }
                d.left -= dt;
                if d.left <= 0.0 {
                    *dot = None;
                }
            }
        }
        if let Some(left) = self.broken_bone.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.broken_bone = None;
            }
        }
        // ApplyRadiationDamage: every 5 s, sick (and hurt) while the meter is empty.
        self.sickness_tick -= dt;
        if self.sickness_tick <= 0.0 {
            self.sickness_tick += SICKNESS_EVERY;
            if radiation >= 100.0 {
                self.radiation_sickness = true;
                damage += SICKNESS_DAMAGE;
            }
        }
        damage
    }

    pub fn bleeding(&self) -> bool {
        self.bleed.is_some() || self.heavy_bleed.is_some()
    }
}

/// Use a consumable: its stats (`Jig.ConsumableStat.*`) and the effects it removes.
pub fn consume(s: &mut SurvivalStats, effects: &mut MedicalEffects, passive: &mut crate::progression::passive::PassiveSkills, def: &super::items::ItemDef) {
    use crate::progression::passive::Passive;
    for (stat, [lo, hi]) in &def.stats {
        let v = (lo + hi) * 0.5;
        match stat.as_str() {
            "Hunger" => s.hunger = (s.hunger + v).clamp(0.0, 100.0),
            "Thirst" => s.thirst = (s.thirst + v).clamp(0.0, 100.0),
            "Medical" | "Health" | "Heal" if v != 0.0 => {
                s.heal(v * passive.multiplier(Passive::FirstAid));
                passive.add_xp(Passive::FirstAid, rand::thread_rng().gen_range(5.0..=10.0));
            }
            "Energy" | "Stamina" => s.stamina = (s.stamina + v).clamp(0.0, s.max_stamina),
            // IncreaseRadiation: the meter goes up, ours down; any rise ends the sickness.
            "Radiation" => {
                s.radiation = (s.radiation - v).clamp(0.0, 100.0);
                if v != 0.0 {
                    effects.radiation_sickness = false;
                }
            }
            _ => {}
        }
    }
    effects.remove(&def.removes);
    s.bleeding = effects.bleeding();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn rng() -> rand::rngs::StdRng {
        rand::rngs::StdRng::seed_from_u64(7)
    }

    fn run(e: &mut MedicalEffects, secs: f32, radiation: f32) -> f32 {
        let mut r = rng();
        (0..(secs * 10.0) as u32).map(|_| e.tick(0.1, radiation, &mut r)).sum()
    }

    #[test]
    fn bleeds_hurt_then_stop_on_their_own() {
        let mut e = MedicalEffects::default();
        e.add_bleed(&mut rng());
        let d = e.bleed.unwrap();
        assert!((1.0..2.0).contains(&d.every) && (10.0..12.0).contains(&d.left));
        // 10-12 s at 2-4 every 1-2 s: between 5 ticks of 2 and 12 of 4.
        let total = run(&mut e, 13.0, 0.0);
        assert!((10.0..=48.0).contains(&total), "{total}");
        assert!(e.bleed.is_none() && !e.bleeding());
    }

    #[test]
    fn a_second_bleed_becomes_heavy() {
        let mut e = MedicalEffects::default();
        e.add_bleed(&mut rng());
        e.add_bleed(&mut rng());
        assert!(e.bleed.is_none() && e.heavy_bleed.is_some());
        let left = e.heavy_bleed.unwrap().left;
        e.add_heavy_bleed(&mut rng());
        assert_eq!(e.heavy_bleed.unwrap().left, left, "a running heavy bleed isn't restarted");
        assert!((14.0..16.0).contains(&left));
    }

    #[test]
    fn broken_bones_shorten_jumps_until_they_heal() {
        let mut e = MedicalEffects::default();
        e.add_broken_bone(&mut rng());
        assert_eq!(e.jump_velocity(), 400.0);
        run(&mut e, 361.0, 0.0);
        assert_eq!(e.jump_velocity(), 600.0);
    }

    #[test]
    fn radiation_sickness_ticks_every_five_seconds_at_full_radiation() {
        let mut e = MedicalEffects::default();
        assert_eq!(run(&mut e, 20.0, 50.0), 0.0);
        assert!(!e.radiation_sickness);
        let dmg = run(&mut e, 10.0, 100.0);
        assert_eq!(dmg, 30.0);
        assert!(e.radiation_sickness);
    }

    #[test]
    fn items_heal_cure_and_clear_radiation() {
        let Ok(text) = std::fs::read_to_string("assets/data/items.json") else { return };
        let db: super::super::items::ItemDb = serde_json::from_str(&text).unwrap();
        let mut s = SurvivalStats::new();
        let mut e = MedicalEffects::default();
        let mut p = crate::progression::passive::PassiveSkills::default();
        s.health = 50.0;
        e.add_bleed(&mut rng());
        e.add_broken_bone(&mut rng());
        consume(&mut s, &mut e, &mut p, &db.items["DA_Bandages"]);
        assert_eq!(s.health, 65.0, "Medical 15");
        let xp = p.get(crate::progression::passive::Passive::FirstAid).xp;
        assert!((5.0..=10.0).contains(&xp), "healing gives 5-10 First Aid XP ({xp})");
        assert!(!e.bleeding() && e.broken_bone.is_some() && !s.bleeding);
        consume(&mut s, &mut e, &mut p, &db.items["DA_Splint"]);
        assert!(e.broken_bone.is_none());
        s.radiation = 100.0;
        e.radiation_sickness = true;
        consume(&mut s, &mut e, &mut p, &db.items["DA_RadiationPills"]);
        assert_eq!(s.radiation, 50.0);
        assert!(!e.radiation_sickness);
        // A trained First Aid heals more.
        p.add_xp(crate::progression::passive::Passive::FirstAid, 1000.0);
        s.health = 50.0;
        consume(&mut s, &mut e, &mut p, &db.items["DA_Bandages"]);
        assert!((s.health - (50.0 + 15.0 * 1.0325)).abs() < 1e-3, "{}", s.health);
    }
}
