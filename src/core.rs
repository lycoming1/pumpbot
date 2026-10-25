//! Nucleo: messaggi → eventi, deduplica, stato, motore, registrazione. Uguale dal vivo e in replay.

use std::sync::mpsc::SyncSender;

use base64::Engine as _;

use crate::b58;
use crate::decode;
use crate::engine::Engine;
use crate::feed::FeedMsg;
use crate::state::{Map, MarketState, Set};
use crate::store::Rec;
use crate::types::*;

/// Deduplica delle firme con due generazioni (ruotate ogni `gen_size` inserimenti).
pub struct SigDedup {
    cur: Set<u64>,
    old: Set<u64>,
    gen_size: usize,
}

impl SigDedup {
    pub fn new(gen_size: usize) -> Self {
        SigDedup { cur: Set::default(), old: Set::default(), gen_size }
    }
    /// true se la firma è nuova
    pub fn insert(&mut self, k: u64) -> bool {
        if self.cur.contains(&k) || self.old.contains(&k) {
            return false;
        }
        if self.cur.len() >= self.gen_size {
            self.old = std::mem::take(&mut self.cur);
        }
        self.cur.insert(k);
        true
    }
}

#[derive(Default, Debug, Clone)]
pub struct CoreStats {
    pub accounts: u64,
    pub accounts_dup: u64,
    pub accounts_bad: u64,
    pub logs: u64,
    pub logs_dup: u64,
    pub logs_err: u64,
    pub txs: u64,
    pub trades: u64,
    pub creates: u64,
}

pub struct Core {
    pub st: MarketState,
    pub eng: Engine,
    rec: Option<SyncSender<Rec>>,
    sigs: SigDedup,
    last_acct_slot: Map<Pubkey, u64>,
    curve_info_sent: Set<Pubkey>,
    pub stats: CoreStats,
    wsol: Pubkey,
    last_watch_clock: u64,
    last_evict_clock: u64,
    pub last_feed_ns: u64,
}

impl Core {
    pub fn new(st: MarketState, eng: Engine, rec: Option<SyncSender<Rec>>) -> Self {
        Core {
            st,
            eng,
            rec,
            sigs: SigDedup::new(200_000),
            last_acct_slot: Map::default(),
            curve_info_sent: Set::default(),
            stats: CoreStats::default(),
            wsol: b58::decode32(WSOL).unwrap(),
            last_watch_clock: 0,
            last_evict_clock: 0,
            last_feed_ns: 0,
        }
    }

    fn record(&self, r: Rec) {
        if let Some(tx) = &self.rec {
            let _ = tx.try_send(r);
        }
    }

    /// Converte un messaggio del feed in evento (decodifica account e log).
    pub fn feed_to_event(&mut self, m: FeedMsg) -> Option<Event> {
        match m {
            FeedMsg::Slot(s) => Some(Event::Slot(s)),
            FeedMsg::Account { slot, pubkey, data, arr } => {
                self.last_feed_ns = arr;
                self.stats.accounts += 1;
                let curve = b58::decode32(&pubkey)?;
                let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).ok()?;
                let Some((state, stat)) = decode::decode_bonding_curve(&bytes, &self.wsol) else {
                    self.stats.accounts_bad += 1;
                    return None;
                };
                Some(Event::Account { slot, curve, state, stat, arrival_ns: arr })
            }
            FeedMsg::Logs { slot, sig, err, logs, key, arr } => {
                self.stats.logs += 1;
                if err {
                    self.stats.logs_err += 1;
                    return None;
                }
                let sigb = b58::decode64(&sig)?;
                if !self.sigs.insert(b58::sig_key(&sigb)) {
                    self.stats.logs_dup += 1;
                    // la tx è già stata elaborata, ma può insegnare mint→curva per questa sottoscrizione
                    if let Some(k) = key {
                        if self.st.curves.get(&k).map(|c| c.mint.is_none()).unwrap_or(false) {
                            let tx = decode::decode_tx_logs(&logs, slot, sigb, arr, &self.wsol);
                            if let Some(t) = tx.trades.first() {
                                if tx.trades.iter().all(|x| x.mint == t.mint) && !self.st.mint_to_curve.contains_key(&t.mint) {
                                    self.st.mint_to_curve.insert(t.mint, k);
                                }
                            }
                        }
                    }
                    return None;
                }
                let tx = decode::decode_tx_logs(&logs, slot, sigb, arr, &self.wsol);
                Some(Event::Tx { tx, sub_curve: key })
            }
        }
    }

    pub fn handle(&mut self, ev: Event) {
        match ev {
            Event::Slot(s) => self.st.observe_slot(s),
            Event::Account { slot, curve, state, stat, arrival_ns } => {
                if self.last_acct_slot.get(&curve).map(|s| slot <= *s).unwrap_or(false) {
                    self.stats.accounts_dup += 1;
                    self.st.observe_slot(slot);
                    return;
                }
                self.last_acct_slot.insert(curve, slot);
                if self.st.on_account(slot, curve, state, stat) {
                    if self.curve_info_sent.insert(curve) {
                        self.record(Rec::CurveInfo { curve, stat });
                    }
                    self.record(Rec::State { slot, curve, s: state, arr: arrival_ns });
                }
            }
            Event::Tx { tx, sub_curve } => {
                self.stats.txs += 1;
                self.stats.trades += tx.trades.len() as u64;
                self.stats.creates += tx.creates.len() as u64;
                self.st.on_tx(&tx, sub_curve);
                if self.rec.is_some() {
                    for t in &tx.trades {
                        if t.quote_sol && !t.mayhem {
                            self.record(Rec::Trade { t: t.clone(), sub_curve, arr: tx.arrival_ns });
                        }
                    }
                    for c in &tx.creates {
                        self.record(Rec::Create { slot: tx.slot, sig: tx.sig, c: c.clone(), arr: tx.arrival_ns });
                    }
                    for c in &tx.completes {
                        self.record(Rec::Complete { slot: tx.slot, sig: tx.sig, c: c.clone(), arr: tx.arrival_ns });
                    }
                }
            }
        }
    }

    /// Finalizza gli slot maturi e fa lavorare il motore. Ritorna le decisioni di osservazione se dovute.
    pub fn tick(&mut self) -> Option<(Vec<Pubkey>, Vec<Pubkey>)> {
        let fin = self.st.finalize();
        for (c, s) in fin {
            self.eng.on_slot(&self.st, &c, s);
        }
        let h = self.st.horizon();
        self.eng.advance_all(&self.st, h);
        let clock = self.st.clock;
        let mut out = None;
        if clock >= self.last_watch_clock + 10 {
            self.last_watch_clock = clock;
            let must = self.eng.interest();
            out = Some(self.st.watch_decisions(&must));
        }
        if clock >= self.last_evict_clock + 300 {
            self.last_evict_clock = clock;
            let keep = self.eng.interest();
            self.st.evict(&keep, 6000);
            if self.last_acct_slot.len() > 20_000 {
                let live: Set<Pubkey> = self.st.curves.keys().copied().collect();
                self.last_acct_slot.retain(|k, _| live.contains(k));
                self.curve_info_sent.retain(|k| live.contains(k));
            }
        }
        out
    }
}
