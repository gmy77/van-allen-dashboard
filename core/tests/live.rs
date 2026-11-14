//! Test contro i servizi reali. Non girano per default:
//!
//! ```text
//! cargo test -p vanallen-core -- --ignored --nocapture live
//! ```
//!
//! Servono a scoprire in anticipo se NOAA o Kyoto cambiano formato, cosa che
//! renderebbe i parser silenziosamente vuoti.

use vanallen_core::physics::RE_KM;
use vanallen_core::time::fmt_stamp;
use vanallen_core::{refresh, Http};

#[test]
#[ignore = "richiede rete"]
fn live_snapshot() {
    let http = Http::new(30);
    let snap = refresh(&http, 30);
    println!("scaricato: {}", fmt_stamp(snap.fetched_at));
    for w in &snap.warnings {
        println!("ATTENZIONE {w}");
    }

    assert!(snap.warnings.len() <= 2, "troppe fonti rotte: {:?}", snap.warnings);
    assert!(snap.kp.at > 0, "Kp non letto");
    assert!(!snap.goes_e.integral_2mev.is_empty(), "serie >2 MeV vuota");
    assert!(snap.goes_e.satellite > 0, "satellite GOES non identificato");
    assert!(!snap.goes_e.spectrum.is_empty(), "spettro differenziale vuoto");
    assert!(snap.wind.now_speed().is_some(), "vento solare assente");
    assert!(snap.imf.now_bz().is_some(), "Bz assente");
    assert!(snap.dst.now().is_some(), "Dst assente");
    assert!(snap.scales.is_some(), "scale NOAA assenti");

    let a = snap.assess();
    let b = a.belt.expect("sezione delle fasce non calcolata");
    println!(
        "GOES-{} · >2 MeV = {:.0} pfu · picco {:.1} R_E ({:.0} km) · bordi {:.1}–{:.1} R_E · contenuto {:.1}",
        snap.goes_e.satellite,
        a.geo_flux.unwrap_or(0.0),
        b.l_peak,
        b.l_peak * RE_KM,
        b.l_inner_outer,
        b.l_outer_edge,
        b.content_index
    );
    println!(
        "Kp {:.1} · Dst {:.0} nT · {:.1} R_E magnetopausa · γ {} · regime {} · allerte {}",
        a.kp.unwrap_or(f64::NAN),
        a.dst.unwrap_or(f64::NAN),
        a.r_mp.unwrap_or(f64::NAN),
        a.gamma.map(|g| format!("{g:.2}")).unwrap_or_else(|| "—".into()),
        b.regime.label(),
        a.alerts.len()
    );

    // invarianti di sanità
    assert!(b.l_peak > 2.0 && b.l_peak < 7.0);
    assert!(b.l_inner_outer < b.l_peak);
    assert!(b.l_outer_edge > b.l_peak);
    assert!(a.geo_flux.unwrap() > 0.0);
}
