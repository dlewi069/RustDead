#!/usr/bin/env python3
"""Infestation class data for gameplay/infestations.rs: assets/data/infestations.json.

usage: gen_infestations.py <json dump root>   (export_assets.sh --infestations, or scripts/dump_bp.sh's
                                              target/bpdump, which has the same layout)

Reads BP_Infestation and its children (Floor / Wall / Ceiling / Corner / BP_PrimaryInfestation),
walking SuperStruct so a child gets its parent's values unless it sets its own, plus
BP_RandomInfestationSpawnPoint's defaults:

    { "classes": { "BP_Infestation_Floor_C": {
          "name": "Secondary Infestation", "primary": false, "show_marker": false,
          "health": 1000.0, "max_health": 1000.0, "deviation": 0.0,
          "mesh": "SurrounDead/Content/.../InfestationFloor", "destroyed_mesh": ".../InfestationFloor_D",
          "armor": null, "loot_table": "Infestation", "unique": [],
          "post_process_radius": 5000.0, "spawn_z_offset": 0.0,
          "waves": [[["BP_Zombie_Infected_Horde_C", 6]], ...],
          "explosion": [x, y, z], "flies": [x, y, z],              (UE cm, the actor's frame)
          "lights": [{"component": "PurpleLight", "at": [x, y, z], "color": [r, g, b], "radius": 5.0,
                      "intensity": 1.0, "inverse_squared": false}] } },
      "random_point": { "chance": 20.0, "mesh": "...", "destroyed_mesh": "..." } }

Per-instance values (a primary's SecondaryInfestations and Splines, AISpawningTransforms,
PostProcessRadius, a spawn point's RandomChance and meshes) are in the level's actors.json `props`.
"""
import json
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
OUT = Path(__file__).resolve().parent.parent / "assets" / "data" / "infestations.json"
CLASSES = ["BP_Infestation", "BP_Infestation_Floor", "BP_Infestation_Wall", "BP_Infestation_Ceiling", "BP_Infestation_Corner", "BP_PrimaryInfestation"]


def load(path):
    return json.loads(path.read_text())


def pkg_path(ref):
    """An object ref's package path without the export index (".../InfestationWall.2")."""
    return ref["ObjectPath"].rsplit(".", 1)[0] if ref else None


def class_name(ref):
    return ref["ObjectName"].split("'")[1] if ref else None


def field(struct, stem):
    """A user struct's field by its name stem (fields are "Amount_12_<guid>")."""
    return next((v for k, v in struct.items() if k.split("_")[0] == stem or k.rsplit("_", 2)[0] == stem), None)


def chain(name):
    """The class and its Blueprint parents, root first: [(exports, default props, templates)]."""
    out = []
    f = next(ROOT.rglob(f"{name}.json"), None)
    while f is not None:
        exports = load(f)
        cls = next(e for e in exports if e.get("Type") == "BlueprintGeneratedClass")
        cdo = next(e for e in exports if e["Name"].startswith("Default__"))
        templates = {e["Name"].removesuffix("_GEN_VARIABLE"): e.get("Properties") or {} for e in exports if e["Name"].endswith("_GEN_VARIABLE")}
        out.append((cdo.get("Properties") or {}, templates))
        parent = (cls.get("SuperStruct") or {}).get("ObjectPath", "")
        f = ROOT / (parent.removeprefix("SurrounDead/Content/").rsplit(".", 1)[0] + ".json") if parent.startswith("SurrounDead/") else None
        if f is not None and not f.exists():
            f = None
    return list(reversed(out))


def merged(links):
    """Defaults and component templates, children over parents."""
    props, comps = {}, {}
    for p, t in links:
        props.update(p)
        for k, v in t.items():
            comps.setdefault(k, {}).update(v)
    return props, comps


def rel(comp):
    v = (comp or {}).get("RelativeLocation") or {}
    return [v.get("X", 0.0), v.get("Y", 0.0), v.get("Z", 0.0)]


def main():
    enum = {}
    for e in load(ROOT / "Inventory/Containers/Enum_ContainerLootTables.json"):
        for kv in (e.get("Properties") or {}).get("DisplayNameMap", []):
            enum[f"Enum_ContainerLootTables::{kv['Key']}"] = kv["Value"].get("SourceString") or kv["Value"].get("CultureInvariantString")
    classes = {}
    for name in CLASSES:
        props, comps = merged(chain(name))
        dmg = comps.get("DamageComponent", {})
        waves = []
        for w in field(props.get("WavesOfEnemies") or {}, "WaveInfo") or []:
            ai, amount = field(w, "AI") or [], field(w, "Amount") or []
            waves.append([[class_name(c), n] for c, n in zip(ai, amount)])
        lights = []
        for comp in ("PurpleLight", "RedLight"):
            c = comps.get(comp, {})
            col = c.get("LightColor") or {"R": 255, "G": 255, "B": 255}
            lights.append({
                "component": comp,
                "at": rel(c),
                "color": [col["R"] / 255.0, col["G"] / 255.0, col["B"] / 255.0],
                # PointLightComponent defaults: AttenuationRadius 1000 cm, Intensity 5000.
                "radius": c.get("AttenuationRadius", 1000.0) / 100.0,
                "intensity": c.get("Intensity", 5000.0),
                "inverse_squared": c.get("bUseInverseSquaredFalloff", True),
            })
        primary = bool(props.get("PrimaryInfestation?", False))
        classes[name + "_C"] = {
            "name": (props.get("Name") or {}).get("SourceString", "Infestation"),
            "primary": primary,
            "show_marker": bool(props.get("ShowMarker?", False)),
            "health": dmg.get("StandardHealthValue", 100.0),
            "max_health": dmg.get("MaxHealth", 100.0),
            "deviation": dmg.get("HealthRandomDeviation", 0.0),
            "mesh": pkg_path(props.get("NormalMesh")),
            "destroyed_mesh": pkg_path(props.get("DestroyedMesh")),
            # The Armor component is in every class; the construction script shows it on primaries only.
            "armor": pkg_path(comps.get("Armor", {}).get("StaticMesh")) if primary else None,
            "loot_table": enum.get(props.get("LootTable", ""), None),
            "unique": props.get("UniqueLootSets") or [],
            "post_process_radius": props.get("PostProcessRadius", 0.0),
            "spawn_z_offset": props.get("SpawnZOffset", 0.0),
            "waves": waves,
            "explosion": rel(comps.get("ExplosionLoc")),
            "flies": rel(comps.get("Flies")),
            "lights": lights,
        }
    sp, _ = merged(chain("BP_RandomInfestationSpawnPoint"))
    out = {
        "classes": classes,
        "random_point": {"chance": sp.get("RandomChance", 0.0), "mesh": pkg_path(sp.get("NormalMesh")), "destroyed_mesh": pkg_path(sp.get("DestroyedMesh"))},
    }
    OUT.write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")
    print(f"infestations: {len(classes)} classes -> {OUT}")


if __name__ == "__main__":
    main()
