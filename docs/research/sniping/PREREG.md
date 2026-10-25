# Ipotesi pre-registrata (24 ott 2026, 20:37 UTC, slot corrente 460.988.394)

Fissata dopo aver visto i dati fino allo slot 460.988.000 circa. La verifica usa SOLO i token
con slot di create S0 > 460.990.000 (holdout, creati dopo questa nota). Nessun parametro va
ritoccato sull'holdout.

Universo: create riusciti, quote SOL, non mayhem, non cashback, S0 visto nel flusso curves,
nessun buco del flusso in [S0-1, S0+25].

Feature: s0_in = virtual_sol a fine slot S0 meno virtual_sol dopo la tx di create (SOL entrati
nello slot del create oltre al dev buy della tx di create).

Strategia primaria: se s0_in >= 5 SOL, compra 0,1 SOL (exact-in) allo stato di fine slot S0+2;
uscita TP +20% / SL -20% sul valore di liquidazione netto a fine slot, eseguita 2 slot dopo la
decisione; time stop 283 slot (60 s). Impatto 'adj'. Costi fissi 0,002 SOL per andata e ritorno.

Criterio di successo: media netta > 0 e limite inferiore dell'IC 95% (bootstrap a blocchi da
~10 minuti) > 0.

Secondari (solo descrittivi, non decidono): L = 1 e 3; uscita H15 (3 s); soglie 3 e 10 SOL;
stesso test su tutti i token (controllo, atteso negativo).
