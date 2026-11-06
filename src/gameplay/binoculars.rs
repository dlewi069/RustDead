//! Binoculars and the enemy tags they place, from BP_PlayerCharacter (IA_Binoculars, UseBinos,
//! BinocularsVisibility, BinosDistanceTimer, Event_TimerBinos, ExitBinos, MarkEnemy), the
//! Binoculars and AIMarkerUI widgets and the AIs' BPI_AI marker functions (decompiled):
//!
//! - **B** (IA_Binoculars) is UseBinos: nothing while in a vehicle, fishing, swimming, falling,
//!   zoomed in, building, with the journal, inventory or map open, aiming, sprinting or
//!   crouch-sprinting; without binoculars in their slot "You do not have Binoculars equipped"
//!   (2 s); else BinocularsVisibility flips.
//! - **BinocularsVisibility(true, the item's BinocularsZoomValue)**: the Binoculars widget
//!   (BinocularsOverlay stretched over the screen; bottom left, 24 px in, "{Zoom}x Zoom" and
//!   "[LMB] Tag Enemy"; bottom right "{A}m"; Montserrat-Bold 16), UsingBinoculars?, the HUD hidden,
//!   both cameras' depth-of-field f-stop 320 (no blur), first person (ChangePlayerPerspective:
//!   FirstPersonCamera on SpringArmFP at the mesh's `camera` socket, 14 cm ahead and 8 cm up along
//!   the view; the head and what hangs on it hidden; the view pitch down to 80°), and its field of
//!   view divided by the zoom (FIRST PERSON FIELD OF VIEW / zoom). False undoes it all, back to
//!   third person. The plain binoculars zoom 4x, the military ones 12x with the range readout.
//! - While they're up: the widget's Interact (F) is ExitBinos; LMB tags (MarkEnemy) instead of
//!   firing; aiming (RMB), the inventory, the journal, sitting down, the camera's wheel and the
//!   roll do nothing; the mouse turns at SCOPE SENSITIVITY MULTIPLIER.
//! - **BinosDistanceTimer** (ShowDistance?): every 0.1 s a 1 cm sphere trace 10 km along the
//!   view (WorldStatic, WorldDynamic, Pawn, PhysicsBody); Distance = FTrunc(the hit's distance,
//!   cm), shown as Distance / 100 "m" (0 with no hit). Without it the readout is hidden.
//! - **MarkEnemy**: a 5 cm sphere trace 10 km along the view against the same types; the first
//!   hit, if a BPI_AI whose CanAddMarkerToAI? says yes, gets a screen-space WidgetComponent
//!   (AIMarkerUI: 15 x 15 px, GetMarker's Texture tinted Color) MarkerOffset cm above it, handed
//!   to AddMarker: CanAddMarker? false, Event_Marker every 1 s: while the player can see the AI's
//!   controller (LineOfSightTo; it stays where the AI spawned) a 60 s RetriggerableDelay restarts;
//!   when it runs out the marker goes and CanAddMarker? is true again. Death takes it too.
//!   Traders always answer yes: tagging one again orphans the old widget, which stays.
//!
//! Ours: the traces are rays (props, the terrain in 1 m steps, the AIs' capsules 5 cm wider; no
//! nest is taggable: CanAddMarkerToAI? false); the line of sight is props and terrain from the
//! camera to the spawn point (`LevelZombie::home`, capsule centre); markers hide behind the
//! camera; a zombie culled to storage loses its marker. Not ported: the radial menu's Use
//! Binoculars, photo mode, the depth of field (ours has none), the notification's icon.

use bevy::input::InputSystems;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

use super::combat::Dead;
use super::equipment::Slot;
use super::interact::{GameOverlay, player_inv};
use super::items::ItemDb;
use super::notify::Notify;
use super::zombies::{CanAdd, MarkerDef};
use crate::level::collision::PropRays;
use crate::level::{KinematicBody, LevelZombie, Terrain};
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::widgets::{MenuFont, pt};
use crate::menus::{AppState, CurrentGame};
use crate::models::{CharacterModel, HeadPart, Locomotion};

/// The traces' length (1000000 cm).
pub const REACH: f32 = 10_000.0;
/// MarkEnemy's sphere radius (5 cm).
const MARK_RADIUS: f32 = 0.05;
/// BinosDistanceTimer's period.
const DISTANCE_EVERY: f32 = 0.1;
/// Event_Marker's timer and the RetriggerableDelay.
const LOOK_EVERY: f32 = 1.0;
const MARKER_LIFE: f32 = 60.0;
/// AIMarkerUI's DrawSize.
const MARKER_PX: f32 = 15.0;
/// FirstPersonCamera's place on SpringArmFP: 14 cm ahead, 8 cm up, along the view.
const FP_AHEAD: f32 = 0.14;
const FP_UP: f32 = 0.08;
const OVERLAY: &str = "game/SurrounDead/Content/UI/Textures/BinocularsOverlay.png";
pub const NOT_EQUIPPED: &str = "You do not have Binoculars equipped";

/// UsingBinoculars? and the widget's values.
#[derive(Resource, Default, Debug)]
pub struct Binoculars {
    pub active: bool,
    /// BinocularsZoomValue.
    pub zoom: f32,
    /// ShowDistance?.
    pub show_distance: bool,
    /// Distance (cm, truncated); 0 when the trace found nothing.
    pub distance: i32,
    distance_in: f32,
    /// LMB this frame: MarkEnemy.
    mark: bool,
}

/// What UseBinos refuses on.
#[derive(Clone, Copy, Debug, Default)]
pub struct Busy {
    pub vehicle: bool,
    pub fishing: bool,
    pub swimming: bool,
    pub falling: bool,
    pub zoomed: bool,
    pub building: bool,
    /// The journal, inventory or map (any overlay here).
    pub overlay: bool,
    pub aiming: bool,
    pub sprinting: bool,
}

impl Busy {
    pub fn any(&self) -> bool {
        self.vehicle || self.fishing || self.swimming || self.falling || self.zoomed || self.building || self.overlay || self.aiming || self.sprinting
    }
}

/// What a press of B does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Use {
    Nothing,
    NotEquipped,
    /// BinocularsVisibility(true, zoom) with ShowDistance?.
    Raise(f32, bool),
    Lower,
}

/// UseBinos.
pub fn use_binos(busy: Busy, equipped: Option<super::items::BinocularsDef>, active: bool) -> Use {
    if busy.any() {
        return Use::Nothing;
    }
    match equipped {
        None => Use::NotEquipped,
        Some(_) if active => Use::Lower,
        Some(b) => Use::Raise(b.zoom, b.distance),
    }
}

/// The widget's texts: "{Zoom}x Zoom" (FTrunc) and "{A}m" (Distance / 100, whole metres).
pub fn zoom_text(zoom: f32) -> String {
    format!("{}x Zoom", zoom.trunc() as i64)
}

pub fn distance_text(cm: i32) -> String {
    format!("{}m", cm / 100)
}

/// IA_CameraRotation's multiplier: SCOPE SENSITIVITY while scoped or using binoculars, AIMING
/// SENSITIVITY while aiming, else SENSITIVITY MULTIPLIER; and INVERT MOUSE Y's -1 for pitch.
pub fn look_sensitivity(store: &SettingsStore, aiming: bool, scope: bool, binoculars: bool) -> f32 {
    let row = if binoculars || (aiming && scope) {
        ("SCOPE SENSITIVITY MULTIPLIER", 0.5)
    } else if aiming {
        ("AIMING SENSITIVITY MULTIPLIER", 1.0)
    } else {
        ("SENSITIVITY MULTIPLIER", 1.5)
    };
    match store.find(&row_key("settings", "Controls", row.0)) {
        Some(Value::Num(v)) => v,
        _ => row.1,
    }
}

pub fn invert_y(store: &SettingsStore) -> f32 {
    if store.find(&row_key("settings", "Controls", "INVERT MOUSE Y")) == Some(Value::Bool(true)) { -1.0 } else { 1.0 }
}

/// FIRST PERSON FIELD OF VIEW (Character.FieldOfViewFP).
pub fn first_person_fov(store: &SettingsStore) -> f32 {
    match store.find(&row_key("settings", "Video", "FIRST PERSON FIELD OF VIEW")) {
        Some(Value::Num(v)) => v,
        _ => 90.0,
    }
}

/// A tagged AI: its marker widget (`live`), widgets a re-tag orphaned (traders), Event_Marker's
/// timer and the 60 s RetriggerableDelay.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct AiMarker {
    pub def: MarkerDef,
    pub live: bool,
    pub orphans: u32,
    tick: Option<f32>,
    pub delay: Option<f32>,
}

impl AiMarker {
    /// AddMarker.
    pub fn new(def: MarkerDef) -> Self {
        Self { def, live: true, orphans: 0, tick: Some(LOOK_EVERY), delay: None }
    }

    /// CanAddMarkerToAI?.
    pub fn can_add(&self) -> bool {
        self.def.can_add == CanAdd::Always || !self.live
    }

    /// Another AddMarker: the old widget (if any) is left behind, the timer restarts.
    pub fn retag(&mut self) {
        if self.live {
            self.orphans += 1;
        }
        self.live = true;
        self.tick = Some(LOOK_EVERY);
    }

    /// One frame; `seen` is asked at each 1 s tick (LineOfSightTo). True when the delay ran out
    /// and the marker went.
    pub fn step(&mut self, dt: f32, mut seen: impl FnMut() -> bool) -> bool {
        // The running delay first: one (re)started this frame counts from its end.
        if let Some(d) = self.delay.as_mut() {
            *d -= dt;
            if *d <= 0.0 {
                self.delay = None;
                self.tick = None;
                self.live = false;
                return true;
            }
        }
        if let Some(t) = self.tick.as_mut() {
            *t -= dt;
            while *t <= 0.0 {
                *t += LOOK_EVERY;
                if seen() {
                    self.delay = Some(MARKER_LIFE);
                }
            }
        }
        false
    }

    /// Death: Marker destroyed, the timer cleared (orphans stay with the body).
    pub fn died(&mut self) {
        self.live = false;
        self.tick = None;
        self.delay = None;
    }

    pub fn shown(&self) -> bool {
        self.live || self.orphans > 0
    }
}

/// Where a ray from `origin` along unit `dir` first meets the terrain, in 1 m steps.
pub fn terrain_hit(terrain: Option<&Terrain>, origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
    let t = terrain?;
    let mut s = 1.0;
    while s <= max {
        if t.blocks(origin + dir * s) {
            return Some(s);
        }
        s += 1.0;
    }
    None
}

/// A ray against an upright capsule (feet at `feet`): the distance, if it meets it.
pub fn ray_capsule(origin: Vec3, dir: Vec3, feet: Vec3, radius: f32, half_height: f32) -> Option<f32> {
    let (a, b) = (feet + Vec3::Y * radius.min(half_height), feet + Vec3::Y * (2.0 * half_height - radius).max(radius.min(half_height)));
    super::combat::ray_capsule_between(origin, dir, a, b, radius)
}

#[allow(clippy::too_many_arguments)]
fn busy(
    driving: &super::vehicles::Driving,
    fishing: &super::fishing::Fishing,
    swim: &super::swim::Swim,
    zoom: &super::attachments::Zoom,
    build: &super::building::BuildMode,
    overlay: &GameOverlay,
    armed: &super::combat::Armed,
    grounded: bool,
    loco: Locomotion,
) -> Busy {
    Busy {
        vehicle: driving.active(),
        fishing: fishing.active,
        swimming: swim.swimming,
        falling: !grounded,
        zoomed: zoom.0.is_some(),
        building: build.on(),
        overlay: overlay.is_open(),
        aiming: armed.aiming,
        sprinting: matches!(loco, Locomotion::Sprint | Locomotion::CrouchSprint),
    }
}

/// B, and while the binoculars are up F (ExitBinos) and LMB (MarkEnemy); then what they block
/// is taken out of this frame's input.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn binocular_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut scroll: ResMut<AccumulatedMouseScroll>,
    mut binos: ResMut<Binoculars>,
    db: Res<ItemDb>,
    mut game: ResMut<CurrentGame>,
    (overlay, driving, fishing, swim, zoom, build, armed): (
        Res<GameOverlay>,
        Res<super::vehicles::Driving>,
        Res<super::fishing::Fishing>,
        Res<super::swim::Swim>,
        Res<super::attachments::Zoom>,
        Res<super::building::BuildMode>,
        Res<super::combat::Armed>,
    ),
    player: Query<(&KinematicBody, &Locomotion), (With<CharacterModel>, With<KinematicBody>)>,
    screen: Res<State<crate::menus::MenuScreen>>,
    mut notify: MessageWriter<Notify>,
) {
    binos.mark = false;
    if *screen.get() != crate::menus::MenuScreen::None {
        return;
    }
    if keys.just_pressed(KeyCode::KeyB)
        && let Ok((body, loco)) = player.single()
    {
        let b = busy(&driving, &fishing, &swim, &zoom, &build, &overlay, &armed, !body.falling(), *loco);
        let item = player_inv(&mut game).equipped.get(&Slot::Binoculars).and_then(|s| db.items.get(&s.item)).and_then(|d| d.binoculars);
        match use_binos(b, item, binos.active) {
            Use::Nothing => info!("binoculars: busy ({b:?})"),
            Use::NotEquipped => {
                notify.write(Notify::new(NOT_EQUIPPED));
            }
            Use::Raise(zoom, distance) => {
                *binos = Binoculars { active: true, zoom, show_distance: distance, ..default() };
                info!("binoculars: up ({zoom}x, distance {distance})");
            }
            Use::Lower => {
                binos.active = false;
                info!("binoculars: down");
            }
        }
    }
    if !binos.active {
        return;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        // The widget's Interact: ExitBinos.
        binos.active = false;
        info!("binoculars: down (Interact)");
    } else {
        binos.mark = mouse.just_pressed(MouseButton::Left);
    }
    // What UsingBinoculars? blocks: firing and swinging, aiming, the inventory, Interact, the
    // journal, sitting down, the roll, the camera's wheel.
    for k in [KeyCode::KeyF, KeyCode::Tab, KeyCode::KeyJ, KeyCode::Minus, KeyCode::KeyC] {
        keys.reset(k);
    }
    for b in [MouseButton::Left, MouseButton::Right] {
        mouse.reset(b);
    }
    scroll.delta = Vec2::ZERO;
}

/// MarkEnemy.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn mark_enemy(
    mut commands: Commands,
    binos: Res<Binoculars>,
    terrain: Option<Res<Terrain>>,
    mut rays: PropRays,
    camera: Query<&GlobalTransform, With<super::PlayerCamera>>,
    mut ais: Query<(Entity, &Transform, &LevelZombie, Option<&mut AiMarker>), Without<Dead>>,
) {
    if !binos.mark {
        return;
    }
    let Ok(cam) = camera.single() else { return };
    let (origin, dir) = (cam.translation(), cam.forward().as_vec3());
    let wall = [rays.cast(origin, dir, REACH), terrain_hit(terrain.as_deref(), origin, dir, REACH)].into_iter().flatten().fold(REACH, f32::min);
    let hit = ais
        .iter()
        .filter_map(|(e, t, z, _)| {
            let c = z.kind.class();
            ray_capsule(origin, dir, t.translation, c.capsule_radius + MARK_RADIUS, c.capsule_half_height).map(|d| (e, d))
        })
        .filter(|(_, d)| *d <= wall)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((e, d)) = hit else {
        info!("binoculars: tag found no AI");
        return;
    };
    let Ok((_, _, z, marker)) = ais.get_mut(e) else { return };
    let Some(def) = z.kind.class().marker.clone() else {
        info!("binoculars: {} can't be tagged", z.kind.class_name());
        return;
    };
    match marker {
        Some(mut m) if m.can_add() => {
            m.retag();
            info!("binoculars: tagged {} again at {d:.0} m ({} left behind)", z.kind.class_name(), m.orphans);
        }
        Some(_) => info!("binoculars: {} already tagged", z.kind.class_name()),
        None => {
            commands.entity(e).insert(AiMarker::new(def));
            info!("binoculars: tagged {} at {d:.0} m", z.kind.class_name());
        }
    }
}

/// Event_Marker's line of sight, the 60 s delay, death.
#[allow(clippy::type_complexity)]
fn run_markers(
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    mut rays: PropRays,
    camera: Query<&GlobalTransform, With<super::PlayerCamera>>,
    mut marked: Query<(&LevelZombie, &mut AiMarker, Has<Dead>)>,
) {
    let Ok(cam) = camera.single() else { return };
    let eye = cam.translation();
    let dt = time.delta_secs();
    for (z, mut m, dead) in &mut marked {
        if dead {
            if m.live {
                m.died();
            }
            continue;
        }
        // LineOfSightTo(its controller): where it spawned, at its capsule's centre.
        let target = z.home + Vec3::Y * z.kind.class().capsule_half_height;
        let ended = m.step(dt, || {
            let to = target - eye;
            let len = to.length();
            len < 1e-3 || (rays.cast(eye, to / len, len).is_none() && super::zombies::clear_line(terrain.as_deref(), eye, target))
        });
        if ended {
            info!("binoculars: {}'s tag ran out", z.kind.class_name());
        }
    }
}

/// BinosDistanceTimer / Event_TimerBinos.
fn read_distance(time: Res<Time>, mut binos: ResMut<Binoculars>, terrain: Option<Res<Terrain>>, mut rays: PropRays, camera: Query<&GlobalTransform, With<super::PlayerCamera>>, ais: Query<(&Transform, &LevelZombie), Without<Dead>>) {
    if !binos.active || !binos.show_distance {
        return;
    }
    binos.distance_in -= time.delta_secs();
    if binos.distance_in > 0.0 {
        return;
    }
    binos.distance_in += DISTANCE_EVERY;
    let Ok(cam) = camera.single() else { return };
    let (origin, dir) = (cam.translation(), cam.forward().as_vec3());
    let pawn = ais.iter().filter_map(|(t, z)| ray_capsule(origin, dir, t.translation, z.kind.class().capsule_radius + 0.01, z.kind.class().capsule_half_height)).fold(f32::INFINITY, f32::min);
    let hit = [rays.cast(origin, dir, REACH), terrain_hit(terrain.as_deref(), origin, dir, REACH), pawn.is_finite().then_some(pawn)].into_iter().flatten().fold(f32::INFINITY, f32::min);
    let cm = if hit.is_finite() { (hit * 100.0).trunc() as i32 } else { 0 };
    if binos.distance != cm {
        binos.distance = cm;
    }
}

#[derive(Component)]
struct BinocularsUi;
#[derive(Component)]
struct ZoomText;
#[derive(Component)]
struct DistanceText;
/// A tagged AI's AIMarkerUI.
#[derive(Component)]
struct MarkerIcon(Entity);

fn text(font: &MenuFont, s: impl Into<String>) -> impl Bundle {
    (Text::new(s), TextFont { font: font.0.clone().into(), font_size: pt(16.0), ..default() }, TextColor(Color::WHITE))
}

/// The Binoculars widget, the HUD and the head (first person) while they're up.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn binocular_ui(
    mut commands: Commands,
    binos: Res<Binoculars>,
    font: Res<MenuFont>,
    assets: Res<AssetServer>,
    ui: Query<Entity, With<BinocularsUi>>,
    mut zoom: Query<&mut Text, (With<ZoomText>, Without<DistanceText>)>,
    mut distance: Query<(&mut Text, &mut Visibility), (With<DistanceText>, Without<ZoomText>, Without<super::hud::HudRoot>, Without<HeadPart>)>,
    mut hud: Query<&mut Visibility, (With<super::hud::HudRoot>, Without<HeadPart>)>,
    mut heads: Query<(&mut Visibility, &ChildOf), (With<HeadPart>, Without<super::hud::HudRoot>)>,
    player: Query<Entity, (With<CharacterModel>, With<KinematicBody>)>,
    overlay: Res<GameOverlay>,
    screen: Res<State<crate::menus::MenuScreen>>,
    mut was: Local<bool>,
) {
    let me = player.single().ok();
    if binos.active != *was {
        *was = binos.active;
        for e in &ui {
            commands.entity(e).despawn();
        }
        if binos.active {
            commands.spawn((
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                // Over the AI markers, under the rest of the UI (the HUD is hidden anyway).
                GlobalZIndex(-1),
                bevy::picking::Pickable::IGNORE,
                BinocularsUi,
                DespawnOnExit(AppState::InGame),
                children![
                    (Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode::new(assets.load(OVERLAY)).with_mode(bevy::ui::widget::NodeImageMode::Stretch)),
                    (
                        Node { position_type: PositionType::Absolute, left: Val::Px(24.0), bottom: Val::Px(24.0), flex_direction: FlexDirection::Column, ..default() },
                        children![(text(&font, zoom_text(binos.zoom)), ZoomText), text(&font, "[LMB] Tag Enemy")],
                    ),
                    (
                        Node { position_type: PositionType::Absolute, right: Val::Px(24.0), bottom: Val::Px(24.0), flex_direction: FlexDirection::Column, ..default() },
                        children![(text(&font, distance_text(binos.distance)), TextLayout { justify: Justify::Right, ..default() }, DistanceText, if binos.show_distance { Visibility::Inherited } else { Visibility::Hidden })],
                    ),
                ],
            ));
        } else {
            // InGameUI back (if nothing else covers it), the head back.
            let shown = !overlay.is_open() && *screen.get() == crate::menus::MenuScreen::None;
            for mut v in &mut hud {
                v.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
            }
            for (mut v, p) in &mut heads {
                if Some(p.parent()) == me {
                    v.set_if_neq(Visibility::Inherited);
                }
            }
        }
    }
    if !binos.active {
        return;
    }
    for mut v in &mut hud {
        v.set_if_neq(Visibility::Hidden);
    }
    for (mut v, p) in &mut heads {
        if Some(p.parent()) == me {
            v.set_if_neq(Visibility::Hidden);
        }
    }
    for mut t in &mut zoom {
        let want = zoom_text(binos.zoom);
        if t.0 != want {
            t.0 = want;
        }
    }
    for (mut t, mut v) in &mut distance {
        let want = distance_text(binos.distance);
        if t.0 != want {
            t.0 = want;
        }
        v.set_if_neq(if binos.show_distance { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// The tags' AIMarkerUI widgets: 15 px, centred MarkerOffset above the AI's capsule centre.
#[allow(clippy::type_complexity)]
fn draw_markers(
    mut commands: Commands,
    assets: Res<AssetServer>,
    camera: Query<(&Camera, &GlobalTransform), With<super::PlayerCamera>>,
    marked: Query<(Entity, &Transform, &LevelZombie, &AiMarker)>,
    mut icons: Query<(Entity, &MarkerIcon, &mut Node, &mut Visibility)>,
) {
    let Ok((cam, cam_t)) = camera.single() else { return };
    for (e, icon, mut node, mut vis) in &mut icons {
        let Ok((_, t, z, m)) = marked.get(icon.0) else {
            commands.entity(e).despawn();
            continue;
        };
        if !m.shown() {
            commands.entity(e).despawn();
            continue;
        }
        let at = t.translation + Vec3::Y * (z.kind.class().capsule_half_height + m.def.offset);
        match cam.world_to_viewport(cam_t, at) {
            Ok(px) => {
                node.left = Val::Px(px.x - MARKER_PX / 2.0);
                node.top = Val::Px(px.y - MARKER_PX / 2.0);
                vis.set_if_neq(Visibility::Inherited);
            }
            Err(_) => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
    for (e, _, _, m) in &marked {
        if m.shown() && !icons.iter().any(|(_, i, ..)| i.0 == e) {
            let [r, g, b, a] = m.def.color;
            commands.spawn((
                Node { position_type: PositionType::Absolute, width: Val::Px(MARKER_PX), height: Val::Px(MARKER_PX), ..default() },
                ImageNode::new(assets.load(format!("game/SurrounDead/Content/{}.png", m.def.texture))).with_color(Color::linear_rgba(r, g, b, a)),
                GlobalZIndex(-2),
                bevy::picking::Pickable::IGNORE,
                Visibility::Hidden,
                MarkerIcon(e),
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// FirstPersonCamera while they're up: SpringArmFP at the `camera` socket (the eyes), the camera
/// 14 cm ahead and 8 cm up along the view.
fn binocular_camera(
    binos: Res<Binoculars>,
    db: Res<ItemDb>,
    player: Query<Entity, (With<CharacterModel>, With<KinematicBody>)>,
    heads: Query<(Entity, &ChildOf), With<HeadPart>>,
    children: Query<&Children>,
    names: Query<&Name>,
    bones: Query<&GlobalTransform>,
    mut camera: Query<&mut Transform, With<super::PlayerCamera>>,
    mut eyes: Local<Option<Entity>>,
) {
    if !binos.active {
        return;
    }
    let (Ok(me), Ok(mut cam)) = (player.single(), camera.single_mut()) else { return };
    if eyes.is_none_or(|e| bones.get(e).is_err()) {
        let head = heads.iter().find(|(_, p)| p.parent() == me).map(|(h, _)| h);
        *eyes = head.and_then(|h| children.iter_descendants(h).find(|&c| names.get(c).is_ok_and(|n| n.as_str() == "eyes")));
    }
    let Some(bone) = eyes.and_then(|e| bones.get(e).ok()) else { return };
    let socket = db.sockets.get("camera").map_or(Transform::IDENTITY, |s| s.transform.to_transform());
    let at = bone.mul_transform(socket).translation();
    let (fwd, up) = (cam.forward().as_vec3(), cam.up().as_vec3());
    cam.translation = at + fwd * FP_AHEAD + up * FP_UP;
}

fn reset(mut binos: ResMut<Binoculars>) {
    *binos = Binoculars::default();
}

/// The survivor died: the binoculars come down.
fn drop_on_death(vitals: Res<super::PlayerVitals>, mut binos: ResMut<Binoculars>) {
    if binos.active && vitals.is_dead() {
        binos.active = false;
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Binoculars>()
        .add_systems(OnEnter(AppState::InGame), reset)
        .add_systems(PreUpdate, binocular_input.after(InputSystems).run_if(in_state(AppState::InGame)))
        .add_systems(Update, (drop_on_death, mark_enemy, run_markers, read_distance, binocular_ui, draw_markers).chain().run_if(in_state(AppState::InGame)))
        .add_systems(PostUpdate, binocular_camera.before(bevy::transform::TransformSystems::Propagate).run_if(in_state(AppState::InGame)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::items::BinocularsDef;

    fn def(can_add: CanAdd) -> MarkerDef {
        MarkerDef { texture: "UI/Textures/x".into(), color: [0.71875, 0.0, 0.0, 1.0], offset: 1.3, can_add }
    }

    #[test]
    fn b_raises_and_lowers_them_unless_busy() {
        let plain = Some(BinocularsDef { zoom: 4.0, distance: false });
        assert_eq!(use_binos(Busy::default(), plain, false), Use::Raise(4.0, false));
        assert_eq!(use_binos(Busy::default(), plain, true), Use::Lower);
        assert_eq!(use_binos(Busy::default(), None, false), Use::NotEquipped);
        // Busy beats everything, even the missing binoculars' message.
        for b in [Busy { sprinting: true, ..default() }, Busy { overlay: true, ..default() }, Busy { falling: true, ..default() }, Busy { vehicle: true, ..default() }] {
            assert_eq!(use_binos(b, None, false), Use::Nothing);
            assert_eq!(use_binos(b, plain, true), Use::Nothing, "can't even put them down");
        }
        assert_eq!((zoom_text(12.0), zoom_text(4.9), distance_text(2599), distance_text(0)), ("12x Zoom".into(), "4x Zoom".into(), "25m".into(), "0m".into()));
    }

    #[test]
    fn a_tag_lasts_a_minute_past_the_last_sight() {
        let mut m = AiMarker::new(def(CanAdd::Flag));
        assert!(!m.can_add() && m.shown());
        // Seen at the first tick: 60 s from then.
        assert!(!m.step(1.0, || true));
        assert!(!m.step(30.0, || false));
        // Seen again at 31 s: the delay restarts.
        assert!(!m.step(1.0, || true));
        assert!(!m.step(59.0, || false));
        assert!(m.step(1.5, || false), "60 s after the last sight");
        assert!(m.can_add() && !m.shown());
        // Never seen: never removed (the delay never starts).
        let mut n = AiMarker::new(def(CanAdd::Flag));
        assert!(!n.step(500.0, || false) && n.shown());
        n.died();
        assert!(!n.shown() && n.can_add());
    }

    #[test]
    fn traders_take_tags_again_and_keep_the_old_ones() {
        let mut m = AiMarker::new(def(CanAdd::Always));
        assert!(m.can_add());
        m.retag();
        assert_eq!(m.orphans, 1);
        assert!(!m.step(1.0, || true));
        assert!(m.step(60.0, || false));
        assert!(m.shown(), "the orphaned widget stays");
    }

    #[test]
    fn every_ai_class_but_none_can_be_tagged() {
        let classes = crate::gameplay::zombies::zombie_classes();
        if classes.is_empty() {
            return;
        }
        for (name, c) in classes {
            let m = c.marker.as_ref().unwrap_or_else(|| panic!("{name} has no marker"));
            assert!(std::path::Path::new(&format!("assets/game/SurrounDead/Content/{}.png", m.texture)).exists(), "{name}: {}", m.texture);
            assert_eq!(m.can_add == CanAdd::Always, name == "BP_MasterTrader_C", "{name}");
        }
        let skull = &classes.iter().find(|(n, _)| n == "BP_Zombie_Roamer_C").unwrap().1;
        assert_eq!(skull.marker.as_ref().map(|m| m.offset), Some(1.3));
    }

    #[test]
    fn a_ray_meets_an_upright_capsule() {
        // A 35 cm capsule 1.8 m tall at the origin, seen from 10 m away at chest height.
        let d = ray_capsule(Vec3::new(-10.0, 1.2, 0.0), Vec3::X, Vec3::ZERO, 0.35, 0.9).unwrap();
        assert!((d - 9.65).abs() < 1e-3, "{d}");
        assert!(ray_capsule(Vec3::new(-10.0, 2.1, 0.0), Vec3::X, Vec3::ZERO, 0.35, 0.9).is_none(), "over its head");
    }
}
