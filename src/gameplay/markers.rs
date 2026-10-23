//! Player-placed map markers (MapWidget.OnMouseButtonDoubleClick -> AddMarker, BPC_MinimapSystem,
//! BP_PlayerMarker, W_PlayerMarker, PlayerMarkerSelectionUI) and their compass icons
//! (BP_CompassMarker, Compass.NavPoints, CompassMarkerUI).
//!
//! - With a GPS, a double click on the map takes the cursor back to the world
//!   (CalculateMarkerToWorld, the inverse of Calculate Actor Position) and traces straight down
//!   (Marker Location: WorldStatic, from 10 km up). Where it hits, a BP_PlayerMarker is spawned
//!   with the minimap component's current MarkerData; off the landscape nothing is placed.
//! - PlayerMarkerSelectionUI (the map's top right, GPS only) sets that MarkerData: one of four
//!   colours, twelve icons (the chosen one at full opacity, the rest at 0.25), the slider's size
//!   (0.5-4, step 0.1) and "Compass Marker?" (CVar Map.CompassMarker, on by default). The
//!   component's MarkerData starts as T_Marker, size 2, white; 0.2 s after the widget is built the
//!   picker selects the tent, while the slider shows 1 until it's moved. So the first markers are
//!   white tents at size 2, as in the game. (We start on the tent: the map can't open in 0.2 s.)
//! - W_PlayerMarker: the 24 px marker box with T_Marker_Fade at 0.75 under the icon (scaled by
//!   its size, tinted), the whole at 0.8; hovering scales it to 1.2 over 0.25 s (Hover). A double
//!   click on one removes it (Remove Player Marker). A double click on a POI or world marker
//!   destroys the minimap component's PlayerMarker, the marker placed last this session, if it's
//!   still there; only otherwise does the click reach the map and place one. (The game leaves
//!   that marker's widget on the map until the next load; ours goes at once.)
//! - Markers are saved with the game (BP_PlayerMarker is a saved actor; ActorLoaded re-adds it).
//! - With Compass Marker?, a BP_CompassMarker stands at the marker. Compass.NavPoints (every
//!   0.02 s) shows a 25 px CompassMarkerUI (the icon, tinted) on the compass strip at
//!   x = 700 tan(angle) + 350, y 5, while the marker is within 40 degrees of the camera's forward
//!   (ObjectBehind: the angle between the camera's forward and the marker-to-camera direction is
//!   under 140). The strip's RetainerBox clips it to the compass's 750 px.
//!
//! Not ported: the 1 s Delay before a new marker's compass icon appears.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, Slider, SliderPrecision, SliderRange, SliderStep, SliderThumb, SliderValue, TrackClick, ValueChange};
use serde::{Deserialize, Serialize};

use super::map::MapData;
use crate::menus::settings::{SettingsStore, Value};
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame, MenuData};

/// The settings key of PlayerMarkerSelectionUI's "Compass Marker?" (CVar Map.CompassMarker).
pub const COMPASS_KEY: &str = "map/Map Markers/Compass Marker?";
/// The marker box (W_PlayerMarker's Marker_Box) and its render scale (Construct Marker).
const BOX: f32 = 24.0;
const RENDER_SCALE: f32 = 0.8;
/// The Hover animation: Marker_Box's scale 1 -> 1.2 over 0.25 s.
const HOVER_SCALE: f32 = 1.2;
const HOVER_TIME: f32 = 0.25;
/// PlayerMarkerSelectionUI's SetRenderOpacity for the buttons not chosen.
const DIMMED: f32 = 0.25;

/// PlayerMarkerSelectionUI's icon buttons, in UniformGridPanel_72's rows of four.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerIcon {
    #[default]
    Tent,
    Skull,
    Animal,
    Cross,
    Right,
    Up,
    Down,
    Left,
    Vehicle,
    Marker1,
    Marker2,
    Home,
}

impl MarkerIcon {
    pub const GRID: [Self; 12] = [Self::Tent, Self::Skull, Self::Animal, Self::Cross, Self::Right, Self::Up, Self::Down, Self::Left, Self::Vehicle, Self::Marker1, Self::Marker2, Self::Home];

    /// The texture each button's click puts in MarkerData (also the button's brush).
    pub fn texture(self) -> &'static str {
        match self {
            Self::Tent => "UI/Textures/Survivors_Icon_Pack/256x256/A_Scavengers_Prey/T_Tent_256x256",
            Self::Skull => "UI/Textures/Survivors_Icon_Pack/256x256/Hunting_For_Food/T_HumanSkull_256x256",
            Self::Animal => "UI/Textures/Survivors_Icon_Pack/256x256/Hunting_For_Food/T_Rabbit_256x256",
            Self::Cross => "UI/Textures/XTexture",
            Self::Right => "UI/Textures/Survival-UI-Bundle/Rhombus/512x512/Arrow_Right",
            Self::Up => "UI/Textures/Survival-UI-Bundle/Rhombus/512x512/Arrow_Up",
            Self::Down => "UI/Textures/Survival-UI-Bundle/Hexagon/512x512/ArrowDown",
            Self::Left => "UI/Textures/Survival-UI-Bundle/Rhombus/512x512/Arrow_Left",
            Self::Vehicle => "UI/Textures/over_250_survival_flat_icons/car_wheel",
            Self::Marker1 => "Blueprints/MinimapSystem/Texture/T_Marker",
            Self::Marker2 => "Blueprints/MinimapSystem/Texture/T_Point",
            Self::Home => "UI/Textures/over_250_survival_flat_icons/wooden_wall",
        }
    }

    pub fn asset(self) -> String {
        format!("game/SurrounDead/Content/{}.png", self.texture())
    }
}

/// HorizontalBox_33's colour buttons: Black, White, Red, Blue. Each button's BackgroundColor (White
/// keeps the default) and the MarkerColor its click sets.
pub const COLOURS: [(Color, [f32; 4]); 4] = [
    (Color::linear_rgb(0.0, 0.0, 0.0), [0.0, 0.0, 0.0, 1.0]),
    (Color::linear_rgb(1.0, 1.0, 1.0), [1.0, 1.0, 1.0, 1.0]),
    (Color::linear_rgb(0.75, 0.0, 0.0), [1.0, 0.0, 0.0, 1.0]),
    (Color::linear_rgb(0.0, 0.0, 0.75), [0.0, 0.0, 1.0, 1.0]),
];

/// A BP_PlayerMarker: where it stands and the S_MarkerData it was placed with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerMarker {
    pub at: [f32; 3],
    pub icon: MarkerIcon,
    /// TextureSize: the icon's render scale on its box.
    pub size: f32,
    pub colour: [f32; 4],
    /// CompassMarker?: it also shows on the compass.
    pub compass: bool,
}

impl PlayerMarker {
    pub fn pos(&self) -> Vec3 {
        Vec3::from_array(self.at)
    }

    pub fn color(&self) -> Color {
        let [r, g, b, a] = self.colour;
        Color::linear_rgba(r, g, b, a)
    }
}

/// BPC_MinimapSystem.MarkerData as PlayerMarkerSelectionUI sets it, and the slider's value.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct MarkerPicker {
    pub icon: MarkerIcon,
    /// The colour button last clicked (none yet: MarkerData's white, every button at full opacity).
    pub colour: Option<usize>,
    pub size: f32,
    pub slider: f32,
    /// BPC_MinimapSystem.PlayerMarker: the marker placed last this session, while it exists.
    pub last: Option<usize>,
}

impl Default for MarkerPicker {
    fn default() -> Self {
        Self { icon: MarkerIcon::Tent, colour: None, size: 2.0, slider: 1.0, last: None }
    }
}

impl MarkerPicker {
    pub fn marker(&self, at: Vec3, compass: bool) -> PlayerMarker {
        let colour = self.colour.map_or([1.0; 4], |i| COLOURS[i].1);
        PlayerMarker { at: at.to_array(), icon: self.icon, size: self.size, colour, compass }
    }

    /// Marker `i` is gone: [`Self::last`] no longer names it, or names the one now at its index.
    pub fn removed(&mut self, i: usize) {
        self.last = match self.last {
            Some(l) if l == i => None,
            Some(l) if l > i => Some(l - 1),
            l => l,
        };
    }
}

/// Bumped whenever the markers change (placed, removed, loaded), so their widgets are rebuilt.
#[derive(Resource, Default)]
pub struct MarkerRev(pub u32);

/// Map.CompassMarker (true unless switched off).
pub fn compass_on(store: &SettingsStore) -> bool {
    !matches!(store.find(COMPASS_KEY), Some(Value::Bool(false)))
}

/// Slate's double click: a second press within the OS double-click time and a few pixels of the
/// first (Windows' defaults, 500 ms and 4 px: ours, the game takes them from the OS).
#[derive(Default)]
pub struct DoubleClick {
    last: Option<(f64, Vec2)>,
}

impl DoubleClick {
    pub fn press(&mut self, now: f64, at: Vec2) -> bool {
        if let Some((t, p)) = self.last
            && now - t <= 0.5
            && p.distance(at) <= 4.0
        {
            self.last = None;
            return true;
        }
        self.last = Some((now, at));
        false
    }
}

/// The topmost (last placed) marker whose box is under map point `m` (map px from the image's centre).
pub fn marker_at(markers: &[PlayerMarker], data: &MapData, m: Vec2) -> Option<usize> {
    markers.iter().rposition(|k| on_box(data.to_map(k.pos()), BOX * RENDER_SCALE, m))
}

/// Is map point `m` on a marker box of `size` px centred on `at`?
pub fn on_box(at: Vec2, size: f32, m: Vec2) -> bool {
    let d = (at - m).abs();
    d.x <= size / 2.0 && d.y <= size / 2.0
}

/// Compass.NewXPosition and ObjectBehind for a marker at `at` seen from a camera at `cam` with
/// `forward` and `right`: the icon's x on the compass panel, or `None` while it's behind.
pub fn compass_x(at: Vec3, cam: Vec3, forward: Vec3, right: Vec3) -> Option<f32> {
    let back = (cam - at).normalize_or_zero();
    if back.dot(forward.normalize_or_zero()).clamp(-1.0, 1.0).acos().to_degrees() < 140.0 {
        return None;
    }
    let flat = Vec3::new(back.x, 0.0, back.z).normalize_or_zero();
    Some(right.dot(flat) / forward.dot(flat) * 700.0 + 350.0)
}

#[derive(Component)]
pub struct PlayerMarkerUi {
    pub index: usize,
    hover: f32,
}
#[derive(Component)]
pub struct MarkerPickerUi;
#[derive(Component, Clone, Copy)]
struct PickIcon(MarkerIcon);
#[derive(Component, Clone, Copy)]
struct PickColour(usize);
#[derive(Component)]
struct PickSize;
#[derive(Component)]
struct SizeText;
#[derive(Component)]
struct PickCompass;
#[derive(Component)]
struct CompassTick;
#[derive(Component)]
pub struct CompassIcon(pub usize);

pub fn plugin(app: &mut App) {
    app.init_resource::<MarkerPicker>()
        .init_resource::<MarkerRev>()
        .add_observer(on_icon)
        .add_observer(on_colour)
        .add_observer(on_size)
        .add_observer(on_compass)
        .add_systems(OnEnter(AppState::InGame), |mut p: ResMut<MarkerPicker>, mut rev: ResMut<MarkerRev>| {
            *p = MarkerPicker::default();
            rev.0 += 1;
        })
        .add_systems(Update, (style_picker, hover_markers, compass_icons).run_if(in_state(AppState::InGame)));
}

/// W_PlayerMarker for each marker, into the map's marker layer.
pub fn spawn_markers(commands: &mut Commands, assets: &AssetServer, data: &MapData, markers: &[PlayerMarker], layer: Entity) {
    let fade = assets.load("game/SurrounDead/Content/Blueprints/MinimapSystem/Texture/T_Marker_Fade.png");
    let fill = || Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    for (i, m) in markers.iter().enumerate() {
        let at = data.to_map(m.pos());
        let e = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    width: Val::Px(BOX),
                    height: Val::Px(BOX),
                    margin: UiRect { left: Val::Px(-BOX / 2.0), top: Val::Px(-BOX / 2.0), ..default() },
                    ..default()
                },
                UiTransform { translation: Val2::px(at.x, at.y), scale: Vec2::splat(RENDER_SCALE), ..default() },
                PlayerMarkerUi { index: i, hover: 0.0 },
                Hovered::default(),
                children![
                    (fill(), ImageNode::new(fade.clone()).with_color(Color::linear_rgba(1.0, 1.0, 1.0, 0.75)), Pickable::IGNORE),
                    (fill(), ImageNode::new(assets.load(m.icon.asset())).with_color(m.color()), UiTransform::from_scale(Vec2::splat(m.size)), Pickable::IGNORE),
                ],
            ))
            .id();
        commands.entity(layer).add_child(e);
    }
}

/// PlayerMarkerSelectionUI, top right of the map (Padding top 10, right 10).
pub fn spawn_picker(commands: &mut Commands, font: &MenuFont, assets: &AssetServer, picker: &MarkerPicker, compass: bool, extra: impl Bundle) -> Entity {
    let text = |s: String, size: f32| (label_colored(font, s, size, Color::WHITE), Pickable::IGNORE);
    let pad = || UiRect::all(Val::Px(5.0));
    // Border_0: black at 0.5, UMG's default 4/2 padding.
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.0, 0.5)),
            MarkerPickerUi,
            Hovered::default(),
            extra,
        ))
        .id();
    let title = commands.spawn((Node { align_self: AlignSelf::Center, ..default() }, Pickable::IGNORE)).with_child(text("Map Markers".into(), 16.0)).id();
    // The colour buttons with 25 px spacers. Empty UMG buttons: 32 px squares (ours, the default
    // brush's size; the asset sets none).
    let colours = commands
        .spawn((Node { margin: pad(), column_gap: Val::Px(25.0), align_self: AlignSelf::Center, ..default() }, Pickable::IGNORE))
        .with_children(|r| {
            for (i, (c, _)) in COLOURS.iter().enumerate() {
                r.spawn((Node { width: Val::Px(32.0), height: Val::Px(32.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(*c), Button, Hovered::default(), PickColour(i)));
            }
        })
        .id();
    let grid = commands
        .spawn((
            Node { margin: pad(), display: Display::Grid, grid_template_columns: RepeatedGridTrack::px(4, 32.0), grid_auto_rows: vec![GridTrack::px(32.0)], align_self: AlignSelf::Center, ..default() },
            Pickable::IGNORE,
        ))
        .with_children(|g| {
            for icon in MarkerIcon::GRID {
                let alpha = if icon == picker.icon { 1.0 } else { DIMMED };
                g.spawn((Node { width: Val::Px(32.0), height: Val::Px(32.0), ..default() }, ImageNode::new(assets.load(icon.asset())).with_color(Color::WHITE.with_alpha(alpha)), Button, Hovered::default(), PickIcon(icon)));
            }
        })
        .id();
    // Slider_168 (UE's default style: a 2 px bar, an 8 x 14 thumb; ours from the engine's look).
    let frac = (picker.slider - 0.5) / 3.5;
    let slider = commands
        .spawn((
            Node { margin: pad(), height: Val::Px(14.0), ..default() },
            Slider { track_click: TrackClick::Snap, ..default() },
            SliderValue(picker.slider),
            SliderRange::new(0.5, 4.0),
            SliderStep(0.1),
            SliderPrecision(1),
            PickSize,
            Hovered::default(),
        ))
        .with_children(|s| {
            s.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), top: Val::Px(6.0), height: Val::Px(2.0), ..default() }, BackgroundColor(Color::srgb(0.6, 0.6, 0.6)), Pickable::IGNORE));
            s.spawn((thumb_node(frac), BackgroundColor(Color::srgb(0.85, 0.85, 0.85)), SliderThumb, Pickable::IGNORE));
        })
        .id();
    let size = commands.spawn((Node { margin: pad(), align_self: AlignSelf::Center, ..default() }, Pickable::IGNORE)).with_child((text(size_text(picker.slider), 10.0), SizeText)).id();
    // SettingRowSmall: the 8 pt label and the settings menu's check box.
    let row = commands
        .spawn((Node { margin: pad(), padding: UiRect::horizontal(Val::Px(20.0)), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() }, Pickable::IGNORE))
        .with_children(|r| {
            r.spawn((Node { margin: UiRect::all(Val::Px(10.0)), ..default() }, Pickable::IGNORE)).with_child(text("Compass Marker?".into(), 8.0));
            r.spawn((
                Node { width: Val::Px(32.0), height: Val::Px(32.0), border: UiRect::all(Val::Px(2.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                BorderColor::all(Color::WHITE.with_alpha(0.7)),
                BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.0, 0.25)),
                Button,
                Hovered::default(),
                PickCompass,
            ))
            .with_child((text(if compass { "X".into() } else { String::new() }, 16.0), CompassTick));
        })
        .id();
    commands.entity(root).add_children(&[title, colours, grid, slider, size, row]);
    root
}

fn thumb_node(frac: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Percent(frac * 100.0), margin: UiRect::left(Val::Px(-8.0 * frac)), width: Val::Px(8.0), height: Val::Px(14.0), ..default() }
}

/// SizeText: Format("Size: {A}") of the slider's float (FText's default: up to 3 decimals).
pub fn size_text(v: f32) -> String {
    let s = format!("{v:.3}");
    format!("Size: {}", s.trim_end_matches('0').trim_end_matches('.'))
}

fn on_icon(ev: On<Activate>, icons: Query<&PickIcon>, mut picker: ResMut<MarkerPicker>) {
    if let Ok(i) = icons.get(ev.entity) {
        picker.icon = i.0;
    }
}

fn on_colour(ev: On<Activate>, colours: Query<&PickColour>, mut picker: ResMut<MarkerPicker>) {
    if let Ok(c) = colours.get(ev.entity) {
        picker.colour = Some(c.0);
    }
}

fn on_size(ev: On<ValueChange<f32>>, mut commands: Commands, sliders: Query<(), With<PickSize>>, mut picker: ResMut<MarkerPicker>) {
    if sliders.get(ev.source).is_ok() {
        picker.slider = ev.value;
        picker.size = ev.value;
        commands.entity(ev.source).insert(SliderValue(ev.value));
    }
}

fn on_compass(ev: On<Activate>, checks: Query<(), With<PickCompass>>, mut store: ResMut<SettingsStore>, data: Res<MenuData>) {
    if checks.get(ev.entity).is_ok() {
        let on = !compass_on(&store);
        store.values.insert(COMPASS_KEY.into(), Value::Bool(on));
        crate::menus::settings::write(&store, &data);
    }
}

/// The picker's render opacities, size text, thumb and check, after a click.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn style_picker(
    picker: Res<MarkerPicker>,
    store: Res<SettingsStore>,
    mut icons: Query<(&PickIcon, &mut ImageNode)>,
    mut colours: Query<(&PickColour, &mut BackgroundColor)>,
    mut size: Query<&mut Text, (With<SizeText>, Without<CompassTick>)>,
    mut tick: Query<&mut Text, (With<CompassTick>, Without<SizeText>)>,
    mut thumbs: Query<&mut Node, With<SliderThumb>>,
    built: Query<(), Added<MarkerPickerUi>>,
) {
    if !picker.is_changed() && !store.is_changed() && built.is_empty() {
        return;
    }
    for (i, mut img) in &mut icons {
        img.color.set_alpha(if i.0 == picker.icon { 1.0 } else { DIMMED });
    }
    for (c, mut bg) in &mut colours {
        bg.0.set_alpha(if picker.colour.is_none_or(|p| p == c.0) { 1.0 } else { DIMMED });
    }
    for mut t in &mut size {
        t.0 = size_text(picker.slider);
    }
    for mut t in &mut tick {
        t.0 = if compass_on(&store) { "X".into() } else { String::new() };
    }
    for mut n in &mut thumbs {
        *n = thumb_node((picker.slider - 0.5) / 3.5);
    }
}

/// W_PlayerMarker's Hover animation, forward on mouse enter and in reverse on leave.
fn hover_markers(time: Res<Time<Real>>, mut markers: Query<(&mut PlayerMarkerUi, &Hovered, &mut UiTransform)>) {
    for (mut m, h, mut t) in &mut markers {
        let dir = if h.get() { 1.0 } else { -1.0 };
        let next = (m.hover + dir * time.delta_secs() / HOVER_TIME).clamp(0.0, 1.0);
        if next != m.hover {
            m.hover = next;
            t.scale = Vec2::splat(RENDER_SCALE * (1.0 + (HOVER_SCALE - 1.0) * next));
        }
    }
}

/// Compass.NavPoints: CompassMarkerUI on the compass strip for each compass marker in view.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn compass_icons(
    mut commands: Commands,
    game: Res<CurrentGame>,
    rev: Res<MarkerRev>,
    assets: Res<AssetServer>,
    strip: Query<Entity, With<super::hud::CompassStrip>>,
    cams: Query<&GlobalTransform, With<super::PlayerCamera>>,
    mut icons: Query<(Entity, &CompassIcon, &mut Node, &mut Visibility)>,
    mut built: Local<Option<(u32, Entity)>>,
) {
    let (Ok(strip), Some(markers)) = (strip.single(), game.state.as_ref().map(|s| &s.map_markers)) else { return };
    if *built != Some((rev.0, strip)) {
        *built = Some((rev.0, strip));
        for (e, ..) in &icons {
            commands.entity(e).despawn();
        }
        for (i, m) in markers.iter().enumerate().filter(|(_, m)| m.compass) {
            let icon = commands
                .spawn((
                    Node { position_type: PositionType::Absolute, top: Val::Px(5.0), width: Val::Px(25.0), height: Val::Px(25.0), ..default() },
                    ImageNode::new(assets.load(m.icon.asset())).with_color(m.color()),
                    CompassIcon(i),
                    Visibility::Hidden,
                ))
                .id();
            commands.entity(strip).add_child(icon);
        }
        return;
    }
    let Some(cam) = cams.iter().next() else { return };
    for (_, icon, mut node, mut vis) in &mut icons {
        let x = markers.get(icon.0).and_then(|m| compass_x(m.pos(), cam.translation(), *cam.forward(), *cam.right()));
        vis.set_if_neq(if x.is_some() { Visibility::Inherited } else { Visibility::Hidden });
        if let Some(x) = x {
            node.left = Val::Px(x);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_click_needs_two_quick_close_presses() {
        let mut d = DoubleClick::default();
        assert!(!d.press(0.0, Vec2::ZERO));
        assert!(d.press(0.3, Vec2::new(2.0, 1.0)));
        // A third press starts over.
        assert!(!d.press(0.4, Vec2::ZERO));
        assert!(!d.press(1.0, Vec2::ZERO), "too slow");
        assert!(!d.press(1.2, Vec2::new(30.0, 0.0)), "too far");
    }

    #[test]
    fn first_markers_are_white_size_two_tents() {
        let m = MarkerPicker::default().marker(Vec3::ONE, true);
        assert_eq!((m.icon, m.size, m.colour, m.compass), (MarkerIcon::Tent, 2.0, [1.0; 4], true));
        let red = MarkerPicker { colour: Some(2), icon: MarkerIcon::Skull, size: 3.5, slider: 3.5, last: None }.marker(Vec3::ONE, false);
        assert_eq!((red.icon, red.size, red.colour), (MarkerIcon::Skull, 3.5, [1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn size_text_formats_like_ftext() {
        assert_eq!(size_text(1.0), "Size: 1");
        assert_eq!(size_text(2.5), "Size: 2.5");
        assert_eq!(size_text(0.7000001), "Size: 0.7");
    }

    #[test]
    fn markers_are_hit_on_their_box_topmost_first() {
        let data = MapData::default();
        let at = Vec3::new(700.0, 10.0, 800.0);
        let m = MarkerPicker::default().marker(at, false);
        let markers = vec![m.clone(), m];
        let c = data.to_map(at);
        assert_eq!(marker_at(&markers, &data, c + Vec2::new(9.0, -9.0)), Some(1));
        assert_eq!(marker_at(&markers, &data, c + Vec2::new(10.0, 0.0)), None);
    }

    #[test]
    fn the_last_marker_follows_removals() {
        let mut p = MarkerPicker { last: Some(2), ..default() };
        p.removed(3);
        assert_eq!(p.last, Some(2));
        p.removed(0);
        assert_eq!(p.last, Some(1));
        p.removed(1);
        assert_eq!(p.last, None);
    }

    #[test]
    fn compass_icons_show_within_forty_degrees() {
        let (cam, f, r) = (Vec3::ZERO, Vec3::X, Vec3::Z);
        // Dead ahead: the panel's 350.
        assert!((compass_x(Vec3::new(100.0, 0.0, 0.0), cam, f, r).unwrap() - 350.0).abs() < 1e-3);
        // 20 degrees to the right: 700 tan 20 further right.
        let right = Vec3::new(100.0, 0.0, 100.0 * 20f32.to_radians().tan());
        assert!((compass_x(right, cam, f, r).unwrap() - (350.0 + 700.0 * 20f32.to_radians().tan())).abs() < 1e-2);
        assert!(compass_x(Vec3::new(100.0, 0.0, -100.0), cam, f, r).is_none(), "45 degrees off");
        assert!(compass_x(Vec3::new(-100.0, 0.0, 0.0), cam, f, r).is_none(), "behind");
    }

    #[test]
    fn markers_round_trip_through_the_save() {
        let m = MarkerPicker { colour: Some(3), icon: MarkerIcon::Home, size: 1.5, slider: 1.5, last: None }.marker(Vec3::new(1.0, 2.0, 3.0), true);
        let back: PlayerMarker = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back, m);
    }
}
