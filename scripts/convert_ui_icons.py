import os
import struct
import subprocess

def probe_and_extract(filepath, out_png):
    if not os.path.exists(filepath):
        print(f"Skipping {filepath} (not found)")
        return False
    with open(filepath, "rb") as f:
        data = f.read()

    formats = [
        (512, 512, b"DXT5", 1, False),
        (512, 512, b"DXT1", 2, False),
        (256, 256, b"DXT5", 1, False),
        (256, 256, b"DXT1", 2, False),
        (1024, 1024, b"DXT5", 1, False),
        (1024, 1024, b"DXT1", 2, False),
        (128, 128, b"DXT5", 1, False),
        (64, 64, b"DXT5", 1, False),
        (256, 256, None, 4, True),
        (512, 512, None, 4, True),
    ]

    for w, h, fourcc, div, is_raw in formats:
        needed = (w * h * div) if is_raw else ((w * h) // div)
        if needed > len(data): continue
        offsets_to_try = [len(data) - needed, len(data) - needed - 4, 0]
        for off in offsets_to_try:
            if off < 0 or off + needed > len(data): continue
            chunk = data[off:off+needed]
            if is_raw:
                tmp_raw = f"/tmp/probe_{os.path.basename(out_png)}.raw"
                with open(tmp_raw, "wb") as f_raw: f_raw.write(chunk)
                res = subprocess.run(["ffmpeg", "-y", "-f", "rawvideo", "-pixel_format", "bgra", "-video_size", f"{w}x{h}", "-i", tmp_raw, out_png], capture_output=True)
                if res.returncode == 0:
                    print(f"✓ RAW BGRA {out_png} ({w}x{h})")
                    return True
            else:
                h_bytes = bytearray(128)
                h_bytes[0:4] = b"DDS "
                struct.pack_into("<IIIIII", h_bytes, 4, 124, 0x00081007, h, w, needed, 0)
                struct.pack_into("<I", h_bytes, 28, 1)
                struct.pack_into("<II4sIIIII", h_bytes, 76, 32, 0x4, fourcc, 0, 0, 0, 0, 0)
                struct.pack_into("<I", h_bytes, 108, 0x1000)
                tmp_dds = f"/tmp/probe_{os.path.basename(out_png)}.dds"
                with open(tmp_dds, "wb") as f_dds: f_dds.write(h_bytes + chunk)
                res = subprocess.run(["ffmpeg", "-y", "-i", tmp_dds, out_png], capture_output=True)
                if res.returncode == 0:
                    print(f"✓ {fourcc.decode()} {out_png} ({w}x{h})")
                    return True
    print(f"✗ Failed {out_png}")
    return False

out_dir = "assets/textures/ui"
os.makedirs(out_dir, exist_ok=True)

targets = [
    ("assets/SurrounDead/Content/UI/Textures/SDLogoTransparent.uexp", f"{out_dir}/logo.png"),
    ("assets/SurrounDead/Content/UI/Textures/Hitmarker.uexp", f"{out_dir}/hitmarker.png"),
    ("assets/SurrounDead/Content/UI/Textures/hunger.uexp", f"{out_dir}/icon_hunger.png"),
    ("assets/SurrounDead/Content/UI/Textures/thirst.uexp", f"{out_dir}/icon_thirst.png"),
    ("assets/SurrounDead/Content/UI/Textures/bleed01.uexp", f"{out_dir}/icon_bleed.png"),
    ("assets/SurrounDead/Content/UI/Textures/brokenbone.uexp", f"{out_dir}/icon_brokenbone.png"),
    ("assets/SurrounDead/Content/UI/Textures/Scopes/SniperScopeDetailedOverlay.uexp", f"{out_dir}/sniper_scope.png"),
    ("assets/SurrounDead/Content/UI/Textures/over_250_survival_flat_icons/backpack.ubulk", f"{out_dir}/icon_backpack.png"),
    ("assets/SurrounDead/Content/UI/Textures/over_250_survival_flat_icons/gun2.ubulk", f"{out_dir}/icon_gun.png"),
    ("assets/SurrounDead/Content/UI/Textures/over_250_survival_flat_icons/bullets.ubulk", f"{out_dir}/icon_bullets.png"),
    ("assets/SurrounDead/Content/UI/Textures/over_250_survival_flat_icons/flashlight_on.ubulk", f"{out_dir}/icon_flashlight.png"),
    ("assets/SurrounDead/Content/UI/Textures/over_250_survival_flat_icons/gas_mask.ubulk", f"{out_dir}/icon_gasmask.png"),
    ("assets/SurrounDead/Content/UI/Textures/over_250_survival_flat_icons/car_wheel.ubulk", f"{out_dir}/icon_wheel.png"),
    ("assets/SurrounDead/Content/UI/Textures/over_250_survival_flat_icons/padlock.ubulk", f"{out_dir}/icon_lock.png"),
    ("assets/SurrounDead/Content/UI/Textures/over_250_survival_flat_icons/map.ubulk", f"{out_dir}/icon_map.png"),
    ("assets/SurrounDead/Content/UI/Textures/over_250_survival_flat_icons/radio.ubulk", f"{out_dir}/icon_radio.png"),
]

for src, dst in targets:
    probe_and_extract(src, dst)
