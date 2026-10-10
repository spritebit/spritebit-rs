//! Aseprite-Dateien (`.aseprite` / `.ase`) lesen und schreiben.
//!
//! Das Format ist offen beschrieben (aseprite/docs, ase-file-specs.md):
//! ein Kopf von 128 Byte, dann je Frame ein Frame-Kopf und Blöcke
//! („Chunks“): Ebenen, Zellen (Pixel mit zlib gepackt), Palette, Tags,
//! Tilesets. Alle Zahlen little-endian.
//!
//! Was übernommen wird — in beide Richtungen:
//! * Ebenen mit Name, sichtbar, gesperrt (= nicht bearbeitbar), Deckkraft
//! * Frames mit Dauer, verknüpfte Zellen
//! * Palette (wird eine eigene Palette), Tags mit Richtung
//! * Tilemap-Ebenen samt Tileset
//!
//! Lesen: indiziert, RGBA und Graustufen. Ebenengruppen werden aufgelöst
//! (ihre Ebenen bleiben), Mischmodi und Zellen-Deckkraft fallen weg. Farben,
//! die nicht in der Palette stehen, werden freie Farben.
//!
//! Schreiben: indiziert (Nummer 0 = transparent, dann die Palette), mit
//! freien Farben RGBA. Masken werden eingerechnet — so, wie man es sieht.

use std::collections::HashMap;
use std::io::{Read, Write};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::image::{Image, Px, FREE_BASE};
use crate::palette::{Palette, Rgb, MAX_COLORS};
use crate::sprite::{Direction, Frame, Layer, Sprite, Tag, MAX_SIDE};
use crate::tilemap::Tileset;
use crate::transform;

const MAGIC: u16 = 0xA5E0;
const FRAME_MAGIC: u16 = 0xF1FA;
const CH_OLD_PAL: u16 = 0x0004;
const CH_OLD_PAL64: u16 = 0x0011;
const CH_LAYER: u16 = 0x2004;
const CH_CEL: u16 = 0x2005;
const CH_TAGS: u16 = 0x2018;
const CH_PALETTE: u16 = 0x2019;
const CH_TILESET: u16 = 0x2023;
/// Farbe der Tags, wenn die Datei keine sinnvolle hat.
const TAG_COLOR: Rgb = [0x6e, 0xa8, 0xfe];

#[derive(Debug, PartialEq)]
pub enum AseError {
    /// Keine Aseprite-Datei.
    NotAse,
    /// Größer als eine Zeichenfläche sein darf.
    TooBig { width: u32, height: u32 },
    /// Kaputt oder abgeschnitten.
    Corrupt(String),
}

/// Ein gelesener Sprite und seine Palette.
pub struct AseImport {
    pub sprite: Sprite,
    pub palette: Palette,
}

// ── Lesen ───────────────────────────────────────────────────────────

struct R<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], AseError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.b.len()).ok_or_else(|| AseError::Corrupt("zu kurz".into()))?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, AseError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, AseError> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn i16(&mut self) -> Result<i16, AseError> {
        Ok(self.u16()? as i16)
    }
    fn u32(&mut self) -> Result<u32, AseError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn string(&mut self) -> Result<String, AseError> {
        let n = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn rest(&mut self) -> &'a [u8] {
        let s = &self.b[self.pos..];
        self.pos = self.b.len();
        s
    }
}

fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>, AseError> {
    let mut out = Vec::new();
    ZlibDecoder::new(data).take(limit as u64 + 1).read_to_end(&mut out).map_err(|e| AseError::Corrupt(e.to_string()))?;
    if out.len() > limit {
        return Err(AseError::Corrupt("Bilddaten zu groß".into()));
    }
    Ok(out)
}

struct AseLayer {
    kind: u16,
    visible: bool,
    editable: bool,
    opacity: u8,
    name: String,
    tileset: u32,
}

struct AseCel {
    frame: usize,
    layer: usize,
    x: i32,
    y: i32,
    kind: u16,
    data: Vec<u8>,
}

struct AseTileset {
    id: u32,
    tw: u32,
    th: u32,
    /// Rohe Pixel (in der Farbtiefe der Datei), Kachel nach Kachel.
    pixels: Vec<u8>,
    count: u32,
}

/// Farbe → Pixelwert: aus der Palette, sonst eine freie Farbe.
struct ColorMap {
    by_rgb: HashMap<Rgb, Px>,
    /// Indiziert: Dateinummer → Pixelwert.
    by_index: Vec<Px>,
}

/// Aseprite-Datei lesen. `name` wird der Name des Sprites (und der Palette).
pub fn read(bytes: &[u8], name: &str) -> Result<AseImport, AseError> {
    if bytes.len() < 128 || u16::from_le_bytes([bytes[4], bytes[5]]) != MAGIC {
        return Err(AseError::NotAse);
    }
    let mut r = R { b: bytes, pos: 6 };
    let n_frames = r.u16()? as usize;
    let (w, h) = (r.u16()? as u32, r.u16()? as u32);
    let depth = r.u16()?;
    if !matches!(depth, 8 | 16 | 32) {
        return Err(AseError::Corrupt(format!("Farbtiefe {depth}")));
    }
    let flags = r.u32()?;
    let speed = r.u16()?;
    r.take(8)?;
    let transparent = r.u8()? as usize;
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(AseError::TooBig { width: w, height: h });
    }
    if n_frames == 0 {
        return Err(AseError::Corrupt("keine Frames".into()));
    }
    let bpp = (depth / 8) as usize;
    r.pos = 128;

    let mut layers: Vec<AseLayer> = Vec::new();
    let mut cels: Vec<AseCel> = Vec::new();
    let mut tags: Vec<Tag> = Vec::new();
    let mut palette: Vec<[u8; 4]> = Vec::new();
    let mut new_palette = false;
    let mut tilesets: Vec<AseTileset> = Vec::new();
    let mut durations: Vec<u32> = Vec::with_capacity(n_frames);

    for f in 0..n_frames {
        let start = r.pos;
        let frame_len = r.u32()? as usize;
        if r.u16()? != FRAME_MAGIC || frame_len < 16 {
            return Err(AseError::Corrupt("Frame-Kennung fehlt".into()));
        }
        let old_chunks = r.u16()? as u32;
        let dur = r.u16()? as u32;
        r.take(2)?;
        let new_chunks = r.u32()?;
        let n_chunks = if new_chunks == 0 { old_chunks } else { new_chunks };
        durations.push(if dur == 0 { speed as u32 } else { dur });
        let end = start.checked_add(frame_len).filter(|&e| e <= bytes.len()).ok_or_else(|| AseError::Corrupt("Frame abgeschnitten".into()))?;
        for _ in 0..n_chunks {
            if r.pos + 6 > end {
                break;
            }
            let size = r.u32()? as usize;
            let kind = r.u16()?;
            let body_len = size.checked_sub(6).ok_or_else(|| AseError::Corrupt("Block zu klein".into()))?;
            let body = r.take(body_len)?;
            let mut c = R { b: body, pos: 0 };
            match kind {
                CH_LAYER => {
                    let lf = c.u16()?;
                    let lk = c.u16()?;
                    let _child = c.u16()?;
                    c.take(6)?;
                    let opacity = c.u8()?;
                    c.take(3)?;
                    let name = c.string()?;
                    let tileset = if lk == 2 { c.u32()? } else { 0 };
                    layers.push(AseLayer {
                        kind: lk,
                        visible: lf & 1 != 0,
                        editable: lf & 2 != 0,
                        opacity: if flags & 1 != 0 { opacity } else { 255 },
                        name,
                        tileset,
                    });
                }
                CH_CEL => {
                    let layer = c.u16()? as usize;
                    let (x, y) = (c.i16()? as i32, c.i16()? as i32);
                    let _opacity = c.u8()?;
                    let ck = c.u16()?;
                    c.take(7)?;
                    cels.push(AseCel { frame: f, layer, x, y, kind: ck, data: c.rest().to_vec() });
                }
                CH_TAGS => {
                    let n = c.u16()?;
                    c.take(8)?;
                    for _ in 0..n {
                        let (from, to) = (c.u16()? as usize, c.u16()? as usize);
                        let dir = c.u8()?;
                        c.take(8)?;
                        let rgb = c.take(3)?;
                        c.take(1)?;
                        let name = c.string()?;
                        let direction = match dir {
                            1 => Direction::Reverse,
                            2 | 3 => Direction::PingPong,
                            _ => Direction::Forward,
                        };
                        let color = if rgb == [0, 0, 0] { TAG_COLOR } else { [rgb[0], rgb[1], rgb[2]] };
                        tags.push(Tag { name, from: from.min(n_frames - 1), to: to.min(n_frames - 1).max(from.min(n_frames - 1)), color, direction });
                    }
                }
                CH_PALETTE => {
                    new_palette = true;
                    let size = (c.u32()? as usize).min(256);
                    let (first, last) = (c.u32()? as usize, c.u32()? as usize);
                    c.take(8)?;
                    palette.resize(size.max(palette.len()), [0, 0, 0, 255]);
                    for i in first..=last.min(first + 1024) {
                        let ef = c.u16()?;
                        let rgba = c.take(4)?;
                        if ef & 1 != 0 {
                            c.string()?;
                        }
                        if i < palette.len() {
                            palette[i] = [rgba[0], rgba[1], rgba[2], rgba[3]];
                        }
                    }
                }
                CH_OLD_PAL | CH_OLD_PAL64 if !new_palette => {
                    let packets = c.u16()?;
                    let mut i = 0usize;
                    for _ in 0..packets {
                        i += c.u8()? as usize;
                        let n = match c.u8()? {
                            0 => 256,
                            k => k as usize,
                        };
                        for _ in 0..n {
                            let rgb = c.take(3)?;
                            let s = |v: u8| if kind == CH_OLD_PAL64 { ((v as u32 * 255) / 63) as u8 } else { v };
                            if i < 256 {
                                if palette.len() <= i {
                                    palette.resize(i + 1, [0, 0, 0, 255]);
                                }
                                palette[i] = [s(rgb[0]), s(rgb[1]), s(rgb[2]), 255];
                            }
                            i += 1;
                        }
                    }
                }
                CH_TILESET => {
                    let id = c.u32()?;
                    let tf = c.u32()?;
                    let count = c.u32()?;
                    let (tw, th) = (c.u16()? as u32, c.u16()? as u32);
                    let _base = c.i16()?;
                    c.take(14)?;
                    let _name = c.string()?;
                    if tf & 1 != 0 {
                        c.take(8)?;
                    }
                    let pixels = if tf & 2 != 0 {
                        let len = c.u32()? as usize;
                        let limit = (tw as usize * th as usize).saturating_mul(count as usize).saturating_mul(bpp);
                        inflate(c.take(len)?, limit.min(512 << 20))?
                    } else {
                        Vec::new()
                    };
                    if tw > 0 && th > 0 {
                        tilesets.push(AseTileset { id, tw, th, pixels, count });
                    }
                }
                _ => {}
            }
        }
        r.pos = end;
    }

    // Palette: Platz 0 (bzw. die transparente Nummer) ist bei uns
    // „transparent“, die übrigen werden Farbe 1, 2, …
    let mut pal_colors: Vec<Rgb> = Vec::new();
    let mut by_index = vec![0 as Px; palette.len().max(256)];
    for (i, c) in palette.iter().enumerate() {
        // Indiziert: die transparente Nummer. Sonst: durchsichtige Einträge.
        if (depth == 8 && i == transparent) || (depth != 8 && c[3] == 0) || pal_colors.len() >= MAX_COLORS {
            continue;
        }
        pal_colors.push([c[0], c[1], c[2]]);
        by_index[i] = pal_colors.len() as Px;
    }
    let by_rgb: HashMap<Rgb, Px> = pal_colors.iter().enumerate().rev().map(|(i, &c)| (c, (i + 1) as Px)).collect();
    let mut cmap = ColorMap { by_rgb, by_index };

    let mut sp = Sprite::new(name, w, h).map_err(|_| AseError::TooBig { width: w, height: h })?;
    sp.palette = name.to_string();
    // Gruppen fallen weg; die anderen Ebenen behalten ihre Reihenfolge.
    let kept: Vec<usize> = layers.iter().enumerate().filter(|(_, l)| l.kind != 1).map(|(i, _)| i).collect();
    let our_of: HashMap<usize, usize> = kept.iter().enumerate().map(|(o, &a)| (a, o)).collect();
    if kept.is_empty() {
        return Err(AseError::Corrupt("keine Ebenen".into()));
    }
    sp.layers = kept
        .iter()
        .map(|&a| {
            let l = &layers[a];
            Layer {
                visible: l.visible,
                locked: !l.editable,
                opacity: l.opacity as f32 / 255.0,
                ..Layer::new(if l.name.is_empty() { "Ebene" } else { &l.name })
            }
        })
        .collect();
    sp.images.clear();
    sp.frames = (0..n_frames)
        .map(|_| Frame {
            cels: (0..kept.len())
                .map(|_| {
                    sp.images.push(Image::new(w, h));
                    sp.images.len() - 1
                })
                .collect(),
            duration_ms: 0,
        })
        .collect();

    // Zellen eintragen (in Frame-Reihenfolge, damit Verknüpfungen ihr Ziel finden).
    let tiles_of = |tid: u32| tilesets.iter().find(|t| t.id == tid);
    for cel in &cels {
        let Some(&l) = our_of.get(&cel.layer) else { continue };
        let f = cel.frame;
        match cel.kind {
            1 => {
                let mut c = R { b: &cel.data, pos: 0 };
                let link = c.u16()? as usize;
                if link < f {
                    sp.frames[f].cels[l] = sp.frames[link].cels[l];
                }
            }
            0 | 2 => {
                let mut c = R { b: &cel.data, pos: 0 };
                let (cw, ch) = (c.u16()? as usize, c.u16()? as usize);
                let need = cw * ch * bpp;
                let px = if cel.kind == 0 { c.take(need)?.to_vec() } else { inflate(c.rest(), need)? };
                if px.len() < need {
                    return Err(AseError::Corrupt("Zelle zu kurz".into()));
                }
                let id = sp.frames[f].cels[l];
                for yy in 0..ch {
                    for xx in 0..cw {
                        let (x, y) = (cel.x + xx as i32, cel.y + yy as i32);
                        if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h {
                            continue;
                        }
                        let o = (yy * cw + xx) * bpp;
                        let v = pixel(&px[o..o + bpp], depth, transparent, &mut cmap, &mut sp.free);
                        if v != 0 {
                            sp.images[id].set(x as u32, y as u32, v);
                        }
                    }
                }
            }
            3 => {
                let Some(ts) = tiles_of(layers[cel.layer].tileset) else { continue };
                let mut c = R { b: &cel.data, pos: 0 };
                let (cols, rows) = (c.u16()? as usize, c.u16()? as usize);
                let bits = c.u16()?;
                let id_mask = c.u32()?;
                c.take(22)?;
                let tb = (bits / 8).max(1) as usize;
                let raw = inflate(c.rest(), cols * rows * tb)?;
                let id = sp.frames[f].cels[l];
                let tile_px = (ts.tw * ts.th) as usize;
                for ty in 0..rows {
                    for tx in 0..cols {
                        let o = (ty * cols + tx) * tb;
                        let Some(t) = raw.get(o..o + tb) else { continue };
                        let mut v = 0u32;
                        for (k, b) in t.iter().enumerate() {
                            v |= (*b as u32) << (8 * k);
                        }
                        let k = (v & id_mask) as usize;
                        if k == 0 || k >= ts.count as usize {
                            continue;
                        }
                        for yy in 0..ts.th as usize {
                            for xx in 0..ts.tw as usize {
                                let o = ((k * ts.th as usize + yy) * ts.tw as usize + xx) * bpp;
                                let Some(src) = ts.pixels.get(o..o + bpp) else { continue };
                                let (x, y) = (cel.x + (tx * ts.tw as usize + xx) as i32, cel.y + (ty * ts.th as usize + yy) as i32);
                                if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h {
                                    continue;
                                }
                                let v = pixel(src, depth, transparent, &mut cmap, &mut sp.free);
                                if v != 0 {
                                    sp.images[id].set(x as u32, y as u32, v);
                                }
                            }
                        }
                        let _ = tile_px;
                    }
                }
            }
            _ => {}
        }
    }

    // Tilemap-Ebenen: der Kachelsatz ergibt sich aus dem Inhalt.
    for (o, &a) in kept.iter().enumerate() {
        if layers[a].kind != 2 {
            continue;
        }
        if let Some(ts) = tiles_of(layers[a].tileset) {
            let ids: Vec<usize> = sp.frames.iter().map(|f| f.cels[o]).collect();
            sp.layers[o].tileset = Some(Tileset::build(ts.tw, ts.th, ids.iter().map(|&i| &sp.images[i])));
        }
    }

    // Tempo: sind alle Frames gleich lang, gilt es als fps, sonst je Frame.
    let first = durations[0].max(1);
    if durations.iter().all(|&d| d == durations[0]) {
        sp.fps = ((1000 + first / 2) / first).clamp(1, 60);
    } else {
        sp.fps = ((1000 + first / 2) / first).clamp(1, 60);
        for (fr, &d) in sp.frames.iter_mut().zip(&durations) {
            fr.duration_ms = d;
        }
    }
    sp.tags = tags;
    sp.layer = sp.layers.len() - 1;
    sp.guides.bottom = h;
    sp.collect_garbage();
    Ok(AseImport { sprite: sp, palette: Palette::new(name, pal_colors) })
}

/// Ein Pixel der Datei → unser Pixelwert (0 = transparent).
fn pixel(p: &[u8], depth: u16, transparent: usize, cmap: &mut ColorMap, free: &mut Vec<Rgb>) -> Px {
    let rgb = match depth {
        8 => {
            let i = p[0] as usize;
            if i == transparent {
                return 0;
            }
            return cmap.by_index.get(i).copied().unwrap_or(0);
        }
        16 => {
            if p[1] == 0 {
                return 0;
            }
            [p[0], p[0], p[0]]
        }
        _ => {
            if p[3] == 0 {
                return 0;
            }
            [p[0], p[1], p[2]]
        }
    };
    if let Some(&v) = cmap.by_rgb.get(&rgb) {
        return v;
    }
    let i = match free.iter().position(|&c| c == rgb) {
        Some(i) => i,
        None => {
            free.push(rgb);
            free.len() - 1
        }
    };
    let v = FREE_BASE + i as Px;
    cmap.by_rgb.insert(rgb, v);
    v
}

// ── Schreiben ───────────────────────────────────────────────────────

struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn zeros(&mut self, n: usize) {
        self.0.extend(std::iter::repeat_n(0, n));
    }
    fn string(&mut self, s: &str) {
        let b = s.as_bytes();
        let n = b.len().min(u16::MAX as usize);
        self.u16(n as u16);
        self.0.extend_from_slice(&b[..n]);
    }
}

fn chunk(out: &mut Vec<Vec<u8>>, kind: u16, body: W) {
    let mut c = W(Vec::with_capacity(body.0.len() + 6));
    c.u32(body.0.len() as u32 + 6);
    c.u16(kind);
    c.0.extend_from_slice(&body.0);
    out.push(c.0);
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data).expect("in den Speicher schreiben");
    enc.finish().expect("in den Speicher schreiben")
}

/// Was man von Ebene `l` in Frame `f` sieht: die Maske eingerechnet.
fn shown(sp: &Sprite, f: usize, l: usize) -> Image {
    let mut img = sp.cel(f, l).clone();
    if let Some(m) = sp.layers[l].mask.as_ref().filter(|m| m.on) {
        let hidden: Vec<(u32, u32)> = img.pixels().filter(|&(x, y, _)| m.hide.get(x, y) != 0).map(|(x, y, _)| (x, y)).collect();
        for (x, y) in hidden {
            img.set(x, y, 0);
        }
    }
    img
}

/// Sprite als Aseprite-Datei.
pub fn write(sp: &Sprite, pal: &Palette) -> Vec<u8> {
    let indexed = sp.free.is_empty();
    let depth: u16 = if indexed { 8 } else { 32 };
    let n_frames = sp.frames.len().min(u16::MAX as usize);
    let fps_ms = (1000 / sp.fps.max(1)).max(1);
    // Pixelwert → Bytes in der Farbtiefe der Datei.
    let rgba = |v: Px| -> [u8; 4] {
        let c = if v >= FREE_BASE { sp.free.get((v - FREE_BASE) as usize).copied() } else { pal.get(v) };
        c.map_or([0, 0, 0, 0], |c| [c[0], c[1], c[2], 255])
    };
    let encode = |img: &Image, x0: u32, y0: u32, w: u32, h: u32| -> Vec<u8> {
        let mut out = Vec::with_capacity((w * h) as usize * if indexed { 1 } else { 4 });
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let v = img.get(x, y);
                if indexed {
                    out.push(if pal.get(v).is_some() { v as u8 } else { 0 });
                } else {
                    out.extend_from_slice(&rgba(v));
                }
            }
        }
        out
    };

    // Tilemap-Ebenen, deren Inhalt ganz aus Kacheln besteht — sonst als
    // gewöhnliche Ebene.
    let mut tilemaps: Vec<(usize, &Tileset)> = Vec::new();
    for (l, layer) in sp.layers.iter().enumerate() {
        if let Some(ts) = &layer.tileset {
            // Auch der Rand, der in keine ganze Kachel passt, muss leer sein —
            // er käme in einer Tilemap nicht mit.
            let (cols, rows) = ts.map_size(sp.width, sp.height);
            let (gw, gh) = (cols * ts.tw, rows * ts.th);
            let ok = (0..n_frames).all(|f| {
                let img = shown(sp, f, l);
                ts.map_of(&img).iter().flatten().all(|&k| k >= 0) && img.pixels().all(|(x, y, _)| x < gw && y < gh)
            });
            if ok && ts.tw <= u16::MAX as u32 && ts.th <= u16::MAX as u32 {
                tilemaps.push((l, ts));
            }
        }
    }
    let tileset_of = |l: usize| tilemaps.iter().position(|(tl, _)| *tl == l);

    let mut frames_out: Vec<Vec<u8>> = Vec::with_capacity(n_frames);
    for f in 0..n_frames {
        let mut chunks: Vec<Vec<u8>> = Vec::new();
        if f == 0 {
            // Palette: 0 = transparent, dann die Farben.
            let n = pal.len() + 1;
            let mut b = W(Vec::new());
            b.u32(n as u32);
            b.u32(0);
            b.u32(n as u32 - 1);
            b.zeros(8);
            b.u16(0);
            b.0.extend_from_slice(&[0, 0, 0, 0]);
            for c in &pal.colors {
                b.u16(0);
                b.0.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
            chunk(&mut chunks, CH_PALETTE, b);
            for (l, layer) in sp.layers.iter().enumerate() {
                let mut b = W(Vec::new());
                b.u16(u16::from(layer.visible) | if layer.locked { 0 } else { 2 });
                let tm = tileset_of(l);
                b.u16(if tm.is_some() { 2 } else { 0 });
                b.u16(0);
                b.zeros(4);
                b.u16(0);
                b.u8((layer.opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
                b.zeros(3);
                b.string(&layer.name);
                if let Some(k) = tm {
                    b.u32(k as u32);
                }
                chunk(&mut chunks, CH_LAYER, b);
            }
            for (k, (l, ts)) in tilemaps.iter().enumerate() {
                // Kachel 0 ist leer, dann die Kacheln untereinander.
                let n = ts.tiles.len() + 1;
                let tpx = (ts.tw * ts.th) as usize;
                let mut img = Vec::with_capacity(n * tpx * if indexed { 1 } else { 4 });
                for t in std::iter::once(&vec![0; tpx]).chain(ts.tiles.iter()) {
                    for &v in t.iter().take(tpx) {
                        if indexed {
                            img.push(if pal.get(v).is_some() { v as u8 } else { 0 });
                        } else {
                            img.extend_from_slice(&rgba(v));
                        }
                    }
                }
                let z = deflate(&img);
                let mut b = W(Vec::new());
                b.u32(k as u32);
                b.u32(2 | 4);
                b.u32(n as u32);
                b.u16(ts.tw as u16);
                b.u16(ts.th as u16);
                b.i16(1);
                b.zeros(14);
                b.string(&sp.layers[*l].name);
                b.u32(z.len() as u32);
                b.0.extend_from_slice(&z);
                chunk(&mut chunks, CH_TILESET, b);
            }
            if !sp.tags.is_empty() {
                let mut b = W(Vec::new());
                b.u16(sp.tags.len().min(u16::MAX as usize) as u16);
                b.zeros(8);
                for t in &sp.tags {
                    b.u16(t.from.min(n_frames - 1) as u16);
                    b.u16(t.to.min(n_frames - 1) as u16);
                    b.u8(match t.direction {
                        Direction::Forward => 0,
                        Direction::Reverse => 1,
                        Direction::PingPong => 2,
                    });
                    b.u16(0);
                    b.zeros(6);
                    b.0.extend_from_slice(&t.color);
                    b.u8(0);
                    b.string(&t.name);
                }
                chunk(&mut chunks, CH_TAGS, b);
            }
        }
        for l in 0..sp.layers.len() {
            let id = sp.frames[f].cels[l];
            // Verknüpft: auf den ersten Frame zeigen, der dasselbe Bild hat.
            if let Some(first) = (0..f).find(|&g| sp.frames[g].cels[l] == id) {
                let mut b = W(Vec::new());
                b.u16(l as u16);
                b.i16(0);
                b.i16(0);
                b.u8(255);
                b.u16(1);
                b.zeros(7);
                b.u16(first as u16);
                chunk(&mut chunks, CH_CEL, b);
                continue;
            }
            let img = shown(sp, f, l);
            if let Some(k) = tileset_of(l) {
                let ts = tilemaps[k].1;
                let map = ts.map_of(&img);
                if map.iter().flatten().all(|&t| t == 0) {
                    continue;
                }
                let (cols, rows) = (map.first().map_or(0, |r| r.len()), map.len());
                let mut raw = Vec::with_capacity(cols * rows * 4);
                for row in &map {
                    for &t in row {
                        raw.extend_from_slice(&(t.max(0) as u32).to_le_bytes());
                    }
                }
                let mut b = W(Vec::new());
                b.u16(l as u16);
                b.i16(0);
                b.i16(0);
                b.u8(255);
                b.u16(3);
                b.zeros(7);
                b.u16(cols as u16);
                b.u16(rows as u16);
                b.u16(32);
                b.u32(0x1fff_ffff);
                b.u32(0x8000_0000);
                b.u32(0x4000_0000);
                b.u32(0x2000_0000);
                b.zeros(10);
                b.0.extend_from_slice(&deflate(&raw));
                chunk(&mut chunks, CH_CEL, b);
                continue;
            }
            let Some((x, y, w, h)) = transform::bounds(&img) else { continue };
            let mut b = W(Vec::new());
            b.u16(l as u16);
            b.i16(x as i16);
            b.i16(y as i16);
            b.u8(255);
            b.u16(2);
            b.zeros(7);
            b.u16(w as u16);
            b.u16(h as u16);
            b.0.extend_from_slice(&deflate(&encode(&img, x, y, w, h)));
            chunk(&mut chunks, CH_CEL, b);
        }
        let body: usize = chunks.iter().map(Vec::len).sum();
        let dur = if sp.frames[f].duration_ms > 0 { sp.frames[f].duration_ms } else { fps_ms };
        let mut fr = W(Vec::with_capacity(body + 16));
        fr.u32(body as u32 + 16);
        fr.u16(FRAME_MAGIC);
        fr.u16(chunks.len().min(0xFFFF) as u16);
        fr.u16(dur.min(u16::MAX as u32) as u16);
        fr.zeros(2);
        fr.u32(chunks.len() as u32);
        for c in chunks {
            fr.0.extend_from_slice(&c);
        }
        frames_out.push(fr.0);
    }

    let total: usize = 128 + frames_out.iter().map(Vec::len).sum::<usize>();
    let mut h = W(Vec::with_capacity(total));
    h.u32(total as u32);
    h.u16(MAGIC);
    h.u16(n_frames as u16);
    h.u16(sp.width as u16);
    h.u16(sp.height as u16);
    h.u16(depth);
    h.u32(1); // Deckkraft der Ebenen gilt
    h.u16(fps_ms.min(u16::MAX as u32) as u16);
    h.zeros(8);
    h.u8(0); // transparente Nummer
    h.zeros(3);
    let n_colors = pal.len() + 1;
    h.u16(if n_colors >= 256 { 0 } else { n_colors as u16 });
    h.u8(1);
    h.u8(1);
    h.i16(0);
    h.i16(0);
    h.u16(16);
    h.u16(16);
    h.zeros(84);
    debug_assert_eq!(h.0.len(), 128);
    for f in frames_out {
        h.0.extend_from_slice(&f);
    }
    h.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (Sprite, Palette) {
        let pal = Palette::new("test", vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]]);
        let mut sp = Sprite::new("held", 12, 8).unwrap();
        sp.palette = "test".into();
        sp.cel_mut(0, 0).set(1, 1, 1);
        sp.cel_mut(0, 0).set(10, 6, 3);
        sp.add_layer(1, "Augen");
        sp.layers[1].opacity = 0.5;
        sp.layers[1].locked = true;
        sp.cel_mut(0, 1).set(4, 4, 2);
        sp.add_frame(0, true); // eigene Kopie
        sp.link(0, 1, 1); // Ebene „Augen“ in Frame 1 = Frame 0
        sp.cel_mut(1, 0).set(5, 5, 2);
        sp.frames[1].duration_ms = 250;
        sp.fps = 10;
        sp.tags.push(Tag { name: "Laufen".into(), from: 0, to: 1, color: [10, 20, 30], direction: Direction::PingPong });
        (sp, pal)
    }

    #[test]
    fn hin_und_zurueck_indiziert() {
        let (sp, pal) = sample();
        let bytes = write(&sp, &pal);
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), MAGIC);
        assert_eq!(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize, bytes.len(), "Dateigröße im Kopf");
        let back = read(&bytes, "held").unwrap();
        let b = &back.sprite;
        assert_eq!(back.palette.colors, pal.colors);
        assert_eq!((b.width, b.height, b.frames.len(), b.layers.len()), (12, 8, 2, 2));
        assert_eq!(b.layers[1].name, "Augen");
        assert!(b.layers[1].locked && !b.layers[0].locked);
        assert!((b.layers[1].opacity - 0.5).abs() < 0.01);
        for f in 0..2 {
            for l in 0..2 {
                for y in 0..8 {
                    for x in 0..12 {
                        assert_eq!(b.cel(f, l).get(x, y), sp.cel(f, l).get(x, y), "Frame {f} Ebene {l} ({x}, {y})");
                    }
                }
            }
        }
        assert!(b.is_linked(1, 1), "verknüpfte Zelle bleibt verknüpft");
        assert!(!b.is_linked(1, 0));
        assert_eq!(b.frames[1].duration_ms, 250);
        assert_eq!(b.fps, 10);
        assert_eq!(b.tags, sp.tags);
    }

    #[test]
    fn freie_farben_werden_rgba() {
        let (mut sp, pal) = sample();
        let v = sp.free_color([1, 2, 3]);
        sp.cel_mut(0, 0).set(0, 7, v);
        let bytes = write(&sp, &pal);
        assert_eq!(u16::from_le_bytes([bytes[12], bytes[13]]), 32, "RGBA");
        let back = read(&bytes, "held").unwrap();
        let b = &back.sprite;
        let got = b.cel(0, 0).get(0, 7);
        assert!(got >= FREE_BASE && b.free[(got - FREE_BASE) as usize] == [1, 2, 3]);
        assert_eq!(b.cel(0, 0).get(1, 1), 1, "Palettenfarbe bleibt Nummer 1");
    }

    #[test]
    fn maske_wird_eingerechnet() {
        let (mut sp, pal) = sample();
        let mut m = crate::mask::Mask::new(12, 8);
        m.hide.set(1, 1, 1);
        sp.layers[0].mask = Some(m);
        let back = read(&write(&sp, &pal), "held").unwrap();
        assert_eq!(back.sprite.cel(0, 0).get(1, 1), 0, "ausgeblendet = weg");
        assert_eq!(back.sprite.cel(0, 0).get(10, 6), 3);
    }

    #[test]
    fn tilemap_ebene_mit_tileset() {
        let pal = Palette::new("t", vec![[255, 255, 255], [0, 0, 0]]);
        let mut sp = Sprite::new("level", 16, 8).unwrap();
        for y in 0..8 {
            for x in 0..8 {
                sp.cel_mut(0, 0).set(x, y, if (x + y) % 2 == 0 { 1 } else { 2 });
                sp.cel_mut(0, 0).set(x + 8, y, if (x + y) % 2 == 0 { 1 } else { 2 });
            }
        }
        let ts = Tileset::build(8, 8, std::iter::once(sp.cel(0, 0)));
        assert_eq!(ts.tiles.len(), 1);
        sp.layers[0].tileset = Some(ts);
        let bytes = write(&sp, &pal);
        let back = read(&bytes, "level").unwrap();
        let b = &back.sprite;
        let ts = b.layers[0].tileset.as_ref().expect("wieder eine Tilemap-Ebene");
        assert_eq!((ts.tw, ts.th, ts.tiles.len()), (8, 8, 1));
        assert_eq!(b.cel(0, 0).get(9, 0), 2);
        assert_eq!(b.cel(0, 0).get(8, 0), 1);
    }

    #[test]
    fn unsinn_ist_kein_absturz() {
        assert_eq!(read(b"kurz", "x").err(), Some(AseError::NotAse));
        let (sp, pal) = sample();
        let bytes = write(&sp, &pal);
        for cut in [130, 200, bytes.len() / 2, bytes.len() - 3] {
            let _ = read(&bytes[..cut], "x");
        }
        let mut bad = bytes.clone();
        bad[12] = 7; // Farbtiefe
        assert!(read(&bad, "x").is_err());
    }
}
