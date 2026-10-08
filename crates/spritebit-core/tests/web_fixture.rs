//! Verträglichkeit mit der Web-Version.
//!
//! `fixtures/web-projekt.json` ist nicht von Hand geschrieben: sie wurde mit
//! den Funktionen der Web-Version erzeugt (makeSprite, framesForSave — so
//! wie deren „Projekt sichern" schreibt). Wenn dieser Test bricht, hat sich
//! eines der beiden Formate verschoben.

use spritebit_core::{export_web, import_web, load_native, save_native, Direction, FREE_BASE};

const WEB: &str = include_str!("fixtures/web-projekt.json");

#[test]
fn projektdatei_der_web_version_lesen() {
    let p = import_web(WEB).expect("Web-Projekt lesbar");
    assert_eq!(p.sprites.len(), 1);
    let s = &p.sprites[0];
    assert_eq!((s.name.as_str(), s.width, s.height, s.fps), ("Held", 6, 4, 10));
    assert_eq!(s.palette, "golden");
    assert_eq!(s.layers.len(), 2);
    assert!(s.layers[0].continuous);
    assert_eq!(s.layers[1].opacity, 0.5);
    assert_eq!(s.frames.len(), 3);
    assert_eq!(s.frames[1].duration_ms, 150);
    // Boden (Ebene 0) ist in allen drei Frames dasselbe Bild.
    assert!(s.is_linked(1, 0) && s.is_linked(2, 0));
    assert_eq!(s.cel(2, 0).get(0, 3), 5);
    assert_eq!(s.cel(0, 1).get(1, 1), 2);
    let free = s.cel(1, 1).get(2, 1);
    assert!(free >= FREE_BASE);
    assert_eq!(s.free[(free - FREE_BASE) as usize], [0xab, 0xcd, 0xef]);
    assert_eq!(s.tags.len(), 1);
    assert_eq!(s.tags[0].direction, Direction::PingPong);
    assert_eq!((s.tags[0].from, s.tags[0].to), (0, 2));
    // Die eingebaute Palette „golden" ist bekannt.
    assert_eq!(p.current_palette().name, "golden");
    assert!(p.current_palette().len() >= 9);
}

#[test]
fn web_datei_durch_eigenes_format_und_zurueck() {
    let p = import_web(WEB).unwrap();
    let native = load_native(&save_native(&p)).unwrap();
    let again = import_web(&export_web(&native)).unwrap();
    let (a, b) = (&p.sprites[0], &again.sprites[0]);
    for f in 0..a.frames.len() {
        for l in 0..a.layers.len() {
            assert!(a.cel(f, l).same_pixels(b.cel(f, l)), "Frame {f} Ebene {l}");
            assert_eq!(a.is_linked(f, l), b.is_linked(f, l));
        }
    }
    assert_eq!(a.free, b.free);
    assert_eq!(a.tags, b.tags);
    assert_eq!(a.layers, b.layers);
}
