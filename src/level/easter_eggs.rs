//! The level's easter eggs, decompiled:
//!
//! - **BP_EasterEgg_GlowingEyes** (6): a hanging body in Mat_PolygonApocalypse_Chr_01_AGhost
//!   (additive, BloodColour (0.146, 0.005, 0)) with two eye cubes on it (M_GlowingEyes_Ghost:
//!   translucent, Colour (0.016, 0.0001, 0) x EmissiveStrength 200). BeginPlay hides it and binds
//!   UDS's Midnight. At midnight it shows for a 60 s Delay. A 0.2 s looping timer turns it to face
//!   the survivor (FacePlayer: RInterpTo at speed 15 by the frame's delta, yaw only). HoverTL
//!   (8 s, looping: 0 -> 1 at 4 s -> 0, cubic auto keys) lifts the body Lerp(0, 25 cm). After the
//!   Delay it hides, the timer is cleared and the timeline stopped.
//! - **BP_EasterEgg_Campfire** (1, by the cave one of the ghosts hangs in): hidden at BeginPlay,
//!   then at Midnight shown for a 60 s Delay. A 0.2 s timer hides it while the survivor is within
//!   15 m (GetDistanceTo <= 1500). SetActorHiddenInGame hides its mesh, light and flames, but not
//!   its Audio (SC_CampFire, auto-activated), so the crackle plays all the time. Its AIOSubject
//!   (100 / 125 m) shows the parts within 100 m only.
//! - **BP_EasterEgg_Nuke** (1, SM_Prop_Nuke_01 on a broken road between two warning signs):
//!   Damage_Object (any hit) -> Svr_Explode -> MC_Explode, once (a DoOnce). It gives
//!   ACH_FinalDecision, FX_Explosion_New at a random yaw scaled 25-30 a side,
//!   explosion_large_07_Cue, a 150 m noise, the M_Burn decal, and the camera shake within its
//!   CameraShakeSphere (125 m). Then every character in its 125 m ExplosionSphere that's a living
//!   AI or the survivor ('Player' tag) takes 250-500 with a white floating number (and
//!   RadialForce's impulse), and the nuke is gone. Damage_Shoved does nothing.
//!
//! Ours: the ghost's materials are guesses (their graphs aren't readable): the body is an unlit
//! additive tint of its BloodColour, the eyes unlit Colour x 200. Its AIOSubject hiding one eye
//! beyond 100 m is left out. The nuke counts a round through its mesh's box, a swing within 2 m in
//! front or a blast reaching it as a hit, and it comes back next session (as the barrels do). Not
//! ported: the achievement, the decal, the camera shake, RadialForce.

use bevy::prelude::*;
use rand::Rng;

use super::{KinematicBody, LevelZombie, PropKey};
use crate::gameplay::combat::{Dead, Health};
use crate::menus::{AppState, CurrentGame};
use crate::models::{CharacterModel, UpperAction};

/// The Midnight event's Delay before hiding again.
pub const SHOWN_FOR: f32 = 60.0;
/// The FacePlayer / campfire distance timer.
const TIMER: f32 = 0.2;
/// FacePlayer's RInterpTo speed.
const FACE_SPEED: f32 = 15.0;
/// HoverTL: its length and the body's lift at the curve's top (Lerp(0, 25) cm).
const HOVER_LOOP: f32 = 8.0;
const HOVER_LIFT: f32 = 0.25;
/// The campfire hides with the survivor this close (1500 cm).
pub const CAMPFIRE_SHY: f32 = 15.0;
/// The campfire's AIOSubject layer 0 (10000 cm): its parts are shown within it.
const CAMPFIRE_SHOWN_WITHIN: f32 = 100.0;
/// The nuke's ExplosionSphere / CameraShakeSphere radius (12500 cm).
pub const NUKE_RADIUS: f32 = 125.0;
/// ApplyDamage's roll.
pub const NUKE_DAMAGE: (f32, f32) = (250.0, 500.0);
/// ReportNoiseEvent's MaxRange (15000 cm).
const NUKE_NOISE: f32 = 150.0;
/// The emitter's Random Vector In Range scale.
const NUKE_FX_SCALE: (f32, f32) = (25.0, 30.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EggKind {
    Eyes,
    Campfire,
}

/// A glowing-eyes ghost or the campfire: hidden until midnight.
#[derive(Clone, Debug, PartialEq)]
pub struct Apparition {
    pub kind: EggKind,
    pub actor: String,
    pub at: Vec3,
    /// Seconds left of the midnight Delay (None: hidden for good until the next midnight).
    pub left: Option<f32>,
    /// SetActorHiddenInGame.
    pub hidden: bool,
    /// The looping timer's countdown.
    timer: f32,
    /// HoverTL's position (s).
    pub hover: f32,
    /// The actor's yaw (radians about +Y; 0 = its +X along world +X).
    pub yaw: f32,
}

/// HoverTL's curve: 0 -> 1 at 4 s -> 0 at 8 s, cubic keys with flat (auto) tangents.
pub fn hover_curve(t: f32) -> f32 {
    let t = t.rem_euclid(HOVER_LOOP) / (HOVER_LOOP / 2.0);
    let x = if t <= 1.0 { t } else { 2.0 - t };
    x * x * (3.0 - 2.0 * x)
}

/// The yaw that points an actor's +X from `from` at `to` (FindLookAtRotation's yaw).
pub fn look_yaw(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    (-d.z).atan2(d.x)
}

/// RInterpTo on the yaw: the shortest way round, by min(dt x speed, 1) of the gap.
pub fn interp_yaw(current: f32, target: f32, dt: f32, speed: f32) -> f32 {
    let gap = (target - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    current + gap * (dt * speed).min(1.0)
}

impl Apparition {
    pub fn new(kind: EggKind, actor: String, at: Vec3, yaw: f32) -> Self {
        Self { kind, actor, at, left: None, hidden: true, timer: TIMER, hover: 0.0, yaw }
    }

    /// UDS's Midnight: shown, the timer (re)started, the hover from its start. The latent Delay
    /// already running carries on.
    pub fn midnight(&mut self) {
        self.timer = TIMER;
        self.hidden = false;
        if self.kind == EggKind::Eyes {
            self.hover = 0.0;
        }
        if self.left.is_none() {
            self.left = Some(SHOWN_FOR);
        }
    }

    /// Whether it's up (the 60 s after a midnight).
    pub fn active(&self) -> bool {
        self.left.is_some()
    }

    /// Advance `dt` with the survivor at `player`; `frame_dt` is the world delta FacePlayer reads.
    pub fn step(&mut self, dt: f32, player: Option<Vec3>, frame_dt: f32) {
        let Some(left) = self.left.as_mut() else { return };
        *left -= dt;
        if *left <= 0.0 {
            self.left = None;
            self.hidden = true;
            return;
        }
        if self.kind == EggKind::Eyes {
            self.hover = (self.hover + dt).rem_euclid(HOVER_LOOP);
        }
        self.timer -= dt;
        while self.timer <= 0.0 {
            self.timer += TIMER;
            let Some(p) = player else { continue };
            match self.kind {
                EggKind::Eyes => self.yaw = interp_yaw(self.yaw, look_yaw(self.at, p), frame_dt, FACE_SPEED),
                EggKind::Campfire => self.hidden = p.distance(self.at) <= CAMPFIRE_SHY,
            }
        }
    }

    /// The body's lift (m).
    pub fn lift(&self) -> f32 {
        if self.active() { HOVER_LIFT * hover_curve(self.hover) } else { 0.0 }
    }
}

/// The nuke: a box round its mesh to hit, and whether it went off.
#[derive(Clone, Debug, PartialEq)]
pub struct Nuke {
    /// The mesh's transform and its half extents in mesh space.
    pub transform: Transform,
    pub half: Vec3,
    pub key: Option<PropKey>,
    pub exploded: bool,
}

impl Nuke {
    /// The box's point nearest `p`.
    pub fn nearest(&self, p: Vec3) -> Vec3 {
        let inv = self.transform.compute_affine().inverse();
        let local = inv.transform_point3(p).clamp(-self.half, self.half);
        self.transform.compute_affine().transform_point3(local)
    }

    /// Whether a round from `origin` along `dir`, stopping `reach` m out, passes through the box.
    pub fn shot_hits(&self, origin: Vec3, dir: Vec3, reach: f32) -> bool {
        let inv = self.transform.compute_affine().inverse();
        let (o, d) = (inv.transform_point3(origin), inv.transform_vector3(dir * reach.max(0.0) + dir * 0.1));
        // Slab test along the segment, in mesh space.
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for k in 0..3 {
            if d[k].abs() < 1e-6 {
                if o[k].abs() > self.half[k] {
                    return false;
                }
                continue;
            }
            let (a, b) = ((-self.half[k] - o[k]) / d[k], (self.half[k] - o[k]) / d[k]);
            lo = lo.max(a.min(b));
            hi = hi.min(a.max(b));
        }
        lo <= hi
    }

    /// MC_Explode's DoOnce: true the first time only.
    pub fn explode(&mut self) -> bool {
        !std::mem::replace(&mut self.exploded, true)
    }
}

#[derive(Resource, Default)]
pub struct EasterEggs {
    pub apparitions: Vec<Apparition>,
    pub nuke: Option<Nuke>,
    /// The day count last seen (a rise is UDS's Midnight).
    day: Option<u32>,
    collected: bool,
}

impl EasterEggs {
    /// Whether the egg campfire `actor` is drawn (its light and flames follow).
    pub fn fire_shown(&self, actor: &str) -> bool {
        self.apparitions.iter().any(|a| a.kind == EggKind::Campfire && a.actor == actor && !a.hidden)
    }
}

/// An apparition's drawn parts: the root (yaw) and, for the ghost, the hovering body.
#[derive(Component)]
pub struct EggRoot(usize);
#[derive(Component)]
pub struct EggBody;

pub(super) fn reset_eggs(mut eggs: ResMut<EasterEggs>) {
    *eggs = EasterEggs::default();
}

/// Take the eggs' meshes out of the static cells and draw them here, hidden.
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_eggs(
    mut commands: Commands,
    assets: Res<AssetServer>,
    level: Option<Res<super::Level>>,
    streaming: Option<ResMut<super::PropStreaming>>,
    mut eggs: ResMut<EasterEggs>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(level), Some(mut streaming)) = (level, streaming) else { return };
    if eggs.collected {
        return;
    }
    eggs.collected = true;
    let mut hide = |key: PropKey, commands: &mut Commands| {
        if let Some(cell) = streaming.hide(key) {
            commands.entity(cell).despawn();
        }
    };
    let ghost = materials.add(StandardMaterial { base_color: Color::linear_rgb(0.145833, 0.005353, 0.0), unlit: true, alpha_mode: AlphaMode::Add, ..default() });
    let eye = materials.add(StandardMaterial { base_color: Color::linear_rgb(0.015625 * 200.0, 0.000149 * 200.0, 0.0), unlit: true, ..default() });
    // The engine's Cube is 256 cm a side.
    let cube = meshes.add(Cuboid::from_length(2.56));
    for a in &level.actors {
        let kind = match a.class.as_str() {
            "BP_EasterEgg_GlowingEyes_C" => EggKind::Eyes,
            "BP_EasterEgg_Campfire_C" => EggKind::Campfire,
            "BP_EasterEgg_Nuke_C" => {
                let at = a.position();
                let found = level.prop_near(at, "SM_Prop_Nuke_01", 0.5);
                let transform = found.as_ref().map_or(super::affine(&a.transform), |f| f.1);
                eggs.nuke = Some(Nuke { transform, half: Vec3::new(2.0, 1.0, 1.0), key: found.map(|f| f.0), exploded: false });
                continue;
            }
            _ => continue,
        };
        let t = super::affine(&a.transform);
        let (x, _, _) = t.rotation.to_euler(EulerRot::YXZ);
        let i = eggs.apparitions.len();
        eggs.apparitions.push(Apparition::new(kind, a.name.clone(), t.translation, x));
        let root = commands.spawn((Transform::from_translation(t.translation).with_rotation(t.rotation), Visibility::Hidden, EggRoot(i), DespawnOnExit(AppState::InGame))).id();
        let inv = t.compute_affine().inverse();
        let local = |w: &Transform| Transform::from_matrix(Mat4::from(inv) * w.to_matrix());
        match kind {
            EggKind::Eyes => {
                let Some((key, bt, glb)) = level.prop_near(t.translation, "SM_Prop_DeadBody_Hanging_Male_02", 0.5) else {
                    warn!("glowing eyes {}: no body mesh", a.name);
                    continue;
                };
                hide(key, &mut commands);
                let body = commands.spawn((crate::models::model(&assets, glb), local(&bt), EggBody, EggGhost(ghost.clone()), ChildOf(root))).id();
                // The eyes hang off the body; the body sits at the actor's origin.
                for (key, et, _) in level.props_near(t.translation, "Cube", 0.5).into_iter().take(2) {
                    hide(key, &mut commands);
                    let rel = Transform::from_matrix(Mat4::from(bt.compute_affine().inverse()) * et.to_matrix());
                    commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(eye.clone()), rel, bevy::light::NotShadowCaster, EggEye, ChildOf(body)));
                }
            }
            EggKind::Campfire => {
                if let Some((key, ft, glb)) = level.prop_near(t.translation, "SM_Prop_CampFire_01", 0.5) {
                    hide(key, &mut commands);
                    commands.spawn((crate::models::model(&assets, glb), local(&ft), ChildOf(root)));
                }
            }
        }
    }
    info!("easter eggs: {} apparitions, nuke {}", eggs.apparitions.len(), eggs.nuke.is_some());
}

/// The ghost's material, swapped in once its scene is in.
#[derive(Component)]
pub(super) struct EggGhost(Handle<StandardMaterial>);

pub(super) fn ghost_material(ghosts: Query<(Entity, &EggGhost)>, children: Query<&Children>, mut mats: Query<&mut MeshMaterial3d<StandardMaterial>, Without<EggEye>>) {
    for (e, g) in &ghosts {
        for c in children.iter_descendants(e) {
            if let Ok(mut m) = mats.get_mut(c)
                && m.0 != g.0
            {
                m.0 = g.0.clone();
            }
        }
    }
}

/// An eye cube (it keeps its own material).
#[derive(Component)]
pub(super) struct EggEye;

/// Midnight, the timers, the hover and the facing; the campfire's light and flames follow it.
#[allow(clippy::type_complexity)]
pub fn run_apparitions(
    time: Res<Time>,
    game: Res<CurrentGame>,
    mut eggs: ResMut<EasterEggs>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>, Without<EggRoot>, Without<EggBody>)>,
    mut roots: Query<(&EggRoot, &mut Transform, &mut Visibility), Without<EggBody>>,
    mut bodies: Query<(&ChildOf, &mut Transform), (With<EggBody>, Without<EggRoot>)>,
    mut fx: Query<&mut crate::gameplay::particles::LevelFx>,
) {
    let Some(day) = game.state.as_ref().map(|s| s.environment.days) else { return };
    let me = player.single().ok().map(|t| t.translation);
    let dt = time.delta_secs();
    let midnight = eggs.day.is_some_and(|d| day > d);
    eggs.day = Some(day);
    for a in &mut eggs.apparitions {
        if midnight {
            a.midnight();
            info!("easter egg {} ({:?}) shows at midnight", a.actor, a.kind);
        }
        let was = a.active();
        a.step(dt, me, dt);
        if was && !a.active() {
            info!("easter egg {} ({:?}) gone again", a.actor, a.kind);
        }
    }
    let eggs = &*eggs;
    for (r, mut t, mut vis) in &mut roots {
        let Some(a) = eggs.apparitions.get(r.0) else { continue };
        let far = a.kind == EggKind::Campfire && me.is_some_and(|p| p.distance(a.at) > CAMPFIRE_SHOWN_WITHIN);
        let want = if a.hidden || far { Visibility::Hidden } else { Visibility::Inherited };
        if *vis != want {
            *vis = want;
        }
        if a.kind == EggKind::Eyes {
            t.rotation = Quat::from_rotation_y(a.yaw);
        }
    }
    for (parent, mut t) in &mut bodies {
        let Ok((r, _, _)) = roots.get(parent.parent()) else { continue };
        let Some(a) = eggs.apparitions.get(r.0) else { continue };
        t.translation.y = a.lift();
    }
    for mut f in &mut fx {
        if f.owner == "BP_EasterEgg_Campfire_C" {
            let out = !eggs.fire_shown(&f.actor);
            if f.out != out {
                f.out = out;
            }
        }
    }
}

/// The nuke's Damage_Object: the survivor's rounds and swings, and blasts.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn nuke_damage(
    mut commands: Commands,
    mut eggs: ResMut<EasterEggs>,
    (mut shots, mut swings, mut blasts): (MessageReader<crate::gameplay::turrets::Shot>, MessageReader<crate::gameplay::harvest::Swing>, MessageReader<crate::gameplay::barrels::ObjectBlast>),
    (db, store): (Res<crate::gameplay::ItemDb>, Res<crate::menus::settings::SettingsStore>),
    mut game: ResMut<CurrentGame>,
    mut streaming: Option<ResMut<super::PropStreaming>>,
    mut cache: ResMut<super::collision::CollisionCache>,
    items: Query<(Entity, &super::PropItem)>,
    mut zombies: Query<(&Transform, &KinematicBody, &mut Health, Option<&mut UpperAction>), (With<LevelZombie>, Without<Dead>)>,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>)>,
    (mut fx, mut noise, mut numbers, mut sound): (
        MessageWriter<crate::gameplay::particles::Fx>,
        MessageWriter<crate::gameplay::zombies::Noise>,
        MessageWriter<crate::gameplay::damage_numbers::DamageNumber>,
        MessageWriter<crate::audio::Sound>,
    ),
) {
    let Some(nuke) = eggs.nuke.as_mut().filter(|n| !n.exploded) else {
        shots.clear();
        swings.clear();
        blasts.clear();
        return;
    };
    let hit = shots.read().any(|s| nuke.shot_hits(s.origin, s.dir, s.reach))
        | swings.read().any(|s| {
            let flat = (nuke.nearest(s.feet) - s.feet).with_y(0.0);
            flat.length() <= 2.0 && flat.normalize_or(s.facing).dot(s.facing) > 0.5
        })
        | blasts.read().any(|b| nuke.nearest(b.at).distance(b.at) <= b.radius);
    if !hit || !nuke.explode() {
        return;
    }
    let at = nuke.transform.translation;
    let mut rng = rand::thread_rng();
    fx.write(crate::gameplay::particles::Fx::at("FX_Explosion_New", at).yawed(rng.gen_range(0.0..std::f32::consts::TAU)).scaled(crate::gameplay::throwables::random_scale(NUKE_FX_SCALE.0, NUKE_FX_SCALE.1, &mut rng)));
    sound.write(crate::audio::Sound::at("explosion_large_07_Cue", at));
    noise.write(crate::gameplay::zombies::Noise { at, range: NUKE_NOISE });
    commands.spawn((
        Transform::from_translation(at + Vec3::Y * 5.0),
        PointLight { color: Color::srgb(1.0, 0.75, 0.4), intensity: 4_000_000_000.0, range: NUKE_RADIUS * 2.0, ..default() },
        crate::gameplay::throwables::Flash(1.5),
        DespawnOnExit(AppState::InGame),
    ));
    let mut hurt = crate::gameplay::throwables::hurt_zombies(at, NUKE_RADIUS, NUKE_DAMAGE, &mut zombies);
    if let Ok((t, body)) = player.single()
        && crate::gameplay::throwables::reaches(at, NUKE_RADIUS, t.translation, body)
    {
        let dmg = rng.gen_range(NUKE_DAMAGE.0..=NUKE_DAMAGE.1);
        let taken = crate::gameplay::survival::take_hit(&mut game, &db, &store, dmg);
        if let Some(s) = game.state.as_mut() {
            s.survival.health = (s.survival.health - taken).max(0.0);
        }
        hurt.push((dmg, crate::gameplay::damage_numbers::body(t.translation, body.height)));
    }
    numbers.write_batch(hurt.iter().map(|&(damage, at)| crate::gameplay::damage_numbers::DamageNumber::blast(damage, at)));
    if let Some(key) = nuke.key {
        if let Some(s) = streaming.as_mut() {
            s.remove(key);
        }
        cache.forget(key);
        for (e, p) in &items {
            if p.0 == key {
                commands.entity(e).despawn();
            }
        }
    }
    info!("nuke exploded at {at:.1}: {} hurt", hurt.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apparitions_show_for_a_minute_at_midnight() {
        let mut a = Apparition::new(EggKind::Eyes, "e".into(), Vec3::ZERO, 0.0);
        a.step(1.0, Some(Vec3::X), 1.0 / 60.0);
        assert!(a.hidden && !a.active(), "hidden until midnight");
        a.midnight();
        assert!(!a.hidden && a.active());
        // The hover: up 25 cm at 4 s, down again at 8 s.
        for _ in 0..20 {
            a.step(0.2, Some(Vec3::new(0.0, 0.0, -10.0)), 1.0 / 60.0);
        }
        assert!((a.lift() - 0.25).abs() < 1e-3, "{}", a.lift());
        assert!((hover_curve(2.0) - 0.5).abs() < 1e-5 && hover_curve(8.0).abs() < 1e-5);
        // It turned towards the survivor (straight down -Z: +90° about Y).
        assert!((a.yaw - std::f32::consts::FRAC_PI_2).abs() < 0.01, "{}", a.yaw);
        // A second midnight while it's up doesn't restart the Delay.
        a.midnight();
        a.step(56.1, None, 1.0 / 60.0);
        assert!(a.hidden && !a.active());
    }

    #[test]
    fn facing_eases_the_short_way_round() {
        assert_eq!(look_yaw(Vec3::ZERO, Vec3::X), 0.0);
        let y = interp_yaw(3.0, -3.0, 1.0 / 60.0, FACE_SPEED);
        assert!(y > 3.0, "across the ±π seam, not back through 0: {y}");
        assert!((interp_yaw(0.0, 1.0, 1.0 / 60.0, FACE_SPEED) - 0.25).abs() < 1e-5);
    }

    #[test]
    fn the_campfire_hides_from_a_survivor_within_15_m() {
        let mut a = Apparition::new(EggKind::Campfire, "c".into(), Vec3::ZERO, 0.0);
        a.midnight();
        a.step(0.25, Some(Vec3::new(20.0, 0.0, 0.0)), 1.0 / 60.0);
        assert!(!a.hidden);
        a.step(0.25, Some(Vec3::new(10.0, 0.0, 0.0)), 1.0 / 60.0);
        assert!(a.hidden);
        a.step(0.25, Some(Vec3::new(16.0, 0.0, 0.0)), 1.0 / 60.0);
        assert!(!a.hidden);
        let mut e = EasterEggs::default();
        e.apparitions.push(a);
        assert!(e.fire_shown("c") && !e.fire_shown("x"));
    }

    #[test]
    fn the_nuke_goes_off_once() {
        let mut n = Nuke { transform: Transform::from_xyz(10.0, 0.0, 0.0), half: Vec3::new(2.0, 1.0, 1.0), key: None, exploded: false };
        assert!(n.shot_hits(Vec3::ZERO, Vec3::X, 8.0), "stopped at its face");
        assert!(!n.shot_hits(Vec3::ZERO, Vec3::X, 5.0), "stopped short");
        assert!(!n.shot_hits(Vec3::new(0.0, 3.0, 0.0), Vec3::X, 20.0), "over it");
        assert_eq!(n.nearest(Vec3::new(20.0, 0.0, 0.0)), Vec3::new(12.0, 0.0, 0.0));
        assert!(n.explode());
        assert!(!n.explode(), "a DoOnce");
    }

    #[test]
    fn level_easter_eggs() {
        let Some(level) = super::super::Level::load() else { return };
        let count = |c: &str| level.actors.iter().filter(|a| a.class == c).count();
        assert_eq!((count("BP_EasterEgg_GlowingEyes_C"), count("BP_EasterEgg_Campfire_C"), count("BP_EasterEgg_Nuke_C")), (6, 1, 1));
        for a in level.actors.iter().filter(|a| a.class == "BP_EasterEgg_GlowingEyes_C") {
            assert!(level.prop_near(a.position(), "SM_Prop_DeadBody_Hanging_Male_02", 0.5).is_some(), "{}", a.name);
            assert_eq!(level.props_near(a.position(), "Cube", 0.5).len(), 2, "{} eyes", a.name);
        }
        let nuke = level.actors.iter().find(|a| a.class == "BP_EasterEgg_Nuke_C").unwrap();
        assert!(level.prop_near(nuke.position(), "SM_Prop_Nuke_01", 0.5).is_some());
    }
}
