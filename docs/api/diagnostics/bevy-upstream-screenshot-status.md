# Bevy-Upstream-Status des Occlusion-Screenshotfehlers

Quellenabfrage vom 2026-09-21 um 17:47 UTC. Die Aussagen beziehen sich auf die
unten genannten Revisionen, nicht auf einen späteren Upstream-Stand.
Ausgewertet wurden öffentliche Releases, Tags, Commits, Quellen, Issues und
Pull Requests von `bevyengine/bevy` über GitHub und die GitHub-API.

## Ergebnis

Der in [black-screenshots.md](black-screenshots.md) beschriebene
Prepare-/Skip-Copy-/Collect-Pfad war in `v0.20.0-rc.1`, `release-0.20.0` und
dem abgefragten `main` weiterhin vorhanden. Die Suche fand keinen Issue oder
PR, der genau diesen Window-Screenshot-Pfad beschreibt oder behebt. Das ist
keine Garantie vollständiger Issue-Erfassung.

Die damalige Quellenprüfung enthielt keine Laufzeittests. Das anschließend
abgeschlossene [eigenständige RC-Repro](screenshot-evidence.md) bestätigte den
Fehler auf macOS/Metal mit `v0.20.0-rc.1` und wgpu 30.0.1. Es war keine
woodpecker-Migration und prüfte nicht das Verhalten des Guards unter Bevy 0.20.

## Geprüfte Revisionen

| Referenz | Fester Stand | Befund zur Abfragezeit |
| --- | --- | --- |
| [`v0.19.1`](https://github.com/bevyengine/bevy/tree/b56fc29d3016e641754765244b5ba3f9cc504671) | `b56fc29d3016e641754765244b5ba3f9cc504671` | Ausgangsstand des Projekts |
| [`v0.20.0-rc.1`](https://github.com/bevyengine/bevy/releases/tag/v0.20.0-rc.1) | [`1b1f3ec1bec87386d7c19bda7d7870d4e18235c5`](https://github.com/bevyengine/bevy/commit/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5) | Veröffentlicht am 2026-09-15; einziger gefundener 0.20-Prerelease |
| `release-0.20.0` | `1b1f3ec1bec87386d7c19bda7d7870d4e18235c5` | Identisch zum RC-Tag |
| [`main`](https://github.com/bevyengine/bevy/tree/1a6377e33b26b6d744f96e35dbe9392e86602ac3) | `1a6377e33b26b6d744f96e35dbe9392e86602ac3` | Damals abgefragter Branch-Head |

Die Contents-API lieferte für RC, Release-Branch und `main` identische Blobs
für `screenshot.rs`, Präfix `54c3dead`, und `window/mod.rs`, Präfix `279569fd`.

## Codevergleich

Alle Zeilen beziehen sich auf `crates/bevy_render/src/view/window/`:

| Schritt | Bevy 0.19.1 | Bevy 0.20.0-rc.1 | Bewertung |
| --- | --- | --- | --- |
| View nach Present entfernen | [`mod.rs:150-157`](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/mod.rs#L150-L157) | [`mod.rs:151-159`](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/mod.rs#L151-L159) | Kein alter View als Ersatz |
| Surface-Akquise bei `Occluded` | [`mod.rs:300-334`](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/mod.rs#L300-L334) | [`mod.rs:304-340`](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/mod.rs#L304-L340) | Kein View, kein Screenshot-Fehlerstatus |
| Textur und Buffer vorbereiten | [`screenshot.rs:266-310`](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L266-L310) | [`screenshot.rs:322-365`](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/screenshot.rs#L322-L365) | Vorbereitung braucht keine erfolgreiche Akquise |
| Ohne Format oder View überspringen | [`screenshot.rs:498-533`](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L498-L533) | [`screenshot.rs:558-599`](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/screenshot.rs#L558-L599) | `render_screenshot` entfällt |
| Einziger Buffer-Copy | [`screenshot.rs:583-608`](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L583-L608) | [`screenshot.rs:648-678`](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/screenshot.rs#L648-L678) | Liegt in der übersprungenen Funktion |
| Alle vorbereiteten Buffer einsammeln | [`screenshot.rs:631-702`](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L631-L702) | [`screenshot.rs:696-767`](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/screenshot.rs#L696-L767) | Kein Nachweis eines ausgeführten Copys erforderlich |

## Verwandte Issues und Pull Requests

Gesucht wurde in offenen und geschlossenen Issues und PRs nach Screenshot-,
Black-/Blank-, Occlusion-, Surface- und Readback-Begriffen sowie den betroffenen
Funktionsnamen. Die folgenden Treffer erklären verwandte Mechanismen;
Statusangaben gelten für den Abfragezeitpunkt.

| Quelle | Einordnung |
| --- | --- |
| [#23504](https://github.com/bevyengine/bevy/issues/23504), offen; [#11229](https://github.com/bevyengine/bevy/issues/11229), offen | Allgemeine Occlusion-/Renderoptimierung, kein Skip-Copy-Screenshotfix. |
| [PR #23277](https://github.com/bevyengine/bevy/pull/23277), gemergt | wgpu-29-Update mit `Occluded => {}`; änderte `screenshot.rs` nicht. In allen geprüften Ständen enthalten. |
| [#25215](https://github.com/bevyengine/bevy/issues/25215), offen; [PR #25229](https://github.com/bevyengine/bevy/pull/25229), ohne Merge geschlossen | `Screenshot::image` ohne Kamera kopiert eine leere Textur. Ähnliches Ergebnis, aber kein ausgelassener Window-Copy. Der [Vorschlag](https://github.com/bevyengine/bevy/commit/814924e90a01441ce5d7b34c6d4c25f6864f24ec) war weder im RC noch in `main`. |
| [#18229](https://github.com/bevyengine/bevy/issues/18229), offen | Alpha-/Transparenzproblem, nicht Surface-Akquise. |
| [#16689](https://github.com/bevyengine/bevy/issues/16689), offen | Fehlendes `bevy_egui` im Screenshot, laut Bericht in `bevy_egui` gelöst; relevant für alternative Ausgabe-Texturen, nicht Occlusion. |
| [PR #14833](https://github.com/bevyengine/bevy/pull/14833), gemergt | Ursprung des Screenshot-Zwischenziels und asynchronen Einsammelns. Kein Fix bei fehlendem View. |
| [PR #23276](https://github.com/bevyengine/bevy/pull/23276), gemergt | Überspringt Akquise ohne schreibende Fensterkamera, nicht das hier untersuchte Fenster mit Kamera. |
| [PR #15087](https://github.com/bevyengine/bevy/pull/15087), gemergt | Resize-/DX12-Korrektur; lässt die Prepare-/Skip-/Collect-Lücke unverändert. |
| [#8604](https://github.com/bevyengine/bevy/issues/8604), geschlossen; [PR #8701](https://github.com/bevyengine/bevy/pull/8701), gemergt | Historischer Wayland-/Nvidia-sRGB-Formatkonflikt mit Crash, kein Nullbuffer ohne Copy. |
| [#19782](https://github.com/bevyengine/bevy/issues/19782), geschlossen; [PR #22077](https://github.com/bevyengine/bevy/pull/22077), gemergt | Ungültige PNG-Daten im JavaScript-/Web-Pfad, keine Surface-Verdeckung. |

## Folgerung und Grenzen

Der Versionssprung auf den geprüften RC beseitigte diesen Fehler nicht.
Den Guard deshalb nicht allein wegen eines Bevy-Upgrades entfernen.
Vor einem späteren Upgrade die tatsächlich gewählte Revision und die
[Capture-Regressionen](screenshot-evidence.md#werkzeuge-und-befehle) erneut prüfen.

Die Suche kann anders benannte oder gelöschte Inhalte übersehen. Der Befund
lautet daher nicht, dass der Fehler niemals gemeldet wurde. Es wurde kein Issue
oder PR veröffentlicht. Eine Veröffentlichung benötigt weiterhin Freigabe.
Der aktuelle Umfang akzeptiert die Ablehnung bei fehlender Renderoberfläche;
siehe [Zielvertrag](../target.md#screenshot).
