//! Storico remoto: i punti registrati 24 ore su 24 dal Worker `worker/`.
//!
//! Il Worker salva solo misure (flusso, Kp, Dst, vento solare, IMF). Qui si
//! ricostruiscono le grandezze di modello con la stessa fisica della
//! fotografia live, così un punto remoto e uno locale sono confrontabili.

use crate::history::HistoryPoint;
use crate::physics;
use serde_json::Value;

/// Misure di un istante, come le salva il Worker.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measures {
    pub t: i64,
    pub flux2: Option<f64>,
    pub kp: Option<f64>,
    pub dst: Option<f64>,
    pub bz: Option<f64>,
    pub speed: Option<f64>,
    pub density: Option<f64>,
}

impl Measures {
    /// Punto di storico con le grandezze di modello ricalcolate.
    ///
    /// Stesse regole di `Snapshot::assess`: magnetopausa solo se ci sono Bz,
    /// densità e velocità; forma delle fasce solo se c'è il flusso, con Kp 2 e
    /// Dst 0 come ripiego quando mancano.
    pub fn to_point(&self) -> HistoryPoint {
        let pdyn = match (self.density, self.speed) {
            (Some(n), Some(v)) => Some(physics::dynamic_pressure_npa(n, v)),
            _ => None,
        };
        let r_mp = match (self.bz, pdyn) {
            (Some(z), Some(p)) => Some(physics::magnetopause_standoff(z, p)),
            _ => None,
        };
        let belt = self
            .flux2
            .filter(|j| *j > 0.0)
            .map(|j| physics::belt_shape(j, self.kp.unwrap_or(2.0), self.dst.unwrap_or(0.0), r_mp));
        HistoryPoint {
            t: self.t,
            flux2: self.flux2,
            kp: self.kp,
            dst: self.dst,
            r_mp,
            l_peak: belt.as_ref().map(|b| b.l_peak),
            l_outer: belt.as_ref().map(|b| b.l_outer_edge),
            content: belt.as_ref().map(|b| b.content_index),
        }
    }
}

/// Interpreta la risposta di `GET /history`.
///
/// Forma: `{"columns": ["t","flux2",…], "points": [[…], …]}`. Le colonne sono
/// cercate per nome, quindi il Worker può aggiungerne o riordinarle senza
/// rompere le versioni vecchie dell'app. Righe senza tempo valido sono saltate.
pub fn parse_history(v: &Value) -> Result<Vec<Measures>, String> {
    let columns = v
        .get("columns")
        .and_then(|c| c.as_array())
        .ok_or("risposta senza elenco delle colonne")?;
    let index = |name: &str| columns.iter().position(|c| c.as_str() == Some(name));
    let i_t = index("t").ok_or("risposta senza la colonna del tempo")?;
    let cols = [
        index("flux2"),
        index("kp"),
        index("dst"),
        index("bz"),
        index("speed"),
        index("density"),
    ];
    let rows = v
        .get("points")
        .and_then(|p| p.as_array())
        .ok_or("risposta senza punti")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(cells) = row.as_array() else { continue };
        let Some(t) = cells.get(i_t).and_then(|x| x.as_i64()).filter(|t| *t > 0) else {
            continue;
        };
        let cell = |i: Option<usize>| -> Option<f64> {
            i.and_then(|i| cells.get(i))
                .and_then(|x| x.as_f64())
                .filter(|x| x.is_finite())
        };
        out.push(Measures {
            t,
            flux2: cell(cols[0]),
            kp: cell(cols[1]),
            dst: cell(cols[2]),
            bz: cell(cols[3]),
            speed: cell(cols[4]),
            density: cell(cols[5]),
        });
    }
    out.sort_by_key(|m| m.t);
    Ok(out)
}

/// Indirizzo del Worker ripulito, oppure None se non è utilizzabile.
///
/// Si accetta solo https (o http verso questo stesso PC, per le prove): lo
/// storico non deve viaggiare in chiaro né finire su indirizzi scritti male.
pub fn clean_url(raw: &str) -> Option<String> {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))?;
    let url = line.trim_end_matches('/');
    let local = url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost");
    let host = url.strip_prefix("https://").unwrap_or("");
    if local || (!host.is_empty() && !host.contains(char::is_whitespace)) {
        Some(url.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_matches_live_physics() {
        let m = Measures {
            t: 1000,
            flux2: Some(5000.0),
            kp: Some(1.0),
            dst: Some(-10.0),
            bz: Some(-2.0),
            speed: Some(400.0),
            density: Some(5.0),
        };
        let p = m.to_point();
        let pdyn = physics::dynamic_pressure_npa(5.0, 400.0);
        let r_mp = physics::magnetopause_standoff(-2.0, pdyn);
        let belt = physics::belt_shape(5000.0, 1.0, -10.0, Some(r_mp));
        assert_eq!(p.t, 1000);
        assert_eq!(p.r_mp, Some(r_mp));
        assert_eq!(p.l_peak, Some(belt.l_peak));
        assert_eq!(p.l_outer, Some(belt.l_outer_edge));
        assert_eq!(p.content, Some(belt.content_index));
    }

    #[test]
    fn missing_measures_give_no_model() {
        let p = Measures {
            t: 5,
            kp: Some(3.0),
            ..Default::default()
        }
        .to_point();
        assert_eq!(p.kp, Some(3.0));
        assert!(p.r_mp.is_none() && p.l_peak.is_none() && p.content.is_none());
        // vento presente ma Bz assente: niente magnetopausa
        let p = Measures {
            t: 5,
            speed: Some(400.0),
            density: Some(5.0),
            ..Default::default()
        }
        .to_point();
        assert!(p.r_mp.is_none());
    }

    #[test]
    fn parses_by_column_name() {
        let v: Value = serde_json::from_str(
            r#"{"columns":["t","kp","nuova","flux2","dst","bz","bt","speed","density"],
                "points":[[600,0.33,"x",5470.2,-2,1.64,3.9,421.9,1.93],
                          [300,null,null,null,null,null,null,null,null],
                          ["rotto",1,1,1,1,1,1,1,1],
                          [0,1,1,1,1,1,1,1,1]]}"#,
        )
        .unwrap();
        let m = parse_history(&v).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].t, 300);
        assert_eq!(m[0].kp, None);
        assert_eq!(m[1].flux2, Some(5470.2));
        assert_eq!(m[1].kp, Some(0.33));
        assert_eq!(m[1].density, Some(1.93));
    }

    #[test]
    fn rejects_other_shapes() {
        assert!(parse_history(&serde_json::json!({"error": "boh"})).is_err());
        assert!(parse_history(&serde_json::json!({"columns": ["kp"], "points": []})).is_err());
        assert!(parse_history(&serde_json::json!([1, 2])).is_err());
    }

    #[test]
    fn urls() {
        assert_eq!(
            clean_url("# commento\n\n  https://esempio.workers.dev/ \n"),
            Some("https://esempio.workers.dev".to_string())
        );
        assert_eq!(clean_url("http://127.0.0.1:8787/"), Some("http://127.0.0.1:8787".to_string()));
        assert_eq!(clean_url("http://esempio.workers.dev"), None);
        assert_eq!(clean_url("esempio.workers.dev"), None);
        assert_eq!(clean_url("https://"), None);
        assert_eq!(clean_url(""), None);
    }
}
