use super::perception::NoiseEvent;
use super::zombie::{AiState, Zombie, ZombieType};
use crate::math::Position3D;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HordeGroup {
    pub id: u32,
    pub member_ids: Vec<u32>,
    pub patrol_waypoints: Vec<Position3D>,
    pub current_waypoint_idx: usize,
    pub is_alerted: bool,
    pub target_point: Option<Position3D>,
}

impl HordeGroup {
    pub fn new(id: u32, member_ids: Vec<u32>, waypoints: Vec<Position3D>) -> Self {
        Self {
            id,
            member_ids,
            patrol_waypoints: waypoints,
            current_waypoint_idx: 0,
            is_alerted: false,
            target_point: None,
        }
    }

    pub fn current_waypoint(&self) -> Option<Position3D> {
        if self.patrol_waypoints.is_empty() {
            None
        } else {
            Some(self.patrol_waypoints[self.current_waypoint_idx % self.patrol_waypoints.len()])
        }
    }

    pub fn advance_waypoint(&mut self) {
        if !self.patrol_waypoints.is_empty() {
            self.current_waypoint_idx = (self.current_waypoint_idx + 1) % self.patrol_waypoints.len();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HordeSpawner {
    pub horde_timer_s: f32,
    pub next_horde_in_s: f32,
    pub next_entity_id: u32,
    pub next_horde_id: u32,
    pub active_hordes: Vec<HordeGroup>,
    pub total_hordes_spawned: u32,
}

impl Default for HordeSpawner {
    fn default() -> Self {
        Self {
            horde_timer_s: 0.0,
            next_horde_in_s: 240.0, // Day cycle base
            next_entity_id: 1000,
            next_horde_id: 1,
            active_hordes: Vec::new(),
            total_hordes_spawned: 0,
        }
    }
}

impl HordeSpawner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Periodic tick with daytime and acoustic perception awareness
    pub fn tick(
        &mut self,
        dt: f32,
        player_pos: &Position3D,
        is_daytime: bool,
        recent_noises: &[NoiseEvent],
        zombies: &mut Vec<Zombie>,
    ) -> Option<String> {
        self.horde_timer_s += dt;

        // Night time spawns hordes much faster (90s vs 240s)
        let interval = if is_daytime { 240.0 } else { 90.0 };
        self.next_horde_in_s = interval;

        let mut spawned_msg = None;

        if self.horde_timer_s >= self.next_horde_in_s {
            self.horde_timer_s = 0.0;
            let pack_size = if is_daytime { 6 } else { 12 };
            self.spawn_horde(player_pos, pack_size, !is_daytime, zombies);
            let time_desc = if is_daytime { "Wandering pack" } else { "Night-stalker horde" };
            spawned_msg = Some(format!("ALERT: {} spotted roaming nearby ({}+ infected)!", time_desc, pack_size));
        }

        // React to sound events
        self.handle_noises(recent_noises, zombies);

        // Update roaming horde waypoints
        self.update_horde_patrols(zombies);

        spawned_msg
    }

    /// Spawns a horde group surrounding the target center
    pub fn spawn_horde(
        &mut self,
        center: &Position3D,
        count: usize,
        is_night: bool,
        zombies: &mut Vec<Zombie>,
    ) -> u32 {
        let mut rng = rand::thread_rng();
        let mut member_ids = Vec::new();
        let horde_id = self.next_horde_id;
        self.next_horde_id += 1;
        self.total_hordes_spawned += 1;

        // Base angle for horde approach vector
        let approach_angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let base_dist = rng.gen_range(35.0..55.0);
        let horde_origin_x = center.x + approach_angle.cos() * base_dist;
        let horde_origin_z = center.z + approach_angle.sin() * base_dist;

        for _ in 0..count {
            let offset_x = rng.gen_range(-6.0..6.0);
            let offset_z = rng.gen_range(-6.0..6.0);
            let spawn_x = horde_origin_x + offset_x;
            let spawn_z = horde_origin_z + offset_z;

            let z_type = if is_night {
                // More dangerous types during nighttime
                match rng.gen_range(0..100) {
                    0..=35 => ZombieType::Walker,
                    36..=65 => ZombieType::Runner,
                    66..=80 => ZombieType::Hound,
                    81..=90 => ZombieType::Screamer,
                    91..=96 => ZombieType::Bloater,
                    _ => ZombieType::Juggernaut,
                }
            } else {
                match rng.gen_range(0..100) {
                    0..=65 => ZombieType::Walker,
                    66..=85 => ZombieType::Runner,
                    86..=94 => ZombieType::Bloater,
                    _ => ZombieType::Juggernaut,
                }
            };

            self.next_entity_id += 1;
            let mut z = Zombie::new(self.next_entity_id, z_type, Position3D::new(spawn_x, center.y, spawn_z));
            z.state = AiState::Investigate;
            z.target_position = Some(*center);
            member_ids.push(self.next_entity_id);
            zombies.push(z);
        }

        // Generate patrol waypoints around center
        let mut waypoints = Vec::new();
        for i in 0..4 {
            let rot = approach_angle + (i as f32) * (std::f32::consts::PI * 0.5);
            let dist = rng.gen_range(20.0..40.0);
            waypoints.push(Position3D::new(center.x + rot.cos() * dist, center.y, center.z + rot.sin() * dist));
        }

        let group = HordeGroup::new(horde_id, member_ids, waypoints);
        self.active_hordes.push(group);

        horde_id
    }

    /// Emergency Horde Surge event (e.g. from WorldEventManager or distress beacon)
    pub fn spawn_surge(&mut self, center: &Position3D, zombies: &mut Vec<Zombie>) -> usize {
        let mut rng = rand::thread_rng();
        let surge_count = 12;
        let mut member_ids = Vec::new();

        for i in 0..surge_count {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let distance = rng.gen_range(25.0..40.0);
            let sx = center.x + angle.cos() * distance;
            let sz = center.z + angle.sin() * distance;

            // Guaranteed Screamer and Juggernaut in surge
            let z_type = match i {
                0 => ZombieType::Screamer,
                1 => ZombieType::Juggernaut,
                2 | 3 | 4 => ZombieType::Runner,
                5 | 6 => ZombieType::Hound,
                7 => ZombieType::Bloater,
                _ => ZombieType::Walker,
            };

            self.next_entity_id += 1;
            let mut z = Zombie::new(self.next_entity_id, z_type, Position3D::new(sx, center.y, sz));
            z.state = AiState::Chase;
            z.target_position = Some(*center);
            member_ids.push(self.next_entity_id);
            zombies.push(z);
        }

        let group = HordeGroup::new(self.next_horde_id, member_ids, vec![*center]);
        self.next_horde_id += 1;
        self.total_hordes_spawned += 1;
        self.active_hordes.push(group);

        surge_count
    }

    /// Attracts hordes and nearby zombies to acoustic noise sources
    pub fn handle_noises(&mut self, noises: &[NoiseEvent], zombies: &mut [Zombie]) {
        for noise in noises {
            if noise.radius_m <= 0.0 {
                continue;
            }

            for horde in &mut self.active_hordes {
                // Check if any alive member is close enough to hear
                let mut heard = false;
                for &mid in &horde.member_ids {
                    if let Some(z) = zombies.iter().find(|z| z.id == mid && z.is_alive()) {
                        if z.position.distance_to(&noise.position) <= noise.radius_m {
                            heard = true;
                            break;
                        }
                    }
                }

                if heard {
                    horde.is_alerted = true;
                    horde.target_point = Some(noise.position);

                    // Redirect all members of this horde toward noise source
                    for &mid in &horde.member_ids {
                        if let Some(z) = zombies.iter_mut().find(|z| z.id == mid && z.is_alive()) {
                            z.state = if noise.intensity >= 0.8 {
                                AiState::Chase
                            } else {
                                AiState::Investigate
                            };
                            z.target_position = Some(noise.position);
                        }
                    }
                }
            }
        }
    }

    /// Advance roaming waypoints for wandering packs
    pub fn update_horde_patrols(&mut self, zombies: &mut [Zombie]) {
        self.active_hordes.retain(|horde| {
            // Check if any member is still alive
            horde.member_ids.iter().any(|&mid| zombies.iter().any(|z| z.id == mid && z.is_alive()))
        });

        for horde in &mut self.active_hordes {
            if horde.is_alerted {
                continue;
            }

            if let Some(wp) = horde.current_waypoint() {
                // Check if alive members have arrived near waypoint
                let mut arrived = false;
                for &mid in &horde.member_ids {
                    if let Some(z) = zombies.iter().find(|z| z.id == mid && z.is_alive()) {
                        if z.position.distance_to(&wp) < 5.0 {
                            arrived = true;
                            break;
                        }
                    }
                }

                if arrived {
                    horde.advance_waypoint();
                    if let Some(new_wp) = horde.current_waypoint() {
                        for &mid in &horde.member_ids {
                            if let Some(z) = zombies.iter_mut().find(|z| z.id == mid && z.is_alive()) {
                                if z.state == AiState::Investigate || z.state == AiState::Wander {
                                    z.target_position = Some(new_wp);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
