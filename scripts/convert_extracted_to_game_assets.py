"""
Converts authentic 3D meshes extracted from SurrounDead PAK into assets/models/*.obj
Preserves original vertex positions, normals, and texture UV coordinates.
"""

import os
import struct
import json
from pathlib import Path

def glb_to_obj(glb_path, obj_path):
    with open(glb_path, 'rb') as f:
        magic, ver, total_len = struct.unpack('<4sII', f.read(12))
        chunk0_len, chunk0_type = struct.unpack('<I4s', f.read(8))
        gltf = json.loads(f.read(chunk0_len).decode('utf-8'))
        chunk1_len, chunk1_type = struct.unpack('<I4s', f.read(8))
        bin_data = f.read(chunk1_len)

    def get_buffer_data(acc_idx):
        acc = gltf['accessors'][acc_idx]
        bv = gltf['bufferViews'][acc['bufferView']]
        offset = bv.get('byteOffset', 0) + acc.get('byteOffset', 0)
        count = acc['count']
        comp_type = acc['componentType'] # 5126=FLOAT, 5123=UNSIGNED_SHORT, 5125=UNSIGNED_INT, 5121=UNSIGNED_BYTE
        type_str = acc['type'] # VEC3, VEC2, SCALAR
        
        type_counts = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4}
        num_elems = type_counts[type_str]
        
        fmt_map = {5126: 'f', 5123: 'H', 5125: 'I', 5121: 'B'}
        elem_fmt = fmt_map[comp_type]
        stride = bv.get('byteStride', num_elems * struct.calcsize(elem_fmt))
        
        result = []
        for i in range(count):
            curr_pos = offset + i * stride
            vals = struct.unpack_from('<' + elem_fmt * num_elems, bin_data, curr_pos)
            result.append(vals if num_elems > 1 else vals[0])
        return result

    lines = [f'# Authentic asset extracted from SurrounDead: {Path(glb_path).name}']
    
    total_verts = 0
    total_faces = 0

    mesh0 = gltf['meshes'][0]
    for prim in mesh0['primitives']:
        pos_idx = prim['attributes']['POSITION']
        uv_idx = prim['attributes'].get('TEXCOORD_0')
        idx_accessor = prim.get('indices')
        
        positions = get_buffer_data(pos_idx)
        uvs = get_buffer_data(uv_idx) if uv_idx is not None else []
        indices = get_buffer_data(idx_accessor) if idx_accessor is not None else list(range(len(positions)))
        
        base_v = total_verts
        
        for p in positions:
            lines.append(f'v {p[0]:.4f} {p[1]:.4f} {p[2]:.4f}')
        for uv in uvs:
            lines.append(f'vt {uv[0]:.4f} {1.0 - uv[1]:.4f}')
            
        for i in range(0, len(indices), 3):
            i0 = base_v + indices[i] + 1
            i1 = base_v + indices[i+1] + 1
            i2 = base_v + indices[i+2] + 1
            if uvs:
                lines.append(f'f {i0}/{i0} {i1}/{i1} {i2}/{i2}')
            else:
                lines.append(f'f {i0} {i1} {i2}')

        total_verts += len(positions)
        total_faces += len(indices) // 3

    os.makedirs(os.path.dirname(obj_path), exist_ok=True)
    with open(obj_path, 'w') as out:
        out.write('\n'.join(lines) + '\n')
    
    size_kb = os.path.getsize(obj_path) / 1024
    print(f'  [OK] Converted {Path(glb_path).name:32} -> {Path(obj_path).name:28} ({total_verts} verts, {total_faces} faces, {size_kb:.1f} KB)')

def main():
    print('=================================================================')
    print('   Converting Authentic SurrounDead Meshes into Engine Assets    ')
    print('=================================================================')

    base_ext = Path('assets/extracted/SurrounDead/Content')
    dest = Path('assets/models')

    mapping = {
        # Player & Characters
        'SM_Char_Player_Male.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Characters/SK_Chr_Soldier_Male_01.glb',
        'SM_Char_Hunter.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Characters/SK_Chr_Hunter_Male_01.glb',
        'SM_Char_Criminal.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Characters/SK_Chr_Criminal_Male_01.glb',

        # Zombies
        'SM_Zombie_Walker.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Characters/SK_Chr_Zombie_Male_01.glb',
        'SM_Zombie_Runner.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Characters/SK_Chr_Zombie_Female_01.glb',
        'SM_Zombie_Juggernaut.obj': base_ext / 'PolygonFiles/PolygonBossZombies/Meshes/Characters/SK_Chr_ZombieBoss_Brute_01.glb',

        # Vehicles
        'SM_Veh_Sedan_01.obj': base_ext / 'Meshes/World/VehicleWrecks/SedanWreck.glb',
        'SM_Veh_Pickup_01.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Vehicles/SK_Veh_Ute_01.glb',
        'SM_Veh_Ambulance_01.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Vehicles/SK_Veh_Ambulance_01.glb',

        # Containers & Props
        'SM_Prop_Crate_Military.obj': base_ext / 'Meshes/Container/Mesh_IndustrialCrates.glb',
        'SM_Prop_Safe_01.obj': base_ext / 'Meshes/Container/Mesh_FirearmsLocker.glb',
        'SM_Prop_Barrier_01.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Props/SM_Prop_Barricade_05.glb',
        'SM_Prop_Dumpster_01.obj': base_ext / 'Meshes/Container/Mesh_Dumpster.glb',
        'SM_Prop_GasPump_01.obj': base_ext / 'Meshes/Container/Mesh_GasPump.glb',

        # Environment & Nature
        'SM_Env_PineTree_01.obj': base_ext / 'PolygonFiles/PolygonNature/Meshes/Trees/SM_Tree_Pine_01.glb',
        'SM_Env_PineTree_02.obj': base_ext / 'PolygonFiles/PolygonNature/Meshes/Trees/SM_Tree_Pine_Large_01.glb',
        'SM_Env_Boulder_01.obj': base_ext / 'Meshes/World/Resource_Rock_01.glb',
        'SM_Env_Road_01.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Environments/SM_Env_RoadPiece_Damaged_01.glb',

        # Weapons
        'SM_Wep_Rifle_AK47.obj': base_ext / 'Meshes/SK_Wep_Rifle_Assault_01.glb',
        'SM_Wep_Pistol_Glock.obj': base_ext / 'Meshes/Firearms/Damaged/M9/SK_Wep_Pistol_Metal_01_Damaged.glb',
        'SM_Wep_Shotgun_870.obj': base_ext / 'PolygonFiles/PolygonMilitary/Meshes/Weapons/SK_Wep_Shotgun_01.glb',
        'SM_Wep_Sniper_Barret.obj': base_ext / 'Meshes/Firearms/Damaged/R700/SK_Wep_Sniper_Mil_01_Damaged.glb',
        'SM_Wep_FireAxe_01.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Weapons/SM_Wep_FireAxe_01.glb',
        'SM_Wep_Bat_01.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Weapons/SM_Wep_Bat_Wood_01.glb',
        'SM_Wep_Knife_01.obj': base_ext / 'PolygonFiles/PolygonApocalypse/Meshes/Weapons/SM_Wep_Melee_HuntingKnife_01.glb',
    }

    for obj_name, glb_path in mapping.items():
        if glb_path.exists():
            glb_to_obj(str(glb_path), str(dest / obj_name))
        else:
            print(f'  [MISSING] {glb_path}')

    print('\n[OK] All authentic game models converted and placed in assets/models/!')

if __name__ == '__main__':
    main()
