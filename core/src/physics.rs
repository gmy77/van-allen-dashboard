//! Fisica e indicatori derivati.
//!
//! Regola del progetto, per non confondere misura e modello:
//! * MISURA  = ciò che un sensore ha effettivamente registrato (GOES, L1, magnetometri, Kp, Dst);
//! * MODELLO = ciò che qui calcoliamo o assumiamo (posizione delle fasce, plasmapausa, magnetopausa).
//! Tutto ciò che è modello è etichettato come tale nell'interfaccia.

use crate::sources::SpectrumPoint;

/// Raggio terrestre equatoriale, km.
pub const RE_KM: f64 = 6371.2;
/// L-shell di un satellite geostazionario.
pub const GEO_L: f64 = 6.6;

/// Pressione dinamica del vento solare in nPa (m_p n V²).
pub fn dynamic_pressure_npa(density_cm3: f64, speed_kms: f64) -> f64 {
    1.672_621_9e-6 * density_cm3 * speed_kms * speed_kms
}

/// Distanza del punto di stagnazione della magnetopausa in R_E (Shue et al. 1998).
pub fn magnetopause_standoff(bz_nt: f64, pdyn_npa: f64) -> f64 {
    let pdyn = pdyn_npa.max(0.3);
    let f = 10.22 + 1.29 * (0.184 * (bz_nt + 8.14)).tanh();
    (f * pdyn.powf(-1.0 / 6.6)).clamp(4.0, 14.0)
}

/// L della plasmapausa (Carpenter & Anderson 1992): L_pp = 5.6 − 0.46·Kp,
/// limitata a 4.9 R_E nelle condizioni più quiete (la relazione è tarata su Kp ≥ 1).
pub fn plasmapause_l(kp: f64) -> f64 {
    (5.6 - 0.46 * kp).clamp(1.6, 4.9)
}

/// Scala G (tempesta geomagnetica) da Kp.
pub fn g_scale(kp: f64) -> u8 {
    match kp {
        k if k >= 9.0 => 5,
        k if k >= 8.0 => 4,
        k if k >= 7.0 => 3,
        k if k >= 6.0 => 2,
        k if k >= 5.0 => 1,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StormLevel {
    #[default]
    Quiet,
    Unsettled,
    Weak,
    Moderate,
    Strong,
    Severe,
}

impl StormLevel {
    pub fn from_dst(dst: f64) -> Self {
        if dst <= -200.0 {
            Self::Severe
        } else if dst <= -100.0 {
            Self::Strong
        } else if dst <= -50.0 {
            Self::Moderate
        } else if dst <= -30.0 {
            Self::Weak
        } else if dst <= -20.0 {
            Self::Unsettled
        } else {
            Self::Quiet
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Quiet => "quieto",
            Self::Unsettled => "perturbato",
            Self::Weak => "tempesta debole",
            Self::Moderate => "tempesta moderata",
            Self::Strong => "tempesta forte",
            Self::Severe => "tempesta severa",
        }
    }
}

/// Indice spettrale γ: pendenza di log10(j) rispetto a log10(E) fra 0.2 e 3 MeV.
/// Valori tipici delle fasce: −1 (duro) … −4 (molle).
pub fn spectral_index(spectrum: &[SpectrumPoint]) -> Option<f64> {
    let pts: Vec<(f64, f64)> = spectrum
        .iter()
        .filter(|p| (0.15..=3.2).contains(&p.energy_mev) && p.j > 0.0)
        .map(|p| (p.energy_mev.ln(), p.j.ln()))
        .collect();
    if pts.len() < 3 {
        return None;
    }
    let n = pts.len() as f64;
    let sx: f64 = pts.iter().map(|(x, _)| x).sum();
    let sy: f64 = pts.iter().map(|(_, y)| y).sum();
    let sxx: f64 = pts.iter().map(|(x, _)| x * x).sum();
    let sxy: f64 = pts.iter().map(|(x, y)| x * y).sum();
    let den = n * sxx - sx * sx;
    if den.abs() < 1e-12 {
        return None;
    }
    Some((n * sxy - sx * sy) / den)
}

/// Flusso di energia integrato sullo spettro differenziale, µW/m² (4π sr).
/// Serve come misura di "quanta energia portano gli elettroni" a L=6.6.
pub fn energy_flux_uw_m2(spectrum: &[SpectrumPoint]) -> Option<f64> {
    let mut pts: Vec<&SpectrumPoint> = spectrum.iter().filter(|p| p.j > 0.0).collect();
    if pts.len() < 2 {
        return None;
    }
    pts.sort_by(|a, b| a.energy_mev.partial_cmp(&b.energy_mev).unwrap());
    // integrazione trapezoidale di j·E su E: MeV/(cm² s sr)
    let mut acc = 0.0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let de = b.energy_mev - a.energy_mev;
        let ja = a.j * a.energy_mev;
        let jb = b.j * b.energy_mev;
        acc += 0.5 * (ja + jb) * de;
    }
    // 1 MeV/(cm² s sr) = 1.602e-9 W/(m² sr) -> ×4π -> W/m² -> ×1e6 -> µW/m²
    Some(acc * 1.602e-9 * 4.0 * std::f64::consts::PI * 1e6)
}

/// Variazione relativa del flusso sull'ultima `hours` ora (log10).
pub fn log10_rate(series: &[crate::sources::Sample], hours: f64) -> Option<f64> {
    let last = *series.last()?;
    let t0 = last.t - (hours * 3600.0) as i64;
    let prev = series.iter().rev().find(|s| s.t <= t0)?.v;
    if prev <= 0.0 || last.v <= 0.0 {
        return None;
    }
    Some(last.v.log10() - prev.log10())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeltRegime {
    Quiet,
    Disturbed,
    StormInjection,
    SlotEroded,
    Dropout,
}

impl BeltRegime {
    pub fn label(self) -> &'static str {
        match self {
            Self::Quiet => "fasce stabili",
            Self::Disturbed => "fasce perturbate",
            Self::StormInjection => "iniezione di elettroni",
            Self::SlotEroded => "slot eroso",
            Self::Dropout => "dropout (fasce svuotate)",
        }
    }
}

/// Soglia di flusso usata per definire "dove arriva la fascia" (pfu, >2 MeV).
/// 100 pfu è il livello sotto il quale il flusso relativistico è di fondo.
pub const BELT_EDGE_PFU: f64 = 100.0;

/// Sezione radiale delle fasce esterne, ancorata alla misura a L=6.6.
#[derive(Clone, Debug)]
pub struct BeltShape {
    /// plasmapausa stimata (modello)
    pub l_pp: f64,
    /// L del massimo della fascia esterna (modello)
    pub l_peak: f64,
    /// larghezza gaussiana in L (modello)
    pub width: f64,
    /// flusso al massimo, pfu (modello ancorato alla misura)
    pub amp: f64,
    /// flusso misurato a L=6.6, pfu
    pub geo_flux: f64,
    /// bordo interno della fascia esterna (modello)
    pub l_inner_outer: f64,
    /// bordo esterno: dove il flusso scende sotto BELT_EDGE_PFU (modello)
    pub l_outer_edge: f64,
    /// (L, log10 pfu) da L=1.0 a L=8.0
    pub profile: Vec<(f64, f64)>,
    /// indice di contenuto: Σ J(L)·L²·ΔL, unità arbitrarie ma monotone
    pub content_index: f64,
    pub regime: BeltRegime,
    /// true se la magnetopausa è dentro il bordo esterno della fascia
    pub truncated_by_magnetopause: bool,
}

impl BeltShape {
    pub fn peak_flux(&self) -> f64 {
        self.amp
    }

    pub fn outer_edge_km(&self) -> f64 {
        self.l_outer_edge * RE_KM
    }

    pub fn peak_km(&self) -> f64 {
        self.l_peak * RE_KM
    }

    /// Ampiezza radiale della fascia (R_E): la "dimensione" da confrontare nel tempo.
    pub fn size_index(&self) -> f64 {
        self.l_outer_edge - self.l_inner_outer
    }
}

/// Costruisce la sezione delle fasce a partire dal flusso misurato a L=6.6.
///
/// Modello (dichiaratamente tale, non una misura):
/// * `J(L) = A·exp(−((L−L_pk)/w)²)·g(L)`, con `A` fissato perché `J(6.6)` coincida
///   con il flusso misurato da GOES;
/// * `g(L)` è una rampa che azzera il flusso dentro la plasmapausa: è ciò che crea
///   lo slot region, il "vuoto" fra fascia interna ed esterna;
/// * `L_pk` si sposta verso l'interno con Kp e con il Dst, `w` cresce con Kp.
///
/// I bordi sono dove il flusso modellato scende sotto [`BELT_EDGE_PFU`], quindi la
/// fascia diventa più grande quando il flusso cresce.
pub fn belt_shape(geo_flux: f64, kp: f64, dst: f64, r_mp: Option<f64>) -> BeltShape {
    let l_pp = plasmapause_l(kp);
    let geo_flux = geo_flux.max(1e-3);

    // L del picco: 5.4 quieto, si sposta a ~3.6 con Kp 8
    let mut l_peak = 5.40 - 0.22 * kp;
    if dst < -50.0 {
        l_peak -= 0.06 * (-dst - 50.0).min(150.0) / 50.0;
    }
    let l_peak = l_peak.clamp(3.0, 5.9).max(l_pp + 0.5);

    // rapporto picco/geostazionario voluto (~1.6 quieto, ~9 con Kp 8):
    // serve solo a scegliere la larghezza del profilo.
    let ratio = 1.6 + 0.9 * kp;
    let width = ((GEO_L - l_peak) / (2.0 * ratio.ln()).sqrt()).clamp(0.55, 2.2);

    let shape_at_geo = (-(((GEO_L - l_peak) / width).powi(2))).exp().max(1e-4);
    let amp = (geo_flux / shape_at_geo).min(geo_flux * 60.0);

    // taglio netto dentro la plasmapausa: è ciò che crea lo slot region
    let gate_at = l_pp + 0.35;
    let gate = move |l: f64| {
        if l >= gate_at {
            1.0
        } else {
            (-(((gate_at - l) / 0.10).powi(2))).exp()
        }
    };

    let mut profile = Vec::with_capacity(141);
    let mut content = 0.0;
    let mut best_l = l_peak;
    let mut best_j = 0.0;
    let mut l = 1.0;
    while l <= 8.0001 {
        let z = (l - l_peak) / width;
        let j = amp * (-z * z).exp() * gate(l);
        if j > best_j {
            best_j = j;
            best_l = l;
        }
        profile.push((l, j.max(1e-9).log10()));
        content += j * l * l * 0.05;
        l += 0.05;
    }

    // bordi: prima e ultima cella sopra la soglia
    let mut inner: Option<f64> = None;
    let mut outer: Option<f64> = None;
    for (ll, lj) in profile.iter() {
        if 10f64.powf(*lj) >= BELT_EDGE_PFU {
            if inner.is_none() {
                inner = Some(*ll);
            }
            outer = Some(*ll);
        }
    }
    let cap = r_mp.map(|r| r.min(9.5)).unwrap_or(9.5);
    let l_inner_outer = inner.unwrap_or(best_l - 0.3).max(2.0).min(best_l - 0.2);
    let l_outer_edge = outer.unwrap_or(best_l + 0.3).min(cap);

    let r_mp_value = r_mp.unwrap_or(10.0);
    let truncated = l_outer_edge > r_mp_value - 0.05;

    let slot_eroded = l_inner_outer <= 3.0;
    let regime = if r_mp_value <= GEO_L - 0.2 {
        BeltRegime::Dropout
    } else if kp >= 5.0 || dst <= -50.0 {
        BeltRegime::StormInjection
    } else if slot_eroded {
        BeltRegime::SlotEroded
    } else if kp >= 4.0 || dst <= -30.0 {
        BeltRegime::Disturbed
    } else {
        BeltRegime::Quiet
    };

    BeltShape {
        l_pp,
        l_peak: best_l,
        width,
        amp: best_j,
        geo_flux,
        l_inner_outer,
        l_outer_edge,
        profile,
        content_index: content / 1e3,
        regime,
        truncated_by_magnetopause: truncated,
    }
}

/// Fascia interna di protoni: forma climatologica, nessuna misura live disponibile
/// (GOES orbita a L=6.6, non attraversa la fascia interna). Valori 0..1.
pub fn inner_belt_density(l: f64) -> f64 {
    // doppio picco: L≈1.5 (protoni) e una spalla a L≈2.0
    let a = (-(((l - 1.52) / 0.30).powi(2))).exp();
    let b = 0.35 * (-(((l - 2.05) / 0.28).powi(2))).exp();
    (a + b).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::Sample;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn pressure() {
        // n=5 cm-3, V=400 km/s -> ~1.34 nPa
        assert!(approx(dynamic_pressure_npa(5.0, 400.0), 1.338, 0.01));
    }

    #[test]
    fn standoff_quiet() {
        // Bz=0, Pdyn=1.34 nPa -> ~10 R_E
        let r = magnetopause_standoff(0.0, 1.34);
        assert!(r > 9.0 && r < 11.0, "r={r}");
        // Bz molto negativo e pressione alta -> più vicino
        let r2 = magnetopause_standoff(-15.0, 8.0);
        assert!(r2 < r && r2 > 4.0, "r2={r2}");
    }

    #[test]
    fn plasmapause() {
        assert!(approx(plasmapause_l(0.0), 4.9, 1e-9));
        assert!(approx(plasmapause_l(5.0), 3.3, 1e-9));
        assert!(plasmapause_l(20.0) >= 1.6);
    }

    #[test]
    fn g_scales() {
        assert_eq!(g_scale(0.0), 0);
        assert_eq!(g_scale(4.9), 0);
        assert_eq!(g_scale(5.0), 1);
        assert_eq!(g_scale(9.0), 5);
        assert_eq!(StormLevel::from_dst(-120.0), StormLevel::Strong);
        assert_eq!(StormLevel::from_dst(5.0), StormLevel::Quiet);
    }

    #[test]
    fn spectrum_anchoring() {
        let geo = 4000.0;
        let b = belt_shape(geo, 0.0, 0.0, Some(10.0));
        // il profilo deve passare per il valore misurato a L=6.6
        let at_geo = b
            .profile
            .iter()
            .find(|(l, _)| (*l - 6.6).abs() < 0.03)
            .map(|(_, v)| 10f64.powf(*v))
            .unwrap();
        assert!((at_geo - geo).abs() / geo < 0.30, "at_geo={at_geo}");
        assert!(b.l_peak < GEO_L);
        assert!(b.l_outer_edge > b.l_peak);
        assert!(b.l_inner_outer < b.l_peak);
        assert!(b.content_index > 0.0);
    }

    #[test]
    fn storm_shifts_peak_inward() {
        let quiet = belt_shape(1000.0, 1.0, 0.0, Some(10.0));
        let storm = belt_shape(1000.0, 8.0, -120.0, Some(6.8));
        assert!(storm.l_peak < quiet.l_peak, "{:?} vs {:?}", storm.l_peak, quiet.l_peak);
        assert!(storm.l_pp < quiet.l_pp);
        assert_eq!(storm.regime, BeltRegime::StormInjection);
    }

    #[test]
    fn stronger_flux_means_bigger_belt() {
        let weak = belt_shape(50.0, 2.0, -5.0, Some(10.0));
        let strong = belt_shape(50_000.0, 2.0, -5.0, Some(10.0));
        assert!(
            strong.size_index() > weak.size_index(),
            "{} vs {}",
            strong.size_index(),
            weak.size_index()
        );
    }

    #[test]
    fn dropout_when_magnetopause_inside_geo() {
        let b = belt_shape(100.0, 8.0, -300.0, Some(6.0));
        assert_eq!(b.regime, BeltRegime::Dropout);
    }

    #[test]
    fn quiet_belt_has_a_slot() {
        // condizioni quiete: fra plasmapausa e fascia esterna c'è un vuoto
        let b = belt_shape(5000.0, 1.0, -10.0, Some(10.0));
        assert!(b.l_inner_outer > b.l_pp, "inner={} pp={}", b.l_inner_outer, b.l_pp);
        assert!(b.profile.iter().any(|(l, j)| (*l - 3.6).abs() < 0.3 && *j < 2.0));
    }

    #[test]
    fn rate() {
        let s: Vec<Sample> = vec![
            Sample { t: 0, v: 100.0 },
            Sample { t: 3600, v: 1000.0 },
        ];
        let r = log10_rate(&s, 1.0).unwrap();
        assert!(approx(r, 1.0, 0.01));
        assert!(log10_rate(&s, 5.0).is_none());
    }

    #[test]
    fn energy_flux_order_of_magnitude() {
        let spec = vec![
            SpectrumPoint { energy_mev: 0.5, j: 1e4 },
            SpectrumPoint { energy_mev: 1.0, j: 1e4 },
            SpectrumPoint { energy_mev: 2.0, j: 1e3 },
        ];
        let f = energy_flux_uw_m2(&spec).unwrap();
        assert!(f > 10.0 && f < 1e4, "f={f}");
    }

    #[test]
    fn inner_belt_shape() {
        assert!(inner_belt_density(1.52) > 0.95);
        assert!(inner_belt_density(4.0) < 0.01);
    }

    #[test]
    fn spectral_index_sign() {
        // spettro che decade: γ negativo
        let spec = vec![
            SpectrumPoint { energy_mev: 0.3, j: 1e4 },
            SpectrumPoint { energy_mev: 1.0, j: 1e3 },
            SpectrumPoint { energy_mev: 2.5, j: 1e2 },
        ];
        let g = spectral_index(&spec).unwrap();
        assert!(g < -0.5, "g={g}");
    }
}