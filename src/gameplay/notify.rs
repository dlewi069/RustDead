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
    app.add_message::<Notify>().add_systems(Update, (show, expire).chain().run_if(in_state(AppState::InGame)));
}

fn show(mut commands: Commands, mut msgs: MessageReader<Notify>, font: Res<MenuFont>, old: Query<Entity, With<Toast>>) {
    let Some(n) = msgs.read().last() else { return };
    for e in &old {
        commands.entity(e).despawn();
    }
    commands.spawn((
        Node { position_type: PositionType::Absolute, top: Val::Percent(18.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
        GlobalZIndex(60),
        Toast(n.secs),
        DespawnOnExit(AppState::InGame),
        children![(
            Node { padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            children![label_colored(&font, n.text.clone(), 16.0, Color::WHITE)],
        )],
    ));
}

fn expire(mut commands: Commands, time: Res<Time>, mut toasts: Query<(Entity, &mut Toast)>) {
    for (e, mut t) in &mut toasts {
        t.0 -= time.delta_secs();
        if t.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}
