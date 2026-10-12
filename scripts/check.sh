#!/usr/bin/env bash
# The full check, run at the end of every piece of work (and by the Claude Code Stop hook):
#
#   1. build          no compiler warnings
#   2. clippy         no more warnings than scripts/clippy_baseline.txt (lower it when you fix some)
#   3. unit tests     every #[test] in the crate (lib + bin)
#   4. data           extractor/scripts compile; scripts/check_data.py consistency checks
#   5. tour           the screenshot tour (SD_MENU_SHOTS) + scripts/check_tour.py assertions
#
#   scripts/check.sh            everything
#   scripts/check.sh --quick    skip the tour (~4 min)
#   scripts/check.sh --hook     Stop-hook mode: skip if nothing changed since the last pass, exit 2
#                               with the report on stderr when a step fails
#
# New tests join automatically: #[test]s anywhere in src/, functions listed in CHECKS in
# check_data.py / check_tour.py, and new tour steps in src/menus/autoshot.rs.
set -uo pipefail
cd "$(dirname "$0")/.."

QUICK=0
HOOK=0
for a in "$@"; do
    case "$a" in
        --quick) QUICK=1 ;;
        --hook) HOOK=1 ;;
    esac
done

OUT=target/check
mkdir -p "$OUT"
REPORT="$OUT/report.txt"

# What the checks depend on: sources, committed data, scripts, extractor code.
fingerprint() {
    find src assets/data extractor scripts Cargo.toml -type f \
        \( -name '*.rs' -o -name '*.json' -o -name '*.py' -o -name '*.sh' -o -name '*.cs' -o -name '*.toml' -o -name '*.wesl' \) \
        -not -path 'extractor/bin/*' -not -path 'extractor/obj/*' -not -path 'extractor/natives/*' -not -path 'extractor/mappings/*' \
        -printf '%p %s %T@\n' 2>/dev/null | sort | sha1sum | cut -d' ' -f1
}

if [[ $HOOK -eq 1 ]]; then
    cat >/dev/null # the hook's JSON input
    FP=$(fingerprint)
    if [[ -f "$OUT/passed" && "$(cat "$OUT/passed")" == "$FP" ]]; then
        exit 0
    fi
fi

: >"$REPORT"
FAILED=0
step() { # name, status, detail
    local mark="PASS"
    [[ $2 -ne 0 ]] && mark="FAIL" && FAILED=1
    printf '%-12s %s  %s\n' "$1" "$mark" "$3" | tee -a "$REPORT"
}

# 1. build
cargo build 2>"$OUT/build.log"; code=$?
warnings=$(grep -c '^warning' "$OUT/build.log")
[[ $code -eq 0 && $warnings -eq 0 ]]; step build $? "$warnings warnings$( [[ $code -ne 0 ]] && echo ', BUILD FAILED')"
[[ $code -ne 0 || $warnings -ne 0 ]] && grep -E '^(warning|error)' -A6 "$OUT/build.log" | head -40 >>"$REPORT"

# 2. clippy (ratchet)
cargo clippy -q 2>"$OUT/clippy.log"
now=$(grep -cE '^(warning|error)' "$OUT/clippy.log")
base=$(cat scripts/clippy_baseline.txt 2>/dev/null || echo 999999)
[[ $now -le $base ]]; step clippy $? "$now warnings (baseline $base)"
if [[ $now -gt $base ]]; then
    echo "  new clippy warnings; see $OUT/clippy.log" >>"$REPORT"
elif [[ $now -lt $base ]]; then
    echo "$now" >scripts/clippy_baseline.txt # ratchet down
fi

# 3. unit tests
cargo test -q --no-fail-fast >"$OUT/test.log" 2>&1
passed=$(grep -E '^test result' "$OUT/test.log" | awk '{p+=$4} END {print p+0}')
failed=$(grep -E '^test result' "$OUT/test.log" | awk '{f+=$6} END {print f+0}')
compiled=$(grep -c '^error' "$OUT/test.log")
[[ $failed -eq 0 && $compiled -eq 0 && $passed -gt 0 ]]; step tests $? "$passed passed, $failed failed"
[[ $failed -ne 0 || $compiled -ne 0 ]] && grep -E 'panicked|FAILED|^error|assert' "$OUT/test.log" | head -30 >>"$REPORT"

# 4. data + scripts
{ python3 -m py_compile extractor/*.py scripts/*.py && python3 scripts/check_data.py; } >"$OUT/data.log" 2>&1
code=$?
step data $code "$(tail -1 "$OUT/data.log")"
[[ $code -ne 0 ]] && grep -E 'ERROR|Error' "$OUT/data.log" | head -20 >>"$REPORT"

# 5. screenshot tour
if [[ $QUICK -eq 0 ]]; then
    rm -rf "$OUT/shots" && mkdir -p "$OUT/shots"
    SD_START=3 SD_MENU_SHOTS="$PWD/$OUT/shots" timeout 900 cargo run -q >"$OUT/tour.log" 2>&1
    code=$?
    python3 scripts/check_tour.py "$OUT/tour.log" "$OUT/shots" "$code" >"$OUT/tour_checks.log" 2>&1
    code=$?
    step tour $code "$(tail -1 "$OUT/tour_checks.log")  (shots: $OUT/shots)"
    [[ $code -ne 0 ]] && grep -E 'ERROR' "$OUT/tour_checks.log" | head -20 >>"$REPORT"
else
    echo "tour         SKIP  (--quick)" | tee -a "$REPORT"
fi

if [[ $FAILED -eq 0 ]]; then
    echo "ALL CHECKS PASSED" | tee -a "$REPORT"
    [[ $QUICK -eq 0 ]] && fingerprint >"$OUT/passed"
    rm -f "$OUT/hook_failures"
    exit 0
fi
echo "CHECKS FAILED (details in $OUT/*.log)" | tee -a "$REPORT"
if [[ $HOOK -eq 1 ]]; then
    # Block the stop so the failures get fixed, but give up after three tries in a row.
    n=$(( $(cat "$OUT/hook_failures" 2>/dev/null || echo 0) + 1 ))
    echo $n >"$OUT/hook_failures"
    if [[ $n -le 3 ]]; then
        { echo "scripts/check.sh failed (attempt $n/3): fix these, then finish."; cat "$REPORT"; } >&2
        exit 2
    fi
    rm -f "$OUT/hook_failures"
    echo "scripts/check.sh still failing after 3 attempts; stopping anyway." >&2
fi
exit 1
