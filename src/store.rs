//! Thread di scrittura SQLite: eventi giornalieri (WAL, batch) e paper.db (synchronous=FULL).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use tracing::{info, warn};

use crate::types::*;

pub enum Rec {
    CurveInfo { curve: Pubkey, stat: CurveStatic },
    State { slot: u64, curve: Pubkey, s: CurveState, arr: u64 },
    Trade { t: TradeRec, sub_curve: Option<Pubkey>, arr: u64 },
    Create { slot: u64, sig: Sig, c: CreateRec, arr: u64 },
    Complete { slot: u64, sig: Sig, c: CompleteRec, arr: u64 },
    Health { ts: u64, data: String },
    Config { id: String, name: String, params: String },
    Signal { key: i64, band: &'static str, variant: &'static str, mint: Pubkey, curve: Pubkey, slot: u64, price: f64, features: String },
    PaperTrade {
        config_id: String,
        signal_key: i64,
        mint: Pubkey,
        curve: Pubkey,
        signal_slot: u64,
        entry_slot: u64,
        entry_price: f64,
        sol_in: u64,
        tokens: u64,
        exit_reason: String,
        exit_trigger_slot: u64,
        exit_slot: u64,
        sol_out: u64,
        retries: u32,
        cost_comp: f64,
        cost_eco: f64,
        pnl_comp: f64,
        pnl_eco: f64,
        outcome: String,
    },
    OpenPos { config_id: String, curve: Pubkey, data: String },
    ClosePos { config_id: String, curve: Pubkey },
    Flush,
}

pub struct StoreOpts {
    pub dir: PathBuf,
    /// None = non registrare gli eventi (replay)
    pub events: bool,
    pub paper_path: PathBuf,
    pub batch_rows: usize,
    pub batch_ms: u64,
    pub retention_days: u64,
}

const EVENTS_SCHEMA: &str = "
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA temp_store=MEMORY;
PRAGMA cache_size=-16384;
PRAGMA wal_autocheckpoint=4000;
PRAGMA journal_size_limit=67108864;
CREATE TABLE IF NOT EXISTS keys(id INTEGER PRIMARY KEY, k BLOB NOT NULL UNIQUE);
CREATE TABLE IF NOT EXISTS curves(curve_id INTEGER PRIMARY KEY, creator_id INTEGER, mayhem INTEGER, cashback INTEGER, quote_sol INTEGER, holder INTEGER);
CREATE TABLE IF NOT EXISTS states(slot INTEGER, curve_id INTEGER, vq INTEGER, vt INTEGER, rq INTEGER, rt INTEGER, complete INTEGER, arr INTEGER);
CREATE TABLE IF NOT EXISTS trades(slot INTEGER, sig BLOB, ev INTEGER, mint_id INTEGER, user_id INTEGER, creator_id INTEGER, is_buy INTEGER, quote INTEGER, tokens INTEGER, vq INTEGER, vt INTEGER, rq INTEGER, rt INTEGER, fee INTEGER, cfee INTEGER, fee_bps INTEGER, cfee_bps INTEGER, mayhem INTEGER, quote_sol INTEGER, ix INTEGER, sub_curve_id INTEGER, arr INTEGER);
CREATE TABLE IF NOT EXISTS creates(slot INTEGER, sig BLOB, mint_id INTEGER, curve_id INTEGER, user_id INTEGER, creator_id INTEGER, mayhem INTEGER, cashback INTEGER, quote_sol INTEGER, holder INTEGER, name TEXT, symbol TEXT, uri TEXT, arr INTEGER);
CREATE TABLE IF NOT EXISTS completes(slot INTEGER, sig BLOB, kind INTEGER, mint_id INTEGER, curve_id INTEGER, pool_id INTEGER, a INTEGER, b INTEGER, c INTEGER, arr INTEGER);
CREATE TABLE IF NOT EXISTS health(ts INTEGER, data TEXT);
";

pub const PAPER_SCHEMA: &str = "
PRAGMA journal_mode=WAL;
PRAGMA synchronous=FULL;
CREATE TABLE IF NOT EXISTS configs(id TEXT PRIMARY KEY, name TEXT, params TEXT, created INTEGER);
CREATE TABLE IF NOT EXISTS signals(key INTEGER PRIMARY KEY, band TEXT, variant TEXT, mint TEXT, curve TEXT, slot INTEGER, price REAL, features TEXT, ts INTEGER);
CREATE TABLE IF NOT EXISTS trades(id INTEGER PRIMARY KEY, config_id TEXT, signal_key INTEGER, mint TEXT, curve TEXT, signal_slot INTEGER, entry_slot INTEGER, entry_price REAL, sol_in INTEGER, tokens INTEGER, exit_reason TEXT, exit_trigger_slot INTEGER, exit_slot INTEGER, sol_out INTEGER, retries INTEGER, cost_comp REAL, cost_eco REAL, pnl_comp REAL, pnl_eco REAL, r_comp REAL, r_eco REAL, outcome TEXT, ts INTEGER);
CREATE INDEX IF NOT EXISTS trades_cfg ON trades(config_id, outcome);
CREATE TABLE IF NOT EXISTS open_positions(config_id TEXT, curve TEXT, data TEXT, PRIMARY KEY(config_id, curve));
";

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

pub fn now_ns() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
}

/// Data UTC (AAAAMMGG) da secondi unix, senza crate di date.
pub fn utc_date(unix_s: u64) -> String {
    let days = (unix_s / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}{m:02}{d:02}")
}

/// Algoritmo di Howard Hinnant.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

struct EventsDb {
    date: String,
    conn: Connection,
    keys: HashMap<Pubkey, i64>,
    curves_seen: HashMap<Pubkey, ()>,
}

impl EventsDb {
    fn open(dir: &Path, date: &str) -> rusqlite::Result<Self> {
        let path = dir.join(format!("events-{date}.db"));
        let conn = Connection::open(&path)?;
        conn.execute_batch(EVENTS_SCHEMA)?;
        let mut keys = HashMap::new();
        {
            let mut st = conn.prepare("SELECT id, k FROM keys")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?;
            for row in rows.flatten() {
                if let Ok(k) = <[u8; 32]>::try_from(row.1.as_slice()) {
                    keys.insert(k, row.0);
                }
            }
        }
        Ok(EventsDb { date: date.to_string(), conn, keys, curves_seen: HashMap::new() })
    }

    fn key(&mut self, k: &Pubkey) -> rusqlite::Result<i64> {
        if let Some(id) = self.keys.get(k) {
            return Ok(*id);
        }
        self.conn.prepare_cached("INSERT INTO keys(k) VALUES (?1)")?.execute(params![k.as_slice()])?;
        let id = self.conn.last_insert_rowid();
        self.keys.insert(*k, id);
        Ok(id)
    }
}

pub fn spawn(opts: StoreOpts) -> anyhow::Result<(SyncSender<Rec>, JoinHandle<()>)> {
    std::fs::create_dir_all(&opts.dir)?;
    let paper = Connection::open(&opts.paper_path)?;
    paper.execute_batch(PAPER_SCHEMA)?;
    let (tx, rx) = sync_channel::<Rec>(65_536);
    let h = std::thread::Builder::new().name("writer".into()).spawn(move || writer_loop(opts, paper, rx))?;
    Ok((tx, h))
}

fn writer_loop(opts: StoreOpts, paper: Connection, rx: Receiver<Rec>) {
    let mut ev: Option<EventsDb> = None;
    let mut in_tx = false;
    let mut rows = 0usize;
    let mut last_commit = Instant::now();
    let mut errors = 0u64;
    // informazioni statiche delle curve, per riscriverle nel file del giorno nuovo
    let mut stat_cache: HashMap<Pubkey, CurveStatic> = HashMap::new();
    loop {
        let msg = rx.recv_timeout(Duration::from_millis(opts.batch_ms));
        let rec = match msg {
            Ok(r) => Some(r),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if let Some(rec) = rec {
            let flush = matches!(rec, Rec::Flush);
            if is_event(&rec) && opts.events {
                let date = utc_date(now_ms() / 1000);
                if ev.as_ref().map(|e| e.date != date).unwrap_or(true) {
                    if let Some(e) = ev.as_ref() {
                        if in_tx {
                            let _ = e.conn.execute_batch("COMMIT");
                            in_tx = false;
                        }
                    }
                    ev = match EventsDb::open(&opts.dir, &date) {
                        Ok(e) => Some(e),
                        Err(e) => {
                            warn!("apertura events db fallita: {e}");
                            None
                        }
                    };
                    cleanup_old(&opts.dir, opts.retention_days);
                }
                if let Some(e) = ev.as_mut() {
                    if !in_tx {
                        if e.conn.execute_batch("BEGIN").is_ok() {
                            in_tx = true;
                        }
                    }
                    if let Err(err) = write_event(e, &mut stat_cache, rec) {
                        errors += 1;
                        if errors % 1000 == 1 {
                            warn!("scrittura evento fallita ({errors}): {err}");
                        }
                    }
                    rows += 1;
                }
            } else if !flush {
                if let Err(err) = write_paper(&paper, rec) {
                    warn!("scrittura paper fallita: {err}");
                }
            }
            if flush {
                rows = opts.batch_rows;
            }
        }
        if in_tx && (rows >= opts.batch_rows || last_commit.elapsed() >= Duration::from_millis(opts.batch_ms)) {
            if let Some(e) = ev.as_ref() {
                if let Err(err) = e.conn.execute_batch("COMMIT") {
                    warn!("commit fallito: {err}");
                }
            }
            in_tx = false;
            rows = 0;
            last_commit = Instant::now();
        } else if !in_tx {
            last_commit = Instant::now();
        }
    }
    if in_tx {
        if let Some(e) = ev.as_ref() {
            let _ = e.conn.execute_batch("COMMIT");
        }
    }
    info!("writer chiuso");
}

fn is_event(r: &Rec) -> bool {
    matches!(r, Rec::CurveInfo { .. } | Rec::State { .. } | Rec::Trade { .. } | Rec::Create { .. } | Rec::Complete { .. } | Rec::Health { .. })
}

fn write_curve(e: &mut EventsDb, curve: &Pubkey, stat: &CurveStatic) -> rusqlite::Result<()> {
    let cid = e.key(curve)?;
    let crid = e.key(&stat.creator)?;
    e.conn.prepare_cached("INSERT OR IGNORE INTO curves VALUES (?1,?2,?3,?4,?5,?6)")?.execute(params![
        cid,
        crid,
        stat.mayhem,
        stat.cashback,
        stat.quote_sol,
        stat.holder_reward
    ])?;
    e.curves_seen.insert(*curve, ());
    Ok(())
}

fn write_event(e: &mut EventsDb, cache: &mut HashMap<Pubkey, CurveStatic>, rec: Rec) -> rusqlite::Result<()> {
    match rec {
        Rec::CurveInfo { curve, stat } => {
            cache.insert(curve, stat);
            if e.curves_seen.contains_key(&curve) {
                return Ok(());
            }
            write_curve(e, &curve, &stat)?;
        }
        Rec::State { slot, curve, s, arr } => {
            if !e.curves_seen.contains_key(&curve) {
                if let Some(stat) = cache.get(&curve).copied() {
                    write_curve(e, &curve, &stat)?;
                }
            }
            let cid = e.key(&curve)?;
            e.conn.prepare_cached("INSERT INTO states VALUES (?1,?2,?3,?4,?5,?6,?7,?8)")?.execute(params![
                slot as i64,
                cid,
                s.vq as i64,
                s.vt as i64,
                s.rq as i64,
                s.rt as i64,
                s.complete,
                arr as i64
            ])?;
        }
        Rec::Trade { t, sub_curve, arr } => {

            let mid = e.key(&t.mint)?;
            let uid = e.key(&t.user)?;
            let crid = e.key(&t.creator)?;
            let scid = match sub_curve {
                Some(c) => Some(e.key(&c)?),
                None => None,
            };
            e.conn.prepare_cached("INSERT INTO trades VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)")?.execute(params![
                t.slot as i64,
                t.sig.as_slice(),
                t.ev_idx,
                mid,
                uid,
                crid,
                t.is_buy,
                t.quote as i64,
                t.tokens as i64,
                t.post.vq as i64,
                t.post.vt as i64,
                t.post.rq as i64,
                t.post.rt as i64,
                t.fee as i64,
                t.cfee as i64,
                t.fee_bps as i64,
                t.cfee_bps as i64,
                t.mayhem,
                t.quote_sol,
                t.ix,
                scid,
                arr as i64
            ])?;
        }
        Rec::Create { slot, sig, c, arr } => {
            let mid = e.key(&c.mint)?;
            let cid = e.key(&c.curve)?;
            let uid = e.key(&c.user)?;
            let crid = e.key(&c.creator)?;
            e.conn.prepare_cached("INSERT INTO creates VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)")?.execute(params![
                slot as i64,
                sig.as_slice(),
                mid,
                cid,
                uid,
                crid,
                c.mayhem,
                c.cashback,
                c.quote_sol,
                c.holder_reward,
                c.name,
                c.symbol,
                c.uri,
                arr as i64
            ])?;
        }
        Rec::Complete { slot, sig, c, arr } => {
            let mid = e.key(&c.mint)?;
            let cid = e.key(&c.curve)?;
            let (kind, pool, a, b, cc) = match c.kind {
                CompleteKind::Complete => (0, None, 0u64, 0u64, 0u64),
                CompleteKind::Migration { pool, mint_amount, sol_amount, migration_fee } => (1, Some(pool), mint_amount, sol_amount, migration_fee),
                // a = base del pool dopo, b = quote del pool dopo, c = quote speso oltre la curva
                CompleteKind::PostCompleteBuy { base_out: _, quote_in, pool_base_after, pool_quote_after } => {
                    (2, None, pool_base_after, pool_quote_after, quote_in)
                }
            };
            let pid = match pool {
                Some(p) => Some(e.key(&p)?),
                None => None,
            };
            e.conn.prepare_cached("INSERT INTO completes VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)")?.execute(params![
                slot as i64,
                sig.as_slice(),
                kind,
                mid,
                cid,
                pid,
                a as i64,
                b as i64,
                cc as i64,
                arr as i64
            ])?;
        }
        Rec::Health { ts, data } => {
            e.conn.prepare_cached("INSERT INTO health VALUES (?1,?2)")?.execute(params![ts as i64, data])?;
        }
        _ => {}
    }
    Ok(())
}

fn write_paper(c: &Connection, rec: Rec) -> rusqlite::Result<()> {
    let ts = now_ms() as i64;
    match rec {
        Rec::Config { id, name, params } => {
            c.prepare_cached("INSERT OR IGNORE INTO configs VALUES (?1,?2,?3,?4)")?.execute(params![id, name, params, ts])?;
        }
        Rec::Signal { key, band, variant, mint, curve, slot, price, features } => {
            c.prepare_cached("INSERT OR IGNORE INTO signals VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)")?.execute(params![
                key,
                band,
                variant,
                crate::b58::encode32(&mint),
                crate::b58::encode32(&curve),
                slot as i64,
                price,
                features,
                ts
            ])?;
        }
        Rec::PaperTrade {
            config_id,
            signal_key,
            mint,
            curve,
            signal_slot,
            entry_slot,
            entry_price,
            sol_in,
            tokens,
            exit_reason,
            exit_trigger_slot,
            exit_slot,
            sol_out,
            retries,
            cost_comp,
            cost_eco,
            pnl_comp,
            pnl_eco,
            outcome,
        } => {
            let b = if sol_in > 0 { sol_in as f64 / 1e9 } else { f64::NAN };
            c.prepare_cached(
                "INSERT INTO trades(config_id, signal_key, mint, curve, signal_slot, entry_slot, entry_price, sol_in, tokens, exit_reason, exit_trigger_slot, exit_slot, sol_out, retries, cost_comp, cost_eco, pnl_comp, pnl_eco, r_comp, r_eco, outcome, ts) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
            )?
            .execute(params![
                config_id,
                signal_key,
                crate::b58::encode32(&mint),
                crate::b58::encode32(&curve),
                signal_slot as i64,
                entry_slot as i64,
                entry_price,
                sol_in as i64,
                tokens as i64,
                exit_reason,
                exit_trigger_slot as i64,
                exit_slot as i64,
                sol_out as i64,
                retries,
                cost_comp,
                cost_eco,
                pnl_comp,
                pnl_eco,
                if b.is_nan() { None } else { Some(pnl_comp / b) },
                if b.is_nan() { None } else { Some(pnl_eco / b) },
                outcome,
                ts
            ])?;
        }
        Rec::OpenPos { config_id, curve, data } => {
            c.prepare_cached("INSERT OR REPLACE INTO open_positions VALUES (?1,?2,?3)")?.execute(params![
                config_id,
                crate::b58::encode32(&curve),
                data
            ])?;
        }
        Rec::ClosePos { config_id, curve } => {
            c.prepare_cached("DELETE FROM open_positions WHERE config_id=?1 AND curve=?2")?.execute(params![
                config_id,
                crate::b58::encode32(&curve)
            ])?;
        }
        _ => {}
    }
    Ok(())
}

fn cleanup_old(dir: &Path, retention_days: u64) {
    let cutoff = utc_date(now_ms() / 1000 - retention_days * 86_400);
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(d) = name.strip_prefix("events-").and_then(|s| s.strip_suffix(".db")) {
            if d < cutoff.as_str() {
                info!("retention: rimuovo {name}");
                let _ = std::fs::remove_file(e.path());
                let _ = std::fs::remove_file(dir.join(format!("{name}-wal")));
                let _ = std::fs::remove_file(dir.join(format!("{name}-shm")));
            }
        }
    }
}

/// All'avvio: le posizioni rimaste aperte da un'esecuzione precedente vengono chiuse come `aborted_restart`.
pub fn abort_open_positions(paper_path: &Path) -> anyhow::Result<usize> {
    let c = Connection::open(paper_path)?;
    c.execute_batch(PAPER_SCHEMA)?;
    let rows: Vec<(String, String, String)> = {
        let mut st = c.prepare("SELECT config_id, curve, data FROM open_positions")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.flatten().collect()
    };
    let ts = now_ms() as i64;
    for (cfg, curve, data) in &rows {
        let v: serde_json::Value = serde_json::from_str(data).unwrap_or_default();
        c.execute(
            "INSERT INTO trades(config_id, signal_key, curve, signal_slot, exit_reason, outcome, ts, pnl_comp, pnl_eco) VALUES (?1,?2,?3,?4,'restart','aborted_restart',?5,0,0)",
            params![cfg, v["signal_key"].as_i64().unwrap_or(0), curve, v["signal_slot"].as_i64().unwrap_or(0), ts],
        )?;
    }
    c.execute("DELETE FROM open_positions", [])?;
    Ok(rows.len())
}
