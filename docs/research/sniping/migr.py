#!/usr/bin/env python3
"""Sniping alla migrazione: rendimento di un acquisto su PumpSwap a M+L e vendita dopo H slot.

Da amm.jsonl (logs per mint nei 10 minuti dopo il messaggio 'migrate' di PumpPortal).
Stato del pool a fine slot s ~ pre-stato del primo Buy/SellEvent con slot > s (Qeff = vault quote + virtual).
Esclusi i token mayhem e i pool non canonici; fee 1,25% per lato (primo scaglione).
Uso: migr.py <recdir>
"""
import collections, json, os, sys

REC = sys.argv[1]
FEE = 0.0125


def main():
    ev = collections.defaultdict(list)  # mint -> [(slot, kind, e)]
    mig_slot = {}
    mayhem = set()
    pool_of = {}
    for line in open(os.path.join(REC, "amm.jsonl")):
        if not line.endswith("\n"):
            continue
        r = json.loads(line)
        if "ev" not in r:
            continue
        m = r["mint"]
        for prog, name, e in r["ev"]:
            if name == "CompletePumpAmmMigrationEvent":
                mig_slot.setdefault(m, r["slot"])
                pool_of[m] = e.get("pool")
            elif name == "TradeEvent" and e.get("mayhem_mode"):
                mayhem.add(m)
            elif name in ("BuyEvent", "SellEvent"):
                if e.get("protocol_fee_basis_points") == 0 and e.get("lp_fee_basis_points") == 0:
                    continue  # boost_buy_and_burn
                ev[m].append((r["slot"], name, e))
    Ls = [0, 1, 2, 3, 5, 10]
    Hs = [5, 25, 50, 150, 300, 700, 1400]
    res = {(L, H): [] for L in Ls for H in Hs}
    n_used = 0
    for m in ev:
        if m in mayhem:
            continue
        pools = collections.Counter(e.get("pool") for _, _, e in ev[m])
        pool = pool_of.get(m) or pools.most_common(1)[0][0]
        M = mig_slot.get(m) or min(s for s, _, e in ev[m] if e.get("pool") == pool)
        tr = sorted(((s, e) for s, _, e in ev[m] if e.get("pool") == pool), key=lambda x: x[0])
        if len(tr) < 3:
            continue
        slots = [s for s, _ in tr]

        def price_end(s):
            # pre-stato del primo trade dopo s
            import bisect
            i = bisect.bisect_right(slots, s)
            if i >= len(tr):
                return None
            e = tr[i][1]
            q = e["pool_quote_token_reserves"] + (e.get("virtual_quote_reserves") or 0)
            b = e["pool_base_token_reserves"]
            return q / b if b else None

        n_used += 1
        for L in Ls:
            p0 = price_end(M + L)
            if not p0:
                continue
            for H in Hs:
                p1 = price_end(M + L + H)
                if p1 is None:
                    # nessun trade dopo: ultimo prezzo noto (token fermo)
                    e = tr[-1][1]
                    p1 = (e["pool_quote_token_reserves"] + (e.get("virtual_quote_reserves") or 0)) / e["pool_base_token_reserves"]
                res[(L, H)].append(p1 / p0 * (1 - FEE) ** 2 - 1)
    print(f"migrazioni con dati: {n_used} (mayhem esclusi: {len(mayhem)})")
    for L in Ls:
        row = []
        for H in Hs:
            v = sorted(res[(L, H)])
            if not v:
                continue
            n = len(v)
            row.append(f"H{H}: n={n} mean={sum(v)/n*100:+.1f}% med={v[n//2]*100:+.1f}% win={sum(1 for x in v if x>0)/n*100:.0f}%")
        print(f"L={L}  " + " | ".join(row))


main()
