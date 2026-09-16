# Weiterarbeit an woodpecker

Übergabe nach dem ersten funktionsfähigen Durchstich, der Umbenennung und dem
v2-Cleanup. Dieses Dokument sammelt offene Arbeit und den nächsten Einstieg.
Es ist keine zweite Spezifikation: Verhaltensregeln stehen in [target.md](target.md),
Interfaces in [goal.rs](goal.rs), Migrationsstatus und Nachweise im
[Implementierungsplan](implementation-plan.md).

## Zu Beginn einer neuen Session

1. Den Branch `code_ownership` mit dem Commit dieser Übergabe verwenden und den
   tatsächlichen Arbeitsbaum mit `git status --short` prüfen. Spätere lokale
   Änderungen einschließlich untracked Dateien erhalten. Der frühere Stand
   `5e9552e` enthält noch nicht den hier beschriebenen Durchstich und Cleanup.
2. [Dokumentindex](README.md), Zielvertrag, Interface-Skizze und
   [Migrationstabelle](implementation-plan.md#migration-und-bereinigung) lesen.
   Die Tabelle trennt Entscheidungen von erledigter Implementation.
3. Für den ausgewählten Durchstich dessen fachliche Regeln und
   [zu portierende Testfälle](implementation-plan.md#fachliche-testfälle-für-die-offenen-durchstiche)
   prüfen. Research und ADRs nur für die jeweilige technische Frage hinzunehmen.
4. Mit dem unten beschriebenen UI-Durchstich beginnen, sofern der Nutzer keine
   andere Priorität vorgibt. Der fehlende Umfang ist beschlossen; reversible
   Implementierungsdetails selbstständig entscheiden.

## Bereits umgesetzt

- Paket und Binary heißen `woodpecker`. Aktuelle Rust-Verweise, interne
  Umgebungsnamen, Dokumentation und Testaufrufe sind angepasst.
- Lokale Clients werden ohne Schlüssel-Authentisierung zugelassen. Server und
  Client verwenden weiterhin explizite Loopback-Adressen; die Origin-Prüfung
  bleibt bestehen. Remote-Betrieb ist nicht unterstützt.
- Die direkte Session besitzt Prozessverwaltung, private Pipe-Verarbeitung,
  Request-Korrelation, typisierte Pending-Ergebnisse, History und Events.
- Warp mit Pace-Wechsel und Stop, reflektiertes Resource-Inspect und Shutdown
  funktionieren. Inspect und der normale äußere Event-Loop erzeugen keine
  zusätzlichen Simulationsticks.
- Der Server verwaltet mehrere unabhängige Sessions über HTTP und fest gebundene
  WebSocket-Verbindungen. Activity besitzt Cursor und erkennbare Lücken.
  Client-Trennung beendet angenommene Arbeit nicht.
- CLI, headless Zähleranwendung, Einzel-Session-Stopp, gemeinsame Serverfrist,
  SIGTERM und erstes/zweites Ctrl+C sind vorhanden.
- Der v2-Ausführungsweg ist entfernt: alte Host-/Driver-Fassade, Plugin- und
  Transport-APIs, Step/Clock, Automation-Marker, Controller-Beispiele und
  `failure.json`-Architektur. Der Handle samt Lebensdauertests wurde nach
  [handle](../../src/handle.rs) übernommen.
- Die Bevy-Szenen und ihre eigenen fachlichen Tests bleiben als native
  Anwendungen erhalten. Nur `counter` ist derzeit an den neuen Session-Weg
  angebunden. Die entfernte `automation`-Anbindung ist kein unterstützter Einstieg.
- Die Bibliothek ohne Features benötigt weder Server-/CLI- noch UI-/Renderer-
  Abhängigkeiten. Render-Abhängigkeiten der Testanwendungen gehören zu deren
  separatem Package.

Das ist weiterhin ein experimenteller v3-Teildurchstich, keine vollständige
Implementation des Zielvertrags. Insbesondere enthält `report` bislang nur
Konfiguration; alte Funktionen werden nicht durch Kompatibilitäts-Exports angeboten.

## Offene Aufgaben in empfohlener Reihenfolge

### 1. Input und vollständiges Inspect

- [ ] Keyboard-, Pointer- und Text-Commands samt Bevy-Anbindung implementieren.
  Key-Tokens, gehaltene Tasten, absolute/relative Bewegung, Button-/Scroll-Zustände,
  Fenstergrenzen, Fokus und Textgrößen aus dem Zielvertrag abdecken.
- [ ] Eingaben prüfen und für den nächsten Tick vormerken. Erfolg darf keine
  bereits erfolgte Verarbeitung durch Anwendungssysteme behaupten.
- [ ] Entity-Queries, Component-Filter, Namen-/Werteprojektionen und Hierarchien
  ergänzen. Den übernommenen Handle verwenden; keine Automation-Marker einführen.
- [ ] Reflection-Fixtures gegen Bevy 0.19.1 vervollständigen: exakte Type Paths,
  fehlende/opaque/nicht serialisierbare Werte, nicht endliche Zahlen, Maps, Sets
  und Asset-Handles.
- [ ] Den realen UI-Durchstich unten als Integrationstest ausführen.

Abschluss: Eine echte Bevy-Anwendung lässt sich über neue Input-Commands verändern
und allgemein inspizieren. Die fachlichen Varianten und Ablehnungen sind geprüft;
die Arbeit endet nicht schon beim ersten erfolgreichen Klick.

### 2. Screenshot und gerenderte Abnahme

- [ ] Das primäre gerenderte Fenster aufnehmen; erst nach GPU-Readback und
  erfolgreichem PNG-Schreiben antworten.
- [ ] Normalisierte Pfade, Symlink-Sicherheit, Root-Isolation und Überschreiben prüfen.
- [ ] Nachweisen, dass Aufnahme und Darstellung keine Simulationsticks oder
  simulierte Zeit hinzufügen.

Abschluss: Die realen UI-Szenen liefern überprüfbare Bilder über denselben
Client-Vertrag; fehlende Unterstützung wird korrekt abgelehnt.

### 3. Recording und Replay

- [ ] Recording-Start/-Stop, Zustände, Dateibarrieren, Pfadsicherheit und das neue
  JSONL-Format implementieren. Commands und Outcomes in Annahmereihenfolge schreiben.
- [ ] Schreibfehler, unvollständige Aufnahme, Session-Ende und Footer-Abschluss prüfen.
- [ ] Replay vollständig vor dem ersten Spiel-Command validieren und in der
  laufenden Session ausführen. Effektiv ausgeführte Warp-Ticks berücksichtigen.
- [ ] Stop während Laden und Ausführung, technische Blockierungen sowie
  Command- und Shutdown-Barrieren umsetzen.

Abschluss: Roundtrips prüfen Reihenfolge und Format, nicht beliebige Gleichheit
mit früheren Spiel-Outcomes. Stop und Ressourcenabschluss funktionieren auch
während aktiver Aufnahme beziehungsweise Wiedergabe.

### 4. Fehlerbeobachtung und Reports

- [ ] Laufende stderr-Diagnose, Panic-Hook, optionales Tracing und Marker-Framing
  ergänzen; Beobachtung von Session-Transport und Command-Outcomes getrennt halten.
- [ ] Beim Start erforderliche Metadaten erfassen und beim Fehler einen
  unveränderlichen Report-Snapshot mit History konstruieren.
- [ ] Titel, Markdown, Fehlerdaten und Signatur einschließlich Golden Vectors umsetzen.
- [ ] Zuerst den lokalen Provider, danach GitHub mit Duplikatsuche und lokalem
  Rückfall implementieren. Echte Veröffentlichung braucht ausdrückliche Freigabe.
- [ ] Clientunabhängige Report-Arbeit in Activity und die gemeinsame Serverfrist
  integrieren. Langsame Provider dürfen Event-Empfang und Pipe-Drain nicht blockieren.

Abschluss: Beobachtete Fehler ergeben Reports ohne `failure.json`-Zwischenformat
und ohne vorherigen Session-Abschluss. Provider-Fehler und ungewisse externe
Ausgänge bleiben sichtbar.

### 5. REPL, Script und Agent-Abnahme

- [ ] Die REPL während ausstehender Commands ansprechbar halten; gemeinsame
  Commands, Inspect-Kurzformen, Pending-Anzeige und Activity verwenden.
- [ ] Scripts vollständig vor Ausführung parsen und validieren; Ergebniszuordnung
  sowie die festgelegten Recording-/Shutdown-Barrieren umsetzen.
- [ ] EOF, Quit und Client-Trennung ohne versteckten Stop oder Shutdown prüfen.
- [ ] Den maschinenlesbaren CLI-Zugang mit einer vom Nutzer gestarteten externen
  Agent-Laufzeit erproben. Modellzugang und Agent-Werkzeugschleife gehören nicht hierher.

Abschluss: Menschliche und maschinelle Bedienung nutzen denselben Ausführungsweg,
auch bei parallelen Clients und Wiederverbindung.

### 6. Vollständige Systemabnahme

- [ ] Die erhaltenen Bevy-Szenen über den neuen Weg automatisieren: Fokus/Text,
  Drag-and-drop, Menüs, Timer, Layout, tote Handles und Bilder.
- [ ] Recording, Replay und Reports gemeinsam mit Client-Trennung, Activity-Lücken,
  Session-Ende und Server-Shutdown testen.
- [ ] Die vorläufige Activity-Grenze von 4 MiB mit echten großen Inspect-Outputs und
  Reports bemessen. Übergröße und verlorene Ergebnisse ausdrücklich behandeln.
- [ ] Öffentliche API, Capabilities, Fehlerformen und Dokumentation gegen den
  vollständigen Zielvertrag prüfen. Pro abgeschlossenem Bereich Migrationsstatus
  und tatsächlichen Nachweis aktualisieren.

## Konkreter nächster Durchstich

Empfehlung: die vorhandene [Context-Menu-Anwendung](../../bevy_test_apps/src/bin/context_menu.rs)
an den neuen Weg anbinden und diesen Ablauf nachweisen:

1. Die Anwendung über Server und Session starten und Ready beobachten.
2. Falls Layout-Initialisierung Ticks benötigt, diese ausdrücklich ausführen.
3. Ein benanntes UI-Element über allgemeines Inspect finden und seinen Handle
   sowie den benötigten Zustand lesen. Das ist kein neuer spezieller Name-Selector.
4. Pointer-Eingabe senden und nachweisen, dass ohne weiteren Tick noch keine
   entsprechende Änderung des Anwendungszustands verarbeitet wurde.
5. Einen ausdrücklichen Warp ausführen und dessen korrelierten Abschluss abwarten.
6. Die erwartete Zustandsänderung erneut per Inspect lesen.

Dieser Test verbindet Input, Entity-Inspect und tatsächliche Bevy-Ausführung.
Danach die übrigen Varianten und Fehlerfälle von Block 1 abschließen, bevor
Screenshot oder Recording begonnen werden.

## Nachweise und bekannte Grenzen

Die zuletzt erfolgreichen Prüfungen umfassten 15 Bibliotheks-/CLI-Tests,
6 Prozess-Integrationstests und 7 fachliche Bevy-Anwendungstests. Beide realen
Abnahmeskripte bestanden, ebenso Feature-Builds, Root-Clippy mit `-D warnings`,
Formatprüfung und `git diff --check`. Das ist der Ausgangsnachweis dieser Übergabe,
kein Ersatz für Prüfungen nach weiteren Änderungen.

Build-, Start- und Testbefehle stehen in [slice.md](slice.md#nachweise).
Die Prozessverwaltung unterstützt derzeit Unix. Weitere Plattformen wären ein
eigener Portierungsblock, falls diese Unterstützung gewünscht wird.
Die bekannte macOS-Gatekeeper-Einschränkung ist
[dort dokumentiert](slice.md#noch-nicht-enthalten): generierte Testprogramme wurden
vereinzelt verzögert oder mit SIGKILL beendet, auch ohne woodpecker-Server.
Fehler diagnostizieren statt still wiederholen oder Sicherheitsregeln verändern.

## Arbeitsregeln und zurückgestellte Änderungen

- Bestehende Nutzeränderungen erhalten. Subagenten nur nach ausdrücklicher Zustimmung.
- Reversible Implementierungsdetails eigenständig entscheiden. Neue Produktentscheidungen
  und echte Konflikte mit dem Zielvertrag zur Entscheidung vorlegen.
- Gemeinsame fachliche Symbol-/Dateipräfixe durch passende Module und Hierarchien
  ausdrücken, statt den Präfix an jedem Namen zu wiederholen.
- Commits nur auf ausdrückliche Anforderung. Dann Conventional Commits, atomar,
  unabhängig baubar und testbar; Beschreibung kleingeschrieben, ohne Schlusspunkt,
  Header höchstens 100 Zeichen. Breaking Changes mit `!` und `BREAKING CHANGE:`.
  Issue-Verweise nur bei direktem Bezug im Footer; `Fixes`/`Closes` nur, wenn der
  Merge tatsächlich das offene Issue schließen soll.
- GitHub-URL und versionierte Report-Signaturbezeichner bleiben vorerst unverändert.
  Der Nutzer koordiniert diese Anpassung später mit einem anderen Projekt.
- Die Schlüssel-Authentisierung ist bewusst entfernt. Lokale Clients gelten als
  vertrauenswürdig; kein Ersatzschlüssel und keine optionale Auth-Schicht ergänzen.

Entfernte Mechanik bei Bedarf aus `5e9552e` lesen, beispielsweise
`git show 5e9552e:src/keyboard.rs`. Die historische Migrationstabelle aus
`de32d09:docs/api/migration.md` ist keine aktuelle Löschliste. Maßgeblich für die
heutige Zuordnung bleibt der bestehende Implementierungsplan.
