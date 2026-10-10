//! Windows: Icon und Dateieigenschaften (Name, Version, Copyright) in die
//! .exe einbetten — sonst zeigt der Explorer das leere Standard-Symbol.
//! Die Icons erzeugt `python tools/make_icons.py --app <ordner>` im
//! Web-Repo (sprite-editor) aus demselben Motiv wie Favicon und PWA-Icons.
//!
//! Außerdem: die Notizen dieser Version (`notes/<version>.md` im Repo) als
//! `notes.md` nach OUT_DIR — „Was ist neu?“ (update.rs) zeigt sie ohne
//! Internet. Fehlt die Datei, bleibt sie leer; die Release (release.yml)
//! bricht dann ab, Entwickler-Fassungen bauen trotzdem.

fn main() {
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let notes = format!("../../notes/{version}.md");
    println!("cargo:rerun-if-changed={notes}");
    println!("cargo:rerun-if-changed=../../notes");
    let text = std::fs::read_to_string(&notes).unwrap_or_default();
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("notes.md"), text).expect("notes.md schreiben");
    println!("cargo:rerun-if-changed=assets/app/spritebit.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app/spritebit.ico")
            .set("ProductName", "spritebit")
            .set("FileDescription", "spritebit — Pixel-Art-Editor")
            .set("LegalCopyright", "© 2026 Marco Jan · MIT-Lizenz");
        res.compile().expect("Icon und Dateieigenschaften einbetten");
    }
}
