# Research: Bevy-Screenshot ohne Simulations-Tick

## Kurzfazit

`crates/bug_hunter/Cargo.toml` fordert Bevy `0.19.0`; der Workspace-Lock löst auf Bevy **0.19.1** auf (einschließlich `bevy_render` und `bevy_app`). Die Antwort lautet für die kontrollierte Komposition **ja**: Ein Screenshot kann angefordert werden, ohne `Update`, `FixedUpdate` oder einen kontrollierten `Clock`-Frame auszuführen. Das ist aber keine Eigenschaft, die Bevy `Screenshot` selbst erzwingt; sie entsteht hier durch `AutomationControlPlugin` und dessen Schedule-Ordnung. Eine App-/Render-Iteration und danach asynchroner GPU-Readback sind weiterhin nötig. Das bestehende PNG wird von diesem Crate absichtlich **nicht** überschrieben.

## Befunde

1. **[Info] Versionsstand und lokale Quelle** — `crates/bug_hunter/Cargo.toml` aktiviert `bevy_render` und `bevy_window`. `Cargo.lock` enthält `[[package]] name = "bevy"; version = "0.19.1"` sowie `bevy_render`/`bevy_app` in 0.19.1. Die installierte Quelle wurde unter `/Users/tim-jonaswechler/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_render-0.19.1/` gelesen.

2. **[Info] Kein kontrollierter Tick für den Screenshot** — `crates/bug_hunter/src/client/plugin.rs` ersetzt `MainScheduleOrder.labels` durch `Input`, `Frames`, `Output`. `run_controlled_frames` führt die ursprünglichen Simulations-Schedules nur bei einem anhängigen `time`-Befehl aus und erhöht erst dann `Clock`. Der Screenshot-Pfad (`receive_request` → `dispatch_request` → `screenshot::start`) läuft in `Input`; ein Screenshot-Befehl lässt `Frames` leer. Die Tests `simulation_and_virtual_time_advance_only_for_time_commands` und `rendered_redraw_updates_do_not_run_simulation_schedules` in derselben Datei belegen das. `TimeReceiver` wird entfernt, die kontrollierte Dauer steht außerhalb eines `time`-Befehls auf null.

   Bevy selbst garantiert diesen No-Tick-Effekt nicht: In einer normalen Bevy-App läuft `Screenshot` einfach neben der vom Host gewählten Main-Schedule-Ordnung. Die Zusicherung gilt hier nach der einmaligen Startup-Initialisierung für die `bug_hunter`-Controlled-Session.

3. **[Info] Ein Render-Frame bleibt zwingend** — Bevy führt laut `bevy_app/src/main_schedule.rs` die Render-SubApp zwischen Main-Schedule-Iterationen aus. `bevy_app/src/sub_app.rs` (`SubApps::update`) läuft zuerst den Main-Schedule und danach `RenderApp.extract` sowie `RenderApp.update`. Ohne diesen Renderlauf wird die `Screenshot`-Komponente nicht extrahiert und keine GPU-Auslese gestartet. `crates/bug_hunter/src/screenshot/plugin.rs` stellt den Dienst außerdem nur bereit, wenn `RenderApp` und `CapturedScreenshots` existieren; ohne Renderer ist das Ergebnis `screenshot_capability_unavailable`.

4. **[Info] Zeitpunkt und Inhalt von `Screenshot::window`** — In Bevy 0.19.1 ist `Screenshot::window(window)` ein `RenderTarget::Window`, kein sofortiger CPU-Snapshot. `extract_screenshots` in `bevy_render/src/view/window/screenshot.rs` sieht die Komponente im `ExtractSchedule`, normalisiert das Fensterziel und markiert die Entität als `Capturing`. `prepare_screenshots` setzt für diesen Render-Frame ein internes Screenshot-Texture-Target als Output-Attachment. Danach rendert `render_system` den Rendergraphen; `submit_screenshot_commands` kopiert den fertigen Target-Texture-Inhalt in einen Readback-Puffer und zeichnet ihn in einem Fullscreen-Pass wieder auf die Swapchain. Das Bild ist somit der Inhalt des betreffenden **gerenderten** Frames, nicht ein OS-/Fensterdekorations-Screenshot.

5. **[Info] Asynchroner Ablauf und aktueller Observer-Vertrag** — Die offizielle API definiert `ScreenshotCaptured` als `EntityEvent` mit `entity` und `image`; der aktuelle Observer ist `On<ScreenshotCaptured>`. Er wird erst ausgelöst, wenn der GPU-Readback fertig und das `Image` in der Main World angekommen ist. Die Doku sagt ausdrücklich, dass dies nicht sofort nach dem Spawn-Frame verfügbar sein muss und dass die Screenshot-Entität nach Capture und Observer-Trigger despawned wird. `collect_screenshots` nutzt `map_async` und einen Kanal, `trigger_screenshots` leert ihn in der Main World.

   Garantiert sind ein fertiges `Image` beim Observer und die Zuordnung zur Screenshot-Entität; **nicht** garantiert sind sofortige Verfügbarkeit, eine feste Anzahl/Frist an `app.update()`-Aufrufen, ein Simulations-Frame-Index im Event oder eine fertige PNG-Datei. Das Crate verarbeitet den Observer in `complete_capture` und räumt die Entität in `collect_screenshot` selbst auf. Wegen GPU-Polling und Deferred Commands kann eine weitere App-/Render-Iteration nötig sein.

   Praktischer Ablauf nach Startup in der kontrollierten Komposition (`crates/bug_hunter/src/client/plugin.rs`):

   ```text
   app.update(): Input spawn(Screenshot::window) -> Frames no-op -> Output pending
                -> RenderApp: ExtractSchedule -> RenderGraph -> GPU copy/readback
   spätere app.update()-Iteration(en): trigger_screenshots -> Observer -> PNG-I/O -> Output
   ```

6. **[Medium – Verhaltensbefund] Überschreiben ist kein Bevy-Capture-Feature** — Bevy selbst kennt beim `Screenshot`-Component keinen Pfad. Der optionale Helper `save_to_disk` ruft in `bevy_render/src/view/window/screenshot.rs` lediglich `image::save_with_format` auf und enthält keine `create_new`- oder Existenzprüfung; ein Overwrite-Vertrag gehört damit nicht zur `Screenshot`-API, sondern zur delegierten Datei-I/O-Bibliothek. `bug_hunter` verwendet diesen Helper nicht: `crates/bug_hunter/src/screenshot/capture.rs::prepare_target` gibt bei einem vorhandenen Ziel explizit `artifact target ... already exists` zurück, und `write_png` öffnet mit `OpenOptions::create_new(true)`. Auch eine Race-Condition beim Schreiben führt zu einem Fehler statt zum Überschreiben. Für dieses Repository gilt daher: **vorhandene PNGs werden nicht überschrieben; das steuert der Crate-Dateischreiber, nicht Bevy.**

## Quellen (nur primär/offiziell)

### Bevy 0.19.1

- [Screenshot-Implementierung](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_render/src/view/window/screenshot.rs) — `Screenshot::window`, Extract/Prepare/Readback, `ScreenshotCaptured`, Observer-Trigger und `save_to_disk`.
- [Render-System](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_render/src/renderer/mod.rs) — Rendergraph, Screenshot-Commands, Submit/Present und asynchrones Collect.
- [ExtractPlugin](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_render/src/extract_plugin.rs) — Trennung von Main World und `ExtractSchedule`/Render World.
- [SubApps](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_app/src/sub_app.rs) und [MainSchedule](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_app/src/main_schedule.rs) — Reihenfolge Main-Iteration versus Render-SubApp.
- [Generierte API-Doku: `Screenshot`](https://docs.rs/bevy_render/0.19.1/bevy_render/view/window/screenshot/struct.Screenshot.html) und [`ScreenshotCaptured`](https://docs.rs/bevy_render/0.19.1/bevy_render/view/window/screenshot/struct.ScreenshotCaptured.html) — aktueller dokumentierter Observer-Vertrag.

### Lokale Repository-Quellen

- `crates/bug_hunter/Cargo.toml`, Root-`Cargo.lock` — Anforderung 0.19.0, Auflösung 0.19.1.
- `crates/bug_hunter/src/client/plugin.rs` — kontrollierte Schedule-Reihenfolge, Clock-Handling und Screenshot-Protokoll.
- `crates/bug_hunter/src/screenshot/plugin.rs` — Ziel-Entität, Observer und Ergebnisübergabe.
- `crates/bug_hunter/src/screenshot/capture.rs` — Sandbox, `prepare_target`, `create_new(true)`, PNG-Validierung.
- `apps/app/src/composition.rs` — reale gerenderte Komposition mit `DefaultPlugins`, `AutomationControlPlugin` und `bug_hunter::screenshot::Plugin`; kein `PipelinedRenderingPlugin` explizit aktiviert.
- Installierte Bevy-Dateien: `/Users/tim-jonaswechler/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_render-0.19.1/src/view/window/screenshot.rs`, `.../bevy_render-0.19.1/src/extract_plugin.rs`, `.../bevy_render-0.19.1/src/renderer/mod.rs`, `.../bevy_app-0.19.1/src/sub_app.rs`.

## Lücken und Rest-Risiken

- **[Medium]** Kein GPU-/Fenster-Integrationstest wurde ausgeführt. Bei minimiertem/occluded Fenster, fehlender Kamera oder fehlender Swapchain kann Bevy Renderziele überspringen; eine Erfolgsantwort ist dafür nicht API-garantiert.
- **[Medium]** Die Repository-Komposition aktiviert kein `PipelinedRenderingPlugin`. Bei anderer Einbettung kann Rendering parallel bzw. frame-versetzt laufen; der No-Tick-Befund bezieht sich auf die geprüfte Controlled-Session-Schedule-Ordnung, nicht auf beliebige eigene Render-Schedules.
- **[Low]** Das Weglassen von `PostUpdate`/Transform-Synchronisierung ist kein Tick, kann aber dazu führen, dass der Screenshot den zuletzt renderbereiten statt einen neu berechneten Transform-/UI-Zustand zeigt.
- **[Low]** Die genaue Overwrite-Semantik des von Bevy delegierten `image`-Helpers wurde nicht als eigene Primärquelle untersucht; für den Repository-Code ist die explizite No-Overwrite-Regel durch `prepare_target` und `create_new` eindeutig.

**Vertrauen:** hoch (ca. 0,95) für Bevy-0.19.1-Ablauf, Render-Frame-Erfordernis und den No-Tick-Pfad; mittel-hoch für Backend-/Pipelining-Randfälle.

## Akzeptanzbericht

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Konkrete Befunde mit Severity und Pfaden: client/plugin.rs (kein Frames-/Clock-Lauf), Bevy screenshot.rs/renderer/mod.rs (Render- und Observer-Ablauf), screenshot/capture.rs (create_new und vorhandenes Ziel wird abgewiesen)."
    }
  ],
  "changedFiles": [
    "/Users/tim-jonaswechler/.pi/agent/sessions/--Users-tim-jonaswechler-GitHub-Projekte-star_sim-crates-bug_hunter--/subagent-artifacts/outputs/27e65ef8-f4fa-4063-8e0f-4e62d96bd94b/bevy-screenshot-semantics.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [],
  "validationOutput": [
    "Cargo.toml/Cargo.lock und installierte Bevy-0.19.1-Quellen gelesen; keine Projektdatei außer dem vorgeschriebenen Brief geändert."
  ],
  "residualRisks": [
    "medium: GPU-/Fenster-Randfälle und Pipelined-Rendering nicht runtime-validiert",
    "low: omitted PostUpdate may leave render-ready state stale"
  ],
  "noStagedFiles": true,
  "diffSummary": "Nur der vorgeschriebene deutsche Forschungsbrief als Subagent-Artefakt; keine Projektdatei geändert.",
  "reviewFindings": [
    "blocker: none",
    "medium: existing PNG overwrite is intentionally rejected by crates/bug_hunter/src/screenshot/capture.rs"
  ],
  "manualNotes": "Confidence hoch für den geprüften Bevy-0.19.1-/Controlled-Session-Pfad; bare Bevy save_to_disk delegates overwrite behavior to image crate."
}
```