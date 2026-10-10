use std::fs;
use std::path::Path;

fn extract_ascii_strings(bytes: &[u8], min_len: usize) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = Vec::new();

    for &b in bytes {
        if b >= 32 && b <= 126 {
            current.push(b);
        } else {
            if current.len() >= min_len {
                if let Ok(s) = String::from_utf8(current.clone()) {
                    strings.push(s);
                }
            }
            current.clear();
        }
    }

    if current.len() >= min_len {
        if let Ok(s) = String::from_utf8(current) {
            strings.push(s);
        }
    }

    strings
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tables = [
        "assets/SurrounDead/Content/Inventory/Containers/LootTables/DA_LootTable_AirdropMilitary.uasset",
        "assets/SurrounDead/Content/Inventory/Containers/LootTables/DA_LootTable_MilitaryHighTier.uasset",
        "assets/SurrounDead/Content/Inventory/Containers/LootTables/DA_LootTable_Safe.uasset",
        "assets/SurrounDead/Content/Inventory/Containers/LootTables/DA_LootTable_Keycards.uasset",
    ];

    for table in &tables {
        println!("==================================================");
        println!("TABLE: {}", table);
        println!("==================================================");
        let path = Path::new(table);
        if !path.exists() {
            println!("Not found: {:?}", path);
            continue;
        }

        let bytes = fs::read(path)?;
        let strings = extract_ascii_strings(&bytes, 4);
        let mut relevant: Vec<_> = strings
            .into_iter()
            .filter(|s| {
                s.starts_with("DA_")
                    || s.starts_with("BP_")
                    || s.contains("Weapon")
                    || s.contains("Ammo")
                    || s.contains("Gun")
                    || s.contains("Rifle")
                    || s.contains("Keycard")
                    || s.contains("Armor")
                    || s.contains("Vest")
                    || s.contains("Helmet")
                    || s.contains("Backpack")
                    || s.contains("Bag")
                    || s.contains("Medical")
                    || s.contains("M4")
                    || s.contains("AK")
                    || s.contains("Scar")
            })
            .collect();
        relevant.sort();
        relevant.dedup();

        for s in relevant {
            println!("  {}", s);
        }
    }

    Ok(())
}
