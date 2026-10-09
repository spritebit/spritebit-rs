//! Werkzeuge: Leiste, Tastenkürzel und was ein Klick auf der Fläche tut.
//!
//! Wie in der Web-Version: H Hand, P Stift, B Pinsel, S Spray, F Füllen,
//! E Radierer, I Linie, R Rechteck, O Ellipse. Rechte Maustaste radiert
//! mit jedem Malwerkzeug. Formen werden beim Ziehen als Vorschau gezeigt
//! und beim Loslassen gemalt — ein Undo-Schritt je Strich bzw. Form.

use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Vec2};
use spritebit_core::tools::{self, Span};
use spritebit_core::Px;

use crate::{icons, SpritebitApp};
use crate::i18n::{keys, tr, trf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Pan,
    Pencil,
    Brush,
    Spray,
    Fill,
    Eraser,
    Line,
    Rect,
    Ellipse,
    Select,
    Lasso,
    Magic,
    Wand,
}

impl Tool {
    const ALL: [Tool; 13] = [
        Tool::Pan,
        Tool::Pencil,
        Tool::Brush,
        Tool::Spray,
        Tool::Fill,
        Tool::Eraser,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Select,
        Tool::Lasso,
        Tool::Magic,
        Tool::Wand,
    ];

    fn label(self) -> &'static str {
        match self {
            Tool::Pan => tr("Hand"),
            Tool::Pencil => tr("Stift"),
            Tool::Brush => tr("Pinsel"),
            Tool::Spray => tr("Spray"),
            Tool::Fill => tr("Füllen"),
            Tool::Eraser => tr("Radierer"),
            Tool::Line => tr("Linie"),
            Tool::Rect => tr("Rechteck"),
            Tool::Ellipse => tr("Ellipse"),
            Tool::Select => tr("Auswahl"),
            Tool::Lasso => tr("Lasso"),
            Tool::Magic => tr("Farbwahl"),
            Tool::Wand => tr("Zauberstab"),
        }
    }

    fn key(self) -> Key {
        match self {
            Tool::Pan => Key::H,
            Tool::Pencil => Key::P,
            Tool::Brush => Key::B,
            Tool::Spray => Key::S,
            Tool::Fill => Key::F,
            Tool::Eraser => Key::E,
            Tool::Line => Key::I,
            Tool::Rect => Key::R,
            Tool::Ellipse => Key::O,
            Tool::Select => Key::A,
            Tool::Lasso => Key::L,
            Tool::Magic => Key::K,
            Tool::Wand => Key::W,
        }
    }

    fn icon(self) -> egui::ImageSource<'static> {
        match self {
            Tool::Pan => icons::HAND,
            Tool::Pencil => icons::PENCIL,
            Tool::Brush => icons::BRUSH,
            Tool::Spray => icons::SPRAY,
            Tool::Fill => icons::FILL,
            Tool::Eraser => icons::ERASER,
            Tool::Line => icons::LINE,
            Tool::Rect => icons::RECT,
            Tool::Ellipse => icons::ELLIPSE,
            Tool::Select => icons::SELECT,
            Tool::Lasso => icons::LASSO,
            Tool::Magic => icons::MAGIC,
            Tool::Wand => icons::WAND,
        }
    }

    /// Hat das Werkzeug eine Größe?
    fn sized(self) -> bool {
        matches!(self, Tool::Brush | Tool::Spray | Tool::Eraser)
    }

    fn is_shape(self) -> bool {
        matches!(self, Tool::Line | Tool::Rect | Tool::Ellipse)
    }
}

/// Was die Zeichenfläche über die Maus weiß.
/// Alt + rechte Maustaste ziehen: so viele Bildschirmpixel je Größenstufe.
const SIZE_STEP_PX: f32 = 12.0;

/// Laufendes Größe-Ziehen: Startpunkt, Größe beim Start und die Zelle, an
/// der die Vorschau stehen bleibt.
pub(crate) struct SizeDrag {
    start_x: f32,
    start_size: u32,
    cell: Option<(i64, i64)>,
}

/// Neue Größe aus dem Mausweg seit dem Start (1–9).
pub(crate) fn dragged_size(start_size: u32, dx: f32) -> u32 {
    (start_size as i64 + (dx / SIZE_STEP_PX).round() as i64).clamp(1, 9) as u32
}

pub(crate) struct Pointer {
    pub cell: Option<(i64, i64)>,
    /// Maus über der Fläche oder ein Zug, der auf ihr begann.
    pub over: bool,
    pub primary: bool,
    pub secondary: bool,
    pub pressed: bool,
    pub released: bool,
    pub panning: bool,
    /// Alt gedrückt: Pipette statt Malen.
    pub alt: bool,
}

impl SpritebitApp {
    pub(crate) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for t in Tool::ALL {
                let key = format!("{:?}", t.key());
                let on = self.tool == t;
                let color = if on { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                let btn = egui::Button::selectable(on, (icons::image(t.icon(), color), t.label()));
                if ui.add(btn).on_hover_text(format!("{} ({key})", t.label())).clicked() {
                    self.tool = t;
                }
                if matches!(t, Tool::Pan | Tool::Eraser | Tool::Ellipse) {
                    ui.separator();
                }
            }
            ui.separator();
            if self.tool.sized() {
                ui.label(tr("Größe"));
                for n in 1..=9u32 {
                    if ui.selectable_label(self.size == n, n.to_string()).clicked() {
                        self.size = n;
                    }
                }
            }
            if matches!(self.tool, Tool::Brush | Tool::Spray | Tool::Eraser) {
                ui.label(tr("Stärke")).on_hover_text(tr("Pinsel und Radierer: Dichte — Spray: Menge je Schritt"));
                ui.add(egui::Slider::new(&mut self.strength, 1..=100).suffix(" %"));
            }
            if matches!(self.tool, Tool::Rect | Tool::Ellipse) {
                ui.checkbox(&mut self.filled, tr("Gefüllt"));
            }
            if matches!(self.tool, Tool::Pencil | Tool::Eraser) {
                let r = ui
                    .checkbox(&mut self.pixel_perfect, tr("Clean Stroke"))
                    .on_hover_text(tr("Wie in bekannten Pixel-Art-Programmen: entfernt beim Zeichnen die doppelten Eckpixel an Treppenstufen — saubere 1-Pixel-Linien (Stift, Radierer mit Größe 1)"));
                if r.changed() {
                    self.save_view();
                }
            }
            ui.separator();
            let c = ui.visuals().text_color();
            let mx = egui::Button::selectable(self.mirror_x, icons::image(icons::MIRROR_X, c));
            if ui.add(mx).on_hover_text(tr("Symmetrie: links ↔ rechts")).clicked() {
                self.mirror_x = !self.mirror_x;
            }
            let my = egui::Button::selectable(self.mirror_y, icons::image(icons::MIRROR_Y, c));
            if ui.add(my).on_hover_text(tr("Symmetrie: oben ↔ unten")).clicked() {
                self.mirror_y = !self.mirror_y;
            }
            self.selection_bar(ui);
            // Rückgängig/Wiederholen griffbereit, wie an der Fläche im Web.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (can_redo, can_undo) = (self.history().can_redo(), self.history().can_undo());
                if icons::button(ui, icons::REDO, &format!("{} ({})", tr("Wiederholen"), keys("Strg+Y")), can_redo).clicked() {
                    self.redo();
                }
                if icons::button(ui, icons::UNDO, &format!("{} ({})", tr("Rückgängig"), keys("Strg+Z")), can_undo).clicked() {
                    self.undo();
                }
            });
        });
    }

    /// Werkzeug per Taste — nur ohne Strg/Alt und wenn kein Feld tippt.
    pub(crate) fn tool_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        for t in Tool::ALL {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, t.key())) {
                self.tool = t;
            }
        }
        // 0–9: Farbe mit dieser Nummer (0 = Transparent).
        let digits = [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
        for (n, k) in digits.into_iter().enumerate() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k)) && n <= self.project.current_palette().len() {
                self.color = n as Px;
            }
        }
    }

    /// Ist die aktive Ebene bemalbar? Sonst Hinweis in die Statusleiste.
    pub(crate) fn layer_ok(&mut self) -> bool {
        let layer = &self.sprite().layers[self.sprite().layer];
        self.hint = if layer.locked {
            Some(trf("Ebene „{name}“ ist gesperrt — Schloss in der Timeline.", &[("name", &layer.name)]))
        } else if !layer.visible {
            Some(trf("Ebene „{name}“ ist ausgeblendet — Auge in der Timeline.", &[("name", &layer.name)]))
        } else {
            None
        };
        self.hint.is_none()
    }

    fn record(&mut self) {
        let cur = self.project.current;
        self.histories[cur].record(&self.project.sprites[cur]);
    }

    /// Ein Schritt des Werkzeugs für diesen Durchlauf.
    /// Alt + rechte Maustaste ziehen verstellt bei Pinsel, Radierer und Spray
    /// die Größe: nach rechts größer, nach links kleiner. `true` = die Eingabe
    /// gehört dem Ziehen (dann wird weder radiert noch die Pipette benutzt).
    pub(crate) fn size_drag_pointer(&mut self, pointer: Option<Pos2>, p: &Pointer, secondary_pressed: bool) -> bool {
        if let Some(d) = &self.size_drag {
            if !p.secondary {
                self.size_drag = None;
                return true;
            }
            if let Some(pos) = pointer {
                self.size = dragged_size(d.start_size, pos.x - d.start_x);
                self.hint = Some(trf("Größe {n}", &[("n", &self.size)]));
            }
            return true;
        }
        if p.alt && secondary_pressed && p.over && self.tool.sized() {
            if let Some(pos) = pointer {
                self.size_drag = Some(SizeDrag { start_x: pos.x, start_size: self.size, cell: p.cell });
                self.hint = Some(trf("Größe {n}", &[("n", &self.size)]));
                return true;
            }
        }
        false
    }

    /// Umriss dessen, was das Werkzeug gleich trifft: ein Quadrat bei Stift,
    /// Pinsel und Radierer, ein Kreis beim Spray. Zwei Striche (dunkel, hell),
    /// damit er auf jedem Untergrund zu sehen ist.
    pub(crate) fn brush_preview(&self, painter: &egui::Painter, origin: Pos2, zoom: f32) {
        if self.playing || self.shape_start.is_some() {
            return;
        }
        let size = match self.tool {
            Tool::Pencil => 1,
            Tool::Brush | Tool::Eraser | Tool::Spray => self.size,
            _ => return,
        };
        let cell = match &self.size_drag {
            Some(d) => d.cell,
            None => self.hover,
        };
        let Some((x, y)) = cell else { return };
        let dark = egui::Stroke::new(3.0, Color32::from_black_alpha(150));
        let light = egui::Stroke::new(1.0, Color32::from_white_alpha(230));
        if self.tool == Tool::Spray {
            let c = origin + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * zoom;
            let r = (size as f32 + 0.5) * zoom;
            painter.circle_stroke(c, r, dark);
            painter.circle_stroke(c, r, light);
        } else {
            let s = tools::stamp(x, y, size);
            let rect = egui::Rect::from_min_max(
                origin + Vec2::new(s.x0 as f32, s.y0 as f32) * zoom,
                origin + Vec2::new((s.x1 + 1) as f32, (s.y1 + 1) as f32) * zoom,
            );
            painter.rect_stroke(rect, 0.0, dark, egui::StrokeKind::Middle);
            painter.rect_stroke(rect, 0.0, light, egui::StrokeKind::Middle);
        }
    }

    pub(crate) fn use_tool(&mut self, p: &Pointer, ctx: &egui::Context) {
        let down = p.primary || p.secondary;
        if p.panning || self.tool == Tool::Pan {
            self.stroke_last = None;
            self.shape_start = None;
            return;
        }
        // Alt+Klick: Pipette — mit jedem Malwerkzeug. In einer Auswahl
        // heißt Alt+Ziehen dagegen: eine Kopie verschieben.
        let in_selection = p.cell.is_some_and(|c| self.selection.as_ref().is_some_and(|s| s.contains(c.0, c.1)));
        if p.alt && p.pressed && p.over && !(self.is_select_tool() && in_selection) {
            if let Some(c) = p.cell {
                self.pick_color(c.0, c.1);
            }
            self.blocked = true;
            return;
        }
        if self.is_select_tool() {
            if p.pressed && p.over && self.playing {
                self.playing = false;
                return;
            }
            self.use_select_tool(p);
            return;
        }
        // Beim Abspielen hält ein Klick an, statt zu malen.
        if p.pressed && p.over && self.playing {
            self.playing = false;
            self.blocked = true;
        }
        if p.pressed && p.over && !self.playing && !self.blocked {
            self.blocked = !self.layer_ok();
            if !self.blocked {
                if let Some(cell) = p.cell {
                    self.begin(cell, p.secondary);
                }
            }
        }
        if !down || p.released {
            // Loslassen: eine Form wird jetzt gemalt.
            if let (Some(start), Some(end)) = (self.shape_start.take(), self.shape_end.take()) {
                self.record();
                let (spans, value) = (self.shape_spans(start, end), self.shape_value);
                self.paint(spans, value);
            }
            self.stroke_last = None;
            self.pp = None;
            self.blocked = false;
            return;
        }
        if self.blocked {
            return;
        }
        let Some(cell) = p.cell else { return };
        if self.shape_start.is_some() {
            self.shape_end = Some(cell);
            return;
        }
        if let Some(last) = self.stroke_last {
            if self.tool == Tool::Spray {
                self.spray_at(cell, p.secondary);
                // Spray sprüht weiter, auch wenn die Maus stillsteht.
                ctx.request_repaint();
            } else if last != cell {
                self.stroke(last, cell, p.secondary);
            }
            self.stroke_last = Some(cell);
        }
    }

    fn value(&self, erase: bool) -> Px {
        if erase || self.tool == Tool::Eraser {
            0
        } else {
            self.color
        }
    }

    /// Druck auf die Fläche: Strich beginnen, füllen oder Form anfangen.
    fn begin(&mut self, cell: (i64, i64), erase: bool) {
        // Wer malt, setzt Schwebendes vorher ab.
        self.commit_float();
        let value = self.value(erase);
        match self.tool {
            Tool::Fill => {
                self.record();
                // Mit Symmetrie auch an den gespiegelten Stellen füllen.
                let (w, h) = (self.sprite().width, self.sprite().height);
                let points = tools::mirror_spans(vec![(cell.1, cell.0, cell.0)], w, h, self.mirror_x, self.mirror_y);
                let img = self.project.sprite_mut().active();
                let n: usize = points.iter().map(|&(y, x, _)| tools::flood_fill(img, x, y, value)).sum();
                if n > 0 {
                    self.changed();
                }
            }
            t if t.is_shape() => {
                self.shape_start = Some(cell);
                self.shape_end = Some(cell);
                self.shape_value = value;
            }
            Tool::Spray => {
                self.record();
                self.stroke_last = Some(cell);
                self.spray_at(cell, erase);
            }
            _ => {
                self.record();
                self.stroke_last = Some(cell);
                // Pixel-perfekt: ein Strich, ein Pfad.
                self.pp = self.pixel_perfect_now().then(tools::PixelPerfect::new);
                self.stroke(cell, cell, erase);
            }
        }
    }

    /// Strich von a nach b: Stift 1 px, Pinsel und Radierer in ihrer Größe.
    /// Gilt Pixel-perfekt gerade? Wie in gängigen Pixel-Art-Programmen: beim Stift und beim
    /// Radierer mit Größe 1.
    pub(crate) fn pixel_perfect_now(&self) -> bool {
        self.pixel_perfect && (self.tool == Tool::Pencil || (self.tool == Tool::Eraser && self.size == 1))
    }

    fn stroke(&mut self, a: (i64, i64), b: (i64, i64), erase: bool) {
        let value = self.value(erase);
        if let Some(mut pp) = self.pp.take() {
            let (mx, my) = (self.mirror_x, self.mirror_y);
            let img = self.project.sprite_mut().active();
            let mut changed = false;
            for (x, y) in tools::line(a.0, a.1, b.0, b.1) {
                changed |= pp.add(img, x, y, value, mx, my);
            }
            self.pp = Some(pp);
            if changed {
                self.changed();
            }
            return;
        }
        let size = if self.tool == Tool::Pencil { 1 } else { self.size };
        let spans: Vec<Span> = tools::line(a.0, a.1, b.0, b.1)
            .into_iter()
            .flat_map(|(x, y)| tools::stamp(x, y, size).spans())
            .collect();
        // Stärke = Dichte bei Pinsel und Radierer: jedes Pixel nur mit dieser
        // Wahrscheinlichkeit (wie im Web). Der Stift malt immer voll.
        let density = self.strength as f64 / 100.0;
        let spans = if self.tool != Tool::Pencil && density < 1.0 {
            let mut out = Vec::new();
            for (y, x0, x1) in spans {
                for x in x0..=x1 {
                    if self.rng.next_f64() <= density {
                        out.push((y, x, x));
                    }
                }
            }
            out
        } else {
            spans
        };
        self.paint(spans, value);
    }

    /// Abschnitte malen — mit Symmetrie auch gespiegelt.
    fn paint(&mut self, spans: Vec<Span>, value: Px) {
        let (w, h) = (self.sprite().width, self.sprite().height);
        let spans = tools::mirror_spans(spans, w, h, self.mirror_x, self.mirror_y);
        tools::fill_spans(self.project.sprite_mut().active(), spans, value);
        self.changed();
    }

    fn spray_at(&mut self, cell: (i64, i64), erase: bool) {
        let value = self.value(erase);
        // Wie im Web: Radius = Größe, Menge je Schritt = Stärke / 10.
        let r = self.size as f64;
        let count = ((self.strength as f64 / 10.0).round() as usize).max(1);
        let pts = tools::spray(cell.0, cell.1, r, count, &mut self.rng);
        self.paint(pts.into_iter().map(|(x, y)| (y, x, x)).collect(), value);
    }

    fn shape_spans(&self, a: (i64, i64), b: (i64, i64)) -> Vec<Span> {
        match self.tool {
            Tool::Rect => tools::rect_spans(a.0, a.1, b.0, b.1, self.filled),
            Tool::Ellipse => tools::ellipse_spans(a.0, a.1, b.0, b.1, self.filled),
            _ => tools::line(a.0, a.1, b.0, b.1).into_iter().map(|(x, y)| (y, x, x)).collect(),
        }
    }

    /// Vorschau der Form beim Ziehen — als Abschnitte gezeichnet, darum auch
    /// bei riesigen Formen billig.
    pub(crate) fn shape_preview(&self, painter: &egui::Painter, origin: Pos2, zoom: f32) {
        let (Some(a), Some(b)) = (self.shape_start, self.shape_end) else { return };
        let palette = self.project.current_palette();
        let color = match palette.get(self.shape_value) {
            Some([r, g, b]) => Color32::from_rgb(r, g, b),
            None => Color32::from_rgba_unmultiplied(242, 139, 130, 110),
        };
        let clip = painter.clip_rect();
        for (y, x0, x1) in self.shape_spans(a, b) {
            let r = egui::Rect::from_min_max(
                origin + Vec2::new(x0 as f32, y as f32) * zoom,
                origin + Vec2::new((x1 + 1) as f32, (y + 1) as f32) * zoom,
            );
            if r.intersects(clip) {
                painter.rect_filled(r, 0.0, color);
            }
        }
    }
}

#[cfg(test)]
mod size_tests {
    use super::dragged_size;

    #[test]
    fn groesse_aus_dem_mausweg() {
        assert_eq!(dragged_size(3, 0.0), 3);
        assert_eq!(dragged_size(3, 24.0), 5, "zwei Stufen");
        assert_eq!(dragged_size(3, 5.0), 3, "unter einer halben Stufe bleibt es");
        assert_eq!(dragged_size(3, -100.0), 1);
        assert_eq!(dragged_size(3, 500.0), 9);
    }
}
