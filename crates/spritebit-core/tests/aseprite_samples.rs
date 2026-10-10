//! Echte Aseprite-Dateien lesen (z. B. aus aseprite/tests/sprites) — von Hand:
//!
//!   ASE_SAMPLES=<ordner> cargo test -p spritebit-core --test aseprite_samples -- --ignored --nocapture
//!
//! Jede Datei wird gelesen, wieder geschrieben und noch einmal gelesen; die
//! Pixel müssen gleich bleiben.
use spritebit_core::aseprite::{read, write};

#[test]
#[ignore = "braucht Beispieldateien"]
fn echte_dateien() {
    let Some(dir) = std::env::var_os("ASE_SAMPLES") else { return };
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if !p.extension().is_some_and(|x| x == "aseprite" || x == "ase") {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        let name = p.file_stem().unwrap().to_string_lossy().into_owned();
        let a = match read(&bytes, &name) {
            Ok(a) => a,
            Err(e) => panic!("{name}: {e:?}"),
        };
        let sp = &a.sprite;
        let px: usize = sp.images.iter().map(|i| i.pixels().count()).sum();
        let tilemaps = sp.layers.iter().filter(|l| l.tileset.is_some()).count();
        eprintln!(
            "{name}: {}×{} · {} Frames · Ebenen {:?} · {} Tags · Palette {} · frei {} · {px} Pixel · Tilemaps {tilemaps} · fps {}",
            sp.width,
            sp.height,
            sp.frames.len(),
            sp.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
            sp.tags.len(),
            a.palette.len(),
            sp.free.len(),
            sp.fps
        );
        let again = read(&write(sp, &a.palette), &name).unwrap();
        for f in 0..sp.frames.len() {
            for l in 0..sp.layers.len() {
                for y in 0..sp.height {
                    for x in 0..sp.width {
                        assert_eq!(again.sprite.cel(f, l).get(x, y), sp.cel(f, l).get(x, y), "{name}: Frame {f} Ebene {l} ({x}, {y})");
                    }
                }
            }
        }
        assert_eq!(again.sprite.tags, sp.tags, "{name}: Tags");
        n += 1;
    }
    assert!(n > 0, "keine Dateien gefunden");
}
