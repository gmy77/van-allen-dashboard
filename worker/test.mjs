// Prova del Worker senza wrangler: node worker/test.mjs
//
// 1. controlla le funzioni di lettura su dati finti;
// 2. scarica le fonti NOAA vere e verifica che ogni misura sia presente;
// 3. fa girare cron e percorsi HTTP contro un D1 finto in memoria.
//
// Con --offline salta il punto 2 e usa dati finti anche per il cron.

import assert from "node:assert/strict";
import worker, { COLUMNS, activeOnly, collect, extract, handle, latest, parseTime, store } from "./src/index.js";

const offline = process.argv.includes("--offline");

// ---- D1 finto: capisce solo le quattro query del Worker ---------------------
function fakeD1() {
  const rows = new Map();
  const sorted = () => [...rows.values()].sort((a, b) => a.t - b.t);
  return {
    rows,
    prepare(sql) {
      let args = [];
      const stmt = {
        bind(...a) {
          args = a;
          return stmt;
        },
        async run() {
          assert.match(sql, /^INSERT OR REPLACE INTO points/);
          assert.equal(args.length, COLUMNS.length);
          rows.set(args[0], Object.fromEntries(COLUMNS.map((c, i) => [c, args[i]])));
          return { success: true };
        },
        async raw() {
          assert.match(sql, /WHERE t > \?1 ORDER BY t LIMIT \?2/);
          return sorted()
            .filter((r) => r.t > args[0])
            .slice(0, args[1])
            .map((r) => COLUMNS.map((c) => r[c]));
        },
        async first() {
          const all = sorted();
          if (/COUNT\(\*\)/.test(sql)) {
            return { n: all.length, first: all[0]?.t ?? null, last: all.at(-1)?.t ?? null };
          }
          return all.at(-1) ?? null;
        },
      };
      return stmt;
    },
  };
}

// ---- 1. funzioni pure --------------------------------------------------------
assert.equal(parseTime("2026-11-14T11:04:00Z"), 1794654240);
assert.equal(parseTime("2026-11-14T11:04:00"), 1794654240);
assert.equal(parseTime("2026-11-14 11:04:00"), 1794654240);
assert.equal(parseTime("boh"), null);

const wind = [
  { time_tag: "2026-11-14T12:40:00", active: true, source: "A", proton_speed: 411.3, proton_density: null },
  { time_tag: "2026-11-14T12:40:00", active: false, source: "B", proton_speed: 431.0, proton_density: 2.7 },
  { time_tag: "2026-11-14T12:39:00", active: true, source: "A", proton_speed: 410.0, proton_density: 2.0 },
];
assert.equal(latest(wind, "proton_speed", activeOnly(wind)), 411.3, "solo la sonda attiva");
assert.equal(latest(wind, "proton_density", activeOnly(wind)), 2.0, "salta i null");
assert.equal(latest([{ time_tag: "2026-11-14T12:40:00", proton_speed: 431 }], "proton_speed", activeOnly([{}])), 431);
assert.equal(latest(undefined, "x"), null);
assert.equal(latest("non un array", "x"), null);

const sample = {
  electrons: [
    { time_tag: "2026-11-14T12:30:00Z", satellite: 19, flux: 5000, energy: ">=2 MeV" },
    { time_tag: "2026-11-14T12:35:00Z", satellite: 19, flux: 5800, energy: ">=2 MeV" },
    { time_tag: "2026-11-14T12:35:00Z", satellite: 19, flux: 9e9, energy: ">=0.8 MeV" },
  ],
  kp: [{ time_tag: "2026-11-14T12:40:00", kp_index: 0, estimated_kp: 0.33 }],
  dst: [{ time_tag: "2026-11-14T12:00:00", dst: -4 }],
  wind,
  mag: [{ time_tag: "2026-11-14T12:40:00", active: true, bt: 3.8, bz_gsm: -1.2 }],
};
assert.deepEqual(extract(sample), { flux2: 5800, kp: 0.33, dst: -4, bz: -1.2, bt: 3.8, speed: 411.3, density: 2.0 });
assert.deepEqual(extract({}), { flux2: null, kp: null, dst: null, bz: null, bt: null, speed: null, density: null });
console.log("ok  funzioni di lettura");

// ---- 2. fonti vere -----------------------------------------------------------
const realFetch = globalThis.fetch;
const cannedFetch = async (url) => {
  const name = Object.keys(sample).find((k) => url.includes(k === "electrons" ? "integral-electrons" : k === "kp" ? "planetary_k" : k === "dst" ? "kyoto-dst" : k === "wind" ? "rtsw_wind" : "rtsw_mag"));
  return new Response(JSON.stringify(sample[name]), { status: 200 });
};

if (!offline) {
  const t0 = performance.now();
  const { point, errors } = await collect();
  console.log(`ok  fonti NOAA in ${Math.round(performance.now() - t0)} ms:`, JSON.stringify(point));
  assert.deepEqual(errors, [], "fonti non raggiunte");
  for (const [k, v] of Object.entries(point)) assert.ok(Number.isFinite(v), `${k} assente`);
  assert.ok(point.flux2 > 0 && point.speed > 150 && point.speed < 3000 && point.kp >= 0 && point.kp <= 9);
} else {
  globalThis.fetch = cannedFetch;
}

// ---- 3. cron e percorsi ------------------------------------------------------
const env = { DB: fakeD1() };
const now = Date.now();

// archivio vuoto
assert.equal((await handle(new Request("https://x/latest"), env, now)).status, 404);
assert.equal((await handle(new Request("https://x/health"), env, now)).status, 503);

// il cron salva un punto arrotondato al minuto
const waits = [];
await worker.scheduled({ scheduledTime: now }, env, { waitUntil: (p) => waits.push(p) });
await Promise.all(waits);
assert.equal(env.DB.rows.size, 1);
const t = Math.floor(now / 60000) * 60;
assert.ok(env.DB.rows.has(t));

// stesso minuto: sostituisce, non duplica
await store(env, now + 1000);
assert.equal(env.DB.rows.size, Math.floor((now + 1000) / 60000) * 60 === t ? 1 : 2);

// se nessuna fonte risponde non salva righe vuote
globalThis.fetch = async () => new Response("giù", { status: 503 });
const before = env.DB.rows.size;
const down = await store(env, now + 600000);
assert.equal(down.saved, false);
assert.equal(down.errors.length, 5);
assert.equal(env.DB.rows.size, before);
globalThis.fetch = offline ? cannedFetch : realFetch;

// /history
let res = await handle(new Request("https://x/history?since=0"), env, now);
assert.equal(res.status, 200);
assert.equal(res.headers.get("Access-Control-Allow-Origin"), "*");
let body = await res.json();
assert.deepEqual(body.columns, COLUMNS);
assert.equal(body.count, before);
assert.equal(body.points[0].length, COLUMNS.length);
assert.equal(body.points[0][0], t);

res = await handle(new Request(`https://x/history?since=${t + 3600}`), env, now);
assert.equal((await res.json()).count, 0, "since esclude i punti vecchi");

// parametri non validi: si usa il predefinito, niente errori
res = await handle(new Request("https://x/history?since=abc&limit=-5"), env, now);
assert.equal(res.status, 200);
body = await res.json();
assert.equal(body.since, Math.floor(now / 1000) - 30 * 86400);

// /latest, /health, resto
res = await handle(new Request("https://x/latest"), env, now);
assert.equal((await res.json()).t >= t, true);
res = await handle(new Request("https://x/health"), env, now);
assert.equal(res.status, 200);
assert.equal((await res.json()).ok, true);
assert.equal((await handle(new Request("https://x/health"), env, now + 3600000)).status, 503, "archivio fermo da un'ora");
assert.equal((await handle(new Request("https://x/"), env, now)).status, 200);
assert.equal((await handle(new Request("https://x/altro"), env, now)).status, 404);
assert.equal((await handle(new Request("https://x/history", { method: "POST" }), env, now)).status, 405);

// un errore del database non deve uscire come pagina di errore di Cloudflare
res = await worker.fetch(new Request("https://x/history"), { DB: { prepare() { throw new Error("D1 giù"); } } });
assert.equal(res.status, 500);

console.log("ok  cron e percorsi HTTP");
console.log("tutto a posto");
