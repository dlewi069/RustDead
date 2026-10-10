use std::path::Path;
use unpak::Pak;

fn main() {
    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    let pak = Pak::new_any(&pak_path, None).unwrap();
    let uasset = pak.get("SurrounDead/Content/Inventory/Items/DataAssets/Attachments/Ammo/DA_12GaugeAmmo.uasset").unwrap();
    let uexp = pak.get("SurrounDead/Content/Inventory/Items/DataAssets/Attachments/Ammo/DA_12GaugeAmmo.uexp").unwrap();
    
    let mut strings = Vec::new();
    let mut current = Vec::new();
    for &b in &uasset {
        if b.is_ascii_graphic() || b == b' ' {
            current.push(b);
        } else {
            if current.len() >= 4 {
                strings.push(String::from_utf8_lossy(&current).into_owned());
            }
            current.clear();
        }
    }
    for s in strings.iter().take(20) {
        println!("uasset: {}", s);
    }
}
