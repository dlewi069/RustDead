#!/usr/bin/env python3
"""Build assets/data/items.json from `SdExport json` dumps of the inventory data.

usage: gen_items.py <json dump root>   (normally run via `export_assets.sh --items`)

Needs dumps of Content/Inventory/Items/DataAssets/ and Content/Inventory/Containers/ plus
JigSInventory/Jigsaw/Data/DataAsset/JigsawItem_DataAsset (class defaults). Writes:

  items        DA name -> name, type tag, grid size, weight, stacking, price, consumable stats,
               inspect mesh (the game renders inventory icons from this mesh)
  loot_tables  table ID -> [{item, chance %, min, max}]  (DA_LootTable_* / ContainerItems_DataAsset)
  containers   container BP class (as it appears in actors.json) -> display name, loot table ID,
               seconds to hold interact, lockpicking, mesh (its StaticMesh component's, for
               containers spawned at runtime)
  unique_loot  DT_UniqueLoot row -> [{item, chance %, min, max}] (sets item spawners add to the
               containers they spawn: the police briefcase's generator key)
"""
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import kismet  # noqa: E402

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


BODY_PARTS = ["Torso", "Arms", "Biceps", "Hands", "Legs", "LowerThighs", "LowerLegs", "Feet"]
CLOTHING_DEFAULTS = None


def clothing_defaults():
    """The ClothingSettings struct's defaults. Unversioned assets leave out fields equal to them,
    so an item's BodyPartSettings is these overlaid with its own (e.g. a shirt that only sets
    Torso/Biceps keeps the default bare Arms)."""
    for f in ROOT.rglob("ClothingSettings.json"):
        for e in load(f):
            if e.get("Type") == "UserDefinedStruct":
                return e.get("DefaultProperties") or {}
    return {}


def clothing(p):
    """The worn mesh per sex, and the body-part meshes BP_PlayerCharacter.BodyPartVisibility swaps
    in while it's worn (MC_AttachClothing passes the item's own ClothingSettings; null hides a
    part). The DT_Clothing path (EquipClothingToMesh) has no callers."""
    global CLOTHING_DEFAULTS
    if CLOTHING_DEFAULTS is None:
        CLOTHING_DEFAULTS = clothing_defaults()
    cs = p.get("ClothingSettings") or {}
    mesh = lambda key: next((v["ObjectPath"].rsplit(".", 1)[0] for k, v in cs.items() if k.startswith(key) and v), None)
    male, female = mesh("MaleMesh"), mesh("FemaleMesh")
    if not (male or female):
        return None
    parts = {**(field(CLOTHING_DEFAULTS, "BodyPartSettings") or {}), **(field(cs, "BodyPartSettings") or {})}
    path = lambda v: v["ObjectPath"].rsplit(".", 1)[0] if v else None
    body = {sex.lower(): {b: path(field(parts, f"{sex}{b}Mesh")) for b in BODY_PARTS} for sex in ("Male", "Female")}
    update_all = field(cs, "UpdateAllBodyParts?")
    if update_all is None:
        update_all = field(CLOTHING_DEFAULTS, "UpdateAllBodyParts?")
    return {"male": male, "female": female or male, "body": body, "update_all": bool(update_all)}


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


def disassembly(p):
    """W_DisassemblyWorkbenchUI: CanBeDisassembled? and DisassemblyItems."""
    items = [
        {"item": pkg_name(field(i, "ItemID")), "chance": field(i, "SpawnChance") or 0.0, "min": field(i, "RandomCountMin") or 1, "max": field(i, "RandomCountMax") or 1}
        for i in p.get("DisassemblyItems") or []
        if field(i, "ItemID")
    ]
    return {"can": bool(p.get("CanBeDisassembled?", True)), "items": items}


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


def weapon_slots(p):
    """The SpecialContainerWidget (SC_<gun>): its JSIContainers in display order, each slot's
    ContainerTag (Jig.AttachmentType.Muzzle / Tactical / Grip / Mag / Sight / Chamber) and
    OnlySupportTheseItems."""
    ref = p.get("SpecialContainerWidget")
    # Only the guns' SC_<gun> (Inventory/Widgets/Firearms); SC_NVG / SC_Respirator are `holds`.
    if not ref or "/Widgets/Firearms/" not in ref["ObjectPath"]:
        return []
    f = ROOT / (ref["ObjectPath"].split(".")[0].removeprefix("SurrounDead/Content/") + ".json")
    if not f.exists():
        return []
    exports = load(f)
    by_name = {e["Name"]: e for e in exports}
    out = []
    # Draw order: the HorizontalBox's Slots, each pointing at its Content by export index.
    panel = next((e for e in exports if e["Type"] in ("HorizontalBox", "VerticalBox") and e.get("Properties", {}).get("Slots")), None)
    order = []
    if panel:
        for sref in panel["Properties"]["Slots"]:
            idx = int(sref["ObjectPath"].rsplit(".", 1)[1])
            slot = exports[idx] if idx < len(exports) else None
            content = (slot or {}).get("Properties", {}).get("Content")
            if content:
                order.append(content["ObjectName"].split(".")[-1].rstrip("'"))
    names = order or [e["Name"] for e in exports if e["Type"] == "JSIContainer_C"]
    for n in names:
        e = by_name.get(n)
        if not e or e["Type"] != "JSIContainer_C":
            continue
        pr = e.get("Properties", {})
        tag = (pr.get("ContainerTag") or {}).get("TagName", "")
        if not tag.startswith("Jig.AttachmentType."):
            continue
        out.append({"tag": tag.rsplit(".", 1)[-1], "items": [pkg_name(r) for r in pr.get("OnlySupportTheseItems") or [] if r]})
    return out


def sc_layout(p):
    """A storage SpecialContainerWidget's layout (Inventory/Widgets/Equipment/*: the backpacks',
    clothes' and cases' SC_ widgets): its boxes, spacers and JSIContainers as a tree, each
    {"v": [..]} / {"h": [..]} (Vertical / HorizontalBox), {"gap": [x, y]} (Spacer) or
    {"grid": i}, "center" on a slot with HAlign_Center. `i` indexes the item's `grids`:
    PreInitSpecialContainer's AllContainers order (from the bytecode: the dump needs
    SD_SCRIPT=1), which is ContainerDimensions' order, not the drawing order."""
    ref = p.get("SpecialContainerWidget")
    if not ref or "/Widgets/Equipment/" not in ref["ObjectPath"] or not wearable_grids(p):
        return None
    f = ROOT / (ref["ObjectPath"].split(".")[0].removeprefix("SurrounDead/Content/") + ".json")
    if not f.exists():
        return None
    exports = load(f)
    by_name = {e["Name"]: e for e in exports}
    content = lambda ref: ref["ObjectName"].rstrip("'").split(".")[-1]
    slot_of = {content(e["Properties"]["Content"]): e["Properties"] for e in exports if "Parent" in e.get("Properties", {}) and "Content" in e["Properties"]}
    code = "\n".join(kismet.e(st) for x in exports for st in x.get("ScriptBytecode") or [])
    m = re.search(r"MakeArray_Array = \[([^\]]*)\]", code)
    order = [n.strip() for n in m[1].split(",")] if m else []

    def node(n):
        e = by_name[n]
        pr = e.get("Properties", {})
        if e["Type"] in ("VerticalBox", "HorizontalBox"):
            kids = [node(content(exports[int(r["ObjectPath"].rsplit(".", 1)[1])]["Properties"]["Content"])) for r in pr.get("Slots", [])]
            out = {"v" if e["Type"] == "VerticalBox" else "h": [k for k in kids if k]}
        elif e["Type"] == "Spacer":
            size = pr.get("Size") or {}
            out = {"gap": [size.get("X", 1.0), size.get("Y", 1.0)]}
        elif e["Type"] == "JSIContainer_C" and n in order:
            out = {"grid": order.index(n)}
        else:
            return None
        if slot_of.get(n, {}).get("HorizontalAlignment") == "EHorizontalAlignment::HAlign_Center":
            out["center"] = True
        return out

    roots = [e["Name"] for e in exports if e["Type"] in ("VerticalBox", "HorizontalBox") and e["Name"] not in slot_of]
    return node(roots[0]) if len(roots) == 1 and order else None


def holds(p):
    """What the item's SpecialContainerWidget takes outside the weapon slots: SC_Battery's
    BatteryContainer (flashlights, tactical lights, NVGs), SC_Respirator's filter."""
    ref = p.get("SpecialContainerWidget")
    if not ref or "/Widgets/Firearms/" in ref["ObjectPath"]:
        return []
    f = ROOT / (ref["ObjectPath"].split(".")[0].removeprefix("SurrounDead/Content/") + ".json")
    if not f.exists():
        return []
    out = []
    for e in load(f):
        pr = e.get("Properties", {})
        if e["Type"] == "JSIContainer_C" and not (pr.get("ContainerTag") or {}).get("TagName", "").startswith("Jig.AttachmentType."):
            out += [pkg_name(r) for r in pr.get("OnlySupportTheseItems") or [] if r]
    return out


def attachment(p, roles):
    """A weapon attachment's Local_AttachmentType, Local_AttachSocket and FOVZoom (sights), and
    which of BP_WeaponsPickupComponent's lists names it (Suppressors, MuzzleBrakes, Sights,
    Scopes)."""
    kind = (p.get("Local_AttachmentType") or {}).get("TagName") or ""
    if not kind.startswith("Jig.AttachmentType.") or kind.endswith(".None"):
        return None
    kind = kind.rsplit(".", 1)[-1]
    return {
        "type": kind,
        "socket": p.get("Local_AttachSocket"),
        # The aiming field of view (ScopeFunction's Zoom); only sights use it.
        "fov_zoom": p.get("FOVZoom") if kind == "Sight" else None,
        "role": roles.get(p.get("_name")),
        # Local_ActorClass: what hangs on the gun's socket.
        "actor": local_actor(p.get("Local_ActorClass")),
    }


def template_transform(t):
    return glb_transform(t.get("RelativeLocation"), rotator_quat(t.get("RelativeRotation")), t.get("RelativeScale3D"))


def local_actor(ref):
    """A BP_AMainLocalAttachment child (Inventory/Items/LocalAttachments): its StaticMesh and
    light components, each relative to the actor (all hang off DefaultSceneRoot), which
    ProcessAttachments snaps to the gun's socket (location, rotation and scale)."""
    if not ref:
        return None
    f = ROOT / (ref["ObjectPath"].split("/Content/", 1)[1].rsplit(".", 1)[0] + ".json")
    if not f.exists():
        return None
    exports = load(f)
    templates = {e["Name"].removesuffix("_GEN_VARIABLE"): e for e in exports if e["Name"].endswith("_GEN_VARIABLE")}
    parents = {}
    for e in exports:
        if e["Type"] == "SCS_Node":
            pr = e.get("Properties", {})
            name = (pr.get("ComponentTemplate") or {}).get("ObjectName", "").split(":")[-1].removesuffix("_GEN_VARIABLE'")
            parents[name] = pr.get("ParentComponentOrVariableName", "DefaultSceneRoot")
    out = {"mesh": None, "lights": []}
    for name, e in templates.items():
        pr = e.get("Properties") or {}
        if parents.get(name, "DefaultSceneRoot") != "DefaultSceneRoot" and e["Type"] != "SceneComponent":
            print(f"warning: {f.name}: {name} hangs off {parents[name]}, taken as the root's child")
        if e["Type"] == "StaticMeshComponent" and pr.get("StaticMesh"):
            out["mesh"] = pr["StaticMesh"]["ObjectPath"].rsplit(".", 1)[0]
            out["transform"] = template_transform(pr)
        elif e["Type"] in ("PointLightComponent", "SpotLightComponent"):
            out["lights"].append({
                "kind": e["Type"].removesuffix("LightComponent").lower(),
                # UE light defaults: Intensity 5000 (point) / 5000 (spot), radius 1000, cone 44.
                "intensity": pr.get("Intensity", 5000.0),
                "radius": pr.get("AttenuationRadius", 1000.0),
                "outer_cone": pr.get("OuterConeAngle", 44.0),
                "transform": template_transform(pr),
            })
    return out if out["mesh"] or out["lights"] else None


def mesh_sockets(mesh):
    """A gun's skeleton sockets (and the mesh's own, which win), bone + glb local transform:
    where ProcessAttachments' K2_AttachToComponent puts each attachment."""
    if not mesh:
        return {}
    f = ROOT / (mesh.split("/Content/", 1)[1] + ".json")
    if not f.exists():
        return {}
    out = {}
    exports = load(f)
    skel = next((e["Properties"]["Skeleton"] for e in exports if e["Type"] == "SkeletalMesh" and (e.get("Properties") or {}).get("Skeleton")), None)
    files = []
    if skel:
        sf = ROOT / (skel["ObjectPath"].split("/Content/", 1)[1].rsplit(".", 1)[0] + ".json")
        if sf.exists():
            files.append(load(sf))
    files.append(exports)
    for exp in files:
        for e in exp:
            if e["Type"] != "SkeletalMeshSocket":
                continue
            pr = e.get("Properties", {})
            if pr.get("SocketName") and pr.get("BoneName"):
                out[pr["SocketName"]] = {"bone": pr["BoneName"], **glb_transform(pr.get("RelativeLocation"), rotator_quat(pr.get("RelativeRotation")), pr.get("RelativeScale"))}
    return out


def attachment_roles():
    f = ROOT / "Inventory/Items/Other/Components/BP_WeaponsPickupComponent.json"
    if not f.exists():
        return {}
    base = next((e.get("Properties", {}) for e in load(f) if e["Name"].startswith("Default__")), {})
    roles = {}
    for key, role in (("Suppressors", "suppressor"), ("MuzzleBrakes", "muzzle_brake"), ("Sights", "sight"), ("Scopes", "scope")):
        for r in base.get(key) or []:
            roles[pkg_name(r)] = role
    return roles


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


GEAR = {"Headwear", "Facewear", "Eyewear", "Accessory"}


def worn(p):
    """Head gear (headwear, facewear, eyewear, accessories): BP_PlayerCharacter's
    EquipActorToSocket hangs the item's PickupClass actor on the socket, so what shows is that
    actor's StaticMeshComponent: its mesh, transform in the actor and OverrideMaterials
    (packages, by slot), walking parent pickups."""
    if (p.get("Type") or {}).get("TagName", "").rsplit(".", 1)[-1] not in GEAR:
        return None
    ref = p.get("PickupClass")
    while ref:
        f = ROOT / (ref["ObjectPath"].split(".")[0].removeprefix("SurrounDead/Content/") + ".json")
        if not f.exists():
            return None
        exports = load(f)
        comp = next((e.get("Properties") or {} for e in exports if e["Type"] == "StaticMeshComponent" and (e.get("Properties") or {}).get("StaticMesh")), None)
        if comp is not None:
            return {
                "mesh": comp["StaticMesh"]["ObjectPath"].rsplit(".", 1)[0],
                "transform": template_transform(comp),
                "materials": [r["ObjectPath"].rsplit(".", 1)[0] if r else None for r in comp.get("OverrideMaterials") or []],
            }
        cls = next((e for e in exports if e["Type"] == "BlueprintGeneratedClass"), None)
        ref = (cls or {}).get("Super") or (cls or {}).get("SuperStruct")
    return None


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


def weapon_component(p):
    """BP_WeaponsPickupComponent on the item's pickup Blueprint, over the component's class
    defaults, walking parent pickups (None for items without one)."""
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
    return c


def weapon_speeds(p):
    """The montage play rates BP_PlayerCharacter starts from: SwingSpeed (melee attacks, ×
    StrengthSpeedMultiplier) and ReloadBaseSpeed (reloads, × ReloadingMultiplier)."""
    c = weapon_component(p) or {}
    return c.get("SwingSpeed", 1.0), c.get("ReloadBaseSpeed", 1.0)


def montage_name(ref):
    return ref["ObjectName"].split("'")[1] if ref else None


def melee(p):
    """BP_PlayerCharacter's melee attacks off the weapon component: Normal / Power
    MeleeAttackMontages (random picks), CrouchedMeleeAttackMontage (crouched or looking 45° down),
    DelayBetweenSwings, MaxActorsToHitInSwing? (BP_MeleePickup.MaxMeleeHits, with MULTI HIT)."""
    kind = (p.get("Type") or {}).get("TagName", "").rsplit(".", 1)[-1]
    c = weapon_component(p)
    if c is None or kind != "Melee":
        return None
    return {
        "normal": [montage_name(m) for m in c.get("NormalMeleeAttackMontages") or []],
        "power": [montage_name(m) for m in c.get("PowerMeleeAttackMontages") or []],
        "crouched": montage_name(c.get("CrouchedMeleeAttackMontage")),
        "delay": c.get("DelayBetweenSwings", 0.75),
        "max_hits": c.get("MaxActorsToHitInSwing?", 2),
    }


def melee_montages(items):
    """The montages the melee weapons name: their sequence (relative to Content/Animations),
    length, and the MeleeHitDetect(_Power) notify state's window [start, duration]."""
    names = {m for d in items.values() for m in ((d.get("melee") or {}).get("normal", []) + (d.get("melee") or {}).get("power", []) + [(d.get("melee") or {}).get("crouched")]) if m}
    out = {}
    for f in (ROOT / "Animations").rglob("*.json"):
        if f.stem not in names:
            continue
        m = next((e for e in load(f) if e["Type"] == "AnimMontage"), None)
        if not m:
            continue
        mp = m["Properties"]
        segs = [s for t in mp.get("SlotAnimTracks", []) for s in t["AnimTrack"]["AnimSegments"]]
        hit = next((n for n in mp.get("Notifies", []) if str(n.get("NotifyName", "")).startswith("MeleeHitDetect")), None)
        clip = segs[0]["AnimReference"]["ObjectPath"].split(".")[0].removeprefix("SurrounDead/Content/Animations/") if segs else None
        out[f.stem] = {
            "clip": clip,
            "length": mp.get("SequenceLength", 0.0),
            "hit": [hit.get("LinkValue", 0.0), hit.get("Duration", 0.0)] if hit else None,
            "power": bool(hit and "Power" in hit["NotifyName"]),
        }
    return out


def harvests(p):
    """What a melee weapon's swings harvest (BP_WeaponsPickupComponent's CanDamageTree? /
    CanDamageRock? / CanDamageTrashObjects?, read by BP_MeleePickup.HitFoliageTree / HitFoliageRock /
    HitTrashObject)."""
    c = weapon_component(p) or {}
    return [k for k, flag in (("tree", "CanDamageTree?"), ("rock", "CanDamageRock?"), ("trash", "CanDamageTrashObjects?")) if c.get(flag)]


def firearm(p):
    """What BP_FirearmPickup's firing reads off the weapon component."""
    c = weapon_component(p)
    if c is None:
        return None
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
        # Recoil_Struct's random deviations and the timelines' rate / the reset's share
        # (BP_FirearmPickup.WeaponRecoilValues).
        "recoil_vertical_deviation": field(recoil, "VerticalRandomDeviation") or 0.0,
        "recoil_horizontal_deviation": field(recoil, "HorizontalRandomDeviation") or 0.0,
        "recoil_rate": c.get("RecoilRate", 2.5),
        "reset_recoil_divider": c.get("ResetRecoilDivider", 1.5),
        "pellets": c.get("ShotgunPellets", 6),
        # The shot's SoundCue (PlaySoundAtLocation at the muzzle), suppressed and chambering ones.
        "sound": pkg_name(c.get("FirearmSound")),
        "suppressed_sound": pkg_name(c.get("FirearmSuppressedSound")),
        "chamber_sound": pkg_name(c.get("FirearmChamberSound")),
        # Svr_WeaponEffects' particle system at the Muzzle socket (SuppressedMuzzleFlash with a
        # suppressor), and the shell SpawnEmitterAtLocation drops at the ejection port.
        "muzzle_flash": pkg_name(c.get("MuzzleFlash")),
        "suppressed_muzzle_flash": pkg_name(c.get("SuppressedMuzzleFlash")),
        "brass": pkg_name(c.get("BrassType")),
        # DoesWeaponUseAProjectile?: BP_FirearmPickup spawns ProjectileClass instead of tracing.
        "projectile": projectile(c),
    }


def projectile(c):
    """The weapon component's ProjectileClass (Inventory/Items/Other/ItemProjectiles), resolved
    down from BP_ProjectileMaster: its ProjectileMovement (speed, gravity scale), StaticMesh,
    PointLight, and its own MinDamage / MaxDamage (the rocket's; the launcher sets MinDmg /
    MaxDmg on spawn, which only the grenade reads)."""
    if not c.get("DoesWeaponUseAProjectile?") or not c.get("ProjectileClass"):
        return None
    cls = c["ProjectileClass"]["ObjectName"].split("'")[1]
    chain = []
    name = cls.removesuffix("_C")
    while name:
        f = ROOT / "Inventory/Items/Other/ItemProjectiles" / f"{name}.json"
        if not f.exists():
            break
        exports = load(f)
        chain.append(exports)
        bgc = next((e for e in exports if e["Type"] == "BlueprintGeneratedClass"), {})
        sup = bgc.get("Super") or bgc.get("SuperStruct")
        name = pkg_name(sup) if sup and "ItemProjectiles" in sup.get("ObjectPath", "") else None
    # Parent first, so the child's templates and defaults win.
    props = {}
    for exports in reversed(chain):
        for e in exports:
            p = e.get("Properties") or {}
            if e["Name"].startswith("Default__"):
                props.setdefault("cdo", {}).update(p)
            elif e["Name"].endswith("_GEN_VARIABLE"):
                props.setdefault(e["Name"].removesuffix("_GEN_VARIABLE"), {}).update(p)
    move, cdo = props.get("ProjectileMovement", {}), props.get("cdo", {})
    light = props.get("PointLight")
    mesh = (props.get("StaticMesh") or {}).get("StaticMesh")
    return {
        "class": cls,
        # cm -> m; ProjectileGravityScale defaults to 1.
        "speed": move.get("InitialSpeed", 0.0) / 100.0,
        "gravity": move.get("ProjectileGravityScale", 1.0),
        "mesh": mesh["ObjectPath"].rsplit(".", 1)[0] if mesh else None,
        "damage": [cdo["MinDamage"], cdo["MaxDamage"]] if "MinDamage" in cdo else None,
        "light": light and {
            "color": [light["LightColor"][k] / 255.0 for k in "RGB"],
            "intensity": light.get("Intensity", 5000.0),
            "radius": light.get("AttenuationRadius", 1000.0) / 100.0,
            "inverse_square": light.get("bUseInverseSquaredFalloff", True),
        },
    }


def item_defaults():
    for e in load(ROOT / "JigSInventory/Jigsaw/Data/DataAsset/JigsawItem_DataAsset.json"):
        if e["Name"].startswith("Default__"):
            return e.get("Properties", {})
    return {}


def throwable(p):
    """The DA's ThrowableClass (Blueprints/Throwables/BP_Throwable_*), with its class defaults
    over BP_MasterThrowable's: MinDamage/MaxDamage and ExplosionDelay (the fuse)."""
    ref = p.get("ThrowableClass")
    if not ref:
        return None
    cls = ref["ObjectName"].split("'")[1]
    defaults = {"MinDamage": 0.0, "MaxDamage": 0.0, "ExplosionDelay": 2.0}
    for name in ("BP_MasterThrowable", cls.removesuffix("_C")):
        f = ROOT / "Blueprints/Throwables" / f"{name}.json"
        if f.exists():
            for e in load(f):
                if e["Name"].startswith("Default__"):
                    defaults.update({k: v for k, v in e.get("Properties", {}).items() if k in defaults})
    return {"class": cls, "min_damage": defaults["MinDamage"], "max_damage": defaults["MaxDamage"], "fuse": defaults["ExplosionDelay"]}


def tech_tree():
    """SkillTree (the Tech Tree plugin's TechnologyTree): each node's BP_SkillTreeAsset (name,
    tooltip, Cost, icon), editor position and parents -> assets/data/techtree.json."""
    f = ROOT / "Blueprints/SkillTree/SkillTree.json"
    if not f.exists():
        return None
    exports = load(f)
    by_ref = {e["Name"]: e for e in exports if e["Type"] == "TTTechNode"}
    tree = next(e for e in exports if e["Type"] == "TechnologyTree")["Properties"]
    nodes = []
    for e in by_ref.values():
        p = e["Properties"]
        skill = pkg_name(p["TechnologyAsset"])
        a = next(x for x in load(ROOT / f"Blueprints/SkillTree/Skills/{skill}.json") if x["Type"] == "BP_SkillTreeAsset_C")["Properties"]
        parents = [pkg_name(by_ref[r["ObjectName"].split(":")[-1].rstrip("'")]["Properties"]["TechnologyAsset"]) for r in p.get("ParentNodes") or []]
        nodes.append({
            "id": skill,
            "name": text(a.get("PrimaryText")),
            "tooltip": text(a.get("TooltipText")),
            "cost": a.get("Cost", 1),
            # Enum_SkillType index: SkillTreeWidgetIcon tints the icon by it (0 red, 1 green, 2 blue, 3 grey).
            "kind": int(str(a.get("Type", "Enum_SkillType::NewEnumerator0")).rsplit("NewEnumerator", 1)[-1] or 0),
            "icon": "game/" + a["TechIcon"]["ResourceObject"]["ObjectPath"].split(".")[0] + ".png",
            "pos": [p.get("NodePosX", 0), p.get("NodePosY", 0)],
            "parents": parents,
        })
    nodes.sort(key=lambda n: (n["pos"][1], n["pos"][0]))
    size = tree.get("TreeSize", {})
    return {"size": [size.get("X", 640.0), size.get("Y", 1000.0)], "slot": [tree.get("SlotSize", {}).get("X", 70.0), tree.get("SlotSize", {}).get("Y", 70.0)], "nodes": nodes}


def main():
    base = item_defaults()
    roles = attachment_roles()
    items = {}
    for f in (ROOT / "Inventory/Items/DataAssets").rglob("*.json"):
        for e in load(f):
            if e["Type"] != "JigsawItem_DataAsset_C":
                continue
            p = {**base, **e.get("Properties", {}), "_name": e["Name"]}
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
                # The disassembly workbench: CanBeDisassembled? (true by default) and what
                # DisassemblyItems rolls (RandomContainerItem: chance, count range).
                "disassembly": disassembly(p),
                # Firearms: their attachment slots (SC_<gun>); attachments: what they are.
                "slots": weapon_slots(p),
                # Firearms: their skeleton's sockets (the attachments' places).
                "mesh_sockets": mesh_sockets((mesh or {}).get("ObjectPath", "").rsplit(".", 1)[0]) if weapon_slots(p) else {},
                "attachment": attachment(p, roles),
                # SC_Battery / SC_Respirator / SC_NVG: the battery or filter it takes.
                "holds": holds(p),
                # SpecialContainerWidget (SC_<gun>, SC_Battery...): JSI_Slot's double click opens
                # it in a window instead of the inspector.
                "special": pkg_name(p["SpecialContainerWidget"]) if p.get("SpecialContainerWidget") else None,
                # TextStats: "Firemodes" ("Semi, Auto"), "Caliber".
                "text_stats": text_stats(p),
                # Magazines: rounds held and which ammo fits.
                "capacity": p.get("ContainerCapacity", 0) if p.get("IsContainer?") and not wearable_grids(p) else 0,
                "ammo": [pkg_name(r) for r in p.get("ContainerOnlySupportedIDs") or [] if r],
                "durability": p.get("MaxDurability", 0.0),
                # Durability lost per shot / swing (ReduceDurability), before the difficulty multiplier.
                "reduce_durability": p.get("ReduceDurabilityAmount", 0.0),
                # IsArmor?: body armor / headwear that divides damage (CalculatePlayerDamage).
                "armor": bool(p.get("IsArmor?")),
                # Flashlights: the spot light's Intensity when switched on (FlashlightToggle).
                "flashlight_intensity": p.get("FlashlightIntensity", 0.0) if (p.get("Type") or {}).get("TagName", "").endswith("Flashlight") else 0.0,
                # ContextMenuOptions (Jig.Context.*): what the right-click menu offers ("ReadBook"...).
                "context": [str(t).rsplit(".", 1)[-1] for t in p.get("ContextMenuOptions") or []],
                # OverItemOptions (Jig.ItemOverItem.*): what dropping it onto another item does
                # (the repair kits' "WeaponRepair" / "AttachmentRepair" / "ArmorRepair").
                "over_item": [str(t).rsplit(".", 1)[-1] for t in p.get("OverItemOptions") or []],
                "swing_speed": weapon_speeds(p)[0],
                "harvests": harvests(p),
                # Melee weapons' attack montages, DelayBetweenSwings, MaxActorsToHitInSwing?.
                "melee": melee(p),
                "reload_speed": weapon_speeds(p)[1],
                "firearm": firearm(p) if (p.get("Type") or {}).get("TagName", "").endswith(("MainFirearm", "Sidearm")) else None,
                "equip": equip(p),
                # Head gear: the pickup actor hung on the socket (BP_PlayerCharacter's OnRep_*Equipped?).
                "worn": worn(p),
                # Binoculars: BinocularsZoomValue (the first-person FOV is divided by it) and
                # ShowDistance? (the widget's range readout).
                "binoculars": {"zoom": p.get("BinocularsZoomValue", 4.0), "distance": bool(p.get("ShowDistance?"))} if (p.get("Type") or {}).get("TagName", "").endswith("Binoculars") else None,
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
                # The item inspector (BP_ItemInspector.SetItemToInspect): MaxZoomIn / MaxZoomOut
                # clamp its FOV; Description is its info text.
                "zoom": [p.get("MaxZoomIn", 1.0), p.get("MaxZoomOut", 100.0)],
                "description": text(p.get("Description")) or "",
                # Wearable containers (backpacks, clothing pockets, rigs, cases): their grids.
                "grids": wearable_grids(p),
                # How their SC_ widget lays those grids out (the inventory column and the
                # container window draw it).
                "layout": sc_layout(p),
                # Clothing worn on the survivor (skeletal meshes on the shared body skeleton).
                "clothing": clothing(p),
                "fov": p.get("FOV", 30.0),
                # Throwables (G): the ThrowableClass and its class defaults.
                "throwable": throwable(p),
            }

    def loot_rows(rows):
        return [
            {
                "item": pkg_name(field(r, "ItemID")),
                "chance": field(r, "SpawnChance") or 0.0,
                "min": field(r, "RandomCountMin") or 1,
                "max": field(r, "RandomCountMax") or 1,
            }
            for r in rows if field(r, "ItemID")
        ]

    tables = {}
    # Container loot tables, plus the AI ones dead zombies/animals carry (AI/AILootTables).
    for f in [*(ROOT / "Inventory/Containers/LootTables").glob("*.json"), *(ROOT / "AI/AILootTables").glob("*.json")]:
        for e in load(f):
            if e["Type"] != "ContainerItems_DataAsset_C":
                continue
            p = e.get("Properties", {})
            rows = field(p.get("Loot Table", {}), "ContainerSettings") or []
            tables[p.get("ID", e["Name"])] = loot_rows(rows)

    # GameFunctionLibrary.AddUniqueLoot appends a row's ItemSet to the container's starting items.
    unique = {row: loot_rows(field(v, "ItemSet") or []) for row, v in load(ROOT / "Inventory/Containers/DT_UniqueLoot.json")[0].get("Rows", {}).items()}

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
            mesh = next((e["Properties"].get("StaticMesh") for e in exports if e["Name"] == "StaticMesh_GEN_VARIABLE" and "Properties" in e), None)
            # The BP_JigMultiplayer template's DoesContainerRefillLoot? (its own or an
            # InheritableComponentHandler override; the component's default is true).
            refill = next((e["Properties"]["DoesContainerRefillLoot?"] for e in exports if e.get("Type") == "BP_JigMultiplayer_C" and "DoesContainerRefillLoot?" in (e.get("Properties") or {})), None)
            classes[cls["Name"]] = {"super": sup.split("'")[1] if "'" in sup else None, "props": cdo.get("Properties", {}), "mesh": mesh["ObjectPath"].rsplit(".", 1)[0] if mesh else None, "refill": refill}

    def inherited(name, key):
        while name in classes:
            v = classes[name]["props"].get(key)
            if v is not None:
                return v
            name = classes[name]["super"]
        return None

    def refills(name):
        while name in classes:
            if classes[name]["refill"] is not None:
                return classes[name]["refill"]
            name = classes[name]["super"]
        return True

    def magnifier(name):
        # MagnifierTransform's translation (UE cm, actor space) in Bevy metres: (X, Z, Y) / 100.
        t = (inherited(name, "MagnifierTransform") or {}).get("Translation") or {}
        return [round(t.get("X", 0.0) / 100, 4), round(t.get("Z", 0.0) / 100, 4), round(t.get("Y", 0.0) / 100, 4)]

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
            # ChanceToProduceNoise (%): Quick Search's per-0.075 s roll for a 40 m noise.
            "noise": (inherited(name, "ChanceToProduceNoise") or 10.0) if is_a(name, "BP_LootContainer_C") else 0.0,
            "mesh": classes[name]["mesh"],
            # LOOT REGENERATION (`gameplay/loot_refill.rs`) and the search icon (DistanceOutline).
            "refills": refills(name),
            "magnifier": magnifier(name),
        }

    # BP_MASTER_ToolRequiredContainer's children (the emergency axe cabinets, gun lockers): held
    # DurationToHold (12 s), and with ToolNeeded? only while ToolNeeded (a data asset; the gun
    # lockers' instances name theirs) is carried.
    for name in classes:
        if not is_a(name, "BP_MASTER_ToolRequiredContainer_C") or name == "BP_MASTER_ToolRequiredContainer_C":
            continue
        tool = inherited(name, "ToolNeeded")
        containers[name] = {
            "name": text(inherited(name, "ContainerName")) or "Locker",
            "table": enum.get(inherited(name, "LootTable") or "Enum_ContainerLootTables::NewEnumerator0"),
            "hold": inherited(name, "DurationToHold") or 12.0,
            "lockpick": False,
            "noise": inherited(name, "DefaultChanceToProduceNoise") or 10.0,
            "mesh": classes[name]["mesh"],
            "tool": {"required": bool(inherited(name, "ToolNeeded?")), "item": pkg_name(tool) if tool else None},
            "refills": refills(name),
            "magnifier": magnifier(name),
        }

    # Airdrop crates (Master_AirdropContainer's children, dropped by AirdropAircraft): held 6 s,
    # opened as "Airdrop" (AddJigWidgetToContent's title), their map marker's title and colour.
    for name in classes:
        if not is_a(name, "Master_AirdropContainer_C") or name == "Master_AirdropContainer_C":
            continue
        colour = inherited(name, "Color") or {}
        containers[name] = {
            "name": "Airdrop",
            "table": enum.get(inherited(name, "LootTable") or "Enum_ContainerLootTables::NewEnumerator0"),
            "hold": inherited(name, "DurationToHold") or 6.0,
            "lockpick": False,
            "noise": inherited(name, "ChanceToProduceNoise") or 10.0,
            "mesh": classes[name]["mesh"] or classes.get(classes[name]["super"], {}).get("mesh"),
            "airdrop": {"title": inherited(name, "Text") or "Airdrop", "color": [colour.get("R", 1.0), colour.get("G", 1.0), colour.get("B", 1.0)]},
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

    tech = tech_tree()
    if tech:
        (OUT.parent / "techtree.json").write_text(json.dumps(tech, indent=1, sort_keys=True))
    OUT.write_text(json.dumps({"items": items, "melee_montages": melee_montages(items), "loot_tables": tables, "containers": containers, "unique_loot": unique, "recipes": recipes, "sockets": sockets(ROOT)}, indent=1, sort_keys=True))
    missing = sorted({c["table"] for c in containers.values() if c["table"] and c["table"] not in tables})
    print(f"items: {len(items)}, loot tables: {len(tables)}, containers: {len(containers)}, recipes: {sum(map(len, recipes.values()))}; unknown tables: {missing}")


main()
