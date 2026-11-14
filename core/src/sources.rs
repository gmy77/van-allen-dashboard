//! Lettura e normalizzazione delle sorgenti live.
//!
//! Ogni parser è tollerante: campo mancante o nullo diventa `None`, non un errore
//! fatale. Le fonti sono tutte pubbliche e senza chiave.

use crate::time::parse_iso_utc;
use serde_json::Value;

pub const URL_GOES_E_DIFF: &str =
    "https://services.swpc.noaa.gov/json/goes/primary/differential-electrons-6-hour.json";
pub const URL_GOES_E_INT_1D: &str =
    "https://services.swpc.noaa.gov/json/goes/primary/integral-electrons-1-day.json";
pub const URL_GOES_E_INT_6H: &str =
    "https://services.swpc.noaa.gov/json/goes/primary/integral-electrons-6-hour.json";
pub const URL_GOES_P_INT_6H: &str =
    "https://services.swpc.noaa.gov/json/goes/primary/integral-protons-6-hour.json";
pub const URL_GOES_MAG_6H: &str =
    "https://services.swpc.noaa.gov/json/goes/primary/magnetometers-6-hour.json";
pub const URL_WIND: &str = "https://services.swpc.noaa.gov/json/rtsw/rtsw_wind_1m.json";
pub const URL_MAG: &str = "https://services.swpc.noaa.gov/json/rtsw/rtsw_mag_1m.json";
pub const URL_KP: &str = "https://services.swpc.noaa.gov/json/planetary_k_index_1m.json";
pub const URL_DST_KYOTO: &str = "https://services.swpc.noaa.gov/products/kyoto-dst.json";
pub const URL_DST_MIN: &str = "https://services.swpc.noaa.gov/json/geospace/geospace_dst_1_hour.json";
pub const URL_SCALES: &str = "https://services.swpc.noaa.gov/products/noaa-scales.json";

/// Un campione (t epoch UTC, valore).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub t: i64,
    pub v: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumPoint {
    pub energy_mev: f64,
    pub j: f64,
}

fn field_f(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|x| x.as_f64())
}

fn field_s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str())
}

fn rows(v: &Value) -> &[Value] {
    v.as_array().map(|a| a.as_slice()).unwrap_or(&[])
}

/// "79 keV" -> 0.079, ">=2 MeV" -> 2.0, ">=800 keV" -> 0.8
pub fn parse_energy_mev(s: &str) -> Option<f64> {
    let t = s.trim().trim_start_matches(">=").trim();
    let mut it = t.split_whitespace();
    let value: f64 = it.next()?.trim().parse().ok()?;
    let unit = it.next().unwrap_or("MeV").to_ascii_lowercase();
    let mev = if unit.starts_with("kev") {
        value / 1000.0
    } else if unit.starts_with("gev") {
        value * 1000.0
    } else {
        value
    };
    if mev.is_finite() && mev > 0.0 {
        Some(mev)
    } else {
        None
    }
}

/// Costruisce una serie da un array di oggetti con `time_tag` e un campo numerico.
fn series(v: &Value, key: &str) -> Vec<Sample> {
    let mut out: Vec<Sample> = rows(v)
        .iter()
        .filter_map(|r| {
            let t = parse_iso_utc(field_s(r, "time_tag")?)?;
            let v = field_f(r, key)?;
            if v.is_finite() { Some(Sample { t, v }) } else { None }
        })
        .collect();
    out.sort_by_key(|s| s.t);
    out
}

/// Come [`series`], ma tiene solo le righe del satellite attivo.
///
/// I file RTSW contengono, per lo stesso minuto, una riga per ogni sonda al
/// punto L1 (quella operativa ha `active: true`, le altre sono di riserva).
/// Mescolarle produce una serie a dente di sega e un "valore attuale" che può
/// venire dalla sonda sbagliata. Se il campo `active` manca del tutto si
/// ricade su tutte le righe.
fn series_active(v: &Value, key: &str) -> Vec<Sample> {
    let is_active = |r: &Value| r.get("active").and_then(|x| x.as_bool()) == Some(true);
    if !rows(v).iter().any(is_active) {
        return series(v, key);
    }
    let mut out: Vec<Sample> = rows(v)
        .iter()
        .filter(|r| is_active(r))
        .filter_map(|r| {
            let t = parse_iso_utc(field_s(r, "time_tag")?)?;
            let v = field_f(r, key)?;
            if v.is_finite() { Some(Sample { t, v }) } else { None }
        })
        .collect();
    out.sort_by_key(|s| s.t);
    out.dedup_by_key(|s| s.t);
    out
}

/// Sigla della sonda attiva (o, in mancanza, della prima riga con una sigla).
fn active_source(v: &Value) -> String {
    rows(v)
        .iter()
        .find(|r| r.get("active").and_then(|x| x.as_bool()) == Some(true))
        .and_then(|r| field_s(r, "source"))
        .or_else(|| rows(v).iter().find_map(|r| field_s(r, "source")))
        .unwrap_or("")
        .to_string()
}

/// Serie filtrata per satellite e (opzionalmente) per canale energetico.
fn goes_series(v: &Value, key: &str, sat: Option<u8>, energy: Option<&str>) -> Vec<Sample> {
    let mut out: Vec<Sample> = rows(v)
        .iter()
        .filter(|r| {
            if let Some(s) = sat {
                if r.get("satellite").and_then(|x| x.as_u64()).map(|x| x as u8) != Some(s) {
                    return false;
                }
            }
            if let Some(e) = energy {
                if field_s(r, "energy") != Some(e) {
                    return false;
                }
            }
            true
        })
        .filter_map(|r| {
            let t = parse_iso_utc(field_s(r, "time_tag")?)?;
            let v = field_f(r, key)?;
            if v.is_finite() && v > 0.0 {
                Some(Sample { t, v })
            } else {
                None
            }
        })
        .collect();
    out.sort_by_key(|s| s.t);
    out
}

/// Satellite GOES presente nella risposta (il più frequente fra le righe valide).
fn goes_satellite(v: &Value) -> u8 {
    let mut counts: Vec<(u8, usize)> = Vec::new();
    for r in rows(v) {
        if let Some(s) = r.get("satellite").and_then(|x| x.as_u64()) {
            let s = s as u8;
            match counts.iter_mut().find(|(k, _)| *k == s) {
                Some((_, c)) => *c += 1,
                None => counts.push((s, 1)),
            }
        }
    }
    counts.into_iter().max_by_key(|(_, c)| *c).map(|(s, _)| s).unwrap_or(0)
}

/// Spettro differenziale nella forma {energie -> ultimo campione disponibile}.
fn latest_spectrum(v: &Value) -> (Vec<SpectrumPoint>, i64) {
    // t -> (somma? no) : prendiamo, per ciascun canale, l'ultimo campione.
    let mut by_energy: Vec<(f64, Sample)> = Vec::new();
    for r in rows(v) {
        let Some(t) = field_s(r, "time_tag").and_then(parse_iso_utc) else {
            continue;
        };
        let Some(e) = field_s(r, "energy").and_then(parse_energy_mev) else {
            continue;
        };
        let Some(j) = field_f(r, "flux") else { continue };
        if !(j.is_finite() && j > 0.0) {
            continue;
        }
        match by_energy.iter_mut().find(|(k, _)| (*k - e).abs() < 1e-9) {
            Some((_, s)) => {
                if t >= s.t {
                    *s = Sample { t, v: j }
                }
            }
            None => by_energy.push((e, Sample { t, v: j })),
        }
    }
    let at = by_energy.iter().map(|(_, s)| s.t).max().unwrap_or(0);
    let mut pts: Vec<SpectrumPoint> = by_energy
        .into_iter()
        .map(|(e, s)| SpectrumPoint {
            energy_mev: e,
            j: s.v,
        })
        .collect();
    pts.sort_by(|a, b| a.energy_mev.partial_cmp(&b.energy_mev).unwrap());
    (pts, at)
}

/// Spettro differenziale medio nella prima mezz'ora coperta dal file.
///
/// Serve al confronto "prima / adesso" nella scheda dello spettro. Non si può
/// cercare "6 ore fa" perché il file differenziale dura esattamente 6 ore: in
/// quel punto non c'è nulla. Si prende invece l'inizio della finestra, mediando
/// su ~30 minuti per smorzare il rumore dei singoli campioni.
fn spectrum_start(v: &Value) -> Vec<SpectrumPoint> {
    let t0 = rows(v)
        .iter()
        .filter_map(|r| field_s(r, "time_tag").and_then(parse_iso_utc))
        .min();
    let Some(t0) = t0 else { return Vec::new() };

    let mut acc: Vec<(f64, f64, usize)> = Vec::new(); // energia, somma log10, conteggio
    for r in rows(v) {
        let Some(t) = field_s(r, "time_tag").and_then(parse_iso_utc) else {
            continue;
        };
        if t > t0 + 30 * 60 {
            continue;
        }
        let Some(e) = field_s(r, "energy").and_then(parse_energy_mev) else {
            continue;
        };
        let Some(j) = field_f(r, "flux") else { continue };
        if !(j.is_finite() && j > 0.0) {
            continue;
        }
        match acc.iter_mut().find(|(k, _, _)| (*k - e).abs() < 1e-9) {
            Some((_, sum, n)) => {
                *sum += j.log10();
                *n += 1;
            }
            None => acc.push((e, j.log10(), 1)),
        }
    }
    let mut pts: Vec<SpectrumPoint> = acc
        .into_iter()
        .map(|(e, sum, n)| SpectrumPoint {
            energy_mev: e,
            j: 10f64.powf(sum / n as f64),
        })
        .collect();
    pts.sort_by(|a, b| a.energy_mev.partial_cmp(&b.energy_mev).unwrap());
    pts
}

#[derive(Clone, Debug, Default)]
pub struct GoesElectrons {
    pub satellite: u8,
    pub integral_2mev: Vec<Sample>,
    pub integral_800kev: Vec<Sample>,
    pub spectrum: Vec<SpectrumPoint>,
    pub spectrum_at: i64,
    pub spectrum_prev: Vec<SpectrumPoint>,
}

impl GoesElectrons {
    pub fn latest(&self) -> Option<Sample> {
        self.integral_2mev.last().copied()
    }

    pub fn latest_800kev(&self) -> Option<Sample> {
        self.integral_800kev.last().copied()
    }
}

#[derive(Clone, Debug, Default)]
pub struct GoesProtons {
    pub satellite: u8,
    pub p1: Vec<Sample>,
    pub p10: Vec<Sample>,
    pub p100: Vec<Sample>,
}

impl GoesProtons {
    pub fn latest_10mev(&self) -> Option<Sample> {
        self.p10.last().copied()
    }
}

#[derive(Clone, Debug, Default)]
pub struct GoesMag {
    pub satellite: u8,
    pub hp: Vec<Sample>,
}

/// Vento solare al punto L1.
#[derive(Clone, Debug, Default)]
pub struct SolarWind {
    pub speed: Vec<Sample>,
    pub density: Vec<Sample>,
    pub temperature: Vec<Sample>,
    pub source: String,
}

impl SolarWind {
    pub fn now_speed(&self) -> Option<f64> {
        self.speed.last().map(|s| s.v)
    }
    pub fn now_density(&self) -> Option<f64> {
        self.density.last().map(|s| s.v)
    }
    pub fn now_temperature(&self) -> Option<f64> {
        self.temperature.last().map(|s| s.v)
    }

    /// Pressione dinamica nel tempo (nPa), dai minuti in cui ci sono sia
    /// densità sia velocità.
    pub fn pressure(&self) -> Vec<Sample> {
        let mut out = Vec::with_capacity(self.speed.len().min(self.density.len()));
        let mut j = 0;
        for d in &self.density {
            while j < self.speed.len() && self.speed[j].t < d.t {
                j += 1;
            }
            if let Some(v) = self.speed.get(j) {
                if v.t == d.t {
                    out.push(Sample {
                        t: d.t,
                        v: crate::physics::dynamic_pressure_npa(d.v, v.v),
                    });
                }
            }
        }
        out
    }
}

#[derive(Clone, Debug, Default)]
pub struct Imf {
    pub bt: Vec<Sample>,
    pub bz: Vec<Sample>,
    pub source: String,
}

impl Imf {
    pub fn now_bz(&self) -> Option<f64> {
        self.bz.last().map(|s| s.v)
    }
    pub fn now_bt(&self) -> Option<f64> {
        self.bt.last().map(|s| s.v)
    }
}

#[derive(Clone, Debug, Default)]
pub struct KpNow {
    pub kp: f64,
    pub estimated: f64,
    pub at: i64,
    pub series: Vec<Sample>,
}

#[derive(Clone, Debug, Default)]
pub struct DstSeries {
    pub series: Vec<Sample>,
    pub official: Option<Sample>,
    pub minute: Vec<Sample>,
}

impl DstSeries {
    /// Dst da mostrare: l'ultimo valore orario di Kyoto (misura). Solo se
    /// manca si ripiega sul nowcast del modello Geospace.
    pub fn now(&self) -> Option<f64> {
        self.series
            .last()
            .map(|s| s.v)
            .or_else(|| self.nowcast(crate::time::now_unix()))
    }

    /// Nowcast del modello Geospace all'istante `now`.
    ///
    /// Il file arriva fino a ~45 minuti nel futuro (il modello propaga il
    /// vento solare da L1 alla Terra): l'ultimo campione è una previsione,
    /// quindi si prende l'ultimo con tempo non successivo ad adesso.
    pub fn nowcast(&self, now: i64) -> Option<f64> {
        self.minute
            .iter()
            .rev()
            .find(|s| s.t <= now)
            .or(self.minute.first())
            .map(|s| s.v)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Scales {
    pub r: u8,
    pub s: u8,
    pub g: u8,
    pub r_text: String,
    pub g_text: String,
    pub at: String,
}

// ---- parser -------------------------------------------------------------

pub fn parse_goes_electrons(int_1d: &Value, diff_6h: &Value) -> GoesElectrons {
    let satellite = goes_satellite(diff_6h);
    let sat = if satellite == 0 { None } else { Some(satellite) };
    let spectrum_at = field_s(diff_6h.as_array().and_then(|a| a.last()).unwrap_or(&Value::Null), "time_tag")
        .and_then(parse_iso_utc)
        .unwrap_or(0);

    let spectrum_prev = spectrum_start(diff_6h);
    let (spectrum, spectrum_at2) = latest_spectrum(diff_6h);
    let spectrum_at = if spectrum_at == 0 {
        spectrum_at2
    } else {
        spectrum_at
    };

    // Preferisce il canale >= 2 MeV; se il satellite non lo pubblica, usa qualunque energia >= 2 MeV.
    let mut integral_2mev = goes_series(int_1d, "flux", sat, Some(">=2 MeV"));
    if integral_2mev.is_empty() {
        integral_2mev = rows(int_1d)
            .iter()
            .filter(|r| {
                field_s(r, "energy")
                    .and_then(parse_energy_mev)
                    .map(|e| e >= 1.9)
                    .unwrap_or(false)
                    && sat.map(|s| r.get("satellite").and_then(|x| x.as_u64()) == Some(s as u64))
                        .unwrap_or(true)
            })
            .filter_map(|r| {
                Some(Sample {
                    t: parse_iso_utc(field_s(r, "time_tag")?)?,
                    v: field_f(r, "flux").filter(|v| v.is_finite() && *v > 0.0)?,
                })
            })
            .collect();
        integral_2mev.sort_by_key(|s| s.t);
    }

    let mut integral_800kev = goes_series(int_1d, "flux", sat, Some(">=0.8 MeV"));
    if integral_800kev.is_empty() {
        integral_800kev = rows(int_1d)
            .iter()
            .filter(|r| {
                field_s(r, "energy")
                    .and_then(parse_energy_mev)
                    .map(|e| (0.75..0.9).contains(&e))
                    .unwrap_or(false)
            })
            .filter_map(|r| {
                Some(Sample {
                    t: parse_iso_utc(field_s(r, "time_tag")?)?,
                    v: field_f(r, "flux").filter(|v| v.is_finite() && *v > 0.0)?,
                })
            })
            .collect();
        integral_800kev.sort_by_key(|s| s.t);
    }

    GoesElectrons {
        satellite,
        integral_2mev,
        integral_800kev,
        spectrum,
        spectrum_at,
        spectrum_prev,
    }
}

pub fn parse_goes_protons(v: &Value) -> GoesProtons {
    let satellite = goes_satellite(v);
    let sat = if satellite == 0 { None } else { Some(satellite) };
    GoesProtons {
        satellite,
        p1: goes_series(v, "flux", sat, Some(">=1 MeV")),
        p10: goes_series(v, "flux", sat, Some(">=10 MeV")),
        p100: goes_series(v, "flux", sat, Some(">=100 MeV")),
    }
}

pub fn parse_goes_mag(v: &Value) -> GoesMag {
    let satellite = goes_satellite(v);
    let sat = if satellite == 0 { None } else { Some(satellite) };
    GoesMag {
        satellite,
        hp: goes_series(v, "Hp", sat, None),
    }
}

pub fn parse_wind(v: &Value) -> SolarWind {
    SolarWind {
        speed: series_active(v, "proton_speed"),
        density: series_active(v, "proton_density"),
        temperature: series_active(v, "proton_temperature"),
        source: active_source(v),
    }
}

pub fn parse_mag(v: &Value) -> Imf {
    Imf {
        bt: series_active(v, "bt"),
        bz: series_active(v, "bz_gsm"),
        source: active_source(v),
    }
}

pub fn parse_kp(v: &Value) -> KpNow {
    let series_all = series(v, "estimated_kp");
    let last = rows(v).last();
    KpNow {
        kp: last
            .and_then(|r| field_f(r, "kp_index"))
            .or_else(|| series_all.last().map(|s| s.v))
            .unwrap_or(0.0),
        estimated: series_all.last().map(|s| s.v).unwrap_or(0.0),
        at: series_all.last().map(|s| s.t).unwrap_or(0),
        series: series_all,
    }
}

pub fn parse_dst_kyoto(v: &Value) -> Vec<Sample> {
    series(v, "dst")
}

pub fn parse_dst_minute(v: &Value) -> Vec<Sample> {
    series(v, "dst")
}

pub fn parse_scales(v: &Value) -> Option<Scales> {
    let r = v.get("0")?;
    let num = |x: Option<&Value>| -> u8 {
        x.and_then(|s| s.get("Scale"))
            .and_then(|s| s.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };
    let text = |x: Option<&Value>| -> String {
        x.and_then(|s| s.get("Text"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string()
    };
    Some(Scales {
        r: num(r.get("R")),
        s: num(r.get("S")),
        g: num(r.get("G")),
        r_text: text(r.get("R")),
        g_text: text(r.get("G")),
        at: format!(
            "{}T{}",
            r.get("DateStamp").and_then(|x| x.as_str()).unwrap_or(""),
            r.get("TimeStamp").and_then(|x| x.as_str()).unwrap_or("")
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energies() {
        assert_eq!(parse_energy_mev("79 keV"), Some(0.079));
        assert_eq!(parse_energy_mev(">=2 MeV"), Some(2.0));
        assert_eq!(parse_energy_mev(">=800 keV"), Some(0.8));
        assert_eq!(parse_energy_mev("2894 keV"), Some(2.894));
        assert_eq!(parse_energy_mev("boh"), None);
    }

    #[test]
    fn spectrum_latest() {
        let raw: Value = serde_json::from_str(
            r#"[
              {"time_tag":"2026-11-14T11:00:00Z","satellite":19,"flux":10.0,"energy":"79 keV"},
              {"time_tag":"2026-11-14T11:05:00Z","satellite":19,"flux":20.0,"energy":"79 keV"},
              {"time_tag":"2026-11-14T11:05:00Z","satellite":19,"flux":2.0,"energy":"1509 keV"}
            ]"#,
        )
        .unwrap();
        let (pts, at) = latest_spectrum(&raw);
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0].j, 20.0);
        assert_eq!(pts[1].energy_mev, 1.509);
        assert_eq!(at, 1_794_654_300);
    }

    #[test]
    fn spectrum_start_average() {
        let raw: Value = serde_json::from_str(
            r#"[
              {"time_tag":"2026-11-14T05:00:00Z","satellite":19,"flux":10.0,"energy":"79 keV"},
              {"time_tag":"2026-11-14T05:10:00Z","satellite":19,"flux":100.0,"energy":"79 keV"},
              {"time_tag":"2026-11-14T06:00:00Z","satellite":19,"flux":1000.0,"energy":"79 keV"},
              {"time_tag":"2026-11-14T11:00:00Z","satellite":19,"flux":9000.0,"energy":"79 keV"}
            ]"#,
        )
        .unwrap();
        let pts = spectrum_start(&raw);
        assert_eq!(pts.len(), 1);
        // media geometrica dei soli campioni nella prima mezz'ora: 10 e 100 -> 31.6
        assert!((pts[0].j - 31.622_776).abs() < 0.01, "j = {}", pts[0].j);
    }

    #[test]
    fn wind_keeps_only_the_active_probe() {
        let raw: Value = serde_json::from_str(
            r#"[
              {"time_tag":"2026-11-14T12:40:00","active":true,"source":"SOLAR1","proton_speed":411.3,"proton_density":2.0,"proton_temperature":1e5},
              {"time_tag":"2026-11-14T12:40:00","active":false,"source":"ACE","proton_speed":431.0,"proton_density":2.7,"proton_temperature":9e4},
              {"time_tag":"2026-11-14T12:39:00","active":true,"source":"SOLAR1","proton_speed":410.0,"proton_density":null,"proton_temperature":1e5},
              {"time_tag":"2026-11-14T12:39:00","active":false,"source":"ACE","proton_speed":430.0,"proton_density":2.6,"proton_temperature":9e4}
            ]"#,
        )
        .unwrap();
        let w = parse_wind(&raw);
        assert_eq!(w.source, "SOLAR1");
        assert_eq!(w.speed.len(), 2);
        assert_eq!(w.now_speed(), Some(411.3));
        assert_eq!(w.density.len(), 1);
        // pressione solo dove ci sono entrambe le grandezze
        let p = w.pressure();
        assert_eq!(p.len(), 1);
        assert!((p[0].v - 1.672_621_9e-6 * 2.0 * 411.3 * 411.3).abs() < 1e-9);
    }

    #[test]
    fn wind_without_active_flag_uses_everything() {
        let raw: Value = serde_json::from_str(
            r#"[{"time_tag":"2026-11-14T12:40:00","source":"ACE","proton_speed":431.0}]"#,
        )
        .unwrap();
        let w = parse_wind(&raw);
        assert_eq!(w.now_speed(), Some(431.0));
        assert_eq!(w.source, "ACE");
    }

    #[test]
    fn dst_prefers_kyoto_and_skips_forecast() {
        let d = DstSeries {
            series: vec![Sample { t: 3600, v: -4.0 }],
            official: None,
            minute: vec![
                Sample { t: 3600, v: -6.0 },
                Sample { t: 5400, v: -7.0 },
                Sample { t: 9000, v: -30.0 },
            ],
        };
        assert_eq!(d.now(), Some(-4.0));
        assert_eq!(d.nowcast(6000), Some(-7.0));
        let only_model = DstSeries { series: vec![], ..d.clone() };
        assert!(only_model.now().is_some());
    }

    #[test]
    fn scales_parse() {
        let raw: Value = serde_json::from_str(
            r#"{"0":{"DateStamp":"2026-11-14","TimeStamp":"11:06:00",
                 "R":{"Scale":"0","Text":"none"},"S":{"Scale":"1","Text":"minor"},
                 "G":{"Scale":"2","Text":"moderate"}}}"#,
        )
        .unwrap();
        let s = parse_scales(&raw).unwrap();
        assert_eq!((s.r, s.s, s.g), (0, 1, 2));
        assert_eq!(s.g_text, "moderate");
    }
}