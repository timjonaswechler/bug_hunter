# Agent gibt Session-Events getrennt von Command-Outcomes aus

## Status

Teilweise wieder geöffnet durch ADR-0022.

## Kontext

`session::Event` transportiert Protokoll-, Recording-, Failure- und Beobachtungsereignisse sowie ein
terminales unerwartetes Session-Ende. Diese Ereignisse gehören keinem Command. Die
maschinenlesbare Agent-Ausgabe enthält zugleich korrelierte Command-Outcomes und lokale
Eingabefehler.

Ein unerwartetes Session-Ende lässt noch offene Commands unbeantwortet. Eine einzelne Endmeldung darf
deren terminale Meldungen nicht ersetzen. Umgekehrt muss ein Empfänger erkennen können, wann alle
Ausgaben dieser Session vollständig sind.

ADR-0022 ersetzt die ausschließlich flüchtige Agent-JSONL-Ausgabe durch einen gemeinsamen,
wiederaufnehmbaren Activity-Stream des Session-Hosts. Die Event- und EndReason-Payloads bleiben
beschlossen. Äußere Hülle, Cursor, Aufbewahrung und die Reihenfolge gegenüber anderen
Activity-Einträgen werden in HS3 und anschließend H3 erneut festgelegt.

## Entscheidung

1. Jedes Session-Event verwendet diese äußere JSONL-Form:

   ```json
   {"request_id":null,"status":"session_event","event":{"kind":"protocol_error","code":"unexpected_ready","message":"received a second ready message"}}
   ```

   `request_id` ist immer `null`. `status: "session_event"` unterscheidet die Meldung von
   `pending`, `completed`, `rejected`, `failed` und `input_error`.
2. Der Agent vergibt keine eigene Event-ID. Die Reihenfolge der JSONL-Zeilen und `event.kind`
   genügen zur Einordnung.
3. Die nicht terminalen Varianten werden unmittelbar abgebildet:
   - `ProtocolError` verwendet `kind: "protocol_error"`, `code` und `message`.
   - `RecordingFailed` verwendet `kind: "recording_failed"`, `path` und `message`.
   - `ObservationError` verwendet `kind: "observation_error"`, `code` und `message`.
   - `Failure` verwendet `kind: "failure"` und ein `failure`-Objekt.
4. Das `failure`-Objekt enthält `message` und `origin`. `origin.kind` ist `panic`,
   `process_exit` oder `tracing_error` und übernimmt die jeweiligen Felder aus
   `report::Origin`. Optionale Meldungen, Locations, Spalten, Backtraces und Targets werden
   ausdrücklich als `null` ausgegeben.
5. Nicht terminale Events beenden den Agent-Ablauf nicht. Der Agent liest weitere Eingaben, und
   angenommene Commands laufen weiter.
6. Session-Events werden untereinander in der FIFO-Reihenfolge der Session ausgegeben.
   Command-Outcomes dürfen nach ihrer Verfügbarkeit zwischen diesen Events erscheinen. Der Agent
   erfindet keine zusätzliche Gesamtordnung zwischen den getrennten Event- und Outcome-Strömen.
7. `Event::Ended` verwendet `kind: "ended"` und bildet den `EndReason` direkt als
   `reason`-Objekt ab. Die Kinds sind `process_exit`, `transport_closed`, `transport_failed` und
   `event_queue_overflow`; Transportkanäle heißen `stdin`, `stdout` und `stderr`.
8. Beim Empfang von `Ended` gibt der Agent zunächst alle bereits vorliegenden Command-Outcomes aus.
   Jeder danach noch unbeantwortete Command erhält in Annahmereihenfolge:

   ```json
   {"request_id":17,"command":"inspect.query","status":"failed","error":{"code":"session_ended","message":"session ended before command completed"}}
   ```

9. Nach allen normalen Events und Command-Outcomes gibt der Agent `Ended` als letzte JSONL-Meldung
   der Session aus. Danach nimmt er keine Eingabe mehr an und erzeugt keine weitere Meldung für diese
   Session.
10. Ein erfolgreicher `Session::shutdown` erzeugt gemäß dem Session-Vertrag kein `Ended`-Event.
11. `Failure` enthält in dieser Event-Form nur den beobachteten Fehler. Der gemeinsame private
    Host-Ablauf erzeugt und veröffentlicht weiterhin den Report. Die spätere Host-Planung legt eine
    getrennte Darstellung von Report und Provider-Ergebnis fest. Falls sie im Agent-Modus eine
    weitere JSONL-Meldung erzeugt, muss diese vor dem terminalen `Ended` erscheinen.

## Folgen

- Session-Events können nicht mit Outcomes eines bestimmten Commands verwechselt werden.
- Jeder angenommene Agent-Command erhält auch bei einem Session-Abbruch genau eine terminale
  Agent-Meldung.
- `Ended` ist eine verlässliche Stromgrenze: Danach folgt für diese Session keine Ausgabe mehr.
- Der Agent kopiert weder die Session-Eventtypen noch ihre Endgründe in ein zweites fachliches
  Modell; die JSON-Kinds spiegeln die bestehenden Enums.
- Die genaue Rust-Rückgabe des Agent-Ablaufs, EOF, kontrollierter Abschluss sowie die Darstellung von
  Report-Provider-Ergebnissen bleiben getrennte Host-Entscheidungen.

## Prüfung

- JSONL-Fixtures decken alle fünf `session::Event`-Varianten, alle vier `EndReason`-Varianten und alle
  drei `TransportChannel`-Werte ab.
- Failure-Fixtures decken alle drei `report::Origin`-Varianten und jedes optionale Feld als `null`
  sowie als vorhandenen Wert ab.
- Mehrere normale Events belegen ihre FIFO-Reihenfolge; ein Command-Outcome zwischen zwei Events
  belegt die erlaubte Verzahnung.
- Terminale Fixtures prüfen bereits beantwortete und noch unbeantwortete Commands, die
  Annahmereihenfolge der synthetisierten `failed`-Meldungen und `Ended` als letzte JSONL-Zeile.
- Eine Panic- und eine Recording-Fixture prüfen `Failure` beziehungsweise `RecordingFailed` vor
  `Ended`.
- Ein erfolgreicher Shutdown erzeugt kein `Ended`.
