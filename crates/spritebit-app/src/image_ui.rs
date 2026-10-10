//! Panels „Bild“ und „Aufräumen“ der rechten Leiste.
//!
//! Bild: spiegeln, 90° und frei drehen, zuschneiden, zentrieren, Leinwand,
//! skalieren. Mit Auswahl wirken Spiegeln und Drehen nur auf sie (ihr
//! Inhalt wird dafür angehoben), sonst auf den ganzen Sprite.
//!
//! Aufräumen: Hintergrund entfernen, glätten, Outline — auf die aktive Zelle.
//!
//! Licht: Lichtquelle aus 8 Richtungen, Kantenlicht und Schlagschatten —
//! nicht-destruktiv als eigene Ebenen über bzw. unter der Figur
//! (spritebit_core::light), für alle Frames; jede Änderung rechnet sie neu.

use eframe::egui;
use spritebit_core::selection::{Clip, Selection};
use spritebit_core::transform::{self as tf, TransformResult};
use spritebit_core::light::{self, FxColor, FxKind, LayerFx, LightDir, LightOpts};
use spritebit_core::{cleanup, Image, Rgb};

use crate::i18n::{tr, trf};
use crate::icons;
use crate::SpritebitApp;

/// Welche Panels der rechten Leiste offen sind — überlebt den Neustart.
/// Kleine Textdatei im Einstellungsordner („p-light=1“ je Zeile), wie die
/// Sprache; in Tests nie (die liefen sonst gegen die echten Einstellungen).
#[derive(Default)]
pub(crate) struct PanelMemory {
    pub(crate) open: std::collections::HashMap<String, bool>,
    /// Der gespeicherte Stand ist gesetzt (nur im ersten Durchlauf).
    pub(crate) applied: bool,
}

impl PanelMemory {
    #[cfg(not(test))]
    fn file() -> Option<std::path::PathBuf> {
        Some(crate::i18n::settings_dir()?.join("panels"))
    }

    pub(crate) fn load() -> Self {
        #[allow(unused_mut)]
        let mut m = Self::default();
        #[cfg(not(test))]
        if let Some(text) = Self::file().and_then(|p| std::fs::read_to_string(p).ok()) {
            for line in text.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    m.open.insert(k.trim().to_string(), v.trim() == "1");
                }
            }
        }
        m
    }

    fn save(&self) {
        #[cfg(not(test))]
        if let Some(p) = Self::file() {
            let mut keys: Vec<_> = self.open.iter().collect();
            keys.sort();
            let out: String = keys.into_iter().map(|(k, v)| format!("{k}={}\n", if *v { 1 } else { 0 })).collect();
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, out);
        }
    }
}

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
    /// „An der Figur wurde weitergemalt“ — zuletzt geprüft für (Sprite, Version).
    pub stale: bool,
    pub stale_key: Option<(usize, u64)>,
    /// Schlagschatten mit anlegen bzw. in der Vorschau zeigen.
    pub cast_on: bool,
    /// Das Licht-Panel wurde in diesem Durchlauf gezeichnet (für die Vorschau).
    pub light_open: bool,
    /// Licht-Vorschau: Schlüssel der Einstellungen und die Kopie, die die
    /// Zeichenfläche statt des Sprites zeigt. `preview_gen` zählt jede
    /// Änderung, damit die Textur neu gebaut wird.
    pub preview: Option<(String, spritebit_core::Sprite)>,
    pub preview_gen: u64,
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
            stale: false,
            stale_key: None,
            cast_on: false,
            light_open: false,
            preview: None,
            preview_gen: 0,
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

    /// Wie die Panels stehen (für Tests).
    #[cfg(test)]
    pub(crate) fn panel_open(&self, id: &str) -> Option<bool> {
        self.panels.open.get(id).copied()
    }

    /// Ein aufklappbares Panel mit Icon, das sich merkt, ob es offen ist —
    /// beim nächsten Start der App steht es wieder so da (PanelMemory).
    /// `extra`: Knöpfe rechts in der Kopfzeile (dock_ui.rs panel_header).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn panel_with(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        icon: egui::ImageSource<'static>,
        id: &'static str,
        default: bool,
        force: Option<bool>,
        extra: impl FnOnce(&mut Self, &mut egui::Ui),
        body: impl FnOnce(&mut Self, &mut egui::Ui),
    ) -> egui::Response {
        use egui::collapsing_header::CollapsingState;
        // An den Schlüssel gebunden, nicht an die Spalte — so bleibt der
        // Zustand, wenn das Panel die Seite wechselt.
        let cid = egui::Id::new(("panel", id));
        let mut state = CollapsingState::load_with_default_open(ui.ctx(), cid, default);
        // Nur im ersten Durchlauf den gespeicherten Zustand setzen — danach
        // gehört das Auf- und Zuklappen wieder dem Nutzer. Aufgezwungen
        // (Menü „Exportieren …“) zählt als offen.
        if !self.panels.applied {
            state.set_open(self.panels.open.get(id).copied().unwrap_or(default));
        }
        if let Some(open) = force {
            state.set_open(open);
        }
        let color = ui.visuals().text_color();
        let header = state.show_header(ui, |ui| {
            ui.add(icons::image(icon, color));
            let name = ui.add(egui::Label::new(egui::RichText::new(title).strong()).selectable(false).sense(egui::Sense::click()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| extra(self, ui));
            name
        });
        let (_, head, _) = header.body(|ui| body(self, ui));
        // Auch ein Klick auf den Namen klappt auf und zu, nicht nur der Pfeil.
        let mut state = CollapsingState::load_with_default_open(ui.ctx(), cid, default);
        if head.inner.clicked() {
            state.toggle(ui);
            state.store(ui.ctx());
        }
        let open = state.is_open();
        if self.panels.open.get(id).copied().unwrap_or(default) != open {
            self.panels.open.insert(id.to_string(), open);
            self.panels.save();
        }
        head.response
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
            ui.add(egui::DragValue::new(&mut self.image.resize_w).range(1..=spritebit_core::MAX_SIDE).custom_parser(spritebit_core::calc::eval));
            ui.label("×");
            ui.add(egui::DragValue::new(&mut self.image.resize_h).range(1..=spritebit_core::MAX_SIDE).custom_parser(spritebit_core::calc::eval));
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

    /// Ebene, auf die das Licht wirkt: die aktive — oder, wenn die aktive
    /// selbst eine Effekt-Ebene ist, ihre Figur.
    fn light_base(&self) -> Option<usize> {
        let sp = self.sprite();
        if sp.layers[sp.layer].fx.is_some() {
            light::fx_source(&sp.layers, sp.layer)
        } else {
            Some(sp.layer)
        }
    }

    fn panel_fx(&self, is_light: bool) -> LayerFx {
        let kind = if is_light {
            FxKind::Light(self.image.light)
        } else {
            FxKind::Shadow { color: FxColor::Rgb(self.image.cast_color), distance: self.image.cast_distance }
        };
        LayerFx { kind, dir: self.image.light_dir, src: String::new() }
    }

    /// Licht- bzw. Schatten-Ebene zur Figur anlegen oder mit den Werten des
    /// Panels neu berechnen. `create = false`: nur, wenn es sie schon gibt.
    pub(crate) fn upsert_light_layer(&mut self, is_light: bool, create: bool) {
        self.finish_rotate();
        let Some(base) = self.light_base() else {
            self.hint = Some(tr("Keine Ebene, auf die das Licht wirken kann.").into());
            return;
        };
        if !create && light::fx_for(&self.sprite().layers, base, is_light).is_none() {
            return;
        }
        let fx = self.panel_fx(is_light);
        let figure = self.sprite().layers[base].name.clone();
        let name = trf(if is_light { "Licht · {name}" } else { "Schatten · {name}" }, &[("name", &figure)]);
        let pal = self.project.current_palette();
        self.edit_sprite(|s| {
            light::upsert_fx(s, base, fx, name, &pal);
        });
    }

    /// Licht und Schatten der Figur neu berechnen (nach Änderungen an ihr).
    fn recompute_light_layers(&mut self) {
        let Some(base) = self.light_base() else { return };
        let layers = &self.sprite().layers;
        let idx: Vec<usize> = [true, false].iter().filter_map(|&l| light::fx_for(layers, base, l)).collect();
        if idx.is_empty() {
            return;
        }
        let pal = self.project.current_palette();
        self.edit_sprite(|s| {
            for &i in &idx {
                light::recompute_fx(s, i, &pal);
            }
        });
    }

    /// Licht (und, wenn angehakt, Schatten) als Ebenen anlegen — ein Undo-Schritt.
    pub(crate) fn commit_light_layers(&mut self) {
        self.finish_rotate();
        let Some(base) = self.light_base() else {
            self.hint = Some(tr("Keine Ebene, auf die das Licht wirken kann.").into());
            return;
        };
        let (lfx, sfx) = (self.panel_fx(true), self.image.cast_on.then(|| self.panel_fx(false)));
        let figure = self.sprite().layers[base].name.clone();
        let (ln, sn) = (trf("Licht · {name}", &[("name", &figure)]), trf("Schatten · {name}", &[("name", &figure)]));
        let pal = self.project.current_palette();
        self.edit_sprite(|s| {
            let mut b = base;
            if let Some(fx) = sfx {
                light::upsert_fx(s, b, fx, sn, &pal);
                b += 1;
            }
            light::upsert_fx(s, b, lfx, ln, &pal);
        });
    }

    /// Vorschau aktualisieren (vor dem Zeichnen der Fläche). Ohne offenes
    /// Panel, mit schwebender Auswahl oder wenn schon alles als Ebene da ist:
    /// keine Vorschau.
    pub(crate) fn update_light_preview(&mut self) {
        let want = if self.image.light_open && self.float.is_none() { self.light_preview_plan() } else { None };
        self.image.light_open = false;
        match want {
            None => {
                if self.image.preview.take().is_some() {
                    self.image.preview_gen += 1;
                }
            }
            Some((key, base, lfx, sfx)) => {
                if self.image.preview.as_ref().map(|p| &p.0) != Some(&key) {
                    let pal = self.project.current_palette();
                    let sp = self.sprite();
                    let pv = light::preview_fx(sp, base, sp.frame, lfx, sfx, &pal);
                    self.image.preview = Some((key, pv));
                    self.image.preview_gen += 1;
                }
            }
        }
    }

    fn light_preview_plan(&self) -> Option<(String, usize, Option<LayerFx>, Option<LayerFx>)> {
        let base = self.light_base()?;
        let layers = &self.sprite().layers;
        let lfx = light::fx_for(layers, base, true).is_none().then(|| self.panel_fx(true));
        let sfx = (self.image.cast_on && light::fx_for(layers, base, false).is_none()).then(|| self.panel_fx(false));
        if lfx.is_none() && sfx.is_none() {
            return None;
        }
        let key = format!("{}|{}|{}|{base}|{lfx:?}|{sfx:?}", self.project.current, self.version, self.sprite().frame);
        Some((key, base, lfx, sfx))
    }

    pub(crate) fn light_panel(&mut self, ui: &mut egui::Ui) {
        // Gibt es zur Figur schon Licht bzw. Schatten, zeigt das Panel deren
        // Einstellungen — und jede Änderung rechnet die Ebene neu.
        let base = self.light_base();
        let (li, si) = {
            let layers = &self.sprite().layers;
            (base.and_then(|b| light::fx_for(layers, b, true)), base.and_then(|b| light::fx_for(layers, b, false)))
        };
        let pal = self.project.current_palette();
        if let Some(fx) = li.and_then(|i| self.sprite().layers[i].fx.clone()) {
            if let FxKind::Light(o) = fx.kind {
                self.image.light = o;
            }
            self.image.light_dir = fx.dir;
        }
        if let Some(fx) = si.and_then(|i| self.sprite().layers[i].fx.clone()) {
            if let FxKind::Shadow { color, distance } = fx.kind {
                self.image.cast_color = match color {
                    FxColor::Rgb(c) => c,
                    FxColor::Index(i) => pal.get(i).unwrap_or([0x1a; 3]),
                };
                self.image.cast_distance = distance;
            }
            if li.is_none() {
                self.image.light_dir = fx.dir;
            }
        }
        if li.is_some() || si.is_some() {
            self.image.cast_on = si.is_some();
        }
        self.image.light_open = true;
        let (mut light_changed, mut shadow_changed) = (false, false);

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
                    // Gedrehtes Pfeil-Icon statt Zeichen — die eingebaute
                    // Schrift hat ↑ ← → ↓ nicht (sie erschienen als Kästchen).
                    let angle = (dx as f32).atan2(-(dy as f32));
                    let c = if on { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                    let img = crate::icons::image(crate::icons::UP, c).rotate(angle, egui::Vec2::splat(0.5)).alt_text(tr(tip));
                    if ui.add_sized([28.0, 24.0], egui::Button::selectable(on, img)).on_hover_text(tr(tip)).clicked() && !on {
                        self.image.light_dir = (dx, dy);
                        light_changed = true;
                        shadow_changed = true;
                    }
                }
                ui.end_row();
            }
        });
        // Beim Ziehen erst beim Loslassen neu rechnen — sonst wäre jedes
        // Zwischenbild ein eigener Undo-Schritt.
        let done = |r: &egui::Response| r.drag_stopped() || (r.changed() && !r.dragged());
        let o = &mut self.image.light;
        ui.horizontal(|ui| {
            ui.label(tr("Stärke"));
            let r = ui.add(egui::Slider::new(&mut o.amount, 0.05..=0.4).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
            light_changed |= done(&r);
        });
        ui.horizontal(|ui| {
            ui.label(tr("Breite"));
            let before = o.width;
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
            light_changed |= o.width != before;
        });
        light_changed |= ui.checkbox(&mut o.highlight, tr("Lichtkante (heller)")).changed();
        light_changed |= ui.checkbox(&mut o.shadow, tr("Schattenkante (dunkler)")).changed();
        light_changed |= ui
            .checkbox(&mut o.allow_free, tr("Auch Farben außerhalb der Palette"))
            .on_hover_text(tr("Fehlt in der Palette eine passende hellere oder dunklere Farbe, wird eine freie Farbe berechnet — sonst bleibt der Pixel, wie er ist"))
            .changed();
        ui.separator();
        let cast_before = self.image.cast_on;
        ui.checkbox(&mut self.image.cast_on, tr("Schlagschatten"));
        ui.horizontal(|ui| {
            ui.add_enabled_ui(self.image.cast_on, |ui| {
                // Farbe: live nur in der Vorschau — als Ebene mit „Neu werfen“
                // (sonst wäre jede Bewegung im Farbwähler ein Undo-Schritt).
                egui::color_picker::color_edit_button_srgb(ui, &mut self.image.cast_color).on_hover_text(tr("Schattenfarbe"));
                let dist = self.image.cast_distance;
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
                shadow_changed |= self.image.cast_distance != dist;
            });
            if si.is_some() && ui.button(tr("Neu werfen")).on_hover_text(tr("Schattenfarbe übernehmen und den Schatten neu berechnen")).clicked() {
                shadow_changed = true;
            }
        });
        // Häkchen umgelegt, während es schon Ebenen gibt: Schatten-Ebene dazu bzw. weg.
        if self.image.cast_on != cast_before && (li.is_some() || si.is_some()) {
            if let Some(base) = base {
                if self.image.cast_on {
                    self.upsert_light_layer(false, true);
                } else {
                    self.edit_sprite(|s| {
                        light::remove_fx(s, base, false);
                    });
                }
            }
            shadow_changed = false;
        }
        if li.is_none() {
            ui.label(egui::RichText::new(tr("Vorschau — im Bild ist noch nichts verändert.")).weak().italics());
            if ui
                .button(tr("Als Ebene übernehmen"))
                .on_hover_text(tr("Legt Licht (und, wenn angehakt, Schatten) als eigene Ebenen an — das Original bleibt unverändert. Danach rechnet jede Änderung hier die Ebenen sofort neu"))
                .clicked()
            {
                self.commit_light_layers();
                light_changed = false;
                shadow_changed = false;
            }
        }
        if light_changed {
            self.upsert_light_layer(true, false);
        }
        if shadow_changed {
            self.upsert_light_layer(false, false);
        }

        // Hinweis, wenn an der Figur weitergemalt wurde — nur neu prüfen,
        // wenn sich am Sprite etwas geändert hat (die Prüfsumme läuft über
        // alle Frames).
        if let (Some(b), true) = (self.light_base(), li.is_some() || si.is_some()) {
            let name = self.sprite().layers[b].name.clone();
            ui.weak(trf("Licht für „{name}“ ist eine eigene Ebene — Änderungen hier rechnen sie neu.", &[("name", &name)]));
        }
        let key = (self.project.current, self.version);
        if self.image.stale_key != Some(key) {
            let sp = self.sprite();
            let stale = [li, si].into_iter().flatten().any(|i| light::fx_stale(sp, i));
            self.image.stale = stale;
            self.image.stale_key = Some(key);
        }
        if self.image.stale {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(ui.visuals().warn_fg_color, tr("An der Figur wurde weitergemalt."));
                if ui.button(tr("Neu berechnen")).clicked() {
                    self.recompute_light_layers();
                }
            });
        }
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
