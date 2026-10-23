//! The player controller's journal stats (BP_PlayerController's ZombieKills, BossZombieKills,
//! HumanKills, AnimalKills, DaysSurvived, DistanceTravelled, InfestationsDestroyed; saved in
//! Save_Player) and DaysSurvivedWidget.
//!
//! - BP_MasterZombie's death adds a zombie kill, BP_ZombieBoss's a boss zombie kill, the animals'
//!   (BP_AnimalDeer/Rabbit/Wolf/Bear) an animal kill (`combat::kill_zombies`). Humans and
//!   infestations aren't in the port yet, so those stay 0.
//! - BP_PlayerCharacter's CheckDistance, every 1 s: the straight-line distance (cm) from where it
//!   was last time is added to DistanceTravelled. A new pawn (game start, load, respawn) starts
//!   from where it stands.
//! - BP_SurroundeadGameMode's EventBindMidnight (UDS's midnight): DaysSurvived + 1, then after a 2 s
//!   Delay the HUD's DaysSurvivedHUD.
//! - Death_PlayerStats on death: with PLAYER JOURNAL STATS RESET ON DEATH? (CVar
//!   Game.PlayerJournalStatsReset, on by default) every stat goes to 0, DaysSurvived too (a new
//!   game's ResetStats sets it to 1, the controller's default).
//! - DaysSurvivedWidget (HUD_Game.DaysSurvivedHUD: on Client_AddUI, i.e. each new pawn, and at
//!   midnight): 4 s after it's made, "Days Survived: N" types out a character every 0.175 s
//!   (Montserrat-Bold 32, grey 0.8, centred, Alignment Y -3: three lines below the middle), holds
//!   2 s and fades out over 1 s (FadeOut).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::survival::PlayerVitals;
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::widgets::pt;
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::CharacterModel;

const SAMPLE_EVERY: f32 = 1.0;
const POPUP_DELAY: f32 = 4.0;
const TYPE_EVERY: f32 = 0.175;
const HOLD: f32 = 2.0;
const FADE: f32 = 1.0;
/// EventBindMidnight's Delay before DaysSurvivedHUD.
const MIDNIGHT_DELAY: f32 = 2.0;

/// Save_Player's stats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JournalStats {
    pub zombie_kills: u32,
    pub boss_zombie_kills: u32,
    pub human_kills: u32,
    pub animal_kills: u32,
    pub days_survived: u32,
    /// DistanceTravelled, UE cm.
    pub distance: f64,
    pub infestations_destroyed: u32,
}

impl Default for JournalStats {
    /// BP_PlayerController's defaults: day 1.
    fn default() -> Self {
        Self { zombie_kills: 0, boss_zombie_kills: 0, human_kills: 0, animal_kills: 0, days_survived: 1, distance: 0.0, infestations_destroyed: 0 }
    }
}

impl JournalStats {
    /// Death_PlayerStats with the reset on: all zero, the days included.
    pub fn reset_on_death(&mut self) {
        *self = Self { days_survived: 0, ..Self::default() };
    }

    /// Journal_InfoUI.GetDistanceTravelled: km to 2 decimals, "1.23km".
    pub fn distance_text(&self) -> String {
        format!("{}km", trim(self.distance / 100000.0, 2))
    }
}

/// FText::AsNumber with at most `digits` fractional digits (trailing zeros dropped).
fn trim(v: f64, digits: usize) -> String {
    let s = format!("{v:.digits$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// PLAYER JOURNAL STATS RESET ON DEATH?
pub fn reset_on_death(store: &SettingsStore) -> bool {
    store.find(&row_key("settings", "Gameplay", "PLAYER JOURNAL STATS RESET ON DEATH?")) == Some(Value::Bool(true))
}

/// What a DaysSurvivedWidget shows `age` seconds after it was made, and its opacity; `None` once
/// it's gone. `full` is "Days Survived: N" (read when the typing starts).
pub fn popup_at(full: &str, age: f32) -> Option<(String, f32)> {
    let typing = age - POPUP_DELAY;
    if typing < 0.0 {
        return Some((String::new(), 1.0));
    }
    let n = full.chars().count();
    let shown = ((typing / TYPE_EVERY).floor() as usize).min(n);
    let done = n as f32 * TYPE_EVERY;
    let fading = typing - done - HOLD;
    if fading >= FADE {
        return None;
    }
    Some((full.chars().take(shown).collect(), if fading > 0.0 { 1.0 - fading / FADE } else { 1.0 }))
}

/// A DaysSurvivedWidget and its age.
#[derive(Component)]
pub struct DaysSurvivedPopup {
    pub age: f32,
    text: Option<String>,
    logged: bool,
}

/// CheckDistance's last location and timer; the pawn and midnight seen last.
#[derive(Resource, Default)]
struct Tracker {
    last: Option<Vec3>,
    timer: f32,
    respawns: u32,
    day: Option<u32>,
    /// EventBindMidnight's Delays still running.
    midnight: Vec<f32>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Tracker>()
        .add_systems(OnEnter(AppState::InGame), |mut commands: Commands, mut t: ResMut<Tracker>| {
            *t = Tracker::default();
            // The pawn's Client_AddUI.
            spawn_popup(&mut commands);
        })
        .add_systems(Update, (track, show_popups).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

fn spawn_popup(commands: &mut Commands) {
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Percent(50.0), ..default() },
        UiTransform { translation: Val2::new(Val::Percent(-50.0), Val::Percent(300.0)), ..default() },
        DaysSurvivedPopup { age: 0.0, text: None, logged: false },
        GlobalZIndex(5),
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
    ));
}

#[allow(clippy::too_many_arguments)]
fn track(
    mut commands: Commands,
    time: Res<Time>,
    mut tracker: ResMut<Tracker>,
    vitals: Res<PlayerVitals>,
    mut game: ResMut<CurrentGame>,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
) {
    let Some(state) = game.state.as_mut() else { return };
    let dt = time.delta_secs();
    // A new pawn after a death: its own CheckDistance from where it stands, and Client_AddUI.
    if vitals.respawns != tracker.respawns {
        tracker.respawns = vitals.respawns;
        tracker.last = None;
        spawn_popup(&mut commands);
    }
    if let Ok(me) = player.single()
        && !vitals.is_dead()
    {
        tracker.timer += dt;
        match tracker.last {
            None => {
                tracker.last = Some(me.translation);
                tracker.timer = 0.0;
            }
            Some(last) if tracker.timer >= SAMPLE_EVERY => {
                tracker.timer -= SAMPLE_EVERY;
                state.stats.distance += (me.translation.distance(last) * 100.0) as f64;
                tracker.last = Some(me.translation);
            }
            _ => {}
        }
    }
    let day = state.environment.days;
    match tracker.day {
        Some(seen) if day > seen => {
            state.stats.days_survived += day - seen;
            tracker.midnight.push(MIDNIGHT_DELAY);
            info!("midnight: days survived {}", state.stats.days_survived);
        }
        _ => {}
    }
    tracker.day = Some(day);
    let before = tracker.midnight.len();
    tracker.midnight.retain_mut(|left| {
        *left -= dt;
        *left > 0.0
    });
    for _ in tracker.midnight.len()..before {
        spawn_popup(&mut commands);
    }
}

fn show_popups(
    mut commands: Commands,
    time: Res<Time>,
    game: Res<CurrentGame>,
    font: Res<crate::menus::widgets::MenuFont>,
    mut popups: Query<(Entity, &mut DaysSurvivedPopup, Option<&Children>)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    let days = game.state.as_ref().map_or(1, |s| s.stats.days_survived);
    for (e, mut p, children) in &mut popups {
        p.age += time.delta_secs();
        // FullText is read when the 4 s Delay ends.
        if p.age >= POPUP_DELAY && p.text.is_none() {
            p.text = Some(format!("Days Survived: {days}"));
        }
        let Some((shown, alpha)) = popup_at(p.text.as_deref().unwrap_or(""), p.age) else {
            commands.entity(e).despawn();
            continue;
        };
        if p.text.as_deref() == Some(shown.as_str()) && !p.logged {
            p.logged = true;
            info!("days survived popup: {shown}");
        }
        let colour = Color::linear_rgba(0.8, 0.8, 0.8, alpha);
        match children.and_then(|c| c.first()).and_then(|&c| texts.get_mut(c).ok()) {
            Some((mut t, mut c)) => {
                if t.0 != shown {
                    t.0 = shown;
                }
                c.0 = colour;
            }
            None => {
                commands.entity(e).with_child((Text::new(shown), TextFont { font: font.0.clone().into(), font_size: pt(32.0), ..default() }, TextColor(colour), Pickable::IGNORE));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_game_is_day_one_and_death_zeroes_it() {
        let mut s = JournalStats::default();
        assert_eq!(s.days_survived, 1);
        s.zombie_kills = 3;
        s.distance = 5.0;
        s.reset_on_death();
        assert_eq!(s, JournalStats { days_survived: 0, ..JournalStats::default() });
    }

    #[test]
    fn distance_reads_in_km() {
        let mut s = JournalStats::default();
        assert_eq!(s.distance_text(), "0km");
        s.distance = 123_456.0;
        assert_eq!(s.distance_text(), "1.23km");
        s.distance = 250_000.0;
        assert_eq!(s.distance_text(), "2.5km");
    }

    #[test]
    fn the_popup_types_holds_and_fades() {
        let full = "Days Survived: 2";
        assert_eq!(popup_at(full, 1.0), Some((String::new(), 1.0)));
        assert_eq!(popup_at(full, 4.0 + 0.175 * 3.5).map(|p| p.0), Some("Day".into()));
        let typed = 4.0 + 0.175 * 16.0;
        assert_eq!(popup_at(full, typed + 1.0), Some((full.into(), 1.0)));
        let (_, a) = popup_at(full, typed + 2.5).unwrap();
        assert!((a - 0.5).abs() < 1e-3);
        assert_eq!(popup_at(full, typed + 3.01), None);
    }

    #[test]
    fn old_saves_start_at_day_one() {
        let s: JournalStats = serde_json::from_str("{}").unwrap();
        assert_eq!(s, JournalStats::default());
    }
}
