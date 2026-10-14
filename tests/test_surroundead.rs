use surroundead::ai::{AiDirector, NoiseEvent, Zombie, ZombieType};
use surroundead::combat::{create_weapon_preset, BallisticShot, TargetHitbox};
use surroundead::crafting::{CraftingRecipe, PlacedStructure, StructureType};
use surroundead::engine::GameState;
use surroundead::items::{create_stock_item, Equipment, GridContainer, Item, ItemType, Rarity};
use surroundead::math::Position3D;
use surroundead::persistence::SaveSystem;
use surroundead::survival::SurvivalStats;
use surroundead::vehicles::{Vehicle, VehicleType};

#[test]
fn test_survival_stats_and_tick() {
    let mut stats = SurvivalStats::new();
    assert_eq!(stats.health, 100.0);
    assert!(stats.is_alive());

    // Tick for 10 seconds while sprinting
    stats.tick(10.0, true);
    assert!(stats.hunger < 100.0);
    assert!(stats.thirst < 100.0);
    assert!(stats.stamina < 100.0);

    // Bleeding damage over time is gameplay::medical's (MedicalComponent); take a hit and bleed.
    stats.take_damage(10.0, false, 0.0);
    stats.bleeding = true;
    assert!(stats.health < 100.0);

    // Apply bandage
    let prev_hp = stats.health;
    stats.apply_bandage();
    assert!(!stats.bleeding);
    assert!(stats.health > prev_hp);

    // Test infection and antibiotics
    stats.infection = 50.0;
    stats.take_antibiotics();
    assert_eq!(stats.infection, 10.0);
}

#[test]
fn test_grid_inventory_and_nesting() {
    let mut grid = GridContainer::new("test_grid", "Test Grid", 4, 4, 20.0);

    // Insert 2x2 item at (0, 0)
    let item1 = Item::new("item1", "Item 1", ItemType::Material, Rarity::Common, 1.0, 2, 2, 1);
    assert!(grid.place_at(item1, 0, 0).is_ok());

    // Attempt to overlap at (1, 1) -> must fail
    let item2 = Item::new("item2", "Item 2", ItemType::Material, Rarity::Common, 1.0, 2, 2, 1);
    assert!(!grid.can_place_at(1, 1, 2, 2, None));

    // Place at free slot (2, 0) -> must succeed
    assert!(grid.can_place_at(2, 0, 2, 2, None));
    assert!(grid.place_at(item2, 2, 0).is_ok());

    // Auto-insert item into remaining space (0, 2)
    let item3 = Item::new("item3", "Item 3", ItemType::Medical, Rarity::Common, 0.5, 1, 1, 5);
    assert!(grid.auto_insert(item3).is_ok());

    // Verify equipment nested storage
    let mut equip = Equipment::new();
    let rucksack = create_stock_item("military_rucksack").unwrap();
    assert!(equip.equip_item(rucksack).is_ok());
    assert!(equip.get_backpack_grid_mut().is_some());
    let bp_grid = equip.get_backpack_grid_mut().unwrap();
    assert_eq!(bp_grid.width, 6);
    assert_eq!(bp_grid.height, 8);
}

#[test]
fn test_combat_ballistics_and_headshots() {
    let m4 = create_weapon_preset("m4a1").unwrap();
    let shot = BallisticShot::from_weapon(
        Position3D::new(0.0, 1.5, 0.0),
        glam::Vec3::new(0.0, 0.0, 1.0),
        &m4,
    );

    // Target zombie directly in line of fire at z = 10m
    let target_pos = Position3D::new(0.0, 0.0, 10.0);
    let hitbox = TargetHitbox::for_humanoid(1, target_pos);

    let hit = hitbox.test_hit(&shot.ray);
    assert!(hit.is_some());
    let (_zone, dist) = hit.unwrap();
    assert!((dist - 10.0).abs() < 2.0);

    // Verify headshot damage calculation
    let headshot_dmg = m4.base_damage * m4.headshot_multiplier;
    assert!(headshot_dmg >= 120.0);
}

#[test]
fn test_zombie_ai_state_machine() {
    let director = AiDirector::new();
    let mut zombie = Zombie::new(1, ZombieType::Walker, Position3D::new(0.0, 0.0, 15.0));
    let player_pos = Position3D::new(0.0, 0.0, 0.0);

    // In broad daylight facing player -> zombie spots player and transitions to Chase
    zombie.yaw = 180.0; // Facing south towards player
    let _ = director.update_zombie(&mut zombie, &player_pos, 0.1, true, false, &[]);
    assert_eq!(zombie.state, surroundead::ai::AiState::Chase);

    // Hearing gunfire: zombie at 80m hears unsuppressed gunshot (radius 120m)
    let mut far_zombie = Zombie::new(2, ZombieType::Walker, Position3D::new(80.0, 0.0, 0.0));
    let gunshot = NoiseEvent {
        position: player_pos,
        radius_m: 120.0,
        intensity: 1.0,
        source_entity: Some(0),
    };

    let _ = director.update_zombie(&mut far_zombie, &player_pos, 0.1, false, false, &[gunshot]);
    assert_eq!(far_zombie.state, surroundead::ai::AiState::Investigate);
}

#[test]
fn test_vehicle_simulation_and_fuel() {
    let mut vehicle = Vehicle::new(1, VehicleType::Sedan, Position3D::new(0.0, 0.0, 0.0));
    assert!(vehicle.start_engine().is_ok());
    assert!(vehicle.is_engine_running);

    let initial_fuel = vehicle.fuel_liters;
    // Accelerate for 5 seconds
    vehicle.drive(1.0, 0.0, 5.0);
    assert!(vehicle.speed_kmh > 0.0);
    assert!(vehicle.fuel_liters < initial_fuel);
    assert!(vehicle.position.z > 0.0);

    // Roadkill kinetic impact
    let dmg = vehicle.roadkill_impact(100.0);
    assert!(dmg > 50.0);
}

#[test]
fn test_crafting_and_building() {
    let recipes = CraftingRecipe::get_all_recipes();
    let bandage_recipe = recipes.iter().find(|r| r.id == "craft_bandage").unwrap();
    assert_eq!(bandage_recipe.result_amount, 1);
    let item = bandage_recipe.instantiate_result().unwrap();
    assert_eq!(item.id, "bandage");

    // Modular structure placement
    let mut wall = PlacedStructure::new(1, StructureType::WoodenWall, Position3D::new(5.0, 0.0, 5.0), 0.0);
    assert_eq!(wall.health, 350.0);
    assert!(!wall.take_damage(50.0));
    assert_eq!(wall.health, 300.0);
    assert!(wall.take_damage(300.0)); // Destroyed!
}

#[test]
fn test_save_load_roundtrip() {
    let state = GameState::new();
    let temp_path = "/tmp/test_surroundead_save.json";

    assert!(SaveSystem::save_to_file(&state, temp_path).is_ok());
    let loaded = SaveSystem::load_from_file(temp_path).expect("Failed to load save");

    assert_eq!(loaded.player_pos, state.player_pos);
    assert_eq!(loaded.survival.health, state.survival.health);
    assert_eq!(loaded.zombies.len(), state.zombies.len());
    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn test_progression_and_skill_tree() {
    use surroundead::progression::{ProgressionSystem, SkillType};

    let mut prog = ProgressionSystem::new();
    assert_eq!(prog.level, 1);
    assert_eq!(prog.unspent_points, 0);

    // LevellingComponent: 500 XP -> level 2 (+1 skill point)
    assert_eq!(prog.add_xp(150), None);
    let lvl = prog.add_xp(350);
    assert_eq!(lvl, Some(2));
    assert_eq!(prog.level, 2);
    assert_eq!(prog.unspent_points, 1);

    // Upgrade Marksmanship perk
    assert!(prog.upgrade_skill(SkillType::Marksmanship).is_ok());
    assert_eq!(prog.get_skill_level(SkillType::Marksmanship), 1);
    assert_eq!(prog.unspent_points, 0);

    // Check recoil reduction
    assert!(prog.recoil_multiplier() < 1.0);
}

#[test]
fn test_lockpicking_mechanics() {
    use surroundead::looting::{LockDifficulty, LockMechanism, LockpickResult};

    let mut lock = LockMechanism::new(LockDifficulty::Novice);
    assert!(lock.is_locked);

    // Align pick to target angle
    lock.set_pick_angle(lock.target_angle_deg);

    // Turn cylinder with tension
    let mut unlocked = false;
    for _ in 0..10 {
        match lock.turn_cylinder(0.5, 1.0) {
            LockpickResult::SuccessUnlocked { xp } => {
                assert!(xp > 0);
                unlocked = true;
                break;
            }
            LockpickResult::Turning { .. } => {}
            _ => panic!("Expected smooth turning when aligned!"),
        }
    }
    assert!(unlocked);
    assert!(!lock.is_locked);
}

#[test]
fn test_weapon_attachments_and_suppression() {
    use surroundead::combat::create_weapon_preset;

    let mut m4 = create_weapon_preset("m4a1").unwrap();
    let unsuppressed_noise = m4.effective_noise_radius();
    assert_eq!(unsuppressed_noise, 120.0);

    // Attach tactical suppressor
    m4.attach_mod("muzzle_suppressor");
    let suppressed_noise = m4.effective_noise_radius();
    assert!(suppressed_noise < 30.0); // 80% noise drop!

    // Attach extended mag
    m4.attach_mod("mag_extended_30");
    assert_eq!(m4.extended_mag.as_deref(), Some("mag_extended_30"));

    // Attach laser
    m4.attach_mod("tactical_laser");
    assert!(m4.has_laser());
}

#[test]
fn test_bandit_ai_and_combat() {
    use surroundead::ai::bandit::{Bandit, BanditState};

    let mut bandit = Bandit::new(101, "Test Raider", Position3D::new(0.0, 0.0, 20.0), "ak47");
    assert!(bandit.is_alive());
    assert_eq!(bandit.state, BanditState::Patrol);

    // When player is close (dist = 20m), bandit detects and engages player
    let player_pos = Position3D::new(0.0, 0.0, 0.0);
    bandit.update(&player_pos, 0.5);
    assert!(bandit.state == BanditState::Alert || bandit.state == BanditState::EngagePlayer);

    // Bandit takes lethal damage and drops loot
    let killed = bandit.take_damage(120.0);
    assert!(killed);
    assert!(!bandit.is_alive());

    let drops = bandit.generate_drops();
    assert!(!drops.is_empty());
}

#[test]
fn test_trader_economy_and_quests() {
    use surroundead::economy::{Quest, QuestType, Trader};
    use surroundead::items::{Item, ItemType, Rarity};

    let mut trader = Trader::new_safezone_merchant(1, Position3D::new(0.0, 0.0, 0.0));
    assert!(trader.credits >= 2000);
    assert!(!trader.offers.is_empty());

    // Player sells an epic weapon to trader
    let gun = Item::new("ak47", "AK-47 Tactical", ItemType::WeaponPrimary, Rarity::Epic, 3.8, 2, 4, 1)
        .with_durability(100.0);
    let val = trader.buy_from_player(&gun).unwrap();
    assert!(val > 100);

    // Player buys first offer
    let (item, cost) = trader.sell_to_player(0, 500).unwrap();
    assert!(cost > 0);
    assert!(!item.id.is_empty());

    // Test quest bounty progression
    let mut quest = Quest::new(1, "Hunt Walkers", "Kill 3", QuestType::KillZombies, 3, 100, 50);
    assert!(!quest.advance(2));
    assert!(!quest.is_completed);
    assert!(quest.advance(1));
    assert!(quest.is_completed);
}

#[test]
fn test_trauma_and_medical_system() {
    use surroundead::survival::{MedicalSystem, SurvivalStats, TraumaType};

    let mut medical = MedicalSystem::new();
    let mut stats = SurvivalStats::new();

    // Suffer fracture
    medical.add_trauma(TraumaType::Fracture, 1.0, 100.0);
    assert!(medical.has_trauma(TraumaType::Fracture));
    assert!(medical.movement_speed_multiplier() < 1.0); // Limping

    // Apply splint
    let res = medical.apply_medical_item(&mut stats, "splint");
    assert!(res.is_some());
    assert!(!medical.has_trauma(TraumaType::Fracture));
    assert_eq!(medical.movement_speed_multiplier(), 1.0);

    // Suffer arterial heavy bleed
    medical.add_trauma(TraumaType::HeavyBleed, 1.0, 50.0);
    let res_tourniquet = medical.apply_medical_item(&mut stats, "tourniquet");
    assert!(res_tourniquet.is_some());
    assert!(!medical.has_trauma(TraumaType::HeavyBleed));
}

#[test]
fn test_world_events_and_airdrops() {
    use surroundead::world::WorldEventManager;

    let mut event_mgr = WorldEventManager::new();
    let mut containers = Vec::new();

    // Fast-forward past airdrop timer
    let triggered = event_mgr.tick(200.0, &mut containers);
    assert_eq!(triggered, Some(surroundead::world::WorldEventType::MilitaryAirdrop));
    assert!(event_mgr.active_airdrop.is_some());

    // Fast-forward parachute descent until landing
    for _ in 0..10 {
        event_mgr.tick(2.0, &mut containers);
    }

    assert!(event_mgr.active_airdrop.as_ref().unwrap().has_landed);
    assert!(!containers.is_empty());
    assert_eq!(containers[0].name, "C-130 Military Supply Drop");
}

#[test]
fn test_authentic_surroundead_loot_tables() {
    use surroundead::looting::{LootTable, LootTier};

    // Roll military airdrop loot table
    let airdrop_loot = LootTable::roll_loot(LootTier::AirdropMilitary, 10);
    assert_eq!(airdrop_loot.len(), 10);
    assert!(airdrop_loot.iter().any(|i| i.rarity == Rarity::Legendary || i.rarity == Rarity::Epic));

    // Roll high tier safe loot table
    let safe_loot = LootTable::roll_loot(LootTier::Safe, 5);
    assert_eq!(safe_loot.len(), 5);
    assert!(safe_loot.iter().any(|i| i.id == "credits" || i.id == "money" || i.id.contains("keycard") || i.id == "deagle" || i.id == "battle_ready_glock"));

    // Roll keycards loot table
    let keycard_loot = LootTable::roll_loot(LootTier::Keycards, 4);
    assert_eq!(keycard_loot.len(), 4);
    for k in &keycard_loot {
        assert_eq!(k.item_type, ItemType::Keycard);
    }

    // Roll infestation and bosses
    let infest_loot = LootTable::roll_loot(LootTier::Infestation, 5);
    assert_eq!(infest_loot.len(), 5);

    let boss_loot = LootTable::roll_loot(LootTier::ZombieBoss, 3);
    assert_eq!(boss_loot.len(), 3);

    let ammo_loot = LootTable::roll_loot(LootTier::Ammunition, 6);
    assert_eq!(ammo_loot.len(), 6);
    assert!(ammo_loot.iter().any(|i| i.item_type == ItemType::Ammo || i.id == "ammo_tin"));
}

#[test]
fn test_authentic_surroundead_arsenal_and_named_weapons() {
    // Test legendary named firearms
    let br_glock = create_weapon_preset("battle_ready_glock").expect("G18 named preset exists");
    assert_eq!(br_glock.fire_mode, surroundead::combat::FireMode::FullAuto);
    assert_eq!(br_glock.rounds_per_minute, 1150);
    assert!(br_glock.suppressor.is_some());

    let cerberus = create_weapon_preset("cerberus").expect("Cerberus exists");
    assert_eq!(cerberus.caliber, surroundead::combat::Caliber::ThreeZeroEight);
    assert!(cerberus.base_damage >= 70.0);

    let valkyrie = create_weapon_preset("valkyrie").expect("Valkyrie exists");
    assert_eq!(valkyrie.mag_capacity, 35);

    let lechie = create_weapon_preset("lechie").expect("Lechie exists");
    assert_eq!(lechie.caliber, surroundead::combat::Caliber::ThreeHundredWin);
    assert!(lechie.base_damage > 200.0);

    // Test heavy weapons & launchers
    let gl = create_weapon_preset("grenade_launcher").expect("MGL exists");
    assert_eq!(gl.caliber, surroundead::combat::Caliber::FortyMmGrenade);
    assert_eq!(gl.mag_capacity, 6);

    let rpg = create_weapon_preset("rocket_launcher").expect("RPG exists");
    assert_eq!(rpg.base_damage, 480.0);

    // Test authentic melee weapons
    let katana = create_weapon_preset("katana").expect("Katana exists");
    assert_eq!(katana.base_damage, 85.0);

    let sledge = create_weapon_preset("sledgehammer").expect("Sledgehammer exists");
    assert_eq!(sledge.base_damage, 110.0);
}

#[test]
fn test_resource_harvesting_wood_and_rock() {
    use surroundead::engine::GameState;
    use surroundead::world::{HarvestNode, HarvestNodeType};

    let mut state = GameState::new();

    // Spawn a pine tree node right next to player
    state.harvest_nodes.push(HarvestNode::new(9901, HarvestNodeType::PineTree, state.player_pos));
    
    // Harvest pine tree
    let harvest_res = state.harvest_nearest(2.0);
    assert!(harvest_res.is_some());
    let (node_type, item_name, xp, was_felled, _) = harvest_res.unwrap();
    assert_eq!(node_type, HarvestNodeType::PineTree);
    assert!(item_name.contains("Wood"));
    assert!(xp > 0);
    assert!(!was_felled);
    assert!(state.equipment.pockets.count_item("wood_plank") >= 1);

    // Clear and spawn a rock boulder next to player
    state.harvest_nodes.retain(|n| n.id != 9901);
    state.harvest_nodes.push(HarvestNode::new(9902, HarvestNodeType::RockBoulder, state.player_pos));
    let harvest_rock = state.harvest_nearest(2.0);
    assert!(harvest_rock.is_some());
    let (rock_type, rock_item, _, _, _) = harvest_rock.unwrap();
    assert_eq!(rock_type, HarvestNodeType::RockBoulder);
    assert!(rock_item.contains("Stone") || rock_item.contains("Scrap"));
    assert!(state.equipment.pockets.count_item("stone") >= 1 || state.equipment.pockets.count_item("scrap_metal") >= 1);
}

#[test]
fn test_container_looting_and_scavenging() {
    use surroundead::engine::GameState;
    use surroundead::looting::{LootContainer, LootTier};
    use surroundead::math::Position3D;

    let mut state = GameState::new();
    let test_pos = Position3D::new(state.player_pos.x + 1.0, 0.0, state.player_pos.z + 1.0);
    let crate_box = LootContainer::new(8888, "Survivor Supply Trunk", test_pos, LootTier::Hardware, 4, 4);
    assert!(!crate_box.storage.items.is_empty());
    let initial_count = crate_box.storage.items.len();
    state.containers.push(crate_box);

    // Nearby container lookup
    assert_eq!(state.get_nearby_container_id(2.5), Some(8888));

    // Loot single item
    let single_looted = state.loot_item_from_container(8888, 0);
    assert!(single_looted.is_some());

    // Loot all remaining items
    let (c_name, count) = state.loot_all_from_container(8888);
    assert_eq!(c_name, "Survivor Supply Trunk");
    assert_eq!(count, initial_count - 1);

    let empty_container = state.containers.iter().find(|c| c.id == 8888).unwrap();
    assert!(empty_container.storage.items.is_empty());
    assert!(empty_container.is_searched);
}

#[test]
fn test_radio_tuning_and_airdrop_dispatch() {
    use surroundead::engine::GameState;

    let mut state = GameState::new();
    assert_eq!(state.radio_frequency_mhz, 89.5);

    // Tune frequency
    state.tune_radio(92.1);
    assert_eq!(state.radio_frequency_mhz, 92.1);
    state.tune_radio(104.3);
    assert_eq!(state.radio_frequency_mhz, 104.3);

    // Give credits to call emergency airdrop
    state.player_credits = 250;
    let drop_res = state.call_emergency_airdrop();
    assert!(drop_res.is_ok());
    assert_eq!(state.player_credits, 150);
    assert!(state.world_events.active_airdrop.is_some());
    assert!(state.containers.iter().any(|c| c.id == 999));
}

#[test]
fn test_dynamic_horde_spawning_and_noise_attraction() {
    use surroundead::ai::{HordeSpawner, NoiseEvent};

    let mut spawner = HordeSpawner::new();
    let mut zombies = Vec::new();
    let p_pos = Position3D::new(0.0, 0.0, 0.0);

    // 1. Spawning day vs night
    let horde_id = spawner.spawn_horde(&p_pos, 8, false, &mut zombies);
    assert_eq!(spawner.active_hordes.len(), 1);
    assert_eq!(spawner.active_hordes[0].id, horde_id);
    assert_eq!(zombies.len(), 8);

    // 2. Emergency surge spawning
    let surge_count = spawner.spawn_surge(&p_pos, &mut zombies);
    assert_eq!(surge_count, 12);
    assert_eq!(zombies.len(), 20);
    assert_eq!(spawner.active_hordes.len(), 2);

    // 3. Acoustic noise attraction
    let gunshot = NoiseEvent {
        position: Position3D::new(10.0, 0.0, 10.0),
        radius_m: 80.0,
        intensity: 0.9,
        source_entity: None,
    };
    let gunshot_pos = gunshot.position;
    spawner.handle_noises(&[gunshot], &mut zombies);

    // Verify horde members switched to chase / investigate gunshot
    assert!(spawner.active_hordes[0].is_alerted);
    assert_eq!(spawner.active_hordes[0].target_point, Some(gunshot_pos));
}

#[test]
fn test_campfire_cooking_and_fuel() {
    use surroundead::engine::GameState;
    use surroundead::items::create_stock_item;

    let mut state = GameState::new();
    state.campfire.position = state.player_pos; // Right next to player
    assert!(state.campfire.is_lit);

    // Check adding fuel
    let initial_fuel = state.campfire.fuel_remaining_s;
    if let Some(plank) = create_stock_item("wood_plank") {
        let _ = state.equipment.pockets.auto_insert(plank);
    }
    let fuel_res = state.add_fuel_to_campfire();
    assert!(fuel_res.is_ok());
    assert!(state.campfire.fuel_remaining_s > initial_fuel);

    // Add ingredients for venison cooking
    if let Some(venison) = create_stock_item("raw_venison") {
        let _ = state.equipment.pockets.auto_insert(venison);
    }
    let cook_res = state.start_campfire_cooking("cook_venison");
    assert!(cook_res.is_ok());
    assert!(state.campfire.active_cooking.is_some());

    // Advance simulation until cooked
    state.update(4.0);
    assert!(state.campfire.active_cooking.is_none());
    assert!(state.equipment.pockets.count_item("cooked_steak") > 0 || state.equipment.get_backpack_grid().map(|b| b.count_item("cooked_steak")).unwrap_or(0) > 0);
}

#[test]
fn test_vehicle_trunk_storage_transfers() {
    use surroundead::engine::GameState;
    use surroundead::items::create_stock_item;

    let mut state = GameState::new();
    let vid = state.vehicles[0].id;
    state.player_pos = state.vehicles[0].position; // Stand next to vehicle

    let initial_pocket_count = state.equipment.pockets.items.len();
    assert!(initial_pocket_count > 0);

    // Store first pocket item into vehicle trunk
    let iname = state.equipment.pockets.items[0].item.name.clone();
    let store_res = state.store_item_in_vehicle_trunk(vid, 0, false);
    assert!(store_res.is_ok());
    assert_eq!(state.vehicles[0].trunk.items.len(), 1);
    assert_eq!(state.vehicles[0].trunk.items[0].item.name, iname);
    assert_eq!(state.equipment.pockets.items.len(), initial_pocket_count - 1);

    // Retrieve item back from trunk
    let take_res = state.take_item_from_vehicle_trunk(vid, 0);
    assert!(take_res.is_ok());
    assert_eq!(state.vehicles[0].trunk.items.len(), 0);

    // Store multiple items then take all
    if let Some(gas) = create_stock_item("gas_can") {
        let _ = state.vehicles[0].auto_insert_trunk(gas);
    }
    if let Some(plank) = create_stock_item("wood_plank") {
        let _ = state.vehicles[0].auto_insert_trunk(plank);
    }
    assert_eq!(state.vehicles[0].trunk_item_count(), 2);

    let taken_all = state.take_all_from_vehicle_trunk(vid);
    assert!(taken_all.is_ok());
    assert_eq!(taken_all.unwrap(), 2);
    assert_eq!(state.vehicles[0].trunk_item_count(), 0);
}

#[test]
fn test_trader_hank_buying_and_selling() {
    use surroundead::engine::GameState;
    use surroundead::items::create_stock_item;

    let mut state = GameState::new();
    state.player_credits = 500;
    let initial_offers = state.trader.offers.len();
    assert!(initial_offers > 0);

    // Buy first item from trader
    let offer_price = state.trader.offers[0].price_credits;
    let buy_res = state.buy_from_trader(0);
    assert!(buy_res.is_ok());
    assert_eq!(state.player_credits, 500 - offer_price);
    assert_eq!(state.trader.offers.len(), initial_offers - 1);

    // Sell an item to trader
    if let Some(plank) = create_stock_item("wood_plank") {
        let _ = state.equipment.pockets.auto_insert(plank);
    }
    let pocket_idx = state.equipment.pockets.items.len() - 1;
    let prev_credits = state.player_credits;
    let sell_res = state.sell_to_trader(pocket_idx, false);
    assert!(sell_res.is_ok());
    assert!(state.player_credits > prev_credits);
}

#[test]
fn test_bunker_keycard_security_doors_and_clearance() {
    use surroundead::engine::GameState;
    use surroundead::items::create_stock_item;
    use surroundead::world::KeycardTier;

    let mut state = GameState::new();
    assert_eq!(state.bunker_doors.len(), 3);

    // Initial doors: Level 1 (Lab Alpha), Level 2 (FOB Bravo), Level 3 (Black Site)
    assert_eq!(state.bunker_doors[0].required_tier, KeycardTier::Level1);
    assert_eq!(state.bunker_doors[1].required_tier, KeycardTier::Level2);
    assert_eq!(state.bunker_doors[2].required_tier, KeycardTier::Level3);

    assert!(!state.bunker_doors[0].is_unlocked);
    assert!(!state.bunker_doors[1].is_unlocked);
    assert!(!state.bunker_doors[2].is_unlocked);

    // Teleport player directly to Level 1 door reader
    state.player_pos = state.bunker_doors[0].position;

    // Player starts with keycard_lvl1: unlock Level 1 door
    let res = state.try_interact_bunker_door();
    assert!(res.is_some());
    let (success, msg) = res.unwrap();
    assert!(success);
    assert!(msg.contains("SECURITY CLEARANCE ACCEPTED"));
    assert!(state.bunker_doors[0].is_unlocked);
    assert!(state.bunker_doors[0].is_open);

    // Teleport to Level 2 door: player only has Level 1 clearance, should be denied
    state.player_pos = state.bunker_doors[1].position;
    let res2 = state.try_interact_bunker_door();
    assert!(res2.is_some());
    let (success2, msg2) = res2.unwrap();
    assert!(!success2);
    assert!(msg2.contains("ACCESS DENIED"));
    assert!(!state.bunker_doors[1].is_unlocked);

    // Acquire Master Keycard and unlock Level 2 and Level 3 doors
    let master_card = create_stock_item("keycard_all_access").unwrap();
    let _ = state.equipment.auto_insert_carried(master_card);

    let res_master2 = state.try_interact_bunker_door();
    assert!(res_master2.is_some());
    assert!(res_master2.unwrap().0);
    assert!(state.bunker_doors[1].is_unlocked);

    state.player_pos = state.bunker_doors[2].position;
    let res_master3 = state.try_interact_bunker_door();
    assert!(res_master3.is_some());
    assert!(res_master3.unwrap().0);
    assert!(state.bunker_doors[2].is_unlocked);
}

#[test]
fn test_weapon_modding_workbench_and_stat_multipliers() {
    use surroundead::combat::AttachmentSlot;
    use surroundead::engine::GameState;
    use surroundead::items::create_stock_item;

    let mut state = GameState::new();
    assert!(state.active_weapon.is_some());

    let base_noise = state.active_weapon.as_ref().unwrap().effective_noise_radius();
    assert_eq!(base_noise, 120.0);
    assert_eq!(state.active_weapon.as_ref().unwrap().effective_zoom(), 1.0);
    assert_eq!(state.active_weapon.as_ref().unwrap().effective_mag_capacity(), 30);

    // Mount ACOG 4x Scope (available in start gear)
    let mount_optic = state.attach_weapon_mod("optic_acog_4x");
    assert!(mount_optic);
    assert_eq!(state.active_weapon.as_ref().unwrap().effective_zoom(), 4.0);
    assert!(state.active_weapon.as_ref().unwrap().optic.is_some());

    // Mount Osprey Tactical Suppressor (available in start gear)
    let mount_supp = state.attach_weapon_mod("muzzle_suppressor");
    assert!(mount_supp);
    assert!(state.active_weapon.as_ref().unwrap().is_suppressed());
    let supp_noise = state.active_weapon.as_ref().unwrap().effective_noise_radius();
    assert!(supp_noise <= 24.0); // 80% reduction

    // Give player drum mag and mount it
    let drum = create_stock_item("mag_drum_60").unwrap();
    let _ = state.equipment.auto_insert_carried(drum);
    let mount_drum = state.attach_weapon_mod("mag_drum_60");
    assert!(mount_drum);
    assert_eq!(state.active_weapon.as_ref().unwrap().effective_mag_capacity(), 60);

    // Mount PEQ-15 green laser module
    let laser = create_stock_item("tactical_laser").unwrap();
    let _ = state.equipment.auto_insert_carried(laser);
    let mount_laser = state.attach_weapon_mod("tactical_laser");
    assert!(mount_laser);
    assert!(state.active_weapon.as_ref().unwrap().has_laser());

    // Detach suppressor: noise reverts and item returns to carried equipment
    let detach_supp = state.detach_weapon_mod(AttachmentSlot::Muzzle);
    assert!(detach_supp);
    assert!(!state.active_weapon.as_ref().unwrap().is_suppressed());
    assert_eq!(state.active_weapon.as_ref().unwrap().effective_noise_radius(), 120.0);
    assert!(state.equipment.has_item("muzzle_suppressor"));
}

#[test]
fn test_horde_beacon_deployment_and_wave_assault() {
    use surroundead::crafting::StructureType;
    use surroundead::engine::GameState;
    use surroundead::items::create_stock_item;

    let mut state = GameState::new();
    let initial_zombies = state.zombies.len();

    // Give player a horde beacon device
    let beacon = create_stock_item("horde_beacon").unwrap();
    let _ = state.equipment.auto_insert_carried(beacon);
    assert!(state.equipment.has_item("horde_beacon"));

    // Deploy and activate beacon
    let deploy_res = state.deploy_horde_beacon();
    assert!(deploy_res);
    assert!(!state.equipment.has_item("horde_beacon")); // consumed on placement

    // Structure placed in world
    assert!(state.structures.iter().any(|s| s.structure_type == StructureType::HordeBeacon));

    // Converging swarm spawned
    assert!(state.zombies.len() > initial_zombies);

    // High intensity acoustic noise event generated alerting valley
    assert!(state.recent_noises.iter().any(|n| n.radius_m >= 180.0));
}

#[test]
fn test_weather_transitions_and_environmental_hazards() {
    use surroundead::engine::GameState;
    use surroundead::world::WeatherType;

    let mut state = GameState::new();
    assert_eq!(state.environment.current_weather, WeatherType::ClearSky);

    // Manually transition to Radiation Storm
    state.environment.current_weather = WeatherType::RadiationStorm;
    assert_eq!(state.environment.current_weather, WeatherType::RadiationStorm);

    // Ambient light calculation during night
    state.environment.time_of_day_hours = 23.0; // 11:00 PM night
    assert!(!state.environment.is_daytime());
    assert_eq!(state.environment.ambient_light_level(), 0.10);

    let formatted = state.environment.formatted_time();
    assert_eq!(formatted, "23:00");
}

#[test]
fn test_tactical_waypoint_placement_and_sectors() {
    use surroundead::engine::GameState;
    use surroundead::math::Position3D;

    let mut state = GameState::new();

    // Check sector naming across the valley map (-250 to +250)
    let center_pos = Position3D::new(0.0, 0.0, 0.0);
    assert_eq!(state.world_map.sector_name_at(&center_pos), "Sector Charlie-3");

    let nw_pos = Position3D::new(-240.0, 0.0, -240.0);
    assert_eq!(state.world_map.sector_name_at(&nw_pos), "Sector Alpha-1");

    let se_pos = Position3D::new(240.0, 0.0, 240.0);
    assert_eq!(state.world_map.sector_name_at(&se_pos), "Sector Echo-5");

    // River detection
    let river_shore = Position3D::new(0.0, 0.0, -20.0);
    assert!(state.world_map.is_near_water(&river_shore));

    let mountain_top = Position3D::new(100.0, 0.0, 100.0);
    assert!(!state.world_map.is_near_water(&mountain_top));

    // Waypoint placement and clearance
    state.world_map.set_waypoint(Position3D::new(50.0, 0.0, -80.0), "Scavenge Rendezvous");
    assert!(state.world_map.waypoint.is_some());
    assert_eq!(state.world_map.waypoint.as_ref().unwrap().label, "Scavenge Rendezvous");

    state.world_map.clear_waypoint();
    assert!(state.world_map.waypoint.is_none());
}

#[test]
fn test_vehicle_obstacle_collision_and_maintenance() {
    use surroundead::engine::GameState;
    use surroundead::items::create_stock_item;
    use surroundead::math::Position3D;

    let mut state = GameState::new();
    let vid = state.vehicles[0].id;
    state.vehicles[0].position = Position3D::new(0.0, 0.0, 0.0);
    state.player_pos = Position3D::new(0.0, 0.0, 0.0);

    // Initial vehicle health
    state.vehicles[0].engine_health = 100.0;
    state.vehicles[0].fuel_liters = 10.0;
    state.vehicles[0].battery_pct = 50.0;

    // Simulate high speed crash into obstacle
    state.vehicles[0].speed_kmh = 55.0;
    let crash_dmg = state.vehicles[0].obstacle_crash();
    assert!(crash_dmg >= 25.0);
    assert!(state.vehicles[0].engine_health < 100.0);
    assert!(state.vehicles[0].speed_kmh < 0.0); // bounced back

    // 1. Refuel with jerrycan
    let can = create_stock_item("gas_can").unwrap();
    let _ = state.equipment.auto_insert_carried(can);
    let prev_fuel = state.vehicles[0].fuel_liters;
    assert!(state.refuel_vehicle(vid).is_ok());
    assert!(state.vehicles[0].fuel_liters > prev_fuel);

    // 2. Patch engine with scrap metal
    let scrap = create_stock_item("scrap_metal").unwrap();
    let _ = state.equipment.auto_insert_carried(scrap);
    let prev_hp = state.vehicles[0].engine_health;
    assert!(state.repair_vehicle_engine(vid, false).is_ok());
    assert!(state.vehicles[0].engine_health > prev_hp);

    // 3. Overhaul engine with vehicle repair kit
    let kit = create_stock_item("vehicle_repair_kit").unwrap();
    let _ = state.equipment.auto_insert_carried(kit);
    let prev_hp2 = state.vehicles[0].engine_health;
    assert!(state.repair_vehicle_engine(vid, true).is_ok());
    assert!(state.vehicles[0].engine_health > prev_hp2);

    // 4. Replace battery with 12V car battery
    let battery = create_stock_item("car_battery").unwrap();
    let _ = state.equipment.auto_insert_carried(battery);
    assert!(state.replace_vehicle_battery(vid).is_ok());
    assert_eq!(state.vehicles[0].battery_pct, 100.0);
}

#[test]
fn test_bandit_tactical_cover_and_combat_barks() {
    use surroundead::ai::{Bandit, BanditState};
    use surroundead::math::Position3D;

    let mut bandit = Bandit::new(99, "Renegade Raider", Position3D::new(0.0, 0.0, 20.0), "ak47");
    assert_eq!(bandit.health, 100.0);
    assert!(!bandit.in_cover);

    // Initial damage
    bandit.take_damage(30.0);
    assert_eq!(bandit.health, 70.0);
    assert!(bandit.shout.is_some());
    assert!(bandit.shout.as_ref().unwrap().is_combat);

    // Heavy damage triggers TakeCover state and 50% damage reduction
    bandit.take_damage(35.0); // health now 35.0 (< 45.0)
    assert_eq!(bandit.state, BanditState::TakeCover);
    assert!(bandit.in_cover);

    // When in cover, 40 damage only inflicts 20 effective damage!
    let prev_hp = bandit.health;
    bandit.take_damage(40.0);
    assert_eq!(bandit.health, prev_hp - 20.0);

    // Drops contain loot
    let drops = bandit.generate_drops();
    assert!(!drops.is_empty());
    assert!(drops.iter().any(|d| d.id == "ak47" || d.id == "ammo_762" || d.id == "cash"));
}

#[test]
fn test_wildlife_hunting_and_drops() {
    use surroundead::ai::{Wildlife, WildlifeType};
    use surroundead::engine::GameState;
    use surroundead::math::Position3D;

    let mut deer = Wildlife::new(1, WildlifeType::WhiteTailedDeer, Position3D::new(0.0, 0.0, 10.0));
    assert!(deer.is_alive());
    assert!(!deer.is_fleeing);

    // Flee when wounded
    let killed = deer.take_damage(15.0);
    assert!(!killed);
    assert!(deer.is_fleeing);

    // Kill deer
    let killed_final = deer.take_damage(35.0);
    assert!(killed_final);

    // Generate harvested venison and pelt hide
    let drops = deer.generate_drops();
    assert!(drops.iter().any(|d| d.id == "raw_venison"));
    assert!(drops.iter().any(|d| d.id == "animal_hide"));

    // Verify GameState wildlife updates
    let mut state = GameState::new();
    assert_eq!(state.wildlife.len(), 5);
    state.update(0.5);
    assert!(state.wildlife[0].is_alive());
}

#[test]
fn test_freshwater_fishing_and_trout_cooking() {
    use surroundead::engine::GameState;
    use surroundead::items::create_stock_item;
    use surroundead::math::Position3D;

    let mut state = GameState::new();

    // 1. Move to river shoreline
    state.player_pos = Position3D::new(0.0, 0.0, -20.0);
    assert!(state.world_map.is_near_water(&state.player_pos));

    // Fishing without rod must fail
    assert!(state.player_fish().is_err());

    // Give player fishing rod
    let rod = create_stock_item("fishing_rod").unwrap();
    let _ = state.equipment.auto_insert_carried(rod);
    assert!(state.equipment.has_item("fishing_rod"));

    // Attempt fish
    let res = state.player_fish();
    assert!(res.is_ok());

    // Manually give raw_fish to test cooking
    let raw_fish = create_stock_item("raw_fish").unwrap();
    let _ = state.equipment.auto_insert_carried(raw_fish);
    assert!(state.equipment.has_item("raw_fish"));

    // Cook trout at campfire
    state.campfire.position = state.player_pos;
    state.campfire.is_lit = true;
    state.campfire.fuel_remaining_s = 60.0;

    let cook_res = state.start_campfire_cooking("cook_fish");
    assert!(cook_res.is_ok());

    // Tick cooking to completion (3.5s)
    let finished = state.campfire.tick(4.0);
    assert!(finished.is_some());
    assert_eq!(finished.as_ref().unwrap().id, "cooked_fish");
}

#[test]
fn test_subterranean_bunker_dungeon_transitions_and_lore() {
    use surroundead::engine::GameState;
    use surroundead::items::create_stock_item;

    let mut state = GameState::new();

    // Position player at Bunker Door 1 (Research Lab Alpha)
    let door_pos = state.bunker_doors[0].position;
    state.player_pos = door_pos;

    // Trying to enter locked door fails
    assert!(state.enter_bunker_facility(1).is_err());

    // Unlock and open door with Level 1 keycard
    let card = create_stock_item("keycard_lvl1").unwrap();
    let _ = state.equipment.auto_insert_carried(card);
    let toggle_res = state.toggle_bunker_door(1);
    assert!(toggle_res.is_ok());
    assert!(state.bunker_doors[0].is_open);

    // Enter underground bunker
    let enter_res = state.enter_bunker_facility(1);
    assert!(enter_res.is_ok());
    assert!(state.active_bunker.is_some());

    let bunker = state.active_bunker.as_ref().unwrap();
    assert_eq!(bunker.facility_id, 1);
    assert_eq!(bunker.rooms.len(), 3);
    assert_eq!(bunker.lore_terminals.len(), 2);
    assert!(!bunker.zombies.is_empty());

    // Read classified research terminal lore log
    let lore_res = state.read_lore_terminal(101);
    assert!(lore_res.is_ok());
    assert!(state.active_bunker.as_ref().unwrap().lore_terminals[0].is_read);

    // Exit bunker back to overland valley
    let exit_res = state.exit_bunker_facility();
    assert!(exit_res.is_ok());
    assert!(state.active_bunker.is_none());
    assert_eq!(state.player_pos.x, door_pos.x);
    assert_eq!(state.player_pos.z, door_pos.z);
}

#[test]
fn test_night_vision_tactical_system() {
    let mut state = GameState::new();

    // Verify initial NVG state (equipped with military NVG by default)
    assert!(state.equipment.nvg.is_some());
    assert!(!state.night_vision.is_active);

    // Toggle NVG ON
    let toggle_on = state.toggle_night_vision();
    assert!(toggle_on.is_ok());
    assert!(state.night_vision.is_active);
    assert_eq!(state.night_vision.equipped_gen, Some(surroundead::survival::NvgGeneration::Military));

    // Verify tint and amplification
    let tint = state.night_vision.get_tint_color().unwrap();
    assert!(tint[1] > 0.8); // High green phosphor
    assert!(state.night_vision.get_light_amplification() >= 3.0);

    // Tick for 50 seconds and check battery drain
    let prev_battery = state.night_vision.battery_charge;
    state.update(50.0);
    assert!(state.night_vision.battery_charge < prev_battery);

    // Test battery depletion
    state.night_vision.battery_charge = 0.0;
    state.update(1.0);
    assert!(!state.night_vision.is_active); // Auto shut off

    // Recharging with 9V battery
    let recharge_res = state.recharge_night_vision();
    assert!(recharge_res.is_ok());
    assert!(state.night_vision.battery_charge >= 50.0);

    // Test unequipped state failure
    state.equipment.nvg = None;
    state.update(0.1);
    let failed_toggle = state.toggle_night_vision();
    assert!(failed_toggle.is_err());
}

#[test]
fn test_authentic_radio_system_and_frequencies() {
    let mut state = GameState::new();

    // Tune to CoyoteCall (91.5 MHz)
    state.tune_radio(91.5);
    assert_eq!(state.radio_frequency_mhz, 91.5);
    let (st1, strength1) = state.radio_system.get_tuned_station().unwrap();
    assert_eq!(st1.id, "coyote_call");
    assert!(strength1 >= 0.95);
    let msg1 = state.radio_system.get_current_transmission_text(10);
    assert!(msg1.contains("Coyote Outpost") || msg1.contains("Camp Timber"));

    // Tune to FireInTheCity (104.2 MHz)
    state.tune_radio(104.2);
    let (st2, _) = state.radio_system.get_tuned_station().unwrap();
    assert_eq!(st2.id, "fire_in_the_city");

    // Tune to MilitaryReinforcements (128.7 MHz) and request an airdrop
    state.tune_radio(128.7);
    let (st3, _) = state.radio_system.get_tuned_station().unwrap();
    assert_eq!(st3.id, "military_reinforcements");

    let airdrop_res = state.call_emergency_airdrop();
    assert!(airdrop_res.is_ok());
    assert!(state.world_events.active_airdrop.is_some());

    // Tune off-frequency (99.0 MHz) -> Static hiss
    state.tune_radio(99.0);
    assert!(state.radio_system.get_tuned_station().is_none());
    let static_txt = state.radio_system.get_current_transmission_text(10);
    assert!(static_txt.contains("STATIC"));
}

#[test]
fn test_game_difficulty_modifiers() {
    let mut state = GameState::new();

    // Default is Standard
    assert_eq!(state.difficulty.level, surroundead::engine::DifficultyLevel::Standard);
    assert_eq!(state.difficulty.player_damage_taken_mult, 1.0);
    assert_eq!(state.difficulty.zombie_health_mult, 1.0);

    // Switch to Nightmare
    state.set_difficulty(surroundead::engine::DifficultyLevel::Nightmare);
    assert_eq!(state.difficulty.level, surroundead::engine::DifficultyLevel::Nightmare);
    assert_eq!(state.difficulty.player_damage_taken_mult, 2.0);
    assert_eq!(state.difficulty.zombie_damage_mult, 2.0);
    assert_eq!(state.difficulty.hunger_thirst_drain_mult, 1.75);

    // Switch to Peaceful
    state.set_difficulty(surroundead::engine::DifficultyLevel::Peaceful);
    assert_eq!(state.difficulty.player_damage_taken_mult, 0.0);
    assert_eq!(state.difficulty.zombie_damage_mult, 0.0);
    assert!(!state.difficulty.hostile_bandits);
}

#[test]
fn test_multislot_save_system() {
    let mut state = GameState::new();
    state.set_difficulty(surroundead::engine::DifficultyLevel::Hardened);
    state.player_credits = 750;
    state.tune_radio(142.0);

    let test_save_dir = std::path::Path::new("target/tmp_saves");
    let _ = std::fs::remove_dir_all(test_save_dir);

    // Save to custom slot
    let save_res = surroundead::persistence::SaveSlotManager::save_slot(
        &state,
        "slot_bunker_run",
        "Bunker Expedition Slot",
        test_save_dir,
    );
    assert!(save_res.is_ok());

    // Verify slot metadata
    let slots = surroundead::persistence::SaveSlotManager::list_slots(test_save_dir);
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].slot_id, "slot_bunker_run");
    assert_eq!(slots[0].difficulty, surroundead::engine::DifficultyLevel::Hardened);

    // Load from slot and verify state
    let loaded = surroundead::persistence::SaveSlotManager::load_slot("slot_bunker_run", test_save_dir);
    assert!(loaded.is_ok());
    let loaded_container = loaded.unwrap();
    assert_eq!(loaded_container.game_state.player_credits, 750);
    assert_eq!(loaded_container.game_state.radio_frequency_mhz, 142.0);
    assert_eq!(loaded_container.game_state.difficulty.level, surroundead::engine::DifficultyLevel::Hardened);

    // Test QuickSave / QuickLoad
    let qsave_res = surroundead::persistence::SaveSlotManager::quick_save(&state, test_save_dir);
    assert!(qsave_res.is_ok());

    let qload_res = surroundead::persistence::SaveSlotManager::quick_load(test_save_dir);
    assert!(qload_res.is_ok());
    assert_eq!(qload_res.unwrap().game_state.player_credits, 750);

    let _ = std::fs::remove_dir_all(test_save_dir);
}





