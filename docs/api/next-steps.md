# Weiterarbeit an woodpecker

## Aktueller Stand

Session-/Prozessverwaltung, explizite Warps, virtuelle Eingaben, allgemeines
Inspect, Screenshot-Grundfunktion, Recording/Replay und Reports sind implementiert.
Server, Client, CLI, REPL und Scripts verwenden denselben Command-/Activity-Weg.
Alle sieben Bevy-Szenen besitzen CLI-Abnahmen.

Offen sind einzelne Steuerungsfälle sowie kombinierte Lifecycle- und Lastabnahmen.
Capture bei vollständig verdecktem Fenster ist nicht zugesichert und kein
Abschlussblocker. Bei fehlender Renderoberfläche bleibt die explizite Guard-Ablehnung;
ein tatsächlich gerendertes schwarzes Bild ist gültig.
Der [Abschlussplan](implementation-plan.md) besitzt die
[Checkliste für den schnellen Überblick](implementation-plan.md#checkliste-für-den-abschluss)
und die [Abdeckungsmatrix](implementation-plan.md#abdeckungsmatrix).
Den Aufgabenstatus nur dort pflegen; die Beispiele stehen bei der
[Steuerungsabnahme](implementation-plan.md#1-steuerungsabnahme-vervollständigen).

Beim Einstieg `git status --short` und den aktuellen Branch prüfen; Änderungen
und unversionierte Dateien erhalten. Den [Zielvertrag](target.md) bei Verhaltensfragen,
[goal.rs](goal.rs) für die Interface-Skizze und [usage.md](usage.md) für Bedienung
und Build-Befehle verwenden. Vorhandene Szenen nicht erneut anbinden.

## Nächste Aktion

Die Steuerungsabnahme von `game_menu` vervollständigen:
[Beispiel 2 im Abschlussplan](implementation-plan.md#beispiel-2-tastenkürzel-und-quit-in-game_menu).

1. `tests/game_menu.py` und die bestehende Szene lesen; Kurzwege `s`, `Escape`,
   `n`, Quit-Button und technische Screenshots ergänzen.
2. Vor expliziten Warps unveränderten Zustand prüfen, nach Bildschirmwechseln
   tote Handles. Quit muss als unerwartetes Prozessende gemeldet werden;
   Server und andere Sessions müssen bedienbar bleiben.
3. Ohne Grafik bauen, `BUILD_READY` melden und vor grafischer Abnahme auf
   `GUI_FREIGABE` warten. Commands, Ergebnisse und Artefaktpfade protokollieren.

Der frische `logical_state`-Nachweis steht bei
[Beispiel 1 im Abschlussplan](implementation-plan.md#beispiel-1-mausklick-in-logical_state).
Weitere offene Szenenfälle stehen in der
[Abdeckungsmatrix](implementation-plan.md#abdeckungsmatrix).
Lokale `target/`-Builds und Evidenz sind nicht im Git-Transfer enthalten.

## Weitere Validierungsfolge

1. Die übrigen Steuerungsfälle und die technische Screenshot-Abnahme einschließlich
   Dateisicherheit und separater Resize-/Kameralücke vervollständigen.
2. Den kombinierten Recording-/Failure-/Report-/Replay-/Stopp-Ablauf und Lastfälle prüfen.
3. Root-Suite, separates Bevy-Paket, Feature-Builds und CLI-Abnahmen ausführen;
   anschließend Vertrags- und Dokumentationsabgleich abschließen.

Die detaillierten Abschlussbedingungen stehen im Abschlussplan. Bei fehlender
Renderoberfläche ist eine Guard-Ablehnung zulässig, aber keine erfolgreiche
Bildaufnahme. Die historischen Diagnoseergebnisse bleiben in den
[Screenshotnachweisen](diagnostics/screenshot-evidence.md) erhalten.

## Letzter dokumentierter Prüfstand

Dies sind historische Ergebnisse, keine neue Prüfung des aktuellen Arbeitsbaums:

- Nach Screenshot-Absicherung bestanden 136 Bibliotheks-/CLI-, 7 Beobachtungs-
  und 13 Session-Prozesstests. Ohne Default-Features bestanden 92 Tests;
  Screenshot-Tests, Root-Clippy mit `-D warnings`, Format- und Diff-Prüfung bestanden.
- Die späteren vollständigen Blend-, Mesh- und UI-Sichtbarkeitstests bestanden
  sichtbar, teilweise verdeckt und nach Wiederherstellung. UI schloss Recording,
  Replay und zwei Sessions ein. Vollverdeckung scheiterte in allen drei Szenen
  mit `screenshot_window_unavailable` und bleibt eine nicht bestandene Bildabnahme.
- Das eigenständige Bevy-`v0.20.0-rc.1`-Repro bestätigte den Skip-Copy-/Nullbuffer-
  Pfad auf Metal ohne woodpecker-Adapter.

Versionsstände, einzelne ungültige Läufe und Artefaktpfade stehen in den
[Screenshotnachweisen](diagnostics/screenshot-evidence.md). Frühere sonstige
CLI-Abnahmen liegen unter `target/logical-state-9q3u1qau`,
`target/game-menu-1go85k9z` und `target/ui-drag-drop-uhob_pu5`.
Die acht nativen App-/Kompositionstests bestanden zuletzt beim Blend-Durchstich;
der spätere Diagnosebuild prüfte nur den Library-Test der kopierten Testanwendungen.
Ein Root-Build deckt das separate Test-App-Paket nicht ab.

## Arbeitsregeln

- Nutzeränderungen erhalten. Commits und Subagenten nur nach ausdrücklicher
  Zustimmung. Neue Produktentscheidungen und Vertragskonflikte vorlegen;
  reversible Implementierungsdetails innerhalb des Auftrags selbst entscheiden.
- Grafiktests nur nach Freigabe und nacheinander ausführen. Eigene Prozesse
  auch bei Fehlern bereinigen und Artefaktpfade protokollieren.
- Simulation und Eingaben bleiben an explizite Warps gebunden. Keine
  Fokusänderung, versteckten Startticks, Schlafzeiten als Fix oder Wiederholungen
  bis zum ersten Erfolg. Schwarze Pixel sind keine Fehlerheuristik.
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

### Vorläufige Clippy-Ausnahmen der Bevy-Szenen

Nur gezielt beim jeweiligen Target verwenden, keine `allow`-Attribute oder
Cargo-Features ergänzen. Nach einem Bevy-Update zuerst ohne Ausnahmen prüfen:

```sh
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin game_menu \
  -- -D warnings -A clippy::type_complexity -A clippy::too_many_arguments
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin mesh_picking \
  -- -D warnings -A clippy::type_complexity
cargo clippy --manifest-path bevy_test_apps/Cargo.toml --features slice --bin blend_modes \
  -- -D warnings -A clippy::too_many_arguments
```

Root-Clippy bleibt ohne diese Ausnahmen. Build- und Test-App-Hinweise stehen im
[Bevy-README](../../bevy_test_apps/README.md).
