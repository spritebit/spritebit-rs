//! Kachelmodus: das Bild ringsum wiederholt, Malen läuft über den Rand (wie
//! js/tiled.js im Web).
//!
//! Für nahtlose Muster (Boden, Wand, Wasser): Neben dem Bild stehen Kopien,
//! waagerecht, senkrecht oder in beide Richtungen. Wer über den Rand hinaus
//! malt, malt auf der anderen Seite weiter — so passen die Kanten von selbst
//! zusammen. Gezeichnet wird in der App (main.rs), hier steht die Rechnung.

use crate::tools::Span;

/// In welche Richtungen das Bild wiederholt wird.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tiled {
    #[default]
    Off,
    /// Ringsum.
    Both,
    /// Nur nebeneinander.
    X,
    /// Nur übereinander.
    Y,
}

impl Tiled {
    /// In der Reihenfolge des Menüs.
    pub const ALL: [Tiled; 4] = [Tiled::Off, Tiled::Both, Tiled::X, Tiled::Y];

    /// Wiederholt waagerecht / senkrecht?
    pub fn axes(self) -> (bool, bool) {
        (matches!(self, Tiled::X | Tiled::Both), matches!(self, Tiled::Y | Tiled::Both))
    }

    pub fn is_on(self) -> bool {
        self != Tiled::Off
    }

    /// Name zum Speichern — wie `state.tiled` im Web.
    pub fn key(self) -> &'static str {
        match self {
            Tiled::Off => "off",
            Tiled::Both => "both",
            Tiled::X => "x",
            Tiled::Y => "y",
        }
    }

    pub fn from_key(s: &str) -> Option<Tiled> {
        Tiled::ALL.into_iter().find(|t| t.key() == s.trim())
    }
}

/// Punkt (x, y) ins Bild `w` × `h` holen: auf den wiederholten Achsen
/// herumgeführt, auf den anderen unverändert (dort fällt er ggf. heraus).
pub fn wrap_point(x: i64, y: i64, w: u32, h: u32, (tx, ty): (bool, bool)) -> (i64, i64) {
    (if tx { x.rem_euclid(w as i64) } else { x }, if ty { y.rem_euclid(h as i64) } else { y })
}

/// Zeilenabschnitte ins Bild holen. Ein Abschnitt, der über den rechten
/// Rand ragt, wird geteilt: der Rest beginnt links wieder. Ist er so breit
/// wie das Bild oder breiter, deckt er die ganze Zeile.
pub fn wrap_spans(spans: Vec<Span>, w: u32, h: u32, (tx, ty): (bool, bool)) -> Vec<Span> {
    if !tx && !ty {
        return spans;
    }
    let (w, h) = (w as i64, h as i64);
    let mut out = Vec::with_capacity(spans.len());
    for (y, a, b) in spans {
        let y = if ty { y.rem_euclid(h) } else { y };
        if !tx {
            out.push((y, a, b));
        } else if b - a + 1 >= w {
            out.push((y, 0, w - 1));
        } else {
            let a2 = a.rem_euclid(w);
            let b2 = a2 + (b - a);
            if b2 < w {
                out.push((y, a2, b2));
            } else {
                out.push((y, a2, w - 1));
                out.push((y, 0, b2 - w));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn achsen_und_namen_wie_im_web() {
        assert_eq!(Tiled::Off.axes(), (false, false));
        assert_eq!(Tiled::X.axes(), (true, false));
        assert_eq!(Tiled::Y.axes(), (false, true));
        assert_eq!(Tiled::Both.axes(), (true, true));
        for t in Tiled::ALL {
            assert_eq!(Tiled::from_key(t.key()), Some(t));
        }
        assert_eq!(Tiled::from_key("quer"), None);
    }

    #[test]
    fn punkt_auf_die_andere_seite() {
        assert_eq!(wrap_point(-1, 20, 16, 8, (true, true)), (15, 4));
        assert_eq!(wrap_point(-1, 20, 16, 8, (true, false)), (15, 20), "senkrecht bleibt er draußen");
        assert_eq!(wrap_point(-17, 8, 16, 8, (true, true)), (15, 0));
    }

    #[test]
    fn abschnitt_ueber_den_rand_wird_geteilt() {
        let both = (true, true);
        assert_eq!(wrap_spans(vec![(2, 14, 17)], 16, 8, both), vec![(2, 14, 15), (2, 0, 1)]);
        assert_eq!(wrap_spans(vec![(-1, -2, 1)], 16, 8, both), vec![(7, 14, 15), (7, 0, 1)]);
        assert_eq!(wrap_spans(vec![(0, -20, 30)], 16, 8, both), vec![(0, 0, 15)], "breiter als das Bild");
        assert_eq!(wrap_spans(vec![(9, 3, 5)], 16, 8, (true, false)), vec![(9, 3, 5)], "nur nebeneinander");
        assert_eq!(wrap_spans(vec![(9, 20, 21)], 16, 8, (false, true)), vec![(1, 20, 21)], "nur übereinander");
        assert_eq!(wrap_spans(vec![(9, 20, 21)], 16, 8, (false, false)), vec![(9, 20, 21)]);
    }
}
