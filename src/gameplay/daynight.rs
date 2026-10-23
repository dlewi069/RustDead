//! Day and night, from LongdownValley's `Ultra_Dynamic_Sky` and the difficulty settings:
//!
//! - The clock (`EnvironmentSystem::time_of_day_hours`, saved with the game) starts at the
//!   level's Time of Day 900 on 9 July 2027 (UDS Year/Month/Day).
//! - Daytime runs from Dawn Time 600 to the level's Dusk Time 1900 and lasts DAY LENGTH real
//!   minutes; the night lasts NIGHT LENGTH (`Save_Difficulties`: 30 and 15).
//! - The sun crosses the sky east to west on UDS's simple arc (pitch = 180° across the day plus
//!   the level's 0.1 vertical offset, about 4°: the cached sun is at 45.5° at 09:00); the moon
//!   takes the night half. NIGHT BRIGHTNESS scales moonlight and night ambient.
//! - `UI/Widgets/Player/TimeUI` (the clock in the top-right corner) shows with the inventory, or
//!   always when CLOCK UI STAYS OPENED? is on (HUD_Game's `UI.TimeStayOpen`).
//!
//! UDS's atmosphere and clouds aren't simulated: the light, ambient, sky and fog colours are
//! blended between day, sunset and night values picked to match the game's look; weather.rs then
//! applies the current weather on top.

use bevy::prelude::*;

use super::interact::GameOverlay;
use super::survival::difficulty;
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame, MenuScreen};

pub const DAWN: f32 = 6.0;
pub const DUSK: f32 = 19.0;
/// The level's Sun Vertical Offset, as degrees of pitch.
const SUN_OFFSET: f32 = 4.0;
/// The date day 0 falls on (UDS Year 2027, Month 7, Day 9).
const START: (i32, u32, u32) = (2027, 7, 9);
/// Clock speed while the inventory is open with OPENING MENUS SLOWS DOWN TIME? on (a guess).
const MENU_SLOWDOWN: f32 = 0.25;

/// Sets the day/night light each frame; weather adjusts it afterwards.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LightTheWorld;

/// The world's directional light (sun by day, moon by night).
#[derive(Component)]
pub struct Sun;

#[derive(Component)]
struct TimeUi;

#[derive(Component)]
enum ClockText {
    Time,
    Month,
    Day,
    Year,
    Season,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::InGame), spawn_clock)
        // The menus' preview scenes expect daylight again.
        .add_systems(OnExit(AppState::InGame), |mut a: ResMut<GlobalAmbientLight>, mut c: ResMut<ClearColor>| {
            a.brightness = 600.0;
            a.color = Color::WHITE;
            c.0 = Color::srgb(0.62, 0.72, 0.82);
        })
        .add_systems(Update, advance_clock.run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)))
        .add_systems(Update, (light_the_world.in_set(LightTheWorld), update_clock).run_if(in_state(AppState::InGame)));
}

fn setting(store: &SettingsStore, page: &str, label: &str) -> bool {
    store.find(&row_key("settings", page, label)) == Some(Value::Bool(true))
}

/// Hours of game time per real second at this hour.
pub fn clock_rate(hour: f32, day_minutes: f32, night_minutes: f32) -> f32 {
    if (DAWN..DUSK).contains(&hour) {
        (DUSK - DAWN) / (day_minutes.max(0.1) * 60.0)
    } else {
        (24.0 - (DUSK - DAWN)) / (night_minutes.max(0.1) * 60.0)
    }
}

fn advance_clock(time: Res<Time>, store: Res<SettingsStore>, overlay: Res<GameOverlay>, mut game: ResMut<CurrentGame>) {
    let Some(state) = game.state.as_mut() else { return };
    let env = &mut state.environment;
    let day = difficulty(&store, "General", "DAY LENGTH");
    let night = difficulty(&store, "General", "NIGHT LENGTH");
    let slow = if overlay.is_open() && setting(&store, "Gameplay", "OPENING MENUS SLOWS DOWN TIME?") { MENU_SLOWDOWN } else { 1.0 };
    env.time_of_day_hours += clock_rate(env.time_of_day_hours, day, night) * time.delta_secs() * slow;
    if env.time_of_day_hours >= 24.0 {
        env.time_of_day_hours -= 24.0;
        env.days += 1;
    }
}

/// The sun's pitch above the eastern horizon in degrees, on a continuous 0..360 cycle: up by day
/// (UDS's arc plus offset), down under the horizon through the night.
pub fn sun_pitch(hour: f32) -> f32 {
    let day = DUSK - DAWN;
    if (DAWN..DUSK).contains(&hour) {
        SUN_OFFSET + 180.0 * (hour - DAWN) / day
    } else {
        let since_dusk = (hour - DUSK).rem_euclid(24.0);
        180.0 + SUN_OFFSET + 180.0 * since_dusk / (24.0 - day)
    }
}

/// Direction towards a body at `pitch` degrees along the east (+Z, UE +Y) to west arc.
fn arc_dir(pitch: f32) -> Vec3 {
    let p = pitch.to_radians();
    Vec3::new(0.0, p.sin(), p.cos())
}

/// Blend of the scene for an hour: (light dir towards source, illuminance, light colour,
/// ambient brightness, ambient colour, sky/fog colour).
pub struct Sky {
    pub towards_light: Vec3,
    pub illuminance: f32,
    pub light: Color,
    pub ambient: f32,
    pub ambient_colour: Color,
    pub sky: Color,
}

pub fn sky_at(hour: f32, night_brightness: f32) -> Sky {
    let pitch = sun_pitch(hour);
    let height = pitch.to_radians().sin(); // sun height, -1..1
    let smooth = |a: f32, b: f32, x: f32| {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    // Twilight stays bright (UDS's sky light keeps lighting the scene around the horizon).
    let day = smooth(-0.05, 0.25, height);
    let glow = smooth(-0.15, 0.25, height);
    let warm = 1.0 - smooth(0.0, 0.4, height);
    let night = smooth(0.0, -0.25, height);
    let nb = night_brightness.max(0.0);
    let mix = |a: Vec3, b: Vec3, t: f32| a.lerp(b, t.clamp(0.0, 1.0));
    let to_col = |v: Vec3| Color::srgb(v.x, v.y, v.z);
    let day_sky = Vec3::new(0.62, 0.72, 0.82);
    let dusk_sky = Vec3::new(0.86, 0.58, 0.42);
    let night_sky = Vec3::new(0.025, 0.035, 0.07) * nb.max(0.2);
    let lit_sky = mix(day_sky, dusk_sky, warm * 0.85);
    let sky = mix(lit_sky * (0.35 + 0.65 * glow), night_sky, night);
    if height > -0.05 {
        // Sun, reddening as it nears the horizon (UDS Sun_Disk_Color).
        Sky {
            towards_light: arc_dir(pitch),
            illuminance: 12000.0 * day,
            light: to_col(mix(Vec3::ONE, Vec3::new(1.0, 0.62, 0.38), warm)),
            // Plus a twilight lift while the sky is lit but the sun is low.
            ambient: 110.0 * nb + 490.0 * glow + 500.0 * (glow - day).max(0.0),
            ambient_colour: to_col(mix(Vec3::new(0.6, 0.7, 1.0), mix(Vec3::ONE, Vec3::new(1.0, 0.8, 0.7), warm), glow)),
            sky: to_col(sky),
        }
    } else {
        // Moon (UDS Moon Light Color), opposite the sun.
        Sky {
            towards_light: arc_dir(pitch - 180.0),
            illuminance: 450.0 * nb * night,
            light: Color::linear_rgb(0.445801, 0.557475, 0.864583),
            ambient: 110.0 * nb + 490.0 * glow,
            ambient_colour: Color::srgb(0.6, 0.7, 1.0),
            sky: to_col(sky),
        }
    }
}

fn light_the_world(
    time: Res<Time>,
    game: Res<CurrentGame>,
    store: Res<SettingsStore>,
    mut cameras: Query<&mut bevy::camera::Exposure, With<super::PlayerCamera>>,
    mut adapted: Local<Option<f32>>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut fog: Query<&mut DistanceFog>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    blood_moon: Res<super::world_events::BloodMoon>,
) {
    let Some(state) = &game.state else { return };
    let mut s = sky_at(state.environment.time_of_day_hours, difficulty(&store, "General", "NIGHT BRIGHTNESS"));
    // SetBloodMoon: UDS Moon Light Color turns red while the moon lights the world.
    if blood_moon.on && sun_pitch(state.environment.time_of_day_hours).to_radians().sin() <= -0.05 {
        s.light = super::world_events::BLOOD_MOON_LIGHT;
    }
    for (mut light, mut t) in &mut sun {
        light.illuminance = s.illuminance;
        light.color = s.light;
        *t = Transform::IDENTITY.looking_to(-s.towards_light, Vec3::X);
    }
    ambient.brightness = s.ambient;
    ambient.color = s.ambient_colour;
    clear.0 = s.sky;
    for mut f in &mut fog {
        f.color = s.sky;
    }
    // Eye adaptation: DefaultCameraPP meters the histogram (speed up 3, down 1, max brightness 8
    // = 3 stops). Metering a light estimate instead of the image keeps it cheap and steady.
    let target = eye_adaptation(&s);
    let a = adapted.get_or_insert(target);
    let rate = if target > *a { 3.0 } else { 1.0 };
    *a += (target - *a).clamp(-rate * time.delta_secs(), rate * time.delta_secs());
    for mut e in &mut cameras {
        e.ev100 = bevy::camera::Exposure::default().ev100 - *a;
    }
}

/// Stops of exposure added for a scene lit like `s`, relative to midday (0..3).
pub fn eye_adaptation(s: &Sky) -> f32 {
    let light = |s: &Sky| s.illuminance * s.towards_light.y.max(0.0) + s.ambient * 3.0;
    let noon = light(&sky_at(12.5, 1.0));
    (noon / light(s).max(1.0)).log2().clamp(0.0, 3.0)
}

// ---------------------------------------------------------------- TimeUI

fn spawn_clock(mut commands: Commands, font: Res<MenuFont>, assets: Res<AssetServer>) {
    // CanvasPanelSlot anchored top-right, alignment (0.83, 0.37): most of the 362 x 443 clock
    // face hangs off the corner.
    let (w, h) = (362.0, 443.0);
    commands.spawn((
        Node { position_type: PositionType::Absolute, right: Val::Px(-(1.0 - 0.83) * w), top: Val::Px(-0.37 * h), width: Val::Px(w), height: Val::Px(h), ..default() },
        Visibility::Hidden,
        TimeUi,
        DespawnOnExit(AppState::InGame),
        GlobalZIndex(3),
        children![
            (Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode::new(assets.load("game/SurrounDead/Content/UI/Textures/ClockInGame.png"))),
            (
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, padding: UiRect::new(Val::Px(5.0), Val::Px(0.0), Val::Px(30.0), Val::Px(0.0)), ..default() },
                children![(label_colored(&font, "09 : 00", 28.0, Color::WHITE), ClockText::Time)],
            ),
            (
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, align_items: AlignItems::Center, padding: UiRect::new(Val::Px(6.0), Val::Px(0.0), Val::Px(0.0), Val::Px(26.0)), ..default() },
                children![
                    (Node::default(), children![(label_colored(&font, "7", 12.0, Color::WHITE), ClockText::Month), label_colored(&font, " / ", 12.0, Color::WHITE), (label_colored(&font, "9", 12.0, Color::WHITE), ClockText::Day)]),
                    (label_colored(&font, "2027", 10.0, Color::WHITE), ClockText::Year),
                ],
            ),
            (
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, padding: UiRect::new(Val::Px(5.0), Val::Px(0.0), Val::Px(87.0), Val::Px(0.0)), ..default() },
                children![(label_colored(&font, "Summer", 10.0, Color::WHITE), ClockText::Season)],
            ),
        ],
    ));
}

/// The calendar date `days` after 9 July 2027.
pub fn date(days: u32) -> (i32, u32, u32) {
    let (mut y, mut m, mut d) = START;
    let month_len = |y: i32, m: u32| match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    for _ in 0..days {
        d += 1;
        if d > month_len(y, m) {
            d = 1;
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }
    }
    (y, m, d)
}

pub fn season(month: u32) -> &'static str {
    match month {
        3..=5 => "Spring",
        6..=8 => "Summer",
        9..=11 => "Autumn",
        _ => "Winter",
    }
}

fn update_clock(
    game: Res<CurrentGame>,
    store: Res<SettingsStore>,
    overlay: Res<GameOverlay>,
    screen: Res<State<MenuScreen>>,
    mut ui: Query<&mut Visibility, With<TimeUi>>,
    mut texts: Query<(&ClockText, &mut Text)>,
) {
    let (Some(state), Ok(mut vis)) = (&game.state, ui.single_mut()) else { return };
    let show = (overlay.shows_inventory() || setting(&store, "Gameplay", "CLOCK UI STAYS OPENED?")) && *screen.get() == MenuScreen::None;
    vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    let env = &state.environment;
    let minutes = (env.time_of_day_hours * 60.0) as u32;
    let (y, m, d) = date(env.days);
    for (which, mut t) in &mut texts {
        let v = match which {
            ClockText::Time => format!("{:02} : {:02}", minutes / 60 % 24, minutes % 60),
            ClockText::Month => m.to_string(),
            ClockText::Day => d.to_string(),
            ClockText::Year => y.to_string(),
            ClockText::Season => season(m).to_string(),
        };
        if t.0 != v {
            t.0 = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_matches_the_levels_cached_position_and_sets_at_dusk() {
        // The level's Ultra_Dynamic_Sky caches Sun World Rotation pitch -45.55 at 09:00.
        assert!((sun_pitch(9.0) - 45.55).abs() < 0.1, "{}", sun_pitch(9.0));
        assert!(sun_pitch(12.5) > 89.0 && sun_pitch(12.5) < 95.0);
        assert!(sky_at(12.0, 1.0).illuminance > 10000.0);
        assert!(sky_at(1.0, 1.0).illuminance < 500.0);
        assert!(sun_pitch(23.0).to_radians().sin() < 0.0, "night is under the horizon");
        // Eyes adapt: none at noon, some at sunset, the full three stops at night.
        assert!(eye_adaptation(&sky_at(12.5, 1.0)) < 0.01);
        assert!((0.5..2.5).contains(&eye_adaptation(&sky_at(18.0, 1.0))));
        assert!(eye_adaptation(&sky_at(23.5, 1.0)) > 2.9);
    }

    #[test]
    fn default_lengths_give_thirty_minute_days_and_fifteen_minute_nights() {
        let day_secs = (DUSK - DAWN) / clock_rate(12.0, 30.0, 15.0);
        let night_secs = (24.0 - (DUSK - DAWN)) / clock_rate(2.0, 30.0, 15.0);
        assert!((day_secs - 1800.0).abs() < 1e-2 && (night_secs - 900.0).abs() < 1e-2);
    }

    #[test]
    fn dates_roll_over_months_and_years() {
        assert_eq!(date(0), (2027, 7, 9));
        assert_eq!(date(23), (2027, 8, 1));
        assert_eq!(date(176), (2028, 1, 1));
        assert_eq!(season(7), "Summer");
    }
}
