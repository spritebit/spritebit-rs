//! Bittys Touren — wie im Web (sprites-editor/js/helper.js):
//!
//! * „Erste Schritte“ beim ersten Start (Bitty fragt vorher), danach jederzeit
//!   über Hilfe → „Touren mit Bitty“ oder Bittys Suche
//! * Themen-Touren: Animation, Ebenen, Kacheln, Foto → Sprite
//!
//! Jeder Schritt zeigt auf ein Ziel — einen Knopf, eine Leiste, ein Panel —
//! und umrandet es blau. Welches Ziel wo liegt, melden die Panels beim
//! Zeichnen selbst (`mark`, gesammelt in `BittyState::targets`). Liegt das
//! Ziel in einem zugeklappten oder gelösten Panel, klappt der Schritt es auf;
//! fehlt das Ziel trotzdem (z. B. Knöpfe, die erst mit einer Tilemap
//! erscheinen), zeigt die Blase aufs Panel. Ohne Ziel steht sie unter Bitty.
//!
//! Dazu die Lektion „Mitmachen: erster animierter Sprite“: Bitty gibt eine
//! Aufgabe und schaut selbst nach, ob sie erledigt ist — gezählt wird, was
//! im Bild steht, nicht geklickt wurde. Sie läuft in einem frischen
//! 16 × 16-Sprite, das eigene Bild bleibt, wie es ist.

use std::hash::{Hash, Hasher};

use eframe::egui::{self, Rect, Stroke};

use crate::bitty_ui::BLUE;
use crate::dock_ui::{PanelId, Side};
use crate::i18n::{tr, trf};
use crate::SpritebitApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TourId {
    Start,
    Anim,
    Layers,
    Tiles,
    Photo,
}

impl TourId {
    pub(crate) const TOPICS: [TourId; 4] = [TourId::Anim, TourId::Layers, TourId::Tiles, TourId::Photo];

    pub(crate) fn name(self) -> &'static str {
        match self {
            TourId::Start => tr("Erste Schritte"),
            TourId::Anim => tr("Animation"),
            TourId::Layers => tr("Ebenen"),
            TourId::Tiles => tr("Kacheln"),
            TourId::Photo => tr("Foto → Sprite"),
        }
    }

    /// Stichwörter für Bittys Suche — in beiden Sprachen, wie im Web.
    pub(crate) fn words(self) -> &'static str {
        match self {
            TourId::Start => "Tour Einstieg Anfang erste Schritte Überblick getting started basics intro overview",
            TourId::Anim => "Tour Animation animieren Frames Timeline abspielen GIF animate",
            TourId::Layers => "Tour Ebenen Ebene Maske Deckkraft layers layer mask opacity",
            TourId::Tiles => "Tour Kacheln Tilemap Level Spiel Godot tiles tilemap game",
            TourId::Photo => "Tour Foto Bild Schablone Vorlage abpausen photo picture template trace",
        }
    }

    fn steps(self) -> &'static [Step] {
        match self {
            TourId::Start => START,
            TourId::Anim => ANIM,
            TourId::Layers => LAYERS,
            TourId::Tiles => TILES,
            TourId::Photo => PHOTO,
        }
    }
}

/// Ein Schritt: Ziele (das erste, das gerade zu sehen ist, gewinnt), das
/// Panel, das dafür aufgeklappt wird, und der Text (deutsch = Schlüssel).
pub(crate) struct Step {
    at: &'static [&'static str],
    panel: Option<PanelId>,
    text: &'static str,
}

const fn s(at: &'static [&'static str], panel: Option<PanelId>, text: &'static str) -> Step {
    Step { at, panel, text }
}

const START: &[Step] = &[
    s(&["toolbar"], None, "Das sind deine Werkzeuge: Stift, Pinsel, Füllen, Radierer und mehr. Fahr mit der Maus drüber, dann siehst du das Tastenkürzel."),
    s(&["colors", "p-colors"], Some(PanelId::Colors), "Hier sind deine Farben. Ein Klick wählt die Farbe, mit der du malst."),
    s(&["canvas"], None, "Und hier malst du. Rechtsklick löscht, Strg + Mausrad zoomt."),
    s(&["timeline"], None, "Die Timeline: Frames für Animationen und Ebenen übereinander. Der Abspiel-Knopf spielt die Animation ab."),
    s(&["p-output"], Some(PanelId::Output), "Fertig? Unter „Code & Export“ speicherst du dein Bild als PNG, GIF, Spritesheet oder als Code."),
    s(&[], None, "Das war’s! Klick mich an, wenn du einen Tipp willst. Alle Kürzel findest du unter Hilfe (F1)."),
];

const ANIM: &[Step] = &[
    s(&["tl-grid", "timeline"], None, "Das ist die Timeline: je Frame eine Spalte, je Ebene eine Zeile. Ein Klick auf eine Zelle wählt Frame und Ebene."),
    s(
        &["tl-dup", "timeline"],
        None,
        "Der einfachste Weg zur Animation: Frame duplizieren und ein bisschen ändern. Das Plus davor fügt einen leeren Frame ein.",
    ),
    s(&["tl-onion", "timeline"], None, "Onion Skin lässt die Nachbar-Frames durchscheinen — so siehst du, wohin sich etwas bewegt."),
    s(&["tl-play", "timeline"], None, "Abspielen (Enter). Ein Klick auf die Fläche hält wieder an."),
    s(&["tl-fps", "timeline"], None, "FPS bestimmt das Tempo. Einzelne Frames können länger stehen bleiben — z. B. für ein Blinzeln."),
    s(&["tl-tag", "timeline"], None, "Tags benennen einen Abschnitt wie „Laufen“ oder „Springen“: Frames markieren, Tag anlegen."),
    s(&["export-gif", "p-output"], Some(PanelId::Output), "Fertig? Als GIF exportieren — es läuft endlos. Mit Tags auch eine Datei je Tag."),
];

const LAYERS: &[Step] = &[
    s(&["ly-list", "p-layers"], Some(PanelId::Layers), "Ebenen liegen übereinander, die oberste ist vorn. Auge blendet aus, Schloss sperrt."),
    s(
        &["ly-buttons", "p-layers"],
        Some(PanelId::Layers),
        "Neue Ebene: z. B. Kontur und Farbe getrennt — dann kannst du eins ändern, ohne das andere zu treffen.",
    ),
    s(&["ly-opacity", "p-layers"], Some(PanelId::Layers), "Deckkraft der aktiven Ebene — praktisch für Schatten oder eine Skizze zum Abpausen."),
    s(
        &["ly-mask", "p-layers"],
        Some(PanelId::Layers),
        "Eine Maske blendet Teile der Ebene aus, ohne etwas zu löschen. Malen blendet aus, Radieren wieder ein.",
    ),
    s(&["ly-buttons", "p-layers"], Some(PanelId::Layers), "Zusammenführen legt die Ebene auf die darunter — in jedem Frame."),
];

const TILES: &[Step] = &[
    s(&["tile-new", "p-tiles"], Some(PanelId::Tiles), "Für Spiel-Levels: eine Tilemap-Ebene besteht aus Kacheln fester Größe. Hier legst du eine neue an."),
    s(&["tile-convert", "p-tiles"], Some(PanelId::Tiles), "Oder du wandelst die aktive Ebene um — gleiche Stellen werden eine Kachel."),
    s(
        &["tile-mode", "p-tiles"],
        Some(PanelId::Tiles),
        "„Kacheln setzen“: eine Kachel wählen und mit dem Stift ins Raster setzen. Malst du eine Kachel an, ändert sie sich überall.",
    ),
    s(&["tile-godot", "p-tiles"], Some(PanelId::Tiles), "Für Godot exportieren: Kachelbild, Szene und JSON — den Ordner einfach ins Projekt legen."),
];

const PHOTO: &[Step] = &[
    s(&["tpl-load", "p-template"], Some(PanelId::Template), "Erst ein Foto als Schablone laden — es liegt halb durchsichtig über dem Raster."),
    s(&["tpl-trace", "p-template"], Some(PanelId::Template), "Dann aufs Raster übernehmen, am besten auf wenige Farben reduziert (etwa 8)."),
    s(&["pal-from-image", "p-palette"], Some(PanelId::Palette), "„Bild » Palette“ macht aus diesen Farben eine Palette, die du bearbeiten kannst."),
    s(&["bg-remove", "p-cleanup"], Some(PanelId::Cleanup), "Hintergrund entfernen — oder mit dem Zauberstab einzelne Flächen löschen."),
    s(
        &["despeckle", "p-cleanup"],
        Some(PanelId::Cleanup),
        "Glätten räumt einzelne Pixel weg, eine Outline gibt Kontur. Den Rest machst du mit Stift und Radierer.",
    ),
];

/// Was Bitty gerade anleitet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Guide {
    /// Erster Start: „Soll ich dir zeigen, wo hier was ist?“
    Hello,
    /// Lektion „Mitmachen“: Bitty fragt, ob es losgehen soll …
    LessonHello,
    /// … und Schritt n (LESSON.len() = geschafft).
    Lesson(usize),
    /// Welche Tour?
    Choose,
    Tour(TourId, usize),
}

/// Die Lektion: Ziel, Text und woran Bitty sieht, dass es geschafft ist
/// (verglichen mit dem Stand beim Beginn des Schritts, `lesson_mark`).
struct Lesson {
    at: &'static [&'static str],
    panel: Option<PanelId>,
    text: &'static str,
}

const LESSON: &[Lesson] = &[
    Lesson { at: &["tool-pencil", "toolbar"], panel: None, text: "Nimm den Stift und male eine kleine Figur — ein Umriss reicht. Ich schaue zu." },
    Lesson { at: &["tool-fill", "toolbar"], panel: None, text: "Jetzt das Füllen-Werkzeug: wähle eine andere Farbe und klick in die Figur." },
    Lesson { at: &["tl-dup", "timeline"], panel: None, text: "Für eine Animation brauchst du einen zweiten Frame. Dupliziere diesen hier." },
    Lesson { at: &["tl-grid", "timeline"], panel: None, text: "Du bist jetzt im neuen Frame. Ändere etwas — Arm hoch, Augen zu, ein Pixel versetzt." },
    Lesson { at: &["tl-play", "timeline"], panel: None, text: "Und jetzt abspielen! (Enter geht auch.)" },
    Lesson { at: &["export-gif", "p-output"], panel: Some(PanelId::Output), text: "Zum Schluss als GIF exportieren — dann kannst du es überall zeigen." },
];

/// Was die Blase zeigt: Text, Schritt-Zeile, Knöpfe (Name, hervorgehoben,
/// Wirkung) und das Ziel, auf das sie zeigt.
type Bubble = (String, Option<String>, Vec<(&'static str, bool, Act)>, Option<Rect>);

/// So lange steht das Lob, bevor der nächste Schritt kommt.
const YAY_SECS: f64 = 1.1;

/// Ein Knopf in der Blase und was er tut.
#[derive(Clone, Copy)]
enum Act {
    Go(Guide),
    Choose,
    /// Lektion beginnen: frischer Sprite, dann Schritt 1.
    Lesson,
    End,
}

/// Erst so viele Sekunden nach dem Start fragt Bitty nach der Tour.
const HELLO_AFTER: f64 = 0.9;

impl SpritebitApp {
    /// Ziel für Tour und Suche melden: wo es in diesem Bild liegt.
    pub(crate) fn mark(&mut self, key: &'static str, rect: Rect) {
        self.bitty.targets.insert(key, rect);
    }

    /// Panel für einen Schritt aufklappen: angepinnt hinscrollen, gelöst
    /// als Fenster neben seinem Icon.
    pub(crate) fn open_panel(&mut self, id: PanelId, ctx: &egui::Context) {
        if self.dock.is_pinned(id) {
            self.dock.focus = Some(id);
            return;
        }
        if self.dock.flyout.is_some_and(|(f, _)| f == id) {
            return;
        }
        let r = ctx.content_rect();
        let at = match self.dock.side_of(id) {
            Side::Left => egui::pos2(r.left() + 42.0, r.top() + 90.0),
            Side::Right => egui::pos2(r.right() - 42.0, r.top() + 90.0),
        };
        self.dock.flyout = Some((id, at));
    }

    /// Gemalte Pixel im Frame `f`, über alle Ebenen.
    fn frame_pixels(&self, f: usize) -> u64 {
        let sp = self.sprite();
        (0..sp.layers.len()).map(|l| sp.cel(f, l).pixels().filter(|&(_, _, v)| v != 0).count() as u64).sum()
    }

    /// Fingerabdruck des Frames `f` (alle Ebenen) — ändert sich, sobald sich darin etwas ändert.
    fn frame_key(&self, f: usize) -> u64 {
        let sp = self.sprite();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for l in 0..sp.layers.len() {
            for (x, y, v) in sp.cel(f, l).pixels() {
                if v != 0 {
                    (l, x, y, v).hash(&mut h);
                }
            }
        }
        h.finish()
    }

    /// Stand beim Beginn von Lektionsschritt `i`.
    fn lesson_mark(&self, i: usize) -> u64 {
        let sp = self.sprite();
        match i {
            0 | 1 => self.frame_pixels(sp.frame),
            2 => sp.frames.len() as u64,
            3 => self.frame_key(sp.frame),
            _ => 0,
        }
    }

    /// Ist Lektionsschritt `i` geschafft?
    fn lesson_done(&self, i: usize) -> bool {
        let sp = self.sprite();
        let start = self.bitty.lesson_start;
        match i {
            0 => self.frame_pixels(sp.frame) >= start + 8,
            1 => self.tool == crate::tools_ui::Tool::Fill && self.frame_pixels(sp.frame) > start,
            2 => sp.frames.len() as u64 > start,
            // Erst wenn sich im aktuellen Frame etwas getan hat UND er sich
            // vom Nachbarn unterscheidet — ein leerer Frame per + zählt nicht.
            3 => {
                let other = if sp.frame == 0 { 1 } else { 0 };
                sp.frames.len() > 1 && self.frame_key(sp.frame) != start && self.frame_key(sp.frame) != self.frame_key(other)
            }
            4 => self.playing,
            5 => self.bitty.gif_clicked,
            _ => false,
        }
    }

    /// Tour starten (Hilfe-Menü, Suche). „Erste Schritte“ fragt nicht nach.
    pub(crate) fn start_guide(&mut self, g: Guide, ctx: &egui::Context) {
        self.bitty.open = false;
        self.bitty.hint = None;
        if let Guide::Tour(t, i) = g {
            if let Some(p) = t.steps()[i].panel {
                self.open_panel(p, ctx);
            }
        }
        if let Guide::Lesson(i) = g {
            if let Some(p) = LESSON.get(i).and_then(|l| l.panel) {
                self.open_panel(p, ctx);
            }
            self.bitty.lesson_start = if i < LESSON.len() { self.lesson_mark(i) } else { 0 };
            self.bitty.lesson_yay = None;
            if i == 5 {
                self.bitty.gif_clicked = false;
            }
        }
        self.bitty.guide = Some(g);
        self.bitty.guide_settle = 2;
        self.bitty.hop(ctx.input(|i| i.time));
    }

    fn end_guide(&mut self) {
        self.bitty.guide = None;
        self.bitty.tour_done = true;
        self.bitty.save();
    }

    /// Erster Start: nach kurzer Zeit die Tour anbieten — aber nicht, solange
    /// ein Fenster offen ist (Startfenster, Hilfe, Fehler).
    fn maybe_hello(&mut self, ctx: &egui::Context) {
        if self.bitty.tour_done || self.bitty.guide.is_some() || self.bitty.open {
            return;
        }
        let busy = self.projects.start_open || self.view.help_open || self.error.is_some() || self.view.restore_ask;
        if busy || ctx.input(|i| i.time) < HELLO_AFTER {
            if !busy {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(HELLO_AFTER));
            }
            return;
        }
        self.bitty.guide = Some(Guide::Hello);
    }

    /// Tour-Blase samt blauem Rahmen. Gibt `true` zurück, wenn eine Tour
    /// läuft (dann zeichnet bitty_bubble nichts anderes).
    pub(crate) fn guide_bubble(&mut self, ctx: &egui::Context) -> bool {
        self.maybe_hello(ctx);
        let Some(g) = self.bitty.guide else { return false };
        // Esc beendet die Tour.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.end_guide();
            return true;
        }
        let (text, step, acts, target): Bubble = match g {
            Guide::Hello => (
                tr("Hi, ich bin Bitty! Soll ich dir kurz zeigen, wo hier was ist?").into(),
                None,
                vec![(tr("Zeig her"), true, Act::Go(Guide::Tour(TourId::Start, 0))), (tr("Nein danke"), false, Act::End)],
                None,
            ),
            Guide::LessonHello => (
                tr("Lust auf eine Mini-Lektion? Wir machen zusammen einen kleinen animierten Sprite — in 6 Schritten. Ich lege dafür einen neuen 16 × 16-Sprite an, dein Bild bleibt, wie es ist.")
                    .into(),
                None,
                vec![(tr("Zeig her"), true, Act::Lesson), (tr("Nein danke"), false, Act::End)],
                None,
            ),
            Guide::Lesson(i) if i >= LESSON.len() => (
                tr("Geschafft — dein erster animierter Sprite! Jetzt kennst du das Wichtigste. Mehr findest du jederzeit, wenn du mich anklickst.").into(),
                None,
                vec![(tr("Los geht’s"), true, Act::End)],
                None,
            ),
            Guide::Lesson(i) => {
                let st = &LESSON[i];
                let now = ctx.input(|i| i.time);
                let step = Some(trf("Lektion · Schritt {n} von {total}", &[("n", &(i + 1)), ("total", &LESSON.len())]));
                // Bitty schaut nach — ein paarmal pro Sekunde reicht.
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
                match self.bitty.lesson_yay {
                    Some(t) if now >= t => {
                        self.start_guide(Guide::Lesson(i + 1), ctx);
                        return true;
                    }
                    Some(_) => {
                        let yay = [tr("Super!"), tr("Genau so!"), tr("Klasse!")][i % 3];
                        (yay.into(), step, Vec::new(), None)
                    }
                    None => {
                        if self.lesson_done(i) {
                            // Geschafft: kurz loben, dann weiter.
                            self.bitty.lesson_yay = Some(now + YAY_SECS);
                            self.bitty.hop(now);
                        }
                        let target = st.at.iter().find_map(|k| self.bitty.targets.get(k).copied()).filter(|r| r.is_positive());
                        let acts = vec![(tr("Überspringen"), false, Act::Go(Guide::Lesson(i + 1))), (tr("Lektion beenden"), false, Act::End)];
                        (tr(st.text).into(), step, acts, target)
                    }
                }
            }
            Guide::Choose => {
                let mut acts = vec![
                    (tr("Mitmachen: erster animierter Sprite"), true, Act::Go(Guide::LessonHello)),
                    (TourId::Start.name(), false, Act::Go(Guide::Tour(TourId::Start, 0))),
                ];
                acts.extend(TourId::TOPICS.iter().map(|t| (t.name(), false, Act::Go(Guide::Tour(*t, 0)))));
                acts.push((tr("Schließen"), false, Act::End));
                (tr("Welche Tour soll ich dir zeigen?").into(), None, acts, None)
            }
            Guide::Tour(t, i) => {
                let steps = t.steps();
                let st = &steps[i.min(steps.len() - 1)];
                let last = i + 1 >= steps.len();
                // „Erste Schritte“ endet mit einer Verabschiedung ohne Ziel — die zählt nicht mit.
                let counted = if t == TourId::Start { steps.len() - 1 } else { steps.len() };
                let mut acts = Vec::new();
                if last {
                    acts.push((tr("Los geht’s"), true, Act::End));
                    if t == TourId::Start {
                        acts.push((tr("Mehr Touren"), false, Act::Choose));
                    }
                } else {
                    acts.push((tr("Weiter"), true, Act::Go(Guide::Tour(t, i + 1))));
                    if i > 0 {
                        acts.push((tr("Zurück"), false, Act::Go(Guide::Tour(t, i - 1))));
                    }
                    acts.push((tr("Tour beenden"), false, Act::End));
                }
                let step = (!(t == TourId::Start && last))
                    .then(|| trf("Schritt {n} von {total}", &[("n", &(i + 1).min(counted)), ("total", &counted)]));
                let target = st.at.iter().find_map(|k| self.bitty.targets.get(k).copied()).filter(|r| r.is_positive());
                (tr(st.text).into(), step, acts, target)
            }
        };
        let screen = ctx.content_rect();
        if let Some(r) = target {
            // Blauer Rahmen ums Ziel — große Ziele (die Fläche) innen umrandet.
            let big = r.height() > screen.height() * 0.4 || r.width() > screen.width() * 0.6;
            let ring = if big { r.shrink(3.0) } else { r.expand(3.0) };
            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("bitty-ring"))).rect_stroke(
                ring,
                4.0,
                Stroke::new(2.0, BLUE),
                egui::StrokeKind::Outside,
            );
        }
        // Blase: unter dem Ziel (passt es nicht, darüber); bei großen Zielen
        // in ihre obere Ecke; ohne Ziel unter Bitty.
        let width = 320.0;
        let h = self.bitty.guide_size.y.max(80.0);
        let anchor = target.unwrap_or(self.bitty.button);
        let big = target.is_some_and(|r| r.height() > screen.height() * 0.5);
        let mut top = if big { anchor.top() + 10.0 } else { anchor.bottom() + 10.0 };
        if !big && top + h > screen.bottom() - 8.0 {
            top = anchor.top() - 10.0 - h;
        }
        let top = top.clamp(screen.top() + 8.0, (screen.bottom() - 8.0 - h).max(screen.top() + 8.0));
        let left = (anchor.center().x - width / 2.0).clamp(screen.left() + 8.0, (screen.right() - 8.0 - width).max(screen.left() + 8.0));
        let mut chosen: Option<Act> = None;
        let resp = egui::Area::new(egui::Id::new("bitty-guide")).order(egui::Order::Foreground).fixed_pos(egui::pos2(left, top)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).stroke(Stroke::new(1.5, BLUE)).show(ui, |ui| {
                ui.set_width(width - 16.0);
                if let Some(s) = &step {
                    ui.weak(s);
                }
                ui.label(&text);
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for (label, primary, act) in &acts {
                        if ui.add(egui::Button::selectable(*primary, *label)).clicked() {
                            chosen = Some(*act);
                        }
                    }
                });
            });
        });
        self.bitty.guide_size = resp.response.rect.size();
        self.bitty.bubble = resp.response.rect;
        match chosen {
            Some(Act::Go(next)) => self.start_guide(next, ctx),
            Some(Act::Choose) => self.bitty.guide = Some(Guide::Choose),
            Some(Act::Lesson) => {
                // Frischer, kleiner Sprite — das eigene Bild bleibt, wie es ist.
                let pal = self.sprite().palette.clone();
                self.create_sprite(tr("Meine erste Animation").into(), pal, 16, 16);
                self.tool = crate::tools_ui::Tool::Pencil;
                self.start_guide(Guide::Lesson(0), ctx);
            }
            Some(Act::End) => self.end_guide(),
            None => {}
        }
        // Nach einem Schrittwechsel klappt das Panel erst auf: zwei Bilder
        // später steht das Ziel fest. Danach nicht weiter drängeln — manche
        // Ziele gibt es gerade nicht (dann zeigt die Blase aufs Panel).
        if self.bitty.guide_settle > 0 {
            self.bitty.guide_settle -= 1;
            ctx.request_repaint();
        }
        true
    }

    /// Menüpunkt „Touren mit Bitty“ (Hilfe-Menü).
    pub(crate) fn tour_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button(tr("Touren mit Bitty")).clicked() {
            self.bitty.guide = Some(Guide::Choose);
            ui.close();
        }
    }
}
