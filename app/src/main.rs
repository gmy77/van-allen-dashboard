//! Dashboard delle fasce di Van Allen.
// In release niente console: l'app è una finestra e basta.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//!
//! Interfaccia in egui con tema WinUI 3 ricostruito (colori Fluent, Segoe UI,
//! card con angoli 8 e bordo 1 px).
//!
//! Flusso: un thread di fondo scarica le fonti NOAA/Kyoto ogni 5 minuti e le
//! manda con un canale; l'interfaccia preleva l'ultima fotografia e ridisegna.
//! La UI non aspetta mai la rete.
//!
//! Variabili d'ambiente (tutte facoltative):
//! * `VANALLEN_PORT`    porta del server locale (predefinita 8787);
//! * `VANALLEN_REMOTE`  indirizzo del Worker dello storico (vedi `worker/`);
//! * `VANALLEN_PRELOAD` scarica i dati prima di aprire la finestra;
//! * `VANALLEN_TAB`     scheda iniziale: fasce, vento, magnetosfera, storico, sorgenti;
//! * `VANALLEN_HELP`    apre subito l'aiuto numero N (ordine di `Topic::ALL`);
//! * `VANALLEN_SHOT`    salva un PNG della finestra nel percorso dato ed esce.

mod charts;
mod help;
mod pages;
mod plots;
mod server;
mod theme;

use eframe::egui::{
    self, pos2, vec2, Align, Align2, CentralPanel, Color32, CornerRadius, Label, Layout, Rect,
    RichText, Sense, StrokeKind, Ui, UiBuilder,
};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;
use help::Topic;
use theme::Fonts;
use vanallen_core::physics::{GEO_L, RE_KM};
use vanallen_core::time::{fmt_short, fmt_stamp};
use vanallen_core::{
    refresh, Alert, Assessment, History, HistoryPoint, Http, Severity, Snapshot, StormLevel,
};

const REFRESH_SECS: u64 = 300;

/// Schede della finestra.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Belts,
    Wind,
    Magneto,
    History,
    Sources,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Self::Belts,
        Self::Wind,
        Self::Magneto,
        Self::History,
        Self::Sources,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Belts => "Fasce",
            Self::Wind => "Vento solare",
            Self::Magneto => "Magnetosfera",
            Self::History => "Storico",
            Self::Sources => "Sorgenti",
        }
    }

    fn from_name(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();
        Self::ALL
            .into_iter()
            .find(|t| t.label().to_lowercase().starts_with(&s) && !s.is_empty())
    }
}

/// Storico remoto: il Worker che registra anche ad app chiusa.
pub(crate) struct Remote {
    /// indirizzo configurato, se c'è
    pub url: Option<String>,
    rx: Option<Receiver<Result<Vec<HistoryPoint>, String>>>,
    /// punti nuovi aggiunti allo storico in questa sessione
    pub added: usize,
    /// ultimo scaricamento riuscito (epoch) e numero di punti ricevuti
    pub last_ok: Option<(i64, usize)>,
    pub error: Option<String>,
}

impl Remote {
    /// Legge l'indirizzo da `VANALLEN_REMOTE` o da `remote.txt` accanto allo
    /// storico; se c'è, avvia il thread che scarica.
    fn start() -> Self {
        let url = std::env::var("VANALLEN_REMOTE")
            .ok()
            .or_else(|| std::fs::read_to_string(History::remote_file()).ok())
            .and_then(|raw| vanallen_core::remote::clean_url(&raw));
        let rx = url.clone().map(spawn_remote);
        Self {
            url,
            rx,
            added: 0,
            last_ok: None,
            error: None,
        }
    }
}

/// Scarica lo storico remoto: 30 giorni all'avvio, poi le ultime 3 ore ogni
/// mezz'ora (i doppioni li scarta `History::merge`).
fn spawn_remote(url: String) -> Receiver<Result<Vec<HistoryPoint>, String>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let http = Http::new(40);
        let mut window = 30 * 86_400;
        loop {
            let since = vanallen_core::time::now_unix() - window;
            let result = http
                .json(&format!("{url}/history?since={since}"))
                .and_then(|v| vanallen_core::remote::parse_history(&v))
                .map(|m| m.iter().map(|x| x.to_point()).collect::<Vec<_>>());
            if result.is_ok() {
                window = 3 * 3600;
            }
            if tx.send(result).is_err() {
                return; // finestra chiusa
            }
            std::thread::sleep(Duration::from_secs(1800));
        }
    });
    rx
}

/// Screenshot automatico (variabile `VANALLEN_SHOT`).
struct Shot {
    path: std::path::PathBuf,
    frames: u32,
    asked: bool,
}

// ---- stato --------------------------------------------------------------

struct VanAllenApp {
    rx: Receiver<Snapshot>,
    trigger: Sender<()>,
    snapshot: Option<Snapshot>,
    assessment: Option<Assessment>,
    history: History,
    fonts: Fonts,
    frame: u64,
    /// Server HTTP locale (porta, contenitore dei contenuti da servire).
    server: Option<(u16, server::Shared)>,
    /// Riquadro di cui mostrare la spiegazione.
    help: Option<Topic>,
    /// Aiuto mostrato al fotogramma precedente: serve a non scambiare il clic
    /// che apre il pannello per un clic "fuori dal pannello".
    help_prev: Option<Topic>,
    tab: Tab,
    hist_range: pages::HistRange,
    shot: Option<Shot>,
    remote: Remote,
}

impl VanAllenApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let fonts = theme::install(&cc.egui_ctx);
        let (rx, trigger) = spawn_worker();
        let history = History::load();

        let port = std::env::var("VANALLEN_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(server::DEFAULT_PORT);
        let srv = server::start(port);
        if let Some((p, _)) = &srv {
            println!("server locale: http://127.0.0.1:{p}");
        }

        let mut app = Self {
            rx,
            trigger,
            snapshot: None,
            assessment: None,
            history,
            fonts,
            frame: 0,
            server: srv,
            help: std::env::var("VANALLEN_HELP")
                .ok()
                .and_then(|n| n.trim().parse::<usize>().ok())
                .and_then(|n| Topic::ALL.get(n).copied()),
            help_prev: None,
            tab: std::env::var("VANALLEN_TAB")
                .ok()
                .and_then(|n| Tab::from_name(&n))
                .unwrap_or(Tab::Belts),
            hist_range: pages::HistRange::All,
            remote: Remote::start(),
            shot: std::env::var_os("VANALLEN_SHOT").map(|p| Shot {
                path: p.into(),
                frames: 0,
                asked: false,
            }),
        };

        // Con VANALLEN_PRELOAD impostata il primo frame ha già i dati: utile per
        // gli screenshot automatici, che eframe scatta al secondo passaggio.
        if std::env::var_os("VANALLEN_PRELOAD").is_some() || app.shot.is_some() {
            let http = Http::new(30);
            let snap = refresh(&http, 30);
            app.ingest(snap);
        }
        app
    }

    fn ingest(&mut self, snap: Snapshot) {
        let a = snap.assess();
        self.history.push(HistoryPoint {
            t: snap.fetched_at,
            flux2: a.geo_flux,
            kp: a.kp,
            dst: a.dst,
            r_mp: a.r_mp,
            l_peak: a.belt.as_ref().map(|b| b.l_peak),
            l_outer: a.belt.as_ref().map(|b| b.l_outer_edge),
            content: a.belt.as_ref().map(|b| b.content_index),
        });
        if let Err(e) = self.history.save() {
            eprintln!("storico non salvato: {e}");
        }
        // il server locale serve sempre l'ultima fotografia
        if let Some((_, shared)) = &self.server {
            if let Ok(mut slot) = shared.lock() {
                *slot = server::payload(&snap, &a);
            }
        }
        self.assessment = Some(a);
        self.snapshot = Some(snap);
        self.frame += 1;
    }

    fn absorb(&mut self) {
        let mut latest = None;
        while let Ok(snap) = self.rx.try_recv() {
            latest = Some(snap);
        }
        if let Some(snap) = latest {
            self.ingest(snap);
        }

        // storico remoto: unisce i punti arrivati e salva
        let arrived: Vec<_> = match &self.remote.rx {
            Some(rx) => std::iter::from_fn(|| rx.try_recv().ok()).collect(),
            None => Vec::new(),
        };
        for result in arrived {
            match result {
                Ok(points) => {
                    let n = points.len();
                    self.remote.added += self.history.merge(points);
                    self.remote.last_ok = Some((vanallen_core::time::now_unix(), n));
                    self.remote.error = None;
                    if let Err(e) = self.history.save() {
                        eprintln!("storico non salvato: {e}");
                    }
                }
                Err(e) => self.remote.error = Some(e),
            }
        }
    }
}

impl eframe::App for VanAllenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.absorb();

        // un ridisegno al secondo: il tempo trascorso va avanti anche senza dati nuovi
        ctx.request_repaint_after(Duration::from_secs(1));

        CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.page(ui));
            });

        self.help_modal(ctx);
        self.take_shot(ctx);
    }
}

impl VanAllenApp {
    /// Con `VANALLEN_SHOT`: aspetta qualche fotogramma, chiede lo screenshot,
    /// lo salva in PNG e chiude la finestra.
    fn take_shot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        shot.frames += 1;
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            match save_png(&shot.path, &img) {
                Ok(()) => println!("screenshot salvato: {}", shot.path.display()),
                Err(e) => eprintln!("screenshot non salvato: {e}"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if !shot.asked && shot.frames >= 5 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            shot.asked = true;
        }
        ctx.request_repaint();
    }
}

fn save_png(path: &std::path::Path, img: &egui::ColorImage) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        img.size[0] as u32,
        img.size[1] as u32,
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(img.as_raw()).map_err(|e| e.to_string())
}

// ---- rete ---------------------------------------------------------------

fn spawn_worker() -> (Receiver<Snapshot>, Sender<()>) {
    let (tx, rx) = channel::<Snapshot>();
    let (trigger_tx, trigger_rx) = channel::<()>();
    std::thread::spawn(move || {
        let http = Http::new(25);
        loop {
            let snap = refresh(&http, 25);
            if tx.send(snap).is_err() {
                return; // finestra chiusa
            }
            match trigger_rx.recv_timeout(Duration::from_secs(REFRESH_SECS)) {
                Ok(()) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
            while trigger_rx.try_recv().is_ok() {}
        }
    });
    (rx, trigger_tx)
}

// ---- componenti ---------------------------------------------------------

fn sev_color(s: Severity) -> Color32 {
    match s {
        Severity::Info => theme::ACCENT,
        Severity::Watch => theme::WARN,
        Severity::Warn => theme::MAG,
        Severity::Severe => theme::BAD,
    }
}

fn storm_color(l: StormLevel) -> Color32 {
    match l {
        StormLevel::Quiet => theme::GOOD,
        StormLevel::Unsettled => theme::FLUX,
        StormLevel::Weak => theme::WARN,
        StormLevel::Moderate => theme::MAG,
        StormLevel::Strong => theme::BAD,
        StormLevel::Severe => theme::SEVERE,
    }
}

/// Pallino "?" in alto a destra: restituisce true se è stato cliccato.
fn help_badge(ui: &mut Ui, rect: Rect, salt: &str, f: &Fonts) -> bool {
    let id = ui.id().with(("aiuto", salt));
    let resp = ui.interact(rect, id, Sense::click());
    let hot = resp.hovered();
    let p = ui.painter();

    let fill = if hot { theme::ACCENT.gamma_multiply(0.28) } else { theme::CARD_ALT };
    let ring = if hot { theme::ACCENT } else { theme::BORDER };
    p.circle_filled(rect.center(), rect.width() * 0.5, fill);
    p.circle_stroke(rect.center(), rect.width() * 0.5, theme::stroke(1.0, ring));
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        "?",
        f.heading(11.5),
        if hot { theme::TEXT } else { theme::SUB },
    );

    if hot {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Card con titolo, sottotitolo e un corpo che riceve l'area utile.
///
/// Restituisce il tema di aiuto se è stato premuto il "?".
fn card(
    ui: &mut Ui,
    w: f32,
    h: f32,
    title: &str,
    subtitle: &str,
    topic: Topic,
    f: &Fonts,
    body: impl FnOnce(&mut Ui, f32),
) -> Option<Topic> {
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(8),
        theme::CARD,
        theme::stroke(1.0, theme::BORDER),
        StrokeKind::Inside,
    );

    let p = ui.painter_at(rect);
    let mut y = rect.top() + 11.0;
    p.text(
        pos2(rect.left() + 14.0, y),
        Align2::LEFT_TOP,
        title,
        f.heading(13.0),
        theme::TEXT,
    );
    y += 19.0;
    if !subtitle.is_empty() {
        p.text(
            pos2(rect.left() + 14.0, y),
            Align2::LEFT_TOP,
            subtitle,
            f.ui(10.5),
            theme::FAINT,
        );
        y += 15.0;
    }

    let inner = egui::Rect::from_min_max(
        pos2(rect.left() + 4.0, y + 4.0),
        pos2(rect.right() - 4.0, rect.bottom() - 8.0),
    );
    if inner.width() > 20.0 {
        ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
            body(ui, inner.width());
        });
    }
    // Il corpo può essere più corto della card: senza questo il cursore del
    // layout resterebbe alla fine del corpo e l'elemento successivo salirebbe
    // sopra il bordo inferiore.
    ui.allocate_rect(Rect::from_min_max(rect.max, rect.max), Sense::hover());

    let badge = Rect::from_center_size(
        pos2(rect.right() - 20.0, rect.top() + 19.0),
        vec2(19.0, 19.0),
    );
    if help_badge(ui, badge, title, f) {
        return Some(topic);
    }
    None
}

fn kpi(
    ui: &mut Ui,
    w: f32,
    title: &str,
    value: &str,
    hint: &str,
    accent: Color32,
    topic: Topic,
    f: &Fonts,
) -> Option<Topic> {
    let (rect, _) = ui.allocate_exact_size(vec2(w, 76.0), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(8),
        theme::CARD,
        theme::stroke(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    let p = ui.painter_at(rect);
    p.text(
        pos2(rect.left() + 12.0, rect.top() + 9.0),
        Align2::LEFT_TOP,
        title,
        f.ui(10.5),
        theme::FAINT,
    );
    p.text(
        pos2(rect.left() + 12.0, rect.top() + 27.0),
        Align2::LEFT_TOP,
        value,
        f.heading(19.0),
        accent,
    );
    p.text(
        pos2(rect.left() + 12.0, rect.bottom() - 9.0),
        Align2::LEFT_BOTTOM,
        hint,
        f.ui(9.5),
        theme::FAINT,
    );

    let badge = Rect::from_center_size(
        pos2(rect.right() - 17.0, rect.top() + 16.0),
        vec2(17.0, 17.0),
    );
    if help_badge(ui, badge, title, f) {
        return Some(topic);
    }
    None
}

// ---- pagina -------------------------------------------------------------

impl VanAllenApp {
    fn page(&mut self, ui: &mut Ui) {
        let full = ui.available_width();

        // intestazione: rettangoli espliciti (sinistra: titolo, destra: stato + pulsante)
        let (row, _) = ui.allocate_exact_size(vec2(full, 60.0), Sense::hover());
        let split = (row.right() - 360.0).max(row.left() + 320.0);

        ui.scope_builder(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(row.min, pos2(split - 16.0, row.bottom())))
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ui.label(self.fonts.title("Fasce di Van Allen — monitoraggio live"));
                ui.add_space(1.0);
                let sub = match &self.snapshot {
                    Some(s) => format!(
                        "GOES-{} · vento solare L1 ({}) · Kp al minuto · Dst Kyoto · posizione delle fasce: modello ancorato alle misure",
                        s.goes_e.satellite,
                        if s.wind.source.is_empty() { "—" } else { &s.wind.source }
                    ),
                    None => "in attesa del primo scaricamento dai servizi NOAA".to_string(),
                };
                ui.label(self.fonts.body(sub));
            },
        );

        let (stamp, badge, color) = match (&self.snapshot, &self.assessment) {
            (Some(s), Some(a)) => {
                let g = a.g_scale;
                (
                    format!("aggiornato {}", fmt_stamp(s.fetched_at)),
                    if g == 0 {
                        "nessuna tempesta geomagnetica".to_string()
                    } else {
                        format!("tempesta geomagnetica G{g}")
                    },
                    if g == 0 { theme::GOOD } else { theme::BAD },
                )
            }
            _ => ("—".to_string(), "in caricamento".to_string(), theme::FAINT),
        };

        ui.scope_builder(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(pos2(split, row.top()), row.max))
                .layout(Layout::right_to_left(Align::Min)),
            |ui| {
                if ui.button("⟳  Aggiorna ora").clicked() {
                    let _ = self.trigger.send(());
                }
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.label(self.fonts.hint(stamp));
                    ui.label(
                        RichText::new(badge)
                            .size(11.5)
                            .color(color)
                            .family(self.fonts.semi.clone()),
                    );
                });
            },
        );

        ui.add_space(8.0);
        self.tab_bar(ui, full);
        ui.add_space(12.0);

        let (Some(snap), Some(a)) = (self.snapshot.clone(), self.assessment.clone()) else {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(6.0);
                ui.label(
                    self.fonts
                        .body("Scaricamento dei dati live da NOAA SWPC e WDC Kyoto…"),
                );
                ui.label(self.fonts.hint(
                    "elettroni GOES-19, spettro differenziale, vento solare e IMF al punto L1, Kp, Dst",
                ));
            });
            return;
        };

        match self.tab {
            Tab::Belts => self.belts_page(ui, &snap, &a),
            Tab::Wind => self.wind_page(ui, &snap, &a),
            Tab::Magneto => self.magneto_page(ui, &snap, &a),
            Tab::History => self.history_page(ui, &snap),
            Tab::Sources => self.sources_page(ui, &snap),
        }
        self.footer(ui, &snap);
    }

    /// Barra delle schede, con la guida generale a destra.
    fn tab_bar(&mut self, ui: &mut Ui, full: f32) {
        let (row, _) = ui.allocate_exact_size(vec2(full, 36.0), Sense::hover());
        let p = ui.painter_at(row);
        p.line_segment(
            [pos2(row.left(), row.bottom() - 0.5), pos2(row.right(), row.bottom() - 0.5)],
            theme::stroke(1.0, theme::BORDER),
        );

        let mut x = row.left();
        for tab in Tab::ALL {
            let font = self.fonts.heading(13.0);
            let w = p
                .layout_no_wrap(tab.label().to_string(), font.clone(), theme::TEXT)
                .size()
                .x
                + 30.0;
            let r = Rect::from_min_size(pos2(x, row.top()), vec2(w, 34.0));
            let resp = ui.interact(r, ui.id().with(("scheda", tab.label())), Sense::click());
            let selected = self.tab == tab;
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                if !selected {
                    p.rect_filled(r.shrink2(vec2(2.0, 4.0)), CornerRadius::same(6), theme::CARD_ALT);
                }
            }
            p.text(
                r.center() - vec2(0.0, 1.0),
                Align2::CENTER_CENTER,
                tab.label(),
                font,
                if selected {
                    theme::TEXT
                } else if resp.hovered() {
                    theme::SUB
                } else {
                    theme::FAINT
                },
            );
            if selected {
                p.rect_filled(
                    Rect::from_min_max(
                        pos2(r.left() + 12.0, row.bottom() - 3.0),
                        pos2(r.right() - 12.0, row.bottom()),
                    ),
                    CornerRadius::same(2),
                    theme::ACCENT,
                );
            }
            if resp.clicked() {
                self.tab = tab;
            }
            x += w + 4.0;
        }

        let badge = Rect::from_center_size(pos2(row.right() - 12.0, row.center().y - 1.0), vec2(20.0, 20.0));
        p.text(
            pos2(badge.left() - 8.0, badge.center().y),
            Align2::RIGHT_CENTER,
            "Guida",
            self.fonts.ui(11.5),
            theme::FAINT,
        );
        if help_badge(ui, badge, "guida", &self.fonts) {
            self.help = Some(Topic::Guide);
        }
    }

    /// Scheda "Fasce": il quadro d'insieme.
    fn belts_page(&mut self, ui: &mut Ui, snap: &Snapshot, a: &Assessment) {
        let full = ui.available_width();

        // riga KPI
        let gap = 10.0;
        let kw = ((full - gap * 5.0) / 6.0).max(120.0);
        let mut asked: Option<Topic> = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            let kp_col = if a.kp.unwrap_or(0.0) >= 5.0 {
                theme::BAD
            } else {
                theme::TEXT
            };
            let flux_col = match a.geo_flux {
                Some(f) if f >= 10_000.0 => theme::BAD,
                Some(f) if f >= 1000.0 => theme::WARN,
                Some(_) => theme::FLUX,
                None => theme::FAINT,
            };
            let mut hit = |t: Option<Topic>| {
                if t.is_some() {
                    asked = t;
                }
            };
            hit(kpi(
                ui,
                kw,
                "Kp stimato",
                &a.kp.map(|k| format!("{k:.1}")).unwrap_or_else(|| "—".into()),
                &format!("scala G{}", a.g_scale),
                kp_col,
                Topic::Kp,
                &self.fonts,
            ));
            hit(kpi(
                ui,
                kw,
                "Dst",
                &a.dst
                    .map(|d| format!("{d:.0} nT"))
                    .unwrap_or_else(|| "—".into()),
                a.storm.label(),
                storm_color(a.storm),
                Topic::Dst,
                &self.fonts,
            ));
            hit(kpi(
                ui,
                kw,
                "Vento solare",
                &a.wind_speed
                    .map(|v| format!("{v:.0} km/s"))
                    .unwrap_or_else(|| "—".into()),
                &format!(
                    "n = {} cm⁻³",
                    a.wind_density
                        .map(|n| format!("{n:.1}"))
                        .unwrap_or_else(|| "—".into())
                ),
                theme::ACCENT,
                Topic::Wind,
                &self.fonts,
            ));
            hit(kpi(
                ui,
                kw,
                "Bz / P dinamica",
                &a.bz
                    .map(|b| format!("{b:+.1} nT"))
                    .unwrap_or_else(|| "—".into()),
                &format!(
                    "P = {} nPa",
                    a.pdyn
                        .map(|x| format!("{x:.2}"))
                        .unwrap_or_else(|| "—".into())
                ),
                if a.bz.unwrap_or(0.0) < -5.0 {
                    theme::MAG
                } else {
                    theme::TEXT
                },
                Topic::Bz,
                &self.fonts,
            ));
            hit(kpi(
                ui,
                kw,
                "Magnetopausa",
                &a.r_mp
                    .map(|r| format!("{r:.1} R⊕"))
                    .unwrap_or_else(|| "—".into()),
                &format!(
                    "{} km",
                    a.r_mp
                        .map(|r| format!("{:.0}", r * RE_KM))
                        .unwrap_or_else(|| "—".into())
                ),
                if a.r_mp.unwrap_or(99.0) < GEO_L + 0.5 {
                    theme::BAD
                } else {
                    theme::TEXT
                },
                Topic::Magnetopause,
                &self.fonts,
            ));
            hit(kpi(
                ui,
                kw,
                "Elettroni >2 MeV",
                &a.geo_flux.map(sci).unwrap_or_else(|| "—".into()),
                "pfu a L = 6.6 (misura)",
                flux_col,
                Topic::Electrons,
                &self.fonts,
            ));
        });
        if asked.is_some() {
            self.help = asked;
        }

        ui.add_space(12.0);

        // striscia radiale
        let strip_sub = match &a.belt {
            Some(b) => format!(
                "bordo interno {:.1} R⊕ · bordo esterno {:.1} R⊕ · ampiezza {:.1} R⊕ · contenuto {} (u.a.)",
                b.l_inner_outer,
                b.l_outer_edge,
                b.size_index(),
                num(b.content_index)
            ),
            None => "dati GOES non disponibili".to_string(),
        };
        let _ = card(
            ui,
            full,
            152.0,
            "Sezione radiale delle fasce",
            &strip_sub,
            Topic::Strip,
            &self.fonts,
            |ui, w| match &a.belt {
                Some(b) => charts::strip(ui, w, b, a.r_mp),
                None => {
                    ui.label(self.fonts.body("nessun flusso misurato a L = 6.6"));
                }
            },
        )
        .map(|t| self.help = Some(t));

        ui.add_space(12.0);

        // profilo + spettro
        let left_w = ((full - 12.0) * 0.58).floor();
        let right_w = full - left_w - 12.0;
        let mut asked: Option<Topic> = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;

            let profile_sub = match &a.belt {
                Some(b) => format!(
                    "picco {:.1} R⊕ ({:.0} km) · larghezza {:.1} R⊕ · regime: {}",
                    b.l_peak,
                    b.peak_km(),
                    b.width,
                    b.regime.label()
                ),
                None => "profilo non calcolabile senza flusso misurato".to_string(),
            };
            let _ = card(
                ui,
                left_w,
                306.0,
                "Profilo radiale del flusso >2 MeV",
                &profile_sub,
                Topic::Profile,
                &self.fonts,
                |ui, w| match &a.belt {
                    Some(b) => charts::profile(ui, w, b, a.r_mp),
                    None => {
                        ui.label(self.fonts.body("—"));
                    }
                },
            )
            .map(|t| asked = Some(t));

            let spectrum_sub = format!(
                "γ = {} · flusso di energia {} µW/m²",
                a.gamma
                    .map(|g| format!("{g:.2}"))
                    .unwrap_or_else(|| "—".into()),
                a.energy_flux
                    .map(|e| format!("{e:.1}"))
                    .unwrap_or_else(|| "—".into())
            );
            let _ = card(
                ui,
                right_w,
                306.0,
                "Spettro differenziale (GOES-19)",
                &spectrum_sub,
                Topic::Spectrum,
                &self.fonts,
                |ui, w| {
                    charts::spectrum(ui, w, &snap.goes_e.spectrum, &snap.goes_e.spectrum_prev)
                },
            )
            .map(|t| asked = Some(t));
        });
        if asked.is_some() {
            self.help = asked;
        }

        ui.add_space(12.0);

        // serie storica
        let trend = match a.rate_1h {
            Some(r) if r > 0.15 => format!("in salita, {r:+.2} decadi/ora"),
            Some(r) if r < -0.15 => format!("in calo, {r:+.2} decadi/ora"),
            Some(r) => format!("stabile, {r:+.2} decadi/ora"),
            None => "variazione non calcolabile".to_string(),
        };
        let _ = card(
            ui,
            full,
            216.0,
            "Elettroni relativistici a L = 6.6 — ultime 24 ore",
            &format!("serie misurata da GOES-{} · {trend}", snap.goes_e.satellite),
            Topic::Series,
            &self.fonts,
            |ui, w| charts::series(ui, w, &snap.goes_e.integral_2mev),
        )
        .map(|t| self.help = Some(t));

        ui.add_space(12.0);

        // allerte
        let rows = a.alerts.len().min(6) as f32;
        let alerts_h = 66.0 + rows * 44.0;
        let (title_a, sub_a) = if a.alerts.is_empty() {
            (
                "Allerte e stato".to_string(),
                "nessuna soglia superata".to_string(),
            )
        } else {
            (
                format!("Allerte e stato ({})", a.alerts.len()),
                "ordinate dalla più grave".to_string(),
            )
        };
        let _ = card(
            ui,
            full,
            alerts_h,
            &title_a,
            &sub_a,
            Topic::Alerts,
            &self.fonts,
            |ui, _| alerts_body(ui, a, &self.fonts),
        )
        .map(|t| self.help = Some(t));

        ui.add_space(10.0);

        // note
        ui.horizontal_wrapped(|ui| {
            ui.label(self.fonts.hint(
                "Misure: flussi GOES (elettroni 79 keV–2.9 MeV, protoni, magnetometro), vento solare e IMF al punto L1, Kp stimato, Dst.",
            ));
            ui.label(self.fonts.hint(
                "Modello: posizione delle fasce, plasmapausa, magnetopausa. Fascia interna di protoni: climatologia, non misura.",
            ));
        });
        if let Some(b) = &a.belt {
            if b.truncated_by_magnetopause {
                ui.label(
                    RichText::new(
                        "La magnetopausa è dentro il bordo esterno stimato: fascia troncata.",
                    )
                    .size(10.5)
                    .color(theme::WARN),
                );
            }
        }
        if !snap.warnings.is_empty() {
            let mut w = String::from("Sorgenti non raggiungibili:");
            for x in snap.warnings.iter().take(3) {
                w.push(' ');
                w.push_str(x);
            }
            ui.label(RichText::new(w).size(10.5).color(theme::BAD));
        }
    }

    /// Riga di stato in fondo a ogni scheda.
    fn footer(&mut self, ui: &mut Ui, snap: &Snapshot) {
        ui.add_space(10.0);
        ui.label(self.fonts.hint(format!(
            "{} · storico locale: {} punti · aggiornamento automatico ogni {} min · server locale {}",
            fmt_short(snap.fetched_at),
            self.history.points.len(),
            REFRESH_SECS / 60,
            match &self.server {
                Some((p, _)) => format!("http://127.0.0.1:{p} (si chiude con l'app)"),
                None => "non disponibile (porta occupata)".to_string(),
            }
        )));
    }
}

impl VanAllenApp {
    /// Pannello di spiegazione del riquadro scelto col "?".
    ///
    /// Disegnato in un'Area con ordine Tooltip: sta sopra a qualsiasi pannello,
    /// compresi i grafici, indipendentemente dall'ordine di creazione dei layer.
    fn help_modal(&mut self, ctx: &egui::Context) {
        let fresh = self.help != self.help_prev;
        self.help_prev = self.help;
        let Some(topic) = self.help else { return };

        let screen = ctx.screen_rect();
        let width = 540.0f32.min(screen.width() - 32.0);

        // velo: scurisce il resto e intercetta i clic fuori
        let veil = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("aiuto-velo"),
        ));
        veil.rect_filled(screen, 0.0, Color32::from_black_alpha(165));

        let frame = egui::Frame::new()
            .fill(theme::CARD)
            .stroke(theme::stroke(1.0, theme::BORDER))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(24, 20))
            .shadow(egui::epaint::Shadow {
                offset: [0, 8],
                blur: 24,
                spread: 0,
                color: Color32::from_black_alpha(120),
            });

        let mut close = false;
        let area = egui::Area::new(egui::Id::new("aiuto-riquadro"))
            .order(egui::Order::Tooltip)
            .fixed_pos(pos2(
                (screen.center().x - width * 0.5).max(screen.left() + 16.0),
                (screen.top() + 90.0).max(screen.top() + 16.0),
            ))
            .interactable(true)
            .show(ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.set_width(width - 48.0);
                    ui.label(
                        RichText::new(topic.title())
                            .size(17.0)
                            .color(theme::TEXT)
                            .family(self.fonts.semi.clone()),
                    );
                    ui.add_space(9.0);
                    egui::ScrollArea::vertical()
                        .max_height((screen.height() - 230.0).max(160.0))
                        .show(ui, |ui| {
                            ui.label(rich_job(topic.body(), width - 60.0, &self.fonts));
                        });
                    ui.add_space(16.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let b = egui::Button::new(
                            RichText::new("Chiudi").size(12.5).color(theme::TEXT),
                        )
                        .fill(theme::ACCENT.gamma_multiply(0.16))
                        .corner_radius(CornerRadius::same(6))
                        .min_size(vec2(84.0, 28.0));
                        if ui.add(b).clicked() {
                            close = true;
                        }
                    });
                });
            });

        let rect = area.response.rect;
        let clicked_outside = ctx.input(|i| {
            i.pointer.any_click()
                && i.pointer
                    .interact_pos()
                    .map(|p| !rect.contains(p))
                    .unwrap_or(false)
        });

        if close || (clicked_outside && !fresh) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.help = None;
        }
    }
}

/// Testo di aiuto impaginato: `**così**` diventa grassetto bianco.
fn rich_job(text: &str, width: f32, f: &Fonts) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let normal = TextFormat {
        font_id: f.ui(12.5),
        color: theme::SUB,
        line_height: Some(18.5),
        valign: Align::BOTTOM,
        ..Default::default()
    };
    let bold = TextFormat {
        font_id: f.heading(12.5),
        color: theme::TEXT,
        line_height: Some(18.5),
        valign: Align::BOTTOM,
        ..Default::default()
    };
    for (i, part) in text.split("**").enumerate() {
        if !part.is_empty() {
            job.append(part, 0.0, if i % 2 == 1 { bold.clone() } else { normal.clone() });
        }
    }
    job
}
fn alerts_body(ui: &mut Ui, a: &Assessment, f: &Fonts) {
    if a.alerts.is_empty() {
        ui.label(
            RichText::new("Fasce in condizioni tranquille: nessuna soglia operativa superata.")
                .size(11.5)
                .color(theme::GOOD),
        );
        return;
    }
    for al in a.alerts.iter().take(6) {
        alert_row(ui, al, f);
        ui.add_space(4.0);
    }
}

fn alert_row(ui: &mut Ui, al: &Alert, f: &Fonts) {
    let color = sev_color(al.severity);
    ui.horizontal(|ui| {
        let (bar, _) = ui.allocate_exact_size(vec2(3.0, 30.0), Sense::hover());
        ui.painter().rect_filled(bar, CornerRadius::same(2), color);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(&al.title)
                    .size(11.5)
                    .color(color)
                    .family(f.semi.clone()),
            );
            ui.add(Label::new(RichText::new(&al.detail).size(10.0).color(theme::FAINT)).wrap());
        });
    });
}

// ---- formattazione ------------------------------------------------------

/// Numero compatto: 3832 → "3.8k", 0.42 → "0.42".
pub fn num(v: f64) -> String {
    let a = v.abs();
    if a >= 100_000.0 {
        format!("{:.0}k", v / 1000.0)
    } else if a >= 10_000.0 {
        format!("{v:.0}")
    } else if a >= 1000.0 {
        format!("{:.1}k", v / 1000.0)
    } else if a >= 100.0 {
        format!("{v:.0}")
    } else if a >= 10.0 {
        format!("{v:.1}")
    } else if a >= 1.0 {
        format!("{v:.2}")
    } else if a > 0.0 {
        format!("{v:.3}")
    } else {
        "—".to_string()
    }
}

/// Potenza di dieci leggibile: 38320 → "3.8·10⁴".
pub fn sci(v: f64) -> String {
    if v <= 0.0 {
        return "—".into();
    }
    let e = v.log10().floor();
    let m = v / 10f64.powf(e);
    if (-2.0..4.0).contains(&e) {
        return num(v);
    }
    let sup = ["⁰", "¹", "²", "³", "⁴", "⁵", "⁶", "⁷", "⁸", "⁹"];
    let mut s = String::from("10");
    if e < 0.0 {
        s.push('⁻');
    }
    for ch in format!("{:.0}", e.abs()).chars() {
        s.push_str(sup[ch.to_digit(10).unwrap_or(0) as usize]);
    }
    format!("{m:.1}·{s}")
}

// ---- avvio --------------------------------------------------------------

/// Icona della finestra, decodificata dal PNG incluso nel binario.
fn window_icon() -> Option<egui::IconData> {
    let png_bytes: &[u8] = include_bytes!("../../assets/icon-64.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());

    // porta tutto in RGBA8: il buffer può arrivare in RGB o in scala di grigi
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for px in buf.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha => {
            let step = if info.color_type == png::ColorType::Grayscale {
                1
            } else {
                2
            };
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for px in buf.chunks_exact(step) {
                let a = if step == 2 { px[1] } else { 255 };
                out.extend_from_slice(&[px[0], px[0], px[0], a]);
            }
            out
        }
        png::ColorType::Indexed => return None,
    };

    Some(egui::IconData {
        rgba,
        width: info.width,
        height: info.height,
    })
}

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Fasce di Van Allen — live")
        .with_inner_size([1460.0, 980.0])
        .with_min_inner_size([1040.0, 720.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Fasce di Van Allen — live",
        options,
        Box::new(|cc| Ok(Box::new(VanAllenApp::new(cc)))),
    )
}
