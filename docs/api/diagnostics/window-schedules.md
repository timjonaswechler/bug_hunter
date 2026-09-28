# Fenstergröße und Kameraaktualisierung bei angehaltener Simulation

## Befund und Status

Winit aktualisiert `WindowResolution` bei Resize und Skalierungswechsel auch
zwischen expliziten Warps. Die Render-App übernimmt die neue Größe. Bevys
`camera_system` läuft im kontrollierten Leerlauf dagegen nicht; berechnete
Kameradaten, Projektion und expliziter Viewport können bis zum nächsten Warp
auf dem alten Stand bleiben.

Diese Aktualitätslücke ist aus den gelockten Quellen von Bevy 0.19.1 belegt.
Eine gezielte Bildregression für Resize und Skalierungswechsel steht aus.
Die Lücke ist kein Beweis für die Ursache historischer schwarzer PNGs.
Im [kontrollierten Occlusion-Versuch](screenshot-evidence.md#framegenaue-befunde)
blieben alle Größen und Kameraangaben gleich; dort entfiel bereits die Bildkopie.

## Beteiligte Schedules

Das [Session-Plugin](../../../src/session/plugin.rs) ersetzt
`MainScheduleOrder.labels` durch `Control` und bewahrt die ursprüngliche
Reihenfolge auf. Nur ein aktiver Warp führt diese Simulations-Schedules aus.
Zwischen Warps laufen daher unter anderem nicht:

- `First` mit normaler Nachrichtenalterung,
- `PostUpdate::camera_system`,
- Transform- und Sichtbarkeitsaktualisierungen in `PostUpdate`,
- main-world Asset-Vorbereitung, soweit sie diesen Schedules zugeordnet ist.

Die Render-App führt weiterhin Extraction und ihre eigenen Schedules aus.
Das Leeren von `TimeReceiver` im Kontrolllauf verhindert einen vollen
Render-Zeitkanal, ohne die Simulationszeit fortzuschreiben. Es ersetzt keine
Kameraaktualisierung.

## Fenster- und Kameradaten

| Quelle in Bevy 0.19.1 | Verhalten |
| --- | --- |
| `bevy_app/src/sub_app.rs:571-590` | Main-World-Update läuft vor Extraction und Update der Sub-Apps. |
| `bevy_winit/src/state.rs:247-259,915-928` | Resize schreibt physische Abmessungen direkt in `Window.resolution`. |
| `bevy_winit/src/state.rs:931-963` | Skalierungswechsel aktualisiert den Faktor und erzeugt die zugehörigen Nachrichten. |
| `bevy_render/src/view/window/mod.rs:125-180,340-464` | Extraction liest die Fenstergröße; die Surface wird bei Änderungen neu konfiguriert. |
| `bevy_render/src/camera.rs:63-80,351-445` | `camera_system` läuft in `PostStartup` und `PostUpdate`, verarbeitet Fensterereignisse und aktualisiert Zielinfo, Viewport und Projektion. |

Das [Input-Gate](../../../src/session/input.rs) verwirft native Eingaben,
erhält aber Resize-, Scale-, Close- und andere Fensterereignisse im
`WindowEvent`-Puffer bis zum nächsten Tick. Es verhindert weder direkte
`Window`-Änderungen durch Winit noch die separaten typisierten Resize-/Scale-
Nachrichten.

Damit kann nach einem nativen Resize folgende Situation entstehen:

| Daten | Zwischen Resize und nächstem Warp |
| --- | --- |
| `WindowResolution`, extrahiertes Fenster, Surface | neue Größe |
| `Camera::computed`, Projektion, expliziter Viewport | möglicherweise alte Größe |

Für einen unveränderten Szenenzustand ist angehaltene Main-World-Verarbeitung
beabsichtigt. Externe Fensteränderungen müssen jedoch gesondert geprüft werden.

## Vorhandene Nachweise

Diese headless Prüfungen bestanden im ursprünglichen Diagnosebericht:

```sh
cargo test --lib idle_control_drains_render_clock_without_advancing_time
cargo test --lib independent_virtual_devices_ignore_native_input_without_window_focus
python3 tests/diagnostics/window_schedules.py
```

Die Rust-Tests belegen unveränderte Simulationszeit und die Aufbewahrung nativer
Fensterereignisse bei verworfener nativer Eingabe. Die Python-Probe prüft statisch
Schedule- und Dependency-Quellen. Keiner dieser Tests beweist korrektes Rendering
nach einem realen Resize.

Historische Bilder zeigen lediglich, dass eine andere PNG-Größe allein RGB0
nicht erklärt: Mesh lieferte korrekte 320×180-Bilder; Blend hatte sowohl korrekte
als auch schwarze Bilder mit 1280×720. Gleiche PNG-Abmessungen schließen einen
Kameraversatz nicht aus.

## Noch erforderliche Regression

Mit einer bekannten Szene und sichtbarem Fenster gezielt Resize und
Skalierungswechsel zwischen zwei Captures auslösen. Dabei erfassen:

1. Native Fenstergröße und Scale-Faktor sowie Resize-/Scale-Nachrichten.
2. `WindowResolution`, extrahierte Fenstergröße und Surface-Konfiguration.
3. Main-World- und extrahierte Kamera-Zielgröße, Viewport und Projektion.
4. Capture-Texturgröße, Copy-Extent und vollständige Acquire-/Copy-/Map-Kette.
5. Bildinhalt sowie unveränderte Simulationszeit und unveränderten Spielzustand.

Zuerst ohne weiteren Warp aufnehmen. Ein anschließend ausdrücklich angeforderter
Diagnose-Warp darf zeigen, ob die Kameradaten aufholen. Er ist kein zulässiger
versteckter Produktfix. Ein Größenversatz allein belegt noch keinen schwarzen
Screenshot; dafür wären erfolgreiche Copy-/Map-Ketten und ein kausal zugeordneter
Bildwechsel nötig.

Die Produktabnahme muss korrekte Dimensionen und Szenenpixel ohne zusätzliche
Simulationsticks nachweisen. Falls dafür eine andere Zuordnung von Render-
Vorbereitung und Simulation nötig wird, den Vertragskonflikt vor der Umsetzung
vorlegen. Arbeitsreihenfolge und Abschlusskriterien stehen im
[Abschlussplan](../implementation-plan.md).
