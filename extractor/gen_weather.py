#!/usr/bin/env python3
"""Weather (Ultra Dynamic Weather) -> assets/data/weather.json.

usage: gen_weather.py <json dump root> <level dir>   (run by export_assets.sh --level)

From the UDS_Weather_Settings presets (unset values are the class default, 0) and the level's
Ultra_Dynamic_Weather / Weather_Override_Volume actors (actors.json `props`):

    { "presets": { "Rain": { "name": "Rain", "cloud": 7.5, "rain": 7, "snow": 0, "thunder": 0,
                             "wind": 3, "fog": 3, "dust": 0, "wetness": 1, "snow_cover": 0,
                             "dust_cover": 0 }, ... },
      "seasons": { "Summer": [["Clear_Skies", 2.0], ...], ... },   # Weather Type Probabilities
      "timing": "Daily", "change_hour": 8, "transition_hours": 0.25, "random_start": true,
      "overrides": [ { "weather": "Foggy", "transition": 75.0, "triangles": [[x, z], ...] } ] }

Positions are Bevy metres (UE X, Y -> x, z).
"""
import json
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
LEVEL = Path(sys.argv[2])
OUT = Path(__file__).resolve().parent.parent / "assets" / "data" / "weather.json"
FIELDS = {
    "Cloud Coverage": "cloud", "Rain": "rain", "Snow": "snow", "Thunder/Lightning": "thunder",
    "Wind Intensity": "wind", "Fog": "fog", "Dust": "dust", "Material Wetness": "wetness",
    "Material Snow Coverage": "snow_cover", "Material Dust Coverage": "dust_cover",
}
# UDS_RandomWeatherTiming values (by enumerator name; the level stores the name).
TIMING = {"NewEnumerator3": "Random Interval", "NewEnumerator0": "Daily", "NewEnumerator1": "Hourly", "NewEnumerator2": "Disabled"}


def key_name(k):
    """"UDS_Weather_Settings_C'.../Weather_Presets/Rain.Rain'" -> "Rain"."""
    return k.rstrip("'").rsplit(".", 1)[-1]


def main():
    presets = {}
    for f in sorted((ROOT / "UltraDynamicSky/Blueprints/Weather_Effects/Weather_Presets").glob("*.json")):
        for e in json.loads(f.read_text()):
            if e["Type"] == "UDS_Weather_Settings_C":
                p = e.get("Properties") or {}
                row = {v: float(p.get(k, 0.0)) for k, v in FIELDS.items()}
                row["name"] = p.get("User Friendly Name", f.stem)
                presets[f.stem] = row
    actors = json.loads((LEVEL / "actors.json").read_text())
    udw = next(a["props"] for a in actors if a["class"] == "Ultra_Dynamic_Weather_C")
    seasons = {}
    for season in ("Spring", "Summer", "Autumn", "Winter"):
        raw = json.loads(udw[f"Weather Type Probabilities ({season})"])
        seasons[season] = [[key_name(e["Key"]), e["Value"]] for e in raw if e["Value"] > 0]
    timing = TIMING.get(udw.get("Random Weather Variation", "").strip('"').split("::")[-1], "Daily")
    overrides = []
    for a in actors:
        if a["class"] != "Weather_Override_Volume_C":
            continue
        p = a["props"]
        pts = json.loads(p["World Space Triangles"])
        overrides.append({
            "weather": p.get("Weather"),
            "transition": float(p.get("Transition Width", 0)) / 100.0,
            "triangles": [[q["X"] / 100.0, q["Y"] / 100.0] for q in pts],
        })
    out = {
        "presets": presets,
        "seasons": seasons,
        "timing": timing,
        # Ultra_Dynamic_Weather class default (the level doesn't override it).
        "change_hour": float(udw.get("Random Weather Change Hour", 8)),
        "transition_hours": float(udw.get("Transition Length", 0.35)),
        "random_start": udw.get("Begin Play Weather is Random", "false") == "true",
        "blend_seasons": udw.get("Blend Season Probabilities", "true") == "true",
        "overrides": overrides,
    }
    OUT.write_text(json.dumps(out, indent=1, sort_keys=True))
    print(f"weather: {len(presets)} presets, timing {timing}, {len(overrides)} override volumes")


if __name__ == "__main__":
    main()
