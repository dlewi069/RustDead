use std::fs::File;
use std::path::Path;
use std::io::{Cursor, Read};
use unreal_asset::{Asset, engine_version::EngineVersion};
use unreal_asset::unversioned::Usmap;
use unpak::Pak;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading Usmap...");
    let mut usmap_file = File::open("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Binaries/Win64/Mappings.usmap")?;
    let mut usmap_data = Vec::new();
    usmap_file.read_to_end(&mut usmap_data)?;
    let usmap_cursor = Cursor::new(usmap_data);
    
    let usmap = Usmap::new(usmap_cursor)?;
    println!("Loaded Usmap! {} names", usmap.name_map.len());
    // (using len() on something else if get_name_map_index_list doesn't exist, I'll just print it worked)

    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    let pak = Pak::new_any(&pak_path, None)?;

    let test_uasset = "SurrounDead/Content/Weapons/M16/M16_Rifle.uasset";
    let test_uexp = "SurrounDead/Content/Weapons/M16/M16_Rifle.uexp";

    if let (Ok(uasset_data), Ok(uexp_data)) = (pak.get(test_uasset), pak.get(test_uexp)) {
        let uasset_cursor = Cursor::new(uasset_data);
        let uexp_cursor = Cursor::new(uexp_data);
        
        println!("Creating Asset...");
        let asset = Asset::new(uasset_cursor, Some(uexp_cursor), EngineVersion::VER_UE5_2);
        match asset {
            Ok(mut a) => {
                a.asset_data.mappings = Some(usmap);
                println!("Loaded asset with {} exports", a.asset_data.exports.len());
            },
            Err(e) => {
                println!("Error loading asset: {:?}", e);
            }
        }
    } else {
        println!("Could not find test uasset/uexp in pak.");
    }

    Ok(())
}
