//! Engine di paper trading: N configurazioni indipendenti, esecuzione a slot (DESIGN §5, sim-model §1.3).

use std::collections::HashMap;
use std::sync::mpsc::SyncSender;

use serde::Serialize;

use crate::config::{Band, EngineCfg, FillMode, StratConfig, Variant, fnv1a};
use crate::math;
use crate::state::{self, CurveData, Features, MarketState, Set};
use crate::store::Rec;
use crate::types::*;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Entry {
    pub exec_slot: u64,
    pub budget: u64,
    pub net: u64,
    pub tokens: u64,
    pub price: f64,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct ExitOrder {
    pub reason: String,
    pub trigger_slot: u64,
    pub exec_slot: u64,
    pub expected_lv: u64,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Pos {
    pub curve: Pubkey,
    pub mint: Pubkey,
    pub signal_key: i64,
    pub signal_slot: u64,
    pub p_trig: f64,
    pub entry_due: u64,
    pub entry: Option<Entry>,
    pub exit: Option<ExitOrder>,
    pub last_eval: u64,
    pub last_activity: u64,
    pub peak_lv: u64,
    pub trail_on: bool,
    pub retries: u32,
    pub failed_tx: u32,
    pub data_gap: bool,
}

pub struct Book {
    pub cfg: StratConfig,
    pub open: HashMap<Pubkey, Pos>,
    pub cooldown: HashMap<Pubkey, u64>,
}

#[derive(Default, Debug, Clone)]
pub struct EngineStats {
    pub signals: u64,
    pub entries: u64,
    pub entry_failed: u64,
    pub closed: u64,
    pub skipped_busy: u64,
}

pub struct Engine {
    pub books: Vec<Book>,
    pub ecfg: EngineCfg,
    rec: SyncSender<Rec>,
    pub stats: EngineStats,
    size: u64,
}

/// Esito di una valutazione di segnali su una curva in uno slot.
struct Triggers {
    band_a: bool,
    band_b: bool,
    v: [bool; 6],
}

fn vidx(v: Variant) -> usize {
    match v {
        Variant::V0Random => 0,
        Variant::V1aLetterale => 1,
        Variant::V1b => 2,
        Variant::V2 => 3,
        Variant::V3 => 4,
        Variant::V4 => 5,
    }
}

impl Engine {
    pub fn new(strats: Vec<StratConfig>, ecfg: EngineCfg, rec: SyncSender<Rec>) -> Self {
        let size = lamports(ecfg.size_sol);
        let books = strats.into_iter().map(|cfg| Book { cfg, open: HashMap::new(), cooldown: HashMap::new() }).collect();
        Engine { books, ecfg, rec, stats: EngineStats::default(), size }
    }

    /// Curve con posizioni o ordini aperti (da tenere osservate e in memoria).
    pub fn interest(&self) -> Set<Pubkey> {
        let mut s = Set::default();
        for b in &self.books {
            for k in b.open.keys() {
                s.insert(*k);
            }
        }
        s
    }

    pub fn n_open(&self) -> usize {
        self.books.iter().map(|b| b.open.len()).sum()
    }

    fn send(&self, r: Rec) {
        let _ = self.rec.try_send(r);
    }

    /// Avanza tutte le posizioni fino a `through` (slot già finalizzati per tutte le curve).
    pub fn advance_all(&mut self, st: &MarketState, through: u64) {
        let fallback = self.ecfg.fallback_fee_bps;
        for bi in 0..self.books.len() {
            let keys: Vec<Pubkey> = self.books[bi].open.keys().copied().collect();
            for k in keys {
                self.advance_pos(bi, st, &k, through, fallback);
            }
        }
    }

    /// Un nuovo slot finalizzato con attività su `curve`: prima le posizioni, poi i segnali.
    pub fn on_slot(&mut self, st: &MarketState, curve: &Pubkey, slot: u64) {
        self.advance_all(st, slot.saturating_sub(1));
        let fallback = self.ecfg.fallback_fee_bps;
        for bi in 0..self.books.len() {
            if self.books[bi].open.contains_key(curve) {
                self.advance_pos(bi, st, curve, slot, fallback);
            }
        }
        self.signals(st, curve, slot);
    }

    fn signals(&mut self, st: &MarketState, curve: &Pubkey, slot: u64) {
        let Some(cd) = st.curves.get(curve) else { return };
        let Some(mint) = cd.mint else { return };
        let Some(s) = cd.state_at(slot) else { return };
        if s.complete || !cd.stat.in_universe() {
            return;
        }
        let (p, rq) = (s.progress(), s.rq_sol());
        let band_a = Band::AUtente.contains(p, rq);
        let band_b = Band::BProgresso.contains(p, rq);
        if !band_a && !band_b {
            return;
        }
        let Some(f) = state::features(cd, slot, &st.agent) else { return };
        let tr = Triggers { band_a, band_b, v: self.variant_triggers(&f) };
        let mut recorded: [bool; 12] = [false; 12];
        let size = self.size;
        for bi in 0..self.books.len() {
            let (band, variant, latency, id) = {
                let c = &self.books[bi].cfg;
                (c.band, c.variant, c.latency, c.id.clone())
            };
            let in_band = match band {
                Band::AUtente => tr.band_a,
                Band::BProgresso => tr.band_b,
            };
            if !in_band {
                continue;
            }
            let fire = if variant == Variant::V0Random {
                let h = fnv1a(&format!("{}{}{}", crate::b58::encode32(&mint), slot, id));
                (h as f64 / u64::MAX as f64) < self.ecfg.v0_prob
            } else {
                tr.v[vidx(variant)]
            };
            if !fire {
                continue;
            }
            let ri = (band as usize) * 6 + vidx(variant);
            let key = signal_key(band, variant, &mint, slot);
            // i segnali di momentum si registrano sempre; quelli del controllo casuale solo se aprono una posizione
            let can_open = {
                let book = &self.books[bi];
                !book.open.contains_key(curve)
                    && !book.cooldown.get(curve).map(|c| slot < *c).unwrap_or(false)
                    && book.open.len() < self.ecfg.max_positions
            };
            if !recorded[ri] && (variant != Variant::V0Random || can_open) {
                recorded[ri] = true;
                self.stats.signals += 1;
                self.send(Rec::Signal {
                    key,
                    band: band.name(),
                    variant: variant.name(),
                    mint,
                    curve: *curve,
                    slot,
                    price: s.price(),
                    features: serde_json::to_string(&f).unwrap_or_default(),
                });
            }
            if !can_open {
                self.stats.skipped_busy += 1;
                continue;
            }
            let _ = size;
            let book = &mut self.books[bi];
            let pos = Pos {
                curve: *curve,
                mint,
                signal_key: key,
                signal_slot: slot,
                p_trig: s.price(),
                entry_due: slot + latency,
                entry: None,
                exit: None,
                last_eval: slot,
                last_activity: slot,
                peak_lv: 0,
                trail_on: false,
                retries: 0,
                failed_tx: 0,
                data_gap: false,
            };
            book.open.insert(*curve, pos.clone());
            self.send(Rec::OpenPos { config_id: id.clone(), curve: *curve, data: serde_json::to_string(&pos).unwrap_or_default() });
        }
    }

    fn variant_triggers(&self, f: &Features) -> [bool; 6] {
        let flow_ok = !f.dirty;
        let t20 = &f.t20;
        let t30 = &f.t30;
        let s50 = &f.s50;
        let v1a = flow_ok && t20.full && t20.n_buy > t20.n_sell && t20.net_sol > 0.0;
        let v1b = flow_ok
            && t20.full
            && t20.n_buy as f64 >= 1.2 * t20.n_sell as f64
            && t20.net_sol >= 0.5
            && t20.uniq_buyers >= 6
            && t20.top3_share <= 0.60;
        let v2 = flow_ok && s50.full && s50.n_buy > s50.n_sell && s50.net_sol >= 1.0 && s50.uniq_buyers >= 6 && f.accel50;
        let v3 = flow_ok
            && s50.full
            && f.dprog50.map(|d| d >= 0.03).unwrap_or(false)
            && f.d2prog.map(|d| d > 0.0).unwrap_or(false)
            && s50.max_buy_share <= 0.40;
        let v4 = flow_ok
            && t30.full
            && t30.ofi >= 0.4
            && t30.n_buy > 0
            && (t30.uniq_buyers as f64 / t30.n_buy as f64) >= 0.6
            && t30.liq_vel >= self.ecfg.v4_liq_velocity_sol;
        [false, v1a, v1b, v2, v3, v4]
    }

    fn advance_pos(&mut self, bi: usize, st: &MarketState, curve: &Pubkey, through: u64, fallback: (u64, u64)) {
        let Some(cd) = st.curves.get(curve) else { return };
        let latency = self.books[bi].cfg.latency;
        let fill = self.books[bi].cfg.fill;
        let spec = self.books[bi].cfg.exit.spec();
        let fees = cd.fees(fallback);
        let size = self.size;
        let mut closed: Option<(Pos, &'static str, u64, u64, String)> = None;
        let mut changed = false;
        {
            let ecfg = &self.ecfg;
            let Some(pos) = self.books[bi].open.get_mut(curve) else { return };
            loop {
                // 1) ingresso in attesa
                if pos.entry.is_none() {
                    if pos.entry_due > through {
                        break;
                    }
                    let Some(s) = fill_state(cd, pos.entry_due, fill, true) else {
                        closed = Some((pos.clone(), "no_state", 0, 0, "cancelled".into()));
                        break;
                    };
                    if s.complete {
                        closed = Some((pos.clone(), "complete_before_entry", 0, 0, "cancelled".into()));
                        break;
                    }
                    if s.price() > pos.p_trig * (1.0 + ecfg.buy_guard) {
                        pos.failed_tx += 1;
                        closed = Some((pos.clone(), "buy_guard", 0, 0, "entry_failed".into()));
                        break;
                    }
                    let (net, tokens) = math::buy_exact_in(&s, size, fees.0, fees.1);
                    if tokens == 0 {
                        closed = Some((pos.clone(), "no_tokens", 0, 0, "cancelled".into()));
                        break;
                    }
                    pos.entry = Some(Entry { exec_slot: pos.entry_due, budget: size, net, tokens, price: s.price() });
                    pos.last_eval = pos.entry_due;
                    pos.last_activity = pos.entry_due;
                    pos.data_gap |= cd.is_dirty(pos.entry_due);
                    changed = true;
                    continue;
                }
                let entry = pos.entry.clone().unwrap();
                // 2) uscita in attesa di esecuzione
                if let Some(ex) = pos.exit.clone() {
                    if ex.exec_slot > through {
                        break;
                    }
                    let s = fill_state(cd, ex.exec_slot, fill, false).unwrap_or_default();
                    let out = math::liquidation_value(&s, entry.tokens, fees.0, fees.1);
                    pos.data_gap |= cd.is_dirty(ex.exec_slot);
                    let floor = (ex.expected_lv as f64 * (1.0 - ecfg.sell_guard)) as u64;
                    if out < floor && pos.retries < ecfg.max_sell_retries && !s.complete {
                        pos.retries += 1;
                        pos.failed_tx += 1;
                        pos.exit = Some(ExitOrder { exec_slot: ex.exec_slot + 1, ..ex });
                        changed = true;
                        continue;
                    }
                    let outcome = if pos.data_gap { "data_gap" } else { "ok" };
                    closed = Some((pos.clone(), "", out, ex.exec_slot, outcome.into()));
                    break;
                }
                // 3) valutazione delle uscite negli slot (last_eval, through]
                let mut trigger: Option<(&'static str, u64)> = None;
                let ts_slot = entry.exec_slot + spec.time_stop;
                for c in cd.change_slots(pos.last_eval, through) {
                    if c.saturating_sub(pos.last_activity) > ecfg.stale_slots {
                        trigger = Some(("stale", pos.last_activity + ecfg.stale_slots));
                        break;
                    }
                    if ts_slot < c {
                        trigger = Some(("time_stop", ts_slot));
                        break;
                    }
                    pos.last_activity = c;
                    pos.data_gap |= cd.is_dirty(c);
                    let s = cd.state_at(c).unwrap_or_default();
                    if s.complete {
                        trigger = Some(("graduated", c));
                        break;
                    }
                    if let Some(a) = state::exit_alarms(cd, c) {
                        trigger = Some((a, c));
                        break;
                    }
                    let lv = math::liquidation_value(&s, entry.tokens, fees.0, fees.1);
                    let r = lv as f64 / size as f64 - 1.0;
                    if let Some(tp) = spec.tp {
                        if r >= tp {
                            trigger = Some(("take_profit", c));
                            break;
                        }
                    }
                    if !pos.trail_on && r <= spec.sl {
                        trigger = Some(("stop_loss", c));
                        break;
                    }
                    if let Some((act, dd)) = spec.trail {
                        if r >= act {
                            pos.trail_on = true;
                        }
                        if pos.trail_on {
                            pos.peak_lv = pos.peak_lv.max(lv);
                            if (lv as f64) <= pos.peak_lv as f64 * (1.0 - dd) {
                                trigger = Some(("trailing", c));
                                break;
                            }
                        }
                    }
                }
                if trigger.is_none() {
                    let stale_at = pos.last_activity + ecfg.stale_slots;
                    let first = [("stale", stale_at), ("time_stop", ts_slot)]
                        .into_iter()
                        .filter(|(_, s)| *s <= through)
                        .min_by_key(|(_, s)| *s);
                    trigger = first;
                }
                pos.last_eval = through;
                match trigger {
                    Some((reason, tslot)) => {
                        let s = cd.state_at(tslot).unwrap_or_default();
                        let expected_lv = math::liquidation_value(&s, entry.tokens, fees.0, fees.1);
                        pos.exit = Some(ExitOrder { reason: reason.into(), trigger_slot: tslot, exec_slot: tslot + latency, expected_lv });
                        changed = true;
                        continue;
                    }
                    None => break,
                }
            }
        }
        let id = self.books[bi].cfg.id.clone();
        if let Some((pos, why, out, exit_slot, outcome)) = closed {
            self.books[bi].open.remove(curve);
            let cool_from = exit_slot.max(pos.entry_due);
            self.books[bi].cooldown.insert(*curve, cool_from + self.ecfg.cooldown_slots);
            self.close(&id, pos, why, out, exit_slot, &outcome);
        } else if changed {
            if let Some(pos) = self.books[bi].open.get(curve) {
                self.send(Rec::OpenPos { config_id: id, curve: *curve, data: serde_json::to_string(pos).unwrap_or_default() });
            }
        }
    }

    fn close(&mut self, config_id: &str, pos: Pos, why: &str, sol_out: u64, exit_slot: u64, outcome: &str) {
        let e = &self.ecfg;
        let (cost_comp, cost_eco, pnl_comp, pnl_eco, sol_in, tokens, entry_slot, entry_price) = match &pos.entry {
            Some(en) => {
                let cc = 2.0 * e.cost_comp_ok + pos.failed_tx as f64 * e.cost_comp_fail;
                let ce = 2.0 * e.cost_eco_ok + pos.failed_tx as f64 * e.cost_eco_fail;
                let gross = sol_out as f64 - en.budget as f64;
                (cc, ce, gross / 1e9 - cc, gross / 1e9 - ce, en.budget, en.tokens, en.exec_slot, en.price)
            }
            None => {
                let cc = pos.failed_tx as f64 * e.cost_comp_fail;
                let ce = pos.failed_tx as f64 * e.cost_eco_fail;
                (cc, ce, -cc, -ce, 0, 0, pos.entry_due, 0.0)
            }
        };
        match outcome {
            "entry_failed" => self.stats.entry_failed += 1,
            "ok" | "data_gap" => {
                self.stats.entries += 1;
                self.stats.closed += 1
            }
            _ => {}
        }
        let reason = pos.exit.as_ref().map(|x| x.reason.clone()).unwrap_or_else(|| why.to_string());
        self.send(Rec::PaperTrade {
            config_id: config_id.to_string(),
            signal_key: pos.signal_key,
            mint: pos.mint,
            curve: pos.curve,
            signal_slot: pos.signal_slot,
            entry_slot,
            entry_price,
            sol_in,
            tokens,
            exit_reason: reason,
            exit_trigger_slot: pos.exit.as_ref().map(|x| x.trigger_slot).unwrap_or(0),
            exit_slot,
            sol_out,
            retries: pos.retries,
            cost_comp,
            cost_eco,
            pnl_comp,
            pnl_eco,
            outcome: outcome.to_string(),
        });
        self.send(Rec::ClosePos { config_id: config_id.to_string(), curve: pos.curve });
    }

    /// Chiude tutte le posizioni senza esito (fine replay o arresto): non entrano nelle metriche.
    pub fn abort_all(&mut self, outcome: &str) {
        for bi in 0..self.books.len() {
            let id = self.books[bi].cfg.id.clone();
            let ps: Vec<Pos> = self.books[bi].open.drain().map(|(_, p)| p).collect();
            for p in ps {
                self.close(&id, p, outcome, 0, 0, outcome);
            }
        }
    }
}

pub fn signal_key(band: Band, variant: Variant, mint: &Pubkey, slot: u64) -> i64 {
    let mut s = String::with_capacity(64);
    s.push_str(band.name());
    s.push_str(variant.name());
    s.push_str(&crate::b58::encode32(mint));
    s.push_str(&slot.to_string());
    (fnv1a(&s) >> 1) as i64
}

/// Stato per il fill: a fine slot, oppure il peggiore dentro lo slot (buy: prezzo più alto; sell: più basso).
fn fill_state(cd: &CurveData, slot: u64, mode: FillMode, buy: bool) -> Option<CurveState> {
    let end = cd.state_at(slot)?;
    if mode == FillMode::EndOfSlot {
        return Some(end);
    }
    let mut best = end;
    for t in cd.trades_in_slot(slot) {
        let s = t.post;
        let worse = if buy { s.price() > best.price() } else { s.price() < best.price() };
        if worse {
            best = s;
        }
    }
    Some(best)
}
