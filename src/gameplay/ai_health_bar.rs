//! The AI health bar: BP_PlayerCharacter's TraceForAI, HUD_Game's ToggleAIHealthBar /
//! UpdateHealth and `UI/Widgets/AI/AIHealthBarUI` (decompiled).
//!
//! - TraceForAI runs every 0.05 s while AI HEALTH BARS ENABLED? (Game.AIHealthBar, on by
//!   default): a 25 cm SphereTraceSingle 100 m along the camera's forward, the survivor ignored.
//!   A hit BPI_AI answers AIHealthBarInfo (Visible?, Name, Boss?), calling HUD.UpdateHealth
//!   (health, its maximum) first; then ToggleAIHealthBar(Visible?, Name, Boss?). Nothing hit, or
//!   not an AI: ToggleAIHealthBar(false).
//! - ToggleAIHealthBar(true) creates the bar (name, the skull for a boss) only when there's none;
//!   looking from one AI straight to another keeps the first one's name. (false) removes it.
//!   UpdateHealth only reaches a bar that exists, so a new bar shows its health from the next
//!   trace (until then: an empty text, the bar full).
//! - The answers: zombies their Name and DamageComponent (current, StandardHealth), not while
//!   dead; BP_ZombieBoss the same with AIName and the skull; the deer "Deer" (Health, 100), the
//!   wolf "Wolf" (Health, 200); the bandit boss (Health, MaxHealth) with the skull, the Coyote
//!   lieutenant the same without; infestations their name (current, StandardHealth), the
//!   skull for a primary, not once destroyed. Other humans, bears and rabbits: no bar.
//! - AIHealthBarUI: top centre, under itself (Alignment Y -1); a 25 % black rounded box
//!   (radius 10, padding 5): the skull (28 px, dark red) and the name (Montserrat-Bold 18,
//!   0.85 grey); under them a 320 x 12 bar (dark red on 50 % black) with "{Health} / {MaxHealth}"
//!   (whole numbers, grouped, the health clamped to 0..max) in Montserrat-Bold 8.
//!
//! Ours: the trace is a ray (the shots' hit shapes, the nests' boxes; props block it); the bar
//! is part of the HUD layer (hidden with it under menus).

use bevy::prelude::*;

use super::combat::{Dead, Health};
use super::infestations::Infestations;
use crate::level::LevelZombie;
use crate::level::collision::PropRays;
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::widgets::{MenuFont, pt};
use crate::menus::{AppState, MenuScreen};

/// TraceForAI's timer.
const EVERY: f32 = 0.05;
/// The trace's length (10000 cm).
const REACH: f32 = 100.0;
const SKULL: &str = "game/SurrounDead/Content/UI/Textures/Survival-UI-Bundle/Rhombus/512x512/Skull.png";
/// The skull's tint and the bar's fill: (0.140625, 0, 0) linear.
const RED: Color = Color::linear_rgb(0.140625, 0.0, 0.0);

/// What AIHealthBarInfo answers: (name, boss, health, the bar's maximum); None = Visible? false.
#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    pub name: String,
    pub boss: bool,
    pub health: f32,
    pub max: f32,
}

/// The HUD's bar: shown (name, boss) and the health UpdateHealth last gave it.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct AiBar {
    pub shown: Option<(String, bool)>,
    pub health: Option<(f32, f32)>,
}

impl AiBar {
    /// One TraceForAI: `hit` is what the trace found (None: nothing, or no bar wanted).
    pub fn trace(&mut self, hit: Option<Info>) {
        let Some(info) = hit else {
            *self = Self::default();
            return;
        };
        // UpdateHealth first: only a bar that's already there takes it.
        if self.shown.is_some() {
            self.health = Some((info.health, info.max));
        }
        if self.shown.is_none() {
            self.shown = Some((info.name, info.boss));
        }
    }
}

/// A zombie class's answer (`health`: current and maximum as its Health holds them).
pub fn zombie_info(kind: super::zombies::ZombieKind, (hp, max): (f32, f32), dead: bool) -> Option<Info> {
    if dead {
        return None;
    }
    let name = kind.class().name.clone().unwrap_or_default();
    let (boss, max) = match kind.class_name() {
        "BP_AnimalDeer_C" => (false, 100.0),
        "BP_AnimalWolf_C" => (false, 200.0),
        "BP_Bandit_Ranged_Boss_C" => (true, max.round()),
        "BP_Military_Boss_C" => (false, max.round()),
        _ if kind.animal().is_some() || kind.human().is_some() => return None,
        _ => (kind.boss().is_some(), max),
    };
    let hp = if kind.human().is_some() { hp.round() } else { hp };
    Some(Info { name, boss, health: hp, max })
}

/// "{Health} / {MaxHealth}": Conv_DoubleToText (half to even, grouped), the health clamped.
pub fn bar_text(health: f32, max: f32) -> String {
    format!("{} / {}", grouped(health.clamp(0.0, max.max(0.0))), grouped(max))
}

fn grouped(v: f32) -> String {
    let n = v.round_ties_even() as i64;
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

fn enabled(store: &SettingsStore) -> bool {
    store.find(&row_key("settings", "Gameplay", "AI HEALTH BARS ENABLED? (RESTART REQUIRED)")) != Some(Value::Bool(false))
}

/// TraceForAI.
#[allow(clippy::too_many_arguments)]
fn trace_for_ai(
    time: Res<Time>,
    store: Res<SettingsStore>,
    inf: Option<Res<Infestations>>,
    mut bar: ResMut<AiBar>,
    mut rays: PropRays,
    camera: Query<&GlobalTransform, With<super::PlayerCamera>>,
    zombies: Query<(&Transform, &LevelZombie, Option<&Health>, Has<Dead>)>,
    mut wait: Local<f32>,
) {
    *wait -= time.delta_secs();
    if *wait > 0.0 || !enabled(&store) {
        return;
    }
    *wait += EVERY;
    let Ok(cam) = camera.single() else { return };
    let (origin, dir) = (cam.translation(), cam.forward().as_vec3());
    let wall = rays.cast(origin, dir, REACH).unwrap_or(REACH);
    let mut best: Option<(f32, Option<Info>)> = None;
    let mut consider = |d: f32, info: Option<Info>| {
        if d <= wall && best.as_ref().is_none_or(|b| d < b.0) {
            best = Some((d, info));
        }
    };
    for (t, z, health, dead) in &zombies {
        if let Some((d, _)) = super::combat::ray_hits(origin, dir, t, z.kind) {
            let h = health.map_or((0.0, 0.0), |h| (h.hp, h.max));
            consider(d, zombie_info(z.kind, h, dead));
        }
    }
    if let Some(inf) = &inf {
        for n in inf.nests.iter().filter(|n| !n.gone) {
            let Some(b) = n.bounds.filter(|b| super::infestations::ray_box(origin, dir, REACH, *b)) else { continue };
            let info = (!n.destroyed).then(|| Info { name: n.def().map_or_else(|| "Infestation".into(), |d| d.name.clone()), boss: n.primary, health: n.health, max: n.standard });
            consider(super::infestations::box_distance(origin, b), info);
        }
    }
    let before = bar.shown.clone();
    bar.trace(best.and_then(|b| b.1));
    if bar.shown != before {
        match &bar.shown {
            Some((name, boss)) => info!("ai health bar: {name} (boss {boss})"),
            None => info!("ai health bar: hidden"),
        }
    }
}

#[derive(Component)]
struct BarRoot;
#[derive(Component)]
struct BarName;
#[derive(Component)]
struct BarSkull;
#[derive(Component)]
struct BarFill;
#[derive(Component)]
struct BarText;

/// A TextBlock without a shadow (Name's ShadowOffset 0; HealthText's shadow colour transparent).
fn text(font: &MenuFont, size: f32, color: Color) -> impl Bundle {
    (Text::new(""), TextFont { font: font.0.clone().into(), font_size: pt(size), ..default() }, TextColor(color))
}

fn spawn_bar(mut commands: Commands, font: Res<MenuFont>, assets: Res<AssetServer>, mut bar: ResMut<AiBar>) {
    *bar = AiBar::default();
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Px(0.0), padding: UiRect::all(Val::Px(5.0)), flex_direction: FlexDirection::Column, border_radius: BorderRadius::all(Val::Px(10.0)), ..default() },
        // Alignment (0.5, -1): centred, its own height down.
        UiTransform::from_translation(Val2::percent(-50.0, 100.0)),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
        Visibility::Hidden,
        bevy::picking::Pickable::IGNORE,
        BarRoot,
        DespawnOnExit(AppState::InGame),
        children![
            (
                Node { align_self: AlignSelf::Center, align_items: AlignItems::Center, ..default() },
                children![
                    (Node { width: Val::Px(28.0), height: Val::Px(28.0), margin: UiRect::all(Val::Px(5.0)), ..default() }, ImageNode::new(assets.load(SKULL)).with_color(RED), BarSkull),
                    (text(&font, 18.0, Color::linear_rgb(0.85, 0.85, 0.85)), BarName),
                ],
            ),
            (
                Node { width: Val::Px(320.0), height: Val::Px(12.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                children![
                    (Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(RED), BarFill),
                    (text(&font, 8.0, Color::WHITE), BarText),
                ],
            ),
        ],
    ));
}

#[allow(clippy::type_complexity)]
fn draw_bar(
    bar: Res<AiBar>,
    overlay: Res<super::GameOverlay>,
    screen: Res<State<MenuScreen>>,
    mut root: Query<&mut Visibility, With<BarRoot>>,
    mut skull: Query<&mut Node, (With<BarSkull>, Without<BarFill>)>,
    mut fill: Query<&mut Node, (With<BarFill>, Without<BarSkull>)>,
    mut texts: Query<(&mut Text, Has<BarName>), Or<(With<BarName>, With<BarText>)>>,
) {
    let Ok(mut vis) = root.single_mut() else { return };
    let hud = !overlay.is_open() && *screen.get() == MenuScreen::None;
    vis.set_if_neq(if bar.shown.is_some() && hud { Visibility::Inherited } else { Visibility::Hidden });
    if !bar.is_changed() {
        return;
    }
    let Some((name, boss)) = &bar.shown else { return };
    for mut n in &mut skull {
        let d = if *boss { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    // TargetPercent starts at 1; the text empty until the first UpdateHealth.
    let (pct, text) = bar.health.map_or((1.0, String::new()), |(h, m)| ((h / m.max(1e-6)).clamp(0.0, 1.0), bar_text(h, m)));
    for mut n in &mut fill {
        n.width = Val::Percent(pct * 100.0);
    }
    for (mut t, is_name) in &mut texts {
        let want = if is_name { name.as_str() } else { text.as_str() };
        if t.0 != want {
            t.0 = want.to_string();
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<AiBar>()
        .add_systems(OnEnter(AppState::InGame), spawn_bar)
        .add_systems(Update, (trace_for_ai, draw_bar).chain().run_if(in_state(AppState::InGame)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, boss: bool, health: f32) -> Info {
        Info { name: name.into(), boss, health, max: 100.0 }
    }

    #[test]
    fn the_bar_keeps_its_first_name_and_takes_health_a_trace_late() {
        let mut bar = AiBar::default();
        bar.trace(Some(info("Undead Shambler", false, 80.0)));
        assert_eq!(bar, AiBar { shown: Some(("Undead Shambler".into(), false)), health: None });
        bar.trace(Some(info("Undead Shambler", false, 70.0)));
        assert_eq!(bar.health, Some((70.0, 100.0)));
        // Straight onto another AI: its health, the old name.
        bar.trace(Some(info("Deer", false, 40.0)));
        assert_eq!(bar.shown, Some(("Undead Shambler".into(), false)));
        assert_eq!(bar.health, Some((40.0, 100.0)));
        bar.trace(None);
        assert_eq!(bar, AiBar::default());
    }

    #[test]
    fn health_text_groups_and_clamps() {
        assert_eq!(bar_text(1750.4, 1750.0), "1,750 / 1,750");
        assert_eq!(bar_text(-20.0, 2500.0), "0 / 2,500");
        assert_eq!(bar_text(62.5, 100.0), "62 / 100");
        assert_eq!(bar_text(63.5, 100.0), "64 / 100");
    }

    #[test]
    fn each_ai_answers_as_its_blueprint_does() {
        use super::super::zombies::ZombieKind;
        let k = |c: &str| ZombieKind::from_class(c);
        let Some(roamer) = k("BP_Zombie_Roamer_C") else { return };
        assert_eq!(zombie_info(roamer, (80.0, 105.0), false), Some(Info { name: "Undead Shambler".into(), boss: false, health: 80.0, max: 105.0 }));
        assert_eq!(zombie_info(roamer, (0.0, 105.0), true), None, "dead");
        // The wolf's bar runs to 200 whatever its health (150).
        assert_eq!(zombie_info(k("BP_AnimalWolf_C").unwrap(), (150.0, 150.0), false).map(|i| (i.name, i.max)), Some(("Wolf".into(), 200.0)));
        assert!(zombie_info(k("BP_ZombieBoss_Juggernaut_C").unwrap(), (4000.0, 4000.0), false).unwrap().boss);
        assert!(zombie_info(k("BP_Bandit_Ranged_Boss_C").unwrap(), (100.0, 100.0), false).unwrap().boss);
        assert!(!zombie_info(k("BP_Military_Boss_C").unwrap(), (100.0, 100.0), false).unwrap().boss);
        assert_eq!(zombie_info(k("BP_Bandit_Melee_C").unwrap(), (100.0, 100.0), false), None);
        assert_eq!(zombie_info(k("BP_AnimalBear_C").unwrap(), (500.0, 500.0), false), None);
    }
}
