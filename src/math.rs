//! Matematica intera della curva e del pool PumpSwap (spec §6–7). Tutto in u128.

use crate::types::{CurveState, POOL_BASE_AT_MIGRATION, POOL_MIGRATION_FEE};

fn ceil_div(a: u128, b: u128) -> u128 {
    a.div_ceil(b)
}

/// Buy exact-in con budget `budget` (lamport, fee comprese). Ritorna (netto in curva, token ricevuti).
pub fn buy_exact_in(s: &CurveState, budget: u64, pbps: u64, cbps: u64) -> (u64, u64) {
    let b = budget as u128;
    let (p, c) = (pbps as u128, cbps as u128);
    let net0 = b * 10_000 / (10_000 + p + c);
    let fp = ceil_div(net0 * p, 10_000);
    let fc = ceil_div(net0 * c, 10_000);
    let total = net0 + fp + fc;
    let net = if total > b { net0 - (total - b) } else { net0 };
    if net <= 1 {
        return (0, 0);
    }
    let vq = s.vq as u128;
    let vt = s.vt as u128;
    let tokens = (net - 1) * vt / (vq + net - 1);
    let tokens = tokens.min(s.rt as u128);
    (net as u64, tokens as u64)
}

/// Sell di `q` token sulla curva: SOL netti ricevuti.
pub fn sell(s: &CurveState, q: u64, pbps: u64, cbps: u64) -> u64 {
    if q == 0 {
        return 0;
    }
    let out = (q as u128) * (s.vq as u128) / (s.vt as u128 + q as u128);
    let fp = ceil_div(out * pbps as u128, 10_000);
    let fc = ceil_div(out * cbps as u128, 10_000);
    out.saturating_sub(fp + fc) as u64
}

/// Sell sul pool PumpSwap appena migrato da una curva completa (primo scaglione: lp 2 + protocol 93 + creator 30).
pub fn pool_sell_after_migration(final_state: &CurveState, q: u64) -> u64 {
    let b = POOL_BASE_AT_MIGRATION as u128;
    let qeff = final_state.rq.saturating_sub(POOL_MIGRATION_FEE) as u128;
    if q == 0 || qeff == 0 {
        return 0;
    }
    let out = qeff * q as u128 / (b + q as u128);
    let fees = ceil_div(out * 2, 10_000) + ceil_div(out * 93, 10_000) + ceil_div(out * 30, 10_000);
    out.saturating_sub(fees) as u64
}

/// Valore di liquidazione netto di `q` token sullo stato `s` (curva o, se completa, pool).
pub fn liquidation_value(s: &CurveState, q: u64, pbps: u64, cbps: u64) -> u64 {
    if s.complete {
        pool_sell_after_migration(s, q)
    } else {
        sell(s, q, pbps, cbps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn round_trip_loses_about_2_47_percent() {
        // spec: round trip immediato -2,4691% a qualsiasi taglia e progresso
        for rq_sol in [1.151_f64, 5.0, 8.548] {
            let rq = lamports(rq_sol);
            let vq = VS0 + rq;
            let vt = ((VS0 as u128 * VT0 as u128) / vq as u128) as u64;
            let s = CurveState { vq, vt, rq, rt: vt - VT_MINUS_RT, complete: false };
            for b in [100_000_000u64, 500_000_000, 1_000_000_000] {
                let (net, tok) = buy_exact_in(&s, b, 95, 30);
                let after = CurveState { vq: s.vq + net, vt: s.vt - tok, rq: s.rq + net, rt: s.rt - tok, complete: false };
                let back = sell(&after, tok, 95, 30);
                let loss = 1.0 - back as f64 / b as f64;
                assert!((loss - 0.024691).abs() < 0.0002, "loss {loss} at rq {rq_sol} b {b}");
            }
        }
    }
}
