//! Infestations: BP_Infestation and its children (Floor / Wall / Ceiling / Corner, the "Secondary
//! Infestation"s, and BP_PrimaryInfestation), BP_InfestationSpline, BP_InfestationManager and
//! BP_RandomInfestationSpawnPoint, decompiled. Class data: `assets/data/infestations.json`
//! (`extractor/gen_infestations.py`); each nest's links and spawn points: actors.json `props`.
//!
//! - **The level's**: 12 primaries, each with 1-2 secondaries (SecondaryInfestations) and as many
//!   vine splines (Splines); 31 nests. BeginPlay: with PRIMARY INFESTATIONS ENABLED? off
//!   (Difficulty.PrimaryInfestations, on by default) a primary destroys its secondaries, its
//!   splines and itself. Health = SetHealth(StandardHealthValue x INFESTATIONS HEALTH MULTIPLIER):
//!   a roll within ±HealthRandomDeviation of it (1000 a secondary, 2500 a primary, 1750 ± 250 the
//!   random ones); MaxHealth stays the template's.
//! - **Damage** (Damage_Object: rounds, swings, blasts): a primary whose CanDamagePrimary? is off
//!   says "Secondary Infestations Not Destroyed In Surrounding Area." (dark red, 2 s) and takes
//!   nothing. Else DamageComponent.DamageTaken: health down, bullet_impact_body_thump_08_Cue; at 0
//!   or more it's StandardDamage, where once health < MaxHealth / 1.5 the waves start (DoOnce). Below 0
//!   it's dead: bone_break_neck_snap_crack_01_Cue and InfestationDestroyed(causer, false).
//! - **Waves** (SpawnWave): WavesOfEnemies[CurrentWave]'s AI x Amount, one at a time 1-1.5 s
//!   apart, each at LocToSpawn (a random AISpawningTransforms point, then a random point within
//!   ±2 m, 10 cm + SpawnZOffset up) and sent to where the survivor is (SetBB_LockOnTarget); then
//!   CurrentWave + 1 and 30 s to the next. Secondaries: 3 waves of 6 BP_Zombie_Infected_Horde;
//!   primaries: 6, 8, 10, then the Infestation horde boss with 8. Nothing stops them: they run
//!   their course even after the nest is gone.
//! - **Destroyed** (Event_OnDestroyed): IsDestroyed?; FX_Explosion_Infestation at ExplosionLoc;
//!   the controller's InfestationsDestroyed + 1; level x 400-500 (primary) / 100-200 XP x
//!   MultiplyValue 1.05 x XP MULTIPLIER (AddXP), "Infestation Destroyed, XP Given: {XP}" (red,
//!   with Game.XpPopUp); 0.2 s later the DestroyedMesh, the RedLight / Flies / dust off, the map
//!   marker gone. Now JigCanInteract: F opens its loot ("Infestation Pile": the class's
//!   LootTable, Infestation / PrimaryInfestation, plus UniqueLootSets: a primary's Infestation,
//!   MilitarySet, BanditSet).
//! - **A secondary's end** reaches its primary (Event_SecondaryDestroyed): once all are destroyed,
//!   CanDamagePrimary?, FX_Explosion_Infestation at the primary's ExplosionLoc, 0.2 s, its Armor
//!   (InfestationLarge_Armor) and its splines destroyed.
//! - **Markers**: ShowMarker? nests (primaries, random ones) put Marker_Infestation on the map,
//!   size 4 / 2, colour (0.245, 0, 0), titled by the nest's name, described "The primary
//!   infestation in the surrounding area. Consisting of multiple dead bodies of unknown origin."
//!   / "A pile of dead bodies that attracts and changes zombies nearby.".
//! - **Loading** (ActorLoaded, IsDestroyed? or health <= 0): the destroyed state again without the
//!   explosion or XP; a destroyed primary also destroys its secondaries outright (piles and all),
//!   its armor and splines.
//! - **BP_InfestationManager**: 4 s after loading, if every primary is destroyed (or gone), each
//!   BP_RandomInfestationSpawnPoint rolls (RANDOM INFESTATIONS ENABLED?): rand(0, 100) <=
//!   RandomChance (20; some 15) x RANDOM INFESTATIONS SPAWN CHANCE MULTIPLIER spawns a
//!   BP_Infestation ("Infestation") with the point's meshes and AISpawningTransforms, the loot
//!   sets Infestation, MilitarySet and BanditSet; else the point is gone.
//! - **NoSpawn** (TraceForActor): a random point whose 5 m sphere touches an actor tagged
//!   NoSpawn (the player's buildables; no placed level actor has it) doesn't spawn.
//! - The PostProcess sphere is `post_volumes`'; AIHealthBarInfo (name, current / standard
//!   health, the skull for a primary, none once destroyed) is `ai_health_bar`'s.
//!
//! Ours: the nests' meshes are drawn by this module (the level's props are taken out) and solid
//! through `PropRays`' dynamic sets, the destroyed pile too (the BP only stops it blocking channel
//! 23); a hit is a round's line through the mesh's bounds, a swing within 2 m of them, a blast
//! reaching them; the lights' exponent-falloff Intensity 1 has no candela equivalent and uses the
//! lab lamps' 3.2 cd (a guess); the flies are FX_Flies on within 50 m (its LDMaxDrawDistance); the
//! random spawn points roll once a save (`InfestationSave::random`) and their nests are saved;
//! wave AIs are put on the floor under their spawn point; NoSpawn is a 5 m sphere against the
//! buildables' collision triangles.
//!
//! Not ported: NS_InfestationDust (a GPU Niagara system: its spawn rate, sizes, colours and
//! lifetimes are compiled into the cooked shader, not in the asset's data), the outline, the grab
//! montage, Damage_Shoved / HitByVehicle (empty).

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::OnceLock;

use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

use super::interact::{Containers, LOOT_COLUMNS, insert_growing};
use super::items::{Grid, ItemDb};
use super::map::{WorldMarker, WorldMarkers};
use super::notify::Notify;
use crate::level::{Level, PropKey};
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::{AppState, CurrentGame};
use crate::models::CharacterModel;

const DATA: &str = "assets/data/infestations.json";
const SPLINE_MESH: &str = "Infestation_Spline";
const MARKER_ICON: &str = "game/SurrounDead/Content/POI/Marker_Infestation.png";
/// The Blueprint's marker descriptions (by PrimaryInfestation?).
const PRIMARY_TEXT: &str = "The primary infestation in the surrounding area. Consisting of multiple dead bodies of unknown origin.";
const OTHER_TEXT: &str = "A pile of dead bodies that attracts and changes zombies nearby.";
const REFUSED: &str = "Secondary Infestations Not Destroyed In Surrounding Area.";
/// AddJigWidgetToContent's title.
pub const PILE: &str = "Infestation Pile";
/// The DamageComponent's DamageSound / DeathSound.
const DAMAGE_SOUND: &str = "bullet_impact_body_thump_08_Cue";
const DEATH_SOUND: &str = "bone_break_neck_snap_crack_01_Cue";
const EXPLOSION: &str = "FX_Explosion_Infestation";
/// Event_OnDestroyed's and Event_SecondaryDestroyed's Delay before the mesh swap / the armor.
const SWAP_DELAY: f32 = 0.2;
/// SpawnWave: between AIs, then between waves.
const BETWEEN_AI: (f32, f32) = (1.0, 1.5);
const BETWEEN_WAVES: f32 = 30.0;
/// LocToSpawn's box (±200, ±200 cm) and lift (10 cm).
const SPAWN_BOX: f32 = 2.0;
const SPAWN_LIFT: f32 = 0.1;
/// BP_InfestationManager's Delay after the save has loaded.
const MANAGER_DELAY: f32 = 4.0;
/// Event_OnDestroyed's XP rolls.
const PRIMARY_XP: (f32, f32) = (400.0, 500.0);
const OTHER_XP: (f32, f32) = (100.0, 200.0);
/// The marker's Texture Size (primary / others) and colour.
const MARKER_SIZE: (f32, f32) = (4.0, 2.0);
/// Ours: the lights' candelas (Intensity 1 with exponent falloff; the lab lamps' 2000 / 625).
const LIGHT_CANDELA: f32 = 3.2;
/// Ours: lights drawn within this of the survivor (`level::lights`' range).
const LIGHT_RANGE: f32 = 70.0;
/// The Flies component's LDMaxDrawDistance (5000 cm).
const FLIES_RANGE: f32 = 50.0;
/// Ours: a swing's reach from the chest to the mesh's bounds.
const SWING_REACH: f32 = 2.0;
const CHEST: f32 = 1.2;
/// Collision keys for the nests' meshes (`PropRays::set_dynamic`) and map marker keys.
const COLLIDE_KEY: usize = 1 << 45;
const MARKER_KEY: usize = 1 << 45;

#[derive(Deserialize, Clone, Debug)]
pub struct LightDef {
    pub at: [f32; 3],
    pub color: [f32; 3],
    pub radius: f32,
}

/// One infestation class (BP_Infestation's chain).
#[derive(Deserialize, Clone, Debug)]
pub struct ClassDef {
    pub name: String,
    pub primary: bool,
    pub show_marker: bool,
    pub health: f32,
    pub max_health: f32,
    pub deviation: f32,
    pub mesh: String,
    pub destroyed_mesh: String,
    pub armor: Option<String>,
    pub loot_table: Option<String>,
    #[serde(default)]
    pub unique: Vec<String>,
    pub spawn_z_offset: f32,
    /// Each wave's (AI class, amount).
    pub waves: Vec<Vec<(String, u32)>>,
    pub explosion: [f32; 3],
    pub flies: [f32; 3],
    pub lights: Vec<LightDef>,
    /// PostProcessRadius (cm): the Sphere a primary's PostProcess fills (half for a random one).
    #[serde(default)]
    pub post_process_radius: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RandomPointDef {
    pub chance: f32,
    pub mesh: String,
    pub destroyed_mesh: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct InfestationDb {
    pub classes: HashMap<String, ClassDef>,
    pub random_point: RandomPointDef,
}

pub fn db() -> &'static InfestationDb {
    static DB: OnceLock<InfestationDb> = OnceLock::new();
    DB.get_or_init(|| {
        std::fs::read_to_string(DATA).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| {
            warn!("{DATA} missing or unreadable; run extractor/export_assets.sh --infestations");
            InfestationDb { classes: HashMap::new(), random_point: RandomPointDef { chance: 0.0, mesh: String::new(), destroyed_mesh: String::new() } }
        })
    })
}

/// The class a spawned random infestation has.
pub const RANDOM_CLASS: &str = "BP_Infestation_C";

/// The pile's container (JigCanInteract once destroyed): a press opens it, named as the nest.
pub fn container(class: &str) -> Option<super::items::ContainerDef> {
    let c = db().classes.get(class)?;
    Some(super::items::ContainerDef { name: c.name.clone(), table: c.loot_table.clone(), hold: 0.0, lockpick: false, noise: 0.0, mesh: None, airdrop: None, tool: None, refills: false, magnifier: [0.0; 3] })
}

/// What's saved (by the nest's actor name; a random one goes by its spawn point's).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InfestationSave {
    pub destroyed: BTreeSet<String>,
    /// DamageComponent's CurrentHealth (SaveValues?) of nests that took damage.
    pub health: BTreeMap<String, f32>,
    /// The destroyed nests' piles.
    pub piles: BTreeMap<String, Grid>,
    /// `None` until BP_InfestationManager has rolled the random spawn points; then the points that
    /// grew an infestation.
    pub random: Option<Vec<String>>,
}

/// SpawnWave's progress.
#[derive(Clone, Debug, PartialEq)]
pub struct Waves {
    pub wave: usize,
    queue: VecDeque<String>,
    timer: f32,
    pub done: bool,
}

impl Waves {
    pub fn start(waves: &[Vec<(String, u32)>]) -> Self {
        Self { wave: 0, queue: expand(waves.first()), timer: 0.0, done: waves.is_empty() }
    }

    /// The AIs to spawn now: one, then 1-1.5 s; a wave's end, then CurrentWave + 1 and 30 s.
    pub fn tick(&mut self, dt: f32, waves: &[Vec<(String, u32)>], rng: &mut impl Rng) -> Vec<String> {
        let mut out = Vec::new();
        if self.done {
            return out;
        }
        self.timer -= dt;
        while self.timer <= 0.0 && !self.done {
            if let Some(c) = self.queue.pop_front() {
                out.push(c);
                self.timer += rng.gen_range(BETWEEN_AI.0..=BETWEEN_AI.1);
            } else {
                self.wave += 1;
                if self.wave >= waves.len() {
                    // Past the last wave SpawnWave finds nothing (and keeps going, idle).
                    self.done = true;
                } else {
                    self.queue = expand(waves.get(self.wave));
                    self.timer += BETWEEN_WAVES;
                }
            }
        }
        out
    }
}

fn expand(wave: Option<&Vec<(String, u32)>>) -> VecDeque<String> {
    wave.into_iter().flatten().flat_map(|(c, n)| std::iter::repeat_n(c.clone(), *n as usize)).collect()
}

/// A hit's outcome (Damage_Object -> DamageTaken).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    /// Destroyed already (DamageComponent not alive).
    Ignored,
    /// A primary with secondaries standing.
    Refused,
    /// StandardDamage; `waves` when this one starts them.
    Damaged { waves: bool },
    Destroyed,
}

/// What follows a nest's destruction (Event_OnDestroyed / Event_SecondaryDestroyed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// FX_Explosion_Infestation at the nest's ExplosionLoc.
    Explode(usize),
    /// Stats, XP and the toast.
    Reward(usize),
    /// 0.2 s on: the DestroyedMesh, lights and flies off, the marker gone, the pile open.
    Swap(usize),
    /// 0.2 s on: a primary's armor and splines gone.
    Armor(usize),
    /// K2_DestroyActor: the nest gone altogether.
    Remove(usize),
}

#[derive(Clone, Debug)]
pub struct Nest {
    /// The level actor (a random one: its spawn point): the pile's container key.
    pub actor: usize,
    pub name: String,
    pub class: String,
    pub transform: Transform,
    pub mesh: String,
    pub destroyed_mesh: String,
    pub health: f32,
    pub max_health: f32,
    /// DamageComponent's StandardHealthValue: the roll SetHealth made at load (the AI health
    /// bar's maximum).
    pub standard: f32,
    pub primary: bool,
    pub random: bool,
    /// Indices in `Infestations::nests`.
    pub secondaries: Vec<usize>,
    pub primary_of: Option<usize>,
    /// BP_InfestationSpline actor names, then their segments.
    pub spline_actors: Vec<String>,
    /// AISpawningTransforms' locations (world).
    pub spawns: Vec<Vec3>,
    pub spawn_z_offset: f32,
    pub destroyed: bool,
    /// Removed altogether (a loaded destroyed primary's secondaries).
    pub gone: bool,
    /// CanDamagePrimary? (secondaries and random ones: always).
    pub can_damage: bool,
    /// The armor and splines are gone.
    pub bare: bool,
    pub waves: Option<Waves>,
    /// World bounds of the drawn mesh, once loaded.
    pub bounds: Option<(Vec3, Vec3)>,
    /// The PostProcess sphere's world radius (primaries and random ones; secondaries destroy it).
    pub haze: Option<f32>,
}

impl Nest {
    pub fn def(&self) -> Option<&'static ClassDef> {
        db().classes.get(&self.class)
    }

    /// Damage_Object then DamageTaken.
    pub fn hit(&mut self, damage: f32) -> Hit {
        if self.destroyed || self.gone {
            return Hit::Ignored;
        }
        if self.primary && !self.can_damage {
            return Hit::Refused;
        }
        self.health -= damage;
        if self.health >= 0.0 {
            // StandardDamage: CurrentHealth < MaxHealth / 1.5, DoOnce.
            let start = self.waves.is_none() && self.health < self.max_health / 1.5;
            return Hit::Damaged { waves: start };
        }
        Hit::Destroyed
    }

    /// A point in the actor's frame (UE cm) in the world.
    pub fn at_ue(&self, cm: [f32; 3]) -> Vec3 {
        self.transform.transform_point(Vec3::new(cm[0], cm[2], cm[1]) / 100.0)
    }

    pub fn explosion_at(&self) -> Vec3 {
        self.def().map_or(self.transform.translation, |d| self.at_ue(d.explosion))
    }

    /// Where the pile is reached from: its bounds' middle, else ExplosionLoc.
    pub fn middle(&self) -> Vec3 {
        self.bounds.map_or_else(|| self.explosion_at(), |(lo, hi)| (lo + hi) * 0.5)
    }

    /// LocToSpawn: a random spawning point, ±2 m about it, 10 cm + SpawnZOffset up.
    pub fn spawn_point(&self, rng: &mut impl Rng) -> Option<Vec3> {
        if self.spawns.is_empty() {
            return None;
        }
        let p = self.spawns[rng.gen_range(0..self.spawns.len())];
        Some(Vec3::new(p.x + rng.gen_range(-SPAWN_BOX..=SPAWN_BOX), p.y + SPAWN_LIFT + self.spawn_z_offset / 100.0, p.z + rng.gen_range(-SPAWN_BOX..=SPAWN_BOX)))
    }
}

/// Event_OnDestroyed on nest `i` (and what reaches its primary). `from_load` is ActorLoaded's
/// replay: no explosion or XP, and a destroyed primary takes its secondaries with it.
pub fn destroy(nests: &mut [Nest], i: usize, from_load: bool) -> Vec<Effect> {
    let mut out = Vec::new();
    if nests[i].gone {
        return out;
    }
    nests[i].destroyed = true;
    if !from_load {
        out.push(Effect::Explode(i));
        out.push(Effect::Reward(i));
    } else if nests[i].primary {
        for s in nests[i].secondaries.clone() {
            nests[s].gone = true;
            out.push(Effect::Remove(s));
        }
        nests[i].can_damage = true;
        out.push(Effect::Armor(i));
    }
    out.push(Effect::Swap(i));
    // Event_SecondaryDestroyed: AreSecondariesDestroyed? -> CanDamagePrimary?.
    if let Some(p) = nests[i].primary_of
        && !nests[p].can_damage
        && nests[p].secondaries.iter().all(|&s| nests[s].destroyed)
    {
        nests[p].can_damage = true;
        if !from_load {
            out.push(Effect::Explode(p));
        }
        out.push(Effect::Armor(p));
    }
    out
}

/// TraceForActor's SphereTraceSingle radius (500 cm).
const NO_SPAWN_RADIUS: f32 = 5.0;
/// The collision sets of the actors tagged NoSpawn: Buildable_MASTER's children (the Horde
/// Beacon too), keyed from building's COLLIDE_KEY up to the nests' own; no placed level actor
/// has the tag.
const NO_SPAWN_KEYS: std::ops::Range<usize> = super::building::COLLIDE_KEY..COLLIDE_KEY;

/// RandomSpawn: rand(0, 100) <= RandomChance x the multiplier.
pub fn random_spawns(chance: f32, multiplier: f32, roll: f32) -> bool {
    roll <= chance * multiplier
}

/// The ray (unit `dir`) meets the box within `reach`.
pub fn ray_box(origin: Vec3, dir: Vec3, reach: f32, (lo, hi): (Vec3, Vec3)) -> bool {
    let (mut t0, mut t1) = (0.0f32, reach);
    for a in 0..3 {
        let (o, d, l, h) = (origin[a], dir[a], lo[a], hi[a]);
        if d.abs() < 1e-6 {
            if o < l || o > h {
                return false;
            }
            continue;
        }
        let (mut ta, mut tb) = ((l - o) / d, (h - o) / d);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
        if t0 > t1 {
            return false;
        }
    }
    true
}

pub fn box_distance(p: Vec3, (lo, hi): (Vec3, Vec3)) -> f32 {
    p.distance(p.clamp(lo, hi))
}

/// The props to take out of the static cells (the systems holding `PropStreaming` do it).
#[derive(Default)]
struct Hide {
    keys: Vec<PropKey>,
}

#[derive(Resource, Default)]
pub struct Infestations {
    pub nests: Vec<Nest>,
    collected: bool,
    /// BP_InfestationManager's countdown.
    manager: Option<f32>,
    /// Delayed effects (Swap / Armor).
    pending: Vec<(f32, Effect)>,
    hide: Hide,
    /// Spline segments by actor name.
    splines: HashMap<String, Vec<PropKey>>,
    /// The armor props by primary.
    armor: HashMap<usize, PropKey>,
    /// Effects that redraw: the nest's model (and lights, flies) is respawned.
    redraw: Vec<usize>,
}

impl Infestations {
    /// Nest `i`'s state as the save keeps it.
    fn save_into(&self, i: usize, save: &mut InfestationSave) {
        let n = &self.nests[i];
        if n.destroyed {
            save.destroyed.insert(n.name.clone());
        }
        if n.def().is_some_and(|d| n.health < d.health) {
            save.health.insert(n.name.clone(), n.health);
        }
    }

    pub fn by_name(&self, name: &str) -> Option<usize> {
        self.nests.iter().position(|n| n.name == name)
    }
}

/// The drawn nest (its mesh: normal or destroyed).
#[derive(Component)]
pub struct NestModel(pub usize, pub bool);
/// A nest's light or flies.
#[derive(Component)]
struct NestPart(usize);

pub fn plugin(app: &mut App) {
    app.init_resource::<Infestations>()
        .add_systems(OnEnter(AppState::InGame), |mut i: ResMut<Infestations>| *i = Infestations::default())
        .add_systems(Update, (collect, manager, take_out, take_damage, delayed, draw, solid, waves, near_parts, sync).chain().run_if(in_state(AppState::InGame)));
}

fn mesh_stem(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// An actor ref's name ("BP_X_C'LongdownValley:PersistentLevel.BP_X_C_1'" -> "BP_X_C_1").
fn ref_name(object_name: &str) -> String {
    object_name.trim_end_matches('\'').rsplit('.').next().unwrap_or(object_name).to_string()
}

fn refs(props: &HashMap<String, String>, key: &str) -> Vec<String> {
    let Some(v) = props.get(key).and_then(|t| serde_json::from_str::<Vec<serde_json::Value>>(t).ok()) else { return Vec::new() };
    v.iter().filter_map(|r| r.get("ObjectName")?.as_str().map(ref_name)).collect()
}

/// AISpawningTransforms' translations (UE cm, the actor's frame).
fn spawn_points(props: &HashMap<String, String>) -> Vec<[f32; 3]> {
    let Some(v) = props.get("AISpawningTransforms").and_then(|t| serde_json::from_str::<Vec<serde_json::Value>>(t).ok()) else { return Vec::new() };
    v.iter()
        .filter_map(|t| {
            let tr = t.get("Translation")?;
            Some([tr.get("X")?.as_f64()? as f32, tr.get("Y")?.as_f64()? as f32, tr.get("Z")?.as_f64()? as f32])
        })
        .collect()
}

fn num(props: &HashMap<String, String>, key: &str) -> Option<f32> {
    props.get(key).and_then(|v| v.trim_matches('"').parse().ok())
}

fn mesh_path(stem: &str) -> String {
    format!("SurrounDead/Content/Infestation/Meshes/Infestations/{stem}")
}

fn setting_on(store: &SettingsStore, label: &str) -> bool {
    store.find(&row_key("difficulty", "General", label)) == Some(Value::Bool(true))
}

/// SetHealth(StandardHealthValue x INFESTATIONS HEALTH MULTIPLIER) within ±HealthRandomDeviation.
fn starting_health(def: &ClassDef, store: &SettingsStore, rng: &mut impl Rng) -> f32 {
    let h = def.health * super::survival::difficulty(store, "General", "INFESTATIONS HEALTH MULTIPLIER");
    if def.deviation > 0.0 { rng.gen_range(h - def.deviation..=h + def.deviation) } else { h }
}

#[allow(clippy::too_many_arguments)]
fn nest_of(actor: usize, a: &crate::level::ActorInfo, class: &str, mesh: String, destroyed_mesh: String, random: bool, def: &ClassDef, store: &SettingsStore, rng: &mut impl Rng) -> Nest {
    let props = a.props.clone().unwrap_or_default();
    let transform = crate::level::affine(&a.transform);
    let local = |cm: [f32; 3]| transform.transform_point(Vec3::new(cm[0], cm[2], cm[1]) / 100.0);
    let health = starting_health(def, store, rng);
    Nest {
        actor,
        name: a.name.clone(),
        class: class.to_string(),
        transform,
        mesh,
        destroyed_mesh,
        health,
        max_health: def.max_health,
        standard: health,
        primary: def.primary,
        random,
        secondaries: Vec::new(),
        primary_of: None,
        spline_actors: if random { Vec::new() } else { refs(&props, "Splines") },
        spawns: spawn_points(&props).into_iter().map(local).collect(),
        spawn_z_offset: num(&props, "SpawnZOffset").unwrap_or(def.spawn_z_offset),
        destroyed: false,
        gone: false,
        can_damage: !def.primary,
        bare: !def.primary,
        waves: None,
        bounds: None,
        haze: haze_radius(def.primary, random, num(&props, "PostProcessRadius").unwrap_or(def.post_process_radius), transform.scale),
    }
}

/// The construction script's Sphere: PostProcessRadius for a primary, half of it for a random
/// one, scaled by the actor (a sphere takes its smallest scale); others destroy it.
pub fn haze_radius(primary: bool, random: bool, cm: f32, scale: Vec3) -> Option<f32> {
    let r = if primary {
        cm
    } else if random {
        cm / 2.0
    } else {
        return None;
    };
    Some(r / 100.0 * scale.abs().min_element())
}

/// The level's nests (and saved random ones), the save applied.
#[allow(clippy::too_many_arguments)]
fn collect(
    level: Option<Res<Level>>,
    store: Res<SettingsStore>,
    db_items: Res<ItemDb>,
    game: Res<CurrentGame>,
    mut inf: ResMut<Infestations>,
    mut containers: ResMut<Containers>,
    mut markers: ResMut<WorldMarkers>,
) {
    let (Some(level), Some(state)) = (level, game.state.as_ref()) else { return };
    if inf.collected {
        return;
    }
    inf.collected = true;
    let save = state.infestations.clone();
    let mut rng = rand::thread_rng();
    let primaries_on = setting_on(&store, "PRIMARY INFESTATIONS ENABLED?");
    let mut nests = Vec::new();
    let mut hide = Vec::new();
    let mut links: Vec<(usize, Vec<String>)> = Vec::new();
    for (i, a) in level.actors.iter().enumerate() {
        let Some(def) = db().classes.get(&a.class) else { continue };
        let n = nest_of(i, a, &a.class, def.mesh.clone(), def.destroyed_mesh.clone(), false, def, &store, &mut rng);
        if n.primary {
            links.push((nests.len(), a.props.as_ref().map(|p| refs(p, "SecondaryInfestations")).unwrap_or_default()));
        }
        nests.push(n);
    }
    // Primaries and their secondaries.
    for (p, names) in links {
        for name in names {
            if let Some(s) = nests.iter().position(|n| n.name == name) {
                nests[p].secondaries.push(s);
                nests[s].primary_of = Some(p);
            }
        }
    }
    // PRIMARY INFESTATIONS ENABLED? off: each primary destroys its secondaries, splines and itself.
    let mut off: Vec<usize> = Vec::new();
    if !primaries_on {
        for p in (0..nests.len()).filter(|&p| nests[p].primary) {
            off.push(p);
            off.extend(nests[p].secondaries.iter().copied());
        }
    }
    // The saved random infestations.
    let point = &db().random_point;
    for name in save.random.iter().flatten() {
        let Some((i, a)) = level.actors.iter().enumerate().find(|(_, a)| &a.name == name) else { continue };
        let Some(def) = db().classes.get(RANDOM_CLASS) else { continue };
        let props = a.props.clone().unwrap_or_default();
        let mesh = props.get("NormalMesh").map(|s| mesh_path(s.trim_matches('"'))).unwrap_or_else(|| point.mesh.clone());
        let destroyed = props.get("DestroyedMesh").map(|s| mesh_path(s.trim_matches('"'))).unwrap_or_else(|| point.destroyed_mesh.clone());
        nests.push(nest_of(i, a, RANDOM_CLASS, mesh, destroyed, true, def, &store, &mut rng));
    }
    // Every level nest's prop, armor and splines, found now; taken out as they go.
    let mut armor = HashMap::new();
    let mut splines = HashMap::new();
    for (i, n) in nests.iter().enumerate().filter(|(_, n)| !n.random) {
        if let Some((k, _, _)) = level.prop_near(n.transform.translation, mesh_stem(&n.mesh), 0.5) {
            hide.push(k);
        }
        if let Some(path) = n.def().and_then(|d| d.armor.as_ref())
            && let Some((k, _, _)) = level.prop_near(n.transform.translation, mesh_stem(path), 0.5)
        {
            armor.insert(i, k);
        }
        for s in &n.spline_actors {
            if let Some(a) = level.actors.iter().find(|a| &a.name == s) {
                splines.insert(s.clone(), level.splines_at(a.position(), SPLINE_MESH, 0.05));
            }
        }
    }
    inf.armor = armor;
    inf.splines = splines;
    inf.nests = nests;
    for &i in &off {
        inf.nests[i].gone = true;
        inf.nests[i].can_damage = true;
        let keys = take_armor(&mut inf, i);
        hide.extend(keys);
    }
    if !off.is_empty() {
        info!("infestations: PRIMARY INFESTATIONS ENABLED? off, {} nests gone", off.len());
    }
    // The save: health, then ActorLoaded's replay of the destroyed.
    for i in 0..inf.nests.len() {
        if let Some(&h) = save.health.get(&inf.nests[i].name) {
            inf.nests[i].health = h;
        }
    }
    let mut effects = Vec::new();
    for i in 0..inf.nests.len() {
        if save.destroyed.contains(&inf.nests[i].name) || inf.nests[i].health < 0.0 {
            effects.extend(destroy(&mut inf.nests, i, true));
        }
    }
    for e in effects {
        apply(&mut inf, e, &db_items, &mut containers, &mut markers, &save, &mut hide);
    }
    for (i, n) in inf.nests.iter().enumerate() {
        if n.def().is_some_and(|d| d.show_marker) && !n.destroyed && !n.gone {
            markers.0.push(marker(i, n));
        }
    }
    inf.hide.keys.extend(hide);
    // The manager waits for the load, then 4 s.
    inf.manager = Some(MANAGER_DELAY);
    let alive = inf.nests.iter().filter(|n| !n.destroyed && !n.gone).count();
    info!("infestations: {} nests ({} primaries, {} random), {alive} standing", inf.nests.len(), inf.nests.iter().filter(|n| n.primary).count(), inf.nests.iter().filter(|n| n.random).count());
}

/// A primary's armor and spline props (Armor's K2_DestroyComponent, the splines' K2_DestroyActor).
fn take_armor(inf: &mut Infestations, i: usize) -> Vec<PropKey> {
    inf.nests[i].bare = true;
    let mut keys: Vec<PropKey> = inf.armor.remove(&i).into_iter().collect();
    for s in inf.nests[i].spline_actors.clone() {
        keys.extend(inf.splines.remove(&s).unwrap_or_default());
    }
    keys
}

fn marker(i: usize, n: &Nest) -> WorldMarker {
    WorldMarker {
        key: MARKER_KEY + i,
        pos: n.transform.translation,
        texture: MARKER_ICON.into(),
        size: if n.primary { MARKER_SIZE.0 } else { MARKER_SIZE.1 },
        colour: Color::linear_rgb(0.244792, 0.0, 0.0),
        title: n.def().map_or_else(|| "Infestation".into(), |d| d.name.clone()),
        description: if n.primary { PRIMARY_TEXT } else { OTHER_TEXT }.into(),
    }
}

/// The pile's loot as BeginPlay sets it: the class's table and UniqueLootSets.
fn roll_pile(db: &ItemDb, n: &Nest) -> Grid {
    let mut g = Grid::new(PILE, LOOT_COLUMNS, 6);
    let mut rng = rand::thread_rng();
    let Some(def) = n.def() else { return g };
    let table = def.loot_table.as_deref().map(|t| db.roll(t, &mut rng)).unwrap_or_default();
    // A random one gets its spawn point's sets.
    let sets: &[&str] = if n.random { &["Infestation", "MilitarySet", "BanditSet"] } else { &[] };
    let unique = def.unique.iter().map(String::as_str).chain(sets.iter().copied()).flat_map(|u| db.roll_unique(u, &mut rng)).collect::<Vec<_>>();
    for s in table.into_iter().chain(unique) {
        insert_growing(db, &mut g, s);
    }
    g
}

/// The immediate part of an effect; Swap and Armor wait in `pending` unless loading.
fn apply(inf: &mut Infestations, e: Effect, db: &ItemDb, containers: &mut Containers, markers: &mut WorldMarkers, save: &InfestationSave, hide: &mut Vec<PropKey>) {
    match e {
        Effect::Swap(i) => {
            let n = &inf.nests[i];
            if n.gone {
                return;
            }
            markers.0.retain(|m| m.key != MARKER_KEY + i);
            let grid = save.piles.get(&n.name).cloned().unwrap_or_else(|| roll_pile(db, n));
            containers.add_at(n.actor, n.middle(), &n.class, Vec::new());
            containers.searched.insert(n.actor, grid);
            inf.redraw.push(i);
        }
        Effect::Armor(i) => {
            let keys = take_armor(inf, i);
            hide.extend(keys);
        }
        Effect::Remove(i) => {
            inf.nests[i].waves = None;
            let key = inf.nests[i].actor;
            containers.list.retain(|c| c.0 != key);
            containers.searched.remove(&key);
            markers.0.retain(|m| m.key != MARKER_KEY + i);
            inf.redraw.push(i);
        }
        Effect::Explode(_) | Effect::Reward(_) => {}
    }
}

/// Props leaving the static cells: their cells reload without them, their collision is dropped.
fn take_out(mut commands: Commands, mut inf: ResMut<Infestations>, streaming: Option<ResMut<crate::level::PropStreaming>>, mut cache: ResMut<crate::level::collision::CollisionCache>) {
    let Some(mut streaming) = streaming else { return };
    if inf.hide.keys.is_empty() {
        return;
    }
    let mut cells = std::collections::HashSet::new();
    for k in std::mem::take(&mut inf.hide.keys) {
        if let Some(c) = streaming.hide(k) {
            cells.insert(c);
        }
        cache.forget(k);
    }
    for c in cells {
        commands.entity(c).despawn();
    }
}

/// BP_InfestationManager: 4 s on, with every primary destroyed, the random spawn points roll.
#[allow(clippy::too_many_arguments)]
fn manager(time: Res<Time>, level: Option<Res<Level>>, store: Res<SettingsStore>, mut game: ResMut<CurrentGame>, mut inf: ResMut<Infestations>, mut markers: ResMut<WorldMarkers>, rays: crate::level::collision::PropRays) {
    let Some(t) = inf.manager.as_mut() else { return };
    *t -= time.delta_secs();
    if *t > 0.0 {
        return;
    }
    inf.manager = None;
    let (Some(level), Some(state)) = (level, game.state.as_mut()) else { return };
    // AreAllPrimariesDestroyed: an invalid (gone) one doesn't count against it.
    let all = inf.nests.iter().filter(|n| n.primary && !n.gone).all(|n| n.destroyed);
    if !all || state.infestations.random.is_some() {
        return;
    }
    info!("infestations: spawning random infestations");
    let on = setting_on(&store, "RANDOM INFESTATIONS ENABLED?");
    let mult = super::survival::difficulty(&store, "General", "RANDOM INFESTATIONS SPAWN CHANCE MULTIPLIER");
    let point = &db().random_point;
    let Some(def) = db().classes.get(RANDOM_CLASS) else { return };
    let mut rng = rand::thread_rng();
    let mut grown = Vec::new();
    for (i, a) in level.actors.iter().enumerate().filter(|(_, a)| a.class == "BP_RandomInfestationSpawnPoint_C") {
        let props = a.props.clone().unwrap_or_default();
        let chance = num(&props, "RandomChance").unwrap_or(point.chance);
        if !(on && random_spawns(chance, mult, rng.gen_range(0.0..=100.0))) {
            continue;
        }
        // TraceForActor: a 5 m sphere touching a NoSpawn actor (the player's buildables).
        if rays.dynamic_within(NO_SPAWN_KEYS, a.position(), NO_SPAWN_RADIUS) {
            info!("infestations: {} by a buildable, no infestation", a.name);
            continue;
        }
        let mesh = props.get("NormalMesh").map(|s| mesh_path(s.trim_matches('"'))).unwrap_or_else(|| point.mesh.clone());
        let destroyed = props.get("DestroyedMesh").map(|s| mesh_path(s.trim_matches('"'))).unwrap_or_else(|| point.destroyed_mesh.clone());
        let n = nest_of(i, a, RANDOM_CLASS, mesh, destroyed, true, def, &store, &mut rng);
        let k = inf.nests.len();
        markers.0.push(marker(k, &n));
        info!("infestations: random infestation at {} ({:.0})", a.name, n.transform.translation);
        grown.push(a.name.clone());
        inf.nests.push(n);
    }
    state.infestations.random = Some(grown);
}

/// Damage_Object from rounds, swings and blasts.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn take_damage(
    mut inf: ResMut<Infestations>,
    (mut shots, mut swings, mut blasts): (MessageReader<super::turrets::Shot>, MessageReader<super::harvest::Swing>, MessageReader<super::barrels::ObjectBlast>),
    store: Res<SettingsStore>,
    db_items: Res<ItemDb>,
    mut game: ResMut<CurrentGame>,
    mut containers: ResMut<Containers>,
    mut markers: ResMut<WorldMarkers>,
    (mut notify, mut sound, mut fx): (MessageWriter<Notify>, MessageWriter<crate::audio::Sound>, MessageWriter<super::particles::Fx>),
) {
    let mut hits: Vec<(usize, f32)> = Vec::new();
    let live = |n: &Nest| !n.destroyed && !n.gone;
    for s in shots.read() {
        // The nearest nest the round's line passes through.
        let best = inf.nests.iter().enumerate().filter(|(_, n)| live(n)).filter_map(|(i, n)| n.bounds.filter(|b| ray_box(s.origin, s.dir, s.reach + 0.5, *b)).map(|b| (i, box_distance(s.origin, b)))).min_by(|a, b| a.1.total_cmp(&b.1));
        hits.extend(best.map(|(i, _)| (i, s.damage)));
    }
    for s in swings.read() {
        let chest = s.feet + Vec3::Y * CHEST;
        for (i, n) in inf.nests.iter().enumerate().filter(|(_, n)| live(n)) {
            let Some(b) = n.bounds else { continue };
            let near = chest.clamp(b.0, b.1);
            let flat = (near - chest).with_y(0.0);
            if chest.distance(near) <= SWING_REACH && (flat.length() < 0.3 || flat.normalize().dot(s.facing) > 0.3) {
                hits.push((i, s.damage));
            }
        }
    }
    for x in blasts.read() {
        hits.extend(inf.nests.iter().enumerate().filter(|(_, n)| live(n)).filter(|(_, n)| n.bounds.is_some_and(|b| box_distance(x.at, b) <= x.radius)).map(|(i, _)| (i, x.damage)));
    }
    if hits.is_empty() {
        return;
    }
    let Some(state) = game.state.as_mut() else { return };
    let mut rng = rand::thread_rng();
    let mut hide = Vec::new();
    let save = state.infestations.clone();
    for (i, dmg) in hits {
        let at = inf.nests[i].transform.translation;
        match inf.nests[i].hit(dmg) {
            Hit::Ignored => {}
            Hit::Refused => {
                notify.write(Notify::new(REFUSED).colored(Color::linear_rgb(0.46875, 0.0, 0.0)));
                info!("infestations: {} refused {dmg:.0} (secondaries standing)", inf.nests[i].name);
            }
            Hit::Damaged { waves } => {
                sound.write(crate::audio::Sound::at(DAMAGE_SOUND, at));
                info!("infestations: {} hit for {dmg:.0}: {:.0} of {:.0} left", inf.nests[i].name, inf.nests[i].health, inf.nests[i].max_health);
                if waves && let Some(def) = inf.nests[i].def() {
                    inf.nests[i].waves = Some(Waves::start(&def.waves));
                    info!("infestations: {} starting to spawn waves of enemies", inf.nests[i].name);
                }
            }
            Hit::Destroyed => {
                sound.write(crate::audio::Sound::at(DEATH_SOUND, at));
                for e in destroy(&mut inf.nests, i, false) {
                    match e {
                        Effect::Explode(k) => {
                            fx.write(super::particles::Fx::at(EXPLOSION, inf.nests[k].explosion_at()));
                        }
                        Effect::Reward(k) => {
                            let n = &inf.nests[k];
                            state.stats.infestations_destroyed += 1;
                            let (lo, hi) = if n.primary { PRIMARY_XP } else { OTHER_XP };
                            let xp = super::combat::kill_xp(state.progression.level, rng.gen_range(lo..=hi), super::survival::difficulty(&store, "General", "XP MULTIPLIER"));
                            state.progression.add_xp(xp.round() as u64);
                            if store.find(&row_key("settings", "Gameplay", "XP POP UP ENABLED?")) == Some(Value::Bool(true)) {
                                notify.write(Notify::new(format!("Infestation Destroyed, XP Given: {}", xp.trunc())).colored(Color::linear_rgb(1.0, 0.0, 0.0)));
                            }
                            info!("infestations: {} destroyed, {xp:.0} XP, {} destroyed so far", n.name, state.stats.infestations_destroyed);
                        }
                        Effect::Swap(_) | Effect::Armor(_) => inf.pending.push((SWAP_DELAY, e)),
                        Effect::Remove(_) => apply(&mut inf, e, &db_items, &mut containers, &mut markers, &save, &mut hide),
                    }
                }
            }
        }
        inf.save_into(i, &mut state.infestations);
    }
    inf.hide.keys.extend(hide);
}

/// The 0.2 s Delays.
fn delayed(time: Res<Time>, mut inf: ResMut<Infestations>, db_items: Res<ItemDb>, game: Res<CurrentGame>, mut containers: ResMut<Containers>, mut markers: ResMut<WorldMarkers>) {
    if inf.pending.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let mut due = Vec::new();
    inf.pending.retain_mut(|(t, e)| {
        *t -= dt;
        if *t <= 0.0 {
            due.push(*e);
            false
        } else {
            true
        }
    });
    let save = game.state.as_ref().map(|s| s.infestations.clone()).unwrap_or_default();
    let mut hide = Vec::new();
    for e in due {
        apply(&mut inf, e, &db_items, &mut containers, &mut markers, &save, &mut hide);
        info!("infestations: {e:?} done");
    }
    inf.hide.keys.extend(hide);
}

/// Each nest's mesh (normal or destroyed), its lights and its flies.
fn draw(mut commands: Commands, assets: Res<AssetServer>, mut inf: ResMut<Infestations>, models: Query<(Entity, &NestModel)>, parts: Query<(Entity, &NestPart)>, mut rays: crate::level::collision::PropRays) {
    let mut have: HashMap<usize, (Entity, bool)> = models.iter().map(|(e, m)| (m.0, (e, m.1))).collect();
    for i in std::mem::take(&mut inf.redraw) {
        if let Some((e, _)) = have.remove(&i) {
            commands.entity(e).despawn();
        }
        for (e, p) in &parts {
            if p.0 == i {
                commands.entity(e).despawn();
            }
        }
        rays.set_dynamic(COLLIDE_KEY + i, Vec::new());
        inf.nests[i].bounds = None;
    }
    for (i, n) in inf.nests.iter().enumerate() {
        if n.gone || have.contains_key(&i) {
            continue;
        }
        let path = if n.destroyed { &n.destroyed_mesh } else { &n.mesh };
        commands.spawn((crate::models::model(&assets, format!("game/{path}.glb")), n.transform, NestModel(i, n.destroyed), DespawnOnExit(AppState::InGame)));
        if n.destroyed {
            continue;
        }
        let Some(def) = n.def() else { continue };
        for l in &def.lights {
            commands.spawn((
                PointLight { color: Color::srgb(l.color[0], l.color[1], l.color[2]), intensity: crate::level::lights::lumens(LIGHT_CANDELA), range: l.radius.max(1.0), shadow_maps_enabled: false, ..default() },
                Transform::from_translation(n.at_ue(l.at)),
                Visibility::Hidden,
                NestPart(i),
                DespawnOnExit(AppState::InGame),
            ));
        }
        commands.spawn((
            Transform::from_translation(n.at_ue(def.flies)),
            Visibility::default(),
            super::particles::Emitter { active: false, ..super::particles::Emitter::new("FX_Flies") },
            super::particles::LevelFx { owner: n.class.clone(), actor: n.name.clone(), range: FLIES_RANGE, out: false },
            NestPart(i),
            DespawnOnExit(AppState::InGame),
        ));
        have.insert(i, (Entity::PLACEHOLDER, n.destroyed));
    }
}

/// Collision and bounds from each drawn mesh, once loaded.
fn solid(assets: Res<AssetServer>, mut inf: ResMut<Infestations>, mut rays: crate::level::collision::PropRays, meshes: Res<Assets<Mesh>>, mut handles: Local<HashMap<String, Handle<Mesh>>>) {
    for i in 0..inf.nests.len() {
        let n = &inf.nests[i];
        if n.gone || n.bounds.is_some() {
            continue;
        }
        let path = format!("game/{}.glb", if n.destroyed { &n.destroyed_mesh } else { &n.mesh });
        let handle = handles.entry(path.clone()).or_insert_with(|| assets.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(path))).clone();
        let Some(mesh) = meshes.get(&handle) else { continue };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { continue };
        let t = n.transform;
        let (lo, hi) = pos.iter().map(|p| t.transform_point(Vec3::from_array(*p))).fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
        rays.set_dynamic_from(COLLIDE_KEY + i, &handle, &t);
        inf.nests[i].bounds = Some((lo, hi));
    }
}

/// SpawnWave for every nest whose waves are running.
#[allow(clippy::type_complexity)]
fn waves(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut rays: crate::level::collision::PropRays,
    mut inf: ResMut<Infestations>,
    player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>,
) {
    let dt = time.delta_secs();
    let me = player.single().ok().map(|t| t.translation);
    let mut rng = rand::thread_rng();
    for i in 0..inf.nests.len() {
        let Some(def) = inf.nests[i].def() else { continue };
        let Some(w) = inf.nests[i].waves.as_mut() else { continue };
        let (out, wave) = (w.tick(dt, &def.waves, &mut rng), w.wave);
        for class in out {
            let Some(kind) = super::zombies::ZombieKind::from_class(&class) else {
                warn!("infestations: no zombie class {class}");
                continue;
            };
            let Some(p) = inf.nests[i].spawn_point(&mut rng) else {
                warn!("infestations: {} has no valid transforms", inf.nests[i].name);
                continue;
            };
            // The pawn drops from its capsule's place to the floor below.
            let prop = rays.cast(p + Vec3::Y * 0.5, Vec3::NEG_Y, 6.0).map(|d| p.y + 0.5 - d);
            let land = terrain.as_ref().and_then(|t| t.ground_under(p + Vec3::Y * 0.5));
            let feet = p.with_y(prop.into_iter().chain(land).reduce(f32::max).unwrap_or(p.y));
            let (model, material) = kind.pick_look(&mut rng);
            let lz = crate::level::LevelZombie { home: feet, spawner: None, model, material, kind };
            let e = crate::level::spawners::spawn_zombie(&mut commands, &assets, Transform::from_translation(feet).with_rotation(Quat::from_rotation_y(rng.gen_range(0.0..std::f32::consts::TAU))), lz);
            if let Some(me) = me {
                commands.entity(e).insert(super::building::LockOnTarget(me));
            }
            info!("infestations: {} wave {} spawned {class} at {feet:.1}", inf.nests[i].name, wave + 1);
        }
    }
}

/// The lights within range of the survivor.
#[allow(clippy::type_complexity)]
fn near_parts(player: Query<&Transform, With<CharacterModel>>, mut lights: Query<(&Transform, &mut Visibility), (With<NestPart>, With<PointLight>, Without<CharacterModel>)>) {
    let Some(me) = player.iter().next().map(|t| t.translation) else { return };
    for (t, mut v) in &mut lights {
        v.set_if_neq(if t.translation.distance(me) <= LIGHT_RANGE { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// The piles' contents into the save; each pile's container point follows the survivor over its
/// bounds (reach is to the mesh, as the game's look trace is, not to its middle).
fn sync(inf: Res<Infestations>, mut containers: ResMut<Containers>, mut game: ResMut<CurrentGame>, player: Query<&Transform, (With<CharacterModel>, With<crate::level::KinematicBody>)>) {
    if let Ok(me) = player.single() {
        for n in inf.nests.iter().filter(|n| n.destroyed && !n.gone) {
            let Some((lo, hi)) = n.bounds else { continue };
            let at = me.translation.clamp(lo, hi);
            if let Some(c) = containers.list.iter_mut().find(|c| c.0 == n.actor) {
                c.1 = at;
            }
        }
    }
    let Some(state) = game.bypass_change_detection().state.as_mut() else { return };
    for n in inf.nests.iter().filter(|n| n.destroyed && !n.gone) {
        if let Some(g) = containers.searched.get(&n.actor)
            && state.infestations.piles.get(&n.name) != Some(g)
        {
            state.infestations.piles.insert(n.name.clone(), g.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nest(name: &str, primary: bool) -> Nest {
        Nest {
            actor: 0,
            name: name.into(),
            class: if primary { "BP_PrimaryInfestation_C" } else { "BP_Infestation_Floor_C" }.into(),
            transform: Transform::IDENTITY,
            mesh: String::new(),
            destroyed_mesh: String::new(),
            health: if primary { 2500.0 } else { 1000.0 },
            max_health: if primary { 2500.0 } else { 1000.0 },
            standard: if primary { 2500.0 } else { 1000.0 },
            primary,
            random: false,
            secondaries: Vec::new(),
            primary_of: None,
            spline_actors: Vec::new(),
            spawns: Vec::new(),
            spawn_z_offset: 0.0,
            destroyed: false,
            gone: false,
            can_damage: !primary,
            bare: !primary,
            waves: None,
            bounds: None,
            haze: primary.then_some(50.0),
        }
    }

    #[test]
    fn the_haze_weighs_in_near_live_nests() {
        use super::super::post_volumes::{INFESTATION, look_at, weight};
        let grain = |v: &[Nest], eye: Vec3| look_at(None, Some(&Infestations { nests: v.to_vec(), ..default() }), eye).grain / INFESTATION.grain.unwrap();
        let mut v = site();
        let p = &v[0];
        // Inside 50 m: full; 50.5 m out: half (the 1 m blend); beyond: none. Secondaries: none.
        assert_eq!(weight(p, Vec3::new(30.0, 0.0, 0.0)), 1.0);
        assert!((weight(p, Vec3::new(50.5, 0.0, 0.0)) - 0.5).abs() < 1e-4);
        assert_eq!(weight(p, Vec3::new(52.0, 0.0, 0.0)), 0.0);
        assert_eq!(weight(&v[1], Vec3::ZERO), 0.0);
        // Two half weights blend to 0.75; a destroyed nest's PostProcess is gone.
        let mut q = nest("Q", true);
        q.transform.translation = Vec3::new(101.0, 0.0, 0.0);
        v.push(q);
        assert!((grain(&v, Vec3::new(50.5, 0.0, 0.0)) - 0.75).abs() < 1e-4);
        v[0].destroyed = true;
        assert!((grain(&v, Vec3::new(50.5, 0.0, 0.0)) - 0.5).abs() < 1e-4);
    }

    /// A primary (0) with two secondaries (1, 2).
    fn site() -> Vec<Nest> {
        let mut v = vec![nest("P", true), nest("A", false), nest("B", false)];
        v[0].secondaries = vec![1, 2];
        v[1].primary_of = Some(0);
        v[2].primary_of = Some(0);
        v
    }

    #[test]
    fn data_has_the_classes() {
        let d = db();
        let p = &d.classes["BP_PrimaryInfestation_C"];
        assert!(p.primary && p.show_marker && p.armor.is_some());
        assert_eq!((p.health, p.loot_table.as_deref()), (2500.0, Some("PrimaryInfestation")));
        assert_eq!(p.unique, ["Infestation", "MilitarySet", "BanditSet"]);
        assert_eq!(p.waves.len(), 4);
        assert_eq!(p.waves[3], vec![("BP_ZombieBossHorde_Infestation_C".to_string(), 1), ("BP_Zombie_Infected_Horde_C".to_string(), 8)]);
        let f = &d.classes["BP_Infestation_Floor_C"];
        assert!(!f.primary && !f.show_marker && f.armor.is_none());
        assert_eq!((f.health, f.name.as_str(), f.loot_table.as_deref()), (1000.0, "Secondary Infestation", Some("Infestation")));
        let r = &d.classes[RANDOM_CLASS];
        assert_eq!((r.health, r.deviation, r.name.as_str()), (1750.0, 250.0, "Infestation"));
        // The lights inherit BP_Infestation's colour and radius.
        assert!((f.lights[0].color[0] - 161.0 / 255.0).abs() < 1e-3 && f.lights[0].radius == 5.0);
        assert_eq!(d.random_point.chance, 20.0);
    }

    #[test]
    fn primaries_refuse_until_their_secondaries_fall() {
        let mut v = site();
        assert_eq!(v[0].hit(100.0), Hit::Refused);
        assert_eq!(v[0].health, 2500.0);
        // 1000 -> 400: under 1000 / 1.5, the waves start; once.
        assert_eq!(v[1].hit(600.0), Hit::Damaged { waves: true });
        v[1].waves = Some(Waves::start(&[]));
        assert_eq!(v[1].hit(100.0), Hit::Damaged { waves: false });
        // Exactly 0 is still alive (StandardDamage at >= 0).
        assert_eq!(v[1].hit(300.0), Hit::Damaged { waves: false });
        assert_eq!(v[1].hit(1.0), Hit::Destroyed);
        let e = destroy(&mut v, 1, false);
        assert_eq!(e, [Effect::Explode(1), Effect::Reward(1), Effect::Swap(1)]);
        assert_eq!(v[1].hit(50.0), Hit::Ignored);
        assert!(!v[0].can_damage);
        v[2].hit(2000.0);
        let e = destroy(&mut v, 2, false);
        assert_eq!(e, [Effect::Explode(2), Effect::Reward(2), Effect::Swap(2), Effect::Explode(0), Effect::Armor(0)]);
        assert!(v[0].can_damage);
        // One hit that skips StandardDamage starts no waves.
        assert_eq!(v[0].hit(3000.0), Hit::Destroyed);
        assert!(v[0].waves.is_none());
    }

    #[test]
    fn a_loaded_destroyed_primary_takes_its_secondaries() {
        let mut v = site();
        let e = destroy(&mut v, 0, true);
        assert_eq!(e, [Effect::Remove(1), Effect::Remove(2), Effect::Armor(0), Effect::Swap(0)]);
        assert!(v[1].gone && v[2].gone && v[0].destroyed);
        // A loaded destroyed secondary: no explosion or XP, but its primary loses the armor.
        let mut v = site();
        destroy(&mut v, 1, true);
        assert_eq!(destroy(&mut v, 2, true), [Effect::Swap(2), Effect::Armor(0)]);
        assert!(v[0].can_damage && !v[0].destroyed);
    }

    #[test]
    fn waves_spawn_one_at_a_time_then_wait() {
        let def = vec![vec![("Z".to_string(), 2)], vec![("Z".to_string(), 1), ("B".to_string(), 1)]];
        let mut w = Waves::start(&def);
        let mut rng = rand::thread_rng();
        assert_eq!(w.tick(0.0, &def, &mut rng), ["Z"]);
        assert!(w.tick(0.9, &def, &mut rng).is_empty());
        assert_eq!(w.tick(0.6, &def, &mut rng), ["Z"]);
        // 1-1.5 s after the last, the next wave is 30 s off.
        assert!(w.tick(1.5, &def, &mut rng).is_empty());
        assert_eq!(w.wave, 1);
        assert!(w.tick(29.0, &def, &mut rng).is_empty());
        assert_eq!(w.tick(1.0, &def, &mut rng), ["Z"]);
        assert_eq!(w.tick(1.5, &def, &mut rng), ["B"]);
        w.tick(40.0, &def, &mut rng);
        assert!(w.done);
        assert!(w.tick(100.0, &def, &mut rng).is_empty());
    }

    #[test]
    fn spawn_points_are_about_the_transforms() {
        let mut n = nest("A", false);
        n.spawns = vec![Vec3::new(10.0, 5.0, -3.0)];
        n.spawn_z_offset = 50.0;
        let mut rng = rand::thread_rng();
        for _ in 0..50 {
            let p = n.spawn_point(&mut rng).unwrap();
            assert!((p.x - 10.0).abs() <= 2.0 && (p.z + 3.0).abs() <= 2.0);
            assert!((p.y - 5.6).abs() < 1e-4);
        }
        assert!(nest("B", false).spawn_point(&mut rng).is_none());
    }

    #[test]
    fn hits_and_rolls() {
        let b = (Vec3::new(4.0, 0.0, -1.0), Vec3::new(6.0, 2.0, 1.0));
        assert!(ray_box(Vec3::new(0.0, 1.0, 0.0), Vec3::X, 10.0, b));
        assert!(!ray_box(Vec3::new(0.0, 1.0, 0.0), Vec3::X, 3.5, b), "short of it");
        assert!(!ray_box(Vec3::new(0.0, 3.0, 0.0), Vec3::X, 10.0, b), "over it");
        assert!(!ray_box(Vec3::new(0.0, 1.0, 0.0), -Vec3::X, 10.0, b), "behind");
        assert!((box_distance(Vec3::new(0.0, 1.0, 0.0), b) - 4.0).abs() < 1e-5);
        assert!(random_spawns(20.0, 1.0, 20.0) && !random_spawns(20.0, 1.0, 20.5) && !random_spawns(15.0, 0.0, 0.1));
        assert_eq!(ref_name("BP_Infestation_Ceiling_C'LongdownValley:PersistentLevel.BP_Infestation_Ceiling_C_1'"), "BP_Infestation_Ceiling_C_1");
    }

    #[test]
    fn the_level_links_its_nests() {
        let Ok(t) = std::fs::read_to_string(format!("{}/actors.json", crate::level::LEVEL_DIR)) else { return };
        let actors: Vec<crate::level::ActorInfo> = serde_json::from_str(&t).unwrap();
        let primaries: Vec<_> = actors.iter().filter(|a| a.class == "BP_PrimaryInfestation_C").collect();
        assert_eq!(primaries.len(), 12);
        let secondaries: usize = primaries.iter().map(|a| refs(a.props.as_ref().unwrap(), "SecondaryInfestations").len()).sum();
        assert_eq!(secondaries, 19);
        for a in &primaries {
            let p = a.props.as_ref().unwrap();
            assert_eq!(refs(p, "Splines").len(), refs(p, "SecondaryInfestations").len(), "{}", a.name);
            assert!(!spawn_points(p).is_empty(), "{}", a.name);
            for s in refs(p, "SecondaryInfestations") {
                assert!(actors.iter().any(|b| b.name == s && db().classes.contains_key(&b.class)), "{s}");
            }
        }
        assert_eq!(actors.iter().filter(|a| a.class == "BP_RandomInfestationSpawnPoint_C").count(), 12);
    }
}
