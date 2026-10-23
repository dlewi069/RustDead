use crate::ai::{AiDirector, Bandit, BanditShot, HordeSpawner, NoiseEvent, Wildlife, WildlifeType, Zombie, ZombieType};
use crate::combat::{create_weapon_preset, BallisticShot, TargetHitbox, WeaponStats};
use crate::crafting::{get_campfire_cooking_recipes, CampfireStation, CraftingRecipe, PlacedStructure, StructureType};
use crate::economy::{generate_starting_bounties, Quest, QuestType, Trader};
use crate::engine::difficulty::{DifficultyLevel, DifficultyModifiers};
use crate::engine::event::GameEvent;
use crate::items::{create_stock_item, get_consumable_effect, Equipment, Item, ItemType, Rarity};
use crate::looting::{LockDifficulty, LockpickResult, LootContainer, LootTier};
use crate::math::{Position3D, Rotation3D};
use crate::progression::{ProgressionSystem, SkillType};
use crate::survival::{MedicalSystem, NightVisionSystem, NvgGeneration, SurvivalStats, TraumaType};
use crate::vehicles::{Vehicle, VehicleType};
use crate::world::{
    generate_valley_bunker_doors, BunkerFacility, BunkerSecurityDoor, EnvironmentSystem,
    HarvestNode, HarvestNodeType, KeycardTier, RadioSystem, WorldEventManager,
    WorldEventType, WorldMap,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub player_pos: Position3D,
    pub player_yaw: f32,
    pub player_pitch: f32,
    pub survival: SurvivalStats,
    pub medical: MedicalSystem,
    /// MedicalComponent's effects (bleeds, broken bone, radiation sickness).
    #[serde(default)]
    pub effects: crate::gameplay::medical::MedicalEffects,
    /// Ultra_Dynamic_Weather's current preset and transition.
    #[serde(default)]
    pub weather: crate::gameplay::weather::WeatherSave,
    pub progression: ProgressionSystem,
    /// PassiveSkillsComponent: Fitness, Toughness, Sneaking... levels and multipliers.
    #[serde(default)]
    pub passive: crate::progression::passive::PassiveSkills,
    /// TechTreeComponent: the unlocked tech tree skills.
    #[serde(default)]
    pub tech: crate::progression::tech::TechTree,
    /// BP_POIManager's Explored? (by actor name): discovered POIs, shown on the map.
    #[serde(default)]
    pub pois_explored: std::collections::BTreeSet<String>,
    /// The player's map markers (BP_PlayerMarker actors).
    #[serde(default)]
    pub map_markers: Vec<crate::gameplay::markers::PlayerMarker>,
    /// BP_PlayerController's journal stats (kills, days survived, distance travelled).
    #[serde(default)]
    pub stats: crate::gameplay::stats::JournalStats,
    /// The world's vehicles (`gameplay::vehicles`); `None` until the spawners have rolled.
    #[serde(default)]
    pub world_vehicles: Option<crate::gameplay::vehicles::VehicleSave>,
    /// NarrativeComponent's quests (`gameplay::quests`).
    #[serde(default)]
    pub quest_log: crate::gameplay::quests::QuestLog,
    /// BP_WaterWell's CurrentWater (EMS actor save), by actor name.
    #[serde(default)]
    pub wells: crate::gameplay::zones::WellSave,
    /// BP_SettlementManager's CampToSpawn (`None` = not picked yet: SettlementSpawned? off).
    #[serde(default)]
    pub settlement: Option<usize>,
    /// Buildables the survivor placed (BuildingComponent.SpawnBuild; `gameplay::building`).
    #[serde(default)]
    pub buildables: Vec<crate::gameplay::building::PlacedBuildable>,
    pub equipment: Equipment,
    pub active_weapon: Option<WeaponStats>,
    pub zombies: Vec<Zombie>,
    pub bandits: Vec<Bandit>,
    pub wildlife: Vec<Wildlife>,
    pub vehicles: Vec<Vehicle>,
    pub containers: Vec<LootContainer>,
    pub structures: Vec<PlacedStructure>,
    pub bunker_doors: Vec<BunkerSecurityDoor>,
    pub active_bunker: Option<BunkerFacility>,
    pub world_map: WorldMap,
    pub trader: Trader,
    pub quests: Vec<Quest>,
    pub world_events: WorldEventManager,
    pub campfire: CampfireStation,
    pub horde_spawner: HordeSpawner,
    pub player_credits: u32,
    pub environment: EnvironmentSystem,
    pub harvest_nodes: Vec<HarvestNode>,
    pub difficulty: DifficultyModifiers,
    pub night_vision: NightVisionSystem,
    pub radio_system: RadioSystem,
    pub radio_frequency_mhz: f32,
    pub active_vehicle_id: Option<u32>,
    pub flashlight_on: bool,
    pub is_sprinting: bool,
    pub recent_noises: Vec<NoiseEvent>,
    pub recent_vehicle_crash: bool,
    pub combat_log: VecDeque<String>,
    pub tick_count: u64,
    pub kills_count: u32,
}

impl GameState {
    pub fn new() -> Self {
        let mut equipment = Equipment::new();

        // Starting loadout
        if let Some(_m4) = create_weapon_preset("m4a1") {
            let m4_item = Item::new("m4a1", "M4A1 Carbine", ItemType::WeaponPrimary, Rarity::Epic, 3.2, 2, 4, 1)
                .with_durability(100.0);
            let _ = equipment.equip_item(m4_item);
        }

        if let Some(bandage) = create_stock_item("bandage") {
            let _ = equipment.pockets.auto_insert(bandage);
        }
        if let Some(water) = create_stock_item("purified_water") {
            let _ = equipment.pockets.auto_insert(water);
        }
        if let Some(food) = create_stock_item("canned_peaches") {
            let _ = equipment.pockets.auto_insert(food);
        }
        if let Some(venison) = create_stock_item("raw_venison") {
            let _ = equipment.pockets.auto_insert(venison);
        }
        if let Some(shrooms) = create_stock_item("forest_mushrooms") {
            let _ = equipment.pockets.auto_insert(shrooms);
        }
        if let Some(plank) = create_stock_item("wood_plank") {
            let _ = equipment.auto_insert_carried(plank);
        }
        if let Some(picks) = create_stock_item("lockpick") {
            let _ = equipment.auto_insert_carried(picks);
        }
        if let Some(bag) = create_stock_item("hiking_backpack") {
            let _ = equipment.equip_item(bag);
        }
        if let Some(card) = create_stock_item("keycard_lvl1") {
            let _ = equipment.auto_insert_carried(card);
        }
        if let Some(acog) = create_stock_item("optic_acog_4x") {
            let _ = equipment.auto_insert_carried(acog);
        }
        if let Some(supp) = create_stock_item("muzzle_suppressor") {
            let _ = equipment.auto_insert_carried(supp);
        }
        if let Some(ammo) = create_stock_item("ammo_556") {
            let _ = equipment.auto_insert_carried(ammo);
        }
        if let Some(splint) = create_stock_item("splint") {
            let _ = equipment.auto_insert_carried(splint);
        }
        if let Some(painkillers) = create_stock_item("painkillers") {
            let _ = equipment.auto_insert_carried(painkillers);
        }
        if let Some(nvg) = create_stock_item("nvg_military") {
            let _ = equipment.equip_item(nvg);
        }
        if let Some(rad) = create_stock_item("emergency_radio") {
            let _ = equipment.equip_item(rad);
        }
        if let Some(bat) = create_stock_item("battery_9v") {
            let _ = equipment.auto_insert_carried(bat);
        }

        let mut zombies = Vec::new();
        let mut rng = rand::thread_rng();

        // Spawn infected population with varied types
        for id in 1..=14 {
            let zx = rng.gen_range(-70.0..70.0);
            let zz = rng.gen_range(20.0..90.0);
            let ztype = match id {
                2 | 6 => ZombieType::Runner,
                4 => ZombieType::Hound,
                8 => ZombieType::Screamer,
                10 => ZombieType::Spitter,
                12 => ZombieType::Bloater,
                14 => ZombieType::Juggernaut,
                _ => ZombieType::Walker,
            };
            zombies.push(Zombie::new(id, ztype, Position3D::new(zx, 0.0, zz)));
        }

        // Spawn hostile human bandits at rogue outposts and elevated watchtowers
        let bandits = vec![
            Bandit::new(101, "Bandit Raider", Position3D::new(-145.0, 0.0, 175.0), "ak47"),
            Bandit::new(102, "Renegade Scout", Position3D::new(-152.0, 0.0, 182.0), "glock17"),
            Bandit::new(103, "Outpost Enforcer", Position3D::new(-138.0, 0.0, 188.0), "remington870"),
            Bandit::new_sniper(104, "Watchtower Marksman", Position3D::new(-113.0, 6.4, -136.0), "remington700"),
        ];

        // Vehicles
        let vehicles = vec![
            Vehicle::new(1, VehicleType::Sedan, Position3D::new(8.0, 0.0, 38.0)),
            Vehicle::new(2, VehicleType::PickupTruck, Position3D::new(-55.0, 0.0, -25.0)),
        ];

        // Containers with locked safes and military caches
        let containers = vec![
            LootContainer::new(1, "Red Rocket Tool Cabinet", Position3D::new(2.0, 0.0, 48.0), LootTier::Hardware, 5, 4),
            LootContainer::new(2, "Abandoned Police Cruiser Trunk", Position3D::new(12.0, 0.0, 40.0), LootTier::Police, 6, 4)
                .with_lock(LockDifficulty::Novice),
            LootContainer::new(3, "Field Hospital Medical Locker", Position3D::new(-105.0, 0.0, 112.0), LootTier::Medical, 5, 5),
            LootContainer::new(4, "FOB Bravo Munitions Crate", Position3D::new(125.0, 0.0, -88.0), LootTier::Military, 6, 6)
                .with_lock(LockDifficulty::Adept),
            LootContainer::new(5, "Lab Alpha Biohazard Safe", Position3D::new(-118.0, 0.0, 118.0), LootTier::Medical, 6, 6),
            LootContainer::new(6, "FOB Bravo Underground Weapons Vault", Position3D::new(132.0, 0.0, -95.0), LootTier::MilitaryHighTier, 7, 6),
            LootContainer::new(7, "Black Site Prototype Vault", Position3D::new(189.0, 0.0, 158.0), LootTier::AirdropMilitary, 8, 6),
        ];

        // Safezone Trader Hank
        let trader = Trader::new_safezone_merchant(1, Position3D::new(-15.0, 0.0, 15.0));

        let mut combat_log = VecDeque::new();
        combat_log.push_back("=====================================================".into());
        combat_log.push_back("SURROUNDEAD ENGINE ACTIVE — Systems Fully Synchronized".into());
        combat_log.push_back("Scavenge loot, level skills, trade at outposts, survive the wasteland.".into());

        // Natural wildlife in the valley pine groves
        let wildlife = vec![
            Wildlife::new(1, WildlifeType::WhiteTailedDeer, Position3D::new(-60.0, 0.0, -60.0)),
            Wildlife::new(2, WildlifeType::WhiteTailedDeer, Position3D::new(-75.0, 0.0, -45.0)),
            Wildlife::new(3, WildlifeType::ForestRabbit, Position3D::new(45.0, 0.0, -15.0)),
            Wildlife::new(4, WildlifeType::ForestRabbit, Position3D::new(55.0, 0.0, -25.0)),
            Wildlife::new(5, WildlifeType::WildBoar, Position3D::new(95.0, 0.0, 45.0)),
        ];

        Self {
            player_pos: Position3D::new(0.0, 0.0, 0.0),
            player_yaw: 0.0,
            player_pitch: 0.0,
            survival: SurvivalStats::new(),
            medical: MedicalSystem::new(),
            effects: Default::default(),
            weather: Default::default(),
            progression: ProgressionSystem::new(),
            passive: Default::default(),
            tech: Default::default(),
            pois_explored: Default::default(),
            map_markers: Vec::new(),
            stats: Default::default(),
            world_vehicles: None,
            quest_log: Default::default(),
            wells: Default::default(),
            settlement: None,
            buildables: Vec::new(),
            equipment,
            active_weapon: create_weapon_preset("m4a1"),
            zombies,
            bandits,
            wildlife,
            vehicles,
            containers,
            structures: Vec::new(),
            bunker_doors: generate_valley_bunker_doors(),
            active_bunker: None,
            world_map: WorldMap::generate_default_valley(),
            trader,
            quests: generate_starting_bounties(),
            world_events: WorldEventManager::new(),
            campfire: CampfireStation::default(),
            horde_spawner: HordeSpawner::new(),
            player_credits: 150,
            environment: EnvironmentSystem::new(),
            harvest_nodes: HarvestNode::generate_valley_nodes(),
            difficulty: DifficultyModifiers::for_level(DifficultyLevel::Standard),
            night_vision: {
                let mut nv = NightVisionSystem::new();
                nv.equip(NvgGeneration::Military);
                nv
            },
            radio_system: RadioSystem::new(),
            radio_frequency_mhz: 89.5,
            active_vehicle_id: None,
            flashlight_on: false,
            is_sprinting: false,
            recent_noises: Vec::new(),
            recent_vehicle_crash: false,
            combat_log,
            tick_count: 0,
            kills_count: 0,
        }
    }

    pub fn log_message(&mut self, msg: impl Into<String>) {
        self.combat_log.push_back(msg.into());
        if self.combat_log.len() > 16 {
            self.combat_log.pop_front();
        }
    }

    /// Advance game world simulation state
    pub fn update(&mut self, dt: f32) {
        self.tick_count += 1;

        // 1. Weather and Day/Night
        self.environment.tick(dt);

        // 1b. Tactical NVG sync & battery tick
        if let Some(ref nvg_item) = self.equipment.nvg {
            let nvg_gen = match nvg_item.id.as_str() {
                "nvg_civilian" => Some(NvgGeneration::Civilian),
                "nvg_military" => Some(NvgGeneration::Military),
                "nvg_specops" => Some(NvgGeneration::SpecOps),
                _ => Some(NvgGeneration::Civilian),
            };
            if self.night_vision.equipped_gen != nvg_gen {
                self.night_vision.equipped_gen = nvg_gen;
            }
        } else {
            self.night_vision.unequip();
        }
        self.night_vision.tick(dt);

        // 2. Radiation check with skill resistance & difficulty scaling
        let map = WorldMap::generate_default_valley();
        let ambient_rad = map.get_radiation_at(&self.player_pos);
        if ambient_rad > 0.0 {
            let resistance = self.progression.radiation_resistance();
            let rad_absorbed = (ambient_rad * (1.0 - resistance) * dt * 0.2 * self.difficulty.infection_rate_mult).max(0.0);
            self.survival.radiation = (self.survival.radiation + rad_absorbed).min(100.0);
        }

        // 3. Survival vitals with skill modifiers & difficulty scaling
        let hunger_mult = self.progression.hunger_decay_multiplier() * self.difficulty.hunger_thirst_drain_mult;
        let thirst_mult = self.progression.thirst_decay_multiplier() * self.difficulty.hunger_thirst_drain_mult;
        self.survival.hunger = (self.survival.hunger - dt * 0.05 * hunger_mult).max(0.0);
        self.survival.thirst = (self.survival.thirst - dt * 0.08 * thirst_mult).max(0.0);
        self.survival.tick(dt, self.is_sprinting && self.active_vehicle_id.is_none());

        // 4. Medical trauma tick
        self.medical.tick(&mut self.survival, dt);

        // 5. Campfire station fuel decay and active cooking tick
        if let Some(cooked_food) = self.campfire.tick(dt) {
            let cname = cooked_food.name.clone();
            let _ = self.equipment.auto_insert_carried(cooked_food);
            self.log_message(format!("CAMPFIRE: Finished cooking {}! Collected into inventory.", cname));
        }

        // 6. World Events (Airdrops & Horde Surges)
        if let Some(event) = self.world_events.tick(dt, &mut self.containers) {
            match event {
                WorldEventType::MilitaryAirdrop => {
                    self.log_message("RADIO: C-130 Hercules spotted overhead! Supply airdrop deployed!");
                }
                WorldEventType::HordeSurge => {
                    let count = self.horde_spawner.spawn_surge(&self.player_pos, &mut self.zombies);
                    self.log_message(format!("WARNING: Infected horde surge detected! {} mutant swarms converging!", count));
                }
                WorldEventType::WanderingBandits => {
                    self.log_message("CAUTION: Armed bandit scouts detected in the valley.");
                }
            }
        }

        // 6b. Roaming dynamic horde spawner tick
        let is_day = self.environment.is_daytime();
        if let Some(horde_alert) = self.horde_spawner.tick(dt, &self.player_pos, is_day, &self.recent_noises, &mut self.zombies) {
            self.log_message(horde_alert);
        }

        // 7. Vehicle physics, obstacle crashes & roadkill impacts
        self.recent_vehicle_crash = false;
        let mut level_up_msg = None;
        if let Some(vid) = self.active_vehicle_id {
            if let Some(veh_idx) = self.vehicles.iter().position(|v| v.id == vid) {
                self.player_pos = self.vehicles[veh_idx].position;
                let veh_speed = self.vehicles[veh_idx].speed_kmh;
                let veh_pos = self.vehicles[veh_idx].position;

                if veh_speed.abs() > 5.0 {
                    self.recent_noises.push(NoiseEvent {
                        position: veh_pos,
                        radius_m: 55.0,
                        intensity: 0.75,
                        source_entity: Some(vid),
                    });
                }

                // Check collisions with world obstacles (trees, boulders)
                if veh_speed.abs() > 14.0 {
                    for obs in &self.world_map.obstacles {
                        if self.vehicles[veh_idx].check_collision_with_circle(&obs.position, obs.radius_m) {
                            let dmg = self.vehicles[veh_idx].obstacle_crash();
                            self.recent_vehicle_crash = true;
                            self.log_message(format!("COLLISION! Vehicle crashed into {}! Engine sustained {:.0} damage!", obs.name, dmg));
                            self.recent_noises.push(NoiseEvent {
                                position: veh_pos,
                                radius_m: 100.0,
                                intensity: 1.0,
                                source_entity: Some(vid),
                            });
                            break;
                        }
                    }
                }

                // High speed roadkill against zombies and bandits
                if veh_speed.abs() > 15.0 {
                    for z in &mut self.zombies {
                        if z.is_alive() && z.position.distance_2d(&veh_pos) < 3.2 {
                            let dmg = self.vehicles[veh_idx].roadkill_impact(z.health);
                            let killed = z.take_damage(dmg);
                            if killed {
                                self.kills_count += 1;
                                let xp = z.zombie_type.xp_reward();
                                if let Some(new_lvl) = self.progression.add_xp(xp) {
                                    level_up_msg = Some(format!("★ LEVEL UP! You reached Level {}! (+1 Skill Point)", new_lvl));
                                }
                            }
                        }
                    }

                    let mut roadkill_msgs = Vec::new();
                    for b in &mut self.bandits {
                        if b.is_alive() && b.position.distance_2d(&veh_pos) < 3.2 {
                            let dmg = self.vehicles[veh_idx].roadkill_impact(b.health);
                            let killed = b.take_damage(dmg);
                            if killed {
                                self.kills_count += 1;
                                roadkill_msgs.push(format!("ROADKILL! Vehicle rammed {}! Target eliminated!", b.name));
                                if let Some(new_lvl) = self.progression.add_xp(65) {
                                    level_up_msg = Some(format!("★ LEVEL UP! You reached Level {}! (+1 Skill Point)", new_lvl));
                                }
                            }
                        }
                    }
                    for msg in roadkill_msgs {
                        self.log_message(msg);
                    }
                }
            }
        }
        if let Some(msg) = level_up_msg {
            self.log_message(msg);
        }

        // 7b. Natural wildlife simulation
        for animal in &mut self.wildlife {
            animal.update(&self.player_pos, &self.recent_noises, dt);
        }

        // 7c. Subterranean Bunker environmental radiation & hazards
        if let Some(bunker) = &mut self.active_bunker {
            let rad_amt = bunker.ambient_radiation * 0.04 * dt;
            self.survival.take_radiation(rad_amt);
        }

        // 8. Zombie AI
        let ai_director = AiDirector::new();
        let is_daytime = self.environment.is_daytime();
        let flashlight = self.flashlight_on || self.active_weapon.as_ref().map(|w| w.has_tactical_light()).unwrap_or(false);
        let p_pos = self.player_pos;

        let mut attacks = Vec::new();
        let mut screamer_alert: Option<Position3D> = None;

        for zombie in &mut self.zombies {
            if let Some(raw_damage) = ai_director.update_zombie(
                zombie,
                &p_pos,
                dt,
                is_daytime,
                flashlight,
                &self.recent_noises,
            ) {
                attacks.push((raw_damage, zombie.zombie_type));
            }

            // Screamer alert shrieks when chasing
            if zombie.zombie_type == ZombieType::Screamer && zombie.state == crate::ai::AiState::Chase {
                screamer_alert = Some(zombie.position);
            }
        }

        if let Some(scream_pos) = screamer_alert {
            self.recent_noises.push(NoiseEvent {
                position: scream_pos,
                radius_m: 120.0,
                intensity: 1.0,
                source_entity: None,
            });
        }

        for (raw_damage, ztype) in attacks {
            let reduction = self.equipment.armor_damage_reduction();
            let actual_damage = raw_damage * self.difficulty.zombie_damage_mult * (1.0 - reduction) * self.difficulty.player_damage_taken_mult;
            if actual_damage > 0.0 {
                self.survival.take_damage(actual_damage, true, 0.25 * self.difficulty.infection_rate_mult);

                // Chance to cause fracture or heavy bleed from heavy hits
                if actual_damage > 30.0 {
                    self.medical.add_trauma(TraumaType::Fracture, 1.0, 60.0);
                    self.log_message("CRUNCH! Your leg has been fractured from the blow! Limping!");
                }

                self.log_message(format!(
                    "OUCH! {:?} hit you for {:.1} damage (absorbed {:.0}% by armor)!",
                    ztype,
                    actual_damage,
                    reduction * 100.0
                ));
            }
        }

        // 9. Bandit AI update & tactical shooting
        let mut bandit_shots: Vec<BanditShot> = Vec::new();
        if self.difficulty.hostile_bandits {
            for bandit in &mut self.bandits {
                if let Some(shot) = bandit.update(&p_pos, dt) {
                    bandit_shots.push(shot);
                }
            }
        }

        for shot in bandit_shots {
            self.recent_noises.push(NoiseEvent {
                position: shot.origin,
                radius_m: 90.0,
                intensity: 0.9,
                source_entity: Some(shot.bandit_id),
            });

            if shot.hit_player {
                let armor_red = self.equipment.armor_damage_reduction();
                let dmg = shot.damage * (1.0 - armor_red) * self.difficulty.player_damage_taken_mult;
                if dmg > 0.0 {
                    self.survival.take_damage(dmg, true, 0.40);
                    self.log_message(format!("INCOMING FIRE! Bandit bullet struck you for {:.1} damage!", dmg));
                }
            } else {
                self.log_message("WHIZZZ! Bandit round flew past your head!");
            }
        }

        // 10. Harvest nodes tick
        for node in &mut self.harvest_nodes {
            node.tick(dt);
        }

        // 11. Clear transient sound events
        self.recent_noises.clear();
    }

    /// Fire active weapon with ballistics against zombies and bandits
    pub fn player_shoot(&mut self) -> Option<GameEvent> {
        let (weapon_name, is_empty, is_melee) = match &self.active_weapon {
            Some(w) => (w.name.clone(), w.current_ammo == 0, w.is_melee()),
            None => return None,
        };

        if is_empty && !is_melee {
            self.log_message(format!("*CLICK* {} is empty! Press [R] to reload.", weapon_name));
            return None;
        }

        let (shot, wname) = {
            let weapon = self.active_weapon.as_mut()?;
            if !weapon.fire() {
                return None;
            }
            let rot = Rotation3D::new(self.player_yaw, self.player_pitch);
            let forward = rot.forward_vector();
            (BallisticShot::from_weapon(self.player_pos, forward, weapon), weapon.name.clone())
        };

        // Sound event alerts infected
        self.recent_noises.push(NoiseEvent {
            position: self.player_pos,
            radius_m: shot.noise_radius,
            intensity: 1.0,
            source_entity: Some(0),
        });

        // 1. Raycast test against Zombies
        let mut hit_zombie: Option<(usize, crate::combat::HitZone, f32)> = None;
        for (idx, zombie) in self.zombies.iter().enumerate() {
            if !zombie.is_alive() {
                continue;
            }
            let hitbox = TargetHitbox::for_humanoid(zombie.id, zombie.position);
            if let Some((zone, dist)) = hitbox.test_hit(&shot.ray) {
                if dist <= shot.max_range {
                    match hit_zombie {
                        Some((_, _, best_dist)) if dist < best_dist => hit_zombie = Some((idx, zone, dist)),
                        None => hit_zombie = Some((idx, zone, dist)),
                        _ => {}
                    }
                }
            }
        }

        // 2. Raycast test against Bandits
        let mut hit_bandit: Option<(usize, f32)> = None;
        for (idx, bandit) in self.bandits.iter().enumerate() {
            if !bandit.is_alive() {
                continue;
            }
            let hitbox = TargetHitbox::for_humanoid(bandit.id, bandit.position);
            if let Some((_zone, dist)) = hitbox.test_hit(&shot.ray) {
                if dist <= shot.max_range {
                    match hit_bandit {
                        Some((_, best_dist)) if dist < best_dist => hit_bandit = Some((idx, dist)),
                        None => hit_bandit = Some((idx, dist)),
                        _ => {}
                    }
                }
            }
        }

        // 3. Raycast test against Wildlife
        let mut hit_wildlife: Option<(usize, f32)> = None;
        for (idx, animal) in self.wildlife.iter().enumerate() {
            if !animal.is_alive() {
                continue;
            }
            let hitbox = TargetHitbox::for_humanoid(animal.id, animal.position);
            if let Some((_zone, dist)) = hitbox.test_hit(&shot.ray) {
                if dist <= shot.max_range {
                    match hit_wildlife {
                        Some((_, best_dist)) if dist < best_dist => hit_wildlife = Some((idx, dist)),
                        None => hit_wildlife = Some((idx, dist)),
                        _ => {}
                    }
                }
            }
        }

        if let Some((z_idx, zone, dist)) = hit_zombie {
            let base_dmg = shot.calculate_damage_at_distance(dist);
            let melee_bonus = if shot.noise_radius < 5.0 { self.progression.melee_damage_multiplier() } else { 1.0 };
            let final_dmg = base_dmg * zone.damage_multiplier(shot.headshot_mult) * melee_bonus;

            let killed = self.zombies[z_idx].take_damage(final_dmg);
            if killed {
                self.kills_count += 1;
                let xp = self.zombies[z_idx].zombie_type.xp_reward();
                if let Some(new_lvl) = self.progression.add_xp(xp) {
                    self.log_message(format!("★ LEVEL UP! You reached Level {}! (+1 Skill Point)", new_lvl));
                }

                // Advance bounties
                for q in &mut self.quests {
                    if q.quest_type == QuestType::KillZombies {
                        q.advance(1);
                    }
                }
            }

            let hit_msg = format!(
                "HIT! Shot {:?} in the {:?} for {:.1} damage at {:.1}m! {}",
                self.zombies[z_idx].zombie_type,
                zone,
                final_dmg,
                dist,
                if killed { ">>> TARGET ELIMINATED <<<" } else { "" }
            );
            self.log_message(hit_msg);

            Some(GameEvent::ZombieHit {
                zombie_id: self.zombies[z_idx].id,
                damage: final_dmg,
                hit_zone: format!("{:?}", zone),
                killed,
            })
        } else if let Some((b_idx, dist)) = hit_bandit {
            let final_dmg = shot.calculate_damage_at_distance(dist);
            let killed = self.bandits[b_idx].take_damage(final_dmg);
            if killed {
                self.kills_count += 1;
                if let Some(new_lvl) = self.progression.add_xp(85) {
                    self.log_message(format!("★ LEVEL UP! You reached Level {}! (+1 Skill Point)", new_lvl));
                }

                // Advance bandit bounty
                for q in &mut self.quests {
                    if q.quest_type == QuestType::KillBandits {
                        q.advance(1);
                    }
                }

                // Collect dropped bandit loot
                let drops = self.bandits[b_idx].generate_drops();
                for drop in drops {
                    let _ = self.equipment.pockets.auto_insert(drop);
                }
                self.log_message(format!("ELIMINATED {}! Looted their gear & credits.", self.bandits[b_idx].name));
            } else {
                self.log_message(format!("HIT! Bandit struck for {:.1} damage at {:.1}m!", final_dmg, dist));
            }

            Some(GameEvent::GunshotFired {
                weapon_name: wname,
                position: self.player_pos,
                noise_radius: shot.noise_radius,
            })
        } else if let Some((w_idx, dist)) = hit_wildlife {
            let final_dmg = shot.calculate_damage_at_distance(dist);
            let killed = self.wildlife[w_idx].take_damage(final_dmg);
            if killed {
                self.kills_count += 1;
                if let Some(new_lvl) = self.progression.add_xp(40) {
                    self.log_message(format!("★ LEVEL UP! You reached Level {}! (+1 Skill Point)", new_lvl));
                }
                let drops = self.wildlife[w_idx].generate_drops();
                for drop in drops {
                    let _ = self.equipment.pockets.auto_insert(drop);
                }
                self.log_message(format!("HUNTED {}! Harvested fresh meat and pelts.", self.wildlife[w_idx].name));
            } else {
                self.log_message(format!("HIT! {} struck for {:.1} damage at {:.1}m! Fleeing into brush!", self.wildlife[w_idx].name, final_dmg, dist));
            }

            Some(GameEvent::GunshotFired {
                weapon_name: wname,
                position: self.player_pos,
                noise_radius: shot.noise_radius,
            })
        } else {
            self.log_message(format!("Fired {} into empty air.", wname));
            Some(GameEvent::GunshotFired {
                weapon_name: wname,
                position: self.player_pos,
                noise_radius: shot.noise_radius,
            })
        }
    }

    /// Reload firearm from carried ammo (pockets or backpack)
    pub fn player_reload(&mut self) -> bool {
        let (wname, caliber, cur_ammo, max_cap) = match &self.active_weapon {
            Some(w) if !w.is_melee() => (w.name.clone(), w.caliber, w.current_ammo, w.effective_mag_capacity()),
            _ => return false,
        };

        let candidate_ammo_ids = match caliber {
            crate::combat::Caliber::NineMm => vec!["ammo_9mm"],
            crate::combat::Caliber::FortyFiveAcp => vec!["ammo_45acp"],
            crate::combat::Caliber::FiveFiveSix => vec!["ammo_556"],
            crate::combat::Caliber::SevenSixTwo => vec!["ammo_762"],
            crate::combat::Caliber::TwelveGauge => vec!["ammo_12gauge", "ammo_12ga"],
            crate::combat::Caliber::ThreeZeroEight => vec!["ammo_308"],
            crate::combat::Caliber::FiftyBmg => vec!["ammo_50bmg"],
            _ => vec!["ammo_general", "ammo_tin"],
        };

        let mut available = 0;
        let mut best_ammo_id = candidate_ammo_ids[0];
        for &id in &candidate_ammo_ids {
            let count_p = self.equipment.pockets.count_item(id);
            let count_b = self.equipment.get_backpack_grid().map(|bp| bp.count_item(id)).unwrap_or(0);
            if count_p + count_b > 0 {
                available = count_p + count_b;
                best_ammo_id = id;
                break;
            }
        }

        if available == 0 {
            self.log_message(format!("No matching ammo ({}) in inventory!", best_ammo_id));
            return false;
        }

        let needed = max_cap.saturating_sub(cur_ammo);
        let to_take = needed.min(available);
        if to_take == 0 {
            return false;
        }

        let mut taken = 0;
        let take_p = to_take.min(self.equipment.pockets.count_item(best_ammo_id));
        if take_p > 0 && self.equipment.pockets.remove_amount(best_ammo_id, take_p) {
            taken += take_p;
        }
        let take_b = to_take - taken;
        if take_b > 0 {
            if let Some(bp) = self.equipment.get_backpack_grid_mut() {
                if bp.remove_amount(best_ammo_id, take_b) {
                    taken += take_b;
                }
            }
        }

        if taken > 0 {
            let info = if let Some(weapon) = self.active_weapon.as_mut() {
                weapon.reload(taken);
                Some((weapon.current_ammo, weapon.effective_mag_capacity()))
            } else {
                None
            };
            if let Some((cur, max)) = info {
                self.log_message(format!("Reloaded {} (+{} rounds). Mag now {}/{}.", wname, taken, cur, max));
            }
            true
        } else {
            false
        }
    }

    /// Lockpicking attempt on closest locked container
    pub fn player_lockpick(&mut self, tension: f32) -> Option<LockpickResult> {
        let p_pos = self.player_pos;
        let skill_mult = self.progression.lockpick_tolerance_multiplier();

        let mut unlocked_xp = 0;
        let mut result = None;

        for container in &mut self.containers {
            if container.can_interact(&p_pos) {
                if let Some(lock) = &mut container.lock {
                    if lock.is_locked {
                        let res = lock.turn_cylinder(tension, skill_mult);
                        if let LockpickResult::SuccessUnlocked { xp } = res {
                            unlocked_xp = xp;
                        }
                        result = Some(res);
                        break;
                    }
                }
            }
        }

        if unlocked_xp > 0 {
            if let Some(new_lvl) = self.progression.add_xp(unlocked_xp) {
                self.log_message(format!("★ LEVEL UP! You reached Level {}!", new_lvl));
            }
            for q in &mut self.quests {
                if q.quest_type == QuestType::LockpickSafes {
                    q.advance(1);
                }
            }
            self.log_message(format!("CLICK! Lock successfully picked! (+{} XP)", unlocked_xp));
        }

        result
    }

    /// Attach a weapon mod to the active weapon
    pub fn player_attach_mod(&mut self, mod_id: &str) -> bool {
        let w_name = if let Some(w) = self.active_weapon.as_mut() {
            w.attach_mod(mod_id);
            Some(w.name.clone())
        } else {
            None
        };

        if let Some(name) = w_name {
            self.log_message(format!("Installed {} onto {}.", mod_id, name));
            true
        } else {
            false
        }
    }

    /// Cook food or boil clean water at campfire
    pub fn player_cook(&mut self, recipe_id: &str) -> bool {
        let recipes = get_campfire_cooking_recipes();
        let recipe = match recipes.iter().find(|r| r.id == recipe_id) {
            Some(r) => r,
            None => return false,
        };

        if !self.campfire.is_lit {
            self.log_message("Campfire is out! Add wood planks to ignite.");
            return false;
        }

        if self.equipment.pockets.count_item(&recipe.input_item_id) < recipe.input_count {
            self.log_message(format!("Missing raw ingredients: {} x{}", recipe.input_item_id, recipe.input_count));
            return false;
        }

        self.equipment.pockets.remove_amount(&recipe.input_item_id, recipe.input_count);
        let _ = self.equipment.pockets.auto_insert(recipe.output_item.clone());
        self.log_message(format!("Cooked {}! Transferred to inventory.", recipe.name));
        true
    }

    /// Sleep to advance time and recover fatigue
    pub fn player_sleep(&mut self, hours: f32) {
        self.environment.time_of_day_hours = (self.environment.time_of_day_hours + hours) % 24.0;
        self.survival.stamina = self.survival.max_stamina;
        self.survival.health = (self.survival.health + hours * 8.0).min(self.survival.max_health);
        self.survival.hunger = (self.survival.hunger - hours * 3.0).max(0.0);
        self.survival.thirst = (self.survival.thirst - hours * 4.0).max(0.0);
        self.log_message(format!("Rested for {:.0} hours. Daylight advanced to {:.1}:00.", hours, self.environment.time_of_day_hours));
    }

    /// Claim completed quest bounty rewards
    pub fn claim_quest(&mut self, quest_id: u32) -> bool {
        let mut claimed = None;
        for q in &mut self.quests {
            if q.id == quest_id && q.is_completed && !q.is_claimed {
                q.is_claimed = true;
                claimed = Some((q.title.clone(), q.reward_credits, q.reward_xp));
                break;
            }
        }

        if let Some((title, credits, xp)) = claimed {
            self.player_credits += credits;
            if let Some(new_lvl) = self.progression.add_xp(xp) {
                self.log_message(format!("★ LEVEL UP! You reached Level {}!", new_lvl));
            }
            self.log_message(format!("CLAIMED BOUNTY: {} (+{} Credits, +{} XP)!", title, credits, xp));
            true
        } else {
            false
        }
    }

    /// Upgrade skill perk using skill point
    pub fn upgrade_skill(&mut self, skill: SkillType) -> Result<u32, &'static str> {
        self.progression.upgrade_skill(skill)
    }

    /// Use medical item, food, or drink
    pub fn use_item(&mut self, item_id: &str) -> bool {
        if !self.equipment.has_item(item_id) {
            return false;
        }

        // First test specialized medical system
        if let Some(med_msg) = self.medical.apply_medical_item(&mut self.survival, item_id) {
            let _ = self.equipment.find_and_remove_item(item_id);
            self.log_message(med_msg);
            return true;
        }

        // Test standard consumable effects
        if let Some(effect) = get_consumable_effect(item_id) {
            if self.equipment.find_and_remove_item(item_id).is_some() {
                effect.apply(&mut self.survival);
                self.log_message(format!("Consumed {}. Vitals updated.", item_id));
                return true;
            }
        }
        false
    }

    /// Craft base recipe
    pub fn craft_recipe(&mut self, recipe_id: &str) -> bool {
        let recipes = CraftingRecipe::get_all_recipes();
        let recipe = match recipes.iter().find(|r| r.id == recipe_id) {
            Some(r) => r,
            None => return false,
        };

        for ing in &recipe.ingredients {
            if self.equipment.pockets.count_item(&ing.item_id) < ing.amount {
                self.log_message(format!("Missing ingredient: {} x{}", ing.item_id, ing.amount));
                return false;
            }
        }

        for ing in &recipe.ingredients {
            self.equipment.pockets.remove_amount(&ing.item_id, ing.amount);
        }

        if let Some(crafted) = recipe.instantiate_result() {
            let name = crafted.name.clone();
            if self.equipment.pockets.auto_insert(crafted).is_ok() {
                self.log_message(format!("Crafted {}!", name));
                return true;
            }
        }
        false
    }

    /// Place a base building structure
    pub fn place_structure(&mut self, structure_type: StructureType) -> bool {
        let sid = (self.structures.len() as u32) + 1;
        let place_pos = Position3D::new(
            self.player_pos.x + self.player_yaw.to_radians().sin() * 2.5,
            self.player_pos.y,
            self.player_pos.z + self.player_yaw.to_radians().cos() * 2.5,
        );

        let structure = PlacedStructure::new(sid, structure_type, place_pos, self.player_yaw);
        let sname = format!("{:?}", structure_type);
        self.structures.push(structure);
        self.log_message(format!("Placed {} at ({:.1}, {:.1})!", sname, place_pos.x, place_pos.z));
        true
    }

    /// Interact with nearby objects
    pub fn player_interact(&mut self) -> bool {
        // Vehicle entry/exit
        if let Some(vid) = self.active_vehicle_id {
            let vname = self.vehicles.iter_mut().find(|v| v.id == vid).map(|veh| {
                veh.stop_engine();
                veh.name.clone()
            });
            if let Some(name) = vname {
                self.log_message(format!("Exited {}.", name));
            }
            self.active_vehicle_id = None;
            return true;
        }

        let entered = self.vehicles.iter_mut().find(|veh| veh.position.distance_to(&self.player_pos) <= 3.5).map(|veh| {
            let _ = veh.start_engine();
            (veh.id, veh.name.clone(), veh.fuel_liters, veh.speed_kmh)
        });

        if let Some((id, name, fuel, speed)) = entered {
            self.active_vehicle_id = Some(id);
            self.log_message(format!("Entered {} (Fuel: {:.1}L, Speed: {:.0}km/h).", name, fuel, speed));
            return true;
        }

        // Bunker security doors
        if let Some((_, _)) = self.try_interact_bunker_door() {
            return true;
        }

        // Loot container
        let p_pos = self.player_pos;
        let container_info = self.containers.iter_mut().find(|c| c.can_interact(&p_pos)).map(|container| {
            if container.is_locked() {
                (container.name.clone(), 0, true)
            } else {
                container.is_searched = true;
                (container.name.clone(), container.storage.items.len(), false)
            }
        });

        if let Some((name, count, locked)) = container_info {
            if locked {
                self.log_message(format!("{} is LOCKED! Press [L] with lockpicks to pick the lock.", name));
            } else {
                self.log_message(format!("Opened {}. Found {} items inside.", name, count));
            }
            return true;
        }

        // Harvest nearby pine tree or rock boulder
        if let Some((node_type, iname, _, _, _)) = self.harvest_nearest(3.5) {
            match node_type {
                HarvestNodeType::PineTree => self.log_message(format!("Chopped tree! Obtained {}.", iname)),
                HarvestNodeType::RockBoulder => self.log_message(format!("Mined rock! Obtained {}.", iname)),
            }
            return true;
        }

        false
    }

    /// Try interacting with nearest electronic bunker security door
    pub fn try_interact_bunker_door(&mut self) -> Option<(bool, String)> {
        let p_pos = self.player_pos;
        let player_tier = if self.equipment.has_item("keycard_all_access") || self.equipment.has_item("all_access_keycard") {
            Some(KeycardTier::AllAccess)
        } else if self.equipment.has_item("keycard_lvl3") {
            Some(KeycardTier::Level3)
        } else if self.equipment.has_item("keycard_lvl2") {
            Some(KeycardTier::Level2)
        } else if self.equipment.has_item("keycard_lvl1") {
            Some(KeycardTier::Level1)
        } else if self.equipment.has_item("keycard_lvl0") {
            Some(KeycardTier::Level0)
        } else {
            None
        };

        for door in &mut self.bunker_doors {
            if door.can_interact(&p_pos) {
                match door.try_toggle(player_tier) {
                    Ok(is_open) => {
                        let state_str = if is_open { "OPEN" } else { "CLOSED" };
                        let msg = format!("SECURITY CLEARANCE ACCEPTED: {} [{}]", door.name, state_str);
                        self.log_message(msg.clone());
                        return Some((true, msg));
                    }
                    Err(err) => {
                        self.log_message(err.clone());
                        return Some((false, err));
                    }
                }
            }
        }
        None
    }

    /// Toggle a specific bunker security door by ID with keycard verification
    pub fn toggle_bunker_door(&mut self, door_id: u32) -> Result<bool, String> {
        let player_tier = if self.equipment.has_item("keycard_all_access") || self.equipment.has_item("all_access_keycard") {
            Some(KeycardTier::AllAccess)
        } else if self.equipment.has_item("keycard_lvl3") {
            Some(KeycardTier::Level3)
        } else if self.equipment.has_item("keycard_lvl2") {
            Some(KeycardTier::Level2)
        } else if self.equipment.has_item("keycard_lvl1") {
            Some(KeycardTier::Level1)
        } else if self.equipment.has_item("keycard_lvl0") {
            Some(KeycardTier::Level0)
        } else {
            None
        };

        if let Some(door) = self.bunker_doors.iter_mut().find(|d| d.id == door_id) {
            match door.try_toggle(player_tier) {
                Ok(is_open) => {
                    let state_str = if is_open { "OPEN" } else { "CLOSED" };
                    let msg = format!("SECURITY CLEARANCE ACCEPTED: {} [{}]", door.name, state_str);
                    self.log_message(msg);
                    Ok(is_open)
                }
                Err(err) => {
                    self.log_message(err.clone());
                    Err(err)
                }
            }
        } else {
            Err(format!("Bunker door #{} not found", door_id))
        }
    }

    /// Attach weapon modification to active weapon from survivor inventory
    pub fn attach_weapon_mod(&mut self, attachment_item_id: &str) -> bool {
        if self.active_weapon.is_none() {
            return false;
        }

        let item = match self.equipment.find_and_remove_item(attachment_item_id) {
            Some(it) => it,
            None => return false,
        };

        let slot = if item.id.starts_with("optic_") || item.id.starts_with("scope_") {
            crate::combat::AttachmentSlot::Optic
        } else if item.id.starts_with("muzzle_") || item.id == "suppressor" {
            crate::combat::AttachmentSlot::Muzzle
        } else if item.id.starts_with("mag_") {
            crate::combat::AttachmentSlot::Magazine
        } else {
            crate::combat::AttachmentSlot::Tactical
        };

        let mut old_mod_to_store = None;
        let mut weapon_name = String::new();

        if let Some(w) = self.active_weapon.as_mut() {
            old_mod_to_store = w.detach_mod(slot);
            w.attach_mod(&item.id);
            weapon_name = w.name.clone();
        }

        if let Some(old_mod_id) = old_mod_to_store {
            if let Some(old_item) = create_stock_item(&old_mod_id) {
                let _ = self.equipment.auto_insert_carried(old_item);
            }
        }

        self.log_message(format!("MOD MOUNTED: Installed {} onto {}.", item.name, weapon_name));
        true
    }

    /// Detach weapon modification from active weapon back to inventory
    pub fn detach_weapon_mod(&mut self, slot: crate::combat::AttachmentSlot) -> bool {
        let detached_mod_id = self.active_weapon.as_mut().and_then(|w| w.detach_mod(slot));

        if let Some(mod_id) = detached_mod_id {
            if let Some(item) = create_stock_item(&mod_id) {
                let name = item.name.clone();
                let _ = self.equipment.auto_insert_carried(item);
                self.log_message(format!("MOD DETACHED: Removed {} from weapon.", name));
                return true;
            }
        }
        false
    }

    /// Deploy and activate a Horde Beacon device
    pub fn deploy_horde_beacon(&mut self) -> bool {
        let p_pos = self.player_pos;
        if self.equipment.has_item("horde_beacon") {
            let _ = self.equipment.find_and_remove_item("horde_beacon");
            self.structures.push(PlacedStructure::new(
                (self.structures.len() as u32) + 1,
                StructureType::HordeBeacon,
                p_pos,
                self.player_yaw,
            ));
            let spawned = self.horde_spawner.spawn_surge(&p_pos, &mut self.zombies);
            self.recent_noises.push(NoiseEvent {
                position: p_pos,
                radius_m: 200.0,
                intensity: 1.0,
                source_entity: None,
            });
            self.log_message(format!(
                "HORDE BEACON DEPLOYED & ACTIVATED! Siren echoing across the valley! {} apex mutants charging your position!",
                spawned
            ));
            true
        } else {
            self.log_message("No Horde Beacon device in inventory!");
            false
        }
    }

    /// Attempt to harvest nearest pine tree or rock boulder
    pub fn harvest_nearest(&mut self, max_distance: f32) -> Option<(HarvestNodeType, String, u32, bool, Position3D)> {
        let p_pos = self.player_pos;
        let mut nearest_idx = None;
        let mut min_dist = max_distance;

        for (idx, node) in self.harvest_nodes.iter().enumerate() {
            if node.is_depleted { continue; }
            let d = node.position.distance_to(&p_pos);
            if d < min_dist {
                min_dist = d;
                nearest_idx = Some(idx);
            }
        }

        if let Some(idx) = nearest_idx {
            let damage = if let Some(w) = &self.active_weapon {
                if w.is_melee() { 40.0 } else { 25.0 }
            } else {
                25.0
            };

            let node_pos = self.harvest_nodes[idx].position;
            let node_type = self.harvest_nodes[idx].node_type;
            if let Some((item, xp, was_felled)) = self.harvest_nodes[idx].strike(damage) {
                let iname = item.name.clone();
                let icount = item.stack_size;
                let _ = self.equipment.auto_insert_carried(item);
                if let Some(new_lvl) = self.progression.add_xp(xp as u64) {
                    self.log_message(format!("★ LEVEL UP! You reached Level {}! (+1 Skill Point)", new_lvl));
                }
                let act_msg = match node_type {
                    HarvestNodeType::PineTree => {
                        if was_felled {
                            format!("FELLED Pine Tree! Harvested {}x {} (+{} XP).", icount, iname, xp)
                        } else {
                            format!("Chopped Pine Tree: +{}x {} (+{} XP).", icount, iname, xp)
                        }
                    }
                    HarvestNodeType::RockBoulder => {
                        if was_felled {
                            format!("SHATTERED Boulder! Mined {}x {} (+{} XP).", icount, iname, xp)
                        } else {
                            format!("Mined Boulder: +{}x {} (+{} XP).", icount, iname, xp)
                        }
                    }
                };
                self.log_message(act_msg);
                return Some((node_type, iname, xp, was_felled, node_pos));
            }
        }

        None
    }

    /// Find nearby container within range
    pub fn get_nearby_container_id(&self, max_distance: f32) -> Option<u32> {
        let p_pos = self.player_pos;
        self.containers
            .iter()
            .find(|c| c.position.distance_to(&p_pos) <= max_distance)
            .map(|c| c.id)
    }

    /// Loot all items from a container and transfer them into survivor inventory
    pub fn loot_all_from_container(&mut self, container_id: u32) -> (String, usize) {
        let mut container_name = "Container".to_string();
        let mut looted_count = 0;

        if let Some(c) = self.containers.iter_mut().find(|c| c.id == container_id) {
            container_name = c.name.clone();
            let items = std::mem::take(&mut c.storage.items);
            for placed in items {
                if self.equipment.auto_insert_carried(placed.item.clone()).is_ok() {
                    looted_count += 1;
                } else {
                    let _ = c.storage.auto_insert(placed.item);
                }
            }
            c.is_searched = true;
        }

        if looted_count > 0 {
            self.log_message(format!("Looted {} items from {}.", looted_count, container_name));
        }

        (container_name, looted_count)
    }

    /// Loot a single item by index from a container
    pub fn loot_item_from_container(&mut self, container_id: u32, item_idx: usize) -> Option<String> {
        let mut result = None;
        if let Some(c) = self.containers.iter_mut().find(|c| c.id == container_id) {
            if item_idx < c.storage.items.len() {
                let placed = c.storage.items.remove(item_idx);
                let iname = placed.item.name.clone();
                let cname = c.name.clone();
                if self.equipment.auto_insert_carried(placed.item).is_ok() {
                    c.is_searched = true;
                    result = Some((iname, cname));
                }
            }
        }
        if let Some((iname, cname)) = result {
            self.log_message(format!("Looted {} from {}.", iname, cname));
            return Some(iname);
        }
        None
    }

    /// Tune the emergency radio frequency
    pub fn tune_radio(&mut self, freq: f32) {
        self.radio_frequency_mhz = (freq.clamp(80.0, 180.0) * 10.0).round() / 10.0;
        self.radio_system.tune(self.radio_frequency_mhz);
        if let Some((station, strength)) = self.radio_system.get_tuned_station() {
            if strength >= 0.85 {
                self.log_message(format!("RADIO: Locked on to [{}] at {:.1} MHz (Signal: {:.0}%)", station.name, station.frequency_mhz, strength * 100.0));
            }
        }
    }

    /// Toggle Tactical Night Vision Goggles
    pub fn toggle_night_vision(&mut self) -> Result<bool, &'static str> {
        if let Some(ref nvg_item) = self.equipment.nvg {
            let nvg_gen = match nvg_item.id.as_str() {
                "nvg_civilian" => Some(NvgGeneration::Civilian),
                "nvg_military" => Some(NvgGeneration::Military),
                "nvg_specops" => Some(NvgGeneration::SpecOps),
                _ => Some(NvgGeneration::Civilian),
            };
            self.night_vision.equipped_gen = nvg_gen;
        } else {
            self.night_vision.unequip();
        }

        let res = self.night_vision.toggle();
        if let Ok(active) = res {
            if active {
                self.log_message("NVG: Tactical Night Vision ACTIVATED.");
            } else {
                self.log_message("NVG: Tactical Night Vision DEACTIVATED.");
            }
        }
        res
    }

    /// Insert 9V battery to recharge Night Vision Goggles
    pub fn recharge_night_vision(&mut self) -> Result<f32, &'static str> {
        if self.equipment.find_and_remove_item("battery_9v").is_some() {
            let restored = self.night_vision.recharge(50.0);
            self.log_message(format!("BATTERY: Inserted 9V alkaline battery into NVG (+{:.0}% charge, now {:.0}%).", restored, self.night_vision.battery_charge));
            Ok(restored)
        } else {
            Err("No 9V battery found in inventory!")
        }
    }

    /// Set Game Difficulty Level
    pub fn set_difficulty(&mut self, level: DifficultyLevel) {
        self.difficulty = DifficultyModifiers::for_level(level);
        self.log_message(format!("DIFFICULTY: Level set to {:?}.", level));
    }

    /// Call emergency airdrop near the player position
    pub fn call_emergency_airdrop(&mut self) -> Result<Position3D, &'static str> {
        if self.player_credits < 100 {
            return Err("Insufficient credits! Military airdrop requisition costs 100 credits.");
        }
        self.player_credits -= 100;

        let drop_pos = Position3D::new(self.player_pos.x + 22.0, 0.0, self.player_pos.z + 18.0);
        self.world_events.active_airdrop = Some(crate::world::ActiveAirdrop {
            position: drop_pos,
            parachute_altitude: 45.0,
            has_landed: false,
            smoke_duration_s: 120.0,
            container_id: 999,
        });

        // Spawn airdrop crate container
        let airdrop_crate = LootContainer::new(
            999,
            "Military Emergency Airdrop Crate [High-Tier]",
            drop_pos,
            LootTier::AirdropMilitary,
            6,
            8,
        );
        self.containers.push(airdrop_crate);
        self.log_message("RADIO: HQ confirmed emergency airdrop dispatched! C-130 cargo drop inbound!");
        Ok(drop_pos)
    }

    /// Buy an item from Trader Hank's stock
    pub fn buy_from_trader(&mut self, offer_index: usize) -> Result<String, String> {
        let (item, price) = self.trader.sell_to_player(offer_index, self.player_credits)
            .map_err(|e| e.to_string())?;

        let iname = item.name.clone();
        if self.equipment.auto_insert_carried(item.clone()).is_err() {
            // Restore credits and trader offer if inventory full
            self.trader.offers.insert(offer_index, crate::economy::TraderOffer { item, price_credits: price });
            self.trader.credits -= price;
            return Err("Carried inventory is full! Make space before purchasing.".to_string());
        }

        self.player_credits -= price;
        let msg = format!("PURCHASED: {} for 💰 {} Credits!", iname, price);
        self.log_message(&msg);
        Ok(msg)
    }

    /// Sell an item from player's inventory to Trader Hank
    pub fn sell_to_trader(&mut self, item_index: usize, from_backpack: bool) -> Result<String, String> {
        let item = if from_backpack {
            if let Some(bp) = self.equipment.get_backpack_grid_mut() {
                bp.remove_at(item_index).ok_or_else(|| "Invalid backpack slot".to_string())?
            } else {
                return Err("No backpack equipped".to_string());
            }
        } else {
            self.equipment.pockets.remove_at(item_index)
                .ok_or_else(|| "Invalid pocket slot".to_string())?
        };

        match self.trader.buy_from_player(&item) {
            Ok(credits_earned) => {
                self.player_credits += credits_earned;
                let msg = format!("SOLD: {} to Hank for 💰 {} Credits!", item.name, credits_earned);
                self.log_message(&msg);
                Ok(msg)
            }
            Err(err) => {
                let _ = self.equipment.auto_insert_carried(item);
                Err(err.to_string())
            }
        }
    }

    /// Store an item from player's inventory into vehicle trunk
    pub fn store_item_in_vehicle_trunk(&mut self, vehicle_id: u32, item_idx: usize, from_backpack: bool) -> Result<String, String> {
        let p_pos = self.player_pos;
        let veh = self.vehicles.iter_mut().find(|v| v.id == vehicle_id)
            .ok_or_else(|| "Vehicle not found".to_string())?;

        if !veh.can_access_trunk(&p_pos) {
            return Err("Too far away from vehicle trunk!".to_string());
        }

        let item = if from_backpack {
            if let Some(bp) = self.equipment.get_backpack_grid_mut() {
                bp.remove_at(item_idx).ok_or_else(|| "Invalid backpack slot".to_string())?
            } else {
                return Err("No backpack equipped".to_string());
            }
        } else {
            self.equipment.pockets.remove_at(item_idx)
                .ok_or_else(|| "Invalid pocket slot".to_string())?
        };

        let iname = item.name.clone();
        let vname = veh.name.clone();
        match veh.auto_insert_trunk(item) {
            Ok(()) => {
                let msg = format!("Stored {} into {} trunk.", iname, vname);
                self.log_message(&msg);
                Ok(msg)
            }
            Err(returned_item) => {
                let _ = self.equipment.auto_insert_carried(returned_item);
                Err("Vehicle trunk is full or exceeds weight limit!".to_string())
            }
        }
    }

    /// Retrieve an item from vehicle trunk into player inventory
    pub fn take_item_from_vehicle_trunk(&mut self, vehicle_id: u32, trunk_idx: usize) -> Result<String, String> {
        let p_pos = self.player_pos;
        let veh = self.vehicles.iter_mut().find(|v| v.id == vehicle_id)
            .ok_or_else(|| "Vehicle not found".to_string())?;

        if !veh.can_access_trunk(&p_pos) {
            return Err("Too far away from vehicle trunk!".to_string());
        }

        let item = veh.remove_trunk_item(trunk_idx)
            .ok_or_else(|| "Invalid trunk slot".to_string())?;

        let iname = item.name.clone();
        let vname = veh.name.clone();
        if self.equipment.auto_insert_carried(item.clone()).is_ok() {
            let msg = format!("Took {} from {} trunk.", iname, vname);
            self.log_message(&msg);
            Ok(msg)
        } else {
            // Re-insert into vehicle trunk
            let veh = self.vehicles.iter_mut().find(|v| v.id == vehicle_id).unwrap();
            let _ = veh.auto_insert_trunk(item);
            Err("Player inventory is full! Cannot take item.".to_string())
        }
    }

    /// Take all items from vehicle trunk into player inventory
    pub fn take_all_from_vehicle_trunk(&mut self, vehicle_id: u32) -> Result<usize, String> {
        let p_pos = self.player_pos;
        let veh = self.vehicles.iter_mut().find(|v| v.id == vehicle_id)
            .ok_or_else(|| "Vehicle not found".to_string())?;

        if !veh.can_access_trunk(&p_pos) {
            return Err("Too far away from vehicle trunk!".to_string());
        }

        let mut taken = 0;
        let mut unplaced = Vec::new();
        while let Some(item) = veh.remove_trunk_item(0) {
            if self.equipment.auto_insert_carried(item.clone()).is_ok() {
                taken += 1;
            } else {
                unplaced.push(item);
                break;
            }
        }

        for item in unplaced {
            let veh = self.vehicles.iter_mut().find(|v| v.id == vehicle_id).unwrap();
            let _ = veh.auto_insert_trunk(item);
        }

        if taken > 0 {
            self.log_message(format!("Took all {} items from vehicle trunk.", taken));
        }
        Ok(taken)
    }

    /// Add wood fuel to nearby campfire (+60s burn time per plank)
    pub fn add_fuel_to_campfire(&mut self) -> Result<String, String> {
        if !self.campfire.is_in_range(&self.player_pos, 4.0) {
            return Err("Too far away from campfire!".to_string());
        }

        let has_wood = if self.equipment.pockets.remove_amount("wood_plank", 1) {
            true
        } else if let Some(bp) = self.equipment.get_backpack_grid_mut() {
            bp.remove_amount("wood_plank", 1)
        } else {
            false
        };

        if !has_wood {
            let has_log = if self.equipment.pockets.remove_amount("wood_log", 1) {
                true
            } else if let Some(bp) = self.equipment.get_backpack_grid_mut() {
                bp.remove_amount("wood_log", 1)
            } else {
                false
            };

            if !has_log {
                return Err("No wood planks or logs in inventory!".to_string());
            }
        }

        self.campfire.add_wood_fuel(1);
        let msg = format!("Added wood fuel to campfire! (Fuel remaining: {:.0}s)", self.campfire.fuel_remaining_s);
        self.log_message(&msg);
        Ok(msg)
    }

    /// Start cooking a recipe at the campfire
    pub fn start_campfire_cooking(&mut self, recipe_id: &str) -> Result<String, String> {
        if !self.campfire.is_in_range(&self.player_pos, 4.0) {
            return Err("Too far away from campfire!".to_string());
        }

        let recipes = get_campfire_cooking_recipes();
        let recipe = recipes.iter().find(|r| r.id == recipe_id)
            .ok_or_else(|| "Unknown cooking recipe".to_string())?
            .clone();

        if !self.campfire.is_lit {
            return Err("Campfire is extinguished! Add wood fuel first.".to_string());
        }

        // Check ingredients
        let available_count = self.equipment.pockets.count_item(&recipe.input_item_id)
            + self.equipment.get_backpack_grid().map(|bp| bp.count_item(&recipe.input_item_id)).unwrap_or(0);

        if available_count < recipe.input_count {
            return Err(format!("Missing ingredients: {} x{}", recipe.input_item_id, recipe.input_count));
        }

        // Deduct ingredients
        let mut needed = recipe.input_count;
        let in_pockets = self.equipment.pockets.count_item(&recipe.input_item_id);
        let deduct_pockets = needed.min(in_pockets);
        if deduct_pockets > 0 {
            self.equipment.pockets.remove_amount(&recipe.input_item_id, deduct_pockets);
            needed -= deduct_pockets;
        }
        if needed > 0 {
            if let Some(bp) = self.equipment.get_backpack_grid_mut() {
                bp.remove_amount(&recipe.input_item_id, needed);
            }
        }

        self.campfire.start_cooking(&recipe).map_err(|e| e.to_string())?;
        let msg = format!("Started cooking {}... (Takes {:.1}s)", recipe.name, recipe.cook_time_s);
        self.log_message(&msg);
        Ok(msg)
    }

    /// Freshwater river fishing simulation
    pub fn player_fish(&mut self) -> Result<String, String> {
        if !self.world_map.is_near_water(&self.player_pos) {
            return Err("You must be standing at the river shore to fish!".to_string());
        }

        if !self.equipment.has_item("fishing_rod") {
            return Err("You need a Fishing Rod equipped or in inventory to catch fish!".to_string());
        }

        if self.survival.stamina < 10.0 {
            return Err("Too exhausted to cast the line. Rest a moment.".to_string());
        }

        self.survival.stamina -= 10.0;
        let mut rng = rand::thread_rng();
        let caught = rng.gen_bool(0.70);

        if caught {
            let fish = create_stock_item("raw_fish").unwrap();
            let name = fish.name.clone();
            let _ = self.equipment.auto_insert_carried(fish);
            if let Some(new_lvl) = self.progression.add_xp(35) {
                self.log_message(format!("★ LEVEL UP! You reached Level {}! (+1 Skill Point)", new_lvl));
            }
            let msg = format!("CAUGHT A FISH! Reeled in a {} from the river!", name);
            self.log_message(&msg);
            Ok(msg)
        } else {
            let msg = "No bites on the line this time. Recast and try again!".to_string();
            self.log_message(&msg);
            Ok(msg)
        }
    }

    /// Vehicle engine and chassis maintenance
    pub fn repair_vehicle_engine(&mut self, vehicle_id: u32, use_kit: bool) -> Result<String, String> {
        let p_pos = self.player_pos;
        let veh = self.vehicles.iter_mut().find(|v| v.id == vehicle_id)
            .ok_or_else(|| "Vehicle not found".to_string())?;

        if !veh.can_access_trunk(&p_pos) {
            return Err("Too far away from vehicle engine hood!".to_string());
        }

        if use_kit {
            if !self.equipment.has_item("vehicle_repair_kit") {
                return Err("Missing Vehicle Repair Kit!".to_string());
            }
            let _ = self.equipment.find_and_remove_item("vehicle_repair_kit");
            veh.repair_engine(50.0);
            let msg = format!("Repaired {} engine by +50% with Vehicle Repair Kit (Now: {:.0}%).", veh.name, veh.engine_health);
            self.log_message(&msg);
            Ok(msg)
        } else {
            if !self.equipment.has_item("scrap_metal") {
                return Err("Requires 2x Scrap Metal to perform engine repair!".to_string());
            }
            let _ = self.equipment.find_and_remove_item("scrap_metal");
            veh.repair_engine(18.0);
            let msg = format!("Patched {} engine by +18% with Scrap Metal (Now: {:.0}%).", veh.name, veh.engine_health);
            self.log_message(&msg);
            Ok(msg)
        }
    }

    /// Replace vehicle battery
    pub fn replace_vehicle_battery(&mut self, vehicle_id: u32) -> Result<String, String> {
        let p_pos = self.player_pos;
        let veh = self.vehicles.iter_mut().find(|v| v.id == vehicle_id)
            .ok_or_else(|| "Vehicle not found".to_string())?;

        if !veh.can_access_trunk(&p_pos) {
            return Err("Too far away from vehicle battery compartment!".to_string());
        }

        if !self.equipment.has_item("car_battery") {
            return Err("Missing 12V Heavy-Duty Vehicle Battery!".to_string());
        }

        let _ = self.equipment.find_and_remove_item("car_battery");
        veh.replace_battery();
        let msg = format!("Installed fresh battery into {}. Electrical systems at 100%!", veh.name);
        self.log_message(&msg);
        Ok(msg)
    }

    /// Refuel vehicle tank with Jerrycan
    pub fn refuel_vehicle(&mut self, vehicle_id: u32) -> Result<String, String> {
        let p_pos = self.player_pos;
        let veh = self.vehicles.iter_mut().find(|v| v.id == vehicle_id)
            .ok_or_else(|| "Vehicle not found".to_string())?;

        if !veh.can_access_trunk(&p_pos) {
            return Err("Too far away from vehicle fuel port!".to_string());
        }

        if !self.equipment.has_item("gas_can") {
            return Err("Missing Jerrycan of fuel!".to_string());
        }

        let _ = self.equipment.find_and_remove_item("gas_can");
        let added = veh.refuel(20.0);
        let msg = format!("Poured {:.1}L fuel into {} (Tank: {:.1}/{:.1}L).", added, veh.name, veh.fuel_liters, veh.max_fuel_liters);
        self.log_message(&msg);
        Ok(msg)
    }

    /// Enter subterranean bunker facility through open keycard blast door
    pub fn enter_bunker_facility(&mut self, door_id: u32) -> Result<String, String> {
        let door = self.bunker_doors.iter().find(|d| d.id == door_id)
            .ok_or_else(|| "Bunker door not found".to_string())?;

        if !door.can_interact(&self.player_pos) {
            return Err("Too far from bunker entrance bulkhead!".to_string());
        }

        if !door.is_unlocked || !door.is_open {
            return Err("Blast door is sealed shut! Unlock it with the required keycard first.".to_string());
        }

        let facility = match door_id {
            1 => BunkerFacility::lab_alpha(),
            2 => BunkerFacility::fob_bravo(),
            3 => BunkerFacility::black_site_omega(),
            _ => return Err("Unknown facility ID".to_string()),
        };

        let msg = format!("DESCENDED INTO {}!", facility.name);
        self.log_message(&msg);
        self.player_pos = facility.ladder_exit_pos;
        self.active_bunker = Some(facility);
        Ok(msg)
    }

    /// Surface from subterranean bunker back to the valley overland map
    pub fn exit_bunker_facility(&mut self) -> Result<String, String> {
        let facility = self.active_bunker.as_ref()
            .ok_or_else(|| "Not currently inside an underground bunker!".to_string())?;

        if self.player_pos.distance_2d(&facility.ladder_exit_pos) > 4.5 {
            return Err("You must be near the bulkhead exit ladder to surface!".to_string());
        }

        let return_pos = facility.surface_return_pos;
        let fac_name = facility.name.clone();
        self.player_pos = return_pos;
        self.active_bunker = None;
        let msg = format!("SURFACED TO VALLEY from {}.", fac_name);
        self.log_message(&msg);
        Ok(msg)
    }

    /// Access and read lore logs from an underground bunker research terminal
    pub fn read_lore_terminal(&mut self, terminal_id: u32) -> Result<String, String> {
        let bunker = self.active_bunker.as_mut()
            .ok_or_else(|| "Not in an underground facility".to_string())?;

        let term = bunker.lore_terminals.iter_mut().find(|t| t.id == terminal_id)
            .ok_or_else(|| "Terminal not found".to_string())?;

        term.is_read = true;
        let display = format!("[TERMINAL ARCHIVE: {}] (By: {}) - {}", term.title, term.author, term.body);
        self.log_message(&display);
        Ok(display)
    }
}
