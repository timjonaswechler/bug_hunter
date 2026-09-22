Untersuche den bekannten Bevy-Screenshotfehler auf diesem Linux-Rechner.
Ziel ist zunächst ein kontrollierter Plattformvergleich mit Bevy 0.19.1.
Keinen Fix, keine Architekturänderung und keine Projektmigration vornehmen.

Arbeitsstand
- Branch: code_ownership
- Ausgangscommit: 7378230
- Prüfe Branch, Commit und Arbeitsbaum. Vorhandene Änderungen erhalten.
- Keine Commits, Pushes, Resets, git clean oder Veröffentlichungen.

Lies zuerst:
- docs/api/next-steps.md
- docs/api/diagnostics/screenshot-bug-explained.md
- docs/api/diagnostics/capture-causality-criteria.md
- docs/api/diagnostics/capture-causality-live.md
- docs/api/diagnostics/capture-causality-rc.md
- docs/api/diagnostics/capture-scene-visibility.md

Bekannter Befund
Auf macOS/Metal wurde der Fehler mit Bevy 0.19.1 und wgpu 29.0.4
in Blend, Mesh und UI nachgewiesen. Ein eigenständiges Bevy-Repro
bestätigte ihn außerdem mit 0.20.0-rc.1 und wgpu 30.0.1.

Vollständige Verdeckung führte zu:
Surface-Akquise meldet Occluded -> Swapchain-View fehlt ->
Screenshot-Kopie übersprungen -> vorbereiteter Buffer wird trotzdem
erfolgreich gemappt -> rohe Bilddaten vollständig null.

Sichtbare, teilweise verdeckte und wieder sichtbare Aufnahmen waren
korrekt. Zwischen den Captures der Diagnose gab es keine zusätzlichen
Simulationsticks. Der normale woodpecker-Guard lehnt den fraglichen
Capture ab, statt ein schwarzes PNG als Erfolg auszugeben.

Nicht voraussetzen, dass Linux bei Verdeckung ebenfalls Occluded meldet.

1. Umgebung erfassen
Dokumentiere:
- Distribution und Kernel;
- Desktop/Compositor, Wayland oder X11;
- GPU und Treiber;
- tatsächlich gewählten wgpu-Adapter und Grafik-Backend;
- aufgelöste Bevy-/wgpu-Versionen.

Keine Systempakete, Treiber oder Desktop-Einstellungen ohne meine
Zustimmung ändern. Falls etwas fehlt, die benötigten Pakete und den
Grund nennen. Normale Downloads öffentlicher Cargo-Abhängigkeiten
sind erlaubt; deren Quellcode im Registry-Cache nicht verändern.

2. Bestehende Werkzeuge prüfen
Die capture_causality_* und capture_scene_visibility* Dateien unter
tests/diagnostics/ enthalten die bisherige Instrumentierung.
AppKit, Swift und die native Fenstersteuerung sind macOS-spezifisch
und dürfen unter Linux nicht einfach unverändert verwendet werden.

Übernimm die korrigierte Diagnoseprotokollierung:
- getrennte Logdateien je Ereignisquelle und Prozess;
- serialisierte Schreibzugriffe innerhalb jeder Quelle;
- vollständige JSON-Zeilen einschließlich Newline;
- sichtbare Schreibfehler;
- ID-basierte Korrelation statt zeitlicher Nähe oder Dateireihenfolge.

Die vorhandenen headless Regressionstests sind die Ausgangsbasis.
Lokale target/-Artefakte der Mac-Messungen sind nicht im Repository.

3. Einen begrenzten Linux-Versuch vorbereiten
Beginne mit einer bekannten nichtschwarzen Szene, bevorzugt dem
vorhandenen blend_modes-Testfall, unter der gelockten Bevy-Version 0.19.1.
Noch keine vollständige Szenenmatrix und keinen RC-Vergleich starten.

Verwende ein separates temporäres Testprojekt beziehungsweise
instrumentierte Kopien unter target/. Produktionscode, normale
Root-Manifeste, Root-Lockfiles und Registry-Quellen unverändert lassen.
Neue Linux-Diagnosewerkzeuge unter tests/diagnostics/ anlegen.
Bestehende macOS-Werkzeuge nicht überschreiben.

4. Sichtbarkeit kontrollieren
Im selben Prozess diese feste Folge prüfen:
sichtbar ohne Fokus -> teilweise verdeckt -> vollständig verdeckt
-> wieder sichtbar.

Nur eigene Testfenster steuern. Verdeckung durch Geometrie und
verfügbare native Zustandsdaten nachweisen. Teilverdeckung nicht
allein aus einem Occlusion-Flag ableiten.
Hidden und minimiert sind andere Zustände und kein Ersatz für
tatsächliche Verdeckung.

Unter Wayland können Positionierung und Zustandsabfragen beschränkt
sein. Falls die Automatisierung daran scheitert, keine Zustände
erfinden und keine Sicherheitsbeschränkungen umgehen. Stattdessen
einen konkreten manuellen Ablauf mit überprüfbaren Messpunkten
vorschlagen. Einen Wechsel zu X11 oder einem anderen Backend vorher
mit mir abstimmen.

5. Kausalkette messen
Prozess, Session/Request soweit vorhanden, Renderframe, Capture,
Textur und Buffer eindeutig zuordnen:

Surface-Akquise -> Vorbereitung -> Copy oder Skip -> Queue-Submit
-> Map-Abschluss -> rohe Bilddaten -> Adapterentscheidung/PNG.

Ein Log vor copy_texture_to_buffer allein ist kein GPU-Nachweis.
Die Zuordnung muss bis zum gemappten Buffer und seinen Bytes reichen.

Den normalen woodpecker-Guard aktiv lassen. Rohe Bevy-Readbacks bei
Bedarf davor getrennt erfassen, ohne sie als normalen erfolgreichen
Capture auszugeben.

Größen, Viewports und Simulationszustand erfassen. Nach den nötigen
expliziten Initialisierungsticks keine weiteren Simulationsticks
zwischen den Captures ausführen. Bekannten Bildinhalt und Rückkehr
zum Ausgangsbild prüfen, nicht nur Dateiexistenz oder nonzero.

6. Vor dem Grafiktest anhalten
Zuerst Codeprüfung, Build und headless Tests abschließen.
Danach BUILD_READY melden mit:
- Umgebung und Versionen;
- genauem Startbefehl;
- geplanter fester Messfolge;
- eventuell benötigten manuellen Schritten.

Auf meine ausdrückliche GUI_FREIGABE warten.
Vorher keine grafischen Tests starten.

Nach Freigabe jeden geplanten Fall einmal und nacheinander ausführen.
Keine automatischen Wiederholungen bis zum Erfolg und keine
abgeschwächten Prüfungen. Bei unerwartetem Infrastrukturfehler,
fehlender Zustandskontrolle oder Cleanupfehler anhalten.

7. Ergebnis dokumentieren
Schreibe:
docs/api/diagnostics/capture-causality-linux.md

Enthalten sein müssen Umgebung, Versionen, Befehle, Artefaktpfade,
tatsächliche Fensterzustände, Acquire-Ergebnisse, Copy-/Readback-Kette,
Bildvergleich und Grenzen.

Unterscheide:
- derselbe Skip-Copy-/Nullbuffer-Fehler reproduziert;
- Verdeckung führt auf diesem System zu keinem Surface-Ausfall;
- anderer Fehlerpfad beobachtet;
- Versuch mangels Zustandskontrolle nicht entscheidbar.

Ein korrektes Bild trotz Verdeckung ist ein gültiges Ergebnis und
widerspricht dem macOS-Befund nicht. Aus diesem Rechner keine Aussage
über alle Linux-Systeme ableiten.

Alle eigenen Testprozesse und Deckfenster anschließend beenden,
Cleanup prüfen und GUI_SLOT_RELEASED melden.
Erst nach unserer Auswertung weitere Szenen oder Bevy 0.20 testen.
