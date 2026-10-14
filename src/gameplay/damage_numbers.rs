//! Floating damage numbers: BFL_AI.SpawnFloatingDamageText + FloatingDamageNumbersUI (decompiled).
//!
//! - Only with DAMAGE NUMBERS ENABLED? (`Character.DamageNumbers`) and damage other than 0.
//! - The number (rounded) starts at the centre of the screen and eases towards a random offset
//!   (30-60 px left or right, 0-90 px up) with Vector2DInterpTo at speed 3; the Fade animation
//!   takes it from 0.75 opacity to 0 over 3 s, and it's removed after 2.5 s.
//! - Montserrat-ExtraBold 14, black shadow, in the caller's colour: firearms and melee
//!   (`CriticalHit`) gold on a crit, grey otherwise; the frag and pipe bomb white. Shoves, stomps,
//!   the Molotov and burning show none.

use bevy::prelude::*;
use rand::Rng;

use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::widgets::pt;
use crate::menus::AppState;

/// CriticalHit's FloatingDmgColour on a crit.
pub const CRIT: Color = Color::linear_rgb(0.796079, 0.588235, 0.109804);
/// CriticalHit's FloatingDmgColour otherwise.
pub const HIT: Color = Color::linear_rgb(0.74902, 0.74902, 0.74902);
/// The explosives' colour.
pub const BLAST: Color = Color::WHITE;

/// When the widget removes itself.
const LIFE: f32 = 2.5;
/// The Fade animation's length and starting opacity.
const FADE_SECS: f32 = 3.0;
const FADE_FROM: f32 = 0.75;
/// Vector2DInterpTo's speed.
const DRIFT_SPEED: f32 = 3.0;

#[derive(Message, Clone, Copy, Debug)]
pub struct DamageNumber {
    pub damage: f32,
    pub colour: Color,
}

impl DamageNumber {
    /// A weapon hit, coloured by whether it was a crit.
    pub fn hit(damage: f32, crit: bool) -> Self {
        Self { damage, colour: if crit { CRIT } else { HIT } }
    }
}

#[derive(Component)]
struct Floating {
    age: f32,
    pos: Vec2,
    target: Vec2,
    colour: Color,
}

pub fn plugin(app: &mut App) {
    app.add_message::<DamageNumber>().add_systems(Update, (spawn, drift).chain().run_if(in_state(AppState::InGame)));
}

/// The text the widget shows: the damage rounded to a whole number.
pub fn text(damage: f32) -> String {
    format!("{}", damage.round() as i64)
}

/// The random end offset (UMG px, y down).
pub fn final_offset(rng: &mut impl Rng) -> Vec2 {
    let x = if rng.gen_bool(0.5) { rng.gen_range(-60.0..=-30.0) } else { rng.gen_range(30.0..=60.0) };
    Vec2::new(x, rng.gen_range(-90.0..=0.0))
}

/// UE's Vector2DInterpTo.
pub fn interp_to(current: Vec2, target: Vec2, dt: f32, speed: f32) -> Vec2 {
    let d = target - current;
    if d.length_squared() < 1e-8 {
        return target;
    }
    current + d * (dt * speed).clamp(0.0, 1.0)
}

/// The Fade track: 0.75 -> 0 over 3 s, a cubic key pair with flat tangents.
pub fn opacity(age: f32) -> f32 {
    let x = (age / FADE_SECS).clamp(0.0, 1.0);
    FADE_FROM * (1.0 - x * x * (3.0 - 2.0 * x))
}

fn spawn(mut commands: Commands, mut msgs: MessageReader<DamageNumber>, store: Res<SettingsStore>, assets: Res<AssetServer>) {
    if msgs.is_empty() {
        return;
    }
    let on = store.find(&row_key("settings", "Gameplay", "DAMAGE NUMBERS ENABLED?")) != Some(Value::Bool(false));
    let mut rng = rand::thread_rng();
    for n in msgs.read() {
        if !on || n.damage == 0.0 {
            continue;
        }
        info!("damage number {}", text(n.damage));
        let font = TextFont { font: assets.load("fonts/Montserrat-ExtraBold.ttf").into(), font_size: pt(14.0), ..default() };
        commands.spawn((
            // The viewport, centring the text (the slot's 0.5 anchors and alignment).
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(55),
            DespawnOnExit(AppState::InGame),
            children![(
                Text::new(text(n.damage)),
                font,
                TextColor(n.colour.with_alpha(FADE_FROM)),
                TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK.with_alpha(FADE_FROM) },
                Node { position_type: PositionType::Relative, ..default() },
                Floating { age: 0.0, pos: Vec2::ZERO, target: final_offset(&mut rng), colour: n.colour },
            )],
        ));
    }
}

fn drift(mut commands: Commands, time: Res<Time>, mut texts: Query<(&ChildOf, &mut Floating, &mut Node, &mut TextColor, &mut TextShadow)>) {
    let dt = time.delta_secs();
    for (parent, mut f, mut node, mut colour, mut shadow) in &mut texts {
        f.age += dt;
        if f.age >= LIFE {
            commands.entity(parent.parent()).despawn();
            continue;
        }
        f.pos = interp_to(f.pos, f.target, dt, DRIFT_SPEED);
        node.left = Val::Px(f.pos.x);
        node.top = Val::Px(f.pos.y);
        let a = opacity(f.age) * f.colour.alpha();
        colour.0 = f.colour.with_alpha(a);
        shadow.color = Color::BLACK.with_alpha(a);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_rounded() {
        assert_eq!(text(42.4), "42");
        assert_eq!(text(42.5), "43");
        assert_eq!(text(115.0), "115");
    }

    #[test]
    fn they_drift_to_the_side_and_up() {
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let o = final_offset(&mut rng);
            assert!((30.0..=60.0).contains(&o.x.abs()) && (-90.0..=0.0).contains(&o.y), "{o}");
        }
        // InterpTo covers dt x speed of the gap each step, and never overshoots.
        let p = interp_to(Vec2::ZERO, Vec2::new(40.0, -60.0), 0.1, 3.0);
        assert!((p - Vec2::new(12.0, -18.0)).length() < 1e-4);
        assert_eq!(interp_to(Vec2::ZERO, Vec2::X * 10.0, 1.0, 3.0), Vec2::X * 10.0);
    }

    #[test]
    fn fade_runs_from_three_quarters_to_nothing() {
        assert_eq!(opacity(0.0), 0.75);
        assert!((opacity(1.5) - 0.375).abs() < 1e-6);
        assert_eq!(opacity(3.0), 0.0);
        // Still faintly there when the widget goes at 2.5 s.
        assert!(opacity(LIFE) > 0.0 && opacity(LIFE) < 0.1);
    }

    #[test]
    fn crits_are_gold() {
        assert_eq!(DamageNumber::hit(10.0, true).colour, CRIT);
        assert_eq!(DamageNumber::hit(10.0, false).colour, HIT);
    }
}
