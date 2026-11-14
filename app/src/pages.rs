//! Schede oltre a "Fasce": Vento solare, Magnetosfera, Storico, Sorgenti.
//!
//! Ogni riquadro passa da `card`/`kpi`, quindi ha il suo "?" con la spiegazione.

use crate::help::Topic;
use crate::plots::{self, Line, Plot};
use crate::theme;
use crate::{card, kpi, num, VanAllenApp, REFRESH_SECS};
use eframe::egui::{pos2, Align2, Color32, RichText, Ui};
use vanallen_core::physics::GEO_L;
use vanallen_core::sources::Sample;
use vanallen_core::time::{fmt_short, fmt_stamp, parse_iso_utc};
use vanallen_core::{Assessment, History, HistoryPoint, Snapshot};

/// Finestra temporale della scheda Storico.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HistRange {
    Day,
    Week,
    Month,
    All,
}

impl HistRange {
    pub const ALL: [HistRange; 4] = [Self::Day, Self::Week, Self::Month, Self::All];

    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "24 ore",
            Self::Week => "7 giorni",
            Self::Month => "30 giorni",
            Self::All => "Tutto",
        }
    }

    fn seconds(self) -> Option<i64> {
        match self {
            Self::Day => Some(86_400),
            Self::Week => Some(7 * 86_400),
            Self::Month => Some(30 * 86_400),
            Self::All => None,
        }
    }
}

const GAP: f32 = 12.0;
const CARD_H: f32 = 252.0;
const PLOT_H: f32 = 190.0;

fn dash() -> String {
    "—".to_string()
}

fn opt(v: Option<f64>, f: impl Fn(f64) -> String) -> String {
    v.map(f).unwrap_or_else(dash)
}

/// "min 380 · max 520 · media 431" sui campioni nella finestra.
fn stats(pts: &[Sample], since: i64) -> String {
    let vals: Vec<f64> = pts
        .iter()
        .filter(|s| s.t >= since && s.v.is_finite())
        .map(|s| s.v)
        .collect();
    if vals.is_empty() {
        return "serie non disponibile".to_string();
    }
    let lo = vals.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    format!(
        "min {} · max {} · media {}",
        plots::fmt_val(lo),
        plots::fmt_val(hi),
        plots::fmt_val(mean)
    )
}

/// Al più `max` punti, presi a passo costante (l'ultimo è sempre incluso).
fn thin(pts: Vec<Sample>, max: usize) -> Vec<Sample> {
    if pts.len() <= max {
        return pts;
    }
    let step = pts.len().div_ceil(max);
    let last = pts[pts.len() - 1];
    let mut out: Vec<Sample> = pts.into_iter().step_by(step).collect();
    if out.last().map(|s| s.t) != Some(last.t) {
        out.push(last);
    }
    out
}

fn from_history(points: &[HistoryPoint], f: impl Fn(&HistoryPoint) -> Option<f64>) -> Vec<Sample> {
    thin(
        points
            .iter()
            .filter_map(|p| f(p).filter(|v| v.is_finite()).map(|v| Sample { t: p.t, v }))
            .collect(),
        1500,
    )
}

fn scale_color(n: u8) -> Color32 {
    match n {
        0 => theme::GOOD,
        1 | 2 => theme::WARN,
        3 => theme::MAG,
        _ => theme::BAD,
    }
}

fn scale_word(n: u8) -> &'static str {
    match n {
        0 => "nessun evento",
        1 => "minore",
        2 => "moderato",
        3 => "forte",
        4 => "severo",
        _ => "estremo",
    }
}

/// "3 min", "2 h 05".
fn age_text(secs: i64) -> String {
    let s = secs.max(0);
    if s < 60 {
        "< 1 min".to_string()
    } else if s < 3600 {
        format!("{} min", s / 60)
    } else if s < 48 * 3600 {
        format!("{} h {:02}", s / 3600, (s % 3600) / 60)
    } else {
        format!("{} giorni", s / 86_400)
    }
}

impl VanAllenApp {
    // ---- vento solare ----------------------------------------------------

    pub(crate) fn wind_page(&mut self, ui: &mut Ui, snap: &Snapshot, a: &Assessment) {
        let full = ui.available_width();
        let f = self.fonts.clone();
        let mut asked: Option<Topic> = None;
        let since = snap.fetched_at - 86_400;
        let span = Some((since, snap.fetched_at));

        let gap = 10.0;
        let kw = ((full - gap * 5.0) / 6.0).max(120.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            let mut hit = |t: Option<Topic>| {
                if t.is_some() {
                    asked = t;
                }
            };
            let v = a.wind_speed;
            hit(kpi(
                ui,
                kw,
                "Velocità",
                &opt(v, |x| format!("{x:.0} km/s")),
                match v {
                    Some(x) if x >= 700.0 => "vento molto veloce",
                    Some(x) if x >= 500.0 => "vento veloce",
                    Some(_) => "vento lento",
                    None => "nessun dato",
                },
                match v {
                    Some(x) if x >= 700.0 => theme::BAD,
                    Some(x) if x >= 500.0 => theme::WARN,
                    _ => theme::ACCENT,
                },
                Topic::Speed,
                &f,
            ));
            hit(kpi(
                ui,
                kw,
                "Densità",
                &opt(a.wind_density, |x| format!("{x:.1} cm⁻³")),
                match a.wind_density {
                    Some(x) if x >= 20.0 => "plasma compresso",
                    Some(x) if x < 1.0 => "plasma rarefatto",
                    Some(_) => "nella norma",
                    None => "nessun dato",
                },
                if a.wind_density.unwrap_or(0.0) >= 20.0 {
                    theme::WARN
                } else {
                    theme::TEXT
                },
                Topic::Density,
                &f,
            ));
            hit(kpi(
                ui,
                kw,
                "Temperatura",
                &opt(a.wind_temp, |x| format!("{} K", plots::fmt_val(x))),
                "protoni, punto L1",
                theme::TEXT,
                Topic::Temperature,
                &f,
            ));
            hit(kpi(
                ui,
                kw,
                "Bt",
                &opt(a.bt, |x| format!("{x:.1} nT")),
                match a.bt {
                    Some(x) if x >= 20.0 => "campo molto forte",
                    Some(x) if x >= 10.0 => "campo rinforzato",
                    Some(_) => "nella norma",
                    None => "nessun dato",
                },
                if a.bt.unwrap_or(0.0) >= 10.0 {
                    theme::WARN
                } else {
                    theme::TEXT
                },
                Topic::Bt,
                &f,
            ));
            hit(kpi(
                ui,
                kw,
                "Bz (GSM)",
                &opt(a.bz, |x| format!("{x:+.1} nT")),
                match a.bz {
                    Some(x) if x <= -10.0 => "sud, forte",
                    Some(x) if x < 0.0 => "verso sud",
                    Some(_) => "verso nord",
                    None => "nessun dato",
                },
                match a.bz {
                    Some(x) if x <= -10.0 => theme::BAD,
                    Some(x) if x < -5.0 => theme::MAG,
                    _ => theme::TEXT,
                },
                Topic::Bz,
                &f,
            ));
            hit(kpi(
                ui,
                kw,
                "Pressione dinamica",
                &opt(a.pdyn, |x| format!("{x:.2} nPa")),
                &format!("magnetopausa {}", opt(a.r_mp, |r| format!("{r:.1} R⊕"))),
                match a.pdyn {
                    Some(x) if x >= 10.0 => theme::BAD,
                    Some(x) if x >= 5.0 => theme::WARN,
                    _ => theme::TEXT,
                },
                Topic::Pressure,
                &f,
            ));
        });

        ui.add_space(GAP);
        let half = ((full - GAP) / 2.0).floor();
        let probe = if snap.wind.source.is_empty() {
            "L1".to_string()
        } else {
            format!("L1, sonda {}", snap.wind.source)
        };

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Velocità — ultime 24 ore",
                &format!("{probe} · {} km/s", stats(&snap.wind.speed, since)),
                Topic::Speed,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[Line {
                            pts: &snap.wind.speed,
                            color: theme::ACCENT,
                            label: "velocità",
                            fill: true,
                        }],
                        &Plot {
                            height: PLOT_H,
                            unit: "km/s",
                            include: &[300.0, 500.0],
                            marks: &[(500.0, theme::WARN, "vento veloce")],
                            span,
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Densità — ultime 24 ore",
                &format!("{probe} · {} cm⁻³", stats(&snap.wind.density, since)),
                Topic::Density,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[Line {
                            pts: &snap.wind.density,
                            color: theme::FLUX,
                            label: "densità",
                            fill: true,
                        }],
                        &Plot {
                            height: PLOT_H,
                            unit: "cm⁻³",
                            include: &[0.0, 10.0],
                            marks: &[(20.0, theme::WARN, "plasma compresso")],
                            span,
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
        });

        ui.add_space(GAP);
        let pressure = snap.wind.pressure();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let imf_probe = if snap.imf.source.is_empty() {
                "L1".to_string()
            } else {
                format!("L1, sonda {}", snap.imf.source)
            };
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Campo interplanetario — ultime 24 ore",
                &format!("{imf_probe} · Bz: {} nT", stats(&snap.imf.bz, since)),
                Topic::ImfSeries,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[
                            Line {
                                pts: &snap.imf.bt,
                                color: theme::ACCENT,
                                label: "Bt",
                                fill: false,
                            },
                            Line {
                                pts: &snap.imf.bz,
                                color: theme::MAG,
                                label: "Bz",
                                fill: true,
                            },
                        ],
                        &Plot {
                            height: PLOT_H,
                            unit: "nT",
                            include: &[-5.0, 5.0],
                            marks: &[(-10.0, theme::BAD, "Bz −10 nT")],
                            span,
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Pressione dinamica — ultime 24 ore",
                &format!("calcolata da n e V · {} nPa", stats(&pressure, since)),
                Topic::Pressure,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[Line {
                            pts: &pressure,
                            color: theme::WARN,
                            label: "P dinamica",
                            fill: true,
                        }],
                        &Plot {
                            height: PLOT_H,
                            unit: "nPa",
                            include: &[0.0, 3.0],
                            marks: &[
                                (5.0, theme::WARN, "compressione"),
                                (10.0, theme::BAD, "compressione forte"),
                            ],
                            span,
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
        });

        ui.add_space(GAP);
        if let Some(t) = card(
            ui,
            full,
            212.0,
            "Temperatura dei protoni — ultime 24 ore",
            &format!("{probe} · {} K", stats(&snap.wind.temperature, since)),
            Topic::Temperature,
            &f,
            |ui, w| {
                plots::timeseries(
                    ui,
                    w,
                    &[Line {
                        pts: &snap.wind.temperature,
                        color: theme::PROTON,
                        label: "temperatura",
                        fill: true,
                    }],
                    &Plot {
                        height: 150.0,
                        unit: "K",
                        log: true,
                        include: &[1.0e4, 1.0e6],
                        span,
                        ..Default::default()
                    },
                )
            },
        ) {
            asked = Some(t);
        }

        if asked.is_some() {
            self.help = asked;
        }
    }

    // ---- magnetosfera ----------------------------------------------------

    pub(crate) fn magneto_page(&mut self, ui: &mut Ui, snap: &Snapshot, a: &Assessment) {
        let full = ui.available_width();
        let f = self.fonts.clone();
        let mut asked: Option<Topic> = None;

        let gap = 10.0;
        let kw = ((full - gap * 5.0) / 6.0).max(120.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            let mut hit = |t: Option<Topic>| {
                if t.is_some() {
                    asked = t;
                }
            };
            let scales = snap.scales.as_ref().map(|s| (s.r, s.s, s.g));
            for (title, value, topic) in [
                ("Blackout radio", scales.map(|s| ('R', s.0)), Topic::ScaleR),
                ("Radiazione solare", scales.map(|s| ('S', s.1)), Topic::ScaleS),
                ("Tempesta geomagnetica", scales.map(|s| ('G', s.2)), Topic::ScaleG),
            ] {
                hit(kpi(
                    ui,
                    kw,
                    title,
                    &value
                        .map(|(c, n)| format!("{c}{n}"))
                        .unwrap_or_else(dash),
                    value.map(|(_, n)| scale_word(n)).unwrap_or("scala NOAA non letta"),
                    value.map(|(_, n)| scale_color(n)).unwrap_or(theme::FAINT),
                    topic,
                    &f,
                ));
            }
            hit(kpi(
                ui,
                kw,
                "Campo a GEO (Hp)",
                &opt(a.geo_hp_nt, |x| format!("{x:.0} nT")),
                &match a.geo_hp_nt {
                    Some(x) if x <= 0.0 => "GOES fuori dalla magnetosfera".to_string(),
                    _ if snap.goes_mag.satellite > 0 => {
                        format!("magnetometro GOES-{}", snap.goes_mag.satellite)
                    }
                    _ => "magnetometro GOES".to_string(),
                },
                if a.geo_hp_nt.unwrap_or(100.0) <= 0.0 {
                    theme::BAD
                } else {
                    theme::TEXT
                },
                Topic::Hp,
                &f,
            ));
            hit(kpi(
                ui,
                kw,
                "Protoni >10 MeV",
                &opt(a.protons_10mev, |x| format!("{} pfu", plots::fmt_val(x))),
                "soglia S1: 10 pfu",
                match a.protons_10mev {
                    Some(x) if x >= 100.0 => theme::BAD,
                    Some(x) if x >= 10.0 => theme::WARN,
                    Some(_) => theme::GOOD,
                    None => theme::FAINT,
                },
                Topic::Protons,
                &f,
            ));
            let l_pp = a
                .belt
                .as_ref()
                .map(|b| b.l_pp)
                .or_else(|| a.kp.map(vanallen_core::physics::plasmapause_l));
            hit(kpi(
                ui,
                kw,
                "Plasmapausa",
                &opt(l_pp, |x| format!("{x:.1} R⊕")),
                "modello, da Kp",
                theme::ACCENT,
                Topic::Plasmapause,
                &f,
            ));
        });

        ui.add_space(GAP);
        let half = ((full - GAP) / 2.0).floor();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let kp_max = snap
                .kp
                .series
                .iter()
                .map(|s| s.v)
                .fold(f64::NEG_INFINITY, f64::max);
            let kp_sub = if snap.kp.series.is_empty() {
                "serie non disponibile".to_string()
            } else {
                format!(
                    "adesso {} · massimo {kp_max:.2} nel periodo · un valore al minuto",
                    opt(a.kp, |k| format!("{k:.2}"))
                )
            };
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Kp stimato — ultime 6 ore",
                &kp_sub,
                Topic::KpSeries,
                &f,
                |ui, w| plots::kp_bars(ui, w, PLOT_H, &snap.kp.series),
            ) {
                asked = Some(t);
            }

            let dst_sub = format!(
                "Kyoto {} · nowcast del modello {}",
                opt(snap.dst.series.last().map(|s| s.v), |d| format!("{d:.0} nT")),
                opt(a.dst_nowcast, |d| format!("{d:.0} nT")),
            );
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Dst — ultimi 7 giorni",
                &dst_sub,
                Topic::DstSeries,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[Line {
                            pts: &snap.dst.series,
                            color: theme::MAG,
                            label: "Dst Kyoto",
                            fill: true,
                        }],
                        &Plot {
                            height: PLOT_H,
                            unit: "nT",
                            include: &[-30.0, 10.0],
                            marks: &[
                                (-50.0, theme::WARN, "tempesta moderata"),
                                (-100.0, theme::BAD, "tempesta forte"),
                            ],
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
        });

        ui.add_space(GAP);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let since = snap.fetched_at - 6 * 3600;
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Campo magnetico a GEO (Hp) — ultime 6 ore",
                &format!("{} nT", stats(&snap.goes_mag.hp, since)),
                Topic::Hp,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[Line {
                            pts: &snap.goes_mag.hp,
                            color: theme::GOOD,
                            label: "Hp",
                            fill: true,
                        }],
                        &Plot {
                            height: PLOT_H,
                            unit: "nT",
                            include: &[0.0, 120.0],
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
            let p_sub = if snap.goes_p.p10.is_empty() {
                "serie non disponibile".to_string()
            } else {
                format!(
                    "GOES-{} · >10 MeV: {} pfu",
                    snap.goes_p.satellite,
                    stats(&snap.goes_p.p10, since)
                )
            };
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Protoni solari — ultime 6 ore",
                &p_sub,
                Topic::Protons,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[
                            Line {
                                pts: &snap.goes_p.p1,
                                color: theme::FLUX,
                                label: ">1 MeV",
                                fill: false,
                            },
                            Line {
                                pts: &snap.goes_p.p10,
                                color: theme::WARN,
                                label: ">10 MeV",
                                fill: false,
                            },
                            Line {
                                pts: &snap.goes_p.p100,
                                color: theme::BAD,
                                label: ">100 MeV",
                                fill: false,
                            },
                        ],
                        &Plot {
                            height: PLOT_H,
                            unit: "pfu",
                            log: true,
                            include: &[0.1, 100.0],
                            marks: &[(10.0, theme::WARN, "soglia S1")],
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
        });

        if asked.is_some() {
            self.help = asked;
        }
    }

    // ---- storico ---------------------------------------------------------

    pub(crate) fn history_page(&mut self, ui: &mut Ui, snap: &Snapshot) {
        let full = ui.available_width();
        let f = self.fonts.clone();
        let mut asked: Option<Topic> = None;

        ui.horizontal(|ui| {
            ui.label(f.body("Periodo:"));
            for r in HistRange::ALL {
                ui.selectable_value(&mut self.hist_range, r, r.label());
            }
        });
        ui.add_space(4.0);

        let now = snap.fetched_at;
        let points: &[HistoryPoint] = match self.hist_range.seconds() {
            Some(s) => self.history.window(now - s),
            None => &self.history.points,
        };
        let span = match (self.hist_range.seconds(), points.first()) {
            (Some(s), _) => Some((now - s, now)),
            (None, Some(first)) => Some((first.t, now)),
            (None, None) => None,
        };
        let summary = match (points.first(), points.last()) {
            (Some(a), Some(b)) => format!(
                "{} punti dal {} al {} UTC · un punto a ogni aggiornamento (5 min) mentre l'app è aperta",
                points.len(),
                fmt_short(a.t),
                fmt_short(b.t)
            ),
            _ => "nessun punto in questo periodo".to_string(),
        };
        ui.label(f.hint(summary));
        ui.add_space(8.0);

        if points.len() < 2 {
            let _ = card(
                ui,
                full,
                120.0,
                "Lo storico è ancora vuoto",
                "si riempie da solo col tempo",
                Topic::HistFlux,
                &f,
                |ui, _| {
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(f.body(
                            "L'app salva un punto a ogni aggiornamento. Lasciala aperta: \
                             dopo qualche ora qui compaiono le curve; scegli «Tutto» per \
                             vedere ciò che è già stato registrato.",
                        ));
                    });
                },
            )
            .map(|t| asked = Some(t));
            if asked.is_some() {
                self.help = asked;
            }
            return;
        }

        let flux = from_history(points, |p| p.flux2);
        let kp = from_history(points, |p| p.kp);
        let dst = from_history(points, |p| p.dst);
        let mp = from_history(points, |p| p.r_mp);
        let peak = from_history(points, |p| p.l_peak);
        let outer = from_history(points, |p| p.l_outer);
        let content = from_history(points, |p| p.content);

        let half = ((full - GAP) / 2.0).floor();
        let last = |v: &[Sample], f: &dyn Fn(f64) -> String| -> String {
            opt(v.last().map(|s| s.v), f)
        };

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Elettroni >2 MeV a L = 6.6",
                &format!("ultimo {} pfu · {}", last(&flux, &num), stats(&flux, 0)),
                Topic::HistFlux,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[Line {
                            pts: &flux,
                            color: theme::FLUX,
                            label: "flusso",
                            fill: true,
                        }],
                        &Plot {
                            height: PLOT_H,
                            unit: "pfu",
                            log: true,
                            include: &[10.0, 10_000.0],
                            marks: &[(1000.0, theme::WARN, "soglia 1000 pfu")],
                            span,
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Kp stimato",
                &format!("ultimo {} · {}", last(&kp, &|k| format!("{k:.1}")), stats(&kp, 0)),
                Topic::HistKp,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[Line {
                            pts: &kp,
                            color: theme::GOOD,
                            label: "Kp",
                            fill: true,
                        }],
                        &Plot {
                            height: PLOT_H,
                            unit: "Kp",
                            include: &[0.0, 5.0],
                            marks: &[(5.0, theme::MAG, "tempesta G1")],
                            span,
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
        });

        ui.add_space(GAP);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Dst",
                &format!("ultimo {} nT · {}", last(&dst, &|d| format!("{d:.0}")), stats(&dst, 0)),
                Topic::HistDst,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[Line {
                            pts: &dst,
                            color: theme::MAG,
                            label: "Dst",
                            fill: true,
                        }],
                        &Plot {
                            height: PLOT_H,
                            unit: "nT",
                            include: &[-30.0, 10.0],
                            marks: &[
                                (-50.0, theme::WARN, "tempesta moderata"),
                                (-100.0, theme::BAD, "tempesta forte"),
                            ],
                            span,
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Magnetopausa (modello)",
                &format!("ultima {} R⊕ · {}", last(&mp, &|r| format!("{r:.1}")), stats(&mp, 0)),
                Topic::HistMagnetopause,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[Line {
                            pts: &mp,
                            color: theme::BAD,
                            label: "distanza",
                            fill: false,
                        }],
                        &Plot {
                            height: PLOT_H,
                            unit: "R⊕",
                            include: &[6.0, 12.0],
                            marks: &[(GEO_L, theme::WARN, "orbita geostazionaria")],
                            span,
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
        });

        ui.add_space(GAP);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Posizione della fascia esterna (modello)",
                &format!(
                    "picco {} R⊕ · bordo esterno {} R⊕",
                    last(&peak, &|x| format!("{x:.1}")),
                    last(&outer, &|x| format!("{x:.1}"))
                ),
                Topic::HistBelt,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[
                            Line {
                                pts: &outer,
                                color: theme::WARN,
                                label: "bordo esterno",
                                fill: false,
                            },
                            Line {
                                pts: &peak,
                                color: theme::GOOD,
                                label: "picco",
                                fill: false,
                            },
                        ],
                        &Plot {
                            height: PLOT_H,
                            unit: "R⊕",
                            include: &[3.0, 8.0],
                            span,
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
            if let Some(t) = card(
                ui,
                half,
                CARD_H,
                "Contenuto della fascia (modello)",
                &format!("ultimo {} (u.a.) · {}", last(&content, &num), stats(&content, 0)),
                Topic::HistContent,
                &f,
                |ui, w| {
                    plots::timeseries(
                        ui,
                        w,
                        &[Line {
                            pts: &content,
                            color: theme::PROTON,
                            label: "contenuto",
                            fill: true,
                        }],
                        &Plot {
                            height: PLOT_H,
                            unit: "u.a.",
                            log: true,
                            span,
                            ..Default::default()
                        },
                    )
                },
            ) {
                asked = Some(t);
            }
        });

        if asked.is_some() {
            self.help = asked;
        }
    }

    // ---- sorgenti --------------------------------------------------------

    pub(crate) fn sources_page(&mut self, ui: &mut Ui, snap: &Snapshot) {
        let full = ui.available_width();
        let f = self.fonts.clone();
        let mut asked: Option<Topic> = None;

        struct Row {
            name: &'static str,
            what: String,
            n: usize,
            last: Option<i64>,
            key: &'static str,
            /// età oltre la quale il dato è considerato in ritardo, secondi
            max_age: i64,
        }
        let last = |v: &[Sample]| v.last().map(|s| s.t);
        let sat = |n: u8| {
            if n > 0 {
                format!("GOES-{n}")
            } else {
                "GOES".to_string()
            }
        };
        let rows = [
            Row {
                name: "Elettroni >2 MeV",
                what: format!("{}, flusso integrale, 24 ore a passi di 5 min", sat(snap.goes_e.satellite)),
                n: snap.goes_e.integral_2mev.len(),
                last: last(&snap.goes_e.integral_2mev),
                key: "goes_e_int",
                max_age: 30 * 60,
            },
            Row {
                name: "Spettro differenziale",
                what: format!("{}, canali da 79 keV a 2,9 MeV", sat(snap.goes_e.satellite)),
                n: snap.goes_e.spectrum.len(),
                last: Some(snap.goes_e.spectrum_at).filter(|t| *t > 0),
                key: "goes_e_diff",
                max_age: 30 * 60,
            },
            Row {
                name: "Protoni",
                what: format!("{}, flusso integrale >1, >10, >100 MeV", sat(snap.goes_p.satellite)),
                n: snap.goes_p.p10.len(),
                last: last(&snap.goes_p.p10),
                key: "goes_p_int",
                max_age: 30 * 60,
            },
            Row {
                name: "Magnetometro",
                what: format!("{}, componente Hp al minuto", sat(snap.goes_mag.satellite)),
                n: snap.goes_mag.hp.len(),
                last: last(&snap.goes_mag.hp),
                key: "goes_mag",
                max_age: 20 * 60,
            },
            Row {
                name: "Vento solare",
                what: format!(
                    "punto L1{}: velocità, densità, temperatura",
                    if snap.wind.source.is_empty() {
                        String::new()
                    } else {
                        format!(", sonda {}", snap.wind.source)
                    }
                ),
                n: snap.wind.speed.len(),
                last: last(&snap.wind.speed),
                key: "wind",
                max_age: 20 * 60,
            },
            Row {
                name: "Campo interplanetario",
                what: format!(
                    "punto L1{}: Bt e Bz",
                    if snap.imf.source.is_empty() {
                        String::new()
                    } else {
                        format!(", sonda {}", snap.imf.source)
                    }
                ),
                n: snap.imf.bz.len(),
                last: last(&snap.imf.bz),
                key: "imf",
                max_age: 20 * 60,
            },
            Row {
                name: "Kp stimato",
                what: "NOAA SWPC, indice planetario al minuto".to_string(),
                n: snap.kp.series.len(),
                last: last(&snap.kp.series),
                key: "kp",
                max_age: 20 * 60,
            },
            Row {
                name: "Dst di Kyoto",
                what: "WDC Kyoto via SWPC, valore orario, 7 giorni".to_string(),
                n: snap.dst.series.len(),
                last: last(&snap.dst.series),
                key: "dst",
                max_age: 3 * 3600,
            },
            Row {
                name: "Dst nowcast",
                what: "modello Geospace di NOAA, al minuto".to_string(),
                n: snap.dst.minute.len(),
                last: last(&snap.dst.minute),
                key: "dst_min",
                max_age: 30 * 60,
            },
            Row {
                name: "Scale NOAA",
                what: "livelli R, S, G in corso".to_string(),
                n: usize::from(snap.scales.is_some()),
                last: snap.scales.as_ref().and_then(|s| parse_iso_utc(&s.at)),
                key: "scales",
                max_age: 30 * 60,
            },
        ];

        let now = snap.fetched_at;
        let mut bad = 0;
        let mut late = 0;
        let states: Vec<(Color32, &str, Option<String>)> = rows
            .iter()
            .map(|r| {
                let err = snap
                    .warnings
                    .iter()
                    .find(|w| w.starts_with(&format!("{}:", r.key)))
                    .cloned();
                if err.is_some() {
                    bad += 1;
                    (theme::BAD, "errore", err)
                } else if r.n == 0 {
                    bad += 1;
                    (theme::BAD, "vuota", None)
                } else if r.last.map(|t| now - t > r.max_age).unwrap_or(false) {
                    late += 1;
                    (theme::WARN, "in ritardo", None)
                } else {
                    (theme::GOOD, "ok", None)
                }
            })
            .collect();

        let sub = if bad == 0 && late == 0 {
            format!("tutte le {} fonti rispondono · scaricate alle {}", rows.len(), fmt_stamp(now))
        } else {
            format!(
                "{} con errore · {} in ritardo · {} regolari · scaricate alle {}",
                bad,
                late,
                rows.len() - bad - late,
                fmt_stamp(now)
            )
        };

        let row_h = 27.0;
        if let Some(t) = card(
            ui,
            full,
            84.0 + row_h * rows.len() as f32,
            "Stato delle sorgenti",
            &sub,
            Topic::Sources,
            &f,
            |ui, w| {
                let (rect, _) = ui.allocate_exact_size(
                    eframe::egui::vec2(w, 26.0 + row_h * rows.len() as f32),
                    eframe::egui::Sense::hover(),
                );
                let p = ui.painter_at(rect);
                let x_name = rect.left() + 34.0;
                let x_what = rect.left() + 230.0;
                let x_n = rect.right() - 330.0;
                let x_last = rect.right() - 300.0;
                let x_age = rect.right() - 170.0;
                let x_state = rect.right() - 86.0;
                let head = rect.top() + 10.0;
                for (x, label, anchor) in [
                    (x_name, "Fonte", Align2::LEFT_CENTER),
                    (x_what, "Cosa contiene", Align2::LEFT_CENTER),
                    (x_n, "Campioni", Align2::RIGHT_CENTER),
                    (x_last, "Ultimo dato (UTC)", Align2::LEFT_CENTER),
                    (x_age, "Età", Align2::LEFT_CENTER),
                    (x_state, "Stato", Align2::LEFT_CENTER),
                ] {
                    p.text(pos2(x, head), anchor, label, f.ui(10.0), theme::FAINT);
                }
                for (i, (r, (color, state, err))) in rows.iter().zip(states.iter()).enumerate() {
                    let y = rect.top() + 26.0 + row_h * i as f32 + row_h * 0.5;
                    p.line_segment(
                        [
                            pos2(rect.left() + 10.0, y - row_h * 0.5),
                            pos2(rect.right() - 10.0, y - row_h * 0.5),
                        ],
                        theme::stroke(1.0, theme::GRID),
                    );
                    p.circle_filled(pos2(rect.left() + 20.0, y), 4.5, *color);
                    p.text(pos2(x_name, y), Align2::LEFT_CENTER, r.name, f.heading(11.5), theme::TEXT);
                    let (what, what_color) = match err {
                        Some(e) => {
                            let short: String = e.chars().take(90).collect();
                            (short, theme::BAD)
                        }
                        None => (r.what.clone(), theme::SUB),
                    };
                    // la descrizione non deve invadere la colonna dei campioni
                    let clip = eframe::egui::Rect::from_min_max(
                        pos2(x_what, y - row_h * 0.5),
                        pos2(x_n - 70.0, y + row_h * 0.5),
                    );
                    p.with_clip_rect(clip).text(
                        pos2(x_what, y),
                        Align2::LEFT_CENTER,
                        what,
                        f.ui(10.5),
                        what_color,
                    );
                    p.text(
                        pos2(x_n, y),
                        Align2::RIGHT_CENTER,
                        r.n.to_string(),
                        f.ui(11.0),
                        theme::SUB,
                    );
                    p.text(
                        pos2(x_last, y),
                        Align2::LEFT_CENTER,
                        r.last.map(fmt_short).unwrap_or_else(dash),
                        f.ui(11.0),
                        theme::SUB,
                    );
                    p.text(
                        pos2(x_age, y),
                        Align2::LEFT_CENTER,
                        r.last.map(|t| age_text(now - t)).unwrap_or_else(dash),
                        f.ui(11.0),
                        theme::SUB,
                    );
                    p.text(pos2(x_state, y), Align2::LEFT_CENTER, *state, f.heading(11.0), *color);
                }
            },
        ) {
            asked = Some(t);
        }

        ui.add_space(GAP);

        let server = self.server.as_ref().map(|(p, _)| *p);
        // Mostrato con la variabile al posto della cartella utente: si può
        // incollare così com'è in Esplora file e non espone il nome dell'account
        // negli screenshot.
        let hist_path = if std::env::var_os("LOCALAPPDATA").is_some() {
            r"%LOCALAPPDATA%\van-allen-dashboard\history.json".to_string()
        } else {
            History::path().display().to_string()
        };
        let hist_n = self.history.points.len();
        let hist_from = self.history.points.first().map(|p| fmt_short(p.t));
        if let Some(t) = card(
            ui,
            full,
            214.0,
            "Server locale e file",
            "attivi solo mentre l'app è aperta",
            Topic::Server,
            &f,
            |ui, _| {
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 5.0;
                        match server {
                            Some(port) => {
                                let base = format!("http://127.0.0.1:{port}");
                                for (label, path, note) in [
                                    ("Pagina riassuntiva", "/", "tabella che si aggiorna ogni 30 s"),
                                    ("Dati in JSON", "/snapshot.json", "per script e altri programmi"),
                                    ("Controllo di vita", "/health", "risponde «ok»"),
                                ] {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("{label}:"))
                                                .size(11.5)
                                                .color(theme::SUB),
                                        );
                                        ui.hyperlink_to(
                                            RichText::new(format!("{base}{path}")).size(11.5),
                                            format!("{base}{path}"),
                                        );
                                        ui.label(f.hint(format!("— {note}")));
                                    });
                                }
                            }
                            None => {
                                ui.label(
                                    RichText::new(
                                        "Server locale non avviato: le porte da 8787 a 8797 sono occupate.",
                                    )
                                    .size(11.5)
                                    .color(theme::WARN),
                                );
                            }
                        }
                        ui.add_space(2.0);
                        ui.label(f.body(format!("Storico: {hist_path}")));
                        ui.label(f.hint(match hist_from {
                            Some(from) => format!(
                                "{hist_n} punti registrati dal {from} UTC · aggiornamento automatico ogni {} minuti · versione {}",
                                REFRESH_SECS / 60,
                                vanallen_core::VERSION
                            ),
                            None => format!(
                                "ancora nessun punto · aggiornamento automatico ogni {} minuti · versione {}",
                                REFRESH_SECS / 60,
                                vanallen_core::VERSION
                            ),
                        }));
                    });
                });
            },
        ) {
            asked = Some(t);
        }

        if asked.is_some() {
            self.help = asked;
        }
    }
}
