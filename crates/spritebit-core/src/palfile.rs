//! Paletten als Datei laden und speichern — wie `palfile.js` der Web-Version.
//!
//! Formate, wie man sie z. B. auf Lospec herunterlädt:
//! * `.gpl` GIMP/Aseprite/Krita — Zeilen „R G B Name“, Farbnamen kommen mit
//! * `.hex` Lospec — eine Farbe je Zeile, „rrggbb“
//! * `.pal` JASC-PAL (Text) oder RIFF-PAL (binär, Microsoft)
//! * `.txt` Paint.NET — „AARRGGBB“ je Zeile, „;“ leitet Kommentare ein
//! * Bild — jede neue deckende Farbe von links oben nach rechts unten
//!   (das Dekodieren macht die App, hier nur [`colors_from_rgba`])
//!
//! Gespeichert wird als `.gpl`, `.hex`, `.pal` (JASC) oder PNG (eine Zeile,
//! 1 px je Farbe, [`rgba_strip`]).

use crate::palette::{Rgb, MAX_COLORS};

/// Inhalt einer Palettendatei.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PalFile {
    pub colors: Vec<Rgb>,
    /// Farbnamen (gleich lang wie `colors`), `None` ohne Namen.
    pub names: Vec<Option<String>>,
    /// Name der Palette aus der Datei (nur `.gpl`).
    pub name: Option<String>,
}

/// Endung einer Datei, klein, ohne Punkt.
pub fn ext_of(filename: &str) -> String {
    filename.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or_default()
}

/// Bilddatei, die die App dekodieren soll?
pub fn is_image_file(filename: &str) -> bool {
    matches!(ext_of(filename).as_str(), "png" | "gif" | "bmp" | "jpg" | "jpeg" | "webp")
}

/// Palettendatei lesen. `None`, wenn keine Farben darin sind.
pub fn parse(filename: &str, bytes: &[u8]) -> Option<PalFile> {
    // RIFF-PAL erkennt man am Kopf, egal wie die Datei heißt.
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"PAL " {
        return finish(parse_riff(bytes));
    }
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    let head = text.trim_start().chars().take(16).collect::<String>().to_ascii_uppercase();
    let ext = ext_of(filename);
    let p = if head.starts_with("GIMP PALETTE") || ext == "gpl" {
        parse_gpl(text)
    } else if head.starts_with("JASC-PAL") {
        parse_jasc(text)
    } else if ext == "txt" {
        parse_paint_net(text)
    } else {
        parse_hex_list(text)
    };
    finish(p)
}

fn finish(mut p: PalFile) -> Option<PalFile> {
    if p.colors.is_empty() {
        return None;
    }
    p.colors.truncate(MAX_COLORS);
    p.names.truncate(MAX_COLORS);
    Some(p)
}

fn plain(colors: Vec<Rgb>) -> PalFile {
    PalFile { names: vec![None; colors.len()], colors, name: None }
}

/// Drei Zahlen 0–255 am Zeilenanfang, dazu der Rest der Zeile.
fn rgb_line(line: &str) -> Option<(Rgb, &str)> {
    let mut rest = line;
    let mut c = [0u8; 3];
    for v in &mut c {
        rest = rest.trim_start();
        let end = rest.find(|ch: char| !ch.is_ascii_digit()).unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        *v = rest[..end].parse::<u8>().ok()?;
        rest = &rest[end..];
    }
    Some((c, rest.trim()))
}

fn parse_gpl(text: &str) -> PalFile {
    let (mut colors, mut names, mut name) = (Vec::new(), Vec::new(), None);
    for raw in text.lines() {
        let line = raw.trim();
        let lower = line.to_ascii_lowercase();
        if line.is_empty() || line.starts_with('#') || lower.starts_with("gimp palette") || lower.starts_with("columns:") {
            continue;
        }
        if lower.starts_with("name:") {
            let n = line[5..].trim();
            name = (!n.is_empty()).then(|| n.to_string());
            continue;
        }
        let Some((c, label)) = rgb_line(line) else { continue };
        colors.push(c);
        // GIMP schreibt „Untitled“ oder den Hex-Wert, wenn es keinen Namen gibt.
        let hexish = label.trim_start_matches('#').len() == 6 && label.trim_start_matches('#').chars().all(|ch| ch.is_ascii_hexdigit());
        names.push((!label.is_empty() && !label.eq_ignore_ascii_case("untitled") && !hexish).then(|| label.to_string()));
    }
    PalFile { colors, names, name }
}

fn parse_jasc(text: &str) -> PalFile {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    plain(lines.iter().skip(3).filter_map(|l| rgb_line(l).map(|(c, _)| c)).collect())
}

fn hex6(s: &str) -> Option<Rgb> {
    if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some([b(0)?, b(2)?, b(4)?])
}

fn parse_paint_net(text: &str) -> PalFile {
    plain(
        text.lines()
            .filter_map(|raw| {
                let line = raw.split(';').next().unwrap_or("").trim();
                match line.len() {
                    8 => hex6(&line[2..]).filter(|_| line[..2].chars().all(|c| c.is_ascii_hexdigit())),
                    6 => hex6(line),
                    _ => None,
                }
            })
            .collect(),
    )
}

/// Jedes „rrggbb“ bzw. „#rrggbb“, das für sich steht (Lospec .hex und Ähnliches).
fn parse_hex_list(text: &str) -> PalFile {
    let re = regex::Regex::new(r"(?i)(?:^|[^0-9a-z#])#?([0-9a-f]{6})(?:$|[^0-9a-z])").expect("gültig");
    let mut colors = Vec::new();
    for line in text.lines() {
        // Zeilenweise: so stören sich Treffer am Zeilenende nicht gegenseitig.
        let mut at = 0;
        while let Some(m) = re.captures_at(line, at) {
            let g = m.get(1).expect("Gruppe 1");
            if let Some(c) = hex6(g.as_str()) {
                colors.push(c);
            }
            at = g.end();
        }
    }
    plain(colors)
}

fn parse_riff(b: &[u8]) -> PalFile {
    let mut colors = Vec::new();
    let mut at = 12;
    while at + 8 <= b.len() {
        let size = u32::from_le_bytes([b[at + 4], b[at + 5], b[at + 6], b[at + 7]]) as usize;
        if &b[at..at + 4] == b"data" {
            if at + 12 > b.len() {
                break;
            }
            let n = u16::from_le_bytes([b[at + 10], b[at + 11]]) as usize;
            for i in 0..n {
                let p = at + 12 + i * 4;
                if p + 3 > b.len() {
                    break;
                }
                colors.push([b[p], b[p + 1], b[p + 2]]);
            }
            break;
        }
        at += 8 + size + (size & 1);
    }
    plain(colors)
}

/// Farben aus RGBA-Pixeln (Bild): jede neue deckende Farbe in Lese-Reihenfolge.
pub fn colors_from_rgba(data: &[u8]) -> Option<PalFile> {
    let mut colors: Vec<Rgb> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for px in data.as_chunks::<4>().0 {
        if colors.len() == MAX_COLORS {
            break;
        }
        if px[3] < 128 {
            continue;
        }
        let c = [px[0], px[1], px[2]];
        if seen.insert(c) {
            colors.push(c);
        }
    }
    finish(plain(colors))
}

fn hex(c: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// GIMP-Palette schreiben (mit Farbnamen, wo es welche gibt).
pub fn write_gpl(name: &str, colors: &[Rgb], names: &[Option<String>]) -> String {
    let mut out = format!("GIMP Palette\nName: {name}\nColumns: {}\n#\n", colors.len().min(16));
    for (i, &c) in colors.iter().enumerate() {
        let label = names.get(i).cloned().flatten().unwrap_or_else(|| hex(c));
        out += &format!("{:>3} {:>3} {:>3}\t{label}\n", c[0], c[1], c[2]);
    }
    out
}

/// Lospec-.hex schreiben.
pub fn write_hex(colors: &[Rgb]) -> String {
    colors.iter().map(|&c| hex(c) + "\n").collect()
}

/// JASC-PAL schreiben (Zeilenende CRLF wie im Original).
pub fn write_jasc(colors: &[Rgb]) -> String {
    let mut out = format!("JASC-PAL\r\n0100\r\n{}\r\n", colors.len());
    for c in colors {
        out += &format!("{} {} {}\r\n", c[0], c[1], c[2]);
    }
    out
}

/// Farben als RGBA-Zeile für ein PNG (Breite = Anzahl, Höhe 1).
pub fn rgba_strip(colors: &[Rgb]) -> Vec<u8> {
    colors.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gimp_palette_mit_name_und_farbnamen() {
        let gpl = "GIMP Palette\nName: Wald\nColumns: 4\n#\n# Kommentar\n  0  0   0\tSchwarz\n255 128 0 Untitled\n 18 52 86\t123456\n";
        let p = parse("wald.gpl", gpl.as_bytes()).unwrap();
        assert_eq!(p.colors, vec![[0, 0, 0], [255, 128, 0], [18, 52, 86]]);
        assert_eq!(p.names, vec![Some("Schwarz".into()), None, None]);
        assert_eq!(p.name.as_deref(), Some("Wald"));
    }

    #[test]
    fn lospec_hex_jasc_und_paint_net() {
        assert_eq!(parse("x.hex", b"ff0000\r\n00FF00\n\n0000ff\n").unwrap().colors, vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]]);
        assert_eq!(parse("x.pal", b"JASC-PAL\r\n0100\r\n2\r\n255 0 0\r\n0 0 255\r\n").unwrap().colors, vec![[255, 0, 0], [0, 0, 255]]);
        assert_eq!(parse("x.txt", "; paint.net\nFFFF0000\nff00ff00 ; grün\n".as_bytes()).unwrap().colors, vec![[255, 0, 0], [0, 255, 0]]);
    }

    #[test]
    fn riff_pal_am_kopf_erkannt() {
        let data = [0x00, 0x03, 2, 0, 255, 0, 0, 0, 0, 0, 255, 0];
        let size = (4 + 8 + data.len()) as u8;
        let mut b = b"RIFF".to_vec();
        b.extend([size, 0, 0, 0]);
        b.extend(b"PAL data");
        b.extend([data.len() as u8, 0, 0, 0]);
        b.extend(data);
        assert_eq!(parse("egal.pal", &b).unwrap().colors, vec![[255, 0, 0], [0, 0, 255]]);
    }

    #[test]
    fn ohne_farben_none_und_bild_in_lesereihenfolge() {
        assert!(parse("leer.hex", b"nichts hier").is_none());
        let px = [255, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255, 0, 0, 255, 255];
        assert_eq!(colors_from_rgba(&px).unwrap().colors, vec![[255, 0, 0], [0, 0, 255]]);
    }

    #[test]
    fn schreiben_und_wieder_lesen() {
        let colors = vec![[0, 0, 0], [255, 128, 0], [18, 52, 86]];
        let names = vec![Some("Schwarz".to_string()), None, None];
        let p = parse("a.gpl", write_gpl("Wald", &colors, &names).as_bytes()).unwrap();
        assert_eq!((p.colors.clone(), p.names, p.name.as_deref()), (colors.clone(), names, Some("Wald")));
        assert_eq!(parse("a.hex", write_hex(&colors).as_bytes()).unwrap().colors, colors);
        assert_eq!(parse("a.pal", write_jasc(&colors).as_bytes()).unwrap().colors, colors);
        assert_eq!(colors_from_rgba(&rgba_strip(&colors)).unwrap().colors, colors);
        // Gleiches Format wie im Web: dieselbe .gpl-Zeile.
        assert!(write_gpl("x", &[[1, 2, 3]], &[None]).ends_with("  1   2   3\t010203\n"));
    }

    #[test]
    fn endungen() {
        assert_eq!(ext_of("Meine.GPL"), "gpl");
        assert!(is_image_file("lospec-1x.png"));
        assert!(!is_image_file("a.hex"));
    }
}
