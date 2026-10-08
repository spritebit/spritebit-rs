//! spritebit — Desktop-App (egui/eframe).
//!
//! Erste Fassung: Menüleiste, Farbleiste, Zeichenfläche mit Zoom und
//! Verschieben, Stift und Radierer, Undo. Gezeichnet wird immer nur der
//! sichtbare Ausschnitt (`spritebit_core::render_rgba_step`) — die Fläche
//! darf darum bis 8192×8192 groß sein.

// Im Release kein Konsolenfenster neben dem Programm.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Sense, Stroke, Vec2};
use spritebit_core::{render_rgba_step, tools, History, Palette, Rect, Sprite, MAX_SIDE};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("spritebit")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native("spritebit", options, Box::new(|_cc| Ok(Box::new(SpritebitApp::new()))))
}

/// Grenzen der Zoomstufe (Bildschirm-Pixel je Sprite-Pixel).
const ZOOM_MIN: f32 = 0.05;
const ZOOM_MAX: f32 = 64.0;
/// Ab dieser Zoomstufe werden Gitterlinien gezeichnet.
const GRID_FROM: f32 = 8.0;

/// Schachbrett für transparente Stellen (dunkel, wie in der Web-Version).
const CHECKER: [[u8; 3]; 2] = [[0x20, 0x20, 0x2c], [0x2a, 0x2a, 0x38]];

/// Die Textur der Zeichenfläche und wofür sie gerechnet wurde. Ändert sich
/// nichts davon, wird sie nicht neu gerechnet.
struct CanvasTexture {
    handle: egui::TextureHandle,
    key: (Rect, u32, usize, u64),
}

/// Dialog „Neuer Sprite".
struct NewDialog {
    width: u32,
    height: u32,
}

struct SpritebitApp {
    sprite: Sprite,
    palette: Palette,
    history: History,
    /// Aktuelle Farbe (Palette-Nummer, 0 = Radierer).
    color: u16,
    zoom: f32,
    /// Lage der linken oberen Sprite-Ecke relativ zur Zeichenfläche.
    pan: Vec2,
    fit_pending: bool,
    show_grid: bool,
    texture: Option<CanvasTexture>,
    /// Zählt jede Änderung am Bild hoch — Teil des Textur-Schlüssels.
    version: u64,
    /// Letzte Pixel-Position eines laufenden Strichs.
    stroke_last: Option<(i64, i64)>,
    hover: Option<(i64, i64)>,
    new_dialog: Option<NewDialog>,
    about_open: bool,
}

impl SpritebitApp {
    fn new() -> Self {
        SpritebitApp {
            sprite: Sprite::new("Sprite 1", 64, 64).expect("gültige Größe"),
            palette: Palette::grayscale(),
            history: History::default(),
            color: 5,
            zoom: 8.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            show_grid: true,
            texture: None,
            version: 0,
            stroke_last: None,
            hover: None,
            new_dialog: None,
            about_open: false,
        }
    }

    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    fn undo(&mut self) {
        if self.history.undo(&mut self.sprite) {
            self.changed();
        }
    }

    fn redo(&mut self) {
        if self.history.redo(&mut self.sprite) {
            self.changed();
        }
    }

    fn new_sprite(&mut self, width: u32, height: u32) {
        if let Ok(sp) = Sprite::new("Sprite 1", width, height) {
            self.sprite = sp;
            self.history.clear();
            self.fit_pending = true;
            self.changed();
        }
    }

    /// Ganze Fläche einpassen und mittig stellen.
    fn fit(&mut self, area: egui::Rect) {
        let (w, h) = (self.sprite.width as f32, self.sprite.height as f32);
        let z = (area.width() / w).min(area.height() / h) * 0.9;
        self.zoom = z.clamp(ZOOM_MIN, ZOOM_MAX);
        self.pan = (area.size() - Vec2::new(w, h) * self.zoom) / 2.0;
    }

    /// Zoomen um einen Bildschirmpunkt: der Sprite-Pixel darunter bleibt stehen.
    fn zoom_at(&mut self, factor: f32, at: Pos2, area: egui::Rect) {
        let new = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        let origin = area.min + self.pan;
        let origin = at - (at - origin) * (new / self.zoom);
        self.pan = origin - area.min;
        self.zoom = new;
    }

    // ── Tastenkürzel ────────────────────────────────────────────────
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let (undo, redo, redo2) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::Y),
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z),
            )
        });
        if undo {
            self.undo();
        }
        if redo || redo2 {
            self.redo();
        }
    }

    // ── Menüleiste ──────────────────────────────────────────────────
    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Datei", |ui| {
                if ui.button("Neuer Sprite …").clicked() {
                    self.new_dialog = Some(NewDialog { width: self.sprite.width, height: self.sprite.height });
                }
                ui.separator();
                if ui.button("Beenden").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Bearbeiten", |ui| {
                if ui
                    .add_enabled(self.history.can_undo(), egui::Button::new("Rückgängig").shortcut_text("Strg+Z"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(self.history.can_redo(), egui::Button::new("Wiederholen").shortcut_text("Strg+Y"))
                    .clicked()
                {
                    self.redo();
                }
            });
            ui.menu_button("Ansicht", |ui| {
                if ui.button("Einpassen").clicked() {
                    self.fit_pending = true;
                }
                if ui.button("100 %").clicked() {
                    self.zoom = 1.0;
                }
                ui.checkbox(&mut self.show_grid, "Gitter");
            });
            ui.menu_button("Hilfe", |ui| {
                if ui.button("Über spritebit").clicked() {
                    self.about_open = true;
                }
            });
        });
    }

    // ── Farbleiste ──────────────────────────────────────────────────
    fn palette_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.label("Farben");
        ui.add_space(4.0);
        let size = Vec2::splat(26.0);
        for i in 0..=self.palette.len() as u16 {
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
            let p = ui.painter();
            match self.palette.get(i) {
                Some([r, g, b]) => {
                    p.rect_filled(rect, 3.0, Color32::from_rgb(r, g, b));
                }
                None => {
                    // Transparent: kleines Schachbrett.
                    let half = rect.size() / 2.0;
                    for (qx, qy) in [(0u8, 0u8), (1, 0), (0, 1), (1, 1)] {
                        let [r, g, b] = CHECKER[((qx + qy) % 2) as usize];
                        let min = rect.min + Vec2::new(qx as f32 * half.x, qy as f32 * half.y);
                        p.rect_filled(egui::Rect::from_min_size(min, half), 0.0, Color32::from_rgb(r, g, b));
                    }
                }
            }
            if self.color == i {
                p.rect_stroke(rect.expand(2.0), 4.0, Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Outside);
            }
            if resp.clicked() {
                self.color = i;
            }
            resp.on_hover_text(if i == 0 { "0 · Transparent (Radierer)".to_string() } else { format!("Farbe {i}") });
        }
    }

    // ── Statusleiste ────────────────────────────────────────────────
    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let sp = &self.sprite;
            ui.label(format!("{} × {} px", sp.width, sp.height));
            ui.separator();
            ui.label(format!("Zoom {:.0} %", self.zoom * 100.0));
            if let Some((x, y)) = self.hover {
                ui.separator();
                ui.label(format!("({x}, {y})"));
            }
            ui.separator();
            let tiles: usize = sp.images.iter().map(|i| i.allocated_tiles()).sum();
            let kib = tiles * (spritebit_core::TILE * spritebit_core::TILE) as usize * 2 / 1024;
            ui.label(format!("{tiles} Kacheln · {kib} KiB"));
        });
    }

    // ── Zeichenfläche ───────────────────────────────────────────────
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let area = resp.rect;
        if self.fit_pending {
            self.fit(area);
            self.fit_pending = false;
        }

        // Eingaben: Mausrad zoomt um den Zeiger, mittlere Taste oder
        // Leertaste + Ziehen verschiebt, links malt, rechts radiert.
        let (scroll, zoom_delta, pointer, primary, secondary, middle, space, delta) = ui.input(|i| {
            (
                i.smooth_scroll_delta.y,
                i.zoom_delta(),
                i.pointer.hover_pos(),
                i.pointer.primary_down(),
                i.pointer.secondary_down(),
                i.pointer.middle_down(),
                i.key_down(Key::Space),
                i.pointer.delta(),
            )
        });
        if let Some(at) = pointer.filter(|p| area.contains(*p)) {
            if zoom_delta != 1.0 {
                self.zoom_at(zoom_delta, at, area);
            } else if scroll != 0.0 {
                self.zoom_at((scroll * 0.0025).exp(), at, area);
            }
        }
        let panning = middle || (space && primary);
        if panning && (resp.hovered() || resp.dragged()) {
            self.pan += delta;
        }

        let origin = area.min + self.pan;
        let zoom = self.zoom;
        let to_cell = |p: Pos2| {
            let v = (p - origin) / zoom;
            (v.x.floor() as i64, v.y.floor() as i64)
        };
        self.hover = pointer.filter(|p| area.contains(*p)).map(to_cell);

        // Malen
        let painting = !panning && (primary || secondary) && (resp.hovered() || resp.dragged());
        if painting {
            if let Some(cell) = pointer.map(to_cell) {
                if self.stroke_last.is_none() {
                    self.history.record(&self.sprite);
                }
                let from = self.stroke_last.unwrap_or(cell);
                let value = if secondary { 0 } else { self.color };
                let (w, h) = (self.sprite.width as i64, self.sprite.height as i64);
                let img = self.sprite.active();
                for (x, y) in tools::line(from.0, from.1, cell.0, cell.1) {
                    if x >= 0 && y >= 0 && x < w && y < h {
                        img.set(x as u32, y as u32, value);
                    }
                }
                self.stroke_last = Some(cell);
                self.changed();
            }
        } else {
            self.stroke_last = None;
        }

        // Sichtbarer Ausschnitt in Sprite-Pixeln.
        let (sw, sh) = (self.sprite.width, self.sprite.height);
        let lo = (area.min - origin) / zoom;
        let hi = (area.max - origin) / zoom;
        let x0 = lo.x.floor().clamp(0.0, sw as f32) as u32;
        let y0 = lo.y.floor().clamp(0.0, sh as f32) as u32;
        let x1 = hi.x.ceil().clamp(0.0, sw as f32) as u32;
        let y1 = hi.y.ceil().clamp(0.0, sh as f32) as u32;
        painter.rect_filled(area, 0.0, Color32::from_rgb(0x14, 0x14, 0x18));
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let rect = Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
        // Herausgezoomt: nur jedes n-te Pixel, sonst wäre die Textur größer
        // als der Bildschirm.
        let step = if zoom < 1.0 { (1.0 / zoom).floor() as u32 } else { 1 };
        let key = (rect, step, self.sprite.frame, self.version);
        if self.texture.as_ref().map(|t| t.key) != Some(key) {
            let (mut buf, tw, th) = render_rgba_step(&self.sprite, &self.palette, self.sprite.frame, rect, step);
            // Transparente Stellen als Schachbrett, ein Feld je Sprite-Pixel.
            for ty in 0..th {
                for tx in 0..tw {
                    let o = ((ty * tw + tx) * 4) as usize;
                    if buf[o + 3] == 0 {
                        let (x, y) = (x0 + tx * step, y0 + ty * step);
                        let c = CHECKER[((x + y) % 2) as usize];
                        buf[o..o + 3].copy_from_slice(&c);
                        buf[o + 3] = 255;
                    }
                }
            }
            let image = egui::ColorImage::from_rgba_unmultiplied([tw as usize, th as usize], &buf);
            match &mut self.texture {
                Some(t) => {
                    t.handle.set(image, egui::TextureOptions::NEAREST);
                    t.key = key;
                }
                None => {
                    let handle = ui.ctx().load_texture("canvas", image, egui::TextureOptions::NEAREST);
                    self.texture = Some(CanvasTexture { handle, key });
                }
            }
        }
        if let Some(t) = &self.texture {
            let [tw, th] = t.handle.size();
            // Die Textur deckt ganze Blöcke ab — am Rand höchstens bis zur Sprite-Kante.
            let ex = (x0 + tw as u32 * step).min(sw);
            let ey = (y0 + th as u32 * step).min(sh);
            let screen = egui::Rect::from_min_max(
                origin + Vec2::new(x0 as f32, y0 as f32) * zoom,
                origin + Vec2::new(ex as f32, ey as f32) * zoom,
            );
            let uv = egui::Rect::from_min_max(
                Pos2::ZERO,
                Pos2::new(
                    (ex - x0) as f32 / (tw as u32 * step) as f32,
                    (ey - y0) as f32 / (th as u32 * step) as f32,
                ),
            );
            painter.image(t.handle.id(), screen, uv, Color32::WHITE);
        }

        // Gitterlinien, nur stark hineingezoomt und nur im Ausschnitt.
        if self.show_grid && zoom >= GRID_FROM {
            let line = Stroke::new(1.0, Color32::from_white_alpha(18));
            for x in x0..=x1 {
                let sx = origin.x + x as f32 * zoom;
                painter.line_segment(
                    [Pos2::new(sx, origin.y + y0 as f32 * zoom), Pos2::new(sx, origin.y + y1 as f32 * zoom)],
                    line,
                );
            }
            for y in y0..=y1 {
                let sy = origin.y + y as f32 * zoom;
                painter.line_segment(
                    [Pos2::new(origin.x + x0 as f32 * zoom, sy), Pos2::new(origin.x + x1 as f32 * zoom, sy)],
                    line,
                );
            }
        }
        // Rand der Fläche.
        let border = egui::Rect::from_min_size(origin, Vec2::new(sw as f32, sh as f32) * zoom);
        painter.rect_stroke(border, 0.0, Stroke::new(1.0, Color32::from_gray(70)), egui::StrokeKind::Outside);
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let mut create = None;
        let mut close = false;
        if let Some(d) = &mut self.new_dialog {
            egui::Window::new("Neuer Sprite").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Breite");
                    ui.add(egui::DragValue::new(&mut d.width).range(1..=MAX_SIDE));
                    ui.label("Höhe");
                    ui.add(egui::DragValue::new(&mut d.height).range(1..=MAX_SIDE));
                });
                ui.horizontal(|ui| {
                    for s in [32, 64, 256, 1024, 4096, 8192] {
                        if ui.button(format!("{s}")).clicked() {
                            d.width = s;
                            d.height = s;
                        }
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Anlegen").clicked() {
                        create = Some((d.width, d.height));
                    }
                    if ui.button("Abbrechen").clicked() {
                        close = true;
                    }
                });
            });
        }
        if let Some((w, h)) = create {
            self.new_sprite(w, h);
            close = true;
        }
        if close {
            self.new_dialog = None;
        }
        if self.about_open {
            egui::Window::new("Über spritebit")
                .collapsible(false)
                .resizable(false)
                .open(&mut self.about_open)
                .show(ctx, |ui| {
                    ui.label("spritebit — Pixel-Art-Editor");
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.label("© 2026 Marco Jan");
                });
        }
    }
}

impl eframe::App for SpritebitApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("palette").resizable(false).exact_size(52.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.palette_panel(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
        self.dialogs(&ctx);
    }
}
