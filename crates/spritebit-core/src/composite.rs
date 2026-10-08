//! Was man sieht: alle sichtbaren Ebenen übereinander — für einen Ausschnitt.
//!
//! Bei großen Flächen wird nie das ganze Bild zusammengesetzt, nur der Teil,
//! der gerade auf dem Bildschirm ist. Die Regeln sind die der Web-Version
//! (flatGrid): volle Deckkraft überschreibt; eine halbdurchsichtige Ebene
//! wird mit dem Pixel darunter gemischt — über leerem Grund bleibt sie
//! deckend, Pixel kennen keine Transparenz-Stufen.

use crate::image::{Px, FREE_BASE};
use crate::palette::{Palette, Rgb};
use crate::sprite::Sprite;

/// Rechteck in Sprite-Pixeln.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    /// Auf die Fläche des Sprites zurechtgestutzt.
    pub fn clamp_to(self, width: u32, height: u32) -> Rect {
        let x = self.x.min(width);
        let y = self.y.min(height);
        Rect { x, y, w: self.w.min(width - x), h: self.h.min(height - y) }
    }
}

fn color_of(px: Px, pal: &Palette, free: &[Rgb]) -> Option<Rgb> {
    if px >= FREE_BASE {
        free.get((px - FREE_BASE) as usize).copied()
    } else {
        pal.get(px)
    }
}

fn mix(below: Rgb, top: Rgb, a: f32) -> Rgb {
    let m = |b: u8, t: u8| (b as f32 * (1.0 - a) + t as f32 * a).round() as u8;
    [m(below[0], top[0]), m(below[1], top[1]), m(below[2], top[2])]
}

/// Frame `frame` im Ausschnitt `rect` als RGBA (4 Byte je Pixel, Zeile für
/// Zeile). Transparente Stellen haben Alpha 0.
pub fn render_rgba(sp: &Sprite, pal: &Palette, frame: usize, rect: Rect) -> Vec<u8> {
    render_rgba_step(sp, pal, frame, rect, 1).0
}

/// Wie [`render_rgba`], aber nur jedes `step`-te Pixel in jeder Richtung —
/// für weit herausgezoomte Ansichten, in denen mehrere Sprite-Pixel auf
/// einen Bildschirm-Pixel fallen. Ergebnis: Puffer und seine Breite/Höhe
/// (aufgerundet, `ceil(w / step)` × `ceil(h / step)`).
pub fn render_rgba_step(sp: &Sprite, pal: &Palette, frame: usize, rect: Rect, step: u32) -> (Vec<u8>, u32, u32) {
    let step = step.max(1);
    let r = rect.clamp_to(sp.width, sp.height);
    let (ow, oh) = (r.w.div_ceil(step), r.h.div_ceil(step));
    let mut out = vec![0u8; (ow * oh * 4) as usize];
    let mut filled = vec![false; (ow * oh) as usize];
    for (l, layer) in sp.layers.iter().enumerate() {
        if !layer.visible || layer.opacity <= 0.0 {
            continue;
        }
        let img = sp.cel(frame, l);
        if img.allocated_tiles() == 0 {
            continue;
        }
        for y in 0..oh {
            for x in 0..ow {
                let px = img.get(r.x + x * step, r.y + y * step);
                if px == 0 {
                    continue;
                }
                let Some(top) = color_of(px, pal, &sp.free) else { continue };
                let i = (y * ow + x) as usize;
                let o = i * 4;
                let rgb = if layer.opacity >= 1.0 || !filled[i] {
                    top
                } else {
                    mix([out[o], out[o + 1], out[o + 2]], top, layer.opacity)
                };
                out[o..o + 3].copy_from_slice(&rgb);
                out[o + 3] = 255;
                filled[i] = true;
            }
        }
    }
    (out, ow, oh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let o = ((y * w + x) * 4) as usize;
        [buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]
    }

    #[test]
    fn obere_ebene_deckt_untere() {
        let pal = Palette::grayscale();
        let mut sp = Sprite::new("a", 4, 4).unwrap();
        sp.cel_mut(0, 0).set(1, 1, 5); // schwarz
        sp.add_layer(1, "oben");
        sp.cel_mut(0, 1).set(1, 1, 1); // weiß
        let buf = render_rgba(&sp, &pal, 0, Rect { x: 0, y: 0, w: 4, h: 4 });
        assert_eq!(px(&buf, 4, 1, 1), [255, 255, 255, 255]);
        assert_eq!(px(&buf, 4, 0, 0)[3], 0, "leer bleibt durchsichtig");
    }

    #[test]
    fn halbdurchsichtig_mischt_ueber_farbe_und_deckt_ueber_leer() {
        let pal = Palette::grayscale();
        let mut sp = Sprite::new("a", 2, 1).unwrap();
        sp.cel_mut(0, 0).set(0, 0, 5); // schwarz
        sp.add_layer(1, "oben");
        sp.layers[1].opacity = 0.5;
        sp.cel_mut(0, 1).set(0, 0, 1); // weiß über schwarz
        sp.cel_mut(0, 1).set(1, 0, 1); // weiß über leer
        let buf = render_rgba(&sp, &pal, 0, Rect { x: 0, y: 0, w: 2, h: 1 });
        assert_eq!(px(&buf, 2, 0, 0), [128, 128, 128, 255]);
        assert_eq!(px(&buf, 2, 1, 0), [255, 255, 255, 255]);
    }

    #[test]
    fn ausgeblendete_ebene_fehlt() {
        let pal = Palette::grayscale();
        let mut sp = Sprite::new("a", 2, 2).unwrap();
        sp.cel_mut(0, 0).set(0, 0, 1);
        sp.layers[0].visible = false;
        let buf = render_rgba(&sp, &pal, 0, Rect { x: 0, y: 0, w: 2, h: 2 });
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn freie_farbe_erscheint_in_ihrer_farbe() {
        let pal = Palette::grayscale();
        let mut sp = Sprite::new("a", 1, 1).unwrap();
        let v = sp.free_color([10, 20, 30]);
        sp.active().set(0, 0, v);
        let buf = render_rgba(&sp, &pal, 0, Rect { x: 0, y: 0, w: 1, h: 1 });
        assert_eq!(px(&buf, 1, 0, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn nur_der_ausschnitt_wird_gerechnet() {
        let pal = Palette::grayscale();
        let mut sp = Sprite::new("gross", 8192, 8192).unwrap();
        sp.active().set(5000, 6000, 5);
        let r = Rect { x: 4990, y: 5990, w: 20, h: 20 };
        let buf = render_rgba(&sp, &pal, 0, r);
        assert_eq!(buf.len(), 20 * 20 * 4);
        assert_eq!(px(&buf, 20, 10, 10), [0, 0, 0, 255]);
    }

    #[test]
    fn mit_schrittweite_nur_jedes_nte_pixel() {
        let pal = Palette::grayscale();
        let mut sp = Sprite::new("a", 10, 10).unwrap();
        sp.active().set(4, 4, 5);
        sp.active().set(5, 5, 1);
        let (buf, w, h) = render_rgba_step(&sp, &pal, 0, Rect { x: 0, y: 0, w: 10, h: 10 }, 4);
        assert_eq!((w, h), (3, 3));
        assert_eq!(px(&buf, w, 1, 1), [0, 0, 0, 255], "Pixel (4,4) steht für den Block");
        assert_eq!(buf.len(), 3 * 3 * 4);
    }

    #[test]
    fn ganz_herausgezoomt_bleibt_der_puffer_klein() {
        let pal = Palette::grayscale();
        let sp = Sprite::new("gross", 8192, 8192).unwrap();
        let (buf, w, h) = render_rgba_step(&sp, &pal, 0, Rect { x: 0, y: 0, w: 8192, h: 8192 }, 8);
        assert_eq!((w, h), (1024, 1024));
        assert_eq!(buf.len(), 1024 * 1024 * 4);
    }

    #[test]
    fn ausschnitt_ueber_den_rand_wird_gekappt() {
        let r = Rect { x: 6, y: 6, w: 10, h: 10 }.clamp_to(8, 8);
        assert_eq!(r, Rect { x: 6, y: 6, w: 2, h: 2 });
        let r = Rect { x: 20, y: 0, w: 5, h: 5 }.clamp_to(8, 8);
        assert_eq!(r.w, 0);
    }
}
