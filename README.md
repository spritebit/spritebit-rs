# spritebit (Rust)

Neubau von [spritebit](https://www.spritebit.at) als Desktop-Programm in Rust —
später auch als Browser-Version über WebAssembly aus derselben Codebasis.

© 2026 Marco Jan — alle Rechte vorbehalten.

## Aufbau

```
crates/
  spritebit-core   Datenmodell und Logik, ohne Oberfläche
  spritebit-app    Oberfläche (egui/eframe 0.36)
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
cargo test                      # alle Tests (Core und Oberfläche)
cargo run -p spritebit-app      # Programm starten
cargo build --release           # schnelle Fassung (target/release)
```

## Bedienung (erste Fassung)

| | |
|---|---|
| Linke Maustaste | malen mit dem gewählten Werkzeug |
| Rechte Maustaste | radieren (mit jedem Malwerkzeug) |
| H P B S F E I R O | Hand, Stift, Pinsel, Spray, Füllen, Radierer, Linie, Rechteck, Ellipse |
| A L K W | Auswahl, Lasso, Farbwahl, Zauberstab |
| Strg+A / C / X / V, Entf, Esc, Pfeile | Alles, Kopieren, Ausschneiden, Einfügen, Leeren, Aufheben, Verschieben |
| Mausrad / Strg+Mausrad | zoomen um den Mauszeiger |
| Mittlere Taste oder Leertaste + Ziehen | Fläche verschieben |
| Strg+Z / Strg+Y | Rückgängig / Wiederholen (je Sprite) |
| Strg+O / Strg+S / Strg+Umschalt+S | Öffnen / Speichern / Speichern unter |
| Datei → Neuer Sprite … | Größe bis 8192 × 8192 |
| Datei → Als Web-Projekt exportieren … | Projektdatei für die Web-Version |
| Ansicht | Einpassen, 100 %, Gitter |
| Timeline | Zelle anklicken wählt Frame und Ebene; Auge, Schloss, Kette schalten Sichtbarkeit, Sperre, „durchgehend“ |
| Enter / , / . / Pos1 / Ende | Abspielen, voriger, nächster, erster, letzter Frame |

Die Statusleiste zeigt, wie viele Kacheln tatsächlich Speicher belegen.

## Tests

* `spritebit-core` — Modell, Formate, Werkzeuge; dazu ein Test gegen eine
  Projektdatei, die mit der Web-Version erzeugt wurde.
* `spritebit-app` — Oberflächen-Tests mit `egui_kittest`: die App läuft ohne
  Fenster, Maus und Tasten werden simuliert, geprüft wird das Bild im Sprite.

## Dateiformate

* **`.spritebit`** — eigenes Format: Kopf als JSON, danach nur die bemalten
  Kacheln, alles zlib-komprimiert. Eine große, kaum bemalte Fläche bleibt klein.
* **Web-Projekt (`.json`)** — die Projektdatei der Web-Version (Schema 2) lässt
  sich öffnen und exportieren. `crates/spritebit-core/tests/fixtures/web-projekt.json`
  wurde mit den Funktionen der Web-Version erzeugt; der Test dazu fällt auf, wenn
  sich eines der Formate verschiebt.
