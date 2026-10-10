use std::fs;
use std::path::Path;
use unpak::Pak;
use serde_json::json;

fn extract_ascii_strings(bytes: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = Vec::new();
    for &b in bytes {
        if b >= 32 && b <= 126 {
            current.push(b);
        } else {
            if current.len() >= 3 {
                if let Ok(s) = String::from_utf8(current.clone()) {
                    strings.push(s);
                }
            }
            current.clear();
        }
    }
    strings
}

fn determine_type(name: &str, path: &str) -> &'static str {
    let lower_path = path.to_lowercase();
    let lower_name = name.to_lowercase();
    if lower_path.contains("ammo") || lower_name.contains("ammunition") { "Ammo" }
    else if lower_path.contains("weapons/firearms") { "WeaponPrimary" }
    else if lower_path.contains("weapons/melee") { "WeaponMelee" }
    else if lower_path.contains("medical") { "Medical" }
    else if lower_path.contains("food") { "Food" }
    else if lower_path.contains("drink") { "Drink" }
    else if lower_path.contains("armor") { "ArmorVest" }
    else if lower_path.contains("helmet") { "ArmorHelmet" }
    else if lower_path.contains("backpack") { "Backpack" }
    else if lower_path.contains("attachments") { "Attachment" }
    else { "Material" }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    let pak = Pak::new_any(&pak_path, None)?;
    
    let json_data = fs::read_to_string("asset_paths.json")?;
    let paths: serde_json::Value = serde_json::from_str(&json_data)?;
    let items = paths["items"].as_array().unwrap();
    
    let mut generated_items = Vec::new();
    
    for item_path in items {
        let pkg_name = item_path.as_str().unwrap();
        let pak_internal_path = pkg_name.replace("/Game/", "SurrounDead/Content/");
        let uexp_path = format!("{}.uexp", pak_internal_path);
        
        if let Ok(bytes) = pak.get(&uexp_path) {
            let strings = extract_ascii_strings(&bytes);
            if strings.len() > 1 {
                let id = Path::new(pkg_name).file_name().unwrap().to_str().unwrap().to_string();
                // Find first string that doesn't look like a hash or internal name
                let mut name = strings[1].clone();
                for s in &strings[1..] {
                    if s.len() > 2 && s.len() < 40 && !s.contains("/") && !s.chars().all(|c| c.is_ascii_hexdigit()) {
                        name = s.clone();
                        break;
                    }
                }
                
                generated_items.push(json!({
                    "id": id,
                    "name": name,
                    "description": "",
                    "item_type": determine_type(&name, pkg_name),
                    "rarity": "Common",
                    "weight_kg": 1.0,
                    "width": 1,
                    "height": 1,
                    "stack_size": 1,
                    "max_stack": 10
                }));
            }
        }
    }
    
    fs::write("surroundead_items.json", serde_json::to_string_pretty(&generated_items)?)?;
    println!("Generated {} items into surroundead_items.json", generated_items.len());
    Ok(())
}
