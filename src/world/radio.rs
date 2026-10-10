use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioStation {
    pub id: String,
    pub name: String,
    pub frequency_mhz: f32,
    pub bandwidth_mhz: f32,
    pub broadcast_title: String,
    pub messages: Vec<String>,
    pub transmission_location: Option<Position3D>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioSystem {
    pub stations: Vec<RadioStation>,
    pub current_frequency_mhz: f32,
    pub is_powered: bool,
    pub volume: f32,
}

impl Default for RadioSystem {
    fn default() -> Self {
        let stations = vec![
            RadioStation {
                id: "coyote_call".into(),
                name: "Coyote Outpost Relay".into(),
                frequency_mhz: 91.5,
                bandwidth_mhz: 0.4,
                broadcast_title: "CoyoteCall — Safezone Coordinates & Merchant Trade".into(),
                messages: vec![
                    "This is Coyote Outpost on 91.5 FM. Camp Timber remains a secure safezone.".into(),
                    "Quartermaster Hank has restocked military ammo tins and medical splints.".into(),
                    "Hostile bandits spotted near the highway checkpoint. Keep your safeties off.".into(),
                ],
                transmission_location: Some(Position3D::new(50.0, 0.0, 50.0)),
            },
            RadioStation {
                id: "fire_in_the_city".into(),
                name: "Urban Disaster Evacuation".into(),
                frequency_mhz: 104.2,
                bandwidth_mhz: 0.4,
                broadcast_title: "FireInTheCity — Evacuation Alarm & Metro Hazard".into(),
                messages: vec![
                    "EMERGENCY BROADCAST: Downtown district is engulfed in structural fires.".into(),
                    "Biohazard infection levels have reached critical threshold. Runner hordes in streets.".into(),
                    "Avoid central transit hub. High radiated bloater presence confirmed.".into(),
                ],
                transmission_location: Some(Position3D::new(-120.0, 0.0, 80.0)),
            },
            RadioStation {
                id: "military_reinforcements".into(),
                name: "FOB Bravo Command Frequency".into(),
                frequency_mhz: 128.7,
                bandwidth_mhz: 0.5,
                broadcast_title: "MilitaryReinforcements — Tactical Airdrop Logistics".into(),
                messages: vec![
                    "Command to any surviving regional operatives on tactical frequency 128.7.".into(),
                    "C-130 Hercules cargo flights are operating on intermittent schedule.".into(),
                    "Authorize emergency supply drop at your GPS coordinates if credits available.".into(),
                ],
                transmission_location: Some(Position3D::new(200.0, 0.0, -150.0)),
            },
            RadioStation {
                id: "the_subway".into(),
                name: "Bunker Subterranean Quarantine".into(),
                frequency_mhz: 142.0,
                bandwidth_mhz: 0.4,
                broadcast_title: "TheSubway — Underground Radiation & Armory Keycards".into(),
                messages: vec![
                    "Subterranean Research Complex Alpha quarantine alert.".into(),
                    "Radiation leakage detected in sector lower levels. Anti-rad injectors recommended.".into(),
                    "Armory bulkhead requires Level 2 keycard clearance. Keycards located in command office.".into(),
                ],
                transmission_location: Some(Position3D::new(0.0, -15.0, 0.0)),
            },
            RadioStation {
                id: "force_from_afar".into(),
                name: "Distant Carrier SOS".into(),
                frequency_mhz: 160.0,
                bandwidth_mhz: 0.5,
                broadcast_title: "ForceFromAfar — Long-Range Naval Relay".into(),
                messages: vec![
                    "MAYDAY... MAYDAY... This is automated beacon on carrier frequency 160.0.".into(),
                    "Offshore support fleet standing by. Maintain survivor beacons across the mainland.".into(),
                    "Stay alive, survivor. Help is coming.".into(),
                ],
                transmission_location: Some(Position3D::new(-350.0, 0.0, -400.0)),
            },
        ];

        Self {
            stations,
            current_frequency_mhz: 91.5,
            is_powered: true,
            volume: 0.8,
        }
    }
}

impl RadioSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tune(&mut self, mhz: f32) {
        self.current_frequency_mhz = (mhz.clamp(80.0, 180.0) * 10.0).round() / 10.0;
    }

    /// Check if currently tuned to a station, returning (Station, Signal Strength [0.0..1.0])
    pub fn get_tuned_station(&self) -> Option<(&RadioStation, f32)> {
        if !self.is_powered {
            return None;
        }

        for station in &self.stations {
            let diff = (self.current_frequency_mhz - station.frequency_mhz).abs();
            if diff <= station.bandwidth_mhz {
                let strength = 1.0 - (diff / station.bandwidth_mhz);
                return Some((station, strength.clamp(0.0, 1.0)));
            }
        }
        None
    }

    /// Current broadcast readout with static noise simulation
    pub fn get_current_transmission_text(&self, tick: u64) -> String {
        if !self.is_powered {
            return "RADIO POWER: OFF".to_string();
        }

        if let Some((station, strength)) = self.get_tuned_station() {
            let msg_idx = ((tick / 240) as usize) % station.messages.len();
            let raw_msg = &station.messages[msg_idx];

            if strength >= 0.85 {
                format!("[{:.1} MHz | {}]: \"{}\"", station.frequency_mhz, station.name, raw_msg)
            } else {
                // Introduce static glitches based on signal strength
                let corrupted: String = raw_msg
                    .chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if (i + (tick as usize)) % 4 == 0 && strength < 0.6 {
                            '#'
                        } else {
                            c
                        }
                    })
                    .collect();
                format!(
                    "[{:.1} MHz | {} (Signal {:.0}%)]: \"{}\"",
                    station.frequency_mhz, station.name, strength * 100.0, corrupted
                )
            }
        } else {
            format!("[{:.1} MHz]: ~ ~ ~ STATIC HISS [NO SIGNAL] ~ ~ ~", self.current_frequency_mhz)
        }
    }
}
