//! Storico compatto delle grandezze derivate, persistito su disco.
//!
//! Serve ai grafici di tendenza: il dato live dura poche ore, questo file
//! accumula un campione per ogni aggiornamento (uno ogni 5 minuti).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryPoint {
    pub t: i64,
    #[serde(default)]
    pub flux2: Option<f64>,
    #[serde(default)]
    pub kp: Option<f64>,
    #[serde(default)]
    pub dst: Option<f64>,
    #[serde(default)]
    pub r_mp: Option<f64>,
    #[serde(default)]
    pub l_peak: Option<f64>,
    #[serde(default)]
    pub l_outer: Option<f64>,
    #[serde(default)]
    pub content: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct History {
    #[serde(default)]
    pub points: Vec<HistoryPoint>,
}

const MAX_POINTS: usize = 20_000;

impl History {
    pub fn path() -> PathBuf {
        let base = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        base.join("van-allen-dashboard").join("history.json")
    }

    pub fn load() -> Self {
        let p = Self::path();
        std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str::<Self>(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(&p, json).map_err(|e| e.to_string())
    }

    /// Aggiunge un punto; ignora i duplicati ravvicinati (stesso minuto).
    pub fn push(&mut self, p: HistoryPoint) {
        if let Some(last) = self.points.last() {
            if (p.t - last.t).abs() < 60 {
                *self.points.last_mut().unwrap() = p;
                return;
            }
        }
        self.points.push(p);
        if self.points.len() > MAX_POINTS {
            let drop = self.points.len() - MAX_POINTS;
            self.points.drain(0..drop);
        }
    }

    /// File accanto allo storico con l'indirizzo del Worker (facoltativo).
    pub fn remote_file() -> PathBuf {
        Self::path().with_file_name("remote.txt")
    }

    /// Unisce punti arrivati da fuori (lo storico remoto) a quelli locali.
    ///
    /// Il risultato resta ordinato nel tempo e senza doppioni ravvicinati: fra
    /// due punti a meno di 150 secondi si tiene il primo. Restituisce quanti
    /// punti in più ci sono dopo l'unione.
    pub fn merge(&mut self, incoming: Vec<HistoryPoint>) -> usize {
        if incoming.is_empty() {
            return 0;
        }
        let before = self.points.len();
        let mut all = std::mem::take(&mut self.points);
        all.extend(incoming);
        all.sort_by_key(|p| p.t); // stabile: a parità di tempo vince il locale
        let mut out: Vec<HistoryPoint> = Vec::with_capacity(all.len());
        for p in all {
            match out.last() {
                Some(last) if p.t - last.t < 150 => {}
                _ => out.push(p),
            }
        }
        if out.len() > MAX_POINTS {
            let drop = out.len() - MAX_POINTS;
            out.drain(0..drop);
        }
        self.points = out;
        self.points.len().saturating_sub(before)
    }

    /// Punti delle ultime `hours` ore.
    pub fn window(&self, since: i64) -> &[HistoryPoint] {
        let start = self.points.partition_point(|p| p.t < since);
        &self.points[start..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t: i64, flux: f64) -> HistoryPoint {
        HistoryPoint {
            t,
            flux2: Some(flux),
            kp: None,
            dst: None,
            r_mp: None,
            l_peak: None,
            l_outer: None,
            content: None,
        }
    }

    #[test]
    fn merge_fills_gaps_without_duplicates() {
        let mut h = History::default();
        h.push(at(1000, 1.0));
        h.push(at(1300, 2.0));
        h.push(at(5000, 3.0));
        // remoto: un doppione esatto, uno ravvicinato, tre nel buco, uno prima
        let added = h.merge(vec![
            at(400, 9.0),
            at(1000, 9.0),
            at(1360, 9.0),
            at(1600, 9.0),
            at(1900, 9.0),
            at(2200, 9.0),
        ]);
        assert_eq!(added, 4);
        let ts: Vec<i64> = h.points.iter().map(|p| p.t).collect();
        assert_eq!(ts, vec![400, 1000, 1300, 1600, 1900, 2200, 5000]);
        // a parità di tempo resta il punto locale
        assert_eq!(h.points[1].flux2, Some(1.0));
        // ripetere la stessa unione non cambia nulla
        assert_eq!(h.merge(vec![at(1600, 7.0), at(1900, 7.0)]), 0);
        assert_eq!(h.points.len(), 7);
        assert_eq!(h.merge(Vec::new()), 0);
    }

    #[test]
    fn push_replaces_within_a_minute() {
        let mut h = History::default();
        h.push(HistoryPoint {
            t: 1000,
            flux2: Some(1.0),
            kp: None,
            dst: None,
            r_mp: None,
            l_peak: None,
            l_outer: None,
            content: None,
        });
        h.push(HistoryPoint {
            t: 1030,
            flux2: Some(2.0),
            kp: None,
            dst: None,
            r_mp: None,
            l_peak: None,
            l_outer: None,
            content: None,
        });
        assert_eq!(h.points.len(), 1);
        assert_eq!(h.points[0].flux2, Some(2.0));
        h.push(HistoryPoint {
            t: 2000,
            flux2: Some(3.0),
            kp: None,
            dst: None,
            r_mp: None,
            l_peak: None,
            l_outer: None,
            content: None,
        });
        assert_eq!(h.points.len(), 2);
        assert_eq!(h.window(1500).len(), 1);
    }
}