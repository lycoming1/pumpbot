#!/usr/bin/env python3
"""Campione di tx fallite nei primi slot dopo il create: chi le manda, con che programmi, che errore, quanto paga."""
import collections, json, os, random, sys, time, urllib.request
REC = sys.argv[1]; OUT = sys.argv[2]; NTOK = int(sys.argv[3]); PER = int(sys.argv[4])
RPC = "https://solana-rpc.publicnode.com"
def rpc(m, p):
    for i in range(5):
        time.sleep(0.8)
        try:
            b = json.dumps({"jsonrpc": "2.0", "id": 1, "method": m, "params": p}).encode()
            r = json.loads(urllib.request.urlopen(urllib.request.Request(RPC, data=b, headers={"content-type": "application/json", "user-agent": "pumpbot-research/0.1"}), timeout=30).read())
            if "result" in r: return r["result"]
        except Exception as e:
            time.sleep(2 * (i + 1))
    return None
toks = [json.loads(l) for l in open(os.path.join(REC, "bf_tokens.jsonl")) if l.endswith("\n")]
toks = [t for t in toks if sum(1 for s in t["sigs"] if s[2] and 1 <= s[0] - t["s0"] <= 5) >= 10]
random.Random(1).shuffle(toks)
f = open(os.path.join(OUT, "failed_tx.jsonl"), "a")
for t in toks[:NTOK]:
    sigs = rpc("getSignaturesForAddress", [t["curve"], {"limit": 1000}])
    if not sigs: continue
    cand = [s for s in sigs if s.get("err") and 1 <= s["slot"] - t["s0"] <= 5]
    random.Random(2).shuffle(cand)
    for s in cand[:PER]:
        tx = rpc("getTransaction", [s["signature"], {"encoding": "jsonParsed", "maxSupportedTransactionVersion": 1}])
        if not tx: continue
        msg = tx["transaction"]["message"]; meta = tx["meta"]
        top = [ix.get("programId") for ix in msg["instructions"]]
        logs = meta.get("logMessages") or []
        errlog = [l for l in logs if "failed" in l or "Error" in l or "error" in l][:3]
        transfers = []
        for ix in msg["instructions"]:
            p = ix.get("parsed")
            if ix.get("program") == "system" and isinstance(p, dict) and p.get("type") == "transfer":
                transfers.append([p["info"]["destination"], p["info"]["lamports"]])
        f.write(json.dumps({"mint": t["mint"], "off": s["slot"] - t["s0"], "payer": msg["accountKeys"][0]["pubkey"],
            "fee": meta.get("fee"), "cu": meta.get("computeUnitsConsumed"), "err": meta.get("err"), "top": top,
            "errlog": errlog, "transfers": transfers, "nlogs": len(logs)}) + "\n"); f.flush()
