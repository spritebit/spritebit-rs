//! Tilemap-Ebenen mit eigenem Kachelsatz (wie in Aseprite) — wie
//! `js/tiles.js` in der Web-Version.
//!
//! Eine Tilemap-Ebene ist ein Raster aus Kacheln fester Größe. Jede Kachel
//! steht einmal im Kachelsatz der Ebene; malt man eine Kachel an, ändern sich
//! alle Stellen mit, an denen sie liegt.
//!
//! Die Ebene speichert weiter ganz normale Pixel — welche Kachel wo liegt,
//! ergibt sich aus dem Inhalt der Zellen ([`map_of`]). So funktionieren
//! Frames, verknüpfte Zellen, Undo, Masken und Export ohne Sonderfall; der
//! Kachelsatz sorgt nur dafür, dass Kacheln gleich bleiben und sich gemeinsam
//! ändern. Kachel 0 ist „leer“ und steht nicht in der Liste.
//!
//! Nach jeder Änderung gleicht [`sync_sprite`] Pixel und Kachelsatz ab:
//! - Pixel malen: geänderte Kacheln werden im Satz geändert und überall
//!   mitgezogen. Auto legt beim Malen in leere Zellen neue Kacheln an,
//!   Manuell nicht.
//! - Alles andere (Kacheln setzen, Frames, Größe …): Inhalt gilt; was neu
//!   ist, kommt in den Satz.
//!
//! Der Rand, der nicht in eine ganze Kachel passt, gehört nicht zur Karte.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::image::{Image, Px};
use crate::sprite::Sprite;

pub const TILE_SIZES: [u32; 6] = [8, 16, 24, 32, 48, 64];
pub const DEFAULT_TILE: u32 = 16;

/// Kachelsatz einer Tilemap-Ebene. Kachel k (ab 1) ist `tiles[k - 1]`,
/// zeilenweise `tw × th` Pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct Tileset {
    pub tw: u32,
    pub th: u32,
    pub tiles: Vec<Vec<Px>>,
}

/// Neue Kacheln beim Malen in leere Zellen?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewTiles {
    Auto,
    Manual,
}

/// Was ein Abgleich getan hat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncSum {
    /// Kacheln, die geändert und überall mitgezogen wurden.
    pub edited: usize,
    /// Neu aufgenommene Kacheln.
    pub added: usize,
    /// Zellen, die im Modus Manuell leer bleiben mussten.
    pub blocked: usize,
}

impl Tileset {
    pub fn new(tw: u32, th: u32) -> Self {
        Tileset { tw, th, tiles: Vec::new() }
    }

    /// Wie viele ganze Kacheln passen in `w × h`? (Spalten, Zeilen)
    pub fn map_size(&self, w: u32, h: u32) -> (u32, u32) {
        (w / self.tw, h / self.th)
    }

    fn read(&self, img: &Image, cx: u32, cy: u32) -> Vec<Px> {
        let mut v = Vec::with_capacity((self.tw * self.th) as usize);
        for y in 0..self.th {
            for x in 0..self.tw {
                v.push(img.get(cx * self.tw + x, cy * self.th + y));
            }
        }
        v
    }

    fn write(&self, img: &mut Image, cx: u32, cy: u32, t: &[Px]) {
        for y in 0..self.th {
            for x in 0..self.tw {
                let v = t[(y * self.tw + x) as usize];
                let (px, py) = (cx * self.tw + x, cy * self.th + y);
                if img.get(px, py) != v {
                    img.set(px, py, v);
                }
            }
        }
    }

    /// Kachel k in Zelle (cx, cy) setzen — 0 leert die Zelle.
    pub fn place(&self, img: &mut Image, cx: u32, cy: u32, k: usize) {
        let blank = vec![0; (self.tw * self.th) as usize];
        let t = if k > 0 { self.tiles.get(k - 1).unwrap_or(&blank) } else { &blank };
        self.write(img, cx, cy, &t.clone());
    }

    fn index(&self) -> HashMap<Vec<Px>, usize> {
        let mut m = HashMap::new();
        for (i, t) in self.tiles.iter().enumerate() {
            m.entry(t.clone()).or_insert(i + 1);
        }
        m
    }

    /// Kachelnummer in Zelle (cx, cy): 0 = leer, -1 = Inhalt, der (noch) keine Kachel ist.
    pub fn tile_at(&self, img: &Image, cx: u32, cy: u32) -> i64 {
        let k = self.read(img, cx, cy);
        if blank(&k) {
            0
        } else {
            self.index().get(&k).map_or(-1, |&i| i as i64)
        }
    }

    /// Welche Kachel liegt wo? Zeilen × Spalten, Werte wie [`Self::tile_at`].
    pub fn map_of(&self, img: &Image) -> Vec<Vec<i64>> {
        let (cols, rows) = self.map_size(img.width(), img.height());
        let idx = self.index();
        (0..rows)
            .map(|cy| {
                (0..cols)
                    .map(|cx| {
                        let k = self.read(img, cx, cy);
                        if blank(&k) {
                            0
                        } else {
                            idx.get(&k).map_or(-1, |&i| i as i64)
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// Doppelte und leere Kacheln aus dem Satz nehmen (Reihenfolge bleibt).
    fn dedupe(&mut self) {
        let mut seen = HashSet::new();
        self.tiles.retain(|t| !blank(t) && seen.insert(t.clone()));
    }

    /// Inhalt gilt: was in den Bildern steht und noch keine Kachel ist, wird
    /// eine (Reihenfolge wie im Bild). Gibt zurück, wie viele neu sind.
    pub fn absorb<'a>(&mut self, imgs: impl IntoIterator<Item = &'a Image>) -> usize {
        self.dedupe();
        let mut idx: HashSet<Vec<Px>> = self.tiles.iter().cloned().collect();
        let mut added = 0;
        for img in imgs {
            let (cols, rows) = self.map_size(img.width(), img.height());
            for cy in 0..rows {
                for cx in 0..cols {
                    // Leere Kacheln (je Bild-Kachel `None`) schnell überspringen.
                    let k = self.read(img, cx, cy);
                    if blank(&k) || idx.contains(&k) {
                        continue;
                    }
                    idx.insert(k.clone());
                    self.tiles.push(k);
                    added += 1;
                }
            }
        }
        added
    }

    /// Kachelsatz neu aus den Bildern (Ebene in Tilemap umwandeln).
    pub fn build<'a>(tw: u32, th: u32, imgs: impl IntoIterator<Item = &'a Image>) -> Self {
        let mut ts = Tileset::new(tw, th);
        ts.absorb(imgs);
        ts
    }

    /// Kacheln, die in keinem Bild mehr vorkommen, entfernen; gibt die Anzahl zurück.
    pub fn prune_unused<'a>(&mut self, imgs: impl IntoIterator<Item = &'a Image>) -> usize {
        let mut used = HashSet::new();
        for img in imgs {
            for row in self.map_of(img) {
                used.extend(row.into_iter().filter(|&k| k > 0));
            }
        }
        let n = self.tiles.len();
        let mut i = 0;
        self.tiles.retain(|_| {
            i += 1;
            used.contains(&(i as i64))
        });
        n - self.tiles.len()
    }

    /// Kacheln flächig füllen: alle zusammenhängenden Zellen mit derselben
    /// Kachel wie (cx, cy) bekommen k. Gibt zurück, wie viele sich änderten.
    pub fn fill(&self, img: &mut Image, cx: u32, cy: u32, k: usize) -> usize {
        let mut map = self.map_of(img);
        let rows = map.len();
        let cols = map.first().map_or(0, Vec::len);
        if cy as usize >= rows || cx as usize >= cols {
            return 0;
        }
        let from = map[cy as usize][cx as usize];
        if from == k as i64 {
            return 0;
        }
        let mut stack = vec![(cx as i64, cy as i64)];
        let mut n = 0;
        while let Some((x, y)) = stack.pop() {
            if x < 0 || y < 0 || x as usize >= cols || y as usize >= rows || map[y as usize][x as usize] != from {
                continue;
            }
            map[y as usize][x as usize] = k as i64;
            self.place(img, x as u32, y as u32, k);
            n += 1;
            stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
        n
    }

    /// Nach einem Strich im Modus „Pixel“: geänderte Kacheln im Satz ändern
    /// und überall mitziehen. `before` ist das bemalte Bild vor dem Strich,
    /// `after` dasselbe Bild jetzt; `others` die übrigen Bilder der Ebene.
    pub fn sync_paint(&mut self, before: &Image, after: &mut Image, others: &mut [&mut Image], mode: NewTiles) -> SyncSum {
        let (cols, rows) = self.map_size(after.width(), after.height());
        let idx = self.index();
        // Je alte Kachel: die neue Fassung aus den Änderungen aller Stellen,
        // an denen sie angemalt wurde. Widersprechen sich zwei Stellen
        // (dasselbe Pixel, verschiedene Farben), gilt dort der Inhalt.
        struct Edit {
            tile: Vec<Px>,
            set: Vec<bool>,
            bad: bool,
        }
        let mut edits: HashMap<usize, Edit> = HashMap::new();
        let mut order = Vec::new();
        let mut blocked = 0;
        for cy in 0..rows {
            for cx in 0..cols {
                let kb = self.read(before, cx, cy);
                let ka = self.read(after, cx, cy);
                if kb == ka {
                    continue;
                }
                if blank(&kb) {
                    if mode == NewTiles::Manual {
                        self.write(after, cx, cy, &kb);
                        blocked += 1;
                    }
                    continue; // Auto: wird unten als neue Kachel aufgenommen
                }
                let Some(&old) = idx.get(&kb) else { continue }; // war keine Kachel — Inhalt gilt
                let e = edits.entry(old).or_insert_with(|| {
                    order.push(old);
                    Edit { tile: self.tiles[old - 1].clone(), set: vec![false; kb.len()], bad: false }
                });
                for i in 0..kb.len() {
                    if ka[i] == kb[i] {
                        continue;
                    }
                    if e.set[i] && e.tile[i] != ka[i] {
                        e.bad = true;
                    }
                    e.tile[i] = ka[i];
                    e.set[i] = true;
                }
            }
        }

        // Mitziehen: jede Stelle, die vorher Kachel k zeigte, zeigt die neue Fassung.
        let changed: Vec<(usize, Vec<Px>)> =
            order.iter().filter(|k| !edits[k].bad).map(|&k| (k, edits[&k].tile.clone())).collect();
        if !changed.is_empty() {
            let old: HashMap<Vec<Px>, &Vec<Px>> = changed.iter().map(|(k, t)| (self.tiles[k - 1].clone(), t)).collect();
            let pull = |ts: &Tileset, src: &Image, dst: &mut Image| {
                for cy in 0..rows {
                    for cx in 0..cols {
                        if let Some(t) = old.get(&ts.read(src, cx, cy)) {
                            ts.write(dst, cx, cy, t);
                        }
                    }
                }
            };
            pull(self, before, after);
            for g in others.iter_mut() {
                let src = (**g).clone();
                pull(self, &src, g);
            }
            for (k, t) in &changed {
                self.tiles[k - 1] = t.clone();
            }
        }
        let added = self.absorb(std::iter::once(&*after).chain(others.iter().map(|g| &**g)));
        SyncSum { edited: changed.len(), added, blocked }
    }

    /// Kachelbild: alle Kacheln in einem Raster (Kachel k an Stelle k-1), als
    /// (Breite, Höhe, Pixel zeilenweise).
    pub fn atlas(&self) -> (u32, u32, Vec<Px>) {
        let n = self.tiles.len();
        let cols = atlas_cols(n) as u32;
        let rows = n.div_ceil(cols as usize).max(1) as u32;
        let (w, h) = (cols * self.tw, rows * self.th);
        let mut px = vec![0; (w * h) as usize];
        for (i, t) in self.tiles.iter().enumerate() {
            let (ox, oy) = ((i as u32 % cols) * self.tw, (i as u32 / cols) * self.th);
            for y in 0..self.th {
                for x in 0..self.tw {
                    px[((oy + y) * w + ox + x) as usize] = t[(y * self.tw + x) as usize];
                }
            }
        }
        (w, h, px)
    }

    /// Für den Speicherstand — Pixel als Zahl (Palette) bzw. Hex (freie Farbe),
    /// wie die Web-Version.
    pub fn to_json(&self, px: impl Fn(Px) -> Value) -> Value {
        let tiles: Vec<Value> = self
            .tiles
            .iter()
            .map(|t| Value::Array(t.chunks(self.tw as usize).map(|r| Value::Array(r.iter().map(|&v| px(v)).collect())).collect()))
            .collect();
        json!({ "tw": self.tw, "th": self.th, "tiles": tiles })
    }

    /// Aus dem Speicherstand; `px` übersetzt einen Wert (Zahl oder Hex).
    /// Unbrauchbares → `None` (dann ist es eine normale Ebene).
    pub fn from_json(v: &Value, mut px: impl FnMut(&Value) -> Px) -> Option<Self> {
        let tw = v.get("tw")?.as_u64()? as u32;
        let th = v.get("th")?.as_u64()? as u32;
        if !(1..=256).contains(&tw) || !(1..=256).contains(&th) {
            return None;
        }
        let mut ts = Tileset::new(tw, th);
        for t in v.get("tiles").and_then(Value::as_array).into_iter().flatten() {
            let Some(rows) = t.as_array().filter(|r| r.len() == th as usize) else { continue };
            let mut data = Vec::with_capacity((tw * th) as usize);
            let mut ok = true;
            for r in rows {
                match r.as_array().filter(|r| r.len() == tw as usize) {
                    Some(r) => data.extend(r.iter().map(&mut px)),
                    None => ok = false,
                }
            }
            if ok {
                ts.tiles.push(data);
            }
        }
        Some(ts)
    }
}

fn blank(t: &[Px]) -> bool {
    t.iter().all(|&v| v == 0)
}

/// Spalten im Kachelbild (Atlas): möglichst quadratisch.
pub fn atlas_cols(n: usize) -> usize {
    ((n as f64).sqrt().ceil() as usize).max(1)
}

/// Alle Tilemap-Ebenen eines Sprites mit ihren Kachelsätzen abgleichen.
/// Als Bemalen der Kacheln zählt es nur, wenn sich genau die aktive Zelle
/// geändert hat und sonst nichts — Frames, Ebenen, Größe und Kachelsatz wie
/// vorher. Alles andere übernimmt den Inhalt. `paint` = Modus „Pixel“.
pub fn sync_sprite(sp: &mut Sprite, before: &Sprite, paint: bool, mode: NewTiles) -> SyncSum {
    let mut sum = SyncSum::default();
    for li in 0..sp.layers.len() {
        let Some(mut ts) = sp.layers[li].tileset.take() else { continue };
        let mut ids: Vec<usize> = Vec::new();
        for f in &sp.frames {
            if !ids.contains(&f.cels[li]) {
                ids.push(f.cels[li]);
            }
        }
        let act = sp.frames[sp.frame].cels[li];
        let editable = paint
            && li == sp.layer
            && before.layer == sp.layer
            && before.frame == sp.frame
            && before.frames.len() == sp.frames.len()
            && before.layers.len() == sp.layers.len()
            && before.width == sp.width
            && before.height == sp.height
            && before.layers[li].tileset.as_ref() == Some(&ts)
            && sp.frames.iter().enumerate().all(|(k, f)| {
                f.cels[li] == act || sp.images[f.cels[li]].same_pixels(&before.images[before.frames[k].cels[li]])
            });
        if editable {
            let b = before.images[before.frames[sp.frame].cels[li]].clone();
            let mut after = std::mem::replace(&mut sp.images[act], Image::new(1, 1));
            let mut others: Vec<Image> =
                ids.iter().filter(|&&i| i != act).map(|&i| std::mem::replace(&mut sp.images[i], Image::new(1, 1))).collect();
            let mut refs: Vec<&mut Image> = others.iter_mut().collect();
            let r = ts.sync_paint(&b, &mut after, &mut refs, mode);
            sp.images[act] = after;
            for (&i, img) in ids.iter().filter(|&&i| i != act).zip(others) {
                sp.images[i] = img;
            }
            sum.edited += r.edited;
            sum.added += r.added;
            sum.blocked += r.blocked;
        } else {
            sum.added += ts.absorb(ids.iter().map(|&i| &sp.images[i]));
        }
        sp.layers[li].tileset = Some(ts);
    }
    sum
}

// ── Export für Godot 4 (TileMapLayer) ───────────────────────────────

/// Eine Ebene für den Godot-Export.
pub struct GodotLayer<'a> {
    pub name: &'a str,
    pub ts: &'a Tileset,
    pub map: Vec<Vec<i64>>,
    /// Pfad im Godot-Projekt, z. B. `res://level/level_boden.png`.
    pub png: String,
    pub visible: bool,
    pub opacity: f32,
}

/// Godots `tile_map_data`: 2 Byte Format (0), dann je belegter Zelle 12 Byte
/// (Little Endian): x, y, Quelle, Atlas-x, Atlas-y, Alternative.
pub fn tile_map_data(map: &[Vec<i64>], n: usize) -> Vec<u8> {
    let cols = atlas_cols(n) as i64;
    let mut out = vec![0, 0];
    for (y, row) in map.iter().enumerate() {
        for (x, &k) in row.iter().enumerate() {
            if k <= 0 {
                continue;
            }
            for v in [x as i64, y as i64, 0, (k - 1) % cols, (k - 1) / cols, 0] {
                out.extend_from_slice(&(v as u16).to_le_bytes());
            }
        }
    }
    out
}

fn gd_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Godot-Szene (.tscn): ein Node2D mit einer TileMapLayer je Tilemap-Ebene.
pub fn godot_scene(name: &str, layers: &[GodotLayer]) -> String {
    let mut parts = vec![format!("[gd_scene load_steps={} format=3]", layers.len() * 3 + 1)];
    for (i, l) in layers.iter().enumerate() {
        parts.push(format!("[ext_resource type=\"Texture2D\" path={} id=\"{}_tex\"]", gd_str(&l.png), i + 1));
    }
    for (i, l) in layers.iter().enumerate() {
        let cols = atlas_cols(l.ts.tiles.len());
        let mut s = vec![
            format!("[sub_resource type=\"TileSetAtlasSource\" id=\"TileSetAtlasSource_{}\"]", i + 1),
            format!("texture = ExtResource(\"{}_tex\")", i + 1),
            format!("texture_region_size = Vector2i({}, {})", l.ts.tw, l.ts.th),
        ];
        s.extend((0..l.ts.tiles.len()).map(|k| format!("{}:{}/0 = 0", k % cols, k / cols)));
        parts.push(s.join("\n"));
        parts.push(
            [
                format!("[sub_resource type=\"TileSet\" id=\"TileSet_{}\"]", i + 1),
                format!("tile_size = Vector2i({}, {})", l.ts.tw, l.ts.th),
                format!("sources/0 = SubResource(\"TileSetAtlasSource_{}\")", i + 1),
            ]
            .join("\n"),
        );
    }
    parts.push(format!("[node name={} type=\"Node2D\"]", gd_str(name)));
    for (i, l) in layers.iter().enumerate() {
        let mut s = vec![format!("[node name={} type=\"TileMapLayer\" parent=\".\"]", gd_str(l.name))];
        if !l.visible {
            s.push("visible = false".into());
        }
        if l.opacity < 1.0 {
            s.push(format!("modulate = Color(1, 1, 1, {})", (l.opacity * 1000.0).round() / 1000.0));
        }
        s.push("texture_filter = 1".into());
        let bytes: Vec<String> = tile_map_data(&l.map, l.ts.tiles.len()).iter().map(u8::to_string).collect();
        s.push(format!("tile_map_data = PackedByteArray({})", bytes.join(", ")));
        s.push(format!("tile_set = SubResource(\"TileSet_{}\")", i + 1));
        parts.push(s.join("\n"));
    }
    parts.join("\n\n") + "\n"
}

/// Allgemeines JSON (für eigene Engines): Kachelgröße, je Ebene die Karte.
pub fn tilemap_json(name: &str, layers: &[GodotLayer]) -> String {
    let ls: Vec<Value> = layers
        .iter()
        .map(|l| {
            let atlas = l.png.rsplit('/').next().unwrap_or(&l.png);
            json!({
                "name": l.name, "tileWidth": l.ts.tw, "tileHeight": l.ts.th,
                "tiles": l.ts.tiles.len(), "atlas": atlas, "atlasColumns": atlas_cols(l.ts.tiles.len()),
                "columns": l.map.first().map_or(0, Vec::len), "rows": l.map.len(),
                "data": l.map.iter().flatten().map(|&k| k.max(0)).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({ "name": name, "layers": ls })).expect("JSON aus eigenen Daten")
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2×2-Kacheln: A = Punkt oben links, B = volle Kachel.
    const A: [Px; 4] = [1, 0, 0, 0];
    const B: [Px; 4] = [2, 2, 2, 2];

    /// 6×4 Pixel = 3×2 Kacheln:  A B A
    ///                           . A .
    fn level() -> (Tileset, Image) {
        let mut ts = Tileset::new(2, 2);
        ts.tiles = vec![A.to_vec(), B.to_vec()];
        let mut g = Image::new(6, 4);
        ts.place(&mut g, 0, 0, 1);
        ts.place(&mut g, 1, 0, 2);
        ts.place(&mut g, 2, 0, 1);
        ts.place(&mut g, 1, 1, 1);
        (ts, g)
    }

    #[test]
    fn karte_aus_dem_inhalt() {
        let (ts, mut g) = level();
        assert_eq!(ts.map_of(&g), vec![vec![1, 2, 1], vec![0, 1, 0]]);
        assert_eq!(ts.tile_at(&g, 1, 0), 2);
        g.set(0, 2, 5);
        assert_eq!(ts.tile_at(&g, 0, 1), -1);
    }

    #[test]
    fn umwandeln() {
        let (_, g) = level();
        let ts = Tileset::build(2, 2, [&g]);
        assert_eq!(ts.tiles, vec![A.to_vec(), B.to_vec()]);
    }

    #[test]
    fn pixel_malen_aendert_die_kachel_ueberall() {
        let (mut ts, mut g) = level();
        let mut other = g.clone();
        let before = g.clone();
        g.set(1, 1, 3);
        let r = ts.sync_paint(&before, &mut g, &mut [&mut other], NewTiles::Auto);
        assert_eq!(r, SyncSum { edited: 1, added: 0, blocked: 0 });
        assert_eq!(ts.tiles[0], vec![1, 0, 0, 3]);
        assert_eq!(g.get(5, 1), 3, "die andere A-Stelle im selben Bild");
        assert_eq!(g.get(3, 3), 3, "A in der zweiten Reihe");
        assert_eq!(other.get(1, 1), 3, "anderer Frame");
    }

    #[test]
    fn auto_und_manuell() {
        let (mut ts, mut g) = level();
        let before = g.clone();
        g.set(0, 2, 4);
        assert_eq!(ts.sync_paint(&before, &mut g, &mut [], NewTiles::Auto).added, 1);
        assert_eq!(ts.map_of(&g)[1], vec![3, 1, 0]);

        let (mut ts, mut g) = level();
        let before = g.clone();
        g.set(0, 2, 4);
        g.set(1, 0, 3);
        let r = ts.sync_paint(&before, &mut g, &mut [], NewTiles::Manual);
        assert_eq!((r.blocked, r.added), (1, 0));
        assert_eq!(g.get(0, 2), 0, "zurückgenommen");
        assert_eq!(ts.tiles[0], vec![1, 3, 0, 0]);
    }

    #[test]
    fn widerspruch_dort_gilt_der_inhalt() {
        let (mut ts, mut g) = level();
        let before = g.clone();
        g.set(1, 0, 3);
        g.set(5, 0, 4);
        let r = ts.sync_paint(&before, &mut g, &mut [], NewTiles::Auto);
        assert_eq!((r.edited, r.added), (0, 2));
        assert_eq!(ts.tiles[0], A.to_vec());
        assert_eq!(g.get(3, 3), 0);
    }

    #[test]
    fn kachel_leer_gemalt_verschwindet() {
        let (mut ts, mut g) = level();
        let before = g.clone();
        g.set(0, 0, 0);
        ts.sync_paint(&before, &mut g, &mut [], NewTiles::Auto);
        assert_eq!(ts.tiles.len(), 1);
        assert_eq!(ts.map_of(&g), vec![vec![0, 1, 0], vec![0, 0, 0]]);
    }

    #[test]
    fn unbenutzte_entfernen_und_fuellen() {
        let (mut ts, mut g) = level();
        ts.tiles.push(vec![7; 4]);
        assert_eq!(ts.prune_unused([&g]), 1);
        assert_eq!(ts.fill(&mut g, 0, 1, 2), 1);
        assert_eq!(ts.map_of(&g), vec![vec![1, 2, 1], vec![2, 1, 0]]);
    }

    #[test]
    fn rand_gehoert_nicht_zur_karte() {
        let mut ts = Tileset::new(2, 2);
        let mut g = Image::new(5, 3);
        g.set(4, 2, 9);
        assert_eq!(ts.map_of(&g), vec![vec![0, 0]]);
        assert_eq!(ts.absorb([&g]), 0);
    }

    #[test]
    fn sync_sprite_strich_und_setzen() {
        let (ts, g) = level();
        let mut sp = Sprite::new("t", 6, 4).unwrap();
        sp.images = vec![g.clone(), g];
        sp.frames = vec![
            crate::sprite::Frame { cels: vec![0], duration_ms: 0 },
            crate::sprite::Frame { cels: vec![1], duration_ms: 0 },
        ];
        sp.layers[0].tileset = Some(ts);
        // Strich in Frame 0 → Frame 1 zieht mit.
        let before = sp.clone();
        sp.active().set(1, 1, 3);
        let r = sync_sprite(&mut sp, &before, true, NewTiles::Auto);
        assert_eq!(r.edited, 1);
        assert_eq!(sp.cel(1, 0).get(1, 1), 3);
        // Kacheln setzen (paint = false): keine Kachel ändert sich.
        let before = sp.clone();
        let ts = sp.layers[0].tileset.clone().unwrap();
        ts.place(sp.active(), 0, 0, 2);
        let r = sync_sprite(&mut sp, &before, false, NewTiles::Auto);
        assert_eq!(r, SyncSum::default());
        assert_eq!(sp.cel(1, 0).get(0, 0), 1, "anderer Frame unverändert");
    }

    #[test]
    fn godot_szene_wie_im_web() {
        assert_eq!(tile_map_data(&[vec![0, 1]], 1), vec![0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(atlas_cols(5), 3);
        let (ts, g) = level();
        let (w, h, px) = ts.atlas();
        assert_eq!((w, h), (4, 2));
        assert_eq!(px, vec![1, 0, 2, 2, 0, 0, 2, 2]);
        let l = GodotLayer { name: "Boden", ts: &ts, map: ts.map_of(&g), png: "res://level/level_boden.png".into(), visible: true, opacity: 0.5 };
        let scene = godot_scene("Level \"1\"", std::slice::from_ref(&l));
        assert!(scene.starts_with("[gd_scene load_steps=4 format=3]"));
        assert!(scene.contains("path=\"res://level/level_boden.png\""));
        assert!(scene.contains("0:0/0 = 0\n1:0/0 = 0"));
        assert!(scene.contains("[node name=\"Level \\\"1\\\"\" type=\"Node2D\"]"));
        assert!(scene.contains("modulate = Color(1, 1, 1, 0.5)"));
        let j: Value = serde_json::from_str(&tilemap_json("Level", &[l])).unwrap();
        assert_eq!(j["layers"][0]["data"], json!([1, 2, 1, 0, 1, 0]));
        assert_eq!(j["layers"][0]["atlas"], "level_boden.png");
    }

    #[test]
    fn json_hin_und_zurueck() {
        let (ts, _) = level();
        let v = ts.to_json(|p| json!(p));
        let back = Tileset::from_json(&v, |v| v.as_u64().unwrap_or(0) as Px).unwrap();
        assert_eq!(back, ts);
        assert!(Tileset::from_json(&json!({ "tw": 0, "th": 2 }), |_| 0).is_none());
    }
}
