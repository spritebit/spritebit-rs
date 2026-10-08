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
