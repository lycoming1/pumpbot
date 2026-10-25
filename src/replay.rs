//! Replay: rigioca il motore sugli eventi registrati, nell'ordine di arrivo, con le stesse regole del vivo.

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags};
use tracing::info;

use crate::config::Config;
use crate::core::Core;
use crate::engine::Engine;
use crate::state::MarketState;
use crate::store::{self, Rec, StoreOpts};
use crate::types::*;

struct Keys(HashMap<i64, Pubkey>);
impl Keys {
    fn get(&self, id: Option<i64>) -> Pubkey {
        id.and_then(|i| self.0.get(&i).copied()).unwrap_or(ZERO_KEY)
    }
}

/// Un elemento del flusso, con priorità a parità di arrivo: create e complete prima dei trade.
enum Item {
    State(u64, Event),
    Create(u64, Vec<u8>, u64, CreateRec),
    Complete(u64, Vec<u8>, u64, CompleteRec),
    Trade(u64, Vec<u8>, Option<Pubkey>, TradeRec),
}

impl Item {
    fn arr(&self) -> u64 {
        match self {
            Item::State(a, _) | Item::Create(a, ..) | Item::Complete(a, ..) | Item::Trade(a, ..) => *a,
        }
    }
    fn prio(&self) -> u8 {
        match self {
            Item::Create(..) => 0,
            Item::Complete(..) => 1,
            Item::Trade(..) => 2,
            Item::State(..) => 3,
        }
    }
}

fn sig_of(b: &[u8]) -> Sig {
    let mut s = [0u8; 64];
    if b.len() == 64 {
        s.copy_from_slice(b);
    }
    s
}

fn row_state(r: &rusqlite::Row, keys: &Keys, stats: &HashMap<i64, CurveStatic>) -> rusqlite::Result<Item> {
    let (slot, cid, vq, vt, rq, rt, complete, arr): (i64, i64, i64, i64, i64, i64, bool, i64) =
        (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?);
    // senza informazioni statiche la curva si assume nell'universo (si registrano solo curve dell'universo)
    let stat = stats.get(&cid).copied().unwrap_or(CurveStatic { quote_sol: true, ..Default::default() });
    let state = CurveState { vq: vq as u64, vt: vt as u64, rq: rq as u64, rt: rt as u64, complete };
    Ok(Item::State(arr as u64, Event::Account { slot: slot as u64, curve: keys.get(Some(cid)), state, stat, arrival_ns: arr as u64 }))
}

fn row_create(r: &rusqlite::Row, keys: &Keys) -> rusqlite::Result<Item> {
    let c = CreateRec {
        name: r.get(10)?,
        symbol: r.get(11)?,
        uri: r.get(12)?,
        mint: keys.get(Some(r.get(2)?)),
        curve: keys.get(Some(r.get(3)?)),
        user: keys.get(Some(r.get(4)?)),
        creator: keys.get(Some(r.get(5)?)),
        mayhem: r.get(6)?,
        cashback: r.get(7)?,
        quote_sol: r.get(8)?,
        holder_reward: r.get(9)?,
    };
    Ok(Item::Create(r.get::<_, i64>(13)? as u64, r.get(1)?, r.get::<_, i64>(0)? as u64, c))
}

fn row_complete(r: &rusqlite::Row, keys: &Keys) -> rusqlite::Result<Item> {
    let (a, b, cc): (i64, i64, i64) = (r.get(6)?, r.get(7)?, r.get(8)?);
    let kind = match r.get::<_, i64>(2)? {
        1 => CompleteKind::Migration { pool: keys.get(r.get(5)?), mint_amount: a as u64, sol_amount: b as u64, migration_fee: cc as u64 },
        2 => CompleteKind::PostCompleteBuy { base_out: 0, quote_in: cc as u64, pool_base_after: a as u64, pool_quote_after: b as u64 },
        _ => CompleteKind::Complete,
    };
    let rec = CompleteRec { mint: keys.get(Some(r.get(3)?)), curve: keys.get(Some(r.get(4)?)), kind };
    Ok(Item::Complete(r.get::<_, i64>(9)? as u64, r.get(1)?, r.get::<_, i64>(0)? as u64, rec))
}

fn row_trade(r: &rusqlite::Row, keys: &Keys) -> rusqlite::Result<Item> {
    let g = |i: usize| -> rusqlite::Result<u64> { Ok(r.get::<_, i64>(i)? as u64) };
    let rt = g(12)?;
    let t = TradeRec {
        slot: g(0)?,
        sig: sig_of(&r.get::<_, Vec<u8>>(1)?),
        ev_idx: g(2)? as u16,
        mint: keys.get(Some(r.get(3)?)),
        user: keys.get(Some(r.get(4)?)),
        creator: keys.get(Some(r.get(5)?)),
        is_buy: r.get(6)?,
        quote: g(7)?,
        tokens: g(8)?,
        post: CurveState { vq: g(9)?, vt: g(10)?, rq: g(11)?, rt, complete: rt == 0 },
        fee: g(13)?,
        cfee: g(14)?,
        fee_bps: g(15)?,
        cfee_bps: g(16)?,
        mayhem: r.get(17)?,
        quote_sol: r.get(18)?,
        ix: g(19)? as u8,
    };
    let sub: Option<i64> = r.get(20)?;
    Ok(Item::Trade(g(21)?, r.get(1)?, sub.map(|s| keys.get(Some(s))), t))
}

fn sig_bytes(it: &Item) -> Option<&Vec<u8>> {
    match it {
        Item::Create(_, s, ..) | Item::Complete(_, s, ..) | Item::Trade(_, s, ..) => Some(s),
        Item::State(..) => None,
    }
}

/// Rigioca un file eventi in streaming (merge delle quattro tabelle per arrivo). Ritorna il numero di eventi.
fn replay_file(path: &PathBuf, core: &mut Core) -> anyhow::Result<u64> {
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut keys = HashMap::new();
    {
        let mut st = c.prepare("SELECT id, k FROM keys")?;
        for r in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?.flatten() {
            if let Ok(k) = <[u8; 32]>::try_from(r.1.as_slice()) {
                keys.insert(r.0, k);
            }
        }
    }
    let keys = Keys(keys);
    let mut stats: HashMap<i64, CurveStatic> = HashMap::new();
    {
        let mut st = c.prepare("SELECT curve_id, creator_id, mayhem, cashback, quote_sol, holder FROM curves")?;
        for r in st
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, bool>(2)?, r.get::<_, bool>(3)?, r.get::<_, bool>(4)?, r.get::<_, bool>(5)?)))?
            .flatten()
        {
            stats.insert(r.0, CurveStatic { creator: keys.get(r.1), mayhem: r.2, cashback: r.3, quote_sol: r.4, holder_reward: r.5 });
        }
    }
    let mut s0 = c.prepare("SELECT slot, curve_id, vq, vt, rq, rt, complete, arr FROM states ORDER BY rowid")?;
    let mut s1 = c.prepare("SELECT slot, sig, mint_id, curve_id, user_id, creator_id, mayhem, cashback, quote_sol, holder, name, symbol, uri, arr FROM creates ORDER BY rowid")?;
    let mut s2 = c.prepare("SELECT slot, sig, kind, mint_id, curve_id, pool_id, a, b, c, arr FROM completes ORDER BY rowid")?;
    let mut s3 = c.prepare(
        "SELECT slot, sig, ev, mint_id, user_id, creator_id, is_buy, quote, tokens, vq, vt, rq, rt, fee, cfee, fee_bps, cfee_bps, mayhem, quote_sol, ix, sub_curve_id, arr FROM trades ORDER BY rowid",
    )?;
    let mut r0 = s0.query([])?;
    let mut r1 = s1.query([])?;
    let mut r2 = s2.query([])?;
    let mut r3 = s3.query([])?;
    let mut heads: [Option<Item>; 4] = [None, None, None, None];
    macro_rules! adv {
        ($i:expr) => {
            heads[$i] = match $i {
                0 => match r0.next()? { Some(r) => Some(row_state(r, &keys, &stats)?), None => None },
                1 => match r1.next()? { Some(r) => Some(row_create(r, &keys)?), None => None },
                2 => match r2.next()? { Some(r) => Some(row_complete(r, &keys)?), None => None },
                _ => match r3.next()? { Some(r) => Some(row_trade(r, &keys)?), None => None },
            }
        };
    }
    for i in 0..4 {
        adv!(i);
    }
    let mut n = 0u64;
    let mut last_tick = 0u64;
    loop {
        let best = (0..4).filter(|i| heads[*i].is_some()).min_by_key(|i| {
            let h = heads[*i].as_ref().unwrap();
            (h.arr(), h.prio())
        });
        let Some(b) = best else { break };
        let arr = heads[b].as_ref().unwrap().arr();
        if b == 0 {
            if let Some(Item::State(_, ev)) = heads[0].take() {
                core.handle(ev);
            }
            adv!(0);
            n += 1;
        } else {
            // create, complete e trade con la stessa firma e lo stesso arrivo formano una tx
            let sig = sig_bytes(heads[b].as_ref().unwrap()).cloned().unwrap_or_default();
            let mut tx = TxEvents { sig: sig_of(&sig), arrival_ns: arr, ..Default::default() };
            let mut sub_curve = None;
            for i in 1..4 {
                loop {
                    let same = heads[i].as_ref().map(|h| h.arr() == arr && sig_bytes(h) == Some(&sig)).unwrap_or(false);
                    if !same {
                        break;
                    }
                    match heads[i].take() {
                        Some(Item::Create(_, _, slot, c)) => {
                            tx.slot = slot;
                            tx.creates.push(c);
                        }
                        Some(Item::Complete(_, _, slot, c)) => {
                            tx.slot = slot;
                            tx.completes.push(c);
                        }
                        Some(Item::Trade(_, _, sub, t)) => {
                            tx.slot = t.slot;
                            sub_curve = sub;
                            tx.trades.push(t);
                        }
                        _ => {}
                    }
                    adv!(i);
                    n += 1;
                }
            }
            core.handle(Event::Tx { tx, sub_curve });
        }
        if arr >= last_tick + 100_000_000 {
            last_tick = arr;
            let _ = core.tick();
        }
    }
    Ok(n)
}

pub fn run(cfg: &Config, files: Vec<PathBuf>, out: PathBuf) -> anyhow::Result<()> {
    let strats = cfg.strategies();
    let out_dir = out.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let (rec, writer) = store::spawn(StoreOpts {
        dir: out_dir,
        events: false,
        paper_path: out.clone(),
        batch_rows: cfg.storage.batch_rows,
        batch_ms: cfg.storage.batch_ms,
        retention_days: cfg.storage.retention_days,
    })?;
    for s in &strats {
        let _ = rec.send(Rec::Config { id: s.id.clone(), name: s.name.clone(), params: format!("{s:?}") });
    }
    let engine = Engine::new(strats, cfg.engine.clone(), rec.clone());
    let mut core = Core::new(MarketState::new(cfg.feed.clone()), engine, Some(rec.clone()));
    let mut files = files;
    files.sort();
    for f in &files {
        let n = replay_file(f, &mut core)?;
        info!("replay {}: {} eventi", f.display(), n);
    }
    // chiusura: finalizza tutto, le posizioni ancora aperte non entrano nelle metriche
    let end = core.st.clock + core.st.cfg.finalize_lag_slots + 1;
    core.st.observe_slot(end);
    let _ = core.tick();
    core.eng.abort_all("replay_end");
    let _ = rec.send(Rec::Flush);
    drop(core);
    drop(rec);
    let _ = writer.join();
    Ok(())
}
