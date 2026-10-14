//! Being on fire (SetBurning), from BP_MasterZombie and BP_PlayerCharacter (decompiled). The
//! Molotov's fire sets whoever stands in it burning, unless they already are:
//!
//! - A zombie (`SetBurning(true, 4.0, player)`): after a 0.1-1 s Delay it catches (the
//!   BurningEffect particles), its Roaming/Alert/Attack Speed are divided by 1.5, and Event_Fire
//!   applies 4 damage every 0.8 s; 12-18 s later StopBurning puts the speeds back. Dying stops it.
//! - The survivor (`SetBurning(true, 3-5)`): Event_Fire applies that damage every 0.8 s
//!   (through CalculatePlayerDamage, so armor helps) for 10-12 s.
//!
//! Ours: a flickering light stands in for the burning particles; the hit montage, camera shake and
//! Toughness XP that each burn tick brings the survivor aren't ported.

use bevy::prelude::*;
use rand::Rng;

use super::combat::{Dead, Health};
use super::items::ItemDb;
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, CurrentGame, MenuScreen};

/// Event_Fire's timer period.
pub const TICK: f32 = 0.8;
/// Burning zombies' speeds are divided by this.
pub const SLOWDOWN: f32 = 1.5;

/// A burn under way: the countdown to catching, how long it lasts once it has, and the damage
/// per tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burn {
    pub delay: f32,
    pub left: f32,
    pub next_tick: f32,
    pub damage: f32,
}

impl Burn {
    /// A zombie set burning by a Molotov.
    pub fn zombie(rng: &mut impl Rng) -> Self {
        Self { delay: rng.gen_range(0.1..=1.0), left: rng.gen_range(12.0..=18.0), next_tick: TICK, damage: 4.0 }
    }

    /// The survivor set burning by a Molotov.
    pub fn player(rng: &mut impl Rng) -> Self {
        Self { delay: 0.0, left: rng.gen_range(10.0..=12.0), next_tick: TICK, damage: rng.gen_range(3.0..=5.0) }
    }

    /// Has it caught yet (the effect is on, speeds lowered)?
    pub fn alight(&self) -> bool {
        self.delay <= 0.0
    }

    /// Advance by `dt`: the damage ticks that fall in it, and whether it's still going.
    pub fn step(&mut self, dt: f32) -> (u32, bool) {
        let mut dt = dt;
        if self.delay > 0.0 {
            self.delay -= dt;
            if self.delay > 0.0 {
                return (0, true);
            }
            dt = -self.delay;
        }
        let run = dt.min(self.left);
        self.left -= dt;
        self.next_tick -= run;
        let mut ticks = 0;
        while self.next_tick <= 0.0 {
            ticks += 1;
            self.next_tick += TICK;
        }
        (ticks, self.left > 0.0)
    }
}

/// A burning zombie.
#[derive(Component, Debug)]
pub struct Burning(pub Burn);

/// The burning stand-in light on a zombie.
#[derive(Component)]
struct Flames;

/// The survivor's burn, if any.
#[derive(Resource, Default)]
pub struct PlayerBurning(pub Option<Burn>);

pub fn plugin(app: &mut App) {
    app.init_resource::<PlayerBurning>()
        .add_systems(OnEnter(AppState::InGame), |mut b: ResMut<PlayerBurning>| b.0 = None)
        .add_systems(Update, (burn_zombies, burn_player, flicker).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

/// The speed factor for a zombie that may be burning.
pub fn speed_factor(burning: Option<&Burning>) -> f32 {
    if burning.is_some_and(|b| b.0.alight()) { 1.0 / SLOWDOWN } else { 1.0 }
}

#[allow(clippy::type_complexity)]
fn burn_zombies(
    mut commands: Commands,
    time: Res<Time>,
    mut zombies: Query<(Entity, &mut Burning, &mut Health, Has<Dead>, Option<&Children>)>,
    flames: Query<(), With<Flames>>,
) {
    for (e, mut b, mut h, dead, children) in &mut zombies {
        let was_alight = b.0.alight();
        let (ticks, going) = if dead { (0, false) } else { b.0.step(time.delta_secs()) };
        h.hp -= ticks as f32 * b.0.damage;
        if !was_alight && b.0.alight() && going {
            commands.entity(e).with_child((
                Transform::from_xyz(0.0, 1.0, 0.0),
                PointLight { color: Color::srgb(1.0, 0.5, 0.15), intensity: 150_000.0, range: 6.0, ..default() },
                Flames,
            ));
            info!("zombie caught fire");
        }
        if !going {
            for c in children.into_iter().flatten().filter(|c| flames.contains(**c)) {
                commands.entity(*c).despawn();
            }
            commands.entity(e).remove::<Burning>();
            info!("zombie stopped burning (hp {:.0})", h.hp);
        }
    }
}

fn burn_player(time: Res<Time>, db: Res<ItemDb>, store: Res<SettingsStore>, mut burning: ResMut<PlayerBurning>, mut game: ResMut<CurrentGame>) {
    let Some(b) = burning.0.as_mut() else { return };
    let (ticks, going) = b.step(time.delta_secs());
    for _ in 0..ticks {
        let taken = super::survival::take_hit(&mut game, &db, &store, b.damage);
        if let Some(s) = game.state.as_mut() {
            s.survival.health = (s.survival.health - taken).max(0.0);
        }
        info!("burning: {taken:.1} damage");
    }
    let dead = game.state.as_ref().is_some_and(|s| s.survival.health <= 0.0);
    if !going || dead {
        burning.0 = None;
        info!("stopped burning");
    }
}

fn flicker(time: Res<Time>, mut flames: Query<&mut PointLight, With<Flames>>) {
    let t = time.elapsed_secs();
    for mut l in &mut flames {
        l.intensity = 150_000.0 * (0.75 + 0.25 * (t * 19.0).sin() * (t * 6.1).cos());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burns_tick_every_0_8_s_after_the_delay() {
        let mut b = Burn { delay: 0.5, left: 3.9, next_tick: TICK, damage: 4.0 };
        assert_eq!(b.step(0.4), (0, true));
        assert!(!b.alight());
        // Catches 0.1 s into this step; the first tick is 0.8 s after that.
        assert_eq!(b.step(0.8), (0, true));
        assert!(b.alight());
        assert_eq!(b.step(0.2), (1, true));
        // 3.9 s alight = ticks at 0.8, 1.6, 2.4 and 3.2 s, then it's out.
        let mut ticks = 1;
        loop {
            let (n, going) = b.step(0.1);
            ticks += n;
            if !going {
                break;
            }
        }
        assert_eq!(ticks, 4);
    }

    #[test]
    fn rolls_match_the_blueprints() {
        let mut rng = rand::thread_rng();
        for _ in 0..50 {
            let z = Burn::zombie(&mut rng);
            assert!((0.1..=1.0).contains(&z.delay) && (12.0..=18.0).contains(&z.left) && z.damage == 4.0);
            let p = Burn::player(&mut rng);
            assert!(p.alight() && (10.0..=12.0).contains(&p.left) && (3.0..=5.0).contains(&p.damage));
        }
    }

    #[test]
    fn burning_zombies_slow_down() {
        assert_eq!(speed_factor(None), 1.0);
        let mut b = Burn { delay: 0.3, left: 12.0, next_tick: TICK, damage: 4.0 };
        assert_eq!(speed_factor(Some(&Burning(b))), 1.0, "not alight yet");
        b.delay = 0.0;
        assert!((speed_factor(Some(&Burning(b))) - 1.0 / 1.5).abs() < 1e-6);
    }
}
