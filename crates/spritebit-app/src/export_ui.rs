//! Export-Dialog: PNG (aktueller Frame oder alle), GIF (ganz oder je Tag),
//! Spritesheet mit JSON-Atlas.

use std::path::{Path, PathBuf};

use eframe::egui;
use spritebit_core::export;

use crate::SpritebitApp;
use crate::i18n::{tr, trf};
use crate::i18n;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportKind {
    PngFrame,
    PngAll,
    Gif,
    GifTags,
    Sheet,
}

impl ExportKind {
    fn label(self) -> &'static str {
        match self {
            ExportKind::PngFrame => tr("PNG — aktueller Frame"),
            ExportKind::PngAll => tr("PNG — jeder Frame eine Datei"),
            ExportKind::Gif => tr("GIF — ganze Animation"),
            ExportKind::GifTags => tr("GIF — eine Datei je Tag"),
            ExportKind::Sheet => tr("Spritesheet (PNG + JSON-Atlas, alle Sprites)"),
        }
    }
}

pub(crate) struct ExportDialog {
    pub kind: ExportKind,
    pub scale: u32,
}

/// Dateiname aus dem Sprite-Namen: nur Buchstaben, Ziffern, _ und -.
fn safe(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    if s.trim_matches('_').is_empty() { "sprite".into() } else { s }
}

impl SpritebitApp {
    pub(crate) fn open_export(&mut self) {
        self.commit_float();
        let kind = if self.project.sprite().frames.len() > 1 { ExportKind::Gif } else { ExportKind::PngFrame };
        self.export_dialog = Some(ExportDialog { kind, scale: 4 });
    }

    pub(crate) fn export_window(&mut self, ctx: &egui::Context) {
        let mut go = false;
        let mut close = false;
        let has_tags = !self.project.sprite().tags.is_empty();
        let multi = self.project.sprite().frames.len() > 1;
        if let Some(d) = &mut self.export_dialog {
            egui::Window::new(tr("Exportieren")).collapsible(false).resizable(false).show(ctx, |ui| {
                for k in [ExportKind::PngFrame, ExportKind::PngAll, ExportKind::Gif, ExportKind::GifTags, ExportKind::Sheet] {
                    let ok = match k {
                        ExportKind::PngAll | ExportKind::Gif => multi,
                        ExportKind::GifTags => has_tags,
                        _ => true,
                    };
                    ui.add_enabled_ui(ok, |ui| ui.radio_value(&mut d.kind, k, k.label()));
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(tr("Vergrößerung"));
                    for s in [1, 2, 4, 8, 16] {
                        ui.selectable_value(&mut d.scale, s, format!("{s}×"));
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr("Exportieren …")).clicked() {
                        go = true;
                    }
                    if ui.button(tr("Abbrechen")).clicked() {
                        close = true;
                    }
                });
            });
        }
        if close {
            self.export_dialog = None;
        }
        if go {
            if let Some(d) = self.export_dialog.take() {
                match self.run_export(d.kind, d.scale) {
                    Ok(Some(msg)) => self.hint = Some(msg),
                    Ok(None) => {}
                    Err(e) => self.error = Some(e),
                }
            }
        }
    }

    fn save_dialog(title: &str, ext: &str, name: String) -> Option<PathBuf> {
        let path = rfd::FileDialog::new().set_title(title).add_filter(ext.to_uppercase(), &[ext]).set_file_name(name).save_file()?;
        Some(if path.extension().is_none() { path.with_extension(ext) } else { path })
    }

    fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
        std::fs::write(path, bytes).map_err(|e| trf("{path} konnte nicht geschrieben werden: {e}", &[("path", &path.display()), ("e", &e)]))
    }

    /// Führt den Export aus. `Ok(None)` = abgebrochen.
    fn run_export(&mut self, kind: ExportKind, scale: u32) -> Result<Option<String>, String> {
        let sp = self.project.sprite().clone();
        let pal = self.project.current_palette();
        let base = safe(&sp.name);
        let err = |e: export::ExportError| i18n::export_error(&e);
        match kind {
            ExportKind::PngFrame => {
                let Some(path) = Self::save_dialog(tr("PNG exportieren"), "png", format!("{base}.png")) else { return Ok(None) };
                Self::write(&path, &export::png(&sp, &pal, sp.frame, scale).map_err(err)?)?;
                Ok(Some(trf("Gespeichert: {path}", &[("path", &path.display())])))
            }
            ExportKind::PngAll => {
                let Some(dir) = rfd::FileDialog::new().set_title(tr("Ordner für die Frames")).pick_folder() else { return Ok(None) };
                let pad = sp.frames.len().to_string().len();
                for f in 0..sp.frames.len() {
                    let path = dir.join(format!("{base}_f{:0pad$}.png", f + 1));
                    Self::write(&path, &export::png(&sp, &pal, f, scale).map_err(err)?)?;
                }
                Ok(Some(trf("{n} PNGs in {dir}", &[("n", &sp.frames.len()), ("dir", &dir.display())])))
            }
            ExportKind::Gif => {
                let Some(path) = Self::save_dialog(tr("GIF exportieren"), "gif", format!("{base}.gif")) else { return Ok(None) };
                let frames: Vec<usize> = (0..sp.frames.len()).collect();
                Self::write(&path, &export::gif(&sp, &pal, &frames, scale).map_err(err)?)?;
                Ok(Some(trf("Gespeichert: {path}", &[("path", &path.display())])))
            }
            ExportKind::GifTags => {
                let Some(dir) = rfd::FileDialog::new().set_title(tr("Ordner für die GIFs")).pick_folder() else { return Ok(None) };
                for t in &sp.tags {
                    let path = dir.join(format!("{base}_{}.gif", safe(&t.name)));
                    Self::write(&path, &export::gif(&sp, &pal, &export::tag_frames(t), scale).map_err(err)?)?;
                }
                Ok(Some(trf("{n} GIFs in {dir}", &[("n", &sp.tags.len()), ("dir", &dir.display())])))
            }
            ExportKind::Sheet => {
                let Some(path) = Self::save_dialog(tr("Spritesheet exportieren"), "png", format!("{base}_sheet.png")) else { return Ok(None) };
                let image_name = path.file_name().map_or_else(|| "sheet.png".to_string(), |n| n.to_string_lossy().into_owned());
                let (png, json) = export::sheet(&self.project, scale, &image_name).map_err(err)?;
                Self::write(&path, &png)?;
                let json_path = path.with_extension("json");
                Self::write(&json_path, json.as_bytes())?;
                Ok(Some(trf("Gespeichert: {a} und {b}", &[("a", &path.display()), ("b", &json_path.display())])))
            }
        }
    }
}
