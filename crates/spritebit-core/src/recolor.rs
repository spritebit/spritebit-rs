//! Eine Farbe überall durch eine andere ersetzen — wie `recolor.js` der Web-Version.
//!
//! * [`replace_color`] — „Farbe ersetzen“ im Panel „Feinschliff“: jede Stelle
//!   mit der alten Farbe bekommt die neue. Verglichen wird nach der FARBE,
//!   nicht nach der Nummer: eine freie Farbe, die genauso aussieht wie
//!   Palettenfarbe 3, wird mit ersetzt. Leer (0) ersetzt nur leere Stellen.
//! * [`targets`] — in welchen Bildern: aktuelles Bild, die Ebene in allen
//!   Frames oder der ganze Sprite (ohne gesperrte Ebenen).
//! * [`fill_all`] — Füll-Eimer mit „Zusammenhängend“ aus: jede Stelle mit
//!   dem Wert unter dem Klick, auch getrennte Flächen.

use crate::image::{Image, Px};
use crate::palette::{Palette, Rgb};
use crate::selection::rgb_of;
use crate::sprite::{ImageId, Sprite};

/// Wo ersetzt wird.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Scope {
    /// Das aktuelle Bild (aktive Ebene im aktuellen Frame).
    #[default]
    Cel,
    /// Die aktive Ebene in allen Frames.
    Layer,
    /// Jede Ebene in jedem Frame, außer gesperrten.
    All,
}

impl Scope {
    pub const ALL: [Scope; 3] = [Scope::Cel, Scope::Layer, Scope::All];

    /// Schlüssel wie im Web (`state.recolorScope`).
    pub fn key(self) -> &'static str {
        match self {
            Scope::Cel => "cel",
            Scope::Layer => "layer",
            Scope::All => "all",
        }
    }

    pub fn from_key(k: &str) -> Option<Scope> {
        Scope::ALL.into_iter().find(|s| s.key() == k)
    }
}

/// Jede Stelle in `img` mit der Farbe von `from` auf `to` setzen; mit `keep`
/// nur Stellen, für die es `true` gibt (Auswahl). Gibt die Zahl geänderter Pixel.
pub fn replace_color(img: &mut Image, from: Px, to: Px, pal: &Palette, free: &[Rgb], keep: Option<&dyn Fn(u32, u32) -> bool>) -> usize {
    let ok = |x: u32, y: u32| keep.is_none_or(|k| k(x, y));
    let mut hits = Vec::new();
    if from == 0 {
        // Leere Stellen liegen auch in nicht belegten Kacheln — ganz durchgehen.
        for y in 0..img.height() {
            for x in 0..img.width() {
                if img.get(x, y) == 0 && to != 0 && ok(x, y) {
                    hits.push((x, y));
                }
            }
        }
    } else {
        let want = rgb_of(from, pal, free);
        let same = |v: Px| v == from || (want.is_some() && rgb_of(v, pal, free) == want);
        hits.extend(img.pixels().filter(|&(x, y, v)| v != to && same(v) && ok(x, y)).map(|(x, y, _)| (x, y)));
    }
    for &(x, y) in &hits {
        img.set(x, y, to);
    }
    hits.len()
}

/// Die Bilder, in denen ersetzt wird. Verknüpfte Zellen kommen nur einmal vor.
pub fn targets(sp: &Sprite, scope: Scope) -> Vec<ImageId> {
    let mut out: Vec<ImageId> = Vec::new();
    for (f, frame) in sp.frames.iter().enumerate() {
        for (l, &id) in frame.cels.iter().enumerate() {
            let take = match scope {
                Scope::Cel => f == sp.frame && l == sp.layer,
                Scope::Layer => l == sp.layer,
                Scope::All => !sp.layers[l].locked,
            };
            if take && !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// Füllen ohne Zusammenhang: jede Stelle, die in `key` (sonst im Bild selbst)
/// den Wert unter (x, y) hat, bekommt `value`. Gibt die Zahl geänderter Pixel.
pub fn fill_all(img: &mut Image, key: Option<&[u32]>, x: i64, y: i64, value: Px) -> usize {
    let (w, h) = (img.width(), img.height());
    if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
        return 0;
    }
    let (x, y) = (x as u32, y as u32);
    let mut hits = Vec::new();
    match key {
        Some(k) if k.len() == (w * h) as usize => {
            let target = k[(y * w + x) as usize];
            for cy in 0..h {
                for cx in 0..w {
                    if k[(cy * w + cx) as usize] == target && img.get(cx, cy) != value {
                        hits.push((cx, cy));
                    }
                }
            }
        }
        Some(_) => return 0,
        None => {
            let target = img.get(x, y);
            if target == value {
                return 0;
            }
            if target == 0 {
                for cy in 0..h {
                    for cx in 0..w {
                        if img.get(cx, cy) == 0 {
                            hits.push((cx, cy));
                        }
                    }
                }
            } else {
                hits.extend(img.pixels().filter(|&(_, _, v)| v == target).map(|(x, y, _)| (x, y)));
            }
        }
    }
    for &(cx, cy) in &hits {
        img.set(cx, cy, value);
    }
    hits.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::FREE_BASE;

    fn pal() -> Palette {
        Palette::new("t", vec![[0, 0, 0], [255, 0, 0], [0, 255, 0]])
    }

    #[test]
    fn ersetzt_auch_gleich_aussehende_freie_farbe() {
        let mut img = Image::new(3, 2);
        img.set(0, 0, 2);
        img.set(2, 0, 2);
        img.set(0, 1, FREE_BASE);
        img.set(1, 1, 1);
        let free = [[255, 0, 0]];
        assert_eq!(replace_color(&mut img, 2, 3, &pal(), &free, None), 3);
        assert_eq!((img.get(0, 0), img.get(2, 0), img.get(0, 1), img.get(1, 1)), (3, 3, 3, 1));
        assert_eq!(replace_color(&mut img, 2, 3, &pal(), &free, None), 0, "nichts mehr zu tun");
    }

    #[test]
    fn leer_nur_leere_stellen_und_auswahl_begrenzt() {
        let mut img = Image::new(2, 2);
        img.set(1, 0, 1);
        let keep = |_x: u32, y: u32| y == 0;
        assert_eq!(replace_color(&mut img, 0, 2, &pal(), &[], Some(&keep)), 1);
        assert_eq!((img.get(0, 0), img.get(1, 0), img.get(0, 1)), (2, 1, 0));
    }

    #[test]
    fn bereiche_bild_ebene_alles_ohne_gesperrte() {
        let mut sp = Sprite::new("t", 4, 4).unwrap();
        sp.add_layer(1, "b");
        sp.add_frame(0, false);
        sp.add_frame(1, false);
        sp.link(0, 1, 0); // Ebene 0: Frame 1 zeigt dasselbe Bild wie Frame 0
        sp.frame = 0;
        sp.layer = 0;
        assert_eq!(targets(&sp, Scope::Cel), vec![sp.frames[0].cels[0]]);
        assert_eq!(targets(&sp, Scope::Layer).len(), 2, "verknüpftes Bild nur einmal");
        sp.layers[1].locked = true;
        assert_eq!(targets(&sp, Scope::All).len(), 2, "gesperrte Ebene bleibt");
        sp.layers[1].locked = false;
        assert_eq!(targets(&sp, Scope::All).len(), 5);
        assert_eq!(Scope::from_key("layer"), Some(Scope::Layer));
    }

    #[test]
    fn fuellen_ohne_zusammenhang_erreicht_getrennte_flaechen() {
        let mut img = Image::new(3, 3);
        for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
            img.set(x, y, 1);
        }
        assert_eq!(fill_all(&mut img, None, 0, 0, 2), 4);
        assert_eq!((img.get(2, 2), img.get(1, 1)), (2, 0));
        let mut img2 = Image::new(3, 1);
        let key = [5, 5, 0];
        assert_eq!(fill_all(&mut img2, Some(&key), 0, 0, 3), 2, "Grenzen aus der Vorlage");
        assert_eq!((img2.get(0, 0), img2.get(1, 0), img2.get(2, 0)), (3, 3, 0));
    }
}
