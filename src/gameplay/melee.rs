//! Melee attacks, from BP_PlayerCharacter's IA_PrimaryAction events, BP_MeleePickup and the
//! AnimNotify_MeleeHitDetect(_Power) notify states (decompiled).
//!
//! - **Input**: IA_PrimaryAction (LMB, no triggers) with a melee weapon drawn. Its Completed
//!   event (the release) reads ElapsedTime: <= 0.5 s is a normal swing, more a power attack.
//!   While it's held (Triggered) past 0.5 s, BP_Ingame's PowerAttackAnim plays on the crosshair
//!   (a 0.5 s turn of 360° and a scale 1 -> 1.5 -> 1, cubic keys; PlayAnimationForward each
//!   Triggered, so it goes round while the button is held).
//! - **Guards**: Attacking? (set at the swing, cleared DelayBetweenSwings x (1, or 2 without the
//!   StaminaDrain to spare) later), a weapon at 0 durability ("Item durability too low"), the
//!   safe zone. A power attack crouched says "You cannot use a power attack while crouched" and
//!   swings normally.
//! - **Normal**: Svr_ReduceStamina(StaminaDrain), the whoosh, CrouchedMeleeAttackMontage when
//!   crouched or looking 45° down (camera pitch <= -45), else a random NormalMeleeAttackMontages,
//!   at SwingSpeed x StrengthSpeedMultiplier x (1, or 0.5 short of stamina).
//! - **Power**: StaminaDrain x rand(6, 8) off, a random PowerMeleeAttackMontages at x (0.75, or
//!   0.45 short of stamina).
//! - **Hits**: inside the montage's MeleeHitDetect(_Power) window the weapon's Start / End
//!   sockets are swept (28 cm spheres; 34 cm for power) every frame; each actor is hit once a
//!   window (HitActors). Normal hits stop at MaxMeleeHits (MaxActorsToHitInSwing? with MELEE MULTI
//!   HIT, else 1); power hits have no cap and do x2.5 (DamageCalc's Power?). Trees and rocks take
//!   3-4 hits from a power swing (1 from a normal one); a power swing doesn't reach trash piles
//!   (Event_OnHitPower has no TrashObject case).
//! - DamageCalc: MeleeDamage x 2 from behind (forward · direction >= 0.25), CriticalHit,
//!   x StrengthMultiplier, NightCreatureCheck, DurabilityCheck, x 2.5 power, x 1.5 head.
//!
//! Ours: the sweep is the old arc (2.2 m, ±60° of the survivor's facing) tested each frame of
//! the window, since our weapon models have no Start / End sockets; no head hits (the arc can't
//! tell); the power montages' root motion isn't applied; one crosshair stands in for
//! CombatCrosshair. Not ported: SmoothYawRotation (turning to the camera), Game.MeleeAim.

use bevy::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;

use super::combat::{Armed, Dead, Health, backstab, drawn, night_creature, store_bool, worn_damage};
use super::items::{ItemDb, MeleeDef};
use super::particles::Fx;
use super::survival::difficulty;
use crate::level::LevelZombie;
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, CurrentGame};
use crate::models::{ActionKind, CharacterModel, UpperAction};
use crate::progression::passive::Passive;
use crate::progression::tech;

/// IA_PrimaryAction's ElapsedTime that makes a power attack.
pub const POWER_HOLD: f32 = 0.5;
/// DamageCalc's Power? multiplier.
pub const POWER_DAMAGE: f32 = 2.5;
/// PowerAttackAnim: 0.5 s.
pub const CUE_SECS: f32 = 0.5;
/// Ours: the swing's reach and half-angle (the old guesses).
const REACH: f32 = 2.2;
const CONE: f32 = 60.0;

/// A swing under way: its montage time and hit window.
#[derive(Clone, Debug)]
pub struct ActiveSwing {
    pub power: bool,
    pub montage: String,
    /// Montage seconds played (x the play rate).
    pub t: f32,
    pub rate: f32,
    /// The hit window, montage seconds.
    pub window: (f32, f32),
    /// HitActors.
    pub hit: Vec<Entity>,
    pub max_hits: usize,
    /// The harvest / object swing sent for this window.
    pub swung: bool,
}

/// The melee state: LMB held, the swing under way, the crosshair cue.
#[derive(Resource, Default)]
pub struct Melee {
    /// IA_PrimaryAction's ElapsedTime while held.
    pub hold: Option<f32>,
    pub swing: Option<ActiveSwing>,
    /// PowerAttackAnim's time while it plays.
    pub cue: Option<f32>,
    /// The last swing started (for the tour): power?, montage, rate.
    pub last: Option<(bool, String, f32)>,
}

/// Normal or power by the hold, and whether a crouched power attack falls back.
pub fn kind_of(held: f32, crouched: bool) -> (bool, bool) {
    let power = held > POWER_HOLD;
    (power && !crouched, power && crouched)
}

/// The montage a swing plays: crouched / looking down = CrouchedMeleeAttackMontage, else a
/// random pick from the normal or power list.
pub fn pick_montage(def: &MeleeDef, power: bool, low: bool, rng: &mut impl Rng) -> Option<String> {
    if !power && low && def.crouched.is_some() {
        return def.crouched.clone();
    }
    let list = if power { &def.power } else { &def.normal };
    list.choose(rng).cloned()
}

/// The play rate: SwingSpeed x StrengthSpeedMultiplier x (normal 1 / 0.5, power 0.75 / 0.45).
pub fn swing_rate(swing_speed: f32, strength_speed: f32, enough: bool, power: bool) -> f32 {
    let k = match (power, enough) {
        (false, true) => 1.0,
        (false, false) => 0.5,
        (true, true) => 0.75,
        (true, false) => 0.45,
    };
    (swing_speed * strength_speed * k).max(0.1)
}

/// Attacking?'s Delay: DelayBetweenSwings x (1, or 2 short of stamina).
pub fn lockout(delay: f32, enough: bool) -> f32 {
    delay * if enough { 1.0 } else { 2.0 }
}

/// The stamina a swing costs: StaminaDrain (x rand(6, 8) for a power attack).
pub fn stamina_cost(drain: f32, power: bool, rng: &mut impl Rng) -> f32 {
    if power { drain * rng.gen_range(6..=8) as f32 } else { drain }
}

/// PowerAttackAnim at `t`: the crosshair's turn (radians) and scale.
pub fn cue_pose(t: f32) -> (f32, f32) {
    let a = (t / CUE_SECS).clamp(0.0, 1.0);
    // Cubic keys with flat tangents: smoothstep between keys.
    let ease = |x: f32| x * x * (3.0 - 2.0 * x);
    let turn = ease(a) * std::f32::consts::TAU;
    let scale = if a < 0.5 { 1.0 + 0.5 * ease(a * 2.0) } else { 1.5 - 0.5 * ease(a * 2.0 - 1.0) };
    (turn, scale)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Melee>()
        .add_systems(OnEnter(AppState::InGame), |mut m: ResMut<Melee>| *m = Melee::default())
        .add_systems(Update, crosshair_cue.run_if(in_state(AppState::InGame)));
}

/// The crosshair's PowerAttackAnim.
fn crosshair_cue(time: Res<Time>, mut melee: ResMut<Melee>, mut cross: Query<&mut UiTransform, With<super::hud::Crosshair>>) {
    let held = melee.hold.is_some_and(|h| h >= POWER_HOLD);
    melee.cue = match melee.cue {
        Some(t) if t + time.delta_secs() < CUE_SECS => Some(t + time.delta_secs()),
        // Finished: again while the button's still held past 0.5 s.
        _ if held => Some(0.0),
        _ => None,
    };
    let (turn, scale) = melee.cue.map_or((0.0, 1.0), cue_pose);
    for mut t in &mut cross {
        t.rotation = Rot2::radians(turn);
        t.scale = Vec2::splat(scale);
    }
}

/// LMB with a melee weapon: the hold, the swing on release, the hits in its window.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn melee(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    db: Res<ItemDb>,
    store: Res<SettingsStore>,
    mut armed: ResMut<Armed>,
    mut state: ResMut<Melee>,
    mut game: ResMut<CurrentGame>,
    camera: Query<&GlobalTransform, With<super::combat::PlayerCamera>>,
    mut player: Query<(&Transform, &mut UpperAction, Option<&crate::models::Locomotion>), (With<CharacterModel>, With<crate::level::KinematicBody>)>,
    mut zombies: Query<(Entity, &Transform, &mut Health, Option<&mut UpperAction>), (With<LevelZombie>, Without<Dead>, Without<CharacterModel>)>,
    safe: Res<super::zones::SafeZones>,
    (mut notify, mut numbers, mut sound, mut swings, mut fx): (
        MessageWriter<super::notify::Notify>,
        MessageWriter<super::damage_numbers::DamageNumber>,
        MessageWriter<crate::audio::Sound>,
        MessageWriter<super::harvest::Swing>,
        MessageWriter<Fx>,
    ),
) {
    let dt = time.delta_secs();
    let weapon = drawn(&armed, &mut game).cloned();
    let def = weapon.as_ref().and_then(|w| db.items.get(&w.item)).filter(|d| d.kind == "Melee");
    let (Some(weapon), Some(def), Ok((me, mut action, loco))) = (weapon, def, player.single_mut()) else {
        state.hold = None;
        state.swing = None;
        return;
    };
    let me = *me;
    let crouched = loco.is_some_and(|l| l.crouched());
    let mut rng = rand::thread_rng();
    let melee_def = def.melee.clone().unwrap_or_default();

    // The hold, and the release.
    if mouse.just_pressed(MouseButton::Left) && state.hold.is_none() && !safe.active() {
        state.hold = Some(0.0);
    }
    let mut released = None;
    if let Some(h) = state.hold.as_mut() {
        if mouse.pressed(MouseButton::Left) {
            *h += dt;
        } else {
            released = Some(*h);
            state.hold = None;
        }
    }
    if let Some(held) = released
        && armed.cooldown <= 0.0
    {
        let durability = weapon.durability.unwrap_or(def.durability);
        if def.durability > 0.0 && durability <= 0.0 {
            notify.write(super::notify::Notify::new("Item durability too low"));
        } else {
            let (power, refused) = kind_of(held, crouched);
            if refused {
                notify.write(super::notify::Notify::new("You cannot use a power attack while crouched"));
            }
            // Looking 45° down (the camera's pitch <= -45).
            let low = crouched || camera.single().is_ok_and(|c| c.forward().y <= -(45f32.to_radians().sin()));
            let drain = weapon.stat(&db, "StaminaDrain").unwrap_or(0.0);
            let cost = stamina_cost(drain, power, &mut rng);
            let (stamina, strength_speed) = game.state.as_mut().map_or((100.0, 1.0), |s| {
                // Svr_ReduceStamina first; the rate and the lockout read what's left.
                s.survival.stamina = (s.survival.stamina - cost).max(0.0);
                (s.survival.stamina, s.passive.strength_speed)
            });
            let enough = stamina >= drain;
            let montage = pick_montage(&melee_def, power, low, &mut rng);
            let info = montage.as_ref().and_then(|m| db.melee_montages.get(m));
            let rate = swing_rate(def.swing_speed, strength_speed, enough, power);
            armed.cooldown = lockout(if melee_def.delay > 0.0 { melee_def.delay } else { 0.75 }, enough);
            let clip = info.and_then(|i| i.clip.as_deref()).and_then(crate::models::anim::melee_clip).unwrap_or(0);
            action.start_at(ActionKind::Swing(clip), Some(rate));
            sound.write(crate::audio::Sound::at("whoosh_weapon_knife_swing_01_Cue", me.translation));
            let window = info.and_then(|i| i.hit).map_or((0.25, 0.45), |[s, d]| (s, s + d));
            let multi = store_bool(&store, "General", "MELEE MULTI HIT ENABLED?");
            let max_hits = if power { usize::MAX } else if multi { melee_def.max_hits.max(1) as usize } else { 1 };
            let name = montage.unwrap_or_default();
            info!("melee: {} swing {name} at {rate:.2} (held {held:.2} s, {cost:.1} stamina, lockout {:.2} s)", if power { "power" } else { "normal" }, armed.cooldown);
            state.last = Some((power, name.clone(), rate));
            state.swing = Some(ActiveSwing { power, montage: name, t: 0.0, rate, window, hit: Vec::new(), max_hits, swung: false });
        }
    }

    // The hit window.
    let Some(sw) = state.swing.as_mut() else { return };
    sw.t += dt * sw.rate;
    if sw.t > sw.window.1 {
        state.swing = None;
        return;
    }
    if sw.t < sw.window.0 {
        return;
    }
    let facing = (me.rotation * Vec3::Z).with_y(0.0).normalize_or_zero();
    let base = weapon.stat(&db, "MeleeDamage").unwrap_or(20.0);
    if !sw.swung {
        // The same sweep can meet a tree, a rock, a trash pile or an object.
        sw.swung = true;
        swings.write(super::harvest::Swing { feet: me.translation, facing, weapon: weapon.item.clone(), damage: base, power: sw.power });
    }
    if sw.hit.len() >= sw.max_hits {
        return;
    }
    let durability = weapon.durability.unwrap_or(def.durability);
    let crit_chance = weapon.stat(&db, "CriticalHitChance").unwrap_or(0.0) * if game.state.as_ref().is_some_and(|s| s.tech.has(tech::CRITICAL_HITTER)) { 2.0 } else { 1.0 };
    let crit_mult = weapon.stat(&db, "CriticalHitMultiplier").unwrap_or(0.0);
    let (strength, night) = game.state.as_ref().map_or((1.0, 1.0), |s| (s.passive.multiplier(Passive::Strength), night_creature(s.tech.has(tech::NIGHT_CREATURE), s.environment.time_of_day_hours)));
    let degrade = store_bool(&store, "General", "MELEE DAMAGE DEGRADATION ENABLED?");
    let mut fresh: Vec<(f32, Entity)> = zombies
        .iter()
        .filter(|(e, t, _, _)| {
            let to = (t.translation - me.translation) * Vec3::new(1.0, 0.0, 1.0);
            !sw.hit.contains(e) && to.length() < REACH && facing.angle_between(to.normalize_or_zero()).to_degrees() < CONE
        })
        .map(|(e, t, _, _)| (t.translation.distance(me.translation), e))
        .collect();
    fresh.sort_by(|a, b| a.0.total_cmp(&b.0));
    let room = sw.max_hits - sw.hit.len();
    let power = sw.power;
    for (_, e) in fresh.into_iter().take(room) {
        sw.hit.push(e);
        let Ok((_, t, mut h, flinch)) = zombies.get_mut(e) else { continue };
        let at = t.translation;
        let back = backstab(t.rotation * Vec3::Z, at - me.translation);
        // DamageCalc: back, CriticalHit, Strength, NightCreature, DurabilityCheck, Power.
        let critical = rng.gen_range(0.0..100.0) < crit_chance;
        let dmg = base * back * if critical { 1.0 + crit_mult / 100.0 } else { 1.0 };
        let dmg = worn_damage(dmg * strength * night, durability, degrade) * if power { POWER_DAMAGE } else { 1.0 };
        if let Some(mut a) = flinch {
            a.start(ActionKind::Hit);
        }
        h.hp -= dmg;
        // HitBody: one of Body_Flesh_1-5.
        sound.write(crate::audio::Sound::at(format!("Body_Flesh_{}_Cue", rng.gen_range(1..=5)), at + Vec3::Y));
        // BP_MeleePickup: FX_BloodHeadshot at the hit, scale 0.6.
        fx.write(Fx::at("FX_BloodHeadshot", at + Vec3::Y * 1.2).pointing(me.translation - at).scaled(Vec3::splat(0.6)));
        numbers.write(super::damage_numbers::DamageNumber::hit(dmg, critical, at + Vec3::Y * 1.2));
        info!("melee: {} hit for {dmg:.1}", if power { "power" } else { "normal" });
        // HitBody / HitHead: Client_StrengthXP(1-2), ReduceDurability(1).
        if let Some(st) = game.state.as_mut() {
            st.passive.add_xp(Passive::Strength, rng.gen_range(1.0..=2.0));
        }
        if store_bool(&store, "General", "WEAPON DURABILITY ENABLED?")
            && let Some(w) = drawn(&armed, &mut game)
        {
            let loss = def.reduce_durability * difficulty(&store, "General", "WEAPON DURABILITY MULTIPLIER");
            let now = w.durability.unwrap_or(def.durability);
            w.durability = Some((now - loss).max(0.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> MeleeDef {
        MeleeDef { normal: vec!["N1".into(), "N2".into()], power: vec!["P1".into()], crouched: Some("C".into()), delay: 0.55, max_hits: 3 }
    }

    #[test]
    fn the_hold_decides() {
        assert_eq!(kind_of(0.3, false), (false, false));
        assert_eq!(kind_of(0.5, false), (false, false), "ElapsedTime <= 0.5 is a normal swing");
        assert_eq!(kind_of(0.8, false), (true, false));
        assert_eq!(kind_of(0.8, true), (false, true), "crouched: refused, a normal swing");
    }

    #[test]
    fn montages_by_kind_and_stance() {
        let mut rng = rand::thread_rng();
        assert_eq!(pick_montage(&def(), false, true, &mut rng).as_deref(), Some("C"));
        assert_eq!(pick_montage(&def(), true, false, &mut rng).as_deref(), Some("P1"));
        assert!(matches!(pick_montage(&def(), false, false, &mut rng).as_deref(), Some("N1" | "N2")));
    }

    #[test]
    fn rates_costs_and_lockouts() {
        assert_eq!(swing_rate(1.1, 1.0, true, false), 1.1);
        assert!((swing_rate(1.0, 1.0, true, true) - 0.75).abs() < 1e-6);
        assert!((swing_rate(1.0, 1.0, false, true) - 0.45).abs() < 1e-6);
        assert_eq!(lockout(0.55, true), 0.55);
        assert_eq!(lockout(0.55, false), 1.1);
        let mut rng = rand::thread_rng();
        let c = stamina_cost(4.0, true, &mut rng);
        assert!((24.0..=32.0).contains(&c));
        assert_eq!(stamina_cost(4.0, false, &mut rng), 4.0);
    }

    #[test]
    fn the_cue_turns_once_and_swells() {
        assert_eq!(cue_pose(0.0), (0.0, 1.0));
        let (_, mid) = cue_pose(0.25);
        assert!((mid - 1.5).abs() < 1e-5);
        let (turn, end) = cue_pose(0.5);
        assert!((turn - std::f32::consts::TAU).abs() < 1e-5 && (end - 1.0).abs() < 1e-5);
    }

    #[test]
    fn weapons_name_real_montages() {
        let db = ItemDb::load();
        let melee: Vec<_> = db.items.iter().filter_map(|(k, d)| d.melee.as_ref().map(|m| (k, m))).collect();
        assert!(melee.len() >= 10);
        for (k, m) in melee {
            for name in m.normal.iter().chain(&m.power).chain(m.crouched.as_ref()) {
                let mt = db.melee_montages.get(name).unwrap_or_else(|| panic!("{k}: {name} missing"));
                let clip = mt.clip.as_deref().unwrap_or_default();
                assert!(crate::models::anim::melee_clip(clip).is_some(), "{k}: {clip} not in MELEE_CLIPS");
                assert!(mt.hit.is_some(), "{k}: {name} has no hit window");
            }
            assert!(!m.power.is_empty(), "{k}: no power montages");
        }
    }
}
