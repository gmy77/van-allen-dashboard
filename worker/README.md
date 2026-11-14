# van-allen-history — storico continuo su Cloudflare

La dashboard registra un punto solo mentre è aperta. Questo Worker fa la stessa
cosa 24 ore su 24: ogni 5 minuti legge le fonti pubbliche NOAA SWPC e salva le
misure in un database D1. L'app, se conosce l'indirizzo del Worker, scarica i
punti mancanti all'avvio (ultimi 30 giorni) e poi ogni mezz'ora, e la scheda
**Storico** non ha più buchi.

È facoltativo: senza Worker la dashboard funziona come prima.

## Cosa salva

Solo **misure**, una riga ogni 5 minuti:

| Colonna | Contenuto |
|---|---|
| `t` | istante, epoch Unix in secondi (UTC) |
| `flux2` | elettroni >2 MeV a L = 6.6, pfu (GOES) |
| `kp` | Kp stimato al minuto |
| `dst` | Dst orario di Kyoto, nT |
| `bz`, `bt` | campo interplanetario a L1, nT |
| `speed`, `density` | vento solare a L1, km/s e cm⁻³ |

Le grandezze di **modello** (magnetopausa, picco, bordo e contenuto della
fascia) non sono salvate: le ricalcola l'app dalle misure con la stessa fisica
della scheda Fasce (`core/src/remote.rs`), così le formule stanno in un posto
solo. Un anno di dati sono circa 105 000 righe, pochi megabyte.

## Percorsi

Tutti in sola lettura, senza chiave.

| Percorso | Risposta |
|---|---|
| `/history?since=<epoch>&limit=<n>` | punti con `t > since` in ordine di tempo; senza `since` gli ultimi 30 giorni; al massimo 20 000 righe |
| `/latest` | ultimo punto |
| `/health` | numero di punti e età dell'ultimo; stato 503 se l'archivio è fermo da più di 20 minuti |

## Requisiti

* un account Cloudflare con **Workers Paid**: leggere i due file del vento
  solare (24 ore al minuto, più sonde) costa qualche decina di millisecondi di
  CPU a ogni giro, oltre i 10 ms del piano Free;
* `wrangler` 4 e Node 20 o successivo.

## Pubblicazione

Da questa cartella, una volta sola:

```powershell
wrangler d1 create van-allen-history
```

Il comando stampa un `database_id`: va copiato in `wrangler.toml` al posto di
`DA-COMPILARE-DOPO-wrangler-d1-create`. Poi:

```powershell
wrangler d1 execute van-allen-history --remote --file schema.sql
wrangler deploy
```

`wrangler deploy` stampa l'indirizzo del Worker
(`https://van-allen-history.<account>.workers.dev`). Dopo 5 minuti
`/health` deve rispondere `"ok": true`.

## Collegare l'app

Scrivi l'indirizzo, da solo su una riga, nel file

```
%LOCALAPPDATA%\van-allen-dashboard\remote.txt
```

oppure imposta la variabile d'ambiente `VANALLEN_REMOTE`. Al prossimo avvio la
scheda **Sorgenti** mostra la riga «Storico remoto» in verde con i punti
ricevuti. È accettato solo `https://` (o `http://127.0.0.1` per le prove).

## Prove

```powershell
node test.mjs            # anche contro le fonti NOAA vere
node test.mjs --offline  # solo dati finti
```

Le prove non usano `wrangler`: fanno girare il cron e i percorsi HTTP contro un
D1 finto in memoria.

## Perché JavaScript

Il resto del progetto è in Rust. Su Workers il Rust diventa WebAssembly, più
pesante da avviare a ogni giro del cron, e qui non c'è logica da condividere
con l'app: solo lettura di JSON e una `INSERT`. La parte che conta, la fisica,
resta in Rust nel core.
