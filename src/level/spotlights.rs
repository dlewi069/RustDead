//! The level's spot lights, decompiled from `BP_RoadLightGenerator` (7), `BP_FloodLights` (4)
//! and `BP_Lights` (2, a tall pole):
//!
//! - BP_RoadLightGenerator: SpotLight1 (Intensity 2.5, cone 25° / 50°, AttenuationRadius 2500,
//!   pitched -35°, 354 cm up) is always on; only its AIOSubject hides it beyond 125 m. Its
//!   LightHum_Cue component plays engine_generator_loop_01_Cue all the time (`power::ambient_sound`).
//! - BP_FloodLights / BP_Lights: BeginPlay waits 0.6 s / 1 s, then (UDS valid) 2 s, binds
//!   TurnOn to UDS's Sunset and TurnOff to its Sunrise, and turns on if Time of Day >= 1799 or
//!   <= 599. TurnOn plays Timeline_TurnOn (4 s; Intensity a cubic 0 -> 10 over 4 s / 0 -> 5 over
//!   6 s) and activates LightHum_Cue (AttenuationSettings CampFire). TurnOff *reverses*
//!   Timeline_TurnOff, which never played forward: one update at time 0, intensity 0 and the hum
//!   off at once. Timeline_TurnOn is never rewound either, so only the first sunset fades in; later
//!   ones jump to the end value. Floods: cone 40° / 45°, radius 2000, pitched -8.5°; the pole's:
//!   50° / 50°, radius 1750, straight down from 730 cm. Both hidden beyond 150 m.
//!
//! Ours: the lights' Sunset / Sunrise are the BeginPlay check's hours (`lights::night_on`).
//! Their intensities use exponent falloff (no candela equivalent): [`exponent_candela`] matches
//! UE's falloff (exponent 8) at a quarter of the radius. No shadows (UE casts them within 25 m).

use bevy::prelude::*;

use super::ActorInfo;
use crate::menus::{AppState, CurrentGame};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotKind {
    RoadLight,
    Flood,
    Pole,
}

/// A class's spot light: (relative location in UE cm, pitch°, inner°, outer°, radius m,
/// shown within m).
struct Spec {
    rel: Vec3,
    pitch: f32,
    inner: f32,
    outer: f32,
    radius: f32,
    shown: f32,
}

impl SpotKind {
    pub fn of(class: &str) -> Option<Self> {
        match class {
            "BP_RoadLightGenerator_C" => Some(Self::RoadLight),
            "BP_FloodLights_C" => Some(Self::Flood),
            "BP_Lights_C" => Some(Self::Pole),
            _ => None,
        }
    }

    fn spec(self) -> Spec {
        match self {
            Self::RoadLight => Spec { rel: Vec3::new(34.37, 0.0, 353.92), pitch: -35.0, inner: 25.0, outer: 50.0, radius: 25.0, shown: 125.0 },
            Self::Flood => Spec { rel: Vec3::new(5.03, 0.0, 13.59), pitch: -8.55, inner: 40.0, outer: 45.0, radius: 20.0, shown: 150.0 },
            Self::Pole => Spec { rel: Vec3::new(0.0, 0.0, 730.29), pitch: -90.0, inner: 50.0, outer: 50.0, radius: 17.5, shown: 150.0 },
        }
    }

    /// BeginPlay's Delays before the first Time of Day check.
    fn start_delay(self) -> f32 {
        match self {
            Self::RoadLight => 0.0,
            Self::Flood => 0.6 + 2.0,
            Self::Pole => 1.0 + 2.0,
        }
    }

    /// The LightHum_Cue component's place (UE cm) for the night lights.
    fn hum(self) -> Option<Vec3> {
        match self {
            Self::RoadLight => None,
            Self::Flood => Some(Vec3::ZERO),
            Self::Pole => Some(Vec3::new(0.0, 0.0, 522.0)),
        }
    }
}

/// Timeline_TurnOn's length.
const TIMELINE: f32 = 4.0;

/// A two-key auto-tangent cubic from 0 at 0 to `value` at `end` (flat at both keys).
fn curve(t: f32, end: f32, value: f32) -> f32 {
    let s = (t / end).clamp(0.0, 1.0);
    value * s * s * (3.0 - 2.0 * s)
}

/// The spot light's intensity (UE units).
pub fn intensity(kind: SpotKind, on: bool, t: f32) -> f32 {
    match kind {
        SpotKind::RoadLight => 2.5,
        _ if !on => 0.0,
        SpotKind::Flood => curve(t, 4.0, 10.0),
        SpotKind::Pole => curve(t, 6.0, 5.0),
    }
}

/// Candelas for an exponent-falloff intensity: UE lights such a light by `intensity` x
/// (1 - (d / radius)^2)^8 at distance d; this matches inverse-square light at d = radius / 4.
pub fn exponent_candela(intensity: f32, radius: f32) -> f32 {
    let d = radius / 4.0;
    intensity * (1.0 - 1.0 / 16.0f32).powi(8) * d * d
}

#[derive(Component, Debug)]
pub struct Spot {
    pub kind: SpotKind,
    pub on: bool,
    /// Timeline_TurnOn's position.
    pub t: f32,
    /// BeginPlay's Delays left.
    wait: f32,
    /// The actor's place (the AIOSubject's distance check).
    at: Vec3,
    hum: Option<Entity>,
}

impl Spot {
    fn new(kind: SpotKind, at: Vec3) -> Self {
        Self { kind, on: kind == SpotKind::RoadLight, t: 0.0, wait: kind.start_delay(), at, hum: None }
    }

    /// One frame at night or day: the BeginPlay wait, then TurnOn at sunset, TurnOff at sunrise.
    /// Returns the switch made, if any.
    pub fn step(&mut self, dt: f32, night: bool) -> Option<bool> {
        if self.kind == SpotKind::RoadLight {
            return None;
        }
        if self.wait > 0.0 {
            self.wait -= dt;
            if self.wait > 0.0 || !night {
                return None;
            }
        }
        if self.on != night {
            self.on = night;
            return Some(night);
        }
        if self.on {
            self.t = (self.t + dt).min(TIMELINE);
        }
        None
    }
}

/// A component's world place on an actor (UE cm, scaled with the actor), and the direction of
/// its +X pitched by `pitch`.
fn place(a: &ActorInfo, rel: Vec3, pitch: f32) -> (Vec3, Vec3, Vec3) {
    let m = &a.transform;
    let (x, z, y) = (Vec3::new(m[0], m[1], m[2]), Vec3::new(m[3], m[4], m[5]), Vec3::new(m[6], m[7], m[8]));
    let at = a.position() + (x * rel.x + y * rel.y + z * rel.z) / 100.0;
    let (xn, zn) = (x.normalize_or(Vec3::X), z.normalize_or(Vec3::Y));
    let p = pitch.to_radians();
    (at, (xn * p.cos() + zn * p.sin()).normalize(), zn)
}

pub(super) fn setup_spotlights(mut commands: Commands, level: Option<Res<super::Level>>) {
    let Some(level) = level else { return };
    for a in &level.actors {
        let Some(kind) = SpotKind::of(&a.class) else { continue };
        let spec = kind.spec();
        let (at, dir, up) = place(a, spec.rel, spec.pitch);
        // Straight down (the pole's): the actor's forward is up for the cone.
        let up = if dir.cross(up).length_squared() < 1e-4 { Vec3::new(a.transform[0], a.transform[1], a.transform[2]).normalize_or(Vec3::X) } else { up };
        let mut spot = Spot::new(kind, a.position());
        spot.hum = kind.hum().map(|rel| {
            commands
                .spawn((
                    Transform::from_translation(place(a, rel, 0.0).0),
                    crate::audio::AmbientLoop { cue: "LightHum_Cue".into(), volume: 1.0, active: false },
                    crate::audio::LoopAttenuation("CampFire".into()),
                    DespawnOnExit(AppState::InGame),
                ))
                .id()
        });
        commands.spawn((
            SpotLight {
                intensity: 0.0,
                range: spec.radius,
                inner_angle: spec.inner.to_radians(),
                outer_angle: spec.outer.to_radians(),
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(at).looking_to(dir, up),
            Visibility::Hidden,
            spot,
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Sunset / sunrise, the turn-on fade, the hum and the AIOSubject's hiding.
pub(super) fn run_spotlights(
    time: Res<Time>,
    game: Res<CurrentGame>,
    players: Query<&Transform, (With<crate::models::CharacterModel>, Without<Spot>)>,
    mut spots: Query<(&mut Spot, &mut SpotLight, &mut Visibility)>,
    mut hums: Query<&mut crate::audio::AmbientLoop>,
) {
    let Some(eye) = players.iter().next().map(|t| t.translation) else { return };
    let hour = game.state.as_ref().map_or(12.0, |s| s.environment.time_of_day_hours);
    let night = super::lights::night_on(hour);
    for (mut s, mut light, mut vis) in &mut spots {
        if let Some(on) = s.step(time.delta_secs(), night) {
            info!("spot light {:?} turned {}", s.kind, if on { "on" } else { "off" });
            if let Some(mut hum) = s.hum.and_then(|h| hums.get_mut(h).ok()) {
                hum.active = on;
            }
        }
        let ue = intensity(s.kind, s.on, s.t);
        // (0 would be the unreadable-timeline default in `lumens`; such lights are hidden below.)
        let lumens = if ue > 0.0 { super::lights::lumens(exponent_candela(ue, light.range)) } else { 0.0 };
        if light.intensity != lumens {
            light.intensity = lumens;
        }
        let shown = ue > 0.0 && s.at.distance(eye) <= s.kind.spec().shown;
        let want = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flood_lights_fade_in_once_and_cut_out() {
        let mut s = Spot::new(SpotKind::Flood, Vec3::ZERO);
        // BeginPlay's 2.6 s of Delays first.
        assert_eq!(s.step(2.0, true), None);
        assert_eq!(s.step(1.0, true), Some(true));
        assert_eq!(intensity(s.kind, s.on, s.t), 0.0);
        s.step(2.0, true);
        assert!((intensity(s.kind, s.on, s.t) - 5.0).abs() < 1e-4, "half way up the cubic");
        s.step(5.0, true);
        assert_eq!(intensity(s.kind, s.on, s.t), 10.0);
        // Sunrise: the reversed TurnOff timeline at 0, dark at once.
        assert_eq!(s.step(0.1, false), Some(false));
        assert_eq!(intensity(s.kind, s.on, s.t), 0.0);
        // The next sunset jumps to full (Timeline_TurnOn stays at its end).
        assert_eq!(s.step(0.1, true), Some(true));
        assert_eq!(intensity(s.kind, s.on, s.t), 10.0);
    }

    #[test]
    fn pole_lights_stop_short_of_their_curve_and_road_lights_stay_on() {
        let mut s = Spot::new(SpotKind::Pole, Vec3::ZERO);
        assert_eq!(s.step(3.5, false), None, "daytime at the check: off");
        assert_eq!(s.step(1.0, true), Some(true));
        s.step(10.0, true);
        // A 6 s curve cut at the 4 s timeline's end.
        assert!((intensity(s.kind, s.on, s.t) - 5.0 * 20.0 / 27.0).abs() < 1e-4);
        let mut r = Spot::new(SpotKind::RoadLight, Vec3::ZERO);
        assert_eq!(r.step(1.0, false), None);
        assert_eq!(intensity(r.kind, r.on, r.t), 2.5);
        assert!((exponent_candela(2.5, 25.0) - 2.5 * 0.9375f32.powi(8) * 39.0625).abs() < 1e-3);
    }

    #[test]
    fn level_spot_lights() {
        let dir = super::super::LEVEL_DIR;
        let Ok(a) = std::fs::read_to_string(format!("{dir}/actors.json")) else { return };
        let actors: Vec<ActorInfo> = serde_json::from_str(&a).unwrap();
        let count = |k| actors.iter().filter(|a| SpotKind::of(&a.class) == Some(k)).count();
        assert_eq!((count(SpotKind::RoadLight), count(SpotKind::Flood), count(SpotKind::Pole)), (7, 4, 2));
        // The road lights shine down and forward from the top of their pole.
        let a = actors.iter().find(|a| a.class == "BP_RoadLightGenerator_C").unwrap();
        let (at, dir, _) = place(a, SpotKind::RoadLight.spec().rel, -35.0);
        assert!(at.y - a.position().y > 3.0 && dir.y < -0.5);
    }
}
