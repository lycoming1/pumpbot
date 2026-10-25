//! Il decoder deve coincidere con il riferimento Python (docs/research/pumpspec_reference.py)
//! sulle transazioni mainnet reali in tests/fixtures (risultati attesi in tests/expected_events.json).

use pumpbot::types::*;
use pumpbot::{b58, decode};
use serde_json::Value;

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

#[test]
fn decoder_matches_python_reference() {
    let exp: Value = serde_json::from_str(&std::fs::read_to_string("tests/expected_events.json").unwrap()).unwrap();
    let wsol = b58::decode32(WSOL).unwrap();
    let zero = "11111111111111111111111111111111";
    let (mut n_tx, mut n_tr, mut n_cr, mut n_co) = (0, 0, 0, 0);
    for (path, e) in exp.as_object().unwrap() {
        let raw = std::fs::read_to_string(format!("tests/fixtures/{path}")).unwrap();
        let tx: Value = serde_json::from_str(&raw).unwrap();
        let logs: Vec<String> = tx["meta"]["logMessages"].as_array().unwrap().iter().map(|l| s(l)).collect();
        let slot = e["slot"].as_u64().unwrap();
        let ev = decode::decode_tx_logs(&logs, slot, [0u8; 64], 0, &wsol);
        assert_eq!(ev.truncated, e["truncated"].as_bool().unwrap(), "{path}: truncated");
        let (mut ti, mut ci, mut oi) = (0usize, 0usize, 0usize);
        for x in e["events"].as_array().unwrap() {
            let f = &x[1];
            match x[0].as_str().unwrap() {
                "TradeEvent" => {
                    let t = &ev.trades[ti];
                    ti += 1;
                    assert_eq!(b58::encode32(&t.mint), s(&f["mint"]), "{path}");
                    assert_eq!(b58::encode32(&t.user), s(&f["user"]), "{path}");
                    assert_eq!(b58::encode32(&t.creator), s(&f["creator"]), "{path}");
                    assert_eq!(t.tokens, f["token_amount"].as_u64().unwrap(), "{path}");
                    assert_eq!(t.is_buy, f["is_buy"].as_bool().unwrap(), "{path}");
                    assert_eq!(t.post.vt, f["virtual_token_reserves"].as_u64().unwrap(), "{path}");
                    assert_eq!(t.post.rt, f["real_token_reserves"].as_u64().unwrap(), "{path}");
                    assert_eq!(t.fee_bps, f["fee_basis_points"].as_u64().unwrap(), "{path}");
                    assert_eq!(t.fee, f["fee"].as_u64().unwrap(), "{path}");
                    assert_eq!(t.cfee_bps, f["creator_fee_basis_points"].as_u64().unwrap(), "{path}");
                    assert_eq!(t.cfee, f["creator_fee"].as_u64().unwrap(), "{path}");
                    assert_eq!(t.mayhem, f["mayhem_mode"].as_bool().unwrap_or(false), "{path}");
                    let qm = s(&f["quote_mint"]);
                    let sol_quote = qm == zero || qm == WSOL || qm.is_empty();
                    assert_eq!(t.quote_sol, sol_quote, "{path}: quote_mint {qm}");
                    if sol_quote {
                        assert_eq!(t.quote, f["sol_amount"].as_u64().unwrap(), "{path}");
                        assert_eq!(t.post.vq, f["virtual_sol_reserves"].as_u64().unwrap(), "{path}");
                        assert_eq!(t.post.rq, f["real_sol_reserves"].as_u64().unwrap(), "{path}");
                        if !t.mayhem {
                            assert!(t.post.is_standard(), "{path}: invarianti della curva standard");
                        }
                    }
                    n_tr += 1;
                }
                "CreateEvent" => {
                    let c = &ev.creates[ci];
                    ci += 1;
                    assert_eq!(c.name, s(&f["name"]), "{path}");
                    assert_eq!(c.symbol, s(&f["symbol"]), "{path}");
                    assert_eq!(c.uri, s(&f["uri"]), "{path}");
                    assert_eq!(b58::encode32(&c.mint), s(&f["mint"]), "{path}");
                    assert_eq!(b58::encode32(&c.curve), s(&f["bonding_curve"]), "{path}");
                    assert_eq!(b58::encode32(&c.user), s(&f["user"]), "{path}");
                    assert_eq!(b58::encode32(&c.creator), s(&f["creator"]), "{path}");
                    assert_eq!(c.mayhem, f["is_mayhem_mode"].as_bool().unwrap(), "{path}");
                    assert_eq!(c.cashback, f["is_cashback_enabled"].as_bool().unwrap(), "{path}");
                    assert_eq!(c.holder_reward, f["is_holder_reward"].as_bool().unwrap(), "{path}");
                    let qm = s(&f["quote_mint"]);
                    assert_eq!(c.quote_sol, qm == zero || qm == WSOL, "{path}");
                    n_cr += 1;
                }
                "CompleteEvent" | "CompletePumpAmmMigrationEvent" => {
                    let c = &ev.completes[oi];
                    oi += 1;
                    assert_eq!(b58::encode32(&c.mint), s(&f["mint"]), "{path}");
                    assert_eq!(b58::encode32(&c.curve), s(&f["bonding_curve"]), "{path}");
                    if let CompleteKind::Migration { pool, mint_amount, sol_amount, migration_fee } = &c.kind {
                        assert_eq!(b58::encode32(pool), s(&f["pool"]), "{path}");
                        assert_eq!(*mint_amount, f["mint_amount"].as_u64().unwrap(), "{path}");
                        assert_eq!(*sol_amount, f["sol_amount"].as_u64().unwrap(), "{path}");
                        assert_eq!(*migration_fee, f["pool_migration_fee"].as_u64().unwrap(), "{path}");
                    }
                    n_co += 1;
                }
                other => panic!("evento inatteso {other}"),
            }
        }
        assert_eq!(ti, ev.trades.len(), "{path}: trade in più");
        assert_eq!(ci, ev.creates.len(), "{path}: create in più");
        assert_eq!(oi, ev.completes.len(), "{path}: complete in più");
        n_tx += 1;
    }
    assert_eq!(n_tx, 217);
    assert_eq!((n_tr, n_cr, n_co), (86, 26, 3));
}

#[test]
fn bonding_curve_accounts_decode_with_standard_invariants() {
    // stato a fine slot delle curve nei trade delle fixture: ricostruito e confrontato con i TradeEvent
    let wsol = b58::decode32(WSOL).unwrap();
    let mut acct = vec![0u8; 166];
    acct[..8].copy_from_slice(&decode::DISC_BONDING_CURVE);
    let s = CurveState { vq: VS0 + 5_000_000_000, vt: 900_000_000_000_000, rq: 5_000_000_000, rt: 900_000_000_000_000 - VT_MINUS_RT, complete: false };
    acct[8..16].copy_from_slice(&s.vt.to_le_bytes());
    acct[16..24].copy_from_slice(&s.vq.to_le_bytes());
    acct[24..32].copy_from_slice(&s.rt.to_le_bytes());
    acct[32..40].copy_from_slice(&s.rq.to_le_bytes());
    let (d, st) = decode::decode_bonding_curve(&acct, &wsol).unwrap();
    assert_eq!(d, s);
    assert!(st.quote_sol && !st.mayhem && st.in_universe());
    assert!(d.is_standard());
}
