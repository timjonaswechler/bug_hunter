# API-Migration

Dieses Dokument beschreibt den Weg vom Ist-Stand in [`current.md`](current.md) zur Zielstruktur aus
[`goal.rs`](goal.rs). Eine Zuordnung wird ergänzt, sobald der jeweilige Zielbereich ausreichend
definiert ist; ungeklärte Teile bleiben ausdrücklich `open`.

## Ausgangslage

Der Umbau ist kein sauberer "alt löschen, neu schreiben"-Schritt. Im Arbeitsbaum liegen bereits
neue Module neben angepassten Modulen und gelöschten alten Dateien. Deshalb werden drei Zustände
unterschieden:

| Status | Bedeutung |
| --- | --- |
| `keep` | Die bestehende Implementation bleibt fachlich und strukturell bestehen. |
| `add` | Für das Zielmodul gibt es keine entsprechende bestehende Implementation. |
| `move` | Die Verantwortung bleibt erhalten, wechselt aber den Owner. |
| `rewrite` | Das Ziel bleibt ähnlich, die Implementation wird neu geschrieben. |
| `replace` | Der alte Bereich wird durch einen neuen Bereich ersetzt. |
| `compat` | Nur ein vorübergehender Re-Export oder Alias für Nutzer des alten Namens. |
| `delete` | Kein Zielmodul; der alte Code wird entfernt. |
| `open` | Entscheidung oder Ziel-API ist noch nicht festgelegt. |
| `done` | Migration und Prüfung sind abgeschlossen. |

Ein Status beschreibt den Übergang, nicht die Qualität des alten Codes.

## Migrationsregeln

- Zuerst wird der Owner festgelegt. Erst danach werden Dateien verschoben oder gelöscht.
- Alte und neue Implementation dürfen nicht dauerhaft dieselbe fachliche Regel besitzen.
- Ein `compat`-Re-Export darf auf die neue Implementation zeigen, niemals auf eine Kopie.
- Jede abgeschlossene Zeile braucht mindestens einen Test oder einen manuellen Prüfpunkt.
- Wenn eine Zielentscheidung offen ist, bleibt der Code lokal und wird nicht weiter verteilt.
- Der Wire-Vertrag wird nicht nebenbei durch eine Modulverschiebung geändert. Breaking Changes
  werden als eigene Entscheidung festgehalten.

## Modulzuordnung

| Ist-Stand | Ziel | Änderung | Status | Prüfung |
| --- | --- | --- | --- | --- |
| [`handle`](../../src/handle/mod.rs) | `handle` | unverändert übernehmen | `keep` | bestehende Handle-Tests |
| [`command::input::keyboard`](../../src/command/input/keyboard.rs) | `command::input::keyboard` | Semantik übernehmen, erfolgreichen Output auf `()` reduzieren | `rewrite` | Keyboard- und Wire-Tests |
| [`command::input::pointer`](../../src/command/input/pointer.rs) | `command::input::pointer` | absolute und relative Bewegung trennen, erfolgreichen Output auf `()` reduzieren | `rewrite` | Pointer- und Wire-Tests |
| [`command::input::text`](../../src/command/input/text.rs) | `command::input::text` | aktuellen Fokus beibehalten, explizites Target ausschließen und erfolgreichen Output auf `()` reduzieren | `rewrite` | Text-, Fokus- und Wire-Tests |
| [`screenshot`](../../src/screenshot/mod.rs) | `command::screenshot` | Target-Auswahl und typisierten Output ergänzen, vorhandene PNG-Dateien ersetzen | `rewrite` | Target-, Pfad-, Overwrite-, Render- und No-Tick-Tests |
| [`command::tick`](../../src/command/tick.rs) | `command::tick` und `command::tick::warp` | synchrones `Step` durch die Warp-Commands ersetzen | `rewrite` | Command-Validierung, Clock, Wire-Fixtures und REPL |
| [`client::plugin`](../../src/client/plugin.rs) und [`session::driver`](../../src/session/driver.rs) | sessionweite Command-Verarbeitung | mehrere ausstehende Requests über interne Request-IDs verwalten, ohne die Command-Annahme zu blockieren | `rewrite` | REPL kann während eines laufenden Warp `set-pace` und `stop` senden |
| [`protocol`](../../src/protocol.rs) und Wire-Abbildung in [`command`](../../src/command/mod.rs) | `session::protocol` | den doppelten v2-Vertrag durch einen zentralen v3-Codec mit qualifizierten Command-Namen und nicht fatalen Protokollfehlermeldungen ersetzen | `replace` | Wire-Fixtures für Ready, alle Commands, ungeordnet eintreffende Responses, Ablehnungen und Protokollfehler |
| [`file_ref`](../../src/file_ref/*) | `-` | löschen | `delete` | ist zu wage  aufgebaut und vieles der aktuellen implementierung ist vermischt mit unterschiedlcihen Dateitypen. Wird während des generellen Refactoring festgestllt dass es mehrere Stellen gibt die die selben funktionallen gruppen hat kann man sich überlegen eine neue sinnvolle Struktur zu entwerfen. bis dahin wird `file_ref` entfernt. |
| [`session::recording`](../../src/session/recording/mod.rs) und Recording-Zustand in [`session::driver`](../../src/session/driver.rs) | `command::recording` und sessionweite Recording-Ausführung | öffentliche Start-/Stop-Commands ergänzen und die Aufzeichnung aller ausgeführten Session-Commands an einem gemeinsamen Ausführungspunkt neu aufbauen | `rewrite` | vollständige und abschnittsweise Aufzeichnung, Command-Ausschlüsse, interne Request-Korrelation ohne persistierte IDs, Flush, Pfade und No-Tick-Verhalten |
| [`session::replay`](../../src/session/replay/mod.rs) | `command::replay` und sessionweite Replay-Ausführung | Replay in der laufenden Controlled Session ausführen, Abweichungen sammeln und Commands mit ihrem gespeicherten Outcome statt mit benachbarten Recording-Einträgen verbinden | `rewrite` | wiederholtes Replay in derselben Session, Ausführung aus unterschiedlichen Ausgangszuständen, Stop, Blockierung, Command-Outcome-Zuordnung und No-Tick-Verhalten |
| [`failure`](../../src/failure/mod.rs) | `-` | `Detail`, `Kind` und das persistierte Zwischenformat `failure.json` entfernen; bestehende Nutzer verwenden die fachlichen Fehler- und Diagnosetypen ihrer jeweiligen Module | `delete` | keine `failure`-Exporte, `failure.json`-Zugriffe oder generischen `Detail`-Verwendungen verbleiben |
| [`report`](../../src/report/mod.rs) und Fehleranteile aus [`session::diagnostics`](../../src/session/diagnostics.rs) | `report` | einen providerunabhängigen Report direkt aus dem beobachteten Fehler und der laufenden Session erzeugen; den reportspezifischen Diagnosekontext, Titel, Fehlersignatur, Provider-Ausführung und Duplikaterkennung dort besitzen | `rewrite` | Panic, Prozessabbruch, optionaler Tracing-Error, reportspezifischer Context und History-Auszug sowie lokale und GitHub-seitige Duplikaterkennung |
| [`session::context`](../../src/session/context.rs) | `report::Context` und interner Zustand von `session::Session` | den eigenständigen Session-Context entfernen; `Report::create` stellt den benötigten Diagnosekontext bei der Report-Erzeugung direkt aus der Session zusammen | `replace` | kein `session::Context` oder doppelt gepflegter Session-Zustand; Report-Fixtures enthalten keine Wire-Request-IDs |
| [`session::controller`](../../src/session/controller.rs) | `session::Session`, `host::repl`, `host::agent`, `host::script` und `command::replay` | gemischte Controller-Orchestrierung nach allgemeiner Session-Ausführung und die konkreten Controller aufteilen; kein allgemeines Controller-Trait einführen | `replace` | REPL, Agent-Controller, Session Script und Replay verwenden denselben Session-Ausführungsweg |
| [`session::diagnostics`](../../src/session/diagnostics.rs) | interne Prozessbeobachtung von `session`, `session::history` und `report` | das Modul auflösen: Session liest stderr und beobachtet den Prozesslebenszyklus, die History behält Command-Outcomes und `report` erkennt und dokumentiert Laufzeit- und Logikfehler | `replace` | stderr-Weiterleitung, History-Grenze, Panic-Erkennung samt Backtrace, Vorrang vor Prozessabbruch und optionaler Tracing-Error |
| übrige Verantwortung aus [`session::driver`](../../src/session/driver.rs) | `session::Session` und interne Prozessverwaltung | Prozesslebenszyklus, Transport und ausstehende Commands hinter dem Session-Interface neu aufbauen | `rewrite` | Start, Ready, parallele Requests, Shutdown, Drop und unerwartetes Ende |
| [`session::launch`](../../src/session/launch.rs) | `session::launch` und `session::Config::launch` | Cargo-only-Launch als `launch::Config` behalten, ein Package und ein Binary oder Example ausdrücklich verlangen und die Prozess-Command-Erzeugung intern halten | `rewrite` | Binary und Example, leere Package- und Target-Namen, Features, Anwendungsargumente und exakt erzeugter Cargo-Aufruf |
| übrige Bevy-seitige Verantwortung aus [`client::plugin`](../../src/client/plugin.rs) | Controlled-Session-Integration unter `session` | Plugin, Command-Dispatcher und laufende Command-Arbeit aus dem irreführenden Modul `client` verschieben | `rewrite` | Bevy-Plugin, mehrere Requests, Render- und No-Tick-Verhalten |
| [`client::transport`](../../src/client/transport.rs) | interne Session-Ein-/Ausgabe | öffentliche Transporttypen entfernen; JSONL über stdin/stdout auf beiden Prozessseiten intern implementieren und In-Memory-Ein-/Ausgabe nur als interne Test-Seam behalten | `rewrite` | JSONL-Framing, Flush, Transportende sowie Controlled-Session- und Host-Tests mit interner In-Memory-Ein-/Ausgabe |
| [`host::command_line`](../../src/host/command_line.rs) | private CLI-Verarbeitung in `host::run` | das öffentliche Modul und seine CLI-Typen entfernen; Prozessargumente ausschließlich intern in den gewählten Host-Ablauf übersetzen | `replace` | REPL-, Agent-, Script- und Report-Aufrufe, Parsefehler sowie keine öffentlichen CLI-Exporte |
| [`host::config`](../../src/host/config.rs) | private Konfigurationsverarbeitung in `host::run` | öffentliche Config-Typen entfernen und den übergebenen versionierten TOML-Inhalt intern in `session::Config` übersetzen | `replace` | Version, unbekannte Felder, Session-Validierung, Artifact-Override und keine öffentlichen Config-Exporte |
| [`host::repl`](../../src/host/repl.rs) | `host::repl` | menschliche Eingaben in die gemeinsame JSON-Command-Form übersetzen und Session-Outcomes mit den von `Session` vergebenen Request-IDs anzeigen | `rewrite` | ungeordnet eintreffende Ergebnisse, Request-IDs, Pending-Liste, Recording und Replay innerhalb derselben Session, Quit, EOF und Ctrl-C |
| `-` | `host::agent` | einen interaktiven maschinenlesbaren Controller ergänzen, über den ein KI-Agent dieselbe laufende Session adaptiv steuert | `add` | JSONL-Fixtures, parallele Commands, ungeordnet eintreffende Ergebnisse, Recording und Replay innerhalb derselben Session, Session-Events und kontrollierter Abschluss |
| [`host::runner`](../../src/host/runner.rs) | `host::run` | das eigenständige Runner-Konzept durch einen direkten Host-Einstiegspunkt ersetzen; Konfiguration, CLI-Auswahl, Session-Lebenszyklus und gewählten Ablauf intern zusammensetzen | `replace` | REPL-, Agent-, Script- und Report-Aufruf, Capability-Prüfung, sauberer Shutdown und Exit-Codes ohne `process::exit` |
| [`host::script`](../../src/host/script.rs) | `host::script` | eine versionierte Liste derselben JSON-Command-Einträge vollständig validieren, in Dateireihenfolge asynchron einreichen und ihre Outcomes einsammeln | `rewrite` | Parse- und Validierungs-Fixtures, Eingangsreihenfolge, parallele Pending-Commands, ungeordnete Outcomes und Fehlerpositionen |
| [`host::mod`](../../src/host/mod.rs) | kleine `host`-Fassade | ausschließlich `host::run` unter dem Feature `host` exportieren; Config-, CLI-, Controller- und Report-Typen nicht re-exportieren | `rewrite` | Exporte mit und ohne Feature `host`; kein Feature-Alias `driver` |
| [`observe`](../../src/observe/mod.rs) | `command::inspect` | markergebundene und fachlich spezielle Beobachtungen durch allgemeine read-only Queries ersetzen | `replace` | Query-, Reflection-, Wire- und No-Tick-Tests |
| [`target.rs`](../../src/target.rs) | `-` | `AutomationTarget` und das öffentliche Target-Modul ersatzlos entfernen | `delete` | Inspektion findet Entities ohne Marker; keine Target-Referenzen verbleiben |

## Virtual Input

### Festgelegtes Ziel

- `command::input` gruppiert Keyboard-, Pointer- und Text-Commands.
- Ein erfolgreicher Input-Command bedeutet, dass die Controlled Session den Command angenommen,
  geprüft und für den nächsten ausgeführten Tick vorgemerkt hat. Erst dieser Tick stellt den Input
  den Anwendungssystemen bereit.
- Der erfolgreiche Output bestätigt nur diese Vormerkung. Er bestätigt nicht, dass
  Anwendungssysteme den Input bereits verarbeitet haben.
- Keyboard-, Pointer- und Text-Commands verwenden deshalb `Output = ()`. Den Anwendungszustand
  liest ein Controller bei Bedarf anschließend über `command::inspect`.
- Keyboard-Commands verwenden weiterhin den eigenen stabilen und layoutunabhängigen
  `keyboard::Key`-Typ. Jede unterstützte Variante besitzt ein festes Wire-Token und wird intern auf
  ein festes Paar aus Beveys physischem `KeyCode` und logischem `Key` abgebildet. Bevy-Typen gehören
  weder zum öffentlichen Command noch zum Wire-Vertrag.
- Pointer-Bewegungen werden als getrennte Varianten `MoveTo` und `MoveBy` modelliert.
- `pointer::Command::Scroll` verwendet ein zweidimensionales `delta` in Bevy-Zeileneinheiten. Eine
  Pixel-Einheit gehört nicht zum Ziel-Interface.
- Texteingabe verwendet immer das aktuell fokussierte editierbare Element. Der Text-Command nimmt
  kein explizites Target entgegen.

### Auswirkungen auf den Ist-Stand

- Die aktuellen Input-Responses enthalten Zustands-Snapshots für Pointer, Keyboard oder Text. Diese
  Daten entfallen aus dem erfolgreichen Command-Output und werden bei Bedarf über `inspect`
  abgefragt.
- Der vorhandene eigene Keyboard-Key-Typ, seine unterstützten Varianten, stabilen Wire-Tokens und
  die feste Bevy-Abbildung bleiben erhalten. Wie der v3-Codec ein unbekanntes Token intern bis zur
  Command-Ablehnung darstellt, ist kein Bestandteil des öffentlichen Key-Typs.
- `pointer::Command::Move` wird durch getrennte absolute und relative Bewegungs-Commands ersetzt.
- Die aktuelle Abbildung von Scroll-Commands auf Beveys `MouseScrollUnit::Line` bleibt erhalten; der
  Wire-Command erhält kein zusätzliches Einheitenfeld. Pointer- und Wire-Tests prüfen die
  Zeileneinheit.
- Die bestehende implizite Fokusauflösung der Texteingabe bleibt erhalten; es wird kein
  target-basierter Text-Command ergänzt.
- Änderungen am Wire-Format der Commands und Outputs werden vor der Implementation als eigene
  Protokollentscheidung festgehalten.

## Inspect

### Festgelegtes Ziel

- `command::inspect` ist ein ausschließlich lesender Query-Command gegen den aktuellen Bevy World
  der Controlled Session. Das Ziel besitzt keinen Inspect-`Set`-Command.
- Inspect führt keinen Simulationstick aus, schreibt die simulierte Zeit nicht fort und verändert
  den World nicht. Zustandsänderungen des geprüften Spiels entstehen ausschließlich über die dafür
  vorgesehenen Input- und Tick-Commands beziehungsweise durch die eigene Spiellogik.
- `Query` durchsucht zwei getrennte Kategorien: Entities samt Components und Resources. Resources
  werden nicht als künstliche Entity-Ergebnisse modelliert. Entity-Queries schließen Child-Entities
  ein; ausschließlich Beveys interne Resource-Entities werden aus dieser Kategorie ausgeschlossen.
- Resource-Queries verwenden einen expliziten `Selector` statt `type_path: Option<String>`.
  `Selector::All` liefert alle aktuell vorhandenen Resources, deren Typ für reflektierten
  Resource-Zugriff registriert ist. Registrierte, aber nicht vorhandene Resource-Typen erzeugen bei
  `All` kein Ergebnis. `Selector::Type { type_path }` fragt den angegebenen vollständigen Type
  Path ab.
- Entity-Queries können auf einen einzelnen sessionlokalen Entity-Handle begrenzt und durch
  Component-Anwesenheit mit `with` und `without` gefiltert werden. Feldvergleiche und eine frei
  formulierbare Bevy-Query-Syntax gehören zunächst nicht zum Interface.
- Entity-Projektionen liefern eine Zusammenfassung, Component-Namen, ausgewählte oder alle
  Component-Werte oder die Hierarchie. Resource-Queries verwenden die getrennten Projektionen
  `Metadata` und `Value`.
- `Metadata` liefert nur die Metadaten des jeweiligen Resource-Ergebnisses. Diese Projektion liest
  und serialisiert den reflektierten Wert nicht und gibt deshalb weder einen Wert noch einen
  Lesbarkeitsstatus aus.
- `Value` fordert den reflektierten Wert an. Jedes Ergebnis ist entweder `Readable { value }` oder
  `Unavailable { status }`; ein optionales JSON-Feld mit mehrfacher Bedeutung wird nicht verwendet.
- Vollständige kanonische Bevy-Type-Paths sind die Wire-Identität für Component- und
  Resource-Typen. Prozesslokale `TypeId`- und `ComponentId`-Werte werden nicht übertragen.
- Eine Query liefert alle passenden Elemente gemeinsam in einer Response. Das Inspect-Interface
  besitzt weder Pagination noch Cursor oder fachliche Größenlimits und kürzt Ergebnisse nicht.
- Nicht vorhandene, nicht registrierte, nicht reflektierbare oder nicht serialisierbare Werte
  brechen eine Value-Query nicht insgesamt ab. Das betroffene Element bleibt als `Unavailable` mit
  `Missing`, `NotRegistered`, `NotReflectable` oder `NotSerializable` sichtbar.
- Reflektierte Wertzugriffe hängen von der Registrierung der Anwendung ab. Fehlende Reflection
  schränkt die Lesbarkeit ein, erfordert aber keinen `AutomationTarget` oder anderen
  `bug_hunter`-Marker im untersuchten Spiel.

### Interne Verwendung von Bevy Remote

Für die Implementation gilt die beschlossene Variante 2: `command::inspect` behält sein eigenes
fachliches Interface und `session::protocol` behält den eigenen v3-Wire-Vertrag. Ein privater
Inspect-Adapter verwendet ausgewählte Funktionen aus `bevy_remote` ausschließlich innerhalb der
Controlled Session.

- Der Adapter darf geeignete öffentliche Built-in-Handler wie `world.query`,
  `world.get_components` und `world.get_resources` verwenden, um Bevys Type-Path-Auflösung,
  dynamische ECS-Queries und Reflection-Serialisierung nicht ein zweites Mal zu implementieren.
- `bug_hunter` installiert weder `RemotePlugin` noch `RemoteHttpPlugin`. Damit entstehen keine
  BRP-Mailbox, kein `RemoteLast`-Schedule, kein HTTP-Endpunkt und keine unbeabsichtigt erreichbaren
  BRP-Methoden für Spawn, Despawn, Insert, Remove, Reparenting, Events oder Messages.
- Die Implementation bindet `bevy_remote` direkt und ohne dessen HTTP-Default-Feature ein. Die
  genaue minimale Featurekombination und ihre Compile-Auswirkung werden vor der Implementation
  geprüft.
- BRP bleibt hinter einer privaten Seam. BRP-Requests, JSON-RPC-IDs, numerische Bevy-Entity-IDs,
  Responses und numerische Fehlercodes werden weder öffentlich exportiert noch über den
  Session-Transport gesendet oder in Recordings und Reports persistiert.
- Der Adapter übersetzt zwischen Bevy-Ergebnissen und den fachlichen Inspect-Typen. Er besitzt
  insbesondere die Abbildung von Bevy-`Entity` auf `handle::Handle`, die Trennung von Entities und
  Resources, den Ausschluss interner Resource-Entities sowie die Abbildung auf `Readable` und die
  vier `Unavailable`-Status.
- Die tolerante BRP-Query-Semantik wird nicht nach außen übernommen. Der Adapter validiert zuerst
  alle vom Aufrufer gelieferten Type Paths nach der strikten Inspect-Regel und ruft BRP erst nach
  erfolgreicher Validierung auf.
- Von BRP übersprungene oder nur geloggte Werte dürfen nicht aus einem Inspect-Ergebnis
  verschwinden, wenn das Ziel-Interface dafür einen `Unavailable`-Eintrag verlangt. In diesem Fall
  ergänzt der Adapter die fehlende Diagnose oder verwendet für diesen Teil einen eigenen Zugriff.
- BRPs Mutationshandler werden nicht verwendet. Die breiteren Stock-BRP-Methoden dürfen die
  read-only Grenze des Inspect-Interfaces nicht umgehen.
- Hierarchieprojektion und weitere nicht von BRP abgedeckte Regeln bleiben Teil der eigenen
  Inspect-Implementation hinter derselben Seam.
- Bei einem Bevy-Upgrade werden die verwendeten Built-in-Handler erneut gegen die Inspect-Fixtures
  geprüft. Eine geänderte BRP-Semantik ändert den Zielvertrag nicht automatisch.

Die technische Bewertung und die Gründe gegen BRP als Session-Protokoll stehen in
[`research/bevy-remote-as-session-protocol.md`](research/bevy-remote-as-session-protocol.md).

### Type-Path-Auflösung

- Jeder vom Aufrufer gelieferte Type Path wird vor der Ausführung exakt über Bevys `TypeRegistry`
  aufgelöst. Es gibt keine eigene Auflösung von Short Paths oder Aliasen. Ein String, den Bevy nicht
  eindeutig als vollständigen Type Path auflöst, gilt als unbekannt.
- Ein unbekannter Type Path lehnt den gesamten Query-Command als fachlichen Fehler ab. Es entstehen
  weder eine leere Ergebnismenge noch ein Teilergebnis oder ein `Unavailable::NotRegistered` für
  diese Eingabe.
- Diese strikte Vorabvalidierung gilt gleichermaßen für `entity.with`, `entity.without`, alle Einträge
  von `component::Selection::Listed` und `resource::Selector::Type`. Sind in einer Query mehrere Type
  Paths enthalten, müssen sie alle auflösbar sein, bevor die World gelesen wird.
- `Unavailable::NotRegistered` bleibt für Werte erhalten, die aus der World entdeckt oder über eine
  bereits gültige Identität adressiert wurden, deren benötigte Registrierung beim Zugriff aber
  fehlt. Der Status ist kein Ersatz für die Ablehnung eines unbekannten, vom Aufrufer gelieferten
  Type Paths.
- Die genaue stabile Fehlercodierung und Wire-Darstellung der Command-Ablehnung wird zusammen mit
  der vollständigen v3-Command-Abbildung festgelegt.

### Offene Fragen

- `OPEN`: Die kanonische JSON-Abbildung reflektierter Structs, Tupel, Enums, Maps, Sets, Handles und
  numerischer Sonderwerte muss vor der Wire-Festlegung für alle Value-Queries beschrieben werden.

### Auswirkungen auf den Ist-Stand

- `Command::Observe` und das Modul `observe` werden durch den read-only Command
  `Command::Inspect` und `command::inspect::query` ersetzt. Die vorhandene Reflection- und
  Hierarchie-Logik kann als technische Ausgangsbasis dienen, besitzt aber nicht das Ziel-Interface.
- `Selector::Targets`, `Selector::Ui`, `Selector::Pointers`, `Selector::VirtualInput` und
  `Selector::Clock` entfallen. UI- und Pointer-Entities werden über Component-Type-Paths gesucht;
  allgemeine Resources einschließlich entsprechend reflektierter und registrierter interner
  Input-/Clock-Resources werden über die Resource-Kategorie gelesen.
- `AutomationTarget`, [`target.rs`](../../src/target.rs) und dessen öffentlicher Re-Export entfallen
  vollständig. Die Anwendung muss keine Entities für `bug_hunter` markieren.
- Die bisherige Component-Projektion besitzt ein Größenlimit und kann beim ersten nicht
  reflektierbaren Wert abbrechen. Im Ziel entfallen Pagination und fachliche Größenlimits; ein
  nicht lesbarer Wert erhält einen Status und die übrige Query wird fortgesetzt.
- Der aktuelle Observe-Pfad ist bereits read-only. Das Ziel verallgemeinert seine Queries, führt
  aber keine mutable World-Seam, Reflect-Deserialisierung oder Change-Detection-Aktualisierung ein.
- Die Wire-Änderung ist breaking: Der bisherige Command mit `"type": "observe"` und den flachen
  Feldern `selector`, `projection`, `limit` und `cursor` entfällt. Inspect-Queries besitzen keine
  Nachfolger für `limit` und `cursor`; sie liefern alle passenden Elemente. Die genaue JSON-Form
  wird zusammen mit `session::protocol` festgelegt und erhält Wire-Fixtures für beide
  Query-Kategorien.
- Aufzeichnungen mit dem alten Observe-Wire-Command benötigen bei einer späteren
  Recording-Formatmigration entweder einen versionsbewussten Adapter oder werden als nicht mehr
  unterstützte Formatversion abgelehnt.

Die technische Grundlage und die Grenzen der Orientierung an `bevy_inspector_egui` stehen in
[`research/bevy-world-inspection-and-mutation.md`](research/bevy-world-inspection-and-mutation.md).

## Screenshot

### Festgelegtes Ziel

- Ein Screenshot führt keinen kontrollierten Simulationstick aus. Tick-Zähler und simulierte Zeit
  bleiben unverändert.
- Für die Aufnahme läuft weiterhin mindestens ein Bevy-Renderdurchlauf. Die Response bleibt
  ausstehend, bis der asynchrone GPU-Readback abgeschlossen und die PNG-Datei geschrieben wurde.
- `Capture { target: None, .. }` nimmt das einzige primäre gerenderte Fenster auf. Fehlt ein
  primäres Fenster oder existieren mehrere, wird der Command abgelehnt.
- `Capture { target: Some(handle), .. }` nimmt genau das durch den Handle bezeichnete Fenster auf.
  Ein unbekannter Handle oder eine Entity, die kein gerendertes Fenster ist, wird abgelehnt.
- Eine bereits vorhandene Datei am gewählten Dateiziel wird ersetzt.
- Der erfolgreiche Output wird erst zurückgegeben, nachdem die Aufnahme abgeschlossen und die
  PNG-Datei erfolgreich gespeichert wurde. Er enthält den relativen Pfad, Breite, Höhe und
  `overwritten`. `overwritten: true` bedeutet, dass eine zuvor vorhandene Datei ersetzt wurde.
- Ohne Renderer oder installierte Screenshot-Unterstützung wird der Command abgelehnt.

### Dateipfad und Überschreiben

- Der Controller gibt den vollständigen konkreten Dateipfad im `Capture`-Command an.
- Der Pfad ist relativ zum konfigurierten Session-Artifact-Verzeichnis. Dieses Verzeichnis darf
  außerhalb des Git-Projekts liegen; der Command darf es jedoch nicht verlassen.
- Der Debug Host oder die Einbettung bestimmt das Session-Artifact-Verzeichnis. Der Screenshot-
  Command bestimmt nur den Pfad darunter.
- Der Pfad muss ein normalisierter UTF-8-Pfad mit Vorwärtsschrägstrichen und der Endung `.png` sein.
  Absolute Pfade, leere Komponenten, `.` und `..`, Backslashes sowie Ausbrüche über symbolische
  Links werden abgelehnt.
- Die Controlled Session erzeugt keine automatischen Dateinamen und wertet keine Platzhalter für
  Datum, Uhrzeit oder Tick-Zähler aus. Ein Controller kann solche Namen vor dem Senden selbst
  erzeugen und übergibt anschließend den fertigen Pfad.
- Existiert die Datei bereits, wird sie ersetzt. Das ist kein Command-Fehler. Der Output meldet den
  Vorgang mit `overwritten: true`, damit der Controller bei Bedarf einen Hinweis anzeigen kann.

### Auswirkungen auf den Ist-Stand

- Der aktuelle Dateischreiber lehnt vorhandene Ziele ab und verwendet `create_new`. Er muss auf das
  beschlossene Ersetzen vorhandener Dateien umgestellt werden.
- Die bisherige erfolgreiche Response wird durch einen typisierten Screenshot-Output einschließlich
  `overwritten` ersetzt.
- Die bestehende Trennung zwischen kontrollierten Simulations-Schedules und Bevy-Rendering bleibt
  erhalten. Der Screenshot darf `command::tick` nicht implizit ausführen.

Die technische Prüfung für Bevy 0.19.1 steht in
[`research/bevy-screenshot-without-simulation-tick.md`](research/bevy-screenshot-without-simulation-tick.md).

## Tick-Warp

### Festgelegtes Ziel

- `command::tick::Command` gruppiert die Commands zur expliziten Ausführung von Ticks und enthält
  `Warp(command::tick::warp::Command)`.
- Ein Tick führt die von der Anwendung konfigurierten Bevy-Simulations-Schedules genau einmal in
  ihrer bestehenden Reihenfolge aus. `session::Plugin` übernimmt nur die Kontrolle darüber, wann
  diese Schedules laufen; es ersetzt weder ihre Systeme noch ihre Reihenfolge.
- Die Anwendung besitzt die Bedeutung eines Simulationsticks und ihre Zeitkonfiguration. Sie darf
  diese während der Session selbst verändern. `bug_hunter` konfiguriert und überschreibt keine
  simulierte Tick-Dauer.
- Nach einem erfolgreich ausgeführten Tick wird der Tick-Zähler der Session um eins erhöht.
- Ticks entstehen nur durch einen expliziten Tick-Command. Andere Commands und der normale
  Bevy-Event-Loop führen keine versteckten Simulationsticks aus.
- `command::tick::Config` enthält mit `pace` genau eine Einstellung. Sie bestimmt ausschließlich,
  wie die Warp-Ausführung in realer Zeit getaktet wird.
- `warp::Pace::AsFastAsPossible` fügt zwischen den Ticks keine absichtliche Wartezeit ein.
- `warp::Pace::TicksPerSecond { target }` beschreibt die angestrebte Anzahl ausgeführter Ticks pro
  realer Sekunde. `target` muss endlich und größer als null sein und besitzt kein festes
  öffentliches Maximum. Der Wert ist ein Ziel und keine Garantie, dass die Maschine ihn erreicht.
- `warp::Command::Start { ticks, pace }` startet einen begrenzten Warp. `ticks` muss größer als null
  sein und besitzt neben dem Wertebereich von `u64` kein zusätzliches fachliches Maximum.
- `Start { pace: None, .. }` verwendet `command::tick::Config::pace`. Eine mit `Some(pace)`
  übergebene Pace gilt nur für diesen Warp. Danach gilt wieder die konfigurierte Pace.
- `warp::Command::SetPace { pace }` ändert die konfigurierte Pace. Bei einem laufenden Warp wird sie
  sofort auf die noch ausstehenden Ticks angewendet. Auch `AsFastAsPossible` kann dadurch erneut
  als Standard gesetzt werden.
- `warp::Command::Stop` beendet einen laufenden Warp. Ohne laufenden Warp ist `Stop` ein
  erfolgreicher No-op.
- Ein großer Warp bleibt durch `Stop` abbrechbar.

Die REPL bildet diese Commands so ab:

```text
tick warp <ticks>
    -> Warp(Start { ticks, pace: None })

tick warp <ticks> pace max
    -> Warp(Start { ticks, pace: Some(AsFastAsPossible) })

tick warp <ticks> pace <ticks-per-second>
    -> Warp(Start { ticks, pace: Some(TicksPerSecond { target }) })

tick warp pace max
    -> Warp(SetPace { pace: AsFastAsPossible })

tick warp pace <ticks-per-second>
    -> Warp(SetPace { pace: TicksPerSecond { target } })

tick warp stop
    -> Warp(Stop)
```

### Response- und Zustandsregeln

- Die Response von `Warp(Start { .. })` bleibt ausstehend, bis der Warp natürlich endet oder durch
  `Stop` beendet wird.
- Die Start-Response enthält `requested_ticks`, `executed_ticks` und `outcome`. `outcome` ist
  entweder `completed` oder `stopped`.
- Die REPL wartet nicht blockierend auf diese Response. Sie zeigt weiter den Prompt und ordnet die
  spätere Response über die nur während der Transportverbindung verwendete Request-ID zu.
- `SetPace` antwortet mit `PaceChanged { pace }`, nachdem die konfigurierte und gegebenenfalls
  aktive Pace übernommen wurde.
- `Stop` antwortet mit `stopped: true`, wenn ein Warp beendet wurde, sonst mit `stopped: false`.
- Ein zweiter `Start` während eines laufenden Warp wird mit `warp_already_running` abgelehnt. Er
  ersetzt den laufenden Warp nicht implizit.

### Warp-Pace und simulierte Zeit

Die Pace steuert nur die reale Ausführungsgeschwindigkeit des Warps. Sie verändert weder die
Zeitkonfiguration der Anwendung noch die Bedeutung eines Simulationsticks. Bei gleichem
Ausgangszustand und deterministischer Anwendungslogik führt derselbe Warp deshalb unabhängig von der
Pace dieselbe Anzahl Anwendungsticks aus und soll denselben fachlichen Zustand erzeugen.

Auch mit `AsFastAsPossible` erhält die Warp-Ausführung nur ein begrenztes Arbeitsbudget pro
Event-Loop-Durchlauf. Nach diesem Budget verarbeitet die Session erneut eingegangene Commands,
bevor sie weitere Warp-Ticks ausführt. Die Größe dieses Arbeitsbudgets ist weder ein Protokollwert
noch eine Pace-Einstellung.

### Auswirkungen auf den Ist-Stand

- `command::tick::Command::Step { frames, step_nanoseconds }` entfällt zugunsten der Warp-Commands.
  Die Controlled Session übernimmt die Ausführungssteuerung der Anwendungsschedules, erhält aber
  keine simulierte Tick-Dauer mehr vom Host. Die bisherige manuelle Dauer pro Step wird entfernt.
- Der aktuelle synchrone Host-Aufruf `session::driver::Session::request` reicht für überlappende
  Requests noch nicht aus und muss bei der Session-Migration geprüft werden.
- Wire-Format, Recordings, Replay, REPL, Beispiele und Tests müssen auf die neue Command-Form
  umgestellt werden.
- Die Zielentscheidung weicht von ADR-0003 ab. Vor der Implementation muss eine neue ADR festhalten,
  welche Teile von ADR-0003 ersetzt werden und welche Regeln, etwa keine versteckten Ticks, bestehen
  bleiben.


## Session Recording

### Festgelegtes Ziel

- `command::recording::Command::Start { path }` startet eine Session Recording am aktuellen
  Zustand der laufenden Controlled Session.
- `command::recording::Command::Stop` beendet und persistiert die aktive Aufzeichnung. Die Controlled
  Session bleibt aktiv.
- Der Debug Host kann `Start` direkt nach dem Start einer Controlled Session ausführen und dadurch
  alle nachfolgenden Session-Commands aufzeichnen. Dasselbe Interface erlaubt einen beliebigen
  späteren Abschnitt.
- Eine Session Recording ist eine geordnete Folge ausgeführter Session-Commands und ihrer
  korrelierten Ergebnisse. Command und Outcome werden als eine fachliche Ausführung erhalten; die
  dafür während der Ausführung verwendete Wire-Request-ID wird nicht aufgezeichnet. Die Recording
  enthält keinen Bevy-World-Snapshot und keine Zusage über den Zustand zu Beginn oder am Ende.
- Der Aufrufer entscheidet, ob eine Aufzeichnung eine Vorbereitung, eine Messung oder eine gesamte
  Sitzung beschreibt. Dafür werden keine unterschiedlichen Recording-Typen eingeführt.
- Recording-Start und -Stop werden vom Debug Host ausgeführt und nicht über das Wire-Protokoll an
  die Controlled Session gesendet.
- Recording- und Replay-Steuercommands werden nicht aufgezeichnet. Wird während einer aktiven
  Aufzeichnung ein Replay ausgeführt, zeichnet der Recorder stattdessen die vom Replay tatsächlich
  ausgeführten Session-Commands und ihre Ergebnisse auf. Die neue Aufzeichnung bleibt dadurch
  flach und verweist nicht rekursiv auf das gestartete Replay.
- Recording-Start und -Stop führen keinen Simulationstick aus und verändern den Zustand der
  Controlled Session nicht.
- Recording kann nur an einer Grenze ohne ausstehende Requests gestartet oder gestoppt werden. Ein
  laufender Warp muss zuvor abgeschlossen oder ausdrücklich gestoppt werden. Recording stoppt ihn
  nicht automatisch.
- Während Requests ausstehen, ordnet der Debug Host Responses über die Wire-Request-ID zu. Vor dem
  Schreiben verbindet der Recorder den Session-Command mit seinem Outcome und verwirft die ID. Das
  Recording-Format verwendet die Wire-Request-ID weder als Identität noch als Reihenfolge.
- `Start` antwortet, sobald der Recorder aktiv ist. `Stop` antwortet erst, nachdem der letzte
  Recording-Eintrag und der Abschluss dauerhaft geschrieben wurden.

### Auswirkungen auf den Ist-Stand

- Die vorhandene hostseitige Verantwortung für Recording bleibt erhalten. Sie wird jedoch an die
  neue sessionweite Command-Ausführung angepasst und nicht als Guest-Funktion implementiert.
- Das aktuelle Format beginnt jeden Abschnitt mit `SessionStarted`. Das neue Format muss zwischen
  dem Start einer Controlled Session und dem Start eines Recording-Abschnitts unterscheiden, ohne
  daraus eine Zustandswiederherstellung abzuleiten.
- `ControllerAction`, Response, Observation, Fehler und Artefakt werden nicht mehr als lose
  benachbarte Ereignisse verbunden. Der gemeinsame Ausführungspunkt korreliert sie zunächst intern
  und schreibt anschließend einen Session-Command mit seinem Outcome ohne Wire-Request-ID.
- Änderungen am Recording-Format erhalten eine eigene Formatversion. Sie sind keine Wire-Änderung
  zwischen Debug Host und Controlled Session.

## Session Replay

### Festgelegtes Ziel

- `command::replay::Command::Start { path }` spielt eine gültige Session Recording in der aktuell
  laufenden Controlled Session ab.
- Replay startet keine neue Controlled Session, setzt keinen Zustand zurück und prüft vor dem Start
  nicht, ob der aktuelle Zustand dem ursprünglichen Ausgangszustand entspricht.
- Jede gültige und unterstützte Session Recording kann als Replay gestartet werden. Ob ihre Commands
  aus dem aktuellen Zustand dieselben Ergebnisse erzeugen, ist Gegenstand des Replays und keine
  Vorbedingung.
- Dieselbe oder eine andere Aufzeichnung kann nach Abschluss erneut in derselben Controlled Session
  gestartet werden. Der Aufrufer besitzt die Verantwortung für den dabei vorhandenen Zustand.
- Replay führt die aufgezeichneten Commands und ausschließlich deren explizite Ticks aus. Laden,
  Vergleichen, Start, Stop und der Wechsel zwischen Recording-Einträgen erzeugen keine Ticks.
- Ein Unterschied zwischen aufgezeichnetem und aktuellem Ergebnis stoppt das Replay nicht. Replay
  versucht weitere Commands auszuführen, solange die Controlled Session weitere Commands annehmen
  kann.
- Replay endet mit `completed`, wenn das Ende der Aufzeichnung erreicht wurde. Erkannte
  Abweichungen ändern dieses Ausführungs-Outcome nicht.
- Replay endet mit `blocked`, wenn kein weiterer aufgezeichneter Command ausgeführt werden kann,
  beispielsweise nach einem unerwarteten Session-Ende oder einem nicht fortsetzbaren
  Kommunikationsfehler.
- `Replay::Stop` beendet die weitere Replay-Ausführung. Es setzt den bereits veränderten Zustand
  nicht zurück und führt keinen ausgleichenden Command aus.
- Die Response von `Replay::Start` bleibt bis `completed`, `stopped` oder `blocked` ausstehend.
  `Replay::Stop` ist währenddessen zulässig. Ohne laufendes Replay ist `Stop` ein erfolgreicher
  No-op.
- Replay-Commands werden vom Debug Host verarbeitet. Die darin enthaltenen aufgezeichneten
  Session-Commands laufen über denselben Ausführungsweg wie Commands eines menschlichen, agentischen
  oder geskripteten Controllers.
- Die genaue Darstellung und Veröffentlichung erkannter Abweichungen bleibt bis zur Planung des
  Reporting-Interfaces offen. Diese offene Darstellung ändert nicht die Regel, dass Replay nach
  einer Abweichung soweit wie möglich fortfährt.

### Auswirkungen auf den Ist-Stand

- Replay erzeugt keine frische Controlled Session. Die bestehende Host-Orchestrierung wird so
  getrennt, dass der Aufrufer die aktuelle Session nach einem Replay weiter verwenden kann.
- Die aktuelle Zuordnung erwarteter Ergebnisse anhand des jeweils nächsten `ControllerAction`-
  Eintrags wird durch Recording-Einträge ersetzt, die Command und erwartetes Outcome gemeinsam
  besitzen.
- Persistierte Commands werden über einen versionsbewussten Recording-Adapter in die aktuelle
  Command-Form überführt. Das Recording-Format und das Wire-Format bleiben getrennte Verträge.
- Die Ausführung muss Abweichungen von Fehlern unterscheiden, die eine weitere Command-Ausführung
  technisch verhindern. Nur Letztere führen zu `blocked`.

## Report

### Festgelegtes Ziel

- Ein Report dokumentiert genau einen während einer Controlled Session beobachteten Laufzeit- oder
  Logikfehler.
- Der Debug Host stellt den Report aus beobachtbaren Informationen zusammen. Die Bevy-Anwendung muss
  keine reportspezifischen Traits, Fehlercodes oder Meldeaufrufe implementieren.
- Die Session liest stderr und beobachtet den Prozessstatus, interpretiert diese Daten aber nicht als
  fachliche Fehler. `report` erkennt daraus Panic, aktivierte Tracing-Errors oder ein unerwartetes
  Prozessende und besitzt die Vorrangregel zwischen ihnen.
- Normale Rust-Assertions und Panics können Logiklücken sichtbar machen. Schlägt beispielsweise
  `assert!` fehl, verwendet der Debug Host die daraus entstehende Panic-Ausgabe als Report-Auslöser.
  Assertions bleiben normale Anwendungslogik und kennen `bug_hunter` nicht.
- Ein Report besitzt einen menschenlesbaren Titel, die unveränderte beobachtete Fehlermeldung, deren
  Ursprung, eine normalisierte Fehlersignatur und einen reportspezifischen Diagnosekontext.
- `Report::create` erhält den beobachteten Fehler und eine Referenz auf die laufende
  `session::Session`. Es leitet Titel und Fehlersignatur aus dem Fehler ab und stellt den
  reportspezifischen Diagnosekontext direkt aus der Session zusammen. Der Aufrufer liefert weder
  eine vorbereitete Signatur noch einen vorbereiteten Diagnosekontext.
- Für einen Panic werden eine bekannte Code-Stelle und die beobachtete Backtrace-Ausgabe aufgenommen.
  Beide dürfen fehlen, ohne die Report-Erzeugung zu verhindern.
- Der Kontext enthält höchstens die letzten 50 gesendeten Commands in Sendereihenfolge samt ihrem
  korrelierten Outcome. Wire-Request-IDs werden nicht in den Report übernommen. Ein Command ohne
  eingetroffene Response bleibt als unbeantwortet sichtbar.
- Die Command-Historie dient als Reproduktionskontext. Sie garantiert nicht, dass sich der Fehler
  ohne passenden Ausgangszustand reproduzieren lässt.
- Ein erkannter Panic oder ein unerwartetes Prozessende löst unabhängig von der
  Report-Konfiguration einen Report aus. Wird bei einem Prozessende bereits ein Panic erkannt,
  beschreibt der Report den Panic und nicht zusätzlich einen zweiten Prozessfehler.
- Eine Command-Ablehnung löst nicht automatisch einen Report aus. Sie kann durch einen unpassenden
  Command oder Ausgangszustand des Controllers verursacht worden sein.
- Die Session-Konfiguration besitzt die standardmäßig deaktivierte Einstellung
  `report.tracing_errors`. Bei Aktivierung behandelt der Debug Host erkannte formatierte
  `tracing`-Events auf Error-Level als Report-Auslöser.
- Eine beliebige Textausgabe auf `stderr`, die nur das Wort `error` enthält, gilt nicht als
  `tracing`-Event.
- Die Session-Konfiguration legt ein relatives Report-Ausgabeverzeichnis innerhalb ihres
  Session-Artifact-Verzeichnisses und genau einen Provider fest. Ein Report darf dieses
  Ausgabeverzeichnis nicht verlassen.
- Jeder Provider besitzt nur seine eigenen zusätzlichen Konfigurationswerte. Zunächst sind ein
  lokaler Markdown-Provider und ein GitHub-Provider vorgesehen.
- `report::submit` übergibt einen bereits zusammengestellten Report an den konfigurierten Provider.
  Der Provider arbeitet ausschließlich innerhalb des Session-Artifact-Verzeichnisses und gibt ein
  gemeinsames `provider::Outcome` mit der erstellten oder bereits vorhandenen Referenz zurück.
- Der lokale Provider schreibt jeden neuen Report in eine eigene Markdown-Datei. Der
  GitHub-Provider kann unter demselben Ausgabeverzeichnis einen providerspezifischen Entwurf
  ablegen.
- Provider verwenden die Fehlersignatur zur Duplikaterkennung. Ist derselbe Fehler bereits beim
  jeweiligen Provider dokumentiert, wird kein zweiter Bericht angelegt; das Outcome verweist auf
  die vorhandene Datei oder das vorhandene Issue.
- Eine vom Provider vergebene Kennung wie eine GitHub-Issue-Nummer entsteht erst bei der
  Veröffentlichung. Sie ist kein von der Anwendung oder Session-Konfiguration gelieferter
  Fehlercode.
- Bildvergleiche und andere visuelle Abweichungen gehören zunächst nicht zum Reporting-Interface.

### Fehlersignatur

- Die Fehlersignatur wird vom Debug Host aus den beobachteten Fehlerdaten abgeleitet. Der Nutzer
  konfiguriert weder Fehlercodes noch Regeln für einzelne Fehlermeldungen.
- Bei einem Panic berücksichtigt sie die Panic-Meldung, die bekannte Code-Stelle und stabile Teile
  des Backtraces.
- Bei einem aktivierten `tracing`-Error berücksichtigt sie mindestens das Tracing-Target und die
  Meldung. Bei einem unerwarteten Prozessende ohne erkannten Panic berücksichtigt sie den
  Prozessstatus und die verfügbare Fehlerausgabe.
- Flüchtige Speicheradressen, absolute Projektpräfixe, Session, Controller und Command-Verlauf
  dürfen die Signatur desselben wiederholt auftretenden Fehlers nicht verändern.
- Der Report bewahrt die ursprüngliche Meldung und Backtrace-Ausgabe unabhängig von dieser
  Normalisierung als Diagnose auf.

### Grenzen der nicht invasiven Erkennung

- Ein intern behandeltes `Result::Err`, das weder einen Panic noch einen aktivierten
  `tracing`-Error oder ein unerwartetes Prozessende auslöst, kann der Debug Host nicht erkennen.
- Eine Logiklücke wird automatisch erkannt, wenn sie eine normale Rust-Assertion oder einen anderen
  Panic auslöst. Ohne Panic, aktivierten `tracing`-Error oder unerwartetes Prozessende entsteht
  zunächst kein automatischer Report.
- `debug_assert!` ist in Builds ohne aktivierte Debug-Assertions nicht vorhanden und kann dort
  keinen Report auslösen.
- Die Anwendung darf Assertions ergänzen, um eigene Invarianten zu prüfen. Das ist keine
  reportspezifische Integration und keine Voraussetzung dafür, dass vorhandene Panics erfasst
  werden.

### Offene Fragen und technische Prüfungen

- `OPEN`: Die genaue Normalisierung, Berechnung und persistierte Darstellung der Fehlersignatur ist
  festzulegen.
- `OPEN`: Die für Diagnose und Reproduktion gespeicherten Anwendungs- und Session-Angaben von
  `report::Context` sind festzulegen.
- `OPEN`: Die notwendigen GitHub-spezifischen Repository- und Veröffentlichungseinstellungen sowie
  das genaue Interface für Duplikatsuche, Entwurf und Veröffentlichung sind festzulegen.
- `OPEN`: Die genaue Liste der Provider- und Persistenzfehler von `report::Error` ist mit der
  Implementation festzulegen.
- Vor der Implementation ist zu untersuchen, wie sich Panics in Bevy-Systemen, asynchronen Tasks und
  Worker-Threads mit den unterstützten Panic-Strategien verhalten und welche Meldung der Debug Host
  zuverlässig beobachten kann.
- Ebenfalls zu untersuchen ist, wie der Debug Host beim Start der Controlled Session einen Backtrace
  anfordert und welche Teile davon zwischen Builds stabil normalisiert werden können.
- Für `report.tracing_errors` ist zu untersuchen, welche Bevy-`tracing`-Ausgabe der Debug Host ohne
  Änderungen an der Spielanwendung zuverlässig als Error-Level-Event erkennen kann. Dazu gehören
  benutzerdefinierte Formatter, ANSI-Ausgabe und mehrzeilige Meldungen.

### Auswirkungen auf den Ist-Stand

- Das öffentliche Modul `failure` entfällt vollständig. Seine generischen Typen werden nicht unter
  `report` nachgebaut: Command-Ablehnungen, Protokolldiagnosen und Session-Fehler behalten ihre
  jeweiligen fachlichen Typen; nur beobachtete Laufzeit- und Logikfehler werden zu
  `report::Failure`.
- Das persistierte Zwischenformat `failure.json` sowie `report::Report::load` entfallen. Die aktuelle
  Report-Erzeugung liest Recordings, Replay-Ergebnisse und mehrere fest benannte Dateien
  nachträglich aus einem Artifact-Verzeichnis. Im Ziel erhält `Report::create` beim erkannten Fehler
  eine Referenz auf die laufende Session und stellt daraus den benötigten Diagnosekontext zusammen.
- `session::diagnostics` entfällt als Zielmodul. `FailureHeadline`, `FailureReport`,
  `DiagnosticArtifacts`, `DiagnosticsError` und das eigenständige öffentliche Konzept `RecentLogs`
  werden nicht übernommen.
- Das Lesen und Weiterleiten von stderr sowie die Beobachtung des Kindprozesses wechseln in die
  interne Prozessverwaltung von `session::Session`. Ein für Panic- und Backtrace-Erkennung nötiger
  Zeilenpuffer bleibt ein privates Implementierungsdetail von `report`; `recent.log` wird nicht als
  separates Diagnoseartefakt fortgeführt.
- Die fortlaufende korrelierte Command-Historie bleibt Eigentum der Session. `report` erhält davon
  nur den für den Report begrenzten Snapshot.
- Die bestehende GitHub-Duplikatsuche vergleicht exakte Titel. Im Ziel verwendet sie die
  normalisierte Fehlersignatur, damit Titeländerungen und gleiche Titel verschiedener Fehler die
  Zuordnung nicht bestimmen.
- Das aktuelle Diagnoseformat besitzt keinen normalisierten Panic-Backtrace als fachlichen Teil der
  Fehlerbeschreibung. Die neue Report-Erzeugung muss Panic-Ausgabe, Code-Stelle und Backtrace
  gemeinsam erfassen, ohne flüchtige Backtrace-Daten zur Duplikatidentität zu machen.
- Die vorhandenen Console-, JSON- und GitHub-Darstellungen dürfen als technische Ausgangsbasis
  dienen. Sie bestimmen nicht die Struktur des providerunabhängigen Reports.

## Session Protocol

### Festgelegtes Ziel

- `session::protocol` besitzt den versionierten Wire-Vertrag zwischen Debug Host und Controlled
  Session. Die fachlichen Commands bleiben im jeweiligen `command`-Modul; das Protokoll besitzt nur
  deren Wire-Abbildung, die Nachrichtenhüllen und die Request-Korrelation.
- Bevy Remote und JSON-RPC gehören nicht zum Wire-Vertrag. Der interne Inspect-Adapter wird nicht
  durchgereicht: Die Wire-Nachrichten enthalten weder `jsonrpc`, BRP-Methodennamen und `params` noch
  BRP-IDs oder numerische BRP-Fehlercodes. `session::protocol` bleibt der einzige Owner der
  transportierten Request-ID, Command-Namen, Outputs und Fehlerformen.
- Das Ziel ist Protokollversion 3 und nicht kompatibel mit dem aktuellen Protokoll v2. Der Debug Host
  sendet keine Requests, bevor er genau eine `Ready`-Nachricht mit der erwarteten Version erhalten
  hat.
- Input, Tick und Inspect gehören fest zur Protokollversion. `Ready` meldet nur Funktionen, deren
  Unterstützung von der konkreten Bevy-Zusammensetzung der Controlled Session abhängt. Zunächst ist
  das die Screenshot-Unterstützung.
- Eine Capability beschreibt, dass die optionale Funktion beim Ready-Handshake vollständig
  installiert und grundsätzlich ausführbar ist. Sie ist weder eine Berechtigung noch eine Aussage,
  dass jeder konkrete Command im aktuellen World-Zustand erfolgreich sein wird.
- Die Controlled Session bildet ihre Bevy-Plugins und Ressourcen auf fachliche Capabilities ab. Sie
  überträgt keine Bevy-Plugin-Namen. `screenshot: true` setzt die vollständige Screenshot-
  Integration einschließlich Renderer voraus; die bloße Existenz eines einzelnen Render-Plugins
  reicht nicht.
- Pointer, Keyboard und Text benötigen keinen Renderer und werden nicht als optionale Capabilities
  gemeldet. Fehlende Fenster, Pointer-Positionen oder Eingabefokusse führen zu fachlichen Command-
  Ablehnungen.
- Nur Input, Tick, Inspect, Screenshot und Shutdown überschreiten den Transport zur Controlled
  Session. Recording und Replay werden vom Debug Host ausgeführt und sind keine Wire-Commands.
- Rust verwendet typisierte Command-Enums. Der Protokoll-Codec bildet jeden konkreten Command auf
  einen vollständig qualifizierten Namen wie `input.keyboard.press`, `tick.warp.stop` oder
  `inspect.query` und ein immer vorhandenes `arguments`-Objekt ab.
- `Session` vergibt für jeden angenommenen Command eine numerische Request-ID. Ein Controller
  liefert keine eigene ID. Für einen Wire-Command verwendet das Protokoll dieselbe ID; für einen
  hostseitigen Recording- oder Replay-Command bleibt sie innerhalb des Debug Hosts.
- Eine Request-ID ist innerhalb der laufenden Session eindeutig und dient ausschließlich dazu, ein
  möglicherweise später oder in anderer Reihenfolge eintreffendes Outcome dem Command zuzuordnen.
  Aus ihrem Wert wird weder eine fachliche Reihenfolge noch eine persistente Session-Identität
  abgeleitet.
- Request-IDs werden nicht in Session Recordings, Reports oder anderen dauerhaften Artefakten
  gespeichert. Der Recorder verwendet sie nur vorübergehend, um Command und Outcome zu verbinden.
  Das ist insbesondere für Aufzeichnungen wichtig, die erst während einer laufenden Controlled
  Session beginnen.
- Requests werden in Eingangsreihenfolge angenommen und gestartet. Responses dürfen in anderer
  Reihenfolge eintreffen. Jeder angenommene Request erzeugt genau eine Response mit derselben ID und
  dem vollständig qualifizierten Namen des beantworteten Commands. Stimmt dieser Name nicht mit dem
  unter der ID ausstehenden Command überein, behandelt der Host die Response als Protokollfehler für
  diesen Request.
- Eine erfolgreiche Response enthält immer `output`; für `Output = ()` ist der Wert `null`. Eine
  abgelehnte Response enthält stattdessen einen stabilen Fehlercode und eine menschenlesbare
  Meldung. Die Rust-Varianten schließen eine gleichzeitige Erfolgs- und Fehlernutzlast aus.
- Eine nicht angenommene oder nicht zuordenbare Protokollnachricht erzeugt eine
  `protocol_error`-Nachricht. Wenn eine gültige Request-ID gelesen werden konnte, enthält die
  Meldung diese ID, andernfalls `null`. Die Controlled Session verarbeitet danach weitere
  JSONL-Zeilen; eine Protokollverletzung beendet sie nicht automatisch.
- Ein Request mit gültiger, derzeit unbenutzter ID, aber ungültigem Command wird als angenommener
  Request mit einer abgelehnten Response beantwortet. Eine bereits einem ausstehenden Request
  zugeordnete ID oder eine Nachricht ohne lesbare ID wird nicht angenommen und erzeugt stattdessen
  `protocol_error`.
- Ein tatsächlicher Transportabbruch oder das Ende des Controlled-Session-Prozesses kann nicht durch
  eine Protokollmeldung geheilt werden und beendet die Verbindung weiterhin.
- `stdout` enthält ausschließlich UTF-8-JSONL-Protokollnachrichten. Diagnoseausgaben der Controlled
  Session werden getrennt über `stderr` beobachtet.

Die Wire-Form folgt diesem Muster:

```json
{"request_id":17,"command":"tick.warp.stop","arguments":{}}
{"request_id":17,"command":"tick.warp.stop","status":"completed","output":null}
```

Eine Command-Ablehnung verwendet dieselbe Request-ID:

```json
{"request_id":17,"command":"tick.warp.stop","status":"rejected","error":{"code":"warp_not_running","message":"no tick warp is running"}}
```

Eine nicht zuordenbare Nachricht wird getrennt gemeldet:

```json
{"request_id":null,"status":"protocol_error","error":{"code":"malformed_request","message":"request ID is missing"}}
```

### Auswirkungen auf den Ist-Stand

- [`protocol.rs`](../../src/protocol.rs) und [`command/mod.rs`](../../src/command/mod.rs) besitzen
  derzeit überlappende Wire-Command- und Request-Typen. `session::protocol` ersetzt diese doppelte
  Verantwortung und wird der einzige Owner der JSON-Abbildung.
- Das aktuelle `Ready` verwendet String-Listen für `controls` und `observation_scopes`. Version 3
  ersetzt sie durch die fest durch die Version vorgegebenen Commands und eine kleine typisierte
  Capability-Struktur für tatsächlich optionale Laufzeitunterstützung.
- Das aktuelle Protokoll verwendet verschachtelte und je Command unterschiedlich abgeflachte
  `type`- und `action`-Objekte. Version 3 verwendet qualifizierte Command-Namen, ein einheitliches
  `arguments`-Objekt und statusbasierte Response- beziehungsweise Protokollfehlermeldungen ohne
  zusätzliches `type`-Feld.
- Der aktuelle Decoder verwendet `sequence: 0`, wenn keine ID gelesen werden kann, und die
  Controlled Session verlangt lückenlos aufsteigende Sequenzen. Im Ziel gibt es keine künstliche
  Ersatz-ID. `Session` vergibt IDs für alle host- und wireseitigen Commands, verwendet eine ID
  innerhalb derselben Session nicht erneut und darf dadurch auf dem Wire Lücken erzeugen. Die
  Controlled Session leitet aus dem Zahlenwert keine Eingangsreihenfolge ab.
- Der aktuelle Host wartet nach jedem gesendeten Request unmittelbar auf genau dessen Response. Die
  neue Session-Ausführung hält mehrere Requests gleichzeitig offen und bewahrt früher eintreffende
  Responses auf, bis der jeweilige Aufrufer sie abholt.
- Wire-Fixtures prüfen Ready, sämtliche Command-Namen und Argumentformen, erfolgreiche und
  abgelehnte Responses einschließlich ihres Command-Namens, vertauschte Response-Reihenfolgen,
  doppelte IDs sowie Protokollfehler mit und ohne lesbare Request-ID.

## Session

### Sessionweite Command-Verarbeitung

Asynchrone Command-Verarbeitung ist keine Sonderregel des Warp. Eine Controlled Session muss
weitere Commands annehmen können, während ein früherer Command noch Arbeit ausführt. Insbesondere
muss die REPL während eines laufenden Warp dessen Pace ändern oder ihn stoppen können.

Daraus folgen diese Anforderungen:

- Ein lang laufender Command darf die Transport- und Request-Verarbeitung nicht blockieren.
- Requests werden in der Reihenfolge ihres Eingangs angenommen und gestartet. Die Request-ID legt
  keine Ausführungsreihenfolge fest.
- Responses dürfen in einer anderen Reihenfolge eintreffen. Genau eine terminale Response pro
  angenommenem Request wird über die von `Session` vergebene Request-ID korreliert.
- Laufende Arbeit darf den Bevy-Event-Loop nicht so lange belegen, dass neue Commands erst nach
  ihrem Abschluss gelesen werden.
- Die konkrete Ausführung darf Bevy World nicht unkontrolliert von einem Hintergrundthread aus
  verändern.
- REPL, Agent und Script erzeugen dieselbe Command-Darstellung aus qualifiziertem Command-Namen und
  `arguments`. Die REPL übersetzt menschliche Eingaben in diese Form, der Agent reicht einzelne
  JSONL-Einträge durch und ein Script enthält eine versionierte Liste derselben Einträge.
- `Session::send` nimmt hostseitig ausgeführte Commands sowie Wire-Commands für die Controlled
  Session über dasselbe Interface an. Es vergibt für jeden angenommenen Command eine
  sessionlokale Request-ID. Bei Wire-Commands wird diese ID in den Protokoll-Request übernommen;
  hostseitige Commands werden unter derselben ID lokal ausgeführt.
- `Session::send` wartet nicht auf den Abschluss, sondern liefert eine nach dem erwarteten Output
  typisierte `Pending`-Referenz. Sie stellt die von der Session vergebene Request-ID und den
  zugehörigen Command nur lesend bereit. `try_receive` und `receive` holen das Ergebnis später ab;
  bereits eingetroffene Responses anderer Requests bleiben bis zu deren Abholung erhalten.
- Controller dürfen beliebig viele Commands einreichen, ohne auf terminale Outcomes früherer
  Commands zu warten. Die gemeinsame Schicht meldet für jeden gültigen JSON-Command zuerst
  `pending` mit Request-ID und qualifiziertem Command-Namen. Später folgt genau ein terminales
  `completed`, `rejected` oder `failed`, ebenfalls mit Request-ID und Command-Namen. Pending-
  Meldungen folgen der Eingangsreihenfolge; terminale Meldungen dürfen ungeordnet eintreffen.
- `try_receive` blockiert nicht und liefert `Ok(None)`, solange genau dieses `Pending` noch kein
  Ergebnis besitzt. `receive` wartet auf genau dieses `Pending`. Beide Methoden verarbeiten
  währenddessen auch andere eintreffende Responses und bewahren deren Ergebnisse für die
  zugehörigen `Pending`-Referenzen auf.
- Wird ein `Pending` fallengelassen, verliert der Aufrufer ausschließlich den späteren Zugriff auf
  dessen Ergebnis. Der bereits gestartete Command wird nicht abgebrochen und erhält keinen
  versteckten Stop- oder Ausgleichs-Command. Die Session korreliert eine spätere Response weiterhin,
  schließt den History-Eintrag ab und darf den nicht mehr abrufbaren Output danach verwerfen.
- Nach `try_receive` mit `Some(output)` ist das geliehene `Pending` terminal ausgelesen. Eine weitere
  Verwendung dieses `Pending` oder die Verwendung eines `Pending` mit einer anderen Session ergibt
  `session::Error::InvalidPending`. `receive` konsumiert das `Pending`, sodass eine erneute
  Verwendung bereits durch Rust ausgeschlossen ist.
- Kann die Session den `output` einer erfolgreichen Wire-Response nicht als erwarteten `C::Output`
  dekodieren, schließt sie das zugehörige `Pending` mit `session::Error::Protocol` und bewahrt in der
  History `ProtocolFailed`. Andere ausstehende Requests bleiben davon unberührt.
- Eine Command-Ablehnung bedeutet, dass die Controlled Session den Command verstanden, aber
  fachlich abgelehnt hat. Sie wird als `Rejected` mit stabilem Code und lesbarer Meldung erhalten.
- `ProtocolFailed` bedeutet, dass für einen bestimmten Command keine gültige Command-Response
  zustande kam. Eine `protocol_error`-Nachricht mit bekannter Request-ID schließt den zugehörigen
  `Pending` mit `session::Error::Protocol` ab, wird ohne Request-ID in die History und bei einem
  späteren Report in dessen Kontext übernommen und beendet die Session nicht.
- Fehlt einer Protokollfehlermeldung eine bekannte Request-ID, bleiben alle ausstehenden Requests
  offen. Die Session erhält die Meldung unabhängig von der Command-Historie.
- Endet der Transport oder die Controlled Session, werden alle noch ausstehenden Commands in der
  History als `Unanswered` markiert. Wartende Aufrufer erhalten `session::Error::Ended`.
- `session::history::Outcome` unterscheidet deshalb `Completed`, `Rejected`, `ProtocolFailed` und
  `Unanswered`. Keines dieser Outcomes speichert die Wire-Request-ID.
- Sessionweite Ereignisse werden ohne Callbacks in einer internen Queue gepuffert.
  `Session::try_receive_event` fragt sie nicht blockierend ab; `Session::receive_event` wartet auf
  das nächste Ereignis. `session::Event` unterscheidet `ProtocolError { code, message }` und
  `Ended`.
- Eine Protokollfehlermeldung ohne bekannte Request-ID erzeugt `Event::ProtocolError`, ohne offene
  Requests zu schließen. Ein unerwartetes Prozess- oder Transportende erzeugt genau einmal
  `Event::Ended` und schließt alle offenen `Pending` mit `Error::Ended`. Nach Entnahme des
  terminalen Events ergeben weitere Empfangsoperationen `Error::Ended`.
- `ProtocolFailed` löst allein keinen Report aus. Der wartende Aufrufer erhält
  `session::Error::Protocol`, und die History bewahrt das Command-Outcome. Entsteht später durch
  einen Panic, einen aktivierten Tracing-Error oder ein unerwartetes Prozessende ein Report, wird
  der Protokollfehler als Teil des reportspezifischen Diagnosekontexts aufgenommen.

### Auflösung des bisherigen Session-Controllers

- Das Ziel besitzt kein `session::controller`-Modul, keinen `ControllerSession`-Wrapper und kein
  allgemeines `Controller`-Trait. `session::Session` ist der gemeinsame Command-Ausführungsweg und
  kennt nicht, ob ein Mensch, Script oder Replay die Commands sendet.
- `session::Session` übernimmt aus dem bisherigen Controller nur allgemeine Session-Verantwortung:
  Start und Handshake, Command-Ausführung, Request-Korrelation, History, Recording, Capabilities,
  Session-Events, Shutdown und Prozesslebenszyklus.
- `host::repl` besitzt Parsing und Darstellung menschlicher Eingaben, Terminal-Ein-/Ausgabe,
  REPL-spezifische Komfortbefehle und die Anzeige ausstehender Commands. Ob normalisierte
  Pointer-Koordinaten oder vergleichbare Hilfen erhalten bleiben, wird ausschließlich mit der REPL
  entschieden.
- `host::script` besitzt nur die versionierte Dateihülle, das Parsing der gemeinsamen
  Command-Einträge, deren asynchrone Einreichung und die Zuordnung der Outcomes zu den
  ursprünglichen Array-Positionen. Es verwendet `session::Session` direkt und erhält keinen
  gemeinsamen Wrapper mit der REPL.
- `command::replay` besitzt das Lesen der Recording-Einträge, deren Ausführung über die bestehende
  Session, Outcome-Vergleiche, Stop und Abschlussstatus. Replay startet keine eigene Controlled
  Session mehr.
- Die bisherigen gemeinsamen Typen `ControllerError`, `Action`, `PointerAction`, `KeyboardAction`,
  `Observation`, `Status` und `SurfaceSize` werden nicht als Session-Typen übernommen. Jeder
  konkrete Controller besitzt nur die Darstellung und Fehler, die sein Interface benötigt.
- `pause`, `resume`, markergebundenes `activate_target` und automatische `startup_frames` entfallen.
  Ein Session-Start und Controller-Hilfen führen keine versteckten Ticks aus. Benötigte anfängliche
  Ticks werden als ausdrückliche Tick-Commands gesendet.
- Recording erfasst am gemeinsamen Session-Ausführungspunkt die tatsächlich ausgeführten Commands
  und ihre Outcomes. Der bisherige Controller muss keine separaten `ControllerAction`-Einträge oder
  Controllerfehler mehr an den Driver melden.

### Session-Lebenszyklus

Die grobe Richtung steht, das öffentliche Interface ist noch offen:

- `session::Session` besitzt genau eine laufende Controlled Session, deren Transport, ausstehende
  Commands und Prozesslebenszyklus.
- `Session::start(config)` validiert die Session- und Launch-Konfiguration, startet den Cargo-
  Prozess, bindet stdin, stdout und stderr an und wartet auf den Ready-Handshake. Eine nutzbare
  `Session` wird erst zurückgegeben, nachdem eine gültige `Ready`-Nachricht mit der erwarteten
  Protokollversion als erste Protokollnachricht verarbeitet wurde.
- Der Session-Start besitzt keinen eingebauten Timeout. Insbesondere darf eine erstmalige Cargo-
  Kompilierung beliebig lange dauern, solange der Prozess weder endet noch eine ungültige erste
  Protokollnachricht sendet.
- Endet der Prozess oder Transport vor `Ready`, ist die erste Nachricht ungültig oder stimmt die
  Protokollversion nicht, schlägt `Session::start` fehl und räumt den gestarteten Kindprozess auf.
- Ein Request vor `Ready` ist über das öffentliche Interface nicht möglich. `Session` besitzt
  deshalb keinen getrennten öffentlichen `ready`-Schritt.
- `Session` übernimmt die Capabilities aus dem gültigen Ready-Handshake und stellt sie über
  `Session::capabilities(&self) -> &session::Capabilities` bereit. Der Snapshot bleibt während der
  Session unverändert; eine spätere Änderung des World-Zustands verändert ihn nicht.
- `session::Capabilities` besitzt zunächst nur `screenshot: bool`. Weitere Felder entstehen erst für
  eine weitere konkrete optionale Funktion.
- `session::Error` unterscheidet `InvalidConfig`, `Launch`, `Io`, `InvalidPending`, `Rejected`,
  `Protocol` und `Ended`. Die Varianten sind die öffentlichen Fehlergruppen; genauere
  Betriebssystemfehler und
  Prozessdiagnosen bleiben Implementierungs- beziehungsweise Beobachtungsdaten.
- `InvalidConfig` wird vor dem Prozessstart für ungültige Session- oder Launch-Konfigurationen
  zurückgegeben. `Launch` bedeutet, dass Cargo nicht gestartet werden konnte oder der gestartete
  Prozess ohne Protokollverletzung keinen gültigen Ready-Handshake erreichte. Ein ungültiger
  Handshake oder eine falsche Protokollversion ist stattdessen `Protocol`.
- `Io` bezeichnet eine fehlgeschlagene Betriebssystemoperation bei Prozess- oder Transportzugriff.
  Ein Transportende ist davon getrennt: vor dem Ready-Handshake verhindert es den Launch, nach dem
  Handshake beendet es die Session mit `Ended`.
- `Rejected { code, message }` erhält eine fachliche Command-Ablehnung. `Protocol { code, message }`
  erhält einen einem Aufruf zugeordneten Protokollfehler einschließlich einer nicht als erwarteter
  Output dekodierbaren erfolgreichen Response.
- Ein sauberer Shutdown beantwortet den Shutdown-Command, beendet den Prozess und schließt offene
  hostseitige Arbeit kontrolliert ab.
- Ein unerwartetes Prozess- oder Transportende bleibt von einer normalen Command-Ablehnung
  unterscheidbar und stellt ausstehende Commands auf `Unanswered`.
- Eine weitere `Ready`-Nachricht nach erfolgreichem Start ist ein nicht fataler Session-
  Protokollfehler. Sie erzeugt `Event::ProtocolError` mit dem stabilen Code `unexpected_ready` und
  verändert die beim ersten Handshake gespeicherten Capabilities nicht.
- `Session::shutdown(&mut self)` beginnt nur, wenn keine Requests mehr ausstehen und kein Recording
  aktiv ist. Andernfalls wird der Shutdown abgelehnt, ohne die Session zu verändern. Ein laufender
  Warp oder ein laufendes Replay muss ausdrücklich gestoppt oder vollständig empfangen und ein
  aktives Recording ausdrücklich mit `recording::Stop` beendet werden; Shutdown sendet keine versteckten Stop-
  Commands.
- Nach erfolgreichem Shutdown ist die Session beendet und weitere Operationen ergeben
  `Error::Ended`. Die Verwendung von `&mut self` statt eines konsumierenden Shutdowns verhindert,
  dass eine Ablehnung durch anschließenden Drop trotzdem einen harten Abbruch verursacht.
- Wird `Session` ohne erfolgreichen `shutdown` fallengelassen, führt Drop keinen fachlich sauberen
  Abschluss aus.
  Drop schließt die Pipes, beendet die Prozessgruppe und sammelt den Kindprozess ein, damit keine
  Prozesse zurückbleiben. Es sendet keinen Shutdown- oder Stop-Command, wartet
  nicht auf ausstehende Commands und garantiert weder vollständige Outcomes noch Recording-
  Abschluss, Report-Erzeugung oder Provider-Ausführung.
- Eine durch Session-Drop veranlasste Prozessbeendigung ist kein unerwarteter Anwendungsfehler und
  löst keinen Report aus. Wer einen sauberen Session-Abschluss benötigt, muss ausdrücklich
  `shutdown` aufrufen.
- Der aktuelle `DriverError::RequestFailed(Response)` wird zu `Rejected { code, message }`.
  `DriverError::Child` entfällt: Vor einem gültigen Ready wird ein entsprechendes Prozessende zu
  `Launch`, danach zu `Ended`. Die bisherigen freien `Launch`, `Io` und `Protocol`-Meldungen werden
  den gleichnamigen Zielvarianten zugeordnet; Konfigurationsvalidierung erhält erstmals die eigene
  Variante `InvalidConfig`.
- `Session` liest und überträgt stderr, beobachtet Transport und Prozessstatus und stellt beim
  Fehlerzeitpunkt den Session- und History-Snapshot bereit. Die Erkennung und fachliche Einordnung
  von Panic, Tracing-Error und unerwartetem Prozessende gehört zu `report`.

### Session Launch

- `session::launch` besitzt die Cargo-Konfiguration zum Start genau einer Controlled Session.
  `session::Config` verwendet sie über das Feld `launch: launch::Config`; die Launch-Felder werden
  nicht direkt in die übrige Session-Konfiguration verteilt.
- Eine Controlled Session wird ausschließlich mit `cargo run` gestartet. Beliebige Executables,
  Shell-Kommandos und andere Cargo-Unterkommandos wie `build`, `test` oder `bench` gehören nicht zum
  Ziel-Interface.
- Jeder Launch nennt genau ein nicht leeres Cargo-Package. Eine implizite Package-Auswahl über das
  aktuelle Arbeitsverzeichnis, das aktuelle Manifest oder Workspace-`default-members` wird nicht
  unterstützt.
- Das Target ist entweder `launch::Target::Binary { name }` oder
  `launch::Target::Example { name }`. Der Name muss nicht leer sein; Cargo wählt weder ein
  `default-run` noch das einzige vorhandene Binary implizit aus.
- `features` aktiviert die angegebenen Cargo-Features für den Launch. `arguments` enthält
  ausschließlich Argumente für die gestartete Anwendung und wird hinter `--` an sie übergeben.
- `session::launch` validiert Package und Target, bevor ein Prozess gestartet wird. Das Erzeugen des
  konkreten `std::process::Command` bleibt Implementierung und ist kein öffentliches Interface.

#### Auswirkungen auf den Ist-Stand

- `session::launch::Spec` wird zu `session::launch::Config`.
- Die getrennten Felder `kind` und `target` werden durch `launch::Target::{Binary, Example}` mit dem
  jeweils zugehörigen Namen ersetzt.
- `Spec::command` entfällt aus dem öffentlichen Interface. Die interne Session-Prozessverwaltung
  baut daraus weiterhin den Cargo-Aufruf.
- Die heutige zusätzliche Validierung von `application.package` und `application.target` in
  `host::Config` entfällt. Der private Host-Config-Parser liest die Launch-Konfiguration unter
  `session.launch`; die fachliche Validierung besitzt ausschließlich `session::launch`.

### Session-Konfiguration und Report-Kontext

- `session::Config` enthält alle Werte, die eine Controlled Session und ihr Verhalten konfigurieren.
  Das Ziel besitzt kein zusätzliches öffentliches Host-Konfigurationsmodell.
- `session::Config` enthält genau `launch: launch::Config`, ein konkretes
  `artifact_dir: PathBuf`, `tick: command::tick::Config` und `report: report::Config`.
- `command::tick::Config` besitzt ausschließlich die Standard-Pace für Warps. Die Zeitkonfiguration
  der Anwendung gehört nicht zur Session-Konfiguration.
- Der private Host-Config-Parser liest diese vollständige `session::Config` aus dem Feld `session`
  des versionierten TOML-Dokuments. `session.artifact_dir` ist der Standardwert; ein CLI-Override
  ersetzt ihn vor `Session::start`. Danach besitzt nur die gestartete Session den wirksamen Pfad.
- Screenshot-, Recording- und Report-Pfade sind relativ zu diesem Session-Artifact-Verzeichnis und
  dürfen es nicht verlassen.
- Recording wird über die öffentlichen Recording-Commands gesteuert und besitzt kein Feld in
  `session::Config`.
- Das Ziel besitzt keinen eigenständigen `session::Context`. `Session` hält ihren Zustand nur einmal
  und pflegt keine zweite öffentliche Darstellung derselben Werte.
- `Report::create` erhält eine Referenz auf die laufende `Session` und stellt bei der
  Report-Erzeugung einen reportspezifischen `report::Context` zusammen. Dieser Auszug enthält keine
  Wire-Request-IDs und ist kein Bevy-World-Snapshot.
- Session Recordings verwenden keinen gemeinsamen `session::Context`. Falls das Recording-Format
  später eigene Anwendungs- oder Session-Metadaten benötigt, werden sie anhand seiner konkreten
  Format- und Diagnoseanforderungen geplant.
- `OPEN`: Die Anwendungs- und Session-Angaben in `report::Context` sowie die Ermittlung der
  Anwendungsversion festlegen.

### Controlled-Session-Integration

- Der aktuelle Name `client` entfällt im Ziel. Das Bevy-Plugin implementiert die Controlled-Session-
  Seite des Session-Interfaces und heißt öffentlich `session::Plugin`. Es ist unabhängig vom
  `host`-Feature verfügbar und wird nicht zusätzlich an der Crate-Wurzel re-exportiert.
- Diese Integration besitzt das Bevy-Plugin, den Command-Dispatcher, laufende Command-Arbeit und die
  Anbindung an die interne Session-Ein-/Ausgabe. Fachliche Command-Typen bleiben bei `command` und
  die Wire-Abbildung bei `session::protocol`.
- Nach Abschluss der Bevy-Plugin-Initialisierung prüft die Controlled-Session-Integration die
  tatsächlich installierten optionalen Funktionen und sendet das Ergebnis mit `Ready`. Sie meldet
  fachliche Capabilities und legt weder Bevy-Plugin-Namen noch ihre Erkennungslogik offen.
- Die Controlled-Session-Integration startet keinen Renderer und führt keinen öffentlichen Modus
  für rendererfreie oder gerenderte Sessions ein. Rendererabhängige Prüfungen bleiben bei den
  betroffenen Funktionen. Ein späterer rendererfreier Anwendungsfall kann deshalb dieselbe Session-
  und Protokollstruktur mit einer anderen Bevy-Zusammensetzung verwenden.
- JSONL über stdin/stdout bleibt die interne Ein-/Ausgabe zwischen Debug Host und Controlled
  Session. Das Ziel besitzt kein öffentliches `transport`-Modul, kein öffentliches Transport-Trait
  und keine öffentliche Konfiguration benutzerdefinierter Ein-/Ausgabeadapter.
- Die Controlled-Session-Seite liest intern stdin und schreibt stdout. Die Debug-Host-Seite besitzt
  intern die entsprechenden Pipes des Kindprozesses. Beide Seiten teilen nur den Codec und die
  Nachrichtentypen aus `session::protocol`, nicht eine gemeinsame Ein-/Ausgabeabstraktion.
- In-Memory-Ein-/Ausgabe bleibt als private oder crate-interne Test-Seam zulässig. Ein Testadapter
  allein begründet kein öffentliches Transport-Interface.
- Die aktuellen öffentlichen Typen `client::transport::{Input, JsonLinesInput, Output,
  StdoutOutput}`, `InputFactory` und die öffentliche Plugin-Konfiguration `with_io` entfallen. Falls
  später eine echte Einbettung ohne Prozessgrenze hinzukommt, wird deren Interface anhand dieses
  konkreten zweiten Nutzers geplant.
- `session::Plugin::default()` erzeugt die Produktionsintegration mit der internen JSONL-Anbindung.
  Die bisherigen Konstruktoren `rendered_stdio` und `logical_stdio` entfallen zusammen mit dem
  öffentlichen Session-Modus. Der bisherige Top-Level-Re-Export `AutomationControlPlugin` wird
  nicht als Kompatibilitätsalias fortgeführt.
- Die interne Verwaltung mehrerer laufender Commands im Bevy-Event-Loop wird während der
  Implementation entworfen. Sie muss neue Requests zwischen begrenzten Arbeitsabschnitten
  verarbeiten und darf den Bevy World nicht von einem unkontrollierten Hintergrundthread aus
  verändern; ihre konkrete Datenstruktur gehört nicht zum Ziel-Interface.

## Debug Host

Die folgenden Punkte legen die Host-Fassade und ihre interne Aufgabenverteilung fest:

- `host` ist die Entwickleranwendung um `session::Session`. Es besitzt Konfigurationsladen,
  Controller-Auswahl, CLI, REPL, Session Scripts und die Zusammensetzung des Gesamtablaufs.
- Das Ziel besitzt kein öffentliches `host::config`-Modul, kein `host::Config`, keinen
  `ConfigError` und keine öffentliche Konfigurationsversionskonstante. Die private
  Konfigurationsverarbeitung in `host::run` übersetzt den übergebenen Inhalt in genau eine
  `session::Config`, statt Session-Regeln zu duplizieren.
- Das Ziel besitzt kein `host::command_line`-Modul und keine öffentlichen CLI-Typen. Die private
  CLI-Verarbeitung in `host::run` parst die Prozessargumente und übersetzt sie in den gewählten
  Host-Ablauf. Der Parser lädt keine Konfiguration, startet keine Session, führt keinen Controller
  aus und bestimmt keine Exit-Codes.
- `host::repl` ist der menschliche Controller einer laufenden Session.
- `host::agent` ist der interaktive maschinenlesbare Controller für einen KI-Agenten. Der Agent
  startet den Debug Host, sendet Commands, wertet strukturierte Ergebnisse aus und entscheidet
  innerhalb derselben laufenden Session adaptiv über den nächsten Command.
- `host::agent` verwendet JSONL über stdin/stdout. stdout enthält in diesem Modus ausschließlich
  JSONL-Nachrichten; Logs und menschliche Diagnoseausgaben gehen an stderr. Der Agent sendet keine
  Request-ID, sondern reicht einzelne Einträge der gemeinsamen Command-Darstellung ein. `Session`
  vergibt die IDs und der Agent-Adapter gibt Pending- und Outcome-Meldungen zurück.
- `host::script` bleibt zusätzlich bestehen. Es liest ein versioniertes JSON-Dokument, das eine
  Liste derselben Command-Einträge enthält, und reicht sie in Dateireihenfolge asynchron an
  `Session` weiter. Das Format besitzt keine eigenen Send-, Receive-, Namens- oder
  Erwartungskonzepte.
- REPL, Agent-Controller und Script verwenden dieselbe `session::Session` und dieselbe Command- und
  Outcome-Darstellung. Die REPL übersetzt zwischen Text und dieser Darstellung, der Agent reicht
  einzelne JSONL-Einträge durch und das Script legt eine Dateihülle um eine Liste dieser Einträge.
- `host::run(config_source: &str) -> std::process::ExitCode` ist der einzige öffentliche
  Programmeinstieg des Debug Hosts. Die Funktion liest den übergebenen Konfigurationsinhalt,
  verarbeitet den CLI-Aufruf und setzt den gewählten Ablauf zusammen.
- Die Implementation liegt in einem privaten `host::run`-Modul. Das Ziel besitzt kein öffentliches
  `runner`-, `program`- oder `entrypoint`-Modul und kein öffentliches Runner-Struct.
- `host::run` ruft nicht `std::process::exit` auf. Die einbettende `main`-Funktion gibt den
  zurückgegebenen `ExitCode` zurück.
- Für REPL, Agent-Controller und Session Script startet `host::run` genau eine Session, prüft die
  vom Ablauf benötigten Capabilities, führt den gewählten Controller aus und versucht anschließend
  einen
  sauberen Shutdown. Ein reiner Report-Aufruf startet keine Controlled Session.
- Der gewählte Host-Ablauf bestimmt seine benötigten Capabilities. Nach `Session::start` vergleicht
  der Host diese Anforderungen mit `Session::capabilities`, bevor er den Controller oder das Script
  startet. Fehlt eine benötigte Capability, beendet er die Controlled Session sauber und meldet
  einen Host-Startfehler. Die konkrete Darstellung der Anforderungen wird erst mit den Host-
  Interfaces festgelegt.
- `host::mod` bleibt eine kleine Fassade und exportiert keine internen CLI- oder
  Orchestrierungsdetails.
- Ein allgemeines `Controller`-Trait wird nicht vorweggenommen. Eine gemeinsame Seam entsteht erst,
  wenn mehrere konkrete Controller dieselbe zusätzliche Regel benötigen, die nicht bereits durch
  `session::Session` abgedeckt ist.
- `config_source` enthält das vollständige versionierte TOML-Dokument. Dessen Formatversion bleibt
  ein privates Persistenzdetail und wird nicht Teil des Laufzeitmodells. Unbekannte Felder und nicht
  unterstützte Versionen werden abgelehnt.
- Der private Config-Parser liest keine Datei. `host::run` erhält den Inhalt vom Aufrufer und erzeugt
  daraus die `session::Config`. Die CLI wählt den Host-Ablauf und darf ausschließlich
  `session.artifact_dir` überschreiben; andere Session-Werte erhalten keine allgemeinen CLI-
  Overrides.
- Die bisherigen Host-Felder `profile_id` und `tool` sowie die getrennte `application`-
  Konfiguration entfallen. Die Launch-Konfiguration besitzt mit `session.launch` genau einen Owner.
- Das Cargo-Feature `host` schaltet ausschließlich das Modul `host` und dessen private
  Abhängigkeiten für CLI, TOML und Ctrl-C frei. `command`, `report`, `session::Plugin`,
  `session::Session` und die übrigen Session-Typen bleiben ohne dieses Feature verfügbar. Der
  Kompatibilitätsalias `driver` entfällt.
- Ohne das Feature `host` existiert das Modul `host` nicht. Mit dem Feature exportiert es
  ausschließlich `host::run`; Config-, CLI-, Controller-, Report- und Orchestrierungstypen bleiben
  privat.

### Agent-Controller

#### Festgelegte gemeinsame Command-Verarbeitung

- `host::agent` ist ein dünner JSONL-Adapter vor derselben asynchronen Session-Ausführung, die REPL
  und Script verwenden. Er besitzt keine eigene Command-Sprache und keine eigene ID-Vergabe.
- Jede Eingabezeile enthält genau einen qualifizierten Command-Namen und das immer vorhandene
  `arguments`-Objekt. Eine Agent-Eingabe enthält keine Request-ID und kein zusätzliches
  `type: "command"`-Feld.
- Der Adapter validiert die Zeile und reicht den Command unmittelbar an `Session::send` weiter. Er
  darf weitere Eingabezeilen lesen und einreichen, während beliebig viele frühere Commands noch
  ausstehen. Ein laufender Warp blockiert daher weder weitere Agent-Eingaben noch Outcomes anderer
  Commands.
- Für jeden angenommenen Command gibt stdout zuerst eine `pending`-Meldung mit der von `Session`
  vergebenen Request-ID und dem qualifizierten Command-Namen aus. Später folgt genau eine
  `completed`-, `rejected`- oder `failed`-Meldung mit derselben ID und demselben Command-Namen.
- Pending-Meldungen werden in Eingangsreihenfolge ausgegeben. Terminale Meldungen dürfen in jeder
  Reihenfolge eintreffen. Der Command-Name wird wiederholt, damit ein Agent das Ergebnis nicht nur
  anhand der numerischen ID einordnen muss. Die Argumente werden in Meldungen nicht wiederholt.
- Recording und Replay verwenden dieselbe Eingabeform. `Session` erkennt anhand des fachlichen
  Command-Typs, ob der Command hostseitig ausgeführt oder über das Wire-Protokoll an die Controlled
  Session gesendet wird.

Die Agent-Form folgt diesem Muster:

```json
{"command":"tick.warp.start","arguments":{"ticks":600}}
{"command":"inspect.query","arguments":{}}
{"command":"tick.warp.stop","arguments":{}}

{"request_id":17,"command":"tick.warp.start","status":"pending"}
{"request_id":18,"command":"inspect.query","status":"pending"}
{"request_id":19,"command":"tick.warp.stop","status":"pending"}
{"request_id":18,"command":"inspect.query","status":"completed","output":{}}
{"request_id":19,"command":"tick.warp.stop","status":"completed","output":{"was_running":true}}
{"request_id":17,"command":"tick.warp.start","status":"completed","output":{"requested_ticks":600,"executed_ticks":42,"outcome":"stopped"}}
```

#### Noch zu planen

- kontrollierter Abschluss und EOF bei noch laufenden Commands oder aktivem Recording,
- ungültige JSONL-Zeilen und nicht als Command dekodierbare Einträge,
- Weitergabe sessionweiter Ereignisse und Verhalten bei unerwartetem Session-Ende.

### REPL

- `host::repl::run` verwendet eine bereits gestartete `session::Session` als mutable Referenz. Die
  REPL startet die Session nicht und führt keinen Shutdown aus; diese Lebenszyklusverantwortung
  bleibt bei `host::run`.
- Die REPL sendet Session-Commands grundsätzlich nicht blockierend und bleibt bei ausstehenden
  Commands ansprechbar. Insbesondere kann sie die Pace eines laufenden Warp ändern, ihn stoppen
  sowie Recording und Replay innerhalb derselben Session starten und stoppen.
- Die REPL übersetzt jede gültige menschliche Eingabe in denselben qualifizierten Command-Namen und
  dasselbe `arguments`-Objekt, die Agent und Script verwenden. Sie erzeugt keine eigene
  Command-Darstellung und vergibt keine eigene Anzeigenummer.
- Jeder gesendete Command wird mit der von `Session` vergebenen Request-ID und seinem Command-Namen
  als `pending` angezeigt. Ergebnisse werden nach ihrem Eintreffen automatisch mit derselben ID und
  demselben Namen ausgegeben. Die REPL besitzt keinen ausdrücklichen `receive`-Befehl; spätere
  Ergebnisse dürfen ungeordnet zwischen Prompts erscheinen.
- Die REPL-eigenen Befehle `help`, `pending` und `quit` werden nicht an `Session::send` übergeben.
  `pending` zeigt alle noch ausstehenden Commands mit ihrer Session-Request-ID.
- Replay ist kein eigener Host-Ablauf. `replay start PATH` und `replay stop` verwenden die
  hostseitigen Replay-Commands innerhalb der bestehenden Session. Dasselbe gilt für
  `recording start PATH` und `recording stop`.
- Parsefehler, Command-Ablehnungen, einem Command zugeordnete Protokollfehler und sessionweite nicht
  fatale Protokollmeldungen werden angezeigt und beenden die REPL nicht. Ein unerwartetes
  Session-Ende oder ein Fehler der Terminal-Ein-/Ausgabe beendet `run` mit einem REPL-eigenen
  Fehler.
- `quit` liefert nur dann `Exit::Quit`, wenn kein Command mehr aussteht und kein Recording aktiv
  ist. Andernfalls zeigt die REPL die blockierende Arbeit und verlangt, sie abzuwarten oder durch
  einen ausdrücklichen Command zu stoppen. `quit` sendet keine versteckten Stop-Commands.
- Ctrl-C liefert `Exit::Interrupted`, wenn kein Command aussteht und kein Recording aktiv ist.
  Andernfalls bleibt die REPL geöffnet, zeigt die blockierende Arbeit und verlangt einen
  ausdrücklichen Stop. Wiederholtes Ctrl-C erzwingt keinen harten Session-Abbruch.
- Bei geschlossener Eingabe nimmt die REPL keine neuen Zeilen mehr an, empfängt aber noch alle
  ausstehenden Ergebnisse. Danach liefert sie `Exit::InputClosed`. Ist anschließend noch ein
  Recording aktiv, ergibt `run` stattdessen `Error::ActiveRecordingOnInputClose`; die REPL beendet
  ein Recording nicht automatisch.
- Der aktuelle `ControllerSession`-Status und seine Felder `paused` und `last_action` werden nicht als
  Session-Zustand übernommen. Die REPL zeigt stattdessen ausstehende Commands und eingetroffene
  Ergebnisse.
- Die einfache Befehlssyntax folgt den fachlichen Command-Gruppen, darunter `tick warp`,
  `recording start|stop` und `replay start|stop`. Die genaue Textdarstellung komplexer Inspect-
  Queries und Sets wird mit deren JSON- und Wire-Abbildung festgelegt, ohne das REPL-Interface oder
  die Pending-Regeln erneut zu öffnen.

### Session Script

#### Festgelegtes Ziel

- `host::script` führt eine vorab beschriebene Liste von Commands gegen eine bereits laufende
  `session::Session` aus. Das Modul startet und beendet keine Session, schreibt keine Darstellung
  und bestimmt keinen Prozess-Exit-Code. `Script::parse` erhält den bereits gelesenen Dateiinhalt;
  Dateizugriff und die übrige Ablaufverantwortung bleiben bei `host::run`.
- Das persistierte Format ist ein versioniertes JSON-Dokument mit einem `commands`-Array. Jeder
  Eintrag besitzt exakt dieselbe Form aus qualifiziertem Command-Namen und `arguments`-Objekt, die
  ein Agent als einzelne JSONL-Zeile einreicht und die die REPL aus menschlicher Eingabe erzeugt.
- Das Script-Format besitzt keine eigenen Step-Varianten, Namen, Request-IDs, Send-/Receive-Regeln,
  Erwartungen, Assertions, Bedingungen, Schleifen oder zeitbasierten Waits.
- `Script::parse` liest und validiert das vollständige Dokument, bevor ein Command ausgeführt werden
  kann. Die Dokumentversion gehört nur zum gespeicherten Format und wird nicht als Feld in das
  aktuelle `Script`-Laufzeitmodell übernommen. Ein späterer Parser darf ältere Formatversionen in
  dieses Modell übersetzen. `Script::new` validiert programmatisch erzeugte Commands nach denselben
  Regeln.
- Ein Script darf keinen Shutdown-Command enthalten. Der aufrufende Host besitzt den
  Session-Lebenszyklus und führt den Shutdown nach dem gewählten Ablauf aus.
- `script::run` reicht alle Commands in Dateireihenfolge an `Session::send` weiter. Nach der
  unmittelbaren Vergabe einer Request-ID wird der nächste Command eingereicht; auf das terminale
  Outcome wird dabei nicht gewartet. Dadurch dürfen beliebig viele Script-Commands gleichzeitig
  ausstehen.
- Nachdem alle Commands eingereicht wurden, sammelt `run` sämtliche noch ausstehenden Outcomes ein.
  Die Zuordnung zum ursprünglichen Array-Eintrag erfolgt intern über die von `Session` vergebene
  Request-ID. Terminale Outcomes dürfen in jeder Reihenfolge eintreffen.
- Ein Script ist `Passed`, wenn alle Commands `completed` wurden. Mindestens ein `rejected` oder
  `failed` ergibt `Outcome::Failed` mit Command-Index, optional bereits vergebener Request-ID,
  Command und Fehlerdaten. Commands werden durch einen Fehler oder das Fallenlassen eines `Pending`
  nicht versteckt abgebrochen.
- Ticks, Warp-Stop, Recording und Replay stehen ausschließlich als ausdrückliche Commands in der
  Liste. Das Script fügt keine Commands und keine versteckten Simulationsticks ein.

Die JSON-Form folgt diesem Muster:

```json
{
  "version": 1,
  "commands": [
    {
      "command": "tick.warp.start",
      "arguments": { "ticks": 600 }
    },
    {
      "command": "inspect.query",
      "arguments": {}
    },
    {
      "command": "tick.warp.stop",
      "arguments": {}
    }
  ]
}
```

#### Auswirkungen auf den Ist-Stand

- Das bestehende Script-Modul wird gegen die gemeinsame asynchrone Session-Ausführung neu
  aufgebaut. Alte Action-, normalisierte Pointer-, markergebundene Click- und Component-Hilfen
  bestimmen weder das neue Format noch die Ausführung.
- Script-Parsing und strukturelle Validierung müssen vor der ersten Session-Mutation abgeschlossen
  sein. Dateipfade und Dateifehler ergänzt der aufrufende Host außerhalb des Script-Moduls.
- Das Script besitzt keine eigene Korrelationslogik im persistierten Format. Es verwendet intern die
  von `Session` vergebenen Request-IDs und dieselben Pending- und Outcome-Regeln wie Agent und REPL.
- Das neue persistierte Format erhält eine eigene Formatversion. Seine Command-Einträge verwenden
  jedoch dieselbe qualifizierte Command- und Argumentform wie Agent und Wire-Codec, damit keine
  zweite Command-Sprache entsteht.
