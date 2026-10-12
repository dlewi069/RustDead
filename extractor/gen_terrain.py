#!/usr/bin/env python3
"""Pack a level's landscape paint layers for the terrain splat shader.

usage: gen_terrain.py <level dir>   (e.g. assets/game/levels/LongdownValley; run by export_assets.sh --level)

Reads landscape.json + weight_<layer>.png from `SdExport level` and writes:
  terrain_weights.png  3 RGBA tiles stacked vertically = 12 weight channels (LAYERS, then NoFoliage)
  terrain_layers.png   12 square albedo tiles stacked vertically (texture array on the GPU)
  terrain.json         layer names, tiling (UV repeats per metre) and which textures were used

Terrain_Material blends these layers by weight in its graph; the graph isn't readable here, so
the layer -> texture pairing below is by name (Rock -> Rock_Rough_Texture_01, ...) and the
tiling comes from the instance's "<Layer>SizeNear" scalars.
"""
import json
import sys
from pathlib import Path

from PIL import Image

LEVEL = Path(sys.argv[1])
GAME = LEVEL.parents[1]
POLY = "SurrounDead/Content/PolygonFiles/"
MEADOW = POLY + "PolygonNatureBiomes_Meadow/PNB_Meadow_Forest/Terrain/"
JUNGLE = POLY + "PolygonNatureBiomes_Jungle/PNB_Tropical_Jungle/Terrain/"

# (weight layer, albedo texture package, Terrain_Material_Inst "<X>SizeNear")
LAYERS = [
    ("Grass_LayerInfo", MEADOW + "Grass_Texture_03", 0.2),
    ("Meadow_LayerInfo", MEADOW + "Grass_Texture_02", 0.25),
    ("Mud_LayerInfo", MEADOW + "Mud_Texture_01", 0.2),
    ("Rock_LayerInfo", MEADOW + "Rock_Rough_Texture_01", 0.075),
    ("Sand_LayerInfo", JUNGLE + "Sand_Wavy_Texture_01", 0.075),
    ("Concrete_LayerInfo", "SurrounDead/Content/Terrain/Textures/ConcreteTexture", 0.25),
    ("Pine_LayerInfo", MEADOW + "Grass_Pebbles_Texture_01", 0.2),
    ("Birch_LayerInfo", MEADOW + "Grass_Leaves_Heavy_Texture_01", 0.2),
    ("Wetland_LayerInfo", JUNGLE + "Grass_Debris_Texture_01", 0.3),
    ("Tropical_LayerInfo", JUNGLE + "Grass_Texture_01", 0.2),
    ("RadArea_LayerInfo", MEADOW + "Leaves_Texture_01", 0.25),
]
TILE = 512


def main():
    info = json.loads((LEVEL / "landscape.json").read_text())
    w, h = info["heightmap_size"]
    half = (w // 2, h // 2)  # 2 m weight resolution is plenty for blending

    channels = []
    for name, _, _ in LAYERS:
        f = LEVEL / f"weight_{name}.png"
        channels.append(Image.open(f).convert("L").resize(half, Image.BILINEAR) if f.exists() else Image.new("L", half, 0))
    # Channel 12: NoFoliage, which keeps landscape grass off (paths, yards); not an albedo layer.
    f = LEVEL / "weight_NoFoliage_LayerInfo.png"
    channels.append(Image.open(f).convert("L").resize(half, Image.BILINEAR) if f.exists() else Image.new("L", half, 0))
    tiles = [Image.merge("RGBA", channels[i:i + 4]) for i in range(0, 12, 4)]
    stacked = Image.new("RGBA", (half[0], half[1] * 3))
    for i, t in enumerate(tiles):
        stacked.paste(t, (0, i * half[1]))
    stacked.save(LEVEL / "terrain_weights.png")

    layers = Image.new("RGBA", (TILE, TILE * 12))
    used = []
    for i, (name, tex, _) in enumerate(LAYERS):
        f = GAME / (tex + ".png")
        if not f.exists():
            print(f"  missing {f} (export it with SdExport tex)")
            continue
        layers.paste(Image.open(f).convert("RGBA").resize((TILE, TILE), Image.LANCZOS), (0, i * TILE))
        used.append(tex)
    layers.save(LEVEL / "terrain_layers.png")

    (LEVEL / "terrain.json").write_text(json.dumps({
        "layers": [l[0] for l in LAYERS],
        # UE "SizeNear" is UV repeats per metre-ish at the near blend; good enough as tiling.
        "tiling": [l[2] for l in LAYERS] + [0.2],
        "textures": used,
    }, indent=1))
    print(f"terrain: {len(LAYERS)} layers, weights {half[0]}x{half[1]}x3, textures {len(used)}")


main()
