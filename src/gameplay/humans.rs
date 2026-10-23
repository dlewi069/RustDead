//! The human AI: SmartAI's BP_MasterAICharacter as BP_MasterBandit / BP_MasterMilitary /
//! BP_MasterScavenger and their children (zombies.json classes with a `human` block,
//! `extractor/gen_ai.py`), spawned by the prefabs' BP_AISpawningVolume_Prefab (`level::prefabs`)
//! and the level's BP_AISpawningVolume_RandomChanceToSpawn (`level::spawners`), with the
//! wandering trader (BP_MasterTrader, `traders.rs`) among them.
//! They run on the zombie brain (`zombies.rs`: senses from their AI controllers, roaming by the
//! SmartAI component's Min/Max Roam Distance and Roam Wait Time, the chase), with:
//!
//! - **Looks** (SetMeshAndMaterial): a random body from Meshes, and one of HairMeshes and
//!   BeardMeshes on the `head` bone (the random hair colour material isn't picked), and the
//!   wandering trader's BackpackMeshes pick on the Backpack socket.
//! - **Weapons** (SetWeapons): one of the class's RangedWeapon and one MeleeWeapon
//!   (BP_AIWeapon_*), the drawn one in its Equip Attach Socket; the survivor's stance animations
//!   for it (rifle, pistol, one-handed melee).
//! - **Range Attack** (BP_SmartAIComponent, trace weapons): once the survivor is in view within
//!   Range Attack Distance and the AI faces within 35°, a shot every Range Attack Frequency
//!   Min-Max s: its sound (at half volume), the muzzle flash, a SphereTraceSingle from the
//!   muzzle along a random direction within Accuracy x Spread (Trace) degrees of the target's
//!   centre, as far as the target plus 2.5 m. A hit takes the weapon's Damage for the hitbox it
//!   lands on (Is Hitbox?: head, chest, legs, feet; arms and hands aren't told apart), through
//!   the survivor's armor, and ApplyMedicalEffect(Bleed, 5 %). Max Magazine Ammo shots, then the
//!   reload montage. The shot is a noise zombies hear (Weapon Noise).
//! - **Melee**: within Melee Distance (2 m), a swing every Melee Attack Frequency for the melee
//!   weapon's chest damage.
//! - **Death** (`combat::kill_zombies`): HumanKills + 1, level x the faction's XP roll, a boss's
//!   25 % skill point, "Bandit Killed, XP Given: N"; the corpse loots by its LootTable plus its
//!   volume's UniqueLoot sets (AddUniqueLoot).
//!
//! Not ported: SmartAI's cover and strafing (EQS), fleeing at low health, searching, blocking,
//! AI-on-AI fights (zombies and humans ignore each other), the projectile weapons' homing,
//! hit reactions, a volume's `NoSpawn` area check (IgnoreNoSpawnArea?).

use bevy::prelude::*;
use rand::Rng;

use super::combat::Dead;
use super::zombies::{AiWeapon, Brain, Zombie, ai_weapons};
use crate::level::{KinematicBody, LevelZombie};
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{ActionKind, CharacterModel, Stance, UpperAction, model, ue_relative};

/// Range Attack only fires within this yaw of the target (NormalizedDeltaRotator's ±35°).
const FACING: f32 = 35.0;
/// The trace runs to the target's distance plus this (GetDistanceTo + 250 cm).
const OVERSHOOT: f32 = 2.5;
/// SphereTraceSingle's radius.
const TRACE_RADIUS: f32 = 0.05;
/// ApplyMedicalEffect(['Bleed'], 5.0, ...) on every hit.
const BLEED_CHANCE: f32 = 0.05;
/// How long a reload holds fire (the reload montages run about this long; a guess).
const RELOAD_SECS: f32 = 2.5;
/// Weapon Noise of the human weapons (cm 4500 on the rifles): how far zombies hear a shot.
const SHOT_NOISE: f32 = 45.0;
/// The muzzle's height on a standing AI and how far ahead (ours: the hand's place in the aim
/// pose, not the weapon's RangeStartPoint).
const MUZZLE: Vec3 = Vec3::new(-0.1, 1.45, 0.55);
/// The survivor's capsule: feet to top, and radius (the player's KinematicBody).
const PLAYER_CENTRE: f32 = 0.82;

/// E_HitboxTypeAI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hitbox {
    Head = 0,
    Chest = 1,
    Legs = 3,
    Feet = 5,
}

/// Is Hitbox? by height on the survivor (head and neck bones from 1.45 m, spine and clavicles
/// from 0.9 m, pelvis, thighs and calves from 0.15 m, the feet below).
pub fn hitbox(height: f32) -> Hitbox {
    match height {
        h if h >= 1.45 => Hitbox::Head,
        h if h >= 0.9 => Hitbox::Chest,
        h if h >= 0.15 => Hitbox::Legs,
        _ => Hitbox::Feet,
    }
}

/// RandomUnitVectorInConeInDegrees.
pub fn in_cone(dir: Vec3, half_deg: f32, rng: &mut impl Rng) -> Vec3 {
    let half = half_deg.to_radians();
    // Uniform over the cap: cos(theta) uniform in [cos(half), 1].
    let cos_t = rng.gen_range(half.cos()..=1.0);
    let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
    let phi = rng.gen_range(0.0..std::f32::consts::TAU);
    let local = Vec3::new(sin_t * phi.cos(), sin_t * phi.sin(), cos_t);
    Quat::from_rotation_arc(Vec3::Z, dir.normalize_or(Vec3::Z)) * local
}

/// Where a ray from `origin` along `dir` (unit) first meets an upright capsule (feet at `feet`):
/// the distance, if within `max`.
pub fn ray_capsule(origin: Vec3, dir: Vec3, feet: Vec3, radius: f32, height: f32, max: f32) -> Option<f32> {
    let (a, b) = (feet + Vec3::Y * radius, feet + Vec3::Y * (height - radius).max(radius));
    // Sample along the ray: the capsule is small and the ray short, so a fine march is enough.
    let steps = ((max / 0.05).ceil() as usize).max(1);
    (0..=steps).map(|i| max * i as f32 / steps as f32).find(|&s| {
        let p = origin + dir * s;
        let axis = Vec3::new(a.x, p.y.clamp(a.y, b.y), a.z);
        p.distance(axis) <= radius + TRACE_RADIUS
    })
}

/// A human's weapons (SetWeapons) and its firing state.
#[derive(Component, Debug)]
pub struct HumanArms {
    pub ranged: Option<&'static str>,
    pub melee: Option<&'static str>,
    pub ammo: u32,
    pub cooldown: f32,
    pub reloading: f32,
    /// The weapon model is on the hand.
    held: bool,
}

impl HumanArms {
    pub fn ranged(&self) -> Option<&'static AiWeapon> {
        self.ranged.and_then(|k| ai_weapons().get(k))
    }

    pub fn melee(&self) -> Option<&'static AiWeapon> {
        self.melee.and_then(|k| ai_weapons().get(k))
    }

    /// The stance its drawn weapon animates with.
    pub fn stance(&self) -> Stance {
        match (self.ranged(), self.melee) {
            (Some(w), _) if w.socket.as_deref() == Some("Sidearm_r") || w.magazine <= 12 && w.range < 70.0 && w.interval[0] < 1.0 => Stance::Pistol,
            (Some(_), _) => Stance::Rifle,
            (None, Some(_)) => Stance::Melee,
            _ => Stance::Unarmed,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (arm, hair_on_head, backpack_on_socket, hold_weapon, fight).chain().run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

/// SetWeapons: a random ranged weapon (if the class has any) and melee weapon.
#[allow(clippy::type_complexity)]
fn arm(mut commands: Commands, assets: Res<AssetServer>, added: Query<(Entity, &LevelZombie), (Added<LevelZombie>, Without<HumanArms>)>) {
    let mut rng = rand::thread_rng();
    for (e, z) in &added {
        let Some(h) = z.kind.human() else { continue };
        let pick = |list: &'static [String], rng: &mut rand::rngs::ThreadRng| (!list.is_empty()).then(|| list[rng.gen_range(0..list.len())].as_str());
        let ranged = pick(&h.ranged, &mut rng);
        let melee = pick(&h.melee, &mut rng);
        let arms = HumanArms { ranged, melee, ammo: 0, cooldown: rng.gen_range(0.5..1.5), reloading: 0.0, held: false };
        let ammo = arms.ranged().map_or(0, |w| w.magazine);
        let stance = arms.stance();
        commands.entity(e).insert((HumanArms { ammo, ..arms }, stance));
        // SetMeshAndMaterial: a random hair and beard, waiting for the body's head bone.
        for list in [&h.hair, &h.beard] {
            if let Some(m) = pick(list, &mut rng) {
                commands.spawn((model(&assets, format!("game/SurrounDead/Content/{m}.glb")), Visibility::Hidden, OnHead(e), ChildOf(e)));
            }
        }
        // BackpackMesh on the Backpack socket (the wandering trader's pack).
        if let Some(b) = &h.backpack {
            let local = Transform::from_translation(Vec3::from(b.t)).with_rotation(Quat::from_array(b.r).normalize());
            commands.spawn((model(&assets, format!("game/SurrounDead/Content/{}.glb", b.mesh)), Visibility::Hidden, OnSocket(e, "Backpack", local), ChildOf(e)));
        }
        info!("human {} armed: {:?} / {:?}", z.kind.class_name(), ranged, melee);
    }
}

/// Hair or beard waiting for its human's `head` bone.
#[derive(Component)]
struct OnHead(Entity);

/// HairMesh / BeardMesh hang off the `head` bone at their components' relative rotation
/// (P90 R-90, as on the traders).
fn hair_on_head(mut commands: Commands, pending: Query<(Entity, &OnHead)>, children: Query<&Children>, names: Query<&Name>) {
    for (e, OnHead(body)) in &pending {
        let Some(bone) = children.iter_descendants(*body).find(|b| names.get(*b).is_ok_and(|n| n.as_str().eq_ignore_ascii_case("head"))) else { continue };
        commands.entity(e).remove::<OnHead>().insert((ue_relative([0.0; 3], 90.0, 0.0, -90.0), Visibility::Inherited));
        commands.entity(bone).add_child(e);
    }
}

/// A mesh waiting for a socket on its human's skeleton, with its transform relative to the socket.
#[derive(Component)]
struct OnSocket(Entity, &'static str, Transform);

/// The survivor skeleton's sockets (the Polygon characters share it) carry the attachments.
fn backpack_on_socket(mut commands: Commands, db: Res<super::ItemDb>, pending: Query<(Entity, &OnSocket)>, children: Query<&Children>, names: Query<&Name>) {
    for (e, OnSocket(body, socket, local)) in &pending {
        let (bone, at) = db.sockets.get(*socket).map_or((*socket, Transform::IDENTITY), |s| (s.bone.as_str(), s.transform.to_transform()));
        let Some(b) = children.iter_descendants(*body).find(|c| names.get(*c).is_ok_and(|n| n.as_str() == bone)) else { continue };
        commands.entity(e).remove::<OnSocket>().insert((at * *local, Visibility::Inherited));
        commands.entity(b).add_child(e);
    }
}

/// The drawn weapon's model in its socket on the body (once the skeleton is in).
fn hold_weapon(
    mut commands: Commands,
    db: Res<super::ItemDb>,
    assets: Res<AssetServer>,
    mut humans: Query<(Entity, &mut HumanArms), Without<Dead>>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (e, mut arms) in &mut humans {
        if arms.held {
            continue;
        }
        let Some(w) = arms.ranged().or(arms.melee()) else {
            arms.held = true;
            continue;
        };
        let (Some(mesh), Some(socket)) = (&w.mesh, w.socket.as_deref()) else {
            arms.held = true;
            continue;
        };
        let (bone, local) = match db.sockets.get(socket) {
            Some(s) => (s.bone.as_str(), s.transform.to_transform()),
            None => (socket, Transform::IDENTITY),
        };
        let found = children.iter_descendants(e).find(|&c| names.get(c).is_ok_and(|n| n.as_str() == bone || n.as_str().strip_prefix(bone).is_some_and(|r| r.starts_with('_') && r[1..].chars().all(|c| c.is_ascii_digit()))));
        let Some(bone) = found else { continue };
        let w = commands.spawn((model(&assets, format!("game/SurrounDead/Content/{mesh}.glb")), local)).id();
        commands.entity(bone).add_child(w);
        arms.held = true;
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fight(
    time: Res<Time>,
    (store, db): (Res<SettingsStore>, Res<super::ItemDb>),
    mut game: ResMut<CurrentGame>,
    vitals: Res<super::PlayerVitals>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut props: crate::level::collision::PropRays,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    mut humans: Query<(&Transform, &LevelZombie, &Zombie, &mut HumanArms, &mut UpperAction), Without<Dead>>,
    (mut sound, mut fx, mut noise): (MessageWriter<crate::audio::Sound>, MessageWriter<super::particles::Fx>, MessageWriter<super::zombies::Noise>),
) {
    let Ok((me, body)) = player.single() else { return };
    if vitals.is_dead() || game.state.is_none() {
        return;
    }
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let target = me.translation + Vec3::Y * PLAYER_CENTRE;
    for (t, z, brain, mut arms, mut action) in &mut humans {
        let Some(h) = z.kind.human() else { continue };
        arms.cooldown -= dt;
        if arms.reloading > 0.0 {
            arms.reloading -= dt;
            if arms.reloading <= 0.0 {
                arms.ammo = arms.ranged().map_or(0, |w| w.magazine);
            }
            continue;
        }
        if brain.brain != Brain::Chase || arms.cooldown > 0.0 {
            continue;
        }
        let flat = (me.translation - t.translation) * Vec3::new(1.0, 0.0, 1.0);
        let facing = (t.rotation * Vec3::Z).angle_between(flat).to_degrees() <= FACING;
        if let Some(w) = arms.ranged() {
            if !(brain.in_sight && flat.length() <= w.range && facing) {
                continue;
            }
            arms.cooldown = rng.gen_range(w.interval[0]..=w.interval[1].max(w.interval[0]));
            let muzzle = t.translation + t.rotation * MUZZLE;
            let aim = target - muzzle;
            let dir = in_cone(aim, h.accuracy * w.spread, &mut rng);
            let reach = aim.length() + OVERSHOOT;
            // Terrain and props stop the round short.
            let wall = props.cast(muzzle, dir, reach).unwrap_or(reach).min(crate::gameplay::combat::terrain_hit(terrain.as_deref(), muzzle, dir, reach).unwrap_or(reach));
            if let Some(cue) = &w.sound {
                let mut s = crate::audio::Sound::at(cue.clone(), muzzle);
                s.volume = 0.5;
                sound.write(s);
            }
            if let Some(flash) = &w.muzzle_flash {
                fx.write(super::particles::Fx::at(flash.clone(), muzzle).pointing(dir).scaled(Vec3::splat(0.75)));
            }
            noise.write(super::zombies::Noise { at: muzzle, range: SHOT_NOISE });
            action.start(ActionKind::Fire);
            if let Some(d) = ray_capsule(muzzle, dir, me.translation, body.radius, body.height, wall) {
                let hb = hitbox((muzzle + dir * d).y - me.translation.y);
                hit(&mut game, &db, &store, w.damage[hb as usize], BLEED_CHANCE, &mut rng);
                info!("{} shot the survivor: {hb:?} for {:.0} from {:.1} m", z.kind.class_name(), w.damage[hb as usize], flat.length());
            }
            arms.ammo = arms.ammo.saturating_sub(1);
            if arms.ammo == 0 {
                arms.reloading = RELOAD_SECS;
                action.start(ActionKind::Reload);
            }
        } else if let Some(w) = arms.melee() {
            if flat.length() > h.melee_distance + 0.3 || !facing {
                continue;
            }
            arms.cooldown = rng.gen_range(w.melee_interval[0]..=w.melee_interval[1].max(w.melee_interval[0])).max(0.5);
            action.start(ActionKind::Swing(0));
            hit(&mut game, &db, &store, w.damage[Hitbox::Chest as usize], BLEED_CHANCE, &mut rng);
            info!("{} hit the survivor for {:.0}", z.kind.class_name(), w.damage[Hitbox::Chest as usize]);
        }
    }
}

/// ApplyDamage to the survivor (AnyDamage: armor, Toughness) and ApplyMedicalEffect's bleed.
pub(crate) fn hit(game: &mut CurrentGame, db: &super::ItemDb, store: &SettingsStore, damage: f32, bleed_chance: f32, rng: &mut impl Rng) {
    let taken = super::survival::take_hit(game, db, store, damage);
    let Some(state) = game.state.as_mut() else { return };
    state.survival.health = (state.survival.health - taken).max(0.0);
    if rng.gen_range(0.0..1.0) < bleed_chance {
        state.effects.add_bleed(rng);
        state.survival.bleeding = state.effects.bleeding();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::zombies::ZombieKind;

    #[test]
    fn hitboxes_follow_the_body() {
        assert_eq!(hitbox(1.55), Hitbox::Head);
        assert_eq!(hitbox(1.2), Hitbox::Chest);
        assert_eq!(hitbox(0.5), Hitbox::Legs);
        assert_eq!(hitbox(0.05), Hitbox::Feet);
    }

    #[test]
    fn shots_scatter_within_the_cone() {
        let mut rng = rand::thread_rng();
        for _ in 0..200 {
            let d = in_cone(Vec3::X, 5.0, &mut rng);
            assert!((d.length() - 1.0).abs() < 1e-4 && d.angle_between(Vec3::X).to_degrees() <= 5.0 + 1e-3);
        }
    }

    #[test]
    fn rays_meet_the_capsule() {
        let feet = Vec3::new(10.0, 0.0, 0.0);
        let d = ray_capsule(Vec3::new(0.0, 1.0, 0.0), Vec3::X, feet, 0.35, 1.64, 20.0).unwrap();
        assert!((d - 9.6).abs() < 0.1, "{d}");
        assert!(ray_capsule(Vec3::new(0.0, 1.0, 1.0), Vec3::X, feet, 0.35, 1.64, 20.0).is_none(), "1 m to the side");
        assert!(ray_capsule(Vec3::new(0.0, 1.0, 0.0), Vec3::X, feet, 0.35, 1.64, 5.0).is_none(), "stops short");
    }

    #[test]
    fn humans_carry_their_blueprint_values() {
        let Some(k) = ZombieKind::from_class("BP_Bandit_Ranged_AssaultRifle_C") else { return };
        let h = k.human().unwrap();
        assert_eq!((h.faction.as_str(), h.killed.as_str(), h.accuracy, h.sight), ("Bandit", "Bandit", 2.5, 50.0));
        assert_eq!(h.ranged, ["BP_AIWeapon_M4_C"]);
        let m4 = &ai_weapons()["BP_AIWeapon_M4_C"];
        assert_eq!((m4.range, m4.spread, m4.magazine, m4.damage[0], m4.damage[1]), (65.0, 2.0, 30, 60.0, 20.0));
        assert_eq!(k.class().loot_table.as_deref(), Some("Bandit"));
        let arms = HumanArms { ranged: Some("BP_AIWeapon_M4_C"), melee: None, ammo: 30, cooldown: 0.0, reloading: 0.0, held: false };
        assert_eq!(arms.stance(), Stance::Rifle);
        let pistol = HumanArms { ranged: Some("BP_AIWeapon_M9_C"), ..arms };
        assert_eq!(pistol.stance(), Stance::Pistol);
        assert_eq!(ZombieKind::from_class("BP_Scavenger_Male_C").unwrap().human().unwrap().faction, "Scavenger");
        assert_eq!(k.anim_set(), crate::models::AnimSet::Survivor);
    }
}
