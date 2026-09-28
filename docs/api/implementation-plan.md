# Abschlussplan

Die wesentliche Command- und Session-Funktionalität ist implementiert.
Offen sind einzelne Steuerungsfälle, die kombinierte Systemabnahme und Lastverhalten.
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
- [ ] `game_menu`: `s`, `Escape`, `n`, Quit-Button und Screenshots prüfen.
- [ ] `ui_drag_drop`: Despawn während eines Drags gezielt in der Testszene auslösen
  und prüfen; Screenshots ergänzen.
- [ ] `mesh_picking`: vertikalen Drag am Würfel sowie Drag an Kugel und Zylinder prüfen.
- [ ] Relevante Layout-/Hierarchiewechsel und ungültig gewordene Entity-Handles
  in den ergänzten Abläufen prüfen.

[Beispiele und Kriterien](#1-steuerungsabnahme-vervollständigen)

### Technische Screenshot-Abnahme

- [ ] Bestehende Blend-, Mesh- und UI-Bildtests mit verfügbarer Renderoberfläche
  ausführen; UI-Overlay und Mehrkamera-Ausgabe prüfen.
- [ ] Unveränderte Simulation, gültige schwarze Bilder, Surface-Ablehnung,
  Timeout, späte Readbacks und sichere Dateipfade erneut prüfen.
- [ ] Resize-/Skalierungswechsel bei angehaltener Simulation prüfen und eine
  nachgewiesene Abweichung innerhalb der Tick-Regeln beheben.

[Kriterien](#2-screenshot-verhalten-absichern). Vollverdeckung und ein allgemeiner
Sollbildvergleich sind keine Abschlussaufgaben.

### Zusammenhängender Ablauf

- [ ] Recording, Eingaben, Ticks, Inspect, Capture, Failure und lokalen Report
  in einem CLI-Ablauf verbinden; Client-Trennung und Wiederverbindung prüfen.
- [ ] Verwaltungs-Stopp bei aktiver Aufnahme prüfen: JSONL-Footer, unveränderlicher
  Report, Session-Ende und Prozessbereinigung.
- [ ] In einer frischen Session den aufgezeichneten Ablauf wiedergeben und den
  Fixture-Fehler reproduzieren; aktive Wiedergabe und unerwartetes Ende ergänzen.

[Kriterien](#3-kombinierten-untersuchungsablauf-abnehmen)

### Last und Fehlerfälle

- [ ] Große Inspect-/Report-Ausgaben und langsame Clients gegen die Activity-Grenze
  prüfen; verlorene und unbestätigte Ergebnisse als unbekannt behandeln.
- [ ] Wartende Reports bei langsamem Provider und anhaltenden Failures bemessen;
  bei nötiger Überlastregel die Vertragsentscheidung vorlegen.
- [ ] Mehrere Sessions mit laufender Arbeit und Reports unter gemeinsamer
  Shutdown-Frist prüfen; vollständige Prozessbereinigung nachweisen.
- [ ] Ein separat mit `panic=abort` gebautes Programm abnehmen.

[Kriterien](#4-last-ergebnislücken-und-shutdown-prüfen)

### Schlussprüfung

- [ ] Öffentliche Requests, Capabilities, Fehlerformen und Features gegen `target.md` prüfen.
- [ ] Root-Tests, separates Bevy-Paket, Feature-Builds und CLI-Abnahmen ausführen.
- [ ] Prüfstand, verbleibende Einschränkungen und Dokumentationslinks aktualisieren.

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

In [tests/logical_state.py](../../tests/logical_state.py) den Button
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

In [tests/game_menu.py](../../tests/game_menu.py) das Hauptmenü erreichen.
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

#### Beispiel 3: Objekt verschwindet während eines Drags

In [tests/ui_drag_drop.py](../../tests/ui_drag_drop.py) die Kachel Amber greifen
und mit expliziten Ticks einen Drag beginnen. Während die Taste gehalten wird,
lässt die Testszene Amber bei einem festgelegten weiteren Tick verschwinden.
Danach den Pointer weiterbewegen, loslassen und die Eingaben per Tick verarbeiten.

Prüfen: kein Panic oder Hänger, der alte Entity-Handle ist ungültig, die virtuelle
Taste lässt sich freigeben und eine andere vorhandene Kachel kann anschließend
normal gezogen werden. Ein Drop darf keine entfernte Kachel vertauschen.
Die genaue Observer-Folge nicht aus dem normalen Drag-Ende ableiten: Die
ursprüngliche Entity existiert dann nicht mehr.

Dafür fehlt bisher ein gezielter Despawn-Auslöser in der Testszene. Diesen als
Testvorbereitung ergänzen, nicht als neuen woodpecker-Mutationscommand.

#### Beispiel 4: Vertikaler Drag und weitere Meshes

In [tests/mesh_picking.py](../../tests/mesh_picking.py) den Würfel greifen und
den virtuellen Pointer beispielsweise zwölf logische Pixel nach unten bewegen.
Vor dem Warp bleiben Transform und Drag-Zähler unverändert. Nach dem Tick muss
`drag_events` um eins steigen und die Rotation den vertikalen Drag berücksichtigen.
Die Szene verwendet `delta.y * 0.02`, hier also 0,24 Radiant um die X-Achse.
Die zusätzlich pro Tick laufende Y-Rotation muss im erwarteten Transform enthalten sein.

Den Ablauf auch für `left-sphere` und `right-cylinder` prüfen. Nur das gegriffene
Objekt erhält den Drag-Zuschlag und ein Drag-Event; die übrigen Meshes behalten
nur ihre normale zeitabhängige Rotation. Anschließend Release und erneutes
Hover prüfen. In dieser Szene dreht Ziehen das Mesh, es verschiebt es nicht.

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
- Die getrennte [Resize-/Kameralücke](diagnostics/window-schedules.md) prüfen.
  Tatsächliche Rendergröße, Skalierung, Kamera und Viewport müssen zusammenpassen.
- `tests/blend_modes.py`, `tests/mesh_picking.py` und `tests/ui.py` mit verfügbarer
  Renderoberfläche ausführen, einschließlich Recording/Replay der UI.
  Bestehende Pixelprüfungen für die bekannten Fixtures bleiben erhalten.

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

### 5. Vertrags- und Dokumentationsabgleich abschließen

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

## Abdeckungsmatrix

CLI-Abnahmen verwenden CLI, HTTP/WebSocket, Session und Bevy. Native Tests
prüfen zusätzliche Fälle. Die Tabelle beschreibt vorhandene Testabdeckung;
konkrete historische Ergebnisse stehen in
[next-steps.md](next-steps.md#letzter-dokumentierter-prüfstand) und den
[Screenshotnachweisen](diagnostics/screenshot-evidence.md).

| Bereich | Vorhandener Nachweis | Noch offen im Abschluss |
| --- | --- | --- |
| `counter` | [slice.py](../../tests/slice.py): Ticks, Stillstand, Pace/Stop, Isolation, Recording/Replay und Verwaltungs-Stopp. | Kombinierter Lifecycle und Last. |
| `context_menu` | [ui.py](../../tests/ui.py): virtuelle Inputs, Fokus, Unicode-Werte und Grenzen, Layout, Bilder, Recording/Replay, zwei Sessions. | Weitere relevante Layout-/Hierarchiewechsel. |
| `logical_state` | [logical_state.py](../../tests/logical_state.py): Update, FixedUpdate, Timer, Keyboard- und Pointer-Press/Hold/Release, Stillstand und fünf PNGs einschließlich vorgemerkter Inputs. | Keine zusätzliche Steuerungsvariante. |
| `game_menu` | [game_menu.py](../../tests/game_menu.py): Navigation, Einstellungen, Timer, Klicks, Hierarchien und tote Handles. | Keyboard-Kurzwege, Quit und Bilder. |
| `ui_drag_drop` | [ui_drag_drop.py](../../tests/ui_drag_drop.py): gültiger/ungültiger Drop, Zwischenposition, Drag-Phasen, Belegung und Darstellung. | Bilder und Despawn während Drag. |
| `mesh_picking` | [mesh_picking.py](../../tests/mesh_picking.py): Hover/Press/Release/Out aller Meshes, horizontaler Würfel-Drag, Rotation und 13 PNGs. | Vertikaler Drag und Kugel-/Zylinder-Drag. |
| `blend_modes` | [blend_modes.py](../../tests/blend_modes.py): zwei Sessions, gehaltene Tasten, Orbit, Alpha-Grenzen, HDR/Unlit, Materialidentität, reproduzierbare Farben und sechs PNGs. | Keine zusätzliche Steuerungsvariante; vollständige Blend-/HDR-Validierung ist zurückgestellt. |
| Inspect | [Entity-Tests](../../src/session/inspect/entities/tests.rs), Reflection-Matrix und Game-Menu-CLI. | Zusätzliche Szenen-Lebenszyklen; keine neue Inspect-Architektur. |
| Recording/Replay/Reports | [session.rs](../../tests/session.rs), [observation.py](../../tests/observation.py), [Report-Tests](../../src/server/report_tests.rs). | Kombinationen aus Aufnahme/Wiedergabe, Failure, Lücke und gemeinsamem Shutdown; `panic=abort`-Build. |
| CLI/REPL/Script | [repl.py](../../tests/repl.py), [script.py](../../tests/script.py), Netzwerkfixtures. | Große Outputs, anhaltende Activity und langsame Clients. |

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
- Änderungen an Render-/Simulationszuständigkeiten wegen Resize müssen die
  expliziten Tick-Regeln erhalten; andernfalls den Konflikt vorlegen.

Private Thread-/Kanalstruktur, begrenzte Arbeitsbudgets, Testhilfen und andere
reversible Details innerhalb des Vertrags brauchen keine neue Produktentscheidung.

## Zurückgestellte Arbeit und bekannte Grenzen

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
