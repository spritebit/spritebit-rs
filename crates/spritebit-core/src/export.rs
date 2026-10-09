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

/// IDs wie in der Web-Version (`makeSpriteId`): Name mit `_` statt
/// Sonderzeichen, doppelte bekommen `_2`, `_3` …
pub fn sprite_ids(p: &Project) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for sp in &p.sprites {
        let base: String = sp.name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
        let base = base.trim_matches('_').to_string();
        let base = if base.is_empty() { "sprite".to_string() } else { base };
        let mut id = base.clone();
        let mut k = 2;
        while ids.contains(&id) {
            id = format!("{base}_{k}");
            k += 1;
        }
        ids.push(id);
    }
    ids
}

/// Spritesheet mit JSON-Atlas — wie in der Web-Version. Gleich große
/// Zellen (so groß wie der größte Sprite), jeder Frame mittig darin.
/// Ohne Animation: ein möglichst quadratisches Raster, ein Sprite je
/// Zelle. Mit Animation: eine Zeile je Sprite, seine Frames nebeneinander;
/// der Atlas nennt dann Frame-Nummer und Dauer.
pub fn sheet(p: &Project, scale: u32, image_name: &str) -> Result<(Vec<u8>, String), ExportError> {
    let scale = scale.max(1);
    let n = p.sprites.len().max(1) as u32;
    let animated = p.sprites.iter().any(|s| s.frames.len() > 1);
    let cw = p.sprites.iter().map(|s| s.width).max().unwrap_or(1);
    let ch = p.sprites.iter().map(|s| s.height).max().unwrap_or(1);
    let cols = if animated { p.sprites.iter().map(|s| s.frames.len()).max().unwrap_or(1) as u32 } else { (n as f64).sqrt().ceil() as u32 };
    let rows = if animated { n } else { n.div_ceil(cols) };
    let (w, h) = check(cols as u64 * cw as u64 * scale as u64, rows as u64 * ch as u64 * scale as u64)?;
    let mut out = vec![0u8; (w * h * 4) as usize];
    let ids = sprite_ids(p);
    let mut frames = Vec::new();
    for (i, sp) in p.sprites.iter().enumerate() {
        let pal = p.palette(&sp.palette);
        let cells: Vec<(usize, u32, u32)> = if animated {
            (0..sp.frames.len()).map(|f| (f, f as u32, i as u32)).collect()
        } else {
            vec![(sp.frame, i as u32 % cols, i as u32 / cols)]
        };
        for (f, col, row) in cells {
            let ox = col * cw + (cw - sp.width) / 2;
            let oy = row * ch + (ch - sp.height) / 2;
            let big = upscale(&frame_rgba(sp, &pal, f), sp.width, sp.height, scale);
            let (x0, y0) = (ox * scale, oy * scale);
            let bw = (sp.width * scale) as usize;
            for y in 0..(sp.height * scale) as usize {
                let dst = ((y0 as usize + y) * w as usize + x0 as usize) * 4;
                out[dst..dst + bw * 4].copy_from_slice(&big[y * bw * 4..(y + 1) * bw * 4]);
            }
            let mut e = json!({
                "name": sp.name, "id": ids[i], "x": x0, "y": y0, "w": sp.width * scale, "h": sp.height * scale, "palette": sp.palette,
            });
            if animated {
                e["frame"] = json!(f);
                e["duration"] = json!(sp.frame_duration(f));
            }
            frames.push(e);
        }
    }
    let atlas = json!({
        "image": image_name,
        "scale": scale,
        "cell": { "w": cw * scale, "h": ch * scale },
        "columns": cols,
        "frames": frames,
        "tags": p.sprites.iter().enumerate().filter(|(_, s)| !s.tags.is_empty()).map(|(i, s)| json!({
            "sprite": s.name,
            "id": ids[i],
            "tags": s.tags.iter().map(|t| json!({
                "name": t.name, "from": t.from, "to": t.to,
                "direction": match t.direction { Direction::Forward => "forward", Direction::Reverse => "reverse", Direction::PingPong => "pingpong" },
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    let json = serde_json::to_string_pretty(&atlas).expect("JSON aus eigenen Daten");
    Ok((encode_png(&out, w, h)?, json))
}

// ── Farb-Legende ────────────────────────────────────────────────────

/// Bis zu so vielen Farben stehen Hex-Codes neben den Feldern; darüber
/// gibt es eine kompakte, nach Farbton sortierte Farbkarte.
pub const LEGEND_LABEL_LIMIT: usize = 48;

/// Ein RGBA-Bild zum Zeichnen der Legende.
struct Canvas {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: u32, h: u32, bg: [u8; 3]) -> Canvas {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            px.extend_from_slice(&[bg[0], bg[1], bg[2], 255]);
        }
        Canvas { w, h, px }
    }

    fn rect(&mut self, x: u32, y: u32, w: u32, h: u32, c: [u8; 3]) {
        for yy in y..(y + h).min(self.h) {
            for xx in x..(x + w).min(self.w) {
                let o = ((yy * self.w + xx) * 4) as usize;
                self.px[o..o + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }

    /// Pixel mit Deckkraft `a` (0–1) über das Vorhandene legen.
    fn blend(&mut self, x: i64, y: i64, c: [u8; 3], a: f32) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 || a <= 0.0 {
            return;
        }
        let o = ((y as u32 * self.w + x as u32) * 4) as usize;
        for (p, &v) in self.px[o..o + 3].iter_mut().zip(&c) {
            *p = (*p as f32 * (1.0 - a) + v as f32 * a).round() as u8;
        }
    }

    /// Text in der Monospace-Schrift (Hack), Oberkante bei `y`.
    fn text(&mut self, x: f32, y: f32, s: &str, px: f32, c: [u8; 3], bold: bool) {
        use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
        let font = FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR).expect("eingebaute Schrift");
        let sf = font.as_scaled(PxScale::from(px));
        let mut pen = x;
        for ch in s.chars() {
            let id = sf.glyph_id(ch);
            let g = id.with_scale_and_position(px, ab_glyph::point(pen, y + sf.ascent()));
            if let Some(o) = font.outline_glyph(g) {
                let b = o.px_bounds();
                o.draw(|gx, gy, cov| {
                    let (xx, yy) = (b.min.x as i64 + gx as i64, b.min.y as i64 + gy as i64);
                    self.blend(xx, yy, c, cov);
                    if bold {
                        self.blend(xx + 1, yy, c, cov * 0.8);
                    }
                });
            }
            pen += sf.h_advance(id);
        }
    }

    fn text_width(s: &str, px: f32) -> f32 {
        use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
        let font = FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR).expect("eingebaute Schrift");
        let sf = font.as_scaled(PxScale::from(px));
        s.chars().map(|c| sf.h_advance(sf.glyph_id(c))).sum()
    }
}

/// Farben des Bildes in der Reihenfolge ihres Auftretens.
pub fn used_colors(rgba: &[u8]) -> Vec<[u8; 3]> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for p in rgba.as_chunks::<4>().0 {
        if p[3] > 0 && seen.insert([p[0], p[1], p[2]]) {
            out.push([p[0], p[1], p[2]]);
        }
    }
    out
}

fn hue_key(c: [u8; 3]) -> f64 {
    let [r, g, b] = c.map(|v| v as f64);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    let mut h = 0.0;
    if d > 0.0 {
        h = if max == r {
            ((g - b) / d) % 6.0
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        } * 60.0;
        if h < 0.0 {
            h += 360.0;
        }
    }
    h * 1000.0 + (max + min) / 2.0
}

/// Texte der Legende (die Oberfläche liefert sie übersetzt).
pub struct LegendText {
    /// z. B. „Palette — {n} Farben“
    pub title: String,
    /// z. B. „Palette — {n} Farben (nach Farbton sortiert)“
    pub sorted: String,
}

fn legend(colors: &[[u8; 3]], min_w: u32, t: &LegendText) -> Canvas {
    const PAD: u32 = 12;
    const HEAD: u32 = 24;
    const FONT: f32 = 12.0;
    const BG: [u8; 3] = [0x1e, 0x1e, 0x1e];
    let n = colors.len().to_string();
    if colors.len() <= LEGEND_LABEL_LIMIT {
        let (sw, gap, col_gap, row_h) = (18u32, 6u32, 18u32, 26u32);
        let text_w = Canvas::text_width("#000000", FONT).ceil() as u32 + 2;
        let col_w = sw + gap + text_w + col_gap;
        let target = min_w.max(280);
        let cols = ((target - PAD * 2 + col_gap) / col_w).max(1);
        let rows = (colors.len() as u32).div_ceil(cols);
        let w = target.max(PAD * 2 + cols * col_w - col_gap);
        let h = PAD * 2 + HEAD + rows * row_h;
        let mut c = Canvas::new(w, h, BG);
        c.text(PAD as f32, PAD as f32, &t.title.replace("{n}", &n), FONT, [0xd0, 0xd0, 0xd0], true);
        for (i, &col) in colors.iter().enumerate() {
            let cx = PAD + (i as u32 % cols) * col_w;
            let cy = PAD + HEAD + (i as u32 / cols) * row_h;
            c.rect(cx, cy, sw, sw, col);
            // Rahmen, 25 % weiß
            for k in 0..sw {
                for (x, y) in [(cx + k, cy), (cx + k, cy + sw - 1), (cx, cy + k), (cx + sw - 1, cy + k)] {
                    c.blend(x as i64, y as i64, [255, 255, 255], 0.25);
                }
            }
            let hex = format!("#{:02x}{:02x}{:02x}", col[0], col[1], col[2]);
            c.text((cx + sw + gap) as f32, cy as f32 + (sw as f32 - FONT) / 2.0, &hex, FONT, [0xc8, 0xc8, 0xc8], false);
        }
        return c;
    }
    let mut sorted = colors.to_vec();
    sorted.sort_by(|a, b| hue_key(*a).total_cmp(&hue_key(*b)));
    let (cell, gap) = (12u32, 1u32);
    let target = min_w.max(480);
    let cols = ((target - PAD * 2 + gap) / (cell + gap)).max(1);
    let rows = (sorted.len() as u32).div_ceil(cols);
    let w = target.max(PAD * 2 + cols * (cell + gap) - gap);
    let h = PAD * 2 + HEAD + rows * (cell + gap);
    let mut c = Canvas::new(w, h, BG);
    c.text(PAD as f32, PAD as f32, &t.sorted.replace("{n}", &n), FONT, [0xd0, 0xd0, 0xd0], true);
    for (i, &col) in sorted.iter().enumerate() {
        c.rect(PAD + (i as u32 % cols) * (cell + gap), PAD + HEAD + (i as u32 / cols) * (cell + gap), cell, cell, col);
    }
    c
}

/// Frame als RGBA, vergrößert, optional mit Farb-Legende darunter.
/// Gibt (RGBA, Breite, Höhe) zurück.
pub fn frame_image(sp: &Sprite, pal: &Palette, f: usize, scale: u32, legend_text: Option<&LegendText>) -> Result<(Vec<u8>, u32, u32), ExportError> {
    let scale = scale.max(1);
    let (w, h) = check(sp.width as u64 * scale as u64, sp.height as u64 * scale as u64)?;
    let rgba = frame_rgba(sp, pal, f);
    let big = upscale(&rgba, sp.width, sp.height, scale);
    let Some(t) = legend_text else { return Ok((big, w, h)) };
    let colors = used_colors(&rgba);
    if colors.is_empty() {
        return Ok((big, w, h));
    }
    let lg = legend(&colors, w, t);
    let (ow, oh) = (w.max(lg.w), h + lg.h);
    let mut out = vec![0u8; (ow * oh * 4) as usize];
    for y in 0..h as usize {
        out[y * ow as usize * 4..][..w as usize * 4].copy_from_slice(&big[y * w as usize * 4..][..w as usize * 4]);
    }
    // Legenden-Band auf volle Breite einfärben.
    for y in h..oh {
        for x in 0..ow {
            let o = ((y * ow + x) * 4) as usize;
            let src = if x < lg.w { &lg.px[(((y - h) * lg.w + x) * 4) as usize..][..4] } else { &[0x1e, 0x1e, 0x1e, 255][..] };
            out[o..o + 4].copy_from_slice(src);
        }
    }
    Ok((out, ow, oh))
}

/// Ein Frame als PNG, optional mit Legende.
pub fn png_with_legend(sp: &Sprite, pal: &Palette, f: usize, scale: u32, legend_text: Option<&LegendText>) -> Result<Vec<u8>, ExportError> {
    let (rgba, w, h) = frame_image(sp, pal, f, scale, legend_text)?;
    encode_png(&rgba, w, h)
}

// ── PDF ─────────────────────────────────────────────────────────────

/// Ein Frame als PDF — eine Seite genau so groß wie das Bild (1 px =
/// 0,75 pt wie bei jsPDF mit `px_scaling`), Transparenz bleibt (Alpha als
/// Maske).
pub fn pdf(sp: &Sprite, pal: &Palette, f: usize, scale: u32, legend_text: Option<&LegendText>) -> Result<Vec<u8>, ExportError> {
    use flate2::write::ZlibEncoder;
    use std::io::Write;
    let (rgba, w, h) = frame_image(sp, pal, f, scale, legend_text)?;
    let zip = |data: &[u8]| -> Result<Vec<u8>, ExportError> {
        let mut e = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).map_err(|e| ExportError::Encode(e.to_string()))?;
        e.finish().map_err(|e| ExportError::Encode(e.to_string()))
    };
    let rgb: Vec<u8> = rgba.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
    let alpha: Vec<u8> = rgba.as_chunks::<4>().0.iter().map(|p| p[3]).collect();
    let (rgb, alpha) = (zip(&rgb)?, zip(&alpha)?);
    let (pw, ph) = (w as f64 * 0.75, h as f64 * 0.75);
    let fmt = |v: f64| {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let content = format!("q {} 0 0 {} 0 0 cm /Im0 Do Q", fmt(pw), fmt(ph));
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    let mut obj = |out: &mut Vec<u8>, head: String, stream: Option<&[u8]>| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{head}\n", offsets.len()).as_bytes());
        if let Some(s) = stream {
            out.extend_from_slice(b"stream\n");
            out.extend_from_slice(s);
            out.extend_from_slice(b"\nendstream\n");
        }
        out.extend_from_slice(b"endobj\n");
    };
    obj(&mut out, "<< /Type /Catalog /Pages 2 0 R >>".into(), None);
    obj(&mut out, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(), None);
    obj(
        &mut out,
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>", fmt(pw), fmt(ph)),
        None,
    );
    obj(&mut out, format!("<< /Length {} >>", content.len()), Some(content.as_bytes()));
    obj(
        &mut out,
        format!("<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /SMask 6 0 R /Interpolate false /Length {} >>", rgb.len()),
        Some(&rgb),
    );
    obj(
        &mut out,
        format!("<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>", alpha.len()),
        Some(&alpha),
    );
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).as_bytes());
    Ok(out)
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
            assert_eq!(f.buffer[(4 + 1) * 4 + 3], 255, "schwarzer Pixel (1,1) sichtbar");
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

    fn texts() -> LegendText {
        LegendText { title: "Palette — {n} Farben".into(), sorted: "Palette — {n} Farben (sortiert)".into() }
    }

    #[test]
    fn legende_unter_dem_bild() {
        let pal = Palette::grayscale();
        let (rgba, w, h) = frame_image(&sprite(), &pal, 0, 4, Some(&texts())).unwrap();
        assert_eq!(w, 280, "mindestens 280 breit");
        assert!(h > 12 + 12 * 2 + 24, "Legende hängt unten an");
        // Oben links Sprite (transparent), darunter dunkles Band.
        assert_eq!(rgba[3], 0);
        let o = (20 * w * 4) as usize;
        assert_eq!(&rgba[o..o + 4], &[0x1e, 0x1e, 0x1e, 255]);
        // Ohne Legende: nur das Bild.
        let (_, w2, h2) = frame_image(&sprite(), &pal, 0, 4, None).unwrap();
        assert_eq!((w2, h2), (16, 12));
    }

    #[test]
    fn viele_farben_ergeben_eine_farbkarte() {
        let colors: Vec<[u8; 3]> = (0..60).map(|i| [i as u8 * 4, 0, 0]).collect();
        let c = legend(&colors, 10, &texts());
        assert_eq!(c.w, 480);
    }

    #[test]
    fn pdf_ist_ein_pdf() {
        let pal = Palette::grayscale();
        let b = pdf(&sprite(), &pal, 0, 2, None).unwrap();
        assert!(b.starts_with(b"%PDF-1.4"));
        assert!(b.ends_with(b"%%EOF\n"));
        let s = String::from_utf8_lossy(&b);
        assert!(s.contains("/MediaBox [0 0 6 4.5]"), "8 × 6 px → 6 × 4,5 pt");
        assert!(s.contains("/SMask 6 0 R"));
        // xref zeigt auf die richtigen Stellen.
        let at = b.windows(7).position(|w| w == b"2 0 obj").unwrap();
        assert!(s.contains(&format!("{at:010} 00000 n")));
    }

    #[test]
    fn spritesheet_ohne_animation_ist_ein_raster() {
        let p = Project {
            sprites: (0..3).map(|i| Sprite::new(format!("S{i}"), 2, 2).unwrap()).collect(),
            ..Default::default()
        };
        let (png_bytes, json) = sheet(&p, 1, "s.png").unwrap();
        assert_eq!(u32::from_be_bytes(png_bytes[16..20].try_into().unwrap()), 4, "2 Spalten");
        assert_eq!(u32::from_be_bytes(png_bytes[20..24].try_into().unwrap()), 4, "2 Zeilen");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["columns"], 2);
        assert!(v["frames"][0].get("frame").is_none());
        assert_eq!(v["frames"][2]["x"], 0);
        assert_eq!(v["frames"][2]["y"], 2);
    }

    #[test]
    fn ids_wie_im_web() {
        let p = Project { sprites: vec![Sprite::new("Held Grün", 1, 1).unwrap(), Sprite::new("Held Grün", 1, 1).unwrap()], ..Default::default() };
        assert_eq!(sprite_ids(&p), vec!["Held_Gr_n", "Held_Gr_n_2"]);
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
