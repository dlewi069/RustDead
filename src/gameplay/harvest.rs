//! Harvesting (BP_HarvestableObject_Tree / _Rocks / _ScrapMetal, BP_MeleePickup's hit switch).
//!
//! - **Trees and rocks**: a melee swing that hits a foliage instance whose surface is Tree or
//!   Stone (`level::Harvestable`, from `harvest.json`) runs HitFoliageTree / HitFoliageRock: a
//!   weapon without CanDamageTree? / CanDamageRock? says "No damage to tree dealt." (".. rock .."),
//!   else the instance becomes a harvestable that takes 6-8 hits (SetMaxHits, on the first one),
//!   each costing ReduceDurability(0.5). A tree's last hit fells it 0.2 s later (FallOverTimeline:
//!   collision off, TreeFallingRandomCue, 5 s to tip 90° over) for 8-12 Wooden Logs, and it's gone
//!   30 s after; a rock's last hit gives 8-12 Rocks and it's gone 0.2 s later. Skill_HarvestMaster
//!   doubles both. Felled trees and broken rocks stay gone for the session (the instance is removed).
//!   The power attack's 3-4 hits per swing isn't ported (nor the power attack itself).
//! - **Burnt-out vehicles** (the level's 78 BP_HarvestableObject_ScrapMetal): "Burnt Out Vehicle:
//!   [F] Salvage" starts a 10 s "Salvaging" timer (F again or walking away stops it); done, it gives
//!   12-20 Scrap Metal (×2 with Harvest Master) and HitScrapMetalRandom_Cue, and is then "No more
//!   Scrap Metal" for the session.
//! - **Trash piles** (the level's 141 TrashObjects): HitObjectWorld → HitTrashObject. A weapon
//!   without CanDamageTrashObjects? says "No damage to trash object dealt."; else the swing's
//!   melee damage comes off the pile's 50 Health (HittingTrash_Cue), at ReduceDurability(0.5).
//!   At 0 it breaks at once (WoodBreaking_Cue, hidden, collision off) for 1-5 Scrap Metal; Harvest
//!   Master doesn't count. The FX_HarvestableRock puff, camera shake and hit marker aren't ported.

use std::collections::HashMap;

use bevy::prelude::*;
use rand::Rng;

use super::interact::{GameOverlay, Prompt, PromptBar, PromptText, player_inv};
use super::items::ItemDb;
use super::notify::Notify;
use crate::level::collision::CollisionCache;
use crate::level::{Harvestable, Level, PropItem, PropKey, PropStreaming};
use crate::menus::{AppState, CurrentGame, MenuScreen};
use crate::progression::tech;

/// A melee swing (written by `combat::fire`): where from, which way, with what.
#[derive(Message, Clone)]
pub struct Swing {
    /// The survivor's feet.
    pub feet: Vec3,
    /// Horizontal facing (unit).
    pub facing: Vec3,
    pub weapon: String,
    /// HitObjectWorld's Dmg: RandomFloatInRange over FindStat(MeleeDamage)'s Min..Max, which for a
    /// rolled instance stat we take as its roll (a guess).
    pub damage: f32,
}

/// How far a swing reaches into a trunk or rock (the weapon sockets' sweep; ours, like
/// `combat::MELEE_REACH`).
const REACH: f32 = 2.0;
/// SphereTraceMulti's radius, 28 cm.
const TRACE_RADIUS: f32 = 0.28;
/// SetMaxHits: RandomIntegerInRange(6, 8).
const MAX_HITS: (i32, i32) = (6, 8);
/// Logs or rocks given: RandomIntegerInRange(8, 12); scrap: (12, 20).
const LOGS: (u32, u32) = (8, 12);
const SCRAP: (u32, u32) = (12, 20);
/// TrashObject: its Health, and the Scrap Metal it gives (RandomIntegerInRange(1, 5)).
const TRASH_HEALTH: f32 = 50.0;
const TRASH_SCRAP: (u32, u32) = (1, 5);
/// The Delay before the tree falls / the rock goes; FallOverTimeline's length; the felled tree's
/// Delay before K2_DestroyActor.
const BREAK_DELAY: f32 = 0.2;
const FALL_SECS: f32 = 5.0;
const FELLED_SECS: f32 = 30.0;
/// The salvage's InteractTimerWidget.MaxTime / timer.
const SALVAGE_SECS: f32 = 10.0;
/// How near a wreck's middle F reaches (the game traces from the camera to its mesh; a car is
/// ~4.5 m long, so ours measures to its centre with room for that).
const SALVAGE_REACH: f32 = 3.5;

/// Per-session harvesting state.
#[derive(Resource, Default)]
pub struct Harvest {
    /// Hits left on props hit at least once (MaxHits).
    pub hits: HashMap<PropKey, i32>,
    /// Trash piles' Health, once hit.
    pub trash: HashMap<PropKey, f32>,
    /// Props on their last hit's Delay: seconds left, the hit point.
    breaking: Vec<(PropKey, f32, Vec3)>,
    /// Wrecks (actor index) salvaged.
    pub salvaged: std::collections::HashSet<usize>,
    /// The wreck being salvaged and seconds into it (IsHeld).
    pub salvaging: Option<(usize, f32)>,
    /// BP_HarvestableObject_ScrapMetal actors (index, position), once the level is in.
    wrecks: Option<Vec<(usize, Vec3)>>,
}

/// A felled tree tipping over (FallOverTimeline), despawned when its time is up.
#[derive(Component)]
pub struct Falling {
    base: Transform,
    axis: Vec3,
    t: f32,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Swing>()
        .init_resource::<Harvest>()
        .add_systems(OnEnter(AppState::InGame), reset)
        .add_systems(
            Update,
            (hit_props.after(super::combat::CombatSet), break_props, fall_trees, salvage.after(crate::level::doors::interact_doors)).run_if(in_state(AppState::InGame)),
        );
}

fn reset(mut harvest: ResMut<Harvest>) {
    *harvest = Harvest::default();
}

/// HitHarvestableObject: `hits` off what's left (6-8 on the first hit). True when that was the last.
pub fn take_hits(left: &mut HashMap<PropKey, i32>, key: PropKey, hits: i32, rng: &mut impl Rng) -> bool {
    let n = left.entry(key).or_insert_with(|| rng.gen_range(MAX_HITS.0..=MAX_HITS.1));
    *n -= hits;
    *n <= 0
}

/// TrashObject's ObjectDamaged: Health -= Dmg. True when that broke it (Health <= 0).
pub fn damage_trash(health: &mut HashMap<PropKey, f32>, key: PropKey, dmg: f32) -> bool {
    let h = health.entry(key).or_insert(TRASH_HEALTH);
    *h -= dmg;
    *h <= 0.0
}

/// What a harvest gives: RandomIntegerInRange(lo, hi) × SelectInt(2, 1, Harvest Master).
pub fn yield_of((lo, hi): (u32, u32), harvest_master: bool, rng: &mut impl Rng) -> u32 {
    rng.gen_range(lo..=hi) * if harvest_master { 2 } else { 1 }
}

/// FallOverTimeline__UpdateFunc's target pitch (degrees, UE): +90 tips the top towards the tree's
/// -forward, -90 towards +forward. Taken as the BP computes it: the angle between the forward and
/// the direction from the trunk's foot at height 0 to `hit`, signed by forward.z × that direction's
/// z, and -90 only below -90°. (So a tree whose forward leans the same way up as the hit's height
/// always tips back, whichever side it was hit from.) Bevy space: y is up.
pub fn fall_pitch(forward: Vec3, at: Vec3, hit: Vec3) -> f32 {
    let to = (hit - at.with_y(0.0)).normalize_or_zero();
    let f = forward.normalize_or_zero();
    let sign = if f.y * to.y > 0.0 { 1.0 } else { -1.0 };
    if f.dot(to).clamp(-1.0, 1.0).acos().to_degrees() * sign < -90.0 { -90.0 } else { 90.0 }
}

/// FallOverAlpha: CurveFloat_0, cubic 0 → 1 over 5 s with flat tangents.
pub fn fall_alpha(t: f32) -> f32 {
    let u = (t / FALL_SECS).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// BP_MeleePickup.HitFoliageTree / HitFoliageRock / HitTrashObject for each swing that meets a
/// Tree or Stone prop or a trash pile.
#[allow(clippy::too_many_arguments)]
fn hit_props(
    mut swings: MessageReader<Swing>,
    db: Res<ItemDb>,
    store: Res<crate::menus::settings::SettingsStore>,
    level: Option<Res<Level>>,
    streaming: Option<Res<PropStreaming>>,
    meshes: Res<Assets<Mesh>>,
    mut cache: ResMut<CollisionCache>,
    mut harvest: ResMut<Harvest>,
    mut game: ResMut<CurrentGame>,
    armed: Res<super::Armed>,
    mut notify: MessageWriter<Notify>,
    mut sound: MessageWriter<crate::audio::Sound>,
    mut fx: MessageWriter<super::particles::Fx>,
) {
    let (Some(level), Some(streaming)) = (level, streaming) else {
        swings.clear();
        return;
    };
    let mut rng = rand::thread_rng();
    for s in swings.read() {
        // The sweep: a fan of rays at waist and chest height, the trace sphere's width apart.
        let side = Vec3::new(s.facing.z, 0.0, -s.facing.x);
        let mut best: Option<(f32, PropKey, Vec3)> = None;
        // (Ours: the lowest row reaches the knee-high trash piles.)
        for h in [0.4, 0.8, 1.2] {
            for k in [-1.0, 0.0, 1.0] {
                let o = s.feet + Vec3::Y * h + side * (k * TRACE_RADIUS);
                if let Some((d, key)) = cache.cast_item(&level, &streaming, &meshes, o, s.facing, REACH)
                    && best.is_none_or(|b| d < b.0)
                {
                    best = Some((d, key, o + s.facing * d));
                }
            }
        }
        let Some((_, key, at)) = best else { continue };
        let Some(&kind) = level.harvest.get(&key) else { continue };
        if streaming.is_removed(key) || harvest.breaking.iter().any(|b| b.0 == key) {
            continue;
        }
        let harvests = db.items.get(&s.weapon).map(|d| d.harvests.clone()).unwrap_or_default();
        let (needs, what, cue) = match kind {
            Harvestable::Tree => ("tree", "tree", "ChopWoodRandom_Cue"),
            Harvestable::Stone => ("rock", "rock", "MineStoneRandom_Cue"),
            Harvestable::Trash => ("trash", "trash object", "HittingTrash_Cue"),
        };
        if !harvests.iter().any(|h| h == needs) {
            notify.write(Notify::new(format!("No damage to {what} dealt.")));
            continue;
        }
        // ReduceDurability(0.5): ReduceDurabilityAmount × WEAPON DURABILITY MULTIPLIER × 0.5.
        if super::combat::store_bool(&store, "General", "WEAPON DURABILITY ENABLED?")
            && let Some(def) = db.items.get(&s.weapon)
            && let Some(w) = super::combat::drawn(&armed, &mut game)
        {
            let loss = def.reduce_durability * super::survival::difficulty(&store, "General", "WEAPON DURABILITY MULTIPLIER") * 0.5;
            w.durability = Some((w.durability.unwrap_or(def.durability) - loss).max(0.0));
        }
        if kind == Harvestable::Trash {
            sound.write(crate::audio::Sound::at(cue, at));
            let broke = damage_trash(&mut harvest.trash, key, s.damage);
            info!("harvest: trash {key:?} hit, {:.0} health left", harvest.trash[&key]);
            if broke {
                // ObjectDestroyed: WoodBreaking_Cue, hidden and collision off at once; the scrap.
                sound.write(crate::audio::Sound::at("WoodBreaking_Cue", at));
                let n = rng.gen_range(TRASH_SCRAP.0..=TRASH_SCRAP.1);
                let rest = player_inv(&mut game).insert(&db, db.stack("DA_ScrapMetal", n));
                info!("harvest: trash {key:?} done: {n} DA_ScrapMetal ({} didn't fit)", rest.map_or(0, |r| r.count));
                harvest.breaking.push((key, 0.0, at));
            }
            continue;
        }
        let last = take_hits(&mut harvest.hits, key, 1, &mut rng);
        info!("harvest: {what} {key:?} hit, {} left", harvest.hits[&key]);
        // The tree plays its chop only while it stands; the rock every hit.
        if !last || kind == Harvestable::Stone {
            sound.write(crate::audio::Sound::at(cue, at));
        }
        if last {
            let master = game.state.as_ref().is_some_and(|st| st.tech.has(tech::HARVEST_MASTER));
            let (item, n) = match kind {
                Harvestable::Tree => ("DA_WoodenLog", yield_of(LOGS, master, &mut rng)),
                Harvestable::Stone | Harvestable::Trash => ("DA_Rock", yield_of(LOGS, master, &mut rng)),
            };
            // JigTryAddItemSomewhere: what doesn't fit is lost.
            let rest = player_inv(&mut game).insert(&db, db.stack(item, n));
            info!("harvest: {what} {key:?} done: {n} {item} ({} didn't fit)", rest.map_or(0, |r| r.count));
            // BP_HarvestableObject_Rocks, out of hits: FX_HarvestableRock 15 cm above the actor,
            // at a quarter scale.
            if kind == Harvestable::Stone {
                let base = level.prop_parts(key).map_or(at, |(_, t)| t.translation);
                fx.write(super::particles::Fx::at("FX_HarvestableRock", base + Vec3::Y * 0.15).scaled(Vec3::splat(0.25)));
            }
            harvest.breaking.push((key, BREAK_DELAY, at));
        }
    }
}

/// After the last hit's Delay: a tree is swapped for a falling copy, a rock (or a trash pile, with
/// no delay) just goes.
#[allow(clippy::too_many_arguments)]
fn break_props(
    mut commands: Commands,
    time: Res<Time>,
    level: Option<Res<Level>>,
    streaming: Option<ResMut<PropStreaming>>,
    mut cache: ResMut<CollisionCache>,
    mut harvest: ResMut<Harvest>,
    items: Query<(Entity, &PropItem)>,
    mut sound: MessageWriter<crate::audio::Sound>,
) {
    let (Some(level), Some(mut streaming)) = (level, streaming) else { return };
    let dt = time.delta_secs();
    let mut due = Vec::new();
    harvest.breaking.retain_mut(|b| {
        b.1 -= dt;
        if b.1 <= 0.0 {
            due.push((b.0, b.2));
        }
        b.1 > 0.0
    });
    for (key, hit) in due {
        if level.harvest.get(&key) == Some(&Harvestable::Tree)
            && let Some((parts, base)) = level.prop_parts(key)
        {
            // MC_FallOverEffect: collision off, the fall's sound, FallOverTimeline.
            let forward = base.rotation * Vec3::X;
            let away = forward.with_y(0.0).normalize_or_zero() * if fall_pitch(forward, base.translation, hit) > 0.0 { -1.0 } else { 1.0 };
            let axis = Vec3::Y.cross(away).normalize_or_zero();
            sound.write(crate::audio::Sound::at("TreeFallingRandomCue", base.translation));
            commands
                .spawn((base, Visibility::default(), Falling { base, axis, t: 0.0 }, DespawnOnExit(AppState::InGame)))
                .with_children(|c| {
                    for (mesh, material) in parts {
                        c.spawn((Mesh3d(mesh), MeshMaterial3d(material), Transform::IDENTITY));
                    }
                });
            info!("harvest: tree {key:?} falls towards {away:.2}");
        }
        streaming.remove(key);
        cache.forget(key);
        for (e, p) in &items {
            if p.0 == key {
                commands.entity(e).despawn();
            }
        }
    }
}

/// FallOverTimeline__UpdateFunc: tip over about the foot; K2_DestroyActor 30 s after the fall.
fn fall_trees(mut commands: Commands, time: Res<Time>, mut trees: Query<(Entity, &mut Falling, &mut Transform)>) {
    for (e, mut f, mut t) in &mut trees {
        f.t += time.delta_secs();
        if f.t >= FELLED_SECS {
            commands.entity(e).despawn();
            continue;
        }
        t.rotation = Quat::from_axis_angle(f.axis, fall_alpha(f.t) * std::f32::consts::FRAC_PI_2) * f.base.rotation;
    }
}

/// BP_HarvestableObject_ScrapMetal's interaction, when no container, door or switch took the prompt.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn salvage(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<State<MenuScreen>>,
    overlay: Res<GameOverlay>,
    db: Res<ItemDb>,
    mut harvest: ResMut<Harvest>,
    mut game: ResMut<CurrentGame>,
    player: Query<&Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut prompt: Query<&mut Visibility, With<Prompt>>,
    mut text: Query<&mut Text, With<PromptText>>,
    mut bar: Query<&mut Node, With<PromptBar>>,
    mut notify: MessageWriter<Notify>,
    mut sound: MessageWriter<crate::audio::Sound>,
    level: Option<Res<Level>>,
    driving: Res<super::vehicles::Driving>,
) {
    if driving.active() {
        return;
    }
    if harvest.wrecks.is_none()
        && let Some(level) = level
    {
        harvest.wrecks = Some(level.actors.iter().enumerate().filter(|(_, a)| a.class == "BP_HarvestableObject_ScrapMetal_C").map(|(i, a)| (i, a.position())).collect());
    }
    let (Ok(p), Ok(mut vis)) = (player.single(), prompt.single_mut()) else { return };
    let p = p.translation;
    let near = harvest.wrecks.iter().flatten().filter(|(_, at)| (at.y - p.y).abs() < 3.0 && at.xz().distance(p.xz()) < SALVAGE_REACH).min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p))).copied();
    // OnEndInteract (looked away): the salvage stops.
    if harvest.salvaging.is_some_and(|(w, _)| near.is_none_or(|n| n.0 != w)) {
        harvest.salvaging = None;
    }
    if *screen.get() != MenuScreen::None || overlay.is_open() || *vis != Visibility::Hidden {
        return;
    }
    let Some((wreck, at)) = near else { return };
    *vis = Visibility::Inherited;
    let done = harvest.salvaged.contains(&wreck);
    if let Ok(mut t) = text.single_mut() {
        // ShowInteractWidget("Burnt Out Vehicle", Option1 "Salvage"); InteractTimerUI "Salvaging".
        t.0 = if harvest.salvaging.is_some() { "Salvaging".into() } else { "Burnt Out Vehicle: [F] Salvage".into() };
    }
    if keys.just_pressed(KeyCode::KeyF) {
        if done {
            notify.write(Notify::new("No more Scrap Metal"));
        } else if harvest.salvaging.is_some() {
            harvest.salvaging = None;
        } else {
            harvest.salvaging = Some((wreck, 0.0));
        }
    }
    let progress = if let Some((w, t)) = harvest.salvaging.as_mut() {
        *t += time.delta_secs();
        let (w, t) = (*w, *t);
        if t >= SALVAGE_SECS {
            harvest.salvaging = None;
            harvest.salvaged.insert(w);
            let master = game.state.as_ref().is_some_and(|st| st.tech.has(tech::HARVEST_MASTER));
            let n = yield_of(SCRAP, master, &mut rand::thread_rng());
            let rest = player_inv(&mut game).insert(&db, db.stack("DA_ScrapMetal", n));
            sound.write(crate::audio::Sound::at("HitScrapMetalRandom_Cue", at));
            info!("harvest: salvaged wreck {w}: {n} DA_ScrapMetal ({} didn't fit)", rest.map_or(0, |r| r.count));
            0.0
        } else {
            t / SALVAGE_SECS
        }
    } else {
        0.0
    };
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(progress * 100.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harvestables_take_six_to_eight_hits() {
        let mut rng = rand::thread_rng();
        for _ in 0..50 {
            let mut left = HashMap::new();
            let key = (IVec2::ZERO, 0);
            let mut n = 1;
            while !take_hits(&mut left, key, 1, &mut rng) {
                n += 1;
            }
            assert!((6..=8).contains(&n), "{n} hits");
        }
    }

    #[test]
    fn trash_piles_break_at_fifty_damage() {
        let mut health = HashMap::new();
        let key = (IVec2::ZERO, 3);
        // A 22-damage crowbar: three swings.
        assert!(!damage_trash(&mut health, key, 22.0));
        assert!(!damage_trash(&mut health, key, 22.0));
        assert!(damage_trash(&mut health, key, 22.0));
        // A 47-damage sledgehammer: two.
        assert!(!damage_trash(&mut health, (IVec2::ONE, 0), 47.0));
        assert!(damage_trash(&mut health, (IVec2::ONE, 0), 47.0));
    }

    #[test]
    fn harvest_master_doubles_the_yield() {
        let mut rng = rand::thread_rng();
        for _ in 0..50 {
            assert!((8..=12).contains(&yield_of(LOGS, false, &mut rng)));
            let x = yield_of(SCRAP, true, &mut rng);
            assert!(x % 2 == 0 && (24..=40).contains(&x));
        }
    }

    #[test]
    fn trees_fall_as_the_timeline_says() {
        // Upright tree (forward flat): the sign is -1, so it tips away from the side hit.
        let fwd = Vec3::X;
        assert_eq!(fall_pitch(fwd, Vec3::new(0.0, 10.0, 0.0), Vec3::new(0.5, 11.0, 0.0)), 90.0);
        assert_eq!(fall_pitch(fwd, Vec3::new(0.0, 10.0, 0.0), Vec3::new(-0.5, 11.0, 0.0)), -90.0);
        // Forward leaning up with the hit above height 0: always back (+90).
        assert_eq!(fall_pitch(Vec3::new(1.0, 0.05, 0.0), Vec3::new(0.0, 10.0, 0.0), Vec3::new(-0.5, 11.0, 0.0)), 90.0);
        assert_eq!(fall_alpha(0.0), 0.0);
        assert!((fall_alpha(2.5) - 0.5).abs() < 1e-6);
        assert_eq!(fall_alpha(9.0), 1.0);
    }
}
