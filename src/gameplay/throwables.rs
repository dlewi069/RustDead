//! Throwables (G = IA_Throwable), from BP_PlayerCharacter and Blueprints/Throwables (decompiled):
//!
//! - G, with no montage playing and not in the inventory, plays Throw_Montage if the Throwable
//!   slot holds an item with a `ThrowableClass`; after a 0.8 s Delay one is used up
//!   (SERVER_RequestConsumeItem) and the class is spawned at the Hand_R socket, its Capsule given
//!   `SetPhysicsLinearVelocity` = the camera's forward x `GrenadeThrowSpeed`: twice the distance
//!   to what the camera looks at within 35 m, clamped to 12.5-37.5 m/s (37.5 when nothing is hit).
//! - The throwable is a simulated rigid body (BP_MasterThrowable's Capsule: radius 15 cm, CCD,
//!   LinearDamping 0.5, 0.75 on the frag, pipe bomb and noise maker) with the engine's
//!   DefaultPhysicalMaterial (restitution 0.3, friction 0.7): it bounces off the ground and props
//!   and slides to a stop.
//! - The fuse (`ExplosionDelay`) starts with the throw (Event_Throw):
//!   - BP_Throwable_Frag: everything within 10 m takes MinDamage-MaxDamage (85-115), the
//!     survivor included, no fall-off; a noise heard 50 m away.
//!   - BP_Throwable_PipeBomb: the same within 8 m (75-105).
//!   - BP_Throwable_NoiseMaker: a firecracker heard 160 m away, no damage.
//! - BP_Throwable_Molotov: its first hit (OnComponentHit) smashes the glass, and 0.2 s later the
//!   fire starts (a noise heard 10 m away): for 10 s, every 0.8 s, zombies within 5 m take 10-25
//!   (Damage_Object; the survivor has no BPI_Damage) and everyone in it who isn't already burning
//!   is set burning (`burning.rs`).
//!
//! Ours: the body is a point of the capsule's radius (no tumbling, no rolling; contacts are
//! impulses with Coulomb friction), it comes to rest below 0.3 m/s. The blast and the fire play
//! FX_Explosion_New / FX_MolotovEffect (`particles.rs`), with lights standing in for their Light
//! modules. Not ported: the radial force on loose props, camera shake, scorch decals.

use std::f32::consts::TAU;

use bevy::prelude::*;
use rand::Rng;

use super::burning::{Burn, Burning, PlayerBurning};
use super::damage_numbers::DamageNumber;
use super::combat::{Armed, Dead, Health, PlayerCamera};
use super::equipment::Slot;
use super::items::{ItemDb, ThrowableDef};
use super::particles::Fx;
use super::zombies::Noise;
use crate::level::{KinematicBody, LevelZombie, Terrain};
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{ActionKind, CharacterModel, UpperAction};

/// Seconds into Throw_Montage when the item leaves the hand.
pub const RELEASE: f32 = 0.8;
/// Throw_Montage's length.
const MONTAGE: f32 = 2.1833;
/// UE's default gravity, m/s².
const GRAVITY: f32 = 9.8;
/// BP_MasterThrowable's Capsule radius, m.
const RADIUS: f32 = 0.15;
/// DefaultPhysicalMaterial (both surfaces, so the averaged combine changes nothing).
const RESTITUTION: f32 = 0.3;
const FRICTION: f32 = 0.7;
/// Slower than this while touching something, it lies still (ours: Chaos sleeps bodies).
const REST_SPEED: f32 = 0.3;
/// The Molotov's fire: length, tick, radius.
const FIRE_SECS: f32 = 10.0;
const FIRE_TICK: f32 = 0.8;
const FIRE_RADIUS: f32 = 5.0;
/// The Molotov's Delay between the glass smashing and the fire.
const SMASH_TO_FIRE: f32 = 0.2;

/// What a throwable class does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// Damage everything within `radius` m; heard `noise` m away.
    Blast { radius: f32, noise: f32 },
    /// Only a noise heard this far.
    Noise(f32),
    /// Catches fire on impact.
    Fire,
}

pub fn effect(class: &str) -> Option<Effect> {
    Some(match class {
        "BP_Throwable_Frag_C" => Effect::Blast { radius: 10.0, noise: 50.0 },
        "BP_Throwable_PipeBomb_C" => Effect::Blast { radius: 8.0, noise: 50.0 },
        "BP_Throwable_NoiseMaker_C" => Effect::Noise(160.0),
        "BP_Throwable_Molotov_C" => Effect::Fire,
        _ => return None,
    })
}

/// The class's Capsule LinearDamping (1/s).
pub fn damping(class: &str) -> f32 {
    if class == "BP_Throwable_Molotov_C" { 0.5 } else { 0.75 }
}

/// Chaos's linear ether drag over one step.
pub fn damp(v: Vec3, damping: f32, dt: f32) -> Vec3 {
    v * (1.0 - damping * dt).max(0.0)
}

/// Velocity after touching a surface with unit normal `n`: the normal part comes back at the
/// restitution, the tangential part loses up to friction x the normal impulse.
pub fn bounce(v: Vec3, n: Vec3) -> Vec3 {
    let vn = v.dot(n);
    if vn >= 0.0 {
        return v;
    }
    let tangent = v - n * vn;
    let slide = tangent.length();
    let lost = (FRICTION * (1.0 + RESTITUTION) * -vn).min(slide);
    let tangent = if slide > 0.0 { tangent * (1.0 - lost / slide) } else { Vec3::ZERO };
    tangent - n * (vn * RESTITUTION)
}

/// GrenadeThrowSpeed in m/s, from the distance to the camera's aim point (None: nothing in 35 m).
pub fn throw_speed(aim: Option<f32>) -> f32 {
    aim.map_or(37.5, |d| (d * 2.0).clamp(12.5, 37.5))
}

/// Does a sphere reach an upright capsule standing at `feet` (SphereOverlapActors on pawns)?
pub fn reaches(centre: Vec3, r: f32, feet: Vec3, body: &KinematicBody) -> bool {
    super::shove::touches(centre, r, feet, body.radius, body.height)
}

/// A throw under way: seconds into the montage, and the item.
#[derive(Resource, Default)]
pub struct Throwing(pub Option<(f32, String)>);

/// A throwable in flight or lying where it landed.
#[derive(Component)]
pub struct Thrown {
    pub item: String,
    pub def: ThrowableDef,
    pub velocity: Vec3,
    /// Seconds since the throw when it first hit something (the Molotov's OnComponentHit).
    pub touched: Option<f32>,
    /// Lying still.
    pub landed: bool,
    /// Seconds since the throw.
    pub age: f32,
}

/// A Molotov's fire.
#[derive(Component)]
pub struct Fire {
    pub age: f32,
    pub next_tick: f32,
    pub damage: (f32, f32),
}

/// A blast's flash (fades out).
#[derive(Component)]
struct Flash(f32);

pub fn plugin(app: &mut App) {
    app.init_resource::<Throwing>()
        .add_systems(OnEnter(AppState::InGame), |mut t: ResMut<Throwing>| t.0 = None)
        .add_systems(
            Update,
            start_throw.after(super::combat::CombatSet).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)).run_if(super::on_foot).run_if(super::swim::not_swimming).run_if(super::building::not_building),
        )
        .add_systems(Update, (release, fly, detonate, burn, fade_flashes).chain().run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

#[allow(clippy::type_complexity)]
fn start_throw(
    keys: Res<ButtonInput<KeyCode>>,
    armed: Res<Armed>,
    db: Res<ItemDb>,
    shoving: Res<super::shove::Shoving>,
    mut throwing: ResMut<Throwing>,
    mut game: ResMut<CurrentGame>,
    mut player: Query<&mut UpperAction, (With<CharacterModel>, With<KinematicBody>)>,
) {
    if !keys.just_pressed(KeyCode::KeyG) || throwing.0.is_some() || shoving.0.is_some() || armed.busy() {
        return;
    }
    let Some(item) = super::player_inv(&mut game).equipped.get(&Slot::Throwable).map(|s| s.item.clone()) else { return };
    if db.items.get(&item).and_then(|d| d.throwable.as_ref()).is_none() {
        return;
    }
    let Ok(mut action) = player.single_mut() else { return };
    action.start(ActionKind::Throw);
    throwing.0 = Some((0.0, item));
}

/// At the release: use one up and send it flying.
#[allow(clippy::too_many_arguments)]
fn release(
    mut commands: Commands,
    time: Res<Time>,
    db: Res<ItemDb>,
    assets: Res<AssetServer>,
    terrain: Option<Res<Terrain>>,
    mut props: crate::level::collision::PropRays,
    mut throwing: ResMut<Throwing>,
    mut game: ResMut<CurrentGame>,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
) {
    let Some((t, item)) = throwing.0.as_mut() else { return };
    let before = *t;
    *t += time.delta_secs();
    if *t >= MONTAGE {
        throwing.0 = None;
        return;
    }
    if !(before < RELEASE && *t >= RELEASE) {
        return;
    }
    let item = item.clone();
    let (Ok(cam), Ok(me)) = (camera.single(), player.single()) else { return };
    let Some(def) = db.items.get(&item).and_then(|d| d.throwable.clone()) else { return };
    // SERVER_RequestConsumeItem on the Throwable slot.
    let inv = super::player_inv(&mut game);
    match inv.equipped.get_mut(&Slot::Throwable) {
        Some(s) if s.item == item && s.count > 1 => s.count -= 1,
        Some(s) if s.item == item => {
            inv.equipped.remove(&Slot::Throwable);
        }
        _ => return,
    }
    let (origin, fwd) = (cam.translation(), cam.forward().as_vec3());
    let aim = [props.cast(origin, fwd, 35.0), super::combat::terrain_hit(terrain.as_deref(), origin, fwd, 35.0)].into_iter().flatten().reduce(f32::min);
    let speed = throw_speed(aim);
    // Hand_R at the release (ours: up and to the right of the survivor).
    let hand = me.translation + Vec3::Y * 1.6 + me.rotation * Vec3::new(-0.25, 0.0, 0.2);
    let mesh = db.items.get(&item).and_then(|d| d.mesh.clone());
    let mut e = commands.spawn((Transform::from_translation(hand), Visibility::default(), Thrown { item, def, velocity: fwd * speed, touched: None, landed: false, age: 0.0 }, DespawnOnExit(AppState::InGame)));
    if let Some(m) = mesh {
        e.with_child(crate::models::model(&assets, format!("game/{m}.glb")));
    }
    info!("thrown at {speed:.1} m/s (aim {aim:?})");
}

/// The rigid body's step: gravity and damping, then bounces off props and the landscape.
fn fly(time: Res<Time>, terrain: Option<Res<Terrain>>, mut props: crate::level::collision::PropRays, mut thrown: Query<(&mut Transform, &mut Thrown)>) {
    let dt = time.delta_secs();
    for (mut t, mut th) in &mut thrown {
        th.age += dt;
        if th.landed {
            continue;
        }
        th.velocity.y -= GRAVITY * dt;
        th.velocity = damp(th.velocity, damping(&th.def.class), dt);
        let step = th.velocity * dt;
        let (len, dir) = (step.length(), step.normalize_or_zero());
        let mut contact = false;
        match props.cast_hit(t.translation, dir, len + RADIUS) {
            Some((d, n)) => {
                t.translation += dir * (d - RADIUS).max(0.0);
                th.velocity = bounce(th.velocity, n);
                contact = true;
            }
            None => t.translation += step,
        }
        if let Some(tr) = terrain.as_deref()
            && let Some(g) = tr.ground_under(t.translation)
            && t.translation.y < g + RADIUS
        {
            t.translation.y = g + RADIUS;
            th.velocity = bounce(th.velocity, tr.normal(t.translation.x, t.translation.z));
            contact = true;
        }
        if contact {
            if th.touched.is_none() {
                info!("{} hit something at {:.1} m/s", th.item, th.velocity.length());
                th.touched = Some(th.age);
            }
            if th.velocity.length() < REST_SPEED {
                th.landed = true;
                th.velocity = Vec3::ZERO;
                info!("{} came to rest at {} after {:.2} s", th.item, t.translation, th.age);
            }
        }
    }
}

/// Damage the survivor (through armor) and every zombie whose capsule the sphere reaches.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn hurt_around(
    at: Vec3,
    radius: f32,
    (lo, hi): (f32, f32),
    zombies: &mut Query<(&Transform, &KinematicBody, &mut Health, Option<&mut UpperAction>), (With<LevelZombie>, Without<Dead>)>,
    player: &Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    game: &mut CurrentGame,
    db: &ItemDb,
    store: &SettingsStore,
) -> Vec<f32> {
    let hit = hurt_zombies(at, radius, (lo, hi), zombies);
    if let Ok((t, body)) = player.single()
        && reaches(at, radius, t.translation, body)
    {
        let taken = super::survival::take_hit(game, db, store, rand::thread_rng().gen_range(lo..=hi));
        if let Some(s) = game.state.as_mut() {
            s.survival.health = (s.survival.health - taken).max(0.0);
        }
        info!("caught in the blast: {taken:.0} damage");
    }
    hit
}

/// Damage every zombie whose capsule the sphere reaches; the damage each took.
#[allow(clippy::type_complexity)]
fn hurt_zombies(at: Vec3, radius: f32, (lo, hi): (f32, f32), zombies: &mut Query<(&Transform, &KinematicBody, &mut Health, Option<&mut UpperAction>), (With<LevelZombie>, Without<Dead>)>) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    let mut hit = Vec::new();
    for (t, body, mut h, flinch) in zombies.iter_mut() {
        if reaches(at, radius, t.translation, body) {
            let dmg = rng.gen_range(lo..=hi);
            h.hp -= dmg;
            hit.push(dmg);
            if let Some(mut a) = flinch {
                a.start(ActionKind::Hit);
            }
        }
    }
    hit
}

/// Fuses running out, and Molotovs hitting something.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn detonate(
    mut commands: Commands,
    db: Res<ItemDb>,
    store: Res<SettingsStore>,
    mut game: ResMut<CurrentGame>,
    thrown: Query<(Entity, &Transform, &Thrown)>,
    mut zombies: Query<(&Transform, &KinematicBody, &mut Health, Option<&mut UpperAction>), (With<LevelZombie>, Without<Dead>)>,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    mut noise: MessageWriter<Noise>,
    (mut numbers, mut sound, mut fx): (MessageWriter<DamageNumber>, MessageWriter<crate::audio::Sound>, MessageWriter<Fx>),
) {
    let mut rng = rand::thread_rng();
    for (e, t, th) in &thrown {
        let at = t.translation;
        let Some(effect) = effect(&th.def.class) else {
            commands.entity(e).despawn();
            continue;
        };
        // Skill_DemolitionExpert (SkillCheck): x1.5 on the blast and the fire.
        let demo = demolition(game.state.as_ref());
        match effect {
            Effect::Fire if th.touched.is_some_and(|at| th.age >= at + SMASH_TO_FIRE) => {
                noise.write(Noise { at, range: 10.0 });
                // FX_MolotovEffect at a random yaw, Deactivate()d when the fire ends.
                fx.write(Fx::at("FX_MolotovEffect", at).yawed(rng.gen_range(0.0..TAU)).stop_after(FIRE_SECS));
                commands.spawn((
                    Transform::from_translation(at + Vec3::Y * 0.5),
                    PointLight { color: Color::srgb(1.0, 0.55, 0.2), intensity: 400_000.0, range: 12.0, ..default() },
                    Fire { age: 0.0, next_tick: FIRE_TICK, damage: (th.def.min_damage * demo, th.def.max_damage * demo) },
                    DespawnOnExit(AppState::InGame),
                ));
                commands.entity(e).despawn();
                info!("{} caught fire at {at}", th.item);
            }
            Effect::Blast { radius, noise: heard } if th.age >= th.def.fuse => {
                noise.write(Noise { at, range: heard });
                let hit = hurt_around(at, radius, (th.def.min_damage * demo, th.def.max_damage * demo), &mut zombies, &player, &mut game, &db, &store);
                // FX_Explosion_New at a random yaw, scaled by Random Vector In Range(1, 2) (each
                // axis its own roll).
                fx.write(Fx::at("FX_Explosion_New", at).yawed(rng.gen_range(0.0..TAU)).scaled(random_scale(1.0, 2.0, &mut rng)));
                commands.spawn((
                    Transform::from_translation(at + Vec3::Y * 1.0),
                    PointLight { color: Color::srgb(1.0, 0.75, 0.4), intensity: 4_000_000.0, range: radius * 2.5, ..default() },
                    Flash(0.35),
                    DespawnOnExit(AppState::InGame),
                ));
                commands.entity(e).despawn();
                info!("{} exploded at {at}: {} zombies in {radius} m", th.item, hit.len());
                sound.write(crate::audio::Sound::at("explosion_large_06_Cue", at));
                // SpawnFloatingDamageText per zombie hit, in white.
                numbers.write_batch(hit.iter().map(|&damage| DamageNumber { damage, colour: super::damage_numbers::BLAST }));
            }
            Effect::Noise(heard) if th.age >= th.def.fuse => {
                noise.write(Noise { at, range: heard });
                // The noise maker's pop: FX_Explosion_New at 0.1-0.2 scale.
                fx.write(Fx::at("FX_Explosion_New", at).yawed(rng.gen_range(0.0..TAU)).scaled(random_scale(0.1, 0.2, &mut rng)));
                commands.spawn((Transform::from_translation(at + Vec3::Y * 0.3), PointLight { color: Color::srgb(1.0, 0.9, 0.6), intensity: 300_000.0, range: 6.0, ..default() }, Flash(0.2), DespawnOnExit(AppState::InGame)));
                commands.entity(e).despawn();
                info!("{} went off at {at}", th.item);
            }
            _ => {}
        }
    }
}

/// UBPP_FunctionLibrary's Random Vector In Range: each axis rolled between `lo` and `hi`.
pub fn random_scale(lo: f32, hi: f32, rng: &mut impl Rng) -> Vec3 {
    Vec3::from_array(std::array::from_fn(|_| rng.gen_range(lo..=hi)))
}

/// BP_MasterThrowable.SkillCheck: x1.5 with Skill_DemolitionExpert.
pub fn demolition(state: Option<&crate::engine::GameState>) -> f32 {
    if state.is_some_and(|s| s.tech.has(crate::progression::tech::DEMOLITION_EXPERT)) { 1.5 } else { 1.0 }
}

/// The Molotov's fire: Fire_Damage every 0.8 s for 10 s, the flame flickering. Zombies in it take
/// the damage, and everyone in it who isn't burning yet is set burning.
#[allow(clippy::type_complexity)]
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut player_burning: ResMut<PlayerBurning>,
    mut fires: Query<(Entity, &Transform, &mut Fire, &mut PointLight)>,
    mut zombies: Query<(&Transform, &KinematicBody, &mut Health, Option<&mut UpperAction>), (With<LevelZombie>, Without<Dead>)>,
    unlit: Query<(Entity, &Transform, &KinematicBody), (With<LevelZombie>, Without<Dead>, Without<Burning>)>,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    for (e, t, mut f, mut light) in &mut fires {
        f.age += dt;
        if f.age >= FIRE_SECS {
            commands.entity(e).despawn();
            continue;
        }
        light.intensity = 400_000.0 * (0.8 + 0.2 * (f.age * 17.0).sin() * (f.age * 7.3).cos());
        if f.age >= f.next_tick {
            f.next_tick += FIRE_TICK;
            let ground = t.translation - Vec3::Y * 0.5;
            hurt_zombies(ground, FIRE_RADIUS, f.damage, &mut zombies);
            for (z, zt, body) in &unlit {
                if reaches(ground, FIRE_RADIUS, zt.translation, body) {
                    commands.entity(z).insert(Burning(Burn::zombie(&mut rng)));
                }
            }
            if player_burning.0.is_none()
                && let Ok((pt, body)) = player.single()
                && reaches(ground, FIRE_RADIUS, pt.translation, body)
            {
                player_burning.0 = Some(Burn::player(&mut rng));
                info!("the survivor caught fire");
            }
        }
    }
}

fn fade_flashes(mut commands: Commands, time: Res<Time>, mut flashes: Query<(Entity, &mut Flash, &mut PointLight)>) {
    for (e, mut f, mut l) in &mut flashes {
        f.0 -= time.delta_secs();
        l.intensity *= 0.85;
        if f.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throw_speed_follows_the_aim() {
        assert_eq!(throw_speed(None), 37.5);
        assert_eq!(throw_speed(Some(10.0)), 20.0);
        assert_eq!(throw_speed(Some(2.0)), 12.5);
        assert_eq!(throw_speed(Some(30.0)), 37.5);
    }

    #[test]
    fn classes_have_their_effects_and_data() {
        assert_eq!(effect("BP_Throwable_Frag_C"), Some(Effect::Blast { radius: 10.0, noise: 50.0 }));
        assert_eq!(effect("BP_Throwable_Molotov_C"), Some(Effect::Fire));
        let Ok(text) = std::fs::read_to_string("assets/data/items.json") else { return };
        let db: ItemDb = serde_json::from_str(&text).unwrap();
        let t = |id: &str| db.items[id].throwable.clone().unwrap();
        assert_eq!((t("DA_FragGrenade").min_damage, t("DA_FragGrenade").max_damage, t("DA_FragGrenade").fuse), (85.0, 115.0, 3.0));
        assert_eq!(t("DA_PipeBomb").max_damage, 105.0);
        // Every throwable item's class is one we handle.
        for (id, d) in &db.items {
            if let Some(th) = &d.throwable {
                assert!(effect(&th.class).is_some(), "{id}: {}", th.class);
            }
        }
    }

    #[test]
    fn bounces_keep_restitution_and_lose_friction() {
        // Straight down onto flat ground: back up at 0.3x.
        assert!((bounce(Vec3::new(0.0, -10.0, 0.0), Vec3::Y) - Vec3::new(0.0, 3.0, 0.0)).length() < 1e-4);
        // A glancing hit: the slide loses 0.7 x 1.3 x the normal speed.
        let v = bounce(Vec3::new(10.0, -2.0, 0.0), Vec3::Y);
        assert!((v - Vec3::new(10.0 - 0.7 * 1.3 * 2.0, 0.6, 0.0)).length() < 1e-4, "{v}");
        // Steep hit: friction can stop the slide but not reverse it.
        assert_eq!(bounce(Vec3::new(1.0, -10.0, 0.0), Vec3::Y).x, 0.0);
        // Moving away from the surface: untouched.
        assert_eq!(bounce(Vec3::new(1.0, 2.0, 0.0), Vec3::Y), Vec3::new(1.0, 2.0, 0.0));
        // Off a wall.
        assert!((bounce(Vec3::new(-5.0, 0.0, 0.0), Vec3::X).x - 1.5).abs() < 1e-4);
    }

    #[test]
    fn blast_scales_roll_each_axis() {
        let mut rng = rand::thread_rng();
        let rolls: Vec<Vec3> = (0..50).map(|_| random_scale(1.0, 2.0, &mut rng)).collect();
        assert!(rolls.iter().all(|s| s.min_element() >= 1.0 && s.max_element() <= 2.0));
        assert!(rolls.iter().any(|s| s.x != s.y), "Random Vector In Range rolls X, Y and Z apart");
    }

    #[test]
    fn damping_follows_the_class() {
        assert_eq!(damping("BP_Throwable_Frag_C"), 0.75);
        assert_eq!(damping("BP_Throwable_Molotov_C"), 0.5);
        assert!((damp(Vec3::X * 10.0, 0.75, 0.1) - Vec3::X * 9.25).length() < 1e-4);
        assert_eq!(damp(Vec3::X, 0.75, 2.0), Vec3::ZERO);
    }

    #[test]
    fn blast_reaches_capsules_by_their_nearest_point() {
        let body = KinematicBody::new(0.35, 1.8);
        assert!(reaches(Vec3::ZERO, 10.0, Vec3::new(10.2, 0.0, 0.0), &body));
        assert!(!reaches(Vec3::ZERO, 10.0, Vec3::new(10.5, 0.0, 0.0), &body));
    }
}
