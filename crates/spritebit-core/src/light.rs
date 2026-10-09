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

// ── Licht und Schatten als eigene Ebene ─────────────────────────────
//
// Nicht-destruktiv wie in der Web-Version (`light.js`): das Original
// bleibt, die Effekt-Ebene hält nur die Pixel, die das Licht ändert
// (Licht-Ebene über der Figur) bzw. die der Schatten belegt (Schatten-
// Ebene unter ihr). Die Einstellungen stehen in [`Layer::fx`]; ändert man
// sie, wird die Ebene aus dem Original neu berechnet.
//
// Eine Licht-Ebene gehört zur nächsten normalen Ebene darunter, eine
// Schatten-Ebene zur nächsten normalen darüber.

use crate::sprite::{Layer, Sprite};
use serde_json::{json, Value};

/// Farbe des Schlagschattens: Palettennummer oder feste Farbe (wird beim
/// Berechnen zur Palettenfarbe, wenn es sie gibt, sonst zur freien Farbe).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FxColor {
    Index(Px),
    Rgb(Rgb),
}

#[derive(Clone, Debug, PartialEq)]
pub enum FxKind {
    Light(LightOpts),
    Shadow { color: FxColor, distance: u32 },
}

/// Effekt einer Ebene — `src` ist die Prüfsumme der Quell-Zellen beim
/// letzten Berechnen (wie im Web gerechnet, damit beide Versionen dieselbe
/// Datei als aktuell erkennen).
#[derive(Clone, Debug, PartialEq)]
pub struct LayerFx {
    pub kind: FxKind,
    pub dir: LightDir,
    pub src: String,
}

impl LayerFx {
    pub fn is_light(&self) -> bool {
        matches!(self.kind, FxKind::Light(_))
    }

    /// Wie `layer.fx` im Web-Projekt.
    pub fn to_json(&self) -> Value {
        let dir = json!({ "dx": self.dir.0, "dy": self.dir.1 });
        match &self.kind {
            FxKind::Light(o) => json!({
                "kind": "light", "dir": dir, "src": self.src,
                "width": o.width, "amount": o.amount,
                "highlight": o.highlight, "shadow": o.shadow, "allowHex": o.allow_free,
            }),
            FxKind::Shadow { color, distance } => json!({
                "kind": "shadow", "dir": dir, "src": self.src, "distance": distance,
                "color": match color {
                    FxColor::Index(i) => json!(i),
                    FxColor::Rgb(c) => json!(format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])),
                },
            }),
        }
    }

    /// Aus fremden Daten — Werte begrenzt wie `normalizeFx` im Web.
    pub fn from_json(v: &Value) -> Option<LayerFx> {
        let kind = v.get("kind")?.as_str()?;
        let comp = |k: &str| v.get("dir").and_then(|d| d.get(k)).and_then(Value::as_i64).filter(|n| (-1..=1).contains(n)).unwrap_or(-1) as i32;
        let (dx, dy) = (comp("dx"), comp("dy"));
        let dir = if dx == 0 && dy == 0 { (-1, -1) } else { (dx, dy) };
        let src = v.get("src").and_then(Value::as_str).unwrap_or("").to_string();
        let int = |k: &str, lo: u32, hi: u32, d: u32| v.get(k).and_then(Value::as_u64).map_or(d, |n| (n as u32).clamp(lo, hi));
        let kind = match kind {
            "light" => FxKind::Light(LightOpts {
                width: int("width", 1, 3, 1),
                amount: v.get("amount").and_then(Value::as_f64).map_or(0.15, |a| a.clamp(0.01, 1.0)),
                highlight: v.get("highlight").and_then(Value::as_bool) != Some(false),
                shadow: v.get("shadow").and_then(Value::as_bool) != Some(false),
                allow_free: v.get("allowHex").and_then(Value::as_bool) == Some(true),
            }),
            "shadow" => {
                let color = match v.get("color") {
                    Some(Value::Number(n)) => n.as_u64().map_or(FxColor::Rgb([0x1a; 3]), |i| FxColor::Index(i as Px)),
                    Some(Value::String(s)) => crate::palette::parse_hex(s).map_or(FxColor::Rgb([0x1a; 3]), FxColor::Rgb),
                    _ => FxColor::Rgb([0x1a; 3]),
                };
                FxKind::Shadow { color, distance: int("distance", 1, 3, 1) }
            }
            _ => return None,
        };
        Some(LayerFx { kind, dir, src })
    }
}

/// Zu welcher Ebene gehört die Effekt-Ebene `i`? Licht: nächste normale
/// darunter, Schatten: nächste normale darüber.
pub fn fx_source(layers: &[Layer], i: usize) -> Option<usize> {
    let light = layers.get(i)?.fx.as_ref()?.is_light();
    if light {
        (0..i).rev().find(|&j| layers[j].fx.is_none())
    } else {
        (i + 1..layers.len()).find(|&j| layers[j].fx.is_none())
    }
}

/// Licht- (`light = true`) bzw. Schatten-Ebene zur Ebene `base`.
pub fn fx_for(layers: &[Layer], base: usize, light: bool) -> Option<usize> {
    (0..layers.len()).find(|&i| layers[i].fx.as_ref().is_some_and(|f| f.is_light() == light) && fx_source(layers, i) == Some(base))
}

/// Prüfsumme über eine Ebene in allen Frames — genau wie `celsHash` im
/// Web (FNV-1a über die Pixel als Text: Nummer, „#rrggbb“ oder leer).
pub fn cels_hash(sp: &Sprite, layer: usize) -> String {
    let mut h: u32 = 2166136261;
    let mut mix = |s: &str| {
        for b in s.bytes() {
            h = (h ^ b as u32).wrapping_mul(16777619);
        }
    };
    for f in 0..sp.frames.len() {
        let img = sp.cel(f, layer);
        mix(&format!("{}x{};", sp.height, sp.width));
        for y in 0..sp.height {
            for x in 0..sp.width {
                let v = img.get(x, y);
                if v == 0 {
                    mix(",");
                } else if v >= FREE_BASE {
                    match sp.free.get((v - FREE_BASE) as usize) {
                        Some(c) => mix(&format!("#{:02x}{:02x}{:02x},", c[0], c[1], c[2])),
                        None => mix(","),
                    }
                } else {
                    mix(&format!("{v},"));
                }
            }
        }
    }
    base36(h)
}

fn base36(mut n: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    while n > 0 {
        out.push(digits[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).expect("ASCII")
}

/// Effekt-Ebene `i` in allen Frames neu berechnen. Verknüpfte Quell-Zellen
/// ergeben dieselbe Effekt-Zelle. `false` = keine Quelle.
pub fn recompute_fx(sp: &mut Sprite, i: usize, pal: &Palette) -> bool {
    let (Some(fx), Some(src)) = (sp.layers.get(i).and_then(|l| l.fx.clone()), fx_source(&sp.layers, i)) else { return false };
    let shadow_value = match fx.kind {
        FxKind::Shadow { color: FxColor::Index(i), .. } => Some(i),
        FxKind::Shadow { color: FxColor::Rgb(c), .. } => Some(match pal.colors.iter().position(|&p| p == c) {
            Some(k) => k as Px + 1,
            None => sp.free_color(c),
        }),
        FxKind::Light(_) => None,
    };
    let mut memo: Vec<(usize, usize)> = Vec::new();
    for f in 0..sp.frames.len() {
        let base_id = sp.frames[f].cels[src];
        let id = match memo.iter().find(|(b, _)| *b == base_id) {
            Some(&(_, id)) => id,
            None => {
                let base = sp.images[base_id].clone();
                let mut copy = base.clone();
                let mut out = Image::new(base.width(), base.height());
                match &fx.kind {
                    FxKind::Light(o) => {
                        let mut free = std::mem::take(&mut sp.free);
                        light(&mut copy, pal, &mut free, fx.dir, *o, |_, _| true);
                        sp.free = free;
                        for (x, y, v) in copy.pixels() {
                            if v != base.get(x, y) {
                                out.set(x, y, v);
                            }
                        }
                    }
                    FxKind::Shadow { distance, .. } => {
                        drop_shadow(&mut copy, fx.dir, shadow_value.unwrap_or(0), *distance, |_, _| true);
                        for (x, y, v) in copy.pixels() {
                            if base.get(x, y) == 0 {
                                out.set(x, y, v);
                            }
                        }
                    }
                }
                sp.images.push(out);
                let id = sp.images.len() - 1;
                memo.push((base_id, id));
                id
            }
        };
        sp.frames[f].cels[i] = id;
    }
    let hash = cels_hash(sp, src);
    if let Some(fx) = sp.layers[i].fx.as_mut() {
        fx.src = hash;
    }
    true
}

/// Wurde an der Figur seit dem Berechnen weitergemalt?
pub fn fx_stale(sp: &Sprite, i: usize) -> bool {
    match (sp.layers.get(i).and_then(|l| l.fx.as_ref()), fx_source(&sp.layers, i)) {
        (Some(fx), Some(src)) => fx.src != cels_hash(sp, src),
        _ => false,
    }
}

/// Effekt-Ebene zur Ebene `base` anlegen (oder, wenn es sie gibt, mit
/// neuen Einstellungen versehen) und berechnen. Gibt ihre Stelle zurück.
/// Die aktive Ebene bleibt dieselbe (ihr Index rückt nach, wenn darunter
/// eingefügt wird).
pub fn upsert_fx(sp: &mut Sprite, base: usize, fx: LayerFx, name: impl Into<String>, pal: &Palette) -> usize {
    let light = fx.is_light();
    let idx = match fx_for(&sp.layers, base, light) {
        Some(i) => {
            sp.layers[i].fx = Some(fx);
            i
        }
        None => {
            let at = if light { base + 1 } else { base };
            let active = sp.layer;
            sp.add_layer(at, name);
            sp.layers[at].locked = true;
            sp.layers[at].fx = Some(fx);
            sp.layer = if active >= at { active + 1 } else { active };
            at
        }
    };
    recompute_fx(sp, idx, pal);
    idx
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

    // ── Effekt-Ebenen ──────────────────────────────────────────────

    fn sprite_with_block() -> Sprite {
        let mut sp = Sprite::new("t", 7, 7).unwrap();
        for y in 1..=5 {
            for x in 1..=5 {
                sp.active().set(x, y, 2);
            }
        }
        sp
    }

    fn light_fx(dir: LightDir) -> LayerFx {
        LayerFx { kind: FxKind::Light(LightOpts::default()), dir, src: String::new() }
    }

    #[test]
    fn licht_ebene_ueber_der_figur_original_bleibt() {
        let mut sp = sprite_with_block();
        let before = sp.cel(0, 0).clone();
        let i = upsert_fx(&mut sp, 0, light_fx(TOP_LEFT), "Licht", &pal());
        assert_eq!(i, 1);
        assert_eq!(sp.layers.len(), 2);
        assert!(sp.layers[1].locked);
        assert!(sp.cel(0, 0).same_pixels(&before), "Original unverändert");
        assert_eq!(sp.cel(0, 1).get(1, 1), 3, "Lichtkante");
        assert_eq!(sp.cel(0, 1).get(5, 5), 1, "Schattenkante");
        assert_eq!(sp.cel(0, 1).get(3, 3), 0, "unveränderte Pixel leer");
        assert_eq!(sp.layer, 0, "aktive Ebene bleibt die Figur");
    }

    #[test]
    fn neue_richtung_ersetzt_das_alte_licht() {
        let mut sp = sprite_with_block();
        upsert_fx(&mut sp, 0, light_fx(TOP_LEFT), "Licht", &pal());
        let i = upsert_fx(&mut sp, 0, light_fx((1, 1)), "Licht", &pal());
        assert_eq!(sp.layers.len(), 2, "keine zweite Licht-Ebene");
        assert_eq!(sp.cel(0, i).get(1, 1), 1, "oben links jetzt dunkel");
        assert_eq!(sp.cel(0, i).get(5, 5), 3, "unten rechts jetzt hell");
    }

    #[test]
    fn schatten_ebene_unter_der_figur() {
        let mut sp = Sprite::new("t", 5, 5).unwrap();
        sp.active().set(1, 1, 2);
        let fx = LayerFx { kind: FxKind::Shadow { color: FxColor::Index(6), distance: 1 }, dir: TOP_LEFT, src: String::new() };
        let i = upsert_fx(&mut sp, 0, fx, "Schatten", &pal());
        assert_eq!(i, 0);
        assert_eq!(sp.layer, 1, "die Figur ist nach oben gerückt und bleibt aktiv");
        assert_eq!(sp.cel(0, 0).get(2, 2), 6);
        assert_eq!(sp.cel(0, 0).get(1, 1), 0);
        assert_eq!(fx_source(&sp.layers, 0), Some(1));
    }

    #[test]
    fn weitermalen_macht_es_veraltet() {
        let mut sp = sprite_with_block();
        let i = upsert_fx(&mut sp, 0, light_fx(TOP_LEFT), "Licht", &pal());
        assert!(!fx_stale(&sp, i));
        sp.cel_mut(0, 0).set(3, 3, 4);
        assert!(fx_stale(&sp, i));
        recompute_fx(&mut sp, i, &pal());
        assert!(!fx_stale(&sp, i));
    }

    #[test]
    fn pruefsumme_wie_im_web() {
        // Web: celsHash([[[0, 2], ["#ff0000", 0]]]) — von Hand mit Node gerechnet.
        let mut sp = Sprite::new("t", 2, 2).unwrap();
        sp.active().set(1, 0, 2);
        let red = sp.free_color([255, 0, 0]);
        sp.active().set(0, 1, red);
        assert_eq!(cels_hash(&sp, 0), "1pan53j");
    }

    #[test]
    fn fx_als_json_hin_und_zurueck() {
        let a = LayerFx { kind: FxKind::Shadow { color: FxColor::Rgb([0x12, 0x34, 0x56]), distance: 2 }, dir: (1, -1), src: "abc".into() };
        assert_eq!(LayerFx::from_json(&a.to_json()), Some(a));
        let b = light_fx((0, 1));
        assert_eq!(LayerFx::from_json(&b.to_json()), Some(b));
        assert_eq!(LayerFx::from_json(&json!({ "kind": "blur" })), None);
    }
}
