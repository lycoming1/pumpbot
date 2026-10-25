//! Metriche per configurazione da paper.db (DESIGN §7, §9).

use std::collections::{BTreeMap, HashMap};

use rusqlite::{Connection, OpenFlags, params};

pub struct Row {
    pub r: f64,
    pub pnl_comp: f64,
    pub pnl_eco: f64,
    pub ts: i64,
}

#[derive(Default)]
pub struct Metrics {
    pub n: usize,
    pub win_rate: f64,
    pub mean: f64,
    pub median: f64,
    pub payoff: f64,
    pub pf: f64,
    pub pnl_comp: f64,
    pub pnl_eco: f64,
    pub max_dd: f64,
    pub mean_wo_top1: f64,
    pub ci: Option<(f64, f64)>,
    pub entry_failed: usize,
    pub entry_failed_cost: f64,
    pub excluded: BTreeMap<String, usize>,
}

pub struct CfgReport {
    pub id: String,
    pub name: String,
    pub m: Metrics,
}

/// xorshift64*: RNG deterministico per il bootstrap.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn bootstrap(rows: &[Row], iters: usize) -> Option<(f64, f64)> {
    if rows.len() < 10 {
        return None;
    }
    // blocchi per giorno UTC se ci sono almeno 3 giorni, altrimenti ricampionamento semplice
    let mut by_day: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
    for r in rows {
        by_day.entry(r.ts / 86_400_000).or_default().push(r.r);
    }
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let mut means = Vec::with_capacity(iters);
    if by_day.len() >= 3 {
        let blocks: Vec<&Vec<f64>> = by_day.values().collect();
        for _ in 0..iters {
            let (mut s, mut n) = (0.0, 0usize);
            for _ in 0..blocks.len() {
                let b = blocks[rng.below(blocks.len())];
                s += b.iter().sum::<f64>();
                n += b.len();
            }
            means.push(s / n as f64);
        }
    } else {
        for _ in 0..iters {
            let mut s = 0.0;
            for _ in 0..rows.len() {
                s += rows[rng.below(rows.len())].r;
            }
            means.push(s / rows.len() as f64);
        }
    }
    means.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some((means[(iters as f64 * 0.025) as usize], means[(iters as f64 * 0.975) as usize]))
}

pub fn metrics(mut rows: Vec<Row>, iters: usize) -> Metrics {
    let mut m = Metrics { n: rows.len(), ..Default::default() };
    if rows.is_empty() {
        return m;
    }
    rows.sort_by_key(|r| r.ts);
    let mut eq = 0.0f64;
    let mut peak = 0.0f64;
    for r in &rows {
        eq += r.pnl_comp;
        peak = peak.max(eq);
        m.max_dd = m.max_dd.max(peak - eq);
        m.pnl_comp += r.pnl_comp;
        m.pnl_eco += r.pnl_eco;
    }
    let mut rs: Vec<f64> = rows.iter().map(|r| r.r).collect();
    rs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = rs.len();
    m.mean = rs.iter().sum::<f64>() / n as f64;
    m.median = rs[n / 2];
    let wins: Vec<f64> = rs.iter().copied().filter(|x| *x > 0.0).collect();
    let losses: Vec<f64> = rs.iter().copied().filter(|x| *x <= 0.0).collect();
    m.win_rate = wins.len() as f64 / n as f64;
    let avg_w = if wins.is_empty() { 0.0 } else { wins.iter().sum::<f64>() / wins.len() as f64 };
    let avg_l = if losses.is_empty() { 0.0 } else { -losses.iter().sum::<f64>() / losses.len() as f64 };
    m.payoff = if avg_l > 0.0 { avg_w / avg_l } else { f64::INFINITY };
    let gp: f64 = rows.iter().map(|r| r.pnl_comp).filter(|x| *x > 0.0).sum();
    let gl: f64 = -rows.iter().map(|r| r.pnl_comp).filter(|x| *x <= 0.0).sum::<f64>();
    m.pf = if gl > 0.0 { gp / gl + 0.0 } else { f64::INFINITY };
    let cut = (n / 100).max(1);
    m.mean_wo_top1 = if n > cut { rs[..n - cut].iter().sum::<f64>() / (n - cut) as f64 } else { f64::NAN };
    m.ci = bootstrap(&rows, iters);
    m
}

pub fn open_ro(path: &str) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
}

pub fn compute(conn: &Connection, since_ms: i64, iters: usize) -> rusqlite::Result<Vec<CfgReport>> {
    let mut names: Vec<(String, String)> = Vec::new();
    {
        let mut st = conn.prepare("SELECT id, name FROM configs ORDER BY name")?;
        for r in st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.flatten() {
            names.push(r);
        }
    }
    let mut ok: HashMap<String, Vec<Row>> = HashMap::new();
    let mut fail: HashMap<String, (usize, f64)> = HashMap::new();
    let mut excl: HashMap<String, BTreeMap<String, usize>> = HashMap::new();
    let mut st = conn.prepare("SELECT config_id, outcome, r_comp, pnl_comp, pnl_eco, ts FROM trades WHERE ts >= ?1")?;
    let rows = st.query_map(params![since_ms], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<f64>>(2)?,
            r.get::<_, Option<f64>>(3)?,
            r.get::<_, Option<f64>>(4)?,
            r.get::<_, i64>(5)?,
        ))
    })?;
    for (cfg, outcome, r, pc, pe, ts) in rows.flatten() {
        match outcome.as_str() {
            "ok" => ok.entry(cfg).or_default().push(Row { r: r.unwrap_or(0.0), pnl_comp: pc.unwrap_or(0.0), pnl_eco: pe.unwrap_or(0.0), ts }),
            "entry_failed" => {
                let e = fail.entry(cfg).or_default();
                e.0 += 1;
                e.1 += pc.unwrap_or(0.0);
            }
            other => *excl.entry(cfg).or_default().entry(other.to_string()).or_default() += 1,
        }
    }
    let mut out = Vec::new();
    for (id, name) in names {
        let mut m = metrics(ok.remove(&id).unwrap_or_default(), iters);
        if let Some((n, c)) = fail.get(&id) {
            m.entry_failed = *n;
            m.entry_failed_cost = *c;
        }
        m.excluded = excl.remove(&id).unwrap_or_default();
        out.push(CfgReport { id, name, m });
    }
    Ok(out)
}

fn pct(x: f64) -> String {
    if x.is_finite() { format!("{:+.1}%", x * 100.0) } else { "n/d".into() }
}

pub fn line(c: &CfgReport) -> String {
    let m = &c.m;
    let ci = m.ci.map(|(a, b)| format!("[{} {}]", pct(a), pct(b))).unwrap_or_else(|| "[n/d]".into());
    format!(
        "{:<34} n={:<4} E={:<7} {:<17} med={:<7} win={:>3.0}% PF={:<5.2} PnL={:+.3}/{:+.3} DD={:.3} E-top1={} fail={}",
        c.name,
        m.n,
        pct(m.mean),
        ci,
        pct(m.median),
        m.win_rate * 100.0,
        m.pf,
        m.pnl_comp + m.entry_failed_cost,
        m.pnl_eco,
        m.max_dd,
        pct(m.mean_wo_top1),
        m.entry_failed
    )
}

/// Testo del report: le migliori `top` configurazioni con almeno `min_n` trade più i controlli V0 della stessa banda.
pub fn text(reps: &[CfgReport], min_n: usize, top: usize) -> String {
    let total: usize = reps.iter().map(|r| r.m.n).sum();
    let mut s = format!("K={} configurazioni, {} trade chiusi (esiti ok). E = expectancy per trade, profilo competitive.\n", reps.len(), total);
    let mut best: Vec<&CfgReport> = reps.iter().filter(|r| r.m.n >= min_n && !r.name.contains("V0_random")).collect();
    best.sort_by(|a, b| b.m.mean.partial_cmp(&a.m.mean).unwrap_or(std::cmp::Ordering::Equal));
    s.push_str(&format!("\nMigliori (n ≥ {min_n}):\n"));
    for r in best.iter().take(top) {
        s.push_str(&line(r));
        s.push('\n');
    }
    s.push_str("\nControlli V0_random:\n");
    let mut v0: Vec<&CfgReport> = reps.iter().filter(|r| r.name.contains("V0_random") && r.name.contains("/L3/EOS")).collect();
    v0.sort_by(|a, b| a.name.cmp(&b.name));
    for r in v0 {
        s.push_str(&line(r));
        s.push('\n');
    }
    s
}

pub fn cli(paper: &str, since: Option<&str>, filter: Option<&str>) -> anyhow::Result<()> {
    let conn = open_ro(paper)?;
    let since_ms = match since {
        Some(d) => parse_date_ms(d)?,
        None => 0,
    };
    let reps = compute(&conn, since_ms, 10_000)?;
    match filter {
        Some(f) => {
            for r in reps.iter().filter(|r| r.id == f || r.name.contains(f)) {
                println!("{}", line(r));
                if !r.m.excluded.is_empty() {
                    println!("    esclusi: {:?}", r.m.excluded);
                }
            }
        }
        None => {
            println!("{}", text(&reps, 30, 20));
            println!("\nTutte le configurazioni:");
            for r in &reps {
                println!("{}", line(r));
            }
        }
    }
    Ok(())
}

/// "AAAA-MM-GG" → ms unix (mezzanotte UTC).
pub fn parse_date_ms(d: &str) -> anyhow::Result<i64> {
    let p: Vec<&str> = d.split('-').collect();
    anyhow::ensure!(p.len() == 3, "data attesa AAAA-MM-GG");
    let (y, m, dd): (i64, i64, i64) = (p[0].parse()?, p[1].parse()?, p[2].parse()?);
    // giorni dal 1970-01-01 (Hinnant, inverso di civil_from_days)
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Ok(days * 86_400_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        let ms = parse_date_ms("2026-10-25").unwrap();
        assert_eq!(crate::store::utc_date((ms / 1000) as u64), "20261025");
    }

    #[test]
    fn metrics_basic() {
        let rows: Vec<Row> = (0..100).map(|i| Row { r: if i % 2 == 0 { 0.1 } else { -0.05 }, pnl_comp: 0.0, pnl_eco: 0.0, ts: i }).collect();
        let m = metrics(rows, 1000);
        assert!((m.mean - 0.025).abs() < 1e-9);
        assert!((m.win_rate - 0.5).abs() < 1e-9);
        let (lo, hi) = m.ci.unwrap();
        assert!(lo < 0.025 && hi > 0.025);
    }
}
