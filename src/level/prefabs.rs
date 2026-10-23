//! Prefabs: BP_PrefabSpawner and BP_PrefabSpawner_Dynamic (decompiled) placing a PrefabMaster
//! Blueprint (`assets/data/prefabs.json`, `extractor/gen_prefabs.py`): abandoned camps, convoys,
//! crashed helicopters and the plane, the quarantine tents...
//!
//! - **The level's 51 spawners** (BeginPlay): Random Check (with RandomPossibilityToSpawn?: a roll
//!   of 0-100 at most Chance x PrefabTypeChanceMulitplier: Helicopter Crash and Other x RANDOM
//!   EVENT SPAWN CHANCE MULTIPLIER, Scavenger / Coyotes / Bandit Camp x their camp multipliers,
//!   Quest x 1), then CheckSurroundingArea (not with the survivor within 25 m; the 50 m check for
//!   player buildings has nothing to find), then the prefab spawns at the spawner's transform and,
//!   with Marker? and RANDOM EVENTS HAVE A MARKER?, its BP_Marker goes on the map (the instance's
//!   Marker Texture and Title, "A random event has occured in The Valley...", size 3.5), taken
//!   off once the survivor is within DistanceToRemoveMarker (20 m). Rolled each session: the
//!   prefabs' containers are Skip Save?.
//! - **Dynamic events** (BP_EventManager's DynamicRandomEventSpawn over the 39 dynamic spawners,
//!   shuffled): every 60-4800 s, once more than 3 days are survived, the next one that's out of
//!   the survivor's view (IsOutsideOfPlayerView?: the sight line is blocked, or it's more than
//!   60° off the camera's forward) spawns after DelayStart (3 s), with SoundOnSpawn
//!   (explosion_deep_low_1) and its marker.
//! - **What spawns**: the prefab's meshes (in the level's cells, hidden per session until their
//!   prefab spawns: `PropStreaming::removed`), its containers (searchable like the level's, keys
//!   from [`CONTAINER_KEYS`]), its zombie spawners (BP_AISpawner_Zombies / ZombieBosses, with the
//!   child actor template's overrides), its particle systems; PrefabMaster's RemoveFoliageComp
//!   takes out the harvestable trees and rocks within FoliageRemoverRadius (the foliage we can
//!   tell apart; bushes stay).
//!
//! - **The settlement** (BP_SettlementManager decompiled): one of its 4 Camps
//!   (BP_SettlementCampSpawner: their Settlement prefab) picked at random once a game and kept in
//!   the save (SettlementSpawned?, CampToSpawn), spawned at BeginPlay. Its traders (Safe Zone and
//!   Black Market) stand and trade like the level's, its BP_EnterSafeZone is a safe zone and its
//!   well a water well (`gameplay::zones`), its crafting and cooking stations work, its
//!   scavenger volume rolls like the level's random-chance volumes, and its BP_Marker puts
//!   "Scavenger Safe Zone" (the SD logo) on the map.
//!
//! Not ported: prefabs whose humans aren't all ported stay unspawned (by chance); the
//! settlement's settlers (BP_AISpawner_SettlementSurvivors: BP_RandomCivilian, a BP_MainFriendly
//! AI), its quest giver (BP_QuestGiver_SettlementOfficer), turret and barrier gate; the other
//! child actors (notes, radios, road-light generators, turrets, ladders, lights, the radiation
//! sphere) are drawn but do nothing.

use std::collections::HashMap;

use bevy::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;
use serde::Deserialize;

use super::{ActorInfo, Level, PropKey, PropStreaming, affine};
use crate::menus::AppState;
use crate::menus::settings::SettingsStore;

/// Prefab containers' keys: per site, 64 a site, above the corpses' range.
pub const CONTAINER_KEYS: usize = 3 << 32;
/// Prefab zombie spawners' indices in `spawners::Spawners`, above any level actor's.
pub const SPAWNER_KEYS: usize = 1 << 24;
/// CheckSurroundingArea's sphere around the spawner the survivor mustn't be in.
const PLAYER_CLEARANCE: f32 = 25.0;
/// DynamicRandomEventSpawn's timer and its days-survived gate.
const DYNAMIC_PERIOD: [f32; 2] = [60.0, 4800.0];
const DYNAMIC_AFTER_DAYS: u32 = 3;

#[derive(Deserialize, Clone, Debug)]
pub struct PrefabActor {
    pub class: String,
    pub transform: [f32; 12],
    #[serde(default)]
    pub props: HashMap<String, String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct PrefabDef {
    pub meshes: Vec<(String, [f32; 12])>,
    pub actors: Vec<PrefabActor>,
    #[serde(default)]
    pub particles: Vec<(String, [f32; 12])>,
    #[serde(default)]
    pub humans: Vec<String>,
    pub foliage_radius: f32,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct SpawnerDefaults {
    pub chance: f32,
    pub random: bool,
    pub delay_start: f32,
    pub marker_texture: Option<String>,
    pub marker_title: String,
    pub marker_description: String,
    pub remove_marker_within: f32,
    pub sound: Option<String>,
    #[serde(default)]
    pub textures: HashMap<String, String>,
}

#[derive(Deserialize, Default)]
pub struct PrefabData {
    pub prefabs: HashMap<String, PrefabDef>,
    #[serde(default)]
    pub spawner: SpawnerDefaults,
}

pub fn data() -> &'static PrefabData {
    static DATA: std::sync::OnceLock<PrefabData> = std::sync::OnceLock::new();
    DATA.get_or_init(|| std::fs::read_to_string("assets/data/prefabs.json").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default())
}

/// Enum_PrefabType.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefabType {
    HelicopterCrash,
    ScavengerCamp,
    CoyotesCamp,
    BanditCamp,
    Other,
    Quest,
}

impl PrefabType {
    fn parse(text: Option<&str>) -> Self {
        match text.unwrap_or("").trim_matches('"').rsplit("NewEnumerator").next().and_then(|n| n.parse::<u8>().ok()) {
            Some(1) => Self::ScavengerCamp,
            Some(2) => Self::CoyotesCamp,
            Some(3) => Self::BanditCamp,
            Some(4) => Self::Other,
            Some(5) => Self::Quest,
            _ => Self::HelicopterCrash,
        }
    }

    /// PrefabTypeChanceMulitplier: the difficulty row scaling its Chance (page, label).
    pub fn multiplier_row(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::HelicopterCrash | Self::Other => Some(("General", "RANDOM EVENT SPAWN CHANCE MULTIPLIER")),
            Self::ScavengerCamp => Some(("AI", "SCAVENGER CAMP SPAWN CHANCE MULTIPLIER")),
            Self::CoyotesCamp => Some(("AI", "MILITARY CAMP SPAWN CHANCE MULTIPLIER")),
            Self::BanditCamp => Some(("AI", "BANDIT CAMP SPAWN CHANCE MULTIPLIER")),
            Self::Quest => None,
        }
    }
}

/// A prefab spawner in the level, with its prefab's meshes placed in the cells.
#[derive(Clone, Debug)]
pub struct PrefabSite {
    /// The spawner's index in `Level::actors`.
    pub actor: usize,
    pub prefab: String,
    pub dynamic: bool,
    pub chance: f32,
    pub random: bool,
    pub kind: PrefabType,
    pub marker: bool,
    pub marker_title: String,
    pub marker_texture: Option<String>,
    pub transform: Transform,
    /// Its prefab's meshes in the level's cells.
    pub keys: Vec<PropKey>,
    /// A BP_SettlementCampSpawner (spawned only when BP_SettlementManager picks it).
    pub settlement: bool,
}

impl PrefabSite {
    pub fn from_actor(actor: usize, a: &ActorInfo) -> Option<Self> {
        // BP_SettlementCampSpawner.SpawnSettlement: its Settlement class at its own transform.
        if a.class == "BP_SettlementCampSpawner_C" {
            let prefab = a.props.as_ref()?.get("Settlement")?.clone();
            return Some(Self {
                actor,
                prefab,
                dynamic: false,
                chance: 0.0,
                random: false,
                kind: PrefabType::Other,
                marker: false,
                marker_title: String::new(),
                marker_texture: None,
                transform: affine(&a.transform),
                keys: Vec::new(),
                settlement: true,
            });
        }
        if !a.class.starts_with("BP_PrefabSpawner") {
            return None;
        }
        let props = a.props.clone().unwrap_or_default();
        let get = |k: &str| props.get(k).map(String::as_str);
        let prefab = get("Prefab")?.to_string();
        let defaults = &data().spawner;
        let dynamic = a.class == "BP_PrefabSpawner_Dynamic_C";
        let title = get("Marker Title")
            .and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok())
            .and_then(|v| v.get("SourceString").and_then(|s| s.as_str()).map(str::to_string))
            .unwrap_or_else(|| defaults.marker_title.clone());
        Some(Self {
            actor,
            prefab,
            dynamic,
            chance: get("Chance").and_then(|c| c.parse().ok()).unwrap_or(defaults.chance),
            // BP_PrefabSpawner_Dynamic's class default turns the random check off.
            random: get("RandomPossibilityToSpawn?").map_or(!dynamic && defaults.random, |v| v == "true"),
            kind: PrefabType::parse(get("PrefabType")),
            marker: get("Marker?") == Some("true"),
            marker_title: title,
            marker_texture: get("Marker Texture").and_then(|n| defaults.textures.get(n).cloned()).or_else(|| defaults.marker_texture.clone()),
            transform: affine(&a.transform),
            keys: Vec::new(),
            settlement: false,
        })
    }

    pub fn def(&self) -> Option<&'static PrefabDef> {
        data().prefabs.get(&self.prefab)
    }

    /// Spawnable at all here: every human its volumes hold is ported (the scavengers' dogs aside;
    /// the settlements' survivor spawners and traders aren't).
    pub fn portable(&self) -> bool {
        self.def().is_some_and(|d| d.humans.iter().all(|c| crate::gameplay::zombies::ZombieKind::from_class(c).is_some() || c.contains("Dog")))
    }
}

/// Random Check: a roll of 0-100 at most Chance x the type's multiplier (always, without
/// RandomPossibilityToSpawn? the check fails: the spawner waits for an event).
pub fn random_check(site: &PrefabSite, multiplier: f32, roll: f32) -> bool {
    site.random && roll <= site.chance * multiplier
}

/// IsOutsideOfPlayerView?: the sight line from the camera is blocked, or the spot is more than
/// 60° off the camera's forward (acos(dot(normal(camera - spot), forward)) <= 120°).
pub fn outside_view(camera: Vec3, forward: Vec3, spot: Vec3, blocked: bool) -> bool {
    let back = (camera - spot).normalize_or_zero();
    blocked || back.dot(forward).clamp(-1.0, 1.0).acos().to_degrees() <= 120.0
}

/// Per-session prefab state.
#[derive(Resource, Default)]
pub struct Prefabs {
    /// Sites (index in `Level::prefabs`) spawned this session.
    pub spawned: Vec<usize>,
    rolled: bool,
    /// Seconds to the next dynamic event, the shuffled dynamic sites and the next to try.
    timer: f32,
    order: Vec<usize>,
    next: usize,
    /// Dynamic sites waiting out DelayStart.
    pending: Vec<(usize, f32)>,
    /// Map markers still up: (marker key, where).
    markers: Vec<(usize, Vec3)>,
}

impl Prefabs {
    /// Spawn site `i` next frame, whatever its roll (the screenshot tour).
    pub fn force(&mut self, i: usize) {
        if !self.spawned.contains(&i) && !self.pending.iter().any(|p| p.0 == i) {
            self.pending.push((i, 0.0));
        }
    }
}

/// World markers' keys for prefab sites.
const MARKER_KEYS: usize = 1 << 28;

pub(super) fn reset(mut prefabs: ResMut<Prefabs>) {
    *prefabs = Prefabs::default();
}

fn difficulty(store: &SettingsStore, (page, label): (&str, &str)) -> f32 {
    crate::gameplay::survival::difficulty(store, page, label)
}

/// What a spawn needs to touch.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct SpawnCtx<'w, 's> {
    commands: Commands<'w, 's>,
    level: Res<'w, Level>,
    streaming: ResMut<'w, PropStreaming>,
    cache: ResMut<'w, super::collision::CollisionCache>,
    containers: ResMut<'w, crate::gameplay::interact::Containers>,
    spawners: ResMut<'w, super::spawners::Spawners>,
    markers: ResMut<'w, crate::gameplay::map::WorldMarkers>,
    store: Res<'w, SettingsStore>,
    db: Res<'w, crate::gameplay::ItemDb>,
    items: Query<'w, 's, (Entity, &'static super::PropItem)>,
    meshes: Res<'w, Assets<Mesh>>,
    sound: MessageWriter<'w, crate::audio::Sound>,
    zones: ResMut<'w, crate::gameplay::zones::SafeZones>,
    wells: ResMut<'w, crate::gameplay::zones::Wells>,
    traders: ResMut<'w, crate::gameplay::traders::Traders>,
    tdb: Res<'w, crate::gameplay::traders::TraderDb>,
    settlers: ResMut<'w, crate::gameplay::civilians::Settlers>,
    turrets: ResMut<'w, crate::gameplay::turrets::Turrets>,
}

/// Put site `i`'s prefab in the world.
fn spawn_site(ctx: &mut SpawnCtx, prefabs: &mut Prefabs, i: usize, sound: bool) {
    let Some(site) = ctx.level.prefabs.get(i).cloned() else { return };
    let Some(def) = site.def() else { return };
    prefabs.spawned.push(i);
    let base = site.transform.to_matrix();
    let place = |m: &[f32; 12]| Transform::from_matrix(base * affine(m).to_matrix());
    // The meshes come back: their cells reload, their collision is rebuilt.
    let reload = ctx.streaming.restore(&site.keys);
    for &k in &site.keys {
        ctx.cache.forget(k);
    }
    for e in reload {
        ctx.commands.entity(e).despawn();
    }
    // RemoveFoliageComp: the trees and rocks within FoliageRemoverRadius go.
    let at = site.transform.translation;
    for kind in [super::Harvestable::Tree, super::Harvestable::Stone] {
        for (key, _) in ctx.level.harvestables_near(at, def.foliage_radius, kind) {
            ctx.streaming.remove(key);
            ctx.cache.forget(key);
            for (e, p) in &ctx.items {
                if p.0 == key {
                    ctx.commands.entity(e).despawn();
                }
            }
        }
    }
    let (mut boxes, mut zombies, mut humans) = (0, 0, 0);
    let mut rng = rand::thread_rng();
    for (j, a) in def.actors.iter().enumerate() {
        let t = place(&a.transform);
        settlement_actor(ctx, a, &t, format!("{}/{j}", site.prefab));
        if ctx.db.containers.contains_key(&a.class) {
            let unique = a.props.get("UniqueLoot").and_then(|u| serde_json::from_str::<Vec<String>>(u).ok()).unwrap_or_default();
            ctx.containers.add_at(CONTAINER_KEYS + i * 64 + j, t.translation, &a.class, unique);
            boxes += 1;
        }
        let info = ActorInfo { class: a.class.clone(), name: format!("{}/{j}", site.prefab), transform: matrix12(&t), props: Some(a.props.clone()) };
        if let Some(d) = super::spawners::SpawnerDef::from_actor(&info) {
            ctx.spawners.add(SPAWNER_KEYS + i * 64 + j, d);
            zombies += 1;
        }
        // A random-chance volume rolls its Chance first, then works like the level's.
        if let Some((chance, defs)) = super::spawners::SpawnerDef::random_chance(&info) {
            if rng.gen_range(0.0..100.0) <= chance {
                for (k, d) in defs.into_iter().enumerate().take(8) {
                    humans += d.amount;
                    ctx.spawners.add(SPAWNER_KEYS + i * 64 * 8 + j * 8 + k + (1 << 20), d);
                }
            }
            continue;
        }
        // BP_AISpawningVolume_Prefab: each Spawning AI element is a group of its own.
        for (k, (kind, count)) in volume_elements(a).into_iter().enumerate() {
            ctx.spawners.add(SPAWNER_KEYS + i * 64 * 8 + j * 8 + k + (1 << 20), super::spawners::SpawnerDef::human_volume(kind, &t, count));
            humans += count;
        }
    }
    for (system, m) in &def.particles {
        let t = place(m);
        ctx.commands.spawn((
            t,
            Visibility::default(),
            crate::gameplay::particles::Emitter::new(system),
            crate::gameplay::particles::LevelFx { owner: site.prefab.clone(), actor: String::new(), range: 100.0, out: false },
            DespawnOnExit(AppState::InGame),
        ));
    }
    if sound && let Some(cue) = &data().spawner.sound {
        ctx.sound.write(crate::audio::Sound::at(cue.clone(), at));
    }
    // SpawnMarker: with Marker? and RANDOM EVENTS HAVE A MARKER?.
    if site.marker && crate::gameplay::combat::store_bool(&ctx.store, "General", "RANDOM EVENTS HAVE A MARKER?") {
        let key = MARKER_KEYS + i;
        ctx.markers.0.push(crate::gameplay::map::WorldMarker {
            key,
            pos: at,
            texture: site.marker_texture.clone().unwrap_or_default(),
            size: 3.5,
            colour: Color::linear_rgb(0.505208, 0.169744, 0.034207),
            title: site.marker_title.clone(),
            description: data().spawner.marker_description.clone(),
        });
        prefabs.markers.push((key, at));
    }
    info!("prefab {} spawned at {at:.0}: {} meshes, {boxes} containers, {zombies} zombie spawners, {humans} humans", site.prefab, site.keys.len());
}

/// A settlement's child actors that aren't containers or zombie spawners: its traders
/// (BP_SafeZoneTrader, BP_BlackMarketTrader), its quest giver, its safe zone, its water well, its
/// settlers' spawners, its turret and its BP_Marker (SDLogoTransparent,
/// size 3, its MarkerTitle: "Scavenger Safe Zone").
fn settlement_actor(ctx: &mut SpawnCtx, a: &PrefabActor, t: &Transform, name: String) {
    let info = ActorInfo { class: a.class.clone(), name: name.clone(), transform: matrix12(t), props: Some(a.props.clone()) };
    if ctx.tdb.classes.contains_key(&a.class) {
        let limited = crate::gameplay::traders::limited(&ctx.store);
        let SpawnCtx { traders, tdb, db, containers, .. } = ctx;
        traders.place(tdb, db, limited, containers, &a.class, Transform::from_translation(t.translation).with_rotation(t.rotation));
    } else if let Some(z) = crate::gameplay::zones::safe_zone(&info) {
        ctx.zones.add(z);
    } else if a.class == "BP_WaterWell_C" {
        ctx.wells.add(&info);
    } else if a.class == "BP_AISpawner_SettlementSurvivors_C" {
        ctx.settlers.add(&info);
    } else if crate::gameplay::turrets::TurretKind::of(&a.class).is_some() {
        let SpawnCtx { turrets, level, streaming, commands, .. } = ctx;
        turrets.add(level, streaming, commands, &a.class, t);
    } else if a.class == "BP_Marker_C" {
        let title = a.props.get("MarkerTitle").and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok()).and_then(|v| v.get("SourceString")?.as_str().map(str::to_string)).unwrap_or_default();
        let key = MARKER_KEYS + (1 << 20) + ctx.markers.0.len();
        ctx.markers.0.push(crate::gameplay::map::WorldMarker {
            key,
            pos: t.translation,
            texture: "game/SurrounDead/Content/UI/Textures/SDLogoTransparent.png".into(),
            size: 3.0,
            colour: Color::WHITE,
            title,
            description: String::new(),
        });
    }
}

/// BP_SettlementManager's Camps (BP_SettlementCampSpawner actors, in order) as prefab sites.
pub fn settlement_camps(level: &Level) -> Vec<usize> {
    let Some(manager) = level.actors.iter().find(|a| a.class == "BP_SettlementManager_C") else { return Vec::new() };
    let camps = manager.props.as_ref().and_then(|p| p.get("Camps")).cloned().unwrap_or_default();
    let names: Vec<&str> = camps.split("PersistentLevel.").skip(1).filter_map(|s| s.split('\'').next()).collect();
    names.iter().filter_map(|n| level.prefabs.iter().position(|s| s.settlement && level.actors.get(s.actor).is_some_and(|a| a.name == *n))).collect()
}

/// A spawning volume's `Spawning AI`: (AICharacter, SpawnCount) for the classes that are ported.
pub fn volume_elements(a: &PrefabActor) -> Vec<(crate::gameplay::zombies::ZombieKind, u32)> {
    if !a.class.starts_with("BP_AISpawningVolume") {
        return Vec::new();
    }
    a.props.get("Spawning AI").map_or_else(Vec::new, |t| spawning_ai(t))
}

/// A `Spawning AI` array (S_AISpawn: AICharacter class, SpawnCount) as exported, for the ported classes.
pub fn spawning_ai(json: &str) -> Vec<(crate::gameplay::zombies::ZombieKind, u32)> {
    let Ok(list) = serde_json::from_str::<Vec<serde_json::Map<String, serde_json::Value>>>(json) else { return Vec::new() };
    list.iter()
        .filter_map(|el| {
            let class = el.iter().find(|(k, _)| k.starts_with("AICharacter"))?.1.get("ObjectName")?.as_str()?.split('\'').nth(1)?.to_string();
            let count = el.iter().find(|(k, _)| k.starts_with("SpawnCount"))?.1.as_u64()? as u32;
            Some((crate::gameplay::zombies::ZombieKind::from_class(&class)?, count))
        })
        .collect()
}

/// A Transform as the level export's 12 floats.
fn matrix12(t: &Transform) -> [f32; 12] {
    let m = t.to_matrix();
    let c = m.to_cols_array();
    [c[0], c[1], c[2], c[4], c[5], c[6], c[8], c[9], c[10], c[12], c[13], c[14]]
}

/// The level's spawners roll once the survivor is in; dynamic events come later.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_prefabs(
    time: Res<Time>,
    mut game: ResMut<crate::menus::CurrentGame>,
    mut prefabs: ResMut<Prefabs>,
    mut ctx: SpawnCtx,
    player: Query<&Transform, (With<crate::models::CharacterModel>, With<super::KinematicBody>)>,
    camera: Query<&GlobalTransform, With<crate::gameplay::PlayerCamera>>,
) {
    let Ok(me) = player.single() else { return };
    let me = me.translation;
    let mut rng = rand::thread_rng();
    if !prefabs.rolled {
        prefabs.rolled = true;
        let sites: Vec<usize> = (0..ctx.level.prefabs.len()).collect();
        // BP_SettlementManager: one of its Camps at random, kept in the save (SettlementSpawned?,
        // CampToSpawn) so the same settlement comes back.
        let camps = settlement_camps(&ctx.level);
        if !camps.is_empty()
            && let Some(state) = game.state.as_mut()
        {
            let pick = *state.settlement.get_or_insert_with(|| rng.gen_range(0..camps.len()));
            if let Some(&i) = camps.get(pick) {
                spawn_site(&mut ctx, &mut prefabs, i, false);
            }
        }
        for &i in &sites {
            let site = &ctx.level.prefabs[i];
            if site.dynamic || site.settlement || !site.portable() {
                continue;
            }
            let mult = site.kind.multiplier_row().map_or(1.0, |r| difficulty(&ctx.store, r));
            if random_check(site, mult, rng.gen_range(0.0..100.0)) && site.transform.translation.distance(me) > PLAYER_CLEARANCE {
                spawn_site(&mut ctx, &mut prefabs, i, false);
            }
        }
        let mut order: Vec<usize> = sites.into_iter().filter(|&i| ctx.level.prefabs[i].dynamic && ctx.level.prefabs[i].portable()).collect();
        order.shuffle(&mut rng);
        prefabs.order = order;
        prefabs.timer = rng.gen_range(DYNAMIC_PERIOD[0]..DYNAMIC_PERIOD[1]);
        info!("prefabs: {} of {} sites spawned; {} dynamic events in waiting", prefabs.spawned.len(), ctx.level.prefabs.len(), prefabs.order.len());
    }
    let dt = time.delta_secs();
    // Markers come off within DistanceToRemoveMarker.
    let within = data().spawner.remove_marker_within;
    let gone: Vec<usize> = prefabs.markers.iter().filter(|(_, at)| at.distance(me) < within).map(|m| m.0).collect();
    if !gone.is_empty() {
        ctx.markers.0.retain(|m| !gone.contains(&m.key));
        prefabs.markers.retain(|m| !gone.contains(&m.0));
    }
    // DynamicRandomEventSpawn.
    prefabs.timer -= dt;
    if prefabs.timer <= 0.0 {
        prefabs.timer = rng.gen_range(DYNAMIC_PERIOD[0]..DYNAMIC_PERIOD[1]);
        let days = game.state.as_ref().map_or(0, |s| s.stats.days_survived);
        if days > DYNAMIC_AFTER_DAYS
            && let Ok(cam) = camera.single()
        {
            let (eye, fwd) = (cam.translation(), cam.forward().as_vec3());
            while prefabs.next < prefabs.order.len() {
                let i = prefabs.order[prefabs.next];
                prefabs.next += 1;
                let spot = ctx.level.prefabs[i].transform.translation + Vec3::Y * 0.5;
                let SpawnCtx { level, streaming, cache, meshes, .. } = &mut ctx;
                let blocked = cache.cast_item(level, streaming, meshes, eye, (spot - eye).normalize_or_zero(), eye.distance(spot)).is_some();
                if outside_view(eye, fwd, spot, blocked) {
                    prefabs.pending.push((i, data().spawner.delay_start));
                    info!("prefabs: dynamic event {} coming", ctx.level.prefabs[i].prefab);
                    break;
                }
            }
        }
    }
    let mut due = Vec::new();
    prefabs.pending.retain_mut(|(i, left)| {
        *left -= dt;
        if *left <= 0.0 {
            due.push(*i);
        }
        *left > 0.0
    });
    for i in due {
        // CheckSurroundingArea again at spawn time.
        if ctx.level.prefabs[i].transform.translation.distance(me) > PLAYER_CLEARANCE {
            spawn_site(&mut ctx, &mut prefabs, i, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(class: &str, props: &[(&str, &str)]) -> ActorInfo {
        ActorInfo {
            class: class.into(),
            name: "x".into(),
            transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 20.0],
            props: Some(props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()),
        }
    }

    #[test]
    fn spawner_instances_override_the_class_defaults() {
        let a = actor("BP_PrefabSpawner_C", &[("Prefab", "Prefab_AbandonedCamp_C"), ("Chance", "30.0"), ("PrefabType", "\"Enum_PrefabType::NewEnumerator4\""), ("Marker?", "true")]);
        let s = PrefabSite::from_actor(0, &a).unwrap();
        assert_eq!((s.chance, s.kind, s.marker, s.dynamic), (30.0, PrefabType::Other, true, false));
        let d = PrefabSite::from_actor(1, &actor("BP_PrefabSpawner_Dynamic_C", &[("Prefab", "Prefab_CrashedHelicopter01_C")])).unwrap();
        assert!(d.dynamic && !d.random && d.kind == PrefabType::HelicopterCrash, "the dynamic class has no random check");
        assert!(PrefabSite::from_actor(2, &actor("BP_AISpawner_Zombies_C", &[])).is_none());
    }

    #[test]
    fn settlement_camps_are_sites_that_never_roll() {
        let s = PrefabSite::from_actor(3, &actor("BP_SettlementCampSpawner_C", &[("Settlement", "Prefab_Settlement2_C")])).unwrap();
        assert!(s.settlement && !s.random && !s.dynamic && !s.marker);
        assert_eq!(s.prefab, "Prefab_Settlement2_C");
        assert!(!random_check(&s, 100.0, 0.0), "only BP_SettlementManager spawns one");
        assert!(PrefabSite::from_actor(3, &actor("BP_SettlementCampSpawner_C", &[])).is_none());
        let p = PrefabSite::from_actor(0, &actor("BP_PrefabSpawner_C", &[("Prefab", "P")])).unwrap();
        assert!(!p.settlement);
    }

    #[test]
    fn random_check_scales_the_chance() {
        let s = PrefabSite::from_actor(0, &actor("BP_PrefabSpawner_C", &[("Prefab", "P"), ("Chance", "30.0")])).unwrap();
        assert!(random_check(&s, 1.0, 29.9) && !random_check(&s, 1.0, 30.1));
        assert!(random_check(&s, 2.0, 59.0), "x2 multiplier");
        assert!(!random_check(&s, 0.0, 0.1), "multiplier 0: never");
        assert_eq!(PrefabType::BanditCamp.multiplier_row(), Some(("AI", "BANDIT CAMP SPAWN CHANCE MULTIPLIER")));
        assert_eq!(PrefabType::Quest.multiplier_row(), None);
    }

    #[test]
    fn out_of_view_is_blocked_or_off_to_the_side() {
        let (eye, fwd) = (Vec3::ZERO, Vec3::NEG_Z);
        assert!(!outside_view(eye, fwd, Vec3::new(0.0, 0.0, -100.0), false), "straight ahead, in sight");
        assert!(outside_view(eye, fwd, Vec3::new(0.0, 0.0, -100.0), true), "ahead but blocked");
        assert!(outside_view(eye, fwd, Vec3::new(100.0, 0.0, 0.0), false), "90° off");
        assert!(!outside_view(eye, fwd, Vec3::new(50.0, 0.0, -100.0), false), "27° off: in view");
        assert!(outside_view(eye, fwd, Vec3::new(0.0, 0.0, 100.0), false), "behind");
    }

    #[test]
    fn prefab_data_resolves() {
        let d = data();
        if d.prefabs.is_empty() {
            return;
        }
        let camp = &d.prefabs["Prefab_AbandonedCamp_C"];
        assert!(camp.humans.is_empty() && camp.meshes.len() >= 12 && camp.foliage_radius > 0.0);
        assert_eq!(camp.actors.iter().filter(|a| a.class.starts_with("Container_")).count(), 5);
        assert!(camp.actors.iter().any(|a| a.class == "BP_AISpawner_Zombies_C" && a.props.contains_key("SpawnAmount")));
        assert!(!d.prefabs["Prefab_SmallBanditCamp_C"].humans.is_empty(), "bandits wait for human AI");
        assert_eq!(d.spawner.chance, 50.0);
        assert_eq!(d.spawner.remove_marker_within, 20.0);
    }
}
