# Umsetzung

Dieser Plan führt zum [Ziel](target.md), ohne es auf den ersten Durchstich zu reduzieren.
Die hier genannten Ausgestaltungen sind Implementierungsaufgaben, keine bereits
vorhandenen Funktionen.

## Erster Durchstich

Ein realer CLI-Client startet einen lokalen Server, erstellt eine Session mit einer kleinen
Bevy-Testanwendung, bindet sich daran, führt einen Tick-Warp aus, beobachtet den geänderten
Zustand und beendet Session und Server geordnet.

Als Command dient `tick.warp.start`. Er prüft den entscheidenden Unterschied zwischen
Annahme und späterem Ergebnis. Eine kleine reflektierte Zähler-Resource und
`inspect.query` machen die tatsächliche Ausführung sichtbar. Der erste Ablauf braucht
keinen Renderer und keinen externen Report-Provider.

### Baufolge

1. **Spielausführung und direkte Session.** Den gemeinsamen Command-Codec, v3-Handshake,
   `session::Plugin`, begrenzte Warp-Ausführung und Resource-Inspect aufbauen. Die Anwendung
   konfiguriert ihre Simulation selbst. Start, Pending, Event-Empfang und Shutdown verwenden
   einen privaten Koordinator; echte Pipes bleiben während ausstehender Arbeit lesbar.
2. **Serverbetrieb.** Leeren Server, Konfigurationsprüfung, ID-Vergabe, sichere exklusive
   ID-Verzeichnisanlage, frühe Erstellungsantwort, Liste und Detail implementieren.
   Start und Shutdown verwenden einen intern abbrechbaren Session-Weg. Die öffentliche
   synchrone Rust-Session bleibt unverändert.
   Der Verwaltungs-Stopp einer ausgewählten Session verwendet denselben Abschlussablauf
   wie Serverende, lässt aber Server und andere Sessions weiterlaufen. Tests prüfen
   den Aufruf während `Starting` und `Ready`, später auch mit aktivem Replay und Recording.
3. **Client und Activity.** Den getrennt versionierten äußeren Vertrag
   einmal implementieren: lokale HTTP-Verwaltung und gebundene WebSocket-Verbindung,
   Annahme, Ergebnis, Events, Cursor und erkennbare Lücken. Mehrere Clients verwenden
   denselben Ausführungsweg; ein Client-Verlust beendet keine Spielarbeit.
4. **CLI und Prozessabschluss.** Explizite Adresse, Config-Dateizugriff, Erstellen,
   Zustand, Einreichen, Beobachten und ausdrückliche Shutdown-Aufrufe anschließen.
   Gemeinsame Serverfrist und Signale an die Prozessbereinigung anbinden.
   Noch nicht vorhandene optionale Funktionen nicht als implementiert ausgeben.

Die Zwischenschritte dürfen nicht als vollständige v3-Implementation veröffentlicht werden,
solange feste Commands der Version fehlen. Erst der vollständige Command-Umfang erfüllt
den gesamten Zielvertrag. Ein Testdurchstich darf gezielt den beschriebenen Teil prüfen.

### Notwendige interne Interfaces

- `session::launch` trennt reine Config-Prüfung von Umgebungsauflösung und Prozessstart.
  Direkter Einstieg und Servererstellung verwenden dieselben Regeln.
- Der Session-Koordinator bietet intern Startfortschritt, Command-/Event-Benachrichtigung,
  Shutdown-Annahme und erzwungene Bereinigung. Ein Abbruch muss auch während Cargo-Start,
  Pipe-Drain und Shutdown erreichbar bleiben. Ein unbegrenzt blockierender Worker mit
  anschließendem `join` erfüllt die Serverfrist nicht.
- `server` besitzt das Verzeichnis und die Aufbewahrung. Sein Eintrag benötigt während
  `Starting` noch keinen erfolgreich gestarteten öffentlichen `Session`-Handle.
- `client` trennt Verwaltungszugriff von einem fest gebundenen Session-Zugang. Es ordnet
  Transportantworten zu, vergibt aber keine Session-Request-IDs.
- Die private äußere Codec-Implementation liegt beim Serververtrag, beispielsweise
  `server::protocol`, und wird vom Client verwendet. Sie nutzt die gemeinsame
  Command-Kodierung aus `session::protocol` statt deren Hüllen oder Pipes durchzureichen.

### Reale Abnahme

Die konkrete CLI-Syntax wird mit den ausführbaren Einstiegen festgelegt. Der Abnahmelauf
muss ohne Testadapter folgende Schritte erlauben:

1. Server mit expliziter Loopback-Adresse und temporärem Basisordner starten.
   Erst nach Bereitschaft erscheint die Adresse; die Liste ist leer.
2. Session mit Manifest, Package und benanntem Test-Binary erstellen. Die Antwort enthält
   sofort die volle ID; ein verzögerter Start bleibt über Detail als `Starting` sichtbar.
3. Client an diese ID binden, Ready beobachten, den Zähler per Inspect lesen.
4. Warp über mehrere Ticks einreichen. Pending mit Request-ID zuerst, später korreliertes
   Ergebnis; danach zeigt Inspect genau die erwartete Zähleränderung.
5. Einen langsamen Warp starten, den Client trennen und neu verbinden. Fortschritt und
   Ergebnis bleiben gemäß Activity-Vertrag beobachtbar. Währenddessen kann ein weiterer
   Client Pace ändern oder Stop senden.
6. Eine zweite Session erstellen und nachweisen, dass gleicher numerischer Request-ID-Wert
   nicht zur falschen Session führt.
7. Nach Abschluss der Commands nur die erste Session herunterfahren. Die zweite und der
   Server bleiben aktiv; Detail der ersten zeigt `Ended`.
8. Serverende anfordern. Auch die zweite Session wird bereinigt. Es bleiben weder Spiel-
   noch Cargo-Prozesse zurück; Artefaktverzeichnisse bleiben erhalten.

### Automatisierte Nachweise

| Seam | Nachweis |
| --- | --- |
| Command-/v3-Codec | Ready, Warp, Inspect und Shutdown; strikte Felder, falsche Version, qualifizierte Namen, ungeordnete Responses, bekannte/unbekannte Request-ID |
| Bevy-Plugin | Keine Ticks im normalen Event-Loop oder bei Inspect; exakte Tickzahl; weiter erreichbarer Stop/SetPace |
| Direkte Session | Fortschritt ohne Polling, Pending-Drop ohne Abbruch, genau einmalige Entnahme, fremdes Pending, History-Grenze, Event-Ende |
| Prozessverwaltung | Echte Kindprozesse mit vollen Pipes, Startfehlern, verzögertem Ready, unerwartetem Exit, hängendem Shutdown und Prozessgruppenbereinigung |
| Server | Frühe Annahme, intrinsische Ablehnung ohne Eintrag, Umgebungsfehler unter ID, Kollisionen und Anlage-Rennen, alle fünf Lebenszykluszustände |
| Äußerer Zugriff | Zugang ohne Zugangsdaten, Loopback-Bindung, Origin-Prüfung, feste Session-Auswahl, parallele Clients, Trennung, Cursor-Lücke und unbekannter Einreichungsausgang |
| Serverende | Gemeinsame Frist statt serieller Einzelfristen; Starting-Abbruch; Fehler einer Session blockiert andere nicht; erstes/zweites Ctrl+C und SIGTERM |
| Paketierung | Direkte Rust-Session und Plugin ohne Server-/CLI-Abhängigkeiten; separate Builds der tatsächlich gewählten Feature-Kombinationen |

Test-Doubles dienen gezielter Fehlerauslösung. Der reale CLI-/Bevy-Prozesslauf bleibt ein
eigenständiger Abnahmenachweis.

## Migration und Bereinigung

Entscheidung und Umsetzungsstatus sind getrennt: `überarbeiten` bedeutet nicht
`bereits vollständig ersetzt`. Maßgeblich bleibt `target.md`.
Die historische Zuordnung steht in `de32d09:docs/api/migration.md`, Zeilen 43–74.
Ihre Dateipfade beschrieben teilweise einen anderen Arbeitsbaum. Die folgende
Tabelle ordnet deshalb die tatsächlichen v2-Dateien ihren heutigen Verantwortlichkeiten zu.

Die entfernten Dateien und Tests bleiben unter Commit `5e9552e` verfügbar, beispielsweise
mit `git show 5e9552e:src/keyboard.rs`. Es gibt keinen parallel gebauten Legacy-Bereich.
Die Entfernung des v2-Einstiegs schließt die noch fehlenden v3-Funktionen nicht ab.

| Bisherige Dateien und Nutzer | Ziel-Owner | Entscheidung | Aktueller Status und Nachweis |
| --- | --- | --- | --- |
| `src/entity.rs`, alte Beobachtung und Controller | [handle](../../src/handle.rs) | übernehmen | Handle und Lebensdauertests übernommen; Fehler heißt `handle::Error`. Kein Root-Re-Export der alten Fassade. |
| `src/client/plugin.rs`, `src/client/transport.rs`, `src/protocol.rs` | [session](../../src/session/mod.rs), [session::protocol](../../src/session/protocol.rs) | ersetzen | v2 entfernt. Codec-, Pending-, Event- und Prozessfixtures prüfen den neuen Transport. Entity-Inspect sowie Pointer-, Keyboard- und Text-Input besitzen Plugin-Tests und die reale [Context-Menu-Abnahme](../../tests/ui.py). |
| `src/time.rs`, alte Step-Commands | [command::tick](../../src/command/tick.rs), [session::Plugin](../../src/session/plugin.rs) | überarbeiten | Step/Clock entfernt. Warp-, Pace-, Stop- und No-Tick-Nachweise vorhanden; Zeitkonfiguration bleibt bei der Anwendung. |
| `src/host/session.rs`, `launch.rs`, `controller.rs`, `diagnostics.rs` | `session`, `session::history`, `report` | ersetzen | Alter Session-Owner entfernt. Neue Prozessverwaltung, History, Fehlerbeobachtung und Snapshot-Konstruktion implementiert. [Prozesstests](../../tests/observation.rs) prüfen Panics, Tracing, Startdiagnosen, Pipe-Drain und unveränderlichen Kontext. |
| `src/host/mod.rs`, `command_line.rs`, `config.rs`, `runner.rs`; alte Controller-Beispiele | `server`, `client`, `cli` | ersetzen | Fassade, `host`/`driver`-Features und alte Beispiele entfernt. Neue CLI mit privaten Config-Typen ist der einzige unterstützte Einstieg. |
| `src/target.rs`; alte Queries und Bevy-Testapps | kein Ersatz | löschen | Marker, Exports, Inserter und Nutzungen entfernt. Bevy-Anwendungen behalten normale Entity-Namen und ihre fachlichen Systeme. |
| `src/keyboard.rs`, `pointer.rs`, `text.rs` | `command::input`, private Bevy-Adapter unter `session` | überarbeiten | Virtueller Pointer, Keyboard und Text implementiert. Adapter-Tests prüfen Tokens, Fenster, Zustände, UTF-8-Grenze und Fokus. [Plugin-Tests](../../src/session/plugin/input_tests.rs) prüfen Vormerkung, gehaltene Tasten, Modifier, native Störeingaben und Isolation; reale UI-Abnahme prüft alle Eingaben in zwei Sessions. `ui` ermöglicht fokussierte Texteingabe und virtuelle `Interaction`. |
| `src/observation.rs` | `command::inspect`, privater Inspect-Adapter | ersetzen | Resource-, Entity-, Component- und Hierarchievarianten implementiert. [Adapter-Tests](../../src/session/inspect/entities/tests.rs) prüfen Filter, Handles, Resource-Isolation, Projektionen, Reihenfolge und Wertstatus; Plugin-Test prüft Lesen ohne Tick/Zeitfortschritt. UI-Abnahme liest Layout und Anwendungszustand. Die [Reflection-Matrix](#reflection-matrix) gegen Bevy 0.19.1 ist geprüft. |
| `src/screenshot.rs` | `command::screenshot`, privater Capture-Adapter | überarbeiten | Optionales `screenshot`-Feature implementiert. [Capture-Tests](../../src/session/screenshot/capture.rs) prüfen verzögerten Abschluss, PNG, Fenster und Readback-Timeout; [Pfadtests](../../src/session/screenshot/destination.rs) prüfen Root-Isolation, atomischen Ersatz und Symlink-Wechsel. [UI-Abnahme](../../tests/ui.py) prüft echte 640×360-PNGs in zwei Sessions, parallele Requests und unveränderte Ticks/Zeit. |
| `src/host/recording.rs`, `replay.rs`; `tests/driver_recording.rs` und alte JSONL-Fixture | `command::recording`, `command::replay`, `session` | überarbeiten | Recording und Replay implementiert. [Recording-Tests](../../src/session/recording/tests.rs) prüfen Dateibarrieren und Schreibfehler; [Replay-Tests](../../src/session/replay/tests.rs) prüfen striktes Laden, effektive Warps, Stop und Blockierungen. [Prozessfixtures](../../tests/session.rs) sowie Zähler-/UI-Abnahmen prüfen Roundtrips und Ressourcenabschluss. Die alte Fixture ist kein gültiger v1-Nachweis. |
| `src/host/report.rs`, `issue_report.rs`, `github.rs`, Fehleranteile von `diagnostics.rs`; alte Report-Tests | `report`, privater Session-Observer | überarbeiten / Zwischenformat löschen | `failure.json` entfernt. Privater Markerparser, Panic-/Tracing-Erfassung und `Report::create` mit Titel, Failure, v1-Signatur und Context implementiert. [Report-Tests](../../src/report/tests.rs) prüfen Golden Vectors und Markdown. `report::submit` unterstützt Local und Github; [Local-Tests](../../src/report/provider/local/tests.rs) und [GitHub-Prozessfixtures](../../src/report/provider/github/tests.rs) prüfen Pfade, Konkurrenz, Pagination, stdin, Fehler und Fallback. Automatische Server-Reports behalten Snapshot und Ergebnis in Activity. [Server-Tests](../../src/server/report_tests.rs) prüfen späte Ergebnisse, Frist, erzwungene Prozessgruppenbereinigung und Shutdown-Failures; [CLI-Abnahme](../../tests/observation.py) prüft echte lokale Dateien nach Client-Trennung. Lastbemessung bleibt offen. |
| `src/host/repl.rs`, `script.rs` | `cli::repl`, `cli::script` über `client` | überarbeiten | REPL und Script über gemeinsamen Client umgesetzt. `tests/repl.py` prüft reale Bevy-Sessions und Terminalbedienung. Script validiert vollständig vor Ausführung, korreliert Outcomes und wartet an Recording-/Shutdown-Barrieren. Netzwerkfixtures und `tests/script.py` prüfen Fehler, Reihenfolge und Client-Trennung ohne versteckten Stop. Die [externe Abnahme mit pi](pi-acceptance.md) bestätigt CLI-/Script-Bedienung und weiterlaufende Arbeit nach Agent-Ende. |
| `bevy_test_apps` mit `automation`, `tests/logical_state.rs`, `examples/*_controller.rs` | normale Testanwendungen; neue Client-Abnahmen | ersetzen | v2-Anbindung und Controller entfernt. Native Bevy-Anwendungen und datenbasierte UI-Komposition bleiben erhalten. `counter`, `context_menu`, `logical_state`, `game_menu` und `ui_drag_drop` sind mit `slice` angebunden. Context-Menu-/UI-Abnahme prüft Menüs und Eingaben erst beim Tick. [tests/logical_state.py](../../tests/logical_state.py) prüft Update, FixedUpdate, Timer und Keyboard über CLI. [tests/game_menu.py](../../tests/game_menu.py) prüft virtuelle Klicks, Einstellungen, Timergrenzen, Hierarchien und tote Handles nach Bildschirmwechseln. [tests/ui_drag_drop.py](../../tests/ui_drag_drop.py) prüft gültigen/ungültigen Drop, Drag-Phasen, Belegung und Layout. Die [Abdeckungsmatrix](next-steps.md#abdeckungsmatrix-für-block-6) benennt Lücken; `mesh_picking` ist als Nächstes offen. |

### Reflection-Matrix

Die [gemeinsamen Fixtures](../../src/session/inspect/reflection_tests.rs) prüfen
die JSON-Ergebnisse über Resource- und Component-Queries. Die Fehlerfälle führen
zu einem Wertstatus, nicht zum Abbruch der Query. Wiederholtes Lesen lässt die
World-, Resource- und Component-Change-Ticks unverändert.

| Fall | Nachweis |
| --- | --- |
| Exakte registrierte Type Paths | Eigener `#[type_path]`; Kurzname, Rust-Typname und falsche Großschreibung werden abgelehnt. Bestehende Entity-Tests prüfen sämtliche Filter-/Listed-Pfade vor Handle-Auflösung. |
| Fehlende und nicht zugängliche Werte | Resource `Missing`/`NotReflectable`, Resource-All-Auswahl und Sortierung; bestehende Entity-Tests unterscheiden `Missing`, `NotRegistered`, `NotReflectable` und `NotSerializable`. |
| Metadaten | Ein Serializer, der bei Aufruf panikt, bleibt bei Resource-Metadata und Component-Namen unberührt. Kein Wertstatus in den Metadaten. |
| Opaque und Registrierungen | Opaque ohne Serializer, absichtlich fehlschlagender Serializer, fehlende verschachtelte Registrierung; opakes Custom-`null` bleibt lesbar. |
| Zahlen | `f32` und `f64`, `NaN` und beide Unendlichkeiten, direkt und verschachtelt in Struct/Vec/Option/Tupel/Map. Der umgebende Wert wird unlesbar. |
| Maps | String-, Zeichen-, boolesche und ganzzahlige Schlüssel einschließlich `u64::MAX`; leere Maps bleiben Objekte. Tupel-/Vec-Schlüssel sind nicht darstellbar. |
| Sets | Leere Sets, lexikalische statt numerischer Reihenfolge, verschiedene Einfügereihenfolgen, verschachtelte HashSets und fehlerhafte Elemente. |
| Kanonische Set-Sortierschlüssel | [Serializer-Test](../../src/session/inspect/value.rs) prüft rekursiv geordnete Objektschlüssel bei unveränderter Array-Reihenfolge, auch mit `serde_json/preserve_order`. |
| Asset-Handles | Typed/Untyped mit UUID, Pfad samt Quelle und Label sowie flüchtiger ID; Klone behalten die Referenz, unterschiedliche IDs bleiben verschieden. Fehlende Handle-/Asset-Registrierungen ergeben `NotSerializable`. Pfadfixtures verwenden eine In-Memory-Quelle. |

Bevy 0.19.1 implementiert `BTreeSet` als opaken Reflect-Typ ohne Serializer.
Die Matrix hält deshalb `NotSerializable` fest und prüft die strukturelle
Set-Regel mit `HashSet`. Es wurde keine zusätzliche Collection-Sonderbehandlung
eingeführt. Die [aktuellen Läufe](next-steps.md#nachweise-und-bekannte-grenzen)
umfassen alle Features, die Bibliothek ohne Features und JSON mit `preserve_order`.

### Fachliche Testfälle für die offenen Durchstiche

Folgende Nachweise sind nicht durch die Entfernung alter Tests erledigt. Die
Quellpfade beziehen sich auf `5e9552e`, nicht auf Dateien im aktuellen Arbeitsbaum.

| Offener Durchstich | Zu übernehmende Prüfungen und Referenzen |
| --- | --- |
| Input | `src/keyboard.rs`: Key-Token-Roundtrip und Press/Release-Zustände. `src/pointer.rs`: endliche Koordinaten, relative/absolute Bewegung, Grenzen und Button-Zustände. `src/text.rs`: UTF-8-Byte-Grenze, eindeutiger lebender editierbarer Fokus. `src/client/plugin.rs`: Input vormerken, gehaltene Tasten und Verarbeitung erst beim Tick, Session-Isolation. Neue Outputs und Ablehnungscodes verwenden. |
| Entity-Inspect | `src/observation.rs`: Reflection-/Opaque-Status und Hierarchietiefe. Das heutige [handle](../../src/handle.rs) bewahrt die Generationstests. Markerfilter, Clock-Spezialabfragen, Pagination und alte Größenkürzungen werden nicht übernommen. Die neuen vollständigen Type Paths und Value-Statusregeln gelten. |
| Screenshot | `src/screenshot.rs`: normalisierte PNG-Pfade, Symlink-Ausbruch, Root-Isolation, lesbares PNG nach Abschluss. Zusätzlich nach neuem Vertrag: Überschreiben, eindeutiges primäres Fenster, kein Tick und keine simulierte Zeitänderung. |
| Recording/Replay | `tests/driver_recording.rs`, `src/host/recording.rs`, `replay.rs`: Reihenfolge, Dateibarrieren, Ausschlüsse, Abschluss und Fehlerfälle fachlich übertragen. Alte redigierte/gekürzte Outputs, eigene Controller-Aktionen und Outcome-Gleichheit nicht als Ziel übernehmen. |
| Reports | `tests/issue_report.rs`, `github_report.rs`: Fehlerdarstellung und Prozessfixtures für Provider prüfen. Kein `failure.json`-Lader, kein vorheriger Session-Abschluss als Voraussetzung; Snapshot und Signatur-Golden-Vectors aus dem aktuellen Zielvertrag ergänzen. |
| UI-Abnahmen | `tests/logical_state.rs`, `examples/*_controller.rs`: Fokus/Text, gehaltene Tasten, Pointer-/Drag-Lebenszyklus, Timer, Layout, tote Handles und Bilder auf neue Commands übertragen. Die vorhandenen nativen Bevy-Anwendungen liefern weiterhin die fachlichen Szenen. |

Die erhaltenen fachlichen Bevy-Tests bleiben ausführbar. Die neuen Session- und
Prozessfixtures decken laufenden Warp, Korrelation, Pending-Drop, Event-Überlauf und
Bereinigung ab. Nach jeder weiteren Portierung wird die zugehörige Zeile erst nach
dem tatsächlichen Nachweis abgeschlossen.

`bevy_test_apps` ist ein eigenes Package mit Pfadabhängigkeit auf `woodpecker`; das
Root-Manifest enthält keine Workspace-Memberliste. Der Durchstich verwendet daher explizit
`bevy_test_apps/Cargo.toml` als Launch-Manifest statt Package-Auswahl vom Repository-Root.
Das Testpackage verlangt Bevy 0.19.1; die Root-Abhängigkeit nennt 0.19.0 als kompatible
Untergrenze. Die Lockfiles halten die geprüfte Auflösung auf 0.19.1 fest.

## Während der Implementation entscheiden

Diese Punkte brauchen keine vorgelagerte Bestätigungsschleife:

- Private Thread-/Kanalstruktur, Warp-Budget, Testadapter und plattformspezifische
  Prozess-/Signalanbindung innerhalb der festgelegten Fortschritts- und Abschlussregeln.
- Lokale Clients ohne Zugangsdaten zulassen. Loopback- und Origin-Prüfungen testen;
  keine optionale Auth-Schicht oder automatische Schlüsselverwaltung ergänzen.
- HTTP-Routen, WebSocket-Framing, Versionsprüfung und stabile Fehlerabbildung aus den
  vorhandenen fachlichen Fehlergruppen. Vor Implementierung als gemeinsame Codec-Fixtures
  festhalten. Ein Verbindungsfehler darf keinen Command-Erfolg behaupten.
- Den Standardwert der Activity-Byte-Grenze mit repräsentativen Inspect-Outputs und Reports
  bemessen. Tests prüfen Verdrängung und einzelne zu große Einträge samt sichtbarer Lücke.
- Konkrete CLI-Flags, private TOML-Felder, Ausgabehüllen, Zeitformat und deterministische
  Sortierung bei gleichen Zeitpunkten. Config-Parser gehört zur CLI, fachliche Prüfung
  zu Server beziehungsweise Session. Die Servererstellung besitzt keinen Artefakt-Override.
- Capability-Prüfung aus tatsächlich verwendeten Commands und `session::Capabilities`
  ableiten. Der interaktive Einstieg setzt keine Screenshot-Unterstützung voraus.
- Zunächst bestehende Crate-Struktur nutzen und Abhängigkeiten über gezielte Features
  trennen; nur benötigte Einstiege exportieren. Weitere Crates erst bei nachgewiesenem
  Abhängigkeitsproblem. Feature-/Binary-Namen im ersten Implementierungsdiff festhalten.
- Fristen müssen intern Start, Datei-/Provider-Arbeit und Ressourcenabschluss erreichen.
  Bei nicht unterbrechbarer Betriebssystemarbeit keinen garantierten erfolgreichen
  Abschluss zur Frist behaupten.

Verändern diese Arbeiten einen beschlossenen Vertrag, ist das keine freie
Implementierungsentscheidung. Der konkrete Konflikt wird mit Auswirkungen und Empfehlung
zur Entscheidung vorgelegt.

## Weitere Durchstiche innerhalb des Gesamtziels

1. Vollständige Input- und Inspect-Varianten, Reflection-Fixtures sowie gerenderter
   Screenshot ohne Simulationstick. Die vorhandenen UI-Testanwendungen liefern reale
   Input-, Fokus- und Bildprüfungen.
2. Recording mit Dateibarrieren und Replay mit vollständiger Vorabvalidierung,
   Warp-Verkürzung, Stop während Laden/Ausführung und technischen Blockierungen.
   Roundtrip-Tests prüfen Reihenfolge und Formate, nicht beliebige Outcome-Gleichheit.
3. Panic-/Tracing-Beobachtung, Report-Snapshot, Signatur-Golden-Vectors und lokaler Provider,
   dann GitHub mit Prozess-Fixtures und lokalem Rückfall. Bei konfiguriertem GitHub-Provider
   veröffentlicht die Anwendung ohne zusätzliche Rückfrage; Entwicklungstests legen
   keine echten Issues an. Langsamer Provider, voller Eventstrom und Serverfrist werden
   gemeinsam getestet.
4. Vollständige REPL und Script einschließlich der festgelegten Abschlussbarrieren; danach
   den Agent-Zugang durch maschinenlesbare CLI-Aufrufe mit einer externen Agent-Laufzeit
   erproben. Wiederverbindung und erkennbare Ergebnislücken bleiben
   Teil derselben Client-Tests.

Technische Quellen bleiben unter [research/](research/). Sie werden gezielt für
Bevy-/Reflection-, Screenshot-, Panic-, Backtrace- und Provider-Fragen gelesen,
nicht als parallel gepflegte Zielbeschreibung.
