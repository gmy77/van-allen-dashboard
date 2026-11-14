//! Server HTTP locale, in sola lettura.
//!
//! Parte insieme all'app e muore con lei: il listener vive dentro il processo,
//! quindi chiudendo la finestra la porta si libera da sola. Serve due cose:
//!
//! * `GET /` — pagina HTML con gli stessi numeri della dashboard (si aggiorna
//!   da sola ogni 30 s con un meta refresh);
//! * `GET /snapshot.json` — gli stessi dati in JSON, per script e altri programmi.
//!
//! Ascolta solo su 127.0.0.1: non è esposto sulla rete.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vanallen_core::physics::RE_KM;
use vanallen_core::time::fmt_stamp;
use vanallen_core::{Assessment, BeltRegime, Severity, Snapshot, StormLevel};

pub const DEFAULT_PORT: u16 = 8787;

#[derive(Clone, Default)]
pub struct Payload {
    pub json: String,
    pub html: String,
}

pub type Shared = Arc<Mutex<Payload>>;

/// Avvia il server, provando la porta richiesta e le successive.
pub fn start(want: u16) -> Option<(u16, Shared)> {
    let mut port = want;
    let listener = loop {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(l) => break l,
            Err(_) => {
                if port >= want.saturating_add(10) {
                    return None;
                }
                port += 1;
            }
        }
    };
    listener.set_nonblocking(true).ok()?;

    let shared: Shared = Arc::new(Mutex::new(Payload::default()));
    let state = Arc::clone(&shared);
    std::thread::spawn(move || loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = serve(stream, &state);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(_) => return,
        }
    });
    Some((port, shared))
}

fn serve(mut stream: TcpStream, state: &Shared) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;

    let path = request
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();

    let (body, kind, status) = match path.as_str() {
        "/" | "/index.html" => (
            state.lock().map(|p| p.html.clone()).unwrap_or_default(),
            "text/html; charset=utf-8",
            200,
        ),
        "/snapshot.json" | "/data.json" => (
            state.lock().map(|p| p.json.clone()).unwrap_or_default(),
            "application/json; charset=utf-8",
            200,
        ),
        "/health" => ("ok\n".to_string(), "text/plain; charset=utf-8", 200),
        _ => (
            "404: percorsi disponibili / , /snapshot.json , /health\n".to_string(),
            "text/plain; charset=utf-8",
            404,
        ),
    };

    let head = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()?;
    Ok(())
}

// ---- costruzione dei contenuti -----------------------------------------

/// Costruisce JSON e HTML dall'ultima fotografia.
pub fn payload(snap: &Snapshot, a: &Assessment) -> Payload {
    Payload {
        json: build_json(snap, a),
        html: build_html(snap, a),
    }
}

/// Numero JSON: `null` se manca il dato.
fn n(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{x:.3}"),
        _ => "null".to_string(),
    }
}

/// Stringa JSON con gli escape minimi.
fn js(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Testo HTML senza caratteri che rompono il markup.
fn he(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn sev(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Watch => "attenzione",
        Severity::Warn => "allerta",
        Severity::Severe => "grave",
    }
}

fn storm(s: StormLevel) -> &'static str {
    s.label()
}

fn regime(r: BeltRegime) -> &'static str {
    r.label()
}

fn build_json(snap: &Snapshot, a: &Assessment) -> String {
    let belt = a.belt.as_ref();
    let alerts: Vec<String> = a
        .alerts
        .iter()
        .map(|al| {
            format!(
                "{{\"livello\":{},\"titolo\":{},\"dettaglio\":{}}}",
                js(sev(al.severity)),
                js(&al.title),
                js(&al.detail)
            )
        })
        .collect();

    format!(
        "{{\n\
         \"aggiornato\": {},\n\
         \"timestamp\": {},\n\
         \"satellite_goes\": {},\n\
         \"kp\": {},\n\
         \"scala_g\": {},\n\
         \"dst_nt\": {},\n\
         \"vento\": {{\"velocita_km_s\": {}, \"densita_cm3\": {}, \"bz_nt\": {}, \"bt_nt\": {}, \"p_dinamica_npa\": {}}},\n\
         \"magnetopausa_re\": {},\n\
         \"magnetopausa_km\": {},\n\
         \"plasmapausa_re\": {},\n\
         \"elettroni\": {{\"flusso_2mev_pfu\": {}, \"flusso_800kev_pfu\": {}, \"gamma\": {}, \"flusso_energia_uw_m2\": {}, \"protoni_10mev_pfu\": {}}},\n\
         \"fasce\": {{\"bordo_interno_re\": {}, \"bordo_esterno_re\": {}, \"picco_re\": {}, \"larghezza_re\": {}, \"contenuto\": {}, \"regime\": {}, \"troncata_dalla_magnetopausa\": {}}},\n\
         \"stato\": {},\n\
         \"allerte\": [{}]\n\
         }}\n",
        js(&fmt_stamp(snap.fetched_at)),
        snap.fetched_at,
        snap.goes_e.satellite,
        n(a.kp),
        a.g_scale,
        n(a.dst),
        n(a.wind_speed),
        n(a.wind_density),
        n(a.bz),
        n(a.bt),
        n(a.pdyn),
        n(a.r_mp),
        n(a.r_mp.map(|r| r * RE_KM)),
        n(belt.map(|b| b.l_pp)),
        n(a.geo_flux),
        n(a.geo_flux_800),
        n(a.gamma),
        n(a.energy_flux),
        n(a.protons_10mev),
        n(belt.map(|b| b.l_inner_outer)),
        n(belt.map(|b| b.l_outer_edge)),
        n(belt.map(|b| b.l_peak)),
        n(belt.map(|b| b.width)),
        n(belt.map(|b| b.content_index)),
        belt.map(|b| js(regime(b.regime)))
            .unwrap_or_else(|| "null".into()),
        belt.map(|b| b.truncated_by_magnetopause).unwrap_or(false),
        js(storm(a.storm)),
        alerts.join(", ")
    )
}

fn build_html(snap: &Snapshot, a: &Assessment) -> String {
    let num = |v: Option<f64>, unit: &str, dec: usize| -> String {
        match v {
            Some(x) => format!("{x:.*} {unit}", dec, x = x).trim().to_string(),
            None => "—".to_string(),
        }
    };

    let rows = vec![
        ("Kp stimato", num(a.kp, "", 1), format!("scala G{}", a.g_scale)),
        ("Dst", num(a.dst, "nT", 0), storm(a.storm).to_string()),
        (
            "Vento solare",
            num(a.wind_speed, "km/s", 0),
            format!("n = {}", num(a.wind_density, "cm⁻³", 1)),
        ),
        (
            "Bz / pressione dinamica",
            num(a.bz, "nT", 1),
            format!("P = {}", num(a.pdyn, "nPa", 2)),
        ),
        (
            "Magnetopausa",
            num(a.r_mp, "R⊕", 1),
            format!("{} km", num(a.r_mp.map(|r| r * RE_KM), "", 0)),
        ),
        (
            "Elettroni >2 MeV a L = 6.6",
            num(a.geo_flux, "pfu", 0),
            format!("800 keV: {}", num(a.geo_flux_800, "pfu", 0)),
        ),
        (
            "Spettro",
            format!("γ = {}", num(a.gamma, "", 2)),
            format!("flusso di energia {}", num(a.energy_flux, "µW/m²", 1)),
        ),
    ];

    let mut table = String::new();
    for (k, v, extra) in &rows {
        table.push_str(&format!(
            "<tr><th>{}</th><td>{}</td><td class=\"hint\">{}</td></tr>\n",
            he(k),
            he(v),
            he(extra)
        ));
    }

    if let Some(b) = &a.belt {
        table.push_str(&format!(
            "<tr><th>Fasce</th><td>bordo interno {:.1} R⊕ · esterno {:.1} R⊕ · picco {:.1} R⊕</td><td class=\"hint\">ampiezza {:.1} R⊕ · contenuto {:.0} (u.a.) · {}</td></tr>\n",
            b.l_inner_outer,
            b.l_outer_edge,
            b.l_peak,
            b.width,
            b.content_index,
            he(regime(b.regime))
        ));
    }

    let mut alerts = String::new();
    if a.alerts.is_empty() {
        alerts.push_str("<p class=\"ok\">Nessuna soglia operativa superata.</p>\n");
    } else {
        alerts.push_str("<ul>\n");
        for al in a.alerts.iter().take(8) {
            alerts.push_str(&format!(
                "<li><b>{}</b> <span class=\"hint\">[{}]</span><br>{}</li>\n",
                he(&al.title),
                he(sev(al.severity)),
                he(&al.detail)
            ));
        }
        alerts.push_str("</ul>\n");
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="it">
<head>
<meta charset="utf-8">
<meta http-equiv="refresh" content="30">
<title>Fasce di Van Allen — live</title>
<style>
  :root {{ color-scheme: dark; }}
  body {{ margin:0; padding:28px; background:#202020; color:#ffffff;
         font:14px/1.5 "Segoe UI", system-ui, sans-serif; }}
  h1 {{ font-size:20px; margin:0 0 4px; }}
  .sub {{ color:#8a8a8a; font-size:12.5px; margin-bottom:20px; }}
  .wrap {{ max-width:820px; }}
  table {{ width:100%; border-collapse:collapse; background:#272727;
           border:1px solid #3d3d3d; border-radius:8px; overflow:hidden; }}
  tr + tr {{ border-top:1px solid #3d3d3d; }}
  th {{ text-align:left; font-weight:600; font-size:12.5px; color:#c8c8c8;
        padding:9px 14px; width:250px; vertical-align:top; }}
  td {{ padding:9px 14px; font-size:15px; }}
  td.hint {{ color:#8a8a8a; font-size:12.5px; }}
  .ok {{ color:#6ccb5f; }}
  ul {{ padding-left:20px; }} li {{ margin-bottom:8px; }}
  .hint {{ color:#8a8a8a; font-size:12px; }}
  a {{ color:#4cc2ff; }}
  footer {{ margin-top:18px; color:#8a8a8a; font-size:12px; }}
</style>
</head>
<body>
<div class="wrap">
<h1>Fasce di Van Allen — live</h1>
<div class="sub">aggiornato {stamp} · satellite GOES-{sat} · pagina servita dall'app,
si aggiorna da sola ogni 30 s</div>
<table>
{table}</table>
<h2 style="font-size:15px;margin:22px 0 8px">Allerte</h2>
{alerts}
<footer>
Misure: flussi GOES, vento solare e IMF a L1, Kp, Dst.
Modello: posizione delle fasce, plasmapausa, magnetopausa.
Dati grezzi: <a href="/snapshot.json">/snapshot.json</a> ·
<a href="/health">/health</a>
</footer>
</div>
</body>
</html>
"#,
        stamp = fmt_stamp(snap.fetched_at),
        sat = snap.goes_e.satellite,
        table = table,
        alerts = alerts
    )
}
