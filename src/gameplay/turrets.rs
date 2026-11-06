//! Turrets: BP_BanditTurret_AI (4 at a bandit base) and BP_SurvivorTurret_AI (the settlement's),
//! both SmartAI's BP_MasterTurret with a BP_TurretAIComponent; and the placed ones,
//! Buildable_Turret / Buildable_SmallTurret (Buildable_TurretMASTER with a
//! BP_BuildableTurretAIComponent), decompiled.
//!
//! - **Idle**: TurretRotateAnimation (16 s, looping) sweeps the gun from FOV Left (90°) to
//!   -FOV Right and back (RLerp over its Rotate Alpha curve).
//! - **Perception Check** every 0.25 s: actors within Detection Range (20 m) tagged AI and with
//!   one of the Reaction Tags (bandit turret: Player, Zombie, Scavenger, EnemyAnimal; survivor
//!   turret: Bandit, Zombie, EnemyAnimal), not dead, inside the FOV (±90° yaw and pitch of the
//!   turret's facing) and in clear view of the muzzle. The first becomes the Attack Target and
//!   the sweep stops (Turret Idle Stop); none and it sweeps again.
//! - **Tracking**: the gun turns to the target at Turret Rotation Speed 15 (RInterpTo, up/down too).
//! - **Turret Fire** every AttackSpeed (0.5 s): a 1 cm sphere trace from the muzzle along the
//!   gun's facing within Spread (3°), 40 m (Weapon Range): the Damage map by hitbox (head 40,
//!   chest 15...), ApplyMedicalEffect(Bleed, 5 %); FireSound M16_Single_Shot_2_Cue and
//!   FX_GunshotRifle. A magazine of 5, then a 3 s reload.
//! - **Damage** (BP_MasterTurret.Damage_Object -> Server Turret Damaged): the survivor's shots and
//!   swings take its 100 health; at 0 it's dead (Turret Destroyed Effect: FX_Explosion_New at 0.2).
//! - **Disable** (the bandit turret's "Turret: Disable"): only from behind it (GetBehindObject?:
//!   the facing points away from the survivor), else "Unable to disable turret"; held for Duration
//!   (5 s, "Disable Turret", starting with Loot_MidShelf_GrabItem): dead, the gun gone, a Turret
//!   Gun (BP_TurretGunPickup) 1.75 m up.
//!
//! The placed turrets (built from DA_Turret / DA_SmallTurret, `building.rs`) are the same brain
//! with BP_BuildableTurretAIComponent's changes:
//! - Reaction Tags Zombie, Bandit, Military, EnemyAnimal (never the survivor); Detection Range
//!   35 m, Weapon Range 30 m, AttackSpeed 0.4 s (the small one 0.3 s), Spread 3°, a magazine of
//!   5 and Buildable_TurretMASTER's Reload Speed 0.1 s; Damage head 85, chest 24, arms 20, legs
//!   18, hands and feet 16, other 18; no bleeding. Health 100 (the small one 75), saved
//!   (ComponentPreSave / ComponentLoaded).
//! - Buildable_Turret has Requires Power?: unpowered, Perception Check does nothing and the
//!   sweep stands still; its interact widget's 'Power' stat says On / Off. The small one is
//!   always on (PoweredOn? true).
//! - Turret Fire: ReportNoiseEvent at the turret (75 m) before every shot; whatever the trace
//!   meets gets ApplyDamage and a white SpawnFloatingDamageText.
//! - The sweep is the Timeline's RLerp(yaw 90, yaw -90) with RInterpTo at 3.
//! - ReceiveAnyDamage: any damage (the survivor's too) goes to Server Turret Damaged. The
//!   zombies go for them (the 'ZombieCanAttack' tag: Zombie_FindPlayerLocation takes the nearest
//!   seen actor with it; Buildable_Turret only while powered, its Timeline drops the tag) and
//!   their swings land on them (`zombies.rs`, `survival.rs`). At 0: Turret Destroyed Effect
//!   (FX_Explosion_New at half size), 0.2 s, and the actor is gone: nothing comes back.
//! - BuildableObjectHealth (the Widget, shown while the survivor is in its 2.5 m Sphere 1.5 m
//!   up): a 200 x 10 bar (690000 on 50 % black) and "health/max" (Montserrat-Bold 6).
//! - Pickup gives the turret back whatever its health (Buildable_MASTER's Svr_Spawn): its own
//!   OnInteractBuildable (full health: drop the pickup; damaged: "Object damaged, returning half
//!   of crafting materials", and its CraftingMaterials are empty) is never called, as no option
//!   reaches MC_OnInteractBuildable.
//!
//! Ours: the muzzle sits 0.6 m along the gun from its pivot and the gun's hit sphere is 0.5 m
//! (the components' places aren't in the dump; the placed turrets' Weapon Muzzle is, without
//! the gun's 1.25 scale); turret kills give the survivor nothing; the placed turrets are hit
//! about their gun only. Not ported: the burn decal, Alert AI (alerting the AI around within
//! 60 m), the AI optimiser's layers (beyond 150 m a turret only sweeps), the animals and bosses
//! going for placed turrets.
use bevy::prelude::*;

use super::combat::{Dead, Health};
use crate::level::{KinematicBody, LevelZombie};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::CharacterModel;

/// BP_TurretAIComponent as the turrets set it.
const DETECTION_RANGE: f32 = 20.0;
const WEAPON_RANGE: f32 = 40.0;
const ROTATION_SPEED: f32 = 15.0;
const SPREAD: f32 = 3.0;
const ATTACK_SPEED: f32 = 0.5;
const MAGAZINE: u32 = 5;
const RELOAD_SECS: f32 = 3.0;
const MAX_HEALTH: f32 = 100.0;
/// Perception Check's timer.
const PERCEPTION: f32 = 0.25;
/// BP_MasterTurret's FOV Left / Right / Up / Down.
const FOV: f32 = 90.0;
/// Damage by E_HitboxTypeAI: Head, Chest, Arms, Legs, Hands, Feet, Other, None.
const DAMAGE: [f32; 8] = [40.0, 15.0, 10.0, 10.0, 10.0, 5.0, 10.0, 15.0];
/// E_HitboxTypeAI's None (Is Hitbox? on anything without bones).
const NO_HITBOX: usize = 7;
/// ApplyMedicalEffect(['Bleed'], 5.0).
const BLEED_CHANCE: f32 = 0.05;
/// The bandit turret's Duration (the disabling hold).
const DISABLE_SECS: f32 = 5.0;
/// How near the survivor must be to use a turret (ours).
const REACH: f32 = 2.2;
/// TurretRotateAnimation: 16 s, Rotate Alpha keys (s, alpha).
const SWEEP_SECS: f32 = 16.0;
const SWEEP: [(f32, f32); 6] = [(0.0, 0.0), (0.25, 0.0), (7.75, 1.0), (8.25, 1.0), (15.75, 0.0), (16.0, 0.0)];
/// Ours: muzzle along the gun, the gun's hit sphere.
const MUZZLE: f32 = 0.6;
const HIT_RADIUS: f32 = 0.5;
/// Turret Fire's ReportNoiseEvent (MaxRange 7500).
pub const NOISE: f32 = 75.0;
/// Buildable_Turret(_Small)'s Sphere: radius and height (the health bar's overlap), and the
/// Widget's height.
const BAR_SPHERE: f32 = 2.5;
const BAR_SPHERE_UP: f32 = 1.5;
const BAR_UP: f32 = 1.5;
/// BuildableObjectHealth's HealthBar.
const BAR_SIZE: Vec2 = Vec2::new(200.0, 10.0);
const BAR_FILL: Color = Color::linear_rgb(0.140625, 0.0, 0.0);
const BAR_BACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);
/// Turret Destroyed Effect's Delay before K2_DestroyActor.
const GONE_AFTER: f32 = 0.2;

/// A turret's AI component settings.
#[derive(Debug)]
pub struct Stats {
    pub detection: f32,
    pub range: f32,
    pub attack_speed: f32,
    pub magazine: u32,
    pub reload: f32,
    /// Damage by E_HitboxTypeAI.
    pub damage: [f32; 8],
    pub max_health: f32,
    pub bleed: f32,
    /// Weapon Muzzle on the gun: along it, up (m).
    pub muzzle: (f32, f32),
    /// The idle sweep's RInterpTo speed.
    pub sweep_speed: f32,
}

/// BP_TurretAIComponent as the level's turrets set it.
pub const LEVEL: Stats = Stats { detection: DETECTION_RANGE, range: WEAPON_RANGE, attack_speed: ATTACK_SPEED, magazine: MAGAZINE, reload: RELOAD_SECS, damage: DAMAGE, max_health: MAX_HEALTH, bleed: BLEED_CHANCE, muzzle: (MUZZLE, 0.0), sweep_speed: ROTATION_SPEED };
/// Buildable_Turret's BP_BuildableTurretAIComponent (Reload Speed from Buildable_TurretMASTER).
pub const PLACED: Stats = Stats {
    detection: 35.0,
    range: 30.0,
    attack_speed: 0.4,
    magazine: 5,
    reload: 0.1,
    damage: [85.0, 24.0, 20.0, 18.0, 16.0, 16.0, 18.0, 18.0],
    max_health: 100.0,
    bleed: 0.0,
    muzzle: (0.802885, 0.164176),
    sweep_speed: 3.0,
};
/// Buildable_SmallTurret's: quicker, frailer, a shorter gun.
pub const PLACED_SMALL: Stats = Stats { attack_speed: 0.3, max_health: 75.0, muzzle: (0.522987, 0.180473), ..PLACED };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurretKind {
    Bandit,
    Survivor,
    /// Buildable_Turret.
    Placed,
    /// Buildable_SmallTurret.
    PlacedSmall,
}

impl TurretKind {
    pub fn of(class: &str) -> Option<Self> {
        match class {
            "BP_BanditTurret_AI_C" => Some(Self::Bandit),
            "BP_SurvivorTurret_AI_C" => Some(Self::Survivor),
            "Buildable_Turret_C" => Some(Self::Placed),
            "Buildable_SmallTurret_C" => Some(Self::PlacedSmall),
            _ => None,
        }
    }

    /// The turret's Reaction Tags.
    pub fn reacts_to(self) -> &'static [&'static str] {
        match self {
            Self::Bandit => &["Player", "Zombie", "Scavenger", "EnemyAnimal"],
            Self::Survivor => &["Bandit", "Zombie", "EnemyAnimal"],
            Self::Placed | Self::PlacedSmall => &["Zombie", "Bandit", "Military", "EnemyAnimal"],
        }
    }

    pub fn placed(self) -> bool {
        matches!(self, Self::Placed | Self::PlacedSmall)
    }

    pub fn stats(self) -> &'static Stats {
        match self {
            Self::Bandit | Self::Survivor => &LEVEL,
            Self::Placed => &PLACED,
            Self::PlacedSmall => &PLACED_SMALL,
        }
    }

    /// Only the bandit turret can be disabled (the survivor turret's BP has no interaction).
    pub fn disarmable(self) -> bool {
        self == Self::Bandit
    }
}

/// An AI class's tags as the turrets read them (the actor Tags beside 'AI').
pub fn tags(kind: super::zombies::ZombieKind) -> Option<&'static str> {
    if let Some(h) = kind.human() {
        return match h.faction.as_str() {
            "Bandit" => Some("Bandit"),
            "Scavenger" => Some("Scavenger"),
            _ => Some("Military"),
        };
    }
    if let Some(a) = kind.animal() {
        // Wolves and bears are EnemyAnimal; the deer and rabbits aren't in any turret's list.
        return (!a.flees).then_some("EnemyAnimal");
    }
    Some("Zombie")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Player,
    Ai(Entity),
}

pub struct Turret {
    pub kind: TurretKind,
    /// The gun's pivot (its static placement's place) and the turret's facing.
    pub pivot: Vec3,
    pub facing: Quat,
    /// The gun's yaw and pitch relative to the facing (UE degrees).
    pub yaw: f32,
    pub pitch: f32,
    sweep: f32,
    pub target: Option<Target>,
    pub ammo: u32,
    reload: f32,
    fire_in: f32,
    perceive_in: f32,
    pub health: f32,
    pub dead: bool,
    pub disabled: bool,
    pub gun: Option<Entity>,
    glb: String,
    /// A placed turret's buildable (`building::Live::id`); its gun is the actor's Turret part.
    pub placed: Option<usize>,
    /// PoweredOn? (the level's and the small turret are always on).
    pub powered: bool,
    /// Seconds since Turret Destroyed Effect (a placed one goes after `GONE_AFTER`).
    pub dying: Option<f32>,
    /// A placed turret's actor location (its feet).
    pub origin: Vec3,
}

impl Turret {
    pub fn new(kind: TurretKind, pivot: Vec3, facing: Quat, glb: String) -> Self {
        let s = kind.stats();
        Self {
            kind,
            pivot,
            facing,
            yaw: FOV,
            pitch: 0.0,
            sweep: 0.0,
            target: None,
            ammo: s.magazine,
            reload: 0.0,
            fire_in: s.attack_speed,
            perceive_in: 0.0,
            health: s.max_health,
            dead: false,
            disabled: false,
            gun: None,
            glb,
            placed: None,
            powered: true,
            dying: None,
            origin: pivot,
        }
    }

    /// A placed turret: `facing` is its actor's rotation, `pivot` its Turret part's place.
    pub fn placed(kind: TurretKind, id: usize, pivot: Vec3, facing: Quat, health: Option<f32>) -> Self {
        let mut t = Self::new(kind, pivot, facing, String::new());
        t.placed = Some(id);
        if let Some(h) = health {
            t.health = h;
        }
        t
    }

    /// Gone for good: picked up, or destroyed and removed.
    pub fn gone(&self) -> bool {
        self.placed.is_some() && self.disabled
    }

    /// The gun's world rotation: UE yaw turns X toward Y (-yaw about +Y here), pitch lifts the nose.
    pub fn gun_rotation(&self) -> Quat {
        self.facing * Quat::from_rotation_y(-self.yaw.to_radians()) * Quat::from_rotation_z(self.pitch.to_radians())
    }

    pub fn forward(&self) -> Vec3 {
        self.gun_rotation() * Vec3::X
    }

    pub fn muzzle(&self) -> Vec3 {
        let (along, up) = self.kind.stats().muzzle;
        self.pivot + self.gun_rotation() * Vec3::new(along, up, 0.0)
    }

    /// The gun's rotation in its actor (a placed turret's Turret part).
    pub fn local_rotation(&self) -> Quat {
        Quat::from_rotation_y(-self.yaw.to_radians()) * Quat::from_rotation_z(self.pitch.to_radians())
    }

    /// What zombies go for (the 'ZombieCanAttack' tag): a placed turret, alive, small or powered.
    pub fn attackable(&self) -> bool {
        self.placed.is_some() && !self.dead && !self.disabled && self.powered
    }

    /// The (yaw, pitch) from the turret's facing to `at`, UE degrees.
    pub fn angles_to(&self, at: Vec3) -> (f32, f32) {
        let local = self.facing.inverse() * (at - self.pivot);
        let yaw = (-local.z).atan2(local.x).to_degrees();
        let pitch = local.y.atan2(local.xz().length()).to_degrees();
        (yaw, pitch)
    }

    /// Perception Check's FOV test (NormalizedDeltaRotator within ±FOV).
    pub fn in_fov(&self, at: Vec3) -> bool {
        let (y, p) = self.angles_to(at);
        (-FOV..=FOV).contains(&y) && (-FOV..=FOV).contains(&p)
    }

    /// TurretRotateAnimation: the sweep's yaw at `t` s (RLerp(FOV Left, -FOV Right, alpha)).
    pub fn sweep_yaw(t: f32) -> f32 {
        let t = t.rem_euclid(SWEEP_SECS);
        let i = SWEEP.iter().position(|k| k.0 > t).unwrap_or(SWEEP.len() - 1).max(1);
        let ((t0, a0), (t1, a1)) = (SWEEP[i - 1], SWEEP[i]);
        let s = ((t - t0) / (t1 - t0).max(1e-6)).clamp(0.0, 1.0);
        let alpha = a0 + (a1 - a0) * s * s * (3.0 - 2.0 * s);
        FOV + (-FOV - FOV) * alpha
    }

    /// RInterpTo toward (yaw, pitch) at `speed`: each component moves dt * speed of the way.
    pub fn turn_to(&mut self, (yaw, pitch): (f32, f32), dt: f32, speed: f32) {
        let k = (dt * speed).clamp(0.0, 1.0);
        let dy = (yaw - self.yaw + 540.0).rem_euclid(360.0) - 180.0;
        self.yaw += dy * k;
        self.pitch += (pitch - self.pitch) * k;
    }

    /// Deduct Health: true when this hit killed it.
    pub fn damage(&mut self, amount: f32) -> bool {
        if self.dead {
            return false;
        }
        self.health = (self.health - amount.trunc()).max(0.0);
        if self.health <= 0.0 {
            self.dead = true;
            return true;
        }
        false
    }
}

/// GetBehindObject?: the survivor is behind when the direction to them is against the facing.
pub fn behind(facing: Vec3, from: Vec3, to: Vec3) -> bool {
    (to - from).normalize_or_zero().dot(facing) < 0.0
}

/// The survivor's shot this frame, for things that take damage from it (BPI Damage_Object).
#[derive(Message, Clone, Copy, Debug)]
pub struct Shot {
    pub origin: Vec3,
    pub dir: Vec3,
    /// Where the round stopped (the first prop, the ground or a zombie).
    pub reach: f32,
    pub damage: f32,
}

#[derive(Resource, Default)]
pub struct Turrets {
    pub list: Vec<Turret>,
    collected: bool,
    /// The hold on a bandit turret: (turret, seconds held).
    pub holding: Option<(usize, f32)>,
}

impl Turrets {
    /// A turret actor (the level's, or a spawned prefab's) at `transform`: its gun is found among
    /// the static props (SM_Wep_Veh_MachineGun_01, 1.26 m up) and taken out of them.
    pub fn add(&mut self, level: &crate::level::Level, streaming: &mut crate::level::PropStreaming, commands: &mut Commands, class: &str, transform: &Transform) -> bool {
        let Some(kind) = TurretKind::of(class) else { return false };
        let want = transform.transform_point(Vec3::new(0.0, 1.2588, 0.0844));
        let Some((key, at, glb)) = level.prop_near(want, "SM_Wep_Veh_MachineGun_01", 1.5) else {
            warn!("turret {class} at {:.0}: no gun mesh found", transform.translation);
            return false;
        };
        if let Some(cell) = streaming.hide(key) {
            commands.entity(cell).despawn();
        }
        let facing = Quat::from_rotation_y(transform.rotation.to_euler(EulerRot::YXZ).0);
        self.list.push(Turret::new(kind, at.translation, facing, glb));
        true
    }
}

/// Kills the turrets made: no XP or kill counts for the survivor (`combat::kill_zombies`).
#[derive(Resource, Default)]
pub struct NoCredit(pub std::collections::HashSet<Entity>);

#[derive(Component)]
struct TurretGun(usize);

/// A placed turret's gun: its actor's Turret part (`building::spawn_actor`), by buildable id.
#[derive(Component)]
pub struct TurretPart(pub usize);

/// A zombie's swing landing on a placed turret (`survival::zombie_attacks`).
#[derive(Message, Clone, Copy, Debug)]
pub struct TurretHit {
    pub turret: usize,
    pub damage: f32,
}

/// A placed turret the zombies may go for (the 'ZombieCanAttack' tag).
#[derive(Clone, Copy, Debug)]
pub struct Attackable {
    /// Its place in [`Turrets::list`].
    pub turret: usize,
    /// The actor's place (its feet) and the gun's.
    pub at: Vec3,
    pub centre: Vec3,
    /// Its collision's key in `PropRays` (a sight line meeting it first sees it).
    pub collide: usize,
}

/// The attackable placed turrets this frame.
#[derive(Resource, Default)]
pub struct ZombieTargets(pub Vec<Attackable>);

impl ZombieTargets {
    pub fn get(&self, turret: usize) -> Option<&Attackable> {
        self.0.iter().find(|a| a.turret == turret)
    }
}

/// BuildableObjectHealth over turret `0`, and its fill and text.
#[derive(Component)]
struct HealthBar(usize);
#[derive(Component)]
struct HealthFill;
#[derive(Component)]
struct HealthText;

pub fn plugin(app: &mut App) {
    app.init_resource::<Turrets>()
        .init_resource::<NoCredit>()
        .init_resource::<ZombieTargets>()
        .add_message::<Shot>()
        .add_message::<TurretHit>()
        .add_systems(OnEnter(AppState::InGame), |mut t: ResMut<Turrets>, mut n: ResMut<NoCredit>, mut z: ResMut<ZombieTargets>| {
            *t = Turrets::default();
            n.0.clear();
            z.0.clear();
        })
        .add_systems(Update, (collect, placed_sync, spawn_guns, take_damage, think, pose_guns, health_bars).chain().after(super::building::BuildSet).run_if(in_state(AppState::InGame)))
        .add_systems(Update, disarm.after(crate::level::doors::interact_doors).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

fn collect(mut commands: Commands, level: Option<Res<crate::level::Level>>, streaming: Option<ResMut<crate::level::PropStreaming>>, mut turrets: ResMut<Turrets>) {
    let (Some(level), Some(mut streaming)) = (level, streaming) else { return };
    if turrets.collected {
        return;
    }
    turrets.collected = true;
    for a in &level.actors {
        if TurretKind::of(&a.class).is_some_and(|k| !k.placed()) {
            turrets.add(&level, &mut streaming, &mut commands, &a.class, &crate::level::affine(&a.transform));
        }
    }
    info!("turrets: {}", turrets.list.len());
}

/// The placed turrets follow their buildables: new ones join, moved ones move, picked-up ones
/// go; PoweredOn? from the generators; the health goes into the save as it changes, and a
/// destroyed one's actor goes `GONE_AFTER` after its effect.
#[allow(clippy::too_many_arguments)]
fn placed_sync(
    mut commands: Commands,
    time: Res<Time>,
    db: Res<super::building::BuildableDb>,
    mut game: ResMut<CurrentGame>,
    mut placed: ResMut<super::building::Buildables>,
    mut rays: crate::level::collision::PropRays,
    mut turrets: ResMut<Turrets>,
    mut targets: ResMut<ZombieTargets>,
) {
    let part = |actor: &str| db.actors.get(actor).and_then(|d| d.parts.iter().find(|p| p.name == "Turret")).map_or(Vec3::ZERO, |p| p.local().translation);
    for l in &placed.list {
        let Some(kind) = TurretKind::of(&l.actor).filter(|k| k.placed()) else { continue };
        if !turrets.list.iter().any(|t| t.placed == Some(l.id)) {
            turrets.list.push(Turret::placed(kind, l.id, l.transform.transform_point(part(&l.actor)), l.transform.rotation, l.health));
            info!("turret {} placed: {:?}, {:.0} health", turrets.list.len() - 1, kind, turrets.list.last().map_or(0.0, |t| t.health));
        }
    }
    let (mut save, mut gone) = (false, Vec::new());
    for t in turrets.list.iter_mut().filter(|t| !t.gone()) {
        let Some(id) = t.placed else { continue };
        let Some(l) = placed.list.iter_mut().find(|l| l.id == id) else {
            // Picked up.
            t.disabled = true;
            t.target = None;
            continue;
        };
        // Moved (ReplacingObject).
        t.origin = l.transform.translation;
        t.pivot = l.transform.transform_point(part(&l.actor));
        t.facing = l.transform.rotation;
        t.powered = !db.actors.get(&l.actor).is_some_and(|d| d.requires_power) || l.power.powered;
        let hurt = (t.health < t.kind.stats().max_health).then_some(t.health);
        if l.health != hurt {
            l.health = hurt;
            save = true;
        }
        if let Some(d) = t.dying.as_mut() {
            *d += time.delta_secs();
            if *d >= GONE_AFTER {
                t.disabled = true;
                gone.push(id);
            }
        }
    }
    for id in gone {
        super::building::remove_placed(&mut commands, &mut game, &mut placed, &mut rays, id);
    }
    if save {
        super::building::save_placed(&mut game, &placed);
    }
    targets.0 = turrets.list.iter().enumerate().filter(|(_, t)| t.attackable()).map(|(i, t)| Attackable { turret: i, at: t.origin, centre: t.pivot, collide: super::building::COLLIDE_KEY + t.placed.unwrap_or_default() }).collect();
}

fn spawn_guns(mut commands: Commands, assets: Res<AssetServer>, mut turrets: ResMut<Turrets>) {
    for (i, t) in turrets.list.iter_mut().enumerate().filter(|(_, t)| t.gun.is_none() && !t.disabled && t.placed.is_none()) {
        let e = commands.spawn((crate::models::model(&assets, t.glb.clone()), Transform::from_translation(t.pivot).with_rotation(t.gun_rotation()), TurretGun(i), DespawnOnExit(AppState::InGame))).id();
        t.gun = Some(e);
    }
}

/// Damage_Object from the survivor's rounds and swings; the placed ones' ReceiveAnyDamage also
/// from blasts and the zombies' swings.
fn take_damage(
    mut turrets: ResMut<Turrets>,
    (mut shots, mut swings, mut blasts, mut zombie_hits): (MessageReader<Shot>, MessageReader<super::harvest::Swing>, MessageReader<super::barrels::ObjectBlast>, MessageReader<TurretHit>),
    mut fx: MessageWriter<super::particles::Fx>,
) {
    let mut hits: Vec<(usize, f32)> = Vec::new();
    for s in shots.read() {
        for (i, t) in turrets.list.iter().enumerate().filter(|(_, t)| !t.dead && !t.gone()) {
            let along = (t.pivot - s.origin).dot(s.dir);
            if along > 0.0 && along <= s.reach + HIT_RADIUS && (s.origin + s.dir * along).distance(t.pivot) <= HIT_RADIUS {
                hits.push((i, s.damage));
            }
        }
    }
    for s in swings.read() {
        for (i, t) in turrets.list.iter().enumerate().filter(|(_, t)| !t.dead && !t.gone()) {
            let flat = (t.pivot - s.feet).with_y(0.0);
            if flat.length() <= 2.0 && flat.normalize_or_zero().dot(s.facing) > 0.5 {
                hits.push((i, s.damage));
            }
        }
    }
    for b in blasts.read() {
        for (i, t) in turrets.list.iter().enumerate().filter(|(_, t)| t.placed.is_some() && !t.dead && !t.gone()) {
            if t.pivot.distance(b.at) <= b.radius {
                hits.push((i, b.damage));
            }
        }
    }
    hits.extend(zombie_hits.read().map(|h| (h.turret, h.damage)));
    for (i, dmg) in hits {
        let Some(t) = turrets.list.get_mut(i) else { continue };
        if t.damage(dmg) {
            // Turret Destroyed Effect: the level's at a fifth of the size at the gun, the placed
            // ones' at half at the actor (then K2_DestroyActor).
            if t.placed.is_some() {
                fx.write(super::particles::Fx::at("FX_Explosion_New", t.origin).scaled(Vec3::splat(0.5)));
                t.dying = Some(0.0);
            } else {
                fx.write(super::particles::Fx::at("FX_Explosion_New", t.pivot).scaled(Vec3::splat(0.2)));
            }
            t.target = None;
            info!("turret {i} destroyed");
        } else if t.health > 0.0 && !t.dead {
            info!("turret {i} hit for {dmg:.0}: {:.0} left", t.health);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn think(
    time: Res<Time>,
    db: Res<super::ItemDb>,
    store: Res<crate::menus::settings::SettingsStore>,
    mut game: ResMut<CurrentGame>,
    vitals: Res<super::PlayerVitals>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut props: crate::level::collision::PropRays,
    mut turrets: ResMut<Turrets>,
    mut no_credit: ResMut<NoCredit>,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    mut ai: Query<(Entity, &Transform, &LevelZombie, Option<&mut Health>), Without<Dead>>,
    (mut sound, mut fx): (MessageWriter<crate::audio::Sound>, MessageWriter<super::particles::Fx>),
    (mut noise, mut numbers): (MessageWriter<super::zombies::Noise>, MessageWriter<super::damage_numbers::DamageNumber>),
) {
    let Ok((me, body)) = player.single() else { return };
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let player_centre = me.translation + Vec3::Y * body.height / 2.0;
    let player_alive = !vitals.is_dead() && game.state.is_some();
    for t in turrets.list.iter_mut().filter(|t| !t.dead && !t.disabled) {
        let s = t.kind.stats();
        // Unpowered: no Perception Check, and the Timeline's update leaves the gun be.
        if !t.powered {
            t.target = None;
            continue;
        }
        // Beyond everything it could see, it only sweeps (ours: no work for far turrets).
        let far = t.pivot.distance(me.translation) > 150.0;
        let centre_of = |e: Entity| ai.get(e).ok().map(|(_, tr, z, _)| tr.translation + Vec3::Y * z.kind.class().capsule_half_height);
        // Perception Check.
        t.perceive_in -= dt;
        if t.perceive_in <= 0.0 && !far {
            t.perceive_in += PERCEPTION;
            let reacts = t.kind.reacts_to();
            let mut seen: Vec<(Target, Vec3)> = Vec::new();
            if player_alive && reacts.contains(&"Player") {
                seen.push((Target::Player, player_centre));
            }
            for (e, tr, z, _) in &ai {
                if tags(z.kind).is_some_and(|tag| reacts.contains(&tag)) {
                    seen.push((Target::Ai(e), tr.translation + Vec3::Y * z.kind.class().capsule_half_height));
                }
            }
            seen.retain(|(_, at)| at.distance(t.pivot) <= s.detection && t.in_fov(*at));
            seen.sort_by(|a, b| a.1.distance(t.pivot).total_cmp(&b.1.distance(t.pivot)));
            let muzzle = t.muzzle();
            let clear = |at: Vec3, props: &mut crate::level::collision::PropRays| {
                let d = at - muzzle;
                let len = d.length();
                let dir = d / len.max(1e-4);
                props.cast(muzzle, dir, len - 0.3).is_none() && crate::gameplay::combat::terrain_hit(terrain.as_deref(), muzzle, dir, len).is_none()
            };
            let pick = seen.into_iter().find(|(_, at)| clear(*at, &mut props)).map(|s| s.0);
            if pick != t.target {
                debug!("turret target {pick:?}");
            }
            t.target = pick;
        }
        let aim = match t.target {
            Some(Target::Player) => Some(player_centre),
            Some(Target::Ai(e)) => centre_of(e),
            None => None,
        };
        if aim.is_none() {
            t.target = None;
        }
        match aim {
            // Track Target: RInterpTo to the look-at rotation.
            Some(at) => {
                let want = t.angles_to(at);
                t.turn_to(want, dt, ROTATION_SPEED);
            }
            // TurretRotateAnimation plays on from where it stopped.
            None => {
                t.sweep += dt;
                let want = (Turret::sweep_yaw(t.sweep), 0.0);
                t.turn_to(want, dt, s.sweep_speed);
            }
        }
        // Reload: Delay(Reload Speed), then a full magazine.
        if t.reload > 0.0 {
            t.reload -= dt;
            if t.reload <= 0.0 {
                t.ammo = s.magazine;
            }
            continue;
        }
        t.fire_in -= dt;
        if t.fire_in > 0.0 {
            continue;
        }
        t.fire_in += s.attack_speed;
        if aim.is_none() || far {
            continue;
        }
        if t.ammo == 0 {
            t.reload = s.reload;
            continue;
        }
        let muzzle = t.muzzle();
        let dir = super::humans::in_cone(t.forward(), SPREAD, &mut rng);
        let ground = crate::gameplay::combat::terrain_hit(terrain.as_deref(), muzzle, dir, s.range).unwrap_or(s.range);
        let wall = props.cast(muzzle, dir, ground).unwrap_or(ground);
        t.ammo -= 1;
        if t.ammo == 0 {
            t.reload = s.reload;
        }
        // The placed ones' ReportNoiseEvent (the survivor as instigator).
        if t.placed.is_some() {
            noise.write(super::zombies::Noise { at: t.origin, range: NOISE });
        }
        sound.write(crate::audio::Sound::at("M16_Single_Shot_2_Cue", muzzle));
        fx.write(super::particles::Fx::at("FX_GunshotRifle", muzzle).pointing(dir));
        // The first pawn the trace meets (the survivor's capsule or an AI's).
        let mut best: Option<(f32, Option<Entity>, f32)> = None;
        if player_alive
            && t.kind.reacts_to().contains(&"Player")
            && let Some(d) = super::humans::ray_capsule(muzzle, dir, me.translation, body.radius, body.height, wall)
        {
            best = Some((d, None, (muzzle + dir * d).y - me.translation.y));
        }
        for (e, tr, z, _) in &ai {
            let c = z.kind.class();
            if let Some(d) = super::humans::ray_capsule(muzzle, dir, tr.translation, c.capsule_radius, c.capsule_half_height * 2.0, wall)
                && best.is_none_or(|b| d < b.0)
            {
                best = Some((d, Some(e), (muzzle + dir * d).y - tr.translation.y));
            }
        }
        // A placed turret's SpawnFloatingDamageText for whatever the trace met (Is Hitbox? on
        // anything but a pawn is None).
        if t.placed.is_some() && (best.is_some() || wall < s.range) {
            let dmg = best.map_or(s.damage[NO_HITBOX], |b| s.damage[super::humans::hitbox(b.2) as usize]);
            numbers.write(super::damage_numbers::DamageNumber::blast(dmg, muzzle + dir * best.map_or(wall, |b| b.0)));
        }
        let Some((d, who, height)) = best else { continue };
        let dmg = s.damage[super::humans::hitbox(height) as usize];
        match who {
            None => {
                super::humans::hit(&mut game, &db, &store, dmg, s.bleed, &mut rng);
                info!("turret shot the survivor for {dmg:.0} from {:.1} m", d);
            }
            Some(e) => {
                if let Ok((_, _, _, Some(mut h))) = ai.get_mut(e) {
                    h.hp -= dmg;
                    if h.hp <= 0.0 {
                        no_credit.0.insert(e);
                    }
                }
            }
        }
    }
}

fn pose_guns(turrets: Res<Turrets>, mut guns: Query<(&TurretGun, &mut Transform)>, mut parts: Query<(&TurretPart, &mut Transform), Without<TurretGun>>) {
    for (g, mut tr) in &mut guns {
        if let Some(t) = turrets.list.get(g.0) {
            tr.rotation = t.gun_rotation();
        }
    }
    for (p, mut tr) in &mut parts {
        if let Some(t) = turrets.list.iter().find(|t| t.placed == Some(p.0)) {
            tr.rotation = t.local_rotation();
        }
    }
}

/// Whether the survivor (feet `p`, capsule radius / height) overlaps a placed turret's Sphere.
pub fn in_bar_sphere(origin: Vec3, p: Vec3, radius: f32, height: f32) -> bool {
    let c = origin + Vec3::Y * BAR_SPHERE_UP;
    // The capsule's axis point nearest the sphere's centre.
    let y = c.y.clamp(p.y + radius, p.y + (height - radius).max(radius));
    c.distance(p.with_y(y)) <= BAR_SPHERE + radius
}

/// BuildableObjectHealth: while the survivor is in a placed turret's Sphere, its bar and
/// "health/max" over the turret (a screen-space WidgetComponent drawn at its desired size).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn health_bars(
    mut commands: Commands,
    font: Res<crate::menus::widgets::MenuFont>,
    turrets: Res<Turrets>,
    camera: Query<(&Camera, &GlobalTransform), With<super::PlayerCamera>>,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    mut bars: Query<(Entity, &HealthBar, &mut Node, &mut Visibility)>,
    mut fills: Query<(&ChildOf, &mut Node), (With<HealthFill>, Without<HealthBar>)>,
    mut texts: Query<(&ChildOf, &mut Text), With<HealthText>>,
    parents: Query<&ChildOf, (Without<HealthFill>, Without<HealthText>)>,
) {
    let (Ok((cam, cam_t)), Ok((me, body))) = (camera.single(), player.single()) else { return };
    let shown: Vec<usize> = turrets.list.iter().enumerate().filter(|(_, t)| t.placed.is_some() && !t.gone() && in_bar_sphere(t.origin, me.translation, body.radius, body.height)).map(|(i, _)| i).collect();
    let mut have = Vec::new();
    for (e, bar, mut node, mut vis) in &mut bars {
        let at = turrets.list.get(bar.0).map(|t| t.origin + Vec3::Y * BAR_UP);
        match at.filter(|_| shown.contains(&bar.0)).map(|at| cam.world_to_viewport(cam_t, at)) {
            Some(Ok(px)) => {
                node.left = Val::Px(px.x);
                node.top = Val::Px(px.y);
                vis.set_if_neq(Visibility::Inherited);
                have.push(bar.0);
            }
            Some(Err(_)) => {
                vis.set_if_neq(Visibility::Hidden);
                have.push(bar.0);
            }
            None => commands.entity(e).despawn(),
        }
    }
    // UpdateHealth: PB_SetPercent(health / max) and "{clamped health}/{max}".
    // fill / text -> the Overlay -> the bar.
    let of = |overlay: Entity| parents.get(overlay).ok().and_then(|p| bars.get(p.parent()).ok()).map(|b| b.1.0);
    for (parent, mut node) in &mut fills {
        if let Some(t) = of(parent.parent()).and_then(|i| turrets.list.get(i)) {
            node.width = Val::Percent(t.health / t.kind.stats().max_health * 100.0);
        }
    }
    for (parent, mut text) in &mut texts {
        if let Some(t) = of(parent.parent()).and_then(|i| turrets.list.get(i)) {
            let s = bar_text(t.health, t.kind.stats().max_health);
            if text.0 != s {
                text.0 = s;
            }
        }
    }
    for i in shown.into_iter().filter(|i| !have.contains(i)) {
        let t = &turrets.list[i];
        let Ok(px) = cam.world_to_viewport(cam_t, t.origin + Vec3::Y * BAR_UP) else { continue };
        let max = t.kind.stats().max_health;
        info!("turret {i}: health bar {}", bar_text(t.health, max));
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(px.x), top: Val::Px(px.y), ..default() },
            UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
            HealthBar(i),
            Visibility::Inherited,
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
            children![(
                // The Overlay: the bar under its centred text.
                Node { width: Val::Px(BAR_SIZE.x), height: Val::Px(BAR_SIZE.y), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                BackgroundColor(BAR_BACK),
                children![
                    (Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(t.health / max * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(BAR_FILL), HealthFill),
                    (crate::menus::widgets::label(&font, bar_text(t.health, max), 6.0), HealthText),
                ],
            )],
        ));
    }
}

/// BuildableObjectHealth's text: Clamp(health, 0, max) "/" max, as whole numbers.
pub fn bar_text(health: f32, max: f32) -> String {
    format!("{}/{}", health.clamp(0.0, max) as i32, max as i32)
}

/// The bandit turret's "Turret: Disable", held from behind for Duration.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn disarm(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    db: Res<super::ItemDb>,
    overlay: Res<super::GameOverlay>,
    mut turrets: ResMut<Turrets>,
    mut ground: ResMut<super::inventory_ui::Ground>,
    mut notify: MessageWriter<super::notify::Notify>,
    mut montage: MessageWriter<super::interact::PlayerMontage>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
    mut prompt: Query<&mut Visibility, With<super::interact::Prompt>>,
    mut text: Query<&mut Text, With<super::interact::PromptText>>,
    mut bar: Query<&mut Node, With<super::interact::PromptBar>>,
    mut highlight: ResMut<super::outline::Highlight>,
) {
    let (Ok(p), Ok(mut vis)) = (player.single(), prompt.single_mut()) else { return };
    let p = p.translation;
    let near = turrets.list.iter().position(|t| t.kind.disarmable() && !t.dead && !t.disabled && t.pivot.with_y(p.y).distance(p) <= REACH && (t.pivot.y - p.y).abs() < 3.0);
    // OnStopExecuteInteract: letting go (or stepping away) stops the timer.
    if let Some((i, held)) = turrets.holding
        && (near != Some(i) || !keys.pressed(KeyCode::KeyF))
    {
        info!("turret {i}: let go after {held:.1} s (near {near:?})");
        turrets.holding = None;
    }
    if overlay.is_open() || *vis != Visibility::Hidden {
        return;
    }
    let Some(i) = near else { return };
    *vis = Visibility::Inherited;
    // Its StaticMesh and Turret Base, about the actor's origin: the gun's pivot less its
    // Turret component's (0, 8.4, 125.9) cm.
    let t = &turrets.list[i];
    let origin = t.pivot - t.facing * Vec3::new(0.0, 1.2588, 0.0844);
    highlight.claim(super::outline::Highlit::Actor { class: "BP_BanditTurret_AI_C".into(), at: origin });
    if keys.just_pressed(KeyCode::KeyF) {
        let t = &turrets.list[i];
        let behind = behind(t.facing * Vec3::X, t.pivot, p);
        info!("turret {i}: F (behind {behind})");
        if behind {
            turrets.holding = Some((i, 0.0));
            // The "Disable Turret" timer starts, then PlayMontage(Loot_MidShelf_GrabItem_Montage).
            montage.write(super::interact::PlayerMontage::new(crate::models::ActionKind::Grab));
        } else {
            notify.write(super::notify::Notify::new("Unable to disable turret"));
        }
    }
    let mut progress = 0.0;
    if let Some((k, held)) = turrets.holding.as_mut() {
        *held += time.delta_secs();
        progress = (*held / DISABLE_SECS).min(1.0);
        if *held >= DISABLE_SECS {
            let k = *k;
            turrets.holding = None;
            let t = &mut turrets.list[k];
            t.disabled = true;
            t.target = None;
            if let Some(g) = t.gun.take() {
                commands.entity(g).despawn();
            }
            // BP_TurretGunPickup 175 cm over the turret.
            ground.drop_at(t.pivot - Vec3::Y * 1.26 + Vec3::Y * 1.75, db.stack("DA_TurretGun", 1));
            info!("turret {k} disabled");
            progress = 0.0;
        }
    }
    if let Ok(mut tx) = text.single_mut() {
        tx.0 = if turrets.holding.is_some() { "Disable Turret".into() } else { "Turret: [F] Disable".into() };
    }
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(progress * 100.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turret() -> Turret {
        Turret::new(TurretKind::Bandit, Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, String::new())
    }

    #[test]
    fn the_sweep_goes_left_to_right_and_back() {
        assert!((Turret::sweep_yaw(0.0) - 90.0).abs() < 1e-3);
        assert!((Turret::sweep_yaw(8.0) + 90.0).abs() < 1e-3);
        assert!(Turret::sweep_yaw(4.0).abs() < 5.0, "{}", Turret::sweep_yaw(4.0));
        assert!((Turret::sweep_yaw(16.0) - 90.0).abs() < 1e-3);
    }

    #[test]
    fn turrets_see_their_front_half_and_turn_to_it() {
        let mut t = turret();
        // UE +X is the facing (Bevy +X here); UE +Y (Bevy +Z) is yaw +90.
        assert!(t.in_fov(Vec3::new(10.0, 1.0, 0.0)));
        assert!(!t.in_fov(Vec3::new(-10.0, 1.0, 0.0)));
        let (y, p) = t.angles_to(Vec3::new(0.0, 1.0, 10.0));
        assert!((y + 90.0).abs() < 1e-3 && p.abs() < 1e-3, "{y} {p}");
        // RInterpTo at 15: a 1/15 s step covers the whole way.
        t.yaw = 0.0;
        t.turn_to((30.0, 10.0), 1.0 / 15.0, ROTATION_SPEED);
        assert!((t.yaw - 30.0).abs() < 1e-3 && (t.pitch - 10.0).abs() < 1e-3);
        assert!(t.forward().dot(Vec3::new(10.0, 0.0, 0.0).normalize()) > 0.8);
    }

    #[test]
    fn a_hundred_health_and_from_behind_only() {
        let mut t = turret();
        assert!(!t.damage(60.0));
        assert!(t.damage(45.5) && t.dead && t.health == 0.0);
        assert!(!t.damage(10.0), "already dead");
        assert!(behind(Vec3::X, Vec3::ZERO, Vec3::new(-1.0, 0.0, 0.3)));
        assert!(!behind(Vec3::X, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)));
        assert_eq!(TurretKind::of("BP_BanditTurret_AI_C"), Some(TurretKind::Bandit));
        assert!(TurretKind::Bandit.reacts_to().contains(&"Player") && !TurretKind::Survivor.reacts_to().contains(&"Player"));
        // BP_TurretAIComponent's Damage map: hands 10, feet 5.
        assert_eq!((LEVEL.damage[4], LEVEL.damage[5]), (10.0, 5.0));
        // The muzzle is 0.6 m along the gun.
        assert!(t.muzzle().distance(t.pivot + t.forward() * 0.6) < 1e-5);
    }

    #[test]
    fn placed_turrets_carry_the_buildable_component() {
        assert_eq!(TurretKind::of("Buildable_Turret_C"), Some(TurretKind::Placed));
        assert_eq!(TurretKind::of("Buildable_SmallTurret_C"), Some(TurretKind::PlacedSmall));
        for k in [TurretKind::Placed, TurretKind::PlacedSmall] {
            assert!(k.placed() && !k.disarmable());
            assert!(!k.reacts_to().contains(&"Player") && k.reacts_to().contains(&"Military"));
        }
        let (big, small) = (TurretKind::Placed.stats(), TurretKind::PlacedSmall.stats());
        assert_eq!((big.detection, big.range, big.attack_speed, big.reload, big.max_health), (35.0, 30.0, 0.4, 0.1, 100.0));
        assert_eq!((small.attack_speed, small.max_health, small.damage[0]), (0.3, 75.0, 85.0));
        assert_eq!((big.damage[NO_HITBOX], big.bleed), (18.0, 0.0));
        // A saved health comes back; FTrunc'd hits; the small one dies at 75.
        let mut t = Turret::placed(TurretKind::PlacedSmall, 3, Vec3::Y, Quat::IDENTITY, Some(40.0));
        assert_eq!((t.placed, t.health, t.ammo), (Some(3), 40.0, 5));
        assert!(t.attackable());
        t.powered = false;
        assert!(!t.attackable(), "unpowered: no 'ZombieCanAttack'");
        t.powered = true;
        assert!(!t.damage(39.9) && t.health == 1.0);
        assert!(t.damage(1.0) && !t.attackable() && !t.gone());
        t.disabled = true;
        assert!(t.gone());
        assert!(!Turret::new(TurretKind::Survivor, Vec3::ZERO, Quat::IDENTITY, String::new()).attackable());
    }

    #[test]
    fn the_muzzle_turns_with_the_placed_gun() {
        // Facing UE +X (Bevy +X), the Weapon Muzzle 0.80 m along and 0.16 m up.
        let mut t = Turret::placed(TurretKind::Placed, 0, Vec3::new(0.0, 1.26, 0.0), Quat::IDENTITY, None);
        t.yaw = 0.0;
        assert!(t.muzzle().distance(Vec3::new(0.802885, 1.26 + 0.164176, 0.0)) < 1e-4);
        // Turned to UE yaw 90 (Bevy +Z): the part's local rotation does the same.
        t.yaw = 90.0;
        assert!((t.muzzle() - t.pivot).normalize().dot(Vec3::Z) > 0.95);
        assert!((t.local_rotation() * Vec3::X).distance(Vec3::Z) < 1e-5);
    }

    #[test]
    fn the_health_bar_shows_in_the_sphere() {
        assert_eq!(bar_text(57.9, 75.0), "57/75");
        assert_eq!(bar_text(-3.0, 100.0), "0/100");
        // The Sphere: 2.5 m around 1.5 m up; the survivor's capsule (0.35 m, 1.8 m) beside it.
        let at = Vec3::new(10.0, 2.0, 10.0);
        assert!(in_bar_sphere(at, at + Vec3::new(2.5, 0.0, 0.0), 0.35, 1.8));
        assert!(!in_bar_sphere(at, at + Vec3::new(3.0, 0.0, 0.0), 0.35, 1.8));
        assert!(!in_bar_sphere(at, at + Vec3::new(0.0, 5.0, 0.0), 0.35, 1.8), "on a roof above it");
    }
}
