use std::path::Path;
use unpak::Pak;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    let pak = Pak::new_any(&pak_path, None)?;
    
    let entries = pak.entries();
    
    println!("Vehicle files:");
    for e in entries.iter().filter(|k| k.contains("Vehicles") && k.contains("DT_")) {
        println!("{}", e);
    }
    
    println!("Horde files:");
    for e in entries.iter().filter(|k| k.contains("Horde") && k.contains("DT_")) {
        println!("{}", e);
    }

    println!("Vendor files:");
    for e in entries.iter().filter(|k| k.contains("Trader") && k.contains("DT_")) {
        println!("{}", e);
    }
    Ok(())
}
