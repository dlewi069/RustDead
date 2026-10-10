use crate::looting::{LootContainer, LootTier};
use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldEventType {
    MilitaryAirdrop,
    HordeSurge,
    WanderingBandits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAirdrop {
    pub position: Position3D,
    pub parachute_altitude: f32, // drops down to 0.0
    pub has_landed: bool,
    pub smoke_duration_s: f32,
    pub container_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEventManager {
    pub active_airdrop: Option<ActiveAirdrop>,
    pub airdrop_timer_s: f32,
    pub horde_surge_timer_s: f32,
    pub next_container_id: u32,
}

impl Default for WorldEventManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldEventManager {
    pub fn new() -> Self {
        Self {
            active_airdrop: None,
            airdrop_timer_s: 180.0,      // First airdrop after 3 minutes
            horde_surge_timer_s: 300.0,  // Horde surge after 5 minutes
            next_container_id: 1000,
        }
    }

    /// Advance event timers and drop parachute crates
    pub fn tick(&mut self, dt: f32, containers: &mut Vec<LootContainer>) -> Option<WorldEventType> {
        self.airdrop_timer_s -= dt;
        self.horde_surge_timer_s -= dt;

        let mut triggered_event = None;

        // Check Airdrop trigger
        if self.airdrop_timer_s <= 0.0 && self.active_airdrop.is_none() {
            // Spawn airdrop at random coordinates within 100m of valley
            let x = rand::random::<f32>() * 120.0 - 60.0;
            let z = rand::random::<f32>() * 120.0 - 60.0;
            let drop_pos = Position3D::new(x, 0.0, z);

            let airdrop_id = self.next_container_id;
            self.next_container_id += 1;

            self.active_airdrop = Some(ActiveAirdrop {
                position: drop_pos,
                parachute_altitude: 75.0, // Falling from sky
                has_landed: false,
                smoke_duration_s: 180.0,
                container_id: airdrop_id,
            });

            self.airdrop_timer_s = 400.0; // Reset timer for next airdrop
            triggered_event = Some(WorldEventType::MilitaryAirdrop);
        }

        // Update active airdrop descent
        if let Some(drop) = &mut self.active_airdrop {
            if !drop.has_landed {
                drop.parachute_altitude -= dt * 10.0; // 10 m/s descent
                if drop.parachute_altitude <= 0.0 {
                    drop.parachute_altitude = 0.0;
                    drop.has_landed = true;

                    // Spawn physical loot container on ground with Military tier loot!
                    let mut crate_box = LootContainer::new(
                        drop.container_id,
                        "C-130 Military Supply Drop",
                        drop.position,
                        LootTier::Military,
                        6,
                        6,
                    );
                    crate_box.interaction_radius_m = 3.5;
                    containers.push(crate_box);
                }
            } else {
                drop.smoke_duration_s = (drop.smoke_duration_s - dt).max(0.0);
            }
        }

        // Check Horde surge trigger
        if self.horde_surge_timer_s <= 0.0 {
            self.horde_surge_timer_s = 600.0;
            triggered_event = Some(WorldEventType::HordeSurge);
        }

        triggered_event
    }
}
