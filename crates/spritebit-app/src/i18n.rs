//! Sprachen: Deutsch, Englisch, Österreichisch — wie in der Web-Version.
//!
//! Der deutsche Text ist selbst der Schlüssel: `tr("Speichern")` gibt je
//! nach Sprache „Save“ oder „Speichern“ zurück. Texte mit Werten haben
//! Platzhalter in geschweiften Klammern und gehen über [`trf`].
//!
//! Österreichisch ist Deutsch mit Dialekt-Einschlag: fehlt ein Eintrag
//! (leer in der Tabelle), gilt der deutsche Text. Fachbegriffe (Sprite,
//! Frame, Tag, PNG …) und Tastenkürzel bleiben, wie in `i18n-at.js`.
//!
//! Die Wahl liegt in einer kleinen Datei im Einstellungsordner des Systems
//! — nicht im Projekt, sonst wanderte sie in fremde Projektdateien.

use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
    At,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::De, Lang::En, Lang::At];

    /// Name der Sprache in ihr selbst — damit man sie immer wiederfindet.
    pub fn name(self) -> &'static str {
        match self {
            Lang::De => "Deutsch",
            Lang::En => "English",
            Lang::At => "Österreichisch",
        }
    }

    fn code(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
            Lang::At => "at",
        }
    }
}

thread_local! {
    // Die Oberfläche läuft in einem Thread; so stören sich parallele Tests nicht.
    static LANG: Cell<Lang> = const { Cell::new(Lang::De) };
}

pub fn lang() -> Lang {
    LANG.with(|l| l.get())
}

pub fn set_lang(l: Lang) {
    LANG.with(|c| c.set(l));
}

fn settings_file() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("spritebit").join("lang"))
}

/// Gespeicherte Sprache laden (sonst bleibt Deutsch).
pub fn load() {
    let stored = settings_file().and_then(|p| std::fs::read_to_string(p).ok());
    if let Some(l) = Lang::ALL.into_iter().find(|l| stored.as_deref().map(str::trim) == Some(l.code())) {
        set_lang(l);
    }
}

/// Sprache wählen und merken. Klappt das Merken nicht, gilt sie trotzdem.
pub fn choose(l: Lang) {
    set_lang(l);
    if let Some(p) = settings_file() {
        let _ = p.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(p, l.code());
    }
}

type Table = HashMap<&'static str, (&'static str, &'static str)>;

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| TEXTS.iter().map(|&(de, en, at)| (de, (en, at))).collect())
}

/// Übersetzung eines festen Textes.
pub fn tr(de: &'static str) -> &'static str {
    match (lang(), table().get(de)) {
        (Lang::En, Some((en, _))) => en,
        (Lang::At, Some((_, at))) if !at.is_empty() => at,
        _ => de,
    }
}

/// Übersetzung mit Werten: `trf("Farbe {n}", &[("n", &5)])`.
pub fn trf(de: &'static str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut s = tr(de).to_string();
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), &v.to_string());
    }
    s
}

/// Tastennamen: auf Englisch „Ctrl“ statt „Strg“ usw.
pub fn keys(de: &'static str) -> String {
    if lang() != Lang::En {
        return de.to_string();
    }
    de.replace("Strg", "Ctrl").replace("Umschalt", "Shift").replace("Entf", "Del").replace("Pos1", "Home").replace("Ende", "End")
}

/// (Deutsch, Englisch, Österreichisch — leer = wie Deutsch)
const TEXTS: &[(&str, &str, &str)] = &[
    // ── Menü ──
    ("Datei", "File", ""),
    ("Neuer Sprite …", "New sprite …", "Neicha Sprite …"),
    ("Öffnen …", "Open …", "Aufmochn …"),
    ("Speichern", "Save", ""),
    ("Speichern unter …", "Save as …", "Speichern ois …"),
    ("Exportieren …", "Export …", "Exportiern …"),
    ("Als Web-Projekt exportieren …", "Export as web project …", "Ois Web-Projekt exportiern …"),
    ("Beenden", "Quit", "Aufhörn"),
    ("Bearbeiten", "Edit", "Bearbeitn"),
    ("Rückgängig", "Undo", "Zruck"),
    ("Wiederholen", "Redo", "Doch wieder"),
    ("Ausschneiden", "Cut", "Ausschneidn"),
    ("Kopieren", "Copy", "Kopiern"),
    ("Einfügen", "Paste", "Einifügn"),
    ("Alles auswählen", "Select all", "Ois auswöhln"),
    ("Auswahl aufheben", "Deselect", "Auswoi weg"),
    ("Auswahl leeren", "Clear selection", "Auswoi ausleern"),
    ("Ansicht", "View", "Aunsicht"),
    ("Einpassen", "Fit to window", "Einipassn"),
    ("Gitter", "Grid", ""),
    ("Sprache", "Language", "Sproch"),
    ("Hilfe", "Help", ""),
    ("Über spritebit", "About spritebit", ""),
    // ── Dateien ──
    ("Projekt öffnen", "Open project", "A Projekt aufmochn"),
    ("Projekt speichern", "Save project", ""),
    ("spritebit-Projekt", "spritebit project", ""),
    ("Alle Dateien", "All files", "Olle Dateien"),
    ("Als Web-Projekt exportieren", "Export as web project", "Ois Web-Projekt exportiern"),
    ("Web-Projekt (JSON)", "Web project (JSON)", ""),
    ("Unbenannt", "Untitled", "Ohne Nom"),
    ("{path} konnte nicht gelesen werden: {e}", "Could not read {path}: {e}", "{path} hot si ned lesn lossn: {e}"),
    ("{path} konnte nicht gespeichert werden: {e}", "Could not save {path}: {e}", "{path} hot si ned speichern lossn: {e}"),
    ("{path} konnte nicht geschrieben werden: {e}", "Could not write {path}: {e}", "{path} hot si ned schreibn lossn: {e}"),
    ("Keine spritebit-Projektdatei ({why}).", "Not a spritebit project file ({why}).", "Des is ka spritebit-Projektdatei ({why})."),
    ("Diese Projektdatei wird nicht unterstützt: {why}", "This project file is not supported: {why}", "De Projektdatei geht ned: {why}"),
    ("Fläche {w} × {h} ist zu groß (höchstens {max} × {max}).", "Canvas {w} × {h} is too big (at most {max} × {max}).", "De Flächn {w} × {h} is z’groß (höchstns {max} × {max})."),
    ("Die Datei ist beschädigt ({why}).", "The file is damaged ({why}).", "De Datei is hin ({why})."),
    // ── Leisten ──
    ("Sprites", "Sprites", ""),
    ("Neuer Sprite", "New sprite", "Neicha Sprite"),
    ("Farben", "Colors", ""),
    ("{tiles} Kacheln · {kib} KiB", "{tiles} tiles · {kib} KiB", "{tiles} Kachln · {kib} KiB"),
    // ── Dialoge ──
    ("Breite", "Width", "Breitn"),
    ("Höhe", "Height", "Höh"),
    ("Anlegen", "Create", "Aunlegn"),
    ("Abbrechen", "Cancel", "Loss ma’s"),
    ("spritebit — Pixel-Art-Editor", "spritebit — pixel art editor", ""),
    ("Hinweis", "Note", ""),
    ("Ungespeicherte Änderungen", "Unsaved changes", "Ned gspeicherte Änderungen"),
    ("Vor dem Beenden speichern?", "Save before quitting?", "Vorm Aufhörn speichern?"),
    ("Vor dem Öffnen eines anderen Projekts speichern?", "Save before opening another project?", "Vorm Aufmochn von an aundern Projekt speichern?"),
    ("Nicht speichern", "Don’t save", "Ned speichern"),
    // ── Werkzeuge ──
    ("Hand", "Hand", ""),
    ("Stift", "Pencil", ""),
    ("Pinsel", "Brush", "Pinsl"),
    ("Spray", "Spray", ""),
    ("Füllen", "Fill", "Aufülln"),
    ("Radierer", "Eraser", "Radiergummi"),
    ("Linie", "Line", ""),
    ("Rechteck", "Rectangle", "Viereck"),
    ("Ellipse", "Ellipse", ""),
    ("Auswahl", "Select", "Auswoi"),
    ("Lasso", "Lasso", ""),
    ("Farbwahl", "Color select", "Farbwoi"),
    ("Zauberstab", "Magic wand", "Zauberstaberl"),
    ("Größe", "Size", "Greß"),
    ("Gefüllt", "Filled", "Ausgfüllt"),
    ("Symmetrie: links ↔ rechts", "Symmetry: left ↔ right", ""),
    ("Symmetrie: oben ↔ unten", "Symmetry: top ↔ bottom", "Symmetrie: obn ↔ untn"),
    ("Ebene „{name}“ ist gesperrt — Schloss in der Timeline.", "Layer “{name}” is locked — see the lock in the timeline.", "D’Ebene „{name}“ is zuagsperrt — s’Schloss in da Timeline."),
    ("Ebene „{name}“ ist ausgeblendet — Auge in der Timeline.", "Layer “{name}” is hidden — see the eye in the timeline.", "D’Ebene „{name}“ is ausblendt — s’Aug in da Timeline."),
    // ── Auswahl ──
    ("Toleranz", "Tolerance", ""),
    ("Auswahl mit der aktuellen Farbe füllen", "Fill the selection with the current color", "D’Auswoi mit da aktuelln Farb aufülln"),
    ("Leeren", "Clear", "Ausleern"),
    ("Aufheben", "Deselect", "Weg damit"),
    // ── Farben ──
    ("Transparent", "Transparent", "Durchsichtig"),
    ("Freie Farbe {hex}", "Free color {hex}", "Freie Farb {hex}"),
    ("Nr. {c} · {hex}", "No. {c} · {hex}", ""),
    ("Nr. {c}", "No. {c}", ""),
    ("Freie Farbe wählen", "Pick a free color", "A freie Farb aussuachn"),
    ("0 · Transparent (Radierer)", "0 · Transparent (eraser)", "0 · Durchsichtig (Radiergummi)"),
    ("Palette bearbeiten", "Edit palette", "Palettn bearbeitn"),
    ("Eingebaute Palette — Änderungen gehen in eine Kopie.", "Built-in palette — changes go into a copy.", "Eibaute Palettn — Änderungen kemman in a Kopie."),
    ("Farbe {n}", "Color {n}", "Farb {n}"),
    ("+ Farbe", "+ Color", "+ Farb"),
    ("− Letzte", "− Last", "− De letzte"),
    ("Palette: {name}", "Palette: {name}", "Palettn: {name}"),
    // ── Timeline ──
    ("Erster Frame (Pos1)", "First frame (Home)", "Da erste Frame (Pos1)"),
    ("Voriger Frame (,)", "Previous frame (,)", "Da Frame davor (,)"),
    ("Abspielen / Anhalten (Enter)", "Play / pause (Enter)", "Obspün / Aunhoitn (Enter)"),
    ("Nächster Frame (.)", "Next frame (.)", "Da nächste Frame (.)"),
    ("Letzter Frame (Ende)", "Last frame (End)", "Da letzte Frame (Ende)"),
    ("Frame", "Frame", ""),
    ("Leerer Frame dahinter", "Empty frame after", "A laara Frame dahinter"),
    ("Frame duplizieren", "Duplicate frame", "Frame verdoppln"),
    ("Frame löschen", "Delete frame", "Frame weghaun"),
    ("Ebene", "Layer", ""),
    ("Ebene {n}", "Layer {n}", ""),
    ("Neue Ebene über der aktiven", "New layer above the active one", "A neiche Ebene über da aktivn"),
    ("Ebene nach oben", "Move layer up", "Ebene aufi"),
    ("Ebene nach unten", "Move layer down", "Ebene obi"),
    ("Ebene löschen", "Delete layer", "Ebene weghaun"),
    ("Zellen", "Cels", "Zön"),
    ("Zellen kopieren (Bereich per Shift-Klick)", "Copy cels (Shift+click for a range)", "Zön kopiern (Bereich mit Shift-Klick)"),
    ("Zellen an der aktiven Zelle einfügen", "Paste cels at the active cel", "Zön bei da aktivn Zön einifügn"),
    ("Hier passt nichts hin — die Zellen haben eine andere Größe.", "Nothing fits here — the cels have a different size.", "Do passt nix hin — de Zön san aundas groß."),
    ("Zellen leeren", "Clear cels", "Zön ausleern"),
    (
        "Verknüpfen — die Frames teilen sich je Ebene ein Bild (ohne Bereich: mit dem Frame davor)",
        "Link — the frames share one image per layer (without a range: with the frame before)",
        "Vabindn — de Frames teiln si pro Ebene a Buidl (ohne Bereich: mit dem Frame davor)",
    ),
    ("Lösen — jede Zelle bekommt ihr eigenes Bild", "Unlink — every cel gets its own image", "Lösn — jede Zön kriagt a eigns Buidl"),
    ("Tag anlegen — benennt den Bereich bzw. den Frame, z. B. „Laufen“", "Add tag — names the range or frame, e.g. “Walk”", "Tag aunlegn — gibt dem Bereich oder Frame an Nom, z. B. „Renna“"),
    ("Onion Skin — voriger (rot) und nächster Frame (blau) scheinen durch", "Onion skin — previous (red) and next frame (blue) show through", "Onion Skin — da Frame davor (rot) und da nächste (blau) scheinan durch"),
    ("FPS", "FPS", ""),
    ("Dauer", "Duration", ""),
    ("0 = nach FPS", "0 = by FPS", "0 = noch FPS"),
    ("Ebene umbenennen", "Rename layer", "Ebene umtaufn"),
    ("OK", "OK", ""),
    ("Tag", "Tag", ""),
    ("Frames", "Frames", ""),
    ("Richtung", "Direction", ""),
    ("Vorwärts", "Forward", "Vüri"),
    ("Rückwärts", "Reverse", "Zruck"),
    ("Ping-Pong", "Ping-pong", ""),
    ("Farbe", "Color", "Farb"),
    ("Abspielen", "Play", "Obspün"),
    ("Löschen", "Delete", "Weghaun"),
    ("Fertig", "Done", "Passt"),
    // ── Export ──
    ("Exportieren", "Export", "Exportiern"),
    ("PNG — aktueller Frame", "PNG — current frame", "PNG — da aktuelle Frame"),
    ("PNG — jeder Frame eine Datei", "PNG — one file per frame", "PNG — jeder Frame a eigne Datei"),
    ("GIF — ganze Animation", "GIF — whole animation", "GIF — de ganze Animation"),
    ("GIF — eine Datei je Tag", "GIF — one file per tag", "GIF — a Datei pro Tag"),
    ("Spritesheet (PNG + JSON-Atlas, alle Sprites)", "Sprite sheet (PNG + JSON atlas, all sprites)", "Spritesheet (PNG + JSON-Atlas, olle Sprites)"),
    ("Vergrößerung", "Scale", "Vagrößerung"),
    ("PNG exportieren", "Export PNG", "PNG exportiern"),
    ("GIF exportieren", "Export GIF", "GIF exportiern"),
    ("Spritesheet exportieren", "Export sprite sheet", "Spritesheet exportiern"),
    ("Ordner für die Frames", "Folder for the frames", "Ordner für de Frames"),
    ("Ordner für die GIFs", "Folder for the GIFs", "Ordner für de GIFs"),
    ("Gespeichert: {path}", "Saved: {path}", "Gspeichert: {path}"),
    ("Gespeichert: {a} und {b}", "Saved: {a} and {b}", "Gspeichert: {a} und {b}"),
    ("{n} PNGs in {dir}", "{n} PNGs in {dir}", ""),
    ("{n} GIFs in {dir}", "{n} GIFs in {dir}", ""),
    (
        "Das Ergebnis wäre {w} × {h} Pixel groß — höchstens {max} je Seite. Kleinere Vergrößerung wählen.",
        "The result would be {w} × {h} pixels — at most {max} per side. Choose a smaller scale.",
        "Des Ergebnis wär {w} × {h} Pixel groß — höchstns {max} pro Seitn. Nimm a klanere Vagrößerung.",
    ),
    ("{n} Farben — ein GIF fasst höchstens 255.", "{n} colors — a GIF holds at most 255.", "{n} Farben — a GIF packt höchstns 255."),
    ("Speichern fehlgeschlagen: {e}", "Saving failed: {e}", "Speichern is danebn gaunga: {e}"),
    // ── Bild und Aufräumen ──
    ("Bild", "Image", "Buidl"),
    ("Aufräumen", "Cleanup", "Zammraman"),
    ("Wirkt auf: Auswahl", "Acts on: selection", "Wirkt auf: Auswoi"),
    ("Wirkt auf: Sprite", "Acts on: sprite", ""),
    ("↔ Spiegeln", "↔ Flip", "↔ Spiagln"),
    ("↕ Spiegeln", "↕ Flip", "↕ Spiagln"),
    ("Waagerecht spiegeln", "Flip horizontally", "Waagrecht spiagln"),
    ("Senkrecht spiegeln", "Flip vertically", "Senkrecht spiagln"),
    ("90° im Uhrzeigersinn drehen", "Rotate 90° clockwise", "90° im Uhrzeigersinn drahn"),
    ("Frei drehen", "Free rotate", "Frei drahn"),
    ("Übernehmen", "Apply", "Übernehma"),
    ("Verwerfen", "Discard", "Weghaun"),
    ("Zuschneiden", "Trim", "Zuaschneidn"),
    ("Leeren Rand rundherum abschneiden", "Cut away the empty border all around", "Den laarn Rand rundumadum wegschneidn"),
    ("Zentrieren", "Center", "Zentriern"),
    ("Inhalt mittig setzen", "Move the content to the middle", "Den Inhalt in d’Mitte gebn"),
    ("Leinwand", "Canvas", "Leinwaund"),
    ("mittig", "centered", ""),
    ("oben links", "top left", "obn links"),
    ("Anwenden", "Apply", "Anwendn"),
    ("Skalieren", "Scale", "Skaliern"),
    ("Auf das Doppelte vergrößern (Pixel bleiben hart)", "Double the size (pixels stay hard)", "Doppelt so groß (d’Pixel bleibn hart)"),
    ("Auf die Hälfte verkleinern — Details gehen verloren", "Halve the size — detail is lost", "Auf d’Hälfte owe — Details san dann weg"),
    ("Jetzt {w} × {h} px.", "Now {w} × {h} px.", "Jetzt {w} × {h} px."),
    ("Jetzt {w} × {h} px — {n} Pixel lagen außerhalb.", "Now {w} × {h} px — {n} pixels were outside.", "Jetzt {w} × {h} px — {n} Pixel san außa gfoin."),
    ("Nichts abzuschneiden.", "Nothing to trim.", "Do gibt’s nix zum Wegschneidn."),
    ("Schon mittig oder leer.", "Already centered or empty.", "Is eh scho in da Mitt oder laar."),
    ("Größe unverändert.", "Size unchanged.", "De Greß bleibt gleich."),
    ("Zu groß — höchstens 8192 × 8192.", "Too big — at most 8192 × 8192.", "Z’groß — höchstns 8192 × 8192."),
    ("Zu klein — mindestens 1 × 1.", "Too small — at least 1 × 1.", "Z’klaa — mindestns 1 × 1."),
    ("Hintergrund", "Background", "Hintagrund"),
    ("Toleranz: wie ähnlich die Farben vom Rand her sein dürfen", "Tolerance: how similar the colors from the border may be", "Toleranz: wia ähnlich de Farben vom Rand her sei dürfn"),
    ("Hintergrund entfernen", "Remove background", "Hintagrund weghaun"),
    ("Hintergrund entfernt — {n} Pixel.", "Background removed — {n} pixels.", "Hintagrund weg — {n} Pixel."),
    ("Nichts entfernt — Toleranz erhöhen?", "Nothing removed — raise the tolerance?", "Nix weg — Toleranz aufdrahn?"),
    ("Glätten", "Despeckle", "Glattmochn"),
    ("Einzelne Streupixel auf die Farbe ihrer Nachbarn setzen", "Set stray single pixels to the majority color of their neighbors", "Einzlne Streupixel auf de Farb vo de Nochbarn setzn"),
    ("Geglättet — {n} Pixel angepasst.", "Despeckled — {n} pixels adjusted.", "Glattgmocht — {n} Pixel ogpasst."),
    ("Nichts zu glätten gefunden.", "Nothing found to despeckle.", "Nix zum Glattmochn gfundn."),
    ("Outline", "Outline", "Umrandung"),
    ("Outline-Farbe", "Outline color", "Farb vo da Umrandung"),
    ("Outline gezeichnet — {n} Pixel.", "Outline drawn — {n} pixels.", "Umrandung zeichnet — {n} Pixel."),
    ("Keine Outline nötig — Sprite leer?", "No outline needed — is the sprite empty?", "Ka Umrandung nötig — is da Sprite laar?"),
];

/// Fehler beim Laden in der gewählten Sprache.
pub fn io_error(e: &spritebit_core::IoError) -> String {
    use spritebit_core::IoError::*;
    match e {
        NotAProject(why) => trf("Keine spritebit-Projektdatei ({why}).", &[("why", why)]),
        Unsupported(why) => trf("Diese Projektdatei wird nicht unterstützt: {why}", &[("why", why)]),
        TooBig { width, height } => trf(
            "Fläche {w} × {h} ist zu groß (höchstens {max} × {max}).",
            &[("w", width), ("h", height), ("max", &spritebit_core::MAX_SIDE)],
        ),
        Corrupt(why) => trf("Die Datei ist beschädigt ({why}).", &[("why", why)]),
    }
}

/// Fehler beim Export in der gewählten Sprache.
pub fn export_error(e: &spritebit_core::export::ExportError) -> String {
    use spritebit_core::export::{ExportError::*, MAX_OUTPUT};
    match e {
        TooBig { width, height } => trf(
            "Das Ergebnis wäre {w} × {h} Pixel groß — höchstens {max} je Seite. Kleinere Vergrößerung wählen.",
            &[("w", width), ("h", height), ("max", &MAX_OUTPUT)],
        ),
        TooManyColors(n) => trf("{n} Farben — ein GIF fasst höchstens 255.", &[("n", n)]),
        Encode(err) => trf("Speichern fehlgeschlagen: {e}", &[("e", err)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Jeder Text, der im Code durch `tr`/`trf` geht, steht in der Tabelle.
    #[test]
    fn alle_texte_sind_uebersetzt() {
        let sources = [
            include_str!("main.rs"),
            include_str!("tools_ui.rs"),
            include_str!("selection_ui.rs"),
            include_str!("timeline.rs"),
            include_str!("palette_ui.rs"),
            include_str!("export_ui.rs"),
            include_str!("image_ui.rs"),
        ];
        let mut missing = Vec::new();
        for src in sources {
            for call in ["tr(\"", "trf(\""] {
                for part in src.split(call).skip(1) {
                    let text = &part[..part.find('"').unwrap()];
                    if !table().contains_key(text) {
                        missing.push(text.to_string());
                    }
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "ohne Übersetzung: {missing:#?}");
    }

    #[test]
    fn keine_doppelten_schluessel() {
        assert_eq!(table().len(), TEXTS.len());
    }

    #[test]
    fn umschalten_und_rueckfall() {
        set_lang(Lang::En);
        assert_eq!(tr("Speichern"), "Save");
        assert_eq!(keys("Strg+Umschalt+S"), "Ctrl+Shift+S");
        set_lang(Lang::At);
        assert_eq!(tr("Rückgängig"), "Zruck");
        assert_eq!(tr("Speichern"), "Speichern", "leer = Deutsch");
        assert_eq!(trf("Farbe {n}", &[("n", &7)]), "Farb 7");
        set_lang(Lang::De);
        assert_eq!(tr("Nicht in der Tabelle"), "Nicht in der Tabelle");
    }
}
