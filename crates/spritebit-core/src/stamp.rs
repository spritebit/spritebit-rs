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

use crate::image::{Px, FREE_BASE};
use crate::palette::{parse_hex, Palette, Rgb};
use crate::selection::{rgb_of, Clip};
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

// ── Gespeicherte Pinsel ─────────────────────────────────────────────
// Wie brushlib.js im Web: für alle Projekte, nach der FARBE gespeichert
// ("#rrggbb", null = leer) — in einem Sprite mit anderer Palette sieht der
// Pinsel so gleich aus. Datei brushes.json im Einstellungsordner, im selben
// Format wie im Web: [{ "w": 2, "h": 1, "px": ["#ff0000", null] }].

/// Höchstens so viele Pinsel — der älteste fällt dann heraus.
pub const MAX_BRUSHES: usize = 32;

/// Ein gespeicherter Pinsel: Farben Zeile für Zeile, `None` = leer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedBrush {
    pub w: u32,
    pub h: u32,
    pub px: Vec<Option<Rgb>>,
}

impl SavedBrush {
    /// Pinsel → gespeicherte Form (Farben statt Nummern).
    pub fn from_clip(clip: &Clip, pal: &Palette, free: &[Rgb]) -> SavedBrush {
        SavedBrush { w: clip.w, h: clip.h, px: clip.data.iter().map(|&v| rgb_of(v, pal, free)).collect() }
    }

    /// Zurück in einen Pinsel: leere Palette, jede Farbe eine freie Farbe.
    pub fn to_clip(&self) -> (Clip, Palette, Vec<Rgb>) {
        let mut free: Vec<Rgb> = Vec::new();
        let data = self
            .px
            .iter()
            .map(|c| match c {
                None => 0,
                Some(c) => {
                    let i = free.iter().position(|f| f == c).unwrap_or_else(|| {
                        free.push(*c);
                        free.len() - 1
                    });
                    FREE_BASE + i as Px
                }
            })
            .collect();
        (Clip { w: self.w, h: self.h, data }, Palette::new("", Vec::new()), free)
    }
}

/// Pinsel vorne in die Liste; ein gleicher wandert nach vorn statt doppelt.
pub fn add_brush(list: &mut Vec<SavedBrush>, b: SavedBrush) {
    list.retain(|x| x != &b);
    list.insert(0, b);
    list.truncate(MAX_BRUSHES);
}

/// Als JSON (Format wie im Web).
pub fn brushes_json(list: &[SavedBrush]) -> serde_json::Value {
    let hex = |c: &Option<Rgb>| match c {
        Some([r, g, b]) => serde_json::Value::String(format!("#{r:02x}{g:02x}{b:02x}")),
        None => serde_json::Value::Null,
    };
    serde_json::Value::Array(list.iter().map(|b| serde_json::json!({ "w": b.w, "h": b.h, "px": b.px.iter().map(hex).collect::<Vec<_>>() })).collect())
}

/// Aus JSON lesen — Kaputtes fällt heraus.
pub fn parse_brushes(v: &serde_json::Value) -> Vec<SavedBrush> {
    let mut out = Vec::new();
    for b in v.as_array().into_iter().flatten() {
        let (Some(w), Some(h)) = (b["w"].as_u64(), b["h"].as_u64()) else { continue };
        let Some(arr) = b["px"].as_array() else { continue };
        if w == 0 || h == 0 || w * h > 512 * 512 || arr.len() as u64 != w * h {
            continue;
        }
        let px: Option<Vec<Option<Rgb>>> = arr
            .iter()
            .map(|c| match c {
                serde_json::Value::Null => Some(None),
                serde_json::Value::String(s) => parse_hex(s).map(Some),
                _ => None,
            })
            .collect();
        match px {
            Some(px) if px.iter().any(Option::is_some) => out.push(SavedBrush { w: w as u32, h: h as u32, px }),
            _ => {}
        }
        if out.len() == MAX_BRUSHES {
            break;
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

    #[test]
    fn gespeichert_nach_farbe_und_zurueck() {
        let pal = Palette::new("t", vec![[0, 0, 0], [255, 0, 0]]);
        let c = Clip { w: 2, h: 2, data: vec![2, 0, FREE_BASE, 1] };
        let b = SavedBrush::from_clip(&c, &pal, &[[0, 255, 0]]);
        assert_eq!(b.px, vec![Some([255, 0, 0]), None, Some([0, 255, 0]), Some([0, 0, 0])]);
        let v = brushes_json(std::slice::from_ref(&b));
        assert_eq!(v[0]["px"][0], "#ff0000");
        assert_eq!(parse_brushes(&v), vec![b.clone()], "gleiches Format hin und zurück");
        let (clip, p, free) = b.to_clip();
        assert!(p.is_empty());
        assert_eq!(clip.data, vec![FREE_BASE, 0, FREE_BASE + 1, FREE_BASE + 2]);
        assert_eq!(free, vec![[255, 0, 0], [0, 255, 0], [0, 0, 0]]);
    }

    #[test]
    fn liste_ohne_doppelte_und_kaputtes_faellt_heraus() {
        let a = SavedBrush { w: 1, h: 1, px: vec![Some([0, 0, 0])] };
        let b = SavedBrush { w: 1, h: 1, px: vec![Some([9, 9, 9])] };
        let mut list = vec![a.clone(), b.clone()];
        add_brush(&mut list, b.clone());
        assert_eq!(list, vec![b, a]);
        let v = serde_json::json!([{ "w": 1, "h": 1, "px": [null] }, { "w": 2, "h": 1, "px": ["#000000"] }, "x", { "w": 1, "h": 1, "px": ["#00ff00"] }]);
        assert_eq!(parse_brushes(&v).len(), 1);
    }
}
