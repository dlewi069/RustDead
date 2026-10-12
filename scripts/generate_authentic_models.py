"""
Authentic Low-Poly Model Generator for RustDead
Generates detailed, recognizable Synty-style low-poly OBJ 3D meshes with
correct geometry, proportions, and UV mapping aligned to the game's color atlases.
"""

import math
from pathlib import Path

class ObjBuilder:
    def __init__(self):
        self.vertices = []
        self.uvs = []
        self.normals = []
        self.faces = []

    def add_vertex(self, x, y, z):
        self.vertices.append((x, y, z))
        return len(self.vertices)

    def add_uv(self, u, v):
        self.uvs.append((u, v))
        return len(self.uvs)

    def add_box(self, cx, cy, cz, sx, sy, sz, uv=(0.5, 0.5), rot_y=0.0):
        """Add an oriented box with width sx, height sy, depth sz centered at cx, cy, cz."""
        hx, hy, hz = sx * 0.5, sy * 0.5, sz * 0.5
        cos_r = math.cos(rot_y)
        sin_r = math.sin(rot_y)

        def rotate(x, z):
            return x * cos_r - z * sin_r, x * sin_r + z * cos_r

        local_corners = [
            (-hx, -hy, -hz),
            ( hx, -hy, -hz),
            ( hx, -hy,  hz),
            (-hx, -hy,  hz),
            (-hx,  hy, -hz),
            ( hx,  hy, -hz),
            ( hx,  hy,  hz),
            (-hx,  hy,  hz),
        ]

        v_start = len(self.vertices) + 1
        uv_idx = self.add_uv(uv[0], uv[1])

        for lx, ly, lz in local_corners:
            rx, rz = rotate(lx, lz)
            self.vertices.append((cx + rx, cy + ly, cz + rz))

        # 6 faces (each split into two triangles)
        # Face quad indices: (v0, v1, v2, v3)
        faces_quads = [
            (0, 1, 5, 4), # Front (-Z)
            (1, 2, 6, 5), # Right (+X)
            (2, 3, 7, 6), # Back (+Z)
            (3, 0, 4, 7), # Left (-X)
            (4, 5, 6, 7), # Top (+Y)
            (3, 2, 1, 0), # Bottom (-Y)
        ]

        for q0, q1, q2, q3 in faces_quads:
            i0 = v_start + q0
            i1 = v_start + q1
            i2 = v_start + q2
            i3 = v_start + q3
            self.faces.append([(i0, uv_idx), (i1, uv_idx), (i2, uv_idx)])
            self.faces.append([(i0, uv_idx), (i2, uv_idx), (i3, uv_idx)])

    def add_cylinder(self, cx, cy, cz, radius, height, segments=8, uv=(0.5, 0.5), axis='y'):
        """Add a cylinder along axis 'x', 'y', or 'z'."""
        v_start = len(self.vertices) + 1
        uv_idx = self.add_uv(uv[0], uv[1])

        top_ring = []
        bot_ring = []
        hh = height * 0.5

        for i in range(segments):
            angle = (2.0 * math.pi * i) / segments
            c = math.cos(angle) * radius
            s = math.sin(angle) * radius

            if axis == 'y':
                bot_ring.append(self.add_vertex(cx + c, cy - hh, cz + s))
                top_ring.append(self.add_vertex(cx + c, cy + hh, cz + s))
            elif axis == 'x':
                bot_ring.append(self.add_vertex(cx - hh, cy + c, cz + s))
                top_ring.append(self.add_vertex(cx + hh, cy + c, cz + s))
            elif axis == 'z':
                bot_ring.append(self.add_vertex(cx + c, cy + s, cz - hh))
                top_ring.append(self.add_vertex(cx + c, cy + s, cz + hh))

        center_bot = self.add_vertex(
            cx - (hh if axis == 'x' else 0),
            cy - (hh if axis == 'y' else 0),
            cz - (hh if axis == 'z' else 0)
        )
        center_top = self.add_vertex(
            cx + (hh if axis == 'x' else 0),
            cy + (hh if axis == 'y' else 0),
            cz + (hh if axis == 'z' else 0)
        )

        for i in range(segments):
            next_i = (i + 1) % segments
            b0, b1 = bot_ring[i], bot_ring[next_i]
            t0, t1 = top_ring[i], top_ring[next_i]

            # Side quad
            self.faces.append([(b0, uv_idx), (b1, uv_idx), (t1, uv_idx)])
            self.faces.append([(b0, uv_idx), (t1, uv_idx), (t0, uv_idx)])

            # Bottom cap
            self.faces.append([(center_bot, uv_idx), (b1, uv_idx), (b0, uv_idx)])
            # Top cap
            self.faces.append([(center_top, uv_idx), (t0, uv_idx), (t1, uv_idx)])

    def add_cone(self, cx, cy, cz, radius, height, segments=8, uv=(0.5, 0.5)):
        """Add a cone tapering up to a peak."""
        uv_idx = self.add_uv(uv[0], uv[1])
        base_ring = []

        for i in range(segments):
            angle = (2.0 * math.pi * i) / segments
            x = cx + math.cos(angle) * radius
            z = cz + math.sin(angle) * radius
            base_ring.append(self.add_vertex(x, cy, z))

        base_center = self.add_vertex(cx, cy, cz)
        peak = self.add_vertex(cx, cy + height, cz)

        for i in range(segments):
            next_i = (i + 1) % segments
            b0, b1 = base_ring[i], base_ring[next_i]
            # Side triangle
            self.faces.append([(b0, uv_idx), (b1, uv_idx), (peak, uv_idx)])
            # Base triangle
            self.faces.append([(base_center, uv_idx), (b1, uv_idx), (b0, uv_idx)])

    def to_obj_string(self):
        lines = ["# Low-Poly Authentic Game Mesh", f"# Vertices: {len(self.vertices)}, Faces: {len(self.faces)}"]
        for v in self.vertices:
            lines.append(f"v {v[0]:.4f} {v[1]:.4f} {v[2]:.4f}")
        for uv in self.uvs:
            lines.append(f"vt {uv[0]:.4f} {uv[1]:.4f}")
        for f in self.faces:
            tokens = [f"{v_idx}/{vt_idx}" for (v_idx, vt_idx) in f]
            lines.append("f " + " ".join(tokens))
        return "\n".join(lines) + "\n"

def build_player_model():
    """Authentic Survivor character with tactical vest, arms, legs, boots, head, and bug-out backpack."""
    b = ObjBuilder()
    
    # UV mappings from military.png
    UV_FLESH = (0.0312, 0.3594)
    UV_VEST = (0.6562, 0.1328)      # Olive drab
    UV_PANTS = (0.6875, 0.5000)     # Tactical black/dark charcoal
    UV_BOOTS = (0.3000, 0.5000)     # Heavy boots
    UV_PACK = (0.3125, 0.1328)      # Brown rucksack
    UV_STEEL = (0.4375, 0.0625)     # Steel buckle / knife

    # 1. Boots & Feet (y: 0.0 to 0.3)
    b.add_box(-0.16, 0.12,  0.03, 0.16, 0.24, 0.28, uv=UV_BOOTS)
    b.add_box( 0.16, 0.12,  0.03, 0.16, 0.24, 0.28, uv=UV_BOOTS)

    # 2. Lower Legs & Knees (y: 0.24 to 0.65)
    b.add_box(-0.16, 0.45, 0.0, 0.14, 0.42, 0.16, uv=UV_PANTS)
    b.add_box( 0.16, 0.45, 0.0, 0.14, 0.42, 0.16, uv=UV_PANTS)
    # Knee pads
    b.add_box(-0.16, 0.45, 0.08, 0.12, 0.12, 0.04, uv=UV_BOOTS)
    b.add_box( 0.16, 0.45, 0.08, 0.12, 0.12, 0.04, uv=UV_BOOTS)

    # 3. Thighs & Hips (y: 0.65 to 1.05)
    b.add_box(-0.15, 0.82, 0.0, 0.16, 0.38, 0.18, uv=UV_PANTS)
    b.add_box( 0.15, 0.82, 0.0, 0.16, 0.38, 0.18, uv=UV_PANTS)
    # Pelvis
    b.add_box(0.0, 1.02, 0.0, 0.44, 0.14, 0.22, uv=UV_PANTS)

    # 4. Torso & Tactical Vest (y: 1.08 to 1.52)
    b.add_box(0.0, 1.28, 0.0, 0.48, 0.42, 0.26, uv=UV_VEST)
    # Chest magazine pouches
    b.add_box(-0.12, 1.25, 0.14, 0.10, 0.14, 0.06, uv=UV_PACK)
    b.add_box( 0.00, 1.25, 0.14, 0.10, 0.14, 0.06, uv=UV_PACK)
    b.add_box( 0.12, 1.25, 0.14, 0.10, 0.14, 0.06, uv=UV_PACK)
    # Radio pouch on shoulder
    b.add_box(-0.18, 1.44, 0.10, 0.08, 0.12, 0.05, uv=UV_BOOTS)

    # 5. Neck & Head (y: 1.50 to 1.82)
    b.add_box(0.0, 1.54, 0.0, 0.14, 0.10, 0.14, uv=UV_FLESH)
    # Head
    b.add_box(0.0, 1.68, 0.01, 0.24, 0.24, 0.24, uv=UV_FLESH)
    # Survivor Cap / Helmet
    b.add_box(0.0, 1.76, 0.01, 0.26, 0.10, 0.26, uv=UV_VEST)
    b.add_box(0.0, 1.74, 0.16, 0.22, 0.03, 0.10, uv=UV_VEST) # Visor

    # 6. Arms & Hands
    # Left Arm
    b.add_box(-0.31, 1.40, 0.0, 0.14, 0.24, 0.16, uv=UV_VEST)  # Shoulder
    b.add_box(-0.32, 1.18, 0.04, 0.12, 0.22, 0.14, uv=UV_FLESH) # Forearm
    b.add_box(-0.32, 0.98, 0.08, 0.10, 0.14, 0.12, uv=UV_BOOTS) # Glove
    # Right Arm
    b.add_box( 0.31, 1.40, 0.0, 0.14, 0.24, 0.16, uv=UV_VEST)  # Shoulder
    b.add_box( 0.32, 1.18, 0.04, 0.12, 0.22, 0.14, uv=UV_FLESH) # Forearm
    b.add_box( 0.32, 0.98, 0.08, 0.10, 0.14, 0.12, uv=UV_BOOTS) # Glove

    # 7. Bug-out Backpack on Back
    b.add_box(0.0, 1.30, -0.22, 0.38, 0.44, 0.20, uv=UV_PACK)
    # Bedroll on top of backpack
    b.add_cylinder(0.0, 1.56, -0.22, 0.09, 0.42, segments=8, uv=UV_VEST, axis='x')
    # Side pouches & canteen
    b.add_box(-0.21, 1.25, -0.20, 0.08, 0.22, 0.14, uv=UV_PACK)
    b.add_box( 0.21, 1.25, -0.20, 0.08, 0.22, 0.14, uv=UV_STEEL)

    return b.to_obj_string()

def build_zombie_model():
    """Authentic low-poly infected walker zombie with hunched posture, tattered clothes, and outstretched reaching arms."""
    b = ObjBuilder()

    UV_SKIN = (0.1641, 0.3594)     # Decaying infected greenish-grey skin
    UV_ROTTEN = (0.0312, 0.2891)   # Rotten dirty clothes
    UV_PANTS = (0.0938, 0.3203)    # Tattered jeans
    UV_BLOOD = (0.1406, 0.4766)    # Dark gore / dried blood

    # 1. Shuffling Feet & Legs (slightly crooked)
    b.add_box(-0.15, 0.10,  0.06, 0.14, 0.20, 0.26, uv=UV_SKIN)
    b.add_box( 0.15, 0.10, -0.04, 0.14, 0.20, 0.26, uv=UV_SKIN)
    b.add_box(-0.15, 0.42,  0.03, 0.14, 0.42, 0.16, uv=UV_PANTS)
    b.add_box( 0.15, 0.42, -0.02, 0.14, 0.42, 0.16, uv=UV_PANTS)
    b.add_box(-0.14, 0.78,  0.02, 0.15, 0.36, 0.18, uv=UV_PANTS)
    b.add_box( 0.14, 0.78, -0.01, 0.15, 0.36, 0.18, uv=UV_PANTS)

    # 2. Hunched Torso leaning forward
    b.add_box(0.0, 1.16, 0.06, 0.46, 0.44, 0.26, uv=UV_ROTTEN, rot_y=0.08)
    # Blood / chest tear
    b.add_box(0.06, 1.22, 0.18, 0.16, 0.20, 0.04, uv=UV_BLOOD)

    # 3. Twisted Neck & Head
    b.add_box(0.03, 1.46, 0.14, 0.14, 0.12, 0.14, uv=UV_SKIN)
    b.add_box(0.05, 1.62, 0.20, 0.24, 0.24, 0.24, uv=UV_SKIN, rot_y=0.15)
    # Open zombie jaw
    b.add_box(0.05, 1.54, 0.31, 0.18, 0.08, 0.10, uv=UV_BLOOD)

    # 4. Outstretched Reaching Zombie Arms (reaching forward along +Z)
    # Left Arm extending forward
    b.add_box(-0.28, 1.34, 0.18, 0.13, 0.14, 0.34, uv=UV_ROTTEN)
    b.add_box(-0.28, 1.32, 0.44, 0.11, 0.12, 0.32, uv=UV_SKIN)
    b.add_box(-0.28, 1.32, 0.63, 0.12, 0.08, 0.14, uv=UV_BLOOD) # Clawed hands

    # Right Arm reaching forward & slightly angled
    b.add_box( 0.28, 1.30, 0.16, 0.13, 0.14, 0.34, uv=UV_ROTTEN)
    b.add_box( 0.28, 1.28, 0.42, 0.11, 0.12, 0.32, uv=UV_SKIN)
    b.add_box( 0.28, 1.28, 0.61, 0.12, 0.08, 0.14, uv=UV_BLOOD)

    return b.to_obj_string()

def build_sedan_model():
    """Authentic low-poly 4-door survival sedan vehicle."""
    b = ObjBuilder()

    UV_BODY = (0.1562, 0.2812)     # Crimson / burgundy survivor car
    UV_TIRE = (0.0938, 0.3203)     # Dark rubber
    UV_RIMS = (0.1328, 0.1484)     # Silver / chrome
    UV_GLASS = (0.1328, 0.1484)    # Tinted glass
    UV_HEADLIGHT = (0.1328, 0.2422) # Light gold/yellow
    UV_TAILLIGHT = (0.1562, 0.2812) # Red brake lights
    UV_GRILL = (0.0938, 0.3203)    # Black radiator grill

    # Dimensions: Length 4.4m (Z: -2.2 to +2.2), Width 1.9m (X: -0.95 to +0.95), Height 1.45m

    # 1. 4 Cylindrical Wheels & Rims (radius 0.36, width 0.26)
    wheel_positions = [
        (-0.92, 0.36,  1.30), # Front Left
        ( 0.92, 0.36,  1.30), # Front Right
        (-0.92, 0.36, -1.30), # Rear Left
        ( 0.92, 0.36, -1.30), # Rear Right
    ]
    for wx, wy, wz in wheel_positions:
        b.add_cylinder(wx, wy, wz, radius=0.36, height=0.24, segments=10, uv=UV_TIRE, axis='x')
        hub_x = wx + (0.02 if wx > 0 else -0.02)
        b.add_cylinder(hub_x, wy, wz, radius=0.20, height=0.26, segments=8, uv=UV_RIMS, axis='x')

    # 2. Main Lower Chassis & Bumpers (y: 0.28 to 0.76)
    b.add_box(0.0, 0.54, 0.0, 1.84, 0.38, 4.20, uv=UV_BODY)
    # Front Bumper & Grill
    b.add_box(0.0, 0.50,  2.16, 1.82, 0.28, 0.18, uv=UV_GRILL)
    b.add_box(0.0, 0.54,  2.24, 0.80, 0.18, 0.04, uv=UV_GRILL)
    # Rear Bumper
    b.add_box(0.0, 0.50, -2.16, 1.82, 0.28, 0.18, uv=UV_GRILL)

    # 3. Headlights & Taillights
    b.add_box(-0.65, 0.60, 2.13, 0.32, 0.14, 0.06, uv=UV_HEADLIGHT)
    b.add_box( 0.65, 0.60, 2.13, 0.32, 0.14, 0.06, uv=UV_HEADLIGHT)
    b.add_box(-0.65, 0.62, -2.13, 0.32, 0.12, 0.06, uv=UV_TAILLIGHT)
    b.add_box( 0.65, 0.62, -2.13, 0.32, 0.12, 0.06, uv=UV_TAILLIGHT)

    # 4. Front Hood & Rear Trunk Deck
    b.add_box(0.0, 0.76, 1.35, 1.76, 0.12, 1.30, uv=UV_BODY) # Hood
    b.add_box(0.0, 0.78, -1.50, 1.76, 0.12, 1.00, uv=UV_BODY) # Trunk

    # 5. Passenger Cabin & Glass Windows (y: 0.78 to 1.38)
    # Cabin body base
    b.add_box(0.0, 0.95, -0.10, 1.68, 0.32, 1.90, uv=UV_BODY)
    # Cabin Roof
    b.add_box(0.0, 1.36, -0.15, 1.50, 0.08, 1.50, uv=UV_BODY)

    # Front Windshield (slanted glass wedge)
    b.add_box(0.0, 1.10, 0.72, 1.52, 0.36, 0.28, uv=UV_GLASS)
    # Rear Windshield
    b.add_box(0.0, 1.10, -0.92, 1.52, 0.36, 0.26, uv=UV_GLASS)
    # Side Windows (Left & Right)
    b.add_box(-0.81, 1.12, -0.12, 0.06, 0.32, 1.40, uv=UV_GLASS)
    b.add_box( 0.81, 1.12, -0.12, 0.06, 0.32, 1.40, uv=UV_GLASS)

    # Side Mirrors
    b.add_box(-0.95, 0.94, 0.62, 0.14, 0.09, 0.12, uv=UV_BODY)
    b.add_box( 0.95, 0.94, 0.62, 0.14, 0.09, 0.12, uv=UV_BODY)

    return b.to_obj_string()

def build_military_crate_model():
    """Authentic reinforced military supply container crate."""
    b = ObjBuilder()

    UV_WOOD = (0.3125, 0.1328)      # Olive/khaki military wood
    UV_METAL = (0.4375, 0.0625)     # Steel edge reinforcements
    UV_STENCIL = (0.6562, 0.1328)   # Military green accents

    # Main wooden box (1.2m x 0.8m x 0.8m)
    b.add_box(0.0, 0.40, 0.0, 1.20, 0.76, 0.80, uv=UV_WOOD)
    # Raised top lid
    b.add_box(0.0, 0.79, 0.0, 1.24, 0.06, 0.84, uv=UV_WOOD)

    # Steel corner angle brackets (4 vertical corners)
    b.add_box(-0.60, 0.40, -0.40, 0.08, 0.78, 0.08, uv=UV_METAL)
    b.add_box( 0.60, 0.40, -0.40, 0.08, 0.78, 0.08, uv=UV_METAL)
    b.add_box(-0.60, 0.40,  0.40, 0.08, 0.78, 0.08, uv=UV_METAL)
    b.add_box( 0.60, 0.40,  0.40, 0.08, 0.78, 0.08, uv=UV_METAL)

    # Steel horizontal reinforcing bands
    b.add_box(0.0, 0.66, 0.0, 1.22, 0.06, 0.82, uv=UV_METAL)
    b.add_box(0.0, 0.16, 0.0, 1.22, 0.06, 0.82, uv=UV_METAL)

    # Front lock hasp & latches
    b.add_box(0.0, 0.74, 0.42, 0.10, 0.12, 0.04, uv=UV_METAL)
    b.add_box(-0.35, 0.74, 0.42, 0.08, 0.10, 0.04, uv=UV_METAL)
    b.add_box( 0.35, 0.74, 0.42, 0.08, 0.10, 0.04, uv=UV_METAL)

    # Side heavy carrying handles
    b.add_box(-0.62, 0.45, 0.0, 0.05, 0.10, 0.22, uv=UV_METAL)
    b.add_box( 0.62, 0.45, 0.0, 0.05, 0.10, 0.22, uv=UV_METAL)

    return b.to_obj_string()

def build_safe_model():
    """Heavy reinforced steel security safe with rotating combination dial and handle."""
    b = ObjBuilder()

    UV_BODY = (0.6875, 0.5000)     # Dark steel/iron body
    UV_DOOR = (0.4375, 0.0625)     # Steel door
    UV_GOLD = (0.1328, 0.2422)     # Brass combination dial
    UV_HANDLE = (0.4375, 0.0625)   # Chrome turn handle

    # Heavy safe body (0.7m x 0.9m x 0.65m)
    b.add_box(0.0, 0.45, 0.0, 0.70, 0.88, 0.64, uv=UV_BODY)
    # Heavy base plinth
    b.add_box(0.0, 0.04, 0.0, 0.74, 0.08, 0.68, uv=UV_BODY)
    # Safe Door (recessed into front at +Z)
    b.add_box(0.0, 0.45, 0.32, 0.58, 0.76, 0.04, uv=UV_DOOR)

    # Heavy right-side door hinges
    b.add_cylinder(0.28, 0.72, 0.33, radius=0.04, height=0.12, segments=6, uv=UV_HANDLE, axis='y')
    b.add_cylinder(0.28, 0.22, 0.33, radius=0.04, height=0.12, segments=6, uv=UV_HANDLE, axis='y')

    # Central combination dial wheel
    b.add_cylinder(0.0, 0.52, 0.35, radius=0.09, height=0.04, segments=10, uv=UV_GOLD, axis='z')
    # Dial center knob
    b.add_cylinder(0.0, 0.52, 0.38, radius=0.04, height=0.03, segments=8, uv=UV_HANDLE, axis='z')

    # 3-Spoke Wheel Turn Handle
    b.add_cylinder(-0.14, 0.42, 0.35, radius=0.03, height=0.04, segments=6, uv=UV_HANDLE, axis='z')
    b.add_box(-0.14, 0.42, 0.37, 0.16, 0.03, 0.03, uv=UV_HANDLE)
    b.add_box(-0.14, 0.42, 0.37, 0.03, 0.16, 0.03, uv=UV_HANDLE)

    return b.to_obj_string()

def build_pine_tree_model():
    """Iconic low-poly layered pine tree with multi-tiered canopy."""
    b = ObjBuilder()

    UV_BARK = (0.3125, 0.1328)      # Tree trunk bark
    UV_NEEDLES = (0.6562, 0.1328)   # Forest pine needles

    # Trunk (height 5.2m, radius 0.24)
    b.add_cylinder(0.0, 2.6, 0.0, radius=0.22, height=5.2, segments=7, uv=UV_BARK, axis='y')

    # 4 Tiered Foliage Cones
    # Tier 1 (Lowest, widest)
    b.add_cone(0.0, 1.8, 0.0, radius=2.20, height=2.0, segments=8, uv=UV_NEEDLES)
    # Tier 2 (Middle-low)
    b.add_cone(0.0, 2.8, 0.0, radius=1.75, height=1.9, segments=8, uv=UV_NEEDLES)
    # Tier 3 (Middle-high)
    b.add_cone(0.0, 3.8, 0.0, radius=1.30, height=1.8, segments=8, uv=UV_NEEDLES)
    # Tier 4 (Top spire)
    b.add_cone(0.0, 4.8, 0.0, radius=0.85, height=1.9, segments=8, uv=UV_NEEDLES)

    return b.to_obj_string()

def build_ak47_model():
    """Low-poly AK-47 assault rifle."""
    b = ObjBuilder()

    UV_STEEL = (0.4375, 0.0625)
    UV_WOOD = (0.3125, 0.1328)
    UV_BLACK = (0.6875, 0.5000)

    # Receiver
    b.add_box(0.0, 0.0, 0.0, 0.06, 0.10, 0.42, uv=UV_STEEL)
    # Wooden Buttstock
    b.add_box(0.0, -0.04, -0.34, 0.05, 0.14, 0.32, uv=UV_WOOD)
    # Wooden Handguard
    b.add_box(0.0, 0.0, 0.32, 0.07, 0.09, 0.24, uv=UV_WOOD)
    # Barrel & Gas Tube
    b.add_cylinder(0.0, 0.02, 0.54, radius=0.016, height=0.32, segments=6, uv=UV_STEEL, axis='z')
    b.add_cylinder(0.0, 0.06, 0.38, radius=0.014, height=0.24, segments=6, uv=UV_STEEL, axis='z')
    # Curved Banana Magazine
    b.add_box(0.0, -0.16, 0.10, 0.04, 0.24, 0.10, uv=UV_STEEL)
    # Pistol Grip
    b.add_box(0.0, -0.12, -0.10, 0.04, 0.15, 0.06, uv=UV_WOOD)
    # Front sight post
    b.add_box(0.0, 0.07, 0.68, 0.02, 0.07, 0.03, uv=UV_STEEL)

    return b.to_obj_string()

def build_glock_model():
    """Low-poly Glock combat pistol."""
    b = ObjBuilder()

    UV_SLIDE = (0.4375, 0.0625)
    UV_FRAME = (0.6875, 0.5000)

    # Slide (Top)
    b.add_box(0.0, 0.06, 0.04, 0.04, 0.06, 0.22, uv=UV_SLIDE)
    # Polymer Frame (Lower)
    b.add_box(0.0, 0.02, 0.02, 0.038, 0.04, 0.20, uv=UV_FRAME)
    # Grip Handle
    b.add_box(0.0, -0.07, -0.02, 0.036, 0.14, 0.06, uv=UV_FRAME)
    # Trigger Guard
    b.add_box(0.0, -0.01, 0.05, 0.02, 0.05, 0.06, uv=UV_FRAME)

    return b.to_obj_string()

def build_shotgun_model():
    """Low-poly 12-gauge pump shotgun."""
    b = ObjBuilder()
    UV_STEEL = (0.4375, 0.0625)
    UV_WOOD = (0.3125, 0.1328)
    b.add_box(0.0, 0.0, 0.0, 0.05, 0.08, 0.36, uv=UV_STEEL)
    b.add_box(0.0, -0.04, -0.28, 0.04, 0.12, 0.28, uv=UV_WOOD)
    b.add_cylinder(0.0, 0.02, 0.44, radius=0.016, height=0.52, segments=6, uv=UV_STEEL, axis='z')
    b.add_cylinder(0.0, -0.02, 0.38, radius=0.018, height=0.38, segments=6, uv=UV_STEEL, axis='z')
    b.add_cylinder(0.0, -0.02, 0.32, radius=0.025, height=0.18, segments=8, uv=UV_WOOD, axis='z')
    return b.to_obj_string()

def build_sniper_model():
    """Low-poly .50 cal long-range sniper rifle."""
    b = ObjBuilder()
    UV_STEEL = (0.4375, 0.0625)
    UV_BLACK = (0.6875, 0.5000)
    b.add_box(0.0, 0.0, 0.0, 0.07, 0.11, 0.56, uv=UV_BLACK)
    b.add_box(0.0, -0.03, -0.38, 0.06, 0.13, 0.34, uv=UV_BLACK)
    b.add_cylinder(0.0, 0.02, 0.68, radius=0.018, height=0.80, segments=6, uv=UV_STEEL, axis='z')
    b.add_box(0.0, 0.02, 1.10, 0.07, 0.06, 0.10, uv=UV_STEEL) # Muzzle brake
    b.add_cylinder(0.0, 0.10, 0.04, radius=0.03, height=0.40, segments=8, uv=UV_STEEL, axis='z') # Scope
    b.add_box(0.0, -0.16, 0.06, 0.05, 0.22, 0.12, uv=UV_BLACK) # Magazine
    return b.to_obj_string()

def build_boulder_model():
    """Low-poly mountain boulder rock."""
    b = ObjBuilder()
    UV_ROCK = (0.4375, 0.0625)
    b.add_box(0.0, 0.6, 0.0, 1.8, 1.2, 1.6, uv=UV_ROCK)
    b.add_box(0.2, 0.9, -0.1, 1.3, 0.8, 1.2, uv=UV_ROCK, rot_y=0.4)
    b.add_box(-0.3, 0.4, 0.2, 1.2, 0.7, 1.3, uv=UV_ROCK, rot_y=-0.3)
    return b.to_obj_string()

def build_barrier_model():
    """Concrete highway jersey barrier."""
    b = ObjBuilder()
    UV_CONCRETE = (0.1328, 0.1484)
    # Jersey barrier profile: wide base, tapered middle, rounded top
    b.add_box(0.0, 0.15, 0.0, 0.70, 0.30, 2.40, uv=UV_CONCRETE)
    b.add_box(0.0, 0.50, 0.0, 0.42, 0.42, 2.40, uv=UV_CONCRETE)
    b.add_box(0.0, 0.80, 0.0, 0.26, 0.22, 2.40, uv=UV_CONCRETE)
    return b.to_obj_string()

def main():
    dest = Path("assets/models")
    dest.mkdir(parents=True, exist_ok=True)

    models = {
        "SM_Char_Player_Male.obj": build_player_model(),
        "SM_Zombie_Walker.obj": build_zombie_model(),
        "SM_Zombie_Runner.obj": build_zombie_model(),
        "SM_Zombie_Juggernaut.obj": build_zombie_model(),
        "SM_Veh_Sedan_01.obj": build_sedan_model(),
        "SM_Veh_Pickup_01.obj": build_sedan_model(),
        "SM_Prop_Crate_Military.obj": build_military_crate_model(),
        "SM_Prop_Safe_01.obj": build_safe_model(),
        "SM_Env_PineTree_01.obj": build_pine_tree_model(),
        "SM_Env_Boulder_01.obj": build_boulder_model(),
        "SM_Prop_Barrier_01.obj": build_barrier_model(),
        "SM_Wep_Rifle_AK47.obj": build_ak47_model(),
        "SM_Wep_Pistol_Glock.obj": build_glock_model(),
        "SM_Wep_Shotgun_870.obj": build_shotgun_model(),
        "SM_Wep_Sniper_Barret.obj": build_sniper_model(),
    }

    print("==================================================")
    print(" Generating Authentic Low-Poly Synty Models       ")
    print("==================================================")

    for name, content in models.items():
        out_path = dest / name
        with open(out_path, "w") as f:
            f.write(content)
        size_kb = len(content) / 1024
        print(f" [OK] Generated: {name:28} ({size_kb:.1f} KB)")

if __name__ == "__main__":
    main()
