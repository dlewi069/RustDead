use crate::combat::create_weapon_preset;
use crate::items::{create_stock_item, Item, ItemType, Rarity};
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootTier {
    Civilian,
    Police,
    Military,
    Medical,
    Hardware,
    AirdropMilitary,
    MilitaryHighTier,
    Safe,
    Keycards,
    Infestation,
    Ammunition,
    BanditBoss,
    ZombieBoss,
    MilitaryBoss,
    Scavenger,
}

pub struct LootTable;

impl LootTable {
    pub fn roll_loot(tier: LootTier, count: usize) -> Vec<Item> {
        let mut rng = rand::thread_rng();
        let mut items = Vec::new();

        for _ in 0..count {
            let roll = rng.gen_range(0..100);
            let item = match tier {
                LootTier::Civilian => match roll {
                    0..=30 => create_stock_item("canned_peaches"),
                    31..=55 => create_stock_item("purified_water"),
                    56..=70 => create_stock_item("bandage"),
                    71..=82 => create_stock_item("wood_plank"),
                    83..=92 => create_stock_item("lockpick"),
                    _ => create_stock_item("hiking_backpack"),
                },
                LootTier::Medical => match roll {
                    0..=35 => create_stock_item("bandage"),
                    36..=55 => create_stock_item("antibiotics"),
                    56..=75 => create_stock_item("painkillers"),
                    76..=88 => create_stock_item("medkit"),
                    89..=95 => create_stock_item("large_medkit"),
                    _ => create_stock_item("radiation_pills"),
                },
                LootTier::Hardware => match roll {
                    0..=40 => create_stock_item("wood_plank"),
                    41..=70 => create_stock_item("scrap_metal"),
                    71..=85 => create_stock_item("gas_can"),
                    86..=93 => create_weapon_preset("crowbar").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Common, 2.5, 1, 3, 1).with_durability(100.0)),
                    _ => Some(Item::new("nails", "Box of Nails", ItemType::Material, Rarity::Common, 0.5, 1, 1, 50)),
                },
                LootTier::Police => match roll {
                    0..=30 => Some(Item::new("ammo_9mm", "9mm Ammo Box (50 rounds)", ItemType::Ammo, Rarity::Uncommon, 0.6, 1, 1, 10).with_stack(50)),
                    31..=50 => create_stock_item("bandage"),
                    51..=70 => create_weapon_preset("glock17").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponSecondary, Rarity::Uncommon, 1.1, 2, 2, 1).with_durability(100.0)),
                    71..=85 => create_weapon_preset("remington870").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Rare, 3.2, 1, 4, 1).with_durability(100.0)),
                    86..=94 => create_weapon_preset("ar45").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Rare, 2.8, 2, 3, 1).with_durability(100.0)),
                    _ => Some(Item::new("police_vest", "Police Kevlar Vest", ItemType::ArmorVest, Rarity::Rare, 4.0, 2, 2, 1).with_durability(100.0)),
                },
                LootTier::Military => match roll {
                    0..=25 => Some(Item::new("ammo_556", "5.56x45mm Ammo (60 rounds)", ItemType::Ammo, Rarity::Rare, 0.9, 1, 1, 10).with_stack(60)),
                    26..=45 => Some(Item::new("ammo_762", "7.62x39mm Ammo (60 rounds)", ItemType::Ammo, Rarity::Rare, 1.1, 1, 1, 10).with_stack(60)),
                    46..=65 => create_weapon_preset("m4a1").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 3.4, 2, 4, 1).with_durability(100.0)),
                    66..=80 => create_weapon_preset("ak47").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 3.8, 2, 4, 1).with_durability(100.0)),
                    81..=90 => create_weapon_preset("hunting_rifle").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Legendary, 4.5, 1, 5, 1).with_durability(100.0)),
                    _ => create_stock_item("military_rucksack"),
                },
                // Authentic SurrounDead DA_LootTable_AirdropMilitary
                LootTier::AirdropMilitary => match roll {
                    0..=12 => create_weapon_preset("barrett_m82").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Legendary, 14.0, 2, 6, 1).with_durability(100.0)),
                    13..=25 => create_weapon_preset("m249").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 7.5, 2, 5, 1).with_durability(100.0)),
                    26..=38 => create_weapon_preset("scar_h").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 4.2, 2, 4, 1).with_durability(100.0)),
                    39..=50 => create_weapon_preset("kriss_vector").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 3.0, 2, 3, 1).with_durability(100.0)),
                    51..=62 => create_weapon_preset("benelli_m4").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 3.8, 1, 5, 1).with_durability(100.0)),
                    63..=75 => create_stock_item("large_black_military_backpack"),
                    76..=85 => create_stock_item("large_medkit"),
                    86..=93 => create_stock_item("ammo_tin"),
                    _ => create_stock_item("keycard_lvl3"),
                },
                // Authentic SurrounDead DA_LootTable_MilitaryHighTier
                LootTier::MilitaryHighTier => match roll {
                    0..=18 => create_weapon_preset("scar_h").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 4.2, 2, 4, 1).with_durability(100.0)),
                    19..=36 => create_weapon_preset("steyr_aug").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 3.6, 2, 4, 1).with_durability(100.0)),
                    37..=52 => create_weapon_preset("p90").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 2.6, 2, 3, 1).with_durability(100.0)),
                    53..=68 => create_weapon_preset("deagle").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponSecondary, Rarity::Rare, 2.0, 2, 2, 1).with_durability(100.0)),
                    69..=80 => create_stock_item("coyote_backpack"),
                    81..=90 => create_stock_item("armor_repair_kit"),
                    _ => create_stock_item("keycard_lvl2"),
                },
                // Authentic SurrounDead DA_LootTable_Safe
                LootTier::Safe => match roll {
                    0..=35 => create_stock_item("money"),
                    36..=60 => create_stock_item("credits"),
                    61..=75 => create_weapon_preset("deagle").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponSecondary, Rarity::Rare, 2.0, 2, 2, 1).with_durability(100.0)),
                    76..=88 => create_stock_item("keycard_lvl2"),
                    89..=96 => create_stock_item("keycard_lvl3"),
                    _ => create_weapon_preset("battle_ready_glock").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponSecondary, Rarity::Legendary, 1.5, 2, 2, 1).with_durability(100.0)),
                },
                // Authentic SurrounDead DA_LootTable_Keycards
                LootTier::Keycards => match roll {
                    0..=45 => create_stock_item("keycard_lvl0"),
                    46..=75 => create_stock_item("keycard_lvl1"),
                    76..=92 => create_stock_item("keycard_lvl2"),
                    _ => create_stock_item("keycard_lvl3"),
                },
                // Authentic SurrounDead DA_LootTable_Infestation
                LootTier::Infestation => match roll {
                    0..=20 => create_stock_item("radiation_pills"),
                    21..=40 => create_weapon_preset("cerberus").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Legendary, 4.8, 2, 5, 1).with_durability(100.0)),
                    41..=60 => create_weapon_preset("lechie").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Legendary, 6.0, 2, 6, 1).with_durability(100.0)),
                    61..=80 => create_weapon_preset("katana").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Legendary, 1.2, 1, 4, 1).with_durability(100.0)),
                    _ => create_stock_item("tier3_loot_box"),
                },
                // Authentic SurrounDead DA_LootTable_Ammunition
                LootTier::Ammunition => match roll {
                    0..=20 => Some(Item::new("ammo_9mm", "9mm Ammo Box (50 rounds)", ItemType::Ammo, Rarity::Common, 0.6, 1, 1, 10).with_stack(50)),
                    21..=40 => Some(Item::new("ammo_556", "5.56x45mm Ammo (60 rounds)", ItemType::Ammo, Rarity::Rare, 0.9, 1, 1, 10).with_stack(60)),
                    41..=60 => Some(Item::new("ammo_762", "7.62x39mm Ammo (60 rounds)", ItemType::Ammo, Rarity::Rare, 1.1, 1, 1, 10).with_stack(60)),
                    61..=75 => Some(Item::new("ammo_12gauge", "12 Gauge Buckshot (25 shells)", ItemType::Ammo, Rarity::Uncommon, 1.2, 1, 1, 10).with_stack(25)),
                    76..=88 => Some(Item::new("ammo_308", ".308 Win Ammo (40 rounds)", ItemType::Ammo, Rarity::Rare, 1.0, 1, 1, 10).with_stack(40)),
                    _ => create_stock_item("ammo_tin"),
                },
                // Authentic SurrounDead DA_LootTable_BanditBoss
                LootTier::BanditBoss => match roll {
                    0..=30 => create_weapon_preset("crusher").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Legendary, 4.5, 2, 4, 1).with_durability(100.0)),
                    31..=60 => create_weapon_preset("scar_h").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 4.2, 2, 4, 1).with_durability(100.0)),
                    61..=80 => create_stock_item("money"),
                    81..=92 => create_stock_item("alice_backpack"),
                    _ => create_stock_item("keycard_lvl2"),
                },
                // Authentic SurrounDead DA_LootTable_ZombieBoss
                LootTier::ZombieBoss => match roll {
                    0..=30 => create_weapon_preset("barrett_m82").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Legendary, 14.0, 2, 6, 1).with_durability(100.0)),
                    31..=55 => create_weapon_preset("xm250").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Epic, 8.0, 2, 5, 1).with_durability(100.0)),
                    56..=75 => create_stock_item("tier3_loot_box"),
                    76..=90 => create_stock_item("large_medkit"),
                    _ => create_stock_item("keycard_lvl3"),
                },
                // Authentic SurrounDead DA_LootTable_MilitaryBoss
                LootTier::MilitaryBoss => match roll {
                    0..=35 => create_weapon_preset("colonels_revenge").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Legendary, 3.8, 2, 4, 1).with_durability(100.0)),
                    36..=65 => create_weapon_preset("valkyrie").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Legendary, 3.7, 2, 4, 1).with_durability(100.0)),
                    66..=85 => create_stock_item("large_black_military_backpack"),
                    _ => create_stock_item("keycard_lvl3"),
                },
                // Authentic SurrounDead DA_LootTable_Scavenger
                LootTier::Scavenger => match roll {
                    0..=35 => create_weapon_preset("hunting_rifle").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Rare, 4.0, 1, 5, 1).with_durability(100.0)),
                    36..=65 => create_weapon_preset("remington870").map(|w| Item::new(w.weapon_id, w.name, ItemType::WeaponPrimary, Rarity::Rare, 3.2, 1, 4, 1).with_durability(100.0)),
                    66..=85 => create_stock_item("hiking_backpack"),
                    _ => create_stock_item("scrap_metal"),
                },
            };

            if let Some(it) = item {
                items.push(it);
            }
        }

        items
    }
}
