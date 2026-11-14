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

    /// Punti delle ultime `hours` ore.
    pub fn window(&self, since: i64) -> &[HistoryPoint] {
        let start = self.points.partition_point(|p| p.t < since);
        &self.points[start..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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