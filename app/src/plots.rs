//! Serie temporali generiche per le schede Vento solare, Magnetosfera e Storico.
//!
//! Un solo grafico configurabile ([`timeseries`]) più le barre del Kp. Ogni
//! grafico mostra il valore sotto il puntatore.

use crate::charts::{dashed_h, decade, filled_area, txt};
use crate::theme as t;
use eframe::egui::{
    pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Shape, StrokeKind, Ui,
};
use vanallen_core::sources::Sample;
use vanallen_core::time::{civil_from_unix, fmt_short};

/// Una curva di una serie temporale.
pub struct Line<'a> {
    pub pts: &'a [Sample],
    pub color: Color32,
    pub label: &'a str,
    /// riempie l'area fra la curva e lo zero (o la base, in scala logaritmica)
    pub fill: bool,
}

/// Aspetto del grafico.
pub struct Plot<'a> {
    pub height: f32,
    /// asse verticale in decadi
    pub log: bool,
    /// valori che l'asse verticale deve comunque comprendere
    pub include: &'a [f64],
    /// soglie orizzontali tratteggiate: (valore, colore, etichetta)
    pub marks: &'a [(f64, Color32, &'a str)],
    pub unit: &'a str,
    /// finestra temporale; se manca si usa l'estensione dei dati
    pub span: Option<(i64, i64)>,
}

impl Default for Plot<'_> {
    fn default() -> Self {
        Self {
            height: 190.0,
            log: false,
            include: &[],
            marks: &[],
            unit: "",
            span: None,
        }
    }
}

/// Passo "tondo" (1, 2, 5 × 10ⁿ) non più piccolo di `x`.
fn nice_step(x: f64) -> f64 {
    if !(x.is_finite() && x > 0.0) {
        return 1.0;
    }
    let mag = 10f64.powf(x.log10().floor());
    let n = x / mag;
    let k = if n <= 1.0 {
        1.0
    } else if n <= 2.0 {
        2.0
    } else if n <= 5.0 {
        5.0
    } else {
        10.0
    };
    k * mag
}

/// Valore leggibile per etichette e fumetti.
pub fn fmt_val(v: f64) -> String {
    let a = v.abs();
    if a >= 1.0e6 {
        format!("{:.2}M", v / 1.0e6)
    } else if a >= 10_000.0 {
        format!("{:.0}k", v / 1000.0)
    } else if a >= 100.0 {
        format!("{v:.0}")
    } else if a >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

fn tick_label(v: f64, step: f64) -> String {
    if v.abs() >= 10_000.0 {
        format!("{:.0}k", v / 1000.0)
    } else if step >= 1.0 {
        format!("{v:.0}")
    } else if step >= 0.1 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// Etichetta di tempo adatta all'ampiezza della finestra.
fn fmt_time(epoch: i64, span: i64) -> String {
    let (_, mo, d, h, mi, _) = civil_from_unix(epoch);
    if span > 4 * 86_400 {
        format!("{d:02}/{mo:02}")
    } else if span > 26 * 3600 {
        format!("{d:02}/{mo:02} {h:02}h")
    } else {
        format!("{h:02}:{mi:02}")
    }
}

/// Campione più vicino a `t`.
fn nearest(pts: &[Sample], t: i64) -> Option<Sample> {
    let i = pts.partition_point(|s| s.t < t);
    let a = if i > 0 { pts.get(i - 1) } else { None };
    let b = pts.get(i);
    match (a, b) {
        (Some(a), Some(b)) => Some(if (t - a.t).abs() <= (b.t - t).abs() {
            *a
        } else {
            *b
        }),
        (Some(a), None) => Some(*a),
        (None, Some(b)) => Some(*b),
        (None, None) => None,
    }
}

/// Fumetto con più righe colorate, tenuto dentro `bounds`.
fn tooltip(p: &Painter, at: Pos2, bounds: Rect, rows: &[(String, Color32)]) {
    let galleys: Vec<_> = rows
        .iter()
        .map(|(s, c)| p.layout_no_wrap(s.clone(), FontId::proportional(10.5), *c))
        .collect();
    let w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + 16.0;
    let h = galleys.iter().map(|g| g.size().y + 2.0).sum::<f32>() + 10.0;
    let mut x = at.x + 12.0;
    if x + w > bounds.right() {
        x = at.x - 12.0 - w;
    }
    let x = x.max(bounds.left());
    let y = (at.y - h * 0.5).clamp(bounds.top(), (bounds.bottom() - h).max(bounds.top()));
    let r = Rect::from_min_size(pos2(x, y), vec2(w, h));
    p.rect_filled(r, 6, Color32::from_rgba_unmultiplied(0x1C, 0x1C, 0x1C, 240));
    p.rect_stroke(r, 6, t::stroke(1.0, t::BORDER), StrokeKind::Inside);
    let mut yy = r.top() + 5.0;
    for (g, (_, c)) in galleys.into_iter().zip(rows.iter()) {
        let gh = g.size().y;
        p.galley(pos2(r.left() + 8.0, yy), g, *c);
        yy += gh + 2.0;
    }
}

/// Etichette dell'asse del tempo.
fn time_axis(p: &Painter, left: f32, right: f32, top: f32, bottom: f32, t0: i64, t1: i64, grid: bool) {
    let divisions = if right - left > 520.0 { 6 } else { 4 };
    for k in 0..=divisions {
        let tk = t0 + ((t1 - t0) * k) / divisions;
        let x = left + (k as f32 / divisions as f32) * (right - left);
        if grid {
            p.line_segment([pos2(x, top), pos2(x, bottom)], t::stroke(1.0, t::GRID));
        }
        let anchor = if k == divisions {
            Align2::RIGHT_TOP
        } else if k == 0 {
            Align2::LEFT_TOP
        } else {
            Align2::CENTER_TOP
        };
        txt(p, pos2(x, bottom + 6.0), anchor, fmt_time(tk, t1 - t0), 9.0, t::FAINT);
    }
}

/// Grafico di una o più serie nel tempo, con griglia, soglie, legenda e
/// lettura del valore sotto il puntatore.
pub fn timeseries(ui: &mut Ui, width: f32, lines: &[Line], plot: &Plot) {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, plot.height), Sense::hover());
    let p = ui.painter_at(rect);

    let left = rect.left() + 46.0;
    let right = rect.right() - 12.0;
    let top = rect.top() + 18.0;
    let bottom = rect.bottom() - 24.0;
    if right - left < 60.0 || bottom - top < 40.0 {
        return;
    }

    let usable = |v: f64| v.is_finite() && (!plot.log || v > 0.0);
    let tr = |v: f64| if plot.log { v.max(1e-12).log10() } else { v };
    let empty = |msg: &str| {
        txt(&p, pos2(left + 8.0, top + 20.0), Align2::LEFT_TOP, msg, 11.0, t::FAINT);
    };

    // finestra temporale
    let span = plot.span.or_else(|| {
        let a = lines.iter().filter_map(|l| l.pts.first()).map(|s| s.t).min()?;
        let b = lines.iter().filter_map(|l| l.pts.last()).map(|s| s.t).max()?;
        Some((a, b))
    });
    let Some((t0, t1)) = span else {
        empty("serie non disponibile");
        return;
    };
    let t1 = t1.max(t0 + 60);
    let dt = (t1 - t0) as f64;

    // intervallo verticale sui soli punti visibili
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut count = 0usize;
    for l in lines {
        for s in l.pts.iter().filter(|s| s.t >= t0 && s.t <= t1 && usable(s.v)) {
            let y = tr(s.v);
            lo = lo.min(y);
            hi = hi.max(y);
            count += 1;
        }
    }
    if count < 2 {
        empty("dati insufficienti in questa finestra");
        return;
    }
    for v in plot.include.iter().filter(|v| usable(**v)) {
        lo = lo.min(tr(*v));
        hi = hi.max(tr(*v));
    }
    let (lo, hi, step) = if plot.log {
        let lo = lo.floor();
        let hi = hi.ceil().max(lo + 1.0);
        (lo, hi, if hi - lo > 6.0 { 2.0 } else { 1.0 })
    } else {
        if hi - lo < 1e-9 {
            let pad = (lo.abs() * 0.1).max(0.5);
            lo -= pad;
            hi += pad;
        }
        let step = nice_step((hi - lo) / 4.0);
        ((lo / step).floor() * step, (hi / step).ceil() * step, step)
    };

    let x_of = |time: i64| left + (((time - t0) as f64 / dt) as f32) * (right - left);
    let y_of = |y: f64| bottom - (((y - lo) / (hi - lo)) as f32) * (bottom - top);

    // griglia orizzontale
    let mut y = lo;
    let mut guard = 0;
    while y <= hi + step * 0.01 && guard < 40 {
        let py = y_of(y);
        p.line_segment([pos2(left, py), pos2(right, py)], t::stroke(1.0, t::GRID));
        let label = if plot.log {
            decade(y.round() as i32)
        } else {
            tick_label(y, step)
        };
        txt(&p, pos2(left - 6.0, py), Align2::RIGHT_CENTER, label, 9.0, t::FAINT);
        y += step;
        guard += 1;
    }
    if !plot.log && lo < 0.0 && hi > 0.0 {
        let py = y_of(0.0);
        p.line_segment([pos2(left, py), pos2(right, py)], t::stroke(1.0, t::BORDER));
    }
    time_axis(&p, left, right, top, bottom, t0, t1, true);

    // unità e legenda
    txt(&p, pos2(left, rect.top()), Align2::LEFT_TOP, plot.unit, 9.5, t::FAINT);
    let mut lx = right;
    for l in lines.iter().rev() {
        if l.label.is_empty() {
            continue;
        }
        let g = p.layout_no_wrap(l.label.to_string(), FontId::proportional(9.5), l.color);
        lx -= g.size().x;
        p.galley(pos2(lx, rect.top()), g, l.color);
        p.line_segment(
            [pos2(lx - 16.0, rect.top() + 7.0), pos2(lx - 4.0, rect.top() + 7.0)],
            t::stroke(2.0, l.color),
        );
        lx -= 30.0;
    }

    // soglie
    for (v, color, label) in plot.marks {
        if !usable(*v) {
            continue;
        }
        let yv = tr(*v);
        if yv < lo || yv > hi {
            continue;
        }
        let py = y_of(yv);
        dashed_h(&p, py, left, right, color.gamma_multiply(0.7), 1.0);
        // a sinistra (a destra c'è il dato più recente) e sotto la linea se
        // questa è vicina al bordo alto, dove sta la legenda
        if py - top < 14.0 {
            txt(&p, pos2(left + 5.0, py + 2.0), Align2::LEFT_TOP, label, 9.0, *color);
        } else {
            txt(&p, pos2(left + 5.0, py - 2.0), Align2::LEFT_BOTTOM, label, 9.0, *color);
        }
    }

    // curve
    let base = if plot.log {
        bottom
    } else {
        y_of(0.0f64.clamp(lo, hi))
    };
    for l in lines {
        let pts: Vec<Pos2> = l
            .pts
            .iter()
            .filter(|s| s.t >= t0 && s.t <= t1 && usable(s.v))
            .map(|s| pos2(x_of(s.t), y_of(tr(s.v).clamp(lo, hi))))
            .collect();
        if pts.len() < 2 {
            continue;
        }
        if l.fill {
            filled_area(ui, &p, &pts, base, l.color.gamma_multiply(0.30), l.color);
        } else {
            p.add(Shape::line(pts.clone(), t::stroke(1.8, l.color)));
        }
        if let Some(last) = pts.last() {
            p.circle_filled(*last, 2.6, l.color);
        }
    }

    // lettura sotto il puntatore
    if let Some(pos) = resp.hover_pos() {
        if pos.x >= left && pos.x <= right && pos.y >= top - 4.0 && pos.y <= bottom + 4.0 {
            let th = t0 + (((pos.x - left) / (right - left)) as f64 * dt) as i64;
            let reach = ((dt / 25.0) as i64).max(900);
            let mut rows: Vec<(String, Color32)> = Vec::new();
            let mut when: Option<i64> = None;
            for l in lines {
                let Some(s) = nearest(l.pts, th) else { continue };
                if (s.t - th).abs() > reach || !usable(s.v) {
                    continue;
                }
                when.get_or_insert(s.t);
                p.circle_filled(pos2(x_of(s.t), y_of(tr(s.v).clamp(lo, hi))), 3.2, l.color);
                rows.push((
                    format!("{}  {} {}", l.label, fmt_val(s.v), plot.unit)
                        .trim()
                        .to_string(),
                    l.color,
                ));
            }
            if let Some(tw) = when {
                p.line_segment([pos2(pos.x, top), pos2(pos.x, bottom)], t::stroke(1.0, t::FAINT));
                rows.insert(0, (fmt_short(tw) + " UTC", t::SUB));
                tooltip(&p, pos, rect, &rows);
            }
        }
    }
}

/// Colore di un valore di Kp: verde quieto, giallo attivo, poi le classi G.
pub fn kp_color(kp: f64) -> Color32 {
    if kp >= 7.0 {
        t::SEVERE
    } else if kp >= 6.0 {
        t::BAD
    } else if kp >= 5.0 {
        t::MAG
    } else if kp >= 4.0 {
        t::WARN
    } else {
        t::GOOD
    }
}

/// Kp nel tempo come barre colorate per classe.
pub fn kp_bars(ui: &mut Ui, width: f32, height: f32, pts: &[Sample]) {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let p = ui.painter_at(rect);
    let left = rect.left() + 46.0;
    let right = rect.right() - 12.0;
    let top = rect.top() + 18.0;
    let bottom = rect.bottom() - 24.0;
    if right - left < 60.0 || bottom - top < 40.0 {
        return;
    }
    let y_of = |k: f64| bottom - ((k / 9.0) as f32) * (bottom - top);
    for k in 0..=9 {
        let y = y_of(k as f64);
        p.line_segment([pos2(left, y), pos2(right, y)], t::stroke(1.0, t::GRID));
        if k % 2 == 1 || k == 0 {
            txt(&p, pos2(left - 6.0, y), Align2::RIGHT_CENTER, k, 9.0, t::FAINT);
        }
    }
    txt(&p, pos2(left, rect.top()), Align2::LEFT_TOP, "Kp", 9.5, t::FAINT);

    if pts.len() < 2 {
        txt(
            &p,
            pos2(left + 8.0, top + 20.0),
            Align2::LEFT_TOP,
            "serie non disponibile",
            11.0,
            t::FAINT,
        );
        return;
    }
    let t0 = pts[0].t;
    let t1 = pts[pts.len() - 1].t.max(t0 + 60);
    let dt = (t1 - t0) as f64;
    let x_of = |time: i64| left + (((time - t0) as f64 / dt) as f32) * (right - left);
    time_axis(&p, left, right, top, bottom, t0, t1, false);

    let mut mesh = eframe::egui::epaint::Mesh::default();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let k = a.v.clamp(0.0, 9.0);
        let xa = x_of(a.t);
        let xb = x_of(b.t).max(xa + 1.0) + 0.5;
        let ya = y_of(k).min(bottom - 2.0);
        let col = kp_color(k);
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(xa, ya), col);
        mesh.colored_vertex(pos2(xb, ya), col);
        mesh.colored_vertex(pos2(xb, bottom), col.gamma_multiply(0.35));
        mesh.colored_vertex(pos2(xa, bottom), col.gamma_multiply(0.35));
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    p.add(Shape::mesh(mesh));

    let y5 = y_of(5.0);
    dashed_h(&p, y5, left, right, t::MAG.gamma_multiply(0.7), 1.0);
    txt(
        &p,
        pos2(left + 5.0, y5 - 2.0),
        Align2::LEFT_BOTTOM,
        "Kp 5 = tempesta G1",
        9.0,
        t::MAG,
    );

    if let Some(pos) = resp.hover_pos() {
        if pos.x >= left && pos.x <= right && pos.y >= top - 4.0 && pos.y <= bottom + 4.0 {
            let th = t0 + (((pos.x - left) / (right - left)) as f64 * dt) as i64;
            if let Some(s) = nearest(pts, th) {
                p.line_segment([pos2(pos.x, top), pos2(pos.x, bottom)], t::stroke(1.0, t::FAINT));
                tooltip(
                    &p,
                    pos,
                    rect,
                    &[
                        (fmt_short(s.t) + " UTC", t::SUB),
                        (format!("Kp stimato  {:.2}", s.v), kp_color(s.v)),
                    ],
                );
            }
        }
    }
}
