//! Radiation zones, respirators and the decontamination shower, from `Blueprints/Zones/
//! BP_RadiationArea_Sphere`, `Blueprints/Components/RadiationComponent`,
//! `GameFunctionLibrary.GetRadiationLevel / ApplyRadiationDamage` and
//! `Buildable_DecontaminationShower` (decompiled):
//!
//! - **Zones**: the level's 10 BP_RadiationArea_Sphere (Sphere radius = ZoneRadius, 10000 cm
//!   unless the instance sets it; RadiationLevel Level 1-3; Name). The survivor beginning to
//!   overlap one (also polled every 0.2 s from BeginPlay + 3 s, for a game that starts inside) is
//!   Event_EnteredRadZone (a DoOnce, reset on leaving): HUD_Game.ZoneAreaUI(true, Name, UI_filter,
//!   white), with AdditionalNotification? its "Increased Radiation [Level 1]"... notification for
//!   5 s, then RadiationComponent.UpdateRadiation(GetRadiationLevel) and CheckStats. Leaving is
//!   ZoneAreaUI(false, ...) and StopRadiation.
//! - GetRadiationLevel: the RadiationTimer period, 0.2 / 0.08 / 0.04 s for Level 1 / 2 / 3,
//!   each divided by Difficulty.RadSeverityLvl1 (all three levels read the Level 1 row: the BP's
//!   own; the Level 2 / 3 rows do nothing in the game).
//! - RadiationTimer (each period): with a respirator in the Facewear slot, SetFilterUI(true) and
//!   UseFilter(its FilterUsagePerSecond stat, per tick despite the name): the filter (the mask's
//!   attachment) loses that x Difficulty.RadFilterDepletionMultiplier of durability; working, the
//!   meter drops ReduceRadiationAmount (0.75) / 2 x (1 - RadiationProtection / 100). "No Filter"
//!   (4 s) without one, and "Filter No Longer Working\r\nFind Another..." (8 s) when it runs out,
//!   then the full 0.75. Without a respirator: 0.75 and 0.1-0.25 Toughness XP.
//! - The meter (CurrentRadiation, 100 = clean; ours counts up, see `medical.rs`) at 0 is radiation
//!   sickness (`MedicalEffects::tick`: 15 damage per 5 s).
//! - DecreaseRadiation's Geiger: with Character.GeigerSound, once the meter is at 99 or less,
//!   Geiger_Counter_Sound_Cue in 2D at (Max - Current) / 100, faded in over 1 s; UpdateGeiger's
//!   DoOnce + RetriggerableDelay(6-8 s) fades it out and destroys it 6-8 s after it started, and
//!   the next decrease starts it again. IncreaseRadiation destroys it.
//! - The zone's PostProcess (unbound off, BlendRadius 250 cm): ColorSaturation 0.75, a faint
//!   green ColorOffset, fringe, vignette 0.25, grain 0.5 inside the sphere, blended over 2.5 m
//!   (`post_volumes`).
//! - **Radiated water** (BP_WaterBox / BP_WaterSpline with Radiated?: BP_WaterBox_C_0 and
//!   BP_WaterSpline_C_1): its mesh's begin overlap with a swimmer (SwimmingComponent) is
//!   RadiationComponent.UpdateRadiation(0.15) and CheckStats (no difficulty divisor, no banner);
//!   the end overlap is StopRadiation, whatever zone the survivor is also in.
//! - ApplyRadiationDamage (zombies' and bosses' hits): Chance for Rad Damage, then the dose, x
//!   (1 - RadiationProtection / 100) when CanMaskProtect? (zombies yes, bosses no) and the mask
//!   has a filter (`hit_dose`).
//! - **Decontamination shower** (Buildable_DecontaminationShower, Requires Power?): Interact plays
//!   the grab montage; without power "No power", during its Cooldown (300 s) "Decontamination
//!   shower cooldown in effect..."; else CooldownRunning?, DecontaminatePlayer (a 50 x 85 x 100 cm
//!   box at Trace, 120 cm up: the survivor in it gets IncreaseRadiation(100), behind a DoOnce
//!   the BP never resets, so each shower cleans once per load), FX_Water and FX_Water2
//!   (FX_Particle_Water) Activate, Deactivate 3 s later. Light / Light2 (M_EmissiveColour_*,
//!   red as built): the power delegates (bound 1 s after BeginPlay) turn them green on Power On
//!   unless cooling and red on Power Off; OnRep_CooldownRunning? orange while it cools, green
//!   when it ends (powered or not).
//! - The Radiation weather is UDW's preset only (fog, dust): no Blueprint reacts to it.
//!
//! Ours: the water's overlap is the capsule against the Cube (the 4 m SplineWaterTEST at
//! Scale X / Y / Z) or the swept spline segments (`SplineMesh::contains`), checked every frame;
//! the power scan's repeated On / Off are taken as edges. Not ported: NS_RadiationDust (a GPU
//! Niagara system: its values are compiled into the cooked shader), the radiated
//! water's look (Mat_Clear_Water_Radiated_Inst's olive ShallowWater: water props share one
//! material per mesh here).

use bevy::prelude::*;
use rand::Rng;

use super::equipment::{PlayerInventory, Slot};
use super::items::{ItemDb, Stack};
use super::notify::Notify;
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::{AppState, CurrentGame};

/// RadiationComponent.ReduceRadiationAmount.
pub const REDUCE: f32 = 0.75;
/// BP_RadiationArea_Sphere's ZoneRadius default (cm).
const ZONE_RADIUS: f32 = 10000.0;
/// Its overlap poll.
const POLL: f32 = 0.2;
/// The PostProcess component's BlendRadius (m); its settings are `post_volumes::RADIATION`.
const BLEND: f32 = 2.5;
pub const ICON: &str = "game/SurrounDead/Content/UI/Textures/UI_filter.png";
const GEIGER: &str = "Geiger_Counter_Sound_Cue";
/// The shower's Cooldown, its DecontaminatePlayer box (half extents, cm) and Trace offset.
pub const SHOWER_COOLDOWN: f32 = 300.0;
const SHOWER_BOX: Vec3 = Vec3::new(0.5, 1.0, 0.85);
const SHOWER_TRACE: f32 = 1.2;
pub const SHOWER: &str = "Buildable_DecontaminationShower_C";
/// FX_Water and FX_Water2 (FX_Particle_Water): location (cm), pitch, yaw, roll in the actor.
const SHOWER_WATER: [([f32; 3], f32, f32, f32); 2] = [([48.984272, 0.0, 220.73384], 35.0, 180.0, 180.0), ([-47.03076, 0.0, 220.99977], -50.0, 0.0, 0.0)];
const WATER_FX: &str = "FX_Particle_Water";
/// The Delay before FX_Water's Deactivate.
const WATER_FOR: f32 = 3.0;

#[derive(Clone, Debug, PartialEq)]
pub struct RadZone {
    pub name: String,
    pub centre: Vec3,
    pub radius: f32,
    /// Enum_RadiationLevel: 0-2.
    pub level: u8,
    /// AdditionalNotification?'s message.
    pub notification: Option<String>,
}

impl RadZone {
    pub fn from_actor(a: &crate::level::ActorInfo) -> Option<Self> {
        if a.class != "BP_RadiationArea_Sphere_C" {
            return None;
        }
        let p = a.props.clone().unwrap_or_default();
        let radius = p.get("ZoneRadius").and_then(|r| r.parse::<f32>().ok()).unwrap_or(ZONE_RADIUS) / 100.0;
        let level = match p.get("RadiationLevel").map(String::as_str) {
            Some(l) if l.contains("NewEnumerator2") => 2,
            Some(l) if l.contains("NewEnumerator1") => 1,
            _ => 0,
        };
        let text = |k: &str| p.get(k).and_then(|j| crate::level::doors::ftext(j));
        let notification = (p.get("AdditionalNotification?").map(String::as_str) == Some("true")).then(|| text("Notification Details Message")).flatten();
        Some(Self { name: text("Name").unwrap_or_default(), centre: a.position(), radius, level, notification })
    }

    /// The PostProcess blend weight at `p`: 1 inside, down to 0 at BlendRadius outside.
    pub fn weight(&self, p: Vec3) -> f32 {
        (1.0 - (p.distance(self.centre) - self.radius).max(0.0) / BLEND).clamp(0.0, 1.0)
    }
}

/// GetRadiationLevel: the RadiationTimer period for a level.
pub fn period(level: u8, severity: f32) -> f32 {
    let base = match level {
        0 => 0.2,
        1 => 0.08,
        _ => 0.04,
    };
    base / severity.max(0.01)
}

/// The respirator in the Facewear slot (an item with a FilterUsagePerSecond stat).
pub fn respirator<'a>(inv: &'a PlayerInventory, db: &ItemDb) -> Option<&'a Stack> {
    inv.equipped.get(&Slot::Facewear).filter(|s| is_respirator(db, &s.item))
}

pub fn is_respirator(db: &ItemDb, item: &str) -> bool {
    db.items.get(item).is_some_and(|d| d.random_stats.iter().any(|r| r.stat == "FilterUsagePerSecond"))
}

/// GetRespiratorStat's MinValue (the rolled stat; 0 when the instance has none).
fn stat(mask: &Stack, name: &str) -> f32 {
    mask.stats.get(name).copied().unwrap_or(0.0)
}

/// What one RadiationTimer tick did.
#[derive(Debug, PartialEq)]
pub struct Tick {
    /// Added to our meter (the game's DecreaseRadiation amount).
    pub dose: f32,
    pub toast: Option<(&'static str, f32)>,
    /// The respirator is on (SetFilterUI(true)).
    pub filter_ui: bool,
    /// AddXP_Toughness (no respirator).
    pub toughness: bool,
}

/// RadiationTimer + UseFilter on the inventory.
pub fn tick(inv: &mut PlayerInventory, db: &ItemDb, depletion: f32) -> Tick {
    let Some(mask) = inv.equipped.get_mut(&Slot::Facewear).filter(|s| is_respirator(db, &s.item)) else {
        return Tick { dose: REDUCE, toast: None, filter_ui: false, toughness: true };
    };
    let (usage, protection) = (stat(mask, "FilterUsagePerSecond"), stat(mask, "RadiationProtection"));
    let Some(filter) = super::flashlight::battery(mask, db).map(|f| f.item.clone()).and_then(|f| mask.attachments.iter_mut().find(|a| a.item == f)) else {
        return Tick { dose: REDUCE, toast: Some(("No Filter", 4.0)), filter_ui: true, toughness: false };
    };
    let left = (super::flashlight::charge(filter, db) - usage * depletion).max(0.0);
    filter.durability = Some(left);
    if left == 0.0 {
        return Tick { dose: REDUCE, toast: Some(("Filter No Longer Working\r\nFind Another...", 8.0)), filter_ui: true, toughness: false };
    }
    Tick { dose: REDUCE / 2.0 * (1.0 - protection / 100.0), toast: None, filter_ui: true, toughness: false }
}

/// ApplyRadiationDamage's dose once its chance passed.
pub fn hit_dose(inv: Option<&PlayerInventory>, db: &ItemDb, dose: f32, mask_protects: bool) -> f32 {
    let Some(mask) = inv.and_then(|i| respirator(i, db)) else { return dose };
    let filtered = super::flashlight::battery(mask, db).is_some();
    if mask_protects && filtered { dose * (1.0 - stat(mask, "RadiationProtection") / 100.0) } else { dose }
}

/// The filter's durability left, whole percent (GetRespiratorDurability).
pub fn filter_left(inv: &PlayerInventory, db: &ItemDb) -> Option<f32> {
    let mask = respirator(inv, db)?;
    super::flashlight::battery(mask, db).map(|f| super::flashlight::charge(f, db))
}

/// The zones and the survivor's RadiationComponent state.
#[derive(Resource, Default)]
pub struct Radiation {
    pub zones: Vec<RadZone>,
    pub water: Vec<RadWater>,
    /// The radiated water the survivor is in.
    pub in_water: Option<usize>,
    collected: bool,
    /// The zones overlapping the survivor.
    pub inside: Vec<usize>,
    poll: f32,
    /// RadiationHandle: the running timer's period and time to the next tick.
    pub timer: Option<(f32, f32)>,
    /// SetFilterUI.
    pub filter_ui: bool,
    /// The Geiger's time left (UpdateGeiger's 6-8 s) and its age (the 1 s FadeIn).
    geiger: Option<(f32, f32)>,
    /// The meter last frame: a drop is IncreaseRadiation (an anti-rad item, the shower), which
    /// destroys the Geiger.
    last_level: f32,
}

#[derive(Component)]
pub struct Geiger;

pub fn plugin(app: &mut App) {
    app.init_resource::<Radiation>()
        .add_systems(OnEnter(AppState::InGame), |mut r: ResMut<Radiation>| *r = Radiation::default())
        .add_systems(Update, (collect, track, irradiate, geiger, run_showers, paint_shower_lamps).chain().run_if(in_state(AppState::InGame)));
}

fn collect(level: Option<Res<crate::level::Level>>, mut rad: ResMut<Radiation>) {
    let Some(level) = level else { return };
    if rad.collected {
        return;
    }
    rad.collected = true;
    rad.zones = level.actors.iter().filter_map(RadZone::from_actor).collect();
    rad.water = level.actors.iter().filter_map(|a| RadWater::from_actor(a, level.splines())).collect();
    info!("radiation zones: {}, radiated water: {}", rad.zones.len(), rad.water.len());
}

/// The water bodies' mesh (BP_WaterBox's Cube, BP_WaterSpline's segments): a 4 m cube.
const WATER_HALF: f32 = 2.0;
/// Radiated? water's UpdateRadiation time (not GetRadiationLevel's: no difficulty divisor).
pub const WATER_PERIOD: f32 = 0.15;

/// A radiated water body (BP_WaterBox / BP_WaterSpline with Radiated?).
#[derive(Clone, Debug)]
pub struct RadWater {
    pub name: String,
    pub shape: WaterShape,
}

#[derive(Clone, Debug)]
pub enum WaterShape {
    /// The Cube at the actor, world scale (Scale X, Y, Z): its world-to-local and half extents.
    Box(Transform, Vec3),
    /// The spline's mesh segments.
    Spline(Vec<crate::level::spline::SplineMesh>),
}

impl RadWater {
    pub fn from_actor(a: &crate::level::ActorInfo, splines: &[crate::level::spline::SplineMesh]) -> Option<Self> {
        let p = a.props.as_ref()?;
        if p.get("Radiated?").map(String::as_str) != Some("true") {
            return None;
        }
        let num = |k: &str| p.get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
        let at = a.position();
        let shape = match a.class.as_str() {
            "BP_WaterBox_C" => {
                // SetWorldScale3D(Scale X, Y, Z): UE X, Y, Z are Bevy x, z, y.
                let mut t = crate::level::affine(&a.transform);
                t.scale = Vec3::ONE;
                WaterShape::Box(t, Vec3::new(num("Scale X"), num("Scale Z"), num("Scale Y")) * WATER_HALF)
            }
            // Its spline mesh components sit at the actor.
            "BP_WaterSpline_C" => WaterShape::Spline(splines.iter().filter(|s| Vec3::new(s.transform[9], s.transform[10], s.transform[11]).distance(at) < 0.05).cloned().collect()),
            _ => return None,
        };
        Some(Self { name: a.name.clone(), shape })
    }

    /// Whether the survivor's capsule (feet at `feet`) overlaps it.
    pub fn overlaps(&self, feet: Vec3) -> bool {
        let centre = feet + Vec3::Y * 0.82;
        match &self.shape {
            WaterShape::Box(t, half) => {
                let l = t.rotation.inverse() * (centre - t.translation);
                l.x.abs() <= half.x + 0.35 && l.z.abs() <= half.z + 0.35 && l.y.abs() <= half.y + 0.82
            }
            // The feet or the head in the swept cube (its cross-section is metres thick).
            WaterShape::Spline(segs) => {
                let (lo, hi) = (Vec3::splat(-WATER_HALF), Vec3::splat(WATER_HALF));
                segs.iter().any(|s| s.contains(feet, lo, hi) || s.contains(feet + Vec3::Y * 1.64, lo, hi))
            }
        }
    }
}

fn severity(store: &SettingsStore) -> f32 {
    super::survival::difficulty(store, "General", "RADIATION SEVERITY MULTIPLIER (LEVEL 1)")
}

/// Begin / end overlap: the banners, the notification, UpdateRadiation / StopRadiation.
fn track(
    time: Res<Time>,
    store: Res<SettingsStore>,
    mut rad: ResMut<Radiation>,
    mut banner: ResMut<super::zones::PendingBanner>,
    mut notify: MessageWriter<Notify>,
    player: Query<&Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
) {
    let Ok(t) = player.single() else { return };
    // Radiated? water: its overlap events (BP_WaterBox / BP_WaterSpline): UpdateRadiation(0.15)
    // on entering, StopRadiation on leaving (whatever zone the survivor is also in).
    let water = rad.water.iter().position(|w| w.overlaps(t.translation));
    if water != rad.in_water {
        if let Some(i) = rad.in_water {
            info!("radiation: out of radiated water {}", rad.water[i].name);
            rad.timer = None;
            rad.filter_ui = false;
            rad.geiger = None;
        }
        if let Some(i) = water {
            info!("radiation: in radiated water {}", rad.water[i].name);
            rad.timer = Some((WATER_PERIOD, WATER_PERIOD));
        }
        rad.in_water = water;
    }
    rad.poll -= time.delta_secs();
    if rad.poll > 0.0 {
        return;
    }
    rad.poll = POLL;
    // The capsule's centre in the sphere.
    let p = t.translation + Vec3::Y * 0.82;
    let now: Vec<usize> = rad.zones.iter().enumerate().filter(|(_, z)| z.centre.distance(p) < z.radius).map(|(i, _)| i).collect();
    for &i in rad.inside.clone().iter().filter(|i| !now.contains(i)) {
        let z = &rad.zones[i];
        banner.0 = Some((false, z.name.clone(), Some(ICON.into())));
        info!("radiation: left {}", z.name);
        // StopRadiation: the Geiger fades out and is destroyed with the timer.
        rad.timer = None;
        rad.filter_ui = false;
        rad.geiger = None;
    }
    let entered: Vec<usize> = now.iter().copied().filter(|i| !rad.inside.contains(i)).collect();
    for i in entered {
        let z = rad.zones[i].clone();
        banner.0 = Some((true, z.name.clone(), Some(ICON.into())));
        if let Some(n) = &z.notification {
            notify.write(Notify { text: n.clone(), secs: 5.0, color: Color::WHITE });
        }
        let period = period(z.level, severity(&store));
        rad.timer = Some((period, period));
        info!("radiation: entered {} (level {}, every {period:.3} s)", z.name, z.level + 1);
    }
    rad.inside = now;
}

/// RadiationTimer.
fn irradiate(time: Res<Time>, db: Res<ItemDb>, store: Res<SettingsStore>, mut game: ResMut<CurrentGame>, mut rad: ResMut<Radiation>, mut notify: MessageWriter<Notify>) {
    let Some((period, left)) = rad.timer.as_mut() else { return };
    *left -= time.delta_secs();
    let mut ticks = 0;
    while *left <= 0.0 {
        *left += *period;
        ticks += 1;
    }
    if ticks == 0 || game.state.as_ref().is_none_or(|s| s.survival.health <= 0.0) {
        return;
    }
    let depletion = super::survival::difficulty(&store, "General", "RADIATION FILTER DEPLETION MULTIPLIER");
    for _ in 0..ticks {
        let out = tick(super::player_inv(&mut game), &db, depletion);
        let Some(state) = game.state.as_mut() else { return };
        state.survival.radiation = (state.survival.radiation + out.dose).clamp(0.0, 100.0);
        if out.toughness {
            state.passive.add_xp(crate::progression::passive::Passive::Toughness, rand::thread_rng().gen_range(0.1..=0.25));
        }
        rad.filter_ui = out.filter_ui;
        if let Some((text, secs)) = out.toast {
            notify.write(Notify { text: text.into(), secs, color: Color::WHITE });
        }
        // DecreaseRadiation's Geiger: started at 99 or less, for 6-8 s.
        if rad.geiger.is_none() && state.survival.radiation >= 1.0 && geiger_on(&store) {
            rad.geiger = Some((rand::thread_rng().gen_range(6.0..=8.0), 0.0));
        }
    }
}

fn geiger_on(store: &SettingsStore) -> bool {
    store.find(&row_key("settings", "Gameplay", "GEIGER SOUND WHEN TAKING RADIATION DAMAGE?")) != Some(Value::Bool(false))
}

/// The Geiger loop on the camera at (Max - Current) / 100, its 1 s FadeIn, gone when its time is up.
#[allow(clippy::type_complexity)]
fn geiger(
    mut commands: Commands,
    time: Res<Time>,
    game: Res<CurrentGame>,
    mut rad: ResMut<Radiation>,
    camera: Query<Entity, With<super::PlayerCamera>>,
    mut loops: Query<(Entity, &mut crate::audio::LoopGain), With<Geiger>>,
) {
    let level = game.state.as_ref().map_or(0.0, |s| s.survival.radiation);
    if level < rad.last_level {
        rad.geiger = None;
    }
    rad.last_level = level;
    let playing = match rad.geiger.as_mut() {
        Some((left, age)) => {
            *left -= time.delta_secs();
            *age += time.delta_secs();
            if *left <= 0.0 || level < 1.0 {
                rad.geiger = None;
                None
            } else {
                Some(*age)
            }
        }
        None => None,
    };
    match (playing, loops.single_mut()) {
        (Some(age), Ok((_, mut g))) => g.0 = level / 100.0 * age.min(1.0),
        (Some(_), Err(_)) => {
            if let Ok(cam) = camera.single() {
                commands.spawn((Transform::default(), crate::audio::AmbientLoop { cue: GEIGER.into(), volume: 1.0, active: true }, crate::audio::LoopGain(0.0), Geiger, ChildOf(cam)));
                info!("radiation: geiger on at {level:.1}");
            }
        }
        (None, Ok((e, _))) => commands.entity(e).despawn(),
        (None, Err(_)) => {}
    }
}

/// The showers: Event_Cooldown's timer, the lights, and the water started by a shower.
fn run_showers(time: Res<Time>, mut placed: ResMut<super::building::Buildables>, mut fx: MessageWriter<super::particles::Fx>) {
    let dt = time.delta_secs();
    for l in placed.list.iter_mut().filter(|l| l.actor == SHOWER) {
        let powered = l.power.powered;
        if l.shower.tick(dt, powered) {
            info!("radiation: shower lights {:?}", l.shower.lamp);
        }
        if std::mem::take(&mut l.shower.water) {
            // FX_Water / FX_Water2: Activate, a 3 s Delay, Deactivate.
            for (cm, pitch, yaw, roll) in SHOWER_WATER {
                fx.write(super::particles::Fx { system: WATER_FX.into(), at: crate::models::ue_relative(cm, pitch, yaw, roll), parent: Some(l.entity), stop_after: Some(WATER_FOR) });
            }
        }
    }
}

/// The shower's Light / Light2 colour (M_EmissiveColour_*).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lamp {
    /// The components' own material, and Off's.
    #[default]
    Red,
    Green,
    /// OnRep_CooldownRunning? (true).
    Orange,
}

impl Lamp {
    pub fn material(self) -> &'static str {
        match self {
            Lamp::Red => "M_EmissiveColour_Red",
            Lamp::Green => "M_EmissiveColour_Green",
            Lamp::Orange => "M_EmissiveColour_Orange",
        }
    }
}

/// The shower's state on a placed buildable (not saved: CooldownRunning? isn't a SaveGame field).
#[derive(Clone, Debug, Default)]
pub struct ShowerState {
    /// Cooldown seconds left.
    pub cooldown: f32,
    /// DecontaminatePlayer's DoOnce has fired.
    pub used: bool,
    pub lamp: Lamp,
    /// PoweredOn? last frame (the power scan's On / Off are edges here).
    powered: bool,
    /// A shower to start (`run_showers` spawns the water).
    pub water: bool,
}

impl ShowerState {
    /// One frame: the cooldown, and the lights on its end and on power coming or going (On: green
    /// unless cooling; Off: red; the cooldown's end: green, powered or not). Whether they changed.
    pub fn tick(&mut self, dt: f32, powered: bool) -> bool {
        let was = self.lamp;
        if self.cooldown > 0.0 {
            self.cooldown = (self.cooldown - dt).max(0.0);
            if self.cooldown == 0.0 {
                self.lamp = Lamp::Green;
            }
        }
        if powered != self.powered {
            self.powered = powered;
            if !powered {
                self.lamp = Lamp::Red;
            } else if self.cooldown == 0.0 {
                self.lamp = Lamp::Green;
            }
        }
        self.lamp != was
    }
}

/// A shower's Light / Light2 part (the placed buildable's id).
#[derive(Component)]
pub struct ShowerLamp(pub usize);

/// Keep the lights' meshes in their shower's colour (once the parts' scenes are in).
fn paint_shower_lamps(
    placed: Res<super::building::Buildables>,
    lamps: Query<(Entity, &ShowerLamp)>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
    assets: Res<AssetServer>,
    mut lib: ResMut<crate::models::GameMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, lamp) in &lamps {
        let Some(l) = placed.list.iter().find(|l| l.id == lamp.0) else { continue };
        let Some(want) = lib.get(l.shower.lamp.material(), &assets, &mut materials) else { continue };
        for c in children.iter_descendants(e) {
            if let Ok(mut m) = meshes.get_mut(c)
                && m.0 != want
            {
                m.0 = want.clone();
            }
        }
    }
}

/// The shower's Interact: the toast, or whether the survivor was cleaned.
pub fn shower(s: &mut ShowerState, powered: bool, under: bool) -> Result<bool, &'static str> {
    if !powered {
        return Err("No power");
    }
    if s.cooldown > 0.0 {
        return Err("Decontamination shower cooldown in effect...");
    }
    s.cooldown = SHOWER_COOLDOWN;
    s.lamp = Lamp::Orange;
    s.water = true;
    if under && !s.used {
        s.used = true;
        return Ok(true);
    }
    Ok(false)
}

/// DecontaminatePlayer's box (at Trace, the actor's rotation) holds the survivor's capsule centre.
pub fn under_shower(shower: &Transform, feet: Vec3) -> bool {
    let local = shower.rotation.inverse() * (feet + Vec3::Y * 0.82 - (shower.translation + Vec3::Y * SHOWER_TRACE));
    // The capsule (35 cm, 82 cm half height) touching the box.
    local.x.abs() <= SHOWER_BOX.x + 0.35 && local.z.abs() <= SHOWER_BOX.z + 0.35 && local.y.abs() <= SHOWER_BOX.y + 0.82
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(props: &[(&str, &str)]) -> crate::level::ActorInfo {
        crate::level::ActorInfo { class: "BP_RadiationArea_Sphere_C".into(), name: "z".into(), transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 0.0], props: Some(props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()) }
    }

    #[test]
    fn zones_read_their_instance_settings() {
        let z = RadZone::from_actor(&zone(&[
            ("ZoneRadius", "5200.0"),
            ("Name", r#"{"CultureInvariantString":"Checkpoint"}"#),
            ("AdditionalNotification?", "true"),
            ("Notification Details Message", r#"{"CultureInvariantString":"Increased Radiation [Level 1]"}"#),
            ("RadiationLevel", r#""Enum_RadiationLevel::NewEnumerator2""#),
        ]))
        .unwrap();
        assert_eq!((z.radius, z.level, z.name.as_str()), (52.0, 2, "Checkpoint"));
        assert_eq!(z.notification.as_deref(), Some("Increased Radiation [Level 1]"));
        let d = RadZone::from_actor(&zone(&[])).unwrap();
        assert_eq!((d.radius, d.level, d.notification.clone()), (100.0, 0, None));
        assert_eq!(d.weight(Vec3::new(50.0, 0.0, 0.0)), 1.0);
        assert!((d.weight(Vec3::new(111.25, 0.0, 0.0)) - 0.5).abs() < 1e-3);
        assert_eq!(d.weight(Vec3::new(120.0, 0.0, 0.0)), 0.0);
        assert!((period(0, 1.0) - 0.2).abs() < 1e-6 && (period(1, 2.0) - 0.04).abs() < 1e-6 && (period(2, 1.0) - 0.04).abs() < 1e-6);
        // The level's ten.
        let Ok(a) = std::fs::read_to_string(format!("{}/actors.json", crate::level::LEVEL_DIR)) else { return };
        let actors: Vec<crate::level::ActorInfo> = serde_json::from_str(&a).unwrap();
        let zones: Vec<_> = actors.iter().filter_map(RadZone::from_actor).collect();
        assert_eq!(zones.len(), 10);
        let rig = zones.iter().find(|z| z.name == "The Oil Rig").expect("oil rig");
        assert_eq!((rig.radius, rig.level), (67.0, 2));
        assert!(zones.iter().all(|z| z.notification.is_some()));
    }

    #[test]
    fn respirators_filter_and_wear_out() {
        let db = ItemDb::load();
        if db.items.is_empty() {
            return;
        }
        let mut inv = PlayerInventory::default();
        // Bare-faced: the full dose and Toughness XP.
        assert_eq!(tick(&mut inv, &db, 1.0), Tick { dose: REDUCE, toast: None, filter_ui: false, toughness: true });
        let mut mask = db.stack("DA_HazmatRespirator", 1);
        mask.stats.insert("RadiationProtection".into(), 96.0);
        mask.stats.insert("FilterUsagePerSecond".into(), 0.01);
        inv.equipped.insert(Slot::Facewear, mask);
        assert_eq!(tick(&mut inv, &db, 1.0).toast, Some(("No Filter", 4.0)));
        let mut filter = db.stack("DA_RadiationFilter", 1);
        filter.durability = Some(0.015);
        inv.equipped.get_mut(&Slot::Facewear).unwrap().attachments.push(filter);
        let t = tick(&mut inv, &db, 1.0);
        assert!((t.dose - REDUCE / 2.0 * 0.04).abs() < 1e-6, "{t:?}");
        assert!((filter_left(&inv, &db).unwrap() - 0.005).abs() < 1e-6);
        // Worn out: the message and the full dose.
        let t = tick(&mut inv, &db, 1.0);
        assert_eq!((t.dose, t.toast.map(|t| t.1)), (REDUCE, Some(8.0)));
        // Hits: only a filtered mask, and only when the caller lets it.
        let mut fresh = PlayerInventory::default();
        let mut m = db.stack("DA_HazmatRespirator", 1);
        m.stats.insert("RadiationProtection".into(), 96.0);
        m.attachments.push(db.stack("DA_RadiationFilter", 1));
        fresh.equipped.insert(Slot::Facewear, m);
        assert!((hit_dose(Some(&fresh), &db, 16.0, true) - 0.64).abs() < 1e-4);
        assert_eq!(hit_dose(Some(&fresh), &db, 16.0, false), 16.0);
        assert_eq!(hit_dose(None, &db, 16.0, true), 16.0);
    }

    #[test]
    fn the_two_radiated_water_bodies() {
        let dir = "assets/game/levels/LongdownValley";
        let (Ok(a), Ok(sp)) = (std::fs::read_to_string(format!("{dir}/actors.json")), std::fs::read_to_string(format!("{dir}/splines.json"))) else { return };
        let actors: Vec<crate::level::ActorInfo> = serde_json::from_str(&a).unwrap();
        let splines: Vec<crate::level::spline::SplineMesh> = serde_json::from_str(&sp).unwrap();
        let water: Vec<RadWater> = actors.iter().filter_map(|a| RadWater::from_actor(a, &splines)).collect();
        assert_eq!(water.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), ["BP_WaterBox_C_0", "BP_WaterSpline_C_1"]);
        // The box: 28 x 32 m (Scale 7 x 8 of the 4 m cube), 4 m deep, turned 25° at (25.6, 6.6, 273.5).
        let WaterShape::Box(t, half) = &water[0].shape else { panic!() };
        assert_eq!(*half, Vec3::new(14.0, 2.0, 16.0));
        let feet = |l: Vec3| t.translation + t.rotation * l - Vec3::Y * 0.82;
        assert!(water[0].overlaps(feet(Vec3::new(13.0, 0.0, -15.0))));
        assert!(!water[0].overlaps(feet(Vec3::new(15.0, 0.0, 0.0))));
        assert!(!water[0].overlaps(feet(Vec3::new(0.0, 3.5, 0.0))), "above the surface");
        // The river: 10 segments, 80 m wide, from (3061.5, 3, -707.2).
        let WaterShape::Spline(segs) = &water[1].shape else { panic!() };
        assert_eq!(segs.len(), 10);
        let start = Vec3::new(3061.49, 3.0, -707.17);
        assert!(water[1].overlaps(start - Vec3::Y));
        assert!(!water[1].overlaps(start + Vec3::Y * 10.0));
    }

    #[test]
    fn the_shower_lights_follow_power_and_the_cooldown() {
        let mut s = ShowerState::default();
        assert_eq!(s.lamp, Lamp::Red);
        // Power: green; a shower: orange and the water; power lost while cooling: red.
        assert!(s.tick(0.1, true) && s.lamp == Lamp::Green);
        assert_eq!(shower(&mut s, true, false), Ok(false));
        assert_eq!((s.lamp, std::mem::take(&mut s.water)), (Lamp::Orange, true));
        assert!(!s.tick(100.0, true) && s.lamp == Lamp::Orange, "On while cooling leaves it");
        assert!(s.tick(1.0, false) && s.lamp == Lamp::Red);
        // The cooldown's end turns it green even without power (OnRep_CooldownRunning?).
        assert!(s.tick(SHOWER_COOLDOWN, false) && s.lamp == Lamp::Green);
        assert!(!s.tick(0.1, true) && s.lamp == Lamp::Green, "power back: already green");
    }

    #[test]
    fn the_shower_cleans_once_then_cools_down() {
        let mut s = ShowerState::default();
        assert_eq!(shower(&mut s, false, true), Err("No power"));
        assert_eq!(shower(&mut s, true, true), Ok(true));
        assert_eq!(shower(&mut s, true, true), Err("Decontamination shower cooldown in effect..."));
        s.cooldown = 0.0;
        // DecontaminatePlayer's DoOnce is never reset.
        assert_eq!(shower(&mut s, true, true), Ok(false));
        let t = Transform::from_xyz(5.0, 0.0, 5.0).with_rotation(Quat::from_rotation_y(1.0));
        assert!(under_shower(&t, Vec3::new(5.0, 0.0, 5.0)));
        assert!(!under_shower(&t, Vec3::new(7.0, 0.0, 5.0)));
    }
}
