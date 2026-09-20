# Weiterarbeit an woodpecker

Übergabe nach dem ersten funktionsfähigen Durchstich, der Umbenennung und dem
v2-Cleanup. Dieses Dokument sammelt offene Arbeit und den nächsten Einstieg.
Es ist keine zweite Spezifikation: Verhaltensregeln stehen in [target.md](target.md),
Interfaces in [goal.rs](goal.rs), Migrationsstatus und Nachweise im
[Implementierungsplan](implementation-plan.md).

## Zu Beginn einer neuen Session

1. Den Branch `code_ownership` mit dem Commit dieser Übergabe verwenden und den
   tatsächlichen Arbeitsbaum mit `git status --short` prüfen. Spätere lokale
   Änderungen einschließlich untracked Dateien erhalten. Der frühere Stand
   `5e9552e` enthält noch nicht den hier beschriebenen Durchstich und Cleanup.
2. [Dokumentindex](README.md), Zielvertrag, Interface-Skizze und
   [Migrationstabelle](implementation-plan.md#migration-und-bereinigung) lesen.
   Die Tabelle trennt Entscheidungen von erledigter Implementation.
3. Für den ausgewählten Durchstich dessen fachliche Regeln und
   [zu portierende Testfälle](implementation-plan.md#fachliche-testfälle-für-die-offenen-durchstiche)
   prüfen. Research und ADRs nur für die jeweilige technische Frage hinzunehmen.
4. Den aktuellen Stand unter „Konkreter nächster Durchstich“ beachten.
   Für Block 6 zuerst die Abdeckungsmatrix und den konkreten Einstieg für
   `mesh_picking` unten lesen.
   Screenshot, Recording und Replay sind umgesetzt. Fehlerbeobachtung und
   Snapshot-Konstruktion liegen in `e655acd` auf `code_ownership`, aufbauend auf
   `82cd34b`. Report-Darstellung, beide Provider, automatische Server-Reports und
   serverseitige Activity-Momentaufnahmen liegen in `8fc09c0`.
   REPL und Script-Ausführung samt Client-Abbruch und Abnahmen liegen in `8b6f5b3`.
   Keine Commits oder Subagenten ohne ausdrückliche Zustimmung.
   Reversible Implementierungsdetails innerhalb des beauftragten Umfangs
   selbstständig entscheiden.

## Bereits umgesetzt

- Paket und Binary heißen `woodpecker`. Aktuelle Rust-Verweise, interne
  Umgebungsnamen, Dokumentation und Testaufrufe sind angepasst.
- Lokale Clients werden ohne Schlüssel-Authentisierung zugelassen. Server und
  Client verwenden weiterhin explizite Loopback-Adressen; die Origin-Prüfung
  bleibt bestehen. Remote-Betrieb ist nicht unterstützt.
- Die direkte Session besitzt Prozessverwaltung, private Pipe-Verarbeitung,
  Request-Korrelation, typisierte Pending-Ergebnisse, History und Events.
- Warp mit Pace-Wechsel und Stop, reflektiertes Resource-Inspect und Shutdown
  funktionieren. Inspect und der normale äußere Event-Loop erzeugen keine
  zusätzlichen Simulationsticks.
- Allgemeines Entity-Inspect besitzt Handle-Abfragen, Componentfilter, Summary,
  Component-Namen/-Werte und Hierarchien. Es verwendet den bestehenden Handle und
  schließt interne Resource-Entities aus. Adapter-, Codec- und Plugin-Tests prüfen
  Type Paths, Ablehnungen, Sortierung und Lesen ohne Tick oder Zeitfortschritt.
- Die Reflection-Matrix gegen Bevy 0.19.1 ist ergänzt. Dieselben Wertfixtures laufen
  durch Resource- und Component-Inspect, einschließlich fehlender Registrierungen,
  opaker Werte, Custom-Serializer-Fehler, Maps, verschachtelter Sets und Asset-Handles.
  Die Zuordnung der Fälle steht im [Implementierungsplan](implementation-plan.md#reflection-matrix).
- Pointer-Bewegung, Buttons und Scroll sind geprüft und bis zum nächsten Tick
  vorgemerkt. Die gerenderte Context-Menu-Abnahme über Server und Session besteht.
  Der Kontrolllauf hält den Render-Zeitkanal frei, ohne Simulationszeit fortzuschreiben.
- Kontrollierte Sessions verwenden jetzt einen eigenen virtuellen Pointer und filtern
  native Maus-/Touch-/Keyboard-/IME-Eingaben. Zwei gerenderte Sessions steuern unabhängig
  voneinander ihre Menüs und Buttonzustände. Das optionale `ui`-Feature bindet auch
  Bevys ältere `Interaction`-Logik an den virtuellen Pointer an.
- Virtuelle Keyboard-Commands besitzen stabile Key-Tokens und halten Tasten bis zum
  ausdrücklichen Release. Sie erzeugen keine Texteingabe und keine Wiederholungs-Events.
  Tests prüfen Vormerkung, Halten über mehrere Ticks, Ablehnungen, Modifikatortasten,
  native Störeingaben und unabhängige Tastaturen in zwei gerenderten Sessions.
- `input.text.input` prüft Fenster, Bevy-Fokus und die 16.384-Byte-UTF-8-Grenze.
  Erst beim Tick erhält die geprüfte `EditableText`-Entity den Edit. Fokuswechsel
  leiten angenommene Eingaben nicht um. Die UI-Abnahme prüft getrennte Unicode-Texte
  in beiden Sessions; fehlender Fokus und Übergröße werden abgelehnt.
- `screenshot.capture` nimmt mit dem optionalen `screenshot`-Feature das primäre
  gerenderte Fenster auf. Readback und PNG-Schreiben laufen ohne zusätzliche Ticks.
  Pfadsandbox, atomisches Überschreiben, parallele Requests und getrennte Roots
  sind geprüft; die reale Context-Menu-Abnahme validiert die PNGs und eingefrorene Zeit.
- Recording-Start/-Stop laufen im Session-Koordinator mit separatem Dateiworker.
  Header-/Footer-Bestätigungen begrenzen den aufgenommenen Abschnitt; Commands und
  Outcomes werden in Annahmereihenfolge geschrieben. Tests prüfen Zustände,
  Pfadsicherheit, Schreibfehler und unerwartetes Prozessende. Die Zähler- und
  UI-Abnahmen prüfen die erzeugten JSONL-Dateien über den tatsächlichen CLI-Weg.
- Replay validiert die vollständige v1-Datei auf einem separaten Worker, bevor
  der erste Spiel-Command ausgeführt wird. Der typisierte Plan berücksichtigt
  effektive Warp-Ticks, Command-Barrieren, Stop und technische Blockierungen.
  Interne Commands teilen IDs, History und Recording mit externen Commands.
  Der Verwaltungs-Stopp beendet Replay vor Recording und Shutdown.
  Zähler- und UI-Roundtrips sind über Server und CLI geprüft.
- Der Server verwaltet mehrere unabhängige Sessions über HTTP und fest gebundene
  WebSocket-Verbindungen. Activity besitzt Cursor und erkennbare Lücken.
  Client-Trennung beendet angenommene Arbeit nicht.
- Fehlerbeobachtung verarbeitet stderr während der laufenden Session. Ein verketteter
  Panic-Hook und ein optionaler Tracing-Layer liefern versionierte Chunk-Marker.
  Failure- und ObservationError-Events bleiben von Command-Outcomes getrennt.
  Der Server übernimmt die Events in Activity. `Report::create` kopiert die
  Startmetadaten und die aktuelle History; spätere Änderungen verändern den Snapshot nicht.
- `Report::create` berechnet außerdem Titel und versionierte v1-Signatur.
  `to_markdown` stellt den vollständigen Snapshot mit sicheren Code-Fences dar.
  Die vier Signatur-Golden-Vectors, Normalisierung, Unicode-Titelgrenzen und
  eingebettete Markdown-/HTML-Syntax sind geprüft. Der private Projektpfad
  dient der Signaturnormalisierung und dem `gh`-Arbeitsverzeichnis, nicht als Kontextfeld.
- `report::submit(&report, &session)` schreibt vollständiges Markdown mit dem
  lokalen Provider. Der beim Start geöffnete Artefakt-Root und die Startkonfiguration
  bleiben maßgeblich, auch nach Session-Ende. No-follow-Pfade, atomische Erstellung
  ohne Überschreiben, Signaturkonflikte, temporäre Dateien und parallele Schreiber
  sind geprüft.
- Bei konfiguriertem `Github` sucht `report::submit` über `gh api --paginate`
  in offenen und geschlossenen Issues und veröffentlicht ohne zusätzliche Rückfrage.
  Titel und vollständiges Markdown gehen über stdin. Fehler führen zum lokalen
  Speicherweg; `Fallback` beziehungsweise `FallbackFailed` behalten die Ursachen.
  Prozessfixtures und eine echte Session mit isoliertem `gh`-Fixture prüfen diesen Weg.
  Dabei wurden keine echten Issues angelegt.
- Der Server erfasst Report-Snapshots sofort beim Failure-Empfang und führt
  Provider-Arbeit auf einem separaten Worker je Session aus. Activity behält Report
  und Ergebnis gemeinsam. Reports können nach Session-Ende abschließen; die gemeinsame
  Serverfrist und erzwungener Stopp erreichen auch diese Worker und ihre `gh`-Prozessgruppen.
  Ein Abbruch meldet ungewissen Ausgang und startet keinen Fallback.
- CLI, headless Zähleranwendung, Einzel-Session-Stopp, gemeinsame Serverfrist,
  SIGTERM und erstes/zweites Ctrl+C sind vorhanden.
- Der v2-Ausführungsweg ist entfernt: alte Host-/Driver-Fassade, Plugin- und
  Transport-APIs, Step/Clock, Automation-Marker, Controller-Beispiele und
  `failure.json`-Architektur. Der Handle samt Lebensdauertests wurde nach
  [handle](../../src/handle.rs) übernommen.
- Die Bevy-Szenen und ihre eigenen fachlichen Tests bleiben als native
  Anwendungen erhalten. `counter`, `context_menu`, `logical_state`, `game_menu` und `ui_drag_drop` sind an den neuen Session-Weg
  angebunden. Die entfernte `automation`-Anbindung ist kein unterstützter Einstieg.
- Die Bibliothek ohne Features benötigt weder Server-/CLI- noch UI-/Renderer-
  Abhängigkeiten. Render-Abhängigkeiten der Testanwendungen gehören zu deren
  separatem Package.

Das ist weiterhin ein experimenteller v3-Teildurchstich, keine vollständige
Implementation des Zielvertrags. Insbesondere fehlen noch vollständige
Szenen-/Lastabnahmen; alte Funktionen werden nicht durch
Kompatibilitäts-Exports angeboten.

## Offene Aufgaben in empfohlener Reihenfolge

### 1. Input und vollständiges Inspect

- [x] Pointer-Commands mit absoluter/relativer Bewegung, Button-/Scroll-Zuständen
  und Fenstergrenzen implementieren, prüfen und bis zum nächsten Tick vormerken.
- [x] Keyboard-Commands samt Bevy-Anbindung implementieren. Key-Tokens, gehaltene
  Tasten, Ablehnungen und Verarbeitung erst beim nächsten Tick sind geprüft.
- [x] Text-Commands samt Bevy-Anbindung implementieren. Fokus und Textgrößen aus
  dem Zielvertrag abdecken. Erfolg bedeutet nur Prüfung und Vormerkung bis zum Tick.
- [x] Entity-Queries, Component-Filter, Namen-/Werteprojektionen und Hierarchien
  ergänzen. Den übernommenen Handle verwenden; keine Automation-Marker einführen.
- [x] Reflection-Fixtures gegen Bevy 0.19.1 vervollständigen: exakte Type Paths,
  fehlende/opaque/nicht serialisierbare Werte, nicht endliche Zahlen, Maps, Sets
  und Asset-Handles.
- [x] Den realen UI-Durchstich unten als Integrationstest ausführen.

Abschluss: Eine echte Bevy-Anwendung lässt sich über neue Input-Commands verändern
und allgemein inspizieren. Die fachlichen Varianten und Ablehnungen sind geprüft;
die Arbeit endet nicht schon beim ersten erfolgreichen Klick.

### 2. Screenshot und gerenderte Abnahme

- [x] Das primäre gerenderte Fenster aufnehmen; erst nach GPU-Readback und
  erfolgreichem PNG-Schreiben antworten.
- [x] Normalisierte Pfade, Symlink-Sicherheit, Root-Isolation und Überschreiben prüfen.
- [x] Nachweisen, dass Aufnahme und Darstellung keine Simulationsticks oder
  simulierte Zeit hinzufügen.

Abschluss: Die realen UI-Szenen liefern überprüfbare Bilder über denselben
Client-Vertrag; fehlende Unterstützung wird korrekt abgelehnt.

### 3. Recording und Replay

- [x] Recording-Start/-Stop, Zustände, Dateibarrieren, Pfadsicherheit und das neue
  JSONL-Format implementieren. Commands und Outcomes in Annahmereihenfolge schreiben.
- [x] Schreibfehler, unvollständige Aufnahme, Session-Ende und Footer-Abschluss prüfen.
- [x] Replay vollständig vor dem ersten Spiel-Command validieren und in der
  laufenden Session ausführen. Effektiv ausgeführte Warp-Ticks berücksichtigen.
- [x] Stop während Laden und Ausführung, technische Blockierungen sowie
  Command- und Shutdown-Barrieren umsetzen.

Abschluss: Roundtrips prüfen Reihenfolge und Format, nicht beliebige Gleichheit
mit früheren Spiel-Outcomes. Stop und Ressourcenabschluss funktionieren auch
während aktiver Aufnahme beziehungsweise Wiedergabe.

### 4. Fehlerbeobachtung und Reports

- [x] Laufende stderr-Diagnose, Panic-Hook, optionales Tracing und Marker-Framing
  ergänzen; Beobachtung von Session-Transport und Command-Outcomes getrennt halten.
- [x] Beim Start erforderliche Metadaten erfassen und über `Report::create`
  einen unveränderlichen Snapshot mit Failure und History konstruieren.
- [x] Titel, Markdown, Fehlerdaten und Signatur einschließlich Golden Vectors umsetzen.
- [x] Den lokalen Provider mit Pfadsandbox, atomischer Erstellung ohne Überschreiben,
  Marker-Vergleich und konkurrierenden Aufrufen implementieren.
- [x] GitHub mit Duplikatsuche und lokalem Rückfall implementieren.
  Bei konfiguriertem GitHub-Provider ohne zusätzliche Rückfrage veröffentlichen;
  `Local` veröffentlicht nichts.
- [x] Clientunabhängige Report-Arbeit in Activity und die gemeinsame Serverfrist
  integrieren. Langsame Provider dürfen Event-Empfang und Pipe-Drain nicht blockieren.

Abschluss: Beobachtete Fehler ergeben Reports ohne `failure.json`-Zwischenformat
und ohne vorherigen Session-Abschluss. Provider-Fehler und ungewisse externe
Ausgänge bleiben sichtbar.

### 5. REPL, Script und Agent-Abnahme

- [x] Die REPL während ausstehender Commands ansprechbar halten; gemeinsame
  Commands, Inspect-Kurzformen, Pending-Anzeige und Activity verwenden.
- [x] Scripts vollständig vor Ausführung parsen und validieren; Ergebniszuordnung
  sowie die festgelegten Recording-/Shutdown-Barrieren umsetzen.
- [x] EOF, Quit und Client-Trennung ohne versteckten Stop oder Shutdown prüfen.
  REPL- und Script-Abnahmen prüfen weiterlaufende angenommene Arbeit. Script liest
  eine vollständig validierte Datei, nicht stdin; Ctrl+C trennt seinen Client.
- [x] Den maschinenlesbaren CLI-Zugang mit einer vom Nutzer gestarteten externen
  Agent-Laufzeit erproben. Modellzugang und Agent-Werkzeugschleife gehören nicht hierher.
  Mit pi durchgeführt; Rohdaten und Recording gegengeprüft. Der Nutzer bestätigt
  erfolgreiche REPL-Inspects und weiterlaufenden Warp nach beendetem pi-Prozess.

Abschluss: Menschliche und maschinelle Bedienung nutzen denselben Ausführungsweg,
auch bei parallelen Clients und Wiederverbindung.

### 6. Vollständige Systemabnahme

- [ ] Die erhaltenen Bevy-Szenen über den neuen Weg automatisieren: Fokus/Text,
  Drag-and-drop, Menüs, Timer, Layout, tote Handles und Bilder.
  Die Abdeckungsmatrix unten trennt vorhandene Nachweise von noch offenen Szenen.
  Die zusätzlichen Durchstiche `logical_state`, `game_menu` und `ui_drag_drop` sind umgesetzt.
  Der Gesamtpunkt bleibt offen, insbesondere für die 3D-Szenen.
- [ ] Recording, Replay und Reports gemeinsam mit Client-Trennung, Activity-Lücken,
  Session-Ende und Server-Shutdown testen.
- [ ] Die vorläufige Activity-Grenze von 4 MiB mit echten großen Inspect-Outputs und
  Reports bemessen. Übergröße und verlorene Ergebnisse ausdrücklich behandeln.
- [ ] Öffentliche API, Capabilities, Fehlerformen und Dokumentation gegen den
  vollständigen Zielvertrag prüfen. Pro abgeschlossenem Bereich Migrationsstatus
  und tatsächlichen Nachweis aktualisieren.

### Abdeckungsmatrix für Block 6

Stand nach Beginn der Systemabnahme. „CLI geprüft“ bedeutet ein echter Prozessweg
über CLI → HTTP/WebSocket → Session → Bevy. Native App-Tests und direkte Plugin-Tests
sind ergänzende Nachweise, aber kein Ersatz dafür. Bestehende Nachweise aus früheren
Sitzungen sind nicht automatisch in dieser Sitzung erneut ausgeführt worden.

| Bereich / Szene | Vorhandener Nachweis | Noch offen |
| --- | --- | --- |
| Grundsteuerung / `counter` | [tests/slice.py](../../tests/slice.py): Tickzahl, eingefrorener Zustand ohne Warp, Pace/Stop, getrennte Sessions, Recording/Replay und Verwaltungs-Stopp. | Kombinierte Fehler-/Lastfälle aus den folgenden Zeilen; der erfolgreiche Zähler allein schließt Block 6 nicht ab. |
| Fokus, Text, gehaltene Tasten / `context_menu` | [tests/ui.py](../../tests/ui.py): zwei echte Fenster, virtuelle Pointer und Tastaturen, Fokuswechsel, Text erst nach Tick, unterschiedliche Unicode-Werte, fehlender Fokus und Textgrößengrenze. | Weitere Szenen mit Fokuswechsel und Hierarchie-Lebensdauer. Fehlende Glyphen bleiben auf Nutzerwunsch zurückgestellt; gespeicherte Textwerte sind geprüft, vollständige Glyphendarstellung nicht. |
| Menüs, Layout und Bilder / `context_menu` | `tests/ui.py` findet benannte Entities per allgemeinem Inspect, liest tatsächliche Layout-Koordinaten, öffnet/schließt Menüs, prüft `Interaction`, PNG-Struktur und Pixeländerungen ohne zusätzliche Ticks. Recording/Replay der UI und Root-Isolation sind enthalten. | Kein Nachweis für sämtliche Layoutvarianten oder andere Szenen. Früher sporadisch schwarze Screenshots; Display-Voraussetzungen weiterhin beachten. |
| Update, FixedUpdate und Timer / `logical_state` | **Neu:** [tests/logical_state.py](../../tests/logical_state.py) prüft den Session-Start, stabile Anfangswerte, reale Wartezeit ohne Fortschritt, 20-ms-Simulationstakte bei 10-ms-FixedUpdate und 40-ms-Timer sowie Keyboard-Press/Hold/Release. | Der Pointer-Observer dieser Szene und ihre Bilddarstellung werden damit nicht abgenommen. Pointer und Bilder besitzen bisher den gesonderten Context-Menu-Nachweis. |
| Bildschirmwechsel, Einstellungen, Timer und tote Handles / `game_menu` | [tests/game_menu.py](../../tests/game_menu.py): Session-Anbindung, Splash → Hauptmenü → Display-/Sound-Einstellungen → Hauptmenü → Spiel → Timer-Rückkehr. Virtuelle Klicks auf ausgelesene Layoutkoordinaten, Hierarchien, persistente Einstellungen und `entity_not_found` für despawnte Bildschirm-/Button-Handles. Zwei native Tests bleiben ergänzend erhalten. | Keine Bildabnahme, keine CLI-Abnahme der Keyboard-Kurzwege oder des Quit-Buttons. |
| UI-Drag-and-drop / `ui_drag_drop` | [tests/ui_drag_drop.py](../../tests/ui_drag_drop.py): echte virtuelle Pointer-Sequenzen für gültigen Drop auf eine andere Tile und ungültiges Ziel auf leerem Hintergrund. Prüft Zwischenposition, aktive Tile, geordnete Drag-Phasen, Belegung, Hierarchie, Layout aller Tiles und Rücksetzen von Transform/Outline/Z-Index. | Keine Bildabnahme, kein Multi-Pointer-Test oder Despawn während eines aktiven Drags. |
| Mesh-Picking / `mesh_picking` | [mesh_picking.rs](../../bevy_test_apps/src/bin/mesh_picking.rs) bewahrt Mesh-Observer, reflektierten Zustand und Transforms. | Session-Anbindung, echte Hover-/Press-/Release-/Drag-Sequenz, zeitabhängige Rotation und Bildabnahme fehlen. |
| Material-/Kamerazustand / `blend_modes` | Drei native Tests in [blend_modes.rs](../../bevy_test_apps/src/bin/blend_modes.rs) prüfen gehaltene Pfeiltasten, getrennte Moduswechsel und deterministische Farbsequenzen. | Session-Anbindung und CLI-/Bildabnahme fehlen. Native Materialzustände sind kein Nachweis für korrekt gerenderte Bilder. |
| Allgemeines Inspect, Hierarchie, ungültige/tote Handles | [entities/tests.rs](../../src/session/inspect/entities/tests.rs), Reflection-Matrix und Plugin-Tests prüfen Filter, Projektionen, Hierarchietiefe und Handle-Ablehnungen. `tests/ui.py` nutzt lebende Handles über CLI. `tests/game_menu.py` prüft Bildschirm-/Button-Despawn, Ablehnung alter Handles auch nach Neuanlage eines gleichnamigen Bildschirms und frisch abgefragte Hierarchien. | Weitere Hierarchie-Lebenszyklen in Drag-and-drop und Mesh-Szenen; der geplante Game-Menu-Handle-Nachweis ist abgeschlossen. |
| Recording, Replay und Reports über Lebenszyklusgrenzen | `tests/slice.py`, `tests/ui.py`, `tests/observation.py`, `tests/session.rs` und `src/server/report_tests.rs` decken die einzelnen Abläufe, Client-Trennung und Report-Abschluss nach Session-Ende ab. | Kombinationen mit aktiver Aufnahme/Wiedergabe, Failure, Activity-Lücke und gemeinsamem Shutdown noch nicht als vollständige Szenariomatrix abgenommen. |
| CLI, REPL, Script und externer Agent | `tests/repl.py`, `tests/script.py` sowie [pi-Abnahme](pi-acceptance.md): gemeinsame Session-Steuerung, Barrieren, Client-Ende ohne impliziten Stop. | Große Outputs und anhaltende Activity unter langsamen Clients gehören noch zur Lastabnahme. |
| Activity- und Report-Last | Kleine Eviction-/Übergröße-/Cursor-Tests sowie kontrolliert langsame Provider sind vorhanden. | 4-MiB-Activity-Grenze mit realen großen Inspect-/Report-Outputs bemessen. Unbegrenzte wartende Report-Queue je Session bleibt offen; begrenzte fertige Activity begrenzt diese Queue nicht. |

### Erster zusätzlicher Szenentest: logical_state

Die erhaltene Szene bleibt ohne `slice` eine native Anwendung. Mit `slice` installiert
sie nach ihren eigenen Systemen `session::Plugin` und die anwendungseigene
`TimeUpdateStrategy::ManualDuration(20 ms)`. Der vorhandene FixedUpdate-Takt bleibt
10 ms, der wiederholende Timer 40 ms. Es gibt keine Änderung am produktiven
woodpecker-Koordinator oder dessen Zeitregeln.

Bevys erster Time-Update initialisiert die Uhr mit Delta 0. Das ist im Test
ausdrücklich berücksichtigt, nicht durch zusätzliche unsichtbare Ticks umgangen:

| Ausgeführte Simulationsticks insgesamt | Erwartete Updates | FixedUpdates | Timer-Abschlüsse |
| --- | --- | --- | --- |
| 0 | 0 | 0 | 0 |
| 1 | 1 | 0 | 0 |
| 2 | 2 | 2 | 0 |
| 3 | 3 | 4 | 1 |
| 4, nach vorgemerkt gedrücktem `a` | 4 | 6 | 1 |
| 7, davon drei Ticks mit Pace 25/s | 7 | 12 | 3 |
| 8, nach vorgemerkt losgelassenem `a` | 8 | 14 | 3 |

Press und Release ändern den reflektierten Zustand nicht vor dem Tick.
Über mehrere gehaltene Ticks entsteht keine zusätzliche Press-Flanke.
Pace begrenzt die reale Ausführungsgeschwindigkeit, nicht die simulierte Tickdauer.
Inspect und kurze reale Wartezeiten lassen alle Beobachtungswerte unverändert.

Reproduzierbarer Einstieg, vom Repository-Root:

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin logical_state
python3 tests/logical_state.py
```

Der separate Vorbau ist erforderlich. Der Test öffnet ein echtes Fenster und benötigt
eine verfügbare Desktop-Sitzung, jedoch keinen nativen Fokus oder physische Eingaben.
Konfiguration: [tests/fixtures/logical_state.toml](../../tests/fixtures/logical_state.toml).
Jeder Lauf behält `server.log`, `commands.jsonl` und Artefakte in einem eigenen
`target/logical-state-*`-Ordner, auch bei Fehlern. Pfad und Ergebnis werden ausgegeben.
Die JSONL-Datei enthält CLI-Aufruf, Exit-Code und vollständige stdout-/stderr-Antworten.
Der Test verwendet Fortsetzungscursor, behandelt `gap` als Fehler und beendet seine
eigene Session und seinen eigenen Server. Fremde laufende Sessions bleiben unberührt.

### Zweiter zusätzlicher Szenentest: game_menu

Mit `slice` installiert die Szene `session::Plugin` und wählt selbst
`TimeUpdateStrategy::ManualDuration(100 ms)`. Ohne `slice` bleiben automatische Zeit
und native Eingaben erhalten. Der Test nutzt vorhandene Namen und reflektierte
`SessionObservation`, `UiGlobalTransform`, `Interaction` und Hierarchien.
Er setzt keine Components direkt und ergänzt keine Automation-Marker.

Die CLI-Abnahme prüft diese Zeitpunkte ausdrücklich:

| Tick / Ereignis | Erwarteter Zustand |
| --- | --- |
| 0 und nach realem Warten | Splash/Disabled, Timer 0 |
| 1 | Erste Time-Initialisierung mit Delta 0, Splash-Timer weiterhin 0 |
| 10 | Splash-Timer 0,9 s, noch Splash |
| 11 | Splash-Timer 1 s, Übergang nur vorgemerkt |
| 12 | Menu/Disabled; `OnEnter` merkt Main vor |
| 13 | Menu/Main; Hauptmenü und Layout vorhanden |
| Klick-Tick auf einen Navigationsbutton | `Interaction::Pressed`, alter State noch aktiv |
| Folgetick | Neuer State und neue Hierarchie; alter Bildschirm despawnt |
| Eintritt ins Spiel | Game/Disabled, Spieltimer bereits 0,1 s |
| Weitere 48 Spielticks | 4,9 s, noch Game |
| Weiterer Tick | 5 s, Rückkehr nur vorgemerkt |
| Zwei weitere Ticks | Erst Menu/Disabled, dann Menu/Main |

Displayqualität `High` und Lautstärke `3` bleiben über Untermenüs, Spiel und Rückkehr
erhalten. Der Test prüft alte Handles von Splash, Hauptmenü, Buttons und Spiel sowie
frisch abgefragte Hauptmenü-Handles. Reale Wartezeiten und Inspect verändern weder
Timer noch State. Alle Warps stehen ausdrücklich im Szenario, auch zwischen Press
und Release; die Input-Helfer führen keine Ticks aus. Für reflektierte `f32`-Sekunden
gilt eine absolute Vergleichstoleranz von 1 µs, States und Einstellungen sind exakt.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin game_menu
python3 tests/game_menu.py
```

Der Test benötigt eine Desktop-Sitzung, aber keinen nativen Fensterfokus.
[tests/fixtures/game_menu.toml](../../tests/fixtures/game_menu.toml) verwendet lokale
Reports. Jeder Lauf behält `server.log`, vollständige CLI-Antworten in `commands.jsonl`
und Artefakte unter `target/game-menu-*`. Fortsetzungscursor verhindern das erneute
Lesen alter Activity; Lücken und unerwartete Outcomes führen zum Fehler.
Cleanup betrifft nur den eigenen Server und seine Session.

### Dritter zusätzlicher Szenentest: ui_drag_drop

Mit `slice` installiert die Szene `session::Plugin` und wählt anwendungseigene
20-ms-Ticks. Ohne `slice` bleiben automatische Zeit und native Eingaben erhalten.
Die bestehenden Drag-Observer und ihre Zustandsregeln bleiben unverändert.

[tests/ui_drag_drop.py](../../tests/ui_drag_drop.py) findet Grid und Tiles über
Namen im allgemeinen Entity-Inspect. Nach einem ausdrücklichen Layouttick liest
der Test tatsächliche Positionen und für das ungültige Ziel auch Tile-Größen.
Input-Helfer senden nur Commands; jeder Warp steht ausdrücklich im Szenario.

- Gültiger Drop: Amber über eine Zwischenposition auf Blue ziehen. Die Folge ist
  `DragStart, Drag, Drag, DragDrop, DragEnd`. Die Belegung wird
  `[Blue, Amber, Green, Rose]`; die tatsächlichen Positionen beider Tiles tauschen.
- Ungültiges Ziel: Amber über leeren Hintergrund außerhalb aller ruhenden Tiles
  ziehen. Die Folge ist `DragStart, Drag, DragEnd`, ohne weiteren akzeptierten Drop.
  Belegung bleibt unverändert, Amber kehrt an seinen Platz zurück.
- Während des Drags stimmen aktive Tile und Zwischenposition; Z-Index und Outline
  ändern sich. Nach beiden Abschlüssen sind Transform, Outline und Z-Index wieder
  im Ausgangszustand. Grid-Hierarchie und Layout aller vier Tiles werden geprüft.
- Angenommene Eingaben und reale Wartezeit ändern den Zustand ohne Tick nicht.
  Zwei abschließende Leerticks erzeugen keine wiederholten Drag-/Drop-Ereignisse.

Der vollständige Lauf umfasst 12 Simulationsticks, zwei DragStarts, drei Drags,
einen akzeptierten DragDrop und zwei DragEnds. Es gibt keine Screenshot-Prüfung.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin ui_drag_drop
python3 tests/ui_drag_drop.py
```

Eine Desktop-Sitzung ist nötig, nativer Fokus nicht. Die Konfiguration steht in
[tests/fixtures/ui_drag_drop.toml](../../tests/fixtures/ui_drag_drop.toml).
Jeder Lauf behält `server.log`, vollständige CLI-Antworten in `commands.jsonl` und
Artefakte unter `target/ui-drag-drop-*`. Der Test verwendet Fortsetzungscursor,
meldet Activity-Lücken als Fehler und beendet nur seinen eigenen Server samt Session.

### Konkreter Einstieg für die nächste Sitzung: mesh_picking

1. Arbeitsbaum prüfen und vorhandene Änderungen erhalten. `game_menu` liegt in
   `9eadb7d`; den aktuellen Commitstatus der Drag-Abnahme prüfen.
2. [mesh_picking.rs](../../bevy_test_apps/src/bin/mesh_picking.rs) lesen.
   Bei `slice` Session-Anbindung und anwendungseigene Zeitkonfiguration ergänzen,
   den nativen Weg erhalten.
3. Meshes, Kamera und reflektierten Zustand über allgemeines Inspect lesen.
   Pointer-Ziele aus der tatsächlichen Kamera-/Objektgeometrie bestimmen;
   keine neuen Automation-Marker oder direkten Observer-Aufrufe einführen.
4. Hover, Press, Release und Drag mit virtuellen Pointer-Commands sowie
   zeitabhängige Rotation durch ausdrückliche Warps prüfen. Bildabnahme ergänzen;
   bloß korrekte Transforms beweisen keine korrekte Darstellung.
5. Matrix und Testnachweise aktualisieren. Danach `blend_modes`, anschließend
   die übergreifende Lebenszyklus-/Lastmatrix bearbeiten. Block 6 bleibt offen.

## Konkreter nächster Durchstich

Entity-Inspect sowie Pointer-, Keyboard- und Text-Commands sind umgesetzt.
Die Reflection-Matrix aus Block 1 ist abgeschlossen. Dafür waren nur Tests und
Dokumentation nötig, keine Änderungen am produktiven Inspect-Verhalten.
Screenshot aus Block 2 ist ebenfalls umgesetzt und mit zwei gerenderten
Context-Menu-Sessions geprüft. Recording und Replay aus Block 3 sind implementiert.
Die gerenderte Replay-Abnahme besteht in vier erneuten Läufen ohne Codeänderung.
Die früheren schwarzen PNGs und die mögliche Display-Bedingung sind unten festgehalten.

Die Diagnoseerfassung, Snapshot-Konstruktion und gemeinsame Report-Darstellung
aus Block 4 sind umgesetzt. `Report` enthält Titel, Failure, Signatur und Context;
`to_markdown` verwendet ausschließlich diesen unveränderlichen Snapshot.
Local und Github einschließlich Duplikatsuche und Fallback sind umgesetzt.
Die automatische clientunabhängige Report-Arbeit ist ebenfalls umgesetzt. Activity
enthält Snapshot und `submitted`/`failed`/`interrupted` gemeinsam. Fristablauf und
erzwungener Stopp brechen Provider-Arbeit ab und warten auf Ressourcenbereinigung.
Ein direktes `report::submit` außerhalb des Servers bleibt synchron und ohne Serverfrist.
Bei konfiguriertem GitHub-Provider veröffentlicht der Server ohne zusätzliche Freigabe
pro Report. Entwicklungstests verwenden weiterhin Fixtures, damit sie keine echten Issues anlegen.
Die REPL aus Block 5 ist auf dem bestehenden Client-/Activity-Vertrag umgesetzt.
`woodpecker --address <adresse> session repl <id>` bietet die festgelegten Kurzformen
und `command <command-json>` für alle gemeinsamen Commands. Ein separater Netzwerkworker
hält Eingabe, Hilfe und Quit während ausstehender Commands und Netzwerkantworten bedienbar.
`pending` liest eine serverseitige Momentaufnahme unabhängig von Activity-Eviction.
Wiederverbindung behält den Cursor; unbestätigte Einreichungen werden nicht wiederholt.
Activity-Lücken melden unbekannte Ausgänge statt Ergebnisse zu erfinden.
Scripts sind ebenfalls umgesetzt: vollständige v1-Vorvalidierung, Zuordnung zu
ursprünglichen Array-Indizes, normale Commands ohne Ergebnisbarriere und ausdrückliche
Barrieren vor Recording-Start/-Stop und Shutdown. Teilweise bekannte Ergebnisse bleiben
bei Client-Fehlern erhalten; unbekannte Ausgänge und nicht eingereichte Commands sind getrennt.
Einstieg: `woodpecker --address <adresse> session script <id> --file <datei>`.
Die externe Agent-Abnahme mit vom Nutzer gestartetem pi ist durchgeführt.
[Nachweis und Startanleitung](pi-acceptance.md) dokumentieren CLI-Ergebnisse,
Recording und die menschliche REPL-Bedienung nach Ende von pi.
Der [Arbeitsauftrag](pi-task.md) bleibt für Wiederholungen verfügbar.
Keinen Modellzugang oder eigene Agent-Werkzeugschleife in woodpecker ergänzen und keine
externe Laufzeit ohne Auftrag starten. Block 6 ist begonnen: Abdeckungsmatrix und
`logical_state`-, `game_menu`- und `ui_drag_drop`-CLI-Abnahmen sind ergänzt. Als Nächstes folgt `mesh_picking` nach dem
konkreten Einstieg oben; System- und Lastabnahme insgesamt bleiben offen.
Es gibt weiterhin kein `failure.json`.
Vor weiterer Arbeit den tatsächlichen Arbeitsbaum prüfen und spätere lokale
Änderungen erhalten.

Die fehlenden Glyphen für `ü`, `ß`, Emoji und Japanisch in der Context-Menu-
Standardschrift sind reproduziert. Die gespeicherten Unicode-Texte sind korrekt.
Font-/Fallback-Korrekturen sind auf Nutzerwunsch zurückgestellt.

Die [Context-Menu-Anwendung](../../bevy_test_apps/src/bin/context_menu.rs)
ist angebunden; [tests/ui.py](../../tests/ui.py) weist diesen Ablauf nach:

1. Die Anwendung über Server und Session starten und Ready beobachten.
2. Falls Layout-Initialisierung Ticks benötigt, diese ausdrücklich ausführen.
3. Ein benanntes UI-Element über allgemeines Inspect finden und seinen Handle
   sowie den benötigten Zustand lesen. Das ist kein neuer spezieller Name-Selector.
4. Pointer-Eingabe senden und nachweisen, dass ohne weiteren Tick noch keine
   entsprechende Änderung des Anwendungszustands verarbeitet wurde.
5. Einen ausdrücklichen Warp ausführen und dessen korrelierten Abschluss abwarten.
6. Die erwartete Zustandsänderung erneut per Inspect lesen.

Dieser Test verbindet Input, Entity-Inspect und tatsächliche Bevy-Ausführung.
Die Reflection-Fixtures ergänzen die Varianten und Fehlerfälle aus Block 1,
ohne dafür Fenster zu öffnen.

## Nachweise und bekannte Grenzen

Nach dem `ui_drag_drop`-Durchstich bestanden:

- CLI-Build und separater Build von `ui_drag_drop` mit `slice`.
- Vollständige CLI-Abnahmen mit gültigem und ungültigem Drop, unter anderem
  `target/ui-drag-drop-6dp197a2`, `target/ui-drag-drop-72zzje5d` und
  `target/ui-drag-drop-kp42tkcg`, unverändert nach Ergänzung beider Szenarien.
  Nach der Rustfmt-Korrektur erneut gebaut und bestanden:
  `target/ui-drag-drop-uhob_pu5`.
- `cargo test --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets`:
  alle 8 App-/Kompositionstests. `ui_drag_drop` selbst besitzt keine nativen Unit-Tests;
  der neue fachliche Nachweis läuft über CLI und echte Session.
- `cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin ui_drag_drop -- -D warnings`:
  ohne Lint-Ausnahmen bestanden.
- `cargo check --manifest-path bevy_test_apps/Cargo.toml --no-default-features --bin ui_drag_drop`.
- Root-Formatprüfung, gezielte Rustfmt-Prüfung der Szene und `git diff --check`.

Vor der Session-Anbindung scheiterte der neue Starttest erwartungsgemäß am fehlenden
Ready (`target/ui-drag-drop-ccbqfkp9`). Nach Ergänzung des Plugins bestand der
Start-/Stillstandstest, anschließend beide Drag-Szenarien. Keine Änderung an der
produktiven Session-/Pointer-Mechanik oder den Drag-Regeln.
Rustfmt verlangte zusätzlich die Umformatierung eines bestehenden mehrzeiligen
`let`-Patterns in `drag::drop`; keine fachliche Änderung.
Die bekannte nicht fatale macOS-Linkerwarnung zur `__eh_frame`-Größe erschien erneut.

Der vorherige Game-Menu-Stand wurde auf Nutzerauftrag als `9eadb7d` committed,
nicht gepusht. Die danach ergänzte Drag-Abnahme wurde nicht committed.
Root-Gesamttests, ältere Python-Abnahmen, Screenshots und Lasttests wurden in
diesem Schritt nicht erneut ausgeführt. Keine Subagenten oder externen Agent-Läufe.

Nach dem `game_menu`-Durchstich bestanden:

- CLI-Build und separater Build von `game_menu` mit `slice`.
- Zwei vollständige unveränderte Läufe von `python3 tests/game_menu.py`:
  `target/game-menu-op5vbwl2` und `target/game-menu-1go85k9z`.
  Beide prüfen Navigation, Einstellungen, virtuelle Klicks, Hierarchien, tote
  Handles und die vollständigen Splash-/Spiel-Timergrenzen.
- `cargo test --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets`:
  alle 8 App-/Kompositionstests, darunter beide nativen Game-Menu-Tests.
- `cargo check --manifest-path bevy_test_apps/Cargo.toml --no-default-features --bin game_menu`.
- Root-Formatprüfung, gezielte Rustfmt-Prüfung der Szene und `git diff --check`.

Der Starttest scheiterte vor der Session-Anbindung erwartungsgemäß am fehlenden
Ready (`target/game-menu-qi_nztnd`). Mit Plugin und anwendungseigenem Zeitschritt
bestand der Start-/Stillstandstest. Beim Ausbau wurden die Testannahmen korrigiert:
`f32`-Sekunden brauchen eine numerische Toleranz, `OnEnter(Menu)` setzt Main erst im
nächsten Tick, und der registrierte Type Path ist `bevy_ui::focus::Interaction`.
Produktive Session-/Zeit-/Input-Mechanik wurde nicht geändert.

Gezieltes Clippy mit `--features slice --bin game_menu -- -D warnings` scheitert
an vier bereits vorhandenen Befunden in der Szene: ableitbares `DisplayQuality::Default`,
zwei komplexe Query-Typen und acht Parameter von `button_actions`. Diese unveränderten
nativen Systeme wurden für die Session-Anbindung nicht umgebaut; Clippy ist für
diesen Durchstich nicht grün. Beim Linken erschien außerdem die bekannte nicht fatale
macOS-Warnung zur `__eh_frame`-Größe.

Nach Rücksprache ist `DisplayQuality::Default` nun abgeleitet; `Medium` bleibt der
Standardwert. Beide nativen Game-Menu-Tests sowie Format- und Diff-Prüfung bestanden
erneut. Clippy bestätigt nur noch die drei Query-/Parameter-Befunde. Diese bleiben
auf Nutzerwunsch vorerst offen; es wurden keine `allow`-Attribute oder Lint-Features
ergänzt.

Bis zur erneuten Bewertung nach dem nächsten Bevy-Update werden die beiden Regeln
nur beim gezielten Clippy-Aufruf für `game_menu` ausgenommen:

```sh
cargo clippy \
  --manifest-path bevy_test_apps/Cargo.toml \
  --features slice --bin game_menu \
  -- -D warnings \
  -A clippy::type_complexity \
  -A clippy::too_many_arguments
```

Dieser Aufruf wurde ausgeführt und besteht. Die Ausnahmen gelten für den geprüften
Target-Code, nicht nur für die drei bekannten Fundstellen. Alle übrigen Warnungen
bleiben Fehler. Kein zusätzliches Cargo-Feature und keine dauerhafte Lint-Ausnahme
im Quellcode oder in der Projektkonfiguration. Nach dem Bevy-Update zuerst ohne
die beiden `-A`-Optionen prüfen und die verbliebenen Befunde neu bewerten.
Der erfolgreiche Lauf mit Ausnahmen ist kein Behebungsnachweis für diese Befunde.

Root-Gesamttests und die älteren Python-Abnahmen wurden nicht erneut ausgeführt.
Keine neue Screenshot-/Font-Abnahme oder Lastbemessung. Keine Commits, Pushes,
Subagenten oder externe Agent-Läufe in diesem Schritt.

Nach dem ersten zusätzlichen Szenendurchstich aus Block 6 bestanden:

- CLI-Build und separater Build von `logical_state` mit `slice`.
- `python3 tests/logical_state.py`: vollständiger CLI-/Bevy-Lauf mit
  8 Updates, 14 FixedUpdates, 3 Timer-Abschlüssen und Keyboard-Press/Hold/Release.
  Zwei vollständige Läufe bestanden unverändert nach Erweiterung um die Keyboard-
  und Pace-Prüfungen. Letzter Ergebnisordner:
  `target/logical-state-9q3u1qau`.
- `cargo test --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets`:
  alle 8 vorhandenen App-/Kompositionstests bestanden, einschließlich der nativen
  Menü- und Materialtests.
- `cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin logical_state -- -D warnings`.
- `cargo check --manifest-path bevy_test_apps/Cargo.toml --no-default-features --bin logical_state`:
  der native Weg bleibt baubar.
- Root-Formatprüfung, gezielte Formatprüfung der geänderten Szene und `git diff --check`.

Der neue Test wurde zuerst ohne Session-Anbindung ausgeführt und scheiterte am fehlenden
Ready. Nach Ergänzung des Plugins bestand der Start-/Stillstandstest. Anschließend
scheiterte die genaue FixedUpdate-Prüfung mit der bisherigen Echtzeitkonfiguration.
Die Szene erhielt daraufhin ausschließlich unter `slice` ihre eigene feste 20-ms-Dauer.
Damit besteht der Timer-Test; die Zeitregeln von woodpecker wurden nicht verändert.
Die fehlgeschlagenen Läufe sind unter `target/logical-state-2kt4gu3c` und
`target/logical-state-y98gftbo` erhalten.

Beim Linken des Fensterbinaries erschien die bekannte macOS-Warnung über eine zu große
`__eh_frame`-Sektion; Build und Abnahme bestanden. Es gab keinen neuen Screenshot- oder
Font-Nachweis. Root-Gesamttests, Context-Menu-CLI-Abnahme und Lasttests wurden in diesem
Schritt nicht erneut ausgeführt. Keine Commits, Pushes oder externen Agent-Läufe.

Nach Ergänzung der Script-Ausführung bestanden:

- `cargo clippy --all-targets --all-features -- -D warnings`, Format- und Diff-Prüfung.
- `cargo test --no-default-features --lib -- --test-threads=1`: 92 Tests.
- Abschließender vollständiger Lauf `cargo test --all-features -- --test-threads=1`:
  132 Bibliotheks-/CLI-Tests, 7 Beobachtungs- und 13 Session-Prozesstests.
- Acht Script-Tests prüfen vollständige Vorvalidierung einschließlich später Fehler,
  Shutdown-Position und nicht darstellbarer Rust-Werte, normale Einreichung ohne
  Ergebnisbarriere, alle drei Abschlussbarrieren, vertauschte Ergebnisse und fremde IDs.
  Vor-/Nachannahme-Ablehnungen, technische Fehler, beschädigte Outputs, Activity-Lücken,
  verlorene Submit-Bestätigung und Session-Ende behalten ihre ursprünglichen Indizes.
- Nach CLI- und Zähler-Vorbau bestanden `python3 tests/script.py`, `tests/repl.py`,
  `tests/slice.py` und `tests/shutdown.py`. Die neue Script-Abnahme nutzt zwei echte
  Bevy-Sessions, prüft die tatsächliche Recording-Datei und lässt einen angenommenen
  Warp nach Ctrl+C weiterlaufen, bis ein anderer Client ihn ausdrücklich stoppt.

In einem vorherigen Gesamtlauf bestanden 131 Bibliotheks-/CLI-Tests; der bestehende
Test langsamer Server-Reports scheiterte. Das `gh`-Fixture erreichte die erwartete
Warteposition nicht innerhalb von 30 Sekunden. Die Ursache ist weiterhin ungeklärt;
die abgefragten Systemlogs lieferten diesmal keine Gatekeeper-Rejection. Der abschließende
Lauf bestand ohne Änderung an diesem Test oder am Report-Provider. Das ist kein
Behebungsnachweis für den bereits dokumentierten sporadischen Fehler.

Script verwendet keine automatische Wiederverbindung und wiederholt keine Einreichungen.
Bekannte Ergebnisse bleiben bei Abbruch erhalten; unbekannte Ausgänge und nicht
eingereichte Positionen sind getrennt. Eine leere Liste ist erfolgreich. Lange Commands
haben keinen pauschalen Script-Timeout; der CLI-Client bleibt über Ctrl+C abbrechbar.
Keine neuen Abhängigkeiten, keine externe Agent-Laufzeit und keine echten
GitHub-Veröffentlichungen in diesem Schritt. Keine Commits oder Pushes.

Nach Ergänzung der REPL bestanden:

- `cargo clippy --all-targets --all-features -- -D warnings`, Format- und Diff-Prüfung.
- `cargo test --no-default-features --lib -- --test-threads=1`: 92 Tests.
- Abschließender vollständiger Lauf `cargo test --all-features -- --test-threads=1`:
  124 Bibliotheks-/CLI-Tests, 7 Beobachtungs- und 13 Session-Prozesstests.
- Parser-Tests prüfen alle kopierbaren Inspect-Beispiele, vollständige Type Paths,
  Handle-Grenzen, gemeinsame JSON-Commands und ungültige Eingaben.
- Eine serverseitige Momentaufnahme behält Pending auch bei vollständiger
  Activity-Verdrängung. Netzwerkfixtures prüfen Wiederverbindung mit altem Cursor,
  sichtbare Lücken, ungewisse Einreichung ohne Wiederholung sowie das Beenden
  blockierter Handshakes und Activity-Antworten.
- Nach CLI- und Zähler-Build: `python3 tests/repl.py`, `tests/slice.py` und
  `tests/shutdown.py`. Die REPL-Abnahme nutzt zwei echte Bevy-Sessions, Pseudoterminals,
  parallele Clients, weiterlaufenden Warp nach Client-Ende, Replay-Stop und Shutdown.

Ein vorheriger Gesamtlauf scheiterte erneut im bestehenden Mehrsession-Fristtest:
Ein Prozessfixture wurde vor Ready mit SIGKILL beendet, zeitgleich steht eine
Gatekeeper-Rejection im Systemlog. Der isolierte Test bestand unverändert.
Ein weiterer Lauf meldete einen Fehler im bestehenden Panic-/Abort-Beobachtungstest;
dessen Diagnose wurde durch gleichzeitig beschriebene Logdateien überschrieben und ist
nicht belastbar rekonstruierbar. Der abschließende Lauf verwendete eine eindeutige
Logdatei und bestand vollständig. Diese Wiederholungen belegen keine Behebung der
sporadischen Fehler.

Der erste Pseudoterminal-Test las nach einem Teil der großen Inspect-Ausgabe nicht weiter
und blockierte dadurch den stdout-Schreiber. Der Test leert den Kanal jetzt auch beim
Beenden. Ein eigener Supervisor hält das macOS-Pseudoterminal bis zur Prüfung der
wiederhergestellten Terminalattribute offen. Produktcode musste für diese Testfehler
nicht geändert werden. Terminalausgabe bleibt synchron; ein nicht lesender Empfänger
kann sie blockieren. Vollständige Lastbemessung bleibt Block 6.

Keine Script-Ausführung, externe Agent-Abnahme, echten GitHub-Veröffentlichungen,
Commits oder Pushes in diesem Schritt.

Nach Ergänzung automatischer Server-Reports bestanden:

- `cargo clippy --all-targets --all-features -- -D warnings`.
- `cargo test --no-default-features --lib -- --test-threads=1`: 92 Tests.
- Ein vollständiger abschließender Lauf `cargo test --all-features -- --test-threads=1`:
  115 Bibliotheks-/CLI-Tests, 7 Beobachtungs- und 13 Session-Prozesstests.
- Server-Tests prüfen unveränderte Snapshots trotz späterer Commands, Report-Ergebnisse
  nach Session-Ende, Submit-Fehler bei weiter bedienbarer Session und einen protokollseitig
  markierten Panic während des Shutdowns. Dessen fehlgeschlagener Command darf den
  getrennten Empfang von Failure, Endevent und Report nicht überspringen.
- Sechs echte Bevy-Sessions mit blockierten `gh`-Fixtures prüfen regulären Abschluss,
  gemeinsame Frist und erzwungenen Stopp. Auch Nachfolger halten dabei Pipes offen:
  der Test erreicht den tatsächlichen Prozessgruppen-/Pipe-Abschluss, nicht nur einen
  Timeout des aufrufenden Threads. Nach erzwungenem Abbruch entsteht kein Fallback.
- Lokale Cancellation-Tests prüfen Reads sowie Abbruch vor und nach temporärem Schreiben.
  Bereits abgebrochene wartende Jobs starten weder `gh` noch Local.
- CLI-Build, Zähler-Vorbau und `python3 tests/observation.py` mit automatischen lokalen
  Markdown-Dateien nach Client-Trennung; danach `tests/slice.py` und `tests/shutdown.py`.
  Die Signal-Abnahme prüft vier Szenarien mit zwölf Spielprozessen.

Ein vorheriger Gesamtlauf bestand mit 114 Bibliotheks-/CLI-Tests; danach wurde der
Shutdown-Failure-Test ergänzt. In einem weiteren Lauf wurde ein Prozessfixture vor
Ready mit SIGKILL beendet; das Systemlog enthält eine zeitlich passende Gatekeeper-Rejection.
Im gleichen Lauf erreichte eines von zwei `gh`-Fixtures seine Warteposition nicht,
obwohl beide Request-Dateien vollständig geschrieben waren. Diese zweite Ursache ist
nicht geklärt. Zusätzliche Testdiagnose erkennt vorzeitige Provider-Ergebnisse und nennt
den Aufrufer eines Timeouts. Der separate Test und der abschließende Gesamtlauf bestanden
ohne Änderung am produktiven Provider für diesen Befund; das ist kein Behebungsnachweis.
Ein Zähler-Abnahmelauf ohne den vorgeschriebenen separaten Vorbau blieb 90 Sekunden in
`Starting`. Nach explizitem Vorbau bestanden Zähler- und Signal-Abnahme.

Die Report-Queue je Session ist derzeit unbeschränkt; die Activity-Byte-Grenze begrenzt
nur fertige Einträge. Anhaltende Fehlerlast bei langsamem Provider muss in Block 6
vermessen werden. Lokales IO bleibt kooperativ abbrechbar, nicht hart unterbrechbar.
Keine gerenderte Abnahme, echte GitHub-Veröffentlichung, Commits oder Pushes in diesem Schritt.
Die früheren Display-/Glyphen-Einschränkungen bleiben unverändert.

Nach Ergänzung des GitHub-Providers bestanden:

- `cargo clippy --all-targets --all-features -- -D warnings`.
- `cargo test --no-default-features --lib -- --test-threads=1`: 89 Tests.
- `cargo test --all-features -- --test-threads=1`: 109 Bibliotheks-/CLI-Tests,
  7 Beobachtungs- und 13 Session-Prozesstests.
- Sieben GitHub-Provider-Tests verwenden ausführbare Prozessfixtures. Sie prüfen
  paginierte offene/geschlossene Issues, Pull-Request-Ausschluss, vollständige
  Marker, ungültige spätere Seiten, unveränderte Titel, vollständiges Markdown
  über stdin und große gleichzeitige Pipe-Ausgaben.
- Fehlendes Programm, Exit-Status und ungültige Antworten bleiben mit `Search` oder
  `Publish` typisiert. Alle sechs Fehler-/Operationskombinationen prüfen lokalen
  Fallback einschließlich vorhandener Dateien und doppeltem Fehler.
- Ein zusätzlicher Bevy-Prozesstest nutzt die öffentliche Session-API mit einem
  ausschließlich im Test-Unterprozess geänderten PATH. Er prüft `Created`,
  `Existing`, Fallback, Projektarbeitspfad, Startkonfiguration und Aufrufe nach
  Session-Ende. Der echte `gh`-Client wurde nicht für API-Aufrufe verwendet.

Die `gh api`-Argumente wurden gegen die lokal installierte CLI-Hilfe geprüft.
Die Suche liest alle von `--paginate` gelieferten JSON-Arrays; unvollständige
Antworten führen nicht zu einer Veröffentlichung. Erfolgreiche Remote-Aufrufe
erzeugen keine lokale Kopie. Ein Publish-Fehler kann trotzdem ein bereits angelegtes
Issue bedeuten; der Provider meldet Fallback und wiederholt POST nicht automatisch.
Zusätzliche Provider-Konfigurationsfelder werden für beide Provider abgelehnt.
Automatische Server-Report-Arbeit, Deadline-Anbindung und gerenderte Abnahmen waren
nicht Teil dieser Erweiterung. Keine echte Veröffentlichung, keine neuen Dependencies.

Nach Ergänzung des lokalen Providers bestanden:

- `cargo clippy --all-targets --all-features -- -D warnings`.
- `cargo test --no-default-features --lib -- --test-threads=1`: 82 Tests.
- `cargo test --all-features -- --test-threads=1`: 102 Bibliotheks-/CLI-Tests,
  6 Beobachtungs- und 13 Session-Prozesstests, vollständig ohne Startfehler.
- Acht Provider-Tests prüfen unabhängige Roots, Unicode-Verzeichnisse, unveränderte
  bestehende Reports, volle Signaturen einschließlich Version/Algorithmus,
  Diagnose-Fences, Symlinks einschließlich In-Root-Links, FIFOs, Pfadwechsel,
  Lese-/Schreib-/Publikationsfehler und Cleanup.
- Acht gleichzeitig bis zur Publikation synchronisierte Schreiber liefern genau
  einmal `Created` und siebenmal `Existing`; die endgültige Datei ist vollständig.
- Der zusätzliche Bevy-Prozesstest prüft tatsächliche Markdown-Dateien während der
  laufenden Session und nach Session-Ende, Startkonfiguration, relative Artefakt-Roots
  sowie unveränderte History und Events.

Der Provider schreibt zunächst eine neue Datei im Zielverzeichnis, synchronisiert
deren Inhalt und setzt den endgültigen Namen per Hardlink ohne Überschreiben ein.
Dateisysteme ohne Hardlink-Unterstützung liefern einen Schreibfehler; es gibt keinen
unsicheren Ersatzweg mit Überschreiben. Unix-Dateien werden mit Modus `0600` angelegt.
Marker innerhalb von Code-Fences zählen nicht; mehrere eigene Markerzeilen sind
mehrdeutig und ergeben `Conflict`. Config-Prüfung und Provider teilen die Pfadregel;
auch innere `.`-Komponenten werden abgelehnt.
GitHub, automatische Server-Report-Arbeit und Render-Abnahmen waren nicht Teil
dieser Erweiterung. Die früheren SIGKILL-/Display-Befunde sind dadurch nicht behoben.

Nach Ergänzung der gemeinsamen Report-Darstellung bestanden:

- `cargo test --no-default-features --lib -- --test-threads=1`: 74 Tests.
- `cargo clippy --all-targets --all-features -- -D warnings`.
- Ein vollständiger Lauf `cargo test --all-features -- --test-threads=1` mit
  94 Bibliotheks-/CLI-Tests, 5 Beobachtungs- und 13 Session-Prozesstests.
- Acht neue Report-Tests prüfen die vier Golden Vectors, vollständige und
  unvollständige ANSI-Sequenzen, beide Projektpfad-Schreibweisen, ASCII-Token-Grenzen,
  Kalender-/Zeit-/Offset-Gültigkeit, Unicode-Skalargrenzen, fehlende Diagnosen
  und vollständige Markdown-Nutzdaten mit langen Backtick-Folgen.
- Der erweiterte reale Bevy-Snapshot-Test prüft die private Projektpfad-Normalisierung
  und identisches Markdown nach History-Änderung und Session-Ende.

Ein weiterer Gesamtlauf scheiterte in zwei bestehenden Session-Tests
(`protocol_errors_are_correlated_without_poisoning_other_work` und
`replay_technical_failures_are_completions_not_old_outcome_comparisons`):
`process_fixture` endete vor Ready mit SIGKILL. Die Report- und Beobachtungstests
bestanden auch dort. Die abgefragten macOS-Systemlogs enthalten Provenance-Einträge,
aber keinen eindeutigen Kill-/Ablehnungsnachweis für diese Starts. Die Ursache
bleibt offen; der erfolgreiche Gesamtlauf beweist keine Behebung dieses Befunds.
Keine Prozess-, Sicherheits- oder Test-Wiederholungsregeln wurden geändert.
Die Python-/Render-Abnahmen wurden für diese Darstellungserweiterung nicht erneut
ausgeführt; deren bisherige Nachweise und Display-/Glyphen-Einschränkungen gelten weiter.

Nach Fehlerbeobachtung und Snapshot-Konstruktion bestanden:

- Clippy für alle Targets/Features mit `-D warnings`.
- `cargo test --all-features -- --test-threads=1`: 86 Bibliotheks-/CLI-Tests,
  5 Beobachtungs-Prozesstests und 13 Session-Prozesstests.
- `cargo test --no-default-features --lib -- --test-threads=1`: 66 Tests.
- CLI-, Zähler- und Context-Menu-Builds.
- `python3 tests/observation.py`: zwei echte Sessions, Failure-Events über
  CLI/Activity, Tracing-Error und behandelter Panic bei weiterhin bedienbarer Session.
- `python3 tests/slice.py` und `python3 tests/ui.py`, einschließlich Replay.
- `python3 tests/shutdown.py`: vier Szenarien mit zwölf Prozessen.
- Format- und Diff-Prüfung.

Ein weiterer kombinierter Prüfaufruf erreichte nach einem erneuten Dependency-Build
die gesetzte 200-Sekunden-Werkzeugfrist. Zu diesem Zeitpunkt waren die 86
Bibliotheks-/CLI-Tests und die 5 Beobachtungs-Prozesstests grün; die laufenden
Session-Tests wurden unterbrochen. Deren separater Wiederholungslauf bestand
mit allen 13 Tests.

Der Markerparser ist an jeder Byte-Trennstelle, mit gemischten Event-IDs,
beschädigten/unvollständigen Markern und unveränderten menschlichen Diagnosebytes
geprüft. Handshake-Tests prüfen beide Reihenfolgen von Ready und Layer-Bestätigung.
Echte Bevy-Prozesse prüfen System-, Worker- und Task-Panics, unbekannte Payloads,
verketteten vorherigen Hook, Fehler vor Ready, Tracing-Filter und fehlenden Layer.
Der Abort-Nachweis beendet den Prozess aus dem verketteten Hook; ein separat mit
`panic=abort` gebautes Programm wurde noch nicht getestet.
Der Snapshot-Nachweis hält einen Warp offen, konstruiert den Report und beendet
danach Warp und Session, ohne dass sich der Report verändert.
Optionale Metadatenabfragen sind auf eine Sekunde je Toolaufruf begrenzt und
abbrechbar; fehlende oder hängende Zusatzwerkzeuge verhindern den Start nicht.

Nach dem Replay-Durchstich bestanden:

- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features -- --test-threads=1`: 79 Bibliotheks-/CLI-Tests
  und 13 Prozess-Integrationstests.
- `cargo test --no-default-features --lib -- --test-threads=1`: 59 Tests.
- CLI-, Zähler- und Context-Menu-Builds sowie `python3 tests/slice.py` mit vier Sessions.
- Vier erneute Läufe von `python3 tests/ui.py --capture-dir ...`, jeweils mit zwei
  gerenderten Sessions, vollständigem Replay, Screenshot-Prüfungen, exakter
  Replay-Ticksumme und unveränderter zweiter Session.
- `cargo fmt --all -- --check` und `git diff --check`.

Die Replay-Tests prüfen Dateistruktur, Version-Priorität, verschachtelte doppelte
Schlüssel, Symlinks, FIFO-Ablehnung, Lesefehler und Footer. Dazu kommen effektive
und Null-Tick-Warps, Stop während Laden und Ausführung, mehrere wartende Stops,
technische Blockierungen und normale interne Request-IDs ohne verwaiste Ergebnisse.
Prozessfixtures prüfen vollständige Vorabvalidierung, Recording-Roundtrip,
Pending-Drop, externe Ablehnungen und Session-Ende. Die Zählerabnahme prüft auch
Replay plus aktives Recording beim Verwaltungs-Stopp.

Vor den vier erfolgreichen UI-Läufen scheiterten zwei Läufe bereits vor Replay:
Inspect meldete ein geöffnetes Menü, aber sowohl die geschlossene als auch die
geöffnete Szene ergaben vollständig schwarze PNGs. Tim hat angegeben, dass er
möglicherweise den Bildschirm vorzeitig zugeklappt hatte. Das ist eine plausible
Umgebungsbedingung, keine bestätigte Ursache. Die vier erneuten Läufe bestanden
ohne Codeänderung; kein Screenshot-Fix auf Verdacht und keine Abschwächung der
Bildprüfung wurden vorgenommen. Für gerenderte Abnahmen den Mac offen und wach
halten. Die Replay-Abnahme ist nachgewiesen, die Ursache der früheren schwarzen
Bilder bleibt ungeklärt. Lokale Bilder liegen unter `target/replay-ui-evidence`
und `target/replay-ui-diagnosis`; diese Verzeichnisse sind keine versionierten Fixtures.
Der Context-Menu-Build meldet weiterhin die nicht fatale macOS-Linkerwarnung
`__eh_frame section too large`.

Nach dem Recording-Durchstich bestanden 64 Bibliotheks-/CLI-Tests mit allen
Features, 9 Prozess-Integrationstests und 44 Bibliothekstests ohne Default-Features.
Clippy für alle Targets/Features mit `-D warnings`, Formatprüfung sowie die
realen Zähler- und UI-Abnahmen bestanden ebenfalls.
Die Aufnahme enthält unter anderem Input, Inspect, Warp, Screenshot und
fachliche Ablehnungen; Commands der zweiten Session und Recording-Steuerung
bleiben draußen. Start und Stop verändern weder Ticks noch simulierte Zeit.
Der Verwaltungs-Stopp schließt eine aktive Aufnahme ausdrücklich mit Recording-Stop,
wartet auf den Footer und führt erst danach den weiterhin strikt geprüften Shutdown aus.
Die Zählerabnahme prüft auch diesen Ablauf.

Unit-Tests injizieren Header-, aktive Schreib-, Footer- und Sync-Fehler.
Sie prüfen `Io`, Header-Cleanup, `RecordingFailed`, Rückkehr nach Idle und die
Fehlerreihenfolge beim Session-Ende. Prozessfixtures prüfen absichtlich vertauschte
Response-Reihenfolge, Shutdown-Blockierung, verworfene Pending-Handles,
Protokollfehler und `unanswered` bei Prozessende.
Dieser ältere Nachweis umfasst noch kein Replay; dessen Stand steht oben.

Nach dem Screenshot-Durchstich bestanden:

- Ein vollständiger Lauf `cargo test --all-features -- --test-threads=1` mit
  57 Bibliotheks-/CLI-Tests und 6 Prozess-Integrationstests.
- 37 Bibliothekstests ohne Default-Features; Clippy für alle Targets und Features
  mit `-D warnings`.
- Der Context-Menu-Anwendungstest, `tests/slice.py` mit vier Sessions und
  `tests/ui.py` mit zwei gerenderten Sessions. Die PNGs wurden auch visuell geprüft.

In einem weiteren vollständigen Rust-Lauf scheiterte der bestehende
Server-Fristtest: ein Kindprozess wurde vor Ready mit SIGKILL beendet.
Die übrigen 56 Tests bestanden. Die Ursache dieses Prozessstarts ist nicht
geklärt; das ist kein Nachweis eines Screenshot-Fehlers und kein durch einen
grünen Lauf aufgehobener Befund. Die Render-Abnahme und der vollständige
erfolgreiche Lauf sind separate Nachweise.

Der Screenshot-Adapter verwendet Bevys Readback-Kanal, liefert ihn aber im
Kontrolllauf statt in Update aus. PNG-Encoding und Datei-I/O blockieren diesen
Loop nicht. Ohne Readback endet der Request nach 30 realen Sekunden; spätes
Ergebnis wird ignoriert. Absolute Symlinks werden von der Pfadsandbox abgelehnt,
auch wenn sie auf einen Ort innerhalb des Roots zeigen. Relative In-Root-Links
sind geprüft. Die übrigen UI-Szenen und `tests/shutdown.py` wurden nicht erneut
abgenommen.

Nach Ergänzung der Reflection-Matrix bestanden:

- `cargo test --all-features -- --test-threads=1`: 49 Bibliotheks-/CLI-Tests
  und 6 Prozess-Integrationstests.
- `cargo test --no-default-features --lib -- --test-threads=1`: 35 Tests.
- `cargo test --all-features --features serde_json/preserve_order --lib session::inspect -- --test-threads=1`:
  alle 16 Inspect-Tests. Die Set-Sortierung hängt nicht von der JSON-Map-Implementierung ab.
- Clippy für alle Targets und Features mit `-D warnings`, Formatprüfung und
  `git diff --check`.

Die neuen Tests prüfen wiederholtes Lesen ohne veränderte World-/Component-/Resource-
Change-Ticks. Bevy 0.19.1 reflektiert `BTreeSet` opak, ohne `ReflectSerialize`;
dieser Fall ergibt erwartungsgemäß `NotSerializable`. Rekursiv reflektierte Sets
werden mit `HashSet` geprüft. Die gerenderten Python-Abnahmen wurden für diese
reine Testerweiterung nicht erneut ausgeführt.

Nach der Erweiterung um die virtuelle Texteingabe bestanden
`cargo test --all-features -- --test-threads=1` mit
38 Bibliotheks-/CLI-Tests und 6 Prozess-Integrationstests, Clippy für alle Targets
und Features mit `-D warnings` sowie 24 Bibliothekstests ohne Default-Features.
`tests/ui.py` mit zwei gerenderten Sessions und `tests/slice.py` bestanden mit den
neu gebauten Anwendungen. Tests prüfen native Störeingaben ohne Fensterfokus,
unabhängige Buttonzustände, genau einmal zugestellte Resize-Ereignisse und die
unveränderte native Cursorposition einschließlich interner Präzision.
Die Keyboard-Abnahme prüft Press/Halten/Release in beiden Sessions; Unit-Tests prüfen
auch Press und Release vor demselben Tick sowie gemeinsam gehaltene logische Modifier.
Die Text-Abnahme prüft getrennte Unicode-Werte ohne Verarbeitung vor dem Tick.
Weitere Tests prüfen die exakte UTF-8-Grenze, leeren Text, ungültigen Fokus,
Fokuswechsel, Despawn ohne Umleitung und native Störeingaben.
Ein vorheriger paralleler Testlauf lief beim Server-Fristtest in den 30-Sekunden-
Timeout; die Ursache ist nicht belegt. Die anschließenden seriellen Läufe bestanden.
Der zusätzliche Context-Menu-Test prüft Öffnen, Ersetzen und Schließen ohne
widersprüchliches `menu_open`. Die übrigen Bevy-Testapps und `tests/shutdown.py`
wurden für diese Erweiterung nicht erneut geprüft.

Die Prüfungen vor diesen Erweiterungen umfassten 15 Bibliotheks-/CLI-Tests,
6 Prozess-Integrationstests und 7 fachliche Bevy-Anwendungstests. Beide realen
Abnahmeskripte bestanden, ebenso Feature-Builds, Root-Clippy mit `-D warnings`,
Formatprüfung und `git diff --check`. Das ist der Ausgangsnachweis dieser Übergabe,
kein Ersatz für Prüfungen nach weiteren Änderungen.

Build-, Start- und Testbefehle stehen in [slice.md](slice.md#nachweise).
Die Prozessverwaltung unterstützt derzeit Unix. Weitere Plattformen wären ein
eigener Portierungsblock, falls diese Unterstützung gewünscht wird.
Die bekannte macOS-Gatekeeper-Einschränkung ist
[dort dokumentiert](slice.md#noch-nicht-enthalten): generierte Testprogramme wurden
vereinzelt verzögert oder mit SIGKILL beendet, auch ohne woodpecker-Server.
Fehler diagnostizieren statt still wiederholen oder Sicherheitsregeln verändern.

## Arbeitsregeln und zurückgestellte Änderungen

- Bestehende Nutzeränderungen erhalten. Subagenten nur nach ausdrücklicher Zustimmung.
- Reversible Implementierungsdetails eigenständig entscheiden. Neue Produktentscheidungen
  und echte Konflikte mit dem Zielvertrag zur Entscheidung vorlegen.
- Gemeinsame fachliche Symbol-/Dateipräfixe durch passende Module und Hierarchien
  ausdrücken, statt den Präfix an jedem Namen zu wiederholen.
- Commits nur auf ausdrückliche Anforderung. Dann Conventional Commits, atomar,
  unabhängig baubar und testbar; Beschreibung kleingeschrieben, ohne Schlusspunkt,
  Header höchstens 100 Zeichen. Breaking Changes mit `!` und `BREAKING CHANGE:`.
  Issue-Verweise nur bei direktem Bezug im Footer; `Fixes`/`Closes` nur, wenn der
  Merge tatsächlich das offene Issue schließen soll.
- GitHub-URL und versionierte Report-Signaturbezeichner bleiben vorerst unverändert.
  Der Nutzer koordiniert diese Anpassung später mit einem anderen Projekt.
- Die Schlüssel-Authentisierung ist bewusst entfernt. Lokale Clients gelten als
  vertrauenswürdig; kein Ersatzschlüssel und keine optionale Auth-Schicht ergänzen.

Entfernte Mechanik bei Bedarf aus `5e9552e` lesen, beispielsweise
`git show 5e9552e:src/keyboard.rs`. Die historische Migrationstabelle aus
`de32d09:docs/api/migration.md` ist keine aktuelle Löschliste. Maßgeblich für die
heutige Zuordnung bleibt der bestehende Implementierungsplan.
