# Arbeitsauftrag für pi: woodpecker-Abnahme

Du prüfst woodpecker als externe Agent-Laufzeit über seine maschinenlesbare CLI.
Du implementierst nichts. Verändere keinen Quellcode, keine Konfiguration und keine
Git-Historie. Starte keine weiteren Agenten, keine GitHub-Veröffentlichung und keinen
zusätzlichen Server.

## Umgebung und Grenzen

- Arbeite im aktuellen Repository-Root.
- Das Binary ist `target/debug/woodpecker`.
- Der Nutzer hat einen eigenen Testserver gestartet. Seine Adresse steht in der
  exportierten Variable `WOODPECKER_ADDRESS`.
- Dein Ausgabeordner steht in `WOODPECKER_ACCEPTANCE_DIR`. Schreibe eigene Hilfsdateien
  nur dort. Das ausdrücklich angeforderte Recording schreibt der Server in seinen
  eigenen Artefakt-Root.
- Verwende `tests/fixtures/counter.toml` zum Erstellen genau einer neuen Session.
  Der konfigurierte Report-Provider ist Local.
- Steuere ausschließlich deine neu erstellte Session. Stoppe keine fremde Session
  und nicht den Server.
- Lies keine Credentials, Umgebungsvariablen mit Schlüsseln oder andere
  Projektquelltexte. Du benötigst nur diesen Auftrag, CLI-Hilfe und CLI-Ergebnisse.
  Gib ausschließlich die beiden oben genannten Umgebungsvariablen aus, nie die
  gesamte Umgebung.
- Jeder CLI-Aufruf läuft separat. Warte begrenzt auf Ergebnisse, maximal 60 Sekunden
  pro Schritt. Bei Fehler oder Zeitüberschreitung dokumentiere den Befund und frage
  den Nutzer, statt weiterzuraten oder einen ungewissen Submit zu wiederholen.
- Wiederhole niemals eine Einreichung, deren Bestätigung verloren ging.
- `session inspect` liest den Lebenszyklus, nicht den Spielzustand.
- `session submit` bestätigt nur Annahme. Das terminale Command-Ergebnis steht in
  Activity und muss über seine Request-ID zugeordnet werden.

## CLI-Vertrag

Alle folgenden Aufrufe beginnen mit:

```sh
target/debug/woodpecker --address "$WOODPECKER_ADDRESS"
```

Operationen:

```text
session create --config tests/fixtures/counter.toml
session inspect <id>
session submit <id> --command '<command-json>'
session poll <id> --wait-ms 1000
session poll <id> --cursor '<cursor-json>' --wait-ms 1000
session script <id> --file <datei>
```

Poll liefert `kind: "activity"`, `entries` und einen Fortsetzungscursor. Ein Event
steht unter `entries[i].event`. Command-Events tragen `request_id` und `command`.
Erfolg ist `kind: "completed"` mit `output`; `rejected` und `failed` sind Fehler.
Ein `gap` bedeutet verlorene Ergebnisse. Behaupte dann keinen erfolgreichen Nachweis.
Die Poll-`wait-ms` sind nur eine begrenzte Wartezeit, kein Command-Timeout.

Ein Inspect des Zählers verwendet dieses Command-Objekt:

```json
{"command":"inspect.query","arguments":{"source":"resources","selector":{"kind":"type","type_path":"counter::Counter"},"projection":{"kind":"value"}}}
```

Der erfolgreiche Output enthält `items[0].result.value.value` mit `ticks`
und `process_id`. Prüfe zuerst, ob das Item tatsächlich lesbar ist.

## Ablauf

1. **Erstellen und inspizieren.**
   Prüfe, dass beide benötigten Umgebungsvariablen gesetzt sind. Erstelle genau eine
   Session, speichere die vollständige ID in `$WOODPECKER_ACCEPTANCE_DIR/session-id`
   und warte auf `Ready`. Inspiziere den Zähler über den Spiel-Command. Speichere
   den anfänglichen Tickwert und die Prozess-ID.

2. **Einzelne Commands auswerten.**
   Reiche `tick.warp.start` mit `{"ticks":13}` ein. Warte über Poll auf genau dessen
   terminales Ergebnis. Prüfe `requested_ticks: 13`, `executed_ticks: 13` und
   `outcome: "completed"`. Inspiziere erst danach erneut. Der Zähler muss um genau
   13 gestiegen sein. Inspect selbst darf keine zusätzlichen Ticks erzeugen.

3. **Script mit echten Recording-Barrieren ausführen.**
   Schreibe folgende Datei in deinen Ausgabeordner:

   ```json
   {
     "version": 1,
     "commands": [
       {"command":"recording.start","arguments":{"path":"recordings/pi.jsonl"}},
       {"command":"tick.warp.start","arguments":{"ticks":7}},
       {"command":"recording.stop","arguments":{}},
       {"command":"inspect.query","arguments":{"source":"resources","selector":{"kind":"type","type_path":"counter::Counter"},"projection":{"kind":"value"}}}
     ]
   }
   ```

   Führe sie mit `session script` aus. Prüfe Exit-Code 0, `kind: "passed"` und
   vier Ergebnisse mit `command_index` 0 bis 3. Recording-Stop muss genau einen
   aufgenommenen Command melden. Der abschließende Inspect muss insgesamt
   Anfangswert + 20 Ticks ergeben. Speichere die vollständige JSON-Zusammenfassung.

4. **Parallele menschliche Bedienung vorbereiten.**
   Starte ausdrücklich einen langen Warp:

   ```json
   {"command":"tick.warp.start","arguments":{"ticks":100000,"pace":{"kind":"ticks_per_second","target":5}}}
   ```

   Speichere seine bestätigte Request-ID. Warte nicht auf seinen vollständigen
   Abschluss. Prüfe mit zwei Inspects im Abstand von etwa einer Sekunde, dass der
   Tickwert steigt. Bewahre beide Ergebnisse auf.

5. **An den Nutzer übergeben.**
   Schreibe `$WOODPECKER_ACCEPTANCE_DIR/result.md` mit Serveradresse, Session-ID,
   Prozess-ID, langem Warp-Request, den geprüften Ergebnissen und etwaigen Fehlern.
   Trenne beobachtete Fakten von noch offenen Nachweisen. Halte ausdrücklich fest:
   Die parallele REPL-Bedienung und das Weiterlaufen nach Ende von pi sind noch
   nicht bestätigt.

   Antworte mit der Session-ID, dem Ergebnisordner und dem kopierbaren REPL-Aufruf.
   Sage dem Nutzer, dass der Warp absichtlich weiterläuft. Er soll zunächst parallel
   die REPL öffnen und dort Pending und Inspect prüfen, dann pi beenden.
   Danach übernimmt der Nutzer beziehungsweise der betreuende Delta-Agent
   die noch offenen Prüfungen und das ausdrückliche Aufräumen.

**Hier endet dein Auftrag.** Sende weder Warp-Stop noch Shutdown. Das absichtliche
Weiterlaufen ist Teil der Abnahme, kein Anlass für automatische Bereinigung.
