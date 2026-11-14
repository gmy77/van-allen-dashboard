//! Grafici disegnati direttamente sul painter di egui.
//!
//! Nessuna libreria di charting: le forme sono poche (polilinea, area, tacche,
//! etichette) e così il controllo su assi, soglie e marcatori è totale.

use crate::theme as t;
use eframe::egui::epaint::{PathShape, PathStroke};
use eframe::egui::{
    pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui,
};
use vanallen_core::physics::{BeltShape, GEO_L};
use vanallen_core::sources::{Sample, SpectrumPoint};
use vanallen_core::time::fmt_hhmm;

pub(crate) fn txt(p: &Painter, at: Pos2, anchor: Align2, s: impl ToString, size: f32, color: Color32) {
    p.text(at, anchor, s.to_string(), FontId::proportional(size), color);
}

/// Area sotto una curva: riempimento semitrasparente + contorno.
///
/// Il riempimento è fatto di quadrilateri verticali (uno per segmento) invece che
/// di un unico poligono: con profili piatti e poi in salita la tassellatura di un
/// poligono unico produce triangoli che sbordano.
///
/// Il colore sfuma: pieno sotto la curva, quasi trasparente alla base.
pub(crate) fn filled_area(
    _ui: &Ui,
    p: &Painter,
    pts: &[Pos2],
    base_y: f32,
    fill: Color32,
    line: Color32,
) {
    if pts.len() < 2 {
        return;
    }
    let faint = fill.gamma_multiply(0.12);
    let mut mesh = eframe::egui::epaint::Mesh::default();
    for w in pts.windows(2) {
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(w[0], fill);
        mesh.colored_vertex(w[1], fill);
        mesh.colored_vertex(pos2(w[1].x, base_y), faint);
        mesh.colored_vertex(pos2(w[0].x, base_y), faint);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    p.add(Shape::mesh(mesh));
    p.add(Shape::line(pts.to_vec(), t::stroke(2.0, line)));
}

/// Decade in apice: 4 → "10⁴".
pub(crate) fn decade(d: i32) -> String {
    let sup = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    let mut s = String::from("10");
    for ch in d.to_string().chars() {
        if ch == '-' {
            s.push('⁻');
        } else if let Some(v) = ch.to_digit(10) {
            s.push(sup[v as usize]);
        }
    }
    s
}

/// Tratteggio orizzontale (soglie).
pub(crate) fn dashed_h(p: &Painter, y: f32, x0: f32, x1: f32, color: Color32, width: f32) {
    let mut x = x0;
    while x < x1 {
        p.line_segment(
            [pos2(x, y), pos2((x + 5.0).min(x1), y)],
            Stroke::new(width, color),
        );
        x += 10.0;
    }
}

// ---- profilo radiale ----------------------------------------------------

pub fn profile(ui: &mut Ui, width: f32, belt: &BeltShape, r_mp: Option<f64>) {
    let h = 246.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let p = ui.painter_at(rect);

    let left = rect.left() + 46.0;
    let right = rect.right() - 88.0;
    let top = rect.top() + 14.0;
    let bottom = rect.bottom() - 34.0;
    if right - left < 60.0 || bottom - top < 60.0 {
        return;
    }

    let (lmin, lmax) = (1.0f64, 8.0f64);
    let (jmin, jmax) = (-1.0f64, 5.0f64);
    let x_of = |l: f64| left + (((l - lmin) / (lmax - lmin)) as f32) * (right - left);
    let y_of = |lj: f64| bottom - (((lj - jmin) / (jmax - jmin)) as f32) * (bottom - top);

    // griglia e assi
    for dec in 0..=5 {
        let y = y_of(dec as f64);
        p.line_segment(
            [pos2(left, y), pos2(right, y)],
            t::stroke(1.0, t::GRID),
        );
        txt(
            &p,
            pos2(rect.left() + 6.0, y),
            Align2::LEFT_CENTER,
            decade(dec as i32),
            9.5,
            t::FAINT,
        );
    }
    let mut l = 1.0;
    while l <= 8.0 {
        let x = x_of(l);
        p.line_segment(
            [pos2(x, top), pos2(x, bottom)],
            t::stroke(1.0, t::GRID),
        );
        txt(
            &p,
            pos2(x, bottom + 6.0),
            Align2::CENTER_TOP,
            format!("{l:.0}"),
            9.5,
            t::FAINT,
        );
        l += 1.0;
    }
    txt(
        &p,
        pos2(left, bottom + 20.0),
        Align2::LEFT_TOP,
        "L-shell (raggi terrestri) · flusso in pfu",
        9.5,
        t::FAINT,
    );

    // banda della fascia interna di protoni (climatologia, non misura)
    let mut band: Vec<Pos2> = vec![pos2(x_of(1.0), bottom)];
    let mut ll = 1.0;
    while ll <= 2.6 {
        band.push(pos2(
            x_of(ll),
            bottom - (vanallen_core::physics::inner_belt_density(ll) as f32) * (bottom - top) * 0.5,
        ));
        ll += 0.05;
    }
    band.push(pos2(x_of(2.6), bottom));
    p.add(Shape::Path(PathShape {
        points: band,
        closed: true,
        fill: t::PROTON.gamma_multiply(0.22),
        stroke: PathStroke::NONE,
    }));
    txt(
        &p,
        pos2(x_of(1.55), top + 12.0),
        Align2::CENTER_TOP,
        "fascia interna (protoni, climatologia)",
        9.0,
        t::PROTON,
    );

    // area e curva del profilo modellato
    let pts: Vec<Pos2> = belt
        .profile
        .iter()
        .map(|(ll, lj)| pos2(x_of(*ll), y_of(lj.clamp(jmin, jmax))))
        .collect();
    filled_area(ui, &p, &pts, bottom, t::FLUX.gamma_multiply(0.34), t::FLUX);

    // marcatori verticali
    let marker = |at: f64, color: Color32, label: &str, dy: f32| {
        let x = x_of(at.clamp(lmin, lmax));
        p.line_segment(
            [pos2(x, top), pos2(x, bottom)],
            t::stroke(1.0, color.gamma_multiply(0.8)),
        );
        txt(
            &p,
            pos2(x + 3.0, bottom - 4.0 - dy),
            Align2::LEFT_BOTTOM,
            label,
            9.0,
            color,
        );
    };
    marker(belt.l_pp, t::ACCENT, "plasmapausa", 0.0);
    marker(GEO_L, t::WARN, "GEO 6.6", 14.0);
    if let Some(r) = r_mp {
        if r <= lmax + 0.3 {
            marker(r, t::BAD, "magnetopausa", 28.0);
        }
    }

    // picco
    let px = x_of(belt.l_peak);
    let py = y_of(belt.amp.max(1e-3).log10());
    p.circle_filled(pos2(px, py), 3.0, t::TEXT);
    txt(
        &p,
        pos2(px + 7.0, py - 6.0),
        Align2::LEFT_BOTTOM,
        format!("picco {:.1} R⊕", belt.l_peak),
        9.5,
        t::TEXT,
    );

    // legenda della scala colore
    let lx = right + 20.0;
    txt(&p, pos2(lx, top), Align2::LEFT_TOP, "intensità", 9.5, t::SUB);
    let steps = 26;
    for i in 0..steps {
        let frac = i as f64 / (steps - 1) as f64;
        let col = t::flux_color(3.5 - 3.5 * frac);
        let y = top + 18.0 + i as f32 * 6.0;
        p.rect_filled(
            Rect::from_min_size(pos2(lx, y), vec2(12.0, 6.0)),
            0,
            col,
        );
    }
    txt(
        &p,
        pos2(lx + 16.0, top + 18.0),
        Align2::LEFT_TOP,
        "10³",
        8.5,
        t::FAINT,
    );
    txt(
        &p,
        pos2(lx + 16.0, top + 18.0 + steps as f32 * 6.0 - 8.0),
        Align2::LEFT_TOP,
        "10⁰",
        8.5,
        t::FAINT,
    );
}

// ---- striscia di intensità lungo l'equatore -----------------------------

pub fn strip(ui: &mut Ui, width: f32, belt: &BeltShape, r_mp: Option<f64>) {
    let h = 92.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let p = ui.painter_at(rect);

    let left = rect.left() + 40.0;
    let right = rect.right() - 8.0;
    if right - left < 80.0 {
        return;
    }
    let (lmin, lmax) = (1.0f64, 8.0f64);
    let x_of = |l: f64| left + (((l - lmin) / (lmax - lmin)) as f32) * (right - left);
    let bar_y = rect.top() + 26.0;
    let bar_h = 26.0;

    // Terra
    p.rect_filled(
        Rect::from_min_size(pos2(left - 13.0, bar_y), vec2(13.0, bar_h)),
        0,
        Color32::from_rgb(0x3B, 0x6E, 0xA5),
    );
    txt(
        &p,
        pos2(left - 26.0, bar_y + bar_h + 12.0),
        Align2::CENTER_TOP,
        "Terra",
        9.0,
        t::FAINT,
    );

    // barra colorata
    let steps = 180;
    for i in 0..steps {
        let la = lmin + (lmax - lmin) * i as f64 / steps as f64;
        let lb = lmin + (lmax - lmin) * (i + 1) as f64 / steps as f64;
        let lmid = 0.5 * (la + lb);
        let idx = (((lmid - 1.0) / 0.05).round() as usize).min(belt.profile.len().saturating_sub(1));
        let lj = belt.profile.get(idx).map(|(_, v)| *v).unwrap_or(-1.0);
        let xa = x_of(la);
        let xb = x_of(lb);
        p.rect_filled(
            Rect::from_min_size(pos2(xa, bar_y), vec2((xb - xa) + 0.7, bar_h)),
            0,
            t::flux_color(lj),
        );
    }

    // tacche L
    let mut l = 1.0;
    while l <= 8.0 {
        let x = x_of(l);
        p.line_segment(
            [pos2(x, bar_y + bar_h), pos2(x, bar_y + bar_h + 4.0)],
            t::stroke(1.0, t::FAINT),
        );
        txt(
            &p,
            pos2(x, bar_y + bar_h + 6.0),
            Align2::CENTER_TOP,
            format!("{l:.0}"),
            9.0,
            t::FAINT,
        );
        l += 1.0;
    }
    txt(
        &p,
        pos2(left, rect.top()),
        Align2::LEFT_TOP,
        "flusso di elettroni >2 MeV lungo l'equatore (modello ancorato a GOES-19)",
        9.5,
        t::SUB,
    );

    // marcatori
    let x_in = x_of(belt.l_inner_outer.clamp(lmin, lmax));
    let x_out = x_of(belt.l_outer_edge.clamp(lmin, lmax));
    p.line_segment(
        [pos2(x_in, bar_y - 3.0), pos2(x_in, bar_y + bar_h + 2.0)],
        t::stroke(1.5, t::GOOD),
    );
    p.line_segment(
        [pos2(x_out, bar_y - 3.0), pos2(x_out, bar_y + bar_h + 2.0)],
        t::stroke(1.5, t::GOOD),
    );
    txt(
        &p,
        pos2(x_in, bar_y - 5.0),
        Align2::CENTER_BOTTOM,
        format!("{:.1}", belt.l_inner_outer),
        9.0,
        t::GOOD,
    );
    txt(
        &p,
        pos2(x_out, bar_y - 5.0),
        Align2::CENTER_BOTTOM,
        format!("{:.1}", belt.l_outer_edge),
        9.0,
        t::GOOD,
    );

    let x_pp = x_of(belt.l_pp.clamp(lmin, lmax));
    p.line_segment(
        [pos2(x_pp, bar_y - 3.0), pos2(x_pp, bar_y + bar_h + 2.0)],
        t::stroke(1.5, t::ACCENT),
    );
    txt(
        &p,
        pos2(x_pp, bar_y + bar_h + 20.0),
        Align2::CENTER_TOP,
        "plasmapausa",
        9.0,
        t::ACCENT,
    );

    let x_geo = x_of(GEO_L);
    p.line_segment(
        [pos2(x_geo, bar_y - 8.0), pos2(x_geo, bar_y + bar_h + 8.0)],
        t::stroke(1.5, t::WARN),
    );
    txt(
        &p,
        pos2(x_geo + 3.0, bar_y - 10.0),
        Align2::LEFT_BOTTOM,
        "GOES 6.6",
        9.0,
        t::WARN,
    );

    if let Some(r) = r_mp {
        if r <= lmax {
            let x = x_of(r);
            p.line_segment(
                [pos2(x, bar_y - 8.0), pos2(x, bar_y + bar_h + 8.0)],
                t::stroke(2.0, t::BAD),
            );
            txt(
                &p,
                pos2(x + 3.0, bar_y - 10.0),
                Align2::LEFT_BOTTOM,
                "magnetopausa",
                9.0,
                t::BAD,
            );
        }
    }
}

// ---- spettro differenziale ---------------------------------------------

pub fn spectrum(ui: &mut Ui, width: f32, now: &[SpectrumPoint], prev: &[SpectrumPoint]) {
    let h = 246.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let p = ui.painter_at(rect);

    let left = rect.left() + 42.0;
    let right = rect.right() - 10.0;
    let top = rect.top() + 14.0;
    let bottom = rect.bottom() - 34.0;
    if right - left < 60.0 || bottom - top < 60.0 {
        return;
    }

    let (emin, emax) = (0.06f64, 4.0f64);
    let (jmin, jmax) = (-1.0f64, 5.0f64);
    let x_of = |e: f64| {
        left + (((e.ln() - emin.ln()) / (emax.ln() - emin.ln())) as f32) * (right - left)
    };
    let y_of = |lj: f64| bottom - (((lj - jmin) / (jmax - jmin)) as f32) * (bottom - top);

    for dec in 0..=5 {
        let y = y_of(dec as f64);
        p.line_segment(
            [pos2(left, y), pos2(right, y)],
            t::stroke(1.0, t::GRID),
        );
        txt(
            &p,
            pos2(rect.left() + 4.0, y),
            Align2::LEFT_CENTER,
            decade(dec as i32),
            9.0,
            t::FAINT,
        );
    }
    for e in [0.08f64, 0.1, 0.2, 0.5, 1.0, 2.0, 4.0] {
        let x = x_of(e);
        p.line_segment(
            [pos2(x, top), pos2(x, bottom)],
            t::stroke(1.0, t::GRID),
        );
        let label = if e < 1.0 {
            format!("{:.0}k", e * 1000.0)
        } else {
            format!("{e:.0}M")
        };
        txt(
            &p,
            pos2(x, bottom + 6.0),
            Align2::CENTER_TOP,
            label,
            9.0,
            t::FAINT,
        );
    }
    txt(
        &p,
        pos2(left, bottom + 20.0),
        Align2::LEFT_TOP,
        "energia (keV / MeV) · pfu per MeV",
        9.0,
        t::FAINT,
    );

    if !prev.is_empty() {
        let pts: Vec<Pos2> = prev
            .iter()
            .map(|q| pos2(x_of(q.energy_mev), y_of(q.j.log10().clamp(jmin, jmax))))
            .collect();
        p.add(Shape::line(pts, t::stroke(1.4, t::FAINT)));
    }
    if !now.is_empty() {
        let pts: Vec<Pos2> = now
            .iter()
            .map(|q| pos2(x_of(q.energy_mev), y_of(q.j.log10().clamp(jmin, jmax))))
            .collect();
        p.add(Shape::line(pts.clone(), t::stroke(2.4, t::MAG)));
        for pt in pts.iter() {
            p.circle_filled(*pt, 2.4, t::MAG);
        }
    }

    txt(
        &p,
        pos2(right, top),
        Align2::RIGHT_TOP,
        "ora",
        9.5,
        t::MAG,
    );
    txt(
        &p,
        pos2(right, top + 13.0),
        Align2::RIGHT_TOP,
        "inizio finestra",
        9.5,
        t::FAINT,
    );
}

// ---- serie temporale ----------------------------------------------------

pub fn series(ui: &mut Ui, width: f32, points: &[Sample]) {
    let h = 168.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let p = ui.painter_at(rect);

    let left = rect.left() + 44.0;
    let right = rect.right() - 8.0;
    let top = rect.top() + 14.0;
    let bottom = rect.bottom() - 30.0;

    let (jmin, jmax) = (0.0f64, 5.0f64);
    let y_of = |lj: f64| bottom - (((lj - jmin) / (jmax - jmin)) as f32) * (bottom - top);

    for dec in 0..=5 {
        let y = y_of(dec as f64);
        p.line_segment(
            [pos2(left, y), pos2(right, y)],
            t::stroke(1.0, t::GRID),
        );
        txt(
            &p,
            pos2(rect.left() + 4.0, y),
            Align2::LEFT_CENTER,
            decade(dec as i32),
            9.0,
            t::FAINT,
        );
    }

    if points.len() < 2 {
        txt(
            &p,
            pos2(left + 8.0, top + 24.0),
            Align2::LEFT_TOP,
            "serie non disponibile",
            11.0,
            t::FAINT,
        );
        return;
    }

    let t0 = points.first().map(|s| s.t).unwrap_or(0);
    let t1 = points.last().map(|s| s.t).unwrap_or(1);
    let span = (t1 - t0).max(1) as f64;
    let x_of = |time: i64| left + (((time - t0) as f64 / span) as f32) * (right - left);

    for k in 0..=6 {
        let tk = t0 + (span as i64) * k / 6;
        let x = x_of(tk);
        p.line_segment(
            [pos2(x, top), pos2(x, bottom)],
            t::stroke(1.0, t::GRID),
        );
        txt(
            &p,
            pos2(x, bottom + 6.0),
            // l'ultima etichetta è allineata a destra: centrata uscirebbe dal riquadro
            if k == 6 { Align2::RIGHT_TOP } else { Align2::CENTER_TOP },
            fmt_hhmm(tk),
            9.0,
            t::FAINT,
        );
    }

    let y_thr = y_of(1000f64.log10());
    dashed_h(&p, y_thr, left, right, t::WARN.gamma_multiply(0.65), 1.0);
    txt(
        &p,
        pos2(right, y_thr - 3.0),
        Align2::RIGHT_BOTTOM,
        "soglia 1000 pfu",
        9.0,
        t::WARN,
    );

    let pts: Vec<Pos2> = points
        .iter()
        .map(|s| pos2(x_of(s.t), y_of(s.v.max(1e-3).log10().clamp(jmin, jmax))))
        .collect();
    filled_area(ui, &p, &pts, bottom, t::FLUX.gamma_multiply(0.32), t::FLUX);

    let last = points[points.len() - 1];
    p.circle_filled(*pts.last().unwrap(), 3.0, t::TEXT);
    txt(
        &p,
        pos2(rect.right() - 6.0, rect.top()),
        Align2::RIGHT_TOP,
        format!("ultimo {:.0} pfu", last.v),
        10.0,
        t::TEXT,
    );
}
