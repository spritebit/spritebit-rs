//! Ebenenmasken — wie `mask.js` der Web-Version: Teile einer Ebene
//! ausblenden, ohne sie zu löschen. Eine Maske je Ebene, gültig für alle
//! Frames.
//!
//! `hide` ist ein Bild in Sprite-Größe: 0 = sichtbar, alles andere =
//! ausgeblendet. Das ist Absicht — im Modus „Maske bearbeiten“ malen die
//! Werkzeuge direkt hinein ([`crate::Sprite::active`]): Malen schreibt eine
//! Farbe (blendet aus), Radieren schreibt 0 (blendet ein).
//!
//! Gespeichert wird die Maske als Lauflängen — abwechselnd sichtbar /
//! ausgeblendet, zeilenweise, mit „sichtbar“ beginnend; genau wie im Web.

use serde_json::{json, Value};

use crate::image::Image;

#[derive(Clone, Debug)]
pub struct Mask {
    /// `false` = vorübergehend aus (alles sichtbar).
    pub on: bool,
    pub hide: Image,
}

// Gleich = gleich geschaltet und dieselben Pixel ausgeblendet (Image selbst
// vergleicht man über same_pixels — auf den Wert, nicht auf die Kacheln).
impl PartialEq for Mask {
    fn eq(&self, other: &Self) -> bool {
        self.on == other.on && self.hide.same_pixels(&other.hide)
    }
}

impl Mask {
    /// Neue Maske: alles sichtbar.
    pub fn new(width: u32, height: u32) -> Self {
        Mask { on: true, hide: Image::new(width, height) }
    }

    /// Ist Pixel (x, y) gerade ausgeblendet?
    pub fn hides(&self, x: u32, y: u32) -> bool {
        self.on && self.hide.get(x, y) != 0
    }

    /// `{ on, runs }` wie im Web-Projekt.
    pub fn to_json(&self) -> Value {
        let (w, h) = (self.hide.width(), self.hide.height());
        let mut runs: Vec<u64> = Vec::new();
        let (mut hidden, mut n) = (false, 0u64);
        for y in 0..h {
            for x in 0..w {
                let hd = self.hide.get(x, y) != 0;
                if hd == hidden {
                    n += 1;
                } else {
                    runs.push(n);
                    hidden = hd;
                    n = 1;
                }
            }
        }
        runs.push(n);
        json!({ "on": self.on, "runs": runs })
    }

    /// Aus `{ on, runs }` oder `{ on, hide: [[…]] }`. Passt die Größe nicht,
    /// gibt es keine Maske.
    pub fn from_json(v: &Value, width: u32, height: u32) -> Option<Mask> {
        let on = v.get("on").and_then(Value::as_bool) != Some(false);
        let mut hide = Image::new(width, height);
        if let Some(rows) = v.get("hide").and_then(Value::as_array) {
            if rows.len() != height as usize {
                return None;
            }
            for (y, row) in rows.iter().enumerate() {
                let row = row.as_array().filter(|r| r.len() == width as usize)?;
                for (x, c) in row.iter().enumerate() {
                    let set = match c {
                        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
                        Value::String(s) => !s.is_empty(),
                        Value::Bool(b) => *b,
                        _ => false,
                    };
                    if set {
                        hide.set(x as u32, y as u32, 1);
                    }
                }
            }
            return Some(Mask { on, hide });
        }
        let runs: Vec<u64> = v.get("runs")?.as_array()?.iter().map(Value::as_u64).collect::<Option<_>>()?;
        if runs.iter().sum::<u64>() != width as u64 * height as u64 {
            return None;
        }
        let mut i: u64 = 0;
        for (k, &n) in runs.iter().enumerate() {
            if k % 2 == 1 {
                for p in i..i + n {
                    hide.set((p % width as u64) as u32, (p / width as u64) as u32, 1);
                }
            }
            i += n;
        }
        Some(Mask { on, hide })
    }
}

/// Bild, wie man es sieht: ausgeblendete Pixel werden 0. Ohne (aktive)
/// Maske eine einfache Kopie (teilt die Kacheln).
pub fn masked(img: &Image, mask: Option<&Mask>) -> Image {
    let mut out = img.clone();
    if let Some(m) = mask.filter(|m| m.on) {
        let hidden: Vec<(u32, u32)> = m.hide.pixels().map(|(x, y, _)| (x, y)).collect();
        for (x, y) in hidden {
            if out.get(x, y) != 0 {
                out.set(x, y, 0);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neue_maske_blendet_nichts_aus() {
        let m = Mask::new(3, 2);
        assert!(!m.hides(1, 1));
    }

    #[test]
    fn jeder_wert_blendet_aus_ausgeschaltet_nicht() {
        let mut m = Mask::new(3, 2);
        m.hide.set(1, 0, 7);
        assert!(m.hides(1, 0));
        m.on = false;
        assert!(!m.hides(1, 0));
    }

    #[test]
    fn laufängen_wie_im_web() {
        let mut m = Mask::new(3, 2);
        m.hide.set(2, 0, 5);
        m.hide.set(0, 1, 5);
        assert_eq!(m.to_json(), json!({ "on": true, "runs": [2, 2, 2] }));
        let back = Mask::from_json(&m.to_json(), 3, 2).unwrap();
        assert!(back.hides(2, 0) && back.hides(0, 1) && !back.hides(1, 1));
        let off = Mask {
            on: false,
            hide: {
                let mut i = Image::new(2, 1);
                i.set(0, 0, 1);
                i.set(1, 0, 1);
                i
            },
        };
        assert_eq!(off.to_json(), json!({ "on": false, "runs": [0, 2] }), "beginnt immer mit sichtbar");
    }

    #[test]
    fn falsche_groesse_keine_maske() {
        assert!(Mask::from_json(&json!({ "on": true, "runs": [5] }), 3, 2).is_none());
        assert!(Mask::from_json(&json!({ "on": true, "hide": [[0, 0]] }), 3, 2).is_none());
        assert!(Mask::from_json(&json!({ "on": true, "hide": [[0, 1, 0], [0, 0, "#ff0000"]] }), 3, 2).is_some_and(|m| m.hides(1, 0) && m.hides(2, 1)));
    }

    #[test]
    fn maskiertes_bild() {
        let mut img = Image::new(3, 1);
        for x in 0..3 {
            img.set(x, 0, 4);
        }
        let mut m = Mask::new(3, 1);
        m.hide.set(1, 0, 1);
        let out = masked(&img, Some(&m));
        assert_eq!((out.get(0, 0), out.get(1, 0), out.get(2, 0)), (4, 0, 4));
        assert_eq!(img.get(1, 0), 4, "Original bleibt");
    }
}
