//! Oberflächen-Tests: die App läuft ohne Fenster (egui_kittest), Maus und
//! Tasten werden simuliert, geprüft wird das Bild im Sprite.

use super::*;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn app<'a>() -> Harness<'a, SpritebitApp> {
    let mut h = Harness::builder().with_size(Vec2::new(1280.0, 2400.0)).build_eframe(|cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        SpritebitApp::new()
    });
    h.run();
    h
}

/// Bildschirmpunkt in der Mitte von Sprite-Pixel (x, y).
fn at(h: &Harness<'_, SpritebitApp>, x: f32, y: f32) -> Pos2 {
    let a = h.state();
    a.canvas_rect.min + a.pan + Vec2::new(x + 0.5, y + 0.5) * a.zoom
}

fn drag(h: &mut Harness<'_, SpritebitApp>, from: (f32, f32), to: (f32, f32)) {
    let (a, b) = (at(h, from.0, from.1), at(h, to.0, to.1));
    h.hover_at(a);
    h.run();
    h.drag_at(a);
    h.run();
    for k in 1..=8 {
        h.hover_at(a + (b - a) * (k as f32 / 8.0));
        h.run();
    }
    h.drop_at(b);
    h.run();
}

fn px(h: &Harness<'_, SpritebitApp>, x: u32, y: u32) -> u16 {
    let s = h.state().project.sprite();
    s.cel(s.frame, s.layer).get(x, y)
}

#[test]
fn hex_eingeben_und_in_die_palette() {
    let mut h = app();
    let n = h.state().project.current_palette().len();
    let tool = h.state().tool;
    let is_field = |n: &egui_kittest::kittest::AccessKitNode<'_>| n.role() == egui::accesskit::Role::TextInput && n.value().as_deref() == Some("#000000");
    h.get_by(is_field).click();
    h.run();
    h.get_by(is_field).type_text("c0ffee");
    h.run();
    h.run();
    assert!(h.state().color >= spritebit_core::FREE_BASE, "neue Farbe ist frei");
    h.get_by_label("+ In Palette").click();
    h.run();
    let a = h.state();
    assert_eq!(a.project.current_palette().len(), n + 1);
    assert_eq!(a.color as usize, n + 1);
    assert_eq!(a.current_rgb(), Some([0xc0, 0xff, 0xee]));
    assert!(a.tool == tool, "Buchstaben im Feld sind keine Kürzel");
}

#[test]
fn reiter_lassen_sich_ziehen() {
    let mut h = app();
    {
        let a = h.state_mut();
        a.project.sprites[0].name = "Aaa".into();
        for n in ["Bbb", "Ccc"] {
            a.project.sprites.push(spritebit_core::Sprite::new(n, 16, 16).unwrap());
        }
        a.tabs = vec![0, 1, 2];
    }
    h.run();
    let r = |h: &Harness<'_, SpritebitApp>, n: &str| h.get_by_label(n).rect();
    let (from, to) = (r(&h, "Aaa").center(), r(&h, "Ccc").right_center() + Vec2::new(4.0, 0.0));
    h.hover_at(from);
    h.run();
    h.drag_at(from);
    h.run();
    for k in 1..=8 {
        h.hover_at(from + (to - from) * (k as f32 / 8.0));
        h.run();
    }
    h.drop_at(to);
    h.run();
    assert_eq!(h.state().tabs, vec![1, 2, 0], "Aaa ganz nach hinten");
}

#[test]
fn reiter_ziehen_viele_schmales_fenster() {
    let mut h = Harness::builder().with_size(Vec2::new(1000.0, 700.0)).build_eframe(|cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        SpritebitApp::new()
    });
    h.run();
    {
        let a = h.state_mut();
        a.project.sprites[0].name = "background".into();
        for n in ["Sprite 4", "Sprite 3", "Sprite 5", "tile-layout", "bitty", "test", "Hero", "Sprite 9", "bitty2"] {
            a.project.sprites.push(spritebit_core::Sprite::new(n, 16, 16).unwrap());
        }
        a.tabs = (0..10).collect();
    }
    h.run();
    let r = |h: &Harness<'_, SpritebitApp>, n: &str| h.get_by_label(n).rect();
    let (from, to) = (r(&h, "Sprite 4").center(), r(&h, "Hero").right_center() + Vec2::new(4.0, 0.0));
    h.hover_at(from);
    h.run();
    h.drag_at(from);
    h.run();
    for k in 1..=8 {
        h.hover_at(from + (to - from) * (k as f32 / 8.0));
        h.run();
    }
    h.drop_at(to);
    h.run();
    assert_eq!(h.state().tabs, vec![0, 2, 3, 4, 5, 6, 7, 1, 8, 9]);
}

#[test]
fn geloestes_panel_geht_bei_klick_daneben_zu() {
    let mut h = Harness::builder().with_size(Vec2::new(1400.0, 860.0)).build_eframe(|cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        SpritebitApp::new()
    });
    h.run();
    h.state_mut().dock.toggle_pin(dock_ui::PanelId::Light);
    h.run();
    let click = |h: &mut Harness<'_, SpritebitApp>, p: egui::Pos2| {
        h.hover_at(p);
        h.run();
        h.drag_at(p);
        h.run();
        h.drop_at(p);
        h.run();
    };
    let open = |h: &mut Harness<'_, SpritebitApp>| {
        h.get_by_label("Licht — Klick klappt auf").click();
        h.run();
        assert!(h.state().dock.flyout.is_some(), "Panel aufgeklappt");
    };
    open(&mut h);
    // Klick ins Panel selbst: bleibt offen
    let inside = h.get_by_label("Schließen").rect().center() - Vec2::new(60.0, 0.0);
    click(&mut h, inside);
    assert!(h.state().dock.flyout.is_some(), "Klick ins Panel lässt es offen");
    // Klick auf die Zeichenfläche: zu
    click(&mut h, egui::pos2(700.0, 430.0));
    assert!(h.state().dock.flyout.is_none(), "Klick daneben schließt");
    // Angepinnt bleibt es ohnehin stehen
    open(&mut h);
    h.state_mut().dock.toggle_pin(dock_ui::PanelId::Light);
    h.run();
    click(&mut h, egui::pos2(700.0, 430.0));
    assert!(h.state().dock.is_pinned(dock_ui::PanelId::Light));
}

#[test]
fn trennlinie_unter_den_vorschaubildern_zieht_die_groesse() {
    let mut h = app();
    for k in 2..9 {
        h.state_mut().project.sprite_mut().add_layer(k, "x");
    }
    h.state_mut().tl.thumb_size = 32.0;
    h.run();
    let before = h.state().tl.thumb_size;
    // Die Linie unter der Kopfzeile
    let num = h.get_by_label("Ziehen: Vorschaubilder größer oder kleiner · Doppelklick: Standardgröße").rect().center();
    let btn = egui::PointerButton::Primary;
    h.hover_at(num);
    h.step();
    h.event(egui::Event::PointerButton { pos: num, button: btn, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for k in 1..=5 {
        h.event(egui::Event::PointerMoved(num + Vec2::new(0.0, 6.0 * k as f32)));
        h.step();
    }
    h.event(egui::Event::PointerButton { pos: num + Vec2::new(0.0, 30.0), button: btn, pressed: false, modifiers: Modifiers::NONE });
    h.run();
    let after = h.state().tl.thumb_size;
    assert!(after > before + 15.0, "größer gezogen: {before} → {after}");
    // Mehr Ebenen ändern die Größe nicht
    h.state_mut().project.sprite_mut().add_layer(9, "y");
    h.run();
    assert_eq!(h.state().tl.thumb_size, after);
}

#[test]
fn grosser_pinsel_malt_die_ganze_flaeche() {
    let mut h = app();
    {
        let a = h.state_mut();
        a.tool = crate::tools_ui::Tool::Brush;
        a.size = crate::tools_ui::MAX_SIZE;
        a.strength = 100;
    }
    h.run();
    drag(&mut h, (10.0, 10.0), (20.0, 12.0));
    let s = h.state().project.sprite();
    for (x, y) in [(0, 0), (s.width - 1, 0), (0, s.height - 1), (s.width - 1, s.height - 1), (15, 11)] {
        assert_eq!(px(&h, x, y), 5, "Pixel ({x}, {y}) vom 1000er-Pinsel getroffen");
    }
}

#[test]
fn stift_malt_einen_strich_ohne_luecken() {
    let mut h = app();
    drag(&mut h, (10.0, 10.0), (20.0, 10.0));
    for x in 10..=20 {
        assert_eq!(px(&h, x, 10), 5, "Pixel ({x}, 10)");
    }
    assert_eq!(px(&h, 21, 10), 0);
    assert!(h.state().dirty);
}

#[test]
fn rueckgaengig_nimmt_den_strich_zurueck() {
    let mut h = app();
    drag(&mut h, (5.0, 5.0), (8.0, 5.0));
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(px(&h, 5, 5), 0);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Y);
    h.run();
    assert_eq!(px(&h, 5, 5), 5);
}

#[test]
fn rechteck_aus_der_werkzeugleiste() {
    let mut h = app();
    h.get_by_label("Rechteck").click();
    h.run();
    assert_eq!(h.state().tool, Tool::Rect);
    drag(&mut h, (10.0, 10.0), (14.0, 13.0));
    assert_eq!(px(&h, 10, 10), 5);
    assert_eq!(px(&h, 14, 13), 5);
    assert_eq!(px(&h, 12, 10), 5, "obere Kante");
    assert_eq!(px(&h, 12, 11), 0, "innen leer");
}

#[test]
fn fuellen_per_taste() {
    let mut h = app();
    h.key_press(Key::F);
    h.run();
    assert_eq!(h.state().tool, Tool::Fill);
    let p = at(&h, 30.0, 30.0);
    h.hover_at(p);
    h.run();
    h.drag_at(p);
    h.run();
    h.drop_at(p);
    h.run();
    assert_eq!(px(&h, 0, 0), 5);
    assert_eq!(px(&h, 63, 63), 5);
}

#[test]
fn gesperrte_ebene_wird_nicht_bemalt() {
    let mut h = app();
    h.state_mut().project.sprite_mut().layers[0].locked = true;
    drag(&mut h, (10.0, 10.0), (12.0, 10.0));
    assert_eq!(px(&h, 10, 10), 0);
    assert!(h.state().hint.as_deref().is_some_and(|t| t.contains("gesperrt")));
}

#[test]
fn panel_merkt_sich_auf_und_zu() {
    let mut h = app();
    assert_eq!(h.state().panel_open("p-light"), None, "noch nichts geändert");
    h.get_by_label("Licht").click();
    h.run();
    assert_eq!(h.state().panel_open("p-light"), Some(true));
    h.get_by_label("Licht").click();
    h.run();
    assert_eq!(h.state().panel_open("p-light"), Some(false));
}

/// Brechen die Werkzeuge um, laufen die Einstellungen in derselben
/// Zeile weiter — keine dritte Zeile, solange in der zweiten Platz ist.
#[test]
fn werkzeugleiste_ohne_unnoetige_dritte_zeile() {
    for width in [860.0, 1000.0, 1700.0] {
        let mut h = Harness::builder().with_size(Vec2::new(width, 900.0)).build_eframe(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            SpritebitApp::new()
        });
        h.run();
        let top = |h: &Harness<'_, SpritebitApp>, label: &str| h.get_by_label(label).rect().top();
        let (hand, wand, clean) = (top(&h, "Hand"), top(&h, "Zauberstab"), top(&h, "Clean Stroke"));
        eprintln!("{width}: Hand {hand}, Zauberstab {wand}, Clean Stroke {clean}");
        if wand > hand + 1.0 {
            assert!((clean - wand).abs() < 6.0, "{width} px: Werkzeuge brechen um, Clean Stroke muss in ihrer zweiten Zeile stehen");
        } else {
            assert!(clean > hand + 1.0, "{width} px: Werkzeuge in einer Zeile, Einstellungen darunter");
        }
    }
}

#[test]
fn was_ist_neu_zeigt_die_notizen_im_band() {
    let mut h = app();
    h.state_mut().update.available = Some("9.9.9".into());
    h.state_mut().update.next_notes = update::NextNotes::Ready("## Deutsch\n- Neuer Pinsel\n".into());
    h.run();
    h.get_by_label("Was ist neu?").click();
    h.run();
    assert!(h.query_by_label("Neu in spritebit 9.9.9").is_some());
    assert!(h.query_by_label("Neuer Pinsel").is_some());
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().update.notes_view, None);
    // Ohne Notizen (kein Netz): das Fenster sagt es, statt leer zu sein.
    h.state_mut().update.next_notes = update::NextNotes::Failed;
    h.state_mut().update.notes_view = Some(update::NotesView::Next);
    h.run();
    assert!(h.query_by_label("Zu dieser Version gibt es keine Notizen.").is_some());
}

#[test]
fn panel_loesen_aufklappen_und_wieder_anpinnen() {
    use dock_ui::{PanelId, Side};
    let mut h = app();
    // Pin-Knopf in der Kopfzeile von „Licht“: das wievielte Panel rechts?
    let i = h.state().dock.pinned(Side::Right).iter().position(|p| *p == PanelId::Light).unwrap();
    let left = h.state().dock.pinned(Side::Left).len();
    let tip = "Angepinnt — klicken, um das Panel zu lösen (dann nur noch als Icon in der Leiste)";
    h.get_all_by_label(tip).nth(left + i).unwrap().click();
    h.run();
    assert!(!h.state().dock.is_pinned(PanelId::Light), "gelöst");
    assert_eq!(h.state().dock.loose_on(Side::Right), [PanelId::Light]);
    // Icon in der Leiste klappt es auf …
    h.get_by_label("Licht — Klick klappt auf").click();
    h.run();
    assert_eq!(h.state().dock.flyout.map(|f| f.0), Some(PanelId::Light));
    assert!(h.query_by_label("Lichtquelle").is_some(), "der Inhalt steht im Fenster");
    // … die Pinnadel im Fenster holt es zurück in die Spalte.
    h.get_by_label("Anpinnen — das Panel steht dann offen in der Spalte").click();
    h.run();
    assert!(h.state().dock.is_pinned(PanelId::Light));
    assert!(h.state().dock.flyout.is_none());
}

/// Ein Icon aus der rechten Leiste auf ein Icon der linken ziehen.
#[test]
fn panel_icon_in_die_andere_leiste_ziehen() {
    use dock_ui::{PanelId, Side};
    let mut h = app();
    let from = h.get_by_label("Licht — angepinnt, Klick springt hin").rect().center();
    let to = h.get_by_label("Farben — angepinnt, Klick springt hin").rect().center() - Vec2::new(0.0, 6.0);
    let btn = egui::PointerButton::Primary;
    h.hover_at(from);
    h.step();
    h.event(egui::Event::PointerButton { pos: from, button: btn, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    // In kleinen Schritten — erst ab ein paar Pixeln wird gezogen.
    for k in 1..=10 {
        let p = from + (to - from) * (k as f32 / 10.0);
        h.event(egui::Event::PointerMoved(p));
        h.step();
    }
    h.event(egui::Event::PointerButton { pos: to, button: btn, pressed: false, modifiers: Modifiers::NONE });
    h.run();
    let d = &h.state().dock;
    assert_eq!(d.all_on(Side::Left), [PanelId::Sprites, PanelId::Light, PanelId::Colors], "vor „Farben“ eingereiht");
    assert!(!d.all_on(Side::Right).contains(&PanelId::Light));
    assert!(d.is_pinned(PanelId::Light), "bleibt angepinnt");
}

#[test]
fn aufgeklapptes_panel_an_der_kopfzeile_verschieben() {
    use dock_ui::{PanelId, Side};
    let mut h = app();
    {
        let d = &mut h.state_mut().dock;
        d.move_to_other_side(PanelId::Colors);
        d.toggle_pin(PanelId::Colors);
    }
    h.run();
    h.get_by_label("Farben — Klick klappt auf").click();
    h.run();
    assert!(h.state().dock.flyout.is_some_and(|(id, _)| id == PanelId::Colors));
    // Der Name in der Kopfzeile des Fensters — gleich rechts vom Icon.
    let from = h.get_all_by_label("Farben").map(|n| n.rect()).find(|r| r.width() < 120.0).expect("Name im Fenster").center();
    let to = h.get_by_label("Sprites").rect().center() - Vec2::new(0.0, 6.0);
    let btn = egui::PointerButton::Primary;
    h.hover_at(from);
    h.step();
    h.event(egui::Event::PointerButton { pos: from, button: btn, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for k in 1..=10 {
        let p = from + (to - from) * (k as f32 / 10.0);
        h.event(egui::Event::PointerMoved(p));
        h.step();
    }
    h.event(egui::Event::PointerButton { pos: to, button: btn, pressed: false, modifiers: Modifiers::NONE });
    h.run();
    let d = &h.state().dock;
    assert!(d.all_on(Side::Left).contains(&PanelId::Colors), "links eingereiht: {:?}", d.all_on(Side::Left));
    assert!(d.is_pinned(PanelId::Colors));
}

/// Ziehen von A nach B in kleinen Schritten (erst ab ein paar Pixeln wird gezogen).
fn drag_from_to(h: &mut Harness<'_, SpritebitApp>, from: Pos2, to: Pos2) {
    let btn = egui::PointerButton::Primary;
    h.hover_at(from);
    h.step();
    h.event(egui::Event::PointerButton { pos: from, button: btn, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for k in 1..=10 {
        h.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 10.0)));
        h.step();
    }
    h.event(egui::Event::PointerButton { pos: to, button: btn, pressed: false, modifiers: Modifiers::NONE });
    h.run();
}

#[test]
fn panel_an_der_freien_stelle_der_kopfzeile_verschieben() {
    use dock_ui::PanelId;
    let mut h = app();
    h.state_mut().dock.toggle_pin(PanelId::Light);
    h.run();
    h.get_by_label("Licht — Klick klappt auf").click();
    h.run();
    let before = h.state().dock.flyout.expect("offen").1;
    // Zwischen Name und Pin — dort lag früher kein Griff.
    let from = h.get_by_label("Schließen").rect().center() - Vec2::new(90.0, 0.0);
    drag_from_to(&mut h, from, from - Vec2::new(200.0, 0.0));
    let (_, at) = h.state().dock.flyout.expect("bleibt offen");
    assert!((at.x - (before.x - 200.0)).abs() < 2.0, "an der Kopfzeile verschoben: {before:?} → {at:?}");
}

#[test]
fn icon_am_rand_ziehen_nimmt_das_fenster_nicht_mit() {
    use dock_ui::{PanelId, Side};
    let mut h = app();
    h.state_mut().dock.toggle_pin(PanelId::Light);
    h.run();
    h.get_by_label("Licht — Klick klappt auf").click();
    h.run();
    assert!(h.state().dock.flyout.is_some());
    let from = h.get_by_label("Licht — Klick klappt auf").rect().center();
    let to = h.get_by_label("Sprites — angepinnt, Klick springt hin").rect().center() + Vec2::new(0.0, 6.0);
    drag_from_to(&mut h, from, to);
    let d = &h.state().dock;
    assert!(d.flyout.is_none(), "das Fenster wandert nicht mit, es klappt zu");
    assert!(d.all_on(Side::Left).contains(&PanelId::Light), "Icon in die linke Leiste umgezogen: {:?}", d.all_on(Side::Left));
}

#[test]
fn aufgeklapptes_panel_frei_verschieben() {
    use dock_ui::PanelId;
    let mut h = app();
    h.state_mut().dock.toggle_pin(PanelId::Light);
    h.run();
    h.get_by_label("Licht — Klick klappt auf").click();
    h.run();
    let before = h.state().dock.flyout.expect("offen").1;
    let from = h.get_all_by_label("Licht").map(|n| n.rect()).find(|r| r.width() < 120.0).expect("Name im Fenster").center();
    let to = from - Vec2::new(300.0, 0.0);
    let btn = egui::PointerButton::Primary;
    h.hover_at(from);
    h.step();
    h.event(egui::Event::PointerButton { pos: from, button: btn, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for k in 1..=10 {
        h.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 10.0)));
        h.step();
    }
    h.event(egui::Event::PointerButton { pos: to, button: btn, pressed: false, modifiers: Modifiers::NONE });
    h.run();
    let (id, at) = h.state().dock.flyout.expect("bleibt offen, wo man es loslässt");
    assert_eq!(id, PanelId::Light);
    assert!((at.x - (before.x - 300.0)).abs() < 2.0, "um 300 px nach links: {before:?} → {at:?}");
    assert!(!h.state().dock.is_pinned(PanelId::Light));
}

#[test]
fn neues_projekt_mit_namen_und_speicherort() {
    let mut h = app();
    h.state_mut().project.palettes.push(spritebit_core::Palette::new("meine", vec![[1, 2, 3]]));
    h.state_mut().project.sprites.push(spritebit_core::Sprite::new("alt", 8, 8).unwrap());
    let dir = std::env::temp_dir().join(format!("spritebit-neues-projekt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Mein Spiel.sb");
    h.state_mut().create_project_at("Mein Spiel".into(), &path);
    h.run();
    let a = h.state();
    assert_eq!(a.project.name, "Mein Spiel");
    assert_eq!(a.project.sprites.len(), 1, "ein leerer Sprite");
    assert!(a.project.palettes.iter().any(|p| p.name == "meine"), "eigene Palette kommt mit");
    assert!(!a.dirty);
    assert_eq!(a.path.as_deref(), Some(path.as_path()));
    assert_eq!(a.projects.recent.first().map(|r| r.1.clone()), Some(path.clone()), "zuletzt geöffnet");
    assert!(a.title.starts_with("Mein Spiel"), "Titelleiste: {}", a.title);
    let back = load_native(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(back.name, "Mein Spiel", "der Name steht in der Datei");
    // Umbenennen über den Dialog
    h.state_mut().projects.rename = Some("Level 2".into());
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().project.name, "Level 2");
    assert!(h.state().dirty, "umbenannt = ungespeichert");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ebenen_panel_waehlt_und_blendet_aus() {
    let mut h = app();
    h.get_all_by_label("Neue Ebene über der aktiven").next().unwrap().click();
    h.run();
    assert_eq!(h.state().project.sprite().layer, 1, "die neue Ebene ist aktiv");
    let first = h.state().project.sprite().layers[0].name.clone();
    h.get_by_label(&first).click();
    h.run();
    assert_eq!(h.state().project.sprite().layer, 0, "Klick auf die Zeile wählt die Ebene");
    // Oberste Zeile = oberste Ebene.
    h.get_all_by_label("Ebene ausblenden").next().unwrap().click();
    h.run();
    assert!(!h.state().project.sprite().layers[1].visible, "das Auge blendet aus");
    assert!(h.state().project.sprite().layers[0].visible);
}

#[test]
fn sprites_und_farben_lassen_sich_einklappen() {
    let mut h = app();
    for (name, id) in [("Sprites", "p-sprites"), ("Farben", "p-colors")] {
        h.get_by_label(name).click();
        h.run();
        assert_eq!(h.state().panel_open(id), Some(false), "{name} zugeklappt");
        h.get_by_label(name).click();
        h.run();
        assert_eq!(h.state().panel_open(id), Some(true), "{name} wieder offen");
    }
}

#[test]
fn bitty_bietet_entsperren_an() {
    let mut h = app();
    h.state_mut().project.sprite_mut().layers[0].locked = true;
    drag(&mut h, (10.0, 10.0), (12.0, 10.0));
    assert_eq!(h.state().bitty.hint_id(), Some("layerLocked"));
    h.get_by_label("Entsperren").click();
    h.run();
    assert!(!h.state().project.sprite().layers[0].locked);
    assert_eq!(h.state().bitty.hint_id(), None);
    // Einmal pro Sitzung: wieder sperren und malen — kein zweiter Hinweis.
    h.state_mut().project.sprite_mut().layers[0].locked = true;
    drag(&mut h, (10.0, 10.0), (12.0, 10.0));
    assert_eq!(h.state().bitty.hint_id(), None);
}

#[test]
fn auswahl_verschieben_und_rueckgaengig() {
    let mut h = app();
    drag(&mut h, (10.0, 10.0), (11.0, 10.0)); // zwei Pixel malen
    h.key_press(Key::A);
    h.run();
    drag(&mut h, (9.0, 9.0), (12.0, 11.0)); // Auswahl um die Pixel
    assert!(h.state().selection.is_some());
    drag(&mut h, (10.0, 10.0), (30.0, 20.0)); // darin ziehen = verschieben
    h.key_press(Key::Escape); // absetzen
    h.run();
    assert_eq!(px(&h, 10, 10), 0, "alte Stelle leer");
    assert_eq!(px(&h, 30, 20), 5);
    assert_eq!(px(&h, 31, 20), 5);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(px(&h, 10, 10), 5, "Undo holt sie zurück");
    assert_eq!(px(&h, 30, 20), 0);
}

/// Im echten Fenster kommen Strg+C/X/V nicht als Tasten an, sondern als
/// Copy/Cut/Paste (egui-winit) — so wie hier.
#[test]
fn kopieren_und_einfuegen_wie_im_fenster() {
    let mut h = app();
    drag(&mut h, (2.0, 2.0), (3.0, 2.0));
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    h.event(egui::Event::Copy);
    h.run();
    assert!(h.state().clipboard.is_some(), "Strg+C kopiert");
    h.key_press(Key::Escape);
    h.run();
    h.event(egui::Event::Paste("spritebit".into()));
    h.run();
    assert!(h.state().float.is_some(), "Strg+V fügt ein");
    h.key_press(Key::Escape);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    h.event(egui::Event::Cut);
    h.run();
    assert_eq!(px(&h, 2, 2), 0, "Strg+X schneidet aus");
}

#[test]
fn strg_alt_n_neuer_sprite_strg_n_neues_projekt() {
    let mut h = app();
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::N);
    h.run();
    assert!(h.state().sprite_dialog.is_some(), "Strg+Alt+N: Neuer Sprite");
    assert!(h.state().projects.new_name.is_none(), "nicht: neues Projekt");
    h.state_mut().sprite_dialog = None;
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    assert!(h.state().projects.new_name.is_some(), "Strg+N: neues Projekt");
    assert!(h.state().sprite_dialog.is_none());
}

#[test]
fn aseprite_datei_kommt_als_sprite_dazu() {
    let mut h = app();
    // Eine Aseprite-Datei aus einem Sprite mit eigener Palette bauen …
    let pal = spritebit_core::Palette::new("aus aseprite", vec![[9, 8, 7], [1, 2, 3]]);
    let mut sp = spritebit_core::Sprite::new("held", 6, 4).unwrap();
    sp.cel_mut(0, 0).set(2, 1, 2);
    let dir = std::env::temp_dir().join(format!("spritebit-ase-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("held.aseprite");
    std::fs::write(&path, spritebit_core::aseprite::write(&sp, &pal)).unwrap();
    // … und öffnen: ein Sprite mehr, mit der Palette der Datei.
    let before = h.state().project.sprites.len();
    h.state_mut().open_path(path.clone());
    h.run();
    let a = h.state();
    assert_eq!(a.project.sprites.len(), before + 1);
    assert_eq!(a.sprite().name, "held");
    assert_eq!((a.sprite().width, a.sprite().height), (6, 4));
    assert_eq!(a.project.current_palette().colors, vec![[9, 8, 7], [1, 2, 3]]);
    assert_eq!(a.sprite().cel(0, 0).get(2, 1), 2);
    assert!(a.dirty);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn projektname_steht_in_der_menuleiste() {
    let mut h = app();
    h.state_mut().project.name = "Mein Spiel".into();
    h.run();
    h.get_by_label("Mein Spiel").click();
    h.run();
    assert_eq!(h.state().projects.rename.as_deref(), Some("Mein Spiel"), "Klick benennt um");
}

#[test]
fn pfeiltasten_blaettern_frames_und_ebenen() {
    let mut h = app();
    {
        let s = h.state_mut().project.sprite_mut();
        s.add_frame(0, false);
        s.add_frame(1, false);
        s.add_layer(1, "Oben");
        s.frame = 0;
        s.layer = 1;
    }
    h.run();
    let at = |h: &Harness<'_, SpritebitApp>| (h.state().sprite().frame, h.state().sprite().layer);
    h.key_press(Key::ArrowRight);
    h.run();
    assert_eq!(at(&h), (1, 1), "rechts: nächster Frame");
    h.key_press(Key::ArrowLeft);
    h.key_press(Key::ArrowLeft);
    h.run();
    assert_eq!(at(&h), (2, 1), "links am Anfang: ans Ende, wie Komma");
    h.key_press(Key::ArrowDown);
    h.run();
    assert_eq!(at(&h), (2, 0), "runter: Ebene darunter");
    h.key_press(Key::ArrowDown);
    h.run();
    assert_eq!(at(&h), (2, 0), "unterste bleibt");
    h.key_press(Key::ArrowUp);
    h.run();
    assert_eq!(at(&h), (2, 1), "hoch: Ebene darüber");
    // Mit Auswahl verschieben die Pfeile sie — Frame und Ebene bleiben.
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    h.key_press(Key::ArrowRight);
    h.run();
    assert_eq!(at(&h), (2, 1));
}

#[test]
fn strg_d_hebt_die_auswahl_auf() {
    let mut h = app();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    assert!(h.state().selection.is_some());
    h.key_press_modifiers(Modifiers::COMMAND, Key::D);
    h.run();
    assert!(h.state().selection.is_none());
}

#[test]
fn kopieren_und_einfuegen() {
    let mut h = app();
    drag(&mut h, (2.0, 2.0), (3.0, 2.0));
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::C);
    h.run();
    h.key_press(Key::Escape);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::V);
    h.run();
    assert!(h.state().float.is_some(), "Eingefügtes schwebt");
    // eins nach rechts schieben und absetzen
    h.key_press(Key::ArrowRight);
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(px(&h, 4, 2), 5);
    assert_eq!(px(&h, 2, 2), 5, "Original bleibt");
}

#[test]
fn zauberstab_loescht_die_flaeche() {
    let mut h = app();
    h.key_press(Key::F);
    h.run();
    let p = at(&h, 5.0, 5.0);
    h.hover_at(p);
    h.run();
    h.drag_at(p);
    h.run();
    h.drop_at(p);
    h.run();
    assert_eq!(px(&h, 0, 0), 5);
    h.key_press(Key::W);
    h.run();
    let p = at(&h, 5.0, 5.0);
    h.hover_at(p);
    h.run();
    h.drag_at(p);
    h.run();
    h.drop_at(p);
    h.run();
    assert_eq!(px(&h, 0, 0), 0);
    assert_eq!(px(&h, 63, 63), 0);
}

#[test]
fn symmetrie_malt_gespiegelt() {
    let mut h = app();
    h.state_mut().mirror_x = true;
    drag(&mut h, (2.0, 5.0), (2.0, 5.0));
    assert_eq!(px(&h, 2, 5), 5);
    assert_eq!(px(&h, 61, 5), 5, "64 - 1 - 2");
}

#[test]
fn kacheln_malen_setzen_und_mitziehen() {
    use spritebit_core::tilemap::Tileset;
    let mut h = app();
    h.state_mut().project.sprite_mut().layers[0].tileset = Some(Tileset::new(8, 8));
    h.state_mut().tool = Tool::Pencil;
    let n_tiles = |h: &Harness<'_, SpritebitApp>| h.state().sprite().layers[0].tileset.as_ref().unwrap().tiles.len();
    // Pixel malen in eine leere Zelle (Auto): neue Kachel.
    drag(&mut h, (1.0, 1.0), (1.0, 1.0));
    assert_eq!(n_tiles(&h), 1);
    // Kacheln setzen: Kachel 1 in die Zelle (2, 0).
    h.state_mut().tiles.mode = tiles_ui::TileMode::Tiles;
    h.state_mut().tiles.tile = 1;
    drag(&mut h, (17.0, 1.0), (17.0, 1.0));
    assert_eq!(px(&h, 17, 1), 5, "gesetzt");
    assert_eq!(n_tiles(&h), 1, "Setzen legt keine Kachel an");
    // Pixel malen in der Kopie: das Original zieht mit.
    h.state_mut().tiles.mode = tiles_ui::TileMode::Pixel;
    drag(&mut h, (20.0, 4.0), (20.0, 4.0));
    assert_eq!(px(&h, 4, 4), 5, "Original mitgezogen");
    assert_eq!(n_tiles(&h), 1);
    // Ein Undo nimmt Strich und Mitziehen zusammen zurück.
    h.state_mut().undo();
    h.run();
    assert_eq!((px(&h, 4, 4), px(&h, 20, 4)), (0, 0));
    // Rechtsklick im Modus „Kacheln“ leert die Zelle.
    h.state_mut().tiles.mode = tiles_ui::TileMode::Tiles;
    let a = at(&h, 17.0, 1.0);
    h.hover_at(a);
    h.run();
    h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE });
    h.run();
    h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE });
    h.run();
    assert_eq!(px(&h, 17, 1), 0, "geleert");
    assert_eq!(px(&h, 1, 1), 5, "die andere Stelle bleibt");
}

#[test]
fn hilfslinien_gleichmaessig_verteilen() {
    let mut h = app();
    h.state_mut().set_even(true, 3);
    h.state_mut().set_even(false, 1);
    let (w, hh) = (h.state().sprite().width, h.state().sprite().height);
    assert_eq!(h.state().sprite().guides.h, spritebit_core::Guides::even_lines(3, hh));
    assert_eq!(h.state().sprite().guides.v, vec![w / 2]);
    assert!(h.state().guides.show);
    h.state_mut().set_even(true, 0);
    assert!(h.state().sprite().guides.h.is_empty());
}

#[test]
fn hilfslinien_layout_speichern_und_anwenden() {
    let mut h = app();
    let (w, hh) = (h.state().sprite().width, h.state().sprite().height);
    {
        let g = h.state_mut().guides_mut();
        g.h = vec![8];
        g.v = vec![16];
    }
    h.state_mut().save_guide_layout(" Held ");
    assert_eq!(h.state().guides.layouts.len(), 1);
    assert_eq!(h.state().guides.layouts[0].name, "Held");
    // Gleicher Name ersetzt.
    h.state_mut().guides_mut().v = vec![20];
    h.state_mut().save_guide_layout("Held");
    assert_eq!(h.state().guides.layouts.len(), 1);
    // Andere Linien, dann anwenden: die gespeicherten sind zurück.
    h.state_mut().guides_mut().h.clear();
    h.state_mut().apply_guide_layout(0);
    assert_eq!(h.state().sprite().guides.h, vec![8]);
    assert_eq!(h.state().sprite().guides.v, vec![20]);
    // JSON hin und zurück (Format wie im Web).
    let list = guides_ui::parse_layouts(&guides_ui::layouts_json(&h.state().guides.layouts));
    assert_eq!(list, h.state().guides.layouts);
    assert_eq!((list[0].width, list[0].height), (w, hh));
}

#[test]
fn dunkel_hell_und_vollbild_immer_in_der_leiste() {
    let mut h = app();
    assert!(!h.state().view.light);
    h.get_by_label("Hell").click();
    h.run();
    assert!(h.state().view.light, "Hell");
    h.get_by_label("Dunkel").click();
    h.run();
    assert!(!h.state().view.light, "Dunkel");
    h.get_by_label("Vollbild").click();
    h.run();
    assert!(h.state().view.fullscreen);
    h.get_by_label("Vollbild beenden").click();
    h.run();
    assert!(!h.state().view.fullscreen);
}

#[test]
fn einfuegen_in_andere_palette_behaelt_die_farben() {
    use spritebit_core::selection::rgb_of;
    let mut h = app();
    h.state_mut().project.sprite_mut().palette = "graustufen".into();
    for x in 0..4u32 {
        h.state_mut().project.sprite_mut().active().set(x, 0, x as u16 + 1);
    }
    let before: Vec<_> = {
        let s = h.state();
        let pal = s.project.current_palette();
        (0..4).map(|x| rgb_of(s.sprite().cel(0, 0).get(x, 0), &pal, &s.sprite().free)).collect()
    };
    h.state_mut().selection = Some(spritebit_core::selection::Selection::rect(0, 0, 3, 0));
    h.state_mut().copy_selection();
    h.state_mut().create_sprite("Bunt".into(), "golden".into(), 16, 16);
    h.run();
    assert_eq!(h.state().sprite().palette, "golden");
    h.state_mut().paste_clipboard();
    h.state_mut().deselect();
    let s = h.state();
    let pal = s.project.current_palette();
    let after: Vec<_> = (0..4).map(|x| rgb_of(s.sprite().cel(0, 0).get(x, 0), &pal, &s.sprite().free)).collect();
    assert_eq!(after, before, "sieht aus wie im Original");
    assert!(s.hint.as_deref().is_some_and(|t| t.contains("Farben des Originals")));
}

#[test]
fn umschalt_malt_gerade_linien() {
    let mut h = app();
    h.state_mut().tool = Tool::Pencil;
    let sh = Modifiers::SHIFT;
    let pts = [(2.0, 5.0), (4.0, 6.0), (6.0, 4.0), (9.0, 6.0), (12.0, 5.0)];
    let a = at(&h, pts[0].0, pts[0].1);
    h.hover_at(a);
    h.run();
    // Umschalt die ganze Zeit gehalten (event_modifiers ließe sie gleich wieder los).
    h.event(egui::Event::ModifiersChanged(sh));
    h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: sh });
    h.run();
    for (x, y) in &pts[1..] {
        h.event(egui::Event::PointerMoved(at(&h, *x, *y)));
        h.run();
    }
    let b = at(&h, 12.0, 5.0);
    h.event(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: sh });
    h.run();
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    h.run();
    for x in 2..=12 {
        assert_eq!(px(&h, x, 5), 5, "gerade Linie bei x = {x}");
    }
    assert_eq!((px(&h, 4, 6), px(&h, 6, 4), px(&h, 9, 6)), (0, 0, 0), "keine Wackler");
}

#[test]
fn fuellen_mit_grenzen_aus_allen_ebenen() {
    let mut h = app();
    // Ebene 1: senkrechte Linie bei x = 8 als Vorlage; gemalt wird auf Ebene 2.
    for y in 0..64 {
        h.state_mut().project.sprite_mut().active().set(8, y, 3);
    }
    h.state_mut().project.sprite_mut().add_layer(1, "Farbe");
    h.state_mut().tool = Tool::Fill;
    h.state_mut().fill_visible = true;
    drag(&mut h, (2.0, 2.0), (2.0, 2.0));
    let sp = h.state().sprite();
    assert_eq!(sp.cel(0, 1).get(2, 2), 5, "links der Linie gefüllt");
    assert_eq!(sp.cel(0, 1).get(7, 40), 5);
    assert_eq!(sp.cel(0, 1).get(10, 2), 0, "rechts der Linie nicht");
    assert_eq!(sp.cel(0, 0).get(2, 2), 0, "die Vorlage bleibt unberührt");
    // Ohne die Option füllt es die ganze (leere) Ebene 2.
    h.state_mut().fill_visible = false;
    h.state_mut().undo();
    h.run();
    drag(&mut h, (2.0, 2.0), (2.0, 2.0));
    assert_eq!(h.state().sprite().cel(0, 1).get(10, 2), 5);
}

#[test]
fn auswahl_mit_anfasser_skalieren() {
    let mut h = app();
    for y in 12..16 {
        for x in 12..16 {
            h.state_mut().project.sprite_mut().active().set(x, y, ((x + y) % 2 + 2) as u16);
        }
    }
    h.state_mut().tool = Tool::Select;
    h.state_mut().selection = Some(spritebit_core::selection::Selection::rect(12, 12, 15, 15));
    h.run();
    // at() zielt auf Zellmitten: 15.5 + 0.5 liegt auf der Ecke (16, 16).
    drag(&mut h, (15.5, 15.5), (19.5, 19.5));
    let s = h.state().selection.clone().unwrap();
    assert_eq!((s.x, s.y, s.w, s.h), (12, 12, 8, 8));
    assert_eq!(h.state().float.as_ref().map(|f| (f.clip.w, f.clip.h)), Some((8, 8)));
    h.state_mut().deselect();
    assert_eq!((px(&h, 12, 12), px(&h, 13, 12), px(&h, 14, 12)), (2, 2, 3), "jedes Pixel verdoppelt");
    assert_eq!(px(&h, 19, 19), 2);
}

#[test]
fn hand_zieht_hilfslinie() {
    let mut h = app();
    h.state_mut().tool = Tool::Pan;
    h.state_mut().project.sprite_mut().guides.v = vec![10];
    h.state_mut().guides.show = true;
    let pan = h.state().pan;
    // at() zielt auf die Zellmitte: 9.5 + 0.5 liegt genau auf der Linie bei x = 10.
    drag(&mut h, (9.5, 5.0), (14.5, 5.0));
    assert_eq!(h.state().sprite().guides.v, vec![15], "Linie mitgezogen");
    assert_eq!(h.state().pan, pan, "Ansicht nicht verschoben");
    // Neben einer Linie verschiebt die Hand wie gewohnt.
    drag(&mut h, (30.0, 30.0), (34.0, 30.0));
    assert_ne!(h.state().pan, pan);
    assert_eq!(h.state().sprite().guides.v, vec![15]);
}

#[test]
fn groesse_mit_alt_und_rechts_ziehen() {
    let mut h = app();
    h.state_mut().tool = Tool::Brush;
    h.state_mut().size = 3;
    // Etwas Gemaltes unter dem Zug — Alt + Rechts darf es nicht radieren.
    for x in 4..12 {
        h.state_mut().project.sprite_mut().active().set(x, 8, 3);
    }
    let a = at(&h, 5.0, 8.0);
    let alt = Modifiers::ALT;
    let btn = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed, modifiers: alt };
    h.hover_at(a);
    h.run();
    h.event_modifiers(btn(a, true), alt);
    h.run();
    // 36 Bildschirmpixel nach rechts (je 6 px eine Stufe), in kleinen Schritten.
    for k in 1..=6 {
        h.event_modifiers(egui::Event::PointerMoved(a + Vec2::new(6.0 * k as f32, 0.0)), alt);
        h.run();
    }
    assert_eq!(h.state().size, 9, "3 + 6 Stufen (36 px, je 6 px)");
    // Weit nach links: nie unter 1.
    h.event_modifiers(egui::Event::PointerMoved(a - Vec2::new(400.0, 0.0)), alt);
    h.run();
    assert_eq!(h.state().size, 1);
    h.event_modifiers(btn(a - Vec2::new(400.0, 0.0), false), alt);
    h.run();
    assert!(h.state().size_drag.is_none(), "Loslassen beendet das Ziehen");
    for x in 4..12 {
        assert_eq!(px(&h, x, 8), 3, "nichts radiert bei x = {x}");
    }
    assert_eq!(h.state().color, 5, "keine Pipette (sonst wäre es 3)");
}

#[test]
fn staerke_ist_anfangs_100_prozent() {
    assert_eq!(app().state().strength, 100);
}

#[test]
fn pipette_mit_alt() {
    let mut h = app();
    h.state_mut().project.sprite_mut().active().set(7, 7, 3);
    let p = at(&h, 7.0, 7.0);
    h.hover_at(p);
    h.run();
    h.event_modifiers(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::ALT }, Modifiers::ALT);
    h.run();
    h.event_modifiers(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::ALT }, Modifiers::ALT);
    h.run();
    assert_eq!(h.state().color, 3);
    assert_eq!(px(&h, 7, 7), 3, "nichts übermalt");
}

#[test]
fn freie_farbe_und_palette_bearbeiten() {
    let mut h = app();
    h.state_mut().set_rgb([1, 2, 3]);
    assert!(h.state().color >= spritebit_core::FREE_BASE);
    h.state_mut().set_rgb([0xff, 0xff, 0xff]);
    assert_eq!(h.state().color, 1, "Palettenfarbe wird erkannt");
}

#[test]
fn rueckfrage_bei_ungespeichertem() {
    let mut h = app();
    h.state_mut().dirty = true;
    h.state_mut().unsaved_ask = Some(Pending::Close);
    h.run();
    h.get_by_label("Abbrechen").click();
    h.run();
    assert!(h.state().unsaved_ask.is_none());
    assert!(!h.state().allow_close, "Abbrechen schließt nicht");
    h.state_mut().unsaved_ask = Some(Pending::Close);
    h.run();
    h.get_by_label("Nicht speichern").click();
    h.run();
    assert!(h.state().allow_close);
}

#[test]
fn sprache_umschalten() {
    i18n::set_lang(i18n::Lang::En);
    let mut h = app();
    h.get_by_label("File");
    h.get_by_label("Edit");
    i18n::set_lang(i18n::Lang::At);
    h.run();
    h.get_by_label("Bearbeitn");
    i18n::set_lang(i18n::Lang::De);
    h.run();
    h.get_by_label("Bearbeiten");
}

#[test]
fn bild_spiegeln_ganzer_sprite_und_undo() {
    let mut h = app();
    h.state_mut().project.sprite_mut().active().set(0, 3, 5);
    h.get_by_label("↔ Spiegeln").click();
    h.run();
    assert_eq!(px(&h, 63, 3), 5);
    assert_eq!(px(&h, 0, 3), 0);
    h.state_mut().undo();
    assert_eq!(px(&h, 0, 3), 5);
}

#[test]
fn bild_drehen_nur_die_auswahl() {
    let mut h = app();
    {
        let a = h.state_mut();
        a.project.sprite_mut().active().set(10, 10, 5);
        a.project.sprite_mut().active().set(40, 40, 3);
        a.selection = Some(Selection::rect(10, 10, 13, 11)); // 4 × 2
    }
    h.state_mut().rotate90();
    h.state_mut().deselect();
    // Mitte bleibt: 4×2 bei (10,10) → 2×4 bei (11,9); (0,0) landet oben rechts.
    assert_eq!(px(&h, 12, 9), 5);
    assert_eq!(px(&h, 40, 40), 3, "außerhalb unberührt");
}

#[test]
fn zuschneiden_und_outline() {
    let mut h = app();
    h.state_mut().project.sprite_mut().active().set(20, 30, 5);
    h.get_by_label("Zuschneiden").click();
    h.run();
    assert_eq!((h.state().sprite().width, h.state().sprite().height), (1, 1));
    h.state_mut().undo();
    assert_eq!(h.state().sprite().width, 64);
    let n = h.state_mut().clean_for_test_outline();
    assert_eq!(n, 4);
}

#[test]
fn reiter_der_geoeffneten_sprites() {
    let mut h = app();
    for n in ["Held", "Baum"] {
        h.state_mut().create_sprite(n.into(), "graustufen".into(), 16, 16);
        h.run(); // jedes Anlegen ist ein eigener Klick, also ein eigener Frame
    }
    assert_eq!(h.state().tabs, vec![0, 1, 2]);
    assert_eq!(h.state().project.current, 2);
    // Klick auf den Reiter wechselt.
    h.get_by_label("Held").click();
    h.run();
    assert_eq!(h.state().project.current, 1);
    // Strg+Tab rundum weiter, Strg+Umschalt+Tab zurück.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
    h.run();
    assert_eq!(h.state().project.current, 2);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
    h.run();
    assert_eq!(h.state().project.current, 0);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Tab);
    h.run();
    assert_eq!(h.state().project.current, 2);
    // Strg+W schließt den Reiter, der Sprite bleibt; der linke wird aktiv.
    h.key_press_modifiers(Modifiers::COMMAND, Key::W);
    h.run();
    assert_eq!(h.state().tabs, vec![0, 1]);
    assert_eq!(h.state().project.current, 1);
    assert_eq!(h.state().project.sprites.len(), 3);
    // Duplizieren schiebt die Reiter dahinter mit.
    h.state_mut().tabs = vec![2, 1, 0];
    h.state_mut().duplicate_sprite(0);
    h.run();
    assert_eq!(h.state().tabs, vec![3, 2, 0, 1], "Kopie an Stelle 1, Reiter hinten dran");
    // Löschen nimmt den Reiter mit.
    h.state_mut().delete_sprite(3);
    h.run();
    assert_eq!(h.state().tabs, vec![2, 0, 1]);
    // Der letzte Reiter lässt sich nicht schließen.
    h.state_mut().tabs = vec![h.state().project.current];
    h.key_press_modifiers(Modifiers::COMMAND, Key::W);
    h.run();
    assert_eq!(h.state().tabs.len(), 1);
}

#[test]
fn maske_auf_gesperrter_licht_ebene() {
    let mut h = app();
    for y in 10..14 {
        for x in 10..14 {
            h.state_mut().project.sprite_mut().active().set(x, y, 3);
        }
    }
    h.state_mut().image.cast_on = false;
    h.state_mut().commit_light_layers();
    h.run();
    let li = h.state().project.sprite().layers.iter().position(|l| l.fx.is_some()).unwrap();
    h.state_mut().project.sprite_mut().layer = li;
    h.run();
    assert!(h.state().project.sprite().layers[li].locked);
    h.get_all_by_label("Maske hinzufügen — damit blendest du Teile der Ebene aus, ohne sie zu löschen").next().unwrap().click();
    h.run();
    assert!(h.state().project.sprite().editing_mask());
    // Mit dem Stift über die Lichtkante oben: die Maske blendet sie aus.
    h.state_mut().tool = Tool::Pencil;
    drag(&mut h, (10.0, 10.0), (13.0, 10.0));
    let sp = h.state().project.sprite();
    assert_eq!(sp.cel(0, li).get(11, 10), 2, "Licht-Ebene selbst unverändert");
    assert!(sp.layers[li].mask.as_ref().unwrap().hides(11, 10), "trotz Sperre in die Maske gemalt");
    let buf = spritebit_core::render_rgba(sp, &h.state().project.current_palette(), 0, spritebit_core::Rect { x: 11, y: 10, w: 1, h: 1 });
    assert_eq!(&buf[..3], &[0x99, 0x99, 0x99], "zu sehen ist die Figur, nicht das Licht");
    // Rückgängig nimmt den Strich in der Maske zurück.
    h.state_mut().undo();
    h.run();
    assert!(!h.state().project.sprite().layers[li].mask.as_ref().unwrap().hides(11, 10));
}

#[test]
fn licht_vorschau_dann_als_ebenen() {
    let mut h = app();
    // 4×4-Block aus Farbe 3 (#999999) bei (10,10)
    for y in 10..14 {
        for x in 10..14 {
            h.state_mut().project.sprite_mut().active().set(x, y, 3);
        }
    }
    h.run();
    h.get_by_label("Licht").click();
    h.run();
    h.run();
    let cel = |h: &Harness<'_, SpritebitApp>, l: usize, x: u32, y: u32| h.state().project.sprite().cel(0, l).get(x, y);
    // Vorschau: noch keine Ebene, die Zeichenfläche zeigt eine Kopie mit Licht.
    assert_eq!(h.state().project.sprite().layers.len(), 1, "Vorschau legt nichts an");
    let pv = &h.state().image.preview.as_ref().expect("Vorschau da").1;
    assert_eq!(pv.cel(0, 1).get(10, 10), 2, "Vorschau: oben links heller");

    h.get_by_label("Als Ebene übernehmen").click();
    h.run();
    let sp = h.state().project.sprite();
    assert_eq!(sp.layers.len(), 2, "Licht-Ebene dazu");
    assert!(sp.layers[1].fx.is_some() && sp.layers[1].locked);
    assert_eq!(sp.layer, 0, "aktiv bleibt die Figur");
    assert_eq!(cel(&h, 0, 10, 10), 3, "Original unverändert");
    assert_eq!(cel(&h, 1, 10, 10), 2);
    h.run();
    assert!(h.state().image.preview.is_none(), "keine Vorschau mehr, die Ebene zeigt es");

    // Andere Richtung: neu gerechnet, die alten Kanten sind weg.
    h.get_by_label("Licht von unten rechts").click();
    h.run();
    assert_eq!(cel(&h, 1, 13, 13), 2, "unten rechts jetzt hell");
    assert_ne!(cel(&h, 1, 10, 10), 2);
    h.state_mut().undo();
    h.run();
    assert_eq!(cel(&h, 1, 10, 10), 2, "Rückgängig holt das vorige Licht");

    // Schlagschatten anhaken: Schatten-Ebene unter der Figur.
    h.get_by_label("Schlagschatten").click();
    h.run();
    let sp = h.state().project.sprite();
    assert_eq!(sp.layers.len(), 3);
    assert_eq!(sp.layer, 1, "die Figur ist nach oben gerückt und bleibt aktiv");
    assert_ne!(cel(&h, 0, 14, 14), 0, "Schatten unten rechts (Licht von oben links)");
    // … und wieder weg.
    h.get_by_label("Schlagschatten").click();
    h.run();
    assert_eq!(h.state().project.sprite().layers.len(), 2);
    assert_eq!(h.state().project.sprite().layer, 0);
}

#[test]
fn sprite_anlegen_duplizieren_loeschen() {
    let mut h = app();
    h.state_mut().open_new_sprite();
    if let Some(sprites_ui::SpriteDialog::New { name, palette, w, h: hh }) = &mut h.state_mut().sprite_dialog {
        *name = "Held".into();
        *palette = "pico8".into();
        *w = 16;
        *hh = 8;
    }
    h.run();
    h.get_by_label("Erstellen").click();
    h.run();
    let a = h.state();
    assert_eq!(a.project.sprites.len(), 2);
    assert_eq!((a.sprite().name.as_str(), a.sprite().palette.as_str(), a.sprite().width), ("Held", "pico8", 16));
    h.state_mut().sprite_dialog = Some(sprites_ui::SpriteDialog::Delete { i: 0 });
    h.run();
    h.get_by_label("Löschen").click();
    h.run();
    assert_eq!(h.state().project.sprites.len(), 1);
    assert_eq!(h.state().sprite().name, "Held", "der verbleibende ist aktiv");
}

#[test]
fn ebene_zusammenlegen_ueber_knopf() {
    let mut h = app();
    h.state_mut().project.sprite_mut().active().set(1, 1, 5);
    h.get_all_by_label("Neue Ebene über der aktiven").next().unwrap().click();
    h.run();
    h.state_mut().project.sprite_mut().active().set(2, 2, 3);
    h.get_all_by_label("Nach unten zusammenlegen — in jedem Frame").next().unwrap().click();
    h.run();
    let sp = h.state().sprite();
    assert_eq!(sp.layers.len(), 1);
    assert_eq!((sp.cel(0, 0).get(1, 1), sp.cel(0, 0).get(2, 2)), (5, 3));
}

/// Was man an (x, y) sieht — als RGB.
fn rgb_at(h: &Harness<'_, SpritebitApp>, x: u32, y: u32) -> Option<spritebit_core::Rgb> {
    let a = h.state();
    spritebit_core::selection::rgb_of(px(h, x, y), &a.project.current_palette(), &a.sprite().free)
}

#[test]
fn farbstufen_ordnen_bild_bleibt_und_undo() {
    let mut h = app();
    for x in 0..5 {
        h.state_mut().project.sprite_mut().active().set(x, 0, x as u16 + 1);
    }
    let before: Vec<_> = (0..5).map(|x| rgb_at(&h, x, 0)).collect();
    let name0 = h.state().sprite().palette.clone();
    h.get_by_label("Nach Farbstufen").click();
    h.run();
    let after: Vec<_> = (0..5).map(|x| rgb_at(&h, x, 0)).collect();
    assert_eq!(before, after, "das Bild sieht gleich aus");
    h.state_mut().undo();
    assert_eq!(h.state().sprite().palette, name0);
    assert_eq!((0..5).map(|x| rgb_at(&h, x, 0)).collect::<Vec<_>>(), before);
}

#[test]
fn palette_zuweisen_behaelt_das_aussehen() {
    let mut h = app();
    h.state_mut().project.sprite_mut().active().set(3, 3, 2);
    let look = rgb_at(&h, 3, 3);
    let other = spritebit_core::builtin::BUILTIN.iter().map(|(n, _)| *n).find(|n| *n != h.state().sprite().palette).unwrap();
    h.state_mut().assign_palette(other, true);
    assert_eq!(h.state().sprite().palette, other);
    assert_eq!(rgb_at(&h, 3, 3), look);
}

#[test]
fn bild_zu_palette_reduziert() {
    let mut h = app();
    for (x, c) in [[250u8, 0, 0], [240, 0, 0], [0, 0, 250]].iter().enumerate() {
        let v = h.state_mut().project.sprite_mut().free_color(*c);
        h.state_mut().project.sprite_mut().active().set(x as u32, 0, v);
    }
    h.get_by_label("Bild » Palette …").click();
    h.run();
    if let Some(r) = &mut h.state_mut().pal.reduce {
        r.count = 2;
    }
    h.run();
    h.get_by_label("Palette anlegen").click();
    h.run();
    let a = h.state();
    assert!(a.sprite().palette.starts_with("foto"));
    assert_eq!(a.project.current_palette().len(), 2);
    assert!(px(&h, 0, 0) < spritebit_core::FREE_BASE && px(&h, 0, 0) == px(&h, 1, 0));
}

#[test]
fn hilfslinie_ziehen_und_hinausziehen_loescht() {
    let mut h = app();
    h.get_by_label("Hilfslinien").click();
    h.run();
    h.get_by_label("+ Waagerecht").click();
    h.run();
    assert_eq!(h.state().sprite().guides.h, vec![32]);
    // Mit der Hand die Linie bei y = 32 auf y = 10 ziehen.
    h.state_mut().tool = tools_ui::Tool::Pan;
    let pan = h.state().pan;
    drag(&mut h, (5.0, 31.6), (5.0, 9.6));
    assert_eq!(h.state().sprite().guides.h, vec![10]);
    assert_eq!(h.state().pan, pan, "auf der Linie zieht die Hand die Linie, nicht die Ansicht");
    // Aus dem Bild ziehen löscht sie.
    drag(&mut h, (5.0, 9.6), (5.0, -6.0));
    assert!(h.state().sprite().guides.h.is_empty());
}

#[test]
fn gesperrte_hilfslinien_bleiben_stehen() {
    let mut h = app();
    h.state_mut().guides.show = true;
    h.state_mut().project.sprite_mut().guides.h = vec![32];
    h.get_by_label("Hilfslinien").click();
    h.run();
    h.get_by_label("Sperren").click();
    h.run();
    assert!(h.state().guides.locked);
    h.state_mut().tool = tools_ui::Tool::Pan;
    drag(&mut h, (5.0, 31.6), (5.0, 9.6));
    assert_eq!(h.state().sprite().guides.h, vec![32], "gesperrt");
    h.get_by_label("Entsperren").click();
    h.run();
    drag(&mut h, (5.0, 31.6), (5.0, 9.6));
    assert_eq!(h.state().sprite().guides.h, vec![10], "wieder frei");
}

#[test]
fn kopfhoehe_zieht_die_ganze_figur() {
    let mut h = app();
    h.state_mut().guides.show = true;
    {
        let g = &mut h.state_mut().project.sprite_mut().guides;
        (g.heads, g.top, g.bottom) = (4, 8, 48);
    }
    h.run();
    h.state_mut().tool = tools_ui::Tool::Pan;
    // Zweite Kopfhöhe liegt bei 8 + 2 × 10 = 28; 6 Pixel nach unten.
    drag(&mut h, (5.0, 27.8), (5.0, 33.8));
    let g = &h.state().sprite().guides;
    assert_eq!((g.top, g.bottom), (14, 54), "verschoben, Größe bleibt");
    // Nicht über den Rand hinaus.
    drag(&mut h, (5.0, 33.8), (5.0, 63.8));
    let g = &h.state().sprite().guides;
    assert_eq!((g.top, g.bottom), (24, 64));
}

#[test]
fn schablone_laden_uebernehmen_und_verschieben() {
    let mut h = app();
    // Ein echtes PNG durch den Lader: 2×1, links weiß, rechts schwarz.
    let mut sp = spritebit_core::Sprite::new("t", 2, 1).unwrap();
    sp.active().set(0, 0, 1);
    sp.active().set(1, 0, 5);
    let png = spritebit_core::export::png(&sp, &h.state().project.current_palette(), 0, 1).unwrap();
    let t = template_ui::decode(&png).unwrap();
    let ctx = h.ctx.clone();
    h.state_mut().set_template(&ctx, t, "test.png".into());
    h.run();
    h.get_by_label("Schablone").click();
    h.run();
    h.get_by_label("Palettenfarben").click();
    h.run();
    // Fläche 64×64, Bild 2:1 eingepasst → Zeilen 16..48 belegt.
    assert_eq!(px(&h, 10, 30), 1);
    assert_eq!(px(&h, 50, 30), 5);
    assert_eq!(px(&h, 10, 5), 0, "Rand bleibt leer");
    // Umschalt+Alt ziehen verschiebt — gemalt wird dabei nicht.
    let before = h.state().template.offset;
    let (a, b) = (at(&h, 20.0, 20.0), at(&h, 30.0, 20.0));
    let m = Modifiers::SHIFT | Modifiers::ALT;
    h.hover_at(a);
    h.run();
    h.event_modifiers(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: m }, m);
    h.run();
    h.event_modifiers(egui::Event::PointerMoved(b), m);
    h.run();
    h.event_modifiers(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: m }, m);
    h.run();
    let after = h.state().template.offset;
    assert!((after.0 - before.0 - 10.0).abs() < 0.5, "{after:?}");
}

#[test]
fn code_importieren_als_neuer_sprite() {
    let mut h = app();
    h.state_mut().out.import = Some(export_ui::ImportModal {
        text: "export const HELD_PALETTE = { 1: '#ff0000', 2: '#00ff00' };
export const HELD = [[0,1],[2,1]];"
            .into(),
        use_palette: true,
    });
    h.run();
    h.get_by_label("Als neuen Sprite").click();
    h.run();
    let a = h.state();
    assert_eq!(a.project.sprites.len(), 2);
    assert_eq!((a.sprite().name.as_str(), a.sprite().width, a.sprite().height), ("HELD", 2, 2));
    assert_eq!(a.sprite().palette, "held2", "„held“ ist eine eingebaute Palette");
    assert_eq!(a.project.current_palette().colors, vec![[255, 0, 0], [0, 255, 0]]);
    assert_eq!(px(&h, 1, 0), 1);
    assert_eq!(px(&h, 0, 1), 2);
}

#[test]
fn code_panel_zeigt_den_code_und_menue_klappt_es_auf() {
    let mut h = app();
    h.state_mut().project.sprite_mut().active().set(0, 0, 3);
    h.state_mut().open_export();
    h.run();
    h.run();
    // Das Panel ist offen: seine Knöpfe sind da.
    h.get_by_label("Palette in den Code schreiben");
    h.get_by_label("PDF");
}

#[test]
fn timeline_einstellungen_links_und_onion() {
    let mut h = app();
    h.get_by_label("Timeline-Einstellungen — Lage, Kopfzeile, Dauer, Onion Skin").click();
    h.run();
    h.get_by_label("Links").click();
    h.run();
    assert_eq!(h.state().tl.zone, tlmenu_ui::Zone::Left);
    h.state_mut().tl_menu_open = false;
    h.run();
    h.get_by_label("Leerer Frame dahinter").click();
    h.run();
    assert_eq!(h.state().sprite().frames.len(), 2, "Timeline arbeitet auch links");
    // Onion Skin an: der Nachbar-Frame wird eine eigene Textur.
    h.state_mut().project.sprite_mut().cel_mut(0, 0).set(1, 1, 5);
    h.state_mut().onion = true;
    h.state_mut().changed();
    let ctx = h.ctx.clone();
    let r = spritebit_core::Rect { x: 0, y: 0, w: 64, h: 64 };
    assert!(h.state_mut().onion_texture(&ctx, r, 1).is_some());
    h.state_mut().onion = false;
    assert!(h.state_mut().onion_texture(&ctx, r, 1).is_none());
}

#[test]
fn ziffern_waehlen_die_farbe_und_strg_rad_zoomt() {
    let mut h = app();
    h.key_press(Key::Num3);
    h.run();
    assert_eq!(h.state().color, 3);
    h.key_press(Key::Num0);
    h.run();
    assert_eq!(h.state().color, 0);
    // Mausrad ohne Strg scrollt nur.
    let z = h.state().zoom;
    let p = at(&h, 10.0, 10.0);
    h.hover_at(p);
    h.run();
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -40.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    });
    h.run();
    assert_eq!(h.state().zoom, z, "kein Zoom ohne Strg");
}

#[test]
fn mausrad_ueber_einem_fenster_scrollt_nicht_die_flaeche() {
    let mut h = app();
    h.state_mut().open_new_sprite();
    h.run();
    let p = h.get_by_label("Erstellen").rect().center();
    assert!(h.state().canvas_rect.contains(p), "das Fenster liegt über der Fläche");
    h.hover_at(p);
    h.run();
    let pan = h.state().pan;
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -120.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    });
    h.run();
    assert_eq!(h.state().pan, pan, "die Fläche dahinter bleibt stehen");
}

#[test]
fn alt_ziehen_verschiebt_eine_kopie() {
    let mut h = app();
    h.state_mut().tool = tools_ui::Tool::Select;
    h.state_mut().project.sprite_mut().active().set(5, 5, 4);
    h.state_mut().selection = Some(Selection::rect(5, 5, 5, 5));
    let (a, b) = (at(&h, 5.0, 5.0), at(&h, 9.0, 5.0));
    h.hover_at(a);
    h.run();
    h.event_modifiers(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::ALT }, Modifiers::ALT);
    h.run();
    for k in 1..=4 {
        h.event_modifiers(egui::Event::PointerMoved(a + (b - a) * (k as f32 / 4.0)), Modifiers::ALT);
        h.run();
    }
    h.event_modifiers(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::ALT }, Modifiers::ALT);
    h.run();
    h.state_mut().deselect();
    assert_eq!(px(&h, 5, 5), 4, "Original bleibt");
    assert_eq!(px(&h, 9, 5), 4, "Kopie verschoben");
}

#[test]
fn markierte_frames_loeschen_und_farbe_zeigen() {
    let mut h = app();
    for _ in 0..3 {
        h.get_by_label("Leerer Frame dahinter").click();
        h.run();
    }
    assert_eq!(h.state().sprite().frames.len(), 4);
    h.state_mut().frame_sel = vec![1, 2];
    h.get_by_label("Frame löschen (markierte alle)").click();
    h.run();
    assert_eq!(h.state().sprite().frames.len(), 2);
    // Farbe zeigen: zählt die Pixel der aktuellen Farbe.
    h.state_mut().project.sprite_mut().active().set(0, 0, 5);
    h.state_mut().project.sprite_mut().active().set(1, 0, 5);
    h.state_mut().color = 5;
    assert_eq!(h.state().count_current_color(), 2);
    h.state_mut().color = 0;
    assert_eq!(h.state().count_current_color(), 64 * 64 - 2);
}

#[test]
fn enter_uebernimmt_die_freie_drehung() {
    let mut h = app();
    h.state_mut().project.sprite_mut().active().set(32, 10, 5);
    h.state_mut().image.angle = 90.0;
    h.get_by_label("Bild").click(); // zu- und wieder aufklappen ist egal
    h.run();
    h.state_mut().flip(true); // ein beliebiger Schritt davor
    h.state_mut().image.live = None;
    let before = h.state().sprite().cel(0, 0).get(31, 10);
    assert_eq!(before, 5);
    h.state_mut().test_rotate(90.0);
    assert!(h.state().image.live.is_some());
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().image.live.is_none(), "übernommen");
    assert!(!h.state().playing, "nicht abgespielt");
}

#[test]
fn pixel_perfekt_zieht_saubere_diagonalen() {
    let mut h = app();
    h.get_by_label("Clean Stroke").click();
    h.run();
    assert!(h.state().pixel_perfect);
    // Treppe von Hand: (2,2) → (3,2) → (3,3) → (4,3) → (4,4)
    let pts = [(2.0, 2.0), (3.0, 2.0), (3.0, 3.0), (4.0, 3.0), (4.0, 4.0)];
    let a = at(&h, pts[0].0, pts[0].1);
    h.hover_at(a);
    h.run();
    h.drag_at(a);
    h.run();
    for p in &pts[1..] {
        h.hover_at(at(&h, p.0, p.1));
        h.run();
    }
    h.drop_at(at(&h, 4.0, 4.0));
    h.run();
    assert_eq!([px(&h, 2, 2), px(&h, 3, 3), px(&h, 4, 4)], [5, 5, 5]);
    assert_eq!([px(&h, 3, 2), px(&h, 4, 3)], [0, 0], "Eckpixel entfernt");
    h.state_mut().undo();
    assert_eq!(px(&h, 2, 2), 0, "ein Undo-Schritt");
}

#[test]
fn neuer_frame_aus_der_timeline() {
    let mut h = app();
    h.get_by_label("Leerer Frame dahinter").click();
    h.run();
    assert_eq!(h.state().project.sprite().frames.len(), 2);
    assert_eq!(h.state().project.sprite().frame, 1);
}

mod monkey;
mod shot;
