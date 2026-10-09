//! Windows: Icon und Dateieigenschaften (Name, Version, Copyright) in die
//! .exe einbetten — sonst zeigt der Explorer das leere Standard-Symbol.
//! Die Icons erzeugt `python tools/make_icons.py --app <ordner>` im
//! Web-Repo (sprite-editor) aus demselben Motiv wie Favicon und PWA-Icons.

fn main() {
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
