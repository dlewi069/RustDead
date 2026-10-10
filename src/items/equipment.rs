use super::inventory::GridContainer;
use super::item::{Item, ItemType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equipment {
    pub primary_weapon: Option<Item>,
    pub secondary_weapon: Option<Item>,
    pub melee_weapon: Option<Item>,
    pub helmet: Option<Item>,
    pub vest: Option<Item>,
    pub backpack: Option<Item>,
    pub nvg: Option<Item>,
    pub radio: Option<Item>,
    // The main on-person grid (pockets / rig)
    pub pockets: GridContainer,
    // Nested storage grids mapped by container ID (e.g., equipped backpack or nested pouches)
    pub containers: std::collections::HashMap<String, GridContainer>,
}

impl Default for Equipment {
    fn default() -> Self {
        Self {
            primary_weapon: None,
            secondary_weapon: None,
            melee_weapon: None,
            helmet: None,
            vest: None,
            backpack: None,
            nvg: None,
            radio: None,
            pockets: GridContainer::new("pockets", "Pockets", 6, 4, 18.0), // 6x4 grid = 24 slots, 18kg
            containers: std::collections::HashMap::new(),
        }
    }
}

impl Equipment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn equip_item(&mut self, item: Item) -> Result<Option<Item>, Item> {
        match item.item_type {
            ItemType::WeaponPrimary => {
                let prev = self.primary_weapon.replace(item);
                Ok(prev)
            }
            ItemType::WeaponSecondary => {
                let prev = self.secondary_weapon.replace(item);
                Ok(prev)
            }
            ItemType::WeaponMelee => {
                let prev = self.melee_weapon.replace(item);
                Ok(prev)
            }
            ItemType::ArmorHelmet => {
                let prev = self.helmet.replace(item);
                Ok(prev)
            }
            ItemType::ArmorVest => {
                let prev = self.vest.replace(item);
                Ok(prev)
            }
            ItemType::NVG => {
                let prev = self.nvg.replace(item);
                Ok(prev)
            }
            ItemType::Radio => {
                let prev = self.radio.replace(item);
                Ok(prev)
            }
            ItemType::Backpack => {
                // Ensure grid container exists for this backpack
                let container_id = item
                    .nested_container_id
                    .clone()
                    .unwrap_or_else(|| format!("backpack_{}", item.id));

                if !self.containers.contains_key(&container_id) {
                    // Military backpack is 6x8 = 48 slots, 45kg capacity
                    let (w, h, max_w) = match item.id.as_str() {
                        "duffel_bag" => (5, 5, 25.0),
                        "hiking_backpack" => (6, 6, 35.0),
                        "military_rucksack" => (6, 8, 50.0),
                        _ => (4, 5, 20.0),
                    };
                    self.containers.insert(
                        container_id.clone(),
                        GridContainer::new(&container_id, &item.name, w, h, max_w),
                    );
                }

                let prev = self.backpack.replace(item);
                Ok(prev)
            }
            _ => Err(item), // Cannot equip non-equippable item directly
        }
    }

    pub fn get_backpack_grid_mut(&mut self) -> Option<&mut GridContainer> {
        if let Some(backpack) = &self.backpack {
            if let Some(ref cid) = backpack.nested_container_id {
                return self.containers.get_mut(cid);
            }
            let default_id = format!("backpack_{}", backpack.id);
            return self.containers.get_mut(&default_id);
        }
        None
    }

    pub fn get_backpack_grid(&self) -> Option<&GridContainer> {
        if let Some(backpack) = &self.backpack {
            if let Some(ref cid) = backpack.nested_container_id {
                return self.containers.get(cid);
            }
            let default_id = format!("backpack_{}", backpack.id);
            return self.containers.get(&default_id);
        }
        None
    }

    /// Insert item into pockets, or overflow into equipped backpack if present
    pub fn auto_insert_carried(&mut self, item: Item) -> Result<(), Item> {
        match self.pockets.auto_insert(item) {
            Ok(_) => Ok(()),
            Err(item) => {
                if let Some(bp) = self.get_backpack_grid_mut() {
                    bp.auto_insert(item)
                } else {
                    Err(item)
                }
            }
        }
    }

    pub fn total_carried_weight(&self) -> f32 {
        let mut total = self.pockets.current_weight();

        if let Some(w) = &self.primary_weapon { total += w.total_weight(); }
        if let Some(w) = &self.secondary_weapon { total += w.total_weight(); }
        if let Some(w) = &self.melee_weapon { total += w.total_weight(); }
        if let Some(w) = &self.helmet { total += w.total_weight(); }
        if let Some(w) = &self.vest { total += w.total_weight(); }
        if let Some(w) = &self.backpack { total += w.total_weight(); }
        if let Some(w) = &self.nvg { total += w.total_weight(); }
        if let Some(w) = &self.radio { total += w.total_weight(); }

        for grid in self.containers.values() {
            total += grid.current_weight();
        }

        total
    }

    pub fn armor_damage_reduction(&self) -> f32 {
        let mut reduction = 0.0;
        if self.helmet.is_some() {
            reduction += 0.20; // 20% protection from helmet
        }
        if self.vest.is_some() {
            reduction += 0.35; // 35% protection from tactical vest
        }
        reduction
    }

    pub fn has_item(&self, item_id: &str) -> bool {
        if self.pockets.count_item(item_id) > 0 {
            return true;
        }
        if let Some(bp) = self.get_backpack_grid() {
            if bp.count_item(item_id) > 0 {
                return true;
            }
        }
        false
    }

    pub fn find_and_remove_item(&mut self, item_id: &str) -> Option<Item> {
        // Try pockets first
        if let Some(pos) = self.pockets.items.iter().position(|p| p.item.id == item_id) {
            let mut item = self.pockets.items[pos].item.clone();
            if self.pockets.items[pos].item.stack_size > 1 {
                self.pockets.items[pos].item.stack_size -= 1;
                item.stack_size = 1;
                return Some(item);
            } else {
                return Some(self.pockets.items.remove(pos).item);
            }
        }
        // Try backpack
        if let Some(bp) = self.get_backpack_grid_mut() {
            if let Some(pos) = bp.items.iter().position(|p| p.item.id == item_id) {
                let mut item = bp.items[pos].item.clone();
                if bp.items[pos].item.stack_size > 1 {
                    bp.items[pos].item.stack_size -= 1;
                    item.stack_size = 1;
                    return Some(item);
                } else {
                    return Some(bp.items.remove(pos).item);
                }
            }
        }
        None
    }

    pub fn find_all_attachments(&self) -> Vec<Item> {
        let mut list = Vec::new();
        for placed in &self.pockets.items {
            if placed.item.item_type == ItemType::Attachment
                || placed.item.id.starts_with("optic_")
                || placed.item.id.starts_with("muzzle_")
                || placed.item.id.starts_with("mag_")
                || placed.item.id.starts_with("tactical_")
            {
                list.push(placed.item.clone());
            }
        }
        if let Some(bp) = self.get_backpack_grid() {
            for placed in &bp.items {
                if placed.item.item_type == ItemType::Attachment
                    || placed.item.id.starts_with("optic_")
                    || placed.item.id.starts_with("muzzle_")
                    || placed.item.id.starts_with("mag_")
                    || placed.item.id.starts_with("tactical_")
                {
                    list.push(placed.item.clone());
                }
            }
        }
        list
    }
}
