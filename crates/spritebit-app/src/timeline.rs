//! Timeline: Ebenen × Frames als Raster, wie in der Web-Version.
//!
//! ```text
//!   [⏮ ◀ ▶ ▶ ⏭]  [+ Frame] [Duplizieren] [Frame löschen] …  FPS  Dauer
//!               │  1   2   3   4
//!   👁 🔒 ⛓ Figur │  ●   ●   ○   ●
//!   👁 🔒 ⛓ Grund │  ●━━━●━━━●━━━●     ← verknüpft: dasselbe Bild
//! ```
//!
//! Ein Klick auf eine Zelle wählt Frame und Ebene, ein Klick auf eine
//! Frame-Nummer nur den Frame. Auge, Schloss und Kette in der Ebenen-Spalte
//! schalten Sichtbarkeit, Sperre und „durchgehend" um.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Sense, Stroke, Vec2};

use crate::SpritebitApp;

const LAYER_W: f32 = 170.0;
const HEAD_H: f32 = 20.0;
const ROW_H: f32 = 22.0;
const CELL_W: f32 = 26.0;
const ICON_W: f32 = 20.0;

const ACCENT: Color32 = Color32::from_rgb(0x6c, 0x9e, 0xf8);
const DIM: Color32 = Color32::from_gray(140);

impl SpritebitApp {
    /// Strukturänderung am aktuellen Sprite — als ein Undo-Schritt.
    fn edit_sprite(&mut self, f: impl FnOnce(&mut spritebit_core::Sprite)) {
        let cur = self.project.current;
        self.histories[cur].record(&self.project.sprites[cur]);
        f(&mut self.project.sprites[cur]);
        self.changed();
    }

    fn go_frame(&mut self, f: usize) {
        let n = self.project.sprite().frames.len();
        self.project.sprite_mut().frame = f.min(n - 1);
        self.stroke_last = None;
    }

    pub(crate) fn timeline(&mut self, ui: &mut egui::Ui) {
        self.timeline_buttons(ui);
        ui.add_space(4.0);
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| self.timeline_grid(ui));
    }

    fn timeline_buttons(&mut self, ui: &mut egui::Ui) {
        let n = self.project.sprite().frames.len();
        let cur = self.project.sprite().frame;
        ui.horizontal(|ui| {
            if ui.button("⏮").on_hover_text("Erster Frame (Pos1)").clicked() {
                self.go_frame(0);
            }
            if ui.button("◀").on_hover_text("Voriger Frame (,)").clicked() {
                self.go_frame((cur + n - 1) % n);
            }
            let play = if self.playing { "⏸" } else { "▶" };
            if ui.add_enabled(n > 1, egui::Button::new(play)).on_hover_text("Abspielen / Anhalten (Enter)").clicked() {
                self.toggle_play(ui.ctx());
            }
            if ui.button("▶|").on_hover_text("Nächster Frame (.)").clicked() {
                self.go_frame((cur + 1) % n);
            }
            if ui.button("⏭").on_hover_text("Letzter Frame (Ende)").clicked() {
                self.go_frame(n - 1);
            }
            ui.separator();
            if ui.button("+ Frame").on_hover_text("Leerer Frame dahinter").clicked() {
                self.edit_sprite(|s| {
                    let f = s.frame;
                    s.add_frame(f, false);
                });
            }
            if ui.button("Duplizieren").on_hover_text("Frame kopieren").clicked() {
                self.edit_sprite(|s| {
                    let f = s.frame;
                    s.add_frame(f, true);
                });
            }
            if ui.add_enabled(n > 1, egui::Button::new("Frame löschen")).clicked() {
                self.edit_sprite(|s| {
                    let f = s.frame;
                    s.delete_frame(f);
                });
            }
            ui.separator();
            if ui.button("+ Ebene").on_hover_text("Neue Ebene über der aktiven").clicked() {
                self.edit_sprite(|s| {
                    let name = format!("Ebene {}", s.layers.len() + 1);
                    let at = s.layer + 1;
                    s.add_layer(at, name);
                });
            }
            let many_layers = self.project.sprite().layers.len() > 1;
            if ui.add_enabled(many_layers, egui::Button::new("Ebene löschen")).clicked() {
                self.edit_sprite(|s| {
                    let l = s.layer;
                    s.delete_layer(l);
                });
            }
            ui.separator();
            let sp = self.project.sprite();
            let (f, l) = (sp.frame, sp.layer);
            let linked = sp.is_linked(f, l);
            if ui
                .add_enabled(f > 0, egui::Button::new("Verknüpfen"))
                .on_hover_text("Diese Zelle zeigt dasselbe Bild wie der Frame davor")
                .clicked()
            {
                self.edit_sprite(|s| s.link(f - 1, f, l));
            }
            if ui.add_enabled(linked, egui::Button::new("Lösen")).on_hover_text("Eigenes Bild mit gleichem Inhalt").clicked() {
                self.edit_sprite(|s| s.unlink(f, l));
            }
            ui.separator();
            if ui.selectable_label(self.onion, "Onion Skin").on_hover_text("Voriger (rot) und nächster Frame (blau) scheinen durch").clicked() {
                self.onion = !self.onion;
                self.version = self.version.wrapping_add(1);
            }
            ui.separator();
            let mut fps = self.project.sprite().fps;
            ui.label("FPS");
            if ui.add(egui::DragValue::new(&mut fps).range(1..=60)).changed() {
                self.project.sprite_mut().fps = fps;
                self.dirty = true;
            }
            let mut dur = self.project.sprite().frames[cur].duration_ms;
            ui.label("Dauer");
            if ui
                .add(egui::DragValue::new(&mut dur).range(0..=10_000).suffix(" ms"))
                .on_hover_text("0 = nach FPS")
                .changed()
            {
                self.project.sprite_mut().frames[cur].duration_ms = dur;
                self.dirty = true;
            }
        });
    }

    fn timeline_grid(&mut self, ui: &mut egui::Ui) {
        let sp = self.project.sprite();
        let (n, nl) = (sp.frames.len(), sp.layers.len());
        let size = Vec2::new(LAYER_W + n as f32 * CELL_W + 8.0, HEAD_H + nl as f32 * ROW_H + 4.0);
        let (resp, painter) = ui.allocate_painter(size, Sense::click());
        let o = resp.rect.min;
        let font = FontId::proportional(12.0);
        // Zeile `r` von oben zeigt Ebene nl-1-r — oberste Ebene oben.
        let row_y = |r: usize| o.y + HEAD_H + r as f32 * ROW_H;
        let col_x = |f: usize| o.x + LAYER_W + f as f32 * CELL_W;

        // Kopfzeile: Frame-Nummern (1-basiert, wie in der Web-Version).
        for f in 0..n {
            let c = egui::Rect::from_min_size(Pos2::new(col_x(f), o.y), Vec2::new(CELL_W, HEAD_H));
            if f == sp.frame {
                painter.rect_filled(c.shrink(1.0), 3.0, ACCENT.gamma_multiply(0.35));
            }
            let in_tag = sp.tags.iter().find(|t| f >= t.from && f <= t.to);
            if let Some(t) = in_tag {
                let [r, g, b] = t.color;
                painter.line_segment([c.left_bottom(), c.right_bottom()], Stroke::new(3.0, Color32::from_rgb(r, g, b)));
            }
            painter.text(c.center(), Align2::CENTER_CENTER, format!("{}", f + 1), font.clone(), if f == sp.frame { Color32::WHITE } else { DIM });
        }

        for r in 0..nl {
            let l = nl - 1 - r;
            let layer = &sp.layers[l];
            let y = row_y(r);
            // Ebenen-Spalte
            let row = egui::Rect::from_min_size(Pos2::new(o.x, y), Vec2::new(LAYER_W - 4.0, ROW_H - 2.0));
            painter.rect_filled(row, 3.0, if l == sp.layer { ACCENT.gamma_multiply(0.25) } else { Color32::from_gray(38) });
            let icon = |k: usize| Pos2::new(o.x + 10.0 + k as f32 * ICON_W, y + ROW_H / 2.0 - 1.0);
            painter.text(icon(0), Align2::CENTER_CENTER, if layer.visible { "👁" } else { "–" }, font.clone(), DIM);
            painter.text(icon(1), Align2::CENTER_CENTER, if layer.locked { "🔒" } else { "🔓" }, font.clone(), if layer.locked { ACCENT } else { DIM });
            painter.text(icon(2), Align2::CENTER_CENTER, "⛓", font.clone(), if layer.continuous { ACCENT } else { Color32::from_gray(80) });
            let name_col = if l == sp.layer { Color32::WHITE } else if layer.visible { Color32::from_gray(200) } else { Color32::from_gray(110) };
            painter.text(Pos2::new(o.x + 3.0 * ICON_W + 6.0, y + ROW_H / 2.0 - 1.0), Align2::LEFT_CENTER, &layer.name, font.clone(), name_col);

            // Zellen
            for f in 0..n {
                let c = egui::Rect::from_min_size(Pos2::new(col_x(f), y), Vec2::new(CELL_W, ROW_H - 2.0));
                let active = f == sp.frame && l == sp.layer;
                let bg = if active {
                    ACCENT.gamma_multiply(0.45)
                } else if f == sp.frame || l == sp.layer {
                    Color32::from_gray(44)
                } else {
                    Color32::from_gray(34)
                };
                painter.rect_filled(c.shrink(1.0), 3.0, bg);
                let id = sp.frames[f].cels[l];
                // Verknüpft mit dem Nachbarn: Strich zwischen den Punkten.
                if f + 1 < n && sp.frames[f + 1].cels[l] == id {
                    painter.line_segment(
                        [c.center(), Pos2::new(col_x(f + 1) + CELL_W / 2.0, c.center().y)],
                        Stroke::new(2.0, DIM),
                    );
                }
                let filled = !sp.cel_is_empty(f, l);
                let dot = if active { ACCENT } else { DIM };
                if filled {
                    painter.circle_filled(c.center(), 4.0, dot);
                } else {
                    painter.circle_stroke(c.center(), 4.0, Stroke::new(1.5, dot.gamma_multiply(0.7)));
                }
            }
        }

        // Klicks auswerten
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
            let fx = ((p.x - o.x - LAYER_W) / CELL_W).floor();
            let ry = ((p.y - o.y - HEAD_H) / ROW_H).floor();
            if p.y < o.y + HEAD_H {
                if fx >= 0.0 && (fx as usize) < n {
                    self.go_frame(fx as usize);
                }
            } else if ry >= 0.0 && (ry as usize) < nl {
                let l = nl - 1 - ry as usize;
                if p.x < o.x + LAYER_W {
                    let k = ((p.x - o.x) / ICON_W).floor() as usize;
                    match k {
                        0 => self.edit_sprite(|s| s.layers[l].visible = !s.layers[l].visible),
                        1 => self.edit_sprite(|s| s.layers[l].locked = !s.layers[l].locked),
                        2 => self.edit_sprite(|s| s.layers[l].continuous = !s.layers[l].continuous),
                        _ => self.project.sprite_mut().layer = l,
                    }
                } else if fx >= 0.0 && (fx as usize) < n {
                    let s = self.project.sprite_mut();
                    s.layer = l;
                    s.frame = fx as usize;
                    self.stroke_last = None;
                }
            }
        }
    }

    // ── Abspielen ───────────────────────────────────────────────────
    pub(crate) fn toggle_play(&mut self, ctx: &egui::Context) {
        self.playing = !self.playing && self.project.sprite().frames.len() > 1;
        self.frame_started = ctx.input(|i| i.time);
    }

    /// Bei jedem Durchlauf: ist die Zeit des Frames um, kommt der nächste.
    /// Steht man in einem Tag, läuft nur dieser.
    pub(crate) fn advance_playback(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let now = ctx.input(|i| i.time);
        let sp = self.project.sprite();
        let dur = sp.frame_duration(sp.frame) as f64 / 1000.0;
        if now - self.frame_started >= dur {
            let f = sp.frame;
            let next = match sp.tags.iter().find(|t| f >= t.from && f <= t.to && t.to > t.from) {
                Some(t) if f >= t.to => t.from,
                Some(_) => f + 1,
                None => (f + 1) % sp.frames.len(),
            };
            self.project.sprite_mut().frame = next;
            self.frame_started = now;
        }
        let sp = self.project.sprite();
        let left = sp.frame_duration(sp.frame) as f64 / 1000.0 - (now - self.frame_started);
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(left.max(0.001)));
    }
}
