use crate::engine::GameState;
use std::io::{self, Write};

pub struct TerminalRenderer;

impl TerminalRenderer {
    pub fn render_frame(state: &GameState) -> io::Result<()> {
        let mut stdout = io::stdout();

        // ANSI clear screen & cursor home
        print!("\x1B[2J\x1B[H");

        // 1. Top Header Banner & Environment (Matching SurrounDead UE5 top bar)
        let weather_str = format!("{:?}", state.environment.current_weather);
        let time_str = state.environment.formatted_time();
        let geiger_rad = state.survival.radiation;
        let heading_deg = (state.player_yaw as i32).rem_euclid(360);
        let cardinal = match heading_deg {
            23..=67 => "NE",
            68..=112 => "E",
            113..=157 => "SE",
            158..=202 => "S",
            203..=247 => "SW",
            248..=292 => "W",
            293..=337 => "NW",
            _ => "N",
        };

        println!("╔══════════════════════════════════════════════════════════════════════════════════════════════╗");
        println!(
            "║ SURROUNDEAD | LVL: {} (XP: {}/100) | {}   {:>4}° [{:<2}]   TIME: {} [{:<8}]   KILLS: {:<3} ║",
            state.progression.level,
            state.progression.current_xp,
            state.player_credits,
            heading_deg,
            cardinal,
            time_str,
            weather_str,
            state.kills_count
        );
        println!("╠══════════════════════════════════════════════════════════════════════════════════════════════╣");

        // 2. Radar & Tactical Map Display (31x15 grid around player)
        let radar_width = 41;
        let radar_height = 13;
        let half_w = radar_width / 2;
        let half_h = radar_height / 2;

        let mut radar_grid = vec![vec!['.'; radar_width]; radar_height];

        // Place points of interest / obstacles
        for obj in &state.structures {
            let dx = (obj.position.x - state.player_pos.x).round() as i32;
            let dz = (obj.position.z - state.player_pos.z).round() as i32;
            let gx = half_w as i32 + dx;
            let gy = half_h as i32 - dz;
            if gx >= 0 && gx < radar_width as i32 && gy >= 0 && gy < radar_height as i32 {
                radar_grid[gy as usize][gx as usize] = '#';
            }
        }

        // Place containers
        for c in &state.containers {
            let dx = (c.position.x - state.player_pos.x).round() as i32;
            let dz = (c.position.z - state.player_pos.z).round() as i32;
            let gx = half_w as i32 + dx;
            let gy = half_h as i32 - dz;
            if gx >= 0 && gx < radar_width as i32 && gy >= 0 && gy < radar_height as i32 {
                radar_grid[gy as usize][gx as usize] = 'C';
            }
        }

        // Place vehicles
        for v in &state.vehicles {
            let dx = (v.position.x - state.player_pos.x).round() as i32;
            let dz = (v.position.z - state.player_pos.z).round() as i32;
            let gx = half_w as i32 + dx;
            let gy = half_h as i32 - dz;
            if gx >= 0 && gx < radar_width as i32 && gy >= 0 && gy < radar_height as i32 {
                radar_grid[gy as usize][gx as usize] = 'V';
            }
        }

        // Place zombies
        for z in &state.zombies {
            if !z.is_alive() { continue; }
            let dx = (z.position.x - state.player_pos.x).round() as i32;
            let dz = (z.position.z - state.player_pos.z).round() as i32;
            let gx = half_w as i32 + dx;
            let gy = half_h as i32 - dz;
            if gx >= 0 && gx < radar_width as i32 && gy >= 0 && gy < radar_height as i32 {
                let symbol = match z.zombie_type {
                    crate::ai::ZombieType::Walker => 'Z',
                    crate::ai::ZombieType::Runner => 'R',
                    crate::ai::ZombieType::Bloater => 'B',
                    crate::ai::ZombieType::Juggernaut => 'J',
                    crate::ai::ZombieType::Screamer => 'S',
                    crate::ai::ZombieType::Spitter => 'P',
                    crate::ai::ZombieType::Hound => 'H',
                };
                radar_grid[gy as usize][gx as usize] = symbol;
            }
        }

        // Player marker in center with direction indicator
        let player_char = match (state.player_yaw as i32).rem_euclid(360) {
            45..=135 => '>',
            136..=225 => 'v',
            226..=315 => '<',
            _ => '^',
        };
        radar_grid[half_h][half_w] = player_char;

        // Print radar alongside vital bars
        let hp_bar = Self::make_bar(state.survival.health, state.survival.max_health);
        let sta_bar = Self::make_bar(state.survival.stamina, state.survival.max_stamina);
        let hun_bar = Self::make_bar(state.survival.hunger, 100.0);
        let thi_bar = Self::make_bar(state.survival.thirst, 100.0);
        let inf_bar = Self::make_bar(state.survival.infection, 100.0);

        let vehicle_status = if let Some(vid) = state.active_vehicle_id {
            if let Some(v) = state.vehicles.iter().find(|veh| veh.id == vid) {
                format!("DRIVING {} | {:.0} km/h | Fuel: {:.1}L", v.name, v.speed_kmh, v.fuel_liters)
            } else {
                "ON FOOT".into()
            }
        } else {
            "ON FOOT".into()
        };

        let weapon_info = if let Some(ref w) = state.active_weapon {
            if w.is_melee() {
                format!("{} [MELEE] (Durability: {:.0}%)", w.name, w.current_durability)
            } else {
                format!("{} [{}/{}] (Durability: {:.0}%)", w.name, w.current_ammo, w.mag_capacity, w.current_durability)
            }
        } else {
            "UNARMED".into()
        };

        for (row_idx, row) in radar_grid.iter().enumerate() {
            let row_str: String = row.iter().collect();
            let side_info = match row_idx {
                0 => format!("  HEALTH:    [{}] {:>3.0}%", hp_bar, state.survival.health),
                1 => format!("  STAMINA:   [{}] {:>3.0}%", sta_bar, state.survival.stamina),
                2 => format!("  HUNGER:    [{}] {:>3.0}%", hun_bar, state.survival.hunger),
                3 => format!("  THIRST:    [{}] {:>3.0}%", thi_bar, state.survival.thirst),
                4 => format!("  INFECTION: [{}] {:>3.0}%", inf_bar, state.survival.infection),
                5 => format!("  RADIATION: {:.1} rads | BLEEDING: {}", geiger_rad, if state.survival.bleeding { "YES! (BANDAGE NEEDED)" } else { "NO" }),
                6 => format!("  STATUS:    {}", vehicle_status),
                7 => format!("  WEAPON:    {}", weapon_info),
                8 => format!("  POS:       ({:.1}, {:.1}) | YAW: {:.0}°", state.player_pos.x, state.player_pos.z, state.player_yaw),
                9 => format!("  LIGHT:     FLASHLIGHT: {}", if state.flashlight_on { "ON [ACTIVE]" } else { "OFF" }),
                10 => "  LEGEND:    @=You Z=Walker R=Runner B=Bloater J=Boss V=Car C=Loot #=Base".into(),
                _ => String::new(),
            };
            println!("║  {}  │ {:<44}║", row_str, side_info);
        }

        println!("╠══════════════════════════════════════════════════════════════════════════════════════════════╣");
        println!("║ COMBAT & MISSION LOG:                                                                        ║");

        let log_lines = state.combat_log.iter().rev().take(4).collect::<Vec<_>>();
        for line in log_lines.iter().rev() {
            println!("║  > {:<88} ║", line);
        }
        for _ in log_lines.len()..4 {
            println!("║                                                                                              ║");
        }

        println!("╠══════════════════════════════════════════════════════════════════════════════════════════════╣");
        println!("║ [WASD] Move/Drive | [Space] Shoot/Melee | [R] Reload | [E] Interact/Vehicle | [H] Medical    ║");
        println!("║ [1-8] Meds | [F] Flashlight | [N] NVG | [TAB] Bag | [C] Craft | [K] Skills | [T] Trader | [Q] Quit ║");
        println!("╚══════════════════════════════════════════════════════════════════════════════════════════════╝");

        stdout.flush()?;
        Ok(())
    }

    fn make_bar(current: f32, max: f32) -> String {
        let pct = (current / max).clamp(0.0, 1.0);
        let filled = (pct * 10.0).round() as usize;
        let empty = 10usize.saturating_sub(filled);
        format!("{}{}", "█".repeat(filled), " ".repeat(empty))
    }
}
