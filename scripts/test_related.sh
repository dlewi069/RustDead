#!/usr/bin/env bash
# Run only the tests relevant to what changed (use while working; scripts/check.sh is the full run).
#
#   scripts/test_related.sh                 # files changed vs git HEAD (+ untracked)
#   scripts/test_related.sh src/gameplay/combat.rs assets/data/items.json
#
# Mapping: src/<a>/<b>.rs -> cargo test filter `<a>::<b>::` (mod.rs -> its directory), src/main.rs
# -> the binary's tests, assets/data/*.json and assets/game/* -> the modules that read them,
# extractor/*.py and scripts/*.py -> compile + data checks, extractor/*.cs -> dotnet build.
# Changed Rust files without any #[test] are listed: give them tests (they then run here and in
# scripts/check.sh automatically).
set -uo pipefail
cd "$(dirname "$0")/.."

if [[ $# -gt 0 ]]; then
    files=("$@")
else
    mapfile -t files < <( { git diff --name-only HEAD 2>/dev/null; git ls-files --others --exclude-standard; } | sort -u)
fi

filters=()
python=0
dotnet=0
bin=0
untested=()
add() { for f in "$@"; do [[ " ${filters[*]-} " == *" $f "* ]] || filters+=("$f"); done; }

for f in "${files[@]}"; do
    case "$f" in
        src/main.rs) bin=1 ;;
        src/*.rs)
            m="${f#src/}"; m="${m%.rs}"; m="${m%/mod}"; m="${m//\//::}"
            [[ "$m" == "lib" ]] && continue
            add "$m::"
            [[ -f "$f" ]] && ! grep -q '#\[test\]' "$f" && untested+=("$f")
            ;;
        assets/data/items.json) add gameplay::items:: gameplay::equipment:: gameplay::inventory_ui:: gameplay::combat:: gameplay::lockpick:: ; python=1 ;;
        assets/data/menus.json) add menus:: models:: gameplay::daynight:: ; python=1 ;;
        assets/data/quests.json) add gameplay::quests:: gameplay::journal:: ; python=1 ;;
        assets/game/*) add models:: level:: ; python=1 ;;
        extractor/*.py|scripts/*.py) python=1 ;;
        extractor/*.cs|extractor/*.csproj) dotnet=1 ;;
    esac
done

status=0
if [[ ${#filters[@]} -gt 0 ]]; then
    echo "== cargo test (lib) ${filters[*]}"
    out=$(cargo test -q --lib -- "${filters[@]}" 2>&1) || status=1
    grep -E "^test result|FAILED|panicked|^error" <<<"$out"
fi
if [[ $bin -eq 1 ]]; then
    echo "== cargo build + test (bin)"
    out=$(cargo test -q --bin surroundead 2>&1) || status=1
    grep -E "^test result|FAILED|panicked|^error" <<<"$out"
fi
if [[ $python -eq 1 ]]; then
    echo "== python compile + data checks"
    python3 -m py_compile extractor/*.py scripts/*.py && python3 scripts/check_data.py || status=1
fi
if [[ $dotnet -eq 1 ]]; then
    echo "== dotnet build (extractor)"
    DOTNET_ROOT="${DOTNET_ROOT:-$HOME/.dotnet}" "${DOTNET_ROOT:-$HOME/.dotnet}/dotnet" build -c Release -o extractor/bin/out extractor/SdExport.csproj 2>&1 | grep -E "error|Build succeeded" || status=1
fi
if [[ ${#filters[@]} -eq 0 && $bin -eq 0 && $python -eq 0 && $dotnet -eq 0 ]]; then
    echo "nothing testable changed"
fi
if [[ ${#untested[@]} -gt 0 ]]; then
    echo "!! changed files with no #[test]: ${untested[*]}"
fi
[[ $status -eq 0 ]] && echo "RELATED TESTS PASSED" || echo "RELATED TESTS FAILED"
exit $status
