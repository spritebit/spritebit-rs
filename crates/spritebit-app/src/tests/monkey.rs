//! Affentest: zufällige Klicks, Ziehen und Tasten — stürzt die App ab?
//! Läuft nicht bei jedem `cargo test`, nur von Hand (MONKEY_STEPS = Anzahl).
//! `MONKEY_SEED=… cargo test -p spritebit-app affentest -- --ignored --nocapture`

use crate::*;
use egui_kittest::Harness;

#[test]
#[ignore = "von Hand: Zufallstest"]
fn affentest() {
    let seed: u64 = std::env::var("MONKEY_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(7);
    let steps: usize = std::env::var("MONKEY_STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
    let mut r = seed.max(1);
    let mut next = move || {
        r ^= r << 13;
        r ^= r >> 7;
        r ^= r << 17;
        r
    };
    let (w, hgt) = (1280.0, 860.0);
    let mut h = Harness::builder().with_size(Vec2::new(w, hgt)).build_eframe(|cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        SpritebitApp::new()
    });
    h.step();
    let keys = [
        Key::B,
        Key::E,
        Key::G,
        Key::H,
        Key::I,
        Key::K,
        Key::L,
        Key::M,
        Key::O,
        Key::P,
        Key::R,
        Key::S,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Num0,
        Key::Num1,
        Key::Num3,
        Key::Num9,
        Key::Delete,
        Key::Backspace,
        Key::Escape,
        Key::Enter,
        Key::ArrowLeft,
        Key::ArrowRight,
        Key::ArrowUp,
        Key::ArrowDown,
        Key::Comma,
        Key::Period,
        Key::Space,
        Key::F11,
    ];
    let mut log: Vec<String> = Vec::new();
    for _ in 0..steps {
        let pos = Pos2::new((next() % w as u64) as f32, (next() % hgt as u64) as f32);
        let choice = next() % 10;
        let what = match choice {
            0..=3 => {
                h.hover_at(pos);
                h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
                h.step();
                h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
                format!("klick {pos:?}")
            }
            4 => {
                h.hover_at(pos);
                h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE });
                h.step();
                h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE });
                format!("rechtsklick {pos:?}")
            }
            5 | 6 => {
                let to = Pos2::new((next() % w as u64) as f32, (next() % hgt as u64) as f32);
                h.hover_at(pos);
                h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
                for k in 1..=4 {
                    h.event(egui::Event::PointerMoved(pos + (to - pos) * (k as f32 / 4.0)));
                    h.step();
                }
                h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
                format!("ziehen {pos:?} -> {to:?}")
            }
            7 | 8 => {
                let k = keys[(next() % keys.len() as u64) as usize];
                h.key_press(k);
                format!("taste {k:?}")
            }
            _ => {
                let k = [Key::Z, Key::Y, Key::C, Key::V, Key::X, Key::A, Key::D][(next() % 7) as usize];
                let shift = next() % 3 == 0;
                let m = if shift { Modifiers::COMMAND | Modifiers::SHIFT } else { Modifiers::COMMAND };
                h.key_press_modifiers(m, k);
                match k {
                    Key::C => h.event(egui::Event::Copy),
                    Key::X => h.event(egui::Event::Cut),
                    Key::V => h.event(egui::Event::Paste("x".into())),
                    _ => {}
                }
                format!("strg{} {k:?}", if shift { "+umschalt" } else { "" })
            }
        };
        log.push(what);
        if log.len() > 12 {
            log.remove(0);
        }
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h.step()));
        if ok.is_err() {
            panic!("Absturz nach: {log:#?}");
        }
    }
}
