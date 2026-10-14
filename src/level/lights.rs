//! The level's point lights (Blueprint lamps and fires), from `lights.json` (`SdExport
//! levellights` + `extractor/gen_lights.py`: each light resolved against its Blueprint's component
//! template, intensity in candelas).
//!
//! - `BP_ResortLight`: on from 17:59 to 05:59 (its Time of Day >= 1799 / <= 599 check), else off.
//! - Fires (`BP_BarrelFire`, `BP_Campfire`, `BP_CookingStation`, `BP_EasterEgg_Campfire`): lit, with
//!   a flame flicker (their particle flames aren't drawn).
//! - `BP_FlickeringLight(Sparks)`: the LightFlicker timeline, approximated by random dips.
//! - Lab lights (`BP_LaboratoryLight(2/3)`): lit while their switch is On (`power`), at the
//!   class's `Intensity` 2000 (unitless: 3.2 cd). Some instances set 1.5 with exponent falloff,
//!   which has no candela equivalent; they use the class value too (a guess).
//! - Infestation lights and lamps belong to systems not ported yet and stay off; a resort light's
//!   on-intensity comes from a timeline that isn't readable, so it uses the engine default (8 cd).
//!
//! Brightness: our night is lit ~1100x brighter than UDS's (moon 450 lux standing in for its 0.4
//! with auto exposure), so lamps are scaled the same way to keep their in-game contrast with
//! the night. The nearest [`MAX_LIGHTS`] within [`RANGE`] are spawned, without shadows.

use bevy::prelude::*;
use rand::Rng;
use serde::Deserialize;

use crate::menus::{AppState, CurrentGame};

const MAX_LIGHTS: usize = 24;
const RANGE: f32 = 70.0;
/// Our moonlight (`daynight::sky_at`, 450 lux) over UDS's Moon Light Intensity (0.4).
const NIGHT_SCALE: f32 = 450.0 / 0.4;
/// The point-light default when a Blueprint drives intensity from an unreadable timeline.
const DEFAULT_CANDELA: f32 = 8.0;

#[derive(Deserialize, Clone, Debug)]
pub struct LevelLight {
    pub owner: String,
    /// The owner actor's name.
    #[serde(default)]
    pub actor: String,
    pub pos: [f32; 3],
    pub candela: f32,
    pub color: [f32; 3],
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behaviour {
    NightOnly,
    Fire,
    Flicker,
    /// Lab lamps: on with their switch.
    Lab,
    Off,
}

/// BP_LaboratoryLight's `Intensity` default (2000 unitless).
const LAB_CANDELA: f32 = 2000.0 / 625.0;

pub fn behaviour(owner: &str) -> Behaviour {
    match owner {
        "BP_ResortLight_C" => Behaviour::NightOnly,
        "BP_BarrelFire_C" | "BP_Campfire_C" | "BP_CookingStation_C" | "BP_EasterEgg_Campfire_C" => Behaviour::Fire,
        "BP_FlickeringLight_C" | "BP_FlickeringLightSparks_C" => Behaviour::Flicker,
        "BP_LaboratoryLight_C" | "BP_LaboratoryLight2_C" | "BP_LaboratoryLight3_C" => Behaviour::Lab,
        _ => Behaviour::Off,
    }
}

/// Is a NightOnly light on at `hour` (UDS Time of Day >= 1799 or <= 599)?
pub fn night_on(hour: f32) -> bool {
    hour * 100.0 >= 1799.0 || hour * 100.0 <= 599.0
}

#[derive(Resource)]
pub struct LevelLights {
    lights: Vec<(LevelLight, Behaviour)>,
    spawned: Vec<(usize, Entity)>,
    refresh: f32,
}

#[derive(Component)]
pub(super) struct LampOf(usize);

pub(super) fn setup_lights(mut commands: Commands) {
    let Some(list) = std::fs::read_to_string(format!("{}/lights.json", super::LEVEL_DIR)).ok().and_then(|t| serde_json::from_str::<Vec<LevelLight>>(&t).ok()) else { return };
    let lights = list.into_iter().map(|l| (l.clone(), behaviour(&l.owner))).filter(|(_, b)| *b != Behaviour::Off).collect();
    commands.insert_resource(LevelLights { lights, spawned: Vec::new(), refresh: 0.0 });
}

pub(super) fn reset_lights(mut commands: Commands) {
    commands.remove_resource::<LevelLights>();
}

/// Lumens for a light's candelas at our light scale.
pub fn lumens(candela: f32) -> f32 {
    let cd = if candela > 0.0 { candela } else { DEFAULT_CANDELA };
    cd * 4.0 * std::f32::consts::PI * NIGHT_SCALE
}

pub(super) fn update_lights(
    mut commands: Commands,
    time: Res<Time>,
    game: Res<CurrentGame>,
    lights: Option<ResMut<LevelLights>>,
    power: Option<Res<super::power::Power>>,
    players: Query<&Transform, With<crate::models::CharacterModel>>,
    mut lamps: Query<(&LampOf, &mut PointLight)>,
) {
    let Some(mut ll) = lights else { return };
    let Some(eye) = players.iter().next().map(|t| t.translation) else { return };
    let hour = game.state.as_ref().map_or(12.0, |s| s.environment.time_of_day_hours);
    ll.refresh -= time.delta_secs();
    if power.as_ref().is_some_and(|p| p.is_changed()) {
        ll.refresh = 0.0;
    }
    if ll.refresh <= 0.0 {
        ll.refresh = 0.5;
        let mut near: Vec<(usize, f32)> = ll
            .lights
            .iter()
            .enumerate()
            .filter(|(_, (_, b))| *b != Behaviour::NightOnly || night_on(hour))
            .filter(|(_, (l, b))| *b != Behaviour::Lab || power.as_ref().is_some_and(|p| p.lit(&l.actor)))
            .map(|(i, (l, _))| (i, Vec3::from(l.pos).distance(eye)))
            .filter(|(_, d)| *d <= RANGE)
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        near.truncate(MAX_LIGHTS);
        let keep: Vec<usize> = near.iter().map(|n| n.0).collect();
        let old = std::mem::take(&mut ll.spawned);
        for (i, e) in old {
            if keep.contains(&i) {
                ll.spawned.push((i, e));
            } else {
                commands.entity(e).despawn();
            }
        }
        for i in keep {
            if ll.spawned.iter().any(|s| s.0 == i) {
                continue;
            }
            let (l, _) = &ll.lights[i];
            let e = commands
                .spawn((
                    PointLight {
                        color: Color::srgb(l.color[0], l.color[1], l.color[2]),
                        intensity: lumens(if ll.lights[i].1 == Behaviour::Lab { LAB_CANDELA } else { l.candela }),
                        range: l.radius.max(1.0),
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(Vec3::from(l.pos)),
                    LampOf(i),
                    DespawnOnExit(AppState::InGame),
                ))
                .id();
            ll.spawned.push((i, e));
        }
    }
    // Flames and failing bulbs.
    let mut rng = rand::thread_rng();
    for (LampOf(i), mut p) in &mut lamps {
        let (l, b) = &ll.lights[*i];
        let base = lumens(if *b == Behaviour::Lab { LAB_CANDELA } else { l.candela });
        p.intensity = match b {
            Behaviour::Fire => base * rng.gen_range(0.75..1.1),
            Behaviour::Flicker => base * if rng.gen_range(0.0..1.0) < 0.08 { 0.1 } else { 1.0 },
            _ => base,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resort_lights_follow_the_clock() {
        assert!(night_on(18.0) && night_on(23.5) && night_on(3.0) && night_on(5.99));
        assert!(!night_on(6.0) && !night_on(12.0) && !night_on(17.9));
    }

    #[test]
    fn exported_lights_resolve_their_owners() {
        let Ok(t) = std::fs::read_to_string(format!("{}/lights.json", crate::level::LEVEL_DIR)) else { return };
        let lights: Vec<LevelLight> = serde_json::from_str(&t).unwrap();
        let fires: Vec<_> = lights.iter().filter(|l| l.owner == "BP_BarrelFire_C").collect();
        assert_eq!(fires.len(), 23);
        // The barrel fire's template: orange (255, 165, 0), 10 m.
        assert!((fires[0].color[1] - 0.647).abs() < 0.01 && fires[0].radius == 10.0);
        assert_eq!(behaviour("BP_ResortLight_C"), Behaviour::NightOnly);
        assert_eq!(behaviour("BP_LaboratoryLight_C"), Behaviour::Lab);
        assert_eq!(behaviour("BP_PrimaryInfestation_C"), Behaviour::Off);
    }
}
