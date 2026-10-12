//! Einstellungen (Menü „Bearbeiten“, Strg+,) — wie im Web: sammelt Sprache,
//! Pixelraster, Kachelmodus, Pinselform und die gespeicherten Pinsel an einem
//! Ort. Die gewohnten Plätze in Menü und Werkzeugleiste bleiben; das Fenster
//! ändert dieselben Felder und ruft dieselben Funktionen.

use eframe::egui::{self, Color32, Key, Modifiers, Vec2};
use spritebit_core::stamp::SavedBrush;
use spritebit_core::tiled::Tiled;

use crate::i18n::{self, tr};
use crate::{icons, SpritebitApp};

/// Kleines Bild eines gespeicherten Pinsels in einem Quadrat der Kante `size`.
pub(crate) fn brush_thumb(ui: &mut egui::Ui, b: &SavedBrush, size: f32, sense: egui::Sense) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), sense);
    let p = ui.painter();
    p.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
    if resp.hovered() && sense.senses_click() {
        p.rect_stroke(rect, 3.0, egui::Stroke::new(2.0, ui.visuals().selection.bg_fill), egui::StrokeKind::Inside);
    }
    let cell = (size - 4.0) / b.w.max(b.h) as f32;
    let o = rect.center() - Vec2::new(b.w as f32, b.h as f32) * cell / 2.0;
    for (n, px) in b.px.iter().enumerate() {
        if let Some([r, g, bl]) = px {
            let (x, y) = ((n as u32 % b.w) as f32, (n as u32 / b.w) as f32);
            p.rect_filled(egui::Rect::from_min_size(o + Vec2::new(x, y) * cell, Vec2::splat(cell)), 0.0, Color32::from_rgb(*r, *g, *bl));
        }
    }
    resp.on_hover_text(format!("{}×{}", b.w, b.h))
}

/// Thema links im Einstellungsfenster.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum SettingsTab {
    #[default]
    Lang,
    View,
    Brush,
}

impl SpritebitApp {
    /// Einstellungsfenster zeichnen; Strg+, schaltet es um.
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context) {
        if !ctx.egui_wants_keyboard_input() && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Comma)) {
            self.settings_open = !self.settings_open;
        }
        if !self.settings_open {
            return;
        }
        let mut open = true;
        egui::Window::new(tr("Einstellungen"))
            .id(egui::Id::new("settings"))
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(540.0, 280.0))
            .show(ctx, |ui| {
                // Wie in Photoshop: links die Themen, ein Trennstrich, rechts
                // die Einstellungen des gewählten Themas.
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(140.0);
                        for (tab, label) in [(SettingsTab::Lang, tr("Sprache")), (SettingsTab::View, tr("Ansicht")), (SettingsTab::Brush, tr("Pinsel"))] {
                            let b = egui::Button::selectable(self.settings_tab == tab, label).min_size(Vec2::new(140.0, 28.0));
                            if ui.add(b).clicked() {
                                self.settings_tab = tab;
                            }
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_min_width(360.0);
                        match self.settings_tab {
                            SettingsTab::Lang => self.settings_lang(ui),
                            SettingsTab::View => self.settings_view(ui),
                            SettingsTab::Brush => self.settings_brush(ui),
                        }
                    });
                });
            });
        if !open {
            self.settings_open = false;
        }
    }

    fn settings_lang(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Sprache"));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for l in i18n::Lang::ALL {
                if ui.selectable_label(i18n::lang() == l, l.name()).clicked() {
                    i18n::choose(l);
                }
            }
        });
    }

    fn settings_view(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Ansicht"));
        ui.add_space(6.0);
        if ui.checkbox(&mut self.show_grid, tr("Pixelraster anzeigen")).changed() {
            self.save_grid_pref();
        }
        ui.add_space(6.0);
        ui.label(tr("Kachelmodus"));
        ui.horizontal_wrapped(|ui| {
            for (t, label) in [(Tiled::Off, tr("Aus")), (Tiled::Both, tr("Ringsum")), (Tiled::X, tr("Nur nebeneinander")), (Tiled::Y, tr("Nur übereinander"))]
            {
                if ui.selectable_label(self.tiled == t, label).clicked() && self.tiled != t {
                    self.set_tiled(t);
                }
            }
        });
    }

    fn settings_brush(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Pinsel"));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(tr("Pinselform"));
            let c = ui.visuals().text_color();
            for (round, icon, label) in [(false, icons::SQUARE, tr("Eckig")), (true, icons::CIRCLE, tr("Rund"))] {
                let b = egui::Button::selectable(self.brush_round == round, (icons::image(icon, c), label));
                if ui.add(b).clicked() && self.brush_round != round {
                    self.brush_round = round;
                    self.save_view();
                }
            }
        });
        ui.add_space(6.0);
        ui.label(tr("Gespeicherte Pinsel"));
        if self.brushes.is_empty() {
            ui.weak(tr("Noch keine. Wähle etwas aus, drück Strg+B und dann „Speichern“ in der Pinsel-Zeile."));
        }
        let mut delete = None;
        ui.horizontal_wrapped(|ui| {
            for (i, b) in self.brushes.iter().enumerate() {
                ui.vertical(|ui| {
                    brush_thumb(ui, b, 44.0, egui::Sense::hover());
                    if ui.small_button("×").on_hover_text(tr("Diesen Pinsel löschen")).clicked() {
                        delete = Some(i);
                    }
                });
            }
        });
        if let Some(i) = delete {
            self.brushes.remove(i);
            self.store_brushes();
        }
    }
}
