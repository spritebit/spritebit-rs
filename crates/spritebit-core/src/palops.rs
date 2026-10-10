//! Paletten und Bild zusammen ändern — wie `palettes.js`, `palorder.js` und
//! `reduce.js` der Web-Version.
//!
//! * [`assign_keep_look`] — andere Palette, das Bild sieht gleich aus
//! * [`add_free_colors`] — freie Farben des Sprites in die Palette
//! * [`shade_order`] / [`reorder`] — Farben umsortieren, Bild bleibt gleich
//! * [`image_colors`] / [`reduce`] — Bildfarben auf eine neue Palette
//!   zusammenfassen (Median-Cut)
//!
//! Die Pixel speichern die NUMMER einer Farbe; wer die Palette umbaut, muss
//! die Pixel mit umschreiben. Das tun die Funktionen hier für alle Bilder
//! eines Sprites.

use std::collections::HashMap;

use crate::cleanup::{median_cut, nearest};
use crate::image::{Px, FREE_BASE};
use crate::palette::{Palette, Rgb, MAX_COLORS};
use crate::selection::rgb_of;
use crate::sprite::Sprite;

/// Jedes Pixel jedes Bildes durch `f` schicken (nur nicht-transparente).
fn remap_pixels(sp: &mut Sprite, mut f: impl FnMut(&mut Sprite, Px) -> Px) {
    for k in 0..sp.images.len() {
        let pixels: Vec<(u32, u32, Px)> = sp.images[k].pixels().collect();
        for (x, y, v) in pixels {
            let n = f(sp, v);
            if n != v {
                sp.images[k].set(x, y, n);
            }
        }
    }
}

/// Palette wechseln, ohne dass sich das Bild ändert: Nummern, deren Farbe
/// in der neuen Palette woanders steht, wandern dorthin; Farben, die es
/// dort nicht gibt, werden freie Farben; freie Farben, die die neue Palette
/// hat, werden wieder Nummern. Gibt die Zahl der Pixel zurück, die frei
/// bleiben mussten. Den Namen setzt der Aufrufer.
pub fn assign_keep_look(sp: &mut Sprite, old: &Palette, new: &Palette) -> usize {
    let mut first: HashMap<Rgb, Px> = HashMap::new();
    for (i, &c) in new.colors.iter().enumerate() {
        first.entry(c).or_insert(i as Px + 1);
    }
    let mut kept = 0;
    remap_pixels(sp, |sp, v| {
        let Some(rgb) = rgb_of(v, old, &sp.free) else { return v };
        if v < FREE_BASE && new.get(v) == Some(rgb) {
            return v;
        }
        match first.get(&rgb) {
            Some(&i) => i,
            None => {
                kept += 1;
                sp.free_color(rgb)
            }
        }
    });
    kept
}

/// Freie Farben, die in den Bildern vorkommen (in der Reihenfolge von `sp.free`).
pub fn used_free_colors(sp: &Sprite) -> Vec<Rgb> {
    let mut used = vec![false; sp.free.len()];
    for img in &sp.images {
        for (_, _, v) in img.pixels() {
            if v >= FREE_BASE {
                if let Some(u) = used.get_mut((v - FREE_BASE) as usize) {
                    *u = true;
                }
            }
        }
    }
    sp.free.iter().zip(used).filter(|(_, u)| *u).map(|(&c, _)| c).collect()
}

/// Freie Farben hinten an die Farben `colors` hängen und die Pixel zu
/// Nummern machen. Gibt die neue Farbliste und die Zahl neuer Farben —
/// oder `Err(n)`, wenn `n` Farben nicht mehr hineinpassen.
pub fn add_free_colors(sp: &mut Sprite, colors: &[Rgb]) -> Result<(Vec<Rgb>, usize), usize> {
    let mut out = colors.to_vec();
    let mut have: HashMap<Rgb, Px> = HashMap::new();
    for (i, &c) in colors.iter().enumerate() {
        have.entry(c).or_insert(i as Px + 1);
    }
    let fresh: Vec<Rgb> = used_free_colors(sp).into_iter().filter(|c| !have.contains_key(c)).collect();
    if out.len() + fresh.len() > MAX_COLORS {
        return Err(fresh.len());
    }
    for c in &fresh {
        out.push(*c);
        have.insert(*c, out.len() as Px);
    }
    let free = sp.free.clone();
    remap_pixels(sp, |_, v| if v >= FREE_BASE { free.get((v - FREE_BASE) as usize).and_then(|c| have.get(c).copied()).unwrap_or(v) } else { v });
    Ok((out, fresh.len()))
}

/// Eine Farbe hinten an `colors` hängen; Pixel, die sie als freie Farbe
/// haben, bekommen die neue Nummer. `None`, wenn sie schon drin ist oder die
/// Palette voll ist.
pub fn add_color(sp: &mut Sprite, colors: &[Rgb], rgb: Rgb) -> Option<Vec<Rgb>> {
    if colors.contains(&rgb) || colors.len() >= MAX_COLORS {
        return None;
    }
    let mut out = colors.to_vec();
    out.push(rgb);
    let n = out.len() as Px;
    let free = sp.free.clone();
    remap_pixels(sp, |_, v| if v >= FREE_BASE && free.get((v - FREE_BASE) as usize) == Some(&rgb) { n } else { v });
    Some(out)
}

/// Farbe Nummer `idx` aus `pal` entfernen; die Nummern dahinter rücken auf
/// (samt Namen). Das Bild bleibt gleich: Pixel der entfernten Farbe zeigen
/// auf dieselbe Farbe an anderer Stelle — oder werden zur freien Farbe.
/// Gibt die Zahl der Pixel zurück, die frei geworden sind; `None`, wenn es
/// die Nummer nicht gibt oder es die letzte Farbe ist.
pub fn remove_color(pal: &mut Palette, idx: usize, sprites: &mut [&mut Sprite]) -> Option<usize> {
    if idx == 0 || idx > pal.colors.len() || pal.colors.len() <= 1 {
        return None;
    }
    let rgb = pal.colors[idx - 1];
    let twin = (1..=pal.colors.len()).find(|&i| i != idx && pal.colors[i - 1] == rgb).map(|t| if t > idx { t - 1 } else { t });
    let idx_px = idx as Px;
    let mut freed = 0;
    for sp in sprites.iter_mut() {
        remap_pixels(sp, |sp, v| {
            if v == idx_px {
                match twin {
                    Some(t) => t as Px,
                    None => {
                        freed += 1;
                        sp.free_color(rgb)
                    }
                }
            } else if v > idx_px && v < FREE_BASE {
                v - 1
            } else {
                v
            }
        });
    }
    pal.colors.remove(idx - 1);
    pal.names = std::mem::take(&mut pal.names).into_iter().filter(|&(i, _)| i != idx_px).map(|(i, n)| (if i > idx_px { i - 1 } else { i }, n)).collect();
    Some(freed)
}

/// HSL eines Farbwerts: Farbton 0–360, Sättigung und Helligkeit 0–1.
fn hsl(c: Rgb) -> (f64, f64, f64) {
    let [r, g, b] = c.map(|v| v as f64 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let (l, d) = ((max + min) / 2.0, max - min);
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let mut h = if max == r {
        ((g - b) / d) % 6.0
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    if h < 0.0 {
        h += 360.0;
    }
    (h, s, l)
}

/// Reihenfolge nach Farbstufen: erst Grautöne dunkel → hell, dann je
/// Farbton (30°-Bereiche, Rot nicht zerteilt) eine Gruppe dunkel → hell.
/// Gibt die alten Nummern (1-basiert) in neuer Reihenfolge zurück.
pub fn shade_order(colors: &[Rgb]) -> Vec<usize> {
    const GRAY_SAT: f64 = 0.15;
    const SECTOR: f64 = 30.0;
    let mut items: Vec<(i32, f64, usize)> = colors
        .iter()
        .enumerate()
        .map(|(k, &c)| {
            let (h, s, l) = hsl(c);
            let group = if s < GRAY_SAT { -1 } else { (((h + SECTOR / 2.0) % 360.0) / SECTOR).floor() as i32 };
            (group, l, k + 1)
        })
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
    items.into_iter().map(|t| t.2).collect()
}

/// Farbe von Platz `from` auf Platz `to` (beide 1-basiert) — als Reihenfolge.
pub fn move_color(n: usize, from: usize, to: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (1..=n).collect();
    if from >= 1 && from <= n && to >= 1 && to <= n {
        let c = order.remove(from - 1);
        order.insert(to - 1, c);
    }
    order
}

/// Neue Reihenfolge anwenden: `order[k]` ist die alte Nummer, die auf
/// Platz k+1 kommt. Gibt die umsortierten Farben zurück und schreibt die
/// Pixel aller `sprites` um. Ist nichts zu tun: `None`.
pub fn reorder(colors: &[Rgb], order: &[usize], sprites: &mut [&mut Sprite]) -> Option<Vec<Rgb>> {
    if order.len() != colors.len() || order.iter().enumerate().all(|(k, &o)| o == k + 1) {
        return None;
    }
    // perm[alt] = neu
    let mut perm = vec![0 as Px; colors.len() + 1];
    for (k, &old) in order.iter().enumerate() {
        perm[old] = k as Px + 1;
    }
    for sp in sprites.iter_mut() {
        remap_pixels(sp, |_, v| if (v as usize) < perm.len() && v > 0 { perm[v as usize] } else { v });
    }
    Some(order.iter().map(|&o| colors[o - 1]).collect())
}

/// Alle Farben im Bild mit ihrer Häufigkeit.
pub fn image_colors(sp: &Sprite, pal: &Palette) -> Vec<(Rgb, usize)> {
    let mut counts: HashMap<Rgb, usize> = HashMap::new();
    let mut order: Vec<Rgb> = Vec::new();
    for img in &sp.images {
        for (_, _, v) in img.pixels() {
            if let Some(c) = rgb_of(v, pal, &sp.free) {
                let e = counts.entry(c).or_insert_with(|| {
                    order.push(c);
                    0
                });
                *e += 1;
            }
        }
    }
    order.into_iter().map(|c| (c, counts[&c])).collect()
}

/// Bildfarben auf höchstens `n` zusammenfassen: die neuen Farben (hell →
/// dunkel) und für jede Bildfarbe ihre neue Nummer.
pub fn reduce_colors(colors: &[(Rgb, usize)], n: usize) -> (Vec<Rgb>, HashMap<Rgb, Px>) {
    let mut out: Vec<Rgb> = if n >= colors.len() {
        colors.iter().map(|c| c.0).collect()
    } else {
        // Jede Farbe zählt so oft, wie sie vorkommt.
        let px: Vec<Rgb> = colors.iter().flat_map(|&(c, k)| std::iter::repeat_n(c, k)).collect();
        median_cut(&px, n)
    };
    let lum = |c: &Rgb| 0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64;
    out.sort_by(|a, b| lum(b).total_cmp(&lum(a)));
    let map = colors.iter().map(|&(c, _)| (c, nearest(c, &out) as Px + 1)).collect();
    (out, map)
}

/// Pixel auf die reduzierte Palette umschreiben (Name setzt der Aufrufer).
pub fn apply_reduce(sp: &mut Sprite, pal: &Palette, map: &HashMap<Rgb, Px>) {
    let free = sp.free.clone();
    remap_pixels(sp, |_, v| rgb_of(v, pal, &free).and_then(|c| map.get(&c).copied()).unwrap_or(0));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pal(colors: &[Rgb]) -> Palette {
        Palette::new("p", colors.to_vec())
    }

    #[test]
    fn zuweisen_ohne_dass_sich_das_bild_aendert() {
        let old = pal(&[[255, 0, 0], [0, 255, 0], [1, 2, 3]]);
        let new = pal(&[[0, 255, 0], [255, 0, 0]]);
        let mut sp = Sprite::new("t", 4, 1).unwrap();
        sp.active().set(0, 0, 1); // rot → in neuer Palette Nr. 2
        sp.active().set(1, 0, 2); // grün → Nr. 1
        sp.active().set(2, 0, 3); // gibt es nicht → frei
        let free = sp.free_color([0, 255, 0]);
        sp.active().set(3, 0, free); // freies Grün → Nr. 1
        assert_eq!(assign_keep_look(&mut sp, &old, &new), 1);
        let img = sp.cel(0, 0);
        assert_eq!([img.get(0, 0), img.get(1, 0), img.get(3, 0)], [2, 1, 1]);
        assert_eq!(rgb_of(img.get(2, 0), &new, &sp.free), Some([1, 2, 3]));
    }

    #[test]
    fn farbe_entfernen_ohne_dass_sich_das_bild_aendert() {
        let mut p = pal(&[[1, 1, 1], [2, 2, 2], [3, 3, 3], [2, 2, 2]]);
        p.names.insert(3, "drei".into());
        p.names.insert(2, "zwei".into());
        let mut sp = Sprite::new("t", 3, 1).unwrap();
        sp.active().set(0, 0, 2); // hat einen Zwilling (Nr. 4 → danach Nr. 3)
        sp.active().set(1, 0, 3); // rückt auf Nr. 2
        sp.active().set(2, 0, 1);
        assert_eq!(remove_color(&mut p, 2, &mut [&mut sp]), Some(0));
        assert_eq!(p.colors, vec![[1, 1, 1], [3, 3, 3], [2, 2, 2]]);
        assert_eq!(p.name_of(2), Some("drei"));
        assert_eq!(p.names.len(), 1);
        let img = sp.cel(0, 0);
        assert_eq!([img.get(0, 0), img.get(1, 0), img.get(2, 0)], [3, 2, 1]);
        // Ohne Zwilling: freie Farbe
        assert_eq!(remove_color(&mut p, 2, &mut [&mut sp]), Some(1));
        assert_eq!(rgb_of(sp.cel(0, 0).get(1, 0), &p, &sp.free), Some([3, 3, 3]));
        let mut q = pal(&[[1, 1, 1], [5, 5, 5]]);
        let mut s2 = Sprite::new("t", 1, 1).unwrap();
        s2.active().set(0, 0, 2);
        assert_eq!(remove_color(&mut q, 2, &mut [&mut s2]), Some(1));
        assert_eq!(rgb_of(s2.cel(0, 0).get(0, 0), &q, &s2.free), Some([5, 5, 5]));
        assert_eq!(remove_color(&mut q, 1, &mut [&mut s2]), None);
    }

    #[test]
    fn freie_farben_in_die_palette() {
        let mut sp = Sprite::new("t", 3, 1).unwrap();
        let a = sp.free_color([9, 9, 9]);
        let b = sp.free_color([255, 0, 0]);
        sp.free_color([7, 7, 7]); // unbenutzt
        sp.active().set(0, 0, a);
        sp.active().set(1, 0, b);
        let (colors, added) = add_free_colors(&mut sp, &[[255, 0, 0]]).unwrap();
        assert_eq!(added, 1);
        assert_eq!(colors, vec![[255, 0, 0], [9, 9, 9]]);
        assert_eq!([sp.cel(0, 0).get(0, 0), sp.cel(0, 0).get(1, 0)], [2, 1]);
        let full = vec![[0, 0, 0]; MAX_COLORS];
        let c = sp.free_color([1, 1, 1]);
        sp.active().set(2, 0, c);
        assert_eq!(add_free_colors(&mut sp, &full), Err(1));
    }

    #[test]
    fn eine_farbe_in_die_palette() {
        let mut sp = Sprite::new("t", 2, 1).unwrap();
        let a = sp.free_color([9, 9, 9]);
        let b = sp.free_color([7, 7, 7]);
        sp.active().set(0, 0, a);
        sp.active().set(1, 0, b);
        let colors = add_color(&mut sp, &[[255, 0, 0]], [9, 9, 9]).unwrap();
        assert_eq!(colors, vec![[255, 0, 0], [9, 9, 9]]);
        assert_eq!([sp.cel(0, 0).get(0, 0), sp.cel(0, 0).get(1, 0)], [2, b], "nur diese Farbe wird zur Nummer");
        assert_eq!(add_color(&mut sp, &colors, [9, 9, 9]), None, "schon drin");
        assert_eq!(add_color(&mut sp, &vec![[0, 0, 0]; MAX_COLORS], [1, 1, 1]), None, "voll");
    }

    #[test]
    fn farbstufen_grau_zuerst_dann_je_farbton() {
        let colors = [[200, 0, 0], [255, 255, 255], [80, 0, 0], [0, 0, 0], [0, 0, 200]];
        assert_eq!(shade_order(&colors), vec![4, 2, 3, 1, 5]);
    }

    #[test]
    fn umsortieren_laesst_das_bild_gleich() {
        let colors = [[1, 1, 1], [2, 2, 2], [3, 3, 3]];
        let mut sp = Sprite::new("t", 3, 1).unwrap();
        for x in 0..3 {
            sp.active().set(x, 0, x as Px + 1);
        }
        let order = move_color(3, 3, 1);
        assert_eq!(order, vec![3, 1, 2]);
        let new = reorder(&colors, &order, &mut [&mut sp]).unwrap();
        assert_eq!(new, vec![[3, 3, 3], [1, 1, 1], [2, 2, 2]]);
        let p = pal(&new);
        for x in 0..3 {
            assert_eq!(rgb_of(sp.cel(0, 0).get(x, 0), &p, &[]), Some(colors[x as usize]));
        }
        assert!(reorder(&new, &[1, 2, 3], &mut [&mut sp]).is_none());
    }

    #[test]
    fn reduzieren_auf_zwei_farben() {
        let p = pal(&[[250, 250, 250], [240, 240, 240], [10, 10, 10]]);
        let mut sp = Sprite::new("t", 3, 1).unwrap();
        for x in 0..3 {
            sp.active().set(x, 0, x as Px + 1);
        }
        let colors = image_colors(&sp, &p);
        assert_eq!(colors.len(), 3);
        let (out, map) = reduce_colors(&colors, 2);
        assert_eq!(out.len(), 2);
        assert!(out[0][0] > out[1][0], "hell zuerst");
        apply_reduce(&mut sp, &p, &map);
        let img = sp.cel(0, 0);
        assert_eq!([img.get(0, 0), img.get(1, 0), img.get(2, 0)], [1, 1, 2]);
    }
}
