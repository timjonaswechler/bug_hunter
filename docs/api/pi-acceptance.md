# Externe Agent-Abnahme mit pi

Dieser Ablauf prüft die menschlich gestartete Agent-Laufzeit gegen die echte CLI.
Der erste Lauf und die Anschlussprüfung sind durchgeführt. Modellzugang und
Werkzeugschleife bleiben vollständig bei pi.

## Nachweis des durchgeführten Laufs

- Server: `127.0.0.1:54017`; Session: `9b94a1e2fbfa4c1dea5deeefe9f1ffbd`.
- Lokale Rohdaten: `target/pi-client.mxGkix`. Diese generierten Dateien sind nicht
  Bestandteil der versionierten Dokumentation.
- Die gespeicherten JSON-Antworten wurden vom Delta-Agenten gegengeprüft:
  Tickwerte `0 → 13 → 20`, erfolgreiches Script mit Indizes 0 bis 3 und genau
  einem aufgenommenen Command. Alle geprüften Poll-Antworten waren `activity`,
  ohne `gap`, `rejected` oder `failed`.
- Auch die tatsächliche Datei unter
  `target/pi-server.FM1w5Y/9b94a1e2fbfa4c1dea5deeefe9f1ffbd/recordings/pi.jsonl`
  wurde geprüft: Header, ein Warp mit 7 Ticks und Footer mit einem Command.
- pi ließ Warp-Request 8 absichtlich offen. Seine letzten Inspects meldeten
  79 und 198 Ticks bei Prozess-ID 4644.
- Der Nutzer bestätigte ausdrücklich, dass pi bereits geschlossen war, als er
  die REPL öffnete. Diese zeigte `Ready` und weiterhin Warp 8 als Pending.
  Inspect-Requests 11 und 12 meldeten 1621 und 1748 Ticks bei derselben Prozess-ID.
  Das Agent-Ende löste somit weder einen versteckten Stop noch Shutdown aus.
- Die menschliche Bedienung erfolgte nach Ende von pi, nicht gleichzeitig mit
  dem offenen pi-Prozess. Gleichzeitige Clients sind durch die separate
  automatisierte REPL-Abnahme abgedeckt; dieser manuelle Lauf behauptet dafür
  keinen zusätzlichen Nachweis.

Der erste pi-Aufruf verwendete das auf macOS nicht vorhandene Shell-Programm
`timeout`. Dabei wurde woodpecker nicht gestartet. Anschließend verwendete pi
die Zeitbegrenzung seines Bash-Werkzeugs. Es wurde keine ungewisse Einreichung wiederholt.

Die funktionale Agent-Abnahme ist abgeschlossen. Das ausdrückliche Aufräumen
des Testservers und der Session wurde zum Zeitpunkt dieser Dokumentation noch
nicht bestätigt; dafür gilt Schritt 4 unten.

## Voraussetzungen

Im Root des aktuellen woodpecker-Arbeitsbaums:

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin counter
```

pi muss installiert sein und einen vom Nutzer eingerichteten Modellzugang besitzen.
Die Startoptionen unten wurden mit `pi 0.85.1 --help` abgeglichen.
Keine Zugangsdaten kopieren oder in Projektdateien speichern.

## 1. Eigenen Testserver starten

In Terminal 1, weiterhin im selben Repository-Root:

```sh
ARTIFACTS="$(mktemp -d "$PWD/target/pi-server.XXXXXX")"
target/debug/woodpecker --address 127.0.0.1:0 \
  server start --artifact-dir "$ARTIFACTS"
```

Der Server bleibt im Vordergrund. Seine erste JSON-Zeile nennt die tatsächlich
gebundene Adresse im Feld `address`. Übernimm diese Adresse in Terminal 2.
Port 0 vermeidet Konflikte mit anderen laufenden Servern.

## 2. pi selbst starten

In Terminal 2, im selben Repository-Root:

```sh
export WOODPECKER_ADDRESS='127.0.0.1:PORT_AUS_DER_SERVERAUSGABE'
export WOODPECKER_ACCEPTANCE_DIR="$(mktemp -d "$PWD/target/pi-client.XXXXXX")"
printf 'Ergebnisordner: %s\n' "$WOODPECKER_ACCEPTANCE_DIR"

pi --tools bash,read \
  --no-extensions --no-skills --no-prompt-templates --no-context-files \
  --session-dir "$WOODPECKER_ACCEPTANCE_DIR/pi-session" \
  @docs/api/pi-task.md
```

Ersetze den Port vor dem Aufruf. Es wird kein Modell gewählt oder umkonfiguriert;
pi verwendet den bereits eingerichteten Zugang. Du startest damit einen echten
Modelllauf beim konfigurierten Anbieter. Übergeben werden der Arbeitsauftrag und
die von pi gelesenen Werkzeugergebnisse.

Die Optionen deaktivieren automatisch geladene Erweiterungen, Skills, Prompt-Vorlagen
und Projekt-Kontextdateien. `bash` bleibt ein vollwertiger Shell-Zugriff, keine Sandbox.
Der Auftrag beschränkt Änderungen auf den Ergebnisordner und die eigens erzeugte
Test-Session. Quellcodeänderungen, weitere Agenten und GitHub-Veröffentlichungen sind
nicht Bestandteil der Abnahme.

pi soll eine Session erstellen, Inspect und Warp prüfen, ein Recording-Script
ausführen und schließlich einen langen Warp laufen lassen. Es schreibt Fakten und
offene Nachweise in `result.md` und gibt einen REPL-Aufruf aus.

## 3. Parallele REPL und Ende von pi prüfen

Lasse pi zunächst geöffnet. Öffne mit seinem ausgegebenen Befehl eine REPL in
Terminal 3. Führe aus:

```text
pending
inspect resource counter::Counter
```

`pending` muss den langen Warp aus dem pi-Auftrag zeigen. Der Inspect muss während
dieses Warps ein Ergebnis liefern. Halte Request-ID und Tickwert fest.

Beende jetzt ausschließlich pi über dessen normale Beenden-Funktion. Schließe weder
den woodpecker-Server noch die REPL. Wiederhole in der REPL:

```text
pending
inspect resource counter::Counter
```

Der Warp muss weiterhin offen sein und der Tickwert weiter steigen. Damit ist
nachgewiesen, dass das Agent-Ende keinen versteckten Stop oder Shutdown auslöst.
Melde den Ergebnisordner und diesen Befund im Delta-Thread. Der Delta-Agent kann
die gespeicherten CLI-Ergebnisse gegenprüfen; bis dahin bleibt der Abnahmepunkt offen.

## 4. Ausdrücklich aufräumen

Nur in der eigens für diese Abnahme angelegten Session:

```text
tick warp stop
```

Warte auf das Stop-Ergebnis mit `was_running: true` und den Abschluss des langen
Warps mit `outcome: "stopped"`. Danach:

```text
shutdown
```

Warte auf den regulären Session-Abschluss. Stoppe anschließend den ausschließlich
für diesen Test gestarteten Server aus Terminal 2:

```sh
target/debug/woodpecker --address "$WOODPECKER_ADDRESS" server stop
```

Terminal 1 muss regulär enden. Bewahre den Ergebnisordner und die lokalen
Recording-Artefakte für die Gegenprüfung auf. Es werden keine echten GitHub-Issues
angelegt; die verwendete Zähler-Konfiguration wählt `Local`.
