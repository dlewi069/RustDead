use crate::items::{create_stock_item, Item, ItemType, Rarity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StationType {
    Handmade,
    ConstructionBench,
    AmmoBench,
    MedicalStation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftingIngredient {
    pub item_id: String,
    pub amount: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftingRecipe {
    pub id: String,
    pub name: String,
    pub station: StationType,
    pub ingredients: Vec<CraftingIngredient>,
    pub result_item_id: String,
    pub result_amount: u32,
    pub craft_time_s: f32,
}

impl CraftingRecipe {
    pub fn get_all_recipes() -> Vec<CraftingRecipe> {
        vec![
            CraftingRecipe {
                id: "craft_bandage".into(),
                name: "Craft Sterile Bandage".into(),
                station: StationType::Handmade,
                ingredients: vec![CraftingIngredient { item_id: "cloth".into(), amount: 2 }],
                result_item_id: "bandage".into(),
                result_amount: 1,
                craft_time_s: 1.5,
            },
            CraftingRecipe {
                id: "craft_wood_wall".into(),
                name: "Construct Wooden Wall".into(),
                station: StationType::ConstructionBench,
                ingredients: vec![
                    CraftingIngredient { item_id: "wood_plank".into(), amount: 4 },
                    CraftingIngredient { item_id: "nails".into(), amount: 8 },
                ],
                result_item_id: "wooden_wall".into(),
                result_amount: 1,
                craft_time_s: 3.0,
            },
            CraftingRecipe {
                id: "craft_wooden_door".into(),
                name: "Construct Fortified Door".into(),
                station: StationType::ConstructionBench,
                ingredients: vec![
                    CraftingIngredient { item_id: "wood_plank".into(), amount: 3 },
                    CraftingIngredient { item_id: "scrap_metal".into(), amount: 2 },
                    CraftingIngredient { item_id: "nails".into(), amount: 6 },
                ],
                result_item_id: "wooden_door".into(),
                result_amount: 1,
                craft_time_s: 4.0,
            },
            CraftingRecipe {
                id: "craft_storage_chest".into(),
                name: "Build Wooden Storage Chest".into(),
                station: StationType::ConstructionBench,
                ingredients: vec![
                    CraftingIngredient { item_id: "wood_plank".into(), amount: 6 },
                    CraftingIngredient { item_id: "nails".into(), amount: 10 },
                ],
                result_item_id: "storage_chest".into(),
                result_amount: 1,
                craft_time_s: 4.5,
            },
            CraftingRecipe {
                id: "craft_campfire".into(),
                name: "Build Campfire".into(),
                station: StationType::Handmade,
                ingredients: vec![
                    CraftingIngredient { item_id: "wood_plank".into(), amount: 2 },
                    CraftingIngredient { item_id: "stone".into(), amount: 4 },
                ],
                result_item_id: "campfire".into(),
                result_amount: 1,
                craft_time_s: 2.0,
            },
            CraftingRecipe {
                id: "craft_ammo_556".into(),
                name: "Reload 5.56x45mm Rounds".into(),
                station: StationType::AmmoBench,
                ingredients: vec![
                    CraftingIngredient { item_id: "scrap_metal".into(), amount: 2 },
                    CraftingIngredient { item_id: "gunpowder".into(), amount: 3 },
                ],
                result_item_id: "ammo_556".into(),
                result_amount: 30,
                craft_time_s: 5.0,
            },
            CraftingRecipe {
                id: "craft_wood_planks".into(),
                name: "Saw Pine Log into Planks".into(),
                station: StationType::Handmade,
                ingredients: vec![
                    CraftingIngredient { item_id: "wood_log".into(), amount: 1 },
                ],
                result_item_id: "wood_plank".into(),
                result_amount: 3,
                craft_time_s: 1.5,
            },
            CraftingRecipe {
                id: "craft_antibiotics".into(),
                name: "Synthesize Antibiotics".into(),
                station: StationType::MedicalStation,
                ingredients: vec![
                    CraftingIngredient { item_id: "purified_water".into(), amount: 1 },
                    CraftingIngredient { item_id: "medicinal_herbs".into(), amount: 3 },
                    CraftingIngredient { item_id: "alcohol".into(), amount: 1 },
                ],
                result_item_id: "antibiotics".into(),
                result_amount: 1,
                craft_time_s: 6.0,
            },
            CraftingRecipe {
                id: "craft_horde_beacon".into(),
                name: "Construct Horde Beacon Device".into(),
                station: StationType::ConstructionBench,
                ingredients: vec![
                    CraftingIngredient { item_id: "scrap_metal".into(), amount: 4 },
                    CraftingIngredient { item_id: "lockpick".into(), amount: 2 },
                ],
                result_item_id: "horde_beacon".into(),
                result_amount: 1,
                craft_time_s: 8.0,
            },
            CraftingRecipe {
                id: "craft_suppressor".into(),
                name: "Fabricate Tactical Suppressor".into(),
                station: StationType::ConstructionBench,
                ingredients: vec![
                    CraftingIngredient { item_id: "scrap_metal".into(), amount: 5 },
                ],
                result_item_id: "muzzle_suppressor".into(),
                result_amount: 1,
                craft_time_s: 5.0,
            },
        ]
    }

    pub fn instantiate_result(&self) -> Option<Item> {
        match self.result_item_id.as_str() {
            "bandage" => create_stock_item("bandage"),
            "wood_plank" => create_stock_item("wood_plank").map(|i| i.with_stack(self.result_amount)),
            "wooden_wall" => Some(Item::new("wooden_wall", "Wooden Wall Kit", ItemType::Material, Rarity::Common, 6.0, 2, 2, 5)),
            "wooden_door" => Some(Item::new("wooden_door", "Wooden Door Kit", ItemType::Material, Rarity::Common, 5.0, 2, 2, 5)),
            "storage_chest" => Some(Item::new("storage_chest", "Storage Chest", ItemType::Container, Rarity::Uncommon, 7.5, 2, 3, 2)),
            "campfire" => Some(Item::new("campfire", "Campfire Kit", ItemType::Material, Rarity::Common, 3.0, 2, 2, 2)),
            "ammo_556" => Some(Item::new("ammo_556", "5.56x45mm Ammo", ItemType::Ammo, Rarity::Rare, 0.5, 1, 1, 60).with_stack(self.result_amount)),
            "antibiotics" => create_stock_item("antibiotics"),
            "horde_beacon" => create_stock_item("horde_beacon"),
            "muzzle_suppressor" => create_stock_item("muzzle_suppressor"),
            _ => None,
        }
    }
}
