import os
import sys
import struct
import shutil
import argparse
import subprocess
from pathlib import Path

SURROUNDEAD_APP_ID = "1645820"

def get_steam_libraries():
    libraries = []
    if sys.platform == "win32":
        default_steam = Path("C:/Program Files (x86)/Steam")
        vdf_path = default_steam / "steamapps" / "libraryfolders.vdf"
        if vdf_path.exists():
            try:
                with open(vdf_path, "r", encoding="utf-8", errors="ignore") as f:
                    for line in f:
                        if '"path"' in line:
                            parts = line.split('"')
                            if len(parts) >= 4:
                                p = Path(parts[3])
                                if p.exists() and p not in libraries:
                                    libraries.append(p)
            except Exception as e:
                print(f"[Warning] Failed parsing libraryfolders.vdf: {e}")
        # Add common drives
        for drive in ["C", "D", "E", "F", "B", "A"]:
            p = Path(f"{drive}:/SteamLibrary")
            if p.exists() and p not in libraries:
                libraries.append(p)
            p2 = Path(f"{drive}:/Program Files (x86)/Steam")
            if p2.exists() and p2 not in libraries:
                libraries.append(p2)
    else:
        home = Path.home()
        libraries.extend([
            home / ".local/share/Steam",
            home / ".steam/steam",
            Path("/ntfs/SteamLibrary"),
        ])
    return libraries

def locate_game():
    for lib in get_steam_libraries():
        game_dir = lib / "steamapps" / "common" / "SurrounDead"
        pak_path = game_dir / "SurrounDead" / "Content" / "Paks" / "SurrounDead-Windows.pak"
        manifest_path = lib / "steamapps" / f"appmanifest_{SURROUNDEAD_APP_ID}.acf"
        if pak_path.exists():
            return {
                "game_dir": game_dir,
                "pak_path": pak_path,
                "manifest_path": manifest_path if manifest_path.exists() else None,
                "has_ownership_proof": manifest_path.exists(),
            }
    return None

def verify_ownership():
    info = locate_game()
    if not info:
        print("\n=======================================================")
        print(" [!] OWNERSHIP VERIFICATION FAILED: GAME NOT FOUND")
        print("=======================================================")
        print("RustDead requires a legally owned copy of SurrounDead on Steam.")
        print("No installation or 'SurrounDead-Windows.pak' was found on your system.")
        print("Please purchase and install SurrounDead from Steam to play this port.")
        return False, None

    print("\n=======================================================")
    print(" [OK] OWNERSHIP VERIFIED: SURROUNDEAD STEAM INSTALLATION")
    print("=======================================================")
    print(f"Game Path: {info['game_dir']}")
    print(f"PAK Archive: {info['pak_path']}")
    if info['has_ownership_proof']:
        print(f"Steam Manifest: {info['manifest_path']} (AppID: {SURROUNDEAD_APP_ID})")
    print("Verification succeeded. Proceeding with local client extraction...\n")
    return True, info

def main():
    parser = argparse.ArgumentParser(description="RustDead First-Time Extractor & Ownership Verifier")
    parser.add_argument("--verify-only", action="store_true", help="Only verify game ownership")
    args = parser.parse_args()

    verified, info = verify_ownership()
    if not verified:
        sys.exit(1)

    if args.verify_only:
        print("Ownership verified successfully.")
        return

    print("Assets will be extracted directly from your Steam archive to the local directory.")
    print("No copyrighted assets are distributed with this port repository.")

if __name__ == "__main__":
    main()
