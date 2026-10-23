//! Particle effects: the game's Cascade particle systems (UParticleSystem), simulated on the CPU.
//! Data: `assets/data/particles.json` (`extractor/gen_particles.py`): each emitter's Required,
//! Spawn and particle modules with their baked lookup tables (FRawDistribution), in Bevy units.
//!
//! What runs, per emitter (FParticleEmitterInstance's tick):
//! - Emitter time loops over EmitterDuration, EmitterLoops times (0 = for ever). Spawn's Rate x
//!   RateScale per second and its BurstList (times are fractions of the duration) make particles;
//!   a deactivated or finished emitter stops spawning and its particles live out their lives.
//! - On spawn: Lifetime, Size (added to a zero base), Velocity (+ radial), VelocityCone,
//!   LocationPrimitiveSphere / Cylinder, Location, Color, ColorOverLife's start, Rotation,
//!   RotationRate, MeshRotation, Acceleration. Offsets and velocities go through the component's
//!   transform, and the component's scale scales the particles too, so a fire scaled to 0.25 on a
//!   barrel is a quarter-size fire (a guess at EmitterToSimulation including the scale).
//! - Each frame, from the base values: ColorOverLife, ColorScaleOverLife, SizeMultiplyLife,
//!   SizeScaleBySpeed, AccelerationDrag, Acceleration (Constant, OverLifetime),
//!   VelocityOverLifetime; then the particle moves and spins.
//!
//! Drawing: mesh particles draw their mesh, sprites a camera-facing quad. The cooked materials
//! keep no expression graph, so (a guess) a particle draws its colour, unlit or lit as its
//! material is. The meshes' vertex colours are Synty mask channels, not colour (the smoke
//! puff's are all 0, the fire sphere's pure red), so they're stripped. Translucent ones blend by the particle's alpha over
//! the emitter's peak alpha (FX_Fire_AI holds 0.03 and the wreck smoke 0 throughout, so the raw
//! alpha can't be the opacity); masked ones (Mat_FX) show while that ratio is at least 1/3.
//!
//! Where effects come from: the level's emitters ([`LEVEL_FILE`]: barrel fires, campfires,
//! cooking stations, wreck smoke, sparking lights), on while the survivor is within the owner's
//! first AIOSubject optimisation layer; and [`Fx`] messages from gameplay (blasts, the Molotov,
//! burning, bullet impacts, muzzle flashes, broken rocks).
//!
//! Not ported: Light modules (blasts and fires keep their stand-in lights), Collision,
//! ParameterDynamic, CameraOffset, SubUV flipbooks (a sprite draws its whole texture),
//! PSA_Velocity alignment (sprites face the camera), Niagara systems (NS_BloodSplatter,
//! NS_InfestationDust, the bullet trails).

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::math::Affine3A;
use bevy::prelude::*;
use rand::Rng;
use serde::Deserialize;

use crate::menus::{AppState, MenuScreen};

/// The level's emitters, resolved by gen_particles.py.
pub const LEVEL_FILE: &str = "particles.json";
/// A cap on live particles per emitter (MaxDrawCount is 0 or 1000 in the data).
const MAX_PARTICLES: usize = 400;

// ---- Data -------------------------------------------------------------------------------------

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    #[default]
    None,
    Random,
    Extreme,
}

/// A baked FRawDistribution: `n` entries over time ((t - bias) x scale indexes them), each one
/// value (op none) or a min and a max to pick between.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Dist {
    #[serde(default)]
    bias: f32,
    #[serde(default)]
    scale: f32,
    #[serde(default = "one_usize")]
    n: usize,
    #[serde(default = "one_usize")]
    dims: usize,
    #[serde(default)]
    op: Op,
    /// EDistributionVectorLockFlags in Bevy axes: 1 XY, 2 XZ, 3 YZ, 4 XYZ.
    #[serde(default)]
    lock: u8,
    v: Vec<f32>,
}

fn one_usize() -> usize {
    1
}

impl Dist {
    pub fn constant(v: &[f32]) -> Self {
        Self { n: 1, dims: v.len(), v: v.to_vec(), ..default() }
    }

    fn stride(&self) -> usize {
        self.dims * if self.op == Op::None { 1 } else { 2 }
    }

    /// The two entries around `t` and the blend between them.
    fn entries(&self, t: f32) -> (usize, usize, f32) {
        if self.n <= 1 {
            return (0, 0, 0.0);
        }
        let x = ((t - self.bias) * self.scale).max(0.0);
        let i = (x.floor() as usize).min(self.n - 1);
        (i, (i + 1).min(self.n - 1), (x - i as f32).clamp(0.0, 1.0))
    }

    fn value(&self, entry: usize, axis: usize, f: f32) -> f32 {
        let base = entry * self.stride();
        let lo = self.v.get(base + axis).copied().unwrap_or(0.0);
        if self.op == Op::None {
            return lo;
        }
        let hi = self.v.get(base + self.dims + axis).copied().unwrap_or(lo);
        lo + (hi - lo) * f
    }

    /// The random fraction per axis (FRawDistribution::GetValue's Fraction, then its lock).
    fn fractions(&self, rng: &mut impl Rng) -> [f32; 3] {
        let mut f = [0.0; 3];
        if self.op == Op::None {
            return f;
        }
        for x in &mut f {
            *x = rng.r#gen::<f32>();
            if self.op == Op::Extreme {
                *x = if *x > 0.5 { 1.0 } else { 0.0 };
            }
        }
        match self.lock {
            1 => f[1] = f[0],
            2 => f[2] = f[0],
            3 => f[2] = f[1],
            4 => f = [f[0]; 3],
            _ => {}
        }
        f
    }

    pub fn scalar(&self, t: f32, rng: &mut impl Rng) -> f32 {
        let (a, b, w) = self.entries(t);
        let f = self.fractions(rng)[0];
        let (x, y) = (self.value(a, 0, f), self.value(b, 0, f));
        x + (y - x) * w
    }

    pub fn vector(&self, t: f32, rng: &mut impl Rng) -> Vec3 {
        if self.dims < 3 {
            return Vec3::splat(self.scalar(t, rng));
        }
        let (a, b, w) = self.entries(t);
        let f = self.fractions(rng);
        Vec3::from_array(std::array::from_fn(|i| {
            let (x, y) = (self.value(a, i, f[i]), self.value(b, i, f[i]));
            x + (y - x) * w
        }))
    }

    /// The largest value anywhere in the table (an alpha curve's peak).
    fn peak(&self) -> f32 {
        self.v.iter().copied().fold(0.0, f32::max)
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Cone {
    angle: Dist,
    velocity: Dist,
    direction: [f32; 3],
    #[serde(default)]
    world: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Sphere {
    radius: Dist,
    offset: Dist,
    #[serde(default)]
    surface: bool,
    #[serde(default)]
    velocity: bool,
    velocity_scale: Dist,
    pos: [bool; 3],
    neg: [bool; 3],
}

#[derive(Deserialize, Clone, Debug)]
pub struct Cylinder {
    radius: Dist,
    height: Dist,
    offset: Dist,
    #[serde(default)]
    surface: bool,
    #[serde(default)]
    velocity: bool,
    velocity_scale: Dist,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ColorScale {
    color: Dist,
    alpha: Dist,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SizeLife {
    v: Dist,
    axes: [bool; 3],
}

#[derive(Deserialize, Clone, Debug)]
pub struct SizeBySpeed {
    scale: [f32; 2],
    max: [f32; 2],
}

#[derive(Deserialize, Clone, Debug)]
pub struct VelocityLife {
    v: Dist,
    #[serde(default)]
    absolute: bool,
}

/// One emitter (a UParticleSpriteEmitter's first LOD).
#[derive(Deserialize, Clone, Debug)]
pub struct EmitterDef {
    #[serde(default)]
    pub name: String,
    pub material: Option<String>,
    #[serde(default = "one")]
    pub duration: f32,
    #[serde(default)]
    pub loops: u32,
    #[serde(default)]
    pub local: bool,
    rate: Dist,
    rate_scale: Dist,
    #[serde(default)]
    bursts: Vec<(i32, i32, f32)>,
    burst_scale: Dist,
    #[serde(default)]
    pub mesh: Option<String>,
    #[serde(default)]
    pub sprite: bool,
    lifetime: Dist,
    size: Option<Dist>,
    velocity: Option<Dist>,
    velocity_radial: Option<Dist>,
    #[serde(default)]
    velocity_world: bool,
    cone: Option<Cone>,
    sphere: Option<Sphere>,
    cylinder: Option<Cylinder>,
    location: Option<Dist>,
    color: Option<Dist>,
    alpha: Option<Dist>,
    color_life: Option<Dist>,
    alpha_life: Option<Dist>,
    #[serde(default)]
    color_scale_life: Vec<ColorScale>,
    #[serde(default)]
    size_life: Vec<SizeLife>,
    size_by_speed: Option<SizeBySpeed>,
    drag: Option<Dist>,
    #[serde(default)]
    accel: Vec<Dist>,
    accel_constant: Option<[f32; 3]>,
    accel_life: Option<Dist>,
    velocity_life: Option<VelocityLife>,
    rotation: Option<Dist>,
    rotation_rate: Option<Dist>,
    mesh_rotation: Option<Dist>,
    /// The alpha that draws fully opaque (see the module docs); 0 = alpha is ignored.
    #[serde(skip)]
    peak_alpha: f32,
}

fn one() -> f32 {
    1.0
}

impl EmitterDef {
    /// How opaque a particle of colour alpha `a` draws.
    pub fn opacity(&self, a: f32) -> f32 {
        if self.peak_alpha <= 1e-4 { 1.0 } else { (a / self.peak_alpha).clamp(0.0, 1.0) }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct SystemDef {
    pub emitters: Vec<EmitterDef>,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Blend {
    #[default]
    Opaque,
    Masked,
    Translucent,
    Additive,
    Modulate,
}

#[derive(Deserialize, Clone, Copy, Debug, Default)]
pub struct MaterialInfo {
    #[serde(default)]
    pub blend: Blend,
    #[serde(default)]
    pub lit: bool,
}

#[derive(Deserialize, Default)]
pub struct FxData {
    pub systems: HashMap<String, SystemDef>,
    #[serde(default)]
    pub materials: HashMap<String, MaterialInfo>,
}

/// Every particle system, by asset name (FX_Fire, FX_Explosion_New...).
pub fn fx_data() -> &'static FxData {
    static DATA: std::sync::OnceLock<FxData> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let mut d: FxData = std::fs::read_to_string("assets/data/particles.json").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        for s in d.systems.values_mut() {
            for e in &mut s.emitters {
                e.peak_alpha = e.alpha_life.as_ref().or(e.alpha.as_ref()).map_or(0.0, Dist::peak);
            }
        }
        d
    })
}

// ---- Simulation -------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct Particle {
    /// World space, or the emitter's space for local emitters.
    pub pos: Vec3,
    base_vel: Vec3,
    pub vel: Vec3,
    base_size: Vec3,
    pub size: Vec3,
    pub rot: f32,
    base_rot_rate: f32,
    rot_rate: f32,
    pub mesh_rot: Quat,
    accel: Vec3,
    base_color: Vec4,
    pub color: Vec4,
    /// Relative time, 0 at birth, 1 at death.
    pub t: f32,
    inv_life: f32,
}

/// One emitter's instance: its clock and its particles.
#[derive(Clone, Debug, Default)]
pub struct EmitterState {
    time: f32,
    loops: u32,
    carry: f32,
    fired: Vec<bool>,
    pub particles: Vec<Particle>,
    pub finished: bool,
    views: Vec<Entity>,
}

/// A random direction for the location primitives, each axis limited by its Positive_/Negative_
/// flags (DetermineUnitDirection).
fn unit_direction(pos: [bool; 3], neg: [bool; 3], rng: &mut impl Rng) -> Vec3 {
    Vec3::from_array(std::array::from_fn(|i| match (pos[i], neg[i]) {
        (true, true) => rng.gen_range(-1.0..=1.0),
        (true, false) => rng.gen_range(0.0..=1.0),
        (false, true) => rng.gen_range(-1.0..=0.0),
        (false, false) => 0.0,
    }))
}

impl EmitterState {
    fn new(def: &EmitterDef) -> Self {
        Self { fired: vec![false; def.bursts.len()], ..default() }
    }

    /// Advance by `dt`. `xf` maps the emitter's space to the world; `spawning` is false once the
    /// effect is deactivated.
    pub fn tick(&mut self, def: &EmitterDef, dt: f32, xf: &Affine3A, spawning: bool, rng: &mut impl Rng) {
        for p in &mut self.particles {
            update(def, p, dt, rng);
        }
        self.particles.retain(|p| p.t < 1.0);
        if self.finished || !spawning {
            return;
        }
        let et = self.time;
        self.time += dt;
        let mut bursts = 0;
        for (i, &(count, low, at)) in def.bursts.iter().enumerate() {
            if !self.fired[i] && at * def.duration <= self.time {
                self.fired[i] = true;
                let n = if low > -1 { low + ((count - low) as f32 * rng.r#gen::<f32>()).round() as i32 } else { count };
                bursts += (n as f32 * def.burst_scale.scalar(et, rng)).max(0.0) as usize;
            }
        }
        let rate = (def.rate.scalar(et, rng) * def.rate_scale.scalar(et, rng)).max(0.0);
        let want = self.carry + rate * dt;
        let steady = want.floor();
        self.carry = want - steady;
        let steady = steady as usize;
        for i in 0..bursts + steady {
            if self.particles.len() >= MAX_PARTICLES {
                break;
            }
            let mut p = spawn(def, et, xf, rng);
            // Rate particles are spread over the frame, as if born between ticks.
            if i >= bursts {
                let age = dt * ((i - bursts) as f32 + 0.5) / steady as f32;
                p.pos += p.vel * age;
                p.t += age * p.inv_life;
            }
            self.particles.push(p);
        }
        if def.duration > 0.0 && self.time >= def.duration {
            self.loops += 1;
            if def.loops != 0 && self.loops >= def.loops {
                self.finished = true;
            } else {
                self.time -= def.duration;
                self.fired.fill(false);
            }
        }
    }

    /// Nothing more will come from it.
    pub fn spent(&self) -> bool {
        self.finished && self.particles.is_empty()
    }
}

fn spawn(def: &EmitterDef, et: f32, xf: &Affine3A, rng: &mut impl Rng) -> Particle {
    let life = def.lifetime.scalar(et, rng);
    let mut offset = Vec3::ZERO;
    let mut vel = def.velocity.as_ref().map_or(Vec3::ZERO, |v| v.vector(et, rng));
    if let Some(s) = &def.sphere {
        let dir = unit_direction(s.pos, s.neg, rng);
        let r = s.radius.scalar(et, rng);
        let at = if s.surface { dir.normalize_or_zero() * r } else { dir.clamp_length_max(1.0) * r };
        offset += s.offset.vector(et, rng) + at;
        if s.velocity {
            vel += at * s.velocity_scale.scalar(et, rng);
        }
    }
    if let Some(c) = &def.cylinder {
        // The height axis is UE Z, Bevy's Y.
        let r = c.radius.scalar(et, rng);
        let disc = Vec2::new(rng.gen_range(-1.0..=1.0), rng.gen_range(-1.0..=1.0));
        let disc = if c.surface { disc.normalize_or_zero() } else { disc.clamp_length_max(1.0) } * r;
        let at = Vec3::new(disc.x, rng.gen_range(-0.5..=0.5) * c.height.scalar(et, rng), disc.y);
        offset += c.offset.vector(et, rng) + at;
        if c.velocity {
            vel += at * c.velocity_scale.scalar(et, rng);
        }
    }
    if let Some(l) = &def.location {
        offset += l.vector(et, rng);
    }
    // StartVelocityRadial: out from the emitter's origin.
    if let Some(r) = &def.velocity_radial {
        vel += offset.normalize_or_zero() * r.scalar(et, rng);
    }
    let mut cone_vel = Vec3::ZERO;
    if let Some(c) = &def.cone {
        // Angle 0-1 is 0-180 degrees off Direction, at a random lathe angle around it.
        let polar = c.angle.scalar(et, rng) * PI;
        let lathe = rng.gen_range(0.0..TAU);
        let fwd = Vec3::from_array(c.direction).normalize_or(Vec3::Y);
        let d = Quat::from_rotation_arc(Vec3::Y, fwd) * (Quat::from_rotation_y(lathe) * Quat::from_rotation_x(polar) * Vec3::Y);
        cone_vel = d * c.velocity.scalar(et, rng);
    }
    let (pos, vel) = if def.local {
        (offset, vel + cone_vel)
    } else {
        let v = if def.velocity_world { vel } else { xf.transform_vector3(vel) };
        let cv = if def.cone.as_ref().is_some_and(|c| c.world) { cone_vel } else { xf.transform_vector3(cone_vel) };
        (xf.transform_point3(offset), v + cv)
    };
    let mut color = def.color.as_ref().map_or(Vec3::ONE, |c| c.vector(et, rng)).extend(def.alpha.as_ref().map_or(1.0, |a| a.scalar(et, rng)));
    if let Some(c) = &def.color_life {
        color = c.vector(0.0, rng).extend(def.alpha_life.as_ref().map_or(1.0, |a| a.scalar(0.0, rng)));
    }
    let size = def.size.as_ref().map_or(Vec3::ONE, |s| s.vector(et, rng));
    let rot_rate = def.rotation_rate.as_ref().map_or(0.0, |r| r.scalar(et, rng)) * TAU;
    let mesh_rot = def.mesh_rotation.as_ref().map_or(Quat::IDENTITY, |r| {
        let e = r.vector(et, rng) * TAU;
        Quat::from_euler(EulerRot::YXZ, e.y, e.x, e.z)
    });
    let accel = def.accel.iter().map(|a| a.vector(et, rng)).sum();
    Particle {
        pos,
        base_vel: vel,
        vel,
        base_size: size,
        size,
        rot: def.rotation.as_ref().map_or(0.0, |r| r.scalar(et, rng)) * TAU,
        base_rot_rate: rot_rate,
        rot_rate,
        mesh_rot,
        accel,
        base_color: color,
        color,
        t: 0.0,
        inv_life: if life > 0.0 { 1.0 / life } else { 0.0 },
    }
}

fn update(def: &EmitterDef, p: &mut Particle, dt: f32, rng: &mut impl Rng) {
    // ResetParticleParameters, then the modules' Update in order.
    p.vel = p.base_vel;
    p.size = p.base_size;
    p.rot_rate = p.base_rot_rate;
    p.color = p.base_color;
    p.t += dt * p.inv_life;
    let t = p.t.min(1.0);
    if let Some(c) = &def.color_life {
        p.color = c.vector(t, rng).extend(def.alpha_life.as_ref().map_or(1.0, |a| a.scalar(t, rng)));
    }
    for s in &def.color_scale_life {
        p.color *= s.color.vector(t, rng).extend(s.alpha.scalar(t, rng));
    }
    for s in &def.size_life {
        let m = s.v.vector(t, rng);
        for i in 0..3 {
            if s.axes[i] {
                p.size[i] *= m[i];
            }
        }
    }
    if let Some(s) = &def.size_by_speed {
        // SpeedScale is per UE unit (cm/s).
        let speed = p.vel.length() * 100.0;
        p.size.x *= (speed * s.scale[0]).clamp(1.0, s.max[0].max(1.0));
        p.size.y *= (speed * s.scale[1]).clamp(1.0, s.max[1].max(1.0));
    }
    if let Some(d) = &def.drag {
        let k = (d.scalar(t, rng) * dt).min(1.0);
        p.vel -= p.vel * k;
        p.base_vel -= p.base_vel * k;
    }
    let mut a = p.accel + def.accel_constant.map_or(Vec3::ZERO, Vec3::from_array);
    if let Some(l) = &def.accel_life {
        a += l.vector(t, rng);
    }
    p.vel += a * dt;
    p.base_vel += a * dt;
    if let Some(l) = &def.velocity_life {
        let m = l.v.vector(t, rng);
        p.vel = if l.absolute { m } else { p.vel * m };
    }
    p.pos += p.vel * dt;
    p.rot += p.rot_rate * dt;
}

// ---- Components and messages ------------------------------------------------------------------

/// A playing particle system. Its entity's GlobalTransform places it (attach it under another
/// entity to follow it).
#[derive(Component, Debug)]
pub struct Emitter {
    pub system: String,
    pub states: Vec<EmitterState>,
    /// Spawning new particles (Deactivate() clears it; what's out lives on).
    pub active: bool,
    /// Deactivate this many seconds from now.
    pub stop_after: Option<f32>,
    /// Despawn once spent (spawned by an [`Fx`] message).
    pub one_shot: bool,
}

impl Emitter {
    pub fn new(system: &str) -> Self {
        let n = fx_data().systems.get(system).map_or(0, |s| s.emitters.len());
        let states = fx_data().systems.get(system).map(|s| s.emitters.iter().map(EmitterState::new).collect()).unwrap_or_else(|| vec![EmitterState::default(); n]);
        Self { system: system.to_string(), states, active: true, stop_after: None, one_shot: false }
    }

    pub fn live(&self) -> usize {
        self.states.iter().map(|s| s.particles.len()).sum()
    }

    pub fn spent(&self) -> bool {
        self.states.iter().all(EmitterState::spent) || (!self.active && self.live() == 0)
    }

    /// Stop spawning and drop what's out (AIO hiding it; leaving range).
    fn clear(&mut self) {
        for s in &mut self.states {
            s.particles.clear();
        }
    }
}

/// Play a particle system (GameplayStatics.SpawnEmitterAtLocation / SpawnEmitterAttached).
#[derive(Message, Clone, Debug)]
pub struct Fx {
    pub system: String,
    pub at: Transform,
    /// Attached to this entity, `at` relative to it.
    pub parent: Option<Entity>,
    /// Deactivate after this long (the Molotov's fire), else it plays out.
    pub stop_after: Option<f32>,
}

impl Fx {
    pub fn at(system: impl Into<String>, pos: Vec3) -> Self {
        Self { system: system.into(), at: Transform::from_translation(pos), parent: None, stop_after: None }
    }

    /// Its UE X (Bevy X) along `dir`: a socket's forward, or MakeRotFromX of an impact normal.
    pub fn pointing(mut self, dir: Vec3) -> Self {
        self.at.rotation = Quat::from_rotation_arc(Vec3::X, dir.normalize_or(Vec3::X));
        self
    }

    /// Turned `yaw` radians about the vertical (MakeRotator(0, 0, Yaw)).
    pub fn yawed(mut self, yaw: f32) -> Self {
        self.at.rotation = Quat::from_rotation_y(yaw) * self.at.rotation;
        self
    }

    pub fn scaled(mut self, s: Vec3) -> Self {
        self.at.scale = s;
        self
    }

    pub fn attached(mut self, parent: Entity) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn stop_after(mut self, secs: f32) -> Self {
        self.stop_after = Some(secs);
        self
    }
}

/// A level emitter: on within `range` of the survivor (its owner's AIO layer 0).
#[derive(Component, Debug)]
pub struct LevelFx {
    pub owner: String,
    pub actor: String,
    pub range: f32,
    /// Put out for good (a salvaged wreck's smoke).
    pub out: bool,
}

/// A drawn particle: which emitter it belongs to.
#[derive(Component)]
struct ParticleView(Entity);

#[derive(Deserialize)]
struct PlacedEmitter {
    owner: String,
    #[serde(default)]
    actor: String,
    system: String,
    transform: [f32; 12],
    #[serde(default = "yes")]
    auto: bool,
    layer: Option<f32>,
}

fn yes() -> bool {
    true
}

/// Which level emitters run: all with bAutoActivate, plus the wreck smoke, which
/// BP_HarvestableObject_ScrapMetal activates on BeginPlay and by its optimisation layer.
/// (BP_Campfire's flames are hidden in its template and shown by the layer too.)
pub fn level_emitter_runs(owner: &str, auto: bool) -> bool {
    auto || owner == "BP_HarvestableObject_ScrapMetal_C"
}

pub fn plugin(app: &mut App) {
    app.add_message::<Fx>()
        .init_resource::<FxAssets>()
        .add_systems(OnEnter(AppState::InGame), place_level_emitters)
        .add_systems(Update, (spawn_fx, wreck_smoke, simulate, strip_vertex_colours, draw, cleanup).chain().run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

fn place_level_emitters(mut commands: Commands) {
    let path = format!("{}/{LEVEL_FILE}", crate::level::LEVEL_DIR);
    let Some(list) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Vec<PlacedEmitter>>(&t).ok()) else {
        warn!("{path} missing; run extractor/export_assets.sh --particles");
        return;
    };
    let mut n = 0;
    for e in list.into_iter().filter(|e| level_emitter_runs(&e.owner, e.auto) && fx_data().systems.contains_key(&e.system)) {
        commands.spawn((
            crate::level::affine(&e.transform),
            Visibility::default(),
            Emitter { active: false, ..Emitter::new(&e.system) },
            LevelFx { owner: e.owner, actor: e.actor, range: e.layer.unwrap_or(50.0), out: false },
            DespawnOnExit(AppState::InGame),
        ));
        n += 1;
    }
    info!("particles: {n} level emitters");
}

fn spawn_fx(mut commands: Commands, mut fx: MessageReader<Fx>) {
    for f in fx.read() {
        if !fx_data().systems.contains_key(&f.system) {
            warn!("particles: no system {}", f.system);
            continue;
        }
        let mut e = commands.spawn((f.at, Visibility::default(), Emitter { stop_after: f.stop_after, one_shot: true, ..Emitter::new(&f.system) }, DespawnOnExit(AppState::InGame)));
        if let Some(p) = f.parent {
            e.insert(ChildOf(p));
        }
    }
}

/// BP_HarvestableObject_ScrapMetal destroys its smoke once salvaged.
fn wreck_smoke(harvest: Res<super::harvest::Harvest>, level: Option<Res<crate::level::Level>>, mut fx: Query<&mut LevelFx>) {
    let Some(level) = level else { return };
    if !harvest.is_changed() {
        return;
    }
    for mut f in &mut fx {
        if f.owner == "BP_HarvestableObject_ScrapMetal_C" && !f.out && level.actors.iter().position(|a| a.name == f.actor).is_some_and(|i| harvest.salvaged.contains(&i)) {
            f.out = true;
        }
    }
}

fn simulate(time: Res<Time>, player: Query<&Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>, mut emitters: Query<(Ref<GlobalTransform>, &mut Emitter, Option<&LevelFx>)>) {
    let dt = time.delta_secs().min(0.1);
    let eye = player.single().map(|t| t.translation).ok();
    let mut rng = rand::thread_rng();
    for (gt, mut em, level) in &mut emitters {
        // Spawned this frame: its GlobalTransform is still identity until propagation, and a
        // burst would go off at the world's origin.
        if gt.is_added() {
            continue;
        }
        if let Some(l) = level {
            // AIO layer 0: shown and spawning inside it, hidden beyond.
            let near = !l.out && eye.is_some_and(|p| p.distance(gt.translation()) <= l.range);
            if !near {
                if em.active || em.live() > 0 {
                    em.active = false;
                    em.clear();
                }
                continue;
            }
            em.active = true;
        }
        if let Some(s) = em.stop_after.as_mut() {
            *s -= dt;
            if *s <= 0.0 {
                em.active = false;
                em.stop_after = None;
            }
        }
        let Some(sys) = fx_data().systems.get(&em.system) else { continue };
        let xf = gt.affine();
        let active = em.active;
        for (state, def) in em.states.iter_mut().zip(&sys.emitters) {
            state.tick(def, dt, &xf, active, &mut rng);
        }
    }
}

/// Meshes and materials for drawing particles.
#[derive(Resource)]
struct FxAssets {
    quad: Handle<Mesh>,
    meshes: HashMap<String, Handle<Mesh>>,
    /// (material name, quantised colour and opacity) -> material.
    materials: HashMap<(String, [u16; 4]), Handle<StandardMaterial>>,
}

impl FromWorld for FxAssets {
    fn from_world(world: &mut World) -> Self {
        let quad = world.resource_mut::<Assets<Mesh>>().add(Rectangle::new(1.0, 1.0));
        Self { quad, meshes: HashMap::new(), materials: HashMap::new() }
    }
}

/// Colour channels on a square-root scale (fine near 0, coarse in the HDR range), opacity in 16ths.
fn quantise(c: Vec3, opacity: f32) -> [u16; 4] {
    let q = |v: f32| (v.clamp(0.0, 16.0).sqrt() * 24.0).round() as u16;
    [q(c.x), q(c.y), q(c.z), (opacity.clamp(0.0, 1.0) * 16.0).round() as u16]
}

fn dequantise(q: [u16; 4]) -> (Vec3, f32) {
    let d = |v: u16| (v as f32 / 24.0).powi(2);
    (Vec3::new(d(q[0]), d(q[1]), d(q[2])), q[3] as f32 / 16.0)
}

/// What a particle draws as: its blend and whether it's visible at all.
pub fn particle_look(info: MaterialInfo, def: &EmitterDef, color: Vec4) -> Option<(Vec3, f32)> {
    let rgb = color.truncate().max(Vec3::ZERO);
    let opacity = def.opacity(color.w);
    match info.blend {
        Blend::Masked if opacity < 1.0 / 3.0 => None,
        Blend::Masked | Blend::Opaque => Some((rgb, 1.0)),
        _ if opacity <= 1.0 / 32.0 => None,
        _ => Some((rgb, opacity)),
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut fx: ResMut<FxAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    library: Res<crate::models::GameMaterials>,
    camera: Query<&GlobalTransform, With<super::combat::PlayerCamera>>,
    mut emitters: Query<(Entity, &GlobalTransform, &mut Emitter)>,
    mut views: Query<(&mut Transform, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility), With<ParticleView>>,
) {
    let cam = camera.single().map(|c| c.rotation()).unwrap_or(Quat::IDENTITY);
    let data = fx_data();
    for (entity, gt, mut em) in &mut emitters {
        let Some(sys) = data.systems.get(&em.system) else { continue };
        let (owner_scale, owner_rot, _) = gt.to_scale_rotation_translation();
        let xf = gt.affine();
        for (state, def) in em.states.iter_mut().zip(&sys.emitters) {
            let mat_name = def.material.clone().unwrap_or_default();
            let info = data.materials.get(&mat_name).copied().unwrap_or_default();
            let mesh = match (&def.mesh, def.sprite) {
                (Some(m), false) => fx.meshes.entry(m.clone()).or_insert_with(|| assets.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(format!("game/{m}.glb")))).clone(),
                _ => fx.quad.clone(),
            };
            let mut used = 0;
            for p in &state.particles {
                let Some((rgb, opacity)) = particle_look(info, def, p.color) else { continue };
                let key = (mat_name.clone(), quantise(rgb, opacity));
                let material = if let Some(h) = fx.materials.get(&key) {
                    h.clone()
                } else {
                    let (rgb, opacity) = dequantise(key.1);
                    let texture = def.sprite.then(|| library.texture(&mat_name, &assets)).flatten();
                    let (base, alpha_mode) = match info.blend {
                        Blend::Masked | Blend::Opaque => (LinearRgba::new(rgb.x, rgb.y, rgb.z, 1.0), AlphaMode::Opaque),
                        Blend::Additive => (LinearRgba::new(rgb.x * opacity, rgb.y * opacity, rgb.z * opacity, 1.0), AlphaMode::Add),
                        _ => (LinearRgba::new(rgb.x, rgb.y, rgb.z, opacity), AlphaMode::Blend),
                    };
                    let h = materials.add(StandardMaterial {
                        base_color: base.into(),
                        base_color_texture: texture,
                        unlit: !info.lit,
                        alpha_mode,
                        perceptual_roughness: 0.9,
                        double_sided: def.sprite,
                        cull_mode: if def.sprite { None } else { Some(bevy::render::render_resource::Face::Back) },
                        ..default()
                    });
                    fx.materials.insert(key, h.clone());
                    h
                };
                let pos = if def.local { xf.transform_point3(p.pos) } else { p.pos };
                let (rotation, scale) = if def.sprite {
                    (cam * Quat::from_rotation_z(p.rot), Vec3::new(p.size.x, p.size.y, 1.0) * owner_scale)
                } else {
                    let base = if def.local { owner_rot } else { Quat::IDENTITY };
                    (base * p.mesh_rot * Quat::from_rotation_y(p.rot), p.size * owner_scale)
                };
                let t = Transform { translation: pos, rotation, scale: scale.max(Vec3::splat(1e-4)) };
                if used == state.views.len() {
                    let v = commands
                        .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material), t, Visibility::Visible, NotShadowCaster, NotShadowReceiver, ParticleView(entity), DespawnOnExit(AppState::InGame)))
                        .id();
                    state.views.push(v);
                } else if let Ok((mut vt, mut vm, mut vis)) = views.get_mut(state.views[used]) {
                    *vt = t;
                    if vm.0 != material {
                        vm.0 = material;
                    }
                    if *vis != Visibility::Visible {
                        *vis = Visibility::Visible;
                    }
                }
                used += 1;
            }
            for &v in &state.views[used..] {
                if let Ok((_, _, mut vis)) = views.get_mut(v)
                    && *vis != Visibility::Hidden
                {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}

/// Particle meshes draw without their vertex colours (see the module docs), once loaded.
fn strip_vertex_colours(fx: Res<FxAssets>, mut meshes: ResMut<Assets<Mesh>>) {
    for h in fx.meshes.values() {
        if meshes.get(h).is_some_and(|m| m.contains_attribute(Mesh::ATTRIBUTE_COLOR))
            && let Some(mut m) = meshes.get_mut(h)
        {
            m.remove_attribute(Mesh::ATTRIBUTE_COLOR);
        }
    }
}

/// Spent one-shot effects go; drawn particles whose emitter has gone go with it.
fn cleanup(mut commands: Commands, emitters: Query<(Entity, &Emitter)>, views: Query<(Entity, &ParticleView)>) {
    for (e, em) in &emitters {
        if em.one_shot && em.spent() {
            commands.entity(e).despawn();
        }
    }
    for (v, of) in &views {
        if emitters.get(of.0).is_err() {
            commands.entity(v).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Option<&'static FxData> {
        let d = fx_data();
        (!d.systems.is_empty()).then_some(d)
    }

    #[test]
    fn distributions_sample_like_the_lookup_tables() {
        let mut rng = rand::thread_rng();
        let c = Dist::constant(&[2.0]);
        assert_eq!(c.scalar(5.0, &mut rng), 2.0);
        // A random range: within min..max.
        let r = Dist { op: Op::Random, v: vec![1.9, 2.2], ..Dist::constant(&[0.0]) };
        for _ in 0..50 {
            assert!((1.9..=2.2).contains(&r.scalar(0.0, &mut rng)));
        }
        // Over life: two entries 0 -> 1 over t 0..1 (scale 1), linear between.
        let l = Dist { n: 2, scale: 1.0, v: vec![0.0, 1.0], ..Dist::constant(&[0.0]) };
        assert!((l.scalar(0.25, &mut rng) - 0.25).abs() < 1e-6);
        assert_eq!(l.scalar(2.0, &mut rng), 1.0, "clamped past the end");
        // Locked XYZ: one fraction for every axis.
        let v = Dist { dims: 3, op: Op::Random, lock: 4, v: vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0], ..Dist::constant(&[0.0]) };
        for _ in 0..20 {
            let s = v.vector(0.0, &mut rng);
            assert!((s.y - 2.0 * s.x).abs() < 1e-5 && (s.z - 3.0 * s.x).abs() < 1e-5);
        }
    }

    #[test]
    fn barrel_fire_burns_steadily_in_bevy_units() {
        let Some(d) = data() else { return };
        let fire = &d.systems["FX_Fire"];
        assert_eq!(fire.emitters.len(), 2);
        let e = &fire.emitters[0];
        // Rate 8 x RateScale 2, looping for ever; a 10 cm sphere; rising 0.5-1 m/s.
        assert_eq!((e.loops, e.duration, e.material.as_deref()), (0, 1.0, Some("Mat_FX")));
        assert_eq!(d.materials["Mat_FX"].blend, Blend::Masked);
        let mut rng = rand::thread_rng();
        let mut s = EmitterState::new(e);
        let xf = Affine3A::IDENTITY;
        for _ in 0..60 {
            s.tick(e, 1.0 / 60.0, &xf, true, &mut rng);
        }
        assert!((15..=17).contains(&s.particles.len()), "16 a second: {}", s.particles.len());
        assert!(s.particles.iter().all(|p| p.vel.y > 0.0 && p.pos.length() < 1.1), "rising from a 10 cm sphere");
        // Lifetime 1.9-2.2 s: after 3 s the count holds near 16 x 2 s.
        for _ in 0..120 {
            s.tick(e, 1.0 / 60.0, &xf, true, &mut rng);
        }
        assert!((28..=38).contains(&s.particles.len()), "steady state: {}", s.particles.len());
        // Size starts at StartSize (0.1-0.15) and grows x1-5 over life.
        assert!(s.particles.iter().all(|p| p.size.x >= 0.1 - 1e-4 && p.size.x <= 0.75 + 1e-4));
        // Deactivated: no more, and they burn out.
        for _ in 0..180 {
            s.tick(e, 1.0 / 60.0, &xf, false, &mut rng);
        }
        assert!(s.particles.is_empty());
    }

    #[test]
    fn a_frag_blast_bursts_once_and_is_spent() {
        let Some(d) = data() else { return };
        let mut em = Emitter::new("FX_Explosion_New");
        assert_eq!(em.states.len(), 3);
        let sys = &d.systems["FX_Explosion_New"];
        assert!(sys.emitters.iter().all(|e| e.loops == 1 && e.duration == 3.0 && !e.bursts.is_empty()));
        let mut rng = rand::thread_rng();
        let xf = Affine3A::from_translation(Vec3::new(10.0, 2.0, 0.0));
        for (s, e) in em.states.iter_mut().zip(&sys.emitters) {
            s.tick(e, 1.0 / 60.0, &xf, true, &mut rng);
        }
        let first = em.live();
        assert!(first >= 20, "the bursts: {first}");
        assert!(em.states.iter().flat_map(|s| &s.particles).all(|p| p.pos.distance(xf.translation.into()) < 3.0));
        for _ in 0..60 * 8 {
            for (s, e) in em.states.iter_mut().zip(&sys.emitters) {
                s.tick(e, 1.0 / 60.0, &xf, true, &mut rng);
            }
        }
        assert!(em.spent(), "3 s of emitter time, one loop, then the particles die");
    }

    #[test]
    fn the_component_scale_shrinks_the_effect() {
        let Some(d) = data() else { return };
        let e = &d.systems["FX_Fire"].emitters[0];
        let mut rng = rand::thread_rng();
        let big = spawn(e, 0.0, &Affine3A::IDENTITY, &mut rng);
        let small = spawn(e, 0.0, &Affine3A::from_scale(Vec3::splat(0.25)), &mut rng);
        assert!(small.pos.length() <= 0.1 * 0.25 + 1e-4 && big.pos.length() <= 0.1 + 1e-4);
        assert!(small.vel.y <= 0.25 + 1e-4 && big.vel.y >= 0.5 - 1e-4);
    }

    #[test]
    fn opacity_follows_the_alpha_curves_peak() {
        let Some(d) = data() else { return };
        // FX_Fire_AI holds alpha 0.03 all its life: fully drawn. The wreck smoke's alpha is 0
        // throughout: ignored. FX_Fire's masked flames show above a third of their peak.
        let ai = &d.systems["FX_Fire_AI"].emitters[0];
        assert!((ai.opacity(0.03) - 1.0).abs() < 1e-3);
        let smoke = &d.systems["FX_SmokeHarvestableVehicle"].emitters[0];
        assert_eq!(smoke.opacity(0.0), 1.0);
        let fire = &d.systems["FX_Fire"].emitters[0];
        let masked = MaterialInfo { blend: Blend::Masked, lit: false };
        assert!(particle_look(masked, fire, Vec4::new(8.0, 7.0, 3.0, 0.1)).is_none());
        assert_eq!(particle_look(masked, fire, Vec4::new(8.0, 7.0, 3.0, 0.8)).map(|l| l.1), Some(1.0));
    }

    #[test]
    fn quantised_colours_round_trip() {
        for c in [Vec3::new(8.0, 7.19, 3.48), Vec3::new(0.5, 0.52, 0.46), Vec3::ZERO] {
            let (back, a) = dequantise(quantise(c, 0.5));
            assert!((back - c).abs().max_element() < 0.25 && (a - 0.5).abs() < 1e-6, "{c} -> {back}");
        }
    }

    #[test]
    fn level_emitters_follow_their_blueprints() {
        assert!(level_emitter_runs("BP_BarrelFire_C", true));
        assert!(level_emitter_runs("BP_HarvestableObject_ScrapMetal_C", false), "activated on BeginPlay");
        assert!(!level_emitter_runs("BP_LaboratoryCircuitBox_C", false));
    }
}
