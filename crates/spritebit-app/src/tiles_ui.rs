//! Panel „Kacheln“: Tilemap-Ebenen wie in der Web-Version (js/tilemap.js).
//!
//! - Tilemap-Ebene anlegen oder die aktive umwandeln, zurück zur normalen
//! - Modus „Pixel malen“: eine Kachel ändert sich überall (Auto legt in
//!   leeren Zellen neue an, Manuell nicht); Modus „Kacheln setzen“: Stift
//!   setzt die gewählte Kachel, Radierer/Rechts leert, Füllen füllt, Alt nimmt auf
//! - Raster der Kacheln über der Zeichenfläche, Vorschau unter dem Zeiger
//! - Export für Godot 4: Ordner mit Kachelbild (PNG), Szene (.tscn) und JSON
//!
//! Die Rechnung steckt in `spritebit_core::tilemap`. Abgeglichen wird nach
//! jedem neuen Undo-Schritt, sobald die Maus losgelassen ist ([`SpritebitApp::tile_sync`]).

use eframe::egui::{self, Color32, Pos2, Vec2};
use spritebit_core::selection::rgb_of;
use spritebit_core::tilemap::{self, GodotLayer, NewTiles, Tileset, DEFAULT_TILE, TILE_SIZES};
use spritebit_core::{export, Px};

use crate::i18n::{tr, trf};
use crate::tools_ui::{Pointer, Tool};
use crate::SpritebitApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileMode {
    Pixel,
    Tiles,
}

pub(crate) struct TileState {
    pub mode: TileMode,
    pub new_tiles: NewTiles,
    /// Gewählte Kachel zum Setzen (ab 1).
    pub tile: usize,
    /// Kachelgröße für neue/umgewandelte Ebenen.
    pub tw: u32,
    pub th: u32,
    /// Laufendes Setzen: welche Kachel (0 = leeren) und die letzte Zelle.
    pub drag: Option<(usize, Option<(u32, u32)>)>,
    /// Bildchen der Kacheln, neu gebaut, wenn sich der Inhalt ändert.
    thumbs: Vec<egui::TextureHandle>,
    thumbs_key: u64,
}

impl Default for TileState {
    fn default() -> Self {
        TileState { mode: TileMode::Pixel, new_tiles: NewTiles::Auto, tile: 1, tw: DEFAULT_TILE, th: DEFAULT_TILE, drag: None, thumbs: Vec::new(), thumbs_key: 0 }
    }
}

fn safe(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '_' }).collect();
    let t = t.trim_matches('_').to_string();
    if t.is_empty() {
        "tiles".into()
    } else {
        t
    }
}

impl SpritebitApp {
    /// Kachelsatz der aktiven Ebene — `None`, wenn sie keine Tilemap ist
    /// (oder gerade die Maske bearbeitet wird).
    pub(crate) fn active_tileset(&self) -> Option<&Tileset> {
        let sp = self.sprite();
        if sp.editing_mask() {
            return None;
        }
        sp.layers.get(sp.layer)?.tileset.as_ref()
    }

    /// Gehört der Zeiger gerade dem Kachel-Setzen?
    pub(crate) fn tile_mode_on(&self) -> bool {
        self.tiles.mode == TileMode::Tiles && self.active_tileset().is_some()
    }

    /// Nach einer Änderung: Tilemap-Ebenen mit ihren Kachelsätzen abgleichen.
    /// Läuft einmal je neuem Undo-Schritt, wenn keine Maustaste mehr gedrückt
    /// ist — die Änderung am Kachelsatz gehört damit zum selben Schritt.
    pub(crate) fn tile_sync(&mut self, pointer_down: bool) {
        if pointer_down {
            return;
        }
        let cur = self.project.current;
        if !self.project.sprites[cur].layers.iter().any(|l| l.tileset.is_some()) {
            // Trotzdem abholen, sonst gälte der Schritt später als neu.
            let _ = self.histories[cur].unseen_step();
            return;
        }
        let paint = self.tiles.mode == TileMode::Pixel;
        let mode = self.tiles.new_tiles;
        let Some(before) = self.histories[cur].unseen_step() else { return };
        let r = tilemap::sync_sprite(&mut self.project.sprites[cur], before, paint, mode);
        if r.blocked > 0 {
            self.hint = Some(tr("Manuell: in leere Zellen wird nicht gemalt (dafür „Auto“ wählen).").into());
        }
        if r.edited > 0 || r.blocked > 0 {
            self.changed();
        }
    }

    // ── Zeiger im Modus „Kacheln“ ───────────────────────────────────

    fn tile_cell(&self, cell: Option<(i64, i64)>) -> Option<(u32, u32)> {
        let ts = self.active_tileset()?;
        let (x, y) = cell?;
        if x < 0 || y < 0 {
            return None;
        }
        let (cols, rows) = ts.map_size(self.sprite().width, self.sprite().height);
        let (cx, cy) = (x as u32 / ts.tw, y as u32 / ts.th);
        (cx < cols && cy < rows).then_some((cx, cy))
    }

    fn place_at(&mut self, c: (u32, u32), k: usize) {
        let Some(ts) = self.active_tileset().cloned() else { return };
        ts.place(self.project.sprite_mut().active(), c.0, c.1, k);
        self.changed();
    }

    /// Statt des Werkzeugs (tools_ui.rs `use_tool`), solange Kacheln gesetzt werden.
    pub(crate) fn use_tile_tool(&mut self, p: &Pointer) {
        let down = p.primary || p.secondary;
        if p.pressed && p.over {
            if self.playing {
                self.playing = false;
                self.blocked = true;
                return;
            }
            let Some(c) = self.tile_cell(p.cell) else {
                self.blocked = true;
                return;
            };
            let ts = self.active_tileset().cloned().expect("tile_cell prüft die Tilemap");
            if p.alt {
                let k = ts.tile_at(self.sprite().target(), c.0, c.1);
                if k > 0 {
                    self.tiles.tile = k as usize;
                    self.hint = Some(trf("Kachel {k} aufgenommen.", &[("k", &k)]));
                } else {
                    self.hint = Some(tr("Hier liegt keine Kachel.").into());
                }
                self.blocked = true;
                return;
            }
            if !self.layer_ok() {
                self.blocked = true;
                return;
            }
            let erase = p.secondary || self.tool == Tool::Eraser;
            if !erase && ts.tiles.get(self.tiles.tile.wrapping_sub(1)).is_none() {
                self.hint = Some(
                    if ts.tiles.is_empty() {
                        tr("Noch keine Kacheln — im Modus „Pixel malen“ in eine leere Zelle malen.")
                    } else {
                        tr("Erst eine Kachel in der Liste wählen.")
                    }
                    .into(),
                );
                self.blocked = true;
                return;
            }
            self.commit_float();
            let k = if erase { 0 } else { self.tiles.tile };
            let cur = self.project.current;
            self.histories[cur].record(&self.project.sprites[cur]);
            if self.tool == Tool::Fill {
                if ts.fill(self.project.sprite_mut().active(), c.0, c.1, k) > 0 {
                    self.changed();
                } else {
                    self.histories[cur].drop_last();
                }
                self.blocked = true;
                return;
            }
            self.tiles.drag = Some((k, None));
        }
        if !down || p.released {
            self.tiles.drag = None;
            self.blocked = false;
            return;
        }
        if self.blocked {
            return;
        }
        let Some((k, last)) = self.tiles.drag else { return };
        let Some(c) = self.tile_cell(p.cell) else { return };
        if last != Some(c) {
            self.tiles.drag = Some((k, Some(c)));
            self.place_at(c, k);
        }
    }

    // ── Raster über der Zeichenfläche ───────────────────────────────

    pub(crate) fn draw_tiles(&self, painter: &egui::Painter, origin: Pos2, zoom: f32) {
        let Some(ts) = self.active_tileset() else { return };
        let sp = self.sprite();
        let (cols, rows) = ts.map_size(sp.width, sp.height);
        let (mw, mh) = ((cols * ts.tw) as f32 * zoom, (rows * ts.th) as f32 * zoom);
        let (w, h) = (sp.width as f32 * zoom, sp.height as f32 * zoom);
        // Der Rand außerhalb der Karte: abgedunkelt (gehört zu keiner Kachel).
        let dim = Color32::from_black_alpha(90);
        if mw < w {
            painter.rect_filled(egui::Rect::from_min_size(origin + Vec2::new(mw, 0.0), Vec2::new(w - mw, h)), 0.0, dim);
        }
        if mh < h {
            painter.rect_filled(egui::Rect::from_min_size(origin + Vec2::new(0.0, mh), Vec2::new(mw, h - mh)), 0.0, dim);
        }
        if ts.tw as f32 * zoom >= 4.0 {
            let stroke = egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(110, 170, 255, 140));
            for x in 0..=cols {
                let px = origin.x + (x * ts.tw) as f32 * zoom;
                painter.line_segment([Pos2::new(px, origin.y), Pos2::new(px, origin.y + mh)], stroke);
            }
            for y in 0..=rows {
                let py = origin.y + (y * ts.th) as f32 * zoom;
                painter.line_segment([Pos2::new(origin.x, py), Pos2::new(origin.x + mw, py)], stroke);
            }
        }
        // Modus „Kacheln“: die gewählte Kachel als Vorschau unter dem Zeiger.
        if self.tiles.mode != TileMode::Tiles || self.tiles.drag.is_some() {
            return;
        }
        let Some((cx, cy)) = self.tile_cell(self.hover) else { return };
        let at = origin + Vec2::new((cx * ts.tw) as f32, (cy * ts.th) as f32) * zoom;
        if self.tool != Tool::Eraser {
            if let Some(tile) = ts.tiles.get(self.tiles.tile.wrapping_sub(1)) {
                let pal = self.project.current_palette();
                for (i, &v) in tile.iter().enumerate() {
                    let Some(c) = rgb_of(v, &pal, &sp.free) else { continue };
                    let (x, y) = (i as u32 % ts.tw, i as u32 / ts.tw);
                    let r = egui::Rect::from_min_size(at + Vec2::new(x as f32, y as f32) * zoom, Vec2::splat(zoom));
                    painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 160));
                }
            }
        }
        let r = egui::Rect::from_min_size(at, Vec2::new(ts.tw as f32, ts.th as f32) * zoom);
        painter.rect_stroke(r, 0.0, egui::Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Middle);
    }

    // ── Panel ───────────────────────────────────────────────────────

    fn tiles_structural(&mut self, f: impl FnOnce(&mut spritebit_core::Sprite)) {
        self.commit_float();
        self.selection = None;
        let cur = self.project.current;
        self.histories[cur].record(&self.project.sprites[cur]);
        f(self.project.sprite_mut());
        self.changed();
    }

    fn tile_size_picker(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(tr("Kachelgröße"));
            let before = self.tiles.tw;
            for (id, v) in [("tile-w", &mut self.tiles.tw), ("tile-h", &mut self.tiles.th)] {
                egui::ComboBox::from_id_salt(id).width(64.0).selected_text(format!("{v} px")).show_ui(ui, |ui| {
                    for n in TILE_SIZES {
                        ui.selectable_value(v, n, format!("{n} px"));
                    }
                });
                if id == "tile-w" {
                    ui.label("×");
                }
            }
            // Quadratisch ist der Normalfall: Breite ändern zieht die Höhe mit.
            if self.tiles.tw != before {
                self.tiles.th = self.tiles.tw;
            }
        });
    }

    pub(crate) fn tiles_panel(&mut self, ui: &mut egui::Ui) {
        if self.sprite().editing_mask() {
            ui.label(tr("Gerade wird eine Maske bearbeitet — Kacheln gibt es erst wieder danach."));
            return;
        }
        let Some(ts) = self.active_tileset().cloned() else {
            ui.label(tr("Die aktive Ebene ist keine Tilemap. Eine Tilemap besteht aus Kacheln fester Größe — malt man eine Kachel an, ändert sie sich überall, wo sie liegt. Gut für Spiel-Levels und Muster."));
            self.tile_size_picker(ui);
            let (tw, th) = (self.tiles.tw, self.tiles.th);
            if ui.button(tr("Neue Tilemap-Ebene")).on_hover_text(tr("Neue, leere Tilemap-Ebene über der aktiven anlegen")).clicked() {
                self.tiles_structural(|sp| {
                    let n = sp.layers.len() + 1;
                    let at = sp.layer + 1;
                    sp.add_layer(at, trf("Tilemap {n}", &[("n", &n)]));
                    sp.layers[at].tileset = Some(Tileset::new(tw, th));
                });
                self.tiles.mode = TileMode::Pixel;
                let sp = self.sprite();
                self.hint = Some(if !sp.width.is_multiple_of(tw) || !sp.height.is_multiple_of(th) {
                    trf("Tilemap angelegt. Der Sprite ist kein Vielfaches von {tw} × {th} — der dunkle Rand gehört zu keiner Kachel.", &[("tw", &tw), ("th", &th)])
                } else {
                    tr("Tilemap angelegt — einfach losmalen, jede bemalte Zelle wird eine Kachel.").into()
                });
            }
            if ui.button(tr("Aktive Ebene umwandeln")).on_hover_text(tr("Die aktive Ebene in Kacheln zerlegen — gleiche Stellen werden eine Kachel")).clicked() {
                let mut n = 0;
                self.tiles_structural(|sp| {
                    let l = sp.layer;
                    let mut ids: Vec<usize> = sp.frames.iter().map(|f| f.cels[l]).collect();
                    ids.dedup();
                    let ts = Tileset::build(tw, th, ids.iter().map(|&i| &sp.images[i]));
                    n = ts.tiles.len();
                    sp.layers[l].tileset = Some(ts);
                });
                self.hint = Some(trf("In Kacheln zerlegt: {n} verschiedene Kacheln.", &[("n", &n)]));
            }
            return;
        };

        ui.horizontal(|ui| {
            for (m, label, tip) in [
                (TileMode::Pixel, tr("Pixel malen"), tr("Kacheln bemalen — eine Kachel ändert sich überall, wo sie liegt")),
                (TileMode::Tiles, tr("Kacheln setzen"), tr("Kacheln aus der Liste ins Raster setzen")),
            ] {
                if ui.add(egui::Button::selectable(self.tiles.mode == m, label)).on_hover_text(tip).clicked() {
                    self.tiles.mode = m;
                }
            }
        });
        if self.tiles.mode == TileMode::Pixel {
            ui.horizontal(|ui| {
                ui.label(tr("Neue Kacheln"));
                let auto = tr("Auto — beim Malen anlegen");
                let manual = tr("Manuell — nur vorhandene ändern");
                egui::ComboBox::from_id_salt("tile-auto")
                    .selected_text(if self.tiles.new_tiles == NewTiles::Auto { auto } else { manual })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.tiles.new_tiles, NewTiles::Auto, auto);
                        ui.selectable_value(&mut self.tiles.new_tiles, NewTiles::Manual, manual);
                    });
            });
        }
        let hint = match (self.tiles.mode, self.tiles.new_tiles) {
            (TileMode::Tiles, _) => tr("Stift setzt die gewählte Kachel, Radierer oder Rechtsklick leert, Füllen füllt, Alt + Klick nimmt eine Kachel auf."),
            (_, NewTiles::Auto) => tr("Malen ändert die Kachel überall, wo sie liegt. Wer in eine leere Zelle malt, legt eine neue Kachel an."),
            (_, NewTiles::Manual) => tr("Malen ändert die Kachel überall, wo sie liegt. Leere Zellen bleiben leer — es entstehen keine neuen Kacheln."),
        };
        ui.small(hint);
        let (cols, rows) = ts.map_size(self.sprite().width, self.sprite().height);
        ui.small(trf("{tw} × {th} px · {n} Kacheln · Raster {cols} × {rows}", &[("tw", &ts.tw), ("th", &ts.th), ("n", &ts.tiles.len()), ("cols", &cols), ("rows", &rows)]));

        if ts.tiles.is_empty() {
            ui.small(tr("Noch keine Kacheln — im Modus „Pixel malen“ (Auto) in eine leere Zelle malen."));
        } else {
            if self.tiles.tile > ts.tiles.len() {
                self.tiles.tile = ts.tiles.len();
            }
            self.update_tile_thumbs(ui.ctx(), &ts);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                for (i, tex) in self.tiles.thumbs.clone().iter().enumerate() {
                    let on = self.tiles.tile == i + 1;
                    let img = egui::Image::new((tex.id(), Vec2::splat(32.0)));
                    let r = ui.add(egui::Button::image(img).selected(on)).on_hover_text(trf("Kachel {k} — zum Setzen wählen", &[("k", &(i + 1))]));
                    if r.clicked() {
                        self.tiles.tile = i + 1;
                        // Wer eine Kachel wählt, will sie meist auch setzen.
                        self.tiles.mode = TileMode::Tiles;
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            if ui.button(tr("Unbenutzte entfernen")).on_hover_text(tr("Kacheln entfernen, die nirgends mehr liegen")).clicked() {
                let mut n = 0;
                self.tiles_structural(|sp| {
                    let l = sp.layer;
                    let ids: Vec<usize> = sp.frames.iter().map(|f| f.cels[l]).collect();
                    let mut ts = sp.layers[l].tileset.take().expect("Tilemap-Ebene");
                    n = ts.prune_unused(ids.iter().map(|&i| &sp.images[i]));
                    sp.layers[l].tileset = Some(ts);
                });
                self.tiles.tile = 1;
                self.hint = Some(if n > 0 { trf("{n} unbenutzte Kacheln entfernt.", &[("n", &n)]) } else { tr("Alle Kacheln werden benutzt.").into() });
            }
            if ui.button(tr("Normale Ebene")).on_hover_text(tr("Wieder eine normale Ebene — die Pixel bleiben, die Kacheln fallen weg")).clicked() {
                self.tiles_structural(|sp| {
                    let l = sp.layer;
                    sp.layers[l].tileset = None;
                });
                self.hint = Some(tr("Wieder eine normale Ebene — die Pixel sind geblieben.").into());
            }
        });
        if ui
            .button(tr("Für Godot exportieren"))
            .on_hover_text(tr("Ordner mit Kachelbild (PNG), Godot-Szene (.tscn mit TileMapLayer) und JSON — den Ordner ins Godot-Projekt (res://) legen"))
            .clicked()
        {
            match self.export_godot() {
                Ok(Some(msg)) => self.hint = Some(msg),
                Ok(None) => {}
                Err(e) => self.error = Some(e),
            }
        }
    }

    fn update_tile_thumbs(&mut self, ctx: &egui::Context, ts: &Tileset) {
        use std::hash::{Hash, Hasher};
        let pal = self.project.current_palette();
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        ts.tiles.hash(&mut hs);
        pal.colors.hash(&mut hs);
        self.sprite().free.hash(&mut hs);
        let key = hs.finish();
        if key == self.tiles.thumbs_key && self.tiles.thumbs.len() == ts.tiles.len() {
            return;
        }
        let free = self.sprite().free.clone();
        self.tiles.thumbs = ts
            .tiles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let rgba: Vec<u8> = t
                    .iter()
                    .flat_map(|&v: &Px| rgb_of(v, &pal, &free).map_or([0, 0, 0, 0], |c| [c[0], c[1], c[2], 255]))
                    .collect();
                let img = egui::ColorImage::from_rgba_unmultiplied([ts.tw as usize, ts.th as usize], &rgba);
                ctx.load_texture(format!("tile-{i}"), img, egui::TextureOptions::NEAREST)
            })
            .collect();
        self.tiles.thumbs_key = key;
    }

    /// Ordner wählen, darin `<sprite>/` mit Kachelbildern, .tscn und .json.
    fn export_godot(&mut self) -> Result<Option<String>, String> {
        self.commit_float();
        let sp = self.sprite().clone();
        let pal = self.project.current_palette();
        let base = safe(&sp.name);
        let mut layers = Vec::new();
        let mut pngs = Vec::new();
        let mut used: Vec<String> = Vec::new();
        for (li, l) in sp.layers.iter().enumerate() {
            let Some(ts) = l.tileset.as_ref().filter(|ts| !ts.tiles.is_empty()) else { continue };
            let mut ln = safe(&l.name);
            while used.contains(&ln) {
                ln.push('_');
            }
            used.push(ln.clone());
            let file = format!("{base}_{ln}.png");
            pngs.push((file.clone(), export::tileset_png(&sp, &pal, ts).map_err(|e| crate::i18n::export_error(&e))?));
            layers.push(GodotLayer {
                name: &l.name,
                ts,
                map: ts.map_of(sp.cel(sp.frame, li)),
                png: format!("res://{base}/{file}"),
                visible: l.visible,
                opacity: l.opacity,
            });
        }
        if layers.is_empty() {
            return Ok(Some(tr("Keine Tilemap-Ebene mit Kacheln zum Exportieren.").into()));
        }
        let Some(parent) = rfd::FileDialog::new().set_title(tr("Godot-Projekt oder Ordner wählen")).pick_folder() else { return Ok(None) };
        let dir = parent.join(&base);
        let err = |e: std::io::Error| trf("{path} konnte nicht geschrieben werden: {e}", &[("path", &dir.display()), ("e", &e)]);
        std::fs::create_dir_all(&dir).map_err(err)?;
        for (name, bytes) in &pngs {
            std::fs::write(dir.join(name), bytes).map_err(err)?;
        }
        std::fs::write(dir.join(format!("{base}.tscn")), tilemap::godot_scene(&sp.name, &layers)).map_err(err)?;
        std::fs::write(dir.join(format!("{base}.json")), tilemap::tilemap_json(&sp.name, &layers)).map_err(err)?;
        Ok(Some(trf("Gespeichert in „{dir}“ — liegt der Ordner im Godot-Projekt, die .tscn öffnen.", &[("dir", &dir.display())])))
    }
}
