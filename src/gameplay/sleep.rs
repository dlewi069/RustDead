//! Sleeping: the beds' and sleeping bag's "Sleep" and the tents' (HUD_Game.ShowSleepWidget,
//! Buildable_Bed), and `UI/Widgets/Buildable/SleepingUI`, decompiled:
//!
//! - Only when UDS's Time of Day is <= 630 or >= 1930 (6.3 / 19.3 h here: UDS's 0-2400 is
//!   linear), else "It's too early to sleep yet, wait until 19:30" (`building::interact`).
//! - SleepingUI: "Do You Want To Sleep Until Morning?", Sleep / Survive The Night. Sleep hides
//!   the choices, shows "Sleeping . . .", and calls UDS's Transition Time of Day(650, 2 s,
//!   EaseInOut, exponent 2, no going backwards): the clock eases forward to 6.5 h, through
//!   midnight into the next day if need be; 2.5 s later the widget goes. Survive The Night and
//!   Esc (IA_PauseMenu, polled on Tick) close it.
//!
//! The needs don't tick through the skipped hours (only the sky's clock moves, as in the game).
//! Ours: BackgroundBlur and its BlurAnim are a dark overlay.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::interact::GameOverlay;
use crate::menus::AppState;
use crate::menus::data::CurrentGame;
use crate::menus::widgets::{ButtonPalette, MenuFont, button, label};

/// The clock the transition eases to (UDS 650).
pub const WAKE_HOUR: f32 = 6.5;
/// Transition Time of Day's duration.
const TRANSITION: f32 = 2.0;
/// SleepingUI's Delay before Exit.
const CLOSE_AFTER: f32 = 2.5;

/// ShowSleepWidget's check: Time of Day <= 630 or >= 1930.
pub fn can_sleep(hour: f32) -> bool {
    hour <= 6.3 || hour >= 19.3
}

/// UE's EaseInOut (EEasingFunc 7) with `exp`.
pub fn ease_in_out(a: f32, exp: f32) -> f32 {
    let a = a.clamp(0.0, 1.0);
    if a < 0.5 { 0.5 * (2.0 * a).powf(exp) } else { 1.0 - 0.5 * (2.0 * (1.0 - a)).powf(exp) }
}

/// UDS's Transition Time of Day: forwards only.
#[derive(Resource, Default, Debug)]
pub struct Sleeping {
    /// (start hour, hours to go, seconds in, the day crossed already)
    transition: Option<(f32, f32, f32, bool)>,
    /// Seconds since Sleep was pressed (the widget's Delay).
    since: Option<f32>,
}

impl Sleeping {
    pub fn start(&mut self, hour: f32) {
        self.transition = Some((hour, (WAKE_HOUR - hour).rem_euclid(24.0), 0.0, false));
        self.since = Some(0.0);
    }

    pub fn active(&self) -> bool {
        self.transition.is_some()
    }

    /// Advance `dt`; the clock to show and whether it passed midnight this step.
    pub fn step(&mut self, dt: f32) -> Option<(f32, bool)> {
        let (from, span, t, crossed) = self.transition.as_mut()?;
        *t += dt;
        let h = *from + *span * ease_in_out(*t / TRANSITION, 2.0);
        let past = h >= 24.0 && !*crossed;
        *crossed |= past;
        let out = (h.rem_euclid(24.0), past);
        if *t >= TRANSITION {
            self.transition = None;
        }
        Some(out)
    }
}

#[derive(Component)]
struct SleepUi;
#[derive(Component)]
struct Choices;
#[derive(Component)]
struct SleepingText;
/// SleepButton (true) / DontSleepButton (false).
#[derive(Component, Clone, Copy)]
pub struct SleepButton(pub bool);

pub fn plugin(app: &mut App) {
    app.init_resource::<Sleeping>()
        .add_systems(OnEnter(AppState::InGame), |mut s: ResMut<Sleeping>| *s = Sleeping::default())
        .add_systems(Update, (open_close, run).chain().after(super::daynight::advance_clock).run_if(in_state(AppState::InGame)))
        .add_observer(on_button);
}

fn open_close(mut commands: Commands, overlay: Res<GameOverlay>, font: Res<MenuFont>, uis: Query<Entity, With<SleepUi>>) {
    let open = *overlay == GameOverlay::Sleep;
    if !open {
        uis.iter().for_each(|e| commands.entity(e).despawn());
    } else if uis.is_empty() {
        spawn(&mut commands, &font);
    }
}

/// SizeBox_55 (900 x 350, centred): the question (46), a 40 px spacer, the two buttons filling a
/// row 50 px apart (text 32); SleepingText (46) over them, hidden until Sleep.
fn spawn(commands: &mut Commands, font: &MenuFont) {
    let root = commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            GlobalZIndex(40),
            DespawnOnExit(AppState::InGame),
            SleepUi,
        ))
        .id();
    let size = commands.spawn((Node { width: Val::Px(900.0), height: Val::Px(350.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, ChildOf(root))).id();
    let choices = commands.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, width: Val::Percent(100.0), ..default() }, Choices, ChildOf(size))).id();
    commands.spawn((label(font, "Do You Want To Sleep Until Morning?", 46.0), ChildOf(choices)));
    let row = commands.spawn((Node { margin: UiRect::top(Val::Px(40.0)), width: Val::Percent(100.0), column_gap: Val::Px(50.0), ..default() }, ChildOf(choices))).id();
    for (sleep, text) in [(true, "Sleep"), (false, "Survive The Night")] {
        let node = Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), padding: UiRect::all(Val::Px(4.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
        commands.spawn((button(node, ButtonPalette::MENU, SleepButton(sleep)), ChildOf(row), children![label(font, text, 32.0)]));
    }
    commands.spawn((label(font, "Sleeping . . .", 46.0), Node { position_type: PositionType::Absolute, ..default() }, Visibility::Hidden, SleepingText, ChildOf(size)));
}

#[allow(clippy::type_complexity)]
fn on_button(
    ev: On<Activate>,
    buttons: Query<&SleepButton>,
    mut overlay: ResMut<GameOverlay>,
    mut sleeping: ResMut<Sleeping>,
    game: Res<CurrentGame>,
    mut choices: Query<&mut Visibility, (With<Choices>, Without<SleepingText>)>,
    mut text: Query<&mut Visibility, (With<SleepingText>, Without<Choices>)>,
) {
    let Ok(SleepButton(sleep)) = buttons.get(ev.entity) else { return };
    if !*sleep {
        *overlay = GameOverlay::None;
        return;
    }
    let Some(hour) = game.state.as_ref().map(|s| s.environment.time_of_day_hours) else { return };
    sleeping.start(hour);
    choices.iter_mut().for_each(|mut v| *v = Visibility::Hidden);
    text.iter_mut().for_each(|mut v| *v = Visibility::Inherited);
    info!("sleep: from {hour:.2} h to {WAKE_HOUR} h");
}

/// The transition on the clock, and the widget's 2.5 s.
fn run(time: Res<Time>, mut sleeping: ResMut<Sleeping>, mut game: ResMut<CurrentGame>, mut overlay: ResMut<GameOverlay>) {
    let dt = time.delta_secs();
    if let Some(s) = sleeping.since.as_mut() {
        *s += dt;
        if *s >= CLOSE_AFTER {
            sleeping.since = None;
            if *overlay == GameOverlay::Sleep {
                *overlay = GameOverlay::None;
            }
        }
    }
    let Some((hour, past_midnight)) = sleeping.step(dt) else { return };
    let Some(state) = game.state.as_mut() else { return };
    state.environment.time_of_day_hours = hour;
    if past_midnight {
        state.environment.days += 1;
    }
    if !sleeping.active() {
        info!("sleep: woke at {hour:.2} h, day {}", state.environment.days);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_at_night() {
        assert!(!can_sleep(12.0) && !can_sleep(19.0) && !can_sleep(6.4));
        assert!(can_sleep(19.3) && can_sleep(23.0) && can_sleep(2.0) && can_sleep(6.3));
    }

    #[test]
    fn eases_forward_through_midnight() {
        assert_eq!(ease_in_out(0.0, 2.0), 0.0);
        assert!((ease_in_out(0.25, 2.0) - 0.125).abs() < 1e-6);
        assert_eq!(ease_in_out(0.5, 2.0), 0.5);
        assert_eq!(ease_in_out(1.0, 2.0), 1.0);
        let mut s = Sleeping::default();
        s.start(22.0);
        let mut crossings = 0;
        let mut last = 22.0;
        while s.active() {
            let (h, past) = s.step(0.1).unwrap();
            crossings += past as u32;
            if !past {
                assert!(h >= last - 1e-4 || last > 20.0, "forwards only: {last} -> {h}");
            }
            last = h;
        }
        assert_eq!(crossings, 1);
        assert!((last - WAKE_HOUR).abs() < 1e-4);
        // Before dawn it's the same day.
        let mut s = Sleeping::default();
        s.start(3.0);
        let mut crossed = false;
        while s.active() {
            crossed |= s.step(0.5).unwrap().1;
        }
        assert!(!crossed);
    }
}
