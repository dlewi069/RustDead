//! Projectile weapons: the grenade launcher, rocket launcher and flare gun, from
//! BP_FirearmPickup's projectile branch and Inventory/Items/Other/ItemProjectiles (decompiled).
//!
//! - **Firing** (BP_FirearmPickup, DoesWeaponUseAProjectile? on the weapon component): the
//!   magazine, jam, durability, recoil, sound and noise are the gun's as usual (`combat::fire`),
//!   but instead of FireTrace, ProjectileTraceFromCamera traces 10 km down the camera's line
//!   (no hip-fire cone) and the ProjectileClass is spawned at the Muzzle socket, facing the point
//!   hit (or the end of the line). Spawning it sets WeaponOwner and MinDmg / MaxDmg from FindStat
//!   (FirearmDamage; FindStat hands back the stat's MaxValue as MinValue and the other way round,
//!   which RandomFloatInRange doesn't mind).
//! - **Flight**: ProjectileMovement at InitialSpeed (= MaxSpeed) with ProjectileGravityScale x
//!   the world's gravity, rotation following the velocity (BP_ProjectileMaster). items.json
//!   `firearm.projectile`: the grenade 75 m/s at 0.3, the rocket 80 m/s at 0.1, the flare 30 m/s
//!   at 0.1. BeginPlay's SetLifeSpan(10): a grenade or rocket that meets nothing in 10 s is gone.
//! - **BP_ProjectileGrenade** (40 mm, the M32): its StaticMesh's OnComponentHit (anything but its
//!   owner) fires RadialForce, FX_Explosion_New at a random yaw and Random Vector In Range(1, 2)
//!   scale, explosion_large_06_Cue, a 50 m ReportNoiseEvent, the M_Burn decal, CameraShake_Explosion
//!   within its 20 m CameraShakeSphere, then SphereOverlapActors(7.5 m): BPI_Damage objects take
//!   Damage_Object(RandomFloatInRange(MinDmg, MaxDmg)); living AI, the Player-tagged survivor and
//!   anything with a DamageComponent take ApplyDamage(SkillCheck(RandomFloatInRange(MinDmg,
//!   MaxDmg))) (Skill_DemolitionExpert: x1.5) with a white SpawnCombatText; then it's destroyed.
//! - **BP_ProjectileRocket** (66 mm): the same with its own MinDamage / MaxDamage (250-350: the
//!   launcher's FirearmDamage, set as MinDmg / MaxDmg, isn't read), FX_ExplosionLarge_New,
//!   explosion_large_07_Cue, the burn decal 500 x 1000 and a 12.5 m sphere; objects get
//!   Damage_Object(MinDamage).
//! - **BP_FlareRoundProjectile**: OnComponentHit does nothing, so it stops where it hits (the
//!   movement component doesn't bounce) and stays: no lifespan. Its PointLight (FF8C8C,
//!   Intensity 4, AttenuationRadius 40 m, no inverse-square falloff) glows the whole time.
//!
//! Ours: the muzzle is `combat`'s fixed offset from the survivor (no Muzzle socket); the flight
//! hits the landscape, props and zombies' shot shapes (`combat::ray_hits`), not the survivor;
//! the flare's exponent-falloff intensity has no candela equivalent: 4 x 25 cd (matched at 5 m, a
//! guess), as `level::lights`. The blast flash is the throwables' light. Not ported: RadialForce,
//! the burn decal, the camera shake, the flare's light on other surfaces' shadows.

use std::f32::consts::TAU;

use bevy::prelude::*;
use rand::Rng;

use super::combat::{Dead, Health};
use super::damage_numbers::DamageNumber;
use super::items::{ItemDb, ProjectileDef};
use super::particles::Fx;
use super::throwables::{Flash, demolition, hurt_around, random_scale};
use super::zombies::Noise;
use crate::level::{KinematicBody, LevelZombie, Terrain};
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{CharacterModel, UpperAction};

/// UE's default gravity, m/s².
const GRAVITY: f32 = 9.8;
/// ProjectileTraceFromCamera's line: forward x 1,000,000 cm.
pub const AIM_TRACE: f32 = 10_000.0;
/// BeginPlay's SetLifeSpan(10.0) on the grenade and the rocket.
const LIFESPAN: f32 = 10.0;
/// The blasts' ReportNoiseEvent MaxRange (5000).
const NOISE: f32 = 50.0;
/// The flare's light: candela per unit of its exponent-falloff Intensity (ours, see above).
const FLARE_CANDELA_PER_UNIT: f32 = 25.0;

/// What a projectile class does when it hits something.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blast {
    /// SphereOverlapActors' radius, m.
    pub radius: f32,
    pub cue: &'static str,
    pub fx: &'static str,
}

/// The class's OnComponentHit: a blast, or nothing (the flare).
pub fn blast(class: &str) -> Option<Blast> {
    match class {
        "BP_ProjectileGrenade_C" => Some(Blast { radius: 7.5, cue: "explosion_large_06_Cue", fx: "FX_Explosion_New" }),
        "BP_ProjectileRocket_C" => Some(Blast { radius: 12.5, cue: "explosion_large_07_Cue", fx: "FX_ExplosionLarge_New" }),
        _ => None,
    }
}

/// The ApplyDamage range: the class's own MinDamage / MaxDamage, else the launcher's MinDmg /
/// MaxDmg (its FirearmDamage; ours rolls one value per gun).
pub fn damage_range(def: &ProjectileDef, firearm_damage: f32) -> (f32, f32) {
    def.damage.map_or((firearm_damage, firearm_damage), |[lo, hi]| (lo, hi))
}

/// Damage_Object on the objects in the blast: the rocket passes its MinDamage, the grenade a roll
/// of MinDmg-MaxDmg (neither through SkillCheck).
pub fn object_damage(def: &ProjectileDef, (lo, hi): (f32, f32), rng: &mut impl Rng) -> f32 {
    match def.damage {
        Some([min, _]) => min,
        None => rng.gen_range(lo..=hi),
    }
}

/// ProjectileMovement's step: gravity on the velocity, capped at MaxSpeed (= InitialSpeed).
pub fn step_velocity(v: Vec3, def: &ProjectileDef, dt: f32) -> Vec3 {
    let v = v - Vec3::Y * GRAVITY * def.gravity * dt;
    if def.speed > 0.0 { v.clamp_length_max(def.speed) } else { v }
}

/// ProjectileTraceFromCamera: from the muzzle towards what the camera's line meets (`hit`, m
/// along it), else towards its far end.
pub fn aim_dir(muzzle: Vec3, cam: Vec3, forward: Vec3, hit: Option<f32>) -> Vec3 {
    let target = cam + forward * hit.unwrap_or(AIM_TRACE);
    (target - muzzle).normalize_or(forward)
}

/// Where a step of `len` m along `dir` first goes under the ground: sampled every 0.25 m and
/// at its end (`combat::terrain_hit` starts 0.5 m out, past a slow flare's whole step).
pub fn ground_hit(blocks: impl Fn(Vec3) -> bool, from: Vec3, dir: Vec3, len: f32) -> Option<f32> {
    let n = (len / 0.25).ceil().max(1.0) as usize;
    (1..=n).map(|i| len * i as f32 / n as f32).find(|&d| blocks(from + dir * d))
}

/// The mesh's UE +X along the flight.
fn facing(dir: Vec3) -> Quat {
    Quat::from_rotation_arc(Vec3::X, dir.normalize_or(Vec3::X))
}

/// A shot from a launcher (`combat::fire`).
#[derive(Message, Clone, Debug)]
pub struct Launch {
    pub def: ProjectileDef,
    pub from: Vec3,
    pub dir: Vec3,
    /// MinDmg / MaxDmg as the launcher set them.
    pub damage: (f32, f32),
}

/// A projectile in flight, or a flare where it stopped.
#[derive(Component)]
pub struct Projectile {
    pub def: ProjectileDef,
    pub velocity: Vec3,
    pub damage: (f32, f32),
    pub age: f32,
    /// Stopped against something (the flare).
    pub stopped: bool,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Launch>().add_systems(
        Update,
        (launch, fly).chain().after(super::combat::CombatSet).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)),
    );
}

fn launch(mut commands: Commands, assets: Res<AssetServer>, mut launches: MessageReader<Launch>) {
    for l in launches.read() {
        let mut e = commands.spawn((
            Transform::from_translation(l.from).with_rotation(facing(l.dir)),
            Visibility::default(),
            Projectile { def: l.def.clone(), velocity: l.dir * l.def.speed, damage: l.damage, age: 0.0, stopped: false },
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(m) = &l.def.mesh {
            e.with_child(crate::models::model(&assets, format!("game/{m}.glb")));
        }
        if let Some(light) = &l.def.light {
            let candela = if light.inverse_square { light.intensity / 625.0 } else { light.intensity * FLARE_CANDELA_PER_UNIT };
            let [r, g, b] = light.color;
            e.with_child(PointLight { color: Color::srgb(r, g, b), intensity: crate::level::lights::lumens(candela), range: light.radius, ..default() });
        }
        info!("projectile: {} launched from {} along {} at {:.0} m/s", l.def.class, l.from, l.dir, l.def.speed);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    db: Res<ItemDb>,
    store: Res<SettingsStore>,
    terrain: Option<Res<Terrain>>,
    mut props: crate::level::collision::PropRays,
    mut game: ResMut<CurrentGame>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile), (Without<LevelZombie>, Without<CharacterModel>)>,
    shapes: Query<(&Transform, &LevelZombie), Without<Dead>>,
    mut zombies: Query<(&Transform, &KinematicBody, &mut Health, Option<&mut UpperAction>), (With<LevelZombie>, Without<Dead>)>,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    (mut noise, mut numbers, mut sound, mut fx, mut objects): (MessageWriter<Noise>, MessageWriter<DamageNumber>, MessageWriter<crate::audio::Sound>, MessageWriter<Fx>, MessageWriter<super::barrels::ObjectBlast>),
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    for (e, mut t, mut p) in &mut projectiles {
        p.age += dt;
        let kind = blast(&p.def.class);
        if p.stopped {
            continue;
        }
        if kind.is_some() && p.age >= LIFESPAN {
            info!("projectile: {} expired after {LIFESPAN} s", p.def.class);
            commands.entity(e).despawn();
            continue;
        }
        p.velocity = step_velocity(p.velocity, &p.def, dt);
        let step = p.velocity * dt;
        let (len, dir) = (step.length(), step.normalize_or_zero());
        if len <= 0.0 {
            continue;
        }
        let from = t.translation;
        // OnComponentHit: the first of the landscape, a prop or a zombie along this step.
        let ground = terrain.as_deref().and_then(|tr| ground_hit(|p| tr.blocks(p), from, dir, len)).unwrap_or(f32::MAX);
        let wall = props.cast(from, dir, len).unwrap_or(f32::MAX);
        let body = shapes.iter().filter_map(|(zt, z)| super::combat::ray_hits(from, dir, zt, z.kind).map(|h| h.0)).filter(|&d| d <= len).reduce(f32::min).unwrap_or(f32::MAX);
        let hit = ground.min(wall).min(body);
        if hit > len {
            t.translation += step;
            t.rotation = facing(p.velocity);
            continue;
        }
        let at = from + dir * hit;
        t.translation = at;
        let Some(b) = kind else {
            // The flare stops where it hit and keeps burning.
            p.stopped = true;
            p.velocity = Vec3::ZERO;
            info!("projectile: {} stopped at {at} after {:.2} s", p.def.class, p.age);
            continue;
        };
        // RadialForce, the emitter, the cue, the noise, then the overlap.
        fx.write(Fx::at(b.fx, at).yawed(rng.gen_range(0.0..TAU)).scaled(random_scale(1.0, 2.0, &mut rng)));
        sound.write(crate::audio::Sound::at(b.cue, at));
        noise.write(Noise { at, range: NOISE });
        objects.write(super::barrels::ObjectBlast { at, radius: b.radius, damage: object_damage(&p.def, p.damage, &mut rng) });
        let demo = demolition(game.state.as_ref());
        let (lo, hi) = p.damage;
        let hurt = hurt_around(at, b.radius, (lo * demo, hi * demo), &mut zombies, &player, &mut game, &db, &store);
        numbers.write_batch(hurt.iter().map(|&(damage, at)| DamageNumber::blast(damage, at)));
        commands.spawn((
            Transform::from_translation(at + Vec3::Y * 1.0),
            PointLight { color: Color::srgb(1.0, 0.75, 0.4), intensity: 4_000_000.0, range: b.radius * 2.5, ..default() },
            Flash(0.35),
            DespawnOnExit(AppState::InGame),
        ));
        commands.entity(e).despawn();
        info!(
            "projectile: {} exploded at {at} after {:.2} s ({}): {} zombies in {} m, damage {:?}",
            p.def.class,
            p.age,
            if body <= hit { "zombie" } else if wall <= hit { "prop" } else { "ground" },
            hurt.len(),
            b.radius,
            hurt.iter().map(|d| d.0.round()).collect::<Vec<_>>()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(class: &str, speed: f32, gravity: f32, damage: Option<[f32; 2]>) -> ProjectileDef {
        ProjectileDef { class: class.into(), speed, gravity, mesh: None, damage, light: None }
    }

    #[test]
    fn only_the_grenade_and_rocket_explode() {
        assert_eq!(blast("BP_ProjectileGrenade_C").map(|b| b.radius), Some(7.5));
        assert_eq!(blast("BP_ProjectileRocket_C").map(|b| (b.radius, b.cue)), Some((12.5, "explosion_large_07_Cue")));
        assert_eq!(blast("BP_FlareRoundProjectile_C"), None);
    }

    #[test]
    fn the_rocket_keeps_its_own_damage() {
        let rocket = def("BP_ProjectileRocket_C", 80.0, 0.1, Some([250.0, 350.0]));
        let grenade = def("BP_ProjectileGrenade_C", 75.0, 0.3, None);
        assert_eq!(damage_range(&rocket, 450.0), (250.0, 350.0));
        assert_eq!(damage_range(&grenade, 180.0), (180.0, 180.0));
        let mut rng = rand::thread_rng();
        assert_eq!(object_damage(&rocket, (250.0, 350.0), &mut rng), 250.0);
        assert_eq!(object_damage(&grenade, (180.0, 180.0), &mut rng), 180.0);
    }

    #[test]
    fn gravity_scales_and_speed_caps() {
        let grenade = def("BP_ProjectileGrenade_C", 75.0, 0.3, None);
        let v = step_velocity(Vec3::new(75.0, 0.0, 0.0), &grenade, 1.0);
        assert!(v.length() <= 75.0 + 1e-3);
        // Straight down it just falls faster, to the cap.
        let v = step_velocity(Vec3::new(0.0, -10.0, 0.0), &grenade, 1.0);
        assert!((v.y - (-10.0 - 9.8 * 0.3)).abs() < 1e-4);
        // A 75 m/s grenade fired level drops about 0.5 g t² over 1 s: 1.47 m.
        let (mut p, mut v) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 75.0));
        for _ in 0..100 {
            v = step_velocity(v, &grenade, 0.01);
            p += v * 0.01;
        }
        assert!((p.y + 1.47).abs() < 0.05, "{p}");
    }

    #[test]
    fn aims_from_the_muzzle_at_the_camera_hit() {
        let d = aim_dir(Vec3::new(0.5, 1.4, 0.0), Vec3::new(0.0, 1.7, -3.0), Vec3::Z, Some(23.0));
        let target = Vec3::new(0.0, 1.7, 20.0);
        assert!(d.dot((target - Vec3::new(0.5, 1.4, 0.0)).normalize()) > 0.9999);
        // Nothing hit: the line's far end, practically the camera's forward.
        assert!(aim_dir(Vec3::ZERO, Vec3::new(0.0, 0.0, -3.0), Vec3::Z, None).dot(Vec3::Z) > 0.99999);
    }

    #[test]
    fn short_steps_still_meet_the_ground() {
        // A flare's 0.5 m frame step ending 2 cm under flat ground at y = 0.
        let under = |p: Vec3| p.y < 0.0;
        let d = ground_hit(under, Vec3::new(0.0, 0.17, 0.0), Vec3::new(0.94, -0.35, 0.0), 0.5).expect("hit");
        assert!(d > 0.25 && d <= 0.5, "{d}");
        assert_eq!(ground_hit(under, Vec3::new(0.0, 5.0, 0.0), Vec3::X, 0.5), None);
    }

    #[test]
    fn launchers_have_their_projectiles() {
        let db = ItemDb::load();
        let p = |id: &str| db.items[id].firearm.as_ref().and_then(|f| f.projectile.clone()).unwrap_or_else(|| panic!("{id} has no projectile"));
        assert_eq!(p("DA_GrenadeLauncher").class, "BP_ProjectileGrenade_C");
        assert_eq!((p("DA_RocketLauncher").speed, p("DA_RocketLauncher").damage), (80.0, Some([250.0, 350.0])));
        let flare = p("DA_FlareGun");
        assert_eq!(flare.gravity, 0.1);
        assert!(flare.light.as_ref().is_some_and(|l| l.radius == 40.0 && !l.inverse_square));
        assert!(db.items["DA_M4CQB"].firearm.as_ref().is_some_and(|f| f.projectile.is_none()));
    }
}
