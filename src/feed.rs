//! WebSocket JSON-RPC Solana: connessione solo IPv4, riconnessione, flussi statici e pool di sottoscrizioni dinamiche.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use memchr::memmem;
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::{debug, info, warn};

use crate::store::now_ns;
use crate::types::Pubkey;

pub type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug)]
pub enum FeedMsg {
    Account { slot: u64, pubkey: String, data: String, arr: u64 },
    Logs { slot: u64, sig: String, err: bool, logs: Vec<String>, key: Option<Pubkey>, arr: u64 },
    Slot(u64),
}

#[derive(Default)]
pub struct StreamStats {
    pub name: String,
    pub msgs: AtomicU64,
    pub bytes: AtomicU64,
    pub skipped_no_data: AtomicU64,
    pub reconnects: AtomicU64,
    pub connected: AtomicBool,
    pub subs: AtomicU64,
    pub sub_errors: AtomicU64,
}

impl StreamStats {
    pub fn new(name: &str) -> Arc<Self> {
        Arc::new(StreamStats { name: name.to_string(), ..Default::default() })
    }
}

pub async fn connect(url: &str) -> anyhow::Result<Ws> {
    let req = url.into_client_request()?;
    let host = req.uri().host().ok_or_else(|| anyhow::anyhow!("url senza host"))?.to_string();
    let port = req.uri().port_u16().unwrap_or(443);
    let addrs: Vec<_> = tokio::net::lookup_host((host.as_str(), port)).await?.filter(|a| a.is_ipv4()).collect();
    let mut last_err = anyhow::anyhow!("nessun indirizzo IPv4 per {host}");
    for addr in addrs {
        match tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(addr)).await {
            Ok(Ok(s)) => {
                s.set_nodelay(true)?;
                let req = url.into_client_request()?;
                match tokio::time::timeout(Duration::from_secs(10), tokio_tungstenite::client_async_tls_with_config(req, s, None, None)).await {
                    Ok(Ok((ws, _))) => return Ok(ws),
                    Ok(Err(e)) => last_err = e.into(),
                    Err(_) => last_err = anyhow::anyhow!("timeout handshake"),
                }
            }
            Ok(Err(e)) => last_err = e.into(),
            Err(_) => last_err = anyhow::anyhow!("timeout connect"),
        }
    }
    Err(last_err)
}

// ---------------------------------------------------------------- parsing

#[derive(Deserialize)]
struct Ctx {
    slot: u64,
}
#[derive(Deserialize)]
struct LogsValue {
    signature: String,
    err: Option<serde::de::IgnoredAny>,
    #[serde(default)]
    logs: Option<Vec<String>>,
}
#[derive(Deserialize)]
struct LogsResult {
    context: Ctx,
    value: LogsValue,
}
#[derive(Deserialize)]
struct LogsParams {
    result: LogsResult,
    subscription: u64,
}
#[derive(Deserialize)]
struct LogsNotif {
    params: LogsParams,
}
#[derive(Deserialize)]
struct Acct {
    data: Vec<String>,
}
#[derive(Deserialize)]
struct ProgValue {
    pubkey: String,
    account: Acct,
}
#[derive(Deserialize)]
struct ProgResult {
    context: Ctx,
    value: ProgValue,
}
#[derive(Deserialize)]
struct ProgParams {
    result: ProgResult,
}
#[derive(Deserialize)]
struct ProgNotif {
    params: ProgParams,
}
#[derive(Deserialize)]
struct SlotResult {
    slot: u64,
}
#[derive(Deserialize)]
struct SlotParams {
    result: SlotResult,
}
#[derive(Deserialize)]
struct SlotNotif {
    params: SlotParams,
}
#[derive(Deserialize)]
struct Reply {
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
}

pub enum Parsed {
    Logs { sub: u64, slot: u64, sig: String, err: bool, logs: Vec<String> },
    Account { slot: u64, pubkey: String, data: String },
    Slot(u64),
    Reply { id: u64, sub: Option<u64>, error: Option<String> },
    Skip,
}

pub fn parse(t: &str, stats: &StreamStats) -> Parsed {
    let b = t.as_bytes();
    if memmem::find(b, b"\"logsNotification\"").is_some() {
        // Senza "Program data:" non ci sono eventi: non serve il parse JSON.
        if memmem::find(b, b"Program data: ").is_none() {
            stats.skipped_no_data.fetch_add(1, Ordering::Relaxed);
            return Parsed::Skip;
        }
        return match serde_json::from_str::<LogsNotif>(t) {
            Ok(n) => {
                let v = n.params.result.value;
                Parsed::Logs {
                    sub: n.params.subscription,
                    slot: n.params.result.context.slot,
                    sig: v.signature,
                    err: v.err.is_some(),
                    logs: v.logs.unwrap_or_default(),
                }
            }
            Err(e) => {
                debug!("logs non decodificabili: {e}");
                Parsed::Skip
            }
        };
    }
    if memmem::find(b, b"\"programNotification\"").is_some() {
        return match serde_json::from_str::<ProgNotif>(t) {
            Ok(n) => {
                let r = n.params.result;
                let data = r.value.account.data.into_iter().next().unwrap_or_default();
                Parsed::Account { slot: r.context.slot, pubkey: r.value.pubkey, data }
            }
            Err(_) => Parsed::Skip,
        };
    }
    if memmem::find(b, b"\"slotNotification\"").is_some() {
        return match serde_json::from_str::<SlotNotif>(t) {
            Ok(n) => Parsed::Slot(n.params.result.slot),
            Err(_) => Parsed::Skip,
        };
    }
    match serde_json::from_str::<Reply>(t) {
        Ok(Reply { id: Some(id), result, error }) => {
            Parsed::Reply { id, sub: result.and_then(|v| v.as_u64()), error: error.map(|e| e.to_string()) }
        }
        _ => Parsed::Skip,
    }
}

fn forward(p: Parsed, key: Option<Pubkey>, tx: &UnboundedSender<FeedMsg>) {
    let arr = now_ns();
    let m = match p {
        Parsed::Logs { slot, sig, err, logs, .. } => FeedMsg::Logs { slot, sig, err, logs, key, arr },
        Parsed::Account { slot, pubkey, data } => FeedMsg::Account { slot, pubkey, data, arr },
        Parsed::Slot(s) => FeedMsg::Slot(s),
        _ => return,
    };
    let _ = tx.send(m);
}

// ---------------------------------------------------------------- flussi statici

/// Flusso con sottoscrizioni fisse (programSubscribe, logsSubscribe su un indirizzo, slotSubscribe).
pub async fn static_stream(
    url: String,
    subs: Vec<String>,
    tx: UnboundedSender<FeedMsg>,
    stats: Arc<StreamStats>,
    ping_secs: u64,
    idle_secs: u64,
) {
    let mut backoff = 1u64;
    loop {
        match connect(&url).await {
            Ok(mut ws) => {
                let mut ok = true;
                for s in &subs {
                    if ws.send(Message::text(s.clone())).await.is_err() {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    stats.connected.store(true, Ordering::Relaxed);
                    info!("{} connesso", stats.name);
                    backoff = 1;
                    run_static(&mut ws, &tx, &stats, ping_secs, idle_secs).await;
                }
                stats.connected.store(false, Ordering::Relaxed);
                stats.reconnects.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => warn!("{} connessione fallita: {e}", stats.name),
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}

async fn run_static(ws: &mut Ws, tx: &UnboundedSender<FeedMsg>, stats: &StreamStats, ping_secs: u64, idle_secs: u64) {
    let mut ping = tokio::time::interval(Duration::from_secs(ping_secs.max(1)));
    ping.tick().await;
    let mut last = Instant::now();
    loop {
        tokio::select! {
            m = ws.next() => match m {
                Some(Ok(Message::Text(t))) => {
                    last = Instant::now();
                    stats.msgs.fetch_add(1, Ordering::Relaxed);
                    stats.bytes.fetch_add(t.len() as u64, Ordering::Relaxed);
                    match parse(t.as_str(), stats) {
                        Parsed::Reply { error: Some(e), .. } => warn!("{}: errore sottoscrizione {e}", stats.name),
                        Parsed::Reply { sub: Some(_), .. } => { stats.subs.fetch_add(1, Ordering::Relaxed); }
                        p => forward(p, None, tx),
                    }
                }
                Some(Ok(Message::Close(f))) => { warn!("{} chiuso dal server: {f:?}", stats.name); return; }
                Some(Ok(_)) => { last = Instant::now(); }
                Some(Err(e)) => { warn!("{} errore: {e}", stats.name); return; }
                None => return,
            },
            _ = ping.tick() => {
                if idle_secs > 0 && last.elapsed() > Duration::from_secs(idle_secs) {
                    warn!("{} muto da {}s: riconnessione", stats.name, last.elapsed().as_secs());
                    return;
                }
                if ws.send(Message::Ping(Vec::new().into())).await.is_err() { return; }
            }
        }
    }
}

// ---------------------------------------------------------------- sottoscrizioni dinamiche (Tier B)

pub enum SubCmd {
    Add(Pubkey),
    Remove(Pubkey),
}

fn logs_sub(id: u64, key: &Pubkey, commitment: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"logsSubscribe","params":[{{"mentions":["{}"]}},{{"commitment":"{commitment}"}}]}}"#,
        crate::b58::encode32(key)
    )
}

fn logs_unsub(id: u64, sub: u64) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"logsUnsubscribe","params":[{sub}]}}"#)
}

#[derive(Clone)]
pub struct DynOpts {
    pub ping_secs: u64,
    pub commitment: String,
    pub spacing_ms: u64,
}

pub async fn dyn_stream(url: String, mut cmd: UnboundedReceiver<SubCmd>, tx: UnboundedSender<FeedMsg>, stats: Arc<StreamStats>, o: DynOpts) {
    let mut desired: HashSet<Pubkey> = HashSet::new();
    let mut backoff = 1u64;
    let mut next_id: u64 = 1000;
    loop {
        // applica i comandi arrivati durante la disconnessione
        while let Ok(c) = cmd.try_recv() {
            match c {
                SubCmd::Add(k) => {
                    desired.insert(k);
                }
                SubCmd::Remove(k) => {
                    desired.remove(&k);
                }
            }
        }
        let mut ws = match connect(&url).await {
            Ok(ws) => ws,
            Err(e) => {
                warn!("{} connessione fallita: {e}", stats.name);
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(30);
                continue;
            }
        };
        backoff = 1;
        stats.connected.store(true, Ordering::Relaxed);
        let mut sub_of: HashMap<Pubkey, u64> = HashMap::new();
        let mut key_of: HashMap<u64, Pubkey> = HashMap::new();
        let mut pending: HashMap<u64, Pubkey> = HashMap::new();
        let mut queue: VecDeque<Pubkey> = desired.iter().copied().collect();
        let mut spacing = tokio::time::interval(Duration::from_millis(o.spacing_ms.max(1)));
        let mut ping = tokio::time::interval(Duration::from_secs(o.ping_secs.max(1)));
        ping.tick().await;
        let alive = 'conn: loop {
            tokio::select! {
                _ = spacing.tick(), if !queue.is_empty() => {
                    if let Some(k) = queue.pop_front() {
                        if desired.contains(&k) && !sub_of.contains_key(&k) && !pending.values().any(|p| p == &k) {
                            next_id += 1;
                            if ws.send(Message::text(logs_sub(next_id, &k, &o.commitment))).await.is_err() { break 'conn false; }
                            pending.insert(next_id, k);
                        }
                    }
                }
                c = cmd.recv() => match c {
                    Some(SubCmd::Add(k)) => { if desired.insert(k) { queue.push_back(k); } }
                    Some(SubCmd::Remove(k)) => {
                        desired.remove(&k);
                        if let Some(sid) = sub_of.remove(&k) {
                            key_of.remove(&sid);
                            next_id += 1;
                            if ws.send(Message::text(logs_unsub(next_id, sid))).await.is_err() { break 'conn false; }
                        }
                    }
                    None => break 'conn true,
                },
                m = ws.next() => match m {
                    Some(Ok(Message::Text(t))) => {
                        stats.msgs.fetch_add(1, Ordering::Relaxed);
                        stats.bytes.fetch_add(t.len() as u64, Ordering::Relaxed);
                        match parse(t.as_str(), &stats) {
                            Parsed::Reply { id, sub, error } => {
                                if let Some(k) = pending.remove(&id) {
                                    match (sub, error) {
                                        (Some(sid), _) => {
                                            if desired.contains(&k) {
                                                sub_of.insert(k, sid);
                                                key_of.insert(sid, k);
                                            } else {
                                                next_id += 1;
                                                if ws.send(Message::text(logs_unsub(next_id, sid))).await.is_err() { break 'conn false; }
                                            }
                                        }
                                        (None, e) => {
                                            stats.sub_errors.fetch_add(1, Ordering::Relaxed);
                                            debug!("{}: sottoscrizione rifiutata {e:?}", stats.name);
                                            queue.push_back(k);
                                        }
                                    }
                                }
                            }
                            p @ Parsed::Logs { .. } => {
                                let sub = if let Parsed::Logs { sub, .. } = &p { *sub } else { 0 };
                                let key = key_of.get(&sub).copied();
                                forward(p, key, &tx);
                            }
                            _ => {}
                        }
                        stats.subs.store(sub_of.len() as u64, Ordering::Relaxed);
                    }
                    Some(Ok(Message::Close(f))) => { warn!("{} chiuso dal server: {f:?}", stats.name); break 'conn false; }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => { warn!("{} errore: {e}", stats.name); break 'conn false; }
                    None => break 'conn false,
                },
                _ = ping.tick() => {
                    if ws.send(Message::Ping(Vec::new().into())).await.is_err() { break 'conn false; }
                }
            }
        };
        stats.connected.store(false, Ordering::Relaxed);
        stats.subs.store(0, Ordering::Relaxed);
        if alive {
            // il gestore è stato chiuso: fine del task
            let _ = ws.close(None).await;
            return;
        }
        stats.reconnects.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}

struct ConnH {
    cmd: UnboundedSender<SubCmd>,
    count: usize,
}

/// Distribuisce le curve osservate su connessioni con al massimo `per_conn` sottoscrizioni, per ogni hostname.
pub struct SubManager {
    urls: Vec<String>,
    per_conn: usize,
    redundant: bool,
    conns: Vec<Vec<ConnH>>,
    assign: HashMap<Pubkey, Vec<(usize, usize)>>,
    tx: UnboundedSender<FeedMsg>,
    opts: DynOpts,
    pub stats: Vec<Arc<StreamStats>>,
}

impl SubManager {
    pub fn new(urls: Vec<String>, per_conn: usize, redundant: bool, tx: UnboundedSender<FeedMsg>, opts: DynOpts) -> Self {
        let n = urls.len();
        SubManager { urls, per_conn: per_conn.max(1), redundant, conns: (0..n).map(|_| Vec::new()).collect(), assign: HashMap::new(), tx, opts, stats: Vec::new() }
    }

    pub fn add(&mut self, k: Pubkey) {
        if self.assign.contains_key(&k) {
            return;
        }
        let n_urls = if self.redundant { self.urls.len() } else { 1.min(self.urls.len()) };
        let mut v = Vec::new();
        for u in 0..n_urls {
            let ci = match self.conns[u].iter().position(|c| c.count < self.per_conn) {
                Some(i) => i,
                None => {
                    let (ctx, crx) = unbounded_channel();
                    let name = format!("tierB-{}-{}", u, self.conns[u].len());
                    let st = StreamStats::new(&name);
                    self.stats.push(st.clone());
                    tokio::spawn(dyn_stream(self.urls[u].clone(), crx, self.tx.clone(), st, self.opts.clone()));
                    self.conns[u].push(ConnH { cmd: ctx, count: 0 });
                    self.conns[u].len() - 1
                }
            };
            let c = &mut self.conns[u][ci];
            let _ = c.cmd.send(SubCmd::Add(k));
            c.count += 1;
            v.push((u, ci));
        }
        self.assign.insert(k, v);
    }

    pub fn remove(&mut self, k: &Pubkey) {
        if let Some(v) = self.assign.remove(k) {
            for (u, ci) in v {
                let c = &mut self.conns[u][ci];
                let _ = c.cmd.send(SubCmd::Remove(*k));
                c.count = c.count.saturating_sub(1);
            }
        }
    }

    pub fn n_assigned(&self) -> usize {
        self.assign.len()
    }

    pub fn n_active_subs(&self) -> u64 {
        self.stats.iter().map(|s| s.subs.load(Ordering::Relaxed)).sum()
    }
}
