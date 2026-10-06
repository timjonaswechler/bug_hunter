# Screenshotnachweise und Reproduktion

Dieses Dokument bündelt die abgeschlossenen Messungen vom September 2026.
Es berichtet historische Ergebnisse, keinen neuen Lauf des aktuellen Arbeitsbaums.
Die damaligen fehlgeschlagenen Bildabnahmen bei Vollverdeckung bleiben dokumentiert.
Der [aktuelle Zielumfang](../target.md#screenshot) akzeptiert dort eine Ablehnung
bei fehlender Renderoberfläche; erfolgreiche Aufnahme ist kein Abschlusskriterium.
[black-screenshots.md](black-screenshots.md) erklärt Fehlerpfad und Guard;
der [Abschlussplan](../implementation-plan.md) besitzt die verbleibenden Aufgaben.

## Systeme und Versionen

| Feld | Bevy-0.19.1-Szenenmatrix | Eigenständiges RC-Repro |
| --- | --- | --- |
| System | macOS 26.6.2, Build 25G83, arm64 | macOS 26.6.2, arm64 |
| GPU und Backend | Apple M1 Pro, Metal | Apple M1 Pro, Metal |
| Bevy | `0.19.1` | `v0.20.0-rc.1` |
| Bevy-Commit | `b56fc29d3016e641754765244b5ba3f9cc504671` | `1b1f3ec1bec87386d7c19bda7d7870d4e18235c5` |
| wgpu, wgpu-core, wgpu-hal | jeweils `29.0.4` | jeweils `30.0.1` |
| woodpecker-Guard | aktiv | nicht verlinkt |

Die Szenenmatrix verwendete `rustc 1.97.1 (8bab26f4f 2026-07-14)` und
`cargo 1.97.1 (c980f4866 2026-06-30)`. Root- und Test-App-Lockfile lösten dieselben
Bevy-/wgpu-Versionen auf. Die temporäre RC-Lockfile hatte SHA-256
`2dfaa9468736dec75e2ddf326320830a5deba08bb41e13a0b92da98a589400c4`.

Das RC-Repro ist kein woodpecker-Migrationstest. Seine statische Szene übernimmt
fünf Blend-Kugeln, Bodenraster, Licht und Kamera, aber keine Bedienlogik oder
UI-Beschriftungen. Fixture-Kennung:
`blend-modes-static-v1:alpha=.9:camera=0,2.5,10:spheres=5:floor=49`.
Die Szene blieb zwischen den Captures unverändert.

## Ergebnisse

| Untersuchung | Befund | Grenze |
| --- | --- | --- |
| Frühe Frozen-State-Reihe | Vier frische Prozesse, je ein Warp und sechs Captures; 24/24 RGB0 bis 9,63 Sekunden nach Warp. | Warten half nicht; Pipeline-Bereitschaft wurde nicht unabhängig von Sichtbarkeit isoliert. |
| Erster kontrollierter 0.19.1-Lauf | Sichtbar ohne Fokus, vollständig verdeckt, wieder sichtbar; Copy entfiel nur bei Verdeckung, Rückkehrbild bytegleich. | Ein Prozess; nicht alle später geforderten Metadaten standen an jeder Logzeile. |
| 0.19.1-Szenendiagnosen | Blend, Mesh und UI bestätigten sichtbar, 50 Prozent verdeckt, vollständig verdeckt und wieder sichtbar im selben Prozess. | Guard- und Kausalitätsdiagnose, keine vollständige Interaktionsabnahme. |
| Vollständige Szenentests | Blend, Mesh und UI bestanden sichtbar, teilweise verdeckt und nach Wiederherstellung. | Vollverdeckung scheiterte jeweils am ersten Screenshot; nachfolgende Prüfungen wurden nicht ausgeführt. |
| Eigenständiges RC-Repro | Derselbe Skip-Copy-/Nullbuffer-Pfad unter `v0.20.0-rc.1`; Teilverdeckung und Wiederherstellung bytegleich zum Ausgangsbild. | Kein woodpecker-Guard und keine Projektmigration. |

Die vollständigen Blend-Tests prüften je sechs PNGs, Mesh je 13.
Die bestandenen UI-Tests prüften je acht PNGs, Recording, Replay und Isolation
zweier Sessions. Bei `restored` wurde die vorherige vollständige Verdeckung im
selben Prozess nachgewiesen, für UI getrennt für beide Sessions.

### Framegenaue Befunde

| Lauf | Sichtbar | Teilweise verdeckt | Vollständig verdeckt | Wieder sichtbar |
| --- | --- | --- | --- | --- |
| Erster 0.19.1-Lauf | Frame 10, IDs `1/1/1` | nicht geprüft | Frame 63, IDs `2/2/2` | Frame 79, IDs `3/3/3` |
| Blend-Matrix | Frame 62, IDs `1/1/1` | Frame 132, IDs `2/2/2` | Frame 202, IDs `3/3/3` | Frame 232, IDs `4/4/4` |
| Mesh-Matrix | Frame 41, IDs `1/1/1` | Frame 75, IDs `2/2/2` | Frame 109, IDs `3/3/3` | Frame 132, IDs `4/4/4` |
| Neueste UI-Matrix | Frame 51, IDs `1/1/1` | Frame 93, IDs `2/2/2` | Frame 132, IDs `3/3/3` | Frame 159, IDs `4/4/4` |
| RC-Repro | Frame 11, IDs `1/1/1` | Frame 59, IDs `2/2/2` | Frame 118, IDs `3/3/3` | Frame 172, IDs `4/4/4` |

IDs bezeichnen Capture, Buffer und Capture-Textur. In jedem vollständig
verdeckten Fall derselben Tabelle war die Kette:

```text
native Vollverdeckung, Visible-Bit fehlt
acquire=occluded, Swapchain-View fehlt
Capture-Textur und Buffer vorbereitet
copy_skipped, Queue ohne Screenshot-Copy eingereicht
map_success für denselben Buffer, Rohdaten RGB0
```

Bei 0.19.1 lehnte der unveränderte Guard ohne PNG ab. Das eigenständige RC-Repro
schrieb dagegen das schwarze PNG. In allen übrigen Zuständen liefen Copy und
Mapping; die Bilder waren innerhalb des jeweiligen Laufs bytegleich.
Szenen-, Kamera-, Größen- und Skalierungsangaben blieben dabei unverändert.
Die 0.19.1-Diagnosen verwendeten einen Starttick für Blend, elf für Mesh und drei
für UI, ohne weitere Ticks zwischen den vier Captures. Der erste Einzelversuch
verwendete genau einen Warp und drei Captures.

Kontrollwerte für die sichtbaren PNGs:

| Szene | PNG-SHA-256 |
| --- | --- |
| Erster 0.19.1-Lauf und Blend-Matrix | `5960ef705882fae4f85ea4746fcae657cb61eb89ed990494663fff23e421fec2` |
| Mesh-Matrix | `c10c958238c29ba3adc870f3f66365bdf22192c64d124bf02f2a9b441d82f134` |
| Neueste UI-Matrix | `35024e5846ae8240c1e8558cc53b241b63e976878328099d2501ebd61046747c` |
| RC-Repro | `c820d153d70a683faa9b491342727106adeed422ed35cf0de815d58c0445af16` |

Der verdeckte RC-Capture lieferte 3.686.400 Nullbytes einschließlich Alpha,
Roh-SHA-256 `0c660f2bd3eff3150dd0040789abe2291613b9af319df870203d4f77a4913a5f`,
PNG-SHA-256 `7f0fcfad3151bf17c4ba0faa396039ee753bfd086f998f751361bedcb6a3e4b4`.
Diese Werte sind Belege der bekannten Fixture, keine allgemeine Schwarzbildheuristik.

## Werkzeuge und Befehle

Alle Befehle laufen vom Repository-Root. Die AppKit-/Swift-Werkzeuge benötigen
macOS, einen wachen Desktop und Xcode Command Line Tools. Builds und GUI-Läufe
sind getrennt. Vor Grafiktests `BUILD_READY` melden und auf `GUI_FREIGABE`
warten; grafische Tests nacheinander ausführen.

Die Treiber instrumentieren wegwerfbare Dependency-/App-Kopien unter `target/`.
Sie verändern keine Registry-Quellen oder produktiven Manifeste. Diagnosepatches
sind keine Produktkorrektur. Keine Wiederholungen bis zum ersten grünen Ergebnis.

### Vorbereitung ohne Grafik

```sh
cargo build --features cli --bin woodpecker
python3 tests/diagnostics/capture_scene_visibility_support_test.py -v
python3 tests/diagnostics/capture_scene_visibility.py --prepare-only
python3 tests/diagnostics/capture_causality_live.py --prepare-only
python3 tests/diagnostics/capture_causality_rc.py --prepare-only
```

Die Szenenmatrix kann gezielt mit `--prepare-scenes mesh_picking ui` gebaut
werden. Ihre Instrumentierung und nativen Messhelfer stehen neben dem Treiber
unter [tests/diagnostics/](../../../tests/diagnostics/).

### Kontrollierte Diagnosen

```sh
python3 tests/diagnostics/capture_causality_live.py
python3 tests/diagnostics/capture_scene_visibility.py --scene blend_modes
python3 tests/diagnostics/capture_scene_visibility.py --scene mesh_picking
python3 tests/diagnostics/capture_scene_visibility.py --scene ui
python3 tests/diagnostics/capture_causality_rc.py
```

Der erste Treiber fährt sichtbar/verdecktes/wieder sichtbares Fenster, die
anderen ergänzen eine geometrisch nachgewiesene 50-Prozent-Verdeckung.
Jeder Befehl ist ein eigener freizugebender Lauf, kein automatischer Gesamtauftrag.

### Strikte vollständige Szenentests

```sh
python3 tests/diagnostics/capture_scene_visibility.py --full-test blend_modes --condition visible
python3 tests/diagnostics/capture_scene_visibility.py --full-test mesh_picking --condition partial
python3 tests/diagnostics/capture_scene_visibility.py --full-test ui --condition restored
```

Für jede der drei Szenen sind `visible`, `partial`, `covered` und `restored`
getrennte Fälle. `covered` bleibt mit dem aktuellen Adapter eine nicht bestandene
Bildabnahme. Der Wrapper darf diese Ablehnung nicht als korrekte Bildaufnahme zählen.

### Begrenzte Frozen-State-/Guard-Prüfung

```sh
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin blend_modes
python3 tests/diagnostics/cold_start_frozen_captures.py --sessions 4 --captures 6
python3 tests/diagnostics/cold_start_frozen_captures.py \
  --sessions 2 --captures 3 --verify-surface-guard
```

Der letzte Modus akzeptiert ein korrektes Bild der bekannten Szene oder eine
explizite Surface-Ablehnung ohne Datei. Er prüft den Guard, nicht eine erfolgreiche
Aufnahme bei Vollverdeckung.

## Erforderliche Messdaten

Für neue Plattformläufe oder einen abweichenden Fehlerbefund gelten diese Kriterien:

1. Native Sichtbarkeit, Fokus, Miniaturisierung, Ziel-/Deckgeometrie, Größe und
   Skalierung im Prozess-Hauptthread erfassen. Sichtbar, verdeckt, versteckt und
   minimiert sind verschiedene Bedingungen. Die vorhandene Szenenmessung
   verlangt mindestens acht stabile Samples über mindestens 100 ms.
2. Session, PID, Command-Request, Screenshot-Entity, Fenster, Renderframe,
   Capture-Textur und Buffer über IDs korrelieren. Zeitnähe oder Dateireihenfolge
   genügt nicht. Die IDs müssen von Vorbereitung bis CPU-Auswertung erhalten bleiben.
3. Acquire-Status, Vorbereitung mit Format und Copy-Extent, Copy oder Skip,
   Queue-Submit und Map-Abschluss für genau denselben Buffer protokollieren.
   Ein Encoder-Log allein belegt keine GPU-Ausführung. Geräteverlust und
   Validierungsfehler gesondert prüfen.
4. Rohbytes vor PNG-Encoding mit Bytezahl und Hash erfassen. Danach den
   Adapterausgang und das erneut dekodierte PNG zuordnen. Bei Ablehnung
   `no_file` beziehungsweise unveränderte vorhandene Datei prüfen.
5. Im gleichen Prozess sichtbar ohne Fokus, teilweise verdeckt, vollständig
   verdeckt und wieder sichtbar messen. Zwischen Captures keine Simulationsticks;
   Zustand, Kameradaten und Abmessungen vergleichen.

Die aktuellen Mehrprozesswerkzeuge schreiben getrennte Shards:

```text
capture-chain/bevy_render-<pid>.jsonl
capture-chain/woodpecker_adapter-<pid>.jsonl
appkit-main/appkit_main-<pid>.jsonl
```

Jede Quelle verwendet einen prozesslokalen `Mutex<File>` und schreibt eine
vollständige JSON-Zeile samt Newline unter derselben Sperre. Der Reader wartet
nur bei einem unvollständigen Tail; eine abgeschlossene ungültige Zeile bleibt
Fehler. Sieben headless Regressionstests prüften zuletzt auch parallele Quellen,
Prozesse, Schreibfehler und ID-Korrelation bei abweichender Ankunftsreihenfolge.

### Auswertung und Grenzen

- Fehlender View, ausgelassener Copy und Nullbytes desselben Buffers bestätigen
  den Skip-Copy-Pfad. Erst native Zustandskontrolle und `acquire=occluded`
  ordnen ihn der Verdeckung zu.
- Erfolgreich eingereichter Copy mit anschließendem RGB0 widerlegt Skip-Copy
  für diesen Capture. Dann Kamera, Renderpipelines, Texturinhalt und
  Formatkonvertierung prüfen, statt automatisch erneut aufzunehmen.
- Ein Größenversatz belegt zunächst nur die
  [Resize-Lücke](window-schedules.md). Gleiche PNG-Abmessungen allein schließen
  veraltete Kameradaten nicht aus.
- Cold-Renderbereitschaft benötigt eine eigene Reihe mit stabiler Sichtbarkeit,
  vollständiger Copy-/Map-Kette und gemessenen Bereitschaftssignalen.
  `Ready` oder vergangene Wartezeit genügt nicht.
- Der erste 0.19.1-Einzelversuch hatte nicht an jeder Zeile PID, Session- und
  Request-ID. Seine lückenlose Frame-/Sample-Zuordnung und stabilen Zustände
  tragen den Einzelprozessbefund, ersetzen aber nicht die spätere Mehrprozesskorrelation.

## Evidenzverzeichnisse

Alle folgenden Pfade liegen unter `target/`. Sie sind lokale, nicht versionierte
Artefakte und werden nicht durch Git übertragen. Ihre Aufbewahrung oder erneute
Verfügbarkeit wird durch diese Dokumentation nicht garantiert.

### Diagnosen und Guard

| Pfad unter `target/` | Ergebnis |
| --- | --- |
| `cold-start-9u7gcjre` | 24/24 schwarze Captures, eingefrorener Zustand. |
| `cold-start-z2kb05em` | Native Occlusion-Probe; 139/139 Frames ohne View, 4/4 RGB0. |
| `cold-start-bjwe7bt3` | Späterer sichtbarer Prozess; 184/184 Frames mit View, 4/4 korrekte Bilder. Kein kontrollierter In-Process-Wechsel. |
| `cold-start-hasx1cq2` | Nach Guard: zwei Sessions, sechs korrekte PNGs. |
| `cold-start-dcykypfz` | Temporär unsichtbares Fenster, drei Ablehnungen ohne Datei; danach Originalbinary wiederhergestellt. |
| `capture-causality-live-x01e81hr` | Erster kontrollierter 0.19.1-Kausalitätslauf. |
| `capture-scene-visibility-blend_modes-kwmwsl9b` | Blend-Diagnose bestanden. |
| `capture-scene-visibility-mesh_picking-yrei5rz6` | Mesh-Diagnose bestanden. |
| `capture-scene-visibility-ui-s7vq0la8` | Frühere korrigierte UI-Diagnose bestanden. |
| `capture-scene-visibility-ui-iivvmkey` | Neueste UI-Diagnose mit vollständiger Shard-/ID-Korrelation bestanden. |
| `capture-causality-rc-run-7grh6a8p` | RC-Repro; `result.json` meldet `reproduced_skip_copy_zero_readback`. |
| `surface-guard-tests.9KTXDU` | Root-Suite nach Absicherung: 136 Bibliotheks-/CLI-, 7 Beobachtungs-, 13 Session-Prozesstests bestanden. |

### Vollständige Sichtbarkeitsmatrix

| Szene | sichtbar: bestanden | teilweise verdeckt: bestanden | vollständig verdeckt: Bildabnahme nicht bestanden | wieder sichtbar: bestanden |
| --- | --- | --- | --- | --- |
| Blend | `capture-scene-full-blend_modes-visible-ddi4e801` | `capture-scene-full-blend_modes-partial-1zo7hq8p` | `capture-scene-full-blend_modes-covered-fu2ank9t` | `capture-scene-full-blend_modes-restored-53wptupm` |
| Mesh | `capture-scene-full-mesh_picking-visible-agv8abch` | `capture-scene-full-mesh_picking-partial-cfiyvs2f` | `capture-scene-full-mesh_picking-covered-xj23ew2r` | `capture-scene-full-mesh_picking-restored-9qntd6z6` |
| UI | `capture-scene-full-ui-visible-c190r5jx` | `capture-scene-full-ui-partial-8smzq328` | `capture-scene-full-ui-covered-xfl0m5lh` | `capture-scene-full-ui-restored-w43vhbi0` |

### Historische Bildläufe ohne vollständige Kausalitätsmessung

| Pfad unter `target/` | Ergebnis |
| --- | --- |
| `mesh-picking-91lxd21b`, `mesh-picking-h342sacr` | Je schwarzes `idle.png`, 640×360. |
| `mesh-picking-cdmyuwwl` | Nichtschwarzes Bild, 320×180; Test scheiterte an fester Größenerwartung. |
| `mesh-picking-1osc9q3p`, `mesh-picking-ha7_94g1` | Je 13 korrekte PNGs vor dem Guard. |
| `blend-modes-tsrdc2m0`, `blend-modes-r2sbu_1r` | Je sechs korrekte PNGs vor dem Guard. |
| `blend-modes-f9gag8t5` | Erstes PNG RGB0 bei 1280×720; Surface-Status unbekannt. |
| `blend-modes-fyz9uxet`, `mesh-picking-1j9wpyb4` | Nach Guard: explizite Surface-Ablehnung beim dritten beziehungsweise ersten Capture. |

## Ungültige Läufe und Werkzeugkorrekturen

Spätere erfolgreiche Läufe ersetzen diese Fehlerbelege nicht.

| Pfad unter `target/` | Fehler und Einordnung |
| --- | --- |
| `cold-start-ht6804un` | Falsch gelesene Komponentenform; Abbruch vor Warp und Capture. |
| `cold-start-pqn58xf7` | AppKit-Probe nicht am Hauptthread; Panic vor gültiger Messung. |
| `cold-start-d8niwlgx` | 137 Frames ohne View und vier schwarze Captures; die nicht hauptthreadgebundene AppKit-Abfrage ist kein nativer Nachweis. |
| `capture-causality-live-xmw1jsh2` | Sampler lief in `First` nur beim Warp, nicht bei jedem App-Durchlauf. Korrigiert nach `Main`; kein zusätzlicher Tick ergänzt. |
| `capture-scene-full-mesh_picking-restored-rdpbnej6` | Reader traf unvollständigen JSONL-Tail. Nach Prozessende gültige Datei, aber Wiederherstellung nicht ausreichend gemessen. |
| `capture-scene-visibility-ui-h5urkuye` | Mehrere Schreiber verschachtelten abgeschlossene JSONL-Zeilen. Zweiter Capture nicht vollständig korrelierbar; keine vollständige UI-Diagnose. |
| `capture-scene-full-ui-visible-m00zty0m` | Stabilitätsprüfung betrachtete nur die letzten acht Samples; diese spannten weniger als 100 ms trotz längerer stabiler Reihe. Abbruch vor Screenshot. |

Frühere Fokus-/Frontversuche unter `cold-start-55atml2a`, `cold-start-x63x19db`,
`cold-start-rtccoh2k`, `cold-start-pd8h_kk4` und `cold-start-opoqnoun` erreichten
keinen zuverlässig gemessenen sichtbaren Zustand. Sie sind kein Fixnachweis.

Die Reader-, Mehrschreiber- und Stabilitätskorrekturen betrafen Diagnosewerkzeuge,
nicht Screenshot- oder Simulationsverhalten. Vorbereitungslogs liegen unter
`capture-scene-visibility/prepare-final.log`, `prepare-continuation.log` und
`prepare-ui-race-fix.log`. Temporäre Instrumentierung und Builds liegen unter
`capture-causality/`, `capture-scene-visibility/` beziehungsweise
`capture-causality-rc/`.

Die dokumentierten Läufe beendeten ihre eigenen App-, Server- und Deckprozesse.
Cleanupdaten stehen in den jeweiligen Ergebnisordnern. Diese historische Aussage
ersetzt keine Prozessprüfung nach einem neuen Lauf. Lokale Evidenz erst nach
Sicherung löschen; diese Dokumentkonsolidierung verändert weder Werkzeuge noch
Artefakte.
