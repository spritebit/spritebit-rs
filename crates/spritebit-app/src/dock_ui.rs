//! Panels anordnen wie in der Web-Version (js/layout.js): jedes Panel hat
//! eine Seite (links, rechts) und einen Platz in deren Reihenfolge, und ist
//! entweder **angepinnt** (steht offen in der Spalte) oder **gelöst** (nur
//! ein Icon in der schmalen Leiste am Rand; ein Klick klappt es als Fenster
//! daneben auf).
//!
//! ```text
//!   ┌──┬──────────┬──────────────┬──────────┬──┐
//!   │▣ │▼ Sprites │              │▼ Ebenen  │▣ │   ▣ = gelöste Panels
//!   │▣ │▼ Farben  │  Zeichen-    │▼ Vorschau│  │
//!   │  │          │  fläche      │          │  │
//!   └──┴──────────┴──────────────┴──────────┴──┘
//! ```
//!
//! In der Kopfzeile eines angepinnten Panels: Pin (lösen) und ⋯ (nach
//! links/rechts, nach oben/unten). Im aufgeklappten Fenster: Anpinnen und
//! die Seite wechseln. Die Anordnung liegt in einer kleinen Datei im
//! Einstellungsordner („layout“), in Tests nie.

use eframe::egui::{self, Align2, Color32, Pos2};

use crate::i18n::{tr, trf};
use crate::{icons, SpritebitApp};

/// Alle Panels der Seitenleisten.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PanelId {
    Sprites,
    Colors,
    Layers,
    Preview,
    Palette,
    Image,
    Cleanup,
    Light,
    Tiles,
    Guides,
    Template,
    Output,
}

impl PanelId {
    pub(crate) const ALL: [PanelId; 12] = [
        PanelId::Sprites,
        PanelId::Colors,
        PanelId::Layers,
        PanelId::Preview,
        PanelId::Palette,
        PanelId::Image,
        PanelId::Cleanup,
        PanelId::Light,
        PanelId::Tiles,
        PanelId::Guides,
        PanelId::Template,
        PanelId::Output,
    ];

    /// Schlüssel für Einstellungen und den Auf-/Zu-Zustand (PanelMemory).
    pub(crate) fn key(self) -> &'static str {
        match self {
            PanelId::Sprites => "p-sprites",
            PanelId::Colors => "p-colors",
            PanelId::Layers => "p-layers",
            PanelId::Preview => "p-preview",
            PanelId::Palette => "p-palette",
            PanelId::Image => "p-image",
            PanelId::Cleanup => "p-cleanup",
            PanelId::Light => "p-light",
            PanelId::Tiles => "p-tiles",
            PanelId::Guides => "p-guides",
            PanelId::Template => "p-template",
            PanelId::Output => "p-output",
        }
    }

    fn from_key(k: &str) -> Option<PanelId> {
        PanelId::ALL.into_iter().find(|p| p.key() == k)
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            PanelId::Sprites => tr("Sprites"),
            PanelId::Colors => tr("Farben"),
            PanelId::Layers => tr("Ebenen"),
            PanelId::Preview => tr("Vorschau"),
            PanelId::Palette => tr("Palette"),
            PanelId::Image => tr("Bild"),
            PanelId::Cleanup => tr("Feinschliff"),
            PanelId::Light => tr("Licht"),
            PanelId::Tiles => tr("Kacheln"),
            PanelId::Guides => tr("Hilfslinien"),
            PanelId::Template => tr("Schablone"),
            PanelId::Output => tr("Code & Export"),
        }
    }

    pub(crate) fn icon(self) -> egui::ImageSource<'static> {
        match self {
            PanelId::Sprites => icons::SPRITES,
            PanelId::Colors => icons::COLORS,
            PanelId::Layers => icons::LAYERS,
            PanelId::Preview => icons::PREVIEW,
            PanelId::Palette => icons::PALETTE,
            PanelId::Image => icons::IMAGE,
            PanelId::Cleanup => icons::CLEANUP,
            PanelId::Light => icons::LIGHT,
            PanelId::Tiles => icons::TILES,
            PanelId::Guides => icons::GUIDES,
            PanelId::Template => icons::TEMPLATE,
            PanelId::Output => icons::OUTPUT,
        }
    }

    /// Beim ersten Start aufgeklappt?
    fn default_open(self) -> bool {
        matches!(self, PanelId::Sprites | PanelId::Colors | PanelId::Layers | PanelId::Preview | PanelId::Palette | PanelId::Image)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
}

impl Side {
    fn other(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

/// Wo welches Panel steht.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DockLayout {
    pub(crate) left: Vec<PanelId>,
    pub(crate) right: Vec<PanelId>,
    /// Gelöst: nur als Icon in der Leiste.
    pub(crate) loose: Vec<PanelId>,
    /// Das gerade aufgeklappte gelöste Panel und wo es steht (anfangs am
    /// Icon, nach dem Verschieben dort, wo man es losgelassen hat).
    pub(crate) flyout: Option<(PanelId, Pos2)>,
    /// Beim Ziehen des Fensters an seiner Kopfzeile: Abstand vom Zeiger zur
    /// Ecke, an der es hängt.
    pub(crate) flyout_grab: Option<egui::Vec2>,
    /// Klick aufs Icon eines angepinnten Panels: hinscrollen und aufklappen.
    pub(crate) focus: Option<PanelId>,
}

impl Default for DockLayout {
    fn default() -> Self {
        use PanelId::*;
        DockLayout {
            left: vec![Sprites, Colors],
            right: vec![Layers, Preview, Palette, Image, Cleanup, Light, Tiles, Guides, Template, Output],
            loose: Vec::new(),
            flyout: None,
            flyout_grab: None,
            focus: None,
        }
    }
}

impl DockLayout {
    pub(crate) fn side_of(&self, id: PanelId) -> Side {
        if self.left.contains(&id) {
            Side::Left
        } else {
            Side::Right
        }
    }

    fn list(&self, side: Side) -> &Vec<PanelId> {
        match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
        }
    }

    fn list_mut(&mut self, side: Side) -> &mut Vec<PanelId> {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        }
    }

    pub(crate) fn is_pinned(&self, id: PanelId) -> bool {
        !self.loose.contains(&id)
    }

    /// Angepinnte Panels einer Seite, in ihrer Reihenfolge.
    pub(crate) fn pinned(&self, side: Side) -> Vec<PanelId> {
        self.list(side).iter().copied().filter(|p| self.is_pinned(*p)).collect()
    }

    /// Gelöste Panels einer Seite.
    #[cfg(test)]
    pub(crate) fn loose_on(&self, side: Side) -> Vec<PanelId> {
        self.list(side).iter().copied().filter(|p| !self.is_pinned(*p)).collect()
    }

    /// Alle Panels einer Seite, angepinnt oder nicht — die Icons der Leiste.
    pub(crate) fn all_on(&self, side: Side) -> Vec<PanelId> {
        self.list(side).clone()
    }

    fn set_pinned(&mut self, id: PanelId, pinned: bool) {
        self.loose.retain(|p| *p != id);
        if !pinned {
            self.loose.push(id);
        }
    }

    /// Abgelegt nach dem Ziehen: auf `side`, vor `before` (sonst ans Ende),
    /// angepinnt oder gelöst (`pin`; `None` = wie bisher).
    pub(crate) fn place(&mut self, id: PanelId, side: Side, before: Option<PanelId>, pin: Option<bool>) {
        if before != Some(id) {
            self.left.retain(|p| *p != id);
            self.right.retain(|p| *p != id);
            let list = self.list_mut(side);
            let at = before.and_then(|b| list.iter().position(|p| *p == b)).unwrap_or(list.len());
            list.insert(at, id);
        }
        if let Some(pinned) = pin {
            self.set_pinned(id, pinned);
        }
        self.flyout = None;
    }

    pub(crate) fn toggle_pin(&mut self, id: PanelId) {
        if let Some(i) = self.loose.iter().position(|p| *p == id) {
            self.loose.remove(i);
        } else {
            self.loose.push(id);
        }
        self.flyout = None;
    }

    /// Auf die andere Seite, dort ans Ende.
    pub(crate) fn move_to_other_side(&mut self, id: PanelId) {
        let from = self.side_of(id);
        self.list_mut(from).retain(|p| *p != id);
        self.list_mut(from.other()).push(id);
        self.flyout = None;
    }

    /// In der Reihenfolge der eigenen Seite um `delta` (−1 = nach oben).
    /// Gelöste zählen mit — so bleibt ihr Platz erhalten, wenn man sie
    /// wieder anpinnt.
    pub(crate) fn move_by(&mut self, id: PanelId, delta: i32) {
        let side = self.side_of(id);
        let pinned = self.pinned(side);
        let Some(at) = pinned.iter().position(|p| *p == id) else { return };
        let to = at as i32 + delta;
        if to < 0 || to as usize >= pinned.len() {
            return;
        }
        let other = pinned[to as usize];
        let list = self.list_mut(side);
        let (a, b) = (list.iter().position(|p| *p == id).unwrap(), list.iter().position(|p| *p == other).unwrap());
        list.swap(a, b);
    }

    pub(crate) fn can_move_by(&self, id: PanelId, delta: i32) -> bool {
        let pinned = self.pinned(self.side_of(id));
        pinned.iter().position(|p| *p == id).is_some_and(|at| {
            let to = at as i32 + delta;
            to >= 0 && (to as usize) < pinned.len()
        })
    }

    /// Als Text für die Einstellungsdatei.
    pub(crate) fn to_text(&self) -> String {
        let keys = |v: &[PanelId]| v.iter().map(|p| p.key()).collect::<Vec<_>>().join(",");
        format!("left={}\nright={}\nloose={}\n", keys(&self.left), keys(&self.right), keys(&self.loose))
    }

    /// Aus der Einstellungsdatei. Unbekanntes fällt weg; Panels, die es noch
    /// nicht gab, kommen an ihren gewohnten Platz.
    pub(crate) fn from_text(text: &str) -> DockLayout {
        let mut d = DockLayout { left: Vec::new(), right: Vec::new(), loose: Vec::new(), flyout: None, flyout_grab: None, focus: None };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let ids: Vec<PanelId> = v.split(',').filter_map(|s| PanelId::from_key(s.trim())).collect();
            match k.trim() {
                "left" => d.left = ids,
                "right" => d.right = ids,
                "loose" => d.loose = ids,
                _ => {}
            }
        }
        // Doppeltes nur einmal: links gewinnt.
        let left = d.left.clone();
        d.right.retain(|p| !left.contains(p));
        let mut seen = Vec::new();
        d.left.retain(|p| {
            if seen.contains(p) {
                false
            } else {
                seen.push(*p);
                true
            }
        });
        d.right.retain(|p| {
            if seen.contains(p) {
                false
            } else {
                seen.push(*p);
                true
            }
        });
        let base = DockLayout::default();
        for p in PanelId::ALL {
            if !seen.contains(&p) {
                d.list_mut(base.side_of(p)).push(p);
            }
        }
        d.loose.dedup();
        d
    }

    #[cfg(not(test))]
    fn file() -> Option<std::path::PathBuf> {
        Some(crate::i18n::settings_dir()?.join("layout"))
    }

    pub(crate) fn load() -> DockLayout {
        #[cfg(not(test))]
        if let Some(text) = Self::file().and_then(|p| std::fs::read_to_string(p).ok()) {
            return DockLayout::from_text(&text);
        }
        DockLayout::default()
    }

    pub(crate) fn save(&self) {
        #[cfg(not(test))]
        if let Some(p) = Self::file() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, self.to_text());
        }
    }
}

/// Breite der Icon-Leiste.
const RAIL_W: f32 = 42.0;
/// Icons in der Leiste — größer als in Knöpfen, sie sind das Einzige dort.
const RAIL_ICON: f32 = 22.0;
/// Farbe für „angepinnt“ und die Ablage-Linie beim Ziehen.
const ACCENT: Color32 = Color32::from_rgb(110, 168, 254);

/// Wohin ein gezogenes Panel fällt.
#[derive(Clone, Copy)]
struct Drop {
    side: Side,
    /// Vor diesem Panel; `None` = ans Ende.
    before: Option<PanelId>,
    /// In der Spalte abgelegt: angepinnt. In der Leiste: wie es war.
    pin: Option<bool>,
}

impl SpritebitApp {
    /// Der Inhalt eines Panels — derselbe, ob angepinnt oder aufgeklappt.
    fn panel_body(&mut self, id: PanelId, ui: &mut egui::Ui) {
        match id {
            PanelId::Sprites => self.sprite_list(ui),
            PanelId::Colors => {
                ui.add_space(4.0);
                self.colors_panel(ui);
            }
            PanelId::Layers => self.layers_panel(ui),
            PanelId::Preview => self.preview_panel(ui),
            PanelId::Palette => self.palette_library(ui),
            PanelId::Image => self.image_panel(ui),
            PanelId::Cleanup => self.cleanup_panel(ui),
            PanelId::Light => self.light_panel(ui),
            PanelId::Tiles => self.tiles_panel(ui),
            PanelId::Guides => self.guides_panel(ui),
            PanelId::Template => self.template_panel(ui),
            PanelId::Output => self.output_panel(ui),
        }
    }

    /// Pinnadel als Fenster-Knopf: blau = angepinnt. Gibt `true` beim Klick.
    fn pin_button(ui: &mut egui::Ui, pinned: bool) -> bool {
        let (c, tip) = if pinned {
            (ACCENT, tr("Angepinnt — klicken, um das Panel zu lösen (dann nur noch als Icon in der Leiste)"))
        } else {
            (ui.visuals().text_color(), tr("Anpinnen — das Panel steht dann offen in der Spalte"))
        };
        ui.add(egui::Button::image(icons::image(icons::PIN, c).alt_text(tip)).frame(false)).on_hover_text(tip).clicked()
    }

    /// Knöpfe rechts in der Kopfzeile (von rechts nach links): Pin, und was
    /// das Panel selbst braucht.
    fn panel_header(&mut self, id: PanelId, ui: &mut egui::Ui) {
        if Self::pin_button(ui, true) {
            self.dock.toggle_pin(id);
            self.dock.save();
        }
        if id == PanelId::Sprites && ui.small_button("+").on_hover_text(tr("Neuer Sprite")).clicked() {
            self.open_new_sprite();
        }
    }

    /// Rechtsklick auf Kopfzeile oder Icon: dieselben Befehle wie das Ziehen.
    fn panel_menu(&mut self, id: PanelId, r: &egui::Response) {
        r.context_menu(|ui| {
            let c = ui.visuals().text_color();
            let pinned = self.dock.is_pinned(id);
            let mut changed = false;
            let (label, icon) = match self.dock.side_of(id) {
                Side::Left => (tr("Nach rechts"), icons::RIGHT),
                Side::Right => (tr("Nach links"), icons::LEFT),
            };
            if ui.add(egui::Button::image_and_text(icons::image(icon, c), label)).clicked() {
                self.dock.move_to_other_side(id);
                changed = true;
            }
            if ui.add_enabled(self.dock.can_move_by(id, -1), egui::Button::image_and_text(icons::image(icons::UP, c), tr("Nach oben"))).clicked() {
                self.dock.move_by(id, -1);
                changed = true;
            }
            if ui.add_enabled(self.dock.can_move_by(id, 1), egui::Button::image_and_text(icons::image(icons::DOWN, c), tr("Nach unten"))).clicked() {
                self.dock.move_by(id, 1);
                changed = true;
            }
            ui.separator();
            let label = if pinned { tr("Lösen") } else { tr("Anpinnen") };
            if ui.add(egui::Button::image_and_text(icons::image(icons::PIN, c), label)).clicked() {
                self.dock.toggle_pin(id);
                changed = true;
            }
            if changed {
                self.dock.save();
                ui.close();
            }
        });
    }

    /// Ziel beim Ziehen: blaue Linie über `r` (oder darunter, `after`), und
    /// beim Loslassen dort ablegen.
    fn drop_target(&mut self, ui: &egui::Ui, r: &egui::Response, after: bool, drop: Drop) {
        if r.dnd_hover_payload::<PanelId>().is_some() {
            let y = if after { r.rect.top() + 2.0 } else { r.rect.top() - 1.0 };
            ui.painter().with_clip_rect(ui.clip_rect().expand(4.0)).hline(r.rect.x_range(), y, egui::Stroke::new(2.0, ACCENT));
        }
        if let Some(id) = r.dnd_release_payload::<PanelId>() {
            self.dock.place(*id, drop.side, drop.before, drop.pin);
            self.dock.save();
        }
    }

    /// Eine Spalte: ihre angepinnten Panels untereinander.
    pub(crate) fn dock_column(&mut self, ui: &mut egui::Ui, side: Side, focus_output: bool) {
        ui.add_space(4.0);
        for id in self.dock.pinned(side) {
            let focus = self.dock.focus == Some(id) || (id == PanelId::Output && focus_output);
            let force = focus.then_some(true);
            let (row, grip) = self.panel_with(
                ui,
                id.title(),
                id.icon(),
                id.key(),
                id.default_open(),
                force,
                move |s, ui| s.panel_header(id, ui),
                move |s, ui| s.panel_body(id, ui),
            );
            if focus {
                row.scroll_to_me(Some(egui::Align::TOP));
                self.dock.focus = None;
            }
            grip.dnd_set_drag_payload(id);
            self.panel_menu(id, &row);
            self.drop_target(ui, &row, false, Drop { side, before: Some(id), pin: Some(true) });
        }
        // Platz darunter: ans Ende dieser Spalte.
        let rest = ui.allocate_response(egui::vec2(ui.available_width(), ui.available_height().max(40.0)), egui::Sense::hover());
        self.drop_target(ui, &rest, true, Drop { side, before: None, pin: Some(true) });
    }

    /// Die Icon-Leiste einer Seite: alle Panels dieser Seite. Angepinnte
    /// tragen einen blauen Punkt (ein Klick springt hin), gelöste klappen
    /// auf. Icons lassen sich ziehen — in die andere Leiste oder in eine Spalte.
    pub(crate) fn dock_rail(&mut self, ui: &mut egui::Ui, side: Side) {
        let panel = match side {
            Side::Left => egui::Panel::left("rail-left"),
            Side::Right => egui::Panel::right("rail-right"),
        };
        panel.exact_size(RAIL_W).resizable(false).show(ui, |ui| {
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                for id in self.dock.all_on(side) {
                    let pinned = self.dock.is_pinned(id);
                    let open = self.dock.flyout.is_some_and(|(f, _)| f == id);
                    let c = if open { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                    let tip = if pinned {
                        trf("{name} — angepinnt, Klick springt hin", &[("name", &id.title())])
                    } else {
                        trf("{name} — Klick klappt auf", &[("name", &id.title())])
                    };
                    let r = ui
                        .add(
                            egui::Button::selectable(open, icons::image(id.icon(), c).fit_to_exact_size(egui::vec2(RAIL_ICON, RAIL_ICON)).alt_text(&tip))
                                .sense(egui::Sense::click_and_drag()),
                        )
                        .on_hover_text(&tip);
                    if pinned {
                        ui.painter().circle_filled(r.rect.right_bottom() - egui::vec2(4.0, 4.0), 3.0, ACCENT);
                    }
                    r.dnd_set_drag_payload(id);
                    if r.clicked() {
                        if pinned {
                            self.dock.focus = Some(id);
                            self.dock.flyout = None;
                        } else {
                            let at = match side {
                                Side::Left => r.rect.right_top() + egui::vec2(8.0, 0.0),
                                Side::Right => r.rect.left_top() - egui::vec2(8.0, 0.0),
                            };
                            self.dock.flyout = if open { None } else { Some((id, at)) };
                        }
                    }
                    self.panel_menu(id, &r);
                    self.drop_target(ui, &r, false, Drop { side, before: Some(id), pin: None });
                }
                let rest = ui.allocate_response(egui::vec2(ui.available_width(), ui.available_height().max(30.0)), egui::Sense::hover());
                self.drop_target(ui, &rest, true, Drop { side, before: None, pin: None });
            });
        });
    }

    /// Das aufgeklappte gelöste Panel, als Fenster neben seinem Icon — mit
    /// eigener Kopfzeile: Icon, Name, rechts Pin und × wie bei einem Fenster.
    pub(crate) fn dock_flyout(&mut self, ctx: &egui::Context) {
        let Some((id, at)) = self.dock.flyout else { return };
        if self.dock.is_pinned(id) {
            self.dock.flyout = None;
            return;
        }
        let pivot = match self.dock.side_of(id) {
            Side::Left => Align2::LEFT_TOP,
            Side::Right => Align2::RIGHT_TOP,
        };
        // Wird das Panel an seiner Kopfzeile gezogen (flyout_grab), wandert
        // das Fenster mit. Klicks gehen dabei durch — darunter liegen die
        // Leisten und Spalten, in die man es andocken kann. Lässt man es
        // woanders los, bleibt es dort stehen.
        // Zieht man stattdessen sein Icon am Rand, ist das Umsortieren — das
        // Fenster wird dafür nicht gebraucht und klappt zu.
        let dragging = egui::DragAndDrop::payload::<PanelId>(ctx).is_some_and(|p| *p == id);
        let mut at = at;
        match (dragging, self.dock.flyout_grab) {
            (true, Some(grab)) => {
                if let Some(p) = ctx.pointer_interact_pos() {
                    let screen = ctx.content_rect();
                    at = (p + grab).clamp(screen.min, screen.max - egui::vec2(40.0, 40.0));
                    self.dock.flyout = Some((id, at));
                }
            }
            (true, None) => {
                self.dock.flyout = None;
                return;
            }
            (false, _) => self.dock.flyout_grab = None,
        }
        let (mut close, mut pin) = (false, false);
        egui::Window::new(id.title())
            .id(egui::Id::new(("flyout", id.key())))
            .title_bar(false)
            .interactable(!dragging)
            .pivot(pivot)
            .fixed_pos(at)
            .resizable(false)
            .default_width(270.0)
            .max_height(ctx.content_rect().height() - at.y - 40.0)
            .show(ctx, |ui| {
                // Die ganze Kopfzeile ist der Griff. Er wird ZUERST angelegt,
                // damit Pin und × darüber liegen und ihre Klicks behalten.
                let head_rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), ui.spacing().interact_size.y));
                let head = ui
                    .interact(head_rect, ui.id().with("flyout-head"), egui::Sense::drag())
                    .on_hover_cursor(egui::CursorIcon::Grab)
                    .on_hover_text(tr("Ziehen: Panel verschieben oder in eine Leiste andocken"));
                head.dnd_set_drag_payload(id);
                if head.drag_started() {
                    let p = ctx.input(|i| i.pointer.press_origin()).or(head.interact_pointer_pos()).unwrap_or(at);
                    self.dock.flyout_grab = Some(at - p);
                }
                let row = ui
                    .horizontal(|ui| {
                        let c = ui.visuals().text_color();
                        ui.add(icons::image(id.icon(), c));
                        ui.add(egui::Label::new(egui::RichText::new(id.title()).strong()).selectable(false));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let tip = tr("Schließen");
                            close = ui.add(egui::Button::image(icons::image(icons::CLOSE, c).alt_text(tip)).frame(false)).on_hover_text(tip).clicked();
                            pin = Self::pin_button(ui, false);
                        });
                    })
                    .response;
                self.panel_menu(id, &row);
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| self.panel_body(id, ui));
            });
        // Klick daneben schließt — wie in der Web-Version. Nur auf der Fläche
        // dahinter (Leisten, Zeichenfläche): Klicks in Menüs, Farbwähler oder
        // Dialoge, die das Panel geöffnet hat, liegen auf eigenen Ebenen
        // davor. Die Icon-Leisten schalten selbst um.
        let outside = ctx.input(|i| i.pointer.any_pressed().then(|| i.pointer.press_origin()).flatten()).is_some_and(|p| {
            let screen = ctx.content_rect();
            let on_rail = p.x < screen.left() + RAIL_W || p.x > screen.right() - RAIL_W;
            !on_rail && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background)
        });
        if close || outside {
            self.dock.flyout = None;
        }
        if pin {
            self.dock.toggle_pin(id);
            self.dock.save();
        }
    }

    /// Während des Ziehens: der Name des Panels am Zeiger.
    pub(crate) fn dock_drag_preview(&self, ctx: &egui::Context) {
        let Some(id) = egui::DragAndDrop::payload::<PanelId>(ctx) else { return };
        let Some(p) = ctx.pointer_interact_pos() else { return };
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        // Das aufgeklappte Fenster wandert selbst mit — kein Schild nötig.
        if self.dock.flyout_grab.is_some() && self.dock.flyout.is_some_and(|(f, _)| f == *id) {
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("dock-drag")));
        let font = egui::FontId::proportional(13.0);
        let text = painter.layout_no_wrap(id.title().to_string(), font, Color32::WHITE);
        let rect = egui::Rect::from_min_size(p + egui::vec2(14.0, 10.0), egui::vec2(text.size().x + 24.0, 24.0));
        painter.rect_filled(rect, 5.0, Color32::from_rgb(40, 52, 76));
        painter.rect_stroke(rect, 5.0, egui::Stroke::new(1.0, ACCENT), egui::StrokeKind::Inside);
        painter.galley(rect.left_center() + egui::vec2(12.0, -text.size().y / 2.0), text, Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anfangs_wie_bisher() {
        let d = DockLayout::default();
        assert_eq!(d.pinned(Side::Left), [PanelId::Sprites, PanelId::Colors]);
        assert_eq!(d.pinned(Side::Right).first(), Some(&PanelId::Layers));
        assert!(d.loose_on(Side::Left).is_empty() && d.loose_on(Side::Right).is_empty());
    }

    #[test]
    fn loesen_verschieben_reihenfolge() {
        let mut d = DockLayout::default();
        d.toggle_pin(PanelId::Light);
        assert_eq!(d.loose_on(Side::Right), [PanelId::Light]);
        assert!(!d.pinned(Side::Right).contains(&PanelId::Light));
        d.move_to_other_side(PanelId::Layers);
        assert_eq!(d.pinned(Side::Left), [PanelId::Sprites, PanelId::Colors, PanelId::Layers]);
        d.move_by(PanelId::Layers, -1);
        assert_eq!(d.pinned(Side::Left), [PanelId::Sprites, PanelId::Layers, PanelId::Colors]);
        assert!(!d.can_move_by(PanelId::Sprites, -1));
        assert!(d.can_move_by(PanelId::Sprites, 1));
        d.toggle_pin(PanelId::Light);
        assert!(d.is_pinned(PanelId::Light));
    }

    #[test]
    fn ablegen_nach_dem_ziehen() {
        let mut d = DockLayout::default();
        // Icon von „Licht“ in die linke Leiste vor „Farben“: Seite gewechselt, angepinnt wie vorher.
        d.place(PanelId::Light, Side::Left, Some(PanelId::Colors), None);
        assert_eq!(d.left, [PanelId::Sprites, PanelId::Light, PanelId::Colors]);
        assert!(!d.right.contains(&PanelId::Light));
        assert!(d.is_pinned(PanelId::Light));
        // Gelöstes Panel in eine Spalte gezogen: angepinnt, ans Ende.
        d.toggle_pin(PanelId::Tiles);
        d.place(PanelId::Tiles, Side::Left, None, Some(true));
        assert_eq!(d.left.last(), Some(&PanelId::Tiles));
        assert!(d.is_pinned(PanelId::Tiles));
        // Auf sich selbst: bleibt, wo es ist.
        let before = d.left.clone();
        d.place(PanelId::Light, Side::Left, Some(PanelId::Light), Some(true));
        assert_eq!(d.left, before);
    }

    #[test]
    fn speichern_und_laden() {
        let mut d = DockLayout::default();
        d.move_to_other_side(PanelId::Preview);
        d.toggle_pin(PanelId::Tiles);
        assert_eq!(DockLayout::from_text(&d.to_text()), d);
    }

    #[test]
    fn kaputtes_und_neue_panels() {
        let d = DockLayout::from_text("left=p-preview,quatsch,p-preview\nright=p-preview\nloose=p-light\nfoo=bar");
        assert_eq!(d.left.first(), Some(&PanelId::Preview), "links gewinnt, doppelt nur einmal");
        assert_eq!(d.left.iter().filter(|p| **p == PanelId::Preview).count(), 1);
        assert!(!d.right.contains(&PanelId::Preview));
        for p in PanelId::ALL {
            assert!(d.left.contains(&p) || d.right.contains(&p), "{p:?} fehlt");
        }
        assert_eq!(d.loose, [PanelId::Light]);
    }
}
