//! The in-game HUD, rebuilt from `Inventory/BP_Ingame` (plus `UI/Widgets/Player/Compass`):
//!
//! - bottom-left `Stats`: hunger / thirst / radiation rings (`ProgressBarCircular`, 40 px,
//!   thickness 0.15) with their icons, health and stamina bars (250 x 12), level + XP bar;
//! - bottom-centre `ActionBar`: nine 100 x 50 equip slots numbered 1-9;
//! - centre `BoxCrosshair`, top-centre compass strip, status effects above the stats.
//!
//! Positions, sizes and colours are the widget's own (UMG linear colours, 1920x1080 canvas).

use bevy::prelude::*;

use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame, MenuScreen};

const TEX: &str = "game/SurrounDead/Content/UI/Textures";
const STAT_TEXT: Color = Color::linear_rgba(0.75, 0.75, 0.75, 1.0);
const RING_BG: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.25);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Ring {
    Hunger,
    Thirst,
    Radiation,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Bar {
    Health,
    Stamina,
    Xp,
}

#[derive(Component)]
struct RingText(Ring);
#[derive(Component)]
struct BarText(Bar);
#[derive(Component)]
struct LevelText;
#[derive(Component)]
struct XpText;
#[derive(Component)]
struct CompassStrip;
#[derive(Component)]
pub(crate) struct MedicalIcon(MedicalUi);

/// The `MedicalUI_*` widgets `CreateMedicalUI` adds to `CurrentEffectsVB`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MedicalUi {
    Bleed,
    HeavyBleed,
    BrokenBone,
    Radiation,
}

impl MedicalUi {
    const ALL: [MedicalUi; 4] = [MedicalUi::Bleed, MedicalUi::HeavyBleed, MedicalUi::BrokenBone, MedicalUi::Radiation];
    fn texture(self) -> &'static str {
        match self {
            MedicalUi::Bleed => "bleed01",
            MedicalUi::HeavyBleed => "bleed02",
            MedicalUi::BrokenBone => "brokenbone",
            MedicalUi::Radiation => "rad01",
        }
    }
    fn active(self, e: &super::medical::MedicalEffects) -> bool {
        match self {
            MedicalUi::Bleed => e.bleed.is_some(),
            MedicalUi::HeavyBleed => e.heavy_bleed.is_some(),
            MedicalUi::BrokenBone => e.broken_bone.is_some(),
            MedicalUi::Radiation => e.radiation_sickness,
        }
    }
}
#[derive(Component)]
struct HudRoot;
/// Action-bar slot `n` (1-9) picture.
#[derive(Component)]
struct ActionIcon(usize);

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::InGame), spawn_hud)
        .add_systems(Update, (update_stats, update_compass, hide_under_inventory, update_action_bar).run_if(in_state(AppState::InGame)))
        .add_systems(OnEnter(MenuScreen::None), |mut q: Query<&mut Visibility, With<HudRoot>>| q.iter_mut().for_each(|mut v| *v = Visibility::Inherited))
        .add_systems(OnExit(MenuScreen::None), |mut q: Query<&mut Visibility, With<HudRoot>>| q.iter_mut().for_each(|mut v| *v = Visibility::Hidden));
}

fn ring_colour(r: Ring) -> Color {
    match r {
        Ring::Hunger => Color::linear_rgb(0.6, 0.174463, 0.0),
        Ring::Thirst => Color::linear_rgb(0.0, 0.401554, 0.5),
        Ring::Radiation => Color::linear_rgb(0.0, 0.4, 0.033258),
    }
}

/// `ProgressBarCircular`: a round border filled clockwise from the top by a hard-stopped conic.
fn ring_gradient(r: Ring, pct: f32) -> BorderGradient {
    let a = pct.clamp(0.0, 1.0) * std::f32::consts::TAU;
    let fill = ring_colour(r);
    BorderGradient::from(ConicGradient::new(
        UiPosition::CENTER,
        vec![
            AngularColorStop::new(fill, 0.0),
            AngularColorStop::new(fill, a),
            AngularColorStop::new(RING_BG, a),
            AngularColorStop::new(RING_BG, std::f32::consts::TAU),
        ],
    ))
}

fn bar_colours(b: Bar) -> (Color, Color) {
    match b {
        Bar::Health => (Color::linear_rgb(0.325, 0.000802, 0.014047), Color::linear_rgba(0.2, 0.0, 0.0, 0.5)),
        Bar::Stamina => (Color::linear_rgb(1.0, 0.378381, 0.0), Color::linear_rgba(0.4, 0.360869, 0.0, 0.5)),
        Bar::Xp => (Color::linear_rgb(0.258273, 0.5, 0.05), Color::linear_rgba(0.0, 0.0, 0.0, 0.25)),
    }
}

fn spawn_hud(mut commands: Commands, font: Res<MenuFont>, assets: Res<AssetServer>) {
    let font = &*font;
    let root = commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            HudRoot,
            DespawnOnExit(AppState::InGame),
        ))
        .id();

    // Crosshair (InteractCrosshair: BoxCrosshair, 20 x 20, centred).
    let cross = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Percent(50.0), margin: UiRect::new(Val::Px(-10.0), Val::ZERO, Val::Px(-10.0), Val::ZERO), width: Val::Px(20.0), height: Val::Px(20.0), ..default() },
            ImageNode::new(assets.load(format!("{TEX}/BoxCrosshair.png"))),
        ))
        .id();

    // Compass: a 750 px window onto the 2700 x 30 strip, marker over its centre.
    let compass = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Px(11.0), margin: UiRect::left(Val::Px(-375.0)), width: Val::Px(750.0), height: Val::Px(30.0), ..default() },
            ImageNode { image: assets.load(format!("{TEX}/Compass_Points.png")), rect: Some(Rect::new(0.0, 0.0, 750.0, 30.0)), ..default() },
            CompassStrip,
        ))
        .id();
    let marker = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Px(-9.0), margin: UiRect::left(Val::Px(-6.0)), width: Val::Px(12.0), height: Val::Px(60.0), ..default() },
            ImageNode::new(assets.load(format!("{TEX}/Compass_Marker.png"))),
        ))
        .id();

    // CurrentEffectsOverlay: bottom-left anchored at (230, -115), 60 x 30, alignment (-1, 1), so
    // its left edge is at 290 and its bottom 115 up; CurrentEffectsVB stacks the MedicalUI widgets
    // (30 x 30, 50 % black behind the icon, 5 bottom padding) from its top down.
    let effects = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(290.0),
            bottom: Val::Px(115.0),
            width: Val::Px(60.0),
            height: Val::Px(30.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    for m in MedicalUi::ALL {
        let icon = commands
            .spawn((
                Node { width: Val::Px(30.0), height: Val::Px(30.0), margin: UiRect::bottom(Val::Px(5.0)), display: Display::None, flex_shrink: 0.0, ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                MedicalIcon(m),
                children![(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode::new(assets.load(format!("{TEX}/{}.png", m.texture()))))],
            ))
            .id();
        commands.entity(effects).add_child(icon);
    }

    let stats = spawn_stats(&mut commands, font, &assets);
    let action_bar = spawn_action_bar(&mut commands, font);
    commands.entity(root).add_children(&[cross, compass, marker, effects, stats, action_bar]);
}

fn bar(commands: &mut Commands, font: &MenuFont, b: Bar, width: Val, height: f32) -> Entity {
    let (fill, bg) = bar_colours(b);
    commands
        .spawn((
            Node { width, height: Val::Px(height), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(bg),
            children![
                (Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: Val::Percent(100.0), ..default() }, BackgroundColor(fill), b),
                (label_colored(font, if b == Bar::Xp { "" } else { "100%" }, 8.0, STAT_TEXT), BarText(b)),
            ],
        ))
        .id()
}

/// `Stats`: 275 x 135 at (10, -10) from the bottom-left, 50% black, 10 px corners.
fn spawn_stats(commands: &mut Commands, font: &MenuFont, assets: &AssetServer) -> Entity {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                bottom: Val::Px(10.0),
                width: Val::Px(275.0),
                height: Val::Px(135.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.0, 0.5)),
        ))
        .id();

    // Rings row with their percentages tucked at the bottom right of each.
    let rings = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    for (r, icon, text_left) in [(Ring::Hunger, "hunger", 60.5), (Ring::Thirst, "thirst", 149.0), (Ring::Radiation, "rad", 237.5)] {
        let cell = commands
            .spawn((
                Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                children![(
                    Node {
                        width: Val::Px(40.0),
                        height: Val::Px(40.0),
                        border: UiRect::all(Val::Px(6.0)),
                        border_radius: BorderRadius::MAX,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ring_gradient(r, 1.0),
                    r,
                    children![(
                        Node { width: Val::Px(25.0), height: Val::Px(25.0), position_type: PositionType::Absolute, ..default() },
                        ImageNode::new(assets.load(format!("{TEX}/{icon}.png"))).with_color(ring_colour(r)),
                    )],
                )],
            ))
            .id();
        let pct = commands
            .spawn((
                label_colored(font, "100%", 8.0, STAT_TEXT),
                Node { position_type: PositionType::Absolute, left: Val::Px(text_left - 5.0), bottom: Val::Px(-2.0), ..default() },
                RingText(r),
            ))
            .id();
        commands.entity(rings).add_children(&[cell, pct]);
    }

    let bars = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    let health = bar(commands, font, Bar::Health, Val::Px(250.0), 12.0);
    let stamina = bar(commands, font, Bar::Stamina, Val::Px(250.0), 12.0);
    commands.entity(bars).add_children(&[health, stamina]);

    // Level row: "LVL: n", XP bar, "xp / max".
    let level = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
    let row = commands
        .spawn((Node { align_items: AlignItems::Center, ..default() }, children![(label_colored(font, "LVL: 1", 10.0, STAT_TEXT), Node { margin: UiRect::horizontal(Val::Px(5.0)), ..default() }, LevelText)]))
        .id();
    let xp = bar(commands, font, Bar::Xp, Val::Auto, 6.0);
    commands.entity(xp).insert(Node { flex_grow: 1.0, height: Val::Px(6.0), ..default() });
    commands.entity(row).add_child(xp);
    let xp_text = commands
        .spawn((Node { justify_content: JustifyContent::FlexEnd, padding: UiRect::right(Val::Px(5.0)), ..default() }, children![(label_colored(font, "0 / 500", 8.0, STAT_TEXT), XpText)]))
        .id();
    commands.entity(level).add_children(&[row, xp_text]);

    commands.entity(panel).add_children(&[rings, bars, level]);
    panel
}

/// `ActionBar`: rounded (15) dark strip of nine 2x1 equip slots (50 px cells), numbered in
/// the game's order: primary, secondary, sidearm, melee, throwable, hotbar 6-8, fishing rod.
fn spawn_action_bar(commands: &mut Commands, font: &MenuFont) -> Entity {
    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::ZERO,
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-(9.0 * 105.0) / 2.0)),
                border_radius: BorderRadius::all(Val::Px(15.0)),
                ..default()
            },
            BackgroundColor(Color::linear_rgba(0.072917, 0.072917, 0.072917, 0.7)),
        ))
        .id();
    for n in 1..=9 {
        let slot = commands
            .spawn((
                Node { margin: UiRect::horizontal(Val::Px(2.5)), width: Val::Px(100.0), height: Val::Px(50.0), ..default() },
                children![
                    (Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.0, 0.5))),
                    (Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode::default(), Visibility::Hidden, ActionIcon(n)),
                    (label_colored(font, n.to_string(), 14.0, Color::linear_rgba(0.78125, 0.78125, 0.78125, 0.5)), Node { margin: UiRect::new(Val::Px(5.0), Val::ZERO, Val::Px(1.0), Val::ZERO), ..default() }),
                ],
            ))
            .id();
        commands.entity(bar).add_child(slot);
    }
    bar
}

#[allow(clippy::type_complexity)]
fn update_stats(
    game: Res<CurrentGame>,
    mut rings: Query<(&Ring, &mut BorderGradient)>,
    mut ring_texts: Query<(&RingText, &mut Text), (Without<BarText>, Without<LevelText>, Without<XpText>)>,
    mut bars: Query<(&Bar, &mut Node)>,
    mut bar_texts: Query<(&BarText, &mut Text), (Without<RingText>, Without<LevelText>, Without<XpText>)>,
    mut level: Query<&mut Text, (With<LevelText>, Without<RingText>, Without<BarText>, Without<XpText>)>,
    mut xp: Query<&mut Text, (With<XpText>, Without<RingText>, Without<BarText>, Without<LevelText>)>,
    mut medical: Query<(&MedicalIcon, &mut Node), (Without<Bar>, Without<Ring>)>,
) {
    let Some(state) = &game.state else { return };
    let s = &state.survival;
    let p = &state.progression;
    let xp_max = p.xp_for_next_level().max(1);
    let ring_pct = |r: Ring| match r {
        Ring::Hunger => s.hunger / 100.0,
        Ring::Thirst => s.thirst / 100.0,
        // The radiation ring shows resistance left, full when clean.
        Ring::Radiation => 1.0 - s.radiation / 100.0,
    };
    let bar_pct = |b: Bar| match b {
        Bar::Health => s.health / s.max_health,
        Bar::Stamina => s.stamina / s.max_stamina,
        Bar::Xp => p.current_xp as f32 / xp_max as f32,
    };
    for (r, mut g) in &mut rings {
        *g = ring_gradient(*r, ring_pct(*r));
    }
    for (rt, mut t) in &mut ring_texts {
        let v = format!("{:.0}%", ring_pct(rt.0) * 100.0);
        if t.0 != v {
            t.0 = v;
        }
    }
    for (b, mut n) in &mut bars {
        n.width = Val::Percent(bar_pct(*b).clamp(0.0, 1.0) * 100.0);
    }
    for (bt, mut t) in &mut bar_texts {
        if bt.0 == Bar::Xp {
            continue;
        }
        let v = format!("{:.0}%", bar_pct(bt.0) * 100.0);
        if t.0 != v {
            t.0 = v;
        }
    }
    if let Ok(mut t) = level.single_mut() {
        let v = format!("LVL: {}", p.level);
        if t.0 != v {
            t.0 = v;
        }
    }
    if let Ok(mut t) = xp.single_mut() {
        let v = format!("{} / {}", p.current_xp, xp_max);
        if t.0 != v {
            t.0 = v;
        }
    }
    for (MedicalIcon(m), mut node) in &mut medical {
        let want = if m.active(&state.effects) { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
}

/// Heading = camera yaw in UE terms (0 = +X = north, clockwise). The strip starts at East and
/// runs 5 px per degree; pick the 750 px window centred on the heading.
fn update_compass(cams: Query<&GlobalTransform, With<Camera3d>>, mut strip: Query<&mut ImageNode, With<CompassStrip>>) {
    let (Some(cam), Ok(mut img)) = (cams.iter().next(), strip.single_mut()) else { return };
    let f = cam.forward();
    let heading = f.z.atan2(f.x).to_degrees().rem_euclid(360.0);
    let mut x = (heading - 90.0).rem_euclid(360.0) * 5.0;
    if x < 375.0 {
        x += 1800.0;
    }
    img.rect = Some(Rect::new(x - 375.0, 0.0, x + 375.0, 30.0));
}

/// The inventory screen covers the HUD in the game; hide it while any overlay is open.
fn hide_under_inventory(overlay: Res<super::GameOverlay>, screen: Res<State<MenuScreen>>, mut q: Query<&mut Visibility, With<HudRoot>>) {
    if !overlay.is_changed() {
        return;
    }
    let want = if overlay.is_open() || *screen.get() != MenuScreen::None { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in &mut q {
        *v = want;
    }
}

/// Equipped weapons/tools show on their action-bar slot (the JSIContainer_* EquipTo slots).
fn update_action_bar(
    mut game: ResMut<CurrentGame>,
    db: Res<super::ItemDb>,
    mut icons: ResMut<super::icons::ItemIcons>,
    mut images: ResMut<Assets<Image>>,
    mut slots: Query<(&ActionIcon, &mut ImageNode, &mut Visibility)>,
) {
    if !game.is_changed() {
        return;
    }
    let inv = super::player_inv(&mut game).clone();
    for (ActionIcon(n), mut img, mut vis) in &mut slots {
        match inv.action_slot(*n).and_then(|s| icons.get(&s.item, &db, &mut images)) {
            Some(h) => {
                img.image = h;
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}
