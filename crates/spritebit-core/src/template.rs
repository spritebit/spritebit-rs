//! Schablone: ein Bild zum Abzeichnen — wie `template.js` der Web-Version.
//!
//! Die Schablone liegt über (oder unter) der Fläche: ein Kasten, so groß wie
//! die Fläche mal `scale`, mittig plus Verschiebung; das Bild steht darin
//! eingepasst („contain“, Seitenverhältnis bleibt). Alle Maße hier sind in
//! Sprite-Pixeln, damit sie nicht vom Zoom abhängen.
//!
//! „Aufs Raster übernehmen“ tastet für jede Zelle die Bildpixel ab, die sie
//! abdeckt (Farbmittel, nach Alpha gewichtet), und malt das Ergebnis in die
//! aktive Zelle: mit der nächsten Palettenfarbe, als Originalfarbe (freie
//! Farbe) oder auf N Farben reduziert (Median-Cut).

use crate::cleanup::{median_cut, nearest};
use crate::image::{Image, Px};
use crate::palette::{Palette, Rgb};
use crate::sprite::Sprite;

/// Darunter gilt ein Schablonen-Pixel als durchsichtig.
pub const TRACE_ALPHA_MIN: u8 = 32;

#[derive(Clone, Debug)]
pub struct Template {
    pub w: u32,
    pub h: u32,
    /// RGBA, zeilenweise.
    pub rgba: Vec<u8>,
}

/// Wo das Bild auf der Fläche steht: (x, y, Breite, Höhe) in Sprite-Pixeln.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TraceMode {
    /// Nächste Farbe der Palette.
    Palette,
    /// Originalfarben (freie Farben).
    Raw,
    /// Auf so viele Farben zusammengefasst (freie Farben).
    Quantize(usize),
}

impl Template {
    /// Lage auf einer Fläche `sw × sh`: Kasten = Fläche × `scale` (1.0 =
    /// gleich groß), Mitte um (`ox`, `oy`) verschoben, Bild eingepasst.
    pub fn placement(&self, sw: u32, sh: u32, scale: f64, ox: f64, oy: f64) -> Placement {
        let (bw, bh) = (sw as f64 * scale, sh as f64 * scale);
        let (bx, by) = (sw as f64 / 2.0 + ox - bw / 2.0, sh as f64 / 2.0 + oy - bh / 2.0);
        let (ai, ab) = (self.w as f64 / self.h as f64, bw / bh);
        if ai > ab {
            let h = bw / ai;
            Placement { x: bx, y: by + (bh - h) / 2.0, w: bw, h }
        } else {
            let w = bh * ai;
            Placement { x: bx + (bw - w) / 2.0, y: by, w, h: bh }
        }
    }

    fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let o = ((y * self.w + x) * 4) as usize;
        [self.rgba[o], self.rgba[o + 1], self.rgba[o + 2], self.rgba[o + 3]]
    }

    /// Farbe genau an der Stelle (x, y) der Fläche (Pipette).
    pub fn pick(&self, p: Placement, x: f64, y: f64) -> Option<[u8; 4]> {
        let (ix, iy) = ((x - p.x) / p.w * self.w as f64, (y - p.y) / p.h * self.h as f64);
        if ix < 0.0 || iy < 0.0 || ix >= self.w as f64 || iy >= self.h as f64 {
            return None;
        }
        Some(self.px(ix as u32, iy as u32))
    }

    /// Mittel der Bildpixel, die die Zelle (gx, gy) abdeckt — nach Alpha
    /// gewichtet. `None`: die Zelle liegt außerhalb des Bildes.
    pub fn cell(&self, p: Placement, gx: u32, gy: u32) -> Option<[u8; 4]> {
        let fx = |x: f64| (x - p.x) / p.w * self.w as f64;
        let fy = |y: f64| (y - p.y) / p.h * self.h as f64;
        let (ax, bx) = (fx(gx as f64), fx(gx as f64 + 1.0));
        let (ay, by) = (fy(gy as f64), fy(gy as f64 + 1.0));
        let x0 = ax.min(bx).floor().max(0.0) as i64;
        let y0 = ay.min(by).floor().max(0.0) as i64;
        let x1 = (ax.max(bx).ceil() as i64 - 1).min(self.w as i64 - 1);
        let y1 = (ay.max(by).ceil() as i64 - 1).min(self.h as i64 - 1);
        if x1 < x0 || y1 < y0 {
            return None;
        }
        let (mut sr, mut sg, mut sb, mut sa, mut n) = (0u64, 0u64, 0u64, 0u64, 0u64);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let [r, g, b, a] = self.px(x as u32, y as u32);
                let a = a as u64;
                sr += r as u64 * a;
                sg += g as u64 * a;
                sb += b as u64 * a;
                sa += a;
                n += 1;
            }
        }
        if sa == 0 {
            return Some([0, 0, 0, 0]);
        }
        let avg = |s: u64| ((s as f64 / sa as f64).round()) as u8;
        Some([avg(sr), avg(sg), avg(sb), (sa as f64 / n as f64).round() as u8])
    }

    /// Zellen, die das Bild überhaupt berührt (x0, y0, x1, y1 — exklusiv).
    pub fn covered(p: Placement, sw: u32, sh: u32) -> (u32, u32, u32, u32) {
        let c = |v: f64, max: u32| v.clamp(0.0, max as f64) as u32;
        (c(p.x.floor(), sw), c(p.y.floor(), sh), c((p.x + p.w).ceil(), sw), c((p.y + p.h).ceil(), sh))
    }
}

/// Schablone in die aktive Zelle malen. Gibt die Zahl geänderter Pixel
/// zurück — oder `None`, wenn es zum Reduzieren keine Farben gibt.
pub fn trace(sp: &mut Sprite, pal: &Palette, tpl: &Template, p: Placement, mode: TraceMode) -> Option<usize> {
    let (x0, y0, x1, y1) = Template::covered(p, sp.width, sp.height);
    let mut cells: Vec<(u32, u32, Rgb)> = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            if let Some([r, g, b, a]) = tpl.cell(p, x, y) {
                if a >= TRACE_ALPHA_MIN {
                    cells.push((x, y, [r, g, b]));
                }
            }
        }
    }
    let qpal = match mode {
        TraceMode::Quantize(n) => {
            // Für den Median-Cut reicht eine Stichprobe von höchstens einer Million.
            let stride = (cells.len() / 1_000_000).max(1);
            let px: Vec<Rgb> = cells.iter().step_by(stride).map(|c| c.2).collect();
            let q = median_cut(&px, n.clamp(2, 64));
            if q.is_empty() {
                return None;
            }
            q
        }
        _ => Vec::new(),
    };
    let mut painted = 0;
    for (x, y, c) in cells {
        let v: Px = match mode {
            TraceMode::Palette => nearest(c, &pal.colors) as Px + 1,
            TraceMode::Raw => sp.free_color(c),
            TraceMode::Quantize(_) => sp.free_color(qpal[nearest(c, &qpal)]),
        };
        let img: &mut Image = sp.active();
        if img.get(x, y) != v {
            img.set(x, y, v);
            painted += 1;
        }
    }
    Some(painted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2×1-Bild: links rot, rechts blau (deckend).
    fn tpl() -> Template {
        Template { w: 2, h: 1, rgba: vec![255, 0, 0, 255, 0, 0, 255, 255] }
    }

    #[test]
    fn einpassen_mit_rand_oben_und_unten() {
        let p = tpl().placement(4, 4, 1.0, 0.0, 0.0);
        assert_eq!(p, Placement { x: 0.0, y: 1.0, w: 4.0, h: 2.0 });
        let p = tpl().placement(4, 4, 0.5, 1.0, 0.0);
        assert_eq!(p, Placement { x: 2.0, y: 1.5, w: 2.0, h: 1.0 });
    }

    #[test]
    fn pipette_und_zellen() {
        let t = tpl();
        let p = t.placement(4, 4, 1.0, 0.0, 0.0);
        assert_eq!(t.pick(p, 0.5, 1.5), Some([255, 0, 0, 255]));
        assert_eq!(t.pick(p, 3.5, 1.5), Some([0, 0, 255, 255]));
        assert_eq!(t.pick(p, 0.5, 0.5), None, "Rand oben");
        assert_eq!(t.cell(p, 0, 0), None);
        assert_eq!(t.cell(p, 1, 2), Some([255, 0, 0, 255]));
        // Eine große Zelle über beide Pixel mittelt.
        let p1 = t.placement(1, 1, 1.0, 0.0, 0.0);
        assert_eq!(t.cell(p1, 0, 0), Some([128, 0, 128, 255]));
    }

    #[test]
    fn uebernehmen_mit_palette_und_original() {
        let pal = Palette::new("p", vec![[250, 0, 0], [0, 0, 250]]);
        let t = tpl();
        let mut sp = Sprite::new("t", 4, 4).unwrap();
        let p = t.placement(4, 4, 1.0, 0.0, 0.0);
        assert_eq!(trace(&mut sp, &pal, &t, p, TraceMode::Palette), Some(8));
        assert_eq!(sp.cel(0, 0).get(0, 1), 1);
        assert_eq!(sp.cel(0, 0).get(3, 2), 2);
        assert_eq!(sp.cel(0, 0).get(0, 0), 0, "außerhalb bleibt leer");
        assert_eq!(trace(&mut sp, &pal, &t, p, TraceMode::Raw), Some(8));
        assert_eq!(sp.free, vec![[255, 0, 0], [0, 0, 255]]);
        let mut sp = Sprite::new("t", 4, 4).unwrap();
        assert_eq!(trace(&mut sp, &pal, &t, p, TraceMode::Quantize(2)), Some(8));
        assert_eq!(sp.free.len(), 2);
    }
}
