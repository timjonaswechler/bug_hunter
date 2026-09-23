# Replay validiert Recordings vollständig vor der Ausführung

Historischer Entscheidungsstand. Für den Rewrite gilt [target.md](../api/target.md).

## Status

Angenommen.

## Kontext

ADR-0015 legt das versionierte JSONL-Format fest, das der Recorder schreibt. Replay benötigt die
umgekehrte Abbildung von einer persistierten Formatversion auf die aktuelle qualifizierte
Command-Form. Dieser Adapter darf weder transportlokale Request-IDs wiederherstellen noch
aufgezeichnete Outcomes zu Erwartungen an den neuen Lauf machen.

Eine zeilenweise Ausführung während des Lesens wäre zwar speichersparend, könnte aber bereits
Commands ausführen, bevor ein ungültiger Footer, eine falsche Anzahl Einträge oder eine beschädigte
spätere Zeile entdeckt wird. Der Aufrufer könnte diesen teilweise ausgeführten Lauf nicht
zurücknehmen.

Recording-Dateien können außerdem von Hand verändert oder aus einer älteren Formatversion stammen.
Replay muss daher Dateisystemfehler, eine ungültige Datei und eine nicht unterstützte Version stabil
unterscheiden. Der angegebene Pfad darf den gemeinsamen Artifact-Root nicht verlassen oder über
Symlinks auf ein anderes Ziel umgeleitet werden.

## Entscheidung

### Privater Ladeadapter

Ein privater Recording-Adapter besitzt die persistierten Typen jeder unterstützten Formatversion.
Er liefert entweder einen typisierten `ReplayPlan` in der aktuellen Command-Form oder einen
typisierten Ladefehler. Der Replay-Koordinator kennt keine JSONL-Zeilen, Versionsdetails oder
persistierten Command-Typen.

Eine weitere unterstützte Formatversion ergänzt einen neuen Adapter. Sie verändert weder das
aktuelle Command-Interface noch die Ausführung eines bereits gebauten Plans.

Nach der Annahme von `Replay::Start { path }` wechselt Replay von `Idle` nach `Preparing`. Der Loader:

1. öffnet die Datei sicher unter dem Artifact-Root,
2. liest und validiert die gesamte Datei,
3. prüft ihre strukturellen und commandspezifischen Zusammenhänge,
4. baut den vollständigen Replay-Plan,
5. gibt bis zum erfolgreichen Abschluss keinen Plan-Command an die Controlled Session weiter.

Erst danach wechselt Replay nach `Running`. Jeder Ladefehler verwirft den Plan und führt Replay nach
`Idle` zurück. Es gibt keine teilweise Ausführung eines ungültigen Recordings.

Der Adapter darf die Datei zeilenweise lesen und bereits validierte Outcomes verwerfen, wenn sie für
den Plan nicht mehr benötigt werden. "Vollständig laden" bedeutet, dass Replay die gesamte Datei
validiert und den gesamten Plan baut, bevor es den ersten Command ausführt. Es verlangt keine
dauerhafte Kopie aller ursprünglichen JSON-Zeilen im Speicher.

### Pfadregeln

Replay verwendet dieselbe normalisierte relative Pfadform wie Recording. Der Pfad:

- ist nicht leer,
- ist UTF-8,
- verwendet `/`,
- endet auf `.jsonl`,
- enthält keine leeren Komponenten,
- enthält weder `.` noch `..`,
- enthält keine Backslashes,
- ist nicht absolut,
- durchläuft keine Symlinks,
- bezeichnet selbst keinen Symlink,
- bezeichnet eine vorhandene reguläre Datei.

Der Loader öffnet die Datei relativ zum bereits geöffneten Artifact-Root, ohne Symlinks zu folgen.
Eine rein vorgezogene Pfadprüfung reicht nicht aus, weil das Ziel zwischen Prüfung und Öffnen
ausgetauscht werden könnte.

Ein syntaktisch ungültiger Pfad, ein Pfad außerhalb des Roots, ein Symlink oder ein nicht reguläres
Ziel wird mit dem bereits festgelegten Code `invalid_recording_path` abgelehnt. Replay führt dafür
keinen zweiten Code `invalid_replay_path` ein. Eine fehlende, nicht lesbare oder während des Lesens
fehlerhafte Datei ergibt dagegen `session::Error::Io`.

### JSONL-Struktur

Eine gültige und unterstützte Recording-Datei erfüllt alle folgenden Regeln:

1. Die erste Zeile ist genau ein `recording_started`-Objekt.
2. Danach folgen null oder mehr `command`-Objekte.
3. Das letzte Objekt ist genau ein `recording_ended`-Objekt.
4. Vor dem Header und nach dem Footer stehen keine weiteren Daten oder Leerzeilen.
5. Innerhalb der Datei stehen keine Leerzeilen.
6. Jede Zeile enthält genau ein syntaktisch gültiges JSON-Objekt.
7. Unbekannte und doppelte Felder sind auf jeder Ebene ungültig.
8. Unbekannte Werte für `type` sind ungültig.
9. `recorded_commands` entspricht exakt der Anzahl der `command`-Zeilen.
10. Ein fehlender oder unvollständiger Footer ist ungültig.

Ein Recording ohne Command-Einträge ist gültig. Nach einem vollständigen Footer darf die Datei mit
einem LF enden; ein abschließendes LF ist keine Voraussetzung für die Gültigkeit.

### Versionsprüfung

Der Loader wertet zunächst nur den Header aus:

1. Ein fehlender oder nicht erkennbarer Header und eine syntaktisch ungültige `format_version`
   ergeben `invalid_recording`.
2. Eine syntaktisch gültige, aber nicht unterstützte Version ergibt
   `unsupported_recording_version`.
3. Nur eine unterstützte Version wird mit ihren vollständigen Formatregeln weiter validiert.

`unsupported_recording_version` hat Vorrang vor Fehlern in späteren Zeilen. Der Loader interpretiert
eine unbekannte Version nicht nach den Regeln von Version 1.

### Outcome-Formen in Version 1

Version 1 akzeptiert genau diese Outcome-Formen:

```json
{"status":"completed","output":null}
```

```json
{"status":"rejected","error":{"code":"entity_not_found","message":"..."}}
```

```json
{"status":"protocol_failed","error":{"code":"invalid_response","message":"..."}}
```

```json
{"status":"io_failed","error":{"message":"..."}}
```

```json
{"status":"unanswered"}
```

Bei `completed` validiert der Adapter `output` gegen den Output-Typ der konkreten qualifizierten
Command-Variante. Ein syntaktisch passender Output einer anderen Variante ist ungültig. Die übrigen
Outcome-Formen besitzen ausschließlich die dargestellten Felder.

`unanswered` ist nur zusammen mit einem Footer mit `outcome: "session_ended"` zulässig. Ein
`stopped`-Recording mit einem `unanswered`-Eintrag ist ungültig. Ein `session_ended`-Recording darf
auch abgeschlossene, abgelehnte oder technisch fehlgeschlagene Einträge enthalten.

### Zulässige Commands

Der Adapter akzeptiert ausschließlich die in ADR-0015 als aufnehmbar festgelegten Commands:

- Input,
- Tick und Warp,
- Inspect,
- Screenshot.

Er lehnt unbekannte qualifizierte Command-Namen, ungültige Argumente sowie Recording-,
Replay- und andere nicht aufnehmbare Steuercommands als `invalid_recording` ab. Dadurch kann eine
manipulierte Datei weder rekursiv Replay starten noch die Session herunterfahren.

### Bedeutung aufgezeichneter Outcomes

Aufgezeichnete Outcomes sind keine Erwartungen für den neuen Lauf. Deshalb machen `rejected`,
`protocol_failed`, `io_failed` und `unanswered` ein ansonsten korrektes Recording nicht ungültig.
Replay führt den zugehörigen Command erneut aus und vergleicht sein neues Outcome nicht mit dem
aufgezeichneten Outcome.

Davon getrennt bleibt ein technischer Fehler während des neuen Replays. Liefert ein neu ausgeführter
Plan-Command `ProtocolFailed` oder `IoFailed` oder bleibt er wegen eines Session-Endes unbeantwortet,
wendet der Replay-Koordinator die bereits festgelegten `Blocked`-Regeln auf den laufenden Replay an.

Nur ein erfolgreich beantworteter Warp-Start stellt Planungsdaten aus seinem aufgezeichneten Outcome
bereit. Für einen `tick.warp.start` mit `completed` prüft der Adapter:

- der Output ist `Finished`,
- `requested_ticks` entspricht dem ursprünglichen Command,
- `executed_ticks` ist nicht größer als `requested_ticks`,
- bei Warp-Outcome `Completed` entspricht `executed_ticks` den angeforderten Ticks,
- bei Warp-Outcome `Stopped` darf `executed_ticks` kleiner sein.

Der Plan verwendet `executed_ticks` als effektive Tickzahl. Bei null Ticks entfällt der Warp. Den
aufgezeichneten Stop, der diesen Warp beendet hat, führt Replay nicht erneut aus.

Besitzt ein Warp-Start kein `completed`-Outcome, gibt es keine vertrauenswürdige effektive Tickzahl.
In diesem Fall bleibt der ursprüngliche Warp-Command im Plan. Sein aufgezeichnetes Outcome wird wie
bei anderen Commands nicht als Erwartung verwendet.

### Fehlerzuordnung

| Situation | Terminales Ergebnis von `Replay::Start` |
| --- | --- |
| Ungültiger Pfad, Root-Verstoß, Symlink oder nicht reguläres Ziel | `Rejected { code: "invalid_recording_path", message }` |
| Datei fehlt, kann nicht geöffnet oder nicht vollständig gelesen werden | `session::Error::Io` |
| Ungültiger Header, Eintrag, Command, Outcome oder Footer | `Rejected { code: "invalid_recording", message }` |
| Syntaktisch gültige, aber nicht unterstützte Formatversion | `Rejected { code: "unsupported_recording_version", message }` |

Die lesbare Meldung enthält den Pfad und, soweit bekannt, die Zeilennummer. Programme werten nur den
stabilen Code aus.

Alle Ladefehler treten nach der Command-Annahme über das `Pending` von `Replay::Start` auf. Sie
erzeugen kein Session-Event und kein `RecordingFailed`, weil kein aktiver Recorder fehlgeschlagen
ist. In der History erscheinen Ablehnungen als `Rejected` und Dateisystemfehler als `IoFailed`.

### Stop während der Vorbereitung

Das Laden blockiert den privaten Session-Koordinator nicht. Eine private Ladeaufgabe liest und
validiert die Datei, während der Koordinator weiterhin Transport, Events und Commands verarbeitet.

Wird während `Preparing` ein `Replay::Stop` angenommen:

1. bricht Replay die Vorbereitung ab,
2. führt es keinen Plan-Command aus,
3. endet der ursprüngliche Start mit `Stopped`,
4. endet Stop mit `was_running: true`,
5. kehrt Replay nach `Idle` zurück.

Die Ladeaufgabe bestätigt ihren Abbruch, bevor Replay einen neuen Start zulässt. Dadurch sammeln
sich bei wiederholtem Start und Stop keine verlassenen Ladeaufgaben an. Trifft ein Ladefehler vor
der Stop-Annahme beim Koordinator ein, beendet stattdessen dieser Fehler den Start; der anschließende
Stop ist dann der erfolgreiche No-op mit `was_running: false`.

## Folgen

- Eine ungültige spätere Zeile oder ein ungültiger Footer kann nicht zu einem teilweise ausgeführten
  Replay führen.
- Versionswissen und persistierte Typen bleiben im Recording-Adapter. Der Replay-Koordinator erhält
  ausschließlich aktuelle Commands.
- Große Outcomes müssen nach ihrer Validierung nicht bis zum Replay-Ende im Speicher bleiben.
- Strikte unbekannte und doppelte Felder verhindern eine mehrdeutige Auslegung manipulierter Dateien.
- Ein gültiges `session_ended`-Recording bleibt trotz `unanswered`-Einträgen replaybar.
- Aufgezeichnete technische Fehler beschreiben weiterhin den ursprünglichen Lauf. Nur neu
  auftretende technische Fehler blockieren den neuen Replay.

## Verworfene Alternativen

Eine Ausführung während des zeilenweisen Lesens wurde verworfen. Sie spart den Plan im Speicher,
erlaubt aber irreversible Teilausführung vor einem später erkannten Dateifehler.

Eine tolerante Best-Effort-Auslegung unbekannter Felder oder Commands wurde verworfen. Sie könnte
einer Datei eine andere Bedeutung geben als der Recorder, der sie erzeugt hat. Neue Formen benötigen
stattdessen eine neue Formatversion und einen eigenen Adapter.

Ein eigener Ablehnungscode `invalid_replay_path` wurde verworfen. Recording und Replay verwenden
dieselbe Pfadform für dieselbe Artefaktart; `invalid_recording_path` beschreibt beide Fälle.

## Prüfung

- Format-Fixtures prüfen fehlende, doppelte und unbekannte Header, Einträge und Footer sowie
  Leerzeilen, zusätzliche Daten und falsche Command-Zähler.
- Versions-Fixtures prüfen die Fehlerpriorität für ungültige und nicht unterstützte
  `format_version`-Werte.
- Command-Fixtures prüfen alle zulässigen qualifizierten Commands, ungültige Argumente,
  commandspezifische Outputs und ausgeschlossene Steuercommands.
- Outcome-Fixtures prüfen alle fünf Statusformen und die Bindung von `unanswered` an
  `session_ended`.
- Warp-Fixtures prüfen angeforderte und ausgeführte Ticks, gestoppte Warps, null Ticks und
  nicht abgeschlossene Warp-Outcomes.
- Pfad-Fixtures prüfen alle verbotenen Komponenten, Symlinks, nicht reguläre Ziele, fehlende Dateien
  und Lesefehler.
- Eine beschädigte letzte Zeile und ein falscher Footer führen zu keinem ausgeführten Plan-Command.
- Nebenläufigkeitstests prüfen Stop während `Preparing`, die Bestätigung der Ladeaufgabe und einen
  direkt anschließenden neuen Replay-Start.
