import os
import struct
import subprocess

def dds_header(width, height, fourcc=b"DXT1"):
    h = bytearray(128)
    h[0:4] = b"DDS "
    bpp_div = 1 if fourcc == b"DXT5" else 2
    struct.pack_into("<IIIIII", h, 4, 124, 0x00081007, height, width, (width * height) // bpp_div, 0)
    struct.pack_into("<I", h, 28, 1) # 1 mipmap
    struct.pack_into("<II4sIIIII", h, 76, 32, 0x4, fourcc, 0, 0, 0, 0, 0)
    struct.pack_into("<I", h, 108, 0x1000)
    return h

out_dir = "assets/textures"
os.makedirs(out_dir, exist_ok=True)

targets = [
    ("assets/SurrounDead/Content/PolygonFiles/PolygonApocalypse/Textures/T_PolygonApocalypse_01_A.ubulk", "assets/textures/world_apocalypse.png", 2048, 2048, b"DXT1"),
    ("assets/SurrounDead/Content/PolygonFiles/PolygonApocalypse/Textures/PolygonApocalypseRoadFixed_Texture_01_A.ubulk", "assets/textures/road.png", 2048, 2048, b"DXT1"),
    ("assets/SurrounDead/Content/PolygonFiles/PolygonCityZombies/Textures/T_PolygonZombie_01_A.ubulk", "assets/textures/zombie.png", 1024, 1024, b"DXT1"),
    ("assets/SurrounDead/Content/PolygonFiles/PolygonMilitary/Textures/T_PolygonMilitary_01_A.ubulk", "assets/textures/military.png", 2048, 2048, b"DXT1"),
    ("assets/SurrounDead/Content/PolygonFiles/PolygonApocalypse/Textures/Veh/T_PolygonApocalypse_Veh_01.ubulk", "assets/textures/vehicle.png", 1024, 1024, b"DXT1"),
    ("assets/SurrounDead/Content/PolygonFiles/PolygonApocalypse/Textures/Misc/T_PolygonApocalypse_Dirt_01.ubulk", "assets/textures/dirt.png", 512, 512, b"DXT1"),
]

for src, dst, w, h, fmt in targets:
    if not os.path.exists(src):
        print(f"Skipping {src}")
        continue
    div = 1 if fmt == b"DXT5" else 2
    with open(src, "rb") as f:
        data = f.read((w * h) // div)
    
    tmp_dds = f"/tmp/{os.path.basename(dst)}.dds"
    hdr = dds_header(w, h, fmt)
    with open(tmp_dds, "wb") as f:
        f.write(hdr + data)
    
    res = subprocess.run(["ffmpeg", "-y", "-i", tmp_dds, dst], capture_output=True)
    if res.returncode == 0:
        print(f"✓ Converted {dst}")
    else:
        print(f"✗ Failed {dst}: {res.stderr.decode('utf-8', errors='ignore')}")
