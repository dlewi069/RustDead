//! Laboratory power, decompiled from `BP_LaboratoryGenerator`, `BP_LaboratoryLightSwitch` and
//! `BP_LaboratoryLight(2/3)`:
//!
//! - A generator (F: "Turn On" / "Turn Off", "Laboratory generator powered on/off...") calls
//!   `SetPower_TurnedOn(true)` on every actor in its `Actors` list when it starts and
//!   `SetPower_Warning(true)` when it stops. Its switches go On / Warning, its sliding doors
//!   Unlocked / Warning (`doors::Doors::set_power`).
//! - A switch is On, Off or Warning (each clears the others) and lights its `Lights` only while
//!   On. F on a switch that isn't in Warning flips it and every switch in its `Switches` list:
//!   On -> Off, otherwise -> On (Event_LightSwitch).
//! - Placed switches start in Warning (lab without power, lights out); a lab light's intensity
//!   is its `Intensity` while lit.
//!
//! `BP_PowerGenerator` (the police station's generator) has two options: "Add Gasoline" uses up
//! a Gas Can once ("Generator Refueled" / "Gas Already Added" / "No Gas Can In Inventory"), and
//! "Turn On" / "Turn Off" (Event_TurnOn): no gas -> "No Gas In Generator"; the first start needs
//! its `RequiredItem` (the Police Generator Key, used up: `ConsumeItemOnUse?`), else "{item} Not
//! Found"; after that it toggles freely (`ItemUsed?`). On/off calls `SetPower_TurnedOn` on its
//! `Actors` (the lever, `doors::Kind::Lever`, which works only while powered).
//!
//! Status lamps: a switch's OnRep_On?/Off?/Warning? and a lab sliding door's
//! OnRep_Locked/Unlocked/Warning set material slot 1 of its mesh (LabLightswitch,
//! LabSmallDoorframe) to M_LaboratoryLightsStatus_* / M_LaboratoryDoorStatus_*. Ours: the streamed
//! prop's slot-1 part carries a [`StatusLamp`], matched to the nearest switch or sliding door.
//!
//! BP_Lever's SetPower_TurnedOn / SetPower_Warning do the same on its Base (LeverBase): grey
//! and green with power, red and grey without.
//!
//! Not ported: the generators' shake and hum, the stairwell switches (a trigger
//! box).

use std::collections::HashMap;

use bevy::prelude::*;

use super::ActorInfo;

const REACH: f32 = 2.0;

/// Which Blueprint recolours a status lamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LampKind {
    /// BP_LaboratoryLightSwitch (LabLightswitch slot 1).
    Switch,
    /// BP_LaboratorySlidingDoor (its Frame, LabSmallDoorframe slot 1).
    Door,
    /// BP_Lever's Base (LeverBase): slot 1 the red "no power" lamp, slot 2 the green one.
    LeverRed,
    LeverGreen,
}


impl LampKind {
    /// The lamp kind of a mesh's part, by its glTF material name (the meshes' defaults).
    pub fn of(mesh: &str, material: &str) -> Option<Self> {
        if mesh.ends_with("/LeverBase.glb") {
            match material {
                "M_EmissiveColour_Red" => Some(Self::LeverRed),
                "M_EmissiveColour_Green" => Some(Self::LeverGreen),
                _ => None,
            }
        } else if material.starts_with("M_LaboratoryLightsStatus") {
            Some(Self::Switch)
        } else if material == "M_LaboratoryStatus" || material.starts_with("M_LaboratoryDoorStatus") {
            Some(Self::Door)
        } else {
            None
        }
    }
}

/// What a status lamp shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lamp {
    On,
    Off,
    Warning,
    Locked,
    Unlocked,
}

impl Lamp {
    pub fn of_switch(state: SwitchState) -> Self {
        match state {
            SwitchState::On => Lamp::On,
            SwitchState::Off => Lamp::Off,
            SwitchState::Warning => Lamp::Warning,
        }
    }

    /// The material the Blueprint sets.
    pub fn material(self, kind: LampKind) -> &'static str {
        match (kind, self) {
            (LampKind::Switch, Lamp::On) => "M_LaboratoryLightsStatus_On",
            (LampKind::Switch, Lamp::Warning) => "M_LaboratoryLightsStatus_Warning",
            (LampKind::Switch, _) => "M_LaboratoryLightsStatus_Off",
            (LampKind::Door, Lamp::Warning) => "M_LaboratoryDoorStatus_Warning",
            (LampKind::Door, Lamp::Locked) => "M_LaboratoryDoorStatus_Locked",
            (LampKind::Door, _) => "M_LaboratoryDoorStatus_Unlocked",
            // SetPower_TurnedOn: slot 1 grey, slot 2 green; SetPower_Warning (and the class's
            // OverrideMaterials before any power): slot 1 red, slot 2 grey.
            (LampKind::LeverRed, Lamp::On) => "M_BlankColour_Grey",
            (LampKind::LeverRed, _) => "M_EmissiveColour_Red",
            (LampKind::LeverGreen, Lamp::On) => "M_EmissiveColour_Green",
            (LampKind::LeverGreen, _) => "M_BlankColour_Grey",
        }
    }
}

/// A streamed prop's status-lamp part.
#[derive(Component, Debug)]
pub struct StatusLamp {
    pub kind: LampKind,
    at: Vec3,
    /// The switch or door index it belongs to, once looked up (None inside: no owner nearby).
    owner: Option<Option<usize>>,
    pub shown: Option<Lamp>,
}

impl StatusLamp {
    pub fn new(kind: LampKind, at: Vec3) -> Self {
        Self { kind, at, owner: None, shown: None }
    }
}

/// How far a lamp's mesh may sit from its actor's origin (both are the actor's transform; guess).
const LAMP_MATCH: f32 = 1.0;

/// Keep every streamed status lamp on its owner's state.
pub(super) fn status_lamps(
    level: Option<Res<super::Level>>,
    power: Option<Res<Power>>,
    doors: Res<super::doors::Doors>,
    assets: Res<AssetServer>,
    mut lib: ResMut<crate::models::GameMaterials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lamps: Query<(&mut StatusLamp, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let (Some(level), Some(power)) = (level, power) else { return };
    for (mut lamp, mut mat) in &mut lamps {
        let (at, kind) = (lamp.at, lamp.kind);
        let owner = *lamp.owner.get_or_insert_with(|| match kind {
            LampKind::Switch => nearest(power.switches.iter().map(|s| s.pos), at),
            LampKind::Door => nearest(
                level.doors.iter().map(|d| if matches!(d.kind, super::doors::Kind::Sliding { .. }) { d.pos } else { Vec3::INFINITY }),
                at,
            ),
            LampKind::LeverRed | LampKind::LeverGreen => nearest(
                level.doors.iter().map(|d| if matches!(d.kind, super::doors::Kind::Lever { .. }) { d.pos } else { Vec3::INFINITY }),
                at,
            ),
        });
        let Some(i) = owner else { continue };
        let want = match lamp.kind {
            LampKind::Switch => Lamp::of_switch(power.switches[i].state),
            LampKind::Door => doors.lamp(i),
            // The lever is "locked" while it has no power (Doors::set_lever_power).
            LampKind::LeverRed | LampKind::LeverGreen => if doors.locked(i) { Lamp::Off } else { Lamp::On },
        };
        if lamp.shown != Some(want) {
            if let Some(h) = lib.get(want.material(lamp.kind), &assets, &mut materials) {
                mat.0 = h;
            }
            lamp.shown = Some(want);
        }
    }
}

fn nearest(points: impl Iterator<Item = Vec3>, at: Vec3) -> Option<usize> {
    points.enumerate().map(|(i, p)| (i, p.distance(at))).filter(|c| c.1 < LAMP_MATCH).min_by(|a, b| a.1.total_cmp(&b.1)).map(|c| c.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchState {
    On,
    Off,
    Warning,
}

#[derive(Clone, Debug)]
pub struct Switch {
    pub pos: Vec3,
    pub state: SwitchState,
    /// Light actor names.
    pub lights: Vec<String>,
    /// Switches flipped with it (indices).
    pub linked: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Generator {
    pub pos: Vec3,
    pub on: bool,
    pub switches: Vec<usize>,
    /// Actor indices of its sliding doors.
    pub doors: Vec<usize>,
}

/// A `BP_PowerGenerator`.
#[derive(Clone, Debug, PartialEq)]
pub struct PowerGenerator {
    pub pos: Vec3,
    pub on: bool,
    /// GasAdded?
    pub gas: bool,
    /// ItemUsed?: the required item was handed over once.
    pub item_used: bool,
    /// RequiredItem when RequiredItemToPowerOn?, and whether it's used up.
    pub required: Option<String>,
    pub consume: bool,
    /// DisplayItemName ("Police Generator Key").
    pub display: String,
    /// Actor indices it powers (its lever).
    pub actors: Vec<usize>,
}

/// What Event_TurnOn did.
#[derive(Clone, Debug, PartialEq)]
pub enum TurnOn {
    NoGas,
    /// "{item} Not Found".
    Missing(String),
    /// Powered on or off: the actors to switch, and the item to take from the survivor.
    Switched { on: bool, actors: Vec<usize>, consume: Option<String> },
}

impl TurnOn {
    pub fn message(&self) -> String {
        match self {
            TurnOn::NoGas => "No Gas In Generator".into(),
            TurnOn::Missing(item) => format!("{item} Not Found"),
            TurnOn::Switched { on: true, .. } => "Generator powered on...".into(),
            TurnOn::Switched { on: false, .. } => "Generator powered off...".into(),
        }
    }
}

impl PowerGenerator {
    /// Event_TurnOn. `has` tells whether the survivor carries an item.
    pub fn turn_on(&mut self, has: impl Fn(&str) -> bool) -> TurnOn {
        if !self.gas {
            return TurnOn::NoGas;
        }
        if self.on {
            self.on = false;
            return TurnOn::Switched { on: false, actors: self.actors.clone(), consume: None };
        }
        let mut consume = None;
        if !self.item_used
            && let Some(item) = &self.required
        {
            if !has(item) {
                return TurnOn::Missing(self.display.clone());
            }
            if self.consume {
                consume = Some(item.clone());
            }
        }
        self.item_used = true;
        self.on = true;
        TurnOn::Switched { on: true, actors: self.actors.clone(), consume }
    }

    /// Event_AddGas: whether a Gas Can is used up, and the message.
    pub fn add_gas(&mut self, has_gas_can: bool) -> (bool, &'static str) {
        if self.gas {
            (false, "Gas Already Added")
        } else if has_gas_can {
            self.gas = true;
            (true, "Generator Refueled")
        } else {
            (false, "No Gas Can In Inventory")
        }
    }
}

/// The lab power network, and the police generator.
#[derive(Resource, Default, Debug)]
pub struct Power {
    pub generators: Vec<Generator>,
    pub switches: Vec<Switch>,
    pub power_generators: Vec<PowerGenerator>,
    /// Lab light actor -> lit.
    lit: HashMap<String, bool>,
}

/// What F is pointed at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Generator(usize),
    Switch(usize),
    PowerGenerator(usize),
}

/// A generator flipped: the door actors to power or cut.
pub struct Flipped {
    pub on: bool,
    pub doors: Vec<usize>,
}

/// Actor names in an exported array of object refs (`...PersistentLevel.BP_X_C_12'`).
pub(super) fn refs(json: &str) -> Vec<String> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    list.iter()
        .filter_map(|v| v.get("ObjectName")?.as_str()?.rsplit('.').next().map(|n| n.trim_end_matches('\'').to_string()))
        .collect()
}

impl Power {
    pub fn from_actors(actors: &[ActorInfo]) -> Self {
        let prop = |a: &ActorInfo, k: &str| a.props.as_ref().and_then(|m| m.get(k)).cloned();
        let flag = |a: &ActorInfo, k: &str, default: bool| prop(a, k).map_or(default, |v| v == "true");
        let switch_ix: HashMap<&str, usize> =
            actors.iter().filter(|a| a.class == "BP_LaboratoryLightSwitch_C").enumerate().map(|(i, a)| (a.name.as_str(), i)).collect();
        let by_name: HashMap<&str, usize> = actors.iter().enumerate().map(|(i, a)| (a.name.as_str(), i)).collect();
        let mut power = Power::default();
        for a in actors.iter().filter(|a| a.class == "BP_LaboratoryLightSwitch_C") {
            // Class defaults: On? true, Off?/Warning? false; placed ones override.
            let state = if flag(a, "Warning?", false) {
                SwitchState::Warning
            } else if flag(a, "On?", true) {
                SwitchState::On
            } else {
                SwitchState::Off
            };
            let lights = prop(a, "Lights").map(|j| refs(&j)).unwrap_or_default();
            let linked = prop(a, "Switches").map(|j| refs(&j)).unwrap_or_default().iter().filter_map(|n| switch_ix.get(n.as_str()).copied()).collect();
            power.switches.push(Switch { pos: a.position(), state, lights, linked });
        }
        for a in actors.iter().filter(|a| a.class == "BP_LaboratoryGenerator_C") {
            let list = prop(a, "Actors").map(|j| refs(&j)).unwrap_or_default();
            power.generators.push(Generator {
                pos: a.position(),
                on: flag(a, "TurnedOn?", false),
                switches: list.iter().filter_map(|n| switch_ix.get(n.as_str()).copied()).collect(),
                doors: list.iter().filter(|n| n.starts_with("BP_LaboratorySlidingDoor_C")).filter_map(|n| by_name.get(n.as_str()).copied()).collect(),
            });
        }
        for a in actors.iter().filter(|a| a.class == "BP_PowerGenerator_C") {
            let display = prop(a, "DisplayItemName")
                .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
                .and_then(|v| v.get("SourceString").and_then(|s| s.as_str()).map(str::to_string))
                // The class default.
                .unwrap_or_else(|| "AllAccessKeycard".into());
            power.power_generators.push(PowerGenerator {
                pos: a.position(),
                on: flag(a, "TurnedOn?", false),
                gas: flag(a, "GasAdded?", false),
                item_used: flag(a, "ItemUsed?", false),
                required: flag(a, "RequiredItemToPowerOn?", false).then(|| prop(a, "RequiredItem")).flatten().map(|v| v.trim_matches('"').to_string()),
                consume: flag(a, "ConsumeItemOnUse?", false),
                display,
                actors: prop(a, "Actors").map(|j| refs(&j)).unwrap_or_default().iter().filter_map(|n| by_name.get(n.as_str()).copied()).collect(),
            });
        }
        power.relight();
        power
    }

    fn relight(&mut self) {
        self.lit.clear();
        for s in &self.switches {
            for l in &s.lights {
                let on = s.state == SwitchState::On;
                *self.lit.entry(l.clone()).or_insert(false) |= on;
            }
        }
    }

    /// Is this lab light actor lit?
    pub fn lit(&self, light: &str) -> bool {
        self.lit.get(light).copied().unwrap_or(false)
    }

    /// Event_LightSwitch (nothing in Warning). Returns whether anything changed.
    pub fn press_switch(&mut self, i: usize) -> bool {
        let Some(s) = self.switches.get(i) else { return false };
        let to = match s.state {
            SwitchState::Warning => return false,
            SwitchState::On => SwitchState::Off,
            SwitchState::Off => SwitchState::On,
        };
        let mut all = s.linked.clone();
        all.push(i);
        for k in all {
            if let Some(s) = self.switches.get_mut(k) {
                s.state = to;
            }
        }
        self.relight();
        true
    }

    /// Event_TurnOn: toggle generator `i`.
    pub fn toggle_generator(&mut self, i: usize) -> Option<Flipped> {
        let g = self.generators.get_mut(i)?;
        g.on = !g.on;
        let (on, switches, doors) = (g.on, g.switches.clone(), g.doors.clone());
        for k in switches {
            if let Some(s) = self.switches.get_mut(k) {
                s.state = if on { SwitchState::On } else { SwitchState::Warning };
            }
        }
        self.relight();
        Some(Flipped { on, doors })
    }

    /// The generator or switch within reach of `p`, nearest first, with its distance.
    pub fn target(&self, p: Vec3) -> Option<(Target, f32)> {
        let near = |q: Vec3| ((q.y - p.y).abs() < 3.0 && q.xz().distance(p.xz()) < REACH).then(|| q.distance(p));
        let g = self.generators.iter().enumerate().filter_map(|(i, g)| near(g.pos).map(|d| (Target::Generator(i), d)));
        let s = self.switches.iter().enumerate().filter_map(|(i, s)| near(s.pos).map(|d| (Target::Switch(i), d)));
        // The police generator is a large prop: reached from a little farther.
        let pg = self.power_generators.iter().enumerate().filter(|(_, g)| (g.pos.y - p.y).abs() < 3.0 && g.pos.xz().distance(p.xz()) < REACH + 0.6).map(|(i, g)| (Target::PowerGenerator(i), g.pos.distance(p)));
        g.chain(s).chain(pg).min_by(|a, b| a.1.total_cmp(&b.1))
    }
}

pub(super) fn setup_power(mut commands: Commands, level: Option<Res<super::Level>>) {
    commands.insert_resource(level.as_ref().map(|l| Power::from_actors(&l.actors)).unwrap_or_default());
    // The actors' looping AudioComponents.
    let Some(level) = level else { return };
    let mut police = 0;
    for a in &level.actors {
        let Some((cue, volume, active)) = ambient_sound(&a.class) else { continue };
        let mut e = commands.spawn((Transform::from_translation(a.position() + Vec3::Y), crate::audio::AmbientLoop { cue: cue.into(), volume, active }, DespawnOnExit(crate::menus::AppState::InGame)));
        if a.class == "BP_PowerGenerator_C" {
            e.insert(GeneratorHum(police));
            police += 1;
        }
    }
}

/// An actor class's looping AudioComponent: (cue, VolumeMultiplier, playing from the start).
/// BP_BarrelFire / BP_Campfire: SC_CampFire at 0.5; BP_Generator (camps): its engine loop at
/// 0.25; BP_PowerGenerator: the engine loop at 0.5 once turned on (bAutoActivate off).
pub fn ambient_sound(class: &str) -> Option<(&'static str, f32, bool)> {
    match class {
        "BP_BarrelFire_C" | "BP_Campfire_C" | "BP_EasterEgg_Campfire_C" => Some(("SC_CampFire", 0.5, true)),
        "BP_Generator_C" => Some(("engine_generator_loop_01_Cue", 0.25, true)),
        "BP_PowerGenerator_C" => Some(("engine_generator_loop_01_Cue", 0.5, false)),
        _ => None,
    }
}

/// The police generator's hum (index into `Power::power_generators`).
#[derive(Component)]
pub(super) struct GeneratorHum(usize);

/// The hum follows the generator's Turn On / Turn Off.
pub(super) fn generator_hum(power: Option<Res<Power>>, mut hums: Query<(&GeneratorHum, &mut crate::audio::AmbientLoop)>) {
    let Some(power) = power else { return };
    for (g, mut l) in &mut hums {
        let on = power.power_generators.get(g.0).is_some_and(|p| p.on);
        if l.active != on {
            l.active = on;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_and_generators_sound() {
        assert_eq!(ambient_sound("BP_BarrelFire_C"), Some(("SC_CampFire", 0.5, true)));
        assert_eq!(ambient_sound("BP_PowerGenerator_C"), Some(("engine_generator_loop_01_Cue", 0.5, false)), "off until started");
        assert_eq!(ambient_sound("BP_LockedDoor_C"), None);
    }

    fn net() -> Power {
        let sw = |state, lights: &[&str], linked: Vec<usize>| Switch { pos: Vec3::ZERO, state, lights: lights.iter().map(|s| s.to_string()).collect(), linked };
        let mut p = Power {
            generators: vec![Generator { pos: Vec3::ZERO, on: false, switches: vec![0, 1], doors: vec![7] }],
            switches: vec![sw(SwitchState::Warning, &["L1", "L2"], vec![1]), sw(SwitchState::Warning, &["L3"], vec![0]), sw(SwitchState::Off, &["L4"], vec![])],
            lit: HashMap::new(),
            power_generators: Vec::new(),
        };
        p.relight();
        p
    }

    fn police() -> PowerGenerator {
        PowerGenerator { pos: Vec3::ZERO, on: false, gas: false, item_used: false, required: Some("DA_PoliceGeneratorKey".into()), consume: true, display: "Police Generator Key".into(), actors: vec![4] }
    }

    #[test]
    fn police_generator_needs_gas_then_the_key_once() {
        let mut g = police();
        assert_eq!(g.turn_on(|_| true), TurnOn::NoGas);
        assert_eq!(g.add_gas(false), (false, "No Gas Can In Inventory"));
        assert_eq!(g.add_gas(true), (true, "Generator Refueled"));
        assert_eq!(g.add_gas(true), (false, "Gas Already Added"));
        let no_key = g.turn_on(|_| false);
        assert_eq!(no_key.message(), "Police Generator Key Not Found");
        assert!(!g.on);
        let on = g.turn_on(|i| i == "DA_PoliceGeneratorKey");
        assert_eq!(on, TurnOn::Switched { on: true, actors: vec![4], consume: Some("DA_PoliceGeneratorKey".into()) });
        assert_eq!(on.message(), "Generator powered on...");
        // Off and on again: the key was used up already.
        assert_eq!(g.turn_on(|_| false), TurnOn::Switched { on: false, actors: vec![4], consume: None });
        assert_eq!(g.turn_on(|_| false), TurnOn::Switched { on: true, actors: vec![4], consume: None });
    }

    #[test]
    fn generators_power_switches_and_switches_light_lamps() {
        let mut p = net();
        assert!(!p.lit("L1") && !p.lit("L4"));
        // No power: the switch does nothing.
        assert!(!p.press_switch(0));
        let f = p.toggle_generator(0).unwrap();
        assert!(f.on && f.doors == [7]);
        assert!(p.lit("L1") && p.lit("L2") && p.lit("L3"));
        // A switch turns its linked switches off with it.
        assert!(p.press_switch(0));
        assert!(!p.lit("L1") && !p.lit("L3"));
        assert!(p.press_switch(1));
        assert!(p.lit("L1") && p.lit("L3"));
        // Generator off: Warning, lamps out, the switch dead again.
        assert!(!p.toggle_generator(0).unwrap().on);
        assert!(!p.lit("L1") && !p.press_switch(0));
        // An unpowered-network switch in Off just toggles.
        assert!(p.press_switch(2) && p.lit("L4"));
    }

    #[test]
    fn level_lab_network_resolves() {
        let dir = crate::level::LEVEL_DIR;
        let Ok(a) = std::fs::read_to_string(format!("{dir}/actors.json")) else { return };
        let actors: Vec<ActorInfo> = serde_json::from_str(&a).unwrap();
        let p = Power::from_actors(&actors);
        assert_eq!(p.generators.len(), 2);
        assert_eq!(p.generators.iter().map(|g| g.switches.len()).sum::<usize>(), 46);
        assert_eq!(p.generators.iter().map(|g| g.doors.len()).sum::<usize>(), 9);
        assert_eq!(p.switches.len(), 55);
        assert!(p.switches.iter().filter(|s| s.state == SwitchState::Warning).count() == 49);
        // Lights resolve to lab light actors, and are all dark without power.
        assert!(p.switches.iter().flat_map(|s| &s.lights).all(|l| l.starts_with("BP_LaboratoryLight")));
        assert!(!p.lit.values().any(|&on| on));
        // The police generator: gas and the key, powering the lever.
        assert_eq!(p.power_generators.len(), 1);
        let g = &p.power_generators[0];
        assert_eq!((g.required.as_deref(), g.consume, g.display.as_str(), g.gas), (Some("DA_PoliceGeneratorKey"), true, "Police Generator Key", false));
        assert_eq!(g.actors.iter().map(|&i| actors[i].class.as_str()).collect::<Vec<_>>(), ["BP_Lever_C"]);
    }

    #[test]
    fn status_lamps_follow_the_switches() {
        assert_eq!(LampKind::of("x/LabLightswitch.glb", "M_LaboratoryLightsStatus_On"), Some(LampKind::Switch));
        assert_eq!(LampKind::of("x/LabSmallDoorframe.glb", "M_LaboratoryStatus"), Some(LampKind::Door));
        assert_eq!(LampKind::of("x/LabLightswitch.glb", "M_Laboratory"), None);
        assert_eq!(LampKind::of("game/x/LeverBase.glb", "M_EmissiveColour_Red"), Some(LampKind::LeverRed));
        assert_eq!(LampKind::of("game/x/Other.glb", "M_EmissiveColour_Red"), None, "only the lever's base");
        // The lever: red without power, green with it.
        assert_eq!(Lamp::Off.material(LampKind::LeverRed), "M_EmissiveColour_Red");
        assert_eq!(Lamp::Off.material(LampKind::LeverGreen), "M_BlankColour_Grey");
        assert_eq!(Lamp::On.material(LampKind::LeverRed), "M_BlankColour_Grey");
        assert_eq!(Lamp::On.material(LampKind::LeverGreen), "M_EmissiveColour_Green");
        let mut p = net();
        let lamp = |p: &Power, i: usize| Lamp::of_switch(p.switches[i].state).material(LampKind::Switch);
        assert_eq!(lamp(&p, 0), "M_LaboratoryLightsStatus_Warning", "placed switches wait for power");
        p.toggle_generator(0);
        assert_eq!(lamp(&p, 0), "M_LaboratoryLightsStatus_On");
        p.press_switch(0);
        assert_eq!(lamp(&p, 0), "M_LaboratoryLightsStatus_Off");
        assert_eq!(Lamp::Locked.material(LampKind::Door), "M_LaboratoryDoorStatus_Locked");
        assert_eq!(Lamp::Unlocked.material(LampKind::Door), "M_LaboratoryDoorStatus_Unlocked");
        assert_eq!(Lamp::Warning.material(LampKind::Door), "M_LaboratoryDoorStatus_Warning");
    }
}
