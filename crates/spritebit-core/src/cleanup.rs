//! Feinschliff (Panel) und Farben zusammenfassen — wie `spritefx.js` der Web-Version.
//!
//! * [`remove_background`] — vom Rand her ähnliche Flächen löschen
//! * [`despeckle`] — einzelne Streupixel auf die Mehrheit der Nachbarn setzen
//! * [`outline`] — Kante um alles Gefüllte
//! * [`median_cut`] / [`nearest`] — viele Farben auf wenige zusammenfassen
//!
//! Alles wirkt auf ein Bild (die aktive Zelle) und rechnet nur im Umkreis
//! des Inhalts, damit leere Riesenflächen nichts kosten.

use crate::image::{Image, Px};
use crate::palette::{Palette, Rgb};
use crate::selection::rgb_of;
use crate::transform::bounds;

fn dist2(a: Rgb, b: Rgb) -> u32 {
    (0..3).map(|i| (a[i] as i32 - b[i] as i32).pow(2) as u32).sum()
}

/// Toleranz 0.0–1.0 als größter erlaubter quadratischer Abstand.
fn tol2(tolerance: f64) -> f64 {
    let lin = tolerance.clamp(0.0, 1.0) * (3.0f64 * 255.0 * 255.0).sqrt();
    lin * lin
}

/// Hintergrund entfernen: Annahme, Hintergrund ist, was am Rand hängt. Von
/// jedem gefüllten Randpixel aus wird nach innen gelöscht, solange der
/// Farbabstand zur gerade besuchten Zelle in der Toleranz bleibt — folgt
/// Verläufen, stoppt am Farbsprung zum Motiv. Gibt die Zahl gelöschter Pixel.
///
/// (Transparente Randflächen führen nie zu Farbe — die Web-Version läuft sie
/// trotzdem ab; hier werden sie übersprungen, das Ergebnis ist dasselbe.)
pub fn remove_background(img: &mut Image, pal: &Palette, free: &[Rgb], tolerance: f64) -> usize {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let t2 = tol2(tolerance);
    let mut seen = vec![false; (w * h) as usize];
    let mut stack: Vec<(i64, i64)> = Vec::new();
    let border = (0..w).flat_map(|x| [(x, 0), (x, h - 1)]).chain((0..h).flat_map(|y| [(0, y), (w - 1, y)]));
    for (x, y) in border {
        if img.get(x as u32, y as u32) != 0 {
            stack.push((x, y));
        }
    }
    let mut removed = 0;
    while let Some((x, y)) = stack.pop() {
        let i = (y * w + x) as usize;
        if seen[i] {
            continue;
        }
        seen[i] = true;
        let Some(cur) = rgb_of(img.get(x as u32, y as u32), pal, free) else { continue };
        img.set(x as u32, y as u32, 0);
        removed += 1;
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if nx < 0 || ny < 0 || nx >= w || ny >= h || seen[(ny * w + nx) as usize] {
                continue;
            }
            if let Some(nb) = rgb_of(img.get(nx as u32, ny as u32), pal, free) {
                if dist2(cur, nb) as f64 <= t2 {
                    stack.push((nx, ny));
                }
            }
        }
    }
    removed
}

/// Glätten: Pixel, deren 8 Nachbarn zu mindestens 5 einen anderen Wert
/// haben, bekommen diesen Wert. Ein Durchlauf, ohne Kaskade.
pub fn despeckle(img: &mut Image) -> usize {
    let Some((bx, by, bw, bh)) = bounds(img) else { return 0 };
    let (w, h) = (img.width() as i64, img.height() as i64);
    let x0 = (bx as i64 - 1).max(0);
    let y0 = (by as i64 - 1).max(0);
    let x1 = (bx as i64 + bw as i64 + 1).min(w);
    let y1 = (by as i64 + bh as i64 + 1).min(h);
    let src = img.clone();
    let mut changed = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let mut counts: Vec<(Px, u32)> = Vec::with_capacity(8);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if (dx == 0 && dy == 0) || nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let v = src.get(nx as u32, ny as u32);
                    match counts.iter_mut().find(|(c, _)| *c == v) {
                        Some(e) => e.1 += 1,
                        None => counts.push((v, 1)),
                    }
                }
            }
            // Bei Gleichstand gewinnt der zuerst gefundene — wie im Web.
            let (top, n) = counts.iter().fold((0, 0), |best, &(v, c)| if c > best.1 { (v, c) } else { best });
            if n >= 5 && top != src.get(x as u32, y as u32) {
                img.set(x as u32, y as u32, top);
                changed += 1;
            }
        }
    }
    changed
}

/// Wo die Outline entsteht.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutlineMode {
    /// Um alles Gefüllte herum (die Figur wird größer).
    #[default]
    Outside,
    /// Auf den gefüllten Randpixeln selbst; der Bildrand zählt nicht als Kante.
    Inside,
    /// Beides.
    Both,
}

/// Outline in `value`, `thickness` Durchläufe dick — wie `outlineGrid` im Web.
/// Gibt die Zahl geänderter Pixel.
pub fn outline(img: &mut Image, value: Px, thickness: u32, mode: OutlineMode) -> usize {
    let mut changed = 0;
    // Innen zuerst — färbt nur Gefülltes um, die Außenkante bleibt gleich.
    if mode != OutlineMode::Outside {
        changed += inner_outline(img, value, thickness);
    }
    if mode != OutlineMode::Inside {
        changed += outer_outline(img, value, thickness);
    }
    changed
}

/// Gefüllte Pixel, die (4er-Nachbarschaft) an Transparentes grenzen, bekommen
/// `value`; jeder weitere Durchlauf geht eine Reihe nach innen.
fn inner_outline(img: &mut Image, value: Px, thickness: u32) -> usize {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let mut edge = vec![false; (w * h) as usize];
    for _ in 0..thickness {
        let mut mark = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if img.get(x as u32, y as u32) == 0 || edge[(y * w + x) as usize] {
                    continue;
                }
                // Außen = transparent oder schon als Kante markiert.
                let out = |nx: i64, ny: i64| {
                    nx >= 0 && ny >= 0 && nx < w && ny < h && (img.get(nx as u32, ny as u32) == 0 || edge[(ny * w + nx) as usize])
                };
                if out(x - 1, y) || out(x + 1, y) || out(x, y - 1) || out(x, y + 1) {
                    mark.push((y * w + x) as usize);
                }
            }
        }
        if mark.is_empty() {
            break;
        }
        for i in mark {
            edge[i] = true;
        }
    }
    let mut changed = 0;
    for (i, _) in edge.iter().enumerate().filter(|(_, e)| **e) {
        let (x, y) = ((i as i64 % w) as u32, (i as i64 / w) as u32);
        if img.get(x, y) != value {
            img.set(x, y, value);
            changed += 1;
        }
    }
    changed
}

/// Transparente Pixel, die (4er-Nachbarschaft) an Gefülltes grenzen, bekommen
/// `value`; `thickness` Durchläufe.
fn outer_outline(img: &mut Image, value: Px, thickness: u32) -> usize {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let mut added = 0;
    for _ in 0..thickness {
        let Some((bx, by, bw, bh)) = bounds(img) else { break };
        let mut to_fill = Vec::new();
        for y in (by as i64 - 1).max(0)..(by as i64 + bh as i64 + 1).min(h) {
            for x in (bx as i64 - 1).max(0)..(bx as i64 + bw as i64 + 1).min(w) {
                if img.get(x as u32, y as u32) != 0 {
                    continue;
                }
                let filled = |nx: i64, ny: i64| nx >= 0 && ny >= 0 && nx < w && ny < h && img.get(nx as u32, ny as u32) != 0;
                if filled(x - 1, y) || filled(x + 1, y) || filled(x, y - 1) || filled(x, y + 1) {
                    to_fill.push((x as u32, y as u32));
                }
            }
        }
        if to_fill.is_empty() {
            break;
        }
        added += to_fill.len();
        for (x, y) in to_fill {
            img.set(x, y, value);
        }
    }
    added
}

/// Median-Cut: bis zu `max` Farben, die `pixels` gut vertreten.
pub fn median_cut(pixels: &[Rgb], max: usize) -> Vec<Rgb> {
    if pixels.is_empty() || max == 0 {
        return Vec::new();
    }
    let range = |bk: &[Rgb]| -> [u8; 3] {
        let mut r = [0u8; 3];
        for (c, slot) in r.iter_mut().enumerate() {
            let (lo, hi) = bk.iter().fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[c]), hi.max(p[c])));
            *slot = hi.saturating_sub(lo);
        }
        r
    };
    let mut buckets: Vec<Vec<Rgb>> = vec![pixels.to_vec()];
    while buckets.len() < max {
        let mut best: Option<(usize, u8, usize)> = None; // (Eimer, Spanne, Kanal)
        for (i, bk) in buckets.iter().enumerate() {
            if bk.len() < 2 {
                continue;
            }
            let rg = range(bk);
            let m = *rg.iter().max().unwrap();
            if best.is_none_or(|b| m > b.1) {
                let chan = if rg[0] >= rg[1] && rg[0] >= rg[2] { 0 } else if rg[1] >= rg[2] { 1 } else { 2 };
                best = Some((i, m, chan));
            }
        }
        let Some((bi, _, chan)) = best else { break };
        let mut bk = buckets.remove(bi);
        bk.sort_by_key(|p| p[chan]);
        let hi = bk.split_off(bk.len() / 2);
        buckets.insert(bi, hi);
        buckets.insert(bi, bk);
    }
    buckets
        .iter()
        .filter(|b| !b.is_empty())
        .map(|bk| {
            let n = bk.len() as f64;
            let avg = |c: usize| (bk.iter().map(|p| p[c] as f64).sum::<f64>() / n).round() as u8;
            [avg(0), avg(1), avg(2)]
        })
        .collect()
}

/// Nummer der nächstgelegenen Farbe in `colors`.
pub fn nearest(rgb: Rgb, colors: &[Rgb]) -> usize {
    (0..colors.len()).min_by_key(|&i| dist2(rgb, colors[i])).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pal() -> Palette {
        Palette::new("t", vec![[255, 255, 255], [250, 250, 250], [0, 0, 0], [200, 0, 0]])
    }

    #[test]
    fn hintergrund_vom_rand_her() {
        // 5×5 weißer Grund (1) mit fast-weißem Verlauf (2), in der Mitte schwarz (3)
        let mut img = Image::new(5, 5);
        for y in 0..5 {
            for x in 0..5 {
                img.set(x, y, if x == 4 { 2 } else { 1 });
            }
        }
        img.set(2, 2, 3);
        let n = remove_background(&mut img, &pal(), &[], 0.05);
        assert_eq!(n, 24);
        assert_eq!(img.get(2, 2), 3, "Motiv bleibt");
    }

    #[test]
    fn hintergrund_mit_toleranz_null_bleibt_bei_gleicher_farbe() {
        let mut img = Image::new(3, 1);
        img.set(0, 0, 1);
        img.set(1, 0, 2);
        img.set(2, 0, 3);
        // Rand: alle drei sind Randpixel → alle drei werden Startpunkte.
        assert_eq!(remove_background(&mut img, &pal(), &[], 0.0), 3);
    }

    #[test]
    fn glaetten_entfernt_streupixel() {
        let mut img = Image::new(5, 5);
        for y in 0..5 {
            for x in 0..5 {
                img.set(x, y, 1);
            }
        }
        img.set(2, 2, 4);
        assert_eq!(despeckle(&mut img), 1);
        assert_eq!(img.get(2, 2), 1);
        let mut lone = Image::new(5, 5);
        lone.set(2, 2, 4);
        assert_eq!(despeckle(&mut lone), 1, "allein auf leerer Fläche verschwindet");
        assert_eq!(lone.get(2, 2), 0);
    }

    #[test]
    fn outline_ein_und_zwei_pixel() {
        let mut img = Image::new(7, 7);
        img.set(3, 3, 1);
        assert_eq!(outline(&mut img, 3, 1, OutlineMode::Outside), 4);
        assert_eq!(img.get(3, 2), 3);
        assert_eq!(img.get(2, 2), 0, "nur 4er-Nachbarn");
        let mut img = Image::new(7, 7);
        img.set(3, 3, 1);
        assert_eq!(outline(&mut img, 3, 2, OutlineMode::Outside), 4 + 8);
    }

    /// 3×3-Block in der Mitte eines 7×7-Bilds.
    fn block() -> Image {
        let mut img = Image::new(7, 7);
        for y in 2..=4 {
            for x in 2..=4 {
                img.set(x, y, 1);
            }
        }
        img
    }

    #[test]
    fn outline_innen_und_beides() {
        let mut img = block();
        assert_eq!(outline(&mut img, 3, 1, OutlineMode::Inside), 8);
        assert_eq!(img.get(3, 3), 1, "Mitte bleibt");
        assert_eq!(img.get(2, 2), 3);
        assert_eq!(img.get(3, 1), 0, "außen bleibt leer");
        let mut img = block();
        assert_eq!(outline(&mut img, 3, 2, OutlineMode::Inside), 9, "zweiter Durchlauf erreicht die Mitte");
        let mut img = block();
        img.set(2, 2, 3);
        assert_eq!(outline(&mut img, 3, 1, OutlineMode::Inside), 7, "schon gleich gefärbt zählt nicht");
        let mut full = Image::new(3, 3);
        for y in 0..3 {
            for x in 0..3 {
                full.set(x, y, 1);
            }
        }
        assert_eq!(outline(&mut full, 3, 1, OutlineMode::Inside), 0, "Bildrand ist keine Kante");
        let mut img = block();
        assert_eq!(outline(&mut img, 3, 1, OutlineMode::Both), 8 + 12);
        assert_eq!(img.get(3, 3), 1);
        assert_eq!(img.get(2, 2), 3);
        assert_eq!(img.get(3, 1), 3);
    }

    #[test]
    fn median_cut_trennt_rot_und_blau() {
        let px = vec![[250, 0, 0], [240, 10, 0], [0, 0, 250], [0, 10, 240]];
        let mut c = median_cut(&px, 2);
        c.sort();
        assert_eq!(c, vec![[0, 5, 245], [245, 5, 0]]);
        assert_eq!(nearest([10, 0, 200], &c), 0);
        assert_eq!(median_cut(&px, 10).len(), 4, "mehr Farben als Pixel");
    }
}
