//! Ebenen: das Panel „Ebenen“ (wie in der Web-Version) und die Knöpfe, die
//! es mit der Timeline teilt (timeline.rs).
//!
//! ```text
//!   👁 🔒 [▦] Figur        80 %   ← aktive Ebene hervorgehoben
//!   👁 🔒 [▦] Grund
//!   [+ ▲ ▼ ⧉ ⤓ ⤓⤓ 🗑]  Maske [..]  Deckkraft [100 %]
//! ```
//!
//! Jede Zeile zeigt ein Vorschaubild der Ebene im aktuellen Frame — so
//! sieht man, was auf welcher Ebene liegt. Klick wählt die Ebene,
//! Doppelklick benennt um.

use std::collections::HashMap;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Sense, Vec2};
use spritebit_core::composite::render_layer_rgba_step;

use crate::i18n::{tr, trf};
use crate::{icons, SpritebitApp};

/// Höhe einer Zeile im Panel.
const ROW_H: f32 = 30.0;
/// Kantenlänge des Vorschaubilds.
const THUMB: f32 = 24.0;
/// Hervorhebung der aktiven Ebene (wie in der Timeline).
const ACCENT: Color32 = Color32::from_rgb(110, 168, 254);

/// Vorschaubilder der Ebenen: Ebene → (was gerechnet wurde, Textur).
#[derive(Default)]
pub(crate) struct LayerThumbs {
    sprite: usize,
    thumbs: HashMap<usize, (String, egui::TextureHandle)>,
}

impl SpritebitApp {
    /// Vorschaubild der Ebene `l` im aktuellen Frame.
    fn layer_thumb(&mut self, ctx: &egui::Context, l: usize) -> egui::TextureId {
        let cur = self.project.current;
        if self.layer_thumbs.sprite != cur {
            self.layer_thumbs.thumbs.clear();
            self.layer_thumbs.sprite = cur;
        }
        let sp = self.sprite();
        let mask = sp.layers[l].mask.as_ref().map(|m| m.on);
        let key = format!("{}|{}|{:?}|{}|{:?}|{}", sp.frame, l, sp.frames[sp.frame].cels[l], self.version, mask, sp.palette);
        if self.layer_thumbs.thumbs.get(&l).is_none_or(|c| c.0 != key) {
            let pal = self.project.current_palette();
            let step = sp.width.max(sp.height).div_ceil(THUMB as u32 * 2).max(1);
            let (rgba, w, h) = render_layer_rgba_step(sp, &pal, sp.frame, l, step);
            let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            let tex = ctx.load_texture(format!("layer-{l}"), img, egui::TextureOptions::NEAREST);
            self.layer_thumbs.thumbs.insert(l, (key, tex));
        }
        self.layer_thumbs.thumbs[&l].1.id()
    }

    /// Das Panel „Ebenen“: oberste Ebene oben.
    pub(crate) fn layers_panel(&mut self, ui: &mut egui::Ui) {
        let nl = self.project.sprite().layers.len();
        for l in (0..nl).rev() {
            self.layer_row(ui, l);
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| self.layer_buttons(ui));
        ui.horizontal_wrapped(|ui| {
            ui.weak(tr("Maske"));
            self.mask_buttons(ui);
        });
        ui.horizontal(|ui| {
            ui.weak(tr("Deckkraft"));
            self.opacity_field(ui);
        });
        ui.weak(tr("Doppelklick auf den Namen benennt um. Export und Vorschau zeigen alle sichtbaren Ebenen übereinander."));
    }

    fn layer_row(&mut self, ui: &mut egui::Ui, l: usize) {
        let thumb = self.layer_thumb(ui.ctx(), l);
        let sp = self.project.sprite();
        let (sw, sh) = (sp.width as f32, sp.height as f32);
        let layer = &sp.layers[l];
        let (name, visible, locked, opacity, has_mask) = (layer.name.clone(), layer.visible, layer.locked, layer.opacity, layer.mask.is_some());
        let active = l == sp.layer;
        let (rect, row) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
        // Name für Screenreader und Tests; gewählt = aktive Ebene.
        row.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, &name));
        let painter = ui.painter_at(rect);
        let bg = if active {
            ACCENT.gamma_multiply(0.28)
        } else if row.hovered() {
            ui.visuals().widgets.hovered.weak_bg_fill
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect.shrink2(Vec2::new(0.0, 1.0)), 4.0, bg);
        if active {
            painter.rect_filled(egui::Rect::from_min_size(rect.min + Vec2::new(0.0, 3.0), Vec2::new(3.0, ROW_H - 6.0)), 1.0, ACCENT);
        }
        let mid = rect.center().y;
        let c = ui.visuals().text_color();
        let dim = Color32::from_gray(110);
        let icon_at = |k: f32| egui::Rect::from_center_size(Pos2::new(rect.min.x + 16.0 + k * 22.0, mid), Vec2::splat(20.0));
        let (eye, eye_tip) = if visible { (icons::EYE, tr("Ebene ausblenden")) } else { (icons::EYE_OFF, tr("Ebene einblenden")) };
        let eye_btn = ui.put(icon_at(0.0), egui::Button::image(icons::image(eye, if visible { c } else { dim }).alt_text(eye_tip)).frame(false)).on_hover_text(eye_tip);
        let (lock, lock_tip) = if locked { (icons::LOCK, tr("Ebene entsperren")) } else { (icons::UNLOCK, tr("Ebene sperren")) };
        let lock_btn = ui.put(icon_at(1.0), egui::Button::image(icons::image(lock, if locked { ACCENT } else { dim }).alt_text(lock_tip)).frame(false)).on_hover_text(lock_tip);
        // Vorschaubild im Seitenverhältnis, auf Karo (= durchsichtig).
        let frame = egui::Rect::from_center_size(Pos2::new(rect.min.x + 16.0 + 2.0 * 22.0 + 6.0, mid), Vec2::splat(THUMB));
        painter.rect_filled(frame, 2.0, Color32::from_gray(52));
        // Karo in 4 × 4 Feldern — fein genug, dass dunkle Pixel davor auffallen.
        let q = THUMB / 4.0;
        for i in 0..4 {
            for j in 0..4 {
                if (i + j) % 2 == 0 {
                    painter.rect_filled(egui::Rect::from_min_size(frame.min + Vec2::new(i as f32 * q, j as f32 * q), Vec2::splat(q)), 0.0, Color32::from_gray(80));
                }
            }
        }
        let k = THUMB / sw.max(sh);
        let img = egui::Rect::from_center_size(frame.center(), Vec2::new(sw * k, sh * k));
        painter.image(thumb, img, egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        painter.rect_stroke(frame, 2.0, egui::Stroke::new(1.0, if active { ACCENT } else { Color32::from_gray(70) }), egui::StrokeKind::Outside);
        let mut x = frame.max.x + 8.0;
        if has_mask {
            let r = egui::Rect::from_center_size(Pos2::new(x + 7.0, mid), Vec2::splat(14.0));
            egui::Image::new(icons::MASK).tint(ACCENT).paint_at(ui, r);
            x += 18.0;
        }
        let name_col = if active { ui.visuals().strong_text_color() } else if visible { c } else { dim };
        let font = if active { FontId::proportional(13.5) } else { FontId::proportional(13.0) };
        let right = rect.max.x - 6.0;
        if opacity < 1.0 {
            painter.text(Pos2::new(right, mid), Align2::RIGHT_CENTER, format!("{} %", (opacity * 100.0).round()), FontId::proportional(11.0), dim);
        }
        painter.with_clip_rect(egui::Rect::from_min_max(Pos2::new(x, rect.min.y), Pos2::new(right - 34.0, rect.max.y))).text(Pos2::new(x, mid), Align2::LEFT_CENTER, &name, font, name_col);

        if eye_btn.clicked() {
            self.edit_sprite(|s| s.layers[l].visible = !s.layers[l].visible);
        } else if lock_btn.clicked() {
            self.edit_sprite(|s| s.layers[l].locked = !s.layers[l].locked);
        } else if row.double_clicked() {
            self.rename_layer = Some((l, name));
        } else if row.clicked() && !active {
            self.deselect();
            self.project.sprite_mut().layer = l;
        }
    }

    /// Neue Ebene, verschieben, verdoppeln, zusammenlegen, löschen —
    /// im Panel und in der Timeline.
    pub(crate) fn layer_buttons(&mut self, ui: &mut egui::Ui) {
        let (nl, l) = (self.project.sprite().layers.len(), self.project.sprite().layer);
        if icons::button(ui, icons::PLUS, tr("Neue Ebene über der aktiven"), true).clicked() {
            self.edit_sprite(|s| {
                let name = trf("Ebene {n}", &[("n", &(s.layers.len() + 1))]);
                let at = s.layer + 1;
                s.add_layer(at, name);
            });
        }
        if icons::button(ui, icons::UP, tr("Ebene nach oben"), l + 1 < nl).clicked() {
            self.edit_sprite(|s| s.move_layer(l, l + 1));
        }
        if icons::button(ui, icons::DOWN, tr("Ebene nach unten"), l > 0).clicked() {
            self.edit_sprite(|s| s.move_layer(l, l - 1));
        }
        if icons::button(ui, icons::COPY, tr("Ebene verdoppeln"), true).clicked() {
            let name = trf("{name} Kopie", &[("name", &self.project.sprite().layers[l].name)]);
            self.edit_sprite(|s| s.duplicate_layer(name));
        }
        if icons::button(ui, icons::MERGE_DOWN, tr("Nach unten zusammenlegen — in jedem Frame"), l > 0).clicked() {
            let pal = self.project.current_palette();
            self.edit_sprite(|s| {
                s.merge_down(&pal);
            });
        }
        let visible = self.project.sprite().layers.iter().filter(|l| l.visible && l.opacity > 0.0).count();
        if icons::button(ui, icons::MERGE_ALL, tr("Alle sichtbaren Ebenen zusammenführen — in jedem Frame, auch Licht und Schatten; ausgeblendete bleiben"), visible >= 2).clicked() {
            let pal = self.project.current_palette();
            let name = tr("Zusammengeführt");
            self.edit_sprite(|s| {
                s.merge_visible(&pal, name);
            });
        }
        if icons::button(ui, icons::TRASH, tr("Ebene löschen"), nl > 1).clicked() {
            self.edit_sprite(|s| {
                let l = s.layer;
                s.delete_layer(l);
            });
        }
    }

    /// Maske der aktiven Ebene (spritebit_core::mask): hinzufügen, bearbeiten,
    /// ein/aus, anwenden, löschen.
    pub(crate) fn mask_buttons(&mut self, ui: &mut egui::Ui) {
        let (has_mask, mask_on, editing) = {
            let sp = self.project.sprite();
            let m = sp.layers[sp.layer].mask.as_ref();
            (m.is_some(), m.is_some_and(|m| m.on), sp.editing_mask())
        };
        if !has_mask {
            if icons::button(ui, icons::MASK, tr("Maske hinzufügen — damit blendest du Teile der Ebene aus, ohne sie zu löschen"), true).clicked() {
                self.edit_sprite(|s| {
                    let l = s.layer;
                    s.layers[l].mask = Some(spritebit_core::mask::Mask::new(s.width, s.height));
                    s.editing_mask = true;
                });
                self.hint = Some(tr("Maske bearbeiten: Malen blendet aus, Radieren blendet wieder ein.").into());
            }
        } else {
            let c = ui.visuals().text_color();
            let b = egui::Button::selectable(editing, icons::image(icons::MASK, c)).wrap_mode(egui::TextWrapMode::Extend);
            if ui.add(b).on_hover_text(tr("Maske bearbeiten: Malen blendet aus, Radieren blendet wieder ein")).clicked() {
                self.commit_float();
                let sp = self.project.sprite_mut();
                sp.editing_mask = !editing;
                self.hint = (!editing).then(|| tr("Maske bearbeiten: Malen blendet aus, Radieren blendet wieder ein.").into());
                self.changed();
            }
            let (eye, tip) = if mask_on { (icons::EYE, tr("Maske ausschalten (alles sichtbar)")) } else { (icons::EYE_OFF, tr("Maske einschalten")) };
            if icons::button(ui, eye, tip, true).clicked() {
                self.edit_sprite(|s| {
                    let l = s.layer;
                    if let Some(m) = s.layers[l].mask.as_mut() {
                        m.on = !m.on;
                    }
                });
            }
            if icons::button(ui, icons::CHECK, tr("Maske anwenden: ausgeblendete Pixel werden gelöscht, die Maske verschwindet"), true).clicked() {
                self.edit_sprite(|s| {
                    s.apply_mask();
                });
            }
            if icons::button(ui, icons::TRASH, tr("Maske löschen — alles wieder sichtbar"), true).clicked() {
                self.edit_sprite(|s| {
                    let l = s.layer;
                    s.layers[l].mask = None;
                    s.editing_mask = false;
                });
            }
        }
    }

    /// Deckkraft der aktiven Ebene.
    pub(crate) fn opacity_field(&mut self, ui: &mut egui::Ui) {
        // Aktive Ebene frisch lesen — Verdoppeln/Zusammenlegen eben hat sie verschoben.
        let la = self.project.sprite().layer;
        let mut op = self.project.sprite().layers[la].opacity * 100.0;
        let r = ui
            .add(egui::DragValue::new(&mut op).range(0.0..=100.0).speed(1.0).suffix(" %").max_decimals(0))
            .on_hover_text(tr("Deckkraft der aktiven Ebene"));
        if r.drag_started() || (r.changed() && !r.dragged()) {
            self.edit_sprite(|_| {});
        }
        if r.changed() {
            self.project.sprite_mut().layers[la].opacity = (op / 100.0).clamp(0.0, 1.0);
            self.changed();
        }
    }
}
