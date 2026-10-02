# Testordnung

Tests nach ihrem Vertrag und Ausführungsweg gruppieren, nicht nach dem Datum ihrer
Entstehung. Ein Erfolgsfall, der nebenbei durchlaufen wird, ersetzt keinen gezielten
Fehlerfall. Keine automatischen Grafikstarts, versteckten Ticks oder Capture-Retries.

## Gruppen

| Ort | Zweck | Ausführung |
| --- | --- | --- |
| `integration/` | Öffentliche Rust-Session-/Report-/Client-Verträge mit echten Prozessen | Explizite Cargo-Test-Targets, siehe unten |
| `acceptance/headless/` | CLI-/HTTP-/WebSocket-/Terminal-Abnahmen ohne Renderer | Vier unabhängig ausführbare Python-Module |
| `acceptance/rendered/` | Szene-spezifische Steuerung/Bilder und kombinierter Untersuchungsablauf | Nur nach frischer GUI-Freigabe je Versuch |
| `unit/` | Schnelle Tests der eigenständigen Prüforakel | Python-Unittest-Discovery |
| `support/` | Serverstart, CLI-Aufruf, Poll-Bedingungen, PNG-Auswertung, Quaternion-/Lifecycle-Orakel | Hilfsbibliothek, kein Test-Runner; keine Prozesse beim Import |
| `fixtures/` | Anwendungen, künstliche Prozesse und Launch-Konfigurationen | Von den jeweiligen Tests gestartet |
| `diagnostics/` | Manuelle historische Diagnoseexperimente und Instrumentierung | Nicht Teil der normalen Suite; eigene Freigaben/Voraussetzungen |
| `src/**` | Modulnahe Rust-Vertragstests | Library-Testlauf; private Details bleiben beim Owner |

Python-Module vom Repository-Root mit `python3 -m …` ausführen. Keine Kompatibilitäts-
Wrapper für alte Pfade: die früheren Testprogramme sollen nicht erneut als vermeintlich
zusätzliche Tests auftauchen. Historische Ergebnislogs bleiben unverändert.

## Normale headless Prüfung

Vor den Prozess-/CLI-Tests ihre Anwendungen bauen:

```sh
cargo build --manifest-path tests/fixtures/process/Cargo.toml
cargo build --example observation_fixture --example activity_fixture
cargo build --manifest-path tests/fixtures/panic_abort/Cargo.toml --locked
cargo build --features cli
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin counter

python3 -m unittest discover -s tests/unit -v
cargo test --all-features --lib --tests -- --test-threads=1

python3 -m tests.acceptance.headless.session_lifecycle
python3 -m tests.acceptance.headless.script
python3 -m tests.acceptance.headless.repl
python3 -m tests.acceptance.headless.server_shutdown
```

`session_lifecycle` enthält benannte Szenarien für Lifecycle, Recording/Replay und
`automatic_reports.check_automatic_reports`: sechs Sessions, davon zwei mit Tracing-
Error bzw. caught Panic. `automatic_reports` ist kein fünfter Runner. Der gemeinsame
Server wird genau einmal gestartet; fachliche Assertions der Report-Szenarien bleiben
explizit. Fehler stoppen den betreffenden Runner, nicht automatisch alle Testgruppen.

Rust-Gruppen lassen sich unabhängig auswählen:

```sh
cargo test --all-features --test session
cargo test --all-features --test failure_reports
cargo test --features client --test activity_retention
cargo test --test panic_abort
```

Der gesamte Library-Lauf enthält auch die servereigenen Report-Queue-/Mehrsession-
Tests. Nur beim gezielten Auswählen dieser Tests heißen sie weiterhin
`server::tests::blocked_provider_accumulates_failure_snapshots_without_blocking_session`
und `server::tests::slow_reports_outlive_sessions_and_share_server_deadline_and_forced_cleanup`.

## Grafische Abnahmen

```sh
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bins
# Erst nach GUI-Freigabe, jeweils einzeln:
python3 -m tests.acceptance.rendered.context_menu
python3 -m tests.acceptance.rendered.logical_state
python3 -m tests.acceptance.rendered.game_menu
python3 -m tests.acceptance.rendered.ui_drag_drop
python3 -m tests.acceptance.rendered.mesh_picking
python3 -m tests.acceptance.rendered.blend_modes
python3 -m tests.acceptance.rendered.investigation
```

`context_menu` unterstützt weiterhin `--capture-dir`. `investigation` benötigt die
beiden Sichtbarkeitsbestätigungen. Fenstergröße, Skalierung und Displayzuordnung
bleiben während jeder Session unverändert. Syntax-/Import-/Bilddecoderchecks sind
keine neue gerenderte Abnahme.

## Behalten, zusammenlegen, entfernen

| Gruppe / bisheriger Name | Entscheidung | Eigenständiger Nachweis / Ersatz |
| --- | --- | --- |
| `slice.py` | Umbenannt zu `session_lifecycle` | CLI-Verwaltung, mehrere Sessions, Stillstand, Pace, Recording/Replay, Stop während Start/Replay und fehlgeschlagener Eintrag |
| `observation.py` (Python) | Separaten Runner entfernt; Szenarien in Lifecycle integriert | Beide Failure-Ursprünge, automatische Report-Persistenz, Signatur/History und unveränderte Report-Activity nach Stop bleiben in `automatic_reports` erhalten |
| `script.py`, `repl.py`, `shutdown.py` | Behalten, als Module geordnet; Shutdown klar benannt | Script-Barrieren/Indexzuordnung, echte PTY-/Terminal-Interaktion bzw. Betriebssystemsignale; nicht durch den Rust-Server-Core ersetzt |
| `ui.py` | Umbenannt zu `context_menu` | Bevy-Textfokus, gespeichertes Unicode, Filter/Selektion, Menü-Hierarchie, Captures und UI-Replay |
| Weitere grafische Szenen | Behalten | Unterschiedliche World-/Picking-/Layout-/Material-Schedules; gemeinsame Tick-/Capture-Routine macht Szenenverhalten nicht redundant |
| `investigation.py` | Behalten | Zusammenhängende Recording–Input–Capture–Failure–Report–Replay-Kette mit Pixelvergleich in frischer Session |
| Rust `session.rs` | Verschoben, unverändert | EOF/Drain, Protokollkorrelation, Überlauf, Recording-Barrieren, Replay-Blockierung und unerwartetes Ende; der glückliche CLI-Pfad deckt das nicht ab |
| Rust `observation.rs` | Umbenannt zu `failure_reports` | Hooks/Filter/Marker, unveränderlicher Kontext und lokale/Remote-Provider-Fehler; automatische Server-Abnahme ersetzt direkte Report-Nutzung nicht |
| Rust `activity.rs` | Umbenannt zu `activity_retention` | Echte übergroße Reflection-/Report-Ergebnisse, Ergebnislücken und nicht lesender Socket |
| Rust `panic_abort.rs` | Behalten, verschoben | Separates echtes Abort-Profil mit compile-time Prüfung; expliziter `process::abort()` ist kein Ersatz |
| Report-Queue-/Mehrsession-Tests | Beim privaten Server-Owner behalten | Blockierter Provider, wartende Report-Kopien, gemeinsame Frist, erzwungene Prozessgruppenbereinigung |
| `test_investigation.py`, `test_mesh_rotation.py` | Umbenannt und nach `unit/` verschoben | Negative Lifecycle-Orakel und unabhängige Rotationsbeispiele verhindern falsch grüne Grafiktests |
| Wiederholte CLI-/Warte-/Bild-/Rotationshilfen | Aus Runnern entfernt und nach `support/` konsolidiert | Eine Implementation, keine Testprogramm-Imports; Journale, Fristen und Capture-Concurrency bleiben erhalten |
| Diagnoseexperimente | Manuell getrennt behalten | Historische Instrumentierung ist keine normale Abnahme und kein nachgewiesenes Duplikat; nicht allein aufgrund ihres Alters löschen |

Keine fachlichen Regressionstests wegen zufälliger Erfolgsüberschneidung gelöscht.
Die Einsparung liegt in einem entfernten Runner/Server-Lifecycle und gemeinsamen
Hilfen, nicht in geschwächten Assertions. Weitere Löschungen brauchen einen konkret
benannten Test, der denselben Fehlerfall zuverlässig erkennt.

## Validierung der Neuordnung

Logs: `target/final-validation/organised-*.log`; statischer Erhaltungsnachweis:
`target/test-organisation/preserved-assertions.json`.

- Alle vier headless Runner und der gesamte serielle Root-Lauf bestanden.
- Die vier verschobenen Rust-Testdateien sind bytegleich zum Commit `eeeb8d0`.
- Die fachlichen Assertions aller sieben grafischen Runner sind AST-gleich erhalten;
  ausgelagerte Hilfsassertions stehen in `support/`.
- Sieben bestehende Unit-Orakel bestehen; Module importieren ohne Prozessstart.
- Sechs gespeicherte PNGs geprüft; ausgelagerte Bild-/Rotations-/Lifecycle-Hilfen
  sind AST-gleich. Drei historische Diagnose-Patches lassen sich auf Quellkopien
  anwenden. Dabei wurde eine bereits veraltete Blend-Capture-Seam aktualisiert;
  Instrumentierung läuft weiterhin ausschließlich in Kopien unter `target/`.
- Root-Clippy, Format und Diff-Prüfung bestanden. Der separate Bevy-Clippy-Befund
  wurde anschließend gezielt ohne Suppression behoben: strenger Gesamt-Clippy,
  alle 10 Bevy-Tests und Check ohne Features grün; Logs `target/clippy-cleanup/`.
- Anschließend bestanden alle sieben grafischen Abnahmen nach frischem Build und
  ausdrücklicher Freigabe: `target/graphical-final-validation/summary.json`.
  Der erste Investigation-Lauf lief vor dem ersten Tick in den Bestätigungs-Timeout;
  erst nach neuer Freigabe bestand der zweite Versuch mit zwei Nutzerbestätigungen.
  Beide Laufnachweise bleiben erhalten; keine eigenen Spiel-/Serverprozesse aktiv.
- Testordnung, Clippy-Bereinigung und Abschlussdoku werden als gemeinsamer
  Abschluss-Checkpoint versioniert; lokale `target/`-Evidenz nicht. Dynamische
  Fensterwechsel bleiben zurückgestellt.
