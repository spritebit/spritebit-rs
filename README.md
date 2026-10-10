# spritebit (Rust)

Neubau von [spritebit](https://spritebit.at) als Desktop-Programm in Rust —
Pixel-Art-Editor für Sprites, Animationen und Spiel-Levels: Ebenen mit Masken,
Timeline als Raster mit Tags und verknüpften Zellen, Licht und Schatten,
**Tilemaps mit Godot-Export**, Code-Export in neun Formaten. Später auch als
Browser-Version über WebAssembly aus derselben Codebasis.

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
Version gibt, und zeigt dann oben ein Band mit **„Jetzt aktualisieren“**, „Später“
und „Diese Version überspringen“. Abgerufen wird nur die neueste Versionsnummer —
keine Daten aus deinen Projekten. Abschalten: Hilfe → „Beim Start nach Updates suchen“.

„Jetzt aktualisieren“ lädt die neue `spritebit.exe` direkt von der Release, prüft sie
gegen die mitveröffentlichte SHA-256-Prüfsumme und tauscht die App an Ort und Stelle
aus (die alte wird zu `spritebit.old.exe` und beim nächsten Start gelöscht) — kein ZIP,
kein Entpacken. „Jetzt neu starten“ macht genau dort weiter, wo du warst, auch mit
Ungespeichertem. Liegt die App in einem schreibgeschützten Ordner, bleibt es beim
Download von Hand.

**„Was ist neu?“** im Band zeigt gleich in der App, was die neue Version mitbringt.
Nach einem Update erscheint das einmal von selbst, später unter Hilfe → „Was ist neu?“.

## Neue Version veröffentlichen

1. In `Cargo.toml` (Abschnitt `[workspace.package]`) die Version anheben, z. B. `1.1.0`,
   und committen. Sie steht in Titelleiste, Hilfe, „Über spritebit“ und den
   Dateieigenschaften der .exe.
2. Die Notizen dazu schreiben: `notes/1.1.0.md` mit den Abschnitten `## Deutsch` und
   `## English`, darin kurze Punkte (`- …`) in der Sprache der Nutzer. Sie werden der
   Text der Release und „Was ist neu?“ in der App. Fehlen sie, schlagen die Tests fehl
   und der Release-Lauf bricht ab. Ausführlich nur neue Funktionen; kleine Fixes und
   Änderungen an der Oberfläche fasst ein allgemeiner Punkt am Ende zusammen
   („Kleinere Verbesserungen und Fehlerbehebungen.“).
3. Den passenden Tag pushen:
   ```
   git tag v1.1.0
   git push origin v1.1.0
   ```

Passt der Tag nicht zur Version in `Cargo.toml`, bricht der Release-Lauf ab.

GitHub Actions (`.github/workflows/release.yml`) testet, baut und hängt
`spritebit-windows-x64.zip` an die Release — dazu `spritebit.exe` und
`spritebit.exe.sha256` für das Aktualisieren aus der App und `notes.md` für „Was ist neu?“. Der Download-Link oben zeigt immer
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
| Umschalt + Malen | nur waagerecht, senkrecht oder 45°; Linie rastet ein, Rechteck/Ellipse werden Quadrat/Kreis |
| Einfügen in einen Sprite mit anderer Palette | überträgt nach der Farbe (sieht aus wie im Original); Strg+Umschalt+V übernimmt die Nummern |
| Anfasser an der Auswahl | skalieren (Ecken/Kanten, Umschalt hält das Seitenverhältnis, Pixel bleiben scharf) |
| Füllen → Grenzen: alle Ebenen | Vorlage auf eigener Ebene ausmalen — die sichtbaren Ebenen geben die Grenzen vor |
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
| Masken-Knopf in der Timeline | Ebenenmaske: hinzufügen, bearbeiten (Malen blendet aus, Radieren ein), an/aus, anwenden, löschen — auch auf gesperrten Ebenen |
| Strg+E | Export: PNG (Frame oder alle), GIF (ganz oder je Tag), Spritesheet + JSON-Atlas |
| Ansicht → Sprache | Deutsch, English, Österreichisch (wird gemerkt) |
| Reiter | Geöffnete Sprites über der Zeichenfläche: Klick wechselt, × / Mittelklick / Strg+W schließt, Strg+Tab schaltet weiter, Ziehen ordnet |
| Rechte Leiste | Vorschau, Palette (Bibliothek), Bild, Aufräumen, Licht (Lichtquelle, Kantenlicht, Schlagschatten — als eigene Ebenen, jederzeit umstellbar), Kacheln, Hilfslinien, Schablone, Code & Export |
| Panel Kacheln | Tilemap-Ebene anlegen oder umwandeln; *Pixel malen* (eine Kachel ändert sich überall, Auto/Manuell) oder *Kacheln setzen* (Stift setzt, Radierer/Rechts leert, Füllen füllt, Alt+Klick nimmt auf); Export für Godot 4 (PNG + .tscn + JSON) |
| Pinselgröße | 1–64 per Regler oder Zahlenfeld, Alt + rechte Maustaste ziehen |
| Größenfelder | rechnen: `24 * 4`, `24x4`, `(16+8)*2`, `96 : 4` |
| Hilfslinien | Hand greift Linien direkt; eigene Layouts speichern und anwenden (anteilig umgerechnet, `guide_layouts.json`) |
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
liegen auch Sprache, Timeline-Einstellungen, Hintergrund, die Schablone und die
eigenen Hilfslinien-Layouts.

## Schriften

Für die Farb-Legende im Export wird Hack verwendet (aus `epaint_default_fonts`,
MIT/Bitstream Vera License).
