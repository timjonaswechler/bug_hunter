# Session transportiert Report-Beobachtungen

Historischer Entscheidungsstand. Für den Rewrite gilt [target.md](../api/target.md).

## Status

Angenommen.

Die damalige Platzierung des gemeinsamen Report-Ablaufs unter `host::run` wird durch
[ADR-0024](0024-separate-server-client-and-cli.md) ersetzt. Der Ablauf liegt nun in `server`;
Markertransport, Fehlerdeutung und Provider-Regeln dieser ADR bleiben unverändert.

## Kontext

`session::Session` besitzt den Kindprozess, seine Pipes, den Prozesslebenszyklus und die
Command-History. `report` besitzt dagegen die fachliche Bedeutung von Panic-, Tracing- und
Prozessfehlern. Würde `Session` Marker selbst deuten oder Report-Daten speichern, läge dieselbe
Reporting-Regel in zwei Modulen.

Ein Prozess kann enden, bevor der Host den letzten Panic-Marker aus stderr gelesen hat. Ein sofort
aus dem Prozessstatus erzeugter Report würde dann den Panic durch einen allgemeineren Prozessfehler
ersetzen. stderr muss außerdem weiter gelesen werden, während der Host einen Provider aufruft, damit
der Kindprozess nicht an einer vollen Pipe blockiert.

`Report::create` benötigt eine erfolgreich gestartete Session. Vor dem Ready-Handshake fehlen die
bestätigte Protokollversion und die Capabilities für den Report-Kontext.

## Entscheidung

1. `session` besitzt weiterhin Kindprozess, Pipes, Prozessstatus und History. Ein crate-interner
   `report::Observer` besitzt den stderr-Zeilenpuffer und deutet ausschließlich gültige interne
   Reporting-Marker.
2. Die interne Prozessverwaltung reicht alle stderr-Bytes an den Observer. Gültige Marker werden
   nicht an die menschliche stderr-Ausgabe weitergereicht. Alle anderen Bytes bleiben unverändert.
   Ein ungültiger oder bei EOF unvollständiger Marker bleibt in der stderr-Ausgabe sichtbar und
   erzeugt einen typisierten Beobachtungsfehler.
3. Marker verwenden ein versioniertes, zeilenbasiertes Chunk-Format. Jeder Chunk enthält Event-ID,
   Chunk-Index, Chunk-Anzahl, Base64-kodierten JSON-Payload und Prüfsumme. Ein Chunk wird mit einem
   einzelnen, begrenzten Schreibaufruf ausgegeben. Dadurch bleiben mehrzeilige Backtraces erhalten
   und gleichzeitig geschriebene Marker können anhand ihrer Event-ID zusammengesetzt werden.
4. Es gibt Marker für Panics, Tracing-Errors und den Registrierungsstatus des Tracing-Error-Layers.
   `session::Plugin` schreibt den Statusmarker vor Abschluss des Ready-Handshakes. `Session::start`
   verarbeitet Statusmarker und stdout-`Ready` unabhängig von ihrer beobachteten Reihenfolge.
5. Bei `report.tracing_errors = true` gibt `Session::start` erst nach einer positiven
   Layer-Bestätigung eine Session zurück. Eine negative Bestätigung ergibt `session::Error::Launch`.
   Bei deaktivierter Tracing-Beobachtung verhindert ein fehlender Layer den Start nicht. Der Status
   wird weder Protokollfeld noch Capability.
6. Ein eigener stderr-Leser beginnt direkt nach dem Prozessstart und liest bis EOF weiter, auch wenn
   der Host gerade einen Command oder Provider-Aufruf bearbeitet. Nach einem Prozessende verarbeitet
   die Session erst die verbleibenden stdout- und stderr-Daten und bewertet danach den finalen
   Prozessstatus.
7. Jeder vollständige Panic-Marker erzeugt `Failure::panic`. Ein aktivierter Tracing-Marker erzeugt
   `Failure::tracing_error`. Ein nicht absichtlich ausgelöstes Prozessende erzeugt erst nach dem
   vollständigen stderr-Drain `Failure::process_exit`.
8. Sobald die Session einen Panic-Marker erkannt hat, erzeugt ein späteres Prozessende keinen
   zusätzlichen `ProcessExit`-Fehler. Ein beschädigter oder unvollständiger Panic-Marker beweist
   keinen Panic; bei einem unerwarteten fehlgeschlagenen Prozessstatus bleibt `ProcessExit` der
   Rückfall.
9. `session::Event::Failure` transportiert einen erkannten `report::Failure`.
   `session::Event::ObservationError` transportiert einen Marker- oder Beobachtungsfehler. Beide
   stehen vor einem abschließenden `Event::Ended` in der Event-Queue. `Ended` bleibt das letzte
   Session-Event. Ungültige und bei EOF unvollständige Marker verwenden den stabilen Code
   `invalid_report_marker`.
10. Der gemeinsame private Ablauf unter `host::run` behandelt `Event::Failure`. Er ruft zuerst
    `Report::create(failure, session)` und danach `report::submit(&report, session)` auf. REPL, Agent
    und Script besitzen keine eigene Report-Auslösung.
11. Der Host behält den erzeugten `Report` zusammen mit dem Ergebnis von `report::submit`. Dadurch
    bleiben Report, Provider-Outcome und ein typisierter Submit-Fehler gemeinsam sichtbar. Die
    konkrete Text-, JSON- und Exit-Code-Darstellung wird mit dem Host-Interface festgelegt.
12. Ein Panic vor einem vollständigen Ready-Handshake kann keinen normalen Report erzeugen. Er bleibt
    Diagnose des `session::Error::Launch`, weil noch keine gültige Session für `Report::create`
    existiert. Ein durch erfolgreichen Shutdown oder `Session::drop` ausgelöstes Prozessende erzeugt
    keinen Report.

## Folgen

- `session` transportiert fachliche Fehler, definiert ihre Erkennung aber nicht selbst.
- `report` kopiert weder Prozesszustand noch History. `Report::create` liest seinen begrenzten
  Kontext weiterhin direkt aus der Session.
- Der Host besitzt genau einen Ablauf für automatische Reports. Andere Nutzer von `Session` können
  dieselben `Failure`-Events mit der öffentlichen Report-API behandeln.
- Ein langsamer Provider hält den Host-Ablauf auf, aber nicht das Leeren der Prozess-Pipes.
- Mehrere beobachtete Panics oder Tracing-Errors dürfen mehrere `Failure`-Events erzeugen. Signatur
  und Provider entscheiden, ob bereits ein persistierter Report existiert.
- Das vollständige stderr-Leeren verhindert, dass der Prozessstatus einen bereits geschriebenen,
  aber noch nicht gelesenen Panic-Marker überholt.

## Prüfung

- Prozess-Fixtures teilen Marker an jeder Byteposition und mischen Chunks zweier Event-IDs.
- Fixtures prüfen mehrzeilige Backtraces, gewöhnliche stderr-Ausgabe, ähnlich aussehende Texte,
  ungültige Marker, fehlende Chunks und ein EOF ohne abschließenden Zeilenumbruch.
- Start-Fixtures prüfen beide beobachtbaren Reihenfolgen von Statusmarker und `Ready`, eine positive
  und negative Layer-Bestätigung sowie deaktivierte Tracing-Beobachtung.
- Prozess-Fixtures prüfen Panic vor erfolgreichem und fehlgeschlagenem Exit, fehlgeschlagenen Exit
  ohne Panic, Panic bei Exit-Status 0 sowie absichtlichen Shutdown und Drop.
- Event-Fixtures prüfen, dass `Failure` und `ObservationError` vor dem genau einmaligen `Ended`
  eintreffen und danach keine weiteren Events angenommen werden.
- Host-Fixtures prüfen den gemeinsamen Aufruf von `Report::create` und `report::submit`, ein
  erfolgreiches Provider-Outcome und einen Submit-Fehler mit weiterhin lesbarem Report.
