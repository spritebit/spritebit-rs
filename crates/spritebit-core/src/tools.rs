//! Werkzeug-Hilfen ohne Oberfläche.

/// Alle Pixel auf der Linie von (x0, y0) nach (x1, y1), beide Enden
/// eingeschlossen (Bresenham). Damit hat ein schneller Strich keine Lücken:
/// zwischen zwei Mausbewegungen wird die Strecke aufgefüllt.
pub fn line(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<(i64, i64)> {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    let mut out = Vec::with_capacity((dx.max(-dy) + 1) as usize);
    loop {
        out.push((x, y));
        if x == x1 && y == y1 {
            return out;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ein_punkt() {
        assert_eq!(line(3, 3, 3, 3), vec![(3, 3)]);
    }

    #[test]
    fn waagerecht_und_rueckwaerts() {
        assert_eq!(line(0, 0, 3, 0), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(line(3, 0, 0, 0), vec![(3, 0), (2, 0), (1, 0), (0, 0)]);
    }

    #[test]
    fn diagonal_ohne_luecken() {
        let l = line(0, 0, 5, 2);
        assert_eq!(l.first(), Some(&(0, 0)));
        assert_eq!(l.last(), Some(&(5, 2)));
        // Jeder Schritt höchstens ein Pixel in jede Richtung.
        for w in l.windows(2) {
            assert!((w[1].0 - w[0].0).abs() <= 1 && (w[1].1 - w[0].1).abs() <= 1);
        }
        assert_eq!(l.len(), 6);
    }
}
