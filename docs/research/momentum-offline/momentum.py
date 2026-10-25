#!/usr/bin/env python3
"""Prima prova offline del momentum (Fase 1, DESIGN.md §5) sugli stati delle curve registrati il 24 ott 2026.

Regole fissate PRIMA di guardare i risultati (25 ott 2026):
- Universo: curve con quote SOL, non mayhem, non cashback, invariante vq - rq = 30 SOL, non complete.
- Bande: A = rq >= 5 SOL e progresso <= 30% (cioè 19,3-30%); B = progresso 5-30%.
- Segnali valutati a fine di ogni slot in cui la curva cambia (dati <= slot t, nessun lookahead):
    V3  = progresso(t) - progresso(t-50) >= 3 pp e seconda differenza > 0
          (la condizione "nessun trade > 40% dell'inflow" non è calcolabile dai soli stati: omessa)
    V3s = come V3 ma >= 5 pp
    V0  = controllo: ingresso casuale deterministico sugli stessi slot idonei, con frequenza pari a V3
- Esecuzione: ingresso a fine slot t+L, L in {1,3,6}; 0,25 SOL exact-in; impatto 'adj'; uscite eseguite L slot
  dopo la decisione. Una posizione per curva, cooldown 50 slot dopo l'uscita. Si salta il segnale se un buco del
  flusso tocca [t-50, t+L].
- Uscite: E1 TP+20/SL-10/283 slot; E2 TP+50/SL-20/1415; E3 trailing da +15% a -10% dal picco, SL-15%, 1415.
  Sempre attiva l'uscita per inattività (150 slot senza cambi). Completamento della curva: uscita allo stato finale.
- Costi: competitive 0,00232 SOL a giro (0,93%), economy 0,00008. K = 2 bande x 3 varianti x 3 L x 3 uscite = 54.
Uso: momentum.py <recdir>
"""
import bisect, collections, hashlib, json, os, random, sys

REC = sys.argv[1]
B = 250_000_000
RT0 = 793_100_000_000_000
COST_COMP, COST_ECO = 0.00232 / 0.25, 0.00008 / 0.25
ZERO = "11111111111111111111111111111111"
WSOL = "So11111111111111111111111111111111111111112"
EXITS = {"E1": ("TPSL", 0.20, -0.10, 283), "E2": ("TPSL", 0.50, -0.20, 1415), "E3": ("TR", 0.15, 0.10, -0.15, 1415)}


def buy(vs, vt, b):
    net = b * 10000 // 10125
    return net, net * vt // (vs + net)


def sell(vs, vt, q):
    g = q * vs // (vt + q)
    return g - (-(-g * 95 // 10000)) - (-(-g * 30 // 10000))


def load():
    static = {}
    for line in open(os.path.join(REC, "curve_static.jsonl")):
        s = json.loads(line)
        if s.get("mayhem") == 0 and s.get("cashback") == 0 and s.get("quote_mint") in (ZERO, WSOL):
            static[s["curve"]] = s
    data = collections.defaultdict(list)
    seen = set()
    last = 0
    with open(os.path.join(REC, "curves.csv")) as f:
        for line in f:
            if not line.endswith("\n"):
                break
            p = line[:-1].split(",")
            if len(p) != 8:
                continue
            s = int(p[1])
            seen.add(s)
            last = max(last, s)
            if p[2] in static:
                vt, vq, rt, rq, c = int(p[3]), int(p[4]), int(p[5]), int(p[6]), int(p[7])
                if vq - rq != 30_000_000_000:
                    continue
                data[p[2]].append((s, vt, vq, rt, rq, c))
    ss = sorted(seen)
    gaps = [(a + 1, b - 1) for a, b in zip(ss, ss[1:]) if b - a > 4]
    for c in data:
        data[c].sort()
    return data, gaps, last


class Curve:
    def __init__(self, rows):
        self.s = [r[0] for r in rows]
        self.st = rows

    def at(self, slot):
        i = bisect.bisect_right(self.s, slot) - 1
        return self.st[i] if i >= 0 else None


def prog(st):
    return 1 - st[3] / RT0


def in_band(st, band):
    p = prog(st)
    if band == "A":
        return st[4] >= 5_000_000_000 and p <= 0.30
    return 0.05 <= p <= 0.30


def gap_hit(gaps_a, gaps, lo, hi):
    i = bisect.bisect_right(gaps_a, hi)
    for a, b in gaps[max(0, i - 3):i + 1]:
        if not (b < lo or a > hi):
            return True
    return False


def lv(cv, slot, q, net):
    st = cv.at(slot)
    vs, vt = st[2], st[1]
    k = vs * vt
    vs2 = vs + net
    return sell(vs2, k // vs2, q), st[5]


def run_exit(cv, e, q, net, L, spec, last):
    """ritorna (sol_out, slot_uscita) o None se i dati finiscono prima"""
    T = spec[-1]
    end = e + T
    if end + L > last:
        return None
    i = bisect.bisect_right(cv.s, e)
    prev = e
    peak = None
    while i < len(cv.s) and cv.s[i] <= end:
        o = cv.s[i]
        if o - prev > 150:  # inattività
            d = prev + 150
            return lv(cv, d + L, q, net)[0], d + L
        v, comp = lv(cv, o, q, net)
        if comp:
            return v, o
        r = v / B - 1
        if spec[0] == "TPSL":
            if r >= spec[1] or r <= spec[2]:
                return lv(cv, o + L, q, net)[0], o + L
        else:
            act, dd, sl = spec[1], spec[2], spec[3]
            if peak is None and r <= sl:
                return lv(cv, o + L, q, net)[0], o + L
            if r >= act:
                peak = max(peak or v, v)
            if peak is not None and v <= peak * (1 - dd):
                return lv(cv, o + L, q, net)[0], o + L
        prev = o
        i += 1
    if end - prev > 150:
        d = prev + 150
        return lv(cv, d + L, q, net)[0], d + L
    return lv(cv, end + L, q, net)[0], end + L


def signals(cv, band, gaps_a, gaps):
    """slot idonei e segnali V3/V3s per una curva"""
    elig, v3, v3s = [], [], []
    first = cv.s[0]
    for idx, t in enumerate(cv.s):
        st = cv.st[idx]
        if st[5] or t - 50 < first or not in_band(st, band):
            continue
        if gap_hit(gaps_a, gaps, t - 50, t + 6):
            continue
        p2 = prog(st)
        p1 = prog(cv.at(t - 25))
        p0 = prog(cv.at(t - 50))
        elig.append(t)
        if (p2 - p1) > (p1 - p0):
            if p2 - p0 >= 0.03:
                v3.append(t)
            if p2 - p0 >= 0.05:
                v3s.append(t)
    return elig, v3, v3s


def trade(curves, sigs, L, spec, last):
    out = []
    for c, ts in sigs.items():
        cv = curves[c]
        busy_until = -1
        for t in ts:
            if t <= busy_until:
                continue
            e = t + L
            st = cv.at(e)
            if st[5] or e > last:
                continue
            net, q = buy(st[2], st[1], B)
            res = run_exit(cv, e, q, net, L, spec, last)
            if res is None:
                continue
            sol, xs = res
            out.append((sol / B - 1, t))
            busy_until = xs + 50
    return out


def stats(rs, block=8490, iters=2000):
    if not rs:
        return None
    v = sorted(r for r, _ in rs)
    n = len(v)
    pos = sum(x - COST_COMP for x in v if x - COST_COMP > 0)
    neg = -sum(x - COST_COMP for x in v if x - COST_COMP <= 0)
    by = collections.defaultdict(list)
    for r, t in rs:
        by[t // block].append(r)
    keys = list(by)
    rnd = random.Random(7)
    ms = []
    if len(keys) >= 3:
        for _ in range(iters):
            tot = cnt = 0
            for _k in keys:
                b = by[rnd.choice(keys)]
                tot += sum(b)
                cnt += len(b)
            ms.append(tot / cnt)
        ms.sort()
    cut = max(1, n // 100)
    return {"n": n, "mean": sum(v) / n, "med": v[n // 2], "win": sum(1 for x in v if x > 0) / n,
            "pf": pos / neg if neg else float("inf"), "wo1": sum(v[:-cut]) / (n - cut) if n > cut else float("nan"),
            "ci": (ms[int(0.025 * iters)], ms[int(0.975 * iters)]) if ms else None}


def main():
    data, gaps, last = load()
    gaps_a = [a for a, _ in gaps]
    curves = {c: Curve(r) for c, r in data.items() if len(r) >= 3}
    print(f"curve SOL non-mayhem con dati: {len(curves)}; buchi del flusso: {len(gaps)}")
    for band in ("A", "B"):
        elig, v3, v3s = {}, {}, {}
        for c, cv in curves.items():
            e, a, b = signals(cv, band, gaps_a, gaps)
            if e:
                elig[c], v3[c], v3s[c] = e, a, b
        n_el = sum(len(x) for x in elig.values())
        n_v3 = sum(len(x) for x in v3.values())
        frac = n_v3 / n_el if n_el else 0
        v0 = {c: [t for t in ts if int.from_bytes(hashlib.sha256(f"{c}{t}".encode()).digest()[:8], "big") / 2**64 < frac]
              for c, ts in elig.items()}
        print(f"\n=== Banda {band}: slot idonei {n_el}, segnali V3 {n_v3} su {sum(1 for x in v3.values() if x)} curve, "
              f"V3s {sum(len(x) for x in v3s.values())}, V0 {sum(len(x) for x in v0.values())}")
        for name, sg in (("V3", v3), ("V3s", v3s), ("V0", v0)):
            for L in (1, 3, 6):
                for ex, spec in EXITS.items():
                    s = stats(trade(curves, sg, L, spec, last))
                    if not s:
                        print(f"{name:4s} L={L} {ex}: n=0")
                        continue
                    ci = s["ci"]
                    cis = f"[{(ci[0]-COST_COMP)*100:+.1f};{(ci[1]-COST_COMP)*100:+.1f}]" if ci else ""
                    print(f"{name:4s} L={L} {ex}: n={s['n']:4d} lordo {s['mean']*100:+6.2f}% netto(comp) "
                          f"{(s['mean']-COST_COMP)*100:+6.2f}% {cis:16s} med {s['med']*100:+6.1f}% win {s['win']*100:3.0f}% "
                          f"PF netto {s['pf']:.2f} senza top1% {(s['wo1']-COST_COMP)*100:+6.2f}%")


main()
