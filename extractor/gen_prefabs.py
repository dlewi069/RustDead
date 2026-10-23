#!/usr/bin/env python3
"""Prefabs (Content/Prefabs: camps, convoys, crashed helicopters, supply trucks...) for level/prefabs.rs.

usage: gen_prefabs.py   (run by export_assets.sh --prefabs)

A prefab is a Blueprint (a PrefabMaster child) whose components are static meshes, particle
systems and child actors (containers, zombie spawners, AI spawning volumes, campfires...). This
dumps every prefab and, recursively, the Blueprints of their child actors and parent classes,
resolves each component's place (its SCS parent chain, templates merged down the class chain:
a child class's template holds only what it changes), and writes assets/data/prefabs.json:

    { "prefabs": { "Prefab_AbandonedCamp_C": {
        "foliage_radius": 20.0,
        "meshes": [ ["<mesh package>", [12 floats]] ],
        "actors": [ { "class": "Container_AmmoLoot_AmmoCrate_C", "transform": [12 floats],
                      "props": { "<property>": "<JSON text>" } } ],
        "particles": [ ["FX_Smoke", [12 floats]] ],
        "humans": ["BP_Bandit_Melee_C"] } } }

Transforms are relative to the prefab's root, in Bevy space (the level export's convention:
(x, y, z) = (X, Z, Y) / 100, a column-major 3x3 then the translation). Meshes include those of
the child actors' own Blueprints (a container's crate, a campfire's logs). `humans` lists the
human AI classes its spawning volumes spawn (prefabs.rs leaves those prefabs out until human AI
is ported). The mesh packages not yet exported are written to mappings/prefab_meshes.txt.
"""
import json
import math
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
SDX = str(HERE / "bin" / "out" / "SdExport")
GAME = ROOT / "assets" / "game"
FOLDERS = ["SurrounDead/Content/Prefabs/"]
# Child actors whose spawning volumes hold human AI (bandits, the Coyotes, scavengers, survivors).
HUMAN_SPAWNERS = {"BP_AISpawningVolume_Prefab_C", "BP_AISpawningVolume_RandomChanceToSpawn_C", "BP_AISpawner_SettlementSurvivors_C"}


def pkg_of(path):
    """An ObjectPath ("SurrounDead/Content/X/Y.3" or "/Game/X/Y.Y_C") as a package path."""
    p = path.rsplit(".", 1)[0]
    if p.startswith("/Game/"):
        p = "SurrounDead/Content/" + p[len("/Game/"):]
    return p.lstrip("/")


class Dumps:
    def __init__(self):
        self.tmp = tempfile.mkdtemp()
        self.cache = {}

    def file(self, pkg):
        return Path(self.tmp) / (pkg.removeprefix("SurrounDead/Content/") + ".json")

    def ensure(self, pkgs):
        todo = sorted({p for p in pkgs if p.startswith("SurrounDead/") and p not in self.cache and not self.file(p).exists()})
        for i in range(0, len(todo), 150):
            subprocess.run([SDX, "json", self.tmp, *todo[i:i + 150]], capture_output=True)

    def get(self, pkg):
        if pkg not in self.cache:
            f = self.file(pkg)
            self.cache[pkg] = json.loads(f.read_text()) if f.exists() else []
        return self.cache[pkg]


def refs(exports):
    """Class packages a Blueprint refers to: its parent class and its child actors' classes."""
    out = set()
    for e in exports:
        if e["Type"] == "BlueprintGeneratedClass" and e.get("SuperStruct"):
            out.add(pkg_of(e["SuperStruct"]["ObjectPath"]))
        cls = (e.get("Properties") or {}).get("ChildActorClass")
        if cls:
            out.add(pkg_of(cls["ObjectPath"]))
    return out


def chain(d, pkg):
    """The Blueprint and its parent Blueprints, most derived first."""
    out, seen = [], set()
    while pkg and pkg not in seen and pkg.startswith("SurrounDead/"):
        seen.add(pkg)
        ex = d.get(pkg)
        if not ex:
            break
        out.append((pkg, ex))
        cls = next((e for e in ex if e["Type"] == "BlueprintGeneratedClass"), None)
        sup = (cls or {}).get("SuperStruct")
        pkg = pkg_of(sup["ObjectPath"]) if sup else None
    return out


def components(d, pkg):
    """name -> {"type", "props", "export", "pkg"} merged down the class chain, and name -> parent."""
    comps, parents = {}, {}
    for p, ex in reversed(chain(d, pkg)):
        nodes = {i: e for i, e in enumerate(ex) if e["Type"] == "SCS_Node"}
        for i, e in enumerate(ex):
            name = e["Name"]
            if name.endswith("_GEN_VARIABLE"):
                n = name[: -len("_GEN_VARIABLE")].removeprefix("ICH-")
                c = comps.setdefault(n, {"type": e["Type"], "props": {}, "export": None, "pkg": p})
                c["props"].update(e.get("Properties") or {})
                c["type"], c["export"], c["pkg"] = e["Type"], e, p
        for i, e in nodes.items():
            pr = e.get("Properties") or {}
            n = pr.get("InternalVariableName")
            if not n:
                continue
            if pr.get("ParentComponentOrVariableName"):
                parents[n] = pr["ParentComponentOrVariableName"]
            for c in pr.get("ChildNodes") or []:
                k = int(c["ObjectPath"].rsplit(".", 1)[1])
                child = (ex[k].get("Properties") or {}).get("InternalVariableName") if k < len(ex) else None
                if child:
                    parents[child] = n
    return comps, parents


def rel_matrix(props):
    """A component's relative transform as a 4x4 column-vector matrix in UE space (cm)."""
    loc = props.get("RelativeLocation") or {}
    rot = props.get("RelativeRotation") or {}
    sc = props.get("RelativeScale3D") or {"X": 1.0, "Y": 1.0, "Z": 1.0}
    p, y, r = (math.radians(rot.get(k, 0.0)) for k in ("Pitch", "Yaw", "Roll"))
    sp, cp, sy, cy, sr, cr = math.sin(p), math.cos(p), math.sin(y), math.cos(y), math.sin(r), math.cos(r)
    # FRotationMatrix's rows are the rotated X, Y and Z axes.
    ax = [cp * cy, cp * sy, sp]
    ay = [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp]
    az = [-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp]
    s = [sc.get("X", 1.0), sc.get("Y", 1.0), sc.get("Z", 1.0)]
    m = [[0.0] * 4 for _ in range(4)]
    for col, (axis, k) in enumerate(zip((ax, ay, az), s)):
        for row in range(3):
            m[row][col] = axis[row] * k
    m[0][3], m[1][3], m[2][3], m[3][3] = loc.get("X", 0.0), loc.get("Y", 0.0), loc.get("Z", 0.0), 1.0
    return m


def mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


IDENTITY = [[1.0 if i == j else 0.0 for j in range(4)] for i in range(4)]


def to_bevy(m):
    """UE column-vector matrix -> Bevy 12 floats (P M P with P swapping Y and Z; cm -> m)."""
    perm = [0, 2, 1]
    out = [0.0] * 12
    for col in range(3):
        for row in range(3):
            out[col * 3 + row] = round(m[perm[row]][perm[col]], 6)
    out[9], out[10], out[11] = round(m[0][3] / 100, 4), round(m[2][3] / 100, 4), round(m[1][3] / 100, 4)
    return out


def hidden(props):
    return props.get("bVisible") is False or props.get("bHiddenInGame") is True


def prop_text(v):
    return json.dumps(v, separators=(",", ":"))


def prop_value(v):
    """A child actor template's property as the level export writes an actor's: JSON text, class
    references as the class's name (SpawnedActorClass), other object references dropped."""
    if isinstance(v, dict) and "ObjectName" in v:
        name = v["ObjectName"].split("'")[1] if "'" in v["ObjectName"] else v["ObjectName"]
        return name if name.endswith("_C") else None
    return prop_text(v)


def expand(d, pkg, base, out, depth=0):
    """Add the Blueprint `pkg`'s meshes, particles and child actors, placed by `base`."""
    comps, parents = components(d, pkg)

    def world(n, seen=()):
        if n in seen or n not in comps:
            return base
        par = parents.get(n)
        if par is None:
            return base  # the root: the actor's own transform
        return mul(world(par, seen + (n,)), rel_matrix(comps[n]["props"]))

    for n, c in comps.items():
        props, t = c["props"], c["type"]
        if hidden(props):
            continue
        if t == "StaticMeshComponent" and props.get("StaticMesh"):
            out["meshes"].append([pkg_of(props["StaticMesh"]["ObjectPath"]), to_bevy(world(n))])
        elif t == "ParticleSystemComponent" and props.get("Template"):
            out["particles"].append([pkg_of(props["Template"]["ObjectPath"]).rsplit("/", 1)[-1], to_bevy(world(n))])
        elif t == "ChildActorComponent" and props.get("ChildActorClass"):
            cls_pkg = pkg_of(props["ChildActorClass"]["ObjectPath"])
            cls = props["ChildActorClass"]["ObjectName"].split("'")[1]
            w = world(n)
            tpl = props.get("ChildActorTemplate")
            tp = {}
            if tpl:
                k = int(tpl["ObjectPath"].rsplit(".", 1)[1])
                ex = d.get(c["pkg"])
                if k < len(ex):
                    tp = ex[k].get("Properties") or {}
            if cls in HUMAN_SPAWNERS:
                for el in tp.get("Spawning AI") or []:
                    for key, v in (el.items() if isinstance(el, dict) else []):
                        if key.startswith("AICharacter") and isinstance(v, dict) and v.get("ObjectName"):
                            out["humans"].add(v["ObjectName"].split("'")[1])
                out["humans"].add(cls) if not tp.get("Spawning AI") else None
            out["actors"].append({"class": cls, "transform": to_bevy(w), "props": {k: v for k, v in ((k, prop_value(v)) for k, v in tp.items()) if v is not None}})
            if depth < 4:
                expand(d, cls_pkg, w, out, depth + 1)


def text(v):
    return v.get("SourceString") or v.get("CultureInvariantString") or "" if isinstance(v, dict) else str(v or "")


def spawner_defaults(d):
    """BP_PrefabSpawner's class defaults, and the marker textures the level's spawners name, as
    exported paths (the level export keeps only the texture's name)."""
    pkg = "SurrounDead/Content/Prefabs/BP_PrefabSpawner"
    d.ensure([pkg])
    p = next((e.get("Properties") or {} for e in d.get(pkg) if e["Name"].startswith("Default__")), {})
    tex = p.get("Marker Texture")
    names = set()
    actors = GAME / "levels" / "LongdownValley" / "actors.json"
    if actors.exists():
        names = {(a.get("props") or {}).get("Marker Texture") for a in json.loads(actors.read_text()) if a["class"].startswith("BP_PrefabSpawner")}
    ui = GAME / "SurrounDead" / "Content" / "UI"
    textures = {}
    for n in sorted(x for x in names if x):
        hit = next(iter(sorted(ui.rglob(f"{n}.png"))), None)
        if hit:
            textures[n] = "game/" + str(hit.relative_to(GAME))
    return {
        "chance": p.get("Chance", 50.0),
        "random": p.get("RandomPossibilityToSpawn?", True),
        "delay_start": p.get("DelayStart", 0.0),
        "marker_texture": ("game/" + pkg_of(tex["ObjectPath"]) + ".png") if tex else None,
        "marker_title": text(p.get("Marker Title")),
        "marker_description": text(p.get("Marker Description")),
        "remove_marker_within": p.get("DistanceToRemoveMarker", 0.0) / 100.0,
        "sound": p["SoundOnSpawn"]["ObjectName"].split("'")[1] if p.get("SoundOnSpawn") else None,
        "textures": textures,
    }


def main():
    d = Dumps()
    listed = subprocess.run([SDX, "list", FOLDERS[0]], capture_output=True, text=True).stdout.split()
    prefabs = [p[: -len(".uasset")] for p in listed if p.endswith(".uasset") and p.rsplit("/", 1)[-1].startswith("Prefab_")]
    want = set(prefabs)
    for _ in range(8):  # parents and child actors, until closed
        d.ensure(want)
        more = set().union(*(refs(d.get(p)) for p in want)) - want
        if not more:
            break
        want |= more
    out = {}
    for pkg in prefabs:
        name = pkg.rsplit("/", 1)[-1] + "_C"
        p = {"meshes": [], "actors": [], "particles": [], "humans": set()}
        expand(d, pkg, IDENTITY, p)
        comps, _ = components(d, pkg)
        radius = (comps.get("FoliageRemover") or {}).get("props", {}).get("SphereRadius")
        master = next((e.get("Properties") or {} for _, ex in chain(d, pkg) for e in ex if e["Name"].startswith("Default__")), {})
        p["foliage_radius"] = (master.get("FoliageRemoverRadius") or 2000.0) / 100.0 if radius is None else radius / 100.0
        p["humans"] = sorted(p["humans"])
        out[name] = p
    spawner = spawner_defaults(d)
    (ROOT / "assets" / "data" / "prefabs.json").write_text(json.dumps({"prefabs": out, "spawner": spawner}, indent=1))
    meshes = sorted({m for p in out.values() for m, _ in p["meshes"]})
    missing = [m for m in meshes if not (GAME / f"{m}.glb").exists()]
    (HERE / "mappings" / "prefab_meshes.txt").write_text("\n".join(missing) + ("\n" if missing else ""))
    print(f"prefabs: {len(out)}, {sum(len(p['meshes']) for p in out.values())} meshes ({len(meshes)} kinds, {len(missing)} to export), "
          f"{sum(len(p['actors']) for p in out.values())} child actors; {sum(1 for p in out.values() if not p['humans'])} without humans")


if __name__ == "__main__":
    main()
