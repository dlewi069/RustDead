#!/usr/bin/env python3
"""Zombie classes (BP_MasterZombie and its children) -> assets/data/zombies.json.

usage: gen_ai.py <json dump root>   (normally run via `export_assets.sh --ai`)

Every value is the class default, inherited down the Blueprint chain (child overrides parent):
the CDO's speeds/damage/medical chances, the DamageComponent, capsule and SD_AIComponent
templates, and the locomotion blendspace's samples. Lengths in metres, speeds in m/s.

    { "classes": { "BP_Zombie_Roamer_C": { "name": "Undead Shambler", "health": 100, ... } } }
"""
import json
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
OUT = Path(__file__).resolve().parent.parent / "assets" / "data" / "zombies.json"
CONTENT = "SurrounDead/Content/"
# ACharacter / BP_MasterZombie engine defaults when nothing in the chain sets them.
CAPSULE = {"CapsuleHalfHeight": 88.0, "CapsuleRadius": 34.0}


def field(d, prefix):
    """Blueprint struct fields carry GUID suffixes: ContainerName_13_2E87..."""
    return next((v for k, v in d.items() if k.split("_")[0] == prefix), None)


def load(p):
    return json.loads(p.read_text())


def pkg(ref):
    return ref["ObjectPath"].split(".")[0] if ref else None


def file_of(package):
    return ROOT / (package.removeprefix(CONTENT) + ".json")


def classes():
    """class name -> {super, cdo, components{type: props}} for every Blueprint under AI/."""
    out = {}
    for f in (ROOT / "AI").rglob("*.json"):
        exports = load(f)
        cls = next((e for e in exports if e["Type"] == "BlueprintGeneratedClass"), None)
        if not cls:
            continue
        comps = {}
        for e in exports:
            if e["Name"].endswith("_GEN_VARIABLE") or e["Type"] in ("CapsuleComponent", "CharacterMovementComponent"):
                comps.setdefault(e["Type"], {}).update(e.get("Properties") or {})
        out[cls["Name"]] = {
            "super": (cls.get("Super") or {}).get("ObjectName", "").split("'")[1] if cls.get("Super") else None,
            "cdo": next((e.get("Properties") or {} for e in exports if e["Name"].startswith("Default__")), {}),
            "components": comps,
        }
    return out


def ai_component_default(key):
    f = ROOT / "AI/SD_AIComponent.json"
    if not f.exists():
        return None
    return next(((e.get("Properties") or {}).get(key) for e in load(f) if e["Name"].startswith("Default__")), None)


def blendspace(path):
    f = file_of(path)
    if not f.exists():
        return []
    for e in load(f):
        samples = (e.get("Properties") or {}).get("SampleData")
        if samples:
            rows = [(pkg(s["Animation"]).removeprefix(CONTENT), s["SampleValue"].get("X", 0.0) / 100.0) for s in samples if s.get("Animation")]
            return sorted(set(rows), key=lambda r: (r[1], r[0]))
    return []


def main():
    cls = classes()

    def chain(name):
        while name in cls:
            yield cls[name]
            name = cls[name]["super"]

    def get(name, key, default=None):
        return next((c["cdo"][key] for c in chain(name) if key in c["cdo"]), default)

    def comp(name, ctype, key, default=None):
        return next((c["components"][ctype][key] for c in chain(name) if key in c["components"].get(ctype, {})), default)

    enum_file = ROOT / "AI/Enum_AILootTables.json"
    e = next(x for x in load(enum_file) if x["Type"] == "UserDefinedEnum")
    shown = {m["Key"]: m["Value"].get("SourceString") for m in e["Properties"]["DisplayNameMap"]}
    loot_by_value = {v: shown.get(k.split("::")[1]) for k, v in e["Names"].items()}

    out = {}
    for name in cls:
        if "BP_MasterZombie_C" not in iter_supers(cls, name):
            continue
        loot = get(name, "LootTable")
        # Unset in the chain: SD_AIComponent's own default (the Roamer's ZombieAILocomotion).
        bs = comp(name, "SD_AIComponent_C", "MovementBlendspace") or ai_component_default("MovementBlendspace")
        out[name] = {
            "name": (get(name, "Name") or {}).get("SourceString"),
            "meshes": [pkg(m).removeprefix(CONTENT) for m in get(name, "Meshes", []) if m],
            "materials": [pkg(m).rsplit("/", 1)[1] for m in get(name, "Materials", []) if m],
            "damage": get(name, "DamageToDo", 12.0),
            "roam_speed": get(name, "Roaming Speed", 100.0) / 100.0,
            "alert_speed": get(name, "Alert Speed", 425.0) / 100.0,
            "health": comp(name, "DamageComponent_C", "StandardHealthValue", 100.0),
            "health_deviation": get(name, "HealthDeviation", 10.0),
            "attack_interval": [get(name, "TimeBetweenAttacksMin", 1.45), get(name, "TimeBetweenAttacksMax", 2.05)],
            "bleed": get(name, "Chance for Bleed", 0.0) / 100.0,
            "heavy_bleed": get(name, "Chance for Heavy Bleed", 0.0) / 100.0,
            "broken_bone": get(name, "Chance for Broken Bone", 0.0) / 100.0,
            "rad_chance": get(name, "Chance for Rad Damage", 0.0) / 100.0,
            "rad_damage": get(name, "Radiation Damage", 0.0),
            "xp": [get(name, "XPMultiplier_Min", 5.0), get(name, "XPMultiplier_Max", 10.0)],
            "loot_table": loot_by_value.get(e["Names"].get(loot, 0) if loot else 0),
            # Damage_Shoved does nothing without a ShovedAnimation (crawlers and hounds clear it).
            "shovable": get(name, "ShovedAnimation") is not None,
            # BP_JigMultiplayer's container: what the corpse's loot grid is called.
            "container": next((field(s, "ContainerName") for c in chain(name) for s in c["components"].get("BP_JigMultiplayer_C", {}).get("ContainersSettings", [])[:1]), None),
            "capsule_half_height": comp(name, "CapsuleComponent", "CapsuleHalfHeight", CAPSULE["CapsuleHalfHeight"]) / 100.0,
            "capsule_radius": comp(name, "CapsuleComponent", "CapsuleRadius", CAPSULE["CapsuleRadius"]) / 100.0,
            # SD_AIComponent.MovementBlendspace samples: (clip under Content/, speed m/s).
            "locomotion": blendspace(pkg(bs)) if bs else [],
        }
    OUT.write_text(json.dumps({"classes": out}, indent=1, sort_keys=True))
    print(f"zombie classes: {sorted(out)}")


def iter_supers(cls, name):
    name = cls.get(name, {}).get("super")
    while name:
        yield name
        name = cls.get(name, {}).get("super")


if __name__ == "__main__":
    main()
