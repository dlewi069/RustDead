#!/usr/bin/env python3
"""Landscape grass (Terrain/Foliage/*Foliage LandscapeGrassType) -> assets/data/grass.json.

usage: gen_grass.py <json dump root>   (run by export_assets.sh --level)

Terrain_Material's grass output pairs each paint layer with the grass type of the same name
(Grass_LayerInfo -> GrassFoliage, ...); the graph isn't readable, the names match 1:1. Each
variety keeps the asset's values, in metres:

    { "Grass_LayerInfo": [ { "mesh": "SurrounDead/Content/.../SM_GrassMeshPatch_03",
        "material": "Mat_Grass_Card" | null, "density": 40 (per 10 x 10 m), "cull": [70, 80],
        "scale": [[1, 1.5], [1, 1.5], [1, 1.5]], "uniform": false, "grid": true, "jitter": 1,
        "random_rotation": true, "align": true, "shadows": true } ] }

Prints the meshes to export (one package per line) on stdout.
"""
import json
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
OUT = Path(__file__).resolve().parent.parent / "assets" / "data" / "grass.json"


def value(v, default):
    """Per-platform values are {"Default": x, ...}."""
    if isinstance(v, dict):
        return v.get("Value", v.get("Default", default))
    return default if v is None else v


def main():
    out, meshes = {}, set()
    for f in sorted((ROOT / "Terrain/Foliage").glob("*Foliage.json")):
        layer = f.stem.removesuffix("Foliage") + "_LayerInfo"
        for e in json.loads(f.read_text()):
            if e["Type"] != "LandscapeGrassType":
                continue
            rows = []
            for v in (e.get("Properties") or {}).get("GrassVarieties", []):
                mesh = (v.get("GrassMesh") or {}).get("ObjectPath", "").split(".")[0]
                if not mesh:
                    continue
                meshes.add(mesh)
                mats = [m for m in v.get("OverrideMaterials") or [] if m]
                scale = [[v.get(k, {}).get("Min", 1.0), v.get(k, {}).get("Max", 1.0)] for k in ("ScaleX", "ScaleY", "ScaleZ")]
                rows.append({
                    "mesh": mesh,
                    "material": mats[0]["ObjectPath"].split(".")[0].rsplit("/", 1)[1] if mats else None,
                    "density": value(v.get("GrassDensity"), 400.0),
                    "cull": [value(v.get("StartCullDistance"), 10000) / 100.0, value(v.get("EndCullDistance"), 10000) / 100.0],
                    "scale": scale,
                    # EGrassScaling: Uniform uses the X range for all axes, Free each its own.
                    "uniform": v.get("Scaling", "EGrassScaling::Uniform").endswith("Uniform"),
                    "grid": bool(v.get("bUseGrid", True)),
                    "jitter": v.get("PlacementJitter", 1.0),
                    "random_rotation": bool(v.get("RandomRotation", True)),
                    "align": bool(v.get("AlignToSurface", True)),
                    "shadows": bool(v.get("bCastDynamicShadow", True)),
                })
            out[layer] = rows
    OUT.write_text(json.dumps(out, indent=1, sort_keys=True))
    print("\n".join(sorted(meshes)))
    print(f"grass: {sum(len(v) for v in out.values())} varieties on {len(out)} layers", file=sys.stderr)


if __name__ == "__main__":
    main()
