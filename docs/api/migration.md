# API-Migration

Dieses Dokument beschreibt den Weg vom Ist-Stand in [`current.md`](current.md) zur Zielstruktur aus
[`goal.rs`](goal.rs). Eine Zuordnung wird ergänzt, sobald der jeweilige Zielbereich ausreichend
definiert ist; ungeklärte Teile bleiben ausdrücklich `open`.

`hostseitig` bezeichnet in älteren Fachabschnitten die Ausführung außerhalb des Spielprozesses,
nicht ein Zielmodul `host`. Session-Ausführung gehört zu `session`, die Bereitstellung mehrerer
Sessions zu `server` und die Terminalbedienung zu `cli`.

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
| [`session::replay`](../../src/session/replay/mod.rs) | `command::replay` und sessionweite Replay-Ausführung | Replay in der laufenden Controlled Session ausführen und die Commands einer Recording ohne automatischen Vergleich ihrer Outcomes erneut senden | `rewrite` | wiederholtes Replay in derselben Session, Ausführung aus unterschiedlichen Ausgangszuständen, Stop, Blockierung und No-Tick-Verhalten |
| [`failure`](../../src/failure/mod.rs) | `-` | `Detail`, `Kind` und das persistierte Zwischenformat `failure.json` entfernen; bestehende Nutzer verwenden die fachlichen Fehler- und Diagnosetypen ihrer jeweiligen Module | `delete` | keine `failure`-Exporte, `failure.json`-Zugriffe oder generischen `Detail`-Verwendungen verbleiben |
| [`report`](../../src/report/mod.rs) und Fehleranteile aus [`session::diagnostics`](../../src/session/diagnostics.rs) | `report` | einen providerunabhängigen Report direkt aus dem beobachteten Fehler und der laufenden Session erzeugen; den reportspezifischen Diagnosekontext, Titel, Fehlersignatur, Provider-Ausführung und Duplikaterkennung dort besitzen | `rewrite` | Panic, Prozessabbruch, optionaler Tracing-Error, reportspezifischer Context und History-Auszug sowie lokale und GitHub-seitige Duplikaterkennung |
| [`session::context`](../../src/session/context.rs) | `report::Context` und interner Zustand von `session::Session` | den eigenständigen Session-Context entfernen; `Report::create` stellt den benötigten Diagnosekontext bei der Report-Erzeugung direkt aus der Session zusammen | `replace` | kein `session::Context` oder doppelt gepflegter Session-Zustand; Report-Fixtures enthalten keine Wire-Request-IDs |
| [`session::controller`](../../src/session/controller.rs) | `session::Session`, `server`, `client`, `cli::repl`, `cli::script` und `command::replay`; Agent-Zuordnung offen | Session-Ausführung von der lokalen Client-Anbindung trennen; ein persistenter Server verwaltet mehrere Sessions und eine gemeinsame Client-Seam verbindet die konkreten Fassaden | `replace` | Client-Trennung ohne Session-Ende, sessionlokale Request-IDs, eindeutig zugeordnete Activity und unabhängige Sessions |
| [`session::diagnostics`](../../src/session/diagnostics.rs) | interne Prozessbeobachtung von `session`, `session::history` und `report` | das Modul auflösen: Session liest stderr und beobachtet den Prozesslebenszyklus, die History behält Command-Outcomes und `report` erkennt und dokumentiert Laufzeit- und Logikfehler | `replace` | stderr-Weiterleitung, History-Grenze, Panic-Erkennung samt Backtrace, Vorrang vor Prozessabbruch und optionaler Tracing-Error |
| übrige Verantwortung aus [`session::driver`](../../src/session/driver.rs) | `session::Session` und interne Prozessverwaltung | Prozesslebenszyklus, Transport und ausstehende Commands hinter dem Session-Interface neu aufbauen | `rewrite` | Start, Ready, parallele Requests, Shutdown, Drop und unerwartetes Ende |
| [`session::launch`](../../src/session/launch.rs) | `session::launch` und `session::Config::launch` | Cargo-only-Launch als `launch::Config` behalten, ein Package und ein Binary oder Example ausdrücklich verlangen und die Prozess-Command-Erzeugung intern halten | `rewrite` | Binary und Example, leere Package- und Target-Namen, Features, Anwendungsargumente und exakt erzeugter Cargo-Aufruf |
| übrige Bevy-seitige Verantwortung aus [`client::plugin`](../../src/client/plugin.rs) | Controlled-Session-Integration unter `session` | Plugin, Command-Dispatcher und laufende Command-Arbeit aus dem irreführenden Modul `client` verschieben | `rewrite` | Bevy-Plugin, mehrere Requests, Render- und No-Tick-Verhalten |
| [`client::transport`](../../src/client/transport.rs) | interne Session-Ein-/Ausgabe | öffentliche Transporttypen entfernen; JSONL über stdin/stdout auf beiden Prozessseiten intern implementieren und In-Memory-Ein-/Ausgabe nur als interne Test-Seam behalten | `rewrite` | JSONL-Framing, Flush, Transportende sowie Controlled-Session- und Host-Tests mit interner In-Memory-Ein-/Ausgabe |
| [`host::command_line`](../../src/host/command_line.rs) | Argumentverarbeitung in `cli` | alte öffentliche CLI-Typen entfernen; Prozessargumente in Serverstart, Session-Verwaltung oder einen konkreten Client-Aufruf übersetzen | `replace` | vollständige CLI-Bedienung, Parsefehler und der in H7 festgelegte Exportvertrag |
| [`host::config`](../../src/host/config.rs) | private Konfigurationsverarbeitung, Zuordnung in HS1/H6 offen | öffentliche Config-Typen entfernen; Dateizugriff, Persistenzformat und fachliche Server-/Session-Konfiguration trennen | `open` | Version, unbekannte Felder, Session-Validierung, Artifact-Override und keine doppelte Config-Validierung |
| [`host::repl`](../../src/host/repl.rs) | `cli::repl` über `client` | menschliche Eingaben in gemeinsame Commands übersetzen und den Activity-Stream einer verbundenen Session darstellen | `rewrite` | Wiederverbindung, Cursor, gemeinsame Request-IDs, parallele Clients sowie Trennung ohne Session-Ende |
| `-` | `server` und `client` | einen persistenten lokalen Server mit Session-Verzeichnis für mehrere unabhängige Sessions und eine gemeinsame versionierte Client-Seam ergänzen | `add` | Discovery, Session-Erzeugen und -Auflisten, Routing, Zugriffsschutz, Cursor-Gaps, Client-Trennung, Session-Shutdown unabhängig vom Server |
| `-` | Agent-Zugang, Modulplatzierung offen | Zugriff über `client` oder maschinenlesbare CLI; eigene oder eingebundene Laufzeit und Modellanbindung in H4 entscheiden, ohne MCP | `open` | nach H4: Agent-Aufrufe, Session-Zuordnung, Command-Send, Activity-Poll und -Wait sowie unabhängige Session-Lebensdauer |
| [`host::runner`](../../src/host/runner.rs) | Ablaufwahl in `cli`, laufender Betrieb in `server` | das eigenständige Runner-Konzept entfernen; CLI wählt den Ablauf, Server besitzt Sessions und die clientunabhängige Verarbeitung | `replace` | Serverstart, REPL-, Agent-, Script- und Report-Aufrufe, Capability-Prüfung, Session-Shutdown und Serverende sowie Exit-Codes ohne `process::exit` |
| [`host::script`](../../src/host/script.rs) | `cli::script` über `client` | eine versionierte Liste gemeinsamer Commands vollständig validieren, über eine verbundene Session einreichen und ihre Outcomes einsammeln | `rewrite` | Parse- und Validierungs-Fixtures, gemeinsame Request-IDs, Wiederaufnahme, Shutdown und Fehlerpositionen |
| [`host::mod`](../../src/host/mod.rs) | getrennte Module `server`, `client`, `cli` | Sammelfassade entfernen; Einstiegspunkte, Exports, Features und Crates nach HS1 bis HS4 in H7 festlegen | `replace` | kein `host`-Wrapper im Ziel; direkte Session-Nutzung ohne Server/CLI, Export- und Feature-Tests nach H7; kein Alias `driver` |
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
- Pointer-Commands gelten immer für das Spielfenster. Sie nehmen keine Surface entgegen und können
  weder ein anderes Fenster noch den Desktop auswählen. `MoveTo` verwendet logische Pixel relativ
  zur linken oberen Ecke des Spielfensters. `MoveBy` verschiebt den Pointer relativ zu seiner
  bekannten Position.
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
- Das bisherige optionale `surface`-Argument von `pointer::Command::Move` entfällt. Die Controlled
  Session löst das Spielfenster intern auf.
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
- `Readable` übernimmt für erfolgreich serialisierbare Werte die Ausgabe von Bevys
  `TypedReflectSerializer`, umgewandelt in `serde_json::Value`. `bug_hunter` führt daneben keinen
  eigenen typisierten Reflection-Baum ein. Schlägt diese Serialisierung fehl, bleibt das betroffene
  Ergebnis mit `Unavailable::NotSerializable` sichtbar.
- Ein kleiner `ReflectSerializerProcessor` prüft direkt reflektierte `f32`- und `f64`-Werte vor
  Bevys normaler oder benutzerdefinierter Serialisierung mit `is_finite()`. `NaN` und positive oder
  negative Unendlichkeit brechen die Serialisierung des betroffenen Component- oder Resource-Werts
  ab und erzeugen `Unavailable::NotSerializable`, statt durch `serde_json::Value` zu `null` zu
  werden. Von einem opaken benutzerdefinierten Serializer ausdrücklich erzeugtes `null` wird nicht
  nachträglich umgedeutet.
- Maps behalten die von Bevy und `serde_json` erzeugte JSON-Objektform. Als JSON-Objektschlüssel
  unterstützte skalare Schlüssel werden dabei zu Strings. Kann `serde_json` einen Map-Schlüssel
  nicht als Objektschlüssel serialisieren, schlägt die Serialisierung des betroffenen Component-
  oder Resource-Werts fehl und erzeugt `Unavailable::NotSerializable`. `bug_hunter` führt keine
  abweichende Liste aus Schlüssel-Wert-Paaren ein.
- Sets behalten Bevys JSON-Arrayform, erhalten aber eine stabile Reihenfolge. Der
  `ReflectSerializerProcessor` serialisiert jedes Element mit denselben Inspect-Regeln, ordnet
  Objektschlüssel im daraus gebildeten kompakten JSON rekursiv alphabetisch und sortiert die
  Elemente lexikografisch nach den Bytes dieser Darstellung. Der Sortierschlüssel erscheint nicht
  zusätzlich im Output. Verschachtelte Sets werden auf dieselbe Weise sortiert. Kann ein Element
  nicht serialisiert werden, wird der umgebende Component- oder Resource-Wert
  `Unavailable::NotSerializable`.
- Asset-Handles mit Pfad oder stabiler UUID folgen Bevys `HandleSerializeProcessor`. Ein flüchtiger
  Handle ohne Pfad und UUID bleibt als `{"Ephemeral":{"id":"..."}}` lesbar. `id` ist die
  16-stellige, kleingeschriebene Hex-Darstellung von `AssetIndex::to_bits()` und ausschließlich ein
  opakes, sessionlokales Token. Bei einem `UntypedHandle` stehen wie bei Bevys
  `TypedHandleReference` zusätzlich der Asset-Type-Path und die Referenz im Output. Innerhalb einer
  Session kann ein Aufrufer damit gleiche Handles erkennen; zwischen Sessions darf sich das Token
  ändern. Das Token ist deshalb nicht für einen automatischen Vergleich zwischen Recording und
  Replay geeignet; Replay vergleicht Outcomes nicht. Fehlende notwendige Handle- oder
  Asset-Typregistrierung erzeugt `Unavailable::NotSerializable`. Bevys `Silent`- und `Warn`-Verhalten
  wird nicht verwendet, weil es einen flüchtigen Handle durch eine falsche Default-Identität ersetzt.
- Repräsentative Wire-Fixtures für die unterstützten Reflection-Kategorien halten die erwartete
  Bevy-0.19.1-Ausgabe fest. Ein Bevy-Upgrade darf diese erwartete Form nicht unbemerkt ändern. Eine
  Abweichung erfordert entweder eine Anpassung hinter dem Inspect-Interface oder eine ausdrückliche
  Änderung des Vertrags.
- Vollständige kanonische Bevy-Type-Paths sind die Wire-Identität für Component- und
  Resource-Typen. Prozesslokale `TypeId`- und `ComponentId`-Werte werden nicht übertragen.
- Eine Query liefert alle passenden Elemente gemeinsam in einer Response. Das Inspect-Interface
  besitzt weder Pagination noch Cursor oder fachliche Größenlimits und kürzt Ergebnisse nicht.
- Ein unbekannter Handle in `entity::Query::entity` lehnt den Command ab. Eine nicht auf einen
  Handle begrenzte Entity-Query ohne Treffer ist dagegen erfolgreich und liefert `items: []`.
- `component::Selection::Listed` liefert für jeden angeforderten, registrierten Type Path einen
  Eintrag in Eingabereihenfolge. Fehlt der Component an einer passenden Entity, enthält der Eintrag
  `Unavailable::Missing`. `Selection::All` liefert nur vorhandene Components.
- `resource::Selector::Type` mit einem registrierten, aber nicht vorhandenen Resource-Typ liefert
  bei der Projektion `Value` einen Eintrag mit `Unavailable::Missing`. Bei `Metadata` entsteht kein
  Eintrag, weil kein vorhandener Resource-Wert beschrieben werden kann. `Selector::All` liefert
  weiterhin nur vorhandene Resources.
- Entity-Items werden aufsteigend nach Handle, zuerst `index` und dann `generation`, sortiert.
  Resource-Items werden nach `type_path` sortiert. Component-Metadaten und `Selection::All` werden
  nach `type_path` und anschließend `name` sortiert; Einträge ohne Type Path werden über `name`
  eingeordnet. `Selection::Listed` folgt der Eingabereihenfolge. Hierarchie-Kinder behalten Bevys
  `Children`-Reihenfolge, weil diese fachlich relevant sein kann.
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
- Ein unbekannter Type Path wird mit `unknown_type_path` abgelehnt. Ein ausdrücklich angegebener
  Entity-Handle, der nicht mehr existiert, wird mit `entity_not_found` abgelehnt.

### JSON- und Wire-Form

Der Wire-Command heißt `inspect.query`. Die allgemeine v3-Hülle mit `request_id`, `command`,
`arguments`, `status` und `output` gehört `session::protocol`; dieser Abschnitt legt die
Inspect-spezifischen Inhalte von `arguments` und `output` fest.

Eine Entity-Query hat diese Form:

```json
{"request_id":17,"command":"inspect.query","arguments":{"source":"entities","entity":null,"with":["game::Player"],"without":[],"projection":{"kind":"summary"}}}
```

Eine Resource-Query hat diese Form:

```json
{"request_id":18,"command":"inspect.query","arguments":{"source":"resources","selector":{"kind":"type","type_path":"game::GameState"},"projection":{"kind":"value"}}}
```

Für die Argumente gelten folgende Regeln:

- `source` ist `entities` oder `resources`.
- Verschachtelte Enums verwenden ein `kind`-Feld mit `snake_case`-Werten.
- Optionale Felder werden mit `null` übertragen und nicht weggelassen. Listen sind auch dann als
  Arrays vorhanden, wenn sie leer sind.
- Entity-Projektionen sind `{"kind":"summary"}`, `{"kind":"component_names"}`,
  `{"kind":"components","selection":{"kind":"all"}}`,
  `{"kind":"components","selection":{"kind":"listed","type_paths":[...]}}` und
  `{"kind":"hierarchy","depth":3}`.
- Resource-Projektionen sind `{"kind":"metadata"}` und `{"kind":"value"}`.
- Resource-Selektoren sind `{"kind":"all"}` und
  `{"kind":"type","type_path":"vollständiger::TypePath"}`.
- Ein `handle::Handle` wird als `{"index":7,"generation":1}` übertragen.
- Die Felder der jeweils anderen Source-Kategorie und unbekannte zusätzliche Felder werden
  abgelehnt.

Ein Entity-Summary-Output sieht so aus:

```json
{"items":[{"kind":"entity","entity":{"index":7,"generation":1},"result":{"kind":"summary","name":"Player","component_count":8}}]}
```

Ein lesbarer Resource-Wert sieht so aus:

```json
{"items":[{"kind":"resource","result":{"kind":"value","type_path":"game::GameState","value":{"status":"readable","value":{"score":42}}}}]}
```

Entity-Resultate verwenden die Kinds `summary`, `component_names`, `components` und `hierarchy`.
Resource-Resultate verwenden `metadata` und `value`. `query::Item` verwendet `kind: "entity"` oder
`kind: "resource"`. Optionale Namen und Type Paths werden als `null` ausgegeben.

Ein lesbarer Wert und ein nicht lesbarer Wert haben unterschiedliche Formen:

```json
{"status":"readable","value":{"score":42}}
{"status":"unavailable","reason":"not_serializable"}
```

Die Werte für `reason` sind `missing`, `not_registered`, `not_reflectable` und `not_serializable`.
Ein `unavailable`-Objekt besitzt kein `value`; ein `readable`-Objekt besitzt kein `reason`.

Die Inspect-Fixtures decken mindestens Folgendes ab:

- jede Entity-Projektion,
- beide Resource-Projektionen und beide Resource-Selektoren,
- `Readable` und alle vier `Unavailable`-Gründe,
- unbekannte Type Paths und unbekannte explizite Entity-Handles,
- leere Ergebnismengen sowie fehlende gelistete Components und Resources,
- die festgelegte Reihenfolge von Entities, Components, Resources und Sets,
- Structs, Tupel, Enums, Maps, Sets, pfadbasierte, UUID- und flüchtige Handles sowie nicht endliche
  Fließkommazahlen,
- Ablehnung unbekannter oder zur gewählten Source unpassender JSON-Felder.

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
- `Capture` nimmt das einzige primäre gerenderte Spielfenster auf. Der Command besitzt kein
  `target`-Argument und kann weder ein anderes Fenster noch den Desktop auswählen. Fehlt ein
  primäres gerendertes Spielfenster oder existieren mehrere, wird der Command abgelehnt.
- Eine bereits vorhandene Datei am gewählten Dateiziel wird ersetzt.
- Der erfolgreiche Output wird erst zurückgegeben, nachdem die Aufnahme abgeschlossen und die
  PNG-Datei erfolgreich gespeichert wurde. Er enthält den relativen Pfad, Breite, Höhe und
  `overwritten`. `overwritten: true` bedeutet, dass eine zuvor vorhandene Datei ersetzt wurde.
- Ohne Renderer oder installierte Screenshot-Unterstützung wird der Command abgelehnt.

### Dateipfad und Überschreiben

- Der Controller gibt den vollständigen konkreten Dateipfad im `Capture`-Command an.
- Der Pfad ist relativ zum konfigurierten Session-Artifact-Verzeichnis. Dieses Verzeichnis darf
  außerhalb des Git-Projekts liegen; der Command darf es jedoch nicht verlassen.
- Der Aufrufer bestimmt über `session::Config` das gemeinsame Session-Artifact-Verzeichnis. Ein
  relativer Root bezieht sich auf den Elternordner des konfigurierten Cargo-Manifests. Der Screenshot-
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
- `Stop` antwortet mit `was_running: true`, wenn ein Warp beendet wurde, sonst mit
  `was_running: false`.
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
- [ADR-0005](../adr/0005-tick-warp-preserves-explicit-simulation-control.md) ersetzt die Step- und
  Zeitdauerregeln aus ADR-0003. Das Verbot versteckter Ticks, gesammelter Virtual Input und der
  Verzicht auf eingebaute Wait-Schleifen bleiben bestehen.


## Session Recording

### Festgelegtes Ziel

- `command::recording::Command::Start { path }` startet eine Session Recording am aktuellen
  Zustand der laufenden Controlled Session.
- `command::recording::Command::Stop` beendet und persistiert die aktive Aufzeichnung. Die Controlled
  Session bleibt aktiv.
- Ein Aufrufer kann `Start` direkt nach dem Start einer Controlled Session einreichen und dadurch
  alle nachfolgenden Session-Commands aufzeichnen. Dasselbe Interface erlaubt einen beliebigen
  späteren Abschnitt.
- Eine Session Recording ist eine geordnete Folge ausgeführter Session-Commands und ihrer
  korrelierten Ergebnisse. Command und Outcome werden als eine fachliche Ausführung erhalten; die
  dafür während der Ausführung verwendete Wire-Request-ID wird nicht aufgezeichnet. Die Recording
  enthält keinen Bevy-World-Snapshot und keine Zusage über den Zustand zu Beginn oder am Ende.
- Der Aufrufer entscheidet, ob eine Aufzeichnung eine Vorbereitung, eine Messung oder eine gesamte
  Sitzung beschreibt. Dafür werden keine unterschiedlichen Recording-Typen eingeführt.
- Recording-Start und -Stop werden von `session` ausgeführt und nicht über das Wire-Protokoll an
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
- Während Requests ausstehen, ordnet `session` Responses über die Wire-Request-ID zu. Vor dem
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
  zwischen `session::Session` und Controlled Application.

## Session Replay

### Festgelegtes Ziel

- `command::replay::Command::Start { path }` spielt eine gültige Session Recording in der aktuell
  laufenden Controlled Session ab.
- Replay startet keine neue Controlled Session, setzt keinen Zustand zurück und prüft vor dem Start
  nicht, ob der aktuelle Zustand dem ursprünglichen Ausgangszustand entspricht.
- Jede gültige und unterstützte Session Recording kann als Replay gestartet werden. Ob ihre Commands
  aus dem aktuellen Zustand dieselben Ergebnisse erzeugen würden, ist keine Vorbedingung.
- Dieselbe oder eine andere Aufzeichnung kann nach Abschluss erneut in derselben Controlled Session
  gestartet werden. Der Aufrufer besitzt die Verantwortung für den dabei vorhandenen Zustand.
- Replay besitzt intern die Zustände `Idle`, `Preparing`, `Running` und `Stopping`. Ein angenommener
  Start wechselt vor dem Laden von `Idle` nach `Preparing`. Nach erfolgreicher Vorbereitung folgt
  `Running`; ein Stop wechselt aus `Preparing` oder `Running` nach `Stopping`. Jeder terminale
  Abschluss führt zurück nach `Idle`.
- `Replay::Start` ist nur in `Idle` zulässig. Ein weiterer Start in `Preparing`, `Running` oder
  `Stopping` wird mit `replay_already_running` abgelehnt. Er ersetzt, verändert und reiht kein
  Replay ein.
- Der Replay-Adapter lädt die Recording vollständig und übersetzt sie vor der Ausführung in einen
  internen Replay-Plan. Die Recording selbst bleibt unverändert und bewahrt die tatsächlich
  ausgeführten Commands und ihre Outcomes.
- Kann die Recording nicht gelesen werden, endet der Start mit `session::Error::Io`. Ein lesbares,
  aber ungültiges Format wird mit `invalid_recording`, eine nicht unterstützte Formatversion mit
  `unsupported_recording_version` abgelehnt. In allen drei Fällen wird kein Recording-Command
  ausgeführt und Replay kehrt nach `Idle` zurück.
- Der Replay-Plan übernimmt die übrigen aufgezeichneten Session-Commands und ersetzt die
  aufgezeichneten Warp-Commands durch ihre effektive Form. Ausschließlich diese abgeleiteten Warps
  führen Ticks aus. Laden, Replay-Start, Replay-Stop und der Wechsel zwischen Recording-Einträgen
  erzeugen keine Ticks.
- Die aufgezeichneten Outcomes bleiben Bestandteil der Session Recording, werden beim Replay aber
  nicht automatisch mit den neuen Outcomes verglichen und bewerten den Replay-Abschluss nicht.
  `executed_ticks` eines aufgezeichneten Warp-Outcomes darf ausschließlich zur Bildung des
  effektiven Replay-Plans verwendet werden. Replay besitzt zunächst keine allgemeine Gleichheits-,
  Toleranz- oder Abweichungsregel für Command-Ergebnisse.
- Endete ein aufgezeichneter Warp nach weniger als den angeforderten Ticks durch `Warp::Stop`,
  enthält der Replay-Plan stattdessen einen Warp über genau `executed_ticks`. Den dazugehörigen
  aufgezeichneten Stop führt Replay nicht erneut aus. Bei `executed_ticks == 0` entfällt der Warp
  vollständig.
- Unmittelbar aufeinanderfolgende Warps darf der Replay-Plan zu einem Warp über die Summe ihrer
  effektiven Ticks zusammenfassen. Dazwischen dürfen kein anderer Command und keine Pace-Änderung
  liegen; beide Warps müssen dieselbe effektive Pace verwenden. Passt die Summe nicht in `u64`,
  bleiben die Warps getrennt.
- Nachfolgende Commands werden erst ausgeführt, nachdem der verkürzte, unveränderte oder
  zusammengefasste Warp seine Abschlussresponse geliefert hat. Replay muss deshalb keinen
  aufgezeichneten Warp-Stop an einer laufenden Tick-Grenze erneut terminieren.
- Zwischen zwei Warps darf Replay mehrere Nicht-Tick-Commands in Recording-Reihenfolge senden, ohne
  jede einzelne Response abzuwarten. Den nächsten Warp gibt es erst frei, nachdem alle zuvor
  gesendeten Commands ein terminales Outcome geliefert haben.
- `Completed` und `Rejected` erfüllen diese Antwortbarriere. Replay interpretiert einen stabilen
  fachlichen Ablehnungscode nicht und setzt den Plan fort. `ProtocolFailed` und `Unanswered`
  verhindern dagegen die weitere Ausführung und führen zu `Blocked`.
- Welche fachlichen Ablehnungscodes ein Replay künftig ebenfalls blockieren sollen, benötigt eine
  eigene anwendungsspezifische Policy. Diese konfigurierbare Bewertung gehört nicht zum aktuellen
  Replay-Interface.
- Replay endet mit `completed`, nachdem das Ende des Replay-Plans erreicht wurde und alle daraus
  gesendeten Commands ein terminales Outcome geliefert haben.
- `blocked` enthält einen stabilen technischen Code und eine menschenlesbare Meldung. Es wird
  verwendet, wenn eine ungültige Command-Response, ein Session- oder Transportende oder ein
  fehlgeschlagener Notaus die weitere geordnete Ausführung verhindert. Eine fachliche
  Command-Ablehnung allein erzeugt kein `blocked`.

Die technischen Blockierungsgründe sind:

| Code | Bedeutung |
| --- | --- |
| `command_protocol_failed` | Für einen normalen Plan-Command kam keine gültige Command-Response zustande. |
| `session_io_failed` | Ein I/O-Fehler der laufenden Session verhindert die weitere Replay-Ausführung. |
| `session_ended` | Die Controlled Session oder ihr Transport endete; noch offene Plan-Commands bleiben unbeantwortet. |
| `request_id_exhausted` | Für einen internen Plan-Command ist keine normale Request-ID mehr verfügbar. |
| `stop_failed` | Der Notaus konnte einen aktiven Warp bei weiter bestehender Session nicht kontrolliert beenden. |

Die lesbare Meldung erhält die konkrete technische Ursache. Diese Codes beschreiben ausschließlich
die technische Replay-Blockierung und übernehmen keine wechselnden Fehlercodes anderer Module.

- `Replay::Stop` ist ein kontrollierter Notaus. Replay gibt sofort keine weiteren Plan-Commands
  frei. Läuft ein vom Replay gestarteter Warp, sendet der Replay-Adapter intern `Warp::Stop` und
  verhindert damit weitere Ticks. Bereits ausgeführte Commands und Ticks werden nicht
  zurückgenommen.
- Bereits gesendete Input-, Inspect- oder Screenshot-Commands besitzen keinen allgemeinen
  Abbruchmechanismus. Im Zustand `Stopping` wartet Replay auf deren terminale Outcomes sowie auf die
  Stop- und Abschlussresponse eines zuvor aktiven Warps.
- Erst danach antworten sowohl der ursprüngliche `Replay::Start` mit `stopped` als auch alle
  wartenden `Replay::Stop`-Commands mit `was_running: true`. Ein weiterer Stop in `Stopping` wartet
  idempotent auf denselben Abschluss. Ohne aktives Replay ist `Stop` ein sofort erfolgreicher No-op
  mit `was_running: false`.
- Scheitert das Anhalten technisch oder endet die Session beim Warten, hat `blocked` Vorrang vor
  `stopped`. Der ursprüngliche Start erhält den Blockierungsgrund; wartende Stop-Commands liefern
  in diesem Fall keinen erfolgreichen Stop-Output, sondern den jeweils auslösenden
  `session::Error`.
- Die Response von `Replay::Start` bleibt bis `completed`, `stopped` oder `blocked` ausstehend.
  `Replay::Stop` ist währenddessen zulässig.
- Während `Preparing`, `Running` und `Stopping` führt Replay die Session exklusiv. Von außen ist nur
  `Replay::Stop` zulässig; einen anderen Command lehnt die Session mit `replay_in_progress` ab. Er
  wird nicht in den Plan eingemischt. Die vom Replay-Plan abgeleiteten Session-Commands passieren
  weiterhin den gemeinsamen Session-Ausführungsweg.
- Replay-Commands werden von `session` verarbeitet. Die aus der Recording abgeleiteten
  Session-Commands laufen über denselben Ausführungsweg wie Commands eines menschlichen,
  agentischen oder geskripteten Controllers.
- Replay besitzt keinen `Deviation`-Typ, veröffentlicht keine Abweichungen und erzeugt daraus keinen
  Report. `Completion` enthält keine Command-Zähler oder Vergleichsstatistik.
- Panics, aktivierte Tracing-Errors und unerwartete Prozessenden bleiben unabhängig vom Replay
  Report-Auslöser. Ein vollständig ausgeführtes Replay belegt ohne solche anwendungsseitig sichtbaren
  Invarianten nur, dass die Command-Folge bis zum Ende verarbeitet werden konnte.
- Spätere Zustandsvergleiche benötigen einen konkreten Anwendungsfall und ausdrückliche fachliche
  Regeln, etwa passende Toleranzen. Sie werden nicht als exakter Vergleich beliebiger
  `serde_json::Value` nachgerüstet.

### Auswirkungen auf den Ist-Stand

- Replay erzeugt keine frische Controlled Session. Die bestehende Host-Orchestrierung wird so
  getrennt, dass der Aufrufer die aktuelle Session nach einem Replay weiter verwenden kann.
- Die aktuelle Zuordnung erwarteter Ergebnisse und ihr exakter Vergleich mit Replay-Antworten
  entfallen. Der Replay-Adapter erstellt vor der Ausführung einen internen Plan. Er verwendet
  `executed_ticks` gestoppter Warps, um deren effektive Tickzahl zu bestimmen; andere
  aufgezeichnete Outcomes und das beim Replay neu entstandene Outcome werden nicht verglichen.
- Die bisherige laufende Wiederholung eines aufgezeichneten Warp-Starts und seines späteren Stops
  entfällt. Tests prüfen die Verkürzung gestoppter Warps, das Entfernen eines Warps mit null
  ausgeführten Ticks und das Zusammenfassen kompatibler benachbarter Warps. Die unveränderte
  Recording bleibt dabei die Quelle für Diagnose und Nachvollziehbarkeit.
- Die Replay-Ausführung erhält eine interne Zustandsmaschine für Vorbereitung, Ausführung und
  kontrollierten Notaus. Tests prüfen Mehrfachstart, Stop in jedem Zustand, wiederholten Stop,
  auslaufende Commands, einen aktiven Warp, technische Fehler beim Stoppen und die exklusive
  Command-Annahme. Session- und Controller-Fixtures prüfen außerdem `replay_already_running`,
  `replay_in_progress`, die vier stabilen Blockierungscodes und den Fehler wartender Stop-Commands
  bei einem blockierten Notaus.
- Persistierte Commands werden über einen versionsbewussten Recording-Adapter in die aktuelle
  Command-Form überführt. Das Recording-Format und das Wire-Format bleiben getrennte Verträge.
- Das aktuelle `replay-result.json` und sein Passed-/Failed-Ergebnis für Outcome-Abweichungen
  entfallen. Der Aufrufer erhält den Replay-Abschluss direkt als Command-Output.

## Report

### Festgelegtes Ziel

- Ein Report dokumentiert genau einen während einer Controlled Session beobachteten Laufzeit- oder
  Logikfehler.
- `report` stellt den Report aus beobachtbaren Informationen zusammen. Die Bevy-Anwendung muss
  keine reportspezifischen Traits, Fehlercodes oder Meldeaufrufe implementieren.
- Die Session liest stderr und beobachtet den Prozessstatus, interpretiert diese Daten aber nicht als
  fachliche Fehler. `report` erkennt daraus Panic, aktivierte Tracing-Errors oder ein unerwartetes
  Prozessende und besitzt die Vorrangregel zwischen ihnen.
- Ein crate-interner `report::Observer` besitzt das Marker-Framing und den stderr-Puffer. Die
  Session reicht ihm gelesene Bytes und den unerwarteten finalen Prozessstatus zu, ohne Marker selbst
  fachlich zu deuten.
- Normale Rust-Assertions und Panics können Logiklücken sichtbar machen. Schlägt beispielsweise
  `assert!` fehl, verwendet der serverseitige Report-Ablauf die erkannte Panic als Report-Auslöser.
  Assertions bleiben normale Anwendungslogik und kennen `bug_hunter` nicht.
- Ein Report besitzt einen menschenlesbaren Titel, die optional vorhandene unveränderte
  Fehlermeldung, deren Ursprung, eine normalisierte Fehlersignatur und einen reportspezifischen
  Diagnosekontext.
- `Report::create` erhält den beobachteten Fehler und eine Referenz auf die laufende
  `session::Session`. Es leitet Titel und Fehlersignatur aus dem Fehler ab und stellt den
  reportspezifischen Diagnosekontext direkt aus der Session zusammen. Der Aufrufer liefert weder
  eine vorbereitete Signatur noch einen vorbereiteten Diagnosekontext.
- Die Felder von `Report`, `Failure` und `Signature` sind privat. Lesende Methoden geben Titel,
  Fehler, Signaturwert und Context frei. Dadurch kann ein Aufrufer die von `Report::create`
  berechneten Werte weder unabhängig zusammensetzen noch nachträglich auseinanderlaufen lassen.
- `Failure::panic`, `Failure::tracing_error` und `Failure::process_exit` sind die einzigen
  öffentlichen Konstruktoren für einen fachlichen Fehler. `Failure::panic` akzeptiert eine optionale
  Meldung, weil ein unbekannter `panic_any`-Payload keinen lesbaren Text liefern muss. Der Konstruktor
  für ein Prozessende nimmt nur den Status entgegen und setzt die feste Meldung selbst.
- Für einen Panic werden eine bekannte Code-Stelle und die beobachtete Backtrace-Ausgabe aufgenommen.
  Beide dürfen fehlen, ohne die Report-Erzeugung zu verhindern.
- Der Kontext enthält höchstens die letzten 50 gesendeten Commands in Sendereihenfolge samt ihrem
  korrelierten Outcome. Wire-Request-IDs werden nicht in den Report übernommen. Ein Command ohne
  eingetroffene Response bleibt als unbeantwortet sichtbar.
- Die Session hält dafür einen Ring der letzten 50 angenommenen Commands. `Report::create` kopiert
  den aktuellen Ring beim Aufruf; spätere Outcomes verändern den erzeugten Context nicht.
- Die Command-Historie dient als Reproduktionskontext. Sie garantiert nicht, dass sich der Fehler
  ohne passenden Ausgangszustand reproduzieren lässt.
- Ein erkannter Panic oder ein unerwartetes Prozessende löst unabhängig von der
  Report-Konfiguration einen Report aus. Wird bei einem Prozessende bereits ein Panic erkannt,
  beschreibt der Report den Panic und nicht zusätzlich einen zweiten Prozessfehler.
- Der gemeinsame private Ablauf in `server` nimmt erkannte Fehler aus
  `session::Event::Failure` entgegen. Er ruft `Report::create` und `report::submit` auf und behält den
  Report zusammen mit dem Provider-Outcome oder Submit-Fehler. REPL, Agent und Script besitzen keine
  eigene Report-Auslösung.
- Ein unerwartetes Prozessende verwendet die feste Meldung `process exited unexpectedly`.
  Der genaue Prozessstatus bleibt als Diagnose in `Origin::ProcessExit::status` und wird nicht in
  die Meldung kopiert. stderr bestimmt die Meldung nicht.
- Eine Command-Ablehnung löst nicht automatisch einen Report aus. Sie kann durch einen unpassenden
  Command oder Ausgangszustand des Controllers verursacht worden sein.
- Die Session-Konfiguration besitzt die standardmäßig deaktivierte Einstellung
  `report.tracing_errors`. Bei Aktivierung behandelt der serverseitige Report-Ablauf die vom registrierten
  `session::tracing_error_layer` beobachteten dispatchten `tracing`-Events auf Error-Level als
  Report-Auslöser.
- Eine beliebige Textausgabe auf `stderr`, die nur das Wort `error` enthält, gilt nicht als
  `tracing`-Event.
- Ein gefiltertes oder durch einen Compile-Time-Filter entferntes Error-Event ist nicht beobachtbar.
  Die Report-Einstellung verändert die Logging-Filter der Anwendung nicht.
- Die Session-Konfiguration legt mit `report.output: PathBuf` ein nicht leeres relatives
  Report-Ausgabeverzeichnis innerhalb ihres Session-Artifact-Verzeichnisses und genau einen Provider
  fest. Ein Report darf dieses Ausgabeverzeichnis nicht verlassen.
- Jeder Bestandteil von `report.output` muss ein normaler Pfadbestandteil sein. Absolute Pfade sowie
  `.` und `..` lehnt `Session::start` als `session::Error::InvalidConfig` ab.
- Jeder Provider besitzt nur seine eigenen zusätzlichen Konfigurationswerte. Zunächst sind ein
  lokaler Markdown-Provider und ein GitHub-Provider vorgesehen.
- `report::submit` übergibt einen bereits zusammengestellten Report direkt an den Provider, der in
  der beim Session-Start wirksamen Report-Konfiguration festgelegt wurde. Es erhält ausschließlich
  den Report und die laufende Session. Der Aufrufer kann Provider oder Ausgabeverzeichnis beim
  Veröffentlichen nicht austauschen.
- Die Session stellt dabei den wirksamen Artifact- und Projektordner bereit, ohne diese internen
  Pfade in `report::Context` zu veröffentlichen. Statisch ungültige Report-Konfigurationen lehnt
  `Session::start` als `session::Error::InvalidConfig` ab.
- Das gemeinsame `provider::Outcome` enthält die erstellte oder bereits vorhandene Referenz.
- Der lokale Provider schreibt jeden neuen Report in eine eigene Markdown-Datei. Der
  GitHub-Provider veröffentlicht dagegen direkt und schreibt bei erfolgreicher Suche oder
  Veröffentlichung keine lokale Datei.
- Schlägt ein Remote-Provider fehl, führt `report::submit` den lokalen Markdown-Provider als
  Rückfall aus. `provider::Outcome::Fallback` enthält den lokalen Pfad und den Remote-Fehler. Es
  wechselt nicht zu einem anderen Remote-Provider und verbirgt den Fehler nicht als lokalen Erfolg.
- `provider::FileReference` enthält den Pfad einer lokalen Report-Datei.
  `provider::Reference::File` umschließt diesen Typ für `Created` und `Existing`.
  `Fallback::reference` verwendet `FileReference` direkt und kann deshalb kein Issue enthalten.
- Schlägt auch der lokale Rückfall fehl, gibt `report::submit` einen Fehler mit Remote- und
  Persistenzursache zurück.
- Provider verwenden die Fehlersignatur zur Duplikaterkennung. Ist derselbe Fehler bereits beim
  jeweiligen Provider dokumentiert, wird kein zweiter Bericht angelegt; das Outcome verweist auf
  die vorhandene Datei oder das vorhandene Issue.
- Eine vom Provider vergebene Kennung wie eine GitHub-Issue-Nummer entsteht erst bei der
  Veröffentlichung. Sie ist kein von der Anwendung oder Session-Konfiguration gelieferter
  Fehlercode.
- Bildvergleiche und andere visuelle Abweichungen gehören zunächst nicht zum Reporting-Interface.

### Grenze zur Session

- `session` besitzt Kindprozess, Pipes und Prozesslebenszyklus. Sie liest stderr als geordnete
  Bytefolgen und beobachtet den finalen Prozessstatus.
- `session` weiß aus ihrem Lebenszyklus, ob ein erfolgreicher Shutdown oder ihr eigener Drop die
  Beendigung ausgelöst hat. Solche absichtlichen Beendigungen leitet sie nicht als Report-Kandidaten
  weiter.
- Ein crate-interner `report::Observer` besitzt den privaten Zeilenpuffer für stderr. Er erkennt und
  dekodiert darin Panic-, Tracing- und Layer-Statusmarker und erzeugt daraus `Failure::panic`,
  `Failure::tracing_error` oder einen Beobachtungsfehler.
- Gültige Marker werden nicht an die menschliche stderr-Ausgabe weitergereicht. Alle anderen Bytes
  bleiben unverändert. Ungültige oder bei EOF unvollständige Marker bleiben sichtbar und erzeugen
  `session::Event::ObservationError` mit dem stabilen Code `invalid_report_marker`.
- Ein nicht absichtlich ausgelöstes Prozessende ohne vorrangigen Panic wird zu
  `Failure::process_exit`.
- Diese Beobachtungsgrenze kopiert weder Command-History noch Session-Metadaten. `Report::create`
  liest den begrenzten History- und Metadaten-Snapshot weiterhin direkt aus der laufenden Session.
- Beobachtungstypen, Zeilenpuffer und Markertransport bleiben crate-intern. Sie sind kein Teil des
  öffentlichen Session- oder Report-Interfaces.
- Marker verwenden ein versioniertes, zeilenbasiertes Chunk-Format mit Event-ID, Chunk-Index,
  Chunk-Anzahl, Base64-kodiertem JSON-Payload und Prüfsumme. Ein begrenzter Chunk wird mit genau einem
  Schreibaufruf ausgegeben. Event-IDs erlauben das Zusammensetzen gleichzeitig geschriebener Marker.
- Ein eigener stderr-Leser läuft ab dem Prozessstart und leert die Pipe bis EOF, auch während der
  Host einen Provider aufruft. Nach einem Prozessende verarbeitet die Session zuerst die
  verbleibenden stdout- und stderr-Daten und bewertet erst danach den Prozessstatus.
- Ein erkannter Panic unterdrückt einen zusätzlichen `ProcessExit`-Fehler derselben Session. Ein
  beschädigter oder unvollständiger Panic-Marker beweist keinen Panic; bei einem unerwarteten
  fehlgeschlagenen Prozessstatus bleibt `ProcessExit` der Rückfall.
- `session::Event::Failure` transportiert erkannte Fehler zum gemeinsamen privaten Ablauf unter
  `server`. Dieser ruft `Report::create` und `report::submit` auf und behält Report und
  Submit-Ergebnis gemeinsam. Terminaldarstellung gehört zu `cli`, nicht zur Report-Auslösung.
- [ADR-0009](../adr/0009-session-transports-report-observations.md) hält Markertransport,
  Vorrangregel und Report-Auslösung fest.

### Report-Kontext

- `report::Context` ist eine unveränderliche, reportspezifische Momentaufnahme der beim Session-Start
  wirksamen Daten. `Report::create` erzeugt sie direkt aus `session::Session`. Der Aufrufer liefert
  weder einen Context noch einzelne Metadaten.
- `Context::application` enthält Package, Version, Target, Features, Anwendungsargumente und eine
  optionale `SourceRevision`.
- Package, Target, Features und Anwendungsargumente stammen aus der validierten und wirksamen
  `session::launch::Config`. Die Reihenfolge der Anwendungsargumente bleibt erhalten.
- Die Anwendungsversion ist `package.version` des ausgewählten Cargo-Packages. Die Session ermittelt
  sie beim Start mit `cargo metadata --format-version 1 --no-deps`. Sie verwendet weder die
  `bug_hunter`-Package-Version noch einen Git-Tag als Anwendungsversion. Kann Cargo das Package oder
  seine Version nicht eindeutig auflösen, startet die Session nicht.
- `SourceRevision` enthält den Commit aus `HEAD` und den Dirty-Status des Git-Repositorys, das das
  Manifest des ausgewählten Packages enthält. Dirty umfasst vorgemerkte, geänderte und nicht
  ignorierte unversionierte Dateien. Fehlt Git oder besitzt das Repository kein `HEAD`, ist
  `Application::source` `None`.
- `Context::bug_hunter_version` stammt aus dem Build der laufenden Host-Implementierung.
- `protocol_version` und `capabilities` sind die durch den erfolgreichen `Ready`-Handshake
  bestätigten Werte. `tick` ist die beim Start wirksame `command::tick::Config`.
- `Platform` enthält ausschließlich OS und Architektur des laufenden Host-Builds.
- `Toolchain` enthält die beim Session-Start im selben Arbeitsordner abgefragten und getrimmten
  Cargo- und Rustc-Versionsausgaben. Beide Werte sind optional. Eine fehlgeschlagene Zusatzabfrage
  verhindert weder Session-Start noch Report-Erzeugung.
- Der Context enthält höchstens die letzten 50 gesendeten Commands samt korreliertem Outcome in
  Sendereihenfolge. Wire-Request-IDs werden nicht übernommen. Ein noch ausstehender Command bleibt
  als `history::Outcome::Unanswered` sichtbar.
- Anwendungsargumente, Commands und Outcomes werden unverändert übernommen. Dazu gehören
  Texteingaben und Inspect-Outcomes. Reports sind deshalb vertrauliche Artefakte. Ein externer
  Provider darf sie nur nach einer ausdrücklichen Veröffentlichungsentscheidung senden. R6 muss
  diese Regel in das GitHub-Provider-Interface aufnehmen.
- Der Context enthält keine absoluten Projekt-, Manifest- oder Artifact-Pfade, Umgebungsvariablen,
  Repository-URL, Branch, Quelltext-Diffs, Session-, Controller- oder Wire-Request-IDs,
  Provider-Konfiguration, vollständigen Cargo-Abhängigkeitsgraphen oder Bevy-World-Snapshot.
- Die Session nimmt die unveränderlichen Metadaten beim Start auf. Ein späterer Wechsel von Branch
  oder Toolchain und spätere Änderungen an ursprünglichen Konfigurationsobjekten verändern den
  Context der laufenden Session nicht.
- Die genaue Auswahl und ihre Gründe stehen in
  [`research/report-context-sources.md`](research/report-context-sources.md).
  [ADR-0007](../adr/0007-report-context-is-a-session-snapshot.md) hält die Entscheidung fest.

#### Prüfung des Report-Kontexts

- Workspace-Fixtures decken direkte und geerbte Package-Versionen sowie die Auswahl des
  konfigurierten Packages und Targets ab.
- Git-Fixtures decken saubere und geänderte Worktrees, unversionierte Dateien, fehlendes Git und ein
  Repository ohne `HEAD` ab.
- Context-Fixtures decken null, genau 50 und mehr als 50 Commands, ihre Sendereihenfolge,
  korrelierte Outcomes, `Unanswered` und das Fehlen von Wire-Request-IDs ab.
- Weitere Fixtures prüfen die unveränderte Übernahme von Anwendungsargumenten, Commands und Outcomes
  sowie den Ausschluss von Umgebung, absoluten Pfaden, Provider-Konfiguration und Quelltext-Diffs.
- Fehlschlagende optionale Git-, Cargo-Versions- und Rustc-Versionsabfragen verhindern die
  Report-Erzeugung nicht.

### Provider-Ausführung

- `provider::Config::Local(local::Config {})` schreibt ausschließlich einen lokalen
  Markdown-Report unter dem konfigurierten relativen Ausgabeverzeichnis.
- `provider::Config::Github(github::Config {})` ist die ausdrückliche Entscheidung für eine direkte
  GitHub-Veröffentlichung. `github::Config` besitzt keine Felder.
- `report::submit` liest Provider und Ausgabeverzeichnis aus dem unveränderlichen Session-Zustand.
  Es nimmt keine zweite `report::Config` entgegen. Eine lokal gestartete Session kann deshalb nicht
  erst beim Submit-Aufruf auf GitHub-Veröffentlichung umgestellt werden.
- Der GitHub-Provider führt `gh` im kanonischen Elternordner des konfigurierten Cargo-Manifests aus.
  Er übergibt kein `--repo` und setzt keine eigenen Repository-, Host- oder
  Authentifizierungswerte. Lokaler Git-Kontext und die von `gh` unterstützte Umgebung bestimmen
  Repository, Host und Anmeldung.
- Die Issue-Erstellung verwendet `Report::title` unverändert. Der Provider fügt weder einen
  GitHub-spezifischen Titelpräfix noch Labels, Assignees oder Milestones hinzu.
- Der Provider übergibt den Markdown-Body nicht interaktiv über stdin. Bei erfolgreicher Suche oder
  Erstellung entsteht keine lokale Zwischendatei.
- Der Body enthält eine eigene Zeile in dieser Form:

```markdown
<!-- bug_hunter-signature: v1:sha256:<64 kleingeschriebene Hex-Zeichen> -->
```

- Die Duplikatsuche vergleicht ausschließlich den vollständigen Marker. Titel, Zeitangaben und
  übriger Body-Inhalt bestimmen die Identität nicht.
- Die Suche umfasst alle paginierten offenen und geschlossenen Issues. Ein Treffer erzeugt
  `provider::Outcome::Existing` mit Issue-Nummer und URL. Der Provider kommentiert oder verändert
  das Issue nicht und öffnet ein geschlossenes Issue nicht erneut.
- Ohne Treffer erstellt der Provider das Issue und gibt `provider::Outcome::Created` mit der
  zurückgegebenen URL aus.
- GitHub besitzt keine atomare Eindeutigkeitsbedingung für den Marker. Zwei gleichzeitige
  Veröffentlichungen können deshalb trotz vorheriger Suche zwei Issues erstellen. Die
  Duplikatsuche garantiert die Wiederverwendung bei sequenziellen Aufrufen.
- Fehlendes `gh`, fehlende Anmeldung, ein nicht auflösbares Repository, Netzwerkfehler und Fehler
  bei Suche oder Erstellung lösen den lokalen Rückfall aus.
- Eigene Repository-, Host-, Authentifizierungs- oder Token-Felder und weitere Issue-Metadaten
  bleiben außerhalb dieses Umbaus. Eine spätere Erweiterung benötigt eine getrennte Entscheidung
  und eigene Tests.

#### Lokaler Zielpfad

- `provider::FileReference::path` ist ein `PathBuf` relativ zum Session-Artifact-Verzeichnis.
- Der lokale Provider bildet den endgültigen Pfad als
  `<report.output>/v1-sha256-<digest>.md`. Version, Algorithmus und Digest stammen aus der
  vollständigen Signatur. Der Titel gehört nicht zum Dateinamen.
- Bereits vorhandene Verzeichniskomponenten unterhalb des Artifact-Verzeichnisses und eine
  vorhandene Zieldatei dürfen keine Symlinks sein. Ein solcher Pfad ergibt
  `local::Error::InvalidPath`.
- Fehlt die Zieldatei, schreibt der Provider den vollständigen Markdown-Report zunächst in eine neue
  temporäre Datei in demselben Verzeichnis. Danach setzt er sie ohne Überschreiben am endgültigen
  Pfad ein.
- Enthält eine vorhandene Zieldatei denselben vollständigen Signatur-Marker, ergibt der Aufruf
  `provider::Outcome::Existing`.
- Fehlt der Marker oder enthält die Datei eine andere Signatur, ergibt der Aufruf
  `local::Error::Conflict`. Der Provider überschreibt die Datei nicht.
- Gewinnt ein paralleler Aufruf das Rennen um denselben endgültigen Pfad, liest der unterlegene
  Aufruf die eingesetzte Datei erneut. Dieselbe Signatur ergibt `Existing`, ein fehlender oder
  anderer Marker `Conflict`.
- Der direkt konfigurierte lokale Provider und der lokale Rückfall nach einem Remote-Fehler
  verwenden dieselbe Pfadbildung und Duplikatregel.
- Technische Quellen, der genaue Ablauf und die Abweichungen vom aktuellen Code stehen in
  [`research/native-gh-provider.md`](research/native-gh-provider.md).
  [ADR-0008](../adr/0008-configured-provider-with-local-fallback.md) hält die Entscheidung fest.

#### Prüfung der Provider-Ausführung

- Lokale Fixtures prüfen neue und bereits vorhandene Signaturen sowie sichere relative Pfade.
- Config-Fixtures lehnen einen leeren oder absoluten `report.output` und die Bestandteile `.` und
  `..` beim Session-Start ab.
- Pfad-Fixtures lehnen Symlinks in vorhandenen Verzeichniskomponenten und als Zieldatei ab.
- Eine Parallelitäts-Fixture prüft zwei gleichzeitige Veröffentlichungen derselben Signatur. Sie
  erzeugen genau eine Datei und liefern `Created` sowie `Existing`.
- Ein Compile-Fail-Test belegt, dass `report::submit` keine zweite Report-Konfiguration annimmt.
- Eine mit lokalem Provider gestartete Session bleibt bei lokaler Speicherung. Eine mit
  GitHub-Provider gestartete Session verwendet GitHub und bei dessen Fehler den lokalen Rückfall.
- GitHub-Command-Fixtures prüfen den Projektordner, das fehlende `--repo`, stdin als Body-Quelle und
  den unveränderten Report-Titel.
- GitHub-Antwort-Fixtures prüfen exakte Marker in offenen und geschlossenen Issues, Pagination,
  abweichende Titel und veränderten übrigen Body-Inhalt.
- Fehler-Fixtures prüfen jeden Remote-Fehler mit erfolgreichem lokalem Rückfall sowie einen
  zusätzlichen Fehler beim lokalen Schreiben.

### Report- und Provider-Fehler

- `Report::create` bleibt unfehlbar. `report::Error` beschreibt ausschließlich Fehler von
  `report::submit` beim Provider-Aufruf oder lokalen Speichern. Beobachtete Panics, Tracing-Errors
  und Prozessenden bleiben `report::Failure`.
- `report::Error::Local(local::Error)` bedeutet, dass der direkt konfigurierte lokale Provider den
  Report nicht speichern konnte.
- `report::Error::FallbackFailed { provider, local }` bedeutet, dass zuerst der konfigurierte
  Remote-Provider und danach der lokale Rückfall fehlgeschlagen sind. Beide typisierten Ursachen
  bleiben erhalten.
- Ein Remote-Fehler mit erfolgreichem lokalem Rückfall ist kein `report::Error`.
  `report::submit` gibt `Ok(provider::Outcome::Fallback { reference, provider_error })` zurück. Die
  Referenz hat den Typ `provider::FileReference`.
- `provider::Error` besitzt für jeden Remote-Provider eine eigene Variante. In diesem Umbau ist nur
  `provider::Error::Github(github::Error)` vorgesehen.

#### GitHub-Fehler

- `github::Error` unterscheidet `Unavailable`, `CommandFailed` und `InvalidResponse`.
- Jede Variante enthält `operation: github::Operation` mit `Search` oder `Publish`.
- `Unavailable` bedeutet, dass der `gh`-Prozess für die Operation nicht gestartet werden konnte.
- `CommandFailed` bedeutet, dass `gh` gestartet wurde, die Operation aber mit einem Fehler beendete.
- `InvalidResponse` bedeutet, dass `gh` einen erfolgreichen Status, aber keine gültige erwartete
  Such- oder Veröffentlichungsantwort lieferte.
- Jede Variante enthält eine menschenlesbare `message`. Ihr genauer Text ist nicht stabil und darf
  keine Controller-Logik steuern.
- `bug_hunter` errät aus stderr keine Kategorien für Anmeldung, Netzwerk, Repository-Auflösung oder
  Berechtigung. Diese Ursachen bleiben `CommandFailed` bei der jeweiligen Operation.

#### Lokale Speicherfehler

- `local::Error::InvalidPath { path }` meldet einen zur Veröffentlichungszeit unsicheren Pfad, etwa
  einen Symlink unterhalb des Session-Artifact-Verzeichnisses. Statisch ungültige Bestandteile von
  `report.output` lehnt bereits `Session::start` als `session::Error::InvalidConfig` ab.
- `local::Error::Conflict { path }` meldet, dass am signaturbasierten Zielpfad bereits eine Datei mit
  anderer oder ungültiger Signatur liegt. Der Provider überschreibt sie nicht.
- `local::Error::Filesystem { operation, path, message }` meldet einen Dateisystemfehler.
  `local::Operation::Read` umfasst das Lesen einer vorhandenen Datei zur Duplikatprüfung.
  `local::Operation::Write` umfasst Verzeichniserzeugung, temporäres Schreiben und das Einsetzen der
  endgültigen Datei.
- Alle `path`-Felder von `local::Error` verwenden `PathBuf`. Sie bewahren dadurch auch Pfade, die
  nicht als UTF-8 darstellbar sind. `Display` verwendet die menschenlesbare Darstellung des Pfads.
- `message` ist eine Diagnose für Menschen und kein stabiler maschinenlesbarer Fehlercode.
  Betriebssystemspezifische Systemaufrufe erhalten keine eigenen öffentlichen Varianten.
- Alle öffentlichen Fehlertypen implementieren `Display` und `std::error::Error`.

#### Prüfung der Fehler

- Ein ungültiger Ausgabepfad, eine kollidierende vorhandene Datei sowie Fehler beim Lesen und
  Schreiben erzeugen die jeweilige `local::Error`-Variante.
- Pfad-Fixtures mit nicht als UTF-8 darstellbaren Bestandteilen bleiben in `local::Error` verlustfrei
  erhalten, soweit die jeweilige Testplattform solche Pfade unterstützt.
- Fehlender oder nicht startbarer `gh`, ein fehlgeschlagener Such- oder Veröffentlichungsbefehl und
  eine ungültige Erfolgsantwort erzeugen die jeweilige `github::Error`-Variante samt Operation.
- Ein erfolgreicher lokaler Rückfall liefert `Ok(Fallback)` und erhält den vollständigen
  `provider::Error`. Seine Referenz ist bereits durch den Typ auf eine lokale Datei begrenzt.
- Ein fehlgeschlagener lokaler Rückfall liefert `Err(FallbackFailed)` und erhält Provider- und
  lokalen Fehler.
- Tests prüfen `Display` und `Error::source`, ohne den genauen Text einer
  Betriebssystemfehlermeldung als stabilen Vertrag festzuschreiben.

#### Prüfung der Report-Konstruktion

- Compile-Fail-Tests belegen, dass Aufrufer `Report`, `Failure` und `Signature` nicht über ihre
  Felder konstruieren oder verändern können.
- Konstruktor-Fixtures prüfen die Zuordnung von Panic, Tracing-Error und Prozessende zu `Origin` und
  `message`.
- Nur `Report::create` berechnet Titel, Signatur und Context. Fixtures vergleichen seine
  Zugriffsmethoden mit den gemeinsam erzeugten Werten.

### Report-Titel

`Report::create` leitet den Titel aus dem Wert von `Failure::message` ab, sofern er vorhanden ist:

1. CRLF und alleinstehendes CR werden zu LF.
2. Ein ANSI-Parser entfernt vollständige Escape-Sequenzen.
3. Der Titel verwendet die erste Zeile, die nach dem Entfernen von führendem und abschließendem
   ASCII-Whitespace nicht leer ist.
4. Großschreibung, Unicode und interner Whitespace dieser Zeile bleiben erhalten.
5. Der Titel enthält höchstens 120 Unicode-Skalarwerte. Eine längere Zeile wird nach 117
   Unicode-Skalarwerten abgeschnitten und um `...` ergänzt.
6. Fehlt die Meldung oder besitzt sie keine verwendbare Zeile, lautet der Titel abhängig von der
   Fehlerart `panic`, `tracing error` oder `process exited unexpectedly`.

Der Titel erhält keinen `bug_hunter`- oder providerspezifischen Präfix. Die Titelbereinigung verändert
weder `Failure::message` noch die für die Signatur getrennt bereinigte Kopie. Der GitHub-Provider
übernimmt den so erzeugten Titel unverändert.

#### Prüfung des Report-Titels

- Fixtures prüfen LF, CRLF, alleinstehendes CR, ANSI-Sequenzen und leere Anfangszeilen.
- Titel mit genau 120 und mehr als 120 Unicode-Skalarwerten prüfen die Grenze, die
  UTF-8-Zeichengrenze und das abschließende `...`.
- Eine fehlende Meldung, leere Meldungen und Meldungen aus ausschließlich ASCII-Whitespace prüfen
  die drei festen Fallback-Titel.
- Mehrzeilige Meldungen bleiben vollständig in `Failure::message`; nur der Titel verwendet ihre
  erste nicht leere Zeile.

### Markdown-Darstellung

`Report::to_markdown` erzeugt die einzige Markdown-Darstellung eines Reports. Der lokale Provider
schreibt diesen String unverändert in die Report-Datei. Der GitHub-Provider verwendet denselben
String unverändert als Issue-Body und übergibt `Report::title` zusätzlich als Issue-Titel.

Der Markdown-Report enthält diese Abschnitte in fester Reihenfolge:

1. `Report::title` als Überschrift erster Ebene,
2. den vollständigen Signatur-Marker
   `<!-- bug_hunter-signature: v1:sha256:<digest> -->`,
3. `Failure` mit Fehlerart, vorhandener Meldung und den zur Fehlerart gehörenden Diagnosen,
4. `Application` mit Package, Version, Target, Features, Argumenten und optionaler Git-Revision,
5. `Environment` mit `bug_hunter`-Version, Protokollversion, Capabilities, Tick-Konfiguration,
   Plattform und Toolchain,
6. `Commands` mit dem begrenzten History-Snapshot und seinen Outcomes.

`Failure` zeigt für einen Panic Code-Stelle und Backtrace, für einen Tracing-Error Target und
Code-Stelle und für ein Prozessende den Status. Eine fehlende Meldung oder optionale Diagnose wird
ausdrücklich als nicht verfügbar dargestellt. Der Renderer erfindet keinen Ersatzwert.

Meldung und Backtrace stehen in Text-Codeblöcken. Strukturierte Anwendungs-, Umgebungs- und
Command-Daten stehen als eingerücktes JSON in Codeblöcken. Der Renderer verwendet für jeden
Codeblock eine Begrenzung aus mindestens drei Backticks, die länger als jede zusammenhängende Folge
von Backticks in den enthaltenen Nutzdaten ist. Dadurch können Meldungen, Argumente und Outcomes
keinen Codeblock vorzeitig schließen.

Die Darstellung verwendet LF und endet mit genau einem LF. Sie enthält keinen
Erzeugungszeitpunkt und keine Provider-Konfiguration oder Provider-Fehler. Sie kürzt Meldung,
Backtrace, Argumente, Commands und Outcomes nicht. Lehnt GitHub einen zu großen Body ab, entsteht ein
Provider-Fehler und `report::submit` führt den lokalen Rückfall mit dem vollständigen Markdown aus.

#### Prüfung der Markdown-Darstellung

- Eine Golden Fixture schreibt Überschriften, Abschnittsreihenfolge, Signatur-Marker und
  abschließendes LF fest.
- Je eine Fixture für Panic, Tracing-Error und Prozessende prüft die zugehörigen Diagnosefelder.
- Fehlende Meldung, Code-Stelle, Backtrace, Target und Toolchain-Werte bleiben ausdrücklich als
  nicht verfügbar sichtbar.
- Nutzdaten mit Backticks, Markdown, ANSI-Sequenzen, Unicode und mehreren Zeilen schließen keinen
  Codeblock und verschwinden nicht aus dem Report.
- Fixtures mit null und 50 History-Einträgen prüfen die vollständige eingerückte JSON-Darstellung.
- Lokaler Provider und GitHub-Command-Fixture erhalten bytegleiches Markdown.
- Eine simulierte Ablehnung wegen der Body-Größe führt zum lokalen Rückfall, dessen Datei weiterhin
  den vollständigen Report enthält.

### Fehlersignatur

- `report` leitet die Fehlersignatur aus den beobachteten Fehlerdaten ab. Der Nutzer
  konfiguriert weder Fehlercodes noch Regeln für einzelne Fehlermeldungen.
- `Signature::value` besitzt die Form `v1:sha256:<digest>`. Der Digest besteht aus genau 64
  kleingeschriebenen Hex-Zeichen.
- Der Titel wird aus der Fehlermeldung für Menschen erzeugt. Er ist weder Eingabe noch
  persistierte Ersatzdarstellung der Signatur.
- Der Report bewahrt eine vorhandene ursprüngliche Meldung und die Backtrace-Ausgabe unabhängig von
  dieser Normalisierung als Diagnose auf.
- Provider speichern und vergleichen die vollständige Signatur einschließlich Version und
  Algorithmus. Ändert sich die Normalisierung oder Feldbelegung, erhält der Vertrag eine neue
  Signaturversion. Bereits persistierte Reports werden nicht mit einer neueren Version
  nachberechnet.

#### Meldungsnormalisierung

`Report::create` normalisiert eine Kopie der vorhandenen `Failure::message` ausschließlich für die
Signatur. Fehlt die Meldung bei einem Panic mit unbekanntem Payload, verwendet das weiterhin
vorhandene Signaturfeld `message` den leeren String:

1. CRLF und alleinstehendes CR werden zu LF.
2. Ein ANSI-Parser entfernt vollständige Escape-Sequenzen. Eine reguläre Expression, die nur CSI-
   Farbcodes kennt, reicht dafür nicht.
3. Der absolute Projektwurzelpfad der gestarteten Cargo-Session wird in beiden
   Verzeichnistrenner-Schreibweisen durch `<project>` ersetzt.
4. Eigenständige ASCII-Tokens der Form `0[xX][0-9a-fA-F]{8,16}` werden durch `<address>` ersetzt.
   Vor und nach dem Token darf kein ASCII-Buchstabe, keine Ziffer und kein Unterstrich stehen. Das
   schließt übliche 32- und 64-Bit-Adressen ein. Andere Hexwerte und Dezimalzahlen bleiben erhalten.
5. Klar erkennbare ISO-Datums- und Uhrzeitangaben werden durch `<timestamp>` ersetzt. Erkannt werden
   gültige eigenständige ASCII-Werte in den Formen `YYYY-MM-DD`, `HH:MM:SS` mit optionalem
   Sekundenbruchteil und Zeitzone sowie die Kombination beider Werte mit `T` oder einem Leerzeichen.
   Kalenderdatum, Uhrzeit und numerischer UTC-Offset müssen gültig sein. Vor und nach dem Wert darf
   kein ASCII-Buchstabe, keine Ziffer und kein Unterstrich stehen. Die längste kombinierte Form wird
   zuerst ersetzt.
6. Führender und abschließender ASCII-Whitespace wird entfernt. Interner Whitespace, Großschreibung,
   Unicode und übrige Zahlen bleiben unverändert.

Diese Regeln entfernen nur bekannte technische Schwankungen. Unix-Zeitstempel, Entity-IDs oder
andere anwendungsspezifische Werte werden nicht anhand allgemeiner Zahlenmuster geraten. Wenn eine
Anwendung solche Werte in ihre Fehlermeldung schreibt, bleiben sie Teil der Identität.

Ein unerwartetes Prozessende verwendet vor der Normalisierung immer die feste Meldung
`process exited unexpectedly`. Der beobachtete Status bleibt in `Origin::ProcessExit::status`.
Dadurch verändern unterschiedliche Exit-Codes oder Signale die Signatur nicht.

Ein Tracing-Event verwendet den Wert seines konventionellen Feldes `message`. Fehlt dieses Feld,
entsteht die Report-Meldung aus den beobachteten übrigen Feldern in Deklarationsreihenfolge als
`name=value`, getrennt durch `, `. Primitive Werte verwenden ihre kanonische Marker-Darstellung,
andere Werte die von `Visit::record_debug` beobachtete Darstellung. Besitzt das Event keine Felder,
wird sein Metadatenname verwendet. Diese Meldung durchläuft danach dieselbe Normalisierung.

Die Projektwurzel ist der normalisierte Arbeitsordner, in dem `Session::start` den Cargo-Prozess
startet. Sie ist internes Session-Wissen und kein Teil von `report::Context`.

#### Signaturfelder

Version 1 besitzt für jede Fehlerart genau zwei geordnete Felder:

| Feld | Panic | Tracing-Error | Prozessende |
| --- | --- | --- | --- |
| `kind` | `panic` | `tracing_error` | `process_exit` |
| `message` | bereinigte Report-Meldung | bereinigte Report-Meldung | bereinigte Report-Meldung |

Code-Stelle, Tracing-Target, Prozessstatus, Backtrace, Session, Controller, Command-Verlauf und
Report-Kontext sind ausdrücklich ausgeschlossen. Sie bleiben im Report als Diagnose erhalten.

#### Binärkodierung und Hash

Der SHA-256-Preimage beginnt mit den ASCII-Bytes `bug_hunter.signature`, gefolgt von einem Nullbyte.
Danach folgen die Signaturversion als Big-Endian-`u32`, die Anzahl der Felder als Big-Endian-`u32`
und die beiden Felder in der Reihenfolge `kind`, `message`.

Jedes Feld wird folgendermaßen kodiert:

1. Länge des UTF-8-Feldnamens als Big-Endian-`u32`,
2. Bytes des Feldnamens,
3. UTF-8-Bytelänge des Werts als Big-Endian-`u64`,
4. Bytes des Werts.

Längenpräfixe verhindern Mehrdeutigkeiten durch Trennzeichen in der Meldung. Rusts `DefaultHasher`
wird nicht verwendet, weil sein Algorithmus und Seed kein persistierter Vertrag sind.

Der Digest wird mit SHA-256 berechnet und als kleingeschriebener Hexwert hinter `v1:sha256:`
gespeichert.

#### Golden Vectors

Die Werte in dieser Tabelle sind verbindliche Fixtures. Die Meldungen sind bereits normalisiert:

| Fehlerart | Eingabefelder | `Signature::value` |
| --- | --- | --- |
| Panic | `kind = "panic"`, `message = "stellar catalog invariant violated"` | `v1:sha256:e539c36fcbeb8357ba855f705c08274f29e82d866288dffaa1406607eba9302c` |
| Panic ohne lesbare Payload | `kind = "panic"`, `message = ""` | `v1:sha256:f7ad1a8211ca86c794de8c16b456c16006393c974b89e5aa11908ff1f35336a6` |
| Tracing-Error | `kind = "tracing_error"`, `message = "stellar catalog invariant violated"` | `v1:sha256:86a70a6857c6f193019c24bf0d04b01bde9f127e28b5c45c8f021de0c9165e46` |
| Prozessende | `kind = "process_exit"`, `message = "process exited unexpectedly"` | `v1:sha256:7ca80a848b0913a8eb5ab37f815efe34cd2c4bcdc46ad2fd1870c08902c3f240` |

Die Fixture-Matrix prüft zusätzlich:

- unterschiedliche Projektwurzelpfade, ANSI-Sequenzen, Speicheradressen und klar erkennbare
  Zeitangaben ergeben nach der Normalisierung dieselbe Signatur,
- eine andere bereinigte Meldung oder Fehlerart ergibt eine andere Signatur,
- Code-Stelle, Tracing-Target, Prozessstatus, Backtrace, Session, Controller und Command-Verlauf
  ändern die Signatur nicht,
- ein unbekannter Panic-Payload und eine ausdrücklich leere Panic-Meldung ergeben dieselbe Signatur,
- verschiedene Exit-Codes und Signale ergeben für ein unerwartetes Prozessende dieselbe Signatur,
- andere Zahlen wie `HTTP 404` und `HTTP 500` bleiben verschieden,
- dieselben Panic-Daten ergeben mit `panic = "unwind"` und `panic = "abort"` dieselbe Signatur.

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

- R1 zeigt, dass der Prozessstatus allein Panics in detached Bevy-Tasks und nicht gejointen
  Worker-Threads unter `panic = "unwind"` nicht erkennt. Rusts prozessweiter Panic-Hook läuft dagegen
  für Systeme, Tasks und Worker vor dem Fangen oder Abbrechen sowohl mit `unwind` als auch `abort`.
  `session::Plugin` muss deshalb den vorhandenen Hook verketten und synchron einen
  maschinenlesbaren Panic-Marker auf stderr schreiben. Der Marker darf nicht erst über einen
  Hintergrundthread ausgegeben werden. Die technische Prüfung und Fixture-Matrix stehen in
  [`research/bevy-panic-observability.md`](research/bevy-panic-observability.md).
- R2 legt fest, dass `session` vor jedem Cargo-Start `RUST_BACKTRACE=1` setzt und
  `session::Plugin` im Panic-Hook zusätzlich `Backtrace::force_capture` verwendet. Der
  maschinenlesbare Marker enthält den Erfassungsstatus und bei Erfolg die unveränderte
  `Display`-Ausgabe. Payload und Panic-Stelle bleiben getrennte strukturierte Felder. Stabiles Rust
  garantiert weder genaue oder buildübergreifend stabile Frames noch einen strukturierten
  Frame-Zugriff. R4 schließt den Backtrace deshalb aus der Signatur aus. Er bleibt als Diagnose im
  Report erhalten. Quellen und Fixtures stehen in
  [`research/rust-backtrace-capture.md`](research/rust-backtrace-capture.md).
- R3 zeigt, dass formatierter stderr-Text keine verlässliche Event-Grenze besitzt. Die Erkennung
  erfolgt deshalb durch `session::tracing_error_layer` vor dem Formatter. Der Layer verarbeitet nur
  dispatchte Events mit `Level::ERROR` und übermittelt normalisierte Metadaten sowie strukturierte
  Felder in einem internen Marker. Direkte stderr-Ausgabe, Error-Spans ohne Event und gefilterte
  Events lösen keinen Tracing-Report aus. Eigene Formatter einschließlich ANSI-Ausgabe bleiben
  möglich. Bei aktiviertem `report.tracing_errors` muss `Session::start` die Layer-Registrierung beim
  Session-Start bestätigen. Quellen und Fixtures stehen in
  [`research/bevy-tracing-error-observability.md`](research/bevy-tracing-error-observability.md).
- R4 legt `v1:sha256:<digest>` als persistierte Signatur fest. SHA-256 verarbeitet die oben
  beschriebene versionierte Binärkodierung aus Fehlerart und bereinigter Meldung. Alle zusätzlichen
  Diagnosewerte bleiben ausgeschlossen.
  [ADR-0006](../adr/0006-versioned-failure-signatures.md) hält die Entscheidung fest.
- R5 legt `report::Context` als unveränderliche Momentaufnahme der beim Session-Start wirksamen
  Anwendungs-, Quell-, Toolchain-, Handshake- und Verlaufsdaten fest. Die Anwendungsversion stammt
  aus Cargo-Metadaten. Reports übernehmen Anwendungsargumente und Diagnosewerte unverändert und
  gelten als vertrauliche Artefakte.
- R6 legt die direkte Ausführung des konfigurierten Providers fest. Der GitHub-Provider verwendet
  natives `gh`, findet offene und geschlossene Issues über den Signatur-Marker und verändert Treffer
  nicht. Jeder Remote-Fehler löst den lokalen Markdown-Rückfall aus und bleibt im Outcome sichtbar.
  [ADR-0008](../adr/0008-configured-provider-with-local-fallback.md) hält die Entscheidung fest.
- R7 trennt lokale Speicherfehler, GitHub-Provider-Fehler und den doppelten Fehlschlag von
  Remote-Provider und lokalem Rückfall. Ein erfolgreicher Rückfall bleibt ein `Outcome` und enthält
  den typisierten Remote-Fehler.

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
- Die heutigen getrennten Fehler aus `session::diagnostics`, `host::issue_report` und `host::github`
  werden durch die nach Provider und Phase typisierten `report`-Fehler ersetzt. Öffentliche
  Betriebssystem- und JSON-Bibliothekstypen bestimmen die Varianten nicht mehr.
- Das Lesen und Weiterleiten von stderr sowie die Beobachtung des Kindprozesses wechseln in die
  interne Prozessverwaltung von `session::Session`. Ein für Panic- und Backtrace-Erkennung nötiger
  Zeilenpuffer bleibt ein privates Implementierungsdetail von `report`; `recent.log` wird nicht als
  separates Diagnoseartefakt fortgeführt.
- Die fortlaufende korrelierte Command-Historie bleibt Eigentum der Session. `report` erhält davon
  nur den für den Report begrenzten Snapshot.
- Die bestehende GitHub-Duplikatsuche vergleicht exakte Titel. Im Ziel verwendet sie die
  normalisierte Fehlersignatur im Issue-Body, damit Titeländerungen und gleiche Titel verschiedener
  Fehler die Zuordnung nicht bestimmen. Die feste Grenze von 1.000 gelisteten Issues entfällt.
- Der aktuelle GitHub-Ablauf schreibt vor jedem Veröffentlichungsversuch einen lokalen Entwurf. Im
  Ziel entsteht die lokale Markdown-Datei nur bei lokalem Provider oder nach einem Remote-Fehler.
- `ReportOptions::create` und der zweistufige aktuelle `github::Report` entfallen. Die Auswahl des
  GitHub-Providers führt bei `report::submit` direkt zur Veröffentlichung.
- Das aktuelle Diagnoseformat besitzt keinen normalisierten Panic-Backtrace als fachlichen Teil der
  Fehlerbeschreibung. Die neue Report-Erzeugung muss Panic-Ausgabe, Code-Stelle und Backtrace
  gemeinsam erfassen, ohne flüchtige Backtrace-Daten zur Duplikatidentität zu machen.
- Die vorhandenen Console-, JSON- und GitHub-Darstellungen dürfen als technische Ausgangsbasis
  dienen. Sie bestimmen nicht die Struktur des providerunabhängigen Reports.

## Session Protocol

### Festgelegtes Ziel

- `session::protocol` besitzt den versionierten Wire-Vertrag zwischen `session::Session` und
  Controlled Application. Die fachlichen Commands bleiben im jeweiligen `command`-Modul; das Protokoll besitzt nur
  deren Wire-Abbildung, die Nachrichtenhüllen und die Request-Korrelation.
- Bevy Remote und JSON-RPC gehören nicht zum Wire-Vertrag. Der interne Inspect-Adapter wird nicht
  durchgereicht: Die Wire-Nachrichten enthalten weder `jsonrpc`, BRP-Methodennamen und `params` noch
  BRP-IDs oder numerische BRP-Fehlercodes. `session::protocol` bleibt der einzige Owner der
  transportierten Request-ID, Command-Namen, Outputs und Fehlerformen.
- Das Ziel ist Protokollversion 3 und nicht kompatibel mit dem aktuellen Protokoll v2. `session`
  sendet keine Requests, bevor er genau eine `Ready`-Nachricht mit der erwarteten Version und die für
  die Report-Konfiguration nötige Layer-Bestätigung erhalten hat.
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
- Die internen Panic-, Tracing- und Layer-Registrierungsmarker werden ausschließlich über stderr
  beobachtet. Sie sind keine v3-Protokollnachrichten und ergänzen weder `Ready` noch
  `session::Capabilities`. Wenn `report.tracing_errors` aktiviert ist, gehört die Bestätigung des
  Error-Layers zur hostseitigen Startprüfung und nicht zum stdout-JSONL-Codec.
- `session::Plugin` schreibt vor Abschluss des Ready-Handshakes immer einen internen
  Layer-Statusmarker. `Session::start` verarbeitet Statusmarker und stdout-`Ready` unabhängig von
  ihrer beobachteten Reihenfolge. Eine negative Bestätigung ergibt bei aktiviertem
  `report.tracing_errors` einen `session::Error::Launch`; bei deaktivierter Beobachtung verhindert
  ein fehlender Layer den Start nicht.
- Nur Input, Tick, Inspect, Screenshot und Shutdown überschreiten den Transport zur Controlled
  Session. Recording und Replay werden von `session` ausgeführt und sind keine Wire-Commands.
- Rust verwendet typisierte Command-Enums. Der Protokoll-Codec bildet jeden konkreten Command auf
  einen vollständig qualifizierten Namen wie `input.keyboard.press`, `tick.warp.stop` oder
  `inspect.query` und ein immer vorhandenes `arguments`-Objekt ab.
- Protokollversion 3 besitzt genau diese Wire-Command-Namen:
  - `input.keyboard.press`
  - `input.keyboard.release`
  - `input.pointer.press`
  - `input.pointer.release`
  - `input.pointer.move_to`
  - `input.pointer.move_by`
  - `input.pointer.scroll`
  - `input.text.input`
  - `tick.warp.start`
  - `tick.warp.set_pace`
  - `tick.warp.stop`
  - `inspect.query`
  - `screenshot.capture`
  - `shutdown`
- Recording und Replay verwenden als hostseitige Commands dieselbe äußere Command-Form, gehören
  aber nicht zu diesen Wire-Commands.
- `Session` vergibt für jeden angenommenen Command eine numerische Request-ID. Ein Controller
  liefert keine eigene ID. Für einen Wire-Command verwendet das Protokoll dieselbe ID; für einen
  hostseitigen Recording- oder Replay-Command bleibt sie innerhalb der Session-Ausführung.
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
- Zwei fachliche Ablehnungen verwenden denselben stabilen Fehlercode, wenn ein maschineller
  Aufrufer auf beide gleich reagieren kann. Der Code beschreibt die notwendige Reaktion, nicht jede
  interne Bevy-Ursache. Die konkrete Ursache bleibt in `message`. Dadurch erhalten unterscheidbare
  Zustandsfehler wie `key_already_pressed` und `key_not_pressed` getrennte Codes, während etwa ein
  fehlendes und ein mehrdeutiges primäres Keyboard-Fenster gemeinsam
  `keyboard_window_unavailable` verwenden.
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
- Wire-Fixtures prüfen, dass Reporting-Marker auf stderr weder als Protokollnachricht dekodiert noch
  als zusätzliches Ready-Feld oder Capability ausgegeben werden.

Die Wire-Form folgt diesem Muster:

```json
{"request_id":17,"command":"input.keyboard.press","arguments":{"key":"a"}}
{"request_id":17,"command":"input.keyboard.press","status":"completed","output":null}
```

Eine Command-Ablehnung verwendet dieselbe Request-ID:

```json
{"request_id":17,"command":"input.keyboard.press","status":"rejected","error":{"code":"key_already_pressed","message":"keyboard key \"a\" is already pressed"}}
```

Eine nicht zuordenbare Nachricht wird getrennt gemeldet:

```json
{"request_id":null,"status":"protocol_error","error":{"code":"malformed_request","message":"request ID is missing"}}
```

### Ready-Handshake

Die erste Protokollnachricht der Controlled Session ist:

```json
{"status":"ready","version":3,"capabilities":{"screenshot":true}}
```

Ready besitzt weder `request_id` noch `command`. `version` muss genau `3` sein.
`capabilities.screenshot` ist immer als Boolean vorhanden. `false` bedeutet, dass
`screenshot.capture` in dieser Controlled Session nicht ausführbar ist. Unbekannte zusätzliche
Felder gehören nicht zu Protokollversion 3.

Ein ungültiger erster Handshake lässt `Session::start` mit `session::Error::Protocol` fehlschlagen
und räumt den gestarteten Prozess auf. Dabei gelten diese stabilen Codes:

| Code | Bedeutung |
| --- | --- |
| `invalid_ready` | Ein Feld fehlt, besitzt den falschen JSON-Typ oder die Nachricht enthält ein zusätzliches Feld. |
| `unsupported_protocol_version` | `version` ist nicht `3`. |

Eine weitere Ready-Nachricht nach dem erfolgreichen Handshake ist der nicht fatale Protokollfehler
`unexpected_ready`. Die Session läuft weiter und behält den Capability-Snapshot des ersten
Handshakes.

### Command-Abbildungen

#### Keyboard

`input.keyboard.press` und `input.keyboard.release` besitzen jeweils genau ein String-Argument
`key`:

```json
{"request_id":17,"command":"input.keyboard.press","arguments":{"key":"a"}}
{"request_id":18,"command":"input.keyboard.release","arguments":{"key":"a"}}
```

Die bestehenden stabilen Tokens aus `keyboard::Key` bleiben der vollständige unterstützte
Wertebereich. Die Wire-Fixtures decken jedes Token mindestens einmal ab. Beide Commands liefern bei
Erfolg `output: null`.

Für Keyboard gelten diese Ablehnungscodes:

| Code | Bedeutung |
| --- | --- |
| `invalid_arguments` | `key` fehlt, ist kein String oder `arguments` enthält ein zusätzliches Feld. |
| `invalid_key` | `key` ist ein String, aber kein unterstütztes stabiles Keyboard-Token. |
| `keyboard_window_unavailable` | Es gibt kein oder mehr als ein primäres Fenster. |
| `key_already_pressed` | Der angegebene Key ist bereits durch Virtual Input gedrückt. |
| `key_not_pressed` | Der angegebene Key ist nicht durch Virtual Input gedrückt. |

#### Pointer-Bewegung

`input.pointer.move_to` setzt die Position innerhalb des Spielfensters. `input.pointer.move_by`
verschiebt den Pointer ausgehend von seiner aktuellen Position:

```json
{"request_id":19,"command":"input.pointer.move_to","arguments":{"position":[320.0,240.0]}}
{"request_id":20,"command":"input.pointer.move_by","arguments":{"delta":[10.0,-5.0]}}
```

`position` und `delta` verwenden logische Pixel. Der Ursprung von `position` liegt links oben im
Spielfenster. Beide Commands liefern bei Erfolg `output: null`. Sie besitzen kein `surface`-Argument.

Eine gültige Zielposition erfüllt `0 <= x < width` und `0 <= y < height` für die logische Größe des
Spielfensters. `move_to` lehnt eine Position außerhalb dieser Grenzen ab. `move_by` berechnet zuerst
die Zielposition und lehnt den Command ab, wenn das Ergebnis außerhalb liegt. Eine Ablehnung
verändert die bisherige Pointer-Position nicht. Positionen werden nicht still auf den Fensterrand
begrenzt.

Für Pointer-Bewegungen gelten diese Ablehnungscodes:

| Code | Bedeutung |
| --- | --- |
| `invalid_arguments` | `position` beziehungsweise `delta` fehlt, ist kein Paar endlicher Zahlen oder `arguments` enthält ein zusätzliches Feld. |
| `pointer_window_unavailable` | Es gibt kein oder mehr als ein Spielfenster. |
| `pointer_location_unavailable` | `move_by` wurde vor einer bekannten Pointer-Position aufgerufen. |
| `pointer_position_out_of_bounds` | Die absolute oder berechnete Zielposition liegt außerhalb des Spielfensters. |

#### Pointer-Buttons

`input.pointer.press` und `input.pointer.release` besitzen jeweils genau ein String-Argument
`button`:

```json
{"request_id":21,"command":"input.pointer.press","arguments":{"button":"left"}}
{"request_id":22,"command":"input.pointer.release","arguments":{"button":"left"}}
```

Die unterstützten Tokens sind `left`, `right` und `middle`. Beide Commands verwenden die aktuelle
Pointer-Position im Spielfenster und liefern bei Erfolg `output: null`.

Für Pointer-Buttons gelten diese Ablehnungscodes:

| Code | Bedeutung |
| --- | --- |
| `invalid_arguments` | `button` fehlt, ist kein String oder `arguments` enthält ein zusätzliches Feld. |
| `invalid_pointer_button` | `button` ist ein String, aber kein unterstütztes Pointer-Button-Token. |
| `pointer_location_unavailable` | Es ist noch keine Pointer-Position im Spielfenster bekannt. |
| `pointer_button_already_pressed` | Der angegebene Button ist bereits durch Virtual Input gedrückt. |
| `pointer_button_not_pressed` | Der angegebene Button ist nicht durch Virtual Input gedrückt. |

#### Pointer-Scroll

`input.pointer.scroll` besitzt genau ein zweidimensionales Argument `delta`:

```json
{"request_id":23,"command":"input.pointer.scroll","arguments":{"delta":[0.0,-2.0]}}
```

`delta[0]` ist die horizontale und `delta[1]` die vertikale Bewegung. Beide Werte stehen in
Bevy-Zeileneinheiten und werden einschließlich ihres Vorzeichens unverändert an Bevy weitergegeben.
Der Command verwendet die aktuelle Pointer-Position und liefert bei Erfolg `output: null`.
`[0.0,0.0]` ist als wirkungsloser Command zulässig.

Für Pointer-Scroll gelten diese Ablehnungscodes:

| Code | Bedeutung |
| --- | --- |
| `invalid_arguments` | `delta` fehlt, ist kein Paar endlicher Zahlen oder `arguments` enthält ein zusätzliches Feld. |
| `pointer_window_unavailable` | Es gibt kein oder mehr als ein Spielfenster. |
| `pointer_location_unavailable` | Es ist noch keine Pointer-Position im Spielfenster bekannt. |

#### Text

`input.text.input` besitzt genau ein String-Argument `text`:

```json
{"request_id":24,"command":"input.text.input","arguments":{"text":"Hello world"}}
```

Der Command liefert bei Erfolg `output: null`. Ein Text darf höchstens 16.384 UTF-8-Bytes
enthalten. Die Grenze zählt Bytes, nicht Unicode-Zeichen. Genau 16.384 Bytes werden angenommen,
16.385 Bytes abgelehnt.

Für Text gelten diese Ablehnungscodes:

| Code | Bedeutung |
| --- | --- |
| `invalid_arguments` | `text` fehlt, ist kein String oder `arguments` enthält ein zusätzliches Feld. |
| `text_too_large` | `text` überschreitet 16.384 UTF-8-Bytes. |
| `text_window_unavailable` | Es gibt kein oder mehr als ein primäres Fenster. |
| `text_focus_unavailable` | Bevy besitzt keinen eindeutigen lebenden Fokus auf eine mit `EditableText` beschreibbare Entity. |

#### Tick-Warp starten

`tick.warp.start` besitzt das ganzzahlige Argument `ticks` und ein optionales Argument `pace`.
Ohne `pace` verwendet der Warp die konfigurierte Standard-Pace:

```json
{"request_id":25,"command":"tick.warp.start","arguments":{"ticks":600}}
{"request_id":26,"command":"tick.warp.start","arguments":{"ticks":600,"pace":{"kind":"as_fast_as_possible"}}}
{"request_id":27,"command":"tick.warp.start","arguments":{"ticks":600,"pace":{"kind":"ticks_per_second","target":60.0}}}
```

Die Response bleibt ausstehend, bis der Warp alle angeforderten Ticks ausgeführt hat oder durch
`tick.warp.stop` beendet wurde. Der erfolgreiche Output unterscheidet beide Fälle mit `outcome`:

```json
{"requested_ticks":600,"executed_ticks":600,"outcome":"completed"}
{"requested_ticks":600,"executed_ticks":42,"outcome":"stopped"}
```

`ticks` muss größer als null sein. `ticks_per_second.target` muss endlich und größer als null sein.

Für den Warp-Start gelten diese Ablehnungscodes:

| Code | Bedeutung |
| --- | --- |
| `invalid_arguments` | `ticks` fehlt, ein vorhandenes `pace` besitzt den falschen JSON-Typ oder `arguments` enthält ein zusätzliches Feld. |
| `invalid_tick_count` | `ticks` hat den Wert `0`. |
| `invalid_pace` | Die Pace-Variante ist unbekannt oder `target` ist nicht endlich und größer als null. |
| `warp_already_running` | Es läuft bereits ein Warp. |

#### Tick-Warp-Pace ändern

`tick.warp.set_pace` besitzt genau ein Argument `pace` und verwendet dieselben beiden Pace-Formen wie
`tick.warp.start`:

```json
{"request_id":28,"command":"tick.warp.set_pace","arguments":{"pace":{"kind":"as_fast_as_possible"}}}
{"request_id":29,"command":"tick.warp.set_pace","arguments":{"pace":{"kind":"ticks_per_second","target":60.0}}}
```

Der Command ändert die konfigurierte Standard-Pace. Während eines laufenden Warp übernimmt er die
neue Pace sofort für die noch ausstehenden Ticks. Ohne laufenden Warp ändert er nur die
Standard-Pace. Beide Fälle sind erfolgreich. Die Response wird gesendet, sobald die Pace übernommen
wurde, und gibt sie im Output zurück:

```json
{"pace":{"kind":"ticks_per_second","target":60.0}}
```

Für die Pace-Änderung gelten diese Ablehnungscodes:

| Code | Bedeutung |
| --- | --- |
| `invalid_arguments` | `pace` fehlt, besitzt den falschen JSON-Typ oder `arguments` enthält ein zusätzliches Feld. |
| `invalid_pace` | Die Pace-Variante ist unbekannt oder `target` ist nicht endlich und größer als null. |

`warp_not_running` ist kein Fehlercode. Eine Pace-Änderung setzt keinen laufenden Warp voraus.

#### Tick-Warp stoppen

`tick.warp.stop` besitzt ein leeres `arguments`-Objekt:

```json
{"request_id":30,"command":"tick.warp.stop","arguments":{}}
```

Wenn ein Warp lief, beendet der Command ihn und liefert:

```json
{"was_running":true}
```

Der ursprüngliche `tick.warp.start` erhält danach seinen terminalen Output mit `outcome: "stopped"`.
Ohne laufenden Warp ist `stop` ein erfolgreicher No-op:

```json
{"was_running":false}
```

Der einzige fachliche Ablehnungscode ist `invalid_arguments`, wenn `arguments` nicht leer ist.
`warp_not_running` ist kein Fehlercode.

#### Inspect-Query

`inspect.query` verwendet die unter "JSON- und Wire-Form" festgelegten Entity- und
Resource-Argumente. Für Inspect gelten diese fachlichen Ablehnungscodes:

| Code | Bedeutung |
| --- | --- |
| `invalid_arguments` | Die JSON-Struktur ist ungültig oder ein Feld passt nicht zur gewählten `source`. |
| `unknown_type_path` | Mindestens ein vom Aufrufer angegebener Type Path lässt sich nicht exakt über Bevys `TypeRegistry` auflösen. |
| `entity_not_found` | Ein ausdrücklich angegebener Entity-Handle existiert nicht. |

Die Validierung aller Type Paths erfolgt vor dem Lesen der World. Ein Fehler lehnt deshalb die
gesamte Query ohne Teilergebnis ab. Fehlende Components und Resources sowie nicht registrierte,
nicht reflektierbare oder nicht serialisierbare Werte bleiben dagegen Teil eines erfolgreichen
Outputs mit dem passenden `Unavailable`-Grund. Eine Query ohne Treffer ist ebenfalls erfolgreich
und liefert `{"items":[]}`.

#### Screenshot aufnehmen

`screenshot.capture` besitzt genau ein String-Argument `path`:

```json
{"request_id":31,"command":"screenshot.capture","arguments":{"path":"screenshots/current.png"}}
```

Der Command nimmt immer das intern aufgelöste primäre gerenderte Spielfenster auf. Er besitzt kein
`target`-Argument und kann kein anderes Fenster auswählen.

Die Response wird erst gesendet, nachdem der GPU-Readback abgeschlossen und die PNG-Datei
vollständig geschrieben wurde:

```json
{"path":"screenshots/current.png","width":1280,"height":720,"overwritten":false}
```

`path` bleibt relativ zum Session-Artifact-Verzeichnis. Eine vorhandene Datei wird ersetzt;
`overwritten` meldet, ob dies geschehen ist. Die Aufnahme führt keinen kontrollierten
Simulationstick aus.

Für Screenshots gelten diese Ablehnungscodes:

| Code | Bedeutung |
| --- | --- |
| `invalid_arguments` | `path` fehlt, ist kein String oder `arguments` enthält ein zusätzliches Feld. |
| `invalid_screenshot_path` | Der Pfad ist absolut, nicht normalisiert, keine `.png`-Datei oder verlässt einschließlich symbolischer Links das Session-Artifact-Verzeichnis. |
| `screenshot_window_unavailable` | Es gibt kein oder mehr als ein primäres gerendertes Spielfenster. |
| `screenshot_unavailable` | Die Screenshot-Capability ist in der Controlled Session nicht installiert. |
| `screenshot_failed` | GPU-Readback, PNG-Erzeugung oder Schreiben der Datei ist fehlgeschlagen. |

Die konkrete technische Ursache eines `screenshot_failed` steht in `message`. Diese Ursachen
erhalten keine getrennten stabilen Codes, weil ein Aufrufer auf sie gleich mit Wiederholen oder
Fehlermeldung reagiert.

#### Shutdown

`shutdown` besitzt ein leeres `arguments`-Objekt:

```json
{"request_id":32,"command":"shutdown","arguments":{}}
```

Der erfolgreiche Output ist `null`. Die Controlled Session schreibt und leert die Response, bevor
sie ihren Prozess beendet. Der einzige fachliche Ablehnungscode auf dem Wire ist
`invalid_arguments`, wenn `arguments` nicht leer ist.

Die hostseitige Prüfung findet vor dem Wire-Command statt. Solange Requests ausstehen oder ein
Recording aktiv ist, sendet `Session::shutdown` keinen Shutdown-Request und gibt stattdessen
`session::Error::ShutdownBlocked` mit `shutdown_commands_pending` oder
`shutdown_recording_active` zurück. Ein laufender Warp muss vorher beendet oder vollständig
abgewartet werden. Shutdown stoppt keine Arbeit implizit.

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
- REPL, Agent und Script erzeugen dieselbe Command-Darstellung aus qualifiziertem Command-Namen
  und `arguments`. Die REPL übersetzt menschliche Eingaben in diese Form, der Agent übergibt strukturierte
  Tool-Argumente und ein Script enthält eine versionierte Liste derselben Einträge.
- `Session::send` nimmt hostseitig ausgeführte Commands sowie Wire-Commands für die Controlled
  Session über dasselbe Interface an. Es vergibt für jeden angenommenen Command eine
  sessionlokale Request-ID. Bei Wire-Commands wird diese ID in den Protokoll-Request übernommen;
  hostseitige Commands werden unter derselben ID lokal ausgeführt.
- `command::Request` ist versiegelt. Nur die von `bug_hunter` definierten konkreten Command-Typen
  können es implementieren und dadurch einen Command mit seinem erwarteten Output-Typ verbinden.
  Eine externe Crate kann keinen abweichenden Output-Typ für einen vorhandenen Command behaupten.
- Die gemeinsame typgelöschte Darstellung `command::Command` implementiert nicht `Request`. Sie
  enthält Commands mit unterschiedlichen Outputs und dient Parsing, History, Recording sowie der
  internen Ausführung. Insbesondere ist ihre Variante `Shutdown` nicht über das allgemeine
  `Session::send` ausführbar; Shutdown beginnt ausschließlich mit `Session::shutdown`.
- `session` besitzt für den Session-Host einen crate-internen Adapter, der bereits validierte
  `command::Command`-Werte über denselben Koordinator annimmt und ihre Outcomes typgelöscht
  bereitstellt. Der Adapter dupliziert keine Annahme-, Korrelations-, History-, Recording- oder
  Fehlerregel. Seine Variante `Shutdown` routet ausschließlich zu `Session::shutdown`.
- Eine crate-interne Aktivitätsbenachrichtigung weckt den Session-Host, wenn ein Command-Ergebnis
  oder Session-Event verfügbar ist. Der Host überführt die Meldung in den gemeinsamen
  Activity-Stream. Das öffentliche Session-Interface erhält kein zweites dynamisches `send`, kein
  `receive_any` und keinen öffentlichen Aktivitätskanal.
- `send` wartet nur auf die Annahme durch den Koordinator. Eine beendete oder technisch nicht mehr
  erreichbare Session und erschöpfte Request-IDs verhindern die Annahme; es entstehen weder
  `Pending` noch ID oder History-Eintrag. Commandspezifische Zustands- und Argumentfehler erhalten
  dagegen zuerst ein `Pending` und werden später terminal.
- `RequestId` ist ein opaker, kopierbarer `u64` mit `as_u64` und `Display`. Die Vergabe beginnt bei 1,
  steigt innerhalb einer Session monoton und verwendet keinen Wert erneut. Interne Replay-Commands
  verwenden denselben Nummernraum; der Zahlenwert besitzt keine fachliche Reihenfolgesemantik.
- `u64::MAX` bleibt für den Wire-Shutdown reserviert. Sind die normalen IDs verbraucht, gibt `send`
  `RequestIdExhausted` zurück. Bereits angenommene Commands laufen weiter und die Session kann den
  reservierten Wert für einen sauberen Shutdown verwenden.
- Nach erfolgreichem Start besitzt ein privater Session-Koordinator Kindprozess, Pipes und
  veränderlichen Session-Zustand. Der synchrone, nicht klonbare `Session`-Handle übergibt ihm
  Commands und empfängt Ergebnisse und Events über interne Kanäle.
- `Session::send` wartet nicht auf den Abschluss, sondern liefert eine nach dem erwarteten Output
  typisierte `Pending`-Referenz. Sie stellt die von der Session vergebene Request-ID und den
  zugehörigen Command nur lesend bereit. `try_receive` und `receive` holen das Ergebnis später ab;
  bereits eingetroffene Responses anderer Requests bleiben bis zu deren Abholung erhalten.
- Angenommene Commands und hostseitige Arbeit laufen unabhängig von weiteren Session-Aufrufen. Ein
  Replay benötigt keine Poll-Schleife des Controllers, um seinen Plan weiter auszuführen.
- Außerhalb eines exklusiv laufenden Replays dürfen Controller beliebig viele Commands einreichen,
  ohne auf terminale Outcomes früherer Commands zu warten. Die gemeinsame Schicht meldet für jeden
  gültigen JSON-Command zuerst `pending` mit Request-ID und qualifiziertem Command-Namen. Später
  folgt genau ein terminales `completed`, `rejected` oder `failed`, ebenfalls mit Request-ID und
  Command-Namen. Pending-Meldungen folgen der Eingangsreihenfolge; terminale Meldungen dürfen
  ungeordnet eintreffen.
- Auch ein commandspezifisch unzulässiger Command, etwa während eines exklusiven Replays, ist bereits
  angenommen. Er erhält `pending` und danach ein terminales `rejected` mit dem zutreffenden stabilen
  Code.
- Während eines Replays ist von außen nur `Replay::Stop` fachlich zulässig. Andere normale
  Requests werden angenommen und anschließend abgelehnt. Die interne Replay-Ausführung reicht
  ihre Plan-Commands weiterhin durch denselben gemeinsamen Ausführungspunkt und erhält für
  jeden davon ein normales `Pending`.
- `try_receive` blockiert nicht und liefert `Ok(None)`, solange genau dieses `Pending` noch kein
  Ergebnis besitzt. Die Methode prüft nur bereits verfügbare interne Nachrichten und führt keine
  blockierende Datei-, Prozess- oder Netzwerkoperation aus. `receive` wartet auf genau dieses
  `Pending`, während der Koordinator andere Responses, Events und hostseitige Arbeit weiter
  verarbeitet. Ergebnisse anderer Requests bleiben für ihre `Pending`-Referenzen erhalten.
- Wird ein `Pending` fallengelassen, verliert der Aufrufer ausschließlich den späteren Zugriff auf
  dessen Ergebnis. Der bereits gestartete Command wird nicht abgebrochen und erhält keinen
  versteckten Stop- oder Ausgleichs-Command. Die Session korreliert eine spätere Response weiterhin,
  schließt den History- und einen aktiven Recording-Eintrag ab und darf den nicht mehr abrufbaren
  Output danach verwerfen. Bis zum terminalen Ergebnis bleibt der Command auch für Shutdown
  ausstehend.
- Nach `try_receive` mit `Some(output)` oder einem terminalen `Rejected`-, `Protocol`- oder
  commandbezogenen `Io`-Fehler ist das geliehene `Pending` terminal ausgelesen. Eine weitere
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
- Die Session hält die letzten 50 angenommenen Commands in einem Ring. Jeder Eintrag entsteht bei
  der Annahme als `Unanswered` und wird an derselben Position terminal ergänzt. Später eintreffende
  Responses verändern die Annahmereihenfolge nicht.
- Wire-Commands, Recording- und Replay-Steuerung, intern vom Replay ausgeführte Plan-Commands und der
  angenommene Shutdown gehören in die History. Vor der Annahme fehlgeschlagene Aufrufe, Events,
  Reports und interne Koordinatornachrichten gehören nicht hinein.
- Die aktive Command-Tabelle bleibt vom History-Ring getrennt. Ein älterer laufender Command kann aus
  dem Ring fallen, bleibt aber bis zur Korrelation empfangbar, für Shutdown ausstehend und für ein
  aktives Recording verfügbar.
- Endet der Transport oder die Controlled Session, bleiben alle noch nicht terminalen Commands in
  der History `Unanswered`. Wartende Aufrufer erhalten `session::Error::Ended`.
- `session::history::Outcome` unterscheidet `Completed`, `Rejected`, `ProtocolFailed`, `IoFailed` und
  `Unanswered`. `IoFailed` erhält die Meldung eines bekannten commandbezogenen Datei- oder
  Flush-Fehlers. Keines dieser Outcomes speichert die Wire-Request-ID.
- `Session` gibt die laufende History nicht über einen öffentlichen Accessor frei. Die öffentlichen
  Typen `history::Entry` und `history::Outcome` erscheinen ausschließlich im begrenzten
  `report::Context`; eine vollständige dauerhafte Aufzeichnung bleibt Aufgabe des Recordings.
- [ADR-0018](../adr/0018-public-requests-are-typed-and-sealed.md) hält die Trennung zwischen dem
  typisierten öffentlichen Request-Interface und der privaten typgelöschten Host-Ausführung fest.
- [ADR-0013](../adr/0013-session-history-is-a-bounded-report-window.md) hält Aufbewahrung,
  Reihenfolge und Sichtbarkeit der History fest.
- Sessionweite Ereignisse werden ohne Callbacks in einer internen Queue gepuffert.
  `Session::try_receive_event` fragt sie nicht blockierend ab; `Session::receive_event` wartet auf
  das nächste Ereignis. `session::Event` unterscheidet `ProtocolError { code, message }`,
  `RecordingFailed { path, message }`, `Failure { failure }`,
  `ObservationError { code, message }` und `Ended { reason }`.
- `EndReason` unterscheidet `ProcessExit { status }`, `TransportClosed { channel }`,
  `TransportFailed { channel, message }` und
  `EventQueueOverflow { capacity, dropped_events }`. `TransportChannel` unterscheidet stdin, stdout
  und stderr.
- Eine Protokollfehlermeldung ohne bekannte Request-ID erzeugt `Event::ProtocolError`, ohne offene
  Requests zu schließen. Ein unerwartetes Prozess- oder Transportende erzeugt genau einmal
  `Event::Ended` und schließt alle offenen `Pending` mit `Error::Ended`. Zuvor leert die Session
  stdout und stderr vollständig und reiht daraus entstehende `Failure`- oder `ObservationError`-
  Events vor `Ended` ein. Nach Entnahme des terminalen Events ergeben weitere Empfangsoperationen
  `Error::Ended`.
- Ein unerwarteter Prozessstatus erzeugt auch bei Status 0 `EndReason::ProcessExit`. Schließt ein
  benötigter Transportkanal bei noch laufendem Prozess, bleibt der Transportgrund primär; der
  Koordinator beendet und sammelt den Prozess, ohne aus diesem eigenen Abbruch einen
  `ProcessExit`-Report zu erzeugen.
- Bereits vor dem Session-Ende terminal eingetroffene Command-Ergebnisse bleiben einmal über ihr
  `Pending` abrufbar. Nur noch offene Commands liefern `Error::Ended` und bleiben in der History
  `Unanswered`.
- Die Queue fasst 256 normale Events und reserviert einen zusätzlichen Platz für `Ended`. Bei
  Überlauf nimmt der Koordinator keine Commands mehr an, beendet den Prozess und zählt weitere
  verlorene Events. Nach den gespeicherten Events folgt `EventQueueOverflow`; dieser Host-Abbruch
  erzeugt keinen Report. Die Kapazität ist nicht konfigurierbar.
- Erfolgreicher `Session::shutdown` und Drop erzeugen kein `Ended`-Event. Nach erfolgreichem Shutdown
  ergeben alle weiteren Session-Operationen einschließlich eines zweiten Shutdowns `Error::Ended`.
- [ADR-0014](../adr/0014-session-end-is-a-terminal-typed-event.md) hält Endgründe, Event-Reihenfolge
  und Überlastungsverhalten fest.
- `ProtocolFailed` löst allein keinen Report aus. Der wartende Aufrufer erhält
  `session::Error::Protocol`, und die History bewahrt das Command-Outcome. Entsteht später durch
  einen Panic, einen aktivierten Tracing-Error oder ein unerwartetes Prozessende ein Report, wird
  der Protokollfehler als Teil des reportspezifischen Diagnosekontexts aufgenommen.

### Auflösung des bisherigen Session-Controllers

- Das Ziel besitzt kein `session::controller`-Modul, keinen `ControllerSession`-Wrapper und kein
  allgemeines `Controller`-Trait. `session::Session` ist der gemeinsame Command-Ausführungsweg und
  kennt keine menschliche, agentische oder geskriptete Controller-Identität. Es unterscheidet nur
  die intern freigegebenen Plan-Commands eines exklusiven Replays von weiteren öffentlichen
  `send`-Aufrufen.
- `session::Session` übernimmt aus dem bisherigen Controller nur allgemeine Session-Verantwortung:
  Start und Handshake, Command-Ausführung, Request-Korrelation, History, Recording, Capabilities,
  Session-Events, Shutdown und Prozesslebenszyklus.
- `cli::repl` besitzt Parsing und Darstellung menschlicher Eingaben, Terminal-Ein-/Ausgabe,
  REPL-spezifische Komfortbefehle und die Anzeige des gemeinsamen Activity-Streams. Ob normalisierte
  Pointer-Koordinaten oder vergleichbare Hilfen erhalten bleiben, wird ausschließlich mit der REPL
  entschieden.
- `cli::script` besitzt nur die versionierte Dateihülle, das Parsing der gemeinsamen
  Command-Einträge, deren asynchrone Einreichung und die Zuordnung der Outcomes zu den
  ursprünglichen Array-Positionen. Es verwendet dieselbe `client`-Seam wie die REPL.
- Agent-Zugang und Modulplatzierung werden in H4 geplant. Ob eine eigene oder eingebundene
  Agent-Laufzeit nötig ist oder ein vorhandener Agent die CLI verwendet, bleibt offen.
  Der Agent besitzt weder Spiel-Session, deren Pending-Arbeit noch Activity-Aufbewahrung.
- `command::replay` besitzt das Lesen der Recording-Einträge, deren Ausführung über die bestehende
  Session, Stop und Abschlussstatus. Replay startet keine eigene Controlled Session mehr und
  vergleicht aufgezeichnete Outcomes nicht mit den neu entstandenen Outcomes.
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

Das öffentliche Session-Interface ist nach S2 abgeschlossen:

- `session::Session` besitzt genau eine laufende Controlled Session, deren Transport, ausstehende
  Commands und Prozesslebenszyklus.
- `Session` ist der synchrone, nicht klonbare Handle zu einem privaten Koordinator. Das öffentliche
  Interface setzt keine Async-Runtime und keinen bestimmten Executor voraus.
- `Session::start(config)` validiert die Session- und Launch-Konfiguration, startet den Cargo-
  Prozess sowie den Koordinator, bindet stdin, stdout und stderr an und wartet auf den
  Ready-Handshake. Eine nutzbare `Session` wird erst zurückgegeben, nachdem eine gültige
  `Ready`-Nachricht mit der erwarteten Protokollversion als erste Protokollnachricht verarbeitet
  wurde.
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
- `session::Error` unterscheidet `InvalidConfig`, `Launch`, `Io`, `InvalidPending`,
  `RequestIdExhausted`, `ShutdownBlocked`, `Rejected`, `Protocol` und `Ended`. Die Varianten sind die
  öffentlichen Fehlergruppen; genauere Betriebssystemfehler und Prozessdiagnosen bleiben
  Implementierungs- beziehungsweise Beobachtungsdaten.
- `InvalidConfig` wird vor dem Prozessstart für ungültige Session- oder Launch-Konfigurationen
  zurückgegeben. `Launch` bedeutet, dass Cargo nicht gestartet werden konnte oder der gestartete
  Prozess ohne Protokollverletzung keinen gültigen Ready-Handshake erreichte. Ein ungültiger
  Handshake oder eine falsche Protokollversion ist stattdessen `Protocol`.
- `Io` bezeichnet eine fehlgeschlagene Betriebssystemoperation beim Start oder bei einer
  commandbezogenen hostseitigen Dateioperation. Ein Fehler oder EOF auf einem benötigten
  Transportkanal ist davon getrennt: vor dem Ready-Handshake verhindert er den Launch, nach dem
  Handshake beendet er die Session mit `Ended`.
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
- `Session::shutdown(&mut self)` beginnt nur, wenn keine Requests mehr ausstehen und der Recorder
  `Idle` ist. Andernfalls liefert es `ShutdownBlocked` mit `shutdown_commands_pending` oder
  `shutdown_recording_active`, ohne die Session zu verändern. Ein laufender Warp oder ein laufendes
  Replay muss ausdrücklich gestoppt oder vollständig empfangen und ein aktives Recording
  ausdrücklich mit `recording::Stop` beendet werden; Shutdown sendet keine versteckten
  Stop-Commands.
- Ein blockierter Shutdown sendet keinen Wire-Request, verbraucht die reservierte Shutdown-ID nicht
  und erzeugt keinen History-Eintrag. Nicht abgeholte Events, vorhandene History und erschöpfte
  normale Request-IDs blockieren ihn nicht.
- Nach Annahme des Wire-Shutdowns ist jeder Ausgang terminal. Nur eine gültige
  `completed`-Response mit `output: null` und ein anschließend erfolgreicher Prozessstatus ergeben
  `Ok(())`. Ablehnung und Protokollfehler werden direkt zurückgegeben; ein selbstständiges Prozess-
  oder Transportende ergibt `Ended`. Die Session räumt den Kindprozess in jedem Fall auf und erlaubt
  keinen zweiten Versuch mit der reservierten ID.
- Nach erfolgreichem Shutdown ist die Session beendet und weitere Operationen ergeben
  `Error::Ended`. Die Verwendung von `&mut self` statt eines konsumierenden Shutdowns verhindert,
  dass eine Blockierung durch anschließenden Drop trotzdem einen harten Abbruch verursacht.
- Shutdown besitzt keinen eingebauten Timeout. Ein später benötigtes Zeitlimit bleibt eine
  ausdrückliche Host-Policy.
- Wird `Session` ohne erfolgreichen `shutdown` fallengelassen, führt Drop keinen fachlich sauberen
  Abschluss aus. Drop fordert den Koordinator zum Abbruch auf. Dieser schließt die Pipes, beendet die
  Prozessgruppe und sammelt den Kindprozess ein, damit keine Prozesse zurückbleiben. Er sendet keinen
  Shutdown- oder Stop-Command, wartet nicht auf ausstehende Commands und garantiert weder
  vollständige Outcomes noch Recording-Abschluss, Report-Erzeugung oder Provider-Ausführung.
- Eine durch Session-Drop veranlasste Prozessbeendigung ist kein unerwarteter Anwendungsfehler und
  löst keinen Report aus. Wer einen sauberen Session-Abschluss benötigt, muss ausdrücklich
  `shutdown` aufrufen.
- [ADR-0017](../adr/0017-session-errors-have-local-or-terminal-scope.md) hält den lokalen oder
  terminalen Geltungsbereich jeder Fehlergruppe, die Shutdown-Grenze und die Drop-Folgen fest.
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
- `launch::Config::manifest_path` nennt verpflichtend ein Cargo-Package- oder Workspace-Manifest.
  `Session::start` löst einen relativen Wert gegen das aktuelle Arbeitsverzeichnis des aufrufenden
  Prozesses auf, verlangt eine vorhandene reguläre Datei und speichert den kanonischen Pfad.
- Der kanonische Elternordner des konfigurierten Manifests ist der unveränderliche `project_dir` der
  Session. `cargo metadata` und `cargo run` erhalten `--manifest-path`; `cargo run` verwendet
  `project_dir` als Arbeitsverzeichnis.
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
- `session::launch` validiert Manifest, Package und Target, bevor ein Prozess gestartet wird. Das
  Erzeugen des konkreten `std::process::Command` bleibt Implementierung und ist kein öffentliches
  Interface.

#### Auswirkungen auf den Ist-Stand

- `session::launch::Spec` wird zu `session::launch::Config`.
- Der bisher implizite Projektbezug über das Arbeitsverzeichnis wird durch `manifest_path` ersetzt.
- Die getrennten Felder `kind` und `target` werden durch `launch::Target::{Binary, Example}` mit dem
  jeweils zugehörigen Namen ersetzt.
- `Spec::command` entfällt aus dem öffentlichen Interface. Die interne Session-Prozessverwaltung
  baut daraus weiterhin den Cargo-Aufruf.
- Die heutige zusätzliche Validierung von `application.package` und `application.target` in
  `host::Config` entfällt. Der private Config-Parser liest die Launch-Konfiguration unter
  `session.launch`; die fachliche Validierung besitzt ausschließlich `session::launch`.

### Session-Konfiguration und Report-Kontext

- `session::Config` enthält alle Werte, die eine Controlled Session und ihr Verhalten konfigurieren.
  Das Ziel besitzt kein zusätzliches öffentliches Host-Konfigurationsmodell.
- `session::Config` enthält genau `launch: launch::Config`, ein konkretes
  `artifact_dir: PathBuf`, `tick: command::tick::Config` und `report: report::Config`.
- `artifact_dir` bezeichnet das einzige Artifact-Root für Screenshot, Recording und Report. Ein
  relativer Wert wird gegen `project_dir` aufgelöst. Ein absoluter Root und ein Root außerhalb des
  Projekts bleiben erlaubt.
- `Session::start` lehnt einen leeren Root und einen Root, der selbst ein Symlink ist, als
  `InvalidConfig` ab. Es legt einen fehlenden Root an, kanonisiert ihn und hält den absoluten Pfad
  während der Session unverändert. Fehler beim Anlegen oder Kanonisieren ergeben `Io`.
- `command::tick::Config` besitzt ausschließlich die Standard-Pace für Warps. Die Zeitkonfiguration
  der Anwendung gehört nicht zur Session-Konfiguration.
- Der private Host-Config-Parser liest diese vollständige `session::Config` aus dem Feld `session`
  des versionierten TOML-Dokuments. `session.artifact_dir` ist der Standardwert; ein CLI-Override
  ersetzt ihn vor `Session::start`. Danach besitzt nur die gestartete Session den wirksamen Pfad.
- Screenshot-, Recording- und Report-Pfade sind relativ zu diesem Session-Artifact-Verzeichnis und
  dürfen es nicht verlassen.
- Der absolute Projekt-, Manifest- und Artifact-Pfad bleibt aus `report::Context` ausgeschlossen.
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
- Die Session hält intern die wirksame Launch- und Tick-Konfiguration, die durch Cargo aufgelöste
  Package-Version, optionale Git- und Toolchain-Angaben, Plattformdaten, die bestätigten
  Handshake-Werte und die korrelierte Command-Historie. Diese Daten werden nicht als zweites
  öffentliches Session-Modell abgebildet.
- `Report::create` kopiert daraus die unter "Report-Kontext" festgelegte Auswahl. Die
  Context-Momentaufnahme enthält höchstens 50 History-Einträge und keine absoluten Pfade,
  Umgebungsvariablen oder Wire-Request-IDs.

### Controlled-Session-Integration

- Die bisherige Bevy-seitige Bedeutung von `client` entfällt. Das Bevy-Plugin implementiert die
  Controlled-Session-Seite des Session-Interfaces und heißt öffentlich `session::Plugin`.
  Das neue Zielmodul `client` bezeichnet dagegen ausschließlich den Zugriff auf den lokalen Server.
  `session::Plugin` bleibt unabhängig von Server-/CLI-Features verfügbar und wird nicht zusätzlich
  an der Crate-Wurzel re-exportiert. Den genauen Feature-Zuschnitt legt H7 fest.
- Diese Integration besitzt das Bevy-Plugin, den Command-Dispatcher, laufende Command-Arbeit und die
  Anbindung an die interne Session-Ein-/Ausgabe. Fachliche Command-Typen bleiben bei `command` und
  die Wire-Abbildung bei `session::protocol`.
- Nach Abschluss der Bevy-Plugin-Initialisierung prüft die Controlled-Session-Integration die
  tatsächlich installierten optionalen Funktionen und sendet das Ergebnis mit `Ready`. Sie meldet
  fachliche Capabilities und legt weder Bevy-Plugin-Namen noch ihre Erkennungslogik offen.
- Die Host-Seite überschreibt für den Kindprozess die reservierte interne Umgebungsvariable
  `BUG_HUNTER_ARTIFACT_DIR` mit dem kanonischen Artifact-Root. `session::Plugin` liest sie einmal vor
  dem Ready-Handshake. Ein fehlender oder ungültiger Wert verhindert Ready und ergibt hostseitig
  `session::Error::Launch`.
- Die Artifact-Umgebungsvariable ist kein öffentliches Konfigurationsinterface, keine Capability und
  kein Feld des v3-Protokolls.
- Die Controlled-Session-Integration startet keinen Renderer und führt keinen öffentlichen Modus
  für rendererfreie oder gerenderte Sessions ein. Rendererabhängige Prüfungen bleiben bei den
  betroffenen Funktionen. Ein späterer rendererfreier Anwendungsfall kann deshalb dieselbe Session-
  und Protokollstruktur mit einer anderen Bevy-Zusammensetzung verwenden.
- JSONL über stdin/stdout bleibt die interne Ein-/Ausgabe zwischen `session::Session` und Controlled
  Application. Das Ziel besitzt kein öffentliches `transport`-Modul, kein öffentliches Transport-Trait
  und keine öffentliche Konfiguration benutzerdefinierter Ein-/Ausgabeadapter.
- Die Controlled-Session-Seite liest intern stdin und schreibt stdout. Der Session-Koordinator besitzt
  intern die entsprechenden Pipes des Kindprozesses. Beide Seiten teilen nur den Codec und die
  Nachrichtentypen aus `session::protocol`, nicht eine gemeinsame Ein-/Ausgabeabstraktion.
- Ein privater Leser nimmt stdin-Nachrichten entgegen, ohne den Bevy-Event-Loop zu blockieren, und
  übergibt dekodierte Requests an die Bevy-Seite. Ausschließlich Systeme im Bevy-Event-Loop greifen
  auf den `World` zu.
- Lang laufende Arbeit erhält pro Event-Loop-Durchlauf ein begrenztes Budget. Ein einzelner interner
  stdout-Writer serialisiert Ready, Responses und Protokollfehler als vollständige
  JSONL-Nachrichten.
- In-Memory-Ein-/Ausgabe bleibt als private oder crate-interne Test-Seam zulässig. Ein Testadapter
  allein begründet kein öffentliches Transport-Interface.
- [ADR-0011](../adr/0011-session-progresses-on-a-private-coordinator.md) hält das Fortschritts- und
  Nebenläufigkeitsmodell fest.
- Die aktuellen öffentlichen Typen `client::transport::{Input, JsonLinesInput, Output,
  StdoutOutput}`, `InputFactory` und die öffentliche Plugin-Konfiguration `with_io` entfallen. Falls
  später eine echte Einbettung ohne Prozessgrenze hinzukommt, wird deren Interface anhand dieses
  konkreten zweiten Nutzers geplant.
- `session::Plugin::default()` erzeugt die Produktionsintegration mit der internen JSONL-Anbindung.
  Die bisherigen Konstruktoren `rendered_stdio` und `logical_stdio` entfallen zusammen mit dem
  öffentlichen Session-Modus. Der bisherige Top-Level-Re-Export `AutomationControlPlugin` wird
  nicht als Kompatibilitätsalias fortgeführt.
- `session::tracing_error_layer` liefert einen `bevy::log::BoxedLayer` für
  `LogPlugin::custom_layer`. Bevy baut ihn damit vor der einmaligen globalen
  Subscriber-Installation ein, sodass er Error-Events unabhängig vom gewählten Formatter
  beobachtet. Verwendet die Anwendung bereits einen eigenen Custom Layer oder Subscriber, muss sie
  beide Layer vor dieser Installation ausdrücklich zusammensetzen.
- Die interne Verwaltung mehrerer laufender Commands im Bevy-Event-Loop wird während der
  Implementation entworfen. Sie muss neue Requests zwischen begrenzten Arbeitsabschnitten
  verarbeiten und darf den Bevy World nicht von einem unkontrollierten Hintergrundthread aus
  verändern; ihre konkrete Datenstruktur gehört nicht zum Ziel-Interface.

## Server, Client-Zugriff und CLI

HS1 ist für den aktuellen Planungsstand abgeschlossen. Die Verhaltensregeln unten bleiben
maßgeblich; offene Protokoll-, Ausgabe-, Konfigurationsformat- und Plattformdetails werden
in HS2/H6 weitergeführt. Das markiert keine vorweggenommene Entscheidung dieser Details.

Die Transport-Grundentscheidung in HS2 verwendet HTTP für Serververwaltung und WebSocket
für dauerhaft an eine Session gebundene Steuerung und Beobachtung, an derselben Serveradresse.
Der Server ist ausschließlich über Loopback erreichbar. Ein gemeinsamer Zugriffsschlüssel
schützt beide Zugänge; die Session-ID ist kein Berechtigungsnachweis. Sichere Bereitstellung
und Übergabe des Schlüssels, Protokollhüllen und konkrete Fehler bleiben in HS2 offen.
Transporttests müssen beide Zugänge mit gültigem und fehlendem/ungültigem Schlüssel prüfen
sowie sicherstellen, dass der Listener nicht auf einer Netzwerkschnittstelle freigegeben wird.

[`ADR-0022`](../adr/0022-session-host-outlives-clients.md) ersetzt die bisherige unmittelbare
Kopplung eines ausgewählten Controllers an `&mut session::Session`.
[`ADR-0023`](../adr/0023-local-server-manages-multiple-sessions.md) ersetzt dessen
Ein-Session-pro-Server-Regel und nimmt MCP aus dem aktuellen Ziel.
[`ADR-0024`](../adr/0024-separate-server-client-and-cli.md) löst das Sammelmodul `host`
in getrennte Module für Bereitstellung, Zugriff und Bedienung auf:

- `server` ist eine langlebige lokale Entwickleranwendung für mehrere unabhängige
  `session::Session`-Werte. Er besitzt Session-Verzeichnis, Identitätszuordnung und Lebensdauer
  der Session-Handles unabhängig von seinen Clients.
- Jede Session behält ihren privaten Koordinator, ihren Spielprozess, dessen Pipes und ihre
  fachliche Ausführung. Serververwaltung dupliziert keine Command-, Recording-, Replay-,
  History- oder Shutdown-Regel.
- Session-Erzeugen, -Auflisten, -Auswählen und -Zustandsabfrage gehören zum Serververtrag,
  nicht zu `command::Command`. Die CLI muss diese Verwaltung sowie Command-Einreichung und
  Beobachtung vollständig ermöglichen. Weboberflächen und weitere UIs bleiben vorerst draußen.
- `client` kapselt Verbindung, Protokollkodierung und Antwortzuordnung für Verwaltungsaufrufe,
  Commands und Activity, ohne Terminaldarstellung oder Spielregeln. Commands adressieren eine
  bestimmte Session. Activity und Ergebnisse müssen dieser Session eindeutig zugeordnet werden.
- `cli` besitzt Argumentverarbeitung, Terminaldarstellung und die Auswahl des Ablaufs.
  Der Serverstart wird dort angestoßen; der laufende Betrieb und die Sessions gehören `server`.
- Agent-Zugang, Laufzeit, Modellanbindung und Modulplatzierung bleiben bis H4 offen.
  MCP entfällt; es wird auch kein optionaler MCP-Vertrag vorgeplant.
- `cli::repl` ist ein menschlicher Client. `cli::script` ist ein endlicher Client für eine
  versionierte Command-Liste. Beide besitzen die Session nicht.
- Clients dürfen sich unabhängig verbinden, trennen und erneut verbinden. Ein Client-Ende sendet
  weder Stop noch Shutdown und beendet weder laufende Commands noch Recording oder Replay.
- Mehrere Clients dürfen gleichzeitig verbunden sein und Commands senden. Der Server übergibt sie
  je Session in eindeutiger Empfangsreihenfolge an deren privaten dynamischen Session-Adapter.
  Ein Client verbindet sich mit dem Server und meldet sich zur Steuerung an genau einer Session
  gleichzeitig an. Seine Commands verwenden diese Bindung statt einer erneut angegebenen
  Session-ID pro Command. Die Auswahl gilt je Client; es gibt keine serverweit aktuelle Session.
  Mehrere Clients dürfen dieselbe Session steuern, die Bindung ist nicht exklusiv.
  Die Bindung bleibt für die gesamte Verbindung fest; ein Session-Wechsel ist nicht erlaubt.
  Für eine andere Session trennt der Client die Verbindung und meldet sich über eine neue an.
  Dies beendet weder die bisherige Session noch deren laufende Arbeit.
  Anmeldung und die konkreten Zugangsinterfaces bleiben in HS1/HS2 offen.
  `session::RequestId` bleibt sessionlokal. Serverweite Korrelation benötigt zusätzlich die
  Session-Identität; ein neuer serverweiter Command-ID-Owner wird nicht eingeführt.
- Der Client-Zugang trennt Serververwaltung von gebundenen Session-Verbindungen.
  Auflisten und Erstellen benötigen keine Session-Bindung. Steuerung und Beobachtung einer
  Session verwenden eine eigene, fest gebundene Verbindung. Eine Verwaltungsverbindung wird
  nicht nachträglich zur Session-Verbindung; nach Erstellen kann der Client mit der erhaltenen
  ID eine eigene Session-Verbindung öffnen. `cli` wählt den Zugang passend zur Aufgabe,
  `client` kapselt dessen Verbindungszugriff und `server` besitzt Verwaltung und Routing.
  Diese Aufgabentrennung verlangt weder getrennte Server noch unterschiedliche Transporte.
  Konkrete Interfaces und ihre öffentlichen Namen bleiben offen.
- Eine Session-Verbindung darf zu jedem vorhandenen Session-Eintrag aufgebaut werden,
  auch während `Starting` und nach `Ended` oder `Failed`. Sie ermöglicht Startbeobachtung
  beziehungsweise die Abfrage des Endzustands und noch verfügbarer Ergebnisse, garantiert
  aber keine Steuerbarkeit und verlängert keine Aufbewahrung. Commands mit
  Bereitschaftsvoraussetzung werden außerhalb von `Ready` abgelehnt, nicht für später
  vorgemerkt. Ein ausdrücklicher Einzel-Session-Abbruch während `Starting` bleibt offen;
  Serverende beendet auch startende Sessions.
  Die genaue Abbildung dieser Ablehnungen im Client-Vertrag bleibt in HS2/HS4 offen;
  die bestehenden Annahme- und Request-ID-Regeln der direkten Session-API werden dadurch
  nicht stillschweigend geändert.
- Der Server übernimmt Command-Annahmen, terminale Outcomes, Session-Events und später
  Report-Ergebnisse in einen begrenzt aufbewahrten Activity-Stream. Ein Transport-Cursor ermöglicht
  `poll`, `wait` und Wiederaufnahme nach einer Client-Trennung. Ob Cursor serverweit oder je
  Session gelten und wie Activity gefiltert wird, bleibt in HS3 offen.
- `command::Command::Shutdown` ist für REPL, Script und Agent ein ausdrücklicher Client-Command.
  Der Server routet ihn zu `Session::shutdown` der adressierten Session; kein Client dupliziert
  dessen Vorbedingungen oder sendet versteckte Stop-Commands. Andere Sessions und der Server
  bleiben davon unberührt. Das gilt auch bei einem unerwarteten Ende dieses Spielprozesses.
- Serverende ist eine eigene Verwaltungsoperation. Sie leitet das Herunterfahren aller Sessions
  ein und beendet anschließend den Server; aktive Sessions sind kein pauschaler Ablehnungsgrund.
  `server` besitzt diesen Ablauf, die fachlichen Shutdown-Regeln bleiben bei `session`.
  Der Server nimmt dabei keine neuen Sessions oder Spiel-Commands mehr an. Er stoppt Replay
  und andere ausdrücklich stoppbare Arbeit über die bestehenden fachlichen Operationen,
  lässt nicht stoppbare laufende Commands abschließen und beendet Recording sauber.
  Anschließend führt er den Session-Shutdown aus und beendet den Server. Der Verwaltungsaufruf
  erlaubt diese Stop-Schritte ausdrücklich; sie sind keine Änderung des normalen
  Einzel-Session-Shutdowns. Ein unmittelbarer harter Prozessabbruch ist nicht der gewählte Ablauf.
  Für den gesamten Server-Shutdown gilt eine konfigurierbare Frist, nicht eine neue volle
  Frist je Session. Nach Ablauf werden verbliebene Spielprozesse erzwungen beendet und ihre
  Ressourcen aufgeräumt. Der Server meldet diesen Ausgang als Fehler statt als erfolgreichen
  geordneten Abschluss; vollständige Command-Ergebnisse und sauberer Recording-Abschluss
  sind dann nicht garantiert. `server` besitzt diese zeitliche Ablaufregel, `session` weiterhin
  die Prozessbereinigung. Der direkte Session-Shutdown erhält dadurch keinen eigenen Timeout.
  Standard sind 30 Sekunden ab Annahme des Verwaltungsaufrufs, anpassbar über die
  Serverkonfiguration. Das Herunterfahren der Sessions macht unabhängig voneinander Fortschritt;
  eine hängende Session darf nicht die Gelegenheit der anderen zum sauberen Abschluss blockieren.
  Sessions in `Starting` werden beim Serverende ebenfalls beendet und unterliegen derselben
  gemeinsamen Frist. Der Server nimmt sie nicht vom Shutdown aus.
  Darstellung und Validierung der Frist in der Konfiguration sowie
  konkrete Fehlercodes bleiben in HS1/HS4 offen.
  Die Entscheidung hebt die Vorbedingungen von
  `Session::shutdown` nicht auf und setzt kein hartes Beenden per Drop voraus.
  Weitere Ergebnisverfügbarkeit nach Serverende und verwaiste Endpoint-Ressourcen bleiben offen.
- Während des geordneten Herunterfahrens verarbeitet `server` Command-Ergebnisse, Session-Events
  und Report-Ergebnisse weiter. Verbundene Clients können sie bis zum Verbindungsende innerhalb
  der geltenden Activity-Aufbewahrung abrufen. Der Server wartet nicht auf vollständiges Abholen
  durch alle Clients; langsame oder verschwundene Clients blockieren das Serverende nicht.
  Daraus folgt keine dauerhafte Speicherung.
  Laufende Report-Erstellung und Provider-Aufrufe dürfen innerhalb derselben gemeinsamen
  Shutdown-Frist abschließen, ebenso Reports echter Spielfehler, die währenddessen erkannt werden.
  Sind Sessions und Report-Arbeit fertig, kann der Server vor Fristende enden. `report` behält
  seine fachlichen Regeln; `server` orchestriert deren Abschluss.
  Absichtliches Beenden eines Spielstarts oder Spielprozesses erzeugt keinen Spielfehler-Report.
  Bei Fristablauf oder zweitem `Ctrl+C` meldet der Server unvollständigen Abschluss und behauptet
  keine erfolgreiche Report-Erstellung oder Übertragung. Der externe Ausgang eines unterbrochenen
  Provider-Aufrufs kann unbekannt sein; lokaler Abbruch bedeutet keine externe Rücknahme.
- Das Session Protocol v3 bleibt die ausschließlich interne Verbindung zwischen
  `session::Session` und Controlled Application. Das lokale Client-Protokoll besitzt eine getrennte
  Version und legt keine internen Prozess-Pipes offen.
- Serverstart und Discovery, Session-Verwaltung und -Identität, Transport und Zugriffsschutz,
  Activity-Cursor sowie Client- und Shutdown-Abläufe werden in HS1 bis HS4 festgelegt.
  ID-Format, zulässige Eingabelängen und Präfixauflösung sind unten festgelegt.
  Bis dahin bleiben konkrete Server- und Client-Signaturen in `goal.rs` bewusst offen.
- Die bisherige `host::run(config_source)`-Sammelfassade entfällt. H7 legt die nötigen
  Einstiegspunkte nach HS1 bis HS4 fest. Module auf oberster Ebene werden nicht allein durch
  ihre Platzierung zu öffentlichen Exports oder eigenen Crates und Programmen.
- Persistenz-, CLI- und Discovery-Typen bleiben privat, soweit die später festgelegten
  Einstiegspunkte sie nicht nach außen tragen müssen. Die alten öffentlichen
  `host::config`- und `host::command_line`-Module werden nicht bloß umbenannt re-exportiert.
- CLI-Einstiege rufen nicht `std::process::exit` auf. Eine einbettende `main`-Funktion gibt den
  bestimmten `ExitCode` zurück.
- Der bisherige `config_source`-Parameter bezeichnete vollständigen versionierten TOML-Inhalt.
  Die neuen Einstiegssignaturen und der Owner des privaten Parsers werden in HS1/H6/H7 festgelegt.
  Formatversionen bleiben Persistenzdetails außerhalb des Laufzeitmodells; unbekannte Felder und
  nicht unterstützte Versionen werden weiterhin abgelehnt.
- Der private Config-Parser liest keine Datei. Serverstart erhält nur serverweite Konfiguration,
  insbesondere Adresse und Shutdown-Frist, keine impliziten Spiel-Launch-Defaults.
  Session-Erzeugen erhält jeweils eine eigene vollständige Session-Konfiguration ohne Vererbung
  vom Server. Die CLI darf dieselbe Datei mehrfach einlesen; daraus entsteht keine gemeinsame
  veränderliche Laufzeitkonfiguration. Der Server startet ohne Session;
  jede Session wird anschließend ausdrücklich erstellt. Anders als beim bisherigen Start
  einer einzelnen Session sind Serverstart und Session-Erzeugen getrennte Operationen.
  Ein fehlgeschlagener Spielstart ist deshalb kein fehlgeschlagener Serverstart.
  Der bisherige CLI-Override `session.artifact_dir` entfällt im Serverbetrieb zugunsten
  des unten festgelegten ID-Unterverzeichnisses. Server- und Session-Auswahl sind keine
  allgemeinen Overrides der Launch-Konfiguration.
- Die erste Version startet den Server im Vordergrund und bietet keinen eingebauten
  Hintergrundstart. Die CLI zeigt die Server-Logs im startenden Terminal; Clients greifen
  aus anderen Aufrufen darauf zu. Hintergrundbetrieb bleibt externen Werkzeugen wie `tmux`
  oder einem Prozessmanager überlassen. Client-Trennung beendet den Server nicht.
  Erstes `Ctrl+C` oder ein reguläres Beendigungssignal wie `SIGTERM` startet denselben
  geordneten Shutdown wie der Verwaltungsaufruf, einschließlich der gemeinsamen Frist.
  Ein zweites `Ctrl+C` während des Herunterfahrens erzwingt den Abbruch ohne Warten auf das
  Fristende, mit denselben Ergebnis- und Recording-Einschränkungen und Fehlerausgang wie
  bei Fristablauf. Die Signalanbindung löst den gemeinsamen Serverablauf aus und dupliziert
  keine Session-Shutdown-Regeln. `SIGKILL` und Serverabstürze garantieren keinen geordneten
  Abschluss; die plattformspezifische Signalanbindung bleibt offen.
- Die CLI erhält die Serveradresse ausdrücklich. Die erste Version sucht keinen Server
  automatisch und startet bei fehlender Verbindung auch keinen. Nach erfolgreichem Serverstart
  zeigt die CLI dessen Adresse an. Ein nicht erreichbarer Server führt zu einem Verbindungsfehler.
  Transport und Adressformat bleiben in HS2 offen; Discovery-Metadaten werden nicht benötigt.
- `server` bestimmt die Startbereitschaft: Die Initialisierung ist abgeschlossen und
  Verwaltungsaufrufe können angenommen werden. Eine Spielinstanz ist dafür nicht nötig.
  Erst dann zeigt `cli` die tatsächlich verwendete Adresse an. Scheitert der Serverstart,
  meldet die CLI den Fehler und der Prozess endet mit einem Fehlerstatus. Eine belegte Adresse
  führt nicht zum stillschweigenden Ausweichen auf eine andere Adresse.
  Die Darstellung des Bereitschaftssignals für Menschen und Skripte bleibt im CLI-Vertrag offen.
- `server` vergibt die Session-ID und stellt ihre Eindeutigkeit im eigenen Session-Verzeichnis
  sicher, auch bei gleichzeitigen Erstellungsaufrufen. Clients übergeben die Launch-Konfiguration
  und erhalten eine zugewiesene ID; sie können keine eigene ID vorgeben.
  Die ID besteht aus 128 zufälligen Bits und wird als 32 kleingeschriebene Hex-Zeichen ohne
  Bindestriche dargestellt. Bei einer Kollision im Verzeichnis erzeugt der Server eine neue ID.
  Menschenlesbare Anzeigen beginnen mit den ersten 8 Zeichen und verlängern bei Mehrdeutigkeit
  bis zur Eindeutigkeit. Maschinenlesbare Antworten enthalten immer die vollständige ID.
  Eingaben müssen 8 bis 32 Hex-Zeichen enthalten. Kürzere Eingaben werden auch dann abgelehnt,
  wenn sie eindeutig wären. Die zulässige Länge ersetzt nicht die Eindeutigkeitsprüfung.
  Frei wählbare Namen sind damit nicht als Funktion beschlossen.
- Die vollständige Session-ID ist die Identität. Die Kurzform ist ausschließlich ein Präfix,
  keine separat vergebene ID. `server` löst es bei der Anmeldung gegen alle Verzeichniseinträge
  einschließlich `Ended` und `Failed` auf: Genau ein Treffer bindet an die vollständige ID,
  kein Treffer ergibt eine unbekannte Session und mehrere Treffer eine mehrdeutige Auswahl.
  Bei Mehrdeutigkeit muss der Client mehr Zeichen angeben. Die Bindung wird danach nicht erneut
  anhand des Präfixes ausgewählt; später hinzukommende Sessions verändern sie nicht.
- Sobald `server` das Erstellen angenommen und die Session in sein Verzeichnis aufgenommen hat,
  liefert er die ID zurück, ohne auf Spielbereitschaft zu warten. Die Antwort bestätigt
  "Session angelegt", nicht "Spiel bereit". Der Client kann den Start über diese ID verfolgen;
  auch ein späterer Spielstartfehler bleibt der angelegten Session zugeordnet.
  Anders als ein abgeschlossener synchroner Spielstart benötigt der Verwaltungsvertrag damit
  getrennte Beobachtung von Annahme und Startausgang.
- Vor der Annahme prüft der Erstellungsablauf die Verständlichkeit der Anfrage und die fachliche
  Gültigkeit der Konfiguration. Unlesbare Anfragen und ungültige Konfigurationen werden ohne
  Session-Eintrag abgelehnt. Vorbereitung und tatsächlicher Spielstart folgen der Annahme;
  Fehler dabei, etwa beim Prozessstart oder Verbindungsaufbau zum Spiel, gehören zur Session-ID.
  Die bestehenden fachlichen Owner behalten ihre Konfigurationsregeln. `server` nutzt diese
  Regeln, statt eine zweite Validierung zu implementieren. Vor Annahme erfolgen Dekodierung,
  Pflichtangaben-, Wertebereichs- und Kombinationsprüfung sowie ID-Auswahl. Nach Annahme folgen
  Cargo-/Manifest-Auflösung, tatsächliches Öffnen benötigter Dateien und Programme,
  Artefaktverzeichnisanlage, Prozessstart und Verbindungsaufbau. Ein fehlender erforderlicher
  Manifestpfad wird direkt abgelehnt; ein angegebener, aber fehlender oder unlesbarer Manifestpfad
  ergibt `Failed` unter der vergebenen ID. Innere Konfigurationsgültigkeit wird vor Annahme
  geprüft, Ausführbarkeit in der Umgebung danach. Die bestehende Prüfung vorhandener
  ID-Verzeichnisse bleibt Teil der ID-Auswahl vor ihrer Rückgabe.
  Der dafür nötige interne Interface-Zuschnitt folgt diesen fachlichen Ownern;
  der direkte synchrone Rust-Einstieg `Session::start` wird dadurch nicht automatisch ersetzt.
- `server` stellt den Session-Lebenszyklus mit fünf Grundzuständen dar: `Starting` für die
  angelegte Session während Vorbereitung und Spielstart, `Ready` nach abgeschlossenem Spielstart
  und Verbindungsaufbau, `Ended` für reguläres Ende und `Failed` für Fehler bei Vorbereitung,
  Spielstart oder weiterem Session-Betrieb. `Stopping` bezeichnet das laufende Herunterfahren.
  Bei `Failed` kommen ein stabiler maschinenlesbarer Fehlercode und eine verständliche Meldung
  hinzu; die konkreten Codes bleiben offen. Einzelne Command-Fehler machen die Session nicht automatisch
  fehlerhaft. Recording, Replay und laufende Commands bleiben getrennte fachliche Zustände;
  `Ready` verspricht keine Untätigkeit. Der Server bildet den Lebenszyklus aus der Session ab,
  ohne deren Ausführungsregeln zu duplizieren.
- Beim Beginn des Herunterfahrens wechseln `Starting` und `Ready` nach `Stopping`. Der Client
  kann weiter beobachten, aber keine neue Spielarbeit einreichen. Bereits beendete Sessions
  bleiben unverändert. Sauberer Abschluss ergibt `Ended`, auch bei absichtlich beendetem Start,
  sofern die Bereinigung gelingt. Shutdown-Fehler oder erzwungener Abbruch ergeben `Failed`.
  Verbliebene Laufzeitressourcen werden bereinigt; ein Fehler blockiert nicht das Herunterfahren
  der anderen Sessions. Nach Abschluss der Bereinigung endet der Server. Mindestens eine nicht
  sauber heruntergefahrene Session führt insgesamt zum Fehlerausgang des Server-Shutdowns.
  Die gemeinsame Frist bleibt wirksam.
- In der ersten Version bewahrt `server` Einträge mit `Ended` oder `Failed` samt Zustand und
  gegebenenfalls Fehlergrund bis zum Serverende im Session-Verzeichnis auf. Eine automatische
  zeitliche Löschung findet nicht statt. Spielprozesse und andere Laufzeitressourcen werden
  trotzdem freigegeben. Die Regel betrifft nur den Verzeichniseintrag, nicht die Aufbewahrung
  von Activity, ausführlichen Logs oder Artefakten. Wiederherstellung nach Serverneustart ist
  nicht zugesagt. Das Verzeichnis kann während der Serverlaufzeit wachsen; eine ausdrückliche
  Entfernen-Operation oder Begrenzung ist für diese Version nicht beschlossen.
- Die bisherigen Host-Felder `profile_id` und `tool` sowie die getrennte `application`-
  Konfiguration entfallen. Die Launch-Konfiguration besitzt mit `session.launch` genau einen Owner.
- Konfiguration gilt in der ersten Version für die jeweilige Lebensdauer, ohne Live-Neukonfiguration.
  Andere Servereinstellungen benötigen einen Serverneustart, andere Session-Einstellungen eine
  neue Session. Spiel-Commands verändern weiterhin Spielzustand, nicht Launch-Konfiguration.
- Die Server-Shutdown-Frist muss positiv und endlich sein, ohne Angabe gilt der Standard
  von 30 Sekunden. Null, negative Werte und unbegrenztes Warten werden abgelehnt.
  Ungültige Serverkonfiguration führt zu Fehlermeldung und Fehlerstatus, nicht zu
  stillschweigender Korrektur. Die fachliche Prüfung serverweiter Werte gehört `server`;
  Persistenzdarstellung und Feldnamen bleiben in H6, transportabhängige Werte in HS2 offen.
- Erstellen bestätigt die Anlage mit der vollständigen Session-ID, ohne Spielbereitschaft
  zu versprechen. Die Verwaltungsliste enthält alle vorhandenen Einträge einschließlich `Ended`
  und `Failed` mit vollständiger ID, aktuellem Lebenszykluszustand und Erstellungszeitpunkt,
  sortiert nach Erstellungszeitpunkt, älteste zuerst. Die Einzelabfrage ergänzt den tatsächlich
  zugewiesenen Artefaktpfad und bei `Failed` Fehlercode und Meldung. Die Launch-Konfiguration,
  insbesondere Umgebungsvariablen und Zugangsdaten, wird nicht automatisch ausgegeben.
  `server` liefert die Momentaufnahmen, `client` kapselt den Zugriff und `cli` stellt sie dar.
  Fortlaufende Änderungen verwenden den Beobachtungsvertrag. Konkrete Signaturen, Hüllen,
  Zeitformat und Reihenfolge bei gleichen Erstellungszeitpunkten bleiben offen.
- Im Serverbetrieb bildet `server` das Artefaktverzeichnis als
  `<Basisordner>/<vollständige Session-ID>/`, etwa `worktrees/<32-stellige ID>/`.
  Dies ersetzt ausdrücklich die zuvor beschlossene manuelle Wahl getrennter Verzeichnisse
  ohne automatische Unterverzeichnisse. Unterschiedliche Session-IDs ergeben getrennte
  Geschwisterverzeichnisse; Kurzformen werden nicht als Verzeichnisnamen verwendet.
  Der Basisordner kommt aus der Serverkonfiguration und wird beim Serverstart einmal absolut
  aufgelöst; relative Angaben beziehen sich auf das Arbeitsverzeichnis beim Serverstart.
  Er ist eine Pflichtangabe ohne versteckten Standardpfad. Leere Angaben und Fehler beim
  Anlegen oder Auflösen verhindern den Serverstart vor dem Bereitschaftssignal.
  Im Serverbetrieb entfällt der bisherige Session-Artefakt-Override; der Server bestimmt den Pfad.
  Direkte Rust-Nutzung behält ihren eigenen Artefaktpfad. Eine Git-Worktree-Erstellung ist nicht beschlossen.
  Die Reservierung endet erst nach Abschluss aller Schreibarbeit einschließlich Reports,
  nicht allein mit dem Spielprozess. Ihre Freigabe löscht keine vorhandenen Dateien und
  erlaubt nicht automatisch deren Überschreiben. Vorhandene ID-Verzeichnisse werden nicht
  wiederverwendet oder überschrieben; vor Rückgabe der ID wird stattdessen eine neue gewählt.
  Die Anlage erfolgt nach Annahme während `Starting`; Anlagefehler ergeben `Failed` unter der
  vergebenen ID. Eine bereits zurückgegebene ID wird nicht nachträglich geändert.
  Beim Session-Ende bleiben Artefakte erhalten; es gibt keine automatische Verzeichnislöschung.
- Die bisherige Zuordnung des Features `host` zum gleichnamigen Sammelmodul wird wieder geöffnet.
  H7 entscheidet Namen und Zuschnitt der Features, Exports, Crates und ausführbaren Programme.
  `command`, `report`, `session::Plugin`, `session::Session` und die übrigen Session-Typen
  müssen weiterhin ohne Server-/CLI-Abhängigkeiten nutzbar sein. Der Alias `driver` bleibt gestrichen.
- Im Serverbetrieb besitzt `server` den clientunabhängigen Ablauf für Session-Events und
  Report-Auslösung. `report` besitzt weiterhin Erstellung, Signatur und Provider-Ausführung;
  `cli` stellt deren Ergebnisse nur dar.

### Prüfung der Multi-Session-Umstellung

- Ein frisch gestarteter Server besitzt keine Session und startet keinen Spielprozess.
  Erst ausdrückliches Session-Erzeugen startet eine Session. Ein fehlgeschlagener Spielstart
  beendet den bereits laufenden Server nicht.
- Der Serverstart bleibt im Vordergrund, statt nach dem Start eines Hintergrundprozesses
  zurückzukehren. Server-Logs sind im startenden Terminal sichtbar.
- Die CLI zeigt nach erfolgreichem Serverstart die Adresse an. Client-Aufrufe verwenden die
  ausdrücklich angegebene Adresse; eine nicht erreichbare Adresse liefert einen Verbindungsfehler,
  ohne einen Ersatzserver zu suchen oder zu starten.
- Das Bereitschaftssignal erscheint erst nach abgeschlossener Initialisierung und
  Annahmebereitschaft für Verwaltungsaufrufe, auch ohne Spielinstanz. Eine belegte Adresse
  liefert eine Fehlermeldung und einen Fehlerstatus, kein Bereitschaftssignal für eine Ersatzadresse.
- CLI-Fixtures erzeugen zwei Sessions, listen sie auf und adressieren Commands an die gewählte
  Session. Dazu sind weder Weboberfläche noch MCP nötig.
- Gleichzeitige Session-Erstellungsaufrufe erhalten unterschiedliche serverseitig vergebene
  IDs. Der Erstellungsvertrag erlaubt keine vom Client vorgegebene Session-ID.
- Serverstart benötigt keine Spielkonfiguration. Zwei aus derselben Datei erstellte Sessions
  besitzen unabhängige Konfigurationen ohne serverseitige Launch-Vererbung. Änderungen an der
  Datei verändern laufende Sessions nicht; Live-Neukonfiguration ist nicht Teil des Vertrags.
- IDs haben genau 32 kleingeschriebene Hex-Zeichen ohne Bindestriche. Ein gezielt wiederholter
  Zufallswert führt zur erneuten Erzeugung statt zur doppelten Vergabe. Die Anzeige verwendet
  8 Zeichen bei Eindeutigkeit und verlängert kollidierende Präfixe; Maschinenantworten liefern
  unabhängig davon immer die vollständige ID.
- Präfixauflösung prüft eindeutige, unbekannte und mehrdeutige Auswahl unter Einbeziehung von
  `Ended` und `Failed`. Eine über ein zunächst eindeutiges Präfix aufgebaute Verbindung bleibt
  an dieselbe vollständige ID gebunden, wenn später ein weiterer passender Eintrag hinzukommt.
- Eingabelängen von 8 und 32 Hex-Zeichen sind zulässig, 7 und 33 werden abgelehnt.
  Ein eindeutiges Präfix mit weniger als 8 Zeichen wird ebenfalls abgelehnt; ein mehrdeutiges
  Präfix mit 8 Zeichen wird nicht allein wegen seiner zulässigen Länge akzeptiert.
- Zwei Clients können dieselbe Session steuern; ihre Commands werden dort eindeutig geordnet.
  Ein an Session A gebundener Client adressiert seine Commands ohne erneute Session-ID.
  Die Bindung eines anderen Clients an Session B verändert dieses Ziel nicht.
- Eine bestehende Verbindung kann nicht von Session A zu Session B wechseln. Der Client muss
  sich über eine neue Verbindung an B anmelden; ausstehende Arbeit in A läuft unabhängig weiter.
- Auflisten und Erstellen funktionieren über die Serververwaltung ohne Session-Bindung.
  Mit der zurückgegebenen ID lässt sich eine eigene Session-Verbindung öffnen. Der Vertrag
  bietet keine Umwandlung einer Verwaltungsverbindung in eine Session-Verbindung.
- Verbindungsaufbau gelingt zu vorhandenen Einträgen in `Starting`, `Ready`, `Ended` und
  `Failed`. Startfortschritt beziehungsweise Endzustand sind beobachtbar. Ein während
  `Starting` abgelehnter Command mit Bereitschaftsvoraussetzung wird beim späteren Übergang
  zu `Ready` nicht nachträglich ausgeführt; auch `Ended` und `Failed` erlauben ihn nicht.
- Bei einem verzögerten Spielstart liefert das Erstellen bereits nach Annahme und
  Verzeichniseintrag die ID. Startfortschritt und ein anschließender Startfehler lassen sich
  unter derselben ID abfragen; die Erstellungsantwort behauptet keine Spielbereitschaft.
- Unlesbare Erstellungsanfragen und fachlich ungültige Konfigurationen hinterlassen keinen
  Session-Eintrag. Fehlende Pflichtangaben und ungültige Wertebereiche oder Kombinationen
  werden vor Annahme abgelehnt. Fehlende oder unlesbare angegebene Manifestdateien sowie
  Anlage- und Prozessstartfehler führen nach Annahme zu `Failed` unter derselben ID.
- Unlesbare Erstellungsanfragen und fachlich ungültige Konfigurationen hinterlassen keinen
  Session-Eintrag. Ein Fehler bei Vorbereitung oder Spielstart nach Annahme bleibt dagegen
  unter der vergebenen ID sichtbar. Direkte Rust-Nutzung und Serverzugriff verwenden dieselben
  fachlichen Konfigurationsregeln.
- Zustandsabfragen zeigen während eines verzögerten Starts `Starting` und erst nach Spielstart
  und Verbindungsaufbau `Ready`. Reguläres Ende ergibt `Ended`, ein Start- oder Betriebsfehler
  `Failed` mit Fehlergrund. Recording, Replay oder ein einzelner Command-Fehler verändern den
  Lebenszyklus nicht allein durch ihr Auftreten.
- Nach Client-Trennung und erneutem Zugriff bleiben `Ended`- und `Failed`-Einträge mit Zustand
  und gegebenenfalls Fehlergrund bis zum Serverende auffindbar, auch nach längerer Wartezeit.
  Die Aufbewahrung der Einträge verhindert nicht die Freigabe ihrer Laufzeitressourcen und
  verlängert nicht automatisch die getrennte Activity-Aufbewahrung.
- Gleiche Request-ID-Werte in zwei Sessions bleiben über die Session-Zuordnung eindeutig.
  Ein aktives Recording oder Replay in Session A verändert die fachlichen Regeln in Session B nicht.
- Session-Shutdown oder Spielprozessverlust in A beendet weder B noch den Server. Das in HS1
  geplante Serverende erhält eigene Prüfungen und verwendet nicht den Shutdown einer
  beliebig ausgewählten Session als Ersatz.
- Ausdrückliches Serverende leitet bei mehreren aktiven Sessions das Herunterfahren aller ein
  und beendet erst anschließend den Server. Neue Sessions und Spiel-Commands werden nicht mehr
  angenommen. Stoppbare Arbeit erhält ihren fachlichen Stop; nicht stoppbare laufende Commands
  schließen ab. Recording wird vor dem Session-Shutdown sauber beendet.
  Eine hängende Session verhindert nicht die Eskalation nach der gemeinsamen konfigurierten
  Frist: Verbliebene Spielprozesse werden erzwungen beendet und aufgeräumt, der Ausgang als
  Fehler gemeldet. Mehrere Sessions erhalten nicht jeweils eine neue volle Frist.
  Ohne Override beträgt die Frist 30 Sekunden ab Annahme; ein konfigurierter Wert ersetzt diesen
  Standard. Eine hängende Session hindert andere Sessions nicht am geordneten Herunterfahren
  innerhalb derselben Frist.
  Eine während der Vorbereitung oder des Spielstarts befindliche Session wird beim Serverende
  ebenfalls beendet; ihre Laufzeitressourcen werden im gemeinsamen Shutdown-Ablauf bereinigt.
  Zustandsprüfungen decken Starting/Ready -> Stopping -> Ended sowie Stopping -> Failed ab.
  In Stopping bleibt Beobachtung möglich, neue Spielarbeit wird abgelehnt. Ein Shutdown-Fehler
  in A verhindert nicht den Abschluss von B; der Server meldet insgesamt einen Fehler.
  Bereits beendete Einträge bleiben unverändert. Konkrete Fehlercodes und Konfiguration folgen HS1/HS4.
- Erstes `Ctrl+C` und `SIGTERM` verwenden den gemeinsamen geordneten Shutdown mit Frist.
  Währenddessen bleiben Ergebnisse innerhalb der Activity-Aufbewahrung abrufbar, ohne dass
  ein nicht abholender Client das Ende blockiert. Laufende Reports erhalten keine zusätzliche
  Frist; abgeschlossene Sessions und Reports erlauben ein früheres Ende.
  Absichtliches Beenden erzeugt keinen Spielfehler-Report. Ein unterbrochener Provider-Aufruf
  wird nicht als erfolgreich übertragen dargestellt; sein externer Ausgang darf unbekannt sein.
  Ein zweites `Ctrl+C` währenddessen erzwingt die Bereinigung ohne Warten auf das Fristende
  und führt zum Fehlerausgang statt zur Meldung eines erfolgreichen geordneten Abschlusses.
- Client-Trennung lässt ausstehende Arbeit weiterlaufen. Spätere Clients können Ergebnisse nach
  dem in HS3 festgelegten Aufbewahrungs- und Gap-Vertrag zuordnen.
- Verwaltungslisten enthalten aktive und beendete Einträge mit ID, Zustand und Erstellungszeit,
  älteste zuerst. Einzelabfragen ergänzen Artefaktpfad und bei `Failed` Code und Meldung.
  Fixtures mit Launch-Umgebungsvariablen und Zugangsdaten prüfen, dass diese nicht automatisch
  ausgegeben werden. Die Erstellungsbestätigung behauptet keine Spielbereitschaft.
- Zwei Sessions erhalten unter demselben Basisordner unterschiedliche Unterverzeichnisse
  mit ihren vollständigen IDs; parallele Starts benötigen keine manuell getrennten Zielpfade.
  Ein noch schreibender Report hält die Reservierung auch nach Spielprozessende.
  Nach Abschluss aller Schreibarbeit wird sie freigegeben, vorhandene Dateien bleiben unverändert
  und unterliegen weiterhin ihren bisherigen Schreibregeln.
- Relative Basisordner werden gegen das Arbeitsverzeichnis beim Serverstart aufgelöst.
  Fehlende oder leere Basisordner-Angaben und Anlage-/Auflösungsfehler verhindern
  das Bereitschaftssignal. Eine fehlende Shutdown-Frist verwendet 30 Sekunden;
  Null, negative und unbegrenzte Werte werden mit Startfehler abgelehnt.
  Ein vorhandenes ID-Verzeichnis bleibt unverändert und führt vor ID-Rückgabe zur Neuwahl.
  Anlagefehler nach Annahme ergeben `Failed` unter der bereits vergebenen ID.
  Im Serverbetrieb ist kein Session-Artefakt-Override vorgesehen; direkte Rust-Nutzung bleibt unabhängig.
- Es wird weder dauerhafte Speicherung des Session-Verzeichnisses noch Wiederherstellung nach Serverneustart
  stillschweigend zugesagt.
- Die öffentlichen Rust-Session-Tests bleiben ohne Server ausführbar. Das interne Session Protocol
  v3 erhält keine serverseitige Session-ID oder Verwaltungsoperation.

### Prüfung der Modultrennung

- CLI, REPL und Script greifen über `client` auf den Server zu, nicht direkt auf Session-Handles.
- Der Serverbetrieb braucht keine Terminal-Ein-/Ausgabe. Reports werden auch ohne verbundenen
  Client ausgelöst; die CLI besitzt keine zweite Auslöseregel.
- Wire-Typen und Versionierung des lokalen Client-Protokolls erhalten in HS2 genau einen Owner.
  Client und Server erhalten keine getrennten Kopien derselben Kodierungs- und Validierungsregeln.
- Die bestehende Bevy-Integration unter `client` wandert nach `session`; das neue `client`
  wird ausschließlich auf Serverzugriff geprüft.
- H7 ergänzt Export- und Feature-Builds, ohne die Modultrennung vorab in Crates aufzuteilen.

### Agent-Zugang und erneut zu prüfende JSON-Payloads

ADR-0022 verwirft den langlebigen stdin/stdout-Controller, dessen Ende die Session beendet.
ADR-0023 entfernt außerdem die danach geplante MCP-Fassade. Der Agent-Zugang verwendet den
gemeinsamen Client-Vertrag; seine Laufzeit und Modellanbindung bleiben in H4 offen.
Die folgenden Command-, Fehler- und Event-Objekte dokumentieren bisherige Payload-Kandidaten,
keinen vollständigen Multi-Session-Client-Vertrag. Session-Adressierung, äußere Client-Hülle,
Cursor und Fortsetzungsregeln werden nach HS2 und HS3 in H2 bis H4 neu festgelegt.

#### Festgelegte gemeinsame Command-Verarbeitung

- Der Agent-Zugang verwendet denselben Client-Vertrag und dieselbe Command-Darstellung wie REPL
  und Script. Seine noch offene Modulplatzierung erzeugt keine eigene Command-Sprache oder
  Request-ID-Vergabe.
- Jede Eingabezeile enthält genau einen qualifizierten Command-Namen und das immer vorhandene
  `arguments`-Objekt. Eine Agent-Eingabe enthält keine Request-ID und kein zusätzliches
  `type: "command"`-Feld.
- Der gemeinsame Serveradapter validiert den Command und reicht ihn an die Session weiter. Mehrere
  Clients dürfen weitere Commands einreichen, während frühere Commands noch ausstehen.
- Während eines exklusiven Replays ist in der betroffenen Session von außen nur `Replay::Stop`
  fachlich zulässig. Andere normale Requests erhalten nach der Annahme ein `rejected` und werden
  nicht zwischen Replay-Plan und Tick-Grenzen eingefügt. Andere Sessions sind davon nicht betroffen.
- Für jeden angenommenen Command übernimmt der Activity-Stream zuerst eine `pending`-Meldung mit der
  von `Session` vergebenen Request-ID und dem qualifizierten Command-Namen. Später folgt genau eine
  `completed`-, `rejected`- oder `failed`-Meldung mit derselben ID und demselben Command-Namen.
- Pending-Meldungen werden in Eingangsreihenfolge ausgegeben. Terminale Meldungen dürfen in jeder
  Reihenfolge eintreffen. Der Command-Name wird wiederholt, damit ein Agent das Ergebnis nicht nur
  anhand der numerischen ID einordnen muss. Die Argumente werden in Meldungen nicht wiederholt.
- Recording und Replay verwenden dieselbe Eingabeform. `Session` erkennt anhand des fachlichen
  Command-Typs, ob der Command hostseitig ausgeführt oder über das Wire-Protokoll an die Controlled
  Session gesendet wird.

Die bisherige Agent-JSON-Form bleibt als Payload-Kandidat erhalten:

```json
{"command":"tick.warp.start","arguments":{"ticks":600}}
{"command":"inspect.query","arguments":{"source":"entities","entity":null,"with":[],"without":[],"projection":{"kind":"summary"}}}
{"command":"tick.warp.stop","arguments":{}}

{"request_id":17,"command":"tick.warp.start","status":"pending"}
{"request_id":18,"command":"inspect.query","status":"pending"}
{"request_id":19,"command":"tick.warp.stop","status":"pending"}
{"request_id":18,"command":"inspect.query","status":"completed","output":{"items":[]}}
{"request_id":19,"command":"tick.warp.stop","status":"completed","output":{"was_running":true}}
{"request_id":17,"command":"tick.warp.start","status":"completed","output":{"requested_ticks":600,"executed_ticks":42,"outcome":"stopped"}}
```

#### Ungültige Agent-Eingaben

Der Agent liest Eingabezeilen als Bytes. Eine vollständig gelesene Zeile mit ungültigem UTF-8 kann
dadurch als lokaler Inhaltsfehler gemeldet werden. Die Zuständigkeitsgrenze ist der gemeinsame
Command-Decoder: Sobald ein erlaubter `command::Command` entstanden ist, übernimmt die Session; alle
Fehler davor gehören der Agent- beziehungsweise Client-Fassade.

Ungültiges JSON einschließlich einer leeren Zeile erzeugt:

```json
{"input_line":4,"request_id":null,"status":"input_error","error":{"code":"invalid_json","message":"expected value at column 18"}}
```

Ein syntaktisch gültiger JSON-Wert, der nicht als erlaubter Agent-Command dekodiert werden kann,
erzeugt:

```json
{"input_line":5,"request_id":null,"status":"input_error","error":{"code":"invalid_command","message":"field \"arguments\" is required"}}
```

`invalid_command` umfasst Nicht-Objekte, fehlende oder unbekannte Felder, unbekannte Command-Namen
und strukturell falsche Argumente. `shutdown` ist ein erlaubter Client-Command; der Serveradapter
routet ihn zu `Session::shutdown`. Eine genauere Ursache steht ausschließlich in `error.message`;
es gibt keine weiteren stabilen Eingabefehlercodes.

`input_line` beginnt bei eins und zählt jede physische Eingabezeile einschließlich leerer und
ungültiger Zeilen. Der ursprüngliche Zeileninhalt wird nicht ausgegeben. `request_id: null` zeigt,
dass die Session die Eingabe nie angenommen hat. Es entstehen keine `pending`-Meldung, kein History-
oder Recording-Eintrag und kein Verbrauch einer Session-Request-ID.

Nach einem `input_error` liest der Agent die nächste Zeile und bereits angenommene Commands laufen
weiter. Ein vollständig dekodierter Command durchläuft dagegen immer die Session-Annahme.
Fachliche Fehler wie ungültige Command-Werte, `unknown_type_path` oder `entity_not_found` erhalten
deshalb eine Request-ID und enden mit einer korrelierten `rejected`-Meldung.

Ein technischer `std::io::Error` beim Lesen beendet `run` mit `agent::Error::Input`, weil die Grenze
zur nächsten Eingabezeile nicht mehr verlässlich bekannt ist. Ein Schreibfehler beendet `run` mit
`agent::Error::Output`. Reguläres EOF ist kein Lesefehler und wird zusammen mit dem kontrollierten
Agent-Abschluss festgelegt.

Die nach HS2 erneut festzulegenden Fixtures decken mindestens kaputtes JSON, ungültiges UTF-8, einen
JSON-Nicht-Objektwert, fehlende und unbekannte Felder, einen unbekannten Command-Namen sowie
strukturell falsche Argumente ab. Weitere Fixtures belegen, dass kein Eingabefehler eine Request-ID,
einen History- oder einen Recording-Eintrag erzeugt, während ein dekodierbarer fachlicher Fehler
normal abgelehnt und `shutdown` als Client-Command angenommen wird. Die bisherige Entscheidung steht
in
[`ADR-0020`](../adr/0020-invalid-agent-input-stays-local.md).

#### Session-Events im Agent-Modus

Session-Events gehören keinem Command. Ihre bisherige JSONL-Hülle verwendet `request_id: null` und
`status: "session_event"`. Der Server vergibt keine zweite fachliche Event-ID; der Activity-Eintrag
erhält jedoch den in HS3 festzulegenden Transport-Cursor.

```json
{"request_id":null,"status":"session_event","event":{"kind":"protocol_error","code":"unexpected_ready","message":"received a second ready message"}}
{"request_id":null,"status":"session_event","event":{"kind":"recording_failed","path":"recordings/run.jsonl","message":"failed to flush recording"}}
{"request_id":null,"status":"session_event","event":{"kind":"observation_error","code":"invalid_report_marker","message":"report marker is incomplete"}}
```

`Event::Failure` übernimmt `report::Failure` unmittelbar in ein `failure`-Objekt:

```json
{"request_id":null,"status":"session_event","event":{"kind":"failure","failure":{"message":"index out of bounds","origin":{"kind":"panic","location":{"file":"src/game.rs","line":42,"column":9},"backtrace":"..."}}}}
```

Weitere Origin-Formen sind:

```json
{"kind":"panic","location":null,"backtrace":null}
{"kind":"process_exit","status":"exit status: 101"}
{"kind":"tracing_error","target":"game::physics","location":null}
```

Optionale Failure-Meldungen, Locations, Spalten, Backtraces und Targets werden ausdrücklich mit
`null` ausgegeben. Das Event enthält nur den beobachteten Fehler. Der gemeinsame private
Host-Ablauf bleibt für `Report::create` und `report::submit` verantwortlich; die Darstellung des
Reports und Provider-Ergebnisses wird mit der übrigen Host-Ausgabe festgelegt.

Nicht terminale Events beenden den Agent-Ablauf nicht. Sie werden untereinander in der
FIFO-Reihenfolge der Session ausgegeben. Command-Outcomes dürfen nach ihrer Verfügbarkeit zwischen
ihnen erscheinen; der Agent behauptet keine zusätzliche Gesamtordnung zwischen Event- und
Outcome-Strom.

Ein unerwartetes Session-Ende verwendet dieselbe Hülle:

```json
{"request_id":null,"status":"session_event","event":{"kind":"ended","reason":{"kind":"process_exit","status":"exit status: 101"}}}
```

Die Endgründe besitzen diese Formen:

```json
{"kind":"process_exit","status":"exit status: 101"}
{"kind":"transport_closed","channel":"stdout"}
{"kind":"transport_failed","channel":"stdin","message":"broken pipe"}
{"kind":"event_queue_overflow","capacity":256,"dropped_events":3}
```

Transportkanäle heißen `stdin`, `stdout` und `stderr`. Beim Empfang von `Ended` gibt der Agent zuerst
alle bereits vorliegenden Command-Outcomes aus. Jeder danach noch unbeantwortete Command erhält in
Annahmereihenfolge:

```json
{"request_id":17,"command":"inspect.query","status":"failed","error":{"code":"session_ended","message":"session ended before command completed"}}
```

Nach allen normalen Events und Command-Outcomes bleibt `Ended` der terminale Session-Eintrag. Ein
erfolgreicher `Session::shutdown` erzeugt kein `Ended`. H3 prüft nach HS3 erneut die genaue äußere
Activity-Form und Wiederaufnahme.

Agent-Fixtures decken alle Event-, EndReason-, TransportChannel- und Failure-Origin-Varianten sowie
optionale Failure-Felder ab. Reihenfolge-Fixtures prüfen normale Events in FIFO-Reihenfolge, erlaubte
Command-Verzahnung, `Failure` und `RecordingFailed` vor `Ended`, die terminalen Meldungen
unbeantworteter Commands in Annahmereihenfolge und `Ended` als letzte JSONL-Zeile. Die Entscheidung
steht in
[`ADR-0021`](../adr/0021-agent-serializes-session-events-separately.md).

#### Noch zu planen

- Agent-Bedienung und Entscheidung über eine eigene oder eingebundene Laufzeit,
- Modellanbindung und Session-Auswahl, Command-Annahme sowie `poll` und `wait`,
- Fehlerabbildung zwischen CLI, Agent und gemeinsamem Client-Vertrag,
- Client-Trennung ohne Auswirkung auf ausstehende Arbeit oder Session-Lebensdauer.

### REPL

- `cli::repl::run` verwendet eine Verbindung über `client`. Die REPL startet und besitzt die
  Session nicht. Eine getrennte REPL beendet weder Server noch Session.
- Die REPL sendet Session-Commands grundsätzlich nicht blockierend und bleibt bei ausstehenden
  Commands ansprechbar. Insbesondere kann sie die Pace eines laufenden Warp ändern, ihn stoppen
  sowie Recording und Replay innerhalb derselben Session starten und stoppen.
- Während eines exklusiven Replays bleibt die REPL ansprechbar, lässt als Session-Command jedoch nur
  `replay stop` zu. Erst nach `completed`, `stopped` oder `blocked` kann sie wieder andere Commands
  einreichen.
- Die REPL übersetzt jede gültige menschliche Eingabe in denselben qualifizierten Command-Namen und
  dasselbe `arguments`-Objekt, die Client-Protokoll, Agent und Script verwenden. Sie erzeugt keine
  eigene Command-Darstellung und vergibt keine eigene Anzeigenummer.
- Jeder gesendete Command wird mit der von `Session` vergebenen Request-ID und seinem Command-Namen
  als `pending` angezeigt. Activity-Einträge werden nach ihrem Eintreffen über den gemeinsamen Cursor
  gelesen; Ergebnisse dürfen ungeordnet zwischen Prompts erscheinen.
- Die REPL-eigenen Befehle `help`, `pending` und `quit` werden nicht an `Session::send` übergeben.
  `pending` zeigt die im gemeinsamen Serveradapter noch ausstehenden Commands mit ihrer
  Session-Request-ID.
- `quit`, Ctrl-C und eine geschlossene REPL-Eingabe trennen nur diesen Client. Sie warten nicht auf
  ausstehende Commands, blockieren nicht bei aktivem Recording und senden weder Stop noch Shutdown.
- `shutdown` ist davon getrennt ein ausdrücklicher `command::Command::Shutdown`. Der Serveradapter
  routet ihn zu `Session::shutdown` und gibt dessen Erfolg oder Blockierung als gemeinsame Activity
  aus.
- Replay ist kein eigener Host-Ablauf. `replay start PATH` und `replay stop` verwenden die
  hostseitigen Replay-Commands innerhalb der bestehenden Session. Dasselbe gilt für
  `recording start PATH` und `recording stop`.
- Parsefehler, Command-Ablehnungen, einem Command zugeordnete Protokollfehler und sessionweite nicht
  fatale Protokollmeldungen werden angezeigt und beenden die REPL nicht. Ein unerwartetes
  Session-Ende oder ein Fehler der Terminal-Ein-/Ausgabe beendet `run` mit einem REPL-eigenen
  Fehler.
- Der aktuelle `ControllerSession`-Status und seine Felder `paused` und `last_action` werden nicht als
  Session-Zustand übernommen. Die REPL zeigt stattdessen die über den Server beobachtbare Activity.
- Die einfache Befehlssyntax folgt den fachlichen Command-Gruppen, darunter `tick warp`,
  `recording start|stop` und `replay start|stop`.

#### Inspect-Eingaben

Die vollständige REPL-Form für Inspect lautet:

```text
inspect query <arguments-json>
```

`<arguments-json>` umfasst den gesamten Rest der Eingabezeile und ist genau das unter "JSON- und
Wire-Form" festgelegte `arguments`-Objekt von `inspect.query`. Es enthält weder den Command-Namen noch
eine Request-ID. Die REPL verwendet dafür denselben Inspect-Arguments-Codec wie Client-Protokoll,
Agent, Script und Wire-Protokoll. Sie besitzt keinen zweiten Query-Typ und keine eigene Flag-Sprache
für Filter, Selektionen oder Projektionen.

Eine kombinierte Entity-Query kann beispielsweise so eingegeben werden:

```text
inspect query {"source":"entities","entity":null,"with":["game::Player"],"without":["game::Dead"],"projection":{"kind":"components","selection":{"kind":"listed","type_paths":["game::Health"]}}}
```

Für häufige Abfragen gibt es genau vier Kurzformen:

```text
inspect entities
    -> {"source":"entities","entity":null,"with":[],"without":[],"projection":{"kind":"summary"}}

inspect entity 7:1
    -> {"source":"entities","entity":{"index":7,"generation":1},"with":[],"without":[],"projection":{"kind":"summary"}}

inspect resources
    -> {"source":"resources","selector":{"kind":"all"},"projection":{"kind":"metadata"}}

inspect resource game::GameState
    -> {"source":"resources","selector":{"kind":"type","type_path":"game::GameState"},"projection":{"kind":"value"}}
```

`index` und `generation` sind in der Kurzform vorzeichenlose dezimale `u32`, getrennt durch genau
einen Doppelpunkt. Bei `inspect resource` wird der nicht leere Rest der Zeile nach äußerem Trimmen als
vollständiger Type Path übernommen. Jede Kurzform erzeugt direkt denselben
`command::inspect::query::Command` wie die ausgeschriebene JSON-Form. Weitere Kurzformen werden nicht
aus dem Inspect-Schema abgeleitet.

Fehlerhafte JSON-Syntax, ein anderer JSON-Wert als ein Objekt, strukturell ungültige Argumente,
unbekannte Felder und fehlerhafte Kurzform-Operanden werden lokal angezeigt. Die REPL sendet in
diesen Fällen keinen Command; deshalb entstehen weder Request-ID noch `pending`-Anzeige. Ein
erfolgreich dekodierter Command wird dagegen normal angenommen. Fachliche Fehler wie
`unknown_type_path` oder `entity_not_found` erscheinen anschließend als korrelierte
Command-Ablehnung.

`help inspect` zeigt die vier Kurzformen und kopierbare JSON-Beispiele für alle Entity- und
Resource-Projektionen. Parser-Fixtures vergleichen jede Kurzform mit ihrer JSON-Form und decken
außerdem alle Projektionen, lokale Parsefehler und korrelierte fachliche Ablehnungen ab. Die
Entscheidung ist in
[`ADR-0019`](../adr/0019-repl-uses-json-for-complex-inspect-queries.md) festgehalten.

### Session Script

#### Festgelegtes Ziel

- `cli::script` führt eine vorab beschriebene Liste von Commands über eine bestehende
  `client`-Verbindung aus. Das Modul startet und besitzt keine Session. Ohne ausdrücklichen
  Shutdown trennt sich der Script-Client nach seinem Ablauf, während die Session bestehen bleibt.
  `Script::parse` erhält den bereits gelesenen Dateiinhalt; Dateizugriff und Client-Verbindung
  bleiben außerhalb des Script-Moduls.
- Das persistierte Format ist ein versioniertes JSON-Dokument mit einem `commands`-Array. Jeder
  Eintrag besitzt exakt dieselbe Form aus qualifiziertem Command-Namen und `arguments`-Objekt, die
  Agent, Client-Protokoll und REPL verwenden.
- Das Script-Format besitzt keine eigenen Step-Varianten, Namen, Request-IDs, Send-/Receive-Regeln,
  Erwartungen, Assertions, Bedingungen, Schleifen oder zeitbasierten Waits.
- `Script::parse` liest und validiert das vollständige Dokument, bevor ein Command ausgeführt werden
  kann. Die Dokumentversion gehört nur zum gespeicherten Format und wird nicht als Feld in das
  aktuelle `Script`-Laufzeitmodell übernommen. Ein späterer Parser darf ältere Formatversionen in
  dieses Modell übersetzen. `Script::new` validiert programmatisch erzeugte Commands nach denselben
  Regeln.
- Ein Script darf `command::Command::Shutdown` ausdrücklich enthalten. Der Serveradapter routet ihn
  zu `Session::shutdown`; das Script implementiert keine Shutdown-Regel. HS4 legt fest, an welcher
  Position Shutdown zulässig ist und wann vorherige Script-Commands terminal sein müssen.
- `script::run` reicht Commands in Dateireihenfolge an `client` weiter. Die genaue
  Einreichungs- und Wartefolge wird nach HS3 und HS4 erneut geprüft, damit der gemeinsame
  Activity-Stream und ein möglicher Shutdown nicht umgangen werden.
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
      "arguments": {
        "source": "entities",
        "entity": null,
        "with": [],
        "without": [],
        "projection": { "kind": "summary" }
      }
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
  von `Session` vergebenen Request-IDs und denselben Activity-Vertrag wie Agent und REPL.
  Die Session-Auswahl liegt außerhalb der persistierten Command-Liste; ihre genaue Bindung
  an `script::run` bleibt bis HS1 und HS2 offen.
- Das neue persistierte Format erhält eine eigene Formatversion. Seine Command-Einträge verwenden
  jedoch dieselbe qualifizierte Command- und Argumentform wie Client-Protokoll, Agent und Wire-Codec,
  damit keine zweite Command-Sprache entsteht.
