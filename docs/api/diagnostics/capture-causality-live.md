# Capture-Kausalität: kontrollierter macOS-Live-Versuch

Stand: Probe kompiliert und kontrollierter A/B/A-Lauf abgeschlossen. Der
beweiskräftige Lauf liegt unter `target/capture-causality-live-x01e81hr/`.
Er enthält genau einen Warp und je einen Capture im sichtbaren, verdeckten und
wieder sichtbaren Zustand.

## Fragestellung

Der Versuch unterscheidet diese Erklärungen für den gemessenen RGB0-Readback:

1. Vollständige Verdeckung führt zu `CurrentSurfaceTexture::Occluded`. Im selben
   Renderframe bereitet Bevy Screenshot-Textur und Buffer vor, überspringt wegen
   der fehlenden Swapchain-View aber `copy_texture_to_buffer`, submittet die
   Queue und mappt anschließend genau diesen unbeschriebenen Buffer.
2. Die Kopie wird aufgezeichnet, liefert wegen eines Render-, Pipeline- oder
   Treiberfehlers aber Nullpixel.
3. Ein veralteter Kamera- oder Größenstand erklärt das Bild, unabhängig von der
   Surface-Akquise.

Die erste Erklärung sagt für einen festen A/B/A-Lauf Folgendes voraus:

- sichtbar, nicht fokussiert: Acquire erfolgreich, Copy aufgezeichnet,
  `map_success` für denselben Buffer, nichtleerer Bevy-Readback;
- vollständig durch ein eigenes Fenster verdeckt: Acquire `occluded`, Copy für
  denselben Capture übersprungen, Mapping erfolgreich, roher Bevy-Readback RGB0;
- wieder sichtbar: Acquire und Copy wieder erfolgreich, PNG bytegleich zum
  Ausgangsbild.

## Dauerhafte Diagnosewerkzeuge

- `tests/diagnostics/capture_causality_live.py` baut und fährt die feste
  Messfolge. Es führt genau einen initialen Warp aus. Danach gibt es keine
  Simulationsticks und keine Wiederholung bis zu einem grünen Bild.
- `tests/diagnostics/capture_causality_instrument.py` instrumentiert eine
  wegwerfbare Kopie von `bevy_render 0.19.1`.
- `tests/diagnostics/capture_causality_native.rs` tastet das eigene `NSWindow`
  mit `NonSend<MainThreadMarker>` auf dem Prozess-Hauptthread ab. Das System
  läuft direkt im bei jedem App-Durchlauf ausgeführten `Main`-Schedule, nicht in
  den nur bei einem Warp laufenden Simulations-Schedules. Erfasst werden
  `visible`, `key`, `miniaturized`, `occlusionState`, native Fenstergeometrie,
  Bevy-Fenstergröße, Skalierungsfaktor sowie Kamera-Zielgröße und Viewport.
- `tests/diagnostics/capture_causality_deck.swift` erzeugt ausschließlich ein
  eigenes AppKit-Fenster. In Stellung `small` nimmt es dem Testfenster den Fokus,
  ohne es abzudecken. In Stellung `cover` bedeckt es die vereinigte Fläche aller
  Bildschirme. Es steuert keine fremden Fenster und benötigt weder
  Accessibility-Rechte noch Änderungen an Systemeinstellungen.

Die Bevy-Probe vergibt monotone `frame_id`, `capture_id`, `buffer_id` und
`capture_texture_id`. Dieselben IDs stehen an diesen Grenzen:

1. Surface-Akquise mit Ergebnis sowie Kamera-, Fenster- und Surface-Größe,
2. Vorbereitung von Screenshot-Textur und Readback-Buffer,
3. Aufruf oder begründetes Überspringen von `copy_texture_to_buffer`,
4. Queue-Submit mit `copy_encoded`,
5. `map_requested` und `map_success`,
6. zurückgeliefertes Bevy-`Image` mit Bytezahl, RGB-Statistik, FNV-1a-Hash und
   einer Rohkopie unter dem Evidenzverzeichnis.

`copy_texture_to_buffer_encoded` belegt nur die Encoderaufzeichnung.
`queue_submitted` und das spätere `map_success` für dieselbe `buffer_id` belegen
die nachfolgende GPU-Nutzung und den abgeschlossenen Readback. Erst die
`image_returned`-Statistik beschreibt den Inhalt.

## Schutz des normalen Adapters

Der bestehende Surface-Guard bleibt aktiv. Der verdeckte Fall muss über die
öffentliche API mit `screenshot_window_unavailable` enden und darf keine PNG
schreiben. Die temporäre Bevy-Probe misst den ursprünglichen fehlerhaften
Readback vor diesem Guard und legt ihn getrennt unter
`target/capture-causality-live-*/raw-readbacks/` ab. Sie führt kein
fehlerhaftes Image als normalen woodpecker-Erfolg ein.

## Ausführung

Voraussetzungen sind ein wacher macOS-Desktop, Xcode Command Line Tools und die
bereits lokal verfügbaren gelockten Cargo-Abhängigkeiten.

```sh
python3 tests/diagnostics/capture_causality_live.py
```

Der Befehl erledigt Folgendes:

1. Er kopiert `bevy_render 0.19.1` und `bevy_test_apps` nach
   `target/capture-causality/`.
2. Er instrumentiert nur diese Kopie und trägt den lokalen `bevy_render`-Patch
   im `Cargo.toml` der kopierten Test-App ein. Der normale Root-CLI-Build bleibt
   ungepatcht. Er läuft mit `cargo build --offline --locked --features cli --bin woodpecker`.
3. Der Test-App-Build läuft ebenfalls mit `--offline`. Registry-Quellen werden
   nicht verändert.
4. Er kompiliert das eigene Deckfenster, startet eine frische Session, führt
   genau `tick.warp.start {"ticks":1}` aus und bestätigt jeden nativen Zustand
   vor dem jeweiligen Capture mit einer Frist von acht Sekunden.
5. Er misst genau einmal in der Folge sichtbar ohne Fokus, vollständig verdeckt
   und wieder sichtbar.

Nur den Build kann man so prüfen:

```sh
python3 tests/diagnostics/capture_causality_live.py --prepare-only
```

Ein erfolgreicher Lauf schreibt:

- `target/capture-causality-live-*/causality.jsonl`
- `target/capture-causality-live-*/appkit-main.jsonl`
- `target/capture-causality-live-*/raw-readbacks/`
- `target/capture-causality-live-*/commands.jsonl`
- `target/capture-causality-live-*/result.json`
- die beiden sichtbaren PNGs im Session-Artefaktverzeichnis

Das Programm verlangt für den Abschluss:

- bestätigte AppKit-Zustände
  `visible=true,key=false,miniaturized=false,occlusion_visible=true`,
  danach `visible=true,miniaturized=false,occlusion_visible=false`, danach
  wieder den ersten Zustand;
- `occluded`, fehlende Swapchain-View, übersprungene Copy, Queue-Submit,
  erfolgreiches Mapping und RGB0-`Image` mit identischen Capture-/Buffer-IDs im
  verdeckten Frame;
- erfolgreiche Copy und nichtleere Pixel in den beiden sichtbaren Frames;
- bytegleiche Ausgangs- und Rückkehr-PNGs;
- identischen reflektierten `SceneState`, `Camera` und `Transform` nach allen
  drei Captures;
- Ablehnung ohne PNG durch den unveränderten woodpecker-Guard im verdeckten
  Fall.

Ein nicht erreichter nativer Zustand bricht die Messung ab. Fokuswunsch,
Deckfensterkommando oder zeitliche Nähe werden nicht als Nachweis gewertet.
`hidden` existiert im Deckwerkzeug nur als zusätzlicher manueller Zustand und
ersetzt die Verdeckungsmessung nicht.

## Vorbereitung und verworfener Vorlauf

`--prepare-only` deckte ausschließlich Fehler in der Diagnoseprobe auf. Die
exakte Quelltextersetzung für den Window-Zweig war noch nicht eindeutig, die
instrumentierte `CurrentSurfaceTexture`-Fallunterscheidung nannte drei in wgpu
29.0.4 nicht vorhandene Varianten, und die direkte AppKit-Abhängigkeit zog mit
ihren Default-Features nicht gecachte Offline-Abhängigkeiten ein. Danach waren
noch `NSResponder` und die aktuelle Schreibweise
`NSWindowOcclusionState::Visible` nötig. Diese Korrekturen ändern keine
Messschranke.

Der erste gestartete Vorlauf unter
`target/capture-causality-live-xmw1jsh2/` brach vor dem ersten Capture ab. Der
AppKit-Sampler hing damals in `First` und lief nach dem einzigen vereinbarten
Warp nicht weiter. Das Log enthält deshalb nur ein natives Sample und keine
Screenshot-Kette. Der Sampler wurde in `Main` verschoben. Es wurde kein
zusätzlicher Warp ergänzt.

Anschließend bestanden Python-Syntaxprüfung, Swift-Kompilation, der ungepatchte
Root-Build mit `--offline --locked` und der gepatchte Test-App-Build mit
`--offline`. Der einzige Linkerhinweis betraf die bekannte Größe von
`__eh_frame`.

## Beweiskräftiger Lauf

Der Lauf `target/capture-causality-live-x01e81hr/` bestand alle Assertions. Die
AppKit-Samples 1 bis 129 und die Surface-Akquiseframes 1 bis 129 bilden im Lauf
eine lückenlose 1:1-Folge. Rund um jeden Capture blieb der native Zustand über
mehrere Samples gleich. Damit ändert auch ein möglicher Pipelineversatz um einen
App-Durchlauf die Zustandsklassifikation nicht.

| Zustand | Frame und nativer Befund | Acquire, Copy, Submit und Map | Bilddaten und Adapter |
| --- | --- | --- | --- |
| sichtbar, nicht fokussiert | Frame/Sample 10: `visible=true`, `key=false`, `miniaturized=false`, `occlusion=8194`, Visible-Bit gesetzt | `success`; Capture/Textur/Buffer `1/1/1`; Copy aufgezeichnet; Queue eingereicht; Mapping erfolgreich | 3.686.400 Rohbytes, 2.763.510 RGB-nichtnullwertige Bytes; PNG 1280×720, SHA-256 `5960ef705882fae4f85ea4746fcae657cb61eb89ed990494663fff23e421fec2` |
| durch eigenes Deckfenster verdeckt | Frame/Sample 63: `visible=true`, `key=false`, `miniaturized=false`, `occlusion=8192`, Visible-Bit nicht gesetzt | `occluded`; Capture/Textur/Buffer `2/2/2`; `copy_skipped` mit `swap_chain_view_missing`; Queue ohne Copy eingereicht; Mapping erfolgreich | 3.686.400 nullwertige Rohbytes, SHA-256 `0c660f2bd3eff3150dd0040789abe2291613b9af319df870203d4f77a4913a5f`; Guard lehnt mit `screenshot_window_unavailable` ab; keine PNG-Datei |
| wieder sichtbar, nicht fokussiert | Frame/Sample 79: derselbe sichtbare native Zustand wie am Anfang | `success`; Capture/Textur/Buffer `3/3/3`; Copy aufgezeichnet; Queue eingereicht; Mapping erfolgreich | Rohbytes bytegleich zu Capture 1; PNG bytegleich zu Capture 1 |

In allen drei Screenshot-Frames stimmen Surface-, Capture-Textur-, extrahierte
Fenster- und Kamera-Zielgröße mit 1280×720 überein. Beide Kameras sind aktiv,
haben keinen Viewport und schreiben in dasselbe Fenster. `size_changed` und
`present_mode_changed` sind jeweils `false`; der Skalierungsfaktor ist 1. Die
reflektierten Werte für `SceneState`, `Camera` und `Transform` blieben nach
allen Captures identisch. `commands.jsonl` enthält genau einen
`tick.warp.start {"ticks":1}` und genau drei Screenshot-Requests. Die Logs
enthalten keinen Geräteverlust und keinen wgpu-Validierungsfehler.

## Kausalurteil und Grenze

Für diesen kontrollierten Lauf ist H1 bestätigt: Der einzige Zustandswechsel ist
die native Verdeckung. Im verdeckten Capture-Frame liefert die Akquise
`occluded`, der Swapchain-View fehlt, Bevy überspringt die Copy desselben
vorbereiteten Textur-/Buffer-Paars, reicht die Queue ein und mappt anschließend
genau diesen Buffer als RGB0. Der unveränderte Guard verhindert das falsche
PNG-Ergebnis. Nach Entfernung des eigenen Occluders kehren Akquise, Copy und das
bytegleiche Bild ohne weiteren Simulationstick zurück.

Eine Resize-/Kameralücke erklärt Capture 2 nicht, weil alle erfassten Größen und
Viewports gleich bleiben und die Copy dort gar nicht stattfindet. Eine
Cold-Renderbereitschaft erklärt den A/B/A-Wechsel ebenfalls nicht: Die beiden
sichtbaren Captures sind bytegleich, während der verdeckte Frame vor dem
Readback-Copy abbricht. Der Lauf ist keine allgemeine Widerlegung anderer
schwarzer Renderpfade und ordnet historische Schwarzbilder ohne diese IDs nicht
nachträglich zu.

Das Protokoll ist für den isolierten Einzelprozess eindeutig, erfüllt die
formale Liste gemeinsamer Metadaten aus den Prüfkriterien aber noch nicht
wörtlich an jeder Zeile. Bevy-Events tragen weder PID noch Session- und
Command-Request-ID; die nativen Zeilen heißen `sample_id` statt
`render_frame_id`. Die lückenlose 1:1-Folge und die über mehrere Frames stabilen
nativen Zustände schließen hier eine falsche Zuordnung aus. Für eine dauerhaft
wiederverwendbare Mehrprozess-Messung sollten diese Felder trotzdem ergänzt
werden. Das ändert das Kausalurteil dieses Laufs nicht und ist kein Capture-Fix.

## Cleanup

Die Diagnose verändert keine Manifeste im Arbeitsbaum. Alle Dependencykopien,
lokalen Cargo-Patches, Binaries und Live-Artefakte liegen unter `target/`.

```sh
rm -rf target/capture-causality target/capture-causality-live-*
```

Dieser Cleanup ist optional. Er darf erst nach Sicherung benötigter Evidenz
ausgeführt werden.
