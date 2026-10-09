//! Speichern und Laden.
//!
//! Zwei Formate:
//!
//! * **`.spritebit`** — das eigene Format. Eine Kopfzeile als JSON (Sprites,
//!   Ebenen, Frames, Tags, Paletten) und danach nur die bemalten Kacheln,
//!   alles mit zlib komprimiert. Eine 8192×8192-Fläche mit einer kleinen
//!   Figur bleibt so auch als Datei klein.
//! * **Web-Projekt (`.json`)** — das Format der Projektdatei der
//!   Web-Version (Schema 2): jedes Pixel als Zahl, freie Farben als
//!   `"#rrggbb"`, verknüpfte Zellen als `{ "link": k }`. Importieren und
//!   exportieren, damit Arbeiten zwischen beiden Versionen wandern können.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Write};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde_json::{json, Map, Value};

use crate::image::{Image, Px, FREE_BASE, TILE};
use crate::palette::{parse_hex, Palette, Rgb};
use crate::project::Project;
use crate::sprite::{Direction, Frame, Layer, Sprite, Tag, MAX_SIDE, Guides};

const MAGIC: &[u8; 10] = b"SPRITEBIT\0";
const NATIVE_VERSION: u32 = 1;
const TILE_PX: usize = (TILE * TILE) as usize;

#[derive(Debug, PartialEq)]
pub enum IoError {
    /// Keine Projektdatei (falsches Format, kaputtes JSON).
    NotAProject(String),
    /// Erkannt, aber diese Fassung wird nicht unterstützt.
    Unsupported(String),
    /// Eine Fläche ist größer als [`MAX_SIDE`].
    TooBig { width: u32, height: u32 },
    /// Datei ist beschädigt (abgeschnitten, Nummern zeigen ins Leere).
    Corrupt(String),
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IoError::NotAProject(why) => write!(f, "Keine spritebit-Projektdatei ({why})."),
            IoError::Unsupported(why) => write!(f, "Diese Projektdatei wird nicht unterstützt: {why}"),
            IoError::TooBig { width, height } => {
                write!(f, "Fläche {width} × {height} ist zu groß (höchstens {MAX_SIDE} × {MAX_SIDE}).")
            }
            IoError::Corrupt(why) => write!(f, "Die Datei ist beschädigt ({why})."),
        }
    }
}

impl std::error::Error for IoError {}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn dir_name(d: Direction) -> &'static str {
    match d {
        Direction::Forward => "forward",
        Direction::Reverse => "reverse",
        Direction::PingPong => "pingpong",
    }
}

fn parse_dir(s: Option<&str>) -> Direction {
    match s {
        Some("reverse") => Direction::Reverse,
        Some("pingpong") => Direction::PingPong,
        _ => Direction::Forward,
    }
}

/// `px` schreibt ein Pixel einer Kachel: im eigenen Format als Zahl, fürs
/// Web als Zahl bzw. Hex (freie Farbe).
fn layer_json(l: &Layer, px: &dyn Fn(Px) -> Value) -> Value {
    let mut v = json!({ "name": l.name, "visible": l.visible, "locked": l.locked, "opacity": l.opacity, "continuous": l.continuous });
    if let Some(fx) = &l.fx {
        v["fx"] = fx.to_json();
    }
    if let Some(m) = &l.mask {
        v["mask"] = m.to_json();
    }
    if let Some(ts) = &l.tileset {
        v["tileset"] = ts.to_json(px);
    }
    v
}

/// Kachelsätze der Tilemap-Ebenen. `web`: Werte als Zahl (Palette) oder Hex
/// (freie Farbe, landet in `sp.free`); sonst rohe Pixelwerte.
fn read_tilesets(sp: &mut Sprite, layers: Option<&Vec<Value>>, web: bool) {
    for i in 0..sp.layers.len() {
        let Some(v) = layers.and_then(|ls| ls.get(i)).and_then(|v| v.get("tileset")) else { continue };
        let ts = crate::tilemap::Tileset::from_json(v, |p| match p {
            Value::Number(n) if web => n.as_u64().filter(|&n| n < FREE_BASE as u64).unwrap_or(0) as Px,
            Value::Number(n) => n.as_u64().filter(|&n| n <= Px::MAX as u64).unwrap_or(0) as Px,
            Value::String(s) if web => parse_hex(s).map_or(0, |c| sp.free_color(c)),
            _ => 0,
        });
        sp.layers[i].tileset = ts;
    }
}

fn parse_layer(v: Option<&Value>, n: usize) -> Layer {
    let mut l = Layer::new(format!("Ebene {n}"));
    if let Some(v) = v {
        if let Some(name) = v.get("name").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
            l.name = name.trim().to_string();
        }
        l.visible = v.get("visible").and_then(Value::as_bool).unwrap_or(true);
        l.locked = v.get("locked").and_then(Value::as_bool).unwrap_or(false);
        l.opacity = v.get("opacity").and_then(Value::as_f64).map_or(1.0, |o| o.clamp(0.0, 1.0) as f32);
        l.continuous = v.get("continuous").and_then(Value::as_bool).unwrap_or(false);
        l.fx = v.get("fx").and_then(crate::light::LayerFx::from_json);
    }
    l
}

/// Masken brauchen die Größe des Sprites — darum nach parse_layer.
fn read_masks(sp: &mut Sprite, layers: Option<&Vec<Value>>) {
    let (w, h) = (sp.width, sp.height);
    for (i, l) in sp.layers.iter_mut().enumerate() {
        l.mask = layers.and_then(|ls| ls.get(i)).and_then(|v| v.get("mask")).and_then(|m| crate::mask::Mask::from_json(m, w, h));
    }
}

fn tag_json(t: &Tag) -> Value {
    json!({ "name": t.name, "from": t.from, "to": t.to, "color": hex(t.color), "dir": dir_name(t.direction) })
}

fn parse_tags(v: Option<&Value>, frames: usize) -> Vec<Tag> {
    let Some(list) = v.and_then(Value::as_array) else { return Vec::new() };
    list.iter()
        .filter_map(|t| {
            let a = t.get("from")?.as_u64()? as usize;
            let b = t.get("to")?.as_u64()? as usize;
            let (from, to) = (a.min(b), a.max(b).min(frames.checked_sub(1)?));
            if from > to {
                return None;
            }
            Some(Tag {
                name: t.get("name").and_then(Value::as_str).unwrap_or("Tag").to_string(),
                from,
                to,
                color: t.get("color").and_then(Value::as_str).and_then(parse_hex).unwrap_or([0x4a, 0xa3, 0xdf]),
                direction: parse_dir(t.get("dir").and_then(Value::as_str)),
            })
        })
        .collect()
}

/// Hilfslinien als JSON (auch für die eigenen Layouts der App).
pub fn guides_json(g: &Guides) -> Value {
    json!({ "h": g.h, "v": g.v, "heads": g.heads, "top": g.top, "bottom": g.bottom })
}

/// Hilfslinien aus JSON, auf `w × h` begrenzt.
pub fn parse_guides(v: Option<&Value>, w: u32, h: u32) -> Guides {
    let nums = |k: &str| -> Vec<u32> {
        v.and_then(|g| g.get(k)).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect()).unwrap_or_default()
    };
    let num = |k: &str| v.and_then(|g| g.get(k)).and_then(Value::as_u64).map(|x| x as u32);
    Guides { h: nums("h"), v: nums("v"), heads: num("heads").unwrap_or(0), top: num("top").unwrap_or(0), bottom: num("bottom").unwrap_or(h) }
        .normalized(w, h)
}

fn materials_json(m: &BTreeMap<String, BTreeMap<u16, String>>) -> Value {
    Value::Object(m.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| (k.clone(), json!(v.iter().map(|(i, m)| (i.to_string(), Value::String(m.clone()))).collect::<serde_json::Map<_, _>>()))).collect())
}

fn parse_materials(v: Option<&Value>) -> BTreeMap<String, BTreeMap<u16, String>> {
    let Some(o) = v.and_then(Value::as_object) else { return BTreeMap::new() };
    o.iter()
        .map(|(name, m)| {
            let inner: BTreeMap<u16, String> = m
                .as_object()
                .map(|m| m.iter().filter_map(|(i, v)| Some((i.parse::<u16>().ok()?, v.as_str()?.to_string()))).collect())
                .unwrap_or_default();
            (name.clone(), inner)
        })
        .filter(|(_, m)| !m.is_empty())
        .collect()
}

fn palette_json(p: &Palette) -> Value {
    Value::Array(p.colors.iter().map(|&c| Value::String(hex(c))).collect())
}

/// Nach dem Laden: aktive Frame-/Ebenen-Nummern in den gültigen Bereich.
fn clamp_cursor(sp: &mut Sprite) {
    sp.frame = sp.frame.min(sp.frames.len() - 1);
    sp.layer = sp.layer.min(sp.layers.len() - 1);
}

// ════════════════════════════════════════════════════════════════════
// Eigenes Format
// ════════════════════════════════════════════════════════════════════

/// Projekt als `.spritebit`-Datei.
pub fn save_native(p: &Project) -> Vec<u8> {
    // Bilder, auf die keine Zelle mehr zeigt, nicht mitspeichern. Der Klon
    // ist billig — er teilt die Kacheln.
    let sprites: Vec<Sprite> = p
        .sprites
        .iter()
        .map(|s| {
            let mut s = s.clone();
            s.collect_garbage();
            s
        })
        .collect();
    let header = json!({
        "current": p.current,
        "palettes": p.palettes.iter().map(|pal| json!({ "name": pal.name, "colors": palette_json(pal) })).collect::<Vec<_>>(),
        "materials": materials_json(&p.materials),
        "sprites": sprites.iter().map(|s| json!({
            "name": s.name, "width": s.width, "height": s.height, "palette": s.palette, "fps": s.fps,
            "frame": s.frame, "layer": s.layer,
            "layers": s.layers.iter().map(|l| layer_json(l, &|p| json!(p))).collect::<Vec<_>>(),
            "free": s.free.iter().map(|&c| hex(c)).collect::<Vec<_>>(),
            "tags": s.tags.iter().map(tag_json).collect::<Vec<_>>(),
            "guides": guides_json(&s.guides),
            "frames": s.frames.iter().map(|f| json!({ "cels": f.cels, "dur": f.duration_ms })).collect::<Vec<_>>(),
            "images": s.images.len(),
        })).collect::<Vec<_>>(),
    });
    let head = serde_json::to_vec(&header).expect("JSON aus eigenen Daten");
    let mut body = Vec::with_capacity(head.len() + 1024);
    body.extend_from_slice(&(head.len() as u32).to_le_bytes());
    body.extend_from_slice(&head);
    for s in &sprites {
        for img in &s.images {
            let tiles: Vec<_> = img.tiles().collect();
            body.extend_from_slice(&(tiles.len() as u32).to_le_bytes());
            for (i, data) in tiles {
                body.extend_from_slice(&(i as u32).to_le_bytes());
                for px in data {
                    body.extend_from_slice(&px.to_le_bytes());
                }
            }
        }
    }
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&body).expect("in den Speicher schreiben");
    let z = enc.finish().expect("in den Speicher schreiben");
    let mut out = Vec::with_capacity(z.len() + 14);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&NATIVE_VERSION.to_le_bytes());
    out.extend_from_slice(&z);
    out
}

/// Liest Zahlen aus dem entpackten Inhalt.
struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], IoError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.b.len()).ok_or_else(|| IoError::Corrupt("zu kurz".into()))?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, IoError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
}

/// `.spritebit`-Datei lesen.
pub fn load_native(bytes: &[u8]) -> Result<Project, IoError> {
    if bytes.len() < 14 || &bytes[..10] != MAGIC {
        return Err(IoError::NotAProject("Kennung fehlt".into()));
    }
    let version = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]);
    if version != NATIVE_VERSION {
        return Err(IoError::Unsupported(format!("Fassung {version} ist neuer als dieses Programm")));
    }
    let mut body = Vec::new();
    ZlibDecoder::new(&bytes[14..]).read_to_end(&mut body).map_err(|e| IoError::Corrupt(e.to_string()))?;
    let mut r = Reader { b: &body, pos: 0 };
    let head_len = r.u32()? as usize;
    let head: Value = serde_json::from_slice(r.take(head_len)?).map_err(|e| IoError::Corrupt(e.to_string()))?;

    let palettes = head["palettes"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|p| {
                    let colors = p["colors"].as_array().map_or_else(Vec::new, |c| {
                        c.iter().map(|h| h.as_str().and_then(parse_hex).unwrap_or([0, 0, 0])).collect()
                    });
                    Palette::new(p["name"].as_str().unwrap_or("palette"), colors)
                })
                .collect()
        })
        .unwrap_or_default();

    let mut sprites = Vec::new();
    for s in head["sprites"].as_array().ok_or_else(|| IoError::Corrupt("keine Sprites".into()))? {
        let (w, h) = (s["width"].as_u64().unwrap_or(0) as u32, s["height"].as_u64().unwrap_or(0) as u32);
        let mut sp = Sprite::new(s["name"].as_str().unwrap_or("Sprite"), w, h)
            .map_err(|_| IoError::TooBig { width: w, height: h })?;
        sp.palette = s["palette"].as_str().unwrap_or("graustufen").to_string();
        sp.fps = s["fps"].as_u64().map_or(8, |v| v.clamp(1, 60) as u32);
        let layers = s["layers"].as_array().ok_or_else(|| IoError::Corrupt("keine Ebenen".into()))?;
        sp.layers = layers.iter().enumerate().map(|(i, l)| parse_layer(Some(l), i + 1)).collect();
        read_masks(&mut sp, Some(layers));
        read_tilesets(&mut sp, Some(layers), false);
        sp.free = s["free"].as_array().map_or_else(Vec::new, |f| {
            f.iter().map(|h| h.as_str().and_then(parse_hex).unwrap_or([0, 0, 0])).collect()
        });
        let n_images = s["images"].as_u64().unwrap_or(0) as usize;
        sp.frames = s["frames"]
            .as_array()
            .ok_or_else(|| IoError::Corrupt("keine Frames".into()))?
            .iter()
            .map(|f| Frame {
                cels: f["cels"].as_array().map_or_else(Vec::new, |c| c.iter().map(|v| v.as_u64().unwrap_or(0) as usize).collect()),
                duration_ms: f["dur"].as_u64().unwrap_or(0) as u32,
            })
            .collect();
        if sp.layers.is_empty() || sp.frames.is_empty() {
            return Err(IoError::Corrupt("leerer Sprite".into()));
        }
        if sp.frames.iter().any(|f| f.cels.len() != sp.layers.len() || f.cels.iter().any(|&id| id >= n_images)) {
            return Err(IoError::Corrupt("Zellen zeigen ins Leere".into()));
        }
        sp.images = (0..n_images).map(|_| Image::new(w, h)).collect();
        for img in &mut sp.images {
            let n = r.u32()?;
            for _ in 0..n {
                let index = r.u32()? as usize;
                let raw = r.take(TILE_PX * 2)?;
                let data: Vec<Px> = raw.as_chunks::<2>().0.iter().map(|c| Px::from_le_bytes(*c)).collect();
                if !img.put_tile(index, data) {
                    return Err(IoError::Corrupt("Kachel außerhalb".into()));
                }
            }
        }
        sp.tags = parse_tags(s.get("tags"), sp.frames.len());
        sp.guides = parse_guides(s.get("guides"), sp.width, sp.height);
        sp.frame = s["frame"].as_u64().unwrap_or(0) as usize;
        sp.layer = s["layer"].as_u64().unwrap_or(0) as usize;
        clamp_cursor(&mut sp);
        sprites.push(sp);
    }
    if sprites.is_empty() {
        return Err(IoError::Corrupt("keine Sprites".into()));
    }
    let current = (head["current"].as_u64().unwrap_or(0) as usize).min(sprites.len() - 1);
    Ok(Project { sprites, palettes, current, materials: parse_materials(head.get("materials")) })
}

// ════════════════════════════════════════════════════════════════════
// Web-Projekt (Projektdatei der Web-Version, Schema 2)
// ════════════════════════════════════════════════════════════════════

/// Projektdatei der Web-Version lesen.
pub fn import_web(text: &str) -> Result<Project, IoError> {
    let v: Value = serde_json::from_str(text).map_err(|e| IoError::NotAProject(e.to_string()))?;
    if v.get("grids").is_some() || v.get("version").is_none() {
        return Err(IoError::Unsupported(
            "alte Fassung der Web-Version — bitte dort einmal öffnen und neu sichern".into(),
        ));
    }
    let list = v.get("sprites").and_then(Value::as_object).ok_or_else(|| IoError::NotAProject("keine Sprites".into()))?;

    let mut palettes = Vec::new();
    if let Some(custom) = v.get("customPalettes").and_then(Value::as_object) {
        for (name, pal) in custom {
            let Some(obj) = pal.as_object() else { continue };
            let max = obj.keys().filter_map(|k| k.parse::<usize>().ok()).max().unwrap_or(0);
            let colors = (1..=max)
                .map(|i| obj.get(&i.to_string()).and_then(Value::as_str).and_then(parse_hex).unwrap_or([0, 0, 0]))
                .collect();
            palettes.push(Palette::new(name.clone(), colors));
        }
    }

    let mut sprites = Vec::new();
    let mut ids = Vec::new();
    for (id, s) in list {
        let Some(frames) = s.get("frames").and_then(Value::as_array) else { continue };
        // Zellen eines Frames: `cels` (mehrere Ebenen) oder `grid` (eine).
        let cels_of = |f: &Value| -> Vec<Value> {
            match (f.get("cels").and_then(Value::as_array), f.get("grid")) {
                (Some(c), _) => c.clone(),
                (None, Some(g)) => vec![g.clone()],
                _ => Vec::new(),
            }
        };
        let all: Vec<Vec<Value>> = frames.iter().map(cels_of).collect();
        let Some(first) = all.iter().flatten().find_map(|c| c.as_array()) else { continue };
        let h = first.len() as u32;
        let w = first.first().and_then(Value::as_array).map_or(0, |r| r.len()) as u32;
        if w > MAX_SIDE || h > MAX_SIDE {
            return Err(IoError::TooBig { width: w, height: h });
        }
        let name = s.get("name").and_then(Value::as_str).unwrap_or(id);
        let Ok(mut sp) = Sprite::new(name, w.max(1), h.max(1)) else { continue };
        let n_layers = all.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let layer_list = s.get("layers").and_then(Value::as_array);
        sp.layers = (0..n_layers).map(|i| parse_layer(layer_list.and_then(|l| l.get(i)), i + 1)).collect();
        read_masks(&mut sp, layer_list);
        read_tilesets(&mut sp, layer_list, true);
        sp.frames.clear();
        sp.images.clear();
        for (i, (cels, fv)) in all.iter().zip(frames).enumerate() {
            let mut ids_here = Vec::with_capacity(n_layers);
            for l in 0..n_layers {
                let cel = cels.get(l);
                let link = cel.and_then(|c| c.get("link")).and_then(Value::as_u64).map(|k| k as usize);
                let id = match (cel.and_then(Value::as_array), link) {
                    (_, Some(k)) if k < i => sp.frames[k].cels[l],
                    (Some(rows), _) => {
                        let mut img = Image::new(sp.width, sp.height);
                        for (y, row) in rows.iter().enumerate().take(sp.height as usize) {
                            let Some(row) = row.as_array() else { continue };
                            for (x, px) in row.iter().enumerate().take(sp.width as usize) {
                                let val: Px = match px {
                                    Value::Number(n) => n.as_u64().filter(|&n| n < FREE_BASE as u64).unwrap_or(0) as Px,
                                    Value::String(s) => parse_hex(s).map_or(0, |c| sp.free_color(c)),
                                    _ => 0,
                                };
                                if val != 0 {
                                    img.set(x as u32, y as u32, val);
                                }
                            }
                        }
                        sp.images.push(img);
                        sp.images.len() - 1
                    }
                    _ => {
                        sp.images.push(Image::new(sp.width, sp.height));
                        sp.images.len() - 1
                    }
                };
                ids_here.push(id);
            }
            sp.frames.push(Frame { cels: ids_here, duration_ms: fv.get("dur").and_then(Value::as_u64).unwrap_or(0) as u32 });
        }
        sp.palette = s.get("palette").and_then(Value::as_str).unwrap_or("graustufen").to_string();
        sp.fps = s.get("fps").and_then(Value::as_u64).map_or(8, |v| v.clamp(1, 60) as u32);
        sp.tags = parse_tags(s.get("tags"), sp.frames.len());
        sp.guides = parse_guides(s.get("guides"), sp.width, sp.height);
        sp.frame = s.get("frame").and_then(Value::as_u64).unwrap_or(0) as usize;
        sp.layer = s.get("layer").and_then(Value::as_u64).unwrap_or(0) as usize;
        clamp_cursor(&mut sp);
        sprites.push(sp);
        ids.push(id.clone());
    }
    if sprites.is_empty() {
        return Err(IoError::NotAProject("keine lesbaren Sprites".into()));
    }
    let cur = v.pointer("/ui/curSprite").and_then(Value::as_str);
    let current = cur.and_then(|c| ids.iter().position(|id| id == c)).unwrap_or(0);
    Ok(Project { sprites, palettes, current, materials: parse_materials(v.get("paletteMaterials")) })
}

/// Id aus einem Namen, wie die Web-Version sie bildet — und eindeutig.
fn web_id(name: &str, taken: &[String]) -> String {
    let base: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    let base = base.trim_matches('_').to_string();
    let base = if base.is_empty() { "sprite".to_string() } else { base };
    if !taken.contains(&base) {
        return base;
    }
    (2..).map(|i| format!("{base}_{i}")).find(|c| !taken.contains(c)).expect("unendlich viele Kandidaten")
}

/// Projekt als Projektdatei der Web-Version (lesbar mit „Öffnen" dort).
pub fn export_web(p: &Project) -> String {
    let mut sprites = Map::new();
    let mut ids: Vec<String> = Vec::new();
    for sp in &p.sprites {
        let id = web_id(&sp.name, &ids);
        ids.push(id.clone());
        let px_json = |v: Px| -> Value {
            if v >= FREE_BASE {
                sp.free.get((v - FREE_BASE) as usize).map_or(json!(0), |&c| json!(hex(c)))
            } else {
                json!(v)
            }
        };
        let frames: Vec<Value> = sp
            .frames
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let cels: Vec<Value> = f
                    .cels
                    .iter()
                    .enumerate()
                    .map(|(l, &img_id)| {
                        if let Some(k) = (0..i).find(|&k| sp.frames[k].cels[l] == img_id) {
                            return json!({ "link": k });
                        }
                        let img = &sp.images[img_id];
                        Value::Array(
                            (0..sp.height)
                                .map(|y| Value::Array((0..sp.width).map(|x| px_json(img.get(x, y))).collect()))
                                .collect(),
                        )
                    })
                    .collect();
                if f.duration_ms > 0 {
                    json!({ "cels": cels, "dur": f.duration_ms })
                } else {
                    json!({ "cels": cels })
                }
            })
            .collect();
        sprites.insert(
            id,
            json!({
                "name": sp.name, "palette": sp.palette, "fps": sp.fps, "frame": sp.frame, "layer": sp.layer,
                "layers": sp.layers.iter().map(|l| layer_json(l, &px_json)).collect::<Vec<_>>(),
                "tags": sp.tags.iter().map(tag_json).collect::<Vec<_>>(),
                "guides": guides_json(&sp.guides),
                "frames": frames,
            }),
        );
    }
    let mut custom = Map::new();
    for pal in &p.palettes {
        let obj: Map<String, Value> = pal.colors.iter().enumerate().map(|(i, &c)| ((i + 1).to_string(), json!(hex(c)))).collect();
        custom.insert(pal.name.clone(), Value::Object(obj));
    }
    let doc = json!({
        "version": 2,
        "sprites": sprites,
        "customPalettes": custom,
        "paletteMaterials": materials_json(&p.materials),
        "ui": { "curSprite": ids.get(p.current) },
    });
    serde_json::to_string(&doc).expect("JSON aus eigenen Daten")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zwei Sprites: einer mit zwei Ebenen, drei Frames, Verknüpfung,
    /// freier Farbe, Tag und Dauer; dazu eine eigene Palette.
    fn sample() -> Project {
        let mut a = Sprite::new("Held", 16, 8).unwrap();
        a.palette = "meine".into();
        a.active().set(1, 1, 3);
        let free = a.free_color([0xab, 0xcd, 0xef]);
        a.active().set(2, 1, free);
        a.add_layer(1, "Oben");
        a.cel_mut(0, 1).set(5, 5, 2);
        a.add_frame(0, true);
        a.add_frame(1, false);
        a.link(0, 2, 0);
        a.frames[1].duration_ms = 250;
        a.layers[1].opacity = 0.5;
        a.layers[0].continuous = true;
        a.tags.push(Tag { name: "Lauf".into(), from: 0, to: 2, color: [1, 2, 3], direction: Direction::PingPong });
        a.guides = Guides { h: vec![3], v: vec![1, 5], heads: 6, top: 2, bottom: 7 };
        let b = Sprite::new("Zweiter", 4, 4).unwrap();
        let mut materials = BTreeMap::new();
        materials.insert("meine".to_string(), BTreeMap::from([(2u16, "sand".to_string())]));
        Project { sprites: vec![a, b], palettes: vec![Palette::new("meine", vec![[9, 9, 9], [8, 8, 8], [7, 7, 7]])], current: 1, materials }
    }

    fn assert_same(a: &Project, b: &Project) {
        assert_eq!(a.current, b.current);
        assert_eq!(a.palettes, b.palettes);
        assert_eq!(a.materials, b.materials);
        assert_eq!(a.sprites.len(), b.sprites.len());
        for (x, y) in a.sprites.iter().zip(&b.sprites) {
            assert_eq!((x.name.as_str(), x.width, x.height, x.fps), (y.name.as_str(), y.width, y.height, y.fps));
            assert_eq!(x.layers, y.layers);
            assert_eq!(x.tags, y.tags);
            assert_eq!(x.guides, y.guides);
            assert_eq!(x.palette, y.palette);
            for f in 0..x.frames.len() {
                assert_eq!(x.frames[f].duration_ms, y.frames[f].duration_ms);
                for l in 0..x.layers.len() {
                    let (cx, cy) = (x.cel(f, l), y.cel(f, l));
                    for py in 0..x.height {
                        for px in 0..x.width {
                            let colour = |s: &Sprite, v: Px| if v >= FREE_BASE { Some(s.free[(v - FREE_BASE) as usize]) } else { None };
                            let (vx, vy) = (cx.get(px, py), cy.get(px, py));
                            assert_eq!(vx < FREE_BASE, vy < FREE_BASE);
                            if vx < FREE_BASE {
                                assert_eq!(vx, vy);
                            } else {
                                assert_eq!(colour(x, vx), colour(y, vy));
                            }
                        }
                    }
                    assert_eq!(x.is_linked(f, l), y.is_linked(f, l), "Verknüpfung Frame {f} Ebene {l}");
                }
            }
        }
    }

    #[test]
    fn eigenes_format_hin_und_zurueck() {
        let p = sample();
        let back = load_native(&save_native(&p)).unwrap();
        assert_same(&p, &back);
        assert_eq!(back.sprites[0].frames[2].cels[0], back.sprites[0].frames[0].cels[0]);
    }

    #[test]
    fn grosse_leere_flaeche_ergibt_kleine_datei() {
        let mut sp = Sprite::new("gross", 8192, 8192).unwrap();
        for i in 0..10 {
            sp.add_frame(i, true);
        }
        sp.active().set(4000, 4000, 1);
        let p = Project { sprites: vec![sp], palettes: vec![], current: 0, materials: BTreeMap::new() };
        let bytes = save_native(&p);
        assert!(bytes.len() < 20_000, "{} Bytes", bytes.len());
        let back = load_native(&bytes).unwrap();
        assert_eq!(back.sprites[0].cel(10, 0).get(4000, 4000), 1);
    }

    #[test]
    fn fremde_und_kaputte_dateien_werden_abgelehnt() {
        assert!(matches!(load_native(b"hallo"), Err(IoError::NotAProject(_))));
        let mut bytes = save_native(&sample());
        bytes.truncate(bytes.len() / 2);
        assert!(matches!(load_native(&bytes), Err(IoError::Corrupt(_))));
        let mut newer = save_native(&sample());
        newer[10] = 99;
        assert!(matches!(load_native(&newer), Err(IoError::Unsupported(_))));
    }

    #[test]
    fn web_projekt_lesen() {
        let text = r##"{
          "version": 2,
          "sprites": {
            "held": {
              "name": "Held", "palette": "meine", "fps": 12, "frame": 1, "layer": 0,
              "layers": [{ "name": "Grund", "continuous": true }, { "name": "Figur", "opacity": 0.5 }],
              "tags": [{ "name": "Lauf", "from": 0, "to": 1, "color": "#4aa3df", "dir": "reverse" }],
              "frames": [
                { "cels": [[[1, 2], [0, "#ABCDEF"]], [[0, 0], [0, 3]]] },
                { "cels": [{ "link": 0 }, [[3, 0], [0, 0]]], "dur": 200 }
              ]
            },
            "alt": { "name": "Alt", "palette": "graustufen", "frames": [{ "grid": [[5]] }] }
          },
          "customPalettes": { "meine": { "1": "#ff0000", "3": "#0000ff" } },
          "ui": { "curSprite": "alt" }
        }"##;
        let p = import_web(text).unwrap();
        assert_eq!(p.sprites.len(), 2);
        assert_eq!(p.current, 1);
        let s = &p.sprites[0];
        assert_eq!((s.width, s.height, s.fps), (2, 2, 12));
        assert_eq!(s.layers[0].name, "Grund");
        assert!(s.layers[0].continuous);
        assert_eq!(s.layers[1].opacity, 0.5);
        assert_eq!(s.cel(0, 0).get(1, 0), 2);
        let free = s.cel(0, 0).get(1, 1);
        assert_eq!(s.free[(free - FREE_BASE) as usize], [0xab, 0xcd, 0xef]);
        assert!(s.is_linked(1, 0));
        assert_eq!(s.frames[1].duration_ms, 200);
        assert_eq!(s.tags[0].direction, Direction::Reverse);
        assert_eq!(p.palettes[0].colors, vec![[255, 0, 0], [0, 0, 0], [0, 0, 255]]);
        assert_eq!(p.sprites[1].cel(0, 0).get(0, 0), 5);
    }

    #[test]
    fn web_export_und_wieder_import() {
        let p = sample();
        let back = import_web(&export_web(&p)).unwrap();
        assert_same(&p, &back);
    }

    #[test]
    fn web_alte_fassung_und_unsinn() {
        assert!(matches!(import_web(r#"{ "grids": {} }"#), Err(IoError::Unsupported(_))));
        assert!(matches!(import_web("kein json"), Err(IoError::NotAProject(_))));
        assert!(matches!(import_web(r#"{ "version": 2, "sprites": {} }"#), Err(IoError::NotAProject(_))));
    }

    #[test]
    fn web_ids_sind_eindeutig() {
        let mut p = sample();
        p.sprites[1].name = "Held".into();
        let doc: Value = serde_json::from_str(&export_web(&p)).unwrap();
        let keys: Vec<_> = doc["sprites"].as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, vec!["Held", "Held_2"]);
    }

    #[test]
    fn effekt_ebenen_ueberstehen_web_und_eigenes_format() {
        let mut p = Project::default();
        let sp = &mut p.sprites[0];
        sp.active().set(3, 3, 2);
        let fx = crate::light::LayerFx {
            kind: crate::light::FxKind::Light(crate::light::LightOpts { width: 2, ..Default::default() }),
            dir: (1, -1),
            src: "xyz".into(),
        };
        crate::light::upsert_fx(sp, 0, fx, "Licht", &Palette::grayscale());
        let want = p.sprites[0].layers[1].fx.clone();
        assert!(want.is_some());
        let web = import_web(&export_web(&p)).expect("Web-Format");
        assert_eq!(web.sprites[0].layers[1].fx, want, "Web-Format");
        let native = load_native(&save_native(&p)).expect("eigenes Format");
        assert_eq!(native.sprites[0].layers[1].fx, want, "eigenes Format");
    }

    #[test]
    fn kachelsaetze_ueberstehen_web_und_eigenes_format() {
        let mut p = sample();
        let free = p.sprites[0].free_color([1, 2, 3]);
        let mut ts = crate::tilemap::Tileset::new(2, 2);
        ts.tiles = vec![vec![1, 0, 0, free], vec![2, 2, 2, 2]];
        p.sprites[0].layers[0].tileset = Some(ts);
        let want = p.sprites[0].layers[0].tileset.clone();
        assert_eq!(load_native(&save_native(&p)).unwrap().sprites[0].layers[0].tileset, want, "eigenes Format");
        let back = import_web(&export_web(&p)).unwrap();
        let ts = back.sprites[0].layers[0].tileset.clone().unwrap();
        assert_eq!(ts.tiles[1], vec![2, 2, 2, 2], "Web-Format");
        let v = ts.tiles[0][3];
        assert_eq!(back.sprites[0].free[(v - FREE_BASE) as usize], [1, 2, 3], "freie Farbe als Hex");
    }

    #[test]
    fn masken_ueberstehen_web_und_eigenes_format() {
        let mut p = Project::default();
        let mut m = crate::mask::Mask::new(64, 64);
        m.hide.set(3, 4, 1);
        m.on = false;
        p.sprites[0].layers[0].mask = Some(m);
        let want = p.sprites[0].layers[0].mask.clone();
        assert_eq!(import_web(&export_web(&p)).unwrap().sprites[0].layers[0].mask, want, "Web-Format");
        assert_eq!(load_native(&save_native(&p)).unwrap().sprites[0].layers[0].mask, want, "eigenes Format");
    }
}
