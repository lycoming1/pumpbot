//! Configurazione TOML e espansione della griglia in configurazioni di paper trading.

use std::fmt;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub feed: FeedCfg,
    pub storage: StorageCfg,
    pub telegram: TelegramCfg,
    pub engine: EngineCfg,
    pub grid: GridCfg,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            feed: FeedCfg::default(),
            storage: StorageCfg::default(),
            telegram: TelegramCfg::default(),
            engine: EngineCfg::default(),
            grid: GridCfg::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FeedCfg {
    /// Hostname WS usati in parallelo (deduplica): almeno due per la ridondanza.
    pub ws_urls: Vec<String>,
    pub commitment: String,
    pub ping_secs: u64,
    /// Riconnessione se un flusso che dovrebbe essere continuo resta muto per questi secondi.
    pub idle_reconnect_secs: u64,
    pub max_subs_per_conn: usize,
    pub max_watched: usize,
    /// Sottoscrive ogni curva watched su tutti gli hostname (true) o solo sul primo.
    pub redundant_tier_b: bool,
    pub watch_rq_sol: f64,
    pub unwatch_rq_sol: f64,
    pub unwatch_low_slots: u64,
    pub watch_idle_slots: u64,
    pub complete_unwatch_slots: u64,
    pub finalize_lag_slots: u64,
    pub dirty_cooldown_slots: u64,
    /// Variazione non spiegata (SOL) oltre la quale un buco nei dati rende il mint dirty.
    pub dirty_min_gap_sol: f64,
    /// Pausa tra due richieste di sottoscrizione sulla stessa connessione (ms).
    pub sub_spacing_ms: u64,
}

impl Default for FeedCfg {
    fn default() -> Self {
        FeedCfg {
            ws_urls: vec!["wss://api.mainnet-beta.solana.com".into(), "wss://api.mainnet.solana.com".into()],
            commitment: "confirmed".into(),
            ping_secs: 20,
            idle_reconnect_secs: 45,
            max_subs_per_conn: 40,
            max_watched: 300,
            redundant_tier_b: true,
            watch_rq_sol: 1.0,
            unwatch_rq_sol: 0.5,
            unwatch_low_slots: 300,
            watch_idle_slots: 3000,
            complete_unwatch_slots: 600,
            finalize_lag_slots: 75,
            dirty_cooldown_slots: 300,
            dirty_min_gap_sol: 0.05,
            sub_spacing_ms: 40,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StorageCfg {
    pub dir: String,
    pub retention_days: u64,
    pub batch_rows: usize,
    pub batch_ms: u64,
}

impl Default for StorageCfg {
    fn default() -> Self {
        StorageCfg { dir: "/data".into(), retention_days: 21, batch_rows: 1000, batch_ms: 250 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TelegramCfg {
    pub token_env: String,
    pub chat_id_env: String,
    pub report_every_hours: u64,
    pub alert_feed_silent_secs: u64,
    pub alert_dirty_threshold: usize,
}

impl Default for TelegramCfg {
    fn default() -> Self {
        TelegramCfg {
            token_env: "TELEGRAM_BOT_TOKEN".into(),
            chat_id_env: "TELEGRAM_CHAT_ID".into(),
            report_every_hours: 6,
            alert_feed_silent_secs: 120,
            alert_dirty_threshold: 50,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct EngineCfg {
    pub size_sol: f64,
    pub bankroll_sol: f64,
    pub max_positions: usize,
    pub cooldown_slots: u64,
    pub buy_guard: f64,
    pub sell_guard: f64,
    pub max_sell_retries: u32,
    pub stale_slots: u64,
    pub v0_prob: f64,
    pub v4_liq_velocity_sol: f64,
    /// Fee in bps se la curva non ha ancora un TradeEvent non-mayhem (protocol, creator).
    pub fallback_fee_bps: (u64, u64),
    /// Profilo competitive: base + priority + tip per tx riuscita (SOL); tx fallita: base + priority.
    pub cost_comp_ok: f64,
    pub cost_comp_fail: f64,
    pub cost_eco_ok: f64,
    pub cost_eco_fail: f64,
}

impl Default for EngineCfg {
    fn default() -> Self {
        EngineCfg {
            size_sol: 0.25,
            bankroll_sol: 10.0,
            max_positions: 5,
            cooldown_slots: 50,
            buy_guard: 0.15,
            sell_guard: 0.25,
            max_sell_retries: 5,
            stale_slots: 150,
            v0_prob: 0.10,
            v4_liq_velocity_sol: 0.05,
            fallback_fee_bps: (95, 30),
            cost_comp_ok: 0.000005 + 0.00015 + 0.001,
            cost_comp_fail: 0.000005 + 0.00015,
            cost_eco_ok: 0.00004,
            cost_eco_fail: 0.000035,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GridCfg {
    pub bands: Vec<Band>,
    pub variants: Vec<Variant>,
    pub exits: Vec<ExitRule>,
    pub latencies: Vec<u64>,
    /// Latenze per cui si aggiunge anche la variante di fill WorstInSlot.
    pub worst_in_slot_latencies: Vec<u64>,
}

impl Default for GridCfg {
    fn default() -> Self {
        GridCfg {
            bands: vec![Band::AUtente, Band::BProgresso],
            variants: vec![Variant::V0Random, Variant::V1aLetterale, Variant::V1b, Variant::V2, Variant::V3, Variant::V4],
            exits: vec![ExitRule::E1, ExitRule::E2, ExitRule::E3],
            latencies: vec![1, 3, 6],
            worst_in_slot_latencies: vec![3],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum Band {
    #[serde(rename = "A_utente")]
    AUtente,
    #[serde(rename = "B_progresso")]
    BProgresso,
}

impl Band {
    pub fn name(&self) -> &'static str {
        match self {
            Band::AUtente => "A_utente",
            Band::BProgresso => "B_progresso",
        }
    }
    /// A: progresso 5–30% e rq ≥ 5 SOL (cioè 19,33–30%); B: progresso 5–30%.
    pub fn contains(&self, progress: f64, rq_sol: f64) -> bool {
        let in_prog = (0.05..=0.30).contains(&progress);
        match self {
            Band::AUtente => in_prog && rq_sol >= 5.0,
            Band::BProgresso => in_prog,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum Variant {
    #[serde(rename = "V0_random")]
    V0Random,
    #[serde(rename = "V1a_letterale")]
    V1aLetterale,
    V1b,
    V2,
    V3,
    V4,
}

impl Variant {
    pub fn name(&self) -> &'static str {
        match self {
            Variant::V0Random => "V0_random",
            Variant::V1aLetterale => "V1a_letterale",
            Variant::V1b => "V1b",
            Variant::V2 => "V2",
            Variant::V3 => "V3",
            Variant::V4 => "V4",
        }
    }
    pub const ALL: [Variant; 6] =
        [Variant::V0Random, Variant::V1aLetterale, Variant::V1b, Variant::V2, Variant::V3, Variant::V4];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum ExitRule {
    E1,
    E2,
    E3,
}

impl ExitRule {
    pub fn name(&self) -> &'static str {
        match self {
            ExitRule::E1 => "E1",
            ExitRule::E2 => "E2",
            ExitRule::E3 => "E3",
        }
    }
    pub fn spec(&self) -> ExitSpec {
        match self {
            ExitRule::E1 => ExitSpec { tp: Some(0.20), sl: -0.10, trail: None, time_stop: 283 },
            ExitRule::E2 => ExitSpec { tp: Some(0.50), sl: -0.20, trail: None, time_stop: 1415 },
            ExitRule::E3 => ExitSpec { tp: None, sl: -0.15, trail: Some((0.15, 0.10)), time_stop: 1415 },
        }
    }
}

/// Soglie sul rendimento del valore di liquidazione netto rispetto alla size.
#[derive(Debug, Clone, Copy)]
pub struct ExitSpec {
    pub tp: Option<f64>,
    pub sl: f64,
    /// (attivazione, drawdown dal picco)
    pub trail: Option<(f64, f64)>,
    pub time_stop: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FillMode {
    EndOfSlot,
    WorstInSlot,
}

impl FillMode {
    pub fn name(&self) -> &'static str {
        match self {
            FillMode::EndOfSlot => "EOS",
            FillMode::WorstInSlot => "WIS",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StratConfig {
    pub id: String,
    pub name: String,
    pub band: Band,
    pub variant: Variant,
    pub exit: ExitRule,
    pub latency: u64,
    pub fill: FillMode,
}

impl fmt::Display for StratConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// FNV-1a 64 bit: id stabile dei parametri.
pub fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Config> {
        let s = std::fs::read_to_string(path)?;
        let c: Config = toml::from_str(&s)?;
        Ok(c)
    }

    pub fn strategies(&self) -> Vec<StratConfig> {
        let mut out = Vec::new();
        for band in &self.grid.bands {
            for variant in &self.grid.variants {
                for exit in &self.grid.exits {
                    let mut fills: Vec<(u64, FillMode)> =
                        self.grid.latencies.iter().map(|l| (*l, FillMode::EndOfSlot)).collect();
                    for l in &self.grid.worst_in_slot_latencies {
                        fills.push((*l, FillMode::WorstInSlot));
                    }
                    for (latency, fill) in fills {
                        let name = format!("{}/{}/{}/L{}/{}", band.name(), variant.name(), exit.name(), latency, fill.name());
                        let e = &self.engine;
                        let params = format!(
                            "{name}|size={}|guard={}/{}|cool={}|stale={}|v0={}|v4={}",
                            e.size_sol, e.buy_guard, e.sell_guard, e.cooldown_slots, e.stale_slots, e.v0_prob,
                            e.v4_liq_velocity_sol
                        );
                        let id = format!("{:016x}", fnv1a(&params));
                        out.push(StratConfig { id, name, band: *band, variant: *variant, exit: *exit, latency, fill });
                    }
                }
            }
        }
        out
    }
}
