//! Ansicht, Hilfe und Sicherung — die Teile der Web-Version rund um die
//! Zeichenfläche:
//!
//! * Hintergrund Dunkel/Hell (Schachbrett, Gitter, „Farbe zeigen“)
//! * Vollbild (F11; Esc beendet), Zoom-Regler in der Statusleiste
//! * „Farbe zeigen“: alles, was nicht die aktuelle Farbe hat, wird
//!   abgedunkelt; die Statusleiste zählt die Pixel dieser Farbe
//! * Hilfe mit allen Tastenkürzeln (F1), Links zu Startseite, Impressum,
//!   Datenschutz
//! * Sicherung: die Sitzung wird laufend in den Einstellungsordner
//!   geschrieben. Stürzt die App ab, ist beim nächsten Start alles wieder
//!   da; der Stand vom Start der Sitzung bleibt als Sicherung, die man in
//!   der Hilfe speichern oder wiederherstellen kann. Beim normalen Beenden
//!   (nach der Rückfrage) wird die laufende Sitzung verworfen.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32};
use spritebit_core::{load_native, save_native, selection::rgb_of, Rect, FREE_BASE};

use crate::i18n::{tr, trf};
use crate::SpritebitApp;

/// Schachbrett der Zeichenfläche: dunkel (wie bisher) und hell.
pub(crate) const CHECKER_DARK: [[u8; 3]; 2] = [[0x20, 0x20, 0x2c], [0x2a, 0x2a, 0x38]];
pub(crate) const CHECKER_LIGHT: [[u8; 3]; 2] = [[0xff, 0xff, 0xff], [0xd8, 0xd8, 0xd8]];

/// Die Webseite der Web-Version.
const SITE: &str = "https://spritebit.at/";

#[derive(Default)]
pub(crate) struct ViewState {
    /// Heller Hintergrund statt dunklem.
    pub light: bool,
    pub fullscreen: bool,
    /// „Farbe zeigen“.
    pub spotlight: bool,
    spot_tex: Option<(String, egui::TextureHandle)>,
    pub help_open: bool,
    /// Sitzung sichern (nur in der echten App).
    pub persist: bool,
    last_change: Option<(u64, Instant)>,
    saved_version: u64,
    pub restore_ask: bool,
}

fn dir() -> Option<PathBuf> {
    crate::i18n::settings_dir()
}

impl SpritebitApp {
    pub(crate) fn checker_colors(&self) -> [[u8; 3]; 2] {
        if self.view.light {
            CHECKER_LIGHT
        } else {
            CHECKER_DARK
        }
    }

    pub(crate) fn grid_color(&self) -> Color32 {
        if self.view.light {
            Color32::from_black_alpha(30)
        } else {
            Color32::from_white_alpha(18)
        }
    }

    // ── Einstellungen ───────────────────────────────────────────────
    pub(crate) fn load_view(&mut self) {
        self.view.persist = true;
        let Some(v) = dir().and_then(|d| std::fs::read_to_string(d.join("view.json")).ok()).and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        else {
            return;
        };
        self.view.light = v["bg"].as_str() == Some("light");
        self.pixel_perfect = v["pixelPerfect"].as_bool().unwrap_or(false);
        self.fill_visible = v["fillVisible"].as_bool().unwrap_or(false);
    }

    pub(crate) fn save_view(&self) {
        if let (true, Some(d)) = (self.view.persist, dir()) {
            let _ = std::fs::create_dir_all(&d);
            let v = serde_json::json!({ "bg": if self.view.light { "light" } else { "dark" }, "pixelPerfect": self.pixel_perfect, "fillVisible": self.fill_visible });
            let _ = std::fs::write(d.join("view.json"), v.to_string());
        }
    }

    pub(crate) fn set_light(&mut self, light: bool) {
        if self.view.light != light {
            self.view.light = light;
            self.checker = None;
            self.view.spot_tex = None;
            self.save_view();
        }
    }

    pub(crate) fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.view.fullscreen = !self.view.fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.view.fullscreen));
    }

    /// F11: Vollbild. Esc: Vollbild beenden (wenn sonst nichts Esc braucht).
    pub(crate) fn view_keys(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
            self.toggle_fullscreen(ctx);
        }
        let nothing_else = self.selection.is_none() && self.image.live.is_none();
        if self.view.fullscreen
            && nothing_else
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.toggle_fullscreen(ctx);
        }
    }

    /// Rechts in der Menüleiste: Vollbild und Dunkel/Hell, immer zu sehen.
    /// (Rechts nach links gelegt — der erste Knopf steht ganz rechts.)
    pub(crate) fn view_quick(&mut self, ui: &mut egui::Ui) {
        let c = ui.visuals().text_color();
        let (icon, label, tip) = if self.view.fullscreen {
            (crate::icons::SHRINK, tr("Vollbild beenden"), tr("Vollbild beenden (F11 oder Esc)"))
        } else {
            (crate::icons::EXPAND, tr("Vollbild"), tr("Vollbild — nur noch spritebit auf dem Bildschirm (F11)"))
        };
        if ui.add(egui::Button::image_and_text(crate::icons::image(icon, c), label)).on_hover_text(tip).clicked() {
            self.toggle_fullscreen(ui.ctx());
        }
        ui.add_space(6.0);
        // Rechts nach links: erst „Hell“, dann „Dunkel“ — auf dem Bildschirm also Dunkel | Hell.
        if ui.add(egui::Button::selectable(self.view.light, tr("Hell"))).on_hover_text(tr("Heller Hintergrund")).clicked() {
            self.set_light(true);
        }
        if ui.add(egui::Button::selectable(!self.view.light, tr("Dunkel"))).on_hover_text(tr("Dunkler Hintergrund")).clicked() {
            self.set_light(false);
        }
    }

    /// Einträge im Menü „Ansicht“.
    pub(crate) fn view_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(tr("Hintergrund"), |ui| {
            if ui.radio(!self.view.light, tr("Dunkel")).clicked() {
                self.set_light(false);
                ui.close();
            }
            if ui.radio(self.view.light, tr("Hell")).clicked() {
                self.set_light(true);
                ui.close();
            }
        });
        if ui.add(egui::Button::new(tr("Vollbild")).shortcut_text("F11")).clicked() {
            self.toggle_fullscreen(ui.ctx());
            ui.close();
        }
    }

    /// Einträge im Menü „Hilfe“.
    pub(crate) fn help_menu(&mut self, ui: &mut egui::Ui) {
        if ui.add(egui::Button::new(tr("Hilfe und Tastenkürzel")).shortcut_text("F1")).clicked() {
            self.view.help_open = true;
            ui.close();
        }
        ui.separator();
        for (label, page) in [(tr("Startseite"), ""), (tr("Impressum"), "impressum.html"), (tr("Datenschutz"), "datenschutz.html")] {
            if ui.button(label).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(format!("{SITE}{page}")));
                ui.close();
            }
        }
        ui.separator();
    }

    /// Zoom-Regler für die Statusleiste: ändert den Zoom um die Mitte der Fläche.
    pub(crate) fn zoom_slider(&mut self, ui: &mut egui::Ui) {
        let mut z = self.zoom;
        let r = ui.add(egui::Slider::new(&mut z, crate::ZOOM_MIN..=crate::ZOOM_MAX).logarithmic(true).show_value(false));
        if r.changed() {
            let area = self.canvas_rect;
            self.zoom_at(z / self.zoom, area.center(), area);
        }
        ui.label(format!("{:.0} %", self.zoom * 100.0));
    }

    // ── Farbe zeigen ────────────────────────────────────────────────

    /// Wie viele Pixel der aktiven Zelle die aktuelle Farbe haben.
    pub(crate) fn count_current_color(&self) -> usize {
        let sp = self.sprite();
        let pal = self.project.current_palette();
        let img = sp.cel(sp.frame, sp.layer);
        let want = rgb_of(self.color, &pal, &sp.free);
        let filled = img.pixels().filter(|&(_, _, v)| rgb_of(v, &pal, &sp.free) == want && want.is_some()).count();
        match want {
            None => (sp.width as usize * sp.height as usize) - img.pixels().count(),
            Some(_) => filled,
        }
    }

    /// Maske über `rect`: dunkel (bzw. hell), wo NICHT die aktuelle Farbe ist.
    pub(crate) fn spotlight_texture(&mut self, ctx: &egui::Context, rect: Rect, step: u32) -> Option<egui::TextureId> {
        if !self.view.spotlight {
            return None;
        }
        let key = format!("{}|{:?}|{}|{}|{}|{}", self.project.current, rect, step, self.version, self.color, self.view.light);
        if self.view.spot_tex.as_ref().is_none_or(|t| t.0 != key) {
            let sp = self.sprite();
            let pal = self.project.current_palette();
            let img = sp.cel(sp.frame, sp.layer);
            let want = rgb_of(self.color, &pal, &sp.free);
            let r = rect.clamp_to(sp.width, sp.height);
            let (w, h) = (r.w.div_ceil(step), r.h.div_ceil(step));
            let dim: [u8; 4] = if self.view.light { [255, 255, 255, 209] } else { [8, 8, 12, 204] };
            let mut rgba = vec![0u8; (w * h * 4) as usize];
            for ty in 0..h {
                for tx in 0..w {
                    let (x, y) = (r.x + tx * step, r.y + ty * step);
                    let v = img.get(x, y);
                    let c = if v >= FREE_BASE || v > 0 { rgb_of(v, &pal, &sp.free) } else { None };
                    if c != want {
                        rgba[((ty * w + tx) * 4) as usize..][..4].copy_from_slice(&dim);
                    }
                }
            }
            let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            self.view.spot_tex = Some((key, ctx.load_texture("spotlight", image, egui::TextureOptions::NEAREST)));
        }
        self.view.spot_tex.as_ref().map(|t| t.1.id())
    }

    // ── Hilfe ───────────────────────────────────────────────────────
    pub(crate) fn help_window(&mut self, ctx: &egui::Context) {
        if !self.view.help_open {
            return;
        }
        let mut open = true;
        let (mut save_backup, mut restore_backup) = (false, false);
        // Bittys Suche hat hierher geführt: diese Zeile wird gezeigt (bitty_ui.rs).
        let mut focus = self.bitty.help_focus;
        egui::Window::new(tr("Hilfe")).open(&mut open).default_size([560.0, 620.0]).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.weak(format!("spritebit {}", crate::VERSION));
                ui.label(tr("Pixel-Editor für Sprites, Animationen und Spiel-Levels. Du malst frei — mit Ebenen, Frames, Licht und Kacheln — oder paust ein Foto als Schablone ab und lässt es automatisch zu einem sauberen Sprite verarbeiten."));
                for (title, lines) in help_sections() {
                    ui.add_space(8.0);
                    ui.strong(title);
                    for l in lines {
                        let r = ui.label(format!("• {l}"));
                        crate::bitty_ui::mark_help_line(ui, &r, l, &mut focus);
                    }
                }
                ui.add_space(8.0);
                ui.strong(tr("Tastenkürzel"));
                egui::Grid::new("keys").num_columns(2).striped(true).show(ui, |ui| {
                    for (k, what) in help_keys() {
                        ui.monospace(k);
                        let r = ui.label(what);
                        crate::bitty_ui::mark_help_line(ui, &r, what, &mut focus);
                        ui.end_row();
                    }
                });
                ui.add_space(8.0);
                ui.strong(tr("Sicherung"));
                ui.label(tr("Die laufende Sitzung wird ständig im Einstellungsordner gesichert — stürzt die App ab, ist beim nächsten Start alles wieder da. Zusätzlich bleibt der Stand vom Start der Sitzung aufgehoben. Am sichersten ist trotzdem „Speichern“."));
                match backup_time() {
                    Some(t) => {
                        ui.weak(trf("Sicherung vom Sitzungsstart: {time}", &[("time", &t)]));
                        ui.horizontal(|ui| {
                            save_backup = ui.button(tr("Herunterladen")).clicked();
                            restore_backup = ui.button(tr("Wiederherstellen")).clicked();
                        });
                    }
                    None => {
                        ui.weak(tr("Noch keine Sicherung vorhanden."));
                    }
                }
            });
        });
        self.bitty.help_focus = focus;
        if !open {
            self.view.help_open = false;
        }
        if save_backup {
            if let (Some(src), Some(dst)) = (dir().map(|d| d.join("backup.spritebit")), rfd::FileDialog::new().set_file_name("sicherung.sb").save_file()) {
                if let Err(e) = std::fs::copy(&src, &dst) {
                    self.error = Some(trf("{path} konnte nicht geschrieben werden: {e}", &[("path", &dst.display()), ("e", &e)]));
                }
            }
        }
        if restore_backup {
            self.view.restore_ask = true;
        }
        self.restore_dialog(ctx);
    }

    fn restore_dialog(&mut self, ctx: &egui::Context) {
        if !self.view.restore_ask {
            return;
        }
        let (mut ok, mut cancel) = (false, false);
        egui::Modal::new(egui::Id::new("restore")).show(ctx, |ui| {
            ui.label(tr("Den aktuellen Stand durch diese Sicherung ersetzen? Der aktuelle Stand wird dabei selbst gesichert."));
            ui.horizontal(|ui| {
                ok = ui.button(tr("Wiederherstellen")).clicked();
                cancel = ui.button(tr("Abbrechen")).clicked();
            });
        });
        if cancel {
            self.view.restore_ask = false;
        }
        if ok {
            self.view.restore_ask = false;
            let Some(d) = dir() else { return };
            match std::fs::read(d.join("backup.spritebit")).map_err(|e| e.to_string()).and_then(|b| load_native(&b).map_err(|e| crate::i18n::io_error(&e))) {
                Ok(p) => {
                    // Der Stand von jetzt wird zur neuen Sicherung.
                    let _ = std::fs::write(d.join("backup.spritebit"), save_native(&self.project));
                    self.replace_project(p, None);
                    self.dirty = true;
                    self.hint = Some(tr("Sicherung wiederhergestellt.").into());
                }
                Err(e) => self.error = Some(e),
            }
        }
    }

    // ── Sitzung sichern ─────────────────────────────────────────────

    /// Beim Start: eine abgestürzte Sitzung zurückholen; ihr Stand (oder der
    /// leere Anfang) wird zur Sicherung vom Sitzungsstart.
    pub(crate) fn start_session(&mut self) {
        self.view.persist = true;
        let Some(d) = dir() else { return };
        let _ = std::fs::create_dir_all(&d);
        // Nach einem Update übergibt die alte Version Datei und Speichern-Status.
        let handoff = crate::selfupdate::take_handoff();
        if let Ok(bytes) = std::fs::read(d.join("session.spritebit")) {
            if let Ok(p) = load_native(&bytes) {
                let _ = std::fs::write(d.join("backup.spritebit"), &bytes);
                match handoff {
                    Some(h) => {
                        self.replace_project(p, h.path);
                        self.dirty = h.dirty;
                        self.hint = Some(trf("Aktualisiert auf spritebit {v} — alles ist wieder da.", &[("v", &crate::VERSION)]));
                    }
                    None => {
                        self.replace_project(p, None);
                        self.dirty = true;
                        self.hint = Some(tr("Die letzte Sitzung wurde nicht sauber beendet — ihr Stand ist wiederhergestellt.").into());
                    }
                }
            }
        }
        self.view.saved_version = self.version;
    }

    /// Nach jeder Änderung: kurz warten, dann die Sitzung schreiben.
    pub(crate) fn autosave(&mut self, ctx: &egui::Context) {
        if !self.view.persist || self.version == self.view.saved_version {
            return;
        }
        let now = Instant::now();
        match self.view.last_change {
            Some((v, t)) if v == self.version => {
                if now.duration_since(t) >= Duration::from_secs(2) {
                    if let Some(d) = dir() {
                        let _ = std::fs::write(d.join("session.spritebit"), save_native(&self.project));
                    }
                    self.view.saved_version = self.version;
                    self.view.last_change = None;
                } else {
                    ctx.request_repaint_after(Duration::from_millis(500));
                }
            }
            _ => {
                self.view.last_change = Some((self.version, now));
                ctx.request_repaint_after(Duration::from_secs(2));
            }
        }
    }

    /// Die Sitzung sofort schreiben (vor dem Neustart nach einem Update).
    pub(crate) fn write_session_now(&mut self) -> bool {
        let Some(d) = dir() else { return false };
        let _ = std::fs::create_dir_all(&d);
        let ok = std::fs::write(d.join("session.spritebit"), save_native(&self.project)).is_ok();
        if ok {
            self.view.saved_version = self.version;
        }
        ok
    }

    /// Sauber beendet: die laufende Sitzung braucht keine Rettung.
    pub(crate) fn end_session(&self) {
        if let (true, Some(d)) = (self.view.persist, dir()) {
            let _ = std::fs::remove_file(d.join("session.spritebit"));
        }
    }
}

/// Zeitpunkt der Sicherung, lesbar.
fn backup_time() -> Option<String> {
    let m = std::fs::metadata(dir()?.join("backup.spritebit")).ok()?.modified().ok()?;
    let secs = m.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    // Ohne Zeitzonen-Bibliothek: UTC, Tag und Uhrzeit.
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, mo, d) = civil(days);
    Some(format!("{d:02}.{mo:02}.{y} {:02}:{:02} UTC", rest / 3600, rest % 3600 / 60))
}

/// Tage seit 1970 → (Jahr, Monat, Tag).
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

pub(crate) fn help_sections() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (tr("Arbeitsfläche"), vec![
            tr("Reiter über der Zeichenfläche zeigen die geöffneten Sprites: Klick wechselt, × oder Mittelklick schließt, Ziehen ordnet."),
            tr("In Feldern für Größen darf man rechnen: 24 * 4, 24x4, (16+8)*2 oder 96 : 4."),
        ]),
        (tr("Farbpaletten"), vec![
            tr("Eine Palette ordnet jeder Nummer eine Farbe zu. 0 ist immer transparent."),
            tr("Jeder Sprite merkt sich seine eigene Palette. Eine Farbe ändern färbt alle Pixel mit dieser Nummer sofort um."),
            tr("Eingebaute Paletten sind schreibgeschützt — „Kopie bearbeiten“ macht eine änderbare eigene daraus."),
            tr("Farben in der Farbzeile ziehen sortiert um, „Nach Farbstufen“ ordnet automatisch — das Bild bleibt gleich."),
        ]),
        (tr("Werkzeuge"), vec![
            tr("Stift — einzelne Pixel. Pinsel — Fläche; Stärke = Dichte, Größe = Kantenlänge. Spray — zufällige Pixel; Stärke = Menge."),
            tr("Füllen — zusammenhängende gleiche Fläche. Radierer — setzt auf transparent. Zauberstab — löscht zusammenhängende ähnliche Fläche."),
            tr("Füllen mit „Grenzen: alle Ebenen“: die Fläche endet, wo sich im sichtbaren Bild etwas ändert — gemalt wird in die aktive Ebene. So malst du eine Vorlage auf eigener Ebene aus."),
            tr("Größe 1–1000 per Regler oder Zahlenfeld; Alt + rechte Maustaste ziehen verstellt sie direkt auf der Fläche. Ein Umriss zeigt, was Pinsel, Radierer und Spray gleich treffen."),
            tr("Clean Stroke — beim Stift (und beim Radierer mit Größe 1) verschwinden die L-Ecken einer freihändigen Linie: saubere 1-Pixel-Linien."),
            tr("Linie · Rechteck · Ellipse — aufziehen, Loslassen zeichnet. „Gefüllt“ schaltet zwischen Kontur und Fläche."),
            tr("Umschalt beim Malen: nur waagerecht, senkrecht oder 45°. Bei den Formen rastet die Linie ein, Rechteck und Ellipse werden Quadrat und Kreis."),
            tr("Hand — schiebt nur die Ansicht; auf einer Hilfslinie zieht sie die Linie. Verschieben geht jederzeit auch mit gehaltener Leertaste oder der mittleren Maustaste."),
        ]),
        (tr("Symmetrie"), vec![tr("Die Knöpfe ↔ und ↕ spiegeln jeden Strich an der Mittelachse — beide zusammen ergeben vier Spiegelungen.")]),
        (tr("Auswahl"), vec![
            tr("Auswahl (A) zieht ein Rechteck auf, Lasso (L) umfährt eine freie Form, Farbwahl (K) nimmt die zusammenhängende ähnliche Fläche."),
            tr("In die Auswahl fassen und ziehen hebt den Inhalt an — er schwebt, bis du ihn absetzt. Alt+Ziehen verschiebt eine Kopie."),
            tr("Pfeiltasten verschieben pixelweise. Esc oder ein Klick daneben hebt die Auswahl auf."),
            tr("Einfügen in einen Sprite mit anderer Palette überträgt nach der Farbe — es sieht aus wie im Original. Strg+Umschalt+V übernimmt stattdessen die Nummern."),
            tr("Skalieren: an den acht Anfassern ziehen — Ecken ändern Breite und Höhe, Kanten nur eine; Umschalt hält das Seitenverhältnis. Pixel bleiben scharf, gerechnet wird immer vom Original."),
        ]),
        (tr("Bild"), vec![
            tr("Spiegeln und Drehen wirken auf die Auswahl, wenn es eine gibt — sonst auf den ganzen Sprite."),
            tr("Frei drehen: der Regler zeigt eine Vorschau; Enter übernimmt, Esc geht zurück auf 0°."),
            tr("Zuschneiden, Zentrieren, Leinwand (ohne zu skalieren) und ×2 / ÷2 wirken auf alle Frames."),
        ]),
        (tr("Hilfslinien"), vec![
            tr("Freie Linien und Figuren-Proportionen (2–8 Kopfhöhen) — nur zum Zeichnen, nie im Export. G blendet sie ein und aus."),
            tr("Linien mit der Hand (H) ziehen — eine Kopfhöhe zieht die ganze Figur; aus dem Bild gezogen ist eine Linie gelöscht. „Sperren“ hält sie fest."),
            tr("Gleichmäßig: eine Anzahl eintippen (z. B. 4 waagerecht, 8 senkrecht) — die Linien verteilen sich sofort gleichmäßig."),
            tr("Eigene Layouts: Linien und Einteilung unter einem Namen speichern und auf jeden Sprite anwenden — bei anderer Größe anteilig umgerechnet."),
        ]),
        (tr("Ebenen und Animation"), vec![
            tr("Gemalt wird in die aktive Ebene; angezeigt werden alle sichtbaren übereinander. Auge blendet aus, Schloss sperrt."),
            tr("In der Timeline: Klick wählt, Strg+Klick und Umschalt+Klick markieren mehrere Frames, Ziehen sortiert Frames und Ebenen um."),
            tr("Ebenen: nach unten oder alle sichtbaren zusammenführen. Eine Maske blendet Teile einer Ebene aus, ohne sie zu löschen — beim Bearbeiten blendet Malen aus und Radieren wieder ein."),
            tr("Zellen: Umschalt+Klick oder Ziehen spannt einen Bereich auf — kopieren, einfügen, leeren, verknüpfen (die Frames teilen sich ein Bild) oder lösen. „Durchgehend“ lässt neue Frames das Bild des vorigen teilen."),
            tr("Tags benennen einen Abschnitt (z. B. „Laufen“) mit Richtung — vorwärts, rückwärts, Ping-Pong; GIFs gehen auch je Tag."),
            tr("Onion Skin lässt Nachbar-Frames durchscheinen — einstellbar im Zahnrad-Menü der Timeline."),
        ]),
        (tr("Licht und Schatten"), vec![
            tr("Panel Licht: Lichtquelle aus 8 Richtungen, Stärke und Breite der Kanten, auf Wunsch Schlagschatten. Solange es offen ist, zeigt die Fläche eine Vorschau."),
            tr("„Als Ebene übernehmen“ legt Licht und Schatten als eigene, gesperrte Ebenen an — für alle Frames. Danach rechnet jede Änderung die Ebenen neu."),
        ]),
        (tr("Kacheln (Tilemaps)"), vec![
            tr("Eine Tilemap-Ebene besteht aus Kacheln fester Größe (8–64 px). Malt man eine Kachel an, ändert sie sich überall, wo sie liegt — auch in anderen Frames."),
            tr("Panel Kacheln: neue Tilemap-Ebene oder die aktive umwandeln. Pixel malen: Auto legt in leeren Zellen neue Kacheln an, Manuell nicht."),
            tr("Kacheln setzen: Kachel in der Liste wählen, dann setzt der Stift sie, Radierer oder Rechtsklick leert, Füllen füllt, Alt+Klick nimmt eine Kachel auf."),
            tr("„Für Godot exportieren“ schreibt einen Ordner mit Kachelbild (PNG), Szene (.tscn mit TileMapLayer) und JSON — in den Godot-Projektordner legen (Godot 4.3 oder neuer)."),
        ]),
        (tr("Schablone"), vec![
            tr("Bild laden, mit Umschalt+Alt ziehen verschieben, Deckkraft und Größe per Regler; Umschalt+Alt halten zeigt sie vorn."),
            tr("Vom Foto zum Sprite: Schablone laden, „Reduzieren auf“ übernehmen, „Bild » Palette“, Hintergrund entfernen, glätten, Outline."),
        ]),
        (tr("Import und Export"), vec![
            tr("Code & Export erzeugt TypeScript, JavaScript, JSON, JSON (Spiel), SVG, CSS, C-Header, Python oder ein Text-Raster — und liest alles davon wieder ein."),
            tr("PNG und PDF exportieren den aktuellen (oder die markierten) Frames, GIF die Animation, das Spritesheet alle Sprites samt JSON-Atlas."),
        ]),
    ]
}

pub(crate) fn help_keys() -> Vec<(&'static str, &'static str)> {
    vec![
        (tr("Klick"), tr("Malen")),
        (tr("Rechtsklick"), tr("Löschen (gedrückt halten = durchgehend)")),
        ("Alt + Klick", tr("Pipette")),
        (tr("Umschalt + Malen"), tr("Nur waagerecht, senkrecht oder 45° · Formen: einrasten, Quadrat, Kreis")),
        (tr("Alt + Klick (Kacheln setzen)"), tr("Kachel aufnehmen")),
        (tr("Alt + Rechts ziehen"), tr("Größe von Pinsel, Radierer und Spray")),
        ("Shift + Alt", tr("Schablone: halten = vorn, ziehen = verschieben, Klick = Farbe")),
        ("0 – 9", tr("Farbe mit dieser Nummer")),
        ("P B S F E W", tr("Stift · Pinsel · Spray · Füllen · Radierer · Zauberstab")),
        ("I R O", tr("Linie · Rechteck · Ellipse")),
        ("A L K", tr("Auswahl · Lasso · Farbwahl")),
        ("H", tr("Hand — Ansicht verschieben")),
        (tr("Mausrad"), tr("Hoch / runter scrollen (Umschalt: links / rechts)")),
        (tr("Strg + Mausrad"), tr("Zoomen auf den Mauszeiger")),
        (tr("Leertaste + Ziehen"), tr("Bild verschieben (auch mit mittlerer Maustaste)")),
        (tr("Pfeiltasten"), tr("Mit Auswahl: sie pixelweise verschieben · ohne: links / rechts Frame, hoch / runter Ebene")),
        (tr("Anfasser ziehen"), tr("Auswahl skalieren (Umschalt: Seitenverhältnis halten)")),
        (tr("Strg + A / C / X / V"), tr("Alles · Kopieren · Ausschneiden · Einfügen")),
        (tr("Strg + Umschalt + V"), tr("Einfügen mit den Nummern statt den Farben (andere Palette)")),
        (tr("Entf"), tr("Auswahl leeren")),
        ("Enter", tr("Drehung übernehmen · sonst Animation abspielen / anhalten")),
        (tr("Pos1 / Ende"), tr("Zum ersten / letzten Frame")),
        (", / .", tr("Voriger / nächster Frame")),
        ("G", tr("Hilfslinien ein / aus")),
        (tr("Strg + Z / Y"), tr("Rückgängig / Wiederholen")),
        (tr("Strg + S / O / E"), tr("Speichern / Öffnen / Exportieren")),
        (tr("Strg + Tab"), tr("Nächster Reiter (mit Umschalt: voriger)")),
        (tr("Strg + W"), tr("Reiter schließen")),
        ("F1", tr("Hilfe")),
        ("F11", tr("Vollbild")),
        (tr("Strg + D"), tr("Auswahl aufheben")),
        (tr("Strg + N"), tr("Neues Projekt")),
        (tr("Strg + Alt + N"), tr("Neuer Sprite")),
        ("Esc", tr("Auswahl aufheben, Drehung verwerfen oder Vollbild beenden")),
    ]
}
