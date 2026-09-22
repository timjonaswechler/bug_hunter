# Weiterarbeit an woodpecker

Diese Übergabe enthält den aktuellen Arbeitsstand, offene Aufgaben und die
maßgeblichen Nachweise. Verhaltensregeln stehen in [target.md](target.md),
Interfaces in [goal.rs](goal.rs), Migrationsstatus im
[Implementierungsplan](implementation-plan.md). Bedienung und Build-Befehle:
[slice.md](slice.md), [Bevy-Testanwendungen](../../bevy_test_apps/README.md).

## Einstieg und Arbeitsstand

1. Branch und Arbeitsbaum mit `git status --short` prüfen. Die Arbeit liegt auf
   `code_ownership`; vorhandene Änderungen einschließlich untracked Dateien erhalten.
   Insbesondere Mesh-/Blend-Abnahmen, Diagnoseberichte und Screenshot-Absicherung
   gehören zum bisherigen Stand und dürfen nicht verloren gehen.
2. Den nächsten Schritt unten und die verlinkte Diagnosematrix lesen.
   Die vorhandenen Szenen nicht erneut anbinden.

Umgesetzt sind Session-/Prozessverwaltung, explizite Warps, allgemeines Inspect
einschließlich Reflection-Matrix, virtuelle Eingaben, Screenshot-Grundfunktion,
Recording/Replay, Fehlerbeobachtung und lokale/GitHub-Reports. Server, Client,
CLI, REPL und Script-Ausführung nutzen denselben Command-/Activity-Weg.
Die externe Agent-Abnahme mit pi ist [dokumentiert](pi-acceptance.md).
Alle sieben Bevy-Szenen besitzen CLI-Abnahmen; die jüngsten strikten Bildläufe
sind wegen nicht verfügbarer Fensteroberflächen noch nicht grün.

Der v2-Ausführungsweg ist entfernt. Kein `automation`-Marker, keine alten
Controller-Exports und kein `failure.json` ergänzen. Modellzugang und
Agent-Werkzeugschleife bleiben außerhalb von woodpecker.

## Nächster Schritt: Capture unabhängig von der Fenstersichtbarkeit

**Ziel:** Das primäre Spielfenster zuverlässig aufnehmen, auch wenn es keinen
nativen Fokus hat oder vollständig von anderen Fenstern verdeckt wird.
Die Aufnahme soll nicht davon abhängen, ob das Betriebssystem das Fenster gerade
auf dem Bildschirm präsentiert.

Fehlender Fokus und Verdeckung sind verschieden: Ein nicht fokussiertes Fenster
kann sichtbar sein und normal rendern. Bei vollständiger Verdeckung kann macOS
es als *occluded* melden; Metal liefert dann möglicherweise keine Surface-Textur.
Ein aktiver externer Bildschirm schließt diesen Zustand des Testfensters nicht aus.

### Ausgangspunkt

Die [Diagnosematrix](diagnostics/black-screenshots.md) bündelt drei Worker-Berichte,
Messreihen und verbleibende Unsicherheiten. Der belegte Bevy-0.19.1-Fehlerpfad:
fehlender Swapchain-View → Bildkopie übersprungen → vorbereiteter Buffer trotzdem
ausgelesen → gültiges RGB0-PNG mit falscher Erfolgsmeldung. Instrumentierte
Live-Läufe stützen diesen Mechanismus; nicht jeder historische Ausfall ist damit
einzeln bewiesen. Einfaches Warten half in 24/24 schwarzen Captures nicht.

Die [framegebundene Absicherung](../../src/session/screenshot/capture/surface.rs)
verhindert inzwischen diesen falschen Erfolg. Fehlen View oder Format im
Capture-Frame, folgt `screenshot_window_unavailable` ohne Datei. Späte Readbacks
werden verworfen; tatsächlich schwarze Bilder bleiben gültig.
Das ist noch keine Aufnahmefähigkeit bei verdecktem Fenster.

Der kontrollierte [Capture-Kausalitätslauf](diagnostics/capture-causality-live.md)
belegt den Mechanismus nun in einem sichtbaren/verdeckten/wieder sichtbaren
A/B/A-Versuch: native Verdeckung führte im selben Capture-Frame zu `occluded`,
fehlendem View, übersprungener Copy und einem erfolgreich gemappten RGB0-Buffer.
Nach Entfernung des eigenen Occluders kehrten Copy und das bytegleiche Bild ohne
weiteren Simulationstick zurück. Der Guard lehnte den verdeckten Capture ohne
Datei ab. Dieser Befund entscheidet noch nicht zwischen den beiden unten
beschriebenen Capture-Architekturen.

Eine [einfache Fehlererklärung](diagnostics/screenshot-bug-explained.md)
führt durch die vier beteiligten Bevy-Funktionen und trennt Befund, Schutz
und noch fehlende Aufnahmefähigkeit.

### Vorgehen und Abschlusskriterien

**Upstream-Prüfung abgeschlossen:** Der
[Recherchebericht](diagnostics/bevy-upstream-screenshot-status.md) dokumentiert
den Stand vom 2026-09-21 mit festen Commit-Links. `v0.20.0-rc.1` ist der einzige
gefundene 0.20-Prerelease. In diesem RC, `release-0.20.0` und dem abgefragten
`main` besteht derselbe Fehlerpfad im Quellcode fort. Die Suche in offenen und
geschlossenen Issues und PRs fand verwandte Fälle, aber keinen exakten Fix.
Das ist kein neuer Laufzeitnachweis gegen 0.20 und keine Garantie vollständiger
Issue-Erfassung.

**Live-Prüfungen auf macOS abgeschlossen:** Das
[eigenständige RC-Repro](diagnostics/capture-causality-rc.md) bestätigt den Fehler
mit `v0.20.0-rc.1` und wgpu 30.0.1 auf Metal, ohne woodpecker-Adapter.
Die [Szenenmatrix](diagnostics/capture-scene-visibility.md) bestätigt ihn unter
Bevy 0.19.1 und wgpu 29.0.4 für Blend, Mesh und UI. Sichtbar, teilweise verdeckt
und nach Wiederherstellung bestanden die vollständigen Szenentests, bei UI
einschließlich Recording, Replay und Sessionisolation. Vollständig verdeckt
scheitern die Bildabnahmen am Guard; die getrennten Diagnosen belegen den
übersprungenen Copy und Nullbuffer. Frühere Werkzeugfehler bleiben dokumentiert.

**Nächster Schritt: Linux-Vergleich**, zuerst mit Bevy 0.19.1 und einer bekannten
nichtschwarzen Szene. Distribution, Kernel, Wayland/X11, Compositor, GPU, Treiber,
wgpu-Adapter und tatsächliches Backend erfassen. Die vorhandenen AppKit-/Swift-
Werkzeuge sind macOS-spezifisch und müssen für Linux ersetzt werden. Die
Mehrschreiberkorrektur mit Logdateien pro Quelle und PID sowie ID-Korrelation
beibehalten. Vor grafischen Tests `BUILD_READY` melden und auf `GUI_FREIGABE`
warten; keine Tests parallel ausführen.

Sichtbar ohne Fokus, teilweise verdeckt, vollständig verdeckt und wieder
sichtbar im selben Prozess messen. Keine zusätzlichen Simulationsticks zwischen
den Captures. Unter Linux nicht `Occluded` voraussetzen: Liefert die Akquise
trotz Verdeckung eine Textur und korrekte Bilder, widerspricht das dem Mac-Befund
nicht. Fehlende native Zustandskontrolle ausdrücklich als Grenze melden.
Zunächst diesen Plattformbefund auswerten, dann über weitere Szenen und den
RC-Vergleich entscheiden. Bericht unter `diagnostics/capture-causality-linux.md`
anlegen. Lokale `target/`-Artefakte und Builds sind nicht im Git-Transfer enthalten.

Keine Projektmigration, Bevy-Korrektur oder Capture-Architekturänderung
vorwegnehmen; kein Upstream-Issue oder PR ohne Freigabe veröffentlichen.
Die bestehende Absicherung bleibt aktiv.

1. Diagnosematrix sowie [aktuellen Screenshot-Ablauf](slice.md#screenshot) lesen.
   Den Schutz im [Adapter](../../src/session/screenshot/capture.rs) erhalten,
   bis ein Ersatz den unbefüllten Readback ausschließt.
2. Zwei Ansätze vergleichen: Bevys Kopierpfad von der Fensterpräsentation trennen
   oder Capture über ein Surface-unabhängiges Renderziel führen.
   Auswirkungen auf Kamera, Auflösung, UI-Overlays und Dependency-Pflege klären.
   Echte Architektur-/Produktkonflikte vor der Umsetzung zur Entscheidung vorlegen.
3. Eine Regression mit tatsächlich verdecktem beziehungsweise unsichtbarem
   Testfenster aufbauen. Sie muss ein korrektes Bild nachweisen, nicht nur eine
   erwartete Ablehnung. Zusätzlich sichtbares, nicht fokussiertes Fenster und
   Wechsel der Sichtbarkeit prüfen.
4. Simulation und Eingaben bleiben ausschließlich an explizite Warps gebunden.
   Keine Fokusänderung, zusätzlichen Startticks, Schlafzeiten als Fix oder
   Wiederholungen bis zum ersten grünen Ergebnis. Schwarze Pixel sind keine
   Fehlerheuristik; eine schwarze Szene muss aufnehmbar bleiben.
   `Ready` nicht auf Verdacht ändern.
5. Tatsächliche Rendergröße und Skalierung verwenden. Die separate
   [Resize-/Camera-Aktualitätslücke](diagnostics/window-schedules.md) berücksichtigen,
   aber nicht mit der Ursache jedes schwarzen Bildes gleichsetzen.
6. Abschließend `tests/blend_modes.py`, `tests/mesh_picking.py` und `tests/ui.py`
   einschließlich Recording/Replay erneut ausführen. Erfolg verlangt korrekte
   Bilder ohne zusätzliche Simulationsticks, unveränderte Datei-/Pfadsicherheit
   und weiterhin klare Fehler bei wirklich nicht unterstützten Renderzielen.
   Matrix und Nachweise aktualisieren.

Die begrenzte Diagnosemessung ist ein ergänzendes Werkzeug, keine Bildabnahme:

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin blend_modes
python3 tests/diagnostics/cold_start_frozen_captures.py \
  --sessions 2 --captures 3 --verify-surface-guard
```

Dieser Prüfmodus akzeptiert korrekte Szenenbilder **oder** explizite
Surface-Ablehnungen ohne Datei. Er prüft den vorhandenen Schutz, nicht das neue
Ziel einer erfolgreichen Aufnahme verdeckter Fenster.
Die PNG-/Quaternion-Helfer werden von `tests/mesh_picking.py` und
`tests/blend_modes.py` gemeinsam genutzt.

### Quellenvergleich und offene Architekturentscheidung

Der lokale Quellstand von `bevy_render 0.19.1` wurde erneut geprüft.
Die Trennung von Capture und Präsentation ist möglich, aber nicht als einfacher
Austausch einer öffentlichen Bevy-Kopierfunktion:

- `view/window/screenshot.rs`: `prepare_screenshots` rendert schon heute in eine
  eigene Textur und ersetzt dafür den Eintrag in `ViewTargetAttachments`.
  Fensterkameras einschließlich einer separaten UI-Kamera können deshalb ihre
  bisherigen Ziele behalten. Die Texturgröße kommt aus der Surface-Konfiguration.
- `submit_screenshot_commands` überspringt bei fehlendem Fenster-View sowohl
  Readback-Kopie als auch Präsentations-Blit. `render_screenshot` führt diese
  beiden Aufgaben gemeinsam aus. Die vorbereitete Textur, der Buffer und ihre
  Resource-Typen sind privat; die Submit-Funktion ist nur `pub(crate)`.
  Ein woodpecker-System kann den vorhandenen Buffer daher nicht über ein
  öffentliches Interface nachträglich befüllen.
- `view/mod.rs`: `ViewTargetAttachments` ist dagegen ausdrücklich als öffentlicher
  Erweiterungspunkt für alternative Ausgabe-Texturen dokumentiert. Ein eigenes
  Capture muss also nicht die `Camera.target`-Werte der Anwendung umschreiben.

| Ansatz | Vorteil | Preis und Grenze |
| --- | --- | --- |
| Bevys Kopie vor die Prüfung des Präsentationsziels ziehen | Fehler an seiner Ursache beheben; bestehende Kamera-, UI- und Readback-Verarbeitung behalten. | Bis zu einer veröffentlichten Korrektur wäre ein gepflegter Bevy-Patch nötig. Cargo-Patches im Manifest einer Bibliothek gelten nicht automatisch für deren Nutzer; auch das separate Test-App-Paket und externe Anwendungen müssten den Patch übernehmen. |
| Eigene Capture-Textur über `ViewTargetAttachments`, eigener Readback | Keine gepatchte Engine beim Nutzer; Kameraziele bleiben gleich. Capture kann unabhängig von einer erworbenen Surface-Textur erfolgen. | woodpecker übernimmt GPU-Kopie, Buffer-Ausrichtung, asynchrones Mapping und Fehlerbehandlung. Für weiterhin korrekte sichtbare Fenster muss das Bild zusätzlich in die vorhandene Surface geblittet werden. |
| Ersatz-View in `ExtractedWindow` nur im Capture-Frame | Könnte den bestehenden Bevy-Kopierpfad ohne Fork freischalten. Das Feld erlaubt laut Dokumentation alternative Texturen. | Zusätzliche Wegwerf-Textur als Blit-Ziel statt einer wirklichen Trennung. Größen-/Formatgleichheit zum privaten Screenshot-Zustand und das Zurücksetzen im nächsten Frame müssten abgesichert werden. Nicht als getestete Lösung behandeln. |

**Bisherige, bis zur Upstream-Prüfung zurückgestellte Empfehlung:**
eigener Capture-Adapter über `ViewTargetAttachments`, sofern wir
den GPU-Readback dauerhaft in woodpecker besitzen wollen. Der bestehende
Screenshot-Command, die Queue und der sichere PNG-Dateischreiber bleiben das
Interface; keine zweite öffentliche Capture-Betriebsart und kein Umschreiben
der Szenenkameras. Gegenüber einem Bevy-Patch entsteht mehr eigener Rendercode,
dafür keine zusätzliche Installationspflicht bei jeder eingebundenen Anwendung.

Diese Wartungsentscheidung ist noch nicht getroffen. Vor ihrer Umsetzung den
Nutzer zwischen eigenem Capture-Adapter und gepflegtem Bevy-Patch entscheiden
lassen. Der bisherige Schutz bleibt unverändert.

Danach zuerst die vereinbarte CLI-Seam rot prüfen: unsichtbares Testfenster,
expliziter Warp, `screenshot.capture`, tatsächliche Szenenpixel und unveränderter
Simulationszustand. Der Test muss Ablehnungen als Fehler behandeln. Anschließend
sichtbar ohne Fokus und Sichtbarkeitswechsel einschließlich UI-Overlay prüfen.
Die Quellenprüfung allein ist kein neuer Laufzeit- oder Bildnachweis.

## Danach: Systemabnahme vervollständigen

Block 6 bleibt offen. Einzelne erfolgreiche Szenen schließen ihn nicht ab.

1. **Kombinierter Lebenszyklus:** Auf [tests/observation.py](../../tests/observation.py),
   [tests/slice.py](../../tests/slice.py) und den bestehenden Recording-/Replay-/
   Report-Tests aufbauen. Aktive Aufnahme, beobachteten Fehler, automatischen
   lokalen Report und Client-Trennung verbinden. Tatsächliche JSONL-Datei,
   unveränderlichen Report-Snapshot und Markdown prüfen. Danach Verwaltungs-Stopp
   mit aktiver Aufnahme, Recording-Abschluss und Session-Ende nachweisen.
   Wiedergabe in einer frischen Session und Fehlerreproduktion ergänzen.
2. **Lücken und Shutdown:** Kombinationen mit Activity-Lücken und gemeinsamer
   Serverfrist prüfen. Bekannte Ergebnisse von unbekannten Ausgängen unterscheiden;
   keine unbestätigten Commands automatisch wiederholen.
3. **Last:** Die vorläufige 4-MiB-Activity-Grenze mit echten großen Inspect-Outputs,
   Reports und langsamen Clients bemessen. Übergröße und verlorene Ergebnisse
   ausdrücklich behandeln. Die wartende Report-Queue je Session ist unbeschränkt;
   begrenzte fertige Activity begrenzt diese Queue nicht.
4. **Vertragsabgleich:** Öffentliche API, Capabilities, Fehlerformen, verbleibende
   Szenenvarianten und Dokumentation gegen den vollständigen Zielvertrag prüfen.

## Abdeckungsmatrix für Block 6

CLI-Nachweise laufen über CLI → HTTP/WebSocket → Session → Bevy.
Native Tests ergänzen diesen Weg. Frühere Erfolge sind keine Bestätigung des
aktuellen Arbeitsbaums; die jüngsten Ergebnisse stehen im folgenden Abschnitt.

| Bereich / Szene | Vorhandener Nachweis | Noch offen |
| --- | --- | --- |
| `counter` | [tests/slice.py](../../tests/slice.py): Ticks, Stillstand, Pace/Stop, Session-Isolation, Recording/Replay und Verwaltungs-Stopp. | Kombinierte Lebenszyklus-/Lastfälle. |
| `context_menu` | [tests/ui.py](../../tests/ui.py): virtuelle Pointer/Keyboard/Text, Fokus, Unicode und Grenzen, Menüs, tatsächliches Layout, PNGs, Recording/Replay und Root-Isolation mit zwei Sessions. | Zuverlässige Aufnahme verdeckter Fenster; weitere Layout-/Hierarchievarianten. Glyphen bleiben zurückgestellt. |
| `logical_state` | [tests/logical_state.py](../../tests/logical_state.py): Update, FixedUpdate, Timer, Keyboard-Press/Hold/Release und Stillstand ohne Warp. | Pointer-Observer dieser Szene und eigene Bildabnahme. |
| `game_menu` | [tests/game_menu.py](../../tests/game_menu.py): Splash, Einstellungen, Spiel, Timer-Rückkehr, virtuelle Klicks, Hierarchien und tote Bildschirm-/Button-Handles. | Bilder, Keyboard-Kurzwege und Quit-Button über CLI. |
| `ui_drag_drop` | [tests/ui_drag_drop.py](../../tests/ui_drag_drop.py): gültiger/ungültiger Drop, Zwischenposition, Drag-Phasen, Belegung, Hierarchie, Layout und zurückgesetzte Darstellungskomponenten. | Bilder, Multi-Pointer und Despawn während eines Drags. |
| `mesh_picking` | [tests/mesh_picking.py](../../tests/mesh_picking.py): Hover/Press/Release/Out aller drei Meshes, horizontaler Würfel-Drag, Rotation und 13 PNGs über 24 Ticks. | Zuverlässiger Capture bei Verdeckung; vertikaler Drag sowie Drag auf Kugel/Zylinder. |
| `blend_modes` | [tests/blend_modes.py](../../tests/blend_modes.py): zwei Sessions, gehaltene Tasten, Kameraorbit, Alpha-Grenzen, HDR/Unlit, stabile Materialidentitäten, reproduzierbare Farben und sechs PNGs. | Zuverlässiger Capture bei Verdeckung; vollständige Blend-Gleichungen bei Zwischenalphas und HDR-Bilder oberhalb des LDR-Bereichs. |
| Allgemeines Inspect | [Reflection-Matrix](implementation-plan.md#reflection-matrix), [Entity-Tests](../../src/session/inspect/entities/tests.rs) und Game-Menu-CLI prüfen Werte, Filter, Hierarchien und Handle-Lebensdauer. | Weitere Szenen-Lebenszyklen; keine neue Inspect-Architektur nötig. |
| Recording/Replay/Reports | [tests/session.rs](../../tests/session.rs), [tests/observation.py](../../tests/observation.py) und [Server-Report-Tests](../../src/server/report_tests.rs) prüfen Einzelabläufe und Abschluss nach Session-Ende. | Kombinationen mit aktiver Aufnahme/Wiedergabe, Failure, Activity-Lücke und gemeinsamem Shutdown. |
| CLI/REPL/Script/Agent | [tests/repl.py](../../tests/repl.py), [tests/script.py](../../tests/script.py), [pi-Abnahme](pi-acceptance.md): gemeinsame Steuerung, Barrieren und Client-Ende ohne impliziten Stop. | Große Outputs und anhaltende Activity unter langsamen Clients. |

## Nachweise und bekannte Grenzen

### Jüngster Prüfstand nach Screenshot-Absicherung

- `cargo test --all-features -- --test-threads=1`: 136 Bibliotheks-/CLI-,
  7 Beobachtungs- und 13 Session-Prozesstests bestanden.
  Vollständiges Log: `target/surface-guard-tests.9KTXDU`.
- Ohne Default-Features: 92 Bibliothekstests bestanden. Alle 11 Screenshot-Tests,
  Root-Clippy mit `-D warnings`, Format- und Diff-Prüfung bestanden.
- `target/cold-start-hasx1cq2`: sechs korrekte Bilder in zwei Sessions,
  unveränderter Zustand ohne zusätzliche Ticks.
- `target/cold-start-dcykypfz`: temporär unsichtbares eigenes Testfenster,
  drei `screenshot_window_unavailable`-Ablehnungen ohne Dateien. Teständerung
  zurückgenommen und Originalbinaries neu gebaut.
- Strikte Bildabnahmen **nicht bestanden**: Blend
  `target/blend-modes-fyz9uxet` beim dritten Capture, Mesh
  `target/mesh-picking-1j9wpyb4` und `tests/ui.py` beim ersten.
  Jeweils explizit fehlende Surface, kein falsch erfolgreiches RGB0-PNG.
  Die Bildprüfungen wurden nicht abgeschwächt.

Die Diagnoseartefakte und Grenzen des Befunds stehen in der
[Screenshot-Matrix](diagnostics/black-screenshots.md). Ergebnisordner unter
`target/` sind lokale Nachweise, keine versionierten Fixtures.

Frühere erfolgreiche Szenenläufe, nicht durch die jüngste Root-Suite ersetzt:

| Szene | Ergebnisordner | Umfang |
| --- | --- | --- |
| `logical_state` | `target/logical-state-9q3u1qau` | 8 Updates, 14 FixedUpdates, 3 Timer-Abschlüsse. |
| `game_menu` | `target/game-menu-1go85k9z` | Vollständige Navigation, Einstellungen, Timer und tote Handles. |
| `ui_drag_drop` | `target/ui-drag-drop-uhob_pu5` | 12 Ticks, gültiger und ungültiger Drop. |
| `mesh_picking` | `target/mesh-picking-1osc9q3p`, `target/mesh-picking-ha7_94g1` | Je 24 Ticks und 13 PNGs vor der Adapter-Absicherung. |
| `blend_modes` | `target/blend-modes-tsrdc2m0`, `target/blend-modes-r2sbu_1r` | Je zwei Sessions mit 189/16 Ticks und sechs PNGs vor der Adapter-Absicherung. |

Die acht nativen App-/Kompositionstests bestanden zuletzt beim Blend-Durchstich.
Die Testanwendungen sind ein separates Cargo-Package; ihr Build/Test ist nicht
durch einen Root-Build abgedeckt. Szenen vor CLI-Abnahmen separat mit `slice`
bauen. Details und Befehle stehen im [App-README](../../bevy_test_apps/README.md).

### Bekannte Grenzen und zurückgestellte Arbeit

- Bevy 0.19.1: Die 3D-Fixtures aktivieren ihre Kamera erst beim ersten expliziten
  Tick, damit die Cluster-Dimensionen vor dem Rendering initialisiert sind.
  Diese Startbedingung und die Surface-Prüfung des Adapters nach dem Bevy-Update
  erneut prüfen. Native Eingaben und Zeit bleiben ohne `slice` erhalten.
- macOS-Prozessfixtures wurden sporadisch vor Ready verzögert oder mit SIGKILL
  beendet; teilweise gibt es passende Gatekeeper-Logs. Ein langsames `gh`-Fixture
  erreichte wiederholt seine Warteposition nicht, Ursache ungeklärt.
  Spätere grüne Läufe beweisen keine Behebung. Eindeutige Logs je Testlauf verwenden;
  Sicherheitsregeln nicht ändern. Die `__eh_frame`-Linkerwarnung war nicht fatal.
- Fehlende Glyphen für `ü`, `ß`, Emoji und Japanisch sind reproduziert.
  Gespeicherte Unicode-Werte stimmen; Font-/Fallback-Korrekturen bleiben auf
  Nutzerwunsch zurückgestellt.
- REPL-Ausgabe bleibt synchron; ein nicht lesender Empfänger kann sie blockieren.
  Lokales Datei-I/O ist nur kooperativ, nicht hart durch eine Frist abbrechbar.
  Direkte Report-Übermittlung außerhalb des Servers besitzt keine Serverfrist.
- Ein fehlgeschlagener GitHub-Publish kann trotzdem ein Issue erzeugt haben;
  POST wird nicht automatisch wiederholt. Der lokale Provider benötigt Hardlinks
  für atomische Veröffentlichung ohne Überschreiben.
- Ein separat mit `panic=abort` gebautes Programm ist noch nicht abgenommen.
  Prozessverwaltung unterstützt derzeit Unix; andere Plattformen wären ein eigener Block.

### Vorläufige Clippy-Ausnahmen der Bevy-Szenen

Nur beim gezielten Aufruf ausnehmen, keine `allow`-Attribute oder Cargo-Features:

```sh
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin game_menu \
  -- -D warnings -A clippy::type_complexity -A clippy::too_many_arguments
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin mesh_picking \
  -- -D warnings -A clippy::type_complexity
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin blend_modes \
  -- -D warnings -A clippy::too_many_arguments
```

Die Ausnahmen gelten für das jeweilige Target, nicht nur einzelne Fundstellen.
Nach dem nächsten Bevy-Update zuerst ohne Ausnahmen prüfen und neu bewerten.
Root-Clippy bleibt ohne diese Ausnahmen.

## Arbeitsregeln

- Bestehende Nutzeränderungen erhalten. Commits und Subagenten nur nach
  ausdrücklicher Zustimmung. Reversible Details innerhalb des Auftrags selbst
  entscheiden; echte Produkt-/Architekturkonflikte vorlegen.
- Bei angeforderten Commits: Conventional Commits, atomar, unabhängig baubar und
  testbar, Beschreibung kleingeschrieben, kein Schlusspunkt, Header höchstens
  100 Zeichen. Breaking Changes mit `!` und `BREAKING CHANGE:`.
  Issue-Verweise nur bei direktem Bezug; `Fixes`/`Closes` nur beim tatsächlichen Schließen.
- Fachliche Modulhierarchien statt wiederholter Symbol-/Dateipräfixe verwenden.
- Tests nutzen lokale Reports oder isolierte `gh`-Fixtures, keine echten Issues.
  Ein konfigurierter GitHub-Provider veröffentlicht im Produkt ohne zusätzliche
  Freigabe pro Report; nicht versehentlich für Entwicklungstests auswählen.
- GitHub-URL und versionierte Report-Signaturbezeichner vorerst unverändert lassen;
  der Nutzer koordiniert deren Anpassung mit einem anderen Projekt.
- Lokale Clients gelten bewusst als vertrauenswürdig. Keine neue Schlüssel-
  oder optionale Auth-Schicht ergänzen; Remote-Betrieb ist nicht unterstützt.
