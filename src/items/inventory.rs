use super::item::Item;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedItem {
    pub item: Item,
    pub x: u8,
    pub y: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridContainer {
    pub id: String,
    pub name: String,
    pub width: u8,
    pub height: u8,
    pub max_weight_kg: f32,
    pub items: Vec<PlacedItem>,
}

impl GridContainer {
    pub fn new(id: impl Into<String>, name: impl Into<String>, width: u8, height: u8, max_weight: f32) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            width,
            height,
            max_weight_kg: max_weight,
            items: Vec::new(),
        }
    }

    /// Checks if an item of (w, h) fits at (gx, gy) without colliding with other items
    pub fn can_place_at(&self, gx: u8, gy: u8, w: u8, h: u8, ignore_index: Option<usize>) -> bool {
        if gx + w > self.width || gy + h > self.height {
            return false;
        }

        for (idx, placed) in self.items.iter().enumerate() {
            if Some(idx) == ignore_index {
                continue;
            }

            let px = placed.x;
            let py = placed.y;
            let pw = placed.item.width;
            let ph = placed.item.height;

            let overlap_x = gx < px + pw && gx + w > px;
            let overlap_y = gy < py + ph && gy + h > py;

            if overlap_x && overlap_y {
                return false;
            }
        }

        true
    }

    /// Finds first available (x, y) slot where the item fits
    pub fn find_free_slot(&self, w: u8, h: u8) -> Option<(u8, u8)> {
        for y in 0..=(self.height.saturating_sub(h)) {
            for x in 0..=(self.width.saturating_sub(w)) {
                if self.can_place_at(x, y, w, h, None) {
                    return Some((x, y));
                }
            }
        }
        None
    }

    /// Insert an item automatically into the first fitting position, stacking if possible
    pub fn auto_insert(&mut self, mut item: Item) -> Result<(), Item> {
        // 1. Try stacking into existing item if stackable
        if item.max_stack > 1 {
            for placed in &mut self.items {
                if placed.item.can_stack_with(&item) {
                    let space = placed.item.max_stack - placed.item.stack_size;
                    let to_add = space.min(item.stack_size);
                    placed.item.stack_size += to_add;
                    item.stack_size -= to_add;

                    if item.stack_size == 0 {
                        return Ok(());
                    }
                }
            }
        }

        // 2. Check total weight limit
        if self.current_weight() + item.total_weight() > self.max_weight_kg {
            return Err(item);
        }

        // 3. Find grid coordinates
        if let Some((x, y)) = self.find_free_slot(item.width, item.height) {
            self.items.push(PlacedItem { item, x, y });
            Ok(())
        } else {
            Err(item)
        }
    }

    /// Place item at exact grid coordinates
    pub fn place_at(&mut self, item: Item, x: u8, y: u8) -> Result<(), Item> {
        if self.can_place_at(x, y, item.width, item.height, None) {
            if self.current_weight() + item.total_weight() <= self.max_weight_kg {
                self.items.push(PlacedItem { item, x, y });
                Ok(())
            } else {
                Err(item)
            }
        } else {
            Err(item)
        }
    }

    pub fn remove_at(&mut self, index: usize) -> Option<Item> {
        if index < self.items.len() {
            Some(self.items.remove(index).item)
        } else {
            None
        }
    }

    pub fn current_weight(&self) -> f32 {
        self.items.iter().map(|p| p.item.total_weight()).sum()
    }

    pub fn count_item(&self, item_id: &str) -> u32 {
        self.items
            .iter()
            .filter(|p| p.item.id == item_id)
            .map(|p| p.item.stack_size)
            .sum()
    }

    pub fn remove_amount(&mut self, item_id: &str, mut amount: u32) -> bool {
        if self.count_item(item_id) < amount {
            return false;
        }

        let mut i = 0;
        while i < self.items.len() && amount > 0 {
            if self.items[i].item.id == item_id {
                if self.items[i].item.stack_size <= amount {
                    amount -= self.items[i].item.stack_size;
                    self.items.remove(i);
                } else {
                    self.items[i].item.stack_size -= amount;
                    amount = 0;
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        true
    }
}
