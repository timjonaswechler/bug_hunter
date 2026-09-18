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
   Screenshot, Recording und Replay sind umgesetzt. Diese Übergabe gehört zum
   Commit für Fehlerbeobachtung und Snapshot-Konstruktion auf `code_ownership`,
   aufbauend auf `82cd34b`.
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
- CLI, headless Zähleranwendung, Einzel-Session-Stopp, gemeinsame Serverfrist,
  SIGTERM und erstes/zweites Ctrl+C sind vorhanden.
- Der v2-Ausführungsweg ist entfernt: alte Host-/Driver-Fassade, Plugin- und
  Transport-APIs, Step/Clock, Automation-Marker, Controller-Beispiele und
  `failure.json`-Architektur. Der Handle samt Lebensdauertests wurde nach
  [handle](../../src/handle.rs) übernommen.
- Die Bevy-Szenen und ihre eigenen fachlichen Tests bleiben als native
  Anwendungen erhalten. `counter` und `context_menu` sind an den neuen Session-Weg
  angebunden. Die entfernte `automation`-Anbindung ist kein unterstützter Einstieg.
- Die Bibliothek ohne Features benötigt weder Server-/CLI- noch UI-/Renderer-
  Abhängigkeiten. Render-Abhängigkeiten der Testanwendungen gehören zu deren
  separatem Package.

Das ist weiterhin ein experimenteller v3-Teildurchstich, keine vollständige
Implementation des Zielvertrags. Insbesondere fehlen noch Report-Titel, Signatur,
Markdown und Provider; alte Funktionen werden nicht durch Kompatibilitäts-Exports angeboten.

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
- [ ] Titel, Markdown, Fehlerdaten und Signatur einschließlich Golden Vectors umsetzen.
- [ ] Zuerst den lokalen Provider, danach GitHub mit Duplikatsuche und lokalem
  Rückfall implementieren. Echte Veröffentlichung braucht ausdrückliche Freigabe.
- [ ] Clientunabhängige Report-Arbeit in Activity und die gemeinsame Serverfrist
  integrieren. Langsame Provider dürfen Event-Empfang und Pipe-Drain nicht blockieren.

Abschluss: Beobachtete Fehler ergeben Reports ohne `failure.json`-Zwischenformat
und ohne vorherigen Session-Abschluss. Provider-Fehler und ungewisse externe
Ausgänge bleiben sichtbar.

### 5. REPL, Script und Agent-Abnahme

- [ ] Die REPL während ausstehender Commands ansprechbar halten; gemeinsame
  Commands, Inspect-Kurzformen, Pending-Anzeige und Activity verwenden.
- [ ] Scripts vollständig vor Ausführung parsen und validieren; Ergebniszuordnung
  sowie die festgelegten Recording-/Shutdown-Barrieren umsetzen.
- [ ] EOF, Quit und Client-Trennung ohne versteckten Stop oder Shutdown prüfen.
- [ ] Den maschinenlesbaren CLI-Zugang mit einer vom Nutzer gestarteten externen
  Agent-Laufzeit erproben. Modellzugang und Agent-Werkzeugschleife gehören nicht hierher.

Abschluss: Menschliche und maschinelle Bedienung nutzen denselben Ausführungsweg,
auch bei parallelen Clients und Wiederverbindung.

### 6. Vollständige Systemabnahme

- [ ] Die erhaltenen Bevy-Szenen über den neuen Weg automatisieren: Fokus/Text,
  Drag-and-drop, Menüs, Timer, Layout, tote Handles und Bilder.
- [ ] Recording, Replay und Reports gemeinsam mit Client-Trennung, Activity-Lücken,
  Session-Ende und Server-Shutdown testen.
- [ ] Die vorläufige Activity-Grenze von 4 MiB mit echten großen Inspect-Outputs und
  Reports bemessen. Übergröße und verlorene Ergebnisse ausdrücklich behandeln.
- [ ] Öffentliche API, Capabilities, Fehlerformen und Dokumentation gegen den
  vollständigen Zielvertrag prüfen. Pro abgeschlossenem Bereich Migrationsstatus
  und tatsächlichen Nachweis aktualisieren.

## Konkreter nächster Durchstich

Entity-Inspect sowie Pointer-, Keyboard- und Text-Commands sind umgesetzt.
Die Reflection-Matrix aus Block 1 ist abgeschlossen. Dafür waren nur Tests und
Dokumentation nötig, keine Änderungen am produktiven Inspect-Verhalten.
Screenshot aus Block 2 ist ebenfalls umgesetzt und mit zwei gerenderten
Context-Menu-Sessions geprüft. Recording und Replay aus Block 3 sind implementiert.
Die gerenderte Replay-Abnahme besteht in vier erneuten Läufen ohne Codeänderung.
Die früheren schwarzen PNGs und die mögliche Display-Bedingung sind unten festgehalten.

Die Diagnoseerfassung und Snapshot-Konstruktion aus Block 4 sind umgesetzt.
`Report` enthält derzeit Failure und Context, noch keinen Titel, keine Signatur
und kein Markdown. Als Nächstes diese gemeinsame Report-Darstellung gemäß Zielvertrag
ergänzen, einschließlich Signatur-Golden-Vectors. Danach folgen Local, Github mit
Fallback und die automatische clientunabhängige Report-Arbeit im Server.
Aktuell werden Failure-Events weitergereicht, aber weder Reports automatisch
persistiert noch veröffentlicht. Es gibt weiterhin kein `failure.json`.
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
