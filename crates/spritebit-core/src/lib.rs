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

pub mod composite;
pub mod history;
pub mod image;
pub mod palette;
pub mod sprite;
pub mod tools;

pub use composite::{render_rgba, render_rgba_step, Rect};
pub use history::History;
pub use image::{Image, Px, FREE_BASE, TILE};
pub use palette::{Palette, Rgb};
pub use sprite::{Direction, Frame, Layer, Sprite, SpriteError, Tag, MAX_SIDE};
