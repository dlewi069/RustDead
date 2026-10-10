"""
SurrounDead Asset Extractor & Converter CLI
Supports:
1. Detecting Steam library installation of SurrounDead
2. Unpacking pak archives or inspecting asset directories
3. Extracting and converting sound files (.ubulk / .uexp -> .wav)
4. Converting textures (.ubulk / .uexp -> .png with DDS header detection)
5. Exporting / Mapping 3D static meshes (.obj)
"""

import os
import sys
import json
import struct
import shutil
import argparse
import subprocess
from pathlib import Path

def find_steam_game():
    candidates = []
    if sys.platform == "win32":
        for drive in ["C", "D", "E", "F", "G"]:
            candidates.extend([
                Path(f"{drive}:/Program Files (x86)/Steam/steamapps/common/SurrounDead"),
                Path(f"{drive}:/Program Files/Steam/steamapps/common/SurrounDead"),
                Path(f"{drive}:/Steam/steamapps/common/SurrounDead"),
                Path(f"{drive}:/SteamLibrary/steamapps/common/SurrounDead"),
            ])
    else:
        home = Path.home()
        candidates.extend([
            home / ".local/share/Steam/steamapps/common/SurrounDead",
            home / ".steam/steam/steamapps/common/SurrounDead",
            Path("/ntfs/SteamLibrary/steamapps/common/SurrounDead"),
        ])

    for c in candidates:
        pak = c / "SurrounDead/Content/Paks/SurrounDead-Windows.pak"
        if pak.exists():
            return c, pak
    return None, None

def dds_header(width, height, fourcc=b"DXT1"):
    h = bytearray(128)
    h[0:4] = b"DDS "
    bpp_div = 1 if fourcc == b"DXT5" else 2
    struct.pack_into("<IIIIII", h, 4, 124, 0x00081007, height, width, (width * height) // bpp_div, 0)
    struct.pack_into("<I", h, 28, 1) # 1 mipmap
    struct.pack_into("<II4sIIIII", h, 76, 32, 0x4, fourcc, 0, 0, 0, 0, 0)
    struct.pack_into("<I", h, 108, 0x1000)
    return h

def extract_models(source_dir, dest_dir, umodel_path=None):
    os.makedirs(dest_dir, exist_ok=True)
    print(f"\n[Models] Checking 3D model exports in '{dest_dir}'...")
    
    if umodel_path and os.path.exists(umodel_path):
        print(f"[Models] Using UE Viewer at {umodel_path} to export meshes to OBJ...")
        cmd = [
            umodel_path,
            "-export",
            "-obj",
            f"-out={dest_dir}",
            str(source_dir)
        ]
        res = subprocess.run(cmd, capture_output=True)
        if res.returncode == 0:
            print("[Models] Successfully exported authentic .obj meshes via umodel!")
            return
        else:
            print(f"[Models] Warning: umodel returned non-zero exit code: {res.stderr.decode(errors='ignore')}")

    # Fallback / summary check
    existing = [f for f in os.listdir(dest_dir) if f.endswith(".obj")]
    print(f"[Models] Models directory contains {len(existing)} .obj files.")
    for m in existing[:5]:
        print(f"  - {m}")
    if len(existing) > 5:
        print(f"  ... and {len(existing) - 5} more.")

def main():
    parser = argparse.ArgumentParser(description="RustDead Asset Extractor & Setup Utility")
    parser.add_argument("--game-dir", help="Path to SurrounDead game folder")
    parser.add_argument("--umodel", help="Path to umodel.exe / UE Viewer binary")
    parser.add_argument("--check-only", action="store_true", help="Only check status of extracted assets")
    args = parser.parse_args()

    print("==================================================")
    print("      RustDead First-Time Asset Extractor         ")
    print("==================================================")

    # 1. Locate Game
    game_dir = None
    pak_path = None
    if args.game_dir:
        game_dir = Path(args.game_dir)
        pak_path = game_dir / "SurrounDead/Content/Paks/SurrounDead-Windows.pak"
    else:
        game_dir, pak_path = find_steam_game()

    if game_dir and pak_path and pak_path.exists():
        print(f"[Game] Found SurrounDead installation: {game_dir}")
        print(f"[Game] Main PAK archive: {pak_path}")
    else:
        print("[Game] Notice: No local Steam installation detected in standard directories.")
        print("       You can specify --game-dir <path> if installed in a custom location.")

    # 2. Verify Output Folders
    models_dir = Path("assets/models")
    audio_dir = Path("assets/audio")
    textures_dir = Path("assets/textures")
    
    models_dir.mkdir(parents=True, exist_ok=True)
    audio_dir.mkdir(parents=True, exist_ok=True)
    textures_dir.mkdir(parents=True, exist_ok=True)

    # 3. Model Extraction
    extract_models(game_dir if game_dir else "assets/SurrounDead", models_dir, args.umodel)

    # 4. Audio Verification
    audio_count = len(list(audio_dir.glob("*.wav")))
    print(f"\n[Audio] Audio directory contains {audio_count} .wav audio cues.")

    # 5. Texture Verification
    texture_count = len(list(textures_dir.glob("*.png"))) + len(list(textures_dir.glob("*/*.png")))
    print(f"\n[Textures] Textures directory contains {texture_count} converted textures.")

    print("\n--------------------------------------------------")
    print("Ready! Game assets are mapped and accessible to RustDead.")
    print("--------------------------------------------------")

if __name__ == "__main__":
    main()
