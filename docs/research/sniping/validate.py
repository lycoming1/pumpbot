#!/usr/bin/env python3
"""Completezza del flusso curves (programSubscribe) rispetto alla verità del backfill.

Per ogni token del backfill: slot con almeno un TradeEvent sul mint (tx scaricate) e, per ciascuno,
lo stato dopo l'ultimo trade dello slot (ordine per transactionIndex). Si confronta con le notifiche
curves.csv della stessa curva: presenza dello slot e uguaglianza di virtual_sol_reserves.
Uso: validate.py <recdir>
"""
import collections, json, os, sys

REC = sys.argv[1]
toks = {}
for line in open(os.path.join(REC, "bf_tokens.jsonl")):
    if line.endswith("\n"):
        r = json.loads(line)
        toks[r["curve"]] = r
by_mint = {t["mint"]: t for t in toks.values()}
truth = collections.defaultdict(dict)  # curve -> slot -> (idx, vs)
seen = set()
for line in open(os.path.join(REC, "bf_tx.jsonl")):
    if not line.endswith("\n"):
        continue
    r = json.loads(line)
    if "fetch_err" in r or r["sig"] in seen or r["mint"] not in by_mint:
        continue
    seen.add(r["sig"])
    t = by_mint[r["mint"]]
    if t["truncated"]:
        continue
    tr = [x for x in r["trades"] if x["mint"] == r["mint"]]
    if not tr:
        continue
    idx = r.get("blk_idx")
    if idx is None:
        continue
    cur = truth[t["curve"]].get(r["slot"])
    if cur is None or idx > cur[0]:
        truth[t["curve"]][r["slot"]] = (idx, tr[-1]["virtual_sol_reserves"])
obs = collections.defaultdict(dict)
want = set(truth)
for line in open(os.path.join(REC, "curves.csv")):
    if not line.endswith("\n"):
        continue
    p = line[:-1].split(",")
    if len(p) == 8 and p[2] in want:
        obs[p[2]][int(p[1])] = int(p[4])
n = hit = eq = 0
miss_by_tok = collections.Counter()
for c, sl in truth.items():
    lo = min(sl)
    hi = max(sl)
    for s, (idx, vs) in sl.items():
        n += 1
        if s in obs[c]:
            hit += 1
            eq += obs[c][s] == vs
        else:
            miss_by_tok[c] += 1
print(json.dumps({"tokens": len(truth), "trade_slots": n, "with_notification": hit,
                  "recall": round(hit / n, 4) if n else None, "state_equal": eq,
                  "state_equal_rate": round(eq / hit, 4) if hit else None,
                  "tokens_with_missing": len(miss_by_tok)}, indent=1))
