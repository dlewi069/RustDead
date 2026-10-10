use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NvgGeneration {
    Civilian, // Gen 1 - Green Phosphor Weak, higher grain
    Military, // Gen 2 - Green Phosphor Strong, crisp clarity
    SpecOps,  // Gen 3 - White Phosphor, high contrast & efficiency
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NightVisionSystem {
    pub is_active: bool,
    pub equipped_gen: Option<NvgGeneration>,
    pub battery_charge: f32, // 0.0 to 100.0
    pub max_battery: f32,
}

impl Default for NightVisionSystem {
    fn default() -> Self {
        Self {
            is_active: false,
            equipped_gen: None,
            battery_charge: 100.0,
            max_battery: 100.0,
        }
    }
}

impl NightVisionSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn equip(&mut self, nvg_gen: NvgGeneration) {
        self.equipped_gen = Some(nvg_gen);
    }

    pub fn unequip(&mut self) {
        self.equipped_gen = None;
        self.is_active = false;
    }

    pub fn toggle(&mut self) -> Result<bool, &'static str> {
        if self.equipped_gen.is_none() {
            return Err("No Night Vision Goggles equipped!");
        }
        if self.battery_charge <= 0.0 {
            return Err("NVG battery depleted! Insert 9V battery to recharge.");
        }
        self.is_active = !self.is_active;
        Ok(self.is_active)
    }

    pub fn tick(&mut self, delta_seconds: f32) {
        if !self.is_active {
            return;
        }

        let drain_rate = match self.equipped_gen {
            Some(NvgGeneration::Civilian) => 0.5, // % per second = 200s total
            Some(NvgGeneration::Military) => 0.3, // % per second = 333s total
            Some(NvgGeneration::SpecOps) => 0.15, // % per second = 666s total
            None => 0.0,
        };

        self.battery_charge = (self.battery_charge - drain_rate * delta_seconds).max(0.0);
        if self.battery_charge <= 0.0 {
            self.is_active = false;
        }
    }

    pub fn recharge(&mut self, amount: f32) -> f32 {
        let prev = self.battery_charge;
        self.battery_charge = (self.battery_charge + amount).min(self.max_battery);
        self.battery_charge - prev
    }

    /// Color tint for screen overlay filter [R, G, B, Alpha]
    pub fn get_tint_color(&self) -> Option<[f32; 4]> {
        if !self.is_active {
            return None;
        }
        match self.equipped_gen {
            Some(NvgGeneration::Civilian) => Some([0.15, 0.85, 0.25, 0.45]), // Greenish weak
            Some(NvgGeneration::Military) => Some([0.10, 0.95, 0.35, 0.55]), // Bright tactical green
            Some(NvgGeneration::SpecOps) => Some([0.80, 0.92, 1.00, 0.50]),  // White phosphor / icy cyan
            None => None,
        }
    }

    pub fn get_grain_intensity(&self) -> f32 {
        match self.equipped_gen {
            Some(NvgGeneration::Civilian) => 0.35,
            Some(NvgGeneration::Military) => 0.15,
            Some(NvgGeneration::SpecOps) => 0.04,
            None => 0.0,
        }
    }

    pub fn get_light_amplification(&self) -> f32 {
        match self.equipped_gen {
            Some(NvgGeneration::Civilian) => 2.0,
            Some(NvgGeneration::Military) => 3.5,
            Some(NvgGeneration::SpecOps) => 5.0,
            None => 1.0,
        }
    }
}
