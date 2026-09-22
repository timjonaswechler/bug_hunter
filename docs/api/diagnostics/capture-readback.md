# Diagnose: erfolgreicher Screenshot mit vollständig nullgesetztem RGB

## Ergebnis

Bevy 0.19.1 kann für ein Fenster-Screenshot ein erfolgreiches `Image` liefern,
obwohl kein `copy_texture_to_buffer` für diesen Screenshot aufgezeichnet wurde.
`wgpu` initialisiert den anschließend zum Lesen gemappten, noch unbeschriebenen
Transferpuffer vorschriftsgemäß mit Nullen. Woodpecker kodiert dieses Bild als
gültiges RGB-PNG und antwortet mit `Completed`.

Für Metal gibt es eine konkrete, zum beobachteten Fehler passende Bedingung:

1. Das `NSWindow` gilt beim Erwerb des nächsten Drawables als nicht sichtbar.
2. `wgpu-hal` liefert `SurfaceError::Occluded`.
3. Bevy behandelt `CurrentSurfaceTexture::Occluded` ohne Warnung oder Fehler und
   lässt `swap_chain_texture_view` leer.
4. Bevys Screenshot-Code hat Capture-Textur und Transferpuffer zu diesem Zeitpunkt
   bereits vorbereitet. Beim Submit überspringt er wegen des fehlenden
   Swapchain-Views aber die gesamte Funktion `render_screenshot`, einschließlich
   der Kopie in den Transferpuffer.
5. `collect_screenshots` mappt den vorbereiteten Transferpuffer trotzdem und
   sendet daraus ein `Image`.

Dieser Pfad erklärt gleichzeitig:

- korrekte PNG-Abmessungen,
- ausschließlich nullgesetzte RGB-Kanäle,
- eine erfolgreiche Antwort statt Timeout,
- keine Renderer-Warnung und
- das sporadische Auftreten bei unverändertem Anwendungscode.

Der Quellpfad ist nachgewiesen. Nicht nachgewiesen ist, dass `Occluded` in den
historischen Fehlerläufen tatsächlich vorlag. Der geschlossene Deckel ist dafür
nur eine mögliche Ursache. Eine nicht sichtbare, minimierte, vollständig
verdeckte oder auf einem anderen Space liegende `NSWindow`-Instanz kann dieselbe
Metal-Bedingung auslösen.

## Quellenbefund

Untersucht wurden die mit `Cargo.lock` aufgelösten Quellen:

- `bevy_render 0.19.1`
- `bevy_core_pipeline 0.19.1`
- `wgpu 29.0.4`
- `wgpu-core 29.0.4`
- `wgpu-hal 29.0.4`

Die Zeilenangaben beziehen sich auf diese Crates im lokalen Cargo-Registry-Quellbaum.

### Anlage und Umleitung des Screenshot-Ziels

`bevy_render/src/view/window/screenshot.rs:266-404` führt für jeden extrahierten
Screenshot `prepare_screenshots` aus:

- Für ein Fenster nimmt Bevy Größe und Format aus der bereits konfigurierten
  `WindowSurface`.
- `prepare_screenshot_state` erzeugt eine neue Textur mit
  `RENDER_ATTACHMENT | COPY_SRC | TEXTURE_BINDING`.
- Es erzeugt außerdem einen Puffer mit `MAP_READ | COPY_DST`.
- Die neue Textur wird als `OutputColorAttachment` für das normalisierte
  Fensterziel in `ViewTargetAttachments` eingetragen.

Die Reihenfolge ist ausdrücklich
`prepare_view_attachments -> prepare_screenshots -> prepare_view_targets`
(`screenshot.rs:428-441`). Kameras dieses Renderframes erhalten damit die neue
Capture-Textur als Ausgabeziel. Das ist keine Kopie des letzten präsentierten
Fensterinhalts.

`bevy_render/src/texture/texture_attachment.rs:144-178` zeigt außerdem, dass die
erste Benutzung eines Output-Attachments den Clear-/Load-Zustand verfolgt.
Die normalen 2D-/3D-Zeitpläne schreiben über den abschließenden Upscaling-Pass
in dieses Attachment. Auf Metal öffnet
`bevy_core_pipeline/src/upscaling/node.rs:63-82` selbst bei einer noch nicht
fertigen Upscaling-Pipeline einen Renderpass, um ein uninitialisiertes Drawable
zu vermeiden. Eine bloß noch kompilierende Upscaling-Pipeline ist daher keine
gute Erklärung für ein vollständig nullgesetztes Bild.

### Der übersprungene Copy

`bevy_render/src/view/window/screenshot.rs:498-629` koppelt zwei verschiedene
Operationen in `render_screenshot`:

1. Capture-Textur nach Transferpuffer kopieren.
2. Capture-Textur zur Präsentation zurück auf den Swapchain-View zeichnen.

Für ein Fenster prüft `submit_screenshot_commands` zuerst:

```text
ExtractedWindow vorhanden?
swap_chain_texture_view_format vorhanden?
swap_chain_texture_view vorhanden?
```

Fehlt der View, führt Bevy `continue` aus (`screenshot.rs:508-520`). Dadurch
entfällt nicht nur das Zurückzeichnen auf das Fenster, sondern auch der
`copy_texture_to_buffer` in `render_screenshot` (`screenshot.rs:592-605`).
Der Transferpuffer bleibt unbeschrieben.

### Trotzdem gestarteter Readback

`bevy_render/src/view/window/screenshot.rs:631-700` iteriert nicht über
erfolgreich kopierte Screenshots, sondern über alle Einträge in
`RenderScreenshotsPrepared`. Für jeden Eintrag:

- wird `map_async(MapMode::Read)` gestartet,
- werden die gemappten Bytes in einen `Vec<u8>` kopiert,
- wird daraus ohne Inhaltsprüfung ein `Image` erzeugt und
- wird dieses `Image` an `CapturedScreenshots` gesendet.

Es gibt in `RenderScreenshotsPrepared` kein Merkmal dafür, ob
`render_screenshot` und damit die Kopie tatsächlich liefen.

### Warum der Puffer genau Nullen enthält

`wgpu-core/src/device/mod.rs:213-291` implementiert das Mapping. Die Kommentare
in Zeile 237-249 nennen die WebGPU-Regel ausdrücklich: Ressourcen müssen sich
beim ersten Lesen wie mit Null initialisiert verhalten. Für alle noch
uninitialisierten Bereiche führt der Code `mapped[fill_range].fill(0)` aus
(`device/mod.rs:258-279`).

Das ist hier der relevante Mechanismus. Nicht die Capture-Textur wird
notwendigerweise null gelesen. Der Copy-Befehl fehlt ganz, und das Mapping
initialisiert den unbeschriebenen Transferpuffer mit Nullen.

### Konkreter Metal-Auslöser ohne Logmeldung

`wgpu-hal/src/metal/surface.rs:116-166` prüft vor `nextDrawable()` den
`occlusionState` des zugehörigen `NSWindow`. Fehlt das Bit
`NS_WINDOW_OCCLUSION_STATE_VISIBLE`, liefert die Funktion
`SurfaceError::Occluded`.

`wgpu-core/src/present.rs:255-273` übersetzt dies in den Surface-Status
`Occluded`; `wgpu/src/api/surface.rs:120-130` macht daraus
`CurrentSurfaceTexture::Occluded`.

`bevy_render/src/view/window/mod.rs:244-337` behandelt beim Acquire:

```rust
wgpu::CurrentSurfaceTexture::Occluded => {}
```

Der Arm ist absichtlich still. Er setzt keinen Swapchain-View. Die
Fensterkonfiguration mit Breite, Höhe und Format bleibt dagegen vorhanden.
Deshalb kann `prepare_screenshots` einen Puffer mit zum Beispiel 1280 x 720
anlegen, obwohl `submit_screenshot_commands` wenige Systeme später die Kopie
wegen des fehlenden Views überspringt.

## Verhalten des Woodpecker-Adapters

Der Adapter prüft vor Annahme eines Requests in
[`window`](../../../src/session/screenshot/capture.rs#L90-L104) nur:

- genau ein primäres Fenster,
- vorhandenen `RawHandleWrapper`,
- physische Breite und Höhe größer null.

Diese Prüfung läuft in der Main World. Sie sagt nichts darüber aus, ob Metal im
späteren Renderframe einen Swapchain-View erwerben konnte.

[`poll`](../../../src/session/screenshot/capture.rs#L155-L230) übernimmt jedes
von Bevy gesendete `(Entity, Image)` für den aktiven Request. Der Adapter kann
nicht erkennen, ob Bevy den GPU-Copy übersprungen hat. Nach erfolgreichem
Encoding und Schreiben erzeugt
[`response`](../../../src/session/screenshot/capture.rs#L122-L130) deshalb
`Completed`.

[`encode`](../../../src/session/screenshot/capture.rs#L133-L153) weist nur
Breite oder Höhe null und nicht unterstützte Formate zurück. `to_rgb8()` entfernt
den Alpha-Kanal. Ein nullgesetzter RGBA-Puffer wird somit zu einem gültigen PNG,
dessen RGB-Kanäle vollständig null sind.

Das 30-Sekunden-Timeout greift nicht: Bevy sendet ein fertiges `Image`. Inhaltlich
ist es falsch, technisch ist der Readback abgeschlossen.

## Reproduzierter Testbefund

Für die Diagnose wurde vorübergehend ein Unit-Test direkt an der vorhandenen
`CapturedScreenshots`-Testnaht in `capture.rs` ergänzt. Er injizierte ein
erfolgreiches Bevy-`Image` mit ausschließlich nullgesetzten Bytes, ließ den
echten Adapter-Worker kodieren und schreiben und prüfte anschließend:

- Antwort ist `Message::Completed`.
- Das geschriebene PNG besitzt ausschließlich nullgesetzte RGB-Kanäle.

Der Test bestand. Er wurde danach vollständig entfernt; `capture.rs` hat keinen
verbleibenden Diff. Dieser Befund beweist die Adaptersemantik ab Bevys
erfolgreichem Kanal. Er reproduziert weder Metal-Occlusion noch den
übersprungenen GPU-Copy.

Ausgeführter Befehl:

```sh
cargo test --all-features \
  session::screenshot::capture::tests::diagnostic_all_zero_readback_is_reported_as_completed \
  -- --exact --nocapture
```

Ergebnis: 1 bestanden, 0 fehlgeschlagen. Ein erster Entwurf des temporären Tests
prüfte fälschlich auch den beim erneuten Dekodieren ergänzten Alpha-Wert. Das
PNG ist absichtlich RGB; nach Beschränkung der Aussage auf die gemeldeten
RGB-Kanäle bestand derselbe Test.

Nach Entfernung der temporären Diagnose wurde der vorhandene Adaptertest erneut
ausgeführt:

```sh
cargo test --all-features \
  session::screenshot::capture::tests::response_waits_for_readback_and_decodable_png_without_advancing_time \
  -- --exact
```

Ergebnis: 1 bestanden, 0 fehlgeschlagen.

Es wurden keine Fenster, gerenderten Anwendungen oder Szenentests gestartet.

## Einordnung der Hypothesen

### Durch Quellen belegter, aber im historischen Lauf noch nicht gemessener Pfad

**Fehlender Swapchain-View beim Screenshot-Submit.** Diese Bedingung reicht aus,
um einen erfolgreichen, vollständig nullgesetzten Screenshot zu erzeugen.
`CurrentSurfaceTexture::Occluded` auf Metal ist eine konkrete stille Ursache
für den fehlenden View. Sie passt besser als ein allgemeiner "Closed-lid"-Verdacht,
weil sie die tatsächliche Bedingung im Code benennt. Ob der Deckel, eine
Fensterverdeckung oder ein anderer Sichtbarkeitszustand sie ausgelöst hat, bleibt
offen.

### Weiter offen

**Capture-Textur wurde trotz vorhandenem Swapchain-View nicht von einem
Kamerapass beschrieben.** Auch das könnte wegen der WebGPU-Nullinitialisierung
ein Nullbild ergeben. Im Blend-Mode-Fall sprechen die aktive UI-Kamera, die nach
dem ersten Tick aktive 3D-Kamera und die nicht schwarze Clear-Farbe dagegen.
Der Quellbefund zum fehlenden Swapchain-View ist spezifischer und erklärt die
fehlende Logmeldung.

**Anderer Grund für einen fehlenden Swapchain-View.** Ein erfolgloser
`Outdated`-Retry oder andere Acquire-Fehler können ebenfalls ohne View enden.
Bevy loggt diese Pfade jedoch als Warnung oder Fehler. Die historischen Logs
ohne Renderer-Warnung sprechen für den stillen `Occluded`-Arm.

**Metal-/Treiberfehler trotz aufgezeichnetem Copy.** Dafür gibt es keinen
positiven Befund. Er bleibt erst dann relevant, wenn eine Probe
`copy_texture_to_buffer` als tatsächlich aufgezeichnet nachweist und das Bild
trotzdem null bleibt.

### Durch Quelle oder Test stark entkräftet

**PNG-Encoding erzeugt die Nullen.** Der vorhandene Adaptertest erhält
`[12, 34, 56]` durch Encoding und Dekodierung. Der temporäre Diagnosetest zeigt
umgekehrt, dass ein bereits nullgesetztes Bevy-Bild unverändert als RGB0
geschrieben und erfolgreich gemeldet wird.

**Readback antwortet vor GPU-Abschluss.** Bevy startet `map_async` auf dem
Transferpuffer; der Callback läuft erst, wenn Polling beziehungsweise weitere
Queue-Aktivität das Mapping abschließt. Erst danach sendet Bevy das `Image`.
Der problematische Pfad ist ein erfolgreiches Mapping ohne vorherigen Copy,
nicht ein verfrühter Callback.

**Queue- oder Ressourcenlebensdauer verliert einen korrekt aufgezeichneten
Copy.** Der Renderablauf reicht zuerst die Kamera-Command-Buffer ein, dann den
Screenshot-Encoder. Die Queue bewahrt Einreichungsreihenfolge. Der asynchrone
Task hält einen Clone des Transferpuffers bis nach dem Mapping. Im untersuchten
Code gibt es keinen Hinweis auf einen vorzeitig freigegebenen Puffer.

**Abmessung null.** Woodpecker lehnt ein Main-World-Fenster mit Breite oder Höhe
null ab. Der bekannte Dimension-0-Clusterfehler vor der Aktivierung der Kamera
ist ein anderer Fehler. Metal-Occlusion lässt die konfigurierte Größe
unverändert und erklärt daher auch 1280 x 720 große RGB0-PNGs.

**Ein Größenwechsel allein.** Erfolgreiche und fehlgeschlagene Blend-Bilder
hatten jeweils 1280 x 720. Der erfolgreiche Mesh-Lauf mit 320 x 180 widerlegt
außerdem die Annahme, eine kleinere Rendergröße führe notwendig zu RGB0.

## Nächste unterscheidende Probe

Diese Probe benötigt mit dem Nutzer abgestimmtes Live-Rendering. Sie soll nicht
automatisch bis zu einem grünen Ergebnis wiederholen.

1. Bevy 0.19.1 als worktree-lokale temporäre Cargo-Patch-Kopie verwenden. Keine
   Datei im globalen Cargo-Registry-Verzeichnis ändern.
2. Ausschließlich vier Ereignisse mit dem Präfix `[CAPTURE-PROBE]` ausgeben:
   - Ergebnis des Surface-Acquire, insbesondere `Occluded`,
   - Screenshot-Entity und Vorhandensein von
     `swap_chain_texture_view_format`/`swap_chain_texture_view`,
   - ob `copy_texture_to_buffer` für diese Entity aufgezeichnet wurde,
   - nach Empfang im Main-World-Adapter Anzahl der von null verschiedenen
     RGB-Bytes und Bildgröße.
3. Einen sichtbaren Baseline-Screenshot aufnehmen.
4. Dasselbe Fenster kontrolliert minimieren oder auf einen nicht sichtbaren
   macOS-Space verschieben. Einen einzelnen Screenshot ohne Retry aufnehmen.
5. Fenster wieder sichtbar machen und einen weiteren Screenshot aufnehmen.

Die Ergebnisse unterscheiden die Ursachen eindeutig:

| Beobachtung | Schluss |
| --- | --- |
| `acquire=Occluded`, View fehlt, `copy=false`, Mapping erfolgreich, RGB0 | Der hier belegte Bevy/wgpu-Pfad ist live reproduziert. |
| View fehlt und `copy=false`, aber kein `Occluded` | Anderer Surface-Acquire-Pfad; dessen Status ist direkt sichtbar. |
| View vorhanden und `copy=true`, RGB0 | Occlusion/Skip-Copy widerlegt; als Nächstes Kamerapass und Capture-Textur mit einem Sentinel-Clear unterscheiden. |
| View vorhanden, `copy=true`, RGB ungleich null | Capture-Pfad funktioniert in dieser Probe; keine automatische Wiederholung als Behebungsnachweis. |

Falls `copy=true` und RGB0 auftritt, ist die kleinste zweite Probe ein
worktree-lokaler Bevy-Patch, der die neue Capture-Textur vor den Kamerapässen mit
einer auffälligen Sentinel-Farbe füllt. Sentinel im PNG bedeutet, dass kein
Kamerapass das umgeleitete Ziel beschrieb. RGB0 trotz überschriebenem Sentinel
würde den Verdacht auf Copy/Readback oder Treiber verschieben.

Die Probe soll keine Fenstertitel, Pfade, Umgebungsvariablen oder sonstigen
privaten Umgebungsdaten loggen. Entity-ID, Größe, boolesche Zustände und der
Surface-Status genügen.
