# Stack Rust per il bot pump.fun (Fase 1, osservatore + paper trading): crate, Cargo.toml, profilo release, Dockerfile, storage, gRPC opzionale, stime di build

## Summary
Ho verificato sulle API di crates.io (24 ott 2026) le versioni attuali e la manutenzione di tutte le crate. Poi ho risolto con cargo 1.99.1, in Docker e senza compilare, l'albero delle dipendenze di ogni variante.

Lo stack consigliato:
- tokio 1.5x su runtime `current_thread`, con tokio-tungstenite 0.30 e rustls 0.23 con ring e webpki-roots (niente OpenSSL e niente aws-lc).
- serde_json con struct "borrowed", base64 0.23, five8 per base58, foldhash per le HashMap.
- rusqlite 0.40.2 `bundled` in un thread di scrittura dedicato.
- ureq 3 per Telegram, in un thread a parte, al posto di reqwest. reqwest funziona, ma aggiunge 44 crate (url/idna/icu e lo stack hyper).
- tracing con il filtro `Targets` (senza regex), toml 1.1.

Il binario base risolve a 91 dipendenze. La feature `grpc` (yellowstone-grpc-client 16 / proto 14 / tonic 0.14) le porta a 184. DuckDB bundled (29 MB di C++) e solana-sdk+client (503 crate) sono da escludere su questa VM.

Lo storage misurato non è un vincolo: SQLite in WAL a batch fa 0,4-0,9 M righe/s (con un indice 56-81 k righe/s), contro le circa 13-30 righe/s che servono. Lo schema normalizzato occupa circa 56-76 B/riga, cioè 112-151 MB/giorno a 2 M righe. La cattura grezza del JSON si comprime 10,8x con zstd -3, ma resta circa 1,9 GB/giorno.

Il tempo di compilazione non l'ho misurato: compilare vuol dire eseguire i build.rs di crate non fidate, quindi ho dato solo stime. Il dato misurato è la compilazione di sqlite3.c (gcc -O3): 26,9 s su un core e 410 MB di picco. Build a freddo stimata in 1,5-3 min con CARGO_BUILD_JOBS=3 e picco di 1,0-1,5 GB. La VM ha 2,1 GB liberi e lo swap pieno.

## Spec
# Stack Rust per pumpbot (Fase 1): specifica

File pronti (manifest validato, lockfile risolto con cargo 1.99.1):
`/tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/rust-stack/deliverable/`
(`Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `Dockerfile`, `docker-compose.yaml`, `.dockerignore`, `config/pumpbot.toml`).

## 1. Scelte (verificate su crates.io il 2026-10-24)

| Ruolo | Scelta | Versione attuale (data) | Perché / alternative scartate |
|---|---|---|---|
| Runtime | `tokio`, `current_thread` | 1.54.1 (21/10/26); 1.53.3 (13/10) | Carico I/O-bound di <1k msg/s: un thread senza work-stealing ha la latenza più bassa. Feature: `rt, net, time, sync, macros, signal, io-util` (niente `rt-multi-thread`, niente `full`) |
| WebSocket | `tokio-tungstenite` + `tungstenite` | 0.30.0 (11/07/26), repo attivo | Handshake e TLS integrati, payload `Bytes` (zero-copy), `disable_nagle`, `WebSocketConfig.read_buffer_size` (default 128 KiB), `client_async_tls_with_config` per passare un `TcpStream` già connesso (solo IPv4). **fastwebsockets 0.10.0** (ultima vera release 14/01/2025, poi solo il backport 0.8.1): servono a mano l'upgrade hyper e tokio-rustls, e il vantaggio conta solo a 100k+ msg/s. Scartata |
| TLS | `rustls` con provider **ring** | 0.23.46 (14/10/26), webpki-roots 1.0.11 | `default-features=false, features=["ring","std","tls12"]`. Un solo provider in tutto il grafo, così `ClientConfig::builder()` lo sceglie da solo. All'avvio chiamare comunque `rustls::crypto::ring::default_provider().install_default()`. Evitare aws-lc-rs: più C da compilare. Attenzione: la feature `rustls` di reqwest 0.13 tira dentro aws-lc |
| JSON | `serde_json` + `serde` derive | 1.0.154 (11/10/26) / 1.0.229 | Bastano 5 crate. **sonic-rs 0.5.10** costa +24 crate e richiede `target-cpu` per la SIMD. **simd-json 0.18.1** costa +20 crate e un buffer mutabile. Secondo il benchmark del README di sonic-rs (twitter.json) sonic-rs è ~1,7x più veloce di `serde_json::from_str`, ma su 1,9 KB/msg fa 2 µs contro 4 µs: irrilevante (sez. 9) |
| base64 | `base64` | 0.23.1 (04/08/26) | `default-features=false, features=["std"]`: evita la nuova feature default `simd-unsafe`, inutile su stringhe di ~520 caratteri. Usare `general_purpose::STANDARD.decode_slice()` su un buffer `[u8;1024]` nello stack |
| base58 | `five8` | 1.0.0 (12/07/25) | `encode_32`/`decode_32`/`decode_64` a dimensione fissa, senza allocazioni (la usa solana-pubkey). `bs58` 0.5.1 è fermo al 03/2024 (repo fermo al 05/2024) |
| Hash map | `foldhash::fast::RandomState` | 0.2.0 (23/08/25) | Seed per processo, zero dipendenze, drop-in per `std::HashMap`. `rustc-hash` 2.1.3 va bene ma non ha seed, e i mint sono "grindabili" (indirizzi vanity), quindi c'è un HashDoS teorico. `ahash` 0.8.12: ultima release 05/2025. A migliaia di operazioni al secondo la scelta pesa meno dello 0,01% di CPU |
| Prefiltro byte | `memchr::memmem` | 2.8.3 | È già nel grafo (serde_json) |
| Storage | `rusqlite` `bundled` | **0.40.2** (08/08/26, SQLite 3.53.2) | La 0.41.0 (SQLite 3.54.0) è uscita oggi, 24/10: aspettare circa 2 settimane. Feature default: `cache` (prepare_cached) |
| Cattura grezza (opz.) | `zstd` | 0.14.3 (zstd 1.5.7) | `default-features=false` (niente `legacy` né `zdict_builder`). Feature `capture`. Se è attiva anche `grpc`, tonic usa zstd 0.13: allineare per non compilare due volte zstd-sys |
| Telegram | **`ureq`** in un thread dedicato | 3.5.0 (21/10/26); 3.4.2 (13/09) | `default-features=false, features=["rustls","json"]` riusa ring e webpki-roots: **+4 crate**. Config: `ip_family(IpFamily::Ipv4Only)`, `timeout_connect`, `timeout_global`. **reqwest 0.13.5** con `rustls-no-provider`+`json` funziona (nemmeno lì OpenSSL), ma porta **+44 crate** (hyper, tower, url/idna/icu…). Tenerlo solo se serve molto HTTP asincrono |
| Log | `tracing` + `tracing-subscriber` | 0.1.44 / 0.3.23 | Feature `["fmt","std"]`. Filtro con `filter::Targets` (implementa `FromStr`): niente `env-filter`, quindi niente regex |
| Config | `toml` + serde | 1.1.8+spec-1.1.0 | `default-features=false, features=["std","serde","parse"]`. I segreti si leggono da env (il config contiene solo i nomi delle variabili) |
| Errori | `anyhow` | 1.0.104 | — |
| Allocatore (opz.) | `mimalloc` | 0.1.52 | Feature `mimalloc`. L'hot path è quasi senza allocazioni: misurare prima di attivarlo |
| gRPC (futuro) | `yellowstone-grpc-client` / `-proto` / `tonic` | 16.0.0 (20/10/26) / 14.0.1 / 0.14.6 | Dietro la feature `grpc`, vedi sez. 7 |
| Toolchain | Rust **1.99.1** (manifest stable del 15/10/26) | immagine `lukemathwalker/cargo-chef:0.1.78-rust-1.99.1-slim-trixie` (22/10, 353 MB compressi) | Runtime `gcr.io/distroless/cc-debian13:nonroot` (glibc trixie come il builder). Non usare il builder trixie con un runtime debian12: la glibc del builder è più nuova |

## 2. Cargo.toml (risolto: 91 dipendenze + root)

```toml
[package]
name = "pumpbot"
version = "0.1.0"
edition = "2024"
rust-version = "1.89"
publish = false

[features]
default = []
capture  = ["dep:zstd"]
mimalloc = ["dep:mimalloc"]
grpc     = ["dep:yellowstone-grpc-client", "dep:yellowstone-grpc-proto", "dep:tonic"]

[dependencies]
tokio = { version = "1.53", default-features = false, features = ["rt", "net", "time", "sync", "macros", "signal", "io-util"] }
tokio-tungstenite = { version = "0.30", default-features = false, features = ["connect", "rustls-tls-webpki-roots"] }
rustls = { version = "0.23.40", default-features = false, features = ["ring", "std", "tls12"] }
futures-util = { version = "0.3.31", default-features = false, features = ["sink", "std"] }
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"
memchr = "2.7"
base64 = { version = "0.23.1", default-features = false, features = ["std"] }
five8 = "1.0"
foldhash = "0.2"
rusqlite = { version = "0.40.2", features = ["bundled"] }
ureq = { version = "3.4", default-features = false, features = ["rustls", "json"] }
tracing = { version = "0.1.44", default-features = false, features = ["std"] }
tracing-subscriber = { version = "0.3.23", default-features = false, features = ["fmt", "std"] }
toml = { version = "1.1", default-features = false, features = ["std", "serde", "parse"] }
anyhow = "1.0.100"

zstd = { version = "0.14.3", default-features = false, optional = true }
mimalloc = { version = "0.1.52", default-features = false, optional = true }
yellowstone-grpc-client = { version = "16.0", optional = true }
yellowstone-grpc-proto = { version = "14.0.1", default-features = false, features = ["tonic"], optional = true }
tonic = { version = "0.14.6", default-features = false, features = ["tls-ring"], optional = true }

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
debug = false
incremental = false

[profile.release-fast]      # per iterare: meno RAM e meno tempo, prestazioni quasi uguali
inherits = "release"
lto = "thin"
codegen-units = 16
```

`.cargo/config.toml`: `[build] rustflags = ["-C", "target-cpu=x86-64-v3"]`. AVX2, BMI2, FMA e MOVBE sono presenti sulla VM (verificato in /proc/cpuinfo, Ryzen 7 5700X). È più portabile di `native`. Se si esporta `RUSTFLAGS`, questo valore viene ignorato.

Note sul profilo:
- **`lto="fat"` + `codegen-units=1`** dà binario minimo e inlining cross-crate. Le dipendenze producono solo bitcode e il lavoro LLVM pesante si concentra nel link finale, single-thread: è il picco di RAM.
- **`panic="abort"`**: un panic in qualunque task fa terminare il processo, e Docker lo riavvia (`restart: unless-stopped`). Si perde al massimo un batch non committato (≤250 ms). Per questo le posizioni paper vanno scritte subito in `paper.db`, a ogni cambio di stato.
- Il bot è limitato dalla rete: fat contro thin cambia meno del 10% su microsecondi di CPU. `release-fast` va benissimo anche in produzione se la RAM di build è un problema.
- Gli script di build e le proc-macro compilano già a `opt-level=0` (default `build-override` di cargo).

## 3. Dockerfile (cargo-chef + cache mount del registry)

```dockerfile
# syntax=docker/dockerfile:1.7
ARG CHEF_IMAGE=lukemathwalker/cargo-chef:0.1.78-rust-1.99.1-slim-trixie
ARG RUNTIME_IMAGE=gcr.io/distroless/cc-debian13:nonroot

FROM ${CHEF_IMAGE} AS chef
WORKDIR /app
ENV CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_NET_RETRY=5 CARGO_TERM_COLOR=never

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG FEATURES=""
ARG PROFILE=release
COPY .cargo ./.cargo                      # stessi rustflags per cook e build
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo chef cook --profile "${PROFILE}" --locked --recipe-path recipe.json ${FEATURES:+--features $FEATURES}
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build --profile "${PROFILE}" --locked --bin pumpbot ${FEATURES:+--features $FEATURES} \
 && cp "target/${PROFILE}/pumpbot" /pumpbot

FROM ${RUNTIME_IMAGE}
COPY --from=builder /pumpbot /usr/local/bin/pumpbot
WORKDIR /data
ENTRYPOINT ["/usr/local/bin/pumpbot"]
CMD ["--config", "/config/pumpbot.toml"]
```

Note:
- cargo-chef 0.1.78 supporta `--profile`, `--features` e `--locked`: verificato nei sorgenti.
- L'immagine chef (base rust:slim-trixie) contiene gcc 14.2, sufficiente per sqlite3.c e ring. Non servono cmake né perl.
- **Limite di RAM della build**: `docker build` con BuildKit non accetta `--memory`. Le opzioni sono due:
  1. `CARGO_BUILD_JOBS=3` (o 2);
  2. un builder dedicato: `docker buildx create --name lowmem --driver docker-container --driver-opt memory=1800m,memory-swap=1800m`, poi `docker buildx build --builder lowmem --load ...` (buildx 0.38.0 è presente sulla VM).

Compose (in `deliverable/docker-compose.yaml`):
- `user: "1000:1000"` per scrivere su `./data` di kraken. Distroless nonroot sarebbe l'UID 65532.
- `read_only: true`, `cap_drop: [ALL]`, `no-new-privileges`, nessuna porta esposta.
- `env_file: .env` con `HELIUS_API_KEY`/`HELIUS_WS_URL`, `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID` (chmod 600).
- `mem_limit: 256m`, `cpus: 2.0`, `stop_grace_period: 15s`.
- Distroless non ha shell: niente healthcheck con curl. Se serve, aggiungere un sottocomando `pumpbot --healthcheck` che legge un file heartbeat in /data.

## 4. Architettura di runtime (3 thread, nessun lock nell'hot path)

1. **Thread principale: tokio `current_thread`.**
   - Un task per sorgente: WS `logsSubscribe` `mentions:[6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P]`, eventualmente PumpPortal, e in futuro gRPC.
   - Il motore di strategia gira **inline** nel loop di ricezione (pochi µs), senza passaggi su canali.
   - Timer `interval(250 ms)` per time stop e trailing sulle posizioni aperte.
   - Le rare chiamate RPC HTTP (bootstrap di una curva, getSlot) vanno con `ureq` dentro `tokio::task::spawn_blocking`.
2. **Thread writer (std::thread)**: possiede le `rusqlite::Connection`.
   - Riceve un `enum Record` da `std::sync::mpsc::sync_channel(65_536)`. Nell'hot path si usa solo `try_send`: se la coda è piena si incrementa `dropped` e non si blocca mai il runtime.
   - Loop con `recv_timeout(250ms)`, poi commit a 1000 righe o 250 ms.
   - Usa `prepare_cached`.
3. **Thread notifier**: `ureq::Agent` IPv4-only, coda bounded di 256 e circa 1 msg/s verso la chat.
   - Il report giornaliero lo calcola il motore a un'ora UTC fissa, così si evitano tz/chrono.
   - Retry 3 volte con backoff.

**Connessione WS:**
- `tokio::net::lookup_host` → solo IPv4 → `TcpStream::connect` con timeout di 5 s → `set_nodelay(true)` → `client_async_tls_with_config(req, stream, Some(cfg), None)`. Il motivo è la lezione del bot Telegram dell'homelab: con l'IPv6 rotto ogni connessione aspettava 15-30 s, e tokio non fa happy-eyeballs.
- Ping ogni 30 s: Helius chiude le connessioni inattive da 10 minuti e consiglia un ping al minuto.
- Riconnessione con backoff esponenziale e jitter.
- Registrare i gap (slot mancanti) a ogni riconnessione.

**Hot path per ogni messaggio:**
1. `Message::Text(t)`: aggiornare i contatori di byte e messaggi.
2. Se `memmem::find(t, b"Program data: ")` non trova nulla, scartare. Le tx con eventi sono circa il 19% (campione datasource), quindi oltre l'80% dei messaggi non viene mai deserializzato.
3. `serde_json::from_str::<Notif<'_>>`, con struct borrowed:
   - `signature: &'a str`;
   - `err: Option<serde::de::IgnoredAny>`;
   - `logs: Vec<Cow<'a, str>>`. Serve `Cow`, non `&str`: le righe `Program log:` possono contenere escape, e con `&str` borrowed la deserializzazione fallirebbe.
   - `context.slot: u64`.
   - Il messaggio di ack della subscription va gestito a parte.
4. `err.is_some()`: scartare. Nel campione il 77,7% delle tx uniche è fallito.
5. Dedup per firma: nel campione datasource (public RPC, processed) ogni firma arrivava circa 2 volte. Si usano due generazioni di `HashSet<u64, foldhash>`, con chiave = primi 8 byte di `five8::decode_64(signature)`, ruotate ogni 60 s.
6. Si scorrono i log con uno **stack di invocazioni** (`Program X invoke [n]` fa push, `Program X success|failed` fa pop). Si decodificano solo i `Program data:` che hanno in cima allo stack il programma pump. Il motivo: il discriminatore Anchor `sha256("event:TradeEvent")[:8] = bddb7fd34ee661ee` è identico per qualunque programma che abbia un evento con quel nome, e nel campione ci sono eventi di altri programmi (MAyh, pAMM…).
7. base64 → `[u8;1024]` nello stack → `match` su discriminatore e lunghezza → decoder Borsh a mano → `engine.on_trade(ev, recv_instant, recv_unix_ms, slot)`.

Altri discriminatori: CreateEvent `1b72a94ddeeb6376`, CompleteEvent `5f72619cd42e9808`.

**Decoder TradeEvent** (IDL attuale del repo pump-public-docs). Gli offset includono gli 8 byte di discriminatore. Il prefisso fisso è **robusto ai campi aggiunti in coda**:

| offset | campo | tipo |
|---|---|---|
| 8 | mint | [u8;32] |
| 40 | sol_amount | u64 LE |
| 48 | token_amount | u64 |
| 56 | is_buy | u8 |
| 57 | user | [u8;32] |
| 89 | timestamp | i64 |
| 97 / 105 | virtual_sol / virtual_token_reserves | u64 |
| 113 / 121 | real_sol / real_token_reserves | u64 |
| 129 | fee_recipient | [u8;32] |
| 161 / 169 | fee_basis_points / fee | u64 |
| 177 | creator | [u8;32] |
| 209 / 217 | creator_fee_basis_points / creator_fee | u64 |
| 225 | track_volume | u8 |
| 226 / 234 / 242 | total_unclaimed / total_claimed / current_sol_volume | u64 |
| 250 | last_update_timestamp | i64 |
| 258 | ix_name | u32 len + bytes |

Dopo `ix_name` vengono, in ordine: mayhem_mode u8, cashback_bps, cashback, buyback_bps, buyback_fee (u64), shareholders `Vec` (u32 len + elementi), quote_mint [32], quote_amount, virtual_quote_reserves, real_quote_reserves, holder_rewards_bps, holder_rewards, creator_fee_unclaimed (u64). Questa coda pesa 125 byte con `shareholders` vuoto.

Verifica sulle dimensioni osservate dall'agente datasource: 390 = 258+4+3 ("buy")+125; 391 per "sell"; 403 per "buy_exact_sol_in"; 405 per "buy_exact_quote_in". Tutto torna. La coda va letta in modo sequenziale e solo se la lunghezza lo permette.

**Stato in memoria:**
- `HashMap<[u8;32], TokenState, foldhash::fast::RandomState>`. TokenState contiene le riserve dell'ultimo trade (ogni TradeEvent porta lo stato post-trade della curva, quindi **non serve fare polling delle curve** se si riceve tutto il flusso del programma), contatori a finestra (buy/sell, SOL buy/sell a 30 s e 120 s), first_seen e last_seen.
- Evizione ogni 60 s dei token fermi da più di 30 min o con curva `complete`. Qualche migliaio di token × ~1-2 KB < 20 MB.
- Le chiavi restano grezze da 32 byte. Il base58 (`five8::encode_32`) si usa solo in output.

**Strumentazione della latenza (senza crate):**
- `recv_unix_ms − timestamp*1000` (granularità di 1 s) e lag di slot contro `slotSubscribe` sulla stessa connessione.
- Istogrammi a bucket fissi in array. Statistiche ogni 60 s nei log e nel report giornaliero.
- Nei log, mai una riga per messaggio.

## 5. Storage

**Dati misurati** (Python sqlite3 sulla VM, 2 M righe sintetiche con entropia realistica: chiavi casuali da 32 B, firme da 64 B). È un limite inferiore per rusqlite.

| Layout | righe/s | B/riga | a 2 M righe/giorno |
|---|---|---|---|
| A: riga completa (mint/user BLOB 32 B, firma 64 B), senza indici | 424.000 | 216 | 432 MB |
| A + indice (mint,ts) | 55.900 | 270 | 541 MB |
| **B: normalizzata (mint_id, user_id INTEGER, senza firma)** | **918.700** | **55,8** | **112 MB** |
| B + indice (mint_id,ts) | 81.000 | 75,7 | 151 MB |
| Binario fisso completo (209 B) + zstd -3 | — | 171,6 (1,22x) | 343 MB |
| Binario normalizzato (57 B) + zstd -3 / -19 | — | 34,4 / 32,5 | 69 / 65 MB |
| JSON grezzo logsNotification (1027 messaggi reali) zstd -1/-3/-9/-19 | — | rapporto 10,5/10,8/12,6/13,4x | 234 kB/s → ~20 GB/giorno grezzi → **~1,9 GB/giorno** con zstd -3 |

**Fabbisogno**: circa 13,4 TradeEvent riusciti al secondo sulla curva (1623 in 121 s, campione dell'agente datasource), cioè ~1,16 M/giorno, più ~2/s di PumpSwap. I picchi arrivano a 685 msg/s. SQLite ha un margine di oltre 1000x in media e oltre 100x sui picchi, anche con un indice. Lo zstd -3 della CLI comprime a circa 160 MB/s su un thread, contro 0,23 MB/s di input.

**Raccomandazione:**
- `trades-YYYYMMDD.db` (un file per giorno UTC: retention con `rm`, senza VACUUM):
  - `PRAGMA journal_mode=WAL; synchronous=NORMAL; temp_store=MEMORY; cache_size=-16384; wal_autocheckpoint=4000; journal_size_limit=67108864;`
  - `trades(id INTEGER PRIMARY KEY, mint_id, user_id, is_buy, sol, tok, vsol, vtok, ts, recv_ms, slot)`
  - `mints(id INTEGER PRIMARY KEY, mint BLOB UNIQUE, first_seen_ms, creator BLOB)`
  - `wallets(id INTEGER PRIMARY KEY, pk BLOB UNIQUE)`
  - I dizionari stanno in RAM nel writer (`HashMap<[u8;32],u32>`) e si azzerano a ogni rotazione.
  - Nessun indice in scrittura: gli indici si creano a posteriori per l'analisi.
- `paper.db` (scritture rare, `synchronous=FULL`): `positions`, `fills` (con la firma del trade che ha fatto scattare l'ingresso o l'uscita e l'istante simulato), `daily_stats`, `config_snapshot`.
- Firme: solo per i fill (`store_signatures="fills_only"`). Tutte le firme costano +64 B incomprimibili a riga.
- Feature `capture`: file `.zst` orari dei messaggi grezzi, solo quelli con `Program data:` e `err:null` (stima ~0,5 GB/giorno). Retention 3-7 giorni.
- **DuckDB/Parquet fuori dal binario**: l'analisi si fa offline con la CLI DuckDB o con Python, leggendo i file SQLite (estensione sqlite). Costo di build pari a zero.

## 6. Telegram

POST `https://api.telegram.org/bot<TOKEN>/sendMessage` con JSON `{chat_id, text, parse_mode:"HTML"}`, via `ureq::Agent` configurato con `ip_family(IpFamily::Ipv4Only)`, `timeout_connect(5s)` e `timeout_global(15s)`. Gira solo nel thread notifier, mai nel runtime tokio.

## 7. Feature `grpc` (Yellowstone), solo per il futuro

- **Peso**: con la feature il grafo passa da 91 a 184 dipendenze (+93), da +37 MB di sorgente Rust e +3,3 MB di C (zstd-sys).
- **Perché pesa**: `yellowstone-grpc-client` 16 dipende da `tonic` senza disattivarne le feature default. Questo trascina router e server (axum, h2 server) anche per un uso solo client. In più ci sono `prost-build`/`tonic-prost-build` (codegen al build), regex, flate2/zlib-rs e zstd 0.13.
- **Proto 14 è leggero**: `yellowstone-grpc-proto` 14 non dipende più da solana-sdk/transaction-status, solo da `solana-pubkey` 4.
- **Provider TLS**: `tls-native-roots` di tonic non sceglie un provider, quindi nel manifest c'è `tonic = { features=["tls-ring"] }`.
- **Supply chain**: il build script di proto **esegue un protoc precompilato** scaricato come crate (`protoc-bin-vendored`).
- **Bonus**: il client 16 espone già `AutoReconnect`, `DedupStream` e `ReconnectGeyserStream`.
- **Disponibilità**: su Helius il gRPC/LaserStream mainnet non è nel piano Free (vedi domande aperte).

## 8. Stime di build (STIME, non misurate)

**Input misurati:**
- Default: 91 dipendenze, ~14 build script, 3 proc-macro (serde_derive, thiserror-impl, tokio-macros). 29,6 MB di sorgente Rust (di cui libc 4,5 MB, quasi tutto escluso dai cfg) e 14,4 MB di C/asm.
- sqlite3.c: 9,5 MB, 269k righe. **gcc 14.2 -O3 su 1 core: 26,9 s, picco del container 410 MB (misurato).**
- ring: 4,85 MB di C/asm.

| Scenario (8 vCPU Zen3) | Tempo stimato | Picco RAM stimato |
|---|---|---|
| Cook dipendenze a freddo, JOBS=3-4 (cammino critico: libsqlite3-sys, cioè gcc 27 s, poi rusqlite) | 60-110 s | 1,0-1,5 GB (JOBS=2: 0,6-0,9 GB) |
| Crate del bot + fat LTO, cgu=1 | 25-60 s | 0,7-1,2 GB (link LTO single-thread) |
| Totale a freddo (escluso il pull di circa 350 MB dell'immagine chef) | 1,5-3 min | ~1,5 GB |
| Rebuild dopo modifica a `src/` (deps nel layer cook) | 25-60 s con `release`; 8-20 s con `release-fast` | 0,5-1,0 GB |
| + feature `grpc` | +1,5-3 min a freddo | +0,3-0,6 GB |
| DuckDB bundled (DuckDB v2.0.0: 2112 file C/C++, 29,3 MB, 44 MB scompattati) | 10-25+ min | diversi GB → **no** |
| solana-sdk 5 + solana-client 4.3 (503 crate, 111 MB Rust, 48 build script) | 8-15 min | 3-4 GB → **no** |

La VM ha 2,1 GB disponibili e lo swap da 4 GB pieno. La memoria dell'homelab segnala che le build pesanti bloccano kraken. Quindi: JOBS=3 oppure il builder buildx con `memory=1800m`, oppure build su un'altra macchina e `docker save | ssh kraken docker load`. Per sostituire le stime con misure: prima build con `cargo build --timings` e lettura di `memory.peak`.

**Runtime** (stima): binario strippato di 4-6 MB, RSS 15-40 MB (buffer WS 128 KiB, cache SQLite 16 MB, mappa dei token sotto i 20 MB). Il `mem_limit` di 256 MB è largo.

## 9. Costo CPU dell'hot path

Campione reale (1027 messaggi): media 1878 B, p50 653 B, p95 6510 B, max 15,7 KB, 20,6 righe di log per messaggio (max 214). Le righe `Program data:` hanno una mediana di 520 caratteri base64 (max 680).

Python `json.loads` sulla VM: 4,9 µs/msg (385 MB/s). serde_json tipizzato e borrowed è dello stesso ordine o più veloce. Al picco di 685 msg/s fanno circa 3 ms di CPU al secondo (0,3% di un core), e il prefiltro memmem ne evita oltre l'80%. Il collo di bottiglia è la rete: sul public RPC il lag p50 dopo slotNotification era di 27 s (agente datasource). Per questo la scelta del parser JSON o dell'hash non sposta la profittabilità; la scelta della sorgente dati sì.

## 10. Crate pubblicate da meno di 14 giorni nel Cargo.lock

cc 1.10.0 (1 giorno), smallvec 1.16.4 (1), webpki-roots 1.0.11 (1), tokio 1.54.1 (3), ureq 3.5.0 (3), rustls-webpki 0.103.16 (4), find-msvc-tools 0.1.15 (5), ureq-proto 0.6.5 (7), libc 0.2.191 (8), rustls 0.23.46 (10), syn 3.0.9 (12), unicode-ident 1.0.27 (12), serde_json 1.0.154 (13).

Per una politica di cooldown, prima della prima build: `cargo update -p tokio --precise 1.53.3`, `cargo update -p ureq --precise 3.4.2`, ecc. Committare sempre `Cargo.lock` e buildare con `--locked`.

## Recommendations
- Usare il Cargo.toml in deliverable/ così com'è: tokio current_thread, tokio-tungstenite 0.30 + rustls/ring/webpki-roots, serde_json borrowed (logs come Cow), base64 senza simd-unsafe, five8, foldhash, rusqlite 0.40.2 bundled, ureq 3 per Telegram, tracing con filtro Targets, toml 1.1. Sono 91 dipendenze, senza OpenSSL né aws-lc.
- Per Telegram scegliere ureq in un thread dedicato invece di reqwest: costa 4 crate contro 44, condivide lo stesso stack TLS ring e ha un'opzione IPv4-only nativa. Se si usa reqwest 0.13, servono `rustls-no-provider` più l'installazione del provider ring, altrimenti la feature `rustls` tira dentro aws-lc.
- Hot path: prefiltro memmem su 'Program data: ' prima di qualsiasi parse JSON, scarto delle tx con err, dedup per firma (nel campione public RPC ogni firma arrivava circa 2 volte), stack di invocazioni per attribuire gli eventi al programma 6EF8 (il discriminatore TradeEvent è uguale per qualunque programma Anchor). Decoder Borsh con prefisso a offset fissi e coda letta in modo sequenziale.
- Non fare polling delle curve se si riceve logsSubscribe sull'intero programma: ogni TradeEvent porta già le riserve virtuali e reali dopo il trade.
- Storage: un file SQLite al giorno con schema normalizzato (mint_id/user_id), WAL + synchronous=NORMAL, batch da 1000 righe o 250 ms, nessun indice in scrittura, firme solo per i fill paper. paper.db separato con synchronous=FULL e posizioni scritte a ogni cambio (serve perché panic=abort).
- Cattura grezza (feature capture) solo con filtro su Program data ed err:null e retention di 3-7 giorni: senza filtro sono circa 1,9 GB/giorno anche compressi.
- DuckDB e Parquet solo offline, leggendo i file SQLite; mai nel binario. Niente solana-sdk/solana-client/anchor.
- Build: CARGO_BUILD_JOBS=3 oppure un builder buildx docker-container con memory=1800m. Usare il profilo release-fast (thin LTO, cgu=16) per iterare e passare a release (fat LTO, cgu=1, panic=abort) per la versione definitiva. Valutare la build su un'altra macchina, visto che kraken ha lo swap pieno.
- Feature grpc solo quando c'è un endpoint Yellowstone (non incluso nel Free di Helius). Allineare zstd alla 0.13 se capture e grpc sono attive insieme. Tenere presente che il build esegue un protoc precompilato.
- Connessioni: risoluzione solo IPv4, TCP_NODELAY, timeout di connect, ping WS ogni 30 s (Helius chiude dopo 10 minuti di inattività), riconnessione con backoff e registrazione dei gap di slot.
- Prima build reale con `cargo build --timings`, leggendo memory.peak, per sostituire le stime di tempo e RAM con misure. Fissare con `cargo update --precise` le crate pubblicate da meno di 14 giorni se si vuole un cooldown sulla supply chain.

## Measured
Versioni e date dall'API di crates.io il 2026-10-24:
- tokio 1.54.1 (21/10), tokio-tungstenite/tungstenite 0.30.0 (11/07), fastwebsockets 0.10.0 (01/2025, poi solo il backport 0.8.1), rustls 0.23.46.
- serde_json 1.0.154, simd-json 0.18.1, sonic-rs 0.5.10, base64 0.23.1, bs58 0.5.1 (03/2024), five8 1.0.0, ahash 0.8.12, rustc-hash 2.1.3, foldhash 0.2.0.
- rusqlite 0.41.0 (uscita OGGI, SQLite 3.54.0) e 0.40.2 (SQLite 3.53.2), zstd 0.14.3, duckdb 1.20000.0 (= DuckDB v2.0.0), parquet/arrow 60.0.0.
- yellowstone-grpc-client 16.0.0, proto 14.0.1, tonic 0.14.6, prost 0.14.4, reqwest 0.13.5, ureq 3.5.0, tracing 0.1.44, tracing-subscriber 0.3.23, toml 1.1.8, cargo-chef 0.1.78.
- Rust 1.99.1 (manifest stable del 15/10). Immagini: rust:1.99.1-slim-trixie e cargo-chef:0.1.78-rust-1.99.1-slim-trixie (353 MB compressi). distroless cc-debian13:nonroot esiste.

Grafi delle dipendenze, risolti con cargo tree 1.99.1 in Docker, senza compilare (crate per target linux x86_64, root incluso):
- consigliato 92; con reqwest al posto di ureq 136; senza client HTTP 90; con ureq 94;
- +capture 96, +grpc 185, tutte le feature 189;
- duckdb bundled 104, più 2112 file C/C++ (29,3 MB, 44 MB scompattati) nel tar; parquet 47; solana-sdk 5 + solana-client 4.3 503 (111 MB di Rust, 48 build script);
- serde_json 6, sonic-rs 30, simd-json 26.
- Sorgenti del consigliato: 29,6 MB di Rust e 14,4 MB di C/asm (sqlite3.c 9,5 MB e 269k righe; ring 4,85 MB). Proc-macro: 3.

Compilazione di sqlite3.c con gcc 14.2 -O3 su 1 core in container: 26,9 s, picco memory.peak 410 MB.

Storage (Python sqlite3, WAL, NORMAL, batch da 2000, 2 M righe):
- completo: 424k righe/s e 216 B/riga; con indice (mint,ts) 55,9k righe/s e 270 B/riga;
- normalizzato: 918,7k righe/s e 55,8 B/riga; con indice 81k righe/s e 75,7 B/riga;
- binario completo + zstd -3: 171,6 B/riga (1,22x); binario normalizzato zstd -3/-19: 34,4/32,5 B/riga;
- zstd CLI -3 a circa 157-162 MB/s su un thread.

Messaggi reali (1027 logsNotification del campione dell'agente datasource):
- media 1878 B, p50 653 B, p95 6510 B, max 15.695 B; 20,6 righe di log per messaggio; base64 di Program data con mediana 520 caratteri;
- zstd -1/-3/-9/-19: rapporto 10,5/10,8/12,6/13,4x;
- Python json.loads 4,9 µs/msg (385 MB/s).

Calcolo: con il layout dell'IDL le dimensioni osservate di TradeEvent (390/391/403/405 B) tornano esattamente.

Altro verificato:
- Discriminatori: TradeEvent bddb7fd34ee661ee, CreateEvent 1b72a94ddeeb6376, CompleteEvent 5f72619cd42e9808.
- CPU della VM: ha AVX2, BMI2, FMA e MOVBE (x86-64-v3 ok); niente AVX-512. buildx v0.38.0.

Dati presi dall'agente datasource, non misurati da me: 160 msg/s (p95 337, max 685), 234 kB/s, 81 tx uniche/s, 77,7% di errori, circa 13,4 TradeEvent riusciti/s, lag del public RPC p50 27 s.

Il tempo di compilazione Rust e la RAM della build NON li ho misurati.

## Open questions
- Tempo e RAM reali della build: non li ho misurati, perché compilare esegue i build.rs e le proc-macro di crate non fidate. Va fatta una prima build con `cargo build --timings` e lettura di memory.peak. Decidere se farla su kraken (2,1 GB liberi, swap pieno) o su un'altra macchina.
- Helius Free: il metering dello streaming a 20 crediti/MB (2 crediti ogni 0,1 MB, dalle FAQ) vale anche per il logsSubscribe standard? Al ritmo misurato sul public RPC (234 kB/s, circa 20 GB/giorno) sarebbero circa 400k crediti/giorno, contro 1 M/mese del piano Free: finirebbero in circa 2,5 giorni. Va chiarito con l'agente datasource prima di scegliere la sorgente.
- Le crate pubblicate da meno di 14 giorni (tokio 1.54.1, ureq 3.5.0, rustls 0.23.46, webpki-roots 1.0.11, cc 1.10.0…) si accettano o si fissano versioni precedenti con `cargo update --precise`? E rusqlite 0.41.0 (uscita oggi) va aspettata?
- panic=abort va bene? Un panic fa riavviare il container e si perde al massimo un batch di 250 ms; le posizioni paper restano solo se vengono scritte subito in paper.db.
- Retention e dimensione del nastro dei trade: schema normalizzato (~112-151 MB/giorno) o completo con firme (~432-541 MB/giorno)? E la cattura grezza (circa 1,9 GB/giorno non filtrata) serve davvero?
- Il layout di TradeEvent viene dall'IDL attuale del repo, e torna con le dimensioni osservate solo con shareholders vuoto. Va confermato dall'agente onchain-spec che pump non emetta varianti (ad esempio create_v2 o quote mint diverso da SOL) che spostano il prefisso fisso.

## Sources
- https://crates.io/api/v1/crates/tokio (e la stessa API per tokio-tungstenite, tungstenite, fastwebsockets, rustls, tokio-rustls, webpki-roots, serde, serde_json, simd-json, sonic-rs, base64, bs58, five8, ahash, rustc-hash, foldhash, hashbrown, rusqlite, libsqlite3-sys, zstd, duckdb, libduckdb-sys, parquet, arrow, yellowstone-grpc-client, yellowstone-grpc-proto, tonic, prost, reqwest, ureq, hyper, tracing, tracing-subscriber, toml, cargo-chef, mimalloc, solana-sdk, solana-client, anchor-lang, memchr)
- https://static.crates.io/crates/<nome>/<nome>-<ver>.crate (Cargo.toml e sorgenti letti, non eseguiti: tokio-tungstenite 0.30.0, tungstenite 0.30.0, reqwest 0.13.5, ureq 3.5.0, rusqlite 0.40.2/0.41.0, yellowstone-grpc-client 16.0.0, yellowstone-grpc-proto 14.0.1, tonic 0.14.6, sonic-rs 0.5.10 README benchmark, simd-json 0.18.1, base64 0.23.1 RELEASE-NOTES, five8 1.0.0, foldhash 0.2.0, cargo-chef 0.1.78, libduckdb-sys duckdb.tar.gz)
- https://static.rust-lang.org/dist/channel-rust-stable.toml
- https://hub.docker.com/v2/repositories/library/rust/tags
- https://hub.docker.com/v2/repositories/lukemathwalker/cargo-chef/tags
- https://gcr.io/v2/distroless/cc-debian13/tags/list
- https://api.github.com/repos/denoland/fastwebsockets (e snapview/tokio-tungstenite, Nullus157/bs58-rs, tkaitchuck/aHash, cloudwego/sonic-rs, simd-lite/simd-json, rpcpool/yellowstone-grpc, algesten/ureq, rusqlite/rusqlite, LukeMathWalker/cargo-chef, seanmonstar/reqwest)
- https://www.helius.dev/docs/faqs/websockets.md
- https://helius.dev/docs/billing/rate-limits
- https://helius.dev/docs/billing/plans
- https://www.helius.dev/blog/laserstream-websockets
- https://docs.docker.com/build/builders/drivers/docker-container/
- https://github.com/cocoindex-io/cocoindex/issues/441 (dato aneddotico fat LTO + cgu=1: build da 1m12s a 2m44s)
- File locale (agente datasource dello stesso workflow): /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/datasource/pub_processed.json e *_sample.jsonl
- File locale (agente onchain-spec): /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/onchain-spec/dl/repo/idl/pump.json
- Deliverable: /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/rust-stack/deliverable/ (Cargo.toml, Cargo.lock, Dockerfile, docker-compose.yaml, .cargo/config.toml, config/pumpbot.toml); script e misure in /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/rust-stack/ (bench_storage.py, closure.py, pkgsize.py, out/*.pkgs). Nota: sulla VM è rimasta l'immagine rust:1.99.1-slim-trixie (1,27 GB), rimovibile con docker rmi.