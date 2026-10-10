//! Sprite aus Text einlesen — Gegenstück zu [`crate::codegen`], wie
//! `tsimport.js` der Web-Version. Das Format wird am Inhalt erkannt, nicht
//! an der Dateiendung — so klappt auch Einfügen aus der Zwischenablage.
//!
//! * Array-Formate (TypeScript, JavaScript, JSON, Python): `number[][]`
//!   bzw. `number[][][]` plus Palettenblock, verlustfrei
//! * SVG (`<rect>`, auch zusammengefasste Läufe), CSS (`box-shadow`),
//!   C-Header (`_WIDTH`/`_HEIGHT`, `_PALETTE`, `_DATA`), Text-Raster
//! * JSON (Spiel): flaches `data` plus `#rrggbbaa`-Palette und Materialien
//!
//! Animationen kommen mit allen Frames und ihrer Dauer zurück.
//!
//! Fehler tragen den deutschen Text als Vorlage ({…}-Platzhalter) — die
//! Oberfläche übersetzt ihn.

use std::collections::{BTreeMap, HashMap};

use regex::Regex;
use serde_json::Value;

use crate::codegen::{MATERIALS, TXT_CHARS};
use crate::image::{Px, FREE_BASE};
use crate::palette::{parse_hex, Rgb, MAX_COLORS};
use crate::sprite::{Frame, Layer, Sprite, MAX_SIDE};

/// Ein Pixel aus dem Text: Nummer oder freie Farbe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Index(u32),
    Color(Rgb),
}

type Grid = Vec<Vec<Cell>>;

#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub w: u32,
    pub h: u32,
    pub frames: usize,
    pub palette_count: usize,
    /// Aus freien Farben zurückgewonnene Pixel.
    pub restored: usize,
    /// Nummern, die im Bild vorkommen, aber in der Palette fehlen.
    pub unknown: Vec<u32>,
    pub format: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    pub frames: Vec<Grid>,
    pub durations: Option<Vec<u32>>,
    pub palette: Option<BTreeMap<u16, Rgb>>,
    pub materials: Option<BTreeMap<u16, String>>,
    pub name: Option<String>,
    pub stats: Stats,
}

/// Fehler: deutscher Text mit {…}-Platzhaltern, dazu die Werte.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportError {
    pub text: &'static str,
    pub args: Vec<(&'static str, String)>,
    /// Bei „JSON (Spiel)“: der eigentliche Grund (füllt `{reason}`).
    pub reason: Option<Box<ImportError>>,
}

impl ImportError {
    fn new(text: &'static str, args: &[(&'static str, String)]) -> Self {
        ImportError { text, args: args.to_vec(), reason: None }
    }

    /// Fertiger deutscher Text (für Tests und Protokolle).
    pub fn german(&self) -> String {
        let mut s = self.text.to_string();
        for (k, v) in &self.args {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        if let Some(r) = &self.reason {
            s = s.replace("{reason}", &r.german());
        }
        s
    }
}

type Res<T> = Result<T, ImportError>;

fn re(p: &str) -> Regex {
    Regex::new(p).expect("gültiges Muster")
}

/// `#abc` → `#aabbcc`, Alpha weg, sonst `None`.
fn normalize_hex(h: &str) -> Option<Rgb> {
    let s = h.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    // Erst prüfen, dann schneiden: mit anderen Zeichen als Hex-Ziffern läge
    // Byte 6 womöglich mitten in einem Zeichen (Absturz statt „unlesbar“).
    if !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let full = match s.len() {
        3 => s.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => s.to_string(),
        8 => s[..6].to_string(),
        _ => return None,
    };
    parse_hex(&format!("#{full}"))
}

// ── Array-Formate ───────────────────────────────────────────────────

/// Das erste balancierte `[[ … ]]` finden und auswerten.
fn extract_frames(text: &str) -> Option<Vec<Grid>> {
    let start = re(r"\[\s*\[").find(text)?.start();
    let bytes = text.as_bytes();
    let (mut depth, mut end, mut in_str) = (0i32, None, None::<u8>);
    let mut i = start;
    while i < bytes.len() {
        let ch = bytes[i];
        if let Some(q) = in_str {
            if ch == b'\\' {
                i += 2;
                continue;
            }
            if ch == q {
                in_str = None;
            }
        } else if ch == b'"' || ch == b'\'' {
            in_str = Some(ch);
        } else if ch == b'[' {
            depth += 1;
        } else if ch == b']' {
            depth -= 1;
            if depth == 0 {
                end = Some(i);
                break;
            }
        }
        i += 1;
    }
    let src = &text[start..=end?];
    // Nur Zahlen, Hex-Texte, Klammern, Kommas und Leerraum.
    if !re(r##"^[\s\[\],0-9'"#a-fA-F_-]*$"##).is_match(src) {
        return None;
    }
    let src = re(r",(\s*[\]}])").replace_all(&src.replace('\'', "\""), "$1").into_owned();
    let arr: Value = serde_json::from_str(&src).ok()?;
    let list = arr.as_array()?;
    let first = list.first()?.as_array()?;
    let grids: Vec<&Value> = if first.first().is_some_and(Value::is_array) { list.iter().collect() } else { vec![&arr] };
    let grids: Option<Vec<Grid>> = grids.into_iter().map(normalize_grid).collect();
    Some(pad_frames(grids?))
}

/// Alle Frames auf die größte Größe, leer aufgefüllt.
fn pad_frames(grids: Vec<Grid>) -> Vec<Grid> {
    let h = grids.iter().map(Vec::len).max().unwrap_or(0);
    let w = grids.iter().map(|g| g.first().map_or(0, Vec::len)).max().unwrap_or(0);
    grids.into_iter().map(|g| (0..h).map(|y| (0..w).map(|x| g.get(y).and_then(|r| r.get(x)).copied().unwrap_or(Cell::Index(0))).collect()).collect()).collect()
}

fn normalize_grid(v: &Value) -> Option<Grid> {
    let rows = v.as_array()?;
    if rows.is_empty() {
        return None;
    }
    let width = rows.iter().map(|r| r.as_array().map_or(0, Vec::len)).max()?;
    if width == 0 {
        return None;
    }
    rows.iter()
        .map(|r| {
            let r = r.as_array()?;
            (0..width)
                .map(|x| match r.get(x) {
                    None => Some(Cell::Index(0)),
                    Some(Value::Number(n)) => n.as_u64().filter(|&v| v <= u32::MAX as u64).map(|v| Cell::Index(v as u32)),
                    Some(Value::String(s)) => normalize_hex(s).map(Cell::Color),
                    _ => None,
                })
                .collect()
        })
        .collect()
}

fn extract_durations(text: &str) -> Option<Vec<u32>> {
    let m = re(r"_DURATIONS\b[^=\n]*=\s*\[([\d\s,]+)\]").captures(text).or_else(|| re(r#"(?i)["']durations["']\s*:\s*\[([\d\s,]+)\]"#).captures(text))?;
    let list: Vec<u32> = m[1].split(',').filter_map(|s| s.trim().parse::<f64>().ok()).filter(|v| *v > 0.0).map(|v| v as u32).collect();
    (!list.is_empty()).then_some(list)
}

/// Dauer-Liste nur, wenn sie zu den Frames passt (und es mehrere gibt).
fn fit_durations(d: Option<Vec<u32>>, n: usize) -> Option<Vec<u32>> {
    d.filter(|d| d.len() == n && n > 1).map(|d| d.into_iter().map(|v| v.max(10)).collect())
}

/// Alle `index: '#hex'`-Paare.
fn extract_palette(text: &str) -> Option<BTreeMap<u32, Rgb>> {
    let mut out = BTreeMap::new();
    let r = re(r#"(?:^|[{,\s])['"]?(\d{1,3})['"]?\s*:\s*['"]\s*(#?[0-9a-fA-F]{3,8})\s*['"]"#);
    for c in r.captures_iter(text) {
        let idx: u32 = c[1].parse().unwrap_or(0);
        if let Some(hex) = normalize_hex(&c[2]) {
            if (1..=999).contains(&idx) {
                out.entry(idx).or_insert(hex);
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

fn extract_name(text: &str) -> Option<String> {
    let decl = re(r"(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=]*)?=\s*\[");
    if let Some(n) = decl.captures_iter(text).map(|c| c[1].to_string()).find(|n| !n.ends_with("_DURATIONS")) {
        return Some(n);
    }
    if let Some(c) = re(r#"["']name["']\s*:\s*["']([^"']+)["']"#).captures(text) {
        return Some(c[1].to_string());
    }
    re(r"(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)_PALETTE\b").captures(text).map(|c| c[1].to_string())
}

// ── Bildhafte Formate ───────────────────────────────────────────────

struct Parsed {
    frames: Vec<Grid>,
    durations: Option<Vec<u32>>,
    palette: Option<BTreeMap<u16, Rgb>>,
    materials: Option<BTreeMap<u16, String>>,
}

/// Farbraster → Nummern in der Reihenfolge des Auftretens. Über 255 bleibt
/// eine Farbe frei.
fn frames_from_colors(color_frames: &[Vec<Vec<Option<Rgb>>>], w: usize, h: usize) -> (Vec<Grid>, Option<BTreeMap<u16, Rgb>>) {
    let mut index: HashMap<Rgb, u32> = HashMap::new();
    let mut order: Vec<Rgb> = Vec::new();
    for colors in color_frames {
        for row in colors.iter().take(h) {
            for c in row.iter().take(w).flatten() {
                index.entry(*c).or_insert_with(|| {
                    order.push(*c);
                    order.len() as u32
                });
            }
        }
    }
    let palette: BTreeMap<u16, Rgb> = order.iter().enumerate().filter(|(i, _)| *i < MAX_COLORS).map(|(i, &c)| (i as u16 + 1, c)).collect();
    let frames = color_frames
        .iter()
        .map(|colors| {
            (0..h)
                .map(|y| {
                    (0..w)
                        .map(|x| match colors[y][x] {
                            None => Cell::Index(0),
                            Some(c) => {
                                let i = index[&c];
                                if i as usize <= MAX_COLORS {
                                    Cell::Index(i)
                                } else {
                                    Cell::Color(c)
                                }
                            }
                        })
                        .collect()
                })
                .collect()
        })
        .collect();
    (frames, (!palette.is_empty()).then_some(palette))
}

fn too_big(text: &'static str, w: i64, h: i64) -> ImportError {
    ImportError::new(text, &[("w", w.to_string()), ("h", h.to_string())])
}

/// Ein `<rect>`: x, y, Breite, Höhe, Farbe.
type SvgRect = (i64, i64, i64, i64, Rgb);

fn parse_svg(text: &str) -> Res<Parsed> {
    let vb = re(r#"(?i)viewBox\s*=\s*["']\s*0\s+0\s+([\d.]+)\s+([\d.]+)"#).captures(text);
    let rect_re = re(r"(?i)<rect\b[^>]*>");
    let rects: Vec<&str> = rect_re.find_iter(text).map(|m| m.as_str()).collect();
    if rects.is_empty() {
        return Err(ImportError::new("SVG erkannt, aber kein <rect> darin gefunden.", &[]));
    }
    let groups: Vec<(String, Vec<&str>)> = re(r"(?is)<g\b([^>]*)>(.*?)</g>")
        .captures_iter(text)
        .filter(|c| rect_re.is_match(c.get(2).map_or("", |m| m.as_str())))
        .map(|c| (c[1].to_string(), rect_re.find_iter(c.get(2).expect("Gruppe").as_str()).map(|m| m.as_str()).collect()))
        .collect();
    let ms_re = re(r#"(?i)data-ms\s*=\s*["'](\d+)"#);
    let parts: Vec<(Vec<&str>, u32)> = if groups.len() >= 2 {
        groups.into_iter().map(|(attrs, r)| (r, ms_re.captures(&attrs).and_then(|c| c[1].parse().ok()).unwrap_or(0))).collect()
    } else {
        vec![(rects, 0)]
    };
    let attr = |tag: &str, name: &str| -> Option<String> { re(&format!(r#"(?i){name}\s*=\s*["']([^"']*)"#)).captures(tag).map(|c| c[1].trim().to_string()) };
    let num = |v: Option<String>, d: f64| v.and_then(|s| s.parse::<f64>().ok()).unwrap_or(d);
    let (mut max_x, mut max_y, mut any) = (0i64, 0i64, false);
    let items: Vec<Vec<SvgRect>> = parts
        .iter()
        .map(|(rects, _)| {
            let mut v = Vec::new();
            for tag in rects {
                let Some(fill) = attr(tag, "fill").and_then(|f| normalize_hex(&f)) else { continue };
                let x = num(attr(tag, "x"), 0.0).round() as i64;
                let y = num(attr(tag, "y"), 0.0).round() as i64;
                let w = (num(attr(tag, "width"), 1.0).round() as i64).max(1);
                let h = (num(attr(tag, "height"), 1.0).round() as i64).max(1);
                v.push((x, y, w, h, fill));
                max_x = max_x.max(x + w);
                max_y = max_y.max(y + h);
                any = true;
            }
            v
        })
        .collect();
    if !any {
        return Err(ImportError::new("SVG erkannt, aber kein <rect> mit Füllfarbe gefunden.", &[]));
    }
    let w = vb.as_ref().map_or(max_x, |c| c[1].parse::<f64>().unwrap_or(0.0).round() as i64).max(1);
    let h = vb.as_ref().map_or(max_y, |c| c[2].parse::<f64>().unwrap_or(0.0).round() as i64).max(1);
    if w > MAX_SIDE as i64 || h > MAX_SIDE as i64 {
        return Err(too_big("SVG ist {w}×{h} groß — das Raster wäre zu fein.", w, h));
    }
    let (wu, hu) = (w as usize, h as usize);
    let color_frames: Vec<Vec<Vec<Option<Rgb>>>> = items
        .iter()
        .map(|items| {
            let mut c = vec![vec![None; wu]; hu];
            for &(x, y, rw, rh, fill) in items {
                for yy in y.max(0)..(y + rh).min(h) {
                    for xx in x.max(0)..(x + rw).min(w) {
                        c[yy as usize][xx as usize] = Some(fill);
                    }
                }
            }
            c
        })
        .collect();
    let (frames, palette) = frames_from_colors(&color_frames, wu, hu);
    Ok(Parsed { frames, durations: Some(parts.iter().map(|p| p.1).collect()), palette, materials: None })
}

fn parse_css(text: &str) -> Res<Parsed> {
    let Some(block) = re(r"(?is)box-shadow\s*:(.*?);").captures(text) else {
        return Err(ImportError::new("Kein box-shadow-Block gefunden.", &[]));
    };
    let mut blocks = vec![block[1].to_string()];
    let mut durations = None;
    if let Some(kf) = re(r"(?i)@keyframes\b").find(text) {
        let rest = &text[kf.start()..];
        let steps: Vec<(f64, String)> =
            re(r"(?is)([\d.]+)%\s*\{\s*box-shadow\s*:(.*?);\s*\}").captures_iter(rest).map(|c| (c[1].parse().unwrap_or(0.0), c[2].to_string())).collect();
        if steps.len() >= 2 {
            blocks = steps.iter().map(|s| s.1.clone()).collect();
            if let Some(am) = re(r"(?i)animation\s*:[^;]*?\b([\d.]+)(ms|s)\b").captures(text) {
                let total = am[1].parse::<f64>().unwrap_or(0.0) * if am[2].eq_ignore_ascii_case("s") { 1000.0 } else { 1.0 };
                let p: Vec<f64> = steps.iter().map(|s| s.0).collect();
                durations = Some((0..p.len()).map(|i| ((((if i + 1 < p.len() { p[i + 1] } else { 100.0 }) - p[i]) / 100.0 * total).round()) as u32).collect());
            }
        }
    }
    let shadow = re(r"(-?\d+)px\s+(-?\d+)px(?:\s+-?\d+(?:px)?){0,2}\s*(#[0-9a-fA-F]{3,8})");
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
    let pts: Vec<Vec<(i64, i64, Rgb)>> = blocks
        .iter()
        .map(|b| {
            let mut v = Vec::new();
            for c in shadow.captures_iter(b) {
                let Some(hex) = normalize_hex(&c[3]) else { continue };
                let (x, y): (i64, i64) = (c[1].parse().unwrap_or(0), c[2].parse().unwrap_or(0));
                v.push((x, y, hex));
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
            v
        })
        .collect();
    if pts.iter().all(Vec::is_empty) {
        return Err(ImportError::new("box-shadow gefunden, aber keine Pixel darin gelesen.", &[]));
    }
    min_x = min_x.min(0);
    min_y = min_y.min(0);
    if let Some(mg) = re(r"(?i)margin\s*:\s*0\s+(\d+)px\s+(\d+)px\s+0").captures(text) {
        if min_x == 0 && min_y == 0 {
            max_x = max_x.max(mg[1].parse().unwrap_or(0));
            max_y = max_y.max(mg[2].parse().unwrap_or(0));
        }
    }
    let (w, h) = (max_x - min_x + 1, max_y - min_y + 1);
    if w > MAX_SIDE as i64 || h > MAX_SIDE as i64 {
        return Err(too_big("Das ergäbe {w}×{h} Pixel — zu groß.", w, h));
    }
    let color_frames: Vec<Vec<Vec<Option<Rgb>>>> = pts
        .iter()
        .map(|p| {
            let mut c = vec![vec![None; w as usize]; h as usize];
            for &(x, y, hex) in p {
                c[(y - min_y) as usize][(x - min_x) as usize] = Some(hex);
            }
            c
        })
        .collect();
    let (frames, palette) = frames_from_colors(&color_frames, w as usize, h as usize);
    Ok(Parsed { frames, durations, palette, materials: None })
}

fn parse_c(text: &str) -> Res<Parsed> {
    let (Some(wm), Some(hm)) = (re(r"(?i)#define\s+\w*_?WIDTH\s+(\d+)").captures(text), re(r"(?i)#define\s+\w*_?HEIGHT\s+(\d+)").captures(text)) else {
        return Err(ImportError::new("C-Header ohne _WIDTH und _HEIGHT — Maße unbekannt.", &[]));
    };
    let (w, h): (i64, i64) = (wm[1].parse().unwrap_or(0), hm[1].parse().unwrap_or(0));
    if w == 0 || h == 0 || w > MAX_SIDE as i64 || h > MAX_SIDE as i64 {
        return Err(too_big("Maße {w}×{h} sind nicht brauchbar.", w, h));
    }
    let mut palette = BTreeMap::new();
    let mut free: HashMap<u32, Rgb> = HashMap::new();
    if let Some(pb) = re(r"(?is)_PALETTE\s*\[[^\]]*\]\s*=\s*\{(.*?)\}").captures(text) {
        for (i, m) in re(r"0x([0-9a-fA-F]{6})").captures_iter(&pb[1]).enumerate() {
            let Some(hex) = parse_hex(&format!("#{}", &m[1])) else { continue };
            if i < 1 {
                continue;
            }
            if i <= MAX_COLORS {
                palette.insert(i as u16, hex);
            } else {
                free.insert(i as u32, hex);
            }
        }
    }
    let data = re(r"(?is)_DATA\s*(?:\[[^\]]*\]\s*)+=\s*\{(.*?)\}\s*;").captures(text).or_else(|| re(r"(?is)_DATA\s*\[[^\]]*\]\s*=\s*\{(.*?)\}").captures(text));
    let Some(data) = data else {
        return Err(ImportError::new("C-Header ohne _DATA-Feld — keine Pixel gefunden.", &[]));
    };
    let clean = re(r"//[^\n]*").replace_all(&re(r"(?s)/\*.*?\*/").replace_all(&data[1], ""), "").into_owned();
    let flat: Vec<u32> = re(r"\d+").find_iter(&clean).map(|m| m.as_str().parse().unwrap_or(0)).collect();
    let n = re(r"(?i)#define\s+\w*_?FRAMES\s+(\d+)").captures(text).and_then(|c| c[1].parse::<usize>().ok()).unwrap_or(1).clamp(1, 512);
    let (wu, hu) = (w as usize, h as usize);
    if flat.len() < wu * hu * n {
        let a = [("have", flat.len().to_string()), ("w", w.to_string()), ("h", (hu * n).to_string()), ("need", (wu * hu * n).to_string())];
        return Err(ImportError::new("_DATA hat {have} Werte, für {w}×{h} braucht es {need}.", &a));
    }
    let frames = (0..n)
        .map(|f| {
            (0..hu)
                .map(|y| {
                    let o = f * wu * hu + y * wu;
                    flat[o..o + wu].iter().map(|&v| free.get(&v).map_or(Cell::Index(v), |&c| Cell::Color(c))).collect()
                })
                .collect()
        })
        .collect();
    let durations = re(r"(?i)_DURATIONS\s*\[[^\]]*\]\s*=\s*\{([^}]*)\}")
        .captures(text)
        .map(|c| re(r"\d+").find_iter(&c[1]).map(|m| m.as_str().parse().unwrap_or(0)).collect());
    Ok(Parsed { frames, durations, palette: (!palette.is_empty()).then_some(palette), materials: None })
}

fn parse_text(text: &str) -> Res<Parsed> {
    let lines: Vec<&str> = text.lines().collect();
    let legend_re = re(r"^\s*(\S)\s*=\s*(#[0-9a-fA-F]{3,8})");
    let mut legend: HashMap<char, Rgb> = HashMap::new();
    for l in &lines {
        if let Some(c) = legend_re.captures(l) {
            let ch = c[1].chars().next().expect("Zeichen");
            if TXT_CHARS.contains(ch) {
                if let Some(hex) = normalize_hex(&c[2]) {
                    legend.insert(ch, hex);
                }
            }
        }
    }
    let row_re = re(r"^[.0-9a-zA-Z]+$");
    let has_re = re(r"[.1-9]");
    let is_row = |l: &str| l.len() >= 2 && row_re.is_match(l) && has_re.is_match(l);
    let header_re = re(r"(?i)^Frame\s+\d+\b.*?(\d+)\s*ms\b");
    let mut blocks: Vec<(Vec<String>, Option<u32>)> = Vec::new();
    let (mut cur, mut header, mut pending): (Vec<String>, Option<u32>, Option<u32>) = (Vec::new(), None, None);
    for raw in &lines {
        let l = raw.trim();
        if is_row(l) && (cur.is_empty() || l.len() == cur[0].len()) {
            if cur.is_empty() {
                header = pending.take();
            }
            cur.push(l.to_string());
            continue;
        }
        if cur.len() >= 2 {
            blocks.push((std::mem::take(&mut cur), header.take()));
        }
        cur.clear();
        header = None;
        pending = match header_re.captures(l) {
            Some(c) => c[1].parse().ok(),
            None if l.is_empty() => pending,
            None => None,
        };
    }
    if cur.len() >= 2 {
        blocks.push((cur, header));
    }
    if blocks.is_empty() {
        return Err(ImportError::new("Kein Zeichenraster gefunden (gleich lange Zeilen aus . und 1-9).", &[]));
    }
    let framed: Vec<&(Vec<String>, Option<u32>)> = blocks.iter().filter(|b| b.1.is_some()).collect();
    let chosen: Vec<&(Vec<String>, Option<u32>)> = if framed.len() >= 2 {
        framed
    } else {
        // Der längste Block gewinnt (bei Gleichstand der erste).
        vec![blocks.iter().fold(&blocks[0], |a, b| if b.0.len() > a.0.len() { b } else { a })]
    };
    let h = chosen.iter().map(|b| b.0.len()).max().unwrap_or(0);
    let w = chosen.iter().map(|b| b.0[0].len()).max().unwrap_or(0);
    if w > MAX_SIDE as usize || h > MAX_SIDE as usize {
        return Err(too_big("Raster ist {w}×{h} — zu groß.", w as i64, h as i64));
    }
    let mut palette = BTreeMap::new();
    let frames = chosen
        .iter()
        .map(|b| {
            (0..h)
                .map(|y| {
                    (0..w)
                        .map(|x| {
                            let ch = b.0.get(y).and_then(|r| r.as_bytes().get(x)).map_or('.', |&c| c as char);
                            let idx = TXT_CHARS.find(ch).unwrap_or(0);
                            if idx == 0 {
                                return Cell::Index(0);
                            }
                            let hex = legend.get(&ch).copied();
                            if idx <= MAX_COLORS {
                                if let Some(hx) = hex {
                                    palette.insert(idx as u16, hx);
                                }
                                Cell::Index(idx as u32)
                            } else {
                                hex.map_or(Cell::Index(0), Cell::Color)
                            }
                        })
                        .collect()
                })
                .collect()
        })
        .collect();
    let durations = (chosen.len() > 1).then(|| chosen.iter().map(|b| b.1.unwrap_or(0)).collect());
    Ok(Parsed { frames, durations, palette: (!palette.is_empty()).then_some(palette), materials: None })
}

// ── JSON (Spiel) ────────────────────────────────────────────────────

fn read_game(text: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(text).ok()?;
    let o = v.as_object()?;
    (o.get("data").is_some_and(Value::is_array) && o.get("palette").is_some_and(Value::is_array) && o.contains_key("width") && o.contains_key("height"))
        .then_some(v)
}

fn game_err(text: &'static str, args: &[(&'static str, String)]) -> ImportError {
    ImportError { text: "JSON (Spiel) erkannt, aber {reason}", args: Vec::new(), reason: Some(Box::new(ImportError::new(text, args))) }
}

fn int(v: Option<&Value>) -> Option<i64> {
    v.and_then(|v| v.as_i64().or_else(|| v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)))
}

/// Prüfen wie `validateGameSprite` — wirft beim ersten Fehler.
fn validate_game(o: &Value) -> Res<()> {
    let show = |v: Option<&Value>| v.map_or("undefined".to_string(), |v| v.as_str().map_or_else(|| v.to_string(), String::from));
    if int(o.get("version")) != Some(1) {
        return Err(game_err("unbekannte Version {version}.", &[("version", show(o.get("version")))]));
    }
    let (w, h) = (int(o.get("width")), int(o.get("height")));
    let (Some(w), Some(h)) = (w.filter(|&w| w >= 1), h.filter(|&h| h >= 1)) else {
        return Err(game_err("ungültige Größe {w}×{h}.", &[("w", show(o.get("width"))), ("h", show(o.get("height")))]));
    };
    let pal = o["palette"].as_array().expect("geprüft");
    if pal.is_empty() {
        return Err(game_err("die Palette ist leer.", &[]));
    }
    let color = |p: &Value| p.get("color").and_then(Value::as_str).unwrap_or("").to_string();
    if color(&pal[0]) != "#00000000" {
        return Err(game_err("Index 0 muss transparent sein, ist aber {color}.", &[("color", color(&pal[0]))]));
    }
    let hex8 = re(r"^#[0-9a-f]{8}$");
    for (i, p) in pal.iter().enumerate() {
        if !hex8.is_match(&color(p)) {
            return Err(game_err("ungültige Farbe {color}.", &[("color", color(p)), ("i", i.to_string())]));
        }
        let m = p.get("material").and_then(Value::as_str).unwrap_or("");
        if !MATERIALS.contains(&m) {
            return Err(game_err("unbekanntes Material „{material}“ bei Index {i}.", &[("material", m.to_string()), ("i", i.to_string())]));
        }
    }
    let data = o["data"].as_array().expect("geprüft");
    if data.len() as i64 != w * h {
        let a = [("len", data.len().to_string()), ("w", w.to_string()), ("h", h.to_string()), ("expected", (w * h).to_string())];
        return Err(game_err("data hat {len} Werte, erwartet sind {w} × {h} = {expected}.", &a));
    }
    for (i, v) in data.iter().enumerate() {
        if !int(Some(v)).is_some_and(|v| v >= 0 && (v as usize) < pal.len()) {
            let a = [("value", show(Some(v))), ("x", (i as i64 % w).to_string()), ("y", (i as i64 / w).to_string()), ("max", (pal.len() - 1).to_string())];
            return Err(game_err("Pixel ({x}, {y}) hat Index {value}, gültig ist 0 bis {max}.", &a));
        }
    }
    let mut need = 1;
    if let Some(sprites) = o.get("sprites").and_then(Value::as_object) {
        need = 0;
        for (k, r) in sprites {
            let f = r.get("frames").map_or(Some(1), |v| int(Some(v)));
            let vals = [int(r.get("x")), int(r.get("y")), int(r.get("w")), int(r.get("h")), f];
            let ok = match vals {
                [Some(x), Some(y), Some(rw), Some(rh), Some(f)] => x >= 0 && y >= 0 && rw >= 1 && rh >= 1 && f >= 1 && x + rw * f <= w && y + rh <= h,
                _ => false,
            };
            if !ok {
                return Err(game_err("Ausschnitt „{name}“ liegt nicht vollständig im Bild.", &[("name", k.clone())]));
            }
            need += f.unwrap_or(1);
        }
    }
    if let Some(d) = o.get("durations") {
        let ok = d.as_array().is_some_and(|d| d.len() as i64 == need && d.iter().all(|v| int(Some(v)).is_some_and(|v| (1..=60000).contains(&v))));
        if !ok {
            return Err(game_err("durations braucht {need} ganze Zahlen (ms), eine je Frame.", &[("need", need.to_string())]));
        }
    }
    Ok(())
}

fn parse_game(text: &str) -> Res<Parsed> {
    let Some(o) = read_game(text) else {
        return Err(game_err("die Palette ist leer.", &[]));
    };
    validate_game(&o)?;
    let pal = o["palette"].as_array().expect("geprüft");
    let hex: Vec<Option<Rgb>> = pal.iter().map(|p| p["color"].as_str().and_then(normalize_hex)).collect();
    let (mut palette, mut materials) = (BTreeMap::new(), BTreeMap::new());
    for i in 1..hex.len().min(MAX_COLORS + 1) {
        if let Some(c) = hex[i] {
            palette.insert(i as u16, c);
        }
        let m = pal[i]["material"].as_str().unwrap_or("");
        if !m.is_empty() && m != "none" && m != "empty" {
            materials.insert(i as u16, m.to_string());
        }
    }
    let (w, h) = (o["width"].as_u64().unwrap_or(0) as usize, o["height"].as_u64().unwrap_or(0) as usize);
    let data: Vec<u32> = o["data"].as_array().expect("geprüft").iter().map(|v| v.as_u64().unwrap_or(0) as u32).collect();
    let cell = |v: u32| if v as usize > MAX_COLORS { hex[v as usize].map_or(Cell::Index(0), Cell::Color) } else { Cell::Index(v) };
    let full: Grid = (0..h).map(|y| data[y * w..(y + 1) * w].iter().map(|&v| cell(v)).collect()).collect();
    let mut frames = vec![full.clone()];
    let mut durations = None;
    if let Some(r) = o.get("sprites").and_then(Value::as_object).and_then(|s| s.values().next()) {
        let g = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as usize;
        let n = r.get("frames").and_then(Value::as_u64).unwrap_or(1) as usize;
        let (x, y, rw, rh) = (g("x"), g("y"), g("w"), g("h"));
        frames = (0..n).map(|f| (0..rh).map(|yy| full[y + yy][x + f * rw..x + (f + 1) * rw].to_vec()).collect()).collect();
        if let Some(d) = o.get("durations").and_then(Value::as_array) {
            durations = Some(d.iter().take(n).map(|v| v.as_u64().unwrap_or(0) as u32).collect());
        }
    }
    Ok(Parsed { frames, durations, palette: (!palette.is_empty()).then_some(palette), materials: (!materials.is_empty()).then_some(materials) })
}

// ── Erkennen und einlesen ───────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Game,
    Svg,
    Css,
    C,
    Array,
    Txt,
}

fn detect(text: &str) -> Kind {
    if re(r"^\s*\{").is_match(text) && re(r#""data"\s*:"#).is_match(text) && read_game(text).is_some() {
        return Kind::Game;
    }
    if re(r"(?i)<svg[\s>]").is_match(text) && re(r"(?i)<rect\b").is_match(text) {
        return Kind::Svg;
    }
    if re(r"(?i)box-shadow\s*:").is_match(text) {
        return Kind::Css;
    }
    if re(r"(?i)#define\s+\w*_?(WIDTH|HEIGHT)\b").is_match(text) || re(r"\b(uint8_t|uint16_t|uint32_t)\b").is_match(text) {
        return Kind::C;
    }
    if re(r"\[\s*\[").is_match(text) {
        return Kind::Array;
    }
    Kind::Txt
}

fn unknown_of(frames: &[Grid], palette: &Option<BTreeMap<u16, Rgb>>) -> Vec<u32> {
    let Some(p) = palette else { return Vec::new() };
    let mut u: Vec<u32> = frames
        .iter()
        .flatten()
        .flatten()
        .filter_map(|c| match c {
            Cell::Index(i) if *i != 0 && (*i > u16::MAX as u32 || !p.contains_key(&(*i as u16))) => Some(*i),
            _ => None,
        })
        .collect();
    u.sort_unstable();
    u.dedup();
    u
}

/// Text einlesen.
pub fn parse(text: &str) -> Res<Imported> {
    if text.trim().is_empty() {
        return Err(ImportError::new("Nichts eingefügt.", &[]));
    }
    let kind = detect(text);
    if kind != Kind::Array {
        let (r, label) = match kind {
            Kind::Svg => (parse_svg(text)?, "SVG"),
            Kind::Css => (parse_css(text)?, "CSS"),
            Kind::C => (parse_c(text)?, "C-Header"),
            Kind::Game => (parse_game(text)?, "JSON (Spiel)"),
            _ => (parse_text(text)?, "Text-Raster"),
        };
        let n = r.frames.len();
        let unknown = unknown_of(&r.frames, &r.palette);
        return Ok(Imported {
            stats: Stats {
                w: r.frames[0][0].len() as u32,
                h: r.frames[0].len() as u32,
                frames: n,
                palette_count: r.palette.as_ref().map_or(0, BTreeMap::len),
                restored: 0,
                unknown,
                format: label,
            },
            durations: fit_durations(r.durations, n),
            frames: r.frames,
            palette: r.palette,
            materials: r.materials,
            name: extract_name(text),
        });
    }
    let Some(mut frames) = extract_frames(text) else {
        return Err(ImportError::new("Kein gültiges number[][]-Array gefunden. Erwartet wird [[0,1,…], …].", &[]));
    };
    let mut palette: Option<BTreeMap<u16, Rgb>> = None;
    let mut free: BTreeMap<u32, Rgb> = BTreeMap::new();
    if let Some(all) = extract_palette(text) {
        let mut p = BTreeMap::new();
        for (i, c) in all {
            if i as usize <= MAX_COLORS {
                p.insert(i as u16, c);
            } else {
                free.insert(i, c);
            }
        }
        palette = (!p.is_empty()).then_some(p);
    }
    let mut restored = 0;
    for c in frames.iter_mut().flatten().flatten() {
        if let Cell::Index(i) = *c {
            if let Some(&hex) = free.get(&i) {
                *c = Cell::Color(hex);
                restored += 1;
            }
        }
    }
    let n = frames.len();
    let unknown = unknown_of(&frames, &palette);
    Ok(Imported {
        stats: Stats {
            w: frames[0][0].len() as u32,
            h: frames[0].len() as u32,
            frames: n,
            palette_count: palette.as_ref().map_or(0, BTreeMap::len),
            restored,
            unknown,
            format: "Array",
        },
        durations: fit_durations(extract_durations(text), n),
        frames,
        palette,
        materials: None,
        name: extract_name(text),
    })
}

/// Eingelesenes als Sprite (eine Ebene). Sind alle Dauern gleich, wird
/// daraus die FPS-Zahl, sonst bekommt jeder Frame seine Dauer. Nummern ohne
/// Platz (über 255 und keine Farbe) werden durchsichtig.
pub fn to_sprite(imp: &Imported, name: &str, palette: &str) -> Option<Sprite> {
    let (w, h) = (imp.stats.w, imp.stats.h);
    let mut sp = Sprite::new(name, w, h).ok()?;
    sp.palette = palette.to_string();
    sp.images.clear();
    sp.frames.clear();
    let durs = imp.durations.clone().unwrap_or_default();
    let same = !durs.is_empty() && durs.iter().all(|&d| d == durs[0]);
    if same {
        sp.fps = ((1000.0 / durs[0] as f64).round() as u32).clamp(1, 60);
    }
    for (f, g) in imp.frames.iter().enumerate() {
        let mut img = crate::image::Image::new(w, h);
        for (y, row) in g.iter().enumerate() {
            for (x, c) in row.iter().enumerate() {
                let v: Px = match *c {
                    Cell::Index(0) => continue,
                    Cell::Index(i) if (i as usize) <= MAX_COLORS => i as Px,
                    Cell::Index(_) => continue,
                    Cell::Color(rgb) => sp.free_color(rgb),
                };
                img.set(x as u32, y as u32, v);
            }
        }
        sp.images.push(img);
        sp.frames.push(Frame { cels: vec![f], duration_ms: if same { 0 } else { durs.get(f).copied().unwrap_or(0) } });
    }
    sp.layers = vec![Layer::new("Ebene 1")];
    sp.guides.bottom = h;
    debug_assert!(sp.free.len() < (u16::MAX - FREE_BASE) as usize);
    Some(sp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_normalisieren() {
        assert_eq!(normalize_hex("#ABC"), Some([0xaa, 0xbb, 0xcc]));
        assert_eq!(normalize_hex("aabbccdd"), Some([0xaa, 0xbb, 0xcc]));
        assert_eq!(normalize_hex("#ab"), None);
        // 8 Bytes, aber ein Zeichen aus mehreren Bytes — vorher ein Absturz.
        assert_eq!(normalize_hex("#ab\u{fffd}cde"), None);
        assert_eq!(normalize_hex("#äbcdef"), None);
    }

    #[test]
    fn von_hand_geschriebenes_array() {
        let r = parse("const X = [[0, 1, '#ff0000'], [2]];").unwrap();
        assert_eq!(r.stats.w, 3);
        assert_eq!(r.frames[0][0][2], Cell::Color([255, 0, 0]));
        assert_eq!(r.frames[0][1][1], Cell::Index(0), "aufgefüllt");
        assert_eq!(r.name.as_deref(), Some("X"));
    }

    #[test]
    fn kein_fremder_code() {
        assert!(parse("const X = [[alert(1)]];").is_err());
    }

    #[test]
    fn leer_und_unbekannt() {
        assert_eq!(parse("  ").unwrap_err().german(), "Nichts eingefügt.");
        assert!(parse("hallo welt").is_err());
    }

    #[test]
    fn spiel_json_fehler_mit_grund() {
        let e = parse(r##"{"version":2,"width":1,"height":1,"palette":[],"data":[0]}"##).unwrap_err();
        assert_eq!(e.german(), "JSON (Spiel) erkannt, aber unbekannte Version 2.");
    }
}
