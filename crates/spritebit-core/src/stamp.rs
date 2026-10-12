//! Eigener Pinsel aus einer Auswahl (Strg+B wie in Aseprite) — wie `stamp.js` der Web-Version.
//!
//! Strg+B macht aus dem Inhalt der Auswahl einen Pinsel. Stift, Pinsel und
//! Radierer setzen dann diese Form: in ihren eigenen Farben oder einfarbig
//! („Aktuelle Farbe“). Leere Stellen der Form malen nicht; der Radierer
//! löscht genau die Form.
//!
//! Der Zeiger liegt wie beim eckigen Pinsel in der Mitte: Versatz
//! -floor((w-1)/2) bzw. -floor((h-1)/2) zur linken oberen Ecke.

use std::collections::BTreeMap;

use crate::image::Px;
use crate::selection::Clip;
use crate::tools::Span;

/// Leere Ränder abschneiden. `None`, wenn nichts darin ist.
pub fn trim(clip: &Clip) -> Option<Clip> {
    let (w, h) = (clip.w as usize, clip.h as usize);
    let at = |x: usize, y: usize| clip.data[y * w + x];
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if at(x, y) != 0 {
                any = true;
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if !any {
        return None;
    }
    let data = (y0..=y1).flat_map(|y| (x0..=x1).map(move |x| (x, y))).map(|(x, y)| at(x, y)).collect();
    Some(Clip { w: (x1 - x0 + 1) as u32, h: (y1 - y0 + 1) as u32, data })
}

/// Linke obere Ecke des Pinsels, wenn der Zeiger auf (x, y) steht.
pub fn origin(clip: &Clip, x: i64, y: i64) -> (i64, i64) {
    (x - (clip.w as i64 - 1) / 2, y - (clip.h as i64 - 1) / 2)
}

/// Die Abschnitte, die der Pinsel auf (x, y) setzt, nach Wert sortiert.
/// `color`: alles in diesem Wert (einfarbig, 0 = radieren), sonst die eigenen Farben.
pub fn spans(clip: &Clip, x: i64, y: i64, color: Option<Px>) -> BTreeMap<Px, Vec<Span>> {
    let (ox, oy) = origin(clip, x, y);
    let mut out: BTreeMap<Px, Vec<Span>> = BTreeMap::new();
    for sy in 0..clip.h as i64 {
        let row = &clip.data[(sy * clip.w as i64) as usize..((sy + 1) * clip.w as i64) as usize];
        let mut sx = 0;
        while sx < row.len() {
            let v = row[sx];
            if v == 0 {
                sx += 1;
                continue;
            }
            // Gleiche Werte nebeneinander zu einem Abschnitt zusammenfassen.
            let val = color.unwrap_or(v);
            let start = sx;
            while sx < row.len() && row[sx] != 0 && color.unwrap_or(row[sx]) == val {
                sx += 1;
            }
            out.entry(val).or_default().push((oy + sy, ox + start as i64, ox + sx as i64 - 1));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leere_raender_fallen_weg() {
        let c = Clip { w: 4, h: 3, data: vec![0, 0, 0, 0, 0, 3, 4, 0, 0, 0, 5, 0] };
        let t = trim(&c).unwrap();
        assert_eq!((t.w, t.h, t.data.clone()), (2, 2, vec![3, 4, 0, 5]));
        assert!(trim(&Clip { w: 2, h: 1, data: vec![0, 0] }).is_none(), "leere Auswahl gibt keinen Pinsel");
    }

    #[test]
    fn zeiger_in_der_mitte_wie_beim_eckigen_pinsel() {
        assert_eq!(origin(&Clip { w: 3, h: 3, data: vec![] }, 10, 10), (9, 9));
        assert_eq!(origin(&Clip { w: 2, h: 4, data: vec![] }, 10, 10), (10, 9));
    }

    #[test]
    fn eigene_farben_oder_einfarbig_leere_stellen_nicht() {
        let c = Clip { w: 3, h: 2, data: vec![3, 3, 0, 0, 0, 7] };
        let s = spans(&c, 5, 5, None);
        assert_eq!(s[&3], vec![(5, 4, 5)]);
        assert_eq!(s[&7], vec![(6, 6, 6)]);
        let m = spans(&c, 5, 5, Some(0));
        assert_eq!(m.len(), 1, "radieren: genau die Form");
        assert_eq!(m[&0], vec![(5, 4, 5), (6, 6, 6)]);
    }
}
