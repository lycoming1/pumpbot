# Verifica: da correggere

## Errori trovati
- §2 TradeEvent.ix_name: non vale solo buy/sell/buy_exact_quote_in/multi_hop_swap. Compare anche `buy_exact_sol_in` (2 casi su 2, dall'istruzione buy_exact_sol_in). Il suffisso v2/v3 manca solo su pump. Su PumpSwap BuyEvent.ix_name lo porta: osservati `buy_v2` e `buy_exact_quote_in_v2`.
- §2 'Ogni self-CPI di evento costa 2170 CU' vale solo per pump (130 casi su 130). Per PumpSwap il costo è 2116 CU (94 casi su 94).
- §2/§10 Troncamento log: il tetto non è di circa 10 KB per tutta la rete. È un parametro di replay del singolo nodo (default Agave 10.000 byte, cambiabile). api.mainnet-beta ha restituito un log di 14.708 byte senza 'Log truncated' (tx 4TTb14…). Il troncamento dipende quindi dal provider; il fallback sul canale (B) resta obbligatorio.
- §5 Pool: l'account NON misura sempre 287 byte. I campi finiscono a 287, ma 20 pool su 23 misurano 301 byte (14 byte a zero in coda, rent 2.179.320) e 3 su 23 misurano 287. Niente filtri dataSize=287: accettare len ≥ 287.
- §7 Trade AMM v2: nella vault restano pf+cf−buyback_fee, non pf+cf, e virtual_quote_reserves cala di (pf+cf−buyback_fee). Il buyback viene trasferito fuori. Verificato a catena su 9 trade v2 su 9. Qeff evolve come nei trade legacy.
- §3/§4 Manca l'evento pump DistributeFeeToHoldersEvent, discriminatore e3bed7ceb0b4a584, 88 byte: timestamp i64, mint, quote_mint, recipients u64, total u64. Lo emette distribute_fee_to_holders (623691610246ad2b) ed è comparso in 14 tx pump riuscite su 80. Mancavano anche i layout di BoostBuyAndBurnEvent (200 B), SweepPoolFeeEvent (177 B), Init/CloseUserVolumeAccumulatorEvent (72 B) ed ExtendAccountEvent (88 B).
- §7 boost_buy_and_burn (694406af000723a2) emette un BuyEvent con fee 0/0/0 e ix_name 'buy_exact_quote_in', seguito da BoostBuyAndBurnEvent. Non è flusso di utenti: va escluso dalle statistiche di acquisto su PumpSwap. Nella spec mancava.
- §2 Lista dei nomi di istruzione incompleta e fuorviante. Mancano: pump DistributeFeeToHolders, ExtendAccount, CloseUserVolumeAccumulator; AMM BuyExactQuoteInV2, BoostBuyAndBurn, Init/CloseUserVolumeAccumulator. Il programma Mayhem (MAyh…) scrive nei log i propri 'Instruction: Buy/BuyV2/Sell/SellV2' prima di fare CPI su pump. Molti trade arrivano via CPI, quindi i nomi non sono dell'istruzione 'esterna'.
- §7 Migrazione: nell'esempio verificato l'ha eseguita un terzo (né il bot pump né il creatore), tramite un programma sniper (migrate_v2 via CPI, stack height 2) seguito da un buy_v2 PumpSwap nella stessa tx, nello slot dopo la graduation. Inoltre un migrate_v2 su una curva già migrata riesce con il log 'Bonding curve already migrated' e senza eventi: la migrazione va dedotta solo dal CompletePumpAmmMigrationEvent.
- §6 Graduation: la rq ideale è 85.005.359.057 (= ceil(k/279,9e12) − 30e9), non …056. La deriva NON viene solo dai buy exact-out: ogni trade arrotonda a favore della curva (buy exact-out +1, exact-in con net−1, sell con floor). Osservato 85.005.359.893 (+836). Di conseguenza il pool nasce con 84.990.359.892 e l'InitBoost sposta floor(Q·2069/1e4) = 17.584.505.461.
- §2 Deduplica: con commitment processed i duplicati non sono un caso isolato. Il 46–49% delle notifiche di create riuscite è arrivato due volte (stesso slot, log identici) in 2 finestre da 60–90 s. Con confirmed: 0 duplicati in 46 s. Le 0,8 notifiche/s contano i duplicati; le create uniche sono 0,39–0,70/s.
- §6 Esclusioni: le quote non-SOL non sono solo USDC, PUMP e pump coin. Osservati come quote token azionari tokenizzati (Xsc9qv…, XsCPL9…, XsbEhL…) e SQQQAa…, abilitati via QuoteControl anche se Global.whitelisted_quote_mints contiene solo USDC: 8 create su 26 non-SOL. Inoltre il CreateEvent di una coin non-SOL ha virtual_sol_reserves = virtual_quote_reserves (≠ 0), a differenza del TradeEvent.
- §6 'Coin cashback (deprecate)': Global.is_cashback_enabled è true, quindi non sono deprecate a livello di programma; nel campione 0 create su 26 cashback. L'esclusione va mantenuta.
- §1 Il bump del pool non è fisso (253 nella spec, 255 nell'esempio verificato): va cercato con find_program_address.
- §8 Prezzo CU e fee: dipendono molto dal campione (qui p50 25.143 µlamport/CU e p75 526k; fee per tx p50 10.000 e p75 88.807 lamport). Le tx v1 portano la priority fee in transactionConfig e non in istruzioni ComputeBudget: misurare da meta.fee.

## Tx
2YwbG7DoYumDkuz611d45yncpdkovKHsTyBwfNLF45Ee9P3zTGyoMvWTL9bdhj9XKA9EySe1FdjLAQqNNaAJRHxV
2bwnEUvP6kMeSmyea12CJbovnv5vZ7hGmxoC6g1ibG4Evz3RoZF9v1itLCYjN4uD2qqcXFoJiE4NHe5ZSgM6HWdu
37Q46T4T5t1LYVReMVr1zEoYMYGTYNUJfRZPCMDVqp2sHC4XVtSeQ1ZVn7rJxKCpAwnYzYaw6sYwZN35GnJMjPtk
5bBh8Xd4DF891mvgsoRQ5pboMkGZWFLAXJ2TkHiNYBCmKT2K3v9zkkTLdq2XavZDU8quqivLPZsPYVUoPXQpFtNX
ee9fDfHgjcvpC9yQ93QUJzJ1ko86GPujX9T1rR8UQtvUPw8QR3LrnxqFjfMrBxbBtoB1eKja4MiSVsCLvBkSDjw
36xJ7nyiQ6qcAVe1wT8TXe1B5MUkS8DvLzhVeo34fiC3LaeG2JDt5k3EiCCqwxQMa2LPZzqbaAVLrEcCEuikqZw1
2VNU5t2ofRXv2LQMpJqwM5PtmsPRnsQVuQRojAkDKjWoxsBdYXVVgqwqBeP3trcxNqCjPkiUQAErhgvMEnfoqRDH
2UqKQHQbY7oK5odef9or8eutb6w5cCnfu4yVWZxg24tRMdtz733EHjCZ8vQvQ3LUUuhJc8AXZPaFBcjxHy3MmwA5
2a75QShV5sVrcMhrYuR9wi1HPASYsc95XjZwepAYdM1T5sp5SQpHt9EprLT8k6L76DW1mhuRJ3cBCCktsZrmc48i
4CgXpKiiRvwi35giPhiC9dfdvZd57vzTUobZQUNHdkoD4EwJBurgbF1vHCUTCBx7pMzJ19FXmGxA44Ty67dz2Rnw
224eWVk54Scc4CgFVGR2BkP11fobhhvT9GrDjW5c9WG5vXvx374iJVhRB8DUyJMBMqB2VEcCHqJVY7hJSya6tTTP
3wxkhXnHfP5EiRG2cDpe2QfMgTziL4jiLUqhqY6a5KtDN1q8eAJHJKFp6NfWzxfvsk4wP4HAoowuJErSTdBvgr7U
4TTb14ZEywsAWB69thJme2qXsicsZ8WeLF73WUrVMnw2oYWDEPn88KoSZ5U5JF5XayFXqELndDNhh8XPNqcAVGGv
2mzdDNCg8AcULSwjo4p27XZ5PC6bnwEWWTLMdznC1U6xqQnd81GpuMKPXQVz1KWRnFsQjQUeSjYk4SjhfFTikTFs
3NftnsBAxXQKTuifrWbbr5GT53pVGR2gbw7HybsoR1QBXu9rujDQYXiP2pVFHYHm52XzD2TT22a55RYpBpzA9kfk
1ywS4jZZG16AUwL6UvfDJ6oUCmhw7pevS4dY11NuDcTVhGkgR7hnb3K3wR41vwq8ywS2qGAEcsWoi4KunXMxGK4
2C6saM4y81kYGfbGQ7oHmWRs2DY9nDH6jDRPHfjZQrV3VaguqzRGh2Bkp3FkwWRQ713fNTdrW6TbAu1HZT49oHy
2Dw2SFn7GenDj6whsiNGLA4jYXV6YVsM17sECL62HcqhwEkdwjF5oVQEkTgjZT4SW7od5JrKUZaAkqh9XfCA3Ekv
62S6xCZouppfffhL71fKDJMBChZZeTnUsJ59s74mqDnivfygfVv2RUsethYNYHMb2nA1DuWLAAiA4ZdWXki5mewR
2GVvFYTV22ZLFaTp8DQMvKnKiBdg8J3RBQ8o8L38hJgK5XkDpMbdd4jCrAoQUtyAAvyQ6wuLRF8oYJK9fc2Pe4PT
2Gsz8cCM83eS4WwR3hgjuaeEEwSXZhApJgB9AnFtQfbknNE3mQF6xj73a1BUPaea6qaR6tFwpintfR6gjeFyAUdh
(+ altre 196 tx riuscite decodificate integralmente, slot 460.881.731-460.886.424; file JSON in /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/verificatore/data/tx_{pump,create,amm,mig,pool1}/)

## Evidence
Metodo: tutto via JSON-RPC e WS pubblici dalla VM. Ho decodificato 217 tx riuscite e uniche, slot 460.881.731–460.886.424, quindi dopo il campione originale (≤460.879.500): 80 dal firehose pump, 25 create, 60 PumpSwap, 14 attorno a una graduation e migrazione reali (mint L32Mpi2q…pump, pool 2vPwtf6e…), 40 primi trade sul pool appena nato. Il decoder Python segue la specifica (/tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/verificatore/scripts/pumpspec.py) ed è eseguito con python3 -I.

Risultati principali:

1) Discriminatori: tutti i 37 della spec ricalcolati con sha256 sono corretti. EVENT_IX_TAG invertito è corretto.

2) Canali A/B: identici in tutte le tx; ogni emit_cpi ha 1 account. Costo per evento: pump 2170 CU (130/130), AMM 2116 CU (94/94).

3) Layout:
- TradeEvent: lunghezza 379+L+34n su 76/76, con 8 byte finali sempre a 0.
- CreateEvent: termina a s+252 (26/26).
- CompleteEvent 136, CompletePumpAmmMigration 192, CreatePool 336, InitBoost 128, BoostBuyAndBurn 200, SellEvent 441, BuyEvent 397+L+89 (489/492/504).
- Tutto coincide con l'IDL ufficiale (pump-public-docs, commit 639b87fb del 13/10/2026) più 8 byte extra. Carbon main conferma il prefisso dei campi: è più vecchio, ma tollera i byte in coda.
- Evento non in spec: e3bed7ceb0b4a584 = DistributeFeeToHoldersEvent, 88 B, in 14 tx su 80.

4) Matematica della curva su 48 TradeEvent SOL non-mayhem, 0 eccezioni:
- invarianti vq−rq=30e9 e vt−rt=279,9e12: 48/48;
- fee in ceil 48/48, buyback in floor 48/48;
- exact-out 17/17, exact-in 15/15 (4 con ex>0), sell 16/16;
- saldo ATA curva = rt+206,9e12: 48/48;
- lamport della curva v3 = ±q + fee − buyback + creator_fee: 30/30.

5) Account letti dal vivo:
- Global 1088 B, valori identici alla spec;
- FeeConfig pump 0/95/30;
- FeeConfig AMM: 25 scaglioni identici alla tabella, flat 25/5/0, exotic 20/5/5;
- 31/31 curve SOL riconciliate con lamports = rent(166) + rq + creator_fee + protocol_fees;
- tutte le BondingCurve misurano 166 B;
- Pool: 301 B in 20 casi su 23, 287 B in 3 (i campi finiscono a 287).

6) PDA: tutte le PDA della spec ricalcolate con un controllo ed25519 off-curve. Pool canonico, lp_mint e vault verificati su un pool reale (bump 255, non 253).

7) AMM su 60 trade WSOL:
- formule exact-out, exact-in e sell valide in tutti i 60 casi;
- pool_*_reserves = preTokenBalances in 59 casi;
- scaglione scelto con mcap Qeff·supply/B, non raw (418,47 → tier 0; 426,65 → tier 420);
- a catena, nei trade v2 vqr cala di pf+cf−buyback, non di pf+cf (9/9);
- boost_buy_and_burn emette un BuyEvent con fee 0.

8) Graduation (3wxkhX…): legacy buy con TradeEvent rt=0 seguito da CompleteEvent, rq=85.005.359.893. Lo slot dopo arriva un trade fallito con 6005.

9) Migrazione (4TTb14…, tx v1, log di 14.708 B non troncato): eseguita via CPI da un programma sniper con buy_v2 nella stessa tx. Eventi in ordine CreatePool → InitBoost → CompletePumpAmmMigration. Pool iniziale 206,9e12 / 84.990.359.892 lamport; boost virtuale 17.584.505.461 = floor(Q·2069/1e4). Un secondo migrate_v2 (2mzdD…) riesce con 'already migrated' e nessun evento.

10) Tabella del progresso, finestra 19,33–30% e round trip 2,4691%: ricalcolati, identici.

11) WS: mentions con 2 pubkey restituisce -32602; con processed il 46–49% delle notifiche di create è duplicato; latenza p50 1,22 s.

12) Rent ricontrollate tutte, uguali alla spec. getTransaction su v1 senza maxSupportedTransactionVersion=1 restituisce -32015.

Non verificati, per mancanza di campioni: PostCompleteBuyEvent / synthetic migration, SweepBondingCurveFeeEvent, cbps=0 con creator di default, misura del firehose WS.

Script e dati: /tmp/claude-1000/-home-kraken/d5825fb0-0d54-44dc-a966-d4ebcf53e8af/scratchpad/pumpwf/verificatore/scripts e …/verificatore/data; IDL e file Carbon (solo letti) in …/verificatore/ext.

Fonti web: https://github.com/pump-fun/pump-public-docs (IDL), https://github.com/sevenlabs-hq/carbon (decoders/pumpfun-decoder e pump-swap-decoder), https://www.helius.dev/docs/rpc/transaction-v1, https://docs.chainstack.com/docs/solana-transaction-v1.md (SIMD-0385/v1).

## Spec corretta
# Specifica on-chain pump.fun + PumpSwap (stato del 24 ott 2026, verificata su mainnet)

> Revisione del verificatore indipendente (24 ott 2026). Ho controllato la specifica su 217 transazioni riuscite, tutte diverse dal campione originale: slot 460.881.731–460.886.424. Le ho confrontate con l'IDL ufficiale (pump-public-docs, commit 639b87fb del 13 ott 2026) e con il decoder Carbon (sevenlabs-hq/carbon main), letto senza eseguirlo. Ho anche letto dal vivo gli account Global, FeeConfig, BondingCurve (43) e Pool (24).
> Le correzioni sono marcate **[corretto]**, le aggiunte **[aggiunto]**.

Convenzioni: tutto è Borsh little-endian. Gli offset degli **eventi** escludono gli 8 byte di discriminatore: nel payload completo vanno aggiunti +8. Gli offset degli **account** (§5) includono già il discriminatore. I `pubkey` sono 32 byte, i `string` sono `u32 len` + UTF-8, i `Vec<T>` sono `u32 len` + elementi, `bool` è 1 byte, `i128`/`u128` sono 16 byte.

## 1. Indirizzi (verificati con la derivazione PDA e con letture RPC)
| Cosa | Indirizzo |
|---|---|
| pump.fun (bonding curve) | `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P` |
| PumpSwap (AMM) | `pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA` |
| Pump Fees program | `pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ` |
| Mayhem program | `MAyhSmzXzV1pTf7LsNkrNwkWKTo4ougAJ1PPg47MD4e` |
| pump Global PDA `["global"]` (bump 255) | `4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf` |
| pump mint-authority PDA `["mint-authority"]` (presente in ogni create) | `TSLvdd1pWpHVjahSpsvCXUbgwsL3JAcvokwaKt1eokM` |
| pump event_authority `["__event_authority"]` | `Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1` |
| pump fee_config = PDA(pfee, `["fee_config", pump_id]`) (bump 253) | `8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt` |
| AMM global_config `["global_config"]` (949 byte) | `ADyA8hdefvWN2dbGGWFotbzWxrAvLW83WG6QCVXvJKqw` |
| AMM fee_config = PDA(pfee, `["fee_config", amm_id]`) | `5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx` |
| AMM event_authority | `GS4CU59F31iL7aR2Q8zVS8DRrcRnXX1yjQ66TqNVQnaR` |
| WSOL | `So11111111111111111111111111111111111111112` |
| Token-2022 (tutte le nuove coin create_v2: 26 su 26) | `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` |
| **[aggiunto]** Global.whitelist_pda = agente mayhem (trade con fee 0) | `BwWK17cbHxwWBKZkUYvzxLcNQ1YVyaFezduWbtm2de6s` |
| **[aggiunto]** Global.reserved_fee_recipient (trade dell'agente mayhem) | `GesfTA3X2arioaHp8bbKdjG9vJtskViWACZoYvxp4twS` |
| **[aggiunto]** Global.withdraw_authority | `39azUYFWPz3VHgKCf3VChUwbpURdCHRxjWVowf5jUJjg` |

PDA per mint:
- `bonding_curve = PDA(pump, ["bonding-curve", mint])`, verificato su 26 create e 76 trade. Il suo ATA base usa il token program della coin: Token-2022 per create_v2, SPL Token per le coin vecchie. Il saldo di questo ATA vale `real_token_reserves + 206_900_000_000_000` (verificato su 48 trade e 26 create).
- **[aggiunto]** Indice di `bonding_curve` negli account dell'istruzione (0-based): legacy `buy`/`sell`/`buy_exact_sol_in` = 3; `*_v2` = 10; `*_v3` = 5; `multi_hop_curve_swap` = 15.
- `creator_vault = PDA(pump, ["creator-vault", creator])`. Le istruzioni v3 non lo passano: la fee creator resta sulla curva.
- `pool_authority = PDA(pump, ["pool-authority", mint])`
- **Pool canonico:** `pool = PDA(amm, ["pool", u16_le(0), pool_authority, mint, WSOL])`. **[corretto]** Il bump varia da pool a pool (253 negli esempi originali, 255 in quello verificato qui): usare find_program_address. Inoltre `Pool.creator == pool_authority`.
- `lp_mint = PDA(amm, ["pool_lp_mint", pool])`. Le vault del pool sono `ATA(pool, mint, base_token_program)` e `ATA(pool, WSOL, SPL Token)` (verificato).
- **[aggiunto]** Esistono pool "invertiti" (base = WSOL, quote = token) e pool non canonici con fee piatte 25/5/0. Il bot deve considerare solo il pool canonico: base = mint, quote = WSOL, index 0, creator = pool_authority.

## 2. Come arrivano gli eventi (verificato)
- **Due canali, stesso contenuto.** Ogni evento di pump e di PumpSwap compare:
  - (A) come riga di log `Program data: <base64(disc8 + borsh)>`, dentro il frame del programma emittente, subito prima del suo self-CPI;
  - (B) come self-CPI `emit_cpi` in `meta.innerInstructions`: programId = pump/amm, unico account = event_authority, `data = EVENT_IX_TAG(8) || disc(8) || borsh`.
- Le due liste di eventi sono identiche e nello stesso ordine: su 76 tx nel campione originale, su altre 165+ tx in questa verifica.
- `EVENT_IX_TAG = e4 45 a5 2e 51 cb 9a 1d`, cioè il u64 LE di 0x1d9acb512ea545e4 = sha256("anchor:event")[0..8] **invertito**. Attenzione: sha256 così com'è dà `1d9acb51…` e non funziona.
- **[corretto]** Costo di ogni self-CPI di evento: **2170 CU su pump** (130 su 130), **2116 CU su PumpSwap** (94 su 94).
- **Attribuzione dal log:** si tiene uno stack: `Program X invoke [n]` fa push, `Program X success|failed` fa pop. Una riga `Program data:` appartiene al programma in cima allo stack. Va accettata solo se il frame è pump o amm, perché gli aggregatori emettono i propri `Program data`.
- **Filtri obbligatori:**
  - `err == null`: nel firehose pump l'81–82% delle tx è fallito;
  - **[corretto]** deduplica per signature, sempre obbligatoria con `processed`. In 2 finestre da 60–90 s sul WS pubblico il **46–49%** delle notifiche di create riuscite è arrivato **due volte**, con stesso slot e log identici. Con `confirmed`: 0 duplicati in 46 s;
  - escludere `mayhem_mode == true`. **[aggiunto]** Nel campione firehose erano mayhem 23 TradeEvent su 76 (30%) e 4 create su 26;
  - accettare solo `quote_mint == 11111111111111111111111111111111` (SOL).
- **[corretto] Troncamento log.** Il limite dipende dal nodo che fa il replay: default Agave 10.000 byte, modificabile con `--log-messages-bytes-limit`. Non è un limite fisso della rete. Su api.mainnet-beta è arrivato intero un log di 14.708 byte (tx di migrazione + snipe). Con altri provider o Geyser il limite può essere diverso. Se una tx contiene `Log truncated`, gli eventi vanno letti dal canale (B), via getTransaction o Geyser: le inner instruction non vengono mai troncate.
- **Transazioni v1** (SIMD-0385; 58 su 217 tx in questo campione, circa 27%; 32% nel campione originale). `getTransaction`/`getBlock` vanno chiamati con `maxSupportedTransactionVersion: 1`: con 0, o senza il parametro, rispondono con errore -32015 (verificato). `logsSubscribe` non ne è toccato. **[aggiunto]** Le tx v1 non usano lookup table e mettono CU limit e priority fee in `transactionConfig` invece che in istruzioni ComputeBudget.
- **[corretto] Nomi nei log** (`Program log: Instruction: X`, da attribuire al frame con lo stack; spesso sono CPI, non l'istruzione esterna):
  - pump: `Buy`, `BuyV2`, `BuyV3`, `BuyExactSolIn`, `BuyExactQuoteInV2`, `BuyExactQuoteInV3`, `Sell`, `SellV2`, `SellV3`, `CreateV2`, `MigrateV2`, `MultiHopCurveSwap`, `DistributeFeeToHolders`, `ExtendAccount`, `CloseUserVolumeAccumulator`;
  - PumpSwap: `Buy`, `BuyV2`, `BuyExactQuoteIn`, `BuyExactQuoteInV2`, `Sell`, `SellV2`, `MultiHopSwap`, `CreatePool`, `InitBoost`, `BoostBuyAndBurn`, `InitUserVolumeAccumulator`, `CloseUserVolumeAccumulator`;
  - Pump Fees: `GetFeesWithQuoteMint` (CPI da pump);
  - il programma Mayhem scrive i **propri** `Instruction: Buy/BuyV2/Sell/SellV2/InitializeMayhemState` e poi fa CPI su pump. Non usare i nomi per classificare: usare gli eventi.
- **[corretto]** `TradeEvent.ix_name` (pump) vale `buy`, `sell`, `buy_exact_sol_in`, `buy_exact_quote_in` o `multi_hop_swap`, **senza suffisso v2/v3**. Una v3 si riconosce da `fee_recipient == chiave zero` (verificato su coin SOL non-mayhem: 26 v3 su 26 con chiave zero, 0 legacy/v2 con chiave zero). **[aggiunto]** `BuyEvent.ix_name` di PumpSwap invece **ha** il suffisso: `buy`, `buy_v2`, `buy_exact_quote_in`, `buy_exact_quote_in_v2`.

## 3. Discriminatori (sha256 = IDL, tutti ricalcolati e confermati)
**Eventi pump**

| Evento | Discriminatore |
|---|---|
| TradeEvent | `bddb7fd34ee661ee` |
| CreateEvent | `1b72a94ddeeb6376` |
| CompleteEvent | `5f72619cd42e9808` |
| CompletePumpAmmMigrationEvent | `bde95db95c94ea94` |
| PostCompleteBuyEvent | `6fb06d8b316cd5fb` |
| SweepBondingCurveFeeEvent | `742b4dbd117a482b` |
| InitUserVolumeAccumulatorEvent | `86240d48e86582d8` |
| CloseUserVolumeAccumulatorEvent | `929fbdac925838f4` |
| **[aggiunto]** DistributeFeeToHoldersEvent | `e3bed7ceb0b4a584` |
| **[aggiunto]** ExtendAccountEvent | `6161d7905d92167c` |

**Eventi AMM**

| Evento | Discriminatore |
|---|---|
| BuyEvent | `67f4521f2cf57777` |
| SellEvent | `3e2f370aa503dc2a` |
| CreatePoolEvent | `b1310cd2a076a774` |
| InitBoostEvent | `ae7c4af90451f611` |
| BoostBuyAndBurnEvent | `3f451c16305cc2b9` |
| SweepPoolFeeEvent | `82a42461e48287a5` |

Il parser deve **ignorare** i discriminatori sconosciuti: l'IDL pump ne definisce 32, quello AMM 27.

**Account**

| Account | Discriminatore |
|---|---|
| BondingCurve | `17b7f83760d8ac60` |
| Global | `a7e8e8b1c86c727f` |
| FeeConfig | `8f3492bbdb7b4c9b` |
| Pool | `f19a6d0411b16dbc` |
| GlobalConfig | `95089ccaa0fcb0d9` |

**Istruzioni pump** (`sha256("global:<snake>")[0..8]`)

| Istruzione | Discriminatore |
|---|---|
| create | `181ec828051c0777` |
| create_v2 | `d6904cec5f8b31b4` |
| buy | `66063d1201daebea` |
| sell | `33e685a4017f83ad` |
| buy_exact_sol_in | `38fc74089edfcd5f` |
| buy_v2 | `b817ee6167c5d33d` |
| sell_v2 | `5df6823ce7e940b2` |
| buy_exact_quote_in_v2 | `c2ab1c46684d5b2f` |
| buy_v3 | `07051dc4f5176550` |
| sell_v3 | `1c92de7726c469d5` |
| buy_exact_quote_in_v3 | `e1f7501ed5b38488` |
| migrate_v2 | `bbcb121fceedfe29` |
| multi_hop_curve_swap | `e19a7516d756f667` |
| **[aggiunto]** distribute_fee_to_holders | `623691610246ad2b` |
| **[aggiunto]** extend_account | `ea66c2cb96483ee5` |
| **[aggiunto]** migrate (legacy) | `9beae792ec9ea21e` |

Argomenti, dopo il discriminatore:
- `buy`/`buy_v2`/`buy_v3`: `amount u64, max_sol_cost u64, …`
- `buy_exact_sol_in`/`buy_exact_quote_in_v2/v3`: `spendable u64, min_tokens_out u64, …`
- `sell*`: `amount u64, min_sol_output u64`

**Istruzioni AMM**

| Istruzione | Discriminatore |
|---|---|
| buy | `66063d1201daebea` |
| sell | `33e685a4017f83ad` |
| buy_exact_quote_in | `c62e1552b4d9e870` |
| buy_v2 | `b817ee6167c5d33d` |
| sell_v2 | `5df6823ce7e940b2` |
| buy_exact_quote_in_v2 | `c2ab1c46684d5b2f` |
| create_pool | `e992d18ecf6840bc` |
| multi_hop_swap | `2b644913e9f66f94` |
| init_boost | `8ce9215e845ac28f` |
| **[aggiunto]** boost_buy_and_burn | `694406af000723a2` |

## 4. Layout degli eventi
Regola generale per le versioni: i campi nuovi vengono **aggiunti in coda**.
- Gli eventi vecchi sono più corti: un campo mancante si legge come 0 / false / default.
- Gli eventi attuali sono **più lunghi dell'IDL più recente** (repo del 13 ott 2026): TradeEvent, BuyEvent e SellEvent hanno **8 byte in più dopo `creator_fee_unclaimed`**, sempre a 0 su più di 150 eventi in questa verifica. Il parser deve ignorare i byte in eccesso, non rifiutarli.
- Carbon (main) decodifica con `BorshDeserialize::deserialize(&mut slice)`, che tollera i byte in coda. Però si ferma a campi più vecchi: TradeEvent finisce a `real_quote_reserves`, BuyEvent/SellEvent a `cashback`, BondingCurve a `quote_mint`, Pool a `is_cashback_coin`. Il prefisso coincide con questa specifica.
- TradeEvent e BuyEvent hanno una `string` (`ix_name`) in mezzo, e TradeEvent anche un `Vec<Shareholder>`: oltre quel punto gli offset sono relativi. Nel campione sono comparsi 2 eventi con 1 shareholder (lunghezze 417 e 431).

### 4.1 TradeEvent (pump)
Lunghezza del payload senza disc: `379 + L + 34·n`, dove L = lunghezza di ix_name e n = numero di shareholder. Esempi: "buy" → 382, "sell" → 383, "multi_hop_swap" → 393, "buy_exact_sol_in" → 395, "buy_exact_quote_in" → 397. La formula è valida su 76 eventi su 76.

Parte fissa:

| Off | Campo | Tipo |
|---|---|---|
| 0 | mint | pubkey |
| 32 | sol_amount | u64 |
| 40 | token_amount | u64 |
| 48 | is_buy | bool |
| 49 | user | pubkey |
| 81 | timestamp | i64 |
| 89 | virtual_sol_reserves | u64 |
| 97 | virtual_token_reserves | u64 |
| 105 | real_sol_reserves | u64 |
| 113 | real_token_reserves | u64 |
| 121 | fee_recipient | pubkey |
| 153 | fee_basis_points | u64 |
| 161 | fee | u64 |
| 169 | creator | pubkey |
| 201 | creator_fee_basis_points | u64 |
| 209 | creator_fee | u64 |
| 217 | track_volume | bool |
| 218 | total_unclaimed_tokens | u64 |
| 226 | total_claimed_tokens | u64 |
| 234 | current_sol_volume | u64 |
| 242 | last_update_timestamp | i64 |
| 250 | ix_name | string (u32 L + L byte) |

Dopo ix_name, con `o = 254 + L`:

| Off | Campo | Tipo |
|---|---|---|
| o | mayhem_mode | bool |
| o+1 | cashback_fee_basis_points | u64 |
| o+9 | cashback | u64 |
| o+17 | buyback_fee_basis_points | u64 |
| o+25 | buyback_fee | u64 |
| o+33 | shareholders | u32 n, poi n × {address pubkey, share_bps u16} = 34 byte ciascuno |

Dopo gli shareholder, con `p = o + 37 + 34n`:

| Off | Campo | Tipo |
|---|---|---|
| p | quote_mint | pubkey (chiave zero = SOL) |
| p+32 | quote_amount | u64 |
| p+40 | virtual_quote_reserves | u64 |
| p+48 | real_quote_reserves | u64 |
| p+56 | holder_rewards_bps | u64 |
| p+64 | holder_rewards | u64 |
| p+72 | creator_fee_unclaimed | u64 |
| p+80 | non documentato | u64 (osservato sempre 0) |

Semantica (verificata su 48 trade SOL non-mayhem, 0 eccezioni):
- Le riserve sono **dopo** il trade.
- `sol_amount` / `quote_amount` è il quote **netto** entrato o uscito dalla curva, senza fee.
- Su coin SOL i campi `*_sol_*` coincidono con `*_quote_*` (48 su 48). Su coin quotate in token i campi sol valgono 0: usare sempre i campi `quote_*`.
- `fee` = fee protocol, buyback incluso. `buyback_fee` = floor(fee·buyback_bps/1e4) (48 su 48).
- v3: `fee_recipient` = chiave zero. Sulla curva restano `fee − buyback_fee + creator_fee`, mentre il buyback esce. Verificato con le variazioni di lamport della curva su 30 trade v3.
- **[aggiunto]** Sulle coin holder-reward (`is_holder_reward`, 5 curve SOL su 31) il TradeEvent ha `holder_rewards_bps = 30`: la fee creator va agli holder. Il costo per chi fa trading non cambia (95 + 30).

### 4.2 CreateEvent (pump)
Tre stringhe in testa: `name`, `symbol`, `uri`. Poi, con `s` = offset subito dopo le stringhe:

| Off | Campo | Tipo |
|---|---|---|
| s | mint | pubkey |
| s+32 | bonding_curve | pubkey |
| s+64 | user | pubkey |
| s+96 | creator | pubkey |
| s+128 | timestamp | i64 |
| s+136 | virtual_token_reserves | u64 |
| s+144 | virtual_sol_reserves | u64 |
| s+152 | real_token_reserves | u64 |
| s+160 | token_total_supply | u64 |
| s+168 | token_program | pubkey |
| s+200 | is_mayhem_mode | bool |
| s+201 | is_cashback_enabled | bool |
| s+202 | quote_mint | pubkey |
| s+234 | virtual_quote_reserves | u64 |
| s+242 | creator_fee_bps | u64 |
| s+250 | is_holder_reward | bool |
| s+251 | depth | u8 |

Fine evento a s+252, senza byte extra (26 su 26). Non esiste un evento separato per create_v2 o Token-2022: tutto passa da questi campi in coda. Le coin quotate in pump coin hanno `depth ≥ 1`, al massimo 2 (Global.max_curve_depth = 2).

**[aggiunto]**
- Per le coin non-SOL `virtual_sol_reserves` = `virtual_quote_reserves` (≠ 0, diverso dal TradeEvent): distinguere SOL solo con `quote_mint`.
- `creator_fee_bps = 0` (13 coin SOL su 18) significa "usa il FeeConfig", cioè 30 bps.
- Non tutti i mint finiscono in "pump" (18 su 26).

### 4.3 Altri eventi pump (tutti a campi fissi)
**CompleteEvent** (verificato)

| Off | Campo | Tipo |
|---|---|---|
| 0 | user | pubkey |
| 32 | mint | pubkey |
| 64 | bonding_curve | pubkey |
| 96 | timestamp | i64 |
| 104 | quote_mint | pubkey |

Totale 136 byte.

**CompletePumpAmmMigrationEvent** (verificato)

| Off | Campo | Tipo |
|---|---|---|
| 0 | user | pubkey |
| 32 | mint | pubkey |
| 64 | mint_amount | u64 |
| 72 | sol_amount | u64 |
| 80 | pool_migration_fee | u64 |
| 88 | bonding_curve | pubkey |
| 120 | timestamp | i64 |
| 128 | pool | pubkey |
| 160 | quote_mint | pubkey |

Totale 192 byte.

**PostCompleteBuyEvent** (non osservato; layout conforme all'IDL)

| Off | Campo | Tipo |
|---|---|---|
| 0 | user | pubkey |
| 32 | mint | pubkey |
| 64 | bonding_curve | pubkey |
| 96 | quote_mint | pubkey |
| 128 | timestamp | i64 |
| 136 | base_out | u64 |
| 144 | quote_in | u64 |
| 152 | fee_basis_points | u64 |
| 160 | fee | u64 |
| 168 | creator_fee_basis_points | u64 |
| 176 | creator_fee | u64 |
| 184 | buyback_fee | u64 |
| 192 | pool_base_reserves_before | u64 |
| 200 | pool_quote_reserves_before | u64 |
| 208 | pool_base_reserves_after | u64 |
| 216 | pool_quote_reserves_after | u64 |

Totale 224 byte.

**SweepBondingCurveFeeEvent** (non osservato; layout conforme all'IDL)

| Off | Campo | Tipo |
|---|---|---|
| 0 | timestamp | i64 |
| 8 | mint | pubkey |
| 40 | bonding_curve | pubkey |
| 72 | quote_mint | pubkey |
| 104 | recipient | pubkey |
| 136 | amount | u64 |
| 144 | bucket | u8 (0 = protocol, 1 = creator) |

Totale 145 byte. Non va contato come ricavo: le fee sono già nei TradeEvent.

**[aggiunto] DistributeFeeToHoldersEvent** (`e3bed7ceb0b4a584`, emesso da `distribute_fee_to_holders`; 14 tx su 80 nel campione firehose)

| Off | Campo | Tipo |
|---|---|---|
| 0 | timestamp | i64 |
| 8 | mint | pubkey |
| 40 | quote_mint | pubkey |
| 72 | recipients | u64 |
| 80 | total | u64 |

Totale 88 byte. Non è un trade.

**[aggiunto] Eventi di servizio**

| Evento | Campi | Totale |
|---|---|---|
| InitUserVolumeAccumulatorEvent | payer pubkey, user pubkey, timestamp i64 | 72 B |
| CloseUserVolumeAccumulatorEvent | user pubkey, timestamp i64, total_unclaimed_tokens u64, total_claimed_tokens u64, current_sol_volume u64, last_update_timestamp i64 | 72 B (osservato) |
| ExtendAccountEvent | account pubkey, user pubkey, current_size u64, new_size u64, timestamp i64 | 88 B |

### 4.4 Eventi PumpSwap
**BuyEvent**: parte fissa fino a 393, poi ix_name. Lunghezza = `397 + L + 89`, osservata 489 / 492 / 504.

| Off | Campo | Tipo |
|---|---|---|
| 0 | timestamp | i64 |
| 8 | base_amount_out | u64 |
| 16 | max_quote_amount_in | u64 |
| 24 | user_base_token_reserves | u64 |
| 32 | user_quote_token_reserves | u64 |
| 40 | pool_base_token_reserves | u64 |
| 48 | pool_quote_token_reserves | u64 |
| 56 | quote_amount_in | u64 |
| 64 | lp_fee_basis_points | u64 |
| 72 | lp_fee | u64 |
| 80 | protocol_fee_basis_points | u64 |
| 88 | protocol_fee | u64 |
| 96 | quote_amount_in_with_lp_fee | u64 |
| 104 | user_quote_amount_in | u64 |
| 112 | pool | pubkey |
| 144 | user | pubkey |
| 176 | user_base_token_account | pubkey |
| 208 | user_quote_token_account | pubkey |
| 240 | protocol_fee_recipient | pubkey |
| 272 | protocol_fee_recipient_token_account | pubkey |
| 304 | coin_creator | pubkey |
| 336 | coin_creator_fee_basis_points | u64 |
| 344 | coin_creator_fee | u64 |
| 352 | track_volume | bool |
| 353 | total_unclaimed_tokens | u64 |
| 361 | total_claimed_tokens | u64 |
| 369 | current_sol_volume | u64 |
| 377 | last_update_timestamp | i64 |
| 385 | min_base_amount_out | u64 |
| 393 | ix_name | string |

Dopo ix_name, con `o = 397 + L`:

| Off | Campo | Tipo |
|---|---|---|
| o | cashback_fee_basis_points | u64 |
| o+8 | cashback | u64 |
| o+16 | buyback_fee_basis_points | u64 |
| o+24 | buyback_fee | u64 |
| o+32 | virtual_quote_reserves | **i128** (valore PRIMA del trade) |
| o+48 | can_boost | bool |
| o+49 | base_supply | u64 |
| o+57 | holder_rewards_bps | u64 |
| o+65 | holder_rewards | u64 |
| o+73 | creator_fee_unclaimed | u64 |
| o+81 | non documentato | u64 (osservato 0) |

**SellEvent**: tutto a campi fissi.

| Off | Campo | Tipo |
|---|---|---|
| 0 | timestamp | i64 |
| 8 | base_amount_in | u64 |
| 16 | min_quote_amount_out | u64 |
| 24 | user_base_token_reserves | u64 |
| 32 | user_quote_token_reserves | u64 |
| 40 | pool_base_token_reserves | u64 |
| 48 | pool_quote_token_reserves | u64 |
| 56 | quote_amount_out | u64 |
| 64 | lp_fee_basis_points | u64 |
| 72 | lp_fee | u64 |
| 80 | protocol_fee_basis_points | u64 |
| 88 | protocol_fee | u64 |
| 96 | quote_amount_out_without_lp_fee | u64 |
| 104 | user_quote_amount_out | u64 |
| 112 | pool | pubkey |
| 144 | user | pubkey |
| 176 | user_base_token_account | pubkey |
| 208 | user_quote_token_account | pubkey |
| 240 | protocol_fee_recipient | pubkey |
| 272 | protocol_fee_recipient_token_account | pubkey |
| 304 | coin_creator | pubkey |
| 336 | coin_creator_fee_basis_points | u64 |
| 344 | coin_creator_fee | u64 |
| 352 | cashback_fee_basis_points | u64 |
| 360 | cashback | u64 |
| 368 | buyback_fee_basis_points | u64 |
| 376 | buyback_fee | u64 |
| 384 | virtual_quote_reserves | i128 |
| 400 | can_boost | bool |
| 401 | base_supply | u64 |
| 409 | holder_rewards_bps | u64 |
| 417 | holder_rewards | u64 |
| 425 | creator_fee_unclaimed | u64 |
| 433 | non documentato | u64 |

Lunghezza osservata: 441 byte (37 su 37).

Semantica di BuyEvent e SellEvent:
- `pool_*_token_reserves` sono i saldi **raw delle vault subito PRIMA dell'istruzione**. Coincidono con i preTokenBalances in 59 casi su 59, quando il pool non viene toccato prima nella stessa tx. Se c'è un'operazione precedente nella stessa tx, per esempio migrazione + snipe, valgono gli stati intermedi.
- Quote effettivo = raw + virtual_quote_reserves, con segno. Il `virtual_quote_reserves` dell'evento è il valore pre-trade.
- **[aggiunto]** `boost_buy_and_burn` emette un **BuyEvent con fee 0/0/0** e ix_name `buy_exact_quote_in`, seguito da BoostBuyAndBurnEvent. Non è flusso di utenti: va escluso dai conteggi di acquisto.
- **[aggiunto]** Nei trade AMM v2 `protocol_fee_recipient` e il relativo token account sono la chiave zero.

**CreatePoolEvent** (verificato, 336 byte)

| Off | Campo | Tipo |
|---|---|---|
| 0 | timestamp | i64 |
| 8 | index | u16 |
| 10 | creator | pubkey |
| 42 | base_mint | pubkey |
| 74 | quote_mint | pubkey |
| 106 | base_mint_decimals | u8 |
| 107 | quote_mint_decimals | u8 |
| 108 | base_amount_in | u64 |
| 116 | quote_amount_in | u64 |
| 124 | pool_base_amount | u64 |
| 132 | pool_quote_amount | u64 |
| 140 | minimum_liquidity | u64 |
| 148 | initial_liquidity | u64 |
| 156 | lp_token_amount_out | u64 |
| 164 | pool_bump | u8 |
| 165 | pool | pubkey |
| 197 | lp_mint | pubkey |
| 229 | user_base_token_account | pubkey |
| 261 | user_quote_token_account | pubkey |
| 293 | coin_creator | pubkey |
| 325 | is_mayhem_mode | bool |
| 326 | creator_fee_bps | u64 |
| 334 | can_edit_creator_fee | bool |
| 335 | is_holder_reward | bool |

Totale 336 byte.

**InitBoostEvent** (verificato, 128 byte)

| Off | Campo | Tipo |
|---|---|---|
| 0 | timestamp | i64 |
| 8 | mint | pubkey |
| 40 | bonding_curve | pubkey |
| 72 | pool | pubkey |
| 104 | virtual_quote_reserves | i128 |
| 120 | real_quote_reserves_after | u64 |

Totale 128 byte.

**[aggiunto] BoostBuyAndBurnEvent** (verificato, 200 byte)

| Off | Campo | Tipo |
|---|---|---|
| 0 | timestamp | i64 |
| 8 | mint | pubkey |
| 40 | bonding_curve | pubkey |
| 72 | pool | pubkey |
| 104 | authority | pubkey |
| 136 | quote_amount_in_requested | u64 |
| 144 | quote_amount_in_used | u64 |
| 152 | base_amount_burned | u64 |
| 160 | virtual_quote_reserves | i128 |
| 176 | real_quote_reserves_after | u64 |
| 184 | base_reserves_after | u64 |
| 192 | boost_vault_remaining | u64 |

**[aggiunto] SweepPoolFeeEvent** (dall'IDL, 177 byte)

| Off | Campo | Tipo |
|---|---|---|
| 0 | timestamp | i64 |
| 8 | pool | pubkey |
| 40 | base_mint | pubkey |
| 72 | quote_mint | pubkey |
| 104 | recipient | pubkey |
| 136 | payer | pubkey |
| 168 | amount | u64 |
| 176 | bucket | u8 |

## 5. Layout degli account (offset con disc incluso)
**BondingCurve**: dimensione attuale 166 byte (43 curve su 43 lette dal vivo). Gli account vecchi possono essere più corti, e `extend_account` li porta a 166: i campi mancanti valgono 0.

| Off | Campo | Tipo |
|---|---|---|
| 8 | virtual_token_reserves | u64 |
| 16 | virtual_quote_reserves | u64 |
| 24 | real_token_reserves | u64 |
| 32 | real_quote_reserves | u64 |
| 40 | token_total_supply | u64 |
| 48 | complete | bool |
| 49 | creator | pubkey |
| 81 | is_mayhem_mode | bool |
| 82 | is_cashback_coin | bool |
| 83 | quote_mint | pubkey (zero = SOL) |
| 115 | creator_fee_bps | u64 |
| 123 | can_edit_creator_fee | bool |
| 124 | is_holder_reward | bool |
| 125 | creator_fee | u64 (fee creator in attesa, v3) |
| 133 | protocol_fees | u64 (fee protocol in attesa, al netto del buyback) |
| 141 | depth | u8 |
| 142 | initial_virtual_quote_reserves | u64 |
| 150 | post_complete_base_out | u64 |
| 158 | post_complete_quote_in | u64 |

- Riconciliazione verificata su 31 curve SOL su 31, residuo 0: `lamports = rent(166) + real_quote_reserves + creator_fee + protocol_fees`, con rent(166) = 1.493.520 lamport.
- Curva migrata: le 4 riserve valgono 0 e complete = true. Sui lamport restano le fee in attesa.

**[corretto] Pool**: i campi finiscono all'offset **287**, ma l'account vero misura **301 byte** (20 pool su 23; 14 byte finali a zero; rent 2.179.320) oppure 287 (3 su 23). Accettare `len ≥ 287` e non filtrare con `dataSize = 287`.

| Off | Campo | Tipo |
|---|---|---|
| 8 | pool_bump | u8 |
| 9 | index | u16 |
| 11 | creator | pubkey |
| 43 | base_mint | pubkey |
| 75 | quote_mint | pubkey |
| 107 | lp_mint | pubkey |
| 139 | pool_base_token_account | pubkey |
| 171 | pool_quote_token_account | pubkey |
| 203 | lp_supply | u64 |
| 211 | coin_creator | pubkey |
| 243 | is_mayhem_mode | bool |
| 244 | is_cashback_coin | bool |
| 245 | virtual_quote_reserves | **i128** |
| 261 | creator_fee_bps | u64 |
| 269 | can_edit | bool |
| 270 | is_holder_reward | bool |
| 271 | protocol_fees | u64 |
| 279 | creator_fees | u64 |
| 287 | (padding, 0 o 14 byte a zero) | |

**Global** (1088 byte, letto allo slot 460.885.770). Valori:
- initial_virtual_token_reserves 1.073.000.000.000.000; initial_virtual_sol_reserves 30.000.000.000; initial_real_token_reserves 793.100.000.000.000; token_total_supply 1e15;
- fee_basis_points 95; creator_fee_basis_points 5 (non usato perché esiste il FeeConfig); pool_migration_fee 15.000.001; buyback_basis_points 5000;
- initial_virtual_quote_reserves 4.292.000.000 (USDC); whitelisted_quote_mints [USDC];
- max_configurable_creator_fee_bps 300; max_curve_depth 2;
- **[aggiunto]** create_v2_enabled = true, mayhem_mode_enabled = true, is_cashback_enabled = true, is_holder_reward_enabled = true, creator_fee_configurable = true.

Altre quote (token azionari tokenizzati, pump coin) passano dagli account `QuoteControl`, non da Global.

**FeeConfig**: `bump u8, admin pubkey, flat_fees{lp,protocol,creator u64}, fee_tiers Vec<{threshold u128 lamports, fees}>, stable_fee_tiers Vec<…>, exotic_flat_fees`. Gli account sono allocati a 4097 byte; i dati finiscono a 177 (pump) e 2097 (AMM).
- Selezione dello scaglione: l'ultimo scaglione con mcap ≥ soglia. Il primo ha soglia 0.
- pump: flat = tier0 = stable0 = exotic = (0, 95, 30).
- AMM: flat (25, 5, 0); exotic (20, 5, 5); fee_tiers come in §7; stable_fee_tiers con soglie in unità della stablecoin (per esempio 59.000 USDC → 20/5/95).

## 6. Matematica della bonding curve (intera, prodotti in u128)
Costanti: VT0 = 1.073e15, VS0 = 30e9 lamport, RT0 = 793,1e12, SUPPLY = 1e15, 6 decimali.
- Invarianti verificati (coin SOL non-mayhem: 42 su 42 nel campione originale, 48 su 48 qui): `vq − rq = 30e9` e `vt − rt = 279,9e12`.

**Buy exact-out** (istruzioni `buy`, `buy_v2`, `buy_v3`) per `a` token (verificato 17 su 17):
- `q = floor(a·vq/(vt−a)) + 1`
- `fee_p = ceil(q·pbps/1e4)`
- `fee_c = ceil(q·cbps/1e4)`, con cbps = 0 se creator == default (non verificato)
- Il compratore paga `q + fee_p + fee_c`.
- Se a > rt l'istruzione fallisce (6021 NotEnoughTokensToBuy) oppure, con `partial_fill`, si ferma al residuo. L'argomento `partial_fill` esiste anche su v3.

**Buy exact-in** (`buy_exact_sol_in`, `buy_exact_quote_in_v2/v3`) con budget B (verificato 15 su 15, di cui 4 con ex > 0; 37 su 37 nel campione originale):
1. `net0 = floor(B·1e4/(1e4+pbps+cbps))`
2. `fp = ceil(net0·pbps/1e4)`, `fc = ceil(net0·cbps/1e4)`
3. `ex = net0+fp+fc−B`; se ex > 0 allora `net = net0−ex`, altrimenti `net = net0`. Le fee restano quelle calcolate su net0.
4. `tokens = floor((net−1)·vt/(vq+net−1))`
5. L'evento riporta `sol_amount = net`, `fee = fp`, `creator_fee = fc`.

**Sell** di `a` token (verificato 16 su 16):
- `out = floor(a·vq/(vt+a))`
- Il venditore riceve `out − ceil(out·pbps/1e4) − ceil(out·cbps/1e4)`.

**Aggiornamento delle riserve:**
- buy: `vq += net`, `rq += net`, `vt −= tok`, `rt −= tok`
- sell: `vq −= out`, `rq −= out`, `vt += a`, `rt += a`

**Buyback:** `floor(fee_p·5000/1e4)` va a uno degli 8 buyback recipient già nel trade. Il resto della fee protocol va al fee_recipient (legacy e v2) oppure resta in `BondingCurve.protocol_fees` (v3). Le coin mayhem non hanno buyback (buyback_bps = 0, verificato).

**Fee live sulla curva** (FeeConfig 8Wf5…, un solo scaglione da mcap 0): protocol 95 bps + creator 30 bps, lp 0, totale 1,25% sul netto.
- Tutti i 48 trade SOL avevano 95/30.
- Coin a coppia custom: creator_fee_bps per coin fino a 300. Osservati 300 e 10; su una coin quotata in pump coin anche protocol 0 e creator 300.
- Per robustezza conviene usare `fee_basis_points` e `creator_fee_basis_points` dell'ultimo TradeEvent di quella mint.
- La mcap usata per scegliere lo scaglione è `vq·1e15/vt`, con supply fissa a 1e15.

**Prezzo spot e market cap:**
- prezzo spot (SOL per token intero) = `vq/vt · 1e-3`
- mcap in SOL = `vq·1e15/vt/1e9`

**Progresso.** Quello della UI e di Bitquery è sui token: `progress = (RT0 − rt)/RT0`, equivalente a `100 − (saldo_ATA_curva − 206,9e12)·100/RT0`. Tabella ricalcolata, identica:

| Progresso token | SOL reali | mcap (SOL) |
|---|---|---|
| 5% | 1,151 | 30,15 |
| 10% | 2,394 | 32,6 |
| **19,33%** | **5,00** | 38,06 |
| 20% | 5,204 | 38,5 |
| 25% | 6,80 | 42,07 |
| **30%** | **8,548** | 46,16 |
| 50% | 17,59 | 70,35 |
| 80% | 43,41 | 167,4 |
| 100% | 85,005 | 410,9 |

Con la strategia "progresso 5–30% e liquidità > 5 SOL" la finestra reale è quindi **19,33–30%**, cioè real_quote_reserves tra 5,0 e 8,55 SOL e mcap tra 38 e 46 SOL. Se invece il progresso si definisse come rq/85,005, il 5–30% corrisponderebbe a 4,25–25,5 SOL.

**Graduation:** avviene quando `real_token_reserves == 0`.
- **[corretto]** La rq minima teorica è **85.005.359.057** (= ceil(k/279,9e12) − 30e9, con k = VT0·VS0). Ogni trade arrotonda a favore della curva (buy exact-out +1, exact-in con `net−1`, sell con floor), quindi k cresce e la rq finale sale con l'attività della coin. Osservati …057, …063 e **…893** (coin molto scambiata, +836 lamport).
- vq ≈ 115,005 SOL, prezzo finale 4,1088e-7 SOL/token.
- Nella stessa tx vengono emessi TradeEvent (rt = 0) e poi CompleteEvent. Verificato su un `buy` legacy (tx 3wxkhX…).
- Da quel momento ogni trade sulla curva fallisce con 6005 BondingCurveComplete finché non c'è la migrazione. Verificato su una tx nello slot successivo.
- Synthetic migration (solo buy v3 e multi-hop; non osservata in questa verifica): il buy che svuota la curva prosegue sul pool non ancora creato ed emette TradeEvent, CompleteEvent e poi PostCompleteBuyEvent. Quotazione: `pool_base = saldo_ATA_curva − residuo`, `pool_quote = rq + costo_curva − 15.000.001`, `out = floor((in−1)·base/(quote+in−1))`.

**Round trip sulla curva** (buy exact-in seguito da sell totale, senza altri trade): perdita del **2,469%** a qualsiasi taglia e progresso (ricalcolato: 2,4691% per 0,1 / 0,5 / 1 SOL al 5%, 20% e 30%). Lo scarto del prezzo medio d'esecuzione rispetto allo spot per 0,1 / 0,5 / 1 SOL al 20% è +0,28% / +1,40% / +2,81%. È simmetrico e si recupera in vendita. Sono esclusi fee di rete, rent degli ATA e movimenti di prezzo.

**Esclusioni:**
- Coin mayhem (`mayhem_mode`):
  - l'agente `BwWK17…` (Global.whitelist_pda) fa trade con fee 0 e fee_recipient `GesfTA…`;
  - gli altri utenti pagano 95/30 senza buyback;
  - vq−rq ≠ 30 SOL (osservati da 0,34 a 41,9 SOL), quindi la formula non vale;
  - sono circa il 30% dei TradeEvent pump.
- `quote_mint ≠ SOL`. **[corretto]** Comprende USDC, PUMP `pumpCmXqMfrsAkQ5r49WcJnRayYRqmXz6ae8H7H9Dfn`, pump coin (depth ≥ 1) e **token azionari tokenizzati** (es. `Xsc9qv…`, `XsCPL9…`, `XsbEhL…`, `SQQQAa…`), abilitati via QuoteControl. Erano 8 create su 26 nel campione.
- **[corretto]** Coin cashback: il flag globale `is_cashback_enabled` è attivo, non deprecato, ma nel campione 0 create su 26 erano cashback. Escluderle comunque.

## 7. Migrazione e PumpSwap
**Migrazione:**
- L'istruzione è `migrate_v2`, permissionless. **[corretto]** Può essere chiamata direttamente o via CPI da programmi sniper. Nell'esempio verificato (tx 4TTb14…) l'ha eseguita un terzo (`LfEcaU…`, né pump né il creatore) dentro un programma che nella stessa tx ha poi fatto un `buy_v2` PumpSwap. È avvenuta nello slot successivo alla graduation (N+1), e consuma circa 344k CU.
- **[aggiunto]** Un `migrate_v2` su una curva già migrata **riesce** con il log `Bonding curve already migrated` e **senza eventi**. La migrazione va riconosciuta solo dal `CompletePumpAmmMigrationEvent`.
- Eventi in ordine (verificato): CreatePoolEvent (amm), InitBoostEvent (amm), CompletePumpAmmMigrationEvent (pump).
- Il pool nasce con base = 206,9e12 e quote = rq_finale − 15.000.001. Nell'esempio: 85.005.359.893 − 15.000.001 = 84.990.359.892; nel campione originale 84.990.359.062.
- InitBoost sposta il 20,69% del quote in un boost vault: `virtual_quote_reserves = floor(Q·2069/1e4)` e raw = Q − virtual. Nell'esempio: virtual 17.584.505.461, raw 67.405.854.431. Il quote effettivo resta invariato, quindi il prezzo è continuo: −0,024% rispetto alla curva, per la migration fee.
- In seguito `boost_buy_and_burn` usa il boost vault per comprare e bruciare token (BuyEvent con fee 0 + BoostBuyAndBurnEvent): `base_supply` scende sotto 1e15.

**Formule AMM** (verificate 24 su 24 nel campione originale, 60 su 60 qui), con Qeff = vault_quote + virtual_quote_reserves (somma in i128, risultato ≥ 0) e B = vault_base:
- **Buy exact-out** di b token:
  - `qin = ceil(Qeff·b/(B−b))`
  - `lp = ceil(qin·lpbps/1e4)`, `pf = ceil(qin·pbps/1e4)`, `cf = ceil(qin·cbps/1e4)`
  - Il compratore paga `qin+lp+pf+cf` (= user_quote_amount_in).
- **Buy exact-in** con budget S:
  - stesso calcolo net0, fee ed excess della curva, con tb = lp+p+c
  - `out = floor((net−1)·B/(Qeff+net−1))`
  - Attenzione, nell'evento: `quote_amount_in = S` (lordo), `user_quote_amount_in = net`, `quote_amount_in_with_lp_fee = net + lp` (33 su 33).
- **Sell** di b token:
  - `out = floor(Qeff·b/(B+b))`
  - Il venditore riceve `out − lp − pf − cf`.
  - `quote_amount_out_without_lp_fee = out − lp`.
- buyback_fee = floor(pf·buyback_bps/1e4) (62 su 62).
- **[corretto] Trade v2** (`buy_v2`, `sell_v2`, `buy_exact_quote_in_v2`): nella vault restano **pf + cf − buyback_fee**, perché il buyback viene trasferito fuori. `virtual_quote_reserves` cala della stessa quantità, `pf + cf − buyback_fee`, e può diventare negativo. Così Qeff evolve come nei trade legacy: `Qeff' = Qeff ± quote netto + lp`. Verificato a catena su 9 trade v2 su 9.
- Nei trade legacy le fee escono e virtual_quote_reserves non cambia (25 su 25 a catena).

**Fee del pool canonico SOL:** mcap = Qeff·base_supply/B, con Qeff e non il raw (verificato: mcap Qeff 418,47 → 2/93/30, 426,65 → 20/5/95, mentre la mcap raw sarebbe stata circa 340). Soglie in SOL lette dal FeeConfig 5PHi…:

| Soglia mcap (SOL) | lp / protocol / creator (bps) |
|---|---|
| 0 | 2 / 93 / 30 |
| 420 | 20 / 5 / 95 |
| 1470 | 20 / 5 / 90 |
| 2460 | creator 85 |
| 3440 | creator 80 |
| 4420 | creator 75 |
| 9820 | creator 70 |
| 14740 | creator 65 |
| 19650 | creator 60 |
| 24560 | creator 55 |
| 29470 | creator 50 |
| 34380 | creator 45 |
| 39300 | creator 40 |
| 44210 | creator 35 |
| 49120 | creator 30 |
| 54030 | creator 28 |
| 58940 | creator 25 |
| 63860 | creator 23 |
| 68770 | creator 20 |
| 73681 | creator 18 |
| 78590 | creator 15 |
| 83500 | creator 13 |
| 88400 | creator 10 |
| 93330 | creator 8 |
| 98240 | creator 5 |

- Dalla soglia 2460 in su lp e protocol restano 20 e 5.
- I pool non canonici, inclusi quelli invertiti con base WSOL, hanno fee piatte 25/5/0 (verificato su 23 eventi).
- `exotic_flat_fees` = 20/5/5.

## 8. Costi di rete misurati (per il simulatore)
- **Fee base:** 5.000 lamport per firma.
- **Compute unit** (mediana del frame del programma; due campioni indipendenti):

| Istruzione | CU (campione originale) | CU (questa verifica) |
|---|---|---|
| buy_v3 | 47–53k | 52,6k |
| buy_exact_quote_in_v3 | 47–53k | 56–65k (range 47–89k) |
| buy legacy | 81k | 67k (CPI) |
| buy_exact_sol_in | – | 83k |
| buy_v2 | 86k | 88,6k |
| buy_exact_quote_in_v2 | 93–140k | 90k |
| sell_v3 | 44k | 44,8–48,5k |
| sell legacy (CPI) | 53k | 53k |
| sell_v2 | 91–96k | 92k |
| **[aggiunto]** create_v2 | – | 115k |
| **[aggiunto]** AMM buy / buy_exact_quote_in / sell | – | 83,6k / 86,4k / 74k |
| **[aggiunto]** AMM buy_v2 / sell_v2 | – | 51,5k / 48,9k |

  Se va creato l'ATA servono circa 20k CU in più.
- **[corretto] Prezzo CU:** varia molto da campione a campione.
  - Campione originale: mediana 50.000 µlamport/CU, p75 248k; fee per tx mediana 25.000 lamport, p75 95.000.
  - Questa verifica: p50 25.143, p75 526.321, p90 1.000.000 µlamport/CU; fee per tx p50 10.000, p75 88.807, p90 405.000 lamport.
  - Le tx v1 non hanno istruzioni ComputeBudget: misurare la fee da `meta.fee`.
  - I tip Jito sono a parte.
- **Rent attuale** (ricontrollata con getMinimumBalanceForRentExemption):

| Account | Rent (lamport) |
|---|---|
| ATA Token-2022, 170 byte | 1.513.840 |
| ATA SPL, 165 byte | 1.488.440 |
| user_volume_accumulator, 137 byte | 1.346.200 |
| BondingCurve, 166 byte | 1.493.520 |
| **[aggiunto]** Pool, 301 byte | 2.179.320 |

  La rent si recupera chiudendo l'account.

## 9. Ingestione dati (misurata dalla VM)
- **Solo create:** `logsSubscribe {mentions:[TSLvdd1pWpHVjahSpsvCXUbgwsL3JAcvokwaKt1eokM]}` con commitment `processed`.
  - Campione originale: 0,8 notifiche/s, 7,1 KB/s (circa 0,63 GB/giorno), p50 1,35 s. Ricontrollato: 0,86 notifiche/s, 7,2 KB/s, latenza tra arrivo locale e timestamp della chain p50 1,22 s e p90 2,58 s.
  - **[corretto]** Quasi la metà delle notifiche riuscite sono **duplicati** (46–49%): le create uniche sono **0,39–0,70/s**.
  - Include la dev-buy se è nella stessa tx.
  - Il timestamp della chain ha risoluzione di 1 s.
- **Firehose:** `mentions:[6EF8…]` dà 143 notifiche/s, di cui l'81% fallite, 342 KB/s (circa 30 GB/giorno). Sull'RPC pubblico la latenza è degradata a p50 10,9 s: non è usabile (non rimisurato).
- **Per i candidati:** `logsSubscribe {mentions:[bonding_curve PDA]}`, dato che ogni trade include la curva, per avere tutti i TradeEvent di quella mint. In alternativa `accountSubscribe` o un polling `getMultipleAccounts` sulla BondingCurve (166 byte, 100 curve per chiamata) come prefiltro economico.
- `mentions` accetta **un solo** pubkey (con 2 l'errore è `-32602 Only 1 address supported`). Il commitment di default è finalized: va impostato esplicitamente. Con `confirmed` non sono comparsi duplicati.
- **Tassi:**
  - circa 230–500 tx/s menzionano pump, con l'81–82% fallite;
  - **[corretto]** create uniche circa 35–60 mila al giorno (0,52/s da getSignaturesForAddress, 0,39–0,70/s dal WS);
  - **[aggiunto]** uno slot dura circa 0,21 s (1760 slot in 374 s): attenzione nel convertire slot in tempo.

## 10. Note per il decoder Rust
- Base64 → `&[u8]`. Controllare la lunghezza minima prima di ogni lettura (per TradeEvent almeno 8+250+4).
- Usare `u64::from_le_bytes` sugli offset fissi; saltare le stringhe leggendo il `u32` di lunghezza; non fallire se avanzano byte in coda. Ignorare i discriminatori sconosciuti, per esempio DistributeFeeToHoldersEvent ed ExtendAccountEvent, che hanno la stessa lunghezza di 88 B ma discriminatori diversi.
- Per i calcoli usare u128 (vq·a arriva a circa 1e26) e i128 per virtual_quote_reserves.
- Hot path: prefisso fisso di 250 byte (mint, importi, is_buy, riserve, fee). Poi saltare ix_name per leggere `mayhem_mode`, saltare gli shareholder per leggere `quote_mint`.
- Ricostruire le riserve pre-trade dall'evento (verificato): buy `vq0 = vq − q`, `vt0 = vt + tok`; sell `vq0 = vq + q`, `vt0 = vt − tok`.
- **[aggiunto]** Quando si legge dal canale (B) via getTransaction:
  - per le tx v0 la lista degli account è `accountKeys + loadedAddresses.writable + loadedAddresses.readonly`;
  - l'istruzione emittente di un evento è l'ultima istruzione dello stesso programma con `stackHeight` = stackHeight dell'evento − 1.
- **[aggiunto]** Per le statistiche di momentum su PumpSwap scartare i BuyEvent generati da `boost_buy_and_burn` (fee 0/0/0).
