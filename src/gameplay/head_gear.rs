//! Head gear on the survivor, as `BP_PlayerCharacter` wears it (decompiled):
//!
//! - Equipping into FACEWEAR / HEADWEAR / EYEWEAR / ACCESSORY (JigMP_OnMainContainerItemAdded)
//!   spawns the item's PickupClass actor and sets `<Slot>Equipped?`, whose OnRep calls
//!   EquipActorToSocket: the actor snapped to DA.PrimaryUnequipSocket (PrimaryUnequipSocketBackpack
//!   while a backpack is worn) and moved to DA.EquippedTransform relative to it. The actor is the
//!   pickup Blueprint, a StaticMeshComponent with the pickup's mesh and OverrideMaterials
//!   (items.json `worn`; the cowboy hat's mesh isn't its inspect mesh). Hats sit on the
//!   `Headwear` socket, masks on `Mask`, glasses on `Eyewear`, scarves on `Neck` / `Scarf`, the
//!   headset on `Head` (no such socket: UE takes the `head` bone, names ignoring case).
//! - OnRep_HeadwearEquipped?: the HairMesh hidden in game unless HIDE PLAYER HEADWEAR?
//!   (Character.HideHeadWear); taking the hat off shows the hair again. OnRep_FacewearEquipped?:
//!   the BeardMesh hidden while a mask is worn.
//! - SetFPMeshesVisibility, third person: the headwear shown unless HIDE PLAYER HEADWEAR? (the hair
//!   shows instead); facewear, eyewear and accessories shown.
//! - BP_Barber's SetPlayerGearVisibility hides the headwear and facewear in the chair (and sets the
//!   hair / beard Visibility, which their hidden-in-game flag still overrides: a hat keeps the
//!   hair hidden there too).
//!
//! Ours: third person only (first person hides the headwear, facewear and whatever hangs on the
//! helmet); the setting applies at once (the game's OnRep leaves a hat put on with it ticked
//! showing until the view changes); leaving the barber doesn't SetVisibility(false) the hair and
//! beard (in the game they then stay invisible after the hat or mask comes off, until a reload).
//! Not ported: actors hung on the helmet (NVGs), turning night vision off with the hat.

use bevy::prelude::*;

use super::equipment::{PlayerInventory, Slot};
use super::interact::{GameOverlay, player_inv};
use super::items::ItemDb;
use crate::level::KinematicBody;
use crate::menus::settings::{SettingsStore, Value, row_key};
use crate::menus::{AppState, CurrentGame};
use crate::models::{AttachToBone, CharacterModel, FaceMesh, HeadPart, MaterialOverride, model};

/// The slots whose items hang on the head and neck.
pub const GEAR_SLOTS: [Slot; 4] = [Slot::Headwear, Slot::Facewear, Slot::Eyewear, Slot::Accessory];

/// A worn item's actor on the survivor.
#[derive(Component)]
pub struct HeadGear(pub Slot);

/// What shows in third person: the gear slots drawn, and whether the hair / beard are hidden.
#[derive(Debug, Default, PartialEq)]
pub struct Look {
    pub shown: Vec<Slot>,
    pub hair_hidden: bool,
    pub beard_hidden: bool,
}

/// SetFPMeshesVisibility (third person), the OnReps' hair / beard and the barber's chair.
pub fn look(worn: &[Slot], hide_headwear: bool, in_barber: bool) -> Look {
    let hat = worn.contains(&Slot::Headwear);
    let shown = worn
        .iter()
        .copied()
        .filter(|s| match s {
            Slot::Headwear => !hide_headwear && !in_barber,
            Slot::Facewear => !in_barber,
            _ => true,
        })
        .collect();
    Look { shown, hair_hidden: hat && !hide_headwear, beard_hidden: worn.contains(&Slot::Facewear) }
}

/// Where an equipped item hangs: the bone, its transform under it (socket, EquippedTransform,
/// the mesh in the actor), the mesh and its override material.
#[derive(Debug, Clone, PartialEq)]
pub struct Hang {
    pub bone: String,
    pub local: Transform,
    pub mesh: String,
    pub material: Option<String>,
}

pub fn hang(db: &ItemDb, inv: &PlayerInventory, slot: Slot) -> Option<Hang> {
    let def = db.items.get(&inv.equipped.get(&slot)?.item)?;
    let worn = def.worn.as_ref()?;
    let e = def.equip.clone().unwrap_or_default();
    let socket = if inv.equipped.contains_key(&Slot::Backpack) { e.primary_unequip_socket_backpack.or(e.primary_unequip_socket) } else { e.primary_unequip_socket }?;
    // A name that isn't a socket is a bone.
    let (bone, at) = db.sockets.get(&socket).map_or_else(|| (socket.to_lowercase(), Transform::IDENTITY), |s| (s.bone.clone(), s.transform.to_transform()));
    let offset = e.transform.map_or(Transform::IDENTITY, |t| t.to_transform());
    // The gear's override materials are all on single-material meshes (a test checks).
    let material = worn.materials.iter().flatten().next().map(|m| m.rsplit('/').next().unwrap_or(m).to_string());
    Some(Hang { bone, local: at * offset * worn.transform.to_transform(), mesh: worn.mesh.clone(), material })
}

fn hide_headwear(store: &SettingsStore) -> bool {
    store.find(&row_key("settings", "Gameplay", "HIDE PLAYER HEADWEAR?")) == Some(Value::Bool(true))
}

/// Bone names match ignoring case (FName); body parts' skeleton copies may carry a `_N` suffix.
fn is_bone(name: &str, bone: &str) -> bool {
    name.eq_ignore_ascii_case(bone) || name.get(..bone.len()).is_some_and(|h| h.eq_ignore_ascii_case(bone)) && name[bone.len()..].strip_prefix('_').is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()))
}

/// The survivor's hair and beard (not those still waiting for their bone).
type FaceParts<'w, 's> = Query<'w, 's, (&'static FaceMesh, &'static mut Visibility), (Without<AttachToBone>, Without<HeadGear>)>;

/// Hang the worn gear on its sockets and show / hide it, the hair and the beard.
#[allow(clippy::too_many_arguments)]
fn wear_head_gear(
    mut commands: Commands,
    db: Res<ItemDb>,
    assets: Res<AssetServer>,
    store: Res<SettingsStore>,
    overlay: Res<GameOverlay>,
    mut game: ResMut<CurrentGame>,
    player: Query<Entity, (With<CharacterModel>, With<KinematicBody>)>,
    heads: Query<(Entity, &ChildOf), With<HeadPart>>,
    tree: (Query<&Children>, Query<&Name>),
    mut gear: Query<(Entity, &HeadGear, &mut Visibility), Without<FaceMesh>>,
    mut faces: FaceParts,
    mut last: Local<Vec<(Slot, Hang)>>,
) {
    let (children, names) = tree;
    let Ok(me) = player.single() else { return };
    let Some(head) = heads.iter().find(|(_, p)| p.parent() == me).map(|(h, _)| h) else { return };
    let inv = player_inv(&mut game);
    let want: Vec<(Slot, Hang)> = GEAR_SLOTS.iter().filter_map(|&s| Some((s, hang(&db, inv, s)?))).collect();
    let worn: Vec<Slot> = want.iter().map(|(s, _)| *s).collect();
    let l = look(&worn, hide_headwear(&store), matches!(*overlay, GameOverlay::Barber(_)));
    // Respawn on a change, or when a gear piece went with its bone (the barber's new head).
    if *last != want || gear.iter().count() != last.len() {
        // The Head's skeleton first (it outlives clothing swaps), then any body part's.
        let bone_of = |bone: &str| {
            let found = |root: Entity| children.iter_descendants(root).find(|&c| names.get(c).is_ok_and(|n| is_bone(n.as_str(), bone)));
            found(head).or_else(|| found(me))
        };
        let mut spawn = Vec::new();
        for (slot, h) in &want {
            let Some(bone) = bone_of(&h.bone) else { return }; // still loading
            spawn.push((bone, *slot, h));
        }
        for (e, _, _) in &gear {
            commands.entity(e).despawn();
        }
        for (bone, slot, h) in spawn {
            let mut e = commands.spawn((model(&assets, format!("game/{}.glb", h.mesh)), h.local, HeadGear(slot), Visibility::Hidden));
            if let Some(m) = &h.material {
                e.insert(MaterialOverride(m.clone()));
            }
            let e = e.id();
            commands.entity(bone).add_child(e);
        }
        info!("head gear: {:?}", want.iter().map(|(s, h)| format!("{s:?} {} on {}", h.mesh.rsplit('/').next().unwrap_or(""), h.bone)).collect::<Vec<_>>());
        *last = want;
        return;
    }
    for (_, g, mut vis) in &mut gear {
        let v = if l.shown.contains(&g.0) { Visibility::Inherited } else { Visibility::Hidden };
        vis.set_if_neq(v);
    }
    for e in children.iter_descendants(head) {
        let Ok((face, mut vis)) = faces.get_mut(e) else { continue };
        let hidden = match face {
            FaceMesh::Hair => l.hair_hidden,
            FaceMesh::Beard => l.beard_hidden,
        };
        vis.set_if_neq(if hidden { Visibility::Hidden } else { Visibility::Inherited });
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, wear_head_gear.run_if(in_state(AppState::InGame)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Option<ItemDb> {
        Some(serde_json::from_str(&std::fs::read_to_string("assets/data/items.json").ok()?).unwrap())
    }

    #[test]
    fn hats_hide_the_hair_and_masks_the_beard() {
        let all = [Slot::Headwear, Slot::Facewear, Slot::Eyewear, Slot::Accessory];
        assert_eq!(look(&all, false, false), Look { shown: all.to_vec(), hair_hidden: true, beard_hidden: true });
        // HIDE PLAYER HEADWEAR?: no hat, the hair back.
        assert_eq!(look(&all, true, false), Look { shown: all[1..].to_vec(), hair_hidden: false, beard_hidden: true });
        // In the barber's chair the hat and mask come off, but the hair and beard stay hidden.
        assert_eq!(look(&all, false, true), Look { shown: all[2..].to_vec(), hair_hidden: true, beard_hidden: true });
        assert_eq!(look(&[Slot::Eyewear], false, false), Look { shown: vec![Slot::Eyewear], ..default() });
    }

    #[test]
    fn gear_hangs_on_the_head_and_neck() {
        let Some(db) = db() else { return };
        let mut inv = PlayerInventory::default();
        let wear = |inv: &mut PlayerInventory, slot, item: &str| {
            inv.equip(slot, db.stack(item, 1));
            hang(&db, inv, slot).unwrap()
        };
        // The cap on the Headwear socket of `head`, as the socket sits.
        let cap = wear(&mut inv, Slot::Headwear, "DA_Cap");
        assert_eq!(cap.bone, "head");
        assert!(cap.local.rotation.abs_diff_eq(db.sockets["Headwear"].transform.to_transform().rotation, 1e-4), "{cap:?}");
        assert!(cap.mesh.ends_with("SM_Chr_Attach_Hat_01") && cap.material.is_none());
        // The second cap is the same hat re-coloured.
        assert_eq!(wear(&mut inv, Slot::Headwear, "DA_Cap2").material.as_deref(), Some("Mat_PolygonConstruction_02_A"));
        // The cowboy hat wears its pickup's PolygonMilitary mesh, not the inspect one.
        assert!(wear(&mut inv, Slot::Headwear, "DA_CowboysHat").mesh.contains("PolygonMilitary"));
        // The respirator 8 cm out along the Mask socket's offset; the headset on the bone.
        let mask = wear(&mut inv, Slot::Facewear, "DA_EnhancedMilitaryRespirator");
        assert_eq!(mask.bone, "head");
        assert!(mask.local.translation.length() > 0.07, "{mask:?}");
        assert_eq!(wear(&mut inv, Slot::Accessory, "DA_MilitaryTacticalHeadset").bone, "head");
        assert_eq!(wear(&mut inv, Slot::Accessory, "DA_Shemagh").bone, "neck_01");
        // Bones match ignoring case and a copy's suffix.
        assert!(is_bone("head", "Head") && is_bone("head_2", "head") && !is_bone("head_r", "head") && !is_bone("headx", "head"));
    }

    #[test]
    fn every_piece_of_head_gear_can_be_worn() {
        let Some(db) = db() else { return };
        if !std::path::Path::new("assets/game/materials.json").exists() {
            return;
        }
        let materials: serde_json::Value = serde_json::from_str(&std::fs::read_to_string("assets/game/materials.json").unwrap()).unwrap();
        let mut n = 0;
        for (id, def) in db.items.iter().filter(|(_, d)| GEAR_SLOTS.iter().any(|s| s.fits(&d.kind))) {
            let mut inv = PlayerInventory::default();
            inv.equip(inv.slot_for(&db, id).unwrap(), db.stack(id, 1));
            let h = hang(&db, &inv, inv.slot_for(&db, id).unwrap()).unwrap_or_else(|| panic!("{id} hangs nowhere"));
            assert!(matches!(h.bone.as_str(), "head" | "neck_01"), "{id}: {h:?}");
            let glb = format!("assets/game/{}.glb", h.mesh);
            let Ok(bytes) = std::fs::read(&glb) else { panic!("{id}: {glb} missing") };
            if let Some(m) = &h.material {
                assert!(materials.get(m).is_some(), "{id}: {m} not in materials.json");
                let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
                let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + len]).unwrap();
                assert_eq!(json["materials"].as_array().map(|a| a.len()), Some(1), "{id}: an override on a multi-material mesh");
            }
            assert!(def.worn.as_ref().unwrap().materials.len() <= 1, "{id}");
            n += 1;
        }
        assert_eq!(n, 40, "19 headwear, 13 facewear, 3 eyewear, 5 accessories");
    }
}
