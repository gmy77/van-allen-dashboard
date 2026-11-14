//! Tema visivo in stile WinUI 3 (Fluent, variante scura).
//!
//! Tutti i colori sono presi dalla palette Fluent di WinUI 3: sfondo #202020,
//! superfici #272727, bordo #3D3D3D, accent #4CC2FF, testo bianco e tre livelli
//! di testo secondario. Il carattere è Segoe UI (presente in ogni Windows),
//! con la variante Semibold per i titoli, come fa WinUI.

use eframe::egui::{
    self, Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, RichText,
    Stroke, Visuals,
};
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(0x20, 0x20, 0x20);
pub const CARD: Color32 = Color32::from_rgb(0x27, 0x27, 0x27);
pub const CARD_ALT: Color32 = Color32::from_rgb(0x2D, 0x2D, 0x2D);
pub const BORDER: Color32 = Color32::from_rgb(0x3D, 0x3D, 0x3D);
pub const GRID: Color32 = Color32::from_rgb(0x35, 0x35, 0x35);
pub const TEXT: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const SUB: Color32 = Color32::from_rgb(0xC8, 0xC8, 0xC8);
pub const FAINT: Color32 = Color32::from_rgb(0x8A, 0x8A, 0x8A);
pub const ACCENT: Color32 = Color32::from_rgb(0x4C, 0xC2, 0xFF);
pub const GOOD: Color32 = Color32::from_rgb(0x6C, 0xCB, 0x5F);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0xD6, 0x5C);
pub const BAD: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x6B);
pub const SEVERE: Color32 = Color32::from_rgb(0xC5, 0x86, 0xFF);
pub const FLUX: Color32 = Color32::from_rgb(0x56, 0xD9, 0xC9);
pub const MAG: Color32 = Color32::from_rgb(0xFF, 0x9A, 0x62);
pub const PROTON: Color32 = Color32::from_rgb(0xD9, 0x8B, 0xE8);

/// Famiglia per i titoli (Segoe UI Semibold), con ripiego sulla normale.
#[derive(Clone)]
pub struct Fonts {
    pub semi: FontFamily,
}

pub fn install(ctx: &Context) -> Fonts {
    let dir = std::path::Path::new("C:\\Windows\\Fonts");
    let read = |name: &str| std::fs::read(dir.join(name)).ok();

    let mut fonts = FontDefinitions::default();
    let mut semi = FontFamily::Proportional;

    if let Some(bytes) = read("segoeui.ttf") {
        fonts
            .font_data
            .insert("segoe".to_owned(), Arc::new(FontData::from_owned(bytes)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "segoe".to_owned());
    }
    // Segoe UI non ha i simboli matematici (⊕ di R⊕, ⟳): senza un ripiego
    // escono come quadratini. Segoe UI Symbol li copre tutti.
    let has_symbols = match read("seguisym.ttf") {
        Some(bytes) => {
            fonts
                .font_data
                .insert("segoe-symbol".to_owned(), Arc::new(FontData::from_owned(bytes)));
            let list = fonts.families.entry(FontFamily::Proportional).or_default();
            let at = list.iter().position(|n| n == "segoe").map_or(0, |i| i + 1);
            list.insert(at, "segoe-symbol".to_owned());
            true
        }
        None => false,
    };
    if let Some(bytes) = read("seguisb.ttf") {
        fonts.font_data.insert(
            "segoe-semi".to_owned(),
            Arc::new(FontData::from_owned(bytes)),
        );
        let name = FontFamily::Name("semibold".into());
        // dopo il semibold, gli stessi ripieghi della famiglia normale
        let mut list = vec!["segoe-semi".to_owned()];
        if has_symbols {
            list.push("segoe-symbol".to_owned());
        }
        if let Some(rest) = fonts.families.get(&FontFamily::Proportional) {
            for n in rest {
                if !list.contains(n) {
                    list.push(n.clone());
                }
            }
        }
        fonts.families.insert(name.clone(), list);
        semi = name;
    }

    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.visuals = Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = CARD;
    style.visuals.extreme_bg_color = Color32::from_rgb(0x1A, 0x1A, 0x1A);
    style.visuals.faint_bg_color = CARD_ALT;
    style.visuals.override_text_color = Some(SUB);
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.widgets.noninteractive.bg_stroke = stroke(1.0, BORDER);
    style.visuals.widgets.inactive.bg_fill = CARD_ALT;
    style.visuals.widgets.inactive.bg_stroke = stroke(1.0, BORDER);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x33, 0x33, 0x33);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(0x3A, 0x3A, 0x3A);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(6);
    style.visuals.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    style.visuals.selection.stroke = stroke(1.0, ACCENT);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    ctx.set_style(style);

    Fonts { semi }
}

impl Fonts {
    /// Testo normale.
    pub fn ui(&self, size: f32) -> FontId {
        FontId::new(size, FontFamily::Proportional)
    }

    /// Titoli e numeri in evidenza.
    pub fn heading(&self, size: f32) -> FontId {
        FontId::new(size, self.semi.clone())
    }

    pub fn title(&self, s: impl Into<String>) -> RichText {
        RichText::new(s).size(24.0).color(TEXT).family(self.semi.clone())
    }

    pub fn body(&self, s: impl Into<String>) -> RichText {
        RichText::new(s).size(11.5).color(SUB)
    }

    pub fn hint(&self, s: impl Into<String>) -> RichText {
        RichText::new(s).size(10.5).color(FAINT)
    }
}

/// Pennello di contorno. Serve a tener fissa la larghezza a `f32` (senza,
/// i letterali `1.0` verrebbero inferiti come `f64`).
#[inline]
pub fn stroke(width: f32, color: Color32) -> Stroke {
    Stroke::new(width, color)
}

/// Scala di colore del flusso: da blu (basso) a rosso (alto) su log10 pfu.
pub fn flux_color(log10_flux: f64) -> Color32 {
    let t = ((log10_flux + 0.5) / 5.5).clamp(0.0, 1.0) as f32;
    let stops: [(f32, u8, u8, u8); 6] = [
        (0.00, 0x1B, 0x33, 0x66),
        (0.22, 0x1D, 0x7A, 0xC8),
        (0.42, 0x24, 0xC6, 0xA8),
        (0.60, 0x7C, 0xD9, 0x55),
        (0.78, 0xF2, 0xC0, 0x3A),
        (1.00, 0xF0, 0x4B, 0x3A),
    ];
    for w in stops.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t <= b.0 {
            let f = if (b.0 - a.0).abs() < 1e-6 {
                0.0
            } else {
                (t - a.0) / (b.0 - a.0)
            };
            let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f).round() as u8;
            return Color32::from_rgb(mix(a.1, b.1), mix(a.2, b.2), mix(a.3, b.3));
        }
    }
    Color32::from_rgb(0xF0, 0x4B, 0x3A)
}
