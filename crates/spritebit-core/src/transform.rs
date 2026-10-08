//! Bild als Ganzes: spiegeln, drehen, zuschneiden, zentrieren, Leinwand
//! ändern, skalieren — wie `transform.js` der Web-Version.
//!
//! Auf dem ganzen Sprite wirkt alles auf ALLE Bilder (jeden Frame, jede
//! Ebene; verknüpfte Zellen teilen sich ein Bild und werden einmal
//! umgerechnet). Sonst hätten die Frames verschiedene Größen. Ausnahme ist
//! die freie Drehung: sie dreht nur die Zelle, die man sieht.
//!
//! Mit Auswahl wirken Spiegeln und Drehen nur auf den schwebenden Inhalt —
//! dafür gibt es die Raster-Funktionen ([`flip_h`] …), die auf einem
//! beliebigen Feld `w × h` arbeiten (Clip-Pixel und Auswahl-Maske).
//!
//! Gerechnet wird über [`Image::pixels`]: nur belegte Kacheln, damit auch
//! 8192 × 8192 mit wenig Inhalt schnell bleibt.

use crate::image::Image;
use crate::sprite::{Sprite, MAX_SIDE};

// ── Raster (Clip, Maske) ────────────────────────────────────────────

/// Waagerecht spiegeln (links ↔ rechts).
pub fn flip_h<T: Copy>(w: u32, h: u32, a: &[T]) -> Vec<T> {
    let (w, h) = (w as usize, h as usize);
    (0..h).flat_map(|y| (0..w).rev().map(move |x| a[y * w + x])).collect()
}

/// Senkrecht spiegeln (oben ↔ unten).
pub fn flip_v<T: Copy>(w: u32, h: u32, a: &[T]) -> Vec<T> {
    let (w, h) = (w as usize, h as usize);
    (0..h).rev().flat_map(|y| (0..w).map(move |x| a[y * w + x])).collect()
}

/// 90° im Uhrzeigersinn: aus `w × h` wird `h × w`.
pub fn rot90<T: Copy + Default>(w: u32, h: u32, a: &[T]) -> Vec<T> {
    let (w, h) = (w as usize, h as usize);
    let mut out = vec![T::default(); w * h];
    for y in 0..h {
        for x in 0..w {
            // out ist h breit: (x, y) → (h-1-y, x)
            out[x * h + (h - 1 - y)] = a[y * w + x];
        }
    }
    out
}

/// Ergebnis einer freien Drehung.
#[derive(Clone, Debug, PartialEq)]
pub struct Rotated<T> {
    pub w: u32,
    pub h: u32,
    pub cells: Vec<T>,
    /// Wo etwas aus der Quelle gelandet ist.
    pub mask: Vec<bool>,
}

/// Um `deg` Grad im Uhrzeigersinn drehen, nächster Nachbar (es wird nichts
/// gemischt). `grow`: das Ergebnis umfasst die ganze gedrehte Form, sonst
/// bleibt die Größe und die Ecken fallen weg. `mask`: nur diese Stellen der
/// Quelle zählen.
pub fn rotate<T: Copy + Default>(w: u32, h: u32, a: &[T], mask: Option<&[bool]>, deg: f64, grow: bool) -> Rotated<T> {
    let rad = deg.to_radians();
    let (cos, sin) = (rad.cos(), rad.sin());
    let (wf, hf) = (w as f64, h as f64);
    let nw = if grow { ((wf * cos).abs() + (hf * sin).abs() - 1e-9).ceil().max(1.0) as u32 } else { w };
    let nh = if grow { ((wf * sin).abs() + (hf * cos).abs() - 1e-9).ceil().max(1.0) as u32 } else { h };
    let (scx, scy, dcx, dcy) = (wf / 2.0, hf / 2.0, nw as f64 / 2.0, nh as f64 / 2.0);
    let mut cells = vec![T::default(); (nw * nh) as usize];
    let mut out_mask = vec![false; (nw * nh) as usize];
    for y in 0..nh {
        for x in 0..nw {
            let (dx, dy) = (x as f64 + 0.5 - dcx, y as f64 + 0.5 - dcy);
            let sx = (dx * cos + dy * sin + scx).floor();
            let sy = (-dx * sin + dy * cos + scy).floor();
            if sx < 0.0 || sy < 0.0 || sx >= wf || sy >= hf {
                continue;
            }
            let i = sy as usize * w as usize + sx as usize;
            if mask.is_some_and(|m| !m[i]) {
                continue;
            }
            cells[(y * nw + x) as usize] = a[i];
            out_mask[(y * nw + x) as usize] = true;
        }
    }
    Rotated { w: nw, h: nh, cells, mask: out_mask }
}

// ── Bilder ──────────────────────────────────────────────────────────

/// Neues Bild `nw × nh`; jedes Pixel landet dort, wohin `f` es schickt
/// (außerhalb fällt es weg).
fn remap(img: &Image, nw: u32, nh: u32, f: impl Fn(u32, u32) -> (i64, i64)) -> (Image, usize) {
    let mut out = Image::new(nw, nh);
    let mut lost = 0;
    for (x, y, v) in img.pixels() {
        let (nx, ny) = f(x, y);
        if nx >= 0 && ny >= 0 && nx < nw as i64 && ny < nh as i64 {
            out.set(nx as u32, ny as u32, v);
        } else {
            lost += 1;
        }
    }
    (out, lost)
}

pub fn image_flip_h(img: &Image) -> Image {
    let w = img.width() as i64;
    remap(img, img.width(), img.height(), |x, y| (w - 1 - x as i64, y as i64)).0
}

pub fn image_flip_v(img: &Image) -> Image {
    let h = img.height() as i64;
    remap(img, img.width(), img.height(), |x, y| (x as i64, h - 1 - y as i64)).0
}

pub fn image_rot90(img: &Image) -> Image {
    let h = img.height() as i64;
    remap(img, img.height(), img.width(), |x, y| (h - 1 - y as i64, x as i64)).0
}

/// Freie Drehung, Größe bleibt (Ecken fallen weg).
pub fn image_rotate(img: &Image, deg: f64) -> Image {
    let (w, h) = (img.width(), img.height());
    let rad = deg.to_radians();
    let (cos, sin) = (rad.cos(), rad.sin());
    let (c_x, c_y) = (w as f64 / 2.0, h as f64 / 2.0);
    let mut out = Image::new(w, h);
    // Rückwärts: für jedes Zielpixel die Quelle. Nur Zielpixel im Umkreis
    // des Inhalts kommen in Frage.
    let Some((bx, by, bw, bh)) = bounds(img) else { return out };
    let corners = [(bx, by), (bx + bw, by), (bx, by + bh), (bx + bw, by + bh)];
    let fwd = |x: f64, y: f64| {
        let (dx, dy) = (x - c_x, y - c_y);
        (dx * cos - dy * sin + c_x, dx * sin + dy * cos + c_y)
    };
    let pts: Vec<(f64, f64)> = corners.iter().map(|&(x, y)| fwd(x as f64, y as f64)).collect();
    let x0 = pts.iter().map(|p| p.0).fold(f64::MAX, f64::min).floor().max(0.0) as u32;
    let y0 = pts.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor().max(0.0) as u32;
    let x1 = (pts.iter().map(|p| p.0).fold(f64::MIN, f64::max).ceil() as i64).clamp(0, w as i64) as u32;
    let y1 = (pts.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil() as i64).clamp(0, h as i64) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let (dx, dy) = (x as f64 + 0.5 - c_x, y as f64 + 0.5 - c_y);
            let sx = (dx * cos + dy * sin + c_x).floor();
            let sy = (-dx * sin + dy * cos + c_y).floor();
            if sx >= 0.0 && sy >= 0.0 && sx < w as f64 && sy < h as f64 {
                let v = img.get(sx as u32, sy as u32);
                if v != 0 {
                    out.set(x, y, v);
                }
            }
        }
    }
    out
}

/// Rechteck (x, y, w, h) um den Inhalt eines Bildes.
pub fn bounds(img: &Image) -> Option<(u32, u32, u32, u32)> {
    let mut b: Option<(u32, u32, u32, u32)> = None;
    for (x, y, _) in img.pixels() {
        b = Some(match b {
            None => (x, y, x, y),
            Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
        });
    }
    b.map(|(x0, y0, x1, y1)| (x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

// ── Sprite ──────────────────────────────────────────────────────────

/// Jedes Bild umrechnen; Größe danach `nw × nh`. Gibt die Zahl der
/// weggefallenen Pixel zurück.
fn map_images(sp: &mut Sprite, nw: u32, nh: u32, f: impl Fn(&Image) -> (Image, usize)) -> usize {
    let mut lost = 0;
    for img in sp.images.iter_mut() {
        let (n, l) = f(img);
        *img = n;
        lost += l;
    }
    sp.width = nw;
    sp.height = nh;
    lost
}

/// Rechteck um den Inhalt aller Bilder — was in irgendeinem Frame steht, zählt.
pub fn sprite_bounds(sp: &Sprite) -> Option<(u32, u32, u32, u32)> {
    let mut u: Option<(u32, u32, u32, u32)> = None;
    for img in &sp.images {
        if let Some((x, y, w, h)) = bounds(img) {
            u = Some(match u {
                None => (x, y, x + w, y + h),
                Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x + w), d.max(y + h)),
            });
        }
    }
    u.map(|(a, b, c, d)| (a, b, c - a, d - b))
}

pub fn flip_sprite(sp: &mut Sprite, horizontal: bool) {
    let (w, h) = (sp.width, sp.height);
    map_images(sp, w, h, |i| (if horizontal { image_flip_h(i) } else { image_flip_v(i) }, 0));
}

pub fn rotate_sprite90(sp: &mut Sprite) {
    let (w, h) = (sp.width, sp.height);
    map_images(sp, h, w, |i| (image_rot90(i), 0));
}

#[derive(Debug, PartialEq)]
pub enum TransformResult {
    Done { w: u32, h: u32, lost: usize },
    /// Nichts zu tun (leer, schon so, gleiche Größe).
    Nothing,
    TooBig,
    TooSmall,
}

/// Auf den Inhalt zuschneiden.
pub fn trim(sp: &mut Sprite) -> TransformResult {
    let Some((x, y, w, h)) = sprite_bounds(sp) else { return TransformResult::Nothing };
    if w == sp.width && h == sp.height {
        return TransformResult::Nothing;
    }
    let (x, y) = (x as i64, y as i64);
    map_images(sp, w, h, |i| remap(i, w, h, |px, py| (px as i64 - x, py as i64 - y)));
    TransformResult::Done { w, h, lost: 0 }
}

/// Inhalt mittig setzen.
pub fn center(sp: &mut Sprite) -> TransformResult {
    let Some((x, y, w, h)) = sprite_bounds(sp) else { return TransformResult::Nothing };
    let dx = ((sp.width - w) as f64 / 2.0).round() as i64 - x as i64;
    let dy = ((sp.height - h) as f64 / 2.0).round() as i64 - y as i64;
    if dx == 0 && dy == 0 {
        return TransformResult::Nothing;
    }
    let (sw, sh) = (sp.width, sp.height);
    map_images(sp, sw, sh, |i| remap(i, sw, sh, |px, py| (px as i64 + dx, py as i64 + dy)));
    TransformResult::Done { w: sw, h: sh, lost: 0 }
}

/// Leinwand ändern (nicht skalieren). `centered`: der alte Inhalt rutscht
/// in die Mitte, sonst bleibt er oben links. Was nicht passt, fällt weg.
pub fn resize_canvas(sp: &mut Sprite, nw: u32, nh: u32, centered: bool) -> TransformResult {
    let (nw, nh) = (nw.clamp(1, MAX_SIDE), nh.clamp(1, MAX_SIDE));
    if nw == sp.width && nh == sp.height {
        return TransformResult::Nothing;
    }
    let dx = if centered { ((nw as f64 - sp.width as f64) / 2.0).round() as i64 } else { 0 };
    let dy = if centered { ((nh as f64 - sp.height as f64) / 2.0).round() as i64 } else { 0 };
    let lost = map_images(sp, nw, nh, |i| remap(i, nw, nh, |x, y| (x as i64 + dx, y as i64 + dy)));
    TransformResult::Done { w: nw, h: nh, lost }
}

/// Skalieren, nächster Nachbar — Pixel bleiben Pixel.
pub fn scale(sp: &mut Sprite, factor: f64) -> TransformResult {
    let nw = (sp.width as f64 * factor).round();
    let nh = (sp.height as f64 * factor).round();
    if nw < 1.0 || nh < 1.0 {
        return TransformResult::TooSmall;
    }
    if nw > MAX_SIDE as f64 || nh > MAX_SIDE as f64 {
        return TransformResult::TooBig;
    }
    let (nw, nh, w, h) = (nw as u32, nh as u32, sp.width, sp.height);
    map_images(sp, nw, nh, |img| {
        let mut out = Image::new(nw, nh);
        if factor >= 1.0 {
            // Vorwärts: jedes Pixel wird zu einem Block.
            for (x, y, v) in img.pixels() {
                let (x0, x1) = ((x as f64 * factor).ceil() as u32, (((x + 1) as f64 * factor).ceil() as u32).min(nw));
                let (y0, y1) = ((y as f64 * factor).ceil() as u32, (((y + 1) as f64 * factor).ceil() as u32).min(nh));
                for yy in y0..y1 {
                    for xx in x0..x1 {
                        out.set(xx, yy, v);
                    }
                }
            }
        } else {
            for y in 0..nh {
                for x in 0..nw {
                    let sx = ((x as f64 / factor).floor() as u32).min(w - 1);
                    let sy = ((y as f64 / factor).floor() as u32).min(h - 1);
                    let v = img.get(sx, sy);
                    if v != 0 {
                        out.set(x, y, v);
                    }
                }
            }
        }
        (out, 0)
    });
    TransformResult::Done { w: nw, h: nh, lost: 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp_with(px: &[(u32, u32, u16)], w: u32, h: u32) -> Sprite {
        let mut sp = Sprite::new("t", w, h).unwrap();
        for &(x, y, v) in px {
            sp.active().set(x, y, v);
        }
        sp
    }

    #[test]
    fn raster_spiegeln_und_drehen() {
        let a = [1, 2, 3, 4, 5, 6]; // 3 × 2
        assert_eq!(flip_h(3, 2, &a), vec![3, 2, 1, 6, 5, 4]);
        assert_eq!(flip_v(3, 2, &a), vec![4, 5, 6, 1, 2, 3]);
        // 90° rechts: 2 × 3
        assert_eq!(rot90(3, 2, &a), vec![4, 1, 5, 2, 6, 3]);
    }

    #[test]
    fn freie_drehung_um_90_wie_rot90() {
        let a = [1u16, 2, 3, 4, 5, 6];
        let r = rotate(3, 2, &a, None, 90.0, true);
        assert_eq!((r.w, r.h), (2, 3));
        assert_eq!(r.cells, rot90(3, 2, &a));
        assert!(r.mask.iter().all(|&m| m));
    }

    #[test]
    fn sprite_drehen_tauscht_die_groesse() {
        let mut sp = sp_with(&[(0, 0, 1)], 4, 2);
        rotate_sprite90(&mut sp);
        assert_eq!((sp.width, sp.height), (2, 4));
        assert_eq!(sp.cel(0, 0).get(1, 0), 1);
    }

    #[test]
    fn spiegeln_trifft_alle_frames() {
        let mut sp = sp_with(&[(0, 1, 3)], 5, 3);
        sp.add_frame(0, true);
        flip_sprite(&mut sp, true);
        assert_eq!(sp.cel(0, 0).get(4, 1), 3);
        assert_eq!(sp.cel(1, 0).get(4, 1), 3);
        flip_sprite(&mut sp, false);
        assert_eq!(sp.cel(1, 0).get(4, 1), 3, "Zeile 1 von 3 bleibt Mitte");
    }

    #[test]
    fn zuschneiden_und_zentrieren() {
        let mut sp = sp_with(&[(2, 3, 1), (4, 5, 2)], 10, 10);
        assert_eq!(trim(&mut sp), TransformResult::Done { w: 3, h: 3, lost: 0 });
        assert_eq!(sp.cel(0, 0).get(0, 0), 1);
        assert_eq!(sp.cel(0, 0).get(2, 2), 2);
        assert_eq!(trim(&mut sp), TransformResult::Nothing);

        let mut sp = sp_with(&[(0, 0, 1)], 5, 5);
        center(&mut sp);
        assert_eq!(sp.cel(0, 0).get(2, 2), 1);
        assert_eq!(center(&mut sp), TransformResult::Nothing);
    }

    #[test]
    fn leinwand_mittig_und_oben_links() {
        let mut sp = sp_with(&[(0, 0, 1), (3, 3, 2)], 4, 4);
        let r = resize_canvas(&mut sp, 2, 2, false);
        assert_eq!(r, TransformResult::Done { w: 2, h: 2, lost: 1 });
        assert_eq!(sp.cel(0, 0).get(0, 0), 1);
        let mut sp = sp_with(&[(0, 0, 1)], 4, 4);
        resize_canvas(&mut sp, 8, 8, true);
        assert_eq!(sp.cel(0, 0).get(2, 2), 1);
    }

    #[test]
    fn skalieren_hoch_und_runter() {
        let mut sp = sp_with(&[(1, 0, 7)], 2, 2);
        scale(&mut sp, 2.0);
        assert_eq!((sp.width, sp.height), (4, 4));
        assert_eq!([sp.cel(0, 0).get(2, 0), sp.cel(0, 0).get(3, 1), sp.cel(0, 0).get(1, 0)], [7, 7, 0]);
        scale(&mut sp, 0.5);
        assert_eq!((sp.width, sp.height), (2, 2));
        assert_eq!(sp.cel(0, 0).get(1, 0), 7);
        assert_eq!(scale(&mut sp, 10_000.0), TransformResult::TooBig);
    }

    #[test]
    fn freie_drehung_des_bildes_behaelt_die_groesse() {
        let mut img = Image::new(9, 9);
        img.set(4, 1, 5);
        let r = image_rotate(&img, 90.0);
        assert_eq!((r.width(), r.height()), (9, 9));
        assert_eq!(r.get(7, 4), 5, "oben → rechts");
    }

    #[test]
    fn pixel_iterator_ueber_kachelgrenzen() {
        let mut img = Image::new(130, 70);
        img.set(129, 69, 3);
        img.set(64, 0, 2);
        let mut v: Vec<_> = img.pixels().collect();
        v.sort();
        assert_eq!(v, vec![(64, 0, 2), (129, 69, 3)]);
    }
}
