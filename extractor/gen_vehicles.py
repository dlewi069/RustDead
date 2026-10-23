#!/usr/bin/env python3
"""Vehicles (BP_VehicleMaster's children, their Chaos wheels and torque curves, DT_VehicleInfo)
-> assets/data/vehicles.json.

usage: gen_vehicles.py <json dump root> <assets/game dir>   (normally `export_assets.sh --vehicles`)

Needs a dump of Content/Blueprints/Vehicles/ (and the vehicle meshes exported as .glb, for the
wheel bones' bind positions). Writes:

  classes   vehicle BP class -> Name, skeletal mesh (+ extra attached skeletal meshes), Colors
            (BeginPlay sets material slot 0, `slot0` in the glb, to a random one), the ChaosWheeledVehicleMovementComponent
            (engine: MaxRPM, MaxTorque, the torque curve in RPM / Nm; transmission: forward and
            reverse ratios, FinalRatio, efficiency; wheels: bone, its bind position in glb space,
            radius, width, MaxSteerAngle, steering/engine/handbrake, brake torques, friction),
            the scene points (PlayerLocation, the exit points, fuel and repair boxes, the hit box,
            the headlights' spotlights)
            in UE units relative to the actor, the storage grid (BP_JigMultiplayer), FuelComponent
            and VehicleHealthComponent values, DmgOnVehicleCollision
  spawners  DT_VehicleInfo row (Enum_VehicleSpawner's value, as spawners' VehicleSelection) -> classes
  trader    the Trader row: class, name, price

Values are inherited down the chain (child overrides BP_VehicleMaster); what no BP sets is
Chaos's own default (CHAOS_* below, from UE 5.3's ChaosVehicleMovementComponent / ChaosVehicleWheel).
"""
import json
import struct
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
GAME = Path(sys.argv[2])
OUT = Path(__file__).resolve().parent.parent / "assets" / "data" / "vehicles.json"
VEH = ROOT / "Blueprints" / "Vehicles"

# UChaosWheeledVehicleMovementComponent / FVehicleEngineConfig / FVehicleTransmissionConfig defaults.
CHAOS_MOVEMENT = {"Mass": 1500.0, "DragCoefficient": 0.3, "ChassisWidth": 180.0, "ChassisHeight": 140.0}
CHAOS_ENGINE = {"MaxTorque": 300.0, "MaxRPM": 4500.0, "EngineIdleRPM": 900.0, "EngineBrakeEffect": 0.2, "EngineRevUpMOI": 5.0, "EngineRevDownRate": 600.0}
CHAOS_TRANSMISSION = {"FinalRatio": 3.08, "ForwardGearRatios": [2.85, 2.02, 1.35, 1.0], "ReverseGearRatios": [2.86], "ChangeUpRPM": 4500.0, "ChangeDownRPM": 2000.0, "GearChangeTime": 0.4, "TransmissionEfficiency": 0.9, "bUseAutomaticGears": True}
# UChaosVehicleWheel defaults.
CHAOS_WHEEL = {
    "WheelRadius": 32.0, "WheelWidth": 20.0, "MaxSteerAngle": 50.0, "bAffectedBySteering": False, "bAffectedByEngine": False,
    "bAffectedByHandbrake": True, "MaxBrakeTorque": 1500.0, "MaxHandBrakeTorque": 3000.0, "FrictionForceMultiplier": 2.0,
    "CorneringStiffness": 1000.0, "SuspensionMaxRaise": 10.0, "SuspensionMaxDrop": 10.0, "SpringRate": 250.0,
}
# Enum_VehicleSpawner's entries by display order (the enum's values).
CATEGORY = {"NewEnumerator0": 0, "NewEnumerator1": 1, "NewEnumerator2": 2, "NewEnumerator6": 3, "NewEnumerator3": 4}


def load(p):
    return json.loads(p.read_text())


def pkg(ref):
    return ref["ObjectPath"].split(".")[0] if ref else None


def field(d, prefix):
    for k, v in (d or {}).items():
        if k.split("_")[0] == prefix:
            return v
    return None


def read_class(f):
    exports = load(f)
    cls = next((e for e in exports if e["Type"] == "BlueprintGeneratedClass"), None)
    if not cls:
        return None
    cdo = next((e for e in exports if e["Name"].startswith("Default__")), {}).get("Properties") or {}
    comps = {}
    for e in exports:
        name = e["Name"].removeprefix("ICH-").removesuffix("_GEN_VARIABLE")
        if e["Name"].endswith("_GEN_VARIABLE") or name in ("VehicleMovementComp", "VehicleMesh"):
            comps.setdefault(name, {}).update(e.get("Properties") or {})
    sup = (cls.get("SuperStruct") or cls.get("Super") or {}).get("ObjectName", "")
    curves = {e["Name"]: e["Properties"] for e in exports if e["Type"] == "CurveFloat"}
    return cls["Name"], {"super": sup.split("'")[1] if "'" in sup else None, "cdo": cdo, "comps": comps, "curves": curves}


def merged(chain, comp):
    out = {}
    for c in reversed(chain):
        for k, v in c["comps"].get(comp, {}).items():
            if isinstance(v, dict) and isinstance(out.get(k), dict):
                out[k] = {**out[k], **v}
            else:
                out[k] = v
    return out


def curve_keys(keys):
    return [[k["Time"], k["Value"]] for k in keys]


def torque_curve(setup):
    tc = setup.get("TorqueCurve") or {}
    if tc.get("ExternalCurve"):
        f = ROOT / (pkg(tc["ExternalCurve"]).removeprefix("SurrounDead/Content/") + ".json")
        c = next(e for e in load(f) if e["Type"] == "CurveFloat")
        return curve_keys(c["Properties"]["FloatCurve"]["Keys"])
    return curve_keys((tc.get("EditorCurveData") or {}).get("Keys", []))


def wheel(ref, classes_by_pkg):
    """A wheel BP's properties down its chain, over Chaos's defaults."""
    out = dict(CHAOS_WHEEL)
    chain = []
    name = ref["ObjectName"].split("'")[1]
    while name in classes_by_pkg:
        chain.append(classes_by_pkg[name])
        name = classes_by_pkg[name]["super"]
    for c in reversed(chain):
        out.update({k: v for k, v in c["cdo"].items() if k in CHAOS_WHEEL or k == "AxleType"})
    return out


def glb_bones(path):
    """Bind-pose world positions of a glb's nodes, by name (glb space)."""
    p = GAME / (path + ".glb")
    if not p.exists():
        return {}
    b = p.read_bytes()
    n = struct.unpack("<I", b[12:16])[0]
    j = json.loads(b[20 : 20 + n])
    nodes = j["nodes"]
    parent = {c: i for i, nd in enumerate(nodes) for c in nd.get("children", [])}

    def qrot(q, v):
        x, y, z, w = q
        u = (x, y, z)
        def cross(a, b):
            return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
        t = tuple(2 * c for c in cross(u, v))
        c2 = cross(u, t)
        return tuple(v[i] + w * t[i] + c2[i] for i in range(3))

    def world(i, v=(0.0, 0.0, 0.0)):
        nd = nodes[i]
        s = nd.get("scale", [1, 1, 1])
        v = tuple(v[k] * s[k] for k in range(3))
        v = qrot(nd.get("rotation", [0, 0, 0, 1]), v)
        t = nd.get("translation", [0, 0, 0])
        v = tuple(v[k] + t[k] for k in range(3))
        return world(parent[i], v) if i in parent else v

    return {nd.get("name"): [round(c, 4) for c in world(i)] for i, nd in enumerate(nodes) if nd.get("name")}


def glb_materials(path):
    """A glb's material names in slot order."""
    p = GAME / (path + ".glb")
    if not p.exists():
        return []
    b = p.read_bytes()
    n = struct.unpack("<I", b[12:16])[0]
    return [m.get("name") for m in json.loads(b[20 : 20 + n]).get("materials", [])]


def loc(comp):
    v = (comp or {}).get("RelativeLocation") or {}
    return [v.get("X", 0.0), v.get("Y", 0.0), v.get("Z", 0.0)]


def main():
    classes = {}
    for f in VEH.rglob("*.json"):
        if c := read_class(f):
            classes[c[0]] = c[1]

    def chain(name):
        out = []
        while name in classes:
            out.append(classes[name])
            name = classes[name]["super"]
        return out

    vehicles = {}
    for name in sorted(classes):
        ch = chain(name)
        if not name.startswith("Vehicle_") or ch[-1] is not classes.get("BP_VehicleMaster_C"):
            continue
        cdo = {}
        for c in reversed(ch):
            cdo.update(c["cdo"])
        mesh = merged(ch, "VehicleMesh")
        mv = merged(ch, "VehicleMovementComp")
        engine = {**CHAOS_ENGINE, **{k: v for k, v in (mv.get("EngineSetup") or {}).items() if k in CHAOS_ENGINE}}
        # The torque curve: the nearest class in the chain that sets one.
        curve = next((torque_curve(c["comps"]["VehicleMovementComp"]["EngineSetup"]) for c in ch
                      if (c["comps"].get("VehicleMovementComp", {}).get("EngineSetup") or {}).get("TorqueCurve")), [])
        trans = {**CHAOS_TRANSMISSION, **(mv.get("TransmissionSetup") or {})}
        mesh_path = pkg(mesh.get("SkeletalMesh") or mesh.get("SkeletalMeshAsset"))
        bones = glb_bones(mesh_path)
        wheels = []
        for w in mv.get("WheelSetups", []):
            props = wheel(w["WheelClass"], classes)
            wheels.append({
                "bone": w["BoneName"], "at": bones.get(w["BoneName"]), "front": props.get("AxleType") == "EAxleType::Front",
                "radius": props["WheelRadius"], "width": props["WheelWidth"], "max_steer": props["MaxSteerAngle"],
                "steering": props["bAffectedBySteering"], "engine": props["bAffectedByEngine"], "handbrake": props["bAffectedByHandbrake"],
                "brake_torque": props["MaxBrakeTorque"], "handbrake_torque": props["MaxHandBrakeTorque"],
                "friction": props["FrictionForceMultiplier"], "cornering": props["CorneringStiffness"],
                "suspension": [props["SuspensionMaxRaise"], props["SuspensionMaxDrop"]],
            })
        extra = [pkg(c.get("SkeletalMesh") or c.get("SkeletalMeshAsset")) for n, c in merged_all(ch).items()
                 if n.startswith("SkeletalMesh") and (c.get("SkeletalMesh") or c.get("SkeletalMeshAsset"))]
        jig = merged(ch, "BP_JigMultiplayer").get("ContainersSettings") or [{}]
        fuel = {"CurrentFuel": 1000.0, "MaxFuel": 1000.0, "WasteTime": 0.01, "WasteOfFuelPerTime": 0.005,
                **{k: v for k, v in merged(ch, "FuelComponent").items() if k in ("CurrentFuel", "MaxFuel", "WasteTime", "WasteOfFuelPerTime")}}
        health = {"CurrentHealth": 100.0, "MaxHealth": 100.0, **{k: v for k, v in merged(ch, "VehicleHealthComponent").items() if k in ("CurrentHealth", "MaxHealth")}}
        hit = merged(ch, "HitBoxCollision")
        ext = hit.get("BoxExtent") or {"X": 250.0, "Y": 100.0, "Z": 32.0}
        sc = hit.get("RelativeScale3D") or {"X": 1.0, "Y": 1.0, "Z": 1.0}
        dmg = cdo.get("DmgOnVehicleCollision") or {"X": 5.0, "Y": 8.0}
        vehicles[name] = {
            "name": cdo.get("Name", "Vehicle"),
            "mesh": mesh_path,
            "extra_meshes": extra,
            "colors": [pkg(c) for c in cdo.get("Colors", []) if c],
            # The material on slot 0 (the one Colors replaces).
            "slot0": (glb_materials(mesh_path) or [None])[0],
            "mass": mv.get("Mass", CHAOS_MOVEMENT["Mass"]),
            "drag": mv.get("DragCoefficient", CHAOS_MOVEMENT["DragCoefficient"]),
            "engine": {"max_rpm": engine["MaxRPM"], "max_torque": engine["MaxTorque"], "idle_rpm": engine["EngineIdleRPM"], "brake_effect": engine["EngineBrakeEffect"], "torque_curve": curve},
            "transmission": {"final": trans["FinalRatio"], "forward": trans["ForwardGearRatios"], "reverse": trans["ReverseGearRatios"],
                             "change_up": trans["ChangeUpRPM"], "change_down": trans["ChangeDownRPM"], "change_time": trans["GearChangeTime"],
                             "efficiency": trans["TransmissionEfficiency"]},
            "wheels": wheels,
            "player": loc(merged(ch, "PlayerLocation")),
            "exit_driver": loc(merged(ch, "ExitPoint_DriverSide")),
            "exit_passenger": loc(merged(ch, "ExitPoint_PassengerSide")),
            "fuel_box": loc(merged(ch, "FuelBoxCollision")),
            "repair_box": loc(merged(ch, "RepairBoxCollision")),
            "hit_box": {"at": loc(hit), "half": [ext["X"] * sc["X"], ext["Y"] * sc["Y"], ext["Z"] * sc["Z"]]},
            "storage": [jig[0].get(next((k for k in jig[0] if k.startswith("Columns")), ""), 10), jig[0].get(next((k for k in jig[0] if k.startswith("Rows")), ""), 30)],
            # TurnOnOffLights' Lights: the SpotLightComponents (LightLeft/Right, some classes a
            # second pair), SetIntensity(50000) when on.
            "lights": [{"at": loc(c), "inner": c.get("InnerConeAngle", 0.0), "outer": c.get("OuterConeAngle", 44.0), "radius": c.get("AttenuationRadius", 1000.0)}
                       for n, c in sorted(merged_all(ch).items()) if n.startswith("Light")],
            "fuel": {"current": fuel["CurrentFuel"], "max": fuel["MaxFuel"], "waste_time": fuel["WasteTime"], "waste": fuel["WasteOfFuelPerTime"]},
            "health": {"current": health["CurrentHealth"], "max": health["MaxHealth"]},
            "collision_damage": [dmg["X"], dmg["Y"]],
        }

    info = next(e for e in load(VEH / "Spawner" / "DT_VehicleInfo.json") if e["Type"] == "DataTable")["Rows"]
    spawners, trader = {}, []
    for row, v in info.items():
        cat = CATEGORY.get(str(field(v, "Type")).split("::")[-1])
        entries = field(v, "Vehicles") or []
        names = [field(e, "Vehicles")["ObjectName"].split("'")[1] for e in entries if field(e, "Vehicles")]
        if row == "Trader":
            trader = [{"class": n, "name": (field(e, "Name") or {}).get("SourceString"), "price": field(e, "Price")} for n, e in zip(names, entries)]
        elif row != "All" and cat is not None:
            spawners[str(cat)] = {"row": row, "classes": names}
    OUT.write_text(json.dumps({"classes": vehicles, "spawners": spawners, "trader": trader}, indent=1, sort_keys=True) + "\n")
    print(f"vehicles: {len(vehicles)} classes, {len(spawners)} spawner rows, {len(trader)} for sale")


def merged_all(chain):
    names = {n for c in chain for n in c["comps"]}
    return {n: merged(chain, n) for n in names}


if __name__ == "__main__":
    main()
