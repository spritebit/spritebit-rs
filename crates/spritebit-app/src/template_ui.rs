//! Panel „Schablone“: ein Foto oder Bild zum Abzeichnen — wie die
//! Web-Version.
//!
//! * Bild laden (PNG, JPEG, GIF, WebP, BMP), Deckkraft 0–100 %, Größe
//!   10–200 %, Zentrieren, Entfernen
//! * Umschalt+Alt+Ziehen verschiebt die Schablone; Umschalt+Alt+Klick nimmt
//!   ihre Farbe (Pipette, genauer Farbwert als freie Farbe)
//! * Umschalt halten: Schablone deckend VOR den Pixeln — zum Vergleichen
//! * „Aufs Raster übernehmen“: Palettenfarben, Originalfarben oder auf N
//!   Farben reduziert — in die aktive Zelle, ein Undo-Schritt
//! * Die Schablone bleibt gemerkt (im Einstellungsordner), bis man sie
//!   entfernt — wie im Browser.

use std::path::Path;

use eframe::egui::{self, Color32, Pos2, Vec2};
use spritebit_core::template::{self as tpl, Template, TraceMode};

use crate::i18n::{tr, trf};
use crate::SpritebitApp;

pub(crate) struct Loaded {
    pub template: Template,
    pub texture: egui::TextureHandle,
    pub name: String,
}

pub(crate) struct TemplateState {
    pub loaded: Option<Loaded>,
    /// 0.0–1.0
    pub opacity: f32,
    /// 0.1–2.0 (10–200 %)
    pub scale: f64,
    /// Verschiebung der Mitte in Sprite-Pixeln.
    pub offset: (f64, f64),
    /// Laufendes Verschieben: Startpunkt (Bildschirm) und Verschiebung davor.
    pub drag: Option<(Pos2, (f64, f64))>,
    pub colors: usize,
    /// Schablone im Einstellungsordner merken (nur in der echten App, nicht in Tests).
    pub persist: bool,
}

impl Default for TemplateState {
    fn default() -> Self {
        TemplateState { loaded: None, opacity: 0.5, scale: 1.0, offset: (0.0, 0.0), drag: None, colors: 8, persist: false }
    }
}

/// Bilddatei lesen → RGBA.
pub(crate) fn decode(bytes: &[u8]) -> Result<Template, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("0 × 0".into());
    }
    Ok(Template { w, h, rgba: img.into_raw() })
}

fn store() -> Option<std::path::PathBuf> {
    crate::i18n::settings_dir()
}

impl SpritebitApp {
    /// Einstellungen der Schablone merken.
    fn save_template_meta(&self) {
        let (Some(dir), Some(l)) = (store().filter(|_| self.template.persist), &self.template.loaded) else { return };
        let t = &self.template;
        let meta = serde_json::json!({ "name": l.name, "opacity": t.opacity, "scale": t.scale, "x": t.offset.0, "y": t.offset.1 });
        let _ = std::fs::write(dir.join("template.json"), meta.to_string());
    }

    /// Beim Start: die zuletzt geladene Schablone wiederherstellen.
    pub(crate) fn restore_template(&mut self, ctx: &egui::Context) {
        self.template.persist = true;
        let Some(dir) = store() else { return };
        let (Ok(bytes), Ok(meta)) = (std::fs::read(dir.join("template.img")), std::fs::read_to_string(dir.join("template.json"))) else { return };
        let Ok(t) = decode(&bytes) else { return };
        let m: serde_json::Value = serde_json::from_str(&meta).unwrap_or_default();
        self.set_template(ctx, t, m["name"].as_str().unwrap_or("Schablone").to_string());
        self.template.opacity = m["opacity"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0) as f32;
        self.template.scale = m["scale"].as_f64().unwrap_or(1.0).clamp(0.1, 2.0);
        self.template.offset = (m["x"].as_f64().unwrap_or(0.0), m["y"].as_f64().unwrap_or(0.0));
    }

    fn forget_template(&mut self) {
        self.template.loaded = None;
        if let Some(dir) = store().filter(|_| self.template.persist) {
            let _ = std::fs::remove_file(dir.join("template.img"));
            let _ = std::fs::remove_file(dir.join("template.json"));
        }
    }

    pub(crate) fn load_template_file(&mut self, ctx: &egui::Context, path: &Path) {
        let name = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let bytes = std::fs::read(path).map_err(|e| e.to_string());
        match bytes.and_then(|b| decode(&b).map(|t| (t, b))) {
            Ok((t, b)) => {
                self.set_template(ctx, t, name);
                if let Some(dir) = store().filter(|_| self.template.persist) {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(dir.join("template.img"), b);
                    self.save_template_meta();
                }
            }
            Err(e) => self.error = Some(trf("Das Bild „{name}“ ließ sich nicht laden: {e}", &[("name", &name), ("e", &e)])),
        }
    }

    pub(crate) fn set_template(&mut self, ctx: &egui::Context, t: Template, name: String) {
        let image = egui::ColorImage::from_rgba_unmultiplied([t.w as usize, t.h as usize], &t.rgba);
        let texture = ctx.load_texture("template", image, egui::TextureOptions::LINEAR);
        self.template.loaded = Some(Loaded { template: t, texture, name });
        self.template.offset = (0.0, 0.0);
    }

    fn placement(&self) -> Option<tpl::Placement> {
        let l = self.template.loaded.as_ref()?;
        let sp = self.sprite();
        let (ox, oy) = self.template.offset;
        Some(l.template.placement(sp.width, sp.height, self.template.scale, ox, oy))
    }

    fn trace_template(&mut self, mode: TraceMode) {
        if !self.layer_ok() {
            return;
        }
        let (Some(p), Some(l)) = (self.placement(), &self.template.loaded) else { return };
        let t = l.template.clone();
        self.deselect();
        let pal = self.project.current_palette();
        let cur = self.project.current;
        self.histories[cur].record(&self.project.sprites[cur]);
        match tpl::trace(self.project.sprite_mut(), &pal, &t, p, mode) {
            None => {
                self.histories[cur].drop_last();
                self.hint = Some(tr("Keine Farben in der Schablone gefunden.").into());
            }
            Some(0) => {
                self.histories[cur].drop_last();
                self.hint = Some(tr("Keine Pixel geändert — Schablone über der Fläche positionieren?").into());
            }
            Some(n) => {
                let how = match mode {
                    TraceMode::Palette => tr("Palettenfarben").to_string(),
                    TraceMode::Raw => tr("Originalfarben").to_string(),
                    TraceMode::Quantize(k) => trf("{n} Farben", &[("n", &k)]),
                };
                self.hint = Some(trf("Schablone übernommen — {n} Pixel ({mode}).", &[("mode", &how), ("n", &n)]));
                self.changed();
            }
        }
    }

    pub(crate) fn template_panel(&mut self, ui: &mut egui::Ui) {
        if ui.button(tr("Bild laden …")).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(tr("Schablone laden"))
                .add_filter(tr("Bilder"), &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
                .pick_file()
            {
                self.load_template_file(ui.ctx(), &path);
            }
        }
        let Some(l) = &self.template.loaded else {
            ui.weak(tr("Ein Foto oder Bild zum Abzeichnen — es liegt hinter den Pixeln."));
            return;
        };
        ui.weak(format!("{} · {} × {}", l.name, l.template.w, l.template.h));
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(tr("Deckkraft"));
            let r = ui.add(egui::Slider::new(&mut self.template.opacity, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
            changed |= r.drag_stopped() || (r.changed() && !r.dragged());
        });
        ui.horizontal(|ui| {
            ui.label(tr("Größe"));
            let r = ui.add(egui::Slider::new(&mut self.template.scale, 0.1..=2.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
            changed |= r.drag_stopped() || (r.changed() && !r.dragged());
        });
        ui.horizontal(|ui| {
            if ui.button(tr("Zentrieren")).clicked() {
                self.template.offset = (0.0, 0.0);
                changed = true;
            }
            if ui.button(tr("Entfernen")).clicked() {
                self.forget_template();
            }
        });
        if changed {
            self.save_template_meta();
        }
        if self.template.loaded.is_none() {
            return;
        }
        ui.weak(tr("Umschalt+Alt ziehen: verschieben · Umschalt+Alt Klick: Farbe nehmen · Umschalt halten: vorne zeigen"));
        ui.separator();
        ui.label(tr("Aufs Raster übernehmen"));
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Palettenfarben")).on_hover_text(tr("Jede Zelle bekommt die nächste Farbe der Palette")).clicked() {
                self.trace_template(TraceMode::Palette);
            }
            if ui.button(tr("Originalfarben")).on_hover_text(tr("Genau die Farben des Bildes — als freie Farben")).clicked() {
                self.trace_template(TraceMode::Raw);
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr("Reduzieren auf"));
            ui.add(egui::DragValue::new(&mut self.template.colors).range(2..=64));
            ui.label(tr("Farben"));
            if ui.button(tr("Übernehmen")).clicked() {
                let n = self.template.colors;
                self.trace_template(TraceMode::Quantize(n));
            }
        });
    }

    /// Umschalt+Alt auf der Fläche: Schablone ziehen oder Farbe nehmen.
    /// `true`, wenn der Zeiger damit verbraucht ist.
    pub(crate) fn template_pointer(&mut self, pointer: Option<Pos2>, pressed: bool, released: bool, over: bool, origin: Pos2, zoom: f32) -> bool {
        if self.template.loaded.is_none() {
            return false;
        }
        let (shift, alt) = (self.modifiers.shift, self.modifiers.alt);
        if pressed && over && shift && alt {
            if let Some(p) = pointer {
                self.template.drag = Some((p, self.template.offset));
                self.blocked = true;
            }
        }
        let Some((start, from)) = self.template.drag else { return false };
        if let Some(p) = pointer {
            let d = (p - start) / zoom;
            self.template.offset = (from.0 + d.x as f64, from.1 + d.y as f64);
        }
        if released {
            self.template.drag = None;
            self.save_template_meta();
            // Kaum bewegt: das war ein Klick → Farbe aus der Schablone.
            if let Some(p) = pointer.filter(|p| (*p - start).length() < 3.0) {
                self.template.offset = from;
                let at = (p - origin) / zoom;
                let pick = self.placement().and_then(|pl| self.template.loaded.as_ref()?.template.pick(pl, at.x as f64, at.y as f64));
                match pick {
                    Some([r, g, b, a]) if a > 0 => {
                        self.set_rgb([r, g, b]);
                        self.hint = Some(trf("Farbe aus der Schablone: {hex}", &[("hex", &crate::palette_ui::hex([r, g, b]))]));
                    }
                    Some(_) => self.hint = Some(tr("Dort ist die Schablone durchsichtig.").into()),
                    None => self.hint = Some(tr("Dort liegt keine Schablone.").into()),
                }
            }
        }
        true
    }

    /// Schablone zeichnen. `front`: der Durchgang über den Pixeln.
    pub(crate) fn draw_template(&self, painter: &egui::Painter, origin: Pos2, zoom: f32, front: bool) {
        let Some(l) = &self.template.loaded else { return };
        // Umschalt (ohne Alt) holt sie nach vorn; Umschalt+Alt ist Verschieben.
        let in_front = self.modifiers.shift && !self.modifiers.alt;
        if front != in_front {
            return;
        }
        let Some(p) = self.placement() else { return };
        let rect = egui::Rect::from_min_size(origin + Vec2::new(p.x as f32, p.y as f32) * zoom, Vec2::new(p.w as f32, p.h as f32) * zoom);
        let alpha = if front { 1.0 } else { self.template.opacity };
        let uv = egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(l.texture.id(), rect, uv, Color32::WHITE.gamma_multiply(alpha));
    }
}
