//! Selbst aktualisieren: „Jetzt aktualisieren“ im Update-Band (update.rs)
//! lädt die neue `spritebit.exe` von der GitHub-Release, prüft sie gegen die
//! dort veröffentlichte SHA-256-Prüfsumme und tauscht die eigene Datei aus —
//! kein ZIP, kein Entpacken, kein Verschieben.
//!
//! Windows lässt ein laufendes Programm seine Datei nicht überschreiben, aber
//! umbenennen. Darum:
//!   1. neue Datei als `spritebit.new.exe` neben die alte schreiben
//!   2. die laufende in `spritebit.old.exe` umbenennen
//!   3. die neue an ihren Platz
//!
//! Geht dabei etwas schief, wird zurückgetauscht. Die alte Datei löscht der
//! nächste Start ([`cleanup_old`]).
//!
//! Neu gestartet wird mit Übergabe ([`SpritebitApp::restart_after_update`]):
//! die Sitzung und eine kleine `restart.json` (Datei, Speichern-Status)
//! landen im Einstellungsordner, die neue Version macht genau dort weiter.
//!
//! Die Tests gehen nie ins Netz; Austauschen und Prüfen laufen auf
//! Dateien in einem eigenen Testordner.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui;
use sha2::{Digest, Sha256};

use crate::i18n::{tr, trf};
use crate::{SpritebitApp, VERSION};

/// Wo die Dateien einer Release liegen (immer die genannte Version, nicht „latest“).
pub(crate) fn asset_url(version: &str, file: &str) -> String {
    format!("https://github.com/spritebit/spritebit-rs/releases/download/v{version}/{file}")
}
const EXE_ASSET: &str = "spritebit.exe";
const SHA_ASSET: &str = "spritebit.exe.sha256";
/// Größer wird die .exe nie — eine Grenze gegen Unsinn aus dem Netz.
const MAX_EXE: u64 = 200 * 1024 * 1024;

/// Prüfsumme aus der `.sha256`-Datei: die ersten 64 Hex-Zeichen (Format von
/// `sha256sum` bzw. `Get-FileHash`, auch groß geschrieben).
pub(crate) fn parse_sha256(text: &str) -> Option<String> {
    let t = text.split_whitespace().next()?.to_ascii_lowercase();
    (t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit())).then_some(t)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Sieht das aus wie ein Windows-Programm (PE-Datei)?
fn looks_like_exe(bytes: &[u8]) -> bool {
    bytes.len() > 1024 && bytes.starts_with(b"MZ")
}

/// Neben `exe`: die Namen für die neue und die abgelöste Datei.
fn side_paths(exe: &Path) -> (PathBuf, PathBuf) {
    let stem = exe.file_stem().and_then(|s| s.to_str()).unwrap_or("spritebit");
    let dir = exe.parent().unwrap_or(Path::new("."));
    (dir.join(format!("{stem}.new.exe")), dir.join(format!("{stem}.old.exe")))
}

/// Die Datei `exe` durch `bytes` ersetzen (siehe oben). Klappt das nicht,
/// bleibt alles, wie es war.
pub(crate) fn swap_exe(exe: &Path, bytes: &[u8]) -> Result<(), String> {
    let (new, old) = side_paths(exe);
    std::fs::write(&new, bytes).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            tr("Der Ordner der App ist schreibgeschützt — bitte die neue Version von Hand laden.").to_string()
        } else {
            e.to_string()
        }
    })?;
    let _ = std::fs::remove_file(&old);
    if let Err(e) = std::fs::rename(exe, &old) {
        let _ = std::fs::remove_file(&new);
        return Err(e.to_string());
    }
    if let Err(e) = std::fs::rename(&new, exe) {
        let _ = std::fs::rename(&old, exe);
        let _ = std::fs::remove_file(&new);
        return Err(e.to_string());
    }
    Ok(())
}

/// Beim Start: die abgelöste Datei vom letzten Update wegräumen.
pub(crate) fn cleanup_old() {
    let Ok(exe) = std::env::current_exe() else { return };
    let (new, old) = side_paths(&exe);
    if old != exe {
        let _ = std::fs::remove_file(old);
    }
    if new != exe {
        let _ = std::fs::remove_file(new);
    }
}

/// Geht Selbst-Aktualisieren hier? Nur in der fertigen Windows-Fassung —
/// eine Entwickler-Fassung (cargo run) soll sich nicht selbst überschreiben.
pub(crate) fn supported() -> bool {
    cfg!(windows) && !cfg!(debug_assertions)
}

/// Herunterladen, prüfen, austauschen (blockierend — läuft im eigenen Thread).
fn download_and_swap(version: &str, exe: &Path) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(180))).build().into();
    let ua = format!("spritebit/{VERSION}");
    let net = |e: ureq::Error| trf("Download fehlgeschlagen: {e}", &[("e", &e)]);
    let sha_text = agent.get(asset_url(version, SHA_ASSET)).header("User-Agent", &ua).call().map_err(net)?.body_mut().read_to_string().map_err(net)?;
    let want = parse_sha256(&sha_text).ok_or_else(|| tr("Die Prüfsumme der Release ist unlesbar.").to_string())?;
    let bytes = agent
        .get(asset_url(version, EXE_ASSET))
        .header("User-Agent", &ua)
        .call()
        .map_err(net)?
        .body_mut()
        .with_config()
        .limit(MAX_EXE)
        .read_to_vec()
        .map_err(net)?;
    if !looks_like_exe(&bytes) || sha256_hex(&bytes) != want {
        return Err(tr("Die geladene Datei passt nicht zur Prüfsumme — es wurde nichts geändert.").into());
    }
    swap_exe(exe, &bytes)
}

/// Stand des Aktualisierens (im Update-Band).
#[derive(Default)]
pub(crate) enum Install {
    #[default]
    Idle,
    Running(Receiver<Result<(), String>>),
    /// Fertig; die neue Version liegt an `exe`.
    Done {
        version: String,
        exe: PathBuf,
    },
    Failed(String),
}

/// Übergabe an die neu gestartete Version.
fn restart_file() -> Option<PathBuf> {
    crate::i18n::settings_dir().map(|d| d.join("restart.json"))
}

/// Was die neue Version nach dem Neustart übernimmt.
pub(crate) struct Handoff {
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub version: String,
}

pub(crate) fn handoff_json(h: &Handoff) -> String {
    serde_json::json!({ "path": h.path.as_ref().map(|p| p.display().to_string()), "dirty": h.dirty, "version": h.version }).to_string()
}

pub(crate) fn parse_handoff(text: &str) -> Option<Handoff> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    Some(Handoff {
        path: v.get("path").and_then(|p| p.as_str()).map(PathBuf::from),
        dirty: v.get("dirty").and_then(|d| d.as_bool()).unwrap_or(true),
        version: v.get("version").and_then(|s| s.as_str()).unwrap_or("").to_string(),
    })
}

/// Beim Start: liegt eine Übergabe vom Update da? (Wird dabei gelöscht.)
pub(crate) fn take_handoff() -> Option<Handoff> {
    let p = restart_file()?;
    let text = std::fs::read_to_string(&p).ok()?;
    let _ = std::fs::remove_file(&p);
    parse_handoff(&text)
}

impl SpritebitApp {
    /// „Jetzt aktualisieren“: im Hintergrund laden und austauschen.
    pub(crate) fn start_install(&mut self, version: String, ctx: &egui::Context) {
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => {
                self.update.install = Install::Failed(e.to_string());
                return;
            }
        };
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let (v, target) = (version.clone(), exe.clone());
        std::thread::spawn(move || {
            let _ = tx.send(download_and_swap(&v, &target));
            ctx.request_repaint();
        });
        self.update.install = Install::Running(rx);
        self.update.install_target = Some((version, exe));
    }

    /// Jeden Frame: ist das Aktualisieren fertig?
    pub(crate) fn poll_install(&mut self) {
        let Install::Running(rx) = &self.update.install else { return };
        let Ok(r) = rx.try_recv() else { return };
        self.update.install = match (r, self.update.install_target.take()) {
            (Ok(()), Some((version, exe))) => Install::Done { version, exe },
            (Ok(()), None) => Install::Failed(tr("Unbekannter Fehler.").into()),
            (Err(e), _) => Install::Failed(e),
        };
    }

    /// Neu starten: den Stand übergeben, die neue Version starten, beenden.
    pub(crate) fn restart_after_update(&mut self, version: &str, exe: &Path) {
        self.commit_float();
        let h = Handoff { path: self.path.clone(), dirty: self.dirty, version: version.to_string() };
        let saved = self.write_session_now() && restart_file().is_some_and(|p| std::fs::write(p, handoff_json(&h)).is_ok());
        if !saved {
            self.error = Some(tr("Der Stand konnte nicht übergeben werden — bitte erst speichern und die App selbst neu starten.").into());
            return;
        }
        match std::process::Command::new(exe).spawn() {
            // Ohne Rückfrage und ohne die Sitzung zu löschen — die neue
            // Version holt sie gleich ab.
            Ok(_) => std::process::exit(0),
            Err(e) => self.error = Some(trf("Die neue Version ließ sich nicht starten: {e}", &[("e", &e)])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("spritebit-selfupdate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn pruefsumme_lesen_und_rechnen() {
        let h = sha256_hex(b"abc");
        assert_eq!(h, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(parse_sha256(&format!("{}  spritebit.exe\n", h.to_uppercase())).as_deref(), Some(h.as_str()));
        assert_eq!(parse_sha256("zu kurz"), None);
        assert_eq!(parse_sha256(""), None);
        assert_eq!(parse_sha256(&"g".repeat(64)), None, "kein Hex");
    }

    #[test]
    fn nur_windows_programme() {
        let mut exe = b"MZ".to_vec();
        exe.resize(4096, 0);
        assert!(looks_like_exe(&exe));
        assert!(!looks_like_exe(b"<html>Not Found</html>"));
    }

    #[test]
    fn austauschen_und_aufraeumen() {
        let d = temp_dir("swap");
        let exe = d.join("spritebit.exe");
        std::fs::write(&exe, b"alt").unwrap();
        swap_exe(&exe, b"neu").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"neu");
        assert_eq!(std::fs::read(d.join("spritebit.old.exe")).unwrap(), b"alt", "die alte liegt daneben");
        assert!(!d.join("spritebit.new.exe").exists());
        // Ein zweites Update überschreibt die alte „old“.
        swap_exe(&exe, b"neuer").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"neuer");
        assert_eq!(std::fs::read(d.join("spritebit.old.exe")).unwrap(), b"neu");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn fehlt_die_datei_bleibt_alles_wie_es_war() {
        let d = temp_dir("missing");
        let exe = d.join("spritebit.exe"); // gibt es nicht
        assert!(swap_exe(&exe, b"neu").is_err());
        assert!(!exe.exists());
        assert!(!d.join("spritebit.new.exe").exists(), "aufgeräumt");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn uebergabe_hin_und_zurueck() {
        let h = Handoff { path: Some(PathBuf::from(r"C:\Bilder\held.spritebit")), dirty: false, version: "1.2.0".into() };
        let back = parse_handoff(&handoff_json(&h)).unwrap();
        assert_eq!(back.path, h.path);
        assert!(!back.dirty);
        assert_eq!(back.version, "1.2.0");
        let none = parse_handoff(r#"{"path":null,"dirty":true,"version":"1.2.0"}"#).unwrap();
        assert!(none.path.is_none() && none.dirty);
        assert!(parse_handoff("Unsinn").is_none());
    }

    #[test]
    fn adresse_der_release_dateien() {
        assert_eq!(asset_url("1.2.0", EXE_ASSET), "https://github.com/spritebit/spritebit-rs/releases/download/v1.2.0/spritebit.exe");
    }
}
