import os
import subprocess
import sys

AUDIO_MAPPINGS = {
    "gun_pistol.wav": "assets/SurrounDead/Content/Audio/Premium_Weapons_Bullets_SFX/WAV/Pistol/Pistol/Pistol_01",
    "gun_rifle.wav": "assets/SurrounDead/Content/Audio/Real_Recorded_Guns/Wavs/Guns/AK_47/AKM_Single_Shot2",
    "gun_rifle_suppressed.wav": "assets/SurrounDead/Content/Audio/Real_Recorded_Guns/Wavs/Guns/AK_47/AKM_suppressed_fire_102_with_Reverb_2",
    "gun_shotgun.wav": "assets/SurrounDead/Content/Audio/WARFARE_SOUNDS/Weapons/Guns/Wavs/gun_shotgun_sawed_off_shot_01",
    "gun_sniper.wav": "assets/SurrounDead/Content/Audio/Premium_Weapons_Bullets_SFX/WAV/Rifle/Sniper_rifle/Barret-M28/Barret-M28_07",
    "hit_headshot.wav": "assets/SurrounDead/Content/Audio/Real_Recorded_Guns/Wavs/Bullet_Impacts_Flybys/Head_shot/Head_shot_1",
    "hit_flesh.wav": "assets/SurrounDead/Content/Audio/Real_Recorded_Guns/Wavs/Bullet_Impacts_Flybys/Body_Flesh_Impact/Body_Flesh_1",
    "zombie_growl.wav": "assets/SurrounDead/Content/Audio/Zombie/Zombie_Sounds_-_Growl_02",
    "zombie_hit.wav": "assets/SurrounDead/Content/Audio/Zombie/Zombie_Sounds_-_Hit_02",
    "player_bandage.wav": "assets/SurrounDead/Content/Audio/Survival_Sound_Kit_HD_Remake/Wavs/Applying_Bandage_to_wound_2",
    "flashlight_toggle.wav": "assets/SurrounDead/Content/Audio/Survival_Sound_Kit_HD_Remake/Wavs/Turning_Flashlight_On",
    "footstep_grass.wav": "assets/SurrounDead/Content/Audio/WARFARE_SOUNDS/Footsteps/Wavs/footstep_grass_walk_04",
    "footstep_concrete.wav": "assets/SurrounDead/Content/Audio/WARFARE_SOUNDS/Footsteps/Wavs/footstep_concrete_walk_02",
    "vehicle_engine.wav": "assets/SurrounDead/Content/Audio/VehicleSounds/Sedan/Sedan_Engine_IdleRPM",
    # Additional authentic audio
    "theme_menu.wav": "assets/SurrounDead/Content/Audio/Music/Menu/SurronDeadThemeWAV",
    "music_safezone.wav": "assets/SurrounDead/Content/Audio/Music/Ingame/SafeZoneWAV",
    "music_combat.wav": "assets/SurrounDead/Content/Audio/Music/Combat/FightWAV",
    "melee_swing.wav": "assets/SurrounDead/Content/Audio/WARFARE_SOUNDS/Weapons/Knife/Wavs/whoosh_weapon_knife_swing_01",
    "gun_reload.wav": "assets/SurrounDead/Content/Audio/WARFARE_SOUNDS/Weapons/Guns/Wavs/gun_shotgun_load_bullet_01",
    "gun_empty.wav": "assets/SurrounDead/Content/Audio/WARFARE_SOUNDS/Weapons/Guns/Wavs/gun_chamber_jammed_01",
    "gun_cock.wav": "assets/SurrounDead/Content/Audio/WARFARE_SOUNDS/Weapons/Guns/Wavs/gun_shotgun_cock_01",
    "player_hurt.wav": "assets/SurrounDead/Content/Audio/Survival_Sound_Kit_HD_Remake/Wavs/Male_taking_damage",
    "zombie_death.wav": "assets/SurrounDead/Content/Audio/Zombie/Zombie_Sounds_-_Death_01",
    "zombie_attack.wav": "assets/SurrounDead/Content/Audio/Zombie_Sounds_Pro/Wavs/Zombie_Attack_01",
    "zombie_boss.wav": "assets/SurrounDead/Content/Audio/Zombie_Sounds_Pro/Wavs/Zombie_Large_01",
    "chop_wood.wav": "assets/SurrounDead/Content/Audio/Survival_Sound_Kit_HD_Remake/Wavs/Chopping_wood",
    "mine_rock.wav": "assets/SurrounDead/Content/Audio/Survival_Sound_Kit_HD_Remake/Wavs/Mining__Hitting_Stone_With_Pickaxe__2",
    "car_horn.wav": "assets/SurrounDead/Content/Audio/VehicleSounds/Sounds/Wav/Car/A_Car_Horn",
    "car_crash.wav": "assets/SurrounDead/Content/Audio/VehicleSounds/_Collisions/VehicleCollision",
    "car_drive.wav": "assets/SurrounDead/Content/Audio/VehicleSounds/Sedan/Sedan_Engine_MedRPM",
    "loot_open.wav": "assets/SurrounDead/Content/Audio/SFXInteraction/Wav/OpenMedicalBox",
    "loot_search.wav": "assets/SurrounDead/Content/Audio/SFXInteraction/Wav/Searching",
    "explosion.wav": "assets/SurrounDead/Content/Audio/WARFARE_SOUNDS/Explosion_Fire_Gas/Wavs/explosion_large_06",
}

BINKADEC_PATH = "/tmp/binkadec/binkadec.exe"
WINE_CMD = [
    "/ntfs/SteamLibrary/steamapps/common/Proton 9.0 (Beta)/files/bin/wine64"
]
ENV = os.environ.copy()
ENV["WINEPREFIX"] = "/ntfs/SteamLibrary/steamapps/compatdata/1645820/pfx"

out_dir = "assets/audio"
os.makedirs(out_dir, exist_ok=True)
os.makedirs("/tmp/binka_tmp", exist_ok=True)

for target_name, base_path in AUDIO_MAPPINGS.items():
    uexp_path = base_path + ".uexp"
    ubulk_path = base_path + ".ubulk"
    out_wav = os.path.join(out_dir, target_name)
    
    if os.path.exists(out_wav) and os.path.getsize(out_wav) > 1000:
        continue # Already converted

    if not os.path.exists(uexp_path) or not os.path.exists(ubulk_path):
        print(f"Skipping {target_name}: source files not found ({uexp_path})")
        continue

    # 1. Read .uexp to locate 28-byte Bink Audio File Header (starts with b"ABEU")
    with open(uexp_path, "rb") as f:
        uexp_data = f.read()

    idx = uexp_data.find(b"ABEU")
    if idx == -1:
        print(f"Skipping {target_name}: 'ABEU' magic not found in {uexp_path}")
        continue

    header = uexp_data[idx:idx+28]

    # 2. Read full .ubulk audio payload
    with open(ubulk_path, "rb") as f:
        ubulk_data = f.read()

    tmp_binka = f"/tmp/binka_tmp/{target_name}.binka"
    tmp_wav = f"/tmp/binka_tmp/{target_name}.wav"

    with open(tmp_binka, "wb") as f:
        f.write(header + ubulk_data)

    # Convert via binkadec through wine
    wine_in = "Z:" + tmp_binka.replace("/", "\\")
    wine_out = "Z:" + tmp_wav.replace("/", "\\")

    cmd = WINE_CMD + [BINKADEC_PATH, "-i", wine_in, "-o", wine_out]
    res = subprocess.run(cmd, env=ENV, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    
    if os.path.exists(tmp_wav):
        import shutil
        shutil.move(tmp_wav, out_wav)
        size_kb = os.path.getsize(out_wav) / 1024.0
        print(f"✓ Converted {target_name} ({size_kb:.1f} KB)")
    else:
        print(f"✗ Failed {target_name}: {res.stdout.decode('utf-8', errors='ignore')[:200]}")
