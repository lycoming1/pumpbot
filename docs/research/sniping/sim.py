#!/usr/bin/env python3
"""Simulazione dello sniping sui token di tokens.jsonl (build.py).

Ingresso: B SOL exact-in sullo stato a fine slot S0+L (L=-1: subito dopo la tx di create, prezzo "insider").
Uscite (esecuzione sullo stato a fine slot, con ritardo Ls = max(L,1) tra decisione ed esecuzione):
  H<n>          vendita a fine slot entry+n
  TP/SL/T       take profit / stop loss sul valore di liquidazione netto (LV), time stop T slot
  TR            trailing: attivo da +a, esce a -b dal picco di LV
  ORACLE        miglior LV nella finestra (limite superiore, non realizzabile)
Modalità di impatto: 'adj' sposta lo stato osservato del nostro acquisto (vs+net), 'obs' usa lo stato osservato.
Rendimento r = (sol_out - B) / B, prima dei costi fissi di rete (riportati a parte per profilo).

Uso: sim.py <recdir> [B_sol] [mode]
"""
import bisect, json, math, os, random, statistics, sys

REC = sys.argv[1]
B = int(float(sys.argv[2]) * 1e9) if len(sys.argv) > 2 else 100_000_000
MODE = sys.argv[3] if len(sys.argv) > 3 else "adj"
FEE_BPS = 125


def buy(vs, vt, b):
    net = b * 10000 // (10000 + FEE_BPS)
    tok = net * vt // (vs + net)
    return net, tok


def sell(vs, vt, q):
    gross = q * vs // (vt + q)
    return gross - (-(-gross * 95 // 10000)) - (-(-gross * 30 // 10000))


class Path:
    def __init__(self, row):
        self.off = [p[0] for p in row["path"]]
        self.st = [(p[1], p[2], p[3]) for p in row["path"]]
        self.pre = (row["post_create_vs"], row["post_create_vt"], 0)

    def at(self, off):
        """stato a fine slot S0+off (ultimo noto <= off); off=-1 -> dopo la tx di create"""
        if off < 0:
            return self.pre
        i = bisect.bisect_right(self.off, off) - 1
        if i < 0:
            return self.pre
        return self.st[i]

    def changes(self, a, b):
        """offset in (a, b] in cui lo stato cambia"""
        i = bisect.bisect_right(self.off, a)
        j = bisect.bisect_right(self.off, b)
        return self.off[i:j]


def lv(path, off, q, net):
    vs, vt, comp = path.at(off)
    if MODE == "adj":
        k = vs * vt
        vs = vs + net
        vt = k // vs
    return sell(vs, vt, q)


def simulate(row, L, rules, maxh):
    path = Path(row)
    vs, vt, comp = path.at(L)
    if comp:
        return None
    net, q = buy(vs, vt, B)
    e = L if L >= 0 else 0  # offset di ingresso (fine slot)
    ls = max(L, 1)  # ritardo di esecuzione delle uscite
    out = {}
    end = e + maxh
    for name, spec in rules.items():
        kind = spec[0]
        if kind == "H":
            out[name] = lv(path, e + spec[1], q, net)
        elif kind == "TPSL":
            tp, sl, T = spec[1], spec[2], spec[3]
            res = None
            for o in path.changes(e, e + T):
                v = lv(path, o, q, net)
                r = v / B - 1
                if r >= tp or r <= sl:
                    res = lv(path, o + ls, q, net)
                    break
            out[name] = res if res is not None else lv(path, e + T, q, net)
        elif kind == "TR":
            act, dd, sl, T = spec[1], spec[2], spec[3], spec[4]
            peak = None
            res = None
            for o in path.changes(e, e + T):
                v = lv(path, o, q, net)
                r = v / B - 1
                if r <= sl and peak is None:
                    res = lv(path, o + ls, q, net)
                    break
                if r >= act:
                    peak = max(peak or v, v)
                if peak is not None and v <= peak * (1 - dd):
                    res = lv(path, o + ls, q, net)
                    break
            out[name] = res if res is not None else lv(path, e + T, q, net)
        elif kind == "ORACLE":
            best = lv(path, e, q, net)
            for o in path.changes(e, e + spec[1]):
                best = max(best, lv(path, o, q, net))
            out[name] = best
    return {k: (v / B - 1) for k, v in out.items()}


RULES = {
    "H1": ("H", 1), "H5": ("H", 5), "H15": ("H", 15), "H50": ("H", 50), "H150": ("H", 150),
    "H300": ("H", 300), "H700": ("H", 700), "H1400": ("H", 1400),
    "TP20/SL20/60s": ("TPSL", 0.20, -0.20, 283), "TP50/SL25/2m": ("TPSL", 0.50, -0.25, 566),
    "TP100/SL35/5m": ("TPSL", 1.00, -0.35, 1415),
    "TR30/15/SL30/5m": ("TR", 0.30, 0.15, -0.30, 1415),
    "ORACLE5m": ("ORACLE", 1415),
}


def summarize(rs):
    n = len(rs)
    if n == 0:
        return None
    s = sorted(rs)
    mean = sum(s) / n
    cut = max(1, n // 100)
    mean_wo = sum(s[:-cut]) / (n - cut) if n > cut else float("nan")
    win = sum(1 for x in s if x > 0) / n
    return {"n": n, "mean": mean, "med": s[n // 2], "win": win, "p10": s[n // 10], "p90": s[(9 * n) // 10],
            "max": s[-1], "mean_wo_top1": mean_wo}


def boot_ci(rs, groups, iters=2000, seed=7):
    """bootstrap a blocchi (gruppi = finestre temporali)"""
    rnd = random.Random(seed)
    by = {}
    for r, g in zip(rs, groups):
        by.setdefault(g, []).append(r)
    keys = list(by)
    if len(keys) < 3:
        return None
    means = []
    for _ in range(iters):
        tot = 0.0
        cnt = 0
        for _k in keys:
            blk = by[rnd.choice(keys)]
            tot += sum(blk)
            cnt += len(blk)
        means.append(tot / cnt)
    means.sort()
    return [means[int(0.025 * iters)], means[int(0.975 * iters)]]


def load(filter_fn=None):
    rows = []
    for line in open(os.path.join(REC, "tokens.jsonl")):
        r = json.loads(line)
        if r["gap"] or not r["s0_seen"]:
            continue
        if filter_fn and not filter_fn(r):
            continue
        rows.append(r)
    return rows


def run(rows, Ls=(-1, 0, 1, 2, 3, 4, 6, 10, 20), rules=RULES, maxh=1415, label="all"):
    res = {}
    for L in Ls:
        per_rule = {k: [] for k in rules}
        groups = []
        for row in rows:
            r = simulate(row, L, rules, maxh)
            if r is None:
                continue
            for k, v in r.items():
                per_rule[k].append(v)
            groups.append(row["s0"] // 2830)  # blocchi da ~10 minuti
        res[L] = {k: (summarize(v), boot_ci(v, groups)) for k, v in per_rule.items()}
    return res


def fmt(res, rules=RULES):
    lines = []
    for L, rr in res.items():
        lines.append(f"--- L={L}")
        for k in rules:
            s, ci = rr[k]
            if not s:
                continue
            ci_s = f"[{ci[0]*100:+.1f},{ci[1]*100:+.1f}]" if ci else ""
            lines.append(f"{k:18s} n={s['n']:5d} mean={s['mean']*100:+7.2f}% {ci_s:16s} med={s['med']*100:+7.2f}% "
                         f"win={s['win']*100:5.1f}% p10={s['p10']*100:+6.1f}% p90={s['p90']*100:+7.1f}% "
                         f"max={s['max']*100:+8.0f}% mean-top1%={s['mean_wo_top1']*100:+7.2f}%")
    return "\n".join(lines)


if __name__ == "__main__":
    rows = load()
    print(f"token utilizzabili: {len(rows)}  B={B/1e9} SOL  mode={MODE}")
    print(fmt(run(rows)))
