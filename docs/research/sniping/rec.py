#!/usr/bin/env python3
"""Registratore per l'analisi dello sniping su pump.fun (sola lettura, dati pubblici).

Flussi:
  curves.csv     programSubscribe(pump, BondingCurve) confirmed: stato a fine slot di ogni curva toccata
  curve_static.jsonl  campi statici di ogni curva vista (creator, mayhem, quote_mint, ...)
  creates.jsonl  logsSubscribe(mentions=mint authority TSLvdd) confirmed: create con slot, eventi decodificati, log
  pp.jsonl       PumpPortal gratuito subscribeNewToken + subscribeMigration (una sola connessione)
  slots.csv      slotSubscribe: primo arrivo di ogni slot
  amm.jsonl      logsSubscribe(mentions=mint) per 15 min dopo ogni migrazione: eventi PumpSwap
  health.jsonl   contatori ogni 60 s, riconnessioni

Uso: rec.py <outdir> <secondi>
"""
import asyncio, base64, json, os, struct, sys, time

sys.path.insert(0, "/home/kraken/docker/pumpbot/docs/research")
import pumpspec_reference as ps  # noqa: E402
import websockets  # noqa: E402

WSS = "wss://api.mainnet-beta.solana.com"
WSS2 = "wss://api.mainnet.solana.com"
PP = "wss://pumpportal.fun/api/data"
PUMP = ps.PUMP
MINT_AUTH = "TSLvdd1pWpHVjahSpsvCXUbgwsL3JAcvokwaKt1eokM"
WSOL = "So11111111111111111111111111111111111111112"
AMM_WATCH_SECS = 600
MAX_AMM_SUBS = 30

OUT = sys.argv[1]
DUR = float(sys.argv[2])
os.makedirs(OUT, exist_ok=True)
T_END = time.time() + DUR


def opener(name):
    return open(os.path.join(OUT, name), "a", buffering=1)


f_curves = opener("curves.csv")
f_static = opener("curve_static.jsonl")
f_creates = opener("creates.jsonl")
f_pp = opener("pp.jsonl")
f_slots = opener("slots.csv")
f_amm = opener("amm.jsonl")
f_health = opener("health.jsonl")

cnt = {k: 0 for k in ("curve", "curve_dup", "create_tx", "create_ok", "pp", "slot", "amm", "amm_subs")}
reconn = {k: 0 for k in ("curves", "curves2", "creates", "creates2", "pp", "slots", "amm")}
seen_static = set()
seen_slots = set()
import collections as _c
_dedup = _c.OrderedDict()


def first_time(key):
    if key in _dedup:
        return False
    _dedup[key] = 1
    if len(_dedup) > 300000:
        _dedup.popitem(last=False)
    return True
amm_queue = asyncio.Queue()


def now_ns():
    return time.time_ns()


def health(kind, **kw):
    f_health.write(json.dumps({"t": time.time(), "kind": kind, **kw}) + "\n")


async def run_ws(name, url, subs, handler):
    """Connessione con riconnessione; subs = lista di richieste da inviare dopo connect."""
    backoff = 1.0
    while time.time() < T_END:
        try:
            async with websockets.connect(url, max_size=None, ping_interval=20, ping_timeout=20,
                                          open_timeout=15) as ws:
                for s in subs:
                    await ws.send(json.dumps(s))
                health("connect", stream=name)
                backoff = 1.0
                while time.time() < T_END:
                    try:
                        raw = await asyncio.wait_for(ws.recv(), timeout=min(30, max(0.1, T_END - time.time())))
                    except asyncio.TimeoutError:
                        if time.time() >= T_END:
                            break
                        health("idle30", stream=name)
                        continue
                    handler(raw, now_ns())
        except Exception as e:  # noqa: BLE001
            reconn[name] += 1
            health("error", stream=name, err=f"{type(e).__name__}: {str(e)[:200]}")
            await asyncio.sleep(backoff)
            backoff = min(30, backoff * 2)


def on_curve(raw, t):
    m = json.loads(raw)
    if "params" not in m:
        health("sub_reply", stream="curves", msg=raw[:200])
        return
    v = m["params"]["result"]
    slot = v["context"]["slot"]
    pk = v["value"]["pubkey"]
    d = base64.b64decode(v["value"]["account"]["data"][0])
    if len(d) < 49:
        return
    vt, vq, rt, rq, sup = struct.unpack_from("<QQQQQ", d, 8)
    complete = d[48]
    if not first_time(("c", slot, pk)):
        cnt["curve_dup"] += 1
        return
    f_curves.write(f"{t},{slot},{pk},{vt},{vq},{rt},{rq},{complete}\n")
    cnt["curve"] += 1
    if pk not in seen_static:
        seen_static.add(pk)
        st = {"curve": pk, "len": len(d), "supply": sup, "first_slot": slot}
        if len(d) >= 115:
            st.update(creator=ps.b58e(d[49:81]), mayhem=d[81], cashback=d[82], quote_mint=ps.b58e(d[83:115]))
        if len(d) >= 142:
            st.update(creator_fee_bps=struct.unpack_from("<Q", d, 115)[0], holder_reward=d[124], depth=d[141])
        if len(d) >= 150:
            st.update(initial_virtual_quote=struct.unpack_from("<Q", d, 142)[0])
        f_static.write(json.dumps(st) + "\n")


def decode_logs(logs):
    evs, trunc = ps.log_events({"meta": {"logMessages": logs}})
    out = []
    for top, b in evs:
        if top not in (ps.PUMP, ps.AMM):
            continue
        name, e = ps.decode_event(b)
        if name == "TradeEvent":
            e = {k: e.get(k) for k in ("mint", "sol_amount", "token_amount", "is_buy", "user", "virtual_sol_reserves",
                                       "virtual_token_reserves", "real_sol_reserves", "real_token_reserves",
                                       "fee_basis_points", "creator_fee_basis_points", "ix_name", "mayhem_mode",
                                       "quote_mint", "quote_amount", "virtual_quote_reserves", "real_quote_reserves")}
        elif name in ("BuyEvent", "SellEvent"):
            keep = ("timestamp", "base_amount_out", "base_amount_in", "pool_base_token_reserves",
                    "pool_quote_token_reserves", "quote_amount_in", "quote_amount_out", "user_quote_amount_in",
                    "user_quote_amount_out", "lp_fee_basis_points", "protocol_fee_basis_points",
                    "coin_creator_fee_basis_points", "pool", "user", "ix_name", "virtual_quote_reserves",
                    "base_supply", "lp_fee", "protocol_fee", "coin_creator_fee")
            e = {k: e.get(k) for k in keep if k in e}
        else:
            e = {k: v for k, v in e.items() if not k.startswith("_")}
        out.append([top[:4], name, e])
    top_progs = []
    depth = 0
    for line in logs:
        if line.startswith("Program ") and " invoke [" in line:
            if line.endswith("invoke [1]"):
                top_progs.append(line.split()[1])
    return out, trunc, top_progs


def on_create(raw, t):
    m = json.loads(raw)
    if "params" not in m:
        health("sub_reply", stream="creates", msg=raw[:200])
        return
    v = m["params"]["result"]
    val = v["value"]
    if not first_time(("s", val["signature"])):
        return
    cnt["create_tx"] += 1
    rec = {"t": t, "slot": v["context"]["slot"], "sig": val["signature"], "err": val["err"]}
    if val["err"] is None:
        cnt["create_ok"] += 1
        evs, trunc, progs = decode_logs(val["logs"] or [])
        rec.update(ev=evs, trunc=trunc, progs=progs, logs=val["logs"])
    f_creates.write(json.dumps(rec) + "\n")


def on_pp(raw, t):
    try:
        m = json.loads(raw)
    except Exception:  # noqa: BLE001
        return
    cnt["pp"] += 1
    f_pp.write(json.dumps({"t": t, "m": m}) + "\n")
    if isinstance(m, dict) and m.get("txType") == "migrate" and m.get("mint"):
        amm_queue.put_nowait((m["mint"], time.time()))


def on_slot(raw, t):
    m = json.loads(raw)
    if "params" not in m:
        return
    s = m["params"]["result"]["slot"]
    if s not in seen_slots:
        seen_slots.add(s)
        cnt["slot"] += 1
        f_slots.write(f"{s},{t}\n")


async def amm_watcher():
    """Una connessione WS che sottoscrive logs(mentions=mint) per i mint migrati, per AMM_WATCH_SECS."""
    active = {}  # sub_id -> (mint, until)
    pending = {}  # req id -> mint
    req = [100]
    while time.time() < T_END:
        try:
            async with websockets.connect(WSS, max_size=None, ping_interval=20, ping_timeout=20) as ws:
                health("connect", stream="amm")
                # ri-sottoscrive quelli ancora validi dopo una riconnessione
                resub = [(mi, until) for (mi, until) in active.values() if until > time.time()]
                active.clear()
                for mi, until in resub:
                    req[0] += 1
                    pending[req[0]] = (mi, until)
                    await ws.send(json.dumps({"jsonrpc": "2.0", "id": req[0], "method": "logsSubscribe",
                                              "params": [{"mentions": [mi]}, {"commitment": "confirmed"}]}))
                while time.time() < T_END:
                    # nuove migrazioni
                    while not amm_queue.empty() and len(active) + len(pending) < MAX_AMM_SUBS:
                        mi, t0 = amm_queue.get_nowait()
                        req[0] += 1
                        pending[req[0]] = (mi, t0 + AMM_WATCH_SECS)
                        await ws.send(json.dumps({"jsonrpc": "2.0", "id": req[0], "method": "logsSubscribe",
                                                  "params": [{"mentions": [mi]}, {"commitment": "confirmed"}]}))
                    # scadenze
                    for sid, (mi, until) in list(active.items()):
                        if until < time.time():
                            req[0] += 1
                            await ws.send(json.dumps({"jsonrpc": "2.0", "id": req[0], "method": "logsUnsubscribe",
                                                      "params": [sid]}))
                            del active[sid]
                    cnt["amm_subs"] = len(active)
                    try:
                        raw = await asyncio.wait_for(ws.recv(), timeout=1.0)
                    except asyncio.TimeoutError:
                        continue
                    t = now_ns()
                    m = json.loads(raw)
                    if "id" in m and m.get("id") in pending:
                        mi, until = pending.pop(m["id"])
                        if "result" in m:
                            active[m["result"]] = (mi, until)
                            health("amm_sub", mint=mi, sub=m["result"])
                        else:
                            health("amm_sub_err", mint=mi, msg=raw[:200])
                        continue
                    if "params" not in m:
                        continue
                    sid = m["params"]["subscription"]
                    v = m["params"]["result"]
                    val = v["value"]
                    mi = active.get(sid, ("?", 0))[0]
                    rec = {"t": t, "slot": v["context"]["slot"], "sig": val["signature"], "mint": mi}
                    if val["err"] is not None:
                        # solo un contatore compatto per i fallimenti
                        f_amm.write(json.dumps({"t": t, "slot": rec["slot"], "mint": mi, "fail": 1,
                                                "err": json.dumps(val["err"])[:80]}) + "\n")
                        continue
                    evs, trunc, progs = decode_logs(val["logs"] or [])
                    if not evs:
                        f_amm.write(json.dumps({"t": t, "slot": rec["slot"], "mint": mi, "noev": 1}) + "\n")
                        continue
                    rec.update(ev=evs, trunc=trunc, progs=progs)
                    cnt["amm"] += 1
                    f_amm.write(json.dumps(rec) + "\n")
        except Exception as e:  # noqa: BLE001
            reconn["amm"] += 1
            health("error", stream="amm", err=f"{type(e).__name__}: {str(e)[:200]}")
            await asyncio.sleep(2)


async def health_loop():
    last = dict(cnt)
    while time.time() < T_END:
        await asyncio.sleep(60)
        rate = {k: round((cnt[k] - last[k]) / 60, 2) for k in cnt}
        last = dict(cnt)
        health("tick", rate=rate, total=dict(cnt), reconn=dict(reconn), curves_seen=len(seen_static))


async def main():
    health("start", dur=DUR)
    curve_sub = {"jsonrpc": "2.0", "id": 1, "method": "programSubscribe",
                 "params": [PUMP, {"encoding": "base64", "commitment": "confirmed",
                                   "filters": [{"memcmp": {"offset": 0, "bytes": "4y6pru6YvC7"}}]}]}
    create_sub = {"jsonrpc": "2.0", "id": 2, "method": "logsSubscribe",
                  "params": [{"mentions": [MINT_AUTH]}, {"commitment": "confirmed"}]}
    slot_sub = {"jsonrpc": "2.0", "id": 3, "method": "slotSubscribe"}
    await asyncio.gather(
        run_ws("curves", WSS, [curve_sub], on_curve),
        run_ws("curves2", WSS2, [curve_sub], on_curve),
        run_ws("creates", WSS, [create_sub], on_create),
        run_ws("creates2", WSS2, [create_sub], on_create),
        run_ws("slots", WSS, [slot_sub], on_slot),
        run_ws("pp", PP, [{"method": "subscribeNewToken"}, {"method": "subscribeMigration"}], on_pp),
        amm_watcher(),
        health_loop(),
    )
    health("end")


asyncio.run(main())
