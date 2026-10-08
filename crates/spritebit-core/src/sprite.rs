//! Ein Sprite: Ebenen × Frames, wie in der Timeline.
//!
//! Die Bilder liegen in einer Liste ([`Sprite::images`]); jede Zelle
//! (Frame `f`, Ebene `l`) zeigt mit einer Nummer darauf. Zeigen zwei Zellen
//! auf dieselbe Nummer, sind sie **verknüpft**: malt man in einer, ändert es
//! sich in beiden — wie in Aseprite und in der Web-Version.

use crate::image::{Image, Px, FREE_BASE};
use crate::palette::Rgb;

/// Nummer eines Bildes in [`Sprite::images`].
pub type ImageId = usize;

/// Größte Kantenlänge einer Zeichenfläche.
pub const MAX_SIDE: u32 = 8192;

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    /// 0.0–1.0
    pub opacity: f32,
    /// Durchgehend: neue Frames verknüpfen hier mit dem vorigen.
    pub continuous: bool,
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Self {
        Layer { name: name.into(), visible: true, locked: false, opacity: 1.0, continuous: false }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Je Ebene ein Bild (Index wie in [`Sprite::layers`], 0 = unterste).
    pub cels: Vec<ImageId>,
    /// 0 = nach den fps des Sprites.
    pub duration_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
    PingPong,
}

/// Benannter Abschnitt der Animation, z. B. „Laufen" in den Frames 0–7.
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    pub name: String,
    pub from: usize,
    pub to: usize,
    pub color: Rgb,
    pub direction: Direction,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SpriteError {
    /// Breite oder Höhe 0 oder über [`MAX_SIDE`].
    BadSize { width: u32, height: u32 },
}

#[derive(Clone, Debug)]
pub struct Sprite {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Name der Palette (die Paletten liegen im Projekt).
    pub palette: String,
    pub fps: u32,
    /// Von unten nach oben.
    pub layers: Vec<Layer>,
    pub frames: Vec<Frame>,
    pub images: Vec<Image>,
    /// Freie Farben ohne Paletten-Platz; im Bild als `FREE_BASE + i`.
    pub free: Vec<Rgb>,
    pub tags: Vec<Tag>,
    /// Aktiver Frame und aktive Ebene.
    pub frame: usize,
    pub layer: usize,
}

impl Sprite {
    /// Neuer Sprite mit einer leeren Ebene in einem Frame.
    pub fn new(name: impl Into<String>, width: u32, height: u32) -> Result<Self, SpriteError> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(SpriteError::BadSize { width, height });
        }
        Ok(Sprite {
            name: name.into(),
            width,
            height,
            palette: "graustufen".into(),
            fps: 8,
            layers: vec![Layer::new("Ebene 1")],
            frames: vec![Frame { cels: vec![0], duration_ms: 0 }],
            images: vec![Image::new(width, height)],
            free: Vec::new(),
            tags: Vec::new(),
            frame: 0,
            layer: 0,
        })
    }

    fn blank(&mut self) -> ImageId {
        self.images.push(Image::new(self.width, self.height));
        self.images.len() - 1
    }

    /// Bild der Zelle (Frame `f`, Ebene `l`).
    pub fn cel(&self, f: usize, l: usize) -> &Image {
        &self.images[self.frames[f].cels[l]]
    }

    /// Zum Malen. Ist die Zelle verknüpft, ändert sich das Bild in allen
    /// verknüpften Frames — so ist es gemeint.
    pub fn cel_mut(&mut self, f: usize, l: usize) -> &mut Image {
        let id = self.frames[f].cels[l];
        &mut self.images[id]
    }

    /// Das Bild, in das gerade gemalt wird.
    pub fn active(&mut self) -> &mut Image {
        self.cel_mut(self.frame, self.layer)
    }

    /// Teilt die Zelle ihr Bild mit einem anderen Frame?
    pub fn is_linked(&self, f: usize, l: usize) -> bool {
        let id = self.frames[f].cels[l];
        self.frames.iter().enumerate().any(|(k, fr)| k != f && fr.cels[l] == id)
    }

    /// Neue leere Ebene an Stelle `at` (0 = unterste), in jedem Frame.
    pub fn add_layer(&mut self, at: usize, name: impl Into<String>) {
        let at = at.min(self.layers.len());
        self.layers.insert(at, Layer::new(name));
        for f in 0..self.frames.len() {
            let id = self.blank();
            self.frames[f].cels.insert(at, id);
        }
        self.layer = at;
    }

    /// Neuer Frame hinter `after`. Durchgehende Ebenen verknüpfen mit dem
    /// Frame davor; die übrigen sind leer — oder, mit `duplicate`, eine
    /// eigene Kopie. Tags wachsen mit, wenn mitten hinein oder ans Ende
    /// eingefügt wird.
    pub fn add_frame(&mut self, after: usize, duplicate: bool) -> usize {
        let after = after.min(self.frames.len() - 1);
        let src = self.frames[after].clone();
        let mut cels = Vec::with_capacity(src.cels.len());
        for (l, &id) in src.cels.iter().enumerate() {
            let new_id = if self.layers[l].continuous {
                id
            } else if duplicate {
                // Kopie teilt sich die Kacheln, bis jemand darin malt.
                self.images.push(self.images[id].clone());
                self.images.len() - 1
            } else {
                self.blank()
            };
            cels.push(new_id);
        }
        let at = after + 1;
        self.frames.insert(at, Frame { cels, duration_ms: if duplicate { src.duration_ms } else { 0 } });
        for t in &mut self.tags {
            if at <= t.from {
                t.from += 1;
                t.to += 1;
            } else if at <= t.to + 1 {
                t.to += 1;
            }
        }
        self.frame = at;
        at
    }

    /// Frame `f` löschen. Der letzte Frame bleibt (`false`). Tags rücken
    /// nach; ein Tag ohne Frame verschwindet.
    pub fn delete_frame(&mut self, f: usize) -> bool {
        if self.frames.len() <= 1 || f >= self.frames.len() {
            return false;
        }
        self.frames.remove(f);
        for t in &mut self.tags {
            if f < t.from {
                t.from -= 1;
                t.to -= 1;
            } else if f <= t.to {
                // to < from heißt danach: der Tag ist leer.
                t.to = t.to.wrapping_sub(1);
            }
        }
        self.tags.retain(|t| t.to != usize::MAX && t.to >= t.from);
        self.frame = self.frame.min(self.frames.len() - 1);
        true
    }

    /// Ebene `l` löschen, in jedem Frame. Die letzte Ebene bleibt (`false`).
    pub fn delete_layer(&mut self, l: usize) -> bool {
        if self.layers.len() <= 1 || l >= self.layers.len() {
            return false;
        }
        self.layers.remove(l);
        for f in &mut self.frames {
            f.cels.remove(l);
        }
        self.layer = self.layer.min(self.layers.len() - 1);
        true
    }

    /// Ebene von Stelle `from` nach `to` verschieben (0 = unterste).
    pub fn move_layer(&mut self, from: usize, to: usize) {
        let n = self.layers.len();
        if from >= n || to >= n || from == to {
            return;
        }
        let l = self.layers.remove(from);
        self.layers.insert(to, l);
        for f in &mut self.frames {
            let c = f.cels.remove(from);
            f.cels.insert(to, c);
        }
        self.layer = to;
    }

    /// Aktive Ebene verdoppeln; die Kopie liegt darüber und wird aktiv.
    /// Verknüpfte Zellen der Vorlage sind in der Kopie wieder verknüpft.
    pub fn duplicate_layer(&mut self, name: impl Into<String>) {
        let src = self.layer;
        let at = src + 1;
        let mut layer = self.layers[src].clone();
        layer.name = name.into();
        self.layers.insert(at, layer);
        let mut memo: Vec<(ImageId, ImageId)> = Vec::new();
        for f in 0..self.frames.len() {
            let id = self.frames[f].cels[src];
            let copy = match memo.iter().find(|(a, _)| *a == id) {
                Some(&(_, c)) => c,
                None => {
                    self.images.push(self.images[id].clone());
                    let c = self.images.len() - 1;
                    memo.push((id, c));
                    c
                }
            };
            self.frames[f].cels.insert(at, copy);
        }
        self.layer = at;
    }

    /// Aktive Ebene in die darunter einrechnen, in jedem Frame. Mit voller
    /// Deckkraft überschreibt sie; halbdurchsichtig wird über einer Farbe
    /// gemischt (ergibt eine freie Farbe), über leerem Grund bleibt die
    /// Farbe deckend. Ohne Ebene darunter: `false`.
    pub fn merge_down(&mut self, pal: &crate::palette::Palette) -> bool {
        let top = self.layer;
        if top == 0 {
            return false;
        }
        let below = top - 1;
        let a = self.layers[top].opacity.clamp(0.0, 1.0) as f64;
        // Dasselbe Paar (oben, unten) ergibt dasselbe neue, wieder geteilte Bild.
        let mut done: Vec<((ImageId, ImageId), ImageId)> = Vec::new();
        for f in 0..self.frames.len() {
            let (src, dst) = (self.frames[f].cels[top], self.frames[f].cels[below]);
            let merged = match done.iter().find(|(k, _)| *k == (src, dst)) {
                Some(&(_, m)) => m,
                None => {
                    let mut out = self.images[dst].clone();
                    let pixels: Vec<(u32, u32, Px)> = self.images[src].pixels().collect();
                    for (x, y, v) in pixels {
                        let under = out.get(x, y);
                        let value = if a >= 1.0 {
                            v
                        } else {
                            let tc = crate::selection::rgb_of(v, pal, &self.free).unwrap_or([0, 0, 0]);
                            match crate::selection::rgb_of(under, pal, &self.free) {
                                Some(bc) if under != 0 => {
                                    let mix = |i: usize| (bc[i] as f64 * (1.0 - a) + tc[i] as f64 * a).round() as u8;
                                    self.free_color([mix(0), mix(1), mix(2)])
                                }
                                _ => self.free_color(tc),
                            }
                        };
                        out.set(x, y, value);
                    }
                    self.images.push(out);
                    let m = self.images.len() - 1;
                    done.push(((src, dst), m));
                    m
                }
            };
            self.frames[f].cels[below] = merged;
            self.frames[f].cels.remove(top);
        }
        self.layers.remove(top);
        self.layer = below;
        true
    }

    /// Frame von Stelle `from` nach `to` verschieben.
    pub fn move_frame(&mut self, from: usize, to: usize) {
        let n = self.frames.len();
        if from >= n || to >= n || from == to {
            return;
        }
        let f = self.frames.remove(from);
        self.frames.insert(to, f);
        self.frame = to;
    }

    /// Wie lange Frame `f` steht, in Millisekunden.
    pub fn frame_duration(&self, f: usize) -> u32 {
        match self.frames.get(f).map(|fr| fr.duration_ms) {
            Some(d) if d > 0 => d,
            _ => 1000 / self.fps.max(1),
        }
    }

    /// Ist die Zelle leer (kein einziger Pixel)?
    pub fn cel_is_empty(&self, f: usize, l: usize) -> bool {
        self.cel(f, l).is_empty()
    }

    /// Frames `from..=to` zeigen auf Ebene `l` alle auf das Bild von `from`.
    pub fn link(&mut self, from: usize, to: usize, l: usize) {
        let id = self.frames[from].cels[l];
        for f in from + 1..=to.min(self.frames.len() - 1) {
            self.frames[f].cels[l] = id;
        }
    }

    /// Die Zelle bekommt ein eigenes Bild mit gleichem Inhalt.
    pub fn unlink(&mut self, f: usize, l: usize) {
        if self.is_linked(f, l) {
            let copy = self.cel(f, l).clone();
            self.images.push(copy);
            self.frames[f].cels[l] = self.images.len() - 1;
        }
    }

    /// Bilder, auf die keine Zelle mehr zeigt, entfernen und die Nummern
    /// nachziehen (vor dem Speichern).
    pub fn collect_garbage(&mut self) {
        let mut used = vec![false; self.images.len()];
        for f in &self.frames {
            for &id in &f.cels {
                used[id] = true;
            }
        }
        let mut new_id = vec![usize::MAX; self.images.len()];
        let mut kept = Vec::new();
        for (old, img) in std::mem::take(&mut self.images).into_iter().enumerate() {
            if used[old] {
                new_id[old] = kept.len();
                kept.push(img);
            }
        }
        self.images = kept;
        for f in &mut self.frames {
            for id in &mut f.cels {
                *id = new_id[*id];
            }
        }
    }

    /// Pixelwert für eine freie Farbe — legt sie bei Bedarf an.
    pub fn free_color(&mut self, rgb: Rgb) -> Px {
        let i = match self.free.iter().position(|&c| c == rgb) {
            Some(i) => i,
            None => {
                self.free.push(rgb);
                self.free.len() - 1
            }
        };
        FREE_BASE + i as Px
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ebene_verdoppeln_haelt_verknuepfung() {
        let mut sp = Sprite::new("t", 4, 4).unwrap();
        sp.active().set(1, 1, 2);
        sp.add_frame(0, false);
        sp.link(0, 1, 0);
        sp.layer = 0;
        sp.duplicate_layer("Kopie");
        assert_eq!(sp.layers.len(), 2);
        assert_eq!(sp.layer, 1);
        assert_eq!(sp.frames[0].cels[1], sp.frames[1].cels[1], "Kopie wieder verknüpft");
        assert_ne!(sp.frames[0].cels[1], sp.frames[0].cels[0]);
        assert_eq!(sp.cel(1, 1).get(1, 1), 2);
    }

    #[test]
    fn zusammenlegen_deckend_und_halb() {
        let pal = crate::palette::Palette::new("p", vec![[0, 0, 0], [255, 255, 255]]);
        let mut sp = Sprite::new("t", 4, 4).unwrap();
        sp.active().set(0, 0, 1); // unten schwarz
        sp.add_layer(1, "oben");
        sp.active().set(0, 0, 2);
        sp.active().set(1, 0, 2);
        sp.layers[1].opacity = 0.5;
        assert!(sp.merge_down(&pal));
        assert_eq!(sp.layers.len(), 1);
        let mixed = sp.cel(0, 0).get(0, 0);
        assert_eq!(sp.free[(mixed - FREE_BASE) as usize], [128, 128, 128]);
        let alone = sp.cel(0, 0).get(1, 0);
        assert_eq!(sp.free[(alone - FREE_BASE) as usize], [255, 255, 255], "über leerem Grund deckend");
        assert!(!sp.merge_down(&pal), "nichts mehr darunter");
    }

    #[test]
    fn frame_verschieben() {
        let mut sp = Sprite::new("t", 2, 2).unwrap();
        sp.active().set(0, 0, 1);
        sp.add_frame(0, false);
        sp.move_frame(0, 1);
        assert_eq!(sp.frame, 1);
        assert_eq!(sp.cel(1, 0).get(0, 0), 1);
        assert_eq!(sp.cel(0, 0).get(0, 0), 0);
    }

    #[test]
    fn groessen_grenzen() {
        assert!(Sprite::new("a", 8192, 8192).is_ok());
        assert_eq!(
            Sprite::new("a", 8193, 10).unwrap_err(),
            SpriteError::BadSize { width: 8193, height: 10 }
        );
        assert!(Sprite::new("a", 0, 10).is_err());
    }

    #[test]
    fn neue_ebene_in_jedem_frame() {
        let mut sp = Sprite::new("a", 16, 16).unwrap();
        sp.add_frame(0, false);
        sp.add_layer(1, "Oben");
        assert_eq!(sp.layers.len(), 2);
        assert!(sp.frames.iter().all(|f| f.cels.len() == 2));
        assert_eq!(sp.layer, 1);
    }

    #[test]
    fn verknuepfte_zellen_malen_gemeinsam() {
        let mut sp = Sprite::new("a", 16, 16).unwrap();
        sp.add_frame(0, false);
        sp.add_frame(1, false);
        sp.link(0, 2, 0);
        sp.cel_mut(1, 0).set(3, 3, 5);
        assert_eq!(sp.cel(0, 0).get(3, 3), 5);
        assert_eq!(sp.cel(2, 0).get(3, 3), 5);
        assert!(sp.is_linked(2, 0));
        sp.unlink(2, 0);
        sp.cel_mut(2, 0).set(3, 3, 1);
        assert_eq!(sp.cel(0, 0).get(3, 3), 5, "gelöste Zelle ist eigenständig");
    }

    #[test]
    fn durchgehende_ebene_verknuepft_neue_frames() {
        let mut sp = Sprite::new("a", 16, 16).unwrap();
        sp.layers[0].continuous = true;
        sp.add_layer(1, "Figur");
        sp.add_frame(0, false);
        assert_eq!(sp.frames[1].cels[0], sp.frames[0].cels[0]);
        assert_ne!(sp.frames[1].cels[1], sp.frames[0].cels[1]);
    }

    #[test]
    fn duplizieren_kopiert_ohne_zu_verknuepfen() {
        let mut sp = Sprite::new("a", 16, 16).unwrap();
        sp.active().set(1, 1, 2);
        sp.add_frame(0, true);
        assert_eq!(sp.cel(1, 0).get(1, 1), 2);
        sp.cel_mut(1, 0).set(1, 1, 9);
        assert_eq!(sp.cel(0, 0).get(1, 1), 2);
    }

    #[test]
    fn tags_wachsen_und_ruecken_mit() {
        let mut sp = Sprite::new("a", 8, 8).unwrap();
        for i in 0..4 {
            sp.add_frame(i, false);
        }
        sp.tags.push(Tag { name: "t".into(), from: 2, to: 3, color: [0, 0, 0], direction: Direction::Forward });
        sp.add_frame(0, false); // davor
        assert_eq!((sp.tags[0].from, sp.tags[0].to), (3, 4));
        sp.add_frame(4, false); // am Ende des Tags
        assert_eq!((sp.tags[0].from, sp.tags[0].to), (3, 5));
    }

    #[test]
    fn frame_loeschen_zieht_tags_nach() {
        let mut sp = Sprite::new("a", 8, 8).unwrap();
        for i in 0..4 {
            sp.add_frame(i, false);
        }
        sp.tags.push(Tag { name: "a".into(), from: 1, to: 2, color: [0, 0, 0], direction: Direction::Forward });
        sp.tags.push(Tag { name: "b".into(), from: 4, to: 4, color: [0, 0, 0], direction: Direction::Forward });
        assert!(sp.delete_frame(0));
        assert_eq!((sp.tags[0].from, sp.tags[0].to), (0, 1));
        assert!(sp.delete_frame(3)); // der einzige Frame von b
        assert_eq!(sp.tags.len(), 1);
        assert_eq!(sp.frames.len(), 3);
    }

    #[test]
    fn letzter_frame_und_letzte_ebene_bleiben() {
        let mut sp = Sprite::new("a", 8, 8).unwrap();
        assert!(!sp.delete_frame(0));
        assert!(!sp.delete_layer(0));
    }

    #[test]
    fn ebene_loeschen_und_verschieben() {
        let mut sp = Sprite::new("a", 8, 8).unwrap();
        sp.add_layer(1, "B");
        sp.add_layer(2, "C");
        sp.move_layer(0, 2);
        let names: Vec<_> = sp.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["B", "C", "Ebene 1"]);
        assert_eq!(sp.layer, 2);
        assert!(sp.delete_layer(1));
        assert_eq!(sp.layers.len(), 2);
        assert!(sp.frames.iter().all(|f| f.cels.len() == 2));
    }

    #[test]
    fn dauer_eigene_oder_nach_fps() {
        let mut sp = Sprite::new("a", 8, 8).unwrap();
        sp.fps = 10;
        assert_eq!(sp.frame_duration(0), 100);
        sp.frames[0].duration_ms = 250;
        assert_eq!(sp.frame_duration(0), 250);
    }

    #[test]
    fn ungenutzte_bilder_werden_aufgeraeumt() {
        let mut sp = Sprite::new("a", 8, 8).unwrap();
        sp.add_frame(0, false);
        sp.link(0, 1, 0);
        assert_eq!(sp.images.len(), 2);
        sp.collect_garbage();
        assert_eq!(sp.images.len(), 1);
        assert_eq!(sp.frames[1].cels[0], 0);
    }

    #[test]
    fn freie_farben_bekommen_feste_werte() {
        let mut sp = Sprite::new("a", 8, 8).unwrap();
        let a = sp.free_color([1, 2, 3]);
        let b = sp.free_color([4, 5, 6]);
        assert_eq!(a, FREE_BASE);
        assert_eq!(b, FREE_BASE + 1);
        assert_eq!(sp.free_color([1, 2, 3]), a);
    }

    #[test]
    fn grosse_flaeche_mit_vielen_frames_bleibt_klein() {
        let mut sp = Sprite::new("gross", 8192, 8192).unwrap();
        for i in 0..20 {
            sp.add_frame(i, true);
        }
        sp.active().set(4000, 4000, 1);
        let tiles: usize = sp.images.iter().map(|i| i.allocated_tiles()).sum();
        assert_eq!(tiles, 1, "nur die eine bemalte Kachel belegt Speicher");
    }
}
