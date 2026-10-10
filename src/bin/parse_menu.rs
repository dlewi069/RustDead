use std::path::Path;
use unpak::Pak;

fn extract_ascii_strings(bytes: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = Vec::new();
    for &b in bytes {
        if b.is_ascii_graphic() || b == b' ' {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    let pak = Pak::new_any(&pak_path, None)?;
    
    let uexp_path = "SurrounDead/Content/UI/Menus/Menu/CharacterCreatorMenu.uexp";
    
    if let Ok(bytes) = pak.get(uexp_path) {
        let strings = extract_ascii_strings(&bytes);
        println!("--- CharacterCreatorMenu.uexp Strings ---");
        for s in strings {
            if s.len() > 3 && !s.contains("/") && !s.chars().all(|c| c.is_ascii_hexdigit()) {
                println!("{}", s);
            }
        }
    } else {
        println!("File not found in pak");
    }
    
    let menu_path = "SurrounDead/Content/UI/Menus/Menu/MenuWidget.uexp";
    if let Ok(bytes) = pak.get(menu_path) {
        let strings = extract_ascii_strings(&bytes);
        println!("--- MenuWidget.uexp Strings ---");
        for s in strings {
            if s.len() > 3 && !s.contains("/") && !s.chars().all(|c| c.is_ascii_hexdigit()) {
                println!("{}", s);
            }
        }
    }
    Ok(())
}
