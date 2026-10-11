//! Kachelsätze speichern und wiederverwenden — wie im Web (js/tilelib.js).
//!
//! Ein Kachelsatz hängt an seiner Tilemap-Ebene ([`crate::tilemap`]). Damit
//! man ihn in einer anderen Ebene, einem anderen Sprite oder einem anderen
//! Projekt wieder nutzen kann, gibt es
//!
//! * die Bibliothek des Projekts ([`crate::Project::tileset_lib`])
//! * eine Datei (`.tileset.json`) für andere Projekte — oder ein fremdes
//!   Kachelbild (PNG), das in Kacheln zerschnitten wird.
//!
//! In Bibliothek und Datei stehen Farben, keine Palettennummern: so passt ein
//! Satz in jeden Sprite, auch mit anderer Palette. Beim Einsetzen wird jede
//! Farbe zur passenden Nummer der Sprite-Palette, sonst zur freien Farbe.
//! Eingesetzt wird immer eine Kopie; die Ebene merkt sich in
//! [`Tileset::lib`], woher sie kommt.
//!
//! Dateiformat und Bibliothek im Speicherstand sind dieselben wie im Web.

use std::collections::{BTreeMap, HashMap};

use serde_json::{json, Value};

use crate::image::{Image, Px, FREE_BASE};
use crate::palette::{parse_hex, Palette, Rgb};
use crate::tilemap::Tileset;

/// Kennung im Dateiformat.
pub const FILE_FORMAT: &str = "spritebit-tileset";

/// Ein Satz der Bibliothek: je Kachel `tw * th` Pixel, `None` = leer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibSet {
    pub tw: u32,
    pub th: u32,
    pub tiles: Vec<Vec<Option<Rgb>>>,
}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

impl LibSet {
    /// Wie im Web: `{ tw, th, tiles: [[[0 | "#rrggbb", …], …], …] }`.
    pub fn to_json(&self) -> Value {
        let tiles: Vec<Value> = self
            .tiles
            .iter()
            .map(|t| {
                Value::Array(t.chunks(self.tw as usize).map(|r| Value::Array(r.iter().map(|c| c.map_or(json!(0), |c| json!(hex(c)))).collect())).collect())
            })
            .collect();
        json!({ "tw": self.tw, "th": self.th, "tiles": tiles })
    }

    /// Unbrauchbares → `None`; einzelne kaputte Kacheln fallen weg.
    pub fn from_json(v: &Value) -> Option<Self> {
        let tw = v.get("tw")?.as_u64()? as u32;
        let th = v.get("th")?.as_u64()? as u32;
        if !(1..=256).contains(&tw) || !(1..=256).contains(&th) {
            return None;
        }
        let mut tiles = Vec::new();
        for t in v.get("tiles").and_then(Value::as_array).into_iter().flatten() {
            let Some(rows) = t.as_array().filter(|r| r.len() == th as usize) else { continue };
            let mut data = Vec::with_capacity((tw * th) as usize);
            let mut ok = true;
            for r in rows {
                match r.as_array().filter(|r| r.len() == tw as usize) {
                    Some(r) => data.extend(r.iter().map(|p| p.as_str().and_then(parse_hex))),
                    None => ok = false,
                }
            }
            if ok {
                tiles.push(data);
            }
        }
        Some(LibSet { tw, th, tiles })
    }
}

/// Die Bibliothek für den Speicherstand: Name → Satz.
pub fn lib_to_json(lib: &BTreeMap<String, LibSet>) -> Value {
    Value::Object(lib.iter().map(|(n, s)| (n.clone(), s.to_json())).collect())
}

/// Aus dem Speicherstand — nur brauchbare Sätze mit Namen.
pub fn lib_from_json(v: Option<&Value>) -> BTreeMap<String, LibSet> {
    let mut out = BTreeMap::new();
    for (name, s) in v.and_then(Value::as_object).into_iter().flatten() {
        if let (false, Some(set)) = (name.trim().is_empty(), LibSet::from_json(s)) {
            out.insert(name.trim().to_string(), set);
        }
    }
    out
}

/// Farbe eines Pixelwerts (0 → `None`).
fn color_of(v: Px, pal: &Palette, free: &[Rgb]) -> Option<Rgb> {
    if v >= FREE_BASE {
        free.get((v - FREE_BASE) as usize).copied()
    } else {
        pal.get(v)
    }
}

/// Kachelsatz einer Ebene → Satz der Bibliothek (Farben statt Nummern).
pub fn to_lib(ts: &Tileset, pal: &Palette, free: &[Rgb]) -> LibSet {
    LibSet { tw: ts.tw, th: ts.th, tiles: ts.tiles.iter().map(|t| t.iter().map(|&v| color_of(v, pal, free)).collect()).collect() }
}

/// Satz der Bibliothek → Kacheln für einen Sprite mit Palette `pal`: jede
/// Farbe wird ihre Nummer (die erste mit dieser Farbe), sonst freie Farbe
/// (neue werden an `free` angehängt).
pub fn from_lib(set: &LibSet, pal: &Palette, free: &mut Vec<Rgb>) -> Vec<Vec<Px>> {
    let mut idx: HashMap<Rgb, Px> = HashMap::new();
    for (k, &c) in pal.colors.iter().enumerate() {
        idx.entry(c).or_insert(k as Px + 1);
    }
    let mut val = |c: Option<Rgb>| -> Px {
        let Some(c) = c else { return 0 };
        if let Some(&i) = idx.get(&c) {
            return i;
        }
        let k = free.iter().position(|&f| f == c).unwrap_or_else(|| {
            free.push(c);
            free.len() - 1
        });
        FREE_BASE + k as Px
    };
    set.tiles.iter().map(|t| t.iter().map(|&c| val(c)).collect()).collect()
}

/// Andere Kacheln in eine Tilemap-Ebene legen, die Anordnung bleibt: wo
/// vorher Kachel k lag, liegt danach die neue Kachel k. Zellen mit einer
/// Nummer, die es im neuen Satz nicht gibt, und Inhalt, der keine Kachel
/// war, bleiben. `false`, wenn die Kachelgröße nicht passt.
pub fn retile(ts: &mut Tileset, tiles: Vec<Vec<Px>>, imgs: &mut [&mut Image]) -> bool {
    let n = (ts.tw * ts.th) as usize;
    if tiles.iter().any(|t| t.len() != n) {
        return false;
    }
    let maps: Vec<Vec<Vec<i64>>> = imgs.iter().map(|img| ts.map_of(img)).collect();
    ts.tiles = tiles;
    for (img, map) in imgs.iter_mut().zip(maps) {
        for (cy, row) in map.iter().enumerate() {
            for (cx, &k) in row.iter().enumerate() {
                if k > 0 && (k as usize) <= ts.tiles.len() {
                    ts.place(img, cx as u32, cy as u32, k as usize);
                }
            }
        }
    }
    true
}

/// Freier Name in der Bibliothek: „Wald“, „Wald 2“, …
pub fn free_name(lib: &BTreeMap<String, LibSet>, base: &str) -> String {
    let b = if base.trim().is_empty() { "Tileset" } else { base.trim() };
    if !lib.contains_key(b) {
        return b.to_string();
    }
    (2..).map(|n| format!("{b} {n}")).find(|n| !lib.contains_key(n)).expect("irgendein Name ist frei")
}

/// Satz als Datei (JSON-Text) — dieselbe wie im Web.
pub fn file_json(name: &str, set: &LibSet) -> String {
    let mut v = set.to_json();
    v["format"] = json!(FILE_FORMAT);
    v["version"] = json!(1);
    v["name"] = json!(name);
    v.to_string()
}

/// Datei lesen → (Name, Satz).
pub fn parse_file(text: &str) -> Option<(String, LibSet)> {
    let v: Value = serde_json::from_str(text).ok()?;
    if v.get("format").and_then(Value::as_str) != Some(FILE_FORMAT) {
        return None;
    }
    let set = LibSet::from_json(&v)?;
    Some((v.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string(), set))
}

/// Fremdes Kachelbild (RGBA) in Kacheln `tw × th` zerschneiden: Zeile für
/// Zeile, leere und doppelte Kacheln fallen weg. Alpha < 128 gilt als leer;
/// der Rand, der keine ganze Kachel mehr ergibt, bleibt außen vor.
pub fn tiles_from_image(rgba: &[u8], w: u32, h: u32, tw: u32, th: u32) -> LibSet {
    let mut tiles: Vec<Vec<Option<Rgb>>> = Vec::new();
    let mut cy = 0;
    while cy + th <= h {
        let mut cx = 0;
        while cx + tw <= w {
            let t: Vec<Option<Rgb>> = (0..th)
                .flat_map(|y| (0..tw).map(move |x| (x, y)))
                .map(|(x, y)| {
                    let o = (((cy + y) * w + cx + x) * 4) as usize;
                    (rgba[o + 3] >= 128).then(|| [rgba[o], rgba[o + 1], rgba[o + 2]])
                })
                .collect();
            if t.iter().any(Option::is_some) && !tiles.contains(&t) {
                tiles.push(t);
            }
            cx += tw;
        }
        cy += th;
    }
    LibSet { tw, th, tiles }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rgb = [0xff, 0, 0];
    const G: Rgb = [0, 0xff, 0];
    const B: Rgb = [0, 0, 0xff];

    fn pal() -> Palette {
        Palette::new("t", vec![R, G, B])
    }

    fn img(rows: &[&[Px]]) -> Image {
        let mut i = Image::new(rows[0].len() as u32, rows.len() as u32);
        for (y, r) in rows.iter().enumerate() {
            for (x, &v) in r.iter().enumerate() {
                i.set(x as u32, y as u32, v);
            }
        }
        i
    }

    fn rows(i: &Image) -> Vec<Vec<Px>> {
        (0..i.height()).map(|y| (0..i.width()).map(|x| i.get(x, y)).collect()).collect()
    }

    #[test]
    fn bibliothek_speichert_farben_und_passt_zu_anderer_palette() {
        let mut ts = Tileset::new(2, 2);
        ts.tiles.push(vec![1, 0, 2, FREE_BASE]);
        let set = to_lib(&ts, &pal(), &[[0x12, 0x34, 0x56]]);
        assert_eq!(set.tiles[0], vec![Some(R), None, Some(G), Some([0x12, 0x34, 0x56])]);
        // Andere Palette: andere Nummern, gleiches Aussehen.
        let other = Palette::new("o", vec![G, R]);
        let mut free = Vec::new();
        assert_eq!(from_lib(&set, &other, &mut free)[0], vec![2, 0, 1, FREE_BASE]);
        assert_eq!(free, vec![[0x12, 0x34, 0x56]]);
    }

    #[test]
    fn anordnung_bleibt_beim_einsetzen() {
        let mut a = img(&[&[1, 1, 2, 2], &[1, 1, 2, 2]]);
        let mut ts = Tileset::build(2, 2, [&a]);
        assert!(retile(&mut ts, vec![vec![3; 4], vec![0, 1, 1, 0]], &mut [&mut a]));
        assert_eq!(rows(&a), vec![vec![3, 3, 0, 1], vec![3, 3, 1, 0]]);
        assert!(!retile(&mut ts, vec![vec![1; 3]], &mut [&mut a]), "falsche Kachelgröße");
    }

    #[test]
    fn datei_wie_im_web() {
        let set = LibSet { tw: 2, th: 2, tiles: vec![vec![Some(R), None, None, Some(G)]] };
        let text = file_json("Wald", &set);
        assert_eq!(parse_file(&text), Some(("Wald".into(), set)));
        // So schreibt sie die Web-Version (js/tilelib.js tilesetFile).
        let web = r##"{"format":"spritebit-tileset","version":1,"name":"Wald","tw":2,"th":2,"tiles":[[["#ff0000",0],[0,"#00ff00"]]]}"##;
        assert_eq!(parse_file(web).map(|(n, s)| (n, s.tiles.len())), Some(("Wald".into(), 1)));
        assert_eq!(parse_file(r#"{"format":"anders"}"#), None);
    }

    #[test]
    fn kachelbild_zerschneiden() {
        // 5 × 2: Kachel rot, Kachel leer, Rand — und rot doppelt fällt weg.
        let mut rgba = Vec::new();
        for _y in 0..2 {
            for x in 0..5 {
                let red = x < 2 || x == 4;
                rgba.extend([if red { 255 } else { 0 }, 0, 0, if red { 255 } else { 0 }]);
            }
        }
        let set = tiles_from_image(&rgba, 5, 2, 2, 2);
        assert_eq!(set.tiles, vec![vec![Some(R); 4]]);
    }

    #[test]
    fn freie_namen() {
        let mut lib = BTreeMap::new();
        assert_eq!(free_name(&lib, "Wald"), "Wald");
        lib.insert("Wald".to_string(), LibSet { tw: 1, th: 1, tiles: vec![] });
        assert_eq!(free_name(&lib, "Wald"), "Wald 2");
        assert_eq!(free_name(&lib, "  "), "Tileset");
    }
}
