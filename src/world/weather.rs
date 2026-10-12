use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherType {
    ClearSky,
    Overcast,
    DenseFog,
    HeavyRain,
    RadiationStorm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSystem {
    pub time_of_day_hours: f32, // 0.0 to 24.0
    /// Whole days since the game started (the clock's date counts from 9 July 2027).
    #[serde(default)]
    pub days: u32,
    pub time_scale: f32,        // 60x means 1 real second = 1 game minute
    pub current_weather: WeatherType,
    pub weather_timer_s: f32,
    pub temperature_c: f32,
}

impl Default for EnvironmentSystem {
    fn default() -> Self {
        Self {
            // LongdownValley's Ultra_Dynamic_Sky starts at Time of Day 900.
            time_of_day_hours: 9.0,
            days: 0,
            time_scale: 24.0,       // 1 hour game time = 2.5 real minutes
            current_weather: WeatherType::ClearSky,
            weather_timer_s: 0.0,
            temperature_c: 20.0,
        }
    }
}

impl EnvironmentSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self, dt: f32) {
        // Advance clock
        self.time_of_day_hours += (dt * self.time_scale) / 3600.0;
        if self.time_of_day_hours >= 24.0 {
            self.time_of_day_hours -= 24.0;
        }

        // Weather cycle
        self.weather_timer_s += dt;
        if self.weather_timer_s > 600.0 {
            // Change weather every 10 minutes
            self.weather_timer_s = 0.0;
            self.current_weather = match rand::random::<u8>() % 5 {
                0 => WeatherType::ClearSky,
                1 => WeatherType::Overcast,
                2 => WeatherType::DenseFog,
                3 => WeatherType::HeavyRain,
                _ => WeatherType::RadiationStorm,
            };
        }
    }

    pub fn is_daytime(&self) -> bool {
        self.time_of_day_hours >= 6.0 && self.time_of_day_hours < 20.5
    }

    pub fn ambient_light_level(&self) -> f32 {
        if self.is_daytime() {
            let noon_diff = (self.time_of_day_hours - 13.0).abs();
            (1.0 - (noon_diff / 8.0) * 0.4).clamp(0.4, 1.0)
        } else {
            0.10 // Dark night
        }
    }

    pub fn formatted_time(&self) -> String {
        let hours = self.time_of_day_hours.floor() as u32;
        let minutes = ((self.time_of_day_hours - hours as f32) * 60.0).floor() as u32;
        format!("{:02}:{:02}", hours, minutes)
    }
}
