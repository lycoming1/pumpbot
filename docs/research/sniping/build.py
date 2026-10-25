#!/usr/bin/env python3
"""Costruisce il dataset per token dai file di rec.py.

Per ogni create riuscito (quote SOL, non mayhem, non cashback) con curva osservata:
  - stato dopo la tx di create (dev buy), stati a fine slot da S0 a S0+HORIZON
  - feature disponibili al momento del create
  - flag di qualità (buchi nel flusso curves, notifica di S0 mancante)
Scrive tokens.jsonl (un token per riga, con il percorso degli stati compresso).

Uso: build.py <recdir> [horizon_slots]
"""
import collections, json, os, sys
from urllib.parse import urlparse

sys.path.insert(0, "/home/kraken/docker/pumpbot/docs/research")
import pumpspec_reference as ps  # noqa: E402

REC = sys.argv[1]
HORIZON = int(sys.argv[2]) if len(sys.argv) > 2 else 2830  # ~10 min
VS0, VT0 = 30_000_000_000, 1_073_000_000_000_000


def load_creates():
    out = {}
    for line in open(os.path.join(REC, "creates.jsonl")):
        if not line.endswith("\n"):
            break
        r = json.loads(line)
        if r.get("err") is not None:
            continue
        ce = [e[2] for e in r.get("ev", []) if e[1] == "CreateEvent"]
        if not ce:
            continue
        c = ce[0]
        trades = [e[2] for e in r["ev"] if e[1] == "TradeEvent" and e[2].get("mint") == c["mint"]]
        out[c["bonding_curve"]] = {"rec": r, "c": c, "trades": trades}
    return out


def load_curves(wanted):
    data = collections.defaultdict(list)
    slots_seen = collections.Counter()
    last_slot = 0
    with open(os.path.join(REC, "curves.csv")) as f:
        for line in f:
            if not line.endswith("\n"):
                break
            p = line[:-1].split(",")
            if len(p) != 8:
                continue
            s = int(p[1])
            slots_seen[s] += 1
            last_slot = max(last_slot, s)
            if p[2] in wanted:
                data[p[2]].append((s, int(p[3]), int(p[4]), int(p[5]), int(p[6]), int(p[7])))
    for c in data:
        data[c].sort()
    return data, slots_seen, last_slot


def stream_gaps(slots_seen, min_gap=4):
    """Intervalli di slot senza alcuna notifica curves (buchi del flusso)."""
    ss = sorted(slots_seen)
    gaps = []
    for a, b in zip(ss, ss[1:]):
        if b - a > min_gap:
            gaps.append((a + 1, b - 1))
    return gaps


def main():
    creates = load_creates()
    eligible = {}
    for curve, d in creates.items():
        c = d["c"]
        if c.get("is_mayhem_mode") or c.get("is_cashback_enabled"):
            continue
        if c.get("quote_mint") not in (ps.ZERO, None):
            continue
        eligible[curve] = d
    curves, slots_seen, last_slot = load_curves(set(eligible))
    gaps = stream_gaps(slots_seen)
    first_slot = min(slots_seen) if slots_seen else 0
    # creatori: numero di create visti finora (cold start) e nomi visti
    creator_count = collections.Counter()
    name_seen = collections.Counter()
    sym_seen = collections.Counter()
    rows = []
    for curve, d in sorted(eligible.items(), key=lambda kv: kv[1]["rec"]["slot"]):
        r, c = d["rec"], d["c"]
        s0 = r["slot"]
        if s0 + HORIZON > last_slot or s0 < first_slot + 5:
            continue
        path = curves.get(curve, [])
        path = [p for p in path if s0 <= p[0] <= s0 + HORIZON]
        gap_entry = any(not (b < s0 - 1 or a > s0 + 25) for a, b in gaps)
        gap_slots = sum(max(0, min(b, s0 + HORIZON) - max(a, s0) + 1) for a, b in gaps)
        # stato dopo la tx di create (ultimo TradeEvent della tx sul mint), altrimenti iniziale
        if d["trades"]:
            last = d["trades"][-1]
            post_vs, post_vt = last["virtual_sol_reserves"], last["virtual_token_reserves"]
        else:
            post_vs, post_vt = VS0, VT0
        dev_sol = sum(t["sol_amount"] for t in d["trades"] if t["is_buy"] and t["user"] == c["user"])
        other_in_tx = sorted({t["user"] for t in d["trades"] if t["user"] != c["user"]})
        other_sol_tx = sum(t["sol_amount"] for t in d["trades"] if t["is_buy"] and t["user"] != c["user"])
        host = ""
        try:
            host = urlparse(c.get("uri") or "").netloc or (c.get("uri") or "").split(":")[0]
        except Exception:  # noqa: BLE001
            pass
        nm = (c.get("name") or "").strip().lower()
        sy = (c.get("symbol") or "").strip().lower()
        row = {
            "curve": curve, "mint": c["mint"], "s0": s0, "t": r["t"], "sig": r["sig"],
            "creator": c.get("creator"), "user": c.get("user"), "name": c.get("name"), "symbol": c.get("symbol"),
            "uri_host": host, "holder_reward": c.get("is_holder_reward"), "creator_fee_bps": c.get("creator_fee_bps"),
            "progs": r.get("progs"), "n_trades_in_create_tx": len(d["trades"]),
            "dev_sol": dev_sol, "other_wallets_in_create_tx": len(other_in_tx), "other_sol_in_create_tx": other_sol_tx,
            "post_create_vs": post_vs, "post_create_vt": post_vt,
            "creator_prev_creates": creator_count[c.get("creator")], "name_prev": name_seen[nm], "sym_prev": sym_seen[sy],
            "gap": gap_entry, "gap_slots": gap_slots, "s0_seen": bool(path and path[0][0] == s0),
            "path": [[p[0] - s0, p[2], p[1], p[5]] for p in path],  # (offset, vq, vt, complete)
        }
        creator_count[c.get("creator")] += 1
        name_seen[nm] += 1
        sym_seen[sy] += 1
        rows.append(row)
    out = os.path.join(REC, "tokens.jsonl")
    with open(out, "w") as f:
        for row in rows:
            f.write(json.dumps(row) + "\n")
    print(json.dumps({"creates_ok": len(creates), "eligible": len(eligible), "written": len(rows),
                      "with_gap": sum(r["gap"] for r in rows), "s0_seen": sum(r["s0_seen"] for r in rows),
                      "stream_gaps": len(gaps), "gap_slots": sum(b - a + 1 for a, b in gaps),
                      "slots_span": [first_slot, last_slot]}, indent=1))


main()
