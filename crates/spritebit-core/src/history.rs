//! Undo/Redo.
//!
//! Ein Schritt sichert den ganzen Sprite — das ist hier billig: ein Klon
//! kopiert nur die Kachel-Zeiger ([`crate::image`]), die Pixel teilen sich
//! Stand und Sicherung, bis gemalt wird. Danach ist nur die bemalte Kachel
//! doppelt vorhanden. So passen viele Schritte auch bei 8192×8192 in den
//! Speicher.

use crate::sprite::Sprite;

#[derive(Debug)]
pub struct History {
    undo: Vec<Sprite>,
    redo: Vec<Sprite>,
    max: usize,
}

impl Default for History {
    fn default() -> Self {
        History::new(100)
    }
}

impl History {
    pub fn new(max: usize) -> Self {
        History { undo: Vec::new(), redo: Vec::new(), max: max.max(1) }
    }

    /// Vor einer Änderung aufrufen (Beginn eines Strichs, einer Aktion).
    pub fn record(&mut self, before: &Sprite) {
        self.undo.push(before.clone());
        if self.undo.len() > self.max {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Letzten Schritt zurücknehmen; `current` wird zum Redo.
    pub fn undo(&mut self, current: &mut Sprite) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(current, prev));
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self, current: &mut Sprite) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(current, next));
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rueckgaengig_und_wiederholen() {
        let mut sp = Sprite::new("a", 16, 16).unwrap();
        let mut h = History::default();
        h.record(&sp);
        sp.active().set(1, 1, 3);
        assert!(h.undo(&mut sp));
        assert_eq!(sp.cel(0, 0).get(1, 1), 0);
        assert!(h.redo(&mut sp));
        assert_eq!(sp.cel(0, 0).get(1, 1), 3);
        assert!(!h.redo(&mut sp));
    }

    #[test]
    fn neue_aenderung_verwirft_redo() {
        let mut sp = Sprite::new("a", 16, 16).unwrap();
        let mut h = History::default();
        h.record(&sp);
        sp.active().set(1, 1, 3);
        h.undo(&mut sp);
        h.record(&sp);
        assert!(!h.can_redo());
    }

    #[test]
    fn begrenzte_zahl_an_schritten() {
        let mut sp = Sprite::new("a", 4, 4).unwrap();
        let mut h = History::new(3);
        for i in 1..=5 {
            h.record(&sp);
            sp.active().set(0, 0, i);
        }
        let mut n = 0;
        while h.undo(&mut sp) {
            n += 1;
        }
        assert_eq!(n, 3);
        assert_eq!(sp.cel(0, 0).get(0, 0), 2, "älteste Schritte sind weg");
    }

    #[test]
    fn sicherung_teilt_unveraenderte_kacheln() {
        let mut sp = Sprite::new("gross", 8192, 8192).unwrap();
        for x in 0..2048 {
            sp.active().set(x, 0, 1); // 32 Kacheln
        }
        let mut h = History::default();
        h.record(&sp);
        sp.active().set(0, 0, 2); // ändert eine davon
        let saved = &h.undo[0];
        assert_eq!(sp.cel(0, 0).shared_tiles(saved.cel(0, 0)), 31);
    }
}
