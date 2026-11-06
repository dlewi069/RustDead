#!/usr/bin/env python3
"""The meshes each interactable Blueprint outlines: assets/data/outlines.json (gameplay/outline.rs).

usage: gen_outlines.py <json dump root>   (scripts/dump_bp.sh's target/bpdump, with bytecode)

Every Blueprint whose interact code calls `<Component>.SetRenderCustomDepth(...)` (OnBeginInteract
turns it on with Video.Outline, OnEndInteract off) gets the mesh stems of those components, read
from its `<Component>_GEN_VARIABLE` templates:

    { "BP_WaterWell_C": ["Mesh_WaterWell"], "BP_Lever_C": ["LeverBase", "Lever"], ... }

A child class inherits its parent's calls, with its own templates' meshes where it overrides them
(BP_Radio_Text_Military's StaticMesh). Components with no mesh on any template (set at runtime,
or GetMesh on a character: '?') are left out; a class with none left is left out.
"""
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "assets" / "data" / "outlines.json"
CALL = re.compile(r"^\s*\d+\s+([\w ?]+?)\.PrimitiveComponent\.SetRenderCustomDepth", re.M)


def stem(ref):
    n = (ref or {}).get("ObjectName", "") if isinstance(ref, dict) else str(ref or "")
    return n.split("'")[1] if "'" in n else None


def main():
    comps, meshes, parent = {}, {}, {}
    for path in sorted(ROOT.rglob("*.json")):
        text = path.read_text()
        cls = path.stem + "_C"
        exports = json.loads(text)
        mine = {}
        for e in exports:
            n = e.get("Name", "")
            if e.get("Type") == "BlueprintGeneratedClass" and n == cls:
                parent[cls] = (stem(e.get("SuperStruct")) or "").removesuffix("_C") + "_C"
            if n.endswith("_GEN_VARIABLE"):
                p = e.get("Properties") or {}
                m = stem(p.get("StaticMesh") or p.get("SkeletalMesh") or p.get("SkinnedAsset"))
                if m:
                    mine[n.removesuffix("_GEN_VARIABLE")] = m
        meshes[cls] = mine
        if "SetRenderCustomDepth" in text:
            code = subprocess.run([sys.executable, str(HERE / "kismet.py"), str(path)], capture_output=True, text=True).stdout
            comps[cls] = list(dict.fromkeys(CALL.findall(code)))

    def lookup(cls, table, seen=()):
        """A class's own entry, else its parent's (children inherit the calls and templates)."""
        if cls in table:
            return table[cls]
        p = parent.get(cls)
        return lookup(p, table, seen + (cls,)) if p and p not in seen else None

    def mesh(cls, comp):
        while cls:
            if comp in meshes.get(cls, {}):
                return meshes[cls][comp]
            cls = parent.get(cls)
        return None

    out = {}
    for cls in meshes:
        called = lookup(cls, comps)
        stems = [m for m in (mesh(cls, c) for c in called or []) if m]
        if stems:
            out[cls] = stems
    OUT.write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")
    print(f"outlines: {len(out)} classes")


if __name__ == "__main__":
    main()
