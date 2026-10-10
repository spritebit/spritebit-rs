//! Austausch mit der Web-Version — beide lesen und schreiben dieselben
//! Sprite-Dateien (.bitty, JSON).
//!
//! In `tests/interop/` (Repo-Wurzel) liegen zwei Beispieldateien, in BEIDEN
//! Repos gleich:
//!   desktop-sprite.bitty  schreibt diese App (Test 1 prüft das)
//!   web-sprite.bitty      schreibt die Web-Version (sprites-editor, tests/interop.test.js)
//! Jede Seite liest die Datei der anderen und prüft denselben Inhalt
//! (`expect_sample`, im Web `expectSample`).
//!
//! Ändert sich das Format absichtlich:
//!   UPDATE_INTEROP=1 cargo test -p spritebit-core --test interop
//!   python tools/sync_interop.py   (im Web-Repo; deploy.py prüft den Abgleich)

use std::path::PathBuf;

use serde_json::Value;
use spritebit_core::{export_sprite, import_web, Direction, Palette, Project, Sprite, Tag, FREE_BASE};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(dir().join(name)).unwrap_or_else(|e| panic!("tests/interop/{name}: {e}"))
}

const PAL: [[u8; 3]; 5] = [[0x1b, 0x2a, 0x1e], [0x2f, 0x5a, 0x33], [0x5d, 0x96, 0x42], [0x9c, 0xc7, 0x5a], [0xe8, 0xf5, 0xb0]];

/// Der Beispiel-Sprite — genau wie `buildSample` in der Web-Version:
/// 6 × 4 Pixel, Palette „interop“ mit Namen und einem Material, zwei Ebenen:
///   „Hinten“ (durchgehend): Boden in Zeile 3, Glanzpunkt oben links —
///                           Frame 2 und 3 mit Frame 1 verknüpft
///   „Vorn“ (gesperrt, halb durchsichtig): ein Punkt, der nach rechts
///                           wandert; in Frame 3 zusätzlich eine freie Farbe
/// Frame 2 dauert 200 ms, ein Tag „lauf“ über alle drei Frames, Ping-Pong.
fn sample() -> Project {
    let mut sp = Sprite::new("held", 6, 4).unwrap();
    sp.palette = "interop".into();
    sp.fps = 12;
    sp.layers[0].name = "Hinten".into();
    sp.layers[0].continuous = true;
    sp.add_layer(1, "Vorn");
    sp.layers[1].locked = true;
    sp.layers[1].opacity = 0.5;
    for x in 0..6 {
        sp.cel_mut(0, 0).set(x, 3, 3);
    }
    sp.cel_mut(0, 0).set(0, 0, 5);
    sp.add_frame(0, false);
    sp.add_frame(1, false);
    let back = sp.frames[0].cels[0];
    for f in 0..3 {
        sp.frames[f].cels[0] = back;
        sp.cel_mut(f, 1).set(f as u32 + 1, 1, 1);
    }
    let free = sp.free_color([0xab, 0xcd, 0xef]);
    sp.cel_mut(2, 1).set(5, 3, free);
    sp.frames[1].duration_ms = 200;
    sp.tags.push(Tag { name: "lauf".into(), from: 0, to: 2, color: [0xe5, 0x53, 0x4b], direction: Direction::PingPong });
    sp.frame = 0;
    sp.layer = 0;
    let mut pal = Palette::new("interop", PAL.to_vec());
    pal.names.insert(1, "Kontur".into());
    pal.names.insert(3, "Grund".into());
    let mut p = Project { name: String::new(), sprites: vec![sp], palettes: vec![pal], current: 0, materials: Default::default() };
    p.materials.insert("interop".into(), [(3u16, "stone".to_string())].into_iter().collect());
    p
}

/// Was beide Seiten aus der Datei der anderen herauslesen müssen.
fn expect_sample(p: &Project) {
    assert_eq!(p.sprites.len(), 1);
    let s = &p.sprites[0];
    assert_eq!((s.name.as_str(), s.width, s.height, s.fps), ("held", 6, 4, 12));
    assert_eq!(s.frames.iter().map(|f| f.duration_ms).collect::<Vec<_>>(), [0, 200, 0], "Dauer je Frame");
    let layers: Vec<_> = s.layers.iter().map(|l| (l.name.as_str(), l.visible, l.locked, l.opacity, l.continuous)).collect();
    assert_eq!(layers, [("Hinten", true, false, 1.0, true), ("Vorn", true, true, 0.5, false)], "Ebenen");
    assert!(s.is_linked(1, 0) && s.frames[1].cels[0] == s.frames[0].cels[0] && s.frames[2].cels[0] == s.frames[0].cels[0], "verknüpft");
    assert_ne!(s.frames[1].cels[1], s.frames[0].cels[1], "vordere Ebene nicht verknüpft");
    assert_eq!((0..6).map(|x| s.cel(0, 0).get(x, 3)).collect::<Vec<_>>(), [3; 6]);
    assert_eq!(s.cel(0, 0).get(0, 0), 5);
    for f in 0..3 {
        assert_eq!(s.cel(f, 1).get(f as u32 + 1, 1), 1, "Punkt in Frame {}", f + 1);
    }
    let free = s.cel(2, 1).get(5, 3);
    assert!(free >= FREE_BASE, "freie Farbe");
    assert_eq!(s.free[(free - FREE_BASE) as usize], [0xab, 0xcd, 0xef]);
    assert_eq!(s.cel(1, 1).get(5, 3), 0);
    let tags: Vec<_> = s.tags.iter().map(|t| (t.name.as_str(), t.from, t.to, t.color, t.direction)).collect();
    assert_eq!(tags, [("lauf", 0, 2, [0xe5, 0x53, 0x4b], Direction::PingPong)]);
    let pal = p.palette(&s.palette);
    assert_eq!(pal.colors, PAL.to_vec(), "Palettenfarben");
    assert_eq!(pal.names.iter().map(|(i, n)| (*i, n.as_str())).collect::<Vec<_>>(), [(1, "Kontur"), (3, "Grund")], "Farbnamen");
    let mats = p.materials.get(&s.palette).expect("Materialien");
    assert_eq!(mats.iter().map(|(i, m)| (*i, m.as_str())).collect::<Vec<_>>(), [(3, "stone")], "Materialien");
}

#[test]
fn desktop_schreibt_wie_desktop_sprite_bitty() {
    let text = export_sprite(&sample(), 0);
    let got: Value = serde_json::from_str(&text).unwrap();
    if std::env::var_os("UPDATE_INTEROP").is_some() {
        std::fs::write(dir().join("desktop-sprite.bitty"), serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
    }
    let want: Value = serde_json::from_str(&read("desktop-sprite.bitty")).unwrap();
    assert_eq!(got, want, "Format geändert? Absichtlich: UPDATE_INTEROP=1 cargo test -p spritebit-core --test interop, dann tools/sync_interop.py im Web-Repo");
}

#[test]
fn desktop_liest_die_eigene_beispieldatei_wieder() {
    expect_sample(&import_web(&read("desktop-sprite.bitty")).expect("lesbar"));
}

#[test]
fn desktop_liest_die_sprite_datei_der_web_version() {
    expect_sample(&import_web(&read("web-sprite.bitty")).expect("Web-Datei lesbar"));
}
