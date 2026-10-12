//! The inventory screen, rebuilt from `Inventory/BP_Inventory` (JigSInventory):
//!
//! ```text
//!  ┌ 675 ─────────────┐┌ 620 ──────────────┐┌ 620 ──────────────┐
//!  │ loot / vicinity  ││ equipment slots   ││ INVENTORY         │
//!  │ grids            ││ (silhouettes)     ││ pockets 4x2       │
//!  │                  ││                   ││ worn containers   │
//!  │ VICINITY | LOOT  ││ EQUIPMENT|CRAFTING││                   │
//!  └──────────────────┘└───────────────────┘└───────────────────┘
//! ```
//!
//! Items are `JSI_Slot`s: rarity-tinted 50 px cells (`InventoryBorder` under each cell), the baked
//! inspect picture, name top-left, count top-right. Drag and drop moves items between grids,
//! equipment slots and the ground; R while dragging turns the item (JSIContainer `RotateKey`),
//! Shift on drop opens `JigSplitWidget` (`SplitKey`); Ctrl+click quick-moves (`QUICK MOVE ITEM`);
//! right click opens the context menu (equip / use / unequip / drop); hovering shows
//! `OnHoverTooltipWidget`. The CRAFTING tab is `W_PlayerCraftingUI` over the "Player" recipes.

use bevy::picking::Pickable;
use bevy::picking::events::{PointerClick, PointerDragDrop, PointerDragEnd, PointerDragStart};
use bevy::picking::hover::Hovered;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::equipment::{GridRef, PlayerInventory, Slot, WORN_GRIDS};
use super::icons::ItemIcons;
use super::interact::{Containers, GameOverlay, insert_growing, player_inv};
use super::items::{Grid, ItemDb, Placed, Stack, footprint};
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame};
use crate::models::{CharacterModel, model};

const CELL: f32 = 50.0;
const PANEL: Color = Color::linear_rgba(0.01, 0.01, 0.01, 0.75);
const TEXT: Color = Color::linear_rgba(0.75, 0.75, 0.75, 1.0);
/// `SlotOnHoverColor`.
const HOVER: Color = Color::linear_rgba(0.313432, 0.599552, 0.697917, 0.5);
/// Outline of the chosen recipe (REQUIRED ITEMS title yellow).
const SELECTED: Color = Color::linear_rgb(1.0, 0.718804, 0.0);
const VICINITY: f32 = 3.0;
const INV_TEX: &str = "game/SurrounDead/Content/Inventory";

/// Rebuild the inventory screen next frame (contents changed).
#[derive(Resource, Default)]
pub struct Dirty(pub bool);

/// Where an item lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Loc {
    Player(GridRef),
    Loot(usize),
    Vicinity,
    Equip(Slot),
    /// CRAFTABLE ITEMS (products of the "Player" recipes; select only).
    Craft,
    /// REQUIRED ITEMS of the selected recipe (display only).
    Required,
}

impl Loc {
    fn display_only(self) -> bool {
        matches!(self, Loc::Craft | Loc::Required)
    }
}

#[derive(Component, Clone, Copy, Debug)]
struct ItemRef {
    loc: Loc,
    index: usize,
}

#[derive(Component, Clone, Copy)]
struct CellRef {
    loc: Loc,
    x: u8,
    y: u8,
}

#[derive(Component, Clone, Copy)]
struct SlotWidget(Slot);

/// Dropping here (outside the panels) drops the item on the ground (`DropItemBackGwidget`).
#[derive(Component)]
struct DropZone;

/// Panels swallow drops that miss a cell.
#[derive(Component)]
struct Panel;

#[derive(Component)]
struct InvRoot;

/// The dragged item's picture; `base` is its upright size in px.
#[derive(Component)]
struct Ghost {
    base: Vec2,
}

#[derive(Component)]
struct GhostImage;

#[derive(Component)]
struct Tooltip;

#[derive(Component, Clone, Copy)]
enum LootTab {
    Vicinity,
    Loot,
}

#[derive(Component, Clone, Copy)]
struct MenuOption(Action, ItemRef);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Equip,
    Unequip,
    Use,
    Drop,
    /// `Jig.Context.Unload`: a weapon's magazine out, or a magazine's rounds out.
    Unload,
}

#[derive(Component)]
struct ContextMenu;

/// EQUIPMENT | CRAFTING tabs under the middle column.
#[derive(Component, Clone, Copy)]
struct EquipTab(bool);

/// `W_PlayerCraftingUI` buttons: Button_Decrease, Button_81 (craft), Button_Increase.
#[derive(Component, Clone, Copy)]
enum CraftButton {
    Less,
    Craft,
    More,
}

/// `JigSplitWidget` pieces.
#[derive(Component, Clone, Copy)]
enum SplitButton {
    Cancel,
    Confirm,
}

#[derive(Component)]
struct SplitTrack;

#[derive(Component)]
struct SplitFill;

#[derive(Component)]
struct SplitCount;

/// A pending Shift+drop split.
#[derive(Clone, Copy)]
struct SplitDialog {
    src: ItemRef,
    cell: CellRef,
    rotated: bool,
    /// Stack size; the slider picks 1..max-1.
    max: u32,
    value: u32,
}

/// The CRAFTING tab's grids as last built, for tooltips and selection.
#[derive(Resource, Default)]
struct CraftView {
    craft: Grid,
    /// Craft grid item index -> index into the "Player" recipes.
    recipe_of: Vec<usize>,
    required: Grid,
}

/// Items lying on the ground (dropped by the player). World models are spawned/despawned by
/// [`sync_ground_models`].
#[derive(Resource, Default)]
struct Ground {
    items: Vec<(Vec3, Stack, Option<Entity>)>,
    despawn: Vec<Entity>,
}

/// The vicinity grid's item order -> index into [`Ground::items`].
#[derive(Resource, Default)]
struct VicinityView(Vec<usize>);

#[derive(Resource, Default)]
struct UiState {
    /// Showing vicinity instead of the searched container.
    vicinity: bool,
    dragging: Option<ItemRef>,
    /// The dragged item is turned (toggled with R).
    drag_rotated: bool,
    /// CRAFTING tab instead of EQUIPMENT.
    crafting: bool,
    /// Selected "Player" recipe and how many times to craft it.
    recipe: Option<usize>,
    craft_times: u32,
    split: Option<SplitDialog>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Dirty>()
        .init_resource::<Ground>()
        .init_resource::<VicinityView>()
        .init_resource::<UiState>()
        .init_resource::<CraftView>()
        .add_systems(OnEnter(AppState::InGame), |mut g: ResMut<Ground>, mut s: ResMut<UiState>| {
            g.items.clear();
            *s = UiState::default();
        })
        .add_systems(Update, (rebuild, hover_and_tooltip, rotate_ghost, follow_cursor, split_slider, sync_ground_models).chain().run_if(in_state(AppState::InGame)))
        .add_observer(on_drag_start)
        .add_observer(on_drag_end)
        .add_observer(on_drop)
        .add_observer(on_click);
}

/// Screenshot tour hooks (`menus/autoshot.rs`): which inventory page to show.
pub(crate) enum TourPage {
    Crafting,
    /// Split dialog over the first pocket stack.
    Split,
    Equipment,
}

pub(crate) fn tour_page(world: &mut World, page: TourPage) {
    let mut ui = world.resource_mut::<UiState>();
    ui.split = None;
    match page {
        TourPage::Crafting => {
            ui.crafting = true;
            ui.recipe = Some(0);
            ui.craft_times = 1;
        }
        TourPage::Equipment => ui.crafting = false,
        TourPage::Split => {
            let count = world.resource_mut::<CurrentGame>().inventory.as_ref().and_then(|i| i.pockets.items.first()).map_or(0, |p| p.stack.count);
            let src = ItemRef { loc: Loc::Player(GridRef::Pockets), index: 0 };
            world.resource_mut::<UiState>().split = Some(SplitDialog { src, cell: CellRef { loc: Loc::Player(GridRef::Pockets), x: 3, y: 1 }, rotated: false, max: count, value: count / 2 });
        }
    }
    world.resource_mut::<Dirty>().0 = true;
}

// ---------------------------------------------------------------- moving items around

struct Ctx<'a> {
    db: &'a ItemDb,
    game: &'a mut CurrentGame,
    containers: &'a mut Containers,
    ground: &'a mut Ground,
    view: &'a VicinityView,
    craft: &'a CraftView,
    player: Vec3,
}

impl Ctx<'_> {
    fn inv(&mut self) -> &mut PlayerInventory {
        player_inv(self.game)
    }

    fn grid(&mut self, loc: Loc) -> Option<&mut Grid> {
        match loc {
            Loc::Player(r) => self.inv().grid_mut(r),
            Loc::Loot(a) => self.containers.searched.get_mut(&a),
            _ => None,
        }
    }

    /// A copy of an item and its grid placement, without moving it.
    fn peek(&mut self, r: ItemRef) -> Option<(Stack, Option<Placed>)> {
        let placed = |g: &Grid| g.items.get(r.index).map(|p| (p.stack.clone(), Some(p.clone())));
        match r.loc {
            Loc::Equip(slot) => self.inv().equipped.get(&slot).map(|s| (s.clone(), None)),
            Loc::Vicinity => self.view.0.get(r.index).and_then(|&i| self.ground.items.get(i)).map(|g| (g.1.clone(), None)),
            Loc::Craft => placed(&self.craft.craft),
            Loc::Required => placed(&self.craft.required),
            loc => self.grid(loc).and_then(|g| placed(g)),
        }
    }

    /// Remove an item, returning it with its grid placement (for exact restore).
    fn take(&mut self, r: ItemRef) -> Option<(Stack, Option<Placed>)> {
        match r.loc {
            Loc::Equip(slot) => self.inv().equipped.remove(&slot).map(|s| (s, None)),
            Loc::Vicinity => {
                let gi = *self.view.0.get(r.index)?;
                if gi >= self.ground.items.len() {
                    return None;
                }
                let (_, stack, e) = self.ground.items.remove(gi);
                self.ground.despawn.extend(e);
                Some((stack, None))
            }
            loc => {
                let g = self.grid(loc)?;
                (r.index < g.items.len()).then(|| {
                    let p = g.items.remove(r.index);
                    (p.stack.clone(), Some(p))
                })
            }
        }
    }

    /// Put a taken item back exactly where it was.
    fn restore(&mut self, r: ItemRef, stack: Stack, placed: Option<Placed>) {
        match (r.loc, placed) {
            (Loc::Equip(slot), _) => {
                self.inv().equip(slot, stack);
            }
            (loc @ (Loc::Player(_) | Loc::Loot(_)), Some(mut p)) => {
                p.stack = stack;
                if let Some(g) = self.grid(loc) {
                    let at = r.index.min(g.items.len());
                    g.items.insert(at, p);
                }
            }
            _ => self.drop_on_ground(stack),
        }
    }

    fn drop_on_ground(&mut self, stack: Stack) {
        let n = self.ground.items.len() as f32;
        let pos = self.player + Vec3::new((n * 1.7).sin() * 0.6, 0.05, (n * 1.7).cos() * 0.6 + 0.4);
        self.ground.items.push((pos, stack, None));
    }

    /// Anywhere in the player's inventory; what doesn't fit lands on the ground.
    fn to_player(&mut self, stack: Stack) {
        let db = self.db;
        if let Some(rest) = self.inv().insert(db, stack) {
            self.drop_on_ground(rest);
        }
    }

    fn equip(&mut self, slot: Slot, stack: Stack) {
        if let Some(prev) = self.inv().equip(slot, stack) {
            self.to_player(prev);
        }
    }

    /// Ctrl+click: inventory <-> open container; loot/ground -> inventory; equipped -> inventory.
    fn quick_move(&mut self, r: ItemRef, loot_open: Option<usize>) {
        let Some((stack, placed)) = self.take(r) else { return };
        match r.loc {
            Loc::Player(_) => match loot_open {
                Some(a) => {
                    let db = self.db;
                    if let Some(g) = self.containers.searched.get_mut(&a) {
                        insert_growing(db, g, stack);
                    }
                }
                None => self.restore(r, stack, placed),
            },
            _ => self.to_player(stack),
        }
    }

    fn drop_onto_cell(&mut self, r: ItemRef, cell: CellRef, rotated: bool) {
        let Some((stack, placed)) = self.take(r) else { return };
        if let Err(stack) = self.place_centred(stack, cell, rotated) {
            self.restore(r, stack, placed);
        }
    }

    /// Place with the item centred on the cell under the cursor (the ghost is drawn centred).
    fn place_centred(&mut self, stack: Stack, cell: CellRef, rotated: bool) -> Result<(), Stack> {
        if cell.loc == Loc::Vicinity {
            self.drop_on_ground(stack);
            return Ok(());
        }
        let db = self.db;
        let (w, h) = footprint(db, &stack.item, rotated).unwrap_or((1, 1));
        match self.grid(cell.loc) {
            Some(g) => {
                // Pulled back inside the grid when the cursor is near its edge.
                let x = cell.x.saturating_sub((w - 1) / 2).min(g.width.saturating_sub(w));
                let y = cell.y.saturating_sub((h - 1) / 2).min(g.height.saturating_sub(h));
                g.place_rotated(db, stack, x, y, rotated)
            }
            None => Err(stack),
        }
    }

    /// `JigSplitWidget` confirm: move `n` off the stack to `cell` (onto a matching stack there
    /// if the cell is taken). Whatever can't go there stays on the source stack.
    fn split(&mut self, r: ItemRef, cell: CellRef, n: u32, rotated: bool) {
        let Some((mut stack, placed)) = self.take(r) else { return };
        let n = n.min(stack.count);
        if n == 0 || n == stack.count {
            self.restore(r, stack, placed);
            if n > 0 {
                self.drop_onto_cell(r, cell, rotated);
            }
            return;
        }
        let part = Stack { item: stack.item.clone(), count: n, ..default() };
        stack.count -= n;
        self.restore(r, stack, placed);
        let db = self.db;
        let Err(part) = self.place_centred(part, cell, rotated) else { return };
        // Onto an occupied cell: stack with what's there, else back where it came from.
        let rest = match self.grid(cell.loc) {
            Some(g) => match g.at(cell.x, cell.y) {
                Some(i) => g.merge_into(db, i, part),
                None => Some(part),
            },
            None => Some(part),
        };
        if let Some(rest) = rest {
            let back = self.grid(r.loc).and_then(|g| g.merge_into(db, r.index, rest));
            if let Some(rest) = back {
                self.drop_on_ground(rest);
            }
        }
    }

    /// Craft the selected "Player" recipe `times` times; products go to the inventory.
    fn craft(&mut self, recipe: usize, times: u32) -> bool {
        let db = self.db;
        let Some(recipe) = db.recipes.get("Player").and_then(|r| r.get(recipe)) else { return false };
        match self.inv().craft(db, recipe, times) {
            Some(made) => {
                self.to_player(made);
                true
            }
            None => false,
        }
    }

    /// The stack an item reference points at, to change in place.
    fn stack_mut(&mut self, r: ItemRef) -> Option<&mut Stack> {
        match r.loc {
            Loc::Equip(slot) => self.inv().equipped.get_mut(&slot),
            loc => self.grid(loc)?.items.get_mut(r.index).map(|p| &mut p.stack),
        }
    }

    /// JigSInventory item-over-item: rounds onto a magazine load it, a magazine onto its
    /// firearm attaches it. Gives back what's left (or everything when neither applies).
    fn item_over_item(db: &ItemDb, stack: Stack, target: &mut Stack) -> Result<Option<Stack>, Stack> {
        let Some(tdef) = db.items.get(&target.item) else { return Err(stack) };
        if tdef.capacity > 0 && tdef.ammo.contains(&stack.item) {
            let n = tdef.capacity.saturating_sub(target.rounds).min(stack.count);
            if n == 0 {
                return Err(stack);
            }
            target.rounds += n;
            let left = stack.count - n;
            return Ok((left > 0).then(|| Stack { count: left, ..stack }));
        }
        let fits_as_mag = tdef.attachments.iter().any(|a| a.kind == "Mag" && a.item == stack.item);
        if fits_as_mag && target.mag(db).is_none() && stack.count == 1 {
            target.attachments.push(stack);
            return Ok(None);
        }
        Err(stack)
    }

    fn drop_onto_slot(&mut self, r: ItemRef, slot: Slot) {
        let Some((stack, placed)) = self.take(r) else { return };
        // A magazine or rounds onto the equipped firearm.
        let db = self.db;
        if let Some(target) = self.inv().equipped.get_mut(&slot) {
            match Self::item_over_item(db, stack, target) {
                Ok(None) => return,
                Ok(Some(rest)) => return self.restore(r, rest, placed),
                Err(s) if !PlayerInventory::can_equip(db, slot, &s.item) => return self.restore(r, s, placed),
                Err(s) => return self.equip(slot, s),
            }
        }
        if PlayerInventory::can_equip(self.db, slot, &stack.item) {
            self.equip(slot, stack);
        } else {
            self.restore(r, stack, placed);
        }
    }

    /// `cell` is the grid cell under the cursor inside the target item, when known: an item
    /// dropped onto itself (after turning it with R) is re-placed there.
    fn drop_onto_item(&mut self, r: ItemRef, target: ItemRef, cell: Option<CellRef>, rotated: bool) {
        if let Loc::Equip(slot) = target.loc {
            self.drop_onto_slot(r, slot);
            return;
        }
        if r.loc == target.loc && r.index == target.index {
            if let Some(cell) = cell {
                self.drop_onto_cell(r, cell, rotated);
            }
            return;
        }
        let Some((stack, placed)) = self.take(r) else { return };
        // Same grid, earlier index: the target shifted down by one when we removed ours.
        let mut ti = target.index;
        if r.loc == target.loc && r.index < target.index {
            ti -= 1;
        }
        let db = self.db;
        let rest = match self.stack_mut(ItemRef { loc: target.loc, index: ti }).map(|t| Self::item_over_item(db, stack.clone(), t)) {
            Some(Ok(rest)) => rest,
            _ => match self.grid(target.loc) {
                Some(g) => g.merge_into(db, ti, stack),
                None => Some(stack),
            },
        };
        if let Some(rest) = rest {
            self.restore(r, rest, placed);
        }
    }

    fn act(&mut self, action: Action, r: ItemRef) {
        match action {
            Action::Unload => {
                let db = self.db;
                let Some(stack) = self.stack_mut(r) else { return };
                let out = if let Some(i) = stack.attachments.iter().position(|a| db.items.get(&a.item).is_some_and(|d| d.capacity > 0)) {
                    Some(stack.attachments.remove(i))
                } else {
                    let ammo = db.items.get(&stack.item).and_then(|d| d.ammo.first().cloned());
                    match ammo {
                        Some(ammo) if stack.rounds > 0 => Some(db.stack(&ammo, std::mem::take(&mut stack.rounds))),
                        _ => None,
                    }
                };
                if let Some(out) = out {
                    self.to_player(out);
                }
            }
            Action::Drop => {
                if let Some((s, _)) = self.take(r) {
                    self.drop_on_ground(s);
                }
            }
            Action::Unequip => {
                if let Some((s, _)) = self.take(r) {
                    self.to_player(s);
                }
            }
            Action::Equip => {
                let Some((stack, placed)) = self.take(r) else { return };
                let slot = player_inv(self.game).slot_for(self.db, &stack.item);
                match slot {
                    Some(slot) => self.equip(slot, stack),
                    None => self.restore(r, stack, placed),
                }
            }
            Action::Use => {
                let Some((mut stack, placed)) = self.take(r) else { return };
                if let (Some(def), Some(state)) = (self.db.items.get(&stack.item), self.game.state.as_mut()) {
                    super::medical::consume(&mut state.survival, &mut state.effects, def);
                    stack.count -= 1;
                }
                if stack.count > 0 {
                    self.restore(r, stack, placed);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- building the screen

fn title(font: &MenuFont, text: &str, size: f32, extra: impl Bundle) -> impl Bundle {
    title_in(font, text, size, Color::WHITE, extra)
}

/// `EquipmentSlotTitle` (black 0.70 back, Montserrat, `Color`).
fn title_in(font: &MenuFont, text: &str, size: f32, colour: Color, extra: impl Bundle) -> impl Bundle {
    (
        Node { width: Val::Percent(100.0), flex_shrink: 0.0, padding: UiRect::all(Val::Px(5.0)), ..default() },
        BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.0, 0.701961)),
        extra,
        children![(label_colored(font, text, size, colour), Pickable::IGNORE)],
    )
}

fn rarity_colour(rarity: &str) -> Color {
    let c = |r, g, b| Color::linear_rgba(r, g, b, 0.8);
    match rarity {
        "Hunger" => c(0.033408, 0.15, 0.024912),
        "Thirst" => c(0.022043, 0.087291, 0.2),
        "Medical" => c(0.2, 0.006294, 0.006294),
        "Energy" => c(0.4, 0.149538, 0.010892),
        "Rare" => c(0.051545, 0.017989, 0.15),
        "Legendary" => c(0.5, 0.401538, 0.009552),
        "Named" => c(0.75, 0.49022, 0.0),
        _ => c(0.25, 0.25, 0.25),
    }
}

struct Build<'a> {
    font: &'a MenuFont,
    db: &'a ItemDb,
    icons: &'a mut ItemIcons,
    images: &'a mut Assets<Image>,
    assets: &'a AssetServer,
    /// REQUIRED ITEMS the survivor doesn't carry enough of (drawn red).
    missing: Vec<String>,
}

impl Build<'_> {
    /// `JSI_Slot`: picture, name top-left, count top-right over the rarity colour. A rotated
    /// item's picture (baked upright) is turned 90° inside its sideways footprint.
    fn item(&mut self, c: &mut Commands, stack: &Stack, r: ItemRef, node: Node, rotated: bool) -> Entity {
        let def = self.db.items.get(&stack.item);
        // Equipped slots show only the picture under the slot's own label.
        let name = if matches!(r.loc, Loc::Equip(_)) { "" } else { def.map_or(stack.item.as_str(), |d| d.name.as_str()) };
        // Shortfalls in REQUIRED ITEMS are tinted red (our cue; the original only greys the button).
        let bg = if r.loc == Loc::Required && self.missing.contains(&stack.item) {
            Color::linear_rgba(0.35, 0.0, 0.0, 0.8)
        } else {
            rarity_colour(def.map_or("", |d| d.rarity.as_str()))
        };
        let icon = self.icons.get(&stack.item, self.db, self.images);
        let picture = match (rotated, node.width, node.height) {
            (true, Val::Px(w), Val::Px(h)) => rotated_picture(w, h),
            _ => (Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, UiTransform::default()),
        };
        c.spawn((
            node,
            BackgroundColor(bg),
            Hovered::default(),
            r,
            children![
                (picture, icon.map(ImageNode::new).unwrap_or_default(), Pickable::IGNORE),
                (label_colored(self.font, name, 7.0, Color::WHITE), Node { position_type: PositionType::Absolute, left: Val::Px(3.0), top: Val::Px(2.0), max_width: Val::Percent(95.0), ..default() }, Pickable::IGNORE),
                (
                    label_colored(self.font, if stack.count > 1 { stack.count.to_string() } else { String::new() }, 8.0, Color::WHITE),
                    Node { position_type: PositionType::Absolute, right: Val::Px(3.0), bottom: Val::Px(2.0), ..default() },
                    Pickable::IGNORE,
                ),
            ],
        ))
        .id()
    }

    fn grid(&mut self, c: &mut Commands, grid: &Grid, loc: Loc) -> Entity {
        let area = c.spawn(Node { width: Val::Px(grid.width as f32 * CELL), height: Val::Px(grid.height as f32 * CELL), flex_shrink: 0.0, ..default() }).id();
        let border: Handle<Image> = self.assets.load(format!("{INV_TEX}/Widgets/InventoryBorder.png"));
        for y in 0..grid.height {
            for x in 0..grid.width {
                let cell = c
                    .spawn((
                        Node { position_type: PositionType::Absolute, left: Val::Px(x as f32 * CELL), top: Val::Px(y as f32 * CELL), width: Val::Px(CELL), height: Val::Px(CELL), ..default() },
                        ImageNode::new(border.clone()),
                        CellRef { loc, x, y },
                    ))
                    .id();
                c.entity(area).add_child(cell);
            }
        }
        for (index, p) in grid.items.iter().enumerate() {
            let node = Node {
                position_type: PositionType::Absolute,
                left: Val::Px(p.x as f32 * CELL + 1.0),
                top: Val::Px(p.y as f32 * CELL + 1.0),
                width: Val::Px(p.w as f32 * CELL - 2.0),
                height: Val::Px(p.h as f32 * CELL - 2.0),
                overflow: Overflow::clip(),
                ..default()
            };
            let e = self.item(c, &p.stack, ItemRef { loc, index }, node, p.rotated);
            c.entity(area).add_child(e);
        }
        area
    }

    /// An equipment slot: label, silhouette, or the equipped item's picture.
    fn slot(&mut self, c: &mut Commands, slot: Slot, equipped: Option<&Stack>) -> Entity {
        let (w, h) = slot.size();
        let e = c
            .spawn((
                Node { width: Val::Px(w), height: Val::Px(h), margin: UiRect::all(Val::Px(6.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.0, 0.35)),
                SlotWidget(slot),
            ))
            .id();
        match equipped {
            Some(stack) => {
                let node = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
                let item = self.item(c, stack, ItemRef { loc: Loc::Equip(slot), index: 0 }, node, false);
                c.entity(e).add_child(item);
            }
            None => {
                let img = c
                    .spawn((
                        Node { width: Val::Percent(70.0), height: Val::Percent(70.0), ..default() },
                        ImageNode::new(self.assets.load(format!("{INV_TEX}/Textures/{}.png", slot.texture()))).with_color(Color::srgba(1.0, 1.0, 1.0, 0.18)),
                        Pickable::IGNORE,
                    ))
                    .id();
                c.entity(e).add_child(img);
            }
        }
        if let Some(l) = slot.label() {
            let t = c
                .spawn((label_colored(self.font, l, 9.0, TEXT), Node { position_type: PositionType::Absolute, left: Val::Px(4.0), top: Val::Px(3.0), ..default() }, Pickable::IGNORE))
                .id();
            c.entity(e).add_child(t);
        }
        e
    }
}

/// An upright picture turned to fill a `w` x `h` (sideways) box.
fn rotated_picture(w: f32, h: f32) -> (Node, UiTransform) {
    (
        Node { position_type: PositionType::Absolute, width: Val::Px(h), height: Val::Px(w), left: Val::Px((w - h) / 2.0), top: Val::Px((h - w) / 2.0), ..default() },
        UiTransform { rotation: Rot2::degrees(90.0), ..default() },
    )
}

fn column(c: &mut Commands, width: f32) -> (Entity, Entity) {
    let col = c.spawn((Node { width: Val::Px(width), height: Val::Px(960.0), flex_direction: FlexDirection::Column, ..default() }, Panel)).id();
    let content = c
        .spawn((
            Node {
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(10.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
            BackgroundColor(PANEL),
        ))
        .id();
    (col, content)
}

#[allow(clippy::too_many_arguments)]
fn rebuild(
    mut commands: Commands,
    overlay: Res<GameOverlay>,
    roots: Query<Entity, With<InvRoot>>,
    font: Res<MenuFont>,
    mut data: InvData,
    mut ui: ResMut<UiState>,
    mut icons: ResMut<ItemIcons>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
) {
    let InvData { db, game, containers, ground, view, dirty, craft } = &mut data;
    let db: &ItemDb = db;
    if !overlay.is_open() {
        ui.split = None;
    }
    if overlay.is_changed() {
        dirty.0 = true;
        if let GameOverlay::Loot(_) = *overlay {
            ui.vicinity = false;
        }
    }
    if !dirty.0 {
        return;
    }
    dirty.0 = false;
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !overlay.shows_inventory() {
        return;
    }
    let c = &mut commands;
    let mut b = Build { font: &font, db, icons: &mut icons, images: &mut images, assets: &assets, missing: Vec::new() };
    let root = c
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            GlobalZIndex(2),
            InvRoot,
            DropZone,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let row = c.spawn(Node { margin: UiRect::top(Val::Px(2.5)), ..default() }).id();
    c.entity(root).add_child(row);

    // --- loot / vicinity
    let (loot_col, loot) = column(c, 675.0);
    let loot_actor = match *overlay {
        GameOverlay::Loot(a) => Some(a),
        _ => None,
    };
    let show_vicinity = ui.vicinity || loot_actor.is_none();
    if show_vicinity {
        let here = player.single().map(|t| t.translation).unwrap_or_default();
        let mut g = Grid::new("Vicinity", super::interact::LOOT_COLUMNS, 6);
        view.0.clear();
        for (i, (pos, stack, _)) in ground.items.iter().enumerate() {
            if pos.distance(here) > VICINITY {
                continue;
            }
            let (w, h) = db.items.get(&stack.item).map_or((1, 1), |d| (d.size[0].max(1), d.size[1].max(1)));
            loop {
                let spot = (0..g.height).flat_map(|y| (0..g.width).map(move |x| (x, y))).find(|&(x, y)| g.free_ignoring(x, y, w, h, None));
                match spot {
                    Some((x, y)) => {
                        g.items.push(Placed { stack: stack.clone(), x, y, w, h, rotated: false });
                        view.0.push(i);
                        break;
                    }
                    None => g.height += 1,
                }
            }
        }
        let t = c.spawn(title(&font, "VICINITY", 15.0, ())).id();
        let grid = b.grid(c, &g, Loc::Vicinity);
        c.entity(loot).add_children(&[t, grid]);
    } else if let Some(a) = loot_actor
        && let Some(g) = containers.searched.get(&a)
    {
        let t = c.spawn(title(&font, &g.name.to_uppercase(), 15.0, ())).id();
        let grid = b.grid(c, g, Loc::Loot(a));
        c.entity(loot).add_children(&[t, grid]);
    }
    let tabs = c.spawn(Node { width: Val::Percent(100.0), ..default() }).id();
    let vt = c.spawn(title(&font, "VICINITY", 16.0, LootTab::Vicinity)).id();
    let lt = c.spawn(title(&font, "LOOT", 16.0, LootTab::Loot)).id();
    c.entity(tabs).add_children(&[vt, lt]);
    for t in [vt, lt] {
        c.entity(t).insert(Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), padding: UiRect::all(Val::Px(5.0)), ..default() });
    }
    c.entity(loot_col).add_children(&[loot, tabs]);

    // --- equipment
    let (eq_col, eq) = column(c, 620.0);
    c.entity(eq).insert(Node {
        flex_grow: 1.0,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        overflow: Overflow::scroll_y(),
        ..default()
    });
    let inv = player_inv(game).clone();
    if ui.crafting {
        build_crafting(c, &mut b, &inv, &mut ui, craft, eq);
    }
    for (ri, slots) in Slot::ROWS.iter().enumerate().filter(|_| !ui.crafting) {
        let r = c.spawn(Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }).id();
        if ri == 2 {
            // Primary over secondary | sidearm over melee.
            let left = c.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
            let right = c.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
            for s in [Slot::Primary, Slot::Secondary] {
                let e = b.slot(c, s, inv.equipped.get(&s));
                c.entity(left).add_child(e);
            }
            for s in [Slot::Sidearm, Slot::Melee] {
                let e = b.slot(c, s, inv.equipped.get(&s));
                c.entity(right).add_child(e);
            }
            c.entity(r).add_children(&[left, right]);
        } else {
            for s in slots.iter() {
                let e = b.slot(c, *s, inv.equipped.get(s));
                c.entity(r).add_child(e);
            }
        }
        c.entity(eq).add_child(r);
    }
    let eq_tabs = c.spawn(Node { width: Val::Percent(100.0), ..default() }).id();
    let et = c.spawn(title(&font, "EQUIPMENT", 16.0, EquipTab(false))).id();
    let ct = c.spawn(title(&font, "CRAFTING", 16.0, EquipTab(true))).id();
    c.entity(eq_tabs).add_children(&[et, ct]);
    for t in [et, ct] {
        c.entity(t).insert(Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), padding: UiRect::all(Val::Px(5.0)), ..default() });
    }
    c.entity(eq_col).add_children(&[eq, eq_tabs]);

    // --- inventory: pockets, then each worn container's compartments
    let (inv_col, invc) = column(c, 620.0);
    let head = c
        .spawn((
            Node { width: Val::Percent(100.0), justify_content: JustifyContent::SpaceBetween, padding: UiRect::all(Val::Px(5.0)), ..default() },
            BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.0, 0.701961)),
            children![
                label_colored(&font, "INVENTORY", 16.0, Color::WHITE),
                label_colored(&font, format!("{:.1} KG", inv.weight(db)), 12.0, TEXT),
            ],
        ))
        .id();
    let pockets = b.grid(c, &inv.pockets, Loc::Player(GridRef::Pockets));
    c.entity(invc).add_child(pockets);
    for slot in WORN_GRIDS {
        let Some(stack) = inv.equipped.get(&slot) else { continue };
        if stack.contents.is_empty() {
            continue;
        }
        let name = db.items.get(&stack.item).map_or(stack.item.clone(), |d| d.name.to_uppercase());
        let t = c.spawn(title(&font, &name, 15.0, ())).id();
        c.entity(t).insert(Node { width: Val::Percent(100.0), flex_shrink: 0.0, padding: UiRect::all(Val::Px(5.0)), ..default() });
        let wrap = c.spawn(Node { flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::Center, column_gap: Val::Px(10.0), row_gap: Val::Px(10.0), ..default() }).id();
        for (i, g) in stack.contents.iter().enumerate() {
            let e = b.grid(c, g, Loc::Player(GridRef::Worn(slot, i)));
            c.entity(wrap).add_child(e);
        }
        c.entity(invc).add_children(&[t, wrap]);
    }
    c.entity(inv_col).add_children(&[head, invc]);

    c.entity(row).add_children(&[loot_col, eq_col, inv_col]);

    let hint = c
        .spawn((
            label_colored(&font, "[Drag] Move   [R] Rotate   [Shift + Drop] Split   [Ctrl + Click] Quick move   [Right Click] Options   [Tab / Esc] Close", 11.0, TEXT),
            Node { position_type: PositionType::Absolute, bottom: Val::Px(14.0), ..default() },
            Pickable::IGNORE,
        ))
        .id();
    let tip = c
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                row_gap: Val::Px(2.0),
                min_width: Val::Px(200.0),
                ..default()
            },
            BackgroundColor(Color::linear_rgba(0.0025, 0.0025, 0.0025, 0.9)),
            Visibility::Hidden,
            GlobalZIndex(20),
            Pickable::IGNORE,
            Tooltip,
        ))
        .id();
    c.entity(root).add_children(&[hint, tip]);
    if let Some(d) = ui.split {
        let name = b.font;
        let dialog = split_dialog(c, name, &d);
        c.entity(root).add_child(dialog);
    }
}

/// `W_PlayerCraftingUI`: CRAFTABLE ITEMS (10x10), REQUIRED ITEMS (7x3, for the chosen amount),
/// then `[<] Craft Selected Item [>]`.
fn build_crafting(c: &mut Commands, b: &mut Build, inv: &PlayerInventory, ui: &mut UiState, view: &mut CraftView, parent: Entity) {
    let db = b.db;
    let recipes = db.recipes.get("Player").map(Vec::as_slice).unwrap_or(&[]);
    view.craft = Grid::new("Craftable", 10, 10);
    view.recipe_of.clear();
    for (i, r) in recipes.iter().enumerate() {
        if let Some((x, y, rot)) = view.craft.free_spot(db, &r.item)
            && view.craft.place_rotated(db, db.stack(&r.item, r.count), x, y, rot).is_ok()
        {
            view.recipe_of.push(i);
        }
    }
    let selected = ui.recipe.and_then(|i| recipes.get(i));
    ui.craft_times = ui.craft_times.max(1);
    let times = ui.craft_times;
    view.required = Grid::new("Required", 7, 3);
    b.missing.clear();
    for (item, n) in selected.map_or(&[][..], |r| r.needs.as_slice()) {
        let need = n * times;
        let _ = view.required.insert(db, db.stack(item, need));
        if inv.count(item) < need {
            b.missing.push(item.clone());
        }
    }
    let can = selected.is_some_and(|r| inv.craftable(r) >= times);

    let font = b.font;
    let top = c.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() }).id();
    let t1 = c.spawn(title(font, "CRAFTABLE ITEMS", 12.0, ())).id();
    let g1 = b.grid(c, &view.craft, Loc::Craft);
    c.entity(top).add_children(&[g1, t1]);
    let bottom = c.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, margin: UiRect::top(Val::Px(5.0)), ..default() }).id();
    let t2 = c.spawn(title_in(font, "REQUIRED ITEMS", 15.0, Color::linear_rgb(1.0, 0.718804, 0.0), ())).id();
    let g2 = b.grid(c, &view.required, Loc::Required);
    c.entity(bottom).add_children(&[t2, g2]);

    let arrow = |c: &mut Commands, n: u32, which: CraftButton| {
        let tex = format!("game/SurrounDead/Content/UI/CleanFlatIcons/Textures/png_128/icon/icon_arrow/icon_arrow_{n}.png");
        c.spawn((Node { width: Val::Px(36.0), height: Val::Px(36.0), ..default() }, ImageNode::new(b.assets.load(tex)).with_color(Color::linear_rgb(0.5, 0.5, 0.5)), Hovered::default(), which)).id()
    };
    let less = arrow(c, 13, CraftButton::Less);
    let more = arrow(c, 14, CraftButton::More);
    // TextBlock_75 is bound to GetText; the amount suffix is our reading of what it shows.
    let text = if times > 1 { format!("Craft Selected Item x{times}") } else { "Craft Selected Item".to_string() };
    let craft = c
        .spawn((
            Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(Color::linear_rgba(0.411458, 0.411458, 0.411458, if can { 1.0 } else { 0.35 })),
            CraftButton::Craft,
            Hovered::default(),
            children![(label_colored(font, text, 18.0, if can { Color::WHITE } else { Color::srgba(1.0, 1.0, 1.0, 0.5) }), Pickable::IGNORE)],
        ))
        .id();
    let buttons = c.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(5.0)), ..default() }).id();
    c.entity(buttons).add_children(&[less, craft, more]);
    c.entity(parent).add_children(&[top, bottom, buttons]);
}

/// `JigSplitWidget`: "Split Item", the amount, a slider, Cancel / Confirm, over a dimmed screen.
fn split_dialog(c: &mut Commands, font: &MenuFont, d: &SplitDialog) -> Entity {
    fn btn(font: &MenuFont, text: &'static str, which: SplitButton, grow: f32) -> impl Bundle {
        (
            Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(4.0)), flex_grow: grow, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(Color::linear_rgb(0.057292, 0.057292, 0.057292)),
            Hovered::default(),
            which,
            children![(label_colored(font, text, 16.0, Color::WHITE), Pickable::IGNORE)],
        )
    }
    let frac = if d.max > 2 { (d.value - 1) as f32 / (d.max - 2) as f32 } else { 1.0 };
    c.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
        GlobalZIndex(50),
        children![(
            Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Stretch, padding: UiRect::all(Val::Px(10.0)), row_gap: Val::Px(5.0), min_width: Val::Px(240.0), ..default() },
            BackgroundColor(Color::linear_rgba(0.01, 0.01, 0.01, 0.95)),
            children![
                (label_colored(font, "Split Item", 22.0, Color::WHITE), Node { align_self: AlignSelf::Center, ..default() }),
                (label_colored(font, d.value.to_string(), 16.0, Color::WHITE), Node { align_self: AlignSelf::End, margin: UiRect::horizontal(Val::Px(5.0)), ..default() }, SplitCount),
                (
                    Node { width: Val::Px(200.0), height: Val::Px(15.0), margin: UiRect::vertical(Val::Px(5.0)), align_self: AlignSelf::Center, ..default() },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15)),
                    Hovered::default(),
                    SplitTrack,
                    children![(Node { width: Val::Percent(frac * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgb(0.8, 0.8, 0.8)), Pickable::IGNORE, SplitFill)],
                ),
                (
                    Node { column_gap: Val::Px(6.0), ..default() },
                    children![(Node { width: Val::Px(15.0), ..default() }), btn(font, "Cancel", SplitButton::Cancel, 0.0), btn(font, "Confirm", SplitButton::Confirm, 1.0)],
                ),
            ],
        )],
    ))
    .id()
}

// ---------------------------------------------------------------- hover, tooltip, drag ghost

fn cursor(windows: &Query<&Window, With<PrimaryWindow>>, scale: &UiScale) -> Option<Vec2> {
    windows.single().ok()?.cursor_position().map(|p| p / scale.0)
}

#[allow(clippy::too_many_arguments)]
fn hover_and_tooltip(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    mut data: InvData,
    ui: Res<UiState>,
    font: Res<MenuFont>,
    mut items: Query<(&Hovered, &ItemRef, &mut BorderColor, &mut Node), Without<Tooltip>>,
    mut tips: Query<(Entity, &mut Node, &mut Visibility), With<Tooltip>>,
    menus: Query<(), With<ContextMenu>>,
    mut shown: Local<Option<(String, u32)>>,
) {
    let mut hovered: Option<ItemRef> = None;
    // The chosen recipe stays outlined in CRAFTABLE ITEMS.
    let selected = ui.recipe.and_then(|i| data.craft.recipe_of.iter().position(|&j| j == i));
    for (h, r, mut border, mut node) in &mut items {
        let on = h.get();
        let picked = r.loc == Loc::Craft && Some(r.index) == selected;
        node.border = UiRect::all(Val::Px(if on || picked { 2.0 } else { 0.0 }));
        *border = BorderColor::all(if on { HOVER } else if picked { SELECTED } else { Color::NONE });
        if on {
            hovered = Some(*r);
        }
    }
    let Ok((tip, mut node, mut vis)) = tips.single_mut() else { return };
    let (mut ctx, _) = data.ctx(Vec3::ZERO);
    let stack = hovered.filter(|_| menus.is_empty() && ui.dragging.is_none()).and_then(|r| ctx.peek(r)).map(|p| p.0);
    let db = ctx.db;
    let Some(stack) = stack else {
        *vis = Visibility::Hidden;
        *shown = None;
        return;
    };
    *vis = Visibility::Inherited;
    if let Some(p) = cursor(&windows, &scale) {
        node.left = Val::Px(p.x + 18.0);
        node.top = Val::Px(p.y + 12.0);
    }
    if shown.as_ref() == Some(&(stack.item.clone(), stack.count + stack.rounds + stack.mag(db).map_or(0, |m| m.rounds + 1000))) {
        return;
    }
    *shown = Some((stack.item.clone(), stack.count + stack.rounds + stack.mag(db).map_or(0, |m| m.rounds + 1000)));
    let Some(def) = db.items.get(&stack.item) else { return };
    commands.entity(tip).despawn_related::<Children>();
    // OnHoverTooltipWidget: Type | Rarity, Name, divider, price / weight, stats.
    let spaced = |s: &str| s.chars().fold(String::new(), |mut o, ch| {
        if ch.is_uppercase() && !o.is_empty() {
            o.push(' ');
        }
        o.push(ch);
        o
    });
    let header = commands
        .spawn((Node { justify_content: JustifyContent::SpaceBetween, column_gap: Val::Px(20.0), ..default() }, children![label_colored(&font, spaced(&def.kind), 10.0, TEXT), label_colored(&font, def.rarity.clone(), 10.0, TEXT)]))
        .id();
    let name = commands.spawn(label_colored(&font, def.name.clone(), 16.0, TEXT)).id();
    let div = commands.spawn((Node { height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(5.0)), ..default() }, BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.4)))).id();
    let mut lines = vec![format!("Price: ${:.0}", def.price), format!("Weight: {:.1}KG", stack.weight(db))];
    if !def.grids.is_empty() {
        lines.push(format!("Storage: {}", def.grids.iter().map(|[w, h]| format!("{w}x{h}")).collect::<Vec<_>>().join(" + ")));
    }
    // Rolled weapon/armour stats of this instance, then magazine contents.
    let unit = |k: &str| match k {
        "CriticalHitChance" | "CriticalHitMultiplier" => "%",
        "DamageFallOff" => "m",
        _ => "",
    };
    for (k, v) in &stack.stats {
        lines.push(format!("{}: {v:.0}{}", spaced(k), unit(k)));
    }
    if def.capacity > 0 {
        lines.push(format!("Rounds: {} / {}", stack.rounds, def.capacity));
    }
    if let Some(mag) = stack.mag(db) {
        let cap = db.items.get(&mag.item).map_or(0, |d| d.capacity);
        lines.push(format!("Magazine: {} / {}", mag.rounds, cap));
    }
    if let Some(modes) = def.text_stats.get("Firemodes") {
        lines.push(format!("Firemodes: {modes}"));
    }
    if let Some(cal) = def.text_stats.get("Caliber") {
        lines.push(format!("Caliber: {cal}"));
    }
    let mut stats: Vec<_> = def.stats.iter().collect();
    stats.sort_by(|a, b| a.0.cmp(b.0));
    for (k, [lo, hi]) in stats {
        lines.push(if lo == hi { format!("{}: {:+.0}", spaced(k), lo) } else { format!("{}: {:+.0} - {:+.0}", spaced(k), lo, hi) });
    }
    let texts: Vec<Entity> = lines.into_iter().map(|l| commands.spawn(label_colored(&font, l, 10.0, TEXT)).id()).collect();
    commands.entity(tip).add_children(&[header, name, div]).add_children(&texts);
}

/// R while dragging turns the item; the ghost swaps its footprint and turns its picture.
fn rotate_ghost(
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<UiState>,
    mut ghosts: Query<(&Ghost, &mut Node, &Children), Without<GhostImage>>,
    mut pictures: Query<(&mut Node, &mut UiTransform), With<GhostImage>>,
) {
    if ui.dragging.is_none() {
        return;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        ui.drag_rotated = !ui.drag_rotated;
    }
    for (g, mut node, kids) in &mut ghosts {
        let (w, h) = if ui.drag_rotated { (g.base.y, g.base.x) } else { (g.base.x, g.base.y) };
        node.width = Val::Px(w);
        node.height = Val::Px(h);
        for k in kids.iter() {
            if let Ok((mut n, mut t)) = pictures.get_mut(k) {
                let (pn, pt) = if ui.drag_rotated { rotated_picture(w, h) } else { (Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, UiTransform::default()) };
                *n = pn;
                *t = pt;
            }
        }
    }
}

fn follow_cursor(windows: Query<&Window, With<PrimaryWindow>>, scale: Res<UiScale>, mut ghosts: Query<(&mut Node, &ComputedNode), With<Ghost>>) {
    let Some(p) = cursor(&windows, &scale) else { return };
    for (mut n, c) in &mut ghosts {
        let size = c.size() * c.inverse_scale_factor();
        n.left = Val::Px(p.x - size.x / 2.0);
        n.top = Val::Px(p.y - size.y / 2.0);
    }
}

/// Dragging along the split slider picks the amount (1 .. stack - 1); arrow keys nudge it.
#[allow(clippy::too_many_arguments)]
fn split_slider(
    mut ui: ResMut<UiState>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    tracks: Query<(&Hovered, &ComputedNode, &UiGlobalTransform), With<SplitTrack>>,
    mut fills: Query<&mut Node, With<SplitFill>>,
    mut counts: Query<&mut Text, With<SplitCount>>,
    mut grabbing: Local<bool>,
) {
    let Some(d) = ui.split.as_mut() else {
        *grabbing = false;
        return;
    };
    let Ok((hovered, node, at)) = tracks.single() else { return };
    let hi = d.max.saturating_sub(1).max(1);
    if mouse.just_pressed(MouseButton::Left) && hovered.get() {
        *grabbing = true;
    }
    if !mouse.pressed(MouseButton::Left) {
        *grabbing = false;
    }
    if *grabbing && let Some(p) = windows.single().ok().and_then(|w| w.physical_cursor_position()) {
        let size = node.size();
        let f = ((p.x - (at.translation.x - size.x / 2.0)) / size.x.max(1.0)).clamp(0.0, 1.0);
        d.value = 1 + (f * (hi - 1) as f32).round() as u32;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        d.value = d.value.saturating_sub(1).max(1);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        d.value = (d.value + 1).min(hi);
    }
    let frac = if hi > 1 { (d.value - 1) as f32 / (hi - 1) as f32 } else { 1.0 };
    for mut n in &mut fills {
        n.width = Val::Percent(frac * 100.0);
    }
    for mut t in &mut counts {
        t.0 = d.value.to_string();
    }
}

/// Dropped items show their inspect mesh where they lie.
fn sync_ground_models(mut commands: Commands, mut ground: ResMut<Ground>, db: Res<ItemDb>, assets: Res<AssetServer>) {
    if !ground.is_changed() {
        return;
    }
    for e in std::mem::take(&mut ground.despawn) {
        commands.entity(e).despawn();
    }
    for (pos, stack, e) in ground.items.iter_mut().filter(|g| g.2.is_none()) {
        let mut ent = commands.spawn((Transform::from_translation(*pos), Visibility::default(), DespawnOnExit(AppState::InGame)));
        if let Some(m) = db.items.get(&stack.item).and_then(|d| d.mesh.as_ref()) {
            ent.insert(model(&assets, format!("game/{m}.glb")));
        }
        *e = Some(ent.id());
    }
}

// ---------------------------------------------------------------- input

/// Everything an inventory edit touches.
#[derive(bevy::ecs::system::SystemParam)]
struct InvData<'w> {
    db: Res<'w, ItemDb>,
    game: ResMut<'w, CurrentGame>,
    containers: ResMut<'w, Containers>,
    ground: ResMut<'w, Ground>,
    view: ResMut<'w, VicinityView>,
    dirty: ResMut<'w, Dirty>,
    craft: ResMut<'w, CraftView>,
}

impl InvData<'_> {
    fn ctx(&mut self, player: Vec3) -> (Ctx<'_>, &mut Dirty) {
        (
            Ctx { db: &self.db, game: &mut self.game, containers: &mut self.containers, ground: &mut self.ground, view: &self.view, craft: &self.craft, player },
            &mut self.dirty,
        )
    }
}

fn on_drag_start(
    ev: On<PointerDragStart>,
    mut commands: Commands,
    refs: Query<(&ItemRef, &Children)>,
    images: Query<&ImageNode>,
    mut ui: ResMut<UiState>,
    mut data: InvData,
) {
    if ev.button != PointerButton::Primary || ui.split.is_some() {
        return;
    }
    let Ok((r, kids)) = refs.get(ev.event_target()) else { return };
    if r.loc.display_only() {
        return;
    }
    let (mut ctx, _) = data.ctx(Vec3::ZERO);
    let Some((stack, placed)) = ctx.peek(*r) else { return };
    let (w, h) = footprint(ctx.db, &stack.item, false).unwrap_or((1, 1));
    ui.dragging = Some(*r);
    ui.drag_rotated = placed.is_some_and(|p| p.rotated);
    let img = kids.iter().find_map(|k| images.get(k).ok().cloned()).unwrap_or_default();
    let base = Vec2::new(w as f32 * CELL - 2.0, h as f32 * CELL - 2.0);
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Px(base.x), height: Val::Px(base.y), ..default() },
        BackgroundColor(Color::linear_rgba(0.0, 0.432292, 0.0, 0.5)),
        GlobalZIndex(30),
        Pickable::IGNORE,
        Ghost { base },
        DespawnOnExit(AppState::InGame),
        children![(
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            UiTransform::default(),
            img.with_color(Color::srgba(1.0, 1.0, 1.0, 0.85)),
            Pickable::IGNORE,
            GhostImage,
        )],
    ));
}

fn on_drag_end(_: On<PointerDragEnd>, mut commands: Commands, ghosts: Query<Entity, With<Ghost>>, mut ui: ResMut<UiState>) {
    ui.dragging = None;
    for g in &ghosts {
        commands.entity(g).despawn();
    }
}

#[allow(clippy::too_many_arguments)]
fn on_drop(
    mut ev: On<PointerDragDrop>,
    cells: Query<&CellRef>,
    slots: Query<&SlotWidget>,
    items: Query<(&ItemRef, &ComputedNode, &UiGlobalTransform)>,
    zones: Query<(), With<DropZone>>,
    panels: Query<(), With<Panel>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut ui: ResMut<UiState>,
    mut data: InvData,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
) {
    let Some(src) = ui.dragging else { return };
    let target = ev.event_target();
    let handled = cells.contains(target) || slots.contains(target) || items.contains(target) || zones.contains(target) || panels.contains(target);
    if !handled {
        return; // keep propagating to something that cares
    }
    ev.propagate(false);
    let rotated = ui.drag_rotated;
    let here = player.single().map(|t| t.translation).unwrap_or_default();
    let (mut ctx, dirty) = data.ctx(here);
    // The grid cell under the cursor, also when the cursor is over an item.
    let item_target = items.get(target).ok().map(|(t, node, at)| (*t, node.size(), at.translation));
    let cell = cells.get(target).ok().copied().or_else(|| {
        let (t, size, centre) = item_target?;
        let p = windows.single().ok()?.physical_cursor_position()?;
        let placed = ctx.peek(t)?.1?;
        let local = (p - (centre - size / 2.0)) / size.max(Vec2::ONE);
        let x = placed.x + ((local.x * placed.w as f32).floor().max(0.0) as u8).min(placed.w - 1);
        let y = placed.y + ((local.y * placed.h as f32).floor().max(0.0) as u8).min(placed.h - 1);
        Some(CellRef { loc: t.loc, x, y })
    });
    if cell.is_some_and(|c| c.loc.display_only()) || item_target.is_some_and(|t| t.0.loc.display_only()) || (panels.contains(target) && cell.is_none()) {
        return;
    }
    // Shift: JigSplitWidget for stacks of 2+ dropped on a grid.
    if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
        && let Some(cell) = cell
        && let Some((stack, _)) = ctx.peek(src)
        && stack.count > 1
        && !matches!(src.loc, Loc::Equip(_))
    {
        ui.split = Some(SplitDialog { src, cell, rotated, max: stack.count, value: stack.count / 2 });
        dirty.0 = true;
        return;
    }
    if let Ok(cell) = cells.get(target) {
        ctx.drop_onto_cell(src, *cell, rotated);
    } else if let Ok(SlotWidget(slot)) = slots.get(target) {
        ctx.drop_onto_slot(src, *slot);
    } else if let Some((t, ..)) = item_target {
        ctx.drop_onto_item(src, t, cell, rotated);
    } else {
        // Outside every panel: drop it on the ground.
        ctx.act(Action::Drop, src);
    }
    dirty.0 = true;
}

/// Everything on the inventory screen that reacts to clicks.
#[derive(bevy::ecs::system::SystemParam)]
struct Clickables<'w, 's> {
    items: Query<'w, 's, &'static ItemRef>,
    tabs: Query<'w, 's, &'static LootTab>,
    options: Query<'w, 's, &'static MenuOption>,
    menus: Query<'w, 's, Entity, With<ContextMenu>>,
    roots: Query<'w, 's, Entity, With<InvRoot>>,
    buttons: Query<'w, 's, &'static CraftButton>,
    equip_tabs: Query<'w, 's, &'static EquipTab>,
    split_buttons: Query<'w, 's, &'static SplitButton>,
}

#[allow(clippy::too_many_arguments)]
fn on_click(
    mut ev: On<PointerClick>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    clickables: Clickables,
    overlay: Res<GameOverlay>,
    mut ui: ResMut<UiState>,
    mut data: InvData,
    font: Res<MenuFont>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
) {
    let Clickables { items, tabs, options, menus, roots, buttons, equip_tabs, split_buttons } = clickables;
    let target = ev.event_target();
    let relevant = items.contains(target)
        || tabs.contains(target)
        || options.contains(target)
        || roots.contains(target)
        || buttons.contains(target)
        || equip_tabs.contains(target)
        || split_buttons.contains(target);
    if !relevant {
        return;
    }
    ev.propagate(false);
    if let Ok(b) = split_buttons.get(target) {
        if let (SplitButton::Confirm, Some(d)) = (b, ui.split) {
            let here = player.single().map(|t| t.translation).unwrap_or_default();
            data.ctx(here).0.split(d.src, d.cell, d.value, d.rotated);
        }
        ui.split = None;
        data.dirty.0 = true;
        return;
    }
    if ui.split.is_some() {
        return; // modal
    }
    // Any click closes an open context menu (after acting on it).
    let menu_open = !menus.is_empty();
    for m in &menus {
        commands.entity(m).despawn();
    }
    let here = player.single().map(|t| t.translation).unwrap_or_default();
    let loot_open = match *overlay {
        GameOverlay::Loot(a) if !ui.vicinity => Some(a),
        _ => None,
    };

    if let Ok(tab) = tabs.get(target) {
        ui.vicinity = matches!(tab, LootTab::Vicinity);
        data.dirty.0 = true;
        return;
    }
    if let Ok(EquipTab(crafting)) = equip_tabs.get(target) {
        ui.crafting = *crafting;
        data.dirty.0 = true;
        return;
    }
    if let Ok(b) = buttons.get(target) {
        let recipe = ui.recipe;
        let (mut ctx, dirty) = data.ctx(here);
        let most = recipe.and_then(|i| ctx.db.recipes.get("Player")?.get(i)).map_or(1, |r| ctx.inv().craftable(r).max(1));
        match b {
            CraftButton::Less => ui.craft_times = ui.craft_times.saturating_sub(1).max(1),
            CraftButton::More => ui.craft_times = (ui.craft_times + 1).min(most),
            CraftButton::Craft => {
                if let Some(i) = recipe
                    && ctx.craft(i, ui.craft_times)
                {
                    ui.craft_times = 1;
                }
            }
        }
        dirty.0 = true;
        return;
    }
    let (mut ctx, dirty) = data.ctx(here);
    let db = ctx.db;
    if let Ok(MenuOption(action, r)) = options.get(target) {
        ctx.act(*action, *r);
        dirty.0 = true;
        return;
    }
    let Ok(r) = items.get(target).copied() else { return };
    if r.loc == Loc::Craft {
        if ev.button == PointerButton::Primary {
            ui.recipe = ctx.craft.recipe_of.get(r.index).copied();
            ui.craft_times = 1;
            dirty.0 = true;
        }
        return;
    }
    if r.loc.display_only() {
        return;
    }
    match ev.button {
        PointerButton::Primary if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) => {
            ctx.quick_move(r, loot_open);
            dirty.0 = true;
        }
        PointerButton::Secondary if !menu_open => {
            // JigContextMenuW: the options this item supports.
            let item = ctx.peek(r).map(|p| p.0.item);
            let Some(key) = item else { return };
            let Some(def) = db.items.get(&key) else { return };
            let mut opts = Vec::new();
            if matches!(r.loc, Loc::Equip(_)) {
                opts.push(("Unequip", Action::Unequip));
            } else {
                if player_inv(ctx.game).slot_for(db, &key).is_some() {
                    opts.push(("Equip", Action::Equip));
                }
                if def.consumable {
                    opts.push((if def.kind.contains("Drink") { "Drink" } else if def.kind.contains("Food") { "Eat" } else { "Use" }, Action::Use));
                }
            }
            if let Some((stack, _)) = ctx.peek(r)
                && r.loc != Loc::Vicinity
                && (stack.mag(db).is_some() || (def.capacity > 0 && stack.rounds > 0))
            {
                opts.push(("Unload", Action::Unload));
            }
            if r.loc != Loc::Vicinity {
                opts.push(("Drop", Action::Drop));
            }
            let at = cursor(&windows, &scale).unwrap_or_default();
            let menu = commands
                .spawn((
                    Node { position_type: PositionType::Absolute, left: Val::Px(at.x), top: Val::Px(at.y), flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(4.0)), min_width: Val::Px(120.0), ..default() },
                    BackgroundColor(Color::linear_rgba(0.0025, 0.0025, 0.0025, 0.95)),
                    GlobalZIndex(40),
                    ContextMenu,
                    DespawnOnExit(AppState::InGame),
                ))
                .id();
            for (text, action) in opts {
                let o = commands
                    .spawn((
                        Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), ..default() },
                        BackgroundColor(Color::NONE),
                        Hovered::default(),
                        MenuOption(action, r),
                        children![(label_colored(&font, text, 11.0, Color::WHITE), Pickable::IGNORE)],
                    ))
                    .id();
                commands.entity(menu).add_child(o);
            }
            if let Some(root) = roots.iter().next() {
                commands.entity(root).add_child(menu);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Option<ItemDb> {
        serde_json::from_str(&std::fs::read_to_string("assets/data/items.json").ok()?).ok()
    }

    fn with_ctx(db: &ItemDb, f: impl FnOnce(&mut Ctx)) -> (CurrentGame, Containers, Ground) {
        let (mut game, mut containers, mut ground, view, craft) = (CurrentGame::default(), Containers::default(), Ground::default(), VicinityView::default(), CraftView::default());
        {
            let mut ctx = Ctx { db, game: &mut game, containers: &mut containers, ground: &mut ground, view: &view, craft: &craft, player: Vec3::ZERO };
            f(&mut ctx);
        }
        (game, containers, ground)
    }

    #[test]
    fn equip_from_pockets_and_swap_back() {
        let Some(db) = db() else { return };
        let (mut game, _, ground) = with_ctx(&db, |ctx| {
            ctx.inv().pockets.place_at(&db, db.stack("DA_WandererJacket", 1), 0, 0).unwrap();
            ctx.act(Action::Equip, ItemRef { loc: Loc::Player(GridRef::Pockets), index: 0 });
        });
        let inv = player_inv(&mut game);
        assert!(inv.pockets.items.is_empty());
        assert_eq!(inv.equipped[&Slot::Torso].item, "DA_WandererJacket");
        assert_eq!(inv.grids().len(), 3, "pockets + the jacket's two 3x2 pockets");
        assert!(ground.items.is_empty());
    }

    #[test]
    fn drop_onto_free_cell_moves_and_onto_occupied_cell_restores() {
        let Some(db) = db() else { return };
        let (_, containers, _) = with_ctx(&db, |ctx| {
            let mut loot = Grid::new("Crate", 10, 6);
            loot.place_at(&db, db.stack("DA_M4CQB", 1), 2, 2).unwrap();
            loot.place_at(&db, db.stack("DA_CannedBakedBeans", 1), 0, 0).unwrap();
            ctx.containers.searched.insert(7, loot);
            // Rifle (4x2) dropped centred on loot cell (5, 0): lands at (4, 0).
            ctx.drop_onto_cell(ItemRef { loc: Loc::Loot(7), index: 0 }, CellRef { loc: Loc::Loot(7), x: 5, y: 0 }, false);
            // Beans dropped onto the rifle's cells: no room, so they stay put.
            ctx.drop_onto_cell(ItemRef { loc: Loc::Loot(7), index: 0 }, CellRef { loc: Loc::Loot(7), x: 4, y: 0 }, false);
        });
        let g = &containers.searched[&7];
        let rifle = g.items.iter().find(|p| p.stack.item == "DA_M4CQB").unwrap();
        assert_eq!((rifle.x, rifle.y), (4, 0));
        let beans = g.items.iter().find(|p| p.stack.item == "DA_CannedBakedBeans").unwrap();
        assert_eq!((beans.x, beans.y), (0, 0));
        assert_eq!(g.items.len(), 2);
    }

    #[test]
    fn rotated_rifle_fits_a_narrow_grid_and_turns_back_on_itself() {
        let Some(db) = db() else { return };
        let (_, containers, _) = with_ctx(&db, |ctx| {
            ctx.containers.searched.insert(1, Grid::new("Narrow", 2, 4));
            ctx.containers.searched.insert(2, Grid::new("Wide", 4, 2));
            ctx.containers.searched.get_mut(&2).unwrap().place_at(&db, db.stack("DA_M4CQB", 1), 0, 0).unwrap();
            // Upright (4x2) doesn't fit 2x4; turned it does.
            ctx.drop_onto_cell(ItemRef { loc: Loc::Loot(2), index: 0 }, CellRef { loc: Loc::Loot(1), x: 0, y: 1 }, false);
            assert_eq!(ctx.containers.searched[&1].items.len(), 0);
            ctx.drop_onto_cell(ItemRef { loc: Loc::Loot(2), index: 0 }, CellRef { loc: Loc::Loot(1), x: 0, y: 1 }, true);
            // Dropped back onto itself upright: no room in 2x4, so it stays turned.
            let at = CellRef { loc: Loc::Loot(1), x: 0, y: 0 };
            ctx.drop_onto_item(ItemRef { loc: Loc::Loot(1), index: 0 }, ItemRef { loc: Loc::Loot(1), index: 0 }, Some(at), false);
        });
        let p = &containers.searched[&1].items[0];
        assert!(p.rotated && (p.w, p.h) == (2, 4) && (p.x, p.y) == (0, 0));
        assert!(containers.searched[&2].items.is_empty());
    }

    #[test]
    fn split_moves_part_and_merges_onto_matching_stack() {
        let Some(db) = db() else { return };
        let (mut game, _, _) = with_ctx(&db, |ctx| {
            ctx.inv().pockets.place_at(&db, db.stack("DA_556Ammo", 60), 0, 0).unwrap();
            let src = ItemRef { loc: Loc::Player(GridRef::Pockets), index: 0 };
            ctx.split(src, CellRef { loc: Loc::Player(GridRef::Pockets), x: 3, y: 1 }, 20, false);
            // Onto the 20-stack's own cell: merges there.
            ctx.split(src, CellRef { loc: Loc::Player(GridRef::Pockets), x: 3, y: 1 }, 5, false);
        });
        let inv = player_inv(&mut game);
        let mut counts: Vec<u32> = inv.pockets.items.iter().map(|p| p.stack.count).collect();
        counts.sort();
        assert_eq!(counts, vec![25, 35]);
    }

    #[test]
    fn crafting_consumes_ingredients_and_gives_the_product() {
        let Some(db) = db() else { return };
        let rags = db.recipes["Player"].iter().position(|r| r.item == "DA_Rags").unwrap();
        let (mut game, _, _) = with_ctx(&db, |ctx| {
            assert!(!ctx.craft(rags, 1));
            ctx.inv().pockets.place_at(&db, db.stack("DA_Cloth", 4), 0, 0).unwrap();
            assert!(ctx.craft(rags, 2));
        });
        let inv = player_inv(&mut game);
        assert_eq!((inv.count("DA_Cloth"), inv.count("DA_Rags")), (0, 4));
    }

    #[test]
    fn rounds_load_magazines_magazines_attach_and_unload_reverses_it() {
        let Some(db) = db() else { return };
        let (mut game, _, _) = with_ctx(&db, |ctx| {
            let pockets = |i| ItemRef { loc: Loc::Player(GridRef::Pockets), index: i };
            ctx.inv().pockets.place_at(&db, db.stack("DA_556Ammo", 40), 0, 0).unwrap();
            ctx.inv().pockets.place_at(&db, db.stack("DA_556Mag", 1), 2, 0).unwrap();
            // 40 rounds onto the empty 30-round magazine: 30 go in, 10 stay.
            ctx.drop_onto_item(pockets(0), pockets(1), None, false);
            let mag = ctx.inv().pockets.items.iter().find(|p| p.stack.item == "DA_556Mag").unwrap().stack.clone();
            assert_eq!(mag.rounds, 30);
            assert_eq!(ctx.inv().count("DA_556Ammo"), 10);
            // The magazine onto the equipped rifle attaches it.
            ctx.inv().equip(Slot::Primary, db.stack("DA_M4CQB", 1));
            let mi = ctx.inv().pockets.items.iter().position(|p| p.stack.item == "DA_556Mag").unwrap();
            ctx.drop_onto_slot(pockets(mi), Slot::Primary);
            assert_eq!(ctx.inv().equipped[&Slot::Primary].mag(&db).map(|m| m.rounds), Some(30));
            assert!(ctx.inv().pockets.items.iter().all(|p| p.stack.item != "DA_556Mag"));
            // Unload the rifle (magazine out), then the magazine (rounds out).
            ctx.act(Action::Unload, ItemRef { loc: Loc::Equip(Slot::Primary), index: 0 });
            assert!(ctx.inv().equipped[&Slot::Primary].mag(&db).is_none());
            let mi = ctx.inv().pockets.items.iter().position(|p| p.stack.item == "DA_556Mag").unwrap();
            ctx.act(Action::Unload, pockets(mi));
        });
        let inv = player_inv(&mut game);
        assert_eq!(inv.count("DA_556Ammo"), 40);
    }

    #[test]
    fn quick_move_both_ways_and_backpacks_keep_contents() {
        let Some(db) = db() else { return };
        let (mut game, containers, ground) = with_ctx(&db, |ctx| {
            let mut loot = Grid::new("Crate", 10, 6);
            loot.place_at(&db, db.stack("DA_CannedBakedBeans", 2), 0, 0).unwrap();
            ctx.containers.searched.insert(1, loot);
            ctx.quick_move(ItemRef { loc: Loc::Loot(1), index: 0 }, Some(1));
            assert_eq!(ctx.inv().pockets.items.len(), 1);
            ctx.quick_move(ItemRef { loc: Loc::Player(GridRef::Pockets), index: 0 }, Some(1));
            assert!(ctx.inv().pockets.items.is_empty());

            // A worn backpack with something inside, unequipped: the item travels inside it.
            let mut pack = db.stack("DA_SchoolBackpack", 1);
            pack.contents[0].place_at(&db, db.stack("DA_WaterBottle", 1), 0, 0).unwrap();
            ctx.inv().equip(Slot::Backpack, pack);
            ctx.act(Action::Unequip, ItemRef { loc: Loc::Equip(Slot::Backpack), index: 0 });
        });
        assert_eq!(containers.searched[&1].items.len(), 1);
        let inv = player_inv(&mut game);
        assert!(!inv.equipped.contains_key(&Slot::Backpack));
        let pack = inv.pockets.items.iter().map(|p| &p.stack).chain(ground.items.iter().map(|g| &g.1)).find(|s| s.item == "DA_SchoolBackpack").expect("backpack kept");
        assert_eq!(pack.contents[0].items[0].stack.item, "DA_WaterBottle");
    }
}
