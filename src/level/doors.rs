//! Interactive doors, from the door Blueprints (decompiled; see each `Kind`):
//!
//! - `BP_LockedDoor`: locked until picked (the lockpicking minigame, XP = level x 100, "Door
//!   Unlocked, XP Given") or forced with a crowbar (10 x OTHER ITEMS DURABILITY MULTIPLIER of its
//!   durability, a noise heard 75 m away). Then F swings it 90°. From behind it opens for 5 s.
//! - `BP_LaboratorySlidingDoor`: slides 125 cm and closes after 5 s. `Unlocked: false` needs its
//!   keycard (consumed, "No Keycard Available"); `Locked` ones need the All Access keycard.
//!   `CanOpenDoorFromBehind?` ones open from behind regardless.
//! - `BP_KeycardDoor(_Child)`: needs its keycard (consumed), swings 90°.
//! - `BP_WarehouseDoors` (halves slide 250 cm apart), `BP_ContainerDoors` (wings swing 90°): free.
//! - `BP_SubwayTravelDoor`: travel: the clock jumps TimeToTravel (UDS units, 100 = 1 h) and the
//!   player arrives at TravelToLoc.
//!
//! The door meshes are the Blueprints' components in placements.json; `find` takes them out of
//! the static cells so they can move. Timelines' lengths aren't readable: doors take 1 s.
//! "Behind" is the actor's -X side (GetBehindObject? isn't decompiled; a guess).

use std::collections::HashSet;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

use super::collision::CollisionCache;
use super::{ActorInfo, Level, affine};
use crate::gameplay::interact::{Containers, GameOverlay, Prompt, PromptBar, PromptText, player_inv};
use crate::gameplay::notify::Notify;
use crate::menus::{AppState, CurrentGame, MenuScreen};

const REACH: f32 = 2.6;
const OPEN_SECS: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    /// Yaw about the part's hinge (UE degrees).
    Yaw(f32),
    /// Slide along the part's local X (UE cm).
    Slide(f32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Locked,
    Sliding { keycard: Option<String>, all_access: bool, from_behind: bool },
    Keycard(String),
    Free,
    Travel { to: Vec3, hours: f32, name: String },
}

#[derive(Clone, Debug)]
pub struct DoorPart {
    pub mesh: u32,
    pub base: Transform,
    pub motion: Motion,
}

#[derive(Clone, Debug)]
pub struct DoorDef {
    pub actor: usize,
    pub kind: Kind,
    pub parts: Vec<DoorPart>,
    pub pos: Vec3,
    /// The actor's +X (front) in world space.
    pub front: Vec3,
    /// Closes itself this long after opening.
    pub auto_close: Option<f32>,
}

/// (mesh file stem, part's relative location in the actor (UE cm), motion) per door class.
fn parts_of(a: &ActorInfo) -> Option<(Vec<(String, Vec3, Motion)>, Option<f32>)> {
    let p = |k: &str| a.props.as_ref().and_then(|m| m.get(k)).map(|v| v.trim_matches('"').to_string());
    Some(match a.class.as_str() {
        "BP_LockedDoor_C" => (vec![(p("Mesh").unwrap_or_else(|| "SM_Bld_Door_01".into()), Vec3::ZERO, Motion::Yaw(90.0))], None),
        "BP_LaboratorySlidingDoor_C" => (vec![("LabSmallDoor".into(), Vec3::ZERO, Motion::Slide(125.0))], Some(5.0)),
        "BP_KeycardDoor_C" | "BP_KeycardDoor_Child_C" => {
            (vec![("SM_Bld_Bunker_Wall_Caution_Door_Opening_01".into(), Vec3::new(11.0, -77.0, 29.0), Motion::Yaw(90.0))], None)
        }
        "BP_WarehouseDoors_C" => (
            vec![("WarehouseDoor_Left".into(), Vec3::ZERO, Motion::Slide(-250.0)), ("WarehouseDoor_Right".into(), Vec3::ZERO, Motion::Slide(250.0))],
            None,
        ),
        // Door1 (-107 Y) turns -180 -> -270, Door2 (+107 Y) -180 -> -90.
        "BP_ContainerDoors_C" => (
            vec![
                ("SM_Prop_Shipping_Container_Door_01".into(), Vec3::new(0.0, -107.19, 0.0), Motion::Yaw(-90.0)),
                ("SM_Prop_Shipping_Container_Door_01".into(), Vec3::new(0.0, 107.19, 0.0), Motion::Yaw(90.0)),
            ],
            None,
        ),
        "BP_SubwayTravelDoor_C" => (vec![("SM_Bld_Door_01".into(), Vec3::ZERO, Motion::Yaw(15.0))], None),
        _ => return None,
    })
}

fn kind_of(a: &ActorInfo) -> Kind {
    let p = |k: &str| a.props.as_ref().and_then(|m| m.get(k)).map(|v| v.trim_matches('"').to_string());
    match a.class.as_str() {
        "BP_LockedDoor_C" => Kind::Locked,
        "BP_LaboratorySlidingDoor_C" => {
            let locked = p("Locked").as_deref() == Some("true");
            let unlocked = p("Unlocked").as_deref() != Some("false");
            Kind::Sliding {
                keycard: (!unlocked && !locked).then(|| p("Keycard").unwrap_or_else(|| "DA_Level2Keycard".into())),
                all_access: locked,
                from_behind: p("CanOpenDoorFromBehind?").as_deref() == Some("true"),
            }
        }
        "BP_KeycardDoor_C" | "BP_KeycardDoor_Child_C" => Kind::Keycard(p("Keycard").unwrap_or_else(|| "DA_Level0Keycard".into())),
        "BP_SubwayTravelDoor_C" => {
            let to = p("TravelToLoc").and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok()).map_or(Vec3::ZERO, |v| {
                let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                Vec3::new(f("X"), f("Z"), f("Y")) / 100.0
            });
            let name = p("TravelToName")
                .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
                .and_then(|v| v.get("SourceString").and_then(|s| s.as_str()).map(str::to_string))
                .unwrap_or_else(|| "Subway".into());
            Kind::Travel { to, hours: p("TimeToTravel").and_then(|v| v.parse().ok()).unwrap_or(300.0) / 100.0, name }
        }
        _ => Kind::Free,
    }
}

/// Doors in the level and the placement instances (mesh path, index) they take over.
pub fn find(actors: &[ActorInfo], placements: &std::collections::HashMap<String, Vec<[f32; 12]>>) -> (Vec<(DoorDef, Vec<String>)>, HashSet<(String, usize)>) {
    let mut taken = HashSet::new();
    let mut doors = Vec::new();
    for (i, a) in actors.iter().enumerate() {
        let Some((specs, auto_close)) = parts_of(a) else { continue };
        let t = &a.transform;
        let (c0, c1, c2) = (Vec3::new(t[0], t[1], t[2]).normalize_or(Vec3::X), Vec3::new(t[3], t[4], t[5]).normalize_or(Vec3::Y), Vec3::new(t[6], t[7], t[8]).normalize_or(Vec3::Z));
        let pos = a.position();
        let mut parts = Vec::new();
        let mut paths = Vec::new();
        for (stem, rel, motion) in specs {
            let want = pos + (c0 * rel.x + c2 * rel.y + c1 * rel.z) / 100.0;
            let best = placements
                .iter()
                .filter(|(path, _)| path.rsplit('/').next() == Some(stem.as_str()))
                .flat_map(|(path, list)| list.iter().enumerate().map(move |(k, m)| (path, k, m)))
                .filter(|(path, k, _)| !taken.contains(&((*path).clone(), *k)))
                .map(|(path, k, m)| (path, k, affine(m), Vec3::new(m[9], m[10], m[11]).distance(want)))
                .filter(|c| c.3 < 3.0)
                .min_by(|a, b| a.3.total_cmp(&b.3));
            if let Some((path, k, base, _)) = best {
                taken.insert((path.clone(), k));
                paths.push(path.clone());
                parts.push(DoorPart { mesh: 0, base, motion });
            }
        }
        if !parts.is_empty() {
            doors.push((DoorDef { actor: i, kind: kind_of(a), parts, pos, front: c0, auto_close }, paths));
        }
    }
    (doors, taken)
}

/// A part's transform at `open` (0 closed .. 1 open).
pub fn part_transform(p: &DoorPart, open: f32) -> Transform {
    let mut t = p.base;
    match p.motion {
        // UE yaw is clockwise seen from above in a left-handed frame; our axes mirror it.
        Motion::Yaw(deg) => t.rotate_local_y(-(deg * open).to_radians()),
        Motion::Slide(cm) => t.translation += t.rotation * Vec3::X * (cm / 100.0 * open),
    }
    t
}

#[derive(Default)]
struct DoorState {
    open: f32,
    target: bool,
    locked: bool,
    since_open: f32,
    /// Opened from behind: closes again after 5 s, still locked.
    temporary: bool,
}

#[derive(Resource, Default)]
pub struct Doors {
    states: Vec<DoorState>,
    parts: Vec<Vec<Entity>>,
    spawned: bool,
    option: usize,
}

pub(super) fn reset_doors(mut commands: Commands) {
    commands.insert_resource(Doors::default());
}

#[derive(Component)]
struct DoorPartOf(usize, usize);

/// Spawn every door's parts once their meshes are ready.
pub(super) fn spawn_doors(mut commands: Commands, level: Option<Res<Level>>, mut doors: ResMut<Doors>) {
    let Some(level) = level else { return };
    if doors.spawned || level.doors.iter().any(|d| d.parts.iter().any(|p| level.meshes[p.mesh as usize].parts.is_none())) {
        return;
    }
    doors.spawned = true;
    for (di, d) in level.doors.iter().enumerate() {
        doors.states.push(DoorState { locked: matches!(d.kind, Kind::Locked | Kind::Keycard(_)) || matches!(&d.kind, Kind::Sliding { keycard, all_access, .. } if keycard.is_some() || *all_access), ..default() });
        let mut ents = Vec::new();
        for (pi, p) in d.parts.iter().enumerate() {
            let root = commands.spawn((part_transform(p, 0.0), Visibility::default(), DoorPartOf(di, pi), DespawnOnExit(AppState::InGame))).id();
            for m in level.meshes[p.mesh as usize].parts.as_deref().unwrap_or_default() {
                commands.spawn((Mesh3d(m.mesh.clone()), MeshMaterial3d(m.material.clone()), Transform::IDENTITY, ChildOf(root)));
            }
            ents.push(root);
        }
        doors.parts.push(ents);
    }
}

/// Animate doors and keep their collision where the door is.
pub(super) fn move_doors(
    time: Res<Time>,
    level: Option<Res<Level>>,
    mut doors: ResMut<Doors>,
    mut parts: Query<(&DoorPartOf, &mut Transform)>,
    mut cache: ResMut<CollisionCache>,
    meshes: Res<Assets<Mesh>>,
) {
    let Some(level) = level else { return };
    let dt = time.delta_secs();
    for (i, s) in doors.states.iter_mut().enumerate() {
        let before = s.open;
        let goal = if s.target { 1.0 } else { 0.0 };
        s.open = if s.open < goal { (s.open + dt / OPEN_SECS).min(1.0) } else { (s.open - dt / OPEN_SECS).max(0.0) };
        if s.target {
            s.since_open += dt;
            let close_after = level.doors[i].auto_close.or(if s.temporary { Some(5.0) } else { None });
            if close_after.is_some_and(|c| s.since_open >= c) {
                s.target = false;
                s.temporary = false;
            }
        }
        if before != s.open || !cache.has_dynamic(i) {
            let d = &level.doors[i];
            let tris: Vec<[Vec3; 3]> = d
                .parts
                .iter()
                .flat_map(|p| {
                    let t = part_transform(p, s.open);
                    level.meshes[p.mesh as usize].parts.as_deref().unwrap_or_default().iter().filter(|m| m.collide).filter_map(move |m| Some((meshes.get(&m.mesh)?.clone(), t)))
                })
                .flat_map(|(m, t)| super::collision::tris_of(&m, &t))
                .collect();
            cache.set_dynamic(i, tris);
        }
    }
    for (DoorPartOf(d, p), mut t) in &mut parts {
        if let Some(s) = doors.states.get(*d) {
            *t = part_transform(&level.doors[*d].parts[*p], s.open);
        }
    }
}

fn prompt_options(d: &DoorDef, s: &DoorState) -> Vec<&'static str> {
    match (&d.kind, s.locked) {
        (Kind::Locked, true) => vec!["Picklock", "Crowbar"],
        (Kind::Travel { .. }, _) => vec!["Travel"],
        _ => vec![if s.target { "Close" } else { "Open" }],
    }
}

/// The door prompt and F, when no container claimed the prompt this frame.
#[allow(clippy::too_many_arguments)]
pub(super) fn interact_doors(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    screen: Res<State<MenuScreen>>,
    level: Option<Res<Level>>,
    store: Res<crate::menus::settings::SettingsStore>,
    mut doors: ResMut<Doors>,
    mut game: ResMut<CurrentGame>,
    mut containers: ResMut<Containers>,
    mut overlay: ResMut<GameOverlay>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<super::KinematicBody>)>,
    mut prompt: Query<&mut Visibility, With<Prompt>>,
    mut text: Query<&mut Text, With<PromptText>>,
    mut bar: Query<&mut Node, With<PromptBar>>,
    mut notify: MessageWriter<Notify>,
    mut noise: MessageWriter<crate::gameplay::zombies::Noise>,
) {
    let Some(level) = level else { return };
    // Doors picked open in the lockpicking minigame.
    for (i, d) in level.doors.iter().enumerate() {
        if let Some(s) = doors.states.get_mut(i)
            && s.locked
            && containers.unlocked.contains(&d.actor)
        {
            s.locked = false;
            s.target = true;
            s.since_open = 0.0;
            if let Some(state) = game.state.as_mut() {
                let xp = state.progression.level.max(1) as u64 * 100;
                state.progression.add_xp(xp);
                notify.write(Notify::new(format!("Door Unlocked, XP Given: {xp}")));
            }
        }
    }
    let (Ok(mut me), Ok(mut vis)) = (player.single_mut(), prompt.single_mut()) else { return };
    if *screen.get() != MenuScreen::None || overlay.is_open() || *vis != Visibility::Hidden {
        return;
    }
    let p = me.translation;
    let Some((i, d)) = level
        .doors
        .iter()
        .enumerate()
        .filter(|(_, d)| (d.pos.y - p.y).abs() < 3.0 && d.pos.xz().distance(p.xz()) < REACH)
        .min_by(|a, b| a.1.pos.distance(p).total_cmp(&b.1.pos.distance(p)))
    else {
        return;
    };
    let Some(s) = doors.states.get(i) else { return };
    let opts = prompt_options(d, s);
    if scroll.delta.y != 0.0 {
        doors.option = (doors.option + 1) % opts.len().max(1);
    }
    let pick = doors.option.min(opts.len() - 1);
    *vis = Visibility::Inherited;
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(0.0);
    }
    let name = match &d.kind {
        Kind::Locked if s.locked => "Locked Door".to_string(),
        Kind::Travel { name, .. } => name.clone(),
        _ => "Door".to_string(),
    };
    if let Ok(mut t) = text.single_mut() {
        let more = if opts.len() > 1 { "  [Wheel] change" } else { "" };
        t.0 = format!("{name}: [F] {}{more}", opts[pick]);
    }
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let behind = (p - d.pos).dot(d.front) < 0.0;
    let s = &mut doors.states[i];
    match (&d.kind, opts[pick]) {
        (Kind::Locked, "Picklock") if behind => {
            s.target = true;
            s.temporary = true;
            s.since_open = 0.0;
        }
        (Kind::Locked, "Picklock") => {
            let lock = *containers.locks.entry(d.actor).or_insert_with(|| crate::gameplay::lockpick::Lock::roll(&mut rand::thread_rng()));
            match crate::gameplay::lockpick::can_try(&mut game, &lock) {
                Ok(()) => *overlay = GameOverlay::Lockpick(d.actor),
                Err(why) => {
                    notify.write(Notify::new(why));
                }
            }
        }
        (Kind::Locked, "Crowbar") => {
            let need = 10.0 * crate::gameplay::survival::difficulty(&store, "General", "OTHER ITEMS DURABILITY MULTIPLIER");
            let inv = player_inv(&mut game);
            match inv.find_mut("DA_Crowbar") {
                None => {
                    notify.write(Notify::new("No crowbar found"));
                }
                Some(c) if c.durability.unwrap_or(100.0) <= need => {
                    notify.write(Notify::new("Crowbar durability too low"));
                }
                Some(c) => {
                    c.durability = Some(c.durability.unwrap_or(100.0) - need);
                    noise.write(crate::gameplay::zombies::Noise { at: p, range: 75.0 });
                    s.target = true;
                    s.since_open = 0.0;
                }
            }
        }
        (Kind::Travel { to, hours, .. }, _) => {
            if let Some(state) = game.state.as_mut() {
                let env = &mut state.environment;
                let t = env.time_of_day_hours + hours;
                env.days += (t / 24.0) as u32;
                env.time_of_day_hours = t.rem_euclid(24.0);
            }
            me.translation = *to;
        }
        (Kind::Keycard(card), _) if s.locked => {
            if player_inv(&mut game).remove(card, 1) {
                s.locked = false;
                s.target = true;
                s.since_open = 0.0;
            } else {
                notify.write(Notify::new("No Keycard Available"));
            }
        }
        (Kind::Sliding { keycard, all_access, from_behind }, _) if s.locked && !(*from_behind && behind) => {
            let card = if *all_access { "DA_AllAccessKeycard" } else { keycard.as_deref().unwrap_or("DA_Level2Keycard") };
            if player_inv(&mut game).remove(card, 1) {
                s.locked = false;
                s.target = true;
                s.since_open = 0.0;
            } else {
                notify.write(Notify::new("No Keycard Available"));
            }
        }
        _ => {
            s.target = !s.target;
            s.since_open = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(motion: Motion) -> DoorPart {
        DoorPart { mesh: 0, base: Transform::default(), motion }
    }

    #[test]
    fn doors_swing_and_slide() {
        let t = part_transform(&part(Motion::Yaw(90.0)), 1.0);
        assert!((t.rotation * Vec3::X).abs_diff_eq(Vec3::Z, 1e-5), "{:?}", t.rotation * Vec3::X);
        assert_eq!(part_transform(&part(Motion::Yaw(90.0)), 0.0), Transform::default());
        let s = part_transform(&part(Motion::Slide(125.0)), 1.0);
        assert!((s.translation - Vec3::new(1.25, 0.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn level_doors_find_their_meshes() {
        let dir = crate::level::LEVEL_DIR;
        let (Ok(a), Ok(p)) = (std::fs::read_to_string(format!("{dir}/actors.json")), std::fs::read_to_string(format!("{dir}/placements.json"))) else { return };
        let actors: Vec<ActorInfo> = serde_json::from_str(&a).unwrap();
        let placements = serde_json::from_str(&p).unwrap();
        let (doors, taken) = find(&actors, &placements);
        let count = |k: &str| doors.iter().filter(|(d, _)| actors[d.actor].class == k).count();
        assert_eq!(count("BP_LockedDoor_C"), 20);
        assert!(count("BP_LaboratorySlidingDoor_C") >= 35, "{}", count("BP_LaboratorySlidingDoor_C"));
        assert_eq!(count("BP_SubwayTravelDoor_C"), 6);
        // Container doors have two wings each.
        assert!(doors.iter().filter(|(d, _)| actors[d.actor].class == "BP_ContainerDoors_C").all(|(d, _)| d.parts.len() == 2));
        assert_eq!(taken.len(), doors.iter().map(|(d, _)| d.parts.len()).sum::<usize>());
        let lab = doors.iter().map(|(d, _)| &d.kind).filter(|k| matches!(k, Kind::Sliding { keycard: Some(_), .. })).count();
        assert!(lab >= 10, "{lab} keycard lab doors");
    }
}
