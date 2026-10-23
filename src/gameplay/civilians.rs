//! Settlers: BP_RandomCivilian_Male / _Female (BP_MainFriendly AI, `assets/data/civilians.json`
//! from `extractor/gen_ai.py`), spawned by BP_AISpawner_SettlementSurvivors (the master spawner
//! with SpawnType 3: the level's 4 at two safe zones, and the settlement's 2).
//!
//! - **Spawning** is the master spawner's (`level::spawners`: SpawnAmount randomised N/1.5..N*1.5,
//!   x Difficulty.SettlementSurvivorAmount, which has no menu row: 1), once the survivor is
//!   within SpawnRadius after InitialSpawnDelay, one per SpawningInterval at a random point of
//!   the box projected onto the ground; never within DontSpawnCloseToPlayerRange unless
//!   SkipPlayerCheck?.
//! - **Looks** (SetMeshAndMaterial): a random body from Meshes, a random HairMeshes and
//!   BeardMeshes on the head, one of BackpackMeshes on the Backpack socket (the random hair
//!   colour isn't picked).
//! - **BT_RandomCivilian**: Wait (1 ± 3 s), Civilian_FindRandomLocation (a reachable point within
//!   20 m), MoveTo (within 70 cm) at CharMoveComp's 1 m/s turning 180°/s, Wait (5 ± 15 s), again.
//!   There's no navmesh here: points are random within the radius and kept within it of where
//!   the settler spawned (ours: so they stay in their camp).
//! - **The name tag** (NameWidget, BP_InteractWidgetCivilian "{ActionName} {Name}"): SetName picks
//!   a surname and a first name (RandomInteger(1000) into rows "1".."1000" of Surname_Names and
//!   Male_Names / FemaleNames; row 0 doesn't exist and leaves the default "Interact"). It shows
//!   105 cm over the capsule centre while the survivor's interact focus is on the settler
//!   (OnBeginInteract / OnEndInteract) and goes once F is pressed on them.
//! - They ignore the Weapons and Visibility channels: shots and melee pass through them.
//!
//! Not ported: the optimiser's despawning (they're hidden and stand still beyond 225 m instead),
//! collision with the survivor, AnimBP_SafeZoneSettlers' BS_AiSettler (the survivor's walk and
//! idle stand in).

use std::collections::HashMap;

use bevy::prelude::*;
use rand::Rng;
use serde::Deserialize;

use crate::level::spawners::{SpawnerDef, SpawnerRuntime};
use crate::level::{ActorInfo, KinematicBody};
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, MenuScreen};
use crate::models::{AnimSet, Animated, CharacterModel, Locomotion, Stance, UpperAction, model, ue_relative};

/// BP_MasterZombie-like AIOSubject reach: frozen and hidden beyond it (ours: the optimiser's last layer).
const SHOWN_WITHIN: f32 = 225.0;
/// The interact trace's reach past the survivor (ours).
const FOCUS_REACH: f32 = 3.5;
/// NameWidget's RelativeLocation Z (cm over the capsule centre).
const TAG_HEIGHT: f32 = 1.05;

#[derive(Deserialize, Clone, Debug)]
pub struct CivilianClass {
    pub male: bool,
    pub meshes: Vec<String>,
    pub hair: Vec<String>,
    pub beard: Vec<String>,
    pub backpacks: Vec<super::zombies::Backpack>,
    pub walk_speed: f32,
    pub turn_rate: f32,
    pub capsule_half_height: f32,
    pub capsule_radius: f32,
}

/// BT_RandomCivilian's waits (WaitTime, RandomDeviation), MoveTo's AcceptableRadius and the
/// wander radius, in s and m.
#[derive(Deserialize, Clone, Debug)]
pub struct Behaviour {
    pub before: [f32; 2],
    pub after: [f32; 2],
    pub accept: f32,
    pub radius: f32,
}

impl Default for Behaviour {
    fn default() -> Self {
        Self { before: [1.0, 3.0], after: [5.0, 15.0], accept: 0.7, radius: 20.0 }
    }
}

#[derive(Deserialize, Default, Debug)]
pub struct Names {
    pub male: Vec<String>,
    pub female: Vec<String>,
    pub surname: Vec<String>,
}

#[derive(Resource, Deserialize, Default, Debug)]
pub struct CivilianDb {
    pub classes: HashMap<String, CivilianClass>,
    #[serde(default)]
    pub behaviour: Behaviour,
    #[serde(default)]
    pub names: Names,
}

impl CivilianDb {
    pub fn load() -> Self {
        std::fs::read_to_string("assets/data/civilians.json").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| {
            warn!("assets/data/civilians.json missing or malformed (extractor/export_assets.sh --ai)");
            Self::default()
        })
    }

    /// SetName: RandomInteger(1000) rows of Surname_Names, then of the first-name table. Row "0"
    /// isn't there: no name (the widget keeps "Interact").
    pub fn name(&self, male: bool, surname_roll: usize, first_roll: usize) -> Option<String> {
        let surname = self.names.surname.get(surname_roll.checked_sub(1)?)?;
        let firsts = if male { &self.names.male } else { &self.names.female };
        Some(format!("{} {surname}", firsts.get(first_roll.checked_sub(1)?)?))
    }
}

/// BTTask_Wait: FRandRange(max(0, WaitTime - RandomDeviation), WaitTime + RandomDeviation).
pub fn wait_secs([time, dev]: [f32; 2], rng: &mut impl Rng) -> f32 {
    let lo = (time - dev).max(0.0);
    let hi = time + dev;
    if hi > lo { rng.gen_range(lo..hi) } else { lo }
}

/// What a settler is doing in BT_RandomCivilian.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wander {
    /// The first Wait, before picking a point.
    Resting(f32),
    Walking(Vec3),
    /// The Wait after arriving.
    Idling(f32),
}

#[derive(Component, Debug)]
pub struct Civilian {
    pub class: String,
    /// SetName's "{first} {surname}"; `None` = the widget's default "Interact".
    pub name: Option<String>,
    pub home: Vec3,
    pub wander: Wander,
    walk_speed: f32,
    turn_rate: f32,
}

/// One step of BT_RandomCivilian: the new state, the point to walk to (if walking).
pub fn wander_step(w: Wander, pos: Vec3, home: Vec3, b: &Behaviour, dt: f32, rng: &mut impl Rng) -> Wander {
    match w {
        Wander::Resting(t) if t > dt => Wander::Resting(t - dt),
        Wander::Resting(_) => {
            // Civilian_FindRandomLocation: within the radius of where it stands, kept within it of home.
            let a = rng.gen_range(0.0..std::f32::consts::TAU);
            let r = rng.gen_range(0.0..b.radius);
            let mut goal = pos + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
            let off = (goal - home).with_y(0.0);
            if off.length() > b.radius {
                goal = home + off.normalize() * b.radius;
            }
            Wander::Walking(goal)
        }
        Wander::Walking(goal) if (goal - pos).xz().length() <= b.accept => Wander::Idling(wait_secs(b.after, rng)),
        Wander::Walking(goal) => Wander::Walking(goal),
        Wander::Idling(t) if t > dt => Wander::Idling(t - dt),
        Wander::Idling(_) => Wander::Resting(wait_secs(b.before, rng)),
    }
}

/// The settlement survivors' spawners (the level's, and a spawned settlement's).
#[derive(Resource, Default)]
pub struct Settlers {
    spawners: Vec<(String, SpawnerDef, SpawnerRuntime)>,
    collected: bool,
    clock: f64,
}

impl Settlers {
    pub fn add(&mut self, a: &ActorInfo) {
        if let Some((class, def)) = SpawnerDef::settlers(a) {
            self.spawners.push((class, def, SpawnerRuntime::default()));
        }
    }

    pub fn spawner_count(&self) -> usize {
        self.spawners.len()
    }
}

/// The name tag shown, and whether F put it away for this focus.
#[derive(Resource, Default)]
struct Focus {
    on: Option<Entity>,
    dismissed: bool,
}

#[derive(Component)]
struct NameTag;

/// Hair, beard or backpack waiting for a bone: (body, bone, transform on it).
#[derive(Component)]
struct OnBone(Entity, String, Transform);

pub fn plugin(app: &mut App) {
    app.insert_resource(CivilianDb::load())
        .init_resource::<Settlers>()
        .init_resource::<Focus>()
        .add_systems(OnEnter(AppState::InGame), |mut s: ResMut<Settlers>, mut f: ResMut<Focus>| {
            *s = Settlers::default();
            *f = Focus::default();
        })
        .add_systems(Update, (collect, spawn_settlers, attach, wander, focus, name_tag).chain().run_if(in_state(AppState::InGame)));
}

fn collect(level: Option<Res<crate::level::Level>>, mut settlers: ResMut<Settlers>) {
    let Some(level) = level else { return };
    if settlers.collected {
        return;
    }
    settlers.collected = true;
    for a in &level.actors {
        settlers.add(a);
    }
    info!("settler spawners: {}", settlers.spawners.len());
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn spawn_settlers(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    db: Res<CivilianDb>,
    level: Option<Res<crate::level::Level>>,
    terrain: Option<Res<crate::level::Terrain>>,
    streaming: Option<Res<crate::level::PropStreaming>>,
    meshes: Res<Assets<Mesh>>,
    mut cache: ResMut<crate::level::collision::CollisionCache>,
    mut settlers: ResMut<Settlers>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>)>,
) {
    let (Some(level), Some(terrain), Ok(me)) = (level, terrain, player.single()) else { return };
    let me = me.translation;
    let dt = time.delta_secs();
    settlers.clock += dt as f64;
    let now = settlers.clock;
    let mut rng = rand::thread_rng();
    let Settlers { spawners, .. } = &mut *settlers;
    for (class, def, rt) in spawners.iter_mut() {
        let Some(c) = db.classes.get(class.as_str()) else { continue };
        if !rt.started() {
            // Difficulty.SettlementSurvivorAmount: no menu row, 1.
            rt.begin(def, 1.0, &mut rng);
        }
        let in_radius = def.pos.distance(me) <= def.radius;
        for _ in 0..rt.tick(def, dt, now, in_radius) {
            let e = def.extent * 100.0;
            let local = Vec3::new(rng.gen_range(-e.x..=e.x), rng.gen_range(-e.y..=e.y), e.z);
            let top = def.local_to_world(local);
            let prop = streaming.as_ref().and_then(|st| cache.prop_floor(&level, st, &meshes, top.xz(), (top.y - def.extent.z * 2.5, top.y)));
            let Some(at) = crate::level::spawners::project(def, top, &terrain, prop) else { continue };
            if at.distance(me) <= def.no_spawn_near_player {
                continue;
            }
            let name = db.name(c.male, rng.gen_range(0..1000), rng.gen_range(0..1000));
            spawn_settler(&mut commands, &assets, c, class, at, rng.gen_range(0.0..std::f32::consts::TAU), name, &db.behaviour, &mut rng);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_settler(commands: &mut Commands, assets: &AssetServer, c: &CivilianClass, class: &str, at: Vec3, yaw: f32, name: Option<String>, b: &Behaviour, rng: &mut impl Rng) -> Option<Entity> {
    let pick = |list: &[String], rng: &mut dyn rand::RngCore| (!list.is_empty()).then(|| list[rng.gen_range(0..list.len())].clone());
    let body = pick(&c.meshes, rng)?;
    let e = commands
        .spawn((
            model(assets, format!("game/SurrounDead/Content/{body}.glb")),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
            KinematicBody::new(c.capsule_radius, c.capsule_half_height * 2.0),
            Animated(AnimSet::Survivor),
            Stance::Unarmed,
            Locomotion::Idle,
            UpperAction::default(),
            Civilian { class: class.to_string(), name: name.clone(), home: at, wander: Wander::Resting(wait_secs(b.before, rng)), walk_speed: c.walk_speed, turn_rate: c.turn_rate },
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    // HairMesh / BeardMesh on `head` (P90 R-90, as on the traders); a backpack on the Backpack socket.
    for list in [&c.hair, &c.beard] {
        if let Some(m) = pick(list, rng) {
            commands.spawn((model(assets, format!("game/SurrounDead/Content/{m}.glb")), Visibility::Hidden, OnBone(e, "head".into(), ue_relative([0.0; 3], 90.0, 0.0, -90.0)), ChildOf(e)));
        }
    }
    if !c.backpacks.is_empty() {
        let p = &c.backpacks[rng.gen_range(0..c.backpacks.len())];
        let local = Transform::from_translation(Vec3::from(p.t)).with_rotation(Quat::from_array(p.r).normalize());
        commands.spawn((model(assets, format!("game/SurrounDead/Content/{}.glb", p.mesh)), Visibility::Hidden, OnBone(e, "Backpack".into(), local), ChildOf(e)));
    }
    info!("settler {class} {:?} at {at:.0}", name);
    Some(e)
}

/// Hang the pending parts off their bones (sockets resolve through the survivor skeleton's table).
fn attach(mut commands: Commands, items: Res<super::ItemDb>, pending: Query<(Entity, &OnBone)>, children: Query<&Children>, names: Query<&Name>) {
    for (e, OnBone(body, bone, local)) in &pending {
        let (bone, at) = items.sockets.get(bone.as_str()).map_or((bone.as_str(), *local), |s| (s.bone.as_str(), s.transform.to_transform() * *local));
        let Some(b) = children.iter_descendants(*body).find(|c| names.get(*c).is_ok_and(|n| n.as_str().eq_ignore_ascii_case(bone))) else { continue };
        commands.entity(e).remove::<OnBone>().insert((at, Visibility::Inherited));
        commands.entity(b).add_child(e);
    }
}

#[allow(clippy::type_complexity)]
fn wander(
    time: Res<Time>,
    db: Res<CivilianDb>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>, Without<Civilian>)>,
    mut civilians: Query<(&mut Civilian, &mut Transform, &mut Locomotion, &mut Visibility)>,
) {
    let Ok(me) = player.single() else { return };
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    for (mut c, mut t, mut loco, mut vis) in &mut civilians {
        let near = t.translation.distance(me.translation) <= SHOWN_WITHIN;
        vis.set_if_neq(if near { Visibility::Inherited } else { Visibility::Hidden });
        if !near {
            continue;
        }
        let pos = t.translation;
        c.wander = wander_step(c.wander, pos, c.home, &db.behaviour, dt, &mut rng);
        match c.wander {
            Wander::Walking(goal) => {
                let flat = (goal - pos).with_y(0.0);
                let want = Quat::from_rotation_y(flat.x.atan2(flat.z));
                let max = c.turn_rate.to_radians() * dt;
                let angle = t.rotation.angle_between(want);
                t.rotation = if angle <= max { want } else { t.rotation.slerp(want, max / angle) };
                let step = (c.walk_speed * dt).min(flat.length());
                let fwd = t.rotation * Vec3::Z;
                t.translation += fwd * step;
                loco.set_if_neq(Locomotion::Walk);
            }
            _ => {
                loco.set_if_neq(Locomotion::Idle);
            }
        }
    }
}

/// The survivor's interact focus: the settler the camera's forward ray meets within reach.
#[allow(clippy::type_complexity)]
fn focus(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<State<MenuScreen>>,
    mut focus: ResMut<Focus>,
    camera: Query<&GlobalTransform, With<super::PlayerCamera>>,
    player: Query<&Transform, (With<CharacterModel>, With<KinematicBody>, Without<Civilian>)>,
    civilians: Query<(Entity, &Transform, &KinematicBody), With<Civilian>>,
) {
    let (Ok(cam), Ok(me)) = (camera.single(), player.single()) else { return };
    let (origin, dir) = (cam.translation(), cam.forward().as_vec3());
    let reach = origin.distance(me.translation + Vec3::Y * 0.82) + FOCUS_REACH;
    let on = civilians
        .iter()
        .filter(|(_, t, _)| t.translation.distance(me.translation) < reach + 2.0)
        .filter_map(|(e, t, body)| super::humans::ray_capsule(origin, dir, t.translation, body.radius, body.height, reach).map(|d| (e, d)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e)
        .filter(|_| *screen.get() == MenuScreen::None);
    if on != focus.on {
        focus.on = on;
        focus.dismissed = false;
    }
    if on.is_some() && keys.just_pressed(KeyCode::KeyF) {
        focus.dismissed = true;
    }
}

fn name_tag(
    mut commands: Commands,
    font: Res<MenuFont>,
    focus: Res<Focus>,
    camera: Query<(&Camera, &GlobalTransform), With<super::PlayerCamera>>,
    civilians: Query<(&Civilian, &Transform, &KinematicBody)>,
    mut tags: Query<(Entity, &mut Node, &mut Text), With<NameTag>>,
) {
    let shown = focus.on.filter(|_| !focus.dismissed).and_then(|e| civilians.get(e).ok());
    let Some((c, t, body)) = shown else {
        for (e, _, _) in &tags {
            commands.entity(e).despawn();
        }
        return;
    };
    let Ok((cam, cam_t)) = camera.single() else { return };
    let head = t.translation + Vec3::Y * (body.height / 2.0 + TAG_HEIGHT);
    let Ok(px) = cam.world_to_viewport(cam_t, head) else { return };
    let text = c.name.clone().unwrap_or_else(|| "Interact".into());
    match tags.iter_mut().next() {
        Some((_, mut n, mut tx)) => {
            n.left = Val::Px(px.x);
            n.top = Val::Px(px.y);
            if tx.0 != text {
                tx.0 = text;
            }
        }
        None => {
            // Screen-space WidgetComponent drawn at its desired size, centred on the point.
            commands.spawn((
                label_colored(&font, text, 14.0, Color::WHITE),
                Node { position_type: PositionType::Absolute, left: Val::Px(px.x), top: Val::Px(px.y), ..default() },
                UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
                NameTag,
                Pickable::IGNORE,
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn settlers_and_their_names_load() {
        let db = CivilianDb::load();
        if db.classes.is_empty() {
            return;
        }
        let m = &db.classes["BP_RandomCivilian_Male_C"];
        assert!(m.male && !m.meshes.is_empty() && !m.beard.is_empty() && !m.backpacks.is_empty());
        assert!(!db.classes["BP_RandomCivilian_Female_C"].male);
        assert_eq!((m.walk_speed, m.turn_rate), (1.0, 180.0));
        assert_eq!(db.names.surname.len(), 1000);
        assert_eq!(db.name(true, 1, 1).as_deref(), Some("JAMES SMITH"));
        assert_eq!(db.name(false, 1, 1).as_deref(), Some("MARY SMITH"));
        // RandomInteger(1000) can give 0: no row "0", no name.
        assert_eq!(db.name(true, 0, 5), None);
        assert_eq!(db.name(true, 5, 0), None);
    }

    #[test]
    fn settlers_wait_walk_and_wait_again() {
        let b = Behaviour::default();
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        for _ in 0..50 {
            let w = wait_secs(b.after, &mut rng);
            assert!((0.0..=20.0).contains(&w));
        }
        let home = Vec3::ZERO;
        // Rested out: a point within 20 m, kept within 20 m of home.
        let Wander::Walking(goal) = wander_step(Wander::Resting(0.0), Vec3::new(19.0, 0.0, 0.0), home, &b, 0.1, &mut rng) else { panic!() };
        assert!(goal.length() <= 20.0 + 1e-3, "{goal}");
        // Within 70 cm: arrived, the second Wait.
        assert!(matches!(wander_step(Wander::Walking(Vec3::X), Vec3::new(0.5, 0.0, 0.0), home, &b, 0.1, &mut rng), Wander::Idling(_)));
        assert_eq!(wander_step(Wander::Walking(Vec3::X * 5.0), Vec3::ZERO, home, &b, 0.1, &mut rng), Wander::Walking(Vec3::X * 5.0));
        assert!(matches!(wander_step(Wander::Idling(0.05), Vec3::ZERO, home, &b, 0.1, &mut rng), Wander::Resting(_)));
    }
}
