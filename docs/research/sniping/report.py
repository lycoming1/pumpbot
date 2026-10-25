#!/usr/bin/env python3
"""Tabelle markdown per il documento: rendimento medio netto per L e regola d'uscita.

Costi fissi per andata e ritorno (SOL): nessuno, economy 0,00008, sniper 0,002 (buy 0,0015 + sell 0,0005).
Uso: report.py <recdir>
"""
import sys

REC = sys.argv[1]
sys.argv = [sys.argv[0], REC, "0.1", "adj"]
import sim  # noqa: E402

PROFILES = {"lordo": 0.0, "economy": 0.00008, "sniper": 0.002}
RULES = ["H5", "H15", "H50", "H150", "H1400", "TP20/SL20/60s", "TR30/15/SL30/5m", "ORACLE5m"]
LS = [-1, 0, 1, 2, 3, 6]
LABEL = {"H5": "1 s", "H15": "3 s", "H50": "10 s", "H150": "32 s", "H1400": "5 min", "TP20/SL20/60s": "TP+20/SL−20/60 s",
         "TR30/15/SL30/5m": "trailing", "ORACLE5m": "oracolo 5 min"}


def table(rows, b, mode, profile):
    sim.B = int(b * 1e9)
    sim.MODE = mode
    rules = {k: sim.RULES[k] for k in RULES}
    res = sim.run(rows, Ls=LS, rules=rules)
    cost = PROFILES[profile] / b
    out = [f"\n**B = {b} SOL, impatto '{mode}', costi '{profile}' ({PROFILES[profile]} SOL a giro = {cost*100:.2f}%)**\n",
           "| L | " + " | ".join(LABEL[k] for k in RULES) + " |", "|---|" + "---|" * len(RULES)]
    for L in LS:
        cells = []
        for k in RULES:
            s, ci = res[L][k]
            if not s:
                cells.append("")
                continue
            m = (s["mean"] - cost) * 100
            c = f" [{(ci[0]-cost)*100:+.1f}; {(ci[1]-cost)*100:+.1f}]" if ci else ""
            cells.append(f"{m:+.1f}%{c}")
        lab = "−1 (tx di create)" if L == -1 else str(L)
        out.append(f"| {lab} | " + " | ".join(cells) + " |")
    n = res[LS[2]][RULES[0]][0]["n"]
    out.append(f"\nn = {n} token")
    return "\n".join(out)


def medians(rows):
    sim.B = 100_000_000
    sim.MODE = "adj"
    rules = {k: sim.RULES[k] for k in RULES}
    res = sim.run(rows, Ls=[1, 2, 3], rules=rules)
    out = ["\n**Mediana, win rate e quota senza il top 1% (B = 0,1 SOL, lordo)**\n",
           "| L | regola | media | mediana | win rate | media senza top 1% | p90 |", "|---|---|---|---|---|---|---|"]
    for L in [1, 2, 3]:
        for k in ["H15", "H150", "TP20/SL20/60s"]:
            s, _ = res[L][k]
            out.append(f"| {L} | {LABEL[k]} | {s['mean']*100:+.1f}% | {s['med']*100:+.1f}% | {s['win']*100:.0f}% | "
                       f"{s['mean_wo_top1']*100:+.1f}% | {s['p90']*100:+.1f}% |")
    return "\n".join(out)


if __name__ == "__main__":
    rows = sim.load()
    print(table(rows, 0.1, "adj", "lordo"))
    print(table(rows, 0.1, "adj", "sniper"))
    print(table(rows, 0.25, "adj", "sniper"))
    print(table(rows, 0.1, "obs", "lordo"))
    print(medians(rows))
