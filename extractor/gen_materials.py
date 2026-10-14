#!/usr/bin/env python3
"""Map glTF material names to the texture/tint each UE material actually draws with.

usage: gen_materials.py            (run by export_assets.sh after a mesh export)

`SdExport mesh` writes .glb files whose materials carry only a name, plus a `<Material>.json`
stub per material package. Synty-style materials sample one atlas texture, found either in a
material instance's TextureParameterValues or in its master material's
CachedExpressionData.ReferencedTextures. This walks each material's parent chain in
`SdExport json` dumps, exports the textures it finds, and writes assets/game/materials.json:

    { "M_Chr_Texture_1A": { "texture": "game/SurrounDead/Content/.../T.png", "color": [r, g, b, a] } }
"""
import json
import os
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
GAME = HERE.parent / "assets" / "game"
CONTENT = GAME / "SurrounDead" / "Content"
SDX = str(HERE / "bin" / "out" / "SdExport")


def glb_materials(path):
    data = path.read_bytes()
    n = struct.unpack("<I", data[12:16])[0]
    return {m.get("name") for m in json.loads(data[20:20 + n]).get("materials", [])}


def pkg_of(obj_path):
    # "SurrounDead/Content/X/Y.0" -> "SurrounDead/Content/X/Y"
    return obj_path.rsplit(".", 1)[0]


NOT_ALBEDO = ("normal", "_n", "rough", "metal", "mask", "emiss", "ao", "orm", "height", "spec", "opacity", "noise", "grunge",
              "_target", "wov_", "dlwe", "weather", "puddle", "snow")
ALBEDO = ("albedo", "base", "diffuse", "color", "colour", "texture", "_a", "_d")


def pick_albedo(candidates):
    """First candidate whose parameter / texture name looks like colour, else the first that
    isn't obviously a data texture (normal, roughness, masks...)."""
    def name(c):
        return (c[0] + " " + c[1].rsplit("/", 1)[-1]).lower()
    usable = [c for c in candidates if not any(k in name(c) for k in NOT_ALBEDO)]
    for c in usable:
        if any(k in name(c) for k in ALBEDO):
            return c[1]
    return usable[0][1] if usable else None


def run_json(out, pkgs):
    for i in range(0, len(pkgs), 200):
        subprocess.run([SDX, "json", out, *pkgs[i:i + 200]], check=False, capture_output=True)


# Materials swapped in at runtime (skin colour, hair colour) that no mesh references by default.
# Materials only blueprints name (survivor skins/hair; zombie skins from BP_MasterZombie `Materials`).
EXTRA_PREFIXES = ["PlayerModel/Materials/", "PlayerModel/Hair/Material/", "PolygonFiles/PolygonCityZombies/Materials/", "PolygonFiles/PolygonDog/Material/", "Terrain/Foliage/Materials/", "PolygonFiles/PolygonNature/Materials/", "Blueprints/Laboratory/Materials/Status/", "Meshes/Materials/"]


def main():
    used = set()
    for glb in CONTENT.rglob("*.glb"):
        used |= glb_materials(glb)
    used.discard(None)

    # Material stubs from the mesh export tell us each material's package path.
    stub = {p.stem: "SurrounDead/Content/" + str(p.relative_to(CONTENT).with_suffix("")) for p in CONTENT.rglob("*.json")}
    for prefix in EXTRA_PREFIXES:
        listed = subprocess.run([SDX, "list", "SurrounDead/Content/" + prefix], capture_output=True, text=True).stdout.split()
        for f in listed:
            if f.endswith(".uasset"):
                pkg = f[: -len(".uasset")]
                stub.setdefault(pkg.rsplit("/", 1)[1], pkg)
                used.add(pkg.rsplit("/", 1)[1])
    todo = sorted(stub[m] for m in used if m in stub)
    missing = sorted(m for m in used if m not in stub)

    tmp = tempfile.mkdtemp()
    dumps = {}

    def load(pkg):
        if pkg not in dumps:
            f = Path(tmp) / (pkg.removeprefix("SurrounDead/Content/") + ".json")
            if not f.exists():
                run_json(tmp, [pkg])
            dumps[pkg] = json.loads(f.read_text()) if f.exists() else []
        return dumps[pkg]

    run_json(tmp, todo)

    trunks = {}

    def resolve(pkg, depth=0):
        """-> (texture package or None, tint or None) for the first material export in pkg."""
        for e in load(pkg):
            if not e["Type"].startswith("Material"):
                continue
            p = e.get("Properties", {})
            tint = None
            trunk = None
            for v in p.get("VectorParameterValues", []):
                if "trunk" in v["ParameterInfo"]["Name"].lower():
                    c = v["ParameterValue"]
                    trunk = [c["R"], c["G"], c["B"], c.get("A", 1.0)]
                n = v["ParameterInfo"]["Name"].lower()
                # "Color", "TreeColour", "Albedo Tint"... but not trunk/emissive/fresnel colours, nor
                # the zombies' BloodColour (alpha 0: it made every zombie invisible).
                if any(k in n for k in ("colo", "tint")) and not any(k in n for k in ("trunk", "emiss", "fresnel", "fog", "rim", "blood")):
                    c = v["ParameterValue"]
                    tint = [c["R"], c["G"], c["B"], c.get("A", 1.0)]
            params = [(t["ParameterInfo"]["Name"], pkg_of(t["ParameterValue"]["ObjectPath"]))
                      for t in p.get("TextureParameterValues", []) if t.get("ParameterValue")]
            refs = [(t["ObjectName"], pkg_of(t["ObjectPath"]))
                    for t in (e.get("CachedExpressionData") or {}).get("ReferencedTextures", []) if t]
            tex = pick_albedo(params) or pick_albedo(refs)
            if trunk:
                trunks[pkg] = trunk
            if tex is None and "Parent" in p and depth < 8:
                ptex, ptint = resolve(pkg_of(p["Parent"]["ObjectPath"]), depth + 1)
                tex, tint = ptex, tint or ptint
            return tex, tint
        return None, None

    result, textures = {}, set()
    for name in sorted(used):
        if name not in stub:
            continue
        tex, tint = resolve(stub[name])
        entry = {}
        if tex:
            textures.add(tex)
            entry["texture"] = "game/" + tex + ".png"
        if tint:
            entry["color"] = tint
        if stub[name] in trunks and tint and tint[0] > 0.5 and max(tint[1], tint[2]) < 0.1:
            # Mat_Tree_Dark_2/3/4 (the common pines) carry a saturated red TreeColour that the
            # tree shader evidently doesn't multiply in: the game's own screenshots
            # (UI/Textures/Backgrounds) show near-black green pines, i.e. Mat_Tree_Dark's grey.
            entry["color"] = [0.130208, 0.130208, 0.130208, 1.0]
        if stub[name] in trunks:
            # Synty trees: vertices with zero vertex alpha are trunk, drawn in this colour.
            entry["trunk_color"] = trunks[stub[name]]
        result[name] = entry

    need = sorted(t for t in textures if not (GAME / (t + ".png")).exists())
    for i in range(0, len(need), 200):
        subprocess.run([SDX, "tex", str(GAME), *need[i:i + 200]], check=False, capture_output=True)

    (GAME / "materials.json").write_text(json.dumps(result, indent=1, sort_keys=True))
    untextured = [n for n, e in result.items() if "texture" not in e]
    print(f"materials: {len(result)} mapped, {len(untextured)} without texture, {len(missing)} without stub, {len(need)} textures exported")
    if missing:
        print("  no stub:", ", ".join(missing[:20]))


if __name__ == "__main__":
    os.environ.setdefault("DOTNET_ROOT", str(Path.home() / ".dotnet"))
    os.environ.setdefault("SD_USMAP", str(HERE / "mappings" / "Mappings.usmap"))
    sys.exit(main())
