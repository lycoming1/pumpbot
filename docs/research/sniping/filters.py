#!/usr/bin/env python3
"""Filtri al momento del create: il rendimento dello snipe cambia con le feature osservabili?

Feature (note entro la fine di S0): dev buy, SOL entrati in S0 oltre la tx di create (bundle),
creatore già visto, nome/simbolo già visto (copycat), host dei metadati, programmi nella tx di create.
Split temporale: prima metà = scoperta, seconda metà = verifica.
Uso: filters.py <recdir> <L> <regola> [B_sol]
"""
import collections, json, os, sys

sys.argv = [sys.argv[0], sys.argv[1], sys.argv[4] if len(sys.argv) > 4 else "0.1", "adj"] + sys.argv[2:4]
import sim  # noqa: E402

REC = sys.argv[1]
L = int(sys.argv[4])
RULE = sys.argv[5]


def feats(row):
    p = sim.Path(row)
    vs_end0 = p.at(0)[0]
    s0_in = (vs_end0 - row["post_create_vs"]) / 1e9
    hosts = row["uri_host"] or ""
    tool = ("ipfs" if "ipfs" in hosts or "pinata" in hosts else
            "j7tracker" if "j7tracker" in hosts else "other")
    progs = set(row.get("progs") or [])
    extra = progs - {"ComputeBudget111111111111111111111111111111", "11111111111111111111111111111111",
                     "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL", sim_PUMP}
    return {
        "dev_sol": row["dev_sol"] / 1e9,
        "s0_in": s0_in,
        "creator_seen": row["creator_prev_creates"] > 0,
        "copycat": (row["name_prev"] > 0) or (row["sym_prev"] > 0),
        "tool": tool,
        "launch_prog": bool(extra),
        "multiwallet_create": row["other_wallets_in_create_tx"] > 0,
        "hour": (row["t"] // 10**9 // 3600) % 24,
    }


sim_PUMP = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"


def bucket_stats(items):
    if not items:
        return "n=0"
    rs = sorted(r for r, _ in items)
    n = len(rs)
    m = sum(rs) / n
    w = sum(1 for r in rs if r > 0) / n
    return f"n={n:4d} mean={m*100:+6.1f}% med={rs[n//2]*100:+6.1f}% win={w*100:4.0f}%"


def main():
    rows = sim.load()
    rows.sort(key=lambda r: r["s0"])
    data = []
    for row in rows:
        r = sim.simulate(row, L, {RULE: sim.RULES[RULE]}, 1415)
        if r is None:
            continue
        data.append((r[RULE], feats(row)))
    half = len(data) // 2
    parts = {"scoperta": data[:half], "verifica": data[half:]}
    print(f"L={L} regola={RULE} B={sim.B/1e9} n={len(data)}")
    tests = [
        ("dev_sol", lambda f: "0" if f["dev_sol"] < 0.05 else "<0.5" if f["dev_sol"] < 0.5 else "<2" if f["dev_sol"] < 2 else ">=2"),
        ("s0_in (bundle)", lambda f: "0" if f["s0_in"] < 0.01 else "<1" if f["s0_in"] < 1 else "<5" if f["s0_in"] < 5 else ">=5"),
        ("creator_seen", lambda f: str(f["creator_seen"])),
        ("copycat", lambda f: str(f["copycat"])),
        ("tool", lambda f: f["tool"]),
        ("launch_prog", lambda f: str(f["launch_prog"])),
        ("multiwallet_create", lambda f: str(f["multiwallet_create"])),
    ]
    for name, fn in tests:
        print(f"\n# {name}")
        for pname, items in parts.items():
            g = collections.defaultdict(list)
            for r, f in items:
                g[fn(f)].append((r, f))
            line = " | ".join(f"{k}: {bucket_stats(v)}" for k, v in sorted(g.items()))
            print(f"  {pname:9s} {line}")


main()
