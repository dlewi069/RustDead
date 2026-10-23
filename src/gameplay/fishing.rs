//! Fishing: BP_FishingRodPickup (CastRod, Event_FishingBehaviour, Event_Fish?, RandomChance,
//! CatchFish, StopFishing), its FishingRodPickupComponent, BP_FishingBuoy and BP_PlayerCharacter's
//! primary action with the rod drawn (9 = IA_FishingRod, EquipFishingRod).
//!
//! - **Primary action** (not swimming, no montage playing): with CanCatchFish? it's CatchFish;
//!   else while CurrentlyFishing? it's StopFishing(false) ("You didn't catch anything"); else
//!   Crouch_MeleeAttack_Montage at 0.8 and Timeline_FishRodMontage (0.25 s turning the survivor
//!   to the camera's yaw), and the cast.
//! - **CastRod**: a line from the rod to the camera + 35 m along its forward. Its first blocking
//!   hit must be water (SurfaceType6), else "Too far from water...". On water: movement off,
//!   CurrentlyFishing?, a BP_FishingBuoy at the hit (the lure, ×2) on a 400 cm cable.
//! - **Event_FishingBehaviour**: "...." and, 6-8 s later, Event_Fish?: RandomChance(30), true when
//!   30 <= RandomFloatInRange(0, 100) / FishingMultiplier. (So the skill's multiplier, which grows
//!   ×1.05 a level, makes bites *rarer*; that's the game's.) No bite: Event_FishingBehaviour again.
//!   A bite: "You feel a tug on the line", CanCatchFish?, the cable taut (length ÷ 5) for 0.2 s;
//!   CanCatchFish? goes again after 1.5-2.5 s ("...."), and 4-8 s after the bite Event_Fish? rolls
//!   again.
//! - **CatchFish**: ChanceToCatch = RandomFloatInRange(0, 100 / FishingMultiplier): >= 20 a Small
//!   Raw Fish (20-25 Fishing XP), > 5 a Raw Fish (40-50), else a Large Raw Fish (80-100). The fish
//!   goes in with JigTryAddItemSomewhere (lost when it doesn't fit), FishReelIN_Cue plays, then
//!   StopFishing(true): "You caught a fish!".
//! - **StopFishing**: the cable and buoy go, Crouch_MeleeAttack_Montage at 1.0, movement back on.
//! - BP_FishingBuoy bobs: MoveComponentTo up 5-15 cm over 1-2 s, then down 10-20 cm over 1-2 s
//!   (eased both ways), for ever.
//!
//! Ours: the cast happens at the montage's ActiveMeleeDetect notify (0.31 s, at rate 0.8); which
//! notify the BP waits for isn't readable in the dump. The trace starts at the rod's tip. The cable
//! is a straight line (no CableComponent sag). Putting the rod away while fishing stops it (the
//! game blocks equipping while CurrentlyFishing?). The ACH_Fish achievement isn't ported.

use bevy::prelude::*;
use rand::Rng;

use super::combat::{HeldWeapon, PlayerCamera};
use super::equipment::Slot;
use super::interact::player_inv;
use super::items::ItemDb;
use super::notify::Notify;
use crate::level::{KinematicBody, Terrain};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::models::{ActionKind, CharacterModel, UpperAction, model};
use crate::progression::passive::Passive;

/// CastRod's trace: the camera's forward × 3500 cm.
pub const CAST_RANGE: f32 = 35.0;
/// Timeline_FishRodMontage's length.
const TURN_SECS: f32 = 0.25;
/// The montage's ActiveMeleeDetect notify (0.308 s) at play rate 0.8.
const CAST_AT: f32 = 0.308 / 0.8;
/// Crouch_MeleeAttack_Montage's length (0.8 s at rate 1).
const MONTAGE_SECS: f32 = 0.8;
/// Event_FishingBehaviour's timer, the bite's re-roll timer and its CanCatchFish? window.
const WAIT: (f32, f32) = (6.0, 8.0);
const REROLL: (f32, f32) = (4.0, 8.0);
const WINDOW: (f32, f32) = (1.5, 2.5);
/// RandomChance's Chance.
const BITE_CHANCE: f32 = 30.0;
/// The cable: CableLength 400 cm, ÷ 5 on a bite for 0.2 s.
const CABLE: f32 = 4.0;
const TAUT_SECS: f32 = 0.2;
/// The rod mesh's tip (glb space; the mesh runs up +Y to 1.32 m).
const ROD_TIP: Vec3 = Vec3::new(0.05, 1.31, 0.0);
const LURE: &str = "game/SurrounDead/Content/PolygonFiles/PolygonShops/Meshes/Props/SM_Prop_Hunting_Fishing_Lure_01.glb";
/// Bait's RelativeScale3D.
const LURE_SCALE: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Catch {
    Small,
    Regular,
    Large,
}

impl Catch {
    pub fn item(self) -> &'static str {
        match self {
            Catch::Small => "DA_SmallRawFish",
            Catch::Regular => "DA_RawFish",
            Catch::Large => "DA_LargeRawFish",
        }
    }

    /// AddXP_Fishing's RandomFloatInRange.
    pub fn xp(self) -> (f32, f32) {
        match self {
            Catch::Small => (20.0, 25.0),
            Catch::Regular => (40.0, 50.0),
            Catch::Large => (80.0, 100.0),
        }
    }
}

/// RandomChance(30): `roll` is RandomFloatInRange(0, 100).
pub fn bites(roll: f32, fishing_multiplier: f32) -> bool {
    BITE_CHANCE <= roll / fishing_multiplier.max(0.01)
}

/// CatchFish's branches on ChanceToCatch (RandomFloatInRange(0, 100 / FishingMultiplier)).
pub fn catch_of(chance: f32) -> Catch {
    if chance >= 20.0 {
        Catch::Small
    } else if chance > 5.0 {
        Catch::Regular
    } else {
        Catch::Large
    }
}

/// Where along a ray (within `max`) it first goes under a water surface: `surface(x, z, below,
/// above)` is the highest water surface over (x, z) in that height range. Sampled every 25 cm.
pub fn water_along(origin: Vec3, dir: Vec3, max: f32, mut surface: impl FnMut(f32, f32, f32, f32) -> Option<f32>) -> Option<f32> {
    const STEP: f32 = 0.25;
    let mut prev = origin;
    let mut d = STEP;
    while d <= max {
        let p = origin + dir * d;
        if let Some(s) = surface(p.x, p.z, p.y.min(prev.y) - 0.01, p.y.max(prev.y) + 0.01)
            && p.y <= s
        {
            // Back up to where the segment crosses the surface.
            let over = (prev.y - s).max(0.0);
            let drop = (prev.y - p.y).max(1e-6);
            return Some(d - STEP + STEP * (over / drop).min(1.0));
        }
        prev = p;
        d += STEP;
    }
    None
}

/// MoveComponentTo with ease in and out (UE's EaseInOut, exponent 2).
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 2.0 * t * t } else { 1.0 - 2.0 * (1.0 - t) * (1.0 - t) }
}

/// The rod's state (FishingRodPickupComponent and BP_FishingRodPickup's variables).
#[derive(Resource, Default)]
pub struct Fishing {
    /// CurrentlyFishing?: the line is out (movement off).
    pub active: bool,
    /// CanCatchFish?: a fish is on.
    pub can_catch: bool,
    /// Seconds into the cast montage, until CastRod.
    cast: Option<f32>,
    /// FishingTimer: seconds to Event_Fish?.
    timer: f32,
    /// The bite's Delay before CanCatchFish? goes.
    window: Option<f32>,
    /// The bite's 0.2 s of taut cable.
    taut: f32,
    /// Where the buoy floats (the cast's hit).
    pub buoy_at: Option<Vec3>,
    buoy: Option<Entity>,
    line: Option<Entity>,
    /// The survivor's yaw turn (Timeline_FishRodMontage): from, to, seconds in.
    turn: Option<(Quat, Quat, f32)>,
    /// Seconds left of the cast or reel montage (GetCurrentMontage is valid: clicks do nothing).
    montage: f32,
}

impl Fishing {
    /// Runs the FishingTimer out now (the tour, so it needn't wait 6-8 s a roll).
    pub fn skip_wait(&mut self) {
        if self.active && !self.can_catch {
            self.timer = self.timer.min(0.0);
        }
    }
}

/// The buoy's bob (the Bait component's MoveComponentTo loop).
#[derive(Component)]
struct Buoy {
    from: f32,
    to: f32,
    t: f32,
    secs: f32,
    up: bool,
}

/// The fishing line (the CableComponent).
#[derive(Component)]
struct Line;

pub fn plugin(app: &mut App) {
    app.init_resource::<Fishing>()
        .add_systems(OnEnter(AppState::InGame), |mut f: ResMut<Fishing>| *f = Fishing::default())
        .add_systems(
            Update,
            use_rod
                .after(super::combat::CombatSet)
                .run_if(in_state(AppState::InGame))
                .run_if(in_state(MenuScreen::None))
                .run_if(super::on_foot)
                .run_if(super::building::not_building),
        )
        .add_systems(Update, (fish, bob, draw_line).chain().run_if(in_state(AppState::InGame)));
}

/// StopFishing: cable and buoy gone, the reel montage, movement back, the message.
fn stop(commands: &mut Commands, f: &mut Fishing, action: Option<&mut UpperAction>, caught: bool, notify: &mut MessageWriter<Notify>) {
    for e in [f.buoy.take(), f.line.take()].into_iter().flatten() {
        commands.entity(e).despawn();
    }
    *f = Fishing::default();
    if let Some(a) = action {
        a.start(ActionKind::Cast);
        f.montage = MONTAGE_SECS;
    }
    notify.write(Notify::new(if caught { "You caught a fish!" } else { "You didn't catch anything" }));
    info!("fishing: stopped (caught {caught})");
}

/// BP_PlayerCharacter's primary action with the rod drawn, and the cast montage.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn use_rod(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    assets: Res<AssetServer>,
    db: Res<ItemDb>,
    armed: Res<super::Armed>,
    swim: Res<super::swim::Swim>,
    terrain: Option<Res<Terrain>>,
    mut rays: crate::level::collision::PropRays,
    mut f: ResMut<Fishing>,
    mut game: ResMut<CurrentGame>,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
    rod: Query<(&HeldWeapon, &GlobalTransform)>,
    mut player: Query<(&mut Transform, &mut UpperAction), (With<CharacterModel>, With<KinematicBody>)>,
    mut notify: MessageWriter<Notify>,
    mut sound: MessageWriter<crate::audio::Sound>,
) {
    let Ok((mut me, mut action)) = player.single_mut() else { return };
    let drawn = armed.drawn == Some(Slot::FishingRod);
    if !drawn {
        if f.active {
            stop(&mut commands, &mut f, None, false, &mut notify);
        }
        f.cast = None;
        f.turn = None;
        return;
    }
    let dt = time.delta_secs();
    f.montage = (f.montage - dt).max(0.0);
    if let Some((from, to, t)) = f.turn.as_mut() {
        *t += dt;
        me.rotation = from.slerp(*to, (*t / TURN_SECS).min(1.0));
        if *t >= TURN_SECS {
            f.turn = None;
        }
    }
    let Ok(cam) = camera.single() else { return };
    if let Some(t) = f.cast.as_mut() {
        *t += dt;
        if *t < CAST_AT {
            return;
        }
        f.cast = None;
        // CastRod: from the rod's tip to camera + forward × 35 m.
        let tip = rod.iter().find(|(h, _)| h.0 == Slot::FishingRod).map_or(me.translation + Vec3::Y * 1.5, |(_, g)| g.transform_point(ROD_TIP));
        let target = cam.translation() + cam.forward().as_vec3() * CAST_RANGE;
        let span = target - tip;
        let (dir, len) = (span.normalize_or_zero(), span.length());
        let ground = super::combat::terrain_hit(terrain.as_deref(), tip, dir, len).unwrap_or(len);
        let block = rays.cast(tip, dir, ground).unwrap_or(ground);
        let water = water_along(tip, dir, block, |x, z, lo, hi| rays.water_surface(x, z, lo, hi));
        let Some(d) = water else {
            notify.write(Notify::new("Too far from water..."));
            let end = tip + dir * block;
            info!(
                "fishing: no water along the cast from {tip:.1} along {dir:.2} (ground at {ground:.1} m, blocked at {block:.1} m; water there {:?})",
                rays.water_surface(end.x, end.z, end.y - 5.0, end.y + 5.0)
            );
            return;
        };
        let at = tip + dir * d;
        f.active = true;
        f.buoy_at = Some(at);
        let buoy = commands.spawn((Transform::from_translation(at), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        commands.spawn((model(&assets, LURE), Transform::from_scale(Vec3::splat(LURE_SCALE)), Buoy { from: 0.0, to: 0.0, t: 1.0, secs: 1.0, up: false }, ChildOf(buoy)));
        f.buoy = Some(buoy);
        let line = commands.spawn((Mesh3d(Handle::default()), Transform::default(), Line, DespawnOnExit(AppState::InGame))).id();
        f.line = Some(line);
        // Event_FishingBehaviour.
        notify.write(Notify::new("...."));
        f.timer = rand::thread_rng().gen_range(WAIT.0..=WAIT.1);
        info!("fishing: cast {d:.1} m to {at:.1}");
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) || swim.swimming || f.montage > 0.0 {
        return;
    }
    if f.can_catch {
        // CatchFish.
        let mult = game.state.as_ref().map_or(1.0, |s| s.passive.multiplier(Passive::Fishing)).max(0.01);
        let mut rng = rand::thread_rng();
        let what = catch_of(rng.gen_range(0.0..100.0 / mult));
        let rest = player_inv(&mut game).insert(&db, db.stack(what.item(), 1));
        let (lo, hi) = what.xp();
        if let Some(st) = game.state.as_mut() {
            st.passive.add_xp(Passive::Fishing, rng.gen_range(lo..=hi));
        }
        sound.write(crate::audio::Sound::at("FishReelIN_Cue", me.translation));
        info!("fishing: caught {what:?} ({}, {} didn't fit)", what.item(), rest.map_or(0, |r| r.count));
        stop(&mut commands, &mut f, Some(&mut action), true, &mut notify);
    } else if f.active {
        stop(&mut commands, &mut f, Some(&mut action), false, &mut notify);
    } else {
        action.start_at(ActionKind::Cast, Some(0.8));
        let fwd = cam.forward().as_vec3().with_y(0.0).normalize_or_zero();
        if fwd != Vec3::ZERO {
            f.turn = Some((me.rotation, Quat::from_rotation_y(fwd.x.atan2(fwd.z)), 0.0));
        }
        f.cast = Some(0.0);
        f.montage = MONTAGE_SECS / 0.8;
    }
}

/// FishingTimer → Event_Fish?, and the bite's Delays.
fn fish(time: Res<Time>, mut f: ResMut<Fishing>, game: Res<CurrentGame>, mut notify: MessageWriter<Notify>) {
    if !f.active {
        return;
    }
    let dt = time.delta_secs();
    f.taut = (f.taut - dt).max(0.0);
    if let Some(w) = f.window.as_mut() {
        *w -= dt;
        if *w <= 0.0 {
            f.window = None;
            f.can_catch = false;
            notify.write(Notify::new("...."));
        }
    }
    f.timer -= dt;
    if f.timer > 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    let mult = game.state.as_ref().map_or(1.0, |s| s.passive.multiplier(Passive::Fishing));
    if bites(rng.gen_range(0.0..100.0), mult) {
        notify.write(Notify::new("You feel a tug on the line"));
        f.can_catch = true;
        f.taut = TAUT_SECS;
        f.window = Some(rng.gen_range(WINDOW.0..=WINDOW.1));
        f.timer = rng.gen_range(REROLL.0..=REROLL.1);
        info!("fishing: a bite");
    } else {
        notify.write(Notify::new("...."));
        f.timer = rng.gen_range(WAIT.0..=WAIT.1);
    }
}

/// BP_FishingBuoy's bob.
fn bob(time: Res<Time>, mut buoys: Query<(&mut Buoy, &mut Transform)>) {
    let mut rng = rand::thread_rng();
    for (mut b, mut t) in &mut buoys {
        b.t += time.delta_secs();
        if b.t >= b.secs {
            b.from = b.to;
            b.up = !b.up;
            b.to = if b.up { rng.gen_range(0.05..=0.15) } else { -rng.gen_range(0.10..=0.20) };
            b.secs = rng.gen_range(1.0..=2.0);
            b.t = 0.0;
        }
        t.translation.y = b.from + (b.to - b.from) * ease(b.t / b.secs);
    }
}

/// The line from the rod's tip to the lure: straight, pulled under while taut.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_line(
    f: Res<Fishing>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    rod: Query<(&HeldWeapon, &GlobalTransform)>,
    buoys: Query<&GlobalTransform, With<Buoy>>,
    mut line: Query<(&mut Transform, &mut Mesh3d, &mut Visibility), (With<Line>, Without<Buoy>)>,
    mut commands: Commands,
) {
    let Some(e) = f.line else { return };
    let Ok((mut t, mut mesh, mut vis)) = line.get_mut(e) else { return };
    let (m, mat) = look
        .get_or_insert_with(|| {
            // CableWidth 3 cm would vanish at a distance; ours is drawn a little finer and dark.
            (meshes.add(Cuboid::new(0.008, 0.008, 1.0)), materials.add(StandardMaterial { base_color: Color::srgb(0.1, 0.1, 0.1), unlit: true, ..default() }))
        })
        .clone();
    if mesh.0 != m {
        mesh.0 = m;
        commands.entity(e).insert(MeshMaterial3d(mat));
    }
    let (Some((_, tip)), Ok(lure)) = (rod.iter().find(|(h, _)| h.0 == Slot::FishingRod), buoys.single()) else {
        *vis = Visibility::Hidden;
        return;
    };
    let a = tip.transform_point(ROD_TIP);
    let b = lure.translation() - if f.taut > 0.0 { Vec3::Y * CABLE / 5.0 * 0.1 } else { Vec3::ZERO };
    let span = b - a;
    if span.length() < 0.01 {
        return;
    }
    *vis = Visibility::Inherited;
    *t = Transform::from_translation(a + span / 2.0).looking_to(span.normalize(), Vec3::Y).with_scale(Vec3::new(1.0, 1.0, span.length()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bites_follow_random_chance() {
        // 30 <= roll / multiplier: 70 % at level 1.
        assert!(!bites(29.9, 1.0));
        assert!(bites(30.0, 1.0));
        // A higher multiplier makes bites rarer (the game's own formula).
        assert!(!bites(31.0, 1.05));
        assert!(bites(31.5, 1.05));
    }

    #[test]
    fn catches_by_chance_to_catch() {
        assert_eq!(catch_of(99.0), Catch::Small);
        assert_eq!(catch_of(20.0), Catch::Small);
        assert_eq!(catch_of(19.9), Catch::Regular);
        assert_eq!(catch_of(5.1), Catch::Regular);
        assert_eq!(catch_of(5.0), Catch::Large);
        assert_eq!(catch_of(0.0), Catch::Large);
        assert_eq!(Catch::Large.xp(), (80.0, 100.0));
        assert_eq!(Catch::Regular.item(), "DA_RawFish");
    }

    #[test]
    fn casts_find_water_before_the_ground() {
        // A flat lake at y = 0 everywhere x > 10.
        let lake = |x: f32, _z: f32, lo: f32, hi: f32| (x > 10.0 && lo <= 0.0 && 0.0 <= hi).then_some(0.0);
        // From 1.5 m up, aimed down at the water 20 m out.
        let origin = Vec3::new(0.0, 1.5, 0.0);
        let dir = (Vec3::new(20.0, 0.0, 0.0) - origin).normalize();
        let d = water_along(origin, dir, CAST_RANGE, lake).expect("hits the lake");
        let hit = origin + dir * d;
        assert!((hit.x - 20.0).abs() < 0.3 && hit.y.abs() < 0.05, "{hit}");
        // Aimed at the shore (x < 10): no water.
        let dir = (Vec3::new(8.0, 0.0, 0.0) - origin).normalize();
        assert_eq!(water_along(origin, dir, 8.2, lake), None);
        // Aimed over the lake level: never goes under.
        assert_eq!(water_along(origin, Vec3::X, CAST_RANGE, lake), None);
    }

    #[test]
    fn the_buoy_eases() {
        assert_eq!(ease(0.0), 0.0);
        assert_eq!(ease(0.5), 0.5);
        assert_eq!(ease(1.0), 1.0);
        assert!(ease(0.25) < 0.25);
    }
}
