//! Zombie bosses: `BP_ZombieBoss` and its children (Undead Boss, Large Undead Boss), placed by
//! the level's `BP_AISpawner_ZombieBosses` (one each, ZOMBIE BOSS SPAWN AMOUNT MULTIPLIER).
//! Their own Blueprint, not BP_MasterZombie's; `zombies.rs` runs `BT_ZombieBoss`, which is the
//! zombie tree with the boss's speeds (80 cm/s roaming, 500 investigating or chasing) and waits.
//!
//! - **Swings** (`ZombieBoss_Attack` -> MeleeAttack, every 0.1 s while `InRangeForMelee?`, i.e.
//!   within `MeleeAttackDistance` 5 m, actor centres): unless `Start_Attacking`, a growl
//!   (ZombieGrowlShort), the AttackMontage on the UpperBody slot (RateScale 0.5), then
//!   Delay(1.35-1.75) before the next. `ZombieBoss_Chase` keeps closing in meanwhile.
//! - **The hit**: in the montage's AnimNotify_ZombieBossAttackRightArm window, RightArmTrace: a
//!   60 cm sphere at the Hand_R socket. The first hit stops the trace; Event_OnHit does
//!   DamageToDo × ZOMBIE DAMAGE MULTIPLIER, Bleed 20 % / Heavy Bleed 10 %, and 16 radiation.
//! - Death (`combat::kill_zombies`): BossZombieKills + 1, level × 200-400 × 1.05 XP, a 25 %
//!   skill point, "Zombie Boss Killed, XP Given: N"; the corpse lies 1200 s and its loot adds
//!   the class's DT_UniqueLoot sets (`interact::track_corpses`). No hit reactions, shoves or
//!   stomps (no HitAnimation, `CanStompAI?` false).
//!
//! Not ported: `RangedAttack` (BP_ZombieBossRock thrown from ThrowLoc; nothing in the tree calls
//! it), the map marker (`AddMarker`), the glowing eyes, ACH_ToughGuy / ACH_TheReckoning, the
//! ragdoll.

use bevy::prelude::*;
use rand::Rng;

use super::combat::Dead;
use super::survival::difficulty;
use super::zombies::{Brain, Zombie};
use crate::level::{KinematicBody, LevelZombie};
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{ActionKind, CharacterModel, UpperAction};

/// The survivor's capsule centre above the feet (half of 1.64 m).
const PLAYER_CENTRE: f32 = 0.82;

/// MeleeAttack's state: `Start_Attacking` and the swing under way.
#[derive(Component, Default, Debug)]
pub struct BossSwing {
    /// Seconds since the swing started, while `Start_Attacking`.
    t: Option<f32>,
    /// The Delay after the montage starts (1.35-1.75 s).
    delay: f32,
    /// RightArmTrace hit this swing (K2_ClearTimer).
    hit: bool,
    /// The Hand_R bone entity, once the model is in.
    hand: Option<Entity>,
}

impl BossSwing {
    /// Advance by `dt`; true while the montage's arm-trace window is open (montage time is real
    /// time × `rate`). Returns (swing over, in window).
    fn tick(&mut self, dt: f32, rate: f32, [w0, w1]: [f32; 2]) -> (bool, bool) {
        let Some(t) = self.t.as_mut() else { return (true, false) };
        *t += dt;
        let m = *t * rate;
        let window = !self.hit && m >= w0 && m <= w1;
        if *t >= self.delay {
            self.t = None;
            return (true, window);
        }
        (false, window)
    }

    fn start(&mut self, rng: &mut impl Rng) {
        self.t = Some(0.0);
        self.delay = rng.gen_range(1.35..1.75);
        self.hit = false;
    }
}

/// Is the survivor's capsule (feet at `feet`, `radius`, `height`) within `reach` of `hand`?
pub fn hand_touches(hand: Vec3, reach: f32, feet: Vec3, radius: f32, height: f32) -> bool {
    let lo = feet.y + radius;
    let hi = (feet.y + height - radius).max(lo);
    let axis = Vec3::new(feet.x, hand.y.clamp(lo, hi), feet.z);
    hand.distance(axis) <= reach + radius
}

/// InRangeForMelee? (ZombieBoss_EnemyInRange): actor centres within MeleeAttackDistance.
pub fn in_melee_range(boss_centre: Vec3, player_feet: Vec3, reach: f32) -> bool {
    boss_centre.distance(player_feet + Vec3::Y * PLAYER_CENTRE) <= reach
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (find_hands, swing).chain().run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

/// Bosses get their swing state, and the Hand_R bone once the model has loaded.
#[allow(clippy::type_complexity)]
fn find_hands(
    mut commands: Commands,
    added: Query<(Entity, &LevelZombie), (Added<LevelZombie>, Without<BossSwing>)>,
    mut swings: Query<(Entity, &mut BossSwing)>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (e, z) in &added {
        if z.kind.boss().is_some() {
            commands.entity(e).insert(BossSwing::default());
        }
    }
    for (e, mut s) in &mut swings {
        if s.hand.is_some() {
            continue;
        }
        // Every body part has its own copy of the skeleton, animated alike; any copy will do.
        s.hand = children.iter_descendants(e).find(|&c| names.get(c).is_ok_and(|n| n.as_str() == "Hand_R" || n.as_str().strip_prefix("Hand_R_").is_some_and(|r| r.chars().all(|c| c.is_ascii_digit()))));
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn swing(
    time: Res<Time>,
    (store, db): (Res<SettingsStore>, Res<super::ItemDb>),
    mut game: ResMut<CurrentGame>,
    vitals: Res<super::PlayerVitals>,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    mut bosses: Query<(&Transform, &LevelZombie, &Zombie, &mut BossSwing, &mut UpperAction), Without<Dead>>,
    hands: Query<&GlobalTransform>,
    mut sound: MessageWriter<crate::audio::Sound>,
) {
    let Ok((me, body)) = player.single() else { return };
    let dt = time.delta_secs();
    let mult = difficulty(&store, "AI", "ZOMBIE DAMAGE MULTIPLIER");
    let mut rng = rand::thread_rng();
    for (t, z, brain, mut s, mut action) in &mut bosses {
        let Some(b) = z.kind.boss() else { continue };
        let (over, window) = s.tick(dt, b.rate, b.hit_window);
        if window && !vitals.is_dead() && game.state.is_some() {
            let hand = s.hand.and_then(|h| hands.get(h).ok()).map(GlobalTransform::translation);
            if let Some(hand) = hand
                && hand_touches(hand, b.hand_radius, me.translation, body.radius, body.height)
            {
                s.hit = true;
                info!("{} hit the survivor", z.kind.class_name());
                super::survival::zombie_hit(&mut game, &db, &store, z.kind.class(), mult);
            }
        }
        // ZombieBoss_Attack runs while the tree is on the chase branch and InRangeForMelee?.
        let chasing = brain.brain == Brain::Chase && !vitals.is_dead();
        if over && chasing && in_melee_range(z.kind.centre(t), me.translation, b.reach) {
            s.start(&mut rng);
            action.start_at(ActionKind::Attack, Some(b.rate));
            sound.write(crate::audio::Sound::at("ZombieGrowlShort", t.translation + Vec3::Y));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boss_classes_carry_their_blueprint_values() {
        let Some(small) = super::super::zombies::ZombieKind::from_class("BP_ZombieBoss_Military_Small_C") else { return };
        let b = small.boss().unwrap();
        assert_eq!((small.class().health, small.class().damage, small.roam_speed(), small.alert_speed()), (2000.0, 30.0, 0.8, 5.0));
        assert_eq!((b.reach, b.hand_radius, b.rate, b.corpse_secs, b.skill_point_chance), (5.0, 0.6, 0.5, 1200.0, 0.25));
        assert_eq!((b.killed.as_str(), small.peripheral(), small.class().loot_table.as_deref()), ("Zombie Boss", 110.0, Some("ZombieBossSmall")));
        assert_eq!(b.unique_sets, ["ColonelsRevenge", "Crusher", "Deadeye"]);
        assert_eq!(small.mesh_scale(), Vec3::new(1.1, 1.2, 1.1));
        assert_eq!(small.voice(), Some("ZombieBossBreathLoopCue"));
        assert_eq!(small.spawn_multiplier(), "ZOMBIE BOSS SPAWN AMOUNT MULTIPLIER");
        assert!(!small.shovable() && small.boss().is_some());
        // The swing clip is the boss's own, on the upper body; no flinch.
        let set = small.anim_set();
        assert_eq!(set.action_clip(crate::models::Stance::Unarmed, ActionKind::Attack), Some("Zombie/ZombieBoss/Anim_Zombie_attack1"));
        assert_eq!(set.action_clip(crate::models::Stance::Unarmed, ActionKind::Hit), None);
        // Every boss class has data and a mesh.
        let bosses: Vec<_> = super::super::zombies::zombie_classes().iter().filter(|c| c.1.boss.is_some()).collect();
        assert_eq!(bosses.len(), 10);
        assert!(bosses.iter().all(|c| c.1.meshes.len() == 1 && c.1.health >= 2000.0));
        let big = super::super::zombies::ZombieKind::from_class("BP_ZombieBoss_Military_C").unwrap();
        assert_eq!((big.class().health, big.class().loot_table.as_deref(), big.boss().unwrap().killed.as_str()), (4000.0, Some("ZombieBoss"), "Zombie Boss"));
    }

    #[test]
    fn swings_open_the_arm_window_at_the_montage_rate() {
        let mut s = BossSwing::default();
        assert_eq!(s.tick(0.1, 0.5, [0.25, 0.42]), (true, false), "idle: free to swing");
        s.start(&mut rand::thread_rng());
        assert!((1.35..1.75).contains(&s.delay));
        // Montage 0.25-0.42 s at RateScale 0.5 is 0.5-0.84 s of real time.
        assert_eq!(s.tick(0.45, 0.5, [0.25, 0.42]), (false, false));
        assert_eq!(s.tick(0.1, 0.5, [0.25, 0.42]), (false, true));
        s.hit = true;
        assert_eq!(s.tick(0.1, 0.5, [0.25, 0.42]), (false, false), "one hit a swing");
        // Start_Attacking clears after the Delay.
        assert!(s.tick(1.2, 0.5, [0.25, 0.42]).0, "past the longest Delay (1.75 s)");
        assert!(s.t.is_none());
    }

    #[test]
    fn the_hand_reaches_the_capsule_not_the_feet() {
        let feet = Vec3::ZERO;
        assert!(hand_touches(Vec3::new(0.9, 1.2, 0.0), 0.6, feet, 0.35, 1.64));
        assert!(!hand_touches(Vec3::new(1.0, 1.2, 0.0), 0.6, feet, 0.35, 1.64));
        assert!(hand_touches(Vec3::new(0.0, 2.2, 0.0), 0.6, feet, 0.35, 1.64), "over the head (top sphere at 1.29 m)");
        assert!(!hand_touches(Vec3::new(0.0, 2.3, 0.0), 0.6, feet, 0.35, 1.64));
        // InRangeForMelee? by actor centres: 5 m.
        assert!(in_melee_range(Vec3::new(0.0, 0.82, 4.9), feet, 5.0));
        assert!(!in_melee_range(Vec3::new(0.0, 0.82, 5.1), feet, 5.0));
    }
}
