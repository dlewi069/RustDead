import os
import struct
import subprocess

def dds_header(width, height, fourcc=b"DXT5"):
    h = bytearray(128)
    h[0:4] = b"DDS "
    struct.pack_into("<IIIIII", h, 4, 124, 0x00081007, height, width, width * height, 0)
    struct.pack_into("<I", h, 28, 1) # 1 mipmap
    # Pixel format at offset 76 (32 bytes)
    struct.pack_into("<II4sIIIII", h, 76, 32, 0x4, fourcc, 0, 0, 0, 0, 0)
    struct.pack_into("<I", h, 108, 0x1000) # caps
    return h

out_dir = "assets/textures/lockpick"
os.makedirs(out_dir, exist_ok=True)

targets = [
    ("assets/SurrounDead/Content/UI/Lockpick/Textures/LockBackdrop.ubulk", "assets/textures/lockpick/lock_backdrop.png", 1024, 1024),
    ("assets/SurrounDead/Content/UI/Lockpick/Textures/LockMobile.ubulk", "assets/textures/lockpick/lock_mobile.png", 1024, 1024),
    ("assets/SurrounDead/Content/UI/Lockpick/Textures/LockStatic.ubulk", "assets/textures/lockpick/lock_static.png", 1024, 1024),
    ("assets/SurrounDead/Content/UI/Lockpick/Textures/Screwdriver.ubulk", "assets/textures/lockpick/screwdriver.png", 1024, 1024),
    ("assets/SurrounDead/Content/UI/Lockpick/Textures/Picks/LP01Mobile.ubulk", "assets/textures/lockpick/pick_mobile.png", 1024, 1024),
]

for src, dst, w, h in targets:
    if not os.path.exists(src):
        print(f"Skipping {src}")
        continue
    with open(src, "rb") as f:
        data = f.read(w * h)
    
    tmp_dds = f"/tmp/{os.path.basename(dst)}.dds"
    hdr = dds_header(w, h, b"DXT5")
    with open(tmp_dds, "wb") as f:
        f.write(hdr + data)
    
    res = subprocess.run(["ffmpeg", "-y", "-i", tmp_dds, dst], capture_output=True)
    if res.returncode == 0:
        print(f"✓ Converted {dst}")
    else:
        print(f"✗ Failed {dst}: {res.stderr.decode('utf-8', errors='ignore')}")
