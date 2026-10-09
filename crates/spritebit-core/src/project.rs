//! Ein Projekt: mehrere Sprites und die eigenen Paletten.

use std::collections::BTreeMap;

use crate::builtin;
use crate::palette::Palette;
use crate::sprite::Sprite;

#[derive(Clone, Debug)]
pub struct Project {
    pub sprites: Vec<Sprite>,
    /// Eigene Paletten. Eingebaute stehen in [`crate::builtin`].
    pub palettes: Vec<Palette>,
    /// Der Sprite, an dem gerade gearbeitet wird.
    pub current: usize,
    /// Materialien je Palette (für „JSON (Spiel)“): Name → Nummer → Material.
    /// Auch für eingebaute Paletten; fehlende Nummern sind „none“.
    pub materials: BTreeMap<String, BTreeMap<u16, String>>,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            sprites: vec![Sprite::new("Sprite 1", 64, 64).expect("gültige Größe")],
            palettes: Vec::new(),
            current: 0,
            materials: BTreeMap::new(),
        }
    }
}

impl Project {
    /// Palette nach Namen: eigene vor eingebauten; unbekannt → Graustufen.
    pub fn palette(&self, name: &str) -> Palette {
        self.palettes
            .iter()
            .find(|p| p.name == name)
            .cloned()
            .or_else(|| builtin::builtin(name))
            .unwrap_or_else(Palette::grayscale)
    }

    pub fn sprite(&self) -> &Sprite {
        &self.sprites[self.current]
    }

    pub fn sprite_mut(&mut self) -> &mut Sprite {
        &mut self.sprites[self.current]
    }

    /// Palette des aktuellen Sprites.
    pub fn current_palette(&self) -> Palette {
        self.palette(&self.sprite().palette)
    }

    /// Ist `name` eine eigene (bearbeitbare) Palette?
    pub fn is_custom(&self, name: &str) -> bool {
        self.palettes.iter().any(|p| p.name == name)
    }

    /// Ein Palettenname, den es noch nicht gibt (weder eigen noch eingebaut):
    /// `base`, sonst `base2`, `base3` …
    pub fn unique_palette_name(&self, base: &str) -> String {
        let taken = |n: &str| self.is_custom(n) || builtin::builtin(n).is_some();
        if !taken(base) {
            return base.to_string();
        }
        (2..).map(|k| format!("{base}{k}")).find(|n| !taken(n)).expect("irgendwann frei")
    }

    /// Ein Name, den noch kein Sprite trägt: „Sprite 2", „Sprite 3" …
    pub fn fresh_name(&self) -> String {
        let mut n = self.sprites.len() + 1;
        loop {
            let name = format!("Sprite {n}");
            if !self.sprites.iter().any(|s| s.name == name) {
                return name;
            }
            n += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eigene_palette_vor_eingebauter() {
        let mut p = Project::default();
        p.palettes.push(Palette::new("graustufen", vec![[1, 2, 3]]));
        assert_eq!(p.palette("graustufen").get(1), Some([1, 2, 3]));
        assert_eq!(p.palette("golden").name, "golden");
        assert_eq!(p.palette("gibtsnicht").name, "graustufen");
    }

    #[test]
    fn freier_name() {
        let p = Project::default();
        assert_eq!(p.fresh_name(), "Sprite 2");
    }
}
