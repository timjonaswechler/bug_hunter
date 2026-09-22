# Bevy-Upstream-Status des Occlusion-Screenshotfehlers

Stand: 2026-09-21 17:47 UTC. Ausschließlich öffentliche Primärquellen aus
`bevyengine/bevy` wurden ausgewertet: Releases, Tags, Commits, Quellcode,
Issues und Pull Requests über GitHub und die GitHub-API.

## Ergebnis

**Der diagnostizierte Mechanismus ist sowohl in Bevy `v0.20.0-rc.1` als auch
im abgefragten `main` unverändert vorhanden.** Die gefundenen Upstream-Issues
behandeln `CurrentSurfaceTexture::Occluded` als allgemeines Render- und
Performance-Thema, nicht die daraus entstehende Screenshot-Prepare/Skip/Collect-Lücke. Die
Suche fand keinen offenen, geschlossenen oder gemergten Bevy-Issue/PR, der den
exakten Window-Screenshot-Mechanismus beschreibt oder behebt.

Das ist ein Suchbefund, keine Garantie, dass der Fehler nie gemeldet wurde.

## Tatsächlich vorhandene Releases und Referenzen

Die Release- und Tagabfrage erfolgte vor dem Codevergleich. Unter `v0.20` gibt
es genau einen öffentlichen Tag und Release:

| Referenz | GitHub-Stand | Befund |
| --- | --- | --- |
| [`v0.19.1`](https://github.com/bevyengine/bevy/tree/b56fc29d3016e641754765244b5ba3f9cc504671) | Commit `b56fc29d3016e641754765244b5ba3f9cc504671` | Bekannter Ausgangsstand |
| [`v0.20.0-rc.1`](https://github.com/bevyengine/bevy/releases/tag/v0.20.0-rc.1) | Veröffentlicht am 2026-09-15, Tag-Commit [`1b1f3ec`](https://github.com/bevyengine/bevy/commit/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5) | Einziger gefundener `v0.20`-Tag und Prerelease; kein `rc.2` oder finales `v0.20.0` vorhanden |
| [`release-0.20.0`](https://github.com/bevyengine/bevy/tree/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5) | Head `1b1f3ec`, identisch zum RC-Tag | Keine neueren Änderungen hinter dem RC |
| [`main`](https://github.com/bevyengine/bevy/tree/1a6377e33b26b6d744f96e35dbe9392e86602ac3) | Head [`1a6377e`](https://github.com/bevyengine/bevy/commit/1a6377e33b26b6d744f96e35dbe9392e86602ac3) | Abgefragter Live-Stand |

GitHubs Contents-API liefert für RC, Release-Branch und `main` dieselben Blobs:
`54c3dead…` für
[`screenshot.rs`](https://github.com/bevyengine/bevy/blob/1a6377e33b26b6d744f96e35dbe9392e86602ac3/crates/bevy_render/src/view/window/screenshot.rs)
und `279569fd…` für
[`window/mod.rs`](https://github.com/bevyengine/bevy/blob/1a6377e33b26b6d744f96e35dbe9392e86602ac3/crates/bevy_render/src/view/window/mod.rs).
Der folgende RC-Befund gilt deshalb bytegleich für `main`.

## Codevergleich

| Schritt | `v0.19.1` | `v0.20.0-rc.1` und `main` | Bewertung |
| --- | --- | --- | --- |
| Nach einem präsentierten Frame wird die alte View entfernt | [`extract_windows`, Zeilen 150–157](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/mod.rs#L150-L157) | [`extract_windows`, Zeilen 151–159](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/mod.rs#L151-L159) | Gleiches Verhalten |
| Surface-Akquise bei Verdeckung | [`CurrentSurfaceTexture::Occluded => {}`, Zeilen 300–334](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/mod.rs#L300-L334) | [`CurrentSurfaceTexture::Occluded => {}`, Zeilen 304–340](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/mod.rs#L304-L340) | Keine View wird gesetzt, kein Fehlerzustand an Screenshots weitergegeben |
| Screenshottextur und Buffer werden vorbereitet | [`prepare_screenshots`, Zeilen 266–310](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L266-L310) | [`prepare_screenshots`, Zeilen 322–365](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/screenshot.rs#L322-L365) | Vorbereitung hängt an `SurfaceData`, nicht an einer erfolgreichen Akquise |
| Window-Zweig überspringt den Screenshot | [`continue` bei fehlendem Format oder View, Zeilen 498–533](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L498-L533) | [Dieselben beiden `continue`, Zeilen 558–599](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/screenshot.rs#L558-L599) | `render_screenshot` wird nicht aufgerufen |
| GPU-Copy liegt nur in `render_screenshot` | [`copy_texture_to_buffer`, Zeilen 583–608](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L583-L608) | [`copy_texture_to_buffer`, Zeilen 648–678](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/screenshot.rs#L648-L678) | Das `continue` überspringt die einzige Copy dieses Pfads |
| Alle vorbereiteten Buffer werden eingesammelt | [`collect_screenshots`, Zeilen 631–702](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/src/view/window/screenshot.rs#L631-L702) | [`collect_screenshots`, Zeilen 696–767](https://github.com/bevyengine/bevy/blob/1b1f3ec1bec87386d7c19bda7d7870d4e18235c5/crates/bevy_render/src/view/window/screenshot.rs#L696-L767) | Keine Kennzeichnung, ob die Copy aufgezeichnet wurde |

Damit gilt weiterhin: `prepare_screenshots` trägt einen vorbereiteten Zustand
ein, der Window-Zweig kann vor `render_screenshot` abbrechen, und
`collect_screenshots` mappt den vorbereiteten Buffer trotzdem. Zwischen RC und
`main` gibt es in diesen beiden Dateien nicht einmal eine Blob-Differenz.

## Issues und Pull Requests

Gesucht wurde in offenen und geschlossenen Issues sowie PRs nach Kombinationen
aus `screenshot(s)`, `black`, `blank`, `occlusion`, `occluded`, `minimized`,
`surface`, `readback`, `covered`, `hidden`, `macOS` und den Codebegriffen
`CurrentSurfaceTexture::Occluded`, `swap_chain_texture_view`,
`prepare_screenshots`, `collect_screenshots`, `copy_texture_to_buffer` und
`map_async`. Die Tabelle enthält die kausal relevanten Treffer, nicht bloß jede
Erwähnung des Wortes "screenshot".

| Quelle und Status | Einordnung | Tag- und Codeprüfung |
| --- | --- | --- |
| [Issue #23504](https://github.com/bevyengine/bevy/issues/23504), offen: "Track Occluded from Winit and don't attempt to render" | Gleicher Auslöser `CurrentSurfaceTexture::Occluded`, aber kein Screenshot, Readback oder vorbereiteter Buffer. Das Issue beschreibt eine mögliche Render-Optimierung. Es hat keine Kommentare und keinen verknüpften Implementierungs-PR. | Verweist auf PR #23277, nicht auf einen Screenshot-Fix. |
| [Issue #11229](https://github.com/bevyengine/bevy/issues/11229), offen: `WindowEvent::Occluded` berücksichtigen | Ähnlicher allgemeiner Verdeckungszustand. Ziel ist, Zeichnen unsichtbarer Fenster zu stoppen. Kein Screenshot- oder Readback-Mechanismus. | Offen seit 2024; kein aktueller Fix für die hier untersuchte Kette. |
| [PR #23277](https://github.com/bevyengine/bevy/pull/23277), gemergt: wgpu 29 | Direkter Vorläufer des heutigen Surface-Verhaltens. Der Merge änderte die Akquise auf `CurrentSurfaceTexture` und fügte den leeren Arm `Occluded => {}` ein; der PR bearbeitete `screenshot.rs` nicht. | Merge-Commit [`87e716f`](https://github.com/bevyengine/bevy/commit/87e716fcb515d7c6a10a28663851b86f40729a76) ist Vorfahr von `v0.19.1`, RC und `main`. |
| [Issue #25215](https://github.com/bevyengine/bevy/issues/25215), offen, und [PR #25229](https://github.com/bevyengine/bevy/pull/25229), geschlossen ohne Merge | Engstes verwandtes Fehlerbild: Ein vorbereiteter Zwischen-Render-Target bleibt unbeschrieben und der Readback wird blank. Der exakte Mechanismus ist anders. Betroffen ist `Screenshot::image` ohne Kamera; dort findet die Buffer-Copy aus einer leeren Textur statt. Beim Occlusion-Window fehlt die Copy vollständig. | Der vorgeschlagene Branch-Commit [`814924e`](https://github.com/bevyengine/bevy/commit/814924e90a01441ce5d7b34c6d4c25f6864f24ec) ändert nur den Image-Zweig. Er ist weder Vorfahr des RC noch von `main`; der aktuelle Code enthält die Änderung nicht. |
| [Issue #18229](https://github.com/bevyengine/bevy/issues/18229), offen: schwarze statt transparente Screenshots | Nur ähnliches schwarzes Symptom. Es geht um Alpha/Transparenz eines Image-Render-Targets, nicht um Surface-Akquise oder eine ausgelassene Copy. | Kein zugehöriger Fix gefunden. |
| [Issue #16689](https://github.com/bevyengine/bevy/issues/16689), offen: `bevy_egui` fehlt im Screenshot | Alternative Texture-View und externes Rendern führten zu fehlendem GUI-Inhalt. Laut Issue wurde es in `bevy_egui` gelöst. Kein Occlusion-Fall. | Verweist auf den Screenshot-Umbau PR #14833. |
| [PR #14833](https://github.com/bevyengine/bevy/pull/14833), gemergt: Screenshot-Umbau | Führte `prepare_screenshots`, den alternativen Render-Target und das spätere asynchrone Einsammeln ein. Der PR behandelt keine fehlende Swapchain-View oder Verdeckung. | Merge-Commit [`d9527c1`](https://github.com/bevyengine/bevy/commit/d9527c101c2f49c6884763cce36ea1d27dd6a597) ist Vorfahr aller drei geprüften Stände. |
| [PR #23276](https://github.com/bevyengine/bevy/pull/23276), gemergt: Akquise ohne Kamera überspringen | Betrifft ebenfalls `get_current_texture`, aber nur Fenster, auf die keine Kamera zielt. Ein normaler Window-Screenshot mit Kamera erreicht weiterhin die Akquise und den `Occluded`-Arm. | Merge-Commit [`62379cf`](https://github.com/bevyengine/bevy/commit/62379cf1eaa1b846630ada4123641d730763931f) ist bereits in `v0.19.1`, RC und `main`. |
| [PR #15087](https://github.com/bevyengine/bevy/pull/15087), gemergt: View-Attachments vor Surface-Resize leeren | Surface-nah, aber anderer Mechanismus: Resize/Neukonfiguration und DX12-Panic. Die Screenshot-Prepare/Skip/Collect-Lücke bleibt unberührt. | Merge-Commit [`2ec164d`](https://github.com/bevyengine/bevy/commit/2ec164d279d1ec5fb289df27c523299cb5fc004b) ist in allen geprüften Ständen. |
| [Issue #8604](https://github.com/bevyengine/bevy/issues/8604), geschlossen, und [PR #8701](https://github.com/bevyengine/bevy/pull/8701), gemergt | Historischer Screenshotfehler auf Wayland/Nvidia. Ursache war ein sRGB-Formatkonflikt mit Crash, nicht Occlusion oder Readback ohne Copy. | Merge-Commit [`5e3ae77`](https://github.com/bevyengine/bevy/commit/5e3ae770ac89192e73f2473468b734d414ea0e16) liegt lange vor den geprüften Tags. |
| [Issue #19782](https://github.com/bevyengine/bevy/issues/19782), geschlossen, und [PR #22077](https://github.com/bevyengine/bevy/pull/22077), gemergt | Ungültige PNG-Daten im Web-Build durch den JavaScript-Datenpfad. Kein schwarzer GPU-Readback und keine Surface-Verdeckung. | Merge-Commit [`5ffc1d6`](https://github.com/bevyengine/bevy/commit/5ffc1d6e3e18fcc31e14f3e84ca656d284efba62) ist in `v0.19.1`, RC und `main`. |

## Vorsichtiger nächster Schritt

Den lokalen Guard gegen eine fehlende Window-Surface nicht wegen des
Versionssprungs auf `0.20` entfernen. Der Code liefert keinen Anhaltspunkt für
eine Upstream-Korrektur.

Als nächste Verifikation bietet sich ein einzelner kontrollierter A/B/A-Lauf
gegen den unveränderten Tag `v0.20.0-rc.1` an, mit denselben IDs und Schranken
wie im vorhandenen Kausalitätsversuch. Erst wenn dieser Lauf den Mechanismus
erneut bestätigt, sollte ein minimales Upstream-Paket vorbereitet werden. Es
sollte #23504 als verwandtes Occlusion-Thema nennen und #25215/#25229 ausdrücklich
als ähnlichen, aber anderen Blank-Readback abgrenzen. Im Rahmen dieser Recherche
wurde nichts gepostet.

## Grenzen der Verifikation

- Auf Anweisung gab es keine GUI-, Build- oder Laufzeittests. Insbesondere wurde
  der A/B/A-Versuch nicht gegen RC oder `main` wiederholt.
- Tag-, Branch-, Blob- und Merge-Zugehörigkeit wurden live per GitHub-API
  geprüft. Die Aussage zum Fortbestand beruht auf dem identischen relevanten
  Quellcode.
- GitHub-Suche kann Treffer übersehen, etwa bei anderen Begriffen oder
  nachträglich gelöschten Inhalten. Deshalb lautet der Negativbefund nicht
  "niemals gemeldet", sondern "in der dokumentierten Suche nicht gefunden".
