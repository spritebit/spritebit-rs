//! Onion Skin: welche Frames durchscheinen und wie stark — wie `onion.js`
//! der Web-Version. Nur Rechnung, keine Oberfläche.

use crate::sprite::Sprite;

/// Höchstens so viele Frames davor bzw. danach.
pub const ONION_MAX: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnionOpts {
    /// `false`: rot/blau getönt, `true`: in echten Farben.
    pub real_colors: bool,
    /// Deckkraft des nächsten Nachbarn (0.1–0.9).
    pub opacity: f32,
    /// Um wie viel jeder weitere Frame blasser wird (0–0.9).
    pub step: f32,
    pub before: u32,
    pub after: u32,
    /// Im Tag bleiben: am Ende des Tags geht es vorn weiter.
    pub loop_tag: bool,
    /// Nur die aktive Ebene zeigen.
    pub layer_only: bool,
    /// Vor statt hinter dem Bild.
    pub front: bool,
}

impl Default for OnionOpts {
    fn default() -> Self {
        OnionOpts { real_colors: false, opacity: 0.3, step: 0.3, before: 1, after: 1, loop_tag: false, layer_only: false, front: false }
    }
}

impl OnionOpts {
    /// Werte in ihre Grenzen bringen.
    pub fn normalized(self) -> Self {
        OnionOpts {
            opacity: self.opacity.clamp(0.1, 0.9),
            step: self.step.clamp(0.0, 0.9),
            before: self.before.min(ONION_MAX),
            after: self.after.min(ONION_MAX),
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnionFrame {
    pub frame: usize,
    /// `true`: davor (rot), `false`: danach (blau).
    pub before: bool,
    pub alpha: f32,
}

/// Frames, die um Frame `f` durchscheinen. Der nächste Nachbar hat
/// `opacity`, jeder weitere `step` weniger (relativ). Mit `loop_tag` läuft
/// die Zählung im Tag um `f` im Kreis, sonst endet sie am Rand.
pub fn onion_frames(sp: &Sprite, f: usize, o: &OnionOpts) -> Vec<OnionFrame> {
    let n = sp.frames.len() as i64;
    let tag = if o.loop_tag { sp.tags.iter().find(|t| f >= t.from && f <= t.to) } else { None };
    let mut out = Vec::new();
    let mut seen = vec![f];
    let mut add = |k: u32, before: bool| {
        let mut i = if before { f as i64 - k as i64 } else { f as i64 + k as i64 };
        if let Some(t) = tag {
            let len = (t.to - t.from + 1) as i64;
            i = t.from as i64 + ((i - t.from as i64) % len + len) % len;
        } else if i < 0 || i >= n {
            return;
        }
        let i = i as usize;
        if seen.contains(&i) {
            return;
        }
        seen.push(i);
        out.push(OnionFrame { frame: i, before, alpha: o.opacity * (1.0 - o.step).powi(k as i32 - 1) });
    };
    for k in 1..=o.before.max(o.after) {
        if k <= o.before {
            add(k, true);
        }
        if k <= o.after {
            add(k, false);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprite::{Direction, Tag};

    fn sp(n: usize) -> Sprite {
        let mut s = Sprite::new("t", 2, 2).unwrap();
        for i in 1..n {
            s.add_frame(i - 1, false);
        }
        s
    }

    #[test]
    fn nachbarn_und_abstufung() {
        let o = OnionOpts { before: 2, after: 1, ..Default::default() };
        let v = onion_frames(&sp(5), 2, &o);
        assert_eq!(v.iter().map(|x| (x.frame, x.before)).collect::<Vec<_>>(), vec![(1, true), (3, false), (0, true)]);
        assert!((v[2].alpha - 0.3 * 0.7).abs() < 1e-6);
    }

    #[test]
    fn am_rand_ohne_tag_endet_es() {
        let v = onion_frames(&sp(3), 0, &OnionOpts::default());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].frame, 1);
    }

    #[test]
    fn im_tag_im_kreis() {
        let mut s = sp(6);
        s.tags.push(Tag { name: "Lauf".into(), from: 1, to: 3, color: [0, 0, 0], direction: Direction::Forward });
        let o = OnionOpts { loop_tag: true, ..Default::default() };
        let v = onion_frames(&s, 3, &o);
        assert_eq!(v.iter().map(|x| x.frame).collect::<Vec<_>>(), vec![2, 1], "nach 3 kommt wieder 1");
    }
}
