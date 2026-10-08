//! Ein Projekt: mehrere Sprites und die eigenen Paletten.

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
}

impl Default for Project {
    fn default() -> Self {
        Project {
            sprites: vec![Sprite::new("Sprite 1", 64, 64).expect("gültige Größe")],
            palettes: Vec::new(),
            current: 0,
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
