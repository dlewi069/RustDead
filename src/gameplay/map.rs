//! The map (IA_Map = M): MapWidget, BPC_MinimapSystem, BP_MapData; and the POIs on it
//! (BP_POIManager, W_POIMarker, W_POIMarkerTooltip, HUD_Game.DiscoverPOI -> POIDiscoveredUI).
//!
//! - BP_MapData_LongdownValley: Map Texture `RT_WorldImageCreator_Tex` (a top-down capture, north
//!   = UE +X up, east = UE +Y right), World Size 4500 m, World Middle (782.6, 752.6) m. MapWidget
//!   draws it as a 4000 x 4000 image in InnerCanvas, centred.
//! - `Calculate Actor Position`: (Y - middle.Y, -(X - middle.X)) / (World Size / 4000), clamped
//!   to the image (±2000). The player arrow (MI_Marker_Player: T_Player, colour 0.75) sits there,
//!   turned by the pawn's yaw, and only shows with a GPS equipped (ToggleMap's IsGPSEquipped?).
//! - Zoom (OnMouseWheel): steps -4..5 from 0; each step scales InnerCanvas by ZoomFactor 0.7975
//!   (the scale first clamped to 0.405..5) about the cursor; the step below MinZoom -3 jumps to
//!   the whole map (0.2675). Drag (OnMouseMove) pans while the button is held, down to MinZoom.
//!   ClampSides keeps the map's edges outside the screen's. OnScroll scales the arrow (3 / 2 /
//!   0.5) and hides the small POIs fully zoomed out. The widget is made once, so the view stays
//!   between openings.
//! - POIs: each BP_POIManager checks every 1-4 s (random) whether the survivor is within its
//!   ExploredCheckRadius; then it's Explored? (saved), gives level x (250 major / 50 minor) x 1.05
//!   x XP MULTIPLIER XP and shows "POI DISCOVERED" with its name and icon. Explored POIs show on
//!   the map: MarkerImage on a 24 px box scaled 5 (major or ForceLargeIcon) or 2, tinted
//!   MarkerColour (dark green when contaminated), over T_Marker_Fade at 0.15. Hovering one shows
//!   W_POIMarkerTooltip: the name in capitals with the icon, contamination level, description,
//!   and for major POIs the boss / special container counts and LOOT TO FIND.
//!
//! - StandardMarkerOverlay (our special-markers layer: world markers like the airdrops', and the
//!   player's markers, `markers.rs`) and PlayerMarkerSelectionUI need a GPS: ToggleMap hides
//!   them without one, and "Toggle Special Markers Overlay" only shows the layer with one.
//!
//! Not ported: BackgroundBlur (Bevy UI has no blur), quest markers, the minimap and fog of war (BP_MapData's minimap material is unused by MapWidget). The small
//! POI overlay comes before the map image in InnerCanvas, which would hide it behind an opaque
//! map; we draw it over the map (M_Map_Base's opacity is unreadable). In the game the map is a
//! GameAndUI overlay, so the survivor can still walk; ours pauses control like other overlays.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button};
use bevy::window::PrimaryWindow;
use rand::Rng;
use serde::Deserialize;

use super::equipment::Slot;
use super::interact::GameOverlay;
use super::markers::{self, MarkerPicker, MarkerRev};
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::CharacterModel;
use crate::menus::settings::SettingsStore;

const TEX: &str = "game/SurrounDead/Content/Blueprints/MinimapSystem/Texture";
const POI_TEX: &str = "game/SurrounDead/Content/POI";
/// The Map image's size in MapWidget (and InnerCanvas's desired size).
pub const MAP_PX: f32 = 4000.0;
const ZOOM_FACTOR: f32 = 0.7975;
const MIN_ZOOM: i32 = -3;
const MAX_ZOOM: i32 = 5;
/// MI_Marker_Player's Color (linear).
const ARROW: Color = Color::linear_rgb(0.75, 0.75, 0.75);
/// MapWidget's text grey (BCBCBC) and the translucent black panels.
const DIM: Color = Color::linear_rgb(0.5, 0.5, 0.5);
const PANEL: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.5);
/// W_POIMarkerTooltip's Border brush and the contaminated title border (BP_POIManager).
const TIP: Color = Color::linear_rgba(0.0025, 0.0025, 0.0025, 0.75);
const TIP_CONTAMINATED: Color = Color::linear_rgba(0.039216, 0.07451, 0.019608, 0.75);

/// BP_MapData's settings (UE cm).
#[derive(Clone, Copy, Debug)]
pub struct MapData {
    pub world_size: f32,
    pub middle: Vec2,
}

impl Default for MapData {
    /// BP_MapData_LongdownValley's values.
    fn default() -> Self {
        Self { world_size: 450000.0, middle: Vec2::new(78257.15, 75257.57) }
    }
}

impl MapData {
    /// MapWidget's `Calculate Actor Position`: where world point `p` (Bevy metres) lands on the
    /// image, from its centre (UMG px, y down).
    pub fn to_map(&self, p: Vec3) -> Vec2 {
        let half = MAP_PX / 2.0;
        let per_px = self.world_size / MAP_PX;
        let (x, y) = (p.x * 100.0 - self.middle.x, p.z * 100.0 - self.middle.y);
        Vec2::new((y / per_px).clamp(-half, half), (-x / per_px).clamp(-half, half))
    }

    /// CalculateMarkerToWorld's last steps: the world x/z (Bevy metres) under map point `m`.
    pub fn to_world(&self, m: Vec2) -> Vec2 {
        let per_px = self.world_size / MAP_PX;
        Vec2::new(self.middle.x - m.y * per_px, self.middle.y + m.x * per_px) / 100.0
    }
}

/// The pawn's yaw in UE degrees (0 = +X = north, 90 = +Y = east), from its Bevy forward.
pub fn ue_yaw(forward: Vec3) -> f32 {
    forward.z.atan2(forward.x).to_degrees()
}

/// A BP_POIManager placed in the level.
#[derive(Clone, Debug)]
pub struct Poi {
    /// The actor's name (what's saved as explored).
    pub actor: String,
    pub name: String,
    pub description: String,
    pub major: bool,
    pub force_large: bool,
    pub contamination: i32,
    /// MarkerImage's texture name (`Marker_Farm`).
    pub image: String,
    pub colour: Color,
    pub loot: String,
    pub bosses: u32,
    /// ExploredCheckRadius, metres.
    pub radius: f32,
    pub pos: Vec3,
}

impl Poi {
    /// Struct_POI's members are named `<Name>_<n>_<GUID>`.
    fn field<'a>(poi: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
        poi.as_object()?.iter().find(|(k, _)| k.split('_').next() == Some(name)).map(|(_, v)| v)
    }

    fn from_actor(a: &crate::level::ActorInfo) -> Option<Self> {
        let props = a.props.as_ref()?;
        let poi: serde_json::Value = serde_json::from_str(props.get("POI")?).ok()?;
        let s = |k: &str| Self::field(&poi, k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        // Missing members are the class default's (IsAMajorPOI? true, white, 4000 cm).
        let flag = |k: &str, d: bool| Self::field(&poi, k).and_then(|v| v.as_bool()).unwrap_or(d);
        let colour = Self::field(&poi, "MarkerColour").and_then(|c| {
            let f = |k: &str| c.get(k).and_then(|v| v.as_f64()).map(|v| v as f32);
            Some(Color::linear_rgba(f("R")?, f("G")?, f("B")?, f("A")?))
        });
        let image = Self::field(&poi, "MarkerImage").and_then(|v| v.get("ObjectPath")).and_then(|v| v.as_str()).and_then(|p| p.rsplit('/').next()).map(|p| p.trim_end_matches(".0").to_string());
        let num = |k: &str| props.get(k).and_then(|v| v.parse::<f32>().ok());
        Some(Self {
            actor: a.name.clone(),
            name: s("Name"),
            description: s("Description"),
            major: flag("IsAMajorPOI?", true),
            force_large: flag("ForceLargeIcon", false),
            contamination: Self::field(&poi, "ContaminationLevel").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            image: image.unwrap_or_default(),
            colour: colour.unwrap_or(Color::WHITE),
            loot: s("ItemsToSpawnDescription"),
            bosses: num("SpawnAmount").unwrap_or(1.0) as u32,
            radius: num("ExploredCheckRadius").unwrap_or(4000.0) / 100.0,
            pos: a.position(),
        })
    }

    /// BP_POIManager's marker data: the large icon for major POIs or ForceLargeIcon.
    pub fn large(&self) -> bool {
        self.major || self.force_large
    }

    /// TextureSize: the icon's render scale on its 24 px box.
    pub fn icon_scale(&self) -> f32 {
        if self.large() { 5.0 } else { 2.0 }
    }

    /// W_POIMarker's Construct Marker: contaminated POIs are dark green.
    pub fn icon_colour(&self) -> Color {
        if self.contamination > 0 { Color::linear_rgb(0.039216, 0.07451, 0.019608) } else { self.colour }
    }

    /// DistanceCheck's XP: XpMultiplierCalc(250 or 50) = level x that x 1.05 x XP MULTIPLIER.
    pub fn xp(&self, level: u32, xp_mult: f32) -> f32 {
        level.max(1) as f32 * if self.major { 250.0 } else { 50.0 } * 1.05 * xp_mult
    }
}

/// The map and its POIs, from the level's actors.
#[derive(Resource, Default)]
pub struct MapDb {
    pub data: MapData,
    pub pois: Vec<Poi>,
}

impl MapDb {
    pub fn from_actors(actors: &[crate::level::ActorInfo]) -> Self {
        let mut data = MapData::default();
        if let Some(p) = actors.iter().find(|a| a.class == "BP_MapData_C").and_then(|a| a.props.as_ref()) {
            if let Some(s) = p.get("World Size").and_then(|v| v.parse().ok()) {
                data.world_size = s;
            }
            #[derive(Deserialize)]
            struct V2 {
                #[serde(rename = "X")]
                x: f32,
                #[serde(rename = "Y")]
                y: f32,
            }
            if let Some(m) = p.get("World Middle Coordinations").and_then(|v| serde_json::from_str::<V2>(v).ok()) {
                data.middle = Vec2::new(m.x, m.y);
            }
        }
        let pois = actors.iter().filter(|a| a.class == "BP_POIManager_C").filter_map(Poi::from_actor).collect();
        Self { data, pois }
    }

    fn load() -> Self {
        let path = format!("{}/actors.json", crate::level::LEVEL_DIR);
        let actors: Vec<crate::level::ActorInfo> = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self::from_actors(&actors)
    }
}

/// FMath::Clamp, which returns `min` when the range is inverted and `x` is below it.
fn ue_clamp(x: f32, min: f32, max: f32) -> f32 {
    if x < min { min } else if x < max { x } else { max }
}

/// InnerCanvas's render transform and the zoom step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub step: i32,
    pub scale: f32,
    pub offset: Vec2,
    /// PlayerTexture's render scale (OnScroll; 1 until the first scroll).
    pub arrow_scale: f32,
}

impl Default for View {
    fn default() -> Self {
        Self { step: 0, scale: 1.0, offset: Vec2::ZERO, arrow_scale: 1.0 }
    }
}

impl View {
    /// ClampSides: the canvas moves only until the map's edge meets the screen's.
    fn clamp_sides(&self, t: Vec2, screen: Vec2) -> Vec2 {
        let b = (Vec2::splat(MAP_PX) * self.scale - screen) * 0.5;
        Vec2::new(ue_clamp(t.x, -b.x, b.x), ue_clamp(t.y, -b.y, b.y))
    }

    /// OnMouseWheel: one notch in (`up`) or out with the cursor at `cursor` (local px).
    pub fn wheel(&mut self, up: bool, cursor: Vec2, screen: Vec2) {
        let apply = |v: &mut View, f: f32| {
            v.scale = v.scale.clamp(0.405, 5.0) * f;
            let d = cursor - screen / 2.0 - v.offset;
            v.offset = v.clamp_sides(v.offset - d * (f - 1.0), screen);
        };
        if up {
            if self.step < MAX_ZOOM {
                self.step += 1;
                apply(self, 1.0 / ZOOM_FACTOR);
            }
        } else if self.step > MIN_ZOOM {
            self.step -= 1;
            apply(self, ZOOM_FACTOR);
        } else if self.step == MIN_ZOOM {
            // The whole map.
            self.step -= 1;
            self.scale = 0.2675;
            self.offset = Vec2::splat(0.5);
        }
        self.on_scroll();
    }

    /// OnScroll's PlayerTexture scale.
    fn on_scroll(&mut self) {
        match self.step {
            -4 => self.arrow_scale = 3.0,
            -3..=0 => self.arrow_scale = 2.0,
            1..=5 => self.arrow_scale = 0.5,
            _ => {}
        }
    }

    /// OnMouseMove while dragging: pan by the cursor's movement.
    pub fn drag(&mut self, delta: Vec2, screen: Vec2) {
        if self.step >= MIN_ZOOM {
            self.offset = self.clamp_sides(self.offset + delta, screen);
        }
    }

    /// Centre the view on map point `at` (ours, for the tour; the BP's unused
    /// SetScreenPositionToPlayer ignores the zoom).
    pub fn look_at(&mut self, at: Vec2, screen: Vec2) {
        self.offset = self.clamp_sides(-at * self.scale, screen);
    }

    /// CalculateMarkerToWorld: the map point (px from the image's centre) under `cursor`.
    pub fn map_point(&self, cursor: Vec2, screen: Vec2) -> Vec2 {
        (cursor - screen / 2.0 - self.offset) / self.scale
    }

    /// Where map point `m` is on the screen (local px).
    pub fn screen_point(&self, m: Vec2, screen: Vec2) -> Vec2 {
        screen / 2.0 + self.offset + m * self.scale
    }

    /// OnScroll: the small POIs are hidden fully zoomed out.
    pub fn small_pois_shown(&self) -> bool {
        self.step > -4
    }
}

#[derive(Resource, Default)]
pub struct MapView {
    pub view: View,
    open: bool,
    dragging: bool,
    show_pois: bool,
    show_special: bool,
    built: bool,
    /// Keeps the 4096² map texture loaded once it's been seen.
    texture: Option<Handle<Image>>,
}

/// The map's own controls (the overlay checkboxes, the marker picker): presses on them don't
/// drag the map or count toward a double click.
#[derive(Component)]
pub struct MapControl;

/// A BP_Marker the Minimap System's Add World Marker put on the map (an airdrop's): W_WorldMarker
/// (the POI marker's 24 px box and fade, the icon scaled by its Texture Size and tinted, the whole
/// at render scale 0.8) under "Toggle Special Markers Overlay", with W_WorldMarkerTooltip.
#[derive(Clone, Debug)]
pub struct WorldMarker {
    pub key: usize,
    pub pos: Vec3,
    /// Its MarkerTexture, as an asset path.
    pub texture: String,
    pub size: f32,
    pub colour: Color,
    pub title: String,
    pub description: String,
}

#[derive(Resource, Default)]
pub struct WorldMarkers(pub Vec<WorldMarker>);

/// Each POI's next DistanceCheck (seconds of play left).
#[derive(Resource, Default)]
struct Checks(Vec<f32>);

#[derive(Component)]
struct MapUi;
#[derive(Component)]
struct InnerCanvas;
#[derive(Component)]
pub struct PlayerArrow;
#[derive(Component)]
struct SmallPois;
#[derive(Component)]
struct LargePois;
#[derive(Component)]
pub struct PoiMarker(pub usize);
#[derive(Component)]
struct PoiTooltip;
#[derive(Component)]
struct SpecialMarkers;
#[derive(Component)]
pub struct SpecialMarker(pub usize);
#[derive(Component, Clone, Copy, PartialEq)]
enum MapCheck {
    Special,
    Pois,
}
#[derive(Component)]
struct CheckTick(MapCheck);

/// POIDiscoveredUI and its age.
#[derive(Component)]
pub struct Discovered(f32);
#[derive(Component)]
struct DiscoveredBack;
#[derive(Component)]
struct DiscoveredFront;

pub fn plugin(app: &mut App) {
    app.insert_resource(MapView { show_pois: true, show_special: true, ..default() })
        .insert_resource(MapDb::load())
        .init_resource::<Checks>()
        .init_resource::<WorldMarkers>()
        .add_observer(on_check)
        .add_systems(OnEnter(AppState::InGame), |mut v: ResMut<MapView>, mut m: ResMut<WorldMarkers>| {
            v.built = false;
            m.0.clear();
        })
        .add_systems(Update, (toggle, add_or_remove_marker, build, (pan_and_zoom, place).chain(), tooltip).chain().run_if(in_state(AppState::InGame)))
        .add_systems(Update, (discover, fade_banner).run_if(in_state(AppState::InGame)));
}

pub fn gps(game: &CurrentGame) -> bool {
    game.inventory.as_ref().is_some_and(|i| i.equipped.contains_key(&Slot::Gps))
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, screen: Res<State<MenuScreen>>, mut overlay: ResMut<GameOverlay>, mut map: ResMut<MapView>) {
    if *screen.get() == MenuScreen::None && keys.just_pressed(KeyCode::KeyM) {
        match *overlay {
            GameOverlay::Map => *overlay = GameOverlay::None,
            GameOverlay::None => *overlay = GameOverlay::Map,
            _ => {}
        }
    }
    let open = *overlay == GameOverlay::Map;
    if open != map.open {
        map.open = open;
        map.built = false;
        map.dragging = false;
    }
}

#[allow(clippy::too_many_arguments)]
fn build(
    mut commands: Commands,
    mut map: ResMut<MapView>,
    db: Res<MapDb>,
    (world, rev, picker, store): (Res<WorldMarkers>, Res<MarkerRev>, Res<MarkerPicker>, Res<SettingsStore>),
    game: Res<CurrentGame>,
    font: Res<MenuFont>,
    assets: Res<AssetServer>,
    uis: Query<Entity, With<MapUi>>,
) {
    if map.built && !(map.open && (world.is_changed() || rev.is_changed())) {
        return;
    }
    map.built = true;
    for e in &uis {
        commands.entity(e).despawn();
    }
    if !map.open {
        return;
    }
    let texture = map.texture.get_or_insert_with(|| assets.load(format!("{TEX}/RT_WorldImageCreator_Tex.png"))).clone();
    let explored = game.state.as_ref().map(|s| s.pois_explored.clone()).unwrap_or_default();
    let fill = || Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    // A box of `size` centred on its parent, moved by `at`.
    let centred = |size: Vec2| Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Percent(50.0),
        width: Val::Px(size.x),
        height: Val::Px(size.y),
        margin: UiRect { left: Val::Px(-size.x / 2.0), top: Val::Px(-size.y / 2.0), ..default() },
        ..default()
    };

    // Border_23 (black, 0.5) over the game.
    let root = commands.spawn((fill(), BackgroundColor(PANEL), GlobalZIndex(40), MapUi, DespawnOnExit(AppState::InGame))).id();
    let inner = commands.spawn((fill(), UiTransform::default(), InnerCanvas, Pickable::IGNORE)).id();
    commands.entity(root).add_child(inner);
    // Border_0 (black) around the Map image, centred.
    let image = commands.spawn((centred(Vec2::splat(MAP_PX)), BackgroundColor(Color::BLACK), ImageNode::new(texture), Pickable::IGNORE)).id();
    let small = commands.spawn((fill(), SmallPois, Pickable::IGNORE)).id();
    let arrow = commands
        .spawn((centred(Vec2::splat(28.0)), ImageNode::new(assets.load(format!("{TEX}/T_Player.png"))).with_color(ARROW), UiTransform::default(), PlayerArrow, Visibility::Hidden, Pickable::IGNORE))
        .id();
    let large = commands.spawn((fill(), LargePois, Pickable::IGNORE)).id();
    let special = commands.spawn((fill(), SpecialMarkers, Pickable::IGNORE)).id();
    commands.entity(inner).add_children(&[image, small, arrow, large, special]);
    let fade = assets.load(format!("{TEX}/T_Marker_Fade.png"));
    for (i, m) in world.0.iter().enumerate() {
        let at = db.data.to_map(m.pos);
        let e = commands
            .spawn((
                centred(Vec2::splat(24.0)),
                UiTransform { translation: Val2::px(at.x, at.y), scale: Vec2::splat(0.8), ..default() },
                SpecialMarker(i),
                Hovered::default(),
                children![
                    (fill(), ImageNode::new(fade.clone()).with_color(Color::linear_rgba(1.0, 1.0, 1.0, 0.15)), Pickable::IGNORE),
                    (fill(), ImageNode::new(assets.load(m.texture.clone())).with_color(m.colour), UiTransform::from_scale(Vec2::splat(m.size)), Pickable::IGNORE),
                ],
            ))
            .id();
        commands.entity(special).add_child(e);
    }
    if let Some(st) = game.state.as_ref() {
        markers::spawn_markers(&mut commands, &assets, &db.data, &st.map_markers, special);
    }
    for (i, poi) in db.pois.iter().enumerate() {
        if !explored.contains(&poi.actor) {
            continue;
        }
        let at = db.data.to_map(poi.pos);
        let icon = assets.load(format!("{POI_TEX}/{}.png", poi.image));
        // Marker_Box (24 px) with Fade at 0.15 and the Icon scaled by TextureSize.
        let m = commands
            .spawn((
                centred(Vec2::splat(24.0)),
                UiTransform::from_translation(Val2::px(at.x, at.y)),
                PoiMarker(i),
                Hovered::default(),
                children![
                    (fill(), ImageNode::new(fade.clone()).with_color(Color::linear_rgba(1.0, 1.0, 1.0, 0.15)), Pickable::IGNORE),
                    (fill(), ImageNode::new(icon).with_color(poi.icon_colour()), UiTransform::from_scale(Vec2::splat(poi.icon_scale())), Pickable::IGNORE),
                ],
            ))
            .id();
        commands.entity(if poi.large() { large } else { small }).add_child(m);
    }

    // Overlay_148: the input help, compass letters and overlay toggles, over the map.
    let text = |s: &'static str, size: f32, c: Color| (label_colored(&font, s, size, c), Pickable::IGNORE);
    let help = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(10.0), bottom: Val::Px(10.0), padding: UiRect::new(Val::Px(20.0), Val::Px(10.0), Val::Px(10.0), Val::Px(10.0)), flex_direction: FlexDirection::Column, ..default() },
            BackgroundColor(PANEL),
            Pickable::IGNORE,
            children![
                (text("Input:", 10.0, Color::WHITE), Node { margin: UiRect::bottom(Val::Px(5.0)), ..default() }),
                (text("[ MOUSE SCROLL ] Zoom Map", 8.0, DIM), Node { margin: UiRect::top(Val::Px(5.0)), ..default() }),
                (text("[ MOUSE DRAG ] Drag Map", 8.0, DIM), Node { margin: UiRect::top(Val::Px(5.0)), ..default() }),
                (text("[ MOUSE DOUBLE CLICK & GPS REQUIRED] Add Marker", 8.0, DIM), Node { margin: UiRect::top(Val::Px(5.0)), ..default() }),
                (text("You require a GPS to see your location", 8.0, DIM), Node { margin: UiRect::top(Val::Px(5.0)), ..default() }),
            ],
        ))
        .id();
    // N/E/S/W: TextBlocks at UMG's default 24 pt, 10 px in from each edge.
    let edge = |n: Node| Node { position_type: PositionType::Absolute, ..n };
    let letters = [
        ("N", edge(Node { top: Val::Px(10.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() })),
        ("S", edge(Node { bottom: Val::Px(10.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() })),
        ("E", edge(Node { right: Val::Px(10.0), height: Val::Percent(100.0), align_items: AlignItems::Center, ..default() })),
        ("W", edge(Node { left: Val::Px(10.0), height: Val::Percent(100.0), align_items: AlignItems::Center, ..default() })),
    ];
    for (l, n) in letters {
        let e = commands.spawn((n, Pickable::IGNORE)).with_child(text(l, 24.0, Color::WHITE)).id();
        commands.entity(root).add_child(e);
    }
    // The bottom-right toggles (both checked by default).
    let toggles = commands
        .spawn((
            Node { position_type: PositionType::Absolute, right: Val::Px(10.0), bottom: Val::Px(5.0), padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() },
            BackgroundColor(PANEL),
            Pickable::IGNORE,
        ))
        .id();
    for (which, label, on) in [(MapCheck::Special, "Toggle Special Markers Overlay", map.show_special), (MapCheck::Pois, "Toggle POI Markers Overlay", map.show_pois)] {
        // UE's default CheckBox: a dark box with a white tick (ours: a filled square).
        let row = commands
            .spawn((Node { align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }, Pickable::IGNORE))
            .with_children(|r| {
                r.spawn((
                    Node { width: Val::Px(20.0), height: Val::Px(20.0), padding: UiRect::all(Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), ..default() },
                    BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
                    BorderColor::all(Color::srgb(0.6, 0.6, 0.6)),
                    Button,
                    which,
                    MapControl,
                    Hovered::default(),
                ))
                .with_child((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::WHITE), if on { Visibility::Inherited } else { Visibility::Hidden }, CheckTick(which), Pickable::IGNORE));
                r.spawn(text(label, 12.0, Color::WHITE));
            })
            .id();
        commands.entity(toggles).add_child(row);
    }
    commands.entity(root).add_children(&[help, toggles]);
    if gps(&game) {
        let picker = markers::spawn_picker(&mut commands, &font, &assets, &picker, markers::compass_on(&store), MapControl);
        commands.entity(root).add_child(picker);
    }
}

fn on_check(ev: On<Activate>, checks: Query<&MapCheck>, mut map: ResMut<MapView>, mut ticks: Query<(&CheckTick, &mut Visibility)>) {
    let Ok(&which) = checks.get(ev.entity) else { return };
    let on = match which {
        MapCheck::Special => {
            map.show_special = !map.show_special;
            map.show_special
        }
        MapCheck::Pois => {
            map.show_pois = !map.show_pois;
            map.show_pois
        }
    };
    for (t, mut v) in &mut ticks {
        if t.0 == which {
            *v = if on { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

/// The map widget's local size and the cursor in it (UMG px).
fn local(windows: &Query<&Window, With<PrimaryWindow>>, scale: &UiScale) -> Option<(Vec2, Option<Vec2>)> {
    let w = windows.single().ok()?;
    Some((w.size() / scale.0, w.cursor_position().map(|p| p / scale.0)))
}

#[allow(clippy::too_many_arguments)]
fn pan_and_zoom(
    mut map: ResMut<MapView>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    checks: Query<&Hovered, With<MapControl>>,
) {
    if !map.open {
        return;
    }
    let Some((screen, cursor)) = local(&windows, &scale) else { return };
    if mouse.just_pressed(MouseButton::Left) && !checks.iter().any(|h| h.get()) {
        map.dragging = true;
    }
    if !mouse.pressed(MouseButton::Left) {
        map.dragging = false;
    }
    if map.dragging && motion.delta != Vec2::ZERO {
        map.view.drag(motion.delta / scale.0, screen);
    }
    // One step per notch, like UMG's wheel events.
    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y.round() as i32,
        MouseScrollUnit::Pixel => (scroll.delta.y / 100.0).round() as i32,
    };
    let at = cursor.unwrap_or(screen / 2.0);
    for _ in 0..notches.unsigned_abs() {
        map.view.wheel(notches > 0, at, screen);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn place(
    map: Res<MapView>,
    db: Res<MapDb>,
    game: Res<CurrentGame>,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
    mut inner: Query<&mut UiTransform, (With<InnerCanvas>, Without<PlayerArrow>)>,
    mut arrow: Query<(&mut UiTransform, &mut Visibility), (With<PlayerArrow>, Without<InnerCanvas>)>,
    mut small: Query<&mut Visibility, (With<SmallPois>, Without<PlayerArrow>, Without<LargePois>)>,
    mut large: Query<&mut Visibility, (With<LargePois>, Without<PlayerArrow>, Without<SmallPois>)>,
    mut special: Query<&mut Visibility, (With<SpecialMarkers>, Without<PlayerArrow>, Without<SmallPois>, Without<LargePois>)>,
) {
    if !map.open {
        return;
    }
    let v = map.view;
    for mut t in &mut inner {
        *t = UiTransform { translation: Val2::px(v.offset.x, v.offset.y), scale: Vec2::splat(v.scale), rotation: Rot2::IDENTITY };
    }
    if let (Ok(me), Ok((mut t, mut vis))) = (player.single(), arrow.single_mut()) {
        let at = db.data.to_map(me.translation);
        let yaw = ue_yaw(me.rotation * Vec3::Z);
        *t = UiTransform { translation: Val2::px(at.x, at.y), scale: Vec2::splat(v.arrow_scale), rotation: Rot2::degrees(yaw) };
        vis.set_if_neq(if gps(&game) { Visibility::Inherited } else { Visibility::Hidden });
    }
    let shown = |b: bool| if b { Visibility::Inherited } else { Visibility::Hidden };
    for mut s in &mut small {
        s.set_if_neq(shown(map.show_pois && v.small_pois_shown()));
    }
    for mut l in &mut large {
        l.set_if_neq(shown(map.show_pois));
    }
    for mut l in &mut special {
        l.set_if_neq(shown(map.show_special && gps(&game)));
    }
}

/// A double click on the map with a GPS (MapWidget.OnMouseButtonDoubleClick -> AddMarker ->
/// Create Checkpoint Marker): on one of the player's markers it removes it (W_PlayerMarker's own
/// double click, which is on top), elsewhere it places one where the trace down hits the world.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn add_or_remove_marker(
    map: Res<MapView>,
    db: Res<MapDb>,
    (windows, scale, time, mouse): (Query<&Window, With<PrimaryWindow>>, Res<UiScale>, Res<Time<Real>>, Res<ButtonInput<MouseButton>>),
    (mut picker, store, world): (ResMut<MarkerPicker>, Res<SettingsStore>, Res<WorldMarkers>),
    mut game: ResMut<CurrentGame>,
    mut rev: ResMut<MarkerRev>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut rays: crate::level::collision::PropRays,
    controls: Query<&Hovered, With<MapControl>>,
    mut clicks: Local<markers::DoubleClick>,
) {
    if !map.open || !mouse.just_pressed(MouseButton::Left) || controls.iter().any(|h| h.get()) {
        return;
    }
    let Some((screen, Some(cursor))) = local(&windows, &scale) else { return };
    if !clicks.press(time.elapsed_secs_f64(), cursor) || !gps(&game) {
        return;
    }
    let m = map.view.map_point(cursor, screen);
    let Some(st) = game.state.as_mut() else { return };
    let remove = |st: &mut crate::engine::GameState, picker: &mut MarkerPicker, rev: &mut MarkerRev, i: usize| {
        st.map_markers.remove(i);
        picker.removed(i);
        rev.0 += 1;
        info!("map marker {i} removed");
    };
    if map.show_special
        && let Some(i) = markers::marker_at(&st.map_markers, &db.data, m)
    {
        remove(st, &mut picker, &mut rev, i);
        return;
    }
    // W_POIMarker's and W_WorldMarker's double click destroy the last placed marker; with none
    // they leave the click unhandled and it falls through to the map.
    let explored = &st.pois_explored;
    let on_world = map.show_special && world.0.iter().any(|w| markers::on_box(db.data.to_map(w.pos), 24.0 * 0.8, m));
    let on_poi = map.show_pois && db.pois.iter().any(|p| explored.contains(&p.actor) && (p.large() || map.view.small_pois_shown()) && markers::on_box(db.data.to_map(p.pos), 24.0, m));
    if (on_world || on_poi)
        && let Some(i) = picker.last
    {
        remove(st, &mut picker, &mut rev, i);
        return;
    }
    // Marker Location: a WorldStatic trace from 10 km up to 10 km down; the landscape (or a prop
    // on it) is what it hits, and off the landscape nothing is.
    let xz = db.data.to_world(m);
    let Some(tr) = terrain else { return };
    let (lo, hi) = (tr.origin, tr.origin + tr.extent());
    if xz.x < lo.x || xz.y < lo.y || xz.x > hi.x || xz.y > hi.y {
        return;
    }
    const ABOVE: f32 = 300.0;
    let ground = tr.height(xz.x, xz.y);
    let top = rays.cast(Vec3::new(xz.x, ground + ABOVE, xz.y), Vec3::NEG_Y, ABOVE).map_or(ground, |d| ground + ABOVE - d);
    let at = Vec3::new(xz.x, top.max(ground), xz.y);
    st.map_markers.push(picker.marker(at, markers::compass_on(&store)));
    picker.last = Some(st.map_markers.len() - 1);
    rev.0 += 1;
    info!("map marker {} placed at {:.1} ({:?})", st.map_markers.len() - 1, at, picker.icon);
}

/// W_POIMarkerTooltip beside the cursor while a marker is hovered.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tooltip(
    mut commands: Commands,
    map: Res<MapView>,
    db: Res<MapDb>,
    font: Res<MenuFont>,
    assets: Res<AssetServer>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    (markers, specials, world): (Query<(&PoiMarker, &Hovered, &InheritedVisibility)>, Query<(&SpecialMarker, &Hovered, &InheritedVisibility)>, Res<WorldMarkers>),
    mut tips: Query<(Entity, &mut Node), With<PoiTooltip>>,
    mut shown: Local<Option<(bool, usize)>>,
) {
    let hovered = if map.open {
        markers.iter().find(|(_, h, v)| h.get() && v.get()).map(|(m, _, _)| (false, m.0)).or_else(|| specials.iter().find(|(_, h, v)| h.get() && v.get()).map(|(m, _, _)| (true, m.0)))
    } else {
        None
    };
    let cursor = local(&windows, &scale).and_then(|(_, c)| c).unwrap_or_default();
    if hovered != *shown {
        *shown = hovered;
        for (e, _) in &tips {
            commands.entity(e).despawn();
        }
        let text = |s: String, size: f32, c: Color| (label_colored(&font, s, size, c), Pickable::IGNORE);
        let boxed = |c: Color| (Node { padding: UiRect::all(Val::Px(10.0)), margin: UiRect::bottom(Val::Px(2.0)), flex_direction: FlexDirection::Column, ..default() }, BackgroundColor(c), Pickable::IGNORE);
        let indent = || Node { margin: UiRect::left(Val::Px(15.0)), ..default() };
        let root = || (Node { position_type: PositionType::Absolute, width: Val::Px(325.0), flex_direction: FlexDirection::Column, ..default() }, GlobalZIndex(45), PoiTooltip, MapUi, Pickable::IGNORE, DespawnOnExit(AppState::InGame));
        // W_WorldMarkerTooltip: the icon in its colour and the name, then the description
        // (DescriptionBorder collapsed when empty), in the POI tooltip's boxes.
        if let Some(m) = hovered.filter(|h| h.0).and_then(|(_, i)| world.0.get(i)) {
            let root = commands.spawn(root()).id();
            let title = commands
                .spawn(boxed(TIP))
                .with_children(|b| {
                    b.spawn((Node { align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }, Pickable::IGNORE)).with_children(|r| {
                        r.spawn((Node { width: Val::Px(50.0), height: Val::Px(50.0), ..default() }, ImageNode::new(assets.load(m.texture.clone())).with_color(m.colour), Pickable::IGNORE));
                        r.spawn(text(m.title.to_uppercase(), 18.0, Color::WHITE));
                    });
                })
                .id();
            commands.entity(root).add_child(title);
            if !m.description.is_empty() {
                let desc = commands.spawn(boxed(TIP)).with_child((text(m.description.clone(), 10.0, Color::WHITE), indent())).id();
                commands.entity(root).add_child(desc);
            }
            return;
        }
        let Some(poi) = hovered.filter(|h| !h.0).and_then(|(_, i)| db.pois.get(i)) else { return };
        let root = commands.spawn(root()).id();
        let title = commands
            .spawn(boxed(if poi.contamination > 0 { TIP_CONTAMINATED } else { TIP }))
            .with_children(|b| {
                b.spawn((Node { align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }, Pickable::IGNORE)).with_children(|r| {
                    r.spawn((Node { width: Val::Px(50.0), height: Val::Px(50.0), ..default() }, ImageNode::new(assets.load(format!("{POI_TEX}/{}.png", poi.image))).with_color(poi.colour), Pickable::IGNORE));
                    r.spawn(text(poi.name.to_uppercase(), 18.0, Color::WHITE));
                });
                if poi.contamination != 0 {
                    b.spawn((Node { align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }, Pickable::IGNORE)).with_children(|r| {
                        r.spawn(text(format!("Level {}", poi.contamination), 10.0, Color::WHITE));
                        r.spawn((Node { width: Val::Px(20.0), height: Val::Px(20.0), ..default() }, ImageNode::new(assets.load("game/SurrounDead/Content/UI/Textures/UI_filter.png")), Pickable::IGNORE));
                    });
                }
            })
            .id();
        commands.entity(root).add_child(title);
        if poi.major {
            let info = commands
                .spawn(boxed(TIP))
                .with_children(|b| {
                    b.spawn((text(format!("0/{} POI Boss Defeated", poi.bosses), 10.0, Color::WHITE), indent()));
                    b.spawn((text("0/1 Special Container Looted".into(), 10.0, Color::WHITE), indent()));
                })
                .id();
            let loot = commands
                .spawn(boxed(TIP))
                .with_children(|b| {
                    b.spawn((Node { align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }, Pickable::IGNORE)).with_children(|r| {
                        r.spawn((Node { width: Val::Px(25.0), height: Val::Px(25.0), ..default() }, ImageNode::new(assets.load("game/SurrounDead/Content/UI/Textures/magnifier.png")), Pickable::IGNORE));
                        r.spawn(text("LOOT TO FIND".into(), 14.0, Color::WHITE));
                    });
                    b.spawn((text(poi.loot.replace("\r\n", "\n"), 10.0, Color::WHITE), indent()));
                })
                .id();
            commands.entity(root).add_children(&[info, loot]);
        }
        let desc = commands.spawn(boxed(TIP)).with_child((text(poi.description.clone(), 10.0, Color::WHITE), indent())).id();
        commands.entity(root).add_child(desc);
        return;
    }
    for (_, mut n) in &mut tips {
        n.left = Val::Px(cursor.x + 16.0);
        n.top = Val::Px(cursor.y + 16.0);
    }
}

/// BP_POIManager.DistanceCheck for every unexplored POI on its own 1-4 s timer.
#[allow(clippy::too_many_arguments)]
fn discover(
    mut commands: Commands,
    time: Res<Time>,
    db: Res<MapDb>,
    mut checks: ResMut<Checks>,
    mut game: ResMut<CurrentGame>,
    store: Res<SettingsStore>,
    font: Res<MenuFont>,
    assets: Res<AssetServer>,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
    banners: Query<Entity, With<Discovered>>,
) {
    let Ok(me) = player.single() else { return };
    let Some(state) = game.state.as_mut() else { return };
    let mut rng = rand::thread_rng();
    if checks.0.len() != db.pois.len() {
        // ReceiveBeginPlay: a 3 s delay, then the looping timer.
        checks.0 = db.pois.iter().map(|_| 3.0 + rng.gen_range(1.0..4.0)).collect();
    }
    let dt = time.delta_secs();
    for (i, poi) in db.pois.iter().enumerate() {
        checks.0[i] -= dt;
        if checks.0[i] > 0.0 {
            continue;
        }
        checks.0[i] = rng.gen_range(1.0..4.0);
        if state.pois_explored.contains(&poi.actor) || me.translation.distance(poi.pos) >= poi.radius {
            continue;
        }
        state.pois_explored.insert(poi.actor.clone());
        let xp = poi.xp(state.progression.level, super::survival::difficulty(&store, "General", "XP MULTIPLIER"));
        state.progression.add_xp(xp.round() as u64);
        info!("POI discovered: {} (+{} XP)", poi.name, xp.round());
        // HUD_Game.DiscoverPOI replaces the banner.
        for e in &banners {
            commands.entity(e).despawn();
        }
        spawn_banner(&mut commands, &font, &assets, poi);
    }
}

/// POIDiscoveredUI: bottom left, alignment (0, 4) of its own height (about 96 px: ours puts its
/// bottom 3 heights up); fade-left tinted black behind "POI DISCOVERED" and the name and icon.
fn spawn_banner(commands: &mut Commands, font: &MenuFont, assets: &AssetServer, poi: &Poi) {
    const HEIGHT: f32 = 96.0;
    let grey = Color::linear_rgb(0.567708, 0.567708, 0.567708);
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), bottom: Val::Px(3.0 * HEIGHT), padding: UiRect::new(Val::Px(50.0), Val::Px(100.0), Val::Px(0.0), Val::Px(0.0)), flex_direction: FlexDirection::Column, ..default() },
        // A Border's brush fills it (Bevy's default image mode keeps the texture's aspect).
        ImageNode::new(assets.load("game/SurrounDead/Content/UI/Textures/fade-left.png")).with_color(Color::BLACK.with_alpha(0.0)).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
        Discovered(0.0),
        DiscoveredBack,
        GlobalZIndex(5),
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
        children![
            (label_colored(font, "POI DISCOVERED", 16.0, grey.with_alpha(0.0)), DiscoveredBack),
            (
                Node { align_items: AlignItems::Center, ..default() },
                children![
                    (label_colored(font, poi.name.to_uppercase(), 32.0, Color::WHITE.with_alpha(0.0)), DiscoveredFront),
                    (
                        Node { width: Val::Px(60.0), height: Val::Px(60.0), margin: UiRect::new(Val::Px(10.0), Val::Px(5.0), Val::Px(5.0), Val::Px(5.0)), ..default() },
                        ImageNode::new(assets.load(format!("{POI_TEX}/{}.png", poi.image))).with_color(poi.colour.with_alpha(0.0)),
                        DiscoveredFront,
                    ),
                ],
            ),
        ],
    ));
}

/// A FadeStart track: (seconds, opacity) keys, eased between (cubic keys with flat tangents).
pub(crate) fn track(keys: &[(f32, f32)], t: f32) -> f32 {
    let Some(i) = keys.iter().position(|k| k.0 > t) else { return keys.last().map_or(0.0, |k| k.1) };
    if i == 0 {
        return keys[0].1;
    }
    let ((t0, a), (t1, b)) = (keys[i - 1], keys[i]);
    let s = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
    a + (b - a) * s * s * (3.0 - 2.0 * s)
}

/// FadeStart (60000 ticks/s): ZoneArea in by 1.76 s, out from 4.29 to 5.55 s; the name and icon
/// in from 1.25 to 2.25 s, out from 4.25 to 4.75 s. The widget goes when it ends (5.75 s).
pub const BANNER_BACK: [(f32, f32); 4] = [(0.0, 0.0), (105899.0 / 60000.0, 1.0), (257183.0 / 60000.0, 1.0), (332825.0 / 60000.0, 0.0)];
pub const BANNER_FRONT: [(f32, f32); 5] = [(0.0, 0.0), (1.25, 0.0), (2.25, 1.0), (4.25, 1.0), (4.75, 0.0)];
const BANNER_END: f32 = 344928.0 / 60000.0;

#[allow(clippy::type_complexity)]
fn fade_banner(
    mut commands: Commands,
    time: Res<Time>,
    mut banners: Query<(Entity, &mut Discovered, &Children)>,
    children: Query<&Children>,
    mut images: Query<(&mut ImageNode, Has<DiscoveredFront>)>,
    mut texts: Query<(&mut TextColor, Has<DiscoveredFront>)>,
) {
    for (e, mut age, kids) in &mut banners {
        age.0 += time.delta_secs();
        if age.0 >= BANNER_END {
            commands.entity(e).despawn();
            continue;
        }
        let (back, front) = (track(&BANNER_BACK, age.0), track(&BANNER_FRONT, age.0));
        let mut all = vec![e];
        let mut stack: Vec<Entity> = kids.iter().collect();
        while let Some(k) = stack.pop() {
            all.push(k);
            if let Ok(c) = children.get(k) {
                stack.extend(c.iter());
            }
        }
        // RenderOpacity multiplies down the tree: the name and icon also fade with ZoneArea.
        for k in all {
            if let Ok((mut img, f)) = images.get_mut(k) {
                img.color.set_alpha(if f { back * front } else { back });
            }
            if let Ok((mut c, f)) = texts.get_mut(k) {
                c.0.set_alpha(if f { back * front } else { back });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_to_map_matches_the_bp() {
        let d = MapData::default();
        // The middle is the image's centre; +X (north) is up, +Y (east) is right.
        assert!(d.to_map(Vec3::new(782.5715, 0.0, 752.5757)).length() < 1e-3);
        assert!(d.to_map(Vec3::new(782.5715 + 112.5, 0.0, 752.5757)).abs_diff_eq(Vec2::new(0.0, -100.0), 1e-2));
        assert!(d.to_map(Vec3::new(782.5715, 0.0, 752.5757 + 112.5)).abs_diff_eq(Vec2::new(100.0, 0.0), 1e-2));
        // Clamped to the image.
        assert_eq!(d.to_map(Vec3::new(-10000.0, 0.0, 10000.0)), Vec2::new(2000.0, 2000.0));
        // Ellwood ("north of the valley") lands top right, on its town in the capture.
        let e = d.to_map(Vec3::new(2254.54, 13.77, 1870.32)) + Vec2::splat(2000.0);
        assert!((e.x / MAP_PX - 0.748).abs() < 0.01 && (e.y / MAP_PX - 0.173).abs() < 0.01, "{e}");
    }

    #[test]
    fn cursor_goes_back_to_the_world() {
        let d = MapData::default();
        let screen = Vec2::new(1920.0, 1080.0);
        let mut v = View::default();
        v.wheel(true, Vec2::new(500.0, 300.0), screen);
        v.drag(Vec2::new(-40.0, 25.0), screen);
        let p = Vec3::new(1234.5, 0.0, 987.6);
        // CalculateMarkerToWorld undoes Calculate Actor Position through the view.
        let cursor = v.screen_point(d.to_map(p), screen);
        let back = d.to_world(v.map_point(cursor, screen));
        assert!(back.abs_diff_eq(Vec2::new(p.x, p.z), 1e-2), "{back}");
    }

    #[test]
    fn yaw_is_ue_degrees() {
        assert!(ue_yaw(Vec3::X).abs() < 1e-4);
        assert!((ue_yaw(Vec3::Z) - 90.0).abs() < 1e-4);
        assert!((ue_yaw(-Vec3::Z) + 90.0).abs() < 1e-4);
    }

    #[test]
    fn zoom_steps_like_the_widget() {
        let screen = Vec2::new(1920.0, 1080.0);
        let mut v = View::default();
        // Zooming in about the centre keeps the centre.
        v.wheel(true, screen / 2.0, screen);
        assert_eq!(v.step, 1);
        assert!((v.scale - 1.0 / ZOOM_FACTOR).abs() < 1e-5 && v.offset.length() < 1e-3);
        assert_eq!(v.arrow_scale, 0.5);
        for _ in 0..10 {
            v.wheel(true, screen / 2.0, screen);
        }
        assert_eq!(v.step, MAX_ZOOM);
        // Out to MinZoom, then one more: the whole map at 0.2675.
        for _ in 0..(MAX_ZOOM - MIN_ZOOM) {
            v.wheel(false, screen / 2.0, screen);
        }
        assert_eq!(v.step, MIN_ZOOM);
        assert_eq!(v.arrow_scale, 2.0);
        assert!(v.small_pois_shown());
        v.wheel(false, Vec2::ZERO, screen);
        assert_eq!((v.step, v.scale, v.offset, v.arrow_scale), (-4, 0.2675, Vec2::splat(0.5), 3.0));
        assert!(!v.small_pois_shown());
        // Stuck there: no further step, no dragging.
        v.wheel(false, Vec2::ZERO, screen);
        assert_eq!(v.step, -4);
        v.drag(Vec2::new(300.0, 0.0), screen);
        assert_eq!(v.offset, Vec2::splat(0.5));
        // Back in: the scale is clamped to 0.405 first.
        v.wheel(true, screen / 2.0, screen);
        assert!((v.scale - 0.405 / ZOOM_FACTOR).abs() < 1e-5);
    }

    #[test]
    fn zoom_follows_the_cursor_and_drag_stops_at_the_edges() {
        let screen = Vec2::new(1920.0, 1080.0);
        let mut v = View::default();
        // Zooming in at a point right of centre moves the map left, keeping that point put.
        let cursor = screen / 2.0 + Vec2::new(400.0, 0.0);
        v.wheel(true, cursor, screen);
        assert!((v.offset.x + 400.0 * (1.0 / ZOOM_FACTOR - 1.0)).abs() < 1e-2);
        // Dragging far stops where the map's edge meets the screen's.
        v.drag(Vec2::new(1e5, -1e5), screen);
        let b = (Vec2::splat(MAP_PX) * v.scale - screen) * 0.5;
        assert!(v.offset.abs_diff_eq(Vec2::new(b.x, -b.y), 1e-2));
    }

    #[test]
    fn level_pois_load() {
        let Ok(a) = std::fs::read_to_string(format!("{}/actors.json", crate::level::LEVEL_DIR)) else { return };
        let actors: Vec<crate::level::ActorInfo> = serde_json::from_str(&a).unwrap();
        let db = MapDb::from_actors(&actors);
        assert_eq!(db.data.world_size, 450000.0);
        assert!((db.data.middle - Vec2::new(78257.15, 75257.57)).length() < 0.1);
        assert_eq!(db.pois.len(), 187);
        assert_eq!(db.pois.iter().filter(|p| p.major).count(), 17);
        let e = db.pois.iter().find(|p| p.name == "Ellwood").unwrap();
        assert!(e.major && e.large() && e.icon_scale() == 5.0 && e.image == "Marker_City", "{e:?}");
        assert!(e.description.contains("north"));
        let barn = db.pois.iter().find(|p| p.name == "Overgrown Barn").unwrap();
        assert!(!barn.major && barn.icon_scale() == 2.0 && (barn.radius - 19.58).abs() < 0.01);
        // Every icon was exported.
        for p in &db.pois {
            assert!(!p.image.is_empty() && p.radius > 0.0, "{p:?}");
        }
        // XP: level x 250 (major) / 50 x 1.05.
        assert_eq!(e.xp(2, 1.0), 525.0);
        assert!((barn.xp(1, 2.0) - 105.0).abs() < 1e-3);
    }

    #[test]
    fn banner_fades_in_and_out() {
        assert_eq!(track(&BANNER_BACK, 0.0), 0.0);
        assert_eq!(track(&BANNER_BACK, 3.0), 1.0);
        assert_eq!(track(&BANNER_FRONT, 1.0), 0.0);
        assert_eq!(track(&BANNER_FRONT, 3.0), 1.0);
        assert_eq!(track(&BANNER_FRONT, 5.0), 0.0);
        assert!(track(&BANNER_BACK, 5.0) > 0.0 && track(&BANNER_BACK, 5.6) == 0.0);
    }
}
