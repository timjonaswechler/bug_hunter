# Weiterarbeit an woodpecker

## Aktueller Stand

Session-/Prozessverwaltung, explizite Warps, virtuelle Eingaben, Inspect,
Screenshot-Grundfunktion, Recording/Replay und Reports sind implementiert.
Die sieben bisherigen Bevy-Szenen besitzen CLI-Abnahmen; die ergänzte Steuerungsabnahme
und die Screenshot-Prüfungen bei stabilen Fensterbedingungen sind abgeschlossen.
Auch alle drei Punkte des zusammenhängenden Untersuchungsablaufs sind abgenommen.

Der Nutzer hat dynamische Resize-, DPI- und Bildschirmwechsel ausdrücklich aus
dem aktuellen Abschlussumfang zurückgestellt. Die experimentelle
Darstellungsaufbereitung, der Layout-Proxy und die Resize-Diagnose-Anbindung
wurden zurückgenommen. Es gibt dafür keinen zusätzlichen Kamera-/UI-Kontrolllauf.
Der Punkt ist **zurückgestellt, nicht bestanden**. Maßgeblich: [target.md](target.md#screenshot).
Erkenntnisse und historische Evidenz: [Resize-Diagnose](diagnostics/window-schedules.md).

Beim Einstieg Branch und `git status --short` prüfen; bestehende Änderungen und
unversionierte Dateien erhalten. [Abschlussplan](implementation-plan.md) enthält
Aufgabenstatus und Abdeckungsmatrix; [usage.md](usage.md) die ausführbaren Befehle.

## Neuer Auftrag: Alien Cake Addict vorbereiten

Nach dem Abschluss-Commit `de4fbe4` wurde auf Nutzerauftrag zuerst der bereitgestellte
Alien-Cake-Ausgangsstand samt drei GLB-Modellen in `b085470` gesichert. Die zunächst
invasive Anpassung wurde auf ausdrücklichen Nutzerauftrag zurückgenommen:
keine geänderte Spiellogik, Systemreihenfolge, Zufallsauswahl, Cooldown-/Spawn-Timer,
Neustartlogik, Fenster-/Kamera-Konfiguration oder feste Tickdauer. Zusätzlicher
Beobachtungszustand, Bewegungs-Script und Tests für geänderte Logik wurden entfernt;
Archiv und historische Prüflogs bleiben unter `target/alien-cake-preparation/`.
Diese 13-Test-Ergebnisse sind historisch und kein Nachweis der Minimal-Anbindung.

Die minimale Vorbereitung benötigt **kein `slice`**. Die Anwendung bleibt nativ,
wenn die interne Session-Umgebung fehlt, und installiert ansonsten das bestehende
`woodpecker::session::Plugin`. Der Plugin-Vertrag selbst ist unverändert und verlangt
derzeit diese Umgebung; bedingungsloses Installieren unterstützt noch keinen normalen
nativen Start. `slice` bleibt nur für die übrigen bestehenden Fixtures als Legacy-
Umschalter bestehen. Woodpecker ist im Test-App-Paket eine normale Abhängigkeit;
`ui` und `screenshot` werden dort aktiviert, nicht durch eine geänderte Spiellogik.

Reflection-Derives und Registrierungen machen die **vorhandenen** Ressourcen `Game`,
`State<GameState>` und `BonusSpawnTimer` inspizierbar. Private Rust-Daten ohne solche
Metadaten sind nicht automatisch lesbar. Der Asset-Pfad und die ursprünglichen RNG-
Abhängigkeiten sind für direkte Binärstarts eingerichtet; ein Query-Alias hält
Clippy ohne Suppression grün. Fixture: `tests/fixtures/alien_cake_addict.toml`,
`features = []`. Bedienung und Grenzen im
[Bevy-README](../../bevy_test_apps/README.md#alien-cake-addict-controlled-preparation).
Prüfstand/Logs: `target/alien-cake-minimal/`. Alle neun Spiel-Funktionskörper und die
Update-Systemreihenfolge stimmen mit `b085470` überein (ohne Format-/Kommentarvergleich).
Die zehn bisherigen Bevy-Tests, strenges Clippy, All-Targets-Check ohne Features und
Alien-Build ohne Features bestehen. Keine neuen Tests behaupten eine Abnahme geänderter
Logik. Build/Tests sind keine reale Session- oder grafische Abnahme; der Live-Inspect
von `Game` ist noch nicht geprüft.
Vorbereitung noch uncommitted. Ein neuer GUI-Lauf erfordert frische Freigabe.

### Neuer SDK-Fix: Render-Bootstrap ohne versteckten Tick

Zwei reale Starts endeten vor dem ersten Tick mit einer GPU-Validierung für
`clustering dummy texture`, `Dimension X is zero`; der zweite ohne Spielagenten.
Die historische Evidenz bleibt unter `target/alien-score-cli/` und
`target/alien-score-cli-retry-1/`. Die öffentliche headless Diagnose unter
`target/render-init-diagnosis/` zeigte: aktive 3D-Kamera mit gültigem Viewport,
aber mangels PostUpdate nullwertige Cluster; nach einem expliziten Tick gültig.

Auf Nutzerauftrag wartet der SDK-Bootstrap jetzt mit Render-Extraction und dem
ursprünglichen Render-Einstiegsschedule bis zum ersten vollständig ausgeführten
expliziten Tick. Danach rendert die Anwendung auch zwischen Warps weiter. Das Gate
bleibt bei Bevys pipelined Render-Handoff am RenderApp. Keine zusätzlichen
Anwendungsschedules, Kameraänderungen oder Zeitüberschreibungen. Bevy-Render-Typen
gehören dafür zum SDK-Kern, unabhängig von der optionalen PNG-Capability; kein
Renderer wird installiert. Capture vor dem ersten Tick endet sofort mit
`screenshot_window_unavailable`, ohne Queueing oder Dateischreiben.

Prüfstand `target/render-bootstrap-fix/`: Regression erst rot, danach grün;
144 Library- und 25 Integrationstests seriell, 94 Tests ohne Features,
zehn Bevy-Tests, strenges Root-/Bevy-Clippy, Feature-Check, Builds und Format-/Diff-
Prüfung bestanden. Eine öffentliche headless Probe mit echten Bevy-Kamera-/Licht-
Systemen bestätigt: vor dem Tick keine Render-Extraction; danach gültige Cluster
und Rendering. Eine erste vollständige Prüfung hatte einen 30-s-Timeout im
bestehenden Report-Queue-Test bei parallelen Cargo-Builds; isolierte und anschließend
vollständige serielle Prüfung bestanden mit unveränderten Fristen. Historische
Fehlerlogs bleiben erhalten, die genaue Timeout-Ursache ist nicht abschließend geklärt.

**Grafischer Versuch noch nicht bestanden:** Unter `target/render-bootstrap-gui/`
blieb das Fenster vor dem ersten Tick stabil und wurde vom Nutzer schwarz sichtbar
bestätigt. Genau ein expliziter Tick wurde abgeschlossen. Beim einzigen Capture
panikte die Bootstrap-Prüfung, weil Control `Bridge` mittels `resource_scope`
vorübergehend aus der World entnimmt. Session/Server wurden beendet; kein Retry.
Ein roter Test über den tatsächlichen Command-Dispatch reproduzierte dies. Der
Render-Startmarker liegt jetzt unabhängig von Bridge in der World und wird erst
nach dem ersten vollständig ausgeführten Tick geöffnet. Command-Pfad-Regression,
144 Library-/25 Integrationstests, 94 Tests ohne Features, Clippy und öffentliche
Kamera-/Lichtprobe bestehen nach der Korrektur. Der explizit genehmigte zweite
Versuch (`target/render-bootstrap-gui-retry-1/`) blieb ebenfalls vor dem Tick
stabil, führte genau einen Tick aus und panikte nicht. Der einzige Capture wurde
jedoch mit `screenshot_window_unavailable` abgelehnt: `primary window has no render
surface for this capture frame`. Session/Server wurden beendet (Server-Exit 0),
keine weiteren Ticks oder Capture-Retries. Reale 3D-Ausgabe und Capture sind weiter
nicht abgenommen. Diagnose ohne neuen GUI-Lauf unter
`target/capture-surface-diagnosis/`: Die Guard-Prüfung entspricht Bevy 0.19.1s
Window-Copy-Voraussetzungen und läuft im echten Render-Basisschedule nach Prepare
und vor Render (neuer Test). 17 Screenshot-Tests, Clippy und synthetische öffentliche
Kamera-/Lichtprobe bestehen. Der konkrete native Surface-Ausfall ist aus den
bisherigen Logs nicht reproduzierbar: Fenster/View/Format und Kamera-Ziele wurden
nicht einzeln protokolliert. Kein Verhalten geändert, Guard nicht abgeschwächt.
Nächster Schritt ist gezielte Capture-Frame-Instrumentierung vor einem separat
freigegebenen GUI-Versuch; Root Cause und grafische Abnahme bleiben offen.
Der genehmigte instrumentierte Versuch (`target/render-bootstrap-gui-retry-2/`)
führte genau einen Tick und einen erfolgreichen Capture aus. View und Format waren
vorhanden; eine schreibende Kamera zielte auf das Fenster. Die 1280×720-PNG ist jedoch
vollständig einfarbig RGB (43, 44, 47): kein bestätigter Spielinhalt. Die reflektierte
Game-Resource war lesbar und blieb durch Capture unverändert. Session/Server sauber
beendet, keine weiteren Ticks/Retrys. Temporäre SDK-Logs entfernt, 17 Screenshot-Tests
nach Cleanup bestanden. Der Erfolg mit Instrumentierung beweist keine Ursache oder
Behebung des vorherigen Surface-Ausfalls. Der folgende Neustart
(`target/render-bootstrap-gui-retry-3/`) endete vor jeglichem Tick nach Ablauf der
expliziten Aktionsfreigabe-Wartefrist; kein Capture, kein automatischer Neustart.
Erst nach erneuter Freigabe lief `target/render-bootstrap-gui-retry-4/`: menschliche
Sichtbarkeitsbestätigung, genau 10 explizite Ticks bei 60 Ticks/s ohne Input, ein
1280×720-Capture. Das Bild zeigt 3D-Kachelbrett, zentrale Figur und `Sugar Rush: 0`
(2684 RGB-Farben, SHA256
`cb1ae3d5fcbd457b0219dce33904e5f5d1cde86cf88f3cd3ece059033c0ae646`).
Reflektierte Game-Resource lesbar, Score 0, durch Capture unverändert. Session/Server
sauber beendet. Echte 3D-Ausgabe und Capture nach expliziten Ticks sind damit in
diesem Lauf bestätigt; Cake-Modell zur Laufzeit und source-blinder Score-30-Test
bleiben offen. Die frühere Surface-Ablehnung bleibt historisch ungeklärt.
Der anschließende source-blinde Sol-6.1-Medium-Test
(`target/alien-score-native-1/`) erreichte maximal **Score 7**, nicht 30. Agent nur
mit `game`/`contact_supervisor`, keine Quellzugriffsversuche. Nach einem lokalen
Inspect-Schemafehler wurde derselbe Agent mit ausdrücklicher Nutzerfreigabe und
reiner API-Klarstellung fortgesetzt. Der Spielprozess endete später unerwartet
mit Exit 0: Monitor-Removed-Meldung, danach fehlende Window-Komponente, schließlich
`No windows are open, exiting`. Ursache nicht bewiesen; keine automatische
Wiederholung. Watchdog sah Failed und beendete den Server; Shutdown meldete
`shutdown_incomplete`. Keine Spiel-/Server-Prozesse übrig. Finaler Teilbericht und
Fehlerhistorie erhalten. Score-30-Abnahme bleibt offen.
Ein erneut ausdrücklich genehmigter frischer Spieler
(`target/alien-score-native-2/`, Workflow `19358739-7ac7-4805-8818-322346d5ffd4`)
erreichte maximal **24**, mit nur zwei relevanten Captures und null
Quellzugriffsversuchen. Derselbe beobachtete Fensterablauf trat wieder auf:
Monitor-Removed, fehlende Window-Komponente, `No windows are open, exiting`,
unexpected exit 0. Agent stoppte ohne Retry; Watchdog-Shutdown meldete erneut
`shutdown_incomplete`. Keine Spiel-/Server-Prozesse übrig. Ursache weiterhin
nicht etabliert; wiederholte zeitliche Korrelation ist kein Kausalnachweis.
Teilbericht/Audit erhalten; vor weiterer grafischer Wiederholung den
Fenster-Lifecycle-Befund gezielt diagnostizieren, ohne Gameplay-Quellzugriff.
Der ausdrücklich genehmigte instrumentierte Lauf `target/window-loss-gui-1/`
führte genau zehn Ticks ohne Input/Capture aus und beobachtete anschließend
20 Minuten nur passiv. Ergebnis `passive_deadline_no_symptom`; danach absichtlicher
Shutdown, keine Spiel-/Server-Prozesse übrig. Der historische Fensterfehler wurde
nicht reproduziert und gilt nicht als behoben.
Der anschließend separat genehmigte instrumentierte source-blinde Score-30-Lauf
unter `target/alien-score-instrumented-1/` ist **bestanden: Live-Score 31,
17 Kuchen**. Frischer Sol 6.1 Medium, nur `game`/`contact_supervisor`, null
Quellzugriffsversuche, vier relevante Captures, 1.931 explizite Agent-Ticks und
keine Parent-Ticks. Keine Ticks nach Erfolgsnachweis; unabhängiger Parent-Inspect
bestätigte identisches Game, Gewinn-PNG zeigt `Sugar Rush: 31`. Recording sauber
mit 448 Commands abgeschlossen; Session/Server/Watchdog beendet, Prozesse absent.
Keine Fenster-Komponentenverluste oder Winit-Query-Warnungen in diesem Lauf;
die historischen Ursachen bleiben ungeklärt, kein Fenster-Fix behauptet.
Temporäre opt-in Beobachter (`[DEBUG-window-loss-20261005]`) samt sechs geprüften
Headless-Diagnose-Seams archiviert unter `target/window-loss-diagnosis/instrumentation/`
und danach aus dem SDK entfernt. Nach Cleanup 145 Library-/25 Integrationstests
seriell und striktes Clippy bestanden. Erster paralleler Cleanup-Testlauf hatte
einen Before-Ready-SIGKILL im Session-Fixture; Fehlerlog erhalten, isolierter und
serieller Gesamtlauf bestanden ohne Deadlineänderungen, Ursache nicht behauptet.
Der Spielquellcode wurde für Diagnose/Fix nicht gelesen oder geändert.
Score-30-Abnahme gilt für den erhaltenen instrumentierten Lauf; Cake-Modell-
Pixelabnahme separat, soweit nicht durch frühere Captures bestätigt.
SDK-Fix und minimale Alien-Anbindung werden mit dem ausdrücklich freigegebenen
Abschluss-Commit versioniert. Die lokale Evidenz unter `target/` bleibt unversioniert.

### Git-Abschluss: Merge-Zuordnung offen

Die Vorprüfung zum Abschluss bestätigt `upstream` als
`https://github.com/timjonaswechler/bug_hunter.git`; `main` und `upstream/main`
stehen auf `2bca2df`. Der offene Draft-[PR #3 „Code ownership“](https://github.com/timjonaswechler/bug_hunter/pull/3)
zielt von `code_ownership` auf `main`, nicht von `reference/window`.
Sein Head `b89d781` enthält fünf zusätzliche Commits seit dem gemeinsamen Stand
`7378230`, darunter die abweichende Headless-Implementierung. Diese gehören nicht
zum hier abgenommenen Fenster-Stand. Deshalb sind Merge und PR-Abschluss bis zur
Klärung dieser Branch-Zuordnung ausgesetzt; kein Force-Push, Reset oder Verwerfen
der abweichenden Arbeit. Format- und Diff-Prüfung wurden vor dem Abschluss-Commit
erneut bestanden; die oben genannten seriellen Tests und Clippy sind durch die
erhaltenen Cleanup-Logs belegt, nicht durch einen neuen Grafiklauf.

## Vorheriger Abschluss-Checkpoint

Die [Testordnung](../../tests/README.md) ist umgesetzt: Rust-Integrationstests unter
`tests/integration/`, Python-Abnahmen unter `tests/acceptance/headless/` und
`tests/acceptance/rendered/`, kleine Prüforakel unter `tests/unit/`, gemeinsame
Hilfen unter `tests/support/`. Lifecycle und automatische Reports laufen jetzt
in **einer** headless Abnahme mit sechs Sessions statt zwei getrennten Runnern.
Alle vier neuen headless Runner und der komplette serielle Root-Testlauf bestanden.
Sieben Prüforakel bestehen ebenfalls; inzwischen sind auch alle sieben grafischen
Abnahmen nach frischem Build und ausdrücklicher Freigabe bestanden. Die unten genannten
alten Dateinamen gehören zu historischen Läufen; aktuelle Befehle im Test-README.
Neuordnung und fortgeschriebene Dokumentation sind Teil dieses Abschluss-Checkpoints;
Nutzerdateien bleiben unverändert und außerhalb des Commits.

Mit [Abschnitt 4: Last, Ergebnislücken und Shutdown](implementation-plan.md#4-last-ergebnislücken-und-shutdown-prüfen)
ist mit vier gezielten Nachweisen abgeschlossen. Auch die
[Schlussprüfung](implementation-plan.md#grafische-schlussprüfung-77-bestanden) ist
für den aktuellen stabilen Fensterumfang abgeschlossen. Historischer Headless-Stand:
Headless-Prüfstand auf Commit `eeeb8d0` ist durchgeführt: 141 Library- und 25
Integrationstests mit allen Features (seriell), 92 ohne Features, 10 Tests des
separaten Bevy-Pakets, Einzel-Feature-Checks, Doctests und alle fünf headless CLI-
Abnahmen bestanden. Die früher hängenden Session-Tests sind einzeln und im seriellen
Gesamtlauf grün; ihre frühere Ursache bleibt ungeklärt. Root-Clippy bestanden.

Der zusätzliche strenge **Bevy-Clippy**-Befund ist nach ausdrücklicher Freigabe
behoben: Query-Aliase, gebündelte Systemparameter und direkte Testinitialisierung,
keine Suppression und keine Timing-/Fensteränderung. Der vollständige Lauf mit
`--all-features --all-targets -- -D warnings`, alle 10 Bevy-Tests, der Check ohne
Features und Format-/Diff-Prüfung bestanden. Logs: `target/clippy-cleanup/`;
der ursprüngliche Fehler bleibt in `target/final-validation/bevy-clippy.log`
und der erneuten Reproduktion `target/clippy-cleanup/before.log` erhalten.
Die neuen grafischen Abschlussläufe sind **7/7 bestanden**. Übersicht und Cleanup:
`target/graphical-final-validation/summary.json`; Logs
`target/final-validation/graphical-final-*.{json,log}`. Investigation bestand unter
`target/investigation-dkaj5b6i/` mit zwei separat vom Nutzer bestätigten Fenstern,
je fünf Ticks und sechs pixelgleichen PNGs. Der vorherige Bestätigungs-Timeout
`target/investigation-xld_z4qa/` bleibt erhalten; der neue Versuch wurde ausdrücklich
freigegeben. Keine eigenen Spiel-/Serverprozesse bleiben aktiv.

Die Testneuordnung, gezielte Clippy-Bereinigung und Doku-Fortschreibung werden mit
diesem Abschluss-Checkpoint auf ausdrücklichen Nutzerauftrag versioniert.
Der aktuelle Umfang ist abgenommen; weitere Produktarbeit erst nach neuem Auftrag.
Lokale Evidenz unter `target/` ist nicht in Git enthalten. Nutzerdateien weiterhin
ausschließen. Keine weiteren GUI-Läufe ohne neue Freigabe.
Keine weitere Queue-Optimierung; die unbeschränkte Queue bleibt eine
[dokumentierte Betriebsgrenze](implementation-plan.md#report-queue-bemessen-betriebsgrenze-dokumentiert).

[Echter separater `panic=abort`-Build](implementation-plan.md#separater-echter-panicabort-build-bestanden)
bestand mit compile-time Strategieprüfung und normalem `panic!` statt explizitem
`process::abort()`: ein Panic-Failure vor SIGABRT-Ende, kein doppelter Exit-Failure,
offener Tick endet mit `Error::Ended`, vollständiger lokaler Report und Prozess weg.
Evidenz `target/panic-abort-acceptance-78733/`, Logs `target/panic-abort-validation/`.
Gezielter Observation-Regressionslauf, Root-/Fixture-Clippy und Format-/Diff-Prüfung
bestanden. Keine Produktänderung, kein Grafiklauf.

[Mehrsession-Shutdown unter laufender Arbeit](implementation-plan.md#mehrsession-shutdown-unter-laufender-arbeit-bestanden)
bestand: zwei Sessions mit offenen 1.000-Tick-Warps und blockierten Reports;
geordnetes Ende, gemeinsame 800-ms-Frist und erzwungener Abbruch. Alle Spiel-/
Provider-/Unterprozesse verschwunden, alle Commands aufgelöst, unterbrochene Reports
korrekt gemeldet. Evidenz: `target/server-report-tests/3b463ad9a5a57c8b4160b8954b4a9db3/`,
Logs `target/multisession-validation/`. Erweiterter Test, Clippy und Format-/Diff-Prüfung
bestanden. Keine Produktänderung und kein Grafiklauf.

Die Queue-Bemessung ist abgeschlossen: Bei blockiertem Provider
entstanden 64 Reports (ein aktiver, 63 weitere nicht abgeschlossen), während Ticks
und Failure-Events weiterliefen. Nach Freigabe wurden alle 64 abgearbeitet; Server-RSS
stieg von 17.376 auf 20.400 KiB (nicht nur Queue-Speicher). Evidenz:
`target/report-queue-load/fc84e5616ddb13bb1d149203eacaa757/`, Logs
`target/report-queue-validation/`. Activity-Regression und Clippy bestanden.

Die Queue ist unbeschränkt; dieser begrenzte Test belegt **keine** Dauerlastsicherheit.
Eine neue Überlastregel bleibt optional für tatsächlichen Bedarf, nicht als nächste
Abschlussaufgabe. Keine Queue-Grenze oder stille Drop-Regel ergänzt.

Der erste Lastpunkt ist abgenommen: `tests/integration/activity_retention.rs` mit echtem headless Bevy-
Prozess prüft 5-MiB-Inspect, vollständige 2-MiB-Ergebnisse, verzögerten Activity-Abruf,
eine nicht lesende WebSocket-Verbindung und einen 11.540.769-Byte-Report gegen die
unveränderte 4-MiB-Activity-Grenze. Verlorene Outcomes bleiben unbekannt; Session und
anderer Client bleiben steuerbar. Evidenz: `target/activity-load-1337/result.json`,
Logs `target/activity-validation/`. Acht Script-Tests für Ergebnisbehandlung,
Root-Clippy, Format-/Diff-Prüfung bestanden ebenfalls. Kein Grafiklauf und keine
Produktänderung. Details und Befehle im [Abschlussplan](implementation-plan.md#activity-grenze-mit-echten-großen-ausgaben-abgenommen).

Der [vollständige Kombinationslauf](implementation-plan.md#vollständiger-kombinationslauf-bestanden)
`python3 tests/investigation.py` bestand nach frischer GUI-Freigabe unter
`target/investigation-ijz15dy0/`. Zwei Sessions mit je fünf expliziten Ticks,
sechs pixelgleichen 640×360-Captures, reproduziertem Fixture-Fehler und gleicher
Report-Signatur; JSONL-Footer, unveränderliche Reports/Markdown und Prozessbereinigung
bestanden. `result.json` bestätigt den Erfolg. Alle Testprozesse sind beendet.

Die Bildschirmplatzierung bleibt auf Nutzerwunsch entfernt: keine Monitorparameter,
keine verzögerte `PreStartup`-Fenstererstellung, ursprüngliche Composition-Schnittstelle.
Historische Fehler und gestoppte Versuche stehen im Abschlussplan; sie werden durch
den bestandenen Lauf nicht rückwirkend als bestanden umgedeutet.

Vor dem Grafiklauf bestanden nach Rücknahme zwei Slice-Szenentests, vier Python-
Orakeltests, Szenen-Clippy, CLI-/Slice-Builds sowie Format-/Syntax-/Diff-Prüfung.
Logs/Archiv: `target/monitor-placement-rollback/`. Weitere Grafik nur nach neuer
Freigabe; Fenster während Sessions unverändert lassen.

Headless: neuer Session-Test für unerwartetes Ende während Replay/Recording,
beide Szenentests und `tests/slice.py` (vier Sessions) bestanden; Clippy und Builds
ebenfalls. Logs: `target/investigation-validation/`.
**Offener Prüfblocker:** Der komplette Session-Testlauf hing bei zwei bestehenden
Tests und wurde nach 200 Sekunden abgebrochen (14 bestanden, zwei unvollständig).
Eigene Restprozesse bereinigt; Ursache nicht geklärt. Details im Abschlussplan.
Keinen grünen Gesamtprüfstand behaupten und nicht unverändert wiederholen.

Danach Last-/Fehlerfälle und die abschließenden Feature-, Vertrags- und
Dokumentationsprüfungen ausführen. Ein Root-Build ersetzt keinen Build des
separaten Bevy-Test-App-Pakets.

## Frisch nach der Rücknahme geprüft

110 Library-Tests mit `screenshot,ui`, 92 ohne Features, drei Blend-Tests,
Root-Clippy, statische Schedule-Probe, Syntax-/Format-/Diff-Prüfung und
CLI-/Blend-Slice-Builds bestanden. Keine Grafik ausgeführt. Logs und Prüfung der
erhaltenen vorherigen Diffs: `target/resize-rollback-validation/`.

## Erhaltene Nachweise und Änderungen

- Headless-Screenshot-/Dateisicherheit einschließlich Timeout-Fix, Request-Isolation
  und Symlink-Sicherheit: `target/screenshot-validation/`, [Prüfstand](implementation-plan.md#headless-readback-frist-und-dateisicherheit).
- Blend mit UI-Overlay/Mehrkamera-Ausgabe bei stabilem Fenster:
  `target/blend-modes-xz52actk/`. Mesh-/UI-Abnahmen stehen ebenfalls im Abschlussplan.
- Steuerungsnachweise einschließlich Drag-Despawn, Mesh-Rotation, Context-Menü,
  EOF-/Quit-Report-Fix und Session-Isolation bleiben erhalten.
- Der separat beobachtete Library-Start-Timeout vor Ready ist nicht als behoben
  anzusehen. Ein gezielter Lauf nach Fixture-Build bestand; Details im Plan.
- Der zurückgenommene Resize-Versuch und die zugehörigen Fehlerbilder sind keine
  aktuelle Produktabnahme. Lokale Archivkopie vor Rücknahme:
  `target/resize-rollback-snapshot/`; native Evidenzpfade stehen in der Diagnose.
- Lokale `target/`-Builds und Evidenz werden nicht durch Git übertragen.

## Arbeitsregeln

- Nutzeränderungen erhalten. Commits und Subagenten nur nach ausdrücklicher
  Zustimmung. Neue Produktentscheidungen und Vertragskonflikte vorlegen;
  reversible Implementierungsdetails innerhalb des Auftrags selbst entscheiden.
- Grafiktests nur nach Freigabe und nacheinander ausführen. Eigene Prozesse
  auch bei Fehlern bereinigen und Artefaktpfade protokollieren.
- Simulation und Eingaben bleiben an explizite Warps gebunden. Keine
  Fokusänderung, versteckten Startticks, Schlafzeiten als Fix oder Wiederholungen
  bis zum ersten Erfolg. Schwarze Pixel sind keine Fehlerheuristik.
- Vollverdeckung ist kein Abschlussblocker. Eine Surface-Guard-Ablehnung ist
  zulässig, aber keine erfolgreiche Bildaufnahme.
- Kein Upstream-Issue oder PR ohne Freigabe veröffentlichen. Entwicklungstests
  nutzen lokale Reports oder isolierte `gh`-Fixtures, keine echten Issues.
- GitHub-URL und versionierte Report-Signaturbezeichner unverändert lassen;
  der Nutzer koordiniert deren Anpassung mit einem anderen Projekt.
- Fachliche Modulhierarchien statt wiederholter Symbol-/Dateipräfixe verwenden.
  Bei beauftragten Commits gelten atomare, unabhängig baubare Conventional Commits:
  kleingeschriebene Beschreibung, kein Schlusspunkt, Header höchstens 100 Zeichen;
  Breaking Changes mit `!` und `BREAKING CHANGE:`. `Fixes`/`Closes` nur beim
  tatsächlichen Schließen eines direkt zugehörigen Issues.
- Bekannte Umgebungsfehler und zurückgestellte Arbeit aus dem
  [Abschlussplan](implementation-plan.md#zurückgestellte-arbeit-und-bekannte-grenzen)
  beachten. Insbesondere keine Sicherheitsregeln für macOS-Fixtures ändern.

### Strenges Clippy der Bevy-Szenen

Die früheren gezielten Ausnahmen sind nicht mehr nötig. Ohne Suppression prüfen:

```sh
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets -- -D warnings
```

Frühere Ausnahmebefehle in historischen Nachweisen bleiben als damaliger Prüfstand
sichtbar; sie sind keine aktuelle Prüfempfehlung. Build- und Test-App-Hinweise stehen im
[Bevy-README](../../bevy_test_apps/README.md).
