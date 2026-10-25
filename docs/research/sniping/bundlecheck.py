import sys
REC = sys.argv[1]
sys.argv = [sys.argv[0], REC, "0.1", "adj"]
import sim
rows = sim.load()
rows.sort(key=lambda r: r["s0"])
def s0in(r):
    p = sim.Path(r); return (p.at(0)[0] - r["post_create_vs"]) / 1e9
rules = {k: sim.RULES[k] for k in ["H5", "H15", "H50", "H150", "TP20/SL20/60s", "TP50/SL25/2m", "TR30/15/SL30/5m"]}
for thr in (3, 5, 10):
    sel = [r for r in rows if s0in(r) >= thr]
    print(f"\n=== s0_in >= {thr} SOL: {len(sel)} token su {len(rows)} ({len(sel)/len(rows)*100:.0f}%)")
    res = sim.run(sel, Ls=(0, 1, 2, 3, 6), rules=rules)
    for L, rr in res.items():
        print(f"L={L}: " + " | ".join(f"{k} {s['mean']*100:+.1f}% [{ci[0]*100:+.1f};{ci[1]*100:+.1f}] med {s['med']*100:+.1f}% w{s['win']*100:.0f}%" if ci else f"{k} {s['mean']*100:+.1f}%" for k, (s, ci) in rr.items()))
# stabilità nel tempo: blocchi da ~20 min
sel = [r for r in rows if s0in(r) >= 5]
import collections
blk = collections.defaultdict(list)
for r in sel:
    x = sim.simulate(r, 1, {"TP": sim.RULES["TP20/SL20/60s"], "H15": sim.RULES["H15"]}, 1415)
    if x: blk[r["s0"] // 5660].append(x)
print("\nper blocchi da 20 min (L=1):")
for b, v in sorted(blk.items()):
    print(b, len(v), "TP20 mean %+.1f%%" % (sum(x["TP"] for x in v) / len(v) * 100), "H15 mean %+.1f%%" % (sum(x["H15"] for x in v) / len(v) * 100))
