use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Model names mapped between game entities and .obj file names
pub const REQUIRED_MODELS: &[&str] = &[
    "SM_Char_Player_Male.obj",
    "SM_Wep_Rifle_AK47.obj",
    "SM_Wep_Pistol_Glock.obj",
    "SM_Wep_Shotgun_870.obj",
    "SM_Wep_Sniper_Barret.obj",
    "SM_Zombie_Walker.obj",
    "SM_Zombie_Runner.obj",
    "SM_Zombie_Juggernaut.obj",
    "SM_Veh_Sedan_01.obj",
    "SM_Veh_Pickup_01.obj",
    "SM_Prop_Safe_01.obj",
    "SM_Prop_Crate_Military.obj",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("==================================================");
    println!("      RustDead First-Time Setup & Extractor       ");
    println!("==================================================");

    // 1. Check or prepare target asset directories
    let models_dir = Path::new("assets/models");
    let audio_dir = Path::new("assets/audio");
    let textures_dir = Path::new("assets/textures");

    fs::create_dir_all(models_dir)?;
    fs::create_dir_all(audio_dir)?;
    fs::create_dir_all(textures_dir)?;

    // 2. Locate Steam Installation of SurrounDead
    match find_steam_install() {
        Ok(steam_path) => {
            println!("[Steam] Located Steam library at: {}", steam_path.display());
            let game_dir = steam_path.join("steamapps/common/SurrounDead");
            let pak_path = game_dir.join("SurrounDead/Content/Paks/SurrounDead-Windows.pak");
            if pak_path.exists() {
                println!("[Game] Found authentic game archive: {}", pak_path.display());
                try_extract_assets(&game_dir, &pak_path, models_dir)?;
            } else {
                println!("[Game] SurrounDead PAK not found in standard steamapps path: {}", pak_path.display());
            }
        }
        Err(e) => {
            println!("[Steam] Notice: {}", e);
            println!("        Ensure SurrounDead is installed or specify the directory manually.");
        }
    }

    // 3. Inspect existing models
    let mut missing = Vec::new();
    for &model in REQUIRED_MODELS {
        let p = models_dir.join(model);
        if !p.exists() {
            missing.push(model);
        }
    }

    if missing.is_empty() {
        println!("\n[Status] All {} required 3D model entries are present in {}!", REQUIRED_MODELS.len(), models_dir.display());
    } else {
        println!("\n[Status] {} model files missing in {}:", missing.len(), models_dir.display());
        for m in &missing {
            println!("  - Missing: {}", m);
        }
        println!("Populating placeholder geometry for missing models so the engine starts cleanly...");
        for m in missing {
            create_dummy_obj(&models_dir.join(m))?;
        }
    }

    println!("\n==================================================");
    println!("Setup completed! Models and assets are ready.");
    println!("==================================================");
    Ok(())
}

fn try_extract_assets(game_dir: &Path, pak_path: &Path, out_models: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let umodel_candidates = [
        PathBuf::from("tools/umodel.exe"),
        PathBuf::from("tools/umodel"),
        PathBuf::from("umodel.exe"),
    ];

    for umodel in &umodel_candidates {
        if umodel.exists() {
            println!("[Extractor] Found UE Viewer at: {}. Extracting models to {}...", umodel.display(), out_models.display());
            let status = Command::new(umodel)
                .arg("-export")
                .arg("-obj")
                .arg(format!("-out={}", out_models.display()))
                .arg(pak_path)
                .status()?;
            if status.success() {
                println!("[Extractor] Successfully exported meshes from PAK!");
                return Ok(());
            }
        }
    }

    // Also run python helper if present
    let script_path = Path::new("scripts/run_setup_extractor.py");
    if script_path.exists() {
        let _ = Command::new("python")
            .arg(script_path)
            .arg("--game-dir")
            .arg(game_dir)
            .status();
    }

    Ok(())
}

fn create_dummy_obj(path: &Path) -> Result<(), std::io::Error> {
    let dummy = "# Placeholder dummy cube\nv -0.5 -0.5 0.5\nv 0.5 -0.5 0.5\nv -0.5 0.5 0.5\nv 0.5 0.5 0.5\nv -0.5 0.5 -0.5\nv 0.5 0.5 -0.5\nv -0.5 -0.5 -0.5\nv 0.5 -0.5 -0.5\nf 1 2 4 3\nf 3 4 6 5\nf 5 6 8 7\nf 7 8 2 1\nf 2 8 6 4\nf 7 1 3 5\n";
    fs::write(path, dummy)
}

fn find_steam_install() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let drives = ["C", "D", "E", "F", "G"];
        for d in drives {
            let paths = [
                format!("{}:\\Program Files (x86)\\Steam", d),
                format!("{}:\\Program Files\\Steam", d),
                format!("{}:\\Steam", d),
                format!("{}:\\SteamLibrary", d),
            ];
            for p_str in paths {
                let p = PathBuf::from(p_str);
                if p.exists() {
                    return Ok(p);
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let candidates = [
                PathBuf::from(&home).join(".local/share/Steam"),
                PathBuf::from(&home).join(".steam/steam"),
                PathBuf::from("/ntfs/SteamLibrary"),
            ];
            for c in candidates {
                if c.exists() {
                    return Ok(c);
                }
            }
        }
    }

    Err("Steam installation directory not found automatically.".into())
}
