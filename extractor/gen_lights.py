#!/usr/bin/env python3
"""Resolve the level's point lights against their Blueprints' component templates.

usage: gen_lights.py <level dir>   (run by export_assets.sh --level after `SdExport levellights`)

`SdExport levellights` writes each PointLightComponent's world position and the properties the
map overrides (`set`); everything else is the owner Blueprint's component template
(`<Component>_GEN_VARIABLE`), then the engine default. This dumps the owner Blueprints and
rewrites lights.json in place with resolved values:

    [ { "owner": "BP_BarrelFire_C", "pos": [x, y, z], "candela": 8.0, "color": [r, g, b],
        "radius": 10.0, "shadows": true, "visible": true } ]

Intensity is converted to candelas: UE's Unitless point-light intensity is candela x 625
(the 5000 default = 8 cd), Lumens / 4pi.
"""
import json
import math
import os
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SDX = str(HERE / "bin" / "out" / "SdExport")
LEVEL = Path(sys.argv[1])
DEFAULTS = {"Intensity": 5000.0, "IntensityUnits": "ELightUnits::Unitless", "AttenuationRadius": 1000.0,
            "LightColor": {"R": 255, "G": 255, "B": 255}, "CastShadows": True, "bVisible": True}


def candela(intensity, units):
    if units.endswith("Candelas"):
        return intensity
    if units.endswith("Lumens"):
        return intensity / (4 * math.pi)
    return intensity / 625.0


def main():
    lights = json.loads((LEVEL / "lights.json").read_text())
    if lights and "candela" in lights[0]:
        return  # already resolved
    # Owners are actor names (BP_Campfire2_5); actors.json has their classes.
    class_of = {a["name"]: a["class"] for a in json.loads((LEVEL / "actors.json").read_text())}
    for l in lights:
        l["owner"] = class_of.get(l["owner"], l["owner"])
    classes = sorted({l["owner"] for l in lights})
    tmp = tempfile.mkdtemp()
    templates = {}
    for cls in classes:
        name = cls.removesuffix("_C")
        listed = subprocess.run([SDX, "list", f"/{name}.uasset"], capture_output=True, text=True).stdout.split()
        pkg = next((p[: -len(".uasset")] for p in listed if p.endswith(f"/{name}.uasset")), None)
        if not pkg:
            continue
        subprocess.run([SDX, "json", tmp, pkg], capture_output=True)
        f = Path(tmp) / (pkg.removeprefix("SurrounDead/Content/") + ".json")
        if not f.exists():
            continue
        for e in json.loads(f.read_text()):
            if e["Name"].endswith("_GEN_VARIABLE") and e["Type"] == "PointLightComponent":
                templates[(cls, e["Name"].removesuffix("_GEN_VARIABLE"))] = e.get("Properties") or {}
    # The map's own values for overridden properties.
    out = []
    for l in lights:
        cls = l["owner"]
        p = dict(DEFAULTS)
        p.update(templates.get((cls, l["component"]), {}))
        if "Intensity" in l["set"]:
            p["Intensity"] = l["intensity"]
        if "AttenuationRadius" in l["set"]:
            p["AttenuationRadius"] = l["radius"] * 100.0
        if "CastShadows" in l["set"]:
            p["CastShadows"] = l["shadows"]
        if "bVisible" in l["set"]:
            p["bVisible"] = l["visible"]
        col = p["LightColor"] if "LightColor" not in l["set"] else {"R": l["color"][0] * 255, "G": l["color"][1] * 255, "B": l["color"][2] * 255}
        out.append({
            "owner": cls,
            "component": l["component"],
            "pos": l["pos"],
            "candela": round(candela(float(p["Intensity"]), str(p.get("IntensityUnits", "Unitless"))), 4),
            "color": [col.get("R", 255) / 255.0, col.get("G", 255) / 255.0, col.get("B", 255) / 255.0],
            "radius": float(p["AttenuationRadius"]) / 100.0,
            "shadows": bool(p["CastShadows"]),
            "visible": bool(p["bVisible"]),
        })
    (LEVEL / "lights.json").write_text(json.dumps(out))
    print(f"lights: {len(out)} ({len(templates)} templates from {len(classes)} Blueprints)")


if __name__ == "__main__":
    main()
