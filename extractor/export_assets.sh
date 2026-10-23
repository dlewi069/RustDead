#!/usr/bin/env bash
# Export the game assets the Rust port loads at runtime into assets/game/ (gitignored).
# Requires a local SurrounDead install; override SD_GAME_DIR if it isn't at the default path.
#
#   export_assets.sh              UI textures only (menus)
#   export_assets.sh --models     + character/zombie/vehicle/prop meshes and materials.json
#   export_assets.sh --level      + LongdownValley: terrain, placements, splines, actors, its meshes
#   export_assets.sh --anims      + character/zombie animations
#   export_assets.sh --menu-data  regenerate assets/data/menus.json (committed)
#   export_assets.sh --ai         regenerate assets/data/zombies.json (committed): zombie classes
#   export_assets.sh --items      regenerate assets/data/items.json + traders.json (committed): items, loot tables, containers, traders
#   export_assets.sh --vehicles   regenerate assets/data/vehicles.json (committed): vehicle classes, spawner rows; vehicle meshes
#   export_assets.sh --quests     regenerate assets/data/quests.json (committed): quest graphs, the quest crate; its meshes
#   export_assets.sh --particles  regenerate assets/data/particles.json (committed): Cascade systems; the level's emitters, the particle meshes
#   export_assets.sh --prefabs    regenerate assets/data/prefabs.json (committed): camps, convoys, crashes...; their meshes (after --level)
#   export_assets.sh --buildables regenerate assets/data/buildables.json (committed): buildable items/actors, the horde beacon; their meshes
#   export_assets.sh --sounds     assets/data/sounds.json + decoded waves (needs vgmstream-cli)
#   export_assets.sh --all        everything above
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$HERE")"
OUT="$ROOT/assets/game"
LEVEL="SurrounDead/Content/Levels/LongdownValley"
LEVEL_OUT="$OUT/levels/LongdownValley"

# A flag was given, or --all (not "$*": that's has()'s own argument, so `has --all` was always true).
has() { [[ " ${FLAGS} " == *" --all "* ]] || [[ " ${FLAGS} " == *" $1 "* ]]; }
FLAGS="$*"

export SD_GAME_DIR="${SD_GAME_DIR:-/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead}"
export SD_PAK_DIR="$SD_GAME_DIR/Content/Paks"
export DOTNET_ROOT="${DOTNET_ROOT:-$HOME/.dotnet}"
DOTNET="${DOTNET:-$DOTNET_ROOT/dotnet}"

# UE4SS writes Mappings.usmap with its magic bytes swapped (30 C4 instead of C4 30); fix a copy.
mkdir -p "$HERE/mappings"
cp "$SD_GAME_DIR/Binaries/Win64/Mappings.usmap" "$HERE/mappings/Mappings.usmap"
printf '\xc4\x30' | dd of="$HERE/mappings/Mappings.usmap" bs=1 seek=0 count=2 conv=notrunc 2>/dev/null
export SD_USMAP="$HERE/mappings/Mappings.usmap"

"$DOTNET" build -c Release -o "$HERE/bin/out" "$HERE/SdExport.csproj" >/dev/null
SDX="$HERE/bin/out/SdExport"

# ACL-compressed animations need CUE4Parse's native library, which NuGet doesn't ship for Linux.
if [[ ! -f "$HERE/bin/out/CUE4Parse-Natives.so" ]] && { has --anims || has --all; }; then
    SRC="$HERE/natives/CUE4Parse"
    [[ -d "$SRC" ]] || git clone --depth 1 --recurse-submodules --shallow-submodules https://github.com/FabianFG/CUE4Parse.git "$SRC"
    cmake -S "$SRC/CUE4Parse-Natives" -B "$SRC/CUE4Parse-Natives/build" -DCMAKE_BUILD_TYPE=Release >/dev/null
    cmake --build "$SRC/CUE4Parse-Natives/build" -j >/dev/null
    cp "$SRC/CUE4Parse-Natives/build/CUE4Parse-Natives.so" "$HERE/bin/out/"
fi

# Menus: every UI texture (title, backgrounds, mode previews, icons).
"$SDX" tex "$OUT" SurrounDead/Content/UI/Textures/ | grep -c '^ok' | xargs echo "ui textures:"
# Lockpicking minigame (UI/Lockpick/LockpickUI, LockWidget).
"$SDX" tex "$OUT" SurrounDead/Content/UI/Lockpick/Textures/ >/dev/null
# The note paper (LetterUI: BP_KeypadNote's code).
"$SDX" tex "$OUT" SurrounDead/Content/UI/Widgets/Journal/Quests/QuestPageBlank >/dev/null
# The journal's Skills and Quests pages (gameplay/journal.rs, journal/quests.rs).
"$SDX" tex "$OUT" SurrounDead/Content/UI/Widgets/Journal/Skills/ SurrounDead/Content/UI/Widgets/Journal/Markers/ SurrounDead/Content/UI/Widgets/Journal/Quests/ SurrounDead/Content/UI/Widgets/Journal/Quests/Icons/ >/dev/null
"$SDX" raw "$OUT" SurrounDead/Content/Blueprints/PluginsAndAssets/Font/F_ESRPG__RU_EN_.ufont >/dev/null
# The map (M; MapWidget): BP_MapData_LongdownValley's Map Texture, the player arrow and marker fade,
# the POI marker icons (BP_POIManager's MarkerImage) and PlayerMarkerSelectionUI's two marker icons
# (its other ten are UI/Textures, exported above).
"$SDX" tex "$OUT" SurrounDead/Content/Blueprints/MinimapSystem/Texture/RT_WorldImageCreator_Tex \
    SurrounDead/Content/Blueprints/MinimapSystem/Texture/T_Player \
    SurrounDead/Content/Blueprints/MinimapSystem/Texture/T_Marker SurrounDead/Content/Blueprints/MinimapSystem/Texture/T_Point \
    SurrounDead/Content/Blueprints/MinimapSystem/Texture/T_Marker_Fade SurrounDead/Content/POI/ >/dev/null
# Crafting amount arrows (W_PlayerCraftingUI).
"$SDX" tex "$OUT" SurrounDead/Content/UI/CleanFlatIcons/Textures/png_128/icon/icon_arrow/icon_arrow_13 \
    SurrounDead/Content/UI/CleanFlatIcons/Textures/png_128/icon/icon_arrow/icon_arrow_14 >/dev/null

# Meshes (.glb) for the survivor, zombies, vehicles and world props.
if has --models; then
    "$SDX" mesh "$OUT" \
        SurrounDead/Content/PlayerModel/ \
        SurrounDead/Content/PolygonFiles/PolygonCityZombies/Meshes/Characters/ \
        SurrounDead/Content/PolygonFiles/PolygonDog/Meshes/SK_Animal_Dog_Zombie_GermanShepherd \
        SurrounDead/Content/PolygonFiles/PolygonBossZombies/Meshes/Characters/ \
        SurrounDead/Content/WIP/ZombieBoss_Butcher SurrounDead/Content/WIP/ZombieBoss_Infestation SurrounDead/Content/WIP/ZombieBoss_Jugger \
        SurrounDead/Content/WIP/ZombieBoss_Scientist SurrounDead/Content/WIP/ZombieBoss_Survivor \
        SurrounDead/Content/AI/Animals/Deer/PolyArtDeer/Meshes/ SurrounDead/Content/AI/Animals/Rabbit/RabbitAnims/Meshes/ \
        SurrounDead/Content/PolygonFiles/PolyArtWolf/Meshes/ SurrounDead/Content/PolygonFiles/PolyArtBear/Meshes/ \
        SurrounDead/Content/PolygonFiles/PolygonApocalypse/Meshes/ \
        SurrounDead/Content/PolygonFiles/PolygonNature/Meshes/ \
        SurrounDead/Content/PolygonFiles/PolygonBattleRoyale/Meshes/Vehicles/SK_Veh_Car_01 \
        SurrounDead/Content/PolygonFiles/PolygonConstruction/Meshes/Vehicles/SK_Veh_Pickup_01 \
        SurrounDead/Content/PolygonFiles/PolygonBattleRoyale/Meshes/Props/SM_Prop_EmergencyDrop_01_Parachute \
        SurrounDead/Content/PolygonFiles/PolygonShops/Meshes/Props/SM_Prop_Hunting_Fishing_Lure_01 \
        SurrounDead/Content/Meshes/World/ | grep -c '^ok' | xargs echo "meshes:"
    # The traders' bodies, hair and beards (traders.json: SkeletalMesh, HairMeshes, BeardMeshes),
    # and the quest givers' revolvers.
    python3 -c "import json; print('\n'.join(sorted({m for c in json.load(open('$ROOT/assets/data/traders.json'))['classes'].values() for m in [c['mesh'], *c['hair'], *c['beard'], c.get('gun')] if m})))" \
        | xargs "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "trader meshes:"
fi

# The map: landscape + placements + splines + actors, every mesh it references, terrain splat.
if has --level; then
    "$SDX" level "$LEVEL_OUT" "$LEVEL"
    python3 - "$LEVEL_OUT" "$OUT" > "$HERE/mappings/level_meshes.txt" <<'EOF'
import json, os, sys
d, out = sys.argv[1], sys.argv[2]
meshes = set(json.load(open(f"{d}/placements.json"))) | {s["mesh"] for s in json.load(open(f"{d}/splines.json"))}
print("\n".join(m for m in sorted(meshes) if not os.path.exists(f"{out}/{m}.glb")))
EOF
    if [[ -s "$HERE/mappings/level_meshes.txt" ]]; then
        xargs -a "$HERE/mappings/level_meshes.txt" -n 300 "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "level meshes:"
    fi
    P=SurrounDead/Content/PolygonFiles
    M=$P/PolygonNatureBiomes_Meadow/PNB_Meadow_Forest/Terrain
    J=$P/PolygonNatureBiomes_Jungle/PNB_Tropical_Jungle/Terrain
    "$SDX" tex "$OUT" $M/Grass_Texture_03 $M/Grass_Texture_02 $M/Mud_Texture_01 $M/Rock_Rough_Texture_01 \
        $J/Sand_Wavy_Texture_01 SurrounDead/Content/Terrain/Textures/ConcreteTexture $M/Grass_Pebbles_Texture_01 \
        $M/Grass_Leaves_Heavy_Texture_01 $J/Grass_Debris_Texture_01 $J/Grass_Texture_01 $M/Leaves_Texture_01 >/dev/null
    python3 "$HERE/gen_terrain.py" "$LEVEL_OUT"
    # Landscape grass (LandscapeGrassType per paint layer) and weather (UDS presets + the level's
    # Ultra_Dynamic_Weather): assets/data/grass.json, weather.json; the grass meshes.
    "$SDX" levelactors "$LEVEL_OUT" "$LEVEL" >/dev/null
    # The trees' simple collision (BodySetup capsules / convex hulls): assets/data/simple_collision.json.
    TREES=$(python3 -c "import json,sys; print(' '.join(m for m in json.load(open(sys.argv[1])) if 'Tree' in m.rsplit('/',1)[1]))" "$LEVEL_OUT/placements.json")
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" $TREES >/dev/null 2>&1 || true
    python3 "$HERE/gen_collision.py" "$TMP" $TREES
    rm -rf "$TMP"
    "$SDX" levellights "$LEVEL_OUT" "$LEVEL" >/dev/null
    python3 "$HERE/gen_lights.py" "$LEVEL_OUT"
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/Terrain/Foliage/ \
        SurrounDead/Content/UltraDynamicSky/Blueprints/Weather_Effects/Weather_Presets/ >/dev/null
    python3 "$HERE/gen_grass.py" "$TMP" > "$HERE/mappings/grass_meshes.txt"
    xargs -a "$HERE/mappings/grass_meshes.txt" "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "grass meshes:"
    python3 "$HERE/gen_weather.py" "$TMP" "$LEVEL_OUT"
    rm -rf "$TMP"
fi

# Particle systems (gameplay/particles.rs): the level's ParticleSystemComponents resolved against
# their Blueprints, every Cascade system under the FX folders, and the meshes mesh emitters draw.
if has --particles; then
    "$SDX" levelparticles "$LEVEL_OUT" "$LEVEL" >/dev/null
    python3 "$HERE/gen_particles.py" "$LEVEL_OUT"
    xargs -a "$HERE/mappings/particle_meshes.txt" "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "particle meshes:"
fi

# Prefabs (level/prefabs.rs): every PrefabMaster child, its child actors' Blueprints resolved;
# the meshes not exported yet. The spawners' Marker Texture names resolve against the UI export.
if has --prefabs; then
    python3 "$HERE/gen_prefabs.py"
    if [[ -s "$HERE/mappings/prefab_meshes.txt" ]]; then
        xargs -a "$HERE/mappings/prefab_meshes.txt" "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "prefab meshes:"
    fi
fi

# Material map for every exported mesh (assets/game/materials.json).
if has --models || has --level || has --particles || has --prefabs; then
    python3 "$HERE/gen_materials.py"
fi

# Animations used by models/anim.rs (AnimSet::clips / actions).
if has --anims; then
    A=SurrounDead/Content/Animations
    CLIPS=(
        Anims/No_Rifle/ThirdPersonIdle Anims/No_Rifle/ThirdPersonWalk Anims/No_Rifle/ThirdPersonRun
        Anims/No_Rifle/ThirdPersonSprint Anims/No_Rifle/A_JumpAir Player/Anims/Random/Anim_Regular_death1
        Anims/No_Rifle/ThirdPersonIdleMelee Anims/Other/Anim_Regular_ladder_up-inplace Anims/Other/Anim_Regular_ladder_idle
        Anims/Rifle_Standing/Rifle_Idle Anims/Rifle_Standing/A_AR_Walk Anims/Rifle_Standing/A_AR_Run
        Anims/Rifle_Standing/Idle_Rifle_Ironsights Anims/Rifle_Standing/Walk_Fwd_Rifle_Ironsights
        Anims/Rifle_Standing/Walk_Bwd_Rifle_Ironsights Anims/Rifle_Standing/Walk_Lt_Rifle_Ironsights
        Anims/Rifle_Standing/Walk_Rt_Rifle_Ironsights
        Anims/Pistol_Standing/A_HG_Idle Anims/Pistol_Standing/A_HG_Walk Anims/Pistol_Standing/A_HG_Run
        Anims/Pistol_Standing/A_HG_Aim_Stand Anims/Pistol_Standing/A_AimWalk_0 Anims/Pistol_Standing/A_AimWalk_180
        Anims/Pistol_Standing/A_AimWalk_-90 Anims/Pistol_Standing/A_AimWalk_90
        Player/1Handed/Walk_F_Seq Player/1Handed/Walk_B_Seq Player/1Handed/Walk_L_Seq Player/1Handed/Walk_R_Seq
        Anims/No_Rifle/A_Crouch_Idle Anims/No_Rifle/A_Crouch_Walk Player/Anims/Actions/SittingPlayer_Anim
        Anims/Rifle_Crouch/A_AR_Crouch_Idle Anims/Rifle_Crouch/A_AR_Crouch_Walk
        Anims/Rifle_Crouch/Crouch_Idle_Rifle_Ironsights Anims/Rifle_Crouch/Crouch_Walk_Fwd_Rifle_Ironsights
        Anims/Rifle_Crouch/Crouch_Walk_BwdRifle_Ironsights Anims/Rifle_Crouch/Crouch_Walk_Lt_Rifle_Ironsights
        Anims/Rifle_Crouch/Crouch_Walk_Rt_Rifle_Ironsights
        Anims/Pistol_Crouch/A_HG_Crouch_Idle Anims/Pistol_Crouch/A_HG_Crouch_Walk Anims/Pistol_Crouch/W1_Crouch_Aim_Idle_IPC
        Anims/Pistol_Crouch/W1_CrouchWalk_Aim_F_Loop_IPC Anims/Pistol_Crouch/W1_CrouchWalk_Aim_B_Loop_IPC
        Anims/Pistol_Crouch/W1_CrouchWalk_Aim_L_Loop_IPC Anims/Pistol_Crouch/W1_CrouchWalk_Aim_R_Loop_IPC
        Anims/MeleeCrouch/Crouch_Idle_Axe Anims/MeleeCrouch/Crouch_Fwd_Axe Anims/MeleeCrouch/Crouch_Bwd_Axe
        Anims/MeleeCrouch/Crouch_Lt_Axe Anims/MeleeCrouch/Crouch_Rt_Axe
        Anims/Other/Fire_Rifle_Ironsights Anims/Other/Reload_Rifle_Hip Anims/Other/Reload_Pistol
        Player/1Handed/Attack_01_Seq Player/1Handed/Attack_02_Seq Player/1Handed/Attack_04_Seq Player/1Handed/Attack_06_Seq
        Zombie/Regular/A_Enemy_Zombie_Idle Zombie/Regular/Locomotion/Anim_Zombie_walk1_inplace
        Zombie/Regular/Locomotion/Anim_Zombie_run1_inplace Zombie/Regular/Attacks/Anim_Zombie_attack1
        Zombie/Regular/Attacks/Anim_Zombie_attack2 Zombie/Regular/Attacks/Anim_Zombie_attack3
        Zombie/Regular/Locomotion/Anim_Zombie_walk2_inplace Zombie/Regular/Locomotion/Anim_Zombie_walk3_inplace
        Zombie/Regular/Locomotion/Anim_Zombie_run2_inplace Zombie/Regular/Other/Other/Anim_Zombie_hit
        Zombie/Regular/Other/Other/Zombie_DeathPosition
        Zombie/ZombieAnims/Zombie_Idle_01 Zombie/Regular/Other/Other/Zombie_Run
        Zombie/Regular/Locomotion/Anim_Zombie_ground-idle Zombie/Regular/Locomotion/Anim_Zombie_ground-move_inplace
        Player/Anims/Actions/Push/Anim_SmallMeleePush Player/Anims/Actions/Push/Anim_PistolPush
        Player/Anims/Actions/Push/Anim_RiflePush Zombie/ZombieAnims/Zombie_HitReact_Head
        Player/Anims/Actions/Throw Anims/Parkour/Roll
        Anims/Parkour/MQ_Climb_RM1 Anims/Parkour/MQ_GettingUp_RM1 Anims/Parkour/MQ_Vault_RM1
        Anims/Parkour/MQ_JumpingDownFromWall_RM1
        Player/Anims/Swim/Anim_SwimIdle Player/Anims/Swim/Anim_SlowSwim Player/Anims/Swim/Anim_FastSwim
        Player/Anims/Swim/Anim_SwimUnderwater
        Anims/No_Rifle/A_Idle
        Anims/Melee/animBatonAttack1 Anims/No_Rifle/A_Car_IdleShortened
        Zombie/ZombieBoss/Anim_Zombie_attack1 Zombie/ZombieBoss/Locomotion/Anim_Zombie_idle
        Zombie/ZombieBoss/Locomotion/Anim_Zombie_walk3_inplace Zombie/ZombieBoss/Locomotion/Anim_Zombie_run1_inplace
    )
    "$SDX" animjson "$OUT" "${CLIPS[@]/#/$A/}" | grep -c '^ok' | xargs echo "animations:"
    # BP_Zombie_Dog: ZombieDogAILocomotion (stand/walk/run), bite and hit montages, death.
    D=SurrounDead/Content/PolygonFiles/PolygonDog/Animation
    DOG=(Locomotion/AS_Dog_Locomotion_Standing Locomotion/AS_Dog_Locomotion_Walking Locomotion/AS_Dog_Locomotion_Running
        Attack/AS_Dog_Attack_Bite Attack/AS_Dog_Attack_Hit Death/AS_Dog_Death_Standing)
    "$SDX" animjson "$OUT" "${DOG[@]/#/$D/}" | grep -c '^ok' | xargs echo "hound animations:"
    # The animals (gameplay/animals.rs): Locomotion1D samples, attack and hit montages, the rabbit's death.
    AN=(AI/Animals/Deer/PolyArtDeer/Animations/Common/Deer_C_Idle_01 AI/Animals/Deer/PolyArtDeer/Animations/InPlace/Deer_IP_Walk_Forward
        AI/Animals/Deer/PolyArtDeer/Animations/InPlace/Deer_IP_Run_Forward AI/Animals/Deer/PolyArtDeer/Animations/InPlace/Deer_IP_Damage_Back_Left
        AI/Animals/Rabbit/RabbitAnims/Animations/Common/Rabbit_C_Idle AI/Animals/Rabbit/RabbitAnims/Animations/InPlace/Rabbit_IP_Walk_Forward
        AI/Animals/Rabbit/RabbitAnims/Animations/InPlace/Rabbit_IP_Run_Forward AI/Animals/Rabbit/RabbitAnims/Animations/RootMotion/Rabbit_RM_Damage_Left
        AI/Animals/Rabbit/RabbitAnims/Animations/Common/Rabbit_C_Death3
        PolygonFiles/PolyArtWolf/Animations/Common/Wolf_C_Idle01 PolygonFiles/PolyArtWolf/Animations/InPlace/Wolf_IP_Walk_Forward
        PolygonFiles/PolyArtWolf/Animations/InPlace/Wolf_IP_Run_Forward PolygonFiles/PolyArtWolf/Animations/InPlace/Wolf_IP_Attack_Bite_Front
        PolygonFiles/PolyArtWolf/Animations/RootMotion/Wolf_RM_Damage_Back_Left PolygonFiles/PolyArtWolf/Animations/RootMotion/Wolf_RM_Damage_Back_Right
        PolygonFiles/PolyArtWolf/Animations/RootMotion/Wolf_RM_Damage_Left PolygonFiles/PolyArtWolf/Animations/RootMotion/Wolf_RM_Damage_Right
        PolygonFiles/PolyArtBear/Animations/Common/Bear_C_Idle PolygonFiles/PolyArtBear/Animations/In_Place/Bear_IP_Walk_Forward
        PolygonFiles/PolyArtBear/Animations/In_Place/Bear_IP_Run_Forward PolygonFiles/PolyArtBear/Animations/Common/Bear_C_Attack_Claw_Right)
    "$SDX" animjson "$OUT" "${AN[@]/#/SurrounDead/Content/}" | grep -c '^ok' | xargs echo "animal animations:"
    # BP_PoliceStationGarageDoor's folding gate (level/garage.rs), mesh and FoldingGate_Anim_Montage's clip.
    "$SDX" mesh "$OUT" SurrounDead/Content/Meshes/Gate/FoldingGate >/dev/null
    "$SDX" animjson "$OUT" SurrounDead/Content/Meshes/Gate/FoldingGate_Anim >/dev/null
fi

# Regenerate assets/data/zombies.json (committed): BP_MasterZombie's children and their values.
# Vehicles (assets/data/vehicles.json): BP_VehicleMaster's children, their wheels and torque
# curves, DT_VehicleInfo; their skeletal meshes (the wheel bones' bind positions come from them).
if has --vehicles; then
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/Blueprints/Vehicles/ >/dev/null
    python3 "$HERE/gen_vehicles.py" "$TMP" "$OUT" >/dev/null
    python3 -c "import json; d=json.load(open('$ROOT/assets/data/vehicles.json')); print('\n'.join(sorted({m for c in d['classes'].values() for m in [c['mesh'], *c['extra_meshes']]})))" \
        | xargs "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "vehicle meshes:"
    python3 "$HERE/gen_vehicles.py" "$TMP" "$OUT"
    python3 "$HERE/gen_materials.py"
    rm -rf "$TMP"
fi

if has --ai; then
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/AI/Zombies/ SurrounDead/Content/AI/Enum_AILootTables SurrounDead/Content/AI/SD_AIComponent >/dev/null
    # The animals: BP_Animal's children, their AI controllers, trees, tasks and blendspaces, with
    # bytecode (their meshes, loot tables and bites are set in Blueprint graphs).
    A=SurrounDead/Content/AI/Animals
    SD_SCRIPT=1 "$SDX" json "$TMP" $A/BP_Animal $A/Deer/ $A/Rabbit/ $A/Wolf/ $A/Bear/ SurrounDead/Content/AI/Zombies/Tasks/AI_FindRandomLocation >/dev/null
    # The zombie bosses: BP_ZombieBoss's graph and the attack montage (its arm-trace notify).
    SD_SCRIPT=1 "$SDX" json "$TMP" SurrounDead/Content/AI/Zombies/Boss/BP_ZombieBoss >/dev/null
    "$SDX" json "$TMP" SurrounDead/Content/Animations/Zombie/ZombieBoss/ >/dev/null
    # The human AI (bandits, the military, scavengers): their Blueprints with bytecode (the kill's
    # XP, the health row), their weapons, SmartAI's component and master controller.
    SD_SCRIPT=1 "$SDX" json "$TMP" SurrounDead/Content/AI/Humans/ >/dev/null
    # The settlers (BP_RandomCivilian_*, BT_RandomCivilian, its wander task, the name tables).
    SD_SCRIPT=1 "$SDX" json "$TMP" SurrounDead/Content/AI/SafeZoneAndCivilians/ SurrounDead/Content/AI/Other/ >/dev/null
    "$SDX" json "$TMP" SurrounDead/Content/SmartAI/Blueprints/AI/Components/BP_SmartAIComponent \
        SurrounDead/Content/SmartAI/Blueprints/AI/AICharacters/BP_MasterAIController >/dev/null
    python3 "$HERE/gen_ai.py" "$TMP"
    # The human AI's bodies, hair, beards and weapons (gameplay/humans.rs), not yet exported.
    python3 -c "import json,os,sys; d=json.load(open('$ROOT/assets/data/zombies.json')); h=[c for c in d['classes'].values() if c.get('human')]; m=[x for c in h for x in c['meshes']+c['human']['hair']+c['human']['beard']+[(c['human'].get('backpack') or {}).get('mesh')] if x]+[x for c in json.load(open('$ROOT/assets/data/civilians.json'))['classes'].values() for x in c['meshes']+c['hair']+c['beard']+[b['mesh'] for b in c['backpacks']]]+[w['mesh'] for w in d.get('weapons',{}).values() if w.get('mesh')]; print('\n'.join(sorted({'SurrounDead/Content/'+x for x in m if not os.path.exists('$OUT/SurrounDead/Content/'+x+'.glb')})))" > "$HERE/mappings/human_meshes.txt"
    if [[ -s "$HERE/mappings/human_meshes.txt" ]]; then
        xargs -a "$HERE/mappings/human_meshes.txt" "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "human meshes:"
        python3 "$HERE/gen_materials.py"
    fi
    rm -rf "$TMP"
fi

# Regenerate assets/data/items.json (committed) from the JigSInventory data assets.
if has --items; then
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/Inventory/Items/DataAssets/ SurrounDead/Content/Inventory/Containers/ SurrounDead/Content/Blueprints/Throwables/ \
        SurrounDead/Content/Inventory/Items/Pickups/Weapons/ SurrounDead/Content/Inventory/Items/Other/Components/BP_WeaponsPickupComponent \
        SurrounDead/Content/Inventory/Items/Other/Components/Data/Enum_Firearms \
        SurrounDead/Content/AI/AILootTables/ SurrounDead/Content/Blueprints/SkillTree/ \
        SurrounDead/Content/Inventory/Crafting/DT_CraftingTableTypes SurrounDead/Content/PlayerModel/Other/ClothingSettings \
        SurrounDead/Content/Meshes/PlayerModels/SK_Chr_ToplessMale_01_Skeleton \
        SurrounDead/Content/JigSInventory/Jigsaw/Data/DataAsset/JigsawItem_DataAsset >/dev/null
    python3 "$HERE/gen_items.py" "$TMP"
    rm -rf "$TMP"
    # Traders (assets/data/traders.json): BP_TraderMaster's children, DT_VendorTable, dialogues.
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/Inventory/Traders/ SurrounDead/Content/Quests/Dialogue/Traders/ SurrounDead/Content/AI/Humans/Trader/BP_MasterTrader SurrounDead/Content/Quests/Blueprints/NPCs/ SurrounDead/Content/Quests/Dialogue/Settlement/ >/dev/null
    python3 "$HERE/gen_traders.py" "$TMP"
    rm -rf "$TMP"
    # Inventory pictures are renders of each item's inspect mesh (gameplay/icons.rs); containers
    # spawned at runtime (police/keycard briefcases) draw their class's mesh.
    python3 -c "import json,os,sys; d=json.load(open('$ROOT/assets/data/items.json')); m=[v['mesh'] for v in [*d['items'].values(), *d['containers'].values()]]; print('\n'.join(sorted({x for x in m if x and not os.path.exists('$OUT/'+x+'.glb')})))" > "$HERE/mappings/item_meshes.txt"
    if [[ -s "$HERE/mappings/item_meshes.txt" ]]; then
        xargs -a "$HERE/mappings/item_meshes.txt" -n 300 "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "item meshes:"
    fi
    python3 "$HERE/gen_materials.py"
fi

# Buildables (assets/data/buildables.json): what BuildingComponent reads off the buildable items,
# the Buildable_* actors' mesh parts and options, the Horde Beacon's waves; their meshes.
if has --buildables; then
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/Inventory/Items/DataAssets/ SurrounDead/Content/Blueprints/BuildingSystem/Actors/ \
        SurrounDead/Content/Blueprints/BuildingSystem/BP_NoBuildArea SurrounDead/Content/Blueprints/BuildingSystem/Enum_BuildingSnapTypes \
        SurrounDead/Content/Blueprints/HordeSystem/ SurrounDead/Content/JigSInventory/Jigsaw/Data/DataAsset/JigsawItem_DataAsset >/dev/null
    python3 "$HERE/gen_buildables.py" "$TMP"
    rm -rf "$TMP"
    python3 -c "import json,os; d=json.load(open('$ROOT/assets/data/buildables.json')); m={i['mesh'] for i in d['items'].values() if i['mesh']}|{p['mesh'] for a in d['actors'].values() for p in a['parts']}; print('\n'.join(sorted(x for x in m if not os.path.exists('$OUT/'+x+'.glb'))))" > "$HERE/mappings/buildable_meshes.txt"
    if [[ -s "$HERE/mappings/buildable_meshes.txt" ]]; then
        xargs -a "$HERE/mappings/buildable_meshes.txt" "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "buildable meshes:"
    fi
    python3 "$HERE/gen_materials.py"
fi

# Quests (assets/data/quests.json): the Narrative quest graphs under Content/Quests/Quests and
# BP_GuaranteedSpawner (the laptop events' crate), its pickups mapped through the item data assets.
if has --quests; then
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/Quests/ SurrounDead/Content/Inventory/Items/DataAssets/ >/dev/null
    python3 "$HERE/gen_quests.py" "$TMP"
    python3 -c "import json; c=json.load(open('$ROOT/assets/data/quests.json'))['crate']; print(c['mesh']); print(c['lid'])" \
        | xargs "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "quest crate meshes:"
    python3 "$HERE/gen_materials.py"
    rm -rf "$TMP"
fi

# Sounds the port plays: assets/data/sounds.json (committed) and decoded waves in assets/game/
# (gen_sounds.py; Bink Audio needs vgmstream-cli). Run after --items (it reads the guns' cues).
if has --sounds || has --all; then
    python3 "$HERE/gen_sounds.py"
fi

# Regenerate assets/data/menus.json (committed) from the menu widget blueprints.
if has --menu-data; then
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/UI/Menus/ SurrounDead/Content/Input/IMC_General \
        SurrounDead/Content/PlayerModel/Other/Enum_Occupation SurrounDead/Content/Blueprints/Saves/Save_Difficulties \
        SurrounDead/Content/Blueprints/Other/DT_Difficulty >/dev/null
    SD_SCRIPT=1 "$SDX" json "$TMP" SurrounDead/Content/Blueprints/SD_GameInstance >/dev/null
    python3 "$HERE/gen_menu_data.py" "$TMP"
    rm -rf "$TMP"
fi
