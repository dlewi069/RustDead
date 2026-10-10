use std::path::Path;
use unpak::Pak;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pak_path = Path::new("/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead/Content/Paks/SurrounDead-Windows.pak");
    let pak = Pak::new_any(&pak_path, None)?;

    let target = "SurrounDead/Config/DefaultGame.ini";
    match pak.get(target) {
        Ok(data) => {
            let s = String::from_utf8_lossy(&data);
            println!("--- {} ---\n{}", target, s);
        }
        Err(e) => println!("Error: {:?}", e),
    }

    Ok(())
}
