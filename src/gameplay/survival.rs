//! The survivor's vitals while playing: `SurvivalStats` (in the session's `GameState`) ticks
//! hunger/thirst/stamina, zombies in reach hurt, consumables restore, and death respawns the
//! player at a PlayerStart.

use bevy::prelude::*;

use super::interact::GameOverlay;
use crate::level::{KinematicBody, Level, LevelZombie, Terrain};
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{CharacterModel, Locomotion};
use crate::survival::stats::SurvivalStats;

/// Ticking clock for each zombie's swings, and the interval to the next one.
#[derive(Component, Default)]
struct SwingTimer(f32, f32);

#[derive(Resource, Default)]
pub struct PlayerVitals {
    /// Seconds since death, while dead.
    dead_for: Option<f32>,
}

#[derive(Component)]
struct DeathBanner;

pub fn plugin(app: &mut App) {
    app.init_resource::<PlayerVitals>()
        .add_systems(OnEnter(AppState::InGame), |mut v: ResMut<PlayerVitals>| v.dead_for = None)
        .add_systems(
            Update,
            (tick_vitals, zombie_attacks, death_and_respawn)
                .chain()
                .run_if(in_state(AppState::InGame))
                .run_if(in_state(MenuScreen::None)),
        );
}

impl PlayerVitals {
    pub fn is_dead(&self) -> bool {
        self.dead_for.is_some()
    }
}

pub(crate) fn difficulty(store: &SettingsStore, page: &str, label: &str) -> f32 {
    match store.find(&row_key("difficulty", page, label)) {
        Some(Value::Num(v)) => v,
        _ => 1.0,
    }
}

fn tick_vitals(
    time: Res<Time>,
    mut game: ResMut<CurrentGame>,
    store: Res<SettingsStore>,
    vitals: Res<PlayerVitals>,
    player: Query<&Locomotion, With<crate::level::KinematicBody>>,
) {
    if vitals.is_dead() {
        return;
    }
    let Some(state) = game.state.as_mut() else { return };
    let sprinting = player.iter().any(|l| *l == Locomotion::Sprint);
    let s = &mut state.survival;
    let (h0, t0) = (s.hunger, s.thirst);
    s.tick(time.delta_secs(), sprinting);
    // MedicalComponent / RadiationComponent: bleeds, heavy bleeds, radiation sickness.
    let hurt = state.effects.tick(time.delta_secs(), s.radiation, &mut rand::thread_rng());
    s.health = (s.health - hurt).max(0.0);
    s.bleeding = state.effects.bleeding();
    // Difficulty > Player: depletion multipliers scale what this frame drained.
    s.hunger = h0 - (h0 - s.hunger) * difficulty(&store, "Player", "HUNGER DEPLETION MULTIPLIER");
    s.thirst = t0 - (t0 - s.thirst) * difficulty(&store, "Player", "THIRST DEPLETION MULTIPLIER");
}

#[allow(clippy::type_complexity)]
fn zombie_attacks(
    mut commands: Commands,
    time: Res<Time>,
    store: Res<SettingsStore>,
    mut game: ResMut<CurrentGame>,
    vitals: Res<PlayerVitals>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
    mut zombies: Query<(Entity, &Transform, &LevelZombie, &Locomotion, Option<&mut SwingTimer>), Without<super::combat::Dead>>,
) {
    let (Ok(p), Some(state)) = (player.single(), game.state.as_mut()) else { return };
    if vitals.is_dead() {
        return;
    }
    let mult = difficulty(&store, "AI", "ZOMBIE DAMAGE MULTIPLIER");
    for (e, t, z, loco, timer) in &mut zombies {
        if *loco != Locomotion::Attack || t.translation.distance(p.translation) > super::zombies::MELEE_RANGE + 0.3 {
            continue;
        }
        let Some(mut timer) = timer else {
            commands.entity(e).insert(SwingTimer::default());
            continue;
        };
        timer.0 += time.delta_secs();
        // The class's DamageToDo per hit, a swing every TimeBetweenAttacks.
        if timer.1 == 0.0 {
            let (lo, hi) = z.kind.attack_interval();
            timer.1 = rand::Rng::gen_range(&mut rand::thread_rng(), lo..hi);
        }
        if timer.0 >= timer.1 {
            timer.0 = 0.0;
            timer.1 = 0.0;
            let c = z.kind.class();
            state.survival.take_damage(c.damage * mult, false, 0.15);
            // `Medical Effects` rolls: Chance for Bleed / Heavy Bleed / Broken Bone (MedicalComponent
            // runs them), then Chance for Rad Damage adds Radiation Damage.
            let roll = |p: f32| rand::random::<f32>() < p;
            let (bleed, heavy, bone) = (roll(c.bleed), roll(c.heavy_bleed), roll(c.broken_bone));
            let mut rng = rand::thread_rng();
            if bleed {
                state.effects.add_bleed(&mut rng);
            }
            if heavy {
                state.effects.add_heavy_bleed(&mut rng);
            }
            if bone {
                state.effects.add_broken_bone(&mut rng);
            }
            state.survival.bleeding = state.effects.bleeding();
            if roll(c.rad_chance) {
                state.survival.radiation = (state.survival.radiation + c.rad_damage).min(100.0);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn death_and_respawn(
    mut commands: Commands,
    time: Res<Time>,
    mut game: ResMut<CurrentGame>,
    mut vitals: ResMut<PlayerVitals>,
    mut overlay: ResMut<GameOverlay>,
    store: Res<SettingsStore>,
    level: Option<Res<Level>>,
    terrain: Option<Res<Terrain>>,
    font: Res<crate::menus::widgets::MenuFont>,
    mut player: Query<(&mut Transform, &mut Locomotion), (With<CharacterModel>, With<KinematicBody>)>,
    banners: Query<Entity, With<DeathBanner>>,
) {
    let (Some(state), Ok((mut t, mut loco))) = (game.state.as_mut(), player.single_mut()) else { return };
    match vitals.dead_for {
        None if !state.survival.is_alive() => {
            vitals.dead_for = Some(0.0);
            *loco = Locomotion::Dead;
            *overlay = GameOverlay::None;
            commands.spawn((
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), top: Val::Percent(40.0), justify_content: JustifyContent::Center, ..default() },
                DeathBanner,
                DespawnOnExit(AppState::InGame),
                children![crate::menus::widgets::label(&font, "YOU DIED", 48.0)],
            ));
        }
        Some(d) => {
            let d = d + time.delta_secs();
            vitals.dead_for = Some(d);
            *loco = Locomotion::Dead;
            if d < 4.0 {
                return;
            }
            // Respawn at the PlayerStart furthest from where they fell, fresh vitals.
            let starts = level.map(|l| l.player_starts()).unwrap_or_default();
            let mut at = starts.iter().copied().max_by(|a, b| a.distance(t.translation).total_cmp(&b.distance(t.translation))).unwrap_or(t.translation);
            at.y = terrain.as_ref().map_or(at.y, |tr| tr.height(at.x, at.z));
            t.translation = at;
            state.survival = SurvivalStats::default();
            if store.find(&row_key("difficulty", "Player", "KEEP INVENTORY ON DEATH?")) != Some(Value::Bool(true)) {
                if let Some(inv) = game.inventory.as_mut() {
                    *inv = Default::default();
                }
            }
            vitals.dead_for = None;
            *loco = Locomotion::Idle;
            for b in &banners {
                commands.entity(b).despawn();
            }
        }
        None => {}
    }
}
