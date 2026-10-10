use std::collections::HashMap;
use std::path::Path;
use unpak::Pak;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    let pak = Pak::new_any(&pak_path, None)?;
    let entries = pak.entries();

    let mut ext_counts: HashMap<String, usize> = HashMap::new();
    for entry in &entries {
        let ext = Path::new(entry)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("no_ext")
            .to_lowercase();
        *ext_counts.entry(ext).or_default() += 1;
    }

    println!("Extension counts in SurrounDead pak:");
    let mut sorted_exts: Vec<_> = ext_counts.into_iter().collect();
    sorted_exts.sort_by(|a, b| b.1.cmp(&a.1));
    for (ext, count) in sorted_exts {
        println!("  .{}: {}", ext, count);
    }

    println!("\nSearching for audio, UI, and icons:");
    for entry in &entries {
        let lower = entry.to_lowercase();
        if lower.contains("icon") || lower.contains("audio") || lower.contains(".png") || lower.contains("ui") {
            if lower.ends_with(".png") || lower.ends_with(".wav") || lower.ends_with(".ogg") {
                println!("  Direct asset: {}", entry);
            }
        }
    }

    Ok(())
}
