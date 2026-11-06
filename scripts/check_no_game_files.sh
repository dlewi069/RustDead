#!/bin/sh
# Refuses game files. A rule in .gitignore does nothing for a file git already
# tracks, so this checks what is really being committed or pushed.
#   check_no_game_files.sh --staged   check the files staged for commit
#   check_no_game_files.sh <rev>      check every path in the history of <rev>
PATTERN='\.(uasset|uexp|ubulk|umap|utoc|ucas|pak|ufont|wem|bnk)$|^assets/(SurrounDead|audio|textures|data|fonts|Engine|game|models|extracted[^/]*)/|^(surroundead_items|asset_paths)\.json$|^savegame\.json|^test_screen[^/]*\.png$|^saves/'

if [ "$1" = "--staged" ]; then
    found=$(git diff --cached --name-only --diff-filter=ACMR | grep -E "$PATTERN")
else
    found=$(git rev-list --objects "$1" | cut -s -d' ' -f2- | grep -E "$PATTERN" | sort -u)
fi

[ -z "$found" ] && exit 0
count=$(printf '%s\n' "$found" | wc -l)
echo "Blocked: $count game file(s) found. They must not go to GitHub." >&2
printf '%s\n' "$found" | head -5 >&2
[ "$count" -gt 5 ] && echo "...and $((count - 5)) more" >&2
exit 1
