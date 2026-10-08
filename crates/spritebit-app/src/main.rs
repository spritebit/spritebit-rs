//! spritebit — Desktop-App (egui/eframe).
//!
//! Menüleiste, Sprite-Liste und Farben links, Zeichenfläche mit Zoom und
//! Verschieben, Stift und Radierer, Undo je Sprite, Speichern und Öffnen
//! (eigenes Format und Projektdatei der Web-Version). Gezeichnet wird immer
//! nur der sichtbare Ausschnitt (`spritebit_core::render_rgba_step`) — die
//! Fläche darf darum bis 8192×8192 groß sein.

// Im Release kein Konsolenfenster neben dem Programm.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod timeline;
mod tools_ui;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Sense, Stroke, Vec2};
use spritebit_core::{
    export_web, import_web, load_native, render_rgba_step, save_native, History, Project, Px, Rect, Sprite, MAX_SIDE,
};
use tools_ui::{Pointer, Tool};

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
/// Dateiendung des eigenen Formats.
const EXT: &str = "spritebit";

/// Schachbrett für transparente Stellen (dunkel, wie in der Web-Version).
const CHECKER: [[u8; 3]; 2] = [[0x20, 0x20, 0x2c], [0x2a, 0x2a, 0x38]];

/// Die Textur der Zeichenfläche und wofür sie gerechnet wurde. Ändert sich
/// nichts davon, wird sie nicht neu gerechnet.
struct CanvasTexture {
    handle: egui::TextureHandle,
    key: (usize, Rect, u32, usize, u64, bool),
}

/// Dialog „Neuer Sprite".
struct NewDialog {
    width: u32,
    height: u32,
}

struct SpritebitApp {
    project: Project,
    /// Undo je Sprite, gleiche Reihenfolge wie `project.sprites`.
    histories: Vec<History>,
    /// Wohin „Speichern" schreibt — `None`, solange nie gespeichert.
    path: Option<PathBuf>,
    dirty: bool,
    /// Zuletzt in die Titelleiste geschriebener Titel.
    title: String,
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
    /// Fehlermeldung, die als Fenster angezeigt wird.
    error: Option<String>,
    /// Hinweis in der Statusleiste (z. B. „Ebene ist gesperrt“).
    hint: Option<String>,
    playing: bool,
    /// Zeitpunkt (egui-Zeit, Sekunden), seit dem der aktuelle Frame steht.
    frame_started: f64,
    onion: bool,
    tool: Tool,
    /// Größe von Pinsel, Radierer und Spray (1–9).
    size: u32,
    /// Rechteck und Ellipse gefüllt.
    filled: bool,
    /// Form, die gerade aufgezogen wird: Anfang, Ende, Farbe.
    shape_start: Option<(i64, i64)>,
    shape_end: Option<(i64, i64)>,
    shape_value: Px,
    /// Der laufende Druck darf nicht malen (gesperrte Ebene, Abspielen beendet).
    blocked: bool,
    rng: spritebit_core::tools::Rng,
    /// Wo die Zeichenfläche zuletzt lag (Bildschirm) — für Tests und Zoom.
    canvas_rect: egui::Rect,
}

impl SpritebitApp {
    fn new() -> Self {
        let project = Project::default();
        let histories = project.sprites.iter().map(|_| History::default()).collect();
        SpritebitApp {
            project,
            histories,
            path: None,
            dirty: false,
            title: String::new(),
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
            error: None,
            hint: None,
            playing: false,
            frame_started: 0.0,
            onion: false,
            tool: Tool::Pencil,
            size: 1,
            filled: false,
            shape_start: None,
            shape_end: None,
            shape_value: 0,
            blocked: false,
            canvas_rect: egui::Rect::NOTHING,
            rng: spritebit_core::tools::Rng::new(
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64),
            ),
        }
    }

    fn sprite(&self) -> &Sprite {
        self.project.sprite()
    }

    fn history(&mut self) -> &mut History {
        &mut self.histories[self.project.current]
    }

    /// Bild hat sich geändert: Textur neu rechnen, als ungespeichert merken.
    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.dirty = true;
    }

    fn undo(&mut self) {
        let cur = self.project.current;
        if self.histories[cur].undo(&mut self.project.sprites[cur]) {
            self.changed();
        }
    }

    fn redo(&mut self) {
        let cur = self.project.current;
        if self.histories[cur].redo(&mut self.project.sprites[cur]) {
            self.changed();
        }
    }

    fn select_sprite(&mut self, i: usize) {
        if i < self.project.sprites.len() && i != self.project.current {
            self.project.current = i;
            self.stroke_last = None;
            self.fit_pending = true;
            self.version = self.version.wrapping_add(1);
        }
    }

    fn add_sprite(&mut self, width: u32, height: u32) {
        let name = self.project.fresh_name();
        if let Ok(sp) = Sprite::new(name, width, height) {
            self.project.sprites.push(sp);
            self.histories.push(History::default());
            self.select_sprite(self.project.sprites.len() - 1);
            self.dirty = true;
        }
    }

    // ── Datei ───────────────────────────────────────────────────────
    fn replace_project(&mut self, project: Project, path: Option<PathBuf>) {
        self.histories = project.sprites.iter().map(|_| History::default()).collect();
        self.project = project;
        self.path = path;
        self.dirty = false;
        self.stroke_last = None;
        self.fit_pending = true;
        self.version = self.version.wrapping_add(1);
    }

    fn open(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Projekt öffnen")
            .add_filter("spritebit-Projekt", &[EXT, "json"])
            .add_filter("Alle Dateien", &["*"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read(&path) {
            Err(e) => self.error = Some(format!("{} konnte nicht gelesen werden: {e}", path.display())),
            Ok(bytes) => {
                // Eigenes Format erkennt man am Anfang; alles andere wird als
                // Projektdatei der Web-Version versucht.
                let result = if bytes.starts_with(b"SPRITEBIT\0") {
                    load_native(&bytes).map(|p| (p, Some(path.clone())))
                } else {
                    // Aus einer Web-Datei wird beim Speichern eine .spritebit-Datei.
                    std::str::from_utf8(&bytes)
                        .map_err(|_| spritebit_core::IoError::NotAProject("kein Text".into()))
                        .and_then(import_web)
                        .map(|p| (p, None))
                };
                match result {
                    Ok((p, keep_path)) => {
                        self.replace_project(p, keep_path);
                        if self.path.is_none() {
                            // Web-Projekt: ungespeichert, damit „Speichern" nach dem Ziel fragt.
                            self.dirty = true;
                        }
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
        }
    }

    fn save(&mut self) {
        match self.path.clone() {
            Some(path) => self.write_native(&path),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) {
        let name = format!("{}.{EXT}", self.sprite().name);
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Projekt speichern")
            .add_filter("spritebit-Projekt", &[EXT])
            .set_file_name(name)
            .save_file()
        {
            let path = if path.extension().is_none() { path.with_extension(EXT) } else { path };
            self.write_native(&path);
        }
    }

    fn write_native(&mut self, path: &Path) {
        match std::fs::write(path, save_native(&self.project)) {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.dirty = false;
            }
            Err(e) => self.error = Some(format!("{} konnte nicht gespeichert werden: {e}", path.display())),
        }
    }

    fn export_web(&mut self) {
        let name = format!("{}.json", self.sprite().name);
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Als Web-Projekt exportieren")
            .add_filter("Web-Projekt (JSON)", &["json"])
            .set_file_name(name)
            .save_file()
        {
            if let Err(e) = std::fs::write(&path, export_web(&self.project)) {
                self.error = Some(format!("{} konnte nicht geschrieben werden: {e}", path.display()));
            }
        }
    }

    /// Titelleiste: Dateiname und ein Sternchen bei ungespeicherten Änderungen.
    fn sync_title(&mut self, ctx: &egui::Context) {
        let file = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(|| "Unbenannt".to_string(), |n| n.to_string_lossy().into_owned());
        let title = format!("{}{} — spritebit", file, if self.dirty { " *" } else { "" });
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    // ── Ansicht ─────────────────────────────────────────────────────
    /// Ganze Fläche einpassen und mittig stellen.
    fn fit(&mut self, area: egui::Rect) {
        let (w, h) = (self.sprite().width as f32, self.sprite().height as f32);
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
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        // Die längeren Kürzel zuerst: Strg+Umschalt+S darf nicht als Strg+S gelten.
        let (save_as, save, open, redo2, undo, redo) = ctx.input_mut(|i| {
            (
                i.consume_key(cmd_shift, Key::S),
                i.consume_key(cmd, Key::S),
                i.consume_key(cmd, Key::O),
                i.consume_key(cmd_shift, Key::Z),
                i.consume_key(cmd, Key::Z),
                i.consume_key(cmd, Key::Y),
            )
        });
        // Timeline-Tasten nur, wenn gerade kein Eingabefeld den Fokus hat.
        if !ctx.egui_wants_keyboard_input() {
            let none = Modifiers::NONE;
            let (play, prev, next, first, last) = ctx.input_mut(|i| {
                (
                    i.consume_key(none, Key::Enter),
                    i.consume_key(none, Key::Comma),
                    i.consume_key(none, Key::Period),
                    i.consume_key(none, Key::Home),
                    i.consume_key(none, Key::End),
                )
            });
            let n = self.sprite().frames.len();
            let f = self.sprite().frame;
            if play {
                self.toggle_play(ctx);
            }
            if prev {
                self.project.sprite_mut().frame = (f + n - 1) % n;
            }
            if next {
                self.project.sprite_mut().frame = (f + 1) % n;
            }
            if first {
                self.project.sprite_mut().frame = 0;
            }
            if last {
                self.project.sprite_mut().frame = n - 1;
            }
        }
        if save_as {
            self.save_as();
        } else if save {
            self.save();
        }
        if open {
            self.open();
        }
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
                    self.new_dialog = Some(NewDialog { width: self.sprite().width, height: self.sprite().height });
                }
                ui.separator();
                if ui.add(egui::Button::new("Öffnen …").shortcut_text("Strg+O")).clicked() {
                    self.open();
                }
                if ui.add(egui::Button::new("Speichern").shortcut_text("Strg+S")).clicked() {
                    self.save();
                }
                if ui.add(egui::Button::new("Speichern unter …").shortcut_text("Strg+Umschalt+S")).clicked() {
                    self.save_as();
                }
                ui.separator();
                if ui.button("Als Web-Projekt exportieren …").clicked() {
                    self.export_web();
                }
                ui.separator();
                if ui.button("Beenden").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Bearbeiten", |ui| {
                let (can_undo, can_redo) = (self.history().can_undo(), self.history().can_redo());
                if ui.add_enabled(can_undo, egui::Button::new("Rückgängig").shortcut_text("Strg+Z")).clicked() {
                    self.undo();
                }
                if ui.add_enabled(can_redo, egui::Button::new("Wiederholen").shortcut_text("Strg+Y")).clicked() {
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

    // ── Linke Leiste: Sprites und Farben ────────────────────────────
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.strong("Sprites");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("+").on_hover_text("Neuer Sprite").clicked() {
                    self.new_dialog = Some(NewDialog { width: self.sprite().width, height: self.sprite().height });
                }
            });
        });
        let mut pick = None;
        for (i, sp) in self.project.sprites.iter().enumerate() {
            let label = format!("{}  ·  {}×{}", sp.name, sp.width, sp.height);
            if ui.selectable_label(i == self.project.current, label).clicked() {
                pick = Some(i);
            }
        }
        if let Some(i) = pick {
            self.select_sprite(i);
        }

        ui.add_space(10.0);
        ui.strong("Farben");
        ui.add_space(4.0);
        let palette = self.project.current_palette();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            let size = Vec2::splat(24.0);
            for i in 0..=palette.len() as u16 {
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let p = ui.painter();
                match palette.get(i) {
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
        });
        ui.add_space(4.0);
        ui.weak(format!("Palette: {}", palette.name));
    }

    // ── Statusleiste ────────────────────────────────────────────────
    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let sp = self.sprite();
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
            if let Some(h) = &self.hint {
                ui.separator();
                ui.colored_label(Color32::from_rgb(0xf2, 0x8b, 0x82), h);
            }
        });
    }

    // ── Zeichenfläche ───────────────────────────────────────────────
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let area = resp.rect;
        self.canvas_rect = area;
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
        let (pressed, released) = ui.input(|i| {
            (
                i.pointer.primary_pressed() || i.pointer.secondary_pressed(),
                i.pointer.primary_released() || i.pointer.secondary_released(),
            )
        });
        if let Some(at) = pointer.filter(|p| area.contains(*p)) {
            if zoom_delta != 1.0 {
                self.zoom_at(zoom_delta, at, area);
            } else if scroll != 0.0 {
                self.zoom_at((scroll * 0.0025).exp(), at, area);
            }
        }
        let panning = middle || (space && primary) || (self.tool == Tool::Pan && primary);
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

        // Werkzeug anwenden (tools_ui.rs).
        let p = Pointer {
            cell: pointer.map(to_cell),
            over: resp.hovered() || resp.dragged(),
            primary,
            secondary,
            pressed,
            released,
            panning,
        };
        self.use_tool(&p, ui.ctx());

        // Sichtbarer Ausschnitt in Sprite-Pixeln.
        let (sw, sh) = (self.sprite().width, self.sprite().height);
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
        let onion = self.onion && !self.playing && self.sprite().frames.len() > 1;
        let key = (self.project.current, rect, step, self.sprite().frame, self.version, onion);
        if self.texture.as_ref().map(|t| t.key) != Some(key) {
            let palette = self.project.current_palette();
            let sp = self.sprite();
            let (mut buf, tw, th) = render_rgba_step(sp, &palette, sp.frame, rect, step);
            // Onion Skin: Nachbar-Frames, nur ihre Form — rot davor, blau danach.
            let n = sp.frames.len();
            let mut neighbours: Vec<(Vec<u8>, [u8; 3])> = Vec::new();
            if onion {
                if sp.frame > 0 {
                    neighbours.push((render_rgba_step(sp, &palette, sp.frame - 1, rect, step).0, [255, 96, 96]));
                }
                if sp.frame + 1 < n {
                    neighbours.push((render_rgba_step(sp, &palette, sp.frame + 1, rect, step).0, [96, 156, 255]));
                }
            }
            // Transparente Stellen als Schachbrett, ein Feld je Sprite-Pixel.
            for ty in 0..th {
                for tx in 0..tw {
                    let o = ((ty * tw + tx) * 4) as usize;
                    if buf[o + 3] == 0 {
                        let (x, y) = (x0 + tx * step, y0 + ty * step);
                        let mut c = CHECKER[((x + y) % 2) as usize];
                        for (nb, tint) in &neighbours {
                            if nb[o + 3] != 0 {
                                for (ch, t) in c.iter_mut().zip(tint) {
                                    *ch = (*ch as f32 * 0.7 + *t as f32 * 0.3) as u8;
                                }
                            }
                        }
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
        // Form, die gerade aufgezogen wird.
        self.shape_preview(&painter, origin, zoom);
        // Rand der Fläche.
        let border = egui::Rect::from_min_size(origin, Vec2::new(sw as f32, sh as f32) * zoom);
        painter.rect_stroke(border, 0.0, Stroke::new(1.0, Color32::from_gray(70)), egui::StrokeKind::Outside);
    }

    // ── Dialoge ─────────────────────────────────────────────────────
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
            self.add_sprite(w, h);
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
        let mut dismiss = false;
        if let Some(msg) = &self.error {
            egui::Window::new("Hinweis").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(msg);
                if ui.button("OK").clicked() {
                    dismiss = true;
                }
            });
        }
        if dismiss {
            self.error = None;
        }
    }
}

impl eframe::App for SpritebitApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        self.tool_keys(&ctx);
        self.advance_playback(&ctx);
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("tools").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("timeline").resizable(true).default_size(150.0).show(ui, |ui| self.timeline(ui));
        egui::Panel::left("side").resizable(true).default_size(180.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
        self.dialogs(&ctx);
        self.sync_title(&ctx);
    }
}

#[cfg(test)]
mod tests {
    //! Oberflächen-Tests: die App läuft ohne Fenster (egui_kittest), Maus und
    //! Tasten werden simuliert, geprüft wird das Bild im Sprite.

    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    fn app<'a>() -> Harness<'a, SpritebitApp> {
        let mut h = Harness::builder().with_size(Vec2::new(1280.0, 800.0)).build_eframe(|_| SpritebitApp::new());
        h.run();
        h
    }

    /// Bildschirmpunkt in der Mitte von Sprite-Pixel (x, y).
    fn at(h: &Harness<'_, SpritebitApp>, x: f32, y: f32) -> Pos2 {
        let a = h.state();
        a.canvas_rect.min + a.pan + Vec2::new(x + 0.5, y + 0.5) * a.zoom
    }

    fn drag(h: &mut Harness<'_, SpritebitApp>, from: (f32, f32), to: (f32, f32)) {
        let (a, b) = (at(h, from.0, from.1), at(h, to.0, to.1));
        h.hover_at(a);
        h.run();
        h.drag_at(a);
        h.run();
        for k in 1..=8 {
            h.hover_at(a + (b - a) * (k as f32 / 8.0));
            h.run();
        }
        h.drop_at(b);
        h.run();
    }

    fn px(h: &Harness<'_, SpritebitApp>, x: u32, y: u32) -> u16 {
        let s = h.state().project.sprite();
        s.cel(s.frame, s.layer).get(x, y)
    }

    #[test]
    fn stift_malt_einen_strich_ohne_luecken() {
        let mut h = app();
        drag(&mut h, (10.0, 10.0), (20.0, 10.0));
        for x in 10..=20 {
            assert_eq!(px(&h, x, 10), 5, "Pixel ({x}, 10)");
        }
        assert_eq!(px(&h, 21, 10), 0);
        assert!(h.state().dirty);
    }

    #[test]
    fn rueckgaengig_nimmt_den_strich_zurueck() {
        let mut h = app();
        drag(&mut h, (5.0, 5.0), (8.0, 5.0));
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run();
        assert_eq!(px(&h, 5, 5), 0);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Y);
        h.run();
        assert_eq!(px(&h, 5, 5), 5);
    }

    #[test]
    fn rechteck_aus_der_werkzeugleiste() {
        let mut h = app();
        h.get_by_label("Rechteck").click();
        h.run();
        assert_eq!(h.state().tool, Tool::Rect);
        drag(&mut h, (10.0, 10.0), (14.0, 13.0));
        assert_eq!(px(&h, 10, 10), 5);
        assert_eq!(px(&h, 14, 13), 5);
        assert_eq!(px(&h, 12, 10), 5, "obere Kante");
        assert_eq!(px(&h, 12, 11), 0, "innen leer");
    }

    #[test]
    fn fuellen_per_taste() {
        let mut h = app();
        h.key_press(Key::F);
        h.run();
        assert_eq!(h.state().tool, Tool::Fill);
        let p = at(&h, 30.0, 30.0);
        h.hover_at(p);
        h.run();
        h.drag_at(p);
        h.run();
        h.drop_at(p);
        h.run();
        assert_eq!(px(&h, 0, 0), 5);
        assert_eq!(px(&h, 63, 63), 5);
    }

    #[test]
    fn gesperrte_ebene_wird_nicht_bemalt() {
        let mut h = app();
        h.state_mut().project.sprite_mut().layers[0].locked = true;
        drag(&mut h, (10.0, 10.0), (12.0, 10.0));
        assert_eq!(px(&h, 10, 10), 0);
        assert!(h.state().hint.as_deref().is_some_and(|t| t.contains("gesperrt")));
    }

    #[test]
    fn neuer_frame_aus_der_timeline() {
        let mut h = app();
        h.get_by_label("+ Frame").click();
        h.run();
        assert_eq!(h.state().project.sprite().frames.len(), 2);
        assert_eq!(h.state().project.sprite().frame, 1);
    }
}
