//! The item inspector, from JigSInventory's ItemInspector (BP_ItemInspector,
//! BP_InspectorWindowWidget, BP_InspectorCanvas; decompiled).
//!
//! - The context menu's Inspect (Jig.Context.Inspect) opens a window over the inventory: the
//!   item's name (20, E1E1E1) and a close button, a 400 x 400 render of the item
//!   (SceneCaptureComponent2D into a 512 x 512 target, the item alone on its lighting channel),
//!   "Price: ${a}" (10), the Description (14) and the stats.
//! - SetItemToInspect: the inspect mesh at its InspectTransform, the capture's FOV = the item's
//!   FOV x 1.5; MaxZoomIn / MaxZoomOut (items.json `zoom`) clamp the zoom.
//! - Mouse on the picture: left drag turns the view (SetMeshRotation: the spring arm's local
//!   rotation by the mouse delta, a degree a pixel), right drag moves it (SetMeshPosition: the
//!   delta / 30 cm), the wheel zooms 5° (OnZoomIn / OnZoomOut). Dragging the title moves the
//!   window. The window goes with the inventory.
//! - JSI_Slot.On_MouseDoubleClick (left button, in a grid with AllowOpeningContainers: all but
//!   the crafting UIs' and the disassembly workbench's): an item with a SpecialContainerWidget
//!   (items.json `special`: the guns' SC_<gun>, the backpacks' compartments, SC_Battery, SC_NVG,
//!   SC_Respirator, the clothes' pockets) opens that widget in a ContainerWindowWidget; any other item goes to
//!   JigContextMenuComp.OnDoubleClickInspect: the inspector, unless that item has one open.
//! - BP_InspectorCanvas holds any number of windows; SetInitPosition puts a new one at the
//!   middle +-50 px each way (RandomFloatInRange); a press on one brings it to the front
//!   (SetActiveWindow: ZOrder 1, the rest 0).
//! - InstallAttachments: a weapon's attachments (its SC_<gun>'s EquipTo containers) spawn their
//!   Local_ActorClass at the slot's socket on the inspected mesh, lit like it.
//!
//! Ours: the camera frames the model from its bounds and starts looking down its thinnest axis,
//! as the icons do (InspectTransform's rotations don't survive the axis conversion for every
//! mesh; until 2026-11-01 it looked down glb Z, so a rifle showed its muzzle); the stats are the tooltip's
//! lines; "this item has one open" compares the stack (two identical stacks share one window);
//! each window draws on its own render layer at its own spot. The storage containers' windows
//! (backpacks, pockets, cases) are `inventory_ui/windows.rs`. Not ported: the guns' SC_<gun>
//! slot panels and SC_Battery / SC_NVG / SC_Respirator windows (a double click on those does
//! nothing yet), AlternateMaterialToInspect, RemoveIronsights (the gun's IronSights bone stays
//! under a sight, as on the held gun), attachments on attachments.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::picking::events::{PointerDrag, PointerPress, PointerScroll};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::picking::hover::Hovered;
use bevy::ui_widgets::Activate;
use bevy::world_serialization::WorldInstanceReady;
use rand::Rng;

use super::interact::GameOverlay;
use super::items::{ItemDb, Stack};
use crate::menus::AppState;
use crate::menus::widgets::{MenuFont, label_colored};
use crate::models::model;

/// The inspector's stage, away from the icon bakes and the map.
const STAGE: Vec3 = Vec3::new(0.0, -6000.0, 0.0);
/// The first window's render layer; the n-th window's is LAYER + n.
const LAYER: usize = 30;
/// The n-th window's stage is STAGE + SPACING x n along X.
const SPACING: f32 = 100.0;
/// SetInitPosition's RandomFloatInRange(-50, 50).
pub const JITTER: f32 = 50.0;
/// CreateRenderTarget2D(512, 512).
const TARGET: u32 = 512;
/// MainSizeBox.
const VIEW: f32 = 400.0;
/// SetItemToInspect: the capture's FOV = the item's FOV x 1.5.
pub const FOV_SCALE: f32 = 1.5;
/// OnZoomIn / OnZoomOut steps.
pub const ZOOM_STEP: f32 = 5.0;
/// SetMeshPosition: the mouse delta / 30 (cm).
const PAN: f32 = 1.0 / 30.0 / 100.0;

/// Open an inspector window on a stack (the context menu's Inspect, a double click).
#[derive(Message, Clone)]
pub struct InspectItem(pub Stack);

/// What JSI_Slot's double click does with an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DoubleClick {
    /// OnDoubleClickInspect.
    Inspect,
    /// Its SpecialContainerWidget in a ContainerWindowWidget (`inventory_ui/windows.rs`: the
    /// storage ones; a gun's or battery's isn't ported).
    Window(String),
}

/// JSI_Slot.On_MouseDoubleClick on an item in a grid that allows opening containers.
pub fn double_click(def: &super::items::ItemDef) -> DoubleClick {
    match &def.special {
        Some(w) => DoubleClick::Window(w.clone()),
        None => DoubleClick::Inspect,
    }
}

/// BP_InspectorCanvas: the open windows, the last pressed in front.
#[derive(Resource, Default)]
pub struct Inspector {
    pub windows: Vec<InspectWindow>,
    next: u32,
}

impl Inspector {
    /// The window on this item, if one is open (`ItemRef.InspectorWidget`).
    pub fn showing(&self, stack: &Stack) -> bool {
        self.windows.iter().any(|w| w.item == *stack)
    }

    fn get(&mut self, id: u32) -> Option<&mut InspectWindow> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    /// The lowest stage slot no window uses.
    fn free_slot(&self) -> usize {
        (0..).find(|n| !self.windows.iter().any(|w| w.slot == *n)).unwrap_or(0)
    }
}

/// One BP_InspectorWindowWidget and its BP_ItemInspector.
pub struct InspectWindow {
    pub id: u32,
    pub item: Stack,
    slot: usize,
    view: View,
    /// The capture's FOV (degrees), the orbit (degrees), the pan (m).
    pub fov: f32,
    pub yaw: f32,
    pub pitch: f32,
    pan: Vec2,
    /// The window's offset from the middle (SetInitPosition's jitter, then drags).
    pub offset: Vec2,
    /// SetActiveWindow: in front.
    pub active: bool,
    /// The weapon's attachments put on its model (InstallAttachments).
    pub attached: Option<usize>,
}

impl InspectWindow {
    fn stage(&self) -> Vec3 {
        STAGE + Vec3::X * SPACING * self.slot as f32
    }
}

struct View {
    model: Entity,
    camera: Entity,
    light: Entity,
    ui: Entity,
    /// The model's bounds' centre, the framing distance and the starting view, once known.
    centre: Option<Vec3>,
    dist: f32,
    base: Quat,
    frames: u32,
}

/// The starting view of a model with these bounds' extents: down its thinnest axis, its own up
/// kept upright unless that's the view axis (the icons' rule for a square cell). The camera's
/// +Z (behind it) is the view axis.
pub fn start_view(ext: Vec3) -> Quat {
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    let mut order = [0usize, 1, 2];
    order.sort_by(|a, b| ext[*a].total_cmp(&ext[*b]));
    let up = super::icons::icon_up(order, false, true);
    Transform::IDENTITY.looking_to(-axes[order[0]], up).rotation
}

/// The window a UI part belongs to.
#[derive(Component, Clone, Copy)]
struct WindowId(u32);

#[derive(Component)]
struct InspectUi;

#[derive(Component)]
struct InspectPicture;

#[derive(Component)]
struct InspectTitle;

#[derive(Component)]
struct InspectClose;

#[derive(Component)]
struct InspectModel;

/// An attachment's model on an inspected weapon.
#[derive(Component)]
pub struct InspectAttachment(pub u32);

/// OnZoomIn / OnZoomOut: 5° less / more, within the item's MaxZoomIn..MaxZoomOut.
pub fn zoom(fov: f32, wheel_up: bool, [lo, hi]: [f32; 2]) -> f32 {
    (fov + if wheel_up { -ZOOM_STEP } else { ZOOM_STEP }).clamp(lo.min(hi), hi.max(lo))
}

/// "Price: ${a}" (the price, up to 3 decimals).
pub fn price_text(price: f32) -> String {
    let s = format!("{price:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("Price: ${s}")
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Inspector>()
        .add_message::<InspectItem>()
        .add_systems(OnEnter(AppState::InGame), |mut i: ResMut<Inspector>| *i = Inspector::default())
        .add_systems(Update, (open, frame, install_attachments, stack_windows, close_with_inventory).chain().run_if(in_state(AppState::InGame)))
        .add_observer(layer_model)
        .add_observer(on_close);
}

/// Scene children don't inherit render layers: tag the whole model with its window's.
#[allow(clippy::type_complexity)]
fn layer_model(ev: On<WorldInstanceReady>, models: Query<&RenderLayers, Or<(With<InspectModel>, With<InspectAttachment>)>>, children: Query<&Children>, mut commands: Commands) {
    if let Ok(layer) = models.get(ev.entity) {
        for e in children.iter_descendants(ev.entity) {
            commands.entity(e).insert(layer.clone());
        }
    }
}

fn despawn(commands: &mut Commands, v: &View) {
    for e in [v.model, v.camera, v.light, v.ui] {
        commands.entity(e).try_despawn();
    }
}

/// OnDoubleClickInspect / the context menu: a new window, unless this item has one.
#[allow(clippy::too_many_arguments)]
fn open(
    mut commands: Commands,
    mut msgs: MessageReader<InspectItem>,
    db: Res<ItemDb>,
    font: Res<MenuFont>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut inspector: ResMut<Inspector>,
) {
    for InspectItem(stack) in msgs.read() {
        let Some(def) = db.items.get(&stack.item) else { continue };
        if inspector.showing(stack) {
            info!("inspect: {} already has a window", stack.item);
            continue;
        }
        let mut img = Image::new_target_texture(TARGET, TARGET, TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb));
        img.asset_usage = RenderAssetUsages::RENDER_WORLD;
        let image = images.add(img);
        let fov = def.fov * FOV_SCALE;
        let slot = inspector.free_slot();
        let stage = STAGE + Vec3::X * SPACING * slot as f32;
        let layer = RenderLayers::layer(LAYER + slot);
        let model = commands
            .spawn((model(&assets, format!("game/{}.glb", def.mesh.as_deref().unwrap_or_default())), Transform::from_translation(stage), InspectModel, layer.clone(), DespawnOnExit(AppState::InGame)))
            .id();
        let camera = commands
            .spawn((
                Camera3d::default(),
                Camera { order: -40 - slot as isize, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
                RenderTarget::Image(image.clone().into()),
                Projection::Perspective(PerspectiveProjection { fov: fov.clamp(1.0, 120.0).to_radians(), ..default() }),
                Transform::from_translation(stage + Vec3::Z * 5.0).looking_at(stage, Vec3::Y),
                layer.clone(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        // SpotLight: the item lit from the front (its lighting channel only).
        let light = commands
            .spawn((DirectionalLight { illuminance: 14000.0, ..default() }, Transform::from_xyz(1.0, 2.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y), layer, DespawnOnExit(AppState::InGame)))
            .id();
        // SetInitPosition: the middle, +-50 px each way.
        let mut rng = rand::thread_rng();
        let offset = Vec2::new(rng.gen_range(-JITTER..=JITTER), rng.gen_range(-JITTER..=JITTER));
        let id = inspector.next;
        inspector.next += 1;
        let ui = spawn_window(&mut commands, &font, image, def, stack, &db, offset, id);
        info!("inspect: {} (fov {fov:.1}, zoom {:?}) window {id} at {offset:.0}", stack.item, def.zoom);
        for w in &mut inspector.windows {
            w.active = false;
        }
        inspector.windows.push(InspectWindow {
            id,
            item: stack.clone(),
            slot,
            view: View { model, camera, light, ui, centre: None, dist: 1.0, base: Quat::IDENTITY, frames: 0 },
            fov,
            yaw: 0.0,
            pitch: 0.0,
            pan: Vec2::ZERO,
            offset,
            active: true,
            attached: if stack.attachments.is_empty() { Some(0) } else { None },
        });
    }
}

/// The window's margins for its offset from the middle.
fn place(n: &mut Node, offset: Vec2) {
    n.margin.left = Val::Px(-220.0 + offset.x);
    n.margin.top = Val::Px(-290.0 + offset.y);
}

/// BP_InspectorWindowWidget.
#[allow(clippy::too_many_arguments)]
fn spawn_window(commands: &mut Commands, font: &MenuFont, image: Handle<Image>, def: &super::items::ItemDef, stack: &Stack, db: &ItemDb, offset: Vec2, id: u32) -> Entity {
    let grey = Color::srgb(0.75, 0.75, 0.75);
    let half = Color::srgba(0.0, 0.0, 0.0, 0.5);
    let mut node = Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Percent(50.0), flex_direction: FlexDirection::Column, padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), width: Val::Px(440.0), ..default() };
    place(&mut node, offset);
    let root = commands
        .spawn((node, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)), GlobalZIndex(46), InspectUi, WindowId(id), DespawnOnExit(AppState::InGame)))
        .observe(press_window)
        .id();
    let column = commands.spawn((Node { flex_direction: FlexDirection::Column, padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), ..default() }, ChildOf(root))).id();
    // TitleBorder: ItemName and CloseBtn.
    let title = commands.spawn((Node { align_items: AlignItems::Center, ..default() }, BackgroundColor(half), InspectTitle, WindowId(id), ChildOf(column))).id();
    commands.spawn((label_colored(font, def.name.clone(), 20.0, grey), Node { flex_grow: 1.0, margin: UiRect { left: Val::Px(10.0), top: Val::Px(5.0), bottom: Val::Px(5.0), ..default() }, ..default() }, Pickable::IGNORE, ChildOf(title)));
    commands.spawn((
        Node { margin: UiRect::all(Val::Px(5.0)), padding: UiRect::axes(Val::Px(6.0), Val::Px(0.0)), ..default() },
        bevy::ui_widgets::Button,
        Hovered::default(),
        InspectClose,
        WindowId(id),
        ChildOf(title),
        children![(label_colored(font, "X", 18.0, grey), Pickable::IGNORE)],
    ));
    // Border_273 / MainSizeBox: the picture.
    commands
        .spawn((
            Node { width: Val::Px(VIEW), height: Val::Px(VIEW), margin: UiRect::bottom(Val::Px(5.0)), align_self: AlignSelf::Center, ..default() },
            ImageNode::new(image),
            InspectPicture,
            WindowId(id),
            ChildOf(column),
        ))
        .observe(drag_picture)
        .observe(scroll_picture);
    let price = stack.price.unwrap_or(def.price);
    commands.spawn((label_colored(font, price_text(price), 10.0, grey), Node { align_self: AlignSelf::End, margin: UiRect::right(Val::Px(5.0)), ..default() }, ChildOf(column)));
    let mut lines: Vec<String> = Vec::new();
    if !def.description.is_empty() {
        lines.push(def.description.clone());
    }
    lines.extend(super::inventory_ui::stat_lines(db, stack));
    if !lines.is_empty() {
        let text = commands.spawn((Node { flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(5.0)), margin: UiRect::top(Val::Px(5.0)), ..default() }, BackgroundColor(half), ChildOf(column))).id();
        for l in lines {
            commands.spawn((label_colored(font, l, 14.0, Color::WHITE), ChildOf(text)));
        }
    }
    commands.entity(title).observe(drag_window);
    root
}

/// A press anywhere on a window: SetActiveWindow.
fn press_window(ev: On<PointerPress>, ids: Query<&WindowId>, mut inspector: ResMut<Inspector>) {
    let Ok(WindowId(id)) = ids.get(ev.event_target()) else { return };
    for w in &mut inspector.windows {
        w.active = w.id == *id;
    }
}

/// Left drag turns the view, right drag moves it.
fn drag_picture(ev: On<PointerDrag>, ids: Query<&WindowId>, mut inspector: ResMut<Inspector>) {
    let Some(w) = ids.get(ev.event_target()).ok().and_then(|i| inspector.get(i.0)) else { return };
    let d = ev.delta;
    match ev.button {
        PointerButton::Primary => {
            w.yaw -= d.x;
            w.pitch = (w.pitch + d.y).clamp(-89.0, 89.0);
        }
        PointerButton::Secondary => w.pan += Vec2::new(-d.x, d.y) * PAN,
        _ => {}
    }
}

/// The wheel: OnZoomIn (up) / OnZoomOut.
fn scroll_picture(ev: On<PointerScroll>, ids: Query<&WindowId>, db: Res<ItemDb>, mut inspector: ResMut<Inspector>) {
    let Some(w) = ids.get(ev.event_target()).ok().and_then(|i| inspector.get(i.0)) else { return };
    let limits = db.items.get(&w.item.item).map_or([1.0, 100.0], |d| d.zoom);
    w.fov = zoom(w.fov, ev.y > 0.0, limits);
}

/// Dragging the title moves its window.
fn drag_window(ev: On<PointerDrag>, ids: Query<&WindowId>, mut inspector: ResMut<Inspector>, mut uis: Query<&mut Node, With<InspectUi>>) {
    if ev.button != PointerButton::Primary {
        return;
    }
    let Some(w) = ids.get(ev.event_target()).ok().and_then(|i| inspector.get(i.0)) else { return };
    w.offset += ev.delta;
    if let Ok(mut n) = uis.get_mut(w.view.ui) {
        place(&mut n, w.offset);
    }
}

fn on_close(ev: On<Activate>, closes: Query<&WindowId, With<InspectClose>>, mut commands: Commands, mut inspector: ResMut<Inspector>) {
    let Ok(WindowId(id)) = closes.get(ev.entity) else { return };
    if let Some(i) = inspector.windows.iter().position(|w| w.id == *id) {
        let w = inspector.windows.remove(i);
        despawn(&mut commands, &w.view);
    }
}

/// SetActiveWindow's ZOrder: the active window over the rest.
fn stack_windows(inspector: Res<Inspector>, mut uis: Query<&mut GlobalZIndex, With<InspectUi>>) {
    if !inspector.is_changed() {
        return;
    }
    for w in &inspector.windows {
        if let Ok(mut z) = uis.get_mut(w.view.ui) {
            z.set_if_neq(GlobalZIndex(if w.active { 46 } else { 45 }));
        }
    }
}

/// Each camera: framed on its model's bounds once it's in, then orbiting with the mouse.
fn frame(
    mut inspector: ResMut<Inspector>,
    children: Query<&Children>,
    bounds: Query<(&Aabb, &GlobalTransform)>,
    attached: Query<(), With<InspectAttachment>>,
    mut cams: Query<(&mut Transform, &mut Projection)>,
) {
    for w in inspector.bypass_change_detection().windows.iter_mut() {
        let (fov, yaw, pitch, pan, stage) = (w.fov, w.yaw, w.pitch, w.pan, w.stage());
        // Framed on the weapon alone, before its attachments go on.
        let v = &mut w.view;
        v.frames += 1;
        if v.centre.is_none() {
            let (mut lo, mut hi, mut settled) = (Vec3::MAX, Vec3::MIN, true);
            for e in children.iter_descendants(v.model) {
                if attached.contains(e) {
                    break;
                }
                if let Ok((aabb, gt)) = bounds.get(e) {
                    if gt.translation().distance(stage) > 50.0 {
                        settled = false;
                    }
                    let (c, h) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
                    for k in 0..8 {
                        let corner = Vec3::new(if k & 1 == 0 { -1.0 } else { 1.0 }, if k & 2 == 0 { -1.0 } else { 1.0 }, if k & 4 == 0 { -1.0 } else { 1.0 });
                        let p = gt.transform_point(c + h * corner);
                        lo = lo.min(p);
                        hi = hi.max(p);
                    }
                }
            }
            if lo.x > hi.x || !settled {
                continue;
            }
            v.centre = Some((lo + hi) * 0.5);
            v.base = start_view(hi - lo);
            // Far enough that the whole model fits at the starting FOV.
            let radius = (hi - lo).length() * 0.5;
            v.dist = radius / (fov.to_radians() * 0.5).tan().max(0.05);
            info!("inspect: window {} framed, radius {radius:.2} m at {:.2} m", w.id, v.dist);
        }
        let Some(centre) = v.centre else { continue };
        let Ok((mut t, mut proj)) = cams.get_mut(v.camera) else { continue };
        let rot = v.base * Quat::from_euler(EulerRot::YXZ, yaw.to_radians(), -pitch.to_radians(), 0.0);
        let eye = centre + rot * (Vec3::Z * v.dist);
        let right = rot * Vec3::X;
        let up = rot * Vec3::Y;
        let shift = right * pan.x + up * pan.y;
        let want = Transform::from_translation(eye + shift).looking_at(centre + shift, up);
        if *t != want {
            *t = want;
        }
        if let Projection::Perspective(p) = &mut *proj
            && p.fov != fov.clamp(1.0, 120.0).to_radians()
        {
            p.fov = fov.clamp(1.0, 120.0).to_radians();
        }
    }
}

/// InstallAttachments: once the weapon is framed and its bones are in, each attachment's
/// Local_ActorClass on its socket (`attachments::placement`, as on the held gun), on the
/// window's layer.
fn install_attachments(
    mut commands: Commands,
    db: Res<ItemDb>,
    assets: Res<AssetServer>,
    mut inspector: ResMut<Inspector>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for w in inspector.bypass_change_detection().windows.iter_mut().filter(|w| w.attached.is_none() && w.view.centre.is_some()) {
        let places: Vec<_> = w.item.attachments.iter().filter_map(|a| super::attachments::placement(&db, &w.item.item, &a.item)).collect();
        let mut parents = Vec::new();
        for (bone, _, _) in &places {
            let parent = match bone {
                None => Some(w.view.model),
                Some(b) => children.iter_descendants(w.view.model).find(|&c| names.get(c).is_ok_and(|n| super::attachments::is_bone(n.as_str(), b))),
            };
            let Some(parent) = parent else { break };
            parents.push(parent);
        }
        if parents.len() < places.len() {
            continue;
        }
        let layer = RenderLayers::layer(LAYER + w.slot);
        for ((_, local, mesh), parent) in places.iter().zip(parents) {
            commands.spawn((crate::models::model(&assets, format!("game/{mesh}.glb")), *local, InspectAttachment(w.id), layer.clone(), ChildOf(parent)));
        }
        w.attached = Some(places.len());
        info!("inspect: window {} {} with {} attachments", w.id, w.item.item, places.len());
    }
}

/// Close them all from the screenshot tour (each X closes its own).
pub(crate) fn close(world: &mut World) {
    let windows = std::mem::take(&mut world.resource_mut::<Inspector>().windows);
    for w in windows {
        for e in [w.view.model, w.view.camera, w.view.light, w.view.ui] {
            if let Ok(e) = world.get_entity_mut(e) {
                e.despawn();
            }
        }
    }
}

/// The windows go with the inventory.
fn close_with_inventory(mut commands: Commands, overlay: Res<GameOverlay>, mut inspector: ResMut<Inspector>) {
    if !overlay.shows_inventory() && !inspector.windows.is_empty() {
        for w in std::mem::take(&mut inspector.windows) {
            despawn(&mut commands, &w.view);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_within_the_items_limits() {
        assert_eq!(zoom(27.0, true, [1.0, 100.0]), 22.0);
        assert_eq!(zoom(27.0, false, [1.0, 100.0]), 32.0);
        assert_eq!(zoom(3.0, true, [1.0, 100.0]), 1.0);
        assert_eq!(zoom(58.0, false, [45.0, 60.0]), 60.0);
    }

    #[test]
    fn price_reads_like_the_widget() {
        assert_eq!(price_text(25.0), "Price: $25");
        assert_eq!(price_text(2.5), "Price: $2.5");
    }

    #[test]
    fn a_rifle_is_seen_side_on_and_upright() {
        // Long along Z, thin along X, a bit taller than wide: looked at down X, Y up.
        let r = start_view(Vec3::new(0.06, 0.25, 0.9));
        assert!((r * Vec3::Z).abs_diff_eq(Vec3::X, 1e-5), "{:?}", r * Vec3::Z);
        assert!((r * Vec3::Y).abs_diff_eq(Vec3::Y, 1e-5));
        // Flat on the ground (Y thinnest): from above, its longest side up the picture.
        let r = start_view(Vec3::new(0.3, 0.02, 0.6));
        assert!((r * Vec3::Z).abs_diff_eq(Vec3::Y, 1e-5) && (r * Vec3::Y).abs_diff_eq(Vec3::Z, 1e-5));
    }

    #[test]
    fn a_double_click_opens_a_special_container_else_inspects() {
        let db = ItemDb::load();
        let def = |i: &str| db.items.get(i).unwrap_or_else(|| panic!("{i}"));
        assert_eq!(double_click(def("DA_M4CQB")), DoubleClick::Window("SC_M4CQB".into()));
        assert_eq!(double_click(def("DA_556Ammo")), DoubleClick::Inspect);
        // Magazines are containers, but OpenContainerOnDoubleClick? is off and they have no widget.
        assert_eq!(double_click(def("DA_556Mag")), DoubleClick::Inspect);
        // A backpack's compartments (SC_<pack>) open in a window.
        assert_eq!(double_click(def("DA_SchoolBackpack")), DoubleClick::Window("SC_SchoolBackpack".into()));
        assert!(matches!(double_click(def("DA_Flashlight")), DoubleClick::Window(w) if w == "SC_Battery"));
    }

    #[test]
    fn new_windows_take_the_lowest_free_stage() {
        let i = Inspector::default();
        assert_eq!(i.free_slot(), 0);
        assert!(!i.showing(&Stack::default()));
    }
}
