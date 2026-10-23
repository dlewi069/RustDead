#!/usr/bin/env python3
"""Build assets/data/buildables.json from `SdExport json` dumps of the building system.

usage: gen_buildables.py <json dump root>   (normally run via `export_assets.sh --buildables`)

Needs dumps of Content/Inventory/Items/DataAssets/, Content/Blueprints/BuildingSystem/Actors/,
Content/Blueprints/BuildingSystem/BP_NoBuildArea, Content/Blueprints/HordeSystem/ and
JigSInventory/Jigsaw/Data/DataAsset/JigsawItem_DataAsset (class defaults). Writes:

  items     buildable DA -> what BuildingComponent reads off it: BuildMesh (the ghost),
            BuildActorClass, SnapType (Enum_BuildingSnapTypes' name), BuildBoundsCheckMultiplier,
            BuildBoundsOverlapOffset (UE cm), BuildAllowBuildingInNoBuildZones
  actors    Buildable_* class -> its InteractOptions labels, Tags and the static mesh parts of
            its construction script (Buildable_MASTER's and its own, with the inherited
            components' overrides): mesh, override materials, collision, parent part and the
            transform relative to it (glb space)
  horde     Buildable_HordeBeacon: DT_HordeSystem's HordeBeacon waves, its Rewards,
            RewardSpawnLoc, Spawn Retries and the dish Timeline's Rotate Alpha keys
  no_build  BP_NoBuildArea's Box extent (UE cm)
"""
import json
import math
import sys
from pathlib import Path

import numpy as np

ROOT = Path(sys.argv[1])
OUT = Path(__file__).resolve().parent.parent / "assets" / "data" / "buildables.json"


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


def class_name(ref):
    return ref["ObjectName"].split("'")[1] if ref else None


def asset_path(ref):
    """An object path without its export index ("SurrounDead/Content/.../SM_X")."""
    return ref["ObjectPath"].rsplit(".", 1)[0] if ref else None


def dump_of(ref):
    """The dump file of a class or asset reference."""
    return ROOT / (asset_path(ref).removeprefix("SurrounDead/Content/") + ".json")


def rotator_quat(r):
    """UE FRotator (degrees) -> FQuat (x, y, z, w), FRotator::Quaternion()."""
    r = r or {}
    p, y, rl = (math.radians(r.get(k, 0.0)) / 2 for k in ("Pitch", "Yaw", "Roll"))
    sp, cp, sy, cy, sr, cr = math.sin(p), math.cos(p), math.sin(y), math.cos(y), math.sin(rl), math.cos(rl)
    return [cr * sp * sy - sr * cp * cy, -cr * sp * cy - sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy]


def quat_matrix(q):
    x, y, z, w = q
    return np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])


def matrix_quat(m):
    t = m[0][0] + m[1][1] + m[2][2]
    if t > 0:
        s = math.sqrt(t + 1.0) * 2
        return [(m[2][1] - m[1][2]) / s, (m[0][2] - m[2][0]) / s, (m[1][0] - m[0][1]) / s, 0.25 * s]
    if m[0][0] > m[1][1] and m[0][0] > m[2][2]:
        s = math.sqrt(1.0 + m[0][0] - m[1][1] - m[2][2]) * 2
        return [0.25 * s, (m[0][1] + m[1][0]) / s, (m[0][2] + m[2][0]) / s, (m[2][1] - m[1][2]) / s]
    if m[1][1] > m[2][2]:
        s = math.sqrt(1.0 + m[1][1] - m[0][0] - m[2][2]) * 2
        return [(m[0][1] + m[1][0]) / s, 0.25 * s, (m[1][2] + m[2][1]) / s, (m[0][2] - m[2][0]) / s]
    s = math.sqrt(1.0 + m[2][2] - m[0][0] - m[1][1]) * 2
    return [(m[0][2] + m[2][0]) / s, (m[1][2] + m[2][1]) / s, 0.25 * s, (m[1][0] - m[0][1]) / s]


def local_matrix(p):
    """A scene component's RelativeLocation / RelativeRotation / RelativeScale3D as a 4x4 (UE cm)."""
    loc, scale = p.get("RelativeLocation") or {}, p.get("RelativeScale3D") or {}
    m = np.identity(4)
    m[:3, :3] = quat_matrix(rotator_quat(p.get("RelativeRotation"))) @ np.diag([scale.get(k, 1.0) for k in "XYZ"])
    m[:3, 3] = [loc.get(k, 0.0) for k in "XYZ"]
    return m


def glb_transform(m):
    """A UE 4x4 -> glb space: translation (X, Z, Y)/100, quat (x, z, y, -w), scale (X, Z, Y)."""
    scale = [float(np.linalg.norm(m[:3, i])) for i in range(3)]
    rot = m[:3, :3] / np.array(scale)
    x, y, z, w = matrix_quat(rot)
    t = m[:3, 3]
    r = lambda v: round(float(v), 6)
    return {"t": [r(t[0] / 100), r(t[2] / 100), r(t[1] / 100)], "r": [r(x), r(z), r(y), r(-w)], "s": [r(scale[0]), r(scale[2]), r(scale[1])]}


def class_chain(path):
    """The class's dump exports, then its parents' up to Buildable_MASTER (root last)."""
    chain = []
    while path and path.exists():
        exports = load(path)
        cls = next(e for e in exports if e["Type"] == "BlueprintGeneratedClass")
        chain.append(exports)
        sup = cls.get("SuperStruct")
        path = dump_of(sup) if sup and "Buildable" in sup.get("ObjectName", "") else None
    return chain


def actor(path):
    """Interact options, tags and static mesh parts of a Buildable_* class."""
    chain = class_chain(path)
    if not chain:
        return None
    cdo = {}
    nodes = {}  # variable name -> {props, parent, class}
    for exports in reversed(chain):
        named = {e["Name"]: e for e in exports}
        cdo.update(next((e.get("Properties", {}) for e in exports if e["Name"].startswith("Default__")), {}))
        scs = [e for e in exports if e["Type"] == "SCS_Node"]
        parent_of = {}
        for e in scs:
            for c in e["Properties"].get("ChildNodes") or []:
                parent_of[c["ObjectName"].rsplit(".", 1)[-1].rstrip("'")] = e["Properties"].get("InternalVariableName")
        for e in scs:
            p = e["Properties"]
            var = p.get("InternalVariableName")
            tmpl = p.get("ComponentTemplate")
            props = dict(named.get(tmpl["ObjectName"].split(":")[-1].rstrip("'"), {}).get("Properties", {})) if tmpl else {}
            nodes[var] = {"props": props, "parent": parent_of.get(e["Name"]) or p.get("ParentComponentOrVariableName"), "class": class_name(p.get("ComponentClass"))}
        # Inherited components' overrides (InheritableComponentHandler records).
        for e in exports:
            if e["Type"] != "InheritableComponentHandler":
                continue
            for rec in e.get("Properties", {}).get("Records", []):
                var = rec["ComponentKey"].get("SCSVariableName")
                tmpl = rec.get("ComponentTemplate")
                if var in nodes and tmpl:
                    nodes[var]["props"].update(named.get(tmpl["ObjectName"].split(":")[-1].rstrip("'"), {}).get("Properties", {}))

    def world(var, seen=()):
        n = nodes.get(var)
        if not n or var in seen:
            return np.identity(4)
        return world(n["parent"], (*seen, var)) @ local_matrix(n["props"])

    def is_part(var):
        n = nodes[var]
        return n["class"] == "StaticMeshComponent" and n["props"].get("StaticMesh") and n["props"].get("bVisible", True) and not n["props"].get("bHiddenInGame")

    parts = []
    for var, n in nodes.items():
        if not is_part(var):
            continue
        # The nearest part above it carries it (the beacon's light cube turns with the dish).
        up = n["parent"]
        while up in nodes and not is_part(up):
            up = nodes[up]["parent"]
        up = up if up in nodes else None
        m = np.linalg.inv(world(up)) @ world(var) if up else world(var)
        body = n["props"].get("BodyInstance") or {}
        parts.append({
            "name": var,
            "parent": up,
            "mesh": asset_path(n["props"]["StaticMesh"]),
            "materials": [r["ObjectName"].split("'")[1] if r else None for r in n["props"].get("OverrideMaterials") or []],
            "collides": body.get("CollisionEnabled", "ECollisionEnabled::QueryAndPhysics") != "ECollisionEnabled::NoCollision",
            **glb_transform(m),
        })
    return {
        "options": [text(o["Value"]) for o in cdo.get("InteractOptions") or []],
        "tags": cdo.get("Tags") or [],
        "pickup": asset_path(cdo.get("PickupDataAsset")).rsplit("/", 1)[1] if cdo.get("PickupDataAsset") else None,
        "parts": parts,
    }


def pickup_items():
    """Pickup BP class -> item data asset (the DA's PickupClass)."""
    out = {}
    for f in (ROOT / "Inventory/Items/DataAssets").rglob("*.json"):
        for e in load(f):
            ref = (e.get("Properties") or {}).get("PickupClass")
            if ref and e["Name"].startswith("DA_"):
                out.setdefault(class_name(ref), e["Name"])
    return out


def horde():
    path = ROOT / "Blueprints/HordeSystem/Beacon/Buildable_HordeBeacon.json"
    exports = load(path)
    named = {e["Name"]: e.get("Properties") or {} for e in exports}
    cdo = named["Default__Buildable_HordeBeacon_C"]
    rows = load(ROOT / "Blueprints/HordeSystem/DT_HordeSystem.json")[0]["Rows"]
    waves = []
    for w in field(rows["HordeBeacon"], "WaveInfo"):
        ai, amount = field(w, "AI") or [], field(w, "Amount") or []
        waves.append({"groups": [[class_name(c), n] for c, n in zip(ai, amount)], "final": bool(field(w, "FinalWave?"))})
    items = pickup_items()
    rewards = []
    for r in field(cdo["Rewards"], "ItemsToSpawn") or []:
        cls = class_name(field(r, "ItemToSpawn"))
        if cls not in items:
            sys.exit(f"horde reward without an item: {cls}")
        rewards.append({"item": items[cls], "chance": field(r, "SpawnChance"), "min": field(r, "MinCount"), "max": field(r, "MaxCount")})
    curve = next(e for e in exports if e["Type"] == "CurveFloat")["Properties"]["FloatCurve"]["Keys"]
    timeline = next(e for e in exports if e["Type"] == "TimelineTemplate")["Properties"]
    loc = named["RewardSpawnLoc_GEN_VARIABLE"].get("RelativeLocation") or {}
    return {
        "waves": waves,
        "rewards": rewards,
        "max_rewards": field(cdo["Rewards"], "MaxItemsToSpawn"),
        "spawn_retries": cdo.get("Spawn Retries", 0),
        "reward_at": [loc.get("X", 0.0) / 100, loc.get("Z", 0.0) / 100, loc.get("Y", 0.0) / 100],
        "dish_secs": timeline["TimelineLength"],
        "dish_alpha": [[k["Time"], k["Value"]] for k in curve],
    }


def main():
    base = {}
    for e in load(ROOT / "JigSInventory/Jigsaw/Data/DataAsset/JigsawItem_DataAsset.json"):
        if e["Name"].startswith("Default__"):
            base = e.get("Properties", {})
    snap_names = {}
    for e in load(ROOT / "Blueprints/BuildingSystem/Enum_BuildingSnapTypes.json"):
        for kv in e.get("Properties", {}).get("DisplayNameMap", []):
            snap_names[f"Enum_BuildingSnapTypes::{kv['Key']}"] = text(kv["Value"])
    items, actors = {}, {}
    for f in sorted((ROOT / "Inventory/Items/DataAssets").rglob("*.json")):
        for e in load(f):
            p = {**base, **(e.get("Properties") or {})}
            ref = p.get("BuildActorClass")
            if e["Type"] != "JigsawItem_DataAsset_C" or not ref:
                continue
            offset = p.get("BuildBoundsOverlapOffset") or {}
            cls = class_name(ref)
            items[e["Name"]] = {
                "mesh": asset_path(p.get("BuildMesh")),
                "actor": cls,
                "snap": snap_names.get(p.get("SnapType") or "Enum_BuildingSnapTypes::NewEnumerator0", "NoSnap"),
                "bounds_multiplier": p.get("BuildBoundsCheckMultiplier", 1.5),
                "overlap_offset": [offset.get(k, 0.0) for k in "XYZ"],
                "allow_no_build": bool(p.get("BuildAllowBuildingInNoBuildZones")),
            }
            if cls not in actors:
                a = actor(dump_of(ref))
                if a:
                    actors[cls] = a
    box = next(e for e in load(ROOT / "Blueprints/BuildingSystem/BP_NoBuildArea.json") if e["Name"] == "Box_GEN_VARIABLE")["Properties"]["BoxExtent"]
    out = {"items": items, "actors": actors, "horde": horde(), "no_build": [box["X"], box["Y"], box["Z"]]}
    OUT.write_text(json.dumps(out, indent=1, sort_keys=True))
    meshes = {i["mesh"] for i in items.values() if i["mesh"]} | {p["mesh"] for a in actors.values() for p in a["parts"]}
    print(f"buildables: {len(items)} items, {len(actors)} actors, {len(meshes)} meshes; horde waves: {len(out['horde']['waves'])}")


if __name__ == "__main__":
    main()
