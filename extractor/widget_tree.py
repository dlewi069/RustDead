#!/usr/bin/env python3
"""Print a UMG widget tree from a `SdExport json` dump: hierarchy, slot layout and key properties.

usage: widget_tree.py <Widget.json>
"""
import json
import sys

exports = json.load(open(sys.argv[1]))
by_name = {}
for e in exports:
    by_name[f"{e['Type']}'{e['Name']}"] = e


def ref_key(ref):
    # "Border'MenuWidget_C:WidgetTree.Border_92'" -> "Border'Border_92"
    name = ref["ObjectName"]
    typ, rest = name.split("'", 1)
    return f"{typ}'{rest.rstrip(chr(39)).split('.')[-1]}"


children = {}
slots = {}
for e in exports:
    p = e.get("Properties", {})
    if "Parent" in p and "Content" in p:
        children.setdefault(ref_key(p["Parent"]), []).append(ref_key(p["Content"]))
        slots[ref_key(p["Content"])] = e


def text_of(t):
    if not isinstance(t, dict):
        return None
    return t.get("SourceString") or t.get("CultureInvariantString")


def describe(e):
    p = e.get("Properties", {})
    bits = []
    if (t := text_of(p.get("Text"))) is not None:
        bits.append(repr(t))
    if "Font" in p:
        bits.append(f"size={p['Font'].get('Size')}")
    for k in ("ColorAndOpacity", "BrushColor", "BackgroundColor", "ContentColorAndOpacity"):
        if k in p:
            c = p[k].get("SpecifiedColor", p[k]) if isinstance(p[k], dict) else p[k]
            bits.append(f"{k}={c}")
    if "Brush" in p:
        b = p["Brush"]
        res = b.get("ResourceObject", {})
        bits.append(f"brush={res.get('ObjectName', '')} size={b.get('ImageSize')} tint={b.get('TintColor', {}).get('SpecifiedColor')}")
    for k in ("WidthOverride", "HeightOverride", "Stretch", "Visibility", "RenderOpacity", "BlurStrength", "Size", "TextSize", "Padding"):
        if k in p:
            bits.append(f"{k}={p[k]}")
    return " ".join(bits)


def slot_desc(s):
    p = s.get("Properties", {})
    out = []
    if "LayoutData" in p:
        out.append(f"layout={json.dumps(p['LayoutData'], separators=(',', ':'))}")
    for k in ("bAutoSize", "ZOrder", "Padding", "Size", "HorizontalAlignment", "VerticalAlignment"):
        if k in p:
            out.append(f"{k}={p[k]}")
    return " ".join(out)


def walk(key, depth):
    e = by_name.get(key)
    ind = "  " * depth
    print(f"{ind}{key.replace(chr(39), ' ')}  {describe(e) if e else ''}")
    if key in slots:
        sd = slot_desc(slots[key])
        if sd:
            print(f"{ind}    [{sd}]")
    kids = children.get(key, [])
    # A panel's "Slots" lists its slot objects in visual (draw) order. Slot names repeat across
    # panels (OverlaySlot_3 in two overlays), so each is resolved by its ObjectPath's export index.
    ordered = []
    for r in (e or {}).get("Properties", {}).get("Slots", []):
        idx = r.get("ObjectPath", "").rsplit(".", 1)[-1]
        slot = exports[int(idx)] if idx.isdigit() and int(idx) < len(exports) else None
        content = slot and slot.get("Properties", {}).get("Content")
        if content and ref_key(content) in kids and ref_key(content) not in ordered:
            ordered.append(ref_key(content))
    kids = ordered + [k for k in kids if k not in ordered]
    for c in kids:
        walk(c, depth + 1)
    # single-content containers (Border, Button, ScaleBox, SizeBox) keep children in Slot objects too
    return


roots = [k for k in children if k not in slots]
for r in roots:
    walk(r, 0)
