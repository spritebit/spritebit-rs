//! # spritebit-core
//!
//! Datenmodell und Logik von spritebit, ohne Oberfläche. Die Desktop-App
//! (und später die Browser-Version über WebAssembly) bauen darauf auf.
//!
//! * [`image`] — ein Bild in Kacheln: leere Flächen kosten nichts, Kopien
//!   teilen sich die Kacheln (Undo, verknüpfte Zellen)
//! * [`sprite`] — Ebenen × Frames, verknüpfte Zellen, durchgehende Ebenen, Tags
//! * [`palette`] — bis zu 255 Farben, 0 ist transparent
//! * [`composite`] — was man sieht, für einen Ausschnitt (auch ausgedünnt)
//! * [`history`] — Undo/Redo, billig dank geteilter Kacheln
//! * [`tools`] — Werkzeug-Hilfen (Linie)
//! * [`project`] — mehrere Sprites und eigene Paletten
//! * [`io`] — eigenes Format `.spritebit` und Projektdatei der Web-Version
//! * [`selection`] — Auswahl (Rechteck, Lasso, Farbe), kopieren, einfügen
//! * [`cels`] — Zellen-Bereiche der Timeline: verschieben, kopieren, verknüpfen
//! * [`cleanup`] — Hintergrund entfernen, glätten, Outline, Median-Cut
//! * [`export`] — PNG, GIF, Spritesheet mit JSON-Atlas
//! * [`transform`] — spiegeln, drehen, zuschneiden, Leinwand, skalieren
//! * [`builtin`] — die eingebauten Paletten der Web-Version

pub mod builtin;
pub mod cels;
pub mod cleanup;
pub mod composite;
pub mod export;
pub mod history;
pub mod image;
pub mod io;
pub mod palette;
pub mod project;
pub mod selection;
pub mod sprite;
pub mod transform;
pub mod tools;

pub use composite::{render_rgba, render_rgba_step, Rect};
pub use history::History;
pub use image::{Image, Px, FREE_BASE, TILE};
pub use io::{export_web, import_web, load_native, save_native, IoError};
pub use palette::{Palette, Rgb};
pub use project::Project;
pub use selection::{Clip, Selection};
pub use sprite::{Direction, Frame, Layer, Sprite, SpriteError, Tag, MAX_SIDE};
