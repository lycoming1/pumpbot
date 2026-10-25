use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use tokio::sync::mpsc::unbounded_channel;
use tracing::{info, warn};

use pumpbot::config::Config;
use pumpbot::core::Core;
use pumpbot::engine::Engine;
use pumpbot::feed::{self, DynOpts, StreamStats, SubManager};
use pumpbot::notify::{self, Note};
use pumpbot::state::MarketState;
use pumpbot::store::{self, Rec, StoreOpts, now_ms};
use pumpbot::types::*;
use pumpbot::{replay, report};

fn usage() -> ! {
    eprintln!(
        "uso:
  pumpbot run [--config /config/pumpbot.toml] [--secs N]
  pumpbot probe --secs N [--config ...]
  pumpbot report [--db /data/paper.db] [--since AAAA-MM-GG] [--filter id|nome]
  pumpbot replay --events a.db[,b.db] --out replay.db [--config ...]
  pumpbot healthcheck [--db /data/heartbeat]"
    );
    std::process::exit(2)
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn load_cfg(args: &[String]) -> anyhow::Result<Config> {
    match arg(args, "--config") {
        Some(p) => Config::load(&p).with_context(|| format!("config {p}")),
        None => {
            let p = "/config/pumpbot.toml";
            if std::path::Path::new(p).exists() { Config::load(p) } else { Ok(Config::default()) }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    tracing_subscriber::fmt().with_ansi(false).with_target(false).with_max_level(tracing::Level::INFO).init();
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("run");
    match cmd {
        "run" => {
            let cfg = load_cfg(&args)?;
            let secs: Option<u64> = arg(&args, "--secs").and_then(|s| s.parse().ok());
            runtime()?.block_on(live(cfg, false, secs))
        }
        "probe" => {
            let cfg = load_cfg(&args)?;
            let secs: u64 = arg(&args, "--secs").and_then(|s| s.parse().ok()).unwrap_or(120);
            runtime()?.block_on(live(cfg, true, Some(secs)))
        }
        "report" => {
            let db = arg(&args, "--db").unwrap_or_else(|| "/data/paper.db".into());
            report::cli(&db, arg(&args, "--since").as_deref(), arg(&args, "--filter").as_deref())
        }
        "replay" => {
            let cfg = load_cfg(&args)?;
            let ev = arg(&args, "--events").unwrap_or_else(|| usage());
            let out = arg(&args, "--out").unwrap_or_else(|| usage());
            let files: Vec<PathBuf> = ev.split(',').map(PathBuf::from).collect();
            replay::run(&cfg, files, PathBuf::from(out))
        }
        "healthcheck" => {
            let p = arg(&args, "--db").unwrap_or_else(|| "/data/heartbeat".into());
            let t: u64 = std::fs::read_to_string(&p)?.trim().parse()?;
            if now_ms().saturating_sub(t) > 180_000 {
                bail!("heartbeat vecchio");
            }
            Ok(())
        }
        _ => usage(),
    }
}

fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread().enable_all().build()?)
}

fn prog_sub(commitment: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"programSubscribe","params":["{PUMP}",{{"encoding":"base64","commitment":"{commitment}","filters":[{{"memcmp":{{"offset":0,"bytes":"{BONDING_CURVE_DISC_B58}"}}}}]}}]}}"#
    )
}

fn create_sub(commitment: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":2,"method":"logsSubscribe","params":[{{"mentions":["{MINT_AUTHORITY}"]}},{{"commitment":"{commitment}"}}]}}"#)
}

const SLOT_SUB: &str = r#"{"jsonrpc":"2.0","id":3,"method":"slotSubscribe"}"#;

struct Rate {
    last: Instant,
    prev: Vec<u64>,
}

async fn live(cfg: Config, probe: bool, probe_secs: Option<u64>) -> anyhow::Result<()> {
    let dir = PathBuf::from(&cfg.storage.dir);
    let paper_path = dir.join(if probe { "probe-paper.db" } else { "paper.db" });
    std::fs::create_dir_all(&dir)?;
    if !probe {
        let n = store::abort_open_positions(&paper_path)?;
        if n > 0 {
            warn!("{n} posizioni aperte dal processo precedente chiuse come aborted_restart");
        }
    }
    let (rec, writer) = store::spawn(StoreOpts {
        dir: dir.clone(),
        events: !probe,
        paper_path: paper_path.clone(),
        batch_rows: cfg.storage.batch_rows,
        batch_ms: cfg.storage.batch_ms,
        retention_days: cfg.storage.retention_days,
    })?;
    let strats = if probe { Vec::new() } else { cfg.strategies() };
    info!("{} configurazioni di paper trading", strats.len());
    for s in &strats {
        let _ = rec.send(Rec::Config { id: s.id.clone(), name: s.name.clone(), params: format!("{s:?}") });
    }
    let notifier = if probe { None } else { Some(notify::spawn(cfg.telegram.clone(), paper_path.to_string_lossy().into_owned())) };
    if let Some(n) = &notifier {
        let _ = n.try_send(Note::Text(format!(
            "pumpbot avviato: {} configurazioni di paper trading, report ogni {} h",
            cfg.strategies().len(),
            cfg.telegram.report_every_hours
        )));
    }

    let (ftx, mut frx) = unbounded_channel();
    let f = &cfg.feed;
    let mut statics: Vec<Arc<StreamStats>> = Vec::new();
    for (i, url) in f.ws_urls.iter().enumerate() {
        let st = StreamStats::new(&format!("tierA-{i}"));
        statics.push(st.clone());
        tokio::spawn(feed::static_stream(url.clone(), vec![prog_sub(&f.commitment)], ftx.clone(), st, f.ping_secs, f.idle_reconnect_secs));
        let st = StreamStats::new(&format!("tierC-{i}"));
        statics.push(st.clone());
        tokio::spawn(feed::static_stream(url.clone(), vec![create_sub(&f.commitment)], ftx.clone(), st, f.ping_secs, 0));
    }
    if let Some(url) = f.ws_urls.first() {
        let st = StreamStats::new("clock");
        statics.push(st.clone());
        tokio::spawn(feed::static_stream(url.clone(), vec![SLOT_SUB.into()], ftx.clone(), st, f.ping_secs, f.idle_reconnect_secs));
    }
    let mut subs = SubManager::new(
        f.ws_urls.clone(),
        f.max_subs_per_conn,
        f.redundant_tier_b,
        ftx.clone(),
        DynOpts { ping_secs: f.ping_secs, commitment: f.commitment.clone(), spacing_ms: f.sub_spacing_ms },
    );
    let engine = Engine::new(strats, cfg.engine.clone(), rec.clone());
    let mut core = Core::new(MarketState::new(cfg.feed.clone()), engine, if probe { None } else { Some(rec.clone()) });

    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut minute = tokio::time::interval(Duration::from_secs(if probe { 10 } else { 60 }));
    minute.tick().await;
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let deadline = probe_secs.map(|s| Instant::now() + Duration::from_secs(s));
    let mut rate = Rate { last: Instant::now(), prev: Vec::new() };
    let mut last_alert: std::collections::HashMap<&'static str, Instant> = std::collections::HashMap::new();
    let started = Instant::now();
    loop {
        tokio::select! {
            m = frx.recv() => {
                let Some(m) = m else { break };
                if let Some(ev) = core.feed_to_event(m) {
                    core.handle(ev);
                }
            }
            _ = tick.tick() => {
                if let Some((add, rem)) = core.tick() {
                    for k in rem { subs.remove(&k); }
                    for k in add { subs.add(k); }
                }
                if let Some(d) = deadline { if Instant::now() >= d { break; } }
            }
            _ = minute.tick() => {
                let line = health(&core, &statics, &subs, &mut rate);
                info!("{line}");
                if !probe {
                    let _ = rec.try_send(Rec::Health { ts: now_ms(), data: line.clone() });
                    let _ = std::fs::write(dir.join("heartbeat"), now_ms().to_string());
                    if let Some(n) = &notifier {
                        alerts(&cfg, &core, &statics, n, &mut last_alert, started);
                    }
                }
            }
            _ = sigterm.recv() => { info!("SIGTERM"); break; }
            _ = tokio::signal::ctrl_c() => { info!("SIGINT"); break; }
        }
    }
    if probe_secs.is_some() {
        println!("{}", health(&core, &statics, &subs, &mut rate));
        println!("{:?}", core.st.stats);
        println!("{:?}", core.stats);
    }
    let _ = rec.send(Rec::Flush);
    drop(core);
    drop(rec);
    drop(subs);
    let _ = tokio::task::spawn_blocking(move || writer.join()).await;
    Ok(())
}

fn health(core: &Core, statics: &[Arc<StreamStats>], subs: &SubManager, rate: &mut Rate) -> String {
    let el = rate.last.elapsed().as_secs_f64().max(1e-3);
    rate.last = Instant::now();
    let mut cur: Vec<u64> = statics.iter().map(|s| s.msgs.load(Ordering::Relaxed)).collect();
    let tb_msgs: u64 = subs.stats.iter().map(|s| s.msgs.load(Ordering::Relaxed)).sum();
    cur.push(tb_msgs);
    cur.push(core.stats.txs);
    cur.push(core.stats.trades);
    let prev = if rate.prev.len() == cur.len() { rate.prev.clone() } else { vec![0; cur.len()] };
    let r: Vec<f64> = cur.iter().zip(prev.iter()).map(|(c, p)| c.saturating_sub(*p) as f64 / el).collect();
    rate.prev = cur;
    let mut s = String::new();
    for (i, st) in statics.iter().enumerate() {
        s.push_str(&format!(
            "{}={:.1}/s{}r{} ",
            st.name,
            r[i],
            if st.connected.load(Ordering::Relaxed) { "" } else { "(giù)" },
            st.reconnects.load(Ordering::Relaxed)
        ));
    }
    let n = statics.len();
    let tb_rec: u64 = subs.stats.iter().map(|s| s.reconnects.load(Ordering::Relaxed)).sum();
    let tb_err: u64 = subs.stats.iter().map(|s| s.sub_errors.load(Ordering::Relaxed)).sum();
    let ss = &core.st.stats;
    format!(
        "{s}tierB={:.1}/s conn={} subs={}/{} r{} suberr={} | tx={:.1}/s trade={:.1}/s | curve={} watched={} dirty={} | fin={} breaks={} gravi={} mismatch={} missed={} late_a={} late_t={} unmapped={} trunc={} dup_logs={} lagT={:?} lagA={:?} | slot={} open={} sig={} entrate={} fallite={} chiuse={}",
        r[n],
        subs.stats.len(),
        subs.n_active_subs(),
        subs.n_assigned(),
        tb_rec,
        tb_err,
        r[n + 1],
        r[n + 2],
        core.st.curves.len(),
        core.st.n_watched(),
        core.st.n_dirty(),
        ss.finalized_slots,
        ss.chain_breaks,
        ss.material_gaps,
        ss.a_mismatch,
        ss.missed_while_watched,
        ss.late_a,
        ss.late_trades,
        ss.unmapped_trades,
        ss.truncated,
        core.stats.logs_dup,
        ss.lag_trades,
        ss.lag_states,
        core.st.clock,
        core.eng.n_open(),
        core.eng.stats.signals,
        core.eng.stats.entries,
        core.eng.stats.entry_failed,
        core.eng.stats.closed,
    )
}

fn alerts(
    cfg: &Config,
    core: &Core,
    statics: &[Arc<StreamStats>],
    n: &std::sync::mpsc::SyncSender<Note>,
    last: &mut std::collections::HashMap<&'static str, Instant>,
    started: Instant,
) {
    let mut fire = |kind: &'static str, text: String| {
        if last.get(kind).map(|t| t.elapsed() < Duration::from_secs(1800)).unwrap_or(false) {
            return;
        }
        last.insert(kind, Instant::now());
        let _ = n.try_send(Note::Text(text));
    };
    if started.elapsed() < Duration::from_secs(120) {
        return;
    }
    let silent_ns = cfg.telegram.alert_feed_silent_secs * 1_000_000_000;
    let tier_a_up = statics.iter().any(|s| s.name.starts_with("tierA") && s.connected.load(Ordering::Relaxed));
    if !tier_a_up || store::now_ns().saturating_sub(core.last_feed_ns) > silent_ns {
        fire("feed", format!("pumpbot: feed Tier A fermo da oltre {} s", cfg.telegram.alert_feed_silent_secs));
    }
    let d = core.st.n_dirty();
    if d > cfg.telegram.alert_dirty_threshold {
        fire("dirty", format!("pumpbot: {d} mint dirty (soglia {})", cfg.telegram.alert_dirty_threshold));
    }
}
