use std::path::Path;
use unpak::Pak;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    let pak = Pak::new_any(&pak_path, None)?;
    let entries = pak.entries();

    let mut audio_files = Vec::new();
    let mut ui_files = Vec::new();
    let mut datatables = Vec::new();

    for entry in &entries {
        let lower = entry.to_lowercase();
        if lower.contains("audio") || lower.contains("sound") {
            audio_files.push(entry.clone());
        }
        if lower.contains("ui") || lower.contains("hud") || lower.contains("crosshair") || lower.contains("inventory/widgets") {
            ui_files.push(entry.clone());
        }
        if lower.contains("datatable") || lower.contains("dt_") || lower.contains("loottable") {
            datatables.push(entry.clone());
        }
    }

    println!("Total Audio files: {}", audio_files.len());
    for a in audio_files.iter().take(25) {
        println!("  Audio: {}", a);
    }

    println!("\nTotal UI / Widget files: {}", ui_files.len());
    for u in ui_files.iter().take(25) {
        println!("  UI: {}", u);
    }

    println!("\nTotal LootTables & DataTables: {}", datatables.len());
    for dt in datatables.iter().take(30) {
        println!("  DT: {}", dt);
    }

    Ok(())
}
