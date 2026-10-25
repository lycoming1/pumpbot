# pumpbot — Fase 1: osservatore e paper trading su pump.fun

Misura se la strategia "compra sulla bonding curve quando c'è momentum di acquisto" è profittevole al netto di fee, costi
di rete, ritardo di esecuzione e slippage.

- Legge solo dati pubblici: **nessun ordine reale, nessun wallet, nessuna chiave privata**.
- Specifica in `DESIGN.md` (la §0 elenca le correzioni dopo le misure del 24 ott 2026).
- Ricerca in `docs/research/`.

## Cosa fa

- Tre flussi dall'RPC pubblico di Solana, ognuno su due hostname con deduplica:
  - **Tier A:** stato di tutte le bonding curve a fine slot (programSubscribe);
  - **Tier B:** log delle curve osservate, cioè quelle con almeno 1 SOL reale;
  - **Tier C:** creazioni (log della mint authority).
- Ricostruisce ogni slot quando è "maturo" (75 slot dopo, ~16 s). Ordina i trade concatenando gli stati e segna
  `dirty` i mint con buchi nei dati.
- Fa girare **144 configurazioni** di paper trading in parallelo: 2 bande × 6 varianti di segnale (V0 casuale come
  controllo) × 3 regole d'uscita × latenze 1/3/6 slot, più il fill pessimistico a L=3.
  - Esecuzione a fine slot S+L, con guard di slippage e retry sulle vendite.
  - Costi nei profili competitive ed economy.
- Registra tutto in SQLite e manda un report su Telegram ogni 6 ore (0, 6, 12, 18 UTC), più gli alert se il feed si
  ferma.

## Comandi

```bash
./build.sh test               # test (decoder contro le tx mainnet di tests/fixtures)
./build.sh release            # compila target/release/pumpbot (container con 1,6 GB, 2 job)
docker compose up -d --build  # avvio (immagine distroless con il binario compilato)
docker compose logs -f        # una riga di salute al minuto

# report delle configurazioni (anche mentre gira)
docker compose run --rm --no-deps pumpbot report --db /data/paper.db
docker compose run --rm --no-deps pumpbot report --db /data/paper.db --filter V3 --since 2026-10-26

# replay degli eventi registrati con un'altra configurazione (es. altre soglie)
docker compose run --rm --no-deps pumpbot replay --events /data/events-20261025.db --out /data/replay.db --config /config/pumpbot.toml

# solo feed e decoder, senza motore, per N secondi
docker compose run --rm --no-deps pumpbot probe --secs 120
```

Dopo una modifica a `src/`: `./build.sh release && docker compose up -d --build`.

## File in `data/`

| File | Contenuto |
|---|---|
| `events-AAAAMMGG.db` | stati Tier A di tutte le curve SOL non-mayhem, trade, create, complete, riga di salute al minuto. Retention 21 giorni |
| `paper.db` | `configs`, `signals` (con tutte le feature in JSON), `trades` (un trade paper per riga, con esito e costi), `open_positions` |
| `heartbeat` | ultimo minuto di vita (usato dall'healthcheck) |

Gli esiti dei trade sono:
- `ok`: entra nelle metriche;
- `data_gap`: mint con buchi durante la posizione, da analizzare a parte;
- `entry_failed`: guard dello slippage, si paga solo la fee;
- `cancelled`, `aborted_restart`, `replay_end`: esclusi dalle metriche.

## Protocollo di valutazione (DESIGN §9)

- Almeno 2 settimane di dati, con gli ultimi 7 giorni come holdout mai toccato.
- Go/no-go per la Fase 2, sull'holdout con L=3 e profilo competitive:
  - limite inferiore dell'IC 95% dell'expectancy > 0;
  - profit factor > 1,2;
  - PnL > 0 anche senza il top 1% dei trade;
  - ancora positivo a L=6;
  - migliore del controllo V0 della stessa banda.
- Una prova offline di V3 sui dati del 24 ott è già negativa (`docs/research/momentum-offline.md`): ci si aspetta un
  no, servono i dati per chiudere V1, V2 e V4.

## Limiti noti

- **RPC pubblico.** Si chiude ogni 2–3 minuti (gestito con riconnessione e ridondanza). Sotto carico consegna i log in
  ritardo: i trade arrivati dopo la finalizzazione rendono il mint `dirty`. Il conteggio è nella riga di salute
  (`late_t`, `lagT`).
- **Migrazione in posizione.** Uscita al prezzo del pool appena nato (motivo `graduated`), senza seguire PumpSwap.
- **Riavvio.** Le posizioni aperte si chiudono come `aborted_restart`.
