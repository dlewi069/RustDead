//! The journal's Skills page: JournalUI + Journal_SkillsUI (widget trees; IA_Journal = J).
//!
//! - The notebook page (SkillsPageBlank) with its bookmark tabs (INFO, SKILLS, QUESTS); only the
//!   Skills page is ported, so the other tabs do nothing.
//! - The passive skills (PassiveSkillsComponent) in the SkillLvlArea frame: name, level -> next
//!   level, the XP bar and "XP / MaxXP", and "x2" while the skill's magazine boost is running.
//! - The tech tree (SkillTree: TreeSize 640 x 1000, 70 px slots at the nodes' editor positions,
//!   1 px #191919 lines to their parents). SkillTreeWidgetIcon: the icon tinted by its
//!   Enum_SkillType (red / green / blue / grey), the button darkened to 0.1 until unlocked;
//!   clicking it is TryToUnlockTechnology; hovering shows SkillTreeTooltip (name in the type's
//!   colour, cost, description).
//! - "N POINTS" over SkillPointsBackground, and RESET POINTS (ResetTree; the game asks first in
//!   ResetSkillsSelect, ours doesn't).
//!
//! Positions are the widgets' CanvasPanelSlot / Overlay paddings on the 1920 x 1080 canvas (a
//! centred Overlay child moves by the whole padding difference, Slate's AlignChild). Inside each
//! passive row the bar and level texts are laid out more simply than the widget's nested overlays
//! (ours).

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button};

use super::interact::GameOverlay;
use crate::menus::widgets::{MenuFont, pt};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::progression::passive::{MAX_LEVEL, Passive};
use crate::progression::tech::TechDb;

const TEX: &str = "game/SurrounDead/Content/UI/Widgets/Journal";
const FONT: &str = "game/Blueprints/PluginsAndAssets/Font/F_ESRPG__RU_EN_.ttf";
const INK: Color = Color::BLACK;

/// Journal_SkillsUI's Overlay_0: 2112 x 1188 at alignment (0.15, 0.480263) on the left middle.
const PAGE: Vec2 = Vec2::new(2112.0, 1188.0);
const PAGE_ALIGN: Vec2 = Vec2::new(0.15, 0.480263);
/// The tree's SizeBox: alignment (-0.5, 0.175), 670 tall.
const TREE_ALIGN: Vec2 = Vec2::new(-0.5, 0.175);
const TREE_HEIGHT: f32 = 670.0;

/// Each passive row: (skill, SizeBox width, padding left, right, bottom) in PassiveSkillsOverlay.
const ROWS: [(Passive, f32, f32, f32, f32); 10] = [
    (Passive::Fitness, 310.0, 0.0, 225.0, 500.0),
    (Passive::Strength, 310.0, 0.0, 225.0, 440.0),
    (Passive::Toughness, 310.0, 0.0, 228.0, 380.0),
    (Passive::Sneaking, 315.0, 0.0, 228.0, 315.0),
    (Passive::FirstAid, 310.0, 0.0, 223.0, 250.0),
    (Passive::Marksmanship, 310.0, 0.0, 225.0, 185.0),
    (Passive::Reloading, 350.0, 120.0, 0.0, 500.0),
    (Passive::Thief, 350.0, 115.0, 0.0, 440.0),
    (Passive::Fishing, 360.0, 117.0, 0.0, 380.0),
    (Passive::Scavenging, 360.0, 117.0, 0.0, 320.0),
];

#[derive(Resource, Default)]
pub struct Journal {
    open: bool,
    /// Rebuild the page (opened, a skill unlocked, points reset).
    dirty: bool,
}

#[derive(Component)]
struct JournalUi;

/// A tree node's button.
#[derive(Component, Clone)]
pub struct TechButton(pub String);

#[derive(Component)]
struct ResetButton;

#[derive(Component)]
struct TechTooltip;

#[derive(Resource)]
struct Tree(TechDb);

pub fn plugin(app: &mut App) {
    app.init_resource::<Journal>()
        .insert_resource(Tree(TechDb::load()))
        .add_observer(on_click)
        .add_systems(Update, (toggle, build, tooltip).chain().run_if(in_state(AppState::InGame)));
}

/// The top-left of the page overlay (design px, from the canvas's left middle).
fn page_origin() -> Vec2 {
    Vec2::new(0.0, 540.0) - PAGE_ALIGN * PAGE
}

/// An absolutely placed box at `top_left` (canvas px from the top left) of `size`.
fn at(top_left: Vec2, size: Vec2) -> Node {
    crate::menus::widgets::anchored(Vec2::new(0.0, 0.5), Vec2::new(top_left.x, top_left.y - 540.0), size, Vec2::ZERO)
}

/// The tree's top left.
fn tree_origin() -> Vec2 {
    let width = 640.0;
    Vec2::new(0.0, 540.0) - TREE_ALIGN * Vec2::new(width, TREE_HEIGHT)
}

/// Slate's AlignChild for a centred Overlay slot: (allotted - child) / 2 + padding before - padding
/// after, on each axis. The child's top left within an overlay of `allotted` size; `pad` is
/// (left, top, right, bottom).
fn overlay_centred(allotted: Vec2, child: Vec2, pad: Vec4) -> Vec2 {
    (allotted - child) / 2.0 + Vec2::new(pad.x - pad.z, pad.y - pad.w)
}

/// Where a tab button's centre lands in the Buttons overlay (1921.9 x 1081.1 from (0, -0.54)).
fn tab_centre(left: f32, bottom: f32) -> Vec2 {
    let overlay = Vec2::new(1921.9219, 1081.081);
    Vec2::new(0.0, -0.5405) + overlay_centred(overlay, Vec2::ZERO, Vec4::new(left, 0.0, 0.0, bottom))
}

/// SkillTreeWidgetIcon's tint for an Enum_SkillType.
pub fn kind_colour(kind: u8) -> Color {
    match kind {
        0 => Color::linear_rgb(0.35, 0.0, 0.004546),
        1 => Color::linear_rgb(0.068966, 0.2, 0.0),
        2 => Color::linear_rgb(0.0, 0.138071, 0.4),
        _ => Color::linear_rgb(0.6, 0.6, 0.6),
    }
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, screen: Res<State<MenuScreen>>, mut overlay: ResMut<GameOverlay>, mut journal: ResMut<Journal>) {
    if *screen.get() == MenuScreen::None && keys.just_pressed(KeyCode::KeyJ) {
        match *overlay {
            GameOverlay::Journal => *overlay = GameOverlay::None,
            GameOverlay::None => *overlay = GameOverlay::Journal,
            _ => {}
        }
    }
    let open = *overlay == GameOverlay::Journal;
    if open != journal.open {
        journal.open = open;
        journal.dirty = true;
    }
}

#[allow(clippy::too_many_arguments)]
fn build(mut commands: Commands, mut journal: ResMut<Journal>, game: Res<CurrentGame>, tree: Res<Tree>, assets: Res<AssetServer>, uis: Query<Entity, With<JournalUi>>) {
    if !journal.dirty {
        return;
    }
    journal.dirty = false;
    for e in &uis {
        commands.entity(e).despawn();
    }
    let (Some(state), true) = (game.state.as_ref(), journal.open) else { return };
    let font: Handle<Font> = assets.load(FONT);
    let img = |p: &str| ImageNode::new(assets.load(format!("{TEX}/{p}.png")));
    let text = |s: String, size: f32| (Text::new(s), TextFont { font: font.clone().into(), font_size: pt(size), ..default() }, TextColor(INK), Pickable::IGNORE);
    // RenderTransform angles (UMG degrees, clockwise on screen).
    let turn = |deg: f32| UiTransform::from_rotation(Rot2::degrees(deg));
    // A box at `top_left` of `size` inside a parent (px from the parent's top left).
    let rel = |top_left: Vec2, size: Vec2| Node { position_type: PositionType::Absolute, left: Val::Px(top_left.x), top: Val::Px(top_left.y), width: Val::Px(size.x), height: Val::Px(size.y), ..default() };

    let root = commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            GlobalZIndex(40),
            JournalUi,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let add = |commands: &mut Commands, b: Entity| {
        commands.entity(root).add_child(b);
    };
    // The other pages' bookmarks behind the page, the Skills one in front of it (each turned 6°).
    let marker = |name: &str, x: f32, y: f32| (at(Vec2::new(x, 540.0 + y), Vec2::new(1920.0, 1080.0)), img(&format!("Markers/{name}")), turn(6.0), Pickable::IGNORE);
    let b = commands.spawn(marker("RedMarker", -189.0, -540.0)).id();
    add(&mut commands, b);
    let b = commands.spawn(marker("YellowMarker", -203.04, -498.0)).id();
    add(&mut commands, b);

    // Overlay_0 (turned 5°): the page and, centred on it, the passive skills and their frame.
    let page = commands.spawn((at(page_origin(), PAGE), turn(5.0), Pickable::IGNORE)).id();
    add(&mut commands, page);
    let fill = commands.spawn((rel(Vec2::ZERO, PAGE), img("Skills/SkillsPageBlank"), Pickable::IGNORE)).id();
    commands.entity(page).add_child(fill);
    let centred = |size: Vec2, l: f32, t: f32, r: f32, b: f32| overlay_centred(PAGE, size, Vec4::new(l, t, r, b));
    let frame = Vec2::new(700.0, 411.0);
    let frame_at = centred(frame, 0.0, 0.0, 42.5, 344.0);
    let f = commands.spawn((rel(frame_at, frame), img("Skills/SkillLvlArea"), Pickable::IGNORE)).id();
    commands.entity(page).add_child(f);
    for (skill, w, l, r, bottom) in ROWS {
        let p = state.passive.get(skill);
        let size = Vec2::new(w, 40.0);
        let row_at = centred(size, l, 0.0, r, bottom);
        let row_w = w;
        let next = (p.level + 1).min(MAX_LEVEL);
        let bar_x = row_w * 0.46;
        let bar_w = row_w * 0.40;
        let filled = if p.max_xp > 0.0 { (p.xp / p.max_xp).clamp(0.0, 1.0) } else { 0.0 };
        let name = format!("{}:", if skill == Passive::FirstAid { "FIRST AID" } else { skill.name() });
        let row = commands
            .spawn((
                rel(row_at, Vec2::new(row_w, 40.0)),
                Pickable::IGNORE,
                children![
                    (Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(4.0), ..default() }, children![text(name, 20.0)]),
                    (Node { position_type: PositionType::Absolute, left: Val::Px(bar_x - 22.0), top: Val::Px(9.0), ..default() }, children![text(p.level.to_string(), 16.0)]),
                    (Node { position_type: PositionType::Absolute, left: Val::Px(bar_x + bar_w + 6.0), top: Val::Px(9.0), ..default() }, children![text(next.to_string(), 16.0)]),
                    (
                        Node { position_type: PositionType::Absolute, left: Val::Px(bar_x), top: Val::Px(13.0), width: Val::Px(bar_w), height: Val::Px(15.0), border: UiRect::all(Val::Px(1.0)), ..default() },
                        BorderColor::all(INK),
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.08)),
                        children![(Node { width: Val::Percent(filled * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)))],
                    ),
                    (Node { position_type: PositionType::Absolute, left: Val::Px(bar_x), top: Val::Px(29.0), width: Val::Px(bar_w), justify_content: JustifyContent::Center, ..default() }, children![text(format!("{:.0} / {:.0}", p.xp, p.max_xp), 8.0)]),
                ],
            ))
            .id();
        if p.book {
            let x2 = commands.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(40.0), top: Val::Px(26.0), ..default() }, children![text("x2".to_string(), 12.0)])).id();
            commands.entity(row).add_child(x2);
        }
        commands.entity(page).add_child(row);
    }
    let b = commands.spawn(marker("BlueMarker", -192.04, -520.0)).id();
    add(&mut commands, b);
    // JournalUI's tab buttons: the Buttons overlay (1921.9 x 1081.1 from (0, -0.54)), each
    // centred by its padding and turned (RenderTransform -80°, SKILLS -82°) to run up its tab.
    for (label, size, left, bottom, angle) in [("INFO", 36.0, 156.0, 193.0, -80.0), ("SKILLS", 32.0, 130.0, 36.0, -82.0), ("QUESTS", 34.0, 112.0, -125.0, -80.0)] {
        let box_size = Vec2::new(200.0, 60.0);
        let centre = tab_centre(left, bottom);
        let b = commands
            .spawn((
                at(centre - box_size / 2.0, box_size),
                turn(angle),
                Pickable::IGNORE,
                children![(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, children![text(label.to_string(), size)])],
            ))
            .id();
        add(&mut commands, b);
    }

    // The tech tree (SizeBox_0, turned 5°): lines under the icons.
    let tree_box = commands.spawn((at(tree_origin(), Vec2::new(640.0, TREE_HEIGHT)), turn(5.0), Pickable::IGNORE)).id();
    add(&mut commands, tree_box);
    let slot = Vec2::new(tree.0.slot[0], tree.0.slot[1]);
    let centre = |pos: [f32; 2]| Vec2::new(pos[0], pos[1]) + slot / 2.0;
    for n in &tree.0.nodes {
        for parent in n.parents.iter().filter_map(|p| tree.0.node(p)) {
            let (a, b) = (centre(parent.pos), centre(n.pos));
            let d = b - a;
            let len = d.length();
            let mid = (a + b) / 2.0;
            let line = commands
                .spawn((rel(mid - Vec2::new(len / 2.0, 0.5), Vec2::new(len, 1.0)), BackgroundColor(Color::linear_rgb(0.01, 0.01, 0.01)), UiTransform::from_rotation(Rot2::radians(d.y.atan2(d.x))), Pickable::IGNORE))
                .id();
            commands.entity(tree_box).add_child(line);
        }
    }
    for n in &tree.0.nodes {
        let unlocked = state.tech.has(&n.id);
        let icon = Vec2::splat(100.0);
        let shade = if unlocked { 1.0 } else { 0.1 };
        let tint = kind_colour(n.kind).to_linear();
        let b = commands
            .spawn((
                rel(centre(n.pos) - icon / 2.0, icon),
                Button,
                Hovered::default(),
                TechButton(n.id.clone()),
                ImageNode::new(assets.load(n.icon.clone())).with_color(Color::linear_rgb(tint.red * shade, tint.green * shade, tint.blue * shade)),
            ))
            .id();
        commands.entity(tree_box).add_child(b);
    }
    // Points (the text turned 10°) and RESET POINTS (turned 10°).
    let points = Vec2::new(140.0, 45.0);
    let p0 = Vec2::new(0.0, 540.0) - Vec2::new(-5.95, 2.15) * points;
    let bg = Vec2::new(251.0, 63.0);
    let b = commands.spawn((at(p0 + points / 2.0 - bg / 2.0, bg), img("Skills/SkillPointsBackground"), Pickable::IGNORE)).id();
    add(&mut commands, b);
    let b = commands
        .spawn((at(p0, points), turn(10.0), Pickable::IGNORE, children![(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, children![text(format!("{} POINTS", state.progression.unspent_points), 16.0)])]))
        .id();
    add(&mut commands, b);
    // ButtonWidget_C ResetButton, alignment (-5.3, 1.575), auto-sized (its size is a guess).
    let reset = Vec2::new(150.0, 30.0);
    let b = commands
        .spawn((
            at(Vec2::new(0.0, 540.0) - Vec2::new(-5.3, 1.575) * reset, reset),
            Button,
            Hovered::default(),
            ResetButton,
            turn(10.0),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            children![(
                Node { width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                Pickable::IGNORE,
                children![(Text::new("RESET POINTS"), TextFont { font: font.clone().into(), font_size: pt(12.0), ..default() }, TextColor(Color::WHITE), Pickable::IGNORE)],
            )],
        ))
        .id();
    add(&mut commands, b);
    // The hover tooltip, filled by `tooltip`.
    let b = commands
        .spawn((
            Node { position_type: PositionType::Absolute, flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(10.0)), max_width: Val::Px(320.0), row_gap: Val::Px(4.0), ..default() },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.02, 0.92)),
            Visibility::Hidden,
            GlobalZIndex(45),
            TechTooltip,
            Pickable::IGNORE,
        ))
        .id();
    add(&mut commands, b);
}

/// SkillTreeTooltip: the hovered skill's name (in its type's colour), cost and description.
fn tooltip(
    mut commands: Commands,
    tree: Res<Tree>,
    font: Res<MenuFont>,
    buttons: Query<(&Hovered, &TechButton, &UiGlobalTransform, &ComputedNode)>,
    mut tips: Query<(Entity, &mut Node, &mut Visibility, Option<&Children>), With<TechTooltip>>,
    mut shown: Local<Option<String>>,
) {
    let Ok((tip, mut node, mut vis, kids)) = tips.single_mut() else {
        *shown = None;
        return;
    };
    let hovered = buttons.iter().find(|(h, ..)| h.get());
    let Some((_, b, gt, cn)) = hovered else {
        *vis = Visibility::Hidden;
        *shown = None;
        return;
    };
    let Some(n) = tree.0.node(&b.0) else { return };
    *vis = Visibility::Inherited;
    let scale = cn.inverse_scale_factor();
    let p = gt.translation * scale + Vec2::new(cn.size().x * scale / 2.0 + 8.0, -cn.size().y * scale / 2.0);
    node.left = Val::Px(p.x);
    node.top = Val::Px(p.y);
    if shown.as_deref() == Some(n.id.as_str()) {
        return;
    }
    *shown = Some(n.id.clone());
    for k in kids.into_iter().flatten() {
        commands.entity(*k).despawn();
    }
    let lines = [(n.name.clone(), 18.0, kind_colour(n.kind).lighter(0.25)), (format!("Cost: {} skill point{}", n.cost, if n.cost == 1 { "" } else { "s" }), 12.0, Color::WHITE), (n.tooltip.clone(), 12.0, Color::srgb(0.85, 0.85, 0.85))];
    for (t, size, colour) in lines {
        let l = commands.spawn(crate::menus::widgets::label_colored(&font, t, size, colour)).id();
        commands.entity(tip).add_child(l);
    }
}

fn on_click(
    ev: On<Activate>,
    techs: Query<&TechButton>,
    resets: Query<(), With<ResetButton>>,
    tree: Res<Tree>,
    mut game: ResMut<CurrentGame>,
    mut journal: ResMut<Journal>,
    mut notify: MessageWriter<super::notify::Notify>,
) {
    let Some(state) = game.state.as_mut() else { return };
    if let Ok(b) = techs.get(ev.entity) {
        match state.tech.try_unlock(&tree.0, &b.0, &mut state.progression.unspent_points) {
            Ok(()) => info!("unlocked {}", b.0),
            Err(why) => {
                info!("{}: {why}", b.0);
                notify.write(super::notify::Notify::new(why));
            }
        }
        journal.dirty = true;
    } else if resets.contains(ev.entity) {
        state.tech.reset(&tree.0, &mut state.progression.unspent_points);
        info!("tech tree reset: {} points", state.progression.unspent_points);
        journal.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_sits_where_the_widget_puts_it() {
        // Overlay_0 (2112 x 1188, alignment 0.15 / 0.480263) on the left middle of 1920 x 1080.
        let o = page_origin();
        assert!((o.x + 316.8).abs() < 0.1 && (o.y + 30.55).abs() < 0.1, "{o}");
        // The tree's SizeBox: 640 wide, alignment -0.5 -> 320 px in; 0.175 of 670 above the middle.
        let t = tree_origin();
        assert!((t.x - 320.0).abs() < 0.1 && (t.y - (540.0 - 117.25)).abs() < 0.1, "{t}");
    }

    #[test]
    fn tabs_sit_on_their_bookmarks() {
        // INFO: padding left 156 / bottom 193 from the Buttons overlay's centre (Slate moves a
        // centred child by the whole padding difference), which is where RedMarker's tab is
        // drawn (~1106, 343 once the marker's 6° turn is applied).
        let c = tab_centre(156.0, 193.0);
        assert!((c.x - (960.96 + 156.0)).abs() < 0.1 && (c.y - (540.0 - 193.0)).abs() < 0.1, "{c}");
        assert!(c.distance(Vec2::new(1106.0, 343.0)) < 20.0);
        // Fitness and Reloading sit side by side without overlapping.
        let f = overlay_centred(PAGE, Vec2::new(310.0, 40.0), Vec4::new(0.0, 0.0, 225.0, 500.0));
        let r = overlay_centred(PAGE, Vec2::new(350.0, 40.0), Vec4::new(120.0, 0.0, 0.0, 500.0));
        assert!(f.x + 310.0 <= r.x + 1.0, "{f} {r}");
    }

    #[test]
    fn icons_take_their_type_colour() {
        assert_eq!(kind_colour(1), Color::linear_rgb(0.068966, 0.2, 0.0));
        assert_eq!(kind_colour(3), Color::linear_rgb(0.6, 0.6, 0.6));
    }
}
