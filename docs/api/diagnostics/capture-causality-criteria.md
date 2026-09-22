# Prüfkriterien für den framegenauen Kausalnachweis

## Fragestellung und Grenze

Der Live-Versuch soll drei Erklärungen für ein vollständig RGB-nullwertiges
Fensterbild unterscheiden:

1. Bevy überspringt die Kopie in den Readback-Buffer, weil im Capture-Frame
   kein verwendbarer Window-View vorhanden ist.
2. Die Kopie läuft, aber Kamera, Viewport oder Renderdaten passen nach einem
   Resize noch nicht zum aktuellen Fenster.
3. Die Kopie läuft, aber Renderpipelines oder andere Renderdaten sind beim
   Cold Start noch nicht bereit.

Ein schwarzes PNG, ein erfolgreicher Map-Callback oder ein Log unmittelbar vor
`copy_texture_to_buffer` entscheidet diese Frage jeweils nicht allein. Der
Nachweis muss dieselbe Capture-Textur und denselben Buffer vom Renderframe bis
zu den dekodierten PNG-Daten verfolgen.

Dieser Bericht beschreibt Mess- und Entscheidungskriterien. Er enthält keinen
Fixvorschlag. Für die Quellenprüfung wurden die in
[`Cargo.lock`](../../../Cargo.lock#L992-L995) und
[`Cargo.lock`](../../../Cargo.lock#L4510-L4588) gelockten Versionen verwendet:
`bevy_render 0.19.1` sowie `wgpu`, `wgpu-core` und `wgpu-hal 29.0.4`.
Es wurde kein Fenster- oder GPU-Test gestartet.

## Was die Quellen tatsächlich sichern

Die folgenden Zeilen beziehen sich auf die lokal installierten Registry-Quellen
der gelockten Crates.

| Aussage | Quellenstelle |
| --- | --- |
| `prepare_screenshots` erzeugt pro vorbereiteter Entity eine Capture-Textur und einen neuen `MAP_READ | COPY_DST`-Buffer. Die Textur ersetzt das Window-Output-Attachment. | `bevy_render-0.19.1/src/view/window/screenshot.rs:266-404` |
| Die Vorbereitung läuft nach `prepare_view_attachments` und vor `prepare_view_targets`. | `screenshot.rs:428-441` |
| Beim Window-Ziel prüft `submit_screenshot_commands` zuerst `ExtractedWindow`, View-Format und View. Fehlt eine dieser Angaben, wird `render_screenshot` nicht aufgerufen. | `screenshot.rs:498-530` |
| `render_screenshot` zeichnet zuerst `copy_texture_to_buffer` auf. Eine noch nicht fertige Screenshot-to-Screen-Pipeline verhindert nur den anschließenden Präsentations-Blit, nicht die Kopie. | `screenshot.rs:582-628` |
| `render_system` führt den Rendergraph aus, zeichnet den Screenshot-Copy auf, reicht den Encoder ein und ruft danach `collect_screenshots` auf. | `bevy_render-0.19.1/src/renderer/mod.rs:69-120` |
| `collect_screenshots` mappt jeden vorbereiteten Buffer, nicht nur nachweislich kopierte Buffer, und sendet nach erfolgreichem Map ein `Image` mit derselben Screenshot-Entity. | `screenshot.rs:631-700` |
| `map_async` macht den Buffer erst nach Ende seiner GPU-Nutzung für die CPU zugänglich. `wgpu-core` ordnet ein Mapping dem jüngsten Submit zu, der diesen Buffer verwendet. | `wgpu-29.0.4/src/api/buffer.rs:69-97,151-171`; `wgpu-core-29.0.4/src/device/life.rs:149-183,283-324` |
| Ein noch unbeschriebener Mappingbereich liest sich als nullinitialisiert. | `wgpu-core-29.0.4/src/device/mod.rs:213-291` |
| Auf macOS prüft `wgpu-hal` direkt vor `nextDrawable()` das Visible-Bit von `NSWindow.occlusionState` und gibt andernfalls `SurfaceError::Occluded` zurück. Fokus oder Bevys `WindowOccluded`-Nachricht werden dabei nicht abgefragt. | `wgpu-hal-29.0.4/src/metal/surface.rs:116-166` |
| Bevy setzt einen neuen View und dessen Format nur bei `Success` oder `Suboptimal`. `Occluded` bleibt still; ein fehlgeschlagener `Outdated`-Retry und andere Fehler lassen ebenfalls keinen neuen View entstehen. | `bevy_render-0.19.1/src/view/window/mod.rs:244-337` |
| Bevy versucht nach dem initialen Present keine Akquise, wenn keine extrahierte Kamera mit `CameraOutputMode::Write` das Fenster als Ziel hat. | `window/mod.rs:251-265` |
| Eine inaktive Kamera wird nicht extrahiert. `CameraOutputMode::Skip` kann die Ausgabe in das endgültige Renderziel absichtlich auslassen. | `bevy_render-0.19.1/src/camera.rs:524-572`; `bevy_camera-0.19.1/src/camera.rs:384-401,859-883` |
| Fehlt auf Metal die Upscaling-Pipeline, öffnet Bevy nur einen Renderpass auf dem Output-Attachment und kehrt zurück. Der konfigurierte Clear bleibt daher als Bild möglich. | `bevy_core_pipeline-0.19.1/src/upscaling/node.rs:31-81` |

Der aktuelle Adapter prüft View und View-Format im ersten Renderframe des
Requests und speichert das Ergebnis unveränderlich in einem `OnceLock`
([`capture/surface.rs`](../../../src/session/screenshot/capture/surface.rs#L24-L57)).
Bei `false` lehnt [`poll`](../../../src/session/screenshot/capture.rs#L172-L203)
den Request vor dem Schreiben ab. Ein später eintreffendes Bevy-Image kann diese
Entscheidung nicht ändern.

## Minimale beweiskräftige Messdaten

### Gemeinsame Schlüssel

Jedes Ereignis braucht diese Schlüssel. Zeitstempel sind nur Zusatzdaten.

- Prozess- und Session-ID
- monotoner `render_frame_id`
- Command-Request-ID und Screenshot-Entity
- Window-Entity
- monotoner `prepare_id`
- eindeutige IDs für Capture-Textur und Readback-Buffer

Die IDs für Textur und Buffer müssen beim Erzeugen vergeben und an Copy,
Map-Callback und CPU-Auswertung weitergegeben werden. Debug-Ausgaben von
Adressen oder nur eine Entity-ID genügen nicht. `RenderScreenshotsPrepared`
wird in jedem Frame geleert, und `map_async` endet später auf einem anderen
Thread.

### Ereignisse in erforderlicher Reihenfolge

Für jeden einzelnen Request muss das Protokoll folgende Kette bilden:

1. **Fensterakquise.** Frame, Window, tatsächlicher
   `CurrentSurfaceTexture`-Status oder der Grund, weshalb Bevy keine Akquise
   versucht hat. Zu den Gründen gehören `reuse_existing`, `no_write_camera`,
   `missing_surface` und der initiale Present-Zustand.
2. **Screenshot-Vorbereitung.** Screenshot-Entity, `prepare_id`,
   Capture-Textur-ID, Buffer-ID, Format, konfigurierte Größe und
   `bytes_per_row`.
3. **Submit-Entscheidung.** Existenz von `ExtractedWindow`, View-Format und
   View. Bei einer Kopie zusätzlich Quelltextur-ID, Ziel-Buffer-ID, Extent und
   der Encoder beziehungsweise die nachfolgende Queue-Submission.
4. **Map-Abschluss.** Screenshot-Entity, `prepare_id`, Buffer-ID,
   `map_async`-Ergebnis, Byteanzahl und Hash der Bytes ohne Zeilenpadding.
   Für die Diagnose werden außerdem mindestens `nonzero_all_bytes`,
   `nonzero_rgb_bytes` und Alpha-Werte gezählt.
5. **Adapterentscheidung.** Ergebnis des framegebundenen Guards mit
   Screenshot-Entity und Capture-Frame sowie Annahme oder Verwerfen des
   Bevy-Images.
6. **Dateiergebnis.** Pfadbezeichner, PNG-Größe, SHA-256 und Zahl der
   RGB-nichtnullwertigen Pixel nach erneutem Dekodieren. Bei Ablehnung wird
   ausdrücklich `no_file` protokolliert.

Ein Log direkt vor `copy_texture_to_buffer` belegt nur, dass der CPU-Pfad die
Aufzeichnung erreicht hat. Es belegt noch keine Einreichung oder GPU-Ausführung.
Für den hier benötigten Nachweis muss das Log denselben Buffer nennen wie der
erfolgreiche Map-Callback. Außerdem müssen der Encoder eingereicht worden sein
und Geräteverlust oder ein wgpu-Validierungsfehler fehlen. Ein erfolgreicher
Map-Callback für genau diesen Buffer ist dann das Abschlussereignis nach seiner
GPU-Nutzung. Ein Map-Erfolg ohne Buffer-ID beweist dagegen nur, dass irgendein
Buffer lesbar wurde.

Die CPU-Bytes vor dem PNG-Encoding sind unverzichtbar. Nur so lassen sich
Render-/Readback-Fehler von Formatkonvertierung, RGB-Quantisierung oder einem
legitim schwarzen Szenenbild trennen.

## Kontrollierte Fensterzustände

Bevys Felder `focused` und `visible` beschreiben nicht die Bedingung, die
`wgpu-hal` auf macOS prüft. Für jeden Capture-Frame sind auf dem
AppKit-Hauptthread mindestens diese nativen Werte nötig:

- `NSWindow.isVisible`
- `NSWindow.isMiniaturized`
- `NSWindow.isKeyWindow`
- Rohwert von `NSWindow.occlusionState` und dessen Visible-Bit

Die Messung soll die Zustände so klassifizieren:

| Zustand | Erforderlicher nativer Befund |
| --- | --- |
| sichtbar, nicht fokussiert | `isVisible=true`, `isMiniaturized=false`, `isKeyWindow=false`, Occlusion-Visible-Bit gesetzt |
| kontrolliert verdeckt | `isVisible=true`, `isMiniaturized=false`, Zielrechteck vom eigenen, höher geordneten Occluder überdeckt, Occlusion-Visible-Bit nicht gesetzt |
| versteckt | `isVisible=false`; gesonderte Klasse, auch wenn das Occlusion-Visible-Bit ebenfalls fehlt |
| minimiert | `isMiniaturized=true`; gesonderte Klasse |
| wieder sichtbar | `isVisible=true`, `isMiniaturized=false`, Occlusion-Visible-Bit gesetzt |

Der entscheidende Wert ist der Rückgabestatus der tatsächlichen Akquise im
Renderframe. Ein AppKit-Sample einige Frames davor oder danach kann den Status
während `get_current_texture()` nicht ersetzen. Umgekehrt beweist
`acquire=Occluded` die von wgpu verwendete Bedingung, aber ohne
`isVisible=true` und `isMiniaturized=false` noch keine Verdeckung im engeren
Sinn. Das eigene Occluder-Fenster muss das Zielrechteck tatsächlich abdecken.

Die kleinste saubere Folge ist in einem Prozess:

1. sichtbar und nicht fokussiert,
2. mit eigenem Fenster vollständig verdeckt,
3. wieder sichtbar und weiterhin ohne zusätzlichen Simulationstick.

Pro Zustand wird genau ein Request ausgewertet. Automatische Wiederholungen bis
zu einem passenden Ergebnis würden die zeitliche Aussage zerstören.

## Falsifizierbare Vorhersagen

### H1: Surface-Schranke überspringt die Kopie

Vorhersage für den verdeckten Capture:

```text
native ordered-in + occlusion_visible=false
-> acquire=Occluded
-> View fehlt im selben render_frame_id
-> prepare(T2, B2) vorhanden
-> kein copy(T2 -> B2)
-> map_success(B2)
-> CPU-Bytes von B2 RGB0
-> Guard lehnt genau diese Entity ab, keine PNG-Datei
```

Das View-Format kann nach einem vorherigen erfolgreichen Frame erhalten bleiben.
`extract_windows` entfernt nach dem Present nur den alten View
(`window/mod.rs:159-165`). In einem von Beginn an verdeckten Prozess können
dagegen View und Format fehlen. Beide Werte müssen deshalb getrennt ins Log.

Die sichtbaren Captures davor und danach müssen bei eingefrorenem
Simulationszustand `Success|Suboptimal`, View, `copy=true`, Map-Erfolg auf ihrem
jeweiligen Buffer und das erwartete nichtschwarze Szenenbild liefern. Damit
ändern sich im reversiblen A/B/A-Versuch Surface-Status, Copy und Bild gemeinsam,
während Kamera- und Szenenzustand gleich bleiben.

**Akzeptiert für diesen Lauf** ist H1 nur mit der vollständigen ID-Kette. Der
Guard darf das schwarze PNG verhindern; die nullwertigen Rohbytes vor dem
Adapter ersetzen in diesem Arbeitsstand das historische Dateiartefakt.

**Widerlegt für einen Capture** ist die Skip-Copy-Erklärung, wenn derselbe
`prepare_id` einen vorhandenen View, eine eingereichte Kopie von seiner
Capture-Textur in seinen Buffer, einen erfolgreichen Map-Abschluss dieses
Buffers und dennoch RGB0 liefert. Dann muss die Ursache vor der Kopie, in den
gerenderten Texturdaten, oder nach dem Map liegen.

**Nicht entschieden** ist H1 bei `copy=true` ohne Submit-/Buffer-Zuordnung,
bei `map_success` ohne Buffer-ID oder wenn der schwarze Capture einen anderen
Kamera- oder Simulationszustand hatte.

### H2: veraltete Kamera- oder Resize-Daten

Für denselben `prepare_id` werden zusätzlich erfasst:

- konfigurierte Surface-Größe,
- `ExtractedWindow.physical_width/height`,
- Capture-Textur- und Copy-Extent,
- Main-World-`Camera::computed`-Zielgröße und Viewport,
- extrahierte Kamera-Zielgröße, Viewport, Ziel-Window, `is_active`,
  `CameraOutputMode` und Kamera-Updategeneration,
- Resize- und Scale-Nachrichten seit dem letzten Warp.

Die Hypothese sagt nach einem nativen Resize zwischen Warp und Capture voraus:

1. Surface, extrahiertes Fenster und Capture-Textur besitzen schon die neue
   Größe, Kamera oder Viewport noch die alte.
2. Akquise, View, Copy und Map sind trotzdem für genau diesen Capture
   erfolgreich.
3. Genau ein kontrollierter Warp verarbeitet die ausstehende
   Kameraaktualisierung und beseitigt die Größenabweichung.
4. Erst danach erscheint bei unverändertem logischem Szenenzustand das
   erwartete Bild.

**Akzeptiert als Ursache eines RGB0-Captures** ist H2 erst, wenn der Capture vor
dem Warp RGB0 und der Capture danach korrekt ist, beide vollständige Copy-/Map-
Ketten besitzen und Surface-Status sowie Renderbereitschaft gleich bleiben.
Eine gemessene Größenabweichung ohne den vorhergesagten Bildwechsel belegt nur
die Schedule-Lücke.

**Widerlegt für einen Capture** ist H2, wenn alle vier Größen und Viewports im
schwarzen Frame übereinstimmen und die extrahierte Kamera dieselbe
Updategeneration wie die Main-World-Kamera trägt. Ein fehlender Copy im selben
Frame macht H2 als Erklärung der ausgelesenen Nullen ebenfalls unnötig, auch
wenn daneben eine Resize-Lücke besteht.

Gleiche PNG-Abmessungen allein widerlegen H2 nicht. Eine alte Kamera kann in
eine neu dimensionierte Capture-Textur zeichnen, deren spätere PNG-Abmessung
trotzdem korrekt ist.

### H3: Cold-Renderbereitschaft

Die enge, prüfbare Form lautet: Bei stabilem Fenster, stabiler Kamera und ohne
weitere Simulationsticks wird aus einem anfangs unvollständigen Renderframe
nach fortschreitender Render-App ein korrektes Bild.

Jeder Capture der Zeitreihe braucht daher eine erfolgreiche Surface-, Copy- und
Map-Kette. Zusätzlich sind bekannte Bereitschaftssignale nötig, zum Beispiel
Status der verwendeten Renderpipelines, Ausführung der relevanten Rendergraph-
Nodes und Anzahl der für die Kamera extrahierten sichtbaren Objekte. Nur
`Ready` oder verstrichene Zeit ist kein solches Signal.

**Akzeptiert für einen Lauf** ist H3, wenn ein früher Capture trotz
vorhandenem View und erfolgreichem Copy/Map RGB0 oder nur den bekannten
Clearwert enthält, ein dazu passendes Bereitschaftssignal noch fehlt und ein
späterer Capture ohne Warp, Resize oder Occlusion-Wechsel nach Umschlag dieses
Signals das erwartete Bild enthält.

**Widerlegt ist die einfache Variante "nur etwas warten"**, wenn bereits der
erste Capture bei vollständiger Bereitschaft korrekt ist oder wenn alle
Bereitschaftssignale umschlagen, die vollständige Copy-Kette stabil bleibt und
das Bild trotzdem RGB0 bleibt. Eine Reihe unter durchgehend fehlendem View
prüft H3 nicht, weil sie den Bildinhalt nie in den Buffer überträgt.

Falls die vermutete Bereitschaft erst einen weiteren Main-World-Tick benötigt,
ist das eine eigene Hypothese. Sie darf durch einen einzelnen diagnostischen
Warp geprüft, aber nicht nachträglich als "Warten" bezeichnet werden.

## Einfluss des aktuellen Guards

Der Guard ist Teil des beobachteten Systems und darf nicht still umgangen
werden:

- Bei fehlendem View oder Format setzt er für den ersten Capture-Frame
  `surface=false`, lehnt früh mit `screenshot_window_unavailable` ab und schreibt
  keine Datei.
- Bevy kann den bereits vorbereiteten Buffer danach trotzdem erfolgreich mappen.
  Dieser späte Readback ist Rohbeleg für den Bevy-Pfad, aber kein zweiter
  Capture und kein Adaptererfolg.
- Bei `surface=true` akzeptiert der Adapter auch ein tatsächlich schwarzes
  Image. Das ist beabsichtigt und verhindert eine Pixelheuristik.
- Ein später sichtbarer Frame kann einen früheren fehlgeschlagenen Frame wegen
  des `OnceLock` nicht nachträglich freigeben.

Darum müssen rohe Bevy-Ereignisse und Adapterentscheidung getrennt protokolliert
werden. "Keine schwarze PNG-Datei" belegt mit aktivem Guard nicht, dass Bevy
keinen nullwertigen Readback erzeugt hat. Umgekehrt wäre ein künstliches
Abschalten des Guards eine Änderung der Versuchsbedingung und für den
Kausalnachweis nicht nötig.

## Andere Bevy-Pfade zu RGB0

Ein vorhandener View schließt schwarze Bilder nicht aus. Er schließt nur den
konkreten `continue` vor `render_screenshot` aus.

1. **Kein final schreibender Kamerapass.** Eine inaktive Kamera wird nicht
   extrahiert. Eine Kamera mit `CameraOutputMode::Skip`, falschem Ziel oder
   fehlendem Rendergraph kann die Capture-Textur unbeschrieben beziehungsweise
   nur gecleart lassen.
2. **Pipeline noch nicht verfügbar.** Der Metal-Upscaling-Pfad öffnet ohne
   fertige Pipeline nur den Output-Renderpass. Bei schwarzer Clear-Farbe kann
   die korrekt kopierte Textur deshalb RGB0 enthalten. Die separate
   Screenshot-to-Screen-Pipeline ist dagegen keine Erklärung für RGB0 im
   Buffer, weil die Kopie vor ihrer Abfrage aufgezeichnet wird.
3. **Veraltete Sichtbarkeit oder Extraktion.** Im kontrollierten Leerlauf
   laufen Main-World-Transform-, Visibility- und Kamera-Systeme nicht. Eine
   korrekte Kamera kann daher eine leere oder alte Draw-Liste besitzen.
4. **Legitim schwarzer Inhalt.** Schwarze Clear-Farbe, eine schwarze Szene oder
   RGB-null bei relevantem Alpha sind gültige Renderresultate. Pixelinhalt
   allein darf keinen Fehlerstatus auslösen.
5. **Surface-Akquise aus anderem Grund.** `Occluded` ist nicht die einzige
   Ursache eines fehlenden Views. `Outdated` mit fehlgeschlagenem Retry,
   `Timeout`, `Lost`, `Validation`, eine fehlende Surface oder das Fehlen einer
   schreibenden Fensterkamera führen ebenfalls dorthin. Diese Fälle bestätigen
   den View/Copy-Mechanismus, aber nicht native Verdeckung als Auslöser.
6. **Größen- oder Formatfehler beim Copy.** Surface-Konfiguration,
   `ExtractedWindow`, Capture-Textur und Copy-Extent stammen aus verschiedenen
   Strukturen. Die normale Bevy-Reihenfolge hält sie zusammen. Ein
   Validierungsfehler muss trotzdem erfasst werden, statt einen vorherigen
   CPU-Log als ausgeführten Copy zu werten.
7. **Konvertierung nach dem Readback.** Raw-Format, Paddingentfernung,
   `try_into_dynamic()` und `to_rgb8()` liegen zwischen Map und PNG. Der
   Vergleich von Rohhash und erneut dekodiertem PNG trennt diesen Pfad von
   Render- und Copyfehlern.

Der spezielle Erfolgspfad "vorbereiteter Buffer wird ohne Copy gemappt" hat in
Bevys Standardablauf eine engere Bedingung. Bei vorhandenem
`ExtractedWindow`, View-Format, View und vorbereitetem Zustand ruft
`submit_screenshot_commands` die Kopie ohne weitere Pipeline-Schranke auf.
Fehlt dagegen der vorbereitete Zustand, iteriert `collect_screenshots` auch
nicht über dessen Buffer. Ein erfolgreich gemapptes, aber nicht kopiertes
Window-Screenshot verlangt daher den fehlenden Window-/View-Zweig oder eine
zusätzliche, eigens nachzuweisende Änderung zwischen Vorbereitung und Submit.

## Abschlussurteil

### Überprüfte Fakten

- Bevy koppelt den Readback-Copy an die Verfügbarkeit des Präsentations-Views
  und mappt vorbereitete Buffer auch nach einem übersprungenen Copy.
- wgpu liefert für unbeschriebene Mappingbereiche Nullen und schließt die
  Buffer-GPU-Nutzung vor dem erfolgreichen Map-Callback ab.
- `wgpu-hal` entscheidet macOS-Occlusion anhand des nativen
  `NSWindow.occlusionState`, nicht anhand von Fokus.
- Der aktuelle Guard verhindert das falsche PNG-Erfolgsergebnis, verändert aber
  nicht Bevys vorherige Vorbereitung und späteres Mapping.
- Resize kann Surface- und Kameradaten wegen der angehaltenen Main-Schedules
  zeitweise auseinanderlaufen.

### Noch zu prüfende Hypothesen

- Der kontrollierte In-Process-Wechsel erzeugt genau im verdeckten
  Capture-Frame `Occluded`, fehlenden View, fehlenden Copy und einen
  nullwertigen Map desselben Buffers.
- Wiederherstellung der nativen Sichtbarkeit stellt bei eingefrorener Szene
  Akquise, Copy und korrektes Bild reversibel wieder her.
- Kein gleichzeitig auftretender Resize-/Kameraversatz oder
  Renderbereitschaftswechsel erklärt denselben Bildwechsel besser.
- Historische Schwarzbilder ohne diese IDs lassen sich dem Mechanismus nicht
  nachträglich framegenau zuordnen.
