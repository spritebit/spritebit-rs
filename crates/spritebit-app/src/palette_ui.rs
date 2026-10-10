//! Farben und Paletten — wie in der Web-Version.
//!
//! Linke Leiste, „Farben“ (die Farbzeile):
//! * aktuelle Farbe mit Farbwähler: liegt die Farbe in der Palette, wird es
//!   deren Nummer, sonst eine freie Farbe des Sprites
//! * Klick wählt eine Farbe; Ziehen einer Farbe auf einen anderen Platz
//!   sortiert um — das Bild bleibt gleich, die Nummern wandern mit
//! * „Nach Farbstufen“ ordnet automatisch (Grau, dann je Farbton, dunkel → hell)
//! * freie Farben des Bildes lassen sich in die Palette übernehmen
//! * „Palette bearbeiten“: die Farbe der gewählten Nummer ändern
//!
//! Rechte Leiste, „Palette“ (die Bibliothek): suchen, filtern, eine
//! Palette ANSEHEN; zugewiesen wird erst per Knopf — „Für Sprite nutzen“
//! (das Bild sieht gleich aus) oder „Sprite umfärben“ (die Nummern
//! bleiben). Dazu neue Palette, Kopie, bearbeiten, löschen und Bild » Palette.
//!
//! Eingebaute Paletten bleiben unverändert — wer sie ändert, bekommt eine
//! Kopie. Alle Änderungen an Paletten sind Undo-Schritte.
//!
//! Alt+Klick auf der Fläche nimmt die Farbe darunter (Pipette).

use std::collections::HashMap;

use eframe::egui::{self, Color32, Sense, Stroke, Vec2};
use spritebit_core::palette::{parse_hex, MAX_COLORS};
use spritebit_core::{builtin, palops, selection::rgb_of, Palette, Px, Rgb, FREE_BASE};

use crate::i18n::{tr, trf};
use crate::SpritebitApp;

/// Schachbrett für Transparent.
const CHECKER: [[u8; 3]; 2] = [[0x20, 0x20, 0x2c], [0x2a, 0x2a, 0x38]];

/// Paletten einer Gruppe der Bibliothek: Name und Farben.
type PalList = Vec<(String, Vec<Rgb>)>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PalFilter {
    All,
    Builtin,
    Custom,
}

/// Dialog „Neue Palette“ / „Palette bearbeiten“.
pub(crate) struct PalModal {
    /// Name der bearbeiteten eigenen Palette; `None` = neue Palette.
    pub edit: Option<String>,
    pub name: String,
    pub colors: Vec<Rgb>,
    /// Name je Farbe (leer = „Farbe 3“), parallel zu `colors`.
    pub names: Vec<String>,
}

impl PalModal {
    fn new(edit: Option<String>, name: String, pal: &Palette) -> Self {
        let names = (1..=pal.len() as Px).map(|i| pal.name_of(i).unwrap_or_default().to_string()).collect();
        PalModal { edit, name, colors: pal.colors.clone(), names }
    }
}

/// Dialog „Bild » Palette“.
pub(crate) struct ReduceModal {
    pub colors: Vec<(Rgb, usize)>,
    pub count: usize,
    pub before: Option<egui::TextureHandle>,
    pub after: Option<(usize, egui::TextureHandle)>,
}

/// Was im Rechtsklick-Menü eines Farbfelds gewählt wurde.
#[derive(Clone, Copy)]
enum SwatchAction {
    /// Das „+“-Feld: neue Farbe anhängen und gleich ändern.
    Add,
    Duplicate(Px),
    Copy(Px),
    Paste(Px),
    Edit(Px),
    Remove(Px),
}

#[derive(Default)]
pub(crate) struct PalState {
    /// Nummer, deren Farbwähler gerade am Feld offen ist.
    pub edit_idx: Option<Px>,
    /// Für den offenen Farbwähler schon ein Undo-Schritt angelegt.
    pub picking: bool,
    /// „Farbe kopieren“ — für „Farbe einfügen“.
    pub copied: Option<Rgb>,
    pub preview: Option<String>,
    pub search: String,
    pub filter: Option<PalFilter>,
    /// Gerade gezogene Farbe (Nummer).
    pub drag_from: Option<Px>,
    pub modal: Option<PalModal>,
    pub reduce: Option<ReduceModal>,
    pub delete: Option<String>,
}

fn swatch(ui: &mut egui::Ui, rgb: Option<Rgb>, size: f32, active: bool, sense: Sense) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), sense);
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

/// Farbstreifen einer Palette (für die Liste).
fn strip(ui: &mut egui::Ui, colors: &[Rgb], width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 10.0), Sense::hover());
    let n = colors.len().clamp(1, 32);
    let step = colors.len().max(1) as f32 / n as f32;
    let w = width / n as f32;
    for k in 0..n {
        let [r, g, b] = colors.get((k as f32 * step) as usize).copied().unwrap_or([0, 0, 0]);
        let x = rect.min.x + k as f32 * w;
        ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, rect.min.y), Vec2::new(w + 0.5, 10.0)), 0.0, Color32::from_rgb(r, g, b));
    }
}

pub(crate) fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Hex-Eingabe für `rgb` (`None` = transparent, Feld leer). Gibt die
/// eingegebene Farbe: sofort bei sechs Stellen, sonst beim Verlassen
/// (`#rgb` geht auch, das `#` darf fehlen).
fn hex_field(ui: &mut egui::Ui, id: &str, rgb: Option<Rgb>) -> Option<Rgb> {
    let id = ui.id().with(id);
    let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| rgb.map(hex).unwrap_or_default());
    let mut edit =
        egui::TextEdit::singleline(&mut text).id(id).desired_width(64.0).char_limit(7).font(egui::TextStyle::Monospace).hint_text("#rrggbb").show(ui);
    // Beim Hineinklicken alles markieren — gleich drüberschreiben
    if edit.response.gained_focus() {
        edit.state.cursor.set_char_range(Some(egui::text::CCursorRange::select_all(&edit.galley)));
        edit.state.store(ui.ctx(), id);
    }
    let resp = edit.response.response;
    let digits = text.trim().trim_start_matches('#').to_string();
    let parsed = parse_hex(&format!("#{digits}")).filter(|c| Some(*c) != rgb);
    let out = if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text));
        parsed.filter(|_| resp.changed() && digits.len() == 6)
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
        parsed.filter(|_| resp.lost_focus())
    };
    resp.on_hover_text(tr("Hex-Wert eingeben, z. B. #6fa211 — liegt die Farbe schon in der Palette, wird sie gewählt"));
    out
}

/// Name für eine eigene Palette: klein, nur a–z, 0–9, _ und -.
fn clean_name(raw: &str) -> String {
    raw.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
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
        let ok = if self.color >= FREE_BASE { ((self.color - FREE_BASE) as usize) < sp.free.len() } else { (self.color as usize) <= pal.len() };
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
        let v =
            (0..sp.layers.len()).rev().filter(|&l| sp.layers[l].visible).map(|l| sp.cel(sp.frame, l).get(x as u32, y as u32)).find(|&v| v != 0).unwrap_or(0);
        self.color = v;
    }

    /// Undo-Schritt, der die Paletten mitsichert.
    pub(crate) fn pal_step(&mut self) {
        self.commit_float();
        let cur = self.project.current;
        self.histories[cur].record_with_palettes(&self.project.sprites[cur], &self.project.palettes);
    }

    /// Eigene, bearbeitbare Palette des aktuellen Sprites. Eine eingebaute
    /// wird dafür kopiert und dem Sprite zugewiesen.
    fn own_palette(&mut self) -> usize {
        let name = self.project.sprite().palette.clone();
        if let Some(i) = self.project.palettes.iter().position(|p| p.name == name) {
            return i;
        }
        let base = self.project.palette(&name);
        let copy = self.project.unique_palette_name(&format!("{name}_kopie"));
        self.project.palettes.push(Palette::new(copy.clone(), base.colors));
        self.project.sprite_mut().palette = copy;
        self.project.palettes.len() - 1
    }

    /// Farben des aktuellen Sprites neu anordnen (`order`: alte Nummern in
    /// neuer Reihenfolge). Eingebaute — oder mit anderen Sprites geteilte —
    /// Paletten werden dafür für diesen Sprite kopiert.
    pub(crate) fn reorder_colors(&mut self, order: &[usize]) -> bool {
        let pal = self.project.current_palette();
        let cur = self.project.current;
        let shared = self.project.sprites.iter().enumerate().any(|(i, s)| i != cur && s.palette == pal.name);
        self.pal_step();
        let Some(colors) = palops::reorder(&pal.colors, order, &mut [&mut self.project.sprites[cur]]) else {
            self.histories[cur].drop_last();
            return false;
        };
        if self.color > 0 && (self.color as usize) <= order.len() {
            self.color = order.iter().position(|&o| o == self.color as usize).map_or(self.color, |k| k as Px + 1);
        }
        // Namen wandern mit ihrer Farbe.
        let names = order.iter().enumerate().filter_map(|(k, &o)| Some((k as Px + 1, pal.name_of(o as Px)?.to_string()))).collect();
        if self.project.is_custom(&pal.name) && !shared {
            let i = self.project.palettes.iter().position(|p| p.name == pal.name).expect("eigene Palette");
            self.project.palettes[i].colors = colors;
            self.project.palettes[i].names = names;
        } else {
            let name = self.project.unique_palette_name(&format!("{}_kopie", pal.name));
            let mut copy = Palette::new(name.clone(), colors);
            copy.names = names;
            self.project.palettes.push(copy);
            self.project.sprite_mut().palette = name.clone();
            self.hint = Some(trf("Umsortiert in der Kopie „{name}“.", &[("name", &name)]));
        }
        self.changed();
        true
    }

    /// Farbe `n` entfernen; die Nummern dahinter rücken auf, das Bild bleibt
    /// gleich (palops::remove_color). Eingebaute — oder mit anderen Sprites
    /// geteilte — Paletten werden dafür für diesen Sprite kopiert.
    fn remove_color(&mut self, n: Px) {
        let pal = self.project.current_palette();
        let cur = self.project.current;
        let shared = self.project.sprites.iter().enumerate().any(|(i, s)| i != cur && s.palette == pal.name);
        self.pal_step();
        let mut new = pal.clone();
        let Some(freed) = palops::remove_color(&mut new, n as usize, &mut [&mut self.project.sprites[cur]]) else {
            self.histories[cur].drop_last();
            return;
        };
        if self.project.is_custom(&pal.name) && !shared {
            let i = self.project.palettes.iter().position(|p| p.name == pal.name).expect("eigene Palette");
            self.project.palettes[i] = new;
            self.hint = Some(if freed > 0 {
                trf("Farbe {n} entfernt — {k} Pixel bleiben als freie Farbe.", &[("n", &n), ("k", &freed)])
            } else {
                trf("Farbe {n} entfernt.", &[("n", &n)])
            });
        } else {
            new.name = self.project.unique_palette_name(&format!("{}_kopie", pal.name));
            self.project.sprite_mut().palette = new.name.clone();
            self.hint = Some(trf("Farbe {n} entfernt — in der Kopie „{name}“.", &[("n", &n), ("name", &new.name)]));
            self.project.palettes.push(new);
        }
        if self.color == n {
            self.color = n.saturating_sub(1).max(1);
        } else if self.color > n && self.color < FREE_BASE {
            self.color -= 1;
        }
        self.clamp_color();
        self.changed();
    }

    /// Palette `name` dem Sprite zuweisen. `keep_look`: das Bild sieht gleich
    /// aus; sonst bleiben die Nummern und die Farben kommen aus der neuen.
    pub(crate) fn assign_palette(&mut self, name: &str, keep_look: bool) {
        self.commit_float();
        let old = self.project.current_palette();
        let new = self.project.palette(name);
        let cur = self.project.current;
        self.histories[cur].record(&self.project.sprites[cur]);
        let kept = if keep_look { palops::assign_keep_look(self.project.sprite_mut(), &old, &new) } else { 0 };
        self.project.sprite_mut().palette = name.to_string();
        self.pal.preview = None;
        self.clamp_color();
        self.hint = Some(if !keep_look {
            trf("Sprite umgefärbt mit „{name}“.", &[("name", &name)])
        } else if kept > 0 {
            trf("„{name}“ zugewiesen — {n} Pixel bleiben als freie Farbe.", &[("name", &name), ("n", &kept)])
        } else {
            trf("„{name}“ zugewiesen.", &[("name", &name)])
        });
        self.changed();
    }

    /// Die aktuelle (freie) Farbe als neue Nummer in die Palette.
    fn add_current_to_palette(&mut self) {
        let Some(rgb) = self.current_rgb() else { return };
        if self.project.current_palette().len() >= MAX_COLORS {
            self.hint = Some(trf("Die Palette ist voll ({n} Farben).", &[("n", &MAX_COLORS)]));
            return;
        }
        self.pal_step();
        let i = self.own_palette();
        let cur = self.project.current;
        let colors = self.project.palettes[i].colors.clone();
        match palops::add_color(&mut self.project.sprites[cur], &colors, rgb) {
            Some(colors) => {
                self.color = colors.len() as Px;
                self.project.palettes[i].colors = colors;
                self.hint = Some(trf("{hex} ist jetzt Nr. {n} der Palette.", &[("hex", &hex(rgb)), ("n", &self.color)]));
                self.changed();
            }
            None => self.histories[cur].drop_last(),
        }
    }

    fn add_free_to_palette(&mut self) {
        let pal = self.project.current_palette();
        self.pal_step();
        let cur = self.project.current;
        match palops::add_free_colors(&mut self.project.sprites[cur], &pal.colors) {
            Ok((_, 0)) => {
                self.histories[cur].drop_last();
            }
            Ok((colors, n)) => {
                let keep = self.current_rgb();
                let i = self.own_palette();
                self.project.palettes[i].colors = colors;
                if let Some(c) = keep {
                    self.set_rgb(c);
                }
                let name = self.project.sprite().palette.clone();
                self.hint = Some(trf("{n} Bildfarben in „{name}“ aufgenommen.", &[("n", &n), ("name", &name)]));
                self.changed();
            }
            Err(n) => {
                self.histories[cur].drop_last();
                self.hint = Some(trf("{n} Farben passen nicht mehr in die Palette — „Bild » Palette“ fasst sie zusammen.", &[("n", &n)]));
            }
        }
    }

    // ── Linke Leiste ────────────────────────────────────────────────
    pub(crate) fn colors_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.project.current_palette();

        // Aktuelle Farbe
        ui.horizontal(|ui| {
            let cur = self.current_rgb();
            swatch(ui, cur, 30.0, false, Sense::hover());
            ui.vertical(|ui| {
                let label = match (self.color, cur) {
                    (0, _) => tr("Transparent").to_string(),
                    (c, Some(rgb)) if c >= FREE_BASE => trf("Freie Farbe {hex}", &[("hex", &hex(rgb))]),
                    (c, Some(rgb)) => match palette.name_of(c) {
                        Some(name) => trf("Nr. {c} · {name} · {hex}", &[("c", &c), ("name", &name), ("hex", &hex(rgb))]),
                        None => trf("Nr. {c} · {hex}", &[("c", &c), ("hex", &hex(rgb))]),
                    },
                    (c, None) => trf("Nr. {c}", &[("c", &c)]),
                };
                ui.label(label);
                ui.horizontal(|ui| {
                    let mut rgb = cur.unwrap_or([0, 0, 0]);
                    if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).on_hover_text(tr("Freie Farbe wählen")).changed() {
                        self.set_rgb(rgb);
                    }
                    if ui
                        .selectable_label(self.view.spotlight, tr("Zeigen"))
                        .on_hover_text(tr("Zeigt, wo die aktuelle Farbe im Bild vorkommt — alles andere wird abgedunkelt"))
                        .clicked()
                    {
                        self.view.spotlight = !self.view.spotlight;
                    }
                });
            });
        });
        // Hex eingeben — findet die Farbe in der Palette oder wird eine freie
        ui.horizontal(|ui| {
            if let Some(rgb) = hex_field(ui, "cur-hex", self.current_rgb()) {
                self.set_rgb(rgb);
            }
            if self.color >= FREE_BASE
                && ui
                    .add_enabled(palette.len() < MAX_COLORS, egui::Button::new(tr("+ In Palette")).small())
                    .on_hover_text(tr("Diese Farbe als neue Nummer in die Palette aufnehmen"))
                    .clicked()
            {
                self.add_current_to_palette();
            }
        });
        ui.add_space(6.0);

        // Farbzeile: Klick wählt, Doppelklick ändert, Rechtsklick öffnet das
        // Menü, Ziehen sortiert um. Das „+“ am Ende hängt eine Farbe an.
        let mut drop_on: Option<Px> = None;
        let dragging = self.pal.drag_from;
        let mut action: Option<SwatchAction> = None;
        let mut edit_anchor: Option<egui::Response> = None;
        let copied = self.pal.copied;
        let last = palette.len() as Px;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            for i in 0..=palette.len() as Px {
                let sense = if i == 0 { Sense::click() } else { Sense::click_and_drag() };
                let resp = swatch(ui, palette.get(i), 22.0, self.color == i, sense);
                if resp.clicked() {
                    self.color = i;
                }
                if i > 0 && resp.double_clicked() {
                    action = Some(SwatchAction::Edit(i));
                }
                if i > 0 {
                    resp.context_menu(|ui| {
                        if ui.button(tr("Duplizieren")).clicked() {
                            action = Some(SwatchAction::Duplicate(i));
                        }
                        if ui.button(tr("Farbe kopieren")).clicked() {
                            action = Some(SwatchAction::Copy(i));
                        }
                        let paste = match copied {
                            Some(c) => trf("{hex} einfügen", &[("hex", &hex(c))]),
                            None => tr("Farbe einfügen").to_string(),
                        };
                        if ui.add_enabled(copied.is_some(), egui::Button::new(paste)).clicked() {
                            action = Some(SwatchAction::Paste(i));
                        }
                        if ui.button(tr("Farbe ändern …")).clicked() {
                            action = Some(SwatchAction::Edit(i));
                        }
                        ui.separator();
                        if ui.add_enabled(last > 1, egui::Button::new(tr("Entfernen"))).clicked() {
                            action = Some(SwatchAction::Remove(i));
                        }
                    });
                }
                if self.pal.edit_idx == Some(i) {
                    edit_anchor = Some(resp.clone());
                }
                if i > 0 && resp.drag_started() {
                    self.pal.drag_from = Some(i);
                }
                if i > 0 && dragging.is_some() {
                    let over = ui.input(|inp| inp.pointer.interact_pos()).is_some_and(|p| resp.rect.expand(2.0).contains(p));
                    if over {
                        // Einfügemarke links vom Feld unter dem Zeiger
                        let r = resp.rect;
                        ui.painter()
                            .line_segment([r.left_top() - Vec2::new(3.0, 0.0), r.left_bottom() - Vec2::new(3.0, 0.0)], Stroke::new(2.0, Color32::WHITE));
                        drop_on = Some(i);
                    }
                }
                resp.on_hover_text(match palette.get(i) {
                    None => tr("0 · Transparent (Radierer)").to_string(),
                    Some(c) => {
                        let name = palette.name_of(i).map(|n| format!("{n} · ")).unwrap_or_default();
                        format!("{i} · {name}{}\n{}", hex(c), tr("Doppelklick: ändern · Rechtsklick: duplizieren, kopieren …"))
                    }
                });
            }
            if palette.len() < MAX_COLORS {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                let col = if resp.hovered() { ui.visuals().strong_text_color() } else { ui.visuals().weak_text_color() };
                ui.painter().rect_stroke(rect.shrink(0.5), 3.0, Stroke::new(1.0, col), egui::StrokeKind::Inside);
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "+", egui::FontId::proportional(16.0), col);
                if resp.clicked() {
                    action = Some(SwatchAction::Add);
                }
                resp.on_hover_text(tr("Neue Farbe hinzufügen"));
            }
        });
        if let Some(a) = action {
            self.swatch_action(ui.ctx(), a);
        }
        self.swatch_editor(ui, edit_anchor);
        if dragging.is_some() && ui.input(|i| i.pointer.any_released()) {
            if let (Some(from), Some(to)) = (self.pal.drag_from.take(), drop_on) {
                if from != to {
                    self.reorder_colors(&palops::move_color(palette.len(), from as usize, to as usize));
                }
            }
            self.pal.drag_from = None;
        }
        if ui
            .small_button(tr("Nach Farbstufen"))
            .on_hover_text(tr("Farben automatisch ordnen: Grau, dann je Farbton von dunkel nach hell — das Bild bleibt gleich"))
            .clicked()
            && !self.reorder_colors(&palops::shade_order(&palette.colors))
        {
            self.hint = Some(tr("Schon nach Farbstufen geordnet.").into());
        }

        // Freie Farben im Bild
        let free = palops::used_free_colors(self.project.sprite());
        if !free.is_empty() {
            ui.add_space(4.0);
            ui.weak(trf("Freie Farben im Bild: {n}", &[("n", &free.len())]));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                for c in free.iter().take(64) {
                    let v = self.project.sprite().free.iter().position(|f| f == c).map(|k| FREE_BASE + k as Px);
                    let r = swatch(ui, Some(*c), 16.0, v == Some(self.color), Sense::click());
                    if r.clicked() {
                        if let Some(v) = v {
                            self.color = v;
                        }
                    }
                    r.on_hover_text(hex(*c));
                }
            });
            if ui.small_button(tr("In die Palette übernehmen")).clicked() {
                self.add_free_to_palette();
            }
        }

        ui.weak(trf("Palette: {name}", &[("name", &palette.name)]));
    }

    fn swatch_action(&mut self, ctx: &egui::Context, action: SwatchAction) {
        let palette = self.project.current_palette();
        match action {
            SwatchAction::Add | SwatchAction::Duplicate(_) => {
                if palette.len() >= MAX_COLORS {
                    self.hint = Some(trf("Die Palette ist voll ({n} Farben).", &[("n", &MAX_COLORS)]));
                    return;
                }
                let rgb = match action {
                    SwatchAction::Duplicate(n) => palette.get(n),
                    _ => self.current_rgb().or(palette.colors.last().copied()),
                }
                .unwrap_or([0x88, 0x88, 0x88]);
                self.pal_step();
                let i = self.own_palette();
                self.project.palettes[i].colors.push(rgb);
                let n = self.project.palettes[i].colors.len() as Px;
                self.color = n;
                if matches!(action, SwatchAction::Add) {
                    self.open_swatch_editor(ctx, n);
                }
                self.changed();
            }
            SwatchAction::Copy(n) => {
                if let Some(c) = palette.get(n) {
                    self.pal.copied = Some(c);
                    ctx.copy_text(hex(c));
                }
            }
            SwatchAction::Paste(n) => {
                if let Some(c) = self.pal.copied {
                    self.set_palette_color(n, c);
                }
            }
            SwatchAction::Edit(n) => self.open_swatch_editor(ctx, n),
            SwatchAction::Remove(n) => self.remove_color(n),
        }
    }

    /// Farbe der Nummer `n` setzen — als eigener Undo-Schritt.
    fn set_palette_color(&mut self, n: Px, rgb: Rgb) {
        self.pal_step();
        let i = self.own_palette();
        self.project.palettes[i].colors[n as usize - 1] = rgb;
        self.changed();
    }

    /// Für das Bild der Oberfläche (Test): Dialog mit zwei benannten Farben.
    #[cfg(test)]
    pub(crate) fn open_palette_modal_for_shot(&mut self) {
        let mut pal = self.project.current_palette();
        pal.names.insert(1, "Licht".into());
        pal.names.insert(5, "Kontur".into());
        self.pal.modal = Some(PalModal::new(Some(pal.name.clone()), pal.name.clone(), &pal));
    }

    pub(crate) fn open_swatch_editor(&mut self, ctx: &egui::Context, n: Px) {
        self.pal.edit_idx = Some(n);
        self.pal.picking = false;
        egui::Popup::open_id(ctx, egui::Id::new("pal-swatch-edit"));
    }

    /// Farbwähler direkt am Feld (Doppelklick, „+“, „Farbe ändern …“).
    /// Ein Undo-Schritt je Öffnen, nicht je Zug.
    fn swatch_editor(&mut self, ui: &mut egui::Ui, anchor: Option<egui::Response>) {
        let Some(n) = self.pal.edit_idx else { return };
        let id = egui::Id::new("pal-swatch-edit");
        // Das Feld gibt es erst ab dem nächsten Bild (nach „+“) — dann warten.
        let Some(anchor) = anchor else {
            if !egui::Popup::is_id_open(ui.ctx(), id) {
                self.pal.edit_idx = None;
            }
            return;
        };
        let Some(cur) = self.project.current_palette().get(n) else {
            self.pal.edit_idx = None;
            return;
        };
        let builtin = !self.project.is_custom(&self.project.sprite().palette);
        let mut picked: Option<Rgb> = None;
        let mut typed: Option<Rgb> = None;
        let shown = egui::Popup::from_response(&anchor).id(id).open_memory(None).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.label(trf("Farbe {n}", &[("n", &n)]));
            if builtin {
                ui.weak(tr("Eingebaute Palette — Änderungen gehen in eine Kopie."));
            }
            let mut c = Color32::from_rgb(cur[0], cur[1], cur[2]);
            ui.spacing_mut().slider_width = 220.0;
            if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
                picked = Some([c.r(), c.g(), c.b()]);
            }
            ui.horizontal(|ui| {
                typed = hex_field(ui, "swatch-hex", Some(cur));
                if ui.button(tr("Fertig")).clicked() {
                    ui.close();
                }
            });
        });
        if let Some(rgb) = typed {
            self.set_palette_color(n, rgb);
        } else if let Some(rgb) = picked.filter(|&c| c != cur) {
            if !self.pal.picking {
                self.pal_step();
                self.pal.picking = true;
            }
            let i = self.own_palette();
            self.project.palettes[i].colors[n as usize - 1] = rgb;
            self.changed();
        }
        if shown.is_none() {
            self.pal.edit_idx = None;
            self.pal.picking = false;
        }
    }

    // ── Rechte Leiste: Bibliothek ───────────────────────────────────
    pub(crate) fn palette_library(&mut self, ui: &mut egui::Ui) {
        let current = self.project.sprite().palette.clone();
        let shown = self.pal.preview.clone().unwrap_or_else(|| current.clone());
        ui.add(egui::TextEdit::singleline(&mut self.pal.search).hint_text(tr("Suchen …")).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            let f = self.pal.filter.unwrap_or(PalFilter::All);
            let mut nf = f;
            ui.selectable_value(&mut nf, PalFilter::All, tr("Alle"));
            ui.selectable_value(&mut nf, PalFilter::Builtin, tr("Eingebaut"));
            ui.selectable_value(&mut nf, PalFilter::Custom, tr("Eigene"));
            if nf != f {
                self.pal.filter = Some(nf);
            }
        });
        let q = self.pal.search.trim().to_lowercase();
        let filter = self.pal.filter.unwrap_or(PalFilter::All);
        let mut groups: Vec<(&str, PalList)> = Vec::new();
        if filter != PalFilter::Custom {
            groups.push((tr("Eingebaut"), builtin::BUILTIN.iter().map(|(n, c)| (n.to_string(), c.to_vec())).collect()));
        }
        if filter != PalFilter::Builtin {
            groups.push((tr("Eigene"), self.project.palettes.iter().map(|p| (p.name.clone(), p.colors.clone())).collect()));
        }
        let mut clicked: Option<String> = None;
        egui::ScrollArea::vertical().id_salt("pal-list").max_height(220.0).show(ui, |ui| {
            for (label, items) in groups {
                let items: Vec<_> = items.into_iter().filter(|(n, _)| q.is_empty() || n.to_lowercase().contains(&q)).collect();
                egui::CollapsingHeader::new(format!("{label} ({})", items.len())).id_salt(label).default_open(true).show(ui, |ui| {
                    for (name, colors) in items {
                        let active = name == shown;
                        ui.horizontal(|ui| {
                            let mark = if name == current { "  •" } else { "" };
                            if ui.selectable_label(active, format!("{name}{mark}")).clicked() {
                                clicked = Some(name.clone());
                            }
                            strip(ui, &colors, 60.0);
                            ui.weak(format!("{}", colors.len()));
                        });
                    }
                });
            }
        });
        if let Some(n) = clicked {
            self.pal.preview = if n == current { None } else { Some(n) };
        }

        // Angezeigte Palette
        let pal = self.project.palette(&shown);
        ui.add_space(4.0);
        ui.label(if shown == current {
            trf("„{name}“ — Palette des Sprites", &[("name", &shown)])
        } else {
            trf("Ansicht: „{name}“", &[("name", &shown)])
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(3.0);
            for (i, &c) in pal.colors.iter().enumerate() {
                swatch(ui, Some(c), 14.0, false, Sense::hover()).on_hover_text(format!("{} · {}", i + 1, hex(c)));
            }
        });
        ui.horizontal_wrapped(|ui| {
            let other = shown != current;
            if ui.add_enabled(other, egui::Button::new(tr("Für Sprite nutzen"))).on_hover_text(tr("Zuweisen — die Zeichnung bleibt, wie sie ist")).clicked()
            {
                self.assign_palette(&shown, true);
            }
            if ui
                .add_enabled(other, egui::Button::new(tr("Sprite umfärben")))
                .on_hover_text(tr("Zuweisen — die Nummern bleiben, die Farben kommen aus dieser Palette"))
                .clicked()
            {
                self.assign_palette(&shown, false);
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("+ Palette")).clicked() {
                self.pal.modal = Some(PalModal::new(None, self.project.unique_palette_name("meine_palette"), &pal));
            }
            if self.project.is_custom(&shown) {
                if ui.button(tr("Bearbeiten")).clicked() {
                    self.pal.modal = Some(PalModal::new(Some(shown.clone()), shown.clone(), &pal));
                }
                if ui.button(tr("Löschen …")).clicked() {
                    self.pal.delete = Some(shown.clone());
                }
            } else if ui.button(tr("Kopie bearbeiten")).clicked() {
                let name = self.project.unique_palette_name(&format!("{shown}_kopie"));
                self.pal_step();
                self.project.palettes.push(Palette::new(name.clone(), pal.colors.clone()));
                if shown == current {
                    self.project.sprite_mut().palette = name.clone();
                    self.pal.preview = None;
                } else {
                    self.pal.preview = Some(name.clone());
                }
                self.pal.modal = Some(PalModal::new(Some(name.clone()), name.clone(), &pal));
                self.changed();
            }
            if ui.button(tr("Bild » Palette …")).on_hover_text(tr("Aus den Farben des Bildes eine Palette machen — wie viele Farben, wählst du aus")).clicked()
            {
                self.open_reduce();
            }
        });
    }

    // ── Dialoge ─────────────────────────────────────────────────────
    pub(crate) fn palette_dialogs(&mut self, ctx: &egui::Context) {
        self.palette_modal(ctx);
        self.reduce_modal(ctx);
        let mut close = false;
        if let Some(name) = self.pal.delete.clone() {
            let users = self.project.sprites.iter().filter(|s| s.palette == name).count();
            egui::Modal::new(egui::Id::new("pal-delete")).show(ctx, |ui| {
                ui.heading(trf("Palette „{name}“ löschen?", &[("name", &name)]));
                if users > 0 {
                    ui.label(trf("{n} Sprites nutzen sie und bekommen die Standard-Palette.", &[("n", &users)]));
                }
                ui.horizontal(|ui| {
                    if ui.button(tr("Löschen")).clicked() {
                        self.project.palettes.retain(|p| p.name != name);
                        for s in &mut self.project.sprites {
                            if s.palette == name {
                                s.palette = Palette::grayscale().name;
                            }
                        }
                        if self.pal.preview.as_deref() == Some(&name) {
                            self.pal.preview = None;
                        }
                        // Undo-Schritte könnten die Palette zurückbringen und
                        // die Sprites trotzdem auf der Standard-Palette lassen.
                        for h in &mut self.histories {
                            h.clear();
                        }
                        self.clamp_color();
                        self.changed();
                        close = true;
                    }
                    if ui.button(tr("Abbrechen")).clicked() {
                        close = true;
                    }
                });
            });
        }
        if close {
            self.pal.delete = None;
        }
    }

    fn palette_modal(&mut self, ctx: &egui::Context) {
        let Some(m) = &mut self.pal.modal else { return };
        let (mut ok, mut cancel) = (false, false);
        let sources: Vec<String> = builtin::BUILTIN.iter().map(|(n, _)| n.to_string()).chain(self.project.palettes.iter().map(|p| p.name.clone())).collect();
        let mut load: Option<String> = None;
        egui::Modal::new(egui::Id::new("pal-modal")).show(ctx, |ui| {
            ui.heading(match &m.edit {
                Some(n) => trf("Palette „{name}“ bearbeiten", &[("name", n)]),
                None => tr("Neue Palette").to_string(),
            });
            egui::Grid::new("pal-grid").num_columns(2).show(ui, |ui| {
                ui.label(tr("Name"));
                ui.text_edit_singleline(&mut m.name);
                ui.end_row();
                if m.edit.is_none() {
                    ui.label(tr("Vorlage"));
                    egui::ComboBox::from_id_salt("pal-src").selected_text(tr("Farben übernehmen aus …")).show_ui(ui, |ui| {
                        for s in &sources {
                            if ui.selectable_label(false, s).clicked() {
                                load = Some(s.clone());
                            }
                        }
                    });
                    ui.end_row();
                }
            });
            ui.label(trf("{n} Farben", &[("n", &m.colors.len())]));
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                m.names.resize(m.colors.len(), String::new());
                for (i, (c, name)) in m.colors.iter_mut().zip(m.names.iter_mut()).enumerate() {
                    ui.horizontal(|ui| {
                        ui.monospace(format!("{:>3}", i + 1));
                        egui::color_picker::color_edit_button_srgb(ui, c);
                        ui.add(egui::TextEdit::singleline(name).hint_text(trf("Farbe {n}", &[("n", &(i + 1))])).char_limit(40).desired_width(140.0))
                            .on_hover_text(tr("Name der Farbe — leer lassen für „Farbe 3“"));
                        ui.monospace(hex(*c));
                    });
                }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(m.colors.len() < MAX_COLORS, egui::Button::new(tr("+ Farbe"))).clicked() {
                    let last = m.colors.last().copied().unwrap_or([0x88, 0x88, 0x88]);
                    m.colors.push(last);
                    m.names.push(String::new());
                }
                if ui.add_enabled(m.colors.len() > 1, egui::Button::new(tr("− Letzte"))).clicked() {
                    m.colors.pop();
                    m.names.pop();
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ok = ui.button(if m.edit.is_some() { tr("Speichern") } else { tr("Erstellen") }).clicked();
                cancel = ui.button(tr("Abbrechen")).clicked();
            });
        });
        if let Some(src) = load {
            let p = self.project.palette(&src);
            *m = PalModal::new(m.edit.take(), std::mem::take(&mut m.name), &p);
        }
        if cancel {
            self.pal.modal = None;
            return;
        }
        if !ok {
            return;
        }
        let name = clean_name(&m.name);
        let editing = m.edit.clone();
        let colors = m.colors.clone();
        let names: std::collections::BTreeMap<Px, String> =
            m.names.iter().enumerate().filter(|(_, n)| !n.trim().is_empty()).map(|(i, n)| (i as Px + 1, n.trim().to_string())).collect();
        if name.is_empty() {
            self.hint = Some(tr("Bitte einen Namen eingeben.").into());
            return;
        }
        if builtin::builtin(&name).is_some() {
            self.hint = Some(trf("„{name}“ ist eine eingebaute Palette — bitte einen anderen Namen.", &[("name", &name)]));
            return;
        }
        if self.project.is_custom(&name) && editing.as_deref() != Some(name.as_str()) {
            self.hint = Some(trf("Eine Palette „{name}“ gibt es schon.", &[("name", &name)]));
            return;
        }
        self.pal_step();
        match editing {
            Some(old) => {
                if let Some(p) = self.project.palettes.iter_mut().find(|p| p.name == old) {
                    p.name = name.clone();
                    p.colors = colors;
                    p.names = names;
                }
                for s in &mut self.project.sprites {
                    if s.palette == old {
                        s.palette = name.clone();
                    }
                }
                if self.pal.preview.as_deref() == Some(&old) {
                    self.pal.preview = Some(name);
                }
            }
            None => {
                let mut p = Palette::new(name.clone(), colors);
                p.names = names;
                self.project.palettes.push(p);
                self.pal.preview = Some(name);
            }
        }
        self.pal.modal = None;
        self.clamp_color();
        self.changed();
    }

    fn open_reduce(&mut self) {
        self.commit_float();
        let colors = palops::image_colors(self.project.sprite(), &self.project.current_palette());
        if colors.is_empty() {
            self.hint = Some(tr("Das Bild ist leer.").into());
            return;
        }
        let count = colors.len().min(MAX_COLORS);
        self.pal.reduce = Some(ReduceModal { colors, count, before: None, after: None });
    }

    /// Aktive Zelle als Bild, Farben über `color_of`.
    fn reduce_texture(&self, ctx: &egui::Context, name: &str, color_of: &dyn Fn(Rgb) -> Rgb) -> egui::TextureHandle {
        let sp = self.project.sprite();
        let pal = self.project.current_palette();
        // Große Bilder ausgedünnt: höchstens 256 Pixel Kante für die Vorschau.
        let step = sp.width.max(sp.height).div_ceil(256).max(1);
        let (w, h) = (sp.width.div_ceil(step), sp.height.div_ceil(step));
        let img = sp.cel(sp.frame, sp.layer);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = rgb_of(img.get(x * step, y * step), &pal, &sp.free) {
                    let [r, g, b] = color_of(c);
                    rgba[((y * w + x) * 4) as usize..][..4].copy_from_slice(&[r, g, b, 255]);
                }
            }
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        ctx.load_texture(name, image, egui::TextureOptions::NEAREST)
    }

    fn reduce_modal(&mut self, ctx: &egui::Context) {
        let Some(r) = &self.pal.reduce else { return };
        let n = r.colors.len();
        let count = r.count;
        let (colors, map) = palops::reduce_colors(&r.colors, count);
        let need_before = r.before.is_none();
        let need_after = r.after.as_ref().is_none_or(|a| a.0 != count);
        if need_before {
            let t = self.reduce_texture(ctx, "reduce-before", &|c| c);
            if let Some(r) = &mut self.pal.reduce {
                r.before = Some(t);
            }
        }
        if need_after {
            let lookup: HashMap<Rgb, Px> = map.clone();
            let cols = colors.clone();
            let t = self.reduce_texture(ctx, "reduce-after", &move |c| lookup.get(&c).map_or(c, |&i| cols[i as usize - 1]));
            if let Some(r) = &mut self.pal.reduce {
                r.after = Some((count, t));
            }
        }
        let Some(r) = &mut self.pal.reduce else { return };
        let (mut ok, mut cancel) = (false, false);
        egui::Modal::new(egui::Id::new("reduce")).show(ctx, |ui| {
            ui.heading(tr("Bild » Palette"));
            ui.label(trf("Das Bild hat {n} Farben, eine Palette fasst höchstens {max}.", &[("n", &n), ("max", &MAX_COLORS)]));
            ui.horizontal(|ui| {
                ui.label(tr("Farben in der Palette"));
                egui::ComboBox::from_id_salt("reduce-n").selected_text(format!("{}", r.count)).show_ui(ui, |ui| {
                    if n <= MAX_COLORS {
                        ui.selectable_value(&mut r.count, n, trf("alle {n}", &[("n", &n)]));
                    }
                    for c in [255, 128, 64, 32, 16, 8, 4] {
                        if c < n {
                            ui.selectable_value(&mut r.count, c, format!("{c}"));
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                for (label, tex) in [(tr("Vorher"), r.before.as_ref()), (tr("Nachher"), r.after.as_ref().map(|a| &a.1))] {
                    ui.vertical(|ui| {
                        ui.weak(label);
                        if let Some(t) = tex {
                            let s = t.size_vec2();
                            let k = (180.0 / s.x.max(s.y)).max(0.1);
                            ui.image((t.id(), s * k));
                        }
                    });
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(2.0);
                for c in &colors {
                    swatch(ui, Some(*c), 12.0, false, Sense::hover());
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ok = ui.button(tr("Palette anlegen")).clicked();
                cancel = ui.button(tr("Abbrechen")).clicked();
            });
        });
        if cancel {
            self.pal.reduce = None;
        }
        if ok {
            self.pal.reduce = None;
            let pal = self.project.current_palette();
            let name = self.project.unique_palette_name("foto");
            self.pal_step();
            palops::apply_reduce(self.project.sprite_mut(), &pal, &map);
            self.project.palettes.push(Palette::new(name.clone(), colors.clone()));
            self.project.sprite_mut().palette = name.clone();
            self.pal.preview = None;
            if self.color == 0 || self.color >= FREE_BASE || self.color as usize > colors.len() {
                self.color = 1;
            }
            self.hint = Some(trf("Palette „{name}“ mit {n} Farben angelegt.", &[("name", &name), ("n", &colors.len())]));
            self.changed();
        }
    }
}
