//! Nur zum Ansehen: `cargo test -p spritebit-app bild_der_oberflaeche -- --ignored`
//! schreibt ein Bild der App nach SPRITEBIT_SHOT (Pfad einer PNG-Datei).

use crate::*;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

#[test]
#[ignore = "nur zum Ansehen, braucht eine Grafikkarte"]
fn bild_der_oberflaeche() {
    let Some(path) = std::env::var_os("SPRITEBIT_SHOT") else { return };
    let mut h = Harness::builder().with_size(Vec2::new(1400.0, 860.0)).wgpu().build_eframe(|cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut app = SpritebitApp::new();
        app.project.name = "Mein Spiel".into();
        let s = app.project.sprite_mut();
        s.add_layer(1, "Figur");
        for x in 4..12 {
            s.cel_mut(0, 1).set(x, 6, 5);
        }
        // SPRITEBIT_SHOT_LAYERS=n: n Ebenen (Timeline mit Überlauf).
        let layers: usize = std::env::var("SPRITEBIT_SHOT_LAYERS").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
        for k in 2..layers {
            s.add_layer(k, format!("Ebene {}", k + 1));
        }
        // SPRITEBIT_SHOT_FRAMES=n: n Frames, jeder etwas anders (Timeline).
        let frames: usize = std::env::var("SPRITEBIT_SHOT_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
        for f in 1..frames {
            let k = s.add_frame(f - 1, true);
            for y in 10..40 {
                s.cel_mut(k, 1).set(10 + 4 * f as u32, y, 3);
            }
        }
        app
    });
    h.run();
    // SPRITEBIT_SHOT_START=1: das Startfenster mit zwei zuletzt geöffneten.
    if std::env::var_os("SPRITEBIT_SHOT_START").is_some() {
        let p = &mut h.state_mut().projects;
        p.recent = vec![("Mein Spiel".into(), "C:/Spiele/Mein Spiel.sb".into()), ("Level 2".into(), "C:/Spiele/level2.sb".into())];
        p.start_open = true;
        h.run();
    }
    // SPRITEBIT_SHOT_DOCK=1: Panels gelöst und verschoben, eins aufgeklappt.
    if std::env::var_os("SPRITEBIT_SHOT_DOCK").is_some() {
        use dock_ui::PanelId;
        let d = &mut h.state_mut().dock;
        d.toggle_pin(PanelId::Light);
        d.toggle_pin(PanelId::Tiles);
        d.move_to_other_side(PanelId::Layers);
        h.run();
        h.get_by_label("Licht — Klick klappt auf").click();
        h.run();
    }
    // SPRITEBIT_SHOT_THUMB=px: Größe der Vorschaubilder in der Timeline.
    if let Some(t) = std::env::var("SPRITEBIT_SHOT_THUMB").ok().and_then(|v| v.parse::<f32>().ok()) {
        h.state_mut().tl.thumb_size = t;
        h.run();
    }
    // SPRITEBIT_SHOT_TLSCROLL=1: Ebenen der Timeline nach unten gescrollt.
    if std::env::var_os("SPRITEBIT_SHOT_TLSCROLL").is_some() {
        h.hover_at(egui::pos2(400.0, 800.0));
        h.run();
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -60.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.run();
        h.run();
    }
    // SPRITEBIT_SHOT_PALPANEL=1: zum Paletten-Panel scrollen (Raster der angezeigten Palette).
    if std::env::var_os("SPRITEBIT_SHOT_PALPANEL").is_some() {
        h.state_mut().dock.focus = Some(crate::dock_ui::PanelId::Palette);
        h.run();
        h.run();
    }
    // SPRITEBIT_SHOT_PAL=1: Farbwähler am Farbfeld 3 offen.
    if std::env::var_os("SPRITEBIT_SHOT_PAL").is_some() {
        let ctx = h.ctx.clone();
        h.state_mut().open_swatch_editor(&ctx, 3);
        h.run();
    }
    // SPRITEBIT_SHOT_PALMODAL=1: Dialog „Palette bearbeiten“ mit Farbnamen.
    if std::env::var_os("SPRITEBIT_SHOT_PALMODAL").is_some() {
        h.state_mut().open_palette_modal_for_shot();
        h.run();
        h.run();
    }
    // SPRITEBIT_SHOT_SHADE=1: Schattieren aktiv, Farbreihe gewählt, Verlauf an.
    if std::env::var_os("SPRITEBIT_SHOT_SHADE").is_some() {
        let a = h.state_mut();
        a.tool = crate::tools_ui::Tool::Shade;
        a.size = 5;
        a.shade_soft = true;
        for v in [1, 3, 4, 9] {
            a.pick_swatch(v);
        }
        h.run();
    }
    // SPRITEBIT_SHOT_SELOP=1: Auswahl-Werkzeug mit Knöpfen Neu/+/−, Auswahl mit Loch.
    if std::env::var_os("SPRITEBIT_SHOT_SELOP").is_some() {
        use spritebit_core::selection::{combine, SelOp, Selection};
        let a = h.state_mut();
        a.tool = crate::tools_ui::Tool::Select;
        a.sel_op = SelOp::Sub;
        let two = combine(Some(&Selection::rect(2, 2, 9, 9)), &Selection::rect(12, 4, 14, 7), SelOp::Add);
        a.selection = combine(two.as_ref(), &Selection::rect(4, 4, 6, 6), SelOp::Sub);
        h.run();
    }
    // SPRITEBIT_SHOT_TOUR=n: Animations-Tour bei Schritt n (0 = Timeline).
    if let Some(n) = std::env::var("SPRITEBIT_SHOT_TOUR").ok().and_then(|v| v.parse::<usize>().ok()) {
        let ctx = h.ctx.clone();
        h.state_mut().start_guide(crate::tour_ui::Guide::Tour(crate::tour_ui::TourId::Anim, n), &ctx);
        h.run();
        h.run();
    }
    // SPRITEBIT_SHOT_SEARCH=wort: Bittys Blase mit diesem Suchwort.
    if let Ok(q) = std::env::var("SPRITEBIT_SHOT_SEARCH") {
        let a = h.state_mut();
        a.bitty.open = true;
        a.bitty.query = q;
        h.run();
        h.run();
    }
    // SPRITEBIT_SHOT_TILELIB=1: Tilemap-Ebene mit Bibliothek, Panel „Kacheln“ offen.
    if std::env::var_os("SPRITEBIT_SHOT_TILELIB").is_some() {
        let a = h.state_mut();
        a.tiles_structural(|sp| {
            let l = sp.layer;
            let img = sp.cel_mut(0, l);
            for x in 0..32 {
                for y in 0..16 {
                    img.set(
                        x,
                        y,
                        if x < 16 {
                            if x < 4 || y > 11 {
                                6
                            } else {
                                3
                            }
                        } else {
                            6
                        },
                    );
                }
            }
            let ts = spritebit_core::tilemap::Tileset::build(16, 16, [&*img]);
            sp.layers[l].tileset = Some(ts);
        });
        a.save_to_lib("Wald".into(), false);
        // SPRITEBIT_SHOT_TILEXF=1: zusätzlich „Kacheln setzen“, gedreht, Rückfrage offen.
        if std::env::var_os("SPRITEBIT_SHOT_TILEXF").is_some() {
            a.tiles.mode = crate::tiles_ui::TileMode::Tiles;
            a.tiles.tile = 1;
            a.turn_tile(Some(spritebit_core::tilemap::Turn::Cw));
            a.tiles.confirm_prune = Some(1);
        }
        a.project.tileset_lib.insert("Höhle".into(), spritebit_core::tilelib::LibSet { tw: 8, th: 8, tiles: vec![vec![Some([9, 9, 9]); 64]] });
        a.dock.focus = Some(crate::dock_ui::PanelId::Tiles);
        h.run();
        h.run();
    }
    let img = h.render().expect("Bild");
    img.save(path).expect("speichern");
}
