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

/// Einstellungsordner der App (Windows: %APPDATA%\spritebit).
pub(crate) fn settings_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("spritebit"))
}

fn settings_file() -> Option<PathBuf> {
    Some(settings_dir()?.join("lang"))
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
    // ── Licht ──
    ("Licht", "Light", "Liacht"),
    // ── Eigene Hilfslinien-Layouts (guides_ui.rs) ──
    ("Eigene Layouts", "Own layouts", ""),
    ("noch keine gespeichert", "none saved yet", "no kane gspeichert"),
    ("Linien und Einteilung dieses Layouts auf den Sprite legen — bei anderer Größe anteilig umgerechnet", "Put the lines and division of this layout on the sprite — scaled proportionally for another size", ""),
    ("Gewähltes Layout löschen", "Delete the chosen layout", ""),
    ("Name des Layouts", "Name of the layout", ""),
    ("Die jetzigen Linien und die Einteilung als Layout speichern — gleicher Name ersetzt", "Save the current lines and division as a layout — the same name replaces it", ""),
    ("Erst einen Namen für das Layout eingeben.", "Enter a name for the layout first.", ""),
    ("Layout „{name}“ gespeichert — gilt für alle Sprites.", "Layout “{name}” saved — it applies to all sprites.", ""),
    ("Layout „{name}“ ersetzt.", "Layout “{name}” replaced.", ""),
    ("Layout „{name}“ angewendet.", "Layout “{name}” applied.", ""),
    ("Layout „{name}“ angewendet — von {w} × {h} auf diese Größe umgerechnet.", "Layout “{name}” applied — scaled from {w} × {h} to this size.", ""),
    ("Layout „{name}“ gelöscht.", "Layout “{name}” deleted.", ""),
    ("Layouts konnten nicht gespeichert werden.", "Layouts could not be saved.", ""),
    // ── Hilfe: neue Abschnitte (view_ui.rs) ──
    ("Pixel-Editor für Sprites, Animationen und Spiel-Levels. Du malst frei — mit Ebenen, Frames, Licht und Kacheln — oder paust ein Foto als Schablone ab und lässt es automatisch zu einem sauberen Sprite verarbeiten.", "A pixel editor for sprites, animations and game levels. Draw freehand — with layers, frames, light and tiles — or trace a photo as a stencil and have it turned into a clean sprite automatically.", ""),
    ("Arbeitsfläche", "Workspace", ""),
    ("Reiter über der Zeichenfläche zeigen die geöffneten Sprites: Klick wechselt, × oder Mittelklick schließt, Ziehen ordnet.", "Tabs above the drawing area show the open sprites: click switches, × or middle-click closes, dragging reorders.", ""),
    ("In Feldern für Größen darf man rechnen: 24 * 4, 24x4, (16+8)*2 oder 96 : 4.", "Size fields can do maths: 24 * 4, 24x4, (16+8)*2 or 96 : 4.", ""),
    ("Größe 1–64 per Regler oder Zahlenfeld; Alt + rechte Maustaste ziehen verstellt sie direkt auf der Fläche. Ein Umriss zeigt, was Pinsel, Radierer und Spray gleich treffen.", "Size 1–64 with the slider or number field; Alt + right-drag changes it right on the canvas. An outline shows what brush, eraser and spray are about to hit.", ""),
    ("Clean Stroke — beim Stift (und beim Radierer mit Größe 1) verschwinden die L-Ecken einer freihändigen Linie: saubere 1-Pixel-Linien.", "Clean Stroke — with the pencil (and the eraser at size 1) the L-shaped corners of a freehand line disappear: clean 1-pixel lines.", ""),
    ("Hand — schiebt nur die Ansicht; auf einer Hilfslinie zieht sie die Linie. Verschieben geht jederzeit auch mit gehaltener Leertaste oder der mittleren Maustaste.", "Hand — moves the view only; on a guide it drags the line. Panning also works any time by holding Space or with the middle mouse button.", ""),
    ("Freie Linien und Figuren-Proportionen (2–8 Kopfhöhen) — nur zum Zeichnen, nie im Export. G blendet sie ein und aus.", "Free lines and figure proportions (2–8 head heights) — for drawing only, never in the export. G shows and hides them.", ""),
    ("Im Modus „Verschieben“ (oder mit der Hand) Linien ziehen; aus dem Bild gezogen ist eine Linie gelöscht.", "In “Move” mode (or with the hand) drag the lines; dragged out of the image, a line is deleted.", ""),
    ("Eigene Layouts: Linien und Einteilung unter einem Namen speichern und auf jeden Sprite anwenden — bei anderer Größe anteilig umgerechnet.", "Own layouts: save lines and division under a name and apply them to any sprite — scaled proportionally for another size.", ""),
    ("Ebenen: nach unten oder alle sichtbaren zusammenführen. Eine Maske blendet Teile einer Ebene aus, ohne sie zu löschen — beim Bearbeiten blendet Malen aus und Radieren wieder ein.", "Layers: merge down or merge all visible. A mask hides parts of a layer without deleting them — while editing, painting hides and erasing reveals again.", ""),
    ("Zellen: Umschalt+Klick oder Ziehen spannt einen Bereich auf — kopieren, einfügen, leeren, verknüpfen (die Frames teilen sich ein Bild) oder lösen. „Durchgehend“ lässt neue Frames das Bild des vorigen teilen.", "Cels: Shift+click or dragging spans a range — copy, paste, clear, link (the frames share one picture) or unlink. “Continuous” makes new frames share the previous picture.", ""),
    ("Tags benennen einen Abschnitt (z. B. „Laufen“) mit Richtung — vorwärts, rückwärts, Ping-Pong; GIFs gehen auch je Tag.", "Tags name a section (e.g. “Walk”) with a direction — forward, reverse, ping-pong; GIFs can be exported per tag.", ""),
    ("Licht und Schatten", "Light and shadow", ""),
    ("Panel Licht: Lichtquelle aus 8 Richtungen, Stärke und Breite der Kanten, auf Wunsch Schlagschatten. Solange es offen ist, zeigt die Fläche eine Vorschau.", "Light panel: light source from 8 directions, strength and width of the edges, optionally a cast shadow. While it is open the canvas shows a preview.", ""),
    ("„Als Ebene übernehmen“ legt Licht und Schatten als eigene, gesperrte Ebenen an — für alle Frames. Danach rechnet jede Änderung die Ebenen neu.", "“Apply as layer” creates light and shadow as separate, locked layers — for all frames. After that every change recomputes them.", ""),
    ("Kacheln (Tilemaps)", "Tiles (tilemaps)", ""),
    ("Eine Tilemap-Ebene besteht aus Kacheln fester Größe (8–64 px). Malt man eine Kachel an, ändert sie sich überall, wo sie liegt — auch in anderen Frames.", "A tilemap layer is made of tiles of a fixed size (8–64 px). Paint a tile and it changes everywhere it is placed — in other frames too.", ""),
    ("Panel Kacheln: neue Tilemap-Ebene oder die aktive umwandeln. Pixel malen: Auto legt in leeren Zellen neue Kacheln an, Manuell nicht.", "Tiles panel: a new tilemap layer or convert the active one. Paint pixels: Auto creates new tiles in empty cells, Manual does not.", ""),
    ("Kacheln setzen: Kachel in der Liste wählen, dann setzt der Stift sie, Radierer oder Rechtsklick leert, Füllen füllt, Alt+Klick nimmt eine Kachel auf.", "Place tiles: pick a tile in the list, then the pencil places it, eraser or right-click clears, fill fills, Alt+click picks a tile.", ""),
    ("„Für Godot exportieren“ schreibt einen Ordner mit Kachelbild (PNG), Szene (.tscn mit TileMapLayer) und JSON — in den Godot-Projektordner legen (Godot 4.3 oder neuer).", "“Export for Godot” writes a folder with tile image (PNG), scene (.tscn with TileMapLayer) and JSON — put it into your Godot project folder (Godot 4.3 or newer).", ""),
    ("Alt + Klick (Kacheln setzen)", "Alt + click (place tiles)", ""),
    ("Kachel aufnehmen", "Pick a tile", ""),
    // ── Kacheln (tiles_ui.rs) ──
    ("Kacheln", "Tiles", "Kachln"),
    ("Kachelgröße", "Tile size", ""),
    ("Die aktive Ebene ist keine Tilemap. Eine Tilemap besteht aus Kacheln fester Größe — malt man eine Kachel an, ändert sie sich überall, wo sie liegt. Gut für Spiel-Levels und Muster.", "The active layer is not a tilemap. A tilemap is made of tiles of a fixed size — paint a tile and it changes everywhere it is placed. Good for game levels and patterns.", ""),
    ("Gerade wird eine Maske bearbeitet — Kacheln gibt es erst wieder danach.", "A mask is being edited — tiles are back once you are done.", ""),
    ("Neue Tilemap-Ebene", "New tilemap layer", ""),
    ("Neue, leere Tilemap-Ebene über der aktiven anlegen", "Add a new, empty tilemap layer above the active one", ""),
    ("Aktive Ebene umwandeln", "Convert active layer", ""),
    ("Die aktive Ebene in Kacheln zerlegen — gleiche Stellen werden eine Kachel", "Cut the active layer into tiles — identical spots become one tile", ""),
    ("Pixel malen", "Paint pixels", ""),
    ("Kacheln bemalen — eine Kachel ändert sich überall, wo sie liegt", "Paint tiles — a tile changes everywhere it is placed", ""),
    ("Kacheln setzen", "Place tiles", "Kachln setzn"),
    ("Kacheln aus der Liste ins Raster setzen", "Place tiles from the list into the grid", ""),
    ("Neue Kacheln", "New tiles", ""),
    ("Auto — beim Malen anlegen", "Auto — create while painting", ""),
    ("Manuell — nur vorhandene ändern", "Manual — only change existing ones", ""),
    ("Stift setzt die gewählte Kachel, Radierer oder Rechtsklick leert, Füllen füllt, Alt + Klick nimmt eine Kachel auf.", "Pencil places the chosen tile, eraser or right-click clears, fill fills, Alt + click picks a tile.", ""),
    ("Malen ändert die Kachel überall, wo sie liegt. Wer in eine leere Zelle malt, legt eine neue Kachel an.", "Painting changes the tile everywhere it is placed. Painting into an empty cell creates a new tile.", ""),
    ("Malen ändert die Kachel überall, wo sie liegt. Leere Zellen bleiben leer — es entstehen keine neuen Kacheln.", "Painting changes the tile everywhere it is placed. Empty cells stay empty — no new tiles are created.", ""),
    ("{tw} × {th} px · {n} Kacheln · Raster {cols} × {rows}", "{tw} × {th} px · {n} tiles · grid {cols} × {rows}", ""),
    ("Noch keine Kacheln — im Modus „Pixel malen“ (Auto) in eine leere Zelle malen.", "No tiles yet — paint into an empty cell in “Paint pixels” mode (Auto).", ""),
    ("Noch keine Kacheln — im Modus „Pixel malen“ in eine leere Zelle malen.", "No tiles yet — paint into an empty cell in “Paint pixels” mode.", ""),
    ("Kachel {k} — zum Setzen wählen", "Tile {k} — pick to place", ""),
    ("Kachel {k} aufgenommen.", "Picked tile {k}.", ""),
    ("Hier liegt keine Kachel.", "There is no tile here.", "Do liegt ka Kachl."),
    ("Erst eine Kachel in der Liste wählen.", "Pick a tile from the list first.", ""),
    ("Unbenutzte entfernen", "Remove unused", ""),
    ("Kacheln entfernen, die nirgends mehr liegen", "Remove tiles that are not placed anywhere", ""),
    ("{n} unbenutzte Kacheln entfernt.", "Removed {n} unused tiles.", ""),
    ("Alle Kacheln werden benutzt.", "All tiles are in use.", ""),
    ("Normale Ebene", "Normal layer", ""),
    ("Wieder eine normale Ebene — die Pixel bleiben, die Kacheln fallen weg", "Back to a normal layer — the pixels stay, the tiles go", ""),
    ("Wieder eine normale Ebene — die Pixel sind geblieben.", "A normal layer again — the pixels stayed.", ""),
    ("Tilemap {n}", "Tilemap {n}", ""),
    ("Tilemap angelegt — einfach losmalen, jede bemalte Zelle wird eine Kachel.", "Tilemap created — just start painting, every painted cell becomes a tile.", ""),
    ("Tilemap angelegt. Der Sprite ist kein Vielfaches von {tw} × {th} — der dunkle Rand gehört zu keiner Kachel.", "Tilemap created. The sprite is not a multiple of {tw} × {th} — the dark edge belongs to no tile.", ""),
    ("In Kacheln zerlegt: {n} verschiedene Kacheln.", "Cut into tiles: {n} different tiles.", ""),
    ("Manuell: in leere Zellen wird nicht gemalt (dafür „Auto“ wählen).", "Manual: empty cells are not painted (choose “Auto” for that).", ""),
    ("Für Godot exportieren", "Export for Godot", ""),
    ("Ordner mit Kachelbild (PNG), Godot-Szene (.tscn mit TileMapLayer) und JSON — den Ordner ins Godot-Projekt (res://) legen", "Folder with tile image (PNG), Godot scene (.tscn with TileMapLayer) and JSON — put the folder into your Godot project (res://)", ""),
    ("Keine Tilemap-Ebene mit Kacheln zum Exportieren.", "No tilemap layer with tiles to export.", ""),
    ("Godot-Projekt oder Ordner wählen", "Choose the Godot project or a folder", ""),
    ("Gespeichert in „{dir}“ — liegt der Ordner im Godot-Projekt, die .tscn öffnen.", "Saved in “{dir}” — with the folder inside your Godot project, open the .tscn.", ""),
    ("Lichtquelle", "Light source", "Wo’s Liacht herkummt"),
    ("Licht von oben links", "Light from the top left", "Liacht vo obn links"),
    ("Licht von oben", "Light from above", "Liacht vo obn"),
    ("Licht von oben rechts", "Light from the top right", "Liacht vo obn rechts"),
    ("Licht von links", "Light from the left", "Liacht vo links"),
    ("Licht von rechts", "Light from the right", "Liacht vo rechts"),
    ("Licht von unten links", "Light from the bottom left", "Liacht vo untn links"),
    ("Licht von unten", "Light from below", "Liacht vo untn"),
    ("Licht von unten rechts", "Light from the bottom right", "Liacht vo untn rechts"),
    ("Wie viele Pixel vom Rand her beleuchtet bzw. schattiert werden", "How many pixels from the edge get lit or shaded", "Wia vü Pixel vom Rand her hö bzw. dunkl wern"),
    ("Lichtkante (heller)", "Light edge (brighter)", "Liachtkantn (heller)"),
    ("Schattenkante (dunkler)", "Shadow edge (darker)", "Schottnkantn (dunkler)"),
    ("Auch Farben außerhalb der Palette", "Allow colors outside the palette", "A Farben außahoib vo da Palettn"),
    (
        "Fehlt in der Palette eine passende hellere oder dunklere Farbe, wird eine freie Farbe berechnet — sonst bleibt der Pixel, wie er ist",
        "If the palette has no matching lighter or darker color, compute a free color — otherwise the pixel stays as it is",
        "",
    ),
    ("Licht anwenden", "Apply light", "Liacht drauf"),
    (
        "Kanten zur Lichtquelle hin aufhellen, abgewandte Kanten abdunkeln — auf der aktiven Zelle, mit Auswahl nur darin",
        "Brighten edges facing the light, darken edges facing away — on the active cel, only inside the selection if there is one",
        "",
    ),
    ("Schlagschatten", "Drop shadow", "Schlagschottn"),
    ("Schattenfarbe", "Shadow color", "Farb vom Schottn"),
    ("Wie weit der Schatten fällt", "How far the shadow falls", "Wia weit da Schottn foit"),
    ("Werfen", "Cast", "Werfn"),
    (
        "Silhouette von der Lichtquelle weg versetzt als Schatten in leere Pixel malen",
        "Paint the silhouette, offset away from the light, as a shadow into empty pixels",
        "",
    ),
    ("Licht gesetzt — {lit} Pixel heller, {shaded} dunkler.", "Light applied — {lit} pixels brighter, {shaded} darker.", "Liacht gsetzt — {lit} Pixel heller, {shaded} dunkler."),
    (
        "Nichts beleuchtet — keine Kanten oder keine passenden Palettenfarben.",
        "Nothing lit — no edges or no matching palette colors.",
        "Nix beleicht — kane Kantn oder kane passendn Farben in da Palettn.",
    ),
    ("Schlagschatten gemalt — {n} Pixel.", "Drop shadow painted — {n} pixels.", "Schlagschottn gmoit — {n} Pixel."),
    (
        "Kein Platz für einen Schatten — Sprite leer oder Rand erreicht?",
        "No room for a shadow — sprite empty or at the border?",
        "Ka Plotz fia an Schottn — Sprite laar oder am Rand?",
    ),
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
    // ── Schablone ──
    ("Schablone", "Stencil", ""),
    ("Aufs Raster übernehmen", "Copy onto the grid", "Aufs Raster übernehma"),
    ("Bild laden …", "Load image …", "Buidl lodn …"),
    ("Bilder", "Images", "Buidln"),
    ("Schablone laden", "Load stencil", "Schablone lodn"),
    ("Das Bild „{name}“ ließ sich nicht laden: {e}", "The image “{name}” could not be loaded: {e}", "S’Buidl „{name}“ hot si ned lodn lossn: {e}"),
    ("Deckkraft", "Opacity", ""),
    ("Dort ist die Schablone durchsichtig.", "The stencil is transparent there.", "Do is d’Schablone durchsichtig."),
    ("Dort liegt keine Schablone.", "There is no stencil there.", "Do liegt ka Schablone."),
    ("Ein Foto oder Bild zum Abzeichnen — es liegt hinter den Pixeln.", "A photo or picture to trace — it lies behind the pixels.", "A Foto oder Buidl zum Obzeichnen — es liegt hinter de Pixel."),
    ("Entfernen", "Remove", "Weghaun"),
    ("Farbe aus der Schablone: {hex}", "Color from the stencil: {hex}", "Farb aus da Schablone: {hex}"),
    ("Genau die Farben des Bildes — als freie Farben", "Exactly the image’s colors — as free colors", "Genau de Farben vom Buidl — ois freie Farben"),
    ("Jede Zelle bekommt die nächste Farbe der Palette", "Each cell gets the closest color of the palette", "Jede Zön kriagt de nächste Farb vo da Palettn"),
    ("Keine Farben in der Schablone gefunden.", "No colors found in the stencil.", "Kane Farben in da Schablone gfunden."),
    ("Keine Pixel geändert — Schablone über der Fläche positionieren?", "No pixels changed — is the stencil placed over the canvas?", "Ka Pixel gändert — d’Schablone über d’Flächn legn?"),
    ("Originalfarben", "Original colors", "Originalfarbn"),
    ("Palettenfarben", "Palette colors", "Palettnfarbn"),
    ("Reduzieren auf", "Reduce to", "Owe auf"),
    ("Schablone übernommen — {n} Pixel ({mode}).", "Stencil applied — {n} pixels ({mode}).", ""),
    ("Umschalt+Alt halten: vorne zeigen · ziehen: verschieben · Klick: Farbe nehmen", "Hold Shift+Alt: show in front · drag: move · click: pick color", "Umschalt+Alt hoitn: vorn zagn · ziagn: vaschiabn · Klick: Farb nehma"),
    // ── Code und Export ──
    (" — Achtung: Index {list} kommt im Grid vor, fehlt aber in der Palette.", " — careful: index {list} appears in the grid but is missing from the palette.", " — Obacht: Index {list} kummt im Raster vor, fehlt oba in da Palettn."),
    ("// Der Sprite ist groß ({n} Pixel über alle Frames) — der Code wird erst beim Kopieren oder Speichern erzeugt.", "// The sprite is large ({n} pixels across all frames) — the code is built only when you copy or save it.", ""),
    ("Alle Pixel dieses Sprites löschen", "Erase every pixel of this sprite", "Olle Pixel von dem Sprite weghaun"),
    ("Alle Sprites in einem Bild — dazu ein JSON-Atlas mit Namen und Koordinaten", "All sprites in one image — plus a JSON atlas with names and coordinates", "Olle Sprites in an Buidl — dazua a JSON-Atlas mit Nom und Koordinatn"),
    ("Als neuen Sprite", "As a new sprite", "Ois neicha Sprite"),
    ("Animation als GIF — alle Frames, läuft endlos", "Animation as GIF — all frames, loops forever", "Animation ois GIF — olle Frames, rennt ewig"),
    ("Bettet eine Farb-Legende ins Bild ein, damit die Farbwerte nicht verloren gehen", "Embeds a color key into the image so the color values are not lost", "Bettet a Farb-Legende ins Buidl ei, damit de Farbwerte ned valurn gengan"),
    ("Code & Export", "Code & export", ""),
    ("Code hier einfügen …", "Paste code here …", "Code do einifügn …"),
    ("Code kopiert.", "Code copied.", "Code kopiert."),
    ("Code speichern", "Save code", ""),
    ("Datei wählen", "Choose file", "A Datei aussuachn"),
    ("Erkannt: ", "Recognised: ", ""),
    ("Farb-Legende ins Bild", "Color key in the image", "Farb-Legende ins Buidl"),
    ("Format", "Format", ""),
    ("GIF: eine Datei je Tag", "GIF: one file per tag", "GIF: a Datei pro Tag"),
    ("Gilt für alle Sprites mit dieser Palette. Index 0 ist immer „empty“, freie Farben bekommen „none“.", "Applies to every sprite using this palette. Index 0 is always “empty”, free colors get “none”.", "Gilt für olle Sprites mit dera Palettn. Index 0 is imma „empty“, freie Farben kriagn „none“."),
    ("Import", "Import", ""),
    ("Import fertig — Palette „{name}“ übernommen und zugewiesen.", "Import done — palette “{name}” taken over and assigned.", "Einelesn passt — Palettn „{name}“ übernumma und zuagwiesn."),
    ("Import fertig. (Keine Palette im Text gefunden — Farben bleiben wie eingestellt.)", "Import done. (No palette found in the text — the colors stay as they are.)", "Einelesn passt. (Kane Palettn im Text gfunden — d’Farben bleibn wia eigstöllt.)"),
    ("Import …", "Import …", "Einelesn …"),
    ("In aktuellen Sprite", "Into the current sprite", "In den aktuelln Sprite"),
    ("JSON (Spiel)", "JSON (game)", ""),
    ("Material je Farbe — Palette „{name}“", "Material per color — palette “{name}”", "Material pro Farb — Palettn „{name}“"),
    ("Name „{name}“", "name “{name}”", "Nom „{name}“"),
    ("PDF exportieren", "Export PDF", "PDF exportiern"),
    ("Palette aus der Datei übernehmen und dem Sprite zuweisen", "Take the palette from the file and assign it to the sprite", "D’Palettn aus da Datei übernehma und an Sprite zuaweisn"),
    ("Palette in den Code schreiben", "Write the palette into the code", "D’Palettn in den Code schreibn"),
    ("Palette mit {n} Farben", "palette with {n} colors", "Palettn mit {n} Farben"),
    ("Palette — {n} Farben", "Palette — {n} colors", ""),
    ("Palette — {n} Farben (nach Farbton sortiert)", "Palette — {n} colors (sorted by hue)", ""),
    ("SVG-Bild", "SVG image", "SVG-Buidl"),
    ("Skalierung", "Scale", "Gressn"),
    ("Speichern …", "Save …", ""),
    ("Sprite importieren", "Import sprite", "Sprite einelesn"),
    ("Spritesheet", "Spritesheet", ""),
    ("Text-Raster", "Text grid", ""),
    ("TypeScript, JavaScript, JSON, JSON (Spiel), Python und C-Header behalten ihre Farb-Nummern; SVG, CSS und Text-Raster werden neu durchnummeriert.", "TypeScript, JavaScript, JSON, JSON (game), Python and C header keep their color numbers; SVG, CSS and text grid are renumbered.", "TypeScript, JavaScript, JSON, JSON (Spiel), Python und C-Header bhoitn eanare Farb-Nummern; SVG, CSS und Text-Raster wern nei durchnummeriert."),
    ("keine Palette gefunden", "no palette found", "kane Palettn gfunden"),
    ("{n} Frames", "{n} frames", ""),
    ("{n} Frames markiert — PNG und PDF speichern je eine Datei, das GIF enthält nur diese Frames.", "{n} frames selected — PNG and PDF save one file each, the GIF holds just these frames.", "{n} Frames markiert — PNG und PDF speichern a Datei pro Frame, des GIF hot nur de."),
    ("{n} GIFs — eine je Tag.", "{n} GIFs — one per tag.", "{n} GIFs — ans pro Tag."),
    ("{n} freie Farb-Pixel wiederhergestellt", "{n} free color pixels restored", "{n} freie Farb-Pixel wieder do"),
    ("{w}×{h} Pixel", "{w}×{h} pixels", ""),
    ("{n} Frames gespeichert — von „{first}“ bis „{last}“ in „{dir}“.", "{n} frames saved — from “{first}” to “{last}” in “{dir}”.", "{n} Frames gspeichert — vo „{first}“ bis „{last}“ in „{dir}“."),
    ("Spritesheet mit {n} Sprites gespeichert — „{png}“ und „{json}“ in „{dir}“.", "Spritesheet with {n} sprites saved — “{png}” and “{json}” in “{dir}”.", "Spritesheet mit {n} Sprites gspeichert — „{png}“ und „{json}“ in „{dir}“."),
    ("Nichts eingefügt.", "Nothing pasted.", "Nix einghaut."),
    ("SVG erkannt, aber kein <rect> darin gefunden.", "SVG recognised, but no <rect> found inside it.", "SVG erkannt, oba ka <rect> drin gfunden."),
    ("SVG erkannt, aber kein <rect> mit Füllfarbe gefunden.", "SVG recognised, but no <rect> with a fill color.", "SVG erkannt, oba ka <rect> mit Füllfarb gfunden."),
    ("SVG ist {w}×{h} groß — das Raster wäre zu fein.", "The SVG is {w}×{h} — that grid would be far too fine.", "S’SVG is {w}×{h} groß — des Raster wär z’fein."),
    ("Kein box-shadow-Block gefunden.", "No box-shadow block found.", "Ka box-shadow-Block gfunden."),
    ("box-shadow gefunden, aber keine Pixel darin gelesen.", "box-shadow found, but no pixels could be read from it.", "box-shadow gfunden, oba kane Pixel drin glesn."),
    ("Das ergäbe {w}×{h} Pixel — zu groß.", "That would be {w}×{h} pixels — too big.", "Des warn {w}×{h} Pixel — z’groß."),
    ("C-Header ohne _WIDTH und _HEIGHT — Maße unbekannt.", "C header without _WIDTH and _HEIGHT — the size is unknown.", "C-Header ohne _WIDTH und _HEIGHT — de Moß san unbekannt."),
    ("Maße {w}×{h} sind nicht brauchbar.", "The size {w}×{h} is not usable.", "De Moß {w}×{h} san ned zum braucha."),
    ("C-Header ohne _DATA-Feld — keine Pixel gefunden.", "C header without a _DATA array — no pixels found.", "C-Header ohne _DATA-Föd — kane Pixel gfunden."),
    ("_DATA hat {have} Werte, für {w}×{h} braucht es {need}.", "_DATA has {have} values, {w}×{h} needs {need}.", "_DATA hot {have} Werte, für {w}×{h} brauchts {need}."),
    ("Kein Zeichenraster gefunden (gleich lange Zeilen aus . und 1-9).", "No character grid found (lines of equal length made of . and 1-9).", "Ka Zeichnraster gfunden (gleich lange Zeiln aus . und 1-9)."),
    ("Raster ist {w}×{h} — zu groß.", "The grid is {w}×{h} — too big.", "S’Raster is {w}×{h} — z’groß."),
    ("Kein gültiges number[][]-Array gefunden. Erwartet wird [[0,1,…], …].", "No valid number[][] array found. Expected [[0,1,…], …].", "Ka gültigs number[][]-Array gfunden. Erwortet wird [[0,1,…], …]."),
    ("JSON (Spiel) erkannt, aber {reason}", "JSON (game) recognised, but {reason}", "JSON (Spiel) erkannt, oba {reason}"),
    ("unbekannte Version {version}.", "unknown version {version}.", ""),
    ("ungültige Größe {w}×{h}.", "invalid size {w}×{h}.", ""),
    ("die Palette ist leer.", "the palette is empty.", ""),
    ("Index 0 muss transparent sein, ist aber {color}.", "index 0 must be transparent but is {color}.", ""),
    ("ungültige Farbe {color}.", "invalid color {color}.", ""),
    ("unbekanntes Material „{material}“ bei Index {i}.", "unknown material “{material}” at index {i}.", ""),
    ("data hat {len} Werte, erwartet sind {w} × {h} = {expected}.", "data has {len} values, expected {w} × {h} = {expected}.", ""),
    ("Pixel ({x}, {y}) hat Index {value}, gültig ist 0 bis {max}.", "pixel ({x}, {y}) has index {value}, valid is 0 to {max}.", ""),
    ("Ausschnitt „{name}“ liegt nicht vollständig im Bild.", "region “{name}” does not lie fully inside the image.", ""),
    ("durations braucht {need} ganze Zahlen (ms), eine je Frame.", "durations needs {need} whole numbers (ms), one per frame.", ""),
    // ── Timeline-Einstellungen ──
    ("Position", "Position", "Wo"),
    ("0 = Dauer nach FPS.", "0 = duration from FPS.", "0 = Dauer noch FPS."),
    ("Abstufung", "Fade", "Obstufung"),
    ("Aktueller Frame", "Current frame", ""),
    ("Am Ende des Tags scheint sein Anfang durch — für Animationen, die im Kreis laufen", "At the end of a tag its start shows through — for animations that loop", "Am End vom Tag scheint sei Ofang durch — für Animationen, de im Kreis rennan"),
    ("Darstellung", "Display", "Ausschaun"),
    ("Davor", "In front", ""),
    ("Erster Frame", "First frame", ""),
    ("Frames davor", "Frames before", ""),
    ("Hinter dem Bild", "Behind the image", "Hintam Büd"),
    ("Im Tag im Kreis", "Loop within the tag", "Im Tag rundumadum"),
    ("Kopfzeile", "Header", "Kopfzeiln"),
    ("Lage", "Position", ""),
    ("Links", "Left", ""),
    ("Nur die aktive Ebene", "Active layer only", "Nur de aktive Ebene"),
    ("Oben", "Top", "Obn"),
    ("Onion Skin", "Onion skin", ""),
    ("Rechts", "Right", ""),
    ("Rot/Blau", "Red/blue", ""),
    ("Timeline", "Timeline", ""),
    ("Timeline-Einstellungen — Lage, Kopfzeile, Dauer, Onion Skin", "Timeline settings — position, header, duration, onion skin", "Timeline-Einstellungen — wo, Kopfzeiln, Dauer, Onion Skin"),
    ("Um wie viel jeder weitere Frame blasser wird", "How much fainter each farther frame gets", "Um wia vü jeder weitere Frame blasser wird"),
    ("Unten", "Bottom", "Untn"),
    ("Vorschaubilder", "Thumbnails", "Vorschaubüdln"),
    ("danach", "after", "danoch"),
    // ── Ansicht, Hilfe, Sicherung ──
    ("Alles · Kopieren · Ausschneiden · Einfügen", "All · Copy · Cut · Paste", "Ois · Kopiern · Ausschneidn · Einifügn"),
    ("Auswahl (A) zieht ein Rechteck auf, Lasso (L) umfährt eine freie Form, Farbwahl (K) nimmt die zusammenhängende ähnliche Fläche.", "Select (A) drags a rectangle, Lasso (L) traces a free shape, Color select (K) takes the connected similar area.", "Auswoi (A) ziagt a Viereck auf, Lasso (L) umfoahrt a freie Form, Farbwoi (K) nimmt de zammhängende ähnliche Flächn."),
    ("Auswahl aufheben, Drehung verwerfen oder Vollbild beenden", "Deselect, discard rotation or leave full screen", "Auswoi weg, Drahung weghaun oder Vollbüd beendn"),
    ("Auswahl pixelweise verschieben", "Move the selection pixel by pixel", "D’Auswoi pixelweis vaschiabn"),
    ("Auswahl · Lasso · Farbwahl", "Select · Lasso · Color select", "Auswoi · Lasso · Farbwoi"),
    ("Bild laden, mit Umschalt+Alt ziehen verschieben, Deckkraft und Größe per Regler; Umschalt+Alt halten zeigt sie vorn.", "Load an image, Shift+Alt drag moves it, opacity and size by slider; holding Shift+Alt shows it in front.", "Buidl lodn, mit Umschalt+Alt ziagn vaschiabn, Deckkraft und Greß per Regler; Umschalt+Alt hoitn zagt s’ vorn."),
    ("Bild verschieben (auch mit mittlerer Maustaste)", "Move the image (also with the middle mouse button)", "S’Buidl vaschiabn (a mit da mittlern Maustastn)"),
    ("Code & Export erzeugt TypeScript, JavaScript, JSON, JSON (Spiel), SVG, CSS, C-Header, Python oder ein Text-Raster — und liest alles davon wieder ein.", "Code & export writes TypeScript, JavaScript, JSON, JSON (game), SVG, CSS, C header, Python or a text grid — and reads all of them back in.", "Code & Export schreibt TypeScript, JavaScript, JSON, JSON (Spiel), SVG, CSS, C-Header, Python oder a Text-Raster — und liest ois wieder ei."),
    ("Datenschutz", "Privacy", ""),
    ("Den aktuellen Stand durch diese Sicherung ersetzen? Der aktuelle Stand wird dabei selbst gesichert.", "Replace the current state with this backup? The current state is backed up first.", "Den aktuelln Stand durch de Sicherung ersetzn? Da aktuelle Stand wird dabei söba gsichert."),
    ("Die Knöpfe ↔ und ↕ spiegeln jeden Strich an der Mittelachse — beide zusammen ergeben vier Spiegelungen.", "The ↔ and ↕ buttons mirror every stroke at the center axis — both together give four reflections.", "De Knöpf ↔ und ↕ spiagln jeden Strich an da Mittlachsn — beide zamm gebn via Spieglungen."),
    ("Die laufende Sitzung wird ständig im Einstellungsordner gesichert — stürzt die App ab, ist beim nächsten Start alles wieder da. Zusätzlich bleibt der Stand vom Start der Sitzung aufgehoben. Am sichersten ist trotzdem „Speichern“.", "The running session is saved continuously in the settings folder — if the app crashes, everything is back at the next start. The state from the start of the session is kept as well. “Save” is still the safest option.", "De laufende Sitzung wird dauernd im Einstellungsordner gsichert — stürzt de App ob, is beim nächstn Start ois wieder do. Dazua bleibt da Stand vom Start da Sitzung aufghobn. Am sichastn is trotzdem „Speichern“."),
    ("Die letzte Sitzung wurde nicht sauber beendet — ihr Stand ist wiederhergestellt.", "The last session did not end cleanly — its state has been restored.", "De letzte Sitzung is ned sauba beendet wordn — ihr Stand is wieder do."),
    ("Drehung übernehmen · sonst Animation abspielen / anhalten", "Apply rotation · otherwise play / pause the animation", "Drahung übernehma · sonst Animation obspün / aunhoitn"),
    ("Dunkel", "Dark", "Dunkl"),
    ("Ebenen und Animation", "Layers and animation", "Ebenen und Animation"),
    ("Eine Palette ordnet jeder Nummer eine Farbe zu. 0 ist immer transparent.", "A palette gives every number a color. 0 is always transparent.", "A Palettn gibt jeda Nummer a Farb. 0 is imma durchsichtig."),
    ("Eingebaute Paletten sind schreibgeschützt — „Kopie bearbeiten“ macht eine änderbare eigene daraus.", "Built-in palettes are read-only — “Edit a copy” makes an editable one of your own.", "Eibaute Palettn san schreibgschützt — „Kopie bearbeitn“ mocht a eigene draus, de ma ändern ko."),
    ("Entf", "Del", ""),
    ("Farbe mit dieser Nummer", "Color with this number", "Farb mit dera Nummer"),
    ("Farben in der Farbzeile ziehen sortiert um, „Nach Farbstufen“ ordnet automatisch — das Bild bleibt gleich.", "Dragging colors in the color row reorders them, “By shades” sorts automatically — the image stays the same.", "Farben in da Farbzeiln ziagn sortiert um, „Noch Farbstufn“ ordnet vo söba — s’Buidl bleibt gleich."),
    ("Farbpaletten", "Color palettes", "Farbpalettn"),
    ("Vom Foto zum Sprite: Schablone laden, „Reduzieren auf“ übernehmen, „Bild » Palette“, Hintergrund entfernen, glätten, Outline.", "From photo to sprite: load a stencil, apply “Reduce to”, “Image » Palette”, remove the background, despeckle, outline.", "Vom Foto zum Sprite: Schablone lodn, „Owe auf“ übernehma, „Buidl » Palettn“, Hintagrund weghaun, glattmochn, Umrandung."),
    ("Frame löschen (markierte alle)", "Delete frame (all marked ones)", "Frame weghaun (olle markiertn)"),
    ("Frei drehen: der Regler zeigt eine Vorschau; Enter übernimmt, Esc geht zurück auf 0°.", "Free rotate: the slider shows a preview; Enter applies, Esc goes back to 0°.", "Frei drahn: da Regler zagt a Vorschau; Enter übernimmt, Esc geht zruck auf 0°."),
    ("Füllen — zusammenhängende gleiche Fläche. Radierer — setzt auf transparent. Zauberstab — löscht zusammenhängende ähnliche Fläche.", "Fill — connected area of the same color. Eraser — sets to transparent. Magic wand — erases a connected similar area.", "Aufülln — zammhängende gleiche Flächn. Radiergummi — mocht durchsichtig. Zauberstaberl — haut a zammhängende ähnliche Flächn weg."),
    ("Gemalt wird in die aktive Ebene; angezeigt werden alle sichtbaren übereinander. Auge blendet aus, Schloss sperrt.", "You paint into the active layer; all visible layers are shown on top of each other. The eye hides, the lock locks.", "Gmoit wird in de aktive Ebene; zagt werdn olle sichtbarn übereinand. S’Aug blendt aus, s’Schloss sperrt."),
    ("Hand — Ansicht verschieben", "Hand — move the view", "Hand — Aunsicht vaschiabn"),
    ("Hand — schiebt nur die Ansicht. Dasselbe geht jederzeit mit gehaltener Leertaste oder der mittleren Maustaste.", "Hand — only moves the view. The same works anytime with the space bar held or the middle mouse button.", "Hand — schiabt nur d’Aunsicht. Des Gleiche geht imma mit ghoitena Leertastn oder da mittlern Maustastn."),
    ("Hell", "Light", "Hö"),
    ("Herunterladen", "Download", "Owalodn"),
    ("Hilfe und Tastenkürzel", "Help and shortcuts", "Hilfe und Tastnkürzl"),
    ("Hilfslinien ein / aus", "Guides on / off", "Hilfslinien ei / aus"),
    ("Hoch / runter scrollen (Umschalt: links / rechts)", "Scroll up / down (Shift: left / right)", "Aufi / obi scrolln (Umschalt: links / rechts)"),
    ("Import und Export", "Import and export", "Einelesn und Export"),
    ("Impressum", "Legal notice", ""),
    ("In der Timeline: Klick wählt, Strg+Klick und Umschalt+Klick markieren mehrere Frames, Ziehen sortiert Frames und Ebenen um.", "In the timeline: click selects, Ctrl+click and Shift+click mark several frames, dragging reorders frames and layers.", "In da Timeline: Klick wöht, Strg+Klick und Umschalt+Klick markiern mehrere Frames, ziagn sortiert Frames und Ebenen um."),
    ("In die Auswahl fassen und ziehen hebt den Inhalt an — er schwebt, bis du ihn absetzt. Alt+Ziehen verschiebt eine Kopie.", "Grab inside the selection and drag to lift the content — it floats until you put it down. Alt+drag moves a copy.", "In d’Auswoi greifn und ziagn hebt den Inhalt au — er schwebt, bis’dn obsetzt. Alt+Ziagn vaschiabt a Kopie."),
    ("Jeder Sprite merkt sich seine eigene Palette. Eine Farbe ändern färbt alle Pixel mit dieser Nummer sofort um.", "Every sprite remembers its own palette. Changing a color recolors every pixel with that number right away.", "Jeda Sprite merkt si sei eigene Palettn. A Farb ändern färbt olle Pixel mit dera Nummer glei um."),
    ("Klick", "Click", ""),
    ("Leertaste + Ziehen", "Space + drag", ""),
    ("Linie · Rechteck · Ellipse", "Line · Rectangle · Ellipse", "Linie · Viereck · Ellipse"),
    ("Linie · Rechteck · Ellipse — aufziehen, Loslassen zeichnet. „Gefüllt“ schaltet zwischen Kontur und Fläche.", "Line · Rectangle · Ellipse — drag out, releasing draws. “Filled” switches between outline and area.", "Linie · Viereck · Ellipse — aufziagn, auslossn zeichnet. „Ausgfüllt“ schoit zwischn Kontur und Flächn."),
    ("Löschen (gedrückt halten = durchgehend)", "Erase (hold = continuously)", "Weghaun (druckt hoitn = durchgehend)"),
    ("Malen", "Paint", "Moin"),
    ("Mausrad", "Mouse wheel", ""),
    ("Noch keine Sicherung vorhanden.", "No backup yet.", "No ka Sicherung do."),
    ("Onion Skin lässt Nachbar-Frames durchscheinen — einstellbar im Zahnrad-Menü der Timeline.", "Onion skin lets neighboring frames show through — adjustable in the gear menu of the timeline.", "Onion Skin lasst de Nochbar-Frames durchscheinan — eistöllbar im Zahnrad-Menü vo da Timeline."),
    ("PNG und PDF exportieren den aktuellen (oder die markierten) Frames, GIF die Animation, das Spritesheet alle Sprites samt JSON-Atlas.", "PNG and PDF export the current (or the marked) frames, GIF the animation, the sprite sheet all sprites with a JSON atlas.", "PNG und PDF exportiern den aktuelln (oder de markiertn) Frames, GIF de Animation, s’Spritesheet olle Sprites mitsamt JSON-Atlas."),
    ("Pfeiltasten", "Arrow keys", "Pfeiltastn"),
    ("Pfeiltasten verschieben pixelweise. Esc oder ein Klick daneben hebt die Auswahl auf.", "Arrow keys move pixel by pixel. Esc or a click next to it removes the selection.", "Pfeiltastn vaschiabn pixelweis. Esc oder a Klick danebn haut d’Auswoi weg."),
    ("Pinsel und Radierer: Dichte — Spray: Menge je Schritt", "Brush and eraser: density — spray: amount per step", "Pinsl und Radiergummi: Dichte — Spray: Mengn pro Schritt"),
    ("Pipette", "Eyedropper", ""),
    ("Pixel-Editor mit Foto-Vorlage. Du malst frei — oder paust ein Foto als Schablone ab und lässt es automatisch zu einem sauberen Sprite verarbeiten.", "A pixel editor with a photo stencil. Draw freehand — or trace a photo as a stencil and have it turned into a clean sprite automatically.", "Pixel-Editor mit Foto-Vorlog. Du moist frei — oder paust a Foto ois Schablone ob und losst’s vo söba zu an saubern Sprite wern."),
    ("Pos1 / Ende", "Home / End", ""),
    ("Rechtsklick", "Right-click", ""),
    ("Rückgängig / Wiederholen", "Undo / Redo", "Zruck / Doch wieder"),
    ("Schablone: halten = vorn, ziehen = verschieben, Klick = Farbe", "Stencil: hold = in front, drag = move, click = color", "Schablone: hoitn = vorn, ziagn = vaschiabn, Klick = Farb"),
    ("Sicherung", "Backup", ""),
    ("Sicherung vom Sitzungsstart: {time}", "Backup from the start of the session: {time}", "Sicherung vom Sitzungsstart: {time}"),
    ("Sicherung wiederhergestellt.", "Backup restored.", "Sicherung is wieder do."),
    ("Speichern / Öffnen / Exportieren", "Save / Open / Export", "Speichern / Aufmochn / Exportiern"),
    ("Spiegeln und Drehen wirken auf die Auswahl, wenn es eine gibt — sonst auf den ganzen Sprite.", "Flip and rotate act on the selection if there is one — otherwise on the whole sprite.", "Spiagln und Drahn wirkn auf d’Auswoi, wenn’s ane gibt — sonst auf’n ganzn Sprite."),
    ("Startseite", "Start page", ""),
    ("Stift · Pinsel · Spray · Füllen · Radierer · Zauberstab", "Pencil · Brush · Spray · Fill · Eraser · Magic wand", "Stift · Pinsl · Spray · Aufülln · Radiergummi · Zauberstaberl"),
    ("Stift — einzelne Pixel. Pinsel — Fläche; Stärke = Dichte, Größe = Kantenlänge. Spray — zufällige Pixel; Stärke = Menge.", "Pencil — single pixels. Brush — area; strength = density, size = edge length. Spray — random pixels; strength = amount.", "Stift — anzelne Pixel. Pinsl — Flächn; Stärkn = Dichte, Greß = Kantnlängn. Spray — zufällige Pixel; Stärkn = Mengn."),
    ("Strg + A / C / X / V", "Ctrl + A / C / X / V", ""),
    ("Strg + Mausrad", "Ctrl + mouse wheel", ""),
    ("Strg + S / O / E", "Ctrl + S / O / E", ""),
    ("Strg + Z / Y", "Ctrl + Z / Y", ""),
    ("Stärke", "Strength", "Stärkn"),
    ("Symmetrie", "Symmetry", ""),
    ("Tastenkürzel", "Shortcuts", "Tastnkürzl"),
    ("Vollbild", "Full screen", "Vollbüd"),
    ("Voriger / nächster Frame", "Previous / next frame", "Da Frame davor / da nächste"),
    ("Werkzeuge", "Tools", "Werkzeig"),
    ("Wiederherstellen", "Restore", "Wieder herhoin"),
    ("Zeigen", "Show", "Herzagn"),
    ("Zeigt, wo die aktuelle Farbe im Bild vorkommt — alles andere wird abgedunkelt", "Shows where the current color appears in the image — everything else is dimmed", "Zagt, wo de aktuelle Farb im Buidl vorkummt — da Rest wird obdunklt"),
    ("Zoom", "Zoom", ""),
    ("Zoomen auf den Mauszeiger", "Zoom at the mouse pointer", "Zoomen auf’n Mauszeiger"),
    ("Zum ersten / letzten Frame", "To the first / last frame", "Zum erstn / letztn Frame"),
    ("Zuschneiden, Zentrieren, Leinwand (ohne zu skalieren) und ×2 / ÷2 wirken auf alle Frames.", "Trim, center, canvas (without scaling) and ×2 / ÷2 act on all frames.", "Zuaschneidn, Zentriern, Leinwaund (ohne Skaliern) und ×2 / ÷2 wirkn auf olle Frames."),
    ("{n} Pixel in dieser Farbe", "{n} pixels in this color", "{n} Pixel in dera Farb"),
    // ── Auswahl-Leiste ──
    ("Alles", "All", "Ois"),
    // ── Licht als Ebene ──
    ("Keine Ebene, auf die das Licht wirken kann.", "No layer the light could act on.", "Ka Ebene, auf de des Liacht wirkn kau."),
    ("Licht · {name}", "Light · {name}", "Liacht · {name}"),
    ("Schatten · {name}", "Shadow · {name}", "Schottn · {name}"),
    ("Licht neu berechnen", "Recompute light", "Liacht nei rechnen"),
    ("Licht-Ebene anlegen", "Add light layer", "Liacht-Ebene aulegn"),
    (
        "Legt über der aktiven Ebene eine Licht-Ebene an: Kanten zur Lichtquelle hin heller, abgewandte dunkler. Das Original bleibt unverändert; Richtung, Stärke und Breite ändern rechnet die Ebene sofort neu",
        "Adds a light layer above the active layer: edges facing the light get brighter, edges facing away darker. The original stays untouched; changing direction, strength or width recomputes the layer right away",
        "",
    ),
    ("Neu werfen", "Cast again", "Nei werfn"),
    ("Schattenfarbe übernehmen und den Schatten neu berechnen", "Apply the shadow colour and recompute the shadow", ""),
    ("Vorschau — im Bild ist noch nichts verändert.", "Preview — nothing in the image has changed yet.", "Vorschau — im Buidl is no nix gändert."),
    ("Als Ebene übernehmen", "Apply as layer", "Ois Ebene übernehma"),
    (
        "Legt Licht (und, wenn angehakt, Schatten) als eigene Ebenen an — das Original bleibt unverändert. Danach rechnet jede Änderung hier die Ebenen sofort neu",
        "Adds light (and, if ticked, shadow) as separate layers — the original stays untouched. Afterwards every change here recomputes the layers right away",
        "",
    ),
    (
        "Legt unter der aktiven Ebene eine Schatten-Ebene an: die Silhouette, von der Lichtquelle weg versetzt",
        "Adds a shadow layer below the active layer: the silhouette, offset away from the light",
        "",
    ),
    ("Licht für „{name}“ ist eine eigene Ebene — Änderungen hier rechnen sie neu.", "Light for “{name}” is a separate layer — changes here recompute it.", ""),
    ("Wirkt auf „{name}“ — als eigene Ebene, das Original bleibt.", "Acts on “{name}” — as a separate layer, the original stays.", ""),
    ("An der Figur wurde weitergemalt.", "The figure has changed since.", "Da Figur is weitagmoit wordn."),
    ("Neu berechnen", "Recompute", "Nei rechnen"),
    ("Alle sichtbaren Ebenen zusammenführen — in jedem Frame, auch Licht und Schatten; ausgeblendete bleiben", "Merge all visible layers — in every frame, light and shadow too; hidden ones stay", ""),
    ("Zusammengeführt", "Merged", "Zammgfiat"),
    // ── Ebenenmaske ──
    ("Maske hinzufügen — damit blendest du Teile der Ebene aus, ohne sie zu löschen", "Add a mask — hides parts of the layer without deleting them", "A Maskn dazua — damit blendst Teile vo da Ebene aus, ohne sie z’löschn"),
    ("Maske bearbeiten: Malen blendet aus, Radieren blendet wieder ein.", "Editing the mask: painting hides, erasing reveals again.", "Maskn bearbeitn: Moin blendt aus, Radiern blendt wieder ei."),
    ("Maske bearbeiten: Malen blendet aus, Radieren blendet wieder ein", "Edit the mask: painting hides, erasing reveals again", "Maskn bearbeitn: Moin blendt aus, Radiern blendt wieder ei"),
    ("Maske ausschalten (alles sichtbar)", "Turn the mask off (everything visible)", "Maskn ausschoitn (ois sichtbar)"),
    ("Maske einschalten", "Turn the mask on", "Maskn eischoitn"),
    ("Maske anwenden: ausgeblendete Pixel werden gelöscht, die Maske verschwindet", "Apply the mask: hidden pixels are deleted, the mask goes away", ""),
    ("Maske löschen — alles wieder sichtbar", "Delete the mask — everything visible again", "Maskn weg — ois wieder sichtbar"),
    // ── Werkzeuggröße per Alt + Rechts ──
    ("Größe {n}", "Size {n}", "Greß {n}"),
    ("Größe von Pinsel, Radierer und Spray — auch mit Alt + rechter Maustaste ziehen", "Size of brush, eraser and spray — or Alt + right-drag", "Greß vom Pinsl, Radiergummi und Spray — a mit Alt + rechta Maustastn ziehn"),
    ("Alt + Rechts ziehen", "Alt + right-drag", "Alt + Rechts ziehn"),
    ("Größe von Pinsel, Radierer und Spray", "Size of brush, eraser and spray", "Greß vom Pinsl, Radiergummi und Spray"),
    // ── Update-Hinweis ──
    ("Neue Version {new} verfügbar", "New version {new} available", "Neiche Version {new} do"),
    ("(du hast {old})", "(you have {old})", "(du host {old})"),
    ("Später", "Later", "Spätta"),
    ("Diese Version überspringen", "Skip this version", "De Version auslossn"),
    ("Beim Start nach Updates suchen", "Check for updates on start", "Beim Start noch Updates schaun"),
    (
        "Fragt beim Start einmal bei GitHub nach, ob es eine neuere Version gibt. Dabei wird nur die neueste Versionsnummer abgerufen — keine Daten aus deinen Projekten.",
        "Asks GitHub once on start whether a newer version exists. Only the latest version number is fetched — no data from your projects.",
        "Frogt beim Start amoi bei GitHub noch, obs a neichere Version gibt. Dabei wird nur de neieste Versionsnummer gholt — kane Daten aus deine Projekte.",
    ),
    // ── Über ──
    ("© 2026 Marco Jan · freie Software unter der MIT-Lizenz", "© 2026 Marco Jan · free software under the MIT license", "© 2026 Marco Jan · freie Software unta da MIT-Lizenz"),
    ("Neue Versionen auf GitHub", "New versions on GitHub", "Neiche Versionen auf GitHub"),
    // ── Reiter ──
    ("Reiter schließen (Mittelklick) — der Sprite bleibt im Projekt", "Close tab (middle click) — the sprite stays in the project", "Reiter zuamochn (Mittelklick) — da Sprite bleibt im Projekt"),
    ("Reiter schließen", "Close tab", "Reiter zuamochn"),
    ("Strg + Tab", "Ctrl + Tab", ""),
    ("Strg + W", "Ctrl + W", ""),
    ("Nächster Reiter (mit Umschalt: voriger)", "Next tab (with Shift: previous)", "Nächsta Reiter (mit Umschoit: voriga)"),
    // ── Clean Stroke (saubere 1-Pixel-Striche) ──
    ("Clean Stroke", "Clean Stroke", ""),
    ("Wie in bekannten Pixel-Art-Programmen: entfernt beim Zeichnen die doppelten Eckpixel an Treppenstufen — saubere 1-Pixel-Linien (Stift, Radierer mit Größe 1)", "As in well-known pixel art tools: removes the doubled corner pixels at stair steps while drawing — clean 1-pixel lines (pencil, eraser at size 1)", "Wia in bekanntn Pixel-Art-Programmen: haut beim Zeichnen de doppltn Eckpixel an de Stiagnstufn weg — saubere 1-Pixel-Linien (Stift, Radiergummi mit Greß 1)"),
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
            include_str!("tabs.rs"),
            include_str!("update.rs"),
            include_str!("sprites_ui.rs"),
            include_str!("preview_ui.rs"),
            include_str!("guides_ui.rs"),
            include_str!("template_ui.rs"),
            include_str!("tlmenu_ui.rs"),
            include_str!("view_ui.rs"),
        ];
        let mut missing = Vec::new();
        for src in sources {
            for call in ["tr(", "trf("] {
                for part in src.split(call).skip(1) {
                    // Der Text darf auch erst in der nächsten Zeile stehen.
                    let Some(part) = part.trim_start().strip_prefix('"') else { continue };
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
