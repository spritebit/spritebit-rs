//! spritebit — Desktop-App (egui/eframe).
//!
//! Menüleiste, Sprite-Liste und Farben links, Zeichenfläche mit Zoom und
//! Verschieben, Stift und Radierer, Undo je Sprite, Speichern und Öffnen
//! (eigenes Format und Projektdatei der Web-Version). Gezeichnet wird immer
//! nur der sichtbare Ausschnitt (`spritebit_core::render_rgba_step`) — die
//! Fläche darf darum bis 8192×8192 groß sein.

// Im Release kein Konsolenfenster neben dem Programm.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bitty_ui;
mod export_ui;
mod guides_ui;
mod i18n;
mod icons;
mod image_ui;
mod palette_ui;
mod preview_ui;
mod selection_ui;
mod selfupdate;
mod sprites_ui;
mod tabs;
mod update;
mod template_ui;
mod tiles_ui;
mod timeline;
mod tlmenu_ui;
mod tools_ui;
mod view_ui;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Sense, Stroke, Vec2};
use spritebit_core::{
    export_web, import_web, load_native, render_rgba_step, save_native, selection, History, Project, Px, Rect,
    Selection, Sprite,
};
use tools_ui::{Pointer, Tool};
use crate::i18n::{tr, trf, keys};

fn main() -> eframe::Result {
    i18n::load();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("spritebit {VERSION}"))
            // Fenster- und Taskleisten-Icon; das der .exe bettet build.rs ein.
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/app/spritebit-256.png")).expect("gültiges PNG"))
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
            let mut app = SpritebitApp::new();
            // Wie im Browser: die zuletzt geladene Schablone ist wieder da.
            app.restore_template(&cc.egui_ctx);
            app.load_tl_opts();
            app.load_view();
            app.load_guide_layouts();
            // Die abgelöste Datei vom letzten Update wegräumen (selfupdate.rs).
            selfupdate::cleanup_old();
            // Nach einem Absturz (oder einem Update): die Sitzung zurückholen.
            app.start_session();
            // Einmal bei GitHub nach einer neueren Version fragen (abschaltbar).
            app.start_update_check(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

/// Grenzen der Zoomstufe (Bildschirm-Pixel je Sprite-Pixel).
pub(crate) const ZOOM_MIN: f32 = 0.05;
pub(crate) const ZOOM_MAX: f32 = 64.0;
/// Ab dieser Zoomstufe werden Gitterlinien gezeichnet.
const GRID_FROM: f32 = 8.0;
/// Dateiendung des eigenen Projektformats. Früher „.spritebit“ — solche
/// Dateien öffnen sich weiter (OLD_EXT), gespeichert wird als .sb.
const EXT: &str = "sb";
const OLD_EXT: &str = "spritebit";
/// Ein einzelner Sprite: JSON wie in der Web-Version, dort genauso lesbar.
const SPRITE_EXT: &str = "bitty";
/// Version aus Cargo.toml — steht in Titelleiste, Hilfe und „Über“.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Unterstützen (Ko-fi) — fließt in ein Code-Signatur-Zertifikat. Wie im Web (js/site-links.js).
const DONATE_URL: &str = "https://ko-fi.com/baloou";

/// Die Textur der Zeichenfläche und wofür sie gerechnet wurde. Ändert sich
/// nichts davon, wird sie nicht neu gerechnet.
struct CanvasTexture {
    handle: egui::TextureHandle,
    /// Sprite, Ausschnitt, Schritt, Frame, Version, Stand der Licht-Vorschau.
    key: (usize, Rect, u32, usize, u64, u64),
}

/// Dialog „Neuer Sprite".
/// Hintergrund der Zeichenfläche — auch der aktive Reiter (tabs.rs) hat ihn.
pub(crate) const STAGE_BG: Color32 = Color32::from_rgb(0x14, 0x14, 0x18);

struct SpritebitApp {
    project: Project,
    /// Undo je Sprite, gleiche Reihenfolge wie `project.sprites`.
    histories: Vec<History>,
    /// Reiter der geöffneten Sprites (Stellen in `project.sprites`), siehe tabs.rs.
    tabs: Vec<usize>,
    /// Update-Hinweis (update.rs).
    update: update::UpdateState,
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
    sprite_dialog: Option<sprites_ui::SpriteDialog>,
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
    /// Stärke 1–100: Dichte bei Pinsel/Radierer, Menge beim Spray.
    strength: u32,
    /// Alt + rechte Maustaste ziehen verstellt gerade die Größe (tools_ui.rs).
    size_drag: Option<tools_ui::SizeDrag>,
    /// Pixel-perfekt (Stift, Radierer 1 px) und der Pfad des laufenden Strichs.
    pixel_perfect: bool,
    /// Füllen: Grenzen von allen sichtbaren Ebenen (wie im Web).
    fill_visible: bool,
    /// Umschalt beim Malen: wo die gerade Linie beginnt und ihre Richtung.
    stroke_lock: Option<tools_ui::StrokeLock>,
    /// Skalieren der Auswahl: unskaliertes Original, seine Maske und das
    /// letzte Ergebnis (gilt nur, solange das Schwebende noch so aussieht).
    scale_base: Option<(spritebit_core::selection::Clip, Option<Vec<bool>>, spritebit_core::selection::Clip)>,
    pp: Option<spritebit_core::tools::PixelPerfect>,
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
    /// Zwischenablage: das Stück und woher seine Farben stammen (Palette und
    /// freie Farben des Quell-Sprites) — siehe selection_ui.rs `paste_clipboard`.
    clipboard: Option<selection_ui::ClipSrc>,
    /// Toleranz von Farbwahl und Zauberstab, 0.0–1.0.
    tolerance: f64,
    /// Symmetrie: an der senkrechten (x) bzw. waagerechten (y) Mitte spiegeln.
    mirror_x: bool,
    mirror_y: bool,
    /// Farbzeile, Paletten-Bibliothek und ihre Dialoge.
    pal: palette_ui::PalState,
    preview: preview_ui::Preview,
    guides: guides_ui::GuideState,
    template: template_ui::TemplateState,
    /// Gedrückte Umschalt-/Alt-/Strg-Tasten in diesem Durchlauf.
    modifiers: Modifiers,
    /// Schachbrett unter den Pixeln (2 × 2, wiederholt).
    checker: Option<egui::TextureHandle>,
    /// Panel „Code & Export“ und der Import-Dialog.
    out: export_ui::OutState,
    /// Timeline-Einstellungen (für alle Sprites), ihr Fenster und Zwischenspeicher.
    tl: tlmenu_ui::TlOpts,
    tl_persist: bool,
    tl_menu_open: bool,
    tl_cache: tlmenu_ui::TlCache,
    /// Hintergrund, Vollbild, „Farbe zeigen“, Hilfe, Sitzungssicherung.
    view: view_ui::ViewState,
    /// Bitty, der Helfer: Blase, Suche, Hinweise (bitty_ui.rs).
    bitty: bitty_ui::BittyState,
    /// Welche Panels rechts offen sind — wird gemerkt (image_ui.rs).
    panels: image_ui::PanelMemory,
    /// In der Kopfzeile markierte Frames und der Ausgangspunkt für Umschalt+Klick.
    frame_sel: Vec<usize>,
    frame_anchor: Option<usize>,
    /// Frame oder Ebene, die gerade in der Timeline gezogen wird.
    tl_drag: Option<timeline::TlDrag>,
    /// Bereich in der Timeline (Shift-Klick) und sein Ausgangspunkt.
    cel_range: Option<spritebit_core::cels::CelRange>,
    cel_anchor: Option<(usize, usize)>,
    cel_clip: Option<(spritebit_core::cels::CelClip, spritebit_core::Palette, Vec<spritebit_core::Rgb>)>,
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
    /// Panel „Kacheln“ (Tilemap-Ebenen).
    tiles: tiles_ui::TileState,
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
            tabs: (0..project.sprites.len()).collect(),
            update: update::UpdateState::default(),
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
            sprite_dialog: None,
            about_open: false,
            error: None,
            hint: None,
            playing: false,
            frame_started: 0.0,
            onion: false,
            tool: Tool::Pencil,
            size: 1,
            strength: 100,
            size_drag: None,
            pixel_perfect: false,
            fill_visible: false,
            stroke_lock: None,
            scale_base: None,
            pp: None,
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
            pal: palette_ui::PalState::default(),
            preview: preview_ui::Preview::default(),
            guides: guides_ui::GuideState::default(),
            template: template_ui::TemplateState::default(),
            modifiers: Modifiers::NONE,
            checker: None,
            out: export_ui::OutState::default(),
            tl: tlmenu_ui::TlOpts::default(),
            tl_persist: false,
            tl_menu_open: false,
            tl_cache: tlmenu_ui::TlCache::default(),
            view: view_ui::ViewState::default(),
            bitty: bitty_ui::BittyState::default(),
            panels: image_ui::PanelMemory::load(),
            frame_sel: Vec::new(),
            frame_anchor: None,
            tl_drag: None,
            cel_range: None,
            cel_anchor: None,
            cel_clip: None,
            tag_edit: None,
            rename_layer: None,
            play_step: 0,
            unsaved_ask: None,
            allow_close: false,
            image: image_ui::ImagePanel::default(),
            tiles: tiles_ui::TileState::default(),
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
        if self.histories[cur].undo(&mut self.project.sprites[cur], &mut self.project.palettes) {
            self.changed();
        }
    }

    fn redo(&mut self) {
        self.drop_selection();
        let cur = self.project.current;
        if self.histories[cur].redo(&mut self.project.sprites[cur], &mut self.project.palettes) {
            self.changed();
        }
    }

    fn select_sprite(&mut self, i: usize) {
        if i < self.project.sprites.len() && i != self.project.current {
            self.finish_rotate();
            self.deselect();
            self.frame_sel.clear();
            self.project.current = i;
            self.clamp_color();
            self.stroke_last = None;
            self.fit_pending = true;
            self.version = self.version.wrapping_add(1);
        }
    }

    // ── Datei ───────────────────────────────────────────────────────
    fn replace_project(&mut self, project: Project, path: Option<PathBuf>) {
        self.drop_selection();
        self.histories = project.sprites.iter().map(|_| History::default()).collect();
        // Ein anderes Projekt: alle seine Sprites sind offen.
        self.tabs = (0..project.sprites.len()).collect();
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
            .add_filter(tr("spritebit-Projekt"), &[EXT, OLD_EXT, SPRITE_EXT, "json"])
            .add_filter(tr("Alle Dateien"), &["*"])
            .pick_file()
        else {
            return;
        };
        // Ein einzelner Sprite ersetzt das Projekt nicht, er kommt dazu — wie im Web.
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(SPRITE_EXT)) {
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()) {
                Ok(text) => match import_web(&text) {
                    Ok(p) => self.merge_project(p),
                    Err(e) => self.error = Some(i18n::io_error(&e)),
                },
                Err(e) => self.error = Some(trf("{path} konnte nicht gelesen werden: {e}", &[("path", &path.display()), ("e", &e)])),
            }
            return;
        }
        match std::fs::read(&path) {
            Err(e) => self.error = Some(trf("{path} konnte nicht gelesen werden: {e}", &[("path", &path.display()), ("e", &e)])),
            Ok(bytes) => {
                // Eigenes Format erkennt man am Anfang; alles andere wird als
                // Projektdatei der Web-Version versucht.
                let result = if bytes.starts_with(b"SPRITEBIT\0") {
                    load_native(&bytes).map(|p| (p, Some(path.clone())))
                } else {
                    // Aus einer Web-Datei wird beim Speichern eine .sb-Datei.
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

    /// Nur den aktuellen Sprite als .bitty sichern (JSON der Web-Version,
    /// dort genauso lesbar) — das Projekt bleibt, wie es ist.
    fn save_sprite_as(&mut self) {
        self.commit_float();
        let name = format!("{}.{SPRITE_EXT}", self.sprite().name);
        if let Some(path) = rfd::FileDialog::new()
            .set_title(tr("Sprite speichern"))
            .add_filter(tr("spritebit-Sprite"), &[SPRITE_EXT])
            .set_file_name(name)
            .save_file()
        {
            let path = if path.extension().is_none() { path.with_extension(SPRITE_EXT) } else { path };
            match std::fs::write(&path, spritebit_core::export_sprite(&self.project, self.project.current)) {
                Ok(()) => self.hint = Some(trf("Sprite gespeichert: {name}", &[("name", &path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()))])),
                Err(e) => self.error = Some(trf("{path} konnte nicht gespeichert werden: {e}", &[("path", &path.display()), ("e", &e)])),
            }
        }
    }

    /// Sprites aus einer Datei zum Projekt dazunehmen: .bitty, eine
    /// spritebit-Projektdatei oder ein Web-Projekt — ersetzt wird nichts
    /// (Project::merge).
    fn add_sprites(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(tr("Sprite hinzufügen"))
            .add_filter(tr("spritebit-Sprite oder -Projekt"), &[SPRITE_EXT, EXT, OLD_EXT, "json"])
            .add_filter(tr("Alle Dateien"), &["*"])
            .pick_file()
        else {
            return;
        };
        let other = match std::fs::read(&path) {
            Err(e) => {
                self.error = Some(trf("{path} konnte nicht gelesen werden: {e}", &[("path", &path.display()), ("e", &e)]));
                return;
            }
            Ok(bytes) if bytes.starts_with(b"SPRITEBIT\0") => load_native(&bytes),
            Ok(bytes) => std::str::from_utf8(&bytes)
                .map_err(|_| spritebit_core::IoError::NotAProject("kein Text".into()))
                .and_then(import_web),
        };
        match other {
            Ok(other) => self.merge_project(other),
            Err(e) => self.error = Some(i18n::io_error(&e)),
        }
    }

    /// Sprites eines anderen Projekts übernehmen: je ein frischer Verlauf,
    /// ein offener Reiter, der erste wird gewählt.
    fn merge_project(&mut self, other: Project) {
        let added = self.project.merge(other);
        let n = added.len();
        if n == 0 {
            return;
        }
        for i in added.clone() {
            self.histories.push(History::default());
            self.tabs.push(i);
        }
        self.select_sprite(added.start);
        self.dirty = true;
        self.version = self.version.wrapping_add(1);
        self.hint = Some(if n == 1 {
            trf("Sprite „{name}“ zum Projekt hinzugefügt.", &[("name", &self.sprite().name)])
        } else {
            trf("{n} Sprites zum Projekt hinzugefügt.", &[("n", &n)])
        });
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
        let title = format!("{}{} — spritebit {VERSION}", file, if self.dirty { " *" } else { "" });
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
                // Enter übernimmt eine laufende freie Drehung, sonst Abspielen.
                if self.image.live.is_some() {
                    self.finish_rotate();
                } else {
                    self.toggle_play(ctx);
                }
            }
            // Esc bei laufender Drehung: zurück auf 0°.
            if self.image.live.is_some() && ctx.input_mut(|i| i.consume_key(none, Key::Escape)) {
                self.cancel_rotate();
            }
            if ctx.input_mut(|i| i.consume_key(none, Key::F1)) {
                self.view.help_open = true;
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
                    self.open_new_sprite();
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
                // Einzelne Sprites statt des ganzen Projekts (.bitty)
                if ui.button(tr("Sprite speichern unter …")).clicked() {
                    self.save_sprite_as();
                }
                if ui.button(tr("Sprite hinzufügen …")).clicked() {
                    self.add_sprites();
                }
                ui.separator();
                if ui.add(egui::Button::new(tr("Exportieren …")).shortcut_text(keys("Strg+E"))).clicked() {
                    self.open_export();
                }
                if ui.button(tr("Als Web-Projekt exportieren …")).clicked() {
                    self.export_web();
                }
                ui.separator();
                if ui.button(tr("Alles zurücksetzen …")).clicked() {
                    self.sprite_dialog = Some(sprites_ui::SpriteDialog::Reset);
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
                self.view_menu(ui);
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
                self.help_menu(ui);
                self.bitty_menu(ui);
                ui.separator();
                self.update_menu(ui);
                ui.separator();
                if ui
                    .button(tr("spritebit unterstützen"))
                    .on_hover_text(tr("Kostenlos bleibt spritebit sowieso. Spenden fließen in ein Code-Signatur-Zertifikat, damit Windows bei der Desktop-App nicht mehr warnt."))
                    .clicked()
                {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(DONATE_URL));
                    ui.close();
                }
                if ui.button(tr("Über spritebit")).clicked() {
                    self.about_open = true;
                }
            });
            // Immer sichtbar, rechts in der Leiste (wie im Web): ganz rechts
            // Bitty, daneben Hintergrund und Vollbild. Im Menü „Ansicht“ gibt
            // es die beiden auch.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.bitty_button(ui);
                ui.add_space(8.0);
                self.view_quick(ui);
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
                    self.open_new_sprite();
                }
            });
        });
        self.sprite_list(ui);

        ui.add_space(10.0);
        ui.strong(tr("Farben"));
        ui.add_space(4.0);
        self.colors_panel(ui);
    }

    // ── Statusleiste ────────────────────────────────────────────────
    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (w, h) = (self.sprite().width, self.sprite().height);
            ui.label(format!("{w} × {h} px"));
            ui.separator();
            ui.label(tr("Zoom"));
            self.zoom_slider(ui);
            if self.view.spotlight {
                ui.separator();
                let n = self.count_current_color();
                ui.label(trf("{n} Pixel in dieser Farbe", &[("n", &n)]));
            }
            if let Some((x, y)) = self.hover {
                ui.separator();
                ui.label(format!("({x}, {y})"));
            }
            ui.separator();
            let tiles: usize = self.sprite().images.iter().map(|i| i.allocated_tiles()).sum();
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
        // Licht-Vorschau: nur, solange das Licht-Panel in diesem Durchlauf
        // gezeichnet wurde (es setzt light_open, die Fläche kommt danach).
        self.update_light_preview();
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
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.pointer.hover_pos(),
                i.pointer.primary_down(),
                i.pointer.secondary_down(),
                i.pointer.middle_down(),
                i.key_down(Key::Space),
                i.pointer.delta(),
            )
        });
        let (pressed, released, alt, secondary_pressed) = ui.input(|i| {
            (
                i.pointer.primary_pressed() || i.pointer.secondary_pressed(),
                i.pointer.primary_released() || i.pointer.secondary_released(),
                i.modifiers.alt,
                i.pointer.secondary_pressed(),
            )
        });
        // Wie im Web: Mausrad scrollt (Umschalt: waagerecht), Strg+Mausrad
        // zoomt um den Zeiger.
        if let Some(at) = pointer.filter(|p| area.contains(*p)) {
            if zoom_delta != 1.0 {
                self.zoom_at(zoom_delta, at, area);
            } else if scroll != Vec2::ZERO {
                self.pan += scroll;
            }
        }
        // Hand auf einer Hilfslinie zieht die Linie, nicht die Ansicht (guides_ui.rs).
        let panning = middle || (space && primary) || (self.tool == Tool::Pan && primary && self.guides.drag.is_none());
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
            pos: pointer,
        };
        // Umschalt+Alt gehört der Schablone, im Hilfslinien-Modus gehört
        // der Zeiger den Linien.
        // Alt + rechte Maustaste ziehen: Größe verstellen statt radieren.
        if !self.size_drag_pointer(pointer, &p, secondary_pressed)
            && !self.template_pointer(pointer, pressed, released, p.over, origin, zoom)
            && !self.guide_pointer(pointer, pressed, released, p.over, origin, zoom)
        {
            // Tilemap im Modus „Kacheln“: setzen statt malen (tiles_ui.rs).
            if self.tile_mode_on() && !p.panning && self.tool != Tool::Pan {
                self.use_tile_tool(&p);
            } else {
                self.use_tool(&p, ui.ctx());
            }
        }

        // Sichtbarer Ausschnitt in Sprite-Pixeln.
        let (sw, sh) = (self.sprite().width, self.sprite().height);
        let lo = (area.min - origin) / zoom;
        let hi = (area.max - origin) / zoom;
        let x0 = lo.x.floor().clamp(0.0, sw as f32) as u32;
        let y0 = lo.y.floor().clamp(0.0, sh as f32) as u32;
        let x1 = hi.x.ceil().clamp(0.0, sw as f32) as u32;
        let y1 = hi.y.ceil().clamp(0.0, sh as f32) as u32;
        painter.rect_filled(area, 0.0, STAGE_BG);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let rect = Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
        // Herausgezoomt: nur jedes n-te Pixel, sonst wäre die Textur größer
        // als der Bildschirm.
        let step = if zoom < 1.0 { (1.0 / zoom).floor() as u32 } else { 1 };
        let key = (self.project.current, rect, step, self.sprite().frame, self.version, self.image.preview_gen);
        if self.texture.as_ref().map(|t| t.key) != Some(key) {
            let palette = self.project.current_palette();
            let sp = self.sprite();
            // Schwebendes wird in eine Kopie eingesetzt und mitgezeichnet —
            // die Kopie teilt die Kacheln, nur die berührten werden kopiert.
            // Die Licht-Vorschau ist ebenso eine Kopie (nur dieser Frame, Frame 0).
            let with_float;
            let (shown, frame) = match (&self.float, &self.image.preview) {
                (Some(f), _) => {
                    let mut c = sp.clone();
                    selection::paste(c.active(), &f.clip, f.x, f.y);
                    with_float = c;
                    (&with_float, sp.frame)
                }
                (None, Some((_, pv))) => (pv, 0),
                (None, None) => (sp, sp.frame),
            };
            let (buf, tw, th) = render_rgba_step(shown, &palette, frame, rect, step);
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
        // Onion Skin: eigene Textur, hinter oder vor den Pixeln.
        let onion_id = self.onion_texture(ui.ctx(), rect, step);
        // „Farbe zeigen“: Maske über allem, was nicht die aktuelle Farbe ist.
        let spot_id = self.spotlight_texture(ui.ctx(), rect, step);
        let checker_colors = self.checker_colors();
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
            // Schachbrett (ein Feld je Textur-Pixel), dann die Schablone, dann die Pixel.
            let checker = self.checker.get_or_insert_with(|| {
                let [a, b] = checker_colors.map(|[r, g, b]| Color32::from_rgb(r, g, b));
                let img = egui::ColorImage::new([2, 2], vec![a, b, b, a]);
                let opts = egui::TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..egui::TextureOptions::NEAREST };
                ui.ctx().load_texture("checker", img, opts)
            });
            let k = 2.0 * step as f32;
            let cuv = egui::Rect::from_min_max(Pos2::new(x0 as f32 / k, y0 as f32 / k), Pos2::new(ex as f32 / k, ey as f32 / k));
            painter.image(checker.id(), screen, cuv, Color32::WHITE);
            self.draw_template(&painter.with_clip_rect(screen.intersect(area)), origin, zoom, false);
            let onion_front = self.tl.onion.front;
            if let (Some(id), false) = (onion_id, onion_front) {
                painter.image(id, screen, uv, Color32::WHITE);
            }
            painter.image(t.handle.id(), screen, uv, Color32::WHITE);
            if let (Some(id), true) = (onion_id, onion_front) {
                painter.image(id, screen, uv, Color32::WHITE);
            }
            if let Some(id) = spot_id {
                painter.image(id, screen, uv, Color32::WHITE);
            }
            self.draw_template(&painter, origin, zoom, true);
        }

        // Gitterlinien, nur stark hineingezoomt und nur im Ausschnitt.
        if self.show_grid && zoom >= GRID_FROM {
            let line = Stroke::new(1.0, self.grid_color());
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
        self.draw_guides(&painter, origin, zoom);
        self.draw_tiles(&painter.with_clip_rect(area), origin, zoom);
        // Form, die gerade aufgezogen wird, und der Rahmen der Auswahl.
        self.shape_preview(&painter, origin, zoom);
        // Maske bearbeiten: Ausgeblendetes rötlich, roter Rahmen.
        if self.sprite().editing_mask() {
            let sp = self.sprite();
            let m = sp.layers[sp.layer].mask.as_ref().expect("editing_mask prüft das");
            let red = Color32::from_rgba_unmultiplied(255, 70, 70, 120);
            let clip = painter.with_clip_rect(area);
            for (x, y, _) in m.hide.pixels() {
                if x < x0 || y < y0 || x >= x1 || y >= y1 {
                    continue;
                }
                let r = egui::Rect::from_min_size(origin + Vec2::new(x as f32, y as f32) * zoom, Vec2::splat(zoom));
                clip.rect_filled(r, 0.0, red);
            }
            let frame = egui::Rect::from_min_size(origin, Vec2::new(sw as f32, sh as f32) * zoom).expand(3.0);
            painter.rect_stroke(frame, 0.0, Stroke::new(2.0, Color32::from_rgb(255, 107, 107)), egui::StrokeKind::Outside);
        }
        // Umriss dessen, was Stift, Pinsel, Radierer oder Spray gleich treffen.
        if !self.tile_mode_on() {
            self.brush_preview(&painter.with_clip_rect(area), origin, zoom);
        }
        self.selection_overlay(&painter, origin, zoom, (x0 as i64, y0 as i64, x1 as i64, y1 as i64));
        // Rand der Fläche.
        let border = egui::Rect::from_min_size(origin, Vec2::new(sw as f32, sh as f32) * zoom);
        painter.rect_stroke(border, 0.0, Stroke::new(1.0, Color32::from_gray(70)), egui::StrokeKind::Outside);
    }

    // ── Dialoge ─────────────────────────────────────────────────────
    fn dialogs(&mut self, ctx: &egui::Context) {
        self.sprite_dialogs(ctx);
        self.palette_dialogs(ctx);
        if self.about_open {
            egui::Window::new(tr("Über spritebit"))
                .collapsible(false)
                .resizable(false)
                .open(&mut self.about_open)
                .show(ctx, |ui| {
                    ui.label(tr("spritebit — Pixel-Art-Editor"));
                    ui.strong(format!("Version {VERSION}"));
                    ui.label(tr("© 2026 Marco Jan · freie Software unter der MIT-Lizenz"));
                    ui.hyperlink_to(tr("Neue Versionen auf GitHub"), "https://github.com/spritebit/spritebit-rs/releases");
                    ui.add_space(6.0);
                    ui.hyperlink_to(tr("spritebit unterstützen"), DONATE_URL)
                        .on_hover_text(tr("Kostenlos bleibt spritebit sowieso. Spenden fließen in ein Code-Signatur-Zertifikat, damit Windows bei der Desktop-App nicht mehr warnt."));
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
        self.tab_keys(&ctx);
        self.modifiers = ctx.input(|i| i.modifiers);
        self.view_keys(&ctx);
        self.guide_keys(&ctx);
        self.selection_keys(&ctx);
        // Vor den Werkzeugen: Strg+K gehört Bitty, K allein der Farbwahl.
        self.bitty_keys(&ctx);
        self.tool_keys(&ctx);
        self.advance_playback(&ctx);
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        self.poll_update(&ctx);
        self.poll_install();
        if self.update.available.is_some() || !matches!(self.update.install, selfupdate::Install::Idle) {
            // Auffällig: farbige Fläche statt grau in grau (update.rs banner_frame).
            let frame = self.banner_frame();
            egui::Panel::top("update").frame(frame).show(ui, |ui| self.update_banner(ui));
        }
        egui::Panel::top("tools").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        // Die Timeline dockt dort an, wo man sie im ⚙-Menü hinstellt.
        match self.tl.zone {
            tlmenu_ui::Zone::Bottom => egui::Panel::bottom("timeline").resizable(true).default_size(150.0).show(ui, |ui| self.timeline(ui)),
            tlmenu_ui::Zone::Top => egui::Panel::top("timeline-top").resizable(true).default_size(150.0).show(ui, |ui| self.timeline(ui)),
            tlmenu_ui::Zone::Left => egui::Panel::left("timeline-left").resizable(true).default_size(360.0).show(ui, |ui| self.timeline(ui)),
            tlmenu_ui::Zone::Right => egui::Panel::right("timeline-right").resizable(true).default_size(360.0).show(ui, |ui| self.timeline(ui)),
        };
        egui::Panel::left("side").resizable(true).default_size(180.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
        });
        egui::Panel::right("panels").resizable(true).default_size(230.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.right_panels(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| {
            // Reiter direkt auf der Zeichenfläche, ohne Spalt dazwischen.
            ui.spacing_mut().item_spacing.y = 0.0;
            self.sprite_tabs(ui);
            self.canvas(ui);
        });
        // Nach einem Strich (Maus los): Kachelsätze abgleichen.
        let down = ctx.input(|i| i.pointer.any_down());
        self.tile_sync(down);
        self.dialogs(&ctx);
        self.notes_window(&ctx);
        self.import_window(&ctx);
        self.tl_menu(&ctx);
        self.help_window(&ctx);
        self.bitty_bubble(&ctx);
        self.autosave(&ctx);
        // Sauber beendet (ohne oder nach der Rückfrage): die Sitzung ist erledigt.
        if ctx.input(|i| i.viewport().close_requested()) && (!self.dirty || self.allow_close) {
            self.end_session();
        }
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
        let mut h = Harness::builder().with_size(Vec2::new(1280.0, 2400.0)).build_eframe(|cc| {
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
    fn panel_merkt_sich_auf_und_zu() {
        let mut h = app();
        assert_eq!(h.state().panel_open("p-light"), None, "noch nichts geändert");
        h.get_by_label("Licht").click();
        h.run();
        assert_eq!(h.state().panel_open("p-light"), Some(true));
        h.get_by_label("Licht").click();
        h.run();
        assert_eq!(h.state().panel_open("p-light"), Some(false));
    }

    #[test]
    fn was_ist_neu_zeigt_die_notizen_im_band() {
        let mut h = app();
        h.state_mut().update.available = Some("9.9.9".into());
        h.state_mut().update.next_notes = update::NextNotes::Ready("## Deutsch\n- Neuer Pinsel\n".into());
        h.run();
        h.get_by_label("Was ist neu?").click();
        h.run();
        assert!(h.query_by_label("Neu in spritebit 9.9.9").is_some());
        assert!(h.query_by_label("Neuer Pinsel").is_some());
        h.get_by_label("OK").click();
        h.run();
        assert_eq!(h.state().update.notes_view, None);
        // Ohne Notizen (kein Netz): das Fenster sagt es, statt leer zu sein.
        h.state_mut().update.next_notes = update::NextNotes::Failed;
        h.state_mut().update.notes_view = Some(update::NotesView::Next);
        h.run();
        assert!(h.query_by_label("Zu dieser Version gibt es keine Notizen.").is_some());
    }

    #[test]
    fn bitty_bietet_entsperren_an() {
        let mut h = app();
        h.state_mut().project.sprite_mut().layers[0].locked = true;
        drag(&mut h, (10.0, 10.0), (12.0, 10.0));
        assert_eq!(h.state().bitty.hint_id(), Some("layerLocked"));
        h.get_by_label("Entsperren").click();
        h.run();
        assert!(!h.state().project.sprite().layers[0].locked);
        assert_eq!(h.state().bitty.hint_id(), None);
        // Einmal pro Sitzung: wieder sperren und malen — kein zweiter Hinweis.
        h.state_mut().project.sprite_mut().layers[0].locked = true;
        drag(&mut h, (10.0, 10.0), (12.0, 10.0));
        assert_eq!(h.state().bitty.hint_id(), None);
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
    fn kacheln_malen_setzen_und_mitziehen() {
        use spritebit_core::tilemap::Tileset;
        let mut h = app();
        h.state_mut().project.sprite_mut().layers[0].tileset = Some(Tileset::new(8, 8));
        h.state_mut().tool = Tool::Pencil;
        let n_tiles = |h: &Harness<'_, SpritebitApp>| h.state().sprite().layers[0].tileset.as_ref().unwrap().tiles.len();
        // Pixel malen in eine leere Zelle (Auto): neue Kachel.
        drag(&mut h, (1.0, 1.0), (1.0, 1.0));
        assert_eq!(n_tiles(&h), 1);
        // Kacheln setzen: Kachel 1 in die Zelle (2, 0).
        h.state_mut().tiles.mode = tiles_ui::TileMode::Tiles;
        h.state_mut().tiles.tile = 1;
        drag(&mut h, (17.0, 1.0), (17.0, 1.0));
        assert_eq!(px(&h, 17, 1), 5, "gesetzt");
        assert_eq!(n_tiles(&h), 1, "Setzen legt keine Kachel an");
        // Pixel malen in der Kopie: das Original zieht mit.
        h.state_mut().tiles.mode = tiles_ui::TileMode::Pixel;
        drag(&mut h, (20.0, 4.0), (20.0, 4.0));
        assert_eq!(px(&h, 4, 4), 5, "Original mitgezogen");
        assert_eq!(n_tiles(&h), 1);
        // Ein Undo nimmt Strich und Mitziehen zusammen zurück.
        h.state_mut().undo();
        h.run();
        assert_eq!((px(&h, 4, 4), px(&h, 20, 4)), (0, 0));
        // Rechtsklick im Modus „Kacheln“ leert die Zelle.
        h.state_mut().tiles.mode = tiles_ui::TileMode::Tiles;
        let a = at(&h, 17.0, 1.0);
        h.hover_at(a);
        h.run();
        h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE });
        h.run();
        h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE });
        h.run();
        assert_eq!(px(&h, 17, 1), 0, "geleert");
        assert_eq!(px(&h, 1, 1), 5, "die andere Stelle bleibt");
    }

    #[test]
    fn hilfslinien_gleichmaessig_verteilen() {
        let mut h = app();
        h.state_mut().set_even(true, 3);
        h.state_mut().set_even(false, 1);
        let (w, hh) = (h.state().sprite().width, h.state().sprite().height);
        assert_eq!(h.state().sprite().guides.h, spritebit_core::Guides::even_lines(3, hh));
        assert_eq!(h.state().sprite().guides.v, vec![w / 2]);
        assert!(h.state().guides.show);
        h.state_mut().set_even(true, 0);
        assert!(h.state().sprite().guides.h.is_empty());
    }

    #[test]
    fn hilfslinien_layout_speichern_und_anwenden() {
        let mut h = app();
        let (w, hh) = (h.state().sprite().width, h.state().sprite().height);
        {
            let g = h.state_mut().guides_mut();
            g.h = vec![8];
            g.v = vec![16];
        }
        h.state_mut().save_guide_layout(" Held ");
        assert_eq!(h.state().guides.layouts.len(), 1);
        assert_eq!(h.state().guides.layouts[0].name, "Held");
        // Gleicher Name ersetzt.
        h.state_mut().guides_mut().v = vec![20];
        h.state_mut().save_guide_layout("Held");
        assert_eq!(h.state().guides.layouts.len(), 1);
        // Andere Linien, dann anwenden: die gespeicherten sind zurück.
        h.state_mut().guides_mut().h.clear();
        h.state_mut().apply_guide_layout(0);
        assert_eq!(h.state().sprite().guides.h, vec![8]);
        assert_eq!(h.state().sprite().guides.v, vec![20]);
        // JSON hin und zurück (Format wie im Web).
        let list = guides_ui::parse_layouts(&guides_ui::layouts_json(&h.state().guides.layouts));
        assert_eq!(list, h.state().guides.layouts);
        assert_eq!((list[0].width, list[0].height), (w, hh));
    }

    #[test]
    fn dunkel_hell_und_vollbild_immer_in_der_leiste() {
        let mut h = app();
        assert!(!h.state().view.light);
        h.get_by_label("Hell").click();
        h.run();
        assert!(h.state().view.light, "Hell");
        h.get_by_label("Dunkel").click();
        h.run();
        assert!(!h.state().view.light, "Dunkel");
        h.get_by_label("Vollbild").click();
        h.run();
        assert!(h.state().view.fullscreen);
        h.get_by_label("Vollbild beenden").click();
        h.run();
        assert!(!h.state().view.fullscreen);
    }

    #[test]
    fn einfuegen_in_andere_palette_behaelt_die_farben() {
        use spritebit_core::selection::rgb_of;
        let mut h = app();
        h.state_mut().project.sprite_mut().palette = "graustufen".into();
        for x in 0..4u32 {
            h.state_mut().project.sprite_mut().active().set(x, 0, x as u16 + 1);
        }
        let before: Vec<_> = {
            let s = h.state();
            let pal = s.project.current_palette();
            (0..4).map(|x| rgb_of(s.sprite().cel(0, 0).get(x, 0), &pal, &s.sprite().free)).collect()
        };
        h.state_mut().selection = Some(spritebit_core::selection::Selection::rect(0, 0, 3, 0));
        h.state_mut().copy_selection();
        h.state_mut().create_sprite("Bunt".into(), "golden".into(), 16, 16);
        h.run();
        assert_eq!(h.state().sprite().palette, "golden");
        h.state_mut().paste_clipboard();
        h.state_mut().deselect();
        let s = h.state();
        let pal = s.project.current_palette();
        let after: Vec<_> = (0..4).map(|x| rgb_of(s.sprite().cel(0, 0).get(x, 0), &pal, &s.sprite().free)).collect();
        assert_eq!(after, before, "sieht aus wie im Original");
        assert!(s.hint.as_deref().is_some_and(|t| t.contains("Farben des Originals")));
    }

    #[test]
    fn umschalt_malt_gerade_linien() {
        let mut h = app();
        h.state_mut().tool = Tool::Pencil;
        let sh = Modifiers::SHIFT;
        let pts = [(2.0, 5.0), (4.0, 6.0), (6.0, 4.0), (9.0, 6.0), (12.0, 5.0)];
        let a = at(&h, pts[0].0, pts[0].1);
        h.hover_at(a);
        h.run();
        // Umschalt die ganze Zeit gehalten (event_modifiers ließe sie gleich wieder los).
        h.event(egui::Event::ModifiersChanged(sh));
        h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: sh });
        h.run();
        for (x, y) in &pts[1..] {
            h.event(egui::Event::PointerMoved(at(&h, *x, *y)));
            h.run();
        }
        let b = at(&h, 12.0, 5.0);
        h.event(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: sh });
        h.run();
        h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
        h.run();
        for x in 2..=12 {
            assert_eq!(px(&h, x, 5), 5, "gerade Linie bei x = {x}");
        }
        assert_eq!((px(&h, 4, 6), px(&h, 6, 4), px(&h, 9, 6)), (0, 0, 0), "keine Wackler");
    }

    #[test]
    fn fuellen_mit_grenzen_aus_allen_ebenen() {
        let mut h = app();
        // Ebene 1: senkrechte Linie bei x = 8 als Vorlage; gemalt wird auf Ebene 2.
        for y in 0..64 {
            h.state_mut().project.sprite_mut().active().set(8, y, 3);
        }
        h.state_mut().project.sprite_mut().add_layer(1, "Farbe");
        h.state_mut().tool = Tool::Fill;
        h.state_mut().fill_visible = true;
        drag(&mut h, (2.0, 2.0), (2.0, 2.0));
        let sp = h.state().sprite();
        assert_eq!(sp.cel(0, 1).get(2, 2), 5, "links der Linie gefüllt");
        assert_eq!(sp.cel(0, 1).get(7, 40), 5);
        assert_eq!(sp.cel(0, 1).get(10, 2), 0, "rechts der Linie nicht");
        assert_eq!(sp.cel(0, 0).get(2, 2), 0, "die Vorlage bleibt unberührt");
        // Ohne die Option füllt es die ganze (leere) Ebene 2.
        h.state_mut().fill_visible = false;
        h.state_mut().undo();
        h.run();
        drag(&mut h, (2.0, 2.0), (2.0, 2.0));
        assert_eq!(h.state().sprite().cel(0, 1).get(10, 2), 5);
    }

    #[test]
    fn auswahl_mit_anfasser_skalieren() {
        let mut h = app();
        for y in 12..16 {
            for x in 12..16 {
                h.state_mut().project.sprite_mut().active().set(x, y, ((x + y) % 2 + 2) as u16);
            }
        }
        h.state_mut().tool = Tool::Select;
        h.state_mut().selection = Some(spritebit_core::selection::Selection::rect(12, 12, 15, 15));
        h.run();
        // at() zielt auf Zellmitten: 15.5 + 0.5 liegt auf der Ecke (16, 16).
        drag(&mut h, (15.5, 15.5), (19.5, 19.5));
        let s = h.state().selection.clone().unwrap();
        assert_eq!((s.x, s.y, s.w, s.h), (12, 12, 8, 8));
        assert_eq!(h.state().float.as_ref().map(|f| (f.clip.w, f.clip.h)), Some((8, 8)));
        h.state_mut().deselect();
        assert_eq!((px(&h, 12, 12), px(&h, 13, 12), px(&h, 14, 12)), (2, 2, 3), "jedes Pixel verdoppelt");
        assert_eq!(px(&h, 19, 19), 2);
    }

    #[test]
    fn hand_zieht_hilfslinie() {
        let mut h = app();
        h.state_mut().tool = Tool::Pan;
        h.state_mut().project.sprite_mut().guides.v = vec![10];
        h.state_mut().guides.show = true;
        let pan = h.state().pan;
        // at() zielt auf die Zellmitte: 9.5 + 0.5 liegt genau auf der Linie bei x = 10.
        drag(&mut h, (9.5, 5.0), (14.5, 5.0));
        assert_eq!(h.state().sprite().guides.v, vec![15], "Linie mitgezogen");
        assert_eq!(h.state().pan, pan, "Ansicht nicht verschoben");
        // Neben einer Linie verschiebt die Hand wie gewohnt.
        drag(&mut h, (30.0, 30.0), (34.0, 30.0));
        assert_ne!(h.state().pan, pan);
        assert_eq!(h.state().sprite().guides.v, vec![15]);
    }

    #[test]
    fn groesse_mit_alt_und_rechts_ziehen() {
        let mut h = app();
        h.state_mut().tool = Tool::Brush;
        h.state_mut().size = 3;
        // Etwas Gemaltes unter dem Zug — Alt + Rechts darf es nicht radieren.
        for x in 4..12 {
            h.state_mut().project.sprite_mut().active().set(x, 8, 3);
        }
        let a = at(&h, 5.0, 8.0);
        let alt = Modifiers::ALT;
        let btn = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed, modifiers: alt };
        h.hover_at(a);
        h.run();
        h.event_modifiers(btn(a, true), alt);
        h.run();
        // 36 Bildschirmpixel nach rechts (je 6 px eine Stufe), in kleinen Schritten.
        for k in 1..=6 {
            h.event_modifiers(egui::Event::PointerMoved(a + Vec2::new(6.0 * k as f32, 0.0)), alt);
            h.run();
        }
        assert_eq!(h.state().size, 9, "3 + 6 Stufen (36 px, je 6 px)");
        // Weit nach links: nie unter 1.
        h.event_modifiers(egui::Event::PointerMoved(a - Vec2::new(400.0, 0.0)), alt);
        h.run();
        assert_eq!(h.state().size, 1);
        h.event_modifiers(btn(a - Vec2::new(400.0, 0.0), false), alt);
        h.run();
        assert!(h.state().size_drag.is_none(), "Loslassen beendet das Ziehen");
        for x in 4..12 {
            assert_eq!(px(&h, x, 8), 3, "nichts radiert bei x = {x}");
        }
        assert_eq!(h.state().color, 5, "keine Pipette (sonst wäre es 3)");
    }

    #[test]
    fn staerke_ist_anfangs_100_prozent() {
        assert_eq!(app().state().strength, 100);
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
    fn reiter_der_geoeffneten_sprites() {
        let mut h = app();
        for n in ["Held", "Baum"] {
            h.state_mut().create_sprite(n.into(), "graustufen".into(), 16, 16);
            h.run(); // jedes Anlegen ist ein eigener Klick, also ein eigener Frame
        }
        assert_eq!(h.state().tabs, vec![0, 1, 2]);
        assert_eq!(h.state().project.current, 2);
        // Klick auf den Reiter wechselt.
        h.get_by_label("Held").click();
        h.run();
        assert_eq!(h.state().project.current, 1);
        // Strg+Tab rundum weiter, Strg+Umschalt+Tab zurück.
        h.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
        h.run();
        assert_eq!(h.state().project.current, 2);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
        h.run();
        assert_eq!(h.state().project.current, 0);
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Tab);
        h.run();
        assert_eq!(h.state().project.current, 2);
        // Strg+W schließt den Reiter, der Sprite bleibt; der linke wird aktiv.
        h.key_press_modifiers(Modifiers::COMMAND, Key::W);
        h.run();
        assert_eq!(h.state().tabs, vec![0, 1]);
        assert_eq!(h.state().project.current, 1);
        assert_eq!(h.state().project.sprites.len(), 3);
        // Duplizieren schiebt die Reiter dahinter mit.
        h.state_mut().tabs = vec![2, 1, 0];
        h.state_mut().duplicate_sprite(0);
        h.run();
        assert_eq!(h.state().tabs, vec![3, 2, 0, 1], "Kopie an Stelle 1, Reiter hinten dran");
        // Löschen nimmt den Reiter mit.
        h.state_mut().delete_sprite(3);
        h.run();
        assert_eq!(h.state().tabs, vec![2, 0, 1]);
        // Der letzte Reiter lässt sich nicht schließen.
        h.state_mut().tabs = vec![h.state().project.current];
        h.key_press_modifiers(Modifiers::COMMAND, Key::W);
        h.run();
        assert_eq!(h.state().tabs.len(), 1);
    }

    #[test]
    fn maske_auf_gesperrter_licht_ebene() {
        let mut h = app();
        for y in 10..14 {
            for x in 10..14 {
                h.state_mut().project.sprite_mut().active().set(x, y, 3);
            }
        }
        h.state_mut().image.cast_on = false;
        h.state_mut().commit_light_layers();
        h.run();
        let li = h.state().project.sprite().layers.iter().position(|l| l.fx.is_some()).unwrap();
        h.state_mut().project.sprite_mut().layer = li;
        h.run();
        assert!(h.state().project.sprite().layers[li].locked);
        h.get_by_label("Maske hinzufügen — damit blendest du Teile der Ebene aus, ohne sie zu löschen").click();
        h.run();
        assert!(h.state().project.sprite().editing_mask());
        // Mit dem Stift über die Lichtkante oben: die Maske blendet sie aus.
        h.state_mut().tool = Tool::Pencil;
        drag(&mut h, (10.0, 10.0), (13.0, 10.0));
        let sp = h.state().project.sprite();
        assert_eq!(sp.cel(0, li).get(11, 10), 2, "Licht-Ebene selbst unverändert");
        assert!(sp.layers[li].mask.as_ref().unwrap().hides(11, 10), "trotz Sperre in die Maske gemalt");
        let buf = spritebit_core::render_rgba(sp, &h.state().project.current_palette(), 0, spritebit_core::Rect { x: 11, y: 10, w: 1, h: 1 });
        assert_eq!(&buf[..3], &[0x99, 0x99, 0x99], "zu sehen ist die Figur, nicht das Licht");
        // Rückgängig nimmt den Strich in der Maske zurück.
        h.state_mut().undo();
        h.run();
        assert!(!h.state().project.sprite().layers[li].mask.as_ref().unwrap().hides(11, 10));
    }

    #[test]
    fn licht_vorschau_dann_als_ebenen() {
        let mut h = app();
        // 4×4-Block aus Farbe 3 (#999999) bei (10,10)
        for y in 10..14 {
            for x in 10..14 {
                h.state_mut().project.sprite_mut().active().set(x, y, 3);
            }
        }
        h.run();
        h.get_by_label("Licht").click();
        h.run();
        h.run();
        let cel = |h: &Harness<'_, SpritebitApp>, l: usize, x: u32, y: u32| h.state().project.sprite().cel(0, l).get(x, y);
        // Vorschau: noch keine Ebene, die Zeichenfläche zeigt eine Kopie mit Licht.
        assert_eq!(h.state().project.sprite().layers.len(), 1, "Vorschau legt nichts an");
        let pv = &h.state().image.preview.as_ref().expect("Vorschau da").1;
        assert_eq!(pv.cel(0, 1).get(10, 10), 2, "Vorschau: oben links heller");

        h.get_by_label("Als Ebene übernehmen").click();
        h.run();
        let sp = h.state().project.sprite();
        assert_eq!(sp.layers.len(), 2, "Licht-Ebene dazu");
        assert!(sp.layers[1].fx.is_some() && sp.layers[1].locked);
        assert_eq!(sp.layer, 0, "aktiv bleibt die Figur");
        assert_eq!(cel(&h, 0, 10, 10), 3, "Original unverändert");
        assert_eq!(cel(&h, 1, 10, 10), 2);
        h.run();
        assert!(h.state().image.preview.is_none(), "keine Vorschau mehr, die Ebene zeigt es");

        // Andere Richtung: neu gerechnet, die alten Kanten sind weg.
        h.get_by_label("↘").click();
        h.run();
        assert_eq!(cel(&h, 1, 13, 13), 2, "unten rechts jetzt hell");
        assert_ne!(cel(&h, 1, 10, 10), 2);
        h.state_mut().undo();
        h.run();
        assert_eq!(cel(&h, 1, 10, 10), 2, "Rückgängig holt das vorige Licht");

        // Schlagschatten anhaken: Schatten-Ebene unter der Figur.
        h.get_by_label("Schlagschatten").click();
        h.run();
        let sp = h.state().project.sprite();
        assert_eq!(sp.layers.len(), 3);
        assert_eq!(sp.layer, 1, "die Figur ist nach oben gerückt und bleibt aktiv");
        assert_ne!(cel(&h, 0, 14, 14), 0, "Schatten unten rechts (Licht von oben links)");
        // … und wieder weg.
        h.get_by_label("Schlagschatten").click();
        h.run();
        assert_eq!(h.state().project.sprite().layers.len(), 2);
        assert_eq!(h.state().project.sprite().layer, 0);
    }

    #[test]
    fn sprite_anlegen_duplizieren_loeschen() {
        let mut h = app();
        h.state_mut().open_new_sprite();
        if let Some(sprites_ui::SpriteDialog::New { name, palette, w, h: hh }) = &mut h.state_mut().sprite_dialog {
            *name = "Held".into();
            *palette = "pico8".into();
            *w = 16;
            *hh = 8;
        }
        h.run();
        h.get_by_label("Erstellen").click();
        h.run();
        let a = h.state();
        assert_eq!(a.project.sprites.len(), 2);
        assert_eq!((a.sprite().name.as_str(), a.sprite().palette.as_str(), a.sprite().width), ("Held", "pico8", 16));
        h.state_mut().sprite_dialog = Some(sprites_ui::SpriteDialog::Delete { i: 0 });
        h.run();
        h.get_by_label("Löschen").click();
        h.run();
        assert_eq!(h.state().project.sprites.len(), 1);
        assert_eq!(h.state().sprite().name, "Held", "der verbleibende ist aktiv");
    }

    #[test]
    fn ebene_zusammenlegen_ueber_knopf() {
        let mut h = app();
        h.state_mut().project.sprite_mut().active().set(1, 1, 5);
        h.get_by_label("Neue Ebene über der aktiven").click();
        h.run();
        h.state_mut().project.sprite_mut().active().set(2, 2, 3);
        h.get_by_label("Nach unten zusammenlegen — in jedem Frame").click();
        h.run();
        let sp = h.state().sprite();
        assert_eq!(sp.layers.len(), 1);
        assert_eq!((sp.cel(0, 0).get(1, 1), sp.cel(0, 0).get(2, 2)), (5, 3));
    }

    /// Was man an (x, y) sieht — als RGB.
    fn rgb_at(h: &Harness<'_, SpritebitApp>, x: u32, y: u32) -> Option<spritebit_core::Rgb> {
        let a = h.state();
        spritebit_core::selection::rgb_of(px(h, x, y), &a.project.current_palette(), &a.sprite().free)
    }

    #[test]
    fn farbstufen_ordnen_bild_bleibt_und_undo() {
        let mut h = app();
        for x in 0..5 {
            h.state_mut().project.sprite_mut().active().set(x, 0, x as u16 + 1);
        }
        let before: Vec<_> = (0..5).map(|x| rgb_at(&h, x, 0)).collect();
        let name0 = h.state().sprite().palette.clone();
        h.get_by_label("Nach Farbstufen").click();
        h.run();
        let after: Vec<_> = (0..5).map(|x| rgb_at(&h, x, 0)).collect();
        assert_eq!(before, after, "das Bild sieht gleich aus");
        h.state_mut().undo();
        assert_eq!(h.state().sprite().palette, name0);
        assert_eq!((0..5).map(|x| rgb_at(&h, x, 0)).collect::<Vec<_>>(), before);
    }

    #[test]
    fn palette_zuweisen_behaelt_das_aussehen() {
        let mut h = app();
        h.state_mut().project.sprite_mut().active().set(3, 3, 2);
        let look = rgb_at(&h, 3, 3);
        let other = spritebit_core::builtin::BUILTIN.iter().map(|(n, _)| *n).find(|n| *n != h.state().sprite().palette).unwrap();
        h.state_mut().assign_palette(other, true);
        assert_eq!(h.state().sprite().palette, other);
        assert_eq!(rgb_at(&h, 3, 3), look);
    }

    #[test]
    fn bild_zu_palette_reduziert() {
        let mut h = app();
        for (x, c) in [[250u8, 0, 0], [240, 0, 0], [0, 0, 250]].iter().enumerate() {
            let v = h.state_mut().project.sprite_mut().free_color(*c);
            h.state_mut().project.sprite_mut().active().set(x as u32, 0, v);
        }
        h.get_by_label("Bild » Palette …").click();
        h.run();
        if let Some(r) = &mut h.state_mut().pal.reduce {
            r.count = 2;
        }
        h.run();
        h.get_by_label("Palette anlegen").click();
        h.run();
        let a = h.state();
        assert!(a.sprite().palette.starts_with("foto"));
        assert_eq!(a.project.current_palette().len(), 2);
        assert!(px(&h, 0, 0) < spritebit_core::FREE_BASE && px(&h, 0, 0) == px(&h, 1, 0));
    }

    #[test]
    fn hilfslinie_ziehen_und_hinausziehen_loescht() {
        let mut h = app();
        h.get_by_label("Hilfslinien").click();
        h.run();
        h.get_by_label("+ Waagerecht").click();
        h.run();
        assert_eq!(h.state().sprite().guides.h, vec![32]);
        assert!(h.state().guides.edit);
        // Linie bei y = 32 auf y = 10 ziehen — gemalt wird dabei nicht.
        drag(&mut h, (5.0, 31.6), (5.0, 9.6));
        assert_eq!(h.state().sprite().guides.h, vec![10]);
        assert_eq!(px(&h, 5, 20), 0, "im Modus wird nicht gemalt");
        // Aus dem Bild ziehen löscht sie.
        drag(&mut h, (5.0, 9.6), (5.0, -6.0));
        assert!(h.state().sprite().guides.h.is_empty());
        assert!(!h.state().guides.edit, "nichts mehr zu verschieben");
    }

    #[test]
    fn schablone_laden_uebernehmen_und_verschieben() {
        let mut h = app();
        // Ein echtes PNG durch den Lader: 2×1, links weiß, rechts schwarz.
        let mut sp = spritebit_core::Sprite::new("t", 2, 1).unwrap();
        sp.active().set(0, 0, 1);
        sp.active().set(1, 0, 5);
        let png = spritebit_core::export::png(&sp, &h.state().project.current_palette(), 0, 1).unwrap();
        let t = template_ui::decode(&png).unwrap();
        let ctx = h.ctx.clone();
        h.state_mut().set_template(&ctx, t, "test.png".into());
        h.run();
        h.get_by_label("Schablone").click();
        h.run();
        h.get_by_label("Palettenfarben").click();
        h.run();
        // Fläche 64×64, Bild 2:1 eingepasst → Zeilen 16..48 belegt.
        assert_eq!(px(&h, 10, 30), 1);
        assert_eq!(px(&h, 50, 30), 5);
        assert_eq!(px(&h, 10, 5), 0, "Rand bleibt leer");
        // Umschalt+Alt ziehen verschiebt — gemalt wird dabei nicht.
        let before = h.state().template.offset;
        let (a, b) = (at(&h, 20.0, 20.0), at(&h, 30.0, 20.0));
        let m = Modifiers::SHIFT | Modifiers::ALT;
        h.hover_at(a);
        h.run();
        h.event_modifiers(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: m }, m);
        h.run();
        h.event_modifiers(egui::Event::PointerMoved(b), m);
        h.run();
        h.event_modifiers(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: m }, m);
        h.run();
        let after = h.state().template.offset;
        assert!((after.0 - before.0 - 10.0).abs() < 0.5, "{after:?}");
    }

    #[test]
    fn code_importieren_als_neuer_sprite() {
        let mut h = app();
        h.state_mut().out.import = Some(export_ui::ImportModal {
            text: "export const HELD_PALETTE = { 1: '#ff0000', 2: '#00ff00' };
export const HELD = [[0,1],[2,1]];".into(),
            use_palette: true,
        });
        h.run();
        h.get_by_label("Als neuen Sprite").click();
        h.run();
        let a = h.state();
        assert_eq!(a.project.sprites.len(), 2);
        assert_eq!((a.sprite().name.as_str(), a.sprite().width, a.sprite().height), ("HELD", 2, 2));
        assert_eq!(a.sprite().palette, "held2", "„held“ ist eine eingebaute Palette");
        assert_eq!(a.project.current_palette().colors, vec![[255, 0, 0], [0, 255, 0]]);
        assert_eq!(px(&h, 1, 0), 1);
        assert_eq!(px(&h, 0, 1), 2);
    }

    #[test]
    fn code_panel_zeigt_den_code_und_menue_klappt_es_auf() {
        let mut h = app();
        h.state_mut().project.sprite_mut().active().set(0, 0, 3);
        h.state_mut().open_export();
        h.run();
        h.run();
        // Das Panel ist offen: seine Knöpfe sind da.
        h.get_by_label("Palette in den Code schreiben");
        h.get_by_label("PDF");
    }

    #[test]
    fn timeline_einstellungen_links_und_onion() {
        let mut h = app();
        h.get_by_label("Timeline-Einstellungen — Lage, Kopfzeile, Dauer, Onion Skin").click();
        h.run();
        h.get_by_label("Links").click();
        h.run();
        assert_eq!(h.state().tl.zone, tlmenu_ui::Zone::Left);
        h.state_mut().tl_menu_open = false;
        h.run();
        h.get_by_label("Leerer Frame dahinter").click();
        h.run();
        assert_eq!(h.state().sprite().frames.len(), 2, "Timeline arbeitet auch links");
        // Onion Skin an: der Nachbar-Frame wird eine eigene Textur.
        h.state_mut().project.sprite_mut().cel_mut(0, 0).set(1, 1, 5);
        h.state_mut().onion = true;
        h.state_mut().changed();
        let ctx = h.ctx.clone();
        let r = spritebit_core::Rect { x: 0, y: 0, w: 64, h: 64 };
        assert!(h.state_mut().onion_texture(&ctx, r, 1).is_some());
        h.state_mut().onion = false;
        assert!(h.state_mut().onion_texture(&ctx, r, 1).is_none());
    }

    #[test]
    fn ziffern_waehlen_die_farbe_und_strg_rad_zoomt() {
        let mut h = app();
        h.key_press(Key::Num3);
        h.run();
        assert_eq!(h.state().color, 3);
        h.key_press(Key::Num0);
        h.run();
        assert_eq!(h.state().color, 0);
        // Mausrad ohne Strg scrollt nur.
        let z = h.state().zoom;
        let p = at(&h, 10.0, 10.0);
        h.hover_at(p);
        h.run();
        h.event(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, -40.0), phase: egui::TouchPhase::Move, modifiers: Modifiers::NONE });
        h.run();
        assert_eq!(h.state().zoom, z, "kein Zoom ohne Strg");
    }

    #[test]
    fn alt_ziehen_verschiebt_eine_kopie() {
        let mut h = app();
        h.state_mut().tool = tools_ui::Tool::Select;
        h.state_mut().project.sprite_mut().active().set(5, 5, 4);
        h.state_mut().selection = Some(Selection::rect(5, 5, 5, 5));
        let (a, b) = (at(&h, 5.0, 5.0), at(&h, 9.0, 5.0));
        h.hover_at(a);
        h.run();
        h.event_modifiers(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::ALT }, Modifiers::ALT);
        h.run();
        for k in 1..=4 {
            h.event_modifiers(egui::Event::PointerMoved(a + (b - a) * (k as f32 / 4.0)), Modifiers::ALT);
            h.run();
        }
        h.event_modifiers(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::ALT }, Modifiers::ALT);
        h.run();
        h.state_mut().deselect();
        assert_eq!(px(&h, 5, 5), 4, "Original bleibt");
        assert_eq!(px(&h, 9, 5), 4, "Kopie verschoben");
    }

    #[test]
    fn markierte_frames_loeschen_und_farbe_zeigen() {
        let mut h = app();
        for _ in 0..3 {
            h.get_by_label("Leerer Frame dahinter").click();
            h.run();
        }
        assert_eq!(h.state().sprite().frames.len(), 4);
        h.state_mut().frame_sel = vec![1, 2];
        h.get_by_label("Frame löschen (markierte alle)").click();
        h.run();
        assert_eq!(h.state().sprite().frames.len(), 2);
        // Farbe zeigen: zählt die Pixel der aktuellen Farbe.
        h.state_mut().project.sprite_mut().active().set(0, 0, 5);
        h.state_mut().project.sprite_mut().active().set(1, 0, 5);
        h.state_mut().color = 5;
        assert_eq!(h.state().count_current_color(), 2);
        h.state_mut().color = 0;
        assert_eq!(h.state().count_current_color(), 64 * 64 - 2);
    }

    #[test]
    fn enter_uebernimmt_die_freie_drehung() {
        let mut h = app();
        h.state_mut().project.sprite_mut().active().set(32, 10, 5);
        h.state_mut().image.angle = 90.0;
        h.get_by_label("Bild").click(); // zu- und wieder aufklappen ist egal
        h.run();
        h.state_mut().flip(true); // ein beliebiger Schritt davor
        h.state_mut().image.live = None;
        let before = h.state().sprite().cel(0, 0).get(31, 10);
        assert_eq!(before, 5);
        h.state_mut().test_rotate(90.0);
        assert!(h.state().image.live.is_some());
        h.key_press(Key::Enter);
        h.run();
        assert!(h.state().image.live.is_none(), "übernommen");
        assert!(!h.state().playing, "nicht abgespielt");
    }

    #[test]
    fn pixel_perfekt_zieht_saubere_diagonalen() {
        let mut h = app();
        h.get_by_label("Clean Stroke").click();
        h.run();
        assert!(h.state().pixel_perfect);
        // Treppe von Hand: (2,2) → (3,2) → (3,3) → (4,3) → (4,4)
        let pts = [(2.0, 2.0), (3.0, 2.0), (3.0, 3.0), (4.0, 3.0), (4.0, 4.0)];
        let a = at(&h, pts[0].0, pts[0].1);
        h.hover_at(a);
        h.run();
        h.drag_at(a);
        h.run();
        for p in &pts[1..] {
            h.hover_at(at(&h, p.0, p.1));
            h.run();
        }
        h.drop_at(at(&h, 4.0, 4.0));
        h.run();
        assert_eq!([px(&h, 2, 2), px(&h, 3, 3), px(&h, 4, 4)], [5, 5, 5]);
        assert_eq!([px(&h, 3, 2), px(&h, 4, 3)], [0, 0], "Eckpixel entfernt");
        h.state_mut().undo();
        assert_eq!(px(&h, 2, 2), 0, "ein Undo-Schritt");
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
