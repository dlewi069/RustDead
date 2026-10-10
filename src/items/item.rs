use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rarity {
    Common,    // Gray
    Uncommon,  // Green
    Rare,      // Blue
    Epic,      // Purple
    Legendary, // Orange
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemType {
    WeaponPrimary,
    WeaponSecondary,
    WeaponMelee,
    Ammo,
    Attachment,
    Medical,
    Food,
    Drink,
    Material,
    Container,
    ArmorHelmet,
    ArmorVest,
    Backpack,
    NVG,
    Radio,
    Tool,
    Keycard,
    Valuable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub description: String,
    pub item_type: ItemType,
    pub rarity: Rarity,
    pub weight_kg: f32,
    pub width: u8,
    pub height: u8,
    pub stack_size: u32,
    pub max_stack: u32,
    pub durability: Option<f32>, // 0.0 to 100.0
    pub max_durability: Option<f32>,
    pub nested_container_id: Option<String>,
}

impl Item {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        item_type: ItemType,
        rarity: Rarity,
        weight_kg: f32,
        width: u8,
        height: u8,
        max_stack: u32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            item_type,
            rarity,
            weight_kg,
            width,
            height,
            stack_size: 1,
            max_stack,
            durability: None,
            max_durability: None,
            nested_container_id: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_durability(mut self, max: f32) -> Self {
        self.durability = Some(max);
        self.max_durability = Some(max);
        self
    }

    pub fn with_stack(mut self, count: u32) -> Self {
        self.stack_size = count.min(self.max_stack);
        self
    }

    pub fn total_weight(&self) -> f32 {
        self.weight_kg * (self.stack_size as f32)
    }

    pub fn can_stack_with(&self, other: &Item) -> bool {
        self.id == other.id
            && self.stack_size < self.max_stack
            && self.durability.is_none()
            && other.durability.is_none()
    }
}
