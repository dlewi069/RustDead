use crate::items::{Item, ItemType, Rarity};
use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookingRecipe {
    pub id: String,
    pub name: String,
    pub input_item_id: String,
    pub input_count: u32,
    pub output_item: Item,
    pub cook_time_s: f32,
    pub required_heat_source: bool,
}

pub fn get_campfire_cooking_recipes() -> Vec<CookingRecipe> {
    vec![
        CookingRecipe {
            id: "cook_venison".into(),
            name: "Charred Venison Steak".into(),
            input_item_id: "raw_venison".into(),
            input_count: 1,
            output_item: Item::new(
                "cooked_steak",
                "Charred Venison Steak (+45 Food)",
                ItemType::Food,
                Rarity::Uncommon,
                0.4,
                1,
                1,
                5,
            ).with_description("Juicy venison steak seared over campfire embers."),
            cook_time_s: 3.5,
            required_heat_source: true,
        },
        CookingRecipe {
            id: "boil_water".into(),
            name: "Boil Purified Water".into(),
            input_item_id: "dirty_water".into(),
            input_count: 1,
            output_item: Item::new(
                "purified_water",
                "Boiled Purified Water (+40 Thirst)",
                ItemType::Drink,
                Rarity::Common,
                0.5,
                1,
                2,
                4,
            ).with_description("Purified water bottle with all bacteria boiled away."),
            cook_time_s: 2.5,
            required_heat_source: true,
        },
        CookingRecipe {
            id: "cook_stew".into(),
            name: "Hearty Hunter Stew".into(),
            input_item_id: "canned_peaches".into(),
            input_count: 2,
            output_item: Item::new(
                "hearty_stew",
                "Hearty Campfire Stew (+60 Food, +30 Thirst)",
                ItemType::Food,
                Rarity::Rare,
                0.6,
                1,
                2,
                4,
            ).with_description("Rich nourishing stew simmered in a cast-iron pot."),
            cook_time_s: 5.0,
            required_heat_source: true,
        },
        CookingRecipe {
            id: "roast_mushrooms".into(),
            name: "Roasted Wild Mushrooms".into(),
            input_item_id: "forest_mushrooms".into(),
            input_count: 2,
            output_item: Item::new(
                "roasted_mushrooms",
                "Roasted Garlic Mushrooms (+30 Food, +10 HP)",
                ItemType::Food,
                Rarity::Uncommon,
                0.2,
                1,
                1,
                10,
            ).with_description("Crispy grilled mushrooms with medicinal properties."),
            cook_time_s: 3.0,
            required_heat_source: true,
        },
        CookingRecipe {
            id: "cook_fish".into(),
            name: "Grilled Campfire Trout".into(),
            input_item_id: "raw_fish".into(),
            input_count: 1,
            output_item: Item::new(
                "cooked_fish",
                "Grilled Campfire Trout (+55 Food, +25 HP)",
                ItemType::Food,
                Rarity::Uncommon,
                0.7,
                1,
                2,
                10,
            ).with_description("Freshwater trout seared over hot coals. Highly nourishing."),
            cook_time_s: 3.5,
            required_heat_source: true,
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveCookingProcess {
    pub recipe_id: String,
    pub recipe_name: String,
    pub progress_s: f32,
    pub total_time_s: f32,
    pub output_item: Item,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampfireStation {
    pub position: Position3D,
    pub is_lit: bool,
    pub fuel_remaining_s: f32,
    pub active_cooking: Option<ActiveCookingProcess>,
}

impl Default for CampfireStation {
    fn default() -> Self {
        Self {
            position: Position3D::new(-10.0, 0.0, 12.0),
            is_lit: true,
            fuel_remaining_s: 300.0, // 5 minutes
            active_cooking: None,
        }
    }
}

impl CampfireStation {
    pub fn new(position: Position3D) -> Self {
        Self {
            position,
            is_lit: true,
            fuel_remaining_s: 300.0,
            active_cooking: None,
        }
    }

    pub fn add_wood_fuel(&mut self, wood_count: u32) {
        self.fuel_remaining_s += wood_count as f32 * 60.0;
        self.is_lit = true;
    }

    pub fn is_in_range(&self, player_pos: &Position3D, max_dist: f32) -> bool {
        self.position.distance_2d(player_pos) <= max_dist
    }

    /// Starts cooking a designated recipe
    pub fn start_cooking(&mut self, recipe: &CookingRecipe) -> Result<(), &'static str> {
        if !self.is_lit {
            return Err("Campfire is extinguished! Add wood fuel to ignite.");
        }
        if self.active_cooking.is_some() {
            return Err("Campfire is already cooking an item!");
        }

        self.active_cooking = Some(ActiveCookingProcess {
            recipe_id: recipe.id.clone(),
            recipe_name: recipe.name.clone(),
            progress_s: 0.0,
            total_time_s: recipe.cook_time_s,
            output_item: recipe.output_item.clone(),
        });

        Ok(())
    }

    /// Progresses burn time and cooking. Returns cooked item when finished.
    pub fn tick(&mut self, dt: f32) -> Option<Item> {
        let mut completed_item = None;

        if self.is_lit {
            self.fuel_remaining_s = (self.fuel_remaining_s - dt).max(0.0);
            if self.fuel_remaining_s <= 0.0 {
                self.is_lit = false;
            }

            if let Some(cooking) = &mut self.active_cooking {
                cooking.progress_s += dt;
                if cooking.progress_s >= cooking.total_time_s {
                    completed_item = Some(cooking.output_item.clone());
                }
            }

            if completed_item.is_some() {
                self.active_cooking = None;
            }
        }

        completed_item
    }

    pub fn cancel_cooking(&mut self) -> Option<ActiveCookingProcess> {
        self.active_cooking.take()
    }
}
