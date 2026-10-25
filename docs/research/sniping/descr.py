import sys, json
REC = sys.argv[1]
sys.argv = [sys.argv[0], REC, "0.1", "adj"]
import sim
rows = sim.load()
K = 30_000_000_000 * 1_073_000_000_000_000
def pct(a, ps=(10, 25, 50, 75, 90, 99)):
    a = sorted(a); return [round(a[min(len(a)-1, int(p/100*len(a)))], 3) for p in ps]
dev = [r["dev_sol"]/1e9 for r in rows]
s0in = []; px = {L: [] for L in (0, 1, 2, 3, 6, 10)}; alive = {H: 0 for H in (15, 150, 1415)}
mc0 = []; grad = 0; trades_after = []
for r in rows:
    p = sim.Path(r)
    pre = r["post_create_vs"]
    s0in.append((p.at(0)[0] - pre)/1e9)
    for L in px:
        px[L].append((p.at(L)[0]/pre)**2 - 1)   # prezzo relativo (vs^2) rispetto a dopo la tx di create
    mc0.append(p.at(0)[0]**2 * 1e6 / K)  # mcap in SOL = vs^2/K*1e6? (vs in lamport: mcap = vs*1e15/vt/1e9)
    vs0 = p.at(2)[0]
    for H in alive:
        alive[H] += any(o > 2 and o <= 2 + H for o in p.off)
    if any(st[2] for st in p.st): grad += 1
    trades_after.append(sum(1 for o in p.off if 2 < o <= 1417))
n = len(rows)
print("token", n)
print("dev buy SOL (tx di create) p10..p99", pct(dev), "zero:", round(sum(1 for x in dev if x < 0.001)/n, 3))
print("SOL entrati in S0 oltre la tx di create p10..p99", pct(s0in), "quota >0:", round(sum(1 for x in s0in if x > 0.001)/n, 3), ">=5:", round(sum(1 for x in s0in if x >= 5)/n, 3))
for L, v in px.items():
    print(f"prezzo a fine S0+{L} vs subito dopo la tx di create: media {sum(v)/n*100:+.1f}% p50 {pct(v,(50,))[0]*100:+.1f}% p90 {pct(v,(90,))[0]*100:+.1f}%")
for H, c in alive.items():
    print(f"token con almeno un cambio di stato tra S0+3 e S0+{2+H}: {c/n*100:.0f}%")
print("slot con cambio di stato nei 5 min dopo S0+2: p50/p90", pct(trades_after, (50, 90)))
print("graduati entro 10 min:", grad, f"({grad/n*100:.2f}%)")
