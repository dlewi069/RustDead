use std::path::Path;
use unpak::Pak;

fn extract_strings(bytes: &[u8]) -> Vec<String> {
    let mut res = Vec::new();
    let mut cur = Vec::new();
    for &b in bytes {
        if b >= 32 && b <= 126 {
            cur.push(b);
        } else {
            if cur.len() >= 3 {
                if let Ok(s) = String::from_utf8(cur.clone()) {
                    res.push(s);
                }
            }
            cur.clear();
        }
    }
    res
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    let pak = Pak::new_any(&pak_path, None)?;

    let target_tables = [
        "SurrounDead/Content/Blueprints/Vehicles/Spawner/DT_VehicleInfo",
        "SurrounDead/Content/Blueprints/HordeSystem/DT_HordeSystem",
        "SurrounDead/Content/Inventory/Traders/DT_VendorTable",
    ];

    for base in &target_tables {
        let uasset_entry = format!("{}.uasset", base);
        let uexp_entry = format!("{}.uexp", base);

        println!("==================================================");
        println!("DATA TABLE: {}", base);
        println!("==================================================");

        if let Ok(uasset_bytes) = pak.get(&uasset_entry) {
            let ustrings = extract_strings(&uasset_bytes);
            println!("-- UASSET Strings ({} bytes, {} strings):", uasset_bytes.len(), ustrings.len());
            for s in ustrings.iter().take(20) {
                println!("   {}", s);
            }
        }

        if let Ok(uexp_bytes) = pak.get(&uexp_entry) {
            let ustrings = extract_strings(&uexp_bytes);
            println!("-- UEXP Strings ({} bytes, {} strings):", uexp_bytes.len(), ustrings.len());
            let unique_strings: Vec<_> = ustrings.into_iter().filter(|s| s.len() > 3).collect();
            for s in unique_strings.iter().take(40) {
                println!("   {}", s);
            }
            
            let mut floats = Vec::new();
            for i in (0..uexp_bytes.len().saturating_sub(7)).step_by(4) {
                let f = f32::from_le_bytes(uexp_bytes[i..i+4].try_into().unwrap());
                if f.is_finite() && f > 0.05 && f <= 10000.0 && !f.is_nan() {
                    let rounded = (f * 100.0).round() / 100.0;
                    if !floats.contains(&rounded) {
                        floats.push(rounded);
                    }
                }
            }
            println!("-- Extracted Float Constants: {:?}", floats.iter().take(15).collect::<Vec<_>>());
        }
    }
    Ok(())
}
