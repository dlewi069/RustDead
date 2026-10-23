#!/usr/bin/env python3
"""Zombie classes (BP_MasterZombie and its children) and the animals (BP_Animal's) ->
assets/data/zombies.json.

usage: gen_ai.py <json dump root>   (normally run via `export_assets.sh --ai`)

Every value is the class default, inherited down the Blueprint chain (child overrides parent):
the CDO's speeds/damage/medical chances, the DamageComponent, capsule and SD_AIComponent
templates, and the locomotion blendspace's samples. Lengths in metres, speeds in m/s.

    { "classes": { "BP_Zombie_Roamer_C": { "name": "Undead Shambler", "health": 100, ... } } }

An animal's class also has an `animal` block: its AI controller's senses, its behaviour tree's
speeds, waits and search radii, and what its graphs pick (meshes, loot table, bite).

The zombie bosses (BP_ZombieBoss's children: their own Blueprint, not BP_MasterZombie's) are
zombie classes too, read from BP_ZombieBoss's graph (bytecode), BT_ZombieBoss and the attack
montage, with a `boss` block (see `boss`).

The human AI (SmartAI's characters: BP_MasterBandit / Military / Scavenger children) are classes
too, with a `human` block (see `human`), and `weapons` holds their BP_AIWeapon_* classes.
"""
import json
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import kismet  # noqa: E402

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
            if e["Name"].endswith("_GEN_VARIABLE") or e["Type"] in ("CapsuleComponent", "CharacterMovementComponent") or e["Name"] == "CharacterMesh0":
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
    bosses = {name: boss(name, get, comp, loot_by_value) for name in cls if "BP_ZombieBoss_C" in iter_supers(cls, name)}
    out.update({k: v for k, v in bosses.items() if v})
    animals = {name: animal(name, cls, get, comp, loot_by_value) for name in cls if "BP_Animal_C" in iter_supers(cls, name)}
    animals = {k: v for k, v in animals.items() if v}
    out.update(animals)
    humans = {name: human(name, cls, get, comp, loot_by_value) for name in cls}
    humans = {k: v for k, v in humans.items() if v}
    out.update(humans)
    OUT.write_text(json.dumps({"classes": out, "weapons": weapons() if (ROOT / "AI/Humans/Weapons").exists() else {}}, indent=1, sort_keys=True))
    if (ROOT / "AI/SafeZoneAndCivilians").exists():
        civilians(cls, get, comp)
    print(f"human classes: {sorted(humans)}")
    print(f"zombie classes: {sorted(set(out) - set(animals) - set(humans))}")
    print(f"animal classes: {sorted(animals)}")


GAME = Path(__file__).resolve().parent.parent / "assets" / "game"


def script(f):
    """function name -> its statements as kismet.py prints them."""
    return {x["Name"]: [kismet.e(st) for st in x["ScriptBytecode"]] for x in load(f) if "ScriptBytecode" in x}


def refs(f):
    """Object name -> package path, for every object a dump refers to."""
    return {m[1]: m[2] for m in re.finditer(r'"ObjectName": "\w+\'([^\']+)\'",\s*"ObjectPath": "([^".]+)', f.read_text())}


def random_picks(lines):
    """SetMeshesAndMaterial: each `switch` on a RandomIntegerInRange(0, K): the setter and the
    names it can pick (None for a null), only the cases 0..K (the rabbit's colour range is 0..4
    over 8 materials)."""
    names = {m[1]: m[2] for l in lines if (m := re.match(r"(Temp_object_Variable(?:_\d+)?) = (\w+)$", l))}
    out, k = [], None
    for l in lines:
        if m := re.search(r"RandomIntegerInRange\(0, (\d+)\)", l):
            k = int(m[1])
        if (m := re.search(r"\.(\w+)\(.*switch \w+ \{ (.*?); _ =>", l)) and k is not None:
            cases = dict(re.findall(r"(\d+) => (\w+)", m[2]))
            picks = [names.get(cases[str(i)]) for i in range(k + 1) if str(i) in cases]
            out.append((m[1], [None if p in (None, "null") else p for p in picks]))
    return out


def glb_bounds(path):
    """A glb's POSITION bounds (Bevy space, metres): [min, max]."""
    data = path.read_bytes()
    n = struct.unpack_from("<I", data, 12)[0]
    gltf = json.loads(data[20:20 + n])
    acc = [gltf["accessors"][p["attributes"]["POSITION"]] for m in gltf["meshes"] for p in m["primitives"]]
    lo = [min(a["min"][i] for a in acc) for i in range(3)]
    hi = [max(a["max"][i] for a in acc) for i in range(3)]
    return [lo, hi]


def tree(bt):
    """A behaviour tree's branches under the root selector: label -> (speed services, waits in
    order, task names), labelled by their blackboard decorators."""
    nodes = {e["Name"]: e for e in load(bt)}
    props = lambda n: nodes[n].get("Properties") or {}
    short = lambda ref: ref["ObjectName"].split(":")[-1].rstrip("'") if ref else None
    root = short(next(props(n)["RootNode"] for n in nodes if nodes[n]["Type"] == "BehaviorTree"))
    branches = {}
    for child in props(root).get("Children", []):
        decs = [props(short(d)).get("CachedDescription", "") for d in child.get("Decorators", [])]
        label = "see" if any("CanSeePlayer is Is Set" in d for d in decs) else "investigate" if any("IsInvestigating is Is Set" in d for d in decs) else "roam"
        branches[short(child["ChildComposite"])] = label

    def branch_of(n):
        while n and n not in branches:
            n = short(props(n).get("ParentNode"))
        return branches.get(n)

    out = {label: {"speeds": [], "waits": [], "tasks": []} for label in branches.values()}
    for n, e in nodes.items():
        b = branch_of(n)
        if not b:
            continue
        p = props(n)
        if "Speed" in p:
            out[b]["speeds"].append(p["Speed"] / 100.0)
        for c in p.get("Children", []):
            task = short(c.get("ChildTask"))
            if task and nodes[task]["Type"] == "BTTask_Wait":
                tp = props(task)
                out[b]["waits"].append([tp.get("WaitTime", 5.0), tp.get("RandomDeviation", 0.0)])
            elif task:
                out[b]["tasks"].append((nodes[task]["Type"], props(task)))
    return out


def animal(name, cls, get, comp, loot_by_value):
    """A BP_Animal child's class data (see the module docs)."""
    species = name.removeprefix("BP_Animal").removesuffix("_C")
    folder = ROOT / "AI/Animals" / species
    bp = folder / f"BP_Animal{species}.json"
    controller = folder / f"BP_Animal{species}_AIController.json"
    bt = folder / f"Animal{species}_BT.json"
    if not (bp.exists() and controller.exists() and bt.exists()):
        return None
    code = script(bp)
    graph = [l for f, ls in code.items() for l in ls]
    where = refs(bp)
    picks = dict(random_picks(code.get("SetMeshesAndMaterial", [])))
    default_mesh = pkg(comp(name, "SkeletalMeshComponentBudgeted", "SkeletalMesh"))
    meshes = [where[m].removeprefix(CONTENT) for m in picks.get("SetSkinnedAssetAndUpdate", []) if m] or [default_mesh.removeprefix(CONTENT)]
    senses = {e["Type"]: e.get("Properties") or {} for e in load(controller)}
    sight, hearing = senses.get("AISenseConfig_Sight", {}), senses.get("AISenseConfig_Hearing", {})
    snarl = next((m[1] for l in script(controller).get(f"PerceptionUpdate", []) if (m := re.search(r"PlaySoundAtLocation\(self, (\w+),", l))), None)
    branches = tree(bt)
    roam, alert = branches.get("roam", {}), branches.get("see", {})
    investigate = branches.get("investigate")
    flees = any("RunAway" in t for t, _ in alert.get("tasks", []))

    def task_script(kind):
        f = next((f for f in [*folder.glob("*.json"), ROOT / "AI/Zombies/Tasks/AI_FindRandomLocation.json"] if f.stem.endswith(kind) or f.stem == kind), None)
        return [l for ls in script(f).values() for l in ls] if f else []

    # The roam target: a task with its own Radius property, else the radius in its graph.
    roam_task = next(((t, p) for t, p in roam.get("tasks", []) if "FindRandomLocation" in t), (None, {}))
    roam_radius = roam_task[1].get("Radius")
    if roam_radius is None:
        lines = task_script(roam_task[0].removesuffix("_C") if roam_task[0] else "FindRandomLocation")
        found = next((float(m[1]) for l in lines if (m := re.search(r"GetRandomReachablePointInRadius\(self, K2_GetActorLocation\(\), \w+, ([\d.]+)", l))), None)
        default = next(((e.get("Properties") or {}).get("Radius") for e in load(ROOT / "AI/Zombies/Tasks/AI_FindRandomLocation.json") if e["Name"].startswith("Default__")), 750.0)
        roam_radius = found or default
    run = task_script(f"{species}_RunAwayFromPlayer") if flees else []
    away = next((-float(m[1]) for l in run if (m := re.search(r"Conv_DoubleToVector\((-?[\d.]+)\)", l))), 500.0)
    flee_radius = next(([float(m[1]) / 100, float(m[2]) / 100] for l in run if (m := re.search(r"RandomFloatInRange\(([\d.]+), ([\d.]+)\)", l))), None)
    bite = next((m for l in graph if (m := re.search(r"ApplyMedicalEffect\(\w+, \w+, ([\d.]+), ([\d.]+), ([\d.]+), self\)", l))), None)
    xp_at = next((i for i, l in enumerate(graph) if ".AddXP(" in l), None)
    xp = next(([float(m[1]), float(m[2])] for l in reversed(graph[:xp_at or 0]) if (m := re.search(r"RandomFloatInRange\(([\d.]+), ([\d.]+)\)", l))), [5.0, 10.0])
    loot = next((int(m[1]) for l in code.get("UserConstructionScript", []) if (m := re.search(r"SetLootTable_AI_DA\((\d+),", l))), None)
    killed = next((m[1] for l in graph if (m := re.search(r"(\w+) Killed, XP Given: \{XP\}", l))), species)
    # The corpse's lifetime: the long Delay after the kill, then K2_DestroyActor.
    corpse = max((float(m[1]) for l in graph if (m := re.search(r"KismetSystemLibrary\.Delay\(self, ([\d.]+),", l))), default=600.0)
    seq = lambda montage: where.get(montage, "").removeprefix(CONTENT).removesuffix("_Montage") or None
    hits = [pkg(m).removeprefix(CONTENT).removesuffix("_Montage") for m in get(name, "HitAnimation", []) if m]
    hits += [s for l in graph for m in re.finditer(r"PlayAnimMontage\((\w+_Montage)", l) if "Death" not in m[1] and (s := seq(m[1]))]
    death = next((s for l in graph for m in re.finditer(r"PlayAnimMontage\((\w+Death\w*_Montage)", l) if (s := seq(m[1]))), None)
    attacks = [pkg(m).removeprefix(CONTENT).removesuffix("_Montage") for m in get(name, "AttackMontage", []) if m]

    def clip_secs(path):
        f = GAME / "SurrounDead/Content" / f"{path}.anim.json"
        if not f.exists():
            return None
        a = load(f)
        return a["duration"] / (a.get("rate") or 1.0)

    attack_secs = clip_secs(attacks[0]) if attacks else None
    voice = next((pkg(c["Sound"]).rsplit("/", 1)[1] for c in [comp(name, "AudioComponent", "Sound") and {"Sound": comp(name, "AudioComponent", "Sound")}] if c), None)
    bounds = glb_bounds(GAME / "SurrounDead/Content" / f"{meshes[0]}.glb") if (GAME / "SurrounDead/Content" / f"{meshes[0]}.glb").exists() else None
    speed = lambda b, d: (b.get("speeds") or [d])[0] if b else d
    return {
        "name": species,
        "meshes": meshes,
        "materials": [m for m in picks.get("SetMaterial", []) if m],
        "damage": get(name, "DamageToDo", 0.0),
        "roam_speed": speed(roam, 1.0),
        "alert_speed": speed(alert, 4.0),
        "health": get(name, "Health", 100.0),
        "health_deviation": 0.0,
        # AttackPlayer: a random AttackMontage, the next as soon as it ends (Delay(montage)).
        "attack_interval": [attack_secs, attack_secs] if attack_secs else [1.0, 1.0],
        # ApplyMedicalEffect(['Bleed'], chance, heavy, bone) on a flesh hit.
        "bleed": float(bite[1]) / 100 if bite else 0.0,
        "heavy_bleed": float(bite[2]) / 100 if bite else 0.0,
        "broken_bone": float(bite[3]) / 100 if bite else 0.0,
        "rad_chance": 0.0,
        "rad_damage": 0.0,
        "xp": xp,
        "loot_table": loot_by_value.get(loot) if loot is not None else None,
        "shovable": False,
        "container": next((field(s, "ContainerName") for c in [cls[name]] for s in c["components"].get("BP_JigMultiplayer_C", {}).get("ContainersSettings", [])[:1]), None),
        "capsule_half_height": comp(name, "CapsuleComponent", "CapsuleHalfHeight", 88.0) / 100.0,
        "capsule_radius": comp(name, "CapsuleComponent", "CapsuleRadius", 34.0) / 100.0,
        "locomotion": blendspace(f"{CONTENT}AI/Animals/{species}/{species}_Locomotion1D"),
        "animal": {
            "flees": flees,
            "killed": killed,
            # AIPerception (UE defaults where the controller leaves them).
            "sight": sight.get("SightRadius", 3000.0) / 100.0,
            "lose_sight": sight.get("LoseSightRadius", 3500.0) / 100.0,
            "peripheral": sight.get("PeripheralVisionAngleDegrees", 90.0),
            "hearing": hearing.get("HearingRange", 3000.0) / 100.0,
            # The hearing stimulus' MaxAge: how long a noise keeps CanSeePlayer / the investigation.
            "hearing_age": hearing.get("MaxAge", 0.0),
            "investigate_speed": speed(investigate, 0.0) if investigate else None,
            "roam_radius": roam_radius / 100.0,
            "roam_waits": roam.get("waits", []),
            "alert_waits": alert.get("waits", []),
            "investigate_waits": investigate.get("waits", []) if investigate else [],
            # Run Away: a reachable point within flee_radius of one `away` m from the threat.
            "away": away / 100.0,
            "flee_radius": flee_radius,
            "arrive": 0.7,
            "attacks": attacks,
            "hits": list(dict.fromkeys(hits)),
            "death": death,
            "corpse_secs": corpse,
            # With Skill_Huntsman a Container_AnimalMeat ("Animal Meat") spawns at the kill.
            "huntsman_meat": any("Container_AnimalMeat_C" in l for l in graph),
            "antlers": [where[m].removeprefix(CONTENT) if m else None for m in picks.get("SetStaticMesh", [])],
            "voice": voice,
            "snarl": snarl,
            "bounds": bounds,
            # CharacterMesh0's RelativeScale3D (the rabbit's 1.2) and RelativeLocation X: the mesh
            # sits 11-55 cm behind the capsule's centre (the body is drawn from the mesh origin).
            "scale": (comp(name, "SkeletalMeshComponentBudgeted", "RelativeScale3D") or {}).get("X", 1.0),
            "mesh_back": -(comp(name, "SkeletalMeshComponentBudgeted", "RelativeLocation") or {}).get("X", 0.0) / 100.0,
            # SD_AIComponent.MeleeAttackDistance: Zombie_EnemyInRange's reach, capsule centre to
            # capsule centre.
            "reach": (comp(name, "SD_AIComponent_C", "MeleeAttackDistance") or ai_component_default("MeleeAttackDistance") or 200.0) / 100.0,
        },
    }


def boss(name, get, comp, loot_by_value):
    """A BP_ZombieBoss child: zombie-class values plus `boss` (the attack montage and its
    AnimNotify_ZombieBossAttackRightArm window, MeleeAttackDistance, the hand trace's radius, the
    death's skill point chance and corpse lifetime, AddUniqueLoot's sets, the mesh's scale and
    the AI controller's peripheral vision)."""
    bp = ROOT / "AI/Zombies/Boss/BP_ZombieBoss.json"
    bt = ROOT / "AI/Zombies/Boss/BT_ZombieBoss.json"
    if not bp.exists() or not bt.exists():
        return None
    code = script(bp)
    graph = [l for ls in code.values() for l in ls]

    def first(pattern):
        """The first match of `pattern` in the graph (its groups), or None."""
        for line in graph:
            if m := re.search(pattern, line):
                return m
        return None

    # MeleeAttack: a random AttackMontage, then Delay(1.35-1.75) before the next.
    m = first(r"RandomFloatInRange\((1\.3\d*), (1\.7\d*)\)")
    delay = [float(m[1]), float(m[2])] if m else [1.35, 1.75]
    # Event_OnHit: ApplyMedicalEffect(['Bleed', 'Heavy Bleed'], 20, 10, 0), ApplyRadiationDamage(100 %, ScalingDamageMultiplier(16)).
    bite = next((m for l in graph if (m := re.search(r"ApplyMedicalEffect\(\w+, \w+, ([\d.]+), ([\d.]+), ([\d.]+), self\)", l))), None)
    rad = next((m for l in graph if (m := re.search(r"ApplyRadiationDamage\(\w+, ([\d.]+),", l))), None)
    rad_dmg = next((float(m[1]) for l in graph if (m := re.search(r"ScalingDamageMultiplier\(([\d.]+), self", l))), 0.0)
    # Death: CurrentLevel x RandomFloatInRange(200, 400) x MultiplyValue XP; RandomFloatInRange(0, 100) <= 25: Add Skill Points.
    xp_at = next((i for i, l in enumerate(graph) if ".AddXP(" in l), 0)
    xp = next(([float(m[1]), float(m[2])] for l in reversed(graph[:xp_at]) if (m := re.search(r"RandomFloatInRange\(([\d.]+), ([\d.]+)\)", l))), [200.0, 400.0])
    skill = next((float(m[1]) for l in graph if (m := re.search(r"LessEqual_DoubleDouble\(\w+, ([\d.]+)\)", l))), 0.0)
    corpse = max((float(m[1]) for l in graph if (m := re.search(r"KismetSystemLibrary\.Delay\(self, ([\d.]+),", l))), default=1200.0)
    trace = next((float(m[1]) for l in code.get("RightArmTrace", []) if (m := re.search(r"SphereTraceSingleForObjects\(self, \w+\(\), \w+\(\), ([\d.]+),", l))), 60.0)
    killed = next((m[1] for l in graph if (m := re.search(r'"(\w[\w ]*) Killed, XP Given: "', l))), "Zombie Boss")
    attacks = [pkg(m).removeprefix(CONTENT) for m in get(name, "AttackMontage", []) if m]
    montage = file_of(CONTENT + attacks[0]) if attacks else None
    window, clip, rate = None, None, 1.0
    if montage and montage.exists():
        for e in load(montage):
            p = e.get("Properties") or {}
            rate = p.get("RateScale", rate)
            for t in p.get("SlotAnimTracks", []):
                for s in t["AnimTrack"]["AnimSegments"][:1]:
                    clip = pkg(s["AnimReference"]).removeprefix(CONTENT)
            for n in p.get("Notifies", []):
                if "AttackRightArm" in n.get("NotifyName", "") or "AttackLeftArm" in n.get("NotifyName", ""):
                    start = n.get("LinkValue", 0.0)
                    window = [start, start + n.get("Duration", 0.0)]
    bs = next((r for r in refs(ROOT / "AI/Zombies/Boss/AnimBP_ZombieBossBrute.json").items() if "Locomotion" in r[0]), None)
    controller = {e["Type"]: e.get("Properties") or {} for e in load(ROOT / "AI/Zombies/Boss/BP_ZombieBoss_AIController.json")}
    # BTTask_Wait_14 (after investigating) and _15 (before roaming): [WaitTime, RandomDeviation].
    waits = [[p.get("WaitTime", 5.0), p.get("RandomDeviation", 0.0)] for e in load(bt) if e["Type"] == "BTTask_Wait" for p in [e.get("Properties") or {}]]
    speeds = [p["Speed"] / 100.0 for e in load(bt) if e["Type"].startswith("ZombieBoss_MovementSpeed") for p in [e.get("Properties") or {}] if "Speed" in p]
    scale = comp(name, "SkeletalMeshComponentBudgeted", "RelativeScale3D") or {}
    mesh = pkg(comp(name, "SkeletalMeshComponentBudgeted", "SkeletalMesh"))
    ai_name = (get(name, "AIName") or {}).get("SourceString")
    loot = get(name, "LootTable")
    enum = next(x for x in load(ROOT / "AI/Enum_AILootTables.json") if x["Type"] == "UserDefinedEnum")
    return {
        "name": ai_name,
        "meshes": [mesh.removeprefix(CONTENT)] if mesh else [],
        "materials": [],
        "damage": get(name, "DamageToDo", 28.0),
        # ZombieBoss_MovementSpeed's Change Speed: 80 idle/roaming, 500 investigating or chasing.
        "roam_speed": min(speeds or [0.8]),
        "alert_speed": max(speeds or [5.0]),
        "health": comp(name, "DamageComponent_C", "StandardHealthValue", 4000.0),
        "health_deviation": 0.0,
        "attack_interval": delay,
        "bleed": float(bite[1]) / 100 if bite else 0.0,
        "heavy_bleed": float(bite[2]) / 100 if bite else 0.0,
        "broken_bone": float(bite[3]) / 100 if bite else 0.0,
        "rad_chance": float(rad[1]) / 100 if rad else 0.0,
        "rad_damage": rad_dmg,
        "xp": xp,
        "loot_table": loot_by_value.get(enum["Names"].get(loot, 0)) if loot else None,
        "shovable": False,
        # The corpse's loot widget: AddJigWidgetToContent(..., AIName).
        "container": ai_name,
        "capsule_half_height": comp(name, "CapsuleComponent", "CapsuleHalfHeight", 100.0) / 100.0,
        "capsule_radius": comp(name, "CapsuleComponent", "CapsuleRadius", 36.0) / 100.0,
        "locomotion": blendspace(bs[1]) if bs else [],
        "boss": {
            "attack": clip,
            # AnimNotify_ZombieBossAttackRightArm: RightArmTrace every frame in this window (montage
            # seconds; the montage plays at `rate`, its RateScale).
            "hit_window": window or [0.25, 0.42],
            "rate": rate,
            "waits": waits,
            "reach": (comp(name, "SD_AIComponent_C", "MeleeAttackDistance") or 500.0) / 100.0,
            "hand_radius": trace / 100.0,
            "skill_point_chance": skill / 100.0,
            "corpse_secs": corpse,
            "unique_sets": get(name, "Unqiue Items Set", []),
            "killed": killed,
            # CharacterMesh0's RelativeScale3D in Bevy order (UE X, Z, Y).
            "scale": [scale.get("X", 1.0), scale.get("Z", 1.0), scale.get("Y", 1.0)],
            "peripheral": controller.get("AISenseConfig_Sight", {}).get("PeripheralVisionAngleDegrees", 70.0),
            "voice": next((pkg(s).rsplit("/", 1)[1] for s in [comp(name, "AudioComponent", "Sound")] if s), None),
        },
    }


# The human factions' master classes: their kill toast's name and health multiplier row come
# from their graphs (Death, AI_Health).
FACTIONS = {"BP_MasterBandit_C": "Bandit", "BP_MasterMilitary_C": "Military", "BP_MasterScavenger_C": "Scavenger"}
# BP_MasterTrader (the wandering trader) is a SmartAI character of its own, tagged Scavenger;
# its graph holds its Death and AI_Health. `trader` marks it (gameplay/traders.rs).
WANDERERS = {"BP_MasterTrader_C": "Scavenger"}
HEALTH_ROWS = {"BanditHealth": "BANDIT HEALTH MULTIPLIER", "MilitaryHealth": "MILITARY HEALTH MULTIPLIER", "ScavengerHealth": "SCAVENGER / TRADER HEALTH MULTIPLIER"}
# E_HitboxTypeAI: Head, Chest, Arms, Legs, Hands, Feet, Other, None.
HITBOXES = 8


def smart_default(key, default=None):
    f = ROOT / "SmartAI/Blueprints/AI/Components/BP_SmartAIComponent.json"
    if not f.exists():
        return default
    return next(((e.get("Properties") or {}).get(key, default) for e in load(f) if e["Name"].startswith("Default__")), default)


def damage_map(entries):
    """A `Damage` map (E_HitboxTypeAI -> damage) as a list indexed by the enum."""
    out = [0.0] * HITBOXES
    for kv in entries or []:
        k = int(kv["Key"].rsplit("NewEnumerator", 1)[1])
        if k < HITBOXES:
            out[k] = kv["Value"]
    return out


def weapons():
    """BP_AIWeapon_* (AI/Humans/Weapons): what the SmartAI component's Range Attack / melee read."""
    out = {}
    master = ROOT / "AI/Humans/Weapons/BP_AIWeapon_Master.json"
    base = next(((e.get("Properties") or {}) for e in load(master) if e["Name"].startswith("Default__")), {}) if master.exists() else {}
    for f in (ROOT / "AI/Humans/Weapons").rglob("BP_AIWeapon_*.json"):
        ex = load(f)
        cls = next((e for e in ex if e["Type"] == "BlueprintGeneratedClass"), None)
        if not cls or f.stem == "BP_AIWeapon_Master":
            continue
        p = dict(base)
        p.update(next(((e.get("Properties") or {}) for e in ex if e["Name"].startswith("Default__")), {}))
        comps = {e["Name"].removesuffix("_GEN_VARIABLE"): e.get("Properties") or {} for e in ex if e["Name"].endswith("_GEN_VARIABLE")}
        mesh = comps.get("SkeletalMesh", {}).get("SkeletalMesh") or comps.get("StaticMesh", {}).get("StaticMesh")
        muzzle = comps.get("RangeStartPoint", {}).get("RelativeLocation") or {}
        out[cls["Name"]] = {
            "ranged": bool(p.get("Can Range Attack", False)),
            "damage": damage_map(p.get("Damage")),
            # Range Attack Distance / Range (Trace) in m; Spread (Trace) degrees (x the AI's Accuracy).
            "range": p.get("Range Attack Distance", 0.0) / 100.0,
            "spread": p.get("Spread (Trace)", 1.0),
            "interval": [p.get("Range Attack Frequency Min", 1.0), p.get("Range Attack Frequency Max", 1.0)],
            "magazine": p.get("Max Magazine Ammo", 0),
            "sound": pkg(p.get("RangeAttackSound")).rsplit("/", 1)[1] if p.get("RangeAttackSound") else None,
            "muzzle_flash": pkg(p.get("Range Attack Particle Effect")).rsplit("/", 1)[1] if p.get("Range Attack Particle Effect") else None,
            "reload": pkg(p.get("Character Reload Montage")).removeprefix(CONTENT) if p.get("Character Reload Montage") else None,
            "melee": [pkg(m).removeprefix(CONTENT) for m in p.get("Melee Animations") or [] if m],
            "melee_interval": [p.get("Melee Attack Frequency Min", 1.0), p.get("Melee Attack Frequency Max", 1.0)],
            "mesh": pkg(mesh).removeprefix(CONTENT) if mesh else None,
            # RangeStartPoint (where shots start), UE cm in the weapon's space.
            "muzzle": [muzzle.get("X", 0.0), muzzle.get("Y", 0.0), muzzle.get("Z", 0.0)],
            "socket": p.get("Equip Attach Socket"),
        }
    return out


def human(name, cls, get, comp, loot_by_value):
    """A human AI class (BP_MasterBandit / Military / Scavenger children, SmartAI's
    BP_MasterAICharacter): zombie-class values plus `human` (its weapons, accuracy, the
    controller's senses, the SmartAI component's speeds and ranges, the kill's XP and toast)."""
    faction_cls = name if name in WANDERERS else next((s for s in [name, *iter_supers(cls, name)] if s in FACTIONS), None)
    if not faction_cls or name in FACTIONS or "Dog" in name:
        return None
    faction = WANDERERS.get(name) or FACTIONS[faction_cls]
    master = next((f for f in (ROOT / "AI/Humans").rglob(faction_cls.removesuffix("_C") + ".json")), None)
    graph = [l for ls in script(master).values() for l in ls] if master else []
    xp_at = next((i for i, l in enumerate(graph) if ".AddXP(" in l), 0)
    xp = next(([float(m[1]), float(m[2])] for l in reversed(graph[:xp_at]) if (m := re.search(r"RandomFloatInRange\(([\d.]+), ([\d.]+)\)", l))), [10.0, 20.0])
    killed = next((m[1] for l in graph if (m := re.search(r'"(\w[\w ]*) Killed, XP Given: ', l))), faction)
    health_row = next((HEALTH_ROWS[m[1]] for l in graph if (m := re.search(r"'Difficulty\.(\w+Health)'", l)) and m[1] in HEALTH_ROWS), None)
    ctrl_cls = (get(name, "AIControllerClass") or {})
    if isinstance(ctrl_cls, str):
        ctrl_name = re.search(r"'(\w+)'", ctrl_cls)[1] if "'" in ctrl_cls else None
    else:
        ctrl_name = ctrl_cls.get("ObjectName", "").split("'")[1] if ctrl_cls.get("ObjectName") else None
    sight = {}
    for c in [ctrl_name, "BP_MasterAIController_C"]:
        f = next((x for x in (ROOT / "AI/Humans").rglob((c or "").removesuffix("_C") + ".json")), None) if c else None
        f = f or (ROOT / "SmartAI/Blueprints/AI/AICharacters/BP_MasterAIController.json")
        if not f.exists():
            continue
        for e in load(f):
            if e["Type"] in ("AISenseConfig_Sight", "AISenseConfig_Hearing"):
                for k, v in (e.get("Properties") or {}).items():
                    sight.setdefault(k, v)
    sm = lambda key, default=None: comp(name, "BP_SmartAIComponent_C", key, smart_default(key, default))  # noqa: E731
    speeds = {kv["Key"].rsplit("NewEnumerator", 1)[1]: kv["Value"] / 100.0 for kv in sm("Movement Speeds", []) or []}
    loot = get(name, "LootTable")
    enum = next(x for x in load(ROOT / "AI/Enum_AILootTables.json") if x["Type"] == "UserDefinedEnum")
    weapon = lambda key: [w["ObjectName"].split("'")[1] for w in get(name, key, []) or [] if w]  # noqa: E731
    death = [pkg(a).removeprefix(CONTENT) for a in sm("Death Animations", []) or [] if a]
    health = sm("MaxHealth", 100)
    # BackpackMeshes (SetMeshAndMaterial picks one): the mesh on the body's Backpack socket at
    # the map value's transform, in glb space (translation (X, Z, Y)/100, quat (x, z, y, -w)).
    backpack = None
    for kv in get(name, "BackpackMeshes", []) or []:
        m = re.search(r"'([^.']+)\.", kv["Key"]) if isinstance(kv.get("Key"), str) else None
        if m:
            t, q = kv["Value"].get("Translation", {}), kv["Value"].get("Rotation", {})
            backpack = {
                "mesh": m[1].removeprefix(CONTENT),
                "t": [t.get("X", 0.0) / 100, t.get("Z", 0.0) / 100, t.get("Y", 0.0) / 100],
                "r": [q.get("X", 0.0), q.get("Z", 0.0), q.get("Y", 0.0), -q.get("W", 1.0)],
            }
            break
    return {
        "name": (get(name, "Name") or {}).get("SourceString"),
        "meshes": [pkg(m).removeprefix(CONTENT) for m in get(name, "Meshes", []) or [] if m],
        "materials": [],
        "damage": 0.0,
        # E_AIBehaviour: 15 Roam, 22 Range Attack (the combat speed).
        "roam_speed": speeds.get("15", 1.8),
        "alert_speed": speeds.get("22", 3.0),
        "health": float(health),
        "health_deviation": 0.0,
        "attack_interval": [1.0, 1.0],
        "bleed": 0.0, "heavy_bleed": 0.0, "broken_bone": 0.0, "rad_chance": 0.0, "rad_damage": 0.0,
        "xp": xp,
        "loot_table": loot_by_value.get(enum["Names"].get(loot, 0)) if loot else None,
        "shovable": False,
        "container": (get(name, "Name") or {}).get("SourceString"),
        "capsule_half_height": comp(name, "CapsuleComponent", "CapsuleHalfHeight", CAPSULE["CapsuleHalfHeight"]) / 100.0,
        "capsule_radius": comp(name, "CapsuleComponent", "CapsuleRadius", CAPSULE["CapsuleRadius"]) / 100.0,
        "locomotion": [],
        "human": {
            "faction": faction,
            "killed": killed,
            "boss": bool(get(name, "Boss?", False)),
            "health_row": health_row,
            "hair": [pkg(m).removeprefix(CONTENT) for m in get(name, "HairMeshes", []) or [] if m],
            "beard": [pkg(m).removeprefix(CONTENT) for m in get(name, "BeardMeshes", []) or [] if m],
            "ranged": weapon("RangedWeapon"),
            "melee": weapon("MeleeWeapon"),
            "start_ranged": bool(get(name, "StartWithRangeWeaponEquipped?", False)),
            "accuracy": sm("Accuracy", 1.0),
            "melee_distance": (sm("Melee Distance", 200.0) or 200.0) / 100.0,
            "melee_speed": speeds.get("21", 2.0),
            "sight": sight.get("SightRadius", 3000.0) / 100.0,
            "lose_sight": sight.get("LoseSightRadius", 3500.0) / 100.0,
            "peripheral": sight.get("PeripheralVisionAngleDegrees", 90.0),
            "hearing": sight.get("HearingRange", 3000.0) / 100.0,
            "roam": [(sm("Min Roam Distance", 300.0) or 300.0) / 100.0, (sm("Max Roam Distance", 900.0) or 900.0) / 100.0],
            "roam_wait": [sm("Roam Wait Time", 4.0), sm("Roam Wait Time Deviation", 1.0)],
            "corpse_secs": sm("Dead Despawned Time", 600.0),
            "death": death,
            "trader": name in WANDERERS,
            "backpack": backpack,
        },
    }


CIVILIANS = Path(__file__).resolve().parent.parent / "assets" / "data" / "civilians.json"


def civilians(cls, get, comp):
    """The settlers (BP_RandomCivilian_Male / _Female, BP_MainFriendly AI): their looks
    (SetMeshAndMaterial's Meshes, HairMeshes, BeardMeshes, BackpackMeshes in glb space), the
    CharMoveComp's walk speed and turn rate, the capsule, BT_RandomCivilian's waits and MoveTo
    radius, Civilian_FindRandomLocation's radius, and the name tables (rows "1".."1000")
    -> assets/data/civilians.json."""
    def mesh_list(key, name):
        return [pkg(m).removeprefix(CONTENT) for m in get(name, key, []) or [] if m]

    out = {}
    for name in cls:
        if "BP_RandomCivilian_C" not in [name, *iter_supers(cls, name)] or name == "BP_RandomCivilian_C":
            continue
        packs = []
        for kv in get(name, "BackpackMeshes", []) or []:
            m = re.search(r"'([^.']+)\.", kv.get("Key", ""))
            if m:
                t, q = kv["Value"].get("Translation", {}), kv["Value"].get("Rotation", {})
                packs.append({"mesh": m[1].removeprefix(CONTENT), "t": [t.get("X", 0.0) / 100, t.get("Z", 0.0) / 100, t.get("Y", 0.0) / 100], "r": [q.get("X", 0.0), q.get("Z", 0.0), q.get("Y", 0.0), -q.get("W", 1.0)]})
        out[name] = {
            "male": bool(get(name, "IsMale?", False)),
            "meshes": mesh_list("Meshes", name),
            "hair": mesh_list("HairMeshes", name),
            "beard": mesh_list("BeardMeshes", name),
            "backpacks": packs,
            "walk_speed": comp(name, "CharacterMovementComponent", "MaxWalkSpeed", 600.0) / 100.0,
            "turn_rate": (comp(name, "CharacterMovementComponent", "RotationRate", {}) or {}).get("Yaw", 360.0),
            "capsule_half_height": comp(name, "CapsuleComponent", "CapsuleHalfHeight", 88.0) / 100.0,
            "capsule_radius": comp(name, "CapsuleComponent", "CapsuleRadius", 34.0) / 100.0,
        }
    bt = {"before": [1.0, 3.0], "after": [5.0, 15.0], "accept": 0.7, "radius": 20.0}
    btf = ROOT / "AI/SafeZoneAndCivilians/RandomCivilian/BT_RandomCivilian.json"
    if btf.exists():
        ex = load(btf)
        by = {e["Name"]: e.get("Properties") or {} for e in ex}
        seq = next((e.get("Properties") or {} for e in ex if e["Type"] == "BTComposite_Sequence"), {})
        waits = [re.split(r"[.:]", c["ChildTask"]["ObjectName"].split("'")[1])[-1] for c in seq.get("Children", []) if c.get("ChildTask") and "Wait" in c["ChildTask"]["ObjectName"]]
        # BTTask_Wait: WaitTime (default 5) +- RandomDeviation, in the sequence's order.
        w = [[by.get(n, {}).get("WaitTime", 5.0), by.get(n, {}).get("RandomDeviation", 0.0)] for n in waits]
        if len(w) == 2:
            bt["before"], bt["after"] = w
        mv = next((p for n, p in by.items() if n.startswith("BTTask_MoveTo")), {})
        bt["accept"] = mv.get("AcceptableRadius", 5.0) / 100.0
    task = ROOT / "AI/SafeZoneAndCivilians/RandomCivilian/Civilian_FindRandomLocation.json"
    if task.exists():
        for l in [l for ls in script(task).values() for l in ls]:
            if m := re.search(r"K2_GetRandomReachablePointInRadius\(self, [^,]+, [^,]+, ([\d.]+)", l):
                bt["radius"] = float(m[1]) / 100.0
    names = {}
    for key, table in (("male", "Male_Names"), ("female", "FemaleNames"), ("surname", "Surname_Names")):
        f = ROOT / f"AI/SafeZoneAndCivilians/Names/{table}.json"
        rows = (load(f)[0].get("Rows") or {}) if f.exists() else {}
        names[key] = [field(rows[k], "Name").get("SourceString", "") for k in sorted(rows, key=lambda k: int(k) if k.isdigit() else 0)]
    CIVILIANS.write_text(json.dumps({"classes": out, "behaviour": bt, "names": names}, indent=1, sort_keys=True))
    print(f"civilians: {sorted(out)}, {len(names['male'])} / {len(names['female'])} / {len(names['surname'])} names")


def iter_supers(cls, name):
    name = cls.get(name, {}).get("super")
    while name:
        yield name
        name = cls.get(name, {}).get("super")


if __name__ == "__main__":
    main()
