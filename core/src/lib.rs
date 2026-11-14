//! # vanallen-core
//!
//! Dati live e fisica delle fasce di Van Allen, indipendenti dall'interfaccia.
//!
//! Fonti (tutte pubbliche, senza chiave):
//! * **NOAA SWPC** — flussi GOES-19 (elettroni 79 keV … 2.9 MeV, protoni, magnetometro),
//!   vento solare e campo interplanetario al punto L1 (RTSW), Kp al minuto, scale R/S/G;
//! * **WDC Kyoto / SWPC** — Dst orario ufficiale e nowcast Dst al minuto.
//!
//! Le due categorie di numeri sono tenute separate anche nell'interfaccia:
//! **misura** (ciò che i sensori hanno registrato) e **modello** (ciò che calcoliamo
//! o assumiamo: posizione delle fasce, plasmapausa, magnetopausa).

pub mod history;
pub mod http;
pub mod physics;
pub mod remote;
pub mod snapshot;
pub mod sources;
pub mod time;

pub use history::{History, HistoryPoint};
pub use http::Http;
pub use physics::{BeltRegime, BeltShape, StormLevel};
pub use snapshot::{refresh, Alert, Assessment, Severity, Snapshot};
pub use sources::{Imf, KpNow, Scales, SolarWind, SpectrumPoint};

/// Versione del crate, per l'intestazione dell'app.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");