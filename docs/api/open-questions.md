# Reihenfolge der offenen API-Entscheidungen

Diese Liste ordnet die offenen Stellen aus [`README.md`](README.md),
[`migration.md`](migration.md) und [`goal.rs`](goal.rs) nach ihren Abhängigkeiten.
Ein Punkt wird erst bearbeitet, wenn alle unter **Benötigt** genannten Punkte abgeschlossen sind.
So werden Entscheidungen zuerst im zuständigen Modul getroffen und danach in die größeren
Session- und Host-Interfaces übernommen.

Die Reihenfolge unterscheidet drei Arten von Arbeit:

- **Recherche** klärt eine technische Tatsache. Sie trifft noch keine Produktentscheidung.
- **Entscheidung** legt eine von außen sichtbare Regel oder Form fest.
- **Abschluss** prüft ein größeres Interface gegen alle darunter getroffenen Entscheidungen.

## Empfohlene Bearbeitungsreihenfolge

Diese lineare Reihenfolge ist der Arbeitsplan. Jeder Punkt wird abgeschlossen und in `goal.rs` und
`migration.md` festgehalten, bevor der nächste Punkt beginnt.

1. [x] **I1:** Verhalten unbekannter und mehrdeutiger Type Paths festlegen.
2. [x] **I2:** Inspect auf read-only Queries begrenzen und `Set` entfernen.
3. [x] **I3:** Allgemeine Asset-Inspection aus dem Zielinterface entfernen.
4. [x] **I4:** Kanonische JSON-Abbildung reflektierter Werte festlegen.
5. [x] **I5:** `command::inspect` einschließlich seiner JSON-Form und Wire-Fixtures abschließen.
6. [x] **T1:** Den Ersatz von ADR-0003 für Tick-Warp festhalten.
7. [x] **P1:** Die vollständige v3-Command-Abbildung abschließen.
8. [x] **R1:** Beobachtbare Panics in Bevy-Systemen, Tasks und Worker-Threads untersuchen.
9. [x] **R2:** Backtrace-Aktivierung, Erfassung und stabile Teile untersuchen.
10. [x] **R3:** Die zuverlässige Erkennung von `tracing`-Errors untersuchen.
11. [x] **R4:** Berechnung, Normalisierung und Speicherung der Fehlersignatur festlegen.
12. [x] **R5:** Inhalt von `report::Context` und Ermittlung der Anwendungsversion festlegen.
13. [x] **R6:** Konfiguration und Operationen des GitHub-Providers festlegen.
14. [x] **R7:** Die öffentlichen Fehlergruppen von `report::Error` festlegen.
15. [x] **R8:** Das gesamte Reporting-Interface abschließen.
16. [x] **RP1:** Automatischen Vergleich und Veröffentlichung von Replay-Abweichungen festlegen.
17. [x] **RP2:** Das Replay-Interface abschließen.
18. [x] **SR1:** Die Seam zwischen Session-Beobachtung und Reporting festlegen.
19. [x] **S1:** Alle verbleibenden Session-Fragen als konkrete `OPEN`-Punkte erfassen.
20. [ ] **S2:** `session::Session` und seine direkten Module abschließen.
21. [ ] **H1:** Die REPL-Syntax für komplexe Inspect-Queries festlegen.
22. [ ] **H2:** Das Agent-Verhalten bei ungültigen JSONL-Eingaben festlegen.
23. [ ] **H3:** Die Ausgabe von Session-Events und unerwartetem Session-Ende im Agent-Modus
    festlegen.
24. [ ] **H4:** EOF und kontrollierten Abschluss des Agent-Modus festlegen.
25. [ ] **H5:** Die Capability-Anforderungen der einzelnen Host-Abläufe festlegen.
26. [ ] **H6:** Alle verbleibenden Host-Fragen als konkrete `OPEN`-Punkte erfassen.
27. [ ] **H7:** `host::run` und die öffentliche Host-Fassade abschließen.

Die Detailabschnitte unten beschreiben für jeden Punkt Owner, Voraussetzungen und erwartetes
Ergebnis.

## Stufe 1: unabhängige Fragen innerhalb eines Moduls

Diese Punkte können unabhängig voneinander bearbeitet werden. Sie sollen keine Session- oder
Host-Regeln vorwegnehmen.

### I1: Verhalten unbekannter und mehrdeutiger Type Paths

- **Art:** Entscheidung
- **Owner:** `command::inspect::query`
- **Benötigt:** nichts

Für jedes Query-Feld ist festzulegen, ob ein unbekannter oder mehrdeutiger Type Path den Command
ablehnt oder ein leeres beziehungsweise statusbehaftetes Ergebnis erzeugt. Dazu gehören mindestens:

- `entity.with` und `entity.without`,
- `component::Selection::Listed`,
- `resource::Selector::Type`.

Das Ergebnis muss in `goal.rs` und im Abschnitt "Inspect" von `migration.md` stehen.

**Entschieden:** Jeder vom Aufrufer gelieferte Type Path muss sich vor der Ausführung exakt über
Bevys `TypeRegistry` auflösen lassen. Andernfalls wird der gesamte Query-Command ohne Teilergebnis
abgelehnt. Short Paths und Aliase werden nicht eigens aufgelöst; `Unavailable::NotRegistered`
ersetzt diese Ablehnung nicht.

### I2: Read-only-Grenze von Inspect

- **Art:** Entscheidung
- **Owner:** `command::inspect`
- **Benötigt:** nichts

**Entschieden:** Inspect dient ausschließlich der Prüfung des aktuellen Spielzustands. Es besitzt
nur Queries und mutiert den World nicht. Ein `Set`-Command und alle Mutations- oder
Benachrichtigungsgarantien entfallen.

### I3: Allgemeine Asset-Inspection

- **Art:** Entscheidung
- **Owner:** `command::inspect`
- **Benötigt:** nichts

**Entschieden:** Das Zielinterface besitzt keine allgemeine Asset-Query, keinen Asset-Schlüssel und
keine Abbildung von Bevy-Asset-IDs. Sichtbare Assetwirkung wird per Screenshot geprüft; fachlich
relevanter Ladezustand soll über Components oder Resources der Anwendung beobachtbar sein. Eine
enge Asset-Status-Abfrage wird erst bei einem konkreten, dadurch nicht abgedeckten Anwendungsfall
neu geplant.

### I4: kanonische JSON-Abbildung reflektierter Werte

- **Art:** Entscheidung
- **Owner:** `command::inspect::value`
- **Benötigt:** nichts

Alle Value-Queries brauchen dieselbe Abbildung für Structs, Tupel, Enums, Maps, Sets, Handles und
numerische Sonderwerte. Festzulegen sind sowohl die JSON-Form als auch Ablehnungs- und
`Unavailable`-Fälle. Diese Entscheidung betrifft nur reflektierte Inspect-Werte. Sie legt nicht die
gesamte Protokollhülle fest.

**Entschieden:** `Readable` übernimmt für erfolgreich serialisierbare Werte die Ausgabe von
Bevys `TypedReflectSerializer` als `serde_json::Value`; ein eigener typisierter Reflection-Baum
entfällt. Repräsentative Wire-Fixtures schreiben die erwartete Bevy-0.19.1-Ausgabe fest, damit ein
Bevy-Upgrade sie nicht unbemerkt ändert. Ein Serialisierungsfehler wird
`Unavailable::NotSerializable`.

Direkt reflektierte `NaN`- und Unendlichkeitswerte werden vor der normalen Serialisierung mit einem
kleinen `ReflectSerializerProcessor` erkannt. Sie machen den betroffenen Component- oder
Resource-Wert `Unavailable::NotSerializable`, statt als `null` zu erscheinen. Von einem opaken
benutzerdefinierten Serializer erzeugtes `null` bleibt dessen Ausgabe.

Maps folgen Bevys Ausgabe als JSON-Objekt. Von `serde_json` unterstützte skalare Schlüssel werden zu
Strings. Ein nicht als JSON-Objektschlüssel darstellbarer Map-Schlüssel macht den betroffenen Wert
`Unavailable::NotSerializable`; eine eigene Liste aus Schlüssel-Wert-Paaren wird nicht eingeführt.

Sets bleiben JSON-Arrays. Der Reflect-Processor serialisiert ihre Elemente nach denselben Regeln und
sortiert sie lexikografisch nach einer kompakten kanonischen JSON-Darstellung mit rekursiv
alphabetisch geordneten Objektschlüsseln. Damit erzeugt derselbe serialisierte Set-Inhalt dasselbe
JSON, ohne den Set-Elementen ein `Ord` abzuverlangen.

Asset-Handles mit Pfad oder stabiler UUID folgen Bevys `HandleSerializeProcessor`. Flüchtige Handles
bleiben als ausdrücklich sessionlokale `Ephemeral`-Referenz lesbar. Ihr opakes Token dient nur dazu,
gleiche Handles innerhalb derselben Session zu erkennen; es darf sich zwischen Sessions ändern.
Fehlende notwendige Typregistrierung erzeugt `Unavailable::NotSerializable`. Ein flüchtiger Handle
wird weder abgelehnt noch durch Bevys Default-Identität ersetzt.

Die repräsentativen Wert-Fixtures werden zusammen mit der vollständigen Inspect-JSON-Form unter I5
festgehalten. Dafür ist keine weitere Produktentscheidung zu I4 nötig.

### T1: Ersatz von ADR-0003 für Tick-Warp

- **Art:** Entscheidung dokumentieren
- **Owner:** `command::tick`
- **Benötigt:** nichts

Die bereits entworfenen Tick-Warp-Regeln widersprechen ADR-0003. Vor der Implementation muss eine
neue ADR festhalten, welche Regeln ersetzt werden und welche, etwa das Verbot versteckter Ticks,
bestehen bleiben.

**Entschieden:** [ADR-0005](../adr/0005-tick-warp-preserves-explicit-simulation-control.md)
ersetzt Step und die hostdefinierte Tick-Dauer durch einen steuerbaren Tick-Warp. Keine versteckten
Ticks, gesammelter Virtual Input und der Verzicht auf eingebaute Wait-Schleifen bleiben erhalten.

### R1: beobachtbare Panics in den unterstützten Ausführungsorten

- **Art:** Recherche
- **Owner:** `report`
- **Benötigt:** nichts

Zu untersuchen sind Panics in Bevy-Systemen, asynchronen Tasks und Worker-Threads sowie die
unterstützten Panic-Strategien. Das Ergebnis muss benennen, welche Meldung und welcher Prozessstatus
für den Debug Host zuverlässig beobachtbar sind.

**Abgeschlossen:** Rusts prozessweiter Panic-Hook läuft an allen untersuchten Ausführungsorten mit
`panic = "unwind"` und `panic = "abort"`. Der Prozessstatus allein reicht nicht, weil ein detached
Bevy-Task oder ein nicht gejointer Worker nach einem Unwind-Panic den Prozess weiterlaufen lassen
kann. Eine verlässliche Erkennung benötigt deshalb einen von `session::Plugin` verketteten Hook, der
synchron einen maschinenlesbaren Marker auf stderr schreibt. Der vorherige Hook bleibt für die
gewohnte Diagnoseausgabe erhalten. Anwendungscode darf den Hook danach nicht erneut ersetzen.

Quellen, Prozess-Fixtures, Ergebnisse und die verbleibende Grenze stehen in
[`research/bevy-panic-observability.md`](research/bevy-panic-observability.md).

### R2: Backtrace-Aktivierung, Erfassung und Stabilität

- **Art:** Recherche
- **Owner:** `report`
- **Benötigt:** nichts

Zu untersuchen ist, wie der Debug Host beim Session-Start einen Backtrace anfordert, welche Daten er
zuverlässig erhält und welche Teile zwischen Builds stabil normalisiert werden können.

**Abgeschlossen:** Der Debug Host setzt vor jedem Cargo-Start `RUST_BACKTRACE=1`.
`session::Plugin` erfasst im Panic-Hook zusätzlich mit `Backtrace::force_capture` den Stack des
panikenden Threads. So hängt der maschinenlesbare Marker weder von `RUST_LIB_BACKTRACE` noch vom
vorherigen Hook ab. Ein nicht unterstützter Backtrace verhindert die Panic-Erkennung und
Report-Erzeugung nicht.

Stabiles Rust bietet keinen stabilen strukturierten Zugriff auf einzelne Frames und garantiert
keine buildübergreifende Genauigkeit der formatierten Ausgabe. Der Report bewahrt den rohen
Backtrace als Diagnose auf. R4 schließt ihn aus der Signatur aus.

Quellen, Prozess-Fixtures und die geprüfte Alternative stehen in
[`research/rust-backtrace-capture.md`](research/rust-backtrace-capture.md).

### R3: Erkennung von `tracing`-Errors

- **Art:** Recherche
- **Owner:** `report`
- **Benötigt:** nichts

Zu untersuchen sind die von Bevy verwendete `tracing`-Ausgabe, benutzerdefinierte Formatter,
ANSI-Ausgabe und mehrzeilige Events. Das Ergebnis muss eine belastbare Grenze zwischen einem
Error-Level-Event und beliebigem Text auf `stderr` liefern.

**Abgeschlossen:** stderr-Text besitzt keine belastbare Event-Grenze. Eigene Formatter dürfen
Präfixe, ANSI, Felder und Ausgabeziel ändern; mehrzeilige Meldungen lassen sich nicht eindeutig von
benachbarten Ausgaben trennen. Die Erkennung erfolgt deshalb vor dem Formatter in einem
`tracing_subscriber::Layer`. Nur ein dispatchtes Event mit `Level::ERROR` gilt als Tracing-Error.
Direkte stderr-Ausgabe und Error-Spans ohne Event gelten nicht als Tracing-Error.

Die Anwendung registriert `session::tracing_error_layer` über `LogPlugin::custom_layer`. Ein eigener
Formatter bleibt möglich. Bei aktiviertem `report.tracing_errors` muss der Debug Host die
Layer-Registrierung vor Abschluss des Session-Starts bestätigt haben. Gefilterte Events bleiben
absichtlich unbeobachtbar.

Quellen, Fixtures und die Grenzen für strukturierte Felder stehen in
[`research/bevy-tracing-error-observability.md`](research/bevy-tracing-error-observability.md).

## Stufe 2: kleine Interfaces aus den lokalen Antworten abschließen

### I5: `command::inspect` abschließen

- **Art:** Abschluss
- **Owner:** `command::inspect`
- **Benötigt:** I1, I2, I3, I4

Die vier Inspect-Entscheidungen werden gemeinsam gegen Query, Output und Fehlerverhalten geprüft.
Danach werden die genaue JSON-Form beider Query-Kategorien und ihre Wire-Fixtures festgelegt.
Erst dann gilt `command::inspect` für die Protokollplanung als geschlossen.

**Abgeschlossen:** Der Wire-Command heißt `inspect.query`; `source` unterscheidet Entity- und
Resource-Queries, verschachtelte Enums verwenden `kind`. Optionale Felder und leere Listen bleiben
als `null` beziehungsweise `[]` sichtbar. Outputs unterscheiden Entity- und Resource-Items sowie
ihre Projektion ausdrücklich und bilden Wertzugriffe als `readable` oder `unavailable` ab.

Ein unbekannter expliziter Entity-Handle lehnt den Command ab, während eine Suche ohne Treffer eine
leere Item-Liste liefert. Gelistete, aber fehlende Components und ausdrücklich per Type Path
angefragte fehlende Resource-Werte bleiben als `Unavailable::Missing` sichtbar. Die festgelegte
Sortierung stabilisiert Entity-, Component-, Resource- und Set-Ausgaben; die fachliche
Children-Reihenfolge bleibt erhalten.

Die vollständigen JSON-Beispiele und die verbindliche Fixture-Matrix stehen im Abschnitt
"JSON- und Wire-Form" von [`migration.md`](migration.md#json--und-wire-form). P1 legt dafür die
fachlichen Ablehnungscodes `invalid_arguments`, `unknown_type_path` und `entity_not_found` fest.

### R4: Fehlersignatur festlegen

- **Art:** Entscheidung
- **Owner:** `report::Signature`
- **Benötigt:** R1, R2, R3

Festzulegen sind Berechnung, Normalisierung und persistierte Darstellung der Signatur für Panic,
Prozessende und aktivierte `tracing`-Errors. Flüchtige Adressen, absolute Projektpräfixe, Session,
Controller und Command-Verlauf dürfen die Identität nicht verändern.

**Abgeschlossen:** Die fachliche Identität verwendet ausschließlich die Fehlerart und die
bereinigte Fehlermeldung. Code-Stelle, Tracing-Target, Prozessstatus, Backtrace und Report-Context
gehören nicht zur Signatur. Dadurch werden gleichartige Fehler nicht durch zusätzliche
Diagnosedetails getrennt.

Die Meldungsbereinigung vereinheitlicht Zeilenenden, entfernt äußeren
ASCII-Leerraum und ANSI-Steuersequenzen und ersetzt absolute Projektpfade, Speicheradressen sowie
klar erkennbare ISO-Datums- und Uhrzeitangaben. Andere Zahlen, Großschreibung, Unicode und interner
Leerraum bleiben erhalten. Unix-Zeitstempel und beliebige Zahlenfolgen werden nicht anhand ihrer
Größe geraten.

Die persistierte Form lautet
`v1:sha256:<64 kleingeschriebene Hex-Zeichen>`. SHA-256 verarbeitet eine versionierte
längenpräfixierte Binärdarstellung aus Fehlerart und bereinigter Meldung.

Code-Stelle, Tracing-Target, Prozessstatus, Backtrace, Session, Controller, Command-Verlauf und
Report-Kontext gehören nicht zur Signatur. Eine Änderung der Identitäts- oder Bereinigungsregeln
benötigt eine neue Signaturversion.

Feldreihenfolge, Kodierung und Golden Vectors stehen im Abschnitt "Fehlersignatur" von
[`migration.md`](migration.md#fehlersignatur). Die Entscheidung steht in
[`ADR-0006`](../adr/0006-versioned-failure-signatures.md).

### R5: Inhalt von `report::Context` und Anwendungsversion

- **Art:** Entscheidung
- **Owner:** `report::Context`
- **Benötigt:** nichts

Festzulegen sind die Anwendungs- und Session-Angaben für Diagnose und Reproduktion sowie die Quelle
der Anwendungsversion. Dabei ist zu prüfen, welche Daten `session::Session` besitzen muss, damit
`Report::create` den Context ohne ein zweites öffentliches Session-Context-Modell erzeugen kann.
Falls dafür neue Session-Daten nötig sind, werden sie hier benannt, aber erst unter S2 in das
Session-Interface übernommen.

**Abgeschlossen:** `report::Context` enthält eine reportspezifische `Application`, die
`bug_hunter`-Version, Handshake-Version und Capabilities, Tick-Konfiguration, OS, Architektur,
optionale Cargo- und Rustc-Versionen sowie höchstens 50 korrelierte History-Einträge. Die
Anwendungsversion stammt aus dem ausgewählten `package.version` von
`cargo metadata --format-version 1 --no-deps`. Eine optionale Git-Revision samt Dirty-Status ergänzt
den Quellstand.

Anwendungsargumente, Commands und Outcomes werden unverändert übernommen. Reports gelten deshalb als
vertrauliche Artefakte. R6 muss für externe Provider eine ausdrückliche
Veröffentlichungsentscheidung vorsehen. Absolute Pfade, Umgebung, Repository-URL, Branch, Diff und
interne IDs bleiben ausgeschlossen.

`Session` nimmt die benötigten Daten beim Start einmalig in ihren privaten Zustand auf. Es entsteht
kein öffentlicher `session::Context`. Die Datenquellen und ausgeschlossenen Alternativen stehen in
[`research/report-context-sources.md`](research/report-context-sources.md). Die Entscheidung steht in
[`ADR-0007`](../adr/0007-report-context-is-a-session-snapshot.md).

### R6: GitHub-Provider-Interface

- **Art:** Entscheidung
- **Owner:** `report::provider::github`
- **Benötigt:** R4, R5

Festzulegen sind Repository- und Veröffentlichungseinstellungen sowie das Interface für
Duplikatsuche, direkten Remote-Versuch und lokalen Rückfall. Die Duplikatsuche verwendet die unter
R4 festgelegte Signatur.

**Abgeschlossen:** `report::submit` verwendet direkt den konfigurierten Provider. Der lokale
Provider schreibt Markdown. Ein Remote-Provider versucht immer die Veröffentlichung bei genau
diesem Ziel. Schlägt der Versuch wegen Netzwerk, fehlendem Werkzeug, fehlender Anmeldung oder eines
anderen Provider-Fehlers fehl, schreibt `report::submit` stattdessen den lokalen Markdown-Report. Es
wechselt nicht zu einem anderen Remote-Provider.

Der Rückfall ist kein normaler lokaler Erfolg. Das Provider-Outcome enthält den lokalen Pfad und den
Fehler des fehlgeschlagenen Remote-Versuchs. R7 legt die öffentliche Darstellung des enthaltenen
Provider-Fehlers fest.

`github::Config` dupliziert keine Einstellungen von `gh`. Der Provider führt
`gh` im Projektordner der Controlled Session aus und übergibt kein `--repo`. Repository, Host und
Anmeldung werden dadurch nativ über den lokalen Git-Kontext sowie die von `gh` unterstützte
Umgebung aufgelöst. Kann `gh` kein Ziel oder keine Anmeldung bestimmen, greift der beschlossene
lokale Rückfall.

Eigene Repository-, Host-, Authentifizierungs- oder Token-Felder gehören nicht zu diesem Umbau.
Spätere Einschränkungen oder Overrides benötigen eine getrennte Entscheidung und eigene Tests.

Der GitHub-Issue-Body enthält die vollständige Signatur als unsichtbaren
Marker. Die Duplikatsuche vergleicht nur diesen Marker und berücksichtigt offene sowie geschlossene
Issues. Bei einem Treffer gibt der Provider `Existing` mit Issue-Nummer und URL zurück. Er erstellt
kein neues Issue, kommentiert den Treffer nicht und öffnet ein geschlossenes Issue nicht wieder.

Der Provider verwendet `Report::title` ohne Präfix und ergänzt keine Labels, Assignees oder
Milestones. Technische Quellen und Abweichungen vom aktuellen Code stehen in
[`research/native-gh-provider.md`](research/native-gh-provider.md). Die Entscheidung steht in
[`ADR-0008`](../adr/0008-configured-provider-with-local-fallback.md).

### R7: öffentliche Fehler von `report`

- **Art:** Entscheidung
- **Owner:** `report::Error`
- **Benötigt:** R6

Die Provider- und Persistenzfehler werden erst nach den Provider-Operationen festgelegt. Obwohl
`migration.md` diese Arbeit bei der Implementation verortet, ist `report::Error` Teil des
öffentlichen Ziel-Interfaces. Seine Fehlergruppen müssen deshalb vor der Implementation feststehen.
Betriebssystemspezifische Details dürfen intern bleiben.

**Abgeschlossen:** `report::Error` trennt einen Fehler des direkt konfigurierten lokalen
Providers von `FallbackFailed`. `FallbackFailed` enthält sowohl den Remote-Provider-Fehler als auch
den Fehler des lokalen Rückfalls. Ein Remote-Fehler mit erfolgreichem lokalen Rückfall bleibt
`Ok(provider::Outcome::Fallback)` und enthält dort den typisierten Provider-Fehler. Beobachtete
Anwendungsfehler gehören weiterhin zu `report::Failure` und nicht zu `report::Error`.

`provider::Error::Github` enthält `github::Error`. GitHub-Fehler unterscheiden
`Unavailable`, `CommandFailed` und `InvalidResponse` sowie die Operation `Search` oder `Publish`.
Anmeldung, Netzwerk, Repository-Auflösung und Berechtigung werden nicht aus stderr geraten.
`message` bleibt eine Diagnose für Menschen und ist kein stabiler maschinenlesbarer Fehlercode.

`local::Error` unterscheidet `InvalidPath`, `Conflict` und `Filesystem`. Dateisystemfehler besitzen
die Operation `Read` oder `Write`, den betroffenen Pfad und eine menschenlesbare Diagnose. Einzelne
Betriebssystem-Systemaufrufe werden nicht zu öffentlichen Varianten.

### R8: Reporting-Interface abschließen

- **Art:** Abschluss
- **Owner:** `report`
- **Benötigt:** R4, R5, R6, R7

`Report`, `Failure`, `Signature`, `Context`, Provider-Konfiguration, Provider-Outcome und
`report::Error` werden zusammen geprüft. Dabei muss auch feststehen, welche Daten die Session nur
beobachtet und an welcher Seam `report` daraus einen fachlichen Fehler erkennt.

**Teilentscheidung:** Ein unerwartetes Prozessende verwendet die feste Meldung
`process exited unexpectedly`. Der beobachtete Prozessstatus bleibt Diagnose und wird nicht in die
Meldung oder Signatur kopiert. stderr bestimmt diese Meldung ebenfalls nicht.

Die Felder von `Report`, `Failure` und `Signature` sind privat und über lesende Methoden zugänglich.
`Failure` besitzt kontrollierte Konstruktoren für Panic, Tracing-Error und Prozessende. Nur
`Report::create` berechnet Titel, Signatur und Context.

`provider::FileReference` stellt eine lokale Report-Datei dar. `Created` und `Existing` verwenden
weiterhin `provider::Reference`, weil beide Outcomes eine Datei oder ein Issue enthalten können.
`Fallback` nimmt direkt eine `FileReference` und kann daher kein Issue enthalten.

`Report::title` verwendet nach Vereinheitlichung der Zeilenenden und Entfernung von ANSI-Sequenzen
die erste nicht leere, außen von ASCII-Whitespace bereinigte Meldungszeile. Der Titel bleibt auf 120
Unicode-Skalarwerte begrenzt und verwendet bei einer Kürzung `...`. Eine Meldung ohne verwendbare
Zeile erhält je nach Fehlerart `panic`, `tracing error` oder `process exited unexpectedly`.

`Failure::message` ist optional, damit ein unbekannter `panic_any`-Payload ohne erfundene Meldung
sichtbar bleibt. Die Signatur verwendet in diesem Fall einen leeren `message`-Wert. Ein unbekannter
Payload und eine ausdrücklich leere Panic-Meldung besitzen deshalb dieselbe Signatur.

`Report::to_markdown` erzeugt die gemeinsame vollständige Markdown-Darstellung für den lokalen
Provider und den GitHub-Issue-Body. Titel, Signatur-Marker, Failure, Application, Environment und
Commands erscheinen in fester Reihenfolge. Nutzdaten werden nicht gekürzt und können durch passend
lange Code-Fences nicht aus ihren Blöcken ausbrechen. Eine Größenablehnung durch GitHub löst den
lokalen Rückfall mit dem vollständigen Report aus.

`report::submit` nimmt nur `Report` und die laufende `Session` entgegen. Provider und
Ausgabeverzeichnis stammen aus der unveränderlichen, beim Session-Start wirksamen
Report-Konfiguration. Sie können beim Submit-Aufruf nicht ausgetauscht werden.

`report.output` und `provider::FileReference::path` verwenden `PathBuf`. Das Ausgabeverzeichnis ist
ein nicht leerer relativer Pfad aus normalen Bestandteilen. Der lokale Dateiname lautet
`v1-sha256-<digest>.md` und enthält keinen Titel. Vorhandene Symlinks werden abgelehnt; parallele
Aufrufe derselben Signatur erzeugen genau eine Datei und liefern `Created` sowie `Existing`.

Die `path`-Felder aller `local::Error`-Varianten verwenden ebenfalls `PathBuf`. Dadurch bleiben
nicht als UTF-8 darstellbare Pfade in typisierten Fehlern erhalten.

`session` besitzt Prozess, Pipes und Lebenszyklus und liefert geordnete stderr-Bytes sowie den
beobachteten Prozessstatus an eine crate-interne Grenze. `report` besitzt Zeilenpuffer,
Markererkennung und die Abbildung auf `Failure`. Absichtlich durch Shutdown oder Drop ausgelöste
Beendigungen sind keine Report-Kandidaten. History und Metadaten liest `Report::create` weiterhin
direkt aus der Session.

**Abgeschlossen:** `Report`, `Failure`, Titel, Signatur, Context, Markdown-Darstellung,
Provider-Konfiguration, lokale Pfade, Provider-Outcomes und `report::Error` besitzen einen
gemeinsamen Vertrag. Die Ablaufentscheidungen an der internen Session-Grenze wurden unter SR1
getroffen und stehen in
[`ADR-0009`](../adr/0009-session-transports-report-observations.md).

## Stufe 3: Verträge, die mehrere kleine Module verbinden

### P1: vollständige v3-Command-Abbildung abschließen

- **Art:** Abschluss
- **Owner:** `session::protocol`
- **Benötigt:** I5, T1

Für Ready werden Handshake-Form und Fehlercodes festgelegt. Für alle Wire-Commands werden
qualifizierter Name, `arguments`, erfolgreicher Output, Ablehnung und stabile Fehlercodes
festgelegt. Die Commands umfassen Input, Tick, Inspect, Screenshot und Shutdown. Danach können
Agent, Script und REPL dieselbe Command-Form verwenden, ohne eine zweite Command-Sprache zu
erfinden.

Recording und Replay bleiben hostseitige Commands. Ihre Command-Einträge verwenden dieselbe
äußere Form, gehören aber nicht zum Wire-Vertrag der Controlled Session.

**Teilentscheidung:** Stabile Command-Fehlercodes beschreiben die notwendige Reaktion eines
maschinellen Aufrufers. Technisch verschiedene Ursachen teilen einen Code, wenn der Aufrufer sie
gleich behandeln kann; die konkrete Ursache steht in `message`. Interne Bevy-Fehlervarianten werden
nicht einzeln Teil des Wire-Vertrags.

Die 14 Wire-Command-Namen sind festgelegt: `input.keyboard.press`, `input.keyboard.release`,
`input.pointer.press`, `input.pointer.release`, `input.pointer.move_to`, `input.pointer.move_by`,
`input.pointer.scroll`, `input.text.input`, `tick.warp.start`, `tick.warp.set_pace`,
`tick.warp.stop`, `inspect.query`, `screenshot.capture` und `shutdown`. Recording und Replay bleiben
hostseitig.

Die Keyboard-Abbildung ist abgeschlossen. `input.keyboard.press` und `input.keyboard.release`
verwenden jeweils `{"key":"<token>"}` und liefern `output: null`. Die stabilen Ablehnungscodes sind
`invalid_arguments`, `invalid_key`, `keyboard_window_unavailable`, `key_already_pressed` und
`key_not_pressed`.

Die Text-Abbildung ist abgeschlossen. `input.text.input` verwendet `{"text":"<utf-8>"}` und liefert
`output: null`. Ein Text darf höchstens 16.384 UTF-8-Bytes enthalten. Die stabilen Ablehnungscodes
sind `invalid_arguments`, `text_too_large`, `text_window_unavailable` und
`text_focus_unavailable`.

Die Grundform der Pointer-Bewegung ist festgelegt. `input.pointer.move_to` verwendet
`{"position":[x,y]}`, `input.pointer.move_by` verwendet `{"delta":[dx,dy]}`. Beide Angaben stehen in
logischen Pixeln und liefern `output: null`. Pointer-Commands gelten ausschließlich für das intern
aufgelöste Spielfenster und besitzen kein `surface`-Argument. Die festgelegten Ablehnungscodes für
die Fenster- und Positionsauflösung sind `pointer_window_unavailable` und
`pointer_location_unavailable`. Eine Bewegung außerhalb der logischen Fenstergrenzen wird mit
`pointer_position_out_of_bounds` abgelehnt und verändert die bisherige Pointer-Position nicht.

Die Pointer-Button-Abbildung ist abgeschlossen. `input.pointer.press` und
`input.pointer.release` verwenden `{"button":"<token>"}` mit den Tokens `left`, `right` und
`middle`. Beide Commands liefern `output: null`. Die stabilen Ablehnungscodes sind
`invalid_arguments`, `invalid_pointer_button`, `pointer_location_unavailable`,
`pointer_button_already_pressed` und `pointer_button_not_pressed`.

Die Pointer-Scroll-Abbildung ist abgeschlossen. `input.pointer.scroll` verwendet
`{"delta":[dx,dy]}` in Bevy-Zeileneinheiten und liefert `output: null`. Der Command übernimmt die
Vorzeichen unverändert und akzeptiert `[0,0]`. Die stabilen Ablehnungscodes sind
`invalid_arguments`, `pointer_window_unavailable` und `pointer_location_unavailable`.

Die Abbildung von `tick.warp.start` ist abgeschlossen. `ticks` ist eine positive Ganzzahl. Die
optionale Pace ist `{"kind":"as_fast_as_possible"}` oder
`{"kind":"ticks_per_second","target":<positive endliche Zahl>}`. Der Output enthält
`requested_ticks`, `executed_ticks` und `outcome` mit `completed` oder `stopped`. Die stabilen
Ablehnungscodes sind `invalid_arguments`, `invalid_tick_count`, `invalid_pace` und
`warp_already_running`.

Die Abbildung von `tick.warp.set_pace` ist abgeschlossen. Der Command verwendet dieselbe
Pace-Struktur wie `tick.warp.start`, ändert die Standard-Pace und übernimmt sie sofort für einen
laufenden Warp. Der Output gibt die übernommene Pace zurück. Die stabilen Ablehnungscodes sind
`invalid_arguments` und `invalid_pace`; ein laufender Warp ist nicht erforderlich.

Die Abbildung von `tick.warp.stop` ist abgeschlossen. Der Command verwendet `{}` und liefert
`{"was_running":true}` oder `{"was_running":false}`. Ohne laufenden Warp ist er ein erfolgreicher
No-op. Der einzige fachliche Ablehnungscode ist `invalid_arguments`; `warp_not_running` gehört nicht
zum Vertrag.

Die Abbildung von `inspect.query` ist abgeschlossen. I5 legt Argumente und Outputs fest. Die stabilen
fachlichen Ablehnungscodes sind `invalid_arguments`, `unknown_type_path` und `entity_not_found`.
Fehlende oder nicht lesbare Werte bleiben erfolgreiche `Unavailable`-Ergebnisse; eine Query ohne
Treffer liefert `{"items":[]}`.

Die Grundform von `screenshot.capture` ist festgelegt. Der Command verwendet
`{"path":"<relativer png-pfad>"}` und nimmt immer das intern aufgelöste primäre gerenderte
Spielfenster auf. Er besitzt kein `target`-Argument.

Die Screenshot-Abbildung ist abgeschlossen. Der Output enthält `path`, `width`, `height` und
`overwritten` und wird erst nach dem vollständigen Schreiben der PNG-Datei gesendet. Die stabilen
Ablehnungscodes sind `invalid_arguments`, `invalid_screenshot_path`,
`screenshot_window_unavailable`, `screenshot_unavailable` und `screenshot_failed`.

Die Shutdown-Abbildung ist abgeschlossen. `shutdown` verwendet `{}` und liefert `output: null`,
bevor die Controlled Session ihren Prozess beendet. Der einzige fachliche Wire-Ablehnungscode ist
`invalid_arguments`. Ausstehende Requests und ein aktives Recording lehnt bereits
`Session::shutdown` mit `session::Error::ShutdownBlocked` ab, ohne einen Wire-Command zu senden.

Die Ready-Abbildung ist abgeschlossen. Die erste Nachricht verwendet
`{"status":"ready","version":3,"capabilities":{"screenshot":<bool>}}`. Die stabilen
Handshake-Fehlercodes sind `invalid_ready` und `unsupported_protocol_version`. Eine weitere
Ready-Nachricht während der Session erzeugt den nicht fatalen Protokollfehler `unexpected_ready`.

**Abgeschlossen:** Ready und alle 14 Wire-Commands besitzen festgelegte Namen, Argumente, Outputs,
Ablehnungen und stabile Fehlercodes. Recording und Replay bleiben hostseitige Commands mit derselben
äußeren Command-Form.

**Nachprüfung nach R8:** Die Reporting-Entscheidungen ändern keinen Command, keine Response und
keinen Ready-Wert des v3-Protokolls. Panic-, Tracing- und Layer-Registrierungsmarker bleiben interne
stderr-Beobachtungen. Sie sind keine Wire-Nachrichten und erzeugen keine zusätzliche Capability.

### RP1: Darstellung und Veröffentlichung von Replay-Abweichungen

- **Art:** Entscheidung
- **Owner:** `command::replay`
- **Benötigt:** R8

Zu prüfen war zunächst, wie automatisch erkannte Abweichungen nach einem Replay zugänglich werden
sollten:

- ob sie Teil von `replay::Completion`, ein getrenntes Ergebnis oder ein Report sind,
- welche Daten pro Abweichung sichtbar werden,
- ob und wann eine Abweichung veröffentlicht wird,
- wie mehrere Abweichungen eines weiterlaufenden Replays dargestellt werden.

Bei der Prüfung wurde die vorher skizzierte Annahme eines automatischen Outcome-Vergleichs
ausdrücklich wieder geöffnet. Ohne konkreten Anwendungsfall würden exakte Vergleiche unter anderem
bei Physikwerten, flüchtigen Handles und nicht deterministischen Reihenfolgen leicht falsche
Abweichungen erzeugen.

**Entschieden:** Das Ziel besitzt zunächst keinen automatischen Vergleich zwischen aufgezeichneten
und beim Replay neu entstandenen Outcomes. Replay übersetzt die Recording vor der Ausführung in
einen internen Replay-Plan. Die aufgezeichneten Outcomes bleiben Teil der unveränderten Recording,
bestimmen aber weder Replay-Abschluss noch Reporting.

Die einzige Verwendung eines aufgezeichneten Outcomes durch diese Übersetzung betrifft gestoppte
Warps: Ihr `executed_ticks` bestimmt die effektive Tickzahl des Replay-Warps. Der Plan lässt den
zugehörigen Stop weg und darf unmittelbar benachbarte Warps mit gleicher effektiver Pace
zusammenfassen. Das ist keine Bewertung des neu entstandenen Outcomes.

Es gibt keinen `Deviation`-Typ, keine Abweichungsliste, keine Vergleichsstatistik und keine
Veröffentlichung von Abweichungen. `Completion` enthält nur den Ausführungsabschluss. Panics,
aktivierte Tracing-Errors und unerwartete Prozessenden laufen unabhängig davon durch das
Reporting-Interface.

Ein vollständig ausgeführtes Replay belegt ohne anwendungsseitig sichtbare Invarianten nur, dass die
Command-Folge verarbeitet werden konnte. Spätere Zustandsvergleiche benötigen einen konkreten
Anwendungsfall und eigene fachliche Vergleichsregeln.

### RP2: Replay-Interface abschließen

- **Art:** Abschluss
- **Owner:** `command::replay`
- **Benötigt:** RP1

`Start`, `Stop`, `Completion` und `Outcome` werden gemeinsam geprüft. Dabei ist insbesondere
festzulegen, wie ein beim Replay abgelehnter Command behandelt wird. Danach darf die
Session-Ausführung die endgültigen Replay-Outputs und Fehler übernehmen.

**Entschieden:** Replay verwendet intern `Idle`, `Preparing`, `Running` und `Stopping`.
`Replay::Start` reserviert die Ausführung bereits vor dem vollständigen Laden; ein weiterer Start
wird in allen aktiven Zuständen mit `replay_already_running` abgelehnt. Eine nicht lesbare Recording
ergibt `session::Error::Io`, ein ungültiges Format `invalid_recording` und eine nicht unterstützte
Version `unsupported_recording_version`.

Der interne Replay-Plan sendet Nicht-Tick-Commands bis zur nächsten Tick-Grenze und wartet dort auf
alle terminalen Outcomes. `Completed` und `Rejected` erfüllen die Barriere; Replay bewertet
fachliche Ablehnungscodes nicht. `ProtocolFailed`, `Unanswered`, ein Session- oder Transportende und
ein fehlgeschlagener Notaus führen zu `Blocked` mit stabilem technischem Code und lesbarer Meldung.
Eine konfigurierbare fachliche Zuordnung von Ablehnungscodes zu `continue` oder `block` bleibt einer
späteren Funktion mit konkretem Anwendungsfall vorbehalten.

`Replay::Stop` ist ein kontrollierter Notaus. Er gibt keine weiteren Plan-Commands frei, beendet
einen aktiven Warp intern mit `Warp::Stop` und wartet auf alle bereits gesendeten Commands. Erst
danach erhalten der ursprüngliche Start `Stopped` und alle wartenden Stop-Commands
`was_running: true`. Wiederholte Stops sind idempotent; ohne aktives Replay ist Stop ein
erfolgreicher No-op. Ein technischer Fehler beim Anhalten hat mit `Blocked` Vorrang vor `Stopped`.
Bereits ausgeführte Commands und Ticks werden nicht zurückgenommen.

Während eines aktiven Replays ist von außen nur `Replay::Stop` zulässig. Andere Controller-Commands
werden mit `replay_in_progress` abgelehnt und nicht zwischen Replay-Plan und Tick-Grenzen
eingemischt.

Die stabilen Blockierungscodes sind `command_protocol_failed`, `session_io_failed`,
`session_ended`, `request_id_exhausted` und `stop_failed`; die lesbare Meldung enthält die konkrete
Ursache. Falls der Notaus technisch blockiert, erhält der ursprüngliche Start dieses
`Blocked`-Outcome. Wartende Stop-Commands liefern den auslösenden `session::Error` und keinen
erfolgreichen Stop-Output. Damit sind Replay-Vorbereitung, Ausführung, Abschluss, Fehler und Notaus
für den aktuellen Planungsstand abgeschlossen.

### SR1: Integration von Session-Beobachtung und Reporting festlegen

- **Art:** Entscheidung
- **Owner:** Seam zwischen `session` und `report`
- **Benötigt:** R8

Die Dokumente legen fest, dass `Session` stderr und Prozessstatus beobachtet, diese Daten aber nicht
fachlich als Panic, `tracing`-Error oder Prozessfehler interpretiert. Gleichzeitig zeigt `goal.rs`
noch keinen Weg, über den `report` diese Beobachtungen erhält. Vor Abschluss des Session-Interfaces
ist daher festzulegen:

- welche Beobachtungsdaten `Session` bereitstellt,
- wer `Report::create` und `report::submit` auslöst,
- wie die Vorrangregel Panic vor Prozessende angewendet wird,
- wie ein erzeugter oder fehlgeschlagener Report für den Host sichtbar wird.

Diese Entscheidung darf kein zweites Modul einführen, das dieselben Prozess- oder History-Daten wie
`Session` besitzt.

**Abgeschlossen:** `session` besitzt Prozess, Pipes, Status und History. Ein crate-interner
`report::Observer` besitzt den stderr-Puffer und deutet die versionierten Panic-, Tracing- und
Layer-Statusmarker. Gültige Marker werden aus der menschlichen stderr-Ausgabe entfernt; andere Bytes
bleiben unverändert. Ein eigener Leser leert stderr bis EOF, auch während der Host einen Provider
aufruft.

`Session::start` verarbeitet stdout-`Ready` und den Status des Tracing-Error-Layers unabhängig von
ihrer beobachteten Reihenfolge. Bei aktivierter Tracing-Beobachtung verhindert eine negative
Layer-Bestätigung den Start. Der Layer-Status bleibt außerhalb von Protokoll und Capabilities.

`session::Event::Failure` transportiert erkannte `report::Failure`-Werte.
`session::Event::ObservationError` macht ungültige oder unvollständige Marker sichtbar. Nach einem
ungültigen oder bei EOF unvollständigen Marker verwendet es den stabilen Code
`invalid_report_marker`. Nach einem Prozessende werden zuerst beide Prozessausgaben vollständig
geleert. Ein erkannter Panic hat dann Vorrang vor `ProcessExit`; absichtlicher Shutdown und Drop
lösen keinen Report aus.

Der gemeinsame private Ablauf unter `host::run` ruft für jedes Failure-Event `Report::create` und
`report::submit` auf. Er behält den Report zusammen mit dem Provider-Outcome oder dem typisierten
Submit-Fehler. REPL, Agent und Script erhalten keine eigene Report-Auslösung. Die konkrete
Host-Darstellung bleibt H3 und H6 zugeordnet.

Marker-Framing, Event-Reihenfolge, Startprüfung und die verworfenen Alternativen stehen in
[`ADR-0009`](../adr/0009-session-transports-report-observations.md).

## Stufe 4: das Session-Interface schließen

### S1: verbleibende Session-Fragen ausdrücklich erfassen

- **Art:** Bestandsaufnahme
- **Owner:** `session`
- **Benötigt:** P1, RP2, SR1, R5

`README.md` nennt die übrige Session-Struktur als offen und `migration.md` bezeichnet das
öffentliche Session-Interface nur als grob festgelegt. Bevor weitere Typen angenommen werden, werden
alle noch fehlenden Entscheidungen als konkrete `OPEN`-Punkte in `goal.rs` eingetragen. Die
Bestandsaufnahme prüft insbesondere:

- Start und Ready-Handshake,
- `Pending`, `send`, `try_receive` und `receive`,
- hostseitige Recording- und Replay-Commands,
- History und sessionweite Events,
- Report-Auslösung aus SR1,
- Shutdown, Drop und unerwartetes Ende,
- die aus R5 benötigten Session-Daten.

Dieser Schritt trifft noch keine neuen Entscheidungen. Er verhindert, dass die pauschale offene
"Session-Struktur" stillschweigend als abgeschlossen behandelt wird.

Die Bestandsaufnahme ergibt acht konkrete Entscheidungspunkte:

1. [x] **S1-A:** Projektordner und gemeinsames Artifact-Verzeichnis festlegen.
2. [x] **S1-B:** Fortschritts- und Nebenläufigkeitsmodell der Session festlegen.
3. [x] **S1-C:** Command-Annahme, `RequestId` und `Pending` abschließen.
4. [x] **S1-D:** Aufbewahrung und Sichtbarkeit der History festlegen.
5. [x] **S1-E:** Event-Queue und terminales Session-Ende festlegen.
6. [x] **S1-F:** Recording-Dateivertrag und Recording-Zustände abschließen.
7. [x] **S1-G:** Replay-Dateivertrag und Dateifehler abschließen.
8. [x] **S1-H:** Wirkung von Session-Fehlern und die Shutdown-Grenze festlegen.

**S1-A abgeschlossen:** `launch::Config::manifest_path` bezeichnet ausdrücklich das Cargo-Manifest.
`Session::start` kanonisiert es und verwendet seinen Elternordner als unveränderlichen
`project_dir`. Relative Artifact-Roots beziehen sich auf diesen Ordner. Die Session erzeugt und
kanonisiert genau ein Artifact-Root für Screenshots, Recordings und Reports und übergibt es dem
Kindprozess über die reservierte interne Umgebungsvariable `BUG_HUNTER_ARTIFACT_DIR`.

Absolute Artifact-Roots und Roots außerhalb des Projekts bleiben erlaubt. Ein leerer Pfad, ein nicht
reguläres Manifest oder ein Artifact-Root, der selbst ein Symlink ist, ergibt `InvalidConfig`.
Fehler beim Erzeugen oder Kanonisieren des Roots ergeben `Io`; eine fehlgeschlagene Cargo-Auflösung
ergibt `Launch`. Die vollständige Entscheidung steht in
[`ADR-0010`](../adr/0010-manifest-rooted-session-paths.md).

**S1-B abgeschlossen:** Eine gestartete Session macht unabhängig von Empfangsaufrufen Fortschritt.
Ein privater Koordinator besitzt Kindprozess, Pipes und veränderlichen Session-Zustand. `Session`
bleibt ein synchroner, nicht klonbarer Handle. Eigene Leser leeren stdout und stderr fortlaufend;
hostseitige Commands, Recording, Replay, History und Events laufen auf dem Koordinator weiter.

`try_receive` und `try_receive_event` prüfen nur bereits verfügbare interne Nachrichten und
blockieren nicht. `receive` und `receive_event` warten auf dem internen Kanal, ohne den Koordinator
anzuhalten. Auf der Controlled-Session-Seite liest eine private Komponente stdin; ausschließlich der
Bevy-Event-Loop greift auf den `World` zu. Ein einzelner Writer serialisiert vollständige
stdout-JSONL-Nachrichten. Die Entscheidung steht in
[`ADR-0011`](../adr/0011-session-progresses-on-a-private-coordinator.md).

**S1-C abgeschlossen:** `Session::send` wartet nur auf die Annahme durch den Koordinator.
Commandspezifische Ablehnungen und Fehler erhalten zuerst eine Request-ID und ein `Pending` und
werden später terminal. Nur eine beendete oder technisch nicht erreichbare Session sowie erschöpfte
Request-IDs verhindern die Annahme.

`RequestId` ist ein opaker, kopierbarer `u64` mit `as_u64` und `Display`. IDs beginnen bei 1, steigen
innerhalb einer Session monoton und werden nicht wiederverwendet. Interne Replay-Commands verwenden
denselben Nummernraum. `u64::MAX` bleibt für den Wire-Shutdown reserviert; danach gibt `send`
`RequestIdExhausted` zurück.

`Pending` ist nicht klonbar und durch eine private Session-Identität gebunden. Jedes terminale
Ergebnis kann genau einmal entnommen werden. Sein Drop bricht den Command nicht ab; der Command
bleibt bis zum terminalen Ergebnis ausstehend und schließt History und Recording ab. Die
Entscheidung steht in
[`ADR-0012`](../adr/0012-command-acceptance-precedes-pending-results.md).

**S1-D abgeschlossen:** Die Session hält einen Ring der letzten 50 angenommenen Commands in
Annahmereihenfolge. Jeder Eintrag beginnt als `Unanswered` und wird an seiner vorhandenen Position
terminal ergänzt. Die getrennte aktive Command-Tabelle behält auch ältere laufende Commands bis zur
Korrelation.

Wire-Commands, Recording- und Replay-Steuerung, interne Replay-Plan-Commands und der angenommene
Shutdown gehören in die History. Vor der Annahme fehlgeschlagene Aufrufe, Events und Reports gehören
nicht hinein. `IoFailed` ergänzt bekannte commandbezogene I/O-Fehler. `Report::create` kopiert den
Ring beim Aufruf. `Session` erhält keinen öffentlichen History-Accessor; vollständige dauerhafte
Abläufe bleiben Aufgabe des Recordings. Die Entscheidung steht in
[`ADR-0013`](../adr/0013-session-history-is-a-bounded-report-window.md).

**S1-E abgeschlossen:** `Event::Ended` enthält einen typisierten `EndReason` für unerwartetes
Prozessende, geschlossenes oder fehlerhaftes stdin, stdout und stderr sowie einen Überlauf der
Event-Queue. Nach einem Prozessende leert der Koordinator beide Ausgaben und reiht alle daraus
entstehenden Events vor dem genau einmaligen terminalen `Ended` ein. Bereits vorliegende
Command-Ergebnisse bleiben abrufbar; offene Commands liefern `Ended`.

Erfolgreicher Shutdown und Drop erzeugen kein Endevent. Die Queue fasst 256 normale Events und hält
einen zusätzlichen Platz für `Ended` frei. Bei Überlauf beendet der Koordinator die Session, zählt
verlorene Events und meldet `EventQueueOverflow`; die dadurch verursachte Prozessbeendigung erzeugt
keinen Report. Die Entscheidung steht in
[`ADR-0014`](../adr/0014-session-end-is-a-terminal-typed-event.md).

**S1-F abgeschlossen:** Jedes Recording ist ein versionierter JSONL-Abschnitt mit Header, einem
Eintrag aus qualifiziertem Command und terminalem Outcome pro aufgenommenem Command sowie einem
Footer. Es speichert keine Wire-Request-IDs und bleibt auch bei ungeordnet eintreffenden Responses in
Annahmereihenfolge. Recording- und Replay-Steuercommands werden nicht aufgenommen; intern
ausgeführte Replay-Plan-Commands dagegen schon.

Start und Stop verwenden die Zustände `Idle`, `Starting`, `Active` und `Stopping` als Barrieren an
Grenzen ohne ältere offene Commands. Pfade bleiben relativ zum gemeinsamen Artifact-Root, dürfen
keine Symlinks durchlaufen und werden mit `create_new` nie überschrieben. Ein Schreibfehler während
der Aufnahme verändert das Command-Outcome nicht, sondern beendet das Recording und erzeugt
`Event::RecordingFailed`. Bei einem unerwarteten Session-Ende versucht der Koordinator, offene
Einträge als `unanswered` und die Datei mit `session_ended` abzuschließen. Die vollständige
Entscheidung steht in
[`ADR-0015`](../adr/0015-recordings-are-ordered-versioned-jsonl-sections.md).

**S1-G abgeschlossen:** Ein privater versionsabhängiger Adapter validiert Pfad, Header, sämtliche
Command-Outcomes und Footer, bevor Replay den ersten Plan-Command ausführt. Er übersetzt eine
unterstützte Formatversion in einen typisierten Plan und hält persistierte Typen aus dem
Replay-Koordinator heraus. Unbekannte und doppelte Felder, unzulässige Commands, falsche
Command-Outputs und inkonsistente Zähler machen die Datei ungültig.

`invalid_recording_path`, `invalid_recording` und `unsupported_recording_version` bleiben stabile
commandspezifische Ablehnungen; Öffnungs- und Lesefehler ergeben `session::Error::Io`. Aufgezeichnete
Outcomes werden nicht mit dem neuen Lauf verglichen. Nur ein erfolgreich beantworteter Warp liefert
seine effektive Tickzahl für den Plan. Das Laden blockiert den Session-Koordinator nicht und kann
durch Stop in `Preparing` kontrolliert abgebrochen werden. Die vollständige Entscheidung steht in
[`ADR-0016`](../adr/0016-replay-validates-recordings-before-execution.md).

**S1-H abgeschlossen:** Jeder Session-Fehler hat lokalen oder terminalen Geltungsbereich.
Command-Ablehnungen, zugeordnete Protokollfehler, hostseitige Dateifehler, ungültige `Pending`-Werte
und eine erschöpfte normale Request-ID beenden die Session nicht. Prozess- und Transportverlust sowie
Event-Überlauf beenden sie dagegen endgültig und lassen noch offene Commands `Unanswered`.

`Session::shutdown` beginnt nur ohne offene Commands und bei `Idle` stehendem Recorder. Andernfalls
liefert es `ShutdownBlocked` mit `shutdown_commands_pending` oder `shutdown_recording_active`, ohne
einen Wire-Command oder History-Eintrag zu erzeugen. Nach dem Senden der einmaligen reservierten
Shutdown-ID ist jeder Ausgang terminal. Nur eine gültige Response mit anschließend erfolgreichem
Prozessende ist ein sauberer Shutdown ohne `Ended`-Event. Drop bleibt ein harter Ressourcenabbruch
ohne fachliche Abschlussgarantien. Die vollständige Entscheidung steht in
[`ADR-0017`](../adr/0017-session-errors-have-local-or-terminal-scope.md).

**S1 abgeschlossen:** Alle acht Session-Fragen S1-A bis S1-H besitzen angenommene Entscheidungen.
S2 kann das öffentliche Session-Interface nun gegen diese Verträge abschließend prüfen.

### S2: `session::Session` und seine direkten Module abschließen

- **Art:** Abschluss
- **Owner:** `session`
- **Benötigt:** S1 und alle dort erfassten Entscheidungen

Abzuschließen sind `session::Config`, `session::launch`, `Capabilities`, `RequestId`, `Pending`,
`history`, `Event`, `Session` und `session::Error`. Jede Signatur in `goal.rs` wird gegen die Regeln
in `migration.md` geprüft. Danach dürfen Host-Module nur noch dieses Interface verwenden und keine
eigenen Session-Regeln ergänzen.

Die interne Datenstruktur für laufende Command-Arbeit im Bevy-Event-Loop wird erst bei der
Implementation entworfen. Sie ist kein offener Teil des öffentlichen Ziel-Interfaces, solange sie
die beschlossenen Fairness-, Reihenfolge- und World-Zugriffsregeln erfüllt.

## Stufe 5: Controller auf dem fertigen Session-Interface abschließen

### H1: komplexe Inspect-Eingaben der REPL

- **Art:** Entscheidung
- **Owner:** `host::repl`
- **Benötigt:** I5, P1, S2

Die Textsyntax für komplexe Inspect-Queries wird auf die fertige gemeinsame Command-Form
abgebildet. Sie darf keine zweite fachliche Query-Darstellung erzeugen.

### H2: ungültige Agent-Eingaben

- **Art:** Entscheidung
- **Owner:** `host::agent`
- **Benötigt:** P1, S2

Festzulegen ist das Verhalten für ungültige JSONL-Zeilen und syntaktisch gültige Einträge, die nicht
als Command dekodiert werden können. Zu bestimmen sind Ausgabeform, Fortsetzung nach dem Fehler und
die Frage, ob für nie angenommene Eingaben eine Request-ID existiert.

### H3: Session-Events im Agent-Modus

- **Art:** Entscheidung
- **Owner:** `host::agent`
- **Benötigt:** S2

Festzulegen ist, wie nicht fatale Session-Events und ein unerwartetes Session-Ende in der
maschinenlesbaren Ausgabe erscheinen. Die Darstellung muss von terminalen Command-Outcomes
unterscheidbar sein.

### H4: EOF und kontrollierter Agent-Abschluss

- **Art:** Entscheidung
- **Owner:** `host::agent`
- **Benötigt:** H2, H3

Festzulegen ist das Verhalten bei EOF mit ausstehenden Commands, laufendem Replay oder aktivem
Recording. Der Agent darf keine versteckten Stop-Commands senden und kein Recording stillschweigend
beenden. Danach kann das `host::agent::Exit`- und Fehler-Interface abgeschlossen werden.

### H5: konkrete Capability-Anforderungen der Host-Abläufe

- **Art:** Entscheidung
- **Owner:** privates `host::run`-Modul
- **Benötigt:** H1, H4 und das bereits skizzierte `host::script`

Für REPL, Agent und Script ist festzulegen, wie der Host ihre benötigten Capabilities vor dem Start
des Ablaufs bestimmt. Die Darstellung bleibt privat, muss aber aus den tatsächlich verwendeten
Commands ableitbar sein und darf keine zweite Capability-Definition neben
`session::Capabilities` schaffen.

## Stufe 6: Host als oberste Schicht abschließen

### H6: verbleibende Host-Fragen ausdrücklich erfassen

- **Art:** Bestandsaufnahme
- **Owner:** `host`
- **Benötigt:** H5

`README.md` nennt die übrige Host-Struktur pauschal als offen. Wie bei S1 werden zuerst konkrete
`OPEN`-Punkte erfasst, statt fehlende Details anzunehmen. Geprüft werden:

- das private versionierte TOML-Format,
- die Auswahl von REPL, Agent, Script und reinem Report-Aufruf,
- CLI-Syntax und erlaubter Artifact-Override,
- Dateizugriffe für Config, Script, Recording, Replay und Reports,
- Fehlerdarstellung und Exit-Code-Zuordnung,
- Capability-Prüfung,
- sauberer Shutdown nach jedem Ablauf.

### H7: `host::run` und die Host-Fassade abschließen

- **Art:** Abschluss
- **Owner:** `host`
- **Benötigt:** H6 und alle dort erfassten Entscheidungen

Zum Schluss wird der gesamte Ablauf von `host::run(config_source)` geprüft. Mit dem Feature `host`
bleibt `host::run` der einzige öffentliche Export. Ohne das Feature existiert `host` nicht. Private
CLI-, Config-, Controller- und Orchestrierungstypen dürfen nicht nach außen gelangen.

## Kurzform der Kausalkette

```text
I1 + I2 + I3 + I4 -> I5 -> P1 ----------------------+
T1 -----------------------> P1                      |
R1 + R2 + R3 -> R4 --+                              |
R5 ------------------+-> R6 -> R7 -> R8 -> RP1 -> RP2
                                  |                  |
                                  +-> SR1 -----------+
R5 -------------------------------------------------+-> S1 -> S2
                                                         |
I5 + P1 + S2 -> H1                                      |
P1 + S2 -> H2 -> H4                                     |
S2 -------> H3 -> H4                                    |
H1 + H4 -> H5 -> H6 -> H7 <-----------------------------+
```

## Nicht als vorgelagerte API-Entscheidung behandeln

Folgende Punkte bleiben bewusst bei der Implementation und blockieren die Reihenfolge nicht:

- die konkrete Datenstruktur für mehrere laufende Commands im Bevy-Event-Loop,
- private In-Memory-Ein-/Ausgabeadapter für Tests,
- private Betriebssystem- und Prozessfehlerdetails innerhalb der festgelegten Fehlergruppen,
- private Hilfsfunktionen, genaue Validierungsreihenfolgen und interne Scheduler-Budgets.

Wenn einer dieser Punkte doch eine öffentliche Signatur, einen Fehlerfall oder eine garantierte
Reihenfolge verändert, wird er vor der Implementation als neuer `OPEN`-Punkt an der zuständigen
Stelle aufgenommen.
