import struct, base64, hashlib
B58="123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
def b58e(b):
    n=int.from_bytes(b,'big'); s=""
    while n: n,r=divmod(n,58); s=B58[r]+s
    pad=len(b)-len(b.lstrip(b'\0'))
    return "1"*pad+s
def b58d(s):
    n=0
    for c in s: n=n*58+B58.index(c)
    b=n.to_bytes((n.bit_length()+7)//8,'big') if n else b''
    pad=len(s)-len(s.lstrip('1'))
    return b'\0'*pad+b
PUMP="6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
AMM="pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"
EVENT_IX_TAG=bytes.fromhex("e445a52e51cb9a1d")
ZERO="11111111111111111111111111111111"
def disc(s): return hashlib.sha256(s.encode()).digest()[:8]
class R:
    def __init__(s,b,o=0): s.b=b; s.o=o
    def rem(s): return len(s.b)-s.o
    def u8(s):
        if s.o+1>len(s.b): s.o+=1; return None
        v=s.b[s.o]; s.o+=1; return v
    def boo(s):
        v=s.u8(); return None if v is None else bool(v)
    def u16(s): return s._f("<H",2)
    def u32(s): return s._f("<I",4)
    def u64(s): return s._f("<Q",8)
    def i64(s): return s._f("<q",8)
    def i128(s):
        if s.o+16>len(s.b): s.o+=16; return None
        v=int.from_bytes(s.b[s.o:s.o+16],'little',signed=True); s.o+=16; return v
    def u128(s):
        if s.o+16>len(s.b): s.o+=16; return None
        v=int.from_bytes(s.b[s.o:s.o+16],'little'); s.o+=16; return v
    def pk(s):
        if s.o+32>len(s.b): s.o+=32; return None
        v=b58e(s.b[s.o:s.o+32]); s.o+=32; return v
    def st(s):
        L=s.u32();
        if L is None: return None
        v=s.b[s.o:s.o+L].decode('utf-8','replace'); s.o+=L; return v
    def _f(s,f,n):
        if s.o+n>len(s.b): s.o+=n; return None
        v=struct.unpack_from(f,s.b,s.o)[0]; s.o+=n; return v

def dec_trade(b):
    r=R(b); e={}
    for k,f in [("mint","pk"),("sol_amount","u64"),("token_amount","u64"),("is_buy","boo"),("user","pk"),("timestamp","i64"),
        ("virtual_sol_reserves","u64"),("virtual_token_reserves","u64"),("real_sol_reserves","u64"),("real_token_reserves","u64"),
        ("fee_recipient","pk"),("fee_basis_points","u64"),("fee","u64"),("creator","pk"),("creator_fee_basis_points","u64"),
        ("creator_fee","u64"),("track_volume","boo"),("total_unclaimed_tokens","u64"),("total_claimed_tokens","u64"),
        ("current_sol_volume","u64"),("last_update_timestamp","i64")]:
        e[k]=getattr(r,f)()
    assert r.o==250
    e["ix_name"]=r.st(); L=len(e["ix_name"].encode()) if e["ix_name"] is not None else 0
    o=r.o; assert o==254+L
    e["mayhem_mode"]=r.boo(); e["cashback_fee_basis_points"]=r.u64(); e["cashback"]=r.u64()
    e["buyback_fee_basis_points"]=r.u64(); e["buyback_fee"]=r.u64()
    n=r.u32(); e["shareholders"]=[]
    for i in range(n or 0):
        e["shareholders"].append((r.pk(), r.u16()))
    p=r.o; assert p==o+37+34*(n or 0)
    e["quote_mint"]=r.pk(); e["quote_amount"]=r.u64(); e["virtual_quote_reserves"]=r.u64(); e["real_quote_reserves"]=r.u64()
    e["holder_rewards_bps"]=r.u64(); e["holder_rewards"]=r.u64(); e["creator_fee_unclaimed"]=r.u64(); e["_extra_u64"]=r.u64()
    e["_len"]=len(b); e["_expected_len"]=379+L+34*(n or 0); e["_tail"]=b[r.o:].hex()
    return e

def dec_create(b):
    r=R(b); e={}
    e["name"]=r.st(); e["symbol"]=r.st(); e["uri"]=r.st(); s=r.o; e["_s"]=s
    for k,f in [("mint","pk"),("bonding_curve","pk"),("user","pk"),("creator","pk"),("timestamp","i64"),("virtual_token_reserves","u64"),
        ("virtual_sol_reserves","u64"),("real_token_reserves","u64"),("token_total_supply","u64"),("token_program","pk"),
        ("is_mayhem_mode","boo"),("is_cashback_enabled","boo"),("quote_mint","pk"),("virtual_quote_reserves","u64"),
        ("creator_fee_bps","u64"),("is_holder_reward","boo"),("depth","u8")]:
        e[k]=getattr(r,f)()
    e["_len"]=len(b); e["_end"]=r.o; e["_tail"]=b[r.o:].hex()
    return e

def fixed(b, fields):
    r=R(b); e={}
    for k,f in fields: e[k]=getattr(r,f)()
    e["_len"]=len(b); e["_end"]=r.o; e["_tail"]=b[r.o:].hex()
    return e
COMPLETE=[("user","pk"),("mint","pk"),("bonding_curve","pk"),("timestamp","i64"),("quote_mint","pk")]
MIGR=[("user","pk"),("mint","pk"),("mint_amount","u64"),("sol_amount","u64"),("pool_migration_fee","u64"),("bonding_curve","pk"),("timestamp","i64"),("pool","pk"),("quote_mint","pk")]
POSTCOMPLETE=[("user","pk"),("mint","pk"),("bonding_curve","pk"),("quote_mint","pk"),("timestamp","i64"),("base_out","u64"),("quote_in","u64"),("fee_basis_points","u64"),("fee","u64"),("creator_fee_basis_points","u64"),("creator_fee","u64"),("buyback_fee","u64"),("pool_base_reserves_before","u64"),("pool_quote_reserves_before","u64"),("pool_base_reserves_after","u64"),("pool_quote_reserves_after","u64")]
SWEEP=[("timestamp","i64"),("mint","pk"),("bonding_curve","pk"),("quote_mint","pk"),("recipient","pk"),("amount","u64"),("bucket","u8")]
AMM_COMMON_HEAD_BUY=[("timestamp","i64"),("base_amount_out","u64"),("max_quote_amount_in","u64"),("user_base_token_reserves","u64"),("user_quote_token_reserves","u64"),("pool_base_token_reserves","u64"),("pool_quote_token_reserves","u64"),("quote_amount_in","u64"),("lp_fee_basis_points","u64"),("lp_fee","u64"),("protocol_fee_basis_points","u64"),("protocol_fee","u64"),("quote_amount_in_with_lp_fee","u64"),("user_quote_amount_in","u64"),("pool","pk"),("user","pk"),("user_base_token_account","pk"),("user_quote_token_account","pk"),("protocol_fee_recipient","pk"),("protocol_fee_recipient_token_account","pk"),("coin_creator","pk"),("coin_creator_fee_basis_points","u64"),("coin_creator_fee","u64"),("track_volume","boo"),("total_unclaimed_tokens","u64"),("total_claimed_tokens","u64"),("current_sol_volume","u64"),("last_update_timestamp","i64"),("min_base_amount_out","u64")]
def dec_ammbuy(b):
    r=R(b); e={}
    for k,f in AMM_COMMON_HEAD_BUY: e[k]=getattr(r,f)()
    assert r.o==393
    e["ix_name"]=r.st(); L=len(e["ix_name"].encode()) if e["ix_name"] else 0
    assert r.o==397+L
    for k,f in [("cashback_fee_basis_points","u64"),("cashback","u64"),("buyback_fee_basis_points","u64"),("buyback_fee","u64"),("virtual_quote_reserves","i128"),("can_boost","boo"),("base_supply","u64"),("holder_rewards_bps","u64"),("holder_rewards","u64"),("creator_fee_unclaimed","u64"),("_extra_u64","u64")]:
        e[k]=getattr(r,f)()
    e["_len"]=len(b); e["_end"]=r.o; e["_tail"]=b[r.o:].hex(); e["_L"]=L
    return e
SELL=[("timestamp","i64"),("base_amount_in","u64"),("min_quote_amount_out","u64"),("user_base_token_reserves","u64"),("user_quote_token_reserves","u64"),("pool_base_token_reserves","u64"),("pool_quote_token_reserves","u64"),("quote_amount_out","u64"),("lp_fee_basis_points","u64"),("lp_fee","u64"),("protocol_fee_basis_points","u64"),("protocol_fee","u64"),("quote_amount_out_without_lp_fee","u64"),("user_quote_amount_out","u64"),("pool","pk"),("user","pk"),("user_base_token_account","pk"),("user_quote_token_account","pk"),("protocol_fee_recipient","pk"),("protocol_fee_recipient_token_account","pk"),("coin_creator","pk"),("coin_creator_fee_basis_points","u64"),("coin_creator_fee","u64"),("cashback_fee_basis_points","u64"),("cashback","u64"),("buyback_fee_basis_points","u64"),("buyback_fee","u64"),("virtual_quote_reserves","i128"),("can_boost","boo"),("base_supply","u64"),("holder_rewards_bps","u64"),("holder_rewards","u64"),("creator_fee_unclaimed","u64"),("_extra_u64","u64")]
CREATEPOOL=[("timestamp","i64"),("index","u16"),("creator","pk"),("base_mint","pk"),("quote_mint","pk"),("base_mint_decimals","u8"),("quote_mint_decimals","u8"),("base_amount_in","u64"),("quote_amount_in","u64"),("pool_base_amount","u64"),("pool_quote_amount","u64"),("minimum_liquidity","u64"),("initial_liquidity","u64"),("lp_token_amount_out","u64"),("pool_bump","u8"),("pool","pk"),("lp_mint","pk"),("user_base_token_account","pk"),("user_quote_token_account","pk"),("coin_creator","pk"),("is_mayhem_mode","boo"),("creator_fee_bps","u64"),("can_edit_creator_fee","boo"),("is_holder_reward","boo")]
INITBOOST=[("timestamp","i64"),("mint","pk"),("bonding_curve","pk"),("pool","pk"),("virtual_quote_reserves","i128"),("real_quote_reserves_after","u64")]
EVENTS={
 disc("event:TradeEvent"):("TradeEvent",dec_trade),
 disc("event:CreateEvent"):("CreateEvent",dec_create),
 disc("event:CompleteEvent"):("CompleteEvent",lambda b:fixed(b,COMPLETE)),
 disc("event:CompletePumpAmmMigrationEvent"):("CompletePumpAmmMigrationEvent",lambda b:fixed(b,MIGR)),
 disc("event:PostCompleteBuyEvent"):("PostCompleteBuyEvent",lambda b:fixed(b,POSTCOMPLETE)),
 disc("event:SweepBondingCurveFeeEvent"):("SweepBondingCurveFeeEvent",lambda b:fixed(b,SWEEP)),
 disc("event:BuyEvent"):("BuyEvent",dec_ammbuy),
 disc("event:SellEvent"):("SellEvent",lambda b:fixed(b,SELL)),
 disc("event:CreatePoolEvent"):("CreatePoolEvent",lambda b:fixed(b,CREATEPOOL)),
 disc("event:InitBoostEvent"):("InitBoostEvent",lambda b:fixed(b,INITBOOST)),
 disc("event:InitUserVolumeAccumulatorEvent"):("InitUserVolumeAccumulatorEvent",lambda b:{"_len":len(b),"_raw":b.hex()}),
 disc("event:CloseUserVolumeAccumulatorEvent"):("CloseUserVolumeAccumulatorEvent",lambda b:{"_len":len(b),"_raw":b.hex()}),
 disc("event:BoostBuyAndBurnEvent"):("BoostBuyAndBurnEvent",lambda b:{"_len":len(b),"_raw":b.hex()}),
 disc("event:SweepPoolFeeEvent"):("SweepPoolFeeEvent",lambda b:{"_len":len(b),"_raw":b.hex()}),
}
def decode_event(raw):
    d=raw[:8]; body=raw[8:]
    if d in EVENTS:
        name,f=EVENTS[d]
        try: return name,f(body)
        except Exception as ex: return name,{"_error":repr(ex),"_len":len(body)}
    return ("UNKNOWN:"+d.hex(),{"_len":len(body)})

def log_events(tx):
    """canale A: Program data con stack"""
    logs=tx["meta"].get("logMessages") or []
    st=[]; out=[]; trunc=False
    for l in logs:
        if l.startswith("Program ") and " invoke [" in l:
            st.append(l.split()[1])
        elif l.startswith("Program ") and (l.endswith(" success") or " failed" in l) and len(l.split())>=3 and l.split()[2] in ("success","failed:"):
            if st: st.pop()
        elif l.startswith("Program data: "):
            top=st[-1] if st else None
            out.append((top, base64.b64decode(l[len("Program data: "):])))
        elif "Log truncated" in l: trunc=True
    return out, trunc

def account_keys(tx):
    m=tx["transaction"]["message"]
    keys=list(m["accountKeys"])
    la=tx["meta"].get("loadedAddresses") or {}
    keys+=la.get("writable",[])+la.get("readonly",[])
    return keys

def cpi_events(tx):
    keys=account_keys(tx); out=[]
    for grp in tx["meta"].get("innerInstructions") or []:
        for ix in grp["instructions"]:
            pid=keys[ix["programIdIndex"]]
            data=b58d(ix["data"])
            if pid in (PUMP,AMM) and data[:8]==EVENT_IX_TAG:
                out.append((pid, data[8:], [keys[a] for a in ix["accounts"]], ix.get("stackHeight")))
    return out
