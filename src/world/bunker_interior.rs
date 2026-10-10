use crate::ai::zombie::{Zombie, ZombieType};
use crate::looting::{LootContainer, LootTier};
use crate::math::Position3D;
use crate::world::security_door::KeycardTier;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BunkerRoom {
    pub id: u32,
    pub name: String,
    pub center: Position3D,
    pub width: f32,
    pub depth: f32,
    pub hazard_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BunkerLoreTerminal {
    pub id: u32,
    pub title: String,
    pub author: String,
    pub timestamp: String,
    pub body: String,
    pub position: Position3D,
    pub is_read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BunkerFacility {
    pub facility_id: u32,
    pub name: String,
    pub required_tier: KeycardTier,
    pub ambient_radiation: f32,
    pub rooms: Vec<BunkerRoom>,
    pub lore_terminals: Vec<BunkerLoreTerminal>,
    pub containers: Vec<LootContainer>,
    pub zombies: Vec<Zombie>,
    pub ladder_exit_pos: Position3D,
    pub surface_return_pos: Position3D,
}

impl BunkerFacility {
    /// Generate Research Lab Alpha Subterranean Biohazard Dungeon
    pub fn lab_alpha() -> Self {
        let rooms = vec![
            BunkerRoom {
                id: 1,
                name: "Decontamination Airlock A-1".into(),
                center: Position3D::new(0.0, -15.0, 0.0),
                width: 14.0,
                depth: 14.0,
                hazard_level: 1,
            },
            BunkerRoom {
                id: 2,
                name: "Centrifuge & Pathology Lab".into(),
                center: Position3D::new(0.0, -15.0, 25.0),
                width: 22.0,
                depth: 20.0,
                hazard_level: 3,
            },
            BunkerRoom {
                id: 3,
                name: "Cryo-Storage Quarantine Core".into(),
                center: Position3D::new(25.0, -15.0, 25.0),
                width: 18.0,
                depth: 18.0,
                hazard_level: 4,
            },
        ];

        let lore_terminals = vec![
            BunkerLoreTerminal {
                id: 101,
                title: "Log #104: Specimen Zero Cellular Division".into(),
                author: "Dr. Andrea Vance (Lead Virologist)".into(),
                timestamp: "Day 3 Post-Breach".into(),
                body: "The necrotizing pathogen exhibits unnatural mitochondrial acceleration. Cryo-stasis containment failure in Sector 3. If you find this terminal, do NOT venture into the sub-basement without heavy radiation shielding.".into(),
                position: Position3D::new(2.0, -15.0, 22.0),
                is_read: false,
            },
            BunkerLoreTerminal {
                id: 102,
                title: "Emergency Evacuation Protocol: Red Horizon".into(),
                author: "Site Director Hayes".into(),
                timestamp: "Final Hours".into(),
                body: "Bulkhead blast doors are sealed. Auxiliary generators are on standby. May God have mercy on whoever remains above.".into(),
                position: Position3D::new(22.0, -15.0, 23.0),
                is_read: false,
            },
        ];

        let mut containers = Vec::new();
        let mut c1 = LootContainer::new(201, "Lab Alpha Bio-Hazard Safe", Position3D::new(4.0, -15.0, 27.0), LootTier::Medical, 6, 6);
        let _ = c1.storage.auto_insert(crate::items::consumable::create_stock_item("large_medkit").unwrap());
        let _ = c1.storage.auto_insert(crate::items::consumable::create_stock_item("antibiotics").unwrap());
        let _ = c1.storage.auto_insert(crate::items::consumable::create_stock_item("anti_rad_pills").unwrap());
        containers.push(c1);

        let mut c2 = LootContainer::new(202, "Cryo-Preservation Lockbox", Position3D::new(28.0, -15.0, 27.0), LootTier::MilitaryHighTier, 6, 6);
        let _ = c2.storage.auto_insert(crate::items::consumable::create_stock_item("tier3_loot_box").unwrap());
        let _ = c2.storage.auto_insert(crate::items::consumable::create_stock_item("keycard_lvl2").unwrap());
        containers.push(c2);

        let zombies = vec![
            Zombie::new(101, ZombieType::Spitter, Position3D::new(0.0, -15.0, 22.0)),
            Zombie::new(102, ZombieType::Runner, Position3D::new(2.0, -15.0, 26.0)),
            Zombie::new(103, ZombieType::Walker, Position3D::new(24.0, -15.0, 24.0)),
        ];

        Self {
            facility_id: 1,
            name: "Research Lab Alpha — Underground Bio-Containment".into(),
            required_tier: KeycardTier::Level1,
            ambient_radiation: 18.0,
            rooms,
            lore_terminals,
            containers,
            zombies,
            ladder_exit_pos: Position3D::new(0.0, -15.0, -2.0),
            surface_return_pos: Position3D::new(-115.0, 0.0, 115.0),
        }
    }

    /// Generate FOB Bravo Deep Armory & War Room
    pub fn fob_bravo() -> Self {
        let rooms = vec![
            BunkerRoom {
                id: 1,
                name: "Armory Staging Airlock".into(),
                center: Position3D::new(0.0, -20.0, 0.0),
                width: 14.0,
                depth: 14.0,
                hazard_level: 1,
            },
            BunkerRoom {
                id: 2,
                name: "Heavy Munitions Depot".into(),
                center: Position3D::new(-20.0, -20.0, 20.0),
                width: 24.0,
                depth: 22.0,
                hazard_level: 2,
            },
            BunkerRoom {
                id: 3,
                name: "FOB Command & Control War Room".into(),
                center: Position3D::new(20.0, -20.0, 20.0),
                width: 20.0,
                depth: 20.0,
                hazard_level: 3,
            },
        ];

        let lore_terminals = vec![
            BunkerLoreTerminal {
                id: 201,
                title: "Tactical Situation Report: Sector West Overrun".into(),
                author: "Major Marcus Briggs".into(),
                timestamp: "Day 12".into(),
                body: "Perimeter fence compromised. We evacuated the valley base survivors down to Bravo Bunker. Munitions are stocked in Depot B-2.".into(),
                position: Position3D::new(-18.0, -20.0, 18.0),
                is_read: false,
            },
        ];

        let mut containers = Vec::new();
        let mut c1 = LootContainer::new(301, "FOB Bravo Heavy Munitions Locker", Position3D::new(-22.0, -20.0, 22.0), LootTier::MilitaryHighTier, 7, 6);
        let _ = c1.storage.auto_insert(crate::items::consumable::create_stock_item("ammo_tin").unwrap());
        let _ = c1.storage.auto_insert(crate::items::consumable::create_stock_item("weapons_case").unwrap());
        let _ = c1.storage.auto_insert(crate::items::consumable::create_stock_item("keycard_lvl3").unwrap());
        containers.push(c1);

        let zombies = vec![
            Zombie::new(201, ZombieType::Runner, Position3D::new(-18.0, -20.0, 18.0)),
            Zombie::new(202, ZombieType::Bloater, Position3D::new(-21.0, -20.0, 23.0)),
            Zombie::new(203, ZombieType::Walker, Position3D::new(18.0, -20.0, 18.0)),
        ];

        Self {
            facility_id: 2,
            name: "FOB Bravo — Underground Armory & Munitions Depot".into(),
            required_tier: KeycardTier::Level2,
            ambient_radiation: 8.0,
            rooms,
            lore_terminals,
            containers,
            zombies,
            ladder_exit_pos: Position3D::new(0.0, -20.0, -2.0),
            surface_return_pos: Position3D::new(128.0, 0.0, -92.0),
        }
    }

    /// Generate Black Site Omega Subterranean Command Core
    pub fn black_site_omega() -> Self {
        let rooms = vec![
            BunkerRoom {
                id: 1,
                name: "Decontamination Pressure Lock".into(),
                center: Position3D::new(0.0, -30.0, 0.0),
                width: 16.0,
                depth: 16.0,
                hazard_level: 2,
            },
            BunkerRoom {
                id: 2,
                name: "Black Site Mainframe Core".into(),
                center: Position3D::new(0.0, -30.0, 30.0),
                width: 26.0,
                depth: 26.0,
                hazard_level: 5,
            },
        ];

        let lore_terminals = vec![
            BunkerLoreTerminal {
                id: 301,
                title: "Classified Directive: Project Obsidian".into(),
                author: "Unknown Administrator".into(),
                timestamp: "Day 0".into(),
                body: "Top Secret: All valley research directives have been terminated. The mutated entities are self-replicating. Final prototype weapons sealed in Command Safe Omega.".into(),
                position: Position3D::new(0.0, -30.0, 28.0),
                is_read: false,
            },
        ];

        let mut containers = Vec::new();
        let mut c1 = LootContainer::new(401, "Black Site Prototype Weapons Vault", Position3D::new(4.0, -30.0, 32.0), LootTier::AirdropMilitary, 8, 6);
        let _ = c1.storage.auto_insert(crate::items::consumable::create_stock_item("horde_beacon").unwrap());
        let _ = c1.storage.auto_insert(crate::items::consumable::create_stock_item("tier3_loot_box").unwrap());
        let _ = c1.storage.auto_insert(crate::items::consumable::create_stock_item("keycard_all_access").unwrap());
        containers.push(c1);

        let zombies = vec![
            Zombie::new(301, ZombieType::Juggernaut, Position3D::new(0.0, -30.0, 25.0)),
            Zombie::new(302, ZombieType::Runner, Position3D::new(-5.0, -30.0, 27.0)),
            Zombie::new(303, ZombieType::Screamer, Position3D::new(5.0, -30.0, 27.0)),
        ];

        Self {
            facility_id: 3,
            name: "Black Site Omega — Subterranean Command Core".into(),
            required_tier: KeycardTier::Level3,
            ambient_radiation: 75.0,
            rooms,
            lore_terminals,
            containers,
            zombies,
            ladder_exit_pos: Position3D::new(0.0, -30.0, -2.0),
            surface_return_pos: Position3D::new(185.0, 0.0, 155.0),
        }
    }
}
