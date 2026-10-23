//! Traders: BP_TraderMaster's children placed in the level (`assets/data/traders.json`, from
//! `extractor/gen_traders.py`).
//!
//! - Each trader is its class's SkeletalMesh (AnimBP_Trader: A_Idle) with a random hair and
//!   beard from HairMeshes / BeardMeshes on the `head` bone (UserConstructionScript), shown
//!   within its AIOSubject's last layer (350 m).
//! - F (OnServerExecuteInteract) begins its Narrative dialogue: the NPC's opening line (one of
//!   its alternatives) and the player's replies; the reply whose NPC node fires
//!   Event_OpenNPCVendor ("What are you selling?") opens the trade screen, the other says
//!   goodbye. While talking or trading the trader turns to face the survivor (FacePlayer:
//!   RInterpTo at 5).
//! - The stock is its BP_JigMultiplayer container (13 columns, the class's rows) filled from
//!   its DT_VendorTable row (SetItemsToSell) like a container's StartingItems, and rolled again
//!   every day at 10:00 (Event_Reset: ClearContainer with ResetInit), when the trader's money
//!   is rolled too.
//! - Trading is moving items between that grid and the survivor's (`inventory_ui.rs`), settled
//!   by [`settle`]: BP_JigMultiplayer's Handle Comp to Comp Move. Buying costs the items'
//!   GetAllItemsSellPrice (count x RepItemInfo.Price, scaled by durability) in the trader's
//!   VendorAcceptedCurrencyID (Money, or Credits); selling pays the same into the inventory
//!   (VendorTryStackMoney, else a free slot: "No empty space to receive currency"). Traders
//!   with AllowSellingItemsToVendor? off refuse ("This trader is not accepting items"), worn
//!   containers must be empty ("Items in container"), and only VendorReSellSoldItems? traders
//!   keep what they buy, priced with its attachments (SetVendorPriceOfSoldItem).
//! - With TRADERS HAVE A SET AMOUNT OF CURRENCY ENABLED? (Difficulty.TraderCurrencyAmount) a
//!   trader has VendorMaxCurrencyAmount's random 300-600: selling needs that much ("Trader does
//!   not have enough money") and buying adds to it. Off (the default) its money never changes
//!   and the screen shows the component's 500.
//!
//! - BP_MilitaryTraderSpawner and BP_ShakesConesSpawner put their trader at one of their five
//!   Spawn0..4 points, picked at random each time the world loads.
//! - Wandering traders (BP_MasterTrader) are human AI (`humans.rs`, friendly like the
//!   scavengers) that the level's BP_AISpawningVolume_RandomChanceToSpawn volumes spawn
//!   (`level::spawners`). Each one met becomes a trader that follows its body: "Press [F] to
//!   Trade with Wandering Trader" begins Dialogue_WanderingTrader, it stands still while talking
//!   or trading (DisableMovement), and its stock is WanderingTraderEquipment (BeginPlay's
//!   SetItemsToSell(22)), rolled once. Killed, it gives "Wandering Trader Killed, XP Given: N"
//!   and its loot is gone (ClearContainer; JigCanInteract is off): searching the body says "All
//!   of the trader's loot is destroyed...".
//!
//! Not ported: the vehicle trader's W_VehicleTraderUI, the outline (Video.Outline), the 3 m
//! CheckDistanceFromActor (gameplay input is paused while trading), collision with the traders'
//! bodies, saving their stock, the wandering trader's map marker (AddMarker).

use std::collections::HashMap;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use rand::Rng;
use serde::Deserialize;

use super::equipment::PlayerInventory;
use super::interact::{Containers, GameOverlay};
use super::items::{Grid, ItemDb, LootRow, Placed, Stack};
use crate::menus::widgets::{ButtonPalette, MenuFont, label_colored, pt};
use crate::menus::{AppState, CurrentGame};
use crate::models::{AnimSet, Animated, CharacterModel, Locomotion, model, ue_relative};

/// Container keys for traders (`interact::Containers`) start above the corpses'.
pub const TRADER_KEYS: usize = 1 << 33;
/// AIOSubject OptimizationLayers' last LayerRadius (35000 cm): hidden beyond it.
const SHOWN_WITHIN: f32 = 350.0;
/// Event_Reset: the hour the stock and money are rolled again.
const RESTOCK_HOUR: u32 = 10;
/// FacePlayer's RInterpTo speed.
const FACE_SPEED: f32 = 5.0;
const SET_CURRENCY: &str = "TRADERS HAVE A SET AMOUNT OF CURRENCY ENABLED?";
/// Classes left out: BP_VehicleTrader sells vehicles through W_VehicleTraderUI (no vehicles yet).
const SKIPPED: [&str; 1] = ["BP_VehicleTrader_C"];

#[derive(Deserialize, Clone, Debug)]
pub struct Reply {
    pub text: String,
    /// Leads to the NPC node that opens the trade screen.
    pub trade: bool,
    /// The NPC nodes it leads to (Narrative takes the first whose conditions pass).
    #[serde(default)]
    pub branches: Vec<Branch>,
}

/// A Narrative NPC node: its line (none for a silent node), the conditions picking it, the events
/// it fires, the NPC nodes that follow.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Branch {
    pub line: Option<String>,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub next: Vec<Branch>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Condition {
    pub class: String,
    #[serde(default)]
    pub not: bool,
    pub price: Option<u32>,
    pub currency: Option<String>,
}

/// What a reply's NPC nodes did: the events fired and the lines to say, in order.
#[derive(Debug, Default, PartialEq)]
pub struct Said {
    pub events: Vec<String>,
    pub lines: Vec<String>,
}

/// Runs a reply's NPC nodes: the first whose conditions all pass is taken, its events fire and
/// its line is said, then the same for the nodes after it. `money(price, currency)` is
/// Condition_CheckMoney's CheckIfHasEnoughMoney, which also takes the money when there is
/// enough (TakeMoney) before bNot flips the answer.
pub fn run_branches(branches: &[Branch], money: &mut impl FnMut(u32, &str) -> bool) -> Said {
    fn walk(list: &[Branch], money: &mut dyn FnMut(u32, &str) -> bool, out: &mut Said) {
        for b in list {
            let pass = b.conditions.iter().all(|c| {
                let r = match (c.class.as_str(), c.price) {
                    ("Condition_CheckMoney", Some(p)) => money(p, c.currency.as_deref().unwrap_or("DA_Money")),
                    _ => true,
                };
                r != c.not
            });
            if pass {
                out.events.extend(b.events.iter().cloned());
                out.lines.extend(b.line.iter().cloned());
                walk(&b.next, money, out);
                return;
            }
        }
    }
    let mut out = Said::default();
    walk(branches, money, &mut out);
    out
}

/// Narrative's line duration: LettersPerSecondLineDuration 25, at least MinDialogueTextDisplayTime 2 s.
pub fn line_secs(line: &str) -> f32 {
    (line.chars().count() as f32 / 25.0).max(2.0)
}

#[derive(Deserialize, Clone, Debug)]
pub struct Dialogue {
    pub speaker: Option<String>,
    pub lines: Vec<String>,
    pub replies: Vec<Reply>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TraderClass {
    pub name: String,
    pub table: Option<String>,
    pub mesh: Option<String>,
    #[serde(default)]
    pub hair: Vec<Option<String>>,
    #[serde(default)]
    pub beard: Vec<Option<String>>,
    pub grid: [u8; 2],
    pub currency: Option<String>,
    pub pool: [f32; 2],
    pub amount: f32,
    pub selling: bool,
    pub resell: bool,
    pub dialogue: Option<Dialogue>,
    /// BP_MasterTrader: an AI that walks around (its body is the human AI's).
    #[serde(default)]
    pub wandering: bool,
    /// BP_QuestGiver: no stock, no FacePlayer; F begins its dialogue and completes
    /// InteractWithActor with its `argument`.
    #[serde(default)]
    pub quest_giver: bool,
    #[serde(default)]
    pub argument: String,
    /// Its Gun component (on the Sidearm socket).
    #[serde(default)]
    pub gun: Option<String>,
}

#[derive(Resource, Deserialize, Default, Debug)]
pub struct TraderDb {
    pub classes: HashMap<String, TraderClass>,
    pub tables: HashMap<String, Vec<LootRow>>,
}

impl TraderDb {
    pub fn load() -> Self {
        std::fs::read_to_string("assets/data/traders.json").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| {
            warn!("assets/data/traders.json missing or malformed (extractor/export_assets.sh --items)");
            Self::default()
        })
    }

    /// A fresh stock: the class's table rolled into its grid.
    pub fn stock(&self, db: &ItemDb, class: &TraderClass, rng: &mut impl Rng) -> Grid {
        let mut g = Grid::new(class.name.clone(), class.grid[0], class.grid[1]);
        let rows = class.table.as_ref().and_then(|t| self.tables.get(t)).map(Vec::as_slice).unwrap_or_default();
        for s in db.roll_rows(rows, rng) {
            super::interact::insert_growing(db, &mut g, s);
        }
        g
    }
}

/// A trader in the level.
pub struct Trader {
    pub class: String,
    pub transform: Transform,
    /// VendorCurrentCurrencyAmount.
    pub money: f32,
    /// The body (a placed trader's skeletal mesh, a wandering trader's AI).
    pub entity: Option<Entity>,
    /// A wandering trader: the spawner group its body came from, and whether it was killed.
    pub spawner: Option<usize>,
    pub dead: bool,
}

#[derive(Resource, Default)]
pub struct Traders {
    pub list: Vec<Trader>,
    collected: bool,
    /// The day the stock was last rolled at 10:00.
    restocked: Option<u32>,
    /// The opening line of the dialogue being shown.
    line: String,
}

/// The NPC's lines after a reply (Narrative plays them in turn; the dialogue ends after the last).
#[derive(Resource, Default)]
pub struct FollowUp {
    pub lines: std::collections::VecDeque<String>,
}

impl Traders {
    /// A trader standing at `transform` (placed in the level or by a spawned prefab): its trade
    /// grid rolled, its money. Gives its container key.
    pub fn place(&mut self, tdb: &TraderDb, db: &ItemDb, limited: bool, containers: &mut Containers, class_name: &str, transform: Transform) -> Option<usize> {
        let class = tdb.classes.get(class_name).filter(|_| !SKIPPED.contains(&class_name))?;
        let mut rng = rand::thread_rng();
        let key = TRADER_KEYS + self.list.len();
        containers.list.push((key, transform.translation, class_name.to_string()));
        if !class.quest_giver {
            containers.searched.insert(key, tdb.stock(db, class, &mut rng));
        }
        self.list.push(Trader { class: class_name.to_string(), transform, money: starting_money(class, limited, &mut rng), entity: None, spawner: None, dead: false });
        Some(key)
    }

    /// The trader behind a container key, with its class.
    pub fn get<'a>(&'a self, db: &'a TraderDb, key: usize) -> Option<(&'a Trader, &'a TraderClass)> {
        let t = self.list.get(key.checked_sub(TRADER_KEYS)?)?;
        Some((t, db.classes.get(&t.class)?))
    }
}

pub fn is_trader(key: usize) -> bool {
    key >= TRADER_KEYS
}

/// The trader's money as W_TraderUI's CurrencyText shows it: "Currency: {Value}{$} " with
/// Value = "$" (Money) or " Cr" (Credits) and $ = the amount.
pub fn currency_text(class: &TraderClass, money: f32) -> String {
    let unit = if class.currency.as_deref() == Some("DA_Credits") { " Cr" } else { "$" };
    format!("Currency: {unit}{} ", grouped(money))
}

/// JSI_Slot's PriceCount: "{Price}{Cur}" (Cur "$", or " Cr" for Credits).
pub fn price_tag(class: &TraderClass, price: f32) -> String {
    let unit = if class.currency.as_deref() == Some("DA_Credits") { " Cr" } else { "$" };
    format!("{}{unit}", grouped(price))
}

/// Conv_DoubleToText with grouping (and no fraction for whole numbers).
fn grouped(v: f32) -> String {
    let whole = v.trunc() as i64;
    let digits = whole.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if whole < 0 {
        out.insert(0, '-');
    }
    let frac = ((v - v.trunc()).abs() * 10.0).round() as u32;
    if frac > 0 && frac < 10 {
        out.push_str(&format!(".{frac}"));
    }
    out
}

/// JSI_Slot.GetPrice for one item: RepItemInfo.Price, times Durability / 100 (truncated) when
/// the item has durability.
pub fn unit_price(db: &ItemDb, s: &Stack) -> f32 {
    let Some(def) = db.items.get(&s.item) else { return 0.0 };
    let price = s.price.unwrap_or(def.price);
    if def.durability <= 0.0 {
        return price;
    }
    (price * s.durability.unwrap_or(def.durability) / 100.0).trunc()
}

/// GetAllItemsSellPrice for a whole stack.
pub fn stack_price(db: &ItemDb, s: &Stack) -> f32 {
    s.count as f32 * unit_price(db, s)
}

/// SetVendorPriceOfSoldItem: an item the trader bought resells at its price plus its
/// attachments'.
fn resale_price(db: &ItemDb, s: &Stack) -> f32 {
    let own = s.price.unwrap_or_else(|| db.items.get(&s.item).map_or(0.0, |d| d.price));
    own + s.attachments.iter().map(|a| a.count as f32 * db.items.get(&a.item).map_or(0.0, |d| d.price)).sum::<f32>()
}

/// What a trader's grid lost to the survivor and gained from them over one inventory action.
#[derive(Debug, Default, PartialEq)]
pub struct Exchange {
    /// Paid by the survivor (bought).
    pub cost: f32,
    /// Paid to the survivor (sold).
    pub earned: f32,
    /// Stacks the survivor sold: (index in the new grid, how many of it are theirs).
    pub sold: Vec<(usize, u32)>,
}

/// Compare a trader's grid before and after an action. Identical stacks match up (moving within
/// the grid is free); for stackable items only the change in count counts.
pub fn exchange(db: &ItemDb, before: &[Placed], after: &[Placed]) -> Exchange {
    let mut left: Vec<Option<&Stack>> = before.iter().map(|p| Some(&p.stack)).collect();
    let mut gained = Vec::new();
    for (i, p) in after.iter().enumerate() {
        match left.iter().position(|b| b.is_some_and(|b| *b == p.stack)) {
            Some(j) => left[j] = None,
            None => gained.push(i),
        }
    }
    let lost: Vec<&Stack> = left.into_iter().flatten().collect();
    let stackable = |item: &str| db.items.get(item).is_some_and(|d| d.is_stackable());
    let mut out = Exchange::default();
    // Stackables: the net change per item.
    let mut net: HashMap<&str, i64> = HashMap::new();
    for s in &lost {
        if stackable(&s.item) {
            *net.entry(&s.item).or_default() -= s.count as i64;
        } else {
            out.cost += stack_price(db, s);
        }
    }
    for &i in &gained {
        let s = &after[i].stack;
        if stackable(&s.item) {
            *net.entry(&s.item).or_default() += s.count as i64;
        }
    }
    for (item, n) in net {
        let unit = unit_price(db, &db.stack(item, 1));
        if n < 0 {
            out.cost += -n as f32 * unit;
        } else if n > 0 {
            out.earned += n as f32 * unit;
            // Credit the newest stacks of it first (the ones the drop made or topped up).
            let mut rest = n as u32;
            for &i in gained.iter().rev().filter(|&&i| after[i].stack.item == item) {
                let take = after[i].stack.count.min(rest);
                out.sold.push((i, take));
                rest -= take;
                if rest == 0 {
                    break;
                }
            }
        }
    }
    for &i in &gained {
        let s = &after[i].stack;
        if !stackable(&s.item) {
            out.earned += stack_price(db, s);
            out.sold.push((i, s.count));
        }
    }
    out.cost = out.cost.trunc();
    out.earned = out.earned.trunc();
    out
}

/// Everything settling a trade touches besides the trader's grid.
pub struct Deal<'a> {
    pub class: &'a TraderClass,
    /// VendorCurrentCurrencyAmount.
    pub money: &'a mut f32,
    /// TRADERS HAVE A SET AMOUNT OF CURRENCY ENABLED?
    pub limited: bool,
}

/// Settle an inventory action against a trader (Handle Comp to Comp Move): charge or pay the
/// survivor, keep or drop what the trader bought. `Err` refuses the whole action with the
/// game's message; the caller puts everything back.
pub fn settle(db: &ItemDb, deal: Deal, before: &[Placed], grid: &mut Grid, inv: &mut PlayerInventory) -> Result<(), &'static str> {
    let ex = exchange(db, before, &grid.items);
    if ex == Exchange::default() {
        return Ok(());
    }
    let currency = deal.class.currency.as_deref().unwrap_or("DA_Money");
    if !ex.sold.is_empty() {
        if !deal.class.selling {
            return Err("This trader is not accepting items");
        }
        // Money isn't sold for money (ours: the game would take it at its price of 1).
        if ex.sold.iter().any(|(i, _)| grid.items[*i].stack.item == currency) {
            return Err("");
        }
        // Worn containers only go empty (Jig_UnhandledEvent's Backpack/BodyArmor/Container check).
        if ex.sold.iter().any(|(i, _)| grid.items[*i].stack.contents.iter().any(|g| !g.items.is_empty())) {
            return Err("Items in container");
        }
    }
    // CheckIfHasEnoughMoney (its PrintString "Not enough money!" is a development message; shown
    // here as a toast, ours).
    if ex.cost > ex.earned && inv.count(currency) < (ex.cost - ex.earned) as u32 {
        return Err("Not enough money!");
    }
    // CheckVendorAmountOfMoney: only with a set amount, and only the trader's spending.
    if deal.limited && ex.earned > *deal.money + ex.cost {
        return Err("Trader does not have enough money");
    }
    let pay = (ex.cost - ex.earned).max(0.0) as u32;
    let get = (ex.earned - ex.cost).max(0.0) as u32;
    if get > 0 && inv.clone().insert(db, db.stack(currency, get)).is_some() {
        return Err("No empty space to receive currency");
    }
    inv.remove(currency, pay);
    if get > 0 {
        inv.insert(db, db.stack(currency, get));
    }
    if deal.limited {
        *deal.money += ex.cost - ex.earned;
    }
    // What the trader bought: kept for resale at its price with attachments, or gone.
    let mut gone: Vec<(usize, u32)> = Vec::new();
    for (i, n) in ex.sold {
        if deal.class.resell {
            let s = &mut grid.items[i].stack;
            if s.attachments.is_empty() {
                continue;
            }
            s.price = Some(resale_price(db, s));
        } else {
            gone.push((i, n));
        }
    }
    gone.sort_by_key(|g| std::cmp::Reverse(g.0));
    for (i, n) in gone {
        let s = &mut grid.items[i].stack;
        s.count -= n.min(s.count);
        if s.count == 0 {
            grid.items.remove(i);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- the world side

pub fn plugin(app: &mut App) {
    app.insert_resource(TraderDb::load())
        .init_resource::<Traders>()
        .init_resource::<FollowUp>()
        .add_systems(OnEnter(AppState::InGame), |mut t: ResMut<Traders>| *t = Traders::default())
        .add_systems(Update, (collect, wanderers, spawn_bodies, attach_hair, attach_gun, show_near, restock, face_player, follow_up, dialogue_ui).chain().run_if(in_state(AppState::InGame)))
        .add_observer(on_reply);
}

pub fn limited(store: &crate::menus::settings::SettingsStore) -> bool {
    use crate::menus::settings::{Value, row_key};
    store.find(&row_key("difficulty", "General", SET_CURRENCY)) == Some(Value::Bool(true))
}

/// BeginPlay: the money a trader starts with (a random VendorMaxCurrencyAmount when the amount
/// is set, else the component's own).
fn starting_money(class: &TraderClass, limited: bool, rng: &mut impl Rng) -> f32 {
    if limited && class.pool[1] >= class.pool[0] { rng.gen_range(class.pool[0]..=class.pool[1]).round() } else { class.amount }
}

#[allow(clippy::too_many_arguments)]
fn collect(
    level: Option<Res<crate::level::Level>>,
    tdb: Res<TraderDb>,
    db: Res<ItemDb>,
    store: Res<crate::menus::settings::SettingsStore>,
    mut traders: ResMut<Traders>,
    mut containers: ResMut<Containers>,
) {
    let Some(level) = level else { return };
    if traders.collected {
        return;
    }
    traders.collected = true;
    let mut rng = rand::thread_rng();
    let limited = limited(&store);
    for a in &level.actors {
        if tdb.classes.get(&a.class).is_some_and(|c| !c.wandering) {
            let t = crate::level::affine(&a.transform);
            traders.place(&tdb, &db, limited, &mut containers, &a.class, Transform::from_translation(t.translation).with_rotation(t.rotation));
        }
    }
    // BP_MilitaryTraderSpawner / BP_ShakesConesSpawner: SpawnActor at a random Spawn0..4.
    for a in &level.actors {
        let Some((_, trader)) = SPAWNED.iter().find(|(s, _)| *s == a.class) else { continue };
        let (rel, yaw) = crate::level::item_spawners::point(a, &format!("Spawn{}", rng.gen_range(0..5)), Vec3::ZERO);
        let at = crate::level::item_spawners::component_world(a, rel, yaw);
        if traders.place(&tdb, &db, limited, &mut containers, trader, at).is_some() {
            info!("trader spawner {}: {trader} at {:.0}", a.class, at.translation);
        }
    }
    info!("traders: {}", traders.list.len());
}

/// The trader spawners and what they spawn.
const SPAWNED: [(&str, &str); 2] = [("BP_MilitaryTraderSpawner_C", "BP_MilitaryTrader_C"), ("BP_ShakesConesSpawner_C", "BP_ShakesConesTrader_C")];

/// BP_MasterTrader's DoOnce notification when its dead body is searched.
pub const LOOT_DESTROYED: &str = "All of the trader's loot is destroyed...";

/// Wandering traders: a trader for each BP_MasterTrader body met (its trade grid and money),
/// following it; holding it still while it talks or trades; gone when it dies.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn wanderers(
    mut commands: Commands,
    tdb: Res<TraderDb>,
    db: Res<ItemDb>,
    store: Res<crate::menus::settings::SettingsStore>,
    mut overlay: ResMut<GameOverlay>,
    mut traders: ResMut<Traders>,
    mut containers: ResMut<Containers>,
    bodies: Query<(Entity, &Transform, &crate::level::LevelZombie, Has<super::zombies::HeldStill>), Without<super::combat::Dead>>,
    died: Query<Entity, (With<crate::level::LevelZombie>, Added<super::combat::Dead>)>,
) {
    for e in &died {
        let Some(i) = traders.list.iter().position(|t| !t.dead && t.entity == Some(e)) else { continue };
        traders.list[i].dead = true;
        let key = TRADER_KEYS + i;
        containers.list.retain(|c| c.0 != key);
        containers.searched.remove(&key);
        if matches!(*overlay, GameOverlay::Loot(k) | GameOverlay::Dialogue(k) if k == key) {
            *overlay = GameOverlay::None;
        }
    }
    for (e, t, z, held) in &bodies {
        if !z.kind.human().is_some_and(|h| h.trader) {
            continue;
        }
        let class_name = z.kind.class_name();
        let i = match traders.list.iter().position(|tr| !tr.dead && (tr.entity == Some(e) || z.spawner.is_some() && tr.spawner == z.spawner)) {
            Some(i) => i,
            None => {
                let Some(class) = tdb.classes.get(class_name) else { continue };
                let mut rng = rand::thread_rng();
                let key = TRADER_KEYS + traders.list.len();
                containers.list.push((key, t.translation, class_name.to_string()));
                containers.searched.insert(key, tdb.stock(&db, class, &mut rng));
                let money = starting_money(class, limited(&store), &mut rng);
                traders.list.push(Trader { class: class_name.to_string(), transform: *t, money, entity: Some(e), spawner: z.spawner, dead: false });
                info!("wandering trader met at {:.0}", t.translation);
                traders.list.len() - 1
            }
        };
        let key = TRADER_KEYS + i;
        let tr = &mut traders.list[i];
        // A body the optimiser brought back is a new entity.
        tr.entity = Some(e);
        tr.transform.translation = t.translation;
        if let Some(c) = containers.list.iter_mut().find(|c| c.0 == key) {
            c.1 = t.translation;
        }
        let busy = matches!(*overlay, GameOverlay::Loot(k) | GameOverlay::Dialogue(k) if k == key);
        if busy && !held {
            commands.entity(e).insert(super::zombies::HeldStill);
        } else if !busy && held {
            commands.entity(e).remove::<super::zombies::HeldStill>();
        }
    }
}

/// The interaction prompt: BP_MasterTrader's Action Name, "talk to" for the others.
pub fn prompt(class: &TraderClass) -> String {
    if class.wandering { format!("Press [F] to Trade with {}", class.name) } else { format!("Press [F] to talk to {}", class.name) }
}

/// The trader's skeletal mesh, a child of its actor so FacePlayer can turn the actor.
#[derive(Component)]
pub struct TraderBody(pub usize);

/// Hair or beard waiting for the body's `head` bone.
#[derive(Component)]
struct OnHead(Entity);

fn spawn_bodies(mut commands: Commands, assets: Res<AssetServer>, tdb: Res<TraderDb>, mut traders: ResMut<Traders>) {
    let mut rng = rand::thread_rng();
    for (i, t) in traders.list.iter_mut().enumerate().filter(|(_, t)| t.entity.is_none()) {
        let Some(class) = tdb.classes.get(&t.class) else { continue };
        let root = commands.spawn((t.transform, Visibility::Hidden, TraderBody(i), DespawnOnExit(AppState::InGame))).id();
        if let Some(mesh) = &class.mesh {
            // SkeletalMesh RelativeRotation yaw -90 (the mesh faces UE +Y).
            let body = commands
                .spawn((model(&assets, format!("game/{mesh}.glb")), Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)), Animated(AnimSet::Trader), Locomotion::Idle, ChildOf(root)))
                .id();
            for list in [&class.hair, &class.beard] {
                if list.is_empty() {
                    continue;
                }
                if let Some(m) = &list[rng.gen_range(0..list.len())] {
                    commands.spawn((model(&assets, format!("game/{m}.glb")), Visibility::Hidden, OnHead(body), ChildOf(root)));
                }
            }
            // BP_QuestGiver's Gun on the Sidearm socket.
            if let Some(g) = &class.gun {
                commands.spawn((model(&assets, format!("game/{g}.glb")), Visibility::Hidden, OnSidearm(body), ChildOf(root)));
            }
        }
        t.entity = Some(root);
    }
}

/// A quest giver's revolver waiting for the body's Sidearm socket.
#[derive(Component)]
struct OnSidearm(Entity);

fn attach_gun(mut commands: Commands, db: Res<ItemDb>, pending: Query<(Entity, &OnSidearm)>, children: Query<&Children>, names: Query<&Name>) {
    for (e, OnSidearm(body)) in &pending {
        let (bone, at) = db.sockets.get("Sidearm").map_or(("thigh_r", Transform::IDENTITY), |s| (s.bone.as_str(), s.transform.to_transform()));
        let Some(b) = children.iter_descendants(*body).find(|c| names.get(*c).is_ok_and(|n| n.as_str() == bone)) else { continue };
        commands.entity(e).remove::<OnSidearm>().insert((at, Visibility::Inherited));
        commands.entity(b).add_child(e);
    }
}

/// Hair and beard hang off the `head` bone at the components' relative rotation (P90 R-90).
fn attach_hair(mut commands: Commands, pending: Query<(Entity, &OnHead)>, children: Query<&Children>, names: Query<&Name>) {
    for (e, OnHead(body)) in &pending {
        let Some(bone) = children.iter_descendants(*body).find(|b| names.get(*b).is_ok_and(|n| n.as_str().eq_ignore_ascii_case("head"))) else { continue };
        commands.entity(e).remove::<OnHead>().insert((ue_relative([0.0; 3], 90.0, 0.0, -90.0), Visibility::Inherited));
        commands.entity(bone).add_child(e);
    }
}

fn show_near(traders: Res<Traders>, player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>, mut vis: Query<&mut Visibility, With<TraderBody>>) {
    let Ok(p) = player.single() else { return };
    for t in &traders.list {
        if let Some(mut v) = t.entity.and_then(|e| vis.get_mut(e).ok()) {
            let want = if t.transform.translation.distance(p.translation) <= SHOWN_WITHIN { Visibility::Inherited } else { Visibility::Hidden };
            v.set_if_neq(want);
        }
    }
}

/// Event_Reset at 10:00: a new stock and new money.
fn restock(game: Res<CurrentGame>, tdb: Res<TraderDb>, db: Res<ItemDb>, store: Res<crate::menus::settings::SettingsStore>, mut traders: ResMut<Traders>, mut containers: ResMut<Containers>) {
    let Some(state) = game.state.as_ref() else { return };
    let (hour, day) = (state.environment.time_of_day_hours as u32, state.environment.days);
    // The first day's stock is the one rolled when the world loaded.
    let last = *traders.restocked.get_or_insert(if hour >= RESTOCK_HOUR { day } else { day.saturating_sub(1) });
    if hour < RESTOCK_HOUR || last >= day {
        return;
    }
    traders.restocked = Some(day);
    let mut rng = rand::thread_rng();
    let limited = limited(&store);
    for (i, t) in traders.list.iter_mut().enumerate() {
        // Event_Reset is BP_TraderMaster's: the wandering traders keep their stock.
        let Some(class) = tdb.classes.get(&t.class).filter(|c| !c.wandering && !c.quest_giver) else { continue };
        containers.searched.insert(TRADER_KEYS + i, tdb.stock(&db, class, &mut rng));
        t.money = starting_money(class, limited, &mut rng);
    }
    info!("traders restocked (day {day})");
}

/// FacePlayer while talking or trading: yaw only, RInterpTo at 5.
fn face_player(
    time: Res<Time>,
    tdb: Res<TraderDb>,
    overlay: Res<GameOverlay>,
    mut traders: ResMut<Traders>,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
    mut bodies: Query<&mut Transform, (With<TraderBody>, Without<CharacterModel>)>,
) {
    let key = match *overlay {
        GameOverlay::Loot(k) | GameOverlay::Dialogue(k) if is_trader(k) => k,
        _ => return,
    };
    let (Ok(p), Some(t)) = (player.single(), traders.list.get_mut(key - TRADER_KEYS)) else { return };
    // BP_QuestGiver has no FacePlayer (only AnimBP_StationaryQuestNPC's head turn).
    if tdb.classes.get(&t.class).is_some_and(|c| c.quest_giver) {
        return;
    }
    let d = p.translation - t.transform.translation;
    if d.xz().length() < 0.01 {
        return;
    }
    // The actor's forward (UE X) is its local +X here.
    let want = Quat::from_rotation_y((-d.z).atan2(d.x));
    t.transform.rotation = t.transform.rotation.slerp(want, (time.delta_secs() * FACE_SPEED).min(1.0));
    if let Some(mut tf) = t.entity.and_then(|e| bodies.get_mut(e).ok()) {
        tf.rotation = t.transform.rotation;
    }
}

// ---------------------------------------------------------------- dialogue (BP_CustomNarrativeDefaultUI)

#[derive(Component)]
struct DialogueUi;

#[derive(Component, Clone, Copy)]
pub struct ReplyButton {
    pub trader: usize,
    /// The reply's index in the dialogue.
    pub reply: usize,
    pub trade: bool,
}

/// BP_CustomDialogueOption's ReplyButton: black at 50 %, 75 % hovered, 100 % pressed.
const REPLY: ButtonPalette = ButtonPalette { normal: Color::srgba(0.0, 0.0, 0.0, 0.5), hovered: Color::srgba(0.0, 0.0, 0.0, 0.75), pressed: Color::srgba(0.0, 0.0, 0.0, 1.0), chosen: Color::srgba(0.0, 0.0, 0.0, 0.5) };

/// The dialogue's opening line: Narrative picks the line or one of its alternatives.
pub fn opening_line(d: &Dialogue, rng: &mut impl Rng) -> String {
    if d.lines.is_empty() { String::new() } else { d.lines[rng.gen_range(0..d.lines.len())].clone() }
}

#[allow(clippy::too_many_arguments)]
fn dialogue_ui(
    mut commands: Commands,
    overlay: Res<GameOverlay>,
    follow: Res<FollowUp>,
    tdb: Res<TraderDb>,
    mut traders: ResMut<Traders>,
    font: Res<MenuFont>,
    assets: Res<AssetServer>,
    shown: Query<Entity, With<DialogueUi>>,
) {
    if !overlay.is_changed() && !follow.is_changed() {
        return;
    }
    for e in &shown {
        commands.entity(e).despawn();
    }
    let GameOverlay::Dialogue(key) = *overlay else { return };
    let Some(dialogue) = traders.get(&tdb, key).and_then(|(_, c)| c.dialogue.clone()) else { return };
    // After a reply the NPC's next lines play without replies.
    let answering = follow.lines.front().cloned();
    match &answering {
        Some(l) => traders.line = l.clone(),
        None => traders.line = opening_line(&dialogue, &mut rand::thread_rng()),
    }
    let extra_bold = assets.load("fonts/Montserrat-ExtraBold.ttf");
    let shadow = |px: f32| TextShadow { offset: Vec2::splat(px), color: Color::BLACK };
    let root = commands.spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, GlobalZIndex(3), DialogueUi, DespawnOnExit(AppState::InGame))).id();
    // VerticalBox_3: 1200 x 118 at the bottom centre, alignment (0.5, 1.6): CurrentSpeaker
    // (Montserrat-ExtraBold, UE's default 24, shadow 3) over CurrentDialogueLine (Bold 16, shadow 2).
    let speech = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), bottom: Val::Px(118.0 * 0.6), width: Val::Px(1200.0), height: Val::Px(118.0), margin: UiRect::left(Val::Px(-600.0)), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
            children![
                (Text::new(dialogue.speaker.clone().unwrap_or_default()), TextFont { font: extra_bold.into(), font_size: pt(24.0), ..default() }, TextColor(Color::WHITE), shadow(3.0)),
                (Text::new(traders.line.clone()), TextFont { font: font.0.clone().into(), font_size: pt(16.0), ..default() }, TextColor(Color::WHITE), shadow(2.0), TextLayout::justify(Justify::Center), Node { margin: UiRect::top(Val::Px(5.0)), ..default() }),
            ],
        ))
        .id();
    // Overlay_0's PlayerReplyBox: 800 x 200, alignment (0.5, 2.0), replies stacked at its bottom.
    let replies = commands
        .spawn(Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), bottom: Val::Px(200.0), width: Val::Px(800.0), height: Val::Px(200.0), margin: UiRect::left(Val::Px(-400.0)), flex_direction: FlexDirection::Column, justify_content: JustifyContent::FlexEnd, row_gap: Val::Px(4.0), ..default() })
        .id();
    for (ri, r) in dialogue.replies.iter().enumerate().filter(|_| answering.is_none()) {
        let b = commands
            .spawn((
                // CornerRadii (15, 5, 15, 5) = top-left, top-right, bottom-right, bottom-left.
                bevy::ui_widgets::Button,
                Hovered::default(),
                Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)), border_radius: BorderRadius::new(Val::Px(15.0), Val::Px(5.0), Val::Px(15.0), Val::Px(5.0)), ..default() },
                BackgroundColor(REPLY.normal),
                REPLY,
                ReplyButton { trader: key, reply: ri, trade: r.trade },
                children![(label_colored(&font, r.text.clone(), 14.0, Color::WHITE), Pickable::IGNORE)],
            ))
            .id();
        commands.entity(replies).add_child(b);
    }
    commands.entity(root).add_children(&[speech, replies]);
}

/// Event_RandomLaptopQuest's toast after BP_LaptopEventManager.CallRandomEvent.
pub const QUEST_PINNED: &str = "Quest pinned to your map";

#[allow(clippy::too_many_arguments)]
fn on_reply(
    ev: On<Activate>,
    replies: Query<&ReplyButton>,
    tdb: Res<TraderDb>,
    traders: Res<Traders>,
    mut game: ResMut<CurrentGame>,
    mut follow: ResMut<FollowUp>,
    mut overlay: ResMut<GameOverlay>,
    mut dirty: ResMut<super::inventory_ui::Dirty>,
    (mut scans, mut notify): (MessageWriter<super::quests::ScanLaptop>, MessageWriter<super::notify::Notify>),
) {
    let Ok(r) = replies.get(ev.entity) else { return };
    dirty.0 = true;
    let reply = traders.get(&tdb, r.trader).and_then(|(_, c)| c.dialogue.as_ref()?.replies.get(r.reply).cloned());
    let branches = reply.map(|r| r.branches).unwrap_or_default();
    if branches.is_empty() {
        // A reply leading nowhere ends the dialogue (an old traders.json: the trade flag).
        *overlay = if r.trade { GameOverlay::Loot(r.trader) } else { GameOverlay::None };
        return;
    }
    let said = run_branches(&branches, &mut |price, currency| {
        let inv = super::player_inv(&mut game);
        let enough = inv.count(currency) >= price;
        if enough {
            inv.remove(currency, price);
        }
        enough
    });
    let mut trade = false;
    for e in &said.events {
        match e.as_str() {
            // Event_OpenNPCVendor_C_0 (an Event_OnInteractNPC) -> OnExecuteInteractDialogue.
            "Event_OnInteractNPC" => trade = true,
            "Event_RandomLaptopQuest" => {
                scans.write(super::quests::ScanLaptop);
                notify.write(super::notify::Notify::new(QUEST_PINNED));
            }
            other => info!("dialogue event {other} not ported"),
        }
    }
    info!("dialogue reply {}: events {:?} lines {:?}", r.reply, said.events, said.lines);
    if trade {
        *overlay = GameOverlay::Loot(r.trader);
    } else if said.lines.is_empty() {
        *overlay = GameOverlay::None;
    } else {
        follow.lines = said.lines.into();
    }
}

/// The NPC's follow-up lines, each for its Narrative duration; the dialogue closes after the last.
fn follow_up(time: Res<Time>, mut follow: ResMut<FollowUp>, mut overlay: ResMut<GameOverlay>, mut left: Local<Option<f32>>) {
    if follow.lines.is_empty() {
        *left = None;
        return;
    }
    if !matches!(*overlay, GameOverlay::Dialogue(_)) {
        follow.lines.clear();
        return;
    }
    let t = left.get_or_insert_with(|| line_secs(&follow.lines[0]));
    *t -= time.delta_secs();
    if *t <= 0.0 {
        follow.lines.pop_front();
        *left = None;
        if follow.lines.is_empty() {
            *overlay = GameOverlay::None;
        }
    }
}

/// What F on a trader does (OnServerExecuteInteract -> BeginDialogue); traders without a
/// dialogue open the trade screen at once.
pub fn talk(tdb: &TraderDb, traders: &Traders, key: usize) -> GameOverlay {
    match traders.get(tdb, key) {
        Some((_, c)) if c.dialogue.as_ref().is_some_and(|d| !d.replies.is_empty()) => GameOverlay::Dialogue(key),
        _ => GameOverlay::Loot(key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dbs() -> Option<(ItemDb, TraderDb)> {
        let db: ItemDb = serde_json::from_str(&std::fs::read_to_string("assets/data/items.json").ok()?).ok()?;
        let tdb: TraderDb = serde_json::from_str(&std::fs::read_to_string("assets/data/traders.json").ok()?).ok()?;
        Some((db, tdb))
    }

    fn placed(db: &ItemDb, item: &str, count: u32, x: u8) -> Placed {
        let (w, h) = super::super::items::footprint(db, item, false).unwrap_or((1, 1));
        Placed { stack: db.stack(item, count), x, y: 0, w, h, rotated: false }
    }

    #[test]
    fn every_trader_table_and_item_exists() {
        let Some((db, tdb)) = dbs() else { return };
        assert!(tdb.classes.len() >= 9, "{} classes", tdb.classes.len());
        // (Quest givers sell nothing.)
        for (name, c) in tdb.classes.iter().filter(|(_, c)| !c.quest_giver) {
            let t = c.table.as_deref().unwrap_or_default();
            assert!(tdb.tables.contains_key(t), "{name}: table {t}");
            assert!(c.currency.as_deref().is_some_and(|m| db.items.contains_key(m)), "{name}: currency");
        }
        for (t, rows) in &tdb.tables {
            for r in rows {
                assert!(db.items.contains_key(&r.item), "{t}: {}", r.item);
            }
        }
        // The safe zone traders' Level1SuppliesTrader.
        let s = &tdb.classes["BP_SuppliesTrader_C"];
        assert_eq!((s.name.as_str(), s.table.as_deref(), s.grid), ("Supplies Trader", Some("Level1SuppliesTrader"), [13, 30]));
        assert!(s.dialogue.as_ref().unwrap().replies.iter().any(|r| r.trade && r.text == "What are you selling?"));
    }

    #[test]
    fn the_wandering_trader_sells_equipment_and_trades_on_f() {
        let Some((db, tdb)) = dbs() else { return };
        let w = &tdb.classes["BP_MasterTrader_C"];
        assert!(w.wandering && w.selling);
        assert_eq!((w.name.as_str(), w.table.as_deref(), w.grid), ("Wandering Trader", Some("WanderingTraderEquipment"), [13, 15]));
        assert_eq!(prompt(w), "Press [F] to Trade with Wandering Trader");
        assert_eq!(prompt(&tdb.classes["BP_SuppliesTrader_C"]), "Press [F] to talk to Supplies Trader");
        let stock = tdb.stock(&db, w, &mut rand::thread_rng());
        assert!(!stock.items.is_empty(), "an empty wandering stock");
        // Its dialogue opens the trade screen from "What are you selling?".
        let d = w.dialogue.as_ref().unwrap();
        assert_eq!(d.speaker.as_deref(), Some("Wandering Trader"));
        assert!(d.replies.iter().any(|r| r.trade && r.text == "What are you selling?"));
        let traders = Traders { list: vec![Trader { class: "BP_MasterTrader_C".into(), transform: Transform::default(), money: 700.0, entity: None, spawner: Some(1), dead: false }], ..default() };
        assert_eq!(talk(&tdb, &traders, TRADER_KEYS), GameOverlay::Dialogue(TRADER_KEYS));
    }

    #[test]
    fn the_settlement_officer_sells_intel_for_2500() {
        let Some((_, tdb)) = dbs() else { return };
        let o = &tdb.classes["BP_QuestGiver_SettlementOfficer_C"];
        assert!(o.quest_giver && o.table.is_none() && o.gun.is_some());
        assert_eq!(o.name, "Settlement Officer");
        let d = o.dialogue.as_ref().unwrap();
        assert_eq!(d.lines[0], "Hey. I've got some intel you may want. 2500 cash a piece. You got that much?");
        let intel = &d.replies[0];
        assert_eq!(intel.text, "What intel do you have? (2500 Cash)");
        // 3000 cash: 2500 is taken, the random laptop quest begins, then the two lines.
        let mut cash = 3000u32;
        let said = run_branches(&intel.branches, &mut |p, c| {
            assert_eq!(c, "DA_Money");
            let ok = cash >= p;
            if ok {
                cash -= p;
            }
            ok
        });
        assert_eq!(cash, 500);
        assert_eq!(said.events, ["Event_RandomLaptopQuest"]);
        assert_eq!(said.lines, ["I've marked the location on your GPS.", "Go out and find it. Good luck."]);
        // 500 left: nothing taken, nothing begun.
        let said = run_branches(&intel.branches, &mut |p, _| {
            let ok = cash >= p;
            if ok {
                cash -= p;
            }
            ok
        });
        assert_eq!(cash, 500);
        assert!(said.events.is_empty());
        assert_eq!(said.lines, ["It doesn't look like you have the right amount of cash yet.", "Make sure to come back when you can pay up."]);
        // "I'm busy at the moment." leads nowhere.
        assert!(d.replies[1].branches.is_empty());
        // A trader's "What are you selling?" fires the vendor event, with nothing to say.
        let t = &tdb.classes["BP_SuppliesTrader_C"].dialogue.as_ref().unwrap().replies[0];
        assert_eq!(run_branches(&t.branches, &mut |_, _| true), Said { events: vec!["Event_OnInteractNPC".into()], lines: vec![] });
        assert_eq!((line_secs("short"), line_secs(&"x".repeat(100))), (2.0, 4.0));
    }

    #[test]
    fn prices_scale_with_durability_and_count() {
        let Some((db, _)) = dbs() else { return };
        let ammo = db.stack("DA_12GaugeAmmo", 30);
        assert_eq!(stack_price(&db, &ammo), 30.0 * db.items["DA_12GaugeAmmo"].price);
        let (gun, def) = db.items.iter().find(|(_, d)| d.durability > 0.0 && d.price >= 10.0).map(|(k, d)| (k.clone(), d.clone())).unwrap();
        let mut worn = db.stack(&gun, 1);
        worn.durability = Some(def.durability / 2.0);
        assert_eq!(unit_price(&db, &worn), (def.price * def.durability / 2.0 / 100.0).trunc());
        assert_eq!(grouped(1234.0), "1,234");
        assert_eq!(grouped(500.0), "500");
    }

    #[test]
    fn buying_charges_money_and_selling_pays_it() {
        let Some((db, tdb)) = dbs() else { return };
        let class = tdb.classes["BP_SuppliesTrader_C"].clone();
        let price = db.items["DA_12GaugeAmmo"].price;
        let mut grid = Grid::new("Supplies Trader", 13, 30);
        grid.items.push(placed(&db, "DA_12GaugeAmmo", 50, 0));
        let mut inv = PlayerInventory::default();
        inv.insert(&db, db.stack("DA_Money", 100));
        let mut money = 500.0;

        // Buy 20 rounds (the stack in the grid drops to 30).
        let before = grid.items.clone();
        grid.items[0].stack.count = 30;
        inv.insert(&db, db.stack("DA_12GaugeAmmo", 20));
        settle(&db, Deal { class: &class, money: &mut money, limited: false }, &before, &mut grid, &mut inv).unwrap();
        assert_eq!(inv.count("DA_Money"), 100 - (20.0 * price) as u32);
        assert_eq!(money, 500.0, "unlimited money doesn't change");

        // With no money left: refused.
        let mut poor = PlayerInventory::default();
        let before = grid.items.clone();
        grid.items.clear();
        assert_eq!(settle(&db, Deal { class: &class, money: &mut money, limited: false }, &before, &mut grid, &mut poor), Err("Not enough money!"));

        // Sell a medkit to a trader that doesn't resell: paid, and it's gone from the grid.
        let mut grid = Grid::new("Supplies Trader", 13, 30);
        let before = grid.items.clone();
        grid.items.push(placed(&db, "DA_SmallMedkit", 1, 0));
        let had = inv.count("DA_Money");
        settle(&db, Deal { class: &class, money: &mut money, limited: true }, &before, &mut grid, &mut inv).unwrap();
        let medkit = db.items["DA_SmallMedkit"].price as u32;
        assert_eq!(inv.count("DA_Money"), had + medkit);
        assert!(grid.items.is_empty(), "Supplies Trader doesn't resell");
        assert_eq!(money, 500.0 - medkit as f32);

        // A trader with a set amount that's run out refuses to buy.
        let mut broke = 0.0;
        let before = grid.items.clone();
        grid.items.push(placed(&db, "DA_SmallMedkit", 1, 0));
        assert_eq!(settle(&db, Deal { class: &class, money: &mut broke, limited: true }, &before, &mut grid, &mut inv), Err("Trader does not have enough money"));

        // The books trader won't take anything.
        let books = tdb.classes["BP_BooksTrader_C"].clone();
        let mut grid = Grid::new("Books", 13, 10);
        let before = grid.items.clone();
        grid.items.push(placed(&db, "DA_SmallMedkit", 1, 0));
        assert_eq!(settle(&db, Deal { class: &books, money: &mut money, limited: false }, &before, &mut grid, &mut inv), Err("This trader is not accepting items"));
    }

    #[test]
    fn moving_stock_around_is_free_and_resold_items_keep_attachment_prices() {
        let Some((db, tdb)) = dbs() else { return };
        let mut grid = Grid::new("T", 13, 30);
        grid.items.push(placed(&db, "DA_12GaugeAmmo", 50, 0));
        let before = grid.items.clone();
        grid.items[0].x = 5;
        assert_eq!(exchange(&db, &before, &grid.items), Exchange::default());
        // A rifle with its magazine sold to the equipment trader resells at both prices.
        let class = tdb.classes["BP_EquipmentAndWeaponsItemTrader_C"].clone();
        let Some((rifle, mag)) = db.items.iter().find_map(|(k, d)| d.attachments.iter().find(|a| a.kind == "Mag").map(|a| (k.clone(), a.item.clone()))) else { return };
        let mut stack = db.stack(&rifle, 1);
        stack.attachments.push(db.stack(&mag, 1));
        let mut inv = PlayerInventory::default();
        let before = Vec::new();
        let mut grid = Grid::new("T", 13, 30);
        grid.items.push(Placed { stack, x: 0, y: 0, w: 1, h: 1, rotated: false });
        let mut money = 500.0;
        settle(&db, Deal { class: &class, money: &mut money, limited: false }, &before, &mut grid, &mut inv).unwrap();
        assert_eq!(inv.count("DA_Money"), db.items[&rifle].price as u32);
        assert_eq!(grid.items[0].stack.price, Some(db.items[&rifle].price + db.items[&mag].price));
    }

    #[test]
    fn currency_text_matches_the_widget() {
        let Some((_, tdb)) = dbs() else { return };
        assert_eq!(currency_text(&tdb.classes["BP_SuppliesTrader_C"], 500.0), "Currency: $500 ");
        assert_eq!(currency_text(&tdb.classes["BP_ShakesConesTrader_C"], 450.0), "Currency:  Cr450 ");
        assert_eq!(price_tag(&tdb.classes["BP_SuppliesTrader_C"], 12.0), "12$");
    }
}
