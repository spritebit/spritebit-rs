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
mod dock_ui;
mod export_ui;
mod guides_ui;
mod i18n;
mod icons;
mod image_ui;
mod layers_ui;
mod palette_ui;
mod preview_ui;
mod projects_ui;
mod selection_ui;
mod selfupdate;
mod sprites_ui;
mod tabs;
mod template_ui;
mod tiles_ui;
mod timeline;
mod tlmenu_ui;
mod tools_ui;
mod update;
mod view_ui;

use std::path::{Path, PathBuf};

use crate::i18n::{keys, tr, trf};
use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Sense, Stroke, Vec2};
use spritebit_core::{export_web, import_web, load_native, render_rgba_step, save_native, selection, History, Project, Px, Rect, Selection, Sprite};
use tools_ui::{Pointer, Tool};

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
            // Beschriftungen nicht mit der Maus markierbar — sonst färbt Ziehen
            // über Reiter und Panels Text blau. Eingabefelder bleiben es.
            cc.egui_ctx.all_styles_mut(|s| s.interaction.selectable_labels = false);
            let mut app = SpritebitApp::new();
            // Wie im Browser: die zuletzt geladene Schablone ist wieder da.
            app.restore_template(&cc.egui_ctx);
            app.load_tl_opts();
            app.load_view();
            app.load_guide_layouts();
            app.load_recent();
            // Die abgelöste Datei vom letzten Update wegräumen (selfupdate.rs).
            selfupdate::cleanup_old();
            // Nach einem Absturz (oder einem Update): die Sitzung zurückholen.
            app.start_session();
            // Frischer Start (keine Sitzung zurückgeholt): das zuletzt geöffnete
            // Projekt — das Startfenster nur, wenn es keins gibt oder es fehlt.
            if app.path.is_none() && !app.dirty {
                app.open_last();
                if app.path.is_none() {
                    app.projects.start_open = true;
                }
            }
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
pub(crate) const EXT: &str = "sb";
const OLD_EXT: &str = "spritebit";
/// Ein einzelner Sprite: JSON wie in der Web-Version, dort genauso lesbar.
const SPRITE_EXT: &str = "bitty";
/// Aseprite-Dateien (spritebit_core::aseprite): lesen beide, geschrieben wird .aseprite.
const ASE_EXTS: [&str; 2] = ["aseprite", "ase"];

/// Ist das eine Aseprite-Datei (nach der Endung)?
fn is_ase(path: &Path) -> bool {
    path.extension().is_some_and(|e| ASE_EXTS.iter().any(|a| e.eq_ignore_ascii_case(a)))
}
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
    /// Text für die Zwischenablage des Systems, sobald kopiert wurde
    /// (selection_ui.rs) — damit Strg+V dort ankommt.
    clip_text: Option<String>,
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
    /// Projektname, zuletzt geöffnet, Startfenster (projects_ui.rs).
    projects: projects_ui::ProjectsUi,
    /// Wo welches Panel steht (dock_ui.rs).
    dock: dock_ui::DockLayout,
    /// Vorschaubilder im Ebenen-Panel (layers_ui.rs).
    layer_thumbs: layers_ui::LayerThumbs,
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
    /// Panels „Bild“ und „Feinschliff“.
    image: image_ui::ImagePanel,
    /// Panel „Kacheln“ (Tilemap-Ebenen).
    tiles: tiles_ui::TileState,
}

/// Was nach der Rückfrage „Ungespeicherte Änderungen" passieren soll.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    Close,
    /// Neues Projekt mit diesem Namen (projects_ui.rs).
    NewProject(String),
    /// Ein Projekt öffnen (Datei → Öffnen, „Zuletzt geöffnet“).
    OpenPath(PathBuf),
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
            clip_text: None,
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
            projects: projects_ui::ProjectsUi::default(),
            dock: dock_ui::DockLayout::load(),
            layer_thumbs: layers_ui::LayerThumbs::default(),
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
            rng: spritebit_core::tools::Rng::new(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)),
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
    /// Öffnen: erst die Datei wählen. Ein Sprite (.bitty, Aseprite) kommt ins
    /// offene Projekt dazu; nur ein Projekt fragt nach Ungespeichertem.
    fn open(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(tr("Öffnen"))
            .add_filter(tr("spritebit-Projekt, Sprite oder Aseprite"), &[EXT, OLD_EXT, SPRITE_EXT, "json", "aseprite", "ase"])
            .add_filter(tr("Aseprite"), &ASE_EXTS)
            .add_filter(tr("Alle Dateien"), &["*"])
            .pick_file()
        else {
            return;
        };
        let adds = is_ase(&path) || path.extension().is_some_and(|e| e.eq_ignore_ascii_case(SPRITE_EXT));
        if self.dirty && !adds {
            self.unsaved_ask = Some(Pending::OpenPath(path));
        } else {
            self.open_path(path);
        }
    }

    /// Eine Datei öffnen: Projekt (.sb, .spritebit), Web-Projekt (.json)
    /// oder einzelner Sprite (.bitty, kommt dazu).
    pub(crate) fn open_path(&mut self, path: PathBuf) {
        // Aseprite: ein Sprite — kommt dazu wie eine .bitty-Datei.
        if is_ase(&path) {
            self.add_ase(&path);
            return;
        }
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
            Err(e) => {
                self.error = Some(trf("{path} konnte nicht gelesen werden: {e}", &[("path", &path.display()), ("e", &e)]));
                self.forget_recent(&path);
            }
            Ok(bytes) => {
                // Eigenes Format erkennt man am Anfang; alles andere wird als
                // Projektdatei der Web-Version versucht.
                let result = if bytes.starts_with(b"SPRITEBIT\0") {
                    load_native(&bytes).map(|p| (p, Some(path.clone())))
                } else {
                    // Aus einer Web-Datei wird beim Speichern eine .sb-Datei.
                    std::str::from_utf8(&bytes).map_err(|_| spritebit_core::IoError::NotAProject("kein Text".into())).and_then(import_web).map(|p| (p, None))
                };
                match result {
                    Ok((p, keep_path)) => {
                        self.replace_project(p, keep_path);
                        if self.path.is_none() {
                            // Web-Projekt: ungespeichert, damit „Speichern" nach dem Ziel fragt.
                            self.dirty = true;
                        } else {
                            self.remember_recent();
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
        let name = format!("{}.{EXT}", projects_ui::file_stem_for(&self.project_display_name()));
        if let Some(path) =
            rfd::FileDialog::new().set_title(tr("Projekt speichern")).add_filter(tr("spritebit-Projekt"), &[EXT]).set_file_name(name).save_file()
        {
            let path = if path.extension().is_none() { path.with_extension(EXT) } else { path };
            self.write_native(&path);
        }
    }

    pub(crate) fn write_native(&mut self, path: &Path) {
        self.commit_float();
        // Ohne eigenen Namen bekommt das Projekt den Dateinamen — er steht
        // dann auch in der Datei (und in der Web-Version).
        if self.project.name.trim().is_empty() {
            if let Some(stem) = path.file_stem() {
                self.project.name = stem.to_string_lossy().into_owned();
            }
        }
        match std::fs::write(path, save_native(&self.project)) {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.dirty = false;
                self.remember_recent();
            }
            Err(e) => self.error = Some(trf("{path} konnte nicht gespeichert werden: {e}", &[("path", &path.display()), ("e", &e)])),
        }
    }

    /// Nur den aktuellen Sprite als .bitty sichern (JSON der Web-Version,
    /// dort genauso lesbar) — das Projekt bleibt, wie es ist.
    fn save_sprite_as(&mut self) {
        self.commit_float();
        let name = format!("{}.{SPRITE_EXT}", self.sprite().name);
        if let Some(path) =
            rfd::FileDialog::new().set_title(tr("Sprite speichern")).add_filter(tr("spritebit-Sprite"), &[SPRITE_EXT]).set_file_name(name).save_file()
        {
            let path = if path.extension().is_none() { path.with_extension(SPRITE_EXT) } else { path };
            match std::fs::write(&path, spritebit_core::export_sprite(&self.project, self.project.current)) {
                Ok(()) => {
                    self.hint =
                        Some(trf("Sprite gespeichert: {name}", &[("name", &path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()))]))
                }
                Err(e) => self.error = Some(trf("{path} konnte nicht gespeichert werden: {e}", &[("path", &path.display()), ("e", &e)])),
            }
        }
    }

    /// Sprites aus einer Datei zum Projekt dazunehmen: .bitty, eine
    /// spritebit-Projektdatei oder ein Web-Projekt — ersetzt wird nichts
    /// (Project::merge).
    fn add_sprites(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(tr("Sprites aus Projekt holen"))
            .add_filter(tr("spritebit-Sprite oder -Projekt"), &[SPRITE_EXT, EXT, OLD_EXT, "json", "aseprite", "ase"])
            .add_filter(tr("Aseprite"), &ASE_EXTS)
            .add_filter(tr("Alle Dateien"), &["*"])
            .pick_file()
        else {
            return;
        };
        if is_ase(&path) {
            self.add_ase(&path);
            return;
        }
        let other = match std::fs::read(&path) {
            Err(e) => {
                self.error = Some(trf("{path} konnte nicht gelesen werden: {e}", &[("path", &path.display()), ("e", &e)]));
                return;
            }
            Ok(bytes) if bytes.starts_with(b"SPRITEBIT\0") => load_native(&bytes),
            Ok(bytes) => std::str::from_utf8(&bytes).map_err(|_| spritebit_core::IoError::NotAProject("kein Text".into())).and_then(import_web),
        };
        match other {
            Ok(other) => self.merge_project(other),
            Err(e) => self.error = Some(i18n::io_error(&e)),
        }
    }

    /// Eine Aseprite-Datei als neuen Sprite dazunehmen — samt ihrer Palette.
    pub(crate) fn add_ase(&mut self, path: &Path) {
        let name = path.file_stem().map_or_else(|| "aseprite".to_string(), |s| s.to_string_lossy().into_owned());
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.error = Some(trf("{path} konnte nicht gelesen werden: {e}", &[("path", &path.display()), ("e", &e)]));
                return;
            }
        };
        match spritebit_core::aseprite::read(&bytes, &name) {
            Ok(a) => {
                let p = Project { name: String::new(), sprites: vec![a.sprite], palettes: vec![a.palette], current: 0, materials: Default::default() };
                self.merge_project(p);
            }
            Err(e) => self.error = Some(i18n::ase_error(&e)),
        }
    }

    /// Den aktuellen Sprite als Aseprite-Datei sichern.
    fn save_ase_as(&mut self) {
        self.commit_float();
        let name = format!("{}.aseprite", projects_ui::file_stem_for(&self.sprite().name));
        if let Some(path) =
            rfd::FileDialog::new().set_title(tr("Als Aseprite speichern")).add_filter(tr("Aseprite"), &["aseprite"]).set_file_name(name).save_file()
        {
            let path = if path.extension().is_none() { path.with_extension("aseprite") } else { path };
            let bytes = spritebit_core::aseprite::write(self.sprite(), &self.project.current_palette());
            match std::fs::write(&path, bytes) {
                Ok(()) => {
                    self.hint = Some(trf(
                        "Als Aseprite gespeichert: {name}",
                        &[("name", &path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()))],
                    ))
                }
                Err(e) => self.error = Some(trf("{path} konnte nicht gespeichert werden: {e}", &[("path", &path.display()), ("e", &e)])),
            }
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
        if let Some(path) =
            rfd::FileDialog::new().set_title(tr("Als Web-Projekt exportieren")).add_filter(tr("Web-Projekt (JSON)"), &["json"]).set_file_name(name).save_file()
        {
            if let Err(e) = std::fs::write(&path, export_web(&self.project)) {
                self.error = Some(trf("{path} konnte nicht geschrieben werden: {e}", &[("path", &path.display()), ("e", &e)]));
            }
        }
    }

    /// Titelleiste: Dateiname und ein Sternchen bei ungespeicherten Änderungen.
    fn sync_title(&mut self, ctx: &egui::Context) {
        let title = format!("{}{} — spritebit {VERSION}", self.project_display_name(), if self.dirty { " *" } else { "" });
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
        // Strg+Alt+N zuerst: egui ignoriert ein zusätzliches Alt, sonst
        // gälte es als Strg+N (neues Projekt).
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::ALT, Key::N)) {
            self.open_new_sprite();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::N)) {
            self.open_new_project();
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
            let (play, mut prev, mut next, first, last) = ctx.input_mut(|i| {
                (
                    i.consume_key(none, Key::Enter),
                    i.consume_key(none, Key::Comma),
                    i.consume_key(none, Key::Period),
                    i.consume_key(none, Key::Home),
                    i.consume_key(none, Key::End),
                )
            });
            // Pfeiltasten ohne Auswahl: links / rechts Frame, hoch / runter
            // Ebene. Mit Auswahl verschieben sie diese (selection_keys).
            if self.selection.is_none() {
                let (left, right, up, down) = ctx.input_mut(|i| {
                    (
                        i.consume_key(none, Key::ArrowLeft),
                        i.consume_key(none, Key::ArrowRight),
                        i.consume_key(none, Key::ArrowUp),
                        i.consume_key(none, Key::ArrowDown),
                    )
                });
                prev |= left;
                next |= right;
                let (l, nl) = (self.sprite().layer, self.sprite().layers.len());
                let to = if up {
                    (l + 1).min(nl - 1)
                } else if down {
                    l.saturating_sub(1)
                } else {
                    l
                };
                if to != l {
                    self.project.sprite_mut().layer = to;
                    self.stroke_last = None;
                }
            }
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
                // Neu, Öffnen, Zuletzt geöffnet, Umbenennen (projects_ui.rs)
                self.projects_menu(ui);
                ui.separator();
                if ui.add(egui::Button::new(tr("Projekt speichern")).shortcut_text(keys("Strg+S"))).clicked() {
                    self.save();
                }
                if ui.add(egui::Button::new(tr("Projekt speichern unter …")).shortcut_text(keys("Strg+Umschalt+S"))).clicked() {
                    self.save_as();
                }
                // Nur der aktuelle Sprite statt des ganzen Projekts
                ui.menu_button(tr("Sprite speichern"), |ui| {
                    if ui.button(tr("als spritebit-Datei (.bitty) …")).on_hover_text(tr("Nur den aktuellen Sprite — zum Weitergeben oder für ein anderes Projekt")).clicked() {
                        self.save_sprite_as();
                        ui.close();
                    }
                    if ui.button(tr("als Aseprite-Datei (.aseprite) …")).on_hover_text(tr("Den aktuellen Sprite als .aseprite-Datei — Ebenen, Frames, Tags, Palette und Tilemaps bleiben; Masken werden eingerechnet")).clicked() {
                        self.save_ase_as();
                        ui.close();
                    }
                });
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
                if ui.add_enabled(has, egui::Button::new(tr("Auswahl aufheben")).shortcut_text(keys("Strg+D"))).clicked() {
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
                // Die Linien erscheinen erst ab GRID_FROM — darunter wären
                // sie dichter als die Pixel. Ohne Hinweis wirkte das wie kaputt.
                ui.checkbox(&mut self.show_grid, tr("Gitter")).on_hover_text(tr("Linien zwischen den Pixeln — sichtbar ab 800 % Zoom, darunter wären sie dichter als das Bild."));
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
            // Welches Projekt offen ist (wie die Schaltfläche im Web); ein
            // Klick benennt es um (projects_ui.rs).
            ui.separator();
            self.project_badge(ui);
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
        // Liegt ein Fenster oder eine aufgeklappte Liste über der Fläche
        // (Neuer Sprite → Farbpalette …), gehört das Mausrad ihr, nicht der
        // Fläche dahinter. contains_pointer beachtet diese Ebenen.
        let over_canvas = resp.contains_pointer();
        // Wie im Web: Mausrad scrollt (Umschalt: waagerecht), Strg+Mausrad
        // zoomt um den Zeiger.
        if let Some(at) = pointer.filter(|p| over_canvas && area.contains(*p)) {
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
        self.hover = pointer.filter(|p| over_canvas && area.contains(*p)).map(to_cell);

        // Werkzeug anwenden (tools_ui.rs).
        let p =
            Pointer { cell: pointer.map(to_cell), over: resp.hovered() || resp.dragged(), primary, secondary, pressed, released, panning, alt, pos: pointer };
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
            let screen = egui::Rect::from_min_max(origin + Vec2::new(x0 as f32, y0 as f32) * zoom, origin + Vec2::new(ex as f32, ey as f32) * zoom);
            let uv =
                egui::Rect::from_min_max(Pos2::ZERO, Pos2::new((ex - x0) as f32 / (tw as u32 * step) as f32, (ey - y0) as f32 / (th as u32 * step) as f32));
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
                painter.line_segment([Pos2::new(sx, origin.y + y0 as f32 * zoom), Pos2::new(sx, origin.y + y1 as f32 * zoom)], line);
            }
            for y in y0..=y1 {
                let sy = origin.y + y as f32 * zoom;
                painter.line_segment([Pos2::new(origin.x + x0 as f32 * zoom, sy), Pos2::new(origin.x + x1 as f32 * zoom, sy)], line);
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
            egui::Window::new(tr("Über spritebit")).collapsible(false).resizable(false).open(&mut self.about_open).show(ctx, |ui| {
                ui.label(tr("spritebit — Pixel-Art-Editor"));
                ui.strong(format!("Version {VERSION}"));
                ui.label(tr("© 2026 Marco Jan · freie Software unter der MIT-Lizenz"));
                ui.hyperlink_to(tr("Neue Versionen auf GitHub"), "https://github.com/spritebit/spritebit-rs/releases");
                ui.add_space(6.0);
                ui.hyperlink_to(tr("spritebit unterstützen"), DONATE_URL).on_hover_text(tr(
                    "Kostenlos bleibt spritebit sowieso. Spenden fließen in ein Code-Signatur-Zertifikat, damit Windows bei der Desktop-App nicht mehr warnt.",
                ));
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
        let Some(pending) = self.unsaved_ask.clone() else { return };
        let (mut save, mut discard, mut cancel) = (false, false, false);
        egui::Modal::new(egui::Id::new("unsaved")).show(ctx, |ui| {
            ui.heading(tr("Ungespeicherte Änderungen"));
            ui.label(match pending {
                Pending::Close => tr("Vor dem Beenden speichern?"),
                Pending::OpenPath(_) => tr("Vor dem Öffnen eines anderen Projekts speichern?"),
                Pending::NewProject(_) => tr("Vor dem Anlegen eines neuen Projekts speichern?"),
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
                Pending::NewProject(name) => self.create_project_now(name),
                Pending::OpenPath(path) => self.open_path(path),
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
            tlmenu_ui::Zone::Bottom => egui::Panel::bottom("timeline").resizable(true).default_size(190.0).show(ui, |ui| self.timeline(ui)),
            tlmenu_ui::Zone::Top => egui::Panel::top("timeline-top").resizable(true).default_size(190.0).show(ui, |ui| self.timeline(ui)),
            tlmenu_ui::Zone::Left => egui::Panel::left("timeline-left").resizable(true).default_size(360.0).show(ui, |ui| self.timeline(ui)),
            tlmenu_ui::Zone::Right => egui::Panel::right("timeline-right").resizable(true).default_size(360.0).show(ui, |ui| self.timeline(ui)),
        };
        // Seitenleisten (dock_ui.rs): außen die Icons gelöster Panels, innen
        // die Spalten mit den angepinnten.
        use dock_ui::{PanelId, Side};
        // „Exportieren …“ im Menü: das Panel aufklappen (oder, gelöst, aufklappen lassen).
        let focus_output = std::mem::take(&mut self.out.focus);
        if focus_output && !self.dock.is_pinned(PanelId::Output) {
            let r = ctx.content_rect();
            let at = match self.dock.side_of(PanelId::Output) {
                Side::Left => Pos2::new(r.left() + 42.0, r.top() + 90.0),
                Side::Right => Pos2::new(r.right() - 42.0, r.top() + 90.0),
            };
            self.dock.flyout = Some((PanelId::Output, at));
        }
        self.dock_rail(ui, Side::Left);
        self.dock_rail(ui, Side::Right);
        if !self.dock.pinned(Side::Left).is_empty() {
            egui::Panel::left("side").resizable(true).default_size(180.0).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("side-scroll").show(ui, |ui| self.dock_column(ui, Side::Left, focus_output));
            });
        }
        if !self.dock.pinned(Side::Right).is_empty() {
            egui::Panel::right("panels").resizable(true).default_size(230.0).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("panels-scroll").show(ui, |ui| self.dock_column(ui, Side::Right, focus_output));
            });
        }
        // Erst nach beiden Spalten: der gespeicherte Auf-/Zu-Stand ist gesetzt.
        self.panels.applied = true;
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
        self.projects_windows(&ctx);
        self.dock_flyout(&ctx);
        self.dock_drag_preview(&ctx);
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
mod tests;
