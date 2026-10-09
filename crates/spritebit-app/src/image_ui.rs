//! Panels „Bild“ und „Aufräumen“ der rechten Leiste.
//!
//! Bild: spiegeln, 90° und frei drehen, zuschneiden, zentrieren, Leinwand,
//! skalieren. Mit Auswahl wirken Spiegeln und Drehen nur auf sie (ihr
//! Inhalt wird dafür angehoben), sonst auf den ganzen Sprite.
//!
//! Aufräumen: Hintergrund entfernen, glätten, Outline — auf die aktive Zelle.
//!
//! Licht: Lichtquelle aus 8 Richtungen, Kantenlicht und Schlagschatten — auf
//! die aktive Zelle, mit Auswahl nur darin.

use eframe::egui;
use spritebit_core::selection::{Clip, Selection};
use spritebit_core::transform::{self as tf, TransformResult};
use spritebit_core::light::{self, LightDir, LightOpts};
use spritebit_core::{cleanup, Image, Palette, Rgb};

use crate::i18n::{tr, trf};
use crate::SpritebitApp;

/// Laufende freie Drehung: das unberührte Original, jede Vorschau rechnet
/// von ihm aus (sonst wäre die Form nach dreimal Ziehen Matsch).
pub(crate) enum RotLive {
    Selection { clip: Clip, mask: Option<Vec<bool>>, x: i64, y: i64 },
    Sprite { image: Image },
}

/// Einstellungen der Panels.
pub(crate) struct ImagePanel {
    pub angle: f64,
    pub live: Option<RotLive>,
    pub resize_w: u32,
    pub resize_h: u32,
    /// Größe des Sprites, zu der die Felder zuletzt gepasst haben.
    seen: (u32, u32),
    pub centered: bool,
    pub bg_tolerance: f64,
    pub outline_color: Rgb,
    pub outline_thickness: u32,
    /// Woher das Licht kommt.
    pub light_dir: LightDir,
    pub light: LightOpts,
    pub cast_color: Rgb,
    pub cast_distance: u32,
}

impl Default for ImagePanel {
    fn default() -> Self {
        ImagePanel {
            angle: 0.0,
            live: None,
            resize_w: 0,
            resize_h: 0,
            seen: (0, 0),
            centered: true,
            bg_tolerance: 0.25,
            outline_color: [0x1a, 0x1a, 0x1a],
            outline_thickness: 1,
            light_dir: (-1, -1),
            light: LightOpts::default(),
            cast_color: [0x1a, 0x1a, 0x1a],
            cast_distance: 1,
        }
    }
}

impl SpritebitApp {
    fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    fn step(&mut self) {
        let cur = self.project.current;
        self.histories[cur].record(&self.project.sprites[cur]);
    }

    /// Für alles, was den ganzen Sprite umbaut: Schwebendes absetzen,
    /// Auswahl weg, Undo-Schritt.
    fn whole_sprite(&mut self) {
        self.finish_rotate();
        self.deselect();
        self.step();
    }

    pub(crate) fn flip(&mut self, horizontal: bool) {
        self.finish_rotate();
        if self.has_selection() {
            self.lift();
            let (Some(f), Some(s)) = (&mut self.float, &mut self.selection) else { return };
            let (w, h) = (f.clip.w, f.clip.h);
            f.clip.data = if horizontal { tf::flip_h(w, h, &f.clip.data) } else { tf::flip_v(w, h, &f.clip.data) };
            if let Some(m) = &s.mask {
                s.mask = Some(if horizontal { tf::flip_h(w, h, m) } else { tf::flip_v(w, h, m) });
            }
        } else {
            self.step();
            tf::flip_sprite(self.project.sprite_mut(), horizontal);
        }
        self.changed();
    }

    pub(crate) fn rotate90(&mut self) {
        self.finish_rotate();
        if self.has_selection() {
            self.lift();
            let (sw, sh) = (self.sprite().width as i64, self.sprite().height as i64);
            let (Some(f), Some(s)) = (&mut self.float, &mut self.selection) else { return };
            let (w, h) = (f.clip.w, f.clip.h);
            // Um die Mitte drehen, damit die Auswahl nicht wegspringt.
            let (nw, nh) = (h, w);
            let nx = (f.x as f64 + (w as f64 - nw as f64) / 2.0).round() as i64;
            let ny = (f.y as f64 + (h as f64 - nh as f64) / 2.0).round() as i64;
            let (nx, ny) = (nx.clamp(1 - nw as i64, sw - 1), ny.clamp(1 - nh as i64, sh - 1));
            f.clip = Clip { w: nw, h: nh, data: tf::rot90(w, h, &f.clip.data) };
            f.x = nx;
            f.y = ny;
            *s = Selection { x: nx, y: ny, w: nw, h: nh, mask: s.mask.as_ref().map(|m| tf::rot90(w, h, m)) };
        } else {
            self.whole_sprite();
            tf::rotate_sprite90(self.project.sprite_mut());
            self.fit_pending = true;
        }
        self.changed();
    }

    /// Freie Drehung auf `angle` setzen (Vorschau; Original bleibt gemerkt).
    fn preview_rotate(&mut self, angle: f64) {
        if self.image.live.is_none() {
            if self.has_selection() {
                self.lift();
                let (Some(f), Some(s)) = (&self.float, &self.selection) else { return };
                self.image.live = Some(RotLive::Selection { clip: f.clip.clone(), mask: s.mask.clone(), x: f.x, y: f.y });
            } else {
                if !self.layer_ok() {
                    return;
                }
                self.step();
                let image = self.project.sprite_mut().active().clone();
                self.image.live = Some(RotLive::Sprite { image });
            }
        }
        let (sw, sh) = (self.sprite().width as i64, self.sprite().height as i64);
        match &self.image.live {
            Some(RotLive::Selection { clip, mask, x, y }) => {
                let r = tf::rotate(clip.w, clip.h, &clip.data, mask.as_deref(), angle, true);
                let cx = *x as f64 + clip.w as f64 / 2.0;
                let cy = *y as f64 + clip.h as f64 / 2.0;
                let nx = ((cx - r.w as f64 / 2.0).round() as i64).clamp(1 - r.w as i64, sw - 1);
                let ny = ((cy - r.h as f64 / 2.0).round() as i64).clamp(1 - r.h as i64, sh - 1);
                if let Some(f) = &mut self.float {
                    f.clip = Clip { w: r.w, h: r.h, data: r.cells };
                    f.x = nx;
                    f.y = ny;
                }
                self.selection = Some(Selection { x: nx, y: ny, w: r.w, h: r.h, mask: Some(r.mask) });
            }
            Some(RotLive::Sprite { image }) => {
                let rotated = tf::image_rotate(image, angle);
                *self.project.sprite_mut().active() = rotated;
            }
            None => {}
        }
        self.changed();
    }

    /// Winkel festschreiben (bei einer Auswahl schwebt das Ergebnis weiter).
    pub(crate) fn finish_rotate(&mut self) {
        self.image.live = None;
        self.image.angle = 0.0;
    }

    /// Zurück aufs Original.
    pub(crate) fn cancel_rotate(&mut self) {
        match self.image.live.take() {
            Some(RotLive::Selection { clip, mask, x, y }) => {
                if let Some(f) = &mut self.float {
                    *f = crate::selection_ui::Float { clip: clip.clone(), x, y };
                }
                self.selection = Some(Selection { x, y, w: clip.w, h: clip.h, mask });
            }
            Some(RotLive::Sprite { image }) => {
                *self.project.sprite_mut().active() = image;
                // Bild wieder wie vorher: der Undo-Schritt ist überflüssig.
                let cur = self.project.current;
                self.histories[cur].drop_last();
            }
            None => {}
        }
        self.image.angle = 0.0;
        self.changed();
    }

    fn report(&mut self, r: TransformResult, nothing: &'static str) {
        self.hint = Some(match r {
            TransformResult::Done { w, h, lost: 0 } => trf("Jetzt {w} × {h} px.", &[("w", &w), ("h", &h)]),
            TransformResult::Done { w, h, lost } => {
                trf("Jetzt {w} × {h} px — {n} Pixel lagen außerhalb.", &[("w", &w), ("h", &h), ("n", &lost)])
            }
            TransformResult::Nothing => tr(nothing).to_string(),
            TransformResult::TooBig => tr("Zu groß — höchstens 8192 × 8192.").to_string(),
            TransformResult::TooSmall => tr("Zu klein — mindestens 1 × 1.").to_string(),
        });
    }

    /// Ganzer-Sprite-Umbau mit Undo; ohne Wirkung wird der Schritt verworfen.
    fn rebuild(&mut self, nothing: &'static str, f: impl FnOnce(&mut spritebit_core::Sprite) -> TransformResult) {
        self.whole_sprite();
        let r = f(self.project.sprite_mut());
        if matches!(r, TransformResult::Done { .. }) {
            self.fit_pending = true;
            self.changed();
        } else {
            let cur = self.project.current;
            self.histories[cur].drop_last();
        }
        self.report(r, nothing);
    }

    /// Die rechte Leiste: aufklappbare Panels wie in der Web-Version.
    pub(crate) fn right_panels(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(tr("Vorschau")).id_salt("p-preview").default_open(true).show(ui, |ui| self.preview_panel(ui));
        egui::CollapsingHeader::new(tr("Palette")).id_salt("p-palette").default_open(true).show(ui, |ui| self.palette_library(ui));
        egui::CollapsingHeader::new(tr("Bild")).id_salt("p-image").default_open(true).show(ui, |ui| self.image_panel(ui));
        egui::CollapsingHeader::new(tr("Aufräumen")).id_salt("p-cleanup").show(ui, |ui| self.cleanup_panel(ui));
        egui::CollapsingHeader::new(tr("Licht")).id_salt("p-light").show(ui, |ui| self.light_panel(ui));
        egui::CollapsingHeader::new(tr("Hilfslinien")).id_salt("p-guides").show(ui, |ui| self.guides_panel(ui));
        egui::CollapsingHeader::new(tr("Schablone")).id_salt("p-template").show(ui, |ui| self.template_panel(ui));
        // „Exportieren …“ im Menü klappt dieses Panel auf und scrollt hin.
        let focus = std::mem::take(&mut self.out.focus);
        let r = egui::CollapsingHeader::new(tr("Code & Export"))
            .id_salt("p-output")
            .open(focus.then_some(true))
            .show(ui, |ui| self.output_panel(ui));
        if focus {
            r.header_response.scroll_to_me(Some(egui::Align::TOP));
        }
    }

    pub(crate) fn image_panel(&mut self, ui: &mut egui::Ui) {
        ui.weak(if self.has_selection() { tr("Wirkt auf: Auswahl") } else { tr("Wirkt auf: Sprite") });
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("↔ Spiegeln")).on_hover_text(tr("Waagerecht spiegeln")).clicked() {
                self.flip(true);
            }
            if ui.button(tr("↕ Spiegeln")).on_hover_text(tr("Senkrecht spiegeln")).clicked() {
                self.flip(false);
            }
            if ui.button("↻ 90°").on_hover_text(tr("90° im Uhrzeigersinn drehen")).clicked() {
                self.rotate90();
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr("Frei drehen"));
            let mut a = self.image.angle;
            let r = ui.add(egui::Slider::new(&mut a, -180.0..=180.0).step_by(1.0).suffix("°"));
            if r.changed() && a != self.image.angle {
                self.image.angle = a;
                self.preview_rotate(a);
            }
        });
        if self.image.live.is_some() {
            ui.horizontal(|ui| {
                if ui.button(tr("Übernehmen")).clicked() {
                    self.finish_rotate();
                }
                if ui.button(tr("Verwerfen")).clicked() {
                    self.cancel_rotate();
                }
            });
        }
        ui.horizontal(|ui| {
            if ui.button(tr("Zuschneiden")).on_hover_text(tr("Leeren Rand rundherum abschneiden")).clicked() {
                self.rebuild("Nichts abzuschneiden.", tf::trim);
            }
            if ui.button(tr("Zentrieren")).on_hover_text(tr("Inhalt mittig setzen")).clicked() {
                self.rebuild("Schon mittig oder leer.", tf::center);
            }
        });
        ui.separator();
        let size = (self.sprite().width, self.sprite().height);
        if self.image.seen != size {
            self.image.seen = size;
            (self.image.resize_w, self.image.resize_h) = size;
        }
        ui.label(tr("Leinwand"));
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.image.resize_w).range(1..=spritebit_core::MAX_SIDE));
            ui.label("×");
            ui.add(egui::DragValue::new(&mut self.image.resize_h).range(1..=spritebit_core::MAX_SIDE));
            ui.label("px");
        });
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("anchor")
                .selected_text(if self.image.centered { tr("mittig") } else { tr("oben links") })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.image.centered, true, tr("mittig"));
                    ui.selectable_value(&mut self.image.centered, false, tr("oben links"));
                });
            if ui.button(tr("Anwenden")).clicked() {
                let (w, h, c) = (self.image.resize_w, self.image.resize_h, self.image.centered);
                self.rebuild("Größe unverändert.", |s| tf::resize_canvas(s, w, h, c));
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr("Skalieren"));
            if ui.button("×2").on_hover_text(tr("Auf das Doppelte vergrößern (Pixel bleiben hart)")).clicked() {
                self.rebuild("", |s| tf::scale(s, 2.0));
            }
            if ui.button("÷2").on_hover_text(tr("Auf die Hälfte verkleinern — Details gehen verloren")).clicked() {
                self.rebuild("", |s| tf::scale(s, 0.5));
            }
        });
    }

    /// Ein Aufräum-Schritt auf der aktiven Zelle.
    fn clean(&mut self, f: impl FnOnce(&mut Image, &spritebit_core::Palette, &[Rgb]) -> usize) -> Option<usize> {
        self.finish_rotate();
        self.deselect();
        if !self.layer_ok() {
            return None;
        }
        self.step();
        let pal = self.project.current_palette();
        let sp = self.project.sprite_mut();
        let free = sp.free.clone();
        let n = f(sp.active(), &pal, &free);
        if n > 0 {
            self.changed();
        } else {
            let cur = self.project.current;
            self.histories[cur].drop_last();
        }
        Some(n)
    }

    pub(crate) fn cleanup_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(tr("Hintergrund"));
            ui.add(egui::Slider::new(&mut self.image.bg_tolerance, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)))
                .on_hover_text(tr("Toleranz: wie ähnlich die Farben vom Rand her sein dürfen"));
        });
        if ui.button(tr("Hintergrund entfernen")).clicked() {
            let tol = self.image.bg_tolerance;
            if let Some(n) = self.clean(|img, pal, free| cleanup::remove_background(img, pal, free, tol)) {
                self.hint = Some(if n > 0 { trf("Hintergrund entfernt — {n} Pixel.", &[("n", &n)]) } else { tr("Nichts entfernt — Toleranz erhöhen?").into() });
            }
        }
        if ui.button(tr("Glätten")).on_hover_text(tr("Einzelne Streupixel auf die Farbe ihrer Nachbarn setzen")).clicked() {
            if let Some(n) = self.clean(|img, _, _| cleanup::despeckle(img)) {
                self.hint = Some(if n > 0 { trf("Geglättet — {n} Pixel angepasst.", &[("n", &n)]) } else { tr("Nichts zu glätten gefunden.").into() });
            }
        }
        ui.horizontal(|ui| {
            ui.label(tr("Outline"));
            egui::color_picker::color_edit_button_srgb(ui, &mut self.image.outline_color).on_hover_text(tr("Outline-Farbe"));
            egui::ComboBox::from_id_salt("outline-thick")
                .width(50.0)
                .selected_text(format!("{} px", self.image.outline_thickness))
                .show_ui(ui, |ui| {
                    for t in 1..=3 {
                        ui.selectable_value(&mut self.image.outline_thickness, t, format!("{t} px"));
                    }
                });
            if ui.button(tr("Anwenden")).clicked() {
                let th = self.image.outline_thickness;
                // Farbe wie beim Farbwähler: Palettennummer, sonst freie Farbe.
                let keep = self.color;
                self.set_rgb(self.image.outline_color);
                let value = std::mem::replace(&mut self.color, keep);
                if let Some(n) = self.clean(|img, _, _| cleanup::outline(img, value, th)) {
                    self.hint = Some(if n > 0 { trf("Outline gezeichnet — {n} Pixel.", &[("n", &n)]) } else { tr("Keine Outline nötig — Sprite leer?").into() });
                }
            }
        });
    }

    /// Ein Licht-Schritt auf der aktiven Zelle. Anders als [`Self::clean`]
    /// bleibt die Auswahl — sie begrenzt, wo das Licht wirkt.
    fn light_step(&mut self, f: impl FnOnce(&mut Image, &Palette, &mut Vec<Rgb>, &dyn Fn(i64, i64) -> bool) -> usize) -> Option<usize> {
        self.finish_rotate();
        self.commit_float();
        if !self.layer_ok() {
            return None;
        }
        self.step();
        let pal = self.project.current_palette();
        let sel = self.selection.clone();
        let inside = move |x: i64, y: i64| sel.as_ref().is_none_or(|s| s.contains(x, y));
        let sp = self.project.sprite_mut();
        let mut free = std::mem::take(&mut sp.free);
        let n = f(sp.active(), &pal, &mut free, &inside);
        sp.free = free;
        if n > 0 {
            self.changed();
        } else {
            let cur = self.project.current;
            self.histories[cur].drop_last();
        }
        Some(n)
    }

    pub(crate) fn apply_light(&mut self) {
        let (dir, opts) = (self.image.light_dir, self.image.light);
        let mut r = (0, 0);
        let done = self.light_step(|img, pal, free, inside| {
            r = light::light(img, pal, free, dir, opts, inside);
            r.0 + r.1
        });
        if let Some(n) = done {
            self.hint = Some(if n > 0 {
                trf("Licht gesetzt — {lit} Pixel heller, {shaded} dunkler.", &[("lit", &r.0), ("shaded", &r.1)])
            } else {
                tr("Nichts beleuchtet — keine Kanten oder keine passenden Palettenfarben.").into()
            });
        }
    }

    pub(crate) fn apply_drop_shadow(&mut self) {
        let (dir, dist) = (self.image.light_dir, self.image.cast_distance);
        // Farbe wie beim Farbwähler: Palettennummer, sonst freie Farbe.
        let keep = self.color;
        self.set_rgb(self.image.cast_color);
        let value = std::mem::replace(&mut self.color, keep);
        if let Some(n) = self.light_step(|img, _, _, inside| light::drop_shadow(img, dir, value, dist, inside)) {
            self.hint = Some(if n > 0 {
                trf("Schlagschatten gemalt — {n} Pixel.", &[("n", &n)])
            } else {
                tr("Kein Platz für einen Schatten — Sprite leer oder Rand erreicht?").into()
            });
        }
    }

    pub(crate) fn light_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(tr("Lichtquelle"));
        const DIRS: [[(i32, i32, &str, &str); 3]; 3] = [
            [(-1, -1, "↖", "Licht von oben links"), (0, -1, "↑", "Licht von oben"), (1, -1, "↗", "Licht von oben rechts")],
            [(-1, 0, "←", "Licht von links"), (0, 0, "☀", ""), (1, 0, "→", "Licht von rechts")],
            [(-1, 1, "↙", "Licht von unten links"), (0, 1, "↓", "Licht von unten"), (1, 1, "↘", "Licht von unten rechts")],
        ];
        egui::Grid::new("light-dirs").spacing([2.0, 2.0]).show(ui, |ui| {
            for row in DIRS {
                for (dx, dy, arrow, tip) in row {
                    if dx == 0 && dy == 0 {
                        let sun = egui::RichText::new(arrow).color(ui.visuals().selection.bg_fill);
                        ui.add_sized([28.0, 24.0], egui::Label::new(sun));
                        continue;
                    }
                    let on = self.image.light_dir == (dx, dy);
                    if ui.add_sized([28.0, 24.0], egui::Button::selectable(on, arrow)).on_hover_text(tr(tip)).clicked() {
                        self.image.light_dir = (dx, dy);
                    }
                }
                ui.end_row();
            }
        });
        let o = &mut self.image.light;
        ui.horizontal(|ui| {
            ui.label(tr("Stärke"));
            ui.add(egui::Slider::new(&mut o.amount, 0.05..=0.4).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
        });
        ui.horizontal(|ui| {
            ui.label(tr("Breite"));
            egui::ComboBox::from_id_salt("light-width")
                .width(50.0)
                .selected_text(format!("{} px", o.width))
                .show_ui(ui, |ui| {
                    for w in 1..=3 {
                        ui.selectable_value(&mut o.width, w, format!("{w} px"));
                    }
                })
                .response
                .on_hover_text(tr("Wie viele Pixel vom Rand her beleuchtet bzw. schattiert werden"));
        });
        ui.checkbox(&mut o.highlight, tr("Lichtkante (heller)"));
        ui.checkbox(&mut o.shadow, tr("Schattenkante (dunkler)"));
        ui.checkbox(&mut o.allow_free, tr("Auch Farben außerhalb der Palette")).on_hover_text(tr(
            "Fehlt in der Palette eine passende hellere oder dunklere Farbe, wird eine freie Farbe berechnet — sonst bleibt der Pixel, wie er ist",
        ));
        if ui
            .button(tr("Licht anwenden"))
            .on_hover_text(tr("Kanten zur Lichtquelle hin aufhellen, abgewandte Kanten abdunkeln — auf der aktiven Zelle, mit Auswahl nur darin"))
            .clicked()
        {
            self.apply_light();
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(tr("Schlagschatten"));
            egui::color_picker::color_edit_button_srgb(ui, &mut self.image.cast_color).on_hover_text(tr("Schattenfarbe"));
            egui::ComboBox::from_id_salt("cast-dist")
                .width(50.0)
                .selected_text(format!("{} px", self.image.cast_distance))
                .show_ui(ui, |ui| {
                    for d in 1..=3 {
                        ui.selectable_value(&mut self.image.cast_distance, d, format!("{d} px"));
                    }
                })
                .response
                .on_hover_text(tr("Wie weit der Schatten fällt"));
            if ui
                .button(tr("Werfen"))
                .on_hover_text(tr("Silhouette von der Lichtquelle weg versetzt als Schatten in leere Pixel malen"))
                .clicked()
            {
                self.apply_drop_shadow();
            }
        });
    }

    #[cfg(test)]
    pub(crate) fn clean_for_test_outline(&mut self) -> usize {
        self.clean(|img, _, _| cleanup::outline(img, 1, 1)).unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn test_rotate(&mut self, angle: f64) {
        self.image.angle = angle;
        self.preview_rotate(angle);
    }
}
