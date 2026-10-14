#!/usr/bin/env python3
"""Consistency checks on the committed game data (assets/data/*.json) and, when present, the
generated assets they point at. Run by scripts/check.sh; exits non-zero on errors.

Add a check: write a function taking `ctx` that calls ctx.err(...) / ctx.warn(...), and list it
in CHECKS.
"""
import json
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


CHECKS = [
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
