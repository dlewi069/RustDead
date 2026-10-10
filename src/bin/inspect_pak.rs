use std::path::Path;
use unpak::Pak;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    println!("Checking pak file: {:?}", pak_path);
    if !pak_path.exists() {
        eprintln!("Pak file does not exist!");
        return Ok(());
    }

    let pak = Pak::new_any(&pak_path, None)?;
    println!("Pak version: {:?}", pak.version());
    println!("Mount point: {}", pak.mount_point());
    let entries = pak.entries();
    println!("Total entries in pak: {}", entries.len());

    println!("\nSample 50 entries:");
    for entry in entries.iter().take(50) {
        println!("  {}", entry);
    }

    Ok(())
}
