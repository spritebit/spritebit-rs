//! Bitty — das Maskottchen als kleiner Helfer, wie in der Web-Version
//! (sprites-editor/js/bitty.js und helper.js).
//!
//! Er sitzt rechts in der Menüleiste und atmet. Ein Klick (oder Strg+K)
//! öffnet seine Blase: oben ein Suchfeld über die Hilfe — in allen Sprachen
//! zugleich; ein Treffer schlägt die Hilfe an der Stelle auf und umrandet
//! sie blau —, darunter ein Tipp. Malt man auf einer gesperrten oder
//! ausgeblendeten Ebene, meldet er sich mit einem Knopf, der es behebt
//! (einmal pro Sitzung, abschaltbar). Beim ersten Export und der ersten
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
}

pub(crate) struct Hint {
    id: &'static str,
    text: String,
    fix: Option<Fix>,
}

/// Ein Treffer der Suche: Name, Ort in der Hilfe und alles, was gefunden werden darf.
#[derive(Clone)]
struct Entry {
    label: String,
    sub: String,
    /// Name in allen Sprachen (zählt wie `label`)
    names: String,
    text: String,
    /// Die Zeile in der Hilfe, die aufgeschlagen wird (so, wie sie dort steht).
    target: &'static str,
}

const IDLE_SECS: f64 = 5.0 * 60.0;

pub(crate) struct BittyState {
    open: bool,
    query: String,
    focus_query: bool,
    sel: usize,
    tip: usize,
    hint: Option<Hint>,
    /// Hinweise, die in dieser Sitzung schon kamen.
    shown: HashSet<&'static str>,
    hints_on: bool,
    off: HashSet<String>,
    moments: HashSet<String>,
    last_input: f64,
    hop_until: f64,
    bubble: Rect,
    button: Rect,
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
                    _ => {}
                }
            }
        }
    }

    fn save(&self) {
        #[cfg(not(test))]
        if let Some(p) = Self::file() {
            let mut out = format!("hints={}\ntip={}\n", if self.hints_on { 1 } else { 0 }, self.tip);
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

    fn hop(&mut self, now: f64) {
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

fn tips() -> [&'static str; 12] {
    [
        tr("Strg + K öffnet mich überall. Tipp ein Stichwort — Enter schlägt die Hilfe an der Stelle auf."),
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
        tr("F1 öffnet die Hilfe mit allen Tastenkürzeln."),
    ]
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
        scored.push((total, idx, e));
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().take(limit).map(|s| s.2).collect()
}

/// Alles, was die Suche finden kann: jede Zeile der Hilfe und jedes Kürzel.
fn entries() -> Vec<Entry> {
    let all = |s: &str| variants(s).join(" ");
    let mut out = Vec::new();
    for (title, lines) in crate::view_ui::help_sections() {
        for l in lines {
            // Das erste Wort ist meist das Stichwort („Stift — …“, „Tags …“) und zählt wie ein Name.
            let head = l.split([' ', '.']).next().unwrap_or(l);
            let label: String = if l.chars().count() > 60 { l.chars().take(57).collect::<String>() + " …" } else { l.to_string() };
            out.push(Entry { label, sub: title.to_string(), names: format!("{head} {}", all(title)), text: format!("{} {}", all(l), all(title)), target: l });
        }
    }
    for (k, what) in crate::view_ui::help_keys() {
        out.push(Entry {
            label: what.to_string(),
            sub: format!("{} · {k}", tr("Tastenkürzel")),
            names: all(what),
            text: format!("{k} {}", all(k)),
            target: what,
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
        if r.clicked() {
            self.bitty.open = !self.bitty.open;
            self.bitty.hint = None;
            self.bitty.focus_query = self.bitty.open;
            self.bitty.hop(now);
        }
    }

    /// Strg+K öffnet die Suche, Esc schließt die Blase.
    pub(crate) fn bitty_keys(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K)) {
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
        b.hint = Some(Hint { id, text, fix });
        b.open = true;
        b.focus_query = false;
        b.hop_until = b.last_input + 0.6;
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
        if !self.bitty.open {
            return;
        }
        // Klick daneben schließt (nicht auf Bitty selbst — der schaltet um).
        if ctx.input(|i| i.pointer.any_pressed()) {
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
        let mut go: Option<&'static str> = None;
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
                            go = Some(h.target);
                        }
                    }
                    ui.add_space(4.0);
                    for (i, h) in hits.iter().enumerate() {
                        let r = ui.add(egui::Button::selectable(i == self.bitty.sel, h.label.as_str())).on_hover_text(&h.sub);
                        ui.weak(&h.sub);
                        if r.clicked() {
                            go = Some(h.target);
                        }
                    }
                } else {
                    let tips = tips();
                    ui.add_space(6.0);
                    ui.label(tips[self.bitty.tip % tips.len()]);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr("Nächster Tipp")).clicked() {
                            self.bitty.tip = (self.bitty.tip + 1) % tips.len();
                            self.bitty.save();
                        }
                        if ui.button(tr("Schließen")).clicked() {
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
                _ => {}
            }
            self.hint = None;
            close = true;
        }
        if let Some(target) = go {
            // Hilfe aufschlagen, an der Stelle; dort drei Sekunden blau umrandet.
            let now = ctx.input(|i| i.time);
            self.view.help_open = true;
            self.bitty.help_focus = Some((target, now + 3.0, false));
            close = true;
        }
        if close {
            self.bitty.open = false;
            self.bitty.hint = None;
            self.bitty.query.clear();
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
        Entry { label: label.into(), sub: String::new(), names: names.into(), text: text.into(), target: "" }
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
}
