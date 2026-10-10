//! Timeline-Einstellungen (Knopf ⚙ in der Timeline) — wie `tlmenu.js` der
//! Web-Version: Lage der Timeline, Zählung und Vorschaubilder der
//! Kopfzeile, Dauer des aktuellen Frames und alles zum Onion Skin.
//!
//! Die Einstellungen gelten für alle Sprites und werden im
//! Einstellungsordner gemerkt (nicht im Projekt). „Zurücksetzen“ gilt dem
//! Onion Skin — Kopfzeile und Lage bleiben.

use std::collections::HashMap;

use eframe::egui::{self, Color32};
use spritebit_core::onion::{onion_frames, OnionOpts, ONION_MAX};
use spritebit_core::{render_rgba_step, Rect, Sprite};

use crate::i18n::tr;
use crate::SpritebitApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Zone {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TlOpts {
    /// Zählung ab 0 oder 1.
    pub first_frame: usize,
    /// Vorschaubilder in der Kopfzeile.
    pub thumbs: bool,
    /// Kantenlänge der Vorschaubilder — die Trennlinie darunter ziehen.
    pub thumb_size: f32,
    pub zone: Zone,
    pub onion: OnionOpts,
}

impl Default for TlOpts {
    fn default() -> Self {
        TlOpts { first_frame: 1, thumbs: true, thumb_size: THUMB_DEFAULT, zone: Zone::Bottom, onion: OnionOpts::default() }
    }
}

impl TlOpts {
    /// Frame-Nummer, wie sie angezeigt wird.
    pub fn label(&self, f: usize) -> usize {
        f + self.first_frame
    }

    fn to_json(self) -> serde_json::Value {
        let o = self.onion;
        serde_json::json!({
            "firstFrame": self.first_frame, "thumbs": self.thumbs, "thumbSize": self.thumb_size,
            "zone": match self.zone { Zone::Top => "top", Zone::Bottom => "bottom", Zone::Left => "left", Zone::Right => "right" },
            "onion": {
                "mode": if o.real_colors { "color" } else { "tint" }, "opacity": o.opacity, "step": o.step,
                "before": o.before, "after": o.after, "loopTag": o.loop_tag, "layerOnly": o.layer_only, "front": o.front,
            },
        })
    }

    fn from_json(v: &serde_json::Value) -> TlOpts {
        let d = TlOpts::default();
        let o = &v["onion"];
        let f = |x: &serde_json::Value, dflt: f32| x.as_f64().map_or(dflt, |n| n as f32);
        TlOpts {
            first_frame: if v["firstFrame"].as_u64() == Some(0) { 0 } else { 1 },
            thumbs: v["thumbs"].as_bool().unwrap_or(true),
            thumb_size: f(&v["thumbSize"], THUMB_DEFAULT).clamp(THUMB_MIN, THUMB_MAX),
            zone: match v["zone"].as_str() {
                Some("top") => Zone::Top,
                Some("left") => Zone::Left,
                Some("right") => Zone::Right,
                _ => Zone::Bottom,
            },
            onion: OnionOpts {
                real_colors: o["mode"].as_str() == Some("color"),
                opacity: f(&o["opacity"], d.onion.opacity),
                step: f(&o["step"], d.onion.step),
                before: o["before"].as_u64().map_or(d.onion.before, |n| n as u32),
                after: o["after"].as_u64().map_or(d.onion.after, |n| n as u32),
                loop_tag: o["loopTag"].as_bool().unwrap_or(false),
                layer_only: o["layerOnly"].as_bool().unwrap_or(false),
                front: o["front"].as_bool().unwrap_or(false),
            }
            .normalized(),
        }
    }
}

/// Zwischenspeicher: Onion-Textur und Vorschaubilder der Kopfzeile.
#[derive(Default)]
pub(crate) struct TlCache {
    onion: Option<(String, egui::TextureHandle)>,
    /// Frame → (was gerechnet wurde, Textur)
    thumbs: HashMap<usize, (String, egui::TextureHandle)>,
    thumbs_sprite: usize,
}

/// Größe der Vorschaubilder in der Kopfzeile (TlOpts::thumb_size): die
/// Trennlinie unter ihnen ziehen (timeline.rs), zwischen diesen Grenzen.
pub(crate) const THUMB_DEFAULT: f32 = 32.0;
pub(crate) const THUMB_MIN: f32 = 20.0;
pub(crate) const THUMB_MAX: f32 = 128.0;

impl SpritebitApp {
    /// Beim Start: gemerkte Einstellungen laden; ab dann auch speichern.
    pub(crate) fn load_tl_opts(&mut self) {
        self.tl_persist = true;
        let Some(p) = crate::i18n::settings_dir().map(|d| d.join("timeline.json")) else { return };
        if let Some(v) = std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()) {
            self.tl = TlOpts::from_json(&v);
        }
    }

    pub(crate) fn save_tl_opts(&self) {
        if !self.tl_persist {
            return;
        }
        if let Some(dir) = crate::i18n::settings_dir() {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join("timeline.json"), self.tl.to_json().to_string());
        }
    }

    /// Das Fenster mit den Einstellungen.
    pub(crate) fn tl_menu(&mut self, ctx: &egui::Context) {
        if !self.tl_menu_open {
            return;
        }
        let before = self.tl;
        let mut open = true;
        let cur = self.sprite().frame;
        let mut dur = self.sprite().frames[cur].duration_ms;
        let old_dur = dur;
        let mut reset = false;
        egui::Window::new(tr("Timeline")).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            let t = &mut self.tl;
            egui::Grid::new("tl-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label(tr("Position"));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut t.zone, Zone::Top, tr("Oben"));
                    ui.selectable_value(&mut t.zone, Zone::Bottom, tr("Unten"));
                    ui.selectable_value(&mut t.zone, Zone::Left, tr("Links"));
                    ui.selectable_value(&mut t.zone, Zone::Right, tr("Rechts"));
                });
                ui.end_row();
                ui.label(tr("Erster Frame"));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut t.first_frame, 0, "0");
                    ui.selectable_value(&mut t.first_frame, 1, "1");
                });
                ui.end_row();
                ui.label(tr("Kopfzeile"));
                ui.checkbox(&mut t.thumbs, tr("Vorschaubilder"));
                ui.end_row();
                ui.label(tr("Aktueller Frame"));
                ui.horizontal(|ui| {
                    ui.label(tr("Dauer"));
                    ui.add(egui::DragValue::new(&mut dur).range(0..=10_000).suffix(" ms"));
                });
                ui.end_row();
            });
            ui.weak(tr("0 = Dauer nach FPS."));
            ui.separator();
            ui.strong(tr("Onion Skin"));
            let o = &mut t.onion;
            egui::Grid::new("tl-onion").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label(tr("Darstellung"));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut o.real_colors, false, tr("Rot/Blau"));
                    ui.selectable_value(&mut o.real_colors, true, tr("Farben"));
                });
                ui.end_row();
                ui.label(tr("Deckkraft"));
                ui.add(egui::Slider::new(&mut o.opacity, 0.1..=0.9).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
                ui.end_row();
                ui.label(tr("Abstufung")).on_hover_text(tr("Um wie viel jeder weitere Frame blasser wird"));
                ui.add(egui::Slider::new(&mut o.step, 0.0..=0.9).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
                ui.end_row();
                ui.label(tr("Frames davor"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut o.before).range(0..=ONION_MAX));
                    ui.label(tr("danach"));
                    ui.add(egui::DragValue::new(&mut o.after).range(0..=ONION_MAX));
                });
                ui.end_row();
                ui.label(tr("Lage"));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut o.front, false, tr("Hinter dem Bild"));
                    ui.selectable_value(&mut o.front, true, tr("Davor"));
                });
                ui.end_row();
            });
            ui.checkbox(&mut o.loop_tag, tr("Im Tag im Kreis"))
                .on_hover_text(tr("Am Ende des Tags scheint sein Anfang durch — für Animationen, die im Kreis laufen"));
            ui.checkbox(&mut o.layer_only, tr("Nur die aktive Ebene"));
            reset = ui.button(tr("Zurücksetzen")).clicked();
        });
        if reset {
            self.tl.onion = OnionOpts::default();
        }
        if !open {
            self.tl_menu_open = false;
        }
        if dur != old_dur {
            self.project.sprite_mut().frames[cur].duration_ms = dur;
            self.dirty = true;
        }
        if self.tl != before {
            self.tl.onion = self.tl.onion.normalized();
            self.save_tl_opts();
        }
    }

    /// Onion Skin als eine Textur über `rect` (ausgedünnt um `step`).
    /// `None`: aus oder nichts zu zeigen.
    pub(crate) fn onion_texture(&mut self, ctx: &egui::Context, rect: Rect, step: u32) -> Option<egui::TextureId> {
        let sp = self.sprite();
        if !self.onion || self.playing || sp.frames.len() < 2 {
            return None;
        }
        let o = self.tl.onion;
        let list = onion_frames(sp, sp.frame, &o);
        if list.is_empty() {
            return None;
        }
        let key = format!("{}|{:?}|{}|{}|{}|{:?}", self.project.current, rect, step, sp.frame, self.version, o);
        if self.tl_cache.onion.as_ref().is_none_or(|c| c.0 != key) {
            let pal = self.project.current_palette();
            // Nur die aktive Ebene: eine Kopie, in der nur sie sichtbar ist.
            let only;
            let src: &Sprite = if o.layer_only {
                let mut c = sp.clone();
                for (l, layer) in c.layers.iter_mut().enumerate() {
                    layer.visible = l == sp.layer;
                    layer.opacity = 1.0;
                }
                only = c;
                &only
            } else {
                sp
            };
            let mut out: Vec<f32> = Vec::new();
            let (mut tw, mut th) = (0, 0);
            // Die fernsten zuerst, die nächsten obenauf.
            for of in list.iter().rev() {
                let (rgba, w, h) = render_rgba_step(src, &pal, of.frame, rect, step);
                (tw, th) = (w, h);
                if out.is_empty() {
                    out = vec![0.0; rgba.len()];
                }
                let tint: Option<[u8; 3]> = (!o.real_colors).then_some(if of.before { [255, 96, 96] } else { [96, 156, 255] });
                for (px, dst) in rgba.as_chunks::<4>().0.iter().zip(out.as_chunks_mut::<4>().0.iter_mut()) {
                    if px[3] == 0 {
                        continue;
                    }
                    let c = tint.unwrap_or([px[0], px[1], px[2]]);
                    let a = of.alpha;
                    let da = dst[3] * (1.0 - a);
                    let na = a + da;
                    for k in 0..3 {
                        dst[k] = (c[k] as f32 * a + dst[k] * da) / na;
                    }
                    dst[3] = na;
                }
            }
            let bytes: Vec<u8> = out.as_chunks::<4>().0.iter().flat_map(|p| [p[0] as u8, p[1] as u8, p[2] as u8, (p[3] * 255.0).round() as u8]).collect();
            let img = egui::ColorImage::from_rgba_unmultiplied([tw as usize, th as usize], &bytes);
            match &mut self.tl_cache.onion {
                Some((k, t)) => {
                    t.set(img, egui::TextureOptions::NEAREST);
                    *k = key;
                }
                None => self.tl_cache.onion = Some((key, ctx.load_texture("onion", img, egui::TextureOptions::NEAREST))),
            }
        }
        self.tl_cache.onion.as_ref().map(|c| c.1.id())
    }

    /// Vorschaubild eines Frames für die Kopfzeile.
    pub(crate) fn thumb(&mut self, ctx: &egui::Context, f: usize) -> Option<egui::TextureId> {
        let cur = self.project.current;
        if self.tl_cache.thumbs_sprite != cur {
            self.tl_cache.thumbs.clear();
            self.tl_cache.thumbs_sprite = cur;
        }
        let sp = self.sprite();
        // Neu gerechnet wird, wenn sich die Zellen oder Ebenen ändern — und
        // für den aktuellen Frame bei jeder Änderung (dort wird gemalt).
        let layers: Vec<(bool, u32)> = sp.layers.iter().map(|l| (l.visible, (l.opacity * 100.0) as u32)).collect();
        let stamp = if f == sp.frame { self.version } else { 0 };
        let key = format!("{:?}|{:?}|{}|{}", sp.frames[f].cels, layers, stamp, sp.palette);
        if self.tl_cache.thumbs.get(&f).is_none_or(|c| c.0 != key) {
            let pal = self.project.current_palette();
            let step = sp.width.max(sp.height).div_ceil(THUMB_MAX as u32 * 2).max(1);
            let (rgba, w, h) = render_rgba_step(sp, &pal, f, Rect { x: 0, y: 0, w: sp.width, h: sp.height }, step);
            let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            let tex = ctx.load_texture(format!("thumb-{f}"), img, egui::TextureOptions::NEAREST);
            self.tl_cache.thumbs.insert(f, (key, tex));
        }
        self.tl_cache.thumbs.get(&f).map(|c| c.1.id())
    }
}

/// Farbe für den Rahmen eines Vorschaubilds.
pub(crate) const THUMB_FRAME: Color32 = Color32::from_gray(60);
