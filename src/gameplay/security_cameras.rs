//! Security cameras: BP_SecurityCamera_AI (5 in the level), a SmartAI BP_MasterCamera (itself a
//! BP_MasterTurret) with a BP_TurretAIComponent, decompiled.
//!
//! - **Idle**: the Rotate Camera Timeline (12 s, looping, autoplay) turns the head (Turret) to
//!   RInterpTo(RLerp((pitch -20, yaw FOV Left), (pitch -20, yaw -FOV Right), Rotate Alpha), 3);
//!   FOV Left / Right are BP_MasterTurret's 90. Rotate Alpha is a cubic curve: 0 until 0.5 s,
//!   up to 1 at 5.5 s, held to 6.5 s, back to 0 at 11.5 s.
//! - **Perception**: the component's Reaction Tags are empty on the class and on every placed
//!   camera, so Perception Check returns at its first test: in this build the cameras never see
//!   anything, never stop sweeping and never alert the bandits (Alert AI / Alert Tags Bandit /
//!   Alert Radius 40 m are set but unreachable). Nothing is ported for them.
//! - **Damage**: Damage_Object (BP_MasterTurret: the survivor's rounds and swings) -> Server
//!   Turret Damaged -> Deduct Health: Starting Health 25 (an int; FTrunc of each hit); at 0, Dead
//!   and Turret Destroyed Effect: FX_Spark_Impact at the Weapon Muzzle (pitch -90, scale 1),
//!   then K2_DestroyActor (the base, head and light-ray cube gone for the session).
//!
//! The head is taken out of the static props and drawn turning (`Level::prop_near`).
//! Ours: the head's hit sphere (0.3 m); swings hit within 2 m in front, as the turrets'. Not
//! ported: hiding past the AIOSubject's 100 m layer, ReceiveAnyDamage's half-size sparks (the
//! survivor's weapons send Damage_Object, not ApplyDamage).

use bevy::prelude::*;

use crate::menus::AppState;

const CLASS: &str = "BP_SecurityCamera_AI_C";
/// Turret (the head) on Turret Base: (20.91, -0.21, 13.67) cm, UE (X, Y, Z) -> our (X, Z, Y).
const HEAD: Vec3 = Vec3::new(0.2091, 0.1367, -0.0021);
/// Weapon Muzzle on the head: (11.97, 0, 8.18) cm.
const MUZZLE: Vec3 = Vec3::new(0.1197, 0.0818, 0.0);
const HEAD_MESH: &str = "SM_Prop_Security_Camera_Head_01";
const BASE_MESH: &str = "SM_Prop_Security_Camera_01";
const RAY_MESH: &str = "SM_LightRay_Cube";
const STARTING_HEALTH: i32 = 25;
const FOV: f32 = 90.0;
const PITCH: f32 = -20.0;
const INTERP: f32 = 3.0;
const TIMELINE: f32 = 12.0;
/// Rotate Alpha's keys: (time, value, arrive tangent, leave tangent).
const ALPHA: [(f32, f32, f32, f32); 6] = [(0.0, 0.0, 0.0, 0.0), (0.5, 0.0, 0.1818, 0.1818), (5.5, 1.0, 0.1667, 0.1667), (6.5, 1.0, -0.1667, -0.1667), (11.5, 0.0, -0.1818, -0.1818), (12.0, 0.0, 0.0, 0.0)];
const HIT_RADIUS: f32 = 0.3;

/// Rotate Alpha at `t` s (UE's cubic keys: Hermite with the keys' tangents).
pub fn rotate_alpha(t: f32) -> f32 {
    let t = t.rem_euclid(TIMELINE);
    let i = ALPHA.iter().position(|k| k.0 > t).unwrap_or(ALPHA.len() - 1).max(1);
    let (k0, k1) = (ALPHA[i - 1], ALPHA[i]);
    let dt = k1.0 - k0.0;
    let s = ((t - k0.0) / dt).clamp(0.0, 1.0);
    let (s2, s3) = (s * s, s * s * s);
    (2.0 * s3 - 3.0 * s2 + 1.0) * k0.1 + (s3 - 2.0 * s2 + s) * k0.3 * dt + (-2.0 * s3 + 3.0 * s2) * k1.1 + (s3 - s2) * k1.2 * dt
}

/// The sweep's target yaw at `t` s: RLerp(FOV Left, -FOV Right, alpha) (no shortest path).
pub fn sweep_yaw(t: f32) -> f32 {
    FOV + (-FOV - FOV) * rotate_alpha(t)
}

pub struct SecurityCamera {
    pub actor: usize,
    /// The actor's transform and the head's pivot on it.
    pub transform: Transform,
    pub pivot: Vec3,
    /// The head's yaw and pitch on the base (UE degrees).
    pub yaw: f32,
    pub pitch: f32,
    time: f32,
    pub health: i32,
    pub dead: bool,
    head: Option<Entity>,
    glb: String,
    /// The base's and light ray's static props, gone with the actor.
    props: Vec<crate::level::PropKey>,
}

impl SecurityCamera {
    pub fn new(actor: usize, transform: Transform) -> Self {
        Self { actor, pivot: transform.transform_point(HEAD), transform, yaw: 0.0, pitch: 0.0, time: 0.0, health: STARTING_HEALTH, dead: false, head: None, glb: String::new(), props: Vec::new() }
    }

    /// The head's rotation in the world (UE yaw turns X toward Y: -yaw about our Y).
    pub fn head_rotation(&self) -> Quat {
        self.transform.rotation * Quat::from_rotation_y(-self.yaw.to_radians()) * Quat::from_rotation_z(self.pitch.to_radians())
    }

    pub fn muzzle(&self) -> Vec3 {
        self.pivot + self.head_rotation() * MUZZLE
    }

    /// The Timeline's update: RInterpTo the head toward the sweep.
    pub fn sweep(&mut self, dt: f32) {
        self.time += dt;
        let k = (dt * INTERP).clamp(0.0, 1.0);
        let dy = (sweep_yaw(self.time) - self.yaw + 540.0).rem_euclid(360.0) - 180.0;
        self.yaw += dy * k;
        self.pitch += (PITCH - self.pitch) * k;
    }

    /// Deduct Health: true when this hit destroyed it.
    pub fn damage(&mut self, amount: f32) -> bool {
        if self.dead {
            return false;
        }
        self.health = (self.health - amount.trunc() as i32).max(0);
        self.dead = self.health <= 0;
        self.dead
    }
}

#[derive(Resource, Default)]
pub struct SecurityCameras {
    pub list: Vec<SecurityCamera>,
    collected: bool,
}

#[derive(Component)]
struct CameraHead(usize);

pub fn plugin(app: &mut App) {
    app.init_resource::<SecurityCameras>()
        .add_systems(OnEnter(AppState::InGame), |mut c: ResMut<SecurityCameras>| *c = SecurityCameras::default())
        .add_systems(Update, (collect, take_damage, sweep).chain().after(super::building::BuildSet).run_if(in_state(AppState::InGame)));
}

fn collect(mut commands: Commands, assets: Res<AssetServer>, level: Option<Res<crate::level::Level>>, streaming: Option<ResMut<crate::level::PropStreaming>>, mut cams: ResMut<SecurityCameras>) {
    let (Some(level), Some(mut streaming)) = (level, streaming) else { return };
    if cams.collected {
        return;
    }
    cams.collected = true;
    for (i, a) in level.actors.iter().enumerate().filter(|(_, a)| a.class == CLASS) {
        let mut c = SecurityCamera::new(i, crate::level::affine(&a.transform));
        let Some((key, at, glb)) = level.prop_near(c.pivot, HEAD_MESH, 0.5) else {
            warn!("security camera {i}: no head mesh at {:.1}", c.pivot);
            continue;
        };
        if let Some(cell) = streaming.hide(key) {
            commands.entity(cell).despawn();
        }
        c.pivot = at.translation;
        c.glb = glb;
        c.props = [BASE_MESH, RAY_MESH].iter().filter_map(|m| level.prop_near(c.pivot, m, 1.0).map(|p| p.0)).collect();
        let n = cams.list.len();
        c.head = Some(commands.spawn((crate::models::model(&assets, c.glb.clone()), Transform::from_translation(c.pivot).with_rotation(c.head_rotation()), CameraHead(n), DespawnOnExit(AppState::InGame))).id());
        cams.list.push(c);
    }
    info!("security cameras: {}", cams.list.len());
}

/// Damage_Object from the survivor's rounds and swings.
fn take_damage(
    mut commands: Commands,
    mut cams: ResMut<SecurityCameras>,
    streaming: Option<ResMut<crate::level::PropStreaming>>,
    (mut shots, mut swings): (MessageReader<super::turrets::Shot>, MessageReader<super::harvest::Swing>),
    mut fx: MessageWriter<super::particles::Fx>,
) {
    let mut hits: Vec<(usize, f32)> = Vec::new();
    for s in shots.read() {
        for (i, c) in cams.list.iter().enumerate().filter(|(_, c)| !c.dead) {
            let along = (c.pivot - s.origin).dot(s.dir);
            if along > 0.0 && along <= s.reach + HIT_RADIUS && (s.origin + s.dir * along).distance(c.pivot) <= HIT_RADIUS {
                hits.push((i, s.damage));
            }
        }
    }
    for s in swings.read() {
        for (i, c) in cams.list.iter().enumerate().filter(|(_, c)| !c.dead) {
            let flat = (c.pivot - s.feet).with_y(0.0);
            if flat.length() <= 2.0 && flat.normalize_or_zero().dot(s.facing) > 0.5 {
                hits.push((i, s.damage));
            }
        }
    }
    let mut streaming = streaming;
    for (i, dmg) in hits {
        let Some(c) = cams.list.get_mut(i) else { continue };
        if !c.damage(dmg) {
            info!("security camera {i} hit for {dmg:.0}: {} left", c.health);
            continue;
        }
        // Turret Destroyed Effect: sparks at the lens, then the actor is gone.
        fx.write(super::particles::Fx::at("FX_Spark_Impact", c.muzzle()).pointing(Vec3::NEG_Y));
        if let Some(h) = c.head.take() {
            commands.entity(h).despawn();
        }
        if let Some(s) = streaming.as_mut() {
            for &k in &c.props {
                if let Some(cell) = s.hide(k) {
                    commands.entity(cell).despawn();
                }
            }
        }
        info!("security camera {i} destroyed ({} props gone)", c.props.len());
    }
}

fn sweep(time: Res<Time>, mut cams: ResMut<SecurityCameras>, mut heads: Query<(&CameraHead, &mut Transform)>) {
    let dt = time.delta_secs();
    for c in cams.list.iter_mut().filter(|c| !c.dead) {
        c.sweep(dt);
    }
    for (h, mut t) in &mut heads {
        if let Some(c) = cams.list.get(h.0) {
            t.rotation = c.head_rotation();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sweep_follows_rotate_alpha() {
        // Held at FOV Left, across to -FOV Right by 5.5 s, held, back by 11.5 s; it loops.
        // (The stored tangents dip the curve a hair below 0 before 0.5 s, as in UE.)
        assert!((sweep_yaw(0.25) - 90.0).abs() < 3.0);
        assert_eq!(sweep_yaw(0.0), 90.0);
        assert!((rotate_alpha(5.5) - 1.0).abs() < 1e-5 && (rotate_alpha(6.0) - 1.0).abs() < 0.05);
        assert!((sweep_yaw(3.0) - 0.0).abs() < 10.0, "halfway {}", sweep_yaw(3.0));
        assert_eq!(sweep_yaw(12.25), sweep_yaw(0.25), "it loops");
        // The head eases there at RInterpTo 3, tipped 20 degrees down.
        let mut c = SecurityCamera::new(0, Transform::IDENTITY);
        for _ in 0..360 {
            c.sweep(1.0 / 60.0);
        }
        assert!((c.pitch - PITCH).abs() < 0.1 && c.yaw < -80.0, "{} {}", c.yaw, c.pitch);
    }

    #[test]
    fn twenty_five_health_in_whole_points() {
        let mut c = SecurityCamera::new(0, Transform::IDENTITY);
        assert!(!c.damage(14.9));
        assert_eq!(c.health, 11);
        assert!(c.damage(11.0) && c.dead);
        assert!(!c.damage(5.0), "already destroyed");
    }

    #[test]
    fn the_head_sits_on_the_base() {
        // A camera turned 90 degrees (UE yaw 90: X toward Y): the head 21 cm along UE Y.
        let t = Transform::from_rotation(Quat::from_rotation_y(-90f32.to_radians()));
        let c = SecurityCamera::new(0, t);
        assert!(c.pivot.distance(Vec3::new(0.0021, 0.1367, 0.2091)) < 1e-3, "{}", c.pivot);
    }
}
