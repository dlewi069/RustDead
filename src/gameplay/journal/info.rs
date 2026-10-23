//! Journal_InfoUI: the journal's INFO page.
//!
//! - The watch (ClockOverlay: the Clock image and the Time / Date / Season texts, each turned 10°):
//!   hour and minute, month / day and year as two-digit (Conv_IntToText's minimum 2) UDS date,
//!   and UDW's season; EventSetTime refreshes them every second.
//! - The survivor's license (PlayerOverlay, turned 10°): "SURVIVOR'S LICENSE", NAME (forename and
//!   surname), SEX, AGE, PRIOR OCCUPATION and "LVL: N".
//! - "SKULL TRACKER" and the controller's kill stats; the note (turned -5°) with Days Survived
//!   and Distance Travelled (`stats.rs`).
//!
//! UMG's Overlay slots default to Left / Top (unversioned properties leave defaults out, so a
//! slot that only names its horizontal alignment sits at the top). A centred child moves by the
//! whole padding difference (`overlay_centred`); text whose size Slate measures is centred by
//! flex with doubled margins, which comes to the same.

use bevy::prelude::*;

use super::{INK, PAGE, TEX, at, overlay_centred, page_origin};
use crate::menus::widgets::pt;
use crate::menus::{CurrentGame, MenuData};

/// Journal_InfoUI's red-brown labels and the license's gold title.
const LABEL: Color = Color::linear_rgb(0.4, 0.026074, 0.0);
const GOLD: Color = Color::linear_rgb(0.7, 0.449474, 0.0);

/// The watch's live texts.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum InfoClock {
    Hour,
    Minute,
    Month,
    Day,
    Year,
    Season,
}

impl InfoClock {
    /// EventSetTime's text for this field at `hours` into day `days`.
    pub fn text(self, days: u32, hours: f32) -> String {
        let (y, m, d) = crate::gameplay::daynight::date(days);
        let minutes = (hours * 60.0).floor() as u32;
        match self {
            Self::Hour => format!("{:02}", minutes / 60 % 24),
            Self::Minute => format!("{:02}", minutes % 60),
            Self::Month => format!("{m:02}"),
            Self::Day => format!("{d:02}"),
            Self::Year => y.to_string(),
            Self::Season => crate::gameplay::daynight::season(m).to_string(),
        }
    }
}

/// A child of an overlay of `size`, placed like a UMG Overlay slot: centred or Left / Top on each
/// axis, `pad` = (left, top, right, bottom). The wrapper fills the overlay; the child takes the
/// returned margin.
fn slot(size: Vec2, h_centre: bool, v_centre: bool, pad: Vec4) -> (Node, UiRect) {
    let wrapper = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Px(size.x),
        height: Val::Px(size.y),
        justify_content: if h_centre { JustifyContent::Center } else { JustifyContent::FlexStart },
        align_items: if v_centre { AlignItems::Center } else { AlignItems::FlexStart },
        padding: UiRect { left: Val::Px(if h_centre { 0.0 } else { pad.x }), top: Val::Px(if v_centre { 0.0 } else { pad.y }), ..default() },
        ..default()
    };
    let margin = UiRect {
        left: Val::Px(if h_centre { 2.0 * pad.x } else { 0.0 }),
        right: Val::Px(if h_centre { 2.0 * pad.z } else { 0.0 }),
        top: Val::Px(if v_centre { 2.0 * pad.y } else { 0.0 }),
        bottom: Val::Px(if v_centre { 2.0 * pad.w } else { 0.0 }),
    };
    (wrapper, margin)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build(commands: &mut Commands, root: Entity, game: &CurrentGame, data: &MenuData, assets: &AssetServer, hand: &Handle<Font>, bold: &Handle<Font>) {
    let Some(state) = game.state.as_ref() else { return };
    let img = |p: &str| ImageNode::new(assets.load(format!("{TEX}/{p}.png")));
    let turn = |deg: f32| UiTransform::from_rotation(Rot2::degrees(deg));
    let text = |font: &Handle<Font>, s: String, size: f32, colour: Color| (Text::new(s), TextFont { font: font.clone().into(), font_size: pt(size), ..default() }, TextColor(colour), TextLayout::no_wrap(), Pickable::IGNORE);
    let rel = |top_left: Vec2, size: Vec2| Node { position_type: PositionType::Absolute, left: Val::Px(top_left.x), top: Val::Px(top_left.y), width: Val::Px(size.x), height: Val::Px(size.y), ..default() };
    let fill = |size: Vec2| rel(Vec2::ZERO, size);
    // Puts `child` (with its own extra components) into `parent` of `size` at a UMG slot.
    let place = |commands: &mut Commands, parent: Entity, size: Vec2, centre: (bool, bool), pad: [f32; 4], child: Entity| {
        let (wrapper, margin) = slot(size, centre.0, centre.1, Vec4::from_array(pad));
        commands.entity(child).entry::<Node>().and_modify(move |mut n| {
            n.margin = margin;
            n.flex_shrink = 0.0;
        });
        let w = commands.spawn((wrapper, Pickable::IGNORE)).add_child(child).id();
        commands.entity(parent).add_child(w);
    };
    let marker = |name: &str, x: f32, y: f32| (at(Vec2::new(x, 540.0 + y), Vec2::new(1920.0, 1080.0)), img(&format!("Markers/{name}")), turn(6.0), Pickable::IGNORE);
    let b = commands.spawn(marker("BlueMarker", -192.04, -520.0)).id();
    commands.entity(root).add_child(b);
    let b = commands.spawn(marker("YellowMarker", -203.04, -498.0)).id();
    commands.entity(root).add_child(b);

    // Overlay_0 (turned 5°): the page first, everything else over it.
    let page = commands.spawn((at(page_origin(), PAGE), turn(5.0), Pickable::IGNORE)).id();
    commands.entity(root).add_child(page);
    let p = commands.spawn((fill(PAGE), img("Info/InfoPageBlank"), Pickable::IGNORE)).id();
    commands.entity(page).add_child(p);

    // ClockOverlay: 960 x 540 (the Clock image), centred, padding left 115 / top 275.
    let clock_size = Vec2::new(960.0, 540.0);
    let clock = commands.spawn((rel(overlay_centred(PAGE, clock_size, Vec4::new(115.0, 275.0, 0.0, 0.0)), clock_size), Pickable::IGNORE)).id();
    commands.entity(page).add_child(clock);
    let face = commands.spawn((fill(clock_size), img("Info/Clock"), turn(10.0), Pickable::IGNORE)).id();
    commands.entity(clock).add_child(face);
    let (days, hours) = (state.environment.days, state.environment.time_of_day_hours);
    let live = |commands: &mut Commands, which: InfoClock, size: f32| commands.spawn((text(bold, which.text(days, hours), size, Color::WHITE), which)).id();
    let time = commands.spawn((Node::default(), turn(10.0), Pickable::IGNORE)).id();
    let hour = live(commands, InfoClock::Hour, 26.0);
    let colon = commands.spawn((text(bold, ":".into(), 26.0, Color::WHITE), Node { margin: UiRect::horizontal(Val::Px(5.0)), ..default() })).id();
    let minute = live(commands, InfoClock::Minute, 26.0);
    commands.entity(time).add_children(&[hour, colon, minute]);
    place(commands, clock, clock_size, (true, true), [0.0, 13.0, 0.0, 0.0], time);
    let date = commands.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() }, turn(10.0), Pickable::IGNORE)).id();
    let row = commands.spawn((Node::default(), Pickable::IGNORE)).id();
    let month = live(commands, InfoClock::Month, 12.0);
    let slash = commands.spawn(text(bold, " / ".into(), 12.0, Color::WHITE)).id();
    let day = live(commands, InfoClock::Day, 12.0);
    commands.entity(row).add_children(&[month, slash, day]);
    let year = live(commands, InfoClock::Year, 10.0);
    commands.entity(date).add_children(&[row, year]);
    place(commands, clock, clock_size, (true, true), [7.5, 0.0, 0.0, 41.0], date);
    let season = commands.spawn((live_node(), turn(10.0))).id();
    let season_text = live(commands, InfoClock::Season, 10.0);
    commands.entity(season).add_child(season_text);
    place(commands, clock, clock_size, (true, true), [0.0, 70.0, 12.5, 0.0], season);

    // PlayerOverlay: the 640 x 360 license, centred across (padding left 50) at the top, turned 10°.
    let card = Vec2::new(640.0, 360.0);
    let license = commands.spawn((rel(Vec2::new(overlay_centred(PAGE, card, Vec4::new(50.0, 0.0, 0.0, 0.0)).x, 0.0), card), turn(10.0), Pickable::IGNORE)).id();
    commands.entity(page).add_child(license);
    let l = commands.spawn((fill(card), img("Info/License"), Pickable::IGNORE)).id();
    commands.entity(license).add_child(l);
    let profile = game.profile.as_ref();
    let name = profile.map(|p| format!("{} {}", p.forename, p.surname)).unwrap_or_default();
    let sex_row = data.creator.body.iter().position(|r| r.label.starts_with("Sex"));
    let sex = profile.zip(sex_row).and_then(|(p, i)| data.creator.body[i].options.get(*p.body.get(i)?).cloned()).unwrap_or_default();
    let age = profile.map(|p| p.age.clone()).unwrap_or_default();
    let job = profile.and_then(|p| data.creator.occupations.get(p.occupation).cloned()).unwrap_or_default();
    let level = commands.spawn(text(hand, format!("LVL: {}", state.progression.level), 24.0, INK)).id();
    place(commands, license, card, (true, true), [0.0, 10.0, 125.0, 0.0], level);
    let entries = [
        ("SURVIVOR'S LICENSE".into(), 14.0, GOLD, bold, false, [310.0, 70.0, 0.0, 0.0]),
        ("NAME : ".into(), 14.0, LABEL, bold, true, [0.0, 0.0, 0.0, 70.0]),
        (name, 14.0, INK, hand, false, [360.0, 0.0, 0.0, 70.0]),
        ("SEX : ".into(), 14.0, LABEL, bold, true, [0.0, 0.0, 11.0, 35.0]),
        (sex, 14.0, INK, hand, false, [340.0, 0.0, 0.0, 35.0]),
        ("AGE : ".into(), 14.0, LABEL, bold, true, [-8.0, 0.0, 0.0, 0.0]),
        (age, 14.0, INK, hand, false, [345.0, 0.0, 0.0, 0.0]),
        ("PRIOR OCCUPATION : ".into(), 14.0, LABEL, bold, true, [74.0, 37.0, 0.0, 0.0]),
        (job, 14.0, INK, hand, false, [285.0, 60.0, 0.0, 0.0]),
        // (Job, an empty VerticalBox, has nothing to draw.)
        (String::new(), 14.0, INK, hand, true, [74.0, 37.0, 0.0, 0.0]),
    ];
    for (i, (s, size, colour, font, h_centre, pad)) in entries.into_iter().enumerate() {
        // The title is Left / Top; the rest are vertically centred (the Box texts Left across).
        let v_centre = i != 0;
        let e = commands.spawn(text(font, s, size, colour)).id();
        place(commands, license, card, (h_centre, v_centre), pad, e);
    }

    // "SKULL TRACKER" and StatsOverlay (VerticalBox_500, padding 4/2, rows padding 10).
    let title = commands.spawn(text(hand, "SKULL TRACKER".into(), 38.0, INK)).id();
    place(commands, page, PAGE, (true, true), [0.0, 0.0, 200.0, 250.0], title);
    let st = &state.stats;
    let rows = [
        ("Zombies Killed : ", st.zombie_kills),
        ("Boss Zombies Killed : ", st.boss_zombie_kills),
        ("Humans Killed : ", st.human_kills),
        ("Animals Killed : ", st.animal_kills),
        ("Infestations Destroyed : ", st.infestations_destroyed),
    ];
    let stats = commands.spawn((Node { flex_direction: FlexDirection::Column, padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)), ..default() }, Pickable::IGNORE)).id();
    for (label, n) in rows {
        let row = commands.spawn((Node { margin: UiRect::all(Val::Px(10.0)), align_items: AlignItems::Center, ..default() }, Pickable::IGNORE)).id();
        let l = commands.spawn(text(hand, label.into(), 20.0, INK)).id();
        let v = commands.spawn((text(hand, n.to_string(), 20.0, INK), Node { margin: UiRect::left(Val::Px(5.0)), ..default() })).id();
        commands.entity(row).add_children(&[l, v]);
        commands.entity(stats).add_child(row);
    }
    place(commands, page, PAGE, (true, true), [0.0, 0.0, 200.0, 75.0], stats);

    // DaysDistanceOverlay: the 640 x 360 note, centred, padding top 250 / right 200, turned -5°.
    let note = commands.spawn((rel(overlay_centred(PAGE, card, Vec4::new(0.0, 250.0, 200.0, 0.0)), card), turn(-5.0), Pickable::IGNORE)).id();
    commands.entity(page).add_child(note);
    let n = commands.spawn((fill(card), img("Info/DistanceTraveled"), Pickable::IGNORE)).id();
    commands.entity(note).add_child(n);
    for (s, pad) in [
        ("Days Survived : ".to_string(), [0.0, 0.0, 60.0, 45.0]),
        (st.days_survived.to_string(), [30.0, 0.0, 0.0, 45.0]),
        ("Distance Travelled : ".to_string(), [0.0, 35.0, 48.0, 0.0]),
        (st.distance_text(), [100.0, 35.0, 0.0, 0.0]),
    ] {
        let e = commands.spawn(text(hand, s, 18.0, INK)).id();
        place(commands, note, card, (true, true), pad, e);
    }

    let b = commands.spawn(marker("RedMarker", -189.0, -540.0)).id();
    commands.entity(root).add_child(b);
}

fn live_node() -> (Node, Pickable) {
    (Node::default(), Pickable::IGNORE)
}

/// EventSetTime (every second; ours every frame).
pub(super) fn update_clock(game: Res<CurrentGame>, mut texts: Query<(&InfoClock, &mut Text)>) {
    let Some(state) = game.state.as_ref() else { return };
    for (which, mut t) in &mut texts {
        let s = which.text(state.environment.days, state.environment.time_of_day_hours);
        if t.0 != s {
            t.0 = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_watch_pads_to_two_digits() {
        // Day 0 is 9 July 2027; 07:05.
        let t = |w: InfoClock| w.text(0, 7.0 + 5.0 / 60.0 + 1e-4);
        assert_eq!([t(InfoClock::Hour), t(InfoClock::Minute), t(InfoClock::Month), t(InfoClock::Day), t(InfoClock::Year)], ["07", "05", "07", "09", "2027"]);
        assert_eq!(t(InfoClock::Season), "Summer");
    }

    #[test]
    fn centred_slots_move_by_the_whole_padding_difference() {
        // Flex centring with doubled margins: (A - C) / 2 + left - right, like overlay_centred.
        let (w, m) = slot(Vec2::new(640.0, 360.0), true, true, Vec4::new(30.0, 0.0, 0.0, 45.0));
        assert_eq!((w.justify_content, w.align_items), (JustifyContent::Center, AlignItems::Center));
        assert_eq!((m.left, m.right, m.top, m.bottom), (Val::Px(60.0), Val::Px(0.0), Val::Px(0.0), Val::Px(90.0)));
        let child = Vec2::new(100.0, 20.0);
        let free = Vec2::new(640.0, 360.0) - child - Vec2::new(60.0, 90.0);
        let x = free.x / 2.0 + 60.0;
        let y = free.y / 2.0;
        assert!(Vec2::new(x, y).abs_diff_eq(overlay_centred(Vec2::new(640.0, 360.0), child, Vec4::new(30.0, 0.0, 0.0, 45.0)), 1e-3));
    }
}
