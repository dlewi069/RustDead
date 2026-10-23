#!/usr/bin/env python3
"""Narrative quests (Content/Quests/Quests) and the quest crate -> assets/data/quests.json.

usage: gen_quests.py <json dump root>   (normally run via `export_assets.sh --quests`)

Needs dumps of Content/Quests/ and Content/Inventory/Items/DataAssets/ (the crate's pickup
classes are mapped to their items through the data assets' PickupClass). Writes:

  quests   quest BP class (as the level's BP_LaptopEventManager / BP_StartQuestArea and
           BP_PlayerCharacter name it) -> QuestName, QuestDescription, the start state's ID and
           its states and branches, keyed by their IDs:
             state   kind (Regular / Success / Failure: StateNodeType), events (run on entering
                     it), branches
             branch  the state it leads to and its tasks (BPT_GoToLocation: goal, tolerance,
                     friendly name; BPT_CompleteDataTask: data task, argument, quantity,
                     optional); DescriptionOverride as `description`
           Events: SpawnMarker / RemoveMarker (BP_QuestMarker), SpawnActor / RemoveActor (by
           tag), RewardPlayerXP, Forget (NE_ForgetQuest); others are kept by class with
           `supported: false`.
  crate    BP_GuaranteedSpawner (its _Classified child overrides nothing): ContainerName,
           DurationToHold, ChanceToProduceNoise, the weapon items it may drop (Items, as item
           IDs; repeats kept, Array_Random weighs them), meshes and component offsets.

Locations are Bevy-space metres: UE (X, Y, Z) cm -> (X, Z, Y) / 100.
"""
import json
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
OUT = Path(__file__).resolve().parent.parent / "assets" / "data" / "quests.json"

SUPPORTED_EVENTS = {"Event_SpawnMarker_C", "Event_RemoveMarker_C", "Event_SpawnActor_C", "Event_RemoveActor_C", "Event_RewardPlayerXP_C", "NE_ForgetQuest_C"}
# BPT_GoToLocation's class defaults.
GOTO_TOLERANCE = 500.0
GOTO_NAME = "Location Name"


def load(p):
    return json.loads(p.read_text())


def text(t):
    return (t or {}).get("SourceString") or (t or {}).get("CultureInvariantString")


def obj_name(ref):
    """`QuestState'Quest_X_C:QuestTemplate.QuestState_3'` -> `QuestState_3`."""
    return ref["ObjectName"].rstrip("'").rsplit(".", 1)[-1].rsplit(":", 1)[-1].split("'")[-1] if ref else None


def class_name(ref):
    return ref["ObjectName"].split("'")[1] if ref else None


def asset(ref, ext):
    """An object path as the runtime's asset path (`game/SurrounDead/Content/...png`)."""
    return "game/" + ref["ObjectPath"].rsplit(".", 1)[0] + ext if ref else None


def vec(v):
    return [v.get("X", 0.0) / 100.0, v.get("Z", 0.0) / 100.0, v.get("Y", 0.0) / 100.0]


def quat(q):
    return [q.get("X", 0.0), q.get("Z", 0.0), q.get("Y", 0.0), -q.get("W", 1.0)]


def colour(c):
    return [c.get("R", 1.0), c.get("G", 1.0), c.get("B", 1.0), c.get("A", 1.0)]


def event(e):
    t, p = e["Type"], e.get("Properties") or {}
    if t == "Event_SpawnMarker_C":
        return {"kind": "SpawnMarker", "at": vec(p.get("Location") or {}), "texture": asset(p.get("Marker Texture"), ".png"), "text": p.get("Text", ""),
                "tag": p.get("ActorTag", ""), "colour": colour(p.get("Color") or {}), "underground": p.get("Underground?", False)}
    if t == "Event_RemoveMarker_C":
        return {"kind": "RemoveMarker", "tag": p.get("ActorTag", "")}
    if t == "Event_SpawnActor_C":
        tr = p.get("Spawn Transform") or {}
        return {"kind": "SpawnActor", "class": class_name(p.get("Actor")), "at": vec(tr.get("Translation") or {}), "rotation": quat(tr.get("Rotation") or {}),
                "tag": p.get("Tag", ""), "argument": p.get("QuestArgument", "")}
    if t == "Event_RemoveActor_C":
        return {"kind": "RemoveActor", "tag": p.get("Tag", "")}
    if t == "Event_RewardPlayerXP_C":
        return {"kind": "RewardPlayerXP", "amount": p.get("Amount", 0.0)}
    if t == "NE_ForgetQuest_C":
        return {"kind": "Forget", "quest": class_name(p.get("Quest"))}
    if t == "Event_SpawnPrefab_C":
        tr = p.get("Transform") or {}
        return {"kind": "SpawnPrefab", "class": class_name(p.get("Prefab")), "at": vec(tr.get("Translation") or {}), "rotation": quat(tr.get("Rotation") or {}), "tag": p.get("Tag", "")}
    if t == "Event_SpawnAIHumanSpawner_C":
        # BP_AISpawningVolume_HumanQuest at Spawn Transform: Spawning AI, its box (the event's
        # defaults 1500 x 1500 x 200 cm), QuestArgument for the AI's KillNPC data task.
        tr = p.get("Spawn Transform") or {}
        spawning = [[class_name(next((v for k, v in el.items() if k.startswith("AICharacter")), None)), next((v for k, v in el.items() if k.startswith("SpawnCount")), 0)]
                    for el in p.get("Spawning AI") or []]
        return {"kind": "SpawnAIHumanSpawner", "at": vec(tr.get("Translation") or {}), "rotation": quat(tr.get("Rotation") or {}), "tag": p.get("Tag", ""),
                "argument": p.get("Argument for Quest", ""), "spawning": [s for s in spawning if s[0]],
                "extent": [p.get("Box Extent X", 1500.0) / 100.0, p.get("Box Extent Z", 200.0) / 100.0, p.get("Box Extent Y", 1500.0) / 100.0]}
    if t == "Event_RewardGiveItem_C":
        # S_RewardGiveItem's DT (a JigsawItem data asset) and Amount; one of ItemSet1-3 at random
        # with RandomSetOfItems?. The laptop quests' entries name no item (EX_NoObject).
        sets = [[[obj_name(next((v for k, v in it.items() if k.startswith("DT")), None)), next((v for k, v in it.items() if k.startswith("Amount")), 1)]
                 for it in p.get(f"ItemSet{i}") or [] if next((v for k, v in it.items() if k.startswith("DT")), None)] for i in (1, 2, 3)]
        return {"kind": "RewardGiveItem", "random": p.get("RandomSetOfItems?", False), "sets": sets}
    return {"kind": t.removesuffix("_C"), "supported": False}


def task(e):
    t, p = e["Type"], e.get("Properties") or {}
    common = {"description": text(p.get("DescriptionOverride")), "optional": p.get("bOptional", False), "hidden": p.get("bHidden", False)}
    if t == "BPT_GoToLocation_C":
        return {"kind": "GoToLocation", "at": vec(p.get("GoalLocation") or {}), "tolerance": p.get("DistanceTolerance", GOTO_TOLERANCE) / 100.0,
                "location": text(p.get("FriendlyLocationName")) or GOTO_NAME, "invert": p.get("Invert?", False), **common}
    if t == "BPT_CompleteDataTask_C":
        dt = p.get("DataTask")
        return {"kind": "DataTask", "task": obj_name(dt) if dt else "LooseTask", "argument": p.get("Argument", ""), "quantity": p.get("RequiredQuantity", 1),
                "retroactive": p.get("Retroactive?", False), **common}
    return {"kind": t.removesuffix("_C"), "supported": False, **common}


def quest(path):
    exports = load(path)
    by_name = {e["Name"]: e for e in exports}
    cls = next(e for e in exports if e["Type"] == "QuestBlueprintGeneratedClass")["Name"]
    cdo = next(e for e in exports if e["Name"] == f"Default__{cls}").get("Properties") or {}
    template = next(e for e in exports if e["Type"] == "Quest").get("Properties") or {}
    states = {e["Name"]: e.get("Properties") or {} for e in exports if e["Type"] == "QuestState"}
    branches = {e["Name"]: e.get("Properties") or {} for e in exports if e["Type"] == "QuestBranch"}
    sid = {n: p.get("ID", n) for n, p in states.items()}
    bid = {n: p.get("ID", n) for n, p in branches.items()}
    # Several exports share a name (two Event_RemoveMarker_C_0 under different states): resolve
    # references by the export index in their ObjectPath.
    def by_ref(ref):
        idx = int(ref["ObjectPath"].rsplit(".", 1)[1])
        return exports[idx] if idx < len(exports) and exports[idx]["Name"] == obj_name(ref) else by_name[obj_name(ref)]

    out_states = {}
    for n, p in states.items():
        kind = (p.get("StateNodeType") or "EStateNodeType::Regular").split("::")[-1]
        out_states[sid[n]] = {"kind": kind, "events": [event(by_ref(r)) for r in p.get("Events") or [] if r], "branches": [bid[obj_name(r)] for r in p.get("Branches") or [] if r]}
    out_branches = {}
    for n, p in branches.items():
        out_branches[bid[n]] = {"to": sid.get(obj_name(p.get("DestinationState"))), "tasks": [task(by_ref(r)) for r in p.get("QuestTasks") or [] if r],
                                "events": [event(by_ref(r)) for r in p.get("Events") or [] if r]}
    return cls, {
        "name": text(cdo.get("QuestName")),
        "description": text(cdo.get("QuestDescription")),
        "start": sid[obj_name(template["QuestStartState"])],
        "states": out_states,
        "branches": out_branches,
    }


def pickup_items():
    """Pickup BP class -> item data asset (the DA's PickupClass)."""
    out = {}
    for f in (ROOT / "Inventory/Items/DataAssets").rglob("*.json"):
        for e in load(f):
            ref = (e.get("Properties") or {}).get("PickupClass")
            if ref and e["Name"].startswith("DA_"):
                out.setdefault(class_name(ref), e["Name"])
    return out


def crate():
    exports = load(ROOT / "Quests/Blueprints/BP_GuaranteedSpawner.json")
    named = {e["Name"]: e.get("Properties") or {} for e in exports}
    cdo = named["Default__BP_GuaranteedSpawner_C"]
    items = pickup_items()
    drops = [items.get(class_name(r)) for r in cdo.get("Items") or []]
    missing = [class_name(r) for r, d in zip(cdo.get("Items") or [], drops) if d is None]
    if missing:
        sys.exit(f"crate pickups without an item: {missing}")
    rel = lambda n, k: named.get(n, {}).get(k) or {}
    arrow = rel("Arrow_GEN_VARIABLE", "RelativeLocation")
    lid = rel("SM_Lid_GEN_VARIABLE", "RelativeLocation")
    timeline = next(e for e in exports if e["Type"] == "TimelineTemplate")["Properties"]
    mesh = lambda n: rel(n, "StaticMesh").get("ObjectPath", "").rsplit(".", 1)[0] or None
    return {
        "class": "BP_GuaranteedSpawner_Classified_C",
        "name": text(cdo.get("ContainerName")),
        "hold": cdo.get("DurationToHold", 0.0),
        "noise": cdo.get("DefaultChanceToProduceNoise", 0.0),
        "items": drops,
        "mesh": mesh("StaticMesh_GEN_VARIABLE"),
        "lid": mesh("SM_Lid_GEN_VARIABLE"),
        # SM_Lid under StaticMesh; Scene (where the weapon spawns) under Arrow, at its origin.
        "lid_at": vec(lid),
        "drop_at": vec(arrow),
        "lid_seconds": timeline.get("TimelineLength", 1.5),
    }


def main():
    quests = dict(quest(p) for p in sorted((ROOT / "Quests/Quests").rglob("Quest_*.json")))
    data = {"quests": quests, "crate": crate()}
    OUT.write_text(json.dumps(data, indent=1, sort_keys=True) + "\n")
    print(f"{OUT}: {len(quests)} quests")


if __name__ == "__main__":
    main()
