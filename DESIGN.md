# pumpbot — Fase 1: osservatore + paper trading pump.fun (Rust)

Obiettivo: misurare se la strategia "compra sulla bonding curve con momentum di acquisto" è profittevole
al netto di fee, costi di rete, ritardo di esecuzione e slippage. **Nessun ordine reale, nessuna chiave
privata, nessun wallet.** Il bot legge dati pubblici e simula.

Documenti di riferimento (verificati il 24/10/2026, leggerli prima di implementare):
- `docs/research/onchain-spec.md` — program ID, discriminatori, layout byte per byte di eventi e account,
  matematica intera della curva e di PumpSwap, fee, migrazione. **È la fonte di verità per il decoder.**
- `docs/research/sim-model.md` — modello di esecuzione a slot (SlotExec), costi, varianti di momentum,
  feature di manipolazione, metriche e protocollo go/no-go.
- `docs/research/data-feed.md` — misure delle sorgenti dati sull'RPC pubblico.
- `docs/research/rust-stack.md` + `docs/research/stack-deliverable/` — crate, versioni, Dockerfile.
- `docs/research/pumpspec_reference.py` — decoder Python di riferimento usato per verificare la spec.
- `tests/fixtures/tx_*/` — transazioni mainnet reali (JSON di getTransaction) per i test del decoder.

---

## 0. Correzioni dopo le misure del 24 ott 2026 (implementate)

Dalla registrazione di 4 ore fatta per l'analisi dello sniping (`docs/research/sniping.md`) sono emersi fatti che
cambiano alcune scelte di questo documento. Dove il resto del testo è in conflitto, vale questa sezione.

- **Ridondanza dei flussi.** Ogni WebSocket dell'RPC pubblico si chiude con errore 1002 ogni 2–3 minuti, e ogni
  chiusura perde 5–50 slot. Una sola connessione programSubscribe riceve il 93–97% delle notifiche dell'unione di due.
  - Tier A, Tier B e Tier C girano quindi su **due hostname** (`api.mainnet-beta.solana.com` e
    `api.mainnet.solana.com`), con deduplica: (slot, curva) per il Tier A, firma della tx per i log.
  - Con l'unione, il 97% degli slot con trade ha la notifica Tier A, con stato identico al 99,7%.
- **Finalizzazione solo per tempo.** Lo slot S di una curva si finalizza quando l'orologio supera
  S + `finalize_lag_slots` (default 75, ~16 s), non all'arrivo di un evento successivo. Il Tier A arriva 2–10 slot dopo i log
  del Tier B: finalizzare al primo evento successivo perderebbe lo stato di fine slot.
  - L'orologio è il massimo slot visto su qualunque flusso: `slotSubscribe` dell'RPC pubblico è in ritardo di 1–5 s
    sul vertice della catena.
- **Ordine dei trade nello slot.** I log non portano l'indice della tx nel blocco: l'ordine si ricostruisce alla
  finalizzazione concatenando pre-stato e post-stato, partendo dallo stato di fine slot precedente. Se la catena non si
  chiude, o non coincide con lo stato Tier A dello slot, il mint diventa `dirty`.
- **Buchi nel feed e soglia di rilevanza.** Anche con due hostname l'RPC pubblico non consegna alcune tx: hanno lo stesso
  backend. Nella prova del 25 ott una tx `Buy` valida non è arrivata su nessuna delle due connessioni.
  - Un mint diventa `dirty` solo se la variazione non spiegata (buco di catena, differenza dallo stato Tier A, trade in
    ritardo) vale almeno `dirty_min_gap_sol` (0,05 SOL).
  - Le rotture minori, tipicamente micro-acquisti da 1.000 lamport, restano contate (`breaks`) ma non sospendono i
    segnali.
- **Migrazione in posizione.** Il Tier D (log del pool) non viene implementato in Fase 1.
  - Quando la curva risulta completa, la posizione si chiude L slot dopo, al prezzo del pool appena creato: base
    206,9e12 token, quote = rq finale − 15.000.001 lamport, formula PumpSwap con fee 1,25%.
  - Il motivo d'uscita è `graduated`, analizzabile a parte.
  - È un evento raro, e il prezzo del pool è continuo con la fine della curva (−0,024%).
- **Valutazione.** Si usa lo stato osservato, senza il nostro acquisto: è prudente, come in §5.2.
- **Archivio per il replay.** Il database eventi registra:
  - gli stati Tier A di **tutte** le curve dell'universo (~40/s, ~0,2 GB/giorno), non solo di quelle watched;
  - i trade Tier B e i create, completi dei campi che servono al motore.

  Così `replay` può riprovare anche bande diverse.

## 1. Vincoli

- VM con ~2,5 GB di RAM disponibile e swap pieno: build **sempre** in container con limite di memoria
  (`-m 1600m --memory-swap 1600m`, `CARGO_BUILD_JOBS=2`), mai due build in parallelo.
  Runtime: `mem_limit: 256m`.
- Connessione residenziale, IPv6 rotto in passato: risoluzione **solo IPv4**, `TCP_NODELAY`.
- Costo zero in Fase 1: RPC pubblico Solana. Sorgente intercambiabile (Helius Free o gRPC in futuro).
- Niente crate Solana pesanti (`solana-sdk`, `solana-client`, `anchor`): decodifica manuale Borsh.
- `panic = "abort"` in release: lo stato del paper trading va scritto su disco a ogni cambiamento.

## 2. Architettura

Un solo binario `pumpbot`, runtime tokio `current_thread` per la parte di rete + un thread di scrittura
SQLite + un thread per Telegram. Il motore di simulazione è una **macchina a stati pura e
deterministica** che consuma `Event` ordinati: lo stesso codice gira dal vivo e in replay.

```
            ┌──────────── RPC pubblico (wss, confirmed) ─────────────┐
 Tier A  programSubscribe(pump, memcmp disc BondingCurve)  → stato di TUTTE le curve, ~37 msg/s
 Tier B  logsSubscribe(mentions = bonding_curve PDA)        → TradeEvent delle curve "watched"
 Tier C  logsSubscribe(mentions = mint-authority TSLvdd…)   → CreateEvent (+ dev buy)
 Tier D  logsSubscribe(mentions = pool PumpSwap)            → Buy/SellEvent dei mint in posizione migrati
 Clock   slotSubscribe                                      → orologio a slot (time stop, finalizzazione)
            └───────────────────────┬────────────────────────────────┘
                     decoder (log → eventi tipizzati, dedup, filtri)
                                    │  Event
                     MarketState (per mint/curva) ── feature, catena degli stati, dirty
                                    │
                     Engine: N configurazioni in parallelo → segnali → SlotExec fill → uscite
                                    │
        writer SQLite (eventi giornalieri, paper.db)      Telegram (report, alert)
```

### 2.1 Tier A — stato di tutte le curve
- `programSubscribe` sul programma pump `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`, filtro `memcmp`
  offset 0 = discriminatore BondingCurve `17b7f83760d8ac60` (in base58), `encoding: base64`,
  `commitment: confirmed`. Misurato: 37-38 msg/s, 22 kB/s, ritardo p50 2-3 slot.
- Decodifica dell'account BondingCurve (offset nella spec §5, account da 166 byte, accettare anche
  account più corti con campi mancanti a 0). Ogni notifica = stato **a fine slot** della curva.
- Universo tradabile: `quote_mint == SOL (chiave zero)`, `is_mayhem_mode == false`,
  `is_cashback_coin == false`, `complete == false`. Tutto il resto si ignora (eventualmente contato).
- Dopo ogni riconnessione: `getMultipleAccounts` (HTTP) su curve watched e posizioni aperte per
  riallineare lo stato.

### 2.2 Tier B — trade delle curve osservate
- Una curva diventa **watched** quando è nell'universo e `real_quote_reserves ≥ watch_rq_sol`
  (default 1,0 SOL ≈ 4,4% di progresso). Smette di esserlo quando `rq < unwatch_rq_sol` (0,5 SOL) per
  più di 300 slot, quando non riceve aggiornamenti da `watch_idle_slots` (3000 ≈ 10 min), o 600 slot
  dopo il complete se non c'è una posizione aperta. Le curve con posizioni aperte restano sempre watched.
- `logsSubscribe {mentions:[bonding_curve]}` con commitment confirmed. Un solo pubkey per
  sottoscrizione (limite RPC). Pool di connessioni WS con al massimo `max_subs_per_conn` (default 40)
  sottoscrizioni ciascuna e `max_watched` totale (default 300, priorità per rq decrescente e posizioni).
- Il mint si ricava dal primo TradeEvent o dal CreateEvent (Tier C).

### 2.3 Tier C — creazioni
- `logsSubscribe {mentions:["TSLvdd1pWpHVjahSpsvCXUbgwsL3JAcvokwaKt1eokM"]}` confirmed (~0,4-0,7 create
  uniche/s). Fornisce mint↔bonding_curve, creator, flag (mayhem, cashback, quote, holder reward), dev
  buy nella stessa tx, slot di creazione. Serve per le feature (bundle nello slot del create, creator
  seriale, quota del creator).

### 2.4 Tier D — PumpSwap per i mint migrati in posizione
- Il pool arriva dal `CompletePumpAmmMigrationEvent` (campo `pool`), che compare nei log della curva
  (Tier B). Si sottoscrive `logsSubscribe {mentions:[pool]}` finché la posizione è aperta.
- Considerare solo il pool canonico (base = mint, quote = WSOL). Escludere i BuyEvent con fee 0/0/0
  generati da `boost_buy_and_burn`.

### 2.5 Clock
- `slotSubscribe` su una connessione: slot corrente, usato per time stop, finalizzazione degli slot
  delle curve silenziose, misura del ritardo (`slot_corrente − slot_evento` all'arrivo).

### 2.6 Archivio completo (opzionale, default OFF)
- `logsSubscribe {mentions:[pump]}` confirmed per registrare tutto. Sull'RPC pubblico perde molti slot:
  attivarlo solo con una sorgente a pagamento. Non deve mai alimentare le decisioni.

## 3. Decoder

Seguire `docs/research/onchain-spec.md` alla lettera. Punti critici:
- Scartare le notifiche con `err != null`. Dedup per `(signature, indice evento)` con una finestra
  limitata (es. ultimi 200k).
- Attribuzione delle righe `Program data:` con lo **stack degli invoke** (`Program X invoke [n]` push,
  `Program X success|failed` pop). Accettare solo i frame pump `6EF8…` e PumpSwap `pAMMBay6…`.
- Prefiltro `memchr::memmem` su `Program data: ` prima del parse JSON. JSON con struct borrowed.
- Eventi da decodificare: pump `TradeEvent`, `CreateEvent`, `CompleteEvent`,
  `CompletePumpAmmMigrationEvent`; AMM `BuyEvent`, `SellEvent`, `CreatePoolEvent`. Ignorare i
  discriminatori sconosciuti. Tollerare byte in coda e campi mancanti in coda (leggi come 0).
- `TradeEvent`: prefisso fisso fino a offset 250, poi `ix_name` (string), poi campi relativi; leggere
  `mayhem_mode` e, dopo gli shareholder, `quote_mint`. Usare i campi `quote_*` per gli importi.
- `Log truncated`: contare e scartare (in Fase 1 non si recupera via getTransaction; il Tier A copre lo
  stato).
- Riserve **dopo** il trade. Pre-stato: buy `vq0 = vq − sol_amount`, `vt0 = vt + token_amount`;
  sell `vq0 = vq + sol_amount`, `vt0 = vt − token_amount`.

## 4. MarketState

Per ogni curva/mint:
- ultimo stato a fine slot (da Tier A, o dal post-stato dell'ultimo trade dello slot se più recente);
- storico breve degli stati per slot (ring, ~2000 slot) per feature di velocità e per i fill;
- trade recenti (ring, ~500) con slot, firma, user, is_buy, quote netto, token, fee bps, post-stato;
- **catena degli stati**: ogni trade deve avere pre-stato uguale al post-stato di un trade già visto
  (o allo stato Tier A). Se la catena si rompe il mint è `dirty`: le feature di flusso non sono
  affidabili e i segnali basati sul flusso sono sospesi per `dirty_cooldown_slots` (default 300).
  Valutazioni e fill usano comunque lo stato di Tier A, che non dipende dalla completezza del Tier B.
- dati dal create: slot di creazione, creator, dev buy, wallet che hanno comprato nello slot del create.

**Finalizzazione dello slot S di una curva**: quando arriva un evento della stessa curva con slot > S,
oppure quando il clock supera S + `finalize_lag_slots` (default 20). Le decisioni si prendono **solo**
su slot finalizzati. Dato che tutto è indicizzato per slot, il ritardo del feed non cambia i risultati,
solo la reattività dal vivo.

### 4.1 Feature (tutte registrate grezze con ogni segnale)
Dal flusso, escludendo l'agente mayhem `BwWK17cbHxwWBKZkUYvzxLcNQ1YVyaFezduWbtm2de6s` e i trade del
creator; i trade multi-wallet nella stessa firma contano come un attore; buy < 0,01 SOL contano nei
totali ma non nelle soglie di "buyer unici":
- finestra ultimi N trade: n_buy, n_sell, SOL buy, SOL sell, flusso netto, buyer unici, quota top-3,
  OFI = (buySOL−sellSOL)/(buySOL+sellSOL), velocità di liquidità ΔvSOL/n_trade, trade max / inflow;
- finestra ultimi W slot: idem + accelerazione (flusso netto seconda metà > prima metà);
- progresso, Δprogresso su W slot, seconda differenza (anche solo da Tier A);
- manipolazione: buy di altri wallet nello slot del create, dev buy %, quota del creator (limite
  superiore buy−sell), micro-buy %, flip rapidi (buy+sell stesso wallet entro 10 slot), sell
  multi-wallet nella stessa tx, wash atomico (buy+sell stesso wallet nella stessa tx), numero di
  create del creator visti dall'avvio.

## 5. Engine di paper trading

### 5.1 Configurazioni
File TOML con una griglia che si espande in un prodotto cartesiano; ogni configurazione ha un id
stabile (hash dei parametri) e un nome leggibile. Ogni configurazione ha posizioni proprie e
indipendenti, bankroll virtuale (default 10 SOL), max posizioni concorrenti (default 5), una posizione
per mint, cooldown per mint dopo l'uscita (default 50 slot).

Griglia iniziale (K dichiarato nel report):
- **Banda d'ingresso**:
  - `A_utente`: progresso 5-30% **e** rq ≥ 5 SOL (equivale a 19,33-30%: rendere visibile nel report);
  - `B_progresso`: progresso 5-30% senza vincolo di liquidità.
- **Varianti di segnale** (sim-model §4):
  - `V0_random` — controllo: ingresso casuale deterministico (hash(mint, slot, config) < soglia) su
    slot idonei della banda, senza momentum. Serve a capire se il momentum aggiunge qualcosa.
  - `V1a_letterale` — la regola dell'utente alla lettera: ultimi 20 trade, n_buy > n_sell, flusso netto > 0.
  - `V1b` — ultimi 20 trade, n_buy ≥ 1,2·n_sell, netto ≥ 0,5 SOL, buyer unici ≥ 6, top-3 ≤ 60%.
  - `V2` — ultimi 50 slot, n_buy > n_sell, netto ≥ 1 SOL, buyer unici ≥ 6, accelerazione > 0.
  - `V3` — Δprogresso ≥ 3 pp in 50 slot, seconda differenza > 0, nessun trade > 40% dell'inflow.
  - `V4` — ultimi 30 trade, OFI ≥ 0,4, buyer unici / n_buy ≥ 0,6, velocità di liquidità ≥ soglia.
- **Uscite** (valutate sul valore di liquidazione netto LV a fine slot, mai sul prezzo spot):
  - `E1`: TP +20%, SL −10%, time stop 283 slot (~60 s);
  - `E2`: TP +50%, SL −20%, time stop 1415 slot (~5 min);
  - `E3`: trailing (attivo da +15%, esce a −10% dal picco di LV), SL −15%, time stop 1415 slot.
  - Sempre attive: stale exit (nessun trade da 150 slot), uscita su sell multi-wallet atomico
    (≥ 3 wallet) e su vendita del creator (registrate come motivo).
- **Latenza**: L ∈ {1, 3, 6} slot, uguale per ingresso e uscita.
- **Modo di fill**: `EndOfSlot` per tutte; in più `WorstInSlot` solo per L = 3.
- **Size**: 0,25 SOL per trade.

### 5.2 SlotExec (sim-model §1.3) — regole anti-lookahead
- Segnale valutato a fine slot S con dati di slot ≤ S. Esecuzione allo **stato di fine slot S+L**
  (EndOfSlot) o al post-stato peggiore dentro S+L (WorstInSlot: per il buy il prezzo più alto, per il
  sell il più basso). Se in S+L non ci sono trade, ultimo stato noto ≤ S+L.
- Buy exact-in con budget B secondo la formula intera della spec §6 (net0, fee ceil, excess, `net−1`),
  limitato da `real_token_reserves`. Fee lette dall'ultimo TradeEvent non-mayhem della curva (fallback
  95/30 bps).
- Guard di slippage buy 15%: se il prezzo a S+L supera quello del trigger di oltre il 15%, la tx
  fallisce: si paga base+priority (non la tip), nessuna posizione.
- Sell di tutti i token a S_uscita + L con la formula di sell; guard 25% rispetto al LV atteso; se
  fallisce, retry allo slot successivo (max 5), pagando ogni volta base+priority; al 6° tentativo si
  vende comunque e si marca.
- Stato osservato non include il nostro acquisto (prudente). Registrare anche la quantità di token per
  analisi successive.
- Se la curva completa mentre siamo dentro: attendere il `CompletePumpAmmMigrationEvent`, poi valutare
  e uscire su PumpSwap con Qeff (i128 con segno), fee a scaglioni sulla mcap Qeff·supply/B (spec §7).
  Se la migrazione non arriva entro 150 slot: esito `unknown_migration`, escluso dalle metriche ma
  contato.
- Mint `dirty` mentre siamo in posizione: si continua a valutare con il Tier A; il trade viene marcato
  `data_gap` (analizzabile a parte).
- Riavvio del processo con posizioni aperte: chiuderle come `aborted_restart`, escluse dalle metriche.

### 5.3 Costi (sim-model §2)
- Profilo `competitive` (default nelle metriche): base 0,000005 + priority 0,00015 + tip 0,001 SOL per
  tx riuscita; tx fallita: base + priority.
- Profilo `economy`: ~0,00004 SOL per tx. I costi fissi si registrano a parte in modo che il report
  possa mostrare entrambi i profili sugli stessi trade.
- Il rent dell'ATA si paga e si recupera (capitale bloccato, non costo).

### 5.4 PnL per trade
`pnl = Σ sol_out − B − costi_fissi(profilo)`; `r = pnl / B`.

## 6. Storage
- `data/events-YYYYMMDD.db` (WAL, `synchronous=NORMAL`, batch da 1000 righe o 250 ms, schema
  normalizzato con tabelle dizionario mint/user): trade Tier B, stati Tier A delle curve watched,
  create, complete/migrazioni, trade PumpSwap dei pool seguiti, `stream_health` al minuto (messaggi/s,
  ritardo p50/p90 in slot, riconnessioni, sottoscrizioni attive, mint dirty, log troncati, duplicati).
  Ogni evento con `arrival_ns`.
- `data/paper.db` (`synchronous=FULL`): configurazioni, segnali (con feature JSON), fill, uscite,
  posizioni aperte (scritte a ogni cambiamento).
- Retention eventi: configurabile (default 21 giorni).

## 7. Report e notifiche
- `pumpbot report [--since <data>] [--config <id|nome>]`: per configurazione n, win rate, expectancy
  media e mediana, payoff, profit factor, PnL totale, max drawdown, expectancy senza il top 1%, IC 95%
  bootstrap (10.000 ricampionamenti, RNG deterministico, a blocchi per giorno quando ci sono ≥ 3
  giorni), costi in profilo competitive ed economy, esiti esclusi (unknown, aborted, data_gap), K
  totale delle configurazioni. Tabella delle migliori e del controllo `V0_random` della stessa banda.
- Telegram (token e chat da env `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`): report ogni 6 ore e alle
  00:00 UTC; alert se il feed è fermo da 2 minuti o i mint dirty superano una soglia (max 1 alert ogni
  30 minuti per tipo).
- Riga di log di salute ogni 60 s.

## 8. Comandi
- `pumpbot run --config /config/pumpbot.toml` (default)
- `pumpbot probe --secs N` — solo feed e decoder, stampa statistiche (msg/s, ritardo, dedup, dirty).
- `pumpbot report …`
- `pumpbot replay --events data/events-*.db --config … --out replay.db` — rigioca il motore sugli eventi
  registrati (stesse regole, per esplorare parametri senza aspettare). Limite: copre solo le curve che
  erano watched.

## 9. Protocollo di valutazione (da sim-model §7)
- Almeno 2 settimane di dati. Walk-forward per giorni; **ultimi 7 giorni come holdout mai toccato**.
- Go/no-go per la Fase 2, sull'holdout con L = 3 e profilo competitive: limite inferiore dell'IC 95%
  dell'expectancy > 0, profit factor > 1,2, PnL > 0 anche senza il top 1% dei trade, ancora positivo con
  L = 6, e migliore del controllo `V0_random` della stessa banda.

## 10. Struttura del codice
```
src/
  main.rs        CLI e avvio
  config.rs      TOML, griglia → configurazioni
  types.rs       tipi condivisi (Pubkey [u8;32], CurveState, TradeEvent, …, Event enum)
  b58.rs         base58 (five8) e utilità pubkey
  decode/        logs.rs (stack invoke, Program data), events.rs (layout), accounts.rs (BondingCurve)
  math.rs        formule intere curva + AMM (u128/i128)
  feed/          ws.rs (connessione, ping, riconnessione, IPv4), subs.rs (gestione tier), rpc_http.rs
  state.rs       MarketState, catena, dirty, finalizzazione, feature
  engine/        strategy.rs (segnali), exec.rs (SlotExec, costi), positions.rs, mod.rs
  store.rs       thread writer SQLite
  report.rs      metriche, bootstrap
  notify.rs      Telegram (ureq, thread dedicato)
  replay.rs
tests/           test d'integrazione su fixtures reali
config/pumpbot.toml
Dockerfile, docker-compose.yaml, README.md (italiano)
```
