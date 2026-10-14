//! The survivor's vitals while playing: `SurvivalStats` (in the session's `GameState`) ticks
//! hunger/thirst (HungerThirstComponent), stamina (StaminaComponent) and health regeneration
//! (BP_PlayerCharacter.RegenHealth) as the Blueprints do, zombies in reach hurt, consumables
//! restore, and death respawns the player at a PlayerStart.

use bevy::prelude::*;

use super::interact::GameOverlay;
use crate::level::{KinematicBody, Level, LevelZombie, Terrain};
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{CharacterModel, Locomotion};
use crate::progression::passive::Passive;
use crate::survival::stats::SurvivalStats;
use rand::Rng;

/// Ticking clock for each zombie's swings, and the interval to the next one.
#[derive(Component, Default)]
struct SwingTimer(f32, f32);

#[derive(Resource, Default)]
pub struct PlayerVitals {
    /// Seconds since death, while dead.
    dead_for: Option<f32>,
}

#[derive(Component)]
struct DeathBanner;

pub fn plugin(app: &mut App) {
    app.init_resource::<PlayerVitals>()
        .init_resource::<NeedTimers>()
        .init_resource::<LandingStun>()
        .add_systems(OnEnter(AppState::InGame), |mut v: ResMut<PlayerVitals>| v.dead_for = None)
        .add_systems(
            Update,
            (tick_vitals, zombie_attacks, fall_damage, death_and_respawn)
                .chain()
                .run_if(in_state(AppState::InGame))
                .run_if(in_state(MenuScreen::None)),
        );
}

impl PlayerVitals {
    pub fn is_dead(&self) -> bool {
        self.dead_for.is_some()
    }
}

pub(crate) fn difficulty(store: &SettingsStore, page: &str, label: &str) -> f32 {
    match store.find(&row_key("difficulty", page, label)) {
        Some(Value::Num(v)) => v,
        _ => 1.0,
    }
}

/// Damage the survivor takes goes through AnyDamage: ÷ the passive ToughnessMultiplier, then
/// CalculatePlayerDamage: worn armor divides it and (with ARMOR DURABILITY ENABLED?) wears down.
/// A hit the survivor lives through gives 2-4 Toughness XP.
pub fn take_hit(game: &mut CurrentGame, db: &super::ItemDb, store: &SettingsStore, damage: f32) -> f32 {
    let damage = damage / toughness(game.state.as_ref());
    let taken = match game.inventory.as_mut() {
        Some(inv) => super::equipment::armor_damage(inv, db, damage, armor_wear(store)),
        None => damage,
    };
    if let Some(s) = game.state.as_mut()
        && s.survival.health - taken > 0.0
    {
        toughness_xp(s);
    }
    taken
}

/// The passive ToughnessMultiplier (AnyDamage divides by it).
fn toughness(state: Option<&crate::engine::GameState>) -> f32 {
    state.map_or(1.0, |s| s.passive.multiplier(Passive::Toughness)).max(0.01)
}

/// AnyDamage's AddXP_Toughness(2-4) after a hit the survivor lives through.
fn toughness_xp(s: &mut crate::engine::GameState) {
    s.passive.add_xp(Passive::Toughness, rand::thread_rng().gen_range(2.0..=4.0));
}

/// ARMOR DURABILITY MULTIPLIER when ARMOR DURABILITY ENABLED? is on.
fn armor_wear(store: &SettingsStore) -> Option<f32> {
    (store.find(&row_key("difficulty", "General", "ARMOR DURABILITY ENABLED?")) == Some(Value::Bool(true))).then(|| difficulty(store, "General", "ARMOR DURABILITY MULTIPLIER"))
}

/// BP_PlayerCharacter's HungerThirstComponent / StaminaComponent overrides and the RegenHealth timer.
const HUNGER_PER_S: f32 = 0.015 / 0.5; // ReduceHungerAmount every 0.5 s: 100 -> 0 in ~56 min
const THIRST_PER_S: f32 = 0.02 / 0.501; // ReduceThirstAmount every 0.501 s: ~42 min
const STARVE_DAMAGE: f32 = 5.0; // ApplyHunger/ThirstDamage: every 5 s at 0
const STARVE_EVERY: f32 = 5.0;
const STAMINA_DRAIN_PER_S: f32 = 0.03 / 0.01; // StaminaDrain every 0.01 s while sprinting and moving
const STAMINA_RECOVER_PER_S: f32 = 0.04 / 0.01; // StaminaRecoverAmount every 0.01 s
const STAMINA_RECOVER_DELAY: f32 = 1.0; // RecoverStamina's initial delay (÷ FitnessStaminaRegainMultiplier)
const REGEN_PER_S: f32 = 0.08 / 0.1; // RegenHealth every 0.1 s, up to 80 HP
const REGEN_BELOW: f32 = 80.0;
const REGEN_DELAY: f32 = 4.0; // Event_RegenerateHealth's RetriggerableDelay after damage

/// The difficulty CVars these read (Difficulty > Player).
#[derive(Clone, Copy, Debug)]
pub struct NeedRates {
    pub hunger: f32,
    pub thirst: f32,
    pub stamina: f32,
    pub stamina_recovery: f32,
    pub regen: f32,
    /// The passive Fitness multipliers: FitnessMultiplier (recovery amount), FitnessStaminaMultiplier
    /// (sprint drain ÷) and FitnessStaminaRegainMultiplier (recovery delay 1 / it).
    pub fitness: f32,
    pub fitness_stamina: f32,
    pub fitness_regain: f32,
}

impl Default for NeedRates {
    fn default() -> Self {
        Self { hunger: 1.0, thirst: 1.0, stamina: 1.0, stamina_recovery: 1.0, regen: 1.0, fitness: 1.0, fitness_stamina: 1.0, fitness_regain: 1.0 }
    }
}

/// The components' timers.
#[derive(Resource, Default, Debug)]
pub struct NeedTimers {
    starve: f32,
    since_sprint: f32,
    since_hurt: f32,
    last_health: Option<f32>,
}

/// One step of HungerThirstComponent, StaminaComponent and RegenHealth. `sprinting`: sprinting
/// and moving; `regen`: the Skill_HealthRegen tech is unlocked (no regeneration without it);
/// `hit`: what a starvation ApplyDamage comes to after armor ([`Armor`]).
pub fn step_needs(s: &mut SurvivalStats, t: &mut NeedTimers, dt: f32, r: NeedRates, sprinting: bool, regen: bool, hit: &mut dyn FnMut(f32) -> f32) {
    s.hunger = (s.hunger - HUNGER_PER_S * r.hunger * dt).clamp(0.0, 100.0);
    s.thirst = (s.thirst - THIRST_PER_S * r.thirst * dt).clamp(0.0, 100.0);
    t.starve += dt;
    while t.starve >= STARVE_EVERY {
        t.starve -= STARVE_EVERY;
        for empty in [s.hunger <= 0.0, s.thirst <= 0.0] {
            if empty {
                s.health = (s.health - hit(STARVE_DAMAGE)).max(0.0);
            }
        }
    }
    if sprinting {
        s.stamina = (s.stamina - STAMINA_DRAIN_PER_S / r.fitness_stamina.max(0.01) * r.stamina * dt).max(0.0);
        t.since_sprint = 0.0;
    } else {
        t.since_sprint += dt;
        if t.since_sprint >= STAMINA_RECOVER_DELAY / r.fitness_regain.max(0.01) {
            s.stamina = (s.stamina + STAMINA_RECOVER_PER_S * r.fitness * r.stamina_recovery * dt).min(s.max_stamina);
        }
    }
    // Any damage (re)starts the 4 s wait.
    if t.last_health.is_some_and(|h| s.health < h) {
        t.since_hurt = 0.0;
    } else {
        t.since_hurt += dt;
    }
    if regen && t.since_hurt >= REGEN_DELAY && s.health > 0.0 && s.health <= REGEN_BELOW {
        s.health = (s.health + REGEN_PER_S * r.regen * dt).min(s.max_health);
    }
    t.last_health = Some(s.health);
}

/// The tech tree's Skill_HealthRegen.
fn health_regen_unlocked(state: &crate::engine::GameState) -> bool {
    state.tech.has(crate::progression::tech::HEALTH_REGEN)
}

fn tick_vitals(
    time: Res<Time>,
    mut game: ResMut<CurrentGame>,
    db: Res<super::ItemDb>,
    store: Res<SettingsStore>,
    vitals: Res<PlayerVitals>,
    mut timers: ResMut<NeedTimers>,
    player: Query<&Locomotion, With<crate::level::KinematicBody>>,
) {
    if vitals.is_dead() {
        return;
    }
    let CurrentGame { state, inventory, .. } = &mut *game;
    let Some(state) = state.as_mut() else { return };
    let wear = armor_wear(&store);
    let tough = toughness(Some(state));
    let mut hits = 0;
    let mut hit = |d: f32| {
        hits += 1;
        let d = d / tough;
        inventory.as_mut().map_or(d, |inv| super::equipment::armor_damage(inv, &db, d, wear))
    };
    // Crouch Sprint runs ToggleSprint too: it drains like a sprint.
    let sprinting = player.iter().any(|l| matches!(l, Locomotion::Sprint | Locomotion::CrouchSprint));
    let rates = NeedRates {
        hunger: difficulty(&store, "Player", "HUNGER DEPLETION MULTIPLIER"),
        thirst: difficulty(&store, "Player", "THIRST DEPLETION MULTIPLIER"),
        stamina: difficulty(&store, "Player", "STAMINA DEPLETION MULTIPLIER"),
        stamina_recovery: difficulty(&store, "Player", "STAMINA RECOVERY MULTIPLIER"),
        regen: difficulty(&store, "Player", "HEALTH REGENERATION MULTIPLIER"),
        fitness: state.passive.multiplier(Passive::Fitness),
        fitness_stamina: state.passive.fitness_stamina,
        fitness_regain: state.passive.fitness_stamina_regain,
    };
    let regen = health_regen_unlocked(state);
    let dt = time.delta_secs();
    // MedicalComponent / RadiationComponent: bleeds, heavy bleeds, radiation sickness.
    // (Their ticks ApplyDamage, so armor takes them too.)
    let hurt = state.effects.tick(dt, state.survival.radiation, &mut rand::thread_rng());
    let s = &mut state.survival;
    if hurt > 0.0 {
        s.health = (s.health - hit(hurt)).max(0.0);
    }
    s.bleeding = state.effects.bleeding();
    step_needs(s, &mut timers, dt, rates, sprinting, regen, &mut hit);
    // f_CheckStamina: 0.002-0.004 Fitness XP per 0.01 s tick while sprinting and moving.
    if sprinting && s.stamina > 0.0 {
        let xp = rand::thread_rng().gen_range(0.002..=0.004) * dt / 0.01;
        state.passive.add_xp(Passive::Fitness, xp);
    }
    let s = &state.survival;
    if s.health > 0.0 {
        for _ in 0..hits {
            toughness_xp(state);
        }
    }
}

/// BP_PlayerCharacter.OnLanded: by the speed of the landing (m/s), 15 / 30 / 60 / 80 damage from
/// 13 / 15 / 18 / 20 m/s, 999 from 22 m/s (about 5 m to 14 m drops at GravityScale 1.75).
pub fn fall_damage_for(speed: f32) -> Option<f32> {
    match speed {
        s if s >= 22.0 => Some(999.0),
        s if s >= 20.0 => Some(80.0),
        s if s >= 18.0 => Some(60.0),
        s if s >= 15.0 => Some(30.0),
        s if s >= 13.0 => Some(15.0),
        _ => None,
    }
}

/// Seconds the survivor can't move after a hard landing (OnLanded: DisableMovement, the
/// Hard_Landing_Montage, a 1.5 s Delay).
#[derive(Resource, Default)]
pub struct LandingStun(pub f32);

/// The hard landing's 1.5 s.
pub const LANDING_STUN: f32 = 1.5;

/// OnLanded: fall damage, and below the fatal speed a broken bone (`BrokenBone? = true`) and the
/// stun. The hard landing montage isn't played.
fn fall_damage(
    time: Res<Time>,
    mut game: ResMut<CurrentGame>,
    (db, store): (Res<super::ItemDb>, Res<SettingsStore>),
    vitals: Res<PlayerVitals>,
    mut stun: ResMut<LandingStun>,
    player: Query<&KinematicBody, (With<CharacterModel>, Without<LevelZombie>)>,
) {
    stun.0 = (stun.0 - time.delta_secs()).max(0.0);
    let Ok(body) = player.single() else { return };
    let Some(damage) = body.landed.and_then(fall_damage_for) else { return };
    if vitals.is_dead() || game.state.is_none() {
        return;
    }
    let taken = take_hit(&mut game, &db, &store, damage);
    info!("hard landing at {:.1} m/s: {damage} damage ({taken:.1} after armor)", body.landed.unwrap_or(0.0));
    let Some(state) = game.state.as_mut() else { return };
    state.survival.health = (state.survival.health - taken).max(0.0);
    if damage < 999.0 {
        state.effects.add_broken_bone(&mut rand::thread_rng());
        stun.0 = LANDING_STUN;
    }
}

#[allow(clippy::type_complexity)]
fn zombie_attacks(
    mut commands: Commands,
    time: Res<Time>,
    (store, db): (Res<SettingsStore>, Res<super::ItemDb>),
    mut game: ResMut<CurrentGame>,
    vitals: Res<PlayerVitals>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
    mut zombies: Query<(Entity, &Transform, &LevelZombie, &Locomotion, Option<&mut SwingTimer>), Without<super::combat::Dead>>,
) {
    let Ok(p) = player.single() else { return };
    if vitals.is_dead() || game.state.is_none() {
        return;
    }
    let mult = difficulty(&store, "AI", "ZOMBIE DAMAGE MULTIPLIER");
    for (e, t, z, loco, timer) in &mut zombies {
        if *loco != Locomotion::Attack || t.translation.distance(p.translation) > super::zombies::MELEE_RANGE + 0.3 {
            continue;
        }
        let Some(mut timer) = timer else {
            commands.entity(e).insert(SwingTimer::default());
            continue;
        };
        timer.0 += time.delta_secs();
        // The class's DamageToDo per hit, a swing every TimeBetweenAttacks.
        if timer.1 == 0.0 {
            let (lo, hi) = z.kind.attack_interval();
            timer.1 = rand::Rng::gen_range(&mut rand::thread_rng(), lo..hi);
        }
        if timer.0 >= timer.1 {
            timer.0 = 0.0;
            timer.1 = 0.0;
            let c = z.kind.class();
            let taken = take_hit(&mut game, &db, &store, c.damage * mult);
            let Some(state) = game.state.as_mut() else { return };
            state.survival.take_damage(taken, false, 0.15);
            // `Medical Effects` rolls: Chance for Bleed / Heavy Bleed / Broken Bone (MedicalComponent
            // runs them), then Chance for Rad Damage adds Radiation Damage.
            let roll = |p: f32| rand::random::<f32>() < p;
            let (bleed, heavy, bone) = (roll(c.bleed), roll(c.heavy_bleed), roll(c.broken_bone));
            let mut rng = rand::thread_rng();
            if bleed {
                state.effects.add_bleed(&mut rng);
            }
            if heavy {
                state.effects.add_heavy_bleed(&mut rng);
            }
            if bone {
                state.effects.add_broken_bone(&mut rng);
            }
            state.survival.bleeding = state.effects.bleeding();
            if roll(c.rad_chance) {
                state.survival.radiation = (state.survival.radiation + c.rad_damage).min(100.0);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn death_and_respawn(
    mut commands: Commands,
    time: Res<Time>,
    mut game: ResMut<CurrentGame>,
    mut vitals: ResMut<PlayerVitals>,
    mut overlay: ResMut<GameOverlay>,
    store: Res<SettingsStore>,
    level: Option<Res<Level>>,
    terrain: Option<Res<Terrain>>,
    font: Res<crate::menus::widgets::MenuFont>,
    mut player: Query<(&mut Transform, &mut Locomotion), (With<CharacterModel>, With<KinematicBody>)>,
    banners: Query<Entity, With<DeathBanner>>,
) {
    let (Some(state), Ok((mut t, mut loco))) = (game.state.as_mut(), player.single_mut()) else { return };
    // Skill_Deathbringer: once each life, a killing blow leaves 25 health instead.
    if vitals.dead_for.is_none() && !state.survival.is_alive() && state.tech.deathbringer() {
        state.survival.health = crate::progression::tech::DEATHBRINGER_HEALTH;
        info!("deathbringer: back to {} health", state.survival.health);
        return;
    }
    match vitals.dead_for {
        None if !state.survival.is_alive() => {
            vitals.dead_for = Some(0.0);
            *loco = Locomotion::Dead;
            *overlay = GameOverlay::None;
            commands.spawn((
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), top: Val::Percent(40.0), justify_content: JustifyContent::Center, ..default() },
                DeathBanner,
                DespawnOnExit(AppState::InGame),
                children![crate::menus::widgets::label(&font, "YOU DIED", 48.0)],
            ));
        }
        Some(d) => {
            let d = d + time.delta_secs();
            vitals.dead_for = Some(d);
            *loco = Locomotion::Dead;
            if d < 4.0 {
                return;
            }
            // Respawn at the PlayerStart furthest from where they fell, fresh vitals.
            let starts = level.map(|l| l.player_starts()).unwrap_or_default();
            let mut at = starts.iter().copied().max_by(|a, b| a.distance(t.translation).total_cmp(&b.distance(t.translation))).unwrap_or(t.translation);
            at.y = terrain.as_ref().map_or(at.y, |tr| tr.height(at.x, at.z));
            t.translation = at;
            state.survival = SurvivalStats::default();
            state.tech.deathbringer_used = false;
            if store.find(&row_key("difficulty", "Player", "KEEP INVENTORY ON DEATH?")) != Some(Value::Bool(true)) {
                if let Some(inv) = game.inventory.as_mut() {
                    *inv = Default::default();
                }
            }
            vitals.dead_for = None;
            *loco = Locomotion::Idle;
            for b in &banners {
                commands.entity(b).despawn();
            }
        }
        None => {}
    }
}

#[cfg(test)]
mod needs_tests {
    use super::*;

    #[test]
    fn fitness_eases_sprinting() {
        let mut s = SurvivalStats::new();
        s.stamina = 50.0;
        let mut t = NeedTimers::default();
        let mut none = |d: f32| d;
        let fit = NeedRates { fitness: 1.01, fitness_stamina: 1.05, fitness_regain: 1.15, ..NeedRates::default() };
        // Sprint drain / FitnessStaminaMultiplier.
        step_needs(&mut s, &mut t, 1.0, fit, true, false, &mut none);
        assert!((s.stamina - (50.0 - 3.0 / 1.05)).abs() < 1e-3, "{}", s.stamina);
        // Recovery after 1 / 1.15 s (not the untrained 1 s), x FitnessMultiplier.
        let before = s.stamina;
        step_needs(&mut s, &mut t, 0.8, fit, false, false, &mut none);
        assert_eq!(s.stamina, before, "still waiting");
        step_needs(&mut s, &mut t, 0.1, fit, false, false, &mut none);
        assert!((s.stamina - (before + 4.0 * 1.01 * 0.1)).abs() < 1e-3, "{}", s.stamina);
    }

    fn run(s: &mut SurvivalStats, t: &mut NeedTimers, secs: f32, sprinting: bool, regen: bool) {
        for _ in 0..(secs * 100.0).round() as u32 {
            step_needs(s, t, 0.01, NeedRates::default(), sprinting, regen, &mut |d| d);
        }
    }

    #[test]
    fn falls_hurt_from_13_m_per_s() {
        assert_eq!(fall_damage_for(6.0), None); // a jump's landing
        assert_eq!(fall_damage_for(12.9), None);
        assert_eq!(fall_damage_for(13.0), Some(15.0));
        assert_eq!(fall_damage_for(16.0), Some(30.0));
        assert_eq!(fall_damage_for(19.0), Some(60.0));
        assert_eq!(fall_damage_for(21.0), Some(80.0));
        assert_eq!(fall_damage_for(30.0), Some(999.0));
    }

    #[test]
    fn hunger_and_thirst_drain_as_the_component_does() {
        let (mut s, mut t) = (SurvivalStats::default(), NeedTimers::default());
        run(&mut s, &mut t, 100.0, false, false);
        // 0.015 per 0.5 s and 0.02 per 0.501 s.
        assert!((s.hunger - 97.0).abs() < 0.05, "{}", s.hunger);
        assert!((s.thirst - (100.0 - 2.0 / 0.501)).abs() < 0.05, "{}", s.thirst);
        assert_eq!(s.health, 100.0);
        // Empty: 5 damage each per 5 s.
        s.hunger = 0.0;
        s.thirst = 0.0;
        run(&mut s, &mut t, 10.0, false, false);
        assert!((s.health - 80.0).abs() < 0.01, "{}", s.health);
    }

    #[test]
    fn sprinting_drains_stamina_and_it_recovers_after_a_second() {
        let (mut s, mut t) = (SurvivalStats::default(), NeedTimers::default());
        run(&mut s, &mut t, 10.0, true, false);
        assert!((s.stamina - 70.0).abs() < 0.1, "3/s: {}", s.stamina);
        run(&mut s, &mut t, 1.0, false, false);
        assert!((s.stamina - 70.0).abs() < 0.1, "waits 1 s: {}", s.stamina);
        run(&mut s, &mut t, 2.0, false, false);
        assert!((s.stamina - 78.0).abs() < 0.1, "4/s: {}", s.stamina);
        run(&mut s, &mut t, 40.0, true, false);
        assert_eq!(s.stamina, 0.0);
    }

    #[test]
    fn health_regenerates_to_80_only_with_the_skill_and_4_s_after_damage() {
        let (mut s, mut t) = (SurvivalStats::default(), NeedTimers::default());
        s.health = 50.0;
        run(&mut s, &mut t, 10.0, false, false);
        assert!((s.health - 50.0).abs() < 0.01, "no skill, no regen");
        // Hurt again: 4 s wait, then 0.8/s.
        s.health = 40.0;
        run(&mut s, &mut t, 6.0, false, true);
        assert!((s.health - 41.6).abs() < 0.1, "{}", s.health);
        run(&mut s, &mut t, 100.0, false, true);
        assert!(s.health > 80.0 && s.health < 80.1, "stops past 80: {}", s.health);
    }
}
