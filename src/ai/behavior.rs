use super::perception::{NoiseEvent, ZombiePerception};
use super::zombie::{AiState, Zombie};
use crate::math::Position3D;
use rand::Rng;

pub struct AiDirector {
    pub perception: ZombiePerception,
}

impl Default for AiDirector {
    fn default() -> Self {
        Self {
            perception: ZombiePerception::default(),
        }
    }
}

impl AiDirector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates zombie decision-making, path movement, and attacks against the player
    pub fn update_zombie(
        &self,
        zombie: &mut Zombie,
        player_pos: &Position3D,
        dt: f32,
        is_daytime: bool,
        flashlight_on: bool,
        recent_noises: &[NoiseEvent],
    ) -> Option<f32> {
        if !zombie.is_alive() {
            return None;
        }

        zombie.attack_timer = (zombie.attack_timer - dt).max(0.0);
        let mut damage_dealt: Option<f32> = None;

        // 1. Handle Stagger state
        if zombie.state == AiState::Staggered {
            zombie.stagger_timer -= dt;
            if zombie.stagger_timer <= 0.0 {
                zombie.state = AiState::Chase;
            }
            return None;
        }

        // 2. Sensory Perception Checks
        let sees_player = self.perception.can_see_player(
            &zombie.position,
            zombie.yaw,
            player_pos,
            is_daytime,
            flashlight_on,
        );

        if sees_player {
            zombie.state = AiState::Chase;
            zombie.target_position = Some(*player_pos);
        } else {
            // Check for nearby noises (gunshots, running)
            for noise in recent_noises {
                if self.perception.can_hear_noise(&zombie.position, noise) {
                    zombie.state = AiState::Investigate;
                    zombie.target_position = Some(noise.position);
                    break;
                }
            }
        }

        // 3. State Machine Execution
        match zombie.state {
            AiState::Idle => {
                zombie.state_timer -= dt;
                if zombie.state_timer <= 0.0 {
                    let mut rng = rand::thread_rng();
                    zombie.state = AiState::Wander;
                    zombie.state_timer = rng.gen_range(4.0..8.0);
                    // Pick wander waypoint within 15 meters
                    let ox = rng.gen_range(-12.0..12.0);
                    let oz = rng.gen_range(-12.0..12.0);
                    zombie.target_position = Some(Position3D::new(
                        zombie.position.x + ox,
                        zombie.position.y,
                        zombie.position.z + oz,
                    ));
                }
            }
            AiState::Wander => {
                zombie.state_timer -= dt;
                if let Some(target) = zombie.target_position {
                    let dist = zombie.position.distance_2d(&target);
                    if dist < 1.0 || zombie.state_timer <= 0.0 {
                        zombie.state = AiState::Idle;
                        zombie.state_timer = rand::thread_rng().gen_range(2.0..5.0);
                        zombie.target_position = None;
                    } else {
                        self.move_towards(zombie, &target, zombie.speed_walk, dt);
                    }
                }
            }
            AiState::Investigate => {
                if let Some(target) = zombie.target_position {
                    let dist = zombie.position.distance_2d(&target);
                    if dist < 1.5 {
                        zombie.state = AiState::Idle;
                        zombie.state_timer = 3.0;
                        zombie.target_position = None;
                    } else {
                        self.move_towards(zombie, &target, zombie.speed_walk * 1.5, dt);
                    }
                }
            }
            AiState::Chase => {
                let dist = zombie.position.distance_to(player_pos);
                if dist <= zombie.attack_range {
                    zombie.state = AiState::Attack;
                } else {
                    self.move_towards(zombie, player_pos, zombie.speed_run, dt);
                }
            }
            AiState::Attack => {
                let dist = zombie.position.distance_to(player_pos);
                if dist > zombie.attack_range * 1.25 {
                    zombie.state = AiState::Chase;
                } else if zombie.attack_timer <= 0.0 {
                    // Execute attack!
                    damage_dealt = Some(zombie.attack_damage);
                    zombie.attack_timer = zombie.attack_cooldown_s;
                }
            }
            AiState::Staggered | AiState::Dead => {}
        }

        damage_dealt
    }

    fn move_towards(&self, zombie: &mut Zombie, target: &Position3D, speed: f32, dt: f32) {
        let dx = target.x - zombie.position.x;
        let dz = target.z - zombie.position.z;
        let len = (dx * dx + dz * dz).sqrt();

        if len > 0.001 {
            let ndx = dx / len;
            let ndz = dz / len;

            zombie.position.x += ndx * speed * dt;
            zombie.position.z += ndz * speed * dt;
            zombie.yaw = dx.atan2(dz).to_degrees();
        }
    }
}
