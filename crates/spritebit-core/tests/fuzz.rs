//! Zufallsdaten an die Lader (Projekt, Web/.bitty, Code-Import, Aseprite) — stürzt
//! etwas ab? Läuft nicht bei jedem `cargo test`, nur von Hand:
//!
//!   FUZZ_SEED=123 cargo test --release -p spritebit-core --test fuzz -- --ignored
//!
//! Ein Fund landet als Datei im Temp-Ordner (spritebit-fuzz-*.bin).
use spritebit_core::codegen::{self, CodeLang, Format};
use spritebit_core::io::{export_sprite, export_web, import_web, load_native, save_native};
use spritebit_core::project::Project;
use spritebit_core::{codeimport, Sprite};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn sample_project() -> Project {
    let mut p = Project::default();
    let s = &mut p.sprites[0];
    s.add_layer(1, "oben");
    s.add_frame(0, true);
    for x in 0..20 {
        s.cel_mut(0, 0).set(x, x, 3);
        s.cel_mut(1, 1).set(x, 5, 2);
    }
    s.guides.h = vec![3, 9];
    s.guides.heads = 4;
    let mut s2 = Sprite::new("klein", 5, 3).unwrap();
    s2.cel_mut(0, 0).set(1, 1, 1);
    p.sprites.push(s2);
    p
}

fn mutate(rng: &mut Rng, base: &[u8]) -> Vec<u8> {
    let mut v = base.to_vec();
    for _ in 0..(1 + rng.below(6)) {
        match rng.below(6) {
            0 if !v.is_empty() => {
                let i = rng.below(v.len());
                v[i] = rng.next() as u8;
            }
            1 if !v.is_empty() => {
                let n = rng.below(v.len());
                v.truncate(n);
            }
            2 => {
                let i = rng.below(v.len() + 1);
                v.insert(i, b"0123456789-,.:[]{}\"e9"[rng.below(21)]);
            }
            3 if v.len() > 4 => {
                let i = rng.below(v.len() - 2);
                v.remove(i);
            }
            4 => {
                // eine Zahl durch eine riesige oder negative ersetzen
                let s = String::from_utf8_lossy(&v).to_string();
                if let Some(pos) = s.char_indices().filter(|(_, c)| c.is_ascii_digit()).map(|(i, _)| i).nth(rng.below(50)) {
                    let rep = ["99999999", "-1", "0", "4294967295", "1e300", "-0.5"][rng.below(6)];
                    let mut t = s.clone();
                    t.replace_range(pos..pos + 1, rep);
                    v = t.into_bytes();
                }
            }
            _ => {}
        }
    }
    v
}

fn check(name: &str, input: &[u8], f: impl FnOnce()) -> bool {
    if catch_unwind(AssertUnwindSafe(f)).is_err() {
        let path = std::env::temp_dir().join(format!("spritebit-fuzz-{name}.bin"));
        std::fs::write(&path, input).unwrap();
        eprintln!("ABSTURZ in {name}, Eingabe: {}", path.display());
        return false;
    }
    true
}

#[test]
#[ignore = "von Hand: Zufallstest"]
fn lader_ueberleben_kaputte_eingaben() {
    std::panic::set_hook(Box::new(|_| {}));
    let p = sample_project();
    let native = save_native(&p);
    let web = export_web(&p).into_bytes();
    let one = export_sprite(&p, 0).into_bytes();
    let pal = p.current_palette();
    let codes: Vec<Vec<u8>> = Format::ALL.iter().map(|f| codegen::build(&p.sprites[0], &pal, *f, true, CodeLang::De, &BTreeMap::new()).into_bytes()).collect();
    let ase = spritebit_core::aseprite::write(&p.sprites[0], &pal);
    let mut rng = Rng(std::env::var("FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(0x9E3779B97F4A7C15));
    let mut bad = 0;
    for round in 0..8000 {
        let n = mutate(&mut rng, &native);
        if !check("native", &n, || {
            if let Ok(p) = load_native(&n) {
                let _ = save_native(&p);
                let _ = export_web(&p);
            }
        }) {
            bad += 1;
        }
        let src = if round % 2 == 0 { &web } else { &one };
        let w = mutate(&mut rng, src);
        let ws = String::from_utf8_lossy(&w).to_string();
        if !check("web", &w, || {
            if let Ok(p) = import_web(&ws) {
                let _ = save_native(&p);
                for i in 0..p.sprites.len() {
                    let _ = export_sprite(&p, i);
                }
            }
        }) {
            bad += 1;
        }
        let c = mutate(&mut rng, &codes[round % codes.len()]);
        let cs = String::from_utf8_lossy(&c).to_string();
        if !check(&format!("code-{}", round % codes.len()), &c, || {
            if let Ok(imp) = codeimport::parse(&cs) {
                let _ = codeimport::to_sprite(&imp, "x", "graustufen");
            }
        }) {
            bad += 1;
        }
        let a = mutate(&mut rng, &ase);
        if !check("aseprite", &a, || {
            if let Ok(imp) = spritebit_core::aseprite::read(&a, "x") {
                let _ = spritebit_core::aseprite::write(&imp.sprite, &imp.palette);
            }
        }) {
            bad += 1;
        }
        if bad > 5 {
            break;
        }
    }
    assert_eq!(bad, 0);
}

