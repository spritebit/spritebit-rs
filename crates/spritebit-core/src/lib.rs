//! # spritebit-core
//!
//! Datenmodell und Logik von spritebit, ohne Oberfläche. Die Desktop-App
//! (und später die Browser-Version über WebAssembly) bauen darauf auf.
//!
//! * [`image`] — ein Bild in Kacheln: leere Flächen kosten nichts, Kopien
//!   teilen sich die Kacheln (Undo, verknüpfte Zellen)
//! * [`sprite`] — Ebenen × Frames, verknüpfte Zellen, durchgehende Ebenen, Tags
//! * [`palette`] — bis zu 255 Farben, 0 ist transparent
//! * [`composite`] — was man sieht, für einen Ausschnitt

pub mod composite;
pub mod image;
pub mod palette;
pub mod sprite;

pub use composite::{render_rgba, Rect};
pub use image::{Image, Px, FREE_BASE, TILE};
pub use palette::{Palette, Rgb};
pub use sprite::{Direction, Frame, Layer, Sprite, SpriteError, Tag, MAX_SIDE};
