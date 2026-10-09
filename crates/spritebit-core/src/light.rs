//! Lichtquelle: Kantenlicht und Schlagschatten — wie `light.js` der Web-Version.
//!
//! Ein 2D-Sprite kennt seine Form nicht. Darum wird nicht „echt“ beleuchtet,
//! sondern so, wie man es in Pixel-Art von Hand macht:
//!
//! * [`light`] — Pixel, an deren Lichtseite (Richtung der Lampe) innerhalb
//!   von `width` Schritten Leere liegt, werden heller; Pixel mit Leere auf der
//!   abgewandten Seite dunkler. Leere auf BEIDEN Seiten (1-Pixel-Linie):
//!   bleibt, wie er ist.
//! * [`drop_shadow`] — die Silhouette, von der Lampe weg versetzt, in bisher
//!   leere Pixel gemalt.
//!
//! Farben: Palettenpixel bekommen eine hellere/dunklere Farbe derselben
//! Familie aus der Palette. Gibt es keine, bleibt der Pixel — außer
//! `allow_free` ist an: dann wird die berechnete Farbe eine freie Farbe.
//! Freie Farben werden immer direkt berechnet, mit dem üblichen Kniff:
//! Licht etwas wärmer, Schatten etwas kühler.
//!
//! Gerechnet wird nur über die gefüllten Pixel (bzw. ihren Umkreis), damit
//! leere Riesenflächen nichts kosten.

use std::collections::HashMap;

use crate::image::{Image, Px, FREE_BASE};
use crate::palette::{Palette, Rgb};
use crate::selection::rgb_of;

/// Richtung, AUS der das Licht kommt: `dx`, `dy` ∈ {-1, 0, 1}; y wächst nach unten.
pub type LightDir = (i32, i32);

/// Einstellungen für [`light`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightOpts {
    /// Wie viele Pixel vom Rand her (1–3).
    pub width: u32,
    /// Helligkeitsschritt 0.0–1.0.
    pub amount: f64,
    pub highlight: bool,
    pub shadow: bool,
    /// Fehlt eine passende Palettenfarbe: freie Farbe berechnen.
    pub allow_free: bool,
}

impl Default for LightOpts {
    fn default() -> Self {
        LightOpts { width: 1, amount: 0.15, highlight: true, shadow: true, allow_free: false }
    }
}

const GRAY_SAT: f64 = 0.15; // darunter gilt eine Farbe als Grau (wie palops)
const HUE_FAMILY: f64 = 40.0; // so weit darf der Farbton einer Abstufung abweichen
const HUE_SHIFT: f64 = 12.0; // Licht wärmer, Schatten kühler

// ── Farb-Helfer ─────────────────────────────────────────────────────

/// (Farbton 0–360, Sättigung 0–1, Helligkeit 0–1)
fn hsl(c: Rgb) -> (f64, f64, f64) {
    let [r, g, b] = c.map(|v| v as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn from_hsl(h: f64, s: f64, l: f64) -> Rgb {
    let h = h.rem_euclid(360.0) / 360.0;
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return [v, v, v];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |t: f64| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    [f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0)]
}

fn hue_dist(a: f64, b: f64) -> f64 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 {
        360.0 - d
    } else {
        d
    }
}

fn dist2(a: Rgb, b: Rgb) -> f64 {
    (0..3).map(|i| (a[i] as f64 - b[i] as f64).powi(2)).sum()
}

/// Farbe um `amount` heller (`sign` +1) oder dunkler (-1). Licht wandert
/// leicht Richtung Gelb (60°), Schatten Richtung Blau (240°); Grau bleibt grau.
pub fn shift(c: Rgb, sign: i32, amount: f64) -> Rgb {
    let (mut h, s, l) = hsl(c);
    let l = (l + sign as f64 * amount).clamp(0.0, 1.0);
    if s >= GRAY_SAT {
        let goal = if sign > 0 { 60.0 } else { 240.0 };
        let d = (goal - h + 540.0).rem_euclid(360.0) - 180.0; // kürzester Weg
        h += d.signum() * d.abs().min(HUE_SHIFT);
    }
    from_hsl(h, s, l)
}

/// Hellere/dunklere Palettenfarbe derselben Familie, möglichst nah an der
/// Wunschfarbe. `None` = keine passende da.
pub fn palette_step(pal: &Palette, index: Px, sign: i32, amount: f64) -> Option<Px> {
    let base = pal.get(index)?;
    let (bh, bs, bl) = hsl(base);
    let goal = shift(base, sign, amount);
    let mut best: Option<(Px, f64)> = None;
    for (k, &c) in pal.colors.iter().enumerate() {
        let i = (k + 1) as Px;
        if i == index || c == base {
            continue;
        }
        let (h, s, l) = hsl(c);
        if if sign > 0 { l <= bl + 0.02 } else { l >= bl - 0.02 } {
            continue;
        }
        // Gleiche Familie: Grau zu Grau, Farbe zu ähnlichem Farbton. Fast
        // Weiß bzw. fast Schwarz passen zu allem.
        let extreme = if sign > 0 { l > 0.92 } else { l < 0.08 };
        let family = if bs < GRAY_SAT { s < GRAY_SAT } else { s >= GRAY_SAT && hue_dist(h, bh) <= HUE_FAMILY };
        if !family && !extreme {
            continue;
        }
        let d = dist2(c, goal) + if extreme && !family { 4000.0 } else { 0.0 };
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// Freie Farbe nachschlagen oder anhängen (wie `Sprite::free_color`).
fn free_px(free: &mut Vec<Rgb>, c: Rgb) -> Px {
    let i = free.iter().position(|&f| f == c).unwrap_or_else(|| {
        free.push(c);
        free.len() - 1
    });
    FREE_BASE + i as Px
}

// ── Kantenlicht ─────────────────────────────────────────────────────

/// Kanten zur Lampe hin aufhellen, abgewandte abdunkeln. `free` sind die
/// freien Farben des Sprites — neue werden angehängt. `inside` begrenzt auf
/// eine Auswahl. Gibt (heller, dunkler) zurück.
pub fn light(
    img: &mut Image,
    pal: &Palette,
    free: &mut Vec<Rgb>,
    dir: LightDir,
    opts: LightOpts,
    inside: impl Fn(i64, i64) -> bool,
) -> (usize, usize) {
    let (dx, dy) = (dir.0.signum() as i64, dir.1.signum() as i64);
    if dx == 0 && dy == 0 {
        return (0, 0);
    }
    let (w, h) = (img.width() as i64, img.height() as i64);
    let src = img.clone();
    let empty = |x: i64, y: i64| x < 0 || y < 0 || x >= w || y >= h || src.get(x as u32, y as u32) == 0;
    let width = opts.width.max(1) as i64;
    let open = |x: i64, y: i64, sx: i64, sy: i64| (1..=width).any(|k| empty(x + sx * k, y + sy * k));

    // Pro (Wert, Richtung) nur einmal rechnen — ein Sprite hat wenige Farben.
    let mut memo: HashMap<(Px, i32), Px> = HashMap::new();
    let (mut lit, mut shaded) = (0, 0);
    let filled: Vec<(u32, u32, Px)> = src.pixels().collect();
    for (x, y, v) in filled {
        let (xi, yi) = (x as i64, y as i64);
        if !inside(xi, yi) {
            continue;
        }
        let to_light = open(xi, yi, dx, dy);
        let away = open(xi, yi, -dx, -dy);
        if to_light == away {
            continue;
        }
        let sign = if to_light { 1 } else { -1 };
        if if sign > 0 { !opts.highlight } else { !opts.shadow } {
            continue;
        }
        let nv = *memo.entry((v, sign)).or_insert_with(|| {
            if v >= FREE_BASE {
                match rgb_of(v, pal, free) {
                    Some(c) => free_px(free, shift(c, sign, opts.amount)),
                    None => v,
                }
            } else if let Some(i) = palette_step(pal, v, sign, opts.amount) {
                i
            } else if let (true, Some(c)) = (opts.allow_free, pal.get(v)) {
                free_px(free, shift(c, sign, opts.amount))
            } else {
                v
            }
        });
        if nv == v {
            continue;
        }
        img.set(x, y, nv);
        if sign > 0 {
            lit += 1;
        } else {
            shaded += 1;
        }
    }
    (lit, shaded)
}

// ── Schlagschatten ──────────────────────────────────────────────────

/// Silhouette um 1..=`distance` Pixel von der Lampe weg versetzt in leere
/// Pixel malen. Gibt die Zahl gemalter Pixel.
pub fn drop_shadow(img: &mut Image, dir: LightDir, value: Px, distance: u32, inside: impl Fn(i64, i64) -> bool) -> usize {
    let (dx, dy) = (dir.0.signum() as i64, dir.1.signum() as i64);
    if (dx == 0 && dy == 0) || value == 0 {
        return 0;
    }
    let (w, h) = (img.width() as i64, img.height() as i64);
    let src = img.clone();
    let filled: Vec<(u32, u32, Px)> = src.pixels().collect();
    let mut n = 0;
    for (x, y, _) in filled {
        let (xi, yi) = (x as i64, y as i64);
        if !inside(xi, yi) {
            continue;
        }
        for k in 1..=distance.max(1) as i64 {
            // Von der Lampe weg: gegen die Lichtrichtung.
            let (tx, ty) = (xi - dx * k, yi - dy * k);
            if tx < 0 || ty < 0 || tx >= w || ty >= h || !inside(tx, ty) {
                continue;
            }
            if src.get(tx as u32, ty as u32) == 0 && img.get(tx as u32, ty as u32) != value {
                img.set(tx as u32, ty as u32, value);
                n += 1;
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    // Grün in drei Stufen (1–3), Grau, Weiß, Schwarz.
    fn pal() -> Palette {
        Palette::new(
            "t",
            vec![[0x2e, 0x7d, 0x32], [0x4c, 0xaf, 0x50], [0xa5, 0xd6, 0xa7], [0x80, 0x80, 0x80], [0xff, 0xff, 0xff], [0, 0, 0]],
        )
    }

    /// `w`×`h`-Block aus `v` mit 1 Pixel Rand.
    fn block(v: Px, w: u32, h: u32) -> Image {
        let mut img = Image::new(w + 2, h + 2);
        for y in 1..=h {
            for x in 1..=w {
                img.set(x, y, v);
            }
        }
        img
    }

    const TOP_LEFT: LightDir = (-1, -1);
    const ALL: fn(i64, i64) -> bool = |_, _| true;

    #[test]
    fn palette_familie() {
        let p = pal();
        assert_eq!(palette_step(&p, 2, 1, 0.15), Some(3));
        assert_eq!(palette_step(&p, 2, -1, 0.15), Some(1));
        assert_eq!(palette_step(&p, 4, 1, 0.15), Some(5), "Grau → Weiß, nicht Grün");
        assert_eq!(palette_step(&p, 4, -1, 0.15), Some(6));
        assert_eq!(palette_step(&p, 3, 1, 0.15), Some(5));
    }

    #[test]
    fn kanten_hell_und_dunkel() {
        let mut img = block(2, 5, 5);
        let mut free = vec![];
        let (lit, shaded) = light(&mut img, &pal(), &mut free, TOP_LEFT, LightOpts::default(), ALL);
        assert_eq!(img.get(1, 1), 3);
        assert_eq!(img.get(3, 1), 3);
        assert_eq!(img.get(1, 3), 3);
        assert_eq!(img.get(5, 5), 1);
        assert_eq!(img.get(3, 5), 1);
        assert_eq!(img.get(3, 3), 2, "Mitte bleibt");
        assert_eq!(img.get(0, 0), 0);
        assert!(lit > 0 && shaded > 0);
        assert!(free.is_empty());
    }

    #[test]
    fn breite_zwei() {
        let mut img = block(2, 6, 6);
        let opts = LightOpts { width: 2, ..Default::default() };
        light(&mut img, &pal(), &mut vec![], (0, -1), opts, ALL);
        assert_eq!(img.get(3, 1), 3);
        assert_eq!(img.get(3, 2), 3);
        assert_eq!(img.get(3, 3), 2);
        assert_eq!(img.get(3, 5), 1);
        assert_eq!(img.get(3, 6), 1);
    }

    #[test]
    fn linie_bleibt() {
        let mut img = Image::new(3, 3);
        img.set(1, 1, 2);
        assert_eq!(light(&mut img, &pal(), &mut vec![], TOP_LEFT, LightOpts::default(), ALL), (0, 0));
        assert_eq!(img.get(1, 1), 2);
    }

    #[test]
    fn nur_licht_oder_nur_schatten() {
        let mut a = block(2, 5, 5);
        light(&mut a, &pal(), &mut vec![], TOP_LEFT, LightOpts { shadow: false, ..Default::default() }, ALL);
        assert_eq!((a.get(1, 1), a.get(5, 5)), (3, 2));
        let mut b = block(2, 5, 5);
        light(&mut b, &pal(), &mut vec![], TOP_LEFT, LightOpts { highlight: false, ..Default::default() }, ALL);
        assert_eq!((b.get(1, 1), b.get(5, 5)), (2, 1));
    }

    #[test]
    fn ohne_passende_farbe_frei_nur_auf_wunsch() {
        let p = Palette::new("g", vec![[0x4c, 0xaf, 0x50]]);
        let mut a = block(1, 5, 5);
        light(&mut a, &p, &mut vec![], TOP_LEFT, LightOpts::default(), ALL);
        assert_eq!(a.get(1, 1), 1);
        let mut b = block(1, 5, 5);
        let mut free = vec![];
        light(&mut b, &p, &mut free, TOP_LEFT, LightOpts { allow_free: true, ..Default::default() }, ALL);
        assert!(b.get(1, 1) >= FREE_BASE);
        assert_eq!(free.len(), 2, "eine hellere, eine dunklere");
    }

    #[test]
    fn freie_farben_direkt() {
        let base = [0x4c, 0xaf, 0x50];
        let mut free = vec![base];
        let mut img = block(FREE_BASE, 5, 5);
        light(&mut img, &pal(), &mut free, TOP_LEFT, LightOpts { amount: 0.2, ..Default::default() }, ALL);
        let sum = |c: Rgb| c.iter().map(|&v| v as u32).sum::<u32>();
        let at = |x, y| free[(img.get(x, y) - FREE_BASE) as usize];
        assert!(sum(at(1, 1)) > sum(base));
        assert!(sum(at(5, 5)) < sum(base));
        assert_eq!(at(3, 3), base);
    }

    #[test]
    fn grau_bleibt_grau() {
        let c = shift([0x80, 0x80, 0x80], 1, 0.2);
        assert!(c[0] == c[1] && c[1] == c[2]);
    }

    #[test]
    fn nur_in_der_auswahl() {
        let mut img = block(2, 5, 5);
        light(&mut img, &pal(), &mut vec![], TOP_LEFT, LightOpts::default(), |x, _| x <= 3);
        assert_eq!(img.get(1, 1), 3);
        assert_eq!(img.get(5, 5), 2);
    }

    #[test]
    fn schlagschatten_von_der_lampe_weg() {
        let mut img = Image::new(4, 4);
        img.set(1, 1, 2);
        assert_eq!(drop_shadow(&mut img, TOP_LEFT, 6, 1, ALL), 1);
        assert_eq!(img.get(2, 2), 6);
        assert_eq!(img.get(1, 1), 2);
    }

    #[test]
    fn schlagschatten_abstand_zwei() {
        let mut img = Image::new(5, 5);
        img.set(1, 1, 2);
        drop_shadow(&mut img, (0, -1), 6, 2, ALL);
        assert_eq!((img.get(1, 2), img.get(1, 3), img.get(1, 4)), (6, 6, 0));
    }

    #[test]
    fn ohne_richtung_nichts() {
        let mut img = block(2, 5, 5);
        assert_eq!(light(&mut img, &pal(), &mut vec![], (0, 0), LightOpts::default(), ALL), (0, 0));
        assert_eq!(drop_shadow(&mut img, (0, 0), 6, 1, ALL), 0);
    }
}
