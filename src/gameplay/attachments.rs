//! Weapon attachments and the camera's field of view, from `BP_FirearmPickup`,
//! `BP_WeaponsPickupComponent`, the SC_<gun> special-container widgets and BP_PlayerCharacter's
//! IA_ScopeZoom / SightFunction / ScopeFunction (decompiled):
//!
//! - Slots: each firearm's SC_<gun> widget holds one JSIContainer per attachment type (Muzzle,
//!   Tactical, Grip, Mag, Sight, Chamber), each taking only its OnlySupportTheseItems
//!   (items.json `slots`). Magazines stay with the magazine code (`Stack::mag`).
//! - FindAttachmentStats sums a stat over every attachment (the magazine included);
//!   FindSpecificAttachmentStat reads one attachment type's stat, 1.0 when the attachment or
//!   the stat is missing (and the BP uses that 1.0).
//! - GetVertical / GetHorizontalRecoilStrength: 1 + VerticalRecoilControl (HorizontalRecoilControl)
//!   / 100 when the sum isn't 0, else 1; StartRecoil multiplies its kicks by them.
//! - DamageMultiplier adds DamageChange to the (headshot-multiplied) FirearmDamage.
//! - EffectiveDamage: the effective range is DamageFallOff / (1 + |muzzle Falloff| / 100), so a
//!   muzzle brake's +30 % and a suppressor's -15 % both shorten it, and no muzzle at all
//!   divides by 1.01 (the 1.0 fallback).
//! - MuzzleEffects: with a suppressor (the component's Suppressors list) whose MuzzleDurability
//!   holds: FirearmSuppressedSound and SuppressedMuzzleFlash, the noise out to DefaultSoundRadius
//!   x (1 + WeaponNoise / 100). A muzzle brake (MuzzleBrakes): the normal sound and flash, the same
//!   noise formula. Otherwise (or a worn-out muzzle) the plain shot. MuzzleDurability (MUZZLE
//!   ATTACHMENT DURABILITY ENABLED?): the muzzle loses ReduceDurabilityAmount x MUZZLE ATTACHMENT
//!   DURABILITY MULTIPLIER x MuzzleDurabilityMultiplier (1) a shot, and at 0 it stops working.
//! - FireBullet / FireTrace: from the hip the round leaves in a random cone of 3 / (1 +
//!   HipFireAccuracy / 100) degrees (3 without the stat); aimed, straight.
//! - IA_ScopeZoom (middle mouse) while aiming: a weapon's sight (Sights list) toggles
//!   SightFunction(FOVZoom), a scope (Scopes) ScopeFunction(FOVZoom, its ScopesUI overlay); the
//!   Phantasm and Survivor have a fixed 45, the XM250 an Acog at 20, the Cerberus a long scope at
//!   15. Third person they set the camera's field of view to the zoom (FInterpTo at speed 0.25 for
//!   one frame: the zoom almost at once).
//! - The camera's field of view is THIRD PERSON FIELD OF VIEW (Character.FieldOfView, 90),
//!   horizontal as in UE.
//!
//! The overlays, sway and holding breath are in `scopes.rs`.
//!
//! - ProcessAttachments (BP_JigPickupComponent): each attachment's Local_ActorClass (items.json
//!   `attachment.actor`: its StaticMesh relative to the actor) snapped to the gun mesh's
//!   Local_AttachSocket (the skeleton's sockets: `mesh_sockets`); a socket the mesh lacks puts
//!   it at the mesh's origin, as UE does (the Range Finder on the Cerberus, Exterminator and
//!   Survivor).
//!
//! Not ported: tactical
//! lights / lasers / the range finder's functions, the sights' depth of field.

use bevy::prelude::*;

use super::items::{ItemDb, Stack};

/// FireTrace's hip-fire cone (degrees) without HipFireAccuracy.
pub const HIP_CONE: f32 = 3.0;

/// The slot `item` fits on `weapon` (a non-magazine attachment), if any.
pub fn slot_for<'a>(db: &'a ItemDb, weapon: &str, item: &str) -> Option<&'a str> {
    let def = db.items.get(weapon)?;
    def.slots.iter().find(|s| !matches!(s.tag.as_str(), "Mag" | "Chamber") && s.items.iter().any(|i| i == item)).map(|s| s.tag.as_str())
}

/// The attachment in a slot.
pub fn in_slot<'a>(db: &ItemDb, weapon: &'a Stack, tag: &str) -> Option<&'a Stack> {
    weapon.attachments.iter().find(|a| db.items.get(&a.item).and_then(|d| d.attachment.as_ref()).is_some_and(|d| d.kind == tag))
}

/// Drop an attachment onto a weapon: into its free slot.
pub fn attach(db: &ItemDb, weapon: &mut Stack, item: Stack) -> Result<(), Stack> {
    let Some(tag) = slot_for(db, &weapon.item, &item.item) else { return Err(item) };
    if item.count != 1 || in_slot(db, weapon, tag).is_some() {
        return Err(item);
    }
    weapon.attachments.push(item);
    Ok(())
}

/// The non-magazine attachments (what Show Attachments can take off).
pub fn removable(db: &ItemDb, weapon: &Stack) -> Vec<usize> {
    weapon
        .attachments
        .iter()
        .enumerate()
        .filter(|(_, a)| db.items.get(&a.item).and_then(|d| d.attachment.as_ref()).is_some_and(|d| d.kind != "Mag"))
        .map(|(i, _)| i)
        .collect()
}

/// An attachment's stat (its ItemStats' MinValue).
fn item_stat(db: &ItemDb, a: &Stack, stat: &str) -> Option<f32> {
    db.items.get(&a.item)?.stats.get(stat).map(|v| v[0])
}

/// FindAttachmentStats: the sum over every attachment, and whether any had it.
pub fn stat_sum(db: &ItemDb, weapon: &Stack, stat: &str) -> Option<f32> {
    let vals: Vec<f32> = weapon.attachments.iter().filter_map(|a| item_stat(db, a, stat)).collect();
    (!vals.is_empty()).then(|| vals.iter().sum())
}

/// FindSpecificAttachmentStat: one type's stat, 1.0 when missing.
pub fn type_stat(db: &ItemDb, weapon: &Stack, kind: &str, stat: &str) -> f32 {
    in_slot(db, weapon, kind).and_then(|a| item_stat(db, a, stat)).unwrap_or(1.0)
}

/// GetVerticalRecoilStrength / GetHorizontalRecoilStrength.
pub fn recoil_strength(db: &ItemDb, weapon: &Stack, stat: &str) -> f32 {
    match stat_sum(db, weapon, stat) {
        Some(v) if v != 0.0 => 1.0 + v / 100.0,
        _ => 1.0,
    }
}

/// EffectiveDamage's range divisor: 1 + |muzzle Falloff| / 100.
pub fn falloff_divisor(db: &ItemDb, weapon: &Stack) -> f32 {
    1.0 + type_stat(db, weapon, "Muzzle", "Falloff").abs() / 100.0
}

/// FireTrace's cone half-angle (degrees): hip fire only.
pub fn hip_cone(db: &ItemDb, weapon: &Stack, hip: bool) -> f32 {
    if !hip {
        return 0.0;
    }
    match stat_sum(db, weapon, "HipFireAccuracy") {
        Some(v) => HIP_CONE / (1.0 + v / 100.0),
        None => HIP_CONE,
    }
}

/// What MuzzleEffects does for a shot.
#[derive(Debug, PartialEq)]
pub struct Muzzle {
    pub suppressed: bool,
    /// ReportNoiseEvent's range as a multiple of DefaultSoundRadius.
    pub noise: f32,
}

/// MuzzleEffects + MuzzleDurability (`wear` = the per-shot loss, or `None` with the setting off).
pub fn muzzle(db: &ItemDb, weapon: &mut Stack, wear: Option<f32>) -> Muzzle {
    let plain = Muzzle { suppressed: false, noise: 1.0 };
    let role = in_slot(db, weapon, "Muzzle").and_then(|a| db.items.get(&a.item)?.attachment.as_ref()?.role.clone());
    if !matches!(role.as_deref(), Some("suppressor" | "muzzle_brake")) {
        return plain;
    }
    let noise = 1.0 + type_stat(db, weapon, "Muzzle", "WeaponNoise") / 100.0;
    if let Some(w) = wear {
        let Some(a) = weapon.attachments.iter_mut().find(|a| db.items.get(&a.item).and_then(|d| d.attachment.as_ref()).is_some_and(|d| d.kind == "Muzzle")) else { return plain };
        let full = db.items.get(&a.item).map_or(0.0, |d| d.durability);
        let left = a.durability.unwrap_or(full);
        if left == 0.0 {
            return plain;
        }
        let loss = db.items.get(&a.item).map_or(0.0, |d| d.reduce_durability) * w;
        a.durability = Some((left - loss).max(0.0));
    }
    Muzzle { suppressed: role.as_deref() == Some("suppressor"), noise }
}

/// Which ScopesUI overlay a scope shows (`scopes.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// AcogScope (AcogDetailedOverlay).
    Acog,
    /// NormalScope as LongScope (SniperScopeDetailedOverlay).
    Long,
    /// MosinSniperScope as PUScope (PUScopeDetailedOverlay).
    Pu,
}

/// BP_WeaponsPickupComponent.ScopeFunction's switch on the scope's ItemID.
pub fn scope_overlay(item_id: &str) -> Scope {
    match item_id {
        "Acog4x32Scope" | "ColonelsRevengeAcog" | "CrusherAcog" | "ValkyrieAcog" | "GuardianAcog" | "LechieAcog" => Scope::Acog,
        "PUScope" => Scope::Pu,
        _ => Scope::Long,
    }
}

/// IA_ScopeZoom's target: a sight's (or built-in sight's) FOVZoom, and the scope's overlay
/// (None for a sight).
pub fn zoom(db: &ItemDb, weapon: &Stack) -> Option<(f32, Option<Scope>)> {
    let id = db.items.get(&weapon.item).map(|d| d.id.as_str());
    match id {
        Some("Phantasm" | "Survivor") => return Some((45.0, None)),
        Some("XM250") => return Some((20.0, Some(Scope::Acog))),
        Some("Cerberus") => return Some((15.0, Some(Scope::Long))),
        _ => {}
    }
    let sight = in_slot(db, weapon, "Sight")?;
    let def = db.items.get(&sight.item)?;
    let a = def.attachment.as_ref()?;
    match a.role.as_deref() {
        Some("sight") => Some((a.fov_zoom?, None)),
        Some("scope") => Some((a.fov_zoom?, Some(scope_overlay(&def.id)))),
        _ => None,
    }
}

/// UE's horizontal field of view as Bevy's vertical one.
pub fn vertical_fov(horizontal_deg: f32, aspect: f32) -> f32 {
    2.0 * ((horizontal_deg.to_radians() / 2.0).tan() / aspect.max(0.1)).atan()
}

/// PlayerUsingScope? and the zoom it set.
#[derive(Resource, Default, Debug)]
pub struct Zoom(pub Option<(f32, Option<Scope>)>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Zoom>()
        .add_systems(OnEnter(crate::menus::AppState::InGame), |mut z: ResMut<Zoom>| z.0 = None)
        .add_systems(Update, (scope_zoom, camera_fov, show_attachments).chain().run_if(in_state(crate::menus::AppState::InGame)));
}

/// Where an attachment's actor goes on a gun: the socket's bone (None = the mesh's origin, as
/// UE attaches to a socket the mesh lacks) and the actor's transform under it.
pub fn placement<'a>(db: &'a ItemDb, weapon: &str, item: &str) -> Option<(Option<&'a str>, Transform, &'a str)> {
    let a = db.items.get(item)?.attachment.as_ref()?;
    let actor = a.actor.as_ref()?;
    let mesh = actor.mesh.as_deref()?;
    let local = actor.transform.map_or(Transform::IDENTITY, |t| t.to_transform());
    let socket = a.socket.as_deref().and_then(|name| db.items.get(weapon)?.mesh_sockets.get(name));
    Some(match socket {
        Some(s) => (Some(s.bone.as_str()), s.transform.to_transform() * local, mesh),
        None => (None, local, mesh),
    })
}

/// A glTF node named for `bone` (or a copy of it: "bone_2").
pub fn is_bone(name: &str, bone: &str) -> bool {
    name == bone || name.strip_prefix(bone).is_some_and(|r| r.starts_with('_') && r[1..].chars().all(|c| c.is_ascii_digit()))
}

/// An attachment's mesh on a drawn or holstered gun.
#[derive(Component)]
pub struct AttachmentModel;

/// The attachments a gun model shows (item ids, in order).
#[derive(Component, Default, PartialEq)]
pub struct ShownAttachments(pub Vec<String>);

/// ProcessAttachments: each attachment's Local_ActorClass spawned and snapped to its socket on
/// the gun's mesh (location, rotation, scale), and destroyed when it comes off.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show_attachments(
    mut commands: Commands,
    db: Res<ItemDb>,
    game: Res<crate::menus::CurrentGame>,
    assets: Res<AssetServer>,
    guns: Query<(Entity, &super::combat::HeldWeapon, Option<&ShownAttachments>)>,
    children: Query<&Children>,
    names: Query<&Name>,
    models: Query<Entity, With<AttachmentModel>>,
) {
    let Some(inv) = game.inventory.as_ref() else { return };
    for (gun, held, shown) in &guns {
        let Some(stack) = inv.equipped.get(&held.0) else { continue };
        let want = ShownAttachments(stack.attachments.iter().map(|a| a.item.clone()).collect());
        if shown == Some(&want) {
            continue;
        }
        let places: Vec<_> = want.0.iter().filter_map(|a| placement(&db, &stack.item, a)).collect();
        // Every bone has to be in (the gun's scene still loading otherwise).
        let mut parents = Vec::new();
        for (bone, _, _) in &places {
            let parent = match bone {
                None => Some(gun),
                Some(b) => children.iter_descendants(gun).find(|&c| names.get(c).is_ok_and(|n| is_bone(n.as_str(), b))),
            };
            let Some(parent) = parent else { break };
            parents.push(parent);
        }
        if parents.len() < places.len() {
            continue;
        }
        for e in children.iter_descendants(gun).filter(|&c| models.contains(c)).collect::<Vec<_>>() {
            commands.entity(e).despawn();
        }
        for (((_, local, mesh), parent), item) in places.into_iter().zip(parents).zip(want.0.iter().filter(|a| placement(&db, &stack.item, a).is_some())) {
            let e = commands.spawn((crate::models::model(&assets, format!("game/{mesh}.glb")), local, AttachmentModel)).id();
            commands.entity(parent).add_child(e);
            // The tactical lights' SpotLights (`tactical.rs` sets their intensity), along UE +X.
            let lights = db.items.get(item).and_then(|d| d.attachment.as_ref()?.actor.as_ref()).map_or(&[][..], |a| &a.lights[..]);
            for l in lights.iter().filter(|l| l.kind == "spot") {
                let mut t = l.transform.to_transform();
                t.rotation *= Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
                commands.spawn((
                    SpotLight { intensity: 0.0, range: l.radius / 100.0, inner_angle: 0.0, outer_angle: l.outer_cone.to_radians(), shadow_maps_enabled: false, ..default() },
                    t,
                    super::tactical::TacticalBeam { slot: held.0, item: item.clone() },
                    ChildOf(e),
                ));
            }
        }
        commands.entity(gun).insert(want);
    }
}

/// IA_ScopeZoom while aiming; letting go of the aim drops the zoom. With AUTOMATICALLY USE SCOPE
/// WHEN AIMING? (Character.AutoScope) the aim's start does what the button does.
pub fn scope_zoom(
    mouse: Res<ButtonInput<MouseButton>>,
    armed: Res<super::combat::Armed>,
    game: Res<crate::menus::CurrentGame>,
    db: Res<ItemDb>,
    store: Res<crate::menus::settings::SettingsStore>,
    mut zoom: ResMut<Zoom>,
    mut was_aiming: Local<bool>,
) {
    let started = armed.aiming && !*was_aiming;
    *was_aiming = armed.aiming;
    if !armed.aiming {
        if zoom.0.is_some() {
            zoom.0 = None;
        }
        return;
    }
    let auto = started && store.find(&crate::menus::settings::row_key("settings", "Gameplay", "AUTOMATICALLY USE SCOPE WHEN AIMING?")) == Some(crate::menus::settings::Value::Bool(true));
    if !mouse.just_pressed(MouseButton::Middle) && !auto {
        return;
    }
    let weapon = armed.drawn.and_then(|s| game.inventory.as_ref()?.equipped.get(&s)).and_then(|w| self::zoom(&db, w));
    zoom.0 = match (zoom.0, weapon) {
        (Some(_), _) | (None, None) => None,
        (None, Some(z)) => Some(z),
    };
    info!("attachments: scope zoom {:?}", zoom.0);
}

/// The camera's horizontal field of view: the setting, or the zoom.
fn camera_fov(
    zoom: Res<Zoom>,
    binos: Res<super::binoculars::Binoculars>,
    store: Res<crate::menus::settings::SettingsStore>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<&mut Projection, With<super::PlayerCamera>>,
) {
    let Ok(window) = windows.single() else { return };
    let base = match store.find(&crate::menus::settings::row_key("settings", "Video", "THIRD PERSON FIELD OF VIEW")) {
        Some(crate::menus::settings::Value::Num(v)) => v,
        _ => 90.0,
    };
    // Binoculars: FirstPersonCamera's FIRST PERSON FIELD OF VIEW over the zoom.
    let h = if binos.active { super::binoculars::first_person_fov(&store) / binos.zoom.max(1e-3) } else { zoom.0.map_or(base, |z| z.0) };
    let v = vertical_fov(h, window.width() / window.height().max(1.0));
    for mut p in &mut cameras {
        if let Projection::Perspective(pp) = &mut *p
            && (pp.fov - v).abs() > 1e-4
        {
            pp.fov = v;
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
    fn attachments_fit_their_slots_once() {
        let Some(db) = db() else { return };
        let mut m4 = db.stack("DA_M4CQB", 1);
        m4.attachments.clear();
        assert_eq!(slot_for(&db, "DA_M4CQB", "DA_MilitarySuppressor"), Some("Muzzle"));
        assert_eq!(slot_for(&db, "DA_M4CQB", "DA_556Mag"), None, "magazines have their own code");
        assert!(attach(&db, &mut m4, db.stack("DA_MilitarySuppressor", 1)).is_ok());
        assert!(attach(&db, &mut m4, db.stack("DA_MuzzleBrake", 1)).is_err(), "one muzzle");
        assert!(attach(&db, &mut m4, db.stack("DA_VerticalForegrip", 1)).is_ok());
        assert!(attach(&db, &mut m4, db.stack("DA_Acog4x32Scope", 1)).is_ok());
        assert!(attach(&db, &mut m4, db.stack("DA_HuntingScope", 1)).is_err(), "not on this gun");
        assert_eq!(removable(&db, &m4).len(), 3);
        // Recoil control: the grip's -30 and the scope's 0 -> 0.7; the suppressor's falloff.
        assert!((recoil_strength(&db, &m4, "VerticalRecoilControl") - 0.7).abs() < 1e-5);
        assert_eq!(recoil_strength(&db, &m4, "HorizontalRecoilControl"), 1.0);
        assert!((falloff_divisor(&db, &m4) - 1.15).abs() < 1e-5);
        assert_eq!(zoom(&db, &m4), Some((25.0, Some(Scope::Acog))));
        // The suppressor: quiet, and it wears.
        let m = muzzle(&db, &mut m4, Some(1.0));
        assert!(m.suppressed && (m.noise - 0.15).abs() < 1e-5, "{m:?}");
        assert!((in_slot(&db, &m4, "Muzzle").unwrap().durability.unwrap() - (100.0 - 0.525)).abs() < 1e-4);
        assert_eq!(hip_cone(&db, &m4, true), HIP_CONE);
        assert_eq!(hip_cone(&db, &m4, false), 0.0);
    }

    #[test]
    fn attachments_hang_on_their_sockets() {
        let Some(db) = db() else { return };
        // The ACOG on the M4CQB's Sight socket, scaled by its StaticMesh's 0.85.
        let (bone, tf, mesh) = placement(&db, "DA_M4CQB", "DA_Acog4x32Scope").unwrap();
        assert_eq!(bone, Some("SM_Wep_Mod_Rail_01"));
        assert!(mesh.ends_with("SM_Wep_Mod_Scope_02"));
        assert!((tf.scale - Vec3::splat(0.85)).abs().max_element() < 1e-4, "{tf:?}");
        // The socket's UE (0, -0.96, -5.25) cm is glb (0, -0.0525, -0.0096) m.
        assert!((tf.translation - Vec3::new(0.0, -0.052534, -0.00961)).abs().max_element() < 1e-4, "{tf:?}");
        // The Cerberus has no RangeFinder socket: the mesh's origin.
        assert_eq!(placement(&db, "DA_Cerberus", "DA_RangeFinder").map(|p| p.0), Some(None));
        // Magazines hang there too.
        assert!(placement(&db, "DA_M4CQB", "DA_556Mag").is_some_and(|p| p.0.is_some()));
    }

    #[test]
    fn bare_guns_and_worn_muzzles() {
        let Some(db) = db() else { return };
        let mut m4 = db.stack("DA_M4CQB", 1);
        m4.attachments.clear();
        // No muzzle: the 1.0 fallback still divides the range by 1.01.
        assert!((falloff_divisor(&db, &m4) - 1.01).abs() < 1e-5);
        assert_eq!(muzzle(&db, &mut m4, Some(1.0)), Muzzle { suppressed: false, noise: 1.0 });
        assert_eq!(zoom(&db, &m4), None);
        let mut s = db.stack("DA_MilitarySuppressor", 1);
        s.durability = Some(0.0);
        attach(&db, &mut m4, s).unwrap();
        assert_eq!(muzzle(&db, &mut m4, Some(1.0)), Muzzle { suppressed: false, noise: 1.0 }, "worn out");
        assert!(muzzle(&db, &mut m4, None).suppressed, "durability off");
        // A laser: the hip cone narrows.
        attach(&db, &mut m4, db.stack("DA_TacticalLaserLightCombo", 1)).unwrap();
        assert!((hip_cone(&db, &m4, true) - 3.0 / 1.9).abs() < 1e-5);
        // ScopesUI's overlay by the scope's ItemID; built-in scopes.
        assert_eq!(scope_overlay("PUScope"), Scope::Pu);
        assert_eq!(scope_overlay("LechieAcog"), Scope::Acog);
        assert_eq!(scope_overlay("HuntingScope"), Scope::Long);
        assert_eq!(zoom(&db, &db.stack("DA_Cerberus", 1)), Some((15.0, Some(Scope::Long))));
        assert_eq!(zoom(&db, &db.stack("DA_XM250", 1)), Some((20.0, Some(Scope::Acog))));
        // 90 degrees across at 16:9.
        assert!((vertical_fov(90.0, 16.0 / 9.0).to_degrees() - 58.7155).abs() < 1e-3);
    }
}
