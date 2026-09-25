# Headless-Integration gegen Bevy 0.19.1

Dieses Dokument ist der **maßgebliche Integrations- und Migrationsplan** für den
Headless-Umbau. Der Produktvertrag steht in [target.md](target.md#headless-betrieb),
der unmittelbar nächste Arbeitsschritt in [next-steps.md](next-steps.md).
Historische ADRs, Research- und Diagnoseberichte erklären Herkunft und Evidenz;
sie sind keine konkurrierenden Arbeitsanweisungen.

## Statusbegriffe

- **IST** bezeichnet heute ausführbaren Code, auch wenn er nur eine begrenzte
  Fixture unterstützt.
- **ZIEL** bezeichnet das gewünschte Verhalten, nicht eine vorhandene Funktion.
- **CPU-belegt** bedeutet Quellen-, Typ-, Schedule- oder Zustandsnachweis ohne
  ausgeführten Renderer.
- **GPU-belegt** gilt nur für den konkret ausgeführten Aufbau, die genannte
  Plattform und die geprüften Bild-/Ereigniskriterien.

Die zentrale transparente Integration ist noch nicht implementiert. Insbesondere
hat die neue öffentliche Render-World-Probe ihren Render-Schedule **nicht**
ausgeführt und weder GPU-Readback noch Pixelinhalt bestätigt.

## Zielvertrag

Woodpecker soll ein nahezu unverändertes Spiel so rendern und bedienen, wie es
im bisherigen Fensterbetrieb läuft. Das Spiel bleibt Eigentümer von:

- seinen echten Main-World-`Window`-/`PrimaryWindow`-Daten einschließlich
  Auflösung und Skalierung,
- `Camera2d` und `Camera3d`, `Projection`, FoV, Clipping, Viewports,
  Kamera-Order/Stacks, `Transform` und `GlobalTransform`,
- Rendergraph, Materialien, Licht, Schatten, UI und normalen Picking-Observern,
- Spielzeit, Schedules und der Bedeutung eines Ticks.

Woodpecker übernimmt zentral nur Ausgabequelle, virtuelle Eingabequelle und die
bereits vorhandene explizite Ticksteuerung. Daraus folgen verbindlich:

1. Kein manueller 2D-/3D-Schalter als Spielvertrag.
2. Keine Headless-Markierung der Spielkamera, Parallelkamera oder
   `RenderTarget::Image`-Umschreibung durch die Anwendung als Zielvoraussetzung.
3. Keine szenenspezifischen Adapter, eigene Projektionsformel, eigenes
   Raycasting oder eigene Sichtbarkeitslogik.
4. Bevy bleibt für Kameraableitung, Rendering, UI-Layout und Picking zuständig.
5. Capture, Inspect und reine Command-Annahme sind keine Ticks. Input wird erst
   im ausdrücklich ausgeführten Warp an Spielsysteme zugestellt.
6. Eine konfigurierte `Window`-Entity ist weder automatisch ein OS-Fenster noch
   eine erfundene Dummyidentität. OS-Handle und Surface sind getrennte Ebenen.
7. Szenenkameras, Materialien, Licht, Schatten und Observer werden nicht für
   einen Test passend verändert. Eine vom Spiel gewählte feste Auflösung oder
   ein FoV ist kein Woodpecker-Eingriff.

Mehrere Kameras desselben normalisierten Ziels bilden einen Stack. Mehrere
unabhängige Ziele dürfen nicht über „erste Kamera“ still ausgewählt werden;
ihre spätere Auswahl braucht einen ausdrücklichen Vertrag oder eine sichtbare
Mehrdeutigkeitsablehnung.

## IST und ZIEL

| Bereich | IST | ZIEL |
| --- | --- | --- |
| Fenster-Capture | Primäres Window mit `RawHandleWrapper`; requestbezogener Surfaceguard | Als unveränderter Fenstermodus erhalten, solange er nicht nachweislich ersetzt ist |
| Headless-Capture | Opt-in `headless-2d`/`headless-3d`, Marker, genau eine Vollbild-Imagekamera und fixture-nahe Readiness | Zentrales internes Outputattachment für die vorhandene normalisierte Window-Zielidentität; keine Spielkameraänderung |
| Kamera | Begrenzte Pfade klassifizieren 2D oder feste Perspektiv-3D | Beliebige normale Camera2d/Camera3d-Stacks bleiben Bevy-/spielbestimmt |
| Keyboard | Window-Events oder interne Headless-Message nach Marker-/Imageauswahl | Vorhandene Windowidentität und normale Bevy-`ButtonInput`-Semantik ohne Markerpflicht |
| Pointer | Primärwindow oder markierte 2D-Imagekamera | PointerLocation auf dem echten normalisierten Spielziel; Bevys `RayMap`/Picking berechnet Rays und Viewports |
| UI | Windowpfad ersetzt `ui_focus_system` kurzzeitig mit virtuellem Windowcursor; Imagefixture hat Sonderweg | Bestehende Bevy-UI-Systeme und echte Window-/Kameradaten verwenden; nur zentrale virtuelle Eingabequelle |
| Tick/Lifecycle | `Control` hält Spielschedules an; Renderer läuft weiter; 3D-Fixture brauchte einen Camera-Startfix | Explizite Warps bleiben; Render-/Cluster-Lifecycle zentral lösen, ohne versteckte Updates oder Spielkamera-Hack |
| Capture-Service | Eine Queue, Requestkorrelation, 30-s-Frist, PNG-Worker, sichere Pfade und Cleanup | Denselben Service erweitern; kein zweiter Screenshot-Service |

Die schmalen Imagepfade bleiben bis zu grünen Ersatznachweisen funktionsfähige
Fixtures. Ihre Grenzen sind kein Produktziel.

## Öffentliche Bevy-Anschlussstelle: CPU-belegt

Die CPU-Probe
[`window_headless_backend_probe.rs`](../../bevy_test_apps/tests/window_headless_backend_probe.rs)
belegt gegen Bevy 0.19.1:

- Eine echte Main-World-Entity mit `Window + PrimaryWindow`, aber ohne
  `RawHandleWrapper`, liefert über Bevys öffentliches `camera_system`
  `ComputedCameraValues`, physische/logische Größen, Scale und Viewportdaten.
- Camera2d und Camera3d behalten `RenderTarget::Window`, Projektion/FoV,
  Transform und Zielidentität. Ein Resize aktualisiert die abgeleiteten Daten.
- Bevys öffentliches `RayMap::repopulate` erzeugt ohne eigene Projektionsformel
  Rays für zwei Kameras desselben normalisierten Window-Ziels und berücksichtigt
  Viewport/Scale.
- Öffentliche `KeyboardInput`-/`MouseButtonInput`-Messages können dieselbe
  Window-Entity tragen und Bevys `ButtonInput` aktualisieren, ohne OS-Eventloop.
- `ViewTargetAttachments` ist die dokumentierte öffentliche Render-World-
  Anschlussstelle: Nach `prepare_view_attachments` und vor
  `prepare_view_targets` kann für denselben `NormalizedRenderTarget` ein
  `OutputColorAttachment` eingesetzt werden.
- Ein internes `GpuImage` mit `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_SRC`
  kann compile-seitig dieses Attachment liefern; `Readback::texture` auf
  denselben `Handle<Image>` ist eine öffentliche mögliche Fortsetzung.

Die Probe registriert den Hook nur. Sie führt weder den Render-Schedule noch
`RenderDevice`, Adapter, Winit oder GPU aus. Deshalb sind Format, tatsächlicher
Draw, Copy, Mapping und Pixelinhalt **nicht** belegt. Der Standardpfad
`Screenshot(Window)` ist keine Ausweichlösung: Bevy 0.19.1 sucht dafür in
`WindowSurfaces` und überspringt ein Windowziel ohne Surface.

Der Kandidat verändert `ExtractedCamera.target` nicht. Die normalisierte
Window-Identität bleibt Key für Kamera-Stack und Attachment; nur das finale
Outputattachment wird intern ersetzt. `Readback::texture` ist ein Kandidat,
nicht vorab die einzig zulässige fertige Architektur.

Ergänzende Rohbelege liegen unter
`target/window-headless-cpu-probe.lnKQuY/`. Das Ziel und diese Zusammenfassung
hängen nicht von der Verfügbarkeit dieses temporären Ordners ab.

## Integrationsgrenzen

### Ausgabe und Capture

Der zentrale Backendadapter muss in den vorhandenen Service unter
`src/session/screenshot/capture.rs` integriert werden:

- Queue, Request-ID-Korrelation, Completion, PNG-Encoding, Pfadsicherung,
  Überschreibregel, gemeinsame Deadline und Cleanup bleiben erhalten.
- Das interne Attachment muss vollständige Zielidentität, physische Größe,
  Skalierung und Format des konkreten normalisierten Targets bewahren.
- Gleiche Target-Keys teilen einen Kamerastack; verschiedene Targets werden
  getrennt gehalten.
- Resize/Scale-/Targetwechsel dürfen kein altes Attachment erfolgreich einem
  neuen Request zuordnen.
- Der bestehende Surfaceguard bleibt für echten Fenstercapture aktiv.
- Mapping oder ein schwarzer Pixelwert allein beweist keine vollständige
  Renderausgabe. Legitim schwarze Bilder bleiben gültig.

`PerspectiveState` in `capture/image_target.rs` ist ein legitimer read-only
Capture-Snapshot. Er prüft, ob eine bereits ausgewählte feste Fixturekamera
zwischen Annahme und Capture verändert wurde; er schreibt weder Projektion noch
FoV. Beim Umbau darf er nicht fälschlich als Kameraeingriff gelöscht werden.
Ob er nach der allgemeinen Zielrevalidierung noch separat nötig ist, wird erst
mit Ersatztests entschieden.

### Input und Picking

Die vorhandenen Commandzustände für Press/Hold/Release, Pointerposition und
Textfokus bleiben der Ausgangspunkt. Die Migration soll:

- eine tatsächlich vorhandene Window-/Targetidentität wiederverwenden,
- Input weiterhin erst unmittelbar vor den gewarpten Spielschedules flushen,
- Bevys `ButtonInput`, `PointerInput`, `RayMap`, UI-Picking und Observer nutzen,
- keine `Window`-Entity erfinden und kein OS-Ereignis vortäuschen, wenn keine
  Spielidentität existiert,
- keine neue generalisierte Inputplattform oder szenenspezifische Raylogik bauen.

Native Cursorgrab-/Relative-Motion-, IME- und OS-Fokusparität sind kein Versprechen
dieser Migration. Text-/UI-Fokus sowie vollständige Hover/Press/Release/Drag-
Reihenfolge benötigen eigene CPU-Verträge und später begrenzte Integrationstests.

### Render- und Cluster-Lifecycle

Das Rendern darf zwischen Warps fortschreiten; die Simulation nicht. Zu lösen
sind insbesondere:

- internes Output-Image anlegen/resize-synchron extrahieren,
- Hook pro Renderframe nach dem Leeren der Attachments einsetzen,
- Readback/Completion ohne `Update` des Spiels zustellen,
- 3D-Cluster und Renderressourcen vor dem ersten tatsächlich renderbaren Frame
  zentral bereitstellen oder sichtbar warten lassen.

Der heutige 3D-Fixture-Startfix bleibt bis zum Ersatznachweis. Er darf nicht
vorschnell entfernt oder als allgemeine Architektur übernommen werden. Kein
versteckter Startup-Tick und kein pauschales `PostUpdate` im Leerlauf.

### Asset- und Nachrichtenpflege

Die vorhandenen CPU-Proben zur Assetpflege zeigen gültige Teilbefunde: registrierte
Asset-Publisher lassen sich vor Schedule-Initialisierung verschieben, und
Assetnachrichten haben unabhängige Spiel-/Renderleser. Sie belegen keinen fertigen
Produktionsadapter, keine GPU-Bereitschaft und kein Speicherlimit. Der kleinste
zentrale Integrationsschritt darf diese Forschung nur übernehmen, wenn der
konkrete Renderpfad sie benötigt; sie ist keine Vorbedingungskette für jede
Headless-Änderung.

## Geschützte Verträge

Folgende Regeln werden nicht für die Migration neu erfunden oder abgeschwächt:

- Spielprotokoll v3 und Requestkorrelation;
- ausschließlich explizite Warps als Simulationsfortschritt;
- Capturefreeze und Inputzustellung erst im Warp;
- Resize erhält Windowidentität und aktualisiert Bounds;
- vollständige Attachmentidentität einschließlich Scale;
- normalisierte sichere PNG-Ziele, atomischer Ersatz und Root-Isolation;
- begrenzte Deadlines, unbekannte mutierende Outcomes niemals automatisch
  wiederholen, Prozessgruppen- und Ressourcencleanup;
- bestehender Fenstermodus und requestbezogener Surfaceguard;
- kein Game-State-Patching, kein Enginefork und keine Szenenadapter.

Tests, die diese Regeln schützen, bleiben erhalten. Tests, die ausschließlich
die heutige Ein-Kamera-/Vollbild-/Markerbegrenzung festschreiben, werden erst
nach grünem Ersatz gezielt geändert; fehlerschließende Readinessprüfungen werden
nicht pauschal gelöscht.

## Migrationsreihenfolge und Abschlusskriterien

### Schritt 0 — Dokumente und CPU-Anschlussstelle

**Status: abgeschlossen.** Dieses Dokument ist konsolidiert; die öffentliche
Attachmentstelle, Windowidentität, Camera-ComputedInfo, RayMap und ButtonInput
sind CPU-seitig belegt.

Abschlusskriterien:

- ein maßgeblicher aktueller Ziel-/Migrationspfad ohne konkurrierende „nächste
  Schritte“;
- Dateimatrix und IST/ZIEL-/CPU/GPU-Abgrenzung;
- keine Behauptung eines ausgeführten Renderhooks.

### Schritt 1 — kleinster zentraler Integrationsschritt

**Status: ausstehend; nächster Entwicklungsschritt.**

In den vorhandenen Capture-/Session-/Inputpfad einen internen Adapter für genau
ein eindeutig gewähltes normales Windowziel integrieren. Ausgangspunkt sind die
öffentlichen Bevy-Hooks und CPU-Verträge; Referenz bleibt die unveränderte
`mesh_picking`-Spielwelt. Kein GPU-Lauf gehört automatisch zu diesem Schritt.

Abschlusskriterien:

- Main-World-Window/Kameras/Projektionen/Viewports/Transforms/Targets sind vor
  und nach Outputvorbereitung unverändert;
- ein Target-Key kann intern ein größen-/formatpassendes Attachment erhalten;
- gleicher Key bildet einen Stack, unabhängige Ziele werden nicht vermischt;
- Queue, PNG, Deadline, Capturefreeze und Cleanup bleiben derselbe Service;
- Input nutzt echte Zielidentität und Bevys Systeme; keine Markerpflicht im
  neuen Pfad;
- CPU-Regressionen für Resize, Scale, Teilviewport, Stack, Ambiguität,
  Input-Warpgrenze, Readbackkorrelation und Cluster-/Lifecycle-Fehler;
- bestehende Fixturepfade und deren Tests bleiben noch vorhanden.

### Schritt 2 — begrenzter GPU-Paritätsnachweis

**Status: nicht freigegeben und nicht ausgeführt.** Er benötigt eine gesonderte
explizite Autorisierung.

Fenster- und Headlesslauf verwenden denselben realen `mesh_picking`-Inhalt, die
selben Bevy-Einstellungen, Ticks und Eingaben. Geprüft werden Bildgröße,
Viewport/Stack, Objekt-/UI-Geometrie, normale Picking-Observer, Eventfolge,
Capturefreeze und begrenzte sachliche Bildabweichung. Keine bitgenaue
PBR-Garantie und keine OS-spezifischen Cursorgrab-/IME-Versprechen.

Abschlusskriterien:

- PBR-Material, Licht, Schatten, Text/UI und normale Camera3d bleiben in beiden
  Läufen unverändert;
- MSAA/HDR/Tonemapping/Outputformat und Readback werden dokumentiert;
- ein eventueller Cluster-Lifecyclefix ist zentral und erzeugt keinen Tick;
- genau autorisierte Läufe, keine automatischen Retries, vollständige Evidenz
  und Cleanup.

### Schritt 3 — alte Einschränkungen abbauen

**Status: erst nach grünen Ersatznachweisen.**

Marker-/Imagefixture-Spielpflichten und zugehörige Begrenzungstests zusammen mit
Source und Anleitung gezielt entfernen oder zu rein historischen Fixtures
machen. Es bleibt kein dauerhaft paralleles Integrationsmodell als gleichwertige
Produktanleitung.

Abschlusskriterien:

- jede entfernte Einschränkung hat einen gleichwertigen grünen CPU-/GPU-
  Ersatznachweis;
- öffentliche Markerexports und Features werden nur koordiniert mit Nutzern,
  Tests und Dokumentation geändert;
- Surface-Fensterpfad wird nur entfernt, wenn sein Funktionsumfang nachweislich
  ersetzt ist.

## Dateigenaue Migrationsmatrix

### Session, Capture und Ausgabe

| Dateien | Heutige Rolle | Zielaktion | Abhängigkeit / erforderlicher Nachweis |
| --- | --- | --- | --- |
| `src/session/plugin.rs` | `Control`, Warp-Schleife, Inputflush, Capturepoll und Render-Leerlauf | **behalten, gezielt anpassen** | Keine Änderung an v3/Korrelation/Tickzählung; zentraler Render-/Cluster-Lifecycle ohne zusätzliche Spielschedules; Capturefreeze-Regression |
| `src/session/screenshot.rs`, `src/session/mod.rs` | Installation, Verfügbarkeit und öffentliche 2D-/3D-Markerexports | **anpassen; Marker erst nach Ersatznachweis entfernen** | Neuer Pfad darf keine Marker verlangen; Feature-/API-Entfernung erst in Schritt 3 mit Source+Tests+Docs |
| `src/session/screenshot/capture.rs` | Einziger Screenshot-Service: Queue, Targetwahl, Deadline, Completion, PNG-Worker | **behalten und erweitern** | Internes Attachmentbackend hier integrieren; kein zweiter Service; Requestkorrelation, Timeout, späte Readbacks und No-Tick belegen |
| `src/session/screenshot/destination.rs` | sichere Zielprüfung und atomisches PNG-Schreiben | **behalten** | Bestehende Pfad-/Symlink-/Overwrite-Tests unverändert grün |
| `src/session/screenshot/capture/surface.rs` | requestbezogener Guard des echten Window-Surfacepfads | **behalten** | Nur nach gleichwertigem Fenstercapture-Ersatz ändern; Headlessbackend darf Guard nicht vortäuschen |
| `src/session/screenshot/capture/image_target.rs` | feste Marker-/Imagekamera-Auswahl, read-only `PerspectiveState`, Renderframe-Verifikation | **schrittweise ersetzen; Snapshot nicht fälschlich löschen** | Allgemeines Target-/Stack-Mapping und zentrale Revalidierung müssen 2D/3D/Viewport/Scale abdecken; `PerspectiveState` ist kein Kameraeingriff |
| `src/session/screenshot/capture/selection.rs` | Fenstervorrang und feste 2D/3D-Ein-Kamera-Regeln | **ersetzen nach Ersatztests** | Explizite Regeln für gleiche Target-Stacks und unabhängige Ziele; keine „erste Kamera“-Wahl |
| `src/session/screenshot/capture/readiness.rs` | fixture-nahe finale Pipeline-/3D-Phase-Prüfung | **anpassen, nicht pauschal entfernen** | Öffentliche allgemeine Readiness soweit möglich; HDR/MSAA/Tonemapping/PBR/Fehlerzweige CPU/GPU prüfen |
| `src/session/screenshot/capture/verification.rs` | requestkorrelierter Warmup-/Capture-Frame-Zustand | **behalten oder in allgemeinen Backendzustand überführen** | Später Frame darf fehlgeschlagenen Request nicht reparieren; Requestentity-/Targetkorrelation belegen |
| `src/session/window.rs` | eindeutige echte `Window + PrimaryWindow`-Identität | **behalten** | CPU-Verträge für kein RawHandle, Resize/Scale und Mehrdeutigkeit |

### Input und Schedules

| Dateien | Heutige Rolle | Zielaktion | Abhängigkeit / erforderlicher Nachweis |
| --- | --- | --- | --- |
| `src/command/input/keyboard.rs`, `src/command/input/pointer.rs`, `src/command/input/text.rs` | öffentliche virtuelle Inputcommands und Zustandsregeln | **behalten** | Keine neue Inputplattform im Headless-Umbau; vorhandene Validation, Recording-/Replaydarstellung und Outcomes bleiben stabil |
| `src/session/input/keyboard.rs` | Window-`KeyboardInput` plus Marker-/Image-basierte interne Headless-Message | **anpassen** | Echte Spiel-Windowidentität und normale `ButtonInput`-Lebensdauer ohne Marker; Press/Hold/Release nur im Warp; keine Dummyidentität |
| `src/session/input/pointer.rs`, `src/session/input/pointer/tests.rs` | Windowziel oder markierte 2D-Imagekamera; `PointerInput`; Resize-Regressionen | **anpassen; Bounds-/Identitytests behalten** | Normalisiertes echtes Ziel, Teilviewport/Scale/Stack, Release/Scroll/Drag und Resize; RayMap statt eigener Projektion |
| `src/session/input/ui.rs` | ersetzt `ui_focus_system` mit temporärer virtueller Windowcursor-Sicht | **gezielt anpassen** | UI-Layout/Fokus/Picking mit echter Windowidentität; Cursorzustand exakt restaurieren; vollständige Eventreihenfolge CPU-prüfen |
| `src/session/input/text.rs`, `src/session/input/text/tests.rs` | virtueller Textfokus und tickgebundene Zustellung | **behalten, gegen neuen Zielpfad regressieren** | Kein OS-Fokus/IME-Versprechen; Entitybindung und Fokuswechsel erhalten |
| `src/session/plugin/input_tests.rs` und Input-Unit-Tests | Warpgrenzen, Isolation, Window-/Fixturefälle | **behalten; Markerbegrenzungen später gezielt ersetzen** | Schutztests zuerst ergänzen, alte Limitierungstests erst nach grünem neuen Pfad ändern |

### CPU-Proben und Begrenzungstests

| Dateien | Heutige Rolle | Zielaktion | Abhängigkeit / erforderlicher Nachweis |
| --- | --- | --- | --- |
| `bevy_test_apps/tests/window_headless_backend_probe.rs` | CPU-Beleg für Window/ComputedInfo/RayMap/Input und compile-only Attachment/Readback | **behalten und als stabile Anschlussprobe nutzen** | Klar halten: Render-Schedule nie ausgeführt, kein GPU-/Readback-/Pixelbeleg |
| `bevy_test_apps/tests/headless.rs`, `bevy_test_apps/tests/headless/readiness.rs` | isolierte GPU-Imageprobe und fixture-nahe Readinessentscheidung | **reine technische/historische Fixture; vorerst behalten** | Nicht als Zielarchitektur verwenden; Fehler-/No-Tick-/schwarze-Szene-Verträge als Referenz |
| `bevy_test_apps/tests/headless_session_capture.rs` | Produktionsquellen eingebundene Marker-/Image-Capturetests | **anpassen, sobald zentraler Pfad existiert** | Neue Tests zuerst rot/grün; alte Marker-/Ein-Kameraassertions erst danach ersetzen |
| `bevy_test_apps/tests/asset_maintenance.rs` und `asset_maintenance/*` | CPU-Forschung zu Publishern, Lesern und Retention | **behalten als begrenzte Forschung** | Nur bei konkretem Bedarf integrieren; kein GPU-/Produktionsnachweis |
| bestehende Window-/Surface-/Destinationtests in `src/session/screenshot/*` | Fenstermodus, Guard, Queue, PNG und sichere Pfade | **behalten** | Müssen während Schritt 1 unverändert grün bleiben |

### Testanwendungen und Szenen

| Dateien | Heutige Rolle | Zielaktion | Abhängigkeit / erforderlicher Nachweis |
| --- | --- | --- | --- |
| `bevy_test_apps/src/lib.rs` (`composition::rendered`) | normale gerenderte App-Komposition mit Window | **Referenz behalten; später zentralen Backendstart ermöglichen** | Keine zweite Szenenkomposition; Windowdaten/Resolution/Scale bleiben erhalten |
| `bevy_test_apps/src/bin/mesh_picking.rs` | reale Camera3d-/PBR-/Licht-/Schatten-/Text-/Picking-Referenz; heutiger Slice-Startfix | **primäre Paritätsreferenz, nicht für Headless umschreiben** | Schritt-2-Vergleich mit identischem Inhalt/Settings/Ticks/Input; Clusterfix nur zentral oder klar fixture-spezifisch |
| `bevy_test_apps/src/bin/game_menu.rs` | normale Menüszene plus heutiger `--headless`-Marker-/Imagezweig | **normale Szene behalten; Headlesszweig nach Ersatznachweis zurückbauen** | Abgeschlossener Menü-GPU-Lauf ist Rückfallevidenz; kein Zielvertrag für Marker/Parallelziel |
| `bevy_test_apps/src/bin/headless_session.rs`, `headless_session_3d.rs`, `headless_session/diagnostics.rs` | feste 2D-/3D-Imagefixtures und Diagnose | **historische/enge Testfixtures behalten** | Nicht zur allgemeinen App-Anleitung erheben; erst nach neuen Paritätsnachweisen entfernen/archivieren |
| übrige gerenderte Binaries (`context_menu`, `ui_drag_drop`, `blend_modes`, `logical_state`) | bestehende Fenster- und Eingaberegressionen | **behalten** | Spätere Breitenprüfung, aber kein Scope für den kleinsten Schritt 1 |

### GPU-Skripte, Fixtures und Anleitungen

| Dateien | Heutige Rolle | Zielaktion | Abhängigkeit / erforderlicher Nachweis |
| --- | --- | --- | --- |
| `tests/headless_session.py`, `tests/headless_session_keyboard.py` | begrenzte 2D-Imagefixture-Abnahmen | **historische Rückfallevidenz; nicht als nächste Zielabnahme** | Keine Ausführung ohne neue Freigabe; Markerpfad erst nach Ersatznachweis abbauen |
| `tests/headless_session_3d.py`, `tests/headless_session_3d_keyboard.py`, `tests/headless_session_3d_expectations.py` | begrenzte prozedurale 3D-/Keyboard-Abnahmen | **historische Rückfallevidenz** | Cluster-/Projectionserkenntnisse erhalten; keine allgemeine PBR-/Pickingaussage |
| `tests/fixtures/headless_session.toml`, `headless_session_3d.toml`, `game_menu_headless.toml` | Launchkonfiguration der heutigen Imagefixtures | **behalten bis Schritt 3** | Nicht als notwendige Konfiguration des zentralen Zielpfads dokumentieren |
| `tests/mesh_picking.py`, `tests/fixtures/mesh_picking.toml` | heutige Fensterreferenz | **für Schritt 2 wiederverwenden/gezielt ergänzen** | Gleiche Szene und Settings; neuer Headlesslauf nur nach ausdrücklicher GPU-Freigabe |
| `README.md`, `bevy_test_apps/README.md` | Nutzerbefehle und Fixturehistorie | **IST/ZIEL klar trennen** | Geplanter zentraler Pfad nicht als nutzbar ausgeben; GPU-Kommandos bleiben opt-in |
| `Cargo.toml`, `bevy_test_apps/Cargo.toml` | heutige `headless-2d`-/`headless-3d`-Features und Fixturetargets | **in Schritt 1 nicht ändern; in Schritt 3 koordiniert bereinigen** | Keine Dependency- oder Lockfileänderung für den Dokument-/CPU-Schritt; Featureänderung erst zusammen mit Source, Nutzern und Ersatztests |

## Historische Evidenz und ihre Reichweite

Die bisherigen Läufe bleiben gültige Rückfallevidenz, begrenzen aber das Ziel
nicht:

- Der feste 2D-Imagepfad und seine Keyboardbewegung wurden auf macOS/Metal
  erfolgreich geprüft. Ein früherer Schwarzbildfehler zeigte, dass vollständige
  `ImageRenderTarget`-Identität einschließlich Scale erhalten werden muss.
- Der feste prozedurale 3D-Imagepfad und seine `D`-Bewegung wurden auf
  macOS/Metal erfolgreich geprüft. Ein vorheriger Fehler zeigte die
  Clusterinitialisierungslücke vor dem ersten Warp.
- Der bildgesteuerte `game_menu`-Lauf erreichte High und anschließend Game mit
  unverändertem Capture-/Warpvertrag. Er belegt nur die heutige begrenzte
  Marker-/Imagefixture.
- Die Fensterdiagnose belegt, dass ein fehlendes Surface nicht als erfolgreicher
  Window-Screenshot gespeichert werden darf.

Diese Ergebnisse sind abgeschlossene Demos und Diagnosebelege. Sie sind keine
Autorität für eine dauerhafte Markerpflicht, feste Vollbildkamera, 2D/3D-Auswahl
oder szenenspezifische Readiness.

## Noch offene Nachweise

| Bereich | Offen |
| --- | --- |
| Renderhook | tatsächlicher GPU-Draw in internes Attachment, Formatkompatibilität, Resize-Umschaltung |
| Readback | Copy/Map/Row-Padding/PNG aus dem internen Output und requestgenaues Cleanup |
| Kamera | Teilviewports, mehrere Kameras eines Stacks, mehrere unabhängige Ziele, Targetwechsel |
| Rendering | PBR, Licht, Schatten, Text/UI, MSAA, HDR, Tonemapping und CompositingSpace |
| Lifecycle | 3D-Cluster vor erstem Frame ohne versteckten Tick; verzögerte Pipelines/Assets |
| Input | vollständige Pointer-/UI-/Text-Eventparität; native Cursorgrab/IME ausdrücklich außerhalb |
| Plattform | begrenzter Linuxlauf ohne Displayserver erst nach separater Freigabe |

Bis diese Nachweise grün sind, bleibt der zentrale Weg ein **machbarer Kandidat**,
nicht eine ausgelieferte transparente Headless-Implementation.
