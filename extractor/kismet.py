#!/usr/bin/env python3
"""Print Blueprint bytecode as readable pseudo-code.

usage: kismet.py <dump.json> [function-name-substring ...]

The dump comes from `SD_SCRIPT=1 bin/out/SdExport json <out> <package>` (ReadScriptData on).
Each function's Kismet statements are printed with their code offsets, so jumps can be
followed by hand: `goto 907` / `if not (cond) goto 907`. Calls show as `Lib.Func(args)`,
variables by name, `CallFunc_X_ReturnValue` temporaries are shortened to `X()`.
"""
import json
import sys


def name(ref):
    if not ref:
        return "None"
    if isinstance(ref, dict) and "ObjectName" in ref:
        n = ref["ObjectName"]
        n = n.split("'")[1] if "'" in n else n
        return n.split(":")[-1].split(".")[-1]
    return str(ref)


def var(v):
    p = (v or {}).get("Property") or {}
    n = p.get("Name", "?")
    if n.startswith("CallFunc_") and n.endswith("_ReturnValue"):
        return n[len("CallFunc_"):-len("_ReturnValue")] + "()"
    return n


def e(x):
    if x is None:
        return "None"
    t = x.get("Token", "?")
    g = x.get
    if t in ("EX_LocalVariable", "EX_InstanceVariable", "EX_LocalOutVariable", "EX_DefaultVariable", "EX_ClassSparseDataVariable"):
        return var(g("Variable"))
    if t in ("EX_IntConst", "EX_FloatConst", "EX_DoubleConst", "EX_ByteConst", "EX_Int64Const", "EX_UInt64Const", "EX_IntConstByte", "EX_SkipOffsetConst"):
        return str(g("Value"))
    if t in ("EX_StringConst", "EX_UnicodeStringConst"):
        return json.dumps(g("Value"))
    if t == "EX_NameConst":
        return f"'{g('Value')}'"
    if t == "EX_TextConst":
        v = g("Value") or {}
        src = v.get("SourceString") or (v.get("Value") or {}).get("SourceString") if isinstance(v, dict) else v
        return f"T{json.dumps(src)}"
    if t == "EX_True":
        return "true"
    if t == "EX_False":
        return "false"
    if t in ("EX_NoObject", "EX_NoInterface"):
        return "null"
    if t == "EX_Self":
        return "self"
    if t == "EX_ObjectConst":
        return name(g("Value"))
    if t in ("EX_VectorConst", "EX_RotationConst", "EX_TransformConst", "EX_Vector3fConst"):
        return f"{t[3:-5]}{json.dumps(g('Value'))}"
    if t == "EX_StructConst":
        return f"{name(g('Struct'))}({', '.join(e(p) for p in g('Properties') or [])})"
    if t in ("EX_CallMath", "EX_FinalFunction", "EX_LocalFinalFunction", "EX_VirtualFunction", "EX_LocalVirtualFunction", "EX_CallMulticastDelegate"):
        f = g("Function") or g("VirtualFunctionName")
        fn = name(f) if isinstance(f, dict) else str(f)
        lib = ""
        if isinstance(f, dict) and ":" in f.get("ObjectName", ""):
            lib = f["ObjectName"].split("'")[1].split(":")[0] + "."
        args = ", ".join(e(p) for p in g("Parameters") or [])
        return f"{lib}{fn}({args})"
    if t in ("EX_Context", "EX_Context_FailSilent", "EX_ClassContext"):
        return f"{e(g('ObjectExpression'))}.{e(g('ContextExpression'))}"
    if t == "EX_InterfaceContext":
        return e(g("InterfaceValue"))
    if t in ("EX_Let", "EX_LetObj", "EX_LetBool", "EX_LetWeakObjPtr", "EX_LetValueOnPersistentFrame", "EX_LetMulticastDelegate", "EX_LetDelegate"):
        lhs = e(g("Variable")) if g("Variable") else var(g("DestinationProperty") or {})
        return f"{lhs} = {e(g('Expression') or g('Assignment'))}"
    if t in ("EX_DynamicCast", "EX_MetaCast", "EX_ObjToInterfaceCast", "EX_CrossInterfaceCast", "EX_InterfaceToObjCast"):
        return f"cast<{name(g('ClassPtr'))}>({e(g('Target') or g('Expression'))})"
    if t == "EX_PrimitiveCast":
        return f"{g('ConversionType', 'cast')}({e(g('Target'))})"
    if t == "EX_StructMemberContext":
        return f"{e(g('StructExpression'))}.{(g('Property') or {}).get('Name', '?')}"
    if t == "EX_ArrayGetByRef":
        return f"{e(g('ArrayVariable'))}[{e(g('ArrayIndex'))}]"
    if t in ("EX_SetArray", "EX_SetSet", "EX_SetMap"):
        return f"{e(g('AssigningProperty'))} = [{', '.join(e(p) for p in g('Elements') or [])}]"
    if t == "EX_ArrayConst":
        return f"[{', '.join(e(p) for p in g('Elements') or [])}]"
    if t == "EX_Jump":
        return f"goto {g('CodeOffset')}"
    if t == "EX_JumpIfNot":
        return f"if not ({e(g('BooleanExpression'))}) goto {g('CodeOffset')}"
    if t == "EX_ComputedJump":
        return f"goto computed {e(g('CodeOffsetExpression'))}"
    if t == "EX_PushExecutionFlow":
        return f"push_flow {g('PushingAddress')}"
    if t in ("EX_PopExecutionFlow",):
        return "pop_flow"
    if t == "EX_PopExecutionFlowIfNot":
        return f"pop_flow if not ({e(g('BooleanExpression'))})"
    if t == "EX_Return":
        return f"return {e(g('ReturnExpression'))}"
    if t == "EX_SwitchValue":
        cases = "; ".join(f"{e(c.get('CaseIndexValueTerm'))} => {e(c.get('CaseTerm'))}" for c in g("Cases") or [])
        return f"switch {e(g('IndexTerm'))} {{ {cases}; _ => {e(g('DefaultTerm'))} }}"
    if t in ("EX_BindDelegate", "EX_AddMulticastDelegate", "EX_RemoveMulticastDelegate"):
        return f"{t[3:]}({e(g('Delegate'))}, {e(g('DelegateToAdd') or g('ObjectTerm'))} {g('FunctionName', '')})"
    if t == "EX_InstanceDelegate":
        return f"delegate {g('FunctionName')}"
    if t in ("EX_Nothing", "EX_EndOfScript", "EX_WireTracepoint", "EX_Tracepoint", "EX_InstrumentationEvent"):
        return ""
    return f"<{t}>"


def main():
    d = json.load(open(sys.argv[1]))
    want = sys.argv[2:]
    for x in d:
        if "ScriptBytecode" not in x or (want and not any(w.lower() in x["Name"].lower() for w in want)):
            continue
        print(f"\n== {x['Name']}")
        for st in x["ScriptBytecode"]:
            line = e(st)
            if line:
                print(f"  {st.get('StatementIndex', ''):>5}  {line}")


if __name__ == "__main__":
    main()
