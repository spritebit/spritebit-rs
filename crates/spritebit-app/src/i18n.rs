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
    // ── Sprites, Ebenen, Frames ──
    ("Alle Sprites und eigenen Paletten dieses Projekts werden verworfen.", "All sprites and custom palettes of this project will be discarded.", "Olle Sprites und eignen Palettn vo dem Projekt kemman weg."),
    ("Alles zurücksetzen …", "Reset everything …", "Ois zrucksetzn …"),
    ("Alles zurücksetzen?", "Reset everything?", "Ois zrucksetzn?"),
    ("Zurücksetzen", "Reset", "Zrucksetzn"),
    ("Anker", "Anchor", ""),
    ("Das lässt sich nicht rückgängig machen.", "This cannot be undone.", "Des geht nimma zruck."),
    ("Deckkraft der aktiven Ebene", "Opacity of the active layer", "Deckkraft vo da aktivn Ebene"),
    ("Die Leinwand wird nur größer oder kleiner — die Pixel behalten ihre Größe.", "Only the canvas grows or shrinks — the pixels keep their size.", "Nur d’Leinwaund wird größer oder klana — de Pixel bleibn, wia s’ san."),
    ("Duplizieren", "Duplicate", "Verdoppln"),
    ("Ebene verdoppeln", "Duplicate layer", "Ebene verdoppln"),
    ("Erstellen", "Create", "Mochn"),
    ("Farbpalette", "Color palette", "Farbpalettn"),
    ("Frame nach links", "Move frame left", "Frame noch links"),
    ("Frame nach rechts", "Move frame right", "Frame noch rechts"),
    ("Größe ändern …", "Resize …", "Greß ändern …"),
    ("Leinwand ändern", "Change canvas", "Leinwaund ändern"),
    ("Löschen …", "Delete …", "Weghaun …"),
    ("Nach unten zusammenlegen — in jedem Frame", "Merge down — in every frame", "Noch untn zammlegn — in jedem Frame"),
    ("Name", "Name", "Nom"),
    ("Rechtsklick: umbenennen, duplizieren, Größe, löschen", "Right-click: rename, duplicate, size, delete", "Rechtsklick: umtaufn, verdoppln, Greß, weghaun"),
    ("Sprite umbenennen", "Rename sprite", "Sprite umtaufn"),
    ("Sprite „{name}“ löschen?", "Delete sprite “{name}”?", "Sprite „{name}“ weghaun?"),
    ("Umbenennen …", "Rename …", "Umtaufn …"),
    ("Zu Frame springen", "Jump to frame", "Zu an Frame hupfn"),
    ("{name} Kopie", "{name} copy", ""),
    ("Ändern", "Change", ""),
    // ── Paletten ──
    ("+ Palette", "+ Palette", "+ Palettn"),
    ("Alle", "All", "Olle"),
    ("Eigene", "Custom", "Eigne"),
    ("Eingebaut", "Built-in", "Eibaut"),
    ("Suchen …", "Search …", "Suachn …"),
    ("Palette", "Palette", "Palettn"),
    ("Ansicht: „{name}“", "Viewing: “{name}”", "Aunsicht: „{name}“"),
    ("„{name}“ — Palette des Sprites", "“{name}” — the sprite’s palette", "„{name}“ — d’Palettn vom Sprite"),
    ("Aus den Farben des Bildes eine Palette machen — wie viele Farben, wählst du aus", "Make a palette from the image’s colors — you choose how many", "Aus de Farben vom Buidl a Palettn mochn — wia vü Farben, suachst da aus"),
    ("Bild » Palette", "Image » Palette", "Buidl » Palettn"),
    ("Bild » Palette …", "Image » Palette …", "Buidl » Palettn …"),
    ("Bitte einen Namen eingeben.", "Please enter a name.", "Bitte an Nom eingebn."),
    ("Das Bild hat {n} Farben, eine Palette fasst höchstens {max}.", "The image has {n} colors; a palette holds at most {max}.", "Des Buidl hot {n} Farben, a Palettn packt höchstns {max}."),
    ("Das Bild ist leer.", "The image is empty.", "Des Buidl is laar."),
    ("Eine Palette „{name}“ gibt es schon.", "A palette “{name}” already exists.", "A Palettn „{name}“ gibt’s scho."),
    ("Farben automatisch ordnen: Grau, dann je Farbton von dunkel nach hell — das Bild bleibt gleich", "Sort colors automatically: grays, then each hue from dark to light — the image stays the same", "Farben vo söba ordnen: Grau, donn jeder Farbton vo dunkl noch hö — des Buidl bleibt gleich"),
    ("Farben in der Palette", "Colors in the palette", "Farben in da Palettn"),
    ("Farben übernehmen aus …", "Take colors from …", "Farben übernehma vo …"),
    ("Freie Farben im Bild: {n}", "Free colors in the image: {n}", "Freie Farben im Buidl: {n}"),
    ("Für Sprite nutzen", "Use for sprite", "Für’n Sprite nehma"),
    ("In die Palette übernehmen", "Add to the palette", "In d’Palettn übernehma"),
    ("Kopie bearbeiten", "Edit a copy", "Kopie bearbeitn"),
    ("Nach Farbstufen", "By shades", "Noch Farbstufn"),
    ("Nachher", "After", "Nochher"),
    ("Vorher", "Before", "Vorher"),
    ("Neue Palette", "New palette", "Neiche Palettn"),
    ("Palette anlegen", "Create palette", "Palettn aunlegn"),
    ("Palette „{name}“ bearbeiten", "Edit palette “{name}”", "Palettn „{name}“ bearbeitn"),
    ("Palette „{name}“ löschen?", "Delete palette “{name}”?", "Palettn „{name}“ weghaun?"),
    ("Palette „{name}“ mit {n} Farben angelegt.", "Created palette “{name}” with {n} colors.", "Palettn „{name}“ mit {n} Farben aunglegt."),
    ("Schon nach Farbstufen geordnet.", "Already sorted by shades.", "Is eh scho noch Farbstufn gordnet."),
    ("Sprite umfärben", "Recolor sprite", "Sprite umfärbn"),
    ("Sprite umgefärbt mit „{name}“.", "Sprite recolored with “{name}”.", "Sprite umgfärbt mit „{name}“."),
    ("Umsortiert in der Kopie „{name}“.", "Reordered in the copy “{name}”.", "In da Kopie „{name}“ umgsortiert."),
    ("Vorlage", "Source", "Vorlog"),
    ("Zuweisen — die Nummern bleiben, die Farben kommen aus dieser Palette", "Assign — the numbers stay, the colors come from this palette", "Zuaweisn — de Nummern bleibn, de Farben kemman aus dera Palettn"),
    ("Zuweisen — die Zeichnung bleibt, wie sie ist", "Assign — the drawing stays as it is", "Zuaweisn — d’Zeichnung bleibt, wia s’ is"),
    ("alle {n}", "all {n}", "olle {n}"),
    ("{n} Bildfarben in „{name}“ aufgenommen.", "Added {n} image colors to “{name}”.", "{n} Buidlfarben in „{name}“ aufgnumma."),
    ("{n} Farben", "{n} colors", ""),
    ("{n} Farben passen nicht mehr in die Palette — „Bild » Palette“ fasst sie zusammen.", "{n} colors no longer fit into the palette — “Image » Palette” merges them.", "{n} Farben passn nimma eini — „Buidl » Palettn“ fosst s’ zamm."),
    ("{n} Sprites nutzen sie und bekommen die Standard-Palette.", "{n} sprites use it and get the default palette.", "{n} Sprites nehman s’ und kriagn de Standard-Palettn."),
    ("„{name}“ ist eine eingebaute Palette — bitte einen anderen Namen.", "“{name}” is a built-in palette — please choose another name.", "„{name}“ is a eibaute Palettn — bitte an aundan Nom."),
    ("„{name}“ zugewiesen — {n} Pixel bleiben als freie Farbe.", "Assigned “{name}” — {n} pixels stay as free colors.", "„{name}“ zuagwiesn — {n} Pixel bleibn ois freie Farb."),
    ("„{name}“ zugewiesen.", "Assigned “{name}”.", "„{name}“ zuagwiesn."),
    // ── Vorschau ──
    ("Vorschau", "Preview", "Vorschau"),
    ("Pixelgröße", "Pixel size", "Pixelgreß"),
    ("{w} × {h} px · {scale}×", "{w} × {h} px · {scale}×", ""),
    // ── Hilfslinien ──
    ("Hilfslinien", "Guides", ""),
    ("Anzeigen", "Show", "Herzagn"),
    ("Alle Hilfslinien ein- und ausblenden (G)", "Show / hide all guides (G)", "Olle Hilfslinien herzagn oder wegtuan (G)"),
    ("Verschieben", "Move", "Vaschiabn"),
    ("Linien auf der Fläche ziehen — solange wird nicht gemalt (Klick daneben oder Esc beendet)", "Drag lines on the drawing area — no painting meanwhile (a click next to the lines or Esc ends it)", "Linien auf da Zeichenflächn ziagn — dawei wird ned gmoit (Klick danebn oder Esc hört auf)"),
    ("Freie Linien", "Free lines", "Freie Linien"),
    ("+ Waagerecht", "+ Horizontal", "+ Waagrecht"),
    ("+ Senkrecht", "+ Vertical", ""),
    ("Eine waagerechte Linie in die Mitte setzen", "Put a horizontal line in the middle", "A waagrechte Linie in d’Mittn setzn"),
    ("Eine senkrechte Linie in die Mitte setzen", "Put a vertical line in the middle", "A senkrechte Linie in d’Mittn setzn"),
    ("Alle löschen", "Delete all", "Olle weghaun"),
    ("Figur — Kopfhöhen", "Figure — head heights", "Figur — Kopfhöhn"),
    ("Aus", "Off", ""),
    ("2 Köpfe — Chibi", "2 heads — chibi", "2 Köpf — Chibi"),
    ("3 Köpfe — klein, niedlich", "3 heads — small, cute", "3 Köpf — kloa und liab"),
    ("4 Köpfe — kompakte Spielfigur", "4 heads — compact game character", "4 Köpf — kompakte Spüfigur"),
    ("6 Köpfe — Comic, Jugendliche", "6 heads — comic, teenager", "6 Köpf — Comic, Jugendliche"),
    ("8 Köpfe — klassisch, heldenhaft", "8 heads — classic, heroic", "8 Köpf — klassisch, heldnhoft"),
    ("An Figur anpassen", "Fit to figure", "An d’Figur anpassn"),
    ("Ober- und Unterkante der Einteilung auf das Gezeichnete setzen", "Set the top and bottom of the division to what is drawn", "Obn und untn von da Einteilung aufs Gmoite setzn"),
    ("Nur zum Zeichnen — die Linien erscheinen in keinem Export.", "Drawing aid only — the lines never show up in an export.", "Nur zum Zeichnen — d’Linien kemman in kan Export."),
    ("Hilfslinien verschieben: Linie anfassen und ziehen, aus dem Bild ziehen löscht. Klick daneben oder Esc beendet.", "Moving guides: grab a line and drag it, dragging it out of the image deletes it. A click next to it or Esc ends this.", "Hilfslinien vaschiabn: Linie packn und ziagn, außem Buidl ziagn haut s’ weg. Klick danebn oder Esc hört auf."),
    ("Hilfslinie entfernt.", "Guide removed.", "Hilfslinie is weg."),
    ("Kinn", "chin", ""),
    ("Brust", "chest", ""),
    ("Nabel", "navel", "Nabl"),
    ("Hüfte", "hip", "Hüftn"),
    ("Schritt", "crotch", ""),
    ("Knie", "knee", "Knia"),
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
            include_str!("sprites_ui.rs"),
            include_str!("preview_ui.rs"),
            include_str!("guides_ui.rs"),
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
