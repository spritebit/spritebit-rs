//! Farben in der linken Leiste: aktuelle Farbe, Palette wählen und
//! bearbeiten, freie Farben.
//!
//! * Klick auf ein Feld wählt die Farbe; Feld 0 ist Transparent (Radierer).
//! * „Aktuelle Farbe" hat einen Farbwähler: liegt die gewählte Farbe in der
//!   Palette, wird es deren Nummer, sonst eine freie Farbe des Sprites.
//! * „Bearbeiten" ändert die Farbe der Palette selbst. Eingebaute Paletten
//!   bleiben unverändert — der Sprite bekommt dann eine Kopie, wie in der
//!   Web-Version.
//! * Alt+Klick auf der Fläche nimmt die Farbe darunter (Pipette).

use eframe::egui::{self, Color32, Sense, Stroke, Vec2};
use spritebit_core::{builtin, palette::MAX_COLORS, selection::rgb_of, Palette, Px, Rgb, FREE_BASE};

use crate::SpritebitApp;
use crate::i18n::{tr, trf};

/// Schachbrett für Transparent.
const CHECKER: [[u8; 3]; 2] = [[0x20, 0x20, 0x2c], [0x2a, 0x2a, 0x38]];

fn swatch(ui: &mut egui::Ui, rgb: Option<Rgb>, size: f32, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let p = ui.painter();
    match rgb {
        Some([r, g, b]) => {
            p.rect_filled(rect, 3.0, Color32::from_rgb(r, g, b));
        }
        None => {
            let half = rect.size() / 2.0;
            for (qx, qy) in [(0u8, 0u8), (1, 0), (0, 1), (1, 1)] {
                let [r, g, b] = CHECKER[((qx + qy) % 2) as usize];
                let min = rect.min + Vec2::new(qx as f32 * half.x, qy as f32 * half.y);
                p.rect_filled(egui::Rect::from_min_size(min, half), 0.0, Color32::from_rgb(r, g, b));
            }
        }
    }
    if active {
        p.rect_stroke(rect.expand(2.0), 4.0, Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Outside);
    }
    resp
}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

impl SpritebitApp {
    /// RGB der aktuellen Farbe (`None` = transparent).
    pub(crate) fn current_rgb(&self) -> Option<Rgb> {
        let sp = self.project.sprite();
        rgb_of(self.color, &self.project.current_palette(), &sp.free)
    }

    /// Farbe setzen: Palettenfarbe, sonst freie Farbe des Sprites.
    pub(crate) fn set_rgb(&mut self, rgb: Rgb) {
        let pal = self.project.current_palette();
        self.color = match pal.colors.iter().position(|&c| c == rgb) {
            Some(i) => i as Px + 1,
            None => self.project.sprite_mut().free_color(rgb),
        };
    }

    /// Freie Farben gehören zu einem Sprite — nach einem Wechsel kann die
    /// Nummer ins Leere zeigen.
    pub(crate) fn clamp_color(&mut self) {
        let sp = self.project.sprite();
        let pal = self.project.current_palette();
        let ok = if self.color >= FREE_BASE {
            ((self.color - FREE_BASE) as usize) < sp.free.len()
        } else {
            (self.color as usize) <= pal.len()
        };
        if !ok {
            self.color = 1;
        }
    }

    /// Pipette: oberste sichtbare Ebene mit einem Pixel an (x, y).
    pub(crate) fn pick_color(&mut self, x: i64, y: i64) {
        let sp = self.project.sprite();
        if x < 0 || y < 0 || x >= sp.width as i64 || y >= sp.height as i64 {
            return;
        }
        let v = (0..sp.layers.len())
            .rev()
            .filter(|&l| sp.layers[l].visible)
            .map(|l| sp.cel(sp.frame, l).get(x as u32, y as u32))
            .find(|&v| v != 0)
            .unwrap_or(0);
        self.color = v;
    }

    /// Eigene, bearbeitbare Palette des aktuellen Sprites. Eine eingebaute
    /// wird dafür kopiert und dem Sprite zugewiesen.
    fn own_palette(&mut self) -> usize {
        let name = self.project.sprite().palette.clone();
        if let Some(i) = self.project.palettes.iter().position(|p| p.name == name) {
            return i;
        }
        let base = self.project.palette(&name);
        let mut copy = format!("{name}_kopie");
        let mut n = 2;
        while self.project.palettes.iter().any(|p| p.name == copy) || builtin::builtin(&copy).is_some() {
            copy = format!("{name}_kopie{n}");
            n += 1;
        }
        self.project.palettes.push(Palette::new(copy.clone(), base.colors));
        self.project.sprite_mut().palette = copy;
        self.project.palettes.len() - 1
    }

    pub(crate) fn colors_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.project.current_palette();

        // Aktuelle Farbe
        ui.horizontal(|ui| {
            let cur = self.current_rgb();
            swatch(ui, cur, 30.0, false);
            ui.vertical(|ui| {
                let label = match (self.color, cur) {
                    (0, _) => tr("Transparent").to_string(),
                    (c, Some(rgb)) if c >= FREE_BASE => trf("Freie Farbe {hex}", &[("hex", &hex(rgb))]),
                    (c, Some(rgb)) => trf("Nr. {c} · {hex}", &[("c", &c), ("hex", &hex(rgb))]),
                    (c, None) => trf("Nr. {c}", &[("c", &c)]),
                };
                ui.label(label);
                let mut rgb = cur.unwrap_or([0, 0, 0]);
                if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).on_hover_text(tr("Freie Farbe wählen")).changed() {
                    self.set_rgb(rgb);
                }
            });
        });
        ui.add_space(6.0);

        // Palette wählen
        let mut names: Vec<String> = builtin::BUILTIN.iter().map(|(n, _)| n.to_string()).collect();
        names.extend(self.project.palettes.iter().map(|p| p.name.clone()));
        let mut chosen = self.project.sprite().palette.clone();
        egui::ComboBox::from_id_salt("palette")
            .selected_text(&chosen)
            .width(150.0)
            .show_ui(ui, |ui| {
                for n in &names {
                    ui.selectable_value(&mut chosen, n.clone(), n);
                }
            });
        if chosen != self.project.sprite().palette {
            // Die Nummern bleiben, die Farben kommen aus der neuen Palette.
            let cur = self.project.current;
            self.histories[cur].record(&self.project.sprites[cur]);
            self.project.sprite_mut().palette = chosen;
            self.clamp_color();
            self.changed();
        }
        ui.add_space(4.0);

        // Farbfelder
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            for i in 0..=palette.len() as Px {
                let resp = swatch(ui, palette.get(i), 22.0, self.color == i);
                if resp.clicked() {
                    self.color = i;
                }
                resp.on_hover_text(match palette.get(i) {
                    None => tr("0 · Transparent (Radierer)").to_string(),
                    Some(c) => format!("{i} · {}", hex(c)),
                });
            }
        });

        // Bearbeiten: Farbe der Palette ändern, Farben dazu/weg
        ui.add_space(4.0);
        ui.checkbox(&mut self.palette_edit, tr("Palette bearbeiten"));
        if self.palette_edit {
            let builtin_now = builtin::builtin(&palette.name).is_some()
                && !self.project.palettes.iter().any(|p| p.name == palette.name);
            if builtin_now {
                ui.weak(tr("Eingebaute Palette — Änderungen gehen in eine Kopie."));
            }
            if (1..=palette.len() as Px).contains(&self.color) {
                let mut rgb = palette.get(self.color).unwrap_or([0, 0, 0]);
                ui.horizontal(|ui| {
                    ui.label(trf("Farbe {n}", &[("n", &self.color)]));
                    if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
                        let i = self.own_palette();
                        self.project.palettes[i].colors[self.color as usize - 1] = rgb;
                        self.changed();
                    }
                });
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(palette.len() < MAX_COLORS, egui::Button::new(tr("+ Farbe"))).clicked() {
                    let i = self.own_palette();
                    let last = self.project.palettes[i].colors.last().copied().unwrap_or([0, 0, 0]);
                    self.project.palettes[i].colors.push(last);
                    self.color = self.project.palettes[i].colors.len() as Px;
                    self.changed();
                }
                if ui.add_enabled(palette.len() > 1, egui::Button::new(tr("− Letzte"))).clicked() {
                    let i = self.own_palette();
                    self.project.palettes[i].colors.pop();
                    self.clamp_color();
                    self.changed();
                }
            });
        }
        ui.weak(trf("Palette: {name}", &[("name", &palette.name)]));
    }
}
