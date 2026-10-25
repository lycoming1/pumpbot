#!/usr/bin/env python3
"""Quanto presto sappiamo di un create: PumpPortal e logsSubscribe(confirmed) rispetto allo slot S0.

Per ogni create con S0 noto (creates.jsonl) e messaggio PumpPortal (pp.jsonl, per mint):
  - ritardo PumpPortal rispetto al primo arrivo di S0 su slotSubscribe (ms)
  - slot più recente già visto all'arrivo del messaggio, meno S0 (quanti slot sono già passati)
Uso: latency.py <recdir>
"""
import bisect, json, os, sys

REC = sys.argv[1]


def pct(a, p):
    a = sorted(a)
    return a[min(len(a) - 1, int(p / 100 * len(a)))] if a else None


slots = []
for line in open(os.path.join(REC, "slots.csv")):
    if line.endswith("\n"):
        s, t = line.split(",")
        slots.append((int(t), int(s)))
slots.sort()
slot_t = {}
for t, s in slots:
    slot_t.setdefault(s, t)
ts = [t for t, _ in slots]
run_max = []
m = 0
for _, s in slots:
    m = max(m, s)
    run_max.append(m)

s0_by_mint = {}
tlog_by_mint = {}
for line in open(os.path.join(REC, "creates.jsonl")):
    if not line.endswith("\n"):
        continue
    r = json.loads(line)
    if r.get("err") is not None:
        continue
    for e in r.get("ev", []):
        if e[1] == "CreateEvent":
            s0_by_mint[e[2]["mint"]] = r["slot"]
            tlog_by_mint[e[2]["mint"]] = r["t"]

d_ms, d_slots, d_logs = [], [], []
n_pp = 0
for line in open(os.path.join(REC, "pp.jsonl")):
    if not line.endswith("\n"):
        continue
    r = json.loads(line)
    msg = r["m"]
    if not isinstance(msg, dict) or msg.get("txType") != "create":
        continue
    n_pp += 1
    mint = msg.get("mint")
    s0 = s0_by_mint.get(mint)
    if s0 is None or s0 not in slot_t:
        continue
    t = r["t"]
    d_ms.append((t - slot_t[s0]) / 1e6)
    i = bisect.bisect_right(ts, t) - 1
    if i >= 0:
        d_slots.append(run_max[i] - s0)
    d_logs.append((tlog_by_mint[mint] - t) / 1e6)

print(json.dumps({
    "pp_creates": n_pp, "matched": len(d_ms),
    "pp_minus_S0_firstseen_ms_p10_p50_p90": [round(pct(d_ms, q)) for q in (10, 50, 90)] if d_ms else None,
    "slots_already_seen_at_pp_arrival_p10_p50_p90": [pct(d_slots, q) for q in (10, 50, 90)] if d_slots else None,
    "logs_confirmed_minus_pp_ms_p10_p50_p90": [round(pct(d_logs, q)) for q in (10, 50, 90)] if d_logs else None,
}, indent=1))
