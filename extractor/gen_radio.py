#!/usr/bin/env python3
"""Build assets/data/radio.json from DT_Radio (Blueprints/Other/Radio): each row's lines and
DelayPerLine, which RadioTextUI shows one after another (gameplay/radio.rs).

usage: gen_radio.py <json dump dir>     (run by `export_assets.sh --items`)

    {"rows": {"CoyoteCall": {"lines": ["[MILITARY] This is the military...", ...], "delays": [8.0, ...]}}}

An empty line in the table (CoyoteCall's second) is kept as "".
"""
import json
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
OUT = Path(__file__).resolve().parent.parent / "assets" / "data" / "radio.json"


def field(d, prefix):
    """Struct fields are suffixed with GUIDs: Line_15_462A..."""
    for k, v in d.items():
        if k.split("_")[0] == prefix:
            return v
    return None


def main():
    table = next(e for e in json.loads((ROOT / "Blueprints/Other/Radio/DT_Radio.json").read_text()) if e["Type"] == "DataTable")
    rows = {}
    for name, row in table["Rows"].items():
        lines = [((t or {}).get("SourceString") or "") for t in field(row, "Line") or []]
        rows[name] = {"lines": lines, "delays": field(row, "DelayPerLine") or []}
    OUT.write_text(json.dumps({"rows": rows}, indent=1, ensure_ascii=False) + "\n")
    print(f"radio: {len(rows)} rows")


if __name__ == "__main__":
    main()
