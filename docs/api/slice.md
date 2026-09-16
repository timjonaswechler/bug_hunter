# Experimenteller Durchstich

Dieser Build ist keine vollständige v3-Implementation. Er implementiert Warp,
Resource-Inspect und Shutdown. Input, Entity-Inspect, Screenshot, Recording, Replay
und Reporting folgen in weiteren Durchstichen. Der einzige unterstützte Weg verwendet
`session`; die v2-Implementation und ihre öffentlichen Einstiegspunkte sind entfernt.

## Paketierung

Die direkte Session und das Plugin brauchen kein Feature. `server` aktiviert HTTP
und WebSocket, `client` den synchronen Netzwerkclient, `cli` das Binary `woodpecker`.
Die Prozessverwaltung dieses Durchstichs unterstützt Unix-Prozessgruppen.
Das Testpackage verwendet für den Zähler das Feature `slice`. Die übrigen Bevy-Anwendungen
laufen mit nativer Eingabe; ihr altes Feature `automation` wurde entfernt.
Die Bibliothek benötigt keinen Renderer. Render-/UI-Abhängigkeiten der Testanwendungen
gehören weiterhin zu deren eigenem Manifest.

Die Lockfiles halten die geprüfte Auflösung auf Bevy 0.19.1 fest.
Das Testpackage hat weiterhin seine eigene Cargo-Auflösung.

## Ausführen

Vom Repository-Root aus bauen:

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin counter
```

Es sind keine Zugangsdaten oder vorbereiteten Umgebungsvariablen nötig.
Der Server vertraut lokalen Clients und bindet ausschließlich an Loopback.
Remote-Betrieb ist nicht unterstützt.

Terminal 1:

```sh
target/debug/woodpecker --address 127.0.0.1:4100 \
  server start --artifact-dir target/slice-artifacts
```

Terminal 2:

```sh
target/debug/woodpecker --address 127.0.0.1:4100 session ls
target/debug/woodpecker --address 127.0.0.1:4100 \
  session create --config tests/fixtures/counter.toml
```

Die vollständige zurückgegebene ID in `ID` übernehmen. Detail zeigt den
Startfortschritt. Erst bei `Ready` Spiel-Commands einreichen:

```sh
target/debug/woodpecker --address 127.0.0.1:4100 session inspect "$ID"
target/debug/woodpecker --address 127.0.0.1:4100 \
  session submit "$ID" --command '{"command":"tick.warp.start","arguments":{"ticks":13}}'
target/debug/woodpecker --address 127.0.0.1:4100 \
  session poll "$ID" --wait-ms 1000
```

`submit` bestätigt ausschließlich Pending. Das korrelierte `completed` steht in
Activity. Für weitere Polls den zurückgegebenen Cursor als JSON mit `--cursor`
übergeben. Erst nach Warp-Abschluss den Zähler lesen:

```sh
target/debug/woodpecker --address 127.0.0.1:4100 \
  session submit "$ID" --command '{"command":"inspect.query","arguments":{"source":"resources","selector":{"kind":"type","type_path":"counter::Counter"},"projection":{"kind":"value"}}}'
target/debug/woodpecker --address 127.0.0.1:4100 session poll "$ID"
target/debug/woodpecker --address 127.0.0.1:4100 session stop "$ID"
target/debug/woodpecker --address 127.0.0.1:4100 server stop
```

Ein einzelner CLI-Aufruf trennt beim Ende nur die Verbindung. Server und
angenommene Commands laufen weiter. `session stop` beendet nur die ausgewählte
Session. Den Gesamtstatus des Serverendes liefert der Vordergrundprozess in
Terminal 1; der Verwaltungsaufruf bestätigt zunächst nur den Stoppauftrag.

Relative Manifestpfade beziehen sich auf das Arbeitsverzeichnis des Servers.
Die Config-Datei verschiebt diesen Bezug nicht.

## Äußerer Codec, Version 1

Die experimentellen Routen liegen unter `/v1` und sind für lokale Clients ohne
Authentisierung erreichbar. Browser-Origin-Header werden mit `403` abgelehnt.

| Methode | Route | Body |
| --- | --- | --- |
| GET | `/v1/sessions` | keiner |
| POST | `/v1/sessions` | Launch-, Tick-, Report-Konfiguration |
| GET | `/v1/sessions/{id}` | keiner |
| POST | `/v1/sessions/{id}/stop` | keiner |
| POST | `/v1/stop` | keiner |
| GET, Upgrade | `/v1/sessions/{id}/connect` | keiner |

HTTP-Erfolg ist JSON mit `version: 1` und `data`. Fehler enthalten stattdessen
`error: {code, message}`. WebSocket-Textnachrichten tragen `version`, eine vom
Client vergebene `call`-Nummer und `operation`. Antworten verwenden dieselbe
`call`-Nummer. Diese Nummer ist keine Session-Request-ID.

Kopierbare Codec-Fixtures:

```json
{"version":1,"call":1,"operation":{"kind":"submit","command":{"command":"tick.warp.start","arguments":{"ticks":3}}}}
{"version":1,"call":1,"result":{"kind":"pending","request_id":1,"command":"tick.warp.start"}}
{"version":1,"call":2,"operation":{"kind":"poll","cursor":null,"wait_ms":0}}
```

Poll liefert `entries` und `cursor`. Ein Cursor ist `{session_id, position}`;
Position 0 liegt vor dem ersten Eintrag. `null` fordert den Anfang an, nicht
automatisch den ältesten noch vorhandenen Eintrag. Bei Verlust folgt `gap` mit
dem ältesten verfügbaren Einstieg als `cursor`. Der Client muss die Lücke
ausdrücklich behandeln. `wait_ms` ist auf 30.000 begrenzt.

Activity enthält Lifecycle-Wechsel, `pending`, `completed`, `rejected`, `failed`
und Session-Events. Outputs werden nicht gekürzt. Die Standardgrenze ist 4 MiB
pro Session, konfigurierbar beim Serverstart. Ein zu großer Eintrag verdrängt
die vorherigen Einträge und hinterlässt eine sichtbare Cursor-Lücke.

Zeitstempel sind Unix-Millisekunden. Listen sind nach Zeitstempel, dann voller ID
sortiert. Die CLI gibt ausschließlich JSON-Ergebnisse auf stdout aus.

## Nachweise

```sh
cargo test --features cli --lib --test session
python3 tests/slice.py
python3 tests/shutdown.py
cargo check --no-default-features --lib
cargo check --no-default-features --features server --lib
cargo check --no-default-features --features client --lib
cargo check --all-features --all-targets
cargo test --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets
```

`slice.py` startet echte CLI- und Bevy-Prozesse. Es prüft leeres Verzeichnis,
frühe Erstellungsantwort, Zählerstillstand ohne Warp, exakte Tickzahl, Trennung
mit weiterlaufendem Warp, Pace/Stop über neue Verbindungen, getrennte
Request-ID-Räume, Einzel-Stopp, Starting-Abbruch und Fehler unter angenommener ID.

`shutdown.py` prüft zwölf echte Spielprozesse in vier Serverläufen. Reguläres
HTTP-Ende, Abbruch nach bereits erfolgtem Prozessstart, SIGTERM mit gemeinsamer
Frist und zweites Ctrl+C müssen alle Prozessgruppen bereinigen.

Die Rust-Prozessfixtures prüfen zusätzlich ungeordnete Antworten, bekannte und
unbekannte IDs, volle stderr-Pipe, Pending-Drop, History-Verdrängung, falsche
Protokollversion, Event-Überlauf und einen Kindprozess, dessen Nachfolger die
Pipes nach dem direkten Prozessende offen hält.

## Noch nicht enthalten

Die oben genannten Folge-Durchstiche bleiben offen. Das gilt auch für
Panic-/Tracing-Reports und deren Snapshot-Metadaten. `report` enthält bisher
nur die validierte Konfiguration; aktiviertes Tracing wird abgelehnt.
Spiel-stderr wird fortlaufend gelesen, aber noch nicht als laufender
Diagnosestrom dargestellt. Startfehler enthalten einen begrenzten stderr-Auszug.

Die Activity-Grenze von 4 MiB ist ein vorläufiger Betriebswert, keine
Vollständigkeitsgarantie. Große Inspect-Werte können sofort eine Lücke
erzeugen. Die Bemessung mit echten Reports folgt mit deren Implementation.
Der Server meldet bei Fristablauf Fehler und wartet auf die Prozessbereinigung;
er behauptet keine harte Zeitgarantie für nicht unterbrechbare Betriebssystemarbeit.

Auf dem verwendeten macOS-Rechner hat Gatekeeper das generierte `process_fixture`
bei wiederholten Starts vereinzelt verzögert oder mit SIGKILL beendet.
Das wurde auch mit direkten `cargo run`-Aufrufen außerhalb des Servers reproduziert;
das Systemlog nennt `Gatekeeper rejection`. Solche Läufe können in `Starting`
verbleiben oder als `Failed` enden. Die Tests enthalten keine automatische
Wiederholung, und die Implementation verändert keine Sicherheitsregeln des Systems.
