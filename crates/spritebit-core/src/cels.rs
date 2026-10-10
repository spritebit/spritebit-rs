//! Zellen der Timeline: Bereiche verschieben, kopieren, einfügen, leeren,
//! verknüpfen — wie `cels.js` der Web-Version.
//!
//! Ein Bereich ist ein Rechteck im Raster: Frames `f0..=f1`, Ebenen
//! `l0..=l1` (0 = unterste). Verschieben nimmt die Bild-Nummern mit (eine
//! Verknüpfung bleibt), Kopieren legt neue Bilder an — wobei innerhalb des
//! kopierten Bereichs Verknüpfte untereinander verknüpft bleiben.

use std::collections::HashMap;

use crate::image::Image;
use crate::sprite::{ImageId, Sprite};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CelRange {
    pub f0: usize,
    pub f1: usize,
    pub l0: usize,
    pub l1: usize,
}

impl CelRange {
    /// Bereich aus zwei beliebigen Ecken.
    pub fn new(fa: usize, la: usize, fb: usize, lb: usize) -> CelRange {
        CelRange { f0: fa.min(fb), f1: fa.max(fb), l0: la.min(lb), l1: la.max(lb) }
    }
    pub fn single(f: usize, l: usize) -> CelRange {
        CelRange { f0: f, f1: f, l0: l, l1: l }
    }
    pub fn contains(&self, f: usize, l: usize) -> bool {
        f >= self.f0 && f <= self.f1 && l >= self.l0 && l <= self.l1
    }
    pub fn size(&self) -> usize {
        (self.f1 - self.f0 + 1) * (self.l1 - self.l0 + 1)
    }
    /// Auf das zurechtgestutzt, was der Sprite hat — oder `None`.
    pub fn clamp(self, sp: &Sprite) -> Option<CelRange> {
        if self.f0 >= sp.frames.len() || self.l0 >= sp.layers.len() {
            return None;
        }
        Some(CelRange { f1: self.f1.min(sp.frames.len() - 1), l1: self.l1.min(sp.layers.len() - 1), ..self })
    }
}

/// Kopierte Zellen: `images[ebene - l0][frame - f0]`, dazu welche davon
/// sich ein Bild teilen (gleiche Nummer in `ids`).
#[derive(Clone, Debug)]
pub struct CelClip {
    pub w: usize,
    pub h: usize,
    images: Vec<Image>,
    ids: Vec<Vec<usize>>,
}

/// Bild kopieren — innerhalb eines Vorgangs ein geteiltes Bild nur einmal.
fn copier<'a>(sp: &'a Sprite) -> impl FnMut(ImageId, &mut Vec<Image>) -> usize + 'a {
    let mut memo: HashMap<ImageId, usize> = HashMap::new();
    move |id, out| {
        *memo.entry(id).or_insert_with(|| {
            out.push(sp.images[id].clone());
            out.len() - 1
        })
    }
}

/// Passt ein um (df, dl) versetzter Bereich noch ins Raster?
pub fn can_shift(sp: &Sprite, r: CelRange, df: isize, dl: isize) -> bool {
    let ok = |v: usize, d: isize, n: usize| (v as isize + d) >= 0 && ((v as isize + d) as usize) < n;
    ok(r.f0, df, sp.frames.len()) && ok(r.f1, df, sp.frames.len()) && ok(r.l0, dl, sp.layers.len()) && ok(r.l1, dl, sp.layers.len())
}

/// Bereich um df Frames und dl Ebenen versetzen. Verschieben lässt leere
/// Zellen zurück, Kopieren das Original stehen. Ziel außerhalb: `false`.
pub fn shift(sp: &mut Sprite, r: CelRange, df: isize, dl: isize, copy: bool) -> bool {
    if (df == 0 && dl == 0) || !can_shift(sp, r, df, dl) {
        return false;
    }
    // Erst einsammeln, dann schreiben — Quelle und Ziel dürfen überlappen.
    let mut moving = Vec::new();
    for l in r.l0..=r.l1 {
        for f in r.f0..=r.f1 {
            moving.push((f, l, sp.frames[f].cels[l]));
        }
    }
    if copy {
        let mut fresh = Vec::new();
        let mapped: Vec<(usize, usize, usize)> = {
            let mut cp = copier(sp);
            moving.iter().map(|&(f, l, id)| (f, l, cp(id, &mut fresh))).collect()
        };
        let base = sp.images.len();
        sp.images.extend(fresh);
        for (f, l, k) in mapped {
            sp.frames[(f as isize + df) as usize].cels[(l as isize + dl) as usize] = base + k;
        }
    } else {
        for &(f, l, _) in &moving {
            sp.images.push(Image::new(sp.width, sp.height));
            sp.frames[f].cels[l] = sp.images.len() - 1;
        }
        for (f, l, id) in moving {
            sp.frames[(f as isize + df) as usize].cels[(l as isize + dl) as usize] = id;
        }
    }
    true
}

/// Zellen leeren — jede bekommt ein eigenes leeres Bild (löst Verknüpfungen).
pub fn clear(sp: &mut Sprite, r: CelRange) {
    for l in r.l0..=r.l1 {
        for f in r.f0..=r.f1 {
            sp.images.push(Image::new(sp.width, sp.height));
            sp.frames[f].cels[l] = sp.images.len() - 1;
        }
    }
}

/// Zellen kopieren.
pub fn copy(sp: &Sprite, r: CelRange) -> CelClip {
    let mut images = Vec::new();
    let mut cp = copier(sp);
    let ids = (r.l0..=r.l1).map(|l| (r.f0..=r.f1).map(|f| cp(sp.frames[f].cels[l], &mut images)).collect()).collect();
    CelClip { w: r.f1 - r.f0 + 1, h: r.l1 - r.l0 + 1, images, ids }
}

impl CelClip {
    /// Alle Bilder der Kopie Pixel für Pixel umrechnen (Einfügen in einen
    /// Sprite mit anderer Palette, siehe [`crate::selection::Remap`]).
    pub fn map_pixels(&self, mut f: impl FnMut(crate::image::Px) -> crate::image::Px) -> CelClip {
        let images = self
            .images
            .iter()
            .map(|img| {
                let mut out = Image::new(img.width(), img.height());
                for (x, y, v) in img.pixels() {
                    let m = f(v);
                    if m != 0 {
                        out.set(x, y, m);
                    }
                }
                out
            })
            .collect();
        CelClip { w: self.w, h: self.h, images, ids: self.ids.clone() }
    }
}

/// Einfügen: erster Frame und OBERSTE Ebene der Kopie landen auf Frame `f`,
/// Ebene `l_top`. Was über den Rand ginge, fällt weg; Bilder anderer Größe
/// passen nicht (`None`). Gibt den belegten Bereich zurück.
pub fn paste(sp: &mut Sprite, clip: &CelClip, f: usize, l_top: usize) -> Option<CelRange> {
    if clip.images.iter().any(|i| i.width() != sp.width || i.height() != sp.height) {
        return None;
    }
    let base = sp.images.len();
    sp.images.extend(clip.images.iter().cloned());
    let mut used: Option<CelRange> = None;
    for dl in 0..clip.h {
        let Some(tl) = (l_top + dl + 1).checked_sub(clip.h) else { continue };
        for df in 0..clip.w {
            let tf = f + df;
            if tf >= sp.frames.len() || tl >= sp.layers.len() {
                continue;
            }
            sp.frames[tf].cels[tl] = base + clip.ids[dl][df];
            used = Some(match used {
                None => CelRange::single(tf, tl),
                Some(u) => CelRange::new(u.f0.min(tf), u.l0.min(tl), u.f1.max(tf), u.l1.max(tl)),
            });
        }
    }
    used
}

/// Je Ebene im Bereich zeigen alle Frames auf das Bild des ersten.
pub fn link(sp: &mut Sprite, r: CelRange) -> bool {
    let mut changed = false;
    for l in r.l0..=r.l1 {
        let id = sp.frames[r.f0].cels[l];
        for f in r.f0 + 1..=r.f1 {
            if sp.frames[f].cels[l] != id {
                sp.frames[f].cels[l] = id;
                changed = true;
            }
        }
    }
    changed
}

/// Jede Zelle im Bereich bekommt ein eigenes Bild mit gleichem Inhalt.
pub fn unlink(sp: &mut Sprite, r: CelRange) -> bool {
    let mut changed = false;
    for l in r.l0..=r.l1 {
        for f in r.f0..=r.f1 {
            if sp.is_linked(f, l) {
                sp.unlink(f, l);
                changed = true;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vier Frames, zwei Ebenen. Zelle (f, l) enthält im Pixel (0,0) den Wert 10*l + f + 1.
    fn grid4() -> Sprite {
        let mut sp = Sprite::new("c", 1, 1).unwrap();
        sp.add_layer(1, "B");
        for i in 0..3 {
            sp.add_frame(i, false);
        }
        for f in 0..4 {
            for l in 0..2 {
                sp.cel_mut(f, l).set(0, 0, (10 * l + f + 1) as u16);
            }
        }
        sp
    }
    fn v(sp: &Sprite, f: usize, l: usize) -> u16 {
        sp.cel(f, l).get(0, 0)
    }

    #[test]
    fn bereich_aus_zwei_ecken() {
        let r = CelRange::new(3, 1, 1, 0);
        assert_eq!(r, CelRange { f0: 1, f1: 3, l0: 0, l1: 1 });
        assert_eq!(r.size(), 6);
        assert!(r.contains(2, 1) && !r.contains(0, 0));
    }

    #[test]
    fn verschieben_laesst_leere_zelle_zurueck() {
        let mut sp = grid4();
        assert!(shift(&mut sp, CelRange::single(0, 0), 2, 0, false));
        assert_eq!(v(&sp, 2, 0), 1);
        assert_eq!(v(&sp, 0, 0), 0);
        assert_eq!(v(&sp, 1, 0), 2);
    }

    #[test]
    fn verschieben_ueber_sich_selbst() {
        let mut sp = grid4();
        shift(&mut sp, CelRange::new(0, 1, 2, 1), 1, 0, false);
        assert_eq!((0..4).map(|f| v(&sp, f, 1)).collect::<Vec<_>>(), vec![0, 11, 12, 13]);
    }

    #[test]
    fn kopieren_teilt_kein_bild() {
        let mut sp = grid4();
        shift(&mut sp, CelRange::single(1, 0), 0, 1, true);
        assert_eq!(v(&sp, 1, 1), 2);
        assert_eq!(v(&sp, 1, 0), 2);
        assert_ne!(sp.frames[1].cels[1], sp.frames[1].cels[0]);
    }

    #[test]
    fn ziel_ausserhalb_aendert_nichts() {
        let mut sp = grid4();
        assert!(!shift(&mut sp, CelRange::single(3, 0), 1, 0, false));
        assert!(!shift(&mut sp, CelRange::single(0, 1), 0, 1, false));
        assert_eq!(v(&sp, 3, 0), 4);
    }

    #[test]
    fn leeren_loest_verknuepfung() {
        let mut sp = grid4();
        sp.link(0, 1, 0);
        clear(&mut sp, CelRange::single(1, 0));
        assert_eq!(v(&sp, 1, 0), 0);
        assert_eq!(v(&sp, 0, 0), 1);
    }

    #[test]
    fn kopieren_und_einfuegen_oberste_ebene_oben() {
        let mut sp = grid4();
        let clip = copy(&sp, CelRange::new(0, 0, 1, 1));
        let used = paste(&mut sp, &clip, 2, 1).unwrap();
        assert_eq!(used, CelRange { f0: 2, f1: 3, l0: 0, l1: 1 });
        assert_eq!([v(&sp, 2, 0), v(&sp, 3, 0), v(&sp, 2, 1), v(&sp, 3, 1)], [1, 2, 11, 12]);
    }

    #[test]
    fn einfuegen_am_rand_schneidet_ab() {
        let mut sp = grid4();
        let clip = copy(&sp, CelRange::new(0, 0, 1, 1));
        let used = paste(&mut sp, &clip, 3, 0).unwrap();
        assert_eq!(used, CelRange::single(3, 0));
        assert_eq!(v(&sp, 3, 0), 11, "oberste Ebene der Kopie");
    }

    #[test]
    fn verknuepfen_und_loesen() {
        let mut sp = grid4();
        assert!(link(&mut sp, CelRange::new(1, 0, 3, 0)));
        assert_eq!(sp.frames[3].cels[0], sp.frames[1].cels[0]);
        assert_ne!(sp.frames[0].cels[0], sp.frames[1].cels[0]);
        assert!(!link(&mut sp, CelRange::new(1, 0, 3, 0)));
        assert!(unlink(&mut sp, CelRange::new(1, 0, 3, 0)));
        assert!(!sp.is_linked(2, 0));
        assert_eq!(v(&sp, 3, 0), 2);
    }

    #[test]
    fn kopie_haelt_verknuepfung_innerhalb() {
        let mut sp = grid4();
        sp.link(0, 1, 0);
        let clip = copy(&sp, CelRange::new(0, 0, 1, 0));
        paste(&mut sp, &clip, 2, 0);
        assert_eq!(sp.frames[2].cels[0], sp.frames[3].cels[0]);
        assert_ne!(sp.frames[2].cels[0], sp.frames[0].cels[0]);
    }
}
