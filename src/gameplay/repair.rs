//! Item repairs and the other item-over-item options (JigSInventory's JigContextMenuComp
//! ContextRepairWeapon / ContextRepairAttachment / ContextRepairArmor and JigItemOverItemComp's
//! GetValid / WeaponRepair / ArmorRepair / RefillRadFilter / PurifyWater).
//!
//! Repairs: an item whose durability isn't its MaxDurability takes
//! DurToFix = FClamp(Max - Durability, 0, kit durability), clamped to Max, and the kit loses
//! DurToFix / ReduceDurabilityAmount (the Weapon Cleaning Kit and Armor Repair Kit 2, the
//! makeshift ones 1; an attachment halves that first). A kit at <= 1 durability is consumed.
//! - The context menu ("Repair Weapon" / "Repair Attachment" / "Repair Gear", from the item's
//!   Jig.Context.WeaponRepair / AttachmentRepair / ArmorRepair): FindJigItemByDA in the item's own
//!   inventory (ContainerMother), the proper kit first, then the makeshift one; none = "No Weapon
//!   Cleaning Kits found in this inventory" / "No Armor repair Kits in this inventory".
//! - Dropping a kit onto an item (its OverItemOptions: WeaponRepair + AttachmentRepair, or
//!   ArmorRepair), the first option GetValid accepts.
//!
//! Charcoal Tablets onto a used Radiation Filter (RefillRadFilter): back to full, one tablet used.
//! Water Purification Tablets onto dirty water (PurifyWater): the bottle / canteen / jug needs
//! 1 / 3 / 5 tablets ("{Count} water purification tablets required"); one dirty item becomes a
//! clean one, in its own slot when that was the last of its stack, else anywhere.
//!
//! Ours: GetValid's item-type tag containers aren't readable in the cooked BP (they print as
//! empty), so a receiver qualifies by its own repair context tag (58 weapons, 15 attachments,
//! 20 armor pieces carry them; WeaponRepair covers exactly the MainFirearm / Sidearm / Melee
//! types). "Item Repaired" is a development PrintString in the game, so nothing is shown.
//! Not ported: DurabilityUI / FilterDurabilityUI, which nothing in the pak creates.

use super::items::{ItemDb, Stack};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepairKind {
    Weapon,
    Attachment,
    Armor,
}

impl RepairKind {
    pub const ALL: [RepairKind; 3] = [RepairKind::Weapon, RepairKind::Attachment, RepairKind::Armor];

    /// The Jig.Context / Jig.ItemOverItem tag's last part.
    pub fn tag(self) -> &'static str {
        match self {
            RepairKind::Weapon => "WeaponRepair",
            RepairKind::Attachment => "AttachmentRepair",
            RepairKind::Armor => "ArmorRepair",
        }
    }

    /// The context menu's entry (JigContextMenuW: REPAIR WEAPON / REPAIR ATTACHMENT / REPAIR GEAR).
    pub fn label(self) -> &'static str {
        match self {
            RepairKind::Weapon => "Repair Weapon",
            RepairKind::Attachment => "Repair Attachment",
            RepairKind::Armor => "Repair Gear",
        }
    }

    /// FindJigItemByDA's order: the proper kit, then the makeshift one.
    pub fn kits(self) -> [&'static str; 2] {
        match self {
            RepairKind::Weapon | RepairKind::Attachment => ["DA_WeaponCleaningKit", "DA_MakeshiftWeaponCleaningKit"],
            RepairKind::Armor => ["DA_ArmorRepairKit", "DA_MakeshiftArmorRepairKit"],
        }
    }

    pub fn no_kit(self) -> &'static str {
        match self {
            RepairKind::Weapon | RepairKind::Attachment => "No Weapon Cleaning Kits found in this inventory",
            RepairKind::Armor => "No Armor repair Kits in this inventory",
        }
    }

    /// The kit's cost divisor: SelectFloat(2, 1, Attachment?).
    fn divisor(self) -> f32 {
        if self == RepairKind::Attachment { 2.0 } else { 1.0 }
    }
}

/// GetDurability: (Durability, MaxDurability); `None` on the stack is full.
pub fn durability(db: &ItemDb, stack: &Stack) -> (f32, f32) {
    let max = db.items.get(&stack.item).map_or(0.0, |d| d.durability);
    (stack.durability.unwrap_or(max), max)
}

/// The repair kinds an item's context menu offers.
pub fn context_kinds(db: &ItemDb, item: &str) -> Vec<RepairKind> {
    let Some(def) = db.items.get(item) else { return Vec::new() };
    RepairKind::ALL.into_iter().filter(|k| def.context.iter().any(|c| c == k.tag())).collect()
}

/// Repair `item` with `kit` (both changed). Returns whether the kit is used up (<= 1 left), or
/// `None` when the item was already at full durability (nothing happens).
pub fn fix(db: &ItemDb, kind: RepairKind, item: &mut Stack, kit: &mut Stack) -> Option<bool> {
    let (now, max) = durability(db, item);
    if now == max {
        return None;
    }
    let (kit_now, _) = durability(db, kit);
    let reduce = db.items.get(&kit.item).map_or(1.0, |d| d.reduce_durability);
    let to_fix = (max - now).clamp(0.0, kit_now.max(0.0));
    item.durability = Some((now + to_fix).clamp(0.0, max));
    let kit_left = kit_now - to_fix / kind.divisor() / reduce;
    kit.durability = Some(kit_left);
    Some(kit_left <= 1.0)
}

/// What dropping `dropped` onto `receiver` does (GetValid: the dropped item's OverItemOptions in
/// order, the first that fits). The Battery option is `inventory_ui::item_over_item`'s own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverItem {
    Repair(RepairKind),
    RefillRadFilter,
    PurifyWater,
}

pub fn over_item(db: &ItemDb, dropped: &Stack, receiver: &Stack) -> Option<OverItem> {
    let def = db.items.get(&dropped.item)?;
    let rdef = db.items.get(&receiver.item)?;
    let (now, max) = durability(db, receiver);
    let worn = now != max;
    let has = |tag: &str| rdef.context.iter().any(|c| c == tag);
    def.over_item.iter().find_map(|opt| match opt.as_str() {
        "WeaponRepair" if worn && has("WeaponRepair") => Some(OverItem::Repair(RepairKind::Weapon)),
        "AttachmentRepair" if worn && has("AttachmentRepair") && !matches!(rdef.id.as_str(), "MakeshiftMuffler" | "MakeshiftMuffler2") => {
            Some(OverItem::Repair(RepairKind::Attachment))
        }
        "ArmorRepair" if worn && rdef.armor && has("ArmorRepair") => Some(OverItem::Repair(RepairKind::Armor)),
        "RefillRadFilter" if worn && receiver.item == "DA_RadiationFilter" => Some(OverItem::RefillRadFilter),
        "PurifyWater" if purified(&receiver.item).is_some() => Some(OverItem::PurifyWater),
        _ => None,
    })
}

/// PurifyWater's switch: the clean item and the tablets it takes.
pub fn purified(item: &str) -> Option<(&'static str, u32)> {
    match item {
        "DA_DirtyWaterBottle" => Some(("DA_WaterBottle", 1)),
        "DA_DirtyWaterCanteen" => Some(("DA_WaterCanteen", 3)),
        "DA_DirtyWaterJug" => Some(("DA_WaterJug", 5)),
        _ => None,
    }
}

/// "{Count} water purification tablets required" (the game's plural for any count).
pub fn tablets_needed(n: u32) -> &'static str {
    match n {
        1 => "1 water purification tablets required",
        3 => "3 water purification tablets required",
        _ => "5 water purification tablets required",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Option<ItemDb> {
        serde_json::from_str(&std::fs::read_to_string("assets/data/items.json").ok()?).ok()
    }

    #[test]
    fn kits_repair_and_wear_out() {
        let Some(db) = db() else { return };
        let mut rifle = db.stack("DA_M4CQB", 1);
        let mut kit = db.stack("DA_WeaponCleaningKit", 1);
        assert_eq!(fix(&db, RepairKind::Weapon, &mut rifle, &mut kit), None, "full: nothing happens");
        rifle.durability = Some(40.0);
        // 60 fixed for 60 / 2 of the kit.
        assert_eq!(fix(&db, RepairKind::Weapon, &mut rifle, &mut kit), Some(false));
        assert_eq!((rifle.durability, kit.durability), (Some(100.0), Some(70.0)));
        // The makeshift kit costs twice as much; an attachment halves it.
        let mut grip = db.stack("DA_MilitarySuppressor", 1);
        grip.durability = Some(20.0);
        let mut makeshift = db.stack("DA_MakeshiftWeaponCleaningKit", 1);
        assert_eq!(fix(&db, RepairKind::Attachment, &mut grip, &mut makeshift), Some(false));
        assert_eq!((grip.durability, makeshift.durability), (Some(100.0), Some(60.0)));
        // A kit can't fix more than it has left, and is used up at <= 1.
        rifle.durability = Some(0.0);
        makeshift.durability = Some(30.0);
        assert_eq!(fix(&db, RepairKind::Weapon, &mut rifle, &mut makeshift), Some(true));
        assert_eq!(rifle.durability, Some(30.0));
    }

    #[test]
    fn kits_fit_their_items() {
        let Some(db) = db() else { return };
        let kit = db.stack("DA_WeaponCleaningKit", 1);
        let armor_kit = db.stack("DA_ArmorRepairKit", 1);
        let mut rifle = db.stack("DA_M4CQB", 1);
        assert_eq!(over_item(&db, &kit, &rifle), None, "full durability");
        rifle.durability = Some(50.0);
        assert_eq!(over_item(&db, &kit, &rifle), Some(OverItem::Repair(RepairKind::Weapon)));
        assert_eq!(over_item(&db, &armor_kit, &rifle), None);
        let mut grip = db.stack("DA_MilitarySuppressor", 1);
        grip.durability = Some(50.0);
        assert_eq!(over_item(&db, &kit, &grip), Some(OverItem::Repair(RepairKind::Attachment)));
        let mut muffler = db.stack("DA_MakeshiftMuffler", 1);
        muffler.durability = Some(50.0);
        assert_eq!(over_item(&db, &kit, &muffler), None, "no AttachmentRepair tag (and GetValid names them out)");
        let mut vest = db.stack("DA_BodyArmor", 1);
        vest.durability = Some(50.0);
        assert_eq!(over_item(&db, &armor_kit, &vest), Some(OverItem::Repair(RepairKind::Armor)));
        assert_eq!(over_item(&db, &kit, &vest), None);
        assert_eq!(context_kinds(&db, "DA_BodyArmor"), vec![RepairKind::Armor]);
        assert_eq!(context_kinds(&db, "DA_M4CQB"), vec![RepairKind::Weapon]);
        let mut filter = db.stack("DA_RadiationFilter", 1);
        let charcoal = db.stack("DA_CharcoalTablets", 1);
        assert_eq!(over_item(&db, &charcoal, &filter), None);
        filter.durability = Some(10.0);
        assert_eq!(over_item(&db, &charcoal, &filter), Some(OverItem::RefillRadFilter));
        let tablets = db.stack("DA_WaterPurificationTablets", 1);
        assert_eq!(over_item(&db, &tablets, &db.stack("DA_DirtyWaterJug", 1)), Some(OverItem::PurifyWater));
        assert_eq!(over_item(&db, &tablets, &db.stack("DA_WaterJug", 1)), None);
        for (dirty, (clean, _)) in ["DA_DirtyWaterBottle", "DA_DirtyWaterCanteen", "DA_DirtyWaterJug"].map(|d| (d, purified(d).unwrap())) {
            assert!(db.items.contains_key(dirty) && db.items.contains_key(clean));
        }
        for kind in RepairKind::ALL {
            assert!(kind.kits().iter().all(|k| db.items[*k].over_item.iter().any(|o| o == kind.tag())));
        }
    }
}
