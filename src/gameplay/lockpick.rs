//! The lockpicking minigame on `PicklockContainer_*` (safes, military lockers), decompiled from
//! `UI/Lockpick/LockpickUI`, `LockWidget`, `BP_LockPickObject` and `LockPickingComponent`:
//!
//! - Each lock rolls a difficulty (Novice / Standard / Hardened / Master) and a sweet spot that
//!   starts at a random 0-180° and is 12 / 7 / 4 / 2° wide. Hardened and Master locks need the
//!   Lockpick+ skill (`AdvancedLockCheck`).
//! - Mouse X turns the pick (cursor delta / 5, clamped 0-180°). Holding W/A/S/D turns the lock
//!   toward 90° minus the pick's distance from the sweet spot (all the way inside it), at
//!   130°/s; reaching 90° picks it.
//! - Forcing it outside the sweet spot jitters the pick and, after 0.5 s, wears it 1.675 every
//!   0.1 s from 25: at zero the lockpick breaks (one `DA_Lockpick` used up).
//! - Success: two seconds, then the container opens. Esc leaves.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use rand::Rng;

use super::interact::{Containers, GameOverlay, player_inv};
use super::inventory_ui::Dirty;
use super::notify::Notify;
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame};

pub const LOCKPICK: &str = "DA_Lockpick";
const PICK_DURABILITY: f32 = 25.0;
const WEAR: f32 = 1.675;
const WEAR_EVERY: f32 = 0.1;
const WEAR_DELAY: f32 = 0.5;
const LOCK_SPEED: f32 = 130.0;
const SUCCESS_WAIT: f32 = 2.0;
const TEX: &str = "game/SurrounDead/Content/UI/Lockpick/Textures";

/// `Enum_LockpickDifficulty`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Novice,
    Standard,
    Hardened,
    Master,
}

impl Difficulty {
    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Novice => "NOVICE LOCK",
            Difficulty::Standard => "STANDARD LOCK",
            Difficulty::Hardened => "HARDENED LOCK",
            Difficulty::Master => "MASTER LOCK",
        }
    }

    /// Sweet spot width in degrees (BP_LockPickObject's select: 12, 7, 4, 2).
    pub fn width(self) -> f32 {
        match self {
            Difficulty::Novice => 12.0,
            Difficulty::Standard => 7.0,
            Difficulty::Hardened => 4.0,
            Difficulty::Master => 2.0,
        }
    }

    pub fn advanced(self) -> bool {
        matches!(self, Difficulty::Hardened | Difficulty::Master)
    }
}

/// A container's lock, rolled once per session.
#[derive(Clone, Copy, Debug)]
pub struct Lock {
    pub difficulty: Difficulty,
    /// `LockpickRange`: the sweet spot's start and end angle.
    pub range: (f32, f32),
}

impl Lock {
    pub fn roll(rng: &mut impl Rng) -> Self {
        let difficulty = [Difficulty::Novice, Difficulty::Standard, Difficulty::Hardened, Difficulty::Master][rng.gen_range(0..4)];
        let start = rng.gen_range(0.0..180.0);
        Lock { difficulty, range: (start, start + difficulty.width()) }
    }

    /// How far the lock can turn with the pick at `pin` degrees.
    pub fn max_turn(&self, pin: f32) -> f32 {
        let (a, b) = self.range;
        if (a..=b).contains(&pin) {
            90.0
        } else {
            90.0 - (pin - a).abs().min((pin - b).abs()).clamp(0.0, 90.0)
        }
    }

    pub fn in_spot(&self, pin: f32) -> bool {
        (self.range.0..=self.range.1).contains(&pin)
    }
}

/// The minigame in progress (`LockpickUI` + the player's `LockPickingComponent`).
#[derive(Resource, Default)]
struct Picking {
    actor: Option<usize>,
    pin: f32,
    lock: f32,
    /// Pick wear left (`LPDurability`), kept between attempts.
    durability: f32,
    /// Seconds of forcing (wear starts after `WEAR_DELAY`, then every `WEAR_EVERY`).
    forcing: f32,
    worn: u32,
    success: Option<f32>,
}

#[derive(Component)]
struct LockpickUi;
#[derive(Component)]
struct Pin;
#[derive(Component)]
struct MovingLock;
#[derive(Component)]
struct PickCount;

pub fn plugin(app: &mut App) {
    app.init_resource::<Picking>().add_systems(Update, (open_close, play).chain().run_if(in_state(AppState::InGame)));
}

/// Whether the survivor may try this lock (lockpicks, skill). Returns the refusal message.
pub fn can_try(game: &mut CurrentGame, lock: &Lock) -> Result<(), &'static str> {
    if player_inv(game).count(LOCKPICK) == 0 {
        return Err("You need a lockpick");
    }
    // The tech tree's Lockpick+.
    let skilled = game.state.as_ref().is_some_and(|s| s.tech.has(crate::progression::tech::LOCKPICK));
    if lock.difficulty.advanced() && !skilled {
        return Err("Advanced lock, unlock Lockpick+ skill to picklock");
    }
    Ok(())
}

fn open_close(mut commands: Commands, overlay: Res<GameOverlay>, mut picking: ResMut<Picking>, containers: Res<Containers>, font: Res<MenuFont>, assets: Res<AssetServer>, uis: Query<Entity, With<LockpickUi>>) {
    let want = match *overlay {
        GameOverlay::Lockpick(a) => Some(a),
        _ => None,
    };
    if want == picking.actor {
        return;
    }
    for e in &uis {
        commands.entity(e).despawn();
    }
    let durability = if picking.durability > 0.0 { picking.durability } else { PICK_DURABILITY };
    *picking = Picking { actor: want, durability, ..default() };
    let Some(lock) = want.and_then(|a| containers.locks.get(&a)) else { return };
    spawn_ui(&mut commands, &font, &assets, lock);
}

fn spawn_ui(commands: &mut Commands, font: &MenuFont, assets: &AssetServer, lock: &Lock) {
    let img = |p: &str| ImageNode::new(assets.load(format!("{TEX}/{p}.png")));
    // A `size` box centred on the parent's centre plus `offset` (CanvasPanelSlot, alignment 0.5).
    let centred = |size: f32, dx: f32, dy: f32| Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Percent(50.0),
        width: Val::Px(size),
        height: Val::Px(size),
        margin: UiRect::new(Val::Px(-size / 2.0 + dx), Val::ZERO, Val::Px(-size / 2.0 + dy), Val::ZERO),
        ..default()
    };
    let bottom_text = |text: String, size: f32, lines_up: f32| {
        (
            Node { position_type: PositionType::Absolute, bottom: Val::Px((lines_up - 1.0) * size * 1.25), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
            children![label_colored(font, text, size, Color::WHITE)],
        )
    };
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        // BackgroundBlur 15.
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        GlobalZIndex(40),
        LockpickUi,
        DespawnOnExit(AppState::InGame),
        children![
            (centred(512.0, 0.0, 0.0), img("LockStatic")),
            (
                // LockWidget: the lock cylinder (backdrop + face), the pick, the screwdriver. The
                // lock turns the whole widget (`LockWidget.SetRenderTransformAngle`), so the pick
                // and screwdriver turn with the cylinder.
                centred(512.0, 0.0, 0.0),
                UiTransform::default(),
                MovingLock,
                children![
                    // MovingLock's images are 1024 brushes centred in a 512 overlay; the three lock
                    // textures share one scale (the backdrop fills LockStatic's hole exactly), so
                    // they're drawn at LockStatic's 512.
                    (centred(512.0, 0.0, 0.0), children![(centred(512.0, 0.0, 0.0), img("LockBackdrop")), (centred(512.0, 0.0, 0.0), img("LockMobile"))]),
                    (centred(1024.0, 0.0, -63.0), UiTransform::default(), Pin, img("Picks/LP01Mobile")),
                    (centred(1024.0, 35.0, 495.0), img("Screwdriver")),
                ],
            ),
            bottom_text(lock.difficulty.name().to_string(), 12.0, 9.5),
            (bottom_text(String::new(), 24.0, 4.0), PickCount),
            bottom_text("[Esc] Exit".to_string(), 18.0, 2.0),
        ],
    ));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn play(
    time: Res<Time>,
    motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<GameOverlay>,
    mut picking: ResMut<Picking>,
    mut containers: ResMut<Containers>,
    mut game: ResMut<CurrentGame>,
    mut notify: MessageWriter<Notify>,
    mut dirty: ResMut<Dirty>,
    mut pins: Query<(&mut UiTransform, &mut Visibility), (With<Pin>, Without<MovingLock>)>,
    mut locks: Query<&mut UiTransform, (With<MovingLock>, Without<Pin>)>,
    counts: Query<&Children, With<PickCount>>,
    mut texts: Query<&mut Text>,
) {
    let Some(actor) = picking.actor else { return };
    let Some(lock) = containers.locks.get(&actor).copied() else { return };
    let dt = time.delta_secs();
    let picks = player_inv(&mut game).count(LOCKPICK);
    for kids in &counts {
        for k in kids.iter() {
            if let Ok(mut t) = texts.get_mut(k) {
                t.0 = format!("Lockpicks: {picks}");
            }
        }
    }
    if let Some(waited) = picking.success.as_mut() {
        *waited += dt;
        if *waited >= SUCCESS_WAIT {
            // LockpickSuccessful -> OpenContainer. BP_MASTER_PicklockContainer: AddXP_Scavenging
            // (4-6) and AddXP_Thief (40-60); BP_LockedDoor: AddXP_Thief (20-30).
            if containers.unlocked.insert(actor)
                && let Some(st) = game.state.as_mut()
            {
                use crate::progression::passive::Passive;
                let mut rng = rand::thread_rng();
                if containers.list.iter().any(|c| c.0 == actor) {
                    st.passive.add_xp(Passive::Scavenging, rng.gen_range(4.0..=6.0));
                    st.passive.add_xp(Passive::Thief, rng.gen_range(40.0..=60.0));
                } else {
                    st.passive.add_xp(Passive::Thief, rng.gen_range(20.0..=30.0));
                }
            }
            *overlay = GameOverlay::None;
            containers.open_now = Some(actor);
            dirty.0 = true;
        }
        return;
    }
    if picks == 0 {
        // Out of lockpicks: the lock springs back and the pick is gone (Pin opacity 0).
        picking.lock = 0.0;
        picking.forcing = 0.0;
        for (mut t, mut v) in &mut pins {
            t.rotation = Rot2::degrees(picking.pin);
            *v = Visibility::Hidden;
        }
        for mut t in &mut locks {
            t.rotation = Rot2::IDENTITY;
        }
        return;
    }
    let trying = keys.any_pressed([KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD]);
    if !trying {
        picking.pin = (picking.pin + motion.delta.x / 5.0).clamp(0.0, 180.0);
    }
    let target = if trying { lock.max_turn(picking.pin) } else { 0.0 };
    let step = LOCK_SPEED * dt;
    picking.lock = if picking.lock < target { (picking.lock + step).min(target) } else { (picking.lock - step).max(target) };
    if trying && lock.in_spot(picking.pin) {
        picking.forcing = 0.0;
        if picking.lock >= 90.0 {
            picking.success = Some(0.0);
            picking.durability = PICK_DURABILITY;
        }
    } else if trying && picking.lock >= target - 0.01 {
        // Forcing against the pins: jitter and wear.
        picking.pin = (picking.pin + rand::thread_rng().gen_range(-0.5..0.5)).clamp(0.0, 180.0);
        picking.forcing += dt;
        let due = if picking.forcing < WEAR_DELAY { 0 } else { 1 + ((picking.forcing - WEAR_DELAY) / WEAR_EVERY) as u32 };
        while picking.worn < due {
            picking.worn += 1;
            picking.durability -= WEAR;
            if picking.durability <= 0.0 {
                // BreakLockpick: one lockpick used up, a fresh one, the pick back at rest.
                player_inv(&mut game).remove(LOCKPICK, 1);
                notify.write(Notify::new("Lockpick broke"));
                picking.durability = PICK_DURABILITY;
                picking.pin = 0.0;
                picking.forcing = 0.0;
                picking.worn = 0;
                break;
            }
        }
    } else {
        picking.forcing = 0.0;
        picking.worn = 0;
    }
    for (mut t, mut v) in &mut pins {
        t.rotation = Rot2::degrees(picking.pin);
        *v = Visibility::Inherited;
    }
    for mut t in &mut locks {
        t.rotation = Rot2::degrees(picking.lock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweet_spot_turns_fully_and_distance_limits_the_rest() {
        let lock = Lock { difficulty: Difficulty::Standard, range: (100.0, 107.0) };
        assert_eq!(lock.max_turn(103.0), 90.0);
        assert_eq!(lock.max_turn(90.0), 80.0);
        assert_eq!(lock.max_turn(127.0), 70.0);
        assert_eq!(lock.max_turn(0.0), 0.0);
        let mut rng = rand::thread_rng();
        for _ in 0..50 {
            let l = Lock::roll(&mut rng);
            assert!((l.range.1 - l.range.0 - l.difficulty.width()).abs() < 1e-4 && l.range.0 < 180.0);
        }
    }
}
