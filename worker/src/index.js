// van-allen-history — storico continuo per la dashboard delle fasce di Van Allen.
//
// La dashboard desktop registra un punto solo mentre è aperta. Questo Worker
// fa la stessa cosa 24 ore su 24: ogni 5 minuti legge le fonti pubbliche NOAA
// SWPC e salva in D1 le sole MISURE (flusso di elettroni, Kp, Dst, vento
// solare, campo interplanetario). Le grandezze di modello (magnetopausa,
// forma delle fasce) non sono salvate: le ricalcola l'app dalle misure, così
// la fisica resta scritta in un posto solo (core/src/physics.rs).
//
// Percorsi (sola lettura):
//   GET /history?since=<epoch>&limit=<n>   punti con t > since, in ordine di tempo
//   GET /latest                            ultimo punto
//   GET /health                            "ok" + numero di punti
//
// Scritto in JavaScript e non in Rust: su Workers il Rust diventa WebAssembly,
// più pesante da avviare a ogni giro del cron, e qui non c'è logica da
// condividere con l'app (solo lettura di JSON e una INSERT).

const BASE = "https://services.swpc.noaa.gov";

export const SOURCES = {
  electrons: "/json/goes/primary/integral-electrons-6-hour.json",
  kp: "/json/planetary_k_index_1m.json",
  dst: "/products/kyoto-dst.json",
  wind: "/json/rtsw/rtsw_wind_1m.json",
  mag: "/json/rtsw/rtsw_mag_1m.json",
};

/** Colonne della tabella, nell'ordine usato da /history. */
export const COLUMNS = ["t", "flux2", "kp", "dst", "bz", "bt", "speed", "density"];

const DAY = 86400;
const DEFAULT_WINDOW = 30 * DAY;
const MAX_ROWS = 20000;

// ---- lettura delle fonti ---------------------------------------------------

/** "2026-11-14T12:40:00", con o senza Z, con T o spazio → epoch in secondi. */
export function parseTime(tag) {
  if (typeof tag !== "string" || tag.length < 16) return null;
  let s = tag.trim().replace(" ", "T");
  if (!/[zZ]$/.test(s) && !/[+-]\d\d:?\d\d$/.test(s)) s += "Z";
  const ms = Date.parse(s);
  return Number.isFinite(ms) ? Math.floor(ms / 1000) : null;
}

/**
 * Valore più recente di `field` fra le righe che superano `keep`.
 * Restituisce null se non ce n'è nessuno valido.
 */
export function latest(rows, field, keep = () => true) {
  if (!Array.isArray(rows)) return null;
  let best = null;
  let bestT = -Infinity;
  for (const r of rows) {
    if (!r || typeof r !== "object" || !keep(r)) continue;
    const v = r[field];
    if (typeof v !== "number" || !Number.isFinite(v)) continue;
    const t = parseTime(r.time_tag);
    if (t === null || t < bestT) continue;
    best = v;
    bestT = t;
  }
  return best;
}

/**
 * Filtro "solo la sonda operativa". I file RTSW hanno, per lo stesso minuto,
 * una riga per ogni sonda al punto L1; quella in uso ha active: true. Se il
 * campo manca del tutto si tengono tutte le righe (stessa regola dell'app).
 */
export function activeOnly(rows) {
  const any = Array.isArray(rows) && rows.some((r) => r && r.active === true);
  return any ? (r) => r.active === true : () => true;
}

/** Dai cinque file già interpretati alle misure del punto. */
export function extract(data) {
  const kp = latest(data.kp, "estimated_kp");
  const flux2 = latest(data.electrons, "flux", (r) => r.energy === ">=2 MeV" && r.flux > 0);
  return {
    flux2,
    kp: kp === null ? null : Math.max(0, kp),
    dst: latest(data.dst, "dst"),
    bz: latest(data.mag, "bz_gsm", activeOnly(data.mag)),
    bt: latest(data.mag, "bt", activeOnly(data.mag)),
    speed: latest(data.wind, "proton_speed", activeOnly(data.wind)),
    density: latest(data.wind, "proton_density", activeOnly(data.wind)),
  };
}

/** Scarica le fonti in parallelo; una fonte che fallisce non blocca le altre. */
export async function collect(fetchFn = fetch) {
  const names = Object.keys(SOURCES);
  const settled = await Promise.allSettled(
    names.map(async (name) => {
      const res = await fetchFn(BASE + SOURCES[name], {
        headers: { "User-Agent": "van-allen-history-worker", Accept: "application/json" },
        signal: AbortSignal.timeout(20000),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      return res.json();
    }),
  );
  const data = {};
  const errors = [];
  settled.forEach((s, i) => {
    if (s.status === "fulfilled") data[names[i]] = s.value;
    else errors.push(`${names[i]}: ${s.reason?.message ?? s.reason}`);
  });
  return { point: extract(data), errors };
}

// ---- archivio --------------------------------------------------------------

/** Legge le fonti e salva un punto. `when` è l'ora del cron in millisecondi. */
export async function store(env, when) {
  const { point, errors } = await collect();
  const t = Math.floor(when / 60000) * 60;
  const values = COLUMNS.slice(1).map((c) => point[c]);
  if (values.every((v) => v === null)) {
    console.warn("nessuna fonte ha risposto, punto non salvato:", errors.join("; "));
    return { saved: false, t, errors };
  }
  await env.DB.prepare(
    `INSERT OR REPLACE INTO points (${COLUMNS.join(",")}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)`,
  )
    .bind(t, ...values)
    .run();
  if (errors.length) console.warn("fonti non raggiunte:", errors.join("; "));
  return { saved: true, t, errors };
}

// ---- HTTP ------------------------------------------------------------------

const CORS = { "Access-Control-Allow-Origin": "*" };

function json(body, status = 200, extra = {}) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", ...CORS, ...extra },
  });
}

function text(body, status = 200) {
  return new Response(body, {
    status,
    headers: { "Content-Type": "text/plain; charset=utf-8", ...CORS },
  });
}

/** Intero non negativo da un parametro, oppure `fallback`. */
function intParam(value, fallback) {
  if (value === null || !/^\d{1,12}$/.test(value)) return fallback;
  return Number(value);
}

export async function handle(request, env, now = Date.now()) {
  if (request.method !== "GET" && request.method !== "HEAD") {
    return text("solo GET\n", 405);
  }
  const url = new URL(request.url);
  const nowS = Math.floor(now / 1000);

  switch (url.pathname) {
    case "/history": {
      const since = intParam(url.searchParams.get("since"), nowS - DEFAULT_WINDOW);
      const limit = Math.min(Math.max(intParam(url.searchParams.get("limit"), MAX_ROWS), 1), MAX_ROWS);
      const points = await env.DB.prepare(
        `SELECT ${COLUMNS.join(",")} FROM points WHERE t > ?1 ORDER BY t LIMIT ?2`,
      )
        .bind(since, limit)
        .raw();
      return json(
        { columns: COLUMNS, since, count: points.length, points },
        200,
        { "Cache-Control": "public, max-age=120" },
      );
    }
    case "/latest": {
      const row = await env.DB.prepare(
        `SELECT ${COLUMNS.join(",")} FROM points ORDER BY t DESC LIMIT 1`,
      ).first();
      return row ? json(row, 200, { "Cache-Control": "public, max-age=60" }) : json({ error: "archivio vuoto" }, 404);
    }
    case "/health": {
      const row = await env.DB.prepare(
        "SELECT COUNT(*) AS n, MIN(t) AS first, MAX(t) AS last FROM points",
      ).first();
      const age = row?.last ? nowS - row.last : null;
      // il cron gira ogni 5 minuti: oltre 20 minuti senza punti nuovi è un guasto
      const ok = age !== null && age < 1200;
      return json({ ok, points: row?.n ?? 0, first: row?.first ?? null, last: row?.last ?? null, age_s: age }, ok ? 200 : 503);
    }
    case "/":
      return text(
        "van-allen-history: storico delle misure per la dashboard delle fasce di Van Allen\n" +
          "  /history?since=<epoch>&limit=<n>\n  /latest\n  /health\n" +
          "codice: https://github.com/gmy77/van-allen-dashboard\n",
      );
    default:
      return text("404: percorsi disponibili /history, /latest, /health\n", 404);
  }
}

export default {
  async scheduled(event, env, ctx) {
    ctx.waitUntil(store(env, event.scheduledTime));
  },
  async fetch(request, env) {
    try {
      return await handle(request, env);
    } catch (e) {
      console.error(e);
      return json({ error: "errore interno" }, 500);
    }
  },
};
