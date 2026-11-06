#!/usr/bin/env bash
# Dump game packages (Blueprints with their Kismet bytecode, widgets, data tables) as JSON for
# porting work. Default: every gameplay folder into target/bpdump/ (gitignored, ~1 min), which
# then answers most questions with grep + extractor/kismet.py + extractor/widget_tree.py:
#
#   scripts/dump_bp.sh                                  # everything below, into target/bpdump
#   scripts/dump_bp.sh <out> SurrounDead/Content/X/Y    # one package (or a folder, trailing /)
#   grep -rl "SpawnFloatingDamageText" target/bpdump     # who calls it
#   python3 extractor/kismet.py target/bpdump/Blueprints/Other/ExplosiveBarrel/BP_ExplosiveBarrelMaster.json [function]
#   python3 extractor/widget_tree.py target/bpdump/UI/Widgets/Other/ModularYesNoUI.json
#   extractor/bin/out/SdExport list <substring>          # find a package (needs the env below)
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
export SD_GAME_DIR="${SD_GAME_DIR:-/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead}"
export SD_PAK_DIR="$SD_GAME_DIR/Content/Paks"
export DOTNET_ROOT="${DOTNET_ROOT:-$HOME/.dotnet}"
export SD_USMAP="$HERE/extractor/mappings/Mappings.usmap"
export SD_SCRIPT=1
SDX="$HERE/extractor/bin/out/SdExport"
if [[ ! -x "$SDX" ]]; then
    "$DOTNET_ROOT/dotnet" build -c Release -o "$HERE/extractor/bin/out" "$HERE/extractor/SdExport.csproj" >/dev/null
fi
OUT="${1:-$HERE/target/bpdump}"
shift || true
mkdir -p "$OUT"
if [[ $# -gt 0 ]]; then
    "$SDX" json "$OUT" "$@" | grep -c '^json' | xargs echo "dumped:"
    exit 0
fi
C=SurrounDead/Content
"$SDX" json "$OUT" $C/Blueprints/ $C/AI/ $C/SmartAI/Blueprints/ $C/UI/ $C/Infestation/ $C/Quests/ $C/Prefabs/ \
    $C/POI/ $C/JigSInventory/ $C/Inventory/Containers/ $C/Inventory/Widgets/ $C/Inventory/Traders/ $C/Inventory/Crafting/ \
    $C/Inventory/Items/Pickups/ $C/Inventory/Items/Other/ $C/Inventory/Items/LocalAttachments/ \
    | grep -c '^json' | xargs echo "dumped:"
