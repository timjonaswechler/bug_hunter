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
   einmal implementieren: authentisierte HTTP-Verwaltung und gebundene WebSocket-Verbindung,
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

1. Server mit expliziter Loopback-Adresse, Zugriffsschlüssel und temporärem Basisordner starten.
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
| Äußerer Zugriff | Fehlender/falscher Schlüssel, Loopback-Bindung, feste Session-Auswahl, parallele Clients, Trennung, Cursor-Lücke und unbekannter Einreichungsausgang |
| Serverende | Gemeinsame Frist statt serieller Einzelfristen; Starting-Abbruch; Fehler einer Session blockiert andere nicht; erstes/zweites Ctrl+C und SIGTERM |
| Paketierung | Direkte Rust-Session und Plugin ohne Server-/CLI-Abhängigkeiten; separate Builds der tatsächlich gewählten Feature-Kombinationen |

Test-Doubles dienen gezielter Fehlerauslösung. Der reale CLI-/Bevy-Prozesslauf bleibt ein
eigenständiger Abnahmenachweis.

## Wiederverwendung aus dem Repository

Die folgenden Verweise sind technische Ausgangspunkte, keine Zielanforderungen:

| Vorhandener Code | Verwendung beim Rewrite |
| --- | --- |
| [entity.rs](../../src/entity.rs) | Handle-Abbildung und Tests für Generation/Lebensdauer |
| [keyboard.rs](../../src/keyboard.rs), [pointer.rs](../../src/pointer.rs), [text.rs](../../src/text.rs) | Stabile Key-Tokens, Bevy-Input- und Fokusabbildung gezielt übernehmen |
| [client/plugin.rs](../../src/client/plugin.rs) | Trennung von Anwendungsschedules und äußerem Event-Loop prüfen; Dispatch und Tick-Ausführung gegen das Ziel neu aufbauen |
| [host/session.rs](../../src/host/session.rs), [host/launch.rs](../../src/host/launch.rs) | Cargo-Argumentbildung, Pipe-Leser und Prozessgruppenbereinigung als technische Referenz |
| [observation.rs](../../src/observation.rs) | Reflection- und Hierarchiezugriffe für den privaten Inspect-Adapter prüfen |
| [screenshot.rs](../../src/screenshot.rs) | GPU-Readback, PNG und `cap-std`-Pfadzugriff für späteren Screenshot-Durchstich |
| [tests/logical_state.rs](../../tests/logical_state.rs), [logical_state.rs](../../bevy_test_apps/src/bin/logical_state.rs) | Vorhandene Testfälle und Bevy-Testanwendung auf Ziel-Commands umstellen |
| [host/recording.rs](../../src/host/recording.rs), [host/replay.rs](../../src/host/replay.rs), [host/github.rs](../../src/host/github.rs) | Datei- und Prozessmechanik prüfen; Zielzustände und Formate eigenständig umsetzen |

`bevy_test_apps` ist ein eigenes Package mit Pfadabhängigkeit auf `bug_hunter`; das
Root-Manifest enthält keine Workspace-Memberliste. Der Durchstich verwendet daher explizit
`bevy_test_apps/Cargo.toml` als Launch-Manifest statt Package-Auswahl vom Repository-Root.
Das Testpackage verlangt Bevy 0.19.1; die Root-Abhängigkeit nennt 0.19.0 als kompatible
Untergrenze. Die konkrete Auflösung wird vor Reflection-Fixtures geprüft und festgehalten.

## Während der Implementation entscheiden

Diese Punkte brauchen keine vorgelagerte Bestätigungsschleife:

- Private Thread-/Kanalstruktur, Warp-Budget, Testadapter und plattformspezifische
  Prozess-/Signalanbindung innerhalb der festgelegten Fortschritts- und Abschlussregeln.
- Sichere Schlüsselzuführung. Für den Durchstich aus einem explizit benannten
  Umgebungseintrag lesen, über Authentisierungsheader übertragen, bei fehlendem Wert
  abbrechen; nicht in URL, Argumentlisten, Logs, Config-Beispielen oder Artefakten ablegen.
  Den Zugriffsschlüssel nicht in die Umgebung gestarteter Cargo-, Spiel- oder Provider-Prozesse
  übernehmen. Loopback- und Origin-Prüfungen vor authentisierten Operationen testen.
  Kein echter Schlüssel wird mit Tests oder Dokumentation eingecheckt.
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
   dann GitHub mit Prozess-Fixtures und lokalem Rückfall. Echte Veröffentlichung nur nach
   ausdrücklicher Freigabe. Langsamer Provider, voller Eventstrom und Serverfrist werden
   gemeinsam getestet.
4. Vollständige REPL und Script einschließlich der festgelegten Abschlussbarrieren; danach
   den Agent-Zugang durch maschinenlesbare CLI-Aufrufe mit einer externen Agent-Laufzeit
   erproben. Wiederverbindung und erkennbare Ergebnislücken bleiben
   Teil derselben Client-Tests.

Technische Quellen bleiben unter [research/](research/). Sie werden gezielt für
Bevy-/Reflection-, Screenshot-, Panic-, Backtrace- und Provider-Fragen gelesen,
nicht als parallel gepflegte Zielbeschreibung.
