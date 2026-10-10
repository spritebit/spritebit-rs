//! Den Sprite als Text ausgeben — wie `codegen.js` und `gamejson.js` der
//! Web-Version: TypeScript, JavaScript, JSON, JSON (Spiel), SVG, CSS,
//! C-Header, Python, Text-Raster.
//!
//! Alle Formate arbeiten auf derselben Vorbereitung ([`prepare`]):
//! exportiert wird, was man sieht (sichtbare Ebenen zusammengefügt);
//! Palettennummern bleiben Nummern, freie Farben bekommen Nummern oberhalb
//! der Palette. So ist jeder Text in sich geschlossen.
//!
//! Animationen: jedes Format trägt ALLE Frames samt Dauer. Mit nur einem
//! Frame bleibt jede Ausgabe wie ohne Frames.
//!
//! Die Kommentare im Code gibt es auf Deutsch und Englisch ([`CodeLang`]);
//! der Rest (Bezeichner, Schlüssel) ist sprachneutral, damit der Import
//! alles wieder lesen kann.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

use crate::palette::{Palette, Rgb};
use crate::selection::rgb_of;
use crate::sprite::Sprite;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeLang {
    De,
    En,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Ts,
    Js,
    Json,
    Game,
    Svg,
    Css,
    C,
    Py,
    Txt,
}

impl Format {
    pub const ALL: [Format; 9] = [Format::Ts, Format::Js, Format::Json, Format::Game, Format::Svg, Format::Css, Format::C, Format::Py, Format::Txt];

    pub fn label(self) -> &'static str {
        match self {
            Format::Ts => "TypeScript",
            Format::Js => "JavaScript (ESM)",
            Format::Json => "JSON",
            Format::Game => "JSON (Spiel)",
            Format::Svg => "SVG-Bild",
            Format::Css => "CSS (box-shadow)",
            Format::C => "C-Header (uint8)",
            Format::Py => "Python",
            Format::Txt => "Text-Raster",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Format::Ts => "ts",
            Format::Js => "js",
            Format::Json | Format::Game => "json",
            Format::Svg => "svg",
            Format::Css => "css",
            Format::C => "h",
            Format::Py => "py",
            Format::Txt => "txt",
        }
    }

    /// Reagiert das Format auf „Palette in den Code schreiben“?
    pub fn pal_option(self) -> bool {
        matches!(self, Format::Ts | Format::Js)
    }

    /// Braucht das Format Materialien je Farbe?
    pub fn materials(self) -> bool {
        self == Format::Game
    }

    /// Code-Sprachen bekommen den Bezeichner als Dateinamen, Assets einen Slug.
    fn upper(self) -> bool {
        matches!(self, Format::Ts | Format::Js | Format::C | Format::Py)
    }
}

/// Materialien für „JSON (Spiel)“, in der Reihenfolge der Auswahl.
pub const MATERIALS: [&str; 9] = ["none", "empty", "sand", "water", "stone", "ice", "steam", "metal", "wood"];
pub const DEFAULT_MATERIAL: &str = "none";

// ── Namen ───────────────────────────────────────────────────────────

/// Umlaute und ß ausschreiben, Akzente weglassen — „Held Grün“ → „Held Gruen“.
pub fn deumlaut(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        let rep = match c {
            'ä' => "ae",
            'ö' => "oe",
            'ü' => "ue",
            'ß' => "ss",
            'å' => "a",
            'æ' => "ae",
            'ø' => "oe",
            'Ä' => "AE",
            'Ö' => "OE",
            'Ü' => "UE",
            'Å' => "A",
            'Æ' => "AE",
            'Ø' => "OE",
            _ => {
                out.push(strip_accent(c));
                continue;
            }
        };
        out.push_str(rep);
    }
    out
}

/// Häufige Buchstaben mit Akzent → ohne (é → e). Der Rest bleibt.
fn strip_accent(c: char) -> char {
    const FROM: &str = "àáâãèéêëìíîïòóôõùúûýÿñçÀÁÂÃÈÉÊËÌÍÎÏÒÓÔÕÙÚÛÝÑÇ";
    const TO: &str = "aaaaeeeeiiiioooouuuyyncAAAAEEEEIIIIOOOOUUUYNC";
    FROM.chars().position(|f| f == c).and_then(|i| TO.chars().nth(i)).unwrap_or(c)
}

/// Gültiger Bezeichner: SCREAMING_SNAKE, nie mit Ziffer vorn.
pub fn identifier(name: &str) -> String {
    let src = if name.is_empty() { "SPRITE" } else { name };
    let mut id = String::new();
    let mut gap = false;
    for c in deumlaut(src).chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            if gap && !id.is_empty() {
                id.push('_');
            }
            gap = false;
            id.push(c.to_ascii_uppercase());
        } else {
            gap = true;
        }
    }
    let id = id.trim_matches('_').to_string();
    let id = if id.is_empty() { "SPRITE".to_string() } else { id };
    if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("S_{id}")
    } else {
        id
    }
}

/// Kleinbuchstaben mit `sep` zwischen den Wörtern.
fn words(name: &str, sep: char) -> String {
    let mut s = String::new();
    let mut gap = false;
    for c in deumlaut(name).to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if gap && !s.is_empty() {
                s.push(sep);
            }
            gap = false;
            s.push(c);
        } else {
            gap = true;
        }
    }
    s
}

/// Dateiname-tauglicher Slug („Held Grün“ → „held-gruen“).
pub fn slug(name: &str) -> String {
    let s = words(if name.is_empty() { "sprite" } else { name }, '-');
    if s.is_empty() {
        "sprite".into()
    } else {
        s
    }
}

/// snake_case für „JSON (Spiel)“ („Berg Hintergrund“ → „berg_hintergrund“).
pub fn game_name(name: &str) -> String {
    let s = words(name, '_');
    if s.is_empty() {
        "sprite".into()
    } else {
        s
    }
}

/// Dateiname zum Format.
pub fn filename(format: Format, sprite_name: &str) -> String {
    let base = match format {
        Format::Game => game_name(&deumlaut(sprite_name)),
        f if f.upper() => identifier(sprite_name),
        _ => slug(sprite_name),
    };
    format!("{base}.{}", format.ext())
}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

// ── Vorbereitung ────────────────────────────────────────────────────

/// Ein Pixel nach dem Zusammenfügen der Ebenen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Flat {
    Empty,
    Index(u16),
    Color(Rgb),
}

/// Sichtbare Ebenen von Frame `f` zusammengefügt — wie `flatGrid` im Web:
/// deckende Ebenen überschreiben, halbdurchsichtige mischen (ergibt eine
/// Farbe), freie Farben werden zu ihrer Farbe.
fn flat_frame(sp: &Sprite, pal: &Palette, f: usize) -> Vec<Flat> {
    let (w, h) = (sp.width as usize, sp.height as usize);
    let mut out = vec![Flat::Empty; w * h];
    let color_of = |v: Flat| match v {
        Flat::Empty => None,
        Flat::Index(i) => pal.get(i),
        Flat::Color(c) => Some(c),
    };
    for (l, layer) in sp.layers.iter().enumerate() {
        if !layer.visible || layer.opacity <= 0.0 {
            continue;
        }
        let a = layer.opacity as f64;
        for (x, y, v) in sp.cel(f, l).pixels() {
            let i = y as usize * w + x as usize;
            let top = if v >= crate::image::FREE_BASE { rgb_of(v, pal, &sp.free).map_or(Flat::Empty, Flat::Color) } else { Flat::Index(v) };
            if top == Flat::Empty {
                continue;
            }
            out[i] = if a >= 1.0 {
                top
            } else if out[i] == Flat::Empty {
                Flat::Color(color_of(top).unwrap_or([0, 0, 0]))
            } else {
                match (color_of(top), color_of(out[i])) {
                    (Some(t), Some(b)) => {
                        let mix = |k: usize| (b[k] as f64 * (1.0 - a) + t[k] as f64 * a).round() as u8;
                        Flat::Color([mix(0), mix(1), mix(2)])
                    }
                    _ => top,
                }
            };
        }
    }
    out
}

/// Alles, was die Formate brauchen.
pub struct Prepared {
    pub name: String,
    pub palette_name: String,
    pub id: String,
    pub w: u32,
    pub h: u32,
    /// Nummern je Frame, zeilenweise (freie Farben oberhalb der Palette).
    pub frames: Vec<Vec<u16>>,
    pub durations: Vec<u32>,
    pub fps: u32,
    /// Alle benutzten Farben: (Nummer, Farbe), Palette zuerst.
    pub entries: Vec<(u16, Rgb)>,
    /// Anzahl der Palettenfarben.
    pub max_idx: u16,
    pub has_raw: bool,
    /// Volle Palette (für „JSON (Spiel)“).
    pub palette: Vec<Rgb>,
    color: HashMap<u16, Rgb>,
}

impl Prepared {
    fn color_at(&self, x: u32, y: u32, f: usize) -> Option<String> {
        let v = self.frames[f][(y * self.w + x) as usize];
        (v != 0).then(|| self.color.get(&v).map(|&c| hex(c))).flatten()
    }
    fn n(&self) -> usize {
        self.frames.len()
    }
    fn rows(&self, f: usize) -> impl Iterator<Item = &[u16]> {
        self.frames[f].chunks(self.w as usize)
    }
}

pub fn prepare(sp: &Sprite, pal: &Palette) -> Prepared {
    let flats: Vec<Vec<Flat>> = (0..sp.frames.len()).map(|f| flat_frame(sp, pal, f)).collect();
    let max_idx = pal.len() as u16;
    // Freie Farben über alle Frames gleich nummerieren.
    let mut raw: Vec<Rgb> = Vec::new();
    let mut raw_idx: HashMap<Rgb, u16> = HashMap::new();
    let frames: Vec<Vec<u16>> = flats
        .iter()
        .map(|g| {
            g.iter()
                .map(|v| match *v {
                    Flat::Empty => 0,
                    Flat::Index(i) => i,
                    Flat::Color(c) => *raw_idx.entry(c).or_insert_with(|| {
                        raw.push(c);
                        max_idx + raw.len() as u16
                    }),
                })
                .collect()
        })
        .collect();
    let mut used = vec![false; max_idx as usize + raw.len() + 1];
    for g in &frames {
        for &v in g {
            if let Some(u) = used.get_mut(v as usize) {
                *u = true;
            }
        }
    }
    let mut entries: Vec<(u16, Rgb)> = Vec::new();
    for i in 1..=max_idx {
        if used[i as usize] {
            entries.push((i, pal.get(i).expect("Palettenfarbe")));
        }
    }
    for (k, &c) in raw.iter().enumerate() {
        let i = max_idx + 1 + k as u16;
        if used[i as usize] {
            entries.push((i, c));
        }
    }
    let color = entries.iter().copied().collect();
    Prepared {
        name: sp.name.clone(),
        palette_name: sp.palette.clone(),
        id: identifier(&sp.name),
        w: sp.width,
        h: sp.height,
        durations: (0..sp.frames.len()).map(|f| sp.frame_duration(f)).collect(),
        fps: sp.fps,
        frames,
        entries,
        max_idx,
        has_raw: !raw.is_empty(),
        palette: pal.colors.clone(),
        color,
    }
}

// ── Texte im Code ───────────────────────────────────────────────────

fn txt(lang: CodeLang, de: &str, en: &str, args: &[(&str, String)]) -> String {
    let mut s = if lang == CodeLang::De { de } else { en }.to_string();
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

fn join<T: ToString>(v: &[T], sep: &str) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(sep)
}

/// Zeilen als "  [0,1,2]".
fn rows(d: &Prepared, f: usize, indent: &str) -> String {
    d.rows(f).map(|r| format!("{indent}[{}]", join(r, ","))).collect::<Vec<_>>().join(",\n")
}

/// Mehrere Frames: "  [\n    [..],\n  ]" je Frame.
fn frame_rows(d: &Prepared, indent: &str) -> String {
    (0..d.n()).map(|f| format!("{indent}[\n{},\n{indent}]", rows(d, f, &format!("{indent}  ")))).collect::<Vec<_>>().join(",\n")
}

// ── Formate ─────────────────────────────────────────────────────────

fn js_like(d: &Prepared, with_palette: bool, typed: bool, lang: CodeLang) -> String {
    let ty = |x: &str| if typed { x.to_string() } else { String::new() };
    // Freie Farben ERZWINGEN den Palettenblock — ohne ihn wären die Nummern wertlos.
    let want_pal = with_palette || d.has_raw;
    let mut out = String::new();
    if d.has_raw {
        let n = d.entries.iter().filter(|e| e.0 > d.max_idx).count();
        let from = (d.max_idx + 1).to_string();
        let args = [("n", n.to_string()), ("from", from)];
        out += &if n == 1 {
            txt(lang, "// {n} freie Farbe wurde als Index {from}+ gesichert (verlustfrei)", "// {n} free color was saved as index {from}+ (lossless)", &args)
        } else {
            txt(
                lang,
                "// {n} freie Farben wurden als Indizes {from}+ gesichert (verlustfrei)",
                "// {n} free colors were saved as indices {from}+ (lossless)",
                &args,
            )
        };
        out.push('\n');
    }
    if want_pal {
        out += &txt(lang, "// Palette „{name}“", "// palette “{name}”", &[("name", d.palette_name.clone())]);
        let _ = write!(out, "\nexport const {}_PALETTE{} = {{\n", d.id, ty(": Record<number, string>"));
        out += &d.entries.iter().map(|(i, c)| format!("  {i}: '{}',", hex(*c))).collect::<Vec<_>>().join("\n");
        out += "\n};\n\n";
    }
    if d.n() == 1 {
        let _ = write!(out, "export const {}{} = [\n{},\n];", d.id, ty(": number[][]"), rows(d, 0, "  "));
        return out;
    }
    out += &txt(
        lang,
        "// Animation: {n} Frames — {id}[Frame][y][x], Dauer je Frame in ms: {id}_DURATIONS",
        "// Animation: {n} frames — {id}[frame][y][x], duration per frame in ms: {id}_DURATIONS",
        &[("n", d.n().to_string()), ("id", d.id.clone())],
    );
    let _ = write!(
        out,
        "\nexport const {id}_DURATIONS{t1} = [{dur}];\n\nexport const {id}{t2} = [\n{fr},\n];",
        id = d.id,
        t1 = ty(": number[]"),
        dur = join(&d.durations, ", "),
        t2 = ty(": number[][][]"),
        fr = frame_rows(d, "  ")
    );
    out
}

fn json(d: &Prepared) -> String {
    let pal = d.entries.iter().map(|(i, c)| format!("    \"{i}\": \"{}\"", hex(*c))).collect::<Vec<_>>().join(",\n");
    let name = d.name.replace('\\', "\\\\").replace('"', "\\\"");
    let mut out = format!("{{\n  \"name\": \"{name}\",\n  \"palette\": \"{}\",\n  \"width\": {},\n  \"height\": {},\n", d.palette_name, d.w, d.h);
    if d.n() == 1 {
        let _ = write!(out, "  \"colors\": {{\n{pal}\n  }},\n  \"grid\": [\n{}\n  ]\n", rows(d, 0, "    "));
    } else {
        let _ = write!(
            out,
            "  \"fps\": {},\n  \"durations\": [{}],\n  \"colors\": {{\n{pal}\n  }},\n  \"frames\": [\n{}\n  ]\n",
            d.fps,
            join(&d.durations, ", "),
            frame_rows(d, "    ")
        );
    }
    out + "}"
}

fn game(d: &Prepared, materials: &BTreeMap<u16, String>) -> String {
    let mut colors: Vec<Rgb> = d.palette.clone();
    let mut free: Vec<(u16, Rgb)> = d.entries.iter().copied().filter(|e| e.0 > d.max_idx).collect();
    free.sort();
    colors.extend(free.iter().map(|e| e.1));
    let name = game_name(&deumlaut(&d.name));
    let (w, n) = (d.w as usize, d.n());
    // Mehrere Frames liegen als Streifen nebeneinander in `data`.
    let width = w * n;
    let mut data = Vec::with_capacity(width * d.h as usize);
    for y in 0..d.h as usize {
        for f in 0..n {
            data.extend_from_slice(&d.frames[f][y * w..(y + 1) * w]);
        }
    }
    let q = |s: &str| serde_json::to_string(s).expect("Text");
    let mut lines = vec![
        "{".to_string(),
        "  \"version\": 1,".into(),
        format!("  \"name\": {},", q(&name)),
        format!("  \"width\": {width},"),
        format!("  \"height\": {},", d.h),
        "  \"palette\": [".into(),
    ];
    let mut pal = vec![format!("    {{ \"color\": {}, \"material\": {} }}", q("#00000000"), q("empty"))];
    for (k, c) in colors.iter().enumerate() {
        let m = materials.get(&(k as u16 + 1)).map(String::as_str).filter(|m| MATERIALS.contains(m)).unwrap_or(DEFAULT_MATERIAL);
        pal.push(format!("    {{ \"color\": {}, \"material\": {} }}", q(&format!("{}ff", hex(*c))), q(m)));
    }
    lines.push(pal.join(",\n"));
    lines.push("  ],".into());
    lines.push("  \"data\": [".into());
    lines.push(data.chunks(width).map(|r| format!("    {}", join(r, ", "))).collect::<Vec<_>>().join(",\n"));
    if n > 1 {
        lines.push("  ],".into());
        lines.push("  \"sprites\": {".into());
        lines.push(format!("    {}: {{ \"x\": 0, \"y\": 0, \"w\": {w}, \"h\": {}, \"frames\": {n} }}", q(&name), d.h));
        lines.push("  },".into());
        lines.push(format!("  \"durations\": [{}]", join(&d.durations, ", ")));
    } else {
        lines.push("  ]".into());
    }
    lines.push("}".into());
    lines.join("\n") + "\n"
}

/// Waagerechte Läufe gleicher Farbe als ein Rechteck.
fn svg_rects(d: &Prepared, f: usize, indent: &str) -> Vec<String> {
    let mut parts = Vec::new();
    for y in 0..d.h {
        let mut x = 0;
        while x < d.w {
            let Some(col) = d.color_at(x, y, f) else {
                x += 1;
                continue;
            };
            let mut len = 1;
            while x + len < d.w && d.color_at(x + len, y, f).as_deref() == Some(col.as_str()) {
                len += 1;
            }
            parts.push(format!("{indent}<rect x=\"{x}\" y=\"{y}\" width=\"{len}\" height=\"1\" fill=\"{col}\"/>"));
            x += len;
        }
    }
    parts
}

/// Prozent mit höchstens zwei Stellen, wie `Math.round(v * 10000) / 100`.
fn pct(v: f64) -> String {
    let p = (v * 10000.0).round() / 100.0;
    format!("{p}")
}

fn svg(d: &Prepared) -> String {
    let open = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{ww}\" height=\"{hh}\" shape-rendering=\"crispEdges\">\n",
        w = d.w,
        h = d.h,
        ww = d.w * 8,
        hh = d.h * 8
    );
    if d.n() == 1 {
        return format!("<!-- {} — {}×{} -->\n{open}{}\n</svg>", d.name, d.w, d.h, svg_rects(d, 0, "  ").join("\n"));
    }
    let cls = slug(&d.name);
    let total: u32 = d.durations.iter().sum();
    let (mut styles, mut groups) = (Vec::new(), Vec::new());
    let mut at = 0u32;
    for (i, &ms) in d.durations.iter().enumerate() {
        let a = (at as f64 * 10000.0 / total as f64).round() / 100.0;
        let b = ((at + ms) as f64 * 10000.0 / total as f64).round() / 100.0;
        at += ms;
        let name = format!("{cls}-f{}", i + 1);
        let mut steps = Vec::new();
        if a > 0.0 {
            steps.push("0%{visibility:hidden}".to_string());
        }
        steps.push(format!("{a}%{{visibility:visible}}"));
        steps.push(if b < 100.0 { format!("{b}%{{visibility:hidden}}") } else { "100%{visibility:visible}".into() });
        styles.push(format!("    .{name}{{animation:{name} {total}ms step-end infinite}}@keyframes {name}{{{}}}", steps.join("")));
        groups.push(format!(
            "  <g class=\"{name}\" data-ms=\"{ms}\"{}>\n{}\n  </g>",
            if i > 0 { " visibility=\"hidden\"" } else { "" },
            svg_rects(d, i, "    ").join("\n")
        ));
    }
    format!(
        "<!-- {} — {}×{} · {} Frames, {total} ms -->\n{open}  <style>\n{}\n  </style>\n{}\n</svg>",
        d.name,
        d.w,
        d.h,
        d.n(),
        styles.join("\n"),
        groups.join("\n")
    )
}

fn css_shadows(d: &Prepared, f: usize, indent: &str) -> String {
    let mut s = Vec::new();
    for y in 0..d.h {
        for x in 0..d.w {
            if let Some(c) = d.color_at(x, y, f) {
                s.push(format!("{indent}{x}px {y}px 0 {c}"));
            }
        }
    }
    if s.is_empty() {
        " none".into()
    } else {
        format!("\n{}", s.join(",\n"))
    }
}

fn css(d: &Prepared, lang: CodeLang) -> String {
    let cls = slug(&d.name);
    let total: u32 = d.durations.iter().sum();
    let mut out = txt(
        lang,
        "/* {name} — {w}×{h}. Benutzung: <div class=\"{cls}\"></div>",
        "/* {name} — {w}×{h}. Usage: <div class=\"{cls}\"></div>",
        &[("name", d.name.clone()), ("w", d.w.to_string()), ("h", d.h.to_string()), ("cls", cls.clone())],
    );
    out.push('\n');
    out += &txt(
        lang,
        "   Ein einziges 1×1-Element, hochskaliert. --px stellt die Pixelgröße. */",
        "   A single 1×1 element, scaled up. --px sets the pixel size. */",
        &[],
    );
    out.push('\n');
    if d.n() > 1 {
        out += &txt(
            lang,
            "/* Animation: {n} Frames, {ms} ms pro Durchlauf, läuft endlos. */",
            "/* Animation: {n} frames, {ms} ms per loop, runs forever. */",
            &[("n", d.n().to_string()), ("ms", total.to_string())],
        );
        out.push('\n');
    }
    let margin = txt(lang, "/* Platz für die Skalierung */", "/* room for the scaling */", &[]);
    let _ = write!(
        out,
        ".{cls} {{\n  --px: 8;\n  width: 1px;\n  height: 1px;\n  transform: scale(var(--px));\n  transform-origin: 0 0;\n  margin: 0 {}px {}px 0; {margin}\n",
        d.w - 1,
        d.h - 1
    );
    if d.n() > 1 {
        let _ = writeln!(out, "  animation: {cls}-anim {total}ms step-end infinite;");
    }
    out += &format!("  box-shadow:{};\n}}", css_shadows(d, 0, "    "));
    if d.n() == 1 {
        return out;
    }
    let mut at = 0u32;
    let keys: Vec<String> = d
        .durations
        .iter()
        .enumerate()
        .map(|(i, &ms)| {
            let k = format!("  {}% {{ box-shadow:{}; }}", pct(at as f64 / total as f64), css_shadows(d, i, "      "));
            at += ms;
            k
        })
        .collect();
    out + &format!("\n@keyframes {cls}-anim {{\n{}\n}}", keys.join("\n"))
}

fn c_header(d: &Prepared, lang: CodeLang) -> String {
    let guard = format!("{}_H", d.id);
    let max_used = d.entries.iter().map(|e| e.0).max().unwrap_or(0);
    let pal_arr: Vec<String> = (0..=max_used)
        .map(|i| match d.color.get(&i) {
            Some(c) => format!("0x{}", hex(*c)[1..].to_uppercase()),
            None => "0x000000".into(),
        })
        .collect();
    let body = |f: usize, ind: &str| {
        d.rows(f).map(|r| format!("{ind}{},", r.iter().map(|v| format!("{v:>2}")).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join("\n")
    };
    let cell = if max_used > 255 { "uint16_t" } else { "uint8_t" };
    let px = d.w * d.h;
    let data = if d.n() == 1 {
        format!("static const {cell} {}_DATA[{px}] = {{\n{}\n}};\n\n", d.id, body(0, "  "))
    } else {
        format!(
            "#define {id}_FRAMES {n}\n\nstatic const uint16_t {id}_DURATIONS[{n}] = {{ {dur} }};\n\nstatic const {cell} {id}_DATA[{n}][{px}] = {{\n{fr}\n}};\n\n",
            id = d.id,
            n = d.n(),
            dur = join(&d.durations, ", "),
            fr = (0..d.n()).map(|f| format!("  {{\n{}\n  }},", body(f, "    "))).collect::<Vec<_>>().join("\n")
        )
    };
    let mut out = txt(
        lang,
        "// {name} — {w}×{h}, {n} Farben",
        "// {name} — {w}×{h}, {n} colors",
        &[("name", d.name.clone()), ("w", d.w.to_string()), ("h", d.h.to_string()), ("n", d.entries.len().to_string())],
    );
    out.push('\n');
    out += &txt(lang, "// Index 0 ist transparent; Farben als 0xRRGGBB.", "// index 0 is transparent; colors as 0xRRGGBB.", &[]);
    out.push('\n');
    if d.n() > 1 {
        out += &txt(
            lang,
            "// Animation: {n} Frames — {id}_DATA[Frame][y * WIDTH + x], Dauer in ms: {id}_DURATIONS",
            "// Animation: {n} frames — {id}_DATA[frame][y * WIDTH + x], duration in ms: {id}_DURATIONS",
            &[("n", d.n().to_string()), ("id", d.id.clone())],
        );
        out.push('\n');
    }
    let _ = write!(
        out,
        "#ifndef {guard}\n#define {guard}\n\n#include <stdint.h>\n\n#define {id}_WIDTH  {w}\n#define {id}_HEIGHT {h}\n\nstatic const uint32_t {id}_PALETTE[{np}] = {{\n  {pal}\n}};\n\n{data}#endif // {guard}\n",
        id = d.id,
        w = d.w,
        h = d.h,
        np = pal_arr.len(),
        pal = pal_arr.join(", ")
    );
    out
}

fn python(d: &Prepared, lang: CodeLang) -> String {
    let pal = d.entries.iter().map(|(i, c)| format!("    {i}: \"{}\",", hex(*c))).collect::<Vec<_>>().join("\n");
    let body = |f: usize, ind: &str| d.rows(f).map(|r| format!("{ind}[{}],", join(r, ", "))).collect::<Vec<_>>().join("\n");
    let head = txt(
        lang,
        "# {name} — {w}×{h}. Index 0 ist transparent.",
        "# {name} — {w}×{h}. Index 0 is transparent.",
        &[("name", d.name.clone()), ("w", d.w.to_string()), ("h", d.h.to_string())],
    ) + "\n";
    if d.n() == 1 {
        return format!("{head}{id}_PALETTE = {{\n{pal}\n}}\n\n{id} = [\n{}\n]\n", body(0, "    "), id = d.id);
    }
    let anim = txt(
        lang,
        "# Animation: {n} Frames — {id}[Frame][y][x], Dauer je Frame in ms: {id}_DURATIONS",
        "# Animation: {n} frames — {id}[frame][y][x], duration per frame in ms: {id}_DURATIONS",
        &[("n", d.n().to_string()), ("id", d.id.clone())],
    );
    format!(
        "{head}{anim}\n{id}_PALETTE = {{\n{pal}\n}}\n\n{id}_DURATIONS = [{dur}]\n\n{id} = [\n{fr}\n]\n",
        id = d.id,
        dur = join(&d.durations, ", "),
        fr = (0..d.n()).map(|f| format!("    [\n{}\n    ],", body(f, "        "))).collect::<Vec<_>>().join("\n")
    )
}

pub const TXT_CHARS: &str = ".123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

fn txt_char(v: u16) -> char {
    TXT_CHARS.chars().nth(v as usize).unwrap_or('?')
}

fn text(d: &Prepared, lang: CodeLang) -> String {
    let raster = |f: usize| d.rows(f).map(|r| r.iter().map(|&v| txt_char(v)).collect::<String>()).collect::<Vec<_>>().join("\n");
    let legend = d.entries.iter().map(|(i, c)| format!("  {} = {}  (Index {i})", txt_char(*i), hex(*c))).collect::<Vec<_>>().join("\n");
    let body = if d.n() == 1 {
        raster(0)
    } else {
        (0..d.n()).map(|f| format!("Frame {} · {} ms\n{}", f + 1, d.durations[f], raster(f))).collect::<Vec<_>>().join("\n\n")
    };
    let frames = if d.n() > 1 { format!(" · {} Frames", d.n()) } else { String::new() };
    format!("{} — {}×{}{frames}\n\n{body}\n\n{}\n{legend}\n", d.name, d.w, d.h, txt(lang, "Legende ('.' = transparent):", "Key ('.' = transparent):", &[]))
}

/// Text im gewünschten Format. `materials`: Material je Palettennummer
/// (nur „JSON (Spiel)“).
pub fn build(sp: &Sprite, pal: &Palette, format: Format, with_palette: bool, lang: CodeLang, materials: &BTreeMap<u16, String>) -> String {
    let d = prepare(sp, pal);
    match format {
        Format::Ts => js_like(&d, with_palette, true, lang),
        Format::Js => js_like(&d, with_palette, false, lang),
        Format::Json => json(&d),
        Format::Game => game(&d, materials),
        Format::Svg => svg(&d),
        Format::Css => css(&d, lang),
        Format::C => c_header(&d, lang),
        Format::Py => python(&d, lang),
        Format::Txt => text(&d, lang),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namen() {
        assert_eq!(identifier("Held Grün"), "HELD_GRUEN");
        assert_eq!(identifier("3 Äpfel!"), "S_3_AEPFEL");
        assert_eq!(identifier(""), "SPRITE");
        assert_eq!(slug("Held Grün"), "held-gruen");
        assert_eq!(slug("Café"), "cafe");
        assert_eq!(game_name("Berg Hintergrund"), "berg_hintergrund");
        assert_eq!(filename(Format::Ts, "Held Grün"), "HELD_GRUEN.ts");
        assert_eq!(filename(Format::Svg, "Held Grün"), "held-gruen.svg");
        assert_eq!(filename(Format::Game, "Held Grün"), "held_gruen.json");
    }

    #[test]
    fn prozent_wie_im_web() {
        assert_eq!(pct(0.0), "0");
        assert_eq!(pct(0.5), "50");
        assert_eq!(pct(1.0 / 3.0), "33.33");
    }
}
