//! Panel „Vorschau“: der ganze Sprite, unabhängig vom Zoom der Fläche —
//! so, wie er später aussieht (sichtbare Ebenen, ohne Gitter, Onion Skin,
//! Hilfslinien). Läuft beim Abspielen mit.
//!
//! Pixelgröße: „Einpassen“ (ganzzahlig, höchstens 8×) oder fest 1× 2× 4× 8×.
//! 1× ist die Größe im Spiel. Große Sprites werden für die Vorschau
//! ausgedünnt gezeichnet.

use eframe::egui::{self, Color32, Sense, Vec2};
use spritebit_core::{render_rgba_step, Rect};

use crate::i18n::{tr, trf};
use crate::SpritebitApp;

/// Größte Kante der Vorschau-Textur; darüber wird ausgedünnt.
const MAX_TEX: u32 = 512;
/// Mehr wird ein Pixel nie — darüber sieht man nur noch Klötze.
const MAX_SCALE: u32 = 8;

#[derive(Default)]
pub(crate) struct Preview {
    /// Feste Pixelgröße; `None` = einpassen.
    pub fixed: Option<u32>,
    texture: Option<((usize, usize, u64), egui::TextureHandle, u32)>,
}

/// Bildschirmpixel je Sprite-Pixel — ganzzahlig, damit die Kanten hart bleiben.
pub(crate) fn preview_scale(w: u32, h: u32, box_w: f32, box_h: f32, fixed: Option<u32>) -> u32 {
    if let Some(f) = fixed {
        return f;
    }
    if w == 0 || h == 0 || box_w <= 0.0 || box_h <= 0.0 {
        return 1;
    }
    ((box_w / w as f32).min(box_h / h as f32).floor() as u32).clamp(1, MAX_SCALE)
}

impl SpritebitApp {
    pub(crate) fn preview_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(tr("Pixelgröße"));
            let p = &mut self.preview.fixed;
            ui.selectable_value(p, None, tr("Einpassen"));
            for s in [1, 2, 4, 8] {
                ui.selectable_value(p, Some(s), format!("{s}×"));
            }
        });
        let sp = self.project.sprite();
        let key = (self.project.current, sp.frame, self.version);
        let step = sp.width.max(sp.height).div_ceil(MAX_TEX).max(1);
        if self.preview.texture.as_ref().is_none_or(|t| t.0 != key) {
            let pal = self.project.current_palette();
            let (rgba, w, h) = render_rgba_step(sp, &pal, sp.frame, Rect { x: 0, y: 0, w: sp.width, h: sp.height }, step);
            let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            let tex = ui.ctx().load_texture("preview", img, egui::TextureOptions::NEAREST);
            self.preview.texture = Some((key, tex, step));
        }
        let (w, h) = (sp.width, sp.height);
        let avail = ui.available_width();
        let scale = preview_scale(w, h, avail, 240.0, self.preview.fixed);
        // Ausgedünnt: die Textur ist kleiner als der Sprite.
        let size = Vec2::new(w as f32, h as f32) * scale as f32;
        let size = if size.x > avail { size * (avail / size.x) } else { size };
        egui::ScrollArea::both().id_salt("preview-scroll").max_height(300.0).show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            // Schachbrett, bei kleiner Darstellung gröber
            let cell = (scale.max(4) * 2) as f32;
            let p = ui.painter_at(rect);
            let (nx, ny) = ((rect.width() / cell).ceil() as i32, (rect.height() / cell).ceil() as i32);
            for y in 0..ny {
                for x in 0..nx {
                    let c = if (x + y) % 2 == 0 { Color32::from_gray(52) } else { Color32::from_gray(66) };
                    let r = egui::Rect::from_min_size(rect.min + Vec2::new(x as f32 * cell, y as f32 * cell), Vec2::splat(cell));
                    p.rect_filled(r, 0.0, c);
                }
            }
            if let Some((_, tex, _)) = &self.preview.texture {
                p.image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            }
        });
        ui.weak(trf("{w} × {h} px · {scale}×", &[("w", &w), ("h", &h), ("scale", &scale)]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn einpassen_ganzzahlig_und_begrenzt() {
        assert_eq!(preview_scale(16, 16, 200.0, 240.0, None), 8, "höchstens 8×");
        assert_eq!(preview_scale(64, 32, 200.0, 240.0, None), 3);
        assert_eq!(preview_scale(1000, 1000, 200.0, 240.0, None), 1, "mindestens 1×");
        assert_eq!(preview_scale(64, 64, 200.0, 240.0, Some(2)), 2);
    }
}
