//! Körperteile einer Figur und die Vorlagen, die daraus eine Bewegung
//! rechnen — wie in der Web-Version (js/rig.js).
//!
//! Hat man eine Figur in Kopf, Rumpf, Arme und Beine zerlegt (je ein Teil
//! auf einer eigenen Ebene), fehlen noch zwei Angaben je Teil:
//!
//! * [`Role`] — welches Körperteil das ist; ein Arm schwingt anders als ein Kopf
//! * `pivot` — wo es angewachsen ist: die Schulter, die Hüfte, der Hals
//!
//! Beides hängt an der EBENE ([`crate::sprite::Layer::part`]), nicht am
//! Sprite: so wandert es beim Kopieren, Sortieren und Löschen von selbst mit
//! und steckt in jedem Undo-Schritt.
//!
//! Das Gelenk zählt in GITTERKANTEN, nicht in Zellen: 0 ist die linke Kante
//! der ersten Spalte, die Mitte der ersten Spalte liegt bei 0,5. So lässt
//! sich eine Schulter auch zwischen zwei Pixel legen — bei 16 px breiten
//! Figuren macht genau das den Unterschied.
//!
//! # Was man dem Ergebnis ansieht
//!
//! 1. **Jedes Teil dreht um sein eigenes Gelenk, es gibt keine Hierarchie.**
//!    Der Kopf hängt nicht am Rumpf. Darum drehen die eingebauten Vorlagen
//!    den Rumpf nie — er wird nur verschoben, sonst stünde der Kopf daneben.
//! 2. **Hoch und runter gilt für die ganze Figur.** Beim Laufen hebt sich
//!    der Körper mitsamt den Beinen. Streng genommen müsste der aufgesetzte
//!    Fuß stehen bleiben; das ginge nur, indem man ihn neu zeichnet. Lieber
//!    eine Figur, die einen Pixel rutscht, als eine, die an der Hüfte
//!    aufreißt.
//!
//! Heraus kommt ein Rohentwurf, an dem man von Hand weiterarbeitet — dafür
//! liegen die Frames danach in der Timeline.

use crate::image::Image;
use crate::sprite::{Layer, Sprite};

/// Die Rollen, in der Reihenfolge fürs Menü. Gedacht ist die Seitenansicht;
/// „hinten“ und „vorne“ sind die beiden Arme bzw. Beine, von denen beim
/// Laufen das hintere verdeckt wird (meist eine Stufe dunkler).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Head,
    Torso,
    ArmBack,
    ArmFront,
    LegBack,
    LegFront,
    Weapon,
    /// Freies Teil: Schwanz, Ohr, Umhang, Fühler, Flügel. Es passt in keine
    /// feste Rolle und bringt seine Bewegung selbst mit ([`Part::amp`],
    /// [`Part::phase`]).
    Free,
}

pub const ROLES: [Role; 8] = [Role::Head, Role::Torso, Role::ArmBack, Role::ArmFront, Role::LegBack, Role::LegFront, Role::Weapon, Role::Free];

impl Role {
    /// Der Name im Speicherstand — derselbe wie in der Web-Version.
    pub fn key(self) -> &'static str {
        match self {
            Role::Head => "head",
            Role::Torso => "torso",
            Role::ArmBack => "armBack",
            Role::ArmFront => "armFront",
            Role::LegBack => "legBack",
            Role::LegFront => "legFront",
            Role::Weapon => "weapon",
            Role::Free => "free",
        }
    }

    pub fn from_key(s: &str) -> Option<Role> {
        ROLES.into_iter().find(|r| r.key() == s)
    }

    /// Arme und Beine hängen OBEN (Schulter, Hüfte), der Rest unten.
    fn hangs_at_top(self) -> bool {
        matches!(self, Role::ArmBack | Role::ArmFront | Role::LegBack | Role::LegFront)
    }

    /// Zu einem vorderen Glied das hintere — für „Hinten“ im Ebenen-Panel.
    pub fn back_of(self) -> Option<Role> {
        match self {
            Role::ArmFront => Some(Role::ArmBack),
            Role::LegFront => Some(Role::LegBack),
            _ => None,
        }
    }
}

/// Vorgabe für den Ausschlag eines freien Teils, in Grad.
pub const FREE_AMP: f64 = 25.0;
/// Vorgabe für „im Takt“, in Grad.
pub const FREE_PHASE: f64 = 90.0;
/// Weiter als das schwingt kein Anhängsel.
pub const FREE_AMP_MAX: f64 = 90.0;

/// Rolle und Gelenk einer Ebene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub role: Role,
    /// In Gitterkanten, darf auf halben Zellen liegen.
    pub pivot: (f64, f64),
    /// Nur bei [`Role::Free`]: wie weit das Teil ausschwingt, in Grad.
    pub amp: f64,
    /// Nur bei [`Role::Free`]: wann im Takt, in Grad. 0 = mit den vorderen
    /// Beinen, 180 = dagegen, 90 = dazwischen.
    pub phase: f64,
}

impl Part {
    /// Ein Teil mit den Vorgaben für ein freies.
    pub fn new(role: Role, pivot: (f64, f64)) -> Part {
        Part { role, pivot, amp: FREE_AMP, phase: FREE_PHASE }
    }

    /// Auf die Zeichenfläche begrenzen und die Werte eines freien Teils
    /// zurechtstutzen. Die Kanten selbst sind erlaubt: 0 und `w`/`h` sind
    /// Gitterlinien, keine Zellen. Der Takt läuft rund: -90 und 270 sind
    /// dieselbe Stelle.
    pub fn clamped(mut self, w: u32, h: u32) -> Part {
        self.pivot.0 = self.pivot.0.clamp(0.0, w as f64);
        self.pivot.1 = self.pivot.1.clamp(0.0, h as f64);
        if self.role == Role::Free {
            self.amp = if self.amp.is_finite() { self.amp.clamp(0.0, FREE_AMP_MAX) } else { FREE_AMP };
            self.phase = if self.phase.is_finite() { self.phase.rem_euclid(360.0) } else { FREE_PHASE };
        } else {
            self.amp = FREE_AMP;
            self.phase = FREE_PHASE;
        }
        self
    }
}

/// Das Rechteck um alles, was gemalt ist — oder `None` bei einem leeren Bild.
/// `x1`/`y1` sind die LETZTE gefüllte Spalte bzw. Zeile.
pub fn content_box(img: &Image) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut any = false;
    for (x, y, v) in img.pixels() {
        if v == 0 {
            continue;
        }
        any = true;
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    any.then_some((x0, y0, x1, y1))
}

/// Ein brauchbares Gelenk zu einer frisch vergebenen Rolle raten, damit man
/// nicht für jedes Teil klicken muss: waagerecht in der Mitte des Teils,
/// senkrecht da, wo es am Körper hängt — Arme und Beine oben (Schulter,
/// Hüfte), Kopf, Rumpf und Waffe unten (Hals, Hüftlinie, Griff).
pub fn auto_pivot(img: &Image, role: Role) -> Option<(f64, f64)> {
    let (x0, y0, x1, y1) = content_box(img)?;
    // Ein freies Teil kann überall sitzen — Schwanz hinten, Ohr oben, Umhang
    // am Rücken. Darum keine feste Kante, sondern die Stelle des Teils, die
    // der Mitte des Bildes am nächsten liegt: dort hängt es am Körper.
    if role == Role::Free {
        let (cx, cy) = (img.width() as f64 / 2.0, img.height() as f64 / 2.0);
        return Some((cx.clamp(x0 as f64, x1 as f64 + 1.0), cy.clamp(y0 as f64, y1 as f64 + 1.0)));
    }
    let x = (x0 as f64 + x1 as f64 + 1.0) / 2.0;
    Some((x, if role.hangs_at_top() { y0 as f64 } else { y1 as f64 + 1.0 }))
}

/// Die Ebenen mit Rolle — von unten nach oben, mit ihrem Index.
pub fn rig_parts(sp: &Sprite) -> Vec<(usize, Part)> {
    sp.layers.iter().enumerate().filter_map(|(i, l)| l.part.map(|p| (i, p))).collect()
}

/// Ist aus den Ebenen überhaupt eine Figur zusammengesetzt?
pub fn has_rig(sp: &Sprite) -> bool {
    sp.layers.iter().any(|l| l.part.is_some())
}

// ════════════════════════════════════════════════════════════════════
// Vorlagen
// ════════════════════════════════════════════════════════════════════

/// Die Stellung eines Teils: Versatz in Pixeln, Drehung in Grad.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub dx: i32,
    pub dy: i32,
    pub rot: f64,
}

/// Stillstand — was eine Vorlage nicht nennt.
pub const REST: Pose = Pose { dx: 0, dy: 0, rot: 0.0 };

/// Die Stellungen aller Rollen in einem Augenblick.
pub type Frame = [Pose; 8];

fn idx(role: Role) -> usize {
    ROLES.iter().position(|r| *r == role).unwrap()
}

/// Die Stellung einer Rolle aus einem Frame.
pub fn pose_of(frame: &Frame, role: Role) -> Pose {
    frame[idx(role)]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Template {
    Idle,
    Walk,
    WalkQuad,
    Run,
    Jump,
    Attack,
    Hit,
    Death,
}

pub const TEMPLATES: [Template; 8] =
    [Template::Idle, Template::Walk, Template::WalkQuad, Template::Run, Template::Jump, Template::Attack, Template::Hit, Template::Death];

impl Template {
    pub fn key(self) -> &'static str {
        match self {
            Template::Idle => "idle",
            Template::Walk => "walk",
            Template::WalkQuad => "walkQuad",
            Template::Run => "run",
            Template::Jump => "jump",
            Template::Attack => "attack",
            Template::Hit => "hit",
            Template::Death => "death",
        }
    }

    pub fn from_key(s: &str) -> Option<Template> {
        TEMPLATES.into_iter().find(|t| t.key() == s)
    }

    /// Vorschlag für die Bildzahl.
    pub fn frames(self) -> usize {
        match self {
            Template::Idle => 4,
            Template::Hit => 4,
            Template::Attack => 5,
            Template::Jump | Template::Death => 6,
            Template::Walk | Template::WalkQuad | Template::Run => 8,
        }
    }

    /// Läuft die Bewegung rund (Laufen) oder einmal ab (Sprung, Schlag)?
    pub fn loops(self) -> bool {
        matches!(self, Template::Idle | Template::Walk | Template::WalkQuad | Template::Run)
    }

    /// Das Signal für freie Teile (Schwanz, Ohr, Umhang …) an der Stelle `p`,
    /// zwischen -1 und 1. Wie weit ein Teil damit dreht, sagt sein eigener
    /// Ausschlag ([`Part::amp`]).
    pub fn swing(self, p: f64) -> f64 {
        match self {
            // Atmen und Schritte: der Schwanz wiegt im Takt mit.
            Template::Idle | Template::Walk | Template::WalkQuad | Template::Run => (std::f64::consts::TAU * p).sin(),
            // Hoch beim Abstoßen, danach schleppt es hinterher.
            Template::Jump => seg(p, &[0.0, 1.0, -1.0, -1.0, -0.5, 0.0]),
            // Mit dem Ausholen zurück, mit dem Schlag nach vorn.
            Template::Attack => seg(p, &[0.0, -1.0, 1.0, 1.0, 0.2]),
            // Fliegt mit dem Rückstoß hoch und federt zurück.
            Template::Hit => seg(p, &[0.0, 1.0, 0.6, 0.0]),
            // Sinkt in eine Richtung und bleibt liegen.
            Template::Death => seg(p, &[0.0, 0.3, 0.6, 0.85, 1.0, 1.0]),
        }
    }
}

/// Lineare Fahrt durch Stützwerte: `seg(0.5, &[0.0, 10.0, 0.0]) == 10.0`.
pub fn seg(p: f64, stops: &[f64]) -> f64 {
    let n = stops.len() - 1;
    let x = p.clamp(0.0, 1.0) * n as f64;
    let i = (x.floor() as usize).min(n - 1);
    stops[i] + (stops[i + 1] - stops[i]) * (x - i as f64)
}

/// Pixel werden ganz gezählt. `round` von Rust rundet halbe Werte vom
/// Nullpunkt weg; die Web-Version rundet mit `Math.round` immer AUFWÄRTS.
/// Damit beide dieselben Frames bauen, wird hier nachgeholfen.
fn px(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}

/// Grad auf Zehntel.
fn deg(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Ein Versatz, den der Oberkörper gegen die Beine macht, reißt die Figur an
/// der Hüfte auf — Teile sind starr, da wächst nichts nach. Ein Pixel liest
/// sich noch als Neigung, zwei als Bruch.
fn lean(v: f64) -> i32 {
    px(v).clamp(-1, 1)
}

fn put(frame: &mut Frame, role: Role, pose: Pose) {
    frame[idx(role)] = pose;
}

fn put_all(frame: &mut Frame, roles: &[Role], pose: Pose) {
    for r in roles {
        put(frame, *r, pose);
    }
}

const BODY: [Role; 5] = [Role::Head, Role::Torso, Role::ArmBack, Role::ArmFront, Role::Weapon];

/// Die Stellungen aller Teile an der Stelle `p` (0..1) der Bewegung.
/// `s` ist die Stärke, 1 = normal.
pub fn pose_at(tpl: Template, p: f64, s: f64) -> Frame {
    let mut f: Frame = [REST; 8];
    let tau = std::f64::consts::TAU;
    let pi = std::f64::consts::PI;
    match tpl {
        // Atmen im Stand. Die GANZE Figur atmet mit demselben Versatz: ginge
        // der Kopf allein einen Pixel hoch, klaffte am Hals eine Lücke.
        Template::Idle => {
            let body = Pose { dx: 0, dy: px(0.6 * s * (tau * p).sin()), rot: 0.0 };
            put_all(&mut f, &ROLES, body);
        }

        // Laufen: Arme und Beine im Gegentakt, der Körper hebt sich zur Mitte
        // jedes Schritts. Eine volle Runde sind zwei Schritte.
        Template::Walk => {
            let a = tau * p;
            // Hoch und runter geht DOPPELT so schnell wie die Schritte, und
            // zwar als volle Welle, nicht als Betrag: ein Betrag wäre um die
            // Spitze spiegelgleich, und dann käme bei sechs oder acht Frames
            // zweimal hintereinander dasselbe Bild heraus.
            let dy = px(0.7 * s * (2.0 * a).sin());
            let bob = Pose { dx: 0, dy, rot: 0.0 };
            let swing = |amp: f64, phase: f64| Pose { dx: 0, dy, rot: deg(amp * s * (a + phase).sin()) };
            put_all(&mut f, &BODY, bob);
            put(&mut f, Role::LegFront, swing(26.0, 0.0));
            put(&mut f, Role::LegBack, swing(26.0, pi));
            put(&mut f, Role::ArmFront, swing(-18.0, 0.0));
            put(&mut f, Role::ArmBack, swing(-18.0, pi));
            put(&mut f, Role::Weapon, swing(-18.0, 0.0));
        }

        // Vier Beine, Gangart Trab: die Beine laufen ÜBER KREUZ, das nahe
        // Vorderbein mit dem fernen Hinterbein. Die Vorderbeine liegen auf
        // den Arm-Rollen; anders als beim Zweibeiner schwingen alle vier
        // gleich weit — ein Hund mit kurz trippelnden Vorderbeinen sieht
        // lahm aus.
        Template::WalkQuad => {
            let a = tau * p;
            let dy = px(0.7 * s * (2.0 * a).sin());
            let bob = Pose { dx: 0, dy, rot: 0.0 };
            let swing = |phase: f64| Pose { dx: 0, dy, rot: deg(30.0 * s * (a + phase).sin()) };
            put_all(&mut f, &[Role::Head, Role::Torso, Role::Weapon], bob);
            put(&mut f, Role::ArmFront, swing(0.0));
            put(&mut f, Role::LegBack, swing(0.0));
            put(&mut f, Role::ArmBack, swing(pi));
            put(&mut f, Role::LegFront, swing(pi));
        }

        // Rennen: dieselbe Mechanik, größere Ausschläge, und die Figur lehnt
        // sich nach vorn — als Versatz, nicht als Drehung.
        Template::Run => {
            let a = tau * p;
            let dy = px(1.2 * s * (2.0 * a).sin());
            let fwd = lean(1.2 * s);
            let swing = |amp: f64, phase: f64, dx: i32| Pose { dx, dy, rot: deg(amp * s * (a + phase).sin()) };
            put(&mut f, Role::Head, Pose { dx: fwd, dy, rot: 0.0 });
            put(&mut f, Role::Torso, Pose { dx: fwd, dy, rot: 0.0 });
            put(&mut f, Role::LegFront, swing(42.0, 0.0, 0));
            put(&mut f, Role::LegBack, swing(42.0, pi, 0));
            put(&mut f, Role::ArmFront, swing(-32.0, 0.0, fwd));
            put(&mut f, Role::ArmBack, swing(-32.0, pi, fwd));
            put(&mut f, Role::Weapon, swing(-32.0, 0.0, fwd));
        }

        // Ducken, abstoßen, steigen, oben, fallen, aufkommen.
        Template::Jump => {
            let dy = px(seg(p, &[0.0, 2.0, -2.0, -3.0, -1.0, 1.0]) * s);
            let body = Pose { dx: 0, dy, rot: 0.0 };
            let leg = deg(seg(p, &[0.0, -12.0, 22.0, 18.0, 6.0, 0.0]) * s);
            let arm = deg(seg(p, &[0.0, 18.0, -30.0, -26.0, -10.0, 0.0]) * s);
            put_all(&mut f, &BODY, body);
            put(&mut f, Role::LegFront, Pose { dx: 0, dy, rot: leg });
            put(&mut f, Role::LegBack, Pose { dx: 0, dy, rot: -leg });
            put_all(&mut f, &[Role::ArmFront, Role::ArmBack, Role::Weapon], Pose { dx: 0, dy, rot: arm });
        }

        // Ausholen, treffen, nachschwingen, zurück. Die Waffe macht die
        // Bewegung des vorderen Arms mit.
        Template::Attack => {
            let arm = deg(seg(p, &[0.0, -48.0, 58.0, 70.0, 12.0]) * s);
            let dx = lean(seg(p, &[0.0, -1.0, 2.0, 1.0, 0.0]) * s);
            let body = Pose { dx, dy: 0, rot: 0.0 };
            put(&mut f, Role::Head, body);
            put(&mut f, Role::Torso, body);
            put(&mut f, Role::ArmFront, Pose { dx, dy: 0, rot: arm });
            put(&mut f, Role::Weapon, Pose { dx, dy: 0, rot: arm });
            put(&mut f, Role::ArmBack, Pose { dx, dy: 0, rot: deg(seg(p, &[0.0, 18.0, -14.0, -10.0, 0.0]) * s) });
        }

        // Zurückgestoßen, der Kopf kippt, die Arme fliegen hoch — und alles
        // federt zurück.
        Template::Hit => {
            let dx = px(seg(p, &[0.0, -3.0, -2.0, 0.0]) * s);
            let arm = deg(seg(p, &[0.0, 16.0, 10.0, 0.0]) * s);
            put(&mut f, Role::Head, Pose { dx, dy: 0, rot: deg(seg(p, &[0.0, -10.0, -5.0, 0.0]) * s) });
            put(&mut f, Role::Torso, Pose { dx, dy: 0, rot: 0.0 });
            put(&mut f, Role::LegFront, Pose { dx, dy: 0, rot: deg(seg(p, &[0.0, -8.0, -4.0, 0.0]) * s) });
            put(&mut f, Role::LegBack, Pose { dx, dy: 0, rot: deg(seg(p, &[0.0, 8.0, 4.0, 0.0]) * s) });
            put_all(&mut f, &[Role::ArmFront, Role::ArmBack, Role::Weapon], Pose { dx, dy: 0, rot: arm });
        }

        // Die Figur sackt zusammen. Umkippen wäre eine Drehung der ganzen
        // Figur — die kann es ohne Hierarchie nicht geben. Der Kopf sinkt dem
        // Rumpf VORAUS: er rutscht in die Schultern statt vom Hals abzuheben.
        Template::Death => {
            let dy = px(seg(p, &[0.0, 0.0, 1.0, 2.0, 3.0, 4.0]) * s);
            let body = Pose { dx: 0, dy, rot: 0.0 };
            let arm = deg(seg(p, &[0.0, 8.0, 18.0, 30.0, 40.0, 46.0]) * s);
            let leg = deg(seg(p, &[0.0, 5.0, 12.0, 20.0, 27.0, 32.0]) * s);
            put(
                &mut f,
                Role::Head,
                Pose { dx: 0, dy: px(seg(p, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0]) * s), rot: deg(seg(p, &[0.0, -6.0, -14.0, -24.0, -32.0, -36.0]) * s) },
            );
            put(&mut f, Role::Torso, body);
            put(&mut f, Role::LegFront, Pose { dx: 0, dy, rot: -leg });
            put(&mut f, Role::LegBack, Pose { dx: 0, dy, rot: leg });
            put_all(&mut f, &[Role::ArmFront, Role::ArmBack, Role::Weapon], Pose { dx: 0, dy, rot: arm });
        }
    }
    f
}

/// Die Stellungen für `n` Frames.
///
/// Rundlaufende Bewegungen werden bei 0, 1/n, 2/n … abgefragt — die Stelle 1
/// wäre wieder 0 und ergäbe einen doppelten Frame. Einmalige Bewegungen
/// laufen dagegen von 0 bis 1 durch, damit auch das Ende dabei ist.
pub fn build_poses(tpl: Template, n: usize, strength: f64) -> Vec<Frame> {
    phases(tpl, n).into_iter().map(|p| pose_at(tpl, p, strength)).collect()
}

/// Die Stellen in der Bewegung, an denen die `n` Frames abgefragt werden.
pub fn phases(tpl: Template, n: usize) -> Vec<f64> {
    let count = if n == 0 { tpl.frames() } else { n.clamp(2, 64) };
    let last = if tpl.loops() { count } else { count - 1 };
    (0..count).map(|i| i as f64 / last as f64).collect()
}

/// Die Stellung eines freien Teils an der Stelle `p`. Es dreht um seinen
/// eigenen Ausschlag und wandert im Übrigen mit dem Körper — ein eigener
/// Versatz würde es vom Rumpf lösen.
///
/// „Im Takt“ verschiebt die Stelle in der Bewegung, bei einer rundlaufenden
/// umlaufend. Bei einer einmaligen gäbe es nichts zu verschieben.
pub fn free_pose(tpl: Template, p: f64, strength: f64, part: &Part, body: Pose) -> Pose {
    let at = if tpl.loops() { (p + part.phase / 360.0).rem_euclid(1.0) } else { p };
    Pose { dx: body.dx, dy: body.dy, rot: deg(part.amp * strength * tpl.swing(at)) }
}

/// Ein Teil verschieben und um sein Gelenk drehen — auf einer Fläche
/// derselben Größe. Gerechnet wird rückwärts (für jedes Zielpixel die Quelle
/// suchen) mit nächstem Nachbarn: es entstehen keine Mischfarben, jede Zelle
/// behält ihren Palette-Index. Positive Grad drehen im Uhrzeigersinn, wie bei
/// der freien Drehung ([`crate::transform`]). Was über den Rand gerät, fällt
/// weg — die Zeichenfläche wächst nicht mit.
pub fn transform_part(img: &Image, pivot: (f64, f64), pose: Pose) -> Image {
    if pose.dx == 0 && pose.dy == 0 && pose.rot == 0.0 {
        return img.clone();
    }
    let (w, h) = (img.width(), img.height());
    let mut out = Image::new(w, h);
    let rad = pose.rot.to_radians();
    let (cos, sin) = (rad.cos(), rad.sin());
    let (pxv, pyv) = pivot;
    for y in 0..h {
        for x in 0..w {
            // Von der Zielmitte zurück: erst den Versatz abziehen, dann um
            // das Gelenk zurückdrehen — das ist die Quellkoordinate.
            let tx = x as f64 + 0.5 - pose.dx as f64 - pxv;
            let ty = y as f64 + 0.5 - pose.dy as f64 - pyv;
            let sx = (tx * cos + ty * sin + pxv).floor();
            let sy = (-tx * sin + ty * cos + pyv).floor();
            if sx < 0.0 || sy < 0.0 || sx >= w as f64 || sy >= h as f64 {
                continue;
            }
            let v = img.get(sx as u32, sy as u32);
            if v != 0 {
                out.set(x, y, v);
            }
        }
    }
    out
}

/// Einen Frame bauen: jede Ebene des Grundframes in ihre Stellung bringen.
/// Ebenen ohne Rolle (Hintergrund, Schatten) kommen unverändert zurück.
pub fn build_frame(base: &[Image], layers: &[Layer], frame: &Frame, free: Option<&dyn Fn(&Part) -> Pose>) -> Vec<Image> {
    base.iter()
        .zip(layers)
        .map(|(img, l)| match l.part {
            // Freie Teile bringen ihre Bewegung selbst mit (siehe free_pose) —
            // die Vorlage kennt nur die festen Rollen.
            Some(p) if p.role == Role::Free => transform_part(img, p.pivot, free.map_or(REST, |f| f(&p))),
            Some(p) => transform_part(img, p.pivot, pose_of(frame, p.role)),
            None => img.clone(),
        })
        .collect()
}

/// Die ganze Bewegung: aus einem Grundframe `n` fertige Frames.
pub fn build_frames(base: &[Image], layers: &[Layer], tpl: Template, n: usize, strength: f64) -> Vec<Vec<Image>> {
    phases(tpl, n)
        .into_iter()
        .map(|p| {
            let frame = pose_at(tpl, p, strength);
            let body = pose_of(&frame, Role::Torso);
            build_frame(base, layers, &frame, Some(&|part: &Part| free_pose(tpl, p, strength, part, body)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprite::Sprite;

    /// Ein 5 × 5-Bild mit einem senkrechten Strich in Spalte 2, Zeilen 1..3.
    fn bar() -> Image {
        let mut img = Image::new(5, 5);
        for y in 1..=3 {
            img.set(2, y, 7);
        }
        img
    }

    fn rows(img: &Image) -> Vec<String> {
        (0..img.height()).map(|y| (0..img.width()).map(|x| if img.get(x, y) == 0 { '0' } else { '7' }).collect()).collect()
    }

    #[test]
    fn content_box_umschliesst_das_gemalte() {
        assert_eq!(content_box(&bar()), Some((2, 1, 2, 3)));
        assert_eq!(content_box(&Image::new(4, 4)), None);
    }

    #[test]
    fn arme_haengen_oben_koepfe_unten() {
        let mut img = Image::new(6, 6);
        for y in 2..=3 {
            for x in 1..=4 {
                img.set(x, y, 1);
            }
        }
        assert_eq!(auto_pivot(&img, Role::ArmFront), Some((3.0, 2.0)));
        assert_eq!(auto_pivot(&img, Role::LegBack), Some((3.0, 2.0)));
        assert_eq!(auto_pivot(&img, Role::Head), Some((3.0, 4.0)));
        assert_eq!(auto_pivot(&img, Role::Weapon), Some((3.0, 4.0)));
        assert_eq!(auto_pivot(&Image::new(3, 3), Role::Head), None);
    }

    #[test]
    fn ungerade_breite_gibt_ein_halbes_gelenk() {
        let mut img = Image::new(4, 4);
        for x in 1..=3 {
            img.set(x, 1, 1);
        }
        assert_eq!(auto_pivot(&img, Role::ArmFront), Some((2.5, 1.0)));
    }

    #[test]
    fn seg_faehrt_linear_durch_die_stuetzwerte() {
        assert_eq!(seg(0.0, &[0.0, 10.0, 0.0]), 0.0);
        assert_eq!(seg(0.5, &[0.0, 10.0, 0.0]), 10.0);
        assert_eq!(seg(1.0, &[0.0, 10.0, 0.0]), 0.0);
        assert_eq!(seg(0.25, &[0.0, 10.0, 0.0]), 5.0);
        assert_eq!(seg(-1.0, &[4.0, 9.0]), 4.0);
        assert_eq!(seg(2.0, &[4.0, 9.0]), 9.0);
    }

    #[test]
    fn rundlaufende_wiederholen_den_ersten_frame_nicht() {
        let ps = build_poses(Template::Walk, 8, 1.0);
        assert_eq!(ps.len(), 8);
        assert_ne!(ps[0], ps[7]);
        assert_eq!(ps[7], pose_at(Template::Walk, 7.0 / 8.0, 1.0));
    }

    #[test]
    fn einmalige_laufen_bis_ans_ende_durch() {
        let ps = build_poses(Template::Attack, 5, 1.0);
        assert_eq!(ps[0], pose_at(Template::Attack, 0.0, 1.0));
        assert_eq!(ps[4], pose_at(Template::Attack, 1.0, 1.0));
    }

    #[test]
    fn bildzahl_bleibt_zwischen_2_und_64() {
        assert_eq!(build_poses(Template::Walk, 1, 1.0).len(), 2);
        assert_eq!(build_poses(Template::Walk, 999, 1.0).len(), 64);
        for t in TEMPLATES {
            assert_eq!(build_poses(t, 0, 1.0).len(), t.frames(), "{}", t.key());
        }
    }

    #[test]
    fn keine_vorlage_dreht_den_rumpf() {
        for t in TEMPLATES {
            for f in build_poses(t, 8, 1.0) {
                assert_eq!(pose_of(&f, Role::Torso).rot, 0.0, "{}", t.key());
            }
        }
    }

    #[test]
    fn oberkoerper_und_beine_laufen_nie_um_mehr_als_einen_pixel_auseinander() {
        for t in TEMPLATES {
            for f in build_poses(t, 8, 2.0) {
                let (legs, torso) = (pose_of(&f, Role::LegFront), pose_of(&f, Role::Torso));
                assert!((torso.dx - legs.dx).abs() <= 1, "{}: dx", t.key());
                assert!((torso.dy - legs.dy).abs() <= 1, "{}: dy", t.key());
            }
        }
    }

    #[test]
    fn der_kopf_sinkt_nie_ueber_den_rumpf_hinaus_nach_oben_weg() {
        for t in TEMPLATES {
            for f in build_poses(t, 8, 2.0) {
                let (head, torso) = (pose_of(&f, Role::Head), pose_of(&f, Role::Torso));
                assert!(head.dy >= torso.dy, "{}: Kopf über Rumpf", t.key());
                assert!((head.dx - torso.dx).abs() <= 1, "{}: dx", t.key());
            }
        }
    }

    #[test]
    fn rundlaufende_zeigen_nie_zweimal_hintereinander_dasselbe_bild() {
        for t in TEMPLATES {
            if !t.loops() {
                continue;
            }
            let ps = build_poses(t, 0, 1.0);
            for i in 0..ps.len() {
                assert_ne!(ps[i], ps[(i + 1) % ps.len()], "{}: Bild {}", t.key(), i + 1);
            }
        }
    }

    #[test]
    fn vierbeiner_laeuft_ueber_kreuz_mit_gleichem_ausschlag() {
        for f in build_poses(Template::WalkQuad, 8, 1.0) {
            let vf = pose_of(&f, Role::ArmFront).rot;
            assert_eq!(pose_of(&f, Role::LegBack).rot, vf);
            assert_eq!(pose_of(&f, Role::ArmBack).rot, -vf);
            assert_eq!(pose_of(&f, Role::LegFront).rot, -vf);
        }
    }

    #[test]
    fn ohne_versatz_und_drehung_kommt_eine_kopie_zurueck() {
        let g = bar();
        assert_eq!(rows(&transform_part(&g, (2.0, 2.0), REST)), rows(&g));
    }

    #[test]
    fn versatz_schiebt_die_pixel_was_hinausfaellt_ist_weg() {
        let out = transform_part(&bar(), (2.0, 2.0), Pose { dx: 1, dy: 0, rot: 0.0 });
        assert_eq!(rows(&out), ["00000", "00070", "00070", "00070", "00000"]);
        let gone = transform_part(&bar(), (2.0, 2.0), Pose { dx: 9, dy: 0, rot: 0.0 });
        assert!(gone.is_empty());
    }

    #[test]
    fn neunzig_grad_um_die_bildmitte_legt_den_strich_quer() {
        let out = transform_part(&bar(), (2.5, 2.5), Pose { dx: 0, dy: 0, rot: 90.0 });
        assert_eq!(rows(&out), ["00000", "00000", "07770", "00000", "00000"]);
    }

    #[test]
    fn drehen_um_ein_gelenk_am_rand_laesst_das_gelenk_liegen() {
        // Der Strich zeigt nach unten; im Uhrzeigersinn (y zählt nach unten)
        // wird daraus links.
        let out = transform_part(&bar(), (2.5, 1.5), Pose { dx: 0, dy: 0, rot: 90.0 });
        assert_eq!(out.get(2, 1), 7, "das Pixel am Gelenk bleibt liegen");
        assert_eq!(rows(&out), ["00000", "77700", "00000", "00000", "00000"]);
    }

    #[test]
    fn build_frame_ruehrt_ebenen_ohne_rolle_nicht_an() {
        let base = vec![bar(), bar()];
        let mut back = Layer::new("Hintergrund");
        back.part = None;
        let mut head = Layer::new("Kopf");
        head.part = Some(Part::new(Role::Head, (2.0, 2.0)));
        let mut f: Frame = [REST; 8];
        put(&mut f, Role::Head, Pose { dx: 1, dy: 0, rot: 0.0 });
        let out = build_frame(&base, &[back, head], &f, None);
        assert_eq!(rows(&out[0]), rows(&bar()));
        assert_eq!(out[1].get(3, 2), 7);
    }

    #[test]
    fn eine_erzeugte_bewegung_bleibt_in_jedem_frame_sichtbar() {
        let mut sp = Sprite::new("x", 16, 16).unwrap();
        sp.add_layer(1, "Kopf");
        for y in 2..6 {
            for x in 5..10 {
                sp.cel_mut(0, 1).set(x, y, 3);
            }
        }
        sp.layers[1].part = Some(Part::new(Role::Head, (7.5, 6.0)));
        let base: Vec<_> = (0..sp.layers.len()).map(|l| sp.cel(0, l).clone()).collect();
        let frames = build_frames(&base, &sp.layers, Template::Walk, 8, 1.2);
        assert_eq!(frames.len(), 8);
        for (i, cels) in frames.iter().enumerate() {
            assert!(!cels[1].is_empty(), "Frame {i} ist leer");
        }
    }

    #[test]
    fn ein_freies_teil_bekommt_die_vorgaben_und_wird_zurechtgestutzt() {
        let p = Part::new(Role::Free, (2.0, 3.0));
        assert_eq!((p.amp, p.phase), (FREE_AMP, FREE_PHASE));
        let wide = Part { amp: 999.0, phase: -90.0, ..Part::new(Role::Free, (2.0, 3.0)) }.clamped(16, 16);
        assert_eq!(wide.amp, FREE_AMP_MAX);
        assert_eq!(wide.phase, 270.0, "minus 90 ist dieselbe Stelle wie 270");
        // Feste Rollen tragen keinen Ausschlag mit sich herum.
        let head = Part { amp: 40.0, phase: 10.0, ..Part::new(Role::Head, (1.0, 1.0)) }.clamped(16, 16);
        assert_eq!((head.amp, head.phase), (FREE_AMP, FREE_PHASE));
    }

    #[test]
    fn der_ausschlag_bestimmt_wie_weit_ein_freies_teil_dreht() {
        let part = Part { amp: 30.0, phase: 0.0, ..Part::new(Role::Free, (0.0, 0.0)) };
        let rots: Vec<f64> = phases(Template::Walk, 8).into_iter().map(|p| free_pose(Template::Walk, p, 1.0, &part, REST).rot).collect();
        assert!(rots.iter().cloned().fold(0.0, f64::max) <= 30.0);
        assert!(rots.iter().any(|r| *r != 0.0));
        let wide = Part { amp: 60.0, ..part };
        for (i, p) in phases(Template::Walk, 8).into_iter().enumerate() {
            let r = free_pose(Template::Walk, p, 1.0, &wide, REST).rot;
            assert!((r - 2.0 * rots[i]).abs() < 0.2, "Bild {}", i + 1);
        }
    }

    #[test]
    fn im_takt_verschiebt_die_bewegung_umlaufend() {
        let part = Part { amp: 30.0, phase: 0.0, ..Part::new(Role::Free, (0.0, 0.0)) };
        for p in phases(Template::Walk, 8) {
            let plain = free_pose(Template::Walk, p, 1.0, &part, REST).rot;
            let half = free_pose(Template::Walk, p, 1.0, &Part { phase: 180.0, ..part }, REST).rot;
            let full = free_pose(Template::Walk, p, 1.0, &Part { phase: 360.0, ..part }, REST).rot;
            assert!((half + plain).abs() < 0.2, "eine halbe Runde später ist es die Gegenbewegung");
            assert_eq!(full, plain, "eine ganze Runde später ist es dasselbe");
        }
    }

    #[test]
    fn ein_freies_teil_wandert_mit_dem_koerper_statt_abzureissen() {
        let part = Part { amp: 30.0, phase: 0.0, ..Part::new(Role::Free, (0.0, 0.0)) };
        for t in TEMPLATES {
            for p in phases(t, 8) {
                let body = pose_of(&pose_at(t, p, 1.0), Role::Torso);
                let free = free_pose(t, p, 1.0, &part, body);
                assert_eq!((free.dx, free.dy), (body.dx, body.dy), "{}", t.key());
            }
            // und jede Vorlage bewegt es überhaupt
            assert!(phases(t, 8).into_iter().any(|p| free_pose(t, p, 1.0, &part, REST).rot != 0.0), "{}", t.key());
        }
    }

    #[test]
    fn ein_freies_teil_haengt_an_der_seite_die_zur_bildmitte_zeigt() {
        // 20 × 20, Mitte bei (10, 10). Schwanz links oben (2..5, 5..9).
        let mut g = Image::new(20, 20);
        for y in 5..=9 {
            for x in 2..=5 {
                g.set(x, y, 1);
            }
        }
        assert_eq!(auto_pivot(&g, Role::Free), Some((6.0, 10.0)), "die untere rechte Ecke");
        // Ohr oben mittig (9..11, 1..3): Unterkante.
        let mut e = Image::new(20, 20);
        for y in 1..=3 {
            for x in 9..=11 {
                e.set(x, y, 1);
            }
        }
        assert_eq!(auto_pivot(&e, Role::Free), Some((10.0, 4.0)));
    }

    #[test]
    fn rollen_und_vorlagen_heissen_wie_im_web() {
        for r in ROLES {
            assert_eq!(Role::from_key(r.key()), Some(r));
        }
        for t in TEMPLATES {
            assert_eq!(Template::from_key(t.key()), Some(t));
        }
        assert_eq!(Role::from_key("nase"), None);
        assert_eq!(Role::ArmFront.back_of(), Some(Role::ArmBack));
        assert_eq!(Role::Head.back_of(), None);
    }
}
