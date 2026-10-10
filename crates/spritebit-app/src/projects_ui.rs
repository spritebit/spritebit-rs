//! Projekte in der Desktop-App: ein Name, „Zuletzt geöffnet“, ein
//! Startfenster und „Neues Projekt“ (Name und Speicherort).
//!
//! Projekte bleiben `.sb`-Dateien; der Name steht in der Datei
//! (spritebit_core::Project::name). Fehlt er, gilt der Dateiname. Die Liste
//! der zuletzt geöffneten liegt im Einstellungsordner („recent“, je Zeile
//! `Name<Tab>Pfad`), in Tests nie.

use std::path::{Path, PathBuf};

use eframe::egui;
use spritebit_core::project::Project;

use crate::i18n::{keys, tr, trf};
use crate::{icons, Pending, SpritebitApp, EXT};

/// So viele merkt sich „Zuletzt geöffnet“.
const RECENT_MAX: usize = 10;

#[derive(Default)]
pub(crate) struct ProjectsUi {
    /// Zuletzt geöffnet: (Name, Pfad), neueste zuerst.
    pub(crate) recent: Vec<(String, PathBuf)>,
    /// Nur die echte App schreibt die Liste (Tests nicht).
    persist: bool,
    /// Das Startfenster ist offen.
    pub(crate) start_open: bool,
    /// Dialog „Neues Projekt“: der Name, der gerade getippt wird.
    pub(crate) new_name: Option<String>,
    /// Dialog „Projekt umbenennen“.
    pub(crate) rename: Option<String>,
}

pub(crate) fn parse_recent(text: &str) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for line in text.lines() {
        let Some((name, path)) = line.split_once('\t') else { continue };
        let path = PathBuf::from(path.trim());
        if path.as_os_str().is_empty() || out.iter().any(|(_, p)| *p == path) {
            continue;
        }
        out.push((name.trim().to_string(), path));
        if out.len() == RECENT_MAX {
            break;
        }
    }
    out
}

pub(crate) fn recent_text(list: &[(String, PathBuf)]) -> String {
    list.iter().map(|(n, p)| format!("{}\t{}\n", n.replace(['\t', '\n'], " "), p.display())).collect()
}

/// Ganz nach vorn (und nur einmal).
pub(crate) fn push_recent(list: &mut Vec<(String, PathBuf)>, name: &str, path: &Path) {
    list.retain(|(_, p)| p != path);
    list.insert(0, (name.to_string(), path.to_path_buf()));
    list.truncate(RECENT_MAX);
}

fn recent_file() -> Option<PathBuf> {
    crate::i18n::settings_dir().map(|d| d.join("recent"))
}

/// Dateiname aus einem Projektnamen: nur, was jedes System verträgt.
pub(crate) fn file_stem_for(name: &str) -> String {
    let s: String = name.trim().chars().map(|c| if c.is_alphanumeric() || " _-".contains(c) { c } else { '_' }).collect();
    let s = s.trim().to_string();
    if s.is_empty() { "spritebit".into() } else { s }
}

impl SpritebitApp {
    /// Name für Titelleiste und Listen: der Projektname, sonst der
    /// Dateiname, sonst „Unbenanntes Projekt“.
    pub(crate) fn project_display_name(&self) -> String {
        if !self.project.name.trim().is_empty() {
            return self.project.name.clone();
        }
        self.path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map_or_else(|| tr("Unbenanntes Projekt").to_string(), |s| s.to_string_lossy().into_owned())
    }

    /// Beim Start: die Liste laden; ab jetzt wird sie gespeichert.
    pub(crate) fn load_recent(&mut self) {
        self.projects.persist = true;
        if let Some(text) = recent_file().and_then(|p| std::fs::read_to_string(p).ok()) {
            self.projects.recent = parse_recent(&text);
        }
    }

    fn save_recent(&self) {
        if !self.projects.persist {
            return;
        }
        if let Some(p) = recent_file() {
            let _ = p.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(p, recent_text(&self.projects.recent));
        }
    }

    /// Nach Öffnen oder Speichern: dieses Projekt nach vorn in „Zuletzt geöffnet“.
    pub(crate) fn remember_recent(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let name = self.project_display_name();
        push_recent(&mut self.projects.recent, &name, &path);
        self.save_recent();
    }

    /// Ein Eintrag, der nicht mehr da ist, fliegt aus der Liste.
    pub(crate) fn forget_recent(&mut self, path: &Path) {
        self.projects.recent.retain(|(_, p)| p != path);
        self.save_recent();
    }

    /// Datei → Neues Projekt …: nach dem Namen fragen.
    pub(crate) fn open_new_project(&mut self) {
        self.projects.start_open = false;
        self.projects.new_name = Some(String::new());
    }

    /// Nach Name und Speicherort: anlegen (bei Ungespeichertem erst nachfragen).
    fn new_project_confirmed(&mut self, name: String) {
        if self.dirty {
            self.unsaved_ask = Some(Pending::NewProject(name));
        } else {
            self.create_project_now(name);
        }
    }

    /// Speicherort wählen, dann anlegen. Ohne Speicherort passiert nichts.
    pub(crate) fn create_project_now(&mut self, name: String) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(tr("Wo soll das neue Projekt liegen?"))
            .add_filter(tr("spritebit-Projekt"), &[EXT])
            .set_file_name(format!("{}.{EXT}", file_stem_for(&name)))
            .save_file()
        else {
            return;
        };
        let path = if path.extension().is_none() { path.with_extension(EXT) } else { path };
        self.create_project_at(name, &path);
    }

    /// Ein neues Projekt `name` in `path`: ein leerer Sprite; eigene Paletten
    /// und ihre Materialien kommen mit (wie im Web).
    pub(crate) fn create_project_at(&mut self, name: String, path: &Path) {
        let p = Project { name: name.trim().to_string(), palettes: self.project.palettes.clone(), materials: self.project.materials.clone(), ..Default::default() };
        self.replace_project(p, None);
        self.write_native(path);
        self.hint = Some(trf("Neues Projekt „{name}“ angelegt.", &[("name", &self.project_display_name())]));
    }

    /// Einträge oben im Menü „Datei“.
    pub(crate) fn projects_menu(&mut self, ui: &mut egui::Ui) {
        if ui.add(egui::Button::new(tr("Neues Projekt …")).shortcut_text(keys("Strg+N"))).clicked() {
            self.open_new_project();
            ui.close();
        }
        if ui.add(egui::Button::new(tr("Öffnen …")).shortcut_text(keys("Strg+O"))).clicked() {
            self.open();
        }
        let recent = self.projects.recent.clone();
        ui.add_enabled_ui(!recent.is_empty(), |ui| {
            ui.menu_button(tr("Zuletzt geöffnet"), |ui| {
                for (name, path) in &recent {
                    let label = if name.is_empty() { path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned()) } else { name.clone() };
                    if ui.button(label).on_hover_text(path.display().to_string()).clicked() {
                        self.open_recent(path.clone());
                        ui.close();
                    }
                }
            });
        });
        if ui.button(tr("Projekt umbenennen …")).clicked() {
            self.projects.rename = Some(self.project_display_name());
            ui.close();
        }
    }

    /// Ein Projekt aus „Zuletzt geöffnet“ — bei Ungespeichertem erst nachfragen.
    pub(crate) fn open_recent(&mut self, path: PathBuf) {
        self.projects.start_open = false;
        if self.dirty {
            self.unsaved_ask = Some(Pending::OpenPath(path));
        } else {
            self.open_path(path);
        }
    }

    /// Fenster: Neues Projekt, Umbenennen, Startfenster.
    pub(crate) fn projects_windows(&mut self, ctx: &egui::Context) {
        // Neues Projekt: Name
        if let Some(mut name) = self.projects.new_name.take() {
            let (mut ok, mut cancel) = (false, false);
            egui::Modal::new(egui::Id::new("new-project")).show(ctx, |ui| {
                ui.heading(tr("Neues Projekt"));
                ui.label(tr("Name des Projekts"));
                let r = ui.add(egui::TextEdit::singleline(&mut name).hint_text(tr("z. B. Mein Spiel")).desired_width(280.0));
                r.request_focus();
                ok = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.weak(tr("Es beginnt mit einem leeren Sprite; deine eigenen Paletten kommen mit. Als Nächstes wählst du, wo es gespeichert wird."));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ok |= ui.add_enabled(!name.trim().is_empty(), egui::Button::new(tr("Anlegen …"))).clicked();
                    cancel = ui.button(tr("Abbrechen")).clicked();
                });
            });
            if ok && !name.trim().is_empty() {
                self.new_project_confirmed(name.trim().to_string());
            } else if !cancel && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.projects.new_name = Some(name);
            }
        }

        // Umbenennen
        if let Some(mut name) = self.projects.rename.take() {
            let (mut ok, mut cancel) = (false, false);
            egui::Modal::new(egui::Id::new("rename-project")).show(ctx, |ui| {
                ui.heading(tr("Projekt umbenennen"));
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(280.0));
                r.request_focus();
                ok = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ok |= ui.add_enabled(!name.trim().is_empty(), egui::Button::new(tr("OK"))).clicked();
                    cancel = ui.button(tr("Abbrechen")).clicked();
                });
            });
            if ok && !name.trim().is_empty() {
                self.project.name = name.trim().to_string();
                self.dirty = true;
            } else if !cancel && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.projects.rename = Some(name);
            }
        }

        // Startfenster: nur beim Start, wenn keine Sitzung zurückgeholt wurde.
        if self.projects.start_open {
            let mut close = false;
            let recent = self.projects.recent.clone();
            egui::Modal::new(egui::Id::new("start")).show(ctx, |ui| {
                ui.set_width(420.0);
                ui.horizontal(|ui| {
                    ui.heading("spritebit");
                    ui.weak(crate::VERSION);
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let c = ui.visuals().text_color();
                    if ui.add(egui::Button::image_and_text(icons::image(icons::PLUS, c), tr("Neues Projekt …"))).clicked() {
                        self.open_new_project();
                    }
                    if ui.add(egui::Button::image_and_text(icons::image(icons::OPEN, c), tr("Öffnen …"))).clicked() {
                        close = true;
                        self.open();
                    }
                });
                ui.add_space(8.0);
                ui.strong(tr("Zuletzt geöffnet"));
                if recent.is_empty() {
                    ui.weak(tr("Noch nichts — leg ein neues Projekt an oder öffne eins."));
                }
                for (name, path) in &recent {
                    let label = if name.is_empty() { path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned()) } else { name.clone() };
                    let r = ui.add(egui::Button::new(egui::RichText::new(label).strong()).frame(false)).on_hover_text(path.display().to_string());
                    ui.weak(path.display().to_string());
                    if r.clicked() {
                        self.open_recent(path.clone());
                    }
                    ui.add_space(2.0);
                }
                ui.add_space(8.0);
                if ui.button(tr("Leer weitermachen")).on_hover_text(tr("Mit einem leeren, noch ungespeicherten Projekt anfangen")).clicked() {
                    close = true;
                }
            });
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.projects.start_open = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zuletzt_geoeffnet_lesen_und_schreiben() {
        let mut list = parse_recent("Mein Spiel\tC:\\a\\spiel.sb\nkaputt\n\tC:\\b.sb\nDoppelt\tC:\\a\\spiel.sb\n");
        assert_eq!(list.len(), 2, "kaputte Zeile weg, doppelte nur einmal");
        assert_eq!(list[0].0, "Mein Spiel");
        push_recent(&mut list, "Neu", Path::new("C:\\n.sb"));
        push_recent(&mut list, "Mein Spiel", Path::new("C:\\a\\spiel.sb"));
        assert_eq!(list[0].1, PathBuf::from("C:\\a\\spiel.sb"), "nach vorn");
        assert_eq!(list.len(), 3);
        assert_eq!(parse_recent(&recent_text(&list)), list);
        for k in 0..20 {
            push_recent(&mut list, "x", &PathBuf::from(format!("C:\\{k}.sb")));
        }
        assert_eq!(list.len(), RECENT_MAX);
    }

    #[test]
    fn dateiname_aus_dem_projektnamen() {
        assert_eq!(file_stem_for("Mein Spiel"), "Mein Spiel");
        assert_eq!(file_stem_for("Level: 1/2"), "Level_ 1_2");
        assert_eq!(file_stem_for("  "), "spritebit");
        assert_eq!(file_stem_for("Größe"), "Größe");
    }
}
