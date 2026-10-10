use std::fs;
use std::path::Path;

#[allow(dead_code)]
fn extract_f64_near(bytes: &[u8], pattern: &[u8]) -> Option<f64> {
    if let Some(pos) = bytes.windows(pattern.len()).position(|w| w == pattern) {
        // Look within next 64 bytes for f64 values
        let search_start = pos + pattern.len();
        let search_end = (search_start + 80).min(bytes.len());
        for i in search_start..search_end.saturating_sub(7) {
            let val = f64::from_le_bytes(bytes[i..i+8].try_into().unwrap());
            if val.is_finite() && val > 0.0 && val < 5000.0 {
                return Some(val);
            }
        }
    }
    None
}

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

fn main() {
    let base_dir = Path::new("assets/SurrounDead/Content/Inventory/Items/DataAssets/Weapons/Firearms");
    let mut entries = Vec::new();

    fn walk_dir(dir: &Path, list: &mut Vec<std::path::PathBuf>) {
        if let Ok(rd) = fs::read_dir(dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_dir(&path, list);
                } else if path.extension().and_then(|s| s.to_str()) == Some("uasset") {
                    list.push(path);
                }
            }
        }
    }

    walk_dir(base_dir, &mut entries);
    entries.sort();

    println!("Found {} firearms:", entries.len());

    for path in entries {
        let uexp_path = path.with_extension("uexp");
        let uasset_bytes = fs::read(&path).unwrap_or_default();
        let uexp_bytes = fs::read(&uexp_path).unwrap_or_default();

        let uasset_strings = extract_ascii_strings(&uasset_bytes);
        let uexp_strings = extract_ascii_strings(&uexp_bytes);

        let file_stem = path.file_stem().unwrap().to_string_lossy();

        // Extract caliber from uasset strings
        let calibers = ["7.62mm", "5.56mm", "9mm", ".45 ACP", ".45ACP", "12 Gauge", ".308", ".300 Win", ".50 Cal", "40mm", "66mm", "5.7mm", "4.6mm"];
        let found_cal = calibers.iter().find(|&&c| uasset_strings.iter().any(|s| s.contains(c))).copied().unwrap_or("Unknown");

        // Look for display name
        let display_name = uexp_strings.iter().find(|s| s.len() > 2 && s.len() < 30 && !s.contains('/') && !s.contains("000") && !s.contains("Game") && s.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '.' || c == '/')).cloned().unwrap_or_else(|| file_stem.to_string());

        // Extract fire modes
        let firemodes = ["Semi, Auto", "Semi", "Auto", "Burst", "Pump", "Bolt Action", "Lever Action"];
        let found_fm = firemodes.iter().find(|&&f| uasset_strings.iter().any(|s| s == f) || uexp_strings.iter().any(|s| s == f)).copied().unwrap_or("Semi");

        // Extract numbers from uexp (damage, rpm)
        let mut floats = Vec::new();
        for i in (0..uexp_bytes.len().saturating_sub(7)).step_by(1) {
            let val = f64::from_le_bytes(uexp_bytes[i..i+8].try_into().unwrap());
            if val.is_finite() && val >= 5.0 && val <= 1200.0 && val.fract() == 0.0 {
                if !floats.contains(&(val as i64)) {
                    floats.push(val as i64);
                }
            }
        }

        println!("• {:<20} | Display: {:<20} | Caliber: {:<10} | Modes: {:<12} | Values: {:?}", 
            file_stem, display_name, found_cal, found_fm, floats);
    }
}
