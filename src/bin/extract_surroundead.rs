use std::path::Path;
use unpak::Pak;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    let pak = Pak::new_any(&pak_path, None)?;
    let entries = pak.entries();

    println!("Total entries: {}", entries.len());

    let mut pngs = Vec::new();
    let mut inis = Vec::new();
    let mut datatables = Vec::new();
    let mut weapons = Vec::new();
    let mut zombies = Vec::new();

    for entry in &entries {
        let lower = entry.to_lowercase();
        if lower.starts_with("surroundead") {
            if lower.ends_with(".png") {
                pngs.push(entry.clone());
            } else if lower.ends_with(".ini") {
                inis.push(entry.clone());
            } else if lower.contains("datatable") || lower.contains("itemdata") || lower.contains("dt_") {
                datatables.push(entry.clone());
            } else if lower.contains("weapon") || lower.contains("firearm") {
                weapons.push(entry.clone());
            } else if lower.contains("zombie") {
                zombies.push(entry.clone());
            }
        }
    }

    println!("SurrounDead PNGs: {}", pngs.len());
    for p in pngs.iter().take(20) {
        println!("  PNG: {}", p);
    }

    println!("\nSurrounDead INIs: {}", inis.len());
    for i in &inis {
        println!("  INI: {}", i);
    }

    println!("\nSurrounDead DataTables/ItemData: {}", datatables.len());
    for dt in datatables.iter().take(30) {
        println!("  DT: {}", dt);
    }

    println!("\nSurrounDead Weapons sample: {}", weapons.len());
    for w in weapons.iter().take(20) {
        println!("  Weapon: {}", w);
    }

    println!("\nSurrounDead Zombies sample: {}", zombies.len());
    for z in zombies.iter().take(20) {
        println!("  Zombie: {}", z);
    }

    Ok(())
}
