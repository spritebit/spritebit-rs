//! Werkzeug-Hilfen ohne Oberfläche: Linie, Pinsel-Stempel, Formen, Füllen,
//! Spray.
//!
//! Große Formen werden als **Zeilenabschnitte** beschrieben ([`Span`]), nicht
//! als einzelne Punkte: eine gefüllte Ellipse über 8192 px wären sonst
//! Dutzende Millionen Koordinaten.

use crate::image::{Image, Px};

/// Ein waagerechter Abschnitt: Zeile `y`, Spalten `x0..=x1`.
pub type Span = (i64, i64, i64);

/// Alle Pixel auf der Linie von (x0, y0) nach (x1, y1), beide Enden
/// eingeschlossen (Bresenham). Damit hat ein schneller Strich keine Lücken:
/// zwischen zwei Mausbewegungen wird die Strecke aufgefüllt.
pub fn line(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<(i64, i64)> {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    let mut out = Vec::with_capacity((dx.max(-dy) + 1) as usize);
    loop {
        out.push((x, y));
        if x == x1 && y == y1 {
            return out;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Quadratischer Pinsel der Kantenlänge `size` um (x, y). Bei gerader Größe
/// liegt der Mittelpunkt links oben von der Mitte — wie in der Web-Version.
pub fn stamp(x: i64, y: i64, size: u32) -> Span3 {
    let n = size.max(1) as i64;
    let lo = -(n - 1) / 2;
    let hi = lo + n - 1;
    Span3 { y0: y + lo, y1: y + hi, x0: x + lo, x1: x + hi }
}

/// Ein Rechteck aus Zeilen `y0..=y1` und Spalten `x0..=x1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span3 {
    pub x0: i64,
    pub y0: i64,
    pub x1: i64,
    pub y1: i64,
}

impl Span3 {
    pub fn spans(self) -> impl Iterator<Item = Span> {
        (self.y0..=self.y1).map(move |y| (y, self.x0, self.x1))
    }
}

/// Abschnitte setzen; was außerhalb des Bildes liegt, fällt weg.
pub fn fill_spans(img: &mut Image, spans: impl IntoIterator<Item = Span>, value: Px) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    for (y, a, b) in spans {
        if y < 0 || y >= h {
            continue;
        }
        for x in a.max(0)..=b.min(w - 1) {
            img.set(x as u32, y as u32, value);
        }
    }
}

fn ordered(a: i64, b: i64) -> (i64, i64) {
    (a.min(b), a.max(b))
}

/// Rechteck zwischen zwei Ecken (beide eingeschlossen).
pub fn rect_spans(x0: i64, y0: i64, x1: i64, y1: i64, filled: bool) -> Vec<Span> {
    let (xa, xb) = ordered(x0, x1);
    let (ya, yb) = ordered(y0, y1);
    let mut out = Vec::new();
    for y in ya..=yb {
        if filled || y == ya || y == yb || xb - xa < 2 {
            out.push((y, xa, xb));
        } else {
            out.push((y, xa, xa));
            out.push((y, xb, xb));
        }
    }
    out
}

/// Spalten einer gefüllten Ellipse in der Box (xa..=xb, ya..=yb) für Zeile y.
fn ellipse_row(xa: i64, xb: i64, ya: i64, yb: i64, y: i64) -> Option<(i64, i64)> {
    if y < ya || y > yb {
        return None;
    }
    let (cx, cy) = ((xa + xb + 1) as f64 / 2.0, (ya + yb + 1) as f64 / 2.0);
    let (rx, ry) = ((xb - xa + 1) as f64 / 2.0, (yb - ya + 1) as f64 / 2.0);
    let dy = (y as f64 + 0.5 - cy) / ry;
    let half = rx * (1.0 - dy * dy).max(0.0).sqrt();
    // Pixel gehört dazu, wenn seine Mitte in der Ellipse liegt.
    let a = (cx - half - 0.5).ceil() as i64;
    let b = (cx + half - 0.5).floor() as i64;
    if a <= b {
        Some((a.max(xa), b.min(xb)))
    } else {
        // Sehr flache Ellipse: wenigstens die Mitte.
        let m = (cx - 0.5).round() as i64;
        Some((m, m))
    }
}

/// Ellipse in der Box zwischen zwei Ecken. Umriss = die Pixel der gefüllten
/// Ellipse, die nicht ringsum von ihr umgeben sind.
pub fn ellipse_spans(x0: i64, y0: i64, x1: i64, y1: i64, filled: bool) -> Vec<Span> {
    let (xa, xb) = ordered(x0, x1);
    let (ya, yb) = ordered(y0, y1);
    let mut out = Vec::new();
    for y in ya..=yb {
        let Some((a, b)) = ellipse_row(xa, xb, ya, yb, y) else { continue };
        if filled {
            out.push((y, a, b));
            continue;
        }
        // Innen ist, was auch oben und unten zur Ellipse gehört — und
        // links/rechts einen Nachbarn in der eigenen Zeile hat.
        let up = ellipse_row(xa, xb, ya, yb, y - 1);
        let dn = ellipse_row(xa, xb, ya, yb, y + 1);
        let inner = match (up, dn) {
            (Some((ua, ub)), Some((da, db))) => {
                let ia = (a + 1).max(ua).max(da);
                let ib = (b - 1).min(ub).min(db);
                (ia <= ib).then_some((ia, ib))
            }
            _ => None,
        };
        match inner {
            None => out.push((y, a, b)),
            Some((ia, ib)) => {
                if a < ia {
                    out.push((y, a, ia - 1));
                }
                if ib < b {
                    out.push((y, ib + 1, b));
                }
            }
        }
    }
    out
}

// ── Umschalt beim Malen: nur waagerecht, senkrecht oder 45° (wie js/lock.js) ──

/// Ab so vielen Pixeln Abstand wird die Richtung festgelegt.
pub const LOCK_AFTER: i64 = 3;

/// Nächste der acht Richtungen für (dx, dy) als (-1/0/1, -1/0/1) — oder
/// `None`, solange der Zeiger noch zu nah am Start ist.
pub fn snap_dir(dx: i64, dy: i64) -> Option<(i64, i64)> {
    if dx.abs().max(dy.abs()) < LOCK_AFTER {
        return None;
    }
    let a = (dy as f64).atan2(dx as f64);
    let k = (a / std::f64::consts::FRAC_PI_4).round();
    let t = k * std::f64::consts::FRAC_PI_4;
    Some((t.cos().round() as i64, t.sin().round() as i64))
}

/// Punkt `p` auf die Linie durch `start` in Richtung `dir` legen.
pub fn project(start: (i64, i64), dir: (i64, i64), p: (i64, i64)) -> (i64, i64) {
    let (ux, uy) = dir;
    let n = (ux * ux + uy * uy) as f64;
    let t = (((p.0 - start.0) * ux + (p.1 - start.1) * uy) as f64 / n).round() as i64;
    (start.0 + t * ux, start.1 + t * uy)
}

/// Linie mit Umschalt: Endpunkt auf 0°, 45° oder 90° einrasten.
pub fn snap_end(start: (i64, i64), p: (i64, i64)) -> (i64, i64) {
    match snap_dir(p.0 - start.0, p.1 - start.1) {
        Some(d) => project(start, d, p),
        None => p,
    }
}

/// Rechteck und Ellipse mit Umschalt: gleich breit wie hoch.
pub fn square_end(start: (i64, i64), p: (i64, i64)) -> (i64, i64) {
    let (dx, dy) = (p.0 - start.0, p.1 - start.1);
    let d = dx.abs().max(dy.abs());
    let (sx, sy) = (if dx < 0 { -1 } else { 1 }, if dy < 0 { -1 } else { 1 });
    (start.0 + d * sx, start.1 + d * sy)
}

/// Füllen mit Grenzen aus einer Vorlage (wie `fillRegion` im Web, js/fill.js):
/// die zusammenhängende Fläche gleicher Werte in `key` (je Pixel ein Wert,
/// zeilenweise, z. B. die sichtbare Farbe aller Ebenen) bestimmt, was in
/// `img` gefüllt wird. Gibt zurück, wie viele Pixel sich geändert haben.
pub fn flood_fill_ref(img: &mut Image, key: &[u32], x: i64, y: i64, value: Px) -> usize {
    let (w, h) = (img.width() as i64, img.height() as i64);
    if x < 0 || y < 0 || x >= w || y >= h || key.len() != (w * h) as usize {
        return 0;
    }
    let target = key[(y * w + x) as usize];
    let mut seen = vec![false; key.len()];
    let mut stack = vec![(x, y)];
    let mut changed = 0;
    while let Some((cx, cy)) = stack.pop() {
        if cx < 0 || cy < 0 || cx >= w || cy >= h {
            continue;
        }
        let i = (cy * w + cx) as usize;
        if seen[i] || key[i] != target {
            continue;
        }
        seen[i] = true;
        if img.get(cx as u32, cy as u32) != value {
            img.set(cx as u32, cy as u32, value);
            changed += 1;
        }
        stack.extend([(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)]);
    }
    changed
}

/// Füllen: die zusammenhängende Fläche gleicher Farbe um (x, y) bekommt
/// `value` (4er-Nachbarschaft). Zeilenweise mit eigenem Stapel — keine
/// Rekursion, die bei großen Flächen den Stack sprengt. Gibt die Zahl der
/// geänderten Pixel zurück.
pub fn flood_fill(img: &mut Image, x: i64, y: i64, value: Px) -> usize {
    let (w, h) = (img.width() as i64, img.height() as i64);
    if x < 0 || y < 0 || x >= w || y >= h {
        return 0;
    }
    let target = img.get(x as u32, y as u32);
    if target == value {
        return 0;
    }
    let mut changed = 0;
    let mut stack = vec![(x, y)];
    while let Some((sx, sy)) = stack.pop() {
        if img.get(sx as u32, sy as u32) != target {
            continue;
        }
        // Nach links und rechts bis zur Grenze.
        let mut a = sx;
        while a > 0 && img.get((a - 1) as u32, sy as u32) == target {
            a -= 1;
        }
        let mut b = sx;
        while b < w - 1 && img.get((b + 1) as u32, sy as u32) == target {
            b += 1;
        }
        for cx in a..=b {
            img.set(cx as u32, sy as u32, value);
        }
        changed += (b - a + 1) as usize;
        // Darüber und darunter: je Abschnitt gleicher Farbe ein Startpunkt.
        for ny in [sy - 1, sy + 1] {
            if ny < 0 || ny >= h {
                continue;
            }
            let mut in_run = false;
            for cx in a..=b {
                let same = img.get(cx as u32, ny as u32) == target;
                if same && !in_run {
                    stack.push((cx, ny));
                }
                in_run = same;
            }
        }
    }
    changed
}

/// Symmetrie: zu jedem Abschnitt die Spiegelbilder an der senkrechten
/// (`mx`, links ↔ rechts) und/oder waagerechten Mittelachse (`my`) einer
/// Fläche `w` × `h` dazu.
pub fn mirror_spans(spans: Vec<Span>, w: u32, h: u32, mx: bool, my: bool) -> Vec<Span> {
    let (w, h) = (w as i64, h as i64);
    let mut out = Vec::with_capacity(spans.len() * (1 + mx as usize) * (1 + my as usize));
    for (y, a, b) in spans {
        out.push((y, a, b));
        if mx {
            out.push((y, w - 1 - b, w - 1 - a));
        }
        if my {
            out.push((h - 1 - y, a, b));
        }
        if mx && my {
            out.push((h - 1 - y, w - 1 - b, w - 1 - a));
        }
    }
    out
}

/// Kleiner Zufallsgenerator (xorshift) — für Spray reicht das, und es
/// braucht keine Abhängigkeit.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    pub fn next_f64(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// `count` zufällige Punkte gleichmäßig verteilt im Kreis mit Radius `r`.
pub fn spray(x: i64, y: i64, r: f64, count: usize, rng: &mut Rng) -> Vec<(i64, i64)> {
    (0..count)
        .map(|_| {
            let a = rng.next_f64() * std::f64::consts::TAU;
            let d = r * rng.next_f64().sqrt();
            (x + (d * a.cos()).round() as i64, y + (d * a.sin()).round() as i64)
        })
        .collect()
}

/// Pixel-perfect: beim freihändigen Zeichnen mit 1 Pixel
/// entstehen an Treppenstufen L-Ecken — zwei Pixel, wo die Linie nur eins
/// braucht. Bilden die letzten drei Punkte so eine Ecke (erster und letzter
/// diagonal benachbart, der mittlere waagerecht/senkrecht neben beiden),
/// bekommt der mittlere seinen alten Wert zurück und fällt aus dem Pfad.
///
/// Ein Strich = ein `PixelPerfect`. Symmetrie wird mitgespiegelt.
#[derive(Debug, Default)]
pub struct PixelPerfect {
    path: Vec<(i64, i64)>,
    /// Wert jedes im Strich berührten Pixels vor dem Strich.
    before: std::collections::HashMap<(i64, i64), Px>,
}

impl PixelPerfect {
    pub fn new() -> Self {
        Self::default()
    }

    /// Punkt (x, y) mit `value` malen; gibt `true` zurück, wenn sich etwas
    /// geändert hat. `mx`/`my`: Symmetrie an der senkrechten/waagerechten Mitte.
    pub fn add(&mut self, img: &mut Image, x: i64, y: i64, value: Px, mx: bool, my: bool) -> bool {
        if self.path.last() == Some(&(x, y)) {
            return false;
        }
        let (w, h) = (img.width(), img.height());
        let mut changed = false;
        for (py, px, _) in mirror_spans(vec![(y, x, x)], w, h, mx, my) {
            if px < 0 || py < 0 || px >= w as i64 || py >= h as i64 {
                continue;
            }
            let old = img.get(px as u32, py as u32);
            self.before.entry((px, py)).or_insert(old);
            if old != value {
                img.set(px as u32, py as u32, value);
                changed = true;
            }
        }
        self.path.push((x, y));
        let n = self.path.len();
        if n >= 3 {
            let (a, b, c) = (self.path[n - 3], self.path[n - 2], self.path[n - 1]);
            let diagonal = (a.0 - c.0).abs() == 1 && (a.1 - c.1).abs() == 1;
            let corner = (b.0 == a.0 || b.1 == a.1) && (b.0 == c.0 || b.1 == c.1);
            if diagonal && corner {
                self.path.remove(n - 2);
                // Liegt der Punkt noch woanders im Pfad, bleibt er gemalt.
                if !self.path.contains(&b) {
                    for (py, px, _) in mirror_spans(vec![(b.1, b.0, b.0)], w, h, mx, my) {
                        if let Some(&old) = self.before.get(&(px, py)) {
                            if img.get(px as u32, py as u32) != old {
                                img.set(px as u32, py as u32, old);
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn umschalt_rastet_wie_im_web_ein() {
        assert_eq!(snap_dir(0, 0), None);
        assert_eq!(snap_dir(2, 1), None, "erst ab 3 Pixeln");
        assert_eq!(snap_dir(5, 1), Some((1, 0)));
        assert_eq!(snap_dir(-1, -6), Some((0, -1)));
        assert_eq!(snap_dir(4, 5), Some((1, 1)));
        assert_eq!(snap_dir(-3, 3), Some((-1, 1)));
        let s = (10, 10);
        assert_eq!(project(s, (1, 0), (17, 13)), (17, 10));
        assert_eq!(project(s, (0, 1), (12, 4)), (10, 4));
        assert_eq!(project(s, (1, 1), (15, 13)), (14, 14));
        assert_eq!(snap_end((0, 0), (9, 2)), (9, 0));
        assert_eq!(snap_end((0, 0), (6, 7)), (7, 7));
        assert_eq!(snap_end((0, 0), (2, 1)), (2, 1));
        assert_eq!(square_end((5, 5), (9, 7)), (9, 9));
        assert_eq!(square_end((5, 5), (2, 7)), (2, 8));
    }

    #[test]
    fn fuellen_mit_grenzen_aus_einer_vorlage() {
        // Vorlage 4×3: eine Linie (7) trennt links oben ab.
        let key: Vec<u32> = vec![0, 0, 7, 0, 0, 0, 7, 0, 7, 7, 7, 0];
        let mut img = Image::new(4, 3);
        assert_eq!(flood_fill_ref(&mut img, &key, 0, 0, 3), 4);
        assert_eq!((img.get(1, 1), img.get(3, 0), img.get(0, 2)), (3, 0, 0));
        // Hat das Bild dort schon die Farbe, wird trotzdem weitergesucht.
        let mut img = Image::new(3, 1);
        img.set(0, 0, 4);
        assert_eq!(flood_fill_ref(&mut img, &[0, 0, 0], 0, 0, 4), 2);
        assert_eq!(flood_fill_ref(&mut img, &[0, 0], 0, 0, 4), 0, "falsche Länge");
    }

    use super::*;

    fn painted(spans: &[Span]) -> std::collections::BTreeSet<(i64, i64)> {
        spans.iter().flat_map(|&(y, a, b)| (a..=b).map(move |x| (x, y))).collect()
    }

    #[test]
    fn ein_punkt() {
        assert_eq!(line(3, 3, 3, 3), vec![(3, 3)]);
    }

    #[test]
    fn waagerecht_und_rueckwaerts() {
        assert_eq!(line(0, 0, 3, 0), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(line(3, 0, 0, 0), vec![(3, 0), (2, 0), (1, 0), (0, 0)]);
    }

    #[test]
    fn diagonal_ohne_luecken() {
        let l = line(0, 0, 5, 2);
        assert_eq!(l.first(), Some(&(0, 0)));
        assert_eq!(l.last(), Some(&(5, 2)));
        for w in l.windows(2) {
            assert!((w[1].0 - w[0].0).abs() <= 1 && (w[1].1 - w[0].1).abs() <= 1);
        }
        assert_eq!(l.len(), 6);
    }

    #[test]
    fn pinsel_stempel() {
        assert_eq!(stamp(5, 5, 1), Span3 { x0: 5, y0: 5, x1: 5, y1: 5 });
        assert_eq!(stamp(5, 5, 3), Span3 { x0: 4, y0: 4, x1: 6, y1: 6 });
        assert_eq!(stamp(5, 5, 2), Span3 { x0: 5, y0: 5, x1: 6, y1: 6 });
        assert_eq!(stamp(5, 5, 3).spans().count(), 3);
    }

    #[test]
    fn rechteck_umriss_und_gefuellt() {
        let o = painted(&rect_spans(4, 4, 0, 0, false));
        assert_eq!(o.len(), 16, "5×5-Umriss");
        assert!(!o.contains(&(2, 2)));
        assert_eq!(painted(&rect_spans(0, 0, 4, 4, true)).len(), 25);
    }

    #[test]
    fn ellipse_symmetrisch_und_umriss_ohne_innen() {
        let f = painted(&ellipse_spans(0, 0, 10, 6, true));
        for &(x, y) in &f {
            assert!(f.contains(&(10 - x, y)) && f.contains(&(x, 6 - y)), "symmetrisch");
        }
        let o = painted(&ellipse_spans(0, 0, 10, 6, false));
        assert!(o.is_subset(&f));
        assert!(!o.contains(&(5, 3)), "Mitte ist innen");
        assert!(o.contains(&(0, 3)) && o.contains(&(10, 3)) && o.contains(&(5, 0)));
    }

    #[test]
    fn ellipse_umriss_hat_keine_luecken() {
        let o = painted(&ellipse_spans(0, 0, 30, 18, false));
        // Jeder Umriss-Pixel hat mindestens zwei Nachbarn (auch diagonal) im Umriss.
        for &(x, y) in &o {
            let n = (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| (dx, dy))).filter(|&(dx, dy)| (dx, dy) != (0, 0) && o.contains(&(x + dx, y + dy))).count();
            assert!(n >= 2, "Lücke bei ({x}, {y})");
        }
    }

    #[test]
    fn winzige_ellipse() {
        assert_eq!(painted(&ellipse_spans(3, 3, 3, 3, true)).len(), 1);
        assert!(!painted(&ellipse_spans(0, 0, 0, 5, false)).is_empty());
    }

    #[test]
    fn fuellen_bleibt_in_der_flaeche() {
        let mut img = Image::new(10, 10);
        fill_spans(&mut img, rect_spans(2, 2, 7, 7, false), 1);
        assert_eq!(flood_fill(&mut img, 4, 4, 3), 16, "4×4 innen");
        assert_eq!(img.get(0, 0), 0, "außen unberührt");
        assert_eq!(img.get(2, 2), 1, "Rand unberührt");
        assert_eq!(flood_fill(&mut img, 4, 4, 3), 0, "schon gefüllt");
    }

    #[test]
    fn fuellen_einer_grossen_flaeche_ohne_stapelueberlauf() {
        let mut img = Image::new(2048, 2048);
        assert_eq!(flood_fill(&mut img, 0, 0, 1), 2048 * 2048);
        assert_eq!(img.get(2047, 2047), 1);
    }

    #[test]
    fn spray_bleibt_im_kreis() {
        let mut rng = Rng::new(42);
        let pts = spray(50, 50, 5.0, 200, &mut rng);
        assert_eq!(pts.len(), 200);
        assert!(pts.iter().all(|&(x, y)| ((x - 50).pow(2) + (y - 50).pow(2)) as f64 <= 6.0 * 6.0));
    }

    /// Pixel-perfect: eine Treppe ohne Doppel.
    #[test]
    fn pixel_perfect_entfernt_l_ecken() {
        let mut img = Image::new(6, 6);
        img.set(1, 0, 7); // war schon da — muss zurückkommen
        let mut pp = PixelPerfect::new();
        // Freihand-Treppe: rechts, runter, rechts, runter …
        for (x, y) in [(0, 0), (1, 0), (1, 1), (2, 1), (2, 2), (3, 2)] {
            pp.add(&mut img, x, y, 3, false, false);
        }
        let painted: Vec<(u32, u32)> = (0..6).flat_map(|y| (0..6).map(move |x| (x, y))).filter(|&(x, y)| img.get(x, y) == 3).collect();
        assert_eq!(painted, vec![(0, 0), (1, 1), (2, 2), (3, 2)], "eine saubere Diagonale");
        assert_eq!(img.get(1, 0), 7, "Eckpixel bekommt seinen alten Wert");
        // Gerade Linien bleiben unberührt.
        let mut img = Image::new(6, 2);
        let mut pp = PixelPerfect::new();
        for x in 0..6 {
            pp.add(&mut img, x, 0, 1, false, false);
        }
        assert!((0..6).all(|x| img.get(x, 0) == 1));
    }

    #[test]
    fn pixel_perfect_mit_symmetrie() {
        let mut img = Image::new(8, 4);
        let mut pp = PixelPerfect::new();
        for (x, y) in [(0, 0), (1, 0), (1, 1)] {
            pp.add(&mut img, x, y, 2, true, false);
        }
        assert_eq!(img.get(1, 0), 0);
        assert_eq!(img.get(6, 0), 0, "Spiegelbild der Ecke auch weg");
        assert_eq!(img.get(6, 1), 2);
    }

    #[test]
    fn spiegeln_an_beiden_achsen() {
        let m = mirror_spans(vec![(1, 0, 2)], 10, 8, true, true);
        assert_eq!(m, vec![(1, 0, 2), (1, 7, 9), (6, 0, 2), (6, 7, 9)]);
        assert_eq!(mirror_spans(vec![(1, 0, 2)], 10, 8, false, false), vec![(1, 0, 2)]);
    }

    #[test]
    fn spans_ausserhalb_werden_abgeschnitten() {
        let mut img = Image::new(4, 4);
        fill_spans(&mut img, [(-1, 0, 3), (2, -5, 10)], 2);
        assert_eq!(img.get(0, 2), 2);
        assert_eq!(img.get(3, 2), 2);
        assert_eq!(img.get(0, 0), 0);
    }
}
