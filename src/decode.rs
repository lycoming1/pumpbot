//! Decoder manuale (Borsh) di log, eventi e account. Fonte di verità: docs/research/onchain-spec.md.

use base64::Engine;

use crate::types::*;

pub const DISC_TRADE: [u8; 8] = [0xbd, 0xdb, 0x7f, 0xd3, 0x4e, 0xe6, 0x61, 0xee];
pub const DISC_CREATE: [u8; 8] = [0x1b, 0x72, 0xa9, 0x4d, 0xde, 0xeb, 0x63, 0x76];
pub const DISC_COMPLETE: [u8; 8] = [0x5f, 0x72, 0x61, 0x9c, 0xd4, 0x2e, 0x98, 0x08];
pub const DISC_POST_COMPLETE_BUY: [u8; 8] = [0x6f, 0xb0, 0x6d, 0x8b, 0x31, 0x6c, 0xd5, 0xfb];
pub const DISC_MIGRATION: [u8; 8] = [0xbd, 0xe9, 0x5d, 0xb9, 0x5c, 0x94, 0xea, 0x94];
pub const DISC_BONDING_CURVE: [u8; 8] = [0x17, 0xb7, 0xf8, 0x37, 0x60, 0xd8, 0xac, 0x60];

/// Programma in cima allo stack degli invoke quando compare una riga `Program data:`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    Pump,
    Amm,
    Other,
}

/// Estrae i payload `Program data:` attribuendoli con lo stack degli invoke.
/// Ritorna (frame, bytes) e se il log è troncato.
pub fn program_data<S: AsRef<str>>(logs: &[S]) -> (Vec<(Frame, Vec<u8>)>, bool) {
    let mut stack: Vec<Frame> = Vec::with_capacity(8);
    let mut out = Vec::new();
    let mut truncated = false;
    for line in logs {
        let l = line.as_ref();
        if let Some(rest) = l.strip_prefix("Program data: ") {
            let top = stack.last().copied().unwrap_or(Frame::Other);
            if let Ok(b) = base64::engine::general_purpose::STANDARD.decode(rest.trim()) {
                out.push((top, b));
            }
        } else if let Some(rest) = l.strip_prefix("Program ") {
            // "Program X invoke [n]" | "Program X success" | "Program X failed: ..." | "Program X consumed ..."
            let mut it = rest.splitn(3, ' ');
            let prog = it.next().unwrap_or("");
            let verb = it.next().unwrap_or("");
            match verb {
                "invoke" => stack.push(match prog {
                    PUMP => Frame::Pump,
                    AMM => Frame::Amm,
                    _ => Frame::Other,
                }),
                "success" | "failed:" | "failed" => {
                    stack.pop();
                }
                _ => {}
            }
        } else if l.contains("Log truncated") {
            truncated = true;
        }
    }
    (out, truncated)
}

/// Lettore Borsh tollerante: i campi mancanti in coda valgono 0.
pub struct R<'a> {
    b: &'a [u8],
    pub o: usize,
}

impl<'a> R<'a> {
    pub fn new(b: &'a [u8], o: usize) -> Self {
        R { b, o }
    }
    pub fn has(&self, n: usize) -> bool {
        self.o + n <= self.b.len()
    }
    pub fn u8(&mut self) -> u8 {
        let v = if self.has(1) { self.b[self.o] } else { 0 };
        self.o += 1;
        v
    }
    pub fn bool(&mut self) -> bool {
        self.u8() != 0
    }
    pub fn u16(&mut self) -> u16 {
        let v = if self.has(2) { u16::from_le_bytes(self.b[self.o..self.o + 2].try_into().unwrap()) } else { 0 };
        self.o += 2;
        v
    }
    pub fn u32(&mut self) -> u32 {
        let v = if self.has(4) { u32::from_le_bytes(self.b[self.o..self.o + 4].try_into().unwrap()) } else { 0 };
        self.o += 4;
        v
    }
    pub fn u64(&mut self) -> u64 {
        let v = if self.has(8) { u64::from_le_bytes(self.b[self.o..self.o + 8].try_into().unwrap()) } else { 0 };
        self.o += 8;
        v
    }
    pub fn i64(&mut self) -> i64 {
        self.u64() as i64
    }
    pub fn pk(&mut self) -> Pubkey {
        let v = if self.has(32) { self.b[self.o..self.o + 32].try_into().unwrap() } else { ZERO_KEY };
        self.o += 32;
        v
    }
    pub fn string(&mut self) -> String {
        let n = self.u32() as usize;
        let v = if self.has(n) { String::from_utf8_lossy(&self.b[self.o..self.o + n]).into_owned() } else { String::new() };
        self.o += n;
        v
    }
}

/// Codice compatto per `ix_name` (solo per statistiche).
pub fn ix_code(name: &str) -> u8 {
    match name {
        "buy" => 1,
        "sell" => 2,
        "buy_exact_sol_in" => 3,
        "buy_exact_quote_in" => 4,
        "multi_hop_swap" => 5,
        _ => 0,
    }
}

fn is_sol_quote(q: &Pubkey, wsol: &Pubkey) -> bool {
    q == &ZERO_KEY || q == wsol
}

/// TradeEvent: prefisso fisso fino all'offset 258 (discriminatore incluso), poi ix_name e coda sequenziale.
pub fn decode_trade(b: &[u8], wsol: &Pubkey) -> Option<TradeRec> {
    if b.len() < 262 || b[..8] != DISC_TRADE {
        return None;
    }
    let mut r = R::new(b, 8);
    let mint = r.pk();
    let sol_amount = r.u64();
    let token_amount = r.u64();
    let is_buy = r.bool();
    let user = r.pk();
    let _ts = r.i64();
    let vs = r.u64();
    let vt = r.u64();
    let rs = r.u64();
    let rtr = r.u64();
    let _fee_recipient = r.pk();
    let fee_bps = r.u64();
    let fee = r.u64();
    let creator = r.pk();
    let cfee_bps = r.u64();
    let cfee = r.u64();
    debug_assert_eq!(r.o, 225);
    r.o = 258;
    let ix = r.string();
    let mayhem = r.bool();
    let _cashback_bps = r.u64();
    let _cashback = r.u64();
    let _buyback_bps = r.u64();
    let _buyback_fee = r.u64();
    let n_sh = r.u32() as usize;
    r.o += n_sh.min(64) * 34;
    let quote_mint = r.pk();
    let quote_amount = r.u64();
    let vq = r.u64();
    let rq = r.u64();
    let quote_sol = is_sol_quote(&quote_mint, wsol);
    // Sulle coin SOL i campi quote coincidono con quelli sol; se la coda manca si usano i campi sol.
    let (q, vqq, rqq) = if quote_amount != 0 || vq != 0 { (quote_amount, vq, rq) } else { (sol_amount, vs, rs) };
    Some(TradeRec {
        slot: 0,
        sig: [0u8; 64],
        ev_idx: 0,
        mint,
        user,
        creator,
        is_buy,
        quote: q,
        tokens: token_amount,
        post: CurveState { vq: vqq, vt, rq: rqq, rt: rtr, complete: rtr == 0 },
        fee,
        cfee,
        fee_bps,
        cfee_bps,
        mayhem,
        quote_sol,
        ix: ix_code(&ix),
    })
}

pub fn decode_create(b: &[u8], wsol: &Pubkey) -> Option<CreateRec> {
    if b.len() < 8 || b[..8] != DISC_CREATE {
        return None;
    }
    let mut r = R::new(b, 8);
    let name = r.string();
    let symbol = r.string();
    let uri = r.string();
    let mint = r.pk();
    let curve = r.pk();
    let user = r.pk();
    let creator = r.pk();
    let _ts = r.i64();
    let _vt = r.u64();
    let _vs = r.u64();
    let _rt = r.u64();
    let _supply = r.u64();
    let _token_program = r.pk();
    let mayhem = r.bool();
    let cashback = r.bool();
    let quote_mint = r.pk();
    let _vq = r.u64();
    let _creator_fee_bps = r.u64();
    let holder_reward = r.bool();
    if !r.has(0) {
        return None;
    }
    Some(CreateRec {
        name,
        symbol,
        uri,
        mint,
        curve,
        user,
        creator,
        mayhem,
        cashback,
        quote_sol: is_sol_quote(&quote_mint, wsol),
        holder_reward,
    })
}

pub fn decode_complete(b: &[u8]) -> Option<CompleteRec> {
    if b.len() < 8 + 96 {
        return None;
    }
    let disc: [u8; 8] = b[..8].try_into().ok()?;
    let mut r = R::new(b, 8);
    match disc {
        DISC_COMPLETE => {
            let _user = r.pk();
            let mint = r.pk();
            let curve = r.pk();
            Some(CompleteRec { mint, curve, kind: CompleteKind::Complete })
        }
        DISC_MIGRATION => {
            let _user = r.pk();
            let mint = r.pk();
            let mint_amount = r.u64();
            let sol_amount = r.u64();
            let migration_fee = r.u64();
            let curve = r.pk();
            let _ts = r.i64();
            let pool = r.pk();
            Some(CompleteRec { mint, curve, kind: CompleteKind::Migration { pool, mint_amount, sol_amount, migration_fee } })
        }
        DISC_POST_COMPLETE_BUY => {
            let _user = r.pk();
            let mint = r.pk();
            let curve = r.pk();
            let _quote_mint = r.pk();
            let _ts = r.i64();
            let base_out = r.u64();
            let quote_in = r.u64();
            let _fee_bps = r.u64();
            let _fee = r.u64();
            let _cfee_bps = r.u64();
            let _cfee = r.u64();
            let _buyback = r.u64();
            let _pb_before = r.u64();
            let _pq_before = r.u64();
            let pool_base_after = r.u64();
            let pool_quote_after = r.u64();
            Some(CompleteRec { mint, curve, kind: CompleteKind::PostCompleteBuy { base_out, quote_in, pool_base_after, pool_quote_after } })
        }
        _ => None,
    }
}

/// Decodifica tutti gli eventi pump dei log di una tx riuscita.
pub fn decode_tx_logs<S: AsRef<str>>(logs: &[S], slot: u64, sig: Sig, arrival_ns: u64, wsol: &Pubkey) -> TxEvents {
    let (datas, truncated) = program_data(logs);
    let mut tx = TxEvents { slot, sig, arrival_ns, truncated, ..Default::default() };
    let mut idx: u16 = 0;
    for (frame, b) in datas {
        if frame != Frame::Pump || b.len() < 8 {
            continue;
        }
        let disc: [u8; 8] = b[..8].try_into().unwrap();
        match disc {
            DISC_TRADE => {
                if let Some(mut t) = decode_trade(&b, wsol) {
                    t.slot = slot;
                    t.sig = sig;
                    t.ev_idx = idx;
                    tx.trades.push(t);
                }
            }
            DISC_CREATE => {
                if let Some(c) = decode_create(&b, wsol) {
                    tx.creates.push(c);
                }
            }
            DISC_COMPLETE | DISC_MIGRATION | DISC_POST_COMPLETE_BUY => {
                if let Some(c) = decode_complete(&b) {
                    tx.completes.push(c);
                }
            }
            _ => {}
        }
        idx = idx.wrapping_add(1);
    }
    tx
}

/// Account BondingCurve (166 byte; accetta account più corti con i campi in coda a 0).
pub fn decode_bonding_curve(d: &[u8], wsol: &Pubkey) -> Option<(CurveState, CurveStatic)> {
    if d.len() < 49 || d[..8] != DISC_BONDING_CURVE {
        return None;
    }
    let mut r = R::new(d, 8);
    let vt = r.u64();
    let vq = r.u64();
    let rt = r.u64();
    let rq = r.u64();
    let _supply = r.u64();
    let complete = r.bool();
    let creator = r.pk();
    let mayhem = r.bool();
    let cashback = r.bool();
    let quote_mint = r.pk();
    let _cfee_bps = r.u64();
    let _can_edit = r.bool();
    let holder_reward = r.bool();
    let state = CurveState { vq, vt, rq, rt, complete };
    let stat = CurveStatic { creator, mayhem, cashback, quote_sol: is_sol_quote(&quote_mint, wsol), holder_reward };
    Some((state, stat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminators_match_sha256_prefixes_in_spec() {
        assert_eq!(hex(&DISC_TRADE), "bddb7fd34ee661ee");
        assert_eq!(hex(&DISC_CREATE), "1b72a94ddeeb6376");
        assert_eq!(hex(&DISC_COMPLETE), "5f72619cd42e9808");
        assert_eq!(hex(&DISC_POST_COMPLETE_BUY), "6fb06d8b316cd5fb");
        assert_eq!(hex(&DISC_MIGRATION), "bde95db95c94ea94");
        assert_eq!(hex(&DISC_BONDING_CURVE), "17b7f83760d8ac60");
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }
}
