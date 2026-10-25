//! Telegram in un thread dedicato (ureq, solo IPv4): report periodici e alert.

use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::time::Duration;

use tracing::{info, warn};

use crate::config::TelegramCfg;
use crate::report;
use crate::store::now_ms;

pub enum Note {
    Text(String),
    ReportNow,
}

pub fn spawn(cfg: TelegramCfg, paper_path: String) -> SyncSender<Note> {
    let (tx, rx) = sync_channel::<Note>(256);
    let _ = std::thread::Builder::new().name("notifier".into()).spawn(move || run(cfg, paper_path, rx));
    tx
}

fn run(cfg: TelegramCfg, paper_path: String, rx: Receiver<Note>) {
    let token = std::env::var(&cfg.token_env).ok().filter(|s| !s.is_empty());
    let chat = std::env::var(&cfg.chat_id_env).ok().filter(|s| !s.is_empty());
    if token.is_none() || chat.is_none() {
        warn!("Telegram non configurato ({} / {}): i messaggi vanno solo nei log", cfg.token_env, cfg.chat_id_env);
    }
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .ip_family(ureq::config::IpFamily::Ipv4Only)
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_global(Some(Duration::from_secs(20)))
            .http_status_as_error(false)
            .build(),
    );
    let every_ms = cfg.report_every_hours.max(1) * 3_600_000;
    let mut next = (now_ms() / every_ms + 1) * every_ms;
    loop {
        let msg = match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(m) => Some(m),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let mut report_now = false;
        match msg {
            Some(Note::Text(t)) => send(&agent, token.as_deref(), chat.as_deref(), &t),
            Some(Note::ReportNow) => report_now = true,
            None => {}
        }
        if report_now || now_ms() >= next {
            if !report_now {
                next += every_ms;
            }
            match build_report(&paper_path) {
                Ok(t) => send(&agent, token.as_deref(), chat.as_deref(), &t),
                Err(e) => warn!("report fallito: {e}"),
            }
        }
    }
}

fn build_report(paper_path: &str) -> anyhow::Result<String> {
    let conn = report::open_ro(paper_path)?;
    let reps = report::compute(&conn, 0, 2_000)?;
    Ok(format!("pumpbot · report paper trading\n{}", report::text(&reps, 30, 8)))
}

fn send(agent: &ureq::Agent, token: Option<&str>, chat: Option<&str>, text: &str) {
    info!("notifica: {}", text.lines().next().unwrap_or(""));
    let (Some(token), Some(chat)) = (token, chat) else { return };
    let body = serde_json::json!({
        "chat_id": chat,
        "text": format!("<pre>{}</pre>", html_escape(&truncate(text, 3900))),
        "parse_mode": "HTML",
        "disable_web_page_preview": true,
    });
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    for attempt in 0..3 {
        match agent.post(&url).send_json(&body) {
            Ok(r) if r.status().is_success() => return,
            Ok(r) => warn!("Telegram HTTP {}", r.status()),
            Err(e) => warn!("Telegram errore: {e}"),
        }
        std::thread::sleep(Duration::from_secs(2 << attempt));
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let mut end = n;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n…", &s[..end])
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
