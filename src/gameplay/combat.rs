//! Combat, from BP_PlayerCharacter's input actions and the weapon data assets:
//!
//! - 1-4 draw the Primary / Secondary / Sidearm / Melee slot (`IA_EquipPrimary`...), `\`
//!   (`IA_Unequip`) holsters. Equipped weapons sit in their skeleton sockets: in the hand when
//!   drawn (`EquipSocket`), on the back / hip otherwise (`*UnequipSocket[Backpack]`).
//! - LMB (`IA_PrimaryAction`) fires the drawn firearm at its rolled `FirearmRPM`, Semi or Auto
//!   (X, `IA_ChangeFireMode`, from the "Firemodes" text stat), or swings the melee weapon.
//! - RMB (`IA_Aim`) aims: the camera moves in over the shoulder (Q/E pick which) and the survivor
//!   turns with it. Shots go down the camera's line through the crosshair.
//! - R (`IA_Reload`) tops the attached magazine up from carried ammo it accepts, or slots in a
//!   carried magazine when none is attached.
//! - Zombies (`DamageComponent`) have 100 ± 50 HP × ZOMBIE HEALTH MULTIPLIER, die, and give
//!   level × 5-10 × 1.05 × XP MULTIPLIER XP (`XpMultiplierCalc`). Head hits kill outright (`CanHeadshot1Shot?`).
//! - The BP_Ingame `AmmoBox`: firearm name, fire-mode icon, rounds in the magazine over carried ammo.

use bevy::prelude::*;
use rand::Rng;

use super::equipment::Slot;
use super::interact::{GameOverlay, player_inv};
use super::items::{ItemDb, Stack};
use super::survival::difficulty;
use super::zombies::{Shape, ZombieKind};
use crate::level::{KinematicBody, LevelZombie, Terrain};
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{ActionKind, AnimLibrary, AnimSet, CharacterModel, Locomotion, Stance, UpperAction, model};

/// Seconds to reload when the stance has no reload clip (the rifle and pistol use their
/// Reload_*_Hip montage's length).
const RELOAD_SECS: f32 = 2.0;
/// Melee reach (a guess) and the shortest time between swings (the swing clips set the rest).
const MELEE_REACH: f32 = 2.2;
const MELEE_COOLDOWN: f32 = 0.6;
/// Half-angle of the melee swing, degrees.
const MELEE_CONE: f32 = 60.0;
/// Furthest a shot can land.
const RANGE: f32 = 300.0;
/// Zombie body: capsule radius and the head sphere above the feet (metres).
const BODY_RADIUS: f32 = 0.35;
const BODY_TOP: f32 = 1.5;
const HEAD_CENTRE: f32 = 1.65;
const HEAD_RADIUS: f32 = 0.18;
/// Corpses stay this long.
const CORPSE_SECS: f32 = 60.0;

/// Combat's input handling (draw, aim, reload, fire), for ordering input injected before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CombatSet;

/// The gameplay camera (the one shots are aimed down).
#[derive(Component)]
pub struct PlayerCamera;

/// The drawn weapon and what it's doing.
#[derive(Resource)]
pub struct Armed {
    pub drawn: Option<Slot>,
    /// Fire mode: automatic instead of semi.
    pub auto: bool,
    pub aiming: bool,
    /// Over the right shoulder (+1) or the left (-1).
    pub shoulder: f32,
    cooldown: f32,
    /// Seconds into a reload.
    reload: Option<f32>,
    /// Next melee swing clip (1HMeleeAttack1-4 in turn).
    swing: u8,
}

impl Default for Armed {
    fn default() -> Self {
        Self { drawn: None, auto: false, aiming: false, shoulder: 1.0, cooldown: 0.0, reload: None, swing: 0 }
    }
}

impl Armed {
    pub fn reloading(&self) -> bool {
        self.reload.is_some()
    }
}

/// Hit points (zombies).
#[derive(Component)]
pub struct Health {
    pub hp: f32,
    pub max: f32,
}

/// Killed; the corpse lingers then goes.
#[derive(Component)]
pub struct Dead {
    secs: f32,
    /// Facing when it died (the fall turns from here).
    upright: Quat,
    /// Feet height when it died.
    ground: f32,
}

/// Seconds a corpse takes to topple. The game ragdolls zombies (there's no death clip, only
/// `Zombie_DeathPosition`, a standing start pose); falling back stiffly stands in for that.
const FALL_SECS: f32 = 0.6;

/// A weapon model held or holstered on the survivor.
#[derive(Component)]
struct HeldWeapon;

#[derive(Component)]
struct Tracer(f32);

#[derive(Resource)]
struct TracerAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

impl FromWorld for TracerAssets {
    fn from_world(world: &mut World) -> Self {
        let mesh = world.resource_mut::<Assets<Mesh>>().add(Cuboid::new(0.015, 0.015, 1.0));
        let material = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.9, 0.6, 0.8),
            emissive: LinearRgba::rgb(6.0, 4.5, 2.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        Self { mesh, material }
    }
}

#[derive(Component)]
struct AmmoBox;

#[derive(Component)]
enum AmmoText {
    Name,
    Current,
    Max,
}

#[derive(Component)]
struct FireModeIcon;

pub fn plugin(app: &mut App) {
    app.init_resource::<Armed>()
        .init_resource::<TracerAssets>()
        .add_systems(OnEnter(AppState::InGame), (|mut a: ResMut<Armed>| *a = Armed::default(), spawn_ammo_box))
        .add_systems(
            Update,
            (draw_and_aim, set_stance, reload, fire, kill_zombies)
                .chain()
                .in_set(CombatSet)
                .run_if(in_state(AppState::InGame))
                .run_if(in_state(MenuScreen::None))
                .run_if(super::player_in_control),
        )
        .add_systems(Update, (give_zombies_health, hold_weapons, fade_tracers, rot_corpses, update_ammo_box).run_if(in_state(AppState::InGame)));
}

fn is_firearm(kind: &str) -> bool {
    matches!(kind, "MainFirearm" | "Sidearm")
}

/// The drawn weapon's stack, if it's still equipped.
fn drawn<'a>(armed: &Armed, game: &'a mut CurrentGame) -> Option<&'a mut Stack> {
    player_inv(game).equipped.get_mut(&armed.drawn?)
}

fn draw_and_aim(keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, db: Res<ItemDb>, mut armed: ResMut<Armed>, mut game: ResMut<CurrentGame>) {
    let inv = player_inv(&mut game);
    for (key, slot) in [(KeyCode::Digit1, Slot::Primary), (KeyCode::Digit2, Slot::Secondary), (KeyCode::Digit3, Slot::Sidearm), (KeyCode::Digit4, Slot::Melee)] {
        if keys.just_pressed(key) && inv.equipped.contains_key(&slot) {
            armed.drawn = if armed.drawn == Some(slot) { None } else { Some(slot) };
            armed.reload = None;
            // Semi when the weapon has it, else whatever it has.
            let modes = inv.equipped.get(&slot).and_then(|s| db.items.get(&s.item)).map(|d| d.firemodes()).unwrap_or_default();
            armed.auto = !modes.contains(&"Semi") && modes.contains(&"Auto");
        }
    }
    if keys.just_pressed(KeyCode::Backslash) {
        armed.drawn = None;
    }
    // Unequipped from the inventory: nothing in hand any more.
    if armed.drawn.is_some_and(|s| !inv.equipped.contains_key(&s)) {
        armed.drawn = None;
        armed.reload = None;
    }
    let def = armed.drawn.and_then(|s| inv.equipped.get(&s)).and_then(|s| db.items.get(&s.item));
    if keys.just_pressed(KeyCode::KeyX)
        && let Some(def) = def
    {
        let modes = def.firemodes();
        if modes.contains(&"Semi") && modes.contains(&"Auto") {
            armed.auto = !armed.auto;
        }
    }
    armed.aiming = def.is_some() && mouse.pressed(MouseButton::Right);
    if armed.aiming {
        if keys.just_pressed(KeyCode::KeyQ) {
            armed.shoulder = -1.0;
        }
        if keys.just_pressed(KeyCode::KeyE) {
            armed.shoulder = 1.0;
        }
    }
}

/// The survivor's animation stance follows the drawn weapon's type.
fn set_stance(armed: Res<Armed>, db: Res<ItemDb>, mut game: ResMut<CurrentGame>, mut player: Query<&mut Stance, (With<CharacterModel>, With<KinematicBody>)>) {
    let stance = match drawn(&armed, &mut game).and_then(|w| db.items.get(&w.item)).map(|d| d.kind.as_str()) {
        Some("MainFirearm") => Stance::Rifle,
        Some("Sidearm") => Stance::Pistol,
        Some("Melee") => Stance::Melee,
        _ => Stance::Unarmed,
    };
    for mut s in &mut player {
        s.set_if_neq(stance);
    }
}

fn play(action: &mut UpperAction, kind: ActionKind) {
    action.kind = Some(kind);
    action.serial = action.serial.wrapping_add(1);
}

type PlayerAnim<'w, 's> = Query<'w, 's, (&'static Transform, &'static Stance, &'static mut UpperAction), (With<CharacterModel>, With<KinematicBody>)>;

/// Fill the attached magazine from carried ammo; with no magazine, slot in a carried one.
/// Takes as long as the stance's reload montage.
#[allow(clippy::too_many_arguments)]
fn reload(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    db: Res<ItemDb>,
    lib: Res<AnimLibrary>,
    mut armed: ResMut<Armed>,
    mut game: ResMut<CurrentGame>,
    mut player: PlayerAnim,
    mut length: Local<f32>,
) {
    let Some(slot) = armed.drawn else { return };
    let inv = player_inv(&mut game);
    let Some(weapon) = inv.equipped.get(&slot).cloned() else { return };
    if !db.items.get(&weapon.item).is_some_and(|d| is_firearm(&d.kind)) {
        return;
    }
    if armed.reload.is_none() {
        if !keys.just_pressed(KeyCode::KeyR) {
            return;
        }
        let needs = match weapon.mag(&db) {
            Some(mag) => {
                let def = &db.items[&mag.item];
                mag.rounds < def.capacity && def.ammo.iter().any(|a| inv.count(a) > 0)
            }
            None => mag_for(&db, &weapon.item).is_some_and(|m| inv.count(&m) > 0),
        };
        if needs {
            armed.reload = Some(0.0);
            if let Ok((_, stance, mut action)) = player.single_mut() {
                *length = lib.action_secs(AnimSet::Survivor, *stance, ActionKind::Reload).unwrap_or(RELOAD_SECS);
                play(&mut action, ActionKind::Reload);
            }
        }
        return;
    }
    let t = armed.reload.unwrap_or(0.0) + time.delta_secs();
    if t < *length {
        armed.reload = Some(t);
        return;
    }
    armed.reload = None;
    if weapon.mag(&db).is_none()
        && let Some(mag_item) = mag_for(&db, &weapon.item)
        && let Some(mag) = take_one(inv, &mag_item)
        && let Some(w) = inv.equipped.get_mut(&slot)
    {
        w.attachments.push(mag);
    }
    // Top up from loose rounds the magazine accepts.
    let Some(mag) = inv.equipped.get(&slot).and_then(|w| w.mag(&db)).cloned() else { return };
    let def = &db.items[&mag.item];
    let mut room = def.capacity.saturating_sub(mag.rounds);
    let mut loaded = 0;
    for ammo in &def.ammo {
        let n = inv.count(ammo).min(room);
        if n > 0 && inv.remove(ammo, n) {
            loaded += n;
            room -= n;
        }
    }
    if let Some(m) = inv.equipped.get_mut(&slot).and_then(|w| w.mag_mut(&db)) {
        m.rounds += loaded;
    }
}

/// The magazine a firearm takes (its default `Mag` attachment).
fn mag_for(db: &ItemDb, weapon: &str) -> Option<String> {
    db.items.get(weapon)?.attachments.iter().find(|a| a.kind == "Mag").map(|a| a.item.clone())
}

/// Take one carried item (a whole magazine, keeping its rounds) out of the pockets/containers.
fn take_one(inv: &mut super::equipment::PlayerInventory, item: &str) -> Option<Stack> {
    let refs: Vec<_> = inv.grids().into_iter().map(|(r, _)| r).collect();
    for r in refs {
        let g = inv.grid_mut(r)?;
        if let Some(i) = g.items.iter().position(|p| p.stack.item == item) {
            let mut p = g.items.remove(i);
            if p.stack.count > 1 {
                let mut rest = p.clone();
                rest.stack.count -= 1;
                g.items.insert(i, rest);
                p.stack.count = 1;
            }
            return Some(p.stack);
        }
    }
    None
}

/// Damage after a critical roll: `CriticalHitChance` %, `CriticalHitMultiplier` % extra.
fn with_crit(base: f32, chance: f32, mult: f32, rng: &mut impl Rng) -> f32 {
    if rng.gen_range(0.0..100.0) < chance { base * (1.0 + mult / 100.0) } else { base }
}

/// `EffectiveDamage`: within the effective range (`DamageFallOff` metres; a muzzle attachment's
/// Falloff stat would shorten it) full damage, beyond it `damage - 7 * dist / range`, at least 1.
/// Projectile weapons (no DamageFallOff stat) don't use it.
fn falloff(damage: f32, dist: f32, range: f32) -> f32 {
    if range <= 0.0 || dist < range { damage } else { (damage - 7.0 * dist / range).clamp(1.0, 9999.0) }
}

/// `DurabilityCheck`: does the round go off? With FIREARM MALFUNCTION on, at 85 durability or less
/// it jams with `malfunction_chance * (1 - durability / 100)`; at zero it always jams. `roll` is
/// RandomFloatInRange(0, 100).
fn goes_off(durability: f32, malfunctions: bool, malfunction_chance: f32, roll: f32) -> bool {
    if durability <= 0.0 {
        return false;
    }
    if !malfunctions || durability > 85.0 {
        return true;
    }
    roll >= malfunction_chance * (1.0 - durability / 100.0) * 100.0
}

/// Distance along unit `dir` into a sphere, if the ray meets it ahead.
fn ray_sphere(origin: Vec3, dir: Vec3, centre: Vec3, r: f32) -> Option<f32> {
    let m = origin - centre;
    let b = m.dot(dir);
    let c = m.dot(m) - r * r;
    let disc = b * b - c;
    (disc >= 0.0 && (c <= 0.0 || b < 0.0)).then(|| (-b - disc.sqrt()).max(0.0))
}

/// Distance along unit `dir` into a capsule from `a` to `b` of radius `r` (Quilez).
fn ray_capsule(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, r: f32) -> Option<f32> {
    let (ba, oa) = (b - a, origin - a);
    let (baba, bard, baoa) = (ba.dot(ba), ba.dot(dir), ba.dot(oa));
    let k2 = baba - bard * bard;
    let k1 = baba * dir.dot(oa) - baoa * bard;
    let k0 = baba * oa.dot(oa) - baoa * baoa - r * r * baba;
    let h = k1 * k1 - k2 * k0;
    if h >= 0.0 && k2.abs() > 1e-9 {
        let t = (-k1 - h.sqrt()) / k2;
        let y = baoa + t * bard;
        if t > 0.0 && y > 0.0 && y < baba {
            return Some(t);
        }
    }
    [ray_sphere(origin, dir, a, r), ray_sphere(origin, dir, b, r)].into_iter().flatten().min_by(f32::total_cmp)
}

/// A hound lying along its facing: a body capsule from haunch to shoulders and a head sphere
/// (from SK_Animal_Dog_Zombie_GermanShepherd: 1.7 m nose to tail, head ~0.75 m up).
fn ray_hits_hound(origin: Vec3, dir: Vec3, t: &Transform) -> Option<(f32, bool)> {
    let at = |v: Vec3| t.translation + t.rotation * v;
    let body = ray_capsule(origin, dir, at(Vec3::new(0.0, 0.5, -0.55)), at(Vec3::new(0.0, 0.55, 0.3)), 0.22);
    let head = ray_sphere(origin, dir, at(Vec3::new(0.0, 0.72, 0.55)), 0.17);
    match (head, body) {
        (Some(h), Some(b)) if b < h => Some((b, false)),
        (Some(h), _) => Some((h, true)),
        (None, Some(b)) => Some((b, false)),
        _ => None,
    }
}

/// A crawler dragging itself along: a 1.8 m body lying along its facing, head forward and low.
/// (Proportions from the upright survivor's; the crawl pose isn't measured.)
fn ray_hits_prone(origin: Vec3, dir: Vec3, t: &Transform) -> Option<(f32, bool)> {
    let at = |v: Vec3| t.translation + t.rotation * v;
    let body = ray_capsule(origin, dir, at(Vec3::new(0.0, 0.25, -1.1)), at(Vec3::new(0.0, 0.3, 0.25)), 0.25);
    let head = ray_sphere(origin, dir, at(Vec3::new(0.0, 0.35, 0.55)), HEAD_RADIUS);
    match (head, body) {
        (Some(h), Some(b)) if b < h => Some((b, false)),
        (Some(h), _) => Some((h, true)),
        (None, Some(b)) => Some((b, false)),
        _ => None,
    }
}

/// Where a ray first meets a zombie of `kind`: (distance, headshot).
fn ray_hits(origin: Vec3, dir: Vec3, t: &Transform, kind: ZombieKind) -> Option<(f32, bool)> {
    match kind.shape() {
        Shape::Upright => ray_hits_zombie(origin, dir, t.translation),
        Shape::Prone => ray_hits_prone(origin, dir, t),
        Shape::Hound => ray_hits_hound(origin, dir, t),
    }
}

/// Where a ray first meets a zombie: (distance, headshot).
fn ray_hits_zombie(origin: Vec3, dir: Vec3, feet: Vec3) -> Option<(f32, bool)> {
    // Head sphere.
    let head = feet + Vec3::Y * HEAD_CENTRE;
    let to = head - origin;
    let along = to.dot(dir);
    let head_hit = (along > 0.0 && (to - dir * along).length() < HEAD_RADIUS).then_some(along);
    // Body: vertical capsule axis from the feet to BODY_TOP, as a cylinder in plan.
    let flat = Vec2::new(dir.x, dir.z);
    let rel = Vec2::new(feet.x - origin.x, feet.z - origin.z);
    let body_hit = (flat.length_squared() > 1e-6).then(|| {
        let a = flat.length_squared();
        let b = -2.0 * rel.dot(flat);
        let c = rel.length_squared() - BODY_RADIUS * BODY_RADIUS;
        let disc = b * b - 4.0 * a * c;
        (disc >= 0.0).then(|| (-b - disc.sqrt()) / (2.0 * a))
    });
    let body_hit = body_hit.flatten().filter(|&t| {
        let y = origin.y + dir.y * t;
        t > 0.0 && y >= feet.y && y <= feet.y + BODY_TOP
    });
    match (head_hit, body_hit) {
        (Some(h), Some(b)) if b < h => Some((b, false)),
        (Some(h), _) => Some((h, true)),
        (None, Some(b)) => Some((b, false)),
        _ => None,
    }
}

/// Distance at which the ray dips under the terrain, if it does within `max`.
fn terrain_hit(terrain: Option<&Terrain>, origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
    let t = terrain?;
    let mut d = 0.5;
    while d < max {
        let p = origin + dir * d;
        if p.y < t.height(p.x, p.z) {
            return Some(d);
        }
        d += 0.75;
    }
    None
}

fn store_bool(store: &SettingsStore, page: &str, label: &str) -> bool {
    store.find(&row_key("difficulty", page, label)) == Some(Value::Bool(true))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fire(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    db: Res<ItemDb>,
    store: Res<SettingsStore>,
    tracer: Res<TracerAssets>,
    terrain: Option<Res<Terrain>>,
    mut props: crate::level::collision::PropRays,
    mut armed: ResMut<Armed>,
    mut game: ResMut<CurrentGame>,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
    mut player: PlayerAnim,
    lib: Res<AnimLibrary>,
    mut zombies: Query<(&Transform, &LevelZombie, &mut Health, Option<&mut UpperAction>), (Without<Dead>, Without<CharacterModel>)>,
    mut noise: MessageWriter<super::zombies::Noise>,
    mut notify: MessageWriter<super::notify::Notify>,
) {
    armed.cooldown = (armed.cooldown - time.delta_secs()).max(0.0);
    if armed.drawn.is_none() {
        return;
    }
    let (Ok(cam), Ok((me, stance, mut action))) = (camera.single(), player.single_mut()) else { return };
    let me = *me;
    let stance = *stance;
    let auto = armed.auto;
    let trigger = if auto { mouse.pressed(MouseButton::Left) } else { mouse.just_pressed(MouseButton::Left) };
    if !trigger || armed.cooldown > 0.0 || armed.reloading() {
        return;
    }
    let headshot_mode = store_bool(&store, "General", "HEADSHOT MODE ENABLED?");
    let multi_hit = store_bool(&store, "General", "MELEE MULTI HIT ENABLED?");
    let Some(weapon) = drawn(&armed, &mut game).cloned() else { return };
    let Some(def) = db.items.get(&weapon.item) else { return };
    let mut rng = rand::thread_rng();
    let crit = |base: f32, rng: &mut rand::rngs::ThreadRng| {
        with_crit(base, weapon.stat(&db, "CriticalHitChance").unwrap_or(0.0), weapon.stat(&db, "CriticalHitMultiplier").unwrap_or(0.0), rng)
    };
    let hurt = |h: &mut Health, flinch: Option<Mut<UpperAction>>, dmg: f32, head: bool| {
        if let Some(mut a) = flinch {
            play(&mut a, ActionKind::Hit);
        }
        if head {
            h.hp = 0.0; // CanHeadshot1Shot?
        } else {
            h.hp -= dmg;
            if headshot_mode {
                h.hp = h.hp.max(1.0); // HEADSHOT MODE: only the head kills
            }
        }
    };

    if is_firearm(&def.kind) {
        let rpm = weapon.stat(&db, "FirearmRPM").unwrap_or(600.0).max(1.0);
        armed.cooldown = 60.0 / rpm;
        let fa = def.firearm.unwrap_or_default();
        // Local_StartShooting: an empty or missing magazine clicks and says so.
        let rounds = drawn(&armed, &mut game).and_then(|w| w.mag_mut(&db).map(|m| m.rounds));
        if rounds.is_none_or(|r| r == 0) {
            armed.cooldown = 0.25;
            if mouse.just_pressed(MouseButton::Left) {
                notify.write(super::notify::Notify::new(if rounds.is_none() { "No mag in firearm" } else { "No ammo" }));
            }
            return;
        }
        // DurabilityCheck: worn guns can jam (the round stays in the magazine).
        let durability = weapon.durability.unwrap_or(def.durability);
        if !goes_off(durability, store_bool(&store, "General", "FIREARM MALFUNCTION ENABLED?"), fa.malfunction_chance, rng.gen_range(0.0..100.0)) {
            armed.cooldown = 0.25;
            notify.write(super::notify::Notify::new("Weapon jammed"));
            return;
        }
        if let Some(m) = drawn(&armed, &mut game).and_then(|w| w.mag_mut(&db)) {
            m.rounds -= 1;
        }
        // ReduceDurability (WEAPON DURABILITY): ReduceDurabilityAmount x the multiplier per shot.
        if store_bool(&store, "General", "WEAPON DURABILITY ENABLED?")
            && let Some(w) = drawn(&armed, &mut game)
        {
            let loss = def.reduce_durability * difficulty(&store, "General", "WEAPON DURABILITY MULTIPLIER");
            w.durability = Some((durability - loss).max(0.0));
        }
        play(&mut action, ActionKind::Fire);
        // ReportNoiseEvent out to the weapon's DefaultSoundRadius (suppressors aren't modelled).
        let heard = if fa.sound_radius > 0.0 { fa.sound_radius } else { super::zombies::HEARING };
        noise.write(super::zombies::Noise { at: me.translation, range: heard.min(super::zombies::HEARING) });
        let origin = cam.translation();
        let dir = cam.forward().as_vec3();
        // Rounds stop at the ground or the first prop (walls, cars, trees with collision).
        let ground = terrain_hit(terrain.as_deref(), origin, dir, RANGE).unwrap_or(RANGE);
        let wall = props.cast(origin, dir, ground).unwrap_or(ground);
        let mut best: Option<(f32, bool, Mut<Health>, Option<Mut<UpperAction>>)> = None;
        for (t, z, h, a) in &mut zombies {
            if let Some((d, head)) = ray_hits(origin, dir, t, z.kind)
                && d < wall
                && best.as_ref().is_none_or(|b| d < b.0)
            {
                best = Some((d, head, h, a));
            }
        }
        debug!("shot from {origin} along {dir}: ground at {wall:.1} m, hit {:?}", best.as_ref().map(|b| (b.0, b.1)));
        let end = best.as_ref().map_or(origin + dir * wall, |b| origin + dir * b.0);
        if let Some((d, head, mut h, a)) = best {
            let base = weapon.stat(&db, "FirearmDamage").unwrap_or(30.0);
            let dmg = falloff(crit(base, &mut rng), d, weapon.stat(&db, "DamageFallOff").unwrap_or(0.0));
            hurt(&mut h, a, dmg, head);
        }
        // Tracer from the weapon (right hand, chest height) to where the round landed.
        let muzzle = me.translation + Vec3::Y * 1.35 + me.rotation * Vec3::new(-0.25, 0.0, 0.5);
        let span = end - muzzle;
        if span.length() > 0.5 {
            commands.spawn((
                Mesh3d(tracer.mesh.clone()),
                MeshMaterial3d(tracer.material.clone()),
                Transform::from_translation(muzzle + span / 2.0).looking_to(span.normalize(), Vec3::Y).with_scale(Vec3::new(1.0, 1.0, span.length())),
                Tracer(0.06),
                DespawnOnExit(AppState::InGame),
            ));
        }
    } else if def.kind == "Melee" {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        // The weapon's attack montages in turn; the next swing waits for most of this one.
        let swing = ActionKind::Swing(armed.swing);
        armed.swing = (armed.swing + 1) % 4;
        armed.cooldown = lib.action_secs(AnimSet::Survivor, stance, swing).map_or(MELEE_COOLDOWN, |d| (d * 0.8).max(MELEE_COOLDOWN));
        play(&mut action, swing);
        if let Some(state) = game.state.as_mut() {
            let drain = weapon.stat(&db, "StaminaDrain").unwrap_or(0.0);
            state.survival.stamina = (state.survival.stamina - drain).max(0.0);
        }
        let facing = me.rotation * Vec3::Z;
        let base = weapon.stat(&db, "MeleeDamage").unwrap_or(20.0);
        let mut hits: Vec<(f32, Mut<Health>, Option<Mut<UpperAction>>)> = zombies
            .iter_mut()
            .filter_map(|(t, _, h, a)| {
                let to = (t.translation - me.translation) * Vec3::new(1.0, 0.0, 1.0);
                let d = to.length();
                (d < MELEE_REACH && facing.angle_between(to.normalize_or_zero()).to_degrees() < MELEE_CONE).then_some((d, h, a))
            })
            .collect();
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        // MELEE MULTI HIT: everyone in the arc, else the nearest.
        for (_, mut h, a) in hits.into_iter().take(if multi_hit { usize::MAX } else { 1 }) {
            let dmg = crit(base, &mut rng);
            hurt(&mut h, a, dmg, false);
        }
    }
}

fn give_zombies_health(mut commands: Commands, store: Res<SettingsStore>, added: Query<(Entity, &LevelZombie), (Added<LevelZombie>, Without<Health>)>) {
    let mult = difficulty(&store, "AI", "ZOMBIE HEALTH MULTIPLIER");
    let mut rng = rand::thread_rng();
    for (e, z) in &added {
        // DamageComponent health +- the class's HealthDeviation (100 +- 50; hounds 125 +- 25).
        let (base, dev) = z.kind.health();
        let hp = (base + rng.gen_range(-dev..=dev)) * mult;
        commands.entity(e).insert(Health { hp, max: hp });
    }
}

/// `XpMultiplierCalc` then `AddXP`: CurrentLevel x Random(XPMultiplier_Min 5, Max 10) x the
/// LevellingComponent's MultiplyValue 1.05, times XP MULTIPLIER (`Difficulty_MultiplierValue`).
fn kill_xp(level: u32, roll: f32, xp_mult: f32) -> f32 {
    level.max(1) as f32 * roll * 1.05 * xp_mult
}

#[allow(clippy::type_complexity)]
fn kill_zombies(
    mut commands: Commands,
    store: Res<SettingsStore>,
    mut game: ResMut<CurrentGame>,
    mut zombies: Query<(Entity, &Health, &Transform, &mut Locomotion, Option<&LevelZombie>), Without<Dead>>,
    mut notify: MessageWriter<super::notify::Notify>,
) {
    let xp_mult = difficulty(&store, "General", "XP MULTIPLIER");
    let popup = store.find(&row_key("settings", "Gameplay", "XP POP UP ENABLED?")) == Some(Value::Bool(true));
    for (e, h, t, mut loco, z) in &mut zombies {
        if h.hp > 0.0 {
            continue;
        }
        *loco = Locomotion::Dead;
        // No longer an obstacle (or a falling body).
        commands.entity(e).insert(Dead { secs: 0.0, upright: t.rotation, ground: t.translation.y }).remove::<KinematicBody>();
        if let Some(state) = game.state.as_mut() {
            let [lo, hi] = z.map_or([5.0, 10.0], |z| z.kind.class().xp);
            let xp = kill_xp(state.progression.level, rand::thread_rng().gen_range(lo..=hi), xp_mult);
            state.progression.add_xp(xp.round() as u64);
            if popup {
                notify.write(super::notify::Notify::new(format!("Zombie Killed, XP Given: {}", xp.trunc())));
            }
        }
    }
}

fn rot_corpses(mut commands: Commands, time: Res<Time>, mut corpses: Query<(Entity, &mut Dead, &mut Locomotion, &mut Transform)>) {
    for (e, mut d, mut loco, mut t) in &mut corpses {
        loco.set_if_neq(Locomotion::Dead);
        d.secs += time.delta_secs();
        let k = (d.secs / FALL_SECS).min(1.0);
        // Over backwards about the feet, accelerating like a fall, lifted by half the body's
        // thickness so it lies on the ground rather than half in it.
        let fall = k * k;
        t.rotation = d.upright * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2 * fall);
        t.translation.y = d.ground + 0.15 * fall;
        if d.secs > CORPSE_SECS {
            commands.entity(e).despawn();
        }
    }
}

fn fade_tracers(mut commands: Commands, time: Res<Time>, mut tracers: Query<(Entity, &mut Tracer)>) {
    for (e, mut t) in &mut tracers {
        t.0 -= time.delta_secs();
        if t.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

/// Weapon models in the survivor's sockets: the drawn one in the hand, the rest holstered.
#[allow(clippy::too_many_arguments)]
fn hold_weapons(
    mut commands: Commands,
    armed: Res<Armed>,
    db: Res<ItemDb>,
    assets: Res<AssetServer>,
    mut game: ResMut<CurrentGame>,
    player: Query<Entity, (With<CharacterModel>, With<KinematicBody>)>,
    children: Query<&Children>,
    names: Query<&Name>,
    held: Query<Entity, With<HeldWeapon>>,
    mut last: Local<Vec<(String, String)>>,
) {
    let Ok(me) = player.single() else { return };
    let inv = player_inv(&mut game);
    let backpack = inv.equipped.contains_key(&Slot::Backpack);
    let mut want = Vec::new();
    for slot in [Slot::Primary, Slot::Secondary, Slot::Sidearm, Slot::Melee] {
        let Some(stack) = inv.equipped.get(&slot) else { continue };
        let Some(def) = db.items.get(&stack.item) else { continue };
        let Some(mesh) = def.mesh.clone() else { continue };
        let e = def.equip.clone().unwrap_or_default();
        let socket = if armed.drawn == Some(slot) {
            e.equip_socket.or_else(|| Some(if slot == Slot::Sidearm { "Sidearm_r" } else { "Weapon_r" }.into()))
        } else {
            match (slot, backpack) {
                (Slot::Secondary, false) => e.secondary_unequip_socket,
                (Slot::Secondary, true) => e.secondary_unequip_socket_backpack.or(e.secondary_unequip_socket),
                (Slot::Sidearm, _) => e.primary_unequip_socket.or_else(|| Some("Sidearm".into())),
                (_, false) => e.primary_unequip_socket,
                (_, true) => e.primary_unequip_socket_backpack.or(e.primary_unequip_socket),
            }
        };
        if let Some(socket) = socket {
            want.push((socket, mesh, stack.item.clone()));
        }
    }
    let key: Vec<(String, String)> = want.iter().map(|w| (w.0.clone(), w.2.clone())).collect();
    if *last == key {
        return;
    }
    // Every body part has its own copy of the skeleton, animated alike; any copy will do.
    let bone_entity = |bone: &str| {
        children.iter_descendants(me).find(|&c| names.get(c).is_ok_and(|n| n.as_str() == bone || n.as_str().strip_prefix(bone).is_some_and(|r| r.starts_with('_') && r[1..].chars().all(|c| c.is_ascii_digit()))))
    };
    let mut spawned = Vec::new();
    for (socket, mesh, item) in &want {
        // A name that isn't a socket is a bone (UE resolves sockets that way).
        let (bone_name, socket_tf) = match db.sockets.get(socket) {
            Some(s) => (s.bone.as_str(), s.transform.to_transform()),
            None if socket != "None" => (socket.as_str(), Transform::IDENTITY),
            None => continue,
        };
        let Some(bone) = bone_entity(bone_name) else { return }; // body still loading
        let offset = db.items.get(item).and_then(|d| d.equip.as_ref()).and_then(|e| e.transform).map_or(Transform::IDENTITY, |t| t.to_transform());
        let local = socket_tf * offset;
        spawned.push((bone, local, mesh.clone()));
    }
    for e in &held {
        commands.entity(e).despawn();
    }
    for (bone, local, mesh) in spawned {
        let w = commands.spawn((model(&assets, format!("game/{mesh}.glb")), local, HeldWeapon)).id();
        commands.entity(bone).add_child(w);
    }
    *last = key;
}

// ---------------------------------------------------------------- BP_Ingame AmmoBox

fn spawn_ammo_box(mut commands: Commands, font: Res<MenuFont>, assets: Res<AssetServer>) {
    let grey = Color::linear_rgb(0.5, 0.5, 0.5);
    commands.spawn((
        Node { position_type: PositionType::Absolute, right: Val::Px(10.0), bottom: Val::Px(10.0), padding: UiRect::all(Val::Px(10.0)), flex_direction: FlexDirection::Column, ..default() },
        Visibility::Hidden,
        AmmoBox,
        DespawnOnExit(AppState::InGame),
        children![
            (
                Node { align_self: AlignSelf::End, padding: UiRect::axes(Val::Px(10.0), Val::Px(2.0)), ..default() },
                ImageNode::new(assets.load("game/SurrounDead/Content/UI/Textures/fade-left.png")).with_color(Color::BLACK),
                children![(label_colored(&font, "", 14.0, grey), AmmoText::Name)],
            ),
            (
                Node { align_items: AlignItems::End, column_gap: Val::Px(10.0), ..default() },
                children![
                    (
                        Node { width: Val::Px(30.0), height: Val::Px(30.0), margin: UiRect::horizontal(Val::Px(5.0)), ..default() },
                        ImageNode::new(assets.load("game/SurrounDead/Content/UI/Textures/Ammo/FireType_Semi.png")),
                        FireModeIcon,
                    ),
                    (
                        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::End, flex_grow: 1.0, ..default() },
                        children![(label_colored(&font, "0", 20.0, Color::WHITE), AmmoText::Current), (label_colored(&font, "0", 14.0, grey), AmmoText::Max)],
                    ),
                ],
            ),
        ],
    ));
}

#[allow(clippy::too_many_arguments)]
fn update_ammo_box(
    armed: Res<Armed>,
    db: Res<ItemDb>,
    assets: Res<AssetServer>,
    overlay: Res<GameOverlay>,
    screen: Res<State<MenuScreen>>,
    mut game: ResMut<CurrentGame>,
    mut boxes: Query<&mut Visibility, With<AmmoBox>>,
    mut texts: Query<(&AmmoText, &mut Text)>,
    mut icons: Query<&mut ImageNode, With<FireModeIcon>>,
) {
    let Ok(mut vis) = boxes.single_mut() else { return };
    let weapon = drawn(&armed, &mut game).cloned();
    let def = weapon.as_ref().and_then(|w| db.items.get(&w.item)).filter(|d| is_firearm(&d.kind));
    let (Some(weapon), Some(def)) = (weapon, def) else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = if overlay.is_open() || *screen.get() != MenuScreen::None { Visibility::Hidden } else { Visibility::Inherited };
    let mag = weapon.mag(&db);
    let current = mag.map_or(0, |m| m.rounds);
    let inv = player_inv(&mut game);
    let ammo_ids = mag.and_then(|m| db.items.get(&m.item)).map(|d| d.ammo.clone()).or_else(|| mag_for(&db, &weapon.item).and_then(|m| db.items.get(&m)).map(|d| d.ammo.clone())).unwrap_or_default();
    let carried: u32 = ammo_ids.iter().map(|a| inv.count(a)).sum();
    for (which, mut t) in &mut texts {
        let v = match which {
            AmmoText::Name => if armed.reloading() { format!("{} (RELOADING)", def.name) } else { def.name.clone() },
            AmmoText::Current => current.to_string(),
            AmmoText::Max => carried.to_string(),
        };
        if t.0 != v {
            t.0 = v;
        }
    }
    let tex = if armed.auto { "Auto" } else { "Semi" };
    for mut img in &mut icons {
        let h = assets.load(format!("game/SurrounDead/Content/UI/Textures/Ammo/FireType_{tex}.png"));
        if img.image != h {
            img.image = h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hounds_are_long_and_low() {
        let dog = Transform::default(); // facing +Z
        // From the side at shoulder height: body. At head height in front: the head.
        assert_eq!(ray_hits(Vec3::new(-5.0, 0.5, -0.4), Vec3::X, &dog, ZombieKind::hound()).map(|h| h.1), Some(false));
        let (d, head) = ray_hits(Vec3::new(0.0, 0.72, 5.0), -Vec3::Z, &dog, ZombieKind::hound()).unwrap();
        assert!(head && (d - (5.0 - 0.72)).abs() < 0.01, "{d}");
        // Over its back, where a walker's chest would be: miss.
        assert!(ray_hits(Vec3::new(-5.0, 1.3, 0.0), Vec3::X, &dog, ZombieKind::hound()).is_none());
        assert!(ray_hits(Vec3::new(-5.0, 1.3, 0.0), Vec3::X, &dog, ZombieKind::default()).is_some());
        // Turned 90°, the tail end is off to the side.
        let turned = Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        assert!(ray_hits(Vec3::new(-0.5, 0.5, -5.0), Vec3::Z, &turned, ZombieKind::hound()).is_some());
        assert!(ray_hits(Vec3::new(-0.5, 0.5, -5.0), Vec3::Z, &dog, ZombieKind::hound()).is_none());
    }

    #[test]
    fn rays_hit_heads_above_bodies_and_miss_beside() {
        let feet = Vec3::new(0.0, 0.0, 10.0);
        let (d, head) = ray_hits_zombie(Vec3::new(0.0, 1.0, 0.0), Vec3::Z, feet).unwrap();
        assert!(!head && (d - (10.0 - BODY_RADIUS)).abs() < 1e-3);
        let (_, head) = ray_hits_zombie(Vec3::new(0.0, HEAD_CENTRE, 0.0), Vec3::Z, feet).unwrap();
        assert!(head);
        assert!(ray_hits_zombie(Vec3::new(1.0, 1.0, 0.0), Vec3::Z, feet).is_none());
        assert!(ray_hits_zombie(Vec3::new(0.0, 1.0, 20.0), Vec3::Z, feet).is_none(), "behind the shooter");
    }

    #[test]
    fn kill_xp_scales_with_level() {
        assert!((kill_xp(1, 5.0, 1.0) - 5.25).abs() < 1e-4);
        assert!((kill_xp(10, 10.0, 2.0) - 210.0).abs() < 1e-3);
        assert!((kill_xp(0, 10.0, 1.0) - 10.5).abs() < 1e-4, "level 0 counts as 1");
    }

    #[test]
    fn falloff_is_effective_damage() {
        // Inside DamageFallOff: full damage. Beyond: damage - 7 * dist / range, never below 1.
        assert_eq!(falloff(40.0, 20.0, 30.0), 40.0);
        assert_eq!(falloff(40.0, 30.0, 30.0), 33.0);
        assert_eq!(falloff(40.0, 60.0, 30.0), 26.0);
        assert_eq!(falloff(40.0, 1000.0, 30.0), 1.0);
        // Projectile weapons have no falloff stat.
        assert_eq!(falloff(40.0, 1000.0, 0.0), 40.0);
    }

    #[test]
    fn worn_guns_jam_only_with_malfunctions_on() {
        // Off: anything above zero durability fires; zero always jams.
        assert!(goes_off(1.0, false, 0.025, 0.0));
        assert!(!goes_off(0.0, false, 0.025, 99.0));
        // On: above 85 never jams; below, chance * (1 - durability / 100).
        assert!(goes_off(90.0, true, 0.5, 0.0));
        // 0.5 * (1 - 0.5) = 25 % jam: rolls under 25 jam.
        assert!(!goes_off(50.0, true, 0.5, 24.9));
        assert!(goes_off(50.0, true, 0.5, 25.0));
    }

    #[test]
    fn spawned_guns_carry_their_pickup_data() {
        let Ok(text) = std::fs::read_to_string("assets/data/items.json") else { return };
        let db: ItemDb = serde_json::from_str(&text).unwrap();
        let m4 = db.items["DA_M4CQB"].firearm.unwrap();
        assert!(m4.automatic && m4.kind == 2 && m4.sound_radius == 70.0);
        assert_eq!(m4.headshot_multiplier(), 2.25);
        let m9 = db.items["DA_M9"].firearm.unwrap();
        assert!(!m9.automatic && m9.sound_radius == 50.0 && m9.headshot_multiplier() == 2.0);
        assert!(db.items["DA_M4CQB"].reduce_durability > 0.0 && db.items["DA_M4CQB"].durability == 100.0);
        // Every gun has its component data.
        for (k, d) in &db.items {
            assert_eq!(is_firearm(&d.kind), d.firearm.is_some(), "{k}");
        }
    }

    #[test]
    fn spawned_rifles_roll_stats_and_magazines() {
        let Ok(text) = std::fs::read_to_string("assets/data/items.json") else { return };
        let db: ItemDb = serde_json::from_str(&text).unwrap();
        let mut rng = rand::thread_rng();
        let mut with_mag = 0;
        for _ in 0..200 {
            let m4 = db.spawn("DA_M4CQB", 1, &mut rng);
            assert_eq!(m4.stats.len(), 4, "MaxStats 4");
            let dmg = m4.stats["FirearmDamage"];
            assert!((38.0..=44.0).contains(&dmg) && dmg.fract() == 0.0);
            if let Some(mag) = m4.mag(&db) {
                with_mag += 1;
                assert!(mag.rounds <= 30);
            }
        }
        assert!((100..=190).contains(&with_mag), "75% spawn with a magazine, got {with_mag}/200");
        assert_eq!(mag_for(&db, "DA_M4CQB").as_deref(), Some("DA_556Mag"));
    }
}
