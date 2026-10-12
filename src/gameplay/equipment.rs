//! The survivor's inventory as `Inventory/BP_Inventory` lays it out: 4x2 pockets, the grids of
//! whatever wearable containers are equipped (body armor, backpack, torso and leg clothing,
//! container case), and the equipment slots (`JSIContainer` EquipTo slots with a
//! `Jig.PlayerSlot.*` tag accepting one `Jig.ItemType.*`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::items::{Grid, ItemDb, Recipe, Stack};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Slot {
    Torso,
    Gloves,
    Legs,
    Feet,
    Backpack,
    Container,
    BodyArmor,
    Primary,
    Secondary,
    Sidearm,
    Melee,
    Gps,
    Compass,
    FishingRod,
    Binoculars,
    Throwable,
    Flashlight,
    Eyewear,
    Accessory,
    Facewear,
    Headwear,
}

impl Slot {
    /// Rows of the equipment panel, top to bottom, as BP_Inventory orders them.
    pub const ROWS: [&'static [Slot]; 5] = [
        &[Slot::Torso, Slot::Gloves, Slot::Legs, Slot::Feet],
        &[Slot::Backpack, Slot::Container, Slot::BodyArmor],
        &[Slot::Primary, Slot::Secondary, Slot::Sidearm, Slot::Melee],
        &[Slot::Gps, Slot::Compass, Slot::FishingRod, Slot::Binoculars, Slot::Throwable, Slot::Flashlight],
        &[Slot::Eyewear, Slot::Accessory, Slot::Facewear, Slot::Headwear],
    ];

    /// `ContainerName` shown on the slot (tool slots have none).
    pub fn label(self) -> Option<&'static str> {
        Some(match self {
            Slot::Torso => "TORSO",
            Slot::Gloves => "GLOVES",
            Slot::Legs => "LEGS",
            Slot::Feet => "FEET",
            Slot::Backpack => "BACKPACK",
            Slot::Container => "CONTAINER",
            Slot::BodyArmor => "BODY ARMOR",
            Slot::Primary => "PRIMARY",
            Slot::Secondary => "SECONDARY",
            Slot::Sidearm => "SIDEARM",
            Slot::Melee => "MELEE",
            Slot::Eyewear => "EYEWEAR",
            Slot::Accessory => "ACCESSORY",
            Slot::Facewear => "FACEWEAR",
            Slot::Headwear => "HEADWEAR",
            _ => return None,
        })
    }

    /// Placeholder silhouette (`SlotImage`) under Content/Inventory/Textures.
    pub fn texture(self) -> &'static str {
        match self {
            Slot::Torso => "TorsoSlot",
            Slot::Gloves => "GlovesSlot",
            Slot::Legs => "LegsSlot",
            Slot::Feet => "FeetSlot",
            Slot::Backpack => "BackpackSlot",
            Slot::Container => "ContainerSlot",
            Slot::BodyArmor => "ArmorSlot",
            Slot::Primary | Slot::Secondary => "PrimarySlot",
            Slot::Sidearm => "SidearmSlot",
            Slot::Melee => "MeleeSlot",
            Slot::Gps => "GPSSlot",
            Slot::Compass => "CompassSlot",
            Slot::FishingRod => "FishingSlot",
            Slot::Binoculars => "BinosSlot",
            Slot::Throwable => "ThrowableSlot",
            Slot::Flashlight => "FlashlightSlot",
            Slot::Eyewear => "EyewearSlot",
            Slot::Accessory => "AccessorySlot",
            Slot::Facewear => "FacewearSlot",
            Slot::Headwear => "HeadwearSlot",
        }
    }

    /// `SlotSize` of the slot widget.
    pub fn size(self) -> (f32, f32) {
        match self {
            Slot::Primary | Slot::Secondary => (360.0, 160.0),
            Slot::Sidearm | Slot::Melee => (200.0, 160.0),
            Slot::Gps | Slot::Compass | Slot::FishingRod | Slot::Binoculars | Slot::Throwable | Slot::Flashlight => (80.0, 80.0),
            _ => (140.0, 140.0),
        }
    }

    /// The `Jig.ItemType.*` this slot accepts (`SupportedItemTypes`).
    pub fn accepts(self) -> &'static str {
        match self {
            Slot::Torso => "Torso",
            Slot::Gloves => "Gloves",
            Slot::Legs => "Legs",
            Slot::Feet => "Feet",
            Slot::Backpack => "Backpack",
            Slot::Container => "Container",
            Slot::BodyArmor => "BodyArmor",
            Slot::Primary | Slot::Secondary => "MainFirearm",
            Slot::Sidearm => "Sidearm",
            Slot::Melee => "Melee",
            Slot::Gps => "GPS",
            Slot::Compass => "Compass",
            Slot::FishingRod => "FishingRod",
            Slot::Binoculars => "Binoculars",
            Slot::Throwable => "Throwable",
            Slot::Flashlight => "Flashlight",
            Slot::Eyewear => "Eyewear",
            Slot::Accessory => "Accessory",
            Slot::Facewear => "Facewear",
            Slot::Headwear => "Headwear",
        }
    }

    pub fn fits(self, kind: &str) -> bool {
        self.accepts() == kind
    }
}

/// Order the equipped containers' grids appear under the pockets (BP_Inventory's
/// B_BodyArmor, B_Backpack, B_Torso, B_Legs, B_Bag borders).
pub const WORN_GRIDS: [Slot; 5] = [Slot::BodyArmor, Slot::Backpack, Slot::Torso, Slot::Legs, Slot::Container];

/// Which grid an item lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GridRef {
    Pockets,
    /// The `n`th compartment of the item equipped in a slot.
    Worn(Slot, usize),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerInventory {
    pub pockets: Grid,
    pub equipped: BTreeMap<Slot, Stack>,
}

impl Default for PlayerInventory {
    fn default() -> Self {
        Self { pockets: Grid::new("Pockets", 4, 2), equipped: BTreeMap::new() }
    }
}

impl PlayerInventory {
    /// Every grid the survivor carries, in display order.
    pub fn grids(&self) -> Vec<(GridRef, &Grid)> {
        let mut out = vec![(GridRef::Pockets, &self.pockets)];
        for slot in WORN_GRIDS {
            if let Some(s) = self.equipped.get(&slot) {
                out.extend(s.contents.iter().enumerate().map(|(i, g)| (GridRef::Worn(slot, i), g)));
            }
        }
        out
    }

    pub fn grid_mut(&mut self, r: GridRef) -> Option<&mut Grid> {
        match r {
            GridRef::Pockets => Some(&mut self.pockets),
            GridRef::Worn(slot, i) => self.equipped.get_mut(&slot)?.contents.get_mut(i),
        }
    }

    /// Put a stack wherever it fits (pockets first, then worn containers). Returns the rest.
    pub fn insert(&mut self, db: &ItemDb, stack: Stack) -> Option<Stack> {
        let refs: Vec<GridRef> = self.grids().into_iter().map(|(r, _)| r).collect();
        let mut rest = Some(stack);
        for r in refs {
            let Some(s) = rest.take() else { break };
            rest = self.grid_mut(r).and_then(|g| g.insert(db, s));
        }
        rest
    }

    /// The slot an item would equip into (firearms take Primary, then Secondary).
    pub fn slot_for(&self, db: &ItemDb, item: &str) -> Option<Slot> {
        let kind = db.items.get(item)?.kind.as_str();
        if kind == "MainFirearm" {
            return Some(if self.equipped.contains_key(&Slot::Primary) && !self.equipped.contains_key(&Slot::Secondary) { Slot::Secondary } else { Slot::Primary });
        }
        Slot::ROWS.iter().flat_map(|r| r.iter()).copied().find(|s| s.fits(kind))
    }

    pub fn can_equip(db: &ItemDb, slot: Slot, item: &str) -> bool {
        db.items.get(item).is_some_and(|d| slot.fits(&d.kind))
    }

    /// Equip into `slot`, returning what was there (with its contents).
    pub fn equip(&mut self, slot: Slot, stack: Stack) -> Option<Stack> {
        self.equipped.insert(slot, stack)
    }

    /// How many of `item` are carried in the pockets and worn containers.
    pub fn count(&self, item: &str) -> u32 {
        self.grids().iter().map(|(_, g)| g.count(item)).sum()
    }

    /// The first `item` carried in a grid or equipped (e.g. a crowbar to spend durability on).
    pub fn find_mut(&mut self, item: &str) -> Option<&mut Stack> {
        if let Some(slot) = self.equipped.iter().find(|(_, s)| s.item == item).map(|(k, _)| *k) {
            return self.equipped.get_mut(&slot);
        }
        let r = self.grids().into_iter().find(|(_, g)| g.items.iter().any(|p| p.stack.item == item)).map(|(r, _)| r)?;
        self.grid_mut(r)?.items.iter_mut().map(|p| &mut p.stack).find(|s| s.item == item)
    }

    /// Remove `n` of `item` across the carried grids; nothing is taken unless all `n` are there.
    pub fn remove(&mut self, item: &str, n: u32) -> bool {
        if self.count(item) < n {
            return false;
        }
        let refs: Vec<GridRef> = self.grids().into_iter().map(|(r, _)| r).collect();
        let mut left = n;
        for r in refs {
            if let Some(g) = self.grid_mut(r) {
                left -= g.remove(item, left);
            }
        }
        true
    }

    /// Times `recipe` can be crafted from what's carried.
    pub fn craftable(&self, recipe: &Recipe) -> u32 {
        recipe.needs.iter().map(|(item, n)| self.count(item) / (*n).max(1)).min().unwrap_or(0)
    }

    /// Craft `times` (W_PlayerCraftingUI): consume the ingredients, return the products.
    /// Products have to be placed by the caller (they may not fit and fall to the ground).
    pub fn craft(&mut self, db: &ItemDb, recipe: &Recipe, times: u32) -> Option<Stack> {
        if times == 0 || self.craftable(recipe) < times {
            return None;
        }
        for (item, n) in &recipe.needs {
            self.remove(item, n * times);
        }
        Some(db.stack(&recipe.item, recipe.count * times))
    }

    pub fn weight(&self, db: &ItemDb) -> f32 {
        self.pockets.weight(db) + self.equipped.values().map(|s| s.weight(db)).sum::<f32>()
    }

    /// Equipped item names for the action bar (1 primary, 2 secondary, 3 sidearm, 4 melee,
    /// 5 throwable, 9 fishing rod).
    pub fn action_slot(&self, n: usize) -> Option<&Stack> {
        let slot = match n {
            1 => Slot::Primary,
            2 => Slot::Secondary,
            3 => Slot::Sidearm,
            4 => Slot::Melee,
            5 => Slot::Throwable,
            9 => Slot::FishingRod,
            _ => return None,
        };
        self.equipped.get(&slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_items_find_their_slots_and_bring_their_pockets() {
        let Ok(text) = std::fs::read_to_string("assets/data/items.json") else { return };
        let db: ItemDb = serde_json::from_str(&text).unwrap();
        let mut inv = PlayerInventory::default();
        assert_eq!(inv.slot_for(&db, "DA_SchoolBackpack"), Some(Slot::Backpack));
        assert_eq!(inv.slot_for(&db, "DA_WinterCoat"), Some(Slot::Torso));
        assert_eq!(inv.slot_for(&db, "DA_M4CQB"), Some(Slot::Primary));
        inv.equip(Slot::Backpack, db.stack("DA_SchoolBackpack", 1));
        assert_eq!(inv.grids().len(), 2, "pockets + the backpack's 4x5");
        // Fill the pockets, then items spill into the backpack.
        for _ in 0..9 {
            assert!(inv.insert(&db, db.stack("DA_CannedBakedBeans", 5)).is_none());
        }
        assert!(!inv.grids()[1].1.items.is_empty());
        // Crafting: rags from cloth, taken out of the backpack too.
        let rags = db.recipes["Player"].iter().find(|r| r.item == "DA_Rags").unwrap();
        assert_eq!(inv.craftable(rags), 0);
        let _ = inv.insert(&db, db.stack("DA_Cloth", 5));
        assert_eq!(inv.craftable(rags), 2);
        let made = inv.craft(&db, rags, 2).unwrap();
        assert_eq!((made.item.as_str(), made.count), ("DA_Rags", 4));
        assert_eq!(inv.count("DA_Cloth"), 1);
        assert!(inv.craft(&db, rags, 1).is_none());
        // Every slot's accepted type exists in the data.
        for slot in Slot::ROWS.iter().flat_map(|r| r.iter()) {
            assert!(db.items.values().any(|d| slot.fits(&d.kind)), "{slot:?} accepts nothing");
        }
    }
}

/// Clothing meshes the equipped items put on the survivor (male/female variant by sex).
pub fn worn_meshes(inv: &PlayerInventory, db: &ItemDb, female: bool) -> Vec<(String, String)> {
    inv.equipped
        .iter()
        .filter_map(|(slot, s)| {
            let c = db.items.get(&s.item)?.clothing.as_ref()?;
            let m = if female { c.female.clone().or(c.male.clone()) } else { c.male.clone().or(c.female.clone()) }?;
            Some((format!("{slot:?}"), m))
        })
        .collect()
}
