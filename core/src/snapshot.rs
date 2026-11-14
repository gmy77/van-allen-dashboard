//! Aggiornamento completo: scarica tutte le fonti in parallelo, le normalizza,
//! calcola gli indicatori e produce la fotografia pronta per l'interfaccia.

use crate::http::Http;
use crate::physics::{self, BeltShape, StormLevel};
use crate::sources::{self, *};
use crate::time::now_unix;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Watch,
    Warn,
    Severe,
}

#[derive(Clone, Debug)]
pub struct Alert {
    pub severity: Severity,
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub fetched_at: i64,
    pub goes_e: GoesElectrons,
    pub goes_p: GoesProtons,
    pub goes_mag: GoesMag,
    pub wind: SolarWind,
    pub imf: Imf,
    pub kp: KpNow,
    pub dst: DstSeries,
    pub scales: Option<Scales>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Assessment {
    pub kp: Option<f64>,
    pub dst: Option<f64>,
    pub wind_speed: Option<f64>,
    pub wind_density: Option<f64>,
    pub bz: Option<f64>,
    pub bt: Option<f64>,
    pub pdyn: Option<f64>,
    pub r_mp: Option<f64>,
    pub geo_flux: Option<f64>,
    pub geo_flux_800: Option<f64>,
    pub protons_10mev: Option<f64>,
    pub geo_hp_nt: Option<f64>,
    pub belt: Option<BeltShape>,
    pub gamma: Option<f64>,
    pub energy_flux: Option<f64>,
    pub rate_1h: Option<f64>,
    pub rate_3h: Option<f64>,
    /// nowcast Dst del modello Geospace (non è la misura di Kyoto)
    pub dst_nowcast: Option<f64>,
    /// temperatura dei protoni del vento solare, K
    pub wind_temp: Option<f64>,
    pub storm: StormLevel,
    pub g_scale: u8,
    pub alerts: Vec<Alert>,
}

impl Snapshot {
    pub fn assess(&self) -> Assessment {
        let kp = if self.kp.at > 0 {
            Some(self.kp.estimated.max(0.0))
        } else {
            None
        };
        let dst = self.dst.now();
        let wind_speed = self.wind.now_speed();
        let wind_density = self.wind.now_density();
        let bz = self.imf.now_bz();
        let bt = self.imf.now_bt();

        let pdyn = match (wind_density, wind_speed) {
            (Some(n), Some(v)) => Some(physics::dynamic_pressure_npa(n, v)),
            _ => None,
        };
        let r_mp = match (bz, pdyn) {
            (Some(z), Some(p)) => Some(physics::magnetopause_standoff(z, p)),
            _ => None,
        };

        let geo = self.goes_e.latest().map(|s| s.v);
        let geo800 = self.goes_e.latest_800kev().map(|s| s.v);
        let belt = geo.map(|j| {
            physics::belt_shape(
                j,
                kp.unwrap_or(2.0),
                dst.unwrap_or(0.0),
                r_mp,
            )
        });

        let dst_value = dst.unwrap_or(0.0);
        let mut a = Assessment {
            kp,
            dst,
            wind_speed,
            wind_density,
            bz,
            bt,
            pdyn,
            r_mp,
            geo_flux: geo,
            geo_flux_800: geo800,
            protons_10mev: self.goes_p.latest_10mev().map(|s| s.v),
            geo_hp_nt: self.goes_mag.hp.last().map(|s| s.v),
            belt,
            gamma: physics::spectral_index(&self.goes_e.spectrum),
            energy_flux: physics::energy_flux_uw_m2(&self.goes_e.spectrum),
            rate_1h: physics::log10_rate(&self.goes_e.integral_2mev, 1.0),
            rate_3h: physics::log10_rate(&self.goes_e.integral_2mev, 3.0),
            dst_nowcast: self.dst.nowcast(self.fetched_at.max(1)),
            wind_temp: self.wind.now_temperature(),
            storm: StormLevel::from_dst(dst_value),
            g_scale: physics::g_scale(kp.unwrap_or(0.0)),
            alerts: Vec::new(),
        };
        a.alerts = build_alerts(&a, self);
        a
    }
}

fn build_alerts(a: &Assessment, s: &Snapshot) -> Vec<Alert> {
    let mut out = Vec::new();

    if let Some(kp) = a.kp {
        let g = physics::g_scale(kp);
        if g > 0 {
            out.push(Alert {
                severity: if g >= 4 { Severity::Severe } else { Severity::Warn },
                title: format!("Tempesta geomagnetica G{g} (Kp {kp:.1})"),
                detail: "Il vento solare sta comprimendo la magnetosfera: le fasce si spostano verso l'interno.".into(),
            });
        }
    }

    if let Some(dst) = a.dst {
        if dst <= -50.0 {
            out.push(Alert {
                severity: if dst <= -150.0 { Severity::Severe } else { Severity::Warn },
                title: format!("Corrente ad anello intensa: Dst {dst:.0} nT"),
                detail: "Campo geomagnetico depresso all'equatore: iniezione di particelle nelle fasce esterne.".into(),
            });
        }
    }

    if let Some(j) = a.geo_flux {
        if j >= 1000.0 {
            let sev = if j >= 10_000.0 { Severity::Severe } else { Severity::Warn };
            out.push(Alert {
                severity: sev,
                title: format!("Elettroni relativistici elevati a L=6.6: {j:.0} pfu"),
                detail: "Flusso >2 MeV sopra la soglia di allerta (1000 pfu): rischio di carica elettrostatica sui satelliti in GEO.".into(),
            });
        }
    }

    if let Some(r) = a.r_mp {
        if r <= physics::GEO_L + 0.1 {
            out.push(Alert {
                severity: Severity::Severe,
                title: format!("Magnetopausa entro l'orbita geostazionaria ({r:.1} R⊕)"),
                detail: "I satelliti in GEO sono esposti direttamente al vento solare: elettroni della fascia esterna persi (dropout).".into(),
            });
        } else if r <= 8.0 {
            out.push(Alert {
                severity: Severity::Watch,
                title: format!("Magnetopausa compressa a {r:.1} R⊕"),
                detail: "Il bordo esterno della fascia esterna può essere troncato.".into(),
            });
        }
    }

    if let Some(b) = &a.belt {
        match b.regime {
            physics::BeltRegime::Dropout => out.push(Alert {
                severity: Severity::Severe,
                title: "Dropout delle fasce".into(),
                detail: "Magnetopausa dentro L=6.6: gli elettroni relativistici vengono persi verso la magnetosheath.".into(),
            }),
            physics::BeltRegime::SlotEroded => out.push(Alert {
                severity: Severity::Warn,
                title: "Slot region eroso".into(),
                detail: "La regione di vuoto fra fascia interna ed esterna si è riempita: fasce fuse.".into(),
            }),
            _ => {}
        }
    }

    if let Some(r) = a.rate_1h {
        if r >= 0.5 {
            out.push(Alert {
                severity: Severity::Watch,
                title: format!("Crescita rapida del flusso (+{r:.1} decadi/ora)"),
                detail: "Iniezione in corso: il flusso >2 MeV sta salendo di oltre 3× all'ora.".into(),
            });
        } else if r <= -0.5 {
            out.push(Alert {
                severity: Severity::Watch,
                title: format!("Crollo del flusso ({r:.1} decadi/ora)"),
                detail: "Perdita di elettroni in corso: tipica del recupero dopo una tempesta o di un dropout.".into(),
            });
        }
    }

    if let Some(p) = a.protons_10mev {
        if p >= 10.0 {
            out.push(Alert {
                severity: if p >= 100.0 { Severity::Severe } else { Severity::Warn },
                title: format!("Protoni solari >10 MeV: {p:.1} pfu"),
                detail: "Evento di particelle solari: possibile contributo alla fascia interna di protoni.".into(),
            });
        }
    }

    if let Some(sc) = &s.scales {
        if sc.r >= 1 && out.iter().all(|a| !a.title.contains("Protoni")) {
            out.push(Alert {
                severity: Severity::Info,
                title: format!("Blackout radio R{} ({})", sc.r, sc.r_text),
                detail: "Riguarda la ionosfera, non le fasce: mostrato solo come contesto.".into(),
            });
        }
    }

    out.sort_by_key(|a| match a.severity {
        Severity::Severe => 0,
        Severity::Warn => 1,
        Severity::Watch => 2,
        Severity::Info => 3,
    });
    out
}

const ENDPOINTS: [(&str, &str); 9] = [
    ("goes_e_diff", sources::URL_GOES_E_DIFF),
    ("goes_e_int", sources::URL_GOES_E_INT_1D),
    ("goes_p_int", sources::URL_GOES_P_INT_6H),
    ("goes_mag", sources::URL_GOES_MAG_6H),
    ("wind", sources::URL_WIND),
    ("imf", sources::URL_MAG),
    ("kp", sources::URL_KP),
    ("dst", sources::URL_DST_KYOTO),
    ("dst_min", sources::URL_DST_MIN),
];

fn parse_json(body: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(body).map_err(|e| format!("json: {e}"))
}

fn panic_message(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        format!("panico: {s}")
    } else if let Some(s) = e.downcast_ref::<String>() {
        format!("panico: {s}")
    } else {
        "panico".to_string()
    }
}

/// Scarica tutto in parallelo e costruisce la fotografia.
pub fn refresh(http: &Http, timeout_s: u64) -> Snapshot {
    let fetched_at = now_unix();
    let http = Arc::new(http.clone());
    let mut bodies: Vec<(&'static str, Result<String, String>)> = Vec::new();

    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (name, url) in ENDPOINTS.iter() {
            let http = Arc::clone(&http);
            let name: &'static str = name;
            let url = *url;
            handles.push((
                name,
                scope.spawn(move || {
                    // un re-try breve: SWPC a volte risponde 503 durante i deploy
                    let mut last = http.text(url);
                    if last.is_err() {
                        std::thread::sleep(std::time::Duration::from_millis(400));
                        last = http.text(url);
                    }
                    last
                }),
            ));
        }
        // le scale NOAA hanno una forma diversa e stanno su un altro path
        let http_scales = Arc::clone(&http);
        let scales = scope.spawn(move || http_scales.text(sources::URL_SCALES));
        for (name, h) in handles {
            let r = h.join().unwrap_or_else(|e| Err(panic_message(e)));
            bodies.push((name, r));
        }
        bodies.push(("scales", scales.join().unwrap_or_else(|e| Err(panic_message(e)))));
    });

    let _ = timeout_s;
    let get = |name: &str| -> Option<serde_json::Value> {
        bodies
            .iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, r)| r.as_ref().ok())
            .and_then(|s| parse_json(s).ok())
    };

    let mut warnings = Vec::new();
    for (n, r) in bodies.iter() {
        if let Err(e) = r {
            warnings.push(format!("{n}: {e}"));
        }
    }

    let goes_e = match (get("goes_e_int"), get("goes_e_diff")) {
        (Some(a), Some(b)) => parse_goes_electrons(&a, &b),
        (Some(a), None) => parse_goes_electrons(&a, &serde_json::json!([])),
        (None, Some(b)) => parse_goes_electrons(&serde_json::json!([]), &b),
        _ => GoesElectrons::default(),
    };

    Snapshot {
        fetched_at,
        goes_e,
        goes_p: get("goes_p_int").map(|v| parse_goes_protons(&v)).unwrap_or_default(),
        goes_mag: get("goes_mag").map(|v| parse_goes_mag(&v)).unwrap_or_default(),
        wind: get("wind").map(|v| parse_wind(&v)).unwrap_or_default(),
        imf: get("imf").map(|v| parse_mag(&v)).unwrap_or_default(),
        kp: get("kp").map(|v| parse_kp(&v)).unwrap_or_default(),
        dst: DstSeries {
            series: get("dst").map(|v| parse_dst_kyoto(&v)).unwrap_or_default(),
            official: get("dst")
                .map(|v| parse_dst_kyoto(&v))
                .and_then(|s| s.last().copied()),
            minute: get("dst_min").map(|v| parse_dst_minute(&v)).unwrap_or_default(),
        },
        scales: get("scales").and_then(|v| parse_scales(&v)),
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_is_safe() {
        let s = Snapshot::default();
        let a = s.assess();
        assert!(a.geo_flux.is_none());
        assert!(a.belt.is_none());
        assert_eq!(a.storm, StormLevel::Quiet);
        assert!(a.alerts.is_empty());
    }

    #[test]
    fn alerts_for_storm() {
        let mut s = Snapshot::default();
        s.goes_e.integral_2mev = vec![
            Sample { t: 0, v: 2000.0 },
            Sample { t: 7200, v: 20_000.0 },
        ];
        s.kp = KpNow {
            kp: 7.0,
            estimated: 7.0,
            at: 7200,
            series: vec![Sample { t: 7200, v: 7.0 }],
        };
        s.dst = DstSeries {
            series: vec![Sample { t: 7200, v: -120.0 }],
            official: Some(Sample { t: 7200, v: -120.0 }),
            minute: vec![Sample { t: 7200, v: -120.0 }],
        };
        s.wind.speed = vec![Sample { t: 7200, v: 700.0 }];
        s.wind.density = vec![Sample { t: 7200, v: 12.0 }];
        s.imf.bz = vec![Sample { t: 7200, v: -18.0 }];
        let a = s.assess();
        assert!(a.geo_flux.is_some());
        assert!(a.alerts.len() >= 3, "alerts={:?}", a.alerts);
        assert!(a.r_mp.unwrap() < 8.0);
    }
}