//! Panel „Code & Export“ — wie in der Web-Version.
//!
//! * Code: Format wählen (TypeScript … Text-Raster), Palette in den Code,
//!   Vorschau, Kopieren, Speichern; bei „JSON (Spiel)“ das Material je Farbe.
//!   Große Sprites (über 300 000 Pixel über alle Frames) bekommen keine
//!   Live-Vorschau — der Code entsteht beim Kopieren oder Speichern.
//! * Import …: Code einfügen oder Datei wählen, das Format wird erkannt;
//!   in den aktuellen Sprite oder als neuen, Palette übernehmen.
//! * Leeren: alle Pixel der aktiven Zelle weg (ein Undo-Schritt).
//! * Bild: Skalierung, PNG, PDF, GIF (oder eines je Tag), Spritesheet mit
//!   Atlas, Farb-Legende ins Bild. Sind in der Timeline mehrere Frames
//!   markiert, speichern PNG und PDF je eine Datei, das GIF nur diese.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use eframe::egui;
use spritebit_core::codegen::{self, CodeLang, Format, MATERIALS};
use spritebit_core::codeimport;
use spritebit_core::{builtin, export, History, Palette, Rgb};

use crate::i18n::{self, tr, trf};
use crate::SpritebitApp;

/// Bis zu so vielen Pixeln (über alle Frames) steht der Code live da.
const LIVE_CODE_MAX: u64 = 300_000;
/// Längere Vorschauen werden gekürzt angezeigt (Kopieren/Speichern nimmt alles).
const SHOW_MAX: usize = 200_000;

pub(crate) struct ImportModal {
    pub text: String,
    pub use_palette: bool,
}

/// Schlüssel der Code-Vorschau: Sprite, Stand, Format, Palette, Sprache.
type CodeKey = (usize, u64, Format, bool, CodeLang);

pub(crate) struct OutState {
    pub format: Format,
    pub with_palette: bool,
    pub scale: u32,
    pub legend: bool,
    pub gif_tags: bool,
    /// Panel aufklappen und hinscrollen (Menü „Exportieren …“).
    pub focus: bool,
    cache: Option<(CodeKey, String)>,
    pub import: Option<ImportModal>,
}

impl Default for OutState {
    fn default() -> Self {
        OutState { format: Format::Ts, with_palette: false, scale: 8, legend: false, gif_tags: false, focus: false, cache: None, import: None }
    }
}

fn format_label(f: Format) -> &'static str {
    match f {
        Format::Game => tr("JSON (Spiel)"),
        Format::Svg => tr("SVG-Bild"),
        Format::Txt => tr("Text-Raster"),
        _ => f.label(),
    }
}

/// Dateiname aus dem Sprite-Namen wie im Web: nur a–z, 0–9, _ und -.
fn safe(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    if s.is_empty() { "sprite".into() } else { s }
}

fn code_lang() -> CodeLang {
    if i18n::lang() == i18n::Lang::En { CodeLang::En } else { CodeLang::De }
}

impl SpritebitApp {
    /// Menü „Exportieren …“ (Strg+E): das Panel aufklappen.
    pub(crate) fn open_export(&mut self) {
        self.commit_float();
        self.out.focus = true;
    }

    fn legend_text(&self) -> Option<export::LegendText> {
        self.out.legend.then(|| export::LegendText { title: tr("Palette — {n} Farben").into(), sorted: tr("Palette — {n} Farben (nach Farbton sortiert)").into() })
    }

    /// Frames für PNG/PDF/GIF: die in der Timeline markierten, sonst der aktive.
    fn frames_to_export(&self) -> Vec<usize> {
        let sp = self.sprite();
        match self.cel_range.and_then(|r| r.clamp(sp)) {
            Some(r) if r.f1 > r.f0 => (r.f0..=r.f1).collect(),
            _ => vec![sp.frame],
        }
    }

    fn materials(&self) -> BTreeMap<u16, String> {
        self.project.materials.get(&self.sprite().palette).cloned().unwrap_or_default()
    }

    /// Der ganze Code im gewählten Format.
    fn build_code(&self) -> String {
        let sp = self.sprite();
        codegen::build(sp, &self.project.current_palette(), self.out.format, self.out.with_palette, code_lang(), &self.materials())
    }

    fn save_dialog(title: &str, ext: &str, name: String) -> Option<PathBuf> {
        let path = rfd::FileDialog::new().set_title(title).add_filter(ext.to_uppercase(), &[ext]).set_file_name(name).save_file()?;
        Some(if path.extension().is_none() { path.with_extension(ext) } else { path })
    }

    fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
        std::fs::write(path, bytes).map_err(|e| trf("{path} konnte nicht geschrieben werden: {e}", &[("path", &path.display()), ("e", &e)]))
    }

    fn report_export(&mut self, r: Result<Option<String>, String>) {
        match r {
            Ok(Some(msg)) => self.hint = Some(msg),
            Ok(None) => {}
            Err(e) => self.error = Some(e),
        }
    }

    // ── Bild-Export ─────────────────────────────────────────────────
    fn export_image(&mut self, ext: &'static str) -> Result<Option<String>, String> {
        self.commit_float();
        let sp = self.sprite().clone();
        let pal = self.project.current_palette();
        let (scale, legend) = (self.out.scale, self.legend_text());
        let make = |f: usize| -> Result<Vec<u8>, String> {
            let r = if ext == "pdf" { export::pdf(&sp, &pal, f, scale, legend.as_ref()) } else { export::png_with_legend(&sp, &pal, f, scale, legend.as_ref()) };
            r.map_err(|e| i18n::export_error(&e))
        };
        let base = safe(&sp.name);
        let ids = self.frames_to_export();
        if ids.len() < 2 {
            let title = if ext == "pdf" { tr("PDF exportieren") } else { tr("PNG exportieren") };
            let Some(path) = Self::save_dialog(title, ext, format!("{base}.{ext}")) else { return Ok(None) };
            Self::write(&path, &make(ids[0])?)?;
            return Ok(Some(trf("Gespeichert: {path}", &[("path", &path.display())])));
        }
        let Some(dir) = rfd::FileDialog::new().set_title(tr("Ordner für die Frames")).pick_folder() else { return Ok(None) };
        let pad = sp.frames.len().to_string().len();
        let names: Vec<String> = ids.iter().map(|f| format!("{base}_f{:0pad$}.{ext}", f + 1)).collect();
        for (f, name) in ids.iter().zip(&names) {
            Self::write(&dir.join(name), &make(*f)?)?;
        }
        Ok(Some(trf(
            "{n} Frames gespeichert — von „{first}“ bis „{last}“ in „{dir}“.",
            &[("n", &ids.len()), ("first", &names[0]), ("last", &names[names.len() - 1]), ("dir", &dir.display())],
        )))
    }

    fn export_gif(&mut self) -> Result<Option<String>, String> {
        self.commit_float();
        let sp = self.sprite().clone();
        let pal = self.project.current_palette();
        let scale = self.out.scale;
        let base = safe(&sp.name);
        let err = |e: export::ExportError| i18n::export_error(&e);
        if self.out.gif_tags && !sp.tags.is_empty() {
            let Some(dir) = rfd::FileDialog::new().set_title(tr("Ordner für die GIFs")).pick_folder() else { return Ok(None) };
            for t in &sp.tags {
                let path = dir.join(format!("{base}_{}.gif", safe(&t.name)));
                Self::write(&path, &export::gif(&sp, &pal, &export::tag_frames(t), scale).map_err(err)?)?;
            }
            return Ok(Some(trf("{n} GIFs — eine je Tag.", &[("n", &sp.tags.len())])));
        }
        let ids = self.frames_to_export();
        let frames: Vec<usize> = if ids.len() > 1 { ids } else { (0..sp.frames.len()).collect() };
        let Some(path) = Self::save_dialog(tr("GIF exportieren"), "gif", format!("{base}.gif")) else { return Ok(None) };
        Self::write(&path, &export::gif(&sp, &pal, &frames, scale).map_err(err)?)?;
        Ok(Some(trf("Gespeichert: {path}", &[("path", &path.display())])))
    }

    fn export_sheet(&mut self) -> Result<Option<String>, String> {
        self.commit_float();
        let base = safe(&self.sprite().name);
        let Some(path) = Self::save_dialog(tr("Spritesheet exportieren"), "png", format!("{base}_sheet.png")) else { return Ok(None) };
        let image_name = path.file_name().map_or_else(|| "sheet.png".to_string(), |n| n.to_string_lossy().into_owned());
        let (png, json) = export::sheet(&self.project, self.out.scale, &image_name).map_err(|e| i18n::export_error(&e))?;
        Self::write(&path, &png)?;
        let json_path = path.with_extension("json");
        Self::write(&json_path, json.as_bytes())?;
        let name = |p: &Path| p.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let dir = path.parent().map_or_else(String::new, |d| d.display().to_string());
        Ok(Some(trf(
            "Spritesheet mit {n} Sprites gespeichert — „{png}“ und „{json}“ in „{dir}“.",
            &[("n", &self.project.sprites.len()), ("png", &name(&path)), ("json", &name(&json_path)), ("dir", &dir)],
        )))
    }

    // ── Panel ───────────────────────────────────────────────────────
    pub(crate) fn output_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(tr("Format"));
            let mut f = self.out.format;
            egui::ComboBox::from_id_salt("out-format").selected_text(format_label(f)).show_ui(ui, |ui| {
                for k in Format::ALL {
                    ui.selectable_value(&mut f, k, format_label(k));
                }
            });
            self.out.format = f;
        });
        let fmt = self.out.format;
        ui.add_enabled_ui(fmt.pal_option(), |ui| ui.checkbox(&mut self.out.with_palette, tr("Palette in den Code schreiben")));

        // Code-Vorschau
        let sp = self.sprite();
        let cells = sp.width as u64 * sp.height as u64 * sp.frames.len() as u64;
        let key = (self.project.current, self.version, fmt, self.out.with_palette, code_lang());
        let text = if cells > LIVE_CODE_MAX {
            tr("// Der Sprite ist groß ({n} Pixel über alle Frames) — der Code wird erst beim Kopieren oder Speichern erzeugt.").replace("{n}", &cells.to_string())
        } else {
            if self.out.cache.as_ref().is_none_or(|c| c.0 != key) {
                self.out.cache = Some((key, self.build_code()));
            }
            self.out.cache.as_ref().map_or_else(String::new, |c| c.1.clone())
        };
        let mut shown = if text.len() > SHOW_MAX {
            let mut cut = SHOW_MAX;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            format!("{}\n…", &text[..cut])
        } else {
            text
        };
        egui::ScrollArea::vertical().id_salt("out-code").max_height(200.0).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut shown).code_editor().desired_width(f32::INFINITY));
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Kopieren")).clicked() {
                let code = self.build_code();
                ui.ctx().copy_text(code);
                self.hint = Some(tr("Code kopiert.").into());
            }
            if ui.button(tr("Speichern …")).clicked() {
                let name = codegen::filename(fmt, &self.sprite().name);
                if let Some(path) = rfd::FileDialog::new().set_title(tr("Code speichern")).set_file_name(name).save_file() {
                    let r = Self::write(&path, self.build_code().as_bytes()).map(|_| Some(trf("Gespeichert: {path}", &[("path", &path.display())])));
                    self.report_export(r);
                }
            }
            if ui.button(tr("Import …")).clicked() {
                self.out.import = Some(ImportModal { text: String::new(), use_palette: true });
            }
            if ui.button(tr("Leeren")).on_hover_text(tr("Alle Pixel dieses Sprites löschen")).clicked() && self.layer_ok() {
                self.deselect();
                let cur = self.project.current;
                self.histories[cur].record(&self.project.sprites[cur]);
                let (w, h) = (self.sprite().width, self.sprite().height);
                *self.project.sprite_mut().active() = spritebit_core::Image::new(w, h);
                self.changed();
            }
        });

        // Material je Farbe (JSON Spiel)
        if fmt.materials() {
            self.materials_box(ui);
        }

        // Bild-Export
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(tr("Skalierung"));
            egui::ComboBox::from_id_salt("out-scale").selected_text(format!("{}×", self.out.scale)).width(60.0).show_ui(ui, |ui| {
                for s in [1, 4, 8, 16, 32] {
                    ui.selectable_value(&mut self.out.scale, s, format!("{s}×"));
                }
            });
        });
        let marked = self.frames_to_export().len();
        if marked > 1 {
            ui.weak(trf("{n} Frames markiert — PNG und PDF speichern je eine Datei, das GIF enthält nur diese Frames.", &[("n", &marked)]));
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("PNG").clicked() {
                let r = self.export_image("png");
                self.report_export(r);
            }
            if ui.button("PDF").clicked() {
                let r = self.export_image("pdf");
                self.report_export(r);
            }
            if ui.button("GIF").on_hover_text(tr("Animation als GIF — alle Frames, läuft endlos")).clicked() {
                let r = self.export_gif();
                self.report_export(r);
            }
            if ui.button(tr("Spritesheet")).on_hover_text(tr("Alle Sprites in einem Bild — dazu ein JSON-Atlas mit Namen und Koordinaten")).clicked() {
                let r = self.export_sheet();
                self.report_export(r);
            }
        });
        if !self.sprite().tags.is_empty() {
            ui.checkbox(&mut self.out.gif_tags, tr("GIF: eine Datei je Tag"));
        }
        ui.checkbox(&mut self.out.legend, tr("Farb-Legende ins Bild")).on_hover_text(tr("Bettet eine Farb-Legende ins Bild ein, damit die Farbwerte nicht verloren gehen"));
    }

    fn materials_box(&mut self, ui: &mut egui::Ui) {
        let pal = self.project.current_palette();
        ui.label(trf("Material je Farbe — Palette „{name}“", &[("name", &pal.name)]));
        let mut mats = self.materials();
        let mut changed = false;
        egui::ScrollArea::vertical().id_salt("out-mats").max_height(160.0).show(ui, |ui| {
            for (k, c) in pal.colors.iter().enumerate() {
                let i = k as u16 + 1;
                ui.horizontal(|ui| {
                    ui.monospace(format!("{i:>3}"));
                    let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                    let before = mats.get(&i).cloned().unwrap_or_else(|| codegen::DEFAULT_MATERIAL.to_string());
                    let mut m = before.clone();
                    egui::ComboBox::from_id_salt(("mat", i)).selected_text(&m).width(90.0).show_ui(ui, |ui| {
                        for opt in MATERIALS {
                            ui.selectable_value(&mut m, opt.to_string(), opt);
                        }
                    });
                    if m != before {
                        if m == codegen::DEFAULT_MATERIAL {
                            mats.remove(&i);
                        } else {
                            mats.insert(i, m);
                        }
                        changed = true;
                    }
                });
            }
        });
        ui.weak(tr("Gilt für alle Sprites mit dieser Palette. Index 0 ist immer „empty“, freie Farben bekommen „none“."));
        if changed {
            if mats.is_empty() {
                self.project.materials.remove(&pal.name);
            } else {
                self.project.materials.insert(pal.name, mats);
            }
            self.changed();
        }
    }

    // ── Import ──────────────────────────────────────────────────────
    pub(crate) fn import_window(&mut self, ctx: &egui::Context) {
        let Some(m) = &mut self.out.import else { return };
        let parsed = (!m.text.trim().is_empty()).then(|| codeimport::parse(&m.text));
        let (mut load_file, mut current, mut new, mut cancel) = (false, false, false, false);
        egui::Modal::new(egui::Id::new("import")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading(tr("Sprite importieren"));
            ui.label(tr("TypeScript, JavaScript, JSON, JSON (Spiel), Python und C-Header behalten ihre Farb-Nummern; SVG, CSS und Text-Raster werden neu durchnummeriert."));
            load_file = ui.button(tr("Datei wählen")).clicked();
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut m.text).code_editor().desired_rows(12).desired_width(f32::INFINITY).hint_text(tr("Code hier einfügen …")));
            });
            match &parsed {
                Some(Ok(r)) => {
                    let s = &r.stats;
                    let mut parts: Vec<String> = Vec::new();
                    if s.format != "Array" {
                        parts.push(format_name(s.format).to_string());
                    }
                    parts.push(trf("{w}×{h} Pixel", &[("w", &s.w), ("h", &s.h)]));
                    if s.frames > 1 {
                        parts.push(trf("{n} Frames", &[("n", &s.frames)]));
                    }
                    parts.push(if s.palette_count > 0 { trf("Palette mit {n} Farben", &[("n", &s.palette_count)]) } else { tr("keine Palette gefunden").into() });
                    if s.restored > 0 {
                        parts.push(trf("{n} freie Farb-Pixel wiederhergestellt", &[("n", &s.restored)]));
                    }
                    if let Some(n) = &r.name {
                        parts.push(trf("Name „{name}“", &[("name", n)]));
                    }
                    let mut msg = format!("{}{}", tr("Erkannt: "), parts.join(" · "));
                    if !s.unknown.is_empty() {
                        let list = s.unknown.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
                        msg += &trf(" — Achtung: Index {list} kommt im Grid vor, fehlt aber in der Palette.", &[("list", &list)]);
                    }
                    ui.colored_label(egui::Color32::from_rgb(0x8f, 0xd1, 0x9e), msg);
                }
                Some(Err(e)) => {
                    ui.colored_label(egui::Color32::from_rgb(0xf2, 0x8b, 0x82), import_error(e));
                }
                None => {}
            }
            let has_pal = matches!(&parsed, Some(Ok(r)) if r.stats.palette_count > 0);
            ui.add_enabled(has_pal, egui::Checkbox::new(&mut m.use_palette, tr("Palette aus der Datei übernehmen und dem Sprite zuweisen")));
            ui.separator();
            ui.horizontal(|ui| {
                let ok = matches!(parsed, Some(Ok(_)));
                current = ui.add_enabled(ok, egui::Button::new(tr("In aktuellen Sprite"))).clicked();
                new = ui.add_enabled(ok, egui::Button::new(tr("Als neuen Sprite"))).clicked();
                cancel = ui.button(tr("Abbrechen")).clicked();
            });
        });
        if load_file {
            if let Some(path) = rfd::FileDialog::new().set_title(tr("Datei wählen")).pick_file() {
                match std::fs::read(&path) {
                    Ok(b) => m.text = String::from_utf8_lossy(&b).into_owned(),
                    Err(e) => self.error = Some(trf("{path} konnte nicht gelesen werden: {e}", &[("path", &path.display()), ("e", &e)])),
                }
            }
        }
        if cancel {
            self.out.import = None;
            return;
        }
        if let (true, Some(Ok(r))) = (current || new, parsed) {
            let use_pal = m.use_palette;
            self.out.import = None;
            self.apply_import(&r, new, use_pal);
        }
    }

    pub(crate) fn apply_import(&mut self, r: &codeimport::Imported, as_new: bool, use_palette: bool) {
        self.finish_rotate();
        self.deselect();
        self.playing = false;
        let cur = self.project.current;
        self.histories[cur].record_with_palettes(&self.project.sprites[cur], &self.project.palettes);
        // Palette anlegen (Lücken wie im Web aus den Graustufen, sonst grau).
        let mut pal_name = None;
        if let (true, Some(p)) = (use_palette, &r.palette) {
            let base = r.name.as_deref().unwrap_or("import").to_lowercase();
            let base: String = base.chars().map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' { c } else { '_' }).collect();
            let name = self.project.unique_palette_name(&base);
            let gray = builtin::builtin("graustufen").map(|p| p.colors).unwrap_or_default();
            let n = *p.keys().max().unwrap_or(&1) as usize;
            let colors: Vec<Rgb> = (1..=n).map(|i| p.get(&(i as u16)).copied().or_else(|| gray.get(i - 1).copied()).unwrap_or([0x88, 0x88, 0x88])).collect();
            self.project.palettes.push(Palette::new(name.clone(), colors));
            if let Some(mats) = &r.materials {
                self.project.materials.insert(name.clone(), mats.clone());
            }
            pal_name = Some(name);
        }
        let palette = pal_name.clone().unwrap_or_else(|| self.sprite().palette.clone());
        if as_new {
            // Der neue Sprite hat seine eigene (leere) Geschichte.
            self.histories[cur].drop_last();
            let name = r.name.clone().unwrap_or_else(|| tr("Import").to_string());
            if let Some(sp) = codeimport::to_sprite(r, &name, &palette) {
                self.project.sprites.push(sp);
                self.histories.push(History::default());
                self.select_sprite(self.project.sprites.len() - 1);
            }
        } else if let Some(mut sp) = codeimport::to_sprite(r, &self.sprite().name, &palette) {
            // Ersetzt den ganzen Sprite — Frames UND Ebenen; Tags, soweit es ihre Frames noch gibt.
            let old = self.project.sprite();
            if r.durations.as_ref().is_none_or(|d| !d.iter().all(|&x| x == d[0])) {
                sp.fps = old.fps;
            }
            let n = sp.frames.len();
            sp.tags = old
                .tags
                .iter()
                .filter(|t| t.from < n)
                .cloned()
                .map(|mut t| {
                    t.to = t.to.min(n - 1);
                    t
                })
                .collect();
            *self.project.sprite_mut() = sp;
            self.fit_pending = true;
        }
        self.clamp_color();
        self.changed();
        self.hint = Some(match pal_name {
            Some(n) => trf("Import fertig — Palette „{name}“ übernommen und zugewiesen.", &[("name", &n)]),
            None => tr("Import fertig. (Keine Palette im Text gefunden — Farben bleiben wie eingestellt.)").into(),
        });
    }
}

/// Name des erkannten Formats in der gewählten Sprache.
fn format_name(f: &str) -> &'static str {
    match f {
        "SVG" => "SVG",
        "CSS" => "CSS",
        "C-Header" => "C-Header",
        "JSON (Spiel)" => tr("JSON (Spiel)"),
        "Text-Raster" => tr("Text-Raster"),
        _ => "Array",
    }
}

/// Fehler beim Import in der gewählten Sprache.
pub(crate) fn import_error(e: &codeimport::ImportError) -> String {
    let mut s = tr(e.text).to_string();
    for (k, v) in &e.args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    if let Some(r) = &e.reason {
        s = s.replace("{reason}", &import_error(r));
    }
    s
}
