use crate::crafting::StructureType;
use crate::engine::GameState;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

pub struct InputHandler;

impl InputHandler {
    pub fn process_input(state: &mut GameState, timeout: Duration) -> std::io::Result<bool> {
        if event::poll(timeout)? {
            if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
                state.is_sprinting = modifiers.contains(KeyModifiers::SHIFT);

                let move_speed = if state.is_sprinting { 3.5 } else { 1.8 };

                match code {
                    KeyCode::Char('w') | KeyCode::Up => {
                        if let Some(vid) = state.active_vehicle_id {
                            if let Some(veh) = state.vehicles.iter_mut().find(|v| v.id == vid) {
                                veh.drive(1.0, 0.0, 0.1);
                            }
                        } else {
                            let yaw_rad = state.player_yaw.to_radians();
                            state.player_pos.x += yaw_rad.sin() * move_speed;
                            state.player_pos.z += yaw_rad.cos() * move_speed;
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Down => {
                        if let Some(vid) = state.active_vehicle_id {
                            if let Some(veh) = state.vehicles.iter_mut().find(|v| v.id == vid) {
                                veh.drive(-1.0, 0.0, 0.1);
                            }
                        } else {
                            let yaw_rad = state.player_yaw.to_radians();
                            state.player_pos.x -= yaw_rad.sin() * move_speed;
                            state.player_pos.z -= yaw_rad.cos() * move_speed;
                        }
                    }
                    KeyCode::Char('a') | KeyCode::Left => {
                        if let Some(vid) = state.active_vehicle_id {
                            if let Some(veh) = state.vehicles.iter_mut().find(|v| v.id == vid) {
                                veh.drive(0.0, -1.0, 0.1);
                            }
                        } else {
                            state.player_yaw = (state.player_yaw - 15.0).rem_euclid(360.0);
                        }
                    }
                    KeyCode::Char('d') | KeyCode::Right => {
                        if let Some(vid) = state.active_vehicle_id {
                            if let Some(veh) = state.vehicles.iter_mut().find(|v| v.id == vid) {
                                veh.drive(0.0, 1.0, 0.1);
                            }
                        } else {
                            state.player_yaw = (state.player_yaw + 15.0).rem_euclid(360.0);
                        }
                    }
                    KeyCode::Char(' ') => {
                        state.player_shoot();
                    }
                    KeyCode::Char('r') => {
                        state.player_reload();
                    }
                    KeyCode::Char('e') => {
                        state.player_interact();
                    }
                    KeyCode::Char('f') => {
                        state.flashlight_on = !state.flashlight_on;
                        state.log_message(format!(
                            "Flashlight {}",
                            if state.flashlight_on { "enabled" } else { "disabled" }
                        ));
                    }
                    KeyCode::Char('1') => {
                        state.use_item("bandage");
                    }
                    KeyCode::Char('2') => {
                        state.use_item("purified_water");
                    }
                    KeyCode::Char('3') => {
                        state.use_item("canned_peaches");
                    }
                    KeyCode::Char('b') => {
                        // Quick build wooden wall in front of player
                        state.place_structure(StructureType::WoodenWall);
                    }
                    KeyCode::Char('c') => {
                        // Quick craft bandage if materials available
                        state.craft_recipe("craft_bandage");
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(false); // Quit signal
                    }
                    _ => {}
                }
            }
        }
        Ok(true) // Continue running
    }
}
