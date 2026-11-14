-- Un punto ogni 5 minuti. Solo misure: le grandezze di modello le ricalcola l'app.
--   t        epoch Unix in secondi (UTC), arrotondato al minuto
--   flux2    elettroni >2 MeV a L = 6.6, pfu (GOES)
--   kp       Kp stimato al minuto (NOAA)
--   dst      Dst orario di Kyoto, nT
--   bz, bt   campo interplanetario a L1, nT (Bz in GSM)
--   speed    velocità del vento solare a L1, km/s
--   density  densità dei protoni a L1, cm^-3
CREATE TABLE IF NOT EXISTS points (
  t       INTEGER PRIMARY KEY,
  flux2   REAL,
  kp      REAL,
  dst     REAL,
  bz      REAL,
  bt      REAL,
  speed   REAL,
  density REAL
) WITHOUT ROWID;
