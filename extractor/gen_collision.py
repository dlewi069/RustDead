#!/usr/bin/env python3
"""Simple collision of the level's trees -> assets/data/simple_collision.json.

UE collides foliage trees against their StaticMesh BodySetup's simple shapes, not the drawn
triangles (which the port can't split into trunk and leaves: the exported vertex alpha is 255
everywhere). The trees' AggGeom is capsules (SphylElems: pines, birches, willows) or convex hulls
(ConvexElems: palms, the forest trees, logs). Each becomes triangles in the mesh's glTF space:

    {"<mesh package>": [[x0, y0, z0, x1, y1, z1, x2, y2, z2], ...], ...}

metres, (x, y, z)_gltf = (X, Z, Y)_ue / 100. A capsule is an 8-sided prism with pointed caps.

    gen_collision.py <SdExport json dump dir> <mesh package>...
"""
import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "assets" / "data" / "simple_collision.json"
SIDES = 8


def rotator_quat(r):
    """FRotator (degrees) -> quaternion (x, y, z, w), as FRotator::Quaternion."""
    p, y, ro = (math.radians(r.get(k, 0.0)) / 2 for k in ("Pitch", "Yaw", "Roll"))
    sp, cp, sy, cy, sr, cr = math.sin(p), math.cos(p), math.sin(y), math.cos(y), math.sin(ro), math.cos(ro)
    return (cr * sp * sy - sr * cp * cy, -cr * sp * cy - sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy)


def rotate(q, v):
    x, y, z, w = q
    # v + 2w (q x v) + 2 q x (q x v)
    tx, ty, tz = 2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0])
    return (v[0] + w * tx + (y * tz - z * ty), v[1] + w * ty + (z * tx - x * tz), v[2] + w * tz + (x * ty - y * tx))


def to_gltf(v):
    return [v[0] / 100.0, v[2] / 100.0, v[1] / 100.0]


def capsule(e):
    r, half = e["Radius"], e["Length"] / 2.0
    q = rotator_quat(e.get("Rotation") or {})
    c = e["Center"]
    ring = lambda z: [(r * math.cos(2 * math.pi * k / SIDES), r * math.sin(2 * math.pi * k / SIDES), z) for k in range(SIDES)]
    lo, hi = ring(-half), ring(half)
    top, bottom = (0.0, 0.0, half + r), (0.0, 0.0, -half - r)
    tris = []
    for k in range(SIDES):
        n = (k + 1) % SIDES
        tris += [(lo[k], lo[n], hi[n]), (lo[k], hi[n], hi[k]), (hi[k], hi[n], top), (lo[n], lo[k], bottom)]
    place = lambda v: to_gltf([a + b for a, b in zip(rotate(q, v), (c["X"], c["Y"], c["Z"]))])
    return [sum((place(v) for v in t), []) for t in tris]


def convex(e):
    t = e.get("Transform") or {}
    rq = t.get("Rotation") or {}
    q = (rq.get("X", 0.0), rq.get("Y", 0.0), rq.get("Z", 0.0), rq.get("W", 1.0))
    tr, sc = t.get("Translation") or {}, t.get("Scale3D") or {}
    pts = []
    for v in e.get("VertexData") or []:
        s = (v["X"] * sc.get("X", 1.0), v["Y"] * sc.get("Y", 1.0), v["Z"] * sc.get("Z", 1.0))
        p = rotate(q, s)
        pts.append(to_gltf((p[0] + tr.get("X", 0.0), p[1] + tr.get("Y", 0.0), p[2] + tr.get("Z", 0.0))))
    idx = e.get("IndexData") or []
    return [pts[idx[i]] + pts[idx[i + 1]] + pts[idx[i + 2]] for i in range(0, len(idx) - 2, 3)]


def main():
    dump, meshes = Path(sys.argv[1]), sys.argv[2:]
    out = {}
    for mesh in meshes:
        f = dump / (mesh.removeprefix("SurrounDead/Content/") + ".json")
        if not f.exists():
            continue
        for e in json.loads(f.read_text()):
            if e["Type"] != "BodySetup":
                continue
            g = (e.get("Properties") or {}).get("AggGeom") or {}
            tris = [t for s in g.get("SphylElems") or [] for t in capsule(s)] + [t for c in g.get("ConvexElems") or [] for t in convex(c)]
            if tris:
                out[mesh] = [[round(x, 4) for x in t] for t in tris]
    OUT.write_text(json.dumps(out, separators=(",", ":")))
    print(f"simple collision: {len(out)} meshes, {sum(len(v) for v in out.values())} triangles")


if __name__ == "__main__":
    main()
