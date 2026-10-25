#!/usr/bin/env python3
"""Backfill a livello di trader per un campione di create registrati da rec.py.

Per ogni token campionato (quote SOL, non mayhem), dopo WINDOW_S secondi dal create:
  - getSignaturesForAddress(bonding_curve): tutte le tx (riuscite e fallite) con slot ≤ S0 + W_SLOTS
  - getBlock(S0, transactionDetails=signatures): ordine delle tx nello slot del create (bundle)
  - getTransaction(jsonParsed) delle tx riuscite, fino a MAX_TX per token, in ordine di slot
Scrive bf_tokens.jsonl (una riga per token) e bf_tx.jsonl (una riga per tx).

Uso: backfill.py <recdir> <secondi> [prob_campione]
"""
import hashlib, json, os, random, sys, time, urllib.error, urllib.request

sys.path.insert(0, "/home/kraken/docker/pumpbot/docs/research")
import pumpspec_reference as ps  # noqa: E402

REC = sys.argv[1]
DUR = float(sys.argv[2])
LO, HI = (float(x) for x in sys.argv[3].split(":")) if len(sys.argv) > 3 else (0.0, 0.5)
RPC = sys.argv[4] if len(sys.argv) > 4 else "https://api.mainnet-beta.solana.com"
TAG = sys.argv[5] if len(sys.argv) > 5 else "a"
WINDOW_S = float(os.environ.get("BF_WINDOW_S", "330"))
W_SLOTS = 1415
MAX_TX = 150
MIN_INTERVAL = 0.36  # ~2,8 richieste/s in totale
T_END = time.time() + DUR

f_tok = open(os.path.join(REC, "bf_tokens.jsonl"), "a", buffering=1)
f_tx = open(os.path.join(REC, "bf_tx.jsonl"), "a", buffering=1)
f_log = open(os.path.join(REC, f"bf_log_{TAG}.jsonl"), "a", buffering=1)
DONE = set()
if os.path.exists(os.path.join(REC, "bf_tokens.jsonl")):
    for _l in open(os.path.join(REC, "bf_tokens.jsonl")):
        if _l.endswith("\n"):
            DONE.add(json.loads(_l)["mint"])

_last = [0.0]
stats = {"calls": 0, "429": 0, "err": 0}


def rpc(method, params, retries=6):
    for i in range(retries):
        wait = MIN_INTERVAL - (time.time() - _last[0])
        if wait > 0:
            time.sleep(wait)
        _last[0] = time.time()
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
        try:
            stats["calls"] += 1
            r = urllib.request.urlopen(urllib.request.Request(RPC, data=body, headers={"content-type": "application/json", "user-agent": "pumpbot-research/0.1"}),
                                       timeout=30)
            m = json.loads(r.read())
            if "error" in m:
                code = m["error"].get("code")
                if code in (-32005, 429, -32429):
                    stats["429"] += 1
                    time.sleep(2 * (i + 1))
                    continue
                return {"_error": m["error"]}
            return m.get("result")
        except urllib.error.HTTPError as e:
            if e.code == 429:
                stats["429"] += 1
                time.sleep(3 * (i + 1))
                continue
            stats["err"] += 1
            time.sleep(1 + i)
        except Exception:  # noqa: BLE001
            stats["err"] += 1
            time.sleep(1 + i)
    return {"_error": "retries"}


def sampled(mint):
    # campione deterministico sul mint, riproducibile
    h = int.from_bytes(hashlib.sha256(mint.encode()).digest()[:8], "big") / 2**64
    return LO <= h < HI and mint not in DONE


def creates_iter():
    """Segue creates.jsonl e produce i create SOL non mayhem riusciti."""
    path = os.path.join(REC, "creates.jsonl")
    while not os.path.exists(path):
        time.sleep(5)
    f = open(path)
    buf = ""
    while time.time() < T_END:
        line = f.readline()
        if not line:
            time.sleep(2)
            continue
        buf += line
        if not buf.endswith("\n"):
            continue
        rec = json.loads(buf)
        buf = ""
        if rec.get("err") is not None:
            continue
        ce = [e[2] for e in rec.get("ev", []) if e[1] == "CreateEvent"]
        if not ce:
            continue
        c = ce[0]
        if c.get("is_mayhem_mode") or c.get("quote_mint") not in (ps.ZERO, None):
            continue
        yield rec, c


def tx_compact(sig, tx):
    meta = tx.get("meta") or {}
    msg = tx["transaction"]["message"]
    keys = [k["pubkey"] if isinstance(k, dict) else k for k in msg["accountKeys"]]
    signers = [k["pubkey"] for k in msg["accountKeys"] if isinstance(k, dict) and k.get("signer")]
    top = [ix.get("programId") for ix in msg["instructions"]]
    transfers = []

    def scan(ixs):
        for ix in ixs:
            p = ix.get("parsed")
            if ix.get("program") == "system" and isinstance(p, dict) and p.get("type") == "transfer":
                inf = p["info"]
                transfers.append([inf["source"], inf["destination"], inf["lamports"]])

    scan(msg["instructions"])
    for g in meta.get("innerInstructions") or []:
        scan(g["instructions"])
    evs, trunc = ps.log_events({"meta": {"logMessages": meta.get("logMessages") or []}})
    trades = []
    other = []
    for topp, b in evs:
        if topp != ps.PUMP:
            continue
        name, e = ps.decode_event(b)
        if name == "TradeEvent":
            trades.append({k: e.get(k) for k in ("mint", "sol_amount", "token_amount", "is_buy", "user",
                                                 "virtual_sol_reserves", "virtual_token_reserves",
                                                 "real_sol_reserves", "real_token_reserves", "fee", "creator_fee",
                                                 "ix_name", "mayhem_mode")})
        else:
            other.append(name)
    return {"sig": sig, "slot": tx.get("slot"), "bt": tx.get("blockTime"), "payer": keys[0], "signers": signers,
            "fee": meta.get("fee"), "cu": meta.get("computeUnitsConsumed"), "top": top, "transfers": transfers,
            "trades": trades, "other": other, "trunc": trunc, "ver": tx.get("version")}


def process(rec, c):
    mint, curve, s0 = c["mint"], c["bonding_curve"], rec["slot"]
    sigs = []
    before = None
    for _ in range(10):
        params = {"limit": 1000, "commitment": "confirmed"}
        if before:
            params["before"] = before
        res = rpc("getSignaturesForAddress", [curve, params])
        if not isinstance(res, list):
            f_log.write(json.dumps({"t": time.time(), "mint": mint, "gsfa_err": str(res)[:200]}) + "\n")
            return
        sigs.extend(res)
        if len(res) < 1000 or res[-1]["slot"] < s0:
            break
        before = res[-1]["signature"]
    sigs = [s for s in sigs if s["slot"] <= s0 + W_SLOTS]
    sigs.sort(key=lambda s: (s["slot"], s.get("blockTime") or 0))
    order = {s["signature"]: s.get("transactionIndex") for s in sigs if s["slot"] == s0}
    tx_index = {s["signature"]: s.get("transactionIndex") for s in sigs}
    ok = [s for s in sigs if s.get("err") is None]
    fetched = 0
    for s in ok[:MAX_TX]:
        tx = rpc("getTransaction", [s["signature"], {"encoding": "jsonParsed", "commitment": "confirmed",
                                                      "maxSupportedTransactionVersion": 1}])
        if not isinstance(tx, dict) or "_error" in tx:
            f_tx.write(json.dumps({"mint": mint, "sig": s["signature"], "slot": s["slot"], "fetch_err": str(tx)[:200]}) + "\n")
            continue
        row = tx_compact(s["signature"], tx)
        row["mint"] = mint
        row["blk_idx"] = tx_index.get(s["signature"])
        f_tx.write(json.dumps(row) + "\n")
        fetched += 1
    f_tok.write(json.dumps({
        "mint": mint, "curve": curve, "s0": s0, "create_sig": rec["sig"], "creator": c.get("creator"),
        "user": c.get("user"), "name": c.get("name"), "symbol": c.get("symbol"), "uri": c.get("uri"),
        "n_sigs": len(sigs), "n_ok": len(ok), "n_fetched": fetched, "truncated": len(ok) > MAX_TX,
        "s0_order": order, "worker": TAG,
        "sigs": [[s["slot"], s["signature"][:16], None if s.get("err") is None else json.dumps(s["err"])[:60],
                  s.get("transactionIndex")] for s in sigs],
    }) + "\n")


def main():
    queue = []
    it = creates_iter()
    done = 0
    while time.time() < T_END:
        # riempi la coda con i create nuovi (non bloccante: legge finché ci sono righe pronte)
        try:
            while True:
                rec, c = next(it)
                if sampled(c["mint"]):
                    queue.append((rec["t"] / 1e9, rec, c))
                if time.time() - rec["t"] / 1e9 < WINDOW_S:
                    break
        except StopIteration:
            break
        ready = [q for q in queue if time.time() - q[0] >= WINDOW_S]
        if not ready:
            time.sleep(5)
            continue
        q = ready[0]
        queue.remove(q)
        process(q[1], q[2])
        done += 1
        if done % 10 == 0:
            f_log.write(json.dumps({"t": time.time(), "done": done, "queue": len(queue), **stats}) + "\n")


main()
