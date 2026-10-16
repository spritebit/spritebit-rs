//! Körperteile und „Bewegung erzeugen“ — die Oberfläche zu
//! [`spritebit_core::rig`], wie in der Web-Version (js/layers.js, js/rigui.js).
//!
//! Drei Stücke, die zusammengehören:
//!
//! * die Zeile im Ebenen-Panel: Rolle der aktiven Ebene, Gelenk setzen,
//!   „Hinten“ (hinteres Glied aus dem vorderen) und „Erzeugen“
//! * die gelben Ringe auf der Zeichenfläche — jedes Gelenk der Figur, das
//!   der aktiven Ebene kräftig
//! * das Fenster „Bewegung erzeugen“: Vorlage, Bildzahl, Stärke, Vorschau
//!
//! Wohin die Frames kommen: hinten angehängt, mit einem Tag, der wie die
//! Bewegung heißt. Gibt es den Tag schon, werden SEINE Frames ersetzt — man
//! kann die Stärke also durchprobieren, ohne hinterher aufzuräumen. Der
//! Grundframe bleibt unangetastet, alles zusammen ist ein Undo-Schritt.

use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
use spritebit_core::rig::{self, Part, Role, Template};
use spritebit_core::sprite::{Direction, Tag};

use crate::i18n::{tr, trf};
use crate::{icons, SpritebitApp};

/// Dieselbe Farbe wie der Ring auf der Fläche.
const PIVOT: Color32 = Color32::from_rgb(255, 204, 51);

/// Das offene Fenster „Bewegung erzeugen“.
pub(crate) struct RigDialog {
    pub template: Template,
    pub count: usize,
    /// In Prozent, 25–200.
    pub strength: u32,
    /// Welcher Frame der Vorschau gerade dran ist.
    pub at: usize,
    /// Zeitpunkt (egui-Zeit), seit dem er steht.
    pub since: f64,
    /// Die Textur der Vorschau. Sie MUSS hier liegen bleiben: egui gibt eine
    /// Textur frei, sobald die letzte Handle fällt — eine, die nur in der
    /// Zeichenfunktion lebt, ist beim Zeichnen schon wieder weg, und es
    /// bliebe ein leeres Feld.
    tex: Option<egui::TextureHandle>,
}

impl RigDialog {
    /// Für den Bild-Test (tests/shot.rs): ein fester Stand ohne Zufall.
    #[cfg(test)]
    pub(crate) fn for_shot() -> Self {
        RigDialog { template: Template::Walk, count: 8, strength: 120, at: 2, since: f64::INFINITY, tex: None }
    }

    fn new() -> Self {
        RigDialog { template: Template::Walk, count: Template::Walk.frames(), strength: 100, at: 0, since: 0.0, tex: None }
    }
}

/// Der Name einer Rolle in der Sprache des Nutzers.
pub(crate) fn role_name(r: Role) -> &'static str {
    tr(match r {
        Role::Head => "Kopf",
        Role::Torso => "Rumpf",
        Role::ArmBack => "Arm hinten",
        Role::ArmFront => "Arm vorne",
        Role::LegBack => "Bein hinten",
        Role::LegFront => "Bein vorne",
        Role::Weapon => "Waffe",
        Role::Free => "Frei",
    })
}

/// Der Name einer Bewegung — er wird auch der Name des Tags.
pub(crate) fn template_name(t: Template) -> &'static str {
    tr(match t {
        Template::Idle => "Stehen",
        Template::Walk => "Laufen",
        Template::WalkQuad => "Laufen (Vierbeiner)",
        Template::Run => "Rennen",
        Template::Jump => "Sprung",
        Template::Attack => "Schlag",
        Template::Hit => "Treffer",
        Template::Death => "Tod",
    })
}

impl SpritebitApp {
    /// Die Zeile „Körperteil“ im Ebenen-Panel.
    pub(crate) fn part_row(&mut self, ui: &mut egui::Ui) {
        let (l, part) = {
            let sp = self.project.sprite();
            (sp.layer, sp.layers[sp.layer].part)
        };
        let mut role = part.map(|p| p.role);
        egui::ComboBox::from_id_salt("ly-part-role")
            .selected_text(role.map_or("—", role_name))
            .width(130.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut role, None, "—");
                for r in rig::ROLES {
                    ui.selectable_value(&mut role, Some(r), role_name(r));
                }
            })
            .response
            .on_hover_text(tr("Welches Körperteil diese Ebene ist — davon hängt ab, wie es sich später bewegt"));

        if role != part.map(|p| p.role) {
            self.set_part_role(l, role);
        }

        if let Some(p) = self.project.sprite().layers[l].part {
            let picking = self.pivot_pick;
            let c = ui.visuals().text_color();
            let b = egui::Button::selectable(picking, icons::image(icons::PIVOT, if picking { PIVOT } else { c })).wrap_mode(egui::TextWrapMode::Extend);
            let fmt = |v: f64| if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.1}") };
            if ui
                .add(b)
                .on_hover_text(trf("Gelenk sitzt bei {x} / {y} — auf die Fläche klicken setzt es neu", &[("x", &fmt(p.pivot.0)), ("y", &fmt(p.pivot.1))]))
                .clicked()
            {
                self.pivot_pick = !picking;
                if self.pivot_pick {
                    self.hint = Some(tr("Jetzt auf die Fläche klicken, wo das Teil angewachsen ist — Schulter, Hüfte oder Hals.").into());
                }
            }
            // „Hinten“ nur bei einem vorderen Glied, und nur solange das hintere fehlt.
            if let Some(back) = p.role.back_of() {
                if !self.project.sprite().layers.iter().any(|x| x.part.is_some_and(|q| q.role == back)) {
                    let tip = tr("Daraus das hintere Glied machen: dieselbe Form eine Stufe dunkler, ganz unten im Stapel — so bekommt die Figur Tiefe");
                    if icons::button(ui, icons::BACK_LIMB, tip, true).clicked() {
                        self.back_limb_from_front(l, back);
                    }
                }
            }
        }

        let has = rig::has_rig(self.project.sprite());
        let tip = tr("Aus den Körperteilen eine Bewegung rechnen — Laufen, Schlag, Sprung und mehr. Die Frames kommen als neuer Tag in die Timeline.");
        if icons::button(ui, icons::RUN, tip, has).clicked() {
            self.playing = false;
            self.commit_float();
            self.rig_dialog = Some(RigDialog::new());
        }
        if !has {
            ui.weak(tr("Erst Teile herauslösen und ihnen eine Rolle geben."));
        }
    }

    /// Auswahl in eine eigene Ebene: der gewählte Teil wird aus der aktiven
    /// Ebene HERAUSGENOMMEN und sitzt danach allein in einer neuen Ebene
    /// darüber, an genau derselben Stelle. Damit zerlegt man eine fertig
    /// gezeichnete Figur in Kopf, Rumpf, Arme und Beine.
    ///
    /// Nur im aktuellen Frame: man zerlegt ein Bild, nicht eine Animation.
    /// Teilt die Quellzelle ihr Bild mit anderen Frames (verknüpft), wird sie
    /// für diesen Frame zuerst gelöst — sonst fehlte das Teil überall.
    pub(crate) fn layer_from_selection(&mut self) {
        let Some(sel) = self.selection.clone() else {
            self.hint = Some(tr("Erst einen Teil auswählen — mit Lasso, Rechteck oder Zauberstab.").into());
            return;
        };
        // Schwebendes zuerst absetzen: der Inhalt gehört ins Raster, aus dem
        // er kam, bevor er herausgenommen wird.
        self.commit_float();
        let (from, f) = {
            let sp = self.project.sprite();
            (sp.layer, sp.frame)
        };
        let empty = {
            let sp = self.project.sprite();
            spritebit_core::selection::copy(sp.cel(f, from), &sel).data.iter().all(|&v| v == 0)
        };
        if empty {
            self.hint = Some(tr("In der Auswahl ist nichts — dort sind alle Pixel leer.").into());
            return;
        }
        let name = trf("Teil {n}", &[("n", &(self.project.sprite().layers.len() + 1))]);
        self.edit_sprite(|s| {
            if s.is_linked(f, from) {
                s.unlink(f, from);
            }
            let clip = spritebit_core::selection::copy(s.cel(f, from), &sel);
            spritebit_core::selection::clear(s.cel_mut(f, from), &sel);
            let at = from + 1;
            s.add_layer(at, name);
            spritebit_core::selection::paste(s.cel_mut(f, at), &clip, sel.x, sel.y);
            s.layer = at;
        });
        self.deselect();
    }

    /// Ausschlag und Takt — nur beim freien Teil (Schwanz, Ohr, Umhang):
    /// dort bringt die Ebene ihre Bewegung selbst mit. Eine eigene Zeile,
    /// damit die Regler breit genug bleiben, um sie zu treffen.
    pub(crate) fn free_swing_row(&mut self, ui: &mut egui::Ui) {
        let l = self.project.sprite().layer;
        let Some(part) = self.project.sprite().layers[l].part.filter(|p| p.role == Role::Free) else { return };
        let (mut amp, mut phase) = (part.amp, part.phase);
        // Nicht umbrechen: im schmalen Panel stünde sonst „Im“ in der einen
        // und „Takt“ in der nächsten Zeile.
        let label = |ui: &mut egui::Ui, t: &str| ui.add(egui::Label::new(egui::RichText::new(t).weak()).wrap_mode(egui::TextWrapMode::Extend));
        label(ui, tr("Ausschlag"));
        let a = ui.add(egui::Slider::new(&mut amp, 0.0..=rig::FREE_AMP_MAX).suffix("°").max_decimals(0));
        ui.add_space(8.0);
        label(ui, tr("Im Takt"));
        let b = ui
            .add(egui::Slider::new(&mut phase, 0.0..=345.0).step_by(15.0).suffix("°").max_decimals(0))
            .on_hover_text(tr("Wann im Takt: 0° mit den vorderen Beinen, 180° dagegen, 90° dazwischen. Bei Sprung, Schlag, Treffer und Tod ohne Bedeutung — dort gibt es keinen Takt."));
        // Ein Zug am Regler ist EIN Undo-Schritt, nicht einer je Zwischenwert.
        if a.drag_started() || b.drag_started() || ((a.changed() || b.changed()) && !a.dragged() && !b.dragged()) {
            self.edit_sprite(|_| {});
        }
        if a.changed() || b.changed() {
            if let Some(p) = self.project.sprite_mut().layers[l].part.as_mut() {
                p.amp = amp;
                p.phase = phase;
            }
            self.changed();
        }
    }

    /// Rolle vergeben oder wegnehmen. Eine frische Rolle bekommt gleich ein
    /// geratenes Gelenk (oben bei Armen und Beinen, unten bei Kopf, Rumpf und
    /// Waffe) — das stimmt bei den meisten gezeichneten Figuren.
    fn set_part_role(&mut self, l: usize, role: Option<Role>) {
        self.pivot_pick = false;
        self.edit_sprite(|s| match role {
            None => s.layers[l].part = None,
            Some(r) => {
                let pivot = s.layers[l]
                    .part
                    .map(|p| p.pivot)
                    .or_else(|| rig::auto_pivot(s.cel(s.frame, l), r))
                    .unwrap_or((s.width as f64 / 2.0, s.height as f64 / 2.0));
                s.layers[l].part = Some(Part { role: r, pivot, ..s.layers[l].part.unwrap_or(Part::new(r, pivot)) }.clamped(s.width, s.height));
            }
        });
    }

    /// Aus einem vorderen Arm oder Bein das hintere: dieselbe Form eine Stufe
    /// dunkler, ganz unten in den Stapel (es liegt hinter ALLEM, auch hinter
    /// dem Rumpf), mit demselben Gelenk — beide hängen an derselben Schulter.
    fn back_limb_from_front(&mut self, l: usize, back: Role) {
        let pal = self.project.current_palette();
        let name = role_name(back);
        self.edit_sprite(|s| {
            let Some(src) = s.layers[l].part else { return };
            // Abgedunkelt wird mit der Automatik des Schattier-Werkzeugs
            // (leere Reihe): passt die dunklere Farbe in die Palette, bekommt
            // sie deren Nummer, sonst wird sie eine freie Farbe.
            let mut shader = spritebit_core::shade::Shader::new(&pal, &[], true, 0);
            let cel = s.cel(s.frame, l).clone();
            let mut free = std::mem::take(&mut s.free);
            let mut dark = spritebit_core::image::Image::new(s.width, s.height);
            for (x, y, v) in cel.pixels() {
                if v != 0 {
                    dark.set(x, y, shader.step(v, &pal, &mut free));
                }
            }
            s.free = free;
            // Unten einfügen: in jedem Frame eine leere Zelle, nur im
            // aktuellen die dunkle Kopie. Die Ebene darüber (das vordere
            // Glied) rutscht dabei auf l + 1.
            s.add_layer(0, name);
            s.layers[0].part = Some(Part::new(back, src.pivot));
            let f = s.frame;
            *s.cel_mut(f, 0) = dark;
            s.layer = 0;
        });
        self.hint = Some(trf("„{name}“ angelegt: dieselbe Form eine Stufe dunkler, ganz unten im Stapel.", &[("name", &role_name(back))]));
    }

    /// Gelenk setzen: der Klick auf die Fläche legt den Drehpunkt fest und
    /// malt nicht. Gerundet auf halbe Zellen — so trifft man die Mitte eines
    /// Pixels ebenso wie die Grenze zwischen zweien.
    pub(crate) fn pivot_pointer(&mut self, pointer: Option<Pos2>, pressed: bool, over: bool, origin: Pos2, zoom: f32) -> bool {
        if !self.pivot_pick {
            return false;
        }
        let sp = self.project.sprite();
        if sp.layers[sp.layer].part.is_none() {
            self.pivot_pick = false;
            return false;
        }
        if !(pressed && over) {
            // Den Zeiger trotzdem festhalten: solange der Modus läuft, wird
            // nicht gemalt, auch nicht beim Ziehen.
            return true;
        }
        let Some(p) = pointer else { return true };
        let (w, h) = (sp.width as f64, sp.height as f64);
        let snap = |v: f32, max: f64| ((v / zoom * 2.0).round() as f64 / 2.0).clamp(0.0, max);
        let pivot = (snap(p.x - origin.x, w), snap(p.y - origin.y, h));
        let l = sp.layer;
        self.edit_sprite(|s| {
            if let Some(part) = s.layers[l].part.as_mut() {
                part.pivot = pivot;
            }
        });
        self.pivot_pick = false;
        true
    }

    /// Die Gelenke aller Körperteile über das Bild zeichnen — das der aktiven
    /// Ebene kräftig, die übrigen blass, damit man sieht, wie die Figur
    /// zusammenhängt.
    pub(crate) fn draw_pivots(&self, painter: &egui::Painter, origin: Pos2, zoom: f32) {
        let sp = self.sprite();
        for (i, p) in rig::rig_parts(sp) {
            let on = i == sp.layer;
            let at = Pos2::new(origin.x + (p.pivot.0 as f32) * zoom, origin.y + (p.pivot.1 as f32) * zoom);
            let r = (zoom * 0.6).clamp(3.0, 7.0);
            let stroke = Stroke::new(if on { 2.0 } else { 1.0 }, if on { PIVOT } else { PIVOT.gamma_multiply(0.45) });
            painter.circle_stroke(at, r, stroke);
            // Kreuz in der Mitte: der Kreis allein ist auf buntem Grund
            // schlecht zu treffen.
            painter.line_segment([Pos2::new(at.x - r, at.y), Pos2::new(at.x + r, at.y)], stroke);
            painter.line_segment([Pos2::new(at.x, at.y - r), Pos2::new(at.x, at.y + r)], stroke);
        }
    }

    /// Das Fenster „Bewegung erzeugen“.
    pub(crate) fn rig_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.rig_dialog.take() else { return };
        let mut open = true;
        let mut make = false;
        // `open` steckt schon im Fenster-Rahmen (das Kreuz); zum Schließen
        // aus dem Inhalt heraus braucht es ein zweites Zeichen.
        let mut cancel = false;
        egui::Window::new(tr("Bewegung erzeugen"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                let parts = rig::rig_parts(self.project.sprite());
                ui.weak(trf(
                    "{n} Körperteile: {list}. Gerechnet wird aus dem Frame, in dem du gerade bist.",
                    &[("n", &parts.len()), ("list", &parts.iter().map(|(_, p)| role_name(p.role)).collect::<Vec<_>>().join(", "))],
                ));
                ui.add_space(6.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.weak(tr("Bewegung"));
                            let before = d.template;
                            egui::ComboBox::from_id_salt("rig-tpl").selected_text(template_name(d.template)).width(170.0).show_ui(ui, |ui| {
                                for t in rig::TEMPLATES {
                                    ui.selectable_value(&mut d.template, t, template_name(t));
                                }
                            });
                            // Die Bildzahl schlägt die Vorlage vor — bis der
                            // Nutzer selbst etwas einstellt.
                            if d.template != before {
                                d.count = d.template.frames();
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.weak(tr("Bilder"));
                            ui.add(egui::DragValue::new(&mut d.count).range(2..=64).speed(0.2));
                        });
                        ui.horizontal(|ui| {
                            ui.weak(tr("Stärke"));
                            ui.add(egui::Slider::new(&mut d.strength, 25..=200).suffix(" %"));
                        });
                    });
                    ui.add_space(10.0);
                    self.rig_preview(ui, &mut d);
                });
                ui.add_space(6.0);
                ui.weak(tr(
                    "Die Frames werden hinten angehängt und bekommen einen Tag mit dem Namen der Bewegung. Gibt es den Tag schon, werden seine Frames ersetzt — so probierst du die Stärke durch. Was dabei herauskommt, ist ein Rohentwurf: danach bessert man von Hand nach.",
                ));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(tr("Abbrechen")).clicked() {
                        cancel = true;
                    }
                    if ui.button(tr("Erzeugen")).clicked() {
                        make = true;
                    }
                });
            });
        if make {
            let (t, n, s) = (d.template, d.count, d.strength as f64 / 100.0);
            self.generate_animation(t, n, s);
            return; // Fenster schließt sich
        }
        if open && !cancel {
            self.rig_dialog = Some(d);
        }
    }

    /// Die laufende Vorschau im Fenster.
    fn rig_preview(&mut self, ui: &mut egui::Ui, d: &mut RigDialog) {
        let now = ui.input(|i| i.time);
        let fps = self.sprite().fps.max(1) as f64;
        if now - d.since > 1.0 / fps {
            d.at += 1;
            d.since = now;
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis((1000.0 / fps) as u64));

        let sp = self.project.sprite();
        let base: Vec<_> = (0..sp.layers.len()).map(|l| sp.cel(sp.frame, l).clone()).collect();
        let frames = rig::build_frames(&base, &sp.layers, d.template, d.count, d.strength as f64 / 100.0);
        if frames.is_empty() {
            return;
        }
        let i = d.at % frames.len();
        // Den gerechneten Frame in einen Sprite hängen, den nur die Vorschau
        // sieht — dann rechnet composite ihn wie jeden anderen.
        let mut shown = sp.clone();
        for (l, img) in frames[i].iter().enumerate() {
            *shown.cel_mut(shown.frame, l) = img.clone();
        }
        let pal = self.project.current_palette();
        let step = shown.width.max(shown.height).div_ceil(160).max(1);
        let rect = spritebit_core::composite::Rect { x: 0, y: 0, w: shown.width, h: shown.height };
        let (rgba, w, h) = spritebit_core::composite::render_rgba_step(&shown, &pal, shown.frame, rect, step);
        let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let tex = d.tex.insert(ui.ctx().load_texture("rig-preview", img, egui::TextureOptions::NEAREST));
        let k = (160.0 / w.max(h) as f32).floor().max(1.0);
        ui.vertical(|ui| {
            // Gemalt wie die Vorschaubilder der Ebenen (layers_ui.rs): Platz
            // nehmen, Karo darunter, Bild darauf.
            let size = Vec2::new(w as f32 * k, h as f32 * k);
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let q = 8.0;
            painter.rect_filled(rect, 2.0, Color32::from_gray(52));
            let (cols, rows) = ((size.x / q).ceil() as i32, (size.y / q).ceil() as i32);
            for i in 0..cols {
                for j in 0..rows {
                    if (i + j) % 2 == 0 {
                        let at = rect.min + Vec2::new(i as f32 * q, j as f32 * q);
                        let cell = egui::Rect::from_min_size(at, Vec2::splat(q)).intersect(rect);
                        painter.rect_filled(cell, 0.0, Color32::from_gray(80));
                    }
                }
            }
            painter.image(tex.id(), rect, egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            ui.weak(trf("Bild {i} von {n}", &[("i", &(i + 1)), ("n", &frames.len())]));
        });
    }

    /// Die Frames der Bewegung anhängen (oder die eines gleichnamigen Tags
    /// ersetzen) und den Tag setzen.
    fn generate_animation(&mut self, tpl: Template, n: usize, strength: f64) {
        let label: &'static str = template_name(tpl);
        let made = {
            let sp = self.project.sprite();
            let base: Vec<_> = (0..sp.layers.len()).map(|l| sp.cel(sp.frame, l).clone()).collect();
            rig::build_frames(&base, &sp.layers, tpl, n, strength)
        };
        if made.is_empty() {
            return;
        }
        let count = made.len();
        let name = label;
        self.edit_sprite(|s| {
            // Lief die Bewegung schon einmal durch, stehen ihre Frames noch
            // da — weg damit, sonst wächst der Sprite bei jedem Versuch. Den
            // Grundframe nie wegwerfen: aus ihm wird gerechnet.
            if let Some(old) = s.tags.iter().find(|t| t.name == name).cloned() {
                for f in (old.from..=old.to).rev() {
                    if f != s.frame && f < s.frames.len() {
                        s.delete_frame(f);
                    }
                }
            }
            // Hinten anhängen verschiebt keine Frame-Nummer, also auch keinen Tag.
            let start = s.frames.len();
            for cels in &made {
                let f = s.frames.len();
                s.add_frame(f.saturating_sub(1), false);
                for (l, img) in cels.iter().enumerate() {
                    *s.cel_mut(f, l) = img.clone();
                }
            }
            let to = start + count - 1;
            // Immer vorwärts: ein Laufzyklus im Hin und Her liefe die zweite
            // Hälfte rückwärts.
            match s.tags.iter_mut().find(|t| t.name == name) {
                Some(t) => {
                    t.from = start;
                    t.to = to;
                }
                None => {
                    let color = crate::timeline::TAG_COLORS[s.tags.len() % crate::timeline::TAG_COLORS.len()];
                    s.tags.push(Tag { name: name.to_string(), from: start, to, color, direction: Direction::Forward });
                }
            }
            s.frame = start;
        });
        self.hint = Some(trf("{n} Frames erzeugt — als Tag „{name}“ in der Timeline.", &[("n", &count), ("name", &label)]));
    }
}
