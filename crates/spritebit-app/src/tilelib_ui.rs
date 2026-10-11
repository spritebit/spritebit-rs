//! Kachelsatz-Bibliothek im Panel „Kacheln“ — wie im Web (js/tilemap.js):
//!
//! * „Speichern“: den Satz der aktiven Tilemap-Ebene unter einem Namen ins
//!   Projekt legen (gleicher Name ersetzt — einen fremden erst nach Rückfrage)
//! * „Neue Ebene“: neue Tilemap-Ebene mit einem gespeicherten Satz, in jedem
//!   Sprite des Projekts
//! * „Übernehmen“ / „Neu laden“: seine Kacheln in die aktive Tilemap-Ebene
//!   legen, die Anordnung bleibt
//! * als Datei speichern und laden — für andere Projekte, auch fremde
//!   Kachelbilder (PNG)
//!
//! Die Rechnung steht in `spritebit_core::tilelib`.

use eframe::egui;
use spritebit_core::tilelib::{self, LibSet};
use spritebit_core::tilemap::Tileset;

use crate::i18n::{tr, trf};
use crate::tiles_ui::TileMode;
use crate::SpritebitApp;

/// Zustand der Bibliothek im Panel.
#[derive(Default)]
pub(crate) struct LibState {
    /// Name im Feld „Speichern“ und für welche Ebene er gilt (Sprite, Ebene).
    pub name: String,
    name_for: Option<(usize, usize)>,
    /// Gewählter Satz in der Liste.
    pub pick: Option<String>,
    /// Rückfragen: einen fremden Satz ersetzen, einen Satz löschen.
    pub confirm_replace: Option<String>,
    pub confirm_del: Option<String>,
}

impl SpritebitApp {
    pub(crate) fn tiles_library(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.strong(tr("Bibliothek"));
        let ts = self.active_tileset().cloned();
        let key = (self.project.current, self.sprite().layer);
        if let Some(ts) = &ts {
            // Name vorbelegen: woher die Ebene kommt, sonst ihr Name.
            if self.tlib.name_for != Some(key) {
                self.tlib.name = ts.lib.clone().unwrap_or_else(|| self.sprite().layers[self.sprite().layer].name.clone());
                self.tlib.name_for = Some(key);
            }
            let src = ts.lib.clone().filter(|l| self.project.tileset_lib.contains_key(l));
            if let Some(src) = &src {
                ui.weak(trf("Diese Ebene kommt aus „{name}“.", &[("name", src)]));
            }
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.tlib.name).desired_width(110.0).hint_text(tr("Name des Kachelsatzes")));
                if ui
                    .button(tr("Speichern"))
                    .on_hover_text(tr("Den Kachelsatz dieser Ebene in die Bibliothek des Projekts speichern — gleicher Name ersetzt"))
                    .clicked()
                {
                    let name = self.tlib.name.trim().to_string();
                    self.save_to_lib(name, false);
                }
                if let Some(src) = &src {
                    if ui.button(tr("Neu laden")).on_hover_text(tr("Die Kacheln aus der Bibliothek neu laden — die Anordnung bleibt")).clicked() {
                        self.use_from_lib(src.clone());
                    }
                }
            });
            if let Some(n) = self.tlib.confirm_replace.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(trf("In der Bibliothek gibt es schon „{name}“. Ersetzen?", &[("name", &n)]));
                    if ui.button(tr("Ersetzen")).clicked() {
                        self.save_to_lib(n.clone(), true);
                    }
                    if ui.button(tr("Abbrechen")).clicked() {
                        self.tlib.confirm_replace = None;
                    }
                });
            }
        }

        let names: Vec<String> = self.project.tileset_lib.keys().cloned().collect();
        if names.is_empty() {
            ui.weak(tr("Noch keine gespeicherten Kachelsätze. Speichere den einer Tilemap-Ebene oder lade eine Datei — auch ein Kachelbild (PNG)."));
        } else {
            let pick = self
                .tlib
                .pick
                .clone()
                .filter(|p| names.contains(p))
                .or_else(|| ts.as_ref().and_then(|t| t.lib.clone()).filter(|l| names.contains(l)))
                .unwrap_or_else(|| names[0].clone());
            let label = |n: &str, s: &LibSet| trf("{name} · {tw}×{th} · {n}", &[("name", &n), ("tw", &s.tw), ("th", &s.th), ("n", &s.tiles.len())]);
            let mut chosen = pick.clone();
            let set = self.project.tileset_lib[&pick].clone();
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("tile-lib")
                    .selected_text(label(&pick, &set))
                    .show_ui(ui, |ui| {
                        for n in &names {
                            let s = &self.project.tileset_lib[n];
                            ui.selectable_value(&mut chosen, n.clone(), label(n, s));
                        }
                    })
                    .response
                    .on_hover_text(tr("Gespeicherte Kachelsätze dieses Projekts — passen in jeden Sprite"));
                if ui.button(tr("Neue Ebene")).on_hover_text(tr("Neue Tilemap-Ebene mit diesem Kachelsatz")).clicked() {
                    self.new_layer_from_lib(pick.clone());
                }
                let fits = ts.as_ref().is_some_and(|t| t.tw == set.tw && t.th == set.th);
                if ui
                    .add_enabled(fits, egui::Button::new(tr("Übernehmen")))
                    .on_hover_text(tr("Diese Kacheln in die aktive Tilemap-Ebene legen — die Anordnung bleibt (gleiche Kachelgröße nötig)"))
                    .clicked()
                {
                    self.use_from_lib(pick.clone());
                }
                if ui.button("×").on_hover_text(tr("Gewählten Kachelsatz aus der Bibliothek löschen")).clicked() {
                    self.tlib.confirm_del = Some(pick.clone());
                }
            });
            self.tlib.pick = Some(chosen);
            if let Some(n) = self.tlib.confirm_del.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(trf("Kachelsatz „{name}“ aus der Bibliothek löschen? Ebenen, die ihn benutzen, behalten ihre Kacheln.", &[("name", &n)]));
                    if ui.button(tr("Löschen")).clicked() {
                        self.project.tileset_lib.remove(&n);
                        self.tlib.confirm_del = None;
                        self.dirty = true;
                    }
                    if ui.button(tr("Abbrechen")).clicked() {
                        self.tlib.confirm_del = None;
                    }
                });
            }
        }
        ui.horizontal_wrapped(|ui| {
            if !names.is_empty()
                && ui.button(tr("Als Datei speichern …")).on_hover_text(tr("Gewählten Kachelsatz als Datei speichern — für andere Projekte")).clicked()
            {
                if let Some(n) = self.tlib.pick.clone() {
                    self.export_lib_set(&n);
                }
            }
            if ui
                .button(tr("Datei laden …"))
                .on_hover_text(tr("Kachelsatz-Datei (.json) oder Kachelbild (PNG) laden — ein Bild wird in Kacheln der eingestellten Größe zerschnitten"))
                .clicked()
            {
                self.import_lib_file();
            }
        });
    }

    /// Satz der aktiven Ebene in die Bibliothek. Einen fremden gleichen
    /// Namens nur mit `replace` (nach Rückfrage) ersetzen.
    pub(crate) fn save_to_lib(&mut self, name: String, replace: bool) {
        let Some(ts) = self.active_tileset().cloned() else { return };
        let name = if name.is_empty() { tilelib::free_name(&self.project.tileset_lib, &self.sprite().layers[self.sprite().layer].name) } else { name };
        if self.project.tileset_lib.contains_key(&name) && ts.lib.as_deref() != Some(&name) && !replace {
            self.tlib.confirm_replace = Some(name);
            return;
        }
        let pal = self.project.current_palette();
        let set = tilelib::to_lib(&ts, &pal, &self.sprite().free);
        self.project.tileset_lib.insert(name.clone(), set);
        let l = self.sprite().layer;
        if let Some(t) = self.project.sprite_mut().layers[l].tileset.as_mut() {
            t.lib = Some(name.clone());
        }
        self.tlib.confirm_replace = None;
        self.tlib.pick = Some(name.clone());
        self.dirty = true;
        self.hint = Some(trf(
            "Kachelsatz „{name}“ gespeichert ({n} Kacheln) — jetzt in jedem Sprite dieses Projekts verfügbar.",
            &[("name", &name), ("n", &ts.tiles.len())],
        ));
    }

    /// Neue Tilemap-Ebene mit einem Satz der Bibliothek.
    pub(crate) fn new_layer_from_lib(&mut self, name: String) {
        let Some(set) = self.project.tileset_lib.get(&name).cloned() else { return };
        let pal = self.project.current_palette();
        let lib = name.clone();
        self.tiles_structural(|sp| {
            let at = sp.layer + 1;
            sp.add_layer(at, lib.clone());
            let tiles = tilelib::from_lib(&set, &pal, &mut sp.free);
            sp.layers[at].tileset = Some(Tileset { tw: set.tw, th: set.th, tiles, lib: Some(lib) });
        });
        // Wer einen fertigen Satz holt, will meist gleich Kacheln setzen.
        self.tiles.tile = 1;
        self.tiles.mode = TileMode::Tiles;
        self.hint = Some(trf("Neue Tilemap-Ebene mit „{name}“ — Kachel wählen und mit dem Stift setzen.", &[("name", &name)]));
    }

    /// Kacheln eines Satzes in die aktive Tilemap-Ebene legen.
    pub(crate) fn use_from_lib(&mut self, name: String) {
        let Some(set) = self.project.tileset_lib.get(&name).cloned() else { return };
        let Some(ts) = self.active_tileset() else { return };
        if (ts.tw, ts.th) != (set.tw, set.th) {
            self.hint = Some(trf(
                "„{name}“ hat Kacheln von {tw} × {th}, diese Ebene {lw} × {lh} — das passt nicht zusammen. „Neue Ebene“ geht immer.",
                &[("name", &name), ("tw", &set.tw), ("th", &set.th), ("lw", &ts.tw), ("lh", &ts.th)],
            ));
            return;
        }
        let pal = self.project.current_palette();
        let lib = name.clone();
        self.tiles_structural(|sp| {
            let l = sp.layer;
            let Some(mut ts) = sp.layers[l].tileset.take() else { return };
            let tiles = tilelib::from_lib(&set, &pal, &mut sp.free);
            let mut ids: Vec<usize> = sp.frames.iter().map(|f| f.cels[l]).collect();
            ids.sort_unstable();
            ids.dedup();
            let mut imgs: Vec<&mut spritebit_core::Image> = sp.images.iter_mut().enumerate().filter(|(i, _)| ids.contains(i)).map(|(_, im)| im).collect();
            tilelib::retile(&mut ts, tiles, &mut imgs);
            ts.lib = Some(lib);
            sp.layers[l].tileset = Some(ts);
        });
        self.hint = Some(trf("Kacheln aus „{name}“ übernommen — die Anordnung ist geblieben.", &[("name", &name)]));
    }

    fn export_lib_set(&mut self, name: &str) {
        let Some(set) = self.project.tileset_lib.get(name) else { return };
        let file: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
        let Some(path) = rfd::FileDialog::new()
            .set_title(tr("Als Datei speichern …"))
            .add_filter("Tileset", &["json"])
            .set_file_name(format!("{file}.tileset.json"))
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, tilelib::file_json(name, set)) {
            Ok(()) => self.hint = Some(trf("Gespeichert: {name}", &[("name", &path.display())])),
            Err(e) => self.error = Some(trf("{path} konnte nicht geschrieben werden: {e}", &[("path", &path.display()), ("e", &e)])),
        }
    }

    fn import_lib_file(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title(tr("Datei laden …")).add_filter("Tileset / PNG", &["json", "png"]).pick_file() else { return };
        let base = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Tileset")
            .trim_end_matches(".json")
            .trim_end_matches(".tileset")
            .trim_end_matches(".png")
            .to_string();
        let png = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("png"));
        let read = std::fs::read(&path).ok();
        let got: Option<(String, LibSet)> = match (read, png) {
            (Some(bytes), true) => {
                // Kachelgröße: die der aktiven Tilemap-Ebene, sonst die eingestellte.
                let (tw, th) = self.active_tileset().map_or((self.tiles.tw, self.tiles.th), |t| (t.tw, t.th));
                image::load_from_memory(&bytes)
                    .ok()
                    .map(|i| i.to_rgba8())
                    .map(|i| (base.clone(), tilelib::tiles_from_image(i.as_raw(), i.width(), i.height(), tw, th)))
            }
            (Some(bytes), false) => tilelib::parse_file(&String::from_utf8_lossy(&bytes)).map(|(n, s)| (if n.is_empty() { base.clone() } else { n }, s)),
            (None, _) => None,
        };
        let Some((name, set)) = got.filter(|(_, s)| !s.tiles.is_empty()) else {
            self.hint = Some(tr("Daraus ließ sich kein Kachelsatz lesen — eine .json von spritebit oder ein PNG-Kachelbild geht.").into());
            return;
        };
        let name = tilelib::free_name(&self.project.tileset_lib, &name);
        self.hint = Some(trf("„{name}“ geladen — {n} Kacheln, {tw} × {th}.", &[("name", &name), ("n", &set.tiles.len()), ("tw", &set.tw), ("th", &set.th)]));
        self.project.tileset_lib.insert(name.clone(), set);
        self.tlib.pick = Some(name);
        self.dirty = true;
    }
}
