//! Auswahl in der App: aufziehen, verschieben, kopieren, einfügen.
//!
//! Werkzeuge: Auswahl (Rechteck), Lasso, Farbwahl (zusammenhängende
//! ähnliche Fläche), Zauberstab (dieselbe Fläche gleich löschen).
//! Ziehen IN einer Auswahl hebt ihren Inhalt an („schwebend") und schiebt
//! ihn; abgesetzt wird er, sobald man etwas anderes tut. Das ganze
//! Verschieben ist ein Undo-Schritt.

use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Stroke, Vec2};
use spritebit_core::selection::{self, Clip, Selection};

use crate::tools_ui::{Pointer, Tool};
use crate::SpritebitApp;
use crate::i18n::{tr, keys};

/// Schwebender Inhalt: Pixel und wo ihre linke obere Ecke gerade liegt.
pub(crate) struct Float {
    pub clip: Clip,
    pub x: i64,
    pub y: i64,
}

/// Ein laufender Zug mit einem Auswahl-Werkzeug.
pub(crate) enum SelDrag {
    /// Rechteck aufziehen ab dieser Ecke.
    Rect((i64, i64)),
    /// Lasso: die bisherigen Punkte.
    Lasso(Vec<(i64, i64)>),
    /// Schwebenden Inhalt schieben: Startpunkt und Lage zu Beginn.
    Move { start: (i64, i64), from: (i64, i64) },
}

impl SpritebitApp {
    /// Schwebenden Inhalt absetzen (ohne neuen Undo-Schritt — das Anheben
    /// hat ihn schon angelegt).
    pub(crate) fn commit_float(&mut self) {
        if let Some(f) = self.float.take() {
            selection::paste(self.project.sprite_mut().active(), &f.clip, f.x, f.y);
            self.changed();
        }
    }

    /// Auswahl aufheben (setzt vorher Schwebendes ab).
    pub(crate) fn deselect(&mut self) {
        self.commit_float();
        self.selection = None;
        self.sel_drag = None;
    }

    /// Inhalt der Auswahl anheben: aus dem Bild nehmen, schwebend halten.
    /// Wie [`Self::lift`], aber das Bild darunter bleibt (Alt+Ziehen).
    fn lift_copy(&mut self) {
        let Some(sel) = self.selection.clone() else { return };
        self.record_step();
        let img = self.project.sprite_mut().active();
        let clip = selection::copy(img, &sel);
        self.float = Some(Float { clip, x: sel.x, y: sel.y });
        self.changed();
    }

    pub(crate) fn lift(&mut self) {
        if self.float.is_some() {
            return;
        }
        let Some(sel) = self.selection.clone() else { return };
        self.record_step();
        let img = self.project.sprite_mut().active();
        let clip = selection::copy(img, &sel);
        selection::clear(img, &sel);
        self.float = Some(Float { clip, x: sel.x, y: sel.y });
        self.changed();
    }

    fn record_step(&mut self) {
        let cur = self.project.current;
        self.histories[cur].record(&self.project.sprites[cur]);
    }

    pub(crate) fn is_select_tool(&self) -> bool {
        matches!(self.tool, Tool::Select | Tool::Lasso | Tool::Magic | Tool::Wand)
    }

    /// Ein Durchlauf eines Auswahl-Werkzeugs.
    pub(crate) fn use_select_tool(&mut self, p: &Pointer) {
        let Some(cell) = p.cell else { return };
        if p.pressed && p.over && !p.secondary {
            let inside = self.selection.as_ref().is_some_and(|s| s.contains(cell.0, cell.1));
            if inside && self.tool != Tool::Wand {
                // Alt: das Original bleibt stehen, verschoben wird eine Kopie.
                if p.alt && self.float.is_none() {
                    self.lift_copy();
                } else {
                    self.lift();
                }
                let from = self.float.as_ref().map_or((0, 0), |f| (f.x, f.y));
                self.sel_drag = Some(SelDrag::Move { start: cell, from });
                return;
            }
            self.deselect();
            let palette = self.project.current_palette();
            match self.tool {
                Tool::Select => self.sel_drag = Some(SelDrag::Rect(cell)),
                Tool::Lasso => self.sel_drag = Some(SelDrag::Lasso(vec![cell])),
                Tool::Magic => {
                    let sp = self.project.sprite();
                    self.selection = selection::region(sp.cel(sp.frame, sp.layer), &palette, &sp.free, cell.0, cell.1, self.tolerance);
                }
                Tool::Wand => {
                    if !self.layer_ok() {
                        return;
                    }
                    let sp = self.project.sprite();
                    if let Some(r) = selection::region(sp.cel(sp.frame, sp.layer), &palette, &sp.free, cell.0, cell.1, self.tolerance) {
                        self.record_step();
                        let n = selection::clear(self.project.sprite_mut().active(), &r);
                        if n > 0 {
                            self.changed();
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        let down = p.primary;
        match &mut self.sel_drag {
            Some(SelDrag::Rect(start)) => {
                let s = *start;
                self.selection = Some(Selection::rect(s.0, s.1, cell.0, cell.1));
                if !down {
                    self.sel_drag = None;
                }
            }
            Some(SelDrag::Lasso(points)) => {
                if points.last() != Some(&cell) {
                    // Lücken zwischen schnellen Bewegungen auffüllen.
                    let last = *points.last().expect("Lasso hat einen Startpunkt");
                    points.extend(spritebit_core::tools::line(last.0, last.1, cell.0, cell.1).into_iter().skip(1));
                }
                if !down {
                    let pts = std::mem::take(points);
                    self.selection = Selection::lasso(&pts);
                    self.sel_drag = None;
                }
            }
            Some(SelDrag::Move { start, from }) => {
                let (dx, dy) = (cell.0 - start.0, cell.1 - start.1);
                let (fx, fy) = *from;
                if let Some(f) = &mut self.float {
                    if (f.x, f.y) != (fx + dx, fy + dy) {
                        f.x = fx + dx;
                        f.y = fy + dy;
                        if let Some(s) = &mut self.selection {
                            *s = Selection { x: f.x, y: f.y, ..s.clone() };
                        }
                        self.version = self.version.wrapping_add(1);
                    }
                }
                if !down {
                    self.sel_drag = None;
                }
            }
            None => {}
        }
    }

    // ── Befehle ─────────────────────────────────────────────────────
    pub(crate) fn select_all(&mut self) {
        self.deselect();
        let sp = self.project.sprite();
        self.selection = Some(Selection::all(sp.width, sp.height));
        self.tool = Tool::Select;
    }

    pub(crate) fn copy_selection(&mut self) {
        let Some(sel) = &self.selection else { return };
        self.clipboard = Some(match &self.float {
            Some(f) => f.clip.clone(),
            None => {
                let sp = self.project.sprite();
                selection::copy(sp.cel(sp.frame, sp.layer), sel)
            }
        });
    }

    pub(crate) fn cut_selection(&mut self) {
        if self.selection.is_none() || !self.layer_ok() {
            return;
        }
        self.copy_selection();
        self.delete_selection();
    }

    /// Inhalt der Auswahl leeren (Entf).
    pub(crate) fn delete_selection(&mut self) {
        if self.float.take().is_some() {
            // Schwebendes einfach verwerfen — das Loch darunter ist schon da.
            self.changed();
            return;
        }
        let Some(sel) = self.selection.clone() else { return };
        if !self.layer_ok() {
            return;
        }
        self.record_step();
        if selection::clear(self.project.sprite_mut().active(), &sel) > 0 {
            self.changed();
        }
    }

    pub(crate) fn fill_selection(&mut self) {
        let Some(sel) = self.selection.clone() else { return };
        if !self.layer_ok() {
            return;
        }
        self.commit_float();
        self.record_step();
        let v = self.color;
        selection::fill(self.project.sprite_mut().active(), &sel, v);
        self.changed();
    }

    /// Zwischenablage einfügen — schwebend, an der Stelle der Auswahl oder
    /// links oben; danach mit der Maus an den Platz ziehen.
    pub(crate) fn paste_clipboard(&mut self) {
        let Some(clip) = self.clipboard.clone() else { return };
        if !self.layer_ok() {
            return;
        }
        let (x, y) = self.selection.as_ref().map_or((0, 0), |s| (s.x, s.y));
        self.deselect();
        self.record_step();
        self.selection = Some(Selection { x, y, w: clip.w, h: clip.h, mask: None });
        self.float = Some(Float { clip, x, y });
        self.tool = Tool::Select;
        self.changed();
    }

    /// Pfeiltasten schieben die Auswahl um einen Pixel.
    fn nudge(&mut self, dx: i64, dy: i64) {
        if self.selection.is_none() {
            return;
        }
        self.lift();
        if let Some(f) = &mut self.float {
            f.x += dx;
            f.y += dy;
        }
        if let Some(s) = &mut self.selection {
            *s = s.moved(dx, dy);
        }
        self.version = self.version.wrapping_add(1);
    }

    /// Tasten für die Auswahl. Gibt es keine Auswahl, bleiben die Pfeile frei.
    pub(crate) fn selection_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let cmd = Modifiers::COMMAND;
        let (all, copy, cut, paste) = ctx.input_mut(|i| {
            (
                i.consume_key(cmd, Key::A),
                i.consume_key(cmd, Key::C),
                i.consume_key(cmd, Key::X),
                i.consume_key(cmd, Key::V),
            )
        });
        if all {
            self.select_all();
        }
        if copy {
            self.copy_selection();
        }
        if cut {
            self.cut_selection();
        }
        if paste {
            self.paste_clipboard();
        }
        if self.selection.is_none() {
            return;
        }
        let none = Modifiers::NONE;
        let (del, back, esc, l, r, u, d) = ctx.input_mut(|i| {
            (
                i.consume_key(none, Key::Delete),
                i.consume_key(none, Key::Backspace),
                i.consume_key(none, Key::Escape),
                i.consume_key(none, Key::ArrowLeft),
                i.consume_key(none, Key::ArrowRight),
                i.consume_key(none, Key::ArrowUp),
                i.consume_key(none, Key::ArrowDown),
            )
        });
        if del || back {
            self.delete_selection();
        }
        if esc {
            self.deselect();
        }
        for (hit, dx, dy) in [(l, -1, 0), (r, 1, 0), (u, 0, -1), (d, 0, 1)] {
            if hit {
                self.nudge(dx, dy);
            }
        }
    }

    /// Knöpfe neben den Werkzeugen, solange es eine Auswahl gibt.
    pub(crate) fn selection_bar(&mut self, ui: &mut egui::Ui) {
        if matches!(self.tool, Tool::Magic | Tool::Wand) {
            ui.label(tr("Toleranz"));
            let mut pct = (self.tolerance * 100.0).round() as u32;
            if ui.add(egui::Slider::new(&mut pct, 0..=100).suffix(" %")).changed() {
                self.tolerance = pct as f64 / 100.0;
            }
        }
        if self.is_select_tool() || self.selection.is_some() || self.clipboard.is_some() {
            ui.separator();
            if ui.button(tr("Alles")).on_hover_text(keys("Strg+A")).clicked() {
                self.select_all();
            }
            let has = self.selection.is_some();
            if ui.add_enabled(has, egui::Button::new(tr("Ausschneiden"))).on_hover_text(keys("Strg+X")).clicked() {
                self.cut_selection();
            }
            if ui.add_enabled(has, egui::Button::new(tr("Kopieren"))).on_hover_text(keys("Strg+C")).clicked() {
                self.copy_selection();
            }
            if ui.add_enabled(self.clipboard.is_some(), egui::Button::new(tr("Einfügen"))).on_hover_text(keys("Strg+V")).clicked() {
                self.paste_clipboard();
            }
            if ui.add_enabled(has, egui::Button::new(tr("Füllen"))).on_hover_text(tr("Auswahl mit der aktuellen Farbe füllen")).clicked() {
                self.fill_selection();
            }
            if ui.add_enabled(has, egui::Button::new(tr("Leeren"))).on_hover_text(keys("Entf")).clicked() {
                self.delete_selection();
            }
            if ui.add_enabled(has, egui::Button::new(tr("Aufheben"))).on_hover_text(keys("Esc")).clicked() {
                self.deselect();
            }
        }
    }

    /// Rahmen der Auswahl („laufende Ameisen", hier still) und das Lasso
    /// beim Ziehen. Bei einer Maske nur die Kanten im sichtbaren Bereich.
    pub(crate) fn selection_overlay(&self, painter: &egui::Painter, origin: Pos2, zoom: f32, vis: (i64, i64, i64, i64)) {
        let dark = Stroke::new(2.0, Color32::from_black_alpha(200));
        let light = Stroke::new(1.0, Color32::WHITE);
        let to = |x: i64, y: i64| origin + Vec2::new(x as f32, y as f32) * zoom;
        if let Some(SelDrag::Lasso(points)) = &self.sel_drag {
            let pts: Vec<Pos2> = points.iter().map(|&(x, y)| to(x, y) + Vec2::splat(zoom / 2.0)).collect();
            painter.add(egui::Shape::line(pts.clone(), dark));
            painter.add(egui::Shape::line(pts, light));
        }
        let Some(s) = &self.selection else { return };
        match &s.mask {
            None => {
                let r = egui::Rect::from_min_max(to(s.x, s.y), to(s.x + s.w as i64, s.y + s.h as i64));
                painter.rect_stroke(r, 0.0, dark, egui::StrokeKind::Middle);
                painter.rect_stroke(r, 0.0, light, egui::StrokeKind::Middle);
            }
            Some(_) => {
                let (vx0, vy0, vx1, vy1) = vis;
                let x0 = s.x.max(vx0);
                let y0 = s.y.max(vy0);
                let x1 = (s.x + s.w as i64).min(vx1);
                let y1 = (s.y + s.h as i64).min(vy1);
                let mut segs = Vec::new();
                for y in y0..y1 {
                    for x in x0..x1 {
                        if !s.contains(x, y) {
                            continue;
                        }
                        if !s.contains(x, y - 1) {
                            segs.push([to(x, y), to(x + 1, y)]);
                        }
                        if !s.contains(x, y + 1) {
                            segs.push([to(x, y + 1), to(x + 1, y + 1)]);
                        }
                        if !s.contains(x - 1, y) {
                            segs.push([to(x, y), to(x, y + 1)]);
                        }
                        if !s.contains(x + 1, y) {
                            segs.push([to(x + 1, y), to(x + 1, y + 1)]);
                        }
                    }
                    if segs.len() > 40_000 {
                        break; // sehr große Masken: nur der Anfang, die Fläche bleibt erkennbar
                    }
                }
                for seg in &segs {
                    painter.line_segment(*seg, dark);
                }
                for seg in segs {
                    painter.line_segment(seg, light);
                }
            }
        }
    }
}
