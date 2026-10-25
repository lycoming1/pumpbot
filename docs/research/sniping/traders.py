#!/usr/bin/env python3
"""Analisi a livello di trader dei token campionati da backfill.py.

- Struttura dello slot del create: bundle (tx contigue al create) vs altri nello stesso slot
- Primi acquirenti per offset di slot, fee e tip pagate, programmi di primo livello
- Tx fallite per offset di slot e codice d'errore
- PnL per wallet nella finestra (vendite - acquisti - fee di rete - tip, più il residuo valutato a fine finestra),
  aggregato per categoria di ingresso

Uso: traders.py <recdir>
"""
import collections, json, os, re, statistics, sys

REC = sys.argv[1]
JITO = {"ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49", "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
        "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL", "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
        "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY", "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
        "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT", "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe"}
HELIUS = {"4ACfpUFoaSD9bfPdeu6DBt89gB6ENTeHBXCAi87NhDEE", "D2L6yPZ2FmmmTKPgzaMKdhu6EWZcTpLy1Vhx8uvZe7NZ",
          "9bnz4RShgq1hAnLnZbP8kbgBg1kEmcJBYQq3gQbmnSta", "5VY91ws6B2hMmBFRsXkoAAdsPHBJwRfBht4DXox3xkwn",
          "2nyhqdwKcJZR2vcqCyrYsaPVdAnFoJjiksCXJ7hfEYgD", "2q5pghRs6arqVjRvT5gfgWfWcHWmw1ZuCzphgd5KfWGJ",
          "wyvPkWjVZz1M8fHQnMMCDTQDbkManefNNhweYk5WkcF", "3KCKozbAaF75qEU33jtzozcJ29yJuaLJTy2jFdzUY8bT",
          "4vieeGHPYPG2MmyPRcYjdiDmmhN3ww7hsFNap8pVN3Ey", "4TQLFNWK8AovT1gFvda5jfw2oJeRMKEmw7aH6MGBJ3or"}


def tip_service(dst):
    if dst in JITO:
        return "jito"
    if dst in HELIUS:
        return "helius"
    if dst.startswith("noz") or dst == "TEMPaMeCRFAS9EKF53Jd6KpHxgL47uWLcpFArU1Fanq":
        return "nozomi"
    if dst.lower().startswith("astra"):
        return "astralane?"
    if dst.lower().startswith("next"):
        return "nextblock?"
    return None


def load():
    toks = {}
    for line in open(os.path.join(REC, "bf_tokens.jsonl")):
        if line.endswith("\n"):
            r = json.loads(line)
            toks[r["mint"]] = r
    txs = collections.defaultdict(list)
    seen = set()
    for line in open(os.path.join(REC, "bf_tx.jsonl")):
        if not line.endswith("\n"):
            continue
        r = json.loads(line)
        if "fetch_err" in r or r["sig"] in seen:
            continue
        seen.add(r["sig"])
        txs[r["mint"]].append(r)
    return toks, txs


def pct(a, p):
    if not a:
        return None
    a = sorted(a)
    return a[min(len(a) - 1, int(p / 100 * len(a)))]


def category(off, in_create_tx, contiguous):
    if in_create_tx:
        return "a_create_tx"
    if off == 0:
        return "b_S0_bundle" if contiguous else "c_S0_other"
    if off == 1:
        return "d_S1"
    if off == 2:
        return "e_S2"
    if off <= 5:
        return "f_S3-5"
    if off <= 20:
        return "g_S6-20"
    if off <= 100:
        return "h_S21-100"
    return "i_S101+"


def main():
    toks, txs = load()
    # destinazioni dei transfer: frequenza per individuare tip account e fee di piattaforma
    dest_cnt = collections.Counter()
    dest_amt = collections.defaultdict(list)
    curves = {t["curve"] for t in toks.values()}
    for mint, rows in txs.items():
        for r in rows:
            users = {tr["user"] for tr in r["trades"]}
            for src, dst, lam in r["transfers"]:
                if dst in curves or dst in users:
                    continue
                dest_cnt[dst] += 1
                dest_amt[dst].append(lam)
    top_dest = dest_cnt.most_common(40)
    tipish = {d for d, n in top_dest if n >= 5}

    s0_struct = collections.Counter()
    first_buy_by_off = collections.defaultdict(list)   # offset -> [sol]
    fee_by_cat = collections.defaultdict(list)
    other_by_cat = collections.defaultdict(list)
    svc_by_cat = collections.defaultdict(collections.Counter)
    svc_cnt = collections.Counter()
    pnl_real_by_cat = collections.defaultdict(list)
    tip_by_cat = collections.defaultdict(list)
    progs_by_cat = collections.defaultdict(collections.Counter)
    fail_by_off = collections.Counter()
    ok_by_off = collections.Counter()
    err_codes = collections.Counter()
    pnl_by_cat = collections.defaultdict(list)
    cost_by_cat = collections.defaultdict(list)
    wallets_by_cat = collections.Counter()
    n_tokens = 0
    truncated = 0
    per_token = []

    for mint, t in toks.items():
        rows = txs.get(mint, [])
        if not rows:
            continue
        n_tokens += 1
        if t["truncated"]:
            truncated += 1
        s0 = t["s0"]
        for sg in t["sigs"]:
            slot, sig16, err = sg[0], sg[1], sg[2]
            off = slot - s0
            if err:
                fail_by_off[min(off, 999)] += 1
                m = re.search(r'"Custom":\s*(\d+)', err)
                err_codes[m.group(1) if m else err[:30]] += 1
            else:
                ok_by_off[min(off, 999)] += 1
        rows.sort(key=lambda r: (r["slot"], r["blk_idx"] if r["blk_idx"] is not None else 10**6))
        create_sig = t["create_sig"]
        order = t.get("s0_order") or {}
        cidx = order.get(create_sig)
        # tx del bundle: indici contigui a partire dal create (tra le tx che toccano la curva)
        idxs = sorted(v for v in order.values())
        bundle_idx = set()
        if cidx is not None:
            j = idxs.index(cidx)
            bundle_idx.add(cidx)
            k = j
            while k + 1 < len(idxs) and idxs[k + 1] == idxs[k] + 1:
                k += 1
                bundle_idx.add(idxs[k])
            k = j
            while k - 1 >= 0 and idxs[k - 1] == idxs[k] - 1:
                k -= 1
                bundle_idx.add(idxs[k])
        n_s0 = len(order)
        n_bundle = len(bundle_idx)
        s0_struct["tokens"] += 1
        s0_struct["create_idx_known"] += cidx is not None
        s0_struct[f"s0_txs={min(n_s0, 6)}"] += 1
        s0_struct[f"bundle_txs={min(n_bundle, 6)}"] += 1

        # PnL per wallet
        w = collections.defaultdict(lambda: {"in": 0, "out": 0, "tok": 0, "netfee": 0, "tip": 0, "first": None,
                                             "cat": None, "buys": 0, "sells": 0})
        last_state = None
        creator = t.get("creator")
        for r in rows:
            off = r["slot"] - s0
            in_create = r["sig"] == create_sig
            contiguous = r.get("blk_idx") in bundle_idx if off == 0 else False
            known = [(tip_service(dst), lam) for src, dst, lam in r["transfers"] if tip_service(dst)]
            tips = sum(lam for _, lam in known)
            other = sum(lam for src, dst, lam in r["transfers"] if dst in tipish and not tip_service(dst))
            for svc, lam in known:
                svc_cnt[svc] += 1
            payer = r["payer"]
            # attribuzione dei costi di rete al pagatore
            w[payer]["netfee"] += r["fee"] or 0
            w[payer]["tip"] += tips + other
            for tr in r["trades"]:
                if tr["mint"] != mint:
                    continue
                u = tr["user"]
                fee = (tr.get("fee") or 0) + (tr.get("creator_fee") or 0)
                if tr["is_buy"]:
                    w[u]["in"] += tr["sol_amount"] + fee
                    w[u]["tok"] += tr["token_amount"]
                    w[u]["buys"] += 1
                    if w[u]["first"] is None:
                        w[u]["first"] = off
                        cat = "z_creator" if u == creator or u == t.get("user") else category(off, in_create, contiguous)
                        w[u]["cat"] = cat
                        first_buy_by_off[min(off, 999)].append(tr["sol_amount"])
                        if cat not in ("z_creator",):
                            fee_by_cat[cat].append(r["fee"] or 0)
                            tip_by_cat[cat].append(tips)
                            other_by_cat[cat].append(other)
                            svc_by_cat[cat][",".join(sorted({svc for svc, _ in known})) or "nessuno"] += 1
                            progs_by_cat[cat][tuple(sorted(set(p for p in r["top"] if not p.startswith("ComputeBudget"))))] += 1
                else:
                    w[u]["out"] += tr["sol_amount"] - fee
                    w[u]["tok"] -= tr["token_amount"]
                    w[u]["sells"] += 1
                last_state = (tr["virtual_sol_reserves"], tr["virtual_token_reserves"])
        # valuta il residuo allo stato finale osservato
        for u, x in w.items():
            if x["cat"] is None:
                continue
            resid = 0
            if x["tok"] > 0 and last_state:
                vs, vt = last_state
                gross = x["tok"] * vs // (vt + x["tok"])
                resid = gross - (-(-gross * 95 // 10000)) - (-(-gross * 30 // 10000))
            pnl = x["out"] + resid - x["in"] - x["netfee"] - x["tip"]
            if t["truncated"]:
                continue
            pnl_by_cat[x["cat"]].append((pnl, x["in"], x["netfee"] + x["tip"], t["truncated"]))
            pnl_real_by_cat[x["cat"]].append(x["out"] - x["in"] - x["netfee"] - x["tip"])
            wallets_by_cat[x["cat"]] += 1
        per_token.append({"mint": mint, "n_s0": n_s0, "n_bundle": n_bundle, "n_ok": t["n_ok"], "n_sigs": t["n_sigs"]})

    out = {"tokens": n_tokens, "truncated_tokens": truncated, "s0_struct": dict(s0_struct)}
    out["first_buyers_by_offset"] = {str(k): {"n": len(v), "sol_p50": pct(v, 50) / 1e9 if v else None}
                                     for k, v in sorted(first_buy_by_off.items()) if k <= 25}
    out["ok_by_offset"] = {str(k): ok_by_off[k] for k in range(0, 21)}
    out["fail_by_offset"] = {str(k): fail_by_off[k] for k in range(0, 21)}
    out["fail_total"], out["ok_total"] = sum(fail_by_off.values()), sum(ok_by_off.values())
    out["err_codes"] = err_codes.most_common(12)
    out["tip_dest_top"] = [(d, n, pct(dest_amt[d], 50)) for d, n in top_dest[:25]]
    cats = sorted(pnl_by_cat)
    tab = {}
    for c in cats:
        v = pnl_by_cat[c]
        pn = [p for p, _, _, _ in v]
        inv = [i for _, i, _, _ in v]
        cst = [x for _, _, x, _ in v]
        tab[c] = {"wallets": len(v), "sum_pnl_SOL": round(sum(pn) / 1e9, 3), "sum_in_SOL": round(sum(inv) / 1e9, 3),
                  "roi": round(sum(pn) / sum(inv), 4) if sum(inv) else None,
                  "win_rate": round(sum(1 for p in pn if p > 0) / len(pn), 3),
                  "pnl_p50_SOL": round(pct(pn, 50) / 1e9, 4), "net_costs_SOL": round(sum(cst) / 1e9, 3),
                  "realized_only_sum_SOL": round(sum(pnl_real_by_cat[c]) / 1e9, 3),
                  "fee_p50_lamports": pct(fee_by_cat.get(c, []), 50), "fee_p90_lamports": pct(fee_by_cat.get(c, []), 90),
                  "tip_p50_lamports": pct(tip_by_cat.get(c, []), 50),
                  "tip_p90_lamports": pct(tip_by_cat.get(c, []), 90),
                  "other_transfers_p50_lamports": pct(other_by_cat.get(c, []), 50),
                  "tip_services": svc_by_cat[c].most_common(5)}
    out["pnl_by_category"] = tab
    out["tip_services_total"] = svc_cnt.most_common()
    out["programs_by_category"] = {c: [(list(k), n) for k, n in progs_by_cat[c].most_common(4)] for c in cats}
    print(json.dumps(out, indent=1))


main()
