# Recordings sind geordnete, versionierte JSONL-Abschnitte

Historischer Entscheidungsstand. Für den Rewrite gilt [target.md](../api/target.md).

## Status

Angenommen.

## Kontext

Ein Recording muss während einer langen Session schrittweise geschrieben werden können. Zugleich
darf die Reihenfolge ungeordnet eintreffender Responses nicht bestimmen, in welcher Reihenfolge ein
späteres Replay die Commands ausführt. Wire-Request-IDs eignen sich nur zur Korrelation innerhalb
einer laufenden Session und dürfen nicht zur dauerhaften Identität eines Recording-Eintrags werden.

Start und Stop laufen als hostseitige Commands auf dem privaten Session-Koordinator. Sie müssen eine
klare Grenze zwischen aufgenommenen und nicht aufgenommenen Commands bilden, ohne die Controlled
Session anzuhalten oder einen versteckten Tick auszuführen.

Recording-Dateien liegen zusammen mit Screenshots und Reports unter dem in ADR-0010 festgelegten
Artifact-Root. Der Dateivertrag muss Überschreiben und das Verlassen dieses Roots verhindern. Er muss
außerdem zwischen einem kontrollierten Stop, einem unerwarteten Ende der Controlled Session und einer
durch `Session::drop` abgebrochenen Datei unterscheiden.

## Entscheidung

### Dateiformat

Jede Datei beschreibt genau einen Recording-Abschnitt als JSONL. Formatversion 1 hat diese Form:

```jsonl
{"type":"recording_started","format_version":1}
{"type":"command","command":"input.keyboard.press","arguments":{"key":"space"},"outcome":{"status":"completed","output":null}}
{"type":"command","command":"inspect.query","arguments":{},"outcome":{"status":"rejected","error":{"code":"entity_not_found","message":"..."}}}
{"type":"recording_ended","outcome":"stopped","recorded_commands":2}
```

Bei einem unerwarteten Ende der Controlled Session verwendet der Footer stattdessen:

```jsonl
{"type":"recording_ended","outcome":"session_ended","recorded_commands":2}
```

Für das Format gelten folgende Regeln:

1. Genau eine `recording_started`-Zeile eröffnet die Datei.
2. Danach folgt für jeden aufgenommenen Command genau eine vollständige JSON-Zeile.
3. Genau eine `recording_ended`-Zeile beendet eine gültige Datei.
4. `format_version` wird unabhängig von der Version des Session-Protokolls weiterentwickelt.
5. Die Dateireihenfolge ist die Annahmereihenfolge der Commands. Das Format speichert keine
   zusätzliche Sequenznummer.
6. Das Format speichert keine Wire-Request-ID.
7. Command und Argumente verwenden die gemeinsame qualifizierte Command-Abbildung.
8. Ein Outcome hat den Status `completed`, `rejected`, `protocol_failed`, `io_failed` oder
   `unanswered`.
9. Ein späterer Loader lehnt unbekannte Felder ab.
10. Der Recorder kürzt oder bereinigt weder Commands noch Outcomes. Recording-Dateien können deshalb
    vertrauliche Daten enthalten.

`unanswered` ist nur in einem Recording mit dem Abschluss `session_ended` zulässig.

### Aufgenommene Commands und Reihenfolge

Der Recorder nimmt die Commands auf, die tatsächlich an die Controlled Session gehen:

- Input,
- Tick und Warp,
- Inspect,
- Screenshot,
- intern von Replay ausgeführte Plan-Commands.

Er nimmt `recording.start`, `recording.stop`, `replay.start`, `replay.stop`, Report-Ausführung,
Session-Events und Shutdown nicht auf. Ein aktives Recording blockiert Shutdown weiterhin. Ein
Replay innerhalb eines aktiven Recordings erscheint flach als Folge seiner ausgeführten
Plan-Commands und nicht als Replay-Steuercommand.

Fachlich abgelehnte Commands werden mit ihrem terminalen Outcome aufgenommen. Sie sind Teil des
tatsächlich ausgeführten Ablaufs.

Responses dürfen in einer anderen Reihenfolge eintreffen. Der Recorder puffert fertige Einträge,
solange ein früher angenommener aufgenommener Command noch kein terminales Ergebnis hat. Er schreibt
die Einträge erst in Annahmereihenfolge. Die aktive Command-Tabelle hält die dafür nötigen Commands
ohnehin bis zu ihrem terminalen Ergebnis.

### Zustände und Barrieren

Der Recorder besitzt intern die vier Zustände `Idle`, `Starting`, `Active` und `Stopping`.

`recording.start` ist nur in `Idle`, ohne früher angenommene offene Commands und mit einem gültigen,
noch nicht belegten Pfad zulässig. Nach der Annahme wechselt der Recorder sofort zu `Starting`.
Später angenommene Commands warten hinter dieser Barriere. Sobald die Datei angelegt und der Header
geschrieben ist, wechselt der Recorder zu `Active`, beantwortet Start mit `Started` und gibt die
wartenden Commands frei. Diese Commands gehören zum neuen Recording-Abschnitt. Schlägt Start fehl,
kehrt der Recorder zu `Idle` zurück und gibt sie ohne Recording frei.

`recording.stop` ist nur in `Active` und ohne früher angenommene offene Commands zulässig. Nach der
Annahme wechselt der Recorder zu `Stopping`. Später angenommene Commands warten hinter der
Stop-Barriere. Der Recorder schreibt den Footer, leert seine Puffer, synchronisiert und schließt die
Datei. Erst danach antwortet Stop mit:

```rust
Stopped {
    path,
    recorded_commands,
}
```

Danach laufen die wartenden Commands ohne Recording weiter.

Commandspezifische Ablehnungen verwenden diese stabilen Codes:

| Code | Bedeutung |
| --- | --- |
| `recording_already_active` | Start wurde bei aktivem Recording gesendet. |
| `recording_not_active` | Stop wurde ohne aktives Recording gesendet. |
| `recording_transition_in_progress` | Start oder Stop läuft gerade. |
| `recording_commands_pending` | Frühere Commands verhindern die benötigte Grenze. |
| `invalid_recording_path` | Der Pfad ist syntaktisch ungültig oder verlässt den Artifact-Root. |
| `recording_path_exists` | Das Ziel existiert bereits. |

Wie andere commandspezifische Ergebnisse folgen diese Ablehnungen erst nach der Annahme und einem
`Pending`. Dateisystemfehler, die einen Start oder Stop abschließen, sind terminale
`session::Error::Io` und erscheinen in der History als `history::Outcome::IoFailed`. Für einen
Schreibfehler während `Active` gilt stattdessen die unten beschriebene Event-Regel.

### Pfade und Sichtbarkeit

`Start { path }` verwendet einen normalisierten UTF-8-Pfad relativ zum gemeinsamen Artifact-Root.
Der Pfad ist nicht leer, verwendet `/`, endet auf `.jsonl` und enthält keine leeren Komponenten,
keine Komponenten `.` oder `..` und keine Backslashes. Er ist nicht absolut.

Der Recorder durchläuft keine vorhandenen Symlinks. Das Ziel darf weder ein vorhandener Symlink noch
eine vorhandene Datei sein. Fehlende Elternverzeichnisse legt der Recorder an und prüft dabei jede
bereits vorhandene Komponente auf Symlinks. Er legt die Zieldatei mit `create_new` an und
überschreibt sie nie.

Der Recorder schreibt direkt in die endgültige Zieldatei. Dadurch bleibt nach einem Host-Absturz
möglicherweise eine sichtbar unvollständige Datei zurück. Replay muss eine Datei ohne gültigen Footer
als `invalid_recording` ablehnen. Wenn schon das Schreiben des Headers fehlschlägt, versucht der
Recorder, die neu angelegte leere oder unvollständige Datei zu entfernen.

### Fehler und Session-Ende

Ein Schreibfehler während eines aktiven Recordings verändert nicht das Outcome des gerade
ausgeführten Anwendungs-Commands. Der Recorder beendet nur die Aufnahme, wechselt zu `Idle`, lässt
die unvollständige Datei zur Diagnose bestehen und reiht folgendes normales Session-Event ein:

```rust
RecordingFailed {
    path: String,
    message: String,
}
```

Ein anschließendes `recording.stop` wird mit `recording_not_active` abgelehnt. Ein neues Recording
mit einem anderen Pfad bleibt möglich. `RecordingFailed` zählt gegen die Grenze von 256 normalen
Events aus ADR-0014.

Bei einem unerwarteten Ende der Controlled Session versucht der Koordinator, ein aktives Recording
abzuschließen. Er setzt die Outcomes aller aufgenommenen offenen Commands auf `unanswered`, schreibt
sie in Annahmereihenfolge, schreibt den Footer mit `outcome: "session_ended"`, synchronisiert und
schließt die Datei. Ein Fehler dabei erzeugt `Event::RecordingFailed` vor dem terminalen
`Event::Ended`.

`Session::drop` gibt diese Abschlussgarantie nicht. Drop versucht nur, Ressourcen freizugeben. Eine
Datei ohne Footer bleibt ungültig.

## Folgen

- Große Recordings benötigen nicht den Speicher für alle bereits geschriebenen Commands.
- Replay erhält die ursprüngliche Command-Reihenfolge, auch wenn Responses ungeordnet eintreffen.
- Persistierte Dateien bleiben unabhängig von transportlokalen Request-IDs und Protokollversion 3.
- Eine sichtbare Datei ist nicht automatisch gültig. Erst der Footer schließt den Recording-Vertrag
  ab.
- Start und Stop können spätere Commands kurz zurückhalten. Sie mischen diese Commands aber nicht in
  den falschen Recording-Abschnitt.
- Ein Fehler des Recorders verfälscht kein bereits bestimmtes Command-Outcome.
- Recordings können vertrauliche Inhalte enthalten und dürfen nicht ungeprüft weitergegeben werden.

## Prüfung

- Format-Fixtures prüfen Header, alle fünf Outcome-Statuswerte, Footer, unbekannte Felder und eine
  fehlende Abschlusszeile.
- Nebenläufigkeitstests lassen Responses in anderer Reihenfolge eintreffen und prüfen die
  Annahmereihenfolge in der Datei.
- Start- und Stop-Tests prüfen alle vier Zustände, beide Barrieren, offene frühere Commands und die
  sechs stabilen Ablehnungscodes.
- Pfadtests prüfen absolute und leere Pfade, `.`, `..`, leere Komponenten, Backslashes, falsche
  Endungen, vorhandene Dateien und Symlinks in jeder Position.
- I/O-Fixtures prüfen Fehler beim Header, während eines aktiven Recordings sowie beim Footer,
  Synchronisieren und Schließen.
- Ein unerwartetes Session-Ende prüft `unanswered`, `session_ended` und `RecordingFailed` vor
  `Ended`. Ein Drop-Test erwartet dagegen keine gültige Abschlusszeile.
