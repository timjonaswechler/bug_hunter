# Screenshotfehler bei vollständiger Fensterverdeckung

## Bestätigter Befund

Auf dem untersuchten macOS-/Metal-System löst vollständige Fensterverdeckung
in Bevy 0.19.1 einen Screenshot-Readback ohne vorherige Bildkopie aus.
Der Buffer liefert Nullbytes. Ohne Absicherung entsteht daraus ein formal
korrektes schwarzes PNG mit falscher Erfolgsmeldung.

Sichtbare Fenster ohne Fokus, zu 50 Prozent verdeckte Fenster und wieder
sichtbare Fenster lieferten korrekte Bilder. Kontrollierte Zustandswechsel im
selben Prozess bestätigten den Zusammenhang ohne weitere Simulationsticks.
Der eigenständige Versuch mit Bevy `v0.20.0-rc.1` reproduzierte denselben Pfad.
Versionen, Messwerte, Werkzeuge und Grenzen stehen in
[screenshot-evidence.md](screenshot-evidence.md).

## Fehlerpfad in Bevy

Capture und Fensterpräsentation sind zwei verschiedene Aufgaben. Eine Aufnahme
braucht eine gerenderte Textur, eine Kopie in einen lesbaren Buffer und dessen
Readback. Die Anzeige im Fenster benötigt zusätzlich eine vom Betriebssystem
bereitgestellte Swapchain-Textur.

In `bevy_render 0.19.1`, `src/view/window/screenshot.rs`, sind diese Aufgaben
so gekoppelt:

1. `prepare_screenshots` erzeugt eine Capture-Textur und einen
   `MAP_READ | COPY_DST`-Buffer. Die Textur ersetzt für diesen Frame den
   Fenstereintrag in `ViewTargetAttachments`.
2. Bei vollständiger Verdeckung liefert wgpu auf dem gemessenen Metal-System
   `CurrentSurfaceTexture::Occluded`. Bevy erhält keinen Swapchain-View.
3. `submit_screenshot_commands` prüft für das Fenster zuerst View und Format.
   Fehlt eines davon, überspringt es `render_screenshot` vollständig.
4. `render_screenshot` enthält sowohl `copy_texture_to_buffer` als auch den
   anschließenden Präsentations-Blit. Der fehlende Fenster-View verhindert
   deshalb auch die Kopie, obwohl diese ihn nicht benötigt.
5. `collect_screenshots` mappt trotzdem alle vorbereiteten Buffer. wgpu liefert
   für einen unbeschriebenen Mappingbereich nullinitialisierte Bytes.

Ein erfolgreicher Map-Callback belegt somit keinen ausgeführten Screenshot-Copy.
Ein 30-Sekunden-Readback-Timeout erkennt diesen Fall nicht, weil ein fertiges
`Image` zurückkommt. Auch korrekte PNG-Abmessungen belegen keinen korrekten Inhalt.

Feste Bevy-Quellreferenzen enthält der
[Upstream-Vergleich](bevy-upstream-screenshot-status.md#codevergleich).
Ergänzende Quellen der gelockten Crates sind:

| Quelle | Bedeutung |
| --- | --- |
| `wgpu-hal 29.0.4`, `src/metal/surface.rs:116-166` | Prüft das Visible-Bit von `NSWindow.occlusionState` vor `nextDrawable()`, nicht den Fensterfokus. |
| `wgpu-core 29.0.4`, `src/device/mod.rs:213-291` | Initialisiert unbeschriebene Mappingbereiche mit Nullen. |
| `wgpu-core 29.0.4`, `src/device/life.rs:149-183,283-324` | Ordnet das Mapping dem Abschluss der Buffer-GPU-Nutzung zu. |
| `bevy_render 0.19.1`, `src/renderer/mod.rs:69-120` | Führt Rendergraph, Screenshot-Submit und anschließendes Einsammeln aus. |

## Aktuelle Absicherung in woodpecker

Der [framegebundene Guard](../../../src/session/screenshot/capture/surface.rs)
prüft nach der Render-Vorbereitung View und Format für den ersten Capture-Frame
jedes Requests. Ein `OnceLock` hält diesen Befund unveränderlich fest.

Der [Capture-Adapter](../../../src/session/screenshot/capture.rs) behandelt ihn so:

- Fehlender View oder fehlendes Format ergibt `screenshot_window_unavailable`.
  Es entsteht keine Datei; eine vorhandene Datei bleibt unverändert.
- Ein fehlender Verifikationsbefund ergibt `screenshot_failed`.
- Späte Readbacks eines abgelehnten Requests werden verworfen. Eine später
  verfügbare Surface kann den alten Request nicht nachträglich freigeben.
- Ein verifiziertes, tatsächlich schwarzes Bild bleibt gültig.

Die Tests prüfen diese Fälle einschließlich unveränderter Dateien und
Request-/Frame-Zuordnung. Der Guard verwendet keine Pixelheuristik, zusätzlichen
Ticks, Fokusänderung oder Wiederholung. Er prüft öffentliche Voraussetzungen des
untersuchten Bevy-Pfads, nicht den privaten Copy-Aufruf selbst. Diese Zuordnung
muss bei einem Dependency-Update erneut geprüft werden.

## Akzeptierte Einschränkung

Der Guard verhindert falschen Erfolg. Er ermöglicht keine erfolgreiche Aufnahme
vollständig verdeckter Fenster. Diese Aufnahmefähigkeit ist im
[aktuellen Zielumfang](../target.md#screenshot) nicht zugesichert und kein
Abschlussblocker. Bei fehlender Renderoberfläche bleibt die explizite Ablehnung.

Die historischen strikten Bildtests behandeln die Ablehnung weiterhin als Fehler;
eine bestandene Guard-Diagnose ersetzt keine Bildabnahme. Diese Ergebnisse werden
durch die Umfangsentscheidung nicht nachträglich zu erfolgreichen Aufnahmen.
Der aktuelle Bedienablauf steht in [usage.md](../usage.md#screenshot).

## Grenzen und getrennte Befunde

- Die kontrollierten Versuche belegen diesen Mechanismus auf macOS mit Metal.
  Sie beweisen nicht die Ursache jedes historischen Schwarzbildes und machen
  keine Aussage über Linux oder andere GPUs und Backends.
- Ein fehlender View kann auch andere Ursachen als Verdeckung haben, etwa eine
  fehlgeschlagene Surface-Akquise. Fokusverlust allein ist kein Occlusion-Nachweis.
- Eine korrekt kopierte Textur kann legitim schwarz sein. Fehlende Kamerapässe,
  nicht bereite Pipelines oder Formatkonvertierung wären bei einer anderen
  vollständigen Copy-/Map-Kette gesondert zu untersuchen.
- Einfaches Warten half in der früheren Messreihe bei 24 von 24 Captures nicht.
  Das schließt zusätzliche Cold-Start-Probleme nicht allgemein aus und begründet
  keine Änderung von `Ready`.
- Die [Resize-/Kamera-Aktualitätslücke](window-schedules.md) ist ein eigener
  Befund. Sie erklärt den kontrollierten Occlusion-Versuch nicht.
- Der GPU-Clusterfehler vor dem ersten expliziten Tick ist ebenfalls getrennt.
  Die 3D-Fixtures aktivieren ihre Kamera deshalb beim ersten Warp.
- Frühere feste Annahmen über die Bildgröße wurden in den Tests korrigiert.
  Die Tests verwenden tatsächliche Rendergröße und Skalierung; dies war kein
  Fix für den ausgelassenen Copy.
