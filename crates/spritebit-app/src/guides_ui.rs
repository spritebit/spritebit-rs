//! Hilfslinien — wie `guides.js` der Web-Version: freie Linien und
//! Figuren-Proportionen. Nur auf der Zeichenfläche zu sehen, nie im
//! Export, kein Undo; gespeichert je Sprite.
//!
//! * Freie Linien: waagerecht und senkrecht, immer auf Pixelgrenzen.
//! * Figur: `heads` Kopfhöhen zwischen Ober- und Unterkante, mit Marken
//!   (Kinn, Brust, Hüfte, Knie …) und der Körperachse in der Mitte.
//!
//! Im Modus „Verschieben“ gehört die Fläche den Linien, gemalt wird nicht.
//! Eine freie Linie, die man aus dem Bild zieht, ist gelöscht; ein Klick
//! neben die Linien oder Esc beendet den Modus. G blendet alle ein und aus.

use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Stroke};
use spritebit_core::{transform, FIGURE_HEADS};

use crate::i18n::tr;
use crate::SpritebitApp;

const FREE: Color32 = Color32::from_rgba_premultiplied(42, 190, 230, 230);
const FIG: Color32 = Color32::from_rgba_premultiplied(230, 115, 180, 230);

/// Welche Linie gezogen wird.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GuideHit {
    H(usize),
    V(usize),
    Top,
    Bottom,
}

#[derive(Default)]
pub(crate) struct GuideState {
    pub show: bool,
    pub edit: bool,
    pub drag: Option<GuideHit>,
}

/// Marken je Einteilung: (Kopfhöhen von oben, Name).
fn marks(heads: u32) -> &'static [(f32, &'static str)] {
    match heads {
        2 => &[(1.0, "Kinn")],
        3 => &[(1.0, "Kinn"), (2.0, "Hüfte")],
        4 => &[(1.0, "Kinn"), (2.0, "Hüfte"), (3.0, "Knie")],
        6 => &[(1.0, "Kinn"), (2.0, "Brust"), (3.0, "Hüfte"), (4.5, "Knie")],
        8 => &[(1.0, "Kinn"), (2.0, "Brust"), (3.0, "Nabel"), (4.0, "Schritt"), (6.0, "Knie")],
        _ => &[],
    }
}

fn heads_label(n: u32) -> &'static str {
    match n {
        2 => tr("2 Köpfe — Chibi"),
        3 => tr("3 Köpfe — klein, niedlich"),
        4 => tr("4 Köpfe — kompakte Spielfigur"),
        6 => tr("6 Köpfe — Comic, Jugendliche"),
        8 => tr("8 Köpfe — klassisch, heldenhaft"),
        _ => tr("Aus"),
    }
}

/// Gestrichelte waagerechte Linie.
fn dashed(p: &egui::Painter, y: f32, x0: f32, x1: f32, dash: f32, gap: f32, stroke: Stroke) {
    let mut x = x0;
    while x < x1 {
        p.line_segment([Pos2::new(x, y), Pos2::new((x + dash).min(x1), y)], stroke);
        x += dash + gap;
    }
}

impl SpritebitApp {
    fn guides_mut(&mut self) -> &mut spritebit_core::Guides {
        &mut self.project.sprite_mut().guides
    }

    pub(crate) fn set_guide_edit(&mut self, on: bool) {
        self.guides.edit = on;
        self.guides.drag = None;
        if on {
            self.guides.show = true;
            self.hint = Some(tr("Hilfslinien verschieben: Linie anfassen und ziehen, aus dem Bild ziehen löscht. Klick daneben oder Esc beendet.").into());
        } else {
            self.hint = None;
        }
    }

    /// Nichts mehr zu verschieben → zurück zum Malen.
    fn leave_if_empty(&mut self) {
        let g = &self.sprite().guides;
        if self.guides.edit && g.h.is_empty() && g.v.is_empty() && g.heads == 0 {
            self.set_guide_edit(false);
        }
    }

    /// Neue Linie in der Mitte; danach gleich verschiebbar.
    fn add_line(&mut self, horizontal: bool) {
        let (w, h) = (self.sprite().width, self.sprite().height);
        let max = if horizontal { h } else { w };
        let g = self.guides_mut();
        let list = if horizontal { &mut g.h } else { &mut g.v };
        let mut pos = max / 2;
        while list.contains(&pos) && pos < max {
            pos += 1;
        }
        list.push(pos);
        list.sort_unstable();
        list.dedup();
        self.dirty = true;
        self.set_guide_edit(true);
    }

    /// Ober- und Unterkante auf den gezeichneten Inhalt aller Ebenen.
    fn fit_figure(&mut self) {
        let sp = self.sprite();
        let f = sp.frame;
        let mut b: Option<(u32, u32)> = None;
        for l in 0..sp.layers.len() {
            if !sp.layers[l].visible {
                continue;
            }
            if let Some((_, y, _, h)) = transform::bounds(sp.cel(f, l)) {
                b = Some(b.map_or((y, y + h), |(t, bo)| (t.min(y), bo.max(y + h))));
            }
        }
        let height = sp.height;
        let g = self.guides_mut();
        (g.top, g.bottom) = b.unwrap_or((0, height));
        if g.heads == 0 {
            g.heads = 6;
        }
        self.guides.show = true;
        self.dirty = true;
    }

    pub(crate) fn guides_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.selectable_label(self.guides.show, tr("Anzeigen")).on_hover_text(tr("Alle Hilfslinien ein- und ausblenden (G)")).clicked() {
                self.guides.show = !self.guides.show;
                if !self.guides.show {
                    self.set_guide_edit(false);
                }
            }
            if ui
                .selectable_label(self.guides.edit, tr("Verschieben"))
                .on_hover_text(tr("Linien auf der Fläche ziehen — solange wird nicht gemalt (Klick daneben oder Esc beendet)"))
                .clicked()
            {
                let on = !self.guides.edit;
                self.set_guide_edit(on);
            }
        });
        ui.label(tr("Freie Linien"));
        ui.horizontal(|ui| {
            if ui.button(tr("+ Waagerecht")).on_hover_text(tr("Eine waagerechte Linie in die Mitte setzen")).clicked() {
                self.add_line(true);
            }
            if ui.button(tr("+ Senkrecht")).on_hover_text(tr("Eine senkrechte Linie in die Mitte setzen")).clicked() {
                self.add_line(false);
            }
        });
        let g = &self.sprite().guides;
        if ui.add_enabled(!g.h.is_empty() || !g.v.is_empty(), egui::Button::new(tr("Alle löschen"))).clicked() {
            let g = self.guides_mut();
            g.h.clear();
            g.v.clear();
            self.dirty = true;
            self.leave_if_empty();
        }
        ui.label(tr("Figur — Kopfhöhen"));
        let mut heads = self.sprite().guides.heads;
        egui::ComboBox::from_id_salt("gd-heads").selected_text(heads_label(heads)).width(200.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut heads, 0, heads_label(0));
            for n in FIGURE_HEADS {
                ui.selectable_value(&mut heads, n, heads_label(n));
            }
        });
        if heads != self.sprite().guides.heads {
            self.guides_mut().heads = heads;
            if heads > 0 {
                self.guides.show = true;
            }
            self.dirty = true;
            self.leave_if_empty();
        }
        if ui.button(tr("An Figur anpassen")).on_hover_text(tr("Ober- und Unterkante der Einteilung auf das Gezeichnete setzen")).clicked() {
            self.fit_figure();
        }
        ui.weak(tr("Nur zum Zeichnen — die Linien erscheinen in keinem Export."));
    }

    /// G: ein/aus. Esc: Verschieben beenden.
    pub(crate) fn guide_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (g, esc) = ctx.input(|i| (i.key_pressed(Key::G) && i.modifiers.is_none(), i.key_pressed(Key::Escape)));
        if g {
            self.guides.show = !self.guides.show;
            if !self.guides.show {
                self.set_guide_edit(false);
            }
        }
        if esc && self.guides.edit {
            self.set_guide_edit(false);
        }
    }

    /// Welche Linie liegt unter dem Zeiger (Bildschirm-Koordinaten)?
    fn guide_hit(&self, p: Pos2, origin: Pos2, zoom: f32) -> Option<GuideHit> {
        let g = self.sprite().guides.normalized(self.sprite().width, self.sprite().height);
        let tol = (zoom / 2.0).max(6.0);
        let mut best: Option<(f32, GuideHit)> = None;
        let mut take = |d: f32, hit: GuideHit| {
            if d <= tol && best.is_none_or(|b| d < b.0) {
                best = Some((d, hit));
            }
        };
        let (px, py) = ((p.x - origin.x) / zoom, (p.y - origin.y) / zoom);
        for (i, &y) in g.h.iter().enumerate() {
            take((py - y as f32).abs() * zoom, GuideHit::H(i));
        }
        for (i, &x) in g.v.iter().enumerate() {
            take((px - x as f32).abs() * zoom, GuideHit::V(i));
        }
        if g.heads > 0 {
            take((py - g.top as f32).abs() * zoom, GuideHit::Top);
            take((py - g.bottom as f32).abs() * zoom, GuideHit::Bottom);
        }
        best.map(|b| b.1)
    }

    /// Zeiger im Verschieben-Modus. Gibt `true` zurück, wenn der Modus den
    /// Zeiger genommen hat (dann wird nicht gemalt).
    pub(crate) fn guide_pointer(&mut self, pointer: Option<Pos2>, pressed: bool, released: bool, over: bool, origin: Pos2, zoom: f32) -> bool {
        // Hand-Werkzeug: eine Linie unter dem Zeiger lässt sich auch ohne den
        // Verschieben-Modus greifen (wie im Web). Daneben verschiebt die Hand.
        if !self.guides.edit {
            if self.tool != crate::tools_ui::Tool::Pan || !self.guides.show {
                return false;
            }
            if pressed && over {
                if let Some(p) = pointer {
                    self.guides.drag = self.guide_hit(p, origin, zoom);
                }
            }
            if self.guides.drag.is_none() {
                return false;
            }
        }
        let (w, h) = (self.sprite().width as i64, self.sprite().height as i64);
        if pressed && over {
            if let Some(p) = pointer {
                // Gespeicherte Werte erst auf die Fläche begrenzen, dann treffen.
                let n = self.sprite().guides.normalized(w as u32, h as u32);
                self.project.sprite_mut().guides = n;
                self.guides.drag = self.guide_hit(p, origin, zoom);
                if self.guides.drag.is_none() {
                    self.set_guide_edit(false);
                    self.blocked = true;
                    return true;
                }
            }
        }
        if let (Some(hit), Some(p)) = (self.guides.drag, pointer) {
            let x = ((p.x - origin.x) / zoom).round() as i64;
            let y = ((p.y - origin.y) / zoom).round() as i64;
            let g = self.guides_mut();
            // Während des Ziehens darf eine freie Linie außerhalb liegen —
            // beim Loslassen ist sie dann weg.
            match hit {
                GuideHit::H(i) => g.h[i] = y.clamp(-1, h + 1) as u32,
                GuideHit::V(i) => g.v[i] = x.clamp(-1, w + 1) as u32,
                GuideHit::Top => g.top = y.clamp(0, g.bottom as i64 - 1) as u32,
                GuideHit::Bottom => g.bottom = y.clamp(g.top as i64 + 1, h) as u32,
            }
            self.dirty = true;
        }
        if released {
            if let Some(hit) = self.guides.drag.take() {
                let g = self.guides_mut();
                let out = |v: u32, max: i64| v as i64 > max || v == u32::MAX;
                let removed = match hit {
                    GuideHit::H(i) if out(g.h[i], h) => {
                        g.h.remove(i);
                        true
                    }
                    GuideHit::V(i) if out(g.v[i], w) => {
                        g.v.remove(i);
                        true
                    }
                    _ => false,
                };
                g.h.sort_unstable();
                g.h.dedup();
                g.v.sort_unstable();
                g.v.dedup();
                if removed {
                    self.hint = Some(tr("Hilfslinie entfernt.").into());
                }
                self.leave_if_empty();
            }
        }
        true
    }

    pub(crate) fn draw_guides(&self, painter: &egui::Painter, origin: Pos2, zoom: f32) {
        if !self.guides.show {
            return;
        }
        let sp = self.sprite();
        let g = sp.guides.normalized(sp.width, sp.height);
        let (w, h) = (sp.width as f32 * zoom, sp.height as f32 * zoom);
        let wide = if self.guides.edit { 2.0 } else { 1.0 };
        let (x0, x1) = (origin.x, origin.x + w);
        if g.heads > 0 {
            let unit = (g.bottom - g.top) as f32 / g.heads as f32;
            let y_of = |k: f32| origin.y + ((g.top as f32 + k * unit) * zoom).round();
            let solid = Stroke::new(wide, FIG);
            for y in [g.top, g.bottom] {
                let y = origin.y + y as f32 * zoom;
                painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], solid);
            }
            let thin = Stroke::new(1.0, FIG);
            for k in 1..g.heads {
                dashed(painter, y_of(k as f32), x0, x1, 4.0, 3.0, thin);
            }
            // Körperachse
            let cx = origin.x + w / 2.0;
            let (ya, yb) = (origin.y + g.top as f32 * zoom, origin.y + g.bottom as f32 * zoom);
            let mut y = ya;
            while y < yb {
                painter.line_segment([Pos2::new(cx, y), Pos2::new(cx, (y + 4.0).min(yb))], thin);
                y += 7.0;
            }
            // Marken zwischen zwei Kopfhöhen bekommen eine eigene, gepunktete Linie.
            for &(k, _) in marks(g.heads) {
                if k.fract() != 0.0 {
                    dashed(painter, y_of(k), x0, x1, 1.0, 3.0, thin);
                }
            }
            if zoom >= 4.0 {
                let font = FontId::proportional(10.0);
                for k in 0..g.heads {
                    painter.text(Pos2::new(x0 + 3.0, y_of(k as f32) + 2.0), Align2::LEFT_TOP, format!("{}", k + 1), font.clone(), FIG);
                }
                for &(k, name) in marks(g.heads) {
                    painter.text(Pos2::new(x1 - 3.0, y_of(k) - 2.0), Align2::RIGHT_BOTTOM, tr(name), font.clone(), FIG);
                }
            }
        }
        let free = Stroke::new(wide, FREE);
        for &y in &g.h {
            let y = origin.y + y as f32 * zoom;
            painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], free);
        }
        for &x in &g.v {
            let x = origin.x + x as f32 * zoom;
            painter.line_segment([Pos2::new(x, origin.y), Pos2::new(x, origin.y + h)], free);
        }
    }
}
