//! Floating damage numbers: BFL_AI.SpawnFloatingDamageText + FloatingDamageNumbersUI (decompiled).
//!
//! - Only with DAMAGE NUMBERS ENABLED? (`Character.DamageNumbers`) and damage other than 0.
//! - The number (rounded) starts at the centre of the screen and eases towards a random offset
//!   (30-60 px left or right, 0-90 px up) with Vector2DInterpTo at speed 3; the Fade animation
//!   takes it from 0.75 opacity to 0 over 3 s, and it's removed after 2.5 s.
//! - Montserrat-ExtraBold 14, black shadow, in the caller's colour: firearms and melee
//!   (`CriticalHit`) gold on a crit, grey otherwise; the frag and pipe bomb, explosive barrels
//!   (`barrels.rs`) and the traps (`traps.rs`) white. The game's only other callers are the
//!   buildable turret and the nuke easter egg (not ported); shoves, stomps, the Molotov,
//!   burning and vehicles show none (their Blueprints never call SpawnFloatingDamageText).
//!
//! Ours (at the user's request, 2026-10-30): the number starts on what was damaged instead of
//! the screen centre (the widget is only AddToViewport'ed in the game): each message carries the
//! world point (`body`: the upper chest of a pawn, else the hit), projected through the player
//! camera every frame, and the drift is an offset from it; hidden while it's behind the camera.

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
    /// Where it was dealt (world space).
    pub at: Vec3,
}

impl DamageNumber {
    /// A weapon hit, coloured by whether it was a crit.
    pub fn hit(damage: f32, crit: bool, at: Vec3) -> Self {
        Self { damage, colour: if crit { CRIT } else { HIT }, at }
    }

    /// An explosive's or trap's (white).
    pub fn blast(damage: f32, at: Vec3) -> Self {
        Self { damage, colour: BLAST, at }
    }
}

/// Where a number goes on a pawn: the upper chest of a capsule standing at `feet`.
pub fn body(feet: Vec3, height: f32) -> Vec3 {
    feet + Vec3::Y * height * 0.8
}

/// A number on screen (its box).
#[derive(Component)]
pub struct Floating {
    age: f32,
    /// The drift (UMG px from the projected point).
    pos: Vec2,
    target: Vec2,
    colour: Color,
    at: Vec3,
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
            // A zero-size box at the projected point, centring the text on it (the slot's 0.5
            // alignment); `drift` places it before it's first drawn.
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
            GlobalZIndex(55),
            DespawnOnExit(AppState::InGame),
            Floating { age: 0.0, pos: Vec2::ZERO, target: final_offset(&mut rng), colour: n.colour, at: n.at },
            children![(
                Text::new(text(n.damage)),
                font,
                TextColor(n.colour.with_alpha(FADE_FROM)),
                TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK.with_alpha(FADE_FROM) },
                bevy::text::TextLayout::linebreak(bevy::text::LineBreak::NoWrap),
            )],
        ));
    }
}

/// Where the number's box goes (UI px): the point's viewport position (logical px) over the UI
/// scale, plus the drift. None behind the camera.
pub fn screen_spot(viewport: Option<Vec2>, ui_scale: f32, drift: Vec2) -> Option<Vec2> {
    viewport.map(|v| v / ui_scale.max(1e-3) + drift)
}

#[allow(clippy::type_complexity)]
fn drift(
    mut commands: Commands,
    time: Res<Time>,
    ui_scale: Res<UiScale>,
    camera: Query<(&Camera, &GlobalTransform), With<super::PlayerCamera>>,
    mut boxes: Query<(Entity, &mut Floating, &mut Node, &mut Visibility, &Children)>,
    mut texts: Query<(&mut TextColor, &mut TextShadow)>,
) {
    let dt = time.delta_secs();
    let cam = camera.single().ok();
    for (e, mut f, mut node, mut vis, kids) in &mut boxes {
        f.age += dt;
        if f.age >= LIFE {
            commands.entity(e).despawn();
            continue;
        }
        let first = f.age == dt;
        f.pos = interp_to(f.pos, f.target, dt, DRIFT_SPEED);
        let viewport = cam.and_then(|(c, t)| c.world_to_viewport(t, f.at).ok());
        match screen_spot(viewport, ui_scale.0, f.pos) {
            Some(p) => {
                if first && let Some(size) = cam.and_then(|(c, _)| c.logical_viewport_size()) {
                    let size = size / ui_scale.0.max(1e-3);
                    info!("damage number placed {:.0} {:.0} of {:.0} {:.0}", p.x, p.y, size.x, size.y);
                }
                node.left = Val::Px(p.x);
                node.top = Val::Px(p.y);
                vis.set_if_neq(Visibility::Inherited);
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
        let a = opacity(f.age) * f.colour.alpha();
        for &k in kids {
            if let Ok((mut colour, mut shadow)) = texts.get_mut(k) {
                colour.0 = f.colour.with_alpha(a);
                shadow.color = Color::BLACK.with_alpha(a);
            }
        }
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
        assert_eq!(DamageNumber::hit(10.0, true, Vec3::ZERO).colour, CRIT);
        assert_eq!(DamageNumber::hit(10.0, false, Vec3::ZERO).colour, HIT);
        assert_eq!(DamageNumber::blast(10.0, Vec3::ZERO).colour, BLAST);
    }

    #[test]
    fn numbers_sit_on_what_was_hit() {
        // A zombie projected at (1200, 400) logical px with UiScale 0.75 is at (1600, 533) UI px,
        // and the drift moves it from there; behind the camera nothing is drawn.
        let p = screen_spot(Some(Vec2::new(1200.0, 400.0)), 0.75, Vec2::new(40.0, -30.0)).unwrap();
        assert!((p - Vec2::new(1640.0, 533.333 - 30.0)).length() < 0.01, "{p}");
        assert_eq!(screen_spot(None, 1.0, Vec2::ZERO), None);
        assert_eq!(body(Vec3::new(1.0, 2.0, 3.0), 1.8), Vec3::new(1.0, 2.0 + 1.44, 3.0));
    }
}
