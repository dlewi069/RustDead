use crate::combat::{create_weapon_preset, WeaponStats};
use crate::items::{Item, ItemType, Rarity};
use crate::math::Position3D;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BanditState {
    Patrol,
    Alert,
    TakeCover,
    EngagePlayer,
    Reloading,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanditBark {
    pub text: String,
    pub timer: f32,
    pub is_combat: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bandit {
    pub id: u32,
    pub name: String,
    pub position: Position3D,
    pub patrol_origin: Position3D,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub weapon: WeaponStats,
    pub state: BanditState,
    pub state_timer: f32,
    pub shoot_cooldown: f32,
    pub shoot_timer: f32,
    pub detection_radius: f32,
    pub in_cover: bool,
    pub shout: Option<BanditBark>,
    pub is_sniper: bool,
    pub laser_aiming_timer: f32,
}

#[derive(Debug, Clone)]
pub struct BanditShot {
    pub bandit_id: u32,
    pub damage: f32,
    pub origin: Position3D,
    pub target: Position3D,
    pub hit_player: bool,
}

impl Bandit {
    pub fn new(id: u32, name: impl Into<String>, position: Position3D, weapon_id: &str) -> Self {
        let weapon = create_weapon_preset(weapon_id).unwrap_or_else(|| {
            create_weapon_preset("glock17").expect("glock17 preset must exist")
        });

        Self {
            id,
            name: name.into(),
            patrol_origin: position,
            position,
            yaw: 0.0,
            health: 100.0,
            max_health: 100.0,
            weapon,
            state: BanditState::Patrol,
            state_timer: 3.0,
            shoot_cooldown: 1.2,
            shoot_timer: 0.5,
            detection_radius: 45.0,
            in_cover: false,
            shout: None,
            is_sniper: false,
            laser_aiming_timer: 0.0,
        }
    }

    pub fn new_sniper(id: u32, name: impl Into<String>, position: Position3D, weapon_id: &str) -> Self {
        let weapon = create_weapon_preset(weapon_id)
            .or_else(|| create_weapon_preset("hunting_rifle"))
            .or_else(|| create_weapon_preset("m4a1"))
            .expect("default sniper or rifle weapon preset must exist");

        Self {
            id,
            name: name.into(),
            patrol_origin: position,
            position,
            yaw: 0.0,
            health: 120.0,
            max_health: 120.0,
            weapon,
            state: BanditState::Patrol,
            state_timer: 2.0,
            shoot_cooldown: 2.5,
            shoot_timer: 1.0,
            detection_radius: 75.0,
            in_cover: false,
            shout: None,
            is_sniper: true,
            laser_aiming_timer: 0.0,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.health > 0.0 && self.state != BanditState::Dead
    }

    pub fn take_damage(&mut self, damage: f32) -> bool {
        if !self.is_alive() {
            return false;
        }

        // Fortified cover reduces incoming damage by 50%
        let effective_dmg = if self.in_cover { damage * 0.5 } else { damage };
        self.health = (self.health - effective_dmg).max(0.0);

        if self.health <= 0.0 {
            self.state = BanditState::Dead;
            self.in_cover = false;
            self.shout = Some(BanditBark {
                text: "Argh... man down!".into(),
                timer: 2.0,
                is_combat: false,
            });
            true // killed
        } else {
            // Bark combat reaction
            if self.health < 45.0 && self.state != BanditState::TakeCover {
                self.state = BanditState::TakeCover;
                self.state_timer = 3.5;
                self.in_cover = true;
                self.shout = Some(BanditBark {
                    text: "I'm hit! Taking cover!".into(),
                    timer: 2.5,
                    is_combat: true,
                });
            } else {
                self.state = BanditState::EngagePlayer;
                self.shout = Some(BanditBark {
                    text: "Returning fire!".into(),
                    timer: 2.0,
                    is_combat: true,
                });
            }
            false
        }
    }

    /// Updates tactical AI and returns a shot event if the bandit fires their weapon at the player
    pub fn update(&mut self, player_pos: &Position3D, dt: f32) -> Option<BanditShot> {
        if !self.is_alive() {
            return None;
        }

        // Tick active voice shout
        if let Some(bark) = &mut self.shout {
            bark.timer -= dt;
            if bark.timer <= 0.0 {
                self.shout = None;
            }
        }

        self.shoot_timer = (self.shoot_timer - dt).max(0.0);
        let dist = self.position.distance_to(player_pos);

        // Turn towards target
        let dx = player_pos.x - self.position.x;
        let dz = player_pos.z - self.position.z;
        self.yaw = dx.atan2(dz).to_degrees();

        let mut shot_event: Option<BanditShot> = None;

        match self.state {
            BanditState::Patrol => {
                self.in_cover = false;
                self.laser_aiming_timer = 0.0;
                self.state_timer -= dt;
                if dist <= self.detection_radius {
                    self.state = BanditState::Alert;
                    self.state_timer = if self.is_sniper { 0.5 } else { 1.0 };
                    self.shout = Some(BanditBark {
                        text: if self.is_sniper { "Target acquired in scope!".into() } else { "CONTACT! Survivor spotted!".into() },
                        timer: 2.5,
                        is_combat: true,
                    });
                } else if self.state_timer <= 0.0 {
                    if !self.is_sniper {
                        // Wander around patrol origin
                        let mut rng = rand::thread_rng();
                        self.state_timer = rng.gen_range(4.0..8.0);
                        let ox = rng.gen_range(-8.0..8.0);
                        let oz = rng.gen_range(-8.0..8.0);
                        self.position.x = self.patrol_origin.x + ox;
                        self.position.z = self.patrol_origin.z + oz;
                    } else {
                        self.position = self.patrol_origin;
                        self.state_timer = 4.0;
                    }
                }
            }
            BanditState::Alert => {
                self.state_timer -= dt;
                if self.state_timer <= 0.0 {
                    self.state = BanditState::EngagePlayer;
                    self.shout = Some(BanditBark {
                        text: if self.is_sniper { "Lining up headshot!".into() } else { "Drop your weapons! Fire!".into() },
                        timer: 2.0,
                        is_combat: true,
                    });
                }
            }
            BanditState::EngagePlayer => {
                self.in_cover = false;
                if dist > self.detection_radius * 1.5 {
                    // Player broke line of sight and escaped
                    self.state = BanditState::Patrol;
                    self.laser_aiming_timer = 0.0;
                    self.shout = Some(BanditBark {
                        text: "Lost visual! Stay sharp.".into(),
                        timer: 2.0,
                        is_combat: false,
                    });
                } else if self.weapon.current_ammo == 0 {
                    self.state = BanditState::Reloading;
                    self.state_timer = 2.2;
                    self.laser_aiming_timer = 0.0;
                    self.shout = Some(BanditBark {
                        text: "I'm reloading! Cover me!".into(),
                        timer: 2.0,
                        is_combat: true,
                    });
                } else if self.is_sniper {
                    self.position = self.patrol_origin;
                    self.laser_aiming_timer += dt;
                    if self.laser_aiming_timer >= 1.2 && self.shoot_timer <= 0.0 {
                        if self.weapon.fire() {
                            let mut rng = rand::thread_rng();
                            let hit_chance = (0.92 - (dist / 140.0)).clamp(0.45, 0.95);
                            let did_hit = rng.gen_bool(hit_chance as f64);

                            shot_event = Some(BanditShot {
                                bandit_id: self.id,
                                damage: self.weapon.base_damage * 1.5,
                                origin: self.position,
                                target: *player_pos,
                                hit_player: did_hit,
                            });

                            self.shoot_timer = self.shoot_cooldown;
                            self.laser_aiming_timer = 0.0;
                        }
                    }
                } else if self.shoot_timer <= 0.0 {
                    // Fire weapon at player!
                    if self.weapon.fire() {
                        let mut rng = rand::thread_rng();
                        // Accuracy rolls based on distance (closer = more accurate)
                        let hit_chance = (0.75 - (dist / 100.0)).clamp(0.25, 0.85);
                        let did_hit = rng.gen_bool(hit_chance as f64);

                        shot_event = Some(BanditShot {
                            bandit_id: self.id,
                            damage: self.weapon.base_damage,
                            origin: self.position,
                            target: *player_pos,
                            hit_player: did_hit,
                        });

                        self.shoot_timer = self.shoot_cooldown;
                    }
                } else {
                    // Tactical movement: Strafe or reposition
                    if dist > 20.0 {
                        // Move closer
                        let speed = 2.4;
                        let len = (dx * dx + dz * dz).sqrt().max(0.001);
                        self.position.x += (dx / len) * speed * dt;
                        self.position.z += (dz / len) * speed * dt;
                    } else if dist < 8.0 {
                        // Back up
                        let speed = 2.0;
                        let len = (dx * dx + dz * dz).sqrt().max(0.001);
                        self.position.x -= (dx / len) * speed * dt;
                        self.position.z -= (dz / len) * speed * dt;
                    }
                }
            }
            BanditState::TakeCover => {
                self.laser_aiming_timer = 0.0;
                self.in_cover = true;
                self.state_timer -= dt;
                if !self.is_sniper {
                    // Move towards defensive origin
                    let tox = self.patrol_origin.x - self.position.x;
                    let toz = self.patrol_origin.z - self.position.z;
                    let to_len = (tox * tox + toz * toz).sqrt();
                    if to_len > 1.5 {
                        self.position.x += (tox / to_len) * 3.2 * dt;
                        self.position.z += (toz / to_len) * 3.2 * dt;
                    }
                } else {
                    self.position = self.patrol_origin;
                }

                if self.state_timer <= 0.0 {
                    self.in_cover = false;
                    self.state = BanditState::EngagePlayer;
                    self.shout = Some(BanditBark {
                        text: "Moving in!".into(),
                        timer: 1.5,
                        is_combat: true,
                    });
                }
            }
            BanditState::Reloading => {
                self.state_timer -= dt;
                if self.state_timer <= 0.0 {
                    let cap = self.weapon.mag_capacity;
                    self.weapon.reload(cap);
                    self.state = BanditState::EngagePlayer;
                }
            }
            BanditState::Dead => {}
        }

        shot_event
    }

    /// Generate loot dropped on bandit death
    pub fn generate_drops(&self) -> Vec<Item> {
        let mut drops = Vec::new();
        // Drop their weapon
        let w_item = Item::new(
            &self.weapon.weapon_id,
            &self.weapon.name,
            ItemType::WeaponPrimary,
            Rarity::Rare,
            3.0,
            2,
            4,
            1,
        )
        .with_durability(self.weapon.current_durability);
        drops.push(w_item);

        // Drop matching ammo
        let ammo = Item::new(
            "ammo_762",
            "7.62x39mm Ammo (30 rounds)",
            ItemType::Ammo,
            Rarity::Uncommon,
            0.6,
            1,
            1,
            10,
        )
        .with_stack(30);
        drops.push(ammo);

        // Bandit cash
        let cash = Item::new("cash", "Survivor Credits", ItemType::Material, Rarity::Common, 0.1, 1, 1, 1000)
            .with_stack(rand::thread_rng().gen_range(25..120));
        drops.push(cash);

        // 50% chance of Armor Repair Kit
        if rand::thread_rng().gen_bool(0.5) {
            let kit = Item::new(
                "armor_repair_kit",
                "Armor Repair Kit",
                ItemType::Material,
                Rarity::Rare,
                2.0,
                2,
                2,
                3,
            );
            drops.push(kit);
        }

        // 35% chance of Level 1 Research Keycard
        if rand::thread_rng().gen_bool(0.35) {
            let keycard = Item::new(
                "keycard_lvl1",
                "Keycard [Level 1 - Research Facility]",
                ItemType::Keycard,
                Rarity::Rare,
                0.1,
                1,
                1,
                1,
            );
            drops.push(keycard);
        }

        drops
    }
}
