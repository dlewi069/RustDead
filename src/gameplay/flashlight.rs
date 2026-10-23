//! The survivor's flashlight (BP_PlayerCharacter `FlashlightToggle`, `Battery_Flashlight` and
//! BatteryComponent):
//!
//! - IA_Flashlight (L): without a flashlight in the Flashlight slot, "You do not have a Flashlight
//!   equipped"; on -> off; off -> CheckBatteryLevel: the battery in the flashlight's container
//!   ("No Battery" without one, "Dead battery" at 0 charge), then on.
//! - On: the `Flashlight` spot light (inner/outer cone 35/40 deg, colour E7E7CB, no shadows,
//!   AttenuationRadius 16384) at the item's `FlashlightIntensity` (unitless, 75000 / 175000 for
//!   the high powered one), on `SpringArmFlashlight` (20 cm over the capsule centre, 30 cm ahead
//!   along the camera) facing where the camera looks.
//! - Every 0.1 s UseBattery takes 0.0125 x the battery multiplier off the battery's durability
//!   (100: about 13 min of light); at 0 "Battery run out" and it goes off. Unequipping turns it off.
//!
//! The BP reads the multiplier from `Difficulty.BatteryMultiplier`; the menu's row is
//! `Difficulty.BatteryDepletionMultiplier` (BATTERY DEPLETION MULTIPLIER), used here (a guess:
//! the name the BP reads isn't registered anywhere found). Not ported: the light function
//! materials (Flashlight_CivilianInst / HighPowerInst), on/off sounds, the flashlight mesh in
//! hand. The HUD's battery box is `hud::FlashlightDur`.

use bevy::prelude::*;

use super::equipment::{PlayerInventory, Slot};
use super::items::{ItemDb, Stack};
use super::notify::Notify;
use crate::level::{KinematicBody, LevelZombie};
use crate::menus::settings::SettingsStore;
use crate::menus::{AppState, CurrentGame};
use crate::models::CharacterModel;

/// Battery durability per `Battery_Flashlight` tick, and the tick.
pub const DRAIN: f32 = 0.0125;
pub const TICK: f32 = 0.1;
const INNER_DEG: f32 = 35.0;
const OUTER_DEG: f32 = 40.0;
const RANGE: f32 = 163.84;

/// Whether it's on, and the battery timer.
#[derive(Resource, Default, Debug)]
pub struct Flashlight {
    pub on: bool,
    timer: f32,
}

/// The spot light entity.
#[derive(Component)]
pub struct FlashlightBeam;

/// What pressing L does.
#[derive(Debug, PartialEq)]
pub enum Toggle {
    On,
    Off,
    /// Refused, with the notification text.
    Refused(&'static str),
}

/// The battery in a flashlight's container (its default attachment).
pub fn battery<'a>(flashlight: &'a Stack, db: &ItemDb) -> Option<&'a Stack> {
    let def = db.items.get(&flashlight.item)?;
    flashlight.attachments.iter().find(|a| def.attachments.iter().any(|r| r.item == a.item))
}

fn battery_mut<'a>(flashlight: &'a mut Stack, db: &ItemDb) -> Option<&'a mut Stack> {
    let def = db.items.get(&flashlight.item)?;
    flashlight.attachments.iter_mut().find(|a| def.attachments.iter().any(|r| r.item == a.item))
}

/// A battery's charge left (durability; `None` is full).
pub fn charge(battery: &Stack, db: &ItemDb) -> f32 {
    battery.durability.unwrap_or_else(|| db.items.get(&battery.item).map_or(0.0, |d| d.durability))
}

/// IA_Flashlight with the light `on` or off.
pub fn toggle(inv: &PlayerInventory, db: &ItemDb, on: bool) -> Toggle {
    let Some(f) = inv.equipped.get(&Slot::Flashlight) else { return Toggle::Refused("You do not have a Flashlight equipped") };
    if on {
        return Toggle::Off;
    }
    match battery(f, db) {
        None => Toggle::Refused("No Battery"),
        Some(b) if charge(b, db) == 0.0 => Toggle::Refused("Dead battery"),
        Some(_) => Toggle::On,
    }
}

/// One UseBattery: the charge left, or why the light goes out.
pub fn drain(inv: &mut PlayerInventory, db: &ItemDb, amount: f32) -> Result<f32, &'static str> {
    // OnRep_FlashlightEquipped? turns it off without a message.
    let Some(f) = inv.equipped.get_mut(&Slot::Flashlight) else { return Err("") };
    let Some(b) = battery_mut(f, db) else { return Err("No Battery") };
    let left = (charge(b, db) - amount).max(0.0);
    b.durability = Some(left);
    if left == 0.0 { Err("Battery run out") } else { Ok(left) }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Flashlight>()
        .add_systems(OnEnter(AppState::InGame), |mut commands: Commands, mut f: ResMut<Flashlight>| {
            *f = Flashlight::default();
            commands.spawn((
                FlashlightBeam,
                SpotLight {
                    intensity: 0.0,
                    color: Color::srgb_u8(0xE7, 0xE7, 0xCB),
                    range: RANGE,
                    inner_angle: INNER_DEG.to_radians(),
                    outer_angle: OUTER_DEG.to_radians(),
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::default(),
                Visibility::Hidden,
                DespawnOnExit(AppState::InGame),
            ));
        })
        .add_systems(
            Update,
            (
                press_l.run_if(in_state(crate::menus::MenuScreen::None)).run_if(super::on_foot),
                use_battery,
                aim_beam,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
}

pub fn press_l(keys: Res<ButtonInput<KeyCode>>, game: Res<CurrentGame>, db: Res<ItemDb>, mut light: ResMut<Flashlight>, mut notify: MessageWriter<Notify>, mut sound: MessageWriter<crate::audio::Sound>) {
    if !keys.just_pressed(KeyCode::KeyL) {
        return;
    }
    let inv = game.inventory.clone().unwrap_or_default();
    match toggle(&inv, &db, light.on) {
        Toggle::On => {
            light.on = true;
            light.timer = 0.0;
            sound.write(crate::audio::Sound::ui("Turning_Flashlight_On_Cue"));
        }
        Toggle::Off => {
            light.on = false;
            sound.write(crate::audio::Sound::ui("Turning_Flashlight_Off_3_Cue"));
        }
        Toggle::Refused(why) => {
            notify.write(Notify::new(why));
        }
    }
}

fn use_battery(time: Res<Time>, store: Res<SettingsStore>, mut game: ResMut<CurrentGame>, db: Res<ItemDb>, mut light: ResMut<Flashlight>, mut notify: MessageWriter<Notify>) {
    if !light.on {
        return;
    }
    let Some(inv) = game.inventory.as_mut() else { return };
    light.timer += time.delta_secs();
    let mult = super::survival::difficulty(&store, "General", "BATTERY DEPLETION MULTIPLIER");
    while light.timer >= TICK {
        light.timer -= TICK;
        if let Err(why) = drain(inv, &db, DRAIN * mult) {
            light.on = false;
            if !why.is_empty() {
                notify.write(Notify::new(why));
            }
            return;
        }
    }
    // Unequipped between ticks.
    if !inv.equipped.contains_key(&Slot::Flashlight) {
        light.on = false;
    }
}

/// SpringArmFlashlight: 20 cm over the capsule centre, 30 cm along the camera's view.
#[allow(clippy::type_complexity)]
fn aim_beam(
    light: Res<Flashlight>,
    game: Res<CurrentGame>,
    db: Res<ItemDb>,
    camera: Query<&GlobalTransform, With<super::PlayerCamera>>,
    player: Query<(&Transform, &KinematicBody), (With<CharacterModel>, Without<LevelZombie>, Without<FlashlightBeam>)>,
    mut beam: Query<(&mut Transform, &mut SpotLight, &mut Visibility), With<FlashlightBeam>>,
) {
    let Ok((mut t, mut spot, mut vis)) = beam.single_mut() else { return };
    let (Ok(cam), Ok((me, body))) = (camera.single(), player.single()) else { return };
    let item = game.inventory.as_ref().and_then(|i| i.equipped.get(&Slot::Flashlight)).and_then(|s| db.items.get(&s.item));
    let want = if light.on { Visibility::Inherited } else { Visibility::Hidden };
    vis.set_if_neq(want);
    if !light.on {
        return;
    }
    // Unitless intensity is candela x 625 (see level/lights.rs).
    spot.intensity = crate::level::lights::lumens(item.map_or(75000.0, |d| d.flashlight_intensity) / 625.0);
    let forward = cam.forward().as_vec3();
    t.translation = me.translation + Vec3::Y * (body.height * 0.5 + 0.2) + forward * 0.3;
    t.rotation = cam.rotation();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Option<ItemDb> {
        serde_json::from_str(&std::fs::read_to_string("assets/data/items.json").ok()?).ok()
    }

    #[test]
    fn the_flashlight_needs_a_charged_battery() {
        let Some(db) = db() else { return };
        let mut inv = PlayerInventory::default();
        assert_eq!(toggle(&inv, &db, false), Toggle::Refused("You do not have a Flashlight equipped"));
        let mut f = db.stack("DA_Flashlight", 1);
        f.attachments.clear();
        inv.equipped.insert(Slot::Flashlight, f);
        assert_eq!(toggle(&inv, &db, false), Toggle::Refused("No Battery"));
        let mut b = db.stack("DA_Battery", 1);
        b.durability = Some(0.0);
        inv.equipped.get_mut(&Slot::Flashlight).unwrap().attachments.push(b);
        assert_eq!(toggle(&inv, &db, false), Toggle::Refused("Dead battery"));
        inv.equipped.get_mut(&Slot::Flashlight).unwrap().attachments[0].durability = None;
        assert_eq!(toggle(&inv, &db, false), Toggle::On);
        assert_eq!(toggle(&inv, &db, true), Toggle::Off);
    }

    #[test]
    fn a_battery_lasts_8000_ticks() {
        let Some(db) = db() else { return };
        let mut inv = PlayerInventory::default();
        let mut f = db.stack("DA_Flashlight", 1);
        f.attachments = vec![db.stack("DA_Battery", 1)];
        inv.equipped.insert(Slot::Flashlight, f);
        // 100 / 0.0125 = 8000 ticks of 0.1 s: 13 min 20 s.
        let mut ticks = 0;
        while drain(&mut inv, &db, DRAIN).is_ok() {
            ticks += 1;
        }
        // (f32 steps: within a tick or two of 8000.)
        assert!((7990..=8010).contains(&ticks), "{ticks}");
        assert_eq!(drain(&mut inv, &db, DRAIN), Err("Battery run out"));
        inv.equipped.remove(&Slot::Flashlight);
        assert_eq!(drain(&mut inv, &db, DRAIN), Err(""));
        // The high powered one is brighter.
        assert!(db.items["DA_HighPoweredFlashlight"].flashlight_intensity > db.items["DA_Flashlight"].flashlight_intensity);
    }
}
