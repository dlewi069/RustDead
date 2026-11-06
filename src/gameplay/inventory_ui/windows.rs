//! Container windows: JigSInventory's ContainerWindowWidget (decompiled) around an item's
//! storage SpecialContainerWidget (the backpacks', clothes', rigs' and cases' SC_ widgets).
//!
//! - JSI_Slot.On_MouseDoubleClick (left button) on an item with a SpecialContainerWidget, and the
//!   context menu's Open (JigContextMenuComp: Jig.Context.Open calls the same double click):
//!   CanOpenSpecialContainer? (its widget isn't shown already) opens a ContainerWindowWidget
//!   with ContainerName = the item's name and the SC_ widget in its VBox; otherwise it's
//!   OnDoubleClickInspect (the inspector). With DISABLE BACKPACK IN BACKPACK?
//!   (Difficulty.DisableBackpackInBackpack) a backpack goes to the inspector instead, and its
//!   context menu has no Open (DifficultyCheckBackpack?).
//! - A worn container's widget is the one in the INVENTORY column, so it's always shown and a
//!   double click there inspects.
//! - The window: MainBorder (0.005 grey at 0.95, padding 5 5 5 10), the TitleBorder (black 0.75:
//!   the name at 16 in E1E1E1 and the red CloseBtn) over the widget. GetPos puts it centred on
//!   the mouse, pulled back inside the viewport. Dragging the title moves it; a press brings it
//!   to the front (BP_InspectorCanvas.SetActiveWindow). Its Weight box is collapsed.
//! - The SC_ widget's boxes, spacers and JSIContainers come from items.json `layout`
//!   (gen_items.py `sc_layout`); the INVENTORY column draws worn containers with it too.
//!
//! Ours: a window sits on its item's cell (or ground spot) and closes when the item leaves it
//! (the game's follows the item's widget); windows open on a window's items too. The trader's
//! stock inspects instead (its bags would otherwise trade their contents for nothing). The
//! inspector's windows stay in front of these. The guns' SC_<gun> slot panels and SC_Battery /
//! SC_NVG / SC_Respirator have no window yet.

use bevy::picking::events::{PointerDrag, PointerPress};

use super::super::items::{ItemDef, Layout};
use super::*;
use crate::menus::settings::{SettingsStore, Value, row_key};

/// Under the inspector's windows (45 / 46) and the dragged item (30).
const Z: i32 = 24;
const PAGE: &str = "Loot / Environment";
const NO_BAG_IN_BAG: &str = "DISABLE BACKPACK IN BACKPACK? (RESTART RECOMMENDED)";

/// Where a window's item is: a grid cell, or its spot on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Owner {
    Cell { loc: Loc, x: u8, y: u8 },
    Ground(Vec3),
}

/// One ContainerWindowWidget.
#[derive(Clone, Debug)]
pub(super) struct ItemWindow {
    pub(super) id: u32,
    pub(super) owner: Owner,
    pub(super) item: String,
    /// Top-left in UI px; `None` until it's laid out and placed (GetPos).
    pub(super) pos: Option<Vec2>,
    /// The cursor when it opened.
    pub(super) at: Vec2,
}

/// The open container windows, the front one last.
#[derive(Resource, Default)]
pub struct ItemWindows {
    pub(super) windows: Vec<ItemWindow>,
    next: u32,
}

impl ItemWindows {
    pub(super) fn open(&mut self, owner: Owner, item: &str, at: Vec2) -> u32 {
        let id = self.next;
        self.next += 1;
        self.windows.push(ItemWindow { id, owner, item: item.to_string(), pos: None, at });
        id
    }

    pub(super) fn showing(&self, owner: Owner) -> bool {
        self.windows.iter().any(|w| w.owner == owner)
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// The tour's log line: each window's item, front last.
    pub fn items(&self) -> Vec<String> {
        self.windows.iter().map(|w| w.item.clone()).collect()
    }

    pub fn clear(&mut self) {
        self.windows.clear();
    }
}

/// What a double click (or Open) does with an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Opens {
    Window,
    Inspect,
    /// A special container we have no window for (a gun's slots, a battery slot).
    NotPorted(String),
}

/// JSI_Slot.On_MouseDoubleClick's choice. `open`: a window on it is up already.
pub(super) fn opens(def: &ItemDef, loc: Loc, open: bool, no_bag_in_bag: bool) -> Opens {
    let super::super::inspect::DoubleClick::Window(w) = super::super::inspect::double_click(def) else { return Opens::Inspect };
    if def.layout.is_none() {
        return Opens::NotPorted(w);
    }
    let vendor = matches!(loc, Loc::Loot(k) if traders::is_trader(k));
    if open || matches!(loc, Loc::Equip(_)) || vendor || (no_bag_in_bag && def.kind == "Backpack") {
        Opens::Inspect
    } else {
        Opens::Window
    }
}

/// Difficulty.DisableBackpackInBackpack.
pub(super) fn no_bag_in_bag(store: &SettingsStore) -> bool {
    store.find(&row_key("difficulty", PAGE, NO_BAG_IN_BAG)) == Some(Value::Bool(true))
}

/// The context menu offers Open (DifficultyCheckBackpack? takes it off backpacks). Ours: not on
/// worn items, whose widget is the INVENTORY column's.
pub(super) fn offers_open(def: &ItemDef, loc: Loc, no_bag_in_bag: bool) -> bool {
    def.context.iter().any(|c| c == "Open") && !matches!(loc, Loc::Equip(_)) && !(no_bag_in_bag && def.kind == "Backpack")
}

/// GetPos: centred on the mouse, pulled back inside the viewport (bottom and right edges
/// first, then the top and left).
pub(super) fn fit(mouse: Vec2, size: Vec2, viewport: Vec2) -> Vec2 {
    let mut p = mouse - size / 2.0;
    if p.y + size.y > viewport.y {
        p.y = viewport.y - size.y;
    } else if p.y < 0.0 {
        p.y = 0.0;
    }
    if p.x < 0.0 {
        p.x = 0.0;
    } else if p.x + size.x > viewport.x {
        p.x = viewport.x - size.x;
    }
    p
}

impl Ctx<'_> {
    /// The stack window `id` shows, while it's still where the window was opened on.
    pub(super) fn window_stack(&mut self, id: u32) -> Option<&mut Stack> {
        let windows = self.windows;
        let w = windows.iter().find(|w| w.id == id)?;
        let stack = match w.owner {
            Owner::Cell { loc, x, y } => &mut self.grid(loc)?.items.iter_mut().find(|p| p.x == x && p.y == y)?.stack,
            Owner::Ground(at) => &mut self.ground.items.iter_mut().find(|g| g.0 == at)?.1,
        };
        (stack.item == w.item).then_some(stack)
    }

    /// Where an item is, for a window on it (not equipped items: their widget is the column's).
    pub(super) fn owner(&mut self, r: ItemRef) -> Option<Owner> {
        match r.loc {
            Loc::Vicinity => {
                let gi = *self.view.0.get(r.index)?;
                self.ground.items.get(gi).map(|g| Owner::Ground(g.0))
            }
            Loc::Player(_) | Loc::Loot(_) | Loc::Window(..) => {
                let p = self.grid(r.loc)?.items.get(r.index)?;
                Some(Owner::Cell { loc: r.loc, x: p.x, y: p.y })
            }
            _ => None,
        }
    }
}

impl InvData<'_> {
    /// Closes the windows whose item moved or went (a window inside one goes with it), and
    /// gives the rest's stacks and places, front last.
    pub(super) fn window_stacks(&mut self) -> Vec<(u32, Stack, Option<Vec2>)> {
        let ids: Vec<(u32, Option<Vec2>)> = self.item_windows.windows.iter().map(|w| (w.id, w.pos)).collect();
        let shown: Vec<(u32, Stack, Option<Vec2>)> = {
            let (mut ctx, _) = self.ctx(Vec3::ZERO);
            ids.into_iter().filter_map(|(id, pos)| Some((id, ctx.window_stack(id)?.clone(), pos))).collect()
        };
        self.item_windows.windows.retain(|w| shown.iter().any(|s| s.0 == w.id));
        shown
    }
}

/// The window's root node.
#[derive(Component)]
pub(super) struct WindowNode(pub(super) u32);

/// Its TitleBorder (dragged to move it).
#[derive(Component)]
pub(super) struct WindowTitle(pub(super) u32);

/// Its CloseBtn.
#[derive(Component)]
pub(super) struct WindowClose(pub(super) u32);

impl Build<'_> {
    /// A storage SC_ widget: `grids[i]` drawn as JSIContainer `i` of the layout.
    pub(super) fn layout(&mut self, c: &mut Commands, layout: &Layout, grids: &[Grid], loc: &impl Fn(usize) -> Loc) -> Entity {
        let align = if layout.center { AlignSelf::Center } else { AlignSelf::Auto };
        if let Some(i) = layout.grid {
            let e = match grids.get(i) {
                Some(g) => self.grid(c, g, loc(i)),
                None => c.spawn(Node::default()).id(),
            };
            if !layout.center {
                return e;
            }
            let wrap = c.spawn(Node { align_self: align, flex_shrink: 0.0, ..default() }).id();
            c.entity(wrap).add_child(e);
            return wrap;
        }
        if let Some([w, h]) = layout.gap {
            return c.spawn(Node { width: Val::Px(w), height: Val::Px(h), flex_shrink: 0.0, align_self: align, ..default() }).id();
        }
        let (dir, kids) = match (&layout.v, &layout.h) {
            (Some(k), _) => (FlexDirection::Column, k.as_slice()),
            (_, Some(k)) => (FlexDirection::Row, k.as_slice()),
            _ => (FlexDirection::Row, &[][..]),
        };
        let b = c.spawn(Node { flex_direction: dir, align_items: AlignItems::FlexStart, flex_shrink: 0.0, align_self: align, ..default() }).id();
        for k in kids {
            let e = self.layout(c, k, grids, loc);
            c.entity(b).add_child(e);
        }
        b
    }

    /// ContainerWindowWidget around `stack`'s SC_ widget.
    pub(super) fn window(&mut self, c: &mut Commands, id: u32, stack: &Stack, pos: Option<Vec2>, front: bool) -> Option<Entity> {
        let def = self.db.items.get(&stack.item)?;
        let layout = def.layout.as_ref()?;
        let grey = Color::linear_rgb(0.75, 0.75, 0.75);
        let p = pos.unwrap_or_default();
        let root = c
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(p.x),
                    top: Val::Px(p.y),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect { left: Val::Px(5.0), right: Val::Px(5.0), top: Val::Px(5.0), bottom: Val::Px(10.0) },
                    ..default()
                },
                BackgroundColor(Color::linear_rgba(0.005, 0.005, 0.005, 0.95)),
                GlobalZIndex(Z + front as i32),
                if pos.is_some() { Visibility::Inherited } else { Visibility::Hidden },
                Panel,
                WindowNode(id),
            ))
            .observe(press_window)
            .id();
        let title = c
            .spawn((
                Node { align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(2.0), Val::Px(0.0)), margin: UiRect::all(Val::Px(1.0)), ..default() },
                BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.0, 0.75)),
                WindowTitle(id),
                ChildOf(root),
            ))
            .observe(drag_window)
            .id();
        c.spawn((label_colored(self.font, def.name.clone(), 16.0, grey), Node { flex_grow: 1.0, margin: UiRect::left(Val::Px(10.0)), ..default() }, Pickable::IGNORE, ChildOf(title)));
        c.spawn((
            Node { margin: UiRect::all(Val::Px(10.0)), padding: UiRect::axes(Val::Px(7.0), Val::Px(1.0)), ..default() },
            BackgroundColor(Color::linear_rgb(0.765625, 0.0, 0.0)),
            Hovered::default(),
            WindowClose(id),
            ChildOf(title),
            children![(label_colored(self.font, "X", 14.0, Color::WHITE), Pickable::IGNORE)],
        ));
        let body = self.layout(c, layout, &stack.contents, &|i| Loc::Window(id, i as u8));
        let vbox = c.spawn((Node { padding: UiRect { left: Val::Px(5.0), right: Val::Px(5.0), top: Val::Px(5.0), bottom: Val::Px(10.0) }, align_self: AlignSelf::Center, ..default() }, ChildOf(root))).id();
        c.entity(vbox).add_child(body);
        Some(root)
    }
}

/// A press anywhere on a window: SetActiveWindow.
fn press_window(ev: On<PointerPress>, ids: Query<&WindowNode>, mut windows: ResMut<ItemWindows>) {
    let Ok(WindowNode(id)) = ids.get(ev.event_target()) else { return };
    if let Some(i) = windows.windows.iter().position(|w| w.id == *id)
        && i + 1 != windows.windows.len()
    {
        let w = windows.windows.remove(i);
        windows.windows.push(w);
    }
}

/// Dragging the title moves its window.
fn drag_window(ev: On<PointerDrag>, ids: Query<&WindowTitle>, scale: Res<UiScale>, mut windows: ResMut<ItemWindows>, mut nodes: Query<(&WindowNode, &mut Node)>) {
    if ev.button != PointerButton::Primary {
        return;
    }
    let Ok(WindowTitle(id)) = ids.get(ev.event_target()) else { return };
    let Some(w) = windows.windows.iter_mut().find(|w| w.id == *id) else { return };
    let Some(pos) = w.pos.as_mut() else { return };
    *pos += ev.delta / scale.0.max(0.01);
    for (n, mut node) in &mut nodes {
        if n.0 == *id {
            node.left = Val::Px(pos.x);
            node.top = Val::Px(pos.y);
        }
    }
}

/// GetPos once a new window is laid out, then the front one over the rest.
pub(super) fn place_windows(
    mut windows: ResMut<ItemWindows>,
    viewport: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    mut nodes: Query<(&WindowNode, &mut Node, &ComputedNode, &mut Visibility, &mut GlobalZIndex)>,
) {
    let Ok(view) = viewport.single() else { return };
    let size = view.size() / scale.0.max(0.01);
    let front = windows.windows.last().map(|w| w.id);
    for (n, mut node, computed, mut vis, mut z) in &mut nodes {
        z.set_if_neq(GlobalZIndex(Z + (Some(n.0) == front) as i32));
        let Some(w) = windows.bypass_change_detection().windows.iter_mut().find(|w| w.id == n.0) else { continue };
        if w.pos.is_some() {
            continue;
        }
        let own = computed.size() * computed.inverse_scale_factor();
        if own.x <= 0.0 {
            continue;
        }
        let p = fit(w.at, own, size);
        w.pos = Some(p);
        node.left = Val::Px(p.x);
        node.top = Val::Px(p.y);
        *vis = Visibility::Inherited;
        info!("inventory: {} window {} at {p:.0} ({own:.0})", w.item, w.id);
    }
}

impl InvData<'_> {
    /// JSI_Slot.On_MouseDoubleClick (and Open): the item's window, or the inspector.
    pub(super) fn double_click(&mut self, r: ItemRef, cursor: Vec2) {
        let no_bag = no_bag_in_bag(&self.store);
        let (stack, owner) = {
            let (mut ctx, _) = self.ctx(Vec3::ZERO);
            (ctx.peek(r).map(|p| p.0), ctx.owner(r))
        };
        let Some(stack) = stack else { return };
        let Some(def) = self.db.items.get(&stack.item) else { return };
        let open = owner.is_some_and(|o| self.item_windows.showing(o));
        match (opens(def, r.loc, open, no_bag), owner) {
            (Opens::Window, Some(owner)) => {
                let id = self.item_windows.open(owner, &stack.item, cursor);
                info!("inventory: opened {}'s {} in window {id}", stack.item, def.special.as_deref().unwrap_or("?"));
                self.dirty.0 = true;
            }
            (Opens::NotPorted(w), _) => info!("inventory: double click on {}: its {w} window isn't ported", stack.item),
            _ => {
                self.inspects.write(super::super::inspect::InspectItem(stack));
            }
        }
    }

    /// CloseBtn: RemoveFromParent.
    pub(super) fn close_window(&mut self, id: u32) {
        self.item_windows.windows.retain(|w| w.id != id);
        self.dirty.0 = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Option<ItemDb> {
        serde_json::from_str(&std::fs::read_to_string("assets/data/items.json").ok()?).ok()
    }

    /// A searched crate (key 1) holding a Lunch Box at 0,0 with a Wallet inside at 0,0.
    fn crate_with_bags(db: &ItemDb) -> Containers {
        let mut wallet = db.stack("DA_Wallet", 1);
        wallet.contents[0].place_at(db, db.stack("DA_CannedBakedBeans", 1), 0, 0).unwrap();
        let mut lunch = db.stack("DA_LunchBox", 1);
        lunch.contents[0].place_at(db, wallet, 0, 0).unwrap();
        let mut loot = Grid::new("Crate", 10, 6);
        loot.place_at(db, lunch, 0, 0).unwrap();
        loot.place_at(db, db.stack("DA_WaterBottle", 1), 5, 0).unwrap();
        let mut containers = Containers::default();
        containers.searched.insert(1, loot);
        containers
    }

    fn with(db: &ItemDb, containers: &mut Containers, windows: &[ItemWindow], f: impl FnOnce(&mut Ctx)) -> CurrentGame {
        let (mut game, mut ground, view, craft) = (CurrentGame::default(), Ground::default(), VicinityView::default(), CraftView::default());
        let mut ctx = Ctx { db, game: &mut game, containers, ground: &mut ground, view: &view, craft: &craft, player: Vec3::ZERO, xp_mult: 1.0, notice: None, windows };
        f(&mut ctx);
        game
    }

    #[test]
    fn storage_items_open_windows_else_inspect() {
        let Some(db) = db() else { return };
        let bag = &db.items["DA_AliceBackpack"];
        let pocket = Loc::Player(GridRef::Pockets);
        assert_eq!(opens(bag, pocket, false, false), Opens::Window);
        assert_eq!(opens(bag, Loc::Loot(1), false, false), Opens::Window);
        // Already up, worn (its widget is the INVENTORY column's), in a trader's stock, or a
        // backpack with DISABLE BACKPACK IN BACKPACK?: the inspector.
        assert_eq!(opens(bag, pocket, true, false), Opens::Inspect);
        assert_eq!(opens(bag, Loc::Equip(Slot::Backpack), false, false), Opens::Inspect);
        assert_eq!(opens(bag, Loc::Loot(traders::TRADER_KEYS), false, false), Opens::Inspect);
        assert_eq!(opens(bag, pocket, false, true), Opens::Inspect);
        // Only backpacks: a wallet still opens.
        assert_eq!(opens(&db.items["DA_Wallet"], pocket, false, true), Opens::Window);
        assert!(!offers_open(bag, pocket, true) && offers_open(&db.items["DA_Wallet"], pocket, true) && !offers_open(bag, Loc::Equip(Slot::Backpack), false));
        // No special container: inspect; a gun's slot panel isn't ported.
        assert_eq!(opens(&db.items["DA_WaterBottle"], pocket, false, false), Opens::Inspect);
        assert!(matches!(opens(&db.items["DA_M4CQB"], pocket, false, false), Opens::NotPorted(w) if w.starts_with("SC_")));
    }

    #[test]
    fn every_storage_layout_draws_each_grid_once() {
        let Some(db) = db() else { return };
        fn grids(l: &Layout, out: &mut Vec<usize>) {
            out.extend(l.grid);
            for k in l.v.iter().chain(l.h.iter()).flatten() {
                grids(k, out);
            }
        }
        let mut n = 0;
        for (key, def) in &db.items {
            if def.grids.is_empty() {
                continue;
            }
            let mut got = Vec::new();
            grids(def.layout.as_ref().unwrap_or_else(|| panic!("{key} has grids but no layout")), &mut got);
            got.sort();
            assert_eq!(got, (0..def.grids.len()).collect::<Vec<_>>(), "{key}");
            n += 1;
        }
        assert!(n > 50);
        // The Coyote Backpack draws its 6x8 main compartment (grid 0) between the 2x3 pockets.
        let coyote = db.items["DA_CoyoteBackpack"].layout.as_ref().unwrap();
        assert_eq!(coyote.h.as_ref().unwrap().iter().filter_map(|l| l.grid).collect::<Vec<_>>(), vec![1, 0, 2]);
    }

    #[test]
    fn window_grids_are_the_items_contents_and_nest() {
        let Some(db) = db() else { return };
        let mut containers = crate_with_bags(&db);
        let lunch = ItemWindow { id: 0, owner: Owner::Cell { loc: Loc::Loot(1), x: 0, y: 0 }, item: "DA_LunchBox".into(), pos: None, at: Vec2::ZERO };
        let wallet = ItemWindow { id: 1, owner: Owner::Cell { loc: Loc::Window(0, 0), x: 0, y: 0 }, item: "DA_Wallet".into(), pos: None, at: Vec2::ZERO };
        let windows = [lunch, wallet];
        with(&db, &mut containers, &windows, |ctx| {
            assert_eq!(ctx.owner(ItemRef { loc: Loc::Loot(1), index: 0 }), Some(windows[0].owner));
            assert_eq!(ctx.grid(Loc::Window(1, 0)).map(|g| g.items[0].stack.item.clone()).as_deref(), Some("DA_CannedBakedBeans"));
            // The water bottle dropped into the lunch box's window, the beans out of the
            // wallet's into the lunch box's.
            ctx.drop_onto_cell(ItemRef { loc: Loc::Loot(1), index: 1 }, CellRef { loc: Loc::Window(0, 0), x: 3, y: 0 }, false);
            ctx.drop_onto_cell(ItemRef { loc: Loc::Window(1, 0), index: 0 }, CellRef { loc: Loc::Window(0, 0), x: 5, y: 3 }, false);
            // The lunch box into its own window: refused, it stays put.
            ctx.drop_onto_cell(ItemRef { loc: Loc::Loot(1), index: 0 }, CellRef { loc: Loc::Window(0, 0), x: 3, y: 2 }, false);
        });
        let crate_grid = &containers.searched[&1];
        assert_eq!(crate_grid.items.len(), 1);
        let lunch = &crate_grid.items[0];
        assert_eq!((lunch.stack.item.as_str(), lunch.x, lunch.y), ("DA_LunchBox", 0, 0));
        let inside: Vec<&str> = lunch.stack.contents[0].items.iter().map(|p| p.stack.item.as_str()).collect();
        assert_eq!(inside, ["DA_Wallet", "DA_WaterBottle", "DA_CannedBakedBeans"]);
        assert!(lunch.stack.contents[0].items[0].stack.contents[0].items.is_empty());
    }

    #[test]
    fn windows_close_when_their_item_moves() {
        let Some(db) = db() else { return };
        let mut containers = crate_with_bags(&db);
        let lunch = ItemWindow { id: 0, owner: Owner::Cell { loc: Loc::Loot(1), x: 0, y: 0 }, item: "DA_LunchBox".into(), pos: None, at: Vec2::ZERO };
        let wallet = ItemWindow { id: 1, owner: Owner::Cell { loc: Loc::Window(0, 0), x: 0, y: 0 }, item: "DA_Wallet".into(), pos: None, at: Vec2::ZERO };
        let windows = [lunch, wallet];
        with(&db, &mut containers, &windows, |ctx| {
            assert!(ctx.window_stack(0).is_some() && ctx.window_stack(1).is_some());
            // The lunch box moved along the crate: its window and the wallet's inside it lose it.
            ctx.drop_onto_cell(ItemRef { loc: Loc::Loot(1), index: 0 }, CellRef { loc: Loc::Loot(1), x: 0, y: 3 }, false);
            assert!(ctx.window_stack(0).is_none() && ctx.window_stack(1).is_none());
            assert!(ctx.grid(Loc::Window(1, 0)).is_none());
        });
        // Something else in its cell isn't it either.
        let mut containers = crate_with_bags(&db);
        let other = [ItemWindow { id: 0, owner: Owner::Cell { loc: Loc::Loot(1), x: 5, y: 0 }, item: "DA_LunchBox".into(), pos: None, at: Vec2::ZERO }];
        with(&db, &mut containers, &other, |ctx| assert!(ctx.window_stack(0).is_none()));
    }

    #[test]
    fn get_pos_centres_on_the_mouse_inside_the_viewport() {
        let view = Vec2::new(1920.0, 1080.0);
        let size = Vec2::new(400.0, 300.0);
        assert_eq!(fit(Vec2::new(960.0, 540.0), size, view), Vec2::new(760.0, 390.0));
        assert_eq!(fit(Vec2::new(10.0, 1070.0), size, view), Vec2::new(0.0, 780.0));
        assert_eq!(fit(Vec2::new(1900.0, 20.0), size, view), Vec2::new(1520.0, 0.0));
    }
}
