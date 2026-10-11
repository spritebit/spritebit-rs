//! Schattieren — Options-Zeile, Farbreihe wählen und der Pinsel selbst.
//! Die Rechnung steht in `spritebit_core::shade`, wie im Web (js/shade.js).
//!
//! Die Farbreihe wählt man wie in Aseprite in der Palette: solange das
//! Werkzeug aktiv ist, nimmt ein Klick auf eine Farbe sie in die Reihe auf
//! oder wieder heraus (statt die Malfarbe zu wählen). Linksklick malt die
//! Reihe von hell nach dunkel, Rechtsklick andersherum — einen Knopf zum
//! Umdrehen braucht es darum hier nicht (im Web nur für Finger).

use eframe::egui::{self, Color32, Sense, Stroke, Vec2};
use spritebit_core::shade::{self, ramp_levels, soft_level};
use spritebit_core::{palops, selection::rgb_of, tools, Px, Rgb};

use crate::i18n::{tr, trf};
use crate::tools_ui::Tool;
use crate::SpritebitApp;

impl SpritebitApp {
    /// Klick auf ein Farbfeld. Beim Schattieren: Farbe in die Reihe bzw.
    /// heraus; sonst wird sie die Malfarbe.
    pub(crate) fn pick_swatch(&mut self, v: Px) {
        if self.tool == Tool::Shade && v != 0 {
            let pal = self.project.current_palette();
            if let Some(c) = rgb_of(v, &pal, &self.sprite().free) {
                self.toggle_shade(c);
            }
            return;
        }
        self.color = v;
    }

    /// Farbe aus einer nur angezeigten Palette (Bibliothek) geklickt.
    /// Gibt `true` zurück, wenn das Schattieren den Klick genommen hat.
    pub(crate) fn pick_shade_rgb(&mut self, c: Rgb) -> bool {
        if self.tool != Tool::Shade {
            return false;
        }
        self.toggle_shade(c);
        true
    }

    fn toggle_shade(&mut self, c: Rgb) {
        self.shade_ramp = shade::toggle_ramp(&self.shade_ramp, c);
        self.save_view();
    }

    /// Ist das Farbfeld markiert? Beim Schattieren: steht es in der Reihe;
    /// sonst: ist es die Malfarbe.
    pub(crate) fn swatch_on(&self, v: Px, c: Option<Rgb>) -> bool {
        if self.tool == Tool::Shade {
            c.is_some_and(|c| self.shade_ramp.contains(&c))
        } else {
            self.color == v
        }
    }

    /// Options-Zeile: die Reihe in Mal-Reihenfolge (Klick nimmt eine Farbe
    /// heraus), „Leeren“ und „Verlauf“.
    pub(crate) fn shade_bar(&mut self, ui: &mut egui::Ui) {
        ui.label(tr("Farbreihe"));
        if self.shade_ramp.is_empty() {
            ui.weak(tr("Aktuelle Farbe — oder Farben in der Palette anklicken"));
        }
        let mut remove = None;
        // Linksklick beginnt mit der hellsten: links steht, was ein leerer Pixel bekommt.
        for &c in self.shade_ramp.iter().rev() {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
            ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(c[0], c[1], c[2]));
            let edge = if resp.hovered() { ui.visuals().strong_text_color() } else { ui.visuals().weak_text_color() };
            ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
            if resp.clicked() {
                remove = Some(c);
            }
            resp.on_hover_text(trf("{hex} — klicken nimmt die Farbe aus der Reihe", &[("hex", &crate::palette_ui::hex(c))]));
        }
        if let Some(c) = remove {
            self.toggle_shade(c);
        }
        if ui.add_enabled(!self.shade_ramp.is_empty(), egui::Button::new(tr("Leeren"))).on_hover_text(tr("Farbreihe leeren")).clicked() {
            self.shade_ramp.clear();
            self.save_view();
        }
        // Aus der aktuellen Farbe passende Töne erzeugen (shade::make_shades).
        if ui
            .button(tr("Abstufungen"))
            .on_hover_text(tr("Aus der aktuellen Farbe passende dunklere und hellere Töne erzeugen — Schatten etwas kühler, Licht etwas wärmer. Sie kommen in die Palette und werden die Farbreihe."))
            .clicked()
        {
            self.make_shade_ramp();
        }
        egui::ComboBox::from_id_salt("shade-gen")
            .width(44.0)
            .selected_text((self.shade_gen * 2 + 1).to_string())
            .show_ui(ui, |ui| {
                for n in 1..=4u32 {
                    ui.selectable_value(&mut self.shade_gen, n, (n * 2 + 1).to_string());
                }
            })
            .response
            .on_hover_text(tr("Wie viele Töne insgesamt"));
        if ui
            .button(tr("Anordnen"))
            .on_hover_text(tr("Palette nach Farbstufen ordnen — die Töne jeder Farbe stehen dann nebeneinander, von dunkel nach hell. Das Bild bleibt gleich."))
            .clicked()
        {
            let pal = self.project.current_palette();
            if !self.reorder_colors(&palops::shade_order(&pal.colors)) {
                self.hint = Some(tr("Schon nach Farbstufen geordnet.").into());
            }
        }
        if ui
            .selectable_label(self.shade_soft, tr("Verlauf"))
            .on_hover_text(tr("Verlauf: runder Pinsel, in der Mitte mehrere Stufen auf einmal, zum Rand hin weniger — braucht eine Größe ab 2"))
            .clicked()
        {
            self.shade_soft = !self.shade_soft;
            self.save_view();
        }
    }

    /// Ein Tupfer an (x, y): jeder Pixel unter dem Pinsel geht so viele
    /// Stufen weiter, wie er in diesem Strich noch nicht gegangen ist — mit
    /// Verlauf in der Mitte mehr als am Rand.
    pub(crate) fn shade_at(&mut self, (x, y): (i64, i64)) {
        let Some(mut s) = self.shade.take() else { return };
        let size = self.size;
        let soft = self.shade_soft && size > 1;
        let levels = ramp_levels(&self.shade_ramp);
        let st = tools::stamp(x, y, size);
        let (mx, my) = ((st.x0 + st.x1) as f64 / 2.0, (st.y0 + st.y1) as f64 / 2.0);
        let (w, h) = (self.sprite().width, self.sprite().height);
        let (mirror_x, mirror_y) = (self.mirror_x, self.mirror_y);
        let pal = self.project.current_palette();
        let sp = self.project.sprite_mut();
        let mut free = std::mem::take(&mut sp.free);
        let mut changed = false;
        for py in st.y0..=st.y1 {
            for px in st.x0..=st.x1 {
                let mut want = if soft { soft_level(px as f64 - mx, py as f64 - my, size, levels) } else { 1 };
                if want == 0 {
                    continue;
                }
                if s.repeat {
                    want = 1;
                }
                for (gy, gx, _) in tools::mirror_spans(vec![(py, px, px)], w, h, mirror_x, mirror_y) {
                    if gx < 0 || gy < 0 || gx >= w as i64 || gy >= h as i64 {
                        continue;
                    }
                    let have = s.done.get(&(gx, gy)).copied().unwrap_or(0);
                    if have >= want {
                        continue;
                    }
                    s.done.insert((gx, gy), want);
                    let img = sp.active();
                    let old = img.get(gx as u32, gy as u32);
                    let mut v = old;
                    for _ in have..want {
                        v = s.shader.step(v, &pal, &mut free);
                    }
                    if v != old {
                        img.set(gx as u32, gy as u32, v);
                        changed = true;
                    }
                }
            }
        }
        sp.free = free;
        s.changed |= changed;
        self.shade = Some(s);
        if changed {
            self.changed();
        }
    }

    /// „Abstufungen“: Töne aus der aktuellen Farbe erzeugen, fehlende in die
    /// Palette, alle zusammen als Farbreihe.
    fn make_shade_ramp(&mut self) {
        let Some(base) = self.current_rgb() else {
            self.hint = Some(tr("Wähle zuerst eine Farbe — aus ihr werden die Abstufungen erzeugt.").into());
            return;
        };
        let shades = shade::make_shades(base, self.shade_gen);
        let added = self.add_colors_to_palette(&shades);
        self.hint = Some(trf("{n} Töne erzeugt und als Farbreihe gewählt — {added} davon neu in der Palette.", &[("n", &shades.len()), ("added", &added)]));
        self.shade_ramp = shades;
        self.save_view();
    }

    /// Statuszeile nach einem Strich, der nichts geändert hat.
    pub(crate) fn shade_nothing(&self) -> String {
        if self.shade_ramp.is_empty() {
            tr("Schattieren hat nichts geändert: in der Palette gibt es keinen dunkleren (bzw. helleren) Ton dieser Farbe. Wähle die Farbreihe selbst — klicke die Farben in der Palette an.").into()
        } else {
            tr("Schattieren hat nichts geändert: Pixel in Farben, die nicht in der Farbreihe stehen, bleiben, wie sie sind — und an der dunkelsten (bzw. hellsten) Farbe geht es nicht weiter.").into()
        }
    }
}
