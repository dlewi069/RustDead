//! The scope overlays, aiming sway and holding breath, from BP_PlayerCharacter's ScopeFunction /
//! SetEnableScope / CameraShake / IA_HoldBreath / Event_HoldBreath, BP_WeaponsPickupComponent's
//! ScopeFunction and the ScopesUI widgets (decompiled):
//!
//! - ScopesUI holds three full-screen overlays, all Hidden: Acog (AcogScope: AcogDetailedOverlay),
//!   LongScope (NormalScope: SniperScopeDetailedOverlay) and PUScope (MosinSniperScope:
//!   PUScopeDetailedOverlay), each an Image stretched over the screen and a BackgroundBlurWithMask
//!   (BlurStrength 20) whose mask leaves the glass clear. The component's ScopeFunction picks the
//!   overlay by the scope's ItemID: Acog4x32Scope and the five named Acogs -> Acog, PUScope ->
//!   PUScope, every other scope -> LongScope; the XM250's built-in sight is an Acog, the
//!   Cerberus's a LongScope (`attachments::zoom`).
//! - SetEnableScope(true): the overlay SelfHitTestInvisible, the survivor's mesh and the weapon
//!   (and its child actors) hidden, the crosshair and the action bar Hidden. False: all three
//!   overlays Hidden, the survivor and weapon shown, crosshair and action bar back.
//! - Camera shakes (legacy oscillators, OscillationDuration -1 = until stopped, blend in / out
//!   0.5 s, single instance, RotOscillation pitch / yaw): CameraShake(Sniper?) picks by the
//!   MarksmanshipMultiplier: < 1.01 tier 1, < 1.15 tier 2, < 1.25 tier 3, < 1.4 tier 4, >= 1.5
//!   tier 5, and nothing at 1.4-1.5 (the BP's branch for that range falls through; kept).
//!   Aiming starts AimingSway (Client_ADSCameraShake(false)); turning a scope on stops all shakes
//!   at once and starts SniperScopeSway; turning it off stops them and starts AimingSway again;
//!   letting go of the aim stops all at once. A sight (SightFunction) changes no shake.
//! - IA_HoldBreath (Left Shift) Started while aiming with PlayerUsingScope? (scopes and sights
//!   alike) sets Event_HoldBreath every 0.01 s: with stamina left, Svr_ReduceStamina(0.3 /
//!   MarksmanshipMultiplier) and StopAllCameraShake (blending out); at 0 stamina, AimingSway
//!   again and the timer cleared. Completed / Canceled while still scoped plays SniperScopeSway_1
//!   *and* AimingSway_1 (whatever the skill) and clears the timer.
//! - A legacy oscillator: offset = rand(0, 2 pi) at start, += Frequency x dt, value Amplitude x
//!   sin(offset) degrees (FFOscillator::UpdateOffset), times the blend weight.
//!
//! Ours: BackgroundBlurWithMask is a darkening through the mask (Bevy UI has no backdrop blur;
//! the 0.55 is a guess); the sway is added to the camera's rotation after the spring arm, so
//! shots down the camera ray follow it as UE's POV does.
//!
//! Not ported: the depth of field the sights set (Video.DOF), first person.

use bevy::prelude::*;

use super::attachments::{Scope, Zoom};
use super::combat::{Armed, PlayerCamera};
use crate::menus::{AppState, CurrentGame};
use crate::models::CharacterModel;
use crate::progression::passive::Passive;

/// A legacy camera shake's two rotation oscillators: (amplitude in degrees, frequency).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShakeDef {
    pub name: &'static str,
    pub pitch: (f32, f32),
    pub yaw: (f32, f32),
}

/// FX/CameraShake/SniperSway/CameraShake_SniperScopeSway_1..5 (class defaults).
pub const SNIPER_SWAY: [ShakeDef; 5] = [
    ShakeDef { name: "CameraShake_SniperScopeSway_1", pitch: (0.225, 1.0), yaw: (0.125, 0.25) },
    ShakeDef { name: "CameraShake_SniperScopeSway_2", pitch: (0.215, 0.9), yaw: (0.12, 0.225) },
    ShakeDef { name: "CameraShake_SniperScopeSway_3", pitch: (0.205, 0.8), yaw: (0.115, 0.2) },
    ShakeDef { name: "CameraShake_SniperScopeSway_4", pitch: (0.19, 0.7), yaw: (0.1, 0.175) },
    ShakeDef { name: "CameraShake_SniperScopeSway_5", pitch: (0.18, 0.5), yaw: (0.075, 0.15) },
];

/// FX/CameraShake/Sway/CameraShake_AimingSway_1..5.
pub const AIMING_SWAY: [ShakeDef; 5] = [
    ShakeDef { name: "CameraShake_AimingSway_1", pitch: (0.2, 1.0), yaw: (0.1, 0.25) },
    ShakeDef { name: "CameraShake_AimingSway_2", pitch: (0.175, 0.8), yaw: (0.08, 0.225) },
    ShakeDef { name: "CameraShake_AimingSway_3", pitch: (0.15, 0.6), yaw: (0.06, 0.2) },
    ShakeDef { name: "CameraShake_AimingSway_4", pitch: (0.125, 0.4), yaw: (0.04, 0.175) },
    ShakeDef { name: "CameraShake_AimingSway_5", pitch: (0.1, 0.2), yaw: (0.02, 0.1) },
];

/// OscillationBlendInTime / OscillationBlendOutTime of every sway.
pub const BLEND: f32 = 0.5;
/// Event_HoldBreath's timer.
pub const BREATH_TICK: f32 = 0.01;
/// Svr_ReduceStamina's 0.3 a tick, before the skill.
pub const BREATH_STAMINA: f32 = 0.3;
/// BackgroundBlurWithMask's stand-in darkness (ours).
const BLUR_DARKNESS: f32 = 0.55;

/// CameraShake(Sniper?)'s pick by MarksmanshipMultiplier (None in the 1.4-1.5 gap).
pub fn sway_for(sniper: bool, marksmanship: f32) -> Option<&'static ShakeDef> {
    let tier = if marksmanship < 1.01 {
        0
    } else if marksmanship < 1.15 {
        1
    } else if marksmanship < 1.25 {
        2
    } else if marksmanship < 1.4 {
        3
    } else if marksmanship >= 1.5 {
        4
    } else {
        return None;
    };
    Some(if sniper { &SNIPER_SWAY[tier] } else { &AIMING_SWAY[tier] })
}

/// One playing shake.
#[derive(Clone, Debug)]
pub struct ActiveShake {
    pub def: &'static ShakeDef,
    /// The oscillators' current offsets (radians).
    offsets: (f32, f32),
    t: f32,
    /// Seconds into the blend out.
    out: Option<f32>,
}

impl ActiveShake {
    fn weight(&self) -> f32 {
        let w = (self.t / BLEND).min(1.0);
        match self.out {
            Some(o) => w.min(1.0 - o / BLEND),
            None => w,
        }
        .max(0.0)
    }
}

/// The camera manager's shakes, and the sway they come to (pitch, yaw degrees).
#[derive(Resource, Default, Debug)]
pub struct Sway {
    pub shakes: Vec<ActiveShake>,
    pub offset: Vec2,
}

impl Sway {
    /// PlayCameraShake: a single instance restarts its blend in.
    pub fn play(&mut self, def: &'static ShakeDef, phase: (f32, f32)) {
        if let Some(s) = self.shakes.iter_mut().find(|s| s.def.name == def.name) {
            s.t = 0.0;
            s.out = None;
            return;
        }
        self.shakes.push(ActiveShake { def, offsets: phase, t: 0.0, out: None });
    }

    /// StopAllCameraShakes(bImmediately).
    pub fn stop_all(&mut self, immediately: bool) {
        if immediately {
            self.shakes.clear();
        } else {
            for s in &mut self.shakes {
                s.out.get_or_insert(0.0);
            }
        }
    }

    /// Advance every shake; the summed rotation.
    pub fn step(&mut self, dt: f32) -> Vec2 {
        let mut sum = Vec2::ZERO;
        for s in &mut self.shakes {
            s.t += dt;
            if let Some(o) = &mut s.out {
                *o += dt;
            }
            s.offsets.0 += dt * s.def.pitch.1;
            s.offsets.1 += dt * s.def.yaw.1;
            let w = s.weight();
            sum += w * Vec2::new(s.def.pitch.0 * s.offsets.0.sin(), s.def.yaw.0 * s.offsets.1.sin());
        }
        self.shakes.retain(|s| s.out.is_none_or(|o| o < BLEND));
        self.offset = sum;
        sum
    }

    fn play_tier(&mut self, sniper: bool, marksmanship: f32) {
        if let Some(def) = sway_for(sniper, marksmanship) {
            self.play(def, random_phase());
        }
    }
}

fn random_phase() -> (f32, f32) {
    use rand::Rng;
    let mut r = rand::thread_rng();
    (r.gen_range(0.0..std::f32::consts::TAU), r.gen_range(0.0..std::f32::consts::TAU))
}

/// Event_HoldBreath's timer (StopScopeSwayTimer) and its accumulated time.
#[derive(Resource, Default, Debug)]
pub struct HoldBreath {
    pub on: bool,
    acc: f32,
}

/// One Event_HoldBreath tick: false when stamina ran out (the timer stops).
pub fn breath_tick(stamina: &mut f32, marksmanship: f32) -> bool {
    if *stamina <= 0.0 {
        return false;
    }
    *stamina = (*stamina - BREATH_STAMINA / marksmanship.max(0.01)).max(0.0);
    true
}

/// A ScopesUI overlay.
#[derive(Component)]
pub struct ScopeOverlay(pub Scope);

/// The mask images, turned into a darkening (the blur's stand-in) once loaded.
#[derive(Component)]
struct BlurMask {
    source: Handle<Image>,
    done: bool,
}

/// SetEnableScope's state: which overlay is up.
#[derive(Resource, Default, Debug)]
pub struct ScopeShown(pub Option<Scope>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Sway>()
        .init_resource::<HoldBreath>()
        .init_resource::<ScopeShown>()
        .add_systems(OnEnter(AppState::InGame), (spawn_scopes_ui, |mut s: ResMut<Sway>, mut b: ResMut<HoldBreath>, mut v: ResMut<ScopeShown>| {
            *s = Sway::default();
            *b = HoldBreath::default();
            v.0 = None;
        }))
        .add_systems(
            Update,
            (shakes_follow_aim, hold_breath, enable_scope, bake_masks)
                .chain()
                .after(super::attachments::scope_zoom)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(PostUpdate, apply_sway.before(bevy::transform::TransformSystems::Propagate).run_if(in_state(AppState::InGame)));
}

const TEX: &str = "game/SurrounDead/Content/UI/Textures/Scopes";

fn overlay_files(scope: Scope) -> (&'static str, &'static str) {
    match scope {
        Scope::Acog => ("AcogDetailedOverlay", "AcogScopeDetailedOverlayMask"),
        Scope::Long => ("SniperScopeDetailedOverlay", "SniperScopeDetailedOverlayMask"),
        Scope::Pu => ("PUScopeDetailedOverlay", "PUScopeDetailedOverlayMask"),
    }
}

/// ScopesUI: the three overlays, Hidden, under the rest of the HUD.
fn spawn_scopes_ui(mut commands: Commands, assets: Res<AssetServer>) {
    let full = || Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    let root = commands.spawn((full(), GlobalZIndex(-1), Pickable::IGNORE, DespawnOnExit(AppState::InGame))).id();
    for scope in [Scope::Acog, Scope::Long, Scope::Pu] {
        let (image, mask) = overlay_files(scope);
        let source: Handle<Image> = assets.load(format!("{TEX}/{mask}.png"));
        commands.spawn((full(), ScopeOverlay(scope), Visibility::Hidden, ChildOf(root))).with_children(|o| {
            o.spawn((full(), ImageNode::default(), BlurMask { source, done: false }));
            o.spawn((full(), ImageNode::new(assets.load(format!("{TEX}/{image}.png")))));
        });
    }
}

/// The blur's stand-in: black, with the mask's white (outside the glass) as alpha.
fn bake_masks(mut images: ResMut<Assets<Image>>, mut q: Query<(&mut BlurMask, &mut ImageNode)>) {
    for (mut m, mut node) in &mut q {
        if m.done {
            continue;
        }
        let Some(src) = images.get(&m.source) else { continue };
        let Ok(rgba) = src.clone().try_into_dynamic().map(|d| d.to_rgba8()) else {
            m.done = true;
            continue;
        };
        let (w, h) = rgba.dimensions();
        let mut out = image::RgbaImage::new(w, h);
        for (o, p) in out.pixels_mut().zip(rgba.pixels()) {
            *o = image::Rgba([0, 0, 0, (p[0] as f32 * BLUR_DARKNESS) as u8]);
        }
        node.image = images.add(Image::from_dynamic(image::DynamicImage::ImageRgba8(out), true, bevy::asset::RenderAssetUsages::RENDER_WORLD));
        m.done = true;
    }
}

fn marksmanship(game: &CurrentGame) -> f32 {
    game.state.as_ref().map_or(1.0, |s| s.passive.multiplier(Passive::Marksmanship))
}

/// Client_ADSCameraShake on aiming and on the scope going on / off; letting go of the aim stops
/// them all.
fn shakes_follow_aim(armed: Res<Armed>, zoom: Res<Zoom>, game: Res<CurrentGame>, mut sway: ResMut<Sway>, mut breath: ResMut<HoldBreath>, mut last: Local<(bool, bool)>) {
    let scoped = zoom.0.is_some_and(|z| z.1.is_some());
    let mm = marksmanship(&game);
    if armed.aiming && !last.0 {
        sway.play_tier(false, mm);
    }
    if armed.aiming && last.0 && scoped != last.1 {
        // ScopeFunction: StopAllCameraShakes(true), then the scope's sway or the aiming one.
        sway.stop_all(true);
        sway.play_tier(scoped, mm);
        breath.on = false;
    }
    if !armed.aiming && last.0 {
        sway.stop_all(true);
        breath.on = false;
    }
    *last = (armed.aiming, scoped);
}

/// IA_HoldBreath and Event_HoldBreath.
fn hold_breath(
    keys: Res<ButtonInput<KeyCode>>,
    armed: Res<Armed>,
    zoom: Res<Zoom>,
    time: Res<Time>,
    mut game: ResMut<CurrentGame>,
    mut sway: ResMut<Sway>,
    mut breath: ResMut<HoldBreath>,
) {
    let using = armed.aiming && zoom.0.is_some();
    if keys.just_pressed(KeyCode::ShiftLeft) && using {
        breath.on = true;
        breath.acc = 0.0;
        info!("scopes: holding breath");
    }
    if keys.just_released(KeyCode::ShiftLeft) && using {
        sway.play(&SNIPER_SWAY[0], random_phase());
        sway.play(&AIMING_SWAY[0], random_phase());
        breath.on = false;
    }
    if !breath.on {
        return;
    }
    let mm = marksmanship(&game);
    let Some(state) = game.state.as_mut() else { return };
    breath.acc += time.delta_secs();
    while breath.acc >= BREATH_TICK {
        breath.acc -= BREATH_TICK;
        if breath_tick(&mut state.survival.stamina, mm) {
            sway.stop_all(false);
        } else {
            sway.play_tier(false, mm);
            breath.on = false;
            info!("scopes: out of breath");
            break;
        }
    }
}

/// SetEnableScope: the overlay, the survivor and weapon, the crosshair and action bar.
#[allow(clippy::type_complexity)]
fn enable_scope(
    armed: Res<Armed>,
    zoom: Res<Zoom>,
    mut shown: ResMut<ScopeShown>,
    mut overlays: Query<(&ScopeOverlay, &mut Visibility), (Without<CharacterModel>, Without<super::hud::HideWhenScoped>)>,
    mut survivor: Query<&mut Visibility, (With<CharacterModel>, With<crate::level::KinematicBody>, Without<super::hud::HideWhenScoped>)>,
    mut hud: Query<&mut Visibility, (With<super::hud::HideWhenScoped>, Without<CharacterModel>)>,
) {
    let want = if armed.aiming { zoom.0.and_then(|z| z.1) } else { None };
    if want == shown.0 {
        return;
    }
    shown.0 = want;
    info!("scopes: overlay {want:?}");
    for (o, mut v) in &mut overlays {
        *v = if Some(o.0) == want { Visibility::Inherited } else { Visibility::Hidden };
    }
    let other = if want.is_some() { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in survivor.iter_mut().chain(hud.iter_mut()) {
        *v = other;
    }
}

/// The shakes' rotation on top of the spring arm's.
fn apply_sway(time: Res<Time>, mut sway: ResMut<Sway>, mut cameras: Query<&mut Transform, With<PlayerCamera>>) {
    let off = sway.step(time.delta_secs());
    if off == Vec2::ZERO {
        return;
    }
    for mut t in &mut cameras {
        // UE pitch up / yaw right: about the camera's own right axis, then the world up.
        t.rotate_local_x(off.x.to_radians());
        t.rotate_y(-off.y.to_radians());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marksmanship_picks_the_sway_tier() {
        assert_eq!(sway_for(true, 1.0).unwrap().name, "CameraShake_SniperScopeSway_1");
        assert_eq!(sway_for(false, 1.04).unwrap().name, "CameraShake_AimingSway_2");
        assert_eq!(sway_for(true, 1.2).unwrap().name, "CameraShake_SniperScopeSway_3");
        assert_eq!(sway_for(true, 1.3).unwrap().name, "CameraShake_SniperScopeSway_4");
        assert!(sway_for(true, 1.45).is_none(), "the BP's 1.4-1.5 gap");
        assert_eq!(sway_for(false, 2.0).unwrap().name, "CameraShake_AimingSway_5");
    }

    #[test]
    fn shakes_blend_in_oscillate_and_blend_out() {
        let mut s = Sway::default();
        s.play(&SNIPER_SWAY[0], (std::f32::consts::FRAC_PI_2, 0.0));
        // Blend weight 0.5 at 0.25 s; pitch offset pi/2 + 0.25.
        let v = s.step(0.25);
        assert!((v.x - 0.5 * 0.225 * (std::f32::consts::FRAC_PI_2 + 0.25).sin()).abs() < 1e-5, "{v}");
        s.step(0.5);
        s.play(&SNIPER_SWAY[0], (0.0, 0.0));
        assert_eq!(s.shakes.len(), 1, "single instance");
        s.stop_all(false);
        s.step(0.3);
        assert_eq!(s.shakes.len(), 1, "still blending out");
        s.step(0.3);
        assert!(s.shakes.is_empty());
        s.play(&AIMING_SWAY[0], (0.0, 0.0));
        s.stop_all(true);
        assert!(s.shakes.is_empty());
    }

    #[test]
    fn holding_breath_costs_stamina_until_none() {
        let mut st = 1.0;
        assert!(breath_tick(&mut st, 1.0));
        assert!((st - 0.7).abs() < 1e-6);
        assert!(breath_tick(&mut st, 1.5));
        assert!((st - 0.5).abs() < 1e-6);
        st = 0.0;
        assert!(!breath_tick(&mut st, 1.0));
    }

    /// The table matches the dumped assets (when `scripts/dump_bp.sh target/bpdump/shake
    /// SurrounDead/Content/FX/CameraShake/` has been run).
    #[test]
    fn sway_table_matches_the_assets() {
        for d in SNIPER_SWAY.iter().chain(&AIMING_SWAY) {
            let dir = if d.name.contains("Sniper") { "SniperSway" } else { "Sway" };
            let path = format!("{}/target/bpdump/shake/FX/CameraShake/{dir}/{}.json", env!("CARGO_MANIFEST_DIR"), d.name);
            let Ok(text) = std::fs::read_to_string(&path) else { return };
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            let cdo = json.as_array().unwrap().iter().find(|e| e["Name"].as_str().is_some_and(|n| n.starts_with("Default__"))).unwrap();
            let p = &cdo["Properties"];
            let r = &p["RotOscillation"];
            let f = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
            assert_eq!((f(&r["Pitch"]["Amplitude"]), f(&r["Pitch"]["Frequency"])), d.pitch, "{}", d.name);
            assert_eq!((f(&r["Yaw"]["Amplitude"]), f(&r["Yaw"]["Frequency"])), d.yaw, "{}", d.name);
            assert_eq!(f(&p["OscillationBlendInTime"]), BLEND);
            assert_eq!(f(&p["OscillationBlendOutTime"]), BLEND);
        }
    }
}
