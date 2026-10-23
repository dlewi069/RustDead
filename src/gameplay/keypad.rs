//! `BP_KeypadDoor`'s code entry and `BP_KeypadNote`, from `UI/Widgets/Other/KeypadUI` and
//! `LetterUI` (decompiled):
//!
//! - The door rolls its code at BeginPlay (1111-9999) and writes it onto its note; reading the
//!   note (F) shows it on LetterUI's paper.
//! - KeypadUI: each digit button appends to the display; once as many digits as the code has are
//!   in, a match fires CorrectCodeEntered and the pad closes 0.5 s later, anything else fires
//!   IncorrectCodeEntered and closes it at once. Exit and Esc close it.
//! - The door's side of it (opening, XP, the 80 m noise of a wrong code) is in `level::doors`.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::interact::GameOverlay;
use crate::menus::AppState;
use crate::menus::widgets::{ButtonPalette, MenuFont, button, label_colored, pt};

const PAPER: &str = "game/SurrounDead/Content/UI/Widgets/Journal/Quests/QuestPageBlank.png";
const NOTE_FONT: &str = "game/Blueprints/PluginsAndAssets/Font/F_ESRPG__RU_EN_.ttf";
const PAD_FONT: &str = "fonts/Montserrat-ExtraBold.ttf";
/// KeypadUI's wait between CorrectCodeEntered and closing.
const CORRECT_WAIT: f32 = 0.5;
/// LetterUI's text colour (2B2B2B).
const INK: Color = Color::linear_rgb(0.025, 0.025, 0.025);

/// A code entered on door `door`'s keypad (KeypadUI's two delegates).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct KeypadCode {
    pub door: usize,
    pub correct: bool,
}

/// What a digit press did.
#[derive(Debug, PartialEq)]
pub enum Entered {
    More,
    Correct,
    Wrong,
}

/// The open keypad: the door (its index in `Level::doors`), its code and the digits so far.
#[derive(Resource, Default)]
pub struct KeypadEntry {
    door: Option<usize>,
    code: u32,
    pub typed: String,
    /// Correct, and how long ago.
    correct: Option<f32>,
}

impl KeypadEntry {
    /// SetCurrentCode: append `digit`; with as many digits as the code, compare.
    pub fn press(&mut self, digit: char) -> Entered {
        if self.correct.is_some() {
            return Entered::More;
        }
        self.typed.push(digit);
        if self.typed.len() < self.code.to_string().len() {
            Entered::More
        } else if self.typed.parse::<u32>().ok() == Some(self.code) {
            self.correct = Some(0.0);
            Entered::Correct
        } else {
            Entered::Wrong
        }
    }
}

#[derive(Component)]
struct KeypadUi;
#[derive(Component)]
struct CodeText;
/// A digit button, or Exit.
#[derive(Component, Clone, Copy)]
struct PadKey(Option<char>);
#[derive(Component)]
struct NoteUi;
#[derive(Component)]
struct StopReading;

pub fn plugin(app: &mut App) {
    app.init_resource::<KeypadEntry>()
        .add_message::<KeypadCode>()
        .add_systems(Update, (open_close, show_code, finish).chain().run_if(in_state(AppState::InGame)))
        .add_observer(on_key)
        .add_observer(on_stop_reading);
}

/// Spawn / despawn the keypad and the note with the overlay.
#[allow(clippy::too_many_arguments)]
fn open_close(
    mut commands: Commands,
    overlay: Res<GameOverlay>,
    mut entry: ResMut<KeypadEntry>,
    doors: Option<Res<crate::level::doors::Doors>>,
    assets: Res<AssetServer>,
    pads: Query<Entity, With<KeypadUi>>,
    notes: Query<Entity, With<NoteUi>>,
) {
    let want = match *overlay {
        GameOverlay::Keypad(d) => Some(d),
        _ => None,
    };
    if want != entry.door {
        for e in &pads {
            commands.entity(e).despawn();
        }
        let code = want.and_then(|d| doors.as_ref().map(|s| s.keypad_code(d))).unwrap_or_default();
        *entry = KeypadEntry { door: want, code, ..default() };
        if want.is_some() {
            spawn_keypad(&mut commands, &assets);
        }
    }
    let note = match *overlay {
        GameOverlay::Note(d) => Some(d),
        _ => None,
    };
    if note.is_none() {
        for e in &notes {
            commands.entity(e).despawn();
        }
    } else if notes.is_empty()
        && let Some(d) = note
    {
        let code = doors.as_ref().map(|s| s.keypad_code(d)).unwrap_or_default();
        spawn_note(&mut commands, &assets, code);
    }
}

/// A full-screen layer over the HUD; BackgroundBlur is approximated with a dark overlay.
fn backdrop(dim: f32) -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, dim)),
        GlobalZIndex(40),
        DespawnOnExit(AppState::InGame),
    )
}

fn spawn_keypad(commands: &mut Commands, assets: &AssetServer) {
    let pad_font = MenuFont(assets.load(PAD_FONT));
    // The buttons' WidgetStyle: black at 50 / 80 / 95%, 25 px corners.
    let palette = ButtonPalette { normal: Color::srgba(0.0, 0.0, 0.0, 0.5), hovered: Color::srgba(0.0, 0.0, 0.0, 0.8), pressed: Color::srgba(0.0, 0.0, 0.0, 0.95), chosen: Color::srgba(0.0, 0.0, 0.0, 0.95) };
    // Montserrat-ExtraBold with UMG LetterSpacing 300 (1/1000 em units).
    let text = |s: &str, size: f32| {
        (label_colored(&pad_font, s.to_string(), size, Color::WHITE), TextLayout::justify(Justify::Center), bevy::text::LetterSpacing::Px(size * 96.0 / 72.0 * 0.3))
    };
    let root = commands.spawn((backdrop(0.6), KeypadUi)).id();
    // Border_41: 650x800 centred, #202020 rounded 25, padding 25 (+ the VerticalBox's 25).
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(650.0),
                height: Val::Px(800.0),
                margin: UiRect::new(Val::Px(-325.0), Val::ZERO, Val::Px(-400.0), Val::ZERO),
                padding: UiRect::all(Val::Px(50.0)),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(Val::Px(25.0)),
                ..default()
            },
            BackgroundColor(Color::linear_rgb(0.015, 0.015, 0.015)),
            ChildOf(root),
        ))
        .id();
    // Border_105: the code display (50 px side margins, 15 top), black at 50%.
    commands.spawn((
        Node { margin: UiRect::new(Val::Px(50.0), Val::Px(50.0), Val::Px(15.0), Val::ZERO), padding: UiRect::all(Val::Px(20.0)), justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        ChildOf(panel),
        children![(text("0000", 48.0), CodeText)],
    ));
    // UniformGridPanel_85: 3 columns x 5 rows, 10 px slot padding; 1-9, then 0, then Exit.
    let grid = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
                grid_template_rows: RepeatedGridTrack::flex(5, 1.0),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let cell = |row: i16, col: i16| Node {
        grid_row: GridPlacement::start(row + 1),
        grid_column: GridPlacement::start(col + 1),
        margin: UiRect::all(Val::Px(10.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    // `button` rounds by 4 px; these are 25.
    let mut key = |node: Node, k: PadKey, label: &str, size: f32| {
        commands.spawn((button(node.clone(), palette, k), ChildOf(grid))).insert(Node { border_radius: BorderRadius::all(Val::Px(25.0)), ..node }).with_child(text(label, size));
    };
    for (i, d) in "1234567890".chars().enumerate() {
        let (row, col) = if d == '0' { (3, 1) } else { (i as i16 / 3, i as i16 % 3) };
        key(cell(row, col), PadKey(Some(d)), &d.to_string(), 56.0);
    }
    // Button_Cancel is centred in its cell (sized to its text).
    key(Node { padding: UiRect::all(Val::Px(10.0)), margin: UiRect::ZERO, align_self: AlignSelf::Center, justify_self: JustifySelf::Center, ..cell(4, 1) }, PadKey(None), "Exit", 36.0);
}

fn spawn_note(commands: &mut Commands, assets: &AssetServer, code: u32) {
    let ink = MenuFont(assets.load(NOTE_FONT));
    let ink_label = |s: String, size: f32| (Text::new(s), TextFont { font: ink.0.clone().into(), font_size: pt(size), ..default() }, TextColor(INK));
    commands.spawn((
        backdrop(0.35),
        NoteUi,
        children![(
            // Overlay_0: 2112x1188 at the left-centre anchor, alignment (0.15, 0.48).
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-0.15 * 2112.0),
                top: Val::Percent(50.0),
                width: Val::Px(2112.0),
                height: Val::Px(1188.0),
                margin: UiRect::top(Val::Px(-0.480263 * 1188.0)),
                padding: UiRect::right(Val::Px(50.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ImageNode::new(assets.load(PAPER)),
            children![
                // SizeBox_0 (563.5 x 935.8) holding Description: the code, no digit grouping.
                (Node { width: Val::Px(563.5264), height: Val::Px(935.8425), padding: UiRect::bottom(Val::Px(15.0)), ..default() }, children![ink_label(code.to_string(), 20.0)]),
                (button(Node { padding: UiRect::all(Val::Px(5.0)), ..default() }, ButtonPalette::BARE, StopReading), children![ink_label("Stop Reading".into(), 20.0)]),
            ],
        )],
    ));
}

fn on_key(ev: On<Activate>, keys: Query<&PadKey>, mut entry: ResMut<KeypadEntry>, mut overlay: ResMut<GameOverlay>, mut codes: MessageWriter<KeypadCode>) {
    let Ok(PadKey(key)) = keys.get(ev.entity) else { return };
    let Some(door) = entry.door else { return };
    match key {
        Some(d) => enter(&mut entry, &mut codes, &mut overlay, door, *d),
        None => *overlay = GameOverlay::None,
    }
}

/// A digit pressed: the delegates fire, and a wrong code closes the pad.
fn enter(entry: &mut KeypadEntry, codes: &mut MessageWriter<KeypadCode>, overlay: &mut GameOverlay, door: usize, digit: char) {
    match entry.press(digit) {
        Entered::More => {}
        Entered::Correct => {
            codes.write(KeypadCode { door, correct: true });
        }
        Entered::Wrong => {
            codes.write(KeypadCode { door, correct: false });
            *overlay = GameOverlay::None;
        }
    }
}

fn on_stop_reading(ev: On<Activate>, stop: Query<(), With<StopReading>>, mut overlay: ResMut<GameOverlay>) {
    if stop.contains(ev.entity) {
        *overlay = GameOverlay::None;
    }
}

/// The display shows what's typed (KeypadUI starts at "0000").
fn show_code(entry: Res<KeypadEntry>, mut texts: Query<&mut Text, With<CodeText>>) {
    if !entry.is_changed() {
        return;
    }
    for mut t in &mut texts {
        t.0 = if entry.typed.is_empty() { "0000".into() } else { entry.typed.clone() };
    }
}

/// Close the pad 0.5 s after a correct code.
fn finish(time: Res<Time>, mut entry: ResMut<KeypadEntry>, mut overlay: ResMut<GameOverlay>) {
    let Some(t) = entry.correct.as_mut() else { return };
    *t += time.delta_secs();
    if *t >= CORRECT_WAIT {
        *overlay = GameOverlay::None;
    }
}

/// Type `digits` on the open keypad as if clicked (the screenshot tour).
pub fn type_digits(entry: &mut KeypadEntry, codes: &mut MessageWriter<KeypadCode>, overlay: &mut GameOverlay, digits: &str) {
    let Some(door) = entry.door else { return };
    for d in digits.chars() {
        enter(entry, codes, overlay, door, d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(code: u32) -> KeypadEntry {
        KeypadEntry { door: Some(0), code, ..default() }
    }

    #[test]
    fn the_code_is_checked_once_all_digits_are_in() {
        let mut e = pad(4721);
        assert_eq!(e.press('4'), Entered::More);
        assert_eq!(e.press('7'), Entered::More);
        assert_eq!(e.press('2'), Entered::More);
        assert_eq!(e.press('1'), Entered::Correct);
        // Waiting to close: further presses do nothing.
        assert_eq!(e.press('9'), Entered::More);
        assert_eq!(e.typed, "4721");
        let mut e = pad(4721);
        for d in "472".chars() {
            e.press(d);
        }
        assert_eq!(e.press('2'), Entered::Wrong);
    }
}
