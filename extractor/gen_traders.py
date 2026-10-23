#!/usr/bin/env python3
"""Trader classes (BP_TraderMaster's children) and their stock tables -> assets/data/traders.json.

usage: gen_traders.py <json dump root>   (normally run via `export_assets.sh --items`)

Needs dumps of Content/Inventory/Traders/ and Content/Quests/Dialogue/Traders/. Writes:

  classes  trader BP class (as in actors.json) -> VendorName, its DT_VendorTable row
           (Enum_VendorTypes' display name; SetItemsToSell), skeletal mesh (AnimBP_Trader plays
           A_Idle on every one),
           HairMeshes / BeardMeshes (UserConstructionScript picks one at random; null = none),
           its BP_JigMultiplayer vendor settings (grid, accepted currency, the random
           VendorMaxCurrencyAmount range, selling allowed, resold items) and its dialogue
           (speaker, the NPC's opening line and alternatives, the player's replies; `trade`
           marks the reply whose NPC node fires Event_OpenNPCVendor)
  tables   DT_VendorTable row -> [{item, chance %, min, max}] (Struct_ItemsToSell, rolled like
           a container's StartingItems)

Values are inherited down the chain (child overrides BP_TraderMaster).
"""
import json
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
OUT = Path(__file__).resolve().parent.parent / "assets" / "data" / "traders.json"
CONTENT = "SurrounDead/Content/"


def load(p):
    return json.loads(p.read_text())


def text(t):
    return (t or {}).get("SourceString") or (t or {}).get("CultureInvariantString")


def field(d, prefix):
    """Blueprint struct fields carry GUID suffixes: ItemID_41_8C5E..."""
    for k, v in (d or {}).items():
        if k.split("_")[0] == prefix:
            return v
    return None


def pkg(ref):
    return ref["ObjectPath"].split(".")[0] if ref else None


def pkg_name(ref):
    return pkg(ref).rsplit("/", 1)[1] if ref else None


def read_class(f):
    exports = load(f)
    cls = next((e for e in exports if e["Type"] == "BlueprintGeneratedClass"), None)
    if not cls:
        return None
    cdo = next((e for e in exports if e["Name"].startswith("Default__")), {}).get("Properties") or {}
    comps = {}
    for e in exports:
        name = e["Name"].removeprefix("ICH-")
        if name.endswith("_GEN_VARIABLE"):
            comps.setdefault(name.removesuffix("_GEN_VARIABLE"), {}).update(e.get("Properties") or {})
    sup = (cls.get("Super") or cls.get("SuperStruct") or {}).get("ObjectName", "")
    return cls["Name"], {"super": sup.split("'")[1] if "'" in sup else None, "cdo": cdo, "comps": comps}


def dialogue(path):
    """Narrative's Dialogue: the root NPC node's lines and the player's replies."""
    if not path:
        return None
    f = ROOT / (path.removeprefix(CONTENT) + ".json")
    if not f.exists():
        return None
    exports = load(f)
    nodes = {e["Name"]: e for e in exports}

    def node(ref):
        # By the ObjectPath's export index: names repeat across outers (two Condition_CheckMoney_C_0).
        if not ref:
            return None
        idx = ref.get("ObjectPath", "").rsplit(".", 1)[-1]
        if idx.isdigit() and int(idx) < len(exports):
            return exports[int(idx)]
        return nodes.get(ref["ObjectName"].rstrip("'").rsplit(".", 1)[-1])

    template = next((e for e in nodes.values() if e["Type"] == "Dialogue"), None)
    root = node((template or {}).get("Properties", {}).get("RootDialogue"))
    if not root:
        return None
    p = root.get("Properties", {})
    lines = [text((p.get("Line") or {}).get("Text"))] + [text(a.get("Text")) for a in p.get("AlternativeLines", [])]
    def condition(ref):
        c = node(ref) or {}
        cp = c.get("Properties", {})
        out = {"class": c.get("Type", "").removesuffix("_C"), "not": bool(cp.get("bNot"))}
        if "Price" in cp:
            out["price"] = cp["Price"]
            out["currency"] = pkg_name(cp.get("Currency"))
        return out

    def branch(ref, depth=0):
        """An NPC node: its line (None for a silent one), the conditions that pick it, the
        events it fires and the NPC nodes that follow it (Narrative takes the first valid)."""
        n = (node(ref) or {}).get("Properties", {})
        return {
            "line": text((n.get("Line") or {}).get("Text")),
            "conditions": [condition(c) for c in n.get("Conditions", []) or [] if c],
            "events": [(node(e) or {}).get("Type", "").removesuffix("_C") for e in n.get("Events", []) or [] if e],
            "next": [branch(x, depth + 1) for x in n.get("NPCReplies", []) or [] if x] if depth < 8 else [],
        }

    replies = []
    for r in p.get("PlayerReplies", []):
        rp = (node(r) or {}).get("Properties", {})
        branches = [branch(n) for n in rp.get("NPCReplies", []) or [] if n]
        # The NPC node a reply leads to opens the trade screen through its Event_OpenNPCVendor_C_0,
        # an Event_OnInteractNPC (the NPC's OnExecuteInteractDialogue: a trader's trade screen).
        trade = any("Event_OnInteractNPC" in b["events"] for b in branches)
        replies.append({"text": text((rp.get("Line") or {}).get("Text")), "trade": trade, "branches": branches, "x": (rp.get("NodePos") or {}).get("X", 0.0)})
    # Listed left to right as laid out in the dialogue graph (guess: Narrative's sorting is
    # native code; the graphs put the replies side by side under the NPC's node).
    replies.sort(key=lambda r: r["x"])
    for r in replies:
        del r["x"]
    return {"speaker": p.get("SpeakerID"), "lines": [l for l in lines if l], "replies": replies}


def main():
    classes = dict(c for f in (ROOT / "Inventory/Traders").rglob("BP_*.json") if (c := read_class(f)))

    def chain(name):
        while name in classes:
            yield classes[name]
            name = classes[name]["super"]

    def cdo(name, key):
        return next((c["cdo"][key] for c in chain(name) if key in c["cdo"]), None)

    def comp(name, comp_name, key):
        return next((c["comps"][comp_name][key] for c in chain(name) if key in c["comps"].get(comp_name, {})), None)

    def is_trader(name):
        return any(c is classes.get("BP_TraderMaster_C") for c in chain(name)) and name != "BP_TraderMaster_C"

    # Enum_VendorTypes value -> DT_VendorTable row (SetItemsToSell looks rows up by display name).
    enum = {}
    for e in load(ROOT / "Inventory/Traders/Enum_VendorTypes.json"):
        for kv in e.get("Properties", {}).get("DisplayNameMap", []):
            enum[f"Enum_VendorTypes::{kv['Key']}"] = text(kv["Value"])
        names = e.get("Names") or {}
    first = min(names, key=names.get) if names else None

    tables = {}
    for row, v in load(ROOT / "Inventory/Traders/DT_VendorTable.json")[0].get("Rows", {}).items():
        tables[row] = [
            {"item": pkg_name(field(r, "ItemID")), "chance": field(r, "SpawnChance") or 0.0, "min": field(r, "RandomCountMin") or 1, "max": field(r, "RandomCountMax") or 1}
            for r in field(v, "ItemsToSell") or [] if field(r, "ItemID")
        ]

    out = {}
    for name in sorted(n for n in classes if is_trader(n)):
        settings = (comp(name, "BP_JigMultiplayer", "ContainersSettings") or [{}])[0]
        pool = comp(name, "BP_JigMultiplayer", "VendorMaxCurrencyAmount") or {"X": 0.0, "Y": 0.0}
        out[name] = {
            "name": text(cdo(name, "VendorName")) or "Trader",
            # Unset = the enum's first value (Level1SuppliesTrader).
            "table": enum.get(cdo(name, "VendorTable") or first),
            "mesh": pkg(comp(name, "SkeletalMesh", "SkeletalMesh")),
            "hair": [pkg(m) for m in cdo(name, "HairMeshes") or []],
            "beard": [pkg(m) for m in cdo(name, "BeardMeshes") or []],
            "grid": [field(settings, "Columns") or 13, field(settings, "Rows") or 15],
            "currency": pkg_name(comp(name, "BP_JigMultiplayer", "VendorAcceptedCurrencyID")),
            "pool": [pool["X"], pool["Y"]],
            # VendorCurrentCurrencyAmount before BeginPlay re-rolls it (shown as is when traders
            # have unlimited money: TRADERS HAVE A SET AMOUNT OF CURRENCY off).
            "amount": comp(name, "BP_JigMultiplayer", "VendorCurrentCurrencyAmount") or 0.0,
            "selling": comp(name, "BP_JigMultiplayer", "AllowSellingItemsToVendor?") is not False,
            "resell": bool(comp(name, "BP_JigMultiplayer", "VendorReSellSoldItems?")),
            "dialogue": dialogue(pkg(cdo(name, "Dialogue"))),
        }
    # The wandering trader (BP_MasterTrader, a SmartAI character spawned by the level's
    # BP_AISpawningVolume_RandomChanceToSpawn): BeginPlay's SetItemsToSell(22) (RandomTraderType's
    # 22/23/24 pick is never called), its BP_JigMultiplayer and Dialogue_WanderingTrader. Its
    # body, weapons and health are the AI's (zombies.json); the name is its widget's title.
    wf = ROOT / "AI/Humans/Trader/BP_MasterTrader.json"
    if wf.exists():
        name, w = read_class(wf)
        jig = w["comps"].get("BP_JigMultiplayer", {})
        settings = (jig.get("ContainersSettings") or [{}])[0]
        pool = jig.get("VendorMaxCurrencyAmount") or {"X": 0.0, "Y": 0.0}
        by_value = {v: k for k, v in names.items()}
        out[name] = {
            "name": "Wandering Trader",
            "table": enum.get(by_value.get(22)),
            "mesh": None, "hair": [], "beard": [],
            "grid": [field(settings, "Columns") or 13, field(settings, "Rows") or 15],
            "currency": pkg_name(jig.get("VendorAcceptedCurrencyID")),
            "pool": [pool["X"], pool["Y"]],
            "amount": jig.get("VendorCurrentCurrencyAmount") or 0.0,
            "selling": jig.get("AllowSellingItemsToVendor?") is not False,
            "resell": bool(jig.get("VendorReSellSoldItems?")),
            "dialogue": dialogue(pkg(w["cdo"].get("Dialogue"))),
            "wandering": True,
        }
    # Quest givers (BP_QuestGiver's children: the settlement's Settlement Officer): no stock, a
    # Name, a dialogue; BP_QuestGiver's body is SK_Chr_Sheriff_Male_01 (A_Idle) with a revolver
    # on the Sidearm socket.
    for qf in (ROOT / "Quests/Blueprints/NPCs").glob("BP_QuestGiver_*.json"):
        name, q = read_class(qf)
        base = read_class(ROOT / "Quests/Blueprints/NPCs/BP_QuestGiver.json")[1]
        comp_of = lambda c, k: q["comps"].get(c, {}).get(k) or base["comps"].get(c, {}).get(k)  # noqa: E731
        out[name] = {
            "name": text(q["cdo"].get("Name") or base["cdo"].get("Name")) or "Quest Giver",
            "table": None,
            "mesh": pkg(comp_of("SkeletalMesh", "SkeletalMesh")),
            "hair": [], "beard": [],
            "grid": [0, 0], "currency": None, "pool": [0.0, 0.0], "amount": 0.0,
            "selling": False, "resell": False,
            "dialogue": dialogue(pkg(q["cdo"].get("Dialogue"))),
            "quest_giver": True,
            "gun": pkg(comp_of("Gun", "SkeletalMesh")),
            # InteractionArgument for CompleteNarrativeDataTask(InteractWithActor).
            "argument": q["cdo"].get("InteractionArgument", base["cdo"].get("InteractionArgument", "")),
        }
    OUT.write_text(json.dumps({"classes": out, "tables": tables}, indent=1, sort_keys=True))
    missing = sorted({c["table"] for c in out.values() if c["table"] and c["table"] not in tables})
    print(f"traders: {len(out)} classes, {len(tables)} tables; unknown tables: {missing}")


main()
