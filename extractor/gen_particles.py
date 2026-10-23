#!/usr/bin/env python3
"""Cascade particle systems (UParticleSystem) as data for gameplay/particles.rs.

usage: gen_particles.py <level dir>   (run by export_assets.sh --particles)

Dumps every ParticleSystem under the FX folders (`SdExport json`) and writes
assets/data/particles.json:

    { "systems": { "FX_Fire": { "emitters": [ { "material": ..., "mesh": ..., "duration": 1.0,
          "loops": 0, "rate": <dist>, "bursts": [[count, low, time]], "lifetime": <dist>,
          "size": <dist>, "color_life": <dist>, ... } ] } },
      "materials": { "Mat_FX": { "blend": "masked", "lit": false } },
      "owners": { "BP_BarrelFire_C": { "system": "FX_Fire", "offset": [12 floats], "visible": true,
          "auto": true, "layer": 50.0 } } }

A <dist> is the module's baked FRawDistribution lookup table, as the engine samples it:
    { "bias": b, "scale": s, "n": entries, "dims": 1|3, "op": "none"|"random"|"extreme",
      "lock": 0-4, "v": [entry 0: min values (+ max values), entry 1: ...] }
Values are converted to Bevy units: locations, velocities and accelerations from UE cm Z-up to
metres Y-up ((x, y, z) = (X, Z, Y) / 100); sizes and life multipliers only swap axes (mesh
particle sizes are scales of the mesh, which the .glb already has in metres; sprite sizes are
cm, so they're divided by 100 as well).

The level's emitters (`SdExport levelparticles` -> <level dir>/particles.json) are resolved
against their owner Blueprints' ParticleSystemComponent templates (Template, bVisible,
bAutoActivate) and AIOSubject's first OptimizationLayers radius, and rewritten in place:
    [ { "owner": "BP_BarrelFire_C", "actor": "BP_BarrelFire2", "system": "FX_Fire",
        "transform": [12 floats], "visible": true, "auto": true, "layer": 50.0 } ]
"""
import json
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
SDX = str(HERE / "bin" / "out" / "SdExport")
FOLDERS = [
    "SurrounDead/Content/PolygonFiles/PolygonApocalypse/ParticleFX/",
    "SurrounDead/Content/PolygonFiles/PolygonMilitary/FX/",
    "SurrounDead/Content/PolygonFiles/PolygonStreetRacer/FX/",
    "SurrounDead/Content/PolygonFiles/FX_Particle_BloodDrip",
    "SurrounDead/Content/SmartAI/Effects/",
    # The gunshot muzzle flashes (BP_WeaponsPickupComponent.MuzzleFlash).
    "SurrounDead/Content/FX/",
]
# EParticleScreenAlignment, EParticleSubUVInterpMethod: the names past the prefix.
LOCKS_SWAPPED = {0: 0, 1: 2, 2: 1, 3: 3, 4: 4}  # UE XY <-> Bevy XZ once Y and Z swap


def sdx(*args):
    return subprocess.run([SDX, *args], capture_output=True, text=True).stdout


def package(name):
    """The package path of a class or asset name (BP_BarrelFire -> SurrounDead/Content/.../BP_BarrelFire)."""
    for p in sdx("list", f"/{name}.uasset").split():
        if p.endswith(f"/{name}.uasset"):
            return p[: -len(".uasset")]
    return None


def dump(tmp, pkgs):
    subprocess.run([SDX, "json", tmp, *pkgs], capture_output=True)


def load(tmp, pkg):
    f = Path(tmp) / (pkg.removeprefix("SurrounDead/Content/") + ".json")
    return json.loads(f.read_text()) if f.exists() else None


def ref_index(obj):
    return int(obj["ObjectPath"].rsplit(".", 1)[1])


def ref_pkg(obj):
    return obj["ObjectPath"].rsplit(".", 1)[0] if obj else None


class Converter:
    """One package's exports; resolves object references by export index."""

    def __init__(self, exports):
        self.e = exports

    def at(self, obj):
        return self.e[ref_index(obj)] if obj else None

    def dist(self, raw, kind="scalar", default=0.0):
        """A FRawDistribution{Float,Vector} as a lookup table in Bevy units. kind: scalar,
        len (cm -> m), color (no conversion), swap (axes only), pos (axes and cm -> m),
        cm (a vector, cm -> m without swapping: sprite sizes)."""
        vec = kind not in ("scalar", "len")
        dims = 3 if vec else 1
        if raw is None:
            v = [default] * dims if not isinstance(default, list) else default
            return {"bias": 0.0, "scale": 0.0, "n": 1, "dims": dims, "op": "none", "lock": 0, "v": v}
        table = raw.get("Table")
        if table and table.get("Values"):
            values = list(table["Values"])
            op = {1: "none", 2: "random", 3: "extreme"}.get(table.get("Op", 1), "none")
            stride = table.get("EntryStride", dims)
            sub = table.get("SubEntryStride", 0)
            n = table.get("EntryCount", 1)
            # Repack each entry as [min..] or [min.., max..] with dims values each.
            entries = []
            for i in range(n):
                e = values[i * stride:(i + 1) * stride]
                lo = e[:dims]
                hi = e[sub:sub + dims] if op != "none" and sub else None
                entries.append((lo, hi))
            out = {"bias": table.get("TimeBias", 0.0), "scale": table.get("TimeScale", 0.0), "n": n, "dims": dims,
                   "op": op, "lock": table.get("LockFlag", 0)}
        else:
            d = self.at(raw.get("Distribution")) if raw.get("Distribution") else None
            p = (d or {}).get("Properties") or {}
            t = (d or {}).get("Type", "")
            out = {"bias": 0.0, "scale": 0.0, "n": 1, "dims": dims, "op": "none", "lock": 0}
            if "Uniform" in t:
                lo, hi = p.get("Min", 0.0), p.get("Max", 0.0)
                if vec:
                    lo, hi = xyz(lo), xyz(hi)
                else:
                    lo, hi = [lo], [hi]
                entries = [(lo, hi)]
                out["op"] = "random"
            elif "ConstantCurve" in t:
                pts = (p.get("ConstantCurve") or {}).get("Points") or [{"InVal": 0.0, "OutVal": 0.0}]
                lo_t, hi_t = pts[0]["InVal"], pts[-1]["InVal"]
                n = 32
                entries = []
                for i in range(n):
                    x = lo_t + (hi_t - lo_t) * i / (n - 1)
                    entries.append(([curve(pts, x, vec)] if not vec else curve(pts, x, vec), None))
                out.update(bias=lo_t, scale=(n - 1) / (hi_t - lo_t) if hi_t > lo_t else 0.0, n=n)
            else:
                c = p.get("Constant", default if not vec else {"X": 0, "Y": 0, "Z": 0})
                entries = [(xyz(c) if vec else [c], None)]
        flat = []
        for lo, hi in entries:
            for part in (lo, hi):
                if part is None:
                    continue
                if kind in ("swap", "pos"):
                    part = [part[0], part[2], part[1]]
                if kind in ("pos", "cm", "len"):
                    part = [x / 100.0 for x in part]
                flat.extend(round(x, 6) for x in part)
        out["v"] = flat
        if kind in ("swap", "pos"):
            out["lock"] = LOCKS_SWAPPED.get(out["lock"], 0)
        return out


def xyz(v):
    if isinstance(v, dict):
        return [v.get("X", 0.0), v.get("Y", 0.0), v.get("Z", 0.0)]
    return [v, v, v]


def curve(pts, x, vec):
    """Linear interpolation of an FInterpCurve's points (curve tangents are ignored)."""
    def out(p):
        return xyz(p["OutVal"]) if vec else p["OutVal"]
    if x <= pts[0]["InVal"]:
        return out(pts[0])
    for a, b in zip(pts, pts[1:]):
        if x <= b["InVal"]:
            f = (x - a["InVal"]) / max(b["InVal"] - a["InVal"], 1e-9)
            oa, ob = out(a), out(b)
            return [u + (w - u) * f for u, w in zip(oa, ob)] if vec else oa + (ob - oa) * f
    return out(pts[-1])


def emitter(c, em, materials):
    lods = (em.get("Properties") or {}).get("LODLevels") or []
    if not lods:
        return None
    lod = c.at(lods[0])["Properties"]
    if lod.get("bEnabled") is False:
        return None
    req = c.at(lod["RequiredModule"])["Properties"]
    mat = ref_pkg(req.get("Material"))
    sprite = True
    out = {
        "name": em["Properties"].get("EmitterName", ""),
        "material": mat.rsplit("/", 1)[-1] if mat else None,
        "duration": req.get("EmitterDuration", 1.0),
        "loops": req.get("EmitterLoops", 0),
        "delay": req.get("EmitterDelay", 0.0),
        "local": req.get("bUseLocalSpace", False),
        "kill_on_completed": req.get("bKillOnCompleted", False),
        "align": req.get("ScreenAlignment", "PSA_Square").split("::")[-1],
        "subuv": [req.get("SubImages_Horizontal", 1), req.get("SubImages_Vertical", 1)],
        "subuv_interp": req.get("InterpolationMethod", "PSUVIM_None").split("::")[-1],
        "max_draw": req.get("MaxDrawCount", 0),
    }
    if mat:
        materials.add(mat)
    spawn = c.at(lod.get("SpawnModule"))
    sp = (spawn or {}).get("Properties") or {}
    rate = c.dist(sp.get("Rate"))
    scale = c.dist(sp.get("RateScale"), default=1.0)
    out["rate"] = rate if sp.get("bProcessSpawnRate", True) else c.dist(None)
    out["rate_scale"] = scale
    out["bursts"] = [[b.get("Count", 0), b.get("CountLow", -1), b.get("Time", 0.0)] for b in sp.get("BurstList") or []] if sp.get("bProcessBurstList", True) else []
    out["burst_scale"] = c.dist(sp.get("BurstScale"), default=1.0)
    td = c.at(lod.get("TypeDataModule"))
    if td:
        tp = td.get("Properties") or {}
        if td["Type"] == "ParticleModuleTypeDataMesh":
            sprite = False
            out["mesh"] = ref_pkg(tp.get("Mesh"))
            out["override_material"] = tp.get("bOverrideMaterial", False)
            out["camera_facing"] = tp.get("bCameraFacing", False)
    out["sprite"] = sprite
    size_kind = "swap" if not sprite else "cm"
    for m in lod.get("Modules") or []:
        mod = c.at(m)
        t, p = mod["Type"].removeprefix("ParticleModule"), mod.get("Properties") or {}
        if p.get("bEnabled") is False:
            continue
        if t == "Lifetime":
            out["lifetime"] = c.dist(p.get("Lifetime"))
        elif t == "Size":
            out["size"] = c.dist(p.get("StartSize"), size_kind, [1.0, 1.0, 1.0])
        elif t == "Velocity":
            out["velocity"] = c.dist(p.get("StartVelocity"), "pos")
            out["velocity_radial"] = c.dist(p.get("StartVelocityRadial"), "len")
            out["velocity_world"] = p.get("bInWorldSpace", False)
            out["velocity_owner_scale"] = p.get("bApplyOwnerScale", False)
        elif t == "VelocityCone":
            d = xyz(p.get("Direction", {"X": 0.0, "Y": 0.0, "Z": 1.0}))
            out["cone"] = {"angle": c.dist(p.get("Angle")), "velocity": c.dist(p.get("Velocity"), "len"),
                           "direction": [d[0], d[2], d[1]], "world": p.get("bInWorldSpace", False)}
        elif t == "LocationPrimitiveSphere":
            out["sphere"] = {"radius": c.dist(p.get("StartRadius"), "len"), "offset": c.dist(p.get("StartLocation"), "pos"),
                             "surface": p.get("SurfaceOnly", False), "velocity": p.get("Velocity", False),
                             "velocity_scale": c.dist(p.get("VelocityScale"), default=1.0),
                             # Positive_X... in UE axes; Bevy's Y is UE Z.
                             "pos": [p.get("Positive_X", True), p.get("Positive_Z", True), p.get("Positive_Y", True)],
                             "neg": [p.get("Negative_X", True), p.get("Negative_Z", True), p.get("Negative_Y", True)]}
        elif t == "LocationPrimitiveCylinder_Seeded" or t == "LocationPrimitiveCylinder":
            out["cylinder"] = {"radius": c.dist(p.get("StartRadius"), "len"), "height": c.dist(p.get("StartHeight"), "len"),
                               "offset": c.dist(p.get("StartLocation"), "pos"), "surface": p.get("SurfaceOnly", False),
                               "radial_velocity": p.get("bRadialVelocity", True), "velocity": p.get("Velocity", False),
                               "velocity_scale": c.dist(p.get("VelocityScale"), default=1.0),
                               "axis": p.get("HeightAxis", "PMLPC_HEIGHTAXIS_Z").split("::")[-1]}
        elif t == "Location":
            out["location"] = c.dist(p.get("StartLocation"), "pos")
        elif t == "Color":
            out["color"] = c.dist(p.get("StartColor"), "color", [1.0, 1.0, 1.0])
            out["alpha"] = c.dist(p.get("StartAlpha"), default=1.0)
        elif t == "ColorOverLife":
            out["color_life"] = c.dist(p.get("ColorOverLife"), "color", [1.0, 1.0, 1.0])
            out["alpha_life"] = c.dist(p.get("AlphaOverLife"), default=1.0)
        elif t == "ColorScaleOverLife":
            # Two of these multiply together.
            out.setdefault("color_scale_life", []).append({
                "color": c.dist(p.get("ColorScaleOverLife"), "color", [1.0, 1.0, 1.0]),
                "alpha": c.dist(p.get("AlphaScaleOverLife"), default=1.0),
                "emitter_time": p.get("bEmitterTime", False)})
        elif t == "SizeMultiplyLife":
            axes = [p.get("MultiplyX", True), p.get("MultiplyZ", True), p.get("MultiplyY", True)]
            out.setdefault("size_life", []).append({"v": c.dist(p.get("LifeMultiplier"), "swap", [1.0, 1.0, 1.0]), "axes": axes})
        elif t == "SizeScaleBySpeed":
            s, mx = p.get("SpeedScale", {"X": 1.0, "Y": 1.0}), p.get("MaxScale", {"X": 1.0, "Y": 1.0})
            out["size_by_speed"] = {"scale": [s.get("X", 1.0), s.get("Y", 1.0)], "max": [mx.get("X", 1.0), mx.get("Y", 1.0)]}
        elif t == "AccelerationDrag":
            out["drag"] = c.dist(p.get("DragCoefficientRaw"))
        elif t == "Acceleration":
            out.setdefault("accel", []).append(c.dist(p.get("Acceleration"), "pos"))
        elif t == "AccelerationConstant":
            a = xyz(p.get("Acceleration", {"X": 0, "Y": 0, "Z": 0}))
            out["accel_constant"] = [a[0] / 100.0, a[2] / 100.0, a[1] / 100.0]
        elif t == "AccelerationOverLifetime":
            out["accel_life"] = c.dist(p.get("AccelOverLife"), "pos")
        elif t == "VelocityOverLifetime":
            out["velocity_life"] = {"v": c.dist(p.get("VelOverLife"), "swap", [1.0, 1.0, 1.0]), "absolute": p.get("Absolute", False)}
        elif t == "Rotation":
            out["rotation"] = c.dist(p.get("StartRotation"))
        elif t == "RotationRate":
            out["rotation_rate"] = c.dist(p.get("StartRotationRate"))
        elif t == "MeshRotation":
            out["mesh_rotation"] = c.dist(p.get("StartRotation"), "swap")
        elif t == "SubUV":
            out["subuv_index"] = c.dist(p.get("SubImageIndex"))
        elif t == "Light":
            out["light"] = {"brightness": c.dist(p.get("BrightnessOverLife"), default=1.0),
                            "color": c.dist(p.get("ColorScaleOverLife"), "color", [1.0, 1.0, 1.0]),
                            "radius": c.dist(p.get("RadiusScale"), default=1.0),  # x the particle size
                            "exponent": c.dist(p.get("LightExponent"), default=16.0),
                            "ratio": p.get("SpawnFraction", 1.0)}
    out.setdefault("lifetime", c.dist(None))
    return out


def system(exports, materials):
    c = Converter(exports)
    ps = next((e for e in exports if e["Type"] == "ParticleSystem"), None)
    if ps is None:
        return None
    ems = [emitter(c, c.at(r), materials) for r in (ps.get("Properties") or {}).get("Emitters") or []]
    return {"emitters": [e for e in ems if e]}


def material(exports):
    m = next((e for e in exports if e["Type"] in ("Material", "MaterialInstanceConstant")), None)
    p = (m or {}).get("Properties") or {}
    blend = p.get("BlendMode", "BLEND_Opaque").split("_")[-1].lower()
    shading = p.get("ShadingModel", "MSM_DefaultLit")
    # The cooked materials keep no expression graph; the Synty FX ones draw the particle colour.
    return {"blend": blend, "lit": not shading.endswith("Unlit")}


def glb_material(path):
    try:
        data = path.read_bytes()
    except OSError:
        return None
    n = struct.unpack("<I", data[12:16])[0]
    mats = json.loads(data[20:20 + n]).get("materials") or []
    return mats[0].get("name") if mats else None


def owners(tmp, level):
    """Each level emitter's owner Blueprint: its ParticleSystemComponent template and AIO layer."""
    f = level / "particles.json"
    if not f.exists():
        return [], {}
    emitters = json.loads(f.read_text())
    classes = sorted({e["owner"] for e in emitters})
    pkgs = {cls: package(cls.removesuffix("_C")) for cls in classes}
    dump(tmp, [p for p in pkgs.values() if p])
    found = {}
    for cls, pkg in pkgs.items():
        ex = load(tmp, pkg) if pkg else None
        if not ex:
            continue
        layer = None
        comps = {}
        for e in ex:
            p = e.get("Properties") or {}
            if e["Type"] == "AIOSubjectComponent" and p.get("OptimizationLayers"):
                layer = p["OptimizationLayers"][0].get("LayerRadius", 0.0) / 100.0
            if e["Type"] == "ParticleSystemComponent" and e["Name"].endswith("_GEN_VARIABLE"):
                comps[e["Name"].removesuffix("_GEN_VARIABLE")] = p
        found[cls] = (layer, comps)
    out = []
    for e in emitters:
        if "system" in e:
            out.append(e)  # already resolved
            continue
        layer, comps = found.get(e["owner"], (None, {}))
        tpl = comps.get(e["component"], {})
        system = e.get("template") if "Template" in e["set"] else ref_pkg(tpl.get("Template"))
        if not system:
            continue
        visible = e["visible"] if "bVisible" in e["set"] else tpl.get("bVisible", True)
        auto = e["auto"] if "bAutoActivate" in e["set"] else tpl.get("bAutoActivate", True)
        out.append({"owner": e["owner"], "actor": e.get("actor", ""), "component": e["component"],
                    "system": system.rsplit("/", 1)[-1], "transform": e["transform"],
                    "visible": bool(visible), "auto": bool(auto), "layer": layer})
    f.write_text(json.dumps(out))
    return out, found


def main():
    level = Path(sys.argv[1]) if len(sys.argv) > 1 else None
    tmp = tempfile.mkdtemp()
    pkgs = []
    for folder in FOLDERS:
        for p in sdx("list", folder).split():
            if p.endswith(".uasset") and p.startswith(folder):
                pkgs.append(p[: -len(".uasset")])
    dump(tmp, pkgs)
    systems, materials = {}, set()
    for pkg in pkgs:
        ex = load(tmp, pkg)
        if not ex:
            continue
        s = system(ex, materials)
        if s and s["emitters"]:
            systems[pkg.rsplit("/", 1)[-1]] = s
    # Mesh emitters that don't override the material draw the mesh's own (its first glTF material).
    for s in systems.values():
        for e in s["emitters"]:
            if e.get("mesh") and not e.get("override_material"):
                name = glb_material(ROOT / "assets" / "game" / (e["mesh"] + ".glb"))
                pkg = package(name) if name and name != "WorldGridMaterial" else None
                if pkg:
                    materials.add(pkg)
                    e["material"] = name
    dump(tmp, sorted(m for m in materials if not m.startswith("Engine/")))
    mats = {}
    for m in sorted(materials):
        ex = load(tmp, m) if not m.startswith("Engine/") else None
        mats[m.rsplit("/", 1)[-1]] = material(ex) if ex else {"blend": "opaque", "lit": True}
    placed, _ = owners(tmp, level) if level else ([], {})
    data = {"systems": systems, "materials": mats}
    (ROOT / "assets" / "data" / "particles.json").write_text(json.dumps(data, indent=1))
    meshes = sorted({e["mesh"] for s in systems.values() for e in s["emitters"] if e.get("mesh")})
    (HERE / "mappings" / "particle_meshes.txt").write_text("\n".join(meshes) + "\n")
    print(f"particles: {len(systems)} systems, {sum(len(s['emitters']) for s in systems.values())} emitters, "
          f"{len(mats)} materials, {len(meshes)} meshes; {len(placed)} level emitters")


if __name__ == "__main__":
    main()
