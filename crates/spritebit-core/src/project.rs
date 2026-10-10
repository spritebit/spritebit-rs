//! Ein Projekt: ein Name, mehrere Sprites und die eigenen Paletten.

use std::collections::BTreeMap;

use crate::builtin;
use crate::palette::Palette;
use crate::sprite::Sprite;

#[derive(Clone, Debug)]
pub struct Project {
    /// Name des Projekts (steht in der Datei, in der Titelleiste und in der
    /// Projektliste der Web-Version). Leer = noch keiner; die App nimmt dann
    /// den Dateinamen.
    pub name: String,
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
            name: String::new(),
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
        self.palettes.iter().find(|p| p.name == name).cloned().or_else(|| builtin::builtin(name)).unwrap_or_else(Palette::grayscale)
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

    /// Nur Sprite `i` als eigenes Projekt — für „Sprite speichern unter …“.
    /// Seine eigene Palette (falls er eine hat) und deren Materialien kommen
    /// mit, damit er anderswo genauso aussieht.
    pub fn single(&self, i: usize) -> Project {
        let sp = self.sprites[i].clone();
        let palettes = self.palettes.iter().filter(|p| p.name == sp.palette).cloned().collect();
        let materials = self.materials.iter().filter(|(k, _)| **k == sp.palette).map(|(k, v)| (k.clone(), v.clone())).collect();
        Project { name: String::new(), sprites: vec![sp], palettes, current: 0, materials }
    }

    /// Die Sprites eines anderen Projekts dazunehmen, ohne hier etwas zu
    /// ersetzen — für „Sprite hinzufügen …“. Eine Palette gleichen Namens
    /// mit denselben Farben wird geteilt; mit anderen Farben (auch eine
    /// eingebaute!) bekommt die neue einen freien Namen, und ihre Sprites
    /// zeigen auf ihn. Gleiche Sprite-Namen bekommen eine Nummer.
    /// Gibt die Stellen der neuen Sprites zurück.
    pub fn merge(&mut self, other: Project) -> std::ops::Range<usize> {
        let mut rename: BTreeMap<String, String> = BTreeMap::new();
        for pal in &other.palettes {
            let existing = self.palettes.iter().find(|p| p.name == pal.name).cloned().or_else(|| builtin::builtin(&pal.name));
            let target = match existing {
                Some(e) if e.colors == pal.colors => pal.name.clone(),
                Some(_) => self.unique_palette_name(&pal.name),
                None => pal.name.clone(),
            };
            if !self.is_custom(&target) && builtin::builtin(&target).is_none() {
                let mut p = pal.clone();
                p.name = target.clone();
                self.palettes.push(p);
            }
            rename.insert(pal.name.clone(), target);
        }
        // Materialien unter dem Namen, unter dem die Palette hier heißt —
        // vorhandene bleiben, wie sie sind.
        for (name, m) in &other.materials {
            let target = rename.get(name).cloned().unwrap_or_else(|| name.clone());
            self.materials.entry(target).or_insert_with(|| m.clone());
        }
        let start = self.sprites.len();
        for mut sp in other.sprites {
            if let Some(t) = rename.get(&sp.palette) {
                sp.palette = t.clone();
            }
            sp.name = self.unique_sprite_name(&sp.name);
            self.sprites.push(sp);
        }
        start..self.sprites.len()
    }

    /// `base`, sonst `base 2`, `base 3` … — je nachdem, was schon vergeben ist.
    fn unique_sprite_name(&self, base: &str) -> String {
        let taken = |n: &str| self.sprites.iter().any(|s| s.name == n);
        if !taken(base) {
            return base.to_string();
        }
        (2..).map(|k| format!("{base} {k}")).find(|n| !taken(n)).expect("irgendwann frei")
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

    fn with_pal(sprite: &str, pal: Palette) -> Project {
        let mut p = Project::default();
        p.sprites[0].name = sprite.into();
        p.sprites[0].palette = pal.name.clone();
        p.palettes.push(pal);
        p
    }

    #[test]
    fn einzeln_nimmt_nur_seine_palette_mit() {
        let mut p = with_pal("held", Palette::new("meine", vec![[1, 2, 3]]));
        p.palettes.push(Palette::new("andere", vec![[9, 9, 9]]));
        p.materials.insert("meine".into(), BTreeMap::from([(1, "stone".into())]));
        p.materials.insert("andere".into(), BTreeMap::new());
        let s = p.single(0);
        assert_eq!(s.sprites.len(), 1);
        assert_eq!(s.palettes.len(), 1);
        assert_eq!(s.palettes[0].name, "meine");
        assert_eq!(s.materials.len(), 1);
    }

    #[test]
    fn hinzufuegen_ersetzt_nichts() {
        let mut p = with_pal("held", Palette::new("meine", vec![[1, 2, 3]]));
        // gleiche Palette, gleicher Name → geteilt; Sprite-Name bekommt eine Nummer
        let r = p.merge(with_pal("held", Palette::new("meine", vec![[1, 2, 3]])));
        assert_eq!(r, 1..2);
        assert_eq!(p.sprites[1].name, "held 2");
        assert_eq!(p.sprites[1].palette, "meine");
        assert_eq!(p.palettes.len(), 1);
        // andere Farben, gleicher Name → eigener Name, die alte bleibt
        p.merge(with_pal("blau", Palette::new("meine", vec![[0, 0, 255]])));
        assert_eq!(p.sprites[2].palette, "meine2");
        assert_eq!(p.palette("meine").get(1), Some([1, 2, 3]));
        assert_eq!(p.palette("meine2").get(1), Some([0, 0, 255]));
    }

    #[test]
    fn eingebaute_palette_wird_nie_ueberschrieben() {
        let mut p = Project::default();
        let before = p.palette("golden");
        p.merge(with_pal("x", Palette::new("golden", vec![[1, 1, 1]])));
        assert_eq!(p.palette("golden"), before);
        assert_ne!(p.sprites[1].palette, "golden");
    }
}
