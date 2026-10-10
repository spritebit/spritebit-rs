//! Hilfslinien — wie `guides.js` der Web-Version: freie Linien und
//! Figuren-Proportionen. Nur auf der Zeichenfläche zu sehen, nie im
//! Export, kein Undo; gespeichert je Sprite.
//!
//! * Freie Linien: waagerecht und senkrecht, immer auf Pixelgrenzen.
//! * Figur: `heads` Kopfhöhen zwischen Ober- und Unterkante, mit Marken
//!   (Kinn, Brust, Hüfte, Knie …) und der Körperachse in der Mitte.
//!
//! Gezogen wird mit dem Hand-Werkzeug: auf einer Linie zieht es die Linie,
//! daneben die Ansicht. Eine Kopfhöhe der Figur zieht die ganze Figur, Ober-
//! und Unterkante ändern ihre Größe. Eine freie Linie, die man aus dem Bild
//! zieht, ist gelöscht. „Sperren“ hält alle Linien fest. G blendet sie ein
//! und aus.

use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Stroke};
use spritebit_core::FIGURE_HEADS;

use crate::i18n::{tr, trf};
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
    /// Die ganze Figur, an einer Kopfhöhe gegriffen: Abstand des Griffs
    /// zur Oberkante (Sprite-Pixel).
    Figure(i64),
}

#[derive(Default)]
pub(crate) struct GuideState {
    pub show: bool,
    /// Gesperrt: keine Linie lässt sich ziehen.
    pub locked: bool,
    pub drag: Option<GuideHit>,
    /// Eigene Layouts — für alle Sprites, im Einstellungsordner gespeichert.
    pub layouts: Vec<GuideLayout>,
    /// Gewähltes Layout in der Liste.
    pub layout_sel: usize,
    /// Name im Feld „Name des Layouts“.
    pub layout_name: String,
    /// Nur die echte App schreibt die Datei (Tests nicht).
    pub persist: bool,
}

/// Ein gespeichertes Hilfslinien-Layout: Linien und Einteilung eines
/// Sprites samt seiner Größe — wie in der Web-Version (js/guidelayouts.js).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GuideLayout {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub guides: spritebit_core::Guides,
}

fn layouts_path() -> Option<std::path::PathBuf> {
    crate::i18n::settings_dir().map(|d| d.join("guide_layouts.json"))
}

/// Liste aus JSON (gleiches Format wie im Web); Unbrauchbares fällt weg.
pub(crate) fn parse_layouts(v: &serde_json::Value) -> Vec<GuideLayout> {
    let mut out: Vec<GuideLayout> = Vec::new();
    for l in v.as_array().into_iter().flatten() {
        let name = l.get("name").and_then(|n| n.as_str()).map(str::trim).unwrap_or("");
        let w = l.get("width").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
        let h = l.get("height").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
        if name.is_empty() || w == 0 || h == 0 || out.iter().any(|o| o.name == name) {
            continue;
        }
        let guides = spritebit_core::io::parse_guides(l.get("guides"), w, h);
        out.push(GuideLayout { name: name.to_string(), width: w, height: h, guides });
    }
    out
}

pub(crate) fn layouts_json(list: &[GuideLayout]) -> serde_json::Value {
    serde_json::Value::Array(
        list.iter()
            .map(|l| serde_json::json!({ "name": l.name, "width": l.width, "height": l.height, "guides": spritebit_core::io::guides_json(&l.guides) }))
            .collect(),
    )
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
    pub(crate) fn guides_mut(&mut self) -> &mut spritebit_core::Guides {
        &mut self.project.sprite_mut().guides
    }

    /// Neue Linie in der Mitte.
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
        self.guides.show = true;
    }

    /// `n` Linien einer Richtung gleichmäßig verteilen (die bisherigen ersetzt).
    pub(crate) fn set_even(&mut self, horizontal: bool, n: u32) {
        let (w, h) = (self.sprite().width, self.sprite().height);
        let lines = spritebit_core::Guides::even_lines(n, if horizontal { h } else { w });
        let any = !lines.is_empty();
        let g = self.guides_mut();
        if horizontal {
            g.h = lines;
        } else {
            g.v = lines;
        }
        if any {
            self.guides.show = true;
        }
        self.dirty = true;
    }

    pub(crate) fn guides_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Sagt, was ein Klick tut — „Anzeigen“ las sich, als wären sie gerade aus.
            let label = if self.guides.show { tr("Ausblenden") } else { tr("Einblenden") };
            if ui.selectable_label(self.guides.show, label).on_hover_text(tr("Alle Hilfslinien ein- und ausblenden (G)")).clicked() {
                self.guides.show = !self.guides.show;
                self.guides.drag = None;
            }
            // Wie Aus-/Einblenden: die Aufschrift sagt, was ein Klick tut.
            let lock = if self.guides.locked { tr("Entsperren") } else { tr("Sperren") };
            if ui
                .selectable_label(self.guides.locked, lock)
                .on_hover_text(tr("Gesperrt lassen sich die Linien nicht verschieben — auch nicht mit der Hand"))
                .clicked()
            {
                self.guides.locked = !self.guides.locked;
                self.guides.drag = None;
            }
        });
        ui.weak(if self.guides.locked {
            tr("Gesperrt — die Linien bleiben, wo sie sind.")
        } else {
            tr("Linien mit der Hand (H) ziehen; aus dem Bild gezogen ist eine Linie gelöscht.")
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
        // Gleichmäßig verteilen (wie in Photoshop): jede Änderung wirkt sofort.
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Gleichmäßig"));
            let tip = tr("Anzahl eintippen — die Linien verteilen sich sofort gleichmäßig (4 Linien = 5 gleiche Teile). 0 entfernt sie.");
            for (horizontal, unit) in [(true, tr("waagerecht")), (false, tr("senkrecht"))] {
                let mut n = if horizontal { self.sprite().guides.h.len() } else { self.sprite().guides.v.len() } as u32;
                if ui.add(egui::DragValue::new(&mut n).range(0..=64).speed(0.1)).on_hover_text(tip).changed() {
                    self.set_even(horizontal, n);
                }
                ui.label(unit);
            }
        });
        let g = &self.sprite().guides;
        if ui.add_enabled(!g.h.is_empty() || !g.v.is_empty(), egui::Button::new(tr("Alle löschen"))).clicked() {
            let g = self.guides_mut();
            g.h.clear();
            g.v.clear();
            self.dirty = true;
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
        }
        self.guide_layouts_ui(ui);
        ui.weak(tr("Nur zum Zeichnen — die Linien erscheinen in keinem Export."));
    }

    /// Beim Start: gespeicherte Layouts laden.
    pub(crate) fn load_guide_layouts(&mut self) {
        self.guides.persist = true;
        if let Some(v) = layouts_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()) {
            self.guides.layouts = parse_layouts(&v);
        }
    }

    fn store_guide_layouts(&mut self) {
        if !self.guides.persist {
            return;
        }
        let Some(p) = layouts_path() else { return };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if std::fs::write(&p, layouts_json(&self.guides.layouts).to_string()).is_err() {
            self.hint = Some(tr("Layouts konnten nicht gespeichert werden.").into());
        }
    }

    /// Jetzige Linien und Einteilung unter `name` speichern (gleicher Name ersetzt).
    pub(crate) fn save_guide_layout(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.hint = Some(tr("Erst einen Namen für das Layout eingeben.").into());
            return;
        }
        let sp = self.sprite();
        let l = GuideLayout { name: name.to_string(), width: sp.width, height: sp.height, guides: sp.guides.normalized(sp.width, sp.height) };
        let had = self.guides.layouts.iter().position(|o| o.name == name);
        match had {
            Some(i) => self.guides.layouts[i] = l,
            None => self.guides.layouts.push(l),
        }
        self.guides.layout_sel = had.unwrap_or(self.guides.layouts.len() - 1);
        self.store_guide_layouts();
        self.hint = Some(if had.is_some() {
            trf("Layout „{name}“ ersetzt.", &[("name", &name)])
        } else {
            trf("Layout „{name}“ gespeichert — gilt für alle Sprites.", &[("name", &name)])
        });
    }

    /// Gewähltes Layout auf den Sprite legen (anteilig umgerechnet).
    pub(crate) fn apply_guide_layout(&mut self, i: usize) {
        let Some(l) = self.guides.layouts.get(i).cloned() else { return };
        let (w, h) = (self.sprite().width, self.sprite().height);
        *self.guides_mut() = l.guides.scaled((l.width, l.height), (w, h));
        self.guides.show = true;
        self.dirty = true;
        self.hint = Some(if (l.width, l.height) == (w, h) {
            trf("Layout „{name}“ angewendet.", &[("name", &l.name)])
        } else {
            trf("Layout „{name}“ angewendet — von {w} × {h} auf diese Größe umgerechnet.", &[("name", &l.name), ("w", &l.width), ("h", &l.height)])
        });
    }

    fn guide_layouts_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(tr("Eigene Layouts"));
        let n = self.guides.layouts.len();
        if self.guides.layout_sel >= n {
            self.guides.layout_sel = n.saturating_sub(1);
        }
        ui.horizontal(|ui| {
            let label = |l: &GuideLayout| format!("{} ({} × {})", l.name, l.width, l.height);
            let text = self.guides.layouts.get(self.guides.layout_sel).map_or_else(|| tr("noch keine gespeichert").to_string(), label);
            ui.add_enabled_ui(n > 0, |ui| {
                egui::ComboBox::from_id_salt("gd-layouts").width(130.0).selected_text(text).show_ui(ui, |ui| {
                    for (i, l) in self.guides.layouts.iter().enumerate() {
                        ui.selectable_value(&mut self.guides.layout_sel, i, label(l));
                    }
                });
            });
            let tip = tr("Linien und Einteilung dieses Layouts auf den Sprite legen — bei anderer Größe anteilig umgerechnet");
            if ui.add_enabled(n > 0, egui::Button::new(tr("Anwenden"))).on_hover_text(tip).clicked() {
                self.apply_guide_layout(self.guides.layout_sel);
            }
            if ui.add_enabled(n > 0, egui::Button::new("×")).on_hover_text(tr("Gewähltes Layout löschen")).clicked() {
                let l = self.guides.layouts.remove(self.guides.layout_sel);
                self.store_guide_layouts();
                self.hint = Some(trf("Layout „{name}“ gelöscht.", &[("name", &l.name)]));
            }
        });
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.guides.layout_name).hint_text(tr("Name des Layouts")).desired_width(130.0).char_limit(40));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let tip = tr("Die jetzigen Linien und die Einteilung als Layout speichern — gleicher Name ersetzt");
            if ui.button(tr("Speichern")).on_hover_text(tip).clicked() || enter {
                let name = std::mem::take(&mut self.guides.layout_name);
                self.save_guide_layout(&name);
                if self.hint.as_deref() == Some(tr("Erst einen Namen für das Layout eingeben.")) {
                    self.guides.layout_name = name;
                }
            }
        });
    }

    /// G: ein/aus.
    pub(crate) fn guide_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::G) && i.modifiers.is_none()) {
            self.guides.show = !self.guides.show;
            self.guides.drag = None;
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
            // Kopfhöhen und Marken dazwischen: greifen die ganze Figur.
            let unit = (g.bottom - g.top) as f32 / g.heads as f32;
            let grab = py.round() as i64 - g.top as i64;
            let inner = (1..g.heads).map(|k| k as f32).chain(marks(g.heads).iter().map(|m| m.0).filter(|k| k.fract() != 0.0));
            for k in inner {
                take((py - (g.top as f32 + k * unit)).abs() * zoom, GuideHit::Figure(grab));
            }
        }
        best.map(|b| b.1)
    }

    /// Hand-Werkzeug auf einer Linie: die Linie ziehen (wie im Web). Gibt
    /// `true` zurück, wenn eine Linie den Zeiger genommen hat — daneben
    /// verschiebt die Hand die Ansicht.
    pub(crate) fn guide_pointer(&mut self, pointer: Option<Pos2>, pressed: bool, released: bool, over: bool, origin: Pos2, zoom: f32) -> bool {
        if self.guides.drag.is_none() {
            if self.tool != crate::tools_ui::Tool::Pan || !self.guides.show || self.guides.locked || !(pressed && over) {
                return false;
            }
            let Some(p) = pointer else { return false };
            // Gespeicherte Werte erst auf die Fläche begrenzen, dann treffen.
            let n = self.sprite().guides.normalized(self.sprite().width, self.sprite().height);
            self.project.sprite_mut().guides = n;
            self.guides.drag = self.guide_hit(p, origin, zoom);
            if self.guides.drag.is_none() {
                return false;
            }
        }
        let (w, h) = (self.sprite().width as i64, self.sprite().height as i64);
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
                GuideHit::Figure(grab) => {
                    let size = (g.bottom - g.top) as i64;
                    let top = (y - grab).clamp(0, (h - size).max(0));
                    g.top = top as u32;
                    g.bottom = (top + size) as u32;
                }
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
        let wide = if self.guides.drag.is_some() { 2.0 } else { 1.0 };
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
