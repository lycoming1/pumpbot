# Sorgente dati in tempo reale per gli eventi pump.fun e PumpSwap: latenza, completezza e costo (misurati il 24 ott 2026)

## Summary
Ho misurato per circa 30 minuti l'RPC pubblico di Solana (logsSubscribe, programSubscribe, getBlock) e il feed gratuito di PumpPortal. Ho anche verificato sulla documentazione e sull'IDL attuali di pump.fun (aggiornati il 13 ottobre 2026) come vengono emessi gli eventi.
1) pump.fun e PumpSwap emettono ogni evento due volte: una nei log ("Program data:") e una come self-CPI nelle inner instruction. Su 25 blocchi completi i conteggi coincidono: 245/245 TradeEvent, 362/362 BuyEvent, 178/178 SellEvent. Quindi logsSubscribe vede tutti gli eventi, tranne i rari casi di "Log truncated" (circa 1 transazione su 10.000).
2) logsSubscribe sull'intero programma pump tramite RPC pubblico non è adatto al trading. Con commitment processed ogni messaggio arriva due volte e il ritardo cresce fino a 326 slot (circa 69 s). Con confirmed il ritardo mediano va da 1,1 a 8 s e il p90 arriva fino a 35 s. Ci sono anche disconnessioni con codice 1002.
3) Le sottoscrizioni con poco traffico, invece, sono veloci anche sull'RPC pubblico. programSubscribe filtrato sul discriminatore BondingCurve pesa 22 kB/s con ritardo mediano di circa 0,6 s. logsSubscribe sulla singola curva ha un ritardo inferiore a uno slot. PumpPortal subscribeNewToken (gratuito) è sostanzialmente in tempo reale.
4) Lo slot oggi dura circa 212 ms, non 400. I volumi stimati sono circa 4 milioni di trade al giorno sulla bonding curve e circa 9 milioni su PumpSwap.
5) Il flusso trade a pagamento di PumpPortal costerebbe circa 4 SOL al giorno (circa 400 $) per seguire tutti i trade sulla curva: da escludere.
Raccomandazione per la fase 1 (costo zero): RPC pubblico con architettura a livelli (stato di tutte le curve via programSubscribe, log di dettaglio solo per i token candidati, PumpPortal gratuito per creazioni e migrazioni) e simulazione basata sul tempo degli eventi (slot). Fallback: piano Free di Helius. Per la fase 2: Yellowstone gRPC in Europa, a circa 40-200 $/mese, scelto con un confronto di una settimana tra Triton (pagamento a consumo) e Solana Vibe Station.

## Spec
# Sorgente dati in tempo reale per pump.fun e PumpSwap: specifica per la fase 1 e la fase 2

Data delle misure: 24 ottobre 2026, dalle 13:56 alle 14:25 UTC, dalla VM kraken (connessione residenziale italiana, download misurato 10,7 MB/s, quindi la banda non è il collo di bottiglia). Script e risultati grezzi sono in `/tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/datasource/`.

## 0. Fatti di base verificati, che cambiano il design

| Fatto | Valore misurato o verificato | Conseguenza |
|---|---|---|
| Durata dello slot | 212 ms (240 slot in 51,1 s; i performance samples danno 208-214 ms/slot). In quei campioni `numTransactions == numNonVoteTransactions`, quindi le transazioni di voto non compaiono più | I ritardi espressi in slot vanno convertiti a 0,212 s/slot. Circa 407.000 slot al giorno |
| Program ID | pump `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`, PumpSwap `pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA`, programma fee `pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ`, event authority di pump `Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1` | |
| Emissione degli eventi | Doppia: `emit!` nei log come `Program data: <base64>` e `emit_cpi` come inner instruction (tag di 8 byte `e445a52e51cb9a1d` seguito dallo stesso payload). Verificato su una transazione v3: inner instruction di 413 byte = 8 di tag + 405 di TradeEvent, identica alla riga di log | logsSubscribe è sufficiente per gli eventi. gRPC e transactionSubscribe danno in più la via CPI, che non si perde con il troncamento dei log |
| Completezza dei log (verità da 25 blocchi finalized, slot 460881070-460881094) | log = CPI per tutti i tipi: TradeEvent 245/245, CreateEvent 8/8, BuyEvent 362/362, SellEvent 178/178 | |
| Log troncati | 2 su 9.800 transazioni uniche (processed) e 1 su 15.742 (confirmed): circa 0,01-0,02% | Solo con i log si perde circa una transazione su 10.000 (se contiene eventi) |
| Versione delle transazioni | Su mainnet esistono transazioni **versione 1**. `getTransaction` fallisce senza `maxSupportedTransactionVersion: 1` | In Rust servono crate aggiornate (solana-transaction 6.x, yellowstone-grpc-proto 14.x) |
| Istruzioni di trade viste | `BuyExactQuoteInV3` (la più frequente), `Sell`, `Buy`, `SellV3`, `BuyExactQuoteInV2`, `BuyExactSolIn`, `SellV2`, `BuyV2`, più `multi_hop_swap` via PumpSwap e router di terzi (Jupiter, `FLAS…`, `6Vo3…`) | Non filtrare per istruzione: decodificare il TradeEvent |
| `ix_name` dentro il TradeEvent | Sempre il nome base (`buy_exact_quote_in`, `sell`, `buy`, `buy_exact_sol_in`, `multi_hop_swap`), anche per le istruzioni V3. La documentazione dice `_v3`, ma i dati osservati no | Non usare `ix_name` per capire la versione |
| Quote mint diverso da SOL | 73-76 curve attive su 249-266 (circa il 29%) hanno `quote_mint` diverso da default/WSOL (USDC o pump coin, fino a profondità 2) | Il filtro "liquidità > 5 SOL" va applicato solo alle curve quotate in SOL, o convertito |
| Fee dentro gli eventi | Coppie (fee protocollo bps, creator bps) più comuni: (95, 30) in 99 casi, (0, 0) in 53, (95, 300) in 16, (95, 100) in 7. Il valore `fee_basis_points=0` compare soprattutto con le istruzioni legacy `Buy`/`Sell` | Le fee per il paper trading vanno ricavate dalla configurazione (Global, programma pfee `GetFeesWithQuoteMint`), non copiate dall'evento. Da verificare nel filone sulle fee |
| Migrazione sintetica (dal 7 ott 2026) | Il buy v3 che completa la curva emite TradeEvent, poi CompleteEvent, poi PostCompleteBuyEvent (`6fb06d8b316cd5fb`) | Il totale dell'acquirente è la somma di TradeEvent e PostCompleteBuyEvent |

### Layout utili

**Eventi** (borsh, dopo il discriminatore di 8 byte):
- pump: TradeEvent `bddb7fd34ee661ee`, CreateEvent `1b72a94ddeeb6376`, CompleteEvent `5f72619cd42e9808`, PostCompleteBuyEvent `6fb06d8b316cd5fb`, CompletePumpAmmMigrationEvent `bde95db95c94ea94`
- PumpSwap: BuyEvent `67f4521f2cf57777`, SellEvent `3e2f370aa503dc2a`, CreatePoolEvent `b1310cd2a076a774`
- Prefissi base64 per un prefiltro veloce sulle stringhe di log: TradeEvent `vdt/007mYe4`, BuyEvent `Z/RSHyz1d3c`, SellEvent `Pi83CqUD3Co`

**Offset del TradeEvent** (byte, discriminatore incluso): mint 8, sol_amount 40, token_amount 48, is_buy 56, user 57, timestamp 89, virtual_sol 97, virtual_token 105, real_sol 113, real_token 121, fee_recipient 129, fee_bps 161, fee 169, creator 177, creator_fee_bps 209, creator_fee 217, track_volume 225, total_unclaimed 226, total_claimed 234, current_sol_volume 242, last_update_ts 250, ix_name (u32 di lunghezza + byte) 258. Seguono: mayhem_mode, cashback_bps, cashback, buyback_bps, buyback_fee, shareholders (vec), quote_mint, quote_amount, virtual_quote, real_quote, holder_rewards_bps, holder_rewards, creator_fee_unclaimed. Dimensioni osservate: 390-405 byte.

**Account BondingCurve** (166 byte, discriminatore `17b7f83760d8ac60`, in base58 `4y6pru6YvC7`): virtual_token u64 @8, virtual_quote u64 @16, real_token u64 @24, real_quote u64 @32, supply u64 @40, complete u8 @48, creator @49, is_mayhem @81, is_cashback @82, quote_mint @83..115, creator_fee_bps u64 @115, can_edit @123, is_holder_reward @124, creator_fee u64 @125, protocol_fees u64 @133, depth u8 @141, initial_virtual_quote u64 @142, post_complete_base_out u64 @150, post_complete_quote_in u64 @158.

**Progresso** = 1 − real_token / initial_real_token, con initial_real_token letto dall'account Global (standard 793.100.000 × 10^6; le monete mayhem hanno parametri propri).

Nota per il filone strategia: sulla curva standard il 5% di progresso corrisponde a circa 1,15 SOL reali e il 30% a circa 8,55 SOL. Il filtro "liquidità > 5 SOL" impone quindi già un progresso di almeno il 19% circa: la fascia effettiva è 19-30%, non 5-30%.

## 1. Le opzioni nel 2026

| Opzione | Costo | Limiti | Latenza attesa o misurata | Inner instruction (self-CPI)? |
|---|---|---|---|---|
| **logsSubscribe sull'RPC pubblico** (`wss://api.mainnet-beta.solana.com`), mentions = programma pump | 0 € | 100 richieste/10 s per IP, 40 connessioni, 100 MB/30 s. "Non pensato per la produzione", l'IP può essere bloccato senza preavviso | **Misurato**: confirmed con mediana 1,1-8,4 s e p90 4-35 s. Processed degrada (ritardo fino a 69 s, ogni messaggio doppio) | **No**: solo log, ma gli eventi ci sono perché pump emette anche via log |
| **programSubscribe sull'RPC pubblico**, filtro memcmp sul discriminatore BondingCurve | 0 € | Come sopra | **Misurato**: confirmed 22 kB/s, ritardo mediano 3 slot (circa 0,6 s), p90 10 (circa 2,1 s), massimo 16 | No: dà lo stato delle curve, non gli eventi. Con confirmed c'è un solo aggiornamento per curva per slot |
| **logsSubscribe sull'RPC pubblico**, mentions = singola bonding curve | 0 € | Numero massimo di sottoscrizioni non documentato | **Misurato**: 2,9 tx/s, 8 kB/s, arriva entro lo slot (ritardo ≤ 0 slot rispetto al tip di slotSubscribe) | No: solo log |
| PublicNode / Allnodes (`wss://solana-rpc.publicnode.com`) | 0 € | | **Misurato**: la sottoscrizione viene accettata ma in 75 s non arriva nessuna notifica | Inutilizzabile con il filtro mentions |
| dRPC, Ankr, OnFinality pubblici | | **Misurato**: dRPC "chain is not available on free plan", Ankr richiede una chiave, OnFinality risponde 429 | | |
| **Helius Free** | 0 $ | 1M crediti/mese. Il traffico WSS è contato dal 1° maggio 2026 a 20 crediti/MB, quindi circa 50 GB/mese. 5 connessioni WS, 1.000 sottoscrizioni per connessione. Solo WSS standard, niente Enhanced né gRPC su mainnet | Più veloce del pubblico secondo Helius (WSS su LaserStream, "fino a 200 ms in meno"); non misurato, serve una chiave | No con logsSubscribe |
| Helius Developer | 49 $/mese | 10M crediti (circa 500 GB/mese). Enhanced WS `transactionSubscribe` (filtri `failed:false`, `vote:false`, `accountInclude`), al massimo 100 sottoscrizioni transactionSubscribe per connessione | | **Sì** con transactionSubscribe (`meta.innerInstructions`, `logMessages`; `maxSupportedTransactionVersion: 1`) |
| Helius Business / Professional | 499 / 999 $/mese | LaserStream gRPC su mainnet (10 / 100 connessioni), 100M / 200M crediti | Fascia alta | Sì |
| **Chainstack** Growth + Yellowstone | 49 $ + 49 $ = **98 $/mese** (2 stream; 149 $ per 7 stream) | Regioni Solana US East, Frankfurt, Tokyo. Conteggio della banda non dichiarato | Non misurato | Sì (gRPC) |
| **Solana Vibe Station** | gRPC Basic **150 $/mese**, oppure **15 $/giorno o 40 $/settimana** (5 connessioni). Shred stream da 250 $/mese ad Amsterdam | Regioni Amsterdam e Atlanta | Non misurato | Sì (gRPC) |
| **Shyft** Build | 199 $/mese, "nessun credito e nessun limite di banda", 10 connessioni gRPC, RabbitStream (shred) incluso | Regioni non indicate sulla pagina | Non misurato | Sì |
| **Triton One** (Dragon's Mouth) | Pagamento a consumo **0,08 $/GB** in streaming, deposito prepagato minimo **125 $** valido 12 mesi. Shred streaming a Frankfurt e Amsterdam: 1.500 $/mese | Infrastruttura degli autori di Yellowstone | Non misurato | Sì |
| Alchemy | gRPC a consumo **75 $/TB**, non disponibile sul piano free. WS conteggiato a byte | | Non misurato | Sì (gRPC) |
| QuickNode | gRPC incluso nel piano Scale (499 $/mese), oppure add-on da 499 $. 10 crediti per 0,1 MB | | | Sì |
| **PumpPortal** `subscribeNewToken` e `subscribeMigration` | **Gratis** | Una sola connessione WS ("PLEASE ONLY USE ONE WEBSOCKET CONNECTION"), ban di un'ora per chi apre molte connessioni | **Misurato**: in pratica tempo reale (vedi §2.5) | No (JSON già elaborato) |
| PumpPortal `subscribeTokenTrade` e `subscribeAccountTrade` | 0,01 SOL ogni 10.000 messaggi, chiave API e wallet con almeno 0,02 SOL | Una sottoscrizione per mint | Simile a sopra (non misurato) | No |
| Jito ShredStream | **Dismesso il 5 settembre 2026** (secondo la documentazione Jito); il sostituto indicato è DoubleZero Edge, pagato in USDC con prezzi non pubblici | Non supportava comunque il NAT | | Gli shred non contengono risultati di esecuzione né eventi |
| WebSocket del frontend pump.fun | | Non documentati; la richiesta alla homepage riceve un 403 di Cloudflare | Non verificabile | Sconsigliati |

## 2. Misure (python3 + websockets 17.3)

### 2.1 logsSubscribe sull'intero programma pump, RPC pubblico

| Esecuzione | Durata | msg/s grezzi | kB/s | tx uniche/s | Duplicati | err ≠ null (uniche) | Con "Program data" | Ritardo in slot rispetto al tip (p50 / p90 / max) | Ritardo dalla notifica di slot (p50 / p90) | Disconnessioni |
|---|---|---|---|---|---|---|---|---|---|---|
| processed (prova) | 21 s | 111 | 158 | | 49% | 79% | 20% | 32 / 47 / 52 | 7,2 s / 11,0 s | 0 |
| processed | 121 s | 160 | 234 | 81 | **49,5%** (ogni tx due volte) | 77,7% | 19,4% | **131 / 317 / 326** (circa 28 / 67 / 69 s) | 27,0 s / 66,1 s | 1 (WS degli slot, codice 1002) |
| processed, 2 connessioni (probe leggero) | 60 s | 133 + 187 | 216 + 293 | 69 + 95 | 48% | 74-75% | | 60-90 / 128-160 / 169 | 13-18 s / 26-33 s | 1 (1002) |
| confirmed | 121 s | 130 | 202 | 130 | 0,08% | 74,8% | 24,2% | 44 / 151 / 212 | 8,4 s / 34,7 s | 0 |
| confirmed, probe leggero (periodo calmo) | 76 s | 63 | 214 | 63 | 0,3% | 27,2% | | **5 / 19 / 28** | **1,1 s / 4,0 s** | 0 |
| confirmed | 76 s | 150 | 297 | 150 | 0,07% | 66,9% | | 26 / 45 / 54 | 5,6 s / 11,5 s | 0 |

- La quota di transazioni fallite (bot che sbagliano) oscilla tra il 27% e il 78% a seconda del momento. Le transazioni riuscite sono stabili intorno a 10 per slot.
- Il ritardo **non dipende dal client**: un probe minimale (str.find, senza parsing JSON) ha gli stessi ritardi, e due connessioni in parallelo ricevono 509 kB/s in totale. Il collo di bottiglia è il pubsub dell'RPC pubblico sui filtri con molto traffico.
- Dentro le stesse transazioni circa il 40% delle righe "Program data" viene da **altri** programmi (MAyh…, MNFS…, GMGN…, whirlpool, Jupiter). Per attribuire correttamente un evento serve lo stack degli invoke.
- Il messaggio di log medio pesa 4,1 kB per una transazione riuscita e 0,75 kB per una fallita.
- Copertura in tempo reale: a confirmed, quando il ritardo resta stabile, si ricevono 43,6 TradeEvent/s contro 46/s della verità da blocchi, cioè circa il 95%. Con processed nel run da 121 s si sono coperti solo 208 slot su circa 570 prodotti: il backlog cresce.

### 2.2 logsSubscribe su PumpSwap, RPC pubblico

| Commitment | msg/s | kB/s | tx uniche/s | Duplicati | err | Ritardo in slot (p50 / p90 / max) | Ritardo dalla notifica di slot (p50) | Disconnessioni |
|---|---|---|---|---|---|---|---|---|
| processed, 90 s | 124 | 321 | 65 | 48% | 24,9% | 79 / 158 / 189 | 17,3 s | 1 (1002) |
| confirmed, 76 s | 103 | **392** | 103 | 0% | 18,6% | 72 / 128 / 156 | 15,4 s | 1 (1002) |

### 2.3 programSubscribe(pump) con memcmp offset 0 = `4y6pru6YvC7` (BondingCurve), RPC pubblico

| Commitment | msg/s | kB/s | Ritardo in slot (p50 / p90 / max) | Curve distinte attive | Note |
|---|---|---|---|---|---|
| confirmed, 45 s | 37 | 21 | 2 / 5 / 10 | 219 | Dati da 166 byte |
| confirmed, 60 s | 38 | 22 | 3 / 10 / 16 | 266 | Nessuna ripetizione stesso slot/curva. Fasce: <5%: 90, 5-30%: 40, >30%: 57, **5-30% e >5 SOL: 9**, quote non SOL: 76, complete: 3 |
| processed, 60 s | 70 | 40 | 2 / 6 / 9 | 249 | **Ogni notifica arriva due volte, identica** (1.872 ripetizioni su 1.872): processed raddoppia la banda senza informazioni in più |

### 2.4 logsSubscribe su una singola bonding curve molto attiva (confirmed, 2 hostname in parallelo, 45 s)

2,9 tx/s, 8,3 kB/s, err 6%. Rispetto al tip di slotSubscribe il ritardo in slot è p50 −11/−12, p90 da −1 a 0, max +2: i log arrivano prima della notifica di slot dello stesso RPC. Fra `api.mainnet-beta` e `api.mainnet.solana.com` la differenza di arrivo è p50 0 ms, p10/p90 ±210 ms.

### 2.5 PumpPortal gratuito (subscribeNewToken + subscribeMigration), 2 run da 150 s

- Run 2: 42 create in 150 s (0,28/s). Come riferimento uso un "oracolo del tip": getSlot processed su PublicNode, api.mainnet-beta e Tatum ogni 300-500 ms, più slotSubscribe. Rispetto al primo istante in cui l'oracolo vede lo slot, PumpPortal arriva a **p10 −412 ms, p50 −16 ms, p90 +186 ms, max +327 ms**: in pratica in tempo reale, entro circa uno slot. Campione piccolo: 19 create abbinati per slot.
- Sugli stessi create PumpPortal arriva **in mediana 19,8 s prima** di logsSubscribe confirmed sull'RPC pubblico (p90: 1,6 s prima). Nel run 1 il margine era di 7,0 s in mediana.
- Campi del messaggio create: signature, mint, traderPublicKey, txType, initialBuy, solAmount, bondingCurveKey, vTokensInBondingCurve, vSolInBondingCurve, marketCapSol, name, symbol, uri, is_mayhem_mode, pool ("pump").

### 2.6 Verità da blocchi (25 slot finalized consecutivi)

| Grandezza | Per slot | Al secondo (×4,7) |
|---|---|---|
| tx totali nel blocco | 873 | |
| tx che citano pump | 35,6 | 167 |
| tx che citano PumpSwap | 105,8 | 497 |
| di cui riuscite (pump + PumpSwap) | 70,5 | 331 |
| TradeEvent pump | 9,8 | **46** |
| BuyEvent + SellEvent PumpSwap | 14,5 + 7,1 | **101** |
| CreateEvent | 0,32 | 1,5 (campione corto: con le misure lunghe sono 0,17-0,3/s) |
| JSON completo delle tx pump/PumpSwap | 1,44 MB (10,2 kB/tx) | 6,8 MB/s |

## 3. Volumi giornalieri e storage per 2 settimane

**Eventi al giorno** (circa 407.000 slot al giorno):
- Trade sulla bonding curve: circa **4,0 milioni al giorno** (9,8/slot; controprova 43,6/s × 86.400 = 3,8 milioni). Intervallo prudente 3-5 milioni.
- Trade su PumpSwap: circa **9 milioni al giorno** (21,6/slot, un solo campione da 25 slot: ±50%).
- Create: **15.000-26.000 al giorno** (0,17-0,3/s).

**Banda grezza** (da non salvare):
- logs del programma pump, confirmed: 200-300 kB/s, cioè 17-26 GB al giorno
- logs PumpSwap: almeno 390 kB/s, cioè almeno 34 GB al giorno
- programSubscribe confirmed: 22 kB/s, cioè 1,9 GB al giorno
- logs per singola curva: circa 8 kB/s per token caldo

**Storage decodificato in SQLite** (WAL, insert a batch, mint e trader mappati su id interi, firma in BLOB da 64 byte; circa 140 B a riga indice compreso):

| Contenuto | Al giorno | 14 giorni |
|---|---|---|
| Trade bonding curve (necessari) | circa 0,56 GB | **circa 8 GB** |
| Archivio facoltativo dei payload TradeEvent grezzi (circa 400 B, zstd circa 3×) | circa 0,5 GB | circa 7 GB |
| Snapshot delle curve da programSubscribe (facoltativi, circa 60 B a riga, 3,3M al giorno) | circa 0,2 GB | circa 3 GB |
| Tutti i trade PumpSwap (facoltativi; consigliato solo per i pool dei token in portafoglio) | circa 1,2 GB | circa 17 GB |

Totale: **8 GB** nella versione minima, **30-35 GB** con tutto. La VM ha un disco da 200 GB, quindi va bene. Schema consigliato: tabella `trades(slot, tx_idx, ev_idx, sig BLOB, mint_id, trader_id, is_buy, quote_amt, token_amt, v_quote, v_token, r_quote, r_token, fee, creator_fee, ts)` con chiave primaria (slot, tx_idx, ev_idx), indice (mint_id, slot) e partizione giornaliera (un file per giorno), così la rotazione è semplice.

**Costo di PumpPortal a pagamento** per avere tutti i trade sulla curva: 4 milioni × 0,01 / 10.000 = **4 SOL al giorno**, circa 405 $ o 362 € al giorno (SOL a 101,35 $ / 90,46 €), cioè circa 11.000 € al mese. Anche limitandosi ai candidati (9-12 curve al minuto in fascia, circa 1,5 milioni di messaggi al giorno) sarebbero circa 1,5 SOL al giorno. **Da escludere.**

## 4. Architettura consigliata per la fase 1 (costo zero)

Si usa l'RPC pubblico, ma solo con sottoscrizioni a basso traffico per le decisioni, più un registratore completo che può permettersi di essere in ritardo.

1. **Livello A, universo delle curve**: `programSubscribe` confirmed:
   `{"jsonrpc":"2.0","id":1,"method":"programSubscribe","params":["6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P",{"encoding":"base64","commitment":"confirmed","filters":[{"memcmp":{"offset":0,"bytes":"4y6pru6YvC7"}}]}]}`
   Da qui per ogni curva: progresso, SOL reali, quote_mint, complete e flusso netto per slot (delta delle riserve). Pesa 22 kB/s con ritardo di circa 0,6 s. Non usare processed (doppioni).
2. **Livello B, dettaglio sui candidati**: quando una curva quotata in SOL supera circa il 3% (prima della fascia), si apre `logsSubscribe {"mentions":["<bonding_curve_pubkey>"]}` confirmed. Così si hanno i TradeEvent esatti (trader, importi, conteggio acquisti/vendite) con ritardo inferiore a uno slot. Si chiude la sottoscrizione all'uscita dalla fascia, a posizione chiusa o a curva completa. Limite prudente: 50 sottoscrizioni distribuite su 2-3 connessioni, con backoff (il limite reale dell'RPC pubblico non è documentato).
3. **Livello C**: PumpPortal `subscribeNewToken` e `subscribeMigration` su **una sola** connessione, per i metadati di creazione e le migrazioni in tempo reale. Serve anche come verifica di completezza.
4. **Registratore**: `logsSubscribe {"mentions":[PUMP]}` confirmed per l'archivio completo dei trade (backtest offline e statistiche "più acquisti che vendite"). Il ritardo qui non conta.
5. **Simulazione sul tempo degli eventi**: il paper trader decide allo slot S dell'evento ed esegue l'ingresso sullo stato della curva allo slot S + d. Il parametro d va calibrato sulla fase 2: per esempio 2-5 slot, cioè 0,4-1,1 s a 212 ms per slot. Così i risultati non sono falsati dal ritardo del feed. Le uscite (take profit, stop loss, trailing) si valutano allo stesso modo, sullo slot dell'evento più il ritardo.
6. **Requisiti del client Rust**:
   - deduplicare per (signature, indice dell'evento)
   - scartare `err != null`
   - attribuire i "Program data" solo quando in cima allo stack degli invoke c'è pump o PumpSwap
   - riconnessione automatica e nuova sottoscrizione (il codice 1002 compare ogni pochi minuti)
   - rilevare i buchi tramite la continuità degli slot
   - parsing JSON veloce (sonic-rs o simd-json, borrowed) e decoder borsh scritto a mano dall'IDL

**Fallback per la fase 1: Helius Free.** Usa gli stessi metodi WS standard; la chiave la crea l'utente.
- programSubscribe confirmed pesa circa 1,9 GB al giorno, cioè 38.000 crediti al giorno: con 1M crediti al mese basta per circa 26 giorni.
- Il registratore completo (17-26 GB al giorno) esaurirebbe il piano in 2-3 giorni, quindi in modalità fallback va spento.
- Fallback secondario: l'hostname `api.mainnet.solana.com`. Probabilmente è la stessa infrastruttura e aiuta solo per problemi di DNS o di edge.

**Alternativa a basso costo** se l'RPC pubblico si rivela inaffidabile (ban, 1002 continui): Triton a consumo. Con un filtro gRPC `account_include=[pump]`, `failed=false`, `vote=false` si stimano circa 42-56 tx riuscite al secondo × circa 3-5 kB (protobuf, **stima**), cioè circa 15-19 GB al giorno, quindi circa 36-46 $ al mese. Il prepagato da 125 $ copre circa 3 mesi. In alternativa Chainstack a 98 $/mese, a canone fisso, con regione Frankfurt.

## 5. Fase 2 (soldi veri)

**Sorgente primaria**: Yellowstone gRPC con commitment processed in Europa (Frankfurt o Amsterdam), configurato così:
- filtro transazioni `account_include=[pump]`, `failed=false`, `vote=false`
- PumpSwap solo per i pool dei token che si hanno in portafoglio
- filtro account con owner = pump e memcmp sul discriminatore BondingCurve

Si usano gli eventi CPI delle inner instruction, che restano completi anche quando i log vengono troncati.

| Candidato | Costo mensile | Pro e contro |
|---|---|---|
| Triton a consumo | circa 40-50 $ (solo pump), circa 200 $ con tutto PumpSwap | Qualità di riferimento, paghi quel che usi |
| Chainstack | 98 $ | Il canone fisso più basso, regione Frankfurt; non è chiaro se la banda venga conteggiata |
| Solana Vibe Station | 150 $ (prova a 40 $/settimana) | Regione Amsterdam |
| Shyft Build | 199 $ | Senza conteggio di banda, RabbitStream incluso |
| Helius Business / QuickNode Scale | 499 $ | Fuori budget per 0,5-1 SOL di capitale |

**Procedura**: fare un confronto di una settimana (SVS a 40 $ più Triton con 125 $ prepagati) misurando il tempo fino all'evento rispetto a PumpPortal gratuito sui create e fra i due provider, poi scegliere. Tenere l'RPC pubblico (livelli A e B) e Helius Free come fallback. Budget realistico: **50-150 $ al mese**.

Gli shred (SVS 250 $/mese, Triton 1.500 $/mese, DoubleZero Edge) contengono transazioni non ancora eseguite, senza eventi né esito. Hanno senso solo se la fase 1 dimostra che il vantaggio si gioca su frazioni di slot.

## 6. Endpoint e crate

**Endpoint**:
- `wss://api.mainnet-beta.solana.com` e `https://api.mainnet-beta.solana.com` (getSlot, getSignatureStatuses, `getTransaction` con `maxSupportedTransactionVersion:1`)
- `wss://pumpportal.fun/api/data` (`{"method":"subscribeNewToken"}`, `{"method":"subscribeMigration"}`)
- Helius: URL WSS dal dashboard, con API key

**Crate** (versioni attuali su crates.io; per la supply chain fissarle a versioni pubblicate da almeno 2 settimane):

| Crate | Versione | Ultimo aggiornamento |
|---|---|---|
| yellowstone-grpc-client | 16.0.0 | 20 ott 2026 |
| yellowstone-grpc-proto | 14.0.1 | |
| helius-laserstream | 1.0.1 | |
| tokio-tungstenite | 0.30.0 | |
| sonic-rs | 0.5.10 | |
| rusqlite | 0.41.0 | pubblicata oggi: usare la precedente |
| borsh | 1.8.1 | |
| solana-transaction | 6.0.0 | |
| pump-rust-client (ufficiale) | 0.5.0 | 19 ott 2026 |

Su pump-rust-client: la documentazione cita ancora la 0.4.1, quindi l'IDL può essere cambiato di nuovo. Va letto e non eseguito senza revisione: conviene un decoder scritto a mano a partire da `idl/pump.json`.

## Recommendations
- Fase 1 primaria a costo zero, RPC pubblico a livelli: programSubscribe(pump, memcmp offset 0 = '4y6pru6YvC7', confirmed) per lo stato di tutte le curve (22 kB/s, ritardo circa 0,6 s), logsSubscribe sulla singola bonding curve solo per i candidati (ritardo inferiore a uno slot), PumpPortal gratuito (subscribeNewToken e subscribeMigration) su una sola connessione.
- Non basare le decisioni su logsSubscribe dell'intero programma via RPC pubblico: ritardo misurato da 1 a 69 s. Usarlo solo come registratore d'archivio confirmed. Non usare mai processed sull'RPC pubblico: ogni notifica (log e account) arriva due volte e il backlog cresce.
- Fare il paper trading sul tempo degli eventi: ingresso e uscite allo slot dell'evento più d slot (d calibrato sulla fase 2, per esempio 2-5 slot da 212 ms), così il ritardo del feed non falsa il P&L.
- Usare 212 ms per slot (misurati), non 400 ms, in tutti i modelli di ritardo e nei time stop.
- Nel decoder Rust: deduplicare per (signature, indice dell'evento), scartare err != null, attribuire i 'Program data' tramite lo stack degli invoke (circa il 40% delle righe Program data viene da altri programmi), supportare le transazioni v1, gestire la riconnessione dopo i codici 1002 e rilevare i buchi tramite la continuità degli slot.
- Filtrare quote_mint: circa il 29% delle curve attive non è quotato in SOL. Comunicare al filone strategia che 'liquidità > 5 SOL' implica già un progresso di almeno il 19% circa (fascia effettiva 19-30%).
- Ricavare le fee del paper trading dalla configurazione on-chain (Global, programma pfee), non dai campi fee del TradeEvent: in circa il 30% degli eventi fee_basis_points vale 0 (soprattutto con le istruzioni Buy/Sell legacy).
- Fallback per la fase 1: piano Helius Free (chiave da creare dall'utente), con solo i livelli A e B (circa 38.000 crediti al giorno); spegnere il registratore completo quando è attivo il fallback.
- Escludere PumpPortal a pagamento per i trade: circa 4 SOL al giorno (circa 400 $) per tutti i trade della curva, circa 1,5 SOL al giorno anche solo per i candidati.
- Fase 2: Yellowstone gRPC processed in Europa con failed=false, usando gli eventi CPI delle inner instruction. Fare prima un confronto di una settimana tra Triton a consumo (125 $ prepagati, circa 40-50 $/mese solo pump) e SVS (40 $/settimana), con Chainstack (98 $/mese, Frankfurt) come alternativa a canone fisso. Budget 50-150 $ al mese.
- Storage: SQLite in WAL con un file per giorno, trade della curva in forma compatta (circa 8 GB per 14 giorni). Trade PumpSwap solo per i pool dei token in portafoglio; archivio grezzo zstd facoltativo (circa 7 GB).

## Measured
Tutte le misure sono del 24 ottobre 2026 (13:56-14:25 UTC) dalla VM kraken, con python3 e websockets 17.3. Sono numeri miei, non presi da documentazione.

- **Slot e verifiche di base**: 240 slot in 51,1 s, cioè 213 ms/slot; i performance samples danno 208-214 ms/slot e numTransactions == numNonVoteTransactions. Download 10,7 MB/s.
- **logsSubscribe pump, processed, 121 s**: 160 msg/s, 234 kB/s, 49,5% di duplicati, 81 tx uniche/s, 77,7% err, 19,4% con Program data. Ritardo in slot p50/p90/max 131/317/326; dalla notifica di slot p50 27 s, p90 66 s. Coperti solo 208 slot su circa 570; una disconnessione 1002.
- **logsSubscribe pump, confirmed**:
  - 121 s: 130 tx/s, 202 kB/s, 74,8% err, ritardo p50 44 slot (8,4 s), p90 151 slot (34,7 s)
  - 76 s in un periodo calmo: 63 tx/s, 27% err, ritardo p50 5 slot (1,1 s), p90 19 slot (4,0 s)
  - 76 s: 150 tx/s, 297 kB/s, 67% err, ritardo p50 26 slot (5,6 s), 43,6 trade/s
- **processed con 2 connessioni**: circa 48% di duplicati, ritardo p50 60-90 slot.
- **PumpSwap**: processed 321 kB/s con 48% di duplicati e ritardo p50 79 slot; confirmed 392 kB/s, 18,6% err, ritardo p50 72 slot (15,4 s), una disconnessione 1002.
- **programSubscribe BondingCurve**:
  - confirmed: 37-38 msg/s, 21-22 kB/s, ritardo p50 2-3 slot, p90 5-10, massimo 16; 219-266 curve attive al minuto, di cui 9-12 nella fascia 5-30% con più di 5 SOL e 29% con quote non in SOL
  - processed: 70 msg/s, 40 kB/s, ripetizioni identiche al 100% (1.872 su 1.872)
- **logsSubscribe su una singola curva**: 2,9 tx/s, 8 kB/s, arriva entro lo slot.
- **PublicNode**: logsSubscribe accettato, 0 notifiche in 75 s. dRPC e Ankr richiedono piano o chiave, OnFinality risponde 429.
- **PumpPortal create**: 0,17-0,28 create/s. Latenza rispetto all'oracolo del tip p50 −16 ms, p90 +186 ms. In mediana arriva da 7 a 19,8 s prima di logsSubscribe confirmed pubblico.
- **getBlock su 25 slot**: per slot 9,8 TradeEvent, 14,5 BuyEvent + 7,1 SellEvent su PumpSwap, 0,32 create, 35,6 tx che citano pump, 105,8 tx che citano PumpSwap, 70,5 tx riuscite. Conteggi via log identici a quelli via CPI (245/245, 362/362, 178/178, 8/8).
- **Transazione v3 verificata**: TradeEvent presente sia nei log (405 B) sia come inner instruction self-CPI (413 B, tag e445a52e51cb9a1d). Transazione in versione 1.
- **Log troncati**: da 1 a 2 su circa 10-16.000 tx.
- **Prezzo SOL** (CoinGecko): 101,35 $ / 90,46 €.

Le cifre su protobuf gRPC e banda sui provider a pagamento sono **stime**, non misure.

## Open questions
- Limite reale di sottoscrizioni logsSubscribe per connessione e per IP sull'RPC pubblico (per il livello B): non documentato e non misurato oltre 2 connessioni.
- Latenza reale di Helius Free (WSS su LaserStream) e dei provider gRPC (Chainstack, SVS, Triton, Shyft) da una connessione residenziale italiana: non misurata perché serve una chiave o un pagamento. Da misurare con il confronto di una settimana.
- Dimensione reale di una transazione pump in protobuf gRPC (stimata 3-5 kB): determina il costo con Triton o Alchemy a consumo.
- Il volume PumpSwap (circa 9 milioni di trade al giorno) viene da un solo campione di 25 slot: va confermato con una misura più lunga.
- Perché circa il 30% dei TradeEvent (soprattutto Buy/Sell legacy) riporta fee_basis_points=0: va capito nel filone sulle fee, perché incide sul calcolo dei costi.
- Le transazioni fallite oscillano tra il 27% e il 78% nell'arco di minuti: va verificato se i picchi coincidono con i lanci più caldi, cioè con la concorrenza dei bot nella stessa fascia della strategia.
- Esiste anche una versione più recente di pump-rust-client (0.5.0 del 19 ottobre) rispetto a quella citata nella documentazione (0.4.1): ricontrollare l'IDL al momento dell'implementazione.
- Il Bitquery dei termini d'uso di PumpPortal e dell'RPC pubblico per un uso continuo 24/7: rischio di ban dell'IP di casa (che è anche l'IP di tutti i servizi del homelab).

## Sources
- https://github.com/pump-fun/pump-public-docs (README, docs/instructions/TRADE_V3.md, docs/SYNTHETIC_MIGRATION.md, docs/instructions/MULTI_HOP_SWAP.md, idl/pump.json, idl/pump_amm.json; ultimo commit 13 ott 2026)
- https://pumpportal.fun/data-api/real-time
- https://www.helius.dev/pricing
- https://www.helius.dev/docs/billing/credits
- https://www.helius.dev/docs/billing/rate-limits
- https://www.helius.dev/blog/laserstream-websockets
- https://www.helius.dev/docs/enhanced-websockets/transaction-subscribe
- https://chainstack.com/pricing/
- https://chainstack.com/yellowstone-grpc-more-streams-same-price/
- https://chainstack.com/learn/compare/best-yellowstone-grpc-providers-solana-2026/
- https://support.alchemy.com/articles/5952597732-how-is-solana-grpc-yellowstone-streaming-priced
- https://www.quicknode.com/blog/solana-grpc-is-now-included-with-scale-and-business-plans
- https://shyft.to/solana-rpc-grpc-pricing
- https://solanavibestation.com/pricing
- https://docs.solanavibestation.com/solana-core-services/geyser-grpc-streaming
- https://triton.one/pricing
- https://docs.jito.wtf/lowlatencytxnfeed/
- https://www.theblock.co/post/397715/doublezero-rolls-out-edge-a-low-latency-onchain-data-delivery-service-on-its-global-fiber-depin-infrastructure
- https://solana.com/docs/references/clusters
- https://coinpaprika.com/education/pumpfun-api-solana-dex-data/
- https://crates.io (yellowstone-grpc-client, yellowstone-grpc-proto, helius-laserstream, pump-rust-client, rusqlite, sonic-rs, tokio-tungstenite, borsh, solana-transaction)
- https://api.coingecko.com/api/v3/simple/price?ids=solana (prezzo SOL)
- Misure proprie: /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/datasource/ (measure.py, lagprobe.py, progsub.py, progdup.py, pp_probe.py, block_sample.py e i relativi *.json: pub_processed.json, pub_confirmed.json, pub_confirmed2.json, pub_pamm_processed.json, pub_pamm_confirmed.json, probe_confirmed.json, probe_processed_2conn.json, probe_curve_confirmed.json, progsub*.json, pp_probe*.json, block_sample.json, tx_v3.json)