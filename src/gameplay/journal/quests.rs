//! Journal_QuestsUI: the journal's QUESTS page.
//!
//! - The shown quest (Show Quest: on opening, the last quest in progress): QuestTitle (red, 36),
//!   QuestDescription (14, centred) and, under the "TO DO:" note (QuestObjectives), the branches of
//!   every state it reached, newest first; branches of states behind it at 0.5 opacity
//!   (Journal_QuestBranchUI's Completed?), tasks as Journal_QuestTask ("- {Description}
//!   {Optional} {Prog}", done ones at 0.15 with the Cross at 0.5).
//! - The two folders: ActiveQuests ("Active [N]": GetInProgressQuests) and CompletedQuests
//!   ("Quest Log [N]": N counts GetSucceededQuests; the list is the failed then the succeeded).
//!   Clicking a folder's button brings it and its list in front (the slots' ZOrders). A quest's
//!   button (Journal_QuestButtonUI: 25 % black, 50 % hovered) shows it.
//! - FORGET (Button_ForgetQuest, the Forget icon; ForgetBW while no quest is shown, ours: the
//!   disabled brush isn't in the dump) asks Journal_QuestConfirmForget ("Are you sure you want to
//!   forget this quest?" YES / NO); YES forgets it (`quests::ForgetQuest`).
//!
//! Not ported: SaveIcon / SaveEnabledIcon ("Random Quest: Doesn't Save"), scrolling the lists,
//! the dialog's BackgroundBlur (a dark overlay, as elsewhere). The Journal_QuestTask RichTextBlock's
//! style isn't readable: the handwriting font at 16 is ours, as are the YES / NO buttons' looks.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button};

use super::{INK, Journal, PAGE, TEX, at, page_origin};
use crate::gameplay::quests::{ForgetQuest, Outcome, QuestDb, QuestLog};
use crate::menus::widgets::pt;

/// QuestTitle's and ObjectivesText's red.
const RED: Color = Color::linear_rgb(0.5, 0.0, 0.0);

/// A list entry (Journal_QuestButtonUI): shows its quest.
#[derive(Component, Clone)]
pub struct QuestEntry(pub String);
/// ActiveButton (false) / CompletedButton (true).
#[derive(Component, Clone, Copy)]
pub struct FolderButton(pub bool);
#[derive(Component)]
pub struct ForgetButton;
/// Journal_QuestConfirmForget's YES (true) / NO (false).
#[derive(Component, Clone, Copy)]
pub struct ConfirmButton(pub bool);
/// Buttons drawn as a 25 % black box, 50 % while hovered.
#[derive(Component)]
struct ShadedButton;

pub(super) fn plugin(app: &mut App) {
    app.add_observer(on_click).add_systems(Update, (shade_buttons, follow_log).run_if(in_state(crate::menus::AppState::InGame)));
}

/// Show Quest's pick on Initialize: the last quest in progress.
pub fn last_active(log: &QuestLog) -> Option<String> {
    log.active().last().map(|q| q.class.clone())
}

/// The lists: in progress, and failed then succeeded; "Quest Log [N]" counts the succeeded.
pub fn lists(log: &QuestLog) -> (Vec<&str>, Vec<&str>, usize) {
    let of = |o: Option<Outcome>| log.quests.iter().filter(move |q| q.outcome == o).map(|q| q.class.as_str());
    let active: Vec<&str> = of(None).collect();
    let done: Vec<&str> = of(Some(Outcome::Failed)).chain(of(Some(Outcome::Succeeded))).collect();
    let succeeded = log.quests.iter().filter(|q| q.outcome == Some(Outcome::Succeeded)).count();
    (active, done, succeeded)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build(commands: &mut Commands, root: Entity, journal: &Journal, log: &QuestLog, qdb: &QuestDb, assets: &AssetServer, font: &Handle<Font>) {
    let img = |p: &str| ImageNode::new(assets.load(format!("{TEX}/{p}.png")));
    let turn = |deg: f32| UiTransform::from_rotation(Rot2::degrees(deg));
    let text = |s: String, size: f32, colour: Color| (Text::new(s), TextFont { font: font.clone().into(), font_size: pt(size), ..default() }, TextColor(colour), Pickable::IGNORE);
    let fill = |size: Vec2| Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(size.x), height: Val::Px(size.y), ..default() };
    let add = |commands: &mut Commands, z: i32, b: Entity| {
        commands.entity(b).insert(ZIndex(z));
        commands.entity(root).add_child(b);
    };
    let marker = |name: &str, x: f32, y: f32| (at(Vec2::new(x, 540.0 + y), Vec2::new(1920.0, 1080.0)), img(&format!("Markers/{name}")), turn(6.0), Pickable::IGNORE);
    let b = commands.spawn(marker("RedMarker", -189.0, -540.0)).id();
    add(commands, 0, b);
    let b = commands.spawn(marker("BlueMarker", -192.04, -520.0)).id();
    add(commands, 0, b);

    // Overlay_0 (turned 5°): the page.
    let page = commands.spawn((at(page_origin(), PAGE), turn(5.0), Pickable::IGNORE, children![(fill(PAGE), img("Quests/QuestPageBlank"), Pickable::IGNORE)])).id();
    add(commands, 0, page);

    let shown = journal.quest.as_deref().and_then(|c| Some((c, qdb.quests.get(c)?)));
    if let Some((_, def)) = shown {
        let title = commands.spawn((at(Vec2::new(410.0, 17.0), Vec2::new(635.0, 108.0)), turn(5.0), Pickable::IGNORE, children![text(def.name.clone(), 36.0, RED)])).id();
        add(commands, 0, title);
        let desc = commands
            .spawn((
                turn(5.0),
                Node { padding: UiRect::all(Val::Px(10.0)), justify_content: JustifyContent::Center, overflow: Overflow::clip(), ..at(Vec2::new(395.0, 137.0), Vec2::new(690.0, 105.0)) },
                Pickable::IGNORE,
                children![(text(def.description.replace("\r\n", "\n"), 14.0, INK), TextLayout::justify(Justify::Center))],
            ))
            .id();
        add(commands, 0, desc);
    }
    // ObjectivesOverlay (turned 5°): the note and "TO DO:" (turned -2°, padding right 10 / bottom 132).
    let note = Vec2::new(853.0, 480.0);
    let objectives = commands
        .spawn((
            at(Vec2::new(246.58, 175.92), note),
            turn(5.0),
            Pickable::IGNORE,
            children![
                (fill(note), img("Quests/QuestObjectives"), Pickable::IGNORE),
                (
                    Node { width: Val::Px(note.x), height: Val::Px(note.y), justify_content: JustifyContent::Center, align_items: AlignItems::Center, padding: UiRect { right: Val::Px(2.0 * 10.0), bottom: Val::Px(2.0 * 132.0), ..default() }, ..default() },
                    Pickable::IGNORE,
                    children![(text("TO DO:".into(), 58.0, RED), turn(-2.0))],
                ),
            ],
        ))
        .id();
    add(commands, 0, objectives);
    // BranchesBox (turned 3.5°).
    let branches = commands.spawn((Node { flex_direction: FlexDirection::Column, overflow: Overflow::clip(), ..at(Vec2::new(433.0, 337.5), Vec2::new(500.0, 200.0)) }, turn(3.5), Pickable::IGNORE)).id();
    add(commands, 0, branches);
    if let Some((class, _)) = shown {
        for (view, behind) in log.journal_branches(qdb, class) {
            let alpha = if behind { 0.5 } else { 1.0 };
            let branch = commands.spawn((Node { flex_direction: FlexDirection::Column, margin: UiRect::top(Val::Px(10.0)), ..default() }, Pickable::IGNORE)).id();
            commands.entity(branches).add_child(branch);
            for (line, done) in view.0 {
                let a = alpha * if done { 0.15 } else { 1.0 };
                let row = commands
                    .spawn((
                        Node { margin: UiRect::left(Val::Px(15.0)), align_self: AlignSelf::FlexStart, ..default() },
                        Pickable::IGNORE,
                        children![
                            text(line, 16.0, INK.with_alpha(a)),
                            (
                                Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), top: Val::Percent(50.0), height: Val::Px(2.0), ..default() },
                                BackgroundColor(INK.with_alpha(alpha * if done { 0.5 } else { 0.0 })),
                                Pickable::IGNORE
                            ),
                        ],
                    ))
                    .id();
                commands.entity(branch).add_child(row);
            }
        }
    }

    // Button_ForgetQuest: the canvas centre + (7, -45), 75 px (turned 5°).
    let forget = commands
        .spawn((at(Vec2::new(960.0 + 7.0, 540.0 - 45.0), Vec2::splat(75.0)), turn(5.0), img(if shown.is_some() { "Quests/Icons/Forget" } else { "Quests/Icons/ForgetBW" }), Hovered::default()))
        .id();
    if shown.is_some() {
        commands.entity(forget).insert((Button, ForgetButton));
    }
    add(commands, 1, forget);

    // The folders, their lists and buttons. The front folder and its list sit at ZOrder 1 / 2.
    let (active, done, succeeded) = lists(log);
    let front_log = journal.log_tab;
    let folder = |name: &str| (at(Vec2::new(21.0, 465.0), Vec2::new(1280.0, 720.0)), img(name), turn(5.0), Pickable::IGNORE);
    let a = commands.spawn(folder("Quests/ActiveQuests")).id();
    add(commands, if front_log { 0 } else { 1 }, a);
    let c = commands.spawn(folder("Quests/CompletedQuests")).id();
    add(commands, if front_log { 1 } else { 0 }, c);
    for (log_list, quests) in [(false, &active), (true, &done)] {
        let list = commands.spawn((Node { flex_direction: FlexDirection::Column, overflow: Overflow::clip(), ..at(Vec2::new(372.0, 655.0), Vec2::new(575.0, 370.0)) }, turn(5.0))).id();
        for q in quests.iter() {
            let name = qdb.quests.get(*q).map_or_else(|| q.to_string(), |d| d.name.clone());
            let b = commands
                .spawn((
                    Node { margin: UiRect::bottom(Val::Px(10.0)), padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)), justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(10.0)), ..default() },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
                    Button,
                    Hovered::default(),
                    ShadedButton,
                    QuestEntry(q.to_string()),
                    children![text(name, 16.0, INK)],
                ))
                .id();
            commands.entity(list).add_child(b);
        }
        let front = log_list == front_log;
        add(commands, if front { if log_list { 2 } else { 1 } } else { 0 }, list);
    }
    for (log_list, label, x, y) in [(false, format!("Active [{}]", active.len()), 419.0, 584.8877), (true, format!("Quest Log [{succeeded}]"), 619.0, 600.0)] {
        let b = commands
            .spawn((
                Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..at(Vec2::new(x, y), Vec2::new(100.0, 30.0)) },
                turn(5.0),
                Button,
                Hovered::default(),
                FolderButton(log_list),
                children![(text(label, 20.0, INK), TextLayout::no_wrap(), UiTransform::default(), FolderLabel)],
            ))
            .id();
        add(commands, 3, b);
    }
    let b = commands.spawn(marker("YellowMarker", -203.04, -498.0)).id();
    add(commands, 0, b);

    if journal.confirm_forget {
        // Journal_QuestConfirmForget (AddToViewport 3): over everything.
        let dialog = commands
            .spawn((
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                children![
                    (at(Vec2::new(960.0 - 480.0, 540.0 - 270.0), Vec2::new(960.0, 540.0)), img("Quests/QuestObjectives"), Pickable::IGNORE),
                    (
                        Node { justify_content: JustifyContent::Center, ..at(Vec2::new(960.0 - 261.0, 540.0 - 0.896725 * 90.0), Vec2::new(522.0, 90.0)) },
                        Pickable::IGNORE,
                        children![(text("Are you sure you want to forget this quest?".into(), 28.0, INK), TextLayout::justify(Justify::Center))],
                    ),
                ],
            ))
            .id();
        for (yes, label, left) in [(true, "YES", 960.0 - 1.25 * 125.0), (false, "NO", 960.0 + 0.25 * 125.0)] {
            let b = commands
                .spawn((
                    Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, border_radius: BorderRadius::all(Val::Px(10.0)), ..at(Vec2::new(left, 540.0 + 30.0), Vec2::new(125.0, 60.0)) },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
                    Button,
                    Hovered::default(),
                    ShadedButton,
                    ConfirmButton(yes),
                    children![text(label.into(), 24.0, INK)],
                ))
                .id();
            commands.entity(dialog).add_child(b);
        }
        add(commands, 10, dialog);
    }
}

#[derive(Component)]
struct FolderLabel;

fn on_click(ev: On<Activate>, entries: Query<&QuestEntry>, folders: Query<&FolderButton>, forgets: Query<(), With<ForgetButton>>, confirms: Query<&ConfirmButton>, mut journal: ResMut<Journal>, mut forget: MessageWriter<ForgetQuest>) {
    let e = ev.entity;
    if let Ok(q) = entries.get(e) {
        journal.quest = Some(q.0.clone());
    } else if let Ok(f) = folders.get(e) {
        journal.log_tab = f.0;
    } else if forgets.contains(e) {
        journal.confirm_forget = true;
    } else if let Ok(c) = confirms.get(e) {
        journal.confirm_forget = false;
        if c.0
            && let Some(q) = journal.quest.take()
        {
            forget.write(ForgetQuest(q));
        }
    } else {
        return;
    }
    journal.dirty = true;
}

/// Quest buttons darken while hovered; ActiveText / CompletedText grow to 1.1 (their OnHovered).
#[allow(clippy::type_complexity)]
fn shade_buttons(
    mut shaded: Query<(&Hovered, &mut BackgroundColor), (With<ShadedButton>, Changed<Hovered>)>,
    folders: Query<(&Hovered, &Children), (With<FolderButton>, Changed<Hovered>)>,
    mut labels: Query<&mut UiTransform, With<FolderLabel>>,
    mut forget: Query<(&Hovered, &mut ImageNode), (With<ForgetButton>, Changed<Hovered>)>,
) {
    for (h, mut bg) in &mut shaded {
        bg.0 = Color::srgba(0.0, 0.0, 0.0, if h.get() { 0.5 } else { 0.25 });
    }
    for (h, kids) in &folders {
        for k in kids.iter() {
            if let Ok(mut t) = labels.get_mut(k) {
                t.scale = Vec2::splat(if h.get() { 1.1 } else { 1.0 });
            }
        }
    }
    // Button_ForgetQuest's Hovered brush: tinted 0.5.
    for (h, mut img) in &mut forget {
        img.color = if h.get() { Color::linear_rgb(0.5, 0.5, 0.5) } else { Color::WHITE };
    }
}

/// Rebuild the page when the quest log changes under it (a quest forgotten, finished...).
fn follow_log(game: Res<crate::menus::CurrentGame>, mut journal: ResMut<Journal>, mut seen: Local<Option<QuestLog>>) {
    let Some(log) = game.state.as_ref().map(|s| &s.quest_log) else { return };
    if seen.as_ref() == Some(log) {
        return;
    }
    *seen = Some(log.clone());
    if journal.open && journal.page == super::Page::Quests {
        if journal.quest.as_ref().is_none_or(|q| !log.started_or_finished(q)) {
            journal.quest = last_active(log);
        }
        journal.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::quests::QuestProgress;

    fn q(class: &str, outcome: Option<Outcome>) -> QuestProgress {
        QuestProgress { class: class.into(), state: String::new(), path: Vec::new(), tasks: Default::default(), outcome }
    }

    #[test]
    fn lists_put_failed_before_succeeded_and_count_only_succeeded() {
        let log = QuestLog { quests: vec![q("A", Some(Outcome::Succeeded)), q("B", None), q("C", Some(Outcome::Failed)), q("D", None)] };
        let (active, done, succeeded) = lists(&log);
        assert_eq!(active, ["B", "D"]);
        assert_eq!(done, ["C", "A"]);
        assert_eq!(succeeded, 1);
        assert_eq!(last_active(&log).as_deref(), Some("D"));
    }
}
