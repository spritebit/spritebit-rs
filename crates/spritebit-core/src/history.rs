//! Undo/Redo.
//!
//! Ein Schritt sichert den ganzen Sprite — das ist hier billig: ein Klon
//! kopiert nur die Kachel-Zeiger ([`crate::image`]), die Pixel teilen sich
//! Stand und Sicherung, bis gemalt wird. Danach ist nur die bemalte Kachel
//! doppelt vorhanden. So passen viele Schritte auch bei 8192×8192 in den
//! Speicher.
//!
//! Ändert ein Schritt auch eigene Paletten (bearbeiten, umsortieren, Bild →
//! Palette), sichert er die Paletten mit ([`History::record_with_palettes`]);
//! Rückgängig setzt dann beides zurück.

use crate::palette::Palette;
use crate::sprite::Sprite;

#[derive(Debug)]
struct Step {
    sprite: Sprite,
    palettes: Option<Vec<Palette>>,
    /// Laufende Nummer beim Anlegen (siehe [`History::last_step`]).
    serial: u64,
}

#[derive(Debug)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    max: usize,
    serial: u64,
    /// Bis zu welchem Schritt [`History::unseen_step`] schon geliefert hat.
    seen: u64,
}

impl Default for History {
    fn default() -> Self {
        History::new(100)
    }
}

impl History {
    pub fn new(max: usize) -> Self {
        History { undo: Vec::new(), redo: Vec::new(), max: max.max(1), serial: 0, seen: 0 }
    }

    /// Vor einer Änderung aufrufen (Beginn eines Strichs, einer Aktion).
    pub fn record(&mut self, before: &Sprite) {
        self.push(Step { sprite: before.clone(), palettes: None, serial: 0 });
    }

    /// Wie [`History::record`], sichert aber auch die eigenen Paletten.
    pub fn record_with_palettes(&mut self, before: &Sprite, palettes: &[Palette]) {
        self.push(Step { sprite: before.clone(), palettes: Some(palettes.to_vec()), serial: 0 });
    }

    fn push(&mut self, mut step: Step) {
        self.serial += 1;
        step.serial = self.serial;
        self.undo.push(step);
        if self.undo.len() > self.max {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Den zuletzt angelegten Schritt verwerfen — für Aktionen, die am Ende
    /// nichts geändert haben.
    pub fn drop_last(&mut self) {
        self.undo.pop();
    }

    /// Einen Schritt von `from` nehmen und den jetzigen Stand auf `to` legen.
    fn swap(from: &mut Vec<Step>, to: &mut Vec<Step>, current: &mut Sprite, palettes: &mut Vec<Palette>) -> bool {
        let Some(step) = from.pop() else { return false };
        let now_pals = step.palettes.map(|p| std::mem::replace(palettes, p));
        to.push(Step { sprite: std::mem::replace(current, step.sprite), palettes: now_pals, serial: step.serial });
        true
    }

    /// Letzten Schritt zurücknehmen; der jetzige Stand wird zum Redo.
    pub fn undo(&mut self, current: &mut Sprite, palettes: &mut Vec<Palette>) -> bool {
        Self::swap(&mut self.undo, &mut self.redo, current, palettes)
    }

    pub fn redo(&mut self, current: &mut Sprite, palettes: &mut Vec<Palette>) -> bool {
        Self::swap(&mut self.redo, &mut self.undo, current, palettes)
    }

    /// Der Stand vor dem neuesten Schritt — aber nur einmal: ist er schon
    /// abgeholt worden (oder wurde seither rückgängig gemacht/wiederholt),
    /// kommt `None`. Für den Tilemap-Abgleich nach einer Änderung (App).
    pub fn unseen_step(&mut self) -> Option<&Sprite> {
        let s = self.undo.last()?;
        if s.serial <= self.seen {
            return None;
        }
        self.seen = s.serial;
        Some(&s.sprite)
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
        let mut pals: Vec<Palette> = Vec::new();
        h.record(&sp);
        sp.active().set(1, 1, 3);
        assert!(h.undo(&mut sp, &mut pals));
        assert_eq!(sp.cel(0, 0).get(1, 1), 0);
        assert!(h.redo(&mut sp, &mut pals));
        assert_eq!(sp.cel(0, 0).get(1, 1), 3);
        assert!(!h.redo(&mut sp, &mut pals));
    }

    #[test]
    fn paletten_kommen_mit_zurueck() {
        let mut sp = Sprite::new("a", 4, 4).unwrap();
        let mut pals = vec![Palette::new("eigen", vec![[1, 2, 3]])];
        let mut h = History::default();
        h.record_with_palettes(&sp, &pals);
        pals[0].colors[0] = [9, 9, 9];
        sp.active().set(0, 0, 1);
        assert!(h.undo(&mut sp, &mut pals));
        assert_eq!(pals[0].colors[0], [1, 2, 3]);
        assert!(h.redo(&mut sp, &mut pals));
        assert_eq!(pals[0].colors[0], [9, 9, 9]);
        // Ein gewöhnlicher Schritt lässt die Paletten in Ruhe.
        h.record(&sp);
        pals[0].colors[0] = [5, 5, 5];
        h.undo(&mut sp, &mut pals);
        assert_eq!(pals[0].colors[0], [5, 5, 5]);
    }

    #[test]
    fn neuer_schritt_wird_einmal_gemeldet() {
        let mut sp = Sprite::new("a", 4, 4).unwrap();
        let mut h = History::default();
        let mut pals: Vec<Palette> = Vec::new();
        assert!(h.unseen_step().is_none());
        h.record(&sp);
        sp.active().set(0, 0, 1);
        assert!(h.unseen_step().is_some_and(|b| b.cel(0, 0).get(0, 0) == 0));
        assert!(h.unseen_step().is_none(), "nur einmal");
        h.undo(&mut sp, &mut pals);
        h.redo(&mut sp, &mut pals);
        assert!(h.unseen_step().is_none(), "Rückgängig/Wiederholen sind kein neuer Schritt");
        h.record(&sp);
        h.drop_last();
        assert!(h.unseen_step().is_none(), "verworfener Schritt");
    }

    #[test]
    fn neue_aenderung_verwirft_redo() {
        let mut sp = Sprite::new("a", 16, 16).unwrap();
        let mut h = History::default();
        let mut pals: Vec<Palette> = Vec::new();
        h.record(&sp);
        sp.active().set(1, 1, 3);
        h.undo(&mut sp, &mut pals);
        h.record(&sp);
        assert!(!h.can_redo());
    }

    #[test]
    fn begrenzte_zahl_an_schritten() {
        let mut sp = Sprite::new("a", 4, 4).unwrap();
        let mut h = History::new(3);
        let mut pals: Vec<Palette> = Vec::new();
        for i in 1..=5 {
            h.record(&sp);
            sp.active().set(0, 0, i);
        }
        let mut n = 0;
        while h.undo(&mut sp, &mut pals) {
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
        let saved = &h.undo[0].sprite;
        assert_eq!(sp.cel(0, 0).shared_tiles(saved.cel(0, 0)), 31);
    }
}
