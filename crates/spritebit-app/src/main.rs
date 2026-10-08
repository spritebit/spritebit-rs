//! spritebit — Desktop-App (egui/eframe).
//!
//! Menüleiste, Sprite-Liste und Farben links, Zeichenfläche mit Zoom und
//! Verschieben, Stift und Radierer, Undo je Sprite, Speichern und Öffnen
//! (eigenes Format und Projektdatei der Web-Version). Gezeichnet wird immer
//! nur der sichtbare Ausschnitt (`spritebit_core::render_rgba_step`) — die
//! Fläche darf darum bis 8192×8192 groß sein.

// Im Release kein Konsolenfenster neben dem Programm.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod export_ui;
mod i18n;
mod icons;
mod image_ui;
mod palette_ui;
mod selection_ui;
mod timeline;
mod tools_ui;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Sense, Stroke, Vec2};
use spritebit_core::{
    export_web, import_web, load_native, render_rgba_step, save_native, selection, Clip, History, Project, Px, Rect,
    Selection, Sprite, MAX_SIDE,
};
use tools_ui::{Pointer, Tool};
use crate::i18n::{tr, trf, keys};

fn main() -> eframe::Result {
    i18n::load();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("spritebit")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "spritebit",
        options,
        Box::new(|cc| {
            // SVG-Icons (icons.rs) brauchen den Bild-Lader von egui_extras.
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(SpritebitApp::new()))
        }),
    )
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
    selection: Option<Selection>,
    /// Angehobener Inhalt der Auswahl, der gerade verschoben wird.
    float: Option<selection_ui::Float>,
    sel_drag: Option<selection_ui::SelDrag>,
    clipboard: Option<Clip>,
    /// Toleranz von Farbwahl und Zauberstab, 0.0–1.0.
    tolerance: f64,
    /// Symmetrie: an der senkrechten (x) bzw. waagerechten (y) Mitte spiegeln.
    mirror_x: bool,
    mirror_y: bool,
    palette_edit: bool,
    export_dialog: Option<export_ui::ExportDialog>,
    /// Bereich in der Timeline (Shift-Klick) und sein Ausgangspunkt.
    cel_range: Option<spritebit_core::cels::CelRange>,
    cel_anchor: Option<(usize, usize)>,
    cel_clip: Option<spritebit_core::cels::CelClip>,
    /// Welcher Tag gerade bearbeitet wird.
    tag_edit: Option<usize>,
    /// Ebene, die gerade umbenannt wird, und der Name im Feld.
    rename_layer: Option<(usize, String)>,
    /// Stelle in der Runde eines Tags beim Abspielen (Ping-Pong).
    play_step: usize,
    /// Rückfrage wegen ungespeicherter Änderungen — und was danach kommt.
    unsaved_ask: Option<Pending>,
    /// Schließen ist bestätigt (nach Speichern oder Verwerfen).
    allow_close: bool,
    /// Panels „Bild“ und „Aufräumen“.
    image: image_ui::ImagePanel,
}

/// Was nach der Rückfrage „Ungespeicherte Änderungen" passieren soll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Close,
    Open,
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
            selection: None,
            float: None,
            sel_drag: None,
            clipboard: None,
            tolerance: 0.25,
            mirror_x: false,
            mirror_y: false,
            palette_edit: false,
            export_dialog: None,
            cel_range: None,
            cel_anchor: None,
            cel_clip: None,
            tag_edit: None,
            rename_layer: None,
            play_step: 0,
            unsaved_ask: None,
            allow_close: false,
            image: image_ui::ImagePanel::default(),
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

    /// Undo/Redo: Schwebendes und Auswahl verwerfen — der Stand davor
    /// kommt ja gerade zurück.
    fn drop_selection(&mut self) {
        self.image.live = None;
        self.image.angle = 0.0;
        self.float = None;
        self.selection = None;
        self.sel_drag = None;
    }

    fn undo(&mut self) {
        self.drop_selection();
        let cur = self.project.current;
        if self.histories[cur].undo(&mut self.project.sprites[cur]) {
            self.changed();
        }
    }

    fn redo(&mut self) {
        self.drop_selection();
        let cur = self.project.current;
        if self.histories[cur].redo(&mut self.project.sprites[cur]) {
            self.changed();
        }
    }

    fn select_sprite(&mut self, i: usize) {
        if i < self.project.sprites.len() && i != self.project.current {
            self.finish_rotate();
            self.deselect();
            self.project.current = i;
            self.clamp_color();
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
        self.drop_selection();
        self.histories = project.sprites.iter().map(|_| History::default()).collect();
        self.project = project;
        self.path = path;
        self.dirty = false;
        self.stroke_last = None;
        self.fit_pending = true;
        self.version = self.version.wrapping_add(1);
    }

    /// Öffnen — bei ungespeicherten Änderungen erst nachfragen.
    fn open(&mut self) {
        if self.dirty {
            self.unsaved_ask = Some(Pending::Open);
        } else {
            self.open_now();
        }
    }

    fn open_now(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(tr("Projekt öffnen"))
            .add_filter(tr("spritebit-Projekt"), &[EXT, "json"])
            .add_filter(tr("Alle Dateien"), &["*"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read(&path) {
            Err(e) => self.error = Some(trf("{path} konnte nicht gelesen werden: {e}", &[("path", &path.display()), ("e", &e)])),
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
                    Err(e) => self.error = Some(i18n::io_error(&e)),
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
            .set_title(tr("Projekt speichern"))
            .add_filter(tr("spritebit-Projekt"), &[EXT])
            .set_file_name(name)
            .save_file()
        {
            let path = if path.extension().is_none() { path.with_extension(EXT) } else { path };
            self.write_native(&path);
        }
    }

    fn write_native(&mut self, path: &Path) {
        self.commit_float();
        match std::fs::write(path, save_native(&self.project)) {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.dirty = false;
            }
            Err(e) => self.error = Some(trf("{path} konnte nicht gespeichert werden: {e}", &[("path", &path.display()), ("e", &e)])),
        }
    }

    fn export_web(&mut self) {
        self.commit_float();
        let name = format!("{}.json", self.sprite().name);
        if let Some(path) = rfd::FileDialog::new()
            .set_title(tr("Als Web-Projekt exportieren"))
            .add_filter(tr("Web-Projekt (JSON)"), &["json"])
            .set_file_name(name)
            .save_file()
        {
            if let Err(e) = std::fs::write(&path, export_web(&self.project)) {
                self.error = Some(trf("{path} konnte nicht geschrieben werden: {e}", &[("path", &path.display()), ("e", &e)]));
            }
        }
    }

    /// Titelleiste: Dateiname und ein Sternchen bei ungespeicherten Änderungen.
    fn sync_title(&mut self, ctx: &egui::Context) {
        let file = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(|| tr("Unbenannt").to_string(), |n| n.to_string_lossy().into_owned());
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
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::E)) {
            self.open_export();
        }
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
            if prev || next || first || last {
                self.deselect();
            }
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
            ui.menu_button(tr("Datei"), |ui| {
                if ui.button(tr("Neuer Sprite …")).clicked() {
                    self.new_dialog = Some(NewDialog { width: self.sprite().width, height: self.sprite().height });
                }
                ui.separator();
                if ui.add(egui::Button::new(tr("Öffnen …")).shortcut_text(keys("Strg+O"))).clicked() {
                    self.open();
                }
                if ui.add(egui::Button::new(tr("Speichern")).shortcut_text(keys("Strg+S"))).clicked() {
                    self.save();
                }
                if ui.add(egui::Button::new(tr("Speichern unter …")).shortcut_text(keys("Strg+Umschalt+S"))).clicked() {
                    self.save_as();
                }
                ui.separator();
                if ui.add(egui::Button::new(tr("Exportieren …")).shortcut_text(keys("Strg+E"))).clicked() {
                    self.open_export();
                }
                if ui.button(tr("Als Web-Projekt exportieren …")).clicked() {
                    self.export_web();
                }
                ui.separator();
                if ui.add(egui::Button::new(tr("Beenden")).shortcut_text(keys("Alt+F4"))).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(tr("Bearbeiten"), |ui| {
                let (can_undo, can_redo) = (self.history().can_undo(), self.history().can_redo());
                if ui.add_enabled(can_undo, egui::Button::new(tr("Rückgängig")).shortcut_text(keys("Strg+Z"))).clicked() {
                    self.undo();
                }
                if ui.add_enabled(can_redo, egui::Button::new(tr("Wiederholen")).shortcut_text(keys("Strg+Y"))).clicked() {
                    self.redo();
                }
                ui.separator();
                let has = self.selection.is_some();
                if ui.add_enabled(has, egui::Button::new(tr("Ausschneiden")).shortcut_text(keys("Strg+X"))).clicked() {
                    self.cut_selection();
                }
                if ui.add_enabled(has, egui::Button::new(tr("Kopieren")).shortcut_text(keys("Strg+C"))).clicked() {
                    self.copy_selection();
                }
                let can_paste = self.clipboard.is_some();
                if ui.add_enabled(can_paste, egui::Button::new(tr("Einfügen")).shortcut_text(keys("Strg+V"))).clicked() {
                    self.paste_clipboard();
                }
                ui.separator();
                if ui.add(egui::Button::new(tr("Alles auswählen")).shortcut_text(keys("Strg+A"))).clicked() {
                    self.select_all();
                }
                if ui.add_enabled(has, egui::Button::new(tr("Auswahl aufheben")).shortcut_text(keys("Esc"))).clicked() {
                    self.deselect();
                }
                if ui.add_enabled(has, egui::Button::new(tr("Auswahl leeren")).shortcut_text(keys("Entf"))).clicked() {
                    self.delete_selection();
                }
            });
            ui.menu_button(tr("Ansicht"), |ui| {
                if ui.button(tr("Einpassen")).clicked() {
                    self.fit_pending = true;
                }
                if ui.button("100 %").clicked() {
                    self.zoom = 1.0;
                }
                ui.checkbox(&mut self.show_grid, tr("Gitter"));
                ui.separator();
                ui.menu_button(tr("Sprache"), |ui| {
                    for l in i18n::Lang::ALL {
                        if ui.radio(i18n::lang() == l, l.name()).clicked() {
                            i18n::choose(l);
                            ui.close();
                        }
                    }
                });
            });
            ui.menu_button(tr("Hilfe"), |ui| {
                if ui.button(tr("Über spritebit")).clicked() {
                    self.about_open = true;
                }
            });
        });
    }

    // ── Linke Leiste: Sprites und Farben ────────────────────────────
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.strong(tr("Sprites"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("+").on_hover_text(tr("Neuer Sprite")).clicked() {
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
        ui.strong(tr("Farben"));
        ui.add_space(4.0);
        self.colors_panel(ui);
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
            ui.label(trf("{tiles} Kacheln · {kib} KiB", &[("tiles", &tiles), ("kib", &kib)]));
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
        let (pressed, released, alt) = ui.input(|i| {
            (
                i.pointer.primary_pressed() || i.pointer.secondary_pressed(),
                i.pointer.primary_released() || i.pointer.secondary_released(),
                i.modifiers.alt,
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
            alt,
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
            // Schwebendes wird in eine Kopie eingesetzt und mitgezeichnet —
            // die Kopie teilt die Kacheln, nur die berührten werden kopiert.
            let with_float;
            let shown = match &self.float {
                Some(f) => {
                    let mut c = sp.clone();
                    selection::paste(c.active(), &f.clip, f.x, f.y);
                    with_float = c;
                    &with_float
                }
                None => sp,
            };
            let (mut buf, tw, th) = render_rgba_step(shown, &palette, sp.frame, rect, step);
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
        // Symmetrie-Achsen
        let axis = Stroke::new(1.0, Color32::from_rgba_unmultiplied(108, 158, 248, 160));
        if self.mirror_x {
            let x = origin.x + sw as f32 * zoom / 2.0;
            painter.line_segment([Pos2::new(x, origin.y), Pos2::new(x, origin.y + sh as f32 * zoom)], axis);
        }
        if self.mirror_y {
            let y = origin.y + sh as f32 * zoom / 2.0;
            painter.line_segment([Pos2::new(origin.x, y), Pos2::new(origin.x + sw as f32 * zoom, y)], axis);
        }
        // Form, die gerade aufgezogen wird, und der Rahmen der Auswahl.
        self.shape_preview(&painter, origin, zoom);
        self.selection_overlay(&painter, origin, zoom, (x0 as i64, y0 as i64, x1 as i64, y1 as i64));
        // Rand der Fläche.
        let border = egui::Rect::from_min_size(origin, Vec2::new(sw as f32, sh as f32) * zoom);
        painter.rect_stroke(border, 0.0, Stroke::new(1.0, Color32::from_gray(70)), egui::StrokeKind::Outside);
    }

    // ── Dialoge ─────────────────────────────────────────────────────
    fn dialogs(&mut self, ctx: &egui::Context) {
        let mut create = None;
        let mut close = false;
        if let Some(d) = &mut self.new_dialog {
            egui::Window::new(tr("Neuer Sprite")).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Breite"));
                    ui.add(egui::DragValue::new(&mut d.width).range(1..=MAX_SIDE));
                    ui.label(tr("Höhe"));
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
                    if ui.button(tr("Anlegen")).clicked() {
                        create = Some((d.width, d.height));
                    }
                    if ui.button(tr("Abbrechen")).clicked() {
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
            egui::Window::new(tr("Über spritebit"))
                .collapsible(false)
                .resizable(false)
                .open(&mut self.about_open)
                .show(ctx, |ui| {
                    ui.label(tr("spritebit — Pixel-Art-Editor"));
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.label("© 2026 Marco Jan");
                });
        }
        let mut dismiss = false;
        if let Some(msg) = &self.error {
            egui::Window::new(tr("Hinweis")).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(msg);
                if ui.button(tr("OK")).clicked() {
                    dismiss = true;
                }
            });
        }
        if dismiss {
            self.error = None;
        }
        self.unsaved_dialog(ctx);
    }

    /// Fenster schließen abfangen, solange etwas ungespeichert ist.
    fn guard_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.unsaved_ask = Some(Pending::Close);
        }
    }

    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.unsaved_ask else { return };
        let (mut save, mut discard, mut cancel) = (false, false, false);
        egui::Modal::new(egui::Id::new("unsaved")).show(ctx, |ui| {
            ui.heading(tr("Ungespeicherte Änderungen"));
            ui.label(match pending {
                Pending::Close => tr("Vor dem Beenden speichern?"),
                Pending::Open => tr("Vor dem Öffnen eines anderen Projekts speichern?"),
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                save = ui.button(tr("Speichern")).clicked();
                discard = ui.button(tr("Nicht speichern")).clicked();
                cancel = ui.button(tr("Abbrechen")).clicked();
            });
        });
        if cancel {
            self.unsaved_ask = None;
            return;
        }
        if save {
            self.save();
            if self.dirty {
                // Speichern abgebrochen oder fehlgeschlagen: nichts verwerfen.
                self.unsaved_ask = None;
                return;
            }
        }
        if save || discard {
            self.unsaved_ask = None;
            match pending {
                Pending::Close => {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Pending::Open => self.open_now(),
            }
        }
    }
}

impl eframe::App for SpritebitApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.guard_close(&ctx);
        self.shortcuts(&ctx);
        self.selection_keys(&ctx);
        self.tool_keys(&ctx);
        self.advance_playback(&ctx);
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("tools").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("timeline").resizable(true).default_size(150.0).show(ui, |ui| self.timeline(ui));
        egui::Panel::left("side").resizable(true).default_size(180.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
        });
        egui::Panel::right("panels").resizable(true).default_size(230.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.right_panels(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
        self.dialogs(&ctx);
        self.export_window(&ctx);
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
        let mut h = Harness::builder().with_size(Vec2::new(1280.0, 800.0)).build_eframe(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            SpritebitApp::new()
        });
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
    fn auswahl_verschieben_und_rueckgaengig() {
        let mut h = app();
        drag(&mut h, (10.0, 10.0), (11.0, 10.0)); // zwei Pixel malen
        h.key_press(Key::A);
        h.run();
        drag(&mut h, (9.0, 9.0), (12.0, 11.0)); // Auswahl um die Pixel
        assert!(h.state().selection.is_some());
        drag(&mut h, (10.0, 10.0), (30.0, 20.0)); // darin ziehen = verschieben
        h.key_press(Key::Escape); // absetzen
        h.run();
        assert_eq!(px(&h, 10, 10), 0, "alte Stelle leer");
        assert_eq!(px(&h, 30, 20), 5);
        assert_eq!(px(&h, 31, 20), 5);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run();
        assert_eq!(px(&h, 10, 10), 5, "Undo holt sie zurück");
        assert_eq!(px(&h, 30, 20), 0);
    }

    #[test]
    fn kopieren_und_einfuegen() {
        let mut h = app();
        drag(&mut h, (2.0, 2.0), (3.0, 2.0));
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        h.run();
        h.key_press_modifiers(Modifiers::COMMAND, Key::C);
        h.run();
        h.key_press(Key::Escape);
        h.run();
        h.key_press_modifiers(Modifiers::COMMAND, Key::V);
        h.run();
        assert!(h.state().float.is_some(), "Eingefügtes schwebt");
        // eins nach rechts schieben und absetzen
        h.key_press(Key::ArrowRight);
        h.run();
        h.key_press(Key::Escape);
        h.run();
        assert_eq!(px(&h, 4, 2), 5);
        assert_eq!(px(&h, 2, 2), 5, "Original bleibt");
    }

    #[test]
    fn zauberstab_loescht_die_flaeche() {
        let mut h = app();
        h.key_press(Key::F);
        h.run();
        let p = at(&h, 5.0, 5.0);
        h.hover_at(p);
        h.run();
        h.drag_at(p);
        h.run();
        h.drop_at(p);
        h.run();
        assert_eq!(px(&h, 0, 0), 5);
        h.key_press(Key::W);
        h.run();
        let p = at(&h, 5.0, 5.0);
        h.hover_at(p);
        h.run();
        h.drag_at(p);
        h.run();
        h.drop_at(p);
        h.run();
        assert_eq!(px(&h, 0, 0), 0);
        assert_eq!(px(&h, 63, 63), 0);
    }

    #[test]
    fn symmetrie_malt_gespiegelt() {
        let mut h = app();
        h.state_mut().mirror_x = true;
        drag(&mut h, (2.0, 5.0), (2.0, 5.0));
        assert_eq!(px(&h, 2, 5), 5);
        assert_eq!(px(&h, 61, 5), 5, "64 - 1 - 2");
    }

    #[test]
    fn pipette_mit_alt() {
        let mut h = app();
        h.state_mut().project.sprite_mut().active().set(7, 7, 3);
        let p = at(&h, 7.0, 7.0);
        h.hover_at(p);
        h.run();
        h.event_modifiers(
            egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::ALT },
            Modifiers::ALT,
        );
        h.run();
        h.event_modifiers(
            egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::ALT },
            Modifiers::ALT,
        );
        h.run();
        assert_eq!(h.state().color, 3);
        assert_eq!(px(&h, 7, 7), 3, "nichts übermalt");
    }

    #[test]
    fn freie_farbe_und_palette_bearbeiten() {
        let mut h = app();
        h.state_mut().set_rgb([1, 2, 3]);
        assert!(h.state().color >= spritebit_core::FREE_BASE);
        h.state_mut().set_rgb([0xff, 0xff, 0xff]);
        assert_eq!(h.state().color, 1, "Palettenfarbe wird erkannt");
    }

    #[test]
    fn rueckfrage_bei_ungespeichertem() {
        let mut h = app();
        h.state_mut().dirty = true;
        h.state_mut().unsaved_ask = Some(Pending::Close);
        h.run();
        h.get_by_label("Abbrechen").click();
        h.run();
        assert!(h.state().unsaved_ask.is_none());
        assert!(!h.state().allow_close, "Abbrechen schließt nicht");
        h.state_mut().unsaved_ask = Some(Pending::Close);
        h.run();
        h.get_by_label("Nicht speichern").click();
        h.run();
        assert!(h.state().allow_close);
    }

    #[test]
    fn sprache_umschalten() {
        i18n::set_lang(i18n::Lang::En);
        let mut h = app();
        h.get_by_label("File");
        h.get_by_label("Edit");
        i18n::set_lang(i18n::Lang::At);
        h.run();
        h.get_by_label("Bearbeitn");
        i18n::set_lang(i18n::Lang::De);
        h.run();
        h.get_by_label("Bearbeiten");
    }

    #[test]
    fn bild_spiegeln_ganzer_sprite_und_undo() {
        let mut h = app();
        h.state_mut().project.sprite_mut().active().set(0, 3, 5);
        h.get_by_label("↔ Spiegeln").click();
        h.run();
        assert_eq!(px(&h, 63, 3), 5);
        assert_eq!(px(&h, 0, 3), 0);
        h.state_mut().undo();
        assert_eq!(px(&h, 0, 3), 5);
    }

    #[test]
    fn bild_drehen_nur_die_auswahl() {
        let mut h = app();
        {
            let a = h.state_mut();
            a.project.sprite_mut().active().set(10, 10, 5);
            a.project.sprite_mut().active().set(40, 40, 3);
            a.selection = Some(Selection::rect(10, 10, 13, 11)); // 4 × 2
        }
        h.state_mut().rotate90();
        h.state_mut().deselect();
        // Mitte bleibt: 4×2 bei (10,10) → 2×4 bei (11,9); (0,0) landet oben rechts.
        assert_eq!(px(&h, 12, 9), 5);
        assert_eq!(px(&h, 40, 40), 3, "außerhalb unberührt");
    }

    #[test]
    fn zuschneiden_und_outline() {
        let mut h = app();
        h.state_mut().project.sprite_mut().active().set(20, 30, 5);
        h.get_by_label("Zuschneiden").click();
        h.run();
        assert_eq!((h.state().sprite().width, h.state().sprite().height), (1, 1));
        h.state_mut().undo();
        assert_eq!(h.state().sprite().width, 64);
        let n = h.state_mut().clean_for_test_outline();
        assert_eq!(n, 4);
    }

    #[test]
    fn neuer_frame_aus_der_timeline() {
        let mut h = app();
        h.get_by_label("Leerer Frame dahinter").click();
        h.run();
        assert_eq!(h.state().project.sprite().frames.len(), 2);
        assert_eq!(h.state().project.sprite().frame, 1);
    }
}
