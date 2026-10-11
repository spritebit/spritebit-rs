//! Schattieren: ein Stift, der mit jedem Strich eine Stufe weitergeht —
//! wie in der Web-Version (js/shade.js).
//!
//! Man stellt eine Farbreihe zusammen — in der Reihenfolge, in der man die
//! Farben anklickt. Dann arbeitet sich das Werkzeug durch sie hindurch
//! (Linksklick):
//!
//! * leerer Pixel → die erste Farbe der Reihe
//! * Farbe aus der Reihe → die nächste, an der letzten Schluss
//! * andere Farbe → bleibt (z. B. die Umrisslinie)
//!
//! Rechtsklick geht die Reihe andersherum. Pro Strich ändert sich jeder
//! Pixel höchstens einmal; gedrückt halten geht Stufe um Stufe weiter (das
//! steuert die App). Nach Helligkeit geordnet wird die Reihe nur auf Wunsch
//! ([`sort_ramp`]) — so lassen sich auch eigene Verläufe bauen.
//!
//! Ohne Reihe geht es automatisch: ein leerer Pixel bekommt die aktuelle
//! Farbe, ein farbiger die nächsthellere bzw. -dunklere seiner Farbfamilie
//! aus der Palette ([`palette_step`]); freie Farben werden direkt ein Stück
//! heller oder dunkler gerechnet.
//!
//! Die Reihe hält Farben statt Nummern — sie überlebt so das Umsortieren
//! der Palette und gilt auch für freie Farben im Bild.

use std::collections::HashMap;

use crate::image::{Px, FREE_BASE};
use crate::light::{palette_step, shift};
use crate::palette::{Palette, Rgb};

/// So weit geht ein Schritt im Automatik-Modus (Helligkeit 0..1).
const AUTO_STEP: f64 = 0.1;

/// So viele Stufen geht die Mitte mit Verlauf, wenn keine Reihe gewählt ist.
pub const AUTO_LEVELS: u32 = 3;

/// Wahrgenommene Helligkeit (Rec. 601).
fn luma(c: Rgb) -> f64 {
    0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64
}

/// Reihe bereinigen: ohne Doppelte. Die Reihenfolge bleibt.
pub fn normalize_ramp(ramp: &[Rgb]) -> Vec<Rgb> {
    let mut out: Vec<Rgb> = Vec::with_capacity(ramp.len());
    for &c in ramp {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

/// Reihe nach Farbstufen ordnen: von hell nach dunkel — so geht ein leerer
/// Pixel beim Linksklick mit der hellsten los und wird dann dunkler.
pub fn sort_ramp(ramp: &[Rgb]) -> Vec<Rgb> {
    let mut out = normalize_ramp(ramp);
    out.sort_by(|a, b| luma(*b).total_cmp(&luma(*a)).then(a.cmp(b)));
    out
}

/// Farbe hinten an die Reihe hängen oder wieder herausnehmen.
pub fn toggle_ramp(ramp: &[Rgb], c: Rgb) -> Vec<Rgb> {
    let mut v = normalize_ramp(ramp);
    if v.contains(&c) {
        v.retain(|&r| r != c);
    } else {
        v.push(c);
    }
    v
}

/// Stufen für einen Pixel im Abstand (dx, dy) zur Pinselmitte, wenn
/// „Verlauf" an ist: in der Mitte `levels`, zum Rand weniger, ganz außen 1.
/// 0 = liegt außerhalb des runden Pinsels.
pub fn soft_level(dx: f64, dy: f64, size: u32, levels: u32) -> u32 {
    let r = (size.max(1) as f64 - 1.0) / 2.0;
    let d = dx.hypot(dy);
    if d > r + 0.5 {
        return 0;
    }
    ((levels as f64 * (1.0 - d / (r + 1.0))).ceil() as u32).max(1)
}

/// Stufen in der Mitte: so viele, wie die Reihe von leer bis zur letzten
/// Farbe braucht; ohne Reihe [`AUTO_LEVELS`].
pub fn ramp_levels(ramp: &[Rgb]) -> u32 {
    if ramp.is_empty() {
        AUTO_LEVELS
    } else {
        ramp.len() as u32
    }
}

// ── Gedrückt halten ─────────────────────────────────────────────────
// Wie eine Airbrush: jeder Pixel zählt, wie lange der Pinsel schon über
// ihm war. Je länger, desto mehr Stufen — erst langsam, dann immer
// schneller: jede Pause ist um HOLD_FACTOR kürzer als die vorige, aber nie
// kürzer als HOLD_MIN_S. Schnell drüberfahren gibt eine Stufe, langsam
// mehr, stillhalten immer weiter (wie im Web).

pub const HOLD_FIRST_S: f64 = 0.35;
pub const HOLD_FACTOR: f64 = 0.7;
pub const HOLD_MIN_S: f64 = 0.06;

/// Wartezeit in Sekunden vor dem Schritt Nummer `n` (0 = erster Schritt
/// nach dem Ansetzen bzw. nach der letzten Bewegung).
pub fn hold_delay(n: u32) -> f64 {
    (HOLD_FIRST_S * HOLD_FACTOR.powi(n.min(64) as i32)).max(HOLD_MIN_S)
}

/// Zusätzliche Stufen nach `t` Sekunden unter dem Pinsel.
pub fn hold_steps(t: f64) -> u32 {
    let (mut n, mut at) = (0, hold_delay(0));
    while t >= at && n < 255 {
        n += 1;
        at += hold_delay(n);
    }
    n
}

// ── Abstufungen erzeugen ────────────────────────────────────────────
// Aus einer Farbe eine passende Reihe machen: `n` dunklere und `n` hellere
// Töne — Schatten etwas kühler, Licht etwas wärmer ([`shift`]). Jeder
// Schritt baut auf dem vorigen auf, der Farbton wandert also mit. Graue
// bleiben grau. Wie im Web (js/shade.js makeShades).

/// So weit liegt ein Ton vom nächsten (Helligkeit 0..1).
pub const GEN_STEP: f64 = 0.12;

/// Grundfarbe in der Mitte, dazu `n` hellere und `n` dunklere Töne — von
/// hell nach dunkel (Mal-Reihenfolge), ohne Doppelte (nah an Schwarz oder
/// Weiß fallen Stufen zusammen).
pub fn make_shades(c: Rgb, n: u32) -> Vec<Rgb> {
    let mut out = vec![c];
    let (mut dark, mut light) = (c, c);
    for _ in 0..n {
        dark = shift(dark, -1, GEN_STEP);
        light = shift(light, 1, GEN_STEP);
        out.push(dark);
        out.push(light);
    }
    sort_ramp(&out)
}

/// Rechner für einen Strich: Pixelwert → Wert eine Stufe weiter.
pub struct Shader {
    /// Die Reihe in Mal-Reihenfolge (erste Farbe = die für leere Pixel).
    order: Vec<Rgb>,
    /// Die Reihe entlang (Linksklick) — ohne Reihe: dunkler.
    along: bool,
    cur: Px,
    /// Farbe → Palettennummer (die erste mit dieser Farbe).
    index: HashMap<Rgb, Px>,
    /// Automatik: einmal gerechnete Schritte merken.
    memo: HashMap<Px, Px>,
}

impl Shader {
    /// `ramp` in Mal-Reihenfolge; `along` = die Reihe entlang (Linksklick),
    /// sonst andersherum — ohne Reihe: dunkler bzw. heller. `cur` = aktuelle
    /// Farbe (nur die Automatik malt damit in leere Pixel).
    pub fn new(pal: &Palette, ramp: &[Rgb], along: bool, cur: Px) -> Self {
        let mut order = ramp.to_vec();
        if !along {
            order.reverse();
        }
        let mut index = HashMap::new();
        for (k, &c) in pal.colors.iter().enumerate() {
            index.entry(c).or_insert(k as Px + 1);
        }
        Shader { order, along, cur, index, memo: HashMap::new() }
    }

    /// Farbe als Pixelwert: Nummer, wenn sie in der Palette steht, sonst
    /// eine freie Farbe (neue werden an `free` angehängt).
    fn value_of(&self, c: Rgb, free: &mut Vec<Rgb>) -> Px {
        if let Some(&i) = self.index.get(&c) {
            return i;
        }
        let k = free.iter().position(|&f| f == c).unwrap_or_else(|| {
            free.push(c);
            free.len() - 1
        });
        FREE_BASE + k as Px
    }

    /// Farbe eines Pixelwerts (0 → `None`).
    fn color_of(v: Px, pal: &Palette, free: &[Rgb]) -> Option<Rgb> {
        if v >= FREE_BASE {
            free.get((v - FREE_BASE) as usize).copied()
        } else {
            pal.get(v)
        }
    }

    /// Ein Schritt für den Pixelwert `v`.
    pub fn step(&mut self, v: Px, pal: &Palette, free: &mut Vec<Rgb>) -> Px {
        if self.order.is_empty() {
            return self.auto(v, pal, free);
        }
        if v == 0 {
            return self.value_of(self.order[0], free);
        }
        let Some(c) = Self::color_of(v, pal, free) else { return v };
        let Some(i) = self.order.iter().position(|&o| o == c) else { return v };
        match self.order.get(i + 1) {
            Some(&next) => self.value_of(next, free),
            None => v,
        }
    }

    fn auto(&mut self, v: Px, pal: &Palette, free: &mut Vec<Rgb>) -> Px {
        if v == 0 {
            return self.cur;
        }
        if let Some(&n) = self.memo.get(&v) {
            return n;
        }
        let sign = if self.along { -1 } else { 1 };
        let n = if v >= FREE_BASE {
            match Self::color_of(v, pal, free) {
                Some(c) => self.value_of(shift(c, sign, AUTO_STEP), free),
                None => v,
            }
        } else {
            palette_step(pal, v, sign, AUTO_STEP).unwrap_or(v)
        };
        self.memo.insert(v, n);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DARK: Rgb = [0x80, 0, 0];
    const RED: Rgb = [0xff, 0, 0];
    const LIGHT: Rgb = [0xff, 0x80, 0x80];

    /// Drei Rottöne, absichtlich nicht nach Helligkeit sortiert, dazu Grün.
    fn pal() -> Palette {
        Palette::new("t", vec![LIGHT, DARK, RED, [0, 0xff, 0]])
    }

    /// Eine selbst zusammengestellte Reihe in Klick-Reihenfolge: rot, hellrot, dunkelrot.
    fn ramp() -> Vec<Rgb> {
        normalize_ramp(&[RED, LIGHT, DARK])
    }

    #[test]
    fn reihe_in_klick_reihenfolge_ohne_doppelte() {
        assert_eq!(ramp(), vec![RED, LIGHT, DARK]);
        assert_eq!(normalize_ramp(&[RED, RED]), vec![RED]);
        let r = toggle_ramp(&[LIGHT], DARK);
        assert_eq!(r, vec![LIGHT, DARK], "hinten angehängt");
        assert_eq!(toggle_ramp(&r, LIGHT), vec![DARK]);
        assert_eq!(sort_ramp(&ramp()), vec![LIGHT, RED, DARK], "auf Wunsch: hell → dunkel");
    }

    #[test]
    fn wie_ein_stift_leer_erste_dann_die_reihe_entlang() {
        let (p, mut free) = (pal(), Vec::new());
        let mut s = Shader::new(&p, &ramp(), true, 0);
        let mut v = 0;
        let mut seen = Vec::new();
        for _ in 0..4 {
            v = s.step(v, &p, &mut free);
            seen.push(v);
        }
        assert_eq!(seen, vec![3, 1, 2, 2], "rot, hellrot, dunkelrot, bleibt");
    }

    #[test]
    fn andersherum_leer_letzte_dann_zurueck() {
        let (p, mut free) = (pal(), Vec::new());
        let mut s = Shader::new(&p, &ramp(), false, 0);
        assert_eq!(s.step(0, &p, &mut free), 2);
        assert_eq!(s.step(2, &p, &mut free), 1);
        assert_eq!(s.step(3, &p, &mut free), 3, "am Anfang der Reihe ist Schluss");
    }

    #[test]
    fn andere_farben_bleiben() {
        let (p, mut free) = (pal(), vec![[1, 2, 3]]);
        let mut s = Shader::new(&p, &ramp(), true, 0);
        assert_eq!(s.step(4, &p, &mut free), 4);
        assert_eq!(s.step(FREE_BASE, &p, &mut free), FREE_BASE);
    }

    #[test]
    fn farbe_nicht_in_der_palette_wird_freie_farbe() {
        let (p, mut free) = (pal(), Vec::new());
        let mid = [0xc0, 0x40, 0x40];
        let mut s = Shader::new(&p, &[DARK, mid], true, 0);
        assert_eq!(s.step(2, &p, &mut free), FREE_BASE);
        assert_eq!(free, vec![mid]);
        let mut back = Shader::new(&p, &[DARK, mid], false, 0);
        assert_eq!(back.step(FREE_BASE, &p, &mut free), 2);
    }

    #[test]
    fn ohne_reihe_aktuelle_farbe_dann_farbfamilie() {
        let p = Palette::new("t", vec![[0x40, 0, 0], [0x90, 0, 0], [0xe0, 0x40, 0x40], [0, 0xc0, 0]]);
        let mut free = Vec::new();
        let mut s = Shader::new(&p, &[], true, 3);
        assert_eq!(s.step(0, &p, &mut free), 3, "leer → aktuelle Farbe");
        assert_eq!(s.step(2, &p, &mut free), 1, "eine Stufe dunkler, nie Grün");
        let mut up = Shader::new(&p, &[], false, 0);
        assert_eq!(up.step(3, &p, &mut free), 3, "heller gibt es nicht");
        assert_eq!(up.step(0, &p, &mut free), 0, "ohne Farbe bleibt leer");
    }

    #[test]
    fn abstufungen_erzeugen() {
        let base = [0xc0, 0x40, 0x40];
        let r = make_shades(base, 2);
        assert_eq!(r.len(), 5);
        assert_eq!(r[2], base, "Grundfarbe in der Mitte");
        assert_eq!(r, sort_ramp(&r), "schon hell → dunkel");
        // Schatten kühler: im dunkelsten Ton ist Blau im Verhältnis stärker.
        assert!(r[4][2] as f64 / (r[4][0] as f64 + 1.0) > 0x40 as f64 / 0xc0 as f64);
        assert!(make_shades([0x80; 3], 2).iter().all(|c| c[0] == c[1] && c[1] == c[2]), "Grau bleibt grau");
        assert!(make_shades([0xff; 3], 2).len() < 5, "an Weiß fallen Stufen zusammen");
    }

    #[test]
    fn zeit_unter_dem_pinsel_gibt_stufen() {
        assert_eq!(hold_steps(0.0), 0);
        assert_eq!(hold_steps(0.349), 0);
        assert_eq!(hold_steps(0.35), 1);
        assert_eq!(hold_steps(0.35 + 0.245), 2);
        assert!(hold_steps(2.0) > hold_steps(1.0) + 5, "später geht es schneller");
    }

    #[test]
    fn halten_wird_immer_schneller() {
        assert_eq!(hold_delay(0), HOLD_FIRST_S);
        assert!(hold_delay(1) < hold_delay(0) && hold_delay(2) < hold_delay(1));
        assert!((hold_delay(1) - 0.245).abs() < 1e-9);
        assert_eq!(hold_delay(50), HOLD_MIN_S);
    }

    #[test]
    fn verlauf_mitte_mehr_als_der_rand() {
        assert_eq!(soft_level(0.0, 0.0, 7, 4), 4);
        let near = soft_level(1.0, 0.0, 7, 4);
        assert!((1..=4).contains(&near));
        assert_eq!(soft_level(3.0, 0.0, 7, 4), 1);
        assert_eq!(soft_level(3.0, 3.0, 7, 4), 0, "Ecke liegt außerhalb des Kreises");
        assert_eq!(ramp_levels(&[]), AUTO_LEVELS);
        assert_eq!(ramp_levels(&[DARK, RED]), 2);
    }
}
