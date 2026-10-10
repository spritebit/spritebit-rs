//! Paletten: bis zu 255 Farben, Nummer 0 ist immer transparent.

use std::collections::BTreeMap;

/// Eine Farbe ohne Alpha.
pub type Rgb = [u8; 3];

/// Größte Zahl an Farben in einer Palette (plus Transparent = 256 Werte).
pub const MAX_COLORS: usize = 255;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub name: String,
    /// `colors[i - 1]` ist Farbe Nummer `i` (1..=len).
    pub colors: Vec<Rgb>,
    /// Eigene Namen einzelner Farben (Nummer → Name), z. B. „Haut“.
    /// Ohne Eintrag heißt die Farbe schlicht „Farbe 3“.
    pub names: BTreeMap<u16, String>,
}

impl Palette {
    pub fn new(name: impl Into<String>, colors: Vec<Rgb>) -> Self {
        let mut colors = colors;
        colors.truncate(MAX_COLORS);
        Palette { name: name.into(), colors, names: BTreeMap::new() }
    }

    /// Farbe Nummer `index` — `None` für 0 (transparent) und unbelegte Plätze.
    pub fn get(&self, index: u16) -> Option<Rgb> {
        if index == 0 {
            return None;
        }
        self.colors.get(index as usize - 1).copied()
    }

    /// Eigener Name der Farbe Nummer `index`, falls vergeben.
    pub fn name_of(&self, index: u16) -> Option<&str> {
        self.names.get(&index).map(String::as_str).filter(|s| !s.is_empty())
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    /// Die Standard-Palette der Web-Version („graustufen").
    pub fn grayscale() -> Self {
        Palette::new(
            "graustufen",
            vec![
                [0xff, 0xff, 0xff],
                [0xcc, 0xcc, 0xcc],
                [0x99, 0x99, 0x99],
                [0x66, 0x66, 0x66],
                [0x00, 0x00, 0x00],
                [0xff, 0x80, 0x80],
                [0xff, 0xff, 0xff],
                [0x80, 0x80, 0x80],
                [0x40, 0x40, 0x40],
            ],
        )
    }
}

/// `#rrggbb` → Farbe. Groß- und Kleinschreibung egal, `#rgb` geht auch.
pub fn parse_hex(s: &str) -> Option<Rgb> {
    let h = s.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    match h.len() {
        6 => Some([byte(0)?, byte(2)?, byte(4)?]),
        3 => {
            let n = |i: usize| u8::from_str_radix(h.get(i..i + 1)?, 16).ok().map(|v| v * 17);
            Some([n(0)?, n(1)?, n(2)?])
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nummer_null_ist_transparent() {
        let p = Palette::grayscale();
        assert_eq!(p.get(0), None);
        assert_eq!(p.get(1), Some([0xff, 0xff, 0xff]));
        assert_eq!(p.get(200), None);
    }

    #[test]
    fn hoechstens_255_farben() {
        let p = Palette::new("x", vec![[0, 0, 0]; 300]);
        assert_eq!(p.len(), MAX_COLORS);
    }

    #[test]
    fn hex_lesen() {
        assert_eq!(parse_hex("#ABCDEF"), Some([0xab, 0xcd, 0xef]));
        assert_eq!(parse_hex("#fff"), Some([255, 255, 255]));
        assert_eq!(parse_hex("abcdef"), None);
        assert_eq!(parse_hex("#12345"), None);
    }
}
