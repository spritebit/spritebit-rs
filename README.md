# spritebit (Rust)

Neubau von [spritebit](https://www.spritebit.at) als Desktop-Programm in Rust —
später auch als Browser-Version über WebAssembly aus derselben Codebasis.

© 2026 Marco Jan — alle Rechte vorbehalten.

## Aufbau

```
crates/
  spritebit-core   Datenmodell und Logik, ohne Oberfläche
  spritebit-app    Oberfläche (egui/eframe) — folgt
```

### Große Zeichenflächen

Bilder liegen in Kacheln von 64×64 Pixeln (`core::image`). Eine Kachel gibt es
erst, wenn auf ihr gemalt wird — eine 8192×8192-Fläche mit einer kleinen Figur
belegt nur ein paar Kilobyte. Kopien eines Bildes teilen sich die Kacheln, bis
eine davon geändert wird (Copy-on-Write); darauf bauen Undo und verknüpfte
Zellen auf. Gezeichnet wird immer nur der sichtbare Ausschnitt
(`core::composite`). Die Grenze steht in `core::sprite::MAX_SIDE` (8192).

## Entwickeln

Voraussetzung: Rust über [rustup](https://rustup.rs) (unter Windows zusätzlich
die C++-Build-Tools von Visual Studio).

```
cargo test        # alle Tests
cargo build       # bauen
```
