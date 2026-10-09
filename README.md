# spritebit (Rust)

Neubau von [spritebit](https://spritebit.at) als Desktop-Programm in Rust —
später auch als Browser-Version über WebAssembly aus derselben Codebasis.

[**⬇ Download für Windows**](https://github.com/spritebit/spritebit-rs/releases/latest/download/spritebit-windows-x64.zip) ·
[Alle Versionen](https://github.com/spritebit/spritebit-rs/releases) ·
[Web-Version](https://github.com/spritebit/sprite-editor) ·
[Im Browser öffnen](https://spritebit.at/editor.html)

© 2026 Marco Jan — freie Software unter der [MIT-Lizenz](LICENSE).

## Installieren

ZIP entpacken und `spritebit.exe` starten — keine Installation nötig. Windows
warnt beim ersten Start vor einem unbekannten Herausgeber, weil die Datei nicht
signiert ist: „Weitere Informationen“ → „Trotzdem ausführen“.

**Updates:** Beim Start fragt die App einmal bei GitHub nach, ob es eine neuere
Version gibt, und zeigt dann oben ein Band mit „Herunterladen“, „Später“ und
„Diese Version überspringen“. Abgerufen wird nur die neueste Versionsnummer —
keine Daten aus deinen Projekten. Abschalten: Hilfe → „Beim Start nach Updates suchen“.

## Neue Version veröffentlichen

1. In `Cargo.toml` (Abschnitt `[workspace.package]`) die Version anheben, z. B. `1.1.0`,
   und committen. Sie steht in Titelleiste, Hilfe, „Über spritebit“ und den
   Dateieigenschaften der .exe.
2. Den passenden Tag pushen:
   ```
   git tag v1.1.0
   git push origin v1.1.0
   ```

Passt der Tag nicht zur Version in `Cargo.toml`, bricht der Release-Lauf ab.

GitHub Actions (`.github/workflows/release.yml`) testet, baut und hängt
`spritebit-windows-x64.zip` an die Release. Der Download-Link oben zeigt immer
auf die neueste Version.

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

## Bedienung

| | |
|---|---|
| Linke Maustaste | malen mit dem gewählten Werkzeug |
| Rechte Maustaste | radieren (mit jedem Malwerkzeug) |
| H P B S F E I R O | Hand, Stift, Pinsel, Spray, Füllen, Radierer, Linie, Rechteck, Ellipse |
| A L K W | Auswahl, Lasso, Farbwahl, Zauberstab |
| Strg+A / C / X / V, Entf, Esc, Pfeile | Alles, Kopieren, Ausschneiden, Einfügen, Leeren, Aufheben, Verschieben |
| Mittlere Taste oder Leertaste + Ziehen | Fläche verschieben |
| Strg+Z / Strg+Y | Rückgängig / Wiederholen (je Sprite) |
| Strg+O / Strg+S / Strg+Umschalt+S | Öffnen / Speichern / Speichern unter |
| Datei → Neuer Sprite … | Größe bis 8192 × 8192 |
| Datei → Als Web-Projekt exportieren … | Projektdatei für die Web-Version |
| Ansicht | Einpassen, 100 %, Gitter |
| Timeline | Zelle anklicken wählt Frame und Ebene; Auge, Schloss, Kette schalten Sichtbarkeit, Sperre, „durchgehend“ |
| Enter / , / . / Pos1 / Ende | Abspielen, voriger, nächster, erster, letzter Frame |
| Alt+Klick | Pipette: nimmt die Farbe unter dem Mauszeiger |
| Symmetrie-Knöpfe (Werkzeugleiste) | spiegelt Striche, Formen, Spray und Füllen an der Mitte |
| Farben (links) | Farbwähler für freie Farben, Palette wählen, „Palette bearbeiten“ (eingebaute → Kopie) |
| Shift-Klick in der Timeline | Zellen-Bereich: kopieren, einfügen, leeren, verknüpfen, lösen |
| Tag-Knopf / Klick auf einen Tag | Tag anlegen bzw. bearbeiten (Bereich, Richtung, Farbe) |
| Doppelklick auf einen Ebenen-Namen | umbenennen; Pfeile verschieben die Ebene |
| Strg+E | Export: PNG (Frame oder alle), GIF (ganz oder je Tag), Spritesheet + JSON-Atlas |
| Ansicht → Sprache | Deutsch, English, Österreichisch (wird gemerkt) |
| Reiter | Geöffnete Sprites über der Zeichenfläche: Klick wechselt, × / Mittelklick / Strg+W schließt, Strg+Tab schaltet weiter, Ziehen ordnet |
| Rechte Leiste | Vorschau, Palette (Bibliothek), Bild, Aufräumen, Licht (Lichtquelle, Kantenlicht, Schlagschatten — als eigene Ebenen, jederzeit umstellbar), Hilfslinien, Schablone, Code & Export |
| 0 – 9 | Farbe mit dieser Nummer |
| Alt + Ziehen in der Auswahl | Kopie verschieben |
| Umschalt + Alt | Schablone: halten = vorn, ziehen = verschieben, Klick = Farbe |
| Mausrad / Strg + Mausrad | scrollen / zoomen |
| Strg/Umschalt + Klick auf Frame-Nummern | mehrere Frames markieren (Löschen, PNG/PDF/GIF) |
| Ziehen in der Timeline | Frames bzw. Ebenen umsortieren |
| Zahnrad in der Timeline | Lage, Zählung, Vorschaubilder, Dauer, Onion Skin |
| G · F1 · F11 | Hilfslinien · Hilfe · Vollbild |

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

## Sicherung

Die laufende Sitzung wird im Einstellungsordner (`%APPDATA%\spritebit`)
gesichert. Stürzt die App ab, ist beim nächsten Start alles wieder da; der
Stand vom Sitzungsstart bleibt als Sicherung (Hilfe → Sicherung). Dort
liegen auch Sprache, Timeline-Einstellungen, Hintergrund und die Schablone.

## Schriften

Für die Farb-Legende im Export wird Hack verwendet (aus `epaint_default_fonts`,
MIT/Bitstream Vera License).
