# Rendering: relevante Befunde vor dem Headless-Umbau

Diese Notiz ersetzt die ausführlichen Fensterdiagnosen. Sie bewahrt belegte
Grenzen, ohne daraus weitere Verdeckungstests als Produktauftrag abzuleiten.
Das neue Ziel steht in [target.md](../target.md#headless-betrieb).

## Fenster-Capture auf macOS/Metal

Mit Bevy 0.19.1 und wgpu 29.0.4 wurde in Blend, Mesh und UI diese Kette
nachgewiesen: vollständige Verdeckung → Surface-Akquise `Occluded` →
fehlender Swapchain-View → Screenshot-Kopie übersprungen → vorbereiteter Buffer
trotzdem erfolgreich gemappt → ausschließlich Nullbytes.

Die Ursache liegt im Bevy-Fensterpfad:

1. `prepare_screenshots` bereitet eine eigene Screenshot-Textur und einen Buffer vor.
2. `submit_screenshot_commands` überspringt bei fehlendem Fenster-View
   `render_screenshot`, obwohl dort auch die unabhängige GPU-Kopie liegt.
3. `collect_screenshots` mappt die vorbereiteten Buffer ohne Nachweis der Kopie.

Quellen am festen Bevy-0.19.1-Commit:
[Prepare](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L266-L310),
[Skip und Copy](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L498-L628),
[Readback](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L631-L702).

Sichtbare, halb verdeckte und nach Vollverdeckung wieder sichtbare Aufnahmen
bestanden die vorhandenen Szenentests. Die eingefrorenen Diagnosefolgen lieferten
nach Wiederherstellung bytegleiche Bilder. Vollverdeckt scheiterten die
Bildabnahmen am woodpecker-Guard; das waren keine erfolgreichen Captures.
Die UI-Volltests prüften außerdem Recording, Replay und Sessionisolation.

Ein eigenständiges Bevy-Repro ohne woodpecker bestätigte denselben Fehler mit
[`v0.20.0-rc.1`](https://github.com/bevyengine/bevy/tree/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5)
und wgpu 30.0.1. Kein Bevy-Fix oder Dependency-Upgrade wurde übernommen.
Der aktuelle [Guard](../../../src/session/screenshot/capture/surface.rs) bleibt
bis zur Ablösung des Fensterpfads erhalten. Er ist kein Headless-Adapter.

## Linux und Renderbereitschaft

Der eingecheckte erste Linux-Lauf mit Bevy 0.19.1, Intel HD Graphics 520,
Mesa 25.2.8 und X11/Vulkan lieferte schon beim sichtbaren Fenster Nullbytes,
obwohl Akquise, Copy und Mapping erfolgreich waren. Verdeckung wurde in diesem
Lauf nicht mehr geprüft. Die Ursache des Nullbilds blieb offen.

Später auf [PR #3 berichtete Messungen](https://github.com/timjonaswechler/bug_hunter/pull/3#issuecomment-5784318315)
reproduzierten das vollständig leere Bild nicht. Der erste Capture enthielt
Draw-Skips und unvollständigen Szeneninhalt, zwei spätere Bilder waren gleich.
Das ist ein berichteter Folgeversuch, kein hier übernommener oder unabhängig
wiederholter Nachweis. Die unveröffentlichten Linux-Werkzeuge werden auf
Nutzerwunsch nicht weitergeführt.

Für den neuen Adapter bleibt die Unterscheidung wichtig:
erfolgreiche Kopie und Mapping beweisen nicht, dass alle erwarteten
Kameraausgaben bereits in der Quelltextur angekommen sind. Eine feste Zahl
Warteframes und die Prüfung auf nichtschwarze Pixel sind kein allgemeiner
Bereitschaftsvertrag.

## Upstream und Headless

Die Recherche vom 21. September 2026 fand den Fensterfehlerpfad auch im
0.20-RC und im damals geprüften `main`. Verwandte Meldungen:
[#23504](https://github.com/bevyengine/bevy/issues/23504) behandelt Occlusion,
[#25215](https://github.com/bevyengine/bevy/issues/25215) und der ungemergte
[PR #25229](https://github.com/bevyengine/bevy/pull/25229) einen anderen
Blank-Readback für Image-Ziele. Das war keine vollständige Negativgarantie
für alle Issues und ist keine Aussage über spätere Upstream-Änderungen.

Das offizielle
[Headless-Beispiel](https://github.com/bevyengine/bevy/blob/1a6377e33b26b6d744f96e35dbe9392e86602ac3/examples/app/headless_renderer.rs)
zeigt Image-Renderziel, eigene GPU-Kopie und Betrieb ohne Winit-Fenster.
Es ist eine technische Referenz, keine fertige woodpecker-Integration:
laufende App-Updates, verworfene Startframes, blockierendes GPU-Warten und
ein unbeschränkter Bildkanal passen nicht unverändert zu unserem Vertrag.

## Erhaltener Stand und Ende der Diagnosekampagne

Die vollständigen eingecheckten Werkzeuge und Berichte sind über Git erhalten:

- `7378230`: Mac-/RC-Diagnosen und Szenenmatrix;
- `dd0203f`: erster Linux-Versuch samt Werkzeugen.

Beispiel: `git show dd0203f:docs/api/diagnostics/capture-causality-linux.md`.
Lokale Messartefakte unter `target/` waren nie versioniert und werden durch
diese Bereinigung nicht gelöscht. Sie gehören nicht zum übertragbaren Testbestand.

Die umfangreiche temporäre Instrumentierung wurde aus dem aktiven Baum entfernt.
Normale Szenentests und Guard-Regressionen bleiben erhalten. Weitere
Verdeckungsmatrizen, GPU-Treiberdiagnosen und Upstream-Veröffentlichungen sind
nicht Teil des nächsten Headless-Durchstichs.
