//! The trash bin, the disassembly workbench and the battery charger, from Buildable_TrashBin,
//! Buildable_DisassemblyWorkbench, Buildable_BatteryCharger and their W_TrashBinUI,
//! W_DisassemblyWorkbenchUI and W_BatteryChargerUI (decompiled).
//!
//! - All three: Interact (MC_OnInteractBuildable) opens the actor's BP_JigMultiplayer grids beside
//!   the inventory (SetupInventoryWidget + AddJigWidgetToContent under the pickup's name);
//!   buildables.json `grids` (gen_buildables.py: ContainersSettings).
//! - **Trash bin** (12 x 10): its widget's "Destroy Items" consumes everything in the grid.
//! - **Disassembly workbench** (12 x 8, then "Disassembled" 12 x 6): "Disassemble Items" goes
//!   through the top grid's items: one with CanBeDisassembled? (true unless the item says
//!   otherwise; items.json `disassembly`) rolls each of its DisassemblyItems: rand(0, 100) <=
//!   SpawnChance gives RandomIntegerInRange(min, max) x the stack's count into the lower grid
//!   (JigTryAddItemSomewhere with the top grid in NonAdd; what doesn't fit is lost), then the
//!   stack is consumed. Others stay and say "Item "{name}" cannot be disassembled".
//! - **Battery charger** (5 x 1, OnlySupportTheseItems DA_Battery; Requires Power?): Event_PowerOn
//!   starts a 5 s looping timer that adds 2 to every battery's durability, up to its max;
//!   Event_PowerOff clears it. A battery put in without power: "Generator needed to charge
//!   battery". UpdateStorageStat: "{Battery|Batteries}: {count} Charging {|[No Power]}" (the
//!   plural from 2).
//!
//! Ours: the buttons sit under the grid in the loot column (the widgets' own layout: the title,
//! the grid, a 15 px spacer, the button, the second grid). The charger's timer restarts with each
//! power-on, as the BP's does. Loot_TableTop_LeftHand_Montage is `building::interact`'s
//! (`ActorDef::interact_montage`). Not ported: CheckDistanceFromActor (walking away closes the
//! grid), the notifications' icons.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use rand::Rng;

use super::building::{BENCH_KEYS, BuildableDb, Buildables};
use super::interact::Containers;
use super::items::{Grid, ItemDb, Stack};
use super::notify::Notify;
use crate::menus::AppState;

/// The disassembly workbench's "Disassembled" grid keys (its first grid is the storage key).
pub const OUTPUT_KEYS: usize = BENCH_KEYS + (1 << 26);
/// The charger: Event_TimerPower's period and the durability it adds.
pub const CHARGE_SECS: f32 = 5.0;
pub const CHARGE_STEP: f32 = 2.0;
/// W_BatteryChargerUI's JSIContainer_1 OnlySupportTheseItems.
pub const BATTERY: &str = "DA_Battery";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    TrashBin,
    Disassembly,
    Charger,
}

impl Kind {
    pub fn of(actor: &str) -> Option<Self> {
        Some(match actor {
            "Buildable_TrashBin_C" => Self::TrashBin,
            "Buildable_DisassemblyWorkbench_C" => Self::Disassembly,
            "Buildable_BatteryCharger_C" => Self::Charger,
            _ => return None,
        })
    }

    /// The widget's button (ExitButton's BackText).
    pub fn button(self) -> Option<&'static str> {
        match self {
            Self::TrashBin => Some("Destroy Items"),
            Self::Disassembly => Some("Disassemble Items"),
            Self::Charger => None,
        }
    }
}

/// What the loot column shows under a container's grid: a button and a second grid.
#[derive(Clone, Debug, PartialEq)]
pub struct LootExtra {
    pub button: Option<&'static str>,
    pub second: Option<usize>,
}

/// The open utility bench's extras, by its grid's key (read by `inventory_ui::rebuild`).
#[derive(Resource, Default)]
pub struct LootExtras(pub HashMap<usize, LootExtra>);

/// A loot-column button (its container key).
#[derive(Component, Clone, Copy)]
pub struct LootButton(pub usize);

/// The chargers' timers and battery counts (by buildable id).
#[derive(Resource, Default)]
pub struct Chargers {
    /// Seconds into Event_TimerPower's period, while powered.
    timers: HashMap<usize, f32>,
    /// Batteries in it last frame (to catch one put in).
    counts: HashMap<usize, u32>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<LootExtras>()
        .init_resource::<Chargers>()
        .add_systems(OnEnter(AppState::InGame), |mut e: ResMut<LootExtras>, mut c: ResMut<Chargers>| {
            e.0.clear();
            *c = Chargers::default();
        })
        .add_systems(Update, charge.after(super::building::BuildSet).run_if(in_state(AppState::InGame)))
        .add_observer(on_button);
}

/// MC_OnInteractBuildable: the grids (made on first use) and the loot column's extras; returns
/// the key to open.
pub fn open(kind: Kind, l: &super::building::Live, grids: &[(i32, i32, String)], name: &str, containers: &mut Containers, extras: &mut LootExtras) -> usize {
    let key = l.storage_key();
    let size = |k: usize| grids.get(k).map_or((1, 1), |g| (g.0.max(1) as u8, g.1.max(1) as u8));
    let (cols, rows) = size(0);
    let g = containers.searched.entry(key).or_insert_with(|| Grid::new(name, cols, rows));
    g.name = name.to_string();
    if kind == Kind::Charger {
        g.only = vec![BATTERY.to_string()];
    }
    let second = (kind == Kind::Disassembly).then(|| {
        let (cols, rows) = size(1);
        let label = grids.get(1).map_or("Disassembled".to_string(), |g| g.2.clone());
        containers.searched.entry(OUTPUT_KEYS + l.id).or_insert_with(|| Grid::new(label, cols, rows));
        OUTPUT_KEYS + l.id
    });
    extras.0.insert(key, LootExtra { button: kind.button(), second });
    key
}

/// W_TrashBinUI's ExitButton: SERVER_RequestConsumeItem on every item.
pub fn destroy_items(g: &mut Grid) -> usize {
    let n = g.items.len();
    g.items.clear();
    n
}

/// W_DisassemblyWorkbenchUI's ExitButton over the top grid into the lower one; the messages.
pub fn disassemble(db: &ItemDb, input: &mut Grid, output: &mut Grid, rng: &mut impl Rng) -> Vec<String> {
    let mut notes = Vec::new();
    let mut keep = Vec::new();
    for p in std::mem::take(&mut input.items) {
        let Some(def) = db.items.get(&p.stack.item) else {
            keep.push(p);
            continue;
        };
        if !def.disassembly.can {
            notes.push(format!("Item \"{}\" cannot be disassembled", def.name));
            keep.push(p);
            continue;
        }
        for part in &def.disassembly.items {
            if rng.gen_range(0.0..=100.0) <= part.chance {
                let n = rng.gen_range(part.min.min(part.max)..=part.max.max(part.min)) * p.stack.count;
                if n > 0 && output.insert(db, db.stack(&part.item, n)).is_some() {
                    info!("disassembly: no room for {} x{n}", part.item);
                }
            }
        }
    }
    input.items = keep;
    notes
}

/// UpdateStorageStat's 'Battery' / 'Batteries' stat: "{A} Charging {C}".
pub fn charger_stat(batteries: usize, powered: bool) -> String {
    format!("{}: {batteries} Charging {}", if batteries > 1 { "Batteries" } else { "Battery" }, if powered { "" } else { "[No Power]" })
}

/// Event_TimerPower's step on one battery: + 2 durability, up to its max (`None` = full).
pub fn charged(durability: Option<f32>, max: f32) -> Option<f32> {
    let now = durability.unwrap_or(max);
    let next = (now + CHARGE_STEP).clamp(0.0, max);
    (next < max).then_some(next)
}

/// The chargers: Event_TimerPower every 5 s while powered; a battery put in without power.
fn charge(
    time: Res<Time>,
    db: Res<ItemDb>,
    placed: Res<Buildables>,
    mut chargers: ResMut<Chargers>,
    mut containers: ResMut<Containers>,
    mut dirty: ResMut<super::inventory_ui::Dirty>,
    mut notify: MessageWriter<Notify>,
) {
    let max = db.items.get(BATTERY).map_or(100.0, |d| d.durability);
    for l in placed.list.iter().filter(|l| Kind::of(&l.actor) == Some(Kind::Charger)) {
        let Some(g) = containers.searched.get_mut(&l.storage_key()) else { continue };
        let count = g.items.iter().map(|p| p.stack.count).sum::<u32>();
        let before = chargers.counts.insert(l.id, count).unwrap_or(count);
        // JigMP_OnMainContainerItemAdded without PoweredOn?.
        if count > before && !l.power.powered {
            notify.write(Notify::new("Generator needed to charge battery"));
        }
        if !l.power.powered {
            // Event_PowerOff: K2_ClearAndInvalidateTimerHandle.
            chargers.timers.remove(&l.id);
            continue;
        }
        let t = chargers.timers.entry(l.id).or_insert(0.0);
        *t += time.delta_secs();
        if *t < CHARGE_SECS {
            continue;
        }
        *t -= CHARGE_SECS;
        for p in g.items.iter_mut().filter(|p| p.stack.item == BATTERY) {
            p.stack.durability = charged(p.stack.durability, max);
        }
        info!("battery charger {}: charged {:?}", l.id, g.items.iter().map(|p| p.stack.durability).collect::<Vec<_>>());
        dirty.0 = true;
    }
}

fn on_button(ev: On<Activate>, buttons: Query<&LootButton>, db: Res<ItemDb>, extras: Res<LootExtras>, mut containers: ResMut<Containers>, mut dirty: ResMut<super::inventory_ui::Dirty>, mut notify: MessageWriter<Notify>) {
    let Ok(&LootButton(key)) = buttons.get(ev.entity) else { return };
    press(key, &db, &extras, &mut containers, &mut notify);
    dirty.0 = true;
}

/// A bench's button (also the tour's way in).
pub fn press(key: usize, db: &ItemDb, extras: &LootExtras, containers: &mut Containers, notify: &mut MessageWriter<Notify>) {
    let Some(extra) = extras.0.get(&key) else { return };
    match extra.second {
        Some(out) => {
            let (Some(mut input), Some(mut output)) = (containers.searched.remove(&key), containers.searched.remove(&out)) else { return };
            let notes = disassemble(db, &mut input, &mut output, &mut rand::thread_rng());
            info!("disassembly: {} item(s) left, {} out", input.items.len(), output.items.len());
            containers.searched.insert(key, input);
            containers.searched.insert(out, output);
            for n in notes {
                notify.write(Notify::new(n));
            }
        }
        None => {
            if let Some(g) = containers.searched.get_mut(&key) {
                info!("trash bin: destroyed {} item(s)", destroy_items(g));
            }
        }
    }
}

/// The charger's stat for the prompt.
pub fn stat(kind: Kind, l: &super::building::Live, containers: &Containers) -> Option<String> {
    (kind == Kind::Charger).then(|| {
        let n = containers.searched.get(&l.storage_key()).map_or(0, |g| g.items.len());
        charger_stat(n, l.power.powered)
    })
}

/// The grid sizes from buildables.json.
pub fn grids(db: &BuildableDb, actor: &str) -> Vec<(i32, i32, String)> {
    db.actors.get(actor).map(|d| d.grids.clone()).unwrap_or_default()
}

/// Unused stacks out of a grid by item (the tour's checks).
pub fn count(g: &Grid, item: &str) -> u32 {
    g.items.iter().filter(|p| p.stack.item == item).map(|p| p.stack.count).sum()
}

/// A battery at `durability` (the tour's).
pub fn battery(db: &ItemDb, durability: f32) -> Stack {
    let mut s = db.stack(BATTERY, 1);
    s.durability = Some(durability);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_charger_adds_two_up_to_full() {
        assert_eq!(charged(Some(50.0), 100.0), Some(52.0));
        assert_eq!(charged(Some(99.0), 100.0), None, "full is None");
        assert_eq!(charged(None, 100.0), None);
        assert_eq!(charger_stat(1, true), "Battery: 1 Charging ");
        assert_eq!(charger_stat(2, false), "Batteries: 2 Charging [No Power]");
        assert_eq!(charger_stat(0, false), "Battery: 0 Charging [No Power]");
    }

    #[test]
    fn disassembly_gives_parts_and_keeps_what_cant_be() {
        let db = ItemDb::load();
        let mut input = Grid::new("Container", 12, 8);
        let mut output = Grid::new("Disassembled", 12, 6);
        // A stack of 12 gauge shells: a casing and gunpowder each (100 %, 1-1).
        assert!(input.insert(&db, db.stack("DA_12GaugeAmmo", 5)).is_none());
        let cant = db.items.iter().find(|(_, d)| !d.disassembly.can).map(|(k, _)| k.clone()).expect("an item that can't be");
        assert!(input.insert(&db, db.stack(&cant, 1)).is_none());
        let notes = disassemble(&db, &mut input, &mut output, &mut rand::thread_rng());
        assert_eq!(count(&output, "DA_ShellCasing"), 5);
        assert_eq!(count(&output, "DA_Gunpowder"), 5);
        assert_eq!(input.items.len(), 1);
        assert_eq!(input.items[0].stack.item, cant);
        assert_eq!(notes, vec![format!("Item \"{}\" cannot be disassembled", db.items[&cant].name)]);
    }

    #[test]
    fn the_charger_takes_only_batteries() {
        let db = ItemDb::load();
        let mut g = Grid::new("Charger", 5, 1);
        g.only = vec![BATTERY.into()];
        assert!(g.insert(&db, db.stack("DA_12GaugeAmmo", 1)).is_some());
        assert!(g.place_at(&db, db.stack("DA_12GaugeAmmo", 1), 0, 0).is_err());
        assert!(g.insert(&db, battery(&db, 10.0)).is_none());
        let mut t = Grid::new("Trash", 12, 10);
        t.insert(&db, db.stack("DA_12GaugeAmmo", 3));
        assert_eq!(destroy_items(&mut t), 1);
        assert!(t.items.is_empty());
    }

    #[test]
    fn output_keys_are_their_own() {
        assert!(OUTPUT_KEYS > BENCH_KEYS + (1 << 20));
        assert!(OUTPUT_KEYS + (1 << 20) < BENCH_KEYS + (1 << 28));
    }
}
