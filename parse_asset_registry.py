import json
import sys

def main():
    try:
        with open('/ntfs/SurrounDead/AssetRegistry.json', 'r') as f:
            data = json.load(f)
        
        items = []
        sounds = []
        textures = []
        
        for asset in data.get('PreallocatedAssetDataBuffers', []):
            cls = asset.get('AssetClass')
            path = asset.get('PackageName')
            if not path:
                continue
                
            if cls == 'JigsawItem_DataAsset_C':
                items.append(path)
            elif cls in ['SoundWave', 'SoundCue']:
                sounds.append(path)
            elif cls == 'Texture2D':
                textures.append(path)
        
        print(f"Items: {len(items)}")
        print(f"Sounds: {len(sounds)}")
        print(f"Textures: {len(textures)}")
        
        with open('asset_paths.json', 'w') as f:
            json.dump({
                'items': items,
                'sounds': sounds,
                'textures': textures
            }, f, indent=2)
            
    except Exception as e:
        print(f"Error: {e}")

main()
