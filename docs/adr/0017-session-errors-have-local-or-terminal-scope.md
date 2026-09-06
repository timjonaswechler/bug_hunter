# Session-Fehler haben lokalen oder terminalen Geltungsbereich

## Status

Angenommen.

## Kontext

`session::Error` wird von Start, Command-Annahme, Ergebnisempfang und Shutdown verwendet. Ohne eine
gemeinsame Wirkungsregel müsste jeder Aufrufer aus einzelnen Varianten erraten, ob die Session nach
einem Fehler weiter verwendbar ist. Besonders ein I/O- oder Protokollfehler kann entweder nur eine
hostseitige Command-Ausführung betreffen oder anzeigen, dass der gesamte Transport nicht mehr
zuverlässig ist.

`Session::shutdown` liegt außerhalb des allgemeinen `send`-Ablaufs. Es darf keine laufende Arbeit
heimlich stoppen und muss das einmalige Wire-Shutdown mit der reservierten Request-ID `u64::MAX`
ausführen. Die bisherige vorläufige Beschreibung verwendete `InvalidPending`, wenn offene Commands
oder ein aktives Recording den Shutdown verhindern. Diese Variante bezeichnet aber bereits die
falsche Verwendung eines konkreten `Pending` und beschreibt keine Session-Grenze.

Nach dem Senden des Wire-Shutdowns kann die Session nicht einfach zu normaler Arbeit zurückkehren.
Die reservierte Request-ID darf nicht wiederverwendet werden, und die Controlled Session könnte den
Abschluss bereits begonnen haben. Vor und nach diesem Punkt benötigt Shutdown deshalb
unterschiedliche Fehlerwirkungen.

## Entscheidung

### Äußerer Session-Zustand

Der äußere Lebenszyklus verwendet intern `Running`, `ShuttingDown` und `Ended`.

- `Running` erlaubt normale Session-Operationen.
- `ShuttingDown` beginnt, sobald die Shutdown-Vorbedingungen erfüllt sind und die reservierte
  Shutdown-ID angenommen wurde.
- `Ended` ist terminal. Die Session kehrt daraus nie nach `Running` zurück.

Recording und Replay behalten ihre eigenen Zustände. Sie bestimmen, ob die Session die
Shutdown-Grenze erreicht hat, erweitern aber nicht den äußeren Lebenszyklus.

### Fehlergruppen und Wirkung

`session::Error` erhält die zusätzliche Variante:

```rust
ShutdownBlocked {
    code: String,
    message: String,
}
```

Die öffentlichen Fehlergruppen haben diese Bedeutung:

| Fehler | Bedeutung | Wirkung |
| --- | --- | --- |
| `InvalidConfig` | Session- oder Launch-Konfiguration ist ungültig | Entsteht nur bei `Session::start`; keine Session wird erzeugt |
| `Launch` | Der Prozess erreicht keinen gültigen Ready-Handshake | Keine Session wird erzeugt; gestartete Ressourcen werden aufgeräumt |
| `Io` | Eine lokale Datei- oder Betriebssystemoperation eines bestimmten Aufrufs ist fehlgeschlagen | Der Aufruf endet; eine bereits gestartete Session bleibt nutzbar, sofern nicht ihr Transport betroffen ist |
| `InvalidPending` | Ein `Pending` ist fremd, bereits entnommen oder anderweitig ungültig | Nur der Empfangsaufruf scheitert |
| `RequestIdExhausted` | Es ist keine normale Request-ID mehr verfügbar | Der Command wird nicht angenommen; laufende Commands und reservierter Shutdown bleiben möglich |
| `ShutdownBlocked` | Die Session hat die Shutdown-Grenze noch nicht erreicht | Kein Shutdown wird gesendet; die Session bleibt unverändert |
| `Rejected` | Ein angenommener Command wurde fachlich abgelehnt | Nur dieser Command endet |
| `Protocol` | Für einen bekannten Aufruf kam kein gültiges Protokollergebnis zustande | Nur dieser Aufruf endet, solange der Transport weiter verwendbar ist |
| `Ended` | Prozess, Transport oder Koordinator ist terminal beendet | Die Session bleibt endgültig beendet |

Die Quelle bestimmt den Geltungsbereich. Derselbe Betriebssystemfehler kann deshalb bei einer
Replay-Datei ein lokales `Io` und bei einer stdout-Pipe ein terminales Session-Ende sein.

### Lokale Fehler

Diese Fehler beenden eine gestartete Session nicht:

- fachliche Command-Ablehnungen,
- ein nicht als erwarteter Command-Output dekodierbares Ergebnis,
- eine Protokollfehlermeldung mit bekannter Request-ID,
- ein fremdes oder bereits entnommenes `Pending`,
- erschöpfte normale Request-IDs,
- Fehler beim Laden eines Replay-Recordings,
- Fehler beim Starten oder Stoppen eines Recordings,
- ein asynchroner Recording-Schreibfehler,
- eine nicht zuordenbare Protokollfehlermeldung,
- ein blockierter Shutdown.

Ein angenommener Command erhält dabei genau ein terminales Ergebnis. Andere Commands laufen weiter.
Ein commandspezifischer I/O-Fehler erscheint in der History als `IoFailed`.

Ein asynchroner Recording-Schreibfehler folgt ADR-0015. Er verändert das Outcome des gerade
ausgeführten Anwendungs-Commands nicht, beendet nur das Recording und erzeugt
`Event::RecordingFailed`.

Eine Protokollfehlermeldung mit bekannter Request-ID und ein nicht dekodierbarer erfolgreicher Output
schließen genau das zugehörige `Pending` mit `Protocol` und den History-Eintrag mit `ProtocolFailed`.
Eine unbekannte, bereits beantwortete oder fehlende Request-ID erzeugt `Event::ProtocolError`, ohne
einen anderen Command zu schließen. Diese Protokollfehler beenden die Session nicht.

Ein ungültiger Ready-Handshake ist davon getrennt. Er tritt vor einer nutzbaren Session auf und lässt
`Session::start` mit `Protocol` fehlschlagen.

### Terminale Fehler

Nach einem erfolgreichen Ready-Handshake beenden folgende Ursachen die Session:

- ein unerwartetes Prozessende,
- EOF auf einem noch benötigten stdin-, stdout- oder stderr-Kanal,
- ein I/O-Fehler auf einem benötigten Transportkanal,
- ein Überlauf der Event-Queue,
- der Verlust des privaten Session-Koordinators.

Ein stdin-Schreibfehler ist kein lokales `IoFailed` des Commands, dessen Ausgabe den Fehler bemerkt.
Der Transport ist danach nicht mehr zuverlässig. Die Session endet, und alle zu diesem Zeitpunkt
noch offenen Wire-Commands bleiben `Unanswered`.

Bei einem terminalen Fehler:

1. nimmt der Koordinator keine weiteren Commands an,
2. bewahrt er genau einen primären Endgrund,
3. leert er stdout und stderr soweit möglich,
4. gibt ein laufendes Replay keine weiteren Plan-Commands frei,
5. endet der ursprüngliche Replay-Start mit `Blocked { code: "session_ended", message }`,
6. schließt er ein aktives Recording nach Möglichkeit mit `unanswered` und `session_ended`,
7. bleiben andere offene Commands in der History `Unanswered` und liefern über ihr `Pending`
   `Error::Ended`,
8. bleiben bereits terminal vorliegende Ergebnisse einmal abrufbar,
9. werden noch entstehende Failure-, Observation- und Recording-Events eingereiht,
10. wird `Event::Ended` als letztes Session-Event eingereiht,
11. beendet und sammelt der Koordinator den Kindprozess ein, falls er noch läuft.

Bei `EventQueueOverflow` bleibt nur der reservierte Platz für `Ended`. Passt ein beim
Recording-Abschluss entstandenes `RecordingFailed` nicht mehr in die Queue, erhöht es
`dropped_events`. Das terminale Event wird nicht verdrängt.

Kann der Session-Handle den Koordinator selbst nicht mehr erreichen, liefern fallible Operationen
`Error::Ended`. Ein terminales Event ist in diesem Sonderfall nicht garantiert, weil gerade sein
Produzent nicht mehr verfügbar ist.

### Erschöpfte Request-IDs während Replay

Kann Replay keinen internen Plan-Command mehr annehmen, endet der ursprüngliche Replay-Start mit:

```rust
Blocked {
    code: "request_id_exhausted",
    message,
}
```

`request_id_exhausted` ergänzt die stabilen technischen Replay-Blockierungscodes. Replay kehrt nach
`Idle` zurück. Die Session endet nicht, und `u64::MAX` bleibt für Shutdown reserviert.

Der nicht angenommene Plan-Command erhält weder Request-ID noch History-Eintrag. Bereits angenommene
Commands laufen bis zu ihrem terminalen Ergebnis weiter.

### Shutdown-Vorbedingungen

`Session::shutdown(&mut self)` darf nur von `Running` nach `ShuttingDown` wechseln, wenn:

- kein angenommener Command mehr offen ist,
- die aktive Command-Tabelle leer ist,
- der Recorder `Idle` ist.

Ein Command bleibt unabhängig davon offen, ob der Aufrufer sein `Pending` noch besitzt. Die Prüfung
erfasst dadurch normale Wire-Commands, laufende Warps, alle Replay-Zustände, Recording-Übergänge und
intern vom Replay ausgeführte Plan-Commands. Ein aktives Recording benötigt die zusätzliche
Recorderprüfung, weil es auch ohne offenen Command bestehen kann.

Nicht abgeholte Events, vorhandene History-Einträge und erschöpfte normale Request-IDs blockieren
Shutdown nicht.

Ein blockierter Shutdown verwendet diese stabilen Codes:

| Code | Bedeutung |
| --- | --- |
| `shutdown_commands_pending` | Mindestens ein früher angenommener Command ist noch nicht terminal. |
| `shutdown_recording_active` | Es gibt keine offenen Commands, aber der Recorder ist noch `Active`. |

Sind beide Bedingungen erfüllt, hat `shutdown_commands_pending` Vorrang. Nach Abschluss der Commands
kann ein erneuter Aufruf den noch aktiven Recorder melden.

Ein blockierter Shutdown:

- sendet keinen Wire-Request,
- verbraucht die reservierte Shutdown-ID nicht,
- erzeugt keinen History-Eintrag,
- verändert weder Session-, Replay- noch Recording-Zustand,
- kann nach Beseitigung des Hindernisses wiederholt werden.

Der Aufrufer muss laufende Arbeit ausdrücklich abschließen oder stoppen und ein aktives Recording
ausdrücklich mit `recording.stop` beenden. Shutdown sendet keine versteckten Stop-Commands.

### Angenommener Shutdown

Nach erfolgreicher Vorbedingungsprüfung:

1. wechselt die Session atomar nach `ShuttingDown`,
2. verwendet sie `u64::MAX` als einmalige Shutdown-ID,
3. fügt sie Shutdown als `Unanswered` in die History ein,
4. nimmt sie keine weiteren Commands an,
5. sendet der Koordinator den Wire-Shutdown,
6. wartet er auf die zugehörige erfolgreiche Response,
7. leert er stdout und stderr und wartet auf das Ende des Kindprozesses.

Ein erfolgreicher Shutdown benötigt eine gültige `completed`-Response mit `output: null` und danach
einen erfolgreichen Prozessstatus. Dann wird der History-Eintrag `Completed`, `shutdown` liefert
`Ok(())`, und die Session wechselt nach `Ended`. Erfolgreicher Shutdown erzeugt weiterhin weder
`Event::Ended` noch einen Report.

Alle späteren falliblen Session-Operationen einschließlich eines zweiten Shutdowns liefern
`Error::Ended`. `capabilities()` und die lesenden Eigenschaften bereits vorhandener `Pending`-Werte
bleiben als unveränderliche Snapshots verfügbar.

### Fehler nach der Shutdown-Annahme

Sobald der Wire-Shutdown gesendet wurde, ist der Vorgang endgültig. Die reservierte ID darf nicht für
einen zweiten Versuch verwendet werden.

- Eine fachliche Shutdown-Ablehnung liefert `Error::Rejected`.
- Eine ungültige zugeordnete Shutdown-Response liefert `Error::Protocol`.
- Ein Prozess- oder Transportende vor der gültigen Response liefert `Error::Ended`.
- Ein nicht erfolgreicher Prozessstatus nach einer gültigen Response liefert `Error::Ended`.
- Nach jedem dieser Ergebnisse räumt der Koordinator den Kindprozess auf und setzt die Session auf
  `Ended`.

Nur ein vor der Annahme gemeldetes `ShutdownBlocked` lässt einen späteren Shutdown-Versuch zu.

Eine direkte Shutdown-Ablehnung oder ein zugeordneter Protokollfehler steht bereits im Rückgabewert
und in der History. Der dadurch veranlasste Host-Abbruch erzeugt kein zusätzliches `Event::Ended` und
keinen Prozessfehler-Report. Endet Prozess oder Transport dagegen von selbst, gelten die Regeln aus
ADR-0014 einschließlich `Event::Ended`.

`Session::shutdown` besitzt vorerst keinen eingebauten Timeout. Ein fester Wert wäre für langsame
CI-Systeme und instrumentierte Builds willkürlich. Ein konfigurierbarer Wert würde das öffentliche
Interface ohne einen bekannten Grenzwert vergrößern. Ein später benötigtes Zeitlimit gehört als
ausdrückliche Host-Policy vor die Session und nicht als versteckte Session-Konstante in ihre
Implementation.

### Drop

`Session::drop` ist ein harter Ressourcenabschluss und kein Ersatz für Shutdown:

1. Drop sendet keinen Shutdown-, Recording-Stop-, Replay-Stop- oder Warp-Stop-Command.
2. Es wartet nicht auf Command-Outcomes.
3. Es erzeugt keine Session-Events und keinen Report.
4. Ein aktives Recording erhält keine Footer-Garantie.
5. Der Koordinator schließt die Pipes, beendet die Prozessgruppe und sammelt den Kindprozess ein.
6. Fehler während Drop können nicht an den Aufrufer zurückgegeben werden und werden nicht
   nachträglich zu Session-Outcomes.

Nach einem blockierten Shutdown bleibt die Session `Running`. Wird sie anschließend fallengelassen,
greift dieser harte Drop-Ablauf. Deshalb bleibt `shutdown` eine Methode auf `&mut self` und konsumiert
die Session nicht vor der Vorbedingungsprüfung.

## Folgen

- Aufrufer können jeder Fehlervariante entnehmen, ob nur ein Aufruf oder die gesamte Session
  betroffen ist.
- `InvalidPending` bezeichnet nur noch die falsche Verwendung eines `Pending`.
- Shutdown überschreitet eine ausdrücklich benannte Grenze und stoppt keine Arbeit im Verborgenen.
- Nach dem Senden der einmaligen Shutdown-ID gibt es keinen mehrdeutigen zweiten Versuch.
- Transportfehler erhalten unabhängig vom auslösenden Command dieselbe terminale Wirkung.
- Ein hängender Kindprozess kann ohne eine spätere ausdrückliche Host-Frist auch den synchronen
  Shutdown aufhalten.

## Verworfene Alternativen

`InvalidPending` für einen blockierten Shutdown wurde verworfen. Die Variante sagt weder, welcher
Zustand den Shutdown verhindert, noch bezeichnet sie in diesem Fall ein fehlerhaftes `Pending`.

Eine automatische Ausführung von Replay-, Warp- oder Recording-Stop während Shutdown wurde
verworfen. Sie würde zusätzliche Commands und Zustandsänderungen verbergen und könnte ein Recording
gegen den Willen des Aufrufers abschließen.

Ein erneuter Shutdown-Versuch nach dem Senden von `u64::MAX` wurde verworfen. Er müsste entweder eine
Request-ID wiederverwenden oder den Shutdown mit einer nicht reservierten ID senden.

Ein fester oder neuer konfigurierbarer Shutdown-Timeout wurde vorerst verworfen. Dafür gibt es noch
keinen belastbaren Grenzwert.

## Prüfung

- Fehler-Fixtures prüfen jede `session::Error`-Variante auf ihre lokale oder terminale Wirkung.
- Ein commandspezifischer I/O-Fehler lässt andere Commands und die Session weiterlaufen; ein
  Transport-I/O-Fehler beendet sie und lässt offene Commands `Unanswered`.
- Protokoll-Fixtures prüfen bekannte, unbekannte, doppelte und fehlende Request-IDs sowie einen
  ungültigen Ready-Handshake.
- Request-ID-Fixtures prüfen die Erschöpfung bei normalen Commands und während Replay sowie den
  weiterhin möglichen reservierten Shutdown.
- Shutdown-Fixtures prüfen offene Commands mit und ohne verbliebenes `Pending`, ein aktives Recording,
  die Priorität der beiden Blockierungscodes und einen erfolgreichen Wiederholungsaufruf.
- Nach Annahme des Wire-Shutdowns prüfen Fixtures Erfolg, Ablehnung, ungültige Response,
  Transportende und einen nicht erfolgreichen Prozessstatus. Nur der Erfolg erzeugt weder
  `Event::Ended` noch einen Report.
- Eine Overflow-Fixture lässt den Recording-Abschluss fehlschlagen und prüft, dass
  `dropped_events` steigt und der reservierte Platz `Ended` enthält.
- Drop-Fixtures prüfen laufende Commands, Replay und Recording sowie das Beenden und Einsammeln der
  Prozessgruppe ohne fachliche Abschlussgarantien.
