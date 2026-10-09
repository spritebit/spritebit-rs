//! Update-Hinweis: beim Start einmal bei GitHub nach der neuesten Release
//! fragen und, wenn es eine neuere gibt, oben ein Band zeigen —
//! „Neue Version 1.0.3 verfügbar“ mit Herunterladen, Später und
//! „Diese Version überspringen“.
//!
//! Die Abfrage läuft in einem eigenen Thread, die App startet also nicht
//! langsamer; ohne Internet passiert einfach nichts. Abschaltbar unter
//! Hilfe → „Beim Start nach Updates suchen“. Gemerkt wird das (und eine
//! übersprungene Version) in `update.json` im Einstellungsordner.
//!
//! Die Rechnung (Versionen vergleichen, Antwort lesen) steht in reinen
//! Funktionen mit Tests; die Tests gehen nie ins Netz — die Abfrage startet
//! nur `main()`, nicht `SpritebitApp::new()`.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui;

use crate::i18n::{tr, trf};
use crate::{SpritebitApp, VERSION};

/// Neueste Release von spritebit-rs (öffentliche GitHub-API, ohne Konto).
const LATEST_API: &str = "https://api.github.com/repos/spritebit/spritebit-rs/releases/latest";
/// Dorthin führt „Herunterladen“: die Release-Seite mit Download und Änderungen.
pub(crate) const LATEST_PAGE: &str = "https://github.com/spritebit/spritebit-rs/releases/latest";

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

/// Ob ein Band gezeigt wird: neuer als diese Version und nicht übersprungen.
pub(crate) fn worth_showing(latest: &str, current: &str, skipped: Option<&str>) -> bool {
    is_newer(latest, current) && skipped != Some(latest)
}

/// Die Abfrage selbst (blockierend — läuft im eigenen Thread).
///
/// Zuerst die normale Release-Seite: sie leitet auf `…/tag/vX.Y.Z` weiter,
/// und diese Weiterleitung zählt nicht zum Kontingent der GitHub-API (ohne
/// Anmeldung 60 Abfragen je Stunde und Internetanschluss — ist es
/// aufgebraucht, käme sonst nie ein Hinweis). Die API nur als Rückfall.
fn fetch_latest() -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .max_redirects(0)
        .build()
        .into();
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
    let mut resp = agent
        .get(LATEST_API)
        .header("User-Agent", &ua)
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let body = resp.body_mut().read_to_string().ok()?;
    version_from_release(&body)
}

/// Zustand des Update-Hinweises.
pub(crate) struct UpdateState {
    /// Beim Start nachsehen? (Hilfe-Menü)
    pub check: bool,
    /// Diese Version nicht mehr anbieten.
    pub skipped: Option<String>,
    /// Gefundene neuere Version, solange das Band offen ist.
    pub available: Option<String>,
    rx: Option<Receiver<Option<String>>>,
    /// Erst nach dem Laden speichern (Tests schreiben nichts).
    persist: bool,
}

impl Default for UpdateState {
    fn default() -> Self {
        UpdateState { check: true, skipped: None, available: None, rx: None, persist: false }
    }
}

fn settings_path() -> Option<std::path::PathBuf> {
    crate::i18n::settings_dir().map(|d| d.join("update.json"))
}

impl SpritebitApp {
    /// Beim Start: Einstellungen laden und — wenn gewünscht — nachsehen.
    pub(crate) fn start_update_check(&mut self, ctx: &egui::Context) {
        self.update.persist = true;
        if let Some(v) = settings_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()) {
            if let Some(c) = v.get("check").and_then(|c| c.as_bool()) {
                self.update.check = c;
            }
            self.update.skipped = v.get("skip").and_then(|s| s.as_str()).map(str::to_string);
        }
        if self.update.check {
            self.spawn_update_check(ctx);
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
            let v = serde_json::json!({ "check": self.update.check, "skip": self.update.skipped });
            let _ = std::fs::write(p, v.to_string());
        }
    }

    /// Jeden Frame: ist die Antwort da?
    pub(crate) fn poll_update(&mut self) {
        let Some(rx) = &self.update.rx else { return };
        let Ok(found) = rx.try_recv() else { return };
        self.update.rx = None;
        if let Some(v) = found {
            if worth_showing(&v, VERSION, self.update.skipped.as_deref()) {
                self.update.available = Some(v);
            }
        }
    }

    /// Inhalt des Bands oben (ui() legt das Panel nur an, solange eine
    /// neuere Version da ist).
    pub(crate) fn update_banner(&mut self, ui: &mut egui::Ui) {
        let Some(v) = self.update.available.clone() else { return };
        let (mut later, mut skip) = (false, false);
        ui.horizontal(|ui| {
            ui.colored_label(ui.visuals().selection.stroke.color, "⬆");
            ui.strong(trf("Neue Version {new} verfügbar", &[("new", &v)]));
            ui.weak(trf("(du hast {old})", &[("old", &VERSION)]));
            ui.hyperlink_to(tr("Herunterladen"), LATEST_PAGE);
            later = ui.button(tr("Später")).clicked();
            skip = ui.button(tr("Diese Version überspringen")).clicked();
        });
        if skip {
            self.update.skipped = Some(v);
            self.save_update_settings();
        }
        if later || skip {
            self.update.available = None;
        }
    }

    /// Eintrag im Hilfe-Menü.
    pub(crate) fn update_menu(&mut self, ui: &mut egui::Ui) {
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

    #[test]
    fn uebersprungene_version_kommt_nicht_wieder() {
        assert!(worth_showing("1.0.3", "1.0.2", None));
        assert!(!worth_showing("1.0.3", "1.0.2", Some("1.0.3")));
        assert!(worth_showing("1.0.4", "1.0.2", Some("1.0.3")), "eine noch neuere schon");
        assert!(!worth_showing("1.0.2", "1.0.2", None));
    }
}
