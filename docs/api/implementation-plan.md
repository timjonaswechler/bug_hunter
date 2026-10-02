# Abschlussplan

Die wesentliche Command- und Session-Funktionalität ist implementiert.
Die ergänzten Steuerungsfälle und technischen Screenshot-Prüfungen bei stabilen
Fensterbedingungen sowie der zusammenhängende Untersuchungsablauf sind abgenommen.
Die gezielten Last-/Fehlerprüfungen und die Schlussprüfungen im aktuellen Umfang
bei stabilen Fensterbedingungen sind abgeschlossen. Diese Fortschreibung versioniert
Testordnung, gezielte Clippy-Bereinigung und Abschlussnachweise.
Dynamische Fensterwechsel sind
ausdrücklich zurückgestellt.
Vollständig verdeckte Fenster sind beim Screenshot nicht zugesichert und kein
Abschlussblocker. Vorhandene Tests belegen ihre jeweiligen Fälle, nicht den
vollständigen Abschluss des Zielvertrags.

[target.md](target.md) bleibt die maßgebliche Verhaltensbeschreibung;
[goal.rs](goal.rs) bleibt als Interface-Skizze erhalten. Dieser Plan besitzt
Aufgaben, Abdeckung und Abschlusskriterien. [next-steps.md](next-steps.md)
beschreibt den unmittelbaren Wiedereinstieg, [usage.md](usage.md) die Bedienung.

## Checkliste für den Abschluss

Die Checkboxen verfolgen die noch offene Arbeit. Erst abhaken, wenn der jeweilige
Nachweis mit Testbefehl, Arbeitsbaum und Ergebnis dokumentiert ist. Ein offener
Punkt bedeutet nicht automatisch fehlende Produktfunktion; oft fehlt nur der
kombinierte oder ergänzende Test. Details stehen in den verlinkten Abschnitten.

### Steuerung

- [x] `logical_state`: Pointer-Press erst beim Tick, keine Wiederholung beim Halten,
  Screenshot ohne Zustandsfortschritt prüfen.
- [x] `game_menu`: `s`, `Escape`, `n`, Quit-Button und Screenshots prüfen.
- [x] `ui_drag_drop`: Despawn während eines Drags gezielt in der Testszene auslösen
  und prüfen; Screenshots ergänzen.
- [x] `mesh_picking`: vertikalen Drag am Würfel sowie Drag an Kugel und Zylinder prüfen.
- [x] Relevante Layout-/Hierarchiewechsel und ungültig gewordene Entity-Handles
  in den ergänzten Abläufen prüfen.

[Beispiele und Kriterien](#1-steuerungsabnahme-vervollständigen)

### Technische Screenshot-Abnahme

- [x] Bestehende Blend-, Mesh- und UI-Bildtests mit verfügbarer Renderoberfläche
  ausführen; UI-Overlay und Mehrkamera-Ausgabe prüfen.
- [x] Unveränderte Simulation, gültige schwarze Bilder, Surface-Ablehnung,
  Timeout, späte Readbacks und sichere Dateipfade erneut prüfen.
- **Zurückgestellt (nicht bestanden):** Dynamische Resize-, DPI- und
  Bildschirmwechsel sind auf ausdrücklichen Nutzerwunsch kein Abschlussblocker.
  Der aktuelle Umfang setzt stabile Fensterbedingungen voraus; der experimentelle
  Resize-Umbau ist zurückgenommen.

[Kriterien](#2-screenshot-verhalten-absichern). Vollverdeckung und ein allgemeiner
Sollbildvergleich sind keine Abschlussaufgaben.

### Zusammenhängender Ablauf

- [x] Recording, Eingaben, Ticks, Inspect, Capture, Failure und lokalen Report
  in einem CLI-Ablauf verbinden; Client-Trennung und Wiederverbindung prüfen.
- [x] Verwaltungs-Stopp bei aktiver Aufnahme prüfen: JSONL-Footer, unveränderlicher
  Report, Session-Ende und Prozessbereinigung.
- [x] In einer frischen Session den aufgezeichneten Ablauf wiedergeben und den
  Fixture-Fehler reproduzieren; aktive Wiedergabe und unerwartetes Ende ergänzen.

[Kriterien](#3-kombinierten-untersuchungsablauf-abnehmen)

### Last und Fehlerfälle

- [x] Große Inspect-/Report-Ausgaben und langsame Clients gegen die Activity-Grenze
  prüfen; verlorene und unbestätigte Ergebnisse als unbekannt behandeln.
- [x] Wartende Reports bei langsamem Provider und anhaltenden Failures bemessen;
  bei nötiger Überlastregel die Vertragsentscheidung vorlegen.
- [x] Mehrere Sessions mit laufender Arbeit und Reports unter gemeinsamer
  Shutdown-Frist prüfen; vollständige Prozessbereinigung nachweisen.
- [x] Ein separat mit `panic=abort` gebautes Programm abnehmen.

[Kriterien](#4-last-ergebnislücken-und-shutdown-prüfen)

### Schlussprüfung

- [x] Öffentliche Requests, Capabilities, Fehlerformen und Features gegen `target.md` prüfen.
- [x] Root-Tests, separates Bevy-Paket, Feature-Builds und CLI-Abnahmen ausführen.
- [x] Prüfstand, verbleibende Einschränkungen und Dokumentationslinks aktualisieren.

[Kriterien](#5-vertrags--und-dokumentationsabgleich-abschließen)

## Umfang und implementierter Stand

Implementiert sind:

- Session-/Prozessverwaltung, explizite Warps, Pace und Stop;
- virtuelle Pointer-, Keyboard- und Texteingaben ohne nativen Fokus;
- allgemeines Entity-/Resource-Inspect einschließlich Reflection-Wertstatus;
- Screenshot-Grundfunktion mit framegebundener Surface-Absicherung;
- Recording, Replay, Failure-Beobachtung und lokale/GitHub-Reports;
- Mehrsession-Server, Client, CLI, REPL und Scripts über denselben Command-Weg.

Alle sieben Bevy-Szenen sind über das Feature `slice` angebunden. Native
Bedienung ohne dieses Feature bleibt erhalten. Die Prozessverwaltung unterstützt
Unix. Externe Agent-Laufzeiten verwenden die CLI; Modellzugang und
Werkzeugschleife gehören nicht zu woodpecker und sind kein eigener Abschlussblock.

Die v2-Migration ist abgeschlossen. Es gibt keinen parallel gebauten Legacy-Weg,
keine `automation`-Marker, alten Controller-Exports oder `failure.json`.
Die frühere Dateizuordnung steht in `de32d09:docs/api/migration.md`; entfernte
v2-Quellen sind unter `5e9552e` verfügbar. Diese Historie ist keine offene Bauliste.

## Verbleibende Arbeit

### 1. Steuerungsabnahme vervollständigen

Die bestehenden CLI-Tests erweitern, keine weitere Steuerungsabstraktion bauen.
Jeder Test liest Positionen, Größen und Zustand über das allgemeine Inspect.
Inputs bleiben bis zum expliziten Tick vorgemerkt; Inspect und Capture dürfen
keinen Spielzustand fortschreiben. Ein zusätzlicher virtueller Pointer gehört
nicht zum Zielvertrag und wird nicht als fehlende Abnahme geführt.

Die folgenden Beispiele beschreiben geplante Prüfungen, keine bereits bestandenen Tests.
Screenshots für `logical_state`, `game_menu` und `ui_drag_drop` ergänzen die
Zustandsprüfungen. Relevante Layout-/Hierarchiewechsel und tote Handles mitprüfen.

#### Beispiel 1: Mausklick in logical_state

In [logical_state.py](../../tests/acceptance/rendered/logical_state.py) den Button
`logical-button` nach einem expliziten Layout-Tick per Inspect finden.
Den virtuellen Pointer zur gelesenen Mitte bewegen und per Tick positionieren.
`pointer_presses` aus `SessionObservation` lesen, dann links drücken.
Vor dem nächsten Warp bleibt der Zähler unverändert. Nach einem expliziten Tick
muss er genau um eins steigen. Weitere Ticks bei gehaltener Taste dürfen keinen
zweiten Press erzeugen. Loslassen und einen weiteren Tick ausführen.

Ein Capture danach darf weder diesen Zähler noch Update-/Timer-Zustand verändern.
So wird geprüft, dass Command-Annahme und Verarbeitung durch das Spiel getrennt sind.

Vorbereitung am 2026-09-28 (Darwin arm64, Branch `reference/window`, Basis
`f493dfd5b29ead0f82faf36f7a76fbba7270c08d` plus Arbeitsbaumänderungen):
`tests/logical_state.py` ergänzt Pointer-Press/Hold/Release und fünf Captures,
auch bei vorgemerktem Press/Release. Die Szene besitzt dafür einen grünen Button
auf schwarzem Hintergrund als technische Pixelfixture. Die vollständige
`SessionObservation` muss bei Inspect und Capture unverändert bleiben.

Ohne Grafik bestanden:

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin logical_state
cargo test --manifest-path bevy_test_apps/Cargo.toml --features slice --bin logical_state
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin logical_state -- -D warnings
cargo fmt --manifest-path bevy_test_apps/Cargo.toml --check
python3 -m py_compile tests/logical_state.py
git diff --check
```

Der gezielte Rust-Test bestand (1 Test); lokale Build-/Test-/Clippy-/Formatlogs
liegen unter `target/logical-state-build/`. Die unabhängige Subagent-Prüfung
fand keine Probleme.

Nach GUI-Freigabe bestand `python3 tests/logical_state.py` am selben Tag auf
Darwin arm64 mit obigem Basiscommit plus den Arbeitsbaumänderungen in
`tests/logical_state.py` und `bevy_test_apps/src/bin/logical_state.rs`.
Ergebnis: 14 Updates, 26 FixedUpdates, 6 Timer-Abschlüsse und genau 1 Pointer-Press.
Alle fünf PNGs bestanden die technischen Pixelprüfungen; Inspect und Capture
ließen die vollständige Beobachtung unverändert, auch bei vorgemerktem Press/Release.
Session und Server wurden regulär beendet (Server-Exit 0).
Commands und Serverlog: `target/logical-state-avcbevh1/`;
PNGs: `target/logical-state-avcbevh1/artifacts/82daf032538ac861f82b04b2ad633c36/screenshots/`.
Die lokalen Artefakte sind nicht im Git-Transfer enthalten.

#### Beispiel 2: Tastenkürzel und Quit in game_menu

In [game_menu.py](../../tests/acceptance/rendered/game_menu.py) das Hauptmenü erreichen.
`s` drücken und die nötigen expliziten Ticks für Eingabeverarbeitung und
State-Wechsel ausführen: Es müssen die Einstellungen erscheinen. Taste freigeben.
Mit `Escape` entsprechend ins Hauptmenü zurückkehren; `n` muss dort das Spiel starten.
Vor den Warps darf sich der jeweilige Bildschirmzustand nicht ändern.
Alte Bildschirm-Handles müssen nach dem Wechsel `entity_not_found` ergeben.

Den Quit-Fall zuletzt oder in einer separaten Session prüfen: Im Hauptmenü den
`quit-button` anhand seiner gelesenen Position drücken und einen Tick ausführen.
Die Szene sendet `AppExit::Success`; der Spielprozess muss enden und die Session
muss das tatsächliche Prozessende melden. Das ist kein angeforderter
Session-Shutdown. Auch Exit-Status 0 gilt nach dem bestehenden Vertrag als
unerwartetes Prozessende, wenn die Session es nicht selbst veranlasst hat.
Server und andere Sessions müssen bedienbar bleiben.

Vorbereitung am 2026-09-28 (Darwin arm64, Branch `reference/window`, Basis
`2e02815` plus Arbeitsbaumänderungen): `tests/game_menu.py` ergänzt die drei
Tastenkürzel mit getrennten Eingabe-/Transition-Ticks und toten Bildschirm-/Button-
Handles sowie neun PNGs, darunter Aufnahmen bei vorgemerkter Tasteneingabe.
Der Quit-Fall prüft den abgeschlossenen Ein-Tick-Warp, natürliches Prozessende
mit Exit 0, Failure-/End-Ereignisse, lokalen Report und die Isolation einer zweiten
Session. Deren Warps und regulärer Stopp bleiben möglich; beim Serverende wird
wegen der fehlgeschlagenen ersten Session `shutdown_incomplete` erwartet.
Diese Erwartungen wurden durch einen read-only Subagenten anhand der Quellen
abgeglichen; das ersetzt keine Ausführung oder unabhängige Diff-Prüfung.
Die bestehende Szene benötigt dafür keine Änderung.

Ohne Grafik bestanden:

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin game_menu
cargo test --manifest-path bevy_test_apps/Cargo.toml --features slice --bin game_menu
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin game_menu -- -D warnings -A clippy::type_complexity -A clippy::too_many_arguments
cargo fmt --manifest-path bevy_test_apps/Cargo.toml --check
python3 -m py_compile tests/game_menu.py
git diff --check
```

Die zwei gezielten Rust-Tests bestanden. Lokale Logs liegen unter
`target/game-menu-build/`.

Nach GUI-Freigabe scheiterte `python3 tests/game_menu.py` am 2026-09-28 auf
Darwin arm64 mit obiger Basis plus Arbeitsbaumänderungen beim Quit-Nachweis:
`missing Quit outcome, process-end event or local report`.
Tastenkürzel, tote Handles und alle neun PNG-Prüfungen bestanden zuvor.
Quit-Warp #228 wurde mit genau einem ausgeführten Tick abgeschlossen.
Danach kam jedoch `Ended { reason: TransportClosed { channel: "stderr" } }`
und Lifecycle `Failed` mit Fehlercode `ended`, kein ProcessExit-Failure und kein
Report. Der Isolationstest nach Quit wurde daher noch nicht erreicht.

Quellenbefund: `src/session/coordinator.rs` setzt beim Pipe-EOF bereits
`TransportClosed`, falls `try_wait()` noch kein Prozessende liefert; die folgende
Bereinigung kann den Prozess terminieren und die Exit-Beobachtung als absichtlich
markieren. Das ist ein konkreter Race-Kandidat, noch keine durch einen minimalen
Regressionstest bestätigte Ursache. Nicht durch Sleeps, Wiederholungen oder eine
abgeschwächte Quit-Erwartung umgehen.

Evidenz: `target/game-menu-p1lmnewh/commands.jsonl`, `server.log` und die neun PNGs
unter dessen `artifacts/`. Der Fehlerpfad beendete den Server; die anschließende
Prozessprüfung fand keine laufenden `woodpecker`-/`game_menu`-Prozesse.
Der Checklistenpunkt blieb in diesem Lauf offen.

Headless-Abgrenzung am selben Tag: Die Fixture-Modi `close_stderr` und
`close_stdout` schließen nach einer gültigen Response genau eine Pipe und bleiben
anschließend ohne Sleep auf stdin blockiert. Der neue Test
`pipe_eof_with_live_child_remains_transport_end_without_process_failure` bestätigt
`TransportClosed`, erhaltene Response, genau ein Endevent, keinen erfundenen
ProcessExit-Failure und vollständige Prozessbereinigung. Der vorhandene Test
`exit_drains_responses_and_closes_events_once` bestand ebenfalls. Befehle:

```sh
cargo test --features cli --test session pipe_eof_with_live_child_remains_transport_end_without_process_failure -- --exact --nocapture
cargo test --features cli --test session exit_drains_responses_and_closes_events_once -- --exact --nocapture
cargo fmt --all --check
cargo fmt --manifest-path tests/fixtures/process/Cargo.toml --check
git diff --check
```

Logs: `target/eof-diagnosis/live-pipe.log` und `process-exit.log`. Dies sind
Abgrenzungstests, kein bestandener Regressionstest für den natürlichen Quit-Race.
Die anschließend ausdrücklich genehmigte EOF-Klärungsfrist ist jetzt in
`src/session/coordinator.rs` implementiert; die maßgebliche Regel steht im
[Zielvertrag](target.md#direkte-session-und-bevy-integration). Ab dem ersten EOF gilt eine Sekunde, ohne
Fristverlängerung bei weiterem EOF. Annahme und Replay-/Deferred-Versand pausieren;
Pipe-Drain und Abbruch bleiben aktiv. Natürlicher Exit behält Status/Failure,
Fristablauf führt verbindlich zu Transport-Cleanup ohne ProcessExit-Failure.

`eof_before_natural_exit_preserves_status_and_failure` reproduzierte vor dem Fix
exakt den stderr-Fehler (rot, `natural-exit-red.log`). Die Fixture gibt den
natürlichen Exit erst nach nachgewiesener Command-Abweisung frei; kein fixer Sleep
entscheidet den Test. Nach dem Fix besteht er für stdout und stderr. Der
unabhängige Reviewer fand ein zusätzliches Rennen am Fristablauf; die Entscheidung
für Transport-Cleanup unterdrückt nun bereits ab diesem Punkt einen ProcessExit-
Failure, auch wenn der Prozess unmittelbar danach selbst endet.

Prüfstand nach Fix (Darwin arm64, Basis `2e02815` plus aktuelle Änderungen):

- `cargo test --features cli --test session -- --test-threads=1`: 15 bestanden.
- `cargo test --no-default-features --lib`: 92 bestanden.
- `cargo clippy --features cli --all-targets -- -D warnings`: bestanden.
- CLI- und separates `game_menu`-/`slice`-Build bestanden; beim Szenenlinker bleibt
  die Warnung über die Größe von `__eh_frame`.
- Root-/Fixture-Formatprüfung, Python-Syntaxprüfung und `git diff --check` bestanden.
- Der breite Lauf `cargo test --features cli --lib --test session -- --test-threads=1`
  stoppte mit 121 bestandenen Library-Tests und einem Start-Timeout in
  `server::tests::report_submit_errors_remain_activity_without_failing_the_live_session`
  vor Ready. Nach explizitem `cargo build --example observation_fixture` bestand
  dessen gezielter Einzeltest. Das beweist keine Behebung des ursprünglichen
  Start-Timeouts und ersetzt keinen vollständig grünen Gesamtlauf.

Alle Logs liegen unter `target/eof-diagnosis/` (`root-tests.log`,
`session-tests.log`, `no-default-tests.log`, `report-start-recheck.log`,
`clippy.log`, `cli-build.log`, `scene-build.log`).

Nach erneuter GUI-Freigabe bestand `python3 tests/game_menu.py` am 2026-09-28 auf
Darwin arm64, Branch `reference/window`, Basis `2e02815` plus den beschriebenen
Arbeitsbaumänderungen einschließlich EOF-Fix. Tastenkürzel, Transition-Ticks,
tote Handles und alle neun PNG-Prüfungen bestanden. Der Ein-Tick-Quit endete
natürlich mit Exit 0; ProcessExit-Failure, terminales Endevent und lokaler Report
wurden nachgewiesen. Die zweite Session blieb unverändert und bedienbar, führte
zwei Ticks aus und ließ sich regulär stoppen. Der Server endete erwartungsgemäß
mit Exit 1 und `shutdown_incomplete` wegen der zuvor fehlgeschlagenen Quit-Session.
Die abschließende Prozessprüfung fand keine laufenden `woodpecker`-/`game_menu`-
Prozesse.

Evidenz: `target/game-menu-6stogoqp/commands.jsonl` und `server.log`.
PNGs liegen unter `target/game-menu-6stogoqp/artifacts/7e2f5386d6881be04315a80edad93f84/screenshots/`;
der lokale Report im benachbarten `reports/`-Verzeichnis heißt
`v1-sha256-7ca80a848b0913a8eb5ab37f815efe34cd2c4bcdc46ad2fd1870c08902c3f240.md`.
Die Artefakte sind lokal, nicht im Git-Transfer enthalten. Der erste fehlgeschlagene
Lauf und der separate Library-Start-Timeout bleiben als historische Befunde erhalten.

#### Beispiel 3: Objekt verschwindet während eines Drags

In [ui_drag_drop.py](../../tests/acceptance/rendered/ui_drag_drop.py) die Kachel Amber greifen
und mit expliziten Ticks einen Drag beginnen. Während die Taste gehalten wird,
lässt die Testszene Amber bei einem festgelegten weiteren Tick verschwinden.
Danach den Pointer weiterbewegen, loslassen und die Eingaben per Tick verarbeiten.

Prüfen: kein Panic oder Hänger, der alte Entity-Handle ist ungültig, die virtuelle
Taste lässt sich freigeben und eine andere vorhandene Kachel kann anschließend
normal gezogen werden. Ein Drop darf keine entfernte Kachel vertauschen.
Die genaue Observer-Folge nicht aus dem normalen Drag-Ende ableiten: Die
ursprüngliche Entity existiert dann nicht mehr.

Vorbereitet am 2026-09-28 (Darwin arm64, Branch `reference/window`, Basis
`2e02815` plus Arbeitsbaumänderungen): Die Szene entfernt im `slice`-Build Amber
bei `D`, sofern gerade ein Amber-Drag aktiv ist. Die Eingabe wirkt erst beim
expliziten Tick. Der Trigger entfernt auch die Szenenbelegung und setzt
`active_tile` zurück, ohne künstliche DragEnd-/Drop-Ereignisse zu erzeugen.
Native Bedienung ohne `slice` bleibt unverändert; es gibt keinen neuen
woodpecker-Mutationscommand.

`tests/ui_drag_drop.py` prüft den toten Kachel- und Kind-Handle, die aktualisierte
Grid-Hierarchie, unveränderte Positionen der übrigen Kacheln, Release ohne
ungültigen Tausch und einen anschließenden Blue→Green-Drag. Acht technische PNGs
prüfen bekannte Szenenfarben und unveränderten Zustand, Geometrie und Darstellung
während Capture, auch bei vorgemerktem Despawn-Key. Die Observer-Folge des toten
Drag-Ziels wird nicht aus dem normalen DragEnd abgeleitet.

Ohne Grafik bestanden:

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin ui_drag_drop
cargo test --manifest-path bevy_test_apps/Cargo.toml --features slice --bin ui_drag_drop
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin ui_drag_drop -- -D warnings
cargo check --manifest-path bevy_test_apps/Cargo.toml --bin ui_drag_drop
cargo fmt --manifest-path bevy_test_apps/Cargo.toml --check
python3 -m py_compile tests/ui_drag_drop.py
git diff --check
```

Der neue Unit-Test des Despawn-Triggers bestand (1 Test). Logs:
`target/ui-drag-drop-build/`; die bekannte `__eh_frame`-Linkerwarnung bleibt.
Die unabhängige Subagent-Prüfung fand keine Probleme.

Nach GUI-Freigabe bestand `python3 tests/ui_drag_drop.py` am 2026-09-28 auf
Darwin arm64, Branch `reference/window`, Basis `2e02815` plus den beschriebenen
Arbeitsbaumänderungen einschließlich EOF-Fix und Despawn-Erweiterung.
Alle neuen Fälle bestanden: tickgebundener Despawn, tote Kachel-/Kind-Handles,
aktualisierte Hierarchie, unveränderte übrige Positionen, Release ohne ungültigen
Tausch und anschließender Blue→Green-Drag. Alle acht PNGs bestanden die technischen
Pixel- und Stillstandsprüfungen. Session und Server endeten regulär (Server-Exit 0);
die anschließende Prozessprüfung fand keine laufenden `woodpecker`-/`ui_drag_drop`-
Prozesse.

Evidenz: `target/ui-drag-drop-h5a1713c/commands.jsonl` und `server.log`.
PNGs: `target/ui-drag-drop-h5a1713c/artifacts/805d5255aaf6b0bc5d6899c94737056c/screenshots/`.
Die lokalen Artefakte sind nicht im Git-Transfer enthalten.

#### Beispiel 4: Vertikaler Drag und weitere Meshes

In [mesh_picking.py](../../tests/acceptance/rendered/mesh_picking.py) den Würfel greifen und
den virtuellen Pointer beispielsweise zwölf logische Pixel nach unten bewegen.
Vor dem Warp bleiben Transform und Drag-Zähler unverändert. Nach dem Tick muss
`drag_events` um eins steigen und die Rotation den vertikalen Drag berücksichtigen.
Die Szene verwendet `delta.y * 0.02`, hier also 0,24 Radiant um die X-Achse.
Die zusätzlich pro Tick laufende Y-Rotation muss im erwarteten Transform enthalten sein.

Den Ablauf auch für `left-sphere` und `right-cylinder` prüfen. Nur das gegriffene
Objekt erhält den Drag-Zuschlag und ein Drag-Event; die übrigen Meshes behalten
nur ihre normale zeitabhängige Rotation. Anschließend Release und erneutes
Hover prüfen. In dieser Szene dreht Ziehen das Mesh, es verschiebt es nicht.

Vorbereitet am 2026-09-28 (Darwin arm64, Branch `reference/window`, Basis
`2e02815` plus Arbeitsbaumänderungen): `tests/mesh_picking.py` ergänzt den vertikalen
Würfel-Drag `[0, 12]` und diagonale Drags an Kugel `[12, 12]` und Zylinder
`[-12, 12]`. Die zusätzliche Tick-Prüfung vergleicht für alle drei Meshes die
vollständige Quaternion, unveränderte Translation/Skalierung und Drag-Zähler.
Die Erwartung berücksichtigt die Weltachsen-Reihenfolge Y-Drag, X-Drag, danach
zeitabhängige Y-Rotation. Ein gehaltener Stillstandstick darf keinen Drag wiederholen;
Release, Out und erneutes Hover werden mitgeprüft.

Sechs zusätzliche Captures prüfen vorgemerkte und verarbeitete Drags sowie
unveränderten Zustand während Capture. Der komplette Test umfasst damit 48 explizite
Ticks und 19 PNGs. Die Szene selbst benötigt keine Änderung.

Ohne Grafik bestanden:

```sh
python3 -m unittest discover -s tests -p 'test_mesh_rotation.py'
python3 -m py_compile tests/mesh_picking.py tests/test_mesh_rotation.py
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin mesh_picking
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin mesh_picking -- -D warnings -A clippy::type_complexity
cargo fmt --manifest-path bevy_test_apps/Cargo.toml --check
git diff --check
```

Die drei Python-Tests prüfen die Rotationsberechnung gegen geschlossene Yaw- und
Basisvektor-Erwartungen einschließlich nicht vertauschbarer X-/Y-Rotationen.
Logs: `target/mesh-picking-build/`. Die unabhängige Subagent-Prüfung fand keine
Probleme und bestätigte Rotationsreihenfolge sowie Tick-/Capture-Anzahl.

Nach GUI-Freigabe bestand `python3 tests/mesh_picking.py` am 2026-09-28 auf Darwin
arm64, Branch `reference/window`, Basis `2e02815` plus den beschriebenen
Arbeitsbaumänderungen einschließlich EOF-Fix und erweiterten Szenentests.
Alle 48 expliziten Ticks und 19 PNG-Prüfungen bestanden. Vertikaler Würfel-Drag,
diagonale Kugel-/Zylinder-Drags, vollständige Quaternionen aller Meshes, unveränderte
Translation/Skalierung, ausbleibende Drag-Wiederholung beim Halten und
Release/Out/erneutes Hover wurden nachgewiesen. Inputs und Capture ließen die
Mesh-Snapshots bis zum nächsten Warp unverändert. Session und Server endeten
regulär (Server-Exit 0); die anschließende Prozessprüfung fand keine laufenden
`woodpecker`-/`mesh_picking`-Prozesse.

Evidenz: `target/mesh-picking-1vxjx8g6/commands.jsonl` und `server.log`.
PNGs: `target/mesh-picking-1vxjx8g6/artifacts/abbb95a578f8762c2a86651523355c85/screenshots/`.
Die lokalen Artefakte sind nicht im Git-Transfer enthalten.

#### Querschnitt: Layout, Hierarchie und tote Handles

Der Abgleich der ergänzten Steuerungsfälle ergibt:

- `game_menu`: Bildschirm- und Button-Handles werden nach Pointer-/Keyboard-
  Navigation ungültig; neue Bildschirmhierarchien und gelesene Buttonpositionen
  sind in der bestandenen Abnahme geprüft.
- `ui_drag_drop`: getauschte Grid-Positionen, Entfernen der Kachel samt Kind,
  aktualisierte Grid-Hierarchie und anschließend gültiger Drag sind bestanden.
- `mesh_picking`: Drags ändern nur Rotation; Translation und Skalierung aller
  Meshes bleiben erhalten. Kein zusätzlicher Hierarchiewechsel entsteht dort.
- `context_menu`: bisher fehlten die CLI-Prüfung des Ersetzens eines noch offenen
  Menüs, seiner Kind-Handles und der Menüeintragsauswahl. Diese konkrete Lücke
  ist nun in `tests/ui.py` vorbereitet; keine zusätzliche Szenenfunktion nötig.

Die Erweiterung ersetzt das Menü über einen aus Buttonlayout und Menüposition
abgeleiteten Klickpunkt, prüft fünf geordnete Items mit Textkindern, deren Lage
innerhalb des Menüs und den neuen Menüanker. Beim Ersetzen, Auswählen von Fuchsia
und anschließenden Hintergrund-Schließen müssen jeweils alle alten Handles des
Teilbaums `entity_not_found` liefern. Button-/Textfeldlayout bleibt erhalten;
vorgemerkte Klicks und Inspect dürfen keinen Zustand fortschreiben. Zwei zusätzliche
Captures begleiten Ersetzen und Auswahl. Der vorhandene Recording-/Replay-/Zwei-
Session-Ablauf bleibt Bestandteil der Abnahme.

Vorbereitung am 2026-09-28 (Darwin arm64, Branch `reference/window`, Basis
`2e02815` plus bisherige Arbeitsbaumänderungen). Ohne Grafik bestanden:

```sh
python3 -m py_compile tests/ui.py
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin context_menu
cargo test --manifest-path bevy_test_apps/Cargo.toml --features slice --bin context_menu
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin context_menu -- -D warnings
cargo fmt --manifest-path bevy_test_apps/Cargo.toml --check
git diff --check
```

Logs: `target/context-menu-build/`. `tests/ui.py` bewahrt jetzt auch bei Fehlern
Serverlog, vollständige CLI-Antworten, Recording und PNGs unter `target/ui-*` auf;
parallele Capture-Aufrufe serialisieren nur das Evidenzschreiben.
Die unabhängige Subagent-Prüfung fand vor der grafischen Ausführung keine Probleme.

Der freigegebene Lauf `python3 tests/ui.py` am 2026-09-28 (Darwin arm64,
Basis `2e02815` plus beschriebene Arbeitsbaumänderungen) scheiterte in der neuen
Menüanker-Erwartung: Pointer-/Buttonmitte `[320, 137.5]`, tatsächliche Menüecke
`[320, 138]`. Bevy 0.19.1 aktiviert Layout-Rundung standardmäßig; Taffy rundet die
Root-Position auf physische Pixel. Dies war ein Testfehler, kein nachgewiesener
Produktfehler. Die Testszene verwendet Skalierungsfaktor 1; die Erwartung rundet
nun positive Ankerkoordinaten mit `floor(value + 0.5)`, statt die Pixeltoleranz
pauschal zu lockern.

Evidenz des fehlgeschlagenen Laufs: `target/ui-_srzqq_2/commands.jsonl`,
`server.log` und `artifacts/`. Die anschließende Prozessprüfung fand keine laufenden
`woodpecker`-/`context_menu`-Prozesse. Menüersetzung, Auswahl und der spätere
Recording-/Replay-Abschluss wurden noch nicht erreicht.
Die korrigierte echte `menu_layout`-Funktion wurde isoliert gegen die aufgezeichneten
Layoutwerte geprüft; die alte Erwartung schlägt fehl, die korrigierte besteht
(`target/context-menu-build/layout-rounding.log`). Das ersetzt nicht die komplette
Hierarchie-/GUI-Abnahme. Python-Syntax- und Diff-Prüfung bestanden erneut.
Nach erneuter GUI-Freigabe bestand `python3 tests/ui.py` am 2026-09-28 auf Darwin
arm64, Branch `reference/window`, Basis `2e02815` plus den beschriebenen
Arbeitsbaumänderungen einschließlich korrigierter Rundungserwartung.
Menüersetzung, Itemlayout, Auswahl, erneutes Öffnen/Schließen und tote Teilbaum-
Handles bestanden. Auch der vollständige bestehende Input-/Screenshot-/Recording-/
Replay-Ablauf mit zwei Sessions bestand. Session-Stopp und Serverende verliefen
regulär (Server-Exit 0); die anschließende Prozessprüfung fand keine laufenden
`woodpecker`-/`context_menu`-Prozesse.

Evidenz: `target/ui-qthjfz0o/commands.jsonl` und `server.log`.
Recording und neun PNG-Dateien der ersten Session liegen unter
`target/ui-qthjfz0o/artifacts/1820cedbcedc179e3eaf9de2175eee55/`;
die zweite Session besitzt zwei PNGs unter
`target/ui-qthjfz0o/artifacts/7090ae7e541cf122a82af6e87f925f8b/`.
Captures werden im Test teils überschrieben und beim Replay erneut ausgeführt;
die Dateianzahl ist daher nicht die Anzahl der Capture-Commands.
Die lokalen Artefakte sind nicht im Git-Transfer enthalten.

Zusammen mit den zugeordneten Game-Menu-/Drag-/Mesh-Nachweisen ist der
querschnittliche Steuerungspunkt abgeschlossen. Das schließt die separaten
Resize-/Skalierungs- und Mehrkamera-Aufgaben der Screenshot-Abnahme nicht ab.

### 2. Screenshot-Verhalten absichern

Der [diagnostizierte Fehler](diagnostics/black-screenshots.md) bleibt durch den
Guard abgesichert. Bei fehlender Renderoberfläche folgt eine explizite Ablehnung,
kein erfolgreich gespeicherter leerer Buffer. Vollverdeckung muss keine erfolgreiche
Aufnahme liefern. Ein tatsächlich gerendertes schwarzes Bild bleibt gültig.

Die bestehenden Tests prüfen die technische Aufnahme, nicht die visuelle
Richtigkeit eines beliebigen Spiels. Ein Sollbildvergleich ist keine Produktfunktion
und gehört nicht zum aktuellen Abschluss.

Abnahme:

- Sichtbare Fenster ohne Fokus, teilweise verdeckte und wieder sichtbare Fenster
  mit verfügbarer Renderoberfläche liefern die gerenderten Szenenbilder.
- UI-Overlays und mehrere Kameras desselben primären Fensters bleiben erfasst.
  Tatsächlich schwarze Szenen werden nicht anhand ihrer Pixelfarben abgelehnt.
- Capture verändert weder Simulationsticks noch simulierte Zeit oder vorgemerkte
  Eingaben. Keine Fokusänderung, versteckten Startticks oder Retries bis zum ersten
  Erfolg. `Ready` wird nicht auf Verdacht geändert.
- Timeout, späte Readbacks, Request-Zuordnung, Root-Isolation, Überschreiben und
  Fehler bei nicht verfügbarer Renderoberfläche bleiben abgesichert.
- Fenstergröße, Skalierung und Bildschirmzuordnung während der Session stabil
  halten. Die [Resize-/Kameralücke](diagnostics/window-schedules.md) ist ausdrücklich
  zurückgestellt, nicht Teil dieser Abnahme.
- `tests/blend_modes.py`, `tests/mesh_picking.py` und `tests/ui.py` mit verfügbarer
  Renderoberfläche ausführen, einschließlich Recording/Replay der UI.
  Bestehende Pixelprüfungen für die bekannten Fixtures bleiben erhalten.

#### Vorbereitung: UI-Overlay und mehrere Kameras

Die vorhandene `blend_modes`-Szene hat bereits zwei Kameras am primären Fenster:
3D mit Order 0 und eine darüber rendernde `Camera2d` mit Order 1 ohne Clear.
Es wird keine weitere Szene oder Steuerungsabstraktion ergänzt.

Am 2026-09-28 vorbereitet (Darwin arm64, Branch `reference/window`, Basis
`2e02815` plus bisherige Arbeitsbaumänderungen): `tests/blend_modes.py` prüft bei
jedem der sechs Captures die aktiven Kameras, identisches primäres Renderziel,
Zielgröße/Skalierung, Renderreihenfolge und fehlenden UI-Clear. Die UI-Bereiche
`controls` und `scene-display` werden über allgemeines Inspect aus ihren
physikalischen Layoutgrenzen gelesen. Helle neutrale Textpixel müssen in beiden
Bereichen vorkommen. Bei Alpha 1→0 bleibt die Controls-Maske gleich, die Status-
Maske ändert sich; bei Rückkehr zu Alpha 1 werden beide wiederhergestellt.
Die bisherigen 3D-Kugel-Pixelprüfungen verwenden dieselben PNGs, sodass beide
Kameraausgaben zusammen geprüft werden. Capture darf weder Szenenzustand noch
Kamerakonfiguration oder UI-Layout ändern. Dies ist eine technische Prüfung der
bekannten Fixture, kein allgemeiner Bildvergleich oder OCR.

Ohne Grafik bestanden:

```sh
python3 -m py_compile tests/blend_modes.py
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin blend_modes
cargo test --manifest-path bevy_test_apps/Cargo.toml --features slice --bin blend_modes
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin blend_modes -- -D warnings -A clippy::too_many_arguments
cargo fmt --manifest-path bevy_test_apps/Cargo.toml --check
git diff --check
```

Logs: `target/blend-modes-build/`. Die read-only Inhaltsprüfung fand keine
Probleme; der Reviewer konnte den angeforderten Diff jedoch nicht abgrenzen.

Der freigegebene Lauf `python3 tests/blend_modes.py` am 2026-09-29 auf Darwin
arm64, Branch `reference/window`, Basis
`2e028150f60637950acbddc80370fbf8bd6a0110` plus Arbeitsbaumänderungen scheiterte
bei Request #342 (`screenshots/hdr.png`) mit `screenshot_window_unavailable`:
`primary window has no render surface for this capture frame`.
Die vier vorherigen Aufnahmen `unlit-alpha-one`, `unlit-alpha-zero`,
`unlit-restored` und `lit` bestanden einschließlich Kamera-, Overlay-/Masken-,
Stillstands- und 3D-Pixelprüfungen. Für HDR wurde kein PNG geschrieben; der
Farbwechsel-Capture und die zweite Session wurden nicht mehr erreicht.
Dies ist ein fehlgeschlagener Gesamtlauf, keine vollständige Bildabnahme.

Evidenz: `target/blend-modes-ral3g3dh/commands.jsonl`, `server.log` und vier PNGs
unter `artifacts/2a9a922f5e9f38f5d1fc2c56fa810b29/screenshots/`.
Der Fehlerpfad bereinigte Server und Session; die anschließende Prozessprüfung
fand keine laufenden `woodpecker`-/`blend_modes`-Prozesse. Das Serverlog enthält
keinen zusätzlichen Surface-/Panic-Befund. Ob Verdeckung/Minimierung/Sperrung oder
ein anderer Renderzustand die fehlende Surface verursachte, ist damit nicht belegt.
Vor weiterem Grafiklauf Fensterzustand klären bzw. gezielt diagnostizieren;
kein unveränderter Retry bis zum ersten Erfolg und keine Abschwächung des Guards.

Der Nutzer meldete mögliche Verdeckung im ersten Lauf. Ein daraufhin freigegebener
Lauf unter der Vorgabe eines sichtbaren, unveränderten Fensters scheiterte am
2026-09-29 erneut, diesmal an geänderten Capture-Abmessungen:
`target/blend-modes-m1vthddx/commands.jsonl` belegt 640×360 für
`unlit-alpha-one` (#169) und 1280×720 für `unlit-alpha-zero` (#209).
Der Nutzer bestätigte anschließend, das Fenster während dieses Laufs verschoben
zu haben. Der erste Capture bestand; der zweite wurde gespeichert, scheiterte aber
an der unveränderten Größen-Erwartung. Die Größenänderung ist belegt, ein konkreter
Skalierungs-/Kamera-Fehler dadurch noch nicht diagnostiziert. Dieser Lauf ersetzt
keine Abnahme bei stabiler Fenstergeometrie und auch keine gezielte Resize-Abnahme.
Server und Session wurden bereinigt; keine laufenden Prozesse in der Nachprüfung.
Vor einem weiteren freigegebenen Lauf das Fenster weder bewegen noch verdecken,
minimieren oder zwischen Displays verschieben.

Nach erneuter Freigabe unter der Vorgabe eines sichtbaren, unveränderten Fensters
bestand `python3 tests/blend_modes.py` am 2026-09-29 auf Darwin arm64, Branch
`reference/window`, Basis `2e028150f60637950acbddc80370fbf8bd6a0110` plus
Arbeitsbaumänderungen. Alle sechs Captures bestanden einschließlich HDR- und
Farbwechsel-Aufnahme. Beide Kameras am primären Fenster (Order 0/1), UI-Grenzen,
Controls-/Status-Glyphen, Alpha-Änderung und Wiederherstellung sowie die 3D-Pixel
in denselben PNGs wurden geprüft. Capture ließ Szenenzustand, Kamerakonfiguration
und UI-Layout unverändert. Auch die zweite Session reproduzierte die beiden
Farbänderungen. Session-Stopps und Serverende verliefen regulär (Server-Exit 0);
die Nachprüfung fand keine laufenden `woodpecker`-/`blend_modes`-Prozesse.

Evidenz: `target/blend-modes-xz52actk/commands.jsonl` und `server.log`.
Sechs PNGs: `target/blend-modes-xz52actk/artifacts/2b0c9a2706c8c8ba69db564469e86e1a/screenshots/`.
Zusammen mit `target/mesh-picking-1vxjx8g6/` und `target/ui-qthjfz0o/` ist der
erste Screenshot-Checklistenpunkt abgeschlossen. Die früheren fehlgeschlagenen
Läufe bleiben erhalten; der erfolgreiche Lauf beweist nicht rückwirkend ihre
Ursache. Resize-/Skalierungswechsel, weitere Fehler-/Dateisicherheitsfälle und
die zurückgestellte vollständige Blend-/HDR-Validierung sind damit nicht erledigt.
Die lokalen Artefakte sind nicht im Git-Transfer enthalten.

#### Headless: Readback-Frist und Dateisicherheit

Am 2026-09-29 geprüft (Darwin arm64, Branch `reference/window`, Basis
`2e028150f60637950acbddc80370fbf8bd6a0110` plus Arbeitsbaumänderungen):
Die vorhandenen Capture-/Destination-Tests decken PNG-Inhalt, unveränderte virtuelle
Zeit, Surface-Ablehnung ohne Dateiersatz, framegebundene Verifikation, gültige
schwarze Bilder, Timeout, Fehler beim Encoding, Root-Isolation und sichere Pfade ab.

Ergänzt wurden:

- Ein verspäteter alter und ein doppelter neuer Readback dürfen die Request-ID
  oder den PNG-Inhalt des nächsten Captures nicht verändern. Das frühere Bild
  bleibt bei Timeout erhalten; nur die neue ID erhält genau einen Abschluss.
- Bereits laufendes Encoding/Schreiben bleibt nach Ablauf der Readback-Frist
  pending und liefert anschließend seinen korrelierten Writer-Fehler, keinen
  erfundenen GPU-Timeout. Der Test steuert den Worker-Kanal ohne 30 Sekunden Schlaf.
- Relative In-Root-Dateilinks einschließlich dangling Links werden am Zielnamen
  atomar ersetzt, nicht ihre verlinkten Dateien. Absolute Links bleiben auch bei
  In-Root-Ziel verboten; keine temporären Dateien bleiben liegen.
- Ein Encoding-Fehler erhält eine bereits vorhandene Bilddatei.

Ein neuer Regressionstest fand außerdem eine Produktlücke: `poll()` verarbeitete
bereits eingetroffene Bilder vor der Timeout-Prüfung. Ein nach Fristablauf
bereitliegender Readback konnte dadurch noch erfolgreich gespeichert werden.
`expired_readback_is_rejected_even_when_an_image_is_already_queued` war vor dem
Fix rot (unerwartetes `Completed`), nach dem Fix grün. Die Fristprüfung steht nun
vor der Readback-Übernahme; Writer bleiben ausgenommen. Kein Simulationstick,
keine verlängerte Frist und keine Wiederholung eines Captures wurden hinzugefügt.
Die Frist wird im Test über den gespeicherten Startzeitpunkt ausgelöst.

Befehle und Ergebnis:

```sh
cargo test --features screenshot --lib session::screenshot -- --test-threads=1
# 15 bestanden
cargo test --all-features --lib session::screenshot -- --test-threads=1
# 15 bestanden
cargo test --features screenshot --lib
# 106 bestanden
cargo test --no-default-features --lib session::screenshot
# 1 bestanden
cargo clippy --features screenshot --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

Logs unter `target/screenshot-validation/`: `expired-readback-red.log`,
`screenshot-tests-final.log`, `all-features-tests.log`, `library-tests.log`,
`no-feature-tests.log` und `clippy-final.log`. Die Headless-Tests verwenden kontrollierte Readback-/Worker-
Kanäle und echte PNG-/Dateioperationen, keine reale GPU oder Fensteroberfläche.
Die ergänzenden frischen CLI-Stillstands-/Pfadnachweise stehen bei
`logical_state` und `context_menu`. Resize und Skalierung sind davon nicht abgedeckt.

Die unabhängige Abdeckungsprüfung bestätigte den Timeout-Fix und fand keine weitere
Implementierungslücke. Ihre drei Hinweise sind abgearbeitet:

- Die bereits besprochene Fristregel steht nun auch im maßgeblichen Zielvertrag:
  30 Sekunden ab Aktivierung, Prüfung vor Readback-Übernahme, keine harte
  Response-Zustellfrist, keine Anwendung auf bereits laufende Writer.
- Der Late-Readback-Test erzwingt denselben Entity-Index mit anderer Generation
  über `despawn_no_free`/`spawn_empty_at`. Ein zunächst hinzugefügter bloßer
  Indexvergleich scheiterte, weil Bevys Allocator freie IDs zunächst puffert
  (`review-tests.log`); die Fixture kontrolliert die Slot-Wiederverwendung nun
  explizit und hängt nicht von zufälliger Allocator-Reihenfolge ab.
- Ein vor dem Dateiersatz geöffneter Reader liest danach weiterhin die alten
  Bytes, während ein neuer Open den vollständigen Ersatz sieht. Das würde bei
  In-Place-Überschreiben fehlschlagen und ergänzt die vorhandenen Inhaltsprüfungen.

Erneut bestanden: 15 Screenshot-Tests mit allen Features, 106 Library-Tests mit
Screenshot-Feature, Clippy und Format-/Diff-Prüfung. Finale Logs:
`target/screenshot-validation/review-tests-final.log`, `library-tests-final.log`
und `review-clippy.log`. Der zweite Screenshot-Checklistenpunkt ist damit
abgeschlossen; es wurden keine Grafiktests und keine Sleeps als Produktfix ergänzt.

#### Historischer Resize-Versuch — vollständig zurückgenommen

**Aktuelle Entscheidung:** Der Nutzer benötigt weder Fenstergrößen-/DPI- noch
Bildschirmwechsel. Diese Arbeit ist aus dem Abschlussumfang zurückgestellt, nicht
als bestanden abgehakt. Zurückgenommen sind Darstellungsaufbereitung und Layout-
Proxy, Feature-Erweiterung, Resize-Tests/Anbindung und die damit eingeführte
Beschriftungs-Schedule-Änderung der Blend-Szene. Die früheren stabilen Screenshot-
Abnahmen, Surface-Guard, Timeout-/Dateisicherheitsfixes und übrige Steuerung bleiben
unverändert erhalten. Der neue Text-/Scheduling-Blocker gehört zum entfernten
Versuch und ist kein offener Blocker des aktuellen Umfangs.

Code-/Dokumentationssnapshot vor Rücknahme: `target/resize-rollback-snapshot/`.
Frisch nach Rücknahme bestanden: 110 Library-Tests mit `screenshot,ui`, 92 ohne
Features, drei ursprüngliche Blend-Tests, Root-Clippy mit `-D warnings`, statische
Schedule-Probe, Python-Syntax-, Format- und Diff-Prüfung sowie CLI-/Blend-Slice-Builds.
Logs: `target/resize-rollback-validation/`. `preservation.log` vergleicht die
unberührten vorherigen EOF-/Screenshot-/Steuerungsdiffs mit dem Snapshot. Kein
Grafiklauf und kein Commit. Als Nächstes folgt Abschnitt 3, nicht weitere Resize-Arbeit.

Die folgenden beiden Unterabschnitte sind ausschließlich historische Nachweise;
ihre offenen Aufgaben, früheren Freigaben und Build-Meldungen gelten nicht mehr
als aktueller Auftrag. Erkenntnisse: [Diagnose](diagnostics/window-schedules.md).

##### Historisch: Headless reproduziert, nativer Versuch bereit

Am 2026-09-29 bestätigen zwei explizit gestartete, weiterhin rote Diagnose-Tests
veraltete Kamerazielgröße/Projektion bzw. Scale/Viewport im echten Control-Loop.
Zeit und Tickzähler bleiben stehen; ein expliziter Vergleichs-Warp aktualisiert
die Kameradaten. Die Tests sind als ungelöste Diagnosen standardmäßig ignoriert.
Kein Produktfix. Der anschließend freigegebene native Resize-Versuch
(`target/window-resize-b9ygon8k/`) bestätigt neue Fenster-/PNG-Maße 766×435
bei weiterhin alten Kameradaten 1280×720 ohne Tick. Alle drei PNGs enthalten Szene
und UI, deren Pixeldarstellung sich nach dem Vergleichs-Tick verändert. Versetzte/
abgeschnittene Beschriftungen danach verhindern eine vollständige Bildabnahme.
Native DPI-Änderung und weitere Ursachentrennung stehen aus; neue Grafik benötigt
frische Freigabe. Ablauf, Grenzen, Vorproben und Logs:
[Window-Diagnose](diagnostics/window-schedules.md#headless-reproduktion-und-vorbereiteter-nativer-versuch).
Der Resize-/Skalierungs-Checklistenpunkt blieb damals offen. Anschließend wurde der
Beschriftungsversatz nach dem Vergleichs-Tick headless isoliert: Berechnung in
Update vor Kameraaktualisierung. Der Szenenfix ordnet die Ankerberechnung nach
Kameraaktualisierung und vor UI-Vorbereitung innerhalb desselben Ticks ein;
Regression vorher rot, danach grün. Sechs Blend- und 106 Library-Tests bestanden.
Der anschließend freigegebene Grafiklauf `target/window-resize-5badl99o/`
(1280×720 → 686×720) bestätigt die korrigierte Beschriftungszuordnung nach dem
Vergleichs-Tick. Ohne Tick sind Szene/UI sichtbar horizontal gestaucht, während
Spielzustand und Zeit unverändert bleiben. Damit ist die verbleibende Produktlücke
klar sichtbar, nicht behoben. Keine Änderung am eingefrorenen Produkt-Scheduling.
Ein nativer DPI-Wechsel ist mangels bestätigter unterschiedlicher Faktoren der
vorhandenen Bildschirme derzeit offen.

##### Historisch: damals freigegebene Darstellungsaufbereitung ohne Tick

Die vorgeschlagene Trennung wurde vom Nutzer ausdrücklich genehmigt und im
damaligen Zielvertrag festgehalten (inzwischen ersetzt durch den
[aktuellen Screenshot-Umfang](target.md#screenshot)).
Die private Aufbereitung reagiert auf Fenstergrößen-/DPI-Änderungen und führt
gezielt Kamera-, Frustum-/CPU-Sichtbarkeits- und UI-Layout-/Glyphen-Built-ins aus,
keine Anwendungsschedules oder Eingabeverarbeitung. Eigene Beschriftungssysteme
bleiben tickgebunden. Headless-Regressionsnachweise, negative Kontrollen, Scope-
Grenzen und Review stehen in der [Diagnose](diagnostics/window-schedules.md#freigegebene-produktaufbereitung-headless-umgesetzt).

Bestanden: 114 Library-Tests mit Screenshot/UI, 92 ohne Features, vier UI-only-
Frozen-Window-Proben, sechs Blend-Tests, fünf Python-Harness-Tests und Clippy.
CLI-/Slice-Builds fertig. Der erste freigegebene Grafikversuch mit Produktfix
(`target/window-resize-l7hg2jk5/`) scheiterte vor Resize bei der Baseline:
`screenshot_window_unavailable`, keine Surface im Capture-Frame. Kein PNG und
keine automatische Wiederholung; Ursache offen, Prozesse beendet. Ein anschließend
vom Nutzer erlaubter Lauf mit bestätigter Sichtbarkeit (`target/window-resize-xf0cixf6/`)
zeigt bei 782×720 bereits ohne Tick aktuelle Kamera-/UI-Projektionen und ungestauchte
Kugeln/Schrift bei unveränderter Zeit. Nach dem Vergleichs-Tick fehlen jedoch der
linke Hinweistext und die gelben Beschriftungen. `pixel-review.json`: `not_accepted`.
Die Gesamtprüfung bleibt wegen dieses Textfolgefehlers sowie Scope-/DPI-Fragen offen.
Der Textfolgefehler wurde anschließend headless reproduziert (Textbreite 216 → 0)
und durch eine gemeinsame UI-Layout-Systeminstanz für Resize und normalen Tick
korrigiert. 115 Library-Tests sowie die neue Pixelprüfung gegen das gespeicherte
Fehlerbild bestanden. Das Scheduling-Review blockiert den Fix jedoch mit P1:
Direkte `.before/.after(ui_layout_system)`-Vorgaben binden nicht mehr an den
Layout-Proxy. Kompatibilität muss erhalten oder eine Einschränkung ausdrücklich
neu genehmigt werden; bislang liegt dafür keine Zustimmung vor. Erneute
Grafikabnahme steht ebenfalls aus; keine Zusatz-Ticks oder Capture-Retries. Details in der Diagnose,
Logs zusätzlich unter `target/ui-return-diagnosis/`.

### 3. Kombinierten Untersuchungsablauf abnehmen

Auf `tests/slice.py`, `tests/observation.py`, `tests/session.rs` und den
Server-Report-Tests aufbauen:

1. Session starten und Recording beginnen.
2. Inspect, Eingaben, explizite Ticks und Screenshots ausführen.
3. Einen kontrollierten Failure beobachten und den automatischen lokalen Report
   einschließlich unveränderlichem Snapshot und Markdown prüfen.
4. Client trennen und neu verbinden; angenommene Arbeit muss weiterlaufen.
5. Verwaltungs-Stopp bei aktiver Aufnahme durchführen. Tatsächliche JSONL-Datei,
   Footer, Session-Ende und Prozessbereinigung prüfen.
6. Recording in einer frischen Session mit bekanntem Ausgangszustand wiedergeben
   und den erwarteten Fehler der deterministischen Fixture reproduzieren.

Replay ist ein Command-Verlauf, kein World-Snapshot und kein allgemeiner
Output-Gleichheitsprüfer. Die Abnahme prüft die Reproduktion ihrer Fixture selbst.
Ergänzend aktive Wiedergabe, unerwartetes Ende und Recording-Abschluss gemeinsam
prüfen. Ein Session-Endevent bestätigt keinen vollständigen Report-Abschluss.

#### Kombinierter CLI-Nachweis: erster Lauf und korrigiertes Prüforakel

`tests/investigation.py` und `tests/fixtures/investigation.toml` verbinden die
vorhandene `logical_state`-Szene mit einem ausschließlich per `slice` und
`--investigation` aktivierten Fixture-Fehler: Der erste explizite Tick nach einem
aufgezeichneten B-Press erzeugt einen Tracing-Error; Halten wiederholt ihn nicht.
Die normale Szene und Produktinterfaces bleiben unverändert.

Vorbereitet sind zwei aufeinanderfolgende Sessions mit je fünf expliziten Ticks:
Recording, Inspect, drei Capture-Prüfungen einschließlich wartendem Input, ein
gepaceter Warp nach Trennung des einreichenden CLI-Clients, Failure/automatischer
lokaler Report mit History/Markdown, anschließende Commands und Verwaltungs-Stopp
bei weiterhin aktivem Recording. Der Harness prüft Footer, unveränderten Report,
abgeschlossenen Shutdown, Lifecycle `Ended` und Prozessbereinigung. In der frischen zweiten Session wird exakt
die erzeugte Recording wiedergegeben, erneut aufgenommen und derselbe Fixture-
Fehler anhand Zustand und Signatur geprüft. Replay-Outputs sind keine allgemeinen
Gleichheitserwartungen; die Bildprüfung ist fixturespezifisch.

Vor jedem der zwei Fenster muss der Mensch die Sichtbarkeit durch
`visible-1.json` bzw. `visible-2.json` im ausgegebenen Evidenzordner bestätigen.
Der Harness wartet höchstens 300 Sekunden und erzeugt dabei keine Ticks. Kein
Capture-Retry, keine Fokusmanipulation und keine dynamischen Fensterwechsel.
`cli.jsonl`, PNGs, Reports und Recording bleiben unter `target/investigation-*/`.
Ein `result.json` mit `acceptance: passed` wird nur nach vollständigem Erfolg erzeugt.

Headless bereits geprüft:

- Neuer öffentlicher Session-Test
  `unexpected_exit_during_replay_closes_recording_without_dispatching_the_tail`:
  unerwartetes Prozessende bei gleichzeitigem Replay/Recording, Blocked-Outcome,
  Failure/Ended, `unanswered`-Eintrag und `session_ended`-Footer, kein Dispatch des
  zweiten Warps und kein verbliebener Fixture-Prozess. Gezielter Lauf bestand.
- Fixture-Test erwartungsgemäß rot mit 0 statt 1 Failure, danach grün; beide
  `logical_state`-Tests bestehen. Das ist kein Beleg gerenderter Pixel.
- Bestehende headless CLI-Abnahme `python3 tests/slice.py` bestand mit vier Sessions;
  sie ergänzt insbesondere Verwaltungs-Stopp während aktivem Replay/Recording.
- Root-/Szenen-Clippy, Python-Syntax, CLI- und Slice-Builds bestanden.
- **Unvollständiger Gesamtlauf:** `cargo test --test session` wurde nach 200 Sekunden
  abgebrochen: 14 Tests bestanden, `pipe_drain_and_progress_do_not_need_polling`
  und `recording_keeps_protocol_failures_and_unanswered_commands_on_process_exit`
  hingen. Zwei verbliebene eigene Fixture-Prozesse wurden per SIGTERM bereinigt.
  Ursache ungeklärt, kein grüner Session-Gesamtnachweis und keine unveränderte
  Wiederholung dieses Laufs.

Logs: `target/investigation-validation/`.

Der anschließend genehmigte Grafiklauf `target/investigation-rhyjnmux/` brach nach
der ersten Session an einer falschen Harness-Erwartung ab: Verwaltungs-Stopp sollte
zusätzlich ein `session::Event::Ended` liefern. Der Zielvertrag sieht bei erfolgreichem
Shutdown ausdrücklich kein solches Event vor. Die gespeicherte Activity enthält den
abgeschlossenen `shutdown` und genau einen Lifecycle-Wechsel zu `Ended` ohne Fehler.
Keine Produktänderung und kein automatischer Grafik-Retry.

Die Auswertung der gespeicherten Daten bestätigt drei gleiche 640×360-Captures mit
grünem Fixture-Rechteck, unverbrauchtem wartendem Input, fünf expliziten Ticks,
einem Tracing-Failure samt lokalem Report und unverändertem Report-Snapshot in der
Activity, 18 Commands mit `stopped`-Footer und verschwundenem Spielprozess.
Die nachgelagerte Markdown-Bytegleichheitsprüfung wurde im ursprünglichen Lauf
nicht erreicht. Session 2/Replay wurde nicht gestartet; `review.json` hält deshalb
`acceptance: not_completed` fest. Server und Spiel sind beendet.

Das Orakel prüft jetzt Shutdown-Completion und Lifecycle `Ended` und weist ein
unerwartetes Session-Endevent zurück. Vier Tests in `tests/test_investigation.py`
bestehen nach nachgewiesenem Rot→Grün-Schritt; die korrigierte Prüfung besteht
auch gegen das gespeicherte CLI-Log. Künftige Läufe speichern zudem den ursprünglichen
Report-/Markdown-Hash in `report-snapshot.json`. Logs: `end-oracle-red.log`,
`end-oracle-green.log`, `saved-gui-review.log` unter `target/investigation-validation/`.

Danach war der korrigierte Harness **BUILD_READY**; Rust-Binaries unverändert.
Zu diesem Zeitpunkt war keiner der drei kombinierten Punkte abgehakt.

#### Zweiter Grafiklauf: erster Ablauf bestanden, Replay-Bildgröße abweichend

Der erneut ausdrücklich freigegebene Lauf `target/investigation-myo7u0cr/`
bestand in Session `41520c3235c12f7af8cb6fa3f9441a91` alle Prüfungen: fünf explizite
Ticks, drei gleiche 640×360-PNGs einschließlich Capture bei wartendem B-Press,
Fixture-Failure/automatischer lokaler Report, getrennte/wiederverbundene CLI-Clients,
Management-Stopp während Recording, 18 Commands mit `stopped`-Footer,
unveränderlicher Report samt Markdown, Lifecycle `Ended` und Prozessbereinigung.
Damit sind die ersten beiden kombinierten Checklistenpunkte abgenommen.

In der frischen Session `6270c5021cd92adc285460771e969eaf` schloss Replay ab und
reproduzierte fünf Ticks, einen Fixture-Failure und dieselbe Report-Signatur.
Alle drei neuen PNGs hatten jedoch **320×180 statt 640×360 Pixel**; die Capture-
Outcomes in `recordings/replayed.jsonl` bestätigen dieselben Maße. Das sichtbare
grüne Rechteck wird unten/rechts abgeschnitten. Die fixturespezifische Bildprüfung
brach daher ab, bevor der zweite reguläre Stopp-Prüfblock ausgeführt wurde.
Die Fehlerbereinigung beendete Server und Spiel und schloss die zweite Recording
mit 20 Commands und `stopped`-Footer. Nachträglich bestätigte Hash-Prüfungen beider
Markdown-Dateien und PID-Prüfungen stehen in `review.json`.

Die Ursache der Größenabweichung ist ungeklärt; keine rückwirkende Resize-/DPI-
Erklärung und kein Produktfix behauptet. Der Nutzer bestätigte beide sichtbaren
Fenster. Vor dem ersten Tick fehlte das grüne Rechteck; das ist nicht dieselbe
Abweichung, da das Layout erst durch den aufgezeichneten Warm-up-Tick aufbereitet wird.
Der Gesamtstatus bleibt `not_completed`, der dritte Checklistenpunkt offen.
Kein weiterer Grafiklauf ohne neue Freigabe; zunächst Fenster-/Bildschirmbedingungen
klären. Dynamische Resize-/DPI-Unterstützung bleibt zurückgestellt.

#### Historischer Bildschirmplatzierungsversuch — zurückgenommen

Die folgenden drei Unterabschnitte dokumentieren nur den Verlauf. Auf ausdrücklichen
Nutzerwunsch ist der Platzierungsumbau inzwischen vollständig zurückgenommen; seine
früheren Freigaben und BUILD_READY-Meldungen sind kein aktueller Auftrag.

##### Vorbereitung auf den ausgewählten Arbeitsbildschirm

Der Nutzer bestätigte danach, die Fenster auf einen anderen Bildschirm verschoben
zu haben, und beauftragte feste Ausgangsbedingungen auf diesem Arbeitsbildschirm.
Damit ist der vorige Größenvergleich kein Nachweis eines Replay-Produktfehlers.

`tests/investigation.py --monitor INDEX` verlangt nun einen nichtnegativen
Monitorindex und speichert eine gemeinsame `launch.toml` für beide Sessions.
Die reine Fixture-Option `--investigation-monitor=INDEX` setzt Bevy
`WindowPosition::Centered(MonitorSelection::Index(INDEX))` bereits vor der
nativen Fenstererstellung. Physische 640×360, Skalierungs-Override 1, keine
Größenänderbarkeit und kein Fokuswunsch bleiben erhalten. Ohne Option behält die
Szene ihre bisherige automatische Platzierung. Keine zusätzlichen Ticks, keine
laufende Darstellungsaufbereitung und keine Produktänderung.

Read-only macOS-Abfrage in derselben Active-Display-Reihenfolge wie winit:
Index 0 = Built-in Retina Display (Faktor 2), Index 1 = LG HDR WQHD (Faktor 1).
Beim Abfragen lag der Mauszeiger auf Index 1. Nach erneuter GUI-Freigabe ist
`python3 tests/investigation.py --monitor 1` vorgesehen. Fenster dort nur auf
Sichtbarkeit prüfen, nicht mehr verschieben. Falsche Platzierung erfordert
Abbruch/Konfigurationskorrektur, keine Reparatur während der Session.

Headless nachgewiesen: Monitor-Test erst rot (`Automatic` statt `Centered(Index(1))`),
danach drei Slice-Szenentests grün; Konfigurations-Test erst rot (fehlendes Argument),
danach sechs Python-Tests grün. Szenen-Clippy und Slice-Build bestanden.
Logs und Monitorliste: `target/investigation-validation/monitor-*`.
**BUILD_READY**, native Platzierung mit der neuen Option noch nicht geprüft;
Größen-/Pixelorakel bleiben unverändert, kein Grafiklauf ohne frische Freigabe.

##### Monitorindex beim nativen Start nicht aufgelöst — Vorbereitung blockiert

Der anschließend freigegebene Lauf `target/investigation-rsqgrazd/` bestand die
erste Session (`fed3c95bb08d4eb1c71736a108c257bb`) auf dem vom Nutzer bestätigten LG.
Das zweite Fenster (`295e1cb405a6c862d7cf1759b00901fe`) erschien laut Nutzer dagegen
auf dem MacBook. Kein Verschieben und kein `visible-2.json`: vor Replay abgebrochen.
Server und Spiel geordnet beendet, den noch auf Sichtbarkeit wartenden Harness
anschließend beendet; alle eigenen Prozess-IDs sind verschwunden. `review.json`
hält den Gesamtstatus `not_completed` fest.

`server.log` enthält bei **beiden** Fenstererstellungen
`Couldn't get monitor selected with: Index(1)`. Die erste passende Platzierung
belegt daher nicht, dass die Index-Auswahl funktioniert. Die Headless-Tests prüften
nur Konfiguration und Weitergabe, nicht die native Auflösung des Index.

Bevy 0.19.1: `bevy_winit/src/state.rs::resumed` erstellt initiale Fenster;
`create_monitors` befüllt die anfänglich leere `WinitMonitors`-Liste erst in
`about_to_wait`. `MonitorSelection::Index` greift auf diese Liste zu. Dieser
Initialisierungspfad passt zur protokollierten fehlgeschlagenen Auflösung.

Damit war die bisherige Vorbereitung nicht mehr BUILD_READY. Ein neuer Grafiklauf
benötigt eine begründete Änderung und frische Freigabe.

##### Korrektur: einmalige Fenstererstellung nach nativer Monitor-Erkennung

Auf erneuten Nutzerauftrag erstellt die Fixture bei gewähltem Monitor nun kein
initiales Fenster. `composition::rendered` akzeptiert dazu zusätzlich `None`;
bestehende Aufrufe mit `Window` behalten ihr Verhalten. Nur die Monitor-Fixture
registriert eine einmalige Fenstererstellung in `PreStartup`, nach Winit-Monitor-
Erkennung in `about_to_wait`. Sie prüft `WinitMonitors::nth(index)`, protokolliert
Index und nativen Namen und erstellt das primäre 640×360-Fenster mit unverändertem
Skalierungs-Override. Bei fehlendem Monitor erfolgt expliziter Abbruch, keine
Fallback-Platzierung. Ohne Option bleibt die ursprüngliche Startkonfiguration.

Keine neuen Produkt-Schedules, kein nachträglicher Resize und kein zusätzlicher
Simulationstick. Die vorhandenen fünf expliziten Ticks pro Session und die
Capture-/Pixelprüfungen bleiben unverändert.

Nachgewiesen: Test auf fehlendes initiales Fenster erst rot, danach fünf Slice-
Szenentests einschließlich explizitem Abbruch bei leerer Monitorliste grün;
drei Tests ohne Slice, sechs Python-Orakeltests, Szenen-Clippy, Compile-Checks
aller sechs gerenderten Bestandsszenen und CLI-/Slice-Builds bestanden.
Logs: `target/investigation-validation/deferred-window-*`.
**BUILD_READY**, aber tatsächliche native Platzierung noch nicht geprüft. Vor
`python3 tests/investigation.py --monitor 1` erneut GUI-Freigabe einholen und beide
Fenster auf dem LG bestätigen lassen; bei falscher Platzierung nicht verschieben.

#### Rücknahme der Bildschirmplatzierung und Rückkehr zum Replay-Nachweis

`target/investigation-6_knvh5c/` erreichte den ersten Sichtbarkeits-Wartepunkt mit
aufgelöstem Monitorindex und wurde dann auf Nutzerwunsch gestoppt, ohne Warm-up,
Capture oder Replay. Server, Spiel und Harness sind beendet. Der Nutzer beobachtete
einen Zusammenhang zwischen Platzierung und zuletzt angeklicktem Bildschirm und
beauftragte ausdrücklich, die Platzierungsarbeit zurückzunehmen.

Entfernt sind Monitoroption und erzeugte Launch-Konfiguration im Harness,
Fixture-Platzierungshelfer samt `PreStartup`-Erstellung und Monitor-Tests.
`composition::rendered` besitzt wieder seine ursprüngliche Schnittstelle.
Der opt-in Fixture-Failure, vier Endevent-Orakeltests, Report-Snapshot-Prüfung,
Recording-/Replay-Ablauf und strenge Bildgrößen-/Pixelprüfungen bleiben erhalten.
Archiv: `target/monitor-placement-rollback/`.
Frisch nach Rücknahme bestanden zwei Slice-Szenentests, vier Python-Orakeltests,
Szenen-Clippy, CLI-/Slice-Builds und Format-/Syntax-/Diff-Prüfung.
Logs: `target/monitor-placement-rollback/validation/`. **BUILD_READY**, kein neuer
Grafiklauf erfolgt.

Aktueller Auftrag ist wieder der dritte kombinierte Checklistenpunkt: vollständiges
Replay in frischer Session. Aufruf ohne Monitorparameter:
`python3 tests/investigation.py`, erst nach neuer GUI-Freigabe. Der Nutzer richtet
seinen Arbeitsbildschirm vorab selbst ein; Fenster während der Sessions nicht
verschieben. Keine automatischen Fokusänderungen, Reparatur-Ticks oder Bild-Retries.
Die ersten beiden kombinierten Abnahmen bleiben bestehen; der dritte Punkt bleibt
bis zum vollständigen Nachweis offen.

#### Vollständiger Kombinationslauf bestanden

Nach neuer GUI-Freigabe bestand `python3 tests/investigation.py` ohne Monitorsteuerung
vollständig: `target/investigation-ijz15dy0/result.json` enthält `acceptance: passed`.
Arbeitsbaum: `reference/window`, HEAD `2e028150f60637950acbddc80370fbf8bd6a0110`,
mit den uncommitteten Fixture-/Harness-/Session-Teständerungen und erhaltenen
vorherigen Änderungen. Die Monitor-/PreStartup-Experimente waren zurückgenommen.

- Session `467662cd5caf8537f31316e62b45f6cb`: Recording mit 18 Commands,
  fünf explizite Ticks, wartender Input ohne vorzeitigen Verbrauch, Inspect und
  drei 640×360-Captures, ein Fixture-Failure, automatischer lokaler Report samt
  History/Markdown, Fortschritt nach CLI-Trennung und Verwaltungs-Stopp.
- Frische Session `0e5b9d29cbb8ff6167596c11b22aaf22`: dieselbe Recording abgespielt;
  fünf Ticks, ein reproduzierter Fixture-Failure, gleiche Report-Signatur und drei
  zum ersten Lauf pixelgleiche 640×360-Captures. Die parallele erneute Aufnahme
  enthält 20 Commands und einen vollständigen `stopped`-Footer.
- Beide Report-Snapshots und Markdown-Dateien bleiben nach weiteren Commands und
  Stop unverändert. Shutdown-Completion, Lifecycle `Ended` und verschwundene
  Spielprozesse geprüft; der Server beendete sich erfolgreich. Nachträglich wurden
  Footer, PNG-Maße, Report-Hashes und Prozessabwesenheit erneut kontrolliert.
- Keine Capture-Retries, keine zusätzlichen Ticks oder Fokus-/Platzierungseingriffe.
  Der Nutzer bestätigte vor jedem Ablauf die Fenstersichtbarkeit.

Zusammen mit der oben dokumentierten headless CLI-Abnahme für aktives Replay/
Recording beim Verwaltungs-Stopp und dem gezielten Session-Test für unerwartetes
Ende während Replay/Recording sind nun alle drei kombinierten Punkte abgenommen.
Die beiden hängenden Tests des separaten Session-Gesamtlaufs bleiben ungeklärt;
dieser Erfolg ist weder ihr Fixnachweis noch eine bestandene Schlussprüfung.
Historische fehlerhafte/abgebrochene Läufe bleiben erhalten.

### 4. Last, Ergebnislücken und Shutdown prüfen

- Die vorläufige Activity-Grenze von 4 MiB mit echten großen Inspect-Werten,
  Reports und langsamen Clients bemessen. Übergröße und Eviction bleiben sichtbare
  Lücken, keine gekürzten Ergebnisse oder behaupteten Erfolge.
- Bekannte Ergebnisse von unbekannten Ausgängen unterscheiden. Eine nicht
  bestätigte Einreichung nach Verbindungsverlust wird nicht automatisch wiederholt.
- Die unbeschränkte wartende Report-Queue je Session bei langsamen Providern und
  anhaltenden Failures messen. Begrenzte fertige Activity begrenzt diese Queue
  nicht. Eine Überlastregel mit Auswirkungen auf den Vertrag vor Umsetzung vorlegen.
- Gleichzeitige Sessions, aktive Warps/Replay/Recording, verspätete Reports und
  Activity-Lücken unter derselben Server-Shutdown-Frist prüfen. Ein Sessionfehler
  darf andere Sessions nicht blockieren; alle eigenen Prozessgruppen bereinigen.
- Ein separat mit `panic=abort` gebautes Programm abnehmen. Ein expliziter
  Prozessabbruch im bisherigen Fixture ersetzt diesen Build-Nachweis nicht.

Nicht unterbrechbare lokale Datei-I/O bleibt kooperativ abbrechbar. Eine Frist
begründet keine garantierte rechtzeitige erfolgreiche Speicherung. Unterbrochene
Remote-Aufrufe können bereits gewirkt haben und müssen als unbekannt gelten können.

#### Activity-Grenze mit echten großen Ausgaben abgenommen

`tests/activity.rs` prüft die öffentliche Server-/Client-Schnittstelle mit echtem
Bevy-Prozess `tests/fixtures/activity.rs`, Reflection und lokalem Report-Provider.
Keine reduzierte Testgrenze: 4.194.304 serialisierte Activity-Bytes, wie im CLI-Default.
Arbeitsbaum `reference/window` auf HEAD `2e028150f60637950acbddc80370fbf8bd6a0110`
mit den uncommitteten Fixture-/Teständerungen; keine Produktimplementierung geändert.

- Ein reflektierter 5-MiB-String überschreitet die Grenze als einzelnes Ergebnis.
  Activity liefert `Gap` mit unbekanntem Ausgang, weder gekürzte Daten noch eine
  behauptete Completion. Die separate Recording enthält den vollständigen String.
  Leere `Snapshot.pending` wird ausdrücklich nicht als Erfolg interpretiert.
- Drei vollständige 2-MiB-Antworten werden von einem schnellen Client empfangen;
  ein parallel verbundener, verzögert pollender Client erhält danach eine Lücke.
  Das älteste Outcome fehlt, das noch gespeicherte Ergebnis bleibt vollständig.
- Ein zusätzlicher WebSocket mit kleinem Empfangspuffer fordert eine große Antwort
  an und liest sie nicht. `peek` bestätigt begonnene Antwortübertragung ohne
  Konsumieren; der andere Client kann weiterhin Inspect/Snapshot abrufen. Das ist
  ein begrenzter Nichtleser-Nachweis, keine unbegrenzte Sättigungs-/Speichergarantie.
- Ein expliziter Tick erzeugt einen lokalen Report aus der tatsächlichen großen
  Inspect-History. Markdown: 11.540.769 Bytes im finalen Lauf, darin der vollständige
  5-MiB-Wert. Das übergroße Report-Activity-Ergebnis erzeugt wiederum `Gap`, kein
  gekürztes oder erfundenes Report-Ergebnis. Session bleibt Ready/steuerbar;
  Counter bleibt bis zum einen expliziten Tick bei 0 und steht danach bei 1.
- Verwaltungs-Stopp, verschwundener Prozess und erfolgreicher Serverabschluss
  werden vor Schreiben des Erfolgsartefakts geprüft.

Befehle:

```sh
cargo build --example activity_fixture
cargo test --features client --test activity -- --nocapture
cargo test --features cli --lib cli::script::tests
cargo clippy --features cli,screenshot,ui --all-targets -- -D warnings
```

Der Test schlug ohne die große reflektierte Resource an der erwarteten Gap-Prüfung
fehl; mit der Fixture bestand er. Nach zusätzlicher Absicherung des Nichtleser-
Starts und des Serverabschlusses bestand der finale Lauf in 3,20 Sekunden:
`target/activity-load-1337/result.json`, Session `3d1f638b48f1c841922fc0e324890175`.
Acht Script-Tests bestehen ebenfalls, einschließlich Activity-Lücken und fehlender
Submit-Bestätigung ohne automatischen Retry. Clippy, Format und Diff-Prüfung bestanden.
Logs: `target/activity-validation/`. Kein Grafiklauf und keine Überlastregel ergänzt.

Queue-Bemessung, Mehrsession-Shutdown und `panic=abort` siehe unten.
Die unbeschränkte Report-Queue bleibt eine dokumentierte Betriebsgrenze. Die beiden zuvor
hängenden Session-Tests sind durch diesen gezielten Lauf nicht als behoben anzusehen.

#### Report-Queue bemessen; Betriebsgrenze dokumentiert

`src/server/report_load_tests.rs` prüft mit einem isolierten Fake-`gh` (keine echte
GitHub-Anfrage) und dem headless `activity_fixture --continuous-failures` 64
aufeinanderfolgende explizite Ticks mit jeweils einem neuen Tracing-Failure.
Der erste Provider-Aufruf bleibt bis zur ausdrücklichen Barrier-Freigabe blockiert.
Weitere Tick-Completions, Failure-Events und leere Pending-Snapshots bleiben erreichbar;
kein Report-Ergebnis wird vor Freigabe behauptet. Ein Provider ist aktiv, zuletzt
63 weitere Reports noch nicht abgeschlossen. Nach Freigabe werden alle 64 Reports
mit erfolgreichem Provider-Ergebnis abgearbeitet; die History-Snapshots enthalten
vertraglich höchstens 50 Commands. Session-/Serverabschluss und Providerbereinigung
bestanden. Keine Queue-/Provider-Produktimplementierung geändert.

Finale Evidenz: `target/report-queue-load/fc84e5616ddb13bb1d149203eacaa757/result.json`,
Session `c02d7f464d49838c8eac79ca8a8cc096`, Laufzeit 3,00 Sekunden.

| Failures | Aktiver Provider | Noch nicht abgeschlossene weitere Reports | Server-RSS (KiB) |
| ---: | ---: | ---: | ---: |
| 1 | 1 | 0 | 17.376 |
| 16 | 1 | 15 | 17.936 |
| 32 | 1 | 31 | 18.608 |
| 64 | 1 | 63 | 20.400 |

Die 64 Report-Snapshots serialisieren zusammen zu 393.725 Bytes; einzeln von 1.046
bis 9.425 Bytes, danach begrenzt die 50-Command-History ihre Größe in dieser Fixture.
RSS wächst um 3.024 KiB, umfasst aber auch Activity, Session-History und Allocator;
es ist ausdrücklich keine isolierte Queue-Heap-Messung. Der Test verwendet 64 MiB
Activity-Retention, um die Messereignisse vollständig zu erhalten. Das ist **keine**
Änderung am CLI-Default und keine Queue-Grenze. Die 4-MiB-Activity-Prüfung bestand
separat erneut unter `target/activity-load-27855/`.

Befehle und Logs:

```sh
cargo build --example activity_fixture
cargo test --features server --lib server::tests::blocked_provider_accumulates_failure_snapshots_without_blocking_session -- --exact --nocapture
cargo test --features client --test activity -- --nocapture
cargo clippy --features cli,screenshot,ui --all-targets -- -D warnings
```

Logs `target/report-queue-validation/`; Clippy, Format-/Diff-Prüfung bestanden.
Der erste Versuch `target/report-queue-load/b4b965be818278897d9abe4b9e79e1f6/`
bleibt fehlgeschlagen: das Harness erwartete ab Report 51 fälschlich mehr als 50
History-Commands. Nur dieses Orakel wurde gemäß bestehendem Vertrag korrigiert.
Kein Grafiklauf, keine automatische Wiederholung des unveränderten Fehlversuchs.

**Befund:** `Reports` verwendet einen unbeschränkten `mpsc::channel<Report>` und einen
Worker pro Session. Die 50-Command-History und Activity-Retention begrenzen nicht
die Zahl wartender Report-Kopien. Bei langfristig schneller eintreffenden Failures
als Provider-Abschlüssen wächst die Queue ohne Grenze; große Inspect-Outcomes
vergrößern zusätzlich jede Kopie. Der begrenzte Versuch belegt Abarbeitung und
Reaktionsfähigkeit, nicht sicheren Dauerbetrieb oder Sättigung unter beliebiger Last.

**Optionaler späterer Vorschlag, nicht umgesetzt:** Eine konfigurierbare
nichtblockierende Report-Queue-Grenze pro Session, nach Zahl **und** Gesamtgröße
bemessen. Bei erschöpftem Budget bleibt das Failure beobachtbar; der zugehörige
Report wird ausdrücklich als nicht zur Submission angenommen gemeldet, nicht
still verworfen und nicht als erfolgreich oder extern unbekannt bezeichnet.
Laufende Provider-Aufträge bleiben unangetastet. Keine Blockierung der Spiel-Pipes,
kein heimliches Dedup und kein automatischer Retry. Konkrete Budgets sowie die
persistierte Nachweisform bei Activity-Eviction benötigen ebenfalls Zustimmung.
Alternative: unbeschränkte Queue bewusst als dokumentierte Betriebsgrenze behalten.
Für den aktuellen Abschluss wird keine neue Überlastregel ergänzt: nach der
Besprechung folgt die Shutdown-Abnahme statt vorsorglicher Queue-Optimierung.
Die unbeschränkte Queue bleibt dokumentierte Betriebsgrenze, nicht als dauerhaft
überlastsicher abgenommen. Der obige Vorschlag bleibt optional für späteren Bedarf.

#### Mehrsession-Shutdown unter laufender Arbeit bestanden

Der vorhandene headless Test
`server::tests::slow_reports_outlive_sessions_and_share_server_deadline_and_forced_cleanup`
bestand zunächst unverändert. Anschließend wurde er für den vollständigen Nachweis
ergänzt: zwei offene, gepacete Warps mit je 1.000 angeforderten Ticks während zweier
blockierter Provider-Aufträge; explizite Spiel-PIDs und Prüfung der Provider-
Unterprozesse. Fake-`gh` nur im isolierten Testprozess, keine echte GitHub-Anfrage.
Die gemeinsame Serverfrist beträgt 800 ms. Erneutes normales Stoppen verlängert
sie nicht; Session-Lifecycle `Ended` allein beendet wartende Report-Arbeit nicht.

Der erweiterte Test bestand in allen drei Phasen:

| Phase | Zeit ab Stop | Report-Ergebnis | Aktive Warps |
| --- | ---: | --- | --- |
| Geordnet, Provider nach Stop freigegeben | 35 ms | `submitted` | Beide `stopped`, je 1 von 1.000 Ticks |
| Gemeinsame Frist abgelaufen | 808 ms | `interrupted` | Beide `stopped`, je 1 von 1.000 Ticks |
| Explizit erzwungener Abbruch | 31 ms | `interrupted` | Beide `stopped`, je 1 von 1.000 Ticks |

Report-Snapshots bleiben unverändert und enthalten den ursprünglichen Command vor
den späteren Ticks/Warps. In den Abbruchphasen startet kein lokaler Fallback nach
Cancellation; der Serverabschluss meldet den unvollständigen Ausgang statt Erfolg.
Die terminalen Report-Ergebnisse stehen nach dem Session-Lifecycle-Ende in Activity.
Alle offenen Commands sind aufgelöst. Spiel-, Provider- und pipehaltende Kindprozesse
sind nachweislich verschwunden (`kill(pid, 0)` / `ESRCH`); die gespeicherten PIDs
wurden anschließend unabhängig erneut geprüft. Keine Produktänderung.

Evidenz: `target/server-report-tests/3b463ad9a5a57c8b4160b8954b4a9db3/`, mit
`graceful/result.json`, `deadline/result.json`, `force/result.json` sowie isolierten
stdout-/stderr-Logs. Gesamtzeit 5,58 Sekunden. Arbeitsbaum `reference/window`,
HEAD `2e028150f60637950acbddc80370fbf8bd6a0110` plus uncommittete Änderungen.

```sh
cargo build --example observation_fixture
cargo test --features server --lib server::tests::slow_reports_outlive_sessions_and_share_server_deadline_and_forced_cleanup -- --exact --nocapture
cargo clippy --features cli,screenshot,ui --all-targets -- -D warnings
```

Logs `target/multisession-validation/`; Clippy, Format-/Diff-Prüfung bestanden.
Kein GUI-Lauf. Der gezielte Test prüft den Server-Core; Listener/HTTP-Signalwege
und der vollständige zuvor teilweise hängende Session-Testlauf sind damit nicht
pauschal neu abgenommen. Die separate `panic=abort`-Abnahme folgt unten.

#### Separater echter `panic=abort`-Build bestanden

`tests/fixtures/panic_abort/Cargo.toml` ist ein separates Cargo-Paket mit
`[profile.dev] panic = "abort"`, eigenem Lockfile und eigenem Build-Verzeichnis.
Der Binary-Einstieg verweigert per `compile_error!` jeden Build ohne
`cfg(panic = "abort")`. Er nutzt die reale Bevy-Observation-Fixture im Modus
`panic`, nicht deren expliziten `process::abort()`-Modus. Der vorhandene Panic-Hook
begrenzt Core-Dumps auch bei echter Abort-Strategie, schreibt den Marker und ruft
den vorherigen Hook auf; Rusts konfigurierte Panic-Strategie beendet das Programm.

`tests/panic_abort.rs` prüft mit der öffentlichen Session-/Report-API:

- Ready, dann genau ein expliziter Tick, der einen normalen `panic!` auslöst.
- Genau ein Panic-Failure mit Nachricht, Quellstelle und erfasstem Backtrace,
  anschließend Ended mit `signal: 6 (SIGABRT)`; kein doppelter ProcessExit-Failure.
- Der offene Tick endet mit `session::Error::Ended`, nicht mit einer Completion.
  Der Report-Snapshot enthält den einen Command als `unanswered`.
- Der vorherige Hook wurde genau einmal aufgerufen. Nach Session-Ende ist lokales
  Report-Submit erfolgreich; Markdown stimmt vollständig mit dem unveränderten
  Snapshot überein. Der Spielprozess ist verschwunden (`ESRCH`).

Finaler Lauf: `target/panic-abort-acceptance-78733/result.json`, 1,27 Sekunden.
Der erste bestandene Lauf unter `target/panic-abort-acceptance-73325/` bleibt
zusätzliche Evidenz; sein gespeicherter Prozess wurde nochmals unabhängig geprüft.
Arbeitsbaum `reference/window`, HEAD `2e028150f60637950acbddc80370fbf8bd6a0110`
plus uncommittete Fixture-/Teständerungen; keine Session-/Report-Produktänderung.

```sh
cargo build --manifest-path tests/fixtures/panic_abort/Cargo.toml --locked
cargo test --test panic_abort -- --nocapture
cargo build --example observation_fixture
cargo test --test observation panic_and_abort_are_drained_before_ended_without_a_second_process_exit_failure -- --exact --nocapture
cargo clippy --features cli,screenshot,ui --all-targets -- -D warnings
cargo clippy --manifest-path tests/fixtures/panic_abort/Cargo.toml -- -D warnings
```

Logs `target/panic-abort-validation/`. Der erste separate Build scheiterte an der
fehlenden direkten `tracing`-Dependency der eingebundenen Fixture; diese wurde
ergänzt, anschließend Build und Abnahme bestanden. Keine unveränderte Wiederholung.
Der bestehende gezielte Observation-Test für Panic, expliziten Abort, Exit und
Markerfehler bestand ebenfalls (6,67 Sekunden); beide Clippy-Prüfstände, Format
und Diff-Prüfung bestanden. Kein GUI-Lauf. Alle vier gezielten Last-/Fehlerpunkte
sind damit abgeschlossen, aber die Schlussprüfung samt ungeklärtem Session-
Gesamtlauf bleibt offen.

### 5. Vertrags- und Dokumentationsabgleich abschließen

Die Tests wurden nach dem Commit neu geordnet; aktuelle Pfade und Befehle sowie
Behalten-/Zusammenlegen-Entscheidungen stehen in [tests/README.md](../../tests/README.md).
Historische Befehle und Dateinamen in den folgenden Nachweisen bezeichnen bewusst
den damaligen Prüfstand. Source-Links und aktuelle Bedienbefehle verwenden die neue
Ordnung. Grafikläufe nach dieser Neuordnung sind noch nicht abgenommen.

Öffentliche Requests, Capabilities, Fehlerformen und Feature-Kombinationen gegen
`target.md` prüfen. Root- und Test-App-Paket getrennt bauen und testen;
`bevy_test_apps` ist kein Workspace-Member. Die Root-Untergrenze `0.19.0` und die
Test-App-Anforderung `0.19.1` werden durch die Lockfiles auf `0.19.1` aufgelöst.

Die vollständige Rust-Suite, Feature-Builds und CLI-Abnahmen aus
[usage.md](usage.md#nachweise) ausführen. Grafische Tests nacheinander nach
Freigabe. Ergebnisse müssen Commit/Arbeitsbaum, Plattform und Artefaktpfade
benennen. Historische grüne Läufe ersetzen diese Abschlussabnahme nicht.

Anschließend Status, Grenzen und Links abgleichen. Die Bedienung bleibt in
`usage.md`, die Abdeckung hier, die technischen Befunde unter `diagnostics/`.

#### Schlussprüfung: aktueller Headless-Stand

Bisheriger Abnahmestand auf ausdrücklichen Auftrag committed:
`eeeb8d0a86147067a4e92a9cf0b9628b4c6e0a8b`
(`test: complete stable-window investigation and failure acceptance`). Die eigenen
unversionierten Assets, `alien_cake_addict.rs`, `docs/api/image.png` und npm-
`package-lock.json` blieben außerhalb des Commits. Darauf wurden die folgenden
Checks ausgeführt, damals ohne GUI-Start oder Produktänderung. Diese Tabelle hält
den historischen HEAD-Prüfstand fest; die nachfolgende Fortschreibung beschreibt
Testneuordnung, Clippy-Bereinigung und neue Grafikläufe im damals uncommitted Arbeitsbaum.
Plattform Darwin arm64, Rust/Cargo 1.97.1.
Logs, begrenzte Run-Ergebnisse und Zusammenfassung: `target/final-validation/`.

| Prüfung | Aktueller Ausgang |
| --- | --- |
| Früher hängende Session-Tests einzeln | Beide bestanden: Pipe-Fortschritt 0,34 s, Recording-/Prozessende 0,37 s |
| `cargo test --all-features --lib --tests -- --test-threads=1` | 141 Library-Tests und 25 Integrationstests bestanden: Activity 1, Observation 7, Abort 1, Session 16 |
| `cargo test --no-default-features --lib` | 92 bestanden |
| Einzelne Features `ui`, `screenshot`, `server`, `client` | Jeweils `cargo check --no-default-features --features …` bestanden |
| CLI-Build | `cargo build --features cli` bestanden |
| Root-Doctests | `cargo test --all-features --doc` bestanden |
| Separates Bevy-Paket | `cargo test --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets`: 10 Tests bestanden |
| Bevy ohne Slice | `cargo check --manifest-path bevy_test_apps/Cargo.toml --no-default-features --all-targets` bestanden |
| Headless CLI | `slice.py`, `observation.py`, `script.py`, `repl.py`, `shutdown.py` alle bestanden |
| Python-Orakel | 7 Unittests bestanden |
| Root-Clippy | `cargo clippy --all-features --all-targets -- -D warnings` bestanden |
| Bevy-Clippy | **Fehlgeschlagen**, siehe unten |
| Builds für weitere grafische Abnahme | `cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bins` bestanden: **BUILD_READY**, noch keine neue GUI-Freigabe |
| Format und Diff | Root-/Bevy-Format und `git diff --check` bestanden |

Die beiden früher hängenden Session-Tests sind im aktuellen Einzel- und gesamten
seriellen Prüfstand grün. Die Ursache des früheren Hängers ist damit nicht ermittelt;
keine Behauptung eines ursächlichen Produktfixes oder bestandener paralleler Suite.
Der zuvor ebenfalls auffällige Report-Starttest ist im vollständigen Library-Lauf
mitgeprüft. Alte Fehlerlogs bleiben erhalten. Bei keinem aktuellen Run war ein
Timeout oder automatischer Retry nötig.

Vertragsabgleich: alle 18 Command-Namen, zugehörige Request-/Output-Typen und die
versiegelte Request-Zuordnung; Spielprotokoll v3/äußerer Codec v1; strikte Ready-
Capabilities und Renderer-abhängiges Screenshot-Angebot; öffentliche Session-
Fehlergruppen und stabile fachliche Codes; Recording-/Replay-/Script-Formate sowie
Feature-Grenzen. Die entsprechenden Codec-, Reflection-, Input-, Screenshot-,
Recording-, Replay-, Netzwerk- und Script-Tests sind im grünen Root-Lauf enthalten.
Im Zieltext fehlten bisher die ausdrücklichen Namen `replay.start`/`replay.stop`;
die bestehende Kodierung samt vier Session-eigenen Command-Outputs wurde als
zusätzliche Tabelle dokumentiert, ohne Verhalten oder Umfang zu ändern.

Der zusätzliche strenge Bevy-Clippy-Lauf findet sechs bestehende Lints:
`too_many_arguments` in Blend/Game-Menu, `type_complexity` in Mesh-Picking und zwei
Game-Menu-Queries sowie `field_reassign_with_default` im Blend-Test. Diese drei
Quelldateien sind gegenüber `2e02815` unverändert. Log `bevy-clippy.log`, Status 101;
keine Suppression, kein Abschwächen und keine ungefragte Handler-Optimierung.
Dieser Lauf ist **nicht bestanden** und bleibt als Schlussprüfungsbefund sichtbar.

**Fortschreibung nach Freigabe:** Die sechs Befunde sind gezielt bereinigt:
benannte Query-Typen, zusammengehörige Systemparameter als Tupel und direkte
`SceneState`-Initialisierung. Keine Lint-Suppression, kein zusätzlicher Tick und
keine Änderung von Schedules, Filtern oder Fensterverhalten. Der erneut ausgeführte
strenge Gesamt-Clippy-Lauf besteht. Alle 10 Bevy-Tests, der All-Targets-Check ohne
Features und Format-/Diff-Prüfung bestehen ebenfalls. Logs:
`target/clippy-cleanup/{before,after,tests,no-features,format-final}.log`.
Der historische fehlgeschlagene Lauf wird dadurch nicht nachträglich grün.

#### Grafische Schlussprüfung: 7/7 bestanden

Auf erneute ausdrückliche Freigabe wurden CLI und alle Slice-Binaries gebaut,
**BUILD_READY** gemeldet und die sieben aktuellen Python-Module nacheinander
geprüft. Prüfstand während der Läufe: Branch `reference/window`, HEAD `eeeb8d0` plus uncommitted
Testneuordnung und gezielte Clippy-Bereinigung; Darwin arm64 / Bevy 0.19.1.
Keine Fenster-/Displayänderung, kein Fokus-Eingriff und kein automatischer Retry.

| Abnahme | Neue Evidenz |
| --- | --- |
| Context Menu | `target/ui-1ub9sma6/` |
| Logical State | `target/logical-state-2u8a5477/` |
| Game Menu | `target/game-menu-v3zgtc2y/` |
| UI Drag & Drop | `target/ui-drag-drop-bktjf_kn/` |
| Mesh Picking | `target/mesh-picking-x1jxbqat/` |
| Blend Modes | `target/blend-modes-pztbse9e/` |
| Investigation | `target/investigation-dkaj5b6i/` |

Die erste Investigation-Ausführung `target/investigation-xld_z4qa/` scheiterte an
der fehlenden Sichtbarkeitsbestätigung innerhalb von 300 Sekunden vor dem ersten
Tick. Sie bleibt fehlgeschlagen, inklusive Log und Artefakten. Erst nach erneuter
Nutzerfreigabe wurde genau ein neuer Versuch gestartet. Der Nutzer bestätigte
beide Fenster getrennt; die Gates wurden nicht automatisch übergangen.

Der neue Versuch bestand mit zwei Sessions, je fünf expliziten Ticks, sechs
pixelgleichen 640×360-PNGs, derselben Failure-/Report-Signatur, unveränderlichen
Reports und gestoppten Recordings mit 18 bzw. 20 Commands. Die kopierte Quelle
enthält ebenfalls 18 Commands. Recording-SHA256:
`cb6f80aa3b3b099f3c817ed45d61c11a08fa1b8e7dd10646b9f5a4d747ff328c`.
Beide Fixture-PIDs wurden anschließend unabhängig als nicht mehr vorhanden geprüft;
keine eigenen Spiel-/Serverprozesse bleiben aktiv.

Maschinenübersicht samt allen Evidenzpfaden, historischem Fehlversuch und Cleanup:
`target/graphical-final-validation/summary.json`. Bounded Runner-Ergebnisse und
Logs: `target/final-validation/graphical-final-*.{json,log}`; Builds unter
`target/graphical-final-validation/`. Damit sind die Schluss-Checkboxen für den
aktuellen Umfang erfüllt. Dynamische Resize-/DPI-/Display-Wechsel bleiben
zurückgestellt; die unbeschränkte Report-Queue bleibt dokumentierte Betriebsgrenze.
Diese Fortschreibung wird auf ausdrücklichen Nutzerauftrag zusammen mit Testordnung
und Clippy-Bereinigung versioniert; lokale `target/`-Evidenz bleibt außerhalb von Git.

## Abdeckungsmatrix

CLI-Abnahmen verwenden CLI, HTTP/WebSocket, Session und Bevy. Native Tests
prüfen zusätzliche Fälle. Die Tabelle beschreibt vorhandene Testabdeckung;
konkrete historische Ergebnisse stehen in
[next-steps.md](next-steps.md#letzter-dokumentierter-prüfstand) und den
[Screenshotnachweisen](diagnostics/screenshot-evidence.md).

| Bereich | Vorhandener Nachweis | Noch offen im Abschluss |
| --- | --- | --- |
| `counter` | [session_lifecycle.py](../../tests/acceptance/headless/session_lifecycle.py): Ticks, Stillstand, Pace/Stop, Isolation, Recording/Replay und Verwaltungs-Stopp. | Kombinierter Lifecycle und Last bestanden; Schlussprüfung im aktuellen Umfang abgeschlossen. |
| `context_menu` | [context_menu.py](../../tests/acceptance/rendered/context_menu.py): virtuelle Inputs, Fokus, Unicode-Werte und Grenzen, Menüersetzung/-auswahl, Itemlayout, tote Teilbaum-Handles, Bilder, Recording/Replay und zwei Sessions. | Keine zusätzliche Steuerungsvariante. |
| `logical_state` | [logical_state.py](../../tests/acceptance/rendered/logical_state.py): Update, FixedUpdate, Timer, Keyboard- und Pointer-Press/Hold/Release, Stillstand und fünf PNGs einschließlich vorgemerkter Inputs. | Keine zusätzliche Steuerungsvariante. |
| `game_menu` | [game_menu.py](../../tests/acceptance/rendered/game_menu.py): Navigation, Einstellungen, Timer, Klicks, Hierarchien, tote Handles, Keyboard-Kurzwege, neun PNGs und Quit mit Failure/Report sowie Session-Isolation. | Keine zusätzliche Steuerungsvariante. |
| `ui_drag_drop` | [ui_drag_drop.py](../../tests/acceptance/rendered/ui_drag_drop.py): gültiger/ungültiger Drop, Zwischenposition, Drag-Phasen, Belegung, Darstellung, Despawn während Drag, tote Handles/Hierarchie, erneuter Drag und acht PNGs. | Keine zusätzliche Steuerungsvariante. |
| `mesh_picking` | [mesh_picking.py](../../tests/acceptance/rendered/mesh_picking.py): Hover/Press/Release/Out aller Meshes, horizontaler/vertikaler Würfel-Drag, diagonale Kugel-/Zylinder-Drags, vollständige Rotation/Isolation, 48 Ticks und 19 PNGs. | Keine zusätzliche Steuerungsvariante. |
| `blend_modes` | [blend_modes.py](../../tests/acceptance/rendered/blend_modes.py): zwei Sessions, gehaltene Tasten, Orbit, Alpha-Grenzen, HDR/Unlit, Materialidentität, reproduzierbare Farben und sechs PNGs. | Keine zusätzliche Steuerungsvariante; vollständige Blend-/HDR-Validierung ist zurückgestellt. |
| Inspect | [Entity-Tests](../../src/session/inspect/entities/tests.rs), Reflection-Matrix sowie Game-Menu-, Context-Menu- und Drag-Lebenszyklen mit toten Handles. | Lastnachweis bestanden; keine neue Inspect-Architektur. |
| Recording/Replay/Reports | [session.rs](../../tests/integration/session.rs), [automatic_reports.py](../../tests/acceptance/headless/automatic_reports.py), [Report-Tests](../../src/server/report_tests.rs). | Kombinations-, Last-/Shutdown-, Abort- und frische grafische Nachweise bestanden; Schlussprüfung abgeschlossen. |
| CLI/REPL/Script | [repl.py](../../tests/acceptance/headless/repl.py), [script.py](../../tests/acceptance/headless/script.py), Netzwerkfixtures. | Große Outputs/Activity und langsame Clients geprüft; frische headless CLI-Nachweise bestanden. |

### Reflection-Matrix

Die [gemeinsamen Reflection-Fixtures](../../src/session/inspect/reflection_tests.rs)
prüfen Resource- und Component-Werte ohne Mutation oder Tickfortschritt:

- exakte registrierte Type Paths und Fehler bei unbekannten Eingabepfaden;
- `Missing`, `NotRegistered`, `NotReflectable` und `NotSerializable`;
- Metadatenzugriff ohne Wertserialisierung;
- opake Typen, Custom Serializer und fehlende verschachtelte Registrierungen;
- endliche Zahlen, Map-Schlüssel und deterministisch sortierte Sets;
- Typed/Untyped Asset-Handles mit Pfad, UUID und flüchtiger Sessionidentität.

[Werttests](../../src/session/inspect/value.rs) prüfen außerdem kanonische
Set-Sortierschlüssel bei `serde_json/preserve_order`. Bevy 0.19.1 reflektiert
`BTreeSet` opak ohne Serializer, deshalb bleibt es `NotSerializable`;
`HashSet` prüft die strukturelle Set-Regel. Keine zusätzliche Collection-
Sonderbehandlung ist vorgesehen. Die Fixtures sichern künftige Bevy-Upgrades ab.

## Entscheidungen mit Freigabe

- Eine Begrenzung der wartenden Report-Queue benötigt eine ausdrückliche Regel
  für Überlast, ohne Event-Empfang oder Spiel-Pipes durch langsame Provider zu blockieren.
- Resize-/DPI-/Bildschirmwechsel wurden ausdrücklich zurückgestellt. Eine spätere
  Wiederaufnahme einschließlich geänderter Render-/Simulationszuständigkeiten
  benötigt eine neue Freigabe; die Tick-Regeln bleiben unverändert.

Private Thread-/Kanalstruktur, begrenzte Arbeitsbudgets, Testhilfen und andere
reversible Details innerhalb des Vertrags brauchen keine neue Produktentscheidung.

## Zurückgestellte Arbeit und bekannte Grenzen

- Dynamische Fenstergrößen-, DPI- und Bildschirmwechsel während der kontrollierten
  Session sind auf Nutzerwunsch zurückgestellt. Der experimentelle Resize-Umbau
  wurde entfernt. Stabile Fensterbedingungen sind Voraussetzung; keine zusätzliche
  Kamera-/UI-Aufbereitung oder versteckten Reparatur-Ticks. Diagnose und Evidenz
  bleiben erhalten, dies ist keine bestandene Resize-Abnahme.
- Font-/Fallback-Korrekturen für `ü`, `ß`, Emoji und Japanisch bleiben auf
  Nutzerwunsch zurückgestellt. Gespeicherte Unicode-Werte sind geprüft.
- Vollständige Blend-Gleichungen bei Zwischenalphas und HDR-Werte oberhalb des
  LDR-Bereichs sind keine Abschlussvoraussetzung für die allgemeine Steuerung.
- Windows-Prozessverwaltung, Remote-Betrieb, Auth-Schicht, Multi-Pointer und eine
  eigene Agent-Werkzeugschleife gehören nicht zu diesem Abschluss.
- REPL-Ausgabe ist synchron; ein nicht lesender Empfänger kann sie blockieren.
  Direkte Report-Übermittlung außerhalb des Servers hat keine Serverfrist.
- Ein fehlgeschlagener GitHub-Publish kann trotzdem ein Issue angelegt haben;
  kein automatischer POST-Retry. Local benötigt Hardlinks für atomisches
  Veröffentlichen ohne Überschreiben.
- macOS-Prozessfixtures wurden sporadisch vor Ready verzögert oder mit SIGKILL
  beendet, teils mit Gatekeeper-Logs. Ein langsames `gh`-Fixture erreichte mehrfach
  seine Warteposition nicht; Ursache ungeklärt. Spätere grüne Läufe beweisen keine
  Behebung. Eindeutige Testlogs verwenden, Sicherheitsregeln nicht ändern.

## Abschlusskriterien

Der Draft ist erst abgeschlossen, wenn die oben genannten Kernfälle mit frischen
Nachweisen bestehen oder eine ausdrücklich genehmigte Einschränkung im Zielvertrag
steht. Insbesondere:

- Steuerung, Inspect und Capture halten die expliziten Tick-/Input-Regeln ein.
- Capture speichert das gerenderte Bild oder meldet einen technischen Fehler.
  Die bekannte Einschränkung bei Vollverdeckung ist zulässig; der Guard bleibt aktiv.
- Recording, Failure, Report, Client-Trennung, Replay und Stopp funktionieren gemeinsam.
- Ergebnislücken und ungewisse Ausgänge bleiben sichtbar; Shutdown bereinigt eigene
  Prozesse auch bei Fehlern und Last.
- Öffentliche Interfaces, Feature-Builds und Dokumentation stimmen mit dem geprüften
  Umfang überein. Keine offene Kernaufgabe wird durch eine bestandene Teilprüfung ersetzt.
