#!/usr/bin/env python3
"""Deliberately broken round-trip implementations (used by selftest.py).

    python broken_sut.py <mode> <input> <output>
"""
import json
import sys


def main() -> int:
    mode, src, dst = sys.argv[1:4]
    objs = [json.loads(line) for line in open(src, encoding="utf-8") if line.strip().startswith("{")]
    if mode == "crash-unknown" and any(o["@type"].startswith("pmef:Future") for o in objs):
        return 3
    out = []
    for o in objs:
        if mode == "drop" and o["@type"] == "pmef:Pump":
            continue
        if mode == "pressure" and "designConditions" in o:
            o["designConditions"]["designPressure"] += 10.0
        if mode == "custom":
            o.pop("customAttributes", None)
            for k in [k for k in o if k.startswith("x-")]:
                o.pop(k)
        if mode == "revision" and "revision" in o:
            o["revision"]["changeState"] = "WIP"
        if mode == "coords":
            for p in o.get("ports", []) or []:
                p["coordinate"] = [c + 0.5 for c in p["coordinate"]]
        out.append(o)
    if mode == "header-last":
        out = out[1:] + out[:1]
    with open(dst, "w", encoding="utf-8") as f:
        for o in out:
            f.write(json.dumps(o) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
