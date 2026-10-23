#!/usr/bin/env python3
"""Consistency checks on the committed game data (assets/data/*.json) and, when present, the
generated assets they point at. Run by scripts/check.sh; exits non-zero on errors.

Add a check: write a function taking `ctx` that calls ctx.err(...) / ctx.warn(...), and list it
in CHECKS.
"""
import json
import re
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "assets" / "data"
GAME = ROOT / "assets" / "game"


class Ctx:
    def __init__(self):
        self.errors, self.warnings = [], []
        self.items = json.loads((DATA / "items.json").read_text())
        self.menus = json.loads((DATA / "menus.json").read_text())

    def err(self, msg):
        self.errors.append(msg)

    def warn(self, msg):
        self.warnings.append(msg)


def recipes_reference_real_items(ctx):
    items = ctx.items["items"]
    for table, recipes in ctx.items.get("recipes", {}).items():
        for r in recipes:
            for name in [r["item"], *[n for n, _ in r["needs"]]]:
                if name not in items:
                    ctx.err(f"recipe {table}/{r['item']}: unknown item {name}")
    if not ctx.items.get("recipes", {}).get("Player"):
        ctx.err("no 'Player' recipes (the CRAFTING tab would be empty)")


def weapons_and_magazines_are_consistent(ctx):
    items = ctx.items["items"]
    for k, d in items.items():
        for a in d.get("attachments", []):
            if a["item"] not in items:
                ctx.err(f"{k}: default attachment {a['item']} unknown")
            if a.get("kind") == "Mag" and not items.get(a["item"], {}).get("capacity"):
                ctx.err(f"{k}: magazine {a['item']} has no capacity")
        for ammo in d.get("ammo", []):
            if ammo not in items:
                ctx.err(f"{k}: accepts unknown ammo {ammo}")
        if d.get("type") in ("MainFirearm", "Sidearm"):
            stats = {s["stat"] for s in d.get("random_stats", [])}
            if "FirearmDamage" not in stats:
                ctx.warn(f"{k}: firearm without FirearmDamage (falls back to 30)")


def equip_sockets_exist(ctx):
    sockets = ctx.items.get("sockets", {})
    if not sockets:
        ctx.err("no skeleton sockets exported")
    # UE resolves a socket name that isn't a socket as a bone name.
    bones = {s["bone"] for s in sockets.values()} | {"spine_01", "spine_02", "spine_03", "Pelvis", "head", "Hand_R", "Hand_L"}
    bad = set()
    for k, d in ctx.items["items"].items():
        if d.get("type") not in ("MainFirearm", "Sidearm", "Melee"):
            continue
        for field, name in (d.get("equip") or {}).items():
            if "Socket" in field and isinstance(name, str) and name != "None" and name not in sockets and name not in bones:
                bad.add(f"{k}.{field}={name}")
    if bad:
        ctx.err(f"weapon sockets that are neither sockets nor bones: {sorted(bad)[:5]}")


def loot_tables_resolve(ctx):
    items, tables = ctx.items["items"], ctx.items["loot_tables"]
    for cls, c in ctx.items["containers"].items():
        if c["table"] and c["table"] not in tables:
            ctx.err(f"container {cls}: unknown loot table {c['table']}")
    missing = {r["item"] for rows in tables.values() for r in rows if r["item"] not in items}
    if missing:
        ctx.warn(f"{len(missing)} loot rows name items that aren't exported (skipped at roll time)")


def difficulty_defaults_in_range(ctx):
    for group in ("settings", "difficulty"):
        for page in ctx.menus[group]:
            for r in page["rows"]:
                d = r.get("default")
                if r.get("kind") == "slider" and d is not None and not (r["min"] <= d <= r["max"]):
                    ctx.err(f"{group}/{page['name']}/{r['label']}: default {d} outside {r['min']}..{r['max']}")
    day = next((r for p in ctx.menus["difficulty"] for r in p["rows"] if r.get("label") == "DAY LENGTH"), None)
    if not day or day.get("default") != 30.0:
        ctx.err("DAY LENGTH default isn't Save_Difficulties' 30")


def generated_assets_present(ctx):
    if not GAME.exists():
        ctx.warn("assets/game missing (run extractor/export_assets.sh --all): asset checks skipped")
        return
    meshes = [d["mesh"] for d in ctx.items["items"].values() if d.get("mesh")]
    missing = [m for m in meshes if not (GAME / f"{m}.glb").exists()]
    if missing:
        ctx.warn(f"{len(missing)} item meshes not exported (e.g. {missing[0]})")
    mats = GAME / "materials.json"
    if mats.exists():
        for name, m in json.loads(mats.read_text()).items():
            tex = m.get("texture")
            if tex and not (ROOT / "assets" / tex).exists():
                ctx.warn(f"material {name}: texture {tex} missing")
                break
    for tex in ["UI/Lockpick/Textures/LockMobile", "UI/Textures/ClockInGame", "UI/Textures/Ammo/FireType_Semi"]:
        if not (GAME / "SurrounDead/Content" / f"{tex}.png").exists():
            ctx.err(f"UI texture {tex} not exported")


def zombie_spawners_carry_props(ctx):
    path = GAME / "levels" / "LongdownValley" / "actors.json"
    if not path.exists():
        return  # generated_assets_present warns
    spawners = [a for a in json.loads(path.read_text()) if a["class"] == "BP_AISpawner_Zombies_C"]
    if not spawners:
        ctx.err("actors.json has no BP_AISpawner_Zombies_C")
    elif all(a.get("props") is None for a in spawners):
        ctx.err("actors.json predates spawner props (run SdExport levelactors, see Level.cs)")
    elif not any("SpawnAmount" in (a.get("props") or {}) for a in spawners):
        ctx.err("no zombie spawner overrides SpawnAmount (props not read?)")


def map_and_pois_exported(ctx):
    """The map (gameplay/map.rs): BP_MapData's texture and every BP_POIManager's props and icon."""
    path = GAME / "levels" / "LongdownValley" / "actors.json"
    if not path.exists():
        return  # generated_assets_present warns
    actors = json.loads(path.read_text())
    data = [a for a in actors if a["class"] == "BP_MapData_C"]
    if not data or not (data[0].get("props") or {}).get("World Size"):
        ctx.err("actors.json has no BP_MapData props (run SdExport levelactors, see Level.cs Mapped)")
    elif (data[0]["props"].get("Map Texture")) != "RT_WorldImageCreator_Tex":
        ctx.err(f"map texture is {data[0]['props'].get('Map Texture')} (the port loads RT_WorldImageCreator_Tex)")
    if not (GAME / "SurrounDead/Content/Blueprints/MinimapSystem/Texture/RT_WorldImageCreator_Tex.png").exists():
        ctx.err("the map texture isn't exported (export_assets.sh)")
    pois = [a for a in actors if a["class"] == "BP_POIManager_C"]
    if len(pois) < 100 or any("POI" not in (a.get("props") or {}) for a in pois):
        ctx.err(f"{len(pois)} POIs, some without their POI struct (run SdExport levelactors)")
        return
    for a in pois:
        poi = json.loads(a["props"]["POI"])
        img = next((v for k, v in poi.items() if k.startswith("MarkerImage_")), None)
        name = img["ObjectPath"].rsplit("/", 1)[1].removesuffix(".0") if img else None
        if not name or not (GAME / f"SurrounDead/Content/POI/{name}.png").exists():
            ctx.err(f"POI {a['name']}: marker image {name} not exported")


def difficulty_presets_resolve(ctx):
    presets = ctx.menus.get("difficulty_presets", [])
    if [p["name"] for p in presets] != ["Peaceful", "Easy", "Standard", "Hardened", "Nightmare"]:
        ctx.err("difficulty_presets aren't DT_Difficulty's five rows (export_assets.sh --menu-data)")
    rows = {(p["name"], r["label"]) for p in ctx.menus["difficulty"] for r in p["rows"] if r.get("label")}
    for p in presets:
        for v in p["values"]:
            if (v["page"], v["label"]) not in rows:
                ctx.err(f"preset {p['name']}: no difficulty row {v['page']}/{v['label']}")
        if len(p["values"]) != 22:
            ctx.err(f"preset {p['name']}: {len(p['values'])} values, SetDifficulty writes 22")


def firearms_have_component_data(ctx):
    for k, v in ctx.items["items"].items():
        gun = v["type"] in ("MainFirearm", "Sidearm")
        if gun != bool(v.get("firearm")):
            ctx.err(f"{k}: firearm data {'missing' if gun else 'on a non-firearm'} (export_assets.sh --items)")


def clothing_has_body_parts(ctx):
    # BodyPartVisibility needs each worn item's body-part meshes (gameplay::equipment::body_meshes).
    parts = {"Torso", "Arms", "Biceps", "Hands", "Legs", "LowerThighs", "LowerLegs", "Feet"}
    for k, v in ctx.items["items"].items():
        c = v.get("clothing")
        if not c:
            continue
        body = c.get("body") or {}
        for sex in ("male", "female"):
            if set(body.get(sex) or {}) != parts:
                ctx.err(f"{k}: clothing without {sex} body parts")
                continue
            for mesh in filter(None, body[sex].values()):
                if GAME.exists() and not (GAME / f"{mesh}.glb").exists():
                    ctx.err(f"{k}: body part {mesh} not exported")


def walk_clips_have_footsteps(ctx):
    # gameplay::footsteps follows the sequences' Footstep notifies (Anim.cs exports them).
    anims = GAME / "SurrounDead" / "Content" / "Animations" / "Anims"
    if not anims.exists():
        return
    for clip in ("No_Rifle/ThirdPersonRun", "No_Rifle/ThirdPersonSprint", "No_Rifle/A_Crouch_Walk",
                 "Rifle_Standing/A_AR_Run", "Pistol_Crouch/A_HG_Crouch_Walk"):
        f = anims / f"{clip}.anim.json"
        if not f.exists():
            ctx.err(f"{clip}: not exported")
            continue
        d = json.loads(f.read_text())
        steps = [n["time"] for n in d.get("notifies", []) if n["name"] == "Footstep"]
        if len(steps) != 2 or not all(0 <= t <= d["duration"] + 0.05 for t in steps):
            ctx.err(f"{clip}: footsteps {steps} in a {d['duration']:.2f} s clip")
    crouch = json.loads((anims / "No_Rifle/A_Crouch_Walk.anim.json").read_text())
    if crouch.get("rate") != 0.8:
        ctx.err(f"A_Crouch_Walk: RateScale {crouch.get('rate')} (0.8 in the game)")


def character_clips_dont_scale(ctx):
    # Survivor and zombie clips never scale bones; a scale (Fire_Rifle_Ironsights came out at 4,
    # an additive clip) blows up the parts hung off bones, like hair on the head.
    anims = GAME / "SurrounDead" / "Content" / "Animations"
    for f in sorted(anims.rglob("*.anim.json")):
        d = json.loads(f.read_text())
        worst = max((abs(x - 1) for v in d["bones"].values() for k in v["s"] for x in k), default=0)
        if worst > 0.01:
            ctx.err(f"{f.relative_to(anims)}: bone scale off by {worst:.2f}")


def traders_resolve(ctx):
    """traders.json (gen_traders.py): every class's stock table, currency and stock items exist,
    each trader that trades has a dialogue reply that opens the trade screen, and its body and
    hair were exported."""
    path = DATA / "traders.json"
    if not path.exists():
        ctx.err("assets/data/traders.json missing (export_assets.sh --items)")
        return
    t = json.loads(path.read_text())
    items = ctx.items["items"]
    for name, c in t["classes"].items():
        d = c.get("dialogue") or {}
        if c.get("quest_giver"):
            # A quest giver sells nothing: its dialogue's replies must lead somewhere instead.
            if not any(r.get("branches") for r in d.get("replies", [])):
                ctx.err(f"quest giver {name}: no reply with NPC nodes (gen_traders.py branch())")
        else:
            if c["table"] not in t["tables"]:
                ctx.err(f"trader {name}: unknown table {c['table']}")
            if c["currency"] not in items:
                ctx.err(f"trader {name}: unknown currency {c['currency']}")
            if d.get("replies") and not any(r["trade"] for r in d["replies"]):
                ctx.err(f"trader {name}: no reply opens the trade screen")
        if GAME.exists():
            for m in [c["mesh"], *c["hair"], *c["beard"], c.get("gun")]:
                if m and not (GAME / f"{m}.glb").exists():
                    ctx.warn(f"trader {name}: {m}.glb not exported (export_assets.sh --models)")
    for table, rows in t["tables"].items():
        for r in rows:
            if r["item"] not in items:
                ctx.err(f"trader table {table}: unknown item {r['item']}")


def airdrops_resolve(ctx):
    """The three airdrop crates are containers with real loot tables and marker data, and the
    level's airdrop region and event manager carry their instance values."""
    containers, tables = ctx.items["containers"], ctx.items["loot_tables"]
    for c in ("AirdropContainer_Military_C", "AirdropContainer_Medical_C", "AirdropContainer_Supplies_C"):
        d = containers.get(c)
        if not d:
            ctx.err(f"airdrop {c} missing from items.json containers")
            continue
        if d["table"] not in tables or not d.get("airdrop") or not d.get("mesh"):
            ctx.err(f"airdrop {c}: table {d['table']}, marker {d.get('airdrop')}, mesh {d.get('mesh')}")
    actors = GAME / "levels" / "LongdownValley" / "actors.json"
    if actors.exists():
        found = {a["class"]: a.get("props") or {} for a in json.loads(actors.read_text()) if a["class"] in ("BP_Event_AirdropGoToLocation_C", "BP_EventManager_C")}
        if "RegionRadius" not in found.get("BP_Event_AirdropGoToLocation_C", {}):
            ctx.err("actors.json: BP_Event_AirdropGoToLocation without RegionRadius (SdExport levelactors)")
        if "DistanceToSpawnAirdrop" not in found.get("BP_EventManager_C", {}):
            ctx.err("actors.json: BP_EventManager without DistanceToSpawnAirdrop (SdExport levelactors)")


def harvesting_resolves(ctx):
    """The harvest yields are items, axes cut trees, picks break rocks and crowbars trash piles
    (CanDamageTree? / CanDamageRock? / CanDamageTrashObjects?), and harvest.json's placement
    indices exist in placements.json (one trash entry per TrashObject actor)."""
    items = ctx.items["items"]
    for i in ("DA_WoodenLog", "DA_Rock", "DA_ScrapMetal"):
        if i not in items:
            ctx.err(f"harvest yield {i} missing from items.json")
    if "tree" not in items.get("DA_FiremansAxe", {}).get("harvests", []) or "rock" not in items.get("DA_Pickaxe", {}).get("harvests", []):
        ctx.err("items.json: the Fireman's Axe / Pickaxe lost their harvests (gen_items.py harvests())")
    if "trash" not in items.get("DA_Crowbar", {}).get("harvests", []) or "trash" in items.get("DA_FiremansAxe", {}).get("harvests", []):
        ctx.err("items.json: only some melee weapons break trash (the Crowbar does, the Fireman's Axe doesn't)")
    for cue in ("HittingTrash_Cue", "WoodBreaking_Cue"):
        if cue not in json.loads((DATA / "sounds.json").read_text())["cues"]:
            ctx.err(f"sounds.json: TrashObject's {cue} missing (gen_sounds.py)")
    level = GAME / "levels" / "LongdownValley"
    if (level / "placements.json").exists():
        if not (level / "harvest.json").exists():
            ctx.err("harvest.json missing (SdExport levelharvest)")
            return
        placements = json.loads((level / "placements.json").read_text())
        harvest = json.loads((level / "harvest.json").read_text())
        for kind in ("tree", "stone", "trash"):
            n = 0
            for mesh, idx in harvest.get(kind, {}).items():
                n += len(idx)
                if mesh not in placements or max(idx) >= len(placements[mesh]):
                    ctx.err(f"harvest.json {kind} {mesh}: indices beyond placements.json")
            if n == 0:
                ctx.err(f"harvest.json: no {kind} instances")
        actors = json.loads((level / "actors.json").read_text())
        trash = sum(len(i) for i in harvest.get("trash", {}).values())
        if trash != sum(a["class"] == "TrashObject_C" for a in actors):
            ctx.err(f"harvest.json: {trash} trash piles, not one per TrashObject actor")
        if any("SwampGrass" in m for m in harvest.get("tree", {})):
            ctx.err("harvest.json: swamp reeds ignore the Weapons channel, they aren't trees")
        # Swings (and bodies, bullets, sight) meet trees through their simple collision.
        simple = json.loads((DATA / "simple_collision.json").read_text()) if (DATA / "simple_collision.json").exists() else {}
        for mesh in harvest.get("tree", {}):
            if not simple.get(mesh):
                ctx.err(f"simple_collision.json: no collision for harvestable tree {mesh} (gen_collision.py)")


def vehicles_resolve(ctx):
    """vehicles.json (gen_vehicles.py): 17 classes with four or more wheels placed on their bones,
    a steering and a driven wheel, a torque curve and gears; spawner rows name real classes; the
    meshes are exported and each class's Colors and slot-0 material are in materials.json; the
    level's spawners carry VehicleSelection; Difficulty.VehicleChance defaults to 30
    (SD_GameInstance)."""
    d = json.loads((DATA / "vehicles.json").read_text())
    classes = d["classes"]
    if len(classes) != 17:
        ctx.err(f"vehicles.json: {len(classes)} classes (want BP_VehicleMaster's 17 children)")
    for k, c in classes.items():
        w = c["wheels"]
        if len(w) < 4 or any(x["at"] is None for x in w) or not any(x["steering"] for x in w) or not any(x["engine"] for x in w):
            ctx.err(f"vehicles.json {k}: wheels {[(x['bone'], x['at']) for x in w]}")
        if not c["transmission"]["forward"] or not c["engine"]["torque_curve"]:
            ctx.err(f"vehicles.json {k}: no gears or torque curve")
    for r in d["spawners"].values():
        for c in r["classes"]:
            if c not in classes:
                ctx.err(f"vehicles.json spawner row {r['row']}: unknown class {c}")
    mats = GAME / "materials.json"
    if mats.exists():
        known = json.loads(mats.read_text())
        for k, c in classes.items():
            for m in [c["mesh"], *c["extra_meshes"]]:
                if not (GAME / f"{m}.glb").exists():
                    ctx.err(f"{k}: mesh {m} not exported (export_assets.sh --vehicles)")
            for col in c["colors"]:
                if col.rsplit("/", 1)[1] not in known:
                    ctx.err(f"{k}: colour {col} not in materials.json")
    actors = GAME / "levels" / "LongdownValley" / "actors.json"
    if actors.exists():
        sp = [a for a in json.loads(actors.read_text()) if a["class"] == "BP_VehicleSpawner_C"]
        if not sp or not any("VehicleSelection" in (a.get("props") or {}) for a in sp):
            ctx.err("actors.json: vehicle spawners without VehicleSelection (Level.cs Vehicles())")
    menus = json.loads((DATA / "menus.json").read_text())
    row = next((r for p in menus["difficulty"] for r in p["rows"] if r.get("cvar") == "Difficulty.VehicleChance"), None)
    if not row or row.get("default") != 30.0:
        ctx.err(f"menus.json: VEHICLE SPAWN CHANCE % default {row and row.get('default')} (SD_GameInstance registers 30)")


def quests_resolve(ctx):
    """quests.json: every quest's start and branch destinations are its states, the crate drops
    real items with exported meshes, marker textures exist, and the level's quest starters carry
    their quests (the laptop pool, BP_StartQuestArea's Quest and box)."""
    data = json.loads((DATA / "quests.json").read_text())
    quests, items = data["quests"], ctx.items["items"]
    for name, q in quests.items():
        states = q["states"]
        if q["start"] not in states:
            ctx.err(f"quest {name}: start {q['start']} isn't a state")
        for bid, b in q["branches"].items():
            if b["to"] not in states:
                ctx.err(f"quest {name}: branch {bid} leads to {b['to']}, not a state")
        for sid, st in states.items():
            for b in st["branches"]:
                if b not in q["branches"]:
                    ctx.err(f"quest {name}: state {sid} has unknown branch {b}")
            for e in st["events"]:
                tex = e.get("texture")
                if tex and GAME.exists() and not (ROOT / "assets" / tex).exists():
                    ctx.err(f"quest {name}: marker texture {tex} not exported")
        if not q["name"]:
            ctx.err(f"quest {name}: no QuestName")
    c = data["crate"]
    for i in c["items"]:
        if i not in items:
            ctx.err(f"quest crate drops unknown item {i}")
    for m in (c["mesh"], c["lid"]):
        if GAME.exists() and not (GAME / f"{m}.glb").exists():
            ctx.err(f"quest crate mesh {m} not exported (export_assets.sh --quests)")
    for q in ("Quest_TheSafeZone_C", "Quest_Dungeon_LaboratoryBunker_C"):
        if q not in quests:
            ctx.err(f"quests.json lacks {q}")
    actors = GAME / "levels" / "LongdownValley" / "actors.json"
    if actors.exists():
        found = {a["class"]: a.get("props") or {} for a in json.loads(actors.read_text()) if a["class"] in ("BP_LaptopEventManager_C", "BP_StartQuestArea_C")}
        pool = json.loads(found.get("BP_LaptopEventManager_C", {}).get("Quests", "[]"))
        names = [r["ObjectName"].split("'")[1] for r in pool if r]
        if len(names) != 16 or any(n not in quests for n in names):
            ctx.err(f"actors.json: the laptop manager's Quests pool {names} (SdExport levelactors)")
        area = found.get("BP_StartQuestArea_C", {})
        if area.get("Quest") not in quests or "Box.BoxExtent" not in area:
            ctx.err(f"actors.json: BP_StartQuestArea without its Quest / Box.BoxExtent: {area}")


def humans_resolve(ctx):
    """The human AI (zombies.json classes with `human`, `weapons`; gen_ai.py): every class's
    weapons are in the weapon table, its loot table exists, its meshes and weapon meshes are
    exported; the bandits, soldiers and scavengers are all there."""
    data = json.loads((DATA / "zombies.json").read_text())
    weapons = data.get("weapons", {})
    humans = {n: c for n, c in data["classes"].items() if c.get("human")}
    for want in ("BP_Bandit_Ranged_AssaultRifle_C", "BP_Bandit_Melee_C", "BP_Military_Rifleman_C", "BP_Scavenger_Male_C"):
        if want not in humans:
            ctx.err(f"zombies.json lacks the human class {want}")
    for name, c in humans.items():
        h = c["human"]
        for w in h["ranged"] + h["melee"]:
            if w not in weapons:
                ctx.err(f"{name}: weapon {w} not in zombies.json weapons")
        if c.get("loot_table") and c["loot_table"] not in ctx.items["loot_tables"]:
            ctx.err(f"{name}: loot table {c['loot_table']} unknown")
        if not h["ranged"] and not h["melee"]:
            ctx.err(f"{name}: no weapons")
    for name, w in weapons.items():
        if w["ranged"] and (w["range"] <= 0 or w["magazine"] <= 0 or w["interval"][0] <= 0):
            ctx.err(f"{name}: range {w['range']} magazine {w['magazine']} interval {w['interval']}")
    if not GAME.exists():
        return
    meshes = {m for c in humans.values() for m in c["meshes"] + c["human"]["hair"] + c["human"]["beard"]} | {w["mesh"] for w in weapons.values() if w.get("mesh")}
    meshes |= {c["human"]["backpack"]["mesh"] for c in humans.values() if c["human"].get("backpack")}
    for m in sorted(meshes):
        if not (GAME / "SurrounDead/Content" / f"{m}.glb").exists():
            ctx.err(f"human mesh {m} not exported (export_assets.sh --ai)")


def random_volumes_resolve(ctx):
    """The level's BP_AISpawningVolume_RandomChanceToSpawn (actors.json, SdExport levelactors):
    each has its Chance and a Spawning AI of known human classes (the scavengers' dogs aside);
    the wandering trader is a human class marked `trader` with a traders.json class (its table,
    a dialogue with a trade reply); the military and Shakes & Cones spawners keep their five
    spawn points."""
    zombies = json.loads((DATA / "zombies.json").read_text())["classes"]
    traders = json.loads((DATA / "traders.json").read_text())
    w = traders["classes"].get("BP_MasterTrader_C")
    if not w or not w.get("wandering") or w.get("table") not in traders["tables"]:
        ctx.err(f"traders.json: the wandering trader BP_MasterTrader_C is missing or has no table: {w and w.get('table')}")
    elif not any(r["trade"] for r in (w.get("dialogue") or {}).get("replies", [])):
        ctx.err("traders.json: Dialogue_WanderingTrader has no reply that opens the trade screen")
    if not (zombies.get("BP_MasterTrader_C", {}).get("human") or {}).get("trader"):
        ctx.err("zombies.json: BP_MasterTrader_C isn't a human class marked trader (gen_ai.py WANDERERS)")
    actors = GAME / "levels" / "LongdownValley" / "actors.json"
    if not actors.exists():
        return
    volumes = traders_found = 0
    for a in json.loads(actors.read_text()):
        props = a.get("props") or {}
        if a["class"] in ("BP_MilitaryTraderSpawner_C", "BP_ShakesConesSpawner_C"):
            missing = [k for k in range(5) if f"Spawn{k}.RelativeLocation" not in props]
            if missing:
                ctx.err(f"{a['name']}: spawn points {missing} not exported (Level.cs Placed)")
        if a["class"] != "BP_AISpawningVolume_RandomChanceToSpawn_C":
            continue
        volumes += 1
        classes = re.findall(r"BlueprintGeneratedClass'(\w+)'", props.get("Spawning AI", ""))
        if "Chance" not in props or not classes:
            ctx.err(f"{a['name']}: no Chance or Spawning AI (Level.cs AISpawningVolume)")
        for c in classes:
            if c not in zombies and "Dog" not in c:
                ctx.err(f"{a['name']}: spawns {c}, not in zombies.json")
        traders_found += "BP_MasterTrader_C" in classes
    if volumes < 60 or traders_found < 10:
        ctx.err(f"actors.json: {volumes} random-chance volumes, {traders_found} with wandering traders (want 69 / 17)")


def settlements_resolve(ctx):
    """BP_SettlementManager's Camps (actors.json, Level.cs Events) name the 4
    BP_SettlementCampSpawners, each with a Settlement prefab in prefabs.json that has its
    traders (traders.json classes), safe zone, well, crafting and cooking stations."""
    prefabs = json.loads((DATA / "prefabs.json").read_text())["prefabs"]
    traders = json.loads((DATA / "traders.json").read_text())["classes"]
    actors = GAME / "levels" / "LongdownValley" / "actors.json"
    if not actors.exists():
        return
    acts = json.loads(actors.read_text())
    manager = next((a for a in acts if a["class"] == "BP_SettlementManager_C"), None)
    camps = {a["name"]: a for a in acts if a["class"] == "BP_SettlementCampSpawner_C"}
    listed = re.findall(r"PersistentLevel\.(\w+)'", ((manager or {}).get("props") or {}).get("Camps", ""))
    if len(listed) != 4 or set(listed) != set(camps):
        ctx.err(f"actors.json: the settlement manager's Camps {listed} vs the camp spawners {sorted(camps)}")
    for name, a in camps.items():
        p = prefabs.get((a.get("props") or {}).get("Settlement", ""))
        if not p:
            ctx.err(f"{name}: Settlement {(a.get('props') or {}).get('Settlement')} not in prefabs.json")
            continue
        classes = {x["class"] for x in p["actors"]}
        need = {"BP_EnterSafeZone_C", "BP_WaterWell_C", "BP_CraftingStation_C", "BP_CookingStation_C", "BP_SafeZoneTrader_C", "BP_BlackMarketTrader_C", "BP_Marker_C"}
        if need - classes:
            ctx.err(f"{name}: its settlement lacks {sorted(need - classes)}")
        for t in ("BP_SafeZoneTrader_C", "BP_BlackMarketTrader_C"):
            if t not in traders:
                ctx.err(f"traders.json lacks the settlement trader {t}")


def prefabs_resolve(ctx):
    """The prefabs (prefabs.json, gen_prefabs.py): every level BP_PrefabSpawner names a known
    prefab; the ones without humans have containers the item data knows and zombie spawners with
    a zombie class; the spawners' marker textures and meshes are exported."""
    data = json.loads((DATA / "prefabs.json").read_text())
    prefabs, spawner = data["prefabs"], data["spawner"]
    zombies = json.loads((DATA / "zombies.json").read_text())["classes"]
    if len(prefabs) < 30:
        ctx.err(f"prefabs.json has {len(prefabs)} prefabs (want Content/Prefabs' 36)")
    for name, p in prefabs.items():
        for a in p["actors"]:
            if a["class"].startswith("Container_") and a["class"] not in ctx.items["containers"]:
                ctx.err(f"{name}: container {a['class']} not in items.json")
            if a["class"] in ("BP_AISpawner_Zombies_C", "BP_AISpawner_ZombieBosses_C") and a["props"].get("SpawnedActorClass", "").strip('"') not in zombies:
                ctx.err(f"{name}: zombie spawner without a known SpawnedActorClass ({a['props'].get('SpawnedActorClass')})")
    if not GAME.exists():
        return
    actors = GAME / "levels" / "LongdownValley" / "actors.json"
    if actors.exists():
        sites = [a for a in json.loads(actors.read_text()) if a["class"].startswith("BP_PrefabSpawner")]
        if len(sites) < 80:
            ctx.err(f"{len(sites)} prefab spawners in actors.json (want 90; SdExport levelactors)")
        for a in sites:
            pf = (a.get("props") or {}).get("Prefab")
            if pf not in prefabs:
                ctx.err(f"{a['name']}: prefab {pf} unknown")
    for path in [spawner.get("marker_texture"), *spawner.get("textures", {}).values()]:
        if path and not (ROOT / "assets" / path).exists():
            ctx.err(f"prefab marker texture {path} not exported")
    for mesh in sorted({m for p in prefabs.values() for m, _ in p["meshes"]}):
        if not (GAME / f"{mesh}.glb").exists():
            ctx.err(f"prefab mesh {mesh} not exported (export_assets.sh --prefabs)")


def particles_resolve(ctx):
    """The particle systems (particles.json, gen_particles.py): well-formed lookup tables, known
    materials, every system the code plays and every firearm's muzzle flash and brass present;
    with assets/game, the level's emitters resolved and the mesh emitters' meshes exported."""
    import re
    data = json.loads((DATA / "particles.json").read_text())
    systems, materials = data["systems"], data["materials"]
    if len(systems) < 35:
        ctx.err(f"particles.json has {len(systems)} systems (want the FX folders' 39)")

    def tables(obj, path):
        if isinstance(obj, dict):
            if {"n", "dims", "op", "v"} <= obj.keys():
                want = obj["n"] * obj["dims"] * (1 if obj["op"] == "none" else 2)
                if len(obj["v"]) != want:
                    ctx.err(f"{path}: {len(obj['v'])} values, want {want}")
                return
            for k, v in obj.items():
                tables(v, f"{path}.{k}")
        elif isinstance(obj, list):
            for i, v in enumerate(obj):
                tables(v, f"{path}[{i}]")

    for name, s in systems.items():
        for i, e in enumerate(s["emitters"]):
            tables(e, f"{name}[{i}]")
            if e.get("material") and e["material"] not in materials:
                ctx.err(f"{name}[{i}]: material {e['material']} not in particles.json materials")
            if not e.get("sprite") and not e.get("mesh"):
                ctx.err(f"{name}[{i}]: a mesh emitter without a mesh")
    played = set()
    for f in (ROOT / "src").rglob("*.rs"):
        played |= set(re.findall(r'(?:Fx::at|Emitter::new)\("(\w+)"', f.read_text()))
    for name in sorted(played - systems.keys()):
        ctx.err(f"the code plays {name}, not in particles.json")
    for item, d in ctx.items["items"].items():
        fa = d.get("firearm") or {}
        for k in ("muzzle_flash", "brass"):
            if fa.get(k) and fa[k] not in systems:
                ctx.err(f"{item}: {k} {fa[k]} not in particles.json")
    if not GAME.exists():
        return
    placed = GAME / "levels" / "LongdownValley" / "particles.json"
    if not placed.exists():
        ctx.err("the level's particles.json is missing (export_assets.sh --particles)")
    else:
        emitters = json.loads(placed.read_text())
        if len(emitters) < 100 or any("system" not in e for e in emitters):
            ctx.err(f"level particles: {len(emitters)} emitters, unresolved ones (gen_particles.py)")
        for e in emitters:
            if e.get("system") and e["system"] not in systems:
                ctx.err(f"level emitter on {e['owner']}: {e['system']} not in particles.json")
    for mesh in sorted({e["mesh"] for s in systems.values() for e in s["emitters"] if e.get("mesh")}):
        if not (GAME / f"{mesh}.glb").exists():
            ctx.err(f"particle mesh {mesh} not exported (export_assets.sh --particles)")


def bosses_resolve(ctx):
    """The zombie bosses (zombies.json `boss` blocks, gen_ai.py): loot tables and DT_UniqueLoot
    sets that exist, a sane attack window, and the level's boss spawners naming boss classes; with
    assets/game, their meshes, clips and textured materials."""
    classes = json.loads((DATA / "zombies.json").read_text())["classes"]
    bosses = {n: c for n, c in classes.items() if c.get("boss")}
    if len(bosses) < 10:
        ctx.err(f"zombies.json has {len(bosses)} boss classes (want BP_ZombieBoss's 10 children)")
    for name, c in bosses.items():
        b = c["boss"]
        if c.get("loot_table") not in ctx.items.get("loot_tables", {}):
            ctx.err(f"{name}: loot table {c.get('loot_table')} unknown")
        for u in b["unique_sets"]:
            if u not in ctx.items.get("unique_loot", {}):
                ctx.err(f"{name}: unique set {u} not in DT_UniqueLoot")
        w0, w1 = b["hit_window"]
        if not (0 <= w0 < w1) or not b["attack"] or not 0 < b["rate"] <= 2 or b["reach"] <= 0:
            ctx.err(f"{name}: attack {b['attack']} window {b['hit_window']} rate {b['rate']} reach {b['reach']}")
    if not GAME.exists():
        return  # generated_assets_present warns
    actors = GAME / "levels" / "LongdownValley" / "actors.json"
    if actors.exists():
        for a in json.loads(actors.read_text()):
            cls = (a.get("props") or {}).get("SpawnedActorClass", "").strip('"')
            if a["class"] == "BP_AISpawner_ZombieBosses_C" and cls not in bosses:
                ctx.err(f"{a['name']}: spawns {cls!r}, not a boss class")
    mats = json.loads((GAME / "materials.json").read_text()) if (GAME / "materials.json").exists() else {}
    for name, c in bosses.items():
        for mesh in c["meshes"]:
            glb = GAME / "SurrounDead/Content" / f"{mesh}.glb"
            if not glb.exists():
                ctx.err(f"{name}: mesh {mesh} not exported (export_assets.sh --models)")
                continue
            data = glb.read_bytes()
            n = int.from_bytes(data[12:16], "little")
            for m in json.loads(data[20:20 + n]).get("materials", []):
                if "GlowingEyes" not in m.get("name", "") and not mats.get(m.get("name"), {}).get("texture"):
                    ctx.err(f"{name}: material {m.get('name')} has no texture in materials.json")
        for clip in [c["boss"]["attack"], *[l[0] for l in c["locomotion"]]]:
            if not (GAME / "SurrounDead/Content" / f"{clip}.anim.json").exists():
                ctx.err(f"{name}: clip {clip} not exported (export_assets.sh --anims)")


def buildables_resolve(ctx):
    """buildables.json (gen_buildables.py): every buildable item exists in items.json with a Place
    option and its actor; the beacon's waves name zombie classes and its rewards real items; with
    assets/game, every ghost and part mesh is exported."""
    path = DATA / "buildables.json"
    if not path.exists():
        ctx.err("assets/data/buildables.json missing (export_assets.sh --buildables)")
        return
    b = json.loads(path.read_text())
    items = ctx.items["items"]
    for name, i in b["items"].items():
        if name not in items:
            ctx.err(f"buildable {name} not in items.json")
        elif "Place" not in items[name]["context"]:
            ctx.warn(f"buildable {name} has no Place option")
        if i["actor"] not in b["actors"]:
            ctx.err(f"buildable {name}: actor {i['actor']} has no parts")
    zombies = json.loads((DATA / "zombies.json").read_text())["classes"]
    h = b["horde"]
    if len(h["waves"]) != 8 or not h["waves"][-1]["final"]:
        ctx.err(f"horde: {len(h['waves'])} waves (want DT_HordeSystem's 8, the last final)")
    for w in h["waves"]:
        for cls, n in w["groups"]:
            if cls not in zombies or n < 1:
                ctx.err(f"horde wave: {cls} x{n} (not a zombies.json class)")
    for r in h["rewards"]:
        if r["item"] not in items:
            ctx.err(f"horde reward {r['item']} not in items.json")
    if not GAME.exists():
        return
    meshes = {i["mesh"] for i in b["items"].values() if i["mesh"]} | {p["mesh"] for a in b["actors"].values() for p in a["parts"]}
    missing = sorted(m for m in meshes if not (GAME / f"{m}.glb").exists())
    if missing:
        ctx.err(f"{len(missing)} buildable meshes not exported (export_assets.sh --buildables): {missing[:3]}")


CHECKS = [
    buildables_resolve,
    settlements_resolve,
    random_volumes_resolve,
    humans_resolve,
    prefabs_resolve,
    particles_resolve,
    bosses_resolve,
    quests_resolve,
    vehicles_resolve,
    harvesting_resolves,
    traders_resolve,
    airdrops_resolve,
    map_and_pois_exported,
    character_clips_dont_scale,
    walk_clips_have_footsteps,
    clothing_has_body_parts,
    recipes_reference_real_items,
    weapons_and_magazines_are_consistent,
    equip_sockets_exist,
    loot_tables_resolve,
    difficulty_defaults_in_range,
    generated_assets_present,
    zombie_spawners_carry_props,
    difficulty_presets_resolve,
    firearms_have_component_data,
]


def main():
    ctx = Ctx()
    for check in CHECKS:
        try:
            check(ctx)
        except Exception as e:  # a broken check is a failure, not a crash
            ctx.err(f"{check.__name__} crashed: {e!r}")
    for w in ctx.warnings:
        print(f"  warn: {w}")
    for e in ctx.errors:
        print(f"  ERROR: {e}")
    print(f"data checks: {len(CHECKS)} run, {len(ctx.errors)} errors, {len(ctx.warnings)} warnings")
    sys.exit(1 if ctx.errors else 0)


if __name__ == "__main__":
    main()
