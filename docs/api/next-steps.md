# Unmittelbar nächste Schritte

Maßgeblicher Ziel- und Migrationsplan:
[headless-integration.md](headless-integration.md). Dieses Dokument enthält
bewusst nur den nächsten ausführbaren Entwicklungsschritt und die Bedingungen
für die beiden Folgephasen.

## Jetzt: Schritt 1 — kleinster zentraler Integrationsschritt

Implementiere im vorhandenen Session-/Capture-Service einen internen
Outputadapter für genau ein eindeutig gewähltes normales Bevy-Windowziel. Die
Spielwelt, insbesondere `mesh_picking`, bleibt dabei unverändert:

- keine Markerpflicht,
- keine Parallelkamera und keine Umschreibung zu `RenderTarget::Image`,
- keine feste 2D-/3D-Auswahl,
- keine eigene Projektions-, Raycasting- oder Sichtbarkeitslogik,
- kein neuer Screenshot-Service und kein versteckter Tick.

Ausgangspunkt sind die CPU-belegten öffentlichen Bevy-0.19.1-Anschlussstellen:
`ViewTargetAttachments`, `OutputColorAttachment`, ein internes `GpuImage`, eine
mögliche `Readback::texture`-Kette sowie echte Windowidentität für
`camera_system`, `RayMap` und Input. Der Renderhook wurde bisher nicht ausgeführt;
deshalb muss die Implementierung zunächst durch CPU-Verträge begrenzt werden.

### Erforderliche Verträge

1. Main-World-Kameras, Projektionen/FoV, Viewports, Order/Stacks, Transforms und
   `RenderTarget::Window` sind vor und nach der Outputvorbereitung unverändert.
2. Attachmentmapping bewahrt vollständige normalisierte Targetidentität,
   physische Größe, Scale und Format; Resize und Targetwechsel werden
   requestgenau revalidiert.
3. Mehrere Kameras desselben Targets bleiben ein Stack. Mehrere unabhängige
   Targets werden nicht implizit durch „erste Kamera“ entschieden.
4. Queue, Protokoll-v3-Korrelation, Deadline, PNG-Worker, sichere Pfade,
   atomischer Ersatz und Cleanup bleiben im bestehenden Service.
5. Capture bleibt ein No-Tick-Vorgang; Input erreicht das Spiel erst im Warp.
6. Keyboard und Pointer nutzen eine tatsächlich vorhandene Window-/Targetentity
   und Bevys `ButtonInput`, `RayMap`, Picking und UI-Systeme; keine
   Dummyidentität oder eigene Raylogik.
7. Fenstercapture und sein requestbezogener Surfaceguard bleiben intakt.
8. Die heutigen Marker-/Imagefixtures und ihre Tests bleiben bis zu grünen
   Ersatznachweisen verfügbar.

Die dateigenauen Änderungen und Nachweise stehen in der
[Migrationsmatrix](headless-integration.md#dateigenaue-migrationsmatrix).
`PerspectiveState` ist ein read-only Capture-Snapshot und darf nicht als
Kameraeingriff behandelt oder vorschnell entfernt werden.

## Danach nur unter folgenden Bedingungen

### Schritt 2 — GPU-Parität

Erst nach grünen CPU-Verträgen und **separater ausdrücklicher GPU-Freigabe**.
Fenster- und Headlesslauf müssen denselben realen `mesh_picking`-Inhalt, dieselben
Bevy-Einstellungen, Ticks und Inputs verwenden. Zu belegen sind mindestens PBR,
Licht, Schatten, Text/UI, Camera3d, normaler Picking-Observer, Teilviewport/
Kamerastack, tatsächlicher Readback sowie die relevanten MSAA-/HDR-/Tonemapping-
Eigenschaften. Der Lauf darf keine automatischen Retries oder Szenenpatches
enthalten.

### Schritt 3 — alte Einschränkungen abbauen

Erst nach grünem Ersatznachweis für die jeweilige Funktion. Dann werden
Markerpflicht, feste Imagekamera-/2D-/3D-Auswahl, Fixturekonfigurationen, Tests
und Anleitungen koordiniert geändert oder als historische Evidenz abgegrenzt.
Der Window-Surfacepfad bleibt, solange sein Funktionsumfang nicht nachweislich
ersetzt ist.

## Nicht als nächster Schritt

- kein weiterer `game_menu`- oder feste-3D-Fixture-GPU-Lauf,
- kein Linux-/DISPLAY-loser Paritätslauf ohne separate Freigabe,
- kein Enginefork und keine Dependency-/Featuremigration,
- keine szenenspezifische Kamera-, Material-, Licht- oder Clusteranpassung,
- kein pauschales Löschen fehlerschließender Readiness- oder Surfaceguard-Tests.
