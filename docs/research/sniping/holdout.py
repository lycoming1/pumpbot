#!/usr/bin/env python3
"""Verifica dell'ipotesi pre-registrata (PREREG.md) sui soli token dell'holdout (S0 > CUTOFF).
Uso: holdout.py <recdir>
"""
import sys

REC = sys.argv[1]
sys.argv = [sys.argv[0], REC, "0.1", "adj"]
import sim  # noqa: E402

CUTOFF = 460_990_000
COST = 0.002 / 0.1


def s0_in(r):
    p = sim.Path(r)
    return (p.at(0)[0] - r["post_create_vs"]) / 1e9


def line(sel, L, rule):
    res = sim.run(sel, Ls=(L,), rules={rule: sim.RULES[rule]})
    s, ci = res[L][rule]
    if not s:
        return f"L={L} {rule}: n=0"
    c = f"[{(ci[0]-COST)*100:+.1f}; {(ci[1]-COST)*100:+.1f}]" if ci else "[n/d]"
    return (f"L={L} {rule:14s} n={s['n']:4d} media netta {(s['mean']-COST)*100:+.1f}% IC95 {c}  "
            f"lorda {s['mean']*100:+.1f}%  mediana {s['med']*100:+.1f}%  win {s['win']*100:.0f}%  "
            f"senza top1% {(s['mean_wo_top1']-COST)*100:+.1f}%")


def main():
    rows = [r for r in sim.load() if r["s0"] > CUTOFF]
    print(f"holdout: {len(rows)} token utilizzabili (S0 > {CUTOFF})")
    if not rows:
        return
    print(f"slot holdout: {min(r['s0'] for r in rows)} - {max(r['s0'] for r in rows)}")
    big = [r for r in rows if s0_in(r) >= 5]
    print(f"\nPRIMARIO (s0_in >= 5 SOL: {len(big)} token, {len(big)/len(rows)*100:.0f}%)")
    print("  " + line(big, 2, "TP20/SL20/60s"))
    print("\nsecondari:")
    for L in (1, 3):
        print("  " + line(big, L, "TP20/SL20/60s"))
    for L in (1, 2, 3):
        print("  " + line(big, L, "H15"))
    for thr in (3, 10):
        sel = [r for r in rows if s0_in(r) >= thr]
        print(f"  soglia {thr}: " + line(sel, 2, "TP20/SL20/60s"))
    print("  tutti i token: " + line(rows, 2, "TP20/SL20/60s"))
    print("  tutti i token: " + line(rows, 2, "H15"))


main()
