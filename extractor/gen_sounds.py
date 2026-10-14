#!/usr/bin/env python3
"""Build assets/data/sounds.json and the decoded waves under assets/game/ from the game's SoundCues.

usage: gen_sounds.py          (run by `export_assets.sh --sounds`; needs vgmstream-cli on PATH)

The cues are the ones the ported Blueprints play (CUES below), plus every firearm's FirearmSound
/ FirearmSuppressedSound / FirearmChamberSound from items.json and the zombies' sounds from
zombies.json. Each cue's node graph is flattened into variants:

    {"cues": {"M-249_01_Cue": {"class": "EffectsAudio", "attenuation": "GunshotsRifle",
       "volume": 0.75, "pitch": 1.0, "variants": [{"waves": ["game/.../M-249_01.wav"], "weight": 1,
       "volume": [0.95, 1.05], "pitch": [0.95, 1.05], "loop": false}]}},
     "attenuation": {"GunshotsRifle": {"inner": 20.0, "falloff": 250.0, "model": "Logarithmic",
       "spatialize": true}}}

Distances are in metres. Waves are Bink Audio in the paks: SdExport audio writes the .binka and
vgmstream-cli decodes it to 16-bit WAV.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
GAME = ROOT / "assets" / "game"
DATA = ROOT / "assets" / "data"
SDX = str(HERE / "bin" / "out" / "SdExport")

# Sounds played by the ported Blueprints (BP_FirearmPickup, BP_MeleePickup, BP_PlayerCharacter,
# the throwables, BP_LootContainer, BP_MasterZombie, the flashlight, the lever...).
CUES = [
    "S_EmptyFire_Cue", "GunJam_Cue", "whoosh_weapon_knife_swing_01_Cue", "Weapon_Hand_Move_02_Cue",
    "Body_Flesh_1_Cue", "Body_Flesh_2_Cue", "Body_Flesh_3_Cue", "Body_Flesh_4_Cue", "Body_Flesh_5_Cue",
    "Head_shot_1_Cue", "Head_shot_2_Cue", "Head_shot_3_Cue", "Head_shot_4_Cue", "Head_shot_5_Cue",
    "explosion_large_06_Cue", "Whistle_Cue", "Turning_Flashlight_On_Cue", "Turning_Flashlight_Off_3_Cue",
    "foley_jump_movement_throw_02_Cue", "Searching_Cue", "BreakOpen_Cue", "door_lock_fail_03_Cue",
    *[f"bullet_impact_body_thump_0{i}_Cue" for i in range(1, 9)],
    # Player_AnimBP's footsteps by physical surface (walk/crouch and sprint sets).
    *[f"{m}{g}_Cue" for m in ("Concrete", "Dirt", "Grass", "Rock", "Metal", "Water", "Wood", "Sand", "Snow") for g in ("", "Sprint")],
    # Doors (the door BPs' PlaySoundAtLocation), the locked door's crowbar, fires, generators.
    "door_lock_open_02_Cue", "door_lock_close_01_Cue", "metal_gate_automatic_door_squeak_05_Cue", "MetalDoorImpact_Cue",
    "WoodBreak_cue", "SC_CampFire", "engine_generator_loop_01_Cue", "LightHum_Cue",
    "ZombieBreathLoopCue", "ZombieGrowlShort", "ZombieDeathShort", "HoundCue", "HoundAttackCue", "UI_click_sound_Cue",
]

# The Audio settings' sliders for each SoundClass (MuffleAudio parents Ambient and Effects).
CLASS_SETTINGS = {"AmbientAudio": "AMBIENT", "EffectsAudio": "EFFECTS", "MusicAudio": "MUSIC", "UIAudio": "UI", "RadioMusicAudio": "RADIO"}


def listing():
    out = subprocess.run([SDX, "list", "SurrounDead/Content/"], capture_output=True, text=True).stdout.split()
    stems = {}
    for f in out:
        if f.endswith(".uasset"):
            pkg = f[: -len(".uasset")]
            stems.setdefault(pkg.rsplit("/", 1)[1], pkg)
    return stems


def dump(tmp, pkgs):
    pkgs = [p for p in pkgs if not (Path(tmp) / (p.removeprefix("SurrounDead/Content/") + ".json")).exists()]
    for i in range(0, len(pkgs), 200):
        subprocess.run([SDX, "json", tmp, *pkgs[i : i + 200]], capture_output=True)


def load(tmp, pkg):
    f = Path(tmp) / (pkg.removeprefix("SurrounDead/Content/") + ".json")
    return json.loads(f.read_text()) if f.exists() else []


def name_of(ref):
    return ref["ObjectName"].split("'")[1].split(":")[-1] if ref else None


def wave_pkg(asset_path):
    # "/Game/Audio/X/Y.Y" -> "SurrounDead/Content/Audio/X/Y"
    return "SurrounDead/Content/" + asset_path.split(".")[0].removeprefix("/Game/")


def flatten(nodes, name, scale_vol=(1.0, 1.0), scale_pitch=(1.0, 1.0)):
    """A node's variants: [{"waves": [...pkgs], "weight", "volume", "pitch", "loop"}]."""
    n = nodes.get(name)
    if n is None:
        return []
    t, p = n["Type"], n.get("Properties") or {}
    kids = [name_of(c) for c in p.get("ChildNodes") or [] if c]
    if t == "SoundNodeWavePlayer":
        ptr = (p.get("SoundWaveAssetPtr") or {}).get("AssetPathName")
        if not ptr:
            return []
        return [{"waves": [wave_pkg(ptr)], "weight": 1.0, "volume": list(scale_vol), "pitch": list(scale_pitch), "loop": bool(p.get("bLooping"))}]
    if t == "SoundNodeModulator":
        # UE defaults: 0.95-1.05 for both.
        v = (scale_vol[0] * p.get("VolumeMin", 0.95), scale_vol[1] * p.get("VolumeMax", 1.05))
        q = (scale_pitch[0] * p.get("PitchMin", 0.95), scale_pitch[1] * p.get("PitchMax", 1.05))
        return [x for k in kids for x in flatten(nodes, k, v, q)]
    if t == "SoundNodeRandom":
        weights = p.get("Weights") or []
        out = []
        for i, k in enumerate(kids):
            w = weights[i] if i < len(weights) else 1.0
            for x in flatten(nodes, k, scale_vol, scale_pitch):
                x["weight"] *= w
                out.append(x)
        return out
    if t == "SoundNodeMixer":
        # All inputs at once: one variant per combination is overkill; mix the first variant of each.
        parts = [flatten(nodes, k, scale_vol, scale_pitch) for k in kids]
        parts = [x[0] for x in parts if x]
        if not parts:
            return []
        return [{"waves": [w for x in parts for w in x["waves"]], "weight": 1.0, "volume": list(scale_vol), "pitch": list(scale_pitch), "loop": any(x["loop"] for x in parts)}]
    if t == "SoundNodeLooping":
        out = [x for k in kids for x in flatten(nodes, k, scale_vol, scale_pitch)]
        for x in out:
            x["loop"] = True
        return out
    # Attenuation, delay, concatenator, switch...: their children.
    return [x for k in kids for x in flatten(nodes, k, scale_vol, scale_pitch)]


def attenuation(tmp, stems):
    out = {}
    pkgs = [p for s, p in stems.items() if "/Audio/Attenuation/" in p]
    dump(tmp, pkgs)
    for pkg in pkgs:
        for e in load(tmp, pkg):
            if e["Type"] != "SoundAttenuation":
                continue
            a = (e.get("Properties") or {}).get("Attenuation") or {}
            out[e["Name"]] = {
                # AttenuationShapeExtents.X: the sphere's full-volume radius; then FalloffDistance.
                "inner": (a.get("AttenuationShapeExtents") or {}).get("X", 400.0) / 100.0,
                "falloff": a.get("FalloffDistance", 3600.0) / 100.0,
                "model": (a.get("DistanceAlgorithm") or "EAttenuationDistanceModel::Linear").split("::")[-1],
                "spatialize": a.get("bSpatialize", True),
            }
    return out


def terrain_surfaces(tmp, stems):
    """Each landscape paint layer's surface: its LayerInfo's PhysMaterial (PhysMat_Grass -> Grass)."""
    pkgs = [p for p in stems.values() if "/Terrain/Layers/" in p and p.endswith("_LayerInfo")]
    dump(tmp, pkgs)
    out = {}
    for pkg in pkgs:
        for e in load(tmp, pkg):
            if e["Type"] == "LandscapeLayerInfoObject":
                pm = name_of((e.get("Properties") or {}).get("PhysMaterial"))
                if pm:
                    out[e["Name"]] = pm.removeprefix("PhysMat_")
    return out


def decode(tmp, pkgs):
    """SdExport audio -> .binka (or .ogg/.wav) -> assets/game/<path>.wav. Returns the waves made."""
    todo = [p for p in pkgs if not (GAME / (p + ".wav")).exists()]
    raw = Path(tmp) / "raw"
    for i in range(0, len(todo), 200):
        subprocess.run([SDX, "audio", str(raw), *todo[i : i + 200]], capture_output=True)
    made = 0
    for p in todo:
        rel = p.removeprefix("SurrounDead/Content/")
        src = next(iter(raw.glob(rel + ".*")), None) if (raw / rel).parent.exists() else None
        if src is None:
            print(f"  no audio for {p}", file=sys.stderr)
            continue
        dst = GAME / (p + ".wav")
        dst.parent.mkdir(parents=True, exist_ok=True)
        if src.suffix == ".wav":
            shutil.copy(src, dst)
        else:
            r = subprocess.run(["vgmstream-cli", "-o", str(dst), str(src)], capture_output=True, text=True)
            if r.returncode != 0:
                print(f"  vgmstream failed on {src}: {r.stderr.strip()[:200]}", file=sys.stderr)
                continue
        made += 1
    return made


def main():
    if not shutil.which("vgmstream-cli"):
        sys.exit("vgmstream-cli not found (needed for Bink Audio)")
    stems = listing()
    if not stems:
        sys.exit("SdExport listed nothing: run through export_assets.sh --sounds (it sets SD_USMAP etc.)")
    wanted = list(CUES)
    items = json.loads((DATA / "items.json").read_text())["items"]
    for it in items.values():
        fa = it.get("firearm") or {}
        wanted += [fa.get(k) for k in ("sound", "suppressed_sound", "chamber_sound") if fa.get(k)]
    wanted = sorted(set(wanted))
    tmp = tempfile.mkdtemp()
    dump(tmp, [stems[w] for w in wanted if w in stems])
    cues, waves = {}, set()
    for w in wanted:
        pkg = stems.get(w)
        if not pkg:
            print(f"  unknown sound {w}", file=sys.stderr)
            continue
        exports = load(tmp, pkg)
        nodes = {e["Name"]: e for e in exports}
        cue = next((e for e in exports if e["Type"] == "SoundCue"), None)
        if cue:
            p = cue.get("Properties") or {}
            variants = flatten(nodes, name_of(p.get("FirstNode")))
            cues[w] = {
                "class": name_of(p.get("SoundClassObject")),
                "attenuation": name_of(p.get("AttenuationSettings")),
                # USoundCue::VolumeMultiplier defaults to 0.75.
                "volume": p.get("VolumeMultiplier", 0.75),
                "pitch": p.get("PitchMultiplier", 1.0),
                "variants": variants,
            }
        else:
            wave = next((e for e in exports if e["Type"] == "SoundWave"), None)
            if not wave:
                continue
            p = wave.get("Properties") or {}
            cues[w] = {"class": name_of(p.get("SoundClassObject")), "attenuation": name_of(p.get("AttenuationSettings")), "volume": p.get("Volume", 1.0), "pitch": p.get("Pitch", 1.0),
                       "variants": [{"waves": [pkg], "weight": 1.0, "volume": [1.0, 1.0], "pitch": [1.0, 1.0], "loop": bool(p.get("bLooping"))}]}
        for v in cues[w]["variants"]:
            waves.update(v["waves"])
    made = decode(tmp, sorted(waves))
    # The runtime loads waves by asset path.
    for c in cues.values():
        for v in c["variants"]:
            v["waves"] = ["game/" + x + ".wav" for x in v["waves"] if (GAME / (x + ".wav")).exists()]
        c["variants"] = [v for v in c["variants"] if v["waves"]]
    out = {"cues": cues, "attenuation": attenuation(tmp, stems), "classes": CLASS_SETTINGS, "terrain_surfaces": terrain_surfaces(tmp, stems)}
    (DATA / "sounds.json").write_text(json.dumps(out, indent=1, sort_keys=True))
    print(f"sounds: {len(cues)} cues, {len(waves)} waves ({made} decoded now)")
    shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
