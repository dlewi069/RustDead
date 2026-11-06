//! Tactical weapon attachments: lights and the range finder, from BP_PlayerCharacter's
//! ToggleTacticalAttachment (IA_ToggleTactical, K), the BP_*TacticalLight* / RangeFinder
//! LocalAttachments' Jig_SetAttachmentActiveState and BatteryComponent (decompiled):
//!
//! - K: the active weapon's Tactical attachments all go to the opposite of the first one's
//!   state (State1 off, State2 on), whatever their battery.
//! - On (Event_Battery(true)): CheckBatteryLevel looks in the attachment's SC_Battery container:
//!   none -> "No Battery", a 0-charge battery -> "Dead battery", and the light stays dark; else
//!   a 0.1 s timer and the SpotLight's SetIntensity: 65000 (BP_TacticalLightLocalAttachment),
//!   50000 (the small light and the laser-light combo); the range finder shows BP_Ingame's
//!   RangeFinderDistance instead. Each tick UseBattery takes 0.0125 (lights) / 0.01 (range
//!   finder) x the battery multiplier; only a missing battery turns it off (Event_Battery(false)):
//!   UseBattery succeeds at 0 charge, so a light left on keeps shining on a flat battery, as in
//!   the game. Off: the timer cleared, intensity 0 (the range finder's text emptied and Hidden).
//! - The range finder's Event_RangeFinderTrace every 0.1 s: a 1 cm sphere from the camera along
//!   its forward for 10 km (world static/dynamic, pawns, physics bodies; not the survivor),
//!   UpdateRangeFinderDistance(FTrunc(Distance)) -> "{Distance / 100}m" with grouping, at the
//!   screen centre + (30, 30), size 12 (0m when nothing is hit).
//! - The lights' templates (items.json `attachment.actor.lights`: OuterConeAngle 35 / 40,
//!   AttenuationRadius 120-164 m) hang on the attachment's mesh (`attachments::show_attachments`),
//!   pointing along UE +X turned by the component's yaw.
//!
//! Ours: the state lives per weapon slot for the session (the game keeps it on the attachment's
//! info); the range finder's trace hits the landscape and props within 500 m (props stream to
//! ~420 m), not pawns.
//! Not ported: the lasers' 250-intensity point lights (AttenuationRadius 1 cm: nothing to see),
//! the NVGs' SC_Battery (night vision isn't ported).

use std::collections::HashMap;

use bevy::prelude::*;

use super::combat::{Armed, PlayerCamera};
use super::equipment::Slot;
use super::items::{ItemDb, Stack};
use super::notify::Notify;
use crate::menus::settings::SettingsStore;
use crate::menus::widgets::MenuFont;
use crate::menus::{AppState, CurrentGame};

/// The battery timer.
pub const TICK: f32 = 0.1;
/// UseBattery's amounts: the lights' Battery_Flashlight, the range finder's.
pub const LIGHT_DRAIN: f32 = 0.0125;
pub const RANGE_FINDER_DRAIN: f32 = 0.01;
/// Event_RangeFinderTrace's 1000000 cm.
const TRACE_CM: f32 = 1_000_000.0;
/// How far our props answer the range finder (ours).
const PROP_REACH: f32 = 500.0;

/// What a tactical attachment does when on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// SetIntensity's literal.
    Light(f32),
    RangeFinder,
}

/// By the attachment item: the literals in each LocalAttachment's Event_Battery.
pub fn kind(item: &str) -> Option<Kind> {
    match item {
        "DA_TacticalLight" => Some(Kind::Light(65000.0)),
        "DA_SmallTacticalLight" | "DA_TacticalLaserLightCombo" => Some(Kind::Light(50000.0)),
        "DA_RangeFinder" => Some(Kind::RangeFinder),
        _ => None,
    }
}

/// The weapon's Tactical attachments (Jig_GetAttachmentArrayByType).
pub fn tacticals<'a>(db: &ItemDb, weapon: &'a Stack) -> Vec<&'a Stack> {
    weapon.attachments.iter().filter(|a| db.items.get(&a.item).and_then(|d| d.attachment.as_ref()).is_some_and(|d| d.kind == "Tactical")).collect()
}

/// CheckBatteryLevel: Ok when it can light, else the message.
pub fn check_battery(db: &ItemDb, tactical: &Stack) -> Result<(), &'static str> {
    match super::flashlight::battery(tactical, db) {
        None => Err("No Battery"),
        Some(b) if super::flashlight::charge(b, db) == 0.0 => Err("Dead battery"),
        Some(_) => Ok(()),
    }
}

/// UseBattery on the weapon's tactical `item`: false when there's no battery (it goes off).
pub fn use_battery(db: &ItemDb, weapon: &mut Stack, item: &str, amount: f32) -> bool {
    let Some(t) = weapon.attachments.iter_mut().find(|a| a.item == item) else { return false };
    let Some(def) = db.items.get(&t.item) else { return false };
    let Some(b) = t.attachments.iter_mut().find(|a| def.holds.contains(&a.item) || def.attachments.iter().any(|r| r.item == a.item)) else { return false };
    let left = super::flashlight::charge(b, db);
    b.durability = Some((left - amount).max(0.0));
    true
}

/// "{A}m": Divide_IntInt(FTrunc(cm), 100) with grouping.
pub fn range_text(cm: f32) -> String {
    let m = (cm.trunc() as i64) / 100;
    let digits = m.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    format!("{}{out}m", if m < 0 { "-" } else { "" })
}

/// The tacticals' states: per weapon slot, State2 (on) and whether each lit.
#[derive(Resource, Default, Debug)]
pub struct Tactical {
    pub on: HashMap<Slot, bool>,
    /// Attachment items lit (their timer running), by slot.
    pub lit: HashMap<Slot, Vec<String>>,
    timer: f32,
    /// The range finder's last reading (cm), while one is lit.
    pub range: Option<f32>,
}

/// A tactical light's beam on an attachment mesh.
#[derive(Component)]
pub struct TacticalBeam {
    pub slot: Slot,
    pub item: String,
}

/// BP_Ingame's RangeFinderDistance.
#[derive(Component)]
pub struct RangeFinderText;

pub fn plugin(app: &mut App) {
    app.init_resource::<Tactical>()
        .add_systems(OnEnter(AppState::InGame), (|mut t: ResMut<Tactical>| *t = Tactical::default(), spawn_text))
        .add_systems(
            Update,
            (
                press_k.run_if(in_state(crate::menus::MenuScreen::None)).run_if(super::on_foot),
                drain_and_trace,
                light_beams,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
}

fn spawn_text(mut commands: Commands, font: Res<MenuFont>) {
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Percent(50.0), top: Val::Percent(50.0), margin: UiRect { left: Val::Px(30.0), top: Val::Px(30.0), ..default() }, ..default() },
        crate::menus::widgets::label_colored(&font, "", 12.0, Color::WHITE),
        Visibility::Hidden,
        RangeFinderText,
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
    ));
}

/// ToggleTacticalAttachment.
pub fn press_k(keys: Res<ButtonInput<KeyCode>>, armed: Res<Armed>, game: Res<CurrentGame>, db: Res<ItemDb>, mut tac: ResMut<Tactical>, mut notify: MessageWriter<Notify>) {
    if !keys.just_pressed(KeyCode::KeyK) {
        return;
    }
    let Some(slot) = armed.drawn else { return };
    let Some(weapon) = game.inventory.as_ref().and_then(|i| i.equipped.get(&slot)) else { return };
    let ts = tacticals(&db, weapon);
    if ts.is_empty() {
        return;
    }
    let on = !tac.on.get(&slot).copied().unwrap_or(false);
    tac.on.insert(slot, on);
    let (mut lit, mut refused) = (Vec::new(), Vec::new());
    if on {
        for t in ts {
            match check_battery(&db, t) {
                Ok(()) => lit.push(t.item.clone()),
                Err(why) => {
                    notify.write(Notify::new(why));
                    refused.push(why);
                }
            }
        }
    }
    info!("tactical: {slot:?} {} lit {lit:?} refused {refused:?}", if on { "on" } else { "off" });
    tac.lit.insert(slot, lit);
    tac.timer = 0.0;
}

/// The battery timers and the range finder's trace.
#[allow(clippy::too_many_arguments)]
fn drain_and_trace(
    time: Res<Time>,
    store: Res<SettingsStore>,
    db: Res<ItemDb>,
    mut game: ResMut<CurrentGame>,
    mut tac: ResMut<Tactical>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut props: crate::level::collision::PropRays,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
    mut text: Query<(&mut Text, &mut Visibility), With<RangeFinderText>>,
) {
    let Some(inv) = game.inventory.as_mut() else { return };
    // A weapon gone or an attachment taken off: its actor (and timer) are destroyed.
    let tac = &mut *tac;
    for (slot, items) in tac.lit.iter_mut() {
        let present = inv.equipped.get(slot).map(|w| tacticals(&db, w).iter().map(|t| t.item.clone()).collect::<Vec<_>>()).unwrap_or_default();
        items.retain(|i| present.contains(i));
        if present.is_empty() {
            tac.on.remove(slot);
        }
    }
    tac.timer += time.delta_secs();
    let mult = super::survival::difficulty(&store, "General", "BATTERY DEPLETION MULTIPLIER");
    let mut ranging = false;
    while tac.timer >= TICK {
        tac.timer -= TICK;
        for (slot, items) in tac.lit.iter_mut() {
            let Some(w) = inv.equipped.get_mut(slot) else { continue };
            items.retain(|i| {
                let amount = if kind(i) == Some(Kind::RangeFinder) { RANGE_FINDER_DRAIN } else { LIGHT_DRAIN };
                use_battery(&db, w, i, amount * mult)
            });
        }
        ranging = true;
    }
    let finder = tac.lit.values().flatten().any(|i| kind(i) == Some(Kind::RangeFinder));
    if !finder {
        tac.range = None;
    } else if ranging || tac.range.is_none() {
        let Ok(cam) = camera.single() else { return };
        let (o, d) = (cam.translation(), cam.forward().as_vec3());
        let max = TRACE_CM / 100.0;
        let ground = super::combat::terrain_hit(terrain.as_deref(), o, d, max);
        let prop = props.cast(o, d, PROP_REACH.min(max));
        let hit = match (ground, prop) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        tac.range = Some(hit.map_or(0.0, |m| m * 100.0));
    }
    if let Ok((mut t, mut v)) = text.single_mut() {
        let (want, vis) = match tac.range {
            Some(cm) => (range_text(cm), Visibility::Inherited),
            None => (String::new(), Visibility::Hidden),
        };
        if t.0 != want {
            t.0 = want;
        }
        v.set_if_neq(vis);
    }
}

/// SetIntensity on the lit lights' beams.
fn light_beams(tac: Res<Tactical>, mut beams: Query<(&TacticalBeam, &mut SpotLight)>) {
    for (b, mut spot) in &mut beams {
        let lit = tac.lit.get(&b.slot).is_some_and(|l| l.contains(&b.item));
        let want = match (lit, kind(&b.item)) {
            // Unitless intensity is candela x 625 (see level/lights.rs).
            (true, Some(Kind::Light(i))) => crate::level::lights::lumens(i / 625.0),
            _ => 0.0,
        };
        if spot.intensity != want {
            spot.intensity = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Option<ItemDb> {
        let db = ItemDb::load();
        (!db.items.is_empty()).then_some(db)
    }

    #[test]
    fn range_finder_text_groups_metres() {
        assert_eq!(range_text(0.0), "0m");
        assert_eq!(range_text(4599.9), "45m");
        assert_eq!(range_text(123_456_7.0), "12,345m");
    }

    #[test]
    fn tactical_lights_need_a_battery_and_drain_it() {
        let Some(db) = db() else { return };
        let mut m4 = db.stack("DA_M4CQB", 1);
        m4.attachments.clear();
        super::super::attachments::attach(&db, &mut m4, db.stack("DA_TacticalLight", 1)).unwrap();
        assert_eq!(tacticals(&db, &m4).len(), 1);
        assert_eq!(check_battery(&db, tacticals(&db, &m4)[0]), Err("No Battery"));
        assert!(!use_battery(&db, &mut m4, "DA_TacticalLight", LIGHT_DRAIN), "no battery: off");
        // A battery dropped into the light (its SC_Battery).
        let light = m4.attachments.iter_mut().find(|a| a.item == "DA_TacticalLight").unwrap();
        light.attachments.push(db.stack("DA_Battery", 1));
        assert_eq!(check_battery(&db, tacticals(&db, &m4)[0]), Ok(()));
        assert!(use_battery(&db, &mut m4, "DA_TacticalLight", LIGHT_DRAIN));
        let b = super::super::flashlight::battery(tacticals(&db, &m4)[0], &db).unwrap();
        let full = db.items["DA_Battery"].durability;
        assert!((super::super::flashlight::charge(b, &db) - (full - LIGHT_DRAIN)).abs() < 1e-5);
        // Flat: refused when turned on, but a lit light keeps using it.
        let light = m4.attachments.iter_mut().find(|a| a.item == "DA_TacticalLight").unwrap();
        light.attachments[0].durability = Some(0.0);
        assert_eq!(check_battery(&db, tacticals(&db, &m4)[0]), Err("Dead battery"));
        assert!(use_battery(&db, &mut m4, "DA_TacticalLight", LIGHT_DRAIN));
        assert_eq!(kind("DA_TacticalLight"), Some(Kind::Light(65000.0)));
        assert_eq!(kind("DA_SmallTacticalLaserSight"), None);
    }
}
