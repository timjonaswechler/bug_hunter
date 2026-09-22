# Szenensichtbarkeit mit Bevy 0.19.1

Stand: Die erste freigegebene Grafikmatrix wurde am 21. September 2026 auf diesem Mac beim zehnten Befehl wegen eines Infrastrukturfehlers angehalten. Eine erste Fortsetzung bestand Mesh-`restored`, stoppte aber bei einer JSONL-Mehrschreiber-Race der UI-Diagnose. Eine zweite UI-Fortsetzung bestand die korrigierte Diagnose und stoppte wegen einer zu engen Stabilitätsauswertung vor dem ersten Screenshot des sichtbaren Volltests. Danach wurde eine dritte UI-Fortsetzung als neue, einmalige Fallfolge freigegeben. Alle fünf angekündigten Befehle liefen am 22. September 2026 jeweils genau einmal und strikt nacheinander. Die UI-Diagnose sowie die Volltests `visible`, `partial` und `restored` bestanden. `covered` endete mit der erwarteten Guard-Ablehnung und bleibt eine nicht bestandene Bildabnahme. Frühere Fehlerbelege wurden nicht ersetzt. Es gab keine Wiederholung und keinen RC-Lauf.

## Ergebnis

Die kontrollierten A/B/C/A-Messungen für `blend_modes` und `mesh_picking` bestätigen denselben Fehlerpfad wie der frühere Einzelversuch:

```text
vollständige geometrische Verdeckung
-> AppKit Visible-Bit fehlt
-> CurrentSurfaceTexture::Occluded
-> Screenshot-Textur und Readback-Buffer vorbereitet
-> copy_texture_to_buffer übersprungen
-> Queue ohne Copy eingereicht
-> Mapping desselben Buffers erfolgreich
-> RGB0-Readback
-> normaler woodpecker-Guard lehnt ohne PNG ab
```

Bei sichtbarem, nicht fokussiertem Fenster und bei exakt 50 Prozent geometrischer Verdeckung liefen Acquire, Copy, Submit und Mapping. Das Bild blieb bytegleich. Nach Entfernung des Deckfensters kehrte ebenfalls das bytegleiche Ausgangsbild zurück. Zustand, Fenstergröße, Skalierung und erfasste Kameraangaben blieben in den vier Captures jeder Szene unverändert.

Die strikten vollständigen Szenentests bestanden für Blend und Mesh im sichtbaren, teilweise verdeckten und wieder sichtbaren Zustand. Für beide `restored`-Abnahmen wurde die vollständige Verdeckung vorher im selben Prozess stabil nachgewiesen. Bei vollständiger Verdeckung brachen beide strikten Tests wie vorgesehen an ihrer ersten Screenshot-Ablehnung ab. Das sind nicht bestandene Bildabnahmen. Die ergänzende Guard-Diagnose macht daraus keinen bestandenen Szenentest.

Der ursprüngliche vollständige Mesh-Test im Fall `restored` bleibt als Infrastrukturfehler dokumentiert. Die Fortsetzung schrieb einen neuen Artefaktordner und ersetzte den alten Befund nicht. Der neue Lauf bestätigte acht stabile Verdeckungssamples über 250,774 ms und danach acht stabile Wiederherstellungssamples über 261,819 ms. Anschließend bestanden alle 13 Captures und der vollständige Mesh-Test.

Die neue UI-Sichtbarkeitsdiagnose bestand die vollständige A/B/C/A-Folge mit lückenloser ID-Korrelation aus den getrennten Logshards. Sichtbar, 50 Prozent verdeckt und wieder sichtbar ergaben bytegleiche gültige PNGs. Bei vollständiger Verdeckung fehlte das AppKit-Visible-Bit, Acquire meldete `occluded`, die Copy wurde übersprungen, das Mapping ergab RGB0 und der Guard lehnte ohne PNG ab.

Die vollständigen UI-Tests `visible`, `partial` und `restored` bestanden mit ihren unveränderten Bild-, Recording-, Replay- und Sessionisolationsprüfungen für zwei Sessions. Jeder bestandene Lauf erzeugte acht abgenommene PNGs. Im Fall `restored` wurden für beide Sessions vor ihrer ersten Abnahme jeweils acht stabile Samples tatsächlicher Vollverdeckung und anschließend acht stabile Samples der Wiederherstellung nachgewiesen. Der vollständige `covered`-Test brach erwartungsgemäß beim ersten Screenshot mit `screenshot_window_unavailable` ab. Das ist eine nicht bestandene Bildabnahme und kein bestandener UI-Test.

## System und gelockte Versionen

| Feld | Wert |
| --- | --- |
| Betriebssystem | macOS 26.6.2, Build 25G83 |
| Architektur | arm64 |
| GPU | Apple M1 Pro |
| Backend | Metal, Systembericht: Metal 4 |
| Hauptbildschirm | 3440 mal 1440 |
| Rust | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| Cargo | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| Bevy | `bevy 0.19.1`, `bevy_render 0.19.1` |
| wgpu | `wgpu`, `wgpu-core`, `wgpu-hal` jeweils `29.0.4` |

Root- und Test-App-Lockfile lösen dieselben Bevy- und wgpu-Versionen auf. Die Aussage gilt nur für diesen Mac und Metal.

## Vorbereitung

Die Vorbereitung startete keine grafischen Prozesse. Verwendet wurden:

```sh
python3 -m py_compile \
  tests/diagnostics/capture_scene_visibility.py \
  tests/diagnostics/capture_scene_visibility_instrument.py \
  tests/diagnostics/capture_scene_visibility_support.py
python3 tests/diagnostics/capture_scene_visibility_support_test.py -v
xcrun swiftc -parse tests/diagnostics/capture_scene_visibility_deck.swift
python3 tests/diagnostics/capture_scene_visibility.py --prepare-only
cargo test --all-features -- --test-threads=1
cargo test --offline \
  --manifest-path target/capture-scene-visibility/root/bevy_test_apps/Cargo.toml \
  --lib
```

Ergebnisse:

- 136 Root-Unit-/CLI-Tests bestanden.
- 7 Beobachtungstests bestanden.
- 13 Session-Prozesstests bestanden.
- Der headless Library-Test der kopierten Bevy-Testanwendungen bestand.
- Python-Syntax, Swift-Parsing, Root-CLI-Build und die drei instrumentierten Szenen-Builds bestanden.
- Das erste Vorbereitungslog liegt unter `target/capture-scene-visibility/prepare-final.log`.
- Vor der Fortsetzung bestanden vier neue headless JSONL-Reader-Regressionstests. Sie prüfen vollständige Zeilen, einen vorläufig ignorierten unvollständigen Tail, dessen späteren Abschluss sowie den Fehler für eine abgeschlossene ungültige Zeile.
- Für die erste Fortsetzung wurden nur Mesh und UI mit `python3 tests/diagnostics/capture_scene_visibility.py --prepare-only --prepare-scenes mesh_picking ui` gebaut. Das Log liegt unter `target/capture-scene-visibility/prepare-continuation.log`.
- Vor der letzten UI-Fortsetzung bestanden sieben headless JSONL-Tests, einschließlich Mehrthread-, Mehrquellen- und Mehrprozess-Schreibtests. Anschließend wurde nur UI gebaut. Das Log liegt unter `target/capture-scene-visibility/prepare-ui-race-fix.log`.

Die Instrumentierung kopiert woodpecker, `bevy_test_apps` und `bevy_render 0.19.1` nach `target/capture-scene-visibility/`. Lokale Cargo-Patches und Testanpassungen existieren nur dort. Root-Manifeste, Root-Lockfiles, Registry-Quellen und Produktionsdateien wurden nicht für die Messung geändert.

## Messaufbau

Die neuen Werkzeuge sind:

- `tests/diagnostics/capture_scene_visibility.py`
- `tests/diagnostics/capture_scene_visibility_instrument.py`
- `tests/diagnostics/capture_scene_visibility_native.rs`
- `tests/diagnostics/capture_scene_visibility_deck.swift`
- `tests/diagnostics/capture_scene_visibility_support.py`
- `tests/diagnostics/capture_scene_visibility_support_test.py`

Das AppKit-System läuft im Prozess-Hauptthread. Jede native Zeile enthält PID, Session-ID aus dem sessioneigenen Artefaktpfad, Fenster- und Inhaltsgeometrie, Fokus, Miniaturisierung, rohen Occlusion-Wert, Visible-Bit, Bevy-Fenstergröße, Skalierung und Kameraangaben. Damit ist jedes Fenster seiner Session und seinem Prozess zugeordnet.

Das Deck ist ein eigenes borderless AppKit-Fenster. Es steuert keine fremden Fenster. Teilverdeckung wird aus der gemessenen Schnittfläche von Ziel- und Deckrechteck bestimmt. Der Test leitet sie nicht aus `occlusionVisible` ab. Der stabile Zustand verlangt acht gleiche Mainthread-Samples über mindestens 100 ms.

Die temporäre Bevy-Instrumentierung vergibt monotone `frame_id`, `capture_id`, `buffer_id` und `capture_texture_id`. Der temporär instrumentierte Adapter ordnet Command-Request-ID, Screenshot-Entity und Window-Entity zu. Die Kette umfasst Acquire, Prepare, Copy oder Skip, Submit, Map und `image_returned`. Der normale Guard bleibt aktiv. Rohe Bevy-Readbacks werden vor seiner Entscheidung separat unter `raw-readbacks/` abgelegt.

Die ergänzende Szenendiagnose verwendet nur die vorhandenen expliziten Startticks: einen Tick für Blend, elf Ticks für Mesh und drei Ticks für UI. Zwischen ihren vier Captures gibt es keine weiteren Ticks. Die vollständigen Tests behalten ihre vorhandenen expliziten Ticks und Bildassertionen.

## Fallmatrix der vollständigen Tests

| Szene | Sichtbarkeit | Ergebnis | Bildabnahme und Umfang | Evidenz |
| --- | --- | --- | --- | --- |
| Blend | sichtbar, nicht fokussiert | bestanden | Vollständiger Test, sechs PNGs und alle vorhandenen Blend-, Alpha-, Lit-, HDR-, Zustands- und Reproduzierbarkeitsprüfungen bestanden. | `target/capture-scene-full-blend_modes-visible-ddi4e801/` |
| Blend | 50 Prozent verdeckt | bestanden | Vollständiger Test mit unveränderten strikten Bildprüfungen bestanden. Gemessene Schnittfläche 481.280 von 962.560 Punkten. | `target/capture-scene-full-blend_modes-partial-1zo7hq8p/` |
| Blend | vollständig verdeckt | nicht bestanden | Erster Capture `unlit-alpha-one` mit `screenshot_window_unavailable` abgelehnt. Keine vollständige Bildabnahme. | `target/capture-scene-full-blend_modes-covered-fu2ank9t/` |
| Blend | wieder sichtbar | bestanden | Vor dem ersten Capture im selben Prozess wurden vollständige Verdeckung und fehlendes Visible-Bit stabil bestätigt. Danach bestanden alle sechs Captures und der vollständige Test. | `target/capture-scene-full-blend_modes-restored-53wptupm/` |
| Mesh | sichtbar, nicht fokussiert | bestanden | Vollständiger Test mit 13 PNGs, Objektfarben, Hover, Press, Release, Out, Drag und Rotation bestanden. | `target/capture-scene-full-mesh_picking-visible-agv8abch/` |
| Mesh | 50 Prozent verdeckt | bestanden | Vollständiger Test mit unveränderten 13 Bild- und Interaktionsprüfungen bestanden. Gemessene Schnittfläche 125.440 von 250.880 Punkten. | `target/capture-scene-full-mesh_picking-partial-cfiyvs2f/` |
| Mesh | vollständig verdeckt | nicht bestanden | Erster Capture `idle` mit `screenshot_window_unavailable` abgelehnt. Keine vollständige Bildabnahme. | `target/capture-scene-full-mesh_picking-covered-xj23ew2r/` |
| Mesh | wieder sichtbar | bestanden in der Fortsetzung | Der neue Lauf bestätigte im selben Prozess zuerst vollständige Verdeckung und dann Wiederherstellung. Alle 13 Captures und Bildprüfungen bestanden. Der ursprüngliche Infrastrukturfehler bleibt separat erhalten. | neu: `target/capture-scene-full-mesh_picking-restored-9qntd6z6/`; alt: `target/capture-scene-full-mesh_picking-restored-rdpbnej6/` |
| UI | sichtbar, nicht fokussiert | bestanden in der letzten Fortsetzung | Acht PNGs sowie alle vorhandenen Bild-, Recording-, Replay- und Sessionisolationsprüfungen für zwei Sessions bestanden. Der frühere Infrastrukturfehler vor Bildabnahme bleibt separat erhalten. | neu: `target/capture-scene-full-ui-visible-c190r5jx/`; alt: `target/capture-scene-full-ui-visible-m00zty0m/` |
| UI | 50 Prozent verdeckt | bestanden | Acht PNGs und der vollständige unveränderte UI-Test bestanden. Alle zehn Platzierungen hatten exakt 125.440 von 250.880 Punkten Schnittfläche. | `target/capture-scene-full-ui-partial-8smzq328/` |
| UI | vollständig verdeckt | nicht bestanden | Der erste Capture wurde mit `screenshot_window_unavailable` abgelehnt. Volle geometrische Verdeckung und fehlendes Visible-Bit waren stabil bestätigt; keine vollständige Bildabnahme. | `target/capture-scene-full-ui-covered-xfl0m5lh/` |
| UI | wieder sichtbar | bestanden | Für beide Sessions wurden vor der ersten Abnahme vollständige Verdeckung und anschließende Wiederherstellung stabil nachgewiesen. Danach bestanden acht PNGs sowie Recording, Replay und Sessionisolation. | `target/capture-scene-full-ui-restored-w43vhbi0/` |

Die vollständigen Verdeckungsfälle sind bewusst rot. Eine korrekte Guard-Ablehnung verhindert das falsche RGB0-PNG, erfüllt aber nicht den Vertrag des vollständigen Szenentests.

## Ergänzende A/B/C/A-Diagnose

Diese Diagnose ist keine vollständige Szenenabnahme. Sie bestätigt den Guard und verfolgt den Bevy-Readback vor dem Guard.

### Blend

Evidenz: `target/capture-scene-visibility-blend_modes-kwmwsl9b/`

Zielrechteck: `(1080, 493, 1280, 752)`, Fläche 962.560 Punkte. Session `699ddadaf98dd086f843adb7e6c867c7`, PID 52171.

| Zustand | Frame | Capture/Buffer/Textur | AppKit und Geometrie | Copy, Map und Bild |
| --- | ---: | --- | --- | --- |
| sichtbar ohne Fokus | 62 | `1/1/1` | `visible=true`, `key=false`, `miniaturized=false`, Occlusion 8194, keine Schnittfläche | Copy und Map erfolgreich; 2.763.510 RGB-Bytes ungleich null; PNG-SHA-256 `5960ef705882fae4f85ea4746fcae657cb61eb89ed990494663fff23e421fec2` |
| teilweise verdeckt | 132 | `2/2/2` | 481.280 von 962.560 Punkten, exakt 50 Prozent; Visible-Bit gesetzt | Copy und Map erfolgreich; PNG bytegleich zum Ausgangsbild |
| vollständig verdeckt | 202 | `3/3/3` | volle Schnittfläche; Occlusion 8192, Visible-Bit fehlt | `acquire=occluded`; Copy wegen `swap_chain_view_missing` übersprungen; Submit ohne Copy; Map erfolgreich; RGB0; Guard-Ablehnung ohne PNG |
| wieder sichtbar | 232 | `4/4/4` | keine Schnittfläche; Ausgangszustand wiederhergestellt | Copy und Map erfolgreich; PNG bytegleich zum Ausgangsbild |

### Mesh

Evidenz: `target/capture-scene-visibility-mesh_picking-yrei5rz6/`

Zielrechteck: `(1400, 763, 640, 392)`, Fläche 250.880 Punkte. Session `38c73a943fc16c44345827db23869f17`, PID 78046.

| Zustand | Frame | Capture/Buffer/Textur | AppKit und Geometrie | Copy, Map und Bild |
| --- | ---: | --- | --- | --- |
| sichtbar ohne Fokus | 41 | `1/1/1` | Occlusion 8194, keine Schnittfläche | Copy und Map erfolgreich; 691.200 RGB-Bytes ungleich null; PNG-SHA-256 `c10c958238c29ba3adc870f3f66365bdf22192c64d124bf02f2a9b441d82f134` |
| teilweise verdeckt | 75 | `2/2/2` | 125.440 von 250.880 Punkten, exakt 50 Prozent | Copy und Map erfolgreich; PNG bytegleich zum Ausgangsbild |
| vollständig verdeckt | 109 | `3/3/3` | volle Schnittfläche; Occlusion 8192, Visible-Bit fehlt | `acquire=occluded`; Copy übersprungen; Submit ohne Copy; Map erfolgreich; RGB0; Guard-Ablehnung ohne PNG |
| wieder sichtbar | 132 | `4/4/4` | keine Schnittfläche; Occlusion 8194 | Copy und Map erfolgreich; PNG bytegleich zum Ausgangsbild |

### Mesh-`restored`-Volltest der Fortsetzung

Evidenz: `target/capture-scene-full-mesh_picking-restored-9qntd6z6/`

Session `e2338a6266aac648dd0ea66a225221bd`, PID 24148. Das Zielrechteck blieb `(1400, 763, 640, 392)`. Vor dem ersten Screenshot bedeckte das Deck die gesamte Fläche von 250.880 Punkten. Samples 126 bis 133 waren über 250,774 ms stabil, mit Occlusion 8192 und ohne Visible-Bit. Nach Entfernung des Decks waren Samples 135 bis 142 über 261,819 ms stabil, mit Occlusion 8194 und gesetztem Visible-Bit. Danach bestanden alle 13 vorhandenen Screenshot-, Farb-, Interaktions- und Rotationsprüfungen. `visibility-states.json` ordnet alle Captures derselben Session und PID zu.

Die PNGs der sichtbaren Zustände besitzen 10.775 verschiedene RGB-Werte bei Blend und 2.680 bei Mesh. Die vollständigen Szenentests prüfen darüber hinaus bekannte Objektfarben und Zustandsänderungen. Die Diagnose verlässt sich nicht bloß auf Dateiexistenz oder einen Nonzero-Test.

### UI-Versuch der Fortsetzung

Evidenz: `target/capture-scene-visibility-ui-h5urkuye/`

Session `69d55e9e8fe0aa00d14ac916f55785a2`, App-PID 29891. Das Zielrechteck war `(1400, 763, 640, 392)`. Der sichtbare, nicht fokussierte Zustand war über acht Mainthread-Samples und 111,179 ms stabil. Capture 1 erzeugte einen vollständigen Roh-Readback. Danach bestätigte die Geometrie für Zustand 2 eine Schnittfläche von 125.440 aus 250.880 Punkten, exakt 50 Prozent, über acht Samples und 100,028 ms.

Der zweite Capture erzeugte ebenfalls einen Roh-Readback. Seine vollständige ID-Kette kann wegen der korrupten Zeilen 88 und 89 in `capture-chain.jsonl` aber nicht beweiskräftig rekonstruiert werden. Vollverdeckung, Guard-Ablehnung und Wiederherstellung wurden in diesem UI-Lauf nicht erreicht. Der gesamte UI-Diagnosefall gilt als Infrastrukturfehler.

### UI-Diagnose nach den Werkzeugkorrekturen

Neueste Evidenz: `target/capture-scene-visibility-ui-iivvmkey/`. Der frühere bestandene Diagnoseordner `target/capture-scene-visibility-ui-s7vq0la8/` bleibt unverändert erhalten.

Session `09ee1427a246284cf7f80670e12f936c`, App-PID 10501. Zielrechteck `(1400, 763, 640, 392)`, Fläche 250.880 Punkte. Alle Ereignisse liegen als gültige JSON-Zeilen in drei getrennten Shards für `bevy_render`, `woodpecker_adapter` und `appkit_main` vor. Die vollständigen ID-Ketten wurden ohne Dateireihenfolge oder Zeitstempel korreliert.

| Zustand | Frame | Capture/Buffer/Textur | AppKit und Geometrie | Copy, Map und Bild |
| --- | ---: | --- | --- | --- |
| sichtbar ohne Fokus | 51 | `1/1/1` | Occlusion 8194, keine Schnittfläche | Copy und Map erfolgreich; 651.174 RGB-Bytes ungleich null; PNG-SHA-256 `35024e5846ae8240c1e8558cc53b241b63e976878328099d2501ebd61046747c` |
| teilweise verdeckt | 93 | `2/2/2` | 125.440 von 250.880 Punkten, exakt 50 Prozent; Visible-Bit gesetzt | Copy und Map erfolgreich; PNG bytegleich zum Ausgangsbild |
| vollständig verdeckt | 132 | `3/3/3` | volle Schnittfläche; Occlusion 8192, Visible-Bit fehlt | `acquire=occluded`; Copy übersprungen; Submit ohne Copy; Map erfolgreich; RGB0; Guard-Ablehnung ohne PNG, daher keine bestandene Bildabnahme |
| wieder sichtbar | 159 | `4/4/4` | keine Schnittfläche; folgte im selben Prozess und derselben Session auf den stabil bestätigten vollständig verdeckten Zustand | Copy und Map erfolgreich; PNG bytegleich zum Ausgangsbild |

Die neueste UI-Diagnose bestand vollständig, einschließlich unverändertem `SessionState`, vier eindeutigen Capture-/Buffer-/Textur-ID-Tupeln und erfolgreichem Cleanup von Deck und Server. Sie bleibt eine Guard- und Kausalitätsdiagnose; die vollständigen UI-Tests liefern den separaten Nachweis für Recording, Replay und Sessionisolation.

### UI-Volltests der letzten Fortsetzung

Die bestandenen Läufe `visible`, `partial` und `restored` meldeten jeweils `acceptance=passed`, Szene `context_menu`, zwei Sessions und Bevy 0.19.1. Je Lauf entstanden acht abgenommene PNGs. Damit liefen neben den Bildassertionen auch die unveränderten Recording-, Replay- und Sessionisolationsprüfungen bis zum Ende.

Im `partial`-Lauf waren alle zehn gemessenen Platzierungen exakt halb verdeckt. Im `restored`-Lauf wurden beide Sessions getrennt geprüft:

- Session `7e25acea85a1db18c29cf950cde50956`: acht stabile Verdeckungssamples über 279,711 ms, danach acht stabile Wiederherstellungssamples über 674,434 ms.
- Session `25cb31ad86c0b5d741ad5a023e773601`: acht stabile Verdeckungssamples über 109,683 ms, danach acht stabile Wiederherstellungssamples über 117,714 ms.

In beiden Fällen deckte das Deck zuvor das vollständige Zielrechteck ab, das Visible-Bit fehlte, und nach Entfernung des Decks war die Schnittfläche null und das Visible-Bit wieder gesetzt. Der `covered`-Lauf bestätigte dieselbe Vollverdeckung, scheiterte aber wie vorgesehen beim ersten Capture am Guard. Er führte deshalb die nachfolgenden UI-Prüfungen nicht aus und zählt nicht als bestandene Bildabnahme.

## Ausgeführte Grafikbefehle

Die folgenden Befehle liefen in dieser Reihenfolge jeweils genau einmal:

```sh
# 1, bestanden
python3 tests/diagnostics/capture_scene_visibility.py --scene blend_modes
# 2, bestanden
python3 tests/diagnostics/capture_scene_visibility.py --full-test blend_modes --condition visible
# 3, bestanden
python3 tests/diagnostics/capture_scene_visibility.py --full-test blend_modes --condition partial
# 4, erwartete Guard-Ablehnung, Bildabnahme nicht bestanden
python3 tests/diagnostics/capture_scene_visibility.py --full-test blend_modes --condition covered
# 5, bestanden, vorherige Verdeckung im selben Prozess nachgewiesen
python3 tests/diagnostics/capture_scene_visibility.py --full-test blend_modes --condition restored

# 6, bestanden
python3 tests/diagnostics/capture_scene_visibility.py --scene mesh_picking
# 7, bestanden
python3 tests/diagnostics/capture_scene_visibility.py --full-test mesh_picking --condition visible
# 8, bestanden
python3 tests/diagnostics/capture_scene_visibility.py --full-test mesh_picking --condition partial
# 9, erwartete Guard-Ablehnung, Bildabnahme nicht bestanden
python3 tests/diagnostics/capture_scene_visibility.py --full-test mesh_picking --condition covered
# 10, Infrastrukturfehler vor dem ersten Screenshot
python3 tests/diagnostics/capture_scene_visibility.py --full-test mesh_picking --condition restored
```

Nach Befehl 10 wurde gemäß Freigaberegel angehalten. Nach der Reader-Korrektur und ihren headless Regressionstests wurden sechs offene Fälle neu freigegeben. Davon liefen diese beiden genau einmal und in dieser Reihenfolge:

```sh
# Fortsetzung 1, bestanden; neuer Artefaktordner
python3 tests/diagnostics/capture_scene_visibility.py --full-test mesh_picking --condition restored

# Fortsetzung 2, Infrastrukturfehler während des zweiten Captures
python3 tests/diagnostics/capture_scene_visibility.py --scene ui
```

Nach dem zweiten Fortsetzungsbefehl wurde erneut angehalten. Nach Korrektur und headless Prüfung der Mehrschreiber-Race wurden fünf UI-Fälle neu freigegeben. Davon liefen diese beiden genau einmal und strikt nacheinander:

```sh
# Zweite UI-Fortsetzung 1, vollständige A/B/C/A-Diagnose bestanden
python3 tests/diagnostics/capture_scene_visibility.py --scene ui

# Zweite UI-Fortsetzung 2, Infrastrukturfehler vor dem ersten Screenshot
python3 tests/diagnostics/capture_scene_visibility.py --full-test ui --condition visible
```

Nach dem Fehler der Zustandskontrolle wurde erneut angehalten. Anschließend wurden alle fünf UI-Fälle als neue Läufe freigegeben und genau einmal strikt nacheinander ausgeführt:

```sh
# Dritte UI-Fortsetzung 1, vollständige A/B/C/A-Diagnose bestanden
python3 tests/diagnostics/capture_scene_visibility.py --scene ui

# Dritte UI-Fortsetzung 2, vollständiger Test bestanden
python3 tests/diagnostics/capture_scene_visibility.py --full-test ui --condition visible

# Dritte UI-Fortsetzung 3, vollständiger Test bestanden
python3 tests/diagnostics/capture_scene_visibility.py --full-test ui --condition partial

# Dritte UI-Fortsetzung 4, erwartete Guard-Ablehnung; Bildabnahme nicht bestanden
python3 tests/diagnostics/capture_scene_visibility.py --full-test ui --condition covered

# Dritte UI-Fortsetzung 5, vollständiger Test nach nachgewiesener Verdeckung bestanden
python3 tests/diagnostics/capture_scene_visibility.py --full-test ui --condition restored
```

## Infrastrukturfehler und Werkzeugstand

Im Mesh-`restored`-Lauf las `read_json_lines` die AppKit-Datei genau während eines Append. Die letzte Zeile war in diesem Moment noch nicht abgeschlossen und `json.loads` meldete `JSONDecodeError` bei Spalte 896. Nach Prozessende ist die gespeicherte Datei vollständig und alle 150 Zeilen sind gültiges JSON. Das ist eine Reader-Race im Diagnosewerkzeug, kein Bevy-, Guard- oder Szenenbefund.

Der Lauf belegt vor dem Fehler neun aufeinanderfolgende vollständig verdeckte Samples 139 bis 147. Die letzten acht spannen 247,241 ms auf und erfüllen damit die Verdeckungsschranke. Sample 148 ist wieder sichtbar, reicht allein aber nicht für die verlangten acht stabilen Wiederherstellungssamples. Deshalb gilt dieser Fall weder als wiederhergestellt noch als Bildabnahme.

Nach dem ersten Halt wurde der Reader so begrenzt, dass er nur einen aktuell unvollständigen, noch nicht mit Newline abgeschlossenen Tail zurückstellt. Eine abgeschlossene ungültige JSONL-Zeile bleibt ein Fehler. Vier headless Regressionstests bestanden. Danach bestand der neue Mesh-`restored`-Lauf unter einem neuen Artefaktpfad. Der ursprüngliche Lauf wurde nicht wiederholt oder überschrieben.

Die UI-Diagnose zeigte eine andere Race. `capture-chain.jsonl` hat mehrere Schreiber im App-Prozess: die Bevy-Render-Instrumentierung und den temporär instrumentierten woodpecker-Adapter. Beide öffnen dieselbe Datei im Append-Modus. Ihre mehrteiligen `writeln!`-Ausgaben verschachtelten sich in den abgeschlossenen Zeilen 88 und 89. Nach Prozessende bleiben beide Zeilen ungültig. Der Reader ignorierte sie deshalb zu Recht nicht. Dieser Mehrschreiberfehler war von den geforderten Tail-Regressionstests nicht abgedeckt. Bis zur getrennten Werkzeugkorrektur und neuen Freigabe gab es keinen weiteren Grafiklauf.

### Schreiberinventar und Korrektur

Vor der Korrektur beschrieben im jeweiligen Bevy-App-Prozess zwei unabhängige Module dieselbe `capture-chain.jsonl`:

- `bevy_render::capture_causality` schreibt Surface-Acquire, Prepare, Copy oder Skip, Submit und Map-Request aus Render-Schedule und Renderer sowie Map-Success und Bildrückgabe aus einem `AsyncComputeTaskPool`-Task.
- `woodpecker::session::screenshot::capture` schreibt Start und Antwort des Adapter-Captures aus dem App-/Control-Pollpfad.

Damit waren mindestens Renderthread, Compute-Task und App-Pollpfad beteiligt. Beide Module lebten im selben PID, besaßen aber keinen gemeinsamen Mutex. Bei UI-Mehrsessionstests erbten außerdem mehrere App-Prozesse denselben Dateipfad. Der AppKit-Sampler schrieb nicht in `capture-chain.jsonl`, sondern in `appkit-main.jsonl`; dort konnten vor der Korrektur jedoch ebenfalls mehrere Sessionprozesse denselben Pfad erben. Der Python-Controller schreibt nur seine eigenen `commands.jsonl`, Ergebnis- und Zustandsdateien.

Die Diagnose verwendet nun getrennte Shards pro Ereignisquelle und PID:

```text
capture-chain/bevy_render-<pid>.jsonl
capture-chain/woodpecker_adapter-<pid>.jsonl
appkit-main/appkit_main-<pid>.jsonl
```

Jede Quelle besitzt genau einen prozesslokalen `Mutex<File>`. Die vollständig serialisierte JSON-Zeile und ihr Newline werden mit `write_all` und `flush` innerhalb dieses Mutex geschrieben. Quellen und Prozesse teilen keine Datei mehr. Öffnungs-, Lock-, Schreib- und Flushfehler werden nicht verworfen, sondern lassen die temporäre Diagnose mit einer sichtbaren Fehlermeldung scheitern.

Die Auswertung liest alle Shards. Sie korreliert Adapter-Request-ID plus Session-ID mit Screenshot-Entity und anschließend Frame-, Capture-, Buffer- und Capture-Texture-ID. Dateireihenfolge und Zeitstempel sind dafür nicht maßgeblich.

Sieben headless Regressionstests bestanden. Die vier bestehenden Reader-Tests blieben erhalten, einschließlich des Fehlers für abgeschlossene ungültige JSONL-Zeilen. Zwei neue Tests verwenden den tatsächlichen Rust-Schreibweg und prüfen drei gleichzeitige Prozesse, je sechs Threads, beide Ereignisquellen, 1.440 eindeutige Events und Nutzlasten von 8.192 Bytes sowie einen sichtbaren Schreibfehler. Jede Zeile war gültiges JSON; kein Ereignis fehlte oder kam doppelt vor; jede Quelle und PID erhielt einen eigenen Shard. Ein weiterer Test prüft die ID-basierte Korrelation bei abweichender Ankunftsreihenfolge. Das UI-Binary und die temporäre UI-Testkopie wurden danach headless neu gebaut.

### Infrastrukturfehler im sichtbaren UI-Volltest

Evidenz: `target/capture-scene-full-ui-visible-m00zty0m/`

Der temporäre UI-Test erreichte die erste `visibility.place`-Operation, aber keinen `screenshot.capture`-Befehl. Für Session `061524ca3731c404585c7e593a634832`, PID 94895, wurden 1.340 gültige AppKit-Samples geschrieben. Der letzte Zustand war sichtbar, nicht miniaturisiert, mit Visible-Bit und unveränderter Geometrie `(704, 654, 320, 212)`. Auch die letzten 20 Samples hatten identische Zustandsfelder und spannten 175,374 ms.

Die Stabilitätsimplementierung betrachtet jedoch nur `tail[-8:]`. Wegen der ungefähr 9-ms-Samplerate spannten die letzten acht identischen Samples lediglich 66,347 ms und unterschritten die unveränderte 100-ms-Schranke. Weitere bereits vorhandene stabile Samples wurden nicht einbezogen. Nach zwölf Sekunden meldete der Controller deshalb `native state not confirmed: owned target window ordered in`. Das ist ein Infrastrukturfehler der Diagnose-Zustandskontrolle, kein UI-Bild-, Recording-, Replay- oder Sessionisolationsbefund. Gemäß Stopregel wurde in jener Fortsetzung nichts wiederholt und kein weiterer UI-Volltest gestartet. Das Deck wurde mit Exitcode 0 beendet; der Test-/Serverprozess ist ebenfalls beendet.

## Artefakte

Jeder ausgeführte Fall besitzt je nach Umfang:

- `result.json` für abgeschlossene Diagnosen und Volltest-Wrapper,
- in älteren Läufen `capture-chain.jsonl` und `appkit-main.jsonl`,
- nach der Mehrschreiberkorrektur `capture-chain/<quelle>-<pid>.jsonl` und `appkit-main/appkit_main-<pid>.jsonl`,
- `raw-readbacks/` mit Bevy-Bytes vor dem Guard,
- `visibility-states.json` mit Ziel-/Deckgeometrie und Sessionzuordnung,
- `stdout.log` und `stderr.log` bei vollständigen Tests,
- `deck-cleanup.json` beziehungsweise `cleanup.json`.

Der erste Infrastrukturfehler trat vor dem Schreiben seines Wrapper-`result.json` auf. Seine übrigen Dateien liegen unter `target/capture-scene-full-mesh_picking-restored-rdpbnej6/`. Der bestandene Fortsetzungslauf liegt getrennt unter `target/capture-scene-full-mesh_picking-restored-9qntd6z6/`.

Die fehlgeschlagene erste UI-Diagnose schrieb `result.json` und vollständige Cleanupdaten nach `target/capture-scene-visibility-ui-h5urkuye/`. Die dauerhaft korrupten Korrelationseinträge bleiben dort als Fehlerbeleg erhalten. Die beiden später bestandenen Diagnosen liegen getrennt unter `target/capture-scene-visibility-ui-s7vq0la8/` und `target/capture-scene-visibility-ui-iivvmkey/`.

Der frühere sichtbare UI-Volltest stoppte vor dem Schreiben seines Wrapper-`result.json`. `stderr.log`, die 1.340 AppKit-Samples, Bevy-Ereignisse und das erfolgreiche Deck-Cleanup bleiben unter `target/capture-scene-full-ui-visible-m00zty0m/` erhalten. Der neue bestandene sichtbare Lauf liegt unter `target/capture-scene-full-ui-visible-c190r5jx/`. Die weiteren neuen Volltests liegen unter `target/capture-scene-full-ui-partial-8smzq328/`, `target/capture-scene-full-ui-covered-xfl0m5lh/` und `target/capture-scene-full-ui-restored-w43vhbi0/`.

## Grenzen

- Die freigegebene Fallmatrix ist vollständig ausgeführt. `covered` bleibt bei Blend, Mesh und UI jeweils eine erwartete Guard-Ablehnung und damit eine nicht bestandene Bildabnahme.
- Für UI liefern `visible`, `partial` und `restored` einen Volltestnachweis für Bilder, Recording, Replay und Isolation von zwei Sessions. Der früh abbrechende `covered`-Fall kann diese nachgelagerten Prüfungen nicht abnehmen.
- Die vollständigen Verdeckungsfälle können mit dem aktuellen Bevy-0.19.1-Window-Capture nicht bestehen. Der Guard verhindert nur die falsche Erfolgsmeldung.
- Die A/B/C/A-Diagnosen frieren einen bekannten Szenenzustand ein. Sie ersetzen nicht die vollständigen Interaktionsabläufe.
- Der Befund gilt für macOS 26.6.2, Apple M1 Pro und Metal. Er ist keine Aussage über andere Betriebssysteme, GPUs oder Backends.
- Es wurde keine Produktionskorrektur, Bevy-Migration oder RC-Prüfung vorgenommen.

## Cleanup und Freigabe des Grafikslots

Alle neunzehn gestarteten Deckprozesse endeten mit Exitcode 0. Der neueste UI-Diagnosetreiber protokolliert für Deck und Server ebenfalls Exitcode 0; beide Prozesse laufen nicht mehr. Alle vier neuen UI-Volltests beendeten ihr Deck im `finally`-Pfad mit Exitcode 0. Das gilt auch für den erwarteten Guard-Abbruch bei `covered`. Die vollständigen Testskripte führten ihre Bereinigung auch bei Guard-Ablehnung und den früheren Infrastrukturfehlern aus. Eine abschließende Prozessprüfung fand keinen laufenden `woodpecker`-, `blend_modes`-, `mesh_picking`-, `context_menu`- oder Deckprozess.

Die grafische Messung ist beendet. `GUI_SLOT_RELEASED`.
