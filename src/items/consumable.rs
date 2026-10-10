use crate::survival::SurvivalStats;
use super::item::{Item, ItemType, Rarity};

#[derive(Debug, Clone)]
pub struct ConsumableEffect {
    pub health: f32,
    pub hunger: f32,
    pub thirst: f32,
    pub stamina: f32,
    pub infection_heal: f32,
    pub radiation_heal: f32,
    pub stops_bleeding: bool,
}

impl ConsumableEffect {
    pub fn apply(&self, stats: &mut SurvivalStats) {
        if self.health > 0.0 { stats.heal(self.health); }
        if self.hunger > 0.0 || self.thirst > 0.0 { stats.eat_food(self.hunger, self.thirst); }
        if self.stamina > 0.0 { stats.stamina = (stats.stamina + self.stamina).min(stats.max_stamina); }
        if self.stops_bleeding { stats.bleeding = false; }
        if self.infection_heal > 0.0 {
            stats.infection = (stats.infection - self.infection_heal).max(0.0);
        }
        if self.radiation_heal > 0.0 {
            stats.radiation = (stats.radiation - self.radiation_heal).max(0.0);
        }
    }
}

pub fn get_consumable_effect(item_id: &str) -> Option<ConsumableEffect> {
    match item_id {
        "bandage" => Some(ConsumableEffect {
            health: 15.0,
            hunger: 0.0,
            thirst: 0.0,
            stamina: 0.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: true,
        }),
        "medkit" => Some(ConsumableEffect {
            health: 65.0,
            hunger: 0.0,
            thirst: 0.0,
            stamina: 0.0,
            infection_heal: 10.0,
            radiation_heal: 0.0,
            stops_bleeding: true,
        }),
        "antibiotics" => Some(ConsumableEffect {
            health: 5.0,
            hunger: 0.0,
            thirst: 0.0,
            stamina: 0.0,
            infection_heal: 45.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "purified_water" => Some(ConsumableEffect {
            health: 0.0,
            hunger: 0.0,
            thirst: 50.0,
            stamina: 10.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "canned_peaches" => Some(ConsumableEffect {
            health: 5.0,
            hunger: 35.0,
            thirst: 15.0,
            stamina: 0.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "mre_ration" => Some(ConsumableEffect {
            health: 10.0,
            hunger: 80.0,
            thirst: 20.0,
            stamina: 25.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "anti_rad_pills" | "radiation_pills" => Some(ConsumableEffect {
            health: 0.0,
            hunger: 0.0,
            thirst: 0.0,
            stamina: 0.0,
            infection_heal: 0.0,
            radiation_heal: 50.0,
            stops_bleeding: false,
        }),
        "large_medkit" => Some(ConsumableEffect {
            health: 100.0,
            hunger: 0.0,
            thirst: 0.0,
            stamina: 25.0,
            infection_heal: 25.0,
            radiation_heal: 15.0,
            stops_bleeding: true,
        }),
        "painkillers" => Some(ConsumableEffect {
            health: 20.0,
            hunger: 0.0,
            thirst: 0.0,
            stamina: 50.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "splint" => Some(ConsumableEffect {
            health: 10.0,
            hunger: 0.0,
            thirst: 0.0,
            stamina: 20.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "raw_venison" => Some(ConsumableEffect {
            health: 5.0,
            hunger: 15.0,
            thirst: 0.0,
            stamina: 0.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "cooked_steak" => Some(ConsumableEffect {
            health: 20.0,
            hunger: 45.0,
            thirst: 5.0,
            stamina: 10.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "dirty_water" => Some(ConsumableEffect {
            health: -5.0,
            hunger: 0.0,
            thirst: 20.0,
            stamina: -5.0,
            infection_heal: -5.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "hearty_stew" => Some(ConsumableEffect {
            health: 25.0,
            hunger: 60.0,
            thirst: 30.0,
            stamina: 20.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "forest_mushrooms" => Some(ConsumableEffect {
            health: 2.0,
            hunger: 10.0,
            thirst: 5.0,
            stamina: 0.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "roasted_mushrooms" => Some(ConsumableEffect {
            health: 10.0,
            hunger: 30.0,
            thirst: 10.0,
            stamina: 10.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "raw_fish" => Some(ConsumableEffect {
            health: 5.0,
            hunger: 20.0,
            thirst: 5.0,
            stamina: 5.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        "cooked_fish" => Some(ConsumableEffect {
            health: 25.0,
            hunger: 55.0,
            thirst: 10.0,
            stamina: 20.0,
            infection_heal: 0.0,
            radiation_heal: 0.0,
            stops_bleeding: false,
        }),
        _ => None,
    }
}

pub fn create_stock_item(item_id: &str) -> Option<Item> {
    match item_id {
        "bandage" => Some(
            Item::new("bandage", "Sterile Bandage", ItemType::Medical, Rarity::Common, 0.1, 1, 1, 5)
                .with_description("Halts active bleeding and provides minor health recovery."),
        ),
        "medkit" => Some(
            Item::new("medkit", "Military First Aid Kit", ItemType::Medical, Rarity::Rare, 1.2, 2, 2, 2)
                .with_description("Comprehensive emergency trauma kit. Restores significant health."),
        ),
        "large_medkit" => Some(
            Item::new("large_medkit", "Large Military Trauma Kit", ItemType::Medical, Rarity::Epic, 2.0, 2, 2, 2)
                .with_description("Advanced military surgical kit. Full recovery and stops bleeding."),
        ),
        "antibiotics" => Some(
            Item::new("antibiotics", "Broad-Spectrum Antibiotics", ItemType::Medical, Rarity::Uncommon, 0.05, 1, 1, 10)
                .with_description("Clears viral infection and halts zombie sickness progression."),
        ),
        "painkillers" => Some(
            Item::new("painkillers", "Painkiller Bottle", ItemType::Medical, Rarity::Uncommon, 0.1, 1, 1, 10)
                .with_description("Analgesic pills that boost stamina and provide light pain relief."),
        ),
        "radiation_pills" | "anti_rad_pills" => Some(
            Item::new("radiation_pills", "Potassium Iodide Anti-Rad Pills", ItemType::Medical, Rarity::Rare, 0.05, 1, 1, 10)
                .with_description("Purges acute radiation toxicity from hazardous contamination zones."),
        ),
        "splint" => Some(
            Item::new("splint", "Medical Fracture Splint", ItemType::Medical, Rarity::Common, 0.3, 1, 2, 3)
                .with_description("Stabilizes broken bones and limb trauma."),
        ),
        "purified_water" => Some(
            Item::new("purified_water", "Purified Water Bottle", ItemType::Drink, Rarity::Common, 0.5, 1, 2, 4)
                .with_description("Clean, boiled drinking water. Quenches thirst."),
        ),
        "canned_peaches" => Some(
            Item::new("canned_peaches", "Canned Peaches", ItemType::Food, Rarity::Common, 0.4, 1, 1, 4)
                .with_description("Sweet preserved fruit. Restores hunger and slight thirst."),
        ),
        "mre_ration" => Some(
            Item::new("mre_ration", "Military MRE", ItemType::Food, Rarity::Rare, 0.8, 2, 2, 3)
                .with_description("High-calorie military field ration."),
        ),
        "raw_venison" => Some(
            Item::new("raw_venison", "Raw Venison Meat", ItemType::Food, Rarity::Common, 0.5, 1, 1, 5)
                .with_description("Fresh game meat hunted in the valley. Must be cooked before eating."),
        ),
        "dirty_water" => Some(
            Item::new("dirty_water", "Dirty Pond Water", ItemType::Drink, Rarity::Common, 0.5, 1, 2, 4)
                .with_description("Turbid stagnant water. Contains parasites; boil at a campfire to purify."),
        ),
        "cooked_steak" => Some(
            Item::new("cooked_steak", "Charred Venison Steak", ItemType::Food, Rarity::Uncommon, 0.4, 1, 1, 5)
                .with_description("Savory venison roasted over open flames. Restores 45 hunger."),
        ),
        "hearty_stew" => Some(
            Item::new("hearty_stew", "Hearty Campfire Stew", ItemType::Food, Rarity::Rare, 0.6, 1, 2, 4)
                .with_description("Thick stew simmered with preserved rations and clean broth. Restores 60 food & 30 thirst."),
        ),
        "forest_mushrooms" => Some(
            Item::new("forest_mushrooms", "Wild Forest Mushrooms", ItemType::Food, Rarity::Common, 0.2, 1, 1, 10)
                .with_description("Edible fungi foraged from pine trunks and moist soil."),
        ),
        "roasted_mushrooms" => Some(
            Item::new("roasted_mushrooms", "Roasted Garlic Mushrooms", ItemType::Food, Rarity::Uncommon, 0.2, 1, 1, 10)
                .with_description("Toasted skewered mushrooms grilled over campfire embers."),
        ),
        "wood_plank" => Some(
            Item::new("wood_plank", "Wood Plank", ItemType::Material, Rarity::Common, 1.5, 1, 3, 20)
                .with_description("Basic timber used for barricades, doors, and workbenches."),
        ),
        "wood_log" => Some(
            Item::new("wood_log", "Pine Wood Log", ItemType::Material, Rarity::Common, 2.5, 1, 3, 10)
                .with_description("Raw lumber harvested from coniferous pine trees. Can be sawed into planks."),
        ),
        "stone" => Some(
            Item::new("stone", "Field Stone", ItemType::Material, Rarity::Common, 1.2, 1, 1, 30)
                .with_description("Rough granite mined from rocky outcrops. Used for campfires and masonry."),
        ),
        "scrap_metal" => Some(
            Item::new("scrap_metal", "Scrap Metal", ItemType::Material, Rarity::Common, 0.8, 1, 1, 30)
                .with_description("Forged iron fragments for repairing weapons and vehicle chassis."),
        ),
        "hiking_backpack" => Some(
            Item::new("hiking_backpack", "Hiking Backpack", ItemType::Backpack, Rarity::Uncommon, 2.0, 3, 3, 1)
                .with_description("Durable 36-slot civilian trekking bag."),
        ),
        "military_rucksack" => Some(
            Item::new("military_rucksack", "Military Tactical Rucksack", ItemType::Backpack, Rarity::Epic, 3.5, 3, 4, 1)
                .with_description("Heavy-duty 48-slot tactical pack with MOLLE webbing."),
        ),
        "coyote_backpack" => Some(
            Item::new("coyote_backpack", "Coyote Assault Pack", ItemType::Backpack, Rarity::Rare, 2.8, 3, 3, 1)
                .with_description("40-slot military deployment bag."),
        ),
        "large_black_military_backpack" | "alice_backpack" => Some(
            Item::new("alice_backpack", "ALICE Field Backpack", ItemType::Backpack, Rarity::Legendary, 4.0, 4, 4, 1)
                .with_description("Massive 64-slot high-capacity expedition rucksack."),
        ),
        "keycard_lvl0" => Some(
            Item::new("keycard_lvl0", "Keycard [Level 0 - Bunker Access]", ItemType::Keycard, Rarity::Uncommon, 0.01, 1, 1, 1)
                .with_description("Grants security clearance to outer bunker perimeter doors."),
        ),
        "keycard_lvl1" => Some(
            Item::new("keycard_lvl1", "Keycard [Level 1 - Research Facility]", ItemType::Keycard, Rarity::Rare, 0.01, 1, 1, 1)
                .with_description("Unlocks laboratory and research compound bulkheads."),
        ),
        "keycard_lvl2" => Some(
            Item::new("keycard_lvl2", "Keycard [Level 2 - Military Armory]", ItemType::Keycard, Rarity::Epic, 0.01, 1, 1, 1)
                .with_description("Security clearance for high-tier military weapon storage."),
        ),
        "keycard_lvl3" => Some(
            Item::new("keycard_lvl3", "Keycard [Level 3 - Black Site Terminal]", ItemType::Keycard, Rarity::Legendary, 0.01, 1, 1, 1)
                .with_description("Executive clearance to secret underground command centers."),
        ),
        "keycard_all_access" | "all_access_keycard" => Some(
            Item::new("keycard_all_access", "Master Keycard [All-Access Clearance]", ItemType::Keycard, Rarity::Legendary, 0.01, 1, 1, 1)
                .with_description("Bypasses all electronic magnetic security locks across all bunker clearance tiers."),
        ),
        // Weapon Attachments (Optics, Muzzles, Mags, Tactical)
        "optic_red_dot" => Some(
            Item::new("optic_red_dot", "Kobra Red Dot Sight", ItemType::Attachment, Rarity::Uncommon, 0.25, 1, 1, 1)
                .with_description("Reflex optic providing 1.2x zoom and 15% spread reduction."),
        ),
        "optic_holo" => Some(
            Item::new("optic_holo", "EOTech Holographic Sight", ItemType::Attachment, Rarity::Rare, 0.35, 1, 1, 1)
                .with_description("Holographic weapon sight with illuminated reticle, 1.3x zoom and 20% spread reduction."),
        ),
        "optic_acog_4x" => Some(
            Item::new("optic_acog_4x", "ACOG 4x Combat Scope", ItemType::Attachment, Rarity::Epic, 0.45, 1, 2, 1)
                .with_description("Medium-range combat optic with 4.0x magnification and 35% spread reduction."),
        ),
        "optic_sniper_8x" => Some(
            Item::new("optic_sniper_8x", "Leupold 8x Sniper Scope", ItemType::Attachment, Rarity::Legendary, 0.75, 1, 2, 1)
                .with_description("Long-range marksman optic with 8.0x magnification and 60% spread reduction."),
        ),
        "muzzle_suppressor" => Some(
            Item::new("muzzle_suppressor", "Osprey Tactical Suppressor", ItemType::Attachment, Rarity::Epic, 0.40, 1, 2, 1)
                .with_description("Muzzle sound dampener. Reduces gunshot acoustic radius by 80% and vertical recoil by 10%."),
        ),
        "muzzle_compensator" => Some(
            Item::new("muzzle_compensator", "Competition Muzzle Compensator", ItemType::Attachment, Rarity::Rare, 0.20, 1, 1, 1)
                .with_description("Vented muzzle brake providing 35% vertical and 30% horizontal recoil reduction."),
        ),
        "mag_extended_30" => Some(
            Item::new("mag_extended_30", "PMAG 40-Round Extended Magazine", ItemType::Attachment, Rarity::Rare, 0.30, 1, 2, 1)
                .with_description("Polymer high-capacity magazine granting +10 round magazine capacity."),
        ),
        "mag_drum_60" => Some(
            Item::new("mag_drum_60", "D-60 60-Round Drum Magazine", ItemType::Attachment, Rarity::Epic, 0.85, 2, 2, 1)
                .with_description("Rotary drum magazine granting +30 round magazine capacity."),
        ),
        "tactical_flashlight" => Some(
            Item::new("tactical_flashlight", "SureFire Weapon Flashlight", ItemType::Attachment, Rarity::Uncommon, 0.20, 1, 1, 1)
                .with_description("Rail-mounted high-lumen weapon light for night combat."),
        ),
        "tactical_laser" => Some(
            Item::new("tactical_laser", "PEQ-15 Green Laser Aiming Module", ItemType::Attachment, Rarity::Epic, 0.25, 1, 1, 1)
                .with_description("Precision green laser designator with 25% spread reduction and visible laser beam."),
        ),
        // Ammunition Boxes
        "ammo_9mm" => Some(
            Item::new("ammo_9mm", "9x19mm Parabellum (50 rounds)", ItemType::Ammo, Rarity::Common, 0.6, 1, 1, 10)
                .with_stack(50)
                .with_description("Standard handgun and submachine gun cartridges."),
        ),
        "ammo_45acp" => Some(
            Item::new("ammo_45acp", ".45 ACP Tactical (50 rounds)", ItemType::Ammo, Rarity::Uncommon, 0.8, 1, 1, 10)
                .with_stack(50)
                .with_description("Heavy stopping power handgun cartridges."),
        ),
        "ammo_556" => Some(
            Item::new("ammo_556", "5.56x45mm NATO (60 rounds)", ItemType::Ammo, Rarity::Rare, 0.9, 1, 1, 10)
                .with_stack(60)
                .with_description("High-velocity intermediate assault rifle cartridges."),
        ),
        "ammo_762" => Some(
            Item::new("ammo_762", "7.62x39mm Soviet (60 rounds)", ItemType::Ammo, Rarity::Rare, 1.1, 1, 1, 10)
                .with_stack(60)
                .with_description("Hard-hitting battle rifle cartridges."),
        ),
        "ammo_12gauge" | "ammo_12ga" => Some(
            Item::new("ammo_12gauge", "12 Gauge Buckshot (25 shells)", ItemType::Ammo, Rarity::Uncommon, 1.2, 1, 1, 10)
                .with_stack(25)
                .with_description("Deadly close-quarters shotgun buckshot shells."),
        ),
        "ammo_308" => Some(
            Item::new("ammo_308", ".308 Winchester Match (40 rounds)", ItemType::Ammo, Rarity::Rare, 1.0, 1, 1, 10)
                .with_stack(40)
                .with_description("Precision long-range sniper rifle cartridges."),
        ),
        "ammo_50bmg" => Some(
            Item::new("ammo_50bmg", ".50 BMG Anti-Materiel (10 rounds)", ItemType::Ammo, Rarity::Legendary, 1.5, 1, 1, 10)
                .with_stack(10)
                .with_description("Armor-piercing anti-materiel rounds designed for the Barrett M82."),
        ),
        "horde_beacon" => Some(
            Item::new("horde_beacon", "Infested Horde Beacon Device", ItemType::Material, Rarity::Legendary, 4.5, 2, 2, 1)
                .with_description("Electronic acoustic lure. Emits distress pulses summoning 3 horde waves. Yields legendary rewards upon victory."),
        ),
        "money" | "credits" => Some(
            Item::new("credits", "Trade Credits (Currency)", ItemType::Material, Rarity::Uncommon, 0.001, 1, 1, 999)
                .with_description("Universal trade currency recognized by post-apocalyptic merchants."),
        ),
        "ammo_tin" => Some(
            Item::new("ammo_tin", "Sealed Military Ammo Tin", ItemType::Material, Rarity::Rare, 3.5, 2, 2, 1)
                .with_description("Airtight container packed with assorted military grade cartridges."),
        ),
        "weapons_case" => Some(
            Item::new("weapons_case", "Hardened Weapons Case", ItemType::Material, Rarity::Epic, 5.0, 3, 2, 1)
                .with_description("Lockable Pelican case containing firearm parts and maintenance kits."),
        ),
        "tier3_loot_box" => Some(
            Item::new("tier3_loot_box", "Tier 3 Infested Loot Cache", ItemType::Material, Rarity::Legendary, 6.0, 2, 3, 1)
                .with_description("Mysterious sealed container salvaged from deep radiation zones."),
        ),
        "armor_repair_kit" => Some(
            Item::new("armor_repair_kit", "Armor Repair Kit", ItemType::Material, Rarity::Rare, 2.0, 2, 2, 3)
                .with_description("Heavy-duty ceramic plate inserts and Kevlar stitching to restore vests."),
        ),
        "weapon_cleaning_kit" => Some(
            Item::new("weapon_cleaning_kit", "Weapon Cleaning Kit", ItemType::Material, Rarity::Rare, 1.0, 1, 2, 5)
                .with_description("Bore snakes, solvent, and oil to restore weapon durability to 100%."),
        ),
        "lockpick" => Some(
            Item::new("lockpick", "Lockpick Set", ItemType::Material, Rarity::Common, 0.2, 1, 1, 10)
                .with_description("Tension wrench and diamond pick for bypassing locked containers."),
        ),
        "gas_can" => Some(
            Item::new("gas_can", "Full Jerrycan (Fuel)", ItemType::Material, Rarity::Uncommon, 5.0, 2, 2, 2)
                .with_description("Contains 20 liters of refined gasoline for vehicle tanks and generators."),
        ),
        "vehicle_repair_kit" => Some(
            Item::new("vehicle_repair_kit", "Heavy Vehicle Repair Kit", ItemType::Material, Rarity::Rare, 4.0, 2, 2, 2)
                .with_description("Professional automotive toolset with replacement gaskets, hoses, and steel plating. Restores 50% vehicle engine health."),
        ),
        "car_battery" => Some(
            Item::new("car_battery", "12V Heavy-Duty Vehicle Battery", ItemType::Material, Rarity::Uncommon, 8.0, 2, 2, 1)
                .with_description("Lead-acid automotive battery. Restores vehicle electrical systems and battery charge to 100%."),
        ),
        "wrench" => Some(
            Item::new("wrench", "Mechanic's Adjustable Wrench", ItemType::Material, Rarity::Common, 1.2, 1, 2, 1)
                .with_description("Heavy steel wrench for engine maintenance and vehicle roadside repairs."),
        ),
        "fishing_rod" => Some(
            Item::new("fishing_rod", "Telescopic Carbon Fishing Rod", ItemType::Material, Rarity::Uncommon, 1.5, 1, 3, 1)
                .with_description("Durable fishing rod with monofilament line and lure. Used to catch freshwater fish at rivers and lakes."),
        ),
        "raw_fish" => Some(
            Item::new("raw_fish", "Fresh Valley Trout", ItemType::Food, Rarity::Common, 0.8, 1, 2, 10)
                .with_description("Freshwater fish caught from the valley river. Can be eaten or grilled over a campfire."),
        ),
        "cooked_fish" => Some(
            Item::new("cooked_fish", "Grilled Campfire Trout", ItemType::Food, Rarity::Common, 0.7, 1, 2, 10)
                .with_description("Hearty fish fillet roasted over open coals. Restores 55 hunger and 25 health."),
        ),
        "animal_hide" => Some(
            Item::new("animal_hide", "Cured Animal Hide", ItemType::Material, Rarity::Common, 1.0, 1, 1, 20)
                .with_description("Tanned leather hide harvested from deer or boars. Used for crafting backpacks and protective armor."),
        ),
        "nvg_civilian" => Some(
            Item::new("nvg_civilian", "Civilian Gen 1 NVG", ItemType::NVG, Rarity::Uncommon, 0.9, 2, 2, 1)
                .with_durability(100.0)
                .with_description("Civilian-grade night vision goggles. Employs green phosphor tube with moderate light gain and grain."),
        ),
        "nvg_military" => Some(
            Item::new("nvg_military", "Military Gen 2 Tactical NVG", ItemType::NVG, Rarity::Rare, 1.1, 2, 2, 1)
                .with_durability(100.0)
                .with_description("Standard issue armed forces night vision goggles with enhanced clarity and high optical amplification."),
        ),
        "nvg_specops" => Some(
            Item::new("nvg_specops", "SpecOps Gen 3 Quad-Node NVG", ItemType::NVG, Rarity::Legendary, 1.4, 2, 2, 1)
                .with_durability(100.0)
                .with_description("Top-secret panoramic night vision optics with white phosphor imagery, wide FOV, and low power consumption."),
        ),
        "emergency_radio" => Some(
            Item::new("emergency_radio", "Handheld Emergency Radio", ItemType::Radio, Rarity::Uncommon, 0.6, 1, 2, 1)
                .with_description("Portable military survival transceiver capable of tuning between 80.0 MHz and 180.0 MHz."),
        ),
        "battery_9v" => Some(
            Item::new("battery_9v", "9V Heavy-Duty Alkaline Battery", ItemType::Material, Rarity::Common, 0.05, 1, 1, 10)
                .with_description("Recharges electronic equipment including Night Vision Goggles and tactical flashlights."),
        ),

        _ => {
            static ITEMS_JSON: &str = include_str!("../../surroundead_items.json");
            static LOADED_ITEMS: std::sync::OnceLock<std::collections::HashMap<String, Item>> = std::sync::OnceLock::new();
            
            let map = LOADED_ITEMS.get_or_init(|| {
                let items: Vec<Item> = serde_json::from_str(ITEMS_JSON).unwrap_or_default();
                items.into_iter().map(|it| (it.id.clone(), it)).collect()
            });
            
            map.get(item_id).cloned()
        }

    }
}
