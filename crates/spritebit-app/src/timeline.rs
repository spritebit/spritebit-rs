//! Timeline: Ebenen × Frames als Raster, wie in der Web-Version.
//!
//! ```text
//!   [⏮ ◀ ▶ ▶ ⏭]  Frame [+ ⧉ 🗑]  Ebene [+ ▲ ▼ 🗑]  Zellen [⧉ 📋 ⌫ ⛓ ⛓̸]  [Tag] [Onion]  FPS  Dauer
//!               │ [Laufen──────]        ← Tags
//!               │  1   2   3   4
//!   👁 🔒 ⛓ Figur │  ●   ●   ○   ●
//!   👁 🔒 ⛓ Grund │  ●━━━●━━━●━━━●     ← verknüpft: dasselbe Bild
//! ```
//!
//! * Klick auf eine Zelle wählt Frame und Ebene; Shift-Klick spannt einen
//!   Bereich auf, auf den die Zellen-Knöpfe wirken.
//! * Klick auf eine Frame-Nummer wählt den Frame, auf einen Tag öffnet ihn.
//! * Auge, Schloss, Kette schalten Sichtbarkeit, Sperre, „durchgehend";
//!   Doppelklick auf den Namen benennt die Ebene um.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Sense, Stroke, Vec2};
use spritebit_core::cels::{self, CelRange};
use spritebit_core::sprite::{Direction, Tag};

use crate::{icons, tlmenu_ui, SpritebitApp};

/// Was in der Timeline gerade gezogen wird.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TlDrag {
    Frame(usize),
    Layer(usize),
}
use crate::i18n::{tr, trf};

const LAYER_W: f32 = 170.0;
const TAG_H: f32 = 16.0;
const HEAD_H: f32 = 20.0;
const ROW_H: f32 = 22.0;
const CELL_W: f32 = 26.0;
const ICON_W: f32 = 20.0;

const ACCENT: Color32 = Color32::from_rgb(0x6c, 0x9e, 0xf8);
const DIM: Color32 = Color32::from_gray(140);
const TAG_COLORS: [[u8; 3]; 6] = [[0xe5, 0x53, 0x4b], [0xe0, 0x82, 0x3d], [0xc9, 0xb3, 0x3a], [0x57, 0xab, 0x5a], [0x4a, 0xa3, 0xdf], [0x98, 0x6e, 0xe2]];

impl SpritebitApp {
    /// Strukturänderung am aktuellen Sprite — als ein Undo-Schritt.
    pub(crate) fn edit_sprite(&mut self, f: impl FnOnce(&mut spritebit_core::Sprite)) {
        self.deselect();
        let cur = self.project.current;
        self.histories[cur].record(&self.project.sprites[cur]);
        f(&mut self.project.sprites[cur]);
        self.changed();
    }

    /// Markierte Frames (Strg/Umschalt+Klick in der Kopfzeile), gültig für
    /// den Sprite; leer, wenn nichts markiert ist.
    pub(crate) fn marked_frames(&self) -> Vec<usize> {
        let n = self.sprite().frames.len();
        self.frame_sel.iter().copied().filter(|&f| f < n).collect()
    }

    pub(crate) fn go_frame(&mut self, f: usize) {
        self.deselect();
        let n = self.project.sprite().frames.len();
        self.project.sprite_mut().frame = f.min(n - 1);
        self.stroke_last = None;
    }

    /// Der Bereich, auf den die Zellen-Knöpfe wirken — ohne Bereich die
    /// aktive Zelle.
    fn cur_range(&self) -> CelRange {
        let sp = self.project.sprite();
        self.cel_range.and_then(|r| r.clamp(sp)).unwrap_or(CelRange::single(sp.frame, sp.layer))
    }

    pub(crate) fn timeline(&mut self, ui: &mut egui::Ui) {
        self.timeline_buttons(ui);
        ui.add_space(4.0);
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| self.timeline_grid(ui));
        self.timeline_windows(ui.ctx());
    }

    fn timeline_buttons(&mut self, ui: &mut egui::Ui) {
        let n = self.project.sprite().frames.len();
        let cur = self.project.sprite().frame;
        ui.horizontal_wrapped(|ui| {
            if icons::button(ui, icons::FIRST, tr("Erster Frame (Pos1)"), true).clicked() {
                self.go_frame(0);
            }
            if icons::button(ui, icons::PREV, tr("Voriger Frame (,)"), true).clicked() {
                self.go_frame((cur + n - 1) % n);
            }
            let play = if self.playing { icons::PAUSE } else { icons::PLAY };
            if icons::button(ui, play, tr("Abspielen / Anhalten (Enter)"), n > 1).clicked() {
                self.toggle_play(ui.ctx());
            }
            if icons::button(ui, icons::NEXT, tr("Nächster Frame (.)"), true).clicked() {
                self.go_frame((cur + 1) % n);
            }
            if icons::button(ui, icons::LAST, tr("Letzter Frame (Ende)"), true).clicked() {
                self.go_frame(n - 1);
            }
            ui.separator();
            ui.weak(tr("Frame"));
            if icons::button(ui, icons::PLUS, tr("Leerer Frame dahinter"), true).clicked() {
                self.edit_sprite(|s| {
                    let f = s.frame;
                    s.add_frame(f, false);
                });
            }
            if icons::button(ui, icons::COPY, tr("Frame duplizieren"), true).clicked() {
                self.edit_sprite(|s| {
                    let f = s.frame;
                    s.add_frame(f, true);
                });
            }
            // Löschen nimmt alle markierten Frames mit (ein Undo-Schritt);
            // einer bleibt immer.
            let marked = self.marked_frames();
            if icons::button(ui, icons::TRASH, tr("Frame löschen (markierte alle)"), n > 1).clicked() {
                self.frame_sel.clear();
                self.edit_sprite(|s| {
                    let list = if marked.len() > 1 { marked } else { vec![s.frame] };
                    for f in list.into_iter().rev() {
                        s.delete_frame(f);
                    }
                });
            }
            if icons::button(ui, icons::LEFT, tr("Frame nach links"), cur > 0).clicked() {
                self.edit_sprite(|s| s.move_frame(cur, cur - 1));
            }
            if icons::button(ui, icons::RIGHT, tr("Frame nach rechts"), cur + 1 < n).clicked() {
                self.edit_sprite(|s| s.move_frame(cur, cur + 1));
            }
            // Sprungfeld: Nummer eintippen, gezählt wie in der Kopfzeile.
            let first = self.tl.first_frame;
            let mut go = cur + first;
            if ui.add(egui::DragValue::new(&mut go).range(first..=n - 1 + first)).on_hover_text(tr("Zu Frame springen")).changed() {
                self.go_frame(go - first);
            }
            ui.separator();
            ui.weak(tr("Ebene"));
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
            // Ebenenmaske der aktiven Ebene (spritebit_core::mask).
            ui.separator();
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
            if icons::button(ui, icons::TRASH, tr("Ebene löschen"), nl > 1).clicked() {
                self.edit_sprite(|s| {
                    let l = s.layer;
                    s.delete_layer(l);
                });
            }
            ui.separator();
            ui.weak(tr("Zellen"));
            let r = self.cur_range();
            let sp = self.project.sprite();
            let linked = (r.l0..=r.l1).any(|l| (r.f0..=r.f1).any(|f| sp.is_linked(f, l)));
            let can_link = r.f1 > r.f0 || sp.frame > 0;
            if icons::button(ui, icons::COPY, tr("Zellen kopieren (Bereich per Shift-Klick)"), true).clicked() {
                self.cel_clip = Some(cels::copy(self.project.sprite(), r));
            }
            if icons::button(ui, icons::PASTE, tr("Zellen an der aktiven Zelle einfügen"), self.cel_clip.is_some()).clicked() {
                if let Some(clip) = self.cel_clip.clone() {
                    let mut used = None;
                    self.edit_sprite(|s| {
                        let (f, l) = (s.frame, s.layer);
                        used = cels::paste(s, &clip, f, l);
                    });
                    match used {
                        Some(u) if u.size() > 1 => self.cel_range = Some(u),
                        Some(_) => self.cel_range = None,
                        None => self.hint = Some(tr("Hier passt nichts hin — die Zellen haben eine andere Größe.").into()),
                    }
                }
            }
            if icons::button(ui, icons::ERASER, tr("Zellen leeren"), true).clicked() {
                self.edit_sprite(|s| cels::clear(s, r));
            }
            if icons::button(ui, icons::LINK, tr("Verknüpfen — die Frames teilen sich je Ebene ein Bild (ohne Bereich: mit dem Frame davor)"), can_link).clicked() {
                let r = if r.f1 > r.f0 { r } else { CelRange { f0: r.f0 - 1, ..r } };
                self.edit_sprite(|s| {
                    cels::link(s, r);
                });
            }
            if icons::button(ui, icons::UNLINK, tr("Lösen — jede Zelle bekommt ihr eigenes Bild"), linked).clicked() {
                self.edit_sprite(|s| {
                    cels::unlink(s, r);
                });
            }
            ui.separator();
            if icons::button(ui, icons::TAG, tr("Tag anlegen — benennt den Bereich bzw. den Frame, z. B. „Laufen“"), true).clicked() {
                let k = self.project.sprite().tags.len();
                self.edit_sprite(|s| {
                    s.tags.push(Tag {
                        name: format!("Tag {}", k + 1),
                        from: r.f0,
                        to: r.f1,
                        color: TAG_COLORS[k % TAG_COLORS.len()],
                        direction: Direction::Forward,
                    });
                });
                self.tag_edit = Some(k);
            }
            let tip = tr("Onion Skin — voriger (rot) und nächster Frame (blau) scheinen durch");
            let onion_btn = egui::Button::selectable(self.onion, icons::image(icons::ONION, ui.visuals().text_color()).alt_text(tip));
            if ui.add(onion_btn).on_hover_text(tip).clicked() {
                self.onion = !self.onion;
                self.version = self.version.wrapping_add(1);
            }
            ui.separator();
            let mut fps = self.project.sprite().fps;
            ui.label(tr("FPS"));
            if ui.add(egui::DragValue::new(&mut fps).range(1..=60)).changed() {
                self.project.sprite_mut().fps = fps;
                self.dirty = true;
            }
            ui.separator();
            let tip = tr("Timeline-Einstellungen — Lage, Kopfzeile, Dauer, Onion Skin");
            let gear = egui::Button::selectable(self.tl_menu_open, icons::image(icons::SLIDERS, ui.visuals().text_color()).alt_text(tip));
            if ui.add(gear).on_hover_text(tip).clicked() {
                self.tl_menu_open = !self.tl_menu_open;
            }
        });
    }

    fn timeline_grid(&mut self, ui: &mut egui::Ui) {
        let tl = self.tl;
        // Vorschaubilder vorher holen — sie brauchen `&mut self`.
        let thumbs: Vec<Option<egui::TextureId>> =
            if tl.thumbs { (0..self.project.sprite().frames.len()).map(|f| self.thumb(ui.ctx(), f)).collect() } else { Vec::new() };
        let head_h = HEAD_H + if tl.thumbs { tlmenu_ui::THUMB + 4.0 } else { 0.0 };
        let sp = self.project.sprite();
        let (n, nl) = (sp.frames.len(), sp.layers.len());
        let tag_h = if sp.tags.is_empty() { 0.0 } else { TAG_H + 2.0 };
        let size = Vec2::new(LAYER_W + n as f32 * CELL_W + 8.0, tag_h + head_h + nl as f32 * ROW_H + 4.0);
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let o = resp.rect.min;
        let font = FontId::proportional(12.0);
        let head_y = o.y + tag_h;
        // Zeile `r` von oben zeigt Ebene nl-1-r — oberste Ebene oben.
        let row_y = |r: usize| head_y + head_h + r as f32 * ROW_H;
        let col_x = |f: usize| o.x + LAYER_W + f as f32 * CELL_W;
        let range = self.cel_range.and_then(|r| r.clamp(sp)).filter(|r| r.size() > 1);

        // Tags über den Frame-Nummern.
        for t in &sp.tags {
            let r = egui::Rect::from_min_max(Pos2::new(col_x(t.from) + 1.0, o.y), Pos2::new(col_x(t.to + 1) - 1.0, o.y + TAG_H));
            let [cr, cg, cb] = t.color;
            painter.rect_filled(r, 3.0, Color32::from_rgb(cr, cg, cb).gamma_multiply(0.45));
            painter.rect_filled(egui::Rect::from_min_size(r.min, Vec2::new(3.0, TAG_H)), 1.0, Color32::from_rgb(cr, cg, cb));
            let mark = match t.direction {
                Direction::Forward => "",
                Direction::Reverse => "« ",
                Direction::PingPong => "↔ ",
            };
            painter.with_clip_rect(r).text(r.left_center() + Vec2::new(6.0, 0.0), Align2::LEFT_CENTER, format!("{mark}{}", t.name), FontId::proportional(11.0), Color32::WHITE);
        }

        // Kopfzeile: Frame-Nummern (1-basiert, wie in der Web-Version).
        for f in 0..n {
            let c = egui::Rect::from_min_size(Pos2::new(col_x(f), head_y), Vec2::new(CELL_W, head_h));
            if f == sp.frame {
                painter.rect_filled(c.shrink(1.0), 3.0, ACCENT.gamma_multiply(0.35));
            } else if self.frame_sel.contains(&f) {
                painter.rect_filled(c.shrink(1.0), 3.0, ACCENT.gamma_multiply(0.2));
            }
            let num = egui::Rect::from_min_size(c.min, Vec2::new(CELL_W, HEAD_H));
            painter.text(num.center(), Align2::CENTER_CENTER, format!("{}", tl.label(f)), font.clone(), if f == sp.frame { Color32::WHITE } else { DIM });
            if let Some(Some(id)) = thumbs.get(f) {
                // Vorschaubild im Seitenverhältnis des Sprites, mittig.
                let k = tlmenu_ui::THUMB / sp.width.max(sp.height) as f32;
                let size = Vec2::new(sp.width as f32 * k, sp.height as f32 * k);
                let r = egui::Rect::from_center_size(Pos2::new(c.center().x, c.min.y + HEAD_H + 2.0 + tlmenu_ui::THUMB / 2.0), size);
                painter.rect_filled(r.expand(1.0), 1.0, tlmenu_ui::THUMB_FRAME);
                painter.image(*id, r, egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
        }

        for r in 0..nl {
            let l = nl - 1 - r;
            let layer = &sp.layers[l];
            let y = row_y(r);
            // Ebenen-Spalte
            let row = egui::Rect::from_min_size(Pos2::new(o.x, y), Vec2::new(LAYER_W - 4.0, ROW_H - 2.0));
            painter.rect_filled(row, 3.0, if l == sp.layer { ACCENT.gamma_multiply(0.25) } else { Color32::from_gray(38) });
            let icon_rect = |k: usize| {
                egui::Rect::from_center_size(Pos2::new(o.x + 10.0 + k as f32 * ICON_W, y + ROW_H / 2.0 - 1.0), Vec2::splat(14.0))
            };
            let eye = if layer.visible { icons::EYE } else { icons::EYE_OFF };
            egui::Image::new(eye).tint(DIM).paint_at(ui, icon_rect(0));
            let (lock, lock_c) = if layer.locked { (icons::LOCK, ACCENT) } else { (icons::UNLOCK, Color32::from_gray(90)) };
            egui::Image::new(lock).tint(lock_c).paint_at(ui, icon_rect(1));
            let (cont, cont_c) = if layer.continuous { (icons::CONT_ON, ACCENT) } else { (icons::CONT_OFF, Color32::from_gray(90)) };
            egui::Image::new(cont).tint(cont_c).paint_at(ui, icon_rect(2));
            let name_col = if l == sp.layer { Color32::WHITE } else if layer.visible { Color32::from_gray(200) } else { Color32::from_gray(110) };
            painter.text(Pos2::new(o.x + 3.0 * ICON_W + 6.0, y + ROW_H / 2.0 - 1.0), Align2::LEFT_CENTER, &layer.name, font.clone(), name_col);

            // Zellen
            for f in 0..n {
                let c = egui::Rect::from_min_size(Pos2::new(col_x(f), y), Vec2::new(CELL_W, ROW_H - 2.0));
                let active = f == sp.frame && l == sp.layer;
                let in_range = range.is_some_and(|rg| rg.contains(f, l));
                let bg = if active {
                    ACCENT.gamma_multiply(0.45)
                } else if in_range {
                    ACCENT.gamma_multiply(0.25)
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

        // Ziehen: Frame-Nummer → Frame verschieben, Ebenen-Name → Ebene verschieben.
        let at_pos = |p: Pos2| {
            let fx = ((p.x - o.x - LAYER_W) / CELL_W).floor();
            let ry = ((p.y - head_y - head_h) / ROW_H).floor();
            ((fx >= 0.0 && (fx as usize) < n).then_some(fx as usize), (ry >= 0.0 && (ry as usize) < nl).then(|| nl - 1 - ry as usize))
        };
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos() {
                let (f, l) = at_pos(p);
                if p.y >= head_y && p.y < head_y + head_h && p.x >= o.x + LAYER_W {
                    self.tl_drag = f.map(TlDrag::Frame);
                } else if p.x < o.x + LAYER_W && p.x >= o.x + 3.0 * ICON_W {
                    self.tl_drag = l.map(TlDrag::Layer);
                }
            }
        }
        if let (Some(d), Some(p)) = (self.tl_drag, ui.input(|i| i.pointer.hover_pos())) {
            let (f, l) = at_pos(p);
            let mark = Stroke::new(2.0, Color32::WHITE);
            match (d, f, l) {
                (TlDrag::Frame(_), Some(f), _) => {
                    painter.line_segment([Pos2::new(col_x(f), head_y), Pos2::new(col_x(f), head_y + head_h)], mark);
                }
                (TlDrag::Layer(_), _, Some(l)) => {
                    let y = row_y(nl - 1 - l);
                    painter.line_segment([Pos2::new(o.x, y), Pos2::new(o.x + LAYER_W - 4.0, y)], mark);
                }
                _ => {}
            }
            if resp.drag_stopped() {
                self.tl_drag = None;
                match (d, f, l) {
                    (TlDrag::Frame(from), Some(to), _) if from != to => {
                        self.frame_sel.clear();
                        self.edit_sprite(|s| s.move_frame(from, to));
                    }
                    (TlDrag::Layer(from), _, Some(to)) if from != to => self.edit_sprite(|s| s.move_layer(from, to)),
                    _ => {}
                }
                return;
            }
        }

        // Klicks auswerten
        let (shift, ctrl) = ui.input(|i| (i.modifiers.shift, i.modifiers.command));
        let double = resp.double_clicked();
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.clicked() || double) {
            let fx = ((p.x - o.x - LAYER_W) / CELL_W).floor();
            let ry = ((p.y - head_y - head_h) / ROW_H).floor();
            let frame_at = (fx >= 0.0 && (fx as usize) < n).then_some(fx as usize);
            if p.y < head_y {
                // Tag-Spur
                if let Some(f) = frame_at {
                    self.tag_edit = self.project.sprite().tags.iter().position(|t| f >= t.from && f <= t.to);
                }
            } else if p.y < head_y + head_h {
                if let Some(f) = frame_at {
                    // Wie im Dateimanager: Strg+Klick nimmt einzelne Frames
                    // dazu, Umschalt+Klick eine Spanne, ein Klick allein hebt auf.
                    let cur = self.project.sprite().frame;
                    if ctrl {
                        if self.frame_sel.is_empty() {
                            self.frame_sel.push(cur);
                        }
                        match self.frame_sel.iter().position(|&x| x == f) {
                            Some(i) => {
                                self.frame_sel.remove(i);
                            }
                            None => self.frame_sel.push(f),
                        }
                        self.frame_sel.sort_unstable();
                    } else if shift {
                        let a = self.frame_anchor.unwrap_or(cur);
                        self.frame_sel = (a.min(f)..=a.max(f)).collect();
                    } else {
                        self.frame_sel.clear();
                        self.frame_anchor = Some(f);
                    }
                    self.cel_range = None;
                    self.go_frame(f);
                }
            } else if ry >= 0.0 && (ry as usize) < nl {
                let l = nl - 1 - ry as usize;
                if p.x < o.x + LAYER_W {
                    let k = ((p.x - o.x) / ICON_W).floor() as usize;
                    match k {
                        0 => self.edit_sprite(|s| s.layers[l].visible = !s.layers[l].visible),
                        1 => self.edit_sprite(|s| s.layers[l].locked = !s.layers[l].locked),
                        2 => self.edit_sprite(|s| s.layers[l].continuous = !s.layers[l].continuous),
                        _ if double => self.rename_layer = Some((l, self.project.sprite().layers[l].name.clone())),
                        _ => {
                            self.deselect();
                            self.project.sprite_mut().layer = l;
                        }
                    }
                } else if let Some(f) = frame_at {
                    self.deselect();
                    if shift {
                        let (af, al) = self.cel_anchor.unwrap_or((self.project.sprite().frame, self.project.sprite().layer));
                        self.cel_range = Some(CelRange::new(af, al, f, l));
                    } else {
                        self.cel_range = None;
                        self.cel_anchor = Some((f, l));
                    }
                    let s = self.project.sprite_mut();
                    s.layer = l;
                    s.frame = f;
                    self.stroke_last = None;
                }
            }
        }
    }

    /// Fenster der Timeline: Ebene umbenennen, Tag bearbeiten.
    fn timeline_windows(&mut self, ctx: &egui::Context) {
        // Ebene umbenennen
        let mut done = None;
        let mut cancel = false;
        if let Some((l, name)) = &mut self.rename_layer {
            egui::Window::new(tr("Ebene umbenennen")).collapsible(false).resizable(false).show(ctx, |ui| {
                let r = ui.text_edit_singleline(name);
                r.request_focus();
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if ui.button(tr("OK")).clicked() || enter {
                        done = Some((*l, name.trim().to_string()));
                    }
                    if ui.button(tr("Abbrechen")).clicked() {
                        cancel = true;
                    }
                });
            });
        }
        if let Some((l, name)) = done {
            if !name.is_empty() && l < self.project.sprite().layers.len() {
                self.edit_sprite(|s| s.layers[l].name = name);
            }
            self.rename_layer = None;
        }
        if cancel {
            self.rename_layer = None;
        }

        // Tag bearbeiten
        let Some(k) = self.tag_edit else { return };
        let n = self.project.sprite().frames.len();
        let Some(mut t) = self.project.sprite().tags.get(k).cloned() else {
            self.tag_edit = None;
            return;
        };
        let before = t.clone();
        let (mut close, mut delete, mut play) = (false, false, false);
        egui::Window::new(tr("Tag")).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.text_edit_singleline(&mut t.name);
            ui.horizontal(|ui| {
                ui.label(tr("Frames"));
                let first = self.tl.first_frame;
                let (mut a, mut b) = (t.from + first, t.to + first);
                ui.add(egui::DragValue::new(&mut a).range(first..=n - 1 + first));
                ui.label("–");
                ui.add(egui::DragValue::new(&mut b).range(first..=n - 1 + first));
                t.from = a.min(b) - first;
                t.to = a.max(b) - first;
            });
            ui.horizontal(|ui| {
                ui.label(tr("Richtung"));
                egui::ComboBox::from_id_salt("tag-dir")
                    .selected_text(match t.direction {
                        Direction::Forward => tr("Vorwärts"),
                        Direction::Reverse => tr("Rückwärts"),
                        Direction::PingPong => tr("Ping-Pong"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut t.direction, Direction::Forward, tr("Vorwärts"));
                        ui.selectable_value(&mut t.direction, Direction::Reverse, tr("Rückwärts"));
                        ui.selectable_value(&mut t.direction, Direction::PingPong, tr("Ping-Pong"));
                    });
            });
            ui.horizontal(|ui| {
                ui.label(tr("Farbe"));
                egui::color_picker::color_edit_button_srgb(ui, &mut t.color);
            });
            ui.separator();
            ui.horizontal(|ui| {
                play = ui.button(tr("Abspielen")).clicked();
                delete = ui.button(tr("Löschen")).clicked();
                close = ui.button(tr("Fertig")).clicked();
            });
        });
        if delete {
            self.edit_sprite(|s| {
                s.tags.remove(k);
            });
            self.tag_edit = None;
            return;
        }
        if t != before {
            if t.name.trim().is_empty() {
                t.name = before.name.clone();
            }
            self.edit_sprite(|s| s.tags[k] = t.clone());
        }
        if play {
            self.go_frame(t.from);
            if !self.playing {
                self.toggle_play(ctx);
            }
            close = true;
        }
        if close {
            self.tag_edit = None;
        }
    }

    // ── Abspielen ───────────────────────────────────────────────────
    pub(crate) fn toggle_play(&mut self, ctx: &egui::Context) {
        self.playing = !self.playing && self.project.sprite().frames.len() > 1;
        self.frame_started = ctx.input(|i| i.time);
        self.play_step = 0;
    }

    /// Bei jedem Durchlauf: ist die Zeit des Frames um, kommt der nächste.
    /// Steht man in einem Tag, läuft nur dieser — in seiner Richtung.
    pub(crate) fn advance_playback(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let now = ctx.input(|i| i.time);
        let sp = self.project.sprite();
        let dur = sp.frame_duration(sp.frame) as f64 / 1000.0;
        if now - self.frame_started >= dur {
            let f = sp.frame;
            let next = match sp.tags.iter().filter(|t| f >= t.from && f <= t.to && t.to > t.from).min_by_key(|t| t.to - t.from) {
                Some(t) => {
                    let order = spritebit_core::export::tag_frames(t);
                    let k = if order.get(self.play_step) == Some(&f) { self.play_step } else { order.iter().position(|&x| x == f).unwrap_or(0) };
                    self.play_step = (k + 1) % order.len();
                    order[self.play_step]
                }
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
