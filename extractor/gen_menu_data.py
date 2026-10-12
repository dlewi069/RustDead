#!/usr/bin/env python3
"""Build assets/data/menus.json from `SdExport json` dumps of the game's menu widgets.

usage: gen_menu_data.py <json dump root>   (normally run via `export_assets.sh --menu-data`)

The output drives the Rust settings / difficulty pages: section headers, row labels, control
kind, slider ranges and combo options, all in the game's own order. Radio-select rows don't
serialize their options (the BP instance has no tagged properties), so those come from the
page's string table and are flagged "inferred".
"""
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "assets" / "data" / "menus.json"

QUALITY = ["LOW", "MEDIUM", "HIGH", "ULTRA", "CINEMATIC"]
RADIO_OPTIONS = {
    "WINDOW MODE": ["WINDOWED", "BORDERLESS", "FULLSCREEN"],
    "ANTI-ALIASING TYPE": ["DISABLED", "FXAA", "MSAA", "TAA", "TSR"],
    "UPSCALING METHOD": ["Off", "NVIDIA DLSS", "AMD FSR"],
    "UPSCALING QUALITY": ["Quality", "Balanced", "Performance", "Ultra Performance"],
    # Not present in the string table; the port's best guess.
    "MSAA SAMPLE COUNT": ["2X", "4X", "8X"],
}

# IMC_General action -> settings row label (InputSettingsPage order uses these labels).
ACTION_LABELS = {
    "IA_MoveForward": "MOVE FORWARD", "IA_MoveBackwards": "MOVE BACKWARDS", "IA_MoveLeft": "MOVE LEFT",
    "IA_MoveRight": "MOVE RIGHT", "IA_Jump": "JUMP / VAULT", "IA_Sprint": "SPRINT", "IA_Crouch": "CROUCH",
    "IA_Roll": "ROLL", "IA_AutoRun": "AUTO WALK/SPRINT", "IA_FloatUpwards": "FLOAT UPWARDS WHILE SWIMMING UNDERWATER",
    "IA_PrimaryAction": "PRIMARY ACTION", "IA_Aim": "AIM", "IA_ScopeZoom": "SCOPE ZOOM",
    "IA_HoldBreath": "HOLD BREATH WHILE SCOPED", "IA_Reload": "RELOAD", "IA_Shove": "SHOVE / STOMP",
    "IA_Throwable": "THROWABLE", "IA_Distract": "DISTRACT / WHISTLE", "IA_SwitchCameraLeft": "ADS SWITCH CAMERA [LEFT]",
    "IA_SwitchCameraRight": "ADS SWITCH CAMERA [RIGHT]", "IA_Interact": "INTERACT", "IA_Inventory": "INVENTORY VISIBILITY",
    "IA_QuickSearch": "QUICK SEARCH", "IA_EquipPrimary": "EQUIP PRIMARY", "IA_EquipSecondary": "EQUIP SECONDARY",
    "IA_EquipSidearm": "EQUIP SIDEARM", "IA_EquipMelee": "EQUIP MELEE", "IA_Hotbar6": "USE HOTBAR [6]",
    "IA_Hotbar7": "USE HOTBAR [7]", "IA_Hotbar8": "USE HOTBAR [8]", "IA_FishingRod": "USE HOTBAR [FISHING ROD]",
    "IA_Unequip": "UNEQUIP", "IA_LootAll": "LOOT ALL ITEMS IN CONTAINER", "IA_ChangeFireMode": "CHANGE FIRING MODE",
    "IA_ToggleTactical": "TOGGLE TACTICAL WEAPON ATTACHMENT", "IA_Journal": "JOURNAL VISIBILITY", "IA_Map": "MAP VISIBILITY",
    "IA_ZoomCamera": "ZOOM PLAYER CAMERA", "IA_Flashlight": "FLASHLIGHT / VEHICLE LIGHTS",
    "IA_NightVision": "TURN ON/OFF NIGHT VISION", "IA_Binoculars": "USE BINOCULARS", "IA_SitDown": "SIT DOWN / REST",
    "IA_FreeLook": "FREE LOOK", "IA_RadialMenu": "RADIAL MENU", "IA_VehicleForward": "VEHICLE FORWARD",
    "IA_VehicleBackwards": "VEHICLE BACKWARDS", "IA_VehicleLeft": "VEHICLE LEFT", "IA_VehicleRight": "VEHICLE RIGHT",
    "IA_VehicleHandbrake": "VEHICLE HANDBRAKE", "IA_VehicleHorn": "VEHICLE HORN", "IA_VehicleFlip": "VEHICLE FLIP",
    "IA_VehicleRadio": "VEHICLE RADIO", "IA_PauseMenu": "PAUSE MENU", "IA_PhotoMode": "PHOTO MODE",
    "IA_Screenshot": "SCREENSHOT", "IA_ChangePerspective": "CHANGE CAMERA PERSPECTIVE",
    "IA_ResetCameraPosition": "RESET CAMERA POSITION", "IA_HideUI": "HIDE UI", "IA_QuickSave": "QUICK SAVE",
    "IA_DebugMenu": "DEBUG MENU",
}


def load(rel):
    return json.load(open(ROOT / rel))


def ordered_tree(rel):
    out = subprocess.run(["python3", str(HERE / "widget_tree.py"), str(ROOT / rel)], capture_output=True, text=True, check=True).stdout
    for line in out.splitlines():
        m = re.match(r"\s*(\S+) (\S+)\s+(.*)", line)
        if m:
            yield m.groups()


def text_of(t):
    return (t or {}).get("SourceString") or (t or {}).get("CultureInvariantString")


def key_bindings():
    keys = {}
    for e in load("Input/IMC_General.json"):
        props = e.get("Properties", {})
        for m in props.get("DefaultKeyMappings", {}).get("Mappings") or props.get("Mappings", []):
            action = m["Action"]["ObjectName"].split("'")[1]
            label = ACTION_LABELS.get(action)
            if label and m["Key"]["KeyName"] != "None":
                keys.setdefault(label, []).append(m["Key"]["KeyName"])
    return keys


def page(rel, keys):
    by = {e["Name"]: e for e in load(rel)}
    rows = []
    for typ, name, rest in ordered_tree(rel):
        if typ == "TextBlock" and "size=28" in rest:
            rows.append({"section": rest.split(" size")[0].strip("'")})
            continue
        if not typ.startswith(("SettingRow", "EnhancedInputRow")):
            continue
        p = by[name]["Properties"]
        label = text_of(p.get("Label"))
        if not label:
            continue
        row = {"label": label.strip()}
        content = None
        for b in p.get("NamedSlotBindings", []):
            content = by.get(b["Content"]["ObjectName"].split(".")[-1].rstrip("'"))
            break
        ctype = content["Type"] if content else ""
        cp = content.get("Properties", {}) if content else {}
        if typ.startswith("EnhancedInputRow") or ctype == "EnhancedInputRow_C":
            row["kind"] = "key"
            row["keys"] = keys.get(row["label"], [])
            # Mouse/modifier combos (SPLIT ITEM STACK etc.) are plain text, not remappable actions.
            for b in cp.get("NamedSlotBindings", []):
                inner = by.get(b["Content"]["ObjectName"].split(".")[-1].rstrip("'"), {})
                if inner.get("Type") == "TextBlock":
                    row["keys"] = [text_of(inner["Properties"].get("Text"))]
        elif ctype == "DefaultCheckBoxSetting_C":
            row["kind"] = "check"
        elif ctype == "StyledSliderSetting_C":
            row.update(kind="slider", min=cp.get("LeftValue", 0.0), max=cp.get("RightValue", 1.0),
                       digits=cp.get("LabelFractionalDigits", 0), multiplier=cp.get("LabelMultiplier", 1.0),
                       cvar=cp.get("CVarName"))
        elif ctype == "StyledComboBoxSetting_C" and "Options" in cp:
            row.update(kind="choice", options=[text_of(o["Label"]) for o in cp["Options"]],
                       values=[o.get("Value") for o in cp["Options"]], cvar=cp.get("CVarName"))
        elif ctype == "StyledComboBoxSetting_C":
            row.update(kind="choice", options=["1280x720", "1600x900", "1920x1080", "2560x1440", "3840x2160"],
                       cvar=cp.get("CVarName"), inferred=True)
        elif ctype == "DefaultRadioSelectSetting_C":
            row.update(kind="choice", options=RADIO_OPTIONS.get(row["label"], QUALITY), inferred=True)
        elif ctype == "ButtonWidget_C":
            row.update(kind="button", text=text_of(cp.get("Text")))
        else:
            row.update(kind="button", text=row["label"])
        rows.append(row)
    return rows


CREATOR_ROWS = [  # (label in the creator panel, ComboBoxString name), in panel order
    ("Sex:", "DropDownSex"), ("Skin Color:", "DropDownSkinColor_2"), ("Hair Type:", "DropDownHairType"),
    ("Hair Color:", "DropDownHairColor"), ("Mouth:", "DropDownMouth"), ("Beard:", "DropDownBeard"),
    ("Beard Color:", "DropDownBeardColor"), ("Eyebrows:", "DropDownEyebrows"), ("Accessory 1:", "DropDownAccessory1"),
    ("Accessory 2:", "DropDownAccessory2"), ("Accessory 3:", "DropDownAccessory3"),
]


def creator():
    by = {e["Name"]: e for e in load("UI/Menus/Menu/CharacterCreatorMenu.json")}
    occupations = []
    for e in load("PlayerModel/Other/Enum_Occupation.json"):
        occupations = [text_of(kv["Value"]) for kv in e.get("Properties", {}).get("DisplayNameMap", [])]
    body = []
    for label, name in CREATOR_ROWS:
        p = by[name]["Properties"]
        body.append({"label": label, "options": p["DefaultOptions"], "default": p.get("SelectedOption", p["DefaultOptions"][0])})
    return {"default_forename": "John", "default_surname": "Doe", "occupations": occupations, "body": body}


# GameFunctionLibrary.SetDifficulty: the DT_Difficulty row field each CVar is saved from (the
# 12 loot rarity fields aren't applied there). Check rows carry no CVar and match by label.
PRESET_FIELDS = {
    "XPMultiplier": "Difficulty.XPMultiplier",
    "ZombieSpawnAmountMultiplier": "Difficulty.ZombieAmount",
    "ZombieBossSpawnAmountMultiplier": "Difficulty.ZombieBossAmount",
    "ZombieHoundSpawnAmountMultiplier": "Difficulty.ZombieHoundAmount",
    "ZombieHealthMultiplier": "Difficulty.ZombieHealth",
    "ZombieSpeedMultiplier": "Difficulty.ZombieSpeed",
    "ZombieDamageMultiplier": "Difficulty.ZombieDamage",
    "BanditHealthMultiplier": "Difficulty.BanditHealth",
    "MilitaryHealthMultiplier": "Difficulty.MilitaryHealth",
    "ScavengerWanderingTraderHealthMultiplier": "Difficulty.ScavengerHealth",
    "AnimalSpawnAmountMultiplier": "Difficulty.AnimalAmount",
    "RandomEventSpawnMultiplier": "Difficulty.RandomEventMultiplier",
    "BanditCampSpawnMultiplier": "Difficulty.BanditsCampMultiplier",
    "MilitaryCampSpawnMultiplier": "Difficulty.MilitaryCampMultiplier",
    "InfestationSpawnMultiplier": "Difficulty.RandomInfestationsMultiplier",
    "InfestationHealthMultiplier": "Difficulty.InfestationsHealth",
    "KeepInventoryOnDeath?": "KEEP INVENTORY ON DEATH?",
    "ResetPassiveSkillsOnDeath?": "RESET PASSIVE SKILLS ON DEATH?",
    "HungerDepletionMultiplier": "Difficulty.HungerMultiplier",
    "ThirstDepletionMultiplier": "Difficulty.ThirstMultiplier",
    "FuelDepletionMultiplier": "Difficulty.FuelMultiplier",
    "DamageDepletionMultiplier": "Difficulty.DamageMultiplier",
}


def difficulty_presets(data):
    """DT_Difficulty rows (Peaceful..Nightmare) as the difficulty rows they set."""
    dt = ROOT / "Blueprints/Other/DT_Difficulty.json"
    if not dt.exists():
        return []
    rows = json.loads(dt.read_text())[0]["Rows"]
    where = {}
    for p in data["difficulty"]:
        for r in p["rows"]:
            for key in (r.get("cvar"), r.get("label")):
                if key:
                    where[key] = (p["name"], r["label"], r.get("kind"))
    presets, missing = [], set()
    for name, row in rows.items():
        values = []
        for field, v in row.items():
            target = PRESET_FIELDS.get(field.split("_")[0])
            if target is None:
                continue
            if target not in where:
                missing.add(target)
                continue
            page, label, kind = where[target]
            values.append({"page": page, "label": label, "value": float(v) if not isinstance(v, bool) else (1.0 if v else 0.0)})
        presets.append({"name": name, "values": values})
    print(f"difficulty presets: {[p['name'] for p in presets]}, unmatched: {sorted(missing)}")
    return presets


def main():
    keys = key_bindings()
    m = "UI/Menus/Pages/"
    data = {
        "settings": [
            {"name": "Video", "rows": page(m + "VideoSettingsPage.json", keys)},
            {"name": "Audio", "rows": page(m + "AudioSettingsPage.json", keys)},
            {"name": "Controls", "rows": page(m + "InputSettingsPage.json", keys)},
            {"name": "Gameplay", "rows": page(m + "GameplaySettingsPage.json", keys)},
        ],
        "difficulty": [
            {"name": "General", "rows": page(m + "Difficulties/Difficulty_General.json", keys)},
            {"name": "AI", "rows": page(m + "Difficulties/Difficulty_AI.json", keys)},
            {"name": "Loot / Environment", "rows": page(m + "Difficulties/Difficulty_Loot.json", keys)},
            {"name": "Player", "rows": page(m + "Difficulties/Difficulty_Player.json", keys)},
            {"name": "Vehicles", "rows": page(m + "Difficulties/Difficulty_Vehicles.json", keys)},
        ],
        "creator": creator(),
        "new_game_rows": [r for r in page(m + "NewGameDifficultySelectionPage.json", keys) if "label" in r],
    }
    # Save_Difficulties' class defaults are the game's starting values for the difficulty
    # sliders (DayLength 30, NightLength 15, AiRespawnTimer 1500...): match them by CVar name.
    sd = ROOT / "Blueprints/Saves/Save_Difficulties.json"
    if sd.exists():
        norm = lambda n: "".join(c for c in n.lower() if c.isalnum())
        cdo = next(e for e in json.loads(sd.read_text()) if e["Name"].startswith("Default__"))
        defaults = {norm(k): v for k, v in cdo.get("Properties", {}).items() if isinstance(v, (int, float))}
        matched = 0
        for p in data["difficulty"]:
            for r in p["rows"]:
                key = norm((r.get("cvar") or "").split(".")[-1])
                if r.get("kind") == "slider" and key in defaults:
                    r["default"] = float(defaults[key])
                    matched += 1
                elif r.get("kind") == "check":
                    # Check rows carry no CVar: match the save's booleans by label ("Airdrop" ->
                    # AIRDROPS ENABLED?). Booleans left at false aren't serialized.
                    label = norm(r.get("label", ""))
                    on = any(v is True and len(k) > 4 and k in label for k, v in defaults.items())
                    r["default"] = 1.0 if on else 0.0
        print(f"difficulty defaults from Save_Difficulties: {matched}")
    data["difficulty_presets"] = difficulty_presets(data)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(data, indent=1))
    n = sum(len(p["rows"]) for p in data["settings"] + data["difficulty"])
    print(f"wrote {OUT} ({n} rows)")


main()
