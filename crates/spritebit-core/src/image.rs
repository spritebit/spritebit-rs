//! Ein Bild (eine Zelle: eine Ebene in einem Frame), in Kacheln abgelegt.
//!
//! Große Zeichenflächen sind meist zum größten Teil leer. Darum liegt ein
//! Bild in Kacheln von [`TILE`]×[`TILE`] Pixeln, und eine Kachel gibt es erst,
//! wenn auf ihr gemalt wird. Eine 8192×8192-Fläche mit einer kleinen Figur
//! darauf braucht so nur ein paar Kilobyte.
//!
//! Die Kacheln stecken in einem [`Arc`]. Ein [`Clone`] des Bildes kopiert
//! darum nur Zeiger — und erst wenn danach eine Kachel geändert wird, bekommt
//! sie eine eigene Kopie (Copy-on-Write). Das ist die Grundlage fürs Undo:
//! ein gesicherter Stand teilt sich alle unveränderten Kacheln mit dem
//! aktuellen, ein Pinselstrich kostet nur die Kacheln, die er berührt.

use std::sync::Arc;

/// Kantenlänge einer Kachel in Pixeln.
pub const TILE: u32 = 64;
const TILE_PX: usize = (TILE * TILE) as usize;

/// Ein Pixel.
///
/// * `0` — transparent
/// * `1..=255` — Farbe aus der Palette (Nummer)
/// * `256..` — freie Farbe: Eintrag `px - 256` in [`crate::Sprite::free`]
pub type Px = u16;

/// Ab diesem Wert ist ein Pixel eine freie Farbe statt einer Palettenfarbe.
pub const FREE_BASE: Px = 256;

type Tile = Arc<Vec<Px>>;

/// Ein Bild in Kacheln. Siehe Modul-Beschreibung.
#[derive(Clone, Debug)]
pub struct Image {
    width: u32,
    height: u32,
    /// Kacheln zeilenweise; `None` = noch nie bemalt (alles transparent).
    tiles: Vec<Option<Tile>>,
}

impl Image {
    /// Leeres Bild — belegt außer der Kachel-Liste keinen Speicher.
    pub fn new(width: u32, height: u32) -> Self {
        let n = (Self::cols_for(width) * Self::rows_for(height)) as usize;
        Image { width, height, tiles: vec![None; n] }
    }

    fn cols_for(w: u32) -> u32 {
        w.div_ceil(TILE)
    }
    fn rows_for(h: u32) -> u32 {
        h.div_ceil(TILE)
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Nummer der Kachel und Stelle darin — oder `None` außerhalb des Bildes.
    fn locate(&self, x: u32, y: u32) -> Option<(usize, usize)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let cols = Self::cols_for(self.width);
        let tile = (y / TILE) * cols + x / TILE;
        let inner = (y % TILE) * TILE + x % TILE;
        Some((tile as usize, inner as usize))
    }

    /// Pixel lesen. Außerhalb des Bildes: transparent.
    pub fn get(&self, x: u32, y: u32) -> Px {
        match self.locate(x, y) {
            Some((t, i)) => self.tiles[t].as_ref().map_or(0, |tile| tile[i]),
            None => 0,
        }
    }

    /// Pixel setzen. Außerhalb des Bildes passiert nichts. Transparent auf
    /// eine leere Kachel legt keine Kachel an.
    pub fn set(&mut self, x: u32, y: u32, v: Px) {
        let Some((t, i)) = self.locate(x, y) else { return };
        if let Some(tile) = &mut self.tiles[t] {
            // make_mut kopiert die Kachel nur, wenn ein anderer Stand
            // (z. B. ein Undo-Schritt) sie noch mitbenutzt.
            Arc::make_mut(tile)[i] = v;
            return;
        }
        if v != 0 {
            let mut tile = vec![0; TILE_PX];
            tile[i] = v;
            self.tiles[t] = Some(Arc::new(tile));
        }
    }

    /// Enthält das Bild überhaupt einen Pixel?
    pub fn is_empty(&self) -> bool {
        self.tiles.iter().flatten().all(|t| t.iter().all(|&p| p == 0))
    }

    /// Wie viele Kacheln tatsächlich Speicher belegen.
    pub fn allocated_tiles(&self) -> usize {
        self.tiles.iter().filter(|t| t.is_some()).count()
    }

    /// Kacheln, die ganz transparent geworden sind, wieder freigeben.
    pub fn compact(&mut self) {
        for t in &mut self.tiles {
            if t.as_ref().is_some_and(|tile| tile.iter().all(|&p| p == 0)) {
                *t = None;
            }
        }
    }

    /// Die belegten Kacheln: (Nummer, Pixel) — zum Speichern.
    pub fn tiles(&self) -> impl Iterator<Item = (usize, &[Px])> {
        self.tiles.iter().enumerate().filter_map(|(i, t)| t.as_ref().map(|t| (i, t.as_slice())))
    }

    /// Anzahl Kacheln insgesamt (belegt oder nicht).
    pub fn tile_slots(&self) -> usize {
        self.tiles.len()
    }

    /// Kachel `index` mit diesen Pixeln belegen — zum Laden. Falsche Nummer
    /// oder Länge: `false`, nichts geändert.
    pub fn put_tile(&mut self, index: usize, data: Vec<Px>) -> bool {
        if index >= self.tiles.len() || data.len() != TILE_PX {
            return false;
        }
        self.tiles[index] = Some(Arc::new(data));
        true
    }

    /// Wie viele Kacheln sich zwei Bilder teilen (gleicher Speicher, nicht
    /// nur gleicher Inhalt) — für Tests und Speicher-Statistik.
    pub fn shared_tiles(&self, other: &Image) -> usize {
        self.tiles
            .iter()
            .zip(&other.tiles)
            .filter(|(a, b)| matches!((a, b), (Some(a), Some(b)) if Arc::ptr_eq(a, b)))
            .count()
    }

    /// Gleiche Pixel? (Größe und Inhalt; leere Kachel = transparente Kachel)
    pub fn same_pixels(&self, other: &Image) -> bool {
        if self.width != other.width || self.height != other.height {
            return false;
        }
        self.tiles.iter().zip(&other.tiles).all(|(a, b)| match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a == b,
            (Some(t), None) | (None, Some(t)) => t.iter().all(|&p| p == 0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leeres_bild_belegt_keine_kacheln() {
        let img = Image::new(8192, 8192);
        assert_eq!(img.allocated_tiles(), 0);
        assert!(img.is_empty());
        assert_eq!(img.get(5000, 5000), 0);
    }

    #[test]
    fn malen_legt_nur_die_betroffene_kachel_an() {
        let mut img = Image::new(8192, 8192);
        img.set(100, 100, 3);
        img.set(101, 100, 3);
        assert_eq!(img.allocated_tiles(), 1);
        assert_eq!(img.get(100, 100), 3);
        assert_eq!(img.get(99, 100), 0);
    }

    #[test]
    fn transparent_auf_leer_legt_nichts_an() {
        let mut img = Image::new(256, 256);
        img.set(10, 10, 0);
        assert_eq!(img.allocated_tiles(), 0);
    }

    #[test]
    fn ausserhalb_wird_ignoriert() {
        let mut img = Image::new(10, 10);
        img.set(10, 0, 5);
        img.set(0, 10, 5);
        assert_eq!(img.get(10, 0), 0);
        assert!(img.is_empty());
    }

    #[test]
    fn ungerade_groessen_und_randkacheln() {
        let mut img = Image::new(65, 1);
        img.set(64, 0, 7);
        assert_eq!(img.get(64, 0), 7);
        assert_eq!(img.allocated_tiles(), 1);
    }

    #[test]
    fn kopie_teilt_kacheln_bis_zur_aenderung() {
        let mut a = Image::new(512, 512);
        a.set(1, 1, 2); // Kachel 0
        a.set(200, 1, 2); // Kachel 3
        let b = a.clone();
        assert_eq!(a.shared_tiles(&b), 2);
        a.set(2, 2, 9); // nur Kachel 0 wird kopiert
        assert_eq!(a.shared_tiles(&b), 1);
        assert_eq!(b.get(2, 2), 0, "die Kopie bleibt, wie sie war");
        assert_eq!(a.get(2, 2), 9);
    }

    #[test]
    fn compact_gibt_leere_kacheln_frei() {
        let mut img = Image::new(128, 128);
        img.set(0, 0, 1);
        img.set(0, 0, 0);
        assert_eq!(img.allocated_tiles(), 1);
        img.compact();
        assert_eq!(img.allocated_tiles(), 0);
    }

    #[test]
    fn same_pixels_vergleicht_inhalt() {
        let mut a = Image::new(100, 100);
        let mut b = Image::new(100, 100);
        a.set(5, 5, 1);
        assert!(!a.same_pixels(&b));
        b.set(5, 5, 1);
        assert!(a.same_pixels(&b));
        a.set(5, 5, 0);
        b.set(5, 5, 0);
        assert!(a.same_pixels(&Image::new(100, 100)), "leere Kachel = transparente Kachel");
    }
}
