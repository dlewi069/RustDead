use crate::items::GridContainer;
use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VehicleType {
    Sedan,
    PickupTruck,
    FourByFourSuv,
    ArmoredTruck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vehicle {
    pub id: u32,
    pub vehicle_type: VehicleType,
    pub name: String,
    pub position: Position3D,
    pub yaw: f32,
    pub speed_kmh: f32,
    pub max_speed_kmh: f32,
    pub fuel_liters: f32,
    pub max_fuel_liters: f32,
    pub fuel_consumption_per_km: f32,
    pub battery_pct: f32,
    pub engine_health: f32,
    pub max_engine_health: f32,
    pub trunk: GridContainer,
    pub is_engine_running: bool,
    pub headlights_on: bool,
}

impl Vehicle {
    pub fn new(id: u32, vehicle_type: VehicleType, position: Position3D) -> Self {
        let (name, max_speed, max_fuel, burn_rate, trunk_w, trunk_h, trunk_cap) = match vehicle_type {
            VehicleType::Sedan => ("Olds 88 Sedan", 110.0, 50.0, 0.12, 6, 4, 60.0),
            VehicleType::PickupTruck => ("D-150 Pickup Truck", 95.0, 65.0, 0.18, 6, 8, 120.0),
            VehicleType::FourByFourSuv => ("Cherokee 4x4 SUV", 100.0, 60.0, 0.16, 6, 6, 90.0),
            VehicleType::ArmoredTruck => ("Bearcat Armored Truck", 80.0, 90.0, 0.28, 6, 8, 180.0),
        };

        let trunk = GridContainer::new(format!("vehicle_trunk_{}", id), format!("{} Trunk", name), trunk_w, trunk_h, trunk_cap);

        Self {
            id,
            vehicle_type,
            name: name.into(),
            position,
            yaw: 0.0,
            speed_kmh: 0.0,
            max_speed_kmh: max_speed,
            fuel_liters: max_fuel * 0.45, // 45% full by default
            max_fuel_liters: max_fuel,
            fuel_consumption_per_km: burn_rate,
            battery_pct: 85.0,
            engine_health: 100.0,
            max_engine_health: 100.0,
            trunk,
            is_engine_running: false,
            headlights_on: false,
        }
    }

    pub fn start_engine(&mut self) -> Result<(), &'static str> {
        if self.fuel_liters <= 0.0 {
            return Err("Out of fuel. Add gasoline with a jerry can.");
        }
        if self.battery_pct < 10.0 {
            return Err("Battery dead. Requires recharge or replacement.");
        }
        if self.engine_health <= 0.0 {
            return Err("Engine destroyed. Repair with scrap metal and wrench.");
        }

        self.is_engine_running = true;
        Ok(())
    }

    pub fn stop_engine(&mut self) {
        self.is_engine_running = false;
        self.speed_kmh = 0.0;
    }

    pub fn drive(&mut self, throttle: f32, steer: f32, dt: f32) {
        if !self.is_engine_running {
            self.speed_kmh = (self.speed_kmh - 25.0 * dt).max(0.0);
            return;
        }

        // Acceleration / Braking
        if throttle > 0.0 {
            let accel = 18.0 * throttle;
            self.speed_kmh = (self.speed_kmh + accel * dt).min(self.max_speed_kmh);
        } else if throttle < 0.0 {
            let brake = 35.0 * throttle.abs();
            self.speed_kmh = (self.speed_kmh - brake * dt).max(-25.0); // reverse max 25 km/h
        } else {
            // Coasting friction
            if self.speed_kmh > 0.0 {
                self.speed_kmh = (self.speed_kmh - 10.0 * dt).max(0.0);
            } else {
                self.speed_kmh = (self.speed_kmh + 10.0 * dt).min(0.0);
            }
        }

        // Steering
        if self.speed_kmh.abs() > 1.0 {
            self.yaw += steer * 45.0 * dt * (self.speed_kmh / self.max_speed_kmh);
        }

        // Movement translation
        let speed_mps = (self.speed_kmh * 1000.0) / 3600.0;
        let distance_m = speed_mps * dt;
        let yaw_rad = self.yaw.to_radians();

        self.position.x += yaw_rad.sin() * distance_m;
        self.position.z += yaw_rad.cos() * distance_m;

        // Fuel burn
        let distance_km = distance_m / 1000.0;
        let burned = distance_km.abs() * self.fuel_consumption_per_km;
        self.fuel_liters = (self.fuel_liters - burned).max(0.0);

        if self.fuel_liters <= 0.0 {
            self.stop_engine();
        }
    }

    pub fn refuel(&mut self, liters: f32) -> f32 {
        let needed = self.max_fuel_liters - self.fuel_liters;
        let added = needed.min(liters);
        self.fuel_liters += added;
        added
    }

    pub fn repair_engine(&mut self, amount: f32) {
        self.engine_health = (self.engine_health + amount).min(self.max_engine_health);
    }

    pub fn replace_battery(&mut self) {
        self.battery_pct = 100.0;
    }

    pub fn is_operational(&self) -> bool {
        self.engine_health > 0.0 && self.fuel_liters > 0.0 && self.battery_pct >= 10.0
    }

    pub fn check_collision_with_circle(&self, center: &Position3D, radius: f32) -> bool {
        self.position.distance_2d(center) <= (radius + 2.0)
    }

    pub fn obstacle_crash(&mut self) -> f32 {
        let speed = self.speed_kmh.abs();
        let crash_damage = (speed * 0.75).clamp(8.0, 65.0);
        self.engine_health = (self.engine_health - crash_damage).max(0.0);
        // Bounce back slightly and stall speed
        self.speed_kmh = -self.speed_kmh * 0.25;
        if self.engine_health <= 0.0 {
            self.stop_engine();
        }
        crash_damage
    }

    pub fn roadkill_impact(&mut self, target_hp: f32) -> f32 {
        // Kinetic damage based on speed: 100 km/h = 250 dmg
        let kinetic_damage = (self.speed_kmh.abs() * 2.5).max(10.0);
        // Engine/chassis absorbs some collision damage
        let chassis_damage = (target_hp * 0.05).min(15.0);
        self.engine_health = (self.engine_health - chassis_damage).max(0.0);
        // Slow down vehicle slightly on impact
        self.speed_kmh *= 0.85;

        kinetic_damage
    }

    pub fn can_access_trunk(&self, player_pos: &Position3D) -> bool {
        self.position.distance_to(player_pos) <= 4.0
    }

    pub fn trunk_item_count(&self) -> usize {
        self.trunk.items.len()
    }

    pub fn trunk_weight(&self) -> f32 {
        self.trunk.current_weight()
    }

    pub fn auto_insert_trunk(&mut self, item: crate::items::Item) -> Result<(), crate::items::Item> {
        self.trunk.auto_insert(item)
    }

    pub fn remove_trunk_item(&mut self, index: usize) -> Option<crate::items::Item> {
        self.trunk.remove_at(index)
    }
}
