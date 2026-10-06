# Fenstergröße und Kameraaktualisierung bei angehaltener Simulation

## Status: auf Nutzerwunsch zurückgestellt

Der Nutzer benötigt während einer kontrollierten Session weder Resize noch
DPI-/Bildschirmwechsel. Auf seine ausdrückliche Anweisung wurde der experimentelle
Resize-Umbau zurückgenommen. Das ersetzt die vorherige Freigabe der zusätzlichen
Darstellungsaufbereitung. Maßgeblich ist der [aktuelle Screenshot-Umfang](../target.md#screenshot).

**Voraussetzung sind stabile Fenstergröße, Skalierung und Bildschirmzuordnung.**
Korrekte Darstellung nach dynamischen Änderungen ohne Tick ist nicht zugesichert.
Der Punkt ist zurückgestellt, nicht bestanden, und blockiert den aktuellen Abschluss
nicht. Das Plugin führt keine zusätzlichen Kamera-/UI-Schedules dafür aus.

Entfernt wurden der `presentation`-Adapter samt Layout-Proxy, dessen Tests und
Feature-Erweiterungen, der native Resize-Harness samt Fixture/Observer und die im
Versuch geänderte Beschriftungsreihenfolge der Blend-Szene. Die vorherigen stabilen
Bildtests sowie Surface-, Timeout- und Dateisicherheitsabsicherungen bleiben erhalten.

Eine lokale Kopie der entfernten Quellen und des ausführlichen Diagnoseverlaufs
liegt unter `target/resize-rollback-snapshot/`. Sie und die nachfolgenden nativen
Artefakte werden nicht durch Git übertragen. Keine der unten beschriebenen
Versuchsvarianten ist die aktuelle Implementierung.

Nach der Rücknahme bestanden 110 Library-Tests mit Screenshot/UI, 92 ohne Features,
drei ursprüngliche Blend-Tests, Clippy, statische Probe und CLI-/Slice-Builds.
Logs: `target/resize-rollback-validation/`. Es wurde kein neuer Grafiklauf ausgeführt.

## Ausgangsbefund aus Bevy 0.19.1

Winit aktualisiert `WindowResolution` bei Resize/Skalierungswechsel zwischen Warps.
Die Render-App übernimmt die neue Größe. Das Session-Plugin führt die ursprünglichen
Simulations-Schedules hingegen ausschließlich bei expliziten Warps aus. Damit können
Kamerazielinfo, Projektion, Viewport und UI-Layout bis zum nächsten Tick veraltet bleiben.

Beteiligte Quellen der gelockten Version:

- `bevy_winit/src/state.rs`: direkte Fenstergrößen-/Faktoränderungen und Nachrichten.
- `bevy_render/src/view/window/mod.rs`: Fenster-Extraction und Surface-Konfiguration.
- `bevy_render/src/camera.rs`: `camera_system` in PostStartup/PostUpdate.
- `src/session/plugin.rs`: geparkte Simulations-Schedules, weiterhin laufendes Rendering.
- `src/session/input.rs`: native Eingaben verwerfen, Fensterereignisse bis zum Tick erhalten.

Die verbliebene statische Probe `python3 tests/diagnostics/window_schedules.py`
prüft diesen Quellpfad, nicht korrektes Rendering nach Resize. Der Befund erklärt
historische schwarze PNGs nicht rückwirkend. Bei belegter fehlender Surface gilt
weiterhin die [Screenshot-Guard](black-screenshots.md), nicht ein Farbheuristik-Fix.

## Headless-Reproduktion und vorbereiteter nativer Versuch

Historisch wurde die Lücke mit echtem Control-Loop und Bevy-Kamerasystem headless
reproduziert: 800×600 → 1200×600 ließ das Kameraziel und Aspect ohne Tick veraltet;
ein modellierter Faktorwechsel 1 → 2 ließ zusätzlich den Kamera-Viewport unverändert.
Zeit und Tickzähler standen dabei still. Ein expliziter Vergleichs-Warp aktualisierte
die Daten. Die zugehörigen Tests und der interaktive Harness sind inzwischen entfernt.

Die native Probe nutzte die vorhandene Blend-Szene mit einem opt-in resizable-Fenster,
Read-only-Beobachter und manueller Bestätigung. Je Lauf ein Warm-up-Tick, ein Frozen-
Capture nach Nutzer-Resize und ein expliziter Vergleichs-Tick, keine Capture-Retries.

| Lokale Evidenz | Ergebnis |
| --- | --- |
| `target/window-resize-b9ygon8k/` | 1280×720 → 766×435: Kameradaten ohne Tick alt; nach Tick aktualisiert, aber Beschriftungen versetzt. |
| `target/window-resize-5badl99o/` | 1280×720 → 686×720 nach Szenen-Reihenfolgekorrektur: Frozen-Bild gestaucht; nach Tick Proportionen und sichtbare Beschriftungsanker passend. |
| `target/window-resize-l7hg2jk5/` | Erster Lauf mit experimenteller Produktaufbereitung: Baseline ohne Surface abgelehnt, kein PNG/Resize. Ursache nicht isoliert. |
| `target/window-resize-xf0cixf6/` | Nach ausdrücklich genehmigtem weiteren Lauf mit Sichtbarkeitsbestätigung: 1280×720 → 782×720 ohne Tick bereits ungestaucht; nach Vergleichs-Tick verschwanden Hinweistext und gelbe Beschriftungen. |

Ein echter nativer DPI-Wechsel wurde nicht abgenommen. Unterschiedliche
Bildschirmauflösungen allein belegen keinen Faktorwechsel. Der erfolgreiche
Sichtbarkeits-Wartepunkt beweist nicht die Ursache der vorherigen Surface-Ablehnung.

## Freigegebene Produktaufbereitung: headless umgesetzt

**Historischer Versuch, vollständig zurückgenommen.** Die zeitweise freigegebene
Implementierung reagierte auf Fensteränderungen mit privaten, ausdrücklich
zusammengestellten Kamera-/Frustum-/CPU-Sichtbarkeits- und UI-/Glyphen-Schedules.
Zeit, Input und Gameplay blieben in den getesteten Fällen unverändert. CPU-Culling
musste eigens aktualisiert werden, damit neu sichtbare Objekte beim Verbreitern
nicht weiterhin ausgeblendet blieben. Eingebettete `ViewportNode`-Rendertexturen
und spezielle Renderpfade waren nicht vollständig abgedeckt.

Die native Aufnahme belegte die Behebung der Stauchung für den getesteten Resize,
aber auch einen neuen Textfolgefehler. Metadaten allein erkannten diesen nicht.
Die nachträgliche fixturespezifische Pixelprüfung verwarf das gespeicherte
Vergleichsbild: links keine weißen Hinweisglyphen und keine gelben Materiallabels,
während der rechte Statusblock und die unteren Bildzeilen erhalten blieben.

## Textfolgefehler und Kompatibilitätsblocker

Die Folge normaler Tick → privates Resize-Layout → normaler Tick ließ die Breite
eines unveränderten Textes headless von 216 auf 0 Pixel fallen. Bevy übernimmt
`ContentSize.measure` mit `take()` ohne neue Change-Markierung. Zwei getrennte
`ui_layout_system`-Instanzen besitzen unterschiedliche Änderungscursor; die zweite
konnte die bereits konsumierten Daten als ausdrückliches Löschen interpretieren.

Eine gemeinsame Layout-Instanz behob diesen Test. Ihr Proxy erhielt jedoch nur
`UiSystems::Layout`, nicht direkte Anwendungsvorgaben `.before/.after(ui_layout_system)`.
Die unabhängige Prüfung bewertete das als **P1-Kompatibilitätsblocker**. Der Versuch
wurde nicht als fertige Lösung übernommen und nicht erneut grafisch abgenommen.
Mit seiner Rücknahme besteht dieser eingeführte Proxy-Blocker im aktuellen Code
nicht mehr; die ursprüngliche Resize-Lücke wird dagegen bewusst nicht behoben.

Historische Logs: `target/window-resize-validation/`, `target/resize-label-diagnosis/`,
`target/presentation-preparation/`, `target/ui-return-diagnosis/`. Grüne Headless-
Ergebnisse dieser Versuche beweisen weder eine vollständige Bildabnahme noch den
Prüfstand des zurückgesetzten Arbeitsbaums.

## Bei einer späteren Wiederaufnahme

Nur mit erneuter Produktfreigabe: Der Nutzen muss den Eingriff in Kamera-/UI-
Scheduling rechtfertigen. Zunächst kompatible gemeinsame Systemzustände und
bestehende Reihenfolgevorgaben prüfen; anschließend native Bilder einschließlich
des ersten normalen Ticks nach Resize abnehmen. Keine versteckten Ticks, keine
Fokusmanipulation und keine Wiederholung bis zum ersten Erfolg.

Der nächste aktuelle Abschlussblock ist dagegen der
[kombinierte Untersuchungsablauf](../implementation-plan.md#3-kombinierten-untersuchungsablauf-abnehmen).
