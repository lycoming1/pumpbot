# Modello di simulazione realistico per il paper trading su pump.fun (latenza a slot, costi 2026, momentum, feature di manipolazione, uscite, metriche, studi pubblici)

## Summary
Il contesto di ottobre 2026 non è più quello del 2024, e ho misurato io i punti chiave. Gli slot durano 212 ms in media (target 200 ms dal 9 ottobre). Alpenglow risulta attivo: le transazioni di voto sono sparite e finalized è solo 2 slot dietro processed. Le fee sulla bonding curve, lette on-chain, sono 0,95% protocol più 0,30% creator, cioè 1,25% per lato (il buyback è metà della quota protocol, non si aggiunge). Su PumpSwap le fee vanno a scaglioni: 1,25% sotto 420 SOL di market cap, poi da 1,20% a scendere. Il rent di un ATA Token-2022 oggi è 0,00151 SOL, più basso dei 0,00204 SOL che si citano di solito.
Il websocket pubblico gratuito non regge: consegna il flusso pump.fun a circa il 28% della velocità reale, si disconnette ogni 1-3 minuti, accumula 30-45 s di ritardo e perde il 59% degli slot. Serve un flusso a pagamento.
Ho misurato anche quanto costa arrivare tardi su un segnale di momentum: il prezzo agli slot S+1..S+4 è in media il 2-4% sopra quello del trigger (mediana 0, coda p90 +16-26%). Il campione è piccolo (57 trigger per la variante principale).
Il costo di esecuzione dei trader reali ha mediana 0,000027 SOL e p90 0,001 SOL per transazione; il 38% paga una tip a un servizio di invio.
Il modello che propongo esegue il trade allo stato della curva alla fine dello slot S+L, con L configurabile (3 come default realistico per un bot domestico con Sender), più una variante pessimistica, il controllo dello slippage e i retry sulle vendite.
Per andare in pari la strategia deve guadagnare circa il 6-8% lordo per trade. Non esiste un backtest pubblico rigoroso di questa strategia. Gli studi accademici dicono che comprare e tenere fino alla graduation, entrando nella banda scelta, ha attesa negativa per un token medio.

## Spec
# Specifica: modello di simulazione per il paper trading su pump.fun (Fase 1)

Tutti i numeri senza la dicitura "stima" li ho misurati io il 24/10/2026 tra le 13:30 e le 14:30 UTC: RPC mainnet, account on-chain, flusso live di 20 minuti, campione di 300 transazioni. Le fonti esterne sono indicate.

---
## 0. Stato attuale che cambia il modello (misurato)

| Fatto | Valore | Come l'ho verificato |
|---|---|---|
| Durata slot | mediana **212,0 ms** (min 206, max 220) su 451 campioni da 60 s (~7,5 h); target 200 ms dal 9/10/2026 | `getRecentPerformanceSamples` |
| Alpenglow | transazioni di voto = 0 (non-vote/total = 0,99998-1,0); processed = confirmed; **finalized = processed − 2 slot** | perf samples + `getSlot` per commitment |
| Leader window | 4 slot consecutivi (≈ 850 ms) | `getSlotLeaders` |
| Client RPC pubblico | Agave 4.3.2, epoch 1066 | `getVersion` / `getEpochInfo` |
| Traffico programma pump | **~111 tx/slot ≈ 524 tx/s** che lo menzionano, 70-86% fallite (spam di bot "guard") | `getSignaturesForAddress` + stream |
| Trade riusciti | ~20,7 per slot osservato ≈ **98 trade/s** (tutte le quote, mayhem incluso) | stream |
| Create | 61/min nella finestra osservata. Estrapolato fa ~88k/giorno, ma è 4 volte i ~21k/giorno del 2025: va verificato con un provider | stream |
| Tipo di quote dei nuovi token | 71% SOL, 15% PUMP (`pumpCmXq…`), 3% USDC, il resto altri pump coin; quote_mint di depth massima 2 | CreateEvent |
| Mayhem | 28,5% dei create, 36% dei trade. Quasi tutti sono dell'agente `BwWK17cb…` (11.164 trade in 20 min, **fee 0**) | TradeEvent |
| Token program | 99,6% Token-2022 (create_v2) | CreateEvent |
| Holder rewards | 9,3% dei create | CreateEvent |

**Conseguenze per il bot:**
- **Universo della Fase 1:** solo `quote_mint == default (SOL)` e `mayhem_mode == false`. Il resto va registrato ma non tradato.
- **Flussi:** escludere sempre i trade dell'agente mayhem dalle feature di flusso.
- **Reserves:** non leggere le reserves dai lamports della curva. Con i trade v3 le fee protocol/creator restano sulla curva (`BondingCurve.protocol_fees`, `creator_fee`) finché qualcuno non fa lo sweep. Usare `real_quote_reserves`/`virtual_quote_reserves` del TradeEvent.

### 0.1 Costanti della curva (Global `4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf`, lette on-chain)
- `initial_virtual_token_reserves` = 1.073.000.000,000000 (raw 1,073e15, 6 decimali)
- `initial_virtual_sol_reserves` = 30 SOL
- `initial_real_token_reserves` = 793,1M
- supply = 1B
- `pool_migration_fee` = 0,015000001 SOL
- `buyback_basis_points` = 5000
- `max_configurable_creator_fee_bps` = 300 (vale solo per le coppie custom, cioè quote = pump coin)
- `initial_virtual_quote_reserves` = 4.292 USDC (coppie USDC)
- Verificato su 22.274 trade: `virtual_sol − real_sol = 30 SOL` nel 100% dei casi

Formule:
- k = 30 × 1.073e9 = 32.190 (SOL·Mtoken)
- progress x = 1 − real_token/793,1e12
- real SOL(x) = 32.190/(1.073 − 793,1·x) − 30
- prezzo p = v_sol/v_token
- mcap = p × 1e9

| progress | real SOL | mcap (SOL) | multiplo fino alla graduation |
|---|---|---|---|
| 5% | 1,151 | 30,2 | 13,6× |
| **19,33%** | **5,0** | 38,0 | 10,8× |
| 30% | 8,548 | 46,2 | 8,9× |
| 100% | 85,005 (vSOL 115,005) | 410,9 | 1× |

**Attenzione:** "liquidità > 5 SOL" più "progress 5-30%" significa in pratica **progress 19,3-30%** (real SOL 5-8,55, mcap 38-46 SOL). La banda è stretta. Va resa esplicita e parametrica.

---
## 1. Latenza e modello di esecuzione basato sugli slot

### 1.1 Componenti (L = slot tra il trade che fa scattare il segnale, nello slot S, e l'inclusione della nostra tx)
1. **Osservazione (Δobs)**
   - **Misurato:** il websocket pubblico `api.mainnet-beta` non è utilizzabile.
     - Con un consumer che non fa altro che ricevere: 151 msg/s, 0,27 MB/s. Copre 69 slot in 53 s contro i 248 reali (circa il 28% del tempo reale), poi si chiude con errore 1002.
     - In 20 minuti: 17 disconnessioni e 59,4% degli slot persi.
     - Ritardo rispetto al `timestamp` del blocco: mediana 7,7 s, in crescita fino a 30-45 s prima di ogni disconnessione.
     - È quasi certamente throttling lato server.
   - **Stima:** con gRPC Yellowstone/LaserStream (o simili) a processed, Δobs è circa 100-400 ms (0,5-2 slot). Va misurato nel bot.
2. **Decisione in Rust:** µs, trascurabile.
3. **Invio:**
   - casa → endpoint Sender a Francoforte/Amsterdam: ~15-25 ms (stima)
   - inoltro staked/Jito al leader: 10-150 ms a seconda di dove sta il leader (stima)
   - allineamento allo slot: in media mezzo slot
4. **Inclusione:** decide lo scheduler del leader (priority fee e asta BAM/Jito ogni 20 ms, fonte CryptoBriefing 13/10/2026), non l'ordine di arrivo. Gli slot saltati alzano la media a 212 ms invece di 200.

### 1.2 Profili di L proposti (stime da calibrare)

| Profilo | Setup | L (slot, 1 slot = 212 ms) | drop |
|---|---|---|---|
| A | casa + RPC/websocket pubblico | non praticabile (Δobs misurato ≥ 35 slot e crescente) | alto |
| **B (default)** | casa + flusso a pagamento + Helius Sender FRA/AMS (tip ≥ 0,001 SOL) | P(2)=0,25 · P(3)=0,40 · P(4)=0,20 · P(5)=0,07 · P(6)=0,03 | 5% |
| C | VPS a Francoforte + shred/gRPC + Sender/Jito | P(1)=0,20 · P(2)=0,50 · P(3)=0,20 · P(≥4)=0,10 | 2-3% |

Riferimento misurato: il primo acquisto di un wallet diverso dal creator arriva nello **stesso slot del create nel 68%** dei token SOL. Nel 50% dei create c'è almeno un wallet esterno nello slot del create, nel 38% almeno 3. Sono bundle legati al creator, e una strategia reattiva non li può battere.

**Calibrazione:** in Fase 2, prima di tradare, mandare 50-100 tx sonda (self-transfer o memo, tip minima) registrando il ms di invio e lo slot di inclusione.

### 1.3 Regola di fill (modello "SlotExec")
- `S` = slot del trade trigger. `S_exec = S + L_buy`, con L deterministico (per il grid di sensibilità L ∈ {1,2,3,4,6,10}) o campionato dal profilo.
- **Fill = stato della curva alla fine dello slot S_exec**: post-stato dell'ultimo trade dello slot, oppure l'ultimo stato noto se nello slot non ci sono trade.
- **Variante pessimistica `WorstInSlot`:** per un buy, il post-stato più caro tra i trade di S_exec; per un sell, il più basso.
- **Guard di slippage sul buy:** se `p(S_exec) > p_trigger × (1+slip_buy)` (default 15%) la tx fallisce.
  - Non si apre la posizione.
  - Si addebitano base più priority fee.
  - La tip non si addebita: se la tx fallisce, l'istruzione di tip fatta con Sender viene revertita, e un bundle Jito che non entra non paga.
  - Riferimento di mercato: circa il 15,6% dei tentativi altrui fallisce per slippage. Nello stream: 6042 BuySlippageBelowMinTokensOut = 5.998, 6002 = 401, 6003 = 174, contro 35.644 trade riusciti non dell'agente.
- **Drop** (tx mai inclusa): nessun costo, nessuna posizione.
- **Sell:** `S_exec_sell = S_trigger_uscita + L_sell` (default uguale al profilo), con `min_sol_output = LV_atteso × (1 − slip_sell)` (default 25%). Se fallisce: retry allo slot successivo (massimo 5), pagando le fee ogni volta.
- **Ordine dei trade nello slot:** si ricostruisce concatenando gli stati. Per un buy: `pre_vs = post_vs − sol_amount`, `pre_vt = post_vt + token_amount`; per un sell segni inversi.
  - Verificato: il pre-stato coincide col post-stato di un altro trade osservato nell'89,6% dei casi; il restante 10,4% corrisponde ai buchi dello stream.
  - Se la catena si rompe, il mint va marcato "dirty" e la simulazione su di esso sospesa: mancano eventi. Il websocket logs non fornisce l'indice della tx nel blocco, il gRPC sì.

### 1.4 Perché gli slot e non i millisecondi
1. Lo stato on-chain cambia a blocchi. Dentro un blocco l'ordine lo decide il leader (fee e asta), non l'arrivo. Un ritardo in ms ci metterebbe "tra" due trade dello stesso blocco, con una precisione che non esiste.
2. Gli eventi portano lo slot. Il `timestamp` ha risoluzione di 1 s. Il modello a slot si rigioca identico sia in live paper sia in replay.
3. Il passaggio da ms a slot non è lineare: leader window di 4 slot, slot saltati, confini a 212 ms. Modellare direttamente la distribuzione di L cattura tutto questo.
4. Eseguire a fine slot è una scelta prudente e deterministica: nei momentum il flusso nello stesso slot va nella nostra direzione, quindi paghiamo il suo impatto e non assumiamo di essere primi nel blocco.
5. Misurato in banda: tra due tx diverse consecutive sullo stesso mint il gap è 0 slot nel 65,7% dei casi, +1 nel 10,7%, ≥ 10 nel 10% (p90 9, p95 20, p99 72 slot). Il prezzo si muove quasi sempre prima che possiamo entrare.

### 1.5 Costo misurato del ritardo (indicativo, campioni piccoli)
Prezzo a fine slot S+L rispetto al post-stato del trigger, su finestre di stream senza buchi:

| Variante trigger | n | L=1 | L=2 | L=3 | L=4 | L=6 | L=10 |
|---|---|---|---|---|---|---|---|
| A (20 trade, buy > sell, netto ≥ 0,5 SOL) | 57 | +2,97% | +3,27% | +3,85% | +3,62% | +3,38% | +2,82% |
| B (20 trade, buy ≥ 1,5×sell, netto ≥ 1 SOL, ≥ 8 buyer unici, top3 < 60%) | 15 | +4,59% | +3,52% | +3,73% | +3,71% | +2,00% | −0,82% |
| C (25 slot, buy > sell, netto ≥ 1 SOL, ≥ 6 buyer) | 31 | +0,71% | +1,48% | +2,33% | +3,19% | +3,71% | +2,30% |

- Le mediane sono circa 0 e il p90 sta tra +16% e +26%: la coda è pesante.
- Il simulatore deve calcolare questo "drift avverso" su ogni segnale e per ogni L.

---
## 2. Costi per transazione (ottobre 2026)

| Voce | Valore | Note / fonte |
|---|---|---|
| Base fee | 5.000 lamport per firma | misurato (fee − priority) |
| Priority fee pagata (300 trade reali) | p10 2,1e-7 · p25 7,2e-6 · **p50 1,9e-5** · p75 1,0e-4 · p90 1,7e-4 · p95 3,0e-4 · p99 2,0e-3 SOL | il 93% imposta un CU price |
| CU consumate | p25 55k · **p50 72k** · p75 93k · p90 163k | |
| CU limit richiesto | p50 125k (limit/usate: mediana 1,44) | la priority si paga sul **limit**: impostarlo a ~1,15× le CU usate |
| CU price | p25 50k · p50 260k · p75 711k · p90 1,54M µlamport/CU | |
| Tip ai servizi di invio | il 38% dei trade ne paga una (Helius 45, Astralane 39, Jito 16, GMGN 16, Nozomi 5 su 300); importo p25 1e-5 · **p50 3e-5** · p75 2,1e-4 · p90 1e-3 · p99 6,1e-3 SOL | misurato |
| Tip floor Jito (tutti i bundle, 13:58 UTC) | p25 1,0e-6 · p50 4,5e-6 · p75 1,0e-5 · p95 4,3e-5 · p99 2,1e-4 SOL | `bundles.jito.wtf/api/v1/bundles/tip_floor` |
| Helius Sender | tip minima **0,001 SOL** per l'instradamento completo; 0,000005 SOL con `swqos_only=true`; priority fee obbligatoria; 50 rps con chiave, 1 rps senza; endpoint `fra-sender`/`ams-sender` | doc Helius |
| Costo totale osservato (base + priority + tip) | p25 1,6e-5 · **p50 2,7e-5** · p75 1,2e-4 · **p90 1,0e-3** · p95 1,2e-3 SOL | misurato |
| ATA Token-2022 (170 B) | **0,00151384 SOL** (rent attuale) | `getMinimumBalanceForRentExemption`; rent tagliato di circa il 27% |
| ATA SPL (165 B) | 0,00148844 SOL | idem; il vecchio valore era 0,00203928 |
| Recupero del rent | sì, con `closeAccount` a saldo 0: metterlo nella stessa tx del sell. È capitale bloccato, non un costo | |
| `user_volume_accumulator` (137 B) | 0,0013462 SOL una tantum per wallet (la doc dice ancora 0,0018444, cioè il rent vecchio) | chiudibile |
| **Fee della bonding curve** | **protocol 95 bps + creator 30 bps = 1,25% per lato**, uguale per ogni mcap (FeeConfig `8Wf5TiAh…`, un solo scaglione). Il buyback è il 50% della protocol, già incluso | on-chain. Osservato nel 99,3% dei trade SOL non mayhem `(95,30,5000)`, nello 0,7% `(95,0)` |
| Coppie custom (quote pump coin) | creator_fee_bps per curva, fino a 300 | va letto da ogni TradeEvent |
| **PumpSwap, pool canonica** (FeeConfig `5PHirr8j…`) | lp/protocol/creator: mcap < 420 SOL → 2/93/30 = **1,25%**; 420 → 20/5/95 = 1,20%; 1.470 → 1,15%; 2.460 → 1,10%; 3.440 → 1,05%; 4.420 → 1,00%; 9.820 → 0,95% … fino a 98.240 SOL → 0,30% | on-chain. Alla migrazione la pool parte da ~85 SOL / 206,9M token, mcap ~411 SOL, quindi primo scaglione |
| PumpSwap, pool non canonica | flat 25/5/0 = 0,30% | on-chain |
| Fee di piattaforma dei bot terminal | 0,3-1% della size | per il nostro bot non si applicano |

**Profili di costo fisso per tx:**
- **Economy:** 100k CU × 300k µL = 0,00003 SOL, più base 0,000005, tip swqos 0,000005 → ~0,00004 SOL.
- **Competitive:** 100k × 1,5M µL = 0,00015, più tip Sender 0,001 → ~**0,00116 SOL**.

**Esempio di round trip con 0,25 SOL (competitive):**

| Voce | Costo |
|---|---|
| Fee della curva | 2,5% |
| Costo fisso (2 × 0,00116 SOL = 0,0023 SOL) | 0,93% |
| Drift avverso misurato (L=3) | ~3% |
| Gap e slippage in uscita | da aggiungere |
| **Soglia di pareggio** | **circa 6-8% di edge lordo per trade** |

---
## 3. Formule di fill (curva, solo SOL)
`bps_p` e `bps_c` vanno letti dall'ultimo TradeEvent del token (in mancanza, dalla FeeConfig).

- **Buy con SOL esatti B:**
  - `net = floor(B·10000/(10000+bps_p+bps_c))` (l'SDK può differire di ±1 lamport)
  - `tok = min(floor(net·vt/(vs+net)), real_tok)`
  - nuovo stato: vs += net, vt −= tok
  - Nel TradeEvent `sol_amount` = net, `fee` = 0,95%·net, `creator_fee` = 0,30%·net, `buyback_fee` = 50% di fee (verificato).
- **Sell di q token:**
  - `gross = floor(q·vs/(vt+q))`
  - `out = gross − ceil(gross·bps_p/1e4) − ceil(gross·bps_c/1e4)`
- **Valore di liquidazione LV(t):** è `out` per tutta la posizione, calcolato sullo stato a fine slot. TP, SL e trailing si valutano su LV, non sul prezzo spot.
- **Ombra della propria posizione:**
  - Lo stato reale non contiene il nostro acquisto, e calcolare l'uscita su di esso è prudente: l'errore è circa pari al nostro impatto (0,3-1,4% per 0,1-0,5 SOL a 5 SOL di real).
  - Variante "realistica": `vt_adj = vt_osservato − q_nostri`.
  - Riportarle entrambe.
- **Impatto di prezzo a 5 SOL di real:** 0,1 SOL → prezzo medio +0,28% (prezzo dopo +0,57%); 0,5 SOL → +1,41% (+2,84%); 1 SOL → +2,82%. Il round trip immediato perde il 2,47% netto, indipendentemente dalla size.
- **Dopo la migrazione (PumpSwap):**
  - `Q_eff = pool_quote_amount + virtual_quote_reserves` (i128 con segno, può essere negativo)
  - sell: `quote_gross = floor(b·Q_eff/(B+b))`, fee secondo lo scaglione di `mcap = Q_eff·supply/B`
- **Migrazione sintetica:** con `buy_v3`, il buy che completa la curva può oltrepassarla. Arrivano TradeEvent, CompleteEvent e PostCompleteBuyEvent. Il totale del compratore è la somma di TradeEvent e PostCompleteBuyEvent. Dopo il complete nessun trade sulla curva è possibile fino a `migrate_v2`: misurato nello stesso slot in 5 casi su 6, una volta dopo 5 slot.

---
## 4. "Momentum di acquisto chiaro": 4 varianti parametriche
**Pre-filtri comuni:**
- quote SOL, non mayhem
- 5 ≤ real_SOL e progress ≤ 30%
- trade dell'agente mayhem esclusi
- i trade multi-wallet nella stessa tx contano come un solo attore
- i buy < 0,01 SOL contano nel totale ma non soddisfano le soglie
- trade del creator esclusi

| Variante | Finestra | Condizioni (parametri da esplorare) |
|---|---|---|
| **V1 a numero di trade** | ultimi N trade (N = 20/30) | n_buy ≥ k·n_sell (k = 1,2/1,5/2); flusso netto ≥ X SOL (0,5/1/2); buyer unici ≥ U (6/8/10); quota volume dei top-3 buyer ≤ 0,6 |
| **V2 a slot** | ultimi W slot (25 ≈ 5,3 s / 50 ≈ 10,6 s / 150 ≈ 32 s) | n_buy > n_sell; flusso netto ≥ X; buyer unici ≥ U; **accelerazione**: flusso netto della seconda metà della finestra > prima metà |
| **V3 velocità di progress / prezzo** | W slot | Δprogress ≥ g punti (+2/+3/+5 pp in 50 slot); seconda differenza > 0; nessun singolo trade > 40% del flusso entrante della finestra |
| **V4 order-flow imbalance + velocità di liquidità** | ultimi 30 trade | OFI = (buySOL − sellSOL)/(buySOL + sellSOL) ≥ 0,3/0,4/0,5; buyer unici / n_buy ≥ 0,6; velocità di liquidità = ΔvSOL / n_trade ≥ soglia (secondo Marino et al. è il predittore singolo più informativo per la graduation) |

Per ogni segnale registrare tutti i valori grezzi, non solo il booleano. Cooldown per mint: almeno 15-50 slot.

---
## 5. Feature di manipolazione da registrare

| Feature | Definizione | Bastano gli eventi? | Valore misurato (mint SOL non mayhem con create osservato, n=105) |
|---|---|---|---|
| Acquisti nello slot del create (bundler) | wallet ≠ creator che comprano in `slot == slot_create` | sì | il 50% ne ha almeno 1, il 38% almeno 3 |
| Dev buy | token comprati dal creator nella tx di create e dopo | sì | mediana 5,9% della supply, p75 15,2%, p90 29,3% |
| Quota ancora in mano al creator | buy − sell del creator | **solo come limite superiore**: i transfer verso wallet satellite non compaiono negli eventi pump | il creator ha venduto entro la finestra nel 52% dei casi |
| Micro-acquisti ripetuti | quota di buy < 0,01 SOL; ripetizioni dallo stesso wallet | sì | mediana 9%, p90 26% |
| Concentrazione | quota del volume di acquisto dei top-3 wallet, Herfindahl | sì | mediana 51%, p90 80% |
| Flip rapidi | sell che chiudono un buy dello stesso wallet entro 10 slot | sì | mediana 13%, p90 73% |
| **Sell multi-wallet atomici** | ≥ 2 wallet che vendono lo stesso mint nella stessa tx (dump coordinato) | sì (raggruppando per firma) | 246 tx con ≥ 5 sell di wallet diversi in 20 min; il 30,8% delle coppie consecutive in banda è nella stessa tx |
| Buy multi-wallet atomici | come sopra, ma buy | sì | rari (22 tx) |
| Wash trading atomico (WT1) | stesso wallet, buy e sell dello stesso mint nella stessa tx | sì | 0 nel campione (lo studio di Szwajcok et al. lo trova sul 6-8% dei coin nel 2024-25) |
| Creator seriale | numero di create dello stesso `user` | sì, ma solo se il bot tiene il proprio storico (partenza a freddo) | — |
| Cluster per finanziatore comune | wallet finanziati dallo stesso indirizzo | **no**, serve RPC/indexer | — |
| Età del wallet, numero di holder reali | | **no**, serve RPC | — |
| Holder rewards / cashback / mayhem / tipo di quote | flag del create | sì | |

---
## 6. Regole di uscita e trappole
- **Uscite:**
  - TP su LV (+X%)
  - SL (−Y%)
  - trailing: si attiva da +A%, esce a −Z% dal picco di LV a fine slot, non dal picco del singolo trade, perché un trade isolato spesso viene subito annullato
  - time stop T slot (283 ≈ 60 s; 1.415 ≈ 5 min)
  - stale exit: nessun trade per G slot
  - opzionale: uscita immediata su sell multi-wallet atomico o su vendita del creator
- **Gap dello stop:** misurato in banda, il salto di prezzo tra due trade consecutivi ha p50 0,86%, p75 3,6%, p90 7,6%, p95 12%, p99 25%. A questo si somma L_sell. Uno SL a −15% può riempirsi a −30/−40% durante un dump: modellarlo eseguendo a fine slot S+L_sell, con guard e retry.
- **La liquidità non sparisce sulla curva:** si può sempre vendere alla curva. Il rischio è il prezzo, che crolla per dump di dev o bundle. Il massimo estraibile da tutti gli holder è `real_SOL`.
- **Migrazione mentre si è in posizione:**
  - Al CompleteEvent la curva si blocca.
  - Valutare e vendere su PumpSwap con le reserves del `CompletePumpAmmMigrationEvent`, le fee a scaglioni e `Q_eff` con segno.
  - Serve sottoscrivere gli eventi PumpSwap dei mint che si tengono.
  - Se la migrazione non arriva entro N slot: marcare la posizione e segnalarla.
- **Token che muoiono:** il 10% dei gap in banda è ≥ 10 slot. Il time stop deve vendere comunque, pagando l'impatto.
- **Buchi nello stream:** se la catena degli stati si rompe, il mint diventa dirty e si esce dalla simulazione registrando un esito "unknown", senza inventare prezzi.

---
## 7. Metriche e protocollo di valutazione
- Per ogni trade i: `PnL_i = Σout − B − costi fissi (buy + sell + tx fallite)`, con il rent pagato e restituito che si compensa; `r_i = PnL_i/B`.
- **Expectancy netta** E = media(r_i) (e in SOL). **Win rate** WR. **Payoff** = media(r|r>0) / |media(r|r≤0)|; WR di pareggio = 1/(1+payoff). **Profit factor** = ΣPnL⁺ / |ΣPnL⁻|.
- **Max drawdown** sulla curva di equity in SOL, con le posizioni concorrenti marcate a LV slot per slot.
- **Code pesanti:** percentili p1-p99, asimmetria, curtosi, quota di PnL dei top 1% e 5% dei trade, E senza il top 1%.
- **Bootstrap a blocchi** per giorno (o token×giorno), 10.000 ricampionamenti, IC 95% (percentile o BCa) per E, PF e WR.
- **Dimensione del campione:** n ≈ (1,96·σ_r/h)². Con σ_r ~ 0,4 e h = 2% servono ~1.500 trade per variante.
- **Overfitting:**
  - poche varianti dichiarate prima (≤ 4 × griglia piccola), contando K configurazioni
  - walk-forward per giorni (es. train 5 g / test 2 g, a rotazione)
  - **holdout finale di 7 giorni mai toccato**
  - nessun token a cavallo tra train e test
  - Deflated Sharpe o White/Hansen SPA
  - con K configurazioni rumorose il massimo atteso cresce come σ_E·√(2 ln K)
- **Sensibilità obbligatoria:** L ∈ {1,2,3,4,6,10}, EndOfSlot contro WorstInSlot, profilo economy contro competitive, size 0,1/0,25/0,5 SOL, fascia oraria UTC (lo sniping si concentra tra le 14 e le 23 UTC secondo Pine Analytics).
- **Regola go/no-go per la Fase 2:**
  - sull'holdout, con L=3 e profilo competitive, il limite inferiore dell'IC 95% di E è > 0
  - PF > 1,2
  - PnL > 0 anche senza il top 1% dei trade
  - risultato ancora positivo a L=4

---
## 8. Studi e dati pubblici
- **Marino et al., arXiv 2602.14860 (2026):**
  - campione: 655.770 token di settembre 2025, graduation 0,63%, mediana 4,4 min alla graduation
  - probabilità di graduation per soglia di vSOL: 50 SOL → 10,0%; 80 → 35,4%; 100 → 63,8%
  - buy-and-hold fino alla graduation in pari solo se p > vSOL²/115²: per la nostra banda (vSOL 35-38,5) servirebbe p > 9,3-11,2%, e la baseline sta sotto
  - il 92% dei token con ≥ 30 swap ha almeno un dump
  - nessun backtest di strategie
- **Szwajcok et al., "Meme Coin Factories", arXiv 2609.10246 (settembre 2026):**
  - campione: 15,2M coin (gennaio 2024 - gennaio 2026), graduation 1,02%
  - wash trading ≈ 17% delle transazioni, sul 6-8% dei coin
  - il top 1% dei cluster di creator crea il 58,6% dei coin
  - copycat: graduation 0,86% contro 9,2% degli originali
- **Mongardini & Mei, arXiv 2507.01963:** l'82,8% dei token con rendimento > 100% mostra crescita artificiale.
- **Pine Analytics (aprile 2025):** sniping finanziato dal deployer, oltre 15.000 SOL di profitto in un mese; il 55% esce entro 1 minuto.
- **CoinGecko (dati Dune), aprile 2024 - aprile 2026:** quota mensile di wallet in profitto (PnL realizzato) tra 30,1% (giugno 2025) e 73,3% (aprile 2026).
- **Dune (gennaio 2025):** solo lo 0,4% dei wallet ha guadagnato più di 10k$.
- **Backtest pubblici:** non ne ho trovato nessuno rigoroso per questa strategia. L'unico (DEV Community: +40 SOL in 1 h, WR 52,7%, 270 combinazioni) viene da un venditore di API, senza out-of-sample: inaffidabile.
- **Dataset:** Bitquery vende dump Parquet pump.fun; non ho trovato dataset aperti su HF o Kaggle.
- **Latenza:** Chorus One (dati 2024) trova che swQoS riduce la latenza e che priority fee e tip incidono poco.

---
## 9. Tabelle SQLite minime
- `events(slot, tx_index NULL, sig, mint, kind, is_buy, user, sol, tok, vs, vt, rs, rt, fee_bps, cfee_bps, ix_name, quote_mint, mayhem, arrival_ns)`
- `signals(id, mint, slot, variant, params_json, features_json, p_trigger)`
- `fills(signal_id, L, mode, ok, fail_reason, slot_exec, sol_in, tok, fixed_cost)`
- `exits(signal_id, L, rule, slot_trigger, slot_exec, sol_out, retries, fixed_cost, pnl, r)`
- `stream_health(slot, gap, dirty_mints)`

Registrare `arrival_ns` per ogni evento: serve a misurare Δobs reale.

## Recommendations
- Non usare il websocket pubblico: ho misurato una consegna al 28% del tempo reale, una disconnessione ogni 1-3 minuti e il 59% degli slot persi. Serve un flusso a pagamento con l'indice della tx nel blocco (gRPC Yellowstone o LaserStream). In alternativa PumpPortal: con circa 98 trade/s osservati, prenderli tutti costerebbe circa 8,5 SOL al giorno alla tariffa citata.
- Implementare il fill SlotExec: esecuzione a fine slot S+L, L di default 3 sia per entrare sia per uscire. Aggiungere la variante WorstInSlot, il guard di slippage (buy 15%, sell 25%), i retry sulle vendite pagando le fee ogni volta, e il drop senza costi. Riportare sempre la griglia L in {1,2,3,4,6,10}.
- Concatenare i trade tramite pre-stato e post-stato per ordinarli dentro lo slot e scoprire gli eventi mancanti. Un mint con la catena rotta va marcato dirty e tolto dalla simulazione.
- Leggere le fee da ogni TradeEvent (fee_basis_points, creator_fee_basis_points), non metterle fisse nel codice. Oggi valgono 1,25% per lato sulla curva e da 1,25% a scendere su PumpSwap, con scaglioni letti on-chain.
- Nella Fase 1 tradare solo token con quote SOL e non mayhem. Escludere i trade dell'agente mayhem (BwWK17cb...) dalle feature di flusso, registrando comunque tutto.
- Rendere esplicita la banda di ingresso. Con real SOL maggiore di 5 e progress tra 5 e 30%, la banda reale è 19,3-30% (mcap 38-46 SOL). Parametrizzarla, invece di dare per scontato che la condizione sui SOL sia indipendente dal progress.
- Valutare TP, SL e trailing sul valore di liquidazione netto (impatto e fee inclusi) a fine slot, non sul prezzo spot o sul singolo trade.
- Dimensionare con la soglia di pareggio: circa 2,5% di fee sulla curva, più 0,0-0,9% di costi fissi (profilo competitive con 0,25 SOL), più circa 3% di drift avverso misurato. Servono circa 6-8% di edge lordo per trade. Con 0,1 SOL i costi fissi della modalità competitive pesano il 2,3%.
- Registrare tutte le feature di manipolazione calcolabili dagli eventi, in particolare i sell multi-wallet nella stessa tx (dump coordinato) e i bundle nello slot del create, senza filtrarle subito.
- Per la valutazione: walk-forward per giorni, holdout finale di 7 giorni mai toccato, bootstrap a blocchi per giorno, almeno ~1.500 trade per variante, numero di configurazioni provate dichiarato. Go/no-go: limite inferiore dell'IC 95% dell'expectancy maggiore di 0 con L=3 e profilo competitive, PF maggiore di 1,2, PnL positivo anche senza il top 1% dei trade.
- Calibrare L all'inizio della Fase 2 con 50-100 tx sonda a costo minimo prima di mettere capitale.
- Sottoscrivere anche gli eventi PumpSwap dei mint in posizione, per gestire la migrazione, che di solito avviene nello stesso slot del complete.

## Measured
Tutto misurato da me il 24/10/2026 tra le 13:30 e le 14:30 UTC.

RPC mainnet:
- Slot: mediana 212,0 ms (206,2-219,8) su 451 campioni.
- Voti assenti (non-vote/total = 0,99998-1,0); finalized = processed − 2 slot; leader window di 4 slot; Agave 4.3.2; epoch 1066.
- TPS non di voto: mediana 1.286.

Account on-chain decodificati con l'IDL ufficiale:
- Global: vT 1,073e15, vS 30 SOL, real token 793,1e12, migration fee 0,015000001 SOL, buyback_bps 5000, max creator fee 300 bps, quote USDC in whitelist.
- FeeConfig della curva 8Wf5…: (0,95,30) piatta.
- FeeConfig di PumpSwap 5PHi…: 25 scaglioni, da 125 bps sotto 420 SOL di mcap a 30 bps oltre 98.240 SOL; flat non canonica 30 bps.

Rent (getMinimumBalanceForRentExemption):
- 165 B = 1.488.440 lamport; 170 B = 1.513.840; 137 B = 1.346.200.

Flusso live (logsSubscribe pump, 20 min, 179.823 tx):
- Il 70,7% delle tx fallisce; getSignaturesForAddress dà 111 tx/slot, l'86% fallite.
- 46.808 TradeEvent, 492 create, 12 complete e 12 migrazioni (5 su 6 nello stesso slot del complete).
- Ritmo stimato sugli slot coperti: ~98 trade/s e 61 create/min.
- Quote dei create: SOL 71%, PUMP 15%, USDC 3%. Mayhem: 28,5% dei create, 36% dei trade (agente con fee 0). Token-2022: 99,6%.
- Fee osservate (95,30,5000) nel 99,3% dei trade SOL non mayhem; buyback = 50% di fee; sol_amount è il netto entrato nella curva.
- Size dei buy: p50 0,147 SOL; il 16,4% sotto 0,01 SOL.
- In banda: gap tra tx consecutive 0 slot nel 65,7%, p95 20 slot; salto di prezzo tra trade p90 7,6%, p99 25%.
- Primo buy di un wallet non creator nello slot del create: 68%. Bundle con almeno 3 wallet: 38%.
- Dev buy: mediana 5,9% della supply.
- 246 tx di sell coordinati con almeno 5 wallet.
- La concatenazione degli stati torna nell'89,6% dei trade.

Drift avverso a S+1..S+4 dopo un trigger di momentum: da +3,0 a +3,9% (variante A, n=57), da +3,5 a +4,6% (B, n=15), da +0,7 a +3,2% (C, n=31). Mediane circa 0, p90 tra +16 e +26%.

Fallimenti per slippage: circa 15,6% dei tentativi (codici 6042, 6002, 6003).

Websocket pubblico:
- 151 msg/s e 0,27 MB/s; copre 69 slot in 53 s invece di 248, poi errore 1002.
- In 20 min: 17 disconnessioni, 59,4% degli slot persi, ritardo mediano 7,7 s che cresce fino a 30-45 s.

Campione di 300 transazioni reali:
- Priority fee: p50 1,9e-5 SOL, p90 1,7e-4 SOL.
- CU usate: p50 72k. CU limit: p50 125k.
- CU price: p50 260k µL.
- Il 38% paga una tip (p50 3e-5, p90 1e-3 SOL).
- Costo totale: p50 2,7e-5 SOL, p90 1,0e-3 SOL.

Tip floor Jito: p50 4,5e-6 SOL, p95 4,3e-5 SOL.

Curva:
- Real SOL 5 = progress 19,33%; progress 30% = 8,548 SOL.
- Round trip immediato: −2,47%.

File di lavoro:
- Script in /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/simmodel/scripts/ (collect.py, analyze.py, analyze2.py, feesample.py, feeanalyze2.py, feecfg.py, glob.py, borsh_idl.py).
- Dati in /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/simmodel/live.jsonl e /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/simmodel/fees.jsonl.
- Doc e IDL in /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/simmodel/docs/ e /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/simmodel/idl/.

Non misurati (stime): la distribuzione di L per un bot domestico che usa Sender o Jito (serve un wallet, quindi Fase 2) e il Δobs con un flusso a pagamento.

## Open questions
- Quale provider di dati si usa (gRPC Helius/Triton, PumpPortal, altro)? Quanto vale il suo Δobs reale da casa, e fornisce l'indice della tx nel blocco? Il websocket logs non lo fornisce.
- Quanto vale davvero L per invii via Helius Sender, Jito o Astralane dall'Italia? È misurabile solo con un wallet, quindi in Fase 2 con tx sonda.
- Qual è l'intento della banda? 'Liquidità maggiore di 5 SOL' sono SOL reali (allora la banda vera è 19,3-30%) oppure qualcos'altro?
- I token con quote PUMP, USDC o altri pump coin (circa il 29% dei create) vanno inclusi più avanti? Servirebbe convertire la quote in SOL lungo la catena.
- I token mayhem (28,5% dei create) vanno esclusi del tutto o tradati togliendo i trade dell'agente dai flussi?
- Che size di posizione si vuole? Determina il peso dei costi fissi (2,3% con 0,1 SOL in modalità competitive) e l'impatto di prezzo.
- Alpenglow è attivo da pochi giorni (21/10/2026) e gli slot sono a 200 ms dal 9/10. Le condizioni di landing, i client Firedancer e le aste BAM potrebbero cambiare: conviene rimisurare tutto periodicamente.
- Il ritmo di creazione osservato (61 al minuto) è circa 4 volte quello riportato per il 2025. Va verificato con un flusso completo, perché quello pubblico perdeva il 59% degli slot.
- Il drift avverso misurato poggia su campioni piccoli (15-57 trigger) presi da finestre di stream con buchi. Va rimisurato con un flusso completo come primo output della Fase 1.

## Sources
- https://github.com/pump-fun/pump-public-docs (README, ultimo commit 13/10/2026)
- https://raw.githubusercontent.com/pump-fun/pump-public-docs/main/docs/FEE_PROGRAM_README.md
- https://raw.githubusercontent.com/pump-fun/pump-public-docs/main/docs/SYNTHETIC_MIGRATION.md
- https://raw.githubusercontent.com/pump-fun/pump-public-docs/main/docs/instructions/TRADE_V3.md
- https://raw.githubusercontent.com/pump-fun/pump-public-docs/main/docs/instructions/SWEEP_FEES.md
- https://raw.githubusercontent.com/pump-fun/pump-public-docs/main/docs/FEE_RECIPIENTS.md
- https://raw.githubusercontent.com/pump-fun/pump-public-docs/main/docs/PUMP_PROGRAM_README.md
- https://raw.githubusercontent.com/pump-fun/pump-public-docs/main/idl/pump.json (anche pump_amm.json, pump_fees.json)
- On-chain via https://api.mainnet-beta.solana.com: Global 4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf, FeeConfig 8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt (curva), FeeConfig 5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx (PumpSwap), getRecentPerformanceSamples, getSlotLeaders, getMinimumBalanceForRentExemption, logsSubscribe, getTransaction
- https://bundles.jito.wtf/api/v1/bundles/tip_floor
- https://www.helius.dev/docs/sending-transactions/sender
- https://solanacompass.com/news/anza-sets-alpenglow-mainnet-date-solanas-consensus-switch-is-scheduled-for-october-21-at-about-1450-utc
- https://cryptobriefing.com/jito-bam-upgrade-cuts-bundle-delays/
- https://chorus.one/reports-research/transaction-latency-on-solana-do-swqos-priority-fees-and-jito-tips-make-your-transactions-land-faster
- https://www.blockrazor.io/blog/20250801Benchmarking
- https://arxiv.org/html/2602.14860v1 (Marino et al., Predicting the success of new crypto-tokens: the Pump.fun case)
- https://www.emergentmind.com/papers/2609.10246 (Szwajcok et al., Meme Coin Factories, arXiv:2609.10246)
- https://arxiv.org/abs/2507.01963v2 (Mongardini & Mei, A Midsummer Meme's Dream)
- https://pineanalytics.substack.com/p/exit-liquidity-machines
- https://www.coingecko.com/research/publications/pump-fun-traders-are-making-a-comeback
- https://decrypt.co/300403/pump-fun-traders-millionaires
- https://dev.to/gengengen/my-pumpfun-bot-makes-4000-a-day-here-are-the-4-numbers-51oa (aneddotico, venditore)
- https://www.theblock.co/post/379285/pump-funs-new-mayhem-mode-fails-boost-token-launches-revenue
- https://blockworks.co/news/pumpdotfun-fee-model
- https://docs.bitquery.io/docs/cloud/pumpfun/