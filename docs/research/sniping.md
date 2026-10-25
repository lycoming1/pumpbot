# Sniping su pump.fun: si può fare da casa, e conviene? (misure del 24 ott 2026)

> Stato: **definitivo**. Registrazione dal vivo 19:44–23:51 UTC; 2.793 token simulati; 1.093 token con tutte le tx dei
> primi 5 minuti. La verifica pre-registrata (§5) è conclusa: **ipotesi respinta**.

## Sintesi

Per **sniping** qui si intende comprare un token appena creato nei primi slot dopo il create (S0) e rivenderlo entro
secondi o minuti.

1. **Costruirlo si può.** Da casa, con il feed gratuito di PumpPortal e Helius Sender, si arriva a fine slot S0+2 o S0+3
   (stima: senza wallet non è misurabile). Con un VPS a Francoforte e un feed gRPC si arriva a S0+1–2.
2. **Lo sniping generico perde soldi.** Comprare ogni token SOL non-mayhem a S0+1 o oltre rende tra −1,9% e −5,4%
   lordo per trade con uscite entro 60 s, e −11% tenendo 5 minuti. Con i costi di rete di uno sniper (0,002 SOL a giro
   su 0,1 SOL) si scende a **−3,9% / −7,4%**. Mediana −2,5%: per la maggior parte dei token, dopo di noi non compra
   nessuno e restano solo le fee. Vince il 14–21% dei trade.
3. **Il guadagno sta tutto prima di S0+1, ed è riservato a chi lancia.** Chi compra dentro la tx di create (il prezzo
   del dev) fa **+11/+16% lordo** con le stesse uscite. A fine S0 il vantaggio è già sparito (da −0,4% a −3,5% lordo).
   Nei dati per wallet gli sniper indipendenti di S0–S2 stanno tra −9% e 0% di ROI, chi entra tra S3 e S100 perde il
   15–17%.
4. **I dati pubblici più ampi dicono lo stesso.** Bitquery ha analizzato 8,2 M di lanci (ott 2025 – ott 2026):
   - gli sniper seriali hanno speso 440 M$ e guadagnato 15–31 M$, cioè il 3,5–7%, **prima delle tip**;
   - i primi 100 wallet si prendono quasi metà del profitto;
   - dev e bundle hanno guadagnato almeno 579 M$.
5. **Lo sniping selettivo non ha retto alla verifica.** Nella prima ora i lanci con ≥ 5 SOL entrati nello slot del
   create sembravano positivi: +6% lordo a S0+2 con TP/SL. Ho fissato l'ipotesi per iscritto e l'ho verificata sui token
   creati dopo: **−7,1% netto (IC 95% −9,7/−4,4) su 374 lanci**. Nessun'altra feature nota al create dà un
   sottoinsieme positivo.
6. **Lo sniping alla migrazione è negativo** su 83 migrazioni non-mayhem: da −1% a −22% in media, a ogni latenza e
   durata.

**Raccomandazione:** non costruire uno sniper, né generico né selettivo con le feature disponibili al create, e non
pagare feed o infrastruttura per farlo. Il filone si chiude. La Fase 1 sul momentum è indipendente e resta com'è: il suo
protocollo di valutazione (paper trading con holdout e go/no-go dichiarati prima) è lo stesso che qui ha evitato di
scambiare un caso fortunato per un edge.

---

## 1. Come nasce un token oggi (misurato)

Fonti:
- create con il loro slot: logsSubscribe sulla mint authority;
- stati delle curve a fine slot: programSubscribe sulle BondingCurve, con due connessioni in unione;
- 2.836 token SOL non-mayhem con almeno 10 minuti di storia.

| Grandezza | Valore |
|---|---|
| Create riusciti | ~32 al minuto (7.955 in 4 h): SOL non-mayhem 40%, SOL mayhem 30%, quotati in altre pump coin 15%, in PUMP 14%, USDC 1% |
| Dev buy nella tx di create | mediana 0,80 SOL, p75 2,5, p90 4,9, p99 11,9; nessun dev buy nel 13% |
| SOL entrati nello slot del create **oltre** la tx di create (bundle e sniper di blocco 0) | presenti nel 46% dei token; ≥ 5 SOL nel 17%; p90 7,0 SOL, p99 19,6 |
| Prezzo a fine S0 rispetto a subito dopo la tx di create | media **+18%**, mediana 0, p90 +46% |
| Prezzo a fine S0+1…S0+10 rispetto a dopo la tx di create | media +16/+17%, mediana +1%: dopo S0, in media, il prezzo resta fermo |
| Token con almeno un trade tra S0+3 e S0+17 (3 s) / S0+152 (32 s) / 5 min | 55% / 72% / 83% |
| Slot con trade nei 5 minuti dopo S0+2 | mediana 5, p90 85 |
| Graduati entro 10 minuti | 57 su 2.836 (2,0%) |

Strumenti di lancio, ricavati dall'host dei metadati dei create SOL non-mayhem: ipfs.io 55%, `metadata.j7tracker.io`
24%, uxento 5%, poi rapidlaunch, pinata e altri.

## 2. Chi compra nei primi slot (1.093 token, tutte le tx dei primi 5 minuti)

**Slot del create.**
- Nel 45% dei token il create è l'unica tx dello slot che tocca la curva.
- Nel 21% c'è un bundle di 4–5 tx contigue (create più acquisti): è il formato del bundle Jito.

**Primi acquirenti per slot.**
- S0: 2.873 wallet (creatori compresi), mediana 1,09 SOL.
- S1: 730, mediana 0,20 SOL.
- S2: 409, mediana 0,17 SOL.

**Tx fallite.**
- L'82% delle tx sulla curva nei primi 5 minuti fallisce: 816.114 contro 173.324 riuscite.
- Per slot: S1 82%, S2 90%, S3 92%.
- Su 48 fallite campionate nei primi 5 slot di 8 token, 36 vengono da un solo wallet. Usa un programma che verifica
  delle condizioni e abortisce di proposito (errore custom 111).
- Ogni tentativo fallito costa ~0,000006 SOL, perché la tip sta nella stessa tx e viene annullata.
- Quindi la "folla" dei primi slot è fatta di pochi bot che sparano acquisti condizionali a ogni slot, non di centinaia
  di concorrenti.

**Servizi di invio.** Conteggio delle tx con una tip a un tip account noto: Helius Sender 7.952, Astralane (attribuito
dal prefisso dell'indirizzo) 2.915, Jito 1.984, Nozomi 854.

**Costi pagati dai primi acquirenti indipendenti:**

| Slot del primo acquisto | Fee di rete p50 / p90 (SOL) | Tip p50 / p90 (SOL) |
|---|---|---|
| S0, non nel bundle | 0,0005 / 0,0061 | 0,00001 / 0,005 |
| S1 | 0,00016 / 0,0040 | 0,00001 / 0,002 |
| S2 | 0,00008 / 0,0013 | 0,000001 / 0,001 |

## 3. Quanto rende entrare a S0+L (simulazione su 2.793 token)

**Metodo.**
- Si compra B SOL (exact-in, fee 1,25%) sullo **stato reale della curva a fine slot S0+L**.
- Si esce con una regola fissa:
  - vendite a tempo;
  - TP/SL sul valore di liquidazione netto, eseguito L slot dopo la decisione;
  - trailing;
  - "oracolo", cioè la miglior uscita possibile nei 5 minuti, come limite superiore.
- Impatto 'adj': lo stato osservato viene spostato del nostro acquisto. La variante 'obs' (senza spostamento) peggiora i
  numeri di circa 0,6 punti.
- IC 95% con bootstrap a blocchi da 10 minuti.
- L = −1 significa comprare dentro la tx di create, cioè al prezzo del dev e prima del bundle.

**B = 0,1 SOL, costi da sniper (0,002 SOL a giro = 2%):**

| L | 1 s | 3 s | 10 s | 32 s | 5 min | TP+20/SL−20/60 s | trailing | oracolo 5 min |
|---|---|---|---|---|---|---|---|---|
| −1 (tx di create) | +13,6% [+10,8; +16,6] | +12,8% [+9,7; +16,1] | +11,8% [+8,5; +15,1] | +9,0% [+6,0; +12,2] | −0,6% [−3,0; +1,8] | +12,2% [+9,2; +15,5] | +9,1% [+6,0; +12,2] | +48,2% |
| 0 | −2,4% [−2,8; −2,0] | −2,6% [−3,4; −1,8] | −3,4% [−4,4; −2,5] | −5,5% [−6,9; −4,2] | −11,9% [−14,2; −9,9] | −3,8% [−4,4; −3,1] | −6,5% [−7,5; −5,6] | +20,4% |
| 1 | −3,9% [−4,3; −3,5] | −4,3% [−4,9; −3,6] | −5,0% [−5,8; −4,2] | −6,9% [−8,2; −5,7] | −13,2% [−15,4; −11,3] | −5,3% [−5,8; −4,8] | −8,0% [−8,9; −7,2] | +18,0% |
| 2 | −4,2% [−4,5; −3,9] | −4,6% [−5,1; −4,1] | −5,2% [−6,0; −4,3] | −7,1% [−8,5; −5,9] | −13,4% [−15,5; −11,4] | −5,6% [−6,2; −5,1] | −8,4% [−9,3; −7,5] | +17,5% |
| 3 | −4,2% [−4,5; −3,8] | −4,5% [−5,1; −3,9] | −5,1% [−5,9; −4,3] | −7,1% [−8,4; −5,9] | −13,4% [−15,5; −11,4] | −5,8% [−6,3; −5,3] | −8,3% [−9,2; −7,5] | +17,2% |
| 6 | −4,7% [−4,9; −4,5] | −4,9% [−5,3; −4,4] | −5,4% [−6,1; −4,7] | −7,4% [−8,7; −6,1] | −13,4% [−15,5; −11,4] | −6,1% [−6,7; −5,4] | −8,8% [−9,7; −7,9] | +15,7% |

**Come leggerla:**
- **Lordo** (senza costi di rete) i valori sono tutti 2 punti più alti. Da L = 0 in poi restano comunque tutti negativi,
  con l'IC sotto zero.
- Con B = 0,25 SOL i costi fissi pesano 0,8% invece del 2%, ma l'impatto cresce: a L = 2 si va da −3,0% a −6,0% sulle
  uscite entro 32 s.
- **Mediana −2,5%, win rate 14–21%, p90 tra +4% e +24%** (L = 1–3). La media si regge su pochi token che esplodono: senza
  il top 1% dei trade scende di altri 1–2 punti.
- **L'oracolo** (+19/+20% lordo a L = 1–3) dice che i soldi ci sarebbero, ma vanno ai pochi token giusti. Con regole che
  non sanno quali, la media è negativa. Tutto il problema è la selezione, non la velocità.
- **La latenza conta poco una volta superato S0.** Tra L = 1 e L = 6 la differenza è inferiore a un punto: in media il
  prezzo dopo S0 non si muove. Arrivare a S0+1 invece che a S0+3 non salva una strategia che è negativa per selezione.

## 4. Chi guadagna davvero

**Nei nostri dati.** 851 token con tutte le tx dei primi 5 minuti; esclusi i 242 token più attivi, oltre 150 tx riuscite.

Metodo:
- per ogni wallet, PnL = vendite − acquisti − fee di rete − tip e fee di piattaforma;
- il residuo è valutato allo stato finale osservato ("a mercato"), oppure a zero ("solo realizzato");
- la categoria dipende dallo slot del primo acquisto.

| Categoria | Wallet | SOL investiti | PnL a mercato (SOL) | ROI | Solo realizzato (SOL) | Win rate |
|---|---|---|---|---|---|---|
| Creatore | 777 | 1.973 | +186,7 | +9% | −952,9 | 36% |
| Bundle nello slot del create (tx contigue al create) | 575 | 1.241 | +437,1 | **+35%** | −491,7 | 37% |
| S0, fuori dal bundle (sniper di blocco 0) | 380 | 241 | −12,2 | −5% | −18,9 | 29% |
| S1 | 419 | 187 | −0,7 | 0% | −28,2 | 27% |
| S2 | 229 | 100 | −9,0 | −9% | −21,9 | 25% |
| S3–S5 | 412 | 248 | −41,8 | −17% | −64,6 | 22% |
| S6–S20 | 1.003 | 561 | −95,8 | −17% | −125,4 | 16% |
| S21–S100 | 1.585 | 795 | −122,2 | −15% | −188,9 | 16% |
| oltre S100 | 1.613 | 684 | +130,7 | +19% | −321,6 | 22% |

Come leggerla:
- **A 5 minuti creatori e bundle hanno venduto poco.** Il realizzato è molto negativo e il valore a mercato è
  ottimistico, perché vendere tutto insieme farebbe crollare il prezzo. Lo stesso vale per il +19% di chi entra oltre
  S100: è quasi tutto invenduto (realizzato −322 SOL).
- **Il dato robusto riguarda gli sniper indipendenti.** Da S0 a S2 stanno tra −9% e 0% a mercato e sono negativi sul
  realizzato. Nessuna categoria di non-insider guadagna su entrambe le misure.
- Il vantaggio degli insider si vede meglio nella simulazione (L = −1, +11/+16% lordo con uscite rapide) e nei dati a
  3 giorni di Bitquery.

**Bitquery** (8,2 M di lanci dal 30 ott 2025 al 7 ott 2026; profitti calcolati sui lanci fino al 30 giu 2026, verificato
sulla pagina):
- **Chi compra nel blocco 0.**
  - Wallet diversi dal dev nel 38% dei lanci.
  - Wallet del dev nel 21%.
  - "Sniper bot" (wallet presenti nei primi 3 blocchi di lanci di almeno 10 dev diversi) nel 28%. La loro presenza è
    scesa dal 39% di novembre al 22% di settembre.
- **Sniper.**
  - 25.882 wallet, 440 M$ spesi.
  - Utile 31,5 M$ sulle vendite, 15,2 M$ contando l'invenduto a zero.
  - In utile il 54% dei wallet; i primi 10 fanno 6,7 M$, i primi 100 13,1 M$.
  - **Le tip non sono contate.**
- **Dev e bundle.**
  - Almeno 579 M$ (529 M$ con l'invenduto a zero), cioè 18 volte gli sniper.
  - In utile sul 71% dei lanci con bundle.

## 5. Sniping selettivo: c'è un sottoinsieme che rende?

**Feature testate** (tutte note entro la fine di S0): dev buy, SOL entrati in S0 oltre la tx di create (`s0_in`),
creatore già visto, nome o simbolo già visti (copycat), strumento di lancio (host dei metadati), programmi extra nella
tx di create.

**Sul campione completo** (L = 2, TP +20% / SL −20% / 60 s, lordo, split temporale in due metà) tutte le fasce di tutte
le feature sono negative in entrambe le metà:

| Feature | Fascia migliore (scoperta / verifica) | Fascia peggiore (scoperta / verifica) |
|---|---|---|
| dev buy | < 0,5 SOL: −1,5% / −2,1% | ≥ 2 SOL: −5,2% / −7,6% |
| `s0_in` | < 1 SOL: −1,8% / −1,2% | 1–5 SOL: −6,7% / −9,5% |
| creatore già visto | no: −2,9% / −4,3% | sì: −3,6% / −4,0% |
| copycat | no: −3,0% / −4,0% | sì: −3,2% / −4,2% |
| strumento | ipfs: −2,5% / −2,7% | altri: −6,6% / −5,8% |

Le fasce "migliori" sono quelle in cui dopo il create non succede quasi niente: si perdono solo le fee.

**L'unico candidato emerso, e la sua verifica.**
- Nella prima ora (456 token), i lanci con `s0_in ≥ 5 SOL` comprati a S0+2 con TP +20% / SL −20% / 60 s davano +6%
  lordo (IC +0,5/+16) su 72 token.
- La lettura plausibile: gli insider spingono per qualche secondo prima di scaricare, e un TP rapido cavalca il picco.
- Era però una soglia scelta sugli stessi dati dopo una ventina di confronti, con segno diverso tra le due metà. Per
  questo l'ipotesi è stata **pre-registrata** alle 20:37 UTC (testo in `docs/research/sniping/PREREG.md`) e verificata
  solo sui token creati dopo (S0 > 460.990.000, 2.135 token):

| Holdout (creati dopo le 20:37 UTC) | n | Media netta | IC 95% | Lorda | Mediana | Win |
|---|---|---|---|---|---|---|
| **Primario:** `s0_in ≥ 5`, S0+2, TP20/SL20/60 s | 374 | **−7,1%** | [−9,7; −4,4] | −5,1% | −18,8% | 38% |
| idem a S0+1 / S0+3 | 374 | −6,9% / −6,9% | [−9,4; −4,7] / [−9,3; −5,0] | | | |
| `s0_in ≥ 5`, uscita a 3 s, S0+1/2/3 | 374 | −6,7% / −7,0% / −6,3% | tutti sotto zero | | | |
| soglia 3 SOL / 10 SOL, S0+2 | 563 / 106 | −8,4% / −0,8% | [−10,4; −6,5] / [−7,7; +5,6] | | | |
| controllo: tutti i token, S0+2 | 2.102 | −5,8% | [−6,4; −5,3] | | | |

**Esito: ipotesi respinta.** Il segnale della prima ora era rumore, o un regime durato meno di un'ora. Anche la soglia a
10 SOL, l'unica non significativamente negativa, è compatibile con zero e non va riaperta senza un nuovo holdout.

## 6. Sniping alla migrazione

**Le regole dal 7 ott 2026 (synthetic migration).**
- L'acquisto `buy_v3` che completa la curva continua da solo "a prezzo di pool".
- `migrate_v2` è permissionless.
- Il pool apre in continuità di prezzo con la fine della curva (−0,024%).

Non c'è quindi un salto di prezzo da catturare: comprare subito dopo la migrazione è una scommessa di momentum sul token
graduato.

**Misurato.** 83 migrazioni non-mayhem (39 mayhem escluse); ingresso a fine slot M+L stimato dal pre-stato dei trade
PumpSwap successivi; fee 1,25% per lato, senza costi di rete.

| L | 1 s | 5 s | 10 s | 32 s | 1 min | 2,5 min | 5 min |
|---|---|---|---|---|---|---|---|
| 0 | −6,1% | −5,0% | −6,5% | −9,4% | −6,4% | −10,9% | −21,5% |
| 2 | −3,8% | −3,4% | −6,4% | −10,3% | −6,6% | −10,3% | −21,6% |

- A 1–2,5 minuti la mediana è positiva (+1/+3%) e il win rate ~52%, ma le cadute pesanti portano la media in negativo.
- A 5 minuti: media −22%, mediana −13%.
- Dal report di ricerca: su 31 migrazioni, 25 le esegue un unico signer, da 1 a 4 slot dopo il completamento. Le altre
  le fanno pochi wallet specializzati con un programma custom (migrate più acquisto nella stessa tx, tip Nozomi).

## 7. Cosa servirebbe tecnicamente, e quanto costa

Questa parte resta valida per qualunque strategia di esecuzione rapida, anche fuori dallo sniping.

| Livello | Rilevazione del create | Invio | L atteso | Costo fisso |
|---|---|---|---|---|
| Casa, gratis | PumpPortal `subscribeNewToken` (in tempo reale; 0,46 s prima dell'RPC pubblico in mediana, p90 4,1 s) | Helius Sender Francoforte/Amsterdam | 2–3 (stima) | 0 €/mese + tip |
| VPS a Francoforte | gRPC Yellowstone processed (Triton a consumo ~40–50 $/mese solo pump, Chainstack 98 $) | Sender, Jito, Nozomi | 1–2 (stima) | ~60–150 $/mese |
| Blocco 0 | shred: DoubleZero Edge ~450–1.500 $/mese (calcolo dal listino), AllenHark 50 $/mese (dichiarato dal venditore) | come sopra, colocation | 0–1 | da ~100 a >1.500 $/mese |
| Dentro la tx di create | — | — | impossibile per un terzo | — |

**Invio.**
- Tip minime: Jito 1.000 lamport; Helius Sender 0,001 SOL (oppure 0,000005 in modalità solo SWQoS, con meno percorsi);
  Nozomi 0,001 SOL (sotto soglia la tx viene scartata in silenzio); bloXroute 0,001 SOL.
- L'ordinamento è un'asta sulla tip in finestre da 20–50 ms (Jito/BAM ~37% dello stake). Arrivare prima conta solo a
  parità di tip.
- Francoforte ha il 35% degli slot leader.

**Feed.**
- L'RPC pubblico non va bene per decidere: in 4 ore ogni suo WebSocket si è chiuso 70–107 volte (errore 1002, una
  volta ogni 2–3 minuti; PumpPortal solo 3), e ogni chiusura perde 5–50 slot.
- Una sola connessione programSubscribe riceve il 93–97% delle notifiche dell'unione di due connessioni. Con l'unione,
  il 97% degli slot con trade ha la notifica, con stato identico al 99,7%.

**Cosa non si misura senza wallet.** La distribuzione reale di L per l'invio. Il modo previsto in `sim-model.md` §1.2:
50–100 tx sonda a costo minimo.

## 8. Rischi (se in futuro si riaprisse)

- **Esecuzione.** Dump di bundle in un solo slot: lo stop si riempie molto sotto la soglia. Le tx che falliscono pagano
  comunque base e priority.
- **Chiavi.** Un hot wallet sulla VM kraken starebbe dove n8n è già di fatto root (rischio noto del homelab). Il bot
  andrebbe in una VM o un container isolati, con poco capitale e una chiave dedicata.
- **IP di casa.** RPC pubblico e PumpPortal possono bannare l'IP, che è lo stesso di tutti i servizi del homelab.
- **Fisco (Italia, da confermare col commercialista).**
  - Plusvalenze al 33% dal 2026, senza franchigia.
  - Se lo scambio SOL→memecoin sia fiscalmente rilevante non è chiarito: la Circ. 30/E esclude solo le permute tra
    cripto con "eguali caratteristiche e funzioni".
  - Un costo non documentato vale zero e le minusvalenze si riportano per 4 anni.
  - Con migliaia di trade serve un registro di ogni operazione.
- **Da chi viene il guadagno.** Il profitto di uno sniper viene da chi compra dopo, quasi sempre piccoli trader che
  perdono. L'unica posizione che rende con continuità, comprare nella propria tx di create e rivendere a chi arriva, è
  il meccanismo di un pump-and-dump: non è un'opzione che consiglio.

## 9. Verdetto

1. **Sniper generico: no.** È negativo a ogni latenza raggiungibile, con IC sotto zero su ~2.800 token. La parte che
   rende (la tx di create e il bundle) non è accessibile a un terzo.
2. **Sniping selettivo sulle feature del create: no.** L'unico candidato è stato respinto dalla verifica pre-registrata
   (−7,1% netto su 374 lanci), e nessun'altra feature dà fasce positive.
3. **Sniping alla migrazione: no.** È negativo su 83 migrazioni.
4. **Niente feed a pagamento, VPS o shred** per questo scopo.
5. **Cosa resta utile:**
   - il registratore a doppia connessione e la ricostruzione degli stati per slot, riusabili nella Fase 1;
   - il dataset di 4 ore (stati di tutte le curve per slot, ~3.000 create, ~55.000 tx), utile per un primo controllo
     offline della variante V3 del momentum, che usa solo gli stati della curva.

---

## Misure

Tutte del 24 ott 2026 (sabato), dalla VM kraken.

- **Registratore** (19:44–23:51 UTC):
  - 7.955 create riusciti, 3.199 SOL non-mayhem.
  - 3.112 token con 10 minuti di storia: 2.836 utilizzabili per le statistiche, 2.793 nella simulazione, dopo aver
    escluso buchi del flusso vicino al create e notifiche di S0 mancanti.
  - 129 buchi del flusso, 1.929 slot su ~70.000 (2,8%).
- **Completezza del flusso:** 97,0% degli slot con trade (8.857 slot su 788 token), stato uguale nel 99,7%.
- **Backfill** su due worker (api.mainnet-beta e PublicNode):
  - 1.093 token campionati a caso (hash del mint), 989.438 firme, 55.597 tx scaricate con `getTransaction`;
  - indice nel blocco da `getSignaturesForAddress` (`transactionIndex`);
  - 242 token troncati a 150 tx riuscite.
- **Latenza:** logsSubscribe confirmed arriva dopo PumpPortal, con ritardo p50 0,46 s e p90 4,1 s. Lo `slotSubscribe`
  pubblico è a sua volta in ritardo sul vertice della catena: PumpPortal arriva prima che veda lo slot del create.
- **Tx fallite** campionate: 48 nei primi 5 slot di 8 token, 8 pagatori distinti, uno ne fa 36. Fee mediana 5.666
  lamport, CU mediane 8.361.
- **Verifica pre-registrata:** testo con SHA-256 `daa5601b…33719462`, scritto alle 20:37 UTC (slot 460.988.394). Holdout
  S0 > 460.990.000.

## Limiti

- **Una sola serata**, di sabato. Il regime cambia: Bitquery vede la presenza degli sniper calare dal 39% al 22% in
  dieci mesi. Il verdetto negativo però è coerente su tutta la sera e con i dati annuali di Bitquery.
- **Esecuzione.** Il modello a fine slot ignora la posizione dentro lo slot. Rispetto a L ignora anche i fallimenti per
  slippage e il rischio di non atterrare: entrambi peggiorerebbero i numeri.
- **Flusso curves.** Manca il 3% degli stati. Uno stato mancante viene sostituito dall'ultimo noto.
- **PnL per wallet.**
  - È a 5 minuti, ed esclude i token più attivi.
  - Il valore "a mercato" è ottimistico per chi detiene molto.
  - Wallet diversi della stessa persona contano separatamente.
- **Migrazioni.** Ingresso approssimato dal pre-stato del trade successivo; senza costi di rete.

## Fonti

- Misure proprie (vedi sotto).
- Bitquery, *Pump.fun snipers and bundles*: https://bitquery.io/investigations/pump-fun-snipers-bundles (letta il 24
  ott 2026).
- Jito, `getTipAccounts` (mainnet block engine) e https://docs.jito.wtf/lowlatencytxnsend/
- Helius Sender: https://www.helius.dev/docs/sending-transactions/sender
- Nozomi: https://use.temporal.xyz/nozomi/tipping-and-faq
- pump.fun public docs (IDL, SYNTHETIC_MIGRATION, FEE_PROGRAM_README): https://github.com/pump-fun/pump-public-docs
- BAM e Alpenglow: https://cryptobriefing.com/jito-bam-upgrade-cuts-bundle-delays/,
  https://solanacompass.com/news/jito-announces-bam-direct-bundle-ingest-routing-solana-bundles-past-the-block-engines-50ms-hop
- Leader a Francoforte: https://cryptobriefing.com/europe-solana-leader-slots-germany-dominance/
- DoubleZero Edge: https://crypto.news/solana-news-today-doublezero-launches-edge-beta/
- Fisco: Circ. AdE 30/E del 27/10/2023; L. 207/2024 c. 24–25; D.Lgs. 117/2026.

## File

- Script: `docs/research/sniping/` (registratore `rec.py`, backfill `backfill.py`, `build.py`, `sim.py`, `report.py`,
  `traders.py`, `filters.py`, `bundlecheck.py`, `holdout.py`, `migr.py`, `latency.py`, `validate.py`, `descr.py`,
  `failed.py`). Usano il decoder `docs/research/pumpspec_reference.py`.
- Ipotesi pre-registrata: `docs/research/sniping/PREREG.md`.
- Output delle analisi finali: `docs/research/sniping/risultati/`.
- Dati grezzi compressi con zstd (non versionati, 93 MB): `data/sniping-20261024/`. Per rigenerare:
  1. decomprimere in una cartella;
  2. `build.py <cartella> 2830`;
  3. poi gli altri script sulla stessa cartella.
