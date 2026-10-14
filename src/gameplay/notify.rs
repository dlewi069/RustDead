//! Short on-screen messages (`GameFunctionLibrary.CreateNotificationUI`: "Lockpick broke",
//! "Advanced lock, unlock Lockpick+ skill to picklock"...). The icon isn't shown.

use bevy::prelude::*;

use crate::menus::AppState;
use crate::menus::widgets::{MenuFont, label_colored};

/// Show `text` for `secs` seconds.
#[derive(Message, Clone)]
pub struct Notify {
    pub text: String,
    pub secs: f32,
}

impl Notify {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), secs: 2.0 }
    }
}

#[derive(Component)]
struct Toast(f32);

pub fn plugin(app: &mut App) {
    app.add_message::<Notify>().add_systems(Update, (announce_level_ups, show, expire).chain().run_if(in_state(AppState::InGame)));
}

/// PassiveSkillsComponent.LevelUpNotification: "Your '{Skill}' skill is now level {Lvl}!" for 6 s.
fn announce_level_ups(mut game: ResMut<crate::menus::CurrentGame>, mut notify: MessageWriter<Notify>) {
    let Some(state) = game.state.as_mut() else { return };
    // LevellingComponent.LevelUp's two notifications.
    for level in std::mem::take(&mut state.progression.level_ups) {
        info!("Level up! You're now level {level}!");
        notify.write(Notify::new(format!("Level up! You're now level {level}!")));
        notify.write(Notify::new("+1 skill point"));
    }
    if state.passive.level_ups.is_empty() {
        return;
    }
    for (skill, level) in std::mem::take(&mut state.passive.level_ups) {
        let text = crate::progression::passive::PassiveSkills::level_up_message(skill, level);
        info!("{text}");
        notify.write(Notify { text, secs: 6.0 });
    }
}

fn show(mut commands: Commands, mut msgs: MessageReader<Notify>, font: Res<MenuFont>, old: Query<Entity, With<Toast>>) {
    // Everything sent this frame shows together (a level-up's "+1 skill point" under its line).
    let mut new: Vec<Notify> = Vec::new();
    for n in msgs.read() {
        if !new.iter().any(|m| m.text == n.text) {
            new.push(n.clone());
        }
    }
    if new.is_empty() {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let secs = new.iter().map(|n| n.secs).fold(0.0, f32::max);
    let root = commands
        .spawn((
            Node { position_type: PositionType::Absolute, top: Val::Percent(18.0), width: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(4.0), ..default() },
            GlobalZIndex(60),
            Toast(secs),
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    for n in new {
        let line = commands
            .spawn((
                Node { padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)), ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                children![label_colored(&font, n.text, 16.0, Color::WHITE)],
            ))
            .id();
        commands.entity(root).add_child(line);
    }
}

fn expire(mut commands: Commands, time: Res<Time>, mut toasts: Query<(Entity, &mut Toast)>) {
    for (e, mut t) in &mut toasts {
        t.0 -= time.delta_secs();
        if t.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}
