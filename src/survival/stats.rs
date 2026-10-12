use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurvivalStats {
    pub health: f32,
    pub max_health: f32,
    pub stamina: f32,
    pub max_stamina: f32,
    pub hunger: f32,      // 0 = starving, 100 = full
    pub thirst: f32,      // 0 = dehydrated, 100 = quenched
    pub infection: f32,   // 0 = healthy, 100 = infected / dead
    pub immunity: f32,    // resistance against infection progression
    pub radiation: f32,   // 0 = safe, 100 = lethal
    pub bleeding: bool,
    pub bleeding_rate: f32, // health loss per second when bleeding
}

impl Default for SurvivalStats {
    fn default() -> Self {
        Self {
            health: 100.0,
            max_health: 100.0,
            stamina: 100.0,
            max_stamina: 100.0,
            hunger: 100.0,
            thirst: 100.0,
            infection: 0.0,
            immunity: 100.0,
            radiation: 0.0,
            bleeding: false,
            bleeding_rate: 2.5,
        }
    }
}

impl SurvivalStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance survival metrics by delta time (in seconds)
    pub fn tick(&mut self, dt: f32, is_sprinting: bool) {
        // 1. Hunger and Thirst decay rates (SurrounDead balance)
        let hunger_decay = 0.15 * dt; // ~11 minutes real time to fully starve
        let thirst_decay = 0.25 * dt; // ~6.6 minutes real time to dehydrate

        self.hunger = (self.hunger - hunger_decay).max(0.0);
        self.thirst = (self.thirst - thirst_decay).max(0.0);

        // 2. Starvation / Dehydration health penalty
        if self.hunger <= 0.0 {
            self.health = (self.health - 1.5 * dt).max(0.0);
        }
        if self.thirst <= 0.0 {
            self.health = (self.health - 2.5 * dt).max(0.0);
        }

        // 3. Bleeding and 5. radiation damage are gameplay::medical (the game's MedicalComponent).

        // 4. Infection progression
        if self.infection > 0.0 {
            let progression_rate = (0.2 - (self.immunity / 100.0) * 0.15).max(0.05);
            self.infection = (self.infection + progression_rate * dt).min(100.0);
            if self.infection >= 80.0 {
                // High infection weakens the player
                self.health = (self.health - 1.0 * dt).max(0.0);
            }
        }


        // 6. Stamina recovery / drain
        if is_sprinting {
            self.stamina = (self.stamina - 15.0 * dt).max(0.0);
        } else if self.stamina < self.max_stamina && self.hunger > 20.0 && self.thirst > 20.0 {
            self.stamina = (self.stamina + 10.0 * dt).min(self.max_stamina);
        } else if self.stamina < self.max_stamina {
            // Slower recovery when malnourished
            self.stamina = (self.stamina + 4.0 * dt).min(self.max_stamina);
        }

        // 7. Natural health regeneration when well fed and not injured
        if self.hunger > 75.0
            && self.thirst > 75.0
            && !self.bleeding
            && self.infection < 20.0
            && self.radiation < 10.0
            && self.health < self.max_health
        {
            self.health = (self.health + 0.5 * dt).min(self.max_health);
        }
    }

    pub fn take_damage(&mut self, amount: f32, can_bleed: bool, infection_chance: f32) {
        self.health = (self.health - amount).max(0.0);
        if can_bleed && rand::random::<f32>() < 0.35 {
            self.bleeding = true;
        }
        if rand::random::<f32>() < infection_chance {
            self.infection = (self.infection + 15.0).min(100.0);
        }
    }

    pub fn heal(&mut self, amount: f32) {
        self.health = (self.health + amount).min(self.max_health);
    }

    pub fn apply_bandage(&mut self) {
        self.bleeding = false;
        self.heal(15.0);
    }

    pub fn take_antibiotics(&mut self) {
        self.infection = (self.infection - 40.0).max(0.0);
        self.immunity = (self.immunity + 25.0).min(100.0);
    }

    pub fn drink_water(&mut self, amount: f32) {
        self.thirst = (self.thirst + amount).min(100.0);
    }

    pub fn eat_food(&mut self, hunger_gain: f32, thirst_gain: f32) {
        self.hunger = (self.hunger + hunger_gain).min(100.0);
        self.thirst = (self.thirst + thirst_gain).min(100.0);
    }

    pub fn drain_stamina(&mut self, amount: f32) -> bool {
        if self.stamina >= amount {
            self.stamina -= amount;
            true
        } else {
            false
        }
    }

    pub fn take_radiation(&mut self, amount: f32) {
        self.radiation = (self.radiation + amount).min(100.0);
    }

    pub fn is_alive(&self) -> bool {
        self.health > 0.0 && self.infection < 100.0
    }
}
