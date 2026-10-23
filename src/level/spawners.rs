//! Zombie spawners: `BP_AISpawner_Master` (AIOptimizer's spawner) as subclassed by
//! `BP_AISpawner_Zombies`, ported from the decompiled Blueprint.
//!
//! - BeginPlay: `SetSpawnCount` scales `SpawnAmount` by `Difficulty.ZombieAmount` (randomised
//!   between N/1.5 and N*1.5 first unless `RandomisedSpawnCount` is off), then after
//!   `InitialSpawnDelay` a looping 0.75-2 s timer waits for the player within `SpawnRadius`.
//! - The player in range queues one group of `SpawnAmount`; actors spawn one per
//!   `SpawningInterval` while the player stays in range. Each spawn picks a random point at the
//!   top of `SpawningBox` projected down onto the landscape (or the `SpawnPoints` in order) and
//!   is skipped if the player is within `DontSpawnCloseToPlayerRange`. Skipped spawns still
//!   count as spawned and alive, as in the BP.
//! - Kills respawn after `Difficulty.AIRespawnTimer` seconds: one actor per kill
//!   (`EachIndividually`, the default) or the whole group once all are dead.
//! - The human AI's spawning volumes (BP_AISpawningVolume_Prefab in prefabs and quests, the
//!   level's 69 BP_AISpawningVolume_RandomChanceToSpawn) run on the same spawner, one group per
//!   `Spawning AI` element ([`SpawnerDef::human_volume`], [`SpawnerDef::random_chance`]); a
//!   random-chance volume is only there when its BeginPlay roll (RandomSpawn: 0-100 <= Chance)
//!   comes up.

use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;
use rand::Rng;

use super::collision::CollisionCache;
use super::{ActorInfo, Level, PropStreaming, Terrain};
use crate::gameplay::combat::{Dead, Health};
use crate::gameplay::zombies::ZombieKind;
use crate::menus::AppState;
use crate::menus::settings::SettingsStore;

/// A zombie spawned by the level; gameplay drives anything tagged with it.
#[derive(Component)]
pub struct LevelZombie {
    pub home: Vec3,
    /// Index of the spawner actor in [`Level::actors`] that made it, if any.
    pub spawner: Option<usize>,
    /// Its .glb and material override, so a culled zombie comes back looking the same.
    pub model: String,
    pub material: Option<String>,
    pub kind: ZombieKind,
}

/// Spawn a level zombie: the model with its material override, tagged for gameplay.
pub fn spawn_zombie(commands: &mut Commands, assets: &AssetServer, transform: Transform, zombie: LevelZombie) -> Entity {
    let mut e = commands.spawn((crate::models::model(assets, zombie.model.clone()), transform, DespawnOnExit(AppState::InGame)));
    if let Some(m) = &zombie.material {
        e.insert(crate::models::MaterialOverride(m.clone()));
    }
    e.insert(zombie).id()
}

/// `BP_MasterZombie`'s AIOSubject `OptimizationLayers` (metres). Beyond the last one
/// `OnOptimizationUpdate` hands the zombie to `K2_DespawnSubject`; the subsystem keeps it and
/// spawns it again when an invoker returns.
pub const OPTIMIZATION_LAYERS: [f32; 5] = [50.0, 75.0, 122.5, 150.0, 225.0];
/// `AIODeveloperSettings.OptimizationUpdateInterval`.
const OPTIMIZATION_INTERVAL: f32 = 0.5;
/// Distance at which a despawned subject comes back. The subsystem is native and its respawn
/// rule isn't readable; a little inside the last layer avoids flicker at the edge (guess).
const RESTORE_RADIUS: f32 = 210.0;

/// The layer a subject at `dist` is in, or `None` beyond the last (despawned).
pub fn optimization_layer(dist: f32) -> Option<usize> {
    OPTIMIZATION_LAYERS.iter().position(|&r| dist <= r)
}

/// A zombie the optimiser despawned, waiting for the player.
#[derive(Clone, Debug)]
struct Stored {
    transform: Transform,
    home: Vec3,
    spawner: Option<usize>,
    model: String,
    material: Option<String>,
    kind: ZombieKind,
    hp: Option<(f32, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RespawnMethod {
    None,
    WholeGroup,
    EachIndividually,
}

/// One spawner actor's settings: `BP_AISpawner_Master` class defaults, the subclass's, then the
/// level instance's overrides. Lengths are metres; the box is in the actor's frame.
#[derive(Clone, Debug)]
pub struct SpawnerDef {
    /// The instance's `SpawnedActorClass`.
    pub kind: ZombieKind,
    /// `SpawnType`'s difficulty row (`GetCVarType`): ZOMBIE / ZOMBIE HOUND SPAWN AMOUNT MULTIPLIER.
    pub multiplier_row: &'static str,
    pub pos: Vec3,
    /// Bevy-space unit axes of UE local X, Y, Z (the box ignores actor scale for its frame).
    pub axes: [Vec3; 3],
    /// Half extents along UE X, Y, Z, scaled by the actor scale (`GetScaledBoxExtent`).
    pub extent: Vec3,
    pub radius: f32,
    pub amount: u32,
    pub randomised: bool,
    pub initial_delay: f32,
    pub interval: f32,
    pub no_spawn_near_player: f32,
    pub respawn: RespawnMethod,
    pub skip_meshes: bool,
    /// `UseSpecifiedSpawnPoints`: box-local (UE cm) positions and UE yaw in degrees.
    pub points: Vec<(Vec3, f32)>,
    /// Tries at a spawn point before the spawn is given up (the volumes' Spawn Retries + 1).
    pub retries: u32,
    /// The volumes' UniqueLoot: DT_UniqueLoot sets added to each spawned AI's loot.
    pub unique: Vec<String>,
}

/// BP_AISpawningVolume_Prefab's AI Spawn Volume (BoxExtent 1000 x 1000 x 200 cm), SpawnRadius
/// (14000 cm) and InitialSpawnDelay (2 s).
const VOLUME_EXTENT: Vec3 = Vec3::new(10.0, 10.0, 2.0);
const VOLUME_RADIUS: f32 = 140.0;
const VOLUME_DELAY: f32 = 2.0;
/// The volumes' Spawn Retries (9): a spawn tries this many points.
const VOLUME_RETRIES: u32 = 10;
/// BP_AISpawningVolume_RandomChanceToSpawn's AI Spawn Volume (BoxExtent 1000 x 1000 x 64 cm,
/// 64.5 cm up), SpawnRadius (14000 cm), InitialSpawnDelay (8 s) and TraceForPlayer's 2000 cm.
const RANDOM_BOX_EXTENT: Vec3 = Vec3::new(1000.0, 1000.0, 64.0);
const RANDOM_BOX_OFFSET: Vec3 = Vec3::new(0.0, 0.0, 64.5);
const RANDOM_RADIUS: f32 = 14000.0;
const RANDOM_DELAY: f32 = 8.0;
const NO_SPAWN_NEAR: f32 = 20.0;
/// Spawner indices of the random-chance volumes' groups: this + actor index x 8 + element.
pub const RANDOM_VOLUME_KEYS: usize = 1 << 22;

impl SpawnerDef {
    /// One element of a BP_AISpawningVolume_Prefab's `Spawning AI` (AICharacter, SpawnCount) at
    /// the volume's transform: SpawnCount of them in its box once the survivor comes within
    /// SpawnRadius, each respawning on the AI respawn timer.
    pub fn human_volume(kind: ZombieKind, t: &Transform, count: u32) -> Self {
        let (scale, rot, pos) = (t.scale, t.rotation, t.translation);
        Self {
            kind,
            multiplier_row: "",
            pos,
            axes: [rot * Vec3::X, rot * Vec3::Z, rot * Vec3::Y],
            extent: VOLUME_EXTENT * Vec3::new(scale.x, scale.z, scale.y),
            radius: VOLUME_RADIUS,
            amount: count,
            randomised: false,
            initial_delay: VOLUME_DELAY,
            interval: 0.5,
            no_spawn_near_player: 0.0,
            respawn: RespawnMethod::EachIndividually,
            // Camps are built of props: the AI stands on them.
            skip_meshes: true,
            points: Vec::new(),
            retries: VOLUME_RETRIES,
            unique: Vec::new(),
        }
    }

    /// A level BP_AISpawningVolume_RandomChanceToSpawn: its `Chance` (%) and one group per
    /// `Spawning AI` element, as [`human_volume`](Self::human_volume) with the instance's AI
    /// Spawn Volume box, SpawnRadius and UniqueLoot. It never respawns (`Respawn AI` is off), it
    /// waits InitialSpawnDelay (8 s), a spawn may land on its `Floors` (otherwise only on the
    /// landscape) and never within 20 m of the survivor (TraceForPlayer) unless SkipPlayerCheck?.
    pub fn random_chance(a: &ActorInfo) -> Option<(f32, Vec<Self>)> {
        if a.class != "BP_AISpawningVolume_RandomChanceToSpawn_C" {
            return None;
        }
        let empty = HashMap::new();
        let props = a.props.as_ref().unwrap_or(&empty);
        let num = |k: &str, d: f32| props.get(k).and_then(|v| v.trim_matches('"').parse::<f32>().ok()).unwrap_or(d);
        let vec = |k: &str, d: Vec3| {
            props.get(k).and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok()).map_or(d, |v| {
                let f = |c: &str, d: f32| v.get(c).and_then(|x| x.as_f64()).map_or(d, |x| x as f32);
                Vec3::new(f("X", d.x), f("Y", d.y), f("Z", d.z))
            })
        };
        let elements = crate::level::prefabs::spawning_ai(props.get("Spawning AI")?);
        let t = crate::level::affine(&a.transform);
        // The box component sits at its RelativeLocation on the root.
        let rel = vec("AI Spawn Volume.RelativeLocation", RANDOM_BOX_OFFSET);
        let at = t.transform_point(Vec3::new(rel.x, rel.z, rel.y) / 100.0);
        let extent = vec("AI Spawn Volume.BoxExtent", RANDOM_BOX_EXTENT);
        let floors = props.get("Floors").is_some_and(|f| f.contains("ObjectName"));
        let player_check = props.get("SkipPlayerCheck?").is_none_or(|v| v != "true");
        let unique = props.get("UniqueLoot").and_then(|u| serde_json::from_str::<Vec<String>>(u).ok()).unwrap_or_default();
        let defs = elements
            .into_iter()
            .map(|(kind, count)| {
                let mut d = Self::human_volume(kind, &Transform { translation: at, ..t }, count);
                d.extent = extent / 100.0 * Vec3::new(t.scale.x, t.scale.z, t.scale.y);
                d.radius = num("SpawnRadius", RANDOM_RADIUS) / 100.0;
                d.initial_delay = RANDOM_DELAY;
                d.respawn = RespawnMethod::None;
                d.skip_meshes = floors;
                d.no_spawn_near_player = if player_check { NO_SPAWN_NEAR } else { 0.0 };
                d.unique = unique.clone();
                d
            })
            .collect();
        Some((num("Chance", 100.0), defs))
    }

    /// A BP_AISpawner_SettlementSurvivors (the master spawner with SpawnType 3) and the class it
    /// spawns (BP_RandomCivilian_Male / _Female: `gameplay::civilians`, not zombie classes).
    /// SkipPlayerCheck? turns TraceForPlayer off.
    pub fn settlers(a: &ActorInfo) -> Option<(String, Self)> {
        if a.class != "BP_AISpawner_SettlementSurvivors_C" {
            return None;
        }
        let empty = HashMap::new();
        let props = a.props.as_ref().unwrap_or(&empty);
        let class = props.get("SpawnedActorClass")?.trim_matches('"').to_string();
        let mut def = Self::build(a, ZombieKind::default(), 3, 12500.0, 5.0);
        if props.get("SkipPlayerCheck?").is_some_and(|v| v == "true") {
            def.no_spawn_near_player = 0.0;
        }
        Some((class, def))
    }

    pub fn from_actor(a: &ActorInfo) -> Option<Self> {
        // (The settlement survivors' spawners: `settlers`.)
        let (default_type, default_radius, default_amount) = match a.class.as_str() {
            // BP_AISpawner_Zombies overrides the master's SpawnRadius 12500; the hounds' and the
            // animals' don't (BP_AISpawner_Animals: SpawnType NewEnumerator4). The bosses'
            // spawns one (SpawnAmount 1, SpawnType NewEnumerator2).
            "BP_AISpawner_Zombies_C" => (0, 14000.0, 5.0),
            "BP_AISpawner_ZombieHounds_C" => (1, 12500.0, 5.0),
            "BP_AISpawner_ZombieBosses_C" => (2, 12500.0, 1.0),
            "BP_AISpawner_Animals_C" => (4, 12500.0, 5.0),
            _ => return None,
        };
        let empty = HashMap::new();
        let props = a.props.as_ref().unwrap_or(&empty);
        // No SpawnedActorClass: InitSpawner's IsValidClass fails and it never spawns. Classes
        // without data are skipped too.
        let kind = ZombieKind::from_class(props.get("SpawnedActorClass")?.trim_matches('"'))?;
        Some(Self::build(a, kind, default_type, default_radius, default_amount))
    }

    /// The master spawner's settings over the subclass's defaults and the instance's overrides.
    fn build(a: &ActorInfo, kind: ZombieKind, default_type: u8, default_radius: f32, default_amount: f32) -> Self {
        let empty = HashMap::new();
        let props = a.props.as_ref().unwrap_or(&empty);
        let spawn_type = props
            .get("SpawnType")
            .and_then(|v| v.trim_matches('"').rsplit("NewEnumerator").next()?.parse::<u8>().ok())
            .unwrap_or(default_type);
        let multiplier_row = match spawn_type {
            1 => "ZOMBIE HOUND SPAWN AMOUNT MULTIPLIER",
            2 => "ZOMBIE BOSS SPAWN AMOUNT MULTIPLIER",
            // GetCVarType: 3 Difficulty.SettlementSurvivorAmount (no menu row: 1), 4 Difficulty.AnimalAmount.
            3 => "",
            4 => "ANIMAL SPAWN AMOUNT MULTIPLIER",
            _ => "ZOMBIE SPAWN AMOUNT MULTIPLIER",
        };
        let t = &a.transform;
        // Bevy columns: 0 = UE X, 1 = UE Z (up), 2 = UE Y.
        let (c0, c1, c2) = (Vec3::new(t[0], t[1], t[2]), Vec3::new(t[3], t[4], t[5]), Vec3::new(t[6], t[7], t[8]));
        let num = |k: &str, d: f32| props.get(k).and_then(|v| v.trim_matches('"').parse::<f32>().ok()).unwrap_or(d);
        let flag = |k: &str, d: bool| props.get(k).map_or(d, |v| v == "true");
        let text = |k: &str| props.get(k).map(|v| v.trim_matches('"').to_string()).unwrap_or_default();
        let respawn = match text("RespawnMethod") {
            s if s.is_empty() || s.ends_with("EachIndividually") => RespawnMethod::EachIndividually,
            s if s.ends_with("None") || s.ends_with("NoRespawn") => RespawnMethod::None,
            _ => RespawnMethod::WholeGroup,
        };
        let points = if text("SelectingSpawnPointsMethod").ends_with("UseSpecifiedSpawnPoints") {
            parse_points(props.get("SpawnPoints").map_or("[]", |s| s.as_str()))
        } else {
            Vec::new()
        };
        Self {
            kind,
            multiplier_row,
            pos: a.position(),
            axes: [c0.normalize_or(Vec3::X), c2.normalize_or(Vec3::Z), c1.normalize_or(Vec3::Y)],
            extent: Vec3::new(
                num("Box Extent X", 1500.0) * c0.length(),
                num("Box Extent Y", 1500.0) * c2.length(),
                num("Box Extent Z", 200.0) * c1.length(),
            ) / 100.0,
            radius: num("SpawnRadius", default_radius) / 100.0,
            amount: num("SpawnAmount", default_amount).max(0.0) as u32,
            randomised: flag("RandomisedSpawnCount", true),
            initial_delay: num("InitialSpawnDelay", 9.0),
            interval: num("SpawningInterval", 0.5),
            no_spawn_near_player: num("DontSpawnCloseToPlayerRange", 2500.0) / 100.0,
            respawn,
            skip_meshes: flag("SkipMeshes?", false),
            points,
            retries: 1,
            unique: Vec::new(),
        }
    }

    /// Box-local UE centimetres to world metres.
    pub fn local_to_world(&self, l: Vec3) -> Vec3 {
        self.pos + (self.axes[0] * l.x + self.axes[1] * l.y + self.axes[2] * l.z) / 100.0
    }

    /// Bevy yaw (about +Y, forward +Z) of a UE local yaw in degrees.
    pub fn world_yaw(&self, ue_yaw_deg: f32) -> f32 {
        let (s, c) = ue_yaw_deg.to_radians().sin_cos();
        let d = self.axes[0] * c + self.axes[1] * s;
        d.x.atan2(d.z)
    }
}

fn parse_points(json: &str) -> Vec<(Vec3, f32)> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    list.iter()
        .filter_map(|p| {
            let t = p.get("Transform")?;
            let tr = t.get("Translation")?;
            let f = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            let r = t.get("Rotation")?;
            // Spawn points only carry yaw (rotation about UE Z).
            let yaw = 2.0 * f(r, "Z").atan2(f(r, "W"));
            Some((Vec3::new(f(tr, "X"), f(tr, "Y"), f(tr, "Z")), yaw.to_degrees()))
        })
        .collect()
}

/// `SetSpawnCount`: randomise between trunc(N/1.5) and trunc(N*1.5) (unless N is 1 or the
/// spawner opts out), then scale by the difficulty's spawn multiplier, truncating.
pub fn spawn_count(amount: u32, randomised: bool, multiplier: f32, rng: &mut impl Rng) -> u32 {
    let n = if randomised && amount != 1 {
        let (lo, hi) = ((amount as f64 / 1.5) as u32, (amount as f64 * 1.5) as u32);
        rng.gen_range(lo..=hi)
    } else {
        amount
    };
    (n as f32 * multiplier).max(0.0) as u32
}

/// A spawner's runtime state (the BP's timers, queues and counters).
#[derive(Default, Debug)]
pub struct SpawnerRuntime {
    started: bool,
    /// `SpawnAmount` after `SetSpawnCount`.
    pub amount: u32,
    /// Waiting for the player in `SpawnRadius` (TimerHandle_CheckSpawnRadius).
    watching: bool,
    check_in: f32,
    check_period: f32,
    /// SpawnGroupsQueue: group sizes; `in_group` counts the head's spawns.
    queue: VecDeque<u32>,
    in_group: u32,
    cooldown: f32,
    /// RespawnGroupsQueue: (game time due, amount).
    respawns: Vec<(f64, u32)>,
    pub total_spawned: u32,
    pub alive: u32,
}

impl SpawnerRuntime {
    pub fn started(&self) -> bool {
        self.started
    }

    pub fn begin(&mut self, def: &SpawnerDef, multiplier: f32, rng: &mut impl Rng) {
        // `multiplier`: the difficulty row for `def.kind` (ZombieKind::spawn_multiplier).
        self.started = true;
        self.amount = spawn_count(def.amount, def.randomised, multiplier, rng);
        if self.amount == 0 {
            return; // InitSpawner: nothing to spawn, no radius timer
        }
        self.watching = true;
        self.check_period = rng.gen_range(0.75..2.0);
        self.check_in = def.initial_delay + self.check_period;
    }

    /// Advance timers; returns how many spawn attempts happen this frame (0 or 1).
    pub fn tick(&mut self, def: &SpawnerDef, dt: f32, now: f64, in_radius: bool) -> u32 {
        if self.watching {
            self.check_in -= dt;
            if self.check_in <= 0.0 {
                self.check_in += self.check_period;
                if in_radius {
                    self.watching = false;
                    self.queue.push_back(self.amount);
                }
            }
        }
        // UpdateRespawningGroups (the BP polls every second; due groups join the spawn queue).
        let queue = &mut self.queue;
        self.respawns.retain(|&(due, n)| {
            if now >= due {
                queue.push_back(n);
                false
            } else {
                true
            }
        });
        // CanSpawnActors: a queued group and the player in range. (The BP's loop stalls if the
        // player leaves mid-group until another group is queued; here it resumes on return.)
        self.cooldown = (self.cooldown - dt).max(0.0);
        if self.queue.is_empty() || !in_radius || self.cooldown > 0.0 {
            return 0;
        }
        self.total_spawned += 1;
        self.alive += 1;
        self.in_group += 1;
        if self.queue.front().is_some_and(|&n| self.in_group >= n) {
            self.queue.pop_front();
            self.in_group = 0;
        }
        self.cooldown = def.interval;
        1
    }

    /// `OnSpawnedActorKilled` -> `UpdateSpawnerAfterActorWasKilled`.
    pub fn killed(&mut self, def: &SpawnerDef, now: f64, respawn_timer: f32) {
        self.alive = self.alive.saturating_sub(1);
        let due = now + respawn_timer as f64;
        match def.respawn {
            RespawnMethod::None => {}
            RespawnMethod::EachIndividually => self.respawns.push((due, 1)),
            RespawnMethod::WholeGroup => {
                if self.total_spawned >= self.amount && self.alive == 0 && self.amount > 0 {
                    self.respawns.push((due, self.amount));
                }
            }
        }
    }

    pub fn pending_respawns(&self) -> u32 {
        self.respawns.iter().map(|r| r.1).sum()
    }
}

/// All zombie spawners of the level and their state; reset with the world.
#[derive(Resource, Default)]
pub struct Spawners {
    defs: Vec<(usize, SpawnerDef)>,
    pub runtime: HashMap<usize, SpawnerRuntime>,
    stored: Vec<Stored>,
    optimize_in: f32,
    clock: f64,
    loaded: bool,
}

impl Spawners {
    /// A spawner that came with a prefab (`prefabs::SPAWNER_KEYS` and up).
    pub fn add(&mut self, index: usize, def: SpawnerDef) {
        self.defs.push((index, def));
    }

    /// A spawner taken away (a quest's volume when the quest is forgotten).
    pub fn remove(&mut self, index: usize) {
        self.defs.retain(|d| d.0 != index);
        self.runtime.remove(&index);
    }

    pub fn has(&self, index: usize) -> bool {
        self.defs.iter().any(|d| d.0 == index)
    }

    /// The UniqueLoot sets a spawner adds to what it spawns.
    pub fn unique(&self, index: usize) -> &[String] {
        self.defs.iter().find(|d| d.0 == index).map_or(&[], |d| d.1.unique.as_slice())
    }

    /// The spawner groups (index, settings).
    pub fn defs(&self) -> impl Iterator<Item = &(usize, SpawnerDef)> {
        self.defs.iter()
    }
}

fn difficulty(store: &SettingsStore, label: &str, default: f32) -> f32 {
    match store.find(&crate::menus::settings::row_key("difficulty", "AI", label)) {
        Some(crate::menus::settings::Value::Num(v)) => v,
        _ => default,
    }
}

/// `ProjectLocation`: trace down 2.5x the box's half height from `top`. The landscape (or any
/// surface when `SkipMeshes?`) is a hit; landing on a prop otherwise fails the spawn. The BP adds
/// `ProjectionZOffset` (1 m) for the capsule centre; our characters stand on their feet.
pub(crate) fn project(def: &SpawnerDef, top: Vec3, terrain: &Terrain, prop_floor: Option<f32>) -> Option<Vec3> {
    let bottom = top.y - def.extent.z * 2.5;
    let ground = terrain.height(top.x, top.z);
    let land = (ground >= bottom && ground <= top.y).then_some(ground);
    match (land, prop_floor) {
        (_, Some(p)) if land.is_none_or(|g| p > g + 0.05) => def.skip_meshes.then_some(Vec3::new(top.x, p, top.z)),
        (Some(g), _) => Some(Vec3::new(top.x, g, top.z)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_spawners(
    mut commands: Commands,
    time: Res<Time>,
    level: Option<Res<Level>>,
    terrain: Option<Res<Terrain>>,
    streaming: Option<Res<PropStreaming>>,
    meshes: Res<Assets<Mesh>>,
    mut cache: ResMut<CollisionCache>,
    store: Res<SettingsStore>,
    mut spawners: ResMut<Spawners>,
    players: Query<&Transform, With<crate::models::CharacterModel>>,
    killed: Query<&LevelZombie, Added<Dead>>,
    assets: Res<AssetServer>,
) {
    let (Some(level), Some(terrain)) = (level, terrain) else { return };
    let Some(player) = players.iter().next().map(|t| t.translation) else { return };
    let s = &mut *spawners;
    if !s.loaded {
        s.loaded = true;
        // (Prefabs' spawners may have been added first: `Spawners::add`.)
        s.defs.extend(level.actors.iter().enumerate().filter_map(|(i, a)| SpawnerDef::from_actor(a).map(|d| (i, d))));
        // RandomSpawn at BeginPlay: RandomFloatInRange(0, 100) <= Chance, else the volume is destroyed.
        let mut rng = rand::thread_rng();
        let (mut volumes, mut groups) = (0, 0);
        for (i, a) in level.actors.iter().enumerate() {
            let Some((chance, defs)) = SpawnerDef::random_chance(a) else { continue };
            if rng.gen_range(0.0..100.0) > chance {
                continue;
            }
            volumes += 1;
            for (k, d) in defs.into_iter().enumerate().take(8) {
                s.defs.push((RANDOM_VOLUME_KEYS + i * 8 + k, d));
                groups += 1;
            }
        }
        info!("random-chance AI volumes: {volumes} rolled in ({groups} groups)");
    }
    let dt = time.delta_secs();
    s.clock += dt as f64;
    let now = s.clock;
    let mut rng = rand::thread_rng();

    let respawn_timer = difficulty(&store, "AI RESPAWN TIMER (SECONDS)", 1500.0);

    let defs: HashMap<usize, &SpawnerDef> = s.defs.iter().map(|(i, d)| (*i, d)).collect();
    for z in &killed {
        if let Some(i) = z.spawner
            && let (Some(def), Some(rt)) = (defs.get(&i), s.runtime.get_mut(&i))
        {
            rt.killed(def, now, respawn_timer);
        }
    }

    for (i, def) in &s.defs {
        let rt = s.runtime.entry(*i).or_default();
        if !rt.started {
            rt.begin(def, difficulty(&store, def.multiplier_row, 1.0), &mut rng);
        }
        let in_radius = def.pos.distance_squared(player) <= def.radius * def.radius;
        let was_watching = rt.watching;
        let attempts = rt.tick(def, dt, now, in_radius);
        if was_watching && !rt.watching {
            info!("zombie spawner {i} woke: group of {}", rt.amount);
        }
        for _ in 0..attempts {
            let (at, yaw) = if def.points.is_empty() {
                let e = def.extent * 100.0;
                // Spawn Location Trace: up to Spawn Retries more points when one misses.
                let found = (0..def.retries.max(1)).find_map(|_| {
                    let local = Vec3::new(rng.gen_range(-e.x..=e.x), rng.gen_range(-e.y..=e.y), e.z);
                    let top = def.local_to_world(local);
                    let prop = streaming.as_ref().and_then(|st| cache.prop_floor(&level, st, &meshes, top.xz(), (top.y - def.extent.z * 2.5, top.y)));
                    project(def, top, &terrain, prop)
                });
                let Some(at) = found else { continue };
                (at, rng.gen_range(0.0..std::f32::consts::TAU))
            } else {
                // GetSpawnPointAtIndex(TotalSpawned % SpawnAmount), random when out of range.
                let k = (rt.total_spawned - 1) % rt.amount.max(1);
                let (p, yaw) = def.points.get(k as usize).copied().unwrap_or(def.points[rng.gen_range(0..def.points.len())]);
                let mut at = def.local_to_world(p);
                at.y = at.y.max(terrain.height(at.x, at.z));
                (at, def.world_yaw(yaw))
            };
            // TraceForPlayer: never pop up right next to the player.
            if at.distance(player) <= def.no_spawn_near_player {
                continue;
            }
            let (model, material) = def.kind.pick_look(&mut rng);
            let zombie = LevelZombie { home: at, spawner: Some(*i), model, material, kind: def.kind };
            spawn_zombie(&mut commands, &assets, Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)), zombie);
        }
    }
}

/// The AIOptimizer side: every `OptimizationUpdateInterval`, living zombies beyond the last
/// optimisation layer are despawned into storage (keeping position, home and hit points), and
/// stored ones within [`RESTORE_RADIUS`] come back. Spawners still count them as alive.
pub(super) fn optimize_subjects(
    mut commands: Commands,
    time: Res<Time>,
    mut spawners: ResMut<Spawners>,
    players: Query<&Transform, With<crate::models::CharacterModel>>,
    zombies: Query<(Entity, &Transform, &LevelZombie, Option<&Health>), Without<Dead>>,
    assets: Res<AssetServer>,
) {
    let Some(player) = players.iter().next().map(|t| t.translation) else { return };
    spawners.optimize_in -= time.delta_secs();
    if spawners.optimize_in > 0.0 {
        return;
    }
    spawners.optimize_in = OPTIMIZATION_INTERVAL;
    for (e, t, z, hp) in &zombies {
        if optimization_layer(t.translation.distance(player)).is_none() {
            spawners.stored.push(Stored { transform: *t, home: z.home, spawner: z.spawner, model: z.model.clone(), material: z.material.clone(), kind: z.kind, hp: hp.map(|h| (h.hp, h.max)) });
            commands.entity(e).despawn();
        }
    }
    let (back, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut spawners.stored).into_iter().partition(|s| s.transform.translation.distance(player) <= RESTORE_RADIUS);
    spawners.stored = keep;
    for s in back {
        let e = spawn_zombie(&mut commands, &assets, s.transform, LevelZombie { home: s.home, spawner: s.spawner, model: s.model, material: s.material, kind: s.kind });
        if let Some((hp, max)) = s.hp {
            commands.entity(e).insert(Health { hp, max });
        }
    }
}

impl Spawners {
    /// Zombies currently held by the optimiser (despawned for distance).
    pub fn stored(&self) -> usize {
        self.stored.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn actor(props: &[(&str, &str)]) -> ActorInfo {
        let mut p: HashMap<String, String> = props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        p.entry("SpawnedActorClass".into()).or_insert_with(|| "BP_Zombie_Roamer_C".into());
        ActorInfo {
            class: "BP_AISpawner_Zombies_C".into(),
            name: "s".into(),
            transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 20.0],
            props: Some(p),
        }
    }

    #[test]
    fn random_chance_volumes_read_their_instance() {
        if crate::gameplay::zombies::zombie_classes().iter().all(|(n, _)| n != "BP_MasterTrader_C") {
            return;
        }
        let ai = r#"[{"AICharacter_8":{"ObjectName":"BlueprintGeneratedClass'BP_Bandit_Melee_C'"},"SpawnCount_5":2},{"AICharacter_8":{"ObjectName":"BlueprintGeneratedClass'BP_MasterTrader_C'"},"SpawnCount_5":1},{"AICharacter_8":{"ObjectName":"BlueprintGeneratedClass'BP_Scavenger_Dog_C'"},"SpawnCount_5":1}]"#;
        let props: HashMap<String, String> = [
            ("Spawning AI", ai),
            ("Chance", "25.0"),
            ("AI Spawn Volume.BoxExtent", r#"{"X":250.0,"Y":500.0,"Z":64.0}"#),
            ("SpawnRadius", "3500.0"),
            ("UniqueLoot", r#"["BanditSet"]"#),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let a = ActorInfo { class: "BP_AISpawningVolume_RandomChanceToSpawn_C".into(), name: "v".into(), transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 20.0], props: Some(props) };
        let (chance, defs) = SpawnerDef::random_chance(&a).unwrap();
        assert_eq!(chance, 25.0);
        // The dog isn't ported; the bandits and the trader are a group each.
        assert_eq!(defs.iter().map(|d| (d.kind.class_name(), d.amount)).collect::<Vec<_>>(), [("BP_Bandit_Melee_C", 2), ("BP_MasterTrader_C", 1)]);
        let d = &defs[0];
        // UE X/Y/Z half extents in m; the box 64.5 cm up; never respawning; 20 m from the player.
        assert_eq!((d.extent, d.radius, d.initial_delay, d.respawn), (Vec3::new(2.5, 5.0, 0.64), 35.0, 8.0, RespawnMethod::None));
        assert!((d.pos - Vec3::new(10.0, 0.645, 20.0)).length() < 1e-4, "{}", d.pos);
        assert_eq!((d.no_spawn_near_player, d.skip_meshes, d.retries), (20.0, false, 10));
        assert_eq!(d.unique, ["BanditSet"]);
        // Other classes aren't volumes.
        assert!(SpawnerDef::random_chance(&actor(&[])).is_none());
    }

    #[test]
    fn spawned_class_and_spawn_type_come_from_the_instance() {
        if crate::gameplay::zombies::zombie_classes().is_empty() {
            return;
        }
        let roamer = SpawnerDef::from_actor(&actor(&[])).unwrap();
        assert_eq!(roamer.kind.class_name(), "BP_Zombie_Roamer_C");
        assert_eq!(roamer.multiplier_row, "ZOMBIE SPAWN AMOUNT MULTIPLIER");
        // A zombie spawner set to hounds (as 9 in the map are) uses the hound row.
        let dogs = SpawnerDef::from_actor(&actor(&[("SpawnedActorClass", "BP_Zombie_Dog_C"), ("SpawnType", "\"Enum_SpawnType::NewEnumerator1\"")])).unwrap();
        assert_eq!((dogs.kind, dogs.multiplier_row), (ZombieKind::hound(), "ZOMBIE HOUND SPAWN AMOUNT MULTIPLIER"));
        // No class: IsValidClass fails, nothing spawns.
        let mut none = actor(&[]);
        none.props.as_mut().unwrap().remove("SpawnedActorClass");
        assert!(SpawnerDef::from_actor(&none).is_none());
        assert!(SpawnerDef::from_actor(&actor(&[("SpawnedActorClass", "BP_NoSuchZombie_C")])).is_none());
    }

    #[test]
    fn defaults_and_overrides() {
        let d = SpawnerDef::from_actor(&actor(&[])).unwrap();
        assert_eq!((d.amount, d.radius, d.respawn), (5, 140.0, RespawnMethod::EachIndividually));
        assert_eq!(d.extent, Vec3::new(15.0, 15.0, 2.0));
        assert_eq!(d.no_spawn_near_player, 25.0);
        let d = SpawnerDef::from_actor(&actor(&[("SpawnAmount", "3"), ("SpawnRadius", "10000.0"), ("Box Extent Y", "600.0"), ("RandomisedSpawnCount", "false")])).unwrap();
        assert_eq!((d.amount, d.radius, d.extent.y, d.randomised), (3, 100.0, 6.0, false));
        let mut hound = actor(&[("SpawnedActorClass", "BP_Zombie_Dog_C")]);
        hound.class = "BP_AISpawner_ZombieHounds_C".into();
        let h = SpawnerDef::from_actor(&hound).unwrap();
        assert_eq!((h.radius, h.multiplier_row), (125.0, "ZOMBIE HOUND SPAWN AMOUNT MULTIPLIER"), "hounds keep the master's radius");
        hound.class = "BP_AISpawner_SettlementSurvivors_C".into();
        assert!(SpawnerDef::from_actor(&hound).is_none());
    }

    #[test]
    fn boss_spawners_place_one_boss() {
        if crate::gameplay::zombies::ZombieKind::from_class("BP_ZombieBoss_Military_Small_C").is_none() {
            return;
        }
        // As the level's BP_AISpawner_ZombieBosses_C_17 is set.
        let mut boss = actor(&[("SpawnedActorClass", "BP_ZombieBoss_Military_Small_C"), ("SpawnRadius", "2500.0"), ("DontSpawnCloseToPlayerRange", "500.0")]);
        boss.class = "BP_AISpawner_ZombieBosses_C".into();
        let d = SpawnerDef::from_actor(&boss).unwrap();
        assert_eq!((d.amount, d.radius, d.no_spawn_near_player, d.multiplier_row), (1, 25.0, 5.0, "ZOMBIE BOSS SPAWN AMOUNT MULTIPLIER"));
        assert!(d.kind.boss().is_some());
        // SetSpawnCount leaves a single spawn alone; the multiplier then truncates (0.5 -> none).
        let mut rng = rand::thread_rng();
        assert_eq!((spawn_count(1, true, 1.0, &mut rng), spawn_count(1, true, 0.5, &mut rng), spawn_count(1, true, 2.0, &mut rng)), (1, 0, 2));
    }

    #[test]
    fn animal_spawners_use_the_animal_multiplier() {
        if crate::gameplay::zombies::ZombieKind::from_class("BP_AnimalDeer_C").is_none() {
            return;
        }
        let mut deer = actor(&[("SpawnedActorClass", "BP_AnimalDeer_C")]);
        deer.class = "BP_AISpawner_Animals_C".into();
        let d = SpawnerDef::from_actor(&deer).unwrap();
        assert_eq!((d.multiplier_row, d.radius), ("ANIMAL SPAWN AMOUNT MULTIPLIER", 125.0));
        assert!(d.kind.animal().is_some());
    }

    #[test]
    fn box_axes_follow_the_actor() {
        // UE local X maps to Bevy column 0, UE Y to column 2, UE Z (up) to column 1.
        let d = SpawnerDef::from_actor(&actor(&[])).unwrap();
        assert_eq!(d.local_to_world(Vec3::new(100.0, 200.0, 300.0)), Vec3::new(11.0, 3.0, 22.0));
        // UE yaw 0 faces local X (Bevy +X here): atan2(1, 0) = 90 degrees.
        assert!((d.world_yaw(0.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!(d.world_yaw(90.0).abs() < 1e-5);
    }

    #[test]
    fn spawn_points_parse() {
        let pts = parse_points(r#"[{"Transform":{"Rotation":{"X":0.0,"Y":0.0,"Z":0.70710677,"W":0.70710677},"Translation":{"X":1.0,"Y":2.0,"Z":3.0}}}]"#);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0].0, Vec3::new(1.0, 2.0, 3.0));
        assert!((pts[0].1 - 90.0).abs() < 1e-3);
    }

    #[test]
    fn spawn_count_matches_set_spawn_count() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        assert_eq!(spawn_count(5, false, 1.0, &mut rng), 5);
        assert_eq!(spawn_count(5, false, 2.5, &mut rng), 12);
        assert_eq!(spawn_count(1, true, 1.0, &mut rng), 1);
        assert_eq!(spawn_count(5, true, 0.0, &mut rng), 0);
        for _ in 0..200 {
            let n = spawn_count(5, true, 1.0, &mut rng);
            assert!((3..=7).contains(&n), "{n}");
        }
    }

    fn run(rt: &mut SpawnerRuntime, d: &SpawnerDef, secs: f32, now: &mut f64, in_radius: bool) -> u32 {
        let mut n = 0;
        for _ in 0..(secs * 10.0) as u32 {
            *now += 0.1;
            n += rt.tick(d, 0.1, *now, in_radius);
        }
        n
    }

    #[test]
    fn waits_for_the_player_then_spawns_one_group() {
        let d = SpawnerDef::from_actor(&actor(&[("RandomisedSpawnCount", "false")])).unwrap();
        let mut rt = SpawnerRuntime::default();
        rt.begin(&d, 1.0, &mut rand::rngs::StdRng::seed_from_u64(2));
        let mut now = 0.0;
        assert_eq!(run(&mut rt, &d, 30.0, &mut now, false), 0);
        // In range: the next radius check (within 2 s) queues 5, spawned one per 0.5 s.
        let first = run(&mut rt, &d, 2.1, &mut now, true);
        assert!((1..=4).contains(&first), "{first}");
        assert_eq!(first + run(&mut rt, &d, 10.0, &mut now, true), 5);
        assert_eq!((rt.total_spawned, rt.alive), (5, 5));
        // No second group while they live, even away and back.
        assert_eq!(run(&mut rt, &d, 10.0, &mut now, false) + run(&mut rt, &d, 10.0, &mut now, true), 0);
    }

    #[test]
    fn initial_delay_holds_the_first_group() {
        let d = SpawnerDef::from_actor(&actor(&[])).unwrap();
        let mut rt = SpawnerRuntime::default();
        rt.begin(&d, 1.0, &mut rand::rngs::StdRng::seed_from_u64(3));
        let mut now = 0.0;
        assert_eq!(run(&mut rt, &d, 9.0, &mut now, true), 0);
        assert!(run(&mut rt, &d, 3.0, &mut now, true) > 0);
    }

    #[test]
    fn kills_respawn_after_the_timer() {
        let d = SpawnerDef::from_actor(&actor(&[("RandomisedSpawnCount", "false"), ("SpawnAmount", "2")])).unwrap();
        let mut rt = SpawnerRuntime::default();
        rt.begin(&d, 1.0, &mut rand::rngs::StdRng::seed_from_u64(4));
        let mut now = 0.0;
        assert_eq!(run(&mut rt, &d, 15.0, &mut now, true), 2);
        rt.killed(&d, now, 60.0);
        assert_eq!((rt.alive, rt.pending_respawns()), (1, 1));
        assert_eq!(run(&mut rt, &d, 59.0, &mut now, true), 0);
        // Due, but only spawned while the player is in range.
        assert_eq!(run(&mut rt, &d, 2.0, &mut now, false), 0);
        assert_eq!(run(&mut rt, &d, 1.0, &mut now, true), 1);
        assert_eq!(rt.alive, 2);
    }

    #[test]
    fn whole_group_waits_for_the_last_kill() {
        let mut d = SpawnerDef::from_actor(&actor(&[("RandomisedSpawnCount", "false"), ("SpawnAmount", "2")])).unwrap();
        d.respawn = RespawnMethod::WholeGroup;
        let mut rt = SpawnerRuntime::default();
        rt.begin(&d, 1.0, &mut rand::rngs::StdRng::seed_from_u64(5));
        let mut now = 0.0;
        run(&mut rt, &d, 15.0, &mut now, true);
        rt.killed(&d, now, 10.0);
        assert_eq!(rt.pending_respawns(), 0);
        rt.killed(&d, now, 10.0);
        assert_eq!(rt.pending_respawns(), 2);
        assert_eq!(run(&mut rt, &d, 12.0, &mut now, true), 2);
    }

    #[test]
    fn optimization_layers_match_the_zombie_bp() {
        assert_eq!(optimization_layer(10.0), Some(0));
        assert_eq!(optimization_layer(60.0), Some(1));
        assert_eq!(optimization_layer(100.0), Some(2));
        assert_eq!(optimization_layer(140.0), Some(3));
        assert_eq!(optimization_layer(225.0), Some(4));
        assert_eq!(optimization_layer(226.0), None);
        // Coming back happens inside the last layer, so a restored zombie isn't culled at once.
        assert!(optimization_layer(RESTORE_RADIUS).is_some());
    }

    #[test]
    fn zero_multiplier_never_spawns() {
        let d = SpawnerDef::from_actor(&actor(&[])).unwrap();
        let mut rt = SpawnerRuntime::default();
        rt.begin(&d, 0.0, &mut rand::rngs::StdRng::seed_from_u64(6));
        let mut now = 0.0;
        assert_eq!(run(&mut rt, &d, 60.0, &mut now, true), 0);
    }
}
