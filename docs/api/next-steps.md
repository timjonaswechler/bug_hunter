# Weiterarbeit an woodpecker

## Aktueller Stand

Session-/Prozessverwaltung, explizite Warps, virtuelle Eingaben, Inspect,
Screenshot-Grundfunktion, Recording/Replay und Reports sind implementiert.
Alle sieben Bevy-Szenen besitzen CLI-Abnahmen; die ergänzte Steuerungsabnahme
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

## Nächste Aktion

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
