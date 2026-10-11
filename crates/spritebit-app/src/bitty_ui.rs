//! Bitty — das Maskottchen als kleiner Helfer, wie in der Web-Version
//! (sprites-editor/js/bitty.js und helper.js).
//!
//! Er sitzt rechts in der Menüleiste und atmet. Ein Klick (oder Strg+K)
//! öffnet seine Blase: oben ein Suchfeld — in allen Sprachen zugleich —
//! über Hilfe, Touren, Tipps, Werkzeuge, Panels und Befehle. Ein Treffer
//! schlägt die Hilfe an der Stelle auf, startet die Tour, wählt das
//! Werkzeug, klappt das Panel auf oder führt den Befehl aus (nie einen, der
//! etwas löscht). Darunter ein Tipp, abwechselnd zur Bedienung und zum
//! Handwerk Pixel-Art; nach einer Weile kommt einer von selbst (abschaltbar).
//! Touren und die Lektion zum Mitmachen stehen in tour_ui.rs.
//!
//! Malt man und sähe nichts — gesperrte, ausgeblendete oder durchsichtige
//! Ebene, Farbe 0, eine Maske über der Stelle, lange im Masken-Modus —,
//! meldet er sich mit einem Knopf, der es behebt (einmal pro Sitzung,
//! abschaltbar). Beim ersten Export und der ersten
//! abgespielten Animation freut er sich, einmal im Leben. Nach fünf Minuten
//! ohne Eingabe döst er ein.
//!
//! Die Figur ist die aus dem Web: IDLE und PALETTE stammen aus js/bitty.js
//! — dort ändern und hier nachziehen. Ein Test prüft die Maße.

use std::collections::HashSet;

use eframe::egui::{self, Color32, Key, Modifiers, Rect, Sense, Stroke, Vec2};

use crate::i18n::{tr, variants};

/// Nummer → Rolle: 1 Kontur · 2 Schatten · 3 Grund · 4 Licht · 5 Glanz
const PALETTE: [Color32; 5] = [
    Color32::from_rgb(0x14, 0x21, 0x3d),
    Color32::from_rgb(0x2d, 0x48, 0x70),
    Color32::from_rgb(0x6e, 0xa8, 0xfe),
    Color32::from_rgb(0xa9, 0xd1, 0xff),
    Color32::from_rgb(0xf2, 0xf8, 0xff),
];
/// Bittys Blau — Rand der Hinweise und Markierung in der Hilfe.
pub(crate) const BLUE: Color32 = Color32::from_rgb(0x6e, 0xa8, 0xfe);

const E: &str = "................";
const IDLE: [&str; 16] = [
    E,
    E,
    ".....111111.....",
    "...1133333311...",
    "..133333333331..",
    "..134433333331..",
    ".13455433333331.",
    ".13344333333331.",
    ".13331331333331.",
    ".13331331333331.",
    ".13333333333331.",
    ".12333333333321.",
    ".12223333332221.",
    "..122222222221..",
    "...1111111111...",
    E,
];
/// Obere Augenzeile geschlossen
const CLOSED: &str = ".13333333333331.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pose {
    Idle,
    /// ausgeatmet: eine Zeile flacher, der Boden bleibt
    Squish,
    Blink,
    /// in der Luft
    Hop,
    /// ausgeatmet mit geschlossenen Augen (Schlafen)
    Doze,
}

fn rows(p: Pose) -> Vec<&'static str> {
    let squish = || {
        let mut v = vec![E];
        v.extend_from_slice(&IDLE[..10]);
        v.extend_from_slice(&IDLE[11..]);
        v
    };
    match p {
        Pose::Idle => IDLE.to_vec(),
        Pose::Squish => squish(),
        Pose::Blink => {
            let mut v = IDLE.to_vec();
            v[8] = CLOSED;
            v
        }
        Pose::Hop => {
            let mut v = IDLE[2..].to_vec();
            v.extend([E, E]);
            v
        }
        Pose::Doze => {
            let mut v = squish();
            v[9] = CLOSED;
            v
        }
    }
}

/// Bitty in `rect` malen (16 × 16 Zellen).
fn paint(painter: &egui::Painter, rect: Rect, p: Pose) {
    let cell = rect.width() / 16.0;
    for (y, row) in rows(p).iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            if c == b'.' {
                continue;
            }
            let min = rect.min + Vec2::new(x as f32 * cell, y as f32 * cell);
            painter.rect_filled(Rect::from_min_size(min, Vec2::splat(cell)), 0.0, PALETTE[(c - b'1') as usize]);
        }
    }
}

// ── Zustand ─────────────────────────────────────────────────────────

/// Behebt, worüber Bitty gerade stolpert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fix {
    Unlock(usize),
    Show(usize),
    /// Deckkraft der Ebene auf 100 %.
    Opacity(usize),
    /// Mit Farbe 1 malen statt mit der durchsichtigen 0.
    Color1,
    /// Maske der Ebene ausschalten.
    MaskOff(usize),
    /// Maske nicht mehr bearbeiten — zurück zum Bild.
    LeaveMask,
}

pub(crate) struct Hint {
    id: &'static str,
    text: String,
    fix: Option<Fix>,
}

/// Was ein Treffer der Suche tut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Go {
    /// Die Zeile der Hilfe aufschlagen (so, wie sie dort steht).
    Help(&'static str),
    Guide(crate::tour_ui::Guide),
    Tip(usize),
    Tool(crate::tools_ui::Tool),
    Panel(crate::dock_ui::PanelId),
    Cmd(Cmd),
}

/// Befehle, die Bitty selbst ausführt — wie im Web, wo er Knöpfe drückt.
/// Nichts davon löscht etwas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cmd {
    Save,
    SaveAs,
    Export,
    ExportGif,
    ExportWeb,
    SaveAse,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    SelectAll,
    Deselect,
    NewSprite,
    NewLayer,
    NewFrame,
    DupFrame,
    Play,
    Onion,
    Guides,
    Fit,
    Fullscreen,
    Help,
}

impl Cmd {
    const ALL: [Cmd; 23] = [
        Cmd::Save,
        Cmd::SaveAs,
        Cmd::Export,
        Cmd::ExportGif,
        Cmd::ExportWeb,
        Cmd::SaveAse,
        Cmd::Undo,
        Cmd::Redo,
        Cmd::Copy,
        Cmd::Cut,
        Cmd::Paste,
        Cmd::SelectAll,
        Cmd::Deselect,
        Cmd::NewSprite,
        Cmd::NewLayer,
        Cmd::NewFrame,
        Cmd::DupFrame,
        Cmd::Play,
        Cmd::Onion,
        Cmd::Guides,
        Cmd::Fit,
        Cmd::Fullscreen,
        Cmd::Help,
    ];

    /// Name (deutsch = Schlüssel) und Ort, wie im Menü bzw. an der Leiste.
    fn label(self) -> (&'static str, &'static str) {
        match self {
            Cmd::Save => (tr("Projekt speichern"), tr("Datei")),
            Cmd::SaveAs => (tr("Projekt speichern unter …"), tr("Datei")),
            Cmd::Export => (tr("Exportieren …"), tr("Datei")),
            Cmd::ExportGif => (tr("Als GIF exportieren"), tr("Code & Export")),
            Cmd::ExportWeb => (tr("Als Web-Projekt exportieren …"), tr("Datei")),
            Cmd::SaveAse => (tr("Sprite als Aseprite-Datei speichern"), tr("Datei")),
            Cmd::Undo => (tr("Rückgängig"), tr("Bearbeiten")),
            Cmd::Redo => (tr("Wiederholen"), tr("Bearbeiten")),
            Cmd::Copy => (tr("Kopieren"), tr("Bearbeiten")),
            Cmd::Cut => (tr("Ausschneiden"), tr("Bearbeiten")),
            Cmd::Paste => (tr("Einfügen"), tr("Bearbeiten")),
            Cmd::SelectAll => (tr("Alles auswählen"), tr("Bearbeiten")),
            Cmd::Deselect => (tr("Auswahl aufheben"), tr("Bearbeiten")),
            Cmd::NewSprite => (tr("Neuer Sprite …"), tr("Sprites")),
            Cmd::NewLayer => (tr("Neue Ebene über der aktiven"), tr("Ebenen")),
            Cmd::NewFrame => (tr("Leerer Frame dahinter"), tr("Timeline")),
            Cmd::DupFrame => (tr("Frame duplizieren"), tr("Timeline")),
            Cmd::Play => (tr("Abspielen / Anhalten (Enter)"), tr("Timeline")),
            Cmd::Onion => (tr("Onion Skin ein / aus"), tr("Timeline")),
            Cmd::Guides => (tr("Hilfslinien ein / aus"), tr("Ansicht")),
            Cmd::Fit => (tr("Einpassen"), tr("Ansicht")),
            Cmd::Fullscreen => (tr("Vollbild"), tr("Ansicht")),
            Cmd::Help => (tr("Hilfe und Tastenkürzel"), tr("Hilfe")),
        }
    }
}

/// Ein Treffer der Suche: Art, Name, Ort und alles, was gefunden werden darf.
#[derive(Clone)]
struct Entry {
    /// „Hilfe“, „Tour“, „Werkzeug“ … — klein vor dem Namen.
    kind: &'static str,
    label: String,
    sub: String,
    /// Name in allen Sprachen (zählt wie `label`)
    names: String,
    text: String,
    go: Go,
}

const IDLE_SECS: f64 = 5.0 * 60.0;
/// Nach so vielen Sekunden kommt (einmal pro Sitzung) ein Tipp von selbst …
const AUTO_TIP_AFTER: f64 = 20.0;
/// … und bleibt so lange stehen.
const AUTO_TIP_SHOW: f64 = 12.0;

pub(crate) struct BittyState {
    pub(crate) open: bool,
    pub(crate) query: String,
    focus_query: bool,
    sel: usize,
    tip: usize,
    pub(crate) hint: Option<Hint>,
    /// Hinweise, die in dieser Sitzung schon kamen.
    shown: HashSet<&'static str>,
    hints_on: bool,
    off: HashSet<String>,
    moments: HashSet<String>,
    last_input: f64,
    hop_until: f64,
    pub(crate) bubble: Rect,
    pub(crate) button: Rect,
    /// Wo Tour-Ziele in diesem Bild liegen (tour_ui.rs `mark`) — wird zu
    /// Beginn jedes Bildes geleert und von den Panels neu gefüllt.
    pub(crate) targets: std::collections::HashMap<&'static str, Rect>,
    /// Laufende Tour (tour_ui.rs) und ob die erste schon angeboten wurde.
    pub(crate) guide: Option<crate::tour_ui::Guide>,
    pub(crate) tour_done: bool,
    /// Größe der Tour-Blase im letzten Bild (zum Platzieren).
    pub(crate) guide_size: Vec2,
    /// So viele Bilder nach einem Schrittwechsel noch neu zeichnen.
    pub(crate) guide_settle: u8,
    /// Tipps nicht von selbst zeigen („Nicht von selbst“).
    pub(crate) quiet: bool,
    /// Der Tipp kam von selbst: bis wann er stehen bleibt.
    pub(crate) auto_close: Option<f64>,
    /// Ob in dieser Sitzung schon ein Tipp von selbst kam.
    auto_shown: bool,
    /// Seit wann (egui-Zeit) in die Maske gemalt wird — nach einer Minute fragt Bitty nach.
    mask_since: Option<f64>,
    /// Der Hinweis kam eben von einem Klick auf die Fläche — derselbe Klick
    /// darf die Blase nicht gleich als „daneben“ wieder schließen.
    just_shown: bool,
    /// Lektion: Stand beim Beginn des Schritts, „gut gemacht“ bis wann, GIF exportiert.
    pub(crate) lesson_start: u64,
    pub(crate) lesson_yay: Option<f64>,
    pub(crate) gif_clicked: bool,
    /// Zeile der Hilfe, die gerade blau umrandet wird, bis wann, und ob schon hingescrollt.
    pub(crate) help_focus: Option<(&'static str, f64, bool)>,
}

impl Default for BittyState {
    fn default() -> Self {
        let mut s = Self {
            open: false,
            query: String::new(),
            focus_query: false,
            sel: 0,
            tip: 0,
            hint: None,
            shown: HashSet::new(),
            hints_on: true,
            off: HashSet::new(),
            moments: HashSet::new(),
            last_input: 0.0,
            hop_until: 0.0,
            bubble: Rect::NOTHING,
            button: Rect::NOTHING,
            targets: std::collections::HashMap::new(),
            guide: None,
            // In Tests nie von selbst eine Tour — sie läge über allem.
            tour_done: cfg!(test),
            guide_size: Vec2::ZERO,
            guide_settle: 0,
            // In Tests nie von selbst — sonst ploppte mitten im Test ein Tipp auf.
            quiet: cfg!(test),
            auto_close: None,
            auto_shown: false,
            mask_since: None,
            just_shown: false,
            lesson_start: 0,
            lesson_yay: None,
            gif_clicked: false,
            help_focus: None,
        };
        s.load();
        s
    }
}

// Vorlieben: kleine Textdatei im Einstellungsordner, je Zeile „schlüssel=wert“.
// In Tests nie — die liefen sonst gegen die echten Einstellungen.
impl BittyState {
    #[cfg(not(test))]
    fn file() -> Option<std::path::PathBuf> {
        Some(crate::i18n::settings_dir()?.join("bitty"))
    }

    fn load(&mut self) {
        #[cfg(not(test))]
        if let Some(text) = Self::file().and_then(|p| std::fs::read_to_string(p).ok()) {
            for line in text.lines() {
                match line.split_once('=') {
                    Some(("hints", v)) => self.hints_on = v != "0",
                    Some(("off", v)) => {
                        self.off.insert(v.to_string());
                    }
                    Some(("moment", v)) => {
                        self.moments.insert(v.to_string());
                    }
                    Some(("tip", v)) => self.tip = v.parse().unwrap_or(0),
                    Some(("tour", v)) => self.tour_done = v != "0",
                    Some(("quiet", v)) => self.quiet = v != "0",
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn save(&self) {
        #[cfg(not(test))]
        if let Some(p) = Self::file() {
            let mut out = format!(
                "hints={}\ntip={}\ntour={}\nquiet={}\n",
                if self.hints_on { 1 } else { 0 },
                self.tip,
                if self.tour_done { 1 } else { 0 },
                if self.quiet { 1 } else { 0 }
            );
            for o in &self.off {
                out += &format!("off={o}\n");
            }
            for m in &self.moments {
                out += &format!("moment={m}\n");
            }
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, out);
        }
    }

    /// Welcher Hinweis gerade in der Blase steht (für Tests).
    #[cfg(test)]
    pub(crate) fn hint_id(&self) -> Option<&'static str> {
        self.hint.as_ref().filter(|_| self.open).map(|h| h.id)
    }

    pub(crate) fn hop(&mut self, now: f64) {
        self.hop_until = now + 0.6;
    }

    /// Welche Haltung jetzt, und wie lange sie noch steht (Sekunden).
    fn pose(&self, now: f64) -> (Pose, f64) {
        if now < self.hop_until {
            let k = ((self.hop_until - now) / 0.15) as i32;
            return (if k % 2 == 0 { Pose::Hop } else { Pose::Squish }, 0.05);
        }
        if now - self.last_input > IDLE_SECS {
            let ph = now % 3.2;
            return if ph < 1.6 { (Pose::Blink, 1.6 - ph) } else { (Pose::Doze, 3.2 - ph) };
        }
        let b = now % 4.7;
        if b < 0.14 {
            return (Pose::Blink, 0.14 - b);
        }
        let p = now % 1.5;
        let (pose, left) = if p < 0.9 { (Pose::Idle, 0.9 - p) } else { (Pose::Squish, 1.5 - p) };
        (pose, left.min(4.7 - b))
    }
}

// ── Tipps ───────────────────────────────────────────────────────────

/// Tipps zur Bedienung und zum Handwerk Pixel-Art, im Wechsel — damit
/// nicht erst zehnmal Tastenkürzel kommen (wie im Web). `true` = Pixel-Art.
fn tips() -> Vec<(&'static str, bool)> {
    let app = [
        tr("Leertaste halten und ziehen verschiebt das Bild — auch mit der mittleren Maustaste."),
        tr("Strg + Mausrad zoomt genau dorthin, wo dein Mauszeiger steht."),
        tr("Umschalt halten beim Malen: der Strich bleibt waagerecht, senkrecht oder im 45°-Winkel."),
        tr("Alt + rechte Maustaste ziehen verstellt die Pinselgröße direkt auf der Fläche."),
        tr("Dein Bild merkt sich Farbnummern, nicht Farben. Mit „Sprite umfärben“ im Paletten-Panel bekommt es auf einen Schlag andere Farben."),
        tr("Onion Skin in der Timeline lässt die Nachbar-Frames durchscheinen — praktisch beim Animieren."),
        tr("Eine Auswahl mit Alt ziehen lässt das Original stehen — du verschiebst eine Kopie."),
        tr("G blendet die Hilfslinien ein und aus. In Exporten tauchen sie nie auf."),
        tr("Strg + S sichert dein ganzes Projekt als Datei — gut als Backup oder für einen anderen Rechner."),
        tr("F1 öffnet die Hilfe mit allen Tastenkürzeln."),
        tr("Strg + K öffnet mich überall. Tipp „neue ebene“ oder „gif“ und drück Enter — ich führe es gleich aus."),
    ];
    let art = [
        tr("Erst die Silhouette: füll deine Figur einfarbig aus. Erkennt man sie dann noch, stimmt die Form."),
        tr("Licht kommt am besten von einer Seite, meist oben links — und zwar überall im Bild gleich."),
        tr("Schatten nicht nur dunkler machen, sondern auch kühler (Richtung Blau/Violett); Licht wärmer (Richtung Gelb). Das wirkt lebendiger."),
        tr("Weniger Farben wirken geschlossener: 4–8 pro Sprite reichen oft."),
        tr("Saubere Linien steigen in gleichmäßigen Stufen (1-1-1 oder 2-2-2). Clean Stroke entfernt die doppelten Eckpixel."),
        tr("Vorsicht vor „Kissen-Schattierung“: Schatten ringsum am Rand macht alles flach. Schattiere von der Lichtrichtung weg."),
        tr("Die Kontur muss nicht schwarz sein — ein dunkler Ton der Füllfarbe wirkt weicher und passt besser."),
        tr("Den höchsten Kontrast hebst du dir für das Wichtigste auf, meist die Augen."),
        tr("Für eine Animation reichen oft 2–4 Frames. Ein Frame, der länger steht, gibt Gewicht — z. B. beim Landen."),
        tr("Gleich breite Farbstreifen parallel zur Kontur („Banding“) wirken matschig. Versetz die Übergänge lieber."),
    ];
    let mut out = Vec::new();
    for i in 0..app.len().max(art.len()) {
        if let Some(t) = app.get(i) {
            out.push((*t, false));
        }
        if let Some(t) = art.get(i) {
            out.push((*t, true));
        }
    }
    out
}

// ── Suche ───────────────────────────────────────────────────────────
// Wie js/search.js im Web: verzeiht Groß/klein, Umlaute, Wortanfänge,
// einen Tippfehler im Namen und kennt ein paar Alltagswörter.

pub(crate) fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.to_lowercase().chars() {
        match c {
            'ä' | 'à' | 'á' | 'â' => out.push('a'),
            'ö' | 'ò' | 'ó' | 'ô' => out.push('o'),
            'ü' | 'ù' | 'ú' | 'û' => out.push('u'),
            'é' | 'è' | 'ê' => out.push('e'),
            'ß' => out += "ss", // nicht push_str: der Übersetzungs-Test liest jedes „tr(“
            c if c.is_ascii_alphanumeric() => out.push(c),
            _ => out.push(' '),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Höchstens ein Tippfehler (ersetzt, fehlt, zu viel, zwei vertauscht)?
fn one_edit(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a == b {
        return true;
    }
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    if a.len() == b.len() {
        let k = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
        if k + 1 < a.len() && a[k] == b[k + 1] && a[k + 1] == b[k] && a[k + 2..] == b[k + 2..] {
            return true;
        }
    }
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        edits += 1;
        if edits > 1 {
            return false;
        }
        if a.len() > b.len() {
            i += 1;
        } else if b.len() > a.len() {
            j += 1;
        } else {
            i += 1;
            j += 1;
        }
    }
    edits + (a.len() - i) + (b.len() - j) <= 1
}

fn word_score(q: &str, list: &[&str], typos: bool) -> u32 {
    let mut best = 0;
    for w in list {
        if *w == q {
            return 3;
        }
        let n = q.chars().count();
        let typo = typos && n >= 5 && (one_edit(q, w) || one_edit(q, &w.chars().take(n).collect::<String>()));
        if w.starts_with(q) {
            best = best.max(2);
        } else if typo || (n >= 4 && w.contains(q)) {
            best = best.max(1);
        }
    }
    best
}

/// Alltagswörter → Wörter, die wirklich in der Hilfe stehen (normalisiert).
const SYNONYMS: &[(&str, &[&str])] = &[
    ("radiergummi", &["radierer", "eraser"]),
    ("gummi", &["radierer", "eraser"]),
    ("eimer", &["fullen", "fill"]),
    ("farbeimer", &["fullen", "fill"]),
    ("bucket", &["fill", "fullen"]),
    ("pipette", &["pipette", "eyedropper", "farbwahl"]),
    ("spiegeln", &["symmetrie", "mirror"]),
    ("layer", &["ebene", "ebenen"]),
    ("ebene", &["layer", "layers"]),
    ("animation", &["frames", "frame", "timeline"]),
    ("speichern", &["sichern", "export", "exportieren"]),
    ("save", &["export", "speichern"]),
    ("loschen", &["radierer", "entf", "delete"]),
    ("durchsichtig", &["transparent"]),
    ("kachel", &["kacheln", "tiles", "tilemap"]),
    ("foto", &["schablone", "photo", "template"]),
    ("vorlage", &["schablone", "template"]),
    ("raster", &["gitter", "grid", "hilfslinien"]),
];

fn variants_of(q: &str) -> Vec<String> {
    let mut out = vec![q.to_string()];
    for (k, vs) in SYNONYMS {
        if *k == q || (q.chars().count() >= 6 && k.starts_with(q)) {
            out.extend(vs.iter().map(|s| s.to_string()));
        }
    }
    out
}

/// Einträge bewerten; jedes Suchwort muss passen. Bessere zuerst.
fn search<'a>(query: &str, entries: &'a [Entry], limit: usize) -> Vec<&'a Entry> {
    let q = normalize(query);
    let qs: Vec<&str> = q.split(' ').filter(|s| !s.is_empty()).collect();
    if qs.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(f32, usize, &Entry)> = Vec::new();
    'entry: for (idx, e) in entries.iter().enumerate() {
        let lw_s = normalize(&format!("{} {}", e.label, e.names));
        let tw_s = normalize(&e.text);
        let lw: Vec<&str> = lw_s.split(' ').collect();
        let tw: Vec<&str> = tw_s.split(' ').collect();
        let mut total = 0.0;
        for q in &qs {
            let mut best = 0.0f32;
            for v in variants_of(q) {
                let s = (word_score(&v, &lw, v == *q) * 3 + word_score(&v, &tw, false)) as f32;
                best = best.max(if v == *q { s } else { s * 0.9 });
            }
            if best == 0.0 {
                continue 'entry;
            }
            total += best;
        }
        // Wer „neue ebene“ tippt, will meist den Befehl, nicht den Tipp, in
        // dem die Worte zufällig stehen: Dinge zum Tun vor Text zum Lesen.
        let weight = match e.go {
            Go::Cmd(_) | Go::Tool(_) | Go::Panel(_) | Go::Guide(_) => 1.25,
            Go::Help(_) => 1.0,
            Go::Tip(_) => 0.8,
        };
        // Heißt der Eintrag genau so, wie getippt wurde, steht er vorn.
        let exact = if normalize(&e.label) == q { 2.0 } else { 1.0 };
        scored.push((total * weight * exact, idx, e));
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().take(limit).map(|s| s.2).collect()
}

/// Alles, was die Suche finden kann: Touren, Tipps, Werkzeuge, Panels,
/// Befehle, jede Zeile der Hilfe und jedes Kürzel.
fn entries() -> Vec<Entry> {
    use crate::tour_ui::{Guide, TourId};
    let all = |s: &str| variants(s).join(" ");
    let mut out = Vec::new();
    // Touren — Stichwörter stehen in beiden Sprachen in `words`.
    out.push(Entry {
        kind: tr("Tour"),
        label: tr("Mitmachen: erster animierter Sprite").into(),
        sub: String::new(),
        names: all("Mitmachen: erster animierter Sprite"),
        text: "Lektion Mitmachen üben lernen Anfänger Tutorial erster animierter Sprite lesson tutorial beginner learn first animated".into(),
        go: Go::Guide(Guide::LessonHello),
    });
    for t in [TourId::Start, TourId::Anim, TourId::Layers, TourId::Tiles, TourId::Photo] {
        out.push(Entry {
            kind: tr("Tour"),
            label: t.name().into(),
            sub: String::new(),
            names: String::new(),
            text: t.words().into(),
            go: Go::Guide(Guide::Tour(t, 0)),
        });
    }
    // Tipps — vor allem die zum Handwerk („schatten“, „kontur“).
    for (k, (t, _)) in tips().into_iter().enumerate() {
        let label = if t.chars().count() > 60 { t.chars().take(57).collect::<String>() + " …" } else { t.to_string() };
        out.push(Entry { kind: tr("Tipp"), label, sub: String::new(), names: String::new(), text: all(t), go: Go::Tip(k) });
    }
    // Werkzeuge: Name und Kürzel
    for t in crate::tools_ui::Tool::ALL {
        out.push(Entry {
            kind: tr("Werkzeug"),
            label: t.label().into(),
            sub: format!("{:?}", t.key()),
            names: all(t.label()),
            text: String::new(),
            go: Go::Tool(t),
        });
    }
    // Panels
    for id in crate::dock_ui::PanelId::ALL {
        out.push(Entry { kind: tr("Panel"), label: id.title().into(), sub: String::new(), names: all(id.title()), text: String::new(), go: Go::Panel(id) });
    }
    // Befehle
    for c in Cmd::ALL {
        let (label, sub) = c.label();
        out.push(Entry { kind: tr("Befehl"), label: label.into(), sub: sub.into(), names: all(label), text: all(sub), go: Go::Cmd(c) });
    }
    for (title, lines) in crate::view_ui::help_sections() {
        for l in lines {
            // Das erste Wort ist meist das Stichwort („Stift — …“, „Tags …“) und zählt wie ein Name.
            let head = l.split([' ', '.']).next().unwrap_or(l);
            let label: String = if l.chars().count() > 60 { l.chars().take(57).collect::<String>() + " …" } else { l.to_string() };
            out.push(Entry {
                kind: tr("Hilfe"),
                label,
                sub: title.to_string(),
                names: format!("{head} {}", all(title)),
                text: format!("{} {}", all(l), all(title)),
                go: Go::Help(l),
            });
        }
    }
    for (k, what) in crate::view_ui::help_keys() {
        out.push(Entry {
            kind: tr("Hilfe"),
            label: what.to_string(),
            sub: format!("{} · {k}", tr("Tastenkürzel")),
            names: all(what),
            text: format!("{k} {}", all(k)),
            go: Go::Help(what),
        });
    }
    out
}

// ── Oberfläche ──────────────────────────────────────────────────────

impl crate::SpritebitApp {
    /// Bitty in der Menüleiste. Klick öffnet bzw. schließt die Blase.
    pub(crate) fn bitty_button(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);
        if ui.input(|i| i.pointer.is_moving() || !i.events.is_empty()) || self.bitty.last_input == 0.0 {
            self.bitty.last_input = now;
        }
        let (rect, r) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
        let r = r.on_hover_text(tr("Bitty — Suche und Tipps (Strg+K)"));
        if r.hovered() || self.bitty.open {
            ui.painter().rect_filled(rect.expand(2.0), 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let (pose, left) = self.bitty.pose(now);
        paint(ui.painter(), rect.shrink(1.0), pose);
        self.bitty.button = rect;
        // Nur so oft neu zeichnen, wie Bitty die Haltung wechselt. In Tests
        // gar nicht — dort soll jeder Durchlauf zur Ruhe kommen.
        #[cfg(not(test))]
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(left.max(0.02)));
        #[cfg(test)]
        let _ = left;
        if r.clicked() && self.bitty.guide.is_none() {
            self.bitty.auto_close = None;
            self.bitty.open = !self.bitty.open;
            self.bitty.hint = None;
            self.bitty.focus_query = self.bitty.open;
            self.bitty.hop(now);
        }
    }

    /// Strg+K öffnet die Suche, Esc schließt die Blase.
    pub(crate) fn bitty_keys(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K)) {
            if self.bitty.guide.is_some() {
                return;
            }
            self.bitty.auto_close = None;
            self.bitty.open = true;
            self.bitty.hint = None;
            self.bitty.focus_query = true;
            let now = ctx.input(|i| i.time);
            self.bitty.hop(now);
        }
        if self.bitty.open && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.bitty.open = false;
        }
    }

    /// Hinweis in einer Sackgasse — höchstens einmal pro Sitzung.
    pub(crate) fn bitty_hint(&mut self, id: &'static str, text: String, fix: Option<Fix>) {
        let b = &mut self.bitty;
        if !b.hints_on || b.off.contains(id) || b.shown.contains(id) || (b.open && b.hint.is_none()) {
            return;
        }
        b.shown.insert(id);
        b.just_shown = true;
        b.hint = Some(Hint { id, text, fix });
        b.open = true;
        b.focus_query = false;
        b.hop_until = b.last_input + 0.6;
    }

    /// Man setzt zum Malen an und sieht vielleicht nichts — weil die Ebene
    /// 0 % Deckkraft hat, die Farbe durchsichtig ist oder die Maske die Stelle
    /// ausblendet. Dann meldet sich Bitty mit dem Grund und einem Knopf, der
    /// es behebt (wie im Web). Gesperrt und ausgeblendet prüft `layer_ok`.
    pub(crate) fn paint_hints(&mut self, cell: (i64, i64), erase: bool, now: f64) {
        use crate::i18n::trf;
        use crate::tools_ui::Tool;
        let sp = self.project.sprite();
        let l = sp.layer;
        let layer = &sp.layers[l];
        let name = layer.name.clone();
        if sp.editing_mask() {
            // Lange im Masken-Modus und wieder am Malen: vielleicht vergessen?
            match self.bitty.mask_since {
                None => self.bitty.mask_since = Some(now),
                Some(t) if now - t > 60.0 => self.bitty_hint(
                    "maskEdit",
                    trf("Du malst gerade in die Maske von „{name}“, nicht ins Bild. Absicht?", &[("name", &name)]),
                    Some(Fix::LeaveMask),
                ),
                Some(_) => {}
            }
            return;
        }
        self.bitty.mask_since = None;
        if layer.opacity <= 0.0 {
            self.bitty_hint(
                "layerOpacity",
                trf("Die Ebene „{name}“ hat 0 % Deckkraft — was du malst, bleibt unsichtbar.", &[("name", &name)]),
                Some(Fix::Opacity(l)),
            );
            return;
        }
        let paints = matches!(self.tool, Tool::Pencil | Tool::Brush | Tool::Spray | Tool::Fill | Tool::Line | Tool::Rect | Tool::Ellipse);
        if paints && !erase && self.color == 0 {
            self.bitty_hint("color0", tr("Du malst mit Farbe 0 — die ist durchsichtig und wirkt wie ein Radierer.").into(), Some(Fix::Color1));
            return;
        }
        let masked = layer.mask.as_ref().is_some_and(|m| m.on && cell.0 >= 0 && cell.1 >= 0 && m.hides(cell.0 as u32, cell.1 as u32));
        if masked {
            self.bitty_hint(
                "masked",
                trf("Hier blendet die Maske von „{name}“ die Ebene aus — deshalb siehst du nichts.", &[("name", &name)]),
                Some(Fix::MaskOff(l)),
            );
        }
    }

    /// Einmal im Leben freut sich Bitty — beim ersten Export, der ersten Animation.
    pub(crate) fn bitty_moment(&mut self, id: &str, text: &'static str) {
        let b = &mut self.bitty;
        b.hop_until = b.last_input + 0.6;
        if b.moments.contains(id) || b.open {
            return;
        }
        b.moments.insert(id.to_string());
        b.save();
        b.hint = Some(Hint { id: "moment", text: text.to_string(), fix: None });
        b.open = true;
        b.focus_query = false;
    }

    /// Menüpunkt „Hinweise von Bitty“ (Hilfe-Menü).
    pub(crate) fn bitty_menu(&mut self, ui: &mut egui::Ui) {
        if ui.checkbox(&mut self.bitty.hints_on, tr("Hinweise von Bitty")).changed() {
            if self.bitty.hints_on {
                self.bitty.off.clear();
            }
            self.bitty.save();
        }
    }

    /// Die Blase unter Bitty.
    pub(crate) fn bitty_bubble(&mut self, ctx: &egui::Context) {
        // Eine Tour hat Vorrang (tour_ui.rs).
        if self.guide_bubble(ctx) {
            return;
        }
        let now = ctx.input(|i| i.time);
        // Ab und zu ein Tipp von selbst: einmal pro Sitzung nach AUTO_TIP_AFTER
        // Sekunden — nicht, wenn gerade etwas offen ist oder Bitty ruhig sein soll.
        if !self.bitty.quiet && !self.bitty.auto_shown && self.bitty.tour_done && !self.bitty.open {
            if now >= AUTO_TIP_AFTER && !self.view.help_open {
                self.bitty.auto_shown = true;
                self.bitty.tip = (self.bitty.tip + 1) % tips().len();
                self.bitty.save();
                self.bitty.open = true;
                self.bitty.focus_query = false;
                self.bitty.auto_close = Some(now + AUTO_TIP_SHOW);
                self.bitty.hop(now);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64((AUTO_TIP_AFTER - now).max(1.0)));
            }
        }
        if let Some(t) = self.bitty.auto_close {
            if now >= t {
                self.bitty.open = false;
                self.bitty.auto_close = None;
            } else if !self.bitty.query.is_empty() {
                // Wer tippt, liest — nicht mehr von selbst schließen.
                self.bitty.auto_close = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(t - now));
            }
        }
        if !self.bitty.open {
            return;
        }
        // Klick daneben schließt (nicht auf Bitty selbst — der schaltet um).
        if std::mem::take(&mut self.bitty.just_shown) {
            // Derselbe Klick hat den Hinweis eben ausgelöst.
        } else if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                if !self.bitty.bubble.contains(p) && !self.bitty.button.contains(p) && self.bitty.bubble != Rect::NOTHING {
                    self.bitty.open = false;
                    return;
                }
            }
        }
        let width = 330.0;
        let pos = egui::pos2((self.bitty.button.right() - width).max(8.0), self.bitty.button.bottom() + 8.0);
        let mut fix_now = None;
        let mut go: Option<Go> = None;
        let mut close = false;
        let mut off: Option<&'static str> = None;
        let hint = self.bitty.hint.as_ref().map(|h| (h.id, h.text.clone(), h.fix));
        let is_hint = hint.is_some();
        let resp = egui::Area::new(egui::Id::new("bitty-bubble")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            let mut frame = egui::Frame::popup(ui.style());
            if is_hint {
                frame = frame.stroke(Stroke::new(1.5, BLUE));
            }
            frame.show(ui, |ui| {
                ui.set_width(width - 16.0);
                if let Some((id, text, fix)) = &hint {
                    // Hinweis oder Freude: Text, Knopf zum Beheben, „nicht mehr“.
                    ui.label(text);
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        if let Some(f) = fix {
                            let label = match f {
                                Fix::Unlock(_) => tr("Entsperren"),
                                Fix::Show(_) => tr("Einblenden"),
                                Fix::Opacity(_) => tr("Auf 100 % stellen"),
                                Fix::Color1 => tr("Farbe 1 nehmen"),
                                Fix::MaskOff(_) => tr("Maske ausschalten"),
                                Fix::LeaveMask => tr("Zurück zum Bild"),
                            };
                            if ui.button(label).clicked() {
                                fix_now = Some(*f);
                            }
                        }
                        if *id == "moment" {
                            if ui.button(tr("Danke")).clicked() {
                                close = true;
                            }
                        } else if ui.button(tr("Nicht mehr zeigen")).clicked() {
                            off = Some(id);
                            close = true;
                        }
                    });
                    return;
                }
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.bitty.query).hint_text(tr("Wonach suchst du? (z. B. Lasso, Ebenen)")).desired_width(f32::INFINITY),
                );
                if self.bitty.focus_query {
                    r.request_focus();
                    self.bitty.focus_query = false;
                }
                if r.changed() {
                    self.bitty.sel = 0;
                }
                let q = self.bitty.query.trim().to_string();
                if !q.is_empty() {
                    let all = entries();
                    let hits = search(&q, &all, 8);
                    if hits.is_empty() {
                        ui.add_space(4.0);
                        ui.weak(tr("Dazu finde ich nichts — versuch ein anderes Wort."));
                    }
                    let n = hits.len();
                    if n > 0 && r.has_focus() {
                        if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                            self.bitty.sel = (self.bitty.sel + 1) % n;
                        }
                        if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                            self.bitty.sel = (self.bitty.sel + n - 1) % n;
                        }
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        if let Some(h) = hits.get(self.bitty.sel.min(n.saturating_sub(1))) {
                            go = Some(h.go);
                        }
                    }
                    ui.add_space(4.0);
                    // Dasselbe gibt es oft zweimal (Befehl „Rückgängig“ und die Zeile der Hilfe) — einmal reicht.
                    let mut seen = HashSet::new();
                    for (i, h) in hits.iter().enumerate() {
                        if !seen.insert(h.label.to_lowercase()) {
                            continue;
                        }
                        let r = ui
                            .horizontal(|ui| {
                                ui.weak(h.kind);
                                ui.add(egui::Button::selectable(i == self.bitty.sel, h.label.as_str()))
                            })
                            .inner
                            .on_hover_text(&h.sub);
                        if !h.sub.is_empty() && h.sub != h.label {
                            ui.weak(&h.sub);
                        }
                        if r.clicked() {
                            go = Some(h.go);
                        }
                    }
                } else {
                    let tips = tips();
                    let (tip, art) = tips[self.bitty.tip % tips.len()];
                    ui.add_space(6.0);
                    // Pixel-Art-Tipps sind eine andere Sorte — kleine Überschrift.
                    if art {
                        ui.weak(tr("Pixel-Art-Tipp"));
                    }
                    ui.label(tip);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::selectable(true, tr("Nächster Tipp"))).clicked() {
                            self.bitty.tip = (self.bitty.tip + 1) % tips.len();
                            self.bitty.auto_close = None;
                            self.bitty.save();
                        }
                        if self.bitty.auto_close.is_some() {
                            // Von selbst gekommen: abschaltbar.
                            if ui.button(tr("Nicht von selbst")).clicked() {
                                self.bitty.quiet = true;
                                self.bitty.save();
                                close = true;
                            }
                        } else if ui.button(tr("Schließen")).clicked() {
                            close = true;
                        }
                    });
                }
            });
        });
        self.bitty.bubble = resp.response.rect;
        if let Some(id) = off {
            self.bitty.off.insert(id.to_string());
            self.bitty.save();
        }
        if let Some(f) = fix_now {
            match f {
                Fix::Unlock(l) if l < self.sprite().layers.len() => self.edit_sprite(|s| s.layers[l].locked = false),
                Fix::Show(l) if l < self.sprite().layers.len() => self.edit_sprite(|s| s.layers[l].visible = true),
                Fix::Opacity(l) if l < self.sprite().layers.len() => self.edit_sprite(|s| s.layers[l].opacity = 1.0),
                Fix::Color1 => self.color = 1,
                Fix::MaskOff(l) if l < self.sprite().layers.len() => self.edit_sprite(|s| {
                    if let Some(m) = s.layers[l].mask.as_mut() {
                        m.on = false;
                    }
                }),
                Fix::LeaveMask => {
                    self.project.sprite_mut().editing_mask = false;
                    self.changed();
                }
                _ => {}
            }
            self.hint = None;
            close = true;
        }
        if let Some(g) = go {
            close = !matches!(g, Go::Tip(_));
            self.bitty_go(g, ctx);
        }
        if close {
            self.bitty.open = false;
            self.bitty.hint = None;
            self.bitty.auto_close = None;
            self.bitty.query.clear();
        }
    }

    /// Zum Treffer der Suche hinführen.
    fn bitty_go(&mut self, g: Go, ctx: &egui::Context) {
        match g {
            Go::Help(target) => {
                // Hilfe aufschlagen, an der Stelle; dort drei Sekunden blau umrandet.
                let now = ctx.input(|i| i.time);
                self.view.help_open = true;
                self.bitty.help_focus = Some((target, now + 3.0, false));
            }
            Go::Guide(g) => self.start_guide(g, ctx),
            Go::Tip(k) => {
                // Suchfeld leeren, sonst stünden weiter die Treffer statt des Tipps da.
                self.bitty.tip = k;
                self.bitty.query.clear();
                self.bitty.save();
            }
            Go::Tool(t) => self.tool = t,
            Go::Panel(p) => self.open_panel(p, ctx),
            Go::Cmd(c) => self.run_cmd(c, ctx),
        }
    }

    fn run_cmd(&mut self, c: Cmd, ctx: &egui::Context) {
        match c {
            Cmd::Save => self.save(),
            Cmd::SaveAs => self.save_as(),
            Cmd::Export => self.open_export(),
            Cmd::ExportGif => {
                self.bitty.gif_clicked = true;
                let r = self.export_gif();
                self.report_export(r);
            }
            Cmd::ExportWeb => self.export_web(),
            Cmd::SaveAse => self.save_ase_as(),
            Cmd::Undo => self.undo(),
            Cmd::Redo => self.redo(),
            Cmd::Copy => self.copy_selection(),
            Cmd::Cut => self.cut_selection(),
            Cmd::Paste => self.paste_clipboard(),
            Cmd::SelectAll => self.select_all(),
            Cmd::Deselect => self.deselect(),
            Cmd::NewSprite => self.open_new_sprite(),
            Cmd::NewLayer => self.edit_sprite(|s| {
                let name = crate::i18n::trf("Ebene {n}", &[("n", &(s.layers.len() + 1))]);
                let at = s.layer + 1;
                s.add_layer(at, name);
            }),
            Cmd::NewFrame => self.edit_sprite(|s| {
                let f = s.frame;
                s.add_frame(f, false);
            }),
            Cmd::DupFrame => self.edit_sprite(|s| {
                let f = s.frame;
                s.add_frame(f, true);
            }),
            Cmd::Play => self.toggle_play(ctx),
            Cmd::Onion => {
                self.onion = !self.onion;
                self.version = self.version.wrapping_add(1);
            }
            Cmd::Guides => self.guides.show = !self.guides.show,
            Cmd::Fit => self.fit_pending = true,
            Cmd::Fullscreen => self.toggle_fullscreen(ctx),
            Cmd::Help => self.view.help_open = true,
        }
    }
}

/// In der Hilfe: ist diese Zeile das Ziel von Bittys Suche? Dann hinscrollen
/// (einmal) und blau umranden, solange die Frist läuft.
pub(crate) fn mark_help_line(ui: &egui::Ui, r: &egui::Response, line: &str, focus: &mut Option<(&'static str, f64, bool)>) {
    let Some((target, until, scrolled)) = *focus else { return };
    if target != line {
        return;
    }
    let now = ui.input(|i| i.time);
    if now > until {
        *focus = None;
        return;
    }
    if !scrolled {
        r.scroll_to_me(Some(egui::Align::Center));
        *focus = Some((target, until, true));
    }
    ui.painter().rect_stroke(r.rect.expand(3.0), 3.0, Stroke::new(2.0, BLUE), egui::StrokeKind::Outside);
    #[cfg(not(test))]
    ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64((until - now).max(0.05)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jede_haltung_ist_16_mal_16() {
        for p in [Pose::Idle, Pose::Squish, Pose::Blink, Pose::Hop, Pose::Doze] {
            let r = rows(p);
            assert_eq!(r.len(), 16, "{p:?}");
            for row in r {
                assert_eq!(row.len(), 16, "{p:?}");
                assert!(row.bytes().all(|c| c == b'.' || (b'1'..=b'5').contains(&c)), "{p:?}");
            }
        }
    }

    #[test]
    fn normalisieren() {
        assert_eq!(normalize("Füllen & Färben!"), "fullen farben");
        assert_eq!(normalize("Größe"), "grosse");
    }

    fn e(label: &str, names: &str, text: &str) -> Entry {
        Entry { kind: "", label: label.into(), sub: String::new(), names: names.into(), text: text.into(), go: Go::Help("") }
    }

    #[test]
    fn suche_verzeiht_und_findet_andere_sprache() {
        let all = vec![e("Lasso", "", "umfährt eine freie Form"), e("Ebenen", "Layers", ""), e("Füllen", "", "Flood-Fill"), e("Export", "", "PNG GIF")];
        let first = |q: &str| search(q, &all, 8).first().map(|h| h.label.clone());
        assert_eq!(first("lassso").as_deref(), Some("Lasso"));
        assert_eq!(first("exprot").as_deref(), Some("Export"));
        assert_eq!(first("layer").as_deref(), Some("Ebenen"));
        assert_eq!(first("farbeimer").as_deref(), Some("Füllen"));
        assert_eq!(first("fullen").as_deref(), Some("Füllen"));
        assert!(search("xyzzy", &all, 8).is_empty());
        assert!(search("lasso export", &all, 8).is_empty(), "alle Wörter müssen passen");
    }

    #[test]
    fn die_hilfe_ist_durchsuchbar() {
        let all = entries();
        assert!(all.len() > 30);
        assert!(!search("Onion", &all, 8).is_empty());
    }

    #[test]
    fn suche_findet_touren_werkzeuge_panels_und_befehle() {
        use crate::tour_ui::{Guide, TourId};
        let all = entries();
        let first = |q: &str| search(q, &all, 8).first().map(|h| h.go);
        assert_eq!(first("neue ebene"), Some(Go::Cmd(Cmd::NewLayer)));
        assert_eq!(first("gif"), Some(Go::Cmd(Cmd::ExportGif)));
        assert_eq!(first("lasso"), Some(Go::Tool(crate::tools_ui::Tool::Lasso)));
        assert_eq!(first("tilemap tour"), Some(Go::Guide(Guide::Tour(TourId::Tiles, 0))));
        assert_eq!(first("kissen"), Some(Go::Tip(11)), "Pixel-Art-Tipp zur Kissen-Schattierung");
        assert!(search("schablone", &all, 8).iter().any(|h| h.go == Go::Panel(crate::dock_ui::PanelId::Template)));
    }

    #[test]
    fn tipps_im_wechsel() {
        let t = tips();
        assert_eq!(t.len(), 21);
        assert!(!t[0].1 && t[1].1 && !t[2].1, "Bedienung, Pixel-Art, Bedienung …");
    }
}
