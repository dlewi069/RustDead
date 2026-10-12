//! Items, loot tables and grid inventories from `assets/data/items.json`
//! (`extractor/gen_items.py`, built from the JigSInventory data assets).

use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// `Jig.Rarity.*` tail (Common, Hunger, Medical, Rare...): tints the inventory cell.
    #[serde(default)]
    pub rarity: String,
    pub size: [u8; 2],
    pub weight: f32,
    pub max_stack: u32,
    pub price: f32,
    pub consumable: bool,
    /// `RemoveMedicalEffects`: effects using it ends ("Bleed", "Heavy Bleed", "Broken Bone").
    #[serde(default)]
    pub removes: Vec<String>,
    /// Consumable effects, e.g. "Hunger": [20, 20] (min, max).
    pub stats: HashMap<String, [f32; 2]>,
    pub mesh: Option<String>,
    /// Inventory picture: inspect mesh rotation (glb space quat) and camera FOV in degrees.
    #[serde(default = "identity")]
    pub inspect_rotation: [f32; 4],
    #[serde(default = "default_fov")]
    pub fov: f32,
    /// Grids this item carries when worn (backpacks, clothing pockets, rigs, cases).
    #[serde(default)]
    pub grids: Vec<[u8; 2]>,
    /// Clothing mesh drawn on the survivor when equipped.
    #[serde(default)]
    pub clothing: Option<Clothing>,
    /// `RandomStats`, rolled per instance in this order until `max_stats` succeed.
    #[serde(default)]
    pub random_stats: Vec<RandomStat>,
    #[serde(default)]
    pub max_stats: usize,
    /// `DefaultAttachments` (a firearm's magazine, a magazine's rounds).
    #[serde(default)]
    pub attachments: Vec<AttachmentRoll>,
    /// `TextStats`: "Firemodes" ("Semi, Auto"), "Caliber".
    #[serde(default)]
    pub text_stats: HashMap<String, String>,
    /// Magazines: rounds held (`ContainerCapacity`) and the ammo that fits.
    #[serde(default)]
    pub capacity: u32,
    #[serde(default)]
    pub ammo: Vec<String>,
    /// Sockets the weapon sits in, drawn and holstered, and its offset from them.
    #[serde(default)]
    pub equip: Option<EquipSockets>,
    /// `MaxDurability`, and `ReduceDurabilityAmount` lost per shot (ReduceDurability).
    #[serde(default)]
    pub durability: f32,
    #[serde(default)]
    pub reduce_durability: f32,
    /// The pickup's `BP_WeaponsPickupComponent` (firearms only).
    #[serde(default)]
    pub firearm: Option<FirearmData>,
}

/// What `BP_FirearmPickup` reads off the weapon's `BP_WeaponsPickupComponent`.
#[derive(Deserialize, Clone, Copy, Debug, Default)]
pub struct FirearmData {
    /// `Enum_Firearms` value: 0 pistol, 1 SMG, 2 rifle, 3 marksman, 4 shotgun, 5 grenade
    /// launcher, 6 rocket launcher, 7 crossbow, 8 flare gun.
    #[serde(rename = "type")]
    pub kind: u8,
    pub automatic: bool,
    /// `FirearmMalfunctionChance` (0-1) at zero durability (DurabilityCheck).
    pub malfunction_chance: f32,
    /// `DefaultSoundRadius` (m): how far the shot is heard (ReportNoiseEvent MaxRange).
    pub sound_radius: f32,
}

impl FirearmData {
    /// `FirearmTypeMultiplier` for a headshot.
    pub fn headshot_multiplier(&self) -> f32 {
        match self.kind {
            0 | 1 => 2.0,
            2 => 2.25,
            3 | 7 => 2.5,
            4 => 1.5,
            _ => 1.0,
        }
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "PascalCase")]
pub struct EquipSockets {
    pub equip_socket: Option<String>,
    pub primary_unequip_socket: Option<String>,
    pub primary_unequip_socket_backpack: Option<String>,
    pub secondary_unequip_socket: Option<String>,
    pub secondary_unequip_socket_backpack: Option<String>,
    #[serde(rename = "transform")]
    pub transform: Option<LocalTransform>,
}

/// A local transform already in glb space (t metres, r quat xyzw, s scale).
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct LocalTransform {
    pub t: [f32; 3],
    pub r: [f32; 4],
    #[serde(default = "one3")]
    pub s: [f32; 3],
}

fn one3() -> [f32; 3] {
    [1.0; 3]
}

impl LocalTransform {
    pub fn to_transform(self) -> Transform {
        Transform { translation: Vec3::from_array(self.t), rotation: Quat::from_array(self.r).normalize(), scale: Vec3::from_array(self.s) }
    }
}

/// A skeleton socket of the survivor (`SK_Chr_ToplessMale_01_Skeleton`).
#[derive(Deserialize, Clone, Debug)]
pub struct Socket {
    pub bone: String,
    #[serde(flatten)]
    pub transform: LocalTransform,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RandomStat {
    pub stat: String,
    pub range: [f32; 2],
    /// Percent.
    pub chance: f32,
    pub rounded: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AttachmentRoll {
    pub item: String,
    /// `Jig.AttachmentType.*` tail ("Mag"), "None" for loose contents (absent inside).
    #[serde(default)]
    pub kind: String,
    pub chance: f32,
    pub min: u32,
    pub max: u32,
    #[serde(default)]
    pub inside: Vec<AttachmentRoll>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Clothing {
    pub male: Option<String>,
    pub female: Option<String>,
}

impl ItemDef {
    /// Only plain items stack; anything with compartments is unique.
    pub fn is_stackable(&self) -> bool {
        self.max_stack > 1 && self.grids.is_empty()
    }

    /// A stat's value for an instance that never rolled it: the middle of its range.
    pub fn stat_default(&self, stat: &str) -> Option<f32> {
        self.random_stats.iter().find(|s| s.stat == stat).map(|s| (s.range[0] + s.range[1]) / 2.0)
    }

    pub fn firemodes(&self) -> Vec<&str> {
        self.text_stats.get("Firemodes").map_or_else(Vec::new, |f| f.split(',').map(str::trim).collect())
    }
}

fn identity() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_fov() -> f32 {
    30.0
}

#[derive(Deserialize, Clone, Debug)]
pub struct LootRow {
    pub item: String,
    /// Percent.
    pub chance: f32,
    pub min: u32,
    pub max: u32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ContainerDef {
    pub name: String,
    pub table: Option<String>,
    /// Seconds the interact key is held to search (BP_LootContainer `DurationToHold`).
    pub hold: f32,
    pub lockpick: bool,
}

/// A `Struct_ItemsToCraft` row of `DT_CraftingTableTypes`.
#[derive(Deserialize, Clone, Debug)]
pub struct Recipe {
    pub item: String,
    pub count: u32,
    /// (item, count) consumed.
    pub needs: Vec<(String, u32)>,
    /// `TimeToCraftInSecs` (0 for every recipe in this build).
    pub time: f32,
}

#[derive(Resource, Deserialize)]
pub struct ItemDb {
    pub items: HashMap<String, ItemDef>,
    pub loot_tables: HashMap<String, Vec<LootRow>>,
    pub containers: HashMap<String, ContainerDef>,
    /// Crafting table ("Player" = the inventory's CRAFTING tab, else a workbench) -> recipes.
    #[serde(default)]
    pub recipes: HashMap<String, Vec<Recipe>>,
    #[serde(default)]
    pub sockets: HashMap<String, Socket>,
}

impl ItemDb {
    pub fn load() -> Self {
        let text = std::fs::read_to_string("assets/data/items.json").expect("assets/data/items.json missing (extractor/export_assets.sh --items)");
        serde_json::from_str(&text).expect("assets/data/items.json malformed")
    }

    /// Roll a table the way the container BPs do: each row independently by its spawn
    /// chance, with a uniform count in [min, max].
    pub fn roll(&self, table: &str, rng: &mut impl Rng) -> Vec<Stack> {
        let mut out = Vec::new();
        for row in self.loot_tables.get(table).into_iter().flatten() {
            if !self.items.contains_key(&row.item) || rng.gen_range(0.0..100.0) >= row.chance {
                continue;
            }
            let count = rng.gen_range(row.min.max(1)..=row.max.max(row.min).max(1));
            out.push(self.spawn(&row.item, count, rng));
        }
        out
    }

    /// A freshly found item: random stats rolled, default attachments by chance (a rifle's
    /// magazine with some rounds in it).
    pub fn spawn(&self, item: &str, count: u32, rng: &mut impl Rng) -> Stack {
        let mut stack = self.stack(item, count);
        let Some(def) = self.items.get(item) else { return stack };
        for s in &def.random_stats {
            if stack.stats.len() >= def.max_stats.max(1) {
                break;
            }
            if rng.gen_range(0.0..100.0) < s.chance {
                let v = if s.range[1] > s.range[0] { rng.gen_range(s.range[0]..=s.range[1]) } else { s.range[0] };
                stack.stats.insert(s.stat.clone(), if s.rounded { v.round() } else { v });
            }
        }
        for a in &def.attachments {
            if !self.items.contains_key(&a.item) || rng.gen_range(0.0..100.0) >= a.chance {
                continue;
            }
            let n = rng.gen_range(a.min.max(1)..=a.max.max(a.min).max(1));
            if def.capacity > 0 {
                // Rounds in a magazine.
                stack.rounds = (stack.rounds + n).min(def.capacity);
            } else {
                let mut part = self.spawn(&a.item, 1, rng);
                for i in &a.inside {
                    if rng.gen_range(0.0..100.0) < i.chance {
                        let cap = self.items.get(&a.item).map_or(0, |d| d.capacity);
                        part.rounds = (part.rounds + rng.gen_range(i.min.max(1)..=i.max.max(i.min).max(1))).min(cap);
                    }
                }
                stack.attachments.push(part);
            }
        }
        stack
    }

    /// A new stack of `item`; wearable containers come with their (empty) grids.
    pub fn stack(&self, item: &str, count: u32) -> Stack {
        let contents = self.items.get(item).map_or_else(Vec::new, |d| {
            d.grids.iter().enumerate().map(|(i, [w, h])| Grid::new(format!("{} {}", d.name, i + 1), *w, *h)).collect()
        });
        Stack { item: item.to_string(), count, contents, ..default() }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    /// Data asset name (key into [`ItemDb::items`]).
    pub item: String,
    pub count: u32,
    /// Grids inside the item (a backpack's compartments); they travel with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contents: Vec<Grid>,
    /// Rolled `RandomStats` of this instance.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stats: BTreeMap<String, f32>,
    /// Weapon attachments (the magazine).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Stack>,
    /// Rounds loaded (magazines).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rounds: u32,
    /// Durability left (weapons); `None` is the item's full `MaxDurability`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durability: Option<f32>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Stack {
    /// This instance's stat, or the item's mid-range value when it didn't roll it.
    pub fn stat(&self, db: &ItemDb, stat: &str) -> Option<f32> {
        self.stats.get(stat).copied().or_else(|| db.items.get(&self.item)?.stat_default(stat))
    }

    /// The attached magazine.
    pub fn mag(&self, db: &ItemDb) -> Option<&Stack> {
        self.attachments.iter().find(|a| db.items.get(&a.item).is_some_and(|d| d.capacity > 0))
    }

    pub fn mag_mut(&mut self, db: &ItemDb) -> Option<&mut Stack> {
        self.attachments.iter_mut().find(|a| db.items.get(&a.item).is_some_and(|d| d.capacity > 0))
    }

    /// Total weight including anything stored inside.
    pub fn weight(&self, db: &ItemDb) -> f32 {
        db.items.get(&self.item).map_or(0.0, |d| d.weight) * self.count as f32 + self.contents.iter().map(|g| g.weight(db)).sum::<f32>()
    }
}

/// Cells an item covers, (w, h) swapped when rotated.
pub fn footprint(db: &ItemDb, item: &str, rotated: bool) -> Option<(u8, u8)> {
    let d = db.items.get(item)?;
    let (w, h) = (d.size[0].max(1), d.size[1].max(1));
    Some(if rotated { (h, w) } else { (w, h) })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placed {
    pub stack: Stack,
    pub x: u8,
    pub y: u8,
    /// Footprint in cells (already swapped when rotated).
    pub w: u8,
    pub h: u8,
    /// Turned 90° (JigSInventory `Rotated?`, R while dragging).
    #[serde(default)]
    pub rotated: bool,
}

/// A JigSInventory-style grid: items occupy w x h cells.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub name: String,
    pub width: u8,
    pub height: u8,
    pub items: Vec<Placed>,
}

impl Grid {
    pub fn new(name: impl Into<String>, width: u8, height: u8) -> Self {
        Self { name: name.into(), width, height, items: Vec::new() }
    }

    fn free(&self, x: u8, y: u8, w: u8, h: u8) -> bool {
        self.free_ignoring(x, y, w, h, None)
    }

    /// Is the w x h area at (x, y) inside the grid and empty (optionally ignoring one item,
    /// e.g. the one being dragged)?
    pub fn free_ignoring(&self, x: u8, y: u8, w: u8, h: u8, ignore: Option<usize>) -> bool {
        if x as u16 + w as u16 > self.width as u16 || y as u16 + h as u16 > self.height as u16 {
            return false;
        }
        !self.items.iter().enumerate().any(|(i, p)| Some(i) != ignore && x < p.x + p.w && p.x < x + w && y < p.y + p.h && p.y < y + h)
    }

    /// Put a stack at a specific cell (drag and drop). Gives it back if it doesn't fit.
    pub fn place_at(&mut self, db: &ItemDb, stack: Stack, x: u8, y: u8) -> Result<(), Stack> {
        self.place_rotated(db, stack, x, y, false)
    }

    pub fn place_rotated(&mut self, db: &ItemDb, stack: Stack, x: u8, y: u8, rotated: bool) -> Result<(), Stack> {
        let Some((w, h)) = footprint(db, &stack.item, rotated) else { return Err(stack) };
        if !self.free(x, y, w, h) {
            return Err(stack);
        }
        self.items.push(Placed { stack, x, y, w, h, rotated });
        Ok(())
    }

    /// First free spot for an item, upright first, then turned (`GetEmptySlotTryRotated`).
    pub fn free_spot(&self, db: &ItemDb, item: &str) -> Option<(u8, u8, bool)> {
        [false, true].into_iter().find_map(|rot| {
            let (w, h) = footprint(db, item, rot)?;
            (0..self.height).flat_map(|y| (0..self.width).map(move |x| (x, y))).find(|&(x, y)| self.free(x, y, w, h)).map(|(x, y)| (x, y, rot))
        })
    }

    /// Index of the item covering a cell.
    pub fn at(&self, x: u8, y: u8) -> Option<usize> {
        self.items.iter().position(|p| x >= p.x && x < p.x + p.w && y >= p.y && y < p.y + p.h)
    }

    /// How many of `item` the grid holds.
    pub fn count(&self, item: &str) -> u32 {
        self.items.iter().filter(|p| p.stack.item == item).map(|p| p.stack.count).sum()
    }

    /// Take up to `n` of `item`, emptiest stacks first; returns how many were removed.
    pub fn remove(&mut self, item: &str, mut n: u32) -> u32 {
        let start = n;
        let mut order: Vec<usize> = (0..self.items.len()).filter(|&i| self.items[i].stack.item == item).collect();
        order.sort_by_key(|&i| self.items[i].stack.count);
        for i in order {
            let take = self.items[i].stack.count.min(n);
            self.items[i].stack.count -= take;
            n -= take;
            if n == 0 {
                break;
            }
        }
        self.items.retain(|p| p.stack.count > 0);
        start - n
    }

    /// Merge into the stack at `index` if it's the same stackable item; returns the remainder.
    pub fn merge_into(&mut self, db: &ItemDb, index: usize, mut stack: Stack) -> Option<Stack> {
        let (Some(def), Some(p)) = (db.items.get(&stack.item), self.items.get_mut(index)) else { return Some(stack) };
        if p.stack.item != stack.item || !def.is_stackable() {
            return Some(stack);
        }
        let moved = def.max_stack.saturating_sub(p.stack.count).min(stack.count);
        p.stack.count += moved;
        stack.count -= moved;
        (stack.count > 0).then_some(stack)
    }

    /// Add a stack: top up matching stacks first, then place in the first free spot.
    /// Returns whatever didn't fit.
    pub fn insert(&mut self, db: &ItemDb, mut stack: Stack) -> Option<Stack> {
        let Some(def) = db.items.get(&stack.item) else { return Some(stack) };
        for p in self.items.iter_mut().filter(|p| p.stack.item == stack.item && def.is_stackable()) {
            let room = def.max_stack.saturating_sub(p.stack.count);
            let moved = room.min(stack.count);
            p.stack.count += moved;
            stack.count -= moved;
            if stack.count == 0 {
                return None;
            }
        }
        while stack.count > 0 {
            let Some((x, y, rotated)) = self.free_spot(db, &stack.item) else { return Some(stack) };
            let (w, h) = footprint(db, &stack.item, rotated).unwrap_or((1, 1));
            let n = stack.count.min(def.max_stack.max(1));
            // Instance data (contents, stats, attachments) goes with the first part; items that
            // carry it never stack, so that's the whole stack.
            let mut part = std::mem::take(&mut stack);
            stack = Stack { item: part.item.clone(), count: part.count - n, ..default() };
            part.count = n;
            self.items.push(Placed { stack: part, x, y, w, h, rotated });
        }
        None
    }

    pub fn weight(&self, db: &ItemDb) -> f32 {
        self.items.iter().map(|p| p.stack.weight(db)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> ItemDb {
        let item = |w, h, stack| ItemDef {
            id: String::new(),
            name: String::new(),
            kind: String::new(),
            rarity: String::new(),
            size: [w, h],
            weight: 1.0,
            max_stack: stack,
            price: 0.0,
            consumable: false,
            removes: Vec::new(),
            stats: HashMap::new(),
            mesh: None,
            inspect_rotation: [0.0, 0.0, 0.0, 1.0],
            fov: 30.0,
            grids: Vec::new(),
            clothing: None,
            random_stats: Vec::new(),
            max_stats: 0,
            attachments: Vec::new(),
            text_stats: HashMap::new(),
            capacity: 0,
            ammo: Vec::new(),
            equip: None,
            durability: 0.0,
            reduce_durability: 0.0,
            firearm: None,
        };
        ItemDb {
            items: HashMap::from([("Can".into(), item(1, 1, 5)), ("Rifle".into(), item(4, 2, 1))]),
            loot_tables: HashMap::new(),
            containers: HashMap::new(),
            recipes: HashMap::new(),
            sockets: HashMap::new(),
        }
    }

    #[test]
    fn stacks_fill_then_spill_into_new_cells() {
        let db = db();
        let mut g = Grid::new("t", 2, 1);
        assert!(g.insert(&db, Stack { item: "Can".into(), count: 7, ..default() }).is_none());
        assert_eq!(g.items.len(), 2);
        assert_eq!(g.items[0].stack.count, 5);
        let left = g.insert(&db, Stack { item: "Can".into(), count: 4, ..default() });
        assert_eq!(left, Some(Stack { item: "Can".into(), count: 1, ..default() }));
    }

    #[test]
    fn big_items_need_room() {
        let db = db();
        let mut g = Grid::new("t", 4, 2);
        assert!(g.insert(&db, Stack { item: "Rifle".into(), count: 1, ..default() }).is_none());
        assert!(g.insert(&db, Stack { item: "Rifle".into(), count: 1, ..default() }).is_some());
    }

    #[test]
    fn long_items_turn_to_fit_and_remove_takes_from_small_stacks() {
        let db = db();
        // A 4x2 rifle only fits a 2x4 grid sideways.
        let mut g = Grid::new("t", 2, 4);
        assert!(g.insert(&db, Stack { item: "Rifle".into(), count: 1, ..default() }).is_none());
        assert!(g.items[0].rotated && (g.items[0].w, g.items[0].h) == (2, 4));
        assert!(g.place_rotated(&db, Stack { item: "Rifle".into(), count: 1, ..default() }, 0, 0, false).is_err());

        let mut g = Grid::new("t", 3, 1);
        g.insert(&db, Stack { item: "Can".into(), count: 7, ..default() });
        assert_eq!(g.count("Can"), 7);
        assert_eq!(g.remove("Can", 3), 3);
        assert_eq!(g.count("Can"), 4);
        assert_eq!(g.items.len(), 1, "the 2-stack emptied first");
        assert_eq!(g.remove("Can", 10), 4);
    }

    #[test]
    fn real_tables_roll_known_items() {
        let Ok(text) = std::fs::read_to_string("assets/data/items.json") else { return };
        let db: ItemDb = serde_json::from_str(&text).unwrap();
        let mut rng = rand::thread_rng();
        let mut seen = 0;
        for _ in 0..200 {
            for s in db.roll("ResidentialFoodDrink", &mut rng) {
                assert!(db.items.contains_key(&s.item));
                seen += 1;
            }
        }
        assert!(seen > 0, "food table never rolled anything");
    }
}
