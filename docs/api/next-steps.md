# Weiterarbeit an woodpecker

## Ziel und aktueller Stand

Der Agent soll ein Spiel **headless über Bilder und virtuelle Eingaben steuern**.
Maßgeblich sind [target.md](target.md#headless-betrieb) und
[ADR-0025](../adr/0025-headless-agent-interaction.md).
Die [Interface-Skizze](goal.rs) markiert noch offene Headless-Interfaces;
der [Implementierungsplan](implementation-plan.md#headless-durchstich) beschreibt
die Baufolge. [slice.md](slice.md) beschreibt weiterhin den ausführbaren Stand.

Vorhanden sind Session-/Prozessverwaltung, Warps, Inspect, virtuelle Eingaben,
Fenster-Capture, Recording/Replay, Fehlerbeobachtung und Reports sowie
Server/Client/CLI/REPL/Script. Diese Module nicht neu bauen.
Die gerenderten Szenen benötigen bisher ein Fenster. Fenstervorschau,
Headless-Adapter und relative Blicksteuerung sind nicht implementiert.

Die Fensterdiagnose ist abgeschlossen und auf
[wesentliche Rendering-Befunde](diagnostics/rendering-findings.md) reduziert.
Die ausführlichen eingecheckten Werkzeuge bleiben über Git verfügbar.
Unveröffentlichte Linux-Folgearbeit wird nicht übernommen. Keine weiteren
Verdeckungsmatrizen oder Treiberdiagnosen als Voraussetzung für Headless starten.

## Nächster Schritt: kleiner Headless-Durchstich

1. Arbeitsbaum und geltende Projektanweisungen prüfen; bestehende Änderungen
   erhalten. Kein Bevy-Upgrade, Fork oder erneutes Anbinden der vorhandenen Szenen.
2. Gegen den gelockten Bevy-Stand die Integration von Bildziel, Runner,
   Kameras und Readback entwerfen. Das offizielle Headless-Beispiel ist eine
   Referenz, kein übernehmbarer Session-Runner.
3. Offene Verträge konkretisieren: Bildzielgröße/-skalierung und Client-Metadaten,
   Renderbereitschaft/Fristen, relative Blickbewegung und Migration der
   fensterbezogenen Fehlerformen. Produktkonflikte vorlegen.
4. Eine bekannte Szene ohne natives Fenster aufnehmen. Zustand und virtuelle
   Zeit bleiben während Capture unverändert. Danach dieselbe Szene über
   explizite Warps bewegen, umsehen und Welt-/UI-Elemente anklicken.
5. Erst nach diesem Nachweis in die vorhandenen Session-Adapter integrieren.
   Sichere Dateiablage, Request-Korrelation, Fristen und Stop weiterverwenden.
   Den heutigen Surface-Guard nicht entfernen, solange sein Fensterpfad existiert.

Abnahme über CLI → Session → Bevy, nicht nur einen alleinstehenden Bildexport:

- Kein natives Fenster; auf Linux keine Displayserver-Verbindung nötig.
- Tatsächlicher Bildinhalt, Maße, UI-Overlay und Pointer-Koordinaten stimmen.
- Eingaben wirken erst beim Warp, gehaltene Tasten bleiben sessionlokal.
- Blickbewegung hängt nicht an Cursorgrenzen oder Betriebssystemfokus.
- Rendering und Readback sind ohne versteckte Ticks bedienbar und begrenzt.
- Schwarze Szenen bleiben gültig; unfertige oder unkopierte Buffer werden nicht
  allein wegen eines erfolgreichen Mappings als fertige Aufnahme ausgegeben.
- Anschließend zwei Sessions, Recording/Replay und geregelten Shutdown prüfen.

## Abdeckungsmatrix für Block 6

Die bisherigen Nachweise gelten für den vorhandenen Ausführungsweg, nicht als
Headless-Abnahme. Build- und Startbefehle: [Bevy-Testapps](../../bevy_test_apps/README.md).

| Bereich | Vorhandener Nachweis | Nächste relevante Lücke |
| --- | --- | --- |
| Session/Ticks | [tests/slice.py](../../tests/slice.py), [Prozesstests](../../tests/session.rs): Stillstand, Pace/Stop, Isolation, Recording/Replay | Headless-Runner ohne zusätzliche Simulationsschritte |
| UI | [tests/ui.py](../../tests/ui.py): Pointer, Keyboard, Text, Layout, PNG, Recording/Replay, zwei Sessions | Layout, Textfokus und Picking ohne Fenster |
| Logik | [tests/logical_state.py](../../tests/logical_state.py): Update, FixedUpdate, Timer, gehaltene Eingaben | Headless-Integration ohne veränderte Ticksemantik |
| Menüs/Drag | [tests/game_menu.py](../../tests/game_menu.py), [tests/ui_drag_drop.py](../../tests/ui_drag_drop.py): Navigation, Handles, Drag-Phasen und Layout | Gleiche Abläufe am Bildziel |
| 3D | [tests/mesh_picking.py](../../tests/mesh_picking.py), [tests/blend_modes.py](../../tests/blend_modes.py): Picking, Transformationen, Materialien und Bilder | Fensterlose Kameraziele sowie relative Blicksteuerung |
| Inspect | [Reflection-Matrix](implementation-plan.md#reflection-matrix), [Entity-Tests](../../src/session/inspect/entities/tests.rs) | Bildziel-Metadaten ohne neue parallele Inspect-Architektur |
| Reports/Lifecycle | [tests/observation.py](../../tests/observation.py), [Server-Tests](../../src/server/report_tests.rs) | Capture, Failure, Client-Trennung und Shutdown kombiniert |
| Client/Agent | [REPL](../../tests/repl.py), [Script](../../tests/script.py) | Reale Headless-Agentenschleife; große Outputs und langsame Clients |

## Danach: Systemabnahme vervollständigen

- Aktive Aufnahme, Failure, lokalen Report, Client-Trennung, Replay und
  Verwaltungs-Stopp kombinieren. Dateiinhalte und unveränderliche Snapshots prüfen.
- Activity-Lücken und gemeinsame Serverfrist prüfen; unbekannte Outcomes nicht
  automatisch erneut einreichen.
- Die vorläufige 4-MiB-Activity-Grenze mit echten großen Outputs bemessen.
  Die wartende Report-Queue je Session ist bisher unbeschränkt.
- Erst nach der Integration verbleibende Szenenvarianten, API und
  Dokumentation gegen den Zielvertrag abgleichen.

## Nachweise und bekannte Grenzen

Nach dieser Bereinigung bestanden erneut 136 Root-Unit-/CLI-, 7 Beobachtungs-
und 13 Sessiontests sowie Format- und Diff-Prüfung.
Log: `target/headless-plan-tests.MebS4c`. Root-Clippy war beim vorherigen
Push-Check grün; Produktionscode und Dependencies wurden hier nicht verändert.
Das ist kein Headless- oder neuer Grafiknachweis.

Die macOS-Fenstertests bestanden sichtbar, teilweise verdeckt und nach
Wiederherstellung. Vollverdeckung führte zur Guard-Ablehnung, nicht zu einer
erfolgreichen Bildabnahme. Details und historische Commit-Referenzen stehen in
den [Rendering-Befunden](diagnostics/rendering-findings.md).

Zu erhalten beziehungsweise ausdrücklich zu prüfen:

- Die 3D-Fixtures aktivieren ihre Kamera im ersten expliziten Tick zur
  Clusterinitialisierung. Kein versteckter Starttick als Ersatz.
- Einzelne macOS-Prozessfixtures hatten Ready-Verzögerungen/SIGKILL; ein langsames
  `gh`-Fixture hatte ungeklärte Timeouts. Spätere Erfolge belegen keine Behebung.
- Fehlende Glyphen für Umlaute, Emoji und Japanisch bleiben zurückgestellt;
  gespeicherte Unicode-Werte stimmen.
- Synchrones REPL-stdout kann blockieren; lokales Datei-I/O ist nur kooperativ
  abbrechbar. Der lokale Report-Provider benötigt Hardlinks.
- Ein unbestätigter GitHub-POST kann trotzdem veröffentlicht worden sein.
  Kein automatisches Wiederholen. Ein separater `panic=abort`-Build ist ungeprüft.
- Prozessverwaltung unterstützt derzeit Unix.

Vorläufige Clippy-Ausnahmen gelten nur beim gezielten CLI-Aufruf:
`game_menu`: `type_complexity`, `too_many_arguments`;
`mesh_picking`: `type_complexity`; `blend_modes`: `too_many_arguments`.
Keine `allow`-Attribute oder Cargo-Features ergänzen. Nach Bevy-Updates ohne
Ausnahmen neu prüfen; Root-Clippy bleibt ohne Ausnahmen.

## Arbeitsregeln

- Keine Commits, Pushes oder Subagenten ohne ausdrücklichen Auftrag.
- Fachliche Modulhierarchien und vorhandene Quellen der Wahrheit verwenden.
- Lokale Reports oder isolierte `gh`-Fixtures verwenden, keine echten
  Veröffentlichungen durch Tests. URL und Report-Signaturbezeichner unverändert lassen.
- Lokale Clients bleiben vertrauenswürdig; keine neue Auth-Schicht oder
  Remote-Unterstützung hinzufügen.
- Kein Architekturentscheid wird durch Löschen historischer Diagnosewerkzeuge
  als implementiert oder getestet ausgegeben.
