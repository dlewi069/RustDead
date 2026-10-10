use crate::combat::create_weapon_preset;
use crate::items::{create_stock_item, Item, ItemType, Rarity};
use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraderOffer {
    pub item: Item,
    pub price_credits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trader {
    pub id: u32,
    pub name: String,
    pub title: String,
    pub position: Position3D,
    pub credits: u32,
    pub offers: Vec<TraderOffer>,
}

impl Trader {
    pub fn new_safezone_merchant(id: u32, position: Position3D) -> Self {
        let mut offers = Vec::new();

        // Medical stock
        if let Some(med) = create_stock_item("medkit") {
            offers.push(TraderOffer { item: med, price_credits: 120 });
        }
        if let Some(anti) = create_stock_item("antibiotics") {
            offers.push(TraderOffer { item: anti, price_credits: 85 });
        }
        if let Some(band) = create_stock_item("bandage") {
            offers.push(TraderOffer { item: band, price_credits: 25 });
        }

        // Ammo stock
        let ammo9 = Item::new("ammo_9mm", "9mm Ammo Box (50 rounds)", ItemType::Ammo, Rarity::Uncommon, 0.6, 1, 1, 10).with_stack(50);
        offers.push(TraderOffer { item: ammo9, price_credits: 60 });

        let ammo556 = Item::new("ammo_556", "5.56 NATO Ammo (60 rounds)", ItemType::Ammo, Rarity::Rare, 0.9, 1, 1, 10).with_stack(60);
        offers.push(TraderOffer { item: ammo556, price_credits: 140 });

        // Weapon stock
        if let Some(ak) = create_weapon_preset("ak47") {
            let ak_item = Item::new(ak.weapon_id, ak.name, ItemType::WeaponPrimary, Rarity::Epic, 3.8, 2, 4, 1).with_durability(100.0);
            offers.push(TraderOffer { item: ak_item, price_credits: 450 });
        }
        if let Some(rem) = create_weapon_preset("remington870") {
            let rem_item = Item::new(rem.weapon_id, rem.name, ItemType::WeaponPrimary, Rarity::Rare, 3.2, 1, 4, 1).with_durability(100.0);
            offers.push(TraderOffer { item: rem_item, price_credits: 280 });
        }

        // Food & Survival stock
        if let Some(food) = create_stock_item("canned_peaches") {
            offers.push(TraderOffer { item: food, price_credits: 30 });
        }
        if let Some(water) = create_stock_item("purified_water") {
            offers.push(TraderOffer { item: water, price_credits: 20 });
        }

        // Backpack stock
        if let Some(bag) = create_stock_item("military_rucksack") {
            offers.push(TraderOffer { item: bag, price_credits: 350 });
        }

        // Field Equipment
        if let Some(picks) = create_stock_item("lockpick") {
            offers.push(TraderOffer { item: picks, price_credits: 40 });
        }
        if let Some(gas) = create_stock_item("gas_can") {
            offers.push(TraderOffer { item: gas, price_credits: 90 });
        }

        Self {
            id,
            name: "Hank the Quartermaster".into(),
            title: "Outpost Trader".into(),
            position,
            credits: 2500,
            offers,
        }
    }

    /// Calculate valuation when player sells an item to the trader
    pub fn evaluate_item_value(&self, item: &Item) -> u32 {
        let base = match item.rarity {
            Rarity::Common => 15,
            Rarity::Uncommon => 35,
            Rarity::Rare => 85,
            Rarity::Epic => 200,
            Rarity::Legendary => 450,
        };

        let type_mult = match item.item_type {
            ItemType::WeaponPrimary => 1.8,
            ItemType::WeaponSecondary => 1.3,
            ItemType::ArmorVest | ItemType::Backpack => 1.5,
            ItemType::Medical => 1.2,
            ItemType::Ammo => 0.8,
            _ => 1.0,
        };

        let dur_mult = match (item.durability, item.max_durability) {
            (Some(d), Some(md)) if md > 0.0 => (d / md).clamp(0.1, 1.0),
            _ => 1.0,
        };
        let stack = item.stack_size.max(1) as f32;

        ((base as f32) * type_mult * dur_mult * (stack / item.max_stack.max(1) as f32)).round().max(1.0) as u32
    }

    /// Player sells an item to trader. Trader gives player credits.
    pub fn buy_from_player(&mut self, item: &Item) -> Result<u32, &'static str> {
        let val = self.evaluate_item_value(item);
        if self.credits < val {
            return Err("Trader does not have enough credits!");
        }

        self.credits -= val;
        Ok(val)
    }

    /// Player buys an item from trader offer
    pub fn sell_to_player(&mut self, offer_index: usize, player_credits: u32) -> Result<(Item, u32), &'static str> {
        if offer_index >= self.offers.len() {
            return Err("Invalid trade offer index!");
        }

        let offer = &self.offers[offer_index];
        if player_credits < offer.price_credits {
            return Err("Not enough credits to purchase item!");
        }

        let price = offer.price_credits;
        let item = self.offers.remove(offer_index).item;
        self.credits += price;

        Ok((item, price))
    }
}
