//! MarketState: stato per curva, finalizzazione per tempo, catena degli stati, feature (DESIGN §0, §4).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet, VecDeque};

use foldhash::fast::RandomState;

use crate::config::FeedCfg;
use crate::types::*;

pub type Map<K, V> = HashMap<K, V, RandomState>;
pub type Set<K> = HashSet<K, RandomState>;

const HIST_CAP: usize = 256;
const TRADES_CAP: usize = 300;

#[derive(Default)]
struct SlotBuf {
    trades: Vec<TradeRec>,
    a_state: Option<CurveState>,
}

#[derive(Clone, Debug)]
pub struct CreateInfo {
    pub slot: u64,
    pub user: Pubkey,
    pub creator: Pubkey,
    pub dev_quote: u64,
    pub dev_tokens: u64,
    pub other_wallets_in_tx: u32,
    pub post_create: CurveState,
    pub creator_prev_creates: u32,
}

pub struct CurveData {
    pub key: Pubkey,
    pub mint: Option<Pubkey>,
    pub stat: CurveStatic,
    pending: BTreeMap<u64, SlotBuf>,
    pub finalized_through: u64,
    /// stato a fine dell'ultimo slot finalizzato
    pub end: Option<CurveState>,
    /// stati di fine slot finalizzati, solo negli slot con cambi
    pub hist: VecDeque<(u64, CurveState)>,
    /// trade finalizzati, in ordine di catena
    pub trades: VecDeque<TradeRec>,
    /// ultimo stato noto (anche non finalizzato): per decidere cosa osservare
    pub last_state: Option<CurveState>,
    pub last_state_slot: u64,
    pub last_change_slot: u64,
    pub chain_started: bool,
    pub dirty_until: u64,
    pub watched: bool,
    pub watch_since: u64,
    pub low_since: Option<u64>,
    pub complete_slot: Option<u64>,
    pub fee_bps: Option<(u64, u64)>,
    pub create: Option<CreateInfo>,
}

impl CurveData {
    fn new(key: Pubkey, stat: CurveStatic) -> Self {
        CurveData {
            key,
            mint: None,
            stat,
            pending: BTreeMap::new(),
            finalized_through: 0,
            end: None,
            hist: VecDeque::new(),
            trades: VecDeque::new(),
            last_state: None,
            last_state_slot: 0,
            last_change_slot: 0,
            chain_started: false,
            dirty_until: 0,
            watched: false,
            watch_since: 0,
            low_since: None,
            complete_slot: None,
            fee_bps: None,
            create: None,
        }
    }

    /// Stato a fine slot `slot` (ultimo noto ≤ slot) tra quelli finalizzati.
    pub fn state_at(&self, slot: u64) -> Option<CurveState> {
        let i = self.hist.partition_point(|(s, _)| *s <= slot);
        if i == 0 {
            return None;
        }
        Some(self.hist[i - 1].1)
    }

    /// Primo slot di cui si conosce lo stato.
    pub fn first_known_slot(&self) -> Option<u64> {
        self.hist.front().map(|(s, _)| *s)
    }

    /// Slot con cambi di stato in (a, b].
    pub fn change_slots(&self, a: u64, b: u64) -> Vec<u64> {
        let i = self.hist.partition_point(|(s, _)| *s <= a);
        self.hist.iter().skip(i).take_while(|(s, _)| *s <= b).map(|(s, _)| *s).collect()
    }

    pub fn trades_in_slot(&self, slot: u64) -> Vec<&TradeRec> {
        let mut v: Vec<&TradeRec> = self.trades.iter().rev().take_while(|t| t.slot >= slot).filter(|t| t.slot == slot).collect();
        v.reverse();
        v
    }

    pub fn is_dirty(&self, slot: u64) -> bool {
        slot <= self.dirty_until
    }

    pub fn fees(&self, fallback: (u64, u64)) -> (u64, u64) {
        self.fee_bps.unwrap_or(fallback)
    }
}

#[derive(Default, Debug, Clone)]
pub struct StateStats {
    pub late_a: u64,
    pub late_trades: u64,
    pub chain_breaks: u64,
    /// buchi che superano dirty_min_gap_sol e rendono il mint dirty
    pub material_gaps: u64,
    pub a_mismatch: u64,
    pub missed_while_watched: u64,
    pub finalized_slots: u64,
    pub unmapped_trades: u64,
    pub non_universe_trades: u64,
    pub truncated: u64,
    pub creates: u64,
    pub completes: u64,
    /// ritardo all'arrivo (orologio - slot) dei trade e degli stati: ≤5, ≤20, ≤75, ≤200, >200 slot
    pub lag_trades: [u64; 5],
    pub lag_states: [u64; 5],
}

pub fn lag_bucket(lag: u64) -> usize {
    match lag {
        0..=5 => 0,
        6..=20 => 1,
        21..=75 => 2,
        76..=200 => 3,
        _ => 4,
    }
}

pub struct MarketState {
    pub curves: Map<Pubkey, CurveData>,
    pub mint_to_curve: Map<Pubkey, Pubkey>,
    pub clock: u64,
    finalize_q: BinaryHeap<Reverse<(u64, Pubkey)>>,
    pub creator_creates: Map<Pubkey, u32>,
    pub cfg: FeedCfg,
    pub stats: StateStats,
    pub agent: Pubkey,
}

/// Uno slot finalizzato con attività, da passare al motore.
pub type Finalized = (Pubkey, u64);

impl MarketState {
    pub fn new(cfg: FeedCfg) -> Self {
        MarketState {
            curves: Map::default(),
            mint_to_curve: Map::default(),
            clock: 0,
            finalize_q: BinaryHeap::new(),
            creator_creates: Map::default(),
            cfg,
            stats: StateStats::default(),
            agent: crate::b58::decode32(MAYHEM_AGENT).unwrap(),
        }
    }

    /// Slot fino al quale tutto è finalizzato.
    pub fn horizon(&self) -> u64 {
        self.clock.saturating_sub(self.cfg.finalize_lag_slots)
    }

    pub fn observe_slot(&mut self, slot: u64) {
        if slot > self.clock {
            self.clock = slot;
        }
    }

    pub fn curve_for_mint(&self, mint: &Pubkey) -> Option<&CurveData> {
        self.mint_to_curve.get(mint).and_then(|c| self.curves.get(c))
    }

    pub fn on_account(&mut self, slot: u64, curve: Pubkey, state: CurveState, stat: CurveStatic) -> bool {
        self.stats.lag_states[lag_bucket(self.clock.saturating_sub(slot))] += 1;
        self.observe_slot(slot);
        if !stat.in_universe() || !state.is_standard() {
            return false;
        }
        let cd = self.curves.entry(curve).or_insert_with(|| CurveData::new(curve, stat));
        if slot <= cd.finalized_through {
            self.stats.late_a += 1;
            return true;
        }
        cd.pending.entry(slot).or_default().a_state = Some(state);
        if slot >= cd.last_state_slot {
            cd.last_state = Some(state);
            cd.last_state_slot = slot;
        }
        self.finalize_q.push(Reverse((slot, curve)));
        true
    }

    /// Una tx decodificata (già deduplicata dal chiamante).
    pub fn on_tx(&mut self, tx: &TxEvents, sub_curve: Option<Pubkey>) {
        if !tx.trades.is_empty() {
            self.stats.lag_trades[lag_bucket(self.clock.saturating_sub(tx.slot))] += 1;
        }
        self.observe_slot(tx.slot);
        if tx.truncated {
            self.stats.truncated += 1;
        }
        for c in &tx.creates {
            self.stats.creates += 1;
            self.mint_to_curve.insert(c.mint, c.curve);
            if !(c.quote_sol && !c.mayhem && !c.cashback) {
                continue;
            }
            let prev = {
                let e = self.creator_creates.entry(c.creator).or_insert(0);
                let p = *e;
                *e += 1;
                p
            };
            let mut dev_quote = 0;
            let mut dev_tokens = 0;
            let mut others: Set<Pubkey> = Set::default();
            let mut post = CurveState { vq: VS0, vt: VT0, rq: 0, rt: RT0, complete: false };
            for t in tx.trades.iter().filter(|t| t.mint == c.mint) {
                if t.user == c.user || t.user == c.creator {
                    if t.is_buy {
                        dev_quote += t.quote;
                        dev_tokens += t.tokens;
                    }
                } else {
                    others.insert(t.user);
                }
                post = t.post;
            }
            let stat = CurveStatic { creator: c.creator, mayhem: c.mayhem, cashback: c.cashback, quote_sol: c.quote_sol, holder_reward: c.holder_reward };
            let cd = self.curves.entry(c.curve).or_insert_with(|| CurveData::new(c.curve, stat));
            cd.mint = Some(c.mint);
            cd.create = Some(CreateInfo {
                slot: tx.slot,
                user: c.user,
                creator: c.creator,
                dev_quote,
                dev_tokens,
                other_wallets_in_tx: others.len() as u32,
                post_create: post,
                creator_prev_creates: prev,
            });
        }
        for c in &tx.completes {
            self.stats.completes += 1;
            self.mint_to_curve.insert(c.mint, c.curve);
        }
        // mint→curva dalla sottoscrizione Tier B se la tx tocca un solo mint
        if let Some(sc) = sub_curve {
            let first = tx.trades.first().map(|t| t.mint);
            if let Some(m) = first {
                if tx.trades.iter().all(|t| t.mint == m) && !self.mint_to_curve.contains_key(&m) {
                    self.mint_to_curve.insert(m, sc);
                }
            }
        }
        for t in &tx.trades {
            let Some(curve) = self.mint_to_curve.get(&t.mint).copied() else {
                self.stats.unmapped_trades += 1;
                continue;
            };
            if !t.quote_sol || t.mayhem {
                self.stats.non_universe_trades += 1;
                continue;
            }
            let Some(cd) = self.curves.get_mut(&curve) else {
                self.stats.non_universe_trades += 1;
                continue;
            };
            if cd.mint.is_none() {
                cd.mint = Some(t.mint);
            }
            if t.slot <= cd.finalized_through {
                self.stats.late_trades += 1;
                if t.quote >= lamports(self.cfg.dirty_min_gap_sol) {
                    self.stats.material_gaps += 1;
                    cd.dirty_until = cd.dirty_until.max(t.slot + self.cfg.dirty_cooldown_slots);
                }
                continue;
            }
            cd.fee_bps = Some((t.fee_bps, t.cfee_bps));
            cd.pending.entry(t.slot).or_default().trades.push(t.clone());
            if t.slot >= cd.last_state_slot {
                cd.last_state = Some(t.post);
                cd.last_state_slot = t.slot;
            }
            self.finalize_q.push(Reverse((t.slot, curve)));
        }
    }

    /// Finalizza tutti gli slot ≤ orizzonte. Ritorna gli slot con attività, in ordine di slot.
    pub fn finalize(&mut self) -> Vec<Finalized> {
        let h = self.horizon();
        let mut out = Vec::new();
        while let Some(Reverse((s, c))) = self.finalize_q.peek().copied() {
            if s > h {
                break;
            }
            self.finalize_q.pop();
            let lag_cooldown = self.cfg.dirty_cooldown_slots;
            let min_gap = lamports(self.cfg.dirty_min_gap_sol);
            let Some(cd) = self.curves.get_mut(&c) else { continue };
            while let Some((&slot, _)) = cd.pending.iter().next() {
                if slot > s {
                    break;
                }
                let buf = cd.pending.remove(&slot).unwrap();
                if finalize_slot(cd, slot, buf, lag_cooldown, min_gap, &mut self.stats) {
                    out.push((c, slot));
                }
            }
        }
        out
    }

    /// Curve da osservare con il Tier B. Ritorna (da aggiungere, da togliere).
    pub fn watch_decisions(&mut self, must: &Set<Pubkey>) -> (Vec<Pubkey>, Vec<Pubkey>) {
        let clock = self.clock;
        let cfg = self.cfg.clone();
        let watch_rq = lamports(cfg.watch_rq_sol);
        let unwatch_rq = lamports(cfg.unwatch_rq_sol);
        let mut candidates: Vec<(bool, u64, Pubkey)> = Vec::new();
        for (k, cd) in self.curves.iter_mut() {
            let Some(s) = cd.last_state else { continue };
            let forced = must.contains(k);
            let idle = clock.saturating_sub(cd.last_state_slot) > cfg.watch_idle_slots;
            let completed_long = cd.complete_slot.map(|c| clock.saturating_sub(c) > cfg.complete_unwatch_slots).unwrap_or(false)
                || (s.complete && clock.saturating_sub(cd.last_state_slot) > cfg.complete_unwatch_slots);
            if s.rq < unwatch_rq {
                cd.low_since.get_or_insert(clock);
            } else {
                cd.low_since = None;
            }
            let low_long = cd.low_since.map(|l| clock.saturating_sub(l) > cfg.unwatch_low_slots).unwrap_or(false);
            let keep = if cd.watched { !(idle || completed_long || low_long) } else { s.rq >= watch_rq && !s.complete && !idle };
            if forced || keep {
                candidates.push((forced, s.rq, *k));
            }
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        let want: Set<Pubkey> = candidates.iter().take(cfg.max_watched.max(must.len())).map(|c| c.2).collect();
        let mut add = Vec::new();
        let mut remove = Vec::new();
        for (k, cd) in self.curves.iter_mut() {
            let w = want.contains(k);
            if w && !cd.watched {
                cd.watched = true;
                cd.watch_since = clock;
                cd.chain_started = false;
                add.push(*k);
            } else if !w && cd.watched {
                cd.watched = false;
                remove.push(*k);
            }
        }
        (add, remove)
    }

    /// Libera le curve ferme da tempo, non osservate e senza interesse del motore.
    pub fn evict(&mut self, keep: &Set<Pubkey>, idle_slots: u64) -> usize {
        let clock = self.clock;
        let before = self.curves.len();
        let mut gone: Vec<(Pubkey, Option<Pubkey>)> = Vec::new();
        self.curves.retain(|k, cd| {
            let stale = clock.saturating_sub(cd.last_state_slot.max(cd.finalized_through)) > idle_slots;
            let drop = stale && !cd.watched && !keep.contains(k) && cd.pending.is_empty();
            if drop {
                gone.push((*k, cd.mint));
            }
            !drop
        });
        for (_, m) in gone {
            if let Some(m) = m {
                self.mint_to_curve.remove(&m);
            }
        }
        before - self.curves.len()
    }

    pub fn n_watched(&self) -> usize {
        self.curves.values().filter(|c| c.watched).count()
    }

    pub fn n_dirty(&self) -> usize {
        let h = self.horizon();
        self.curves.values().filter(|c| c.watched && c.dirty_until >= h).count()
    }
}

/// Esito dell'ordinamento per catena dei trade di uno slot.
pub struct Chain {
    pub ordered: Vec<TradeRec>,
    /// lamport non spiegati tra lo stato precedente e la testa della catena
    pub gap: u64,
    /// tutti i trade concatenati in un'unica catena
    pub chained: bool,
    pub end: Option<CurveState>,
}

/// Ordina i trade dello slot concatenando pre/post stato, partendo dallo stato di fine slot precedente.
pub fn order_chain(prev: Option<CurveState>, started: bool, mut rest: Vec<TradeRec>) -> Chain {
    if rest.is_empty() {
        return Chain { ordered: rest, gap: 0, chained: true, end: None };
    }
    let prev_key = if started { prev.map(|p| (p.vq, p.vt)) } else { None };
    let mut gap = 0u64;
    let mut cur: Option<(u64, u64)> = prev_key.filter(|k| rest.iter().any(|t| t.pre() == Some(*k)));
    if cur.is_none() {
        // testa = trade il cui pre-stato non è il post-stato di nessun altro
        let posts: Vec<(u64, u64)> = rest.iter().map(|t| (t.post.vq, t.post.vt)).collect();
        let head = rest.iter().position(|t| t.pre().map(|p| !posts.contains(&p)).unwrap_or(true)).unwrap_or(0);
        cur = rest[head].pre();
        if let (Some(p), Some(c)) = (prev_key, cur) {
            gap = p.0.abs_diff(c.0);
        }
    }
    let mut ordered = Vec::with_capacity(rest.len());
    while let Some(c) = cur {
        let Some(i) = rest.iter().position(|t| t.pre() == Some(c)) else { break };
        let t = rest.remove(i);
        cur = Some((t.post.vq, t.post.vt));
        ordered.push(t);
    }
    let chained = rest.is_empty();
    let end = if chained {
        ordered.last().map(|t| t.post)
    } else {
        // best effort: in coda i trade non concatenati, nell'ordine di arrivo
        let e = rest.last().map(|t| t.post);
        ordered.extend(rest);
        e
    };
    Chain { ordered, gap, chained, end }
}

fn finalize_slot(cd: &mut CurveData, slot: u64, buf: SlotBuf, cooldown: u64, min_gap: u64, stats: &mut StateStats) -> bool {
    stats.finalized_slots += 1;
    let prev = cd.end;
    let had_trades = !buf.trades.is_empty();
    let ch = order_chain(prev, cd.chain_started, buf.trades);
    let mut dirty = false;
    if had_trades {
        if ch.gap > 0 || !ch.chained {
            stats.chain_breaks += 1;
            if ch.gap >= min_gap || !ch.chained {
                stats.material_gaps += 1;
                dirty = true;
            }
        }
        if let (Some(a), Some(e)) = (buf.a_state, ch.end) {
            if !a.same_reserves(&e) {
                stats.a_mismatch += 1;
                if a.vq.abs_diff(e.vq) >= min_gap {
                    stats.material_gaps += 1;
                    dirty = true;
                }
            }
        }
        cd.chain_started = true;
    } else if cd.watched && cd.chain_started && slot > cd.watch_since {
        // cambio di stato senza trade ricevuti mentre la curva è osservata: trade persi
        if let (Some(a), Some(p)) = (buf.a_state, prev) {
            if !a.same_reserves(&p) {
                stats.missed_while_watched += 1;
                if a.vq.abs_diff(p.vq) >= min_gap {
                    stats.material_gaps += 1;
                    dirty = true;
                }
            }
        }
    }
    if dirty {
        cd.dirty_until = cd.dirty_until.max(slot + cooldown);
    }
    let end = match (buf.a_state, ch.end) {
        (Some(a), _) => a,
        (None, Some(e)) => e,
        (None, None) => match prev {
            Some(p) => p,
            None => return false,
        },
    };
    for t in ch.ordered {
        if cd.trades.len() >= TRADES_CAP {
            cd.trades.pop_front();
        }
        cd.trades.push_back(t);
    }
    let changed = prev.map(|p| !p.same_reserves(&end) || p.complete != end.complete).unwrap_or(true);
    if changed {
        if cd.hist.len() >= HIST_CAP {
            cd.hist.pop_front();
        }
        cd.hist.push_back((slot, end));
        cd.last_change_slot = slot;
    }
    if end.complete && cd.complete_slot.is_none() {
        cd.complete_slot = Some(slot);
    }
    cd.end = Some(end);
    cd.finalized_through = slot;
    changed || had_trades
}

// ---------------------------------------------------------------- feature

/// Finestra di flusso aggregata per attore (tx multi-wallet = un attore).
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Win {
    pub n: usize,
    pub n_buy: usize,
    pub n_sell: usize,
    pub buy_sol: f64,
    pub sell_sol: f64,
    pub net_sol: f64,
    pub uniq_buyers: usize,
    pub top3_share: f64,
    pub ofi: f64,
    pub liq_vel: f64,
    pub max_buy_share: f64,
    pub full: bool,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Features {
    pub slot: u64,
    pub progress: f64,
    pub rq_sol: f64,
    pub price: f64,
    pub dirty: bool,
    pub watched_slots: u64,
    pub dprog50: Option<f64>,
    pub d2prog: Option<f64>,
    pub t20: Win,
    pub t30: Win,
    pub s50: Win,
    pub accel50: bool,
    pub s0_in_sol: Option<f64>,
    pub dev_buy_pct: Option<f64>,
    pub other_wallets_in_create_tx: Option<u32>,
    pub creator_prev_creates: Option<u32>,
    pub creator_net_pct: f64,
    pub creator_sold: bool,
    pub micro_buy_pct: f64,
    pub flip_pct: f64,
    pub multi_sell_txs: u32,
    pub wash_atomic: u32,
}

struct Actor {
    slot: u64,
    user: Pubkey,
    net: i128,
    sellers: u32,
    buy_users: u32,
    wash: bool,
    pre_vq: u64,
    post_vq: u64,
}

/// Raggruppa i trade (in ordine di catena) per firma, escludendo agente mayhem e creatore.
fn actors(trades: &[&TradeRec], agent: &Pubkey, creator: &Pubkey) -> Vec<Actor> {
    let mut out: Vec<Actor> = Vec::new();
    let mut last_sig: Option<&Sig> = None;
    for t in trades {
        if &t.user == agent || &t.user == creator {
            continue;
        }
        let signed = if t.is_buy { t.quote as i128 } else { -(t.quote as i128) };
        let pre_vq = t.pre().map(|p| p.0).unwrap_or(t.post.vq);
        if last_sig == Some(&t.sig) {
            let a = out.last_mut().unwrap();
            if a.user != t.user && !t.is_buy {
                a.sellers += 1;
            }
            if a.user == t.user && (t.is_buy != (a.net > 0)) {
                a.wash = true;
            }
            if t.is_buy {
                a.buy_users += 1;
            }
            a.net += signed;
            a.post_vq = t.post.vq;
        } else {
            out.push(Actor {
                slot: t.slot,
                user: t.user,
                net: signed,
                sellers: if t.is_buy { 0 } else { 1 },
                buy_users: if t.is_buy { 1 } else { 0 },
                wash: false,
                pre_vq,
                post_vq: t.post.vq,
            });
            last_sig = Some(&t.sig);
        }
    }
    out
}

fn window(acts: &[Actor], full: bool) -> Win {
    let mut w = Win { n: acts.len(), full, ..Default::default() };
    if acts.is_empty() {
        return w;
    }
    let mut per_buyer: Map<Pubkey, f64> = Map::default();
    let mut max_buy = 0.0f64;
    for a in acts {
        let v = a.net as f64 / LAMPORTS_PER_SOL;
        if a.net > 0 {
            w.n_buy += 1;
            w.buy_sol += v;
            max_buy = max_buy.max(v);
            if v >= 0.01 {
                *per_buyer.entry(a.user).or_insert(0.0) += v;
            }
        } else if a.net < 0 {
            w.n_sell += 1;
            w.sell_sol += -v;
        }
    }
    w.net_sol = w.buy_sol - w.sell_sol;
    w.uniq_buyers = per_buyer.len();
    let mut vols: Vec<f64> = per_buyer.values().copied().collect();
    vols.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let top3: f64 = vols.iter().take(3).sum();
    w.top3_share = if w.buy_sol > 0.0 { top3 / w.buy_sol } else { 0.0 };
    let tot = w.buy_sol + w.sell_sol;
    w.ofi = if tot > 0.0 { (w.buy_sol - w.sell_sol) / tot } else { 0.0 };
    let dvq = acts.last().unwrap().post_vq as f64 - acts.first().unwrap().pre_vq as f64;
    w.liq_vel = dvq / LAMPORTS_PER_SOL / acts.len() as f64;
    w.max_buy_share = if w.buy_sol > 0.0 { max_buy / w.buy_sol } else { 0.0 };
    w
}

pub fn features(cd: &CurveData, slot: u64, agent: &Pubkey) -> Option<Features> {
    let s = cd.state_at(slot)?;
    let creator = cd.stat.creator;
    let mut f = Features {
        slot,
        progress: s.progress(),
        rq_sol: s.rq_sol(),
        price: s.price(),
        dirty: cd.is_dirty(slot),
        watched_slots: if cd.watched { slot.saturating_sub(cd.watch_since) } else { 0 },
        ..Default::default()
    };
    // progresso a S-25 e S-50 (solo se lo stato era già noto allora)
    if let Some(first) = cd.first_known_slot() {
        if first <= slot.saturating_sub(50) {
            let p50 = cd.state_at(slot - 50).map(|x| x.progress());
            let p25 = cd.state_at(slot - 25).map(|x| x.progress());
            if let (Some(p50), Some(p25)) = (p50, p25) {
                f.dprog50 = Some(f.progress - p50);
                f.d2prog = Some((f.progress - p25) - (p25 - p50));
            }
        }
    }
    let trades: Vec<&TradeRec> = cd.trades.iter().filter(|t| t.slot <= slot).collect();
    let acts = actors(&trades, agent, &creator);
    let watched_ok = cd.watched && cd.chain_started;
    for (n, dst) in [(20usize, &mut f.t20), (30usize, &mut f.t30)] {
        let k = acts.len().saturating_sub(n);
        *dst = window(&acts[k..], watched_ok && acts.len() >= n);
    }
    let lo = slot.saturating_sub(50);
    let mid = slot.saturating_sub(25);
    let k50 = acts.partition_point(|a| a.slot <= lo);
    let kmid = acts.partition_point(|a| a.slot <= mid);
    let full50 = watched_ok && cd.watch_since <= lo;
    f.s50 = window(&acts[k50..], full50);
    let first_half: i128 = acts[k50..kmid].iter().map(|a| a.net).sum();
    let second_half: i128 = acts[kmid..].iter().map(|a| a.net).sum();
    f.accel50 = second_half > first_half;
    // manipolazione
    let recent = &acts[acts.len().saturating_sub(50)..];
    let buys: Vec<&Actor> = recent.iter().filter(|a| a.net > 0).collect();
    f.micro_buy_pct = if buys.is_empty() { 0.0 } else { buys.iter().filter(|a| a.net < 10_000_000).count() as f64 / buys.len() as f64 };
    let sells: Vec<&Actor> = recent.iter().filter(|a| a.net < 0).collect();
    let flips = sells
        .iter()
        .filter(|s| recent.iter().any(|b| b.net > 0 && b.user == s.user && b.slot <= s.slot && s.slot - b.slot <= 10))
        .count();
    f.flip_pct = if sells.is_empty() { 0.0 } else { flips as f64 / sells.len() as f64 };
    f.multi_sell_txs = recent.iter().filter(|a| a.sellers >= 2).count() as u32;
    f.wash_atomic = recent.iter().filter(|a| a.wash).count() as u32;
    let mut creator_net: i128 = 0;
    for t in cd.trades.iter().filter(|t| t.slot <= slot && t.user == creator) {
        if t.is_buy {
            creator_net += t.tokens as i128;
        } else {
            creator_net -= t.tokens as i128;
            f.creator_sold = true;
        }
    }
    if let Some(ci) = &cd.create {
        creator_net += ci.dev_tokens as i128;
        f.dev_buy_pct = Some(ci.dev_tokens as f64 / 1e15 * 100.0);
        f.other_wallets_in_create_tx = Some(ci.other_wallets_in_tx);
        f.creator_prev_creates = Some(ci.creator_prev_creates);
        if let Some(e0) = cd.state_at(ci.slot) {
            f.s0_in_sol = Some((e0.vq as f64 - ci.post_create.vq as f64) / LAMPORTS_PER_SOL);
        }
    }
    f.creator_net_pct = creator_net as f64 / 1e15 * 100.0;
    Some(f)
}

/// Segnali d'uscita sempre attivi nello slot: vendita del creatore, sell multi-wallet atomico (≥3 wallet).
pub fn exit_alarms(cd: &CurveData, slot: u64) -> Option<&'static str> {
    let ts = cd.trades_in_slot(slot);
    if ts.iter().any(|t| !t.is_buy && t.user == cd.stat.creator) {
        return Some("creator_sell");
    }
    let mut i = 0;
    while i < ts.len() {
        let sig = ts[i].sig;
        let mut sellers: Vec<Pubkey> = Vec::new();
        while i < ts.len() && ts[i].sig == sig {
            if !ts[i].is_buy && !sellers.contains(&ts[i].user) {
                sellers.push(ts[i].user);
            }
            i += 1;
        }
        if sellers.len() >= 3 {
            return Some("multi_wallet_sell");
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade(vq0: u64, vt0: u64, buy: bool, q: u64, tok: u64, sig: u8) -> TradeRec {
        let (vq, vt) = if buy { (vq0 + q, vt0 - tok) } else { (vq0 - q, vt0 + tok) };
        TradeRec {
            slot: 10,
            sig: [sig; 64],
            ev_idx: 0,
            mint: [1; 32],
            user: [sig; 32],
            creator: [9; 32],
            is_buy: buy,
            quote: q,
            tokens: tok,
            post: CurveState { vq, vt, rq: vq - VS0, rt: vt - VT_MINUS_RT, complete: false },
            fee: 0,
            cfee: 0,
            fee_bps: 95,
            cfee_bps: 30,
            mayhem: false,
            quote_sol: true,
            ix: 1,
        }
    }

    #[test]
    fn chain_orders_shuffled_trades() {
        let s0 = CurveState { vq: VS0 + 5_000_000_000, vt: 900_000_000_000_000, rq: 5_000_000_000, rt: 900_000_000_000_000 - VT_MINUS_RT, complete: false };
        let a = trade(s0.vq, s0.vt, true, 100, 1000, 1);
        let b = trade(a.post.vq, a.post.vt, false, 50, 400, 2);
        let c = trade(b.post.vq, b.post.vt, true, 70, 600, 3);
        let ch = order_chain(Some(s0), true, vec![c.clone(), a.clone(), b.clone()]);
        assert!(ch.chained && ch.gap == 0);
        assert_eq!(ch.ordered.iter().map(|t| t.sig[0]).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(ch.end.unwrap().vq, c.post.vq);
        // senza il trade di mezzo la catena si rompe
        let ch2 = order_chain(Some(s0), true, vec![c.clone(), a.clone()]);
        assert!(!ch2.chained);
        // manca il primo trade dello slot: catena intera dalla testa, buco pari al suo importo
        let ch4 = order_chain(Some(s0), true, vec![c, b.clone()]);
        assert!(ch4.chained);
        assert_eq!(ch4.gap, 100);
        // catena non ancora avviata: trova la testa da sola
        let ch3 = order_chain(None, false, vec![b.clone(), trade(b.post.vq, b.post.vt, true, 5, 9, 4)]);
        assert!(ch3.chained && ch3.gap == 0);
        assert_eq!(ch3.ordered[0].sig[0], 2);
    }
}
