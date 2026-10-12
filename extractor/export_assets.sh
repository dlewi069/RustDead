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
#   export_assets.sh --items      regenerate assets/data/items.json (committed): items, loot tables, containers
#   export_assets.sh --all        everything above
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$HERE")"
OUT="$ROOT/assets/game"
LEVEL="SurrounDead/Content/Levels/LongdownValley"
LEVEL_OUT="$OUT/levels/LongdownValley"

has() { [[ " $* " == *" --all "* ]] || [[ " ${FLAGS} " == *" $1 "* ]]; }
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
# Crafting amount arrows (W_PlayerCraftingUI).
"$SDX" tex "$OUT" SurrounDead/Content/UI/CleanFlatIcons/Textures/png_128/icon/icon_arrow/icon_arrow_13 \
    SurrounDead/Content/UI/CleanFlatIcons/Textures/png_128/icon/icon_arrow/icon_arrow_14 >/dev/null

# Meshes (.glb) for the survivor, zombies, vehicles and world props.
if has --models; then
    "$SDX" mesh "$OUT" \
        SurrounDead/Content/PlayerModel/ \
        SurrounDead/Content/PolygonFiles/PolygonCityZombies/Meshes/Characters/ \
        SurrounDead/Content/PolygonFiles/PolygonDog/Meshes/SK_Animal_Dog_Zombie_GermanShepherd \
        SurrounDead/Content/PolygonFiles/PolygonApocalypse/Meshes/ \
        SurrounDead/Content/PolygonFiles/PolygonNature/Meshes/ \
        SurrounDead/Content/PolygonFiles/PolygonBattleRoyale/Meshes/Vehicles/SK_Veh_Car_01 \
        SurrounDead/Content/PolygonFiles/PolygonConstruction/Meshes/Vehicles/SK_Veh_Pickup_01 \
        SurrounDead/Content/Meshes/World/ | grep -c '^ok' | xargs echo "meshes:"
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

# Material map for every exported mesh (assets/game/materials.json).
if has --models || has --level; then
    python3 "$HERE/gen_materials.py"
fi

# Animations used by models/anim.rs (AnimSet::clips / actions).
if has --anims; then
    A=SurrounDead/Content/Animations
    CLIPS=(
        Anims/No_Rifle/ThirdPersonIdle Anims/No_Rifle/ThirdPersonWalk Anims/No_Rifle/ThirdPersonRun
        Anims/No_Rifle/ThirdPersonSprint Anims/No_Rifle/A_JumpAir Player/Anims/Random/Anim_Regular_death1
        Anims/No_Rifle/ThirdPersonIdleMelee
        Anims/Rifle_Standing/Rifle_Idle Anims/Rifle_Standing/A_AR_Walk Anims/Rifle_Standing/A_AR_Run
        Anims/Rifle_Standing/Idle_Rifle_Ironsights Anims/Rifle_Standing/Walk_Fwd_Rifle_Ironsights
        Anims/Rifle_Standing/Walk_Bwd_Rifle_Ironsights Anims/Rifle_Standing/Walk_Lt_Rifle_Ironsights
        Anims/Rifle_Standing/Walk_Rt_Rifle_Ironsights
        Anims/Pistol_Standing/A_HG_Idle Anims/Pistol_Standing/A_HG_Walk Anims/Pistol_Standing/A_HG_Run
        Anims/Pistol_Standing/A_HG_Aim_Stand Anims/Pistol_Standing/A_AimWalk_0 Anims/Pistol_Standing/A_AimWalk_180
        Anims/Pistol_Standing/A_AimWalk_-90 Anims/Pistol_Standing/A_AimWalk_90
        Player/1Handed/Walk_F_Seq Player/1Handed/Walk_B_Seq Player/1Handed/Walk_L_Seq Player/1Handed/Walk_R_Seq
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
    )
    "$SDX" animjson "$OUT" "${CLIPS[@]/#/$A/}" | grep -c '^ok' | xargs echo "animations:"
    # BP_Zombie_Dog: ZombieDogAILocomotion (stand/walk/run), bite and hit montages, death.
    D=SurrounDead/Content/PolygonFiles/PolygonDog/Animation
    DOG=(Locomotion/AS_Dog_Locomotion_Standing Locomotion/AS_Dog_Locomotion_Walking Locomotion/AS_Dog_Locomotion_Running
        Attack/AS_Dog_Attack_Bite Attack/AS_Dog_Attack_Hit Death/AS_Dog_Death_Standing)
    "$SDX" animjson "$OUT" "${DOG[@]/#/$D/}" | grep -c '^ok' | xargs echo "hound animations:"
fi

# Regenerate assets/data/zombies.json (committed): BP_MasterZombie's children and their values.
if has --ai; then
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/AI/Zombies/ SurrounDead/Content/AI/Enum_AILootTables SurrounDead/Content/AI/SD_AIComponent >/dev/null
    python3 "$HERE/gen_ai.py" "$TMP"
    rm -rf "$TMP"
fi

# Regenerate assets/data/items.json (committed) from the JigSInventory data assets.
if has --items; then
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/Inventory/Items/DataAssets/ SurrounDead/Content/Inventory/Containers/ \
        SurrounDead/Content/Inventory/Items/Pickups/Weapons/ SurrounDead/Content/Inventory/Items/Other/Components/BP_WeaponsPickupComponent \
        SurrounDead/Content/Inventory/Items/Other/Components/Data/Enum_Firearms \
        SurrounDead/Content/AI/AILootTables/ \
        SurrounDead/Content/Inventory/Crafting/DT_CraftingTableTypes \
        SurrounDead/Content/Meshes/PlayerModels/SK_Chr_ToplessMale_01_Skeleton \
        SurrounDead/Content/JigSInventory/Jigsaw/Data/DataAsset/JigsawItem_DataAsset >/dev/null
    python3 "$HERE/gen_items.py" "$TMP"
    rm -rf "$TMP"
    # Inventory pictures are renders of each item's inspect mesh (gameplay/icons.rs).
    python3 -c "import json,os,sys; d=json.load(open('$ROOT/assets/data/items.json')); print('\n'.join(sorted({v['mesh'] for v in d['items'].values() if v['mesh'] and not os.path.exists('$OUT/'+v['mesh']+'.glb')})))" > "$HERE/mappings/item_meshes.txt"
    if [[ -s "$HERE/mappings/item_meshes.txt" ]]; then
        xargs -a "$HERE/mappings/item_meshes.txt" -n 300 "$SDX" mesh "$OUT" | grep -c '^ok' | xargs echo "item meshes:"
    fi
    python3 "$HERE/gen_materials.py"
fi

# Regenerate assets/data/menus.json (committed) from the menu widget blueprints.
if has --menu-data; then
    TMP="$(mktemp -d)"
    "$SDX" json "$TMP" SurrounDead/Content/UI/Menus/ SurrounDead/Content/Input/IMC_General \
        SurrounDead/Content/PlayerModel/Other/Enum_Occupation SurrounDead/Content/Blueprints/Saves/Save_Difficulties \
        SurrounDead/Content/Blueprints/Other/DT_Difficulty >/dev/null
    python3 "$HERE/gen_menu_data.py" "$TMP"
    rm -rf "$TMP"
fi
