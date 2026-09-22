# Diagnosematrix: vollständig schwarze Screenshots

## Auftrag und Grenze

Untersucht wird ein sporadischer Fehler im CLI-/Session-/Bevy-Weg: Der
Screenshot-Command meldet Erfolg und liefert ein strukturell gültiges PNG,
dessen gesamte RGB-Nutzlast null ist. Nicht nur die erwarteten Objekte fehlen.
Die Szene lässt sich zuvor über Inspect und explizite Warps bedienen.

Die Nutzerhypothese lautet: Die Session meldet Ready, bevor der Renderer bereit
ist. Ready und ein abgeschlossener Warp sind derzeit kein Nachweis eines vollständig
gerenderten Bildes. Ob dieser Unterschied die schwarzen Bilder verursacht,
ist noch offen.

**Ergebnis der drei Untersuchungen:** Der Fehlerpfad „fehlender Swapchain-View,
übersprungener Bildtransfer, trotzdem erfolgreicher Readback“ ist aus Quellen
belegt und durch instrumentierte Live-Läufe gestützt. Der schwarze Lauf hatte
ein nativ als verdeckt gemeldetes Fenster. Ein späterer sichtbarer Gegenlauf
lieferte korrekte Bilder. Die Ursache jedes historischen Ausfalls ist damit
nicht bewiesen; der Wechsel zwischen den Live-Bedingungen war nicht kontrolliert.

Die Untersuchung erfolgt mit drei isolierten Workern. Nur der Cold-Start-Worker
führt zunächst Live-Render-Versuche aus. Die anderen Worker untersuchen Quellpfade
und führen headless Tests aus. So verändern parallele Testfenster und GPU-Last
nicht unkontrolliert dieselbe Messreihe. Fehlende Live-Belege bleiben ausdrücklich
offen. In dieser ursprünglichen Diagnosephase gab es keine Produktkorrekturen,
automatischen Erfolgswiederholungen oder Änderungen am Ready-Vertrag.
Die anschließende Adapter-Absicherung ist weiter unten getrennt dokumentiert.

## Statusbegriffe

- **Beobachtet:** In Artefakten oder einem ausgeführten Test nachgewiesen.
- **Mechanismus belegt:** Der konkrete Codepfad ist nachgewiesen; seine Beteiligung
  an den historischen schwarzen Aufnahmen kann trotzdem offen sein.
- **Offen:** Plausible Hypothese ohne ausreichenden unterscheidenden Versuch.
- **Widerlegt:** Eine konkrete Vorhersage scheitert an einer kontrollierten Messung.
  Das widerlegt nicht automatisch alle verwandten Ursachen.

## Hypothesen und Zuordnung

| ID | Hypothese / Vorhersage | Worker / Test | Status und Befund |
| --- | --- | --- | --- |
| H1 | Capture kommt vor dem ersten vollständigen Renderdurchlauf. Spätere Aufnahmen desselben eingefrorenen Zustands werden ohne weitere Ticks korrekt. | Cold Start: begrenzte Capture-Zeitreihe mit unverändertem Zustand und vollständiger Erfassung aller Ausgänge. | Erste Messreihe: vier frische Prozesse, je Warp(1) und sechs Captures, alle 24 Bilder RGB0, auch bis 9,6 s nach Warp. „Kurz warten genügt“ bestätigt sich nicht. Cold-Start-Abhängigkeiten insgesamt bleiben offen. |
| H2 | Assets oder GPU-Pipelines sind noch nicht bereit, beim Start oder nach HDR-/Unlit-Wechsel. | Cold Start: Capture-Zeitreihe und Fenster-/Renderprobe. | Nicht vollständig ausgeschlossen oder separat reproduziert. Der gemessene fehlende Swapchain-View liefert bereits eine ausreichende Erklärung für die aktuelle Reproduktion. Frischer Prozessstart ist nicht gleich leerer GPU-/Shader-Cache. |
| H3 | Ein vorbereiteter Readback-Buffer wird ausgelesen, obwohl kein Bild hineinkopiert wurde. Readback-Erfolg wird fälschlich als Bildbereitschaft verstanden. | Capture/Readback: Zielumleitung, Renderpass, GPU-Copy und Abschluss verfolgen; headless Tests an vorhandenen Nähten. | Mechanismus im Quellpfad belegt: ohne Swapchain-View überspringt Bevy `render_screenshot`, sammelt den vorbereiteten Buffer aber trotzdem ein. Beteiligung an historischen Aufnahmen noch nicht bewiesen. |
| H4 | Verdeckung oder eine nicht verfügbare Window-Surface verhindert die GPU-Kopie, nicht aber Capture-Abschluss. | Fenster/Schedules: Quellenpfade; Cold Start: native Fenster-/Render-Probe. | Live-Befund stützt den Mechanismus: verdeckter Lauf 139/139 Frames ohne Swapchain-Textur/View/Format und 4/4 PNGs RGB0. Sichtbarer Gegenlauf 184/184 Frames mit diesen Ressourcen und 4/4 PNGs korrekt. Kein kontrollierter Wechsel im selben Prozess; historischer Surface-Status unbekannt. |
| H5 | Angehaltene Main-Schedules lassen notwendige Render-Vorbereitung aus; erst ein weiterer Warp repariert sie. | Fenster/Schedules: Zuordnung von Kamera-/Asset-/Fenstervorbereitung und kontrolliertem Leerlauf prüfen. | Offen für RGB0. Der separate Cluster-Initialisierungsfehler vor dem ersten Tick ist bereits beobachtet, aber nicht als Ursache der späteren schwarzen Bilder belegt. |
| H6 | Größe oder Identität des Renderziels wechselt; Capture und Kamera verwenden unterschiedliche Stände. | Fenster/Schedules und Capture/Readback: Größe, Skalierung und Zielidentität verfolgen. | Größenabweichung im Test beobachtet und korrigiert. Sie erklärt nicht allein die schwarzen Blend-PNGs: dort haben erfolgreiche und fehlgeschlagene Bilder dieselben 1280×720 Pixel. |

Die Worker liefern Berichte mit Testbefehlen, Quellstellen, Ergebnis und
verbleibender Unsicherheit. Eine endgültige Ursache der historischen Ausfälle
ist noch nicht festgestellt.

### Erster Quellenbefund: fehlender Swapchain-View

Der Fenster-/Schedule-Worker meldet einen zusammenhängenden Fehlerpfad.
Die Elternuntersuchung hat die entscheidenden Verzweigungen ebenfalls in den
lokal verwendeten Abhängigkeiten geprüft:

1. wgpu-hal 29.0.4, `src/metal/surface.rs`: Ein nicht sichtbarer
   `NSWindow.occlusionState` kann `SurfaceError::Occluded` auslösen.
2. Bevy 0.19.1 lässt in diesem Fall den Swapchain-View des Fensters leer.
3. In `bevy_render/src/view/window/screenshot.rs`,
   `submit_screenshot_commands`, führt ein fehlender View zu `continue`,
   bevor `render_screenshot` und damit die GPU-Kopie aufgerufen werden.
4. `collect_screenshots` iteriert trotzdem über alle vorbereiteten Screenshots
   und mappt deren Buffer. Ein erfolgreiches Mapping besagt nicht, dass zuvor
   tatsächlich ein Bild hineinkopiert wurde.

Das ist enger als die anfängliche Hypothese einer nicht gezeichneten Szene:
Auch ein vorbereiteter Screenshot kann den Kopierschritt überspringen.
Der Capture-Worker prüft diesen Pfad unabhängig. Der Live-Worker wurde gebeten,
sichtbares und verdecktes eigenes Testfenster gezielt zu vergleichen.
Es gibt noch keinen Produktfix. Der später erhobene Live-Befund steht unten.

### Abgeschlossener Fenster-/Schedule-Bericht

Der Bericht [window-schedules.md](window-schedules.md) enthält die vollständige
Quellenkette und einen Vorschlag für gezielte Live-Instrumentierung.
Der Worker hat diese Prüfungen erfolgreich ausgeführt:

```sh
cargo test --lib idle_control_drains_render_clock_without_advancing_time
cargo test --lib independent_virtual_devices_ignore_native_input_without_window_focus
python3 tests/diagnostics/window_schedules.py
```

Die ersten beiden sind bestehende headless Verhaltenstests. Die
[neue Quellenprobe](../../../tests/diagnostics/window_schedules.py) ist ausdrücklich
statisch: Sie prüft relevante Verzweigungen in den gelockten Abhängigkeiten,
reproduziert aber keinen schwarzen GPU-Frame.

Separater Befund für H5/H6: Winit kann die `WindowResolution` und die Render-App
die Surface bereits aktualisieren, während `Camera::computed` und die Projektion
bis zum nächsten Warp unverändert bleiben. Das ist eine belegte Aktualisierungslücke,
noch kein Nachweis ihrer Beteiligung an den historischen RGB0-Bildern.

### Abgeschlossener Capture-/Readback-Bericht

Der unabhängige Bericht [capture-readback.md](capture-readback.md) bestätigt
denselben fehlenden Kopierschritt und ergänzt das letzte Glied: wgpu initialisiert
unbeschriebene Mapping-Bereiche mit Nullen. Dadurch kann ein erfolgreicher
Buffer-Readback ein vollständig nullwertiges Bild ergeben. Woodpecker encodiert
dieses Bild als gültiges PNG und meldet `Completed`.

Ein temporärer Test an der vorhandenen `CapturedScreenshots`-Adaptertestnaht
bestätigte `Completed` und RGB0-PNG für ein eingespeistes Null-Image.
Das belegt die Reaktion des Adapters, nicht die Entstehung des Null-Images auf der
GPU. Die temporäre Änderung wurde anschließend vollständig entfernt; der
vorhandene Readback-/PNG-Test bestand erneut. Es wurde keine Render-App gestartet.

Damit bestätigen zwei unabhängige Quellenuntersuchungen H3/H4. Das exakte
Surface-Acquire-Ergebnis wurde auch im späteren Live-Lauf nicht direkt geloggt.
Cold Start und Pipeline-Bereitschaft bleiben mögliche zusätzliche Fehlerquellen,
sind aber zur Erklärung dieser konkreten Quellenkette nicht erforderlich.

### Erste Live-Messreihe

Der Cold-Start-Worker meldet vier frische `blend_modes`-Prozesse mit jeweils
genau einem expliziten Tick und sechs nachfolgenden Captures ohne weitere Ticks.
Alle 24 PNGs waren vollständig RGB0; der reflektierte Simulationszustand blieb
stabil. Folgecaptures bis 9,6 Sekunden nach Warp waren weiterhin schwarz.

Damit hat schlichtes Warten den Fehler in dieser Messreihe nicht beseitigt.
Es ist kein Ausschluss aller Cold-Start-/Asset-Probleme. Als nächste isolierte
Intervention testet der Worker `focused: true` für sein eigenes Testfenster und
setzt die temporäre Änderung danach zurück. Fokus und tatsächliche Sichtbarkeit
sind nicht gleichzusetzen; ein Erfolg unter Fokus allein wäre noch kein
vollständiger Occlusion-Nachweis. Der abgeschlossene Bericht folgt unten.

### Abgeschlossener Live-Bericht

Der Bericht [cold-start.md](cold-start.md) dokumentiert die Messungen und ihre
Grenzen. Das wiederverwendbare
[Messprogramm](../../../tests/diagnostics/cold_start_frozen_captures.py)
führt genau einen Warp aus und nimmt danach eine festgelegte Anzahl Bilder
ohne weitere Ticks auf. Schwarz ist ein protokolliertes Messergebnis, kein
Anlass für automatische Erfolgswiederholungen.

| Lauf | Beobachtung | Ergebnis |
| --- | --- | --- |
| `target/cold-start-9u7gcjre` | Vier frische Prozesse, je Warp(1), dann sechs Captures. | 24/24 RGB0 bis 9,63 s nach Warp; State stabil. |
| `target/cold-start-z2kb05em` | Hauptthread-AppKit-Probe: `visible=true`, `key=false`, `occlusion_visible=false`. | 139/139 Frames ohne Swapchain-Textur/View/Format; 4/4 Captures schwarz. |
| `target/cold-start-bjwe7bt3` | Späterer frischer Prozess mit lesender Probe: `key=true`, `occlusion_visible=true`. | 184/184 Frames mit Swapchain-Textur/View/Format; 4/4 Captures korrekt. |

Die Probes verbinden aktuell schwarze Captures mit fehlenden Swapchain-Ressourcen
und nativ gemeldeter Occlusion. Zusammen mit dem unabhängig geprüften Codepfad
ist dies eine belastbare Erklärung für diese Reproduktion. Das Acquire-Ergebnis
und der tatsächliche GPU-Copy-Aufruf wurden nicht direkt instrumentiert.
Historische Läufe wie `blend-modes-f9gag8t5` besitzen keine solche Fenster-/Renderprobe.

Fokus-/Frontanforderungen am eigenen Fenster erreichten nicht zuverlässig den
gemessenen sichtbaren Zustand. Ein Fokuswunsch ist daher weder ein belastbarer
Fix noch eine Render-Bereitschaftsprüfung. Der sichtbare Gegenlauf entstand in
einem späteren Prozess, nicht durch einen kontrollierten In-Process-Wechsel.

Temporäre Rust-/Cargo-Probes wurden entfernt, das ursprüngliche Binary erneut
gebaut und die eigenen Renderprozesse beendet. Kein produktiver Screenshot-
oder Ready-Vertrag wurde verändert.

## Umgesetzte Absicherung und verbleibende Grenze

Der Adapter prüft jetzt in der Render-World nach der Vorbereitung und vor
`RenderSystems::Render`, ob Window-View und Format vorhanden sind. Diese beiden
Voraussetzungen benötigt der untersuchte Bevy-0.19.1-Kopierpfad. Die Prüfung
gehört zum ersten Extraktionsframe des einzelnen Requests, nicht zu einem
veränderlichen globalen „Renderer bereit“-Flag.

Fehlen diese Ressourcen, folgt `screenshot_window_unavailable` ohne Schreiben
oder Überschreiben. Ein später eingehender Readback wird verworfen. Ein fehlender
Verifikationsbefund schlägt mit `screenshot_failed` fehl, statt ungeprüft Erfolg
zu melden. Vorhandene Frame-Verifikation plus tatsächlich schwarzes Image bleibt
zulässig. Es gibt keine Pixelheuristik, zusätzlichen Ticks, Fokusänderung,
automatische Wiederholung, Bevy-Fork oder Änderung von `Ready`.

Die Implementierung steht in
[capture/surface.rs](../../../src/session/screenshot/capture/surface.rs) und
[capture.rs](../../../src/session/screenshot/capture.rs). Sie prüft die
öffentlichen Voraussetzungen des konkreten Bevy-Kopierpfads, nicht dessen
privaten Copy-Aufruf. Bei einem Bevy-Update muss diese Zuordnung erneut geprüft werden.

Die bewusste erste Korrektur verhindert falschen Erfolg. Sie garantiert nicht,
dass ein verdecktes Fenster trotzdem aufgenommen werden kann. Ein Bevy-Patch,
der Copy und Präsentation trennt, oder ein von der Window-Surface unabhängiger
Capture-Weg wären weitergehende Lösungen. Cold-Start-/Pipeline-Bereitschaft
und Resize-/Camera-Aktualität bleiben eigene Fragen.

### Nachweise der Absicherung

- Neuer Render-World-/Adaptertest „vorbereitet, aber keine Surface“ scheiterte
  vor der Ablehnungslogik und besteht danach. Weitere Tests prüfen unveränderte
  vorhandene Dateien, späte Readbacks, unveränderliche Frame-Zuordnung,
  fehlende Verifikation und ein legitimes schwarzes Bild.
- `target/cold-start-hasx1cq2`: zwei reale Sessions, je drei korrekte PNGs,
  kein RGB0 und keine Ablehnung. Ein Tick pro Session, State stabil.
- `target/cold-start-dcykypfz`: Testanwendung temporär mit unsichtbarem Fenster
  gestartet. Drei Captures jeweils ausdrücklich `screenshot_window_unavailable`,
  keine PNG-Datei, State stabil. Die temporäre Fensteränderung wurde entfernt
  und die ursprünglichen Szenenbinaries erneut gebaut.
- Vollständige Root-Suite bestanden: 136 Bibliotheks-/CLI-Tests,
  7 Beobachtungs- und 13 Session-Prozesstests. Log:
  `target/surface-guard-tests.9KTXDU`. Ohne Default-Features 92 Tests bestanden.
- Erneute strikte Bildabnahmen sind **nicht grün**: Blend
  `target/blend-modes-fyz9uxet` lehnte den dritten Capture ab, Mesh
  `target/mesh-picking-1j9wpyb4` und Context-Menu den ersten.
  Jeweils `screenshot_window_unavailable`, nicht mehr ein erfolgreiches RGB0-PNG.
  Die bestehenden Bildtests wurden nicht auf Akzeptanz dieser Ablehnung abgeschwächt.

Reproduzierbare begrenzte Messreihe mit Prüfung der Absicherung:

```sh
python3 tests/diagnostics/cold_start_frozen_captures.py \
  --sessions 2 --captures 3 --verify-surface-guard
```

Der Prüfmodus verlangt für diese bekannte nichtschwarze Szene entweder ein
nichtschwarzes Bild oder die explizite Surface-Ablehnung ohne Datei. Er protokolliert
beide Fälle getrennt und ersetzt nicht die strikten Szenen-Bildabnahmen.

## Erneut geprüfte vorhandene Artefakte

Die PNGs wurden aus den lokalen Ergebnisordnern erneut vollständig dekodiert,
einschließlich der RGB-Zeilenfilter. Die folgenden Ordner liegen unter `target/`
und sind keine versionierten Fixtures.

| Lauf | Bilder / Größe | Ergebnis |
| --- | --- | --- |
| `mesh-picking-91lxd21b` | `idle.png`, 640×360 | Alle RGB-Werte null. |
| `mesh-picking-h342sacr` | `idle.png`, 640×360 | Alle RGB-Werte null. |
| `mesh-picking-cdmyuwwl` | `idle.png`, 320×180 | Nicht vollständig schwarz; Test scheiterte an fest erwarteter Bildgröße. |
| `mesh-picking-1osc9q3p` | 13 PNGs, jeweils 320×180 | Vollständige Abnahme bestanden, keines der Bilder vollständig schwarz. |
| `mesh-picking-ha7_94g1` | 13 PNGs, jeweils 320×180 | Vollständige Abnahme bestanden, keines der Bilder vollständig schwarz. |
| `blend-modes-tsrdc2m0` | 6 PNGs, jeweils 1280×720 | Vollständige Abnahme bestanden, keines der Bilder vollständig schwarz. |
| `blend-modes-r2sbu_1r` | 6 PNGs, jeweils 1280×720 | Vollständige Abnahme bestanden, keines der Bilder vollständig schwarz. |
| `blend-modes-f9gag8t5` | `unlit-alpha-one.png`, 1280×720 | Alle RGB-Werte null; erster Capture nach vorherigen erfolgreichen Steuerungsprüfungen. |

Der Nutzer arbeitete auch bei geschlossenem Laptopdeckel am externen Bildschirm.
Er hat den Deckel anschließend geöffnet. Es gibt keine kontrollierte Messung,
die den Deckelzustand als Ursache festlegt.

## Getrennt zu behandelnde Befunde

1. **Cluster-Dimension 0 vor dem ersten Tick:** Bevy 0.19.1 beendete die Anwendung
   mit einem GPU-Validierungsfehler. Die 3D-Testanwendungen aktivieren ihre Kamera
   nun im ersten expliziten Tick. Dieser Fehler besitzt eine klare Logdiagnose
   und darf nicht mit erfolgreichem Capture schwarzer PNGs gleichgesetzt werden.
2. **Fest angenommene Bildgröße:** Der Mesh-Test erwartete 640×360, erhielt aber
   tatsächlich 320×180. Er liest nun Kamera-Zielgröße und Skalierung.
   Das ist eine Testkorrektur, kein Behebungsnachweis für RGB0.
3. **Vollständig schwarze PNGs:** Gegenstand dieser Untersuchung. Kein Fix
   nachgewiesen; grüne Wiederholungen schließen den Fehler nicht.
