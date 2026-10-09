//! Sprites verwalten: Liste, anlegen (Name, Palette, Größe), umbenennen,
//! duplizieren, Größe ändern, löschen — und „Alles zurücksetzen“.
//!
//! Rechtsklick auf einen Sprite öffnet sein Menü, Doppelklick benennt um.

use eframe::egui;
use spritebit_core::transform::{self as tf, TransformResult};
use spritebit_core::{builtin, History, Project, Sprite, MAX_SIDE};

use crate::i18n::{tr, trf};
use crate::SpritebitApp;

pub(crate) enum SpriteDialog {
    New { name: String, palette: String, w: u32, h: u32 },
    Rename { i: usize, name: String },
    Resize { i: usize, w: u32, h: u32, centered: bool },
    Delete { i: usize },
    Reset,
}

/// Größen zum schnellen Wählen.
const PRESETS: [u32; 8] = [16, 24, 32, 64, 128, 256, 1024, 4096];

impl SpritebitApp {
    pub(crate) fn open_new_sprite(&mut self) {
        let sp = self.sprite();
        self.sprite_dialog = Some(SpriteDialog::New {
            name: self.project.fresh_name(),
            palette: sp.palette.clone(),
            w: sp.width,
            h: sp.height,
        });
    }

    pub(crate) fn create_sprite(&mut self, name: String, palette: String, w: u32, h: u32) {
        let name = if name.trim().is_empty() { self.project.fresh_name() } else { name.trim().to_string() };
        if let Ok(mut sp) = Sprite::new(name, w, h) {
            sp.palette = palette;
            self.project.sprites.push(sp);
            self.histories.push(History::default());
            self.select_sprite(self.project.sprites.len() - 1);
            self.dirty = true;
        }
    }

    pub(crate) fn duplicate_sprite(&mut self, i: usize) {
        self.finish_rotate();
        self.deselect();
        let mut copy = self.project.sprites[i].clone();
        copy.name = trf("{name} Kopie", &[("name", &copy.name)]);
        self.project.sprites.insert(i + 1, copy);
        self.histories.insert(i + 1, History::default());
        crate::tabs::on_insert(&mut self.tabs, i + 1);
        self.project.current = i; // damit select_sprite wirklich wechselt
        self.select_sprite(i + 1);
        self.changed();
    }

    pub(crate) fn delete_sprite(&mut self, i: usize) {
        self.finish_rotate();
        self.deselect();
        self.project.sprites.remove(i);
        self.histories.remove(i);
        crate::tabs::on_remove(&mut self.tabs, i);
        if self.project.sprites.is_empty() {
            self.project.sprites.push(Sprite::new(self.project.fresh_name(), 64, 64).expect("gültige Größe"));
            self.histories.push(History::default());
        }
        let cur = self.project.current;
        self.project.current = if cur > i { cur - 1 } else { cur.min(self.project.sprites.len() - 1) };
        self.clamp_color();
        self.fit_pending = true;
        self.changed();
    }

    /// Die Liste in der linken Leiste.
    pub(crate) fn sprite_list(&mut self, ui: &mut egui::Ui) {
        let mut pick = None;
        let mut action: Option<SpriteDialog> = None;
        let mut duplicate = None;
        for (i, sp) in self.project.sprites.iter().enumerate() {
            let label = format!("{}  ·  {}×{}", sp.name, sp.width, sp.height);
            let r = ui.selectable_label(i == self.project.current, label);
            if r.clicked() {
                pick = Some(i);
            }
            if r.double_clicked() {
                action = Some(SpriteDialog::Rename { i, name: sp.name.clone() });
            }
            r.context_menu(|ui| {
                if ui.button(tr("Umbenennen …")).clicked() {
                    action = Some(SpriteDialog::Rename { i, name: sp.name.clone() });
                    ui.close();
                }
                if ui.button(tr("Duplizieren")).clicked() {
                    duplicate = Some(i);
                    ui.close();
                }
                if ui.button(tr("Größe ändern …")).clicked() {
                    action = Some(SpriteDialog::Resize { i, w: sp.width, h: sp.height, centered: true });
                    ui.close();
                }
                ui.separator();
                if ui.button(tr("Löschen …")).clicked() {
                    action = Some(SpriteDialog::Delete { i });
                    ui.close();
                }
            });
        }
        if let Some(i) = pick {
            self.select_sprite(i);
        }
        if let Some(i) = duplicate {
            self.duplicate_sprite(i);
        }
        if action.is_some() {
            self.sprite_dialog = action;
        }
        ui.weak(tr("Rechtsklick: umbenennen, duplizieren, Größe, löschen"));
    }

    pub(crate) fn sprite_dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.sprite_dialog else { return };
        let mut ok = false;
        let mut cancel = false;
        let palettes: Vec<String> =
            builtin::BUILTIN.iter().map(|(n, _)| n.to_string()).chain(self.project.palettes.iter().map(|p| p.name.clone())).collect();
        let buttons = |ui: &mut egui::Ui, ok_label: &str, ok: &mut bool, cancel: &mut bool| {
            ui.separator();
            ui.horizontal(|ui| {
                *ok = ui.button(ok_label).clicked();
                *cancel = ui.button(tr("Abbrechen")).clicked();
            });
        };
        match dialog {
            SpriteDialog::New { name, palette, w, h } => {
                egui::Modal::new(egui::Id::new("new-sprite")).show(ctx, |ui| {
                    ui.heading(tr("Neuer Sprite"));
                    egui::Grid::new("new-grid").num_columns(2).show(ui, |ui| {
                        ui.label(tr("Name"));
                        ui.text_edit_singleline(name);
                        ui.end_row();
                        ui.label(tr("Farbpalette"));
                        egui::ComboBox::from_id_salt("new-pal").selected_text(palette.as_str()).show_ui(ui, |ui| {
                            for p in &palettes {
                                ui.selectable_value(palette, p.clone(), p);
                            }
                        });
                        ui.end_row();
                        ui.label(tr("Größe"));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(w).range(1..=MAX_SIDE));
                            ui.label("×");
                            ui.add(egui::DragValue::new(h).range(1..=MAX_SIDE));
                        });
                        ui.end_row();
                    });
                    ui.horizontal_wrapped(|ui| {
                        for s in PRESETS {
                            if ui.button(format!("{s}")).clicked() {
                                *w = s;
                                *h = s;
                            }
                        }
                    });
                    buttons(ui, tr("Erstellen"), &mut ok, &mut cancel);
                });
            }
            SpriteDialog::Rename { name, .. } => {
                egui::Modal::new(egui::Id::new("rename-sprite")).show(ctx, |ui| {
                    ui.heading(tr("Sprite umbenennen"));
                    let r = ui.text_edit_singleline(name);
                    r.request_focus();
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        ok = true;
                    }
                    buttons(ui, tr("Speichern"), &mut ok, &mut cancel);
                });
            }
            SpriteDialog::Resize { w, h, centered, .. } => {
                egui::Modal::new(egui::Id::new("resize-sprite")).show(ctx, |ui| {
                    ui.heading(tr("Leinwand ändern"));
                    ui.horizontal(|ui| {
                        ui.label(tr("Breite"));
                        ui.add(egui::DragValue::new(w).range(1..=MAX_SIDE));
                        ui.label(tr("Höhe"));
                        ui.add(egui::DragValue::new(h).range(1..=MAX_SIDE));
                    });
                    ui.horizontal(|ui| {
                        ui.label(tr("Anker"));
                        ui.selectable_value(centered, true, tr("mittig"));
                        ui.selectable_value(centered, false, tr("oben links"));
                    });
                    ui.weak(tr("Die Leinwand wird nur größer oder kleiner — die Pixel behalten ihre Größe."));
                    buttons(ui, tr("Ändern"), &mut ok, &mut cancel);
                });
            }
            SpriteDialog::Delete { i } => {
                let name = self.project.sprites.get(*i).map(|s| s.name.clone()).unwrap_or_default();
                egui::Modal::new(egui::Id::new("delete-sprite")).show(ctx, |ui| {
                    ui.heading(trf("Sprite „{name}“ löschen?", &[("name", &name)]));
                    ui.label(tr("Das lässt sich nicht rückgängig machen."));
                    buttons(ui, tr("Löschen"), &mut ok, &mut cancel);
                });
            }
            SpriteDialog::Reset => {
                egui::Modal::new(egui::Id::new("reset-all")).show(ctx, |ui| {
                    ui.heading(tr("Alles zurücksetzen?"));
                    ui.label(tr("Alle Sprites und eigenen Paletten dieses Projekts werden verworfen."));
                    buttons(ui, tr("Zurücksetzen"), &mut ok, &mut cancel);
                });
            }
        }
        if cancel {
            self.sprite_dialog = None;
            return;
        }
        if !ok {
            return;
        }
        match self.sprite_dialog.take() {
            Some(SpriteDialog::New { name, palette, w, h }) => self.create_sprite(name, palette, w, h),
            Some(SpriteDialog::Rename { i, name }) => {
                let name = name.trim().to_string();
                if !name.is_empty() && i < self.project.sprites.len() {
                    self.histories[i].record(&self.project.sprites[i]);
                    self.project.sprites[i].name = name;
                    self.changed();
                }
            }
            Some(SpriteDialog::Resize { i, w, h, centered }) => {
                if i == self.project.current {
                    self.finish_rotate();
                    self.deselect();
                }
                if i < self.project.sprites.len() {
                    self.histories[i].record(&self.project.sprites[i]);
                    let r = tf::resize_canvas(&mut self.project.sprites[i], w, h, centered);
                    if matches!(r, TransformResult::Done { .. }) {
                        if i == self.project.current {
                            self.fit_pending = true;
                        }
                        self.changed();
                    } else {
                        self.histories[i].drop_last();
                    }
                }
            }
            Some(SpriteDialog::Delete { i }) => {
                if i < self.project.sprites.len() {
                    self.delete_sprite(i);
                }
            }
            Some(SpriteDialog::Reset) => {
                self.replace_project(Project::default(), None);
                self.dirty = true;
            }
            None => {}
        }
    }
}
