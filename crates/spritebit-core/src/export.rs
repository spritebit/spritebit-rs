//! Export: PNG, GIF und Spritesheet mit JSON-Atlas.
//!
//! Alles zeigt, was man sieht — die sichtbaren Ebenen übereinander
//! ([`crate::composite`]) — und lässt sich ganzzahlig vergrößern, mit harten
//! Pixelkanten. Zu große Ergebnisse (über [`MAX_OUTPUT`] Pixel Kante) werden
//! abgelehnt statt den Speicher zu sprengen.

use std::collections::HashMap;
use std::fmt;

use serde_json::json;

use crate::composite::{render_rgba, Rect};
use crate::palette::Palette;
use crate::project::Project;
use crate::sprite::{Direction, Sprite, Tag};

/// Größte Kantenlänge eines exportierten Bildes.
pub const MAX_OUTPUT: u32 = 16_384;

#[derive(Debug, PartialEq)]
pub enum ExportError {
    TooBig { width: u64, height: u64 },
    /// GIF fasst 255 Farben plus Transparent.
    TooManyColors(usize),
    Encode(String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::TooBig { width, height } => {
                write!(f, "Das Ergebnis wäre {width} × {height} Pixel groß — höchstens {MAX_OUTPUT} je Seite. Kleinere Vergrößerung wählen.")
            }
            ExportError::TooManyColors(n) => write!(f, "{n} Farben — ein GIF fasst höchstens 255."),
            ExportError::Encode(e) => write!(f, "Speichern fehlgeschlagen: {e}"),
        }
    }
}

impl std::error::Error for ExportError {}

fn check(w: u64, h: u64) -> Result<(u32, u32), ExportError> {
    if w == 0 || h == 0 || w > MAX_OUTPUT as u64 || h > MAX_OUTPUT as u64 {
        return Err(ExportError::TooBig { width: w, height: h });
    }
    Ok((w as u32, h as u32))
}

/// Frame `f` als RGBA in Sprite-Größe.
pub fn frame_rgba(sp: &Sprite, pal: &Palette, f: usize) -> Vec<u8> {
    render_rgba(sp, pal, f, Rect { x: 0, y: 0, w: sp.width, h: sp.height })
}

/// RGBA ganzzahlig vergrößern (nächster Nachbar).
fn upscale(rgba: &[u8], w: u32, h: u32, scale: u32) -> Vec<u8> {
    if scale == 1 {
        return rgba.to_vec();
    }
    let (ow, s) = ((w * scale) as usize, scale as usize);
    let mut out = vec![0u8; ow * (h * scale) as usize * 4];
    for y in 0..h as usize {
        for x in 0..w as usize {
            let src = &rgba[(y * w as usize + x) * 4..][..4];
            for dy in 0..s {
                let row = (y * s + dy) * ow;
                for dx in 0..s {
                    out[(row + x * s + dx) * 4..][..4].copy_from_slice(src);
                }
            }
        }
    }
    out
}

fn encode_png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, ExportError> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().map_err(|e| ExportError::Encode(e.to_string()))?;
        wr.write_image_data(rgba).map_err(|e| ExportError::Encode(e.to_string()))?;
    }
    Ok(out)
}

/// Ein Frame als PNG.
pub fn png(sp: &Sprite, pal: &Palette, f: usize, scale: u32) -> Result<Vec<u8>, ExportError> {
    let scale = scale.max(1);
    let (w, h) = check(sp.width as u64 * scale as u64, sp.height as u64 * scale as u64)?;
    encode_png(&upscale(&frame_rgba(sp, pal, f), sp.width, sp.height, scale), w, h)
}

/// Reihenfolge der Frames eines Tags beim Abspielen (eine Runde).
pub fn tag_frames(t: &Tag) -> Vec<usize> {
    let fwd: Vec<usize> = (t.from..=t.to).collect();
    match t.direction {
        Direction::Forward => fwd,
        Direction::Reverse => fwd.into_iter().rev().collect(),
        Direction::PingPong if fwd.len() > 2 => {
            let back: Vec<usize> = fwd[1..fwd.len() - 1].iter().rev().copied().collect();
            fwd.into_iter().chain(back).collect()
        }
        Direction::PingPong => fwd,
    }
}

/// GIF aus den Frames `frames` (in dieser Reihenfolge), endlos wiederholt.
/// Transparenz bleibt erhalten (Farbe 0 der GIF-Palette).
pub fn gif(sp: &Sprite, pal: &Palette, frames: &[usize], scale: u32) -> Result<Vec<u8>, ExportError> {
    let scale = scale.max(1);
    let (w, h) = check(sp.width as u64 * scale as u64, sp.height as u64 * scale as u64)?;
    if w > u16::MAX as u32 || h > u16::MAX as u32 {
        return Err(ExportError::TooBig { width: w as u64, height: h as u64 });
    }
    // Farben aller Frames einsammeln; Index 0 ist transparent.
    let rendered: Vec<Vec<u8>> = frames.iter().map(|&f| frame_rgba(sp, pal, f)).collect();
    let mut index: HashMap<[u8; 3], u8> = HashMap::new();
    let mut colors: Vec<u8> = vec![0, 0, 0];
    for rgba in &rendered {
        for px in rgba.as_chunks::<4>().0 {
            if px[3] == 0 {
                continue;
            }
            let c = [px[0], px[1], px[2]];
            if !index.contains_key(&c) {
                if index.len() >= 255 {
                    return Err(ExportError::TooManyColors(index.len() + 1));
                }
                index.insert(c, (index.len() + 1) as u8);
                colors.extend_from_slice(&c);
            }
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = gif::Encoder::new(&mut out, w as u16, h as u16, &colors).map_err(|e| ExportError::Encode(e.to_string()))?;
        enc.set_repeat(gif::Repeat::Infinite).map_err(|e| ExportError::Encode(e.to_string()))?;
        for (rgba, &f) in rendered.iter().zip(frames) {
            let big = upscale(rgba, sp.width, sp.height, scale);
            let idx: Vec<u8> = big.as_chunks::<4>().0.iter().map(|p| if p[3] == 0 { 0 } else { index[&[p[0], p[1], p[2]]] }).collect();
            let mut frame = gif::Frame::from_indexed_pixels(w as u16, h as u16, idx, Some(0));
            // GIF zählt in Hundertstelsekunden.
            frame.delay = (sp.frame_duration(f) as f64 / 10.0).round().max(1.0) as u16;
            frame.dispose = gif::DisposalMethod::Background;
            enc.write_frame(&frame).map_err(|e| ExportError::Encode(e.to_string()))?;
        }
    }
    Ok(out)
}

/// Spritesheet: eine Zeile je Sprite, seine Frames nebeneinander, jede
/// Zelle so groß wie der größte Sprite (Sprites mittig darin). Dazu der
/// Atlas als JSON — dasselbe Format wie in der Web-Version.
pub fn sheet(p: &Project, scale: u32, image_name: &str) -> Result<(Vec<u8>, String), ExportError> {
    let scale = scale.max(1);
    let cw = p.sprites.iter().map(|s| s.width).max().unwrap_or(1);
    let ch = p.sprites.iter().map(|s| s.height).max().unwrap_or(1);
    let cols = p.sprites.iter().map(|s| s.frames.len()).max().unwrap_or(1) as u32;
    let rows = p.sprites.len() as u32;
    let (w, h) = check(cols as u64 * cw as u64 * scale as u64, rows as u64 * ch as u64 * scale as u64)?;
    let mut out = vec![0u8; (w * h * 4) as usize];
    let mut frames = Vec::new();
    for (row, sp) in p.sprites.iter().enumerate() {
        let pal = p.palette(&sp.palette);
        let ox = (cw - sp.width) / 2;
        let oy = (ch - sp.height) / 2;
        for f in 0..sp.frames.len() {
            let big = upscale(&frame_rgba(sp, &pal, f), sp.width, sp.height, scale);
            let (x0, y0) = ((f as u32 * cw + ox) * scale, (row as u32 * ch + oy) * scale);
            let bw = (sp.width * scale) as usize;
            for y in 0..(sp.height * scale) as usize {
                let dst = ((y0 as usize + y) * w as usize + x0 as usize) * 4;
                out[dst..dst + bw * 4].copy_from_slice(&big[y * bw * 4..(y + 1) * bw * 4]);
            }
            frames.push(json!({
                "name": sp.name, "frame": f, "duration": sp.frame_duration(f),
                "x": x0, "y": y0, "w": sp.width * scale, "h": sp.height * scale, "palette": sp.palette,
            }));
        }
    }
    let atlas = json!({
        "image": image_name,
        "scale": scale,
        "cell": { "w": cw * scale, "h": ch * scale },
        "columns": cols,
        "frames": frames,
        "tags": p.sprites.iter().filter(|s| !s.tags.is_empty()).map(|s| json!({
            "sprite": s.name,
            "tags": s.tags.iter().map(|t| json!({
                "name": t.name, "from": t.from, "to": t.to,
                "direction": match t.direction { Direction::Forward => "forward", Direction::Reverse => "reverse", Direction::PingPong => "pingpong" },
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    let json = serde_json::to_string_pretty(&atlas).expect("JSON aus eigenen Daten");
    Ok((encode_png(&out, w, h)?, json))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite() -> Sprite {
        let mut sp = Sprite::new("Held", 4, 3).unwrap();
        sp.active().set(1, 1, 5); // schwarz
        sp.add_frame(0, true);
        sp.cel_mut(1, 0).set(2, 2, 1); // weiß
        sp
    }

    #[test]
    fn png_hat_die_richtige_groesse_und_signatur() {
        let pal = Palette::grayscale();
        let bytes = png(&sprite(), &pal, 0, 4).unwrap();
        assert_eq!(&bytes[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        // IHDR: Breite und Höhe ab Byte 16
        assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), 16);
        assert_eq!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()), 12);
    }

    #[test]
    fn vergroessern_ist_hart() {
        let rgba = vec![1, 2, 3, 255, 9, 9, 9, 0];
        let big = upscale(&rgba, 2, 1, 2);
        assert_eq!(big.len(), 4 * 2 * 4);
        assert_eq!(&big[0..4], &[1, 2, 3, 255]);
        assert_eq!(&big[4..8], &[1, 2, 3, 255]);
        assert_eq!(&big[8..12], &[9, 9, 9, 0]);
    }

    #[test]
    fn zu_gross_wird_abgelehnt() {
        let sp = Sprite::new("g", 8192, 8192).unwrap();
        assert!(matches!(png(&sp, &Palette::grayscale(), 0, 4), Err(ExportError::TooBig { .. })));
    }

    #[test]
    fn gif_beginnt_mit_der_signatur_und_hat_alle_frames() {
        let pal = Palette::grayscale();
        let bytes = gif(&sprite(), &pal, &[0, 1], 2).unwrap();
        assert_eq!(&bytes[..6], b"GIF89a");
        // Jeder Frame beginnt mit einem Bildblock (0x2C).
        let blocks = bytes.windows(1).filter(|w| w[0] == 0x2c).count();
        assert!(blocks >= 2);
        assert_eq!(*bytes.last().unwrap(), 0x3b, "GIF-Ende");
    }

    #[test]
    fn png_laesst_sich_wieder_lesen() {
        let pal = Palette::grayscale();
        let bytes = png(&sprite(), &pal, 0, 2).unwrap();
        let dec = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut r = dec.read_info().unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let info = r.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (8, 6));
        let at = |x: usize, y: usize| &buf[(y * 8 + x) * 4..][..4];
        assert_eq!(at(2, 2), &[0, 0, 0, 255], "Pixel (1,1) doppelt so groß");
        assert_eq!(at(3, 3), &[0, 0, 0, 255]);
        assert_eq!(at(0, 0)[3], 0, "transparent bleibt transparent");
    }

    #[test]
    fn gif_laesst_sich_wieder_lesen() {
        let pal = Palette::grayscale();
        let mut sp = sprite();
        sp.frames[1].duration_ms = 300;
        let bytes = gif(&sp, &pal, &[0, 1], 1).unwrap();
        let mut opts = gif::DecodeOptions::new();
        opts.set_color_output(gif::ColorOutput::RGBA);
        let mut dec = opts.read_info(std::io::Cursor::new(bytes)).unwrap();
        let mut delays = Vec::new();
        while let Some(f) = dec.read_next_frame().unwrap() {
            assert_eq!((f.width, f.height), (4, 3));
            delays.push(f.delay);
            assert_eq!(f.buffer[(1 * 4 + 1) * 4 + 3], 255, "schwarzer Pixel sichtbar");
            assert_eq!(f.buffer[3], 0, "Ecke transparent");
        }
        assert_eq!(delays, vec![13, 30], "125 ms → 13, 300 ms → 30 Hundertstel");
    }

    #[test]
    fn tag_reihenfolge() {
        let t = |d| Tag { name: "t".into(), from: 2, to: 5, color: [0, 0, 0], direction: d };
        assert_eq!(tag_frames(&t(Direction::Forward)), vec![2, 3, 4, 5]);
        assert_eq!(tag_frames(&t(Direction::Reverse)), vec![5, 4, 3, 2]);
        assert_eq!(tag_frames(&t(Direction::PingPong)), vec![2, 3, 4, 5, 4, 3]);
    }

    #[test]
    fn spritesheet_mit_atlas() {
        let mut p = Project { sprites: vec![sprite(), Sprite::new("Klein", 2, 2).unwrap()], ..Default::default() };
        p.sprites[0].tags.push(Tag { name: "Lauf".into(), from: 0, to: 1, color: [0, 0, 0], direction: Direction::Forward });
        let (png_bytes, json) = sheet(&p, 1, "sheet.png").unwrap();
        assert_eq!(u32::from_be_bytes(png_bytes[16..20].try_into().unwrap()), 8, "2 Spalten × 4");
        assert_eq!(u32::from_be_bytes(png_bytes[20..24].try_into().unwrap()), 6, "2 Zeilen × 3");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["frames"].as_array().unwrap().len(), 3);
        assert_eq!(v["frames"][2]["x"], 1, "kleiner Sprite mittig in der Zelle");
        assert_eq!(v["tags"][0]["tags"][0]["name"], "Lauf");
    }
}
