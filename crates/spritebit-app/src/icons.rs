//! Linien-Icons — dieselben wie in der Web-Version.
//!
//! Erzeugt aus sprites-editor/js/icons.js (dort ändern und neu erzeugen).
//! Gezeichnet in Weiß; egui färbt sie beim Anzeigen in die Textfarbe ein
//! (Image::tint).

// Die Liste ist vollständig erzeugt — nicht jedes Icon ist schon in Gebrauch.
#![allow(dead_code)]

use eframe::egui::{self, include_image, Color32, ImageSource, Response, Ui, Vec2};

pub const BRUSH: ImageSource<'static> = include_image!("../assets/icons/brush.svg");
pub const CLEANUP: ImageSource<'static> = include_image!("../assets/icons/cleanup.svg");
pub const CLOSE: ImageSource<'static> = include_image!("../assets/icons/close.svg");
pub const COLORS: ImageSource<'static> = include_image!("../assets/icons/colors.svg");
pub const CONT_OFF: ImageSource<'static> = include_image!("../assets/icons/contOff.svg");
pub const CONT_ON: ImageSource<'static> = include_image!("../assets/icons/contOn.svg");
pub const COPY: ImageSource<'static> = include_image!("../assets/icons/copy.svg");
pub const CUT: ImageSource<'static> = include_image!("../assets/icons/cut.svg");
pub const DESELECT: ImageSource<'static> = include_image!("../assets/icons/deselect.svg");
pub const ELLIPSE: ImageSource<'static> = include_image!("../assets/icons/ellipse.svg");
pub const ERASER: ImageSource<'static> = include_image!("../assets/icons/eraser.svg");
pub const EXPAND: ImageSource<'static> = include_image!("../assets/icons/expand.svg");
pub const EYE: ImageSource<'static> = include_image!("../assets/icons/eye.svg");
pub const EYE_OFF: ImageSource<'static> = include_image!("../assets/icons/eyeOff.svg");
pub const FILL: ImageSource<'static> = include_image!("../assets/icons/fill.svg");
pub const FIRST: ImageSource<'static> = include_image!("../assets/icons/first.svg");
pub const FLOAT: ImageSource<'static> = include_image!("../assets/icons/float.svg");
pub const FOLDER: ImageSource<'static> = include_image!("../assets/icons/folder.svg");
pub const FRAMES: ImageSource<'static> = include_image!("../assets/icons/frames.svg");
pub const GRIP: ImageSource<'static> = include_image!("../assets/icons/grip.svg");
pub const GUIDES: ImageSource<'static> = include_image!("../assets/icons/guides.svg");
pub const HAND: ImageSource<'static> = include_image!("../assets/icons/hand.svg");
pub const HELP: ImageSource<'static> = include_image!("../assets/icons/help.svg");
pub const IMAGE: ImageSource<'static> = include_image!("../assets/icons/image.svg");
pub const INSTALL: ImageSource<'static> = include_image!("../assets/icons/install.svg");
pub const LASSO: ImageSource<'static> = include_image!("../assets/icons/lasso.svg");
pub const LAST: ImageSource<'static> = include_image!("../assets/icons/last.svg");
pub const LAYERS: ImageSource<'static> = include_image!("../assets/icons/layers.svg");
pub const LINE: ImageSource<'static> = include_image!("../assets/icons/line.svg");
pub const LINK: ImageSource<'static> = include_image!("../assets/icons/link.svg");
pub const LOCK: ImageSource<'static> = include_image!("../assets/icons/lock.svg");
pub const MAGIC: ImageSource<'static> = include_image!("../assets/icons/magic.svg");
pub const MENU: ImageSource<'static> = include_image!("../assets/icons/menu.svg");
pub const MERGE_DOWN: ImageSource<'static> = include_image!("../assets/icons/mergeDown.svg");
pub const MIRROR_X: ImageSource<'static> = include_image!("../assets/icons/mirrorX.svg");
pub const MIRROR_Y: ImageSource<'static> = include_image!("../assets/icons/mirrorY.svg");
pub const LEFT: ImageSource<'static> = include_image!("../assets/icons/left.svg");
pub const RIGHT: ImageSource<'static> = include_image!("../assets/icons/right.svg");
pub const UP: ImageSource<'static> = include_image!("../assets/icons/up.svg");
pub const DOWN: ImageSource<'static> = include_image!("../assets/icons/down.svg");
pub const NEXT: ImageSource<'static> = include_image!("../assets/icons/next.svg");
pub const ONION: ImageSource<'static> = include_image!("../assets/icons/onion.svg");
pub const OPEN: ImageSource<'static> = include_image!("../assets/icons/open.svg");
pub const OUTPUT: ImageSource<'static> = include_image!("../assets/icons/output.svg");
pub const PALETTE: ImageSource<'static> = include_image!("../assets/icons/palette.svg");
pub const PASTE: ImageSource<'static> = include_image!("../assets/icons/paste.svg");
pub const PAUSE: ImageSource<'static> = include_image!("../assets/icons/pause.svg");
pub const PENCIL: ImageSource<'static> = include_image!("../assets/icons/pencil.svg");
pub const PICKER: ImageSource<'static> = include_image!("../assets/icons/picker.svg");
pub const PIN: ImageSource<'static> = include_image!("../assets/icons/pin.svg");
pub const PLAY: ImageSource<'static> = include_image!("../assets/icons/play.svg");
pub const PLUS: ImageSource<'static> = include_image!("../assets/icons/plus.svg");
pub const PREV: ImageSource<'static> = include_image!("../assets/icons/prev.svg");
pub const PREVIEW: ImageSource<'static> = include_image!("../assets/icons/preview.svg");
pub const RECT: ImageSource<'static> = include_image!("../assets/icons/rect.svg");
pub const REDO: ImageSource<'static> = include_image!("../assets/icons/redo.svg");
pub const RESET: ImageSource<'static> = include_image!("../assets/icons/reset.svg");
pub const SAVE: ImageSource<'static> = include_image!("../assets/icons/save.svg");
pub const SEL_ALL: ImageSource<'static> = include_image!("../assets/icons/selAll.svg");
pub const SELECT: ImageSource<'static> = include_image!("../assets/icons/select.svg");
pub const SHADES: ImageSource<'static> = include_image!("../assets/icons/shades.svg");
pub const SHRINK: ImageSource<'static> = include_image!("../assets/icons/shrink.svg");
pub const SLIDERS: ImageSource<'static> = include_image!("../assets/icons/sliders.svg");
pub const SPOT: ImageSource<'static> = include_image!("../assets/icons/spot.svg");
pub const SPRAY: ImageSource<'static> = include_image!("../assets/icons/spray.svg");
pub const SPRITES: ImageSource<'static> = include_image!("../assets/icons/sprites.svg");
pub const TAG: ImageSource<'static> = include_image!("../assets/icons/tag.svg");
pub const TEMPLATE: ImageSource<'static> = include_image!("../assets/icons/template.svg");
pub const TOOLS: ImageSource<'static> = include_image!("../assets/icons/tools.svg");
pub const TRASH: ImageSource<'static> = include_image!("../assets/icons/trash.svg");
pub const UNDO: ImageSource<'static> = include_image!("../assets/icons/undo.svg");
pub const UNLINK: ImageSource<'static> = include_image!("../assets/icons/unlink.svg");
pub const UNLOCK: ImageSource<'static> = include_image!("../assets/icons/unlock.svg");
pub const WAND: ImageSource<'static> = include_image!("../assets/icons/wand.svg");

/// Kantenlänge der Icons in Knöpfen.
pub const SIZE: f32 = 16.0;

/// Icon in fester Größe, in `color` eingefärbt.
pub fn image(src: ImageSource<'static>, color: Color32) -> egui::Image<'static> {
    egui::Image::new(src).fit_to_exact_size(Vec2::splat(SIZE)).tint(color)
}

/// Knopf nur mit Icon; der Name steht im Tooltip.
pub fn button(ui: &mut Ui, src: ImageSource<'static>, tip: &str, enabled: bool) -> Response {
    let color = ui.visuals().text_color();
    // alt_text: der Knopf hat damit einen Namen — für Screenreader und Tests.
    ui.add_enabled(enabled, egui::Button::image(image(src, color).alt_text(tip))).on_hover_text(tip)
}
