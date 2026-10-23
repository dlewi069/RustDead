//! Turrets: BP_BanditTurret_AI (4 at a bandit base) and BP_SurvivorTurret_AI (the settlement's),
//! both SmartAI's BP_MasterTurret with a BP_TurretAIComponent (decompiled).
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
//!   (5 s, "Disable Turret"): dead, the gun gone, a Turret Gun (BP_TurretGunPickup) 1.75 m up.
//!
//! Ours: the muzzle sits 0.6 m along the gun from its pivot and the gun's hit sphere is 0.5 m
//! (the components' places aren't in the dump); turret kills give the survivor nothing.
//! Not ported: the burn decal, Alert AI (alerting the AI around within 60 m).

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
const DAMAGE: [f32; 8] = [40.0, 15.0, 10.0, 10.0, 5.0, 10.0, 10.0, 15.0];
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurretKind {
    Bandit,
    Survivor,
}

impl TurretKind {
    pub fn of(class: &str) -> Option<Self> {
        match class {
            "BP_BanditTurret_AI_C" => Some(Self::Bandit),
            "BP_SurvivorTurret_AI_C" => Some(Self::Survivor),
            _ => None,
        }
    }

    /// The turret's Reaction Tags.
    pub fn reacts_to(self) -> &'static [&'static str] {
        match self {
            Self::Bandit => &["Player", "Zombie", "Scavenger", "EnemyAnimal"],
            Self::Survivor => &["Bandit", "Zombie", "EnemyAnimal"],
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
}

impl Turret {
    pub fn new(kind: TurretKind, pivot: Vec3, facing: Quat, glb: String) -> Self {
        Self { kind, pivot, facing, yaw: FOV, pitch: 0.0, sweep: 0.0, target: None, ammo: MAGAZINE, reload: 0.0, fire_in: ATTACK_SPEED, perceive_in: 0.0, health: MAX_HEALTH, dead: false, disabled: false, gun: None, glb }
    }

    /// The gun's world rotation: UE yaw turns X toward Y (-yaw about +Y here), pitch lifts the nose.
    pub fn gun_rotation(&self) -> Quat {
        self.facing * Quat::from_rotation_y(-self.yaw.to_radians()) * Quat::from_rotation_z(self.pitch.to_radians())
    }

    pub fn forward(&self) -> Vec3 {
        self.gun_rotation() * Vec3::X
    }

    pub fn muzzle(&self) -> Vec3 {
        self.pivot + self.forward() * MUZZLE
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
    pub fn turn_to(&mut self, (yaw, pitch): (f32, f32), dt: f32) {
        let k = (dt * ROTATION_SPEED).clamp(0.0, 1.0);
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

pub fn plugin(app: &mut App) {
    app.init_resource::<Turrets>()
        .init_resource::<NoCredit>()
        .add_message::<Shot>()
        .add_systems(OnEnter(AppState::InGame), |mut t: ResMut<Turrets>, mut n: ResMut<NoCredit>| {
            *t = Turrets::default();
            n.0.clear();
        })
        .add_systems(Update, (collect, spawn_guns, take_damage, think, pose_guns).chain().run_if(in_state(AppState::InGame)))
        .add_systems(Update, disarm.after(crate::level::doors::interact_doors).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

fn collect(mut commands: Commands, level: Option<Res<crate::level::Level>>, streaming: Option<ResMut<crate::level::PropStreaming>>, mut turrets: ResMut<Turrets>) {
    let (Some(level), Some(mut streaming)) = (level, streaming) else { return };
    if turrets.collected {
        return;
    }
    turrets.collected = true;
    for a in &level.actors {
        if TurretKind::of(&a.class).is_some() {
            turrets.add(&level, &mut streaming, &mut commands, &a.class, &crate::level::affine(&a.transform));
        }
    }
    info!("turrets: {}", turrets.list.len());
}

fn spawn_guns(mut commands: Commands, assets: Res<AssetServer>, mut turrets: ResMut<Turrets>) {
    for (i, t) in turrets.list.iter_mut().enumerate().filter(|(_, t)| t.gun.is_none() && !t.disabled) {
        let e = commands.spawn((crate::models::model(&assets, t.glb.clone()), Transform::from_translation(t.pivot).with_rotation(t.gun_rotation()), TurretGun(i), DespawnOnExit(AppState::InGame))).id();
        t.gun = Some(e);
    }
}

/// Damage_Object from the survivor's rounds and swings.
fn take_damage(mut turrets: ResMut<Turrets>, mut shots: MessageReader<Shot>, mut swings: MessageReader<super::harvest::Swing>, mut fx: MessageWriter<super::particles::Fx>) {
    let mut hits: Vec<(usize, f32)> = Vec::new();
    for s in shots.read() {
        for (i, t) in turrets.list.iter().enumerate().filter(|(_, t)| !t.dead) {
            let along = (t.pivot - s.origin).dot(s.dir);
            if along > 0.0 && along <= s.reach + HIT_RADIUS && (s.origin + s.dir * along).distance(t.pivot) <= HIT_RADIUS {
                hits.push((i, s.damage));
            }
        }
    }
    for s in swings.read() {
        for (i, t) in turrets.list.iter().enumerate().filter(|(_, t)| !t.dead) {
            let flat = (t.pivot - s.feet).with_y(0.0);
            if flat.length() <= 2.0 && flat.normalize_or_zero().dot(s.facing) > 0.5 {
                hits.push((i, s.damage));
            }
        }
    }
    for (i, dmg) in hits {
        let t = &mut turrets.list[i];
        if t.damage(dmg) {
            // Turret Destroyed Effect.
            fx.write(super::particles::Fx::at("FX_Explosion_New", t.pivot).scaled(Vec3::splat(0.2)));
            t.target = None;
            info!("turret {i} destroyed");
        } else {
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
) {
    let Ok((me, body)) = player.single() else { return };
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let player_centre = me.translation + Vec3::Y * body.height / 2.0;
    let player_alive = !vitals.is_dead() && game.state.is_some();
    for t in turrets.list.iter_mut().filter(|t| !t.dead && !t.disabled) {
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
            seen.retain(|(_, at)| at.distance(t.pivot) <= DETECTION_RANGE && t.in_fov(*at));
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
                t.turn_to(want, dt);
            }
            // TurretRotateAnimation plays on from where it stopped.
            None => {
                t.sweep += dt;
                let want = (Turret::sweep_yaw(t.sweep), 0.0);
                t.turn_to(want, dt);
            }
        }
        // Reload: Delay(Reload Speed), then a full magazine.
        if t.reload > 0.0 {
            t.reload -= dt;
            if t.reload <= 0.0 {
                t.ammo = MAGAZINE;
            }
            continue;
        }
        t.fire_in -= dt;
        if t.fire_in > 0.0 {
            continue;
        }
        t.fire_in += ATTACK_SPEED;
        if aim.is_none() || far {
            continue;
        }
        if t.ammo == 0 {
            t.reload = RELOAD_SECS;
            continue;
        }
        let muzzle = t.muzzle();
        let dir = super::humans::in_cone(t.forward(), SPREAD, &mut rng);
        let ground = crate::gameplay::combat::terrain_hit(terrain.as_deref(), muzzle, dir, WEAPON_RANGE).unwrap_or(WEAPON_RANGE);
        let wall = props.cast(muzzle, dir, ground).unwrap_or(ground);
        t.ammo -= 1;
        if t.ammo == 0 {
            t.reload = RELOAD_SECS;
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
        let Some((d, who, height)) = best else { continue };
        let dmg = DAMAGE[super::humans::hitbox(height) as usize];
        match who {
            None => {
                super::humans::hit(&mut game, &db, &store, dmg, BLEED_CHANCE, &mut rng);
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

fn pose_guns(turrets: Res<Turrets>, mut guns: Query<(&TurretGun, &mut Transform)>) {
    for (g, mut tr) in &mut guns {
        if let Some(t) = turrets.list.get(g.0) {
            tr.rotation = t.gun_rotation();
        }
    }
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
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
    mut prompt: Query<&mut Visibility, With<super::interact::Prompt>>,
    mut text: Query<&mut Text, With<super::interact::PromptText>>,
    mut bar: Query<&mut Node, With<super::interact::PromptBar>>,
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
    if keys.just_pressed(KeyCode::KeyF) {
        let t = &turrets.list[i];
        let behind = behind(t.facing * Vec3::X, t.pivot, p);
        info!("turret {i}: F (behind {behind})");
        if behind {
            turrets.holding = Some((i, 0.0));
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
        t.turn_to((30.0, 10.0), 1.0 / 15.0);
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
    }
}
