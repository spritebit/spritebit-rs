//! Reiter der geöffneten Sprites über der Zeichenfläche — wie in der
//! Web-Version (`tabs.js`) und in Grafikprogrammen üblich.
//!
//! Ein Reiter ist ein geöffneter Sprite. Klick wechselt, × oder Mittelklick
//! schließt den Reiter (der Sprite bleibt im Projekt, ein Klick in der
//! Sprite-Liste öffnet ihn wieder), Doppelklick benennt um, Ziehen ordnet,
//! der Knopf „+“ legt einen neuen an. Strg+Tab / Strg+Umschalt+Tab schalten
//! weiter, Strg+W schließt. Der aktive Sprite hat immer einen Reiter, darum
//! lässt sich der letzte nicht schließen.
//!
//! Sprites werden hier über ihre Stelle in `project.sprites` angesprochen.
//! Wer dort einfügt oder löscht, ruft [`on_insert`] bzw. [`on_remove`];
//! alles andere (neuer aktiver Sprite, Projekt gewechselt) gleicht [`sync`]
//! in jedem Frame ab.

use eframe::egui;

use crate::i18n::tr;
use crate::sprites_ui::SpriteDialog;
use crate::SpritebitApp;

/// Reiter an den Projektstand anpassen: ungültige und doppelte fallen weg,
/// der aktive kommt hinten dazu, falls er fehlt.
pub(crate) fn sync(tabs: &mut Vec<usize>, len: usize, cur: usize) {
    let mut seen = Vec::with_capacity(tabs.len());
    tabs.retain(|&t| {
        let keep = t < len && !seen.contains(&t);
        seen.push(t);
        keep
    });
    if cur < len && !tabs.contains(&cur) {
        tabs.push(cur);
    }
}

/// An Stelle `i` wurde ein Sprite eingefügt: alles dahinter rückt auf.
pub(crate) fn on_insert(tabs: &mut [usize], i: usize) {
    for t in tabs.iter_mut().filter(|t| **t >= i) {
        *t += 1;
    }
}

/// Sprite `i` wurde gelöscht: sein Reiter fällt weg, alles dahinter rückt vor.
pub(crate) fn on_remove(tabs: &mut Vec<usize>, i: usize) {
    tabs.retain(|&t| t != i);
    for t in tabs.iter_mut().filter(|t| **t > i) {
        *t -= 1;
    }
}

/// Reiter von Sprite `i` schließen. Gibt den neuen aktiven Sprite zurück,
/// wenn es der aktive war: rechter Nachbar, am Ende der linke.
pub(crate) fn close(tabs: &mut Vec<usize>, i: usize, cur: usize) -> Option<usize> {
    let pos = tabs.iter().position(|&t| t == i)?;
    if tabs.len() <= 1 {
        return None;
    }
    tabs.remove(pos);
    (i == cur).then(|| tabs[pos.min(tabs.len() - 1)])
}

/// Reiter an Stelle `from` vor die Stelle `to` setzen (`to == len` = ans Ende).
pub(crate) fn move_tab(tabs: &mut Vec<usize>, from: usize, to: usize) {
    if from >= tabs.len() || from == to || from + 1 == to {
        return;
    }
    let t = tabs.remove(from);
    let to = if to > from { to - 1 } else { to };
    tabs.insert(to.min(tabs.len()), t);
}

/// Stelle, vor die ein Reiter beim Loslassen an `x` kommt: hinter alle,
/// deren Mitte links davon liegt (`rects.len()` = ans Ende).
pub(crate) fn insert_at(rects: &[egui::Rect], x: f32) -> usize {
    rects.iter().filter(|r| r.center().x < x).count()
}

/// Nachbar zum Weiterschalten: +1 rechts, -1 links, rundum.
pub(crate) fn step(tabs: &[usize], cur: usize, dir: i32) -> Option<usize> {
    if tabs.is_empty() {
        return None;
    }
    let n = tabs.len() as i32;
    let i = tabs.iter().position(|&t| t == cur).map_or(0, |p| (p as i32 + dir).rem_euclid(n));
    Some(tabs[i as usize])
}

impl SpritebitApp {
    fn close_tab(&mut self, i: usize) {
        if let Some(next) = close(&mut self.tabs, i, self.project.current) {
            self.select_sprite(next);
        }
    }

    /// Strg+Tab, Strg+Umschalt+Tab, Strg+W.
    pub(crate) fn tab_keys(&mut self, ctx: &egui::Context) {
        let (back, fwd, shut) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Tab),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Tab),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::W),
            )
        });
        sync(&mut self.tabs, self.project.sprites.len(), self.project.current);
        let cur = self.project.current;
        if let Some(i) = if fwd { step(&self.tabs, cur, 1) } else if back { step(&self.tabs, cur, -1) } else { None } {
            self.select_sprite(i);
        }
        if shut {
            self.close_tab(cur);
        }
    }

    /// Die Reiterzeile über der Zeichenfläche.
    pub(crate) fn sprite_tabs(&mut self, ui: &mut egui::Ui) {
        sync(&mut self.tabs, self.project.sprites.len(), self.project.current);
        let cur = self.project.current;
        let closable = self.tabs.len() > 1;
        let (mut pick, mut shut, mut rename, mut moved) = (None, None, None, None);
        // Karteireiter: oben rund, unten gerade. Unter allen
        // läuft eine Linie; der aktive hat die Farbe der Fläche und
        // unterbricht sie — so geht er in die Zeichenfläche über.
        let below = crate::STAGE_BG;
        let line = ui.visuals().widgets.noninteractive.bg_stroke;
        let accent = ui.visuals().selection.stroke.color;
        let radius = egui::CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 };
        let full = ui.max_rect().x_range();
        let mut active_rect = None;
        let mut tabs_bottom = f32::NEG_INFINITY;
        // Ziehen: welcher Reiter gezogen wird, wo alle liegen. Die Namen sind
        // nicht markierbar — sonst nimmt die Textauswahl das Ziehen.
        let (mut dragging, mut dropped) = (None, None);
        let mut rects = Vec::with_capacity(self.tabs.len());
        let row = egui::ScrollArea::horizontal().id_salt("sprite-tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (pos, &i) in self.tabs.iter().enumerate() {
                    let sp = &self.project.sprites[i];
                    let active = i == cur;
                    let frame = egui::Frame::new()
                        .inner_margin(egui::Margin { left: 10, right: 4, top: 4, bottom: 4 })
                        .corner_radius(radius)
                        .stroke(line)
                        .fill(if active { below } else { ui.visuals().extreme_bg_color });
                    let inner = frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let text = egui::RichText::new(sp.name.as_str());
                            let label = ui
                                .add(egui::Label::new(if active { text.strong() } else { text }).truncate().selectable(false).sense(egui::Sense::click_and_drag()))
                                .on_hover_text(format!("{} · {}×{} · {}", sp.name, sp.width, sp.height, sp.palette));
                            if closable && ui.add(egui::Button::new("×").small().frame(false)).on_hover_text(tr("Reiter schließen (Mittelklick) — der Sprite bleibt im Projekt")).clicked() {
                                shut = Some(i);
                            }
                            label
                        })
                        .inner
                    });
                    let label = inner.inner;
                    tabs_bottom = tabs_bottom.max(inner.response.rect.bottom());
                    if active {
                        active_rect = Some(inner.response.rect);
                    }
                    if label.clicked() {
                        pick = Some(i);
                    }
                    if label.middle_clicked() && closable {
                        shut = Some(i);
                    }
                    if label.double_clicked() {
                        rename = Some(i);
                    }
                    rects.push(inner.response.rect);
                    // Schon beim Drücken die greifende Hand — man sieht, dass man
                    // den Reiter festhält, bevor er sich bewegt.
                    if label.is_pointer_button_down_on() || label.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                    if label.dragged() {
                        dragging = Some(pos);
                    }
                    if label.drag_stopped() {
                        dropped = Some(pos);
                    }
                }
                // Ziehen ordnet: die Maus entscheidet, zwischen welchen Reitern
                // er landet — auch ganz vorne oder hinten.
                if let (Some(from), Some(x)) = (dragging.or(dropped), ui.ctx().pointer_interact_pos().map(|p| p.x)) {
                    let to = insert_at(&rects, x);
                    if dropped.is_some() {
                        moved = Some((from, to));
                    } else if to != from && to != from + 1 {
                        let x = if to < rects.len() { rects[to].left() - 1.0 } else { rects[rects.len() - 1].right() + 1.0 };
                        ui.painter().vline(x, rects[from].y_range(), egui::Stroke::new(2.0, accent));
                    }
                }
                if ui.button("+").on_hover_text(tr("Neuer Sprite")).clicked() {
                    self.open_new_sprite();
                }
            });
        });
        // Grundlinie, vom aktiven Reiter unterbrochen; oben am aktiven die Akzentlinie.
        let bottom = if tabs_bottom.is_finite() { tabs_bottom } else { row.inner_rect.bottom() };
        let p = ui.painter();
        p.hline(full, bottom - 0.5, line);
        if let Some(r) = active_rect {
            p.hline(r.x_range().shrink(line.width), bottom - 0.5, egui::Stroke::new(line.width + 1.5, below));
            p.hline(r.x_range().shrink(4.0), r.top() + 1.0, egui::Stroke::new(2.0, accent));
        }
        if let Some((from, to)) = moved {
            move_tab(&mut self.tabs, from, to);
        }
        if let Some(i) = shut {
            self.close_tab(i);
        } else if let Some(i) = pick {
            self.select_sprite(i);
        }
        if let Some(i) = rename {
            self.sprite_dialog = Some(SpriteDialog::Rename { i, name: self.project.sprites[i].name.clone() });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_raeumt_auf_und_nimmt_den_aktiven_auf() {
        let mut t = vec![0, 5, 0, 2];
        sync(&mut t, 3, 1);
        assert_eq!(t, vec![0, 2, 1]);
    }

    #[test]
    fn einfuegen_und_loeschen_verschieben_mit() {
        let mut t = vec![0, 1, 2];
        on_insert(&mut t, 1);
        assert_eq!(t, vec![0, 2, 3]);
        on_remove(&mut t, 2);
        assert_eq!(t, vec![0, 2]);
    }

    #[test]
    fn schliessen_waehlt_den_nachbarn() {
        let mut t = vec![0, 1, 2];
        assert_eq!(close(&mut t, 1, 1), Some(2));
        assert_eq!(t, vec![0, 2]);
        assert_eq!(close(&mut t, 2, 2), Some(0), "am Ende der linke");
        assert_eq!(close(&mut t, 0, 0), None, "letzter bleibt");
        assert_eq!(t, vec![0]);
    }

    #[test]
    fn anderen_schliessen_laesst_den_aktiven() {
        let mut t = vec![0, 1, 2];
        assert_eq!(close(&mut t, 0, 2), None);
        assert_eq!(t, vec![1, 2]);
    }

    #[test]
    fn verschieben() {
        let mut t = vec![10, 11, 12];
        move_tab(&mut t, 2, 0);
        assert_eq!(t, vec![12, 10, 11]);
        move_tab(&mut t, 0, 3);
        assert_eq!(t, vec![10, 11, 12]);
        move_tab(&mut t, 1, 2);
        assert_eq!(t, vec![10, 11, 12], "vor den eigenen Nachbarn = bleibt");
    }

    #[test]
    fn ablegen_nach_mausposition() {
        let r = |x: f32| egui::Rect::from_min_size(egui::pos2(x, 0.0), egui::vec2(50.0, 20.0));
        let rects = [r(0.0), r(52.0), r(104.0)];
        assert_eq!(insert_at(&rects, -5.0), 0);
        assert_eq!(insert_at(&rects, 30.0), 1, "rechte Hälfte des ersten = dahinter");
        assert_eq!(insert_at(&rects, 90.0), 2);
        assert_eq!(insert_at(&rects, 500.0), 3, "ganz hinten");
        let mut t = vec![10, 11, 12];
        move_tab(&mut t, 0, insert_at(&rects, 90.0));
        assert_eq!(t, vec![11, 10, 12], "auf den rechten Nachbarn ziehen tauscht");
    }

    #[test]
    fn weiterschalten_rundum() {
        assert_eq!(step(&[3, 1, 2], 2, 1), Some(3));
        assert_eq!(step(&[3, 1, 2], 3, -1), Some(2));
        assert_eq!(step(&[], 0, 1), None);
    }
}
