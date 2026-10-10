//! Update-Hinweis: beim Start einmal bei GitHub nach der neuesten Release
//! fragen und, wenn es eine neuere gibt, oben ein Band zeigen —
//! „Neue Version 1.0.3 verfügbar“ mit Aktualisieren und Später. „Später“
//! merkt sich nichts: beim nächsten Start ist das Band wieder da, solange
//! es eine neuere Version gibt.
//!
//! Die Abfrage läuft in einem eigenen Thread, die App startet also nicht
//! langsamer; ohne Internet passiert einfach nichts. Abschaltbar unter
//! Hilfe → „Beim Start nach Updates suchen“, gemerkt in `update.json` im
//! Einstellungsordner. Hilfe → „Jetzt nach Updates suchen“ fragt sofort nach.
//!
//! „Was ist neu?“: Jede Version hat Notizen in `notes/<version>.md` (im
//! Repo, Abschnitte `## Deutsch` und `## English`). Die eigenen bettet
//! build.rs ein — nach einem Update zeigt die App sie einmal von selbst.
//! Die der neuen Version hängt release.yml als `notes.md` an die Release;
//! das Band lädt sie, sobald es eine neuere Version meldet.
//!
//! Die Rechnung (Versionen vergleichen, Antwort lesen) steht in reinen
//! Funktionen mit Tests; die Tests gehen nie ins Netz — die Abfrage startet
//! nur `main()`, nicht `SpritebitApp::new()`.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui::{self, Color32};

use crate::i18n::{lang, tr, trf, Lang};
use crate::{SpritebitApp, VERSION};

/// Neueste Release von spritebit-rs (öffentliche GitHub-API, ohne Konto).
const LATEST_API: &str = "https://api.github.com/repos/spritebit/spritebit-rs/releases/latest";
/// Dorthin führt „Herunterladen“: die Release-Seite mit Download und Änderungen.
pub(crate) const LATEST_PAGE: &str = "https://github.com/spritebit/spritebit-rs/releases/latest";
/// Alle Versionen mit ihren Notizen.
const RELEASES_PAGE: &str = "https://github.com/spritebit/spritebit-rs/releases";

/// Notizen dieser Version (`notes/<version>.md`, eingebettet von build.rs).
pub(crate) const OWN_NOTES: &str = include_str!(concat!(env!("OUT_DIR"), "/notes.md"));
/// Notizen einer Release — Datei neben der .exe (release.yml).
const NOTES_ASSET: &str = "notes.md";
/// Länger sind Notizen nie — eine Grenze gegen Unsinn aus dem Netz.
const MAX_NOTES: u64 = 64 * 1024;

/// Die Punkte der Notizen in einer Sprache: Abschnitt `## Deutsch`,
/// `## English` oder `## Österreichisch`, darin Zeilen mit `- `; weitere
/// Zeilen gehören zum Punkt davor. Fehlt die Sprache, gilt Deutsch.
pub(crate) fn note_points(md: &str, lang: Lang) -> Vec<String> {
    let section = |name: &str| {
        let mut out: Vec<String> = Vec::new();
        let mut inside = false;
        for line in md.lines() {
            let t = line.trim();
            if let Some(h) = t.strip_prefix("## ") {
                inside = h.trim().eq_ignore_ascii_case(name);
                continue;
            }
            if !inside || t.is_empty() {
                continue;
            }
            // Fett und Code-Schrift kann ein Label nicht — die Zeichen weg.
            let t = t.replace("**", "").replace('`', "");
            if let Some(p) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
                out.push(p.trim().to_string());
            } else if let Some(last) = out.last_mut() {
                last.push(' ');
                last.push_str(&t);
            }
        }
        out
    };
    let name = match lang {
        Lang::De => "Deutsch",
        Lang::En => "English",
        Lang::At => "Österreichisch",
    };
    let points = section(name);
    if points.is_empty() {
        section("Deutsch")
    } else {
        points
    }
}

/// Notizen der Version `version` von ihrer Release (blockierend — eigener Thread).
fn fetch_notes(version: &str) -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().into();
    let ua = format!("spritebit/{VERSION}");
    let mut resp = agent.get(crate::selfupdate::asset_url(version, NOTES_ASSET)).header("User-Agent", &ua).call().ok()?;
    let text = resp.body_mut().with_config().limit(MAX_NOTES).read_to_string().ok()?;
    (!note_points(&text, Lang::De).is_empty()).then_some(text)
}

/// Geht „Jetzt aktualisieren“ hier? (In der Entwickler-Fassung mit
/// SPRITEBIT_FAKE_UPDATE zum Ansehen auch.)
fn can_self_update() -> bool {
    crate::selfupdate::supported() || (cfg!(debug_assertions) && std::env::var_os("SPRITEBIT_FAKE_UPDATE").is_some())
}

/// "v1.2.3" oder "1.2.3" → (1, 2, 3).
pub(crate) fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut it = s.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u32>().ok());
    let v = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(v)
}

/// Ist `latest` neuer als `current`? Unlesbares zählt nie als neuer.
pub(crate) fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_version(latest), parse_version(current)), (Some(a), Some(b)) if a > b)
}

/// Version ("1.2.3") aus der Antwort von /releases/latest.
pub(crate) fn version_from_release(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    if v.get("draft").and_then(|d| d.as_bool()) == Some(true) {
        return None;
    }
    let tag = v.get("tag_name")?.as_str()?;
    let (a, b, c) = parse_version(tag)?;
    Some(format!("{a}.{b}.{c}"))
}

/// Version aus der Weiterleitung von `/releases/latest`
/// (`…/releases/tag/v1.2.3`).
pub(crate) fn version_from_location(location: &str) -> Option<String> {
    let tag = location.trim().rsplit_once("/releases/tag/")?.1;
    let (a, b, c) = parse_version(tag.split(['?', '#']).next()?)?;
    Some(format!("{a}.{b}.{c}"))
}

/// Nach dem Start die eigenen Notizen zeigen? Nur wenn vorher eine ältere
/// Version lief (oder eine, die sich das noch nicht merkte) und es Notizen gibt.
pub(crate) fn show_own_notes(seen: Option<&str>, current: &str, notes: &str) -> bool {
    seen.is_none_or(|s| is_newer(current, s)) && !note_points(notes, Lang::De).is_empty()
}

/// Die Abfrage selbst (blockierend — läuft im eigenen Thread).
///
/// Zuerst die normale Release-Seite: sie leitet auf `…/tag/vX.Y.Z` weiter,
/// und diese Weiterleitung zählt nicht zum Kontingent der GitHub-API (ohne
/// Anmeldung 60 Abfragen je Stunde und Internetanschluss — ist es
/// aufgebraucht, käme sonst nie ein Hinweis). Die API nur als Rückfall.
fn fetch_latest() -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(8))).max_redirects(0).build().into();
    let ua = format!("spritebit/{VERSION}");
    let from_page = agent
        .get(LATEST_PAGE)
        .header("User-Agent", &ua)
        .call()
        .ok()
        .and_then(|r| r.headers().get("location").and_then(|l| l.to_str().ok()).and_then(version_from_location));
    if from_page.is_some() {
        return from_page;
    }
    let mut resp = agent.get(LATEST_API).header("User-Agent", &ua).header("Accept", "application/vnd.github+json").call().ok()?;
    let body = resp.body_mut().read_to_string().ok()?;
    version_from_release(&body)
}

/// Welche Notizen das Fenster „Was ist neu?“ zeigt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotesView {
    /// Die dieser Version (eingebettet).
    Own,
    /// Die der neueren Version aus dem Band.
    Next,
}

/// Notizen der neueren Version.
#[derive(Default)]
pub(crate) enum NextNotes {
    #[default]
    Idle,
    Loading(Receiver<Option<String>>),
    Ready(String),
    Failed,
}

/// Zustand des Update-Hinweises.
pub(crate) struct UpdateState {
    /// Beim Start nachsehen? (Hilfe-Menü)
    pub check: bool,
    /// Diese Version nicht mehr anbieten.
    /// Gefundene neuere Version, solange das Band offen ist.
    pub available: Option<String>,
    rx: Option<Receiver<Option<String>>>,
    /// Die laufende Abfrage kam aus dem Hilfe-Menü: Ergebnis immer melden.
    manual: bool,
    /// Erst nach dem Laden speichern (Tests schreiben nichts).
    persist: bool,
    /// „Jetzt aktualisieren“ (selfupdate.rs).
    pub install: crate::selfupdate::Install,
    /// Welche Version wohin — solange geladen wird.
    pub install_target: Option<(String, std::path::PathBuf)>,
    /// Zuletzt gestartete Version — ist diese neuer, kommen ihre Notizen.
    pub seen: Option<String>,
    /// Offenes Fenster „Was ist neu?“.
    pub notes_view: Option<NotesView>,
    /// Notizen der Version in `available`.
    pub next_notes: NextNotes,
}

impl Default for UpdateState {
    fn default() -> Self {
        UpdateState {
            check: true,
            available: None,
            rx: None,
            manual: false,
            persist: false,
            install: Default::default(),
            install_target: None,
            seen: None,
            notes_view: None,
            next_notes: NextNotes::Idle,
        }
    }
}

fn settings_path() -> Option<std::path::PathBuf> {
    crate::i18n::settings_dir().map(|d| d.join("update.json"))
}

impl SpritebitApp {
    /// Beim Start: Einstellungen laden und — wenn gewünscht — nachsehen.
    pub(crate) fn start_update_check(&mut self, ctx: &egui::Context) {
        self.update.persist = true;
        let stored = settings_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
        if let Some(v) = &stored {
            if let Some(c) = v.get("check").and_then(|c| c.as_bool()) {
                self.update.check = c;
            }
            self.update.seen = v.get("seen").and_then(|s| s.as_str()).map(str::to_string);
        }
        // Neue Version (per Update oder von Hand): einmal ihre Notizen zeigen.
        // Beim allerersten Start nicht — da ist alles neu.
        if stored.is_some() && show_own_notes(self.update.seen.as_deref(), VERSION, OWN_NOTES) {
            self.update.notes_view = Some(NotesView::Own);
        }
        if self.update.seen.as_deref() != Some(VERSION) {
            self.update.seen = Some(VERSION.to_string());
            self.save_update_settings();
        }
        if self.update.check {
            self.spawn_update_check(ctx);
        }
        // Nur Entwickler-Fassung: SPRITEBIT_FAKE_UPDATE=9.9.9 zeigt das Band, ohne
        // dass es eine neue Release braucht (zum Ansehen und Gestalten).
        if cfg!(debug_assertions) {
            if let Ok(v) = std::env::var("SPRITEBIT_FAKE_UPDATE") {
                self.update.available = Some(v);
                self.update.rx = None;
                // Zum Ansehen: die eigenen Notizen stehen für die neue Version.
                self.update.next_notes = NextNotes::Ready(OWN_NOTES.to_string());
            }
        }
    }

    fn spawn_update_check(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(fetch_latest());
            ctx.request_repaint();
        });
        self.update.rx = Some(rx);
    }

    fn save_update_settings(&self) {
        if !self.update.persist {
            return;
        }
        if let Some(p) = settings_path() {
            let _ = p.parent().map(std::fs::create_dir_all);
            let v = serde_json::json!({ "check": self.update.check, "seen": self.update.seen });
            let _ = std::fs::write(p, v.to_string());
        }
    }

    /// Jeden Frame: ist die Antwort da? Sind die Notizen da?
    pub(crate) fn poll_update(&mut self, ctx: &egui::Context) {
        if let NextNotes::Loading(rx) = &self.update.next_notes {
            if let Ok(found) = rx.try_recv() {
                self.update.next_notes = found.map_or(NextNotes::Failed, NextNotes::Ready);
            }
        }
        let Some(rx) = &self.update.rx else { return };
        let Ok(found) = rx.try_recv() else { return };
        self.update.rx = None;
        let manual = std::mem::take(&mut self.update.manual);
        match found {
            Some(v) if is_newer(&v, VERSION) => {
                self.spawn_notes_fetch(&v, ctx);
                self.update.available = Some(v);
            }
            _ if !manual => {}
            Some(_) => self.error = Some(trf("spritebit {v} ist die neueste Version.", &[("v", &VERSION)])),
            None => self.error = Some(tr("GitHub ist gerade nicht erreichbar — später noch einmal versuchen.").into()),
        }
    }

    /// Hilfe → „Jetzt nach Updates suchen“: sofort fragen und das Ergebnis
    /// auf jeden Fall melden.
    pub(crate) fn check_updates_now(&mut self, ctx: &egui::Context) {
        self.update.manual = true;
        self.spawn_update_check(ctx);
    }

    /// Notizen der neueren Version gleich mitladen — „Was ist neu?“ soll
    /// sofort etwas zeigen.
    fn spawn_notes_fetch(&mut self, version: &str, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        let (ctx, v) = (ctx.clone(), version.to_string());
        std::thread::spawn(move || {
            let _ = tx.send(fetch_notes(&v));
            ctx.request_repaint();
        });
        self.update.next_notes = NextNotes::Loading(rx);
    }

    /// Rahmen des Bands: Blau für „neu da“ und „installiert“, Rot bei einem Fehler.
    pub(crate) fn banner_frame(&self) -> egui::Frame {
        let failed = matches!(self.update.install, crate::selfupdate::Install::Failed(_));
        let (fill, line) = if failed {
            (Color32::from_rgb(92, 34, 40), Color32::from_rgb(224, 108, 117))
        } else {
            (Color32::from_rgb(28, 58, 110), Color32::from_rgb(110, 168, 254))
        };
        egui::Frame::new().fill(fill).stroke(egui::Stroke::new(1.0, line)).inner_margin(egui::Margin::symmetric(14, 10))
    }

    /// Hauptknopf im Band: hell gefüllt, damit man ihn sofort sieht.
    fn banner_button(ui: &mut egui::Ui, text: &str, tip: &str) -> bool {
        let label = egui::RichText::new(text).strong().size(15.0).color(Color32::from_rgb(16, 32, 64));
        ui.add(egui::Button::new(label).fill(Color32::from_rgb(170, 205, 255)).corner_radius(6.0).min_size(egui::vec2(0.0, 28.0))).on_hover_text(tip).clicked()
    }

    /// Nebenknopf im Band (Später, Schließen): dezent.
    fn banner_link(ui: &mut egui::Ui, text: &str) -> bool {
        ui.add(egui::Button::new(egui::RichText::new(text).color(Color32::from_rgb(214, 226, 245))).frame(false)).clicked()
    }

    fn banner_title(ui: &mut egui::Ui, icon: &str, text: &str) {
        ui.label(egui::RichText::new(icon).size(20.0).color(Color32::WHITE));
        ui.label(egui::RichText::new(text).strong().size(16.0).color(Color32::WHITE));
    }

    /// Inhalt des Bands oben (ui() legt das Panel nur an, solange eine
    /// neuere Version da ist oder ein Update läuft).
    pub(crate) fn update_banner(&mut self, ui: &mut egui::Ui) {
        use crate::selfupdate::Install;
        let soft = Color32::from_rgb(196, 212, 238);
        // Läuft schon ein Update (oder ist fertig), zeigt das Band dessen Stand.
        match &self.update.install {
            Install::Running(_) => {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(18.0).color(Color32::WHITE));
                    Self::banner_title(ui, "", tr("Neue Version wird geladen und geprüft …"));
                });
                return;
            }
            Install::Done { version, exe } => {
                let (version, exe) = (version.clone(), exe.clone());
                let (mut restart, mut later) = (false, false);
                ui.horizontal(|ui| {
                    Self::banner_title(ui, "✔", &trf("spritebit {new} ist installiert.", &[("new", &version)]));
                    ui.add_space(8.0);
                    restart =
                        Self::banner_button(ui, tr("Jetzt neu starten"), tr("Die App startet neu und macht genau hier weiter — auch Ungespeichertes bleibt."));
                    later = Self::banner_link(ui, tr("Später"));
                });
                if restart {
                    self.restart_after_update(&version, &exe);
                }
                if later {
                    self.update.install = Install::Idle;
                    self.update.available = None;
                }
                return;
            }
            Install::Failed(e) => {
                let e = e.clone();
                let mut close = false;
                ui.horizontal_wrapped(|ui| {
                    Self::banner_title(ui, "⚠", &trf("Aktualisieren hat nicht geklappt: {e}", &[("e", &e)]));
                    ui.add_space(8.0);
                    ui.hyperlink_to(egui::RichText::new(tr("Von Hand herunterladen")).strong().color(Color32::WHITE), LATEST_PAGE);
                    close = Self::banner_link(ui, tr("Schließen"));
                });
                if close {
                    self.update.install = Install::Idle;
                    self.update.available = None;
                }
                return;
            }
            Install::Idle => {}
        }
        let Some(v) = self.update.available.clone() else { return };
        let (mut later, mut install, mut notes) = (false, false, false);
        ui.horizontal_wrapped(|ui| {
            Self::banner_title(ui, "⬆", &trf("Neue Version {new} verfügbar", &[("new", &v)]));
            ui.label(egui::RichText::new(trf("(du hast {old})", &[("old", &VERSION)])).color(soft));
            ui.add_space(8.0);
            if can_self_update() {
                install = Self::banner_button(
                    ui,
                    tr("Jetzt aktualisieren"),
                    tr("Lädt die neue Version, prüft sie und tauscht die App aus — ohne ZIP und ohne Entpacken."),
                );
            } else {
                ui.hyperlink_to(egui::RichText::new(tr("Herunterladen")).strong().color(Color32::WHITE), LATEST_PAGE);
            }
            notes = ui.add(egui::Button::new(egui::RichText::new(tr("Was ist neu?")).color(Color32::WHITE).underline()).frame(false)).clicked();
            ui.add_space(8.0);
            later = Self::banner_link(ui, tr("Später"));
        });
        if notes {
            self.update.notes_view = Some(NotesView::Next);
        }
        if install {
            self.start_install(v.clone(), ui.ctx());
            return;
        }
        // Nur für diese Sitzung — nach dem nächsten Start kommt das Band wieder.
        if later {
            self.update.available = None;
        }
    }

    /// Fenster „Was ist neu?“: die Punkte der Notizen, bei einer neueren
    /// Version samt „Jetzt aktualisieren“.
    pub(crate) fn notes_window(&mut self, ctx: &egui::Context) {
        use crate::selfupdate::Install;
        let Some(view) = self.update.notes_view else { return };
        let (title, points, loading) = match (view, &self.update.available) {
            (NotesView::Own, _) => (trf("Neu in spritebit {v}", &[("v", &VERSION)]), note_points(OWN_NOTES, lang()), false),
            (NotesView::Next, Some(v)) => {
                let title = trf("Neu in spritebit {v}", &[("v", v)]);
                match &self.update.next_notes {
                    NextNotes::Ready(t) => (title, note_points(t, lang()), false),
                    NextNotes::Loading(_) => (title, Vec::new(), true),
                    NextNotes::Idle | NextNotes::Failed => (title, Vec::new(), false),
                }
            }
            // Das Band ist weg (Später): das Fenster auch.
            (NotesView::Next, None) => {
                self.update.notes_view = None;
                return;
            }
        };
        let can_install = view == NotesView::Next && matches!(self.update.install, Install::Idle) && can_self_update();
        let (mut open, mut close, mut install) = (true, false, false);
        egui::Window::new(title).id(egui::Id::new("whats-new")).collapsible(false).resizable(false).default_width(420.0).open(&mut open).show(ctx, |ui| {
            ui.set_max_width(420.0);
            if loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(tr("Notizen werden geladen …"));
                });
            } else if points.is_empty() {
                ui.label(tr("Zu dieser Version gibt es keine Notizen."));
            }
            for p in &points {
                ui.horizontal_top(|ui| {
                    ui.label("•");
                    ui.add(egui::Label::new(p).wrap());
                });
                ui.add_space(2.0);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if can_install {
                    install = ui.button(egui::RichText::new(tr("Jetzt aktualisieren")).strong()).clicked();
                }
                close = ui.button(tr("OK")).clicked();
                ui.add_space(8.0);
                ui.hyperlink_to(tr("Alle Versionen auf GitHub"), RELEASES_PAGE);
            });
        });
        if !open || close || install {
            self.update.notes_view = None;
        }
        if install {
            if let Some(v) = self.update.available.clone() {
                self.start_install(v, ctx);
            }
        }
    }

    /// Einträge im Hilfe-Menü.
    pub(crate) fn update_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button(tr("Was ist neu?")).on_hover_text(tr("Was diese Version mitbringt")).clicked() {
            self.update.notes_view = Some(NotesView::Own);
            ui.close();
        }
        let busy = self.update.rx.is_some() || self.update.available.is_some() || !matches!(self.update.install, crate::selfupdate::Install::Idle);
        if ui
            .add_enabled(!busy, egui::Button::new(tr("Jetzt nach Updates suchen")))
            .on_hover_text(tr("Fragt sofort bei GitHub nach, ob es eine neuere Version gibt."))
            .clicked()
        {
            self.check_updates_now(ui.ctx());
            ui.close();
        }
        let before = self.update.check;
        ui.checkbox(&mut self.update.check, tr("Beim Start nach Updates suchen")).on_hover_text(tr(
            "Fragt beim Start einmal bei GitHub nach, ob es eine neuere Version gibt. Dabei wird nur die neueste Versionsnummer abgerufen — keine Daten aus deinen Projekten.",
        ));
        if self.update.check != before {
            self.save_update_settings();
            if self.update.check && self.update.persist {
                self.spawn_update_check(ui.ctx());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versionen_lesen() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.0.10"), Some((1, 0, 10)));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("v1.x.3"), None);
    }

    #[test]
    fn neuer_vergleicht_zahlen_nicht_text() {
        assert!(is_newer("1.0.10", "1.0.9"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "1.0.0"));
        assert!(!is_newer("kaputt", "1.0.0"));
    }

    #[test]
    fn antwort_von_github() {
        let json = r#"{"tag_name":"v1.0.3","name":"v1.0.3","draft":false,"prerelease":false}"#;
        assert_eq!(version_from_release(json).as_deref(), Some("1.0.3"));
        assert_eq!(version_from_release(r#"{"tag_name":"v1.0.3","draft":true}"#), None);
        assert_eq!(version_from_release(r#"{"message":"Not Found"}"#), None);
        assert_eq!(version_from_release("kein json"), None);
    }

    /// Fragt wirklich bei GitHub nach — läuft nur mit `cargo test -- --ignored`.
    #[test]
    #[ignore = "braucht Internet"]
    fn echte_abfrage_bei_github() {
        let v = fetch_latest().expect("Antwort von GitHub");
        assert!(parse_version(&v).is_some(), "{v}");
    }

    #[test]
    fn version_aus_der_weiterleitung() {
        assert_eq!(version_from_location("https://github.com/spritebit/spritebit-rs/releases/tag/v1.0.5").as_deref(), Some("1.0.5"));
        assert_eq!(version_from_location("/spritebit/spritebit-rs/releases/tag/v2.10.0?x=1").as_deref(), Some("2.10.0"));
        assert_eq!(version_from_location("https://github.com/spritebit/spritebit-rs/releases"), None, "ohne Release");
        assert_eq!(version_from_location("https://github.com/x/releases/tag/nightly"), None);
    }

    const NOTES: &str = "## Deutsch\n\n- Erster **Punkt**\n  geht weiter\n- Zweiter mit `code`\n\n## English\n\n- First point\n";

    #[test]
    fn notizen_lesen() {
        assert_eq!(note_points(NOTES, Lang::De), ["Erster Punkt geht weiter", "Zweiter mit code"]);
        assert_eq!(note_points(NOTES, Lang::En), ["First point"]);
        assert_eq!(note_points(NOTES, Lang::At), note_points(NOTES, Lang::De), "ohne eigenen Abschnitt: Deutsch");
        assert_eq!(note_points("## English\n- only\n", Lang::En), ["only"]);
        assert!(note_points("", Lang::De).is_empty());
        assert!(note_points("- ohne Abschnitt\n", Lang::De).is_empty());
    }

    #[test]
    fn diese_version_hat_notizen() {
        assert!(!note_points(OWN_NOTES, Lang::De).is_empty(), "notes/{VERSION}.md fehlt oder hat keinen Abschnitt „## Deutsch“");
        assert!(!note_points(OWN_NOTES, Lang::En).is_empty());
    }

    #[test]
    fn eigene_notizen_einmal_nach_neuer_version() {
        assert!(show_own_notes(Some("1.1.9"), "1.1.10", NOTES));
        assert!(show_own_notes(None, "1.1.10", NOTES), "Einstellungen von vor dieser Funktion");
        assert!(!show_own_notes(Some("1.1.10"), "1.1.10", NOTES), "schon gesehen");
        assert!(!show_own_notes(Some("1.2.0"), "1.1.10", NOTES), "zurückgestuft");
        assert!(!show_own_notes(Some("1.1.9"), "1.1.10", ""), "ohne Notizen");
    }

    #[test]
    fn von_hand_suchen_meldet_immer_etwas() {
        let ctx = egui::Context::default();
        let mut app = crate::SpritebitApp::new();
        // Antwort von Hand einspeisen statt ins Netz zu gehen.
        let answer = |app: &mut crate::SpritebitApp, v: Option<&str>| {
            let (tx, rx) = mpsc::channel();
            tx.send(v.map(str::to_string)).unwrap();
            app.update.rx = Some(rx);
            app.update.manual = true;
            app.poll_update(&ctx);
        };
        answer(&mut app, Some("9.9.9"));
        assert_eq!(app.update.available.as_deref(), Some("9.9.9"));
        app.update.available = None;
        answer(&mut app, Some(VERSION));
        assert!(app.error.as_deref().is_some_and(|h| h.contains(VERSION)), "neueste Version gemeldet");
        app.error = None;
        answer(&mut app, None);
        assert!(app.error.is_some(), "ohne Netz eine Meldung");
        assert!(!app.update.manual);
    }
}
