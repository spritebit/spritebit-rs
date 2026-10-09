//! Code-Ausgaben gegen die Web-Version.
//!
//! `fixtures/codegen/*.txt` sind nicht von Hand geschrieben: sie stammen aus
//! dem Codegenerator der Web-Version (`fixtures/gen-codegen.mjs`, unter Node
//! auf `fixtures/web-projekt.json` angewendet). Jedes Format muss Zeichen für
//! Zeichen gleich herauskommen.

use std::collections::BTreeMap;

use spritebit_core::codegen::{build, CodeLang, Format};
use spritebit_core::{codeimport, import_web, selection::rgb_of};

const WEB: &str = include_str!("fixtures/web-projekt.json");

fn key(f: Format) -> &'static str {
    match f {
        Format::Ts => "ts",
        Format::Js => "js",
        Format::Json => "json",
        Format::Game => "game",
        Format::Svg => "svg",
        Format::Css => "css",
        Format::C => "c",
        Format::Py => "py",
        Format::Txt => "txt",
    }
}

#[test]
fn alle_formate_wie_im_web() {
    let p = import_web(WEB).unwrap();
    let sp = &p.sprites[0];
    let pal = p.palette(&sp.palette);
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/codegen");
    let mut checked = 0;
    for f in Format::ALL {
        for (lang, l) in [(CodeLang::De, "de"), (CodeLang::En, "en")] {
            for with_pal in [false, true] {
                if with_pal && !f.pal_option() {
                    continue;
                }
                let name = format!("{}{}.{l}.txt", key(f), if with_pal { "-pal" } else { "" });
                let want = std::fs::read_to_string(format!("{dir}/{name}")).unwrap().replace("\r\n", "\n");
                let got = build(sp, &pal, f, with_pal, lang, &BTreeMap::new());
                assert_eq!(got, want, "{name}");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 22);
}

/// Jede Ausgabe der Web-Version kommt wieder herein — mit denselben Farben
/// an denselben Stellen, allen Frames und ihrer Dauer.
#[test]
fn alle_formate_wieder_einlesen() {
    let p = import_web(WEB).unwrap();
    let sp = &p.sprites[0];
    let pal = p.palette(&sp.palette);
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/codegen");
    // Was man sieht, je Frame: Farbe je Pixel (halbdurchsichtige Ebene gemischt).
    let seen = |s: &spritebit_core::Sprite, pal: &spritebit_core::Palette| -> Vec<Vec<Option<[u8; 3]>>> {
        (0..s.frames.len())
            .map(|f| {
                let rgba = spritebit_core::export::frame_rgba(s, pal, f);
                rgba.chunks(4).map(|c| (c[3] > 0).then(|| [c[0], c[1], c[2]])).collect()
            })
            .collect()
    };
    let want = seen(sp, &pal);
    for f in Format::ALL {
        let name = format!("{}.de.txt", key(f));
        let text = std::fs::read_to_string(format!("{dir}/{name}")).unwrap();
        let imp = codeimport::parse(&text).unwrap_or_else(|e| panic!("{name}: {}", e.german()));
        assert_eq!((imp.stats.w, imp.stats.h, imp.stats.frames), (6, 4, 3), "{name}");
        assert_eq!(imp.durations, Some(vec![100, 150, 100]), "{name}");
        // Palette aus dem Text, sonst die des Sprites.
        let ipal = match &imp.palette {
            Some(m) => {
                let n = *m.keys().max().unwrap() as usize;
                spritebit_core::Palette::new("i", (1..=n).map(|i| m.get(&(i as u16)).copied().unwrap_or([0, 0, 0])).collect())
            }
            None => pal.clone(),
        };
        let back = codeimport::to_sprite(&imp, "x", "i").unwrap();
        let got = seen(&back, &ipal);
        assert_eq!(got, want, "{name}");
        let _ = rgb_of;
    }
}
