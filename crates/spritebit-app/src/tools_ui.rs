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
}

impl Tool {
    const ALL: [Tool; 9] =
        [Tool::Pan, Tool::Pencil, Tool::Brush, Tool::Spray, Tool::Fill, Tool::Eraser, Tool::Line, Tool::Rect, Tool::Ellipse];

    fn label(self) -> &'static str {
        match self {
            Tool::Pan => "Hand",
            Tool::Pencil => "Stift",
            Tool::Brush => "Pinsel",
            Tool::Spray => "Spray",
            Tool::Fill => "Füllen",
            Tool::Eraser => "Radierer",
            Tool::Line => "Linie",
            Tool::Rect => "Rechteck",
            Tool::Ellipse => "Ellipse",
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
pub(crate) struct Pointer {
    pub cell: Option<(i64, i64)>,
    /// Maus über der Fläche oder ein Zug, der auf ihr begann.
    pub over: bool,
    pub primary: bool,
    pub secondary: bool,
    pub pressed: bool,
    pub released: bool,
    pub panning: bool,
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
                if t == Tool::Pan || t == Tool::Eraser {
                    ui.separator();
                }
            }
            ui.separator();
            if self.tool.sized() {
                ui.label("Größe");
                for n in 1..=9u32 {
                    if ui.selectable_label(self.size == n, n.to_string()).clicked() {
                        self.size = n;
                    }
                }
            }
            if matches!(self.tool, Tool::Rect | Tool::Ellipse) {
                ui.checkbox(&mut self.filled, "Gefüllt");
            }
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
    }

    /// Ist die aktive Ebene bemalbar? Sonst Hinweis in die Statusleiste.
    fn layer_ok(&mut self) -> bool {
        let layer = &self.sprite().layers[self.sprite().layer];
        self.hint = if layer.locked {
            Some(format!("Ebene „{}“ ist gesperrt — Schloss in der Timeline.", layer.name))
        } else if !layer.visible {
            Some(format!("Ebene „{}“ ist ausgeblendet — Auge in der Timeline.", layer.name))
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
    pub(crate) fn use_tool(&mut self, p: &Pointer, ctx: &egui::Context) {
        let down = p.primary || p.secondary;
        if p.panning || self.tool == Tool::Pan {
            self.stroke_last = None;
            self.shape_start = None;
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
                tools::fill_spans(self.project.sprite_mut().active(), spans, value);
                self.changed();
            }
            self.stroke_last = None;
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
        let value = self.value(erase);
        match self.tool {
            Tool::Fill => {
                self.record();
                let n = tools::flood_fill(self.project.sprite_mut().active(), cell.0, cell.1, value);
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
                self.stroke(cell, cell, erase);
            }
        }
    }

    /// Strich von a nach b: Stift 1 px, Pinsel und Radierer in ihrer Größe.
    fn stroke(&mut self, a: (i64, i64), b: (i64, i64), erase: bool) {
        let value = self.value(erase);
        let size = if self.tool == Tool::Pencil { 1 } else { self.size };
        let img = self.project.sprite_mut().active();
        for (x, y) in tools::line(a.0, a.1, b.0, b.1) {
            tools::fill_spans(img, tools::stamp(x, y, size).spans(), value);
        }
        self.changed();
    }

    fn spray_at(&mut self, cell: (i64, i64), erase: bool) {
        let value = self.value(erase);
        let r = self.size as f64 * 1.5 + 1.0;
        let count = (self.size as usize).max(1) * 2;
        let pts = tools::spray(cell.0, cell.1, r, count, &mut self.rng);
        let img = self.project.sprite_mut().active();
        tools::fill_spans(img, pts.into_iter().map(|(x, y)| (y, x, x)), value);
        self.changed();
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
