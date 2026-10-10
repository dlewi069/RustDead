"""
RustDead Clean-Room First-Time Asset Extraction Pipeline
Requires a legally owned copy of SurrounDead on Steam.
Extracts models, audio, textures, and fonts from the player's local installation
directly into assets/, without distributing copyrighted assets in the git repository.
"""

import os
import sys
import shutil
import struct
import subprocess
from pathlib import Path

# Verify ownership first
from verify_ownership import verify_ownership

def extract_fonts(game_dir):
    font_src = game_dir / "SurrounDead" / "Content" / "UI" / "Fonts"
    dest_dir = Path("assets/fonts")
    dest_dir.mkdir(parents=True, exist_ok=True)
    
    font_map = [
        ("Montserrat-Bold.ufont", "Montserrat-Bold.ttf"),
        ("Montserrat-ExtraBold.ufont", "Montserrat-ExtraBold.ttf"),
    ]
    
    copied = 0
    for src_name, dst_name in font_map:
        src = font_src / src_name
        dst = dest_dir / dst_name
        if src.exists():
            shutil.copyfile(src, dst)
            copied += 1
            print(f"  [Font] Extracted: {dst_name}")
    print(f"[Fonts] {copied} fonts extracted to assets/fonts/")

def extract_backgrounds(game_dir):
    from PIL import Image
    bg_src = game_dir / "SurrounDead" / "Content" / "UI" / "Textures" / "Backgrounds"
    dest_dir = Path("assets/textures/backgrounds")
    dest_dir.mkdir(parents=True, exist_ok=True)

    def dds_header(width, height, fourcc=b"DXT1"):
        h = bytearray(128)
        h[0:4] = b"DDS "
        struct.pack_into("<IIIIII", h, 4, 124, 0x00081007, height, width, (width * height) // 2, 0)
        struct.pack_into("<I", h, 28, 1)
        struct.pack_into("<II4sIIIII", h, 76, 32, 0x4, fourcc, 0, 0, 0, 0, 0)
        struct.pack_into("<I", h, 108, 0x1000)
        return h

    count = 0
    for i in range(1, 10):
        uexp = bg_src / f"Screenshot{i}.uexp"
        dst_png = dest_dir / f"screenshot{i}.png"
        if uexp.exists():
            with open(uexp, "rb") as f:
                data = f.read()
            payload = data[123:123 + 1843200]
            if len(payload) == 1843200:
                tmp_dds = dest_dir / f"temp_{i}.dds"
                with open(tmp_dds, "wb") as f_out:
                    f_out.write(dds_header(2560, 1440, b"DXT1") + payload)
                try:
                    with Image.open(tmp_dds) as im:
                        im.save(dst_png)
                    os.remove(tmp_dds)
                    count += 1
                    print(f"  [Background] Extracted: screenshot{i}.png (2560x1440)")
                except Exception as e:
                    if tmp_dds.exists(): os.remove(tmp_dds)
    print(f"[Backgrounds] {count} 2.5K panoramas extracted to assets/textures/backgrounds/")

def extract_models(game_dir):
    dest_dir = Path("assets/models")
    dest_dir.mkdir(parents=True, exist_ok=True)
    
    bulk_cli = Path("tools/UEBulkExport/UEBulkExport/UEBulkExport.Cli.exe")
    pak_path = game_dir / "SurrounDead" / "Content" / "Paks"
    usmap_src = game_dir / "SurrounDead" / "Binaries" / "Win64" / "Mappings.usmap"
    usmap_swapped = Path("tools/UEBulkExport/test_mappings.usmap")

    if bulk_cli.exists() and pak_path.exists():
        print(f"[Models] Extracting authentic 3D meshes from {pak_path} using UEBulkExport...")
        
        # Ensure swapped magic usmap exists
        if not usmap_swapped.exists() and usmap_src.exists():
            with open(usmap_src, "rb") as f:
                data = bytearray(f.read())
            data[0], data[1] = data[1], data[0]
            with open(usmap_swapped, "wb") as f:
                f.write(data)

        cmd = [
            str(bulk_cli),
            "--paks", str(pak_path),
            "--usmap", str(usmap_swapped),
            "--out", "assets/extracted",
            "--mode", "full",
            "--include", "(Characters/SK_Chr_|PolygonCityZombies/Meshes/Characters|PolygonBossZombies/Meshes/Characters|PolygonApocalypse/Meshes/Vehicles|Meshes/World|PolygonFiles/PolygonNature/Meshes/Trees|Meshes/Container|PolygonApocalypse/Meshes/Props|PolygonApocalypse/Meshes/Weapons|Meshes/SK_Wep|Meshes/Firearms)",
            "--game", "5.3",
            "--no-json"
        ]
        subprocess.run(cmd, check=False)

        # Run conversion script
        import subprocess as sp
        sp.run([sys.executable, "scripts/convert_extracted_to_game_assets.py"])
    
    existing = list(dest_dir.glob("*.obj"))
    print(f"[Models] {len(existing)} authentic .obj models ready in assets/models/")

def main():
    print("=================================================================")
    print("      RustDead: First-Time Extractor & Ownership Pipeline        ")
    print("=================================================================")

    verified, info = verify_ownership()
    if not verified:
        print("\nAborting setup. You must legally own SurrounDead to continue.")
        sys.exit(1)

    game_dir = info["game_dir"]
    print(f"\nExtracting assets from: {game_dir} ...\n")

    # 1. Fonts
    extract_fonts(game_dir)

    # 2. 2.5K Panoramic Backgrounds
    extract_backgrounds(game_dir)

    # 3. Models
    extract_models(game_dir)

    print("\n=================================================================")
    print(" [OK] First-time setup complete! Engine assets ready for launch.  ")
    print("=================================================================")

if __name__ == "__main__":
    main()
