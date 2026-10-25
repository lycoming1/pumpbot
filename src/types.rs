//! Tipi condivisi tra decoder, stato, motore e storage.

pub type Pubkey = [u8; 32];
pub type Sig = [u8; 64];

pub const ZERO_KEY: Pubkey = [0u8; 32];
pub const PUMP: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const AMM: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
pub const MINT_AUTHORITY: &str = "TSLvdd1pWpHVjahSpsvCXUbgwsL3JAcvokwaKt1eokM";
pub const MAYHEM_AGENT: &str = "BwWK17cbHxwWBKZkUYvzxLcNQ1YVyaFezduWbtm2de6s";
pub const WSOL: &str = "So11111111111111111111111111111111111111112";
/// Discriminatore dell'account BondingCurve in base58 (memcmp offset 0).
pub const BONDING_CURVE_DISC_B58: &str = "4y6pru6YvC7";

/// Costanti della curva standard (Global, verificate on-chain).
pub const VS0: u64 = 30_000_000_000;
pub const VT0: u64 = 1_073_000_000_000_000;
pub const RT0: u64 = 793_100_000_000_000;
pub const VT_MINUS_RT: u64 = 279_900_000_000_000;
/// Base del pool alla migrazione e fee di migrazione (spec §7).
pub const POOL_BASE_AT_MIGRATION: u64 = 206_900_000_000_000;
pub const POOL_MIGRATION_FEE: u64 = 15_000_001;

pub const LAMPORTS_PER_SOL: f64 = 1e9;

pub fn sol(lamports: u64) -> f64 {
    lamports as f64 / LAMPORTS_PER_SOL
}

pub fn lamports(sol: f64) -> u64 {
    (sol * LAMPORTS_PER_SOL).round() as u64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CurveState {
    pub vq: u64,
    pub vt: u64,
    pub rq: u64,
    pub rt: u64,
    pub complete: bool,
}

impl CurveState {
    /// Progresso sui token, come la UI: 1 - rt / RT0.
    pub fn progress(&self) -> f64 {
        1.0 - self.rt as f64 / RT0 as f64
    }
    /// Prezzo spot in lamport per unità raw di token.
    pub fn price(&self) -> f64 {
        if self.vt == 0 {
            return f64::INFINITY;
        }
        self.vq as f64 / self.vt as f64
    }
    pub fn rq_sol(&self) -> f64 {
        sol(self.rq)
    }
    /// Curva standard (non mayhem): invarianti verificati su tutte le coin SOL.
    pub fn is_standard(&self) -> bool {
        self.vq.checked_sub(self.rq) == Some(VS0) && self.vt.checked_sub(self.rt) == Some(VT_MINUS_RT)
    }
    pub fn same_reserves(&self, o: &CurveState) -> bool {
        self.vq == o.vq && self.vt == o.vt
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CurveStatic {
    pub creator: Pubkey,
    pub mayhem: bool,
    pub cashback: bool,
    pub quote_sol: bool,
    pub holder_reward: bool,
}

impl CurveStatic {
    pub fn in_universe(&self) -> bool {
        self.quote_sol && !self.mayhem && !self.cashback
    }
}

/// Un TradeEvent decodificato (riserve DOPO il trade).
#[derive(Clone, Debug)]
pub struct TradeRec {
    pub slot: u64,
    pub sig: Sig,
    pub ev_idx: u16,
    pub mint: Pubkey,
    pub user: Pubkey,
    pub creator: Pubkey,
    pub is_buy: bool,
    /// quote netto entrato o uscito dalla curva (senza fee)
    pub quote: u64,
    pub tokens: u64,
    pub post: CurveState,
    pub fee: u64,
    pub cfee: u64,
    pub fee_bps: u64,
    pub cfee_bps: u64,
    pub mayhem: bool,
    pub quote_sol: bool,
    pub ix: u8,
}

impl TradeRec {
    /// Pre-stato ricostruito dal post-stato (spec: buy vq0 = vq - q, vt0 = vt + t; sell al contrario).
    pub fn pre(&self) -> Option<(u64, u64)> {
        if self.is_buy {
            Some((self.post.vq.checked_sub(self.quote)?, self.post.vt.checked_add(self.tokens)?))
        } else {
            Some((self.post.vq.checked_add(self.quote)?, self.post.vt.checked_sub(self.tokens)?))
        }
    }
    /// SOL pagati (buy) o incassati (sell) dall'utente, fee comprese.
    pub fn user_quote(&self) -> u64 {
        if self.is_buy {
            self.quote + self.fee + self.cfee
        } else {
            self.quote.saturating_sub(self.fee + self.cfee)
        }
    }
}

#[derive(Clone, Debug)]
pub struct CreateRec {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub mint: Pubkey,
    pub curve: Pubkey,
    pub user: Pubkey,
    pub creator: Pubkey,
    pub mayhem: bool,
    pub cashback: bool,
    pub quote_sol: bool,
    pub holder_reward: bool,
}

#[derive(Clone, Debug)]
pub enum CompleteKind {
    Complete,
    Migration { pool: Pubkey, mint_amount: u64, sol_amount: u64, migration_fee: u64 },
    PostCompleteBuy { base_out: u64, quote_in: u64, pool_base_after: u64, pool_quote_after: u64 },
}

#[derive(Clone, Debug)]
pub struct CompleteRec {
    pub mint: Pubkey,
    pub curve: Pubkey,
    pub kind: CompleteKind,
}

/// Tutti gli eventi pump di una tx riuscita.
#[derive(Clone, Debug)]
pub struct TxEvents {
    pub slot: u64,
    pub sig: Sig,
    pub arrival_ns: u64,
    pub trades: Vec<TradeRec>,
    pub creates: Vec<CreateRec>,
    pub completes: Vec<CompleteRec>,
    pub truncated: bool,
}

impl Default for TxEvents {
    fn default() -> Self {
        TxEvents { slot: 0, sig: [0u8; 64], arrival_ns: 0, trades: Vec::new(), creates: Vec::new(), completes: Vec::new(), truncated: false }
    }
}

/// Evento in ingresso al nucleo (dal vivo o dal replay).
#[derive(Clone, Debug)]
pub enum Event {
    Account { slot: u64, curve: Pubkey, state: CurveState, stat: CurveStatic, arrival_ns: u64 },
    /// `sub_curve`: curva della sottoscrizione Tier B che ha consegnato la tx (per imparare mint→curva)
    Tx { tx: TxEvents, sub_curve: Option<Pubkey> },
    Slot(u64),
}
