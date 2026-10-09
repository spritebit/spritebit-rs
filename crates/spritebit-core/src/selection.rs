//! Auswahl: Rechteck, Lasso, nach Farbe — und was man damit tut
//! (kopieren, leeren, einfügen, verschieben).
//!
//! Eine Auswahl ist ein Rechteck mit optionaler Maske. Ohne Maske gehört
//! das ganze Rechteck dazu (Rechteck-Auswahl); mit Maske nur die markierten
//! Stellen (Lasso, Farbwahl).

use crate::image::{Image, Px, FREE_BASE};
use crate::palette::{Palette, Rgb};

#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    pub x: i64,
    pub y: i64,
    pub w: u32,
    pub h: u32,
    /// `w * h` Einträge, zeilenweise; `None` = ganzes Rechteck.
    pub mask: Option<Vec<bool>>,
}

/// Ausgeschnittene oder kopierte Pixel; 0 = gehört nicht dazu / transparent.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub w: u32,
    pub h: u32,
    pub data: Vec<Px>,
}

impl Selection {
    /// Rechteck zwischen zwei Ecken (beide eingeschlossen).
    pub fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Selection {
        let (xa, xb) = (x0.min(x1), x0.max(x1));
        let (ya, yb) = (y0.min(y1), y0.max(y1));
        Selection { x: xa, y: ya, w: (xb - xa + 1) as u32, h: (yb - ya + 1) as u32, mask: None }
    }

    /// Alles.
    pub fn all(width: u32, height: u32) -> Selection {
        Selection { x: 0, y: 0, w: width, h: height, mask: None }
    }

    /// Lasso: die Fläche innerhalb des Polygons durch `points` (wird
    /// geschlossen). Ein Pixel gehört dazu, wenn seine Mitte innen liegt.
    pub fn lasso(points: &[(i64, i64)]) -> Option<Selection> {
        if points.len() < 3 {
            return None;
        }
        let (xa, xb) = (points.iter().map(|p| p.0).min()?, points.iter().map(|p| p.0).max()?);
        let (ya, yb) = (points.iter().map(|p| p.1).min()?, points.iter().map(|p| p.1).max()?);
        let (w, h) = ((xb - xa + 1) as u32, (yb - ya + 1) as u32);
        let mut mask = vec![false; (w * h) as usize];
        let pts: Vec<(f64, f64)> = points.iter().map(|&(x, y)| (x as f64 + 0.5, y as f64 + 0.5)).collect();
        for row in 0..h {
            let py = ya as f64 + row as f64 + 0.5;
            // Schnittpunkte der Zeile mit den Kanten (gerade/ungerade).
            let mut xs: Vec<f64> = Vec::new();
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                if (a.1 <= py && b.1 > py) || (b.1 <= py && a.1 > py) {
                    xs.push(a.0 + (py - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
            }
            xs.sort_by(|a, b| a.total_cmp(b));
            for pair in xs.as_chunks::<2>().0 {
                for col in 0..w {
                    let px = xa as f64 + col as f64 + 0.5;
                    if px >= pair[0] && px <= pair[1] {
                        mask[(row * w + col) as usize] = true;
                    }
                }
            }
        }
        // Auch der gezeichnete Umriss selbst gehört dazu.
        for &(x, y) in points {
            mask[((y - ya) as u32 * w + (x - xa) as u32) as usize] = true;
        }
        Some(Selection { x: xa, y: ya, w, h, mask: Some(mask) }.trimmed())
    }

    /// Gehört Pixel (x, y) dazu?
    pub fn contains(&self, x: i64, y: i64) -> bool {
        if x < self.x || y < self.y || x >= self.x + self.w as i64 || y >= self.y + self.h as i64 {
            return false;
        }
        match &self.mask {
            None => true,
            Some(m) => m[((y - self.y) as u32 * self.w + (x - self.x) as u32) as usize],
        }
    }

    /// Auf die markierten Stellen verkleinern (Rechteck um die Maske).
    fn trimmed(self) -> Selection {
        let Some(m) = &self.mask else { return self };
        let mut b: Option<(u32, u32, u32, u32)> = None;
        for y in 0..self.h {
            for x in 0..self.w {
                if m[(y * self.w + x) as usize] {
                    b = Some(match b {
                        None => (x, y, x, y),
                        Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
                    });
                }
            }
        }
        let Some((x0, y0, x1, y1)) = b else { return self };
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut mask = vec![false; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                mask[(y * w + x) as usize] = m[((y + y0) * self.w + x + x0) as usize];
            }
        }
        Selection { x: self.x + x0 as i64, y: self.y + y0 as i64, w, h, mask: Some(mask) }
    }

    /// Verschoben um (dx, dy).
    pub fn moved(&self, dx: i64, dy: i64) -> Selection {
        Selection { x: self.x + dx, y: self.y + dy, ..self.clone() }
    }
}

/// Die acht Anfasser einer Auswahl (wie js/scale.js).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
}

impl Handle {
    pub const ALL: [Handle; 8] = [Handle::Nw, Handle::N, Handle::Ne, Handle::E, Handle::Se, Handle::S, Handle::Sw, Handle::W];

    fn west(self) -> bool {
        matches!(self, Handle::Nw | Handle::W | Handle::Sw)
    }
    fn east(self) -> bool {
        matches!(self, Handle::Ne | Handle::E | Handle::Se)
    }
    fn north(self) -> bool {
        matches!(self, Handle::Nw | Handle::N | Handle::Ne)
    }
    fn south(self) -> bool {
        matches!(self, Handle::Sw | Handle::S | Handle::Se)
    }
    fn corner(self) -> bool {
        matches!(self, Handle::Nw | Handle::Ne | Handle::Se | Handle::Sw)
    }

    /// Lage am Rechteck (x, y, w, h) — in Zellen, auf den Zellkanten.
    pub fn pos(self, r: (i64, i64, u32, u32)) -> (f32, f32) {
        let (x, y, w, h) = (r.0 as f32, r.1 as f32, r.2 as f32, r.3 as f32);
        let px = if self.west() { x } else if self.east() { x + w } else { x + w / 2.0 };
        let py = if self.north() { y } else if self.south() { y + h } else { y + h / 2.0 };
        (px, py)
    }

    /// Neues Rechteck, wenn der Anfasser auf die Gitterlinie (gx, gy) gezogen
    /// wird; die Gegenseite bleibt, nie kleiner als 1 × 1, kein Umklappen.
    /// `keep` hält das Seitenverhältnis (an der Ecke gewinnt die stärker
    /// gezogene Richtung, an einer Kante wächst die andere Seite mittig mit).
    pub fn drag(self, r: (i64, i64, u32, u32), gx: f32, gy: f32, keep: bool) -> (i64, i64, u32, u32) {
        let (mut x0, mut y0) = (r.0, r.1);
        let (mut x1, mut y1) = (r.0 + r.2 as i64, r.1 + r.3 as i64);
        if self.west() {
            x0 = (gx.round() as i64).min(x1 - 1);
        }
        if self.east() {
            x1 = (gx.round() as i64).max(x0 + 1);
        }
        if self.north() {
            y0 = (gy.round() as i64).min(y1 - 1);
        }
        if self.south() {
            y1 = (gy.round() as i64).max(y0 + 1);
        }
        let (mut w, mut h) = ((x1 - x0) as f64, (y1 - y0) as f64);
        if keep && r.2 > 0 && r.3 > 0 {
            let (rw, rh) = (r.2 as f64, r.3 as f64);
            let k = if self.corner() {
                (w / rw).max(h / rh)
            } else if matches!(self, Handle::N | Handle::S) {
                h / rh
            } else {
                w / rw
            };
            w = (rw * k).round().max(1.0);
            h = (rh * k).round().max(1.0);
            if self.west() {
                x0 = x1 - w as i64;
            } else if !self.east() {
                x0 = (r.0 as f64 + (rw - w) / 2.0).round() as i64;
            }
            if self.north() {
                y0 = y1 - h as i64;
            } else if !self.south() {
                y0 = (r.1 as f64 + (rh - h) / 2.0).round() as i64;
            }
        }
        (x0, y0, w as u32, h as u32)
    }
}

/// Zeilenweises Raster auf `w × h` bringen — nächster Nachbar.
pub fn scale_nearest<T: Copy>(data: &[T], sw: u32, sh: u32, w: u32, h: u32) -> Vec<T> {
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        let sy = (((y as u64 * 2 + 1) * sh as u64) / (2 * h as u64)).min(sh as u64 - 1) as u32;
        for x in 0..w {
            let sx = (((x as u64 * 2 + 1) * sw as u64) / (2 * w as u64)).min(sw as u64 - 1) as u32;
            out.push(data[(sy * sw + sx) as usize]);
        }
    }
    out
}

/// Ausgeschnittene Pixel auf `w × h` skalieren (nächster Nachbar).
pub fn scale_clip(clip: &Clip, w: u32, h: u32) -> Clip {
    Clip { w, h, data: scale_nearest(&clip.data, clip.w, clip.h, w, h) }
}

/// Pixel von einer Palette in eine andere übertragen (wie js/remap.js).
/// Nach der FARBE: gibt es sie in der Ziel-Palette, bekommt der Pixel deren
/// Nummer, sonst wird sie eine freie Farbe des Ziel-Sprites. Mit
/// `keep_numbers` (Strg+Umschalt+V) bleiben Palettennummern, nur freie Farben
/// werden umgerechnet — die gehören ja zu ihrem Sprite.
pub struct Remap<'a> {
    from_pal: &'a Palette,
    from_free: &'a [Rgb],
    index: std::collections::HashMap<Rgb, Px>,
    keep_numbers: bool,
    memo: std::collections::HashMap<Px, Px>,
    /// Wie viele verschiedene Farben im Ziel neu als freie Farbe angelegt wurden.
    pub free: usize,
    /// Wie viele verschiedene Farben eine andere Nummer bekamen.
    pub mapped: usize,
}

impl<'a> Remap<'a> {
    pub fn new(from_pal: &'a Palette, from_free: &'a [Rgb], to_pal: &Palette, keep_numbers: bool) -> Self {
        let mut index = std::collections::HashMap::new();
        for (i, &c) in to_pal.colors.iter().enumerate() {
            index.entry(c).or_insert(i as Px + 1);
        }
        Remap { from_pal, from_free, index, keep_numbers, memo: std::collections::HashMap::new(), free: 0, mapped: 0 }
    }

    /// Ein Pixel; `free_color` legt im Ziel eine freie Farbe an.
    pub fn px(&mut self, v: Px, free_color: &mut impl FnMut(Rgb) -> Px) -> Px {
        if v == 0 || (self.keep_numbers && v < FREE_BASE) {
            return v;
        }
        if let Some(&m) = self.memo.get(&v) {
            return m;
        }
        let out = match rgb_of(v, self.from_pal, self.from_free) {
            None => v,
            Some(c) => match self.index.get(&c) {
                Some(&n) if !self.keep_numbers => {
                    if n != v {
                        self.mapped += 1;
                    }
                    n
                }
                _ => {
                    self.free += 1;
                    free_color(c)
                }
            },
        };
        self.memo.insert(v, out);
        out
    }

    /// Ein ausgeschnittenes Stück übertragen.
    pub fn clip(&mut self, clip: &Clip, free_color: &mut impl FnMut(Rgb) -> Px) -> Clip {
        Clip { w: clip.w, h: clip.h, data: clip.data.iter().map(|&v| self.px(v, free_color)).collect() }
    }
}

/// RGB eines Pixels; `None` für transparent.
pub fn rgb_of(px: Px, pal: &Palette, free: &[Rgb]) -> Option<Rgb> {
    if px >= FREE_BASE {
        free.get((px - FREE_BASE) as usize).copied()
    } else {
        pal.get(px)
    }
}

fn close(a: Option<Rgb>, b: Option<Rgb>, tolerance: f64) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let d: f64 = (0..3).map(|i| (a[i] as f64 - b[i] as f64).powi(2)).sum::<f64>().sqrt();
            // 0 % = nur genau gleich, 100 % = alles (größter Abstand ≈ 441).
            d <= tolerance.clamp(0.0, 1.0) * 441.7
        }
        _ => false,
    }
}

/// Zusammenhängende Fläche ähnlicher Farbe um (x, y) — für Farbwahl und
/// Zauberstab. `tolerance` 0.0–1.0.
pub fn region(img: &Image, pal: &Palette, free: &[Rgb], x: i64, y: i64, tolerance: f64) -> Option<Selection> {
    let (w, h) = (img.width() as i64, img.height() as i64);
    if x < 0 || y < 0 || x >= w || y >= h {
        return None;
    }
    let target = rgb_of(img.get(x as u32, y as u32), pal, free);
    let mut seen = vec![false; (w * h) as usize];
    let mut stack = vec![(x, y)];
    let (mut x0, mut y0, mut x1, mut y1) = (x, y, x, y);
    while let Some((cx, cy)) = stack.pop() {
        let i = (cy * w + cx) as usize;
        if seen[i] || !close(rgb_of(img.get(cx as u32, cy as u32), pal, free), target, tolerance) {
            continue;
        }
        seen[i] = true;
        x0 = x0.min(cx);
        y0 = y0.min(cy);
        x1 = x1.max(cx);
        y1 = y1.max(cy);
        for (nx, ny) in [(cx - 1, cy), (cx + 1, cy), (cx, cy - 1), (cx, cy + 1)] {
            if nx >= 0 && ny >= 0 && nx < w && ny < h && !seen[(ny * w + nx) as usize] {
                stack.push((nx, ny));
            }
        }
    }
    let (sw, sh) = ((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
    let mut mask = vec![false; (sw * sh) as usize];
    for yy in 0..sh {
        for xx in 0..sw {
            mask[(yy * sw + xx) as usize] = seen[((y0 + yy as i64) * w + x0 + xx as i64) as usize];
        }
    }
    Some(Selection { x: x0, y: y0, w: sw, h: sh, mask: Some(mask) })
}

/// Pixel der Auswahl kopieren.
pub fn copy(img: &Image, sel: &Selection) -> Clip {
    let mut data = vec![0; (sel.w * sel.h) as usize];
    for yy in 0..sel.h {
        for xx in 0..sel.w {
            let (x, y) = (sel.x + xx as i64, sel.y + yy as i64);
            if sel.contains(x, y) && x >= 0 && y >= 0 {
                data[(yy * sel.w + xx) as usize] = img.get(x as u32, y as u32);
            }
        }
    }
    Clip { w: sel.w, h: sel.h, data }
}

/// Pixel der Auswahl leeren. Gibt die Zahl der geleerten Pixel zurück.
pub fn clear(img: &mut Image, sel: &Selection) -> usize {
    let mut n = 0;
    for yy in 0..sel.h {
        for xx in 0..sel.w {
            let (x, y) = (sel.x + xx as i64, sel.y + yy as i64);
            if sel.contains(x, y) && x >= 0 && y >= 0 && img.get(x as u32, y as u32) != 0 {
                img.set(x as u32, y as u32, 0);
                n += 1;
            }
        }
    }
    n
}

/// Alle Stellen der Auswahl mit `value` füllen.
pub fn fill(img: &mut Image, sel: &Selection, value: Px) {
    for yy in 0..sel.h {
        for xx in 0..sel.w {
            let (x, y) = (sel.x + xx as i64, sel.y + yy as i64);
            if sel.contains(x, y) && x >= 0 && y >= 0 {
                img.set(x as u32, y as u32, value);
            }
        }
    }
}

/// Clip mit linker oberer Ecke bei (x, y) einsetzen; transparente Stellen
/// lassen das Bild darunter stehen, was über den Rand ragt, fällt weg.
pub fn paste(img: &mut Image, clip: &Clip, x: i64, y: i64) {
    for yy in 0..clip.h {
        for xx in 0..clip.w {
            let v = clip.data[(yy * clip.w + xx) as usize];
            let (tx, ty) = (x + xx as i64, y + yy as i64);
            if v != 0 && tx >= 0 && ty >= 0 {
                img.set(tx as u32, ty as u32, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn nach_der_farbe_uebertragen_wie_im_web() {
        let grau = Palette::new("grau", vec![[255, 255, 255], [170, 170, 170], [85, 85, 85], [0, 0, 0]]);
        let bunt = Palette::new("bunt", vec![[0, 0, 0], [255, 0, 0], [0, 255, 0], [170, 170, 170]]);
        let mut free: Vec<Rgb> = Vec::new();
        let mut new_free = |c: Rgb| {
            free.push(c);
            FREE_BASE + free.len() as Px - 1
        };
        let mut r = Remap::new(&grau, &[], &bunt, false);
        let c = r.clip(&Clip { w: 5, h: 1, data: vec![0, 2, 4, 1, 3] }, &mut new_free);
        assert_eq!(c.data, vec![0, 4, 1, FREE_BASE, FREE_BASE + 1]);
        assert_eq!((r.mapped, r.free), (2, 2));
        assert_eq!(free, vec![[255, 255, 255], [85, 85, 85]]);
        // Nummern behalten: Palette bleibt, freie Farben werden trotzdem umgerechnet.
        let src_free = vec![[1, 2, 3]];
        let mut free2: Vec<Rgb> = vec![[9, 9, 9]];
        let mut r = Remap::new(&grau, &src_free, &bunt, true);
        let c = r.clip(&Clip { w: 2, h: 1, data: vec![3, FREE_BASE] }, &mut |c| {
            free2.push(c);
            FREE_BASE + free2.len() as Px - 1
        });
        assert_eq!(c.data, vec![3, FREE_BASE + 1]);
    }


    #[test]
    fn anfasser_wie_im_web() {
        let r = (2, 3, 4, 2);
        assert_eq!(Handle::Se.drag(r, 10.0, 7.0, false), (2, 3, 8, 4));
        assert_eq!(Handle::E.drag(r, 9.0, 99.0, false), (2, 3, 7, 2));
        assert_eq!(Handle::N.drag(r, 99.0, 1.0, false), (2, 1, 4, 4));
        assert_eq!(Handle::Se.drag(r, -5.0, -5.0, false), (2, 3, 1, 1), "nie kleiner als 1 × 1");
        assert_eq!(Handle::Nw.drag(r, 50.0, 50.0, false), (5, 4, 1, 1), "kein Umklappen");
        assert_eq!(Handle::Se.drag(r, 10.0, 5.0, true), (2, 3, 8, 4), "Seitenverhältnis");
        assert_eq!(Handle::E.drag(r, 10.0, 0.0, true), (2, 2, 8, 4));
        assert_eq!(Handle::Nw.drag(r, -2.0, 1.0, true), (-2, 1, 8, 4));
        assert_eq!(Handle::Nw.drag(r, -2.0, 0.0, true), (-4, 0, 10, 5));
        assert_eq!(Handle::S.pos(r), (4.0, 5.0));
    }

    #[test]
    fn naechster_nachbar_wie_im_web() {
        let c = Clip { w: 2, h: 2, data: vec![1, 2, 3, 4] };
        let big = scale_clip(&c, 4, 4);
        assert_eq!(big.data, vec![1, 1, 2, 2, 1, 1, 2, 2, 3, 3, 4, 4, 3, 3, 4, 4]);
        assert_eq!(scale_clip(&big, 2, 2), c, "zurück ergibt das Original");
        assert_eq!(scale_nearest(&[1, 2, 3], 3, 1, 1, 1), vec![2]);
        assert_eq!(scale_nearest(&[true, false], 2, 1, 4, 1), vec![true, true, false, false]);
    }

    use super::*;

    #[test]
    fn rechteck_aus_zwei_ecken() {
        let s = Selection::rect(5, 7, 2, 3);
        assert_eq!((s.x, s.y, s.w, s.h), (2, 3, 4, 5));
        assert!(s.contains(2, 3) && s.contains(5, 7) && !s.contains(6, 7));
    }

    #[test]
    fn lasso_dreieck() {
        let s = Selection::lasso(&[(0, 0), (8, 0), (0, 8)]).unwrap();
        assert!(s.contains(1, 1));
        assert!(!s.contains(7, 7));
        assert!(s.contains(8, 0) && s.contains(0, 8), "Umriss gehört dazu");
    }

    #[test]
    fn lasso_mit_zu_wenig_punkten() {
        assert!(Selection::lasso(&[(0, 0), (5, 5)]).is_none());
    }

    #[test]
    fn kopieren_leeren_einfuegen() {
        let mut img = Image::new(10, 10);
        img.set(2, 2, 3);
        img.set(3, 2, 4);
        let sel = Selection::rect(2, 2, 3, 2);
        let clip = copy(&img, &sel);
        assert_eq!(clip.data, vec![3, 4]);
        assert_eq!(clear(&mut img, &sel), 2);
        assert!(img.is_empty());
        paste(&mut img, &clip, 8, 8);
        assert_eq!(img.get(8, 8), 3);
        assert_eq!(img.get(9, 8), 4);
        paste(&mut img, &clip, 9, 9); // ragt über den Rand
        assert_eq!(img.get(9, 9), 3);
    }

    #[test]
    fn transparent_im_clip_laesst_darunter_stehen() {
        let mut img = Image::new(4, 1);
        img.set(1, 0, 5);
        paste(&mut img, &Clip { w: 2, h: 1, data: vec![0, 7] }, 1, 0);
        assert_eq!(img.get(1, 0), 5);
        assert_eq!(img.get(2, 0), 7);
    }

    #[test]
    fn farbwahl_nur_zusammenhaengend() {
        let pal = Palette::grayscale();
        let mut img = Image::new(6, 3);
        for x in 0..2 {
            img.set(x, 1, 5);
        }
        img.set(4, 1, 5); // gleiche Farbe, aber getrennt
        let s = region(&img, &pal, &[], 0, 1, 0.0).unwrap();
        assert!(s.contains(0, 1) && s.contains(1, 1));
        assert!(!s.contains(4, 1));
        assert!(!s.contains(0, 0));
    }

    #[test]
    fn toleranz_nimmt_aehnliche_farben_mit() {
        let pal = Palette::grayscale(); // 2 = #cccccc, 3 = #999999
        let mut img = Image::new(3, 1);
        img.set(0, 0, 2);
        img.set(1, 0, 3);
        assert!(!region(&img, &pal, &[], 0, 0, 0.0).unwrap().contains(1, 0));
        assert!(region(&img, &pal, &[], 0, 0, 0.3).unwrap().contains(1, 0));
    }

    #[test]
    fn auswahl_verschieben() {
        let s = Selection::rect(0, 0, 1, 1).moved(5, 6);
        assert!(s.contains(5, 6) && s.contains(6, 7) && !s.contains(0, 0));
    }
}
