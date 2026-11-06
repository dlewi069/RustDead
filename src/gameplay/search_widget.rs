//! The container prompt: BP_LootContainerWidget + SquareProgressBar (HUD_Game.
//! ShowLootContainerWidget, decompiled), shown by `interact::search_containers` for loot
//! containers (BP_LootContainer, the tool-required master, airdrops, the quest crate).
//!
//! - Border_0: 600 x 125 at the screen centre, alignment (0.0525, 0.255), so the 64 x 64 key
//!   square sits on the crosshair; ShowLootContainerWidget hides every crosshair meanwhile
//!   (AllCrosshairsVisibility(Hidden)).
//! - The square: T_SquareTransparent (the grey track) under SquareProgressBar, whose
//!   SquareProgressMaterial shows T_Square's white outline where T_CircularMask (an angle sweep
//!   clockwise from the top centre) is under Alpha = CurrentTime / MaxTime; InteractTxt = the
//!   IA_Interact key ("F"), Montserrat-Bold 12, shadow (2, 2) at 0.25.
//! - Right of it (11 px down): Border_1 (fade-left, rounded) with Txt = ContainerName (14) and
//!   AdditionalTxt (12, #59AA00) = "[Looted]" once searched ("[Storage]" when marked);
//!   ContainerLootTypeTxt (8) = GetLootTableName(LootTable); QuickSearch (8) = "[{key}] Quick
//!   Search", Visible only while the hold is on (IsHeld?), else Collapsed; InteractOptions:
//!   BP_InteractWidgetOption rows (padding 2 above and below) from GetInteractOptions (`options`:
//!   BP_LootContainer's "Search Container" and "Mark As Storage Container", the picklock and
//!   tool-required masters' "Picklock" / "Break Open", the airdrops' and quest crate's "Search
//!   Container"), the first selected at SetupWidget; selected: black 0.6 behind, opacity 1, white
//!   text; the others black 0.2, opacity 0.75, text 0.75 grey. Option2 is disabled until looted
//!   or marked; marked, it reads "Mark As Loot Container" and AdditionalTxt "[Storage]".
//!
//! Ours: the square's sweep is 8 half-side bars cut where the angle meets the outline (the mask's
//! gradient is only nearly linear in angle); IA_QuickSearch's short key name is the designer's
//! default text, "LShift". The wheel picks the option (the Jig interaction component's own
//! input isn't readable; the other prompts use the wheel too), and a disabled option looks like
//! any unselected one. The rest of the containers (infestation piles: ShowInteractWidget in the
//! game) also show this widget with Search Container. Not ported: the BackgroundBlur behind each
//! option, Border_1's rounded corners.

use bevy::prelude::*;

use crate::menus::AppState;
use crate::menus::widgets::{MenuFont, pt};

const TEX: &str = "game/SurrounDead/Content/UI/SquareProgressBar";
/// The overlay's size and T_Square's outline in it (66-99 of 384 px).
const SQUARE: f32 = 64.0;
const INSET: f32 = SQUARE * 66.0 / 384.0;
const THICK: f32 = SQUARE * 34.0 / 384.0;
/// Border_0's alignment x its 600 x 125 size.
const ALIGN: Vec2 = Vec2::new(600.0 * 0.0525, 125.0 * 0.255);
const GREEN: Color = Color::linear_rgb(0.100222, 0.4, 0.0);
/// UMG's default shadow on these texts.
const SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.25);
pub const QUICK_SEARCH_TEXT: &str = "[LShift] Quick Search";
pub const SEARCH: &str = "Search Container";
pub const MARK_STORAGE: &str = "Mark As Storage Container";
pub const MARK_LOOT: &str = "Mark As Loot Container";

/// GetInteractOptions: BP_LootContainer's Search Container and Mark As Storage Container ("Mark
/// As Loot Container" once marked: ShowLootContainerWidget's UpdateLootOptions), the picklock
/// master's Picklock, the tool-required master's Break Open, the airdrops' and the quest crate's
/// (and the rest's, ours) Search Container. `storage`: a BP_LootContainer-family class.
pub fn options(def: &super::items::ContainerDef, storage: bool, marked: bool) -> Vec<&'static str> {
    if def.lockpick {
        vec!["Picklock"]
    } else if def.tool.is_some() {
        vec!["Break Open"]
    } else if storage && def.airdrop.is_none() {
        vec![SEARCH, if marked { MARK_LOOT } else { MARK_STORAGE }]
    } else {
        vec![SEARCH]
    }
}

/// AdditionalTxt: "[Storage]" when marked, else "[Looted]" once searched.
pub fn additional(looted: bool, marked: bool) -> &'static str {
    if marked {
        "[Storage]"
    } else if looted {
        "[Looted]"
    } else {
        ""
    }
}

/// What the widget shows this frame (`interact::search_containers` fills it in).
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct SearchView {
    pub shown: bool,
    /// The container's key (`interact::Containers`).
    pub key: Option<usize>,
    pub name: String,
    pub looted: bool,
    /// MarkedAsStorageContainer.
    pub marked: bool,
    pub loot_type: String,
    /// IsHeld?: the QuickSearch line shows.
    pub held: bool,
    /// CurrentTime / MaxTime.
    pub progress: f32,
    /// GetInteractOptions' labels and the selected one.
    pub options: Vec<String>,
    pub selected: usize,
}

#[derive(Component)]
struct SearchWidget;
#[derive(Component)]
struct Segment(usize);
#[derive(Component)]
enum Field {
    Name,
    Additional,
    LootType,
    Quick,
}
#[derive(Component)]
struct OptionRow(usize);

pub fn plugin(app: &mut App) {
    app.init_resource::<SearchView>()
        .add_systems(OnEnter(AppState::InGame), (|mut v: ResMut<SearchView>| *v = SearchView::default(), spawn))
        .add_systems(Update, draw.after(super::interact::search_containers).run_if(in_state(AppState::InGame)));
}

/// GetLootTableName: the loot type line for a container's Enum_ContainerLootTables value (our
/// table IDs are its display names; the switch is by byte, mapped through the enum's Names).
pub fn loot_type_name(table: Option<&str>) -> &'static str {
    match table.unwrap_or("") {
        "Ammuniton" => "Ammunition",
        "Clothing" => "Clothing",
        "GasPump" => "Gas Pump",
        "FirearmsDisplayCase" => "Firearms Display Case",
        "MilitaryHighTier" => "Military [High Tier]",
        "Industrial" => "Industrial",
        "Keycards" => "Keycards",
        "Laboratory" => "Laboratory",
        "LaboratoryMilitary" => "Laboratory [Military]",
        "Medical" => "Medical",
        "Military" => "Military",
        "Random" => "Random",
        "Residential" => "Residential",
        "ResidentialFoodDrink" => "Residential [Food & Drink]",
        "Shop" => "Shop",
        "ShopFoodDrink" => "Shop [Food & Drink]",
        "CementMixer" => "Cement Mixer",
        "Safe" => "Safe",
        "Axe" => "Fireman's Axe",
        "AnimalMeat" | "AnimalSmallMeat" => "Animal Meat",
        "AirdropMilitary" => "Military Supplies",
        "AirdropMedical" => "Medical Supplies",
        "AirdropSupplies" => "Random Supplies",
        "PrimaryInfestation" => "Infestation [Primary]",
        "Infestation" => "Infestation",
        "Test" => "Test",
        _ => "",
    }
}

/// How much of each half side of the outline is lit at `alpha` (0-1 of a turn clockwise from the
/// top centre): top right, right top, right bottom, bottom right, bottom left, left bottom, left
/// top, top left. The sweep's ray meets a half side running out from a side's middle at tan(phi)
/// of its length, and one running in from a corner at 1 - tan(45 - phi).
pub fn outline_fill(alpha: f32) -> [f32; 8] {
    let deg = alpha.clamp(0.0, 1.0) * 360.0;
    std::array::from_fn(|k| {
        let phi = (deg - 45.0 * k as f32).clamp(0.0, 45.0).to_radians();
        let f = if k % 2 == 0 { phi.tan() } else { 1.0 - (std::f32::consts::FRAC_PI_4 - phi).tan() };
        f.clamp(0.0, 1.0)
    })
}

/// A half side's lit bar in the 64 px square: (left, top, width, height).
pub fn segment_rect(k: usize, f: f32) -> (f32, f32, f32, f32) {
    let (lo, hi, c) = (INSET, SQUARE - INSET, SQUARE / 2.0);
    let len = (c - lo) * f;
    match k {
        0 => (c, lo, len, THICK),
        1 => (hi - THICK, lo, THICK, len),
        2 => (hi - THICK, c, THICK, len),
        3 => (hi - len, hi - THICK, len, THICK),
        4 => (c - len, hi - THICK, len, THICK),
        5 => (lo, hi - len, THICK, len),
        6 => (lo, c - len, THICK, len),
        _ => (lo, lo, len, THICK),
    }
}

/// A Montserrat-Bold TextBlock with UMG's 0.25 black shadow at (2, 2).
fn text(font: &MenuFont, s: &str, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), TextFont { font: font.0.clone().into(), font_size: pt(size), ..default() }, TextColor(color), TextShadow { offset: Vec2::splat(2.0), color: SHADOW })
}

fn spawn(mut commands: Commands, font: Res<MenuFont>, assets: Res<AssetServer>) {
    let square = commands
        .spawn((Node { width: Val::Px(SQUARE), height: Val::Px(SQUARE), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },))
        .with_children(|s| {
            s.spawn((
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                ImageNode::new(assets.load(format!("{TEX}/T_SquareTransparent.png"))).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
            ));
            for k in 0..8 {
                s.spawn((Node { position_type: PositionType::Absolute, ..default() }, BackgroundColor(Color::WHITE), Segment(k)));
            }
            s.spawn(text(&font, "F", 12.0, Color::WHITE));
        })
        .id();
    let option = |i: usize| {
        (
            Node { margin: UiRect::vertical(Val::Px(2.0)), padding: UiRect::right(Val::Px(4.0)), align_self: AlignSelf::FlexStart, ..default() },
            BackgroundColor(Color::NONE),
            OptionRow(i),
            children![text(&font, "", 10.0, Color::WHITE)],
        )
    };
    let column = commands
        .spawn((
            Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexStart, margin: UiRect::top(Val::Px(11.0)), ..default() },
            children![
                (
                    Node { align_items: AlignItems::Center, ..default() },
                    ImageNode::new(assets.load("game/SurrounDead/Content/UI/Textures/fade-left.png")).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
                    children![
                        (text(&font, "", 14.0, Color::WHITE), Node { margin: UiRect::new(Val::Px(2.0), Val::ZERO, Val::Px(2.0), Val::Px(2.0)), ..default() }, Field::Name),
                        (text(&font, "", 12.0, GREEN), Node { margin: UiRect::new(Val::Px(10.0), Val::ZERO, Val::Px(2.0), Val::Px(2.0)), ..default() }, Field::Additional),
                    ],
                ),
                (text(&font, "", 8.0, Color::WHITE), Node { margin: UiRect::left(Val::Px(2.0)), ..default() }, Field::LootType),
                (text(&font, QUICK_SEARCH_TEXT, 8.0, Color::WHITE), Node { margin: UiRect::left(Val::Px(2.0)), display: Display::None, ..default() }, Field::Quick),
                (Node { flex_direction: FlexDirection::Column, margin: UiRect::top(Val::Px(2.0)), ..default() }, children![option(0), option(1)]),
            ],
        ))
        .id();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                margin: UiRect::new(Val::Px(-ALIGN.x), Val::ZERO, Val::Px(-ALIGN.y), Val::ZERO),
                width: Val::Px(600.0),
                height: Val::Px(125.0),
                align_items: AlignItems::FlexStart,
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
            SearchWidget,
            DespawnOnExit(AppState::InGame),
        ))
        .add_children(&[square, column]);
}

#[allow(clippy::type_complexity)]
fn draw(
    view: Res<SearchView>,
    mut root: Query<&mut Visibility, With<SearchWidget>>,
    mut segments: Query<(&Segment, &mut Node), (Without<Field>, Without<OptionRow>)>,
    mut fields: Query<(&Field, &mut Text, &mut Node), (Without<Segment>, Without<OptionRow>)>,
    mut options: Query<(&OptionRow, &mut BackgroundColor, &mut Node, &Children), (Without<Segment>, Without<Field>)>,
    mut labels: Query<(&mut TextColor, &mut Text), (Without<Field>, Without<OptionRow>, Without<Segment>)>,
    mut crosshair: Query<&mut ImageNode, With<super::hud::Crosshair>>,
) {
    // AllCrosshairsVisibility(Hidden) while it's up.
    for mut img in &mut crosshair {
        let a = if view.shown { 0.0 } else { 1.0 };
        if img.color.alpha() != a {
            img.color.set_alpha(a);
        }
    }
    let Ok(mut vis) = root.single_mut() else { return };
    vis.set_if_neq(if view.shown { Visibility::Inherited } else { Visibility::Hidden });
    if !view.shown || !view.is_changed() {
        return;
    }
    let fill = outline_fill(view.progress);
    for (s, mut n) in &mut segments {
        let (l, t, w, h) = segment_rect(s.0, fill[s.0]);
        n.left = Val::Px(l);
        n.top = Val::Px(t);
        n.width = Val::Px(w);
        n.height = Val::Px(h);
    }
    for (f, mut t, mut n) in &mut fields {
        let s = match f {
            Field::Name => view.name.clone(),
            Field::Additional => additional(view.looted, view.marked).into(),
            Field::LootType => view.loot_type.clone(),
            Field::Quick => {
                n.display = if view.held { Display::Flex } else { Display::None };
                continue;
            }
        };
        if t.0 != s {
            t.0 = s;
        }
    }
    // SetSelected: the selected row black 0.6, opacity 1, white text; the others black 0.2,
    // opacity 0.75, text 0.75 grey at 0.75. Ours: Option2's SetIsEnabled(false) until looted or
    // marked isn't drawn any differently (Slate's disabled effect).
    for (o, mut bg, mut node, kids) in &mut options {
        let Some(label) = view.options.get(o.0) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let selected = o.0 == view.selected;
        bg.0 = Color::BLACK.with_alpha(if selected { 0.6 } else { 0.2 * 0.75 });
        let c = if selected { Color::WHITE } else { Color::srgba(0.75, 0.75, 0.75, 0.75 * 0.75) };
        for &k in kids {
            if let Ok((mut tc, mut t)) = labels.get_mut(k) {
                tc.0 = c;
                if t.0 != *label {
                    t.0 = label.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_outline_fills_clockwise_from_the_top() {
        assert_eq!(outline_fill(0.0), [0.0; 8]);
        assert!(outline_fill(1.0).iter().all(|&f| (f - 1.0).abs() < 1e-5));
        // An eighth of a turn reaches the top right corner; a quarter, the right side's middle.
        let e = outline_fill(0.125);
        assert!((e[0] - 1.0).abs() < 1e-5 && e[1] == 0.0);
        let q = outline_fill(0.25);
        assert!(q[..2].iter().all(|&f| (f - 1.0).abs() < 1e-5) && q[2] == 0.0);
        // 22.5 degrees in: tan(22.5) of the first half side.
        assert!((outline_fill(0.0625)[0] - 22.5f32.to_radians().tan()).abs() < 1e-5);
        // Half way: the bottom's middle.
        let h = outline_fill(0.5);
        assert!(h[..4].iter().all(|&f| (f - 1.0).abs() < 1e-5) && h[4..].iter().all(|&f| f == 0.0));
    }

    #[test]
    fn segments_trace_the_outline() {
        // Full bars meet at the corners and the sides' middles.
        assert_eq!(segment_rect(0, 1.0).0 + segment_rect(0, 1.0).2, SQUARE - INSET);
        let (l, t, w, h) = segment_rect(3, 1.0);
        assert_eq!((l, t + h, l + w), (SQUARE / 2.0, SQUARE - INSET, SQUARE - INSET));
        // Corner-to-middle bars grow from the corner.
        assert_eq!(segment_rect(5, 0.5).1 + segment_rect(5, 0.5).3, SQUARE - INSET);
        assert_eq!(segment_rect(7, 0.25).0, INSET);
    }

    #[test]
    fn loot_types_follow_get_loot_table_name() {
        assert_eq!(loot_type_name(Some("Ammuniton")), "Ammunition");
        assert_eq!(loot_type_name(Some("ResidentialFoodDrink")), "Residential [Food & Drink]");
        assert_eq!(loot_type_name(Some("AnimalSmallMeat")), "Animal Meat");
        assert_eq!(loot_type_name(Some("AirdropMedical")), "Medical Supplies");
        assert_eq!(loot_type_name(Some("Nothing")), "");
        assert_eq!(loot_type_name(None), "");
        // Every container table in items.json has a name (Nothing excepted).
        let items: serde_json::Value = serde_json::from_str(&std::fs::read_to_string("assets/data/items.json").unwrap()).unwrap();
        for (class, c) in items["containers"].as_object().unwrap() {
            if let Some(t) = c["table"].as_str().filter(|t| *t != "Nothing") {
                assert!(!loot_type_name(Some(t)).is_empty(), "{class}: {t}");
            }
        }
    }

    #[test]
    fn each_master_has_its_own_options() {
        let db = super::super::items::ItemDb::load();
        let def = |c: &str| db.containers.get(c).cloned().unwrap_or_else(|| panic!("{c}"));
        let any = |f: &dyn Fn(&super::super::items::ContainerDef) -> bool| db.containers.values().find(|d| f(d)).cloned().unwrap();
        let plain = any(&|d| !d.lockpick && d.tool.is_none() && d.airdrop.is_none());
        assert_eq!(options(&plain, true, false), vec![SEARCH, MARK_STORAGE]);
        assert_eq!(options(&plain, true, true), vec![SEARCH, MARK_LOOT]);
        // The quest crate (not a BP_LootContainer): Search Container only.
        assert_eq!(options(&plain, false, false), vec![SEARCH]);
        assert_eq!(options(&any(&|d| d.lockpick), true, false), vec!["Picklock"]);
        assert_eq!(options(&any(&|d| d.tool.is_some()), true, false), vec!["Break Open"]);
        assert_eq!(options(&def("AirdropContainer_Medical_C"), true, false), vec![SEARCH]);
    }

    #[test]
    fn storage_reads_over_looted() {
        assert_eq!(additional(false, false), "");
        assert_eq!(additional(true, false), "[Looted]");
        assert_eq!(additional(true, true), "[Storage]");
    }
}
