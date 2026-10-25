# Momentum: prima prova offline della variante V3 (25 ott 2026)

Domanda: la strategia della Fase 1 ("compra sulla bonding curve quando c'è momentum") può funzionare?

**Dati.**
- Gli stati a fine slot di tutte le curve, registrati sabato 24 ott 2026 dalle 19:44 alle 23:51 UTC per l'analisi dello
  sniping (`data/sniping-20261024/curves.csv.zst`).
- È lo stesso flusso del Tier A di `DESIGN.md`.
- 4.006 curve SOL non-mayhem.

**Metodo.** Regole fissate prima di guardare i risultati: docstring di `momentum-offline/momentum.py`.
- **Bande:**
  - A: progresso ≤ 30% con almeno 5 SOL reali, quindi 19,3–30%;
  - B: progresso 5–30%.
- **Segnali** (dati fino a fine slot t, senza lookahead):
  - V3 = progresso cresciuto di almeno 3 punti in 50 slot, con accelerazione;
  - V3s = idem con almeno 5 punti;
  - V0 = ingressi casuali sugli stessi slot idonei, con la stessa frequenza di V3.
- **Esecuzione:**
  - ingresso a fine slot t+L, L ∈ {1, 3, 6}, 0,25 SOL;
  - uscite E1/E2/E3 come in `DESIGN.md` §5.1, eseguite L slot dopo la decisione;
  - uscita per inattività dopo 150 slot.
- **Costi:** profilo competitive, 0,93% a giro.
- **Configurazioni:** K = 54.

**Risultato: tutte le 54 configurazioni perdono, con l'IC 95% interamente sotto zero.**

| | Banda A (19–30%) | Banda B (5–30%) |
|---|---|---|
| Segnali V3 / curve | 1.850 / 427 | 3.290 / 758 |
| V3, uscita E1 (TP+20/SL−10/60 s), L = 1–6 | lordo −3,1/−3,3%, netto −4,1/−4,3%, win 28%, PF netto 0,51–0,54 | lordo −2,1/−2,4%, netto −3,0/−3,4%, win 25%, PF 0,52–0,56 |
| V3, uscita E2 (TP+50/SL−20/5 min) | netto −6,0/−6,4% | netto −4,7/−5,1% |
| V3, uscita E3 (trailing) | netto −5,6% | netto −4,4/−4,8% |
| V3s (soglia 5 punti) | peggio di V3: netto −4,8/−6,5% | netto −3,4/−5,5% |
| V0 casuale, E1 | lordo −4,6/−5,1% | lordo −3,6/−3,8% |

**Lettura:**
- **Il segnale contiene informazione, ma troppo poca.** V3 batte gli ingressi casuali di 1,3–3 punti lordi a parità di
  banda e uscita. Il criterio go/no-go di `DESIGN.md` §9 richiede invece IC > 0 e PF > 1,2. Il migliore arriva a PF
  0,56 netto.
- **Lordo, il caso migliore (banda B, E1) perde il 2,1%.** Un giro immediato senza movimento di prezzo perde il 2,47%
  di fee. Quindi il momentum aggiunge solo +0,4 punti di deriva favorevole prima delle fee, contro i 3–4 punti che
  servirebbero per andare in pari con i costi.
- **La latenza conta poco.** Tra L = 1 e L = 6 cambia meno di mezzo punto.
- **Accelerazioni più forti non aiutano.** V3s è peggio di V3, coerente con pump seguiti da dump.

**Limiti.**
- Una sola serata.
- Mancano V1, V2 e V4, che richiedono i singoli trade (numero di acquisti e vendite, buyer unici, concentrazione). Il
  registro ha i trade solo per i primi 5 minuti di 1.093 token, con troncamento proprio sui token più attivi.
- Manca anche il filtro "nessun trade > 40% dell'inflow" di V3.
- Il flusso perde il 3% degli stati.

**Conclusione.** Sulla variante testabile la risposta è no, con margine ampio. Perché V1/V2/V4 vadano in pari
dovrebbero fare 4–5 punti lordi meglio di V3. Non è impossibile: i filtri sui buyer unici potrebbero scartare i pump
fatti da pochi wallet. Ma è improbabile, alla luce di questo risultato, del drift avverso già misurato
(`sim-model.md` §1.5) e degli studi citati lì. L'unico modo serio per chiuderla è il registratore della Fase 1 con i log
per curva (Tier B), lasciato girare almeno 2 settimane con l'holdout già previsto.
