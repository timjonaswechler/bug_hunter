# Experimenteller Durchstich

Dieses Dokument beschreibt den vorhandenen ausführbaren Stand, nicht den bereits
umgesetzten Headless-Zielvertrag. Gerenderte Szenen, Capture und Teile der
Eingabeadapter benötigen derzeit ein primäres Fenster. Headless-Rendering,
fensterlose UI-/Picking-Zuordnung und relative Blicksteuerung sind noch offen.
Der nächste Umbau steht im [Headless-Plan](implementation-plan.md#headless-durchstich).

Dieser Build ist keine vollständige v3-Implementation. Er implementiert Warp,
Resource-/Entity-Inspect, Pointer-/Keyboard-/Text-Input, Screenshot, Recording, Replay und Shutdown.
Fehlerbeobachtung, Report-Snapshots, Report-Darstellung sowie Local und GitHub sind vorhanden.
Der Server verarbeitet Reports automatisch und unabhängig von Clients. Der einzige unterstützte Weg verwendet
`session`; die v2-Implementation und ihre öffentlichen Einstiegspunkte sind entfernt.

Entity-Inspect unterstützt Handle-Abfragen, Componentfilter, Summary, Component-Namen,
Component-Werte und Hierarchien. Adapter-, Codec- und Plugin-Tests prüfen diese Varianten
einschließlich unverändertem Anwendungszustand und unveränderter Zeit zwischen Warps.
Der reale Context-Menu-Ablauf über Server, Pointer-Eingabe und Warp ist in
`tests/ui.py` geprüft. Er findet den Button per allgemeinem Inspect und liest seine
Layoutposition, statt Koordinaten für den Klick festzuschreiben.
Pointer-Erfolg bestätigt nur die Vormerkung. Bevy erhält die Input-Messages erst
beim nächsten Tick. Der Kontrolllauf entleert zwischen Ticks den Render-Zeitkanal,
ohne die Simulationszeit fortzuschreiben.

Der Pointer ist virtuell und sessionlokal. Commands bewegen den Betriebssystem-Cursor
nicht und funktionieren ohne nativen Fensterfokus. Native Maus-, Touch-, Keyboard-
und IME-Eingaben werden verworfen, Resize-/Close-Ereignisse bleiben erhalten.
`tests/ui.py` steuert zwei gleichzeitig laufende Anwendungen mit getrennten Buttonzuständen.
Keyboard-Commands halten Tasten pro Session bis zum ausdrücklichen Release.
Die Abnahme prüft `a` in beiden Sessions vor und nach Ticks sowie über mehrere Ticks.
Text-Commands verwenden den beim Annehmen geprüften Bevy-Fokus. Die Abnahme fokussiert
zwei Textfelder per virtuellem Pointer und prüft getrennte Unicode-Eingaben.

`input.text.input` nimmt `{"text":"Grüße 🦜"}` mit höchstens 16.384 UTF-8-Bytes an.
Erfolg liefert `null`; das Textfeld bleibt bis zum Tick unverändert. Leerer Text ist
zulässig. Voraussetzung sind ein eindeutiges primäres Fenster und ein lebender
Bevy-Fokus auf einer `EditableText`-Entity. Die Anwendung benötigt `woodpecker/ui`.
Ohne diese Unterstützung wird die Eingabe mit `text_focus_unavailable` abgelehnt,
bei fehlendem Fenster mit `text_window_unavailable`. Übergröße ergibt `text_too_large`.
Der Adapter stellt den Edit erst beim Tick dem geprüften Feld zu. Er verändert
weder die Betriebssystem-Zwischenablage noch sendet er native IME-Ereignisse.

Keyboard-Commands verwenden `{"key":"a"}` mit `input.keyboard.press` oder
`input.keyboard.release`. Erfolg liefert `null` und bedeutet nur Vormerkung.
Unterstützt sind Buchstaben `a` bis `z`, Ziffern `digit_0` bis `digit_9`, Satzzeichen,
Navigation, linke/rechte Modifikatortasten und `f1` bis `f12`.
Die vollständigen Tokens und festen Bevy-Zuordnungen stehen in
[keyboard.rs](../../src/command/input/keyboard.rs).
Die Zuordnung hängt nicht vom Betriebssystemlayout ab; Shift verändert den
festen logischen Buchstaben nicht. Text kommt ausschließlich über den
Text-Command, nicht über Keyboard-Events. Es gibt keinen automatischen Key-Repeat.
Bei beiden gehaltenen Shift-Tasten bleibt logisch Shift gedrückt, bis beide losgelassen sind.

Die gerenderte Abnahme benötigt eine Desktop-Sitzung:

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin context_menu
python3 tests/ui.py
```

## Screenshot

Die Spielanwendung aktiviert `woodpecker/screenshot` und installiert ihren Renderer
selbst. Das Session-Plugin meldet die Capability erst, wenn RenderApp, RenderDevice,
Bevys Screenshot-Kanal und das Artefaktverzeichnis verfügbar sind. Server und CLI
benötigen dieses Feature nicht.

```sh
target/debug/woodpecker --address 127.0.0.1:4100 session submit "$ID" \
  --command '{"command":"screenshot.capture","arguments":{"path":"screenshots/current.png"}}'
```

Das korrelierte Ergebnis kommt nach GPU-Readback, PNG-Encoding und erfolgreichem
Schreiben, beispielsweise:

```json
{"path":"screenshots/current.png","width":640,"height":360,"overwritten":false}
```

Der relative Pfad gehört zum `artifact_dir` aus `session inspect`. Elternverzeichnisse
werden angelegt. Vorhandene Dateien werden erst nach vollständigem Schreiben über
eine temporäre Datei atomar ersetzt. Alle Dateioperationen verwenden eine
`cap_std::fs::Dir`-Capability; Symlinks können nicht aus ihr ausbrechen.
Relative Symlinks innerhalb des Roots sind möglich, absolute Symlinks werden abgelehnt.
Der Adapter prüft das Ziel vor Annahme und erneut beim Schreiben.

Aufnahmen derselben Session laufen nacheinander, da Bevy konkurrierende Aufnahmen
des gleichen Renderziels verwirft. Der Kontrolllauf nimmt weiter Commands an.
PNG-Encoding und Datei-I/O laufen auf einem Worker-Thread. Ein fehlender
GPU-Readback wird nach 30 realen Sekunden mit `screenshot_failed` abgeschlossen;
die Anwendung wird dafür nicht getickt. Verspätete Readbacks werden verworfen.
Die Frist gilt nicht für bereits laufendes Schreiben.

Für jeden Request prüft der Adapter im tatsächlichen Capture-Frame nach der
Render-Vorbereitung, ob der Fenster-View und sein Format verfügbar sind.
Bevy 0.19.1 überspringt ohne diese Ressourcen die Bildkopie, liefert aber trotzdem
einen nullinitialisierten Readback. Der Adapter lehnt diesen Fall mit
`screenshot_window_unavailable` ab, ohne eine PNG-Datei zu schreiben oder eine
vorhandene Datei zu ersetzen. Der Befund bleibt an diesen Request/Frame gebunden:
Eine später verfügbare Surface kann den alten Readback nicht nachträglich freigeben.
Fehlt die Frame-Verifikation selbst, lautet der Fehler `screenshot_failed`.

Diese Absicherung erzeugt keine zusätzlichen Ticks, verändert keinen Fensterfokus
und wiederholt den Request nicht. Sie macht die Aufnahme eines vollständig
verdeckten Fensters auf Metal noch nicht möglich. Eine tatsächlich schwarze Szene
bleibt ein gültiges Bild; Pixelwerte werden nicht zur Verfügbarkeitsprüfung benutzt.

`tests/ui.py` prüft vollständige PNGs, sichtbare Menüänderung nach einem ausdrücklichen
Tick, Überschreiben, parallele Requests und Session-Isolation. Tickzähler,
simulierte Zeit und Anwendungszustand bleiben während Capture unverändert.
Mit `--capture-dir target/ui-captures` bleiben die Bilder nach der Abnahme erhalten.

## Recording

Recording gehört zur direkten Session und benötigt kein zusätzliches Feature.
Der Koordinator führt diese Commands selbst aus; sie gehen nicht an das Spiel:

```sh
target/debug/woodpecker --address 127.0.0.1:4100 session submit "$ID" \
  --command '{"command":"recording.start","arguments":{"path":"recordings/run.jsonl"}}'
# Nach den gewünschten Spiel-Commands und deren Abschlüssen:
target/debug/woodpecker --address 127.0.0.1:4100 session submit "$ID" \
  --command '{"command":"recording.stop","arguments":{}}'
```

Start liefert nach geschriebenem Header `{"path":"recordings/run.jsonl"}`.
Stop liefert nach Footer, Flush, Synchronisierung und Dateischluss
`{"path":"recordings/run.jsonl","recorded_commands":42}`.
Beide Operationen benötigen zuvor abgeschlossene Commands. Während ihrer
Dateiarbeit werden spätere Commands angenommen, aber noch nicht ausgeführt.
Die Arbeit bleibt auch nach Client-Trennung oder verworfenem Pending bestehen.

Der relative `.jsonl`-Pfad darf keine Symlinks enthalten, auch keine In-Root-Links.
Jede Verzeichniskomponente wird ohne Symlink-Folgen geöffnet, die Datei mit
`create_new` angelegt. Vorhandene Dateien werden nicht überschrieben.
Diese Regeln sind strenger als die Screenshot-Pfadregeln.

Die Version-1-Datei enthält Header, Spiel-Commands mit Outcomes in Annahmereihenfolge
und Footer. Recording-/Replay-Steuerung, Shutdown und Events werden nicht aufgenommen.
Interne Replay-Plan-Commands werden dagegen wie andere Spiel-Commands aufgezeichnet.
Dateien enthalten ungekürzte Argumente und Inspect-Ergebnisse und können vertraulich
sein. Aufnahmen erzeugen selbst keine Ticks.

Bei einem aktiven Schreibfehler bleibt die Datei unvollständig. Nur Recording endet;
die Session meldet `RecordingFailed`, ohne das Spiel-Command-Ergebnis zu ändern.
Start-/Stop-Dateifehler liefern `Io`. Bei unerwartetem Prozessende versucht der
Dateiworker noch offene Commands als `unanswered` und einen `session_ended`-Footer
zu schreiben. Ein erzwungener Abbruch wartet nicht unbegrenzt auf blockierende Datei-I/O.
Reguläres Shutdown verlangt zuerst den Abschluss offener Commands und Recording-Stop.
`session stop` und das Serverende stellen diese Vorbedingungen selbst durch
ausdrücklichen Replay-/Warp-/Recording-Stop her. Der Verwaltungs-Stopp wartet dabei auf
den Recording-Footer; ein Fehler dieses Abschlusses ergibt `Failed`.

## Replay

```sh
target/debug/woodpecker --address 127.0.0.1:4100 session submit "$ID" \
  --command '{"command":"replay.start","arguments":{"path":"recordings/run.jsonl"}}'
target/debug/woodpecker --address 127.0.0.1:4100 session submit "$ID" \
  --command '{"command":"replay.stop","arguments":{}}'
```

Start bleibt bis zum Ende des Plans pending. Erfolg liefert
`{"outcome":{"kind":"completed"}}`, kontrollierter Stop
`{"outcome":{"kind":"stopped"}}`. Technische Blockierungen liefern
`{"outcome":{"kind":"blocked","code":"command_protocol_failed","message":"..."}}`.
Stop antwortet erst nach dem Auslaufen bereits gesendeter Commands mit
`{"was_running":true}`. Ohne laufendes Replay liefert er `false`.

Der private Loader öffnet den normalisierten `.jsonl`-Pfad ohne Symlinks und
validiert die gesamte Datei vor dem ersten Spiel-Command. Dazu gehören Header,
Version, doppelte und unbekannte Felder, Argumente, konkrete Output-Typen und Footer.
Ladefehler enthalten Pfad und soweit bekannt Zeile. Sie gehen an das Start-Pending,
nicht in den Session-Eventstrom. Der vollständige Vertrag steht in
[target.md](target.md#recording-dateivertrag).

Replay verwendet den aktuellen Spielzustand. Alte Inspect-Werte, Ablehnungen oder
Screenshot-Overwrite-Flags sind keine Erwartungen an den neuen Lauf.
Erfolgreiche aufgezeichnete Warps werden auf `executed_ticks` verkürzt;
Null-Tick-Warps und zugehörige Stops entfallen. Benachbarte Warps werden derzeit
nicht zusammengefasst. Vor jedem Warp wartet Replay auf frühere Outcomes und
nach jedem Warp auf dessen Abschluss.

Während Vorbereitung, Ausführung und Stop ist von außen nur `replay.stop`
zulässig. Weitere Starts ergeben `replay_already_running`, andere Commands
`replay_in_progress`. Direktes Shutdown liefert bei offenem Replay
`shutdown_commands_pending`, ohne ID oder History-Eintrag.
Stop während des Ladens wartet auf die Bestätigung der Ladeaufgabe.
Eine vorher angenommene Recording-Dateibarriere hält auch den Loader zurück.

`tests/slice.py` prüft eine echte Zähleraufnahme, vollständige Vorabvalidierung
ohne Tick sowie Verwaltungs-Stopp bei gleichzeitigem Replay und Recording.
`tests/ui.py` enthält zusätzlich die Wiederholung der vollständigen UI-Aufnahme.
Der aktuelle Nachweis und offene Render-Befund stehen in
[next-steps.md](next-steps.md#nachweise-und-bekannte-grenzen).

## Fehlerbeobachtung und Snapshots

Das Session-Plugin verkettet den vorhandenen Panic-Hook. Jeder beobachtete Panic
liefert ein `session::Event::Failure`, auch bei `catch_unwind` oder auf einem
Worker-Thread. Payload, Location, Backtrace-Status und rohe Backtrace-Ausgabe bleiben
getrennt. Die Anwendung darf den Hook danach nicht ersetzen.
Gewöhnlicher stderr-Text, selbst mit dem Wort `ERROR`, ist kein Fehlerauslöser.

Tracing-Errors sind standardmäßig aus. Zur Aktivierung setzt die Startkonfiguration
`report.tracing_errors = true`, und die Anwendung registriert den Layer:

```rust
app.add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
    custom_layer: woodpecker::session::tracing_error_layer,
    ..Default::default()
}));
// Weitere Anwendungsplugins und Systeme, danach:
app.add_plugins(woodpecker::session::Plugin);
```

Eigene Custom-Layer müssen ausdrücklich mit diesem Layer zusammengesetzt werden.
Die Integration ändert keine Filter; nur tatsächlich dispatchte Error-Events werden
beobachtet. Der Start wartet bei aktivierter Beobachtung auf die positive
Layer-Bestätigung. Ein nicht installierter Layer ergibt einen Launch-Fehler.
Die gerenderte `slice`-Komposition der Testanwendungen registriert den Callback bereits.

Marker verwenden versionierte JSON-Chunks mit Event-ID, Index, Anzahl,
Base64-Payload und SHA-256-Prüfsumme. Jeder Schreibaufruf ist höchstens 512 Bytes lang.
Nur vollständig validierte Marker werden aus den menschlichen Diagnosebytes entfernt.
Beschädigte oder bei EOF unvollständige Marker bleiben sichtbar und liefern
`ObservationError { code: "invalid_report_marker", .. }`.
Ein unerwartetes Prozessende liefert nach dem Pipe-Drain einen `ProcessExit`-Failure,
sofern kein Panic erkannt wurde. Absichtliches Shutdown und Drop erzeugen keinen solchen
Failure. Fehler vor Ready bleiben Startdiagnose.

Direkte Nutzer können nach einem Failure-Event `report::Report::create(failure, &session)`
aufrufen. Der Snapshot kopiert Failure, Startmetadaten und die aktuelle korrelierte
History. Optionale Git-/Toolchain-Abfragen laufen nur beim Start, mit Zeitgrenze
und Abbruchmöglichkeit. Der Snapshot enthält keine eigenen Felder für absolute
Projektpfade, Umgebung oder Repository-URL. Unveränderte Argumente und Diagnosen
können trotzdem vertrauliche Informationen enthalten.

`Report::title()` und `Report::signature()` liefern die beim Create-Aufruf berechneten
Werte. Die Signatur verwendet den v1-Vertrag einschließlich des unveränderten
Hash-Namensraums `bug_hunter.signature`. Nur Kind und normalisierte Meldung bestimmen
die Identität, nicht Backtrace, Location, Exit-Status oder Kontext.
Der beim Start aufgelöste Projektpfad bleibt privat und dient der Normalisierung.
`Report::to_markdown()` liefert den gemeinsamen vollständigen Text mit H1,
Signatur-Marker, Failure, Application, Environment und Commands.
Die H1 maskiert Markdown-Syntax; Meldung und Backtrace stehen in Text-Codeblöcken,
strukturierte Daten in Pretty-JSON. Jeder Fence ist länger als alle Backtick-Folgen
seiner Nutzdaten. Fehlende Diagnosen erscheinen als `Unavailable.` oder als ausdrücklich
erklärtes JSON-`null`. Der Text verwendet LF und endet mit genau einem LF.

Der Server übernimmt Failure- und ObservationError-Events in Activity; ein
behandelter Panic oder Tracing-Error beendet nicht automatisch die Session.
Das Erzeugen oder Rendern eines Reports schreibt keine Datei und veröffentlicht nichts.

`report::submit(&report, &session)` verwendet den ausgewählten Provider.
Es verwendet den beim Start geöffneten Artefakt-Root und die damalige Report-Konfiguration,
nicht eine neu aufgelöste absolute Pfadzeichenfolge. Der Aufruf ist synchron, arbeitet
außerhalb des Session-Koordinators und bleibt nach Session-Ende verfügbar.

Für `{"kind":"local"}` gilt:

- Ziel ist `<output>/v1-sha256-<digest>.md`, die zurückgegebene `FileReference.path`
  bleibt relativ zum Artefakt-Root.
- Jede vorhandene Verzeichniskomponente und die Zieldatei werden ohne Symlink-Folgen
  geöffnet. Auch Links innerhalb des Roots sind ungültig.
- Eine neue temporäre Datei wird vollständig geschrieben und synchronisiert, dann
  per Hardlink ohne Überschreiben unter dem endgültigen Namen eingesetzt.
  Temporäre Dateien werden anschließend entfernt. Fehlende Hardlink-Unterstützung
  ist ein Schreibfehler, kein Anlass für einen überschreibenden Ersatzweg.
- `Created` meldet eine neue Datei, `Existing` eine unveränderte Datei mit derselben
  vollständigen Signatur. Ein fehlender, anderer oder mehrdeutiger Marker ergibt
  `report::Error::Local(local::Error::Conflict { path })`. Marker in Code-Fences
  sind Nutzdaten und zählen nicht.
- Ungültige Pfade ergeben `InvalidPath`; Dateisystemfehler enthalten `Read` oder
  `Write`, den relativen Pfad und eine lesbare Meldung. Fehlertexte sind kein Steuervertrag.

Für `{"kind":"github"}` gilt:

- `gh` läuft im beim Start aufgelösten Projektverzeichnis. Repository, Host und
  Anmeldung kommen aus dessen Git-Kontext und der von `gh` unterstützten Umgebung.
  Der Provider besitzt keine weiteren Konfigurationsfelder.
- `gh api --paginate` fragt alle offenen und geschlossenen Issues ab. Pull Requests
  zählen nicht. Ein vollständiger eigener Marker ergibt `Existing` mit Nummer als
  `identifier` und URL; das Issue wird nicht geändert.
- Ohne Treffer sendet `gh api --method POST --input -` Titel und vollständiges
  Markdown als JSON über stdin. Es gibt keine zusätzliche Rückfrage.
  Die Antwort ergibt `Created` mit Issue-Referenz, ohne lokale Datei.
- Jeder Remote-Fehler führt zum selben lokalen Speicherweg. `Fallback` enthält
  Datei-Referenz und Provider-Fehler, auch wenn die lokale Datei bereits existierte.
  Scheitert auch Local, liefert `report::Error::FallbackFailed` beide Ursachen.
- GitHub-Fehler unterscheiden `Unavailable`, `CommandFailed` und `InvalidResponse`,
  jeweils mit `Search` oder `Publish`. Freie stderr-Texte werden nicht in vermeintliche
  Authentisierungs- oder Netzwerkkategorien umgedeutet.

Ein fehlgeschlagener Publish-Aufruf beweist nicht, dass kein Issue angelegt wurde.
Es gibt keinen automatischen POST-Retry und keine atomare Remote-Eindeutigkeit bei
gleichzeitigen Aufrufen. Die nächste Suche kann ein zuvor angelegtes Issue finden.
Der direkte synchrone `submit` wartet ohne Serverfrist auf den Abschluss des Providers.
Im Server verwenden dieselben Provider zusätzlich dessen gemeinsames Abbruchsignal.

### Automatische Reports im Server

Bei jedem empfangenen Failure-Event erzeugt der Server sofort den Report-Snapshot.
Ein eigener Worker je Session verarbeitet die Reports in Empfangsreihenfolge.
Langsame Provider blockieren weder neue Commands noch den Empfang weiterer Events
oder das Leeren der Spiel-Pipes. Clients müssen dafür weder verbunden bleiben noch
einen zusätzlichen Submit-Aufruf senden.

Ein Activity-Eintrag enthält Snapshot und Ergebnis gemeinsam:

```json
{"kind":"report","report":{"title":"...","failure":{},"signature":{},"context":{}},"result":{"status":"submitted","outcome":{"kind":"created","reference":{"kind":"issue","reference":{"identifier":"42","url":"https://example.test/org/repo/issues/42"}}}}}
```

Die leeren Report-Objekte oben sind Platzhalter; tatsächliche Einträge enthalten den
vollständigen Snapshot. `result.status` ist `submitted` mit Provider-Outcome,
`failed` mit typisiertem Submit-Fehler oder `interrupted` mit lesbarer Meldung.
Provider-Fehler beenden eine ansonsten bedienbare Session nicht automatisch.
Wie andere Activity-Daten unterliegen auch diese Einträge der Byte-Grenze;
ein übergroßer Report erzeugt eine erkennbare Lücke statt gekürzter Nutzdaten.

Session-Ende und Report-Abschluss sind getrennt. Ergebnisse können nach einem
Session-Endevent oder dem Lifecycle-Zustand `Ended` eintreffen. Ein regulärer
Session-Stopp lässt Reports abschließen. Das Serverende wartet auf Sessions und
Report-Worker, aber nicht auf das Abholen ihrer Ergebnisse durch Clients.
Die gemeinsame Serverfrist und ein erzwungener Stopp erreichen auch Reports bereits
terminaler Sessions. `gh` samt eigener Prozessgruppe wird beendet und bereinigt;
nach Abbruch beginnt weder ein weiterer Remote-Aufruf noch ein lokaler Fallback.
Lokale Reads und Writes prüfen den Abbruch zwischen IO-Schritten und entfernen
unveröffentlichte temporäre Dateien. Nicht unterbrechbare Dateisystemaufrufe können
die Bereinigung verzögern; der Server wartet auf den tatsächlichen Ressourcenabschluss.
Ein unterbrochener Remote-Aufruf kann bereits ein Issue angelegt haben.
`interrupted` behauptet daher weder erfolgreiche Übertragung noch Rücknahme.
Unvollständige Report-Arbeit führt beim Serverende zum Fehlerstatus.

Nachweise:

```sh
cargo test --no-default-features --lib report::provider -- --test-threads=1
cargo test --all-features --test observation -- --test-threads=1
cargo build --features cli --bin woodpecker
python3 tests/observation.py
```

## Paketierung

Die direkte Session und das Plugin brauchen kein Feature. `server` aktiviert HTTP
und WebSocket, `client` den synchronen Netzwerkclient, `cli` das Binary `woodpecker`.
Anwendungen mit Bevy UI müssen zusätzlich `woodpecker/ui` aktivieren. Damit verwendet
auch die ältere `Interaction`-Logik den virtuellen Pointer statt der nativen Maus.
Die Pointerposition wird über `PointerLocation` gelesen; `Window` bleibt die
Beschreibung des nativen Fensters, kein virtueller Eingabestatus.
Die Prozessverwaltung dieses Durchstichs unterstützt Unix-Prozessgruppen.
Das Testpackage verwendet für Zähler und Context-Menu das Feature `slice`. Die übrigen Bevy-Anwendungen
laufen mit nativer Eingabe; ihr altes Feature `automation` wurde entfernt.
Ohne `screenshot` benötigt die Bibliothek keinen Renderer. Das optionale Feature
verwendet Bevy Render und PNG-Encoding, installiert aber keinen Renderer.
Die übrigen Render-/UI-Abhängigkeiten der Testanwendungen gehören zu deren eigenem Manifest.

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

## Interaktive REPL

```sh
target/debug/woodpecker --address 127.0.0.1:4100 session repl "$ID"
```

Beispiele innerhalb der REPL:

```text
help
tick warp 10000 pace 20
pending
inspect resource counter::Counter
tick warp stop
help inspect
command {"command":"screenshot.capture","arguments":{"path":"shots/current.png"}}
quit
```

Die Kurzformen entsprechen dem Zielvertrag. `command` dekodiert das gemeinsame
Command-/Arguments-Objekt und erreicht auch Input- und Screenshot-Commands.
Ein Parsefehler vergibt keine Request-ID. Fachliche Ablehnungen erscheinen
dagegen als korreliertes Ergebnis eines angenommenen Commands.

Die REPL beobachtet Activity ab der ersten serverseitigen Momentaufnahme und zeigt
bereits offene Commands, auch von anderen Clients. Vergangene abgeschlossene Ergebnisse
werden beim Einstieg nicht erneut ausgegeben. `pending` fragt den Server erneut ab;
es ist keine Rekonstruktion aus möglicherweise bereits verdrängten Activity-Einträgen.
Bei Wiederverbindung bleibt der bisherige Cursor erhalten. Eine Lücke wird ausdrücklich
als unbekannter Ausgang gemeldet, danach geht die Beobachtung am ältesten behaltenen
Eintrag weiter. Eine unbestätigte Einreichung wird nie automatisch wiederholt.

Ein Netzwerkworker trennt blockierende Client-Aufrufe von der Terminaleingabe.
Die lokalen Queues sind auf 16 Eingaben und 32 Antworten begrenzt; volle Eingabequeues
werden sichtbar abgelehnt, nicht stillschweigend verworfen. Terminalausgabe ist synchron:
Ein nicht lesender stdout-Empfänger kann sie wie bei anderen CLI-Ausgaben blockieren.
Am Terminal funktionieren Unicode-Eingabe, Pfeiltasten, Home/End, Backspace/Delete und
Ctrl+U. Ausgaben erhalten die aktuelle Eingabe; Steuerzeichen aus Nutzdaten werden escaped.
`crossterm` und `unicode-width` sind nur im `cli`-Feature eingebunden.

Quit, Ctrl+C und EOF trennen nur den Client. Der Netzwerkworker schließt dabei seine
Verbindung und wird vollständig beendet, auch bei blockiertem Handshake oder Poll.
`shutdown` fordert ausdrücklich den Session-Abschluss an. Ein von dieser REPL bestätigter
Shutdown endet bei regulärem Lifecycle-Ende erfolgreich; unerwartetes Session-Ende und
Terminal-I/O-Fehler liefern einen Fehler. Das REPL-Ende bestätigt keinen Abschluss noch
laufender Reports. Während Replay ist nur Replay-Stop als Spielsteuerung möglich.

## Scripts

```sh
target/debug/woodpecker --address 127.0.0.1:4100 \
  session script "$ID" --file commands.json
```

Die Session-Auswahl steht außerhalb der Datei. Beispiel für `commands.json`:

```json
{
  "version": 1,
  "commands": [
    {"command": "recording.start", "arguments": {"path": "recordings/script.jsonl"}},
    {"command": "tick.warp.start", "arguments": {"ticks": 30}},
    {"command": "recording.stop", "arguments": {}},
    {"command": "shutdown", "arguments": {}}
  ]
}
```

`cli::script::Script::parse` liest keine Dateien und reicht keine Commands ein.
Die CLI liest die Datei und validiert sie vollständig, bevor sie die Session bindet.
Unbekannte oder doppelte Felder, andere Versionen, ungültige Command-Strukturen und
Shutdown vor dem letzten Eintrag verhindern die gesamte Ausführung. `Script::new`
wendet dieselben Regeln auf Rust-Listen an, einschließlich der JSON-Darstellbarkeit.
Fachliche Vorbedingungen wie gültige Pace, vorhandene Handles oder aktives Recording
bleiben bei der Session und können korrelierte Ablehnungen ergeben.

Normale Commands werden in Dateireihenfolge ohne Ergebnisbarriere eingereicht.
Ein Inspect unmittelbar nach einem Warp wartet **nicht** auf dessen Ende. Vor
Recording-Start, Recording-Stop und Shutdown wartet der Script-Client dagegen alle
vorherigen eigenen Ergebnisse ab. Fremde Client-Arbeit bleibt unter der Kontrolle
der Session und kann dort weiterhin eine Ablehnung verursachen.

Die JSON-Zusammenfassung enthält `kind: "passed"` und `completed`, oder `kind: "failed"`,
`completed` und `failures`. Beide Listen sind nach der ursprünglichen, nullbasierten
`command_index` sortiert. Erfolgreiche Einträge enthalten außerdem `request_id`,
das vollständige `command`-Objekt und `output`. Fehlereinträge enthalten den Index,
Command, eine optionale bestätigte ID und `reason`:

- `rejected` oder `failed` behält den Fehlercode und die Meldung.
- `unknown` bedeutet, dass kein verlässliches Ergebnis vorliegt. Eine fehlende ID
  bei unbestätigter Einreichung beweist nicht, dass der Server nichts angenommen hat.
- `not_submitted` bezeichnet wegen eines Abbruchs nicht mehr eingereichte Commands.

Fachliche Ablehnungen verhindern nicht die Einreichung späterer Commands. Bei
Verbindungsfehler, Activity-Lücke oder beschädigter Korrelation stoppt die Einreichung;
bereits bekannte Ergebnisse bleiben erhalten. Offene Ausgänge werden ausdrücklich
unbekannt, statt Erfolg oder Rücknahme zu behaupten. Es gibt keine automatische
Wiederverbindung oder Wiederholung einer Script-Einreichung.

Nur ausschließlich abgeschlossene Commands ergeben Exit-Code 0; eine leere Liste
ist erfolgreich. Parsefehler liefern vor der Ausführung `invalid_script` mit optionalem
Index. Bei Fehlern ist der Exit-Code ungleich 0. Ctrl+C unterbricht nur diesen Client,
schließt seine Verbindung und sammelt den Worker ein. Angenommene Arbeit läuft weiter.
stdin ist kein Script-Eingabekanal; EOF dort beendet weder das Script noch die Session.
Es gibt keinen impliziten Stop oder Shutdown und keine pauschale Ausführungsfrist.

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
{"version":1,"call":3,"operation":{"kind":"snapshot"}}
```

`snapshot` antwortet mit `result: {kind: "snapshot", snapshot: {cursor, state, pending}}`.
`pending` enthält nach Request-ID sortierte Objekte mit `request_id` und `command`.
Der Server hält diese offenen Annahmen unabhängig von Activity-Eviction; ihre Updates
und die zugehörigen Activity-Einträge erfolgen unter derselben Sperre. Die Abfrage
verbraucht keine Session-Request-ID. Terminale Sessions besitzen keine offenen Commands.

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
sortiert. Die einzelnen Verwaltungs-, Submit- und Poll-Aufrufe geben JSON-Ergebnisse
auf stdout aus. Die REPL verwendet dagegen einen Prompt, Statuszeilen und JSON-Payloads.

## Nachweise

```sh
cargo test --features cli --lib --test session
python3 tests/slice.py
python3 tests/shutdown.py
python3 tests/repl.py
python3 tests/script.py
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

`repl.py` prüft zwei echte Bevy-Sessions, laufenden Warp mit weiteren Eingaben,
Pending anderer Clients, Replay-Gate, ausdrücklichen Shutdown und unerwartetes Ende.
Pseudoterminal-Tests prüfen die teilweise eingegebene Zeile während Activity-Ausgabe,
Ctrl+C, Ctrl+D und die Wiederherstellung des Terminalmodus. Pipe-EOF, Quit und SIGINT
lassen angenommene Arbeit weiterlaufen. Rust-Netzwerkfixtures prüfen zusätzlich
Wiederverbindung, Cursor-Lücke, ungewisse Einreichung ohne Wiederholung und den Abbruch
blockierter Handshakes beziehungsweise Activity-Antworten.

`script.py` prüft vollständige Vorvalidierung ohne ausgeführtes Präfix, weiterlaufende
normale Commands, fachliche Ablehnungen, reale Recording-Header-/Footer-Barrieren,
Shutdown sowie Ctrl+C bei weiterhin laufendem Warp. Netzwerkfixtures prüfen zusätzlich
vertauschte Outcomes, fremde Request-IDs, Cursor-Lücken, verlorene Submit-Bestätigung,
fehlerhafte Outputs und Session-Ende. Die Zusammenfassung behält ursprüngliche Indizes.

## Noch nicht enthalten

Die vollständige Szenen-/Lastabnahme bleibt offen. Ein früherer externer pi-Lauf
prüfte CLI, Inspect, Warp und Recording am Zähler sowie das Weiterlaufen nach
Agent-Ende. Er war keine Headless-Bild-/Interaktionsabnahme; gleichzeitige
menschliche Bedienung und abschließendes Cleanup wurden in diesem Lauf nicht
bestätigt. Das ausführliche Protokoll bleibt im Git-Stand `b8d18e5` erhalten.
Panic-/Tracing-Beobachtung, Snapshot-Metadaten, gemeinsame Report-Darstellung
und beide Report-Provider einschließlich automatischer Server-Verarbeitung sind implementiert.
Spiel-stderr bleibt ein laufender menschlicher Diagnosestrom;
vollständig validierte interne Marker werden entfernt.
Startfehler enthalten einen begrenzten stderr-Auszug.

Die Activity-Grenze von 4 MiB ist ein vorläufiger Betriebswert, keine
Vollständigkeitsgarantie. Große Inspect-Werte können sofort eine Lücke
erzeugen. Die Bemessung unter anhaltender Fehlerlast bleibt offen. Ausstehende
Report-Snapshots warten derzeit in einer unbeschränkten Queue je Session; ein langsamer
Provider kann daher Speicherbedarf ansammeln, obwohl die fertige Activity begrenzt ist.
Der Server meldet bei Fristablauf Fehler und wartet auf die Prozessbereinigung;
er behauptet keine harte Zeitgarantie für nicht unterbrechbare Betriebssystemarbeit.

Auf dem verwendeten macOS-Rechner hat Gatekeeper das generierte `process_fixture`
bei wiederholten Starts vereinzelt verzögert oder mit SIGKILL beendet.
Das wurde auch mit direkten `cargo run`-Aufrufen außerhalb des Servers reproduziert;
das Systemlog nennt `Gatekeeper rejection`. Solche Läufe können in `Starting`
verbleiben oder als `Failed` enden. Die Tests enthalten keine automatische
Wiederholung, und die Implementation verändert keine Sicherheitsregeln des Systems.
