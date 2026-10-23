#!/usr/bin/env python3
"""Print a MetaSoundSource's graph as readable dataflow.

usage: metasound.py <dump.json>   (from `SdExport json <out> <MetaSound package>`)

Each node is printed as `N<i> Namespace.Name(.Variant)` with its literal inputs (the values set
on its pins), then each node's incoming edges as `input <- N<j>.output`. Graph inputs show their
default values; wave assets show as their package names.
"""
import json
import sys


def literal(lit):
    if not lit:
        return None
    for k in ("AsFloat", "AsInteger", "AsBoolean", "AsString"):
        if lit.get(k):
            v = lit[k]
            return v[0] if len(v) == 1 else v
    if lit.get("AsUObject"):
        return [o.get("ObjectName", o) if isinstance(o, dict) else o for o in lit["AsUObject"]]
    return None


def main():
    exports = json.load(open(sys.argv[1]))
    src = next(e for e in exports if e["Type"].startswith("MetaSound"))
    doc = src["Properties"]["RootMetaSoundDocument"]
    classes = {d["ID"]: d["MetaData"]["ClassName"] for d in doc.get("Dependencies", [])}
    graph = doc["RootGraph"]["Graph"]
    print("inputs:")
    for i in doc["RootGraph"]["Interface"].get("Inputs", []):
        print(f"  {i['Name']} ({i['TypeName']}) = {literal(i.get('DefaultLiteral'))}")
    nodes = graph["Nodes"]
    index = {n["ID"]: k for k, n in enumerate(nodes)}
    # Vertex IDs repeat across nodes (every graph-input node has the same ones): key by node.
    pins = {}
    for n in nodes:
        for side in ("Inputs", "Outputs"):
            for v in n["Interface"].get(side, []):
                pins[(n["ID"], v["VertexID"])] = v["Name"]
    for v in graph.get("Variables", []):
        print(f"variable {v.get('Name')} ({v.get('TypeName')}) = {literal(v.get('Literal'))}")
    incoming = {}
    for e in graph["Edges"]:
        incoming.setdefault(e["ToNodeID"], []).append(e)
    for k, n in enumerate(nodes):
        c = classes.get(n["ClassID"], {})
        name = ".".join(x for x in (c.get("Namespace"), c.get("Name"), c.get("Variant")) if x and x != "None")
        lits = {pins.get((n["ID"], l["VertexID"]), "?"): literal(l.get("Value")) for l in n.get("InputLiterals", [])}
        print(f"N{k} {name or n['Name']}" + (f" [{n['Name']}]" if n["Name"] != c.get("Name") else "") + (f" {lits}" if lits else ""))
        for e in incoming.get(n["ID"], []):
            print(f"    {pins.get((n['ID'], e['ToVertexID']), '?')} <- N{index.get(e['FromNodeID'], '?')}.{pins.get((e['FromNodeID'], e['FromVertexID']), '?')}")


if __name__ == "__main__":
    main()
