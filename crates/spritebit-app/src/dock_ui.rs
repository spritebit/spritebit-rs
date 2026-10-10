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

use eframe::egui::{self, Align2, Pos2, Vec2};

use crate::i18n::tr;
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
            PanelId::Cleanup => tr("Aufräumen"),
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
    /// Das gerade aufgeklappte gelöste Panel und wo sein Icon sitzt.
    pub(crate) flyout: Option<(PanelId, Pos2)>,
}

impl Default for DockLayout {
    fn default() -> Self {
        use PanelId::*;
        DockLayout {
            left: vec![Sprites, Colors],
            right: vec![Layers, Preview, Palette, Image, Cleanup, Light, Tiles, Guides, Template, Output],
            loose: Vec::new(),
            flyout: None,
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

    /// Gelöste Panels einer Seite — die Icons der Leiste.
    pub(crate) fn loose_on(&self, side: Side) -> Vec<PanelId> {
        self.list(side).iter().copied().filter(|p| !self.is_pinned(*p)).collect()
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
        let mut d = DockLayout { left: Vec::new(), right: Vec::new(), loose: Vec::new(), flyout: None };
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
        d.left.retain(|p| if seen.contains(p) { false } else { seen.push(*p); true });
        d.right.retain(|p| if seen.contains(p) { false } else { seen.push(*p); true });
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
const RAIL_W: f32 = 34.0;

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

    /// Knöpfe rechts in der Kopfzeile (von rechts nach links): ⋯, Pin, und
    /// was das Panel selbst braucht.
    fn panel_header(&mut self, id: PanelId, ui: &mut egui::Ui) {
        let c = ui.visuals().weak_text_color();
        let side = self.dock.side_of(id);
        let mut changed = false;
        ui.menu_image_button(icons::image(icons::MENU, c).alt_text(tr("Panel verschieben")), |ui| {
            let (label, icon) = match side {
                Side::Left => (tr("Nach rechts"), icons::RIGHT),
                Side::Right => (tr("Nach links"), icons::LEFT),
            };
            if ui.add(egui::Button::image_and_text(icons::image(icon, ui.visuals().text_color()), label)).clicked() {
                self.dock.move_to_other_side(id);
                changed = true;
                ui.close();
            }
            if ui.add_enabled(self.dock.can_move_by(id, -1), egui::Button::image_and_text(icons::image(icons::UP, ui.visuals().text_color()), tr("Nach oben"))).clicked() {
                self.dock.move_by(id, -1);
                changed = true;
                ui.close();
            }
            if ui.add_enabled(self.dock.can_move_by(id, 1), egui::Button::image_and_text(icons::image(icons::DOWN, ui.visuals().text_color()), tr("Nach unten"))).clicked() {
                self.dock.move_by(id, 1);
                changed = true;
                ui.close();
            }
        })
        .response
        .on_hover_text(tr("Panel verschieben"));
        let tip = tr("Lösen — das Panel steht dann nur noch als Icon in der Leiste am Rand");
        if ui.add(egui::Button::image(icons::image(icons::PIN, c).alt_text(tip)).frame(false)).on_hover_text(tip).clicked() {
            self.dock.toggle_pin(id);
            changed = true;
        }
        if id == PanelId::Sprites && ui.small_button("+").on_hover_text(tr("Neuer Sprite")).clicked() {
            self.open_new_sprite();
        }
        if changed {
            self.dock.save();
        }
    }

    /// Eine Spalte: ihre angepinnten Panels untereinander.
    pub(crate) fn dock_column(&mut self, ui: &mut egui::Ui, side: Side, focus_output: bool) {
        ui.add_space(4.0);
        for id in self.dock.pinned(side) {
            let force = (id == PanelId::Output && focus_output).then_some(true);
            let r = self.panel_with(ui, id.title(), id.icon(), id.key(), id.default_open(), force, move |s, ui| s.panel_header(id, ui), move |s, ui| s.panel_body(id, ui));
            if force.is_some() {
                r.scroll_to_me(Some(egui::Align::TOP));
            }
        }
    }

    /// Die Icon-Leiste einer Seite (nur, wenn dort etwas gelöst ist).
    pub(crate) fn dock_rail(&mut self, ui: &mut egui::Ui, side: Side) {
        let loose = self.dock.loose_on(side);
        if loose.is_empty() {
            return;
        }
        let panel = match side {
            Side::Left => egui::Panel::left("rail-left"),
            Side::Right => egui::Panel::right("rail-right"),
        };
        panel.exact_size(RAIL_W).resizable(false).show(ui, |ui| {
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                for id in loose {
                    let open = self.dock.flyout.is_some_and(|(f, _)| f == id);
                    let c = if open { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                    let r = ui.add(egui::Button::selectable(open, icons::image(id.icon(), c).alt_text(id.title()))).on_hover_text(id.title());
                    if r.clicked() {
                        let at = match side {
                            Side::Left => r.rect.right_top() + Vec2::new(8.0, 0.0),
                            Side::Right => r.rect.left_top() - Vec2::new(8.0, 0.0),
                        };
                        self.dock.flyout = if open { None } else { Some((id, at)) };
                    }
                }
            });
        });
    }

    /// Das aufgeklappte gelöste Panel, als Fenster neben seinem Icon.
    pub(crate) fn dock_flyout(&mut self, ctx: &egui::Context) {
        let Some((id, at)) = self.dock.flyout else { return };
        if self.dock.is_pinned(id) {
            self.dock.flyout = None;
            return;
        }
        let side = self.dock.side_of(id);
        let pivot = match side {
            Side::Left => Align2::LEFT_TOP,
            Side::Right => Align2::RIGHT_TOP,
        };
        let mut open = true;
        let mut changed = false;
        egui::Window::new(id.title())
            .id(egui::Id::new(("flyout", id.key())))
            .pivot(pivot)
            .fixed_pos(at)
            .collapsible(false)
            .resizable(false)
            .default_width(270.0)
            .max_height(ctx.content_rect().height() - at.y - 40.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let c = ui.visuals().text_color();
                    if ui.add(egui::Button::image_and_text(icons::image(icons::PIN, c), tr("Anpinnen"))).on_hover_text(tr("Das Panel steht dann offen in der Spalte")).clicked() {
                        self.dock.toggle_pin(id);
                        changed = true;
                    }
                    let (label, icon) = match side {
                        Side::Left => (tr("Nach rechts"), icons::RIGHT),
                        Side::Right => (tr("Nach links"), icons::LEFT),
                    };
                    if ui.add(egui::Button::image_and_text(icons::image(icon, c), label)).clicked() {
                        self.dock.move_to_other_side(id);
                        changed = true;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| self.panel_body(id, ui));
            });
        if !open {
            self.dock.flyout = None;
        }
        if changed {
            self.dock.save();
        }
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
