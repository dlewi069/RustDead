#!/usr/bin/env python3
"""Build assets/data/items.json from `SdExport json` dumps of the inventory data.

usage: gen_items.py <json dump root>   (normally run via `export_assets.sh --items`)

Needs dumps of Content/Inventory/Items/DataAssets/ and Content/Inventory/Containers/ plus
JigSInventory/Jigsaw/Data/DataAsset/JigsawItem_DataAsset (class defaults). Writes:

  items        DA name -> name, type tag, grid size, weight, stacking, price, consumable stats,
               inspect mesh (the game renders inventory icons from this mesh)
  loot_tables  table ID -> [{item, chance %, min, max}]  (DA_LootTable_* / ContainerItems_DataAsset)
  containers   container BP class (as it appears in actors.json) -> display name, loot table ID,
               seconds to hold interact, lockpicking
"""
import json
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
OUT = Path(__file__).resolve().parent.parent / "assets" / "data" / "items.json"


def load(p):
    return json.loads(p.read_text())


def text(t):
    return (t or {}).get("SourceString") or (t or {}).get("CultureInvariantString")


def field(d, prefix):
    """Blueprint struct fields are suffixed with GUIDs: ItemID_41_8C5E..."""
    for k, v in d.items():
        if k.split("_")[0] == prefix or k.startswith(prefix + "_"):
            return v
    return None


def pkg_name(ref):
    return ref["ObjectPath"].rsplit("/", 1)[1].split(".")[0] if ref else None


WEARABLE = {"Backpack", "Torso", "Legs", "BodyArmor", "Container"}


def wearable_grids(p):
    kind = (p.get("Type") or {}).get("TagName", "").rsplit(".", 1)[-1]
    if kind not in WEARABLE or not p.get("IsContainer?"):
        return []
    return [[int(d["X"]), int(d["Y"])] for d in p.get("ContainerDimensions", []) if d.get("X", -1) > 0 and d.get("Y", -1) > 0]


def clothing(p):
    cs = p.get("ClothingSettings") or {}
    mesh = lambda key: next((v["ObjectPath"].rsplit(".", 1)[0] for k, v in cs.items() if k.startswith(key) and v), None)
    male, female = mesh("MaleMesh"), mesh("FemaleMesh")
    return {"male": male, "female": female or male} if male or female else None


def random_stats(p):
    """RandomStats: per-instance stats rolled in [min, max] (weapons, armour), in the asset's
    order: rolls stop after MaxStats successes, so order matters."""
    rs = p.get("RandomStats") or {}
    out = []
    for s in field(rs, "Stats") or []:
        tag = (field(s, "STAT") or {}).get("TagName", "")
        lo, hi = field(s, "RandomMinValue"), field(s, "RandomMaxValue")
        if tag and lo is not None:
            out.append({
                "stat": tag.rsplit(".", 1)[-1],
                "range": [lo, hi if hi is not None else lo],
                "chance": field(s, "Chance") if field(s, "Chance") is not None else 100.0,
                "rounded": bool(field(s, "Rounded?")),
            })
    return out, field(rs, "MaxStats") or len(out)


def attachments(p):
    """DefaultAttachments: what a weapon (or mag) spawns loaded with, by chance."""
    out = []
    for a in p.get("DefaultAttachments") or []:
        item = pkg_name(field(a, "ID"))
        if not item:
            continue
        out.append({
            "item": item,
            "kind": ((field(a, "Type") or {}).get("TagName") or "None").rsplit(".", 1)[-1],
            "chance": field(a, "SpawnChance") or 0.0,
            "min": field(a, "MinCount") or 1,
            "max": field(a, "MaxCount") or 1,
            "inside": [
                {"item": pkg_name(field(i, "ID")), "chance": field(i, "SpawnChance") or 0.0, "min": field(i, "MinCount") or 1, "max": field(i, "MaxCount") or 1}
                for i in field(a, "InContainerItem") or [] if field(i, "ID")
            ],
        })
    return out


def text_stats(p):
    return {field(t, "STAT"): field(t, "Txt") for t in p.get("TextStats") or [] if field(t, "STAT")}


def rotator_quat(r):
    """UE FRotator (degrees) -> FQuat (x, y, z, w), FRotator::Quaternion()."""
    import math
    r = r or {}
    p, y, rl = (math.radians(r.get(k, 0.0)) / 2 for k in ("Pitch", "Yaw", "Roll"))
    sp, cp, sy, cy, sr, cr = math.sin(p), math.cos(p), math.sin(y), math.cos(y), math.sin(rl), math.cos(rl)
    return [cr * sp * sy - sr * cp * cy, -cr * sp * cy - sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy]


def glb_transform(loc, quat, scale=None):
    """UE local transform -> glb space: translation (X, Z, Y)/100, quat (x, z, y, -w)."""
    loc, scale = loc or {}, scale or {}
    x, y, z, w = quat
    return {
        "t": [loc.get("X", 0.0) / 100, loc.get("Z", 0.0) / 100, loc.get("Y", 0.0) / 100],
        "r": [x, z, y, -w],
        "s": [scale.get("X", 1.0), scale.get("Z", 1.0), scale.get("Y", 1.0)],
    }


def equip(p):
    """Where a weapon sits: EquipSocket in the hands, *UnequipSocket(Backpack) when holstered,
    EquippedTransform relative to the socket."""
    t = p.get("EquippedTransform")
    out = {k: p[k] for k in ("EquipSocket", "PrimaryUnequipSocket", "PrimaryUnequipSocketBackpack", "SecondaryUnequipSocket", "SecondaryUnequipSocketBackpack") if p.get(k)}
    if t:
        q = t.get("Rotation", {})
        out["transform"] = glb_transform(t.get("Translation"), [q.get("X", 0.0), q.get("Y", 0.0), q.get("Z", 0.0), q.get("W", 1.0)], t.get("Scale3D"))
    return out or None


def sockets(root):
    """The survivor skeleton's sockets (bone + local transform), for weapons and gear."""
    f = root / "Meshes/PlayerModels/SK_Chr_ToplessMale_01_Skeleton.json"
    if not f.exists():
        return {}
    exports = load(f)
    by_name = {e["Name"]: e for e in exports}
    out = {}
    for e in exports:
        if e["Type"] != "SkeletalMeshSocket":
            continue
        p = e.get("Properties", {})
        if p.get("SocketName") and p.get("BoneName"):
            out[p["SocketName"]] = {"bone": p["BoneName"], **glb_transform(p.get("RelativeLocation"), rotator_quat(p.get("RelativeRotation")), p.get("RelativeScale"))}
    return out


def inspect_quat(t):
    q = (t or {}).get("Rotation") or {"X": 0.0, "Y": 0.0, "Z": 0.0, "W": 1.0}
    return [q.get("X", 0.0), q.get("Z", 0.0), q.get("Y", 0.0), -q.get("W", 1.0)]


def FIREARM_TYPES():
    """Enum_Firearms names -> byte values (the BP switches on the value; names aren't in order)."""
    f = ROOT / "Inventory/Items/Other/Components/Data/Enum_Firearms.json"
    return next((e.get("Names", {}) for e in load(f) if e["Type"] == "UserDefinedEnum"), {}) if f.exists() else {}


def firearm(p):
    """BP_WeaponsPickupComponent on the item's pickup Blueprint (over the component's class
    defaults, walking parent pickups): what BP_FirearmPickup's firing reads."""
    comp_file = ROOT / "Inventory/Items/Other/Components/BP_WeaponsPickupComponent.json"
    if not comp_file.exists():
        return None
    base = next((e.get("Properties", {}) for e in load(comp_file) if e["Name"].startswith("Default__")), {})
    ref = p.get("PickupClass")
    chain = []
    while ref:
        f = ROOT / (ref["ObjectPath"].split(".")[0].removeprefix("SurrounDead/Content/") + ".json")
        if not f.exists():
            break
        exports = load(f)
        comp = next((e.get("Properties", {}) for e in exports if e["Type"] == "BP_WeaponsPickupComponent_C"), None)
        if comp is not None:
            chain.append(comp)
        cls = next((e for e in exports if e["Type"] == "BlueprintGeneratedClass"), None)
        ref = (cls or {}).get("Super") or (cls or {}).get("SuperStruct")
        if ref and "Pickups/" not in ref.get("ObjectPath", ""):
            break
    if not chain:
        return None
    c = dict(base)
    for comp in reversed(chain):
        c.update(comp)
    recoil = c.get("RecoilData") or {}
    return {
        # Enum_Firearms index: picks the headshot multiplier (FirearmTypeMultiplier).
        "type": FIREARM_TYPES().get(c.get("FirearmType", "Enum_Firearms::NewEnumerator0"), 0),
        "automatic": bool(c.get("IsWeaponAutomatic?", False)),
        "malfunction_chance": c.get("FirearmMalfunctionChance", 0.025),
        # Gunshot noise radius (cm in the BP), what alerts the AI.
        "sound_radius": c.get("DefaultSoundRadius", 5000.0) / 100.0,
        "recoil_horizontal": field(recoil, "RecoilHorizontal") or 0.0,
        "recoil_vertical": field(recoil, "RecoilVertical") or 0.0,
        "pellets": c.get("ShotgunPellets", 6),
    }


def item_defaults():
    for e in load(ROOT / "JigSInventory/Jigsaw/Data/DataAsset/JigsawItem_DataAsset.json"):
        if e["Name"].startswith("Default__"):
            return e.get("Properties", {})
    return {}


def main():
    base = item_defaults()
    items = {}
    for f in (ROOT / "Inventory/Items/DataAssets").rglob("*.json"):
        for e in load(f):
            if e["Type"] != "JigsawItem_DataAsset_C":
                continue
            p = {**base, **e.get("Properties", {})}
            dims = p.get("SlotDimensions", {"X": 1, "Y": 1})
            stats = {}
            for s in p.get("ItemStats", []):
                tag = (field(s, "STAT") or {}).get("TagName", "")
                lo, hi = field(s, "MinValue"), field(s, "MaxValue")
                if tag and lo is not None:
                    stats[tag.rsplit(".", 1)[-1]] = [lo, hi if hi is not None else lo]
            mesh = p.get("StaticMeshToInspect") or p.get("SkeletalMeshToInspect")
            rstats, max_stats = random_stats(p)
            items[e["Name"]] = {
                # Per-instance rolls (Jig.Stat.FirearmDamage, FirearmRPM, MeleeDamage, ...).
                "random_stats": rstats,
                "max_stats": max_stats,
                "attachments": attachments(p),
                # TextStats: "Firemodes" ("Semi, Auto"), "Caliber".
                "text_stats": text_stats(p),
                # Magazines: rounds held and which ammo fits.
                "capacity": p.get("ContainerCapacity", 0) if p.get("IsContainer?") and not wearable_grids(p) else 0,
                "ammo": [pkg_name(r) for r in p.get("ContainerOnlySupportedIDs") or [] if r],
                "durability": p.get("MaxDurability", 0.0),
                # Durability lost per shot / swing (ReduceDurability), before the difficulty multiplier.
                "reduce_durability": p.get("ReduceDurabilityAmount", 0.0),
                "firearm": firearm(p) if (p.get("Type") or {}).get("TagName", "").endswith(("MainFirearm", "Sidearm")) else None,
                "equip": equip(p),
                "id": p.get("ItemID", e["Name"]),
                "name": text(p.get("name")) or e["Name"],
                "type": (p.get("Type") or {}).get("TagName", "Jig.ItemType.Other").rsplit(".", 1)[-1],
                "rarity": (p.get("Rarity") or {}).get("TagName", "").rsplit(".", 1)[-1],
                "size": [int(dims.get("X", 1)), int(dims.get("Y", 1))],
                "weight": p.get("Weight", 1.0),
                "max_stack": p.get("MaxStack", 1) if p.get("CanStack?") else 1,
                "price": p.get("Price", 5.0),
                "consumable": bool(p.get("CanBeConsumed?")),
                # MedicalComponent effects using it ends ("Bleed", "Heavy Bleed", "Broken Bone").
                "removes": [str(x) for x in p.get("RemoveMedicalEffects") or []],
                "stats": stats,
                "mesh": mesh["ObjectPath"].rsplit(".", 1)[0] if mesh else None,
                # Inventory icons are renders of the inspect mesh at this rotation (UE quat
                # converted to the glb space: x, z, y, -w) and field of view.
                "inspect_rotation": inspect_quat(p.get("InspectTransform")),
                # Wearable containers (backpacks, clothing pockets, rigs, cases): their grids.
                "grids": wearable_grids(p),
                # Clothing worn on the survivor (skeletal meshes on the shared body skeleton).
                "clothing": clothing(p),
                "fov": p.get("FOV", 30.0),
            }

    tables = {}
    # Container loot tables, plus the AI ones dead zombies/animals carry (AI/AILootTables).
    for f in [*(ROOT / "Inventory/Containers/LootTables").glob("*.json"), *(ROOT / "AI/AILootTables").glob("*.json")]:
        for e in load(f):
            if e["Type"] != "ContainerItems_DataAsset_C":
                continue
            p = e.get("Properties", {})
            rows = field(p.get("Loot Table", {}), "ContainerSettings") or []
            tables[p.get("ID", e["Name"])] = [
                {
                    "item": pkg_name(field(r, "ItemID")),
                    "chance": field(r, "SpawnChance") or 0.0,
                    "min": field(r, "RandomCountMin") or 1,
                    "max": field(r, "RandomCountMax") or 1,
                }
                for r in rows if field(r, "ItemID")
            ]

    # Enum_ContainerLootTables value -> table ID (its display names are the IDs).
    enum = {}
    for e in load(ROOT / "Inventory/Containers/Enum_ContainerLootTables.json"):
        for kv in e.get("Properties", {}).get("DisplayNameMap", []):
            enum[f"Enum_ContainerLootTables::{kv['Key']}"] = text(kv["Value"])

    # Container classes: walk SuperStruct for inherited name / table / hold duration.
    classes = {}
    for f in (ROOT / "Inventory/Containers").rglob("*.json"):
        exports = load(f)
        cls = next((e for e in exports if e["Type"] == "BlueprintGeneratedClass"), None)
        cdo = next((e for e in exports if e["Name"].startswith("Default__")), None)
        if cls and cdo:
            sup = (cls.get("SuperStruct") or {}).get("ObjectName", "")
            classes[cls["Name"]] = {"super": sup.split("'")[1] if "'" in sup else None, "props": cdo.get("Properties", {})}

    def inherited(name, key):
        while name in classes:
            v = classes[name]["props"].get(key)
            if v is not None:
                return v
            name = classes[name]["super"]
        return None

    def is_a(name, base):
        while name:
            if name == base:
                return True
            name = classes.get(name, {}).get("super")
        return False

    containers = {}
    for name in classes:
        if not (is_a(name, "BP_LootContainer_C") or is_a(name, "BP_MASTER_PicklockContainer_C")):
            continue
        # Unset means the enum's default, its first entry ("Ammuniton", sic).
        table = enum.get(inherited(name, "LootTable") or "Enum_ContainerLootTables::NewEnumerator0")
        containers[name] = {
            "name": text(inherited(name, "ContainerName")) or "Loot Container",
            "table": table,
            "hold": inherited(name, "DurationToHold") or 4.0,
            "lockpick": is_a(name, "BP_MASTER_PicklockContainer_C"),
        }

    # DT_CraftingTableTypes: one row per crafting UI ("Player" is the inventory's CRAFTING tab,
    # the rest are workbenches/campfire), each a list of Struct_ItemsToCraft.
    recipes = {}
    for f in ROOT.rglob("DT_CraftingTableTypes.json"):
        for row, v in load(f)[0].get("Rows", {}).items():
            out = []
            for r in field(v, "ItemsToCraft") or []:
                made = field(r, "ItemToCraft")
                item = pkg_name(field(made, "ItemID"))
                if item not in items:
                    continue
                out.append({
                    "item": item,
                    "count": field(made, "Count") or 1,
                    "needs": [[pkg_name(field(n, "ItemID")), field(n, "Count") or 1] for n in field(r, "ItemsNeeded") or [] if field(n, "ItemID")],
                    "time": field(r, "TimeToCraftInSecs") or 0.0,
                })
            recipes[row] = out

    OUT.write_text(json.dumps({"items": items, "loot_tables": tables, "containers": containers, "recipes": recipes, "sockets": sockets(ROOT)}, indent=1, sort_keys=True))
    missing = sorted({c["table"] for c in containers.values() if c["table"] and c["table"] not in tables})
    print(f"items: {len(items)}, loot tables: {len(tables)}, containers: {len(containers)}, recipes: {sum(map(len, recipes.values()))}; unknown tables: {missing}")


main()
