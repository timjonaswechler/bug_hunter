# Diagnose schwarzer Screenshots: Cold Start und Renderbereitschaft

Stand: 21. September 2026. Die Messung lief auf dem gemeldeten Mac M1 Pro mit
Bevy 0.19.1. Sie untersucht den Befund, sie ändert kein Runtime-Verhalten.

## Ergebnis

Die enge Messreihe stützt keinen einfachen Cold-Start-Ablauf nach dem Muster
"der erste Capture ist zu früh, weitere Captures desselben Zustands werden
später grün". In vier frischen Prozessen blieben alle 24 Captures vollständig
RGB-null, bis zu 9,63 Sekunden nach dem einzigen expliziten Warp. Der
reflektierte Szenen-, Kamera- und Transformzustand blieb dabei unverändert.

Für die in dieser Untersuchung erzeugten Schwarzbilder ist dagegen eine
vollständige Occlusion-Kette belegt:

1. AppKit meldete das eigene `NSWindow` als sichtbar, aber nicht key und ohne
   `NSWindowOcclusionState::Visible`.
2. Nach Bevys `prepare_windows` hatten alle 139 beobachteten Renderframes weder
   Swapchain-Texture noch Swapchain-View oder View-Format.
3. Alle vier Captures dieses Laufs waren vollständig RGB-null.

Ein späterer frischer Prozess lieferte die Gegenbeobachtung. AppKit meldete das
Fenster als key und `occlusion_visible=true`. Alle 184 beobachteten Renderframes
hatten Texture, View und View-Format. Alle vier Captures enthielten jeweils
2.763.510 RGB-Bytes ungleich null. Beide Bedingungen verwendeten genau
einen Warp-Tick und danach keine weiteren Ticks.

Das belegt den Mechanismus für die hier gemessenen schwarzen Bilder. Es beweist
nicht, dass der historische Lauf `target/blend-modes-f9gag8t5` oder jeder
frühere Mesh-Befund durch Occlusion verursacht wurde. Der Wechsel zum späteren
sichtbaren Prozess war nicht kontrolliert reproduzierbar. Er ist deshalb kein
Nachweis einer Cold-Start-Ursache.

## Messprogramm und Befehl

Das dauerhafte Messprogramm liegt unter
`tests/diagnostics/cold_start_frozen_captures.py`. Der Hauptlauf war:

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml \
  --features slice --bin blend_modes
python3 tests/diagnostics/cold_start_frozen_captures.py \
  --sessions 4 --captures 6
```

Das Programm startet einen eigenen Server auf `127.0.0.1:0`. Pro Session
startet es einen frischen `blend_modes`-Prozess und führt folgende Folge aus:

1. `Ready` abwarten.
2. Die benannte Kamera und ihren Zustand lesen. Vor dem Warp ist die
   3D-Kamera erwartungsgemäß inaktiv.
3. Genau `tick.warp.start {"ticks": 1}` ausführen.
4. Sechs unterschiedliche PNGs nacheinander aufnehmen, ohne einen weiteren
   Tick anzufordern.
5. Nach jedem Capture `SceneState`, `Camera` und `Transform` lesen.
6. Dauer, RGB-Nullprüfung, PNG-SHA-256, Command-Ausgabe und Zustand als JSONL
   schreiben.

Schwarz ist ein Messergebnis und kein Abbruchkriterium. Das Programm wiederholt
nicht bis zu einem grünen Bild. Es löscht keine Datei-, Cargo-, Shader- oder
GPU-Caches.

## Hauptmessreihe

Artefakte: `target/cold-start-9u7gcjre`

| frischer Prozess | Ready nach | Warp-Dauer | erster Capture nach Warp | letzter Capture nach Warp | Ergebnis |
| --- | ---: | ---: | ---: | ---: | --- |
| 1 | 9.146 ms | 115 ms | 3.241 ms | 6.892 ms | 6/6 RGB-null |
| 2 | 10.136 ms | 408 ms | 4.116 ms | 9.634 ms | 6/6 RGB-null |
| 3 | 11.084 ms | 376 ms | 3.242 ms | 7.182 ms | 6/6 RGB-null |
| 4 | 9.161 ms | 122 ms | 3.388 ms | 6.906 ms | 6/6 RGB-null |

Alle 24 PNGs hatten denselben SHA-256-Wert. In jeder Session war der Zustand
über alle Captures identisch. Die Kamera war nach dem Warp aktiv und ihr
berechnetes Renderziel hatte durchgehend 1280 × 720 physische Pixel.

Damit ist für diese Bedingung Folgendes diskriminiert:

- Reale Wartezeit und weitere Render-/Capture-Durchläufe ohne Simulationstick
  genügten nicht. Es gab kein spontanes Später-grün innerhalb eines Prozesses.
- Ein erster kalter Prozess unterschied sich nicht von den drei weiteren
  frischen Prozessen mit bereits warmen Dateisystem- und Treibercaches.
- Die schwarze Ausgabe entstand nicht durch einen fortgeschriebenen
  Testzustand. Es gab nach dem festen Warp keinen weiteren Tick.

## Fenster- und Renderprobe

Die Probe war ausschließlich temporär in der privaten Kopie eingebaut und mit
`[DEBUG-cold-start-window]` sowie `[DEBUG-cold-start-view]` markiert. Die
AppKit-Abfrage war durch `NonSend<MainThreadMarker>` an den Prozess-Hauptthread
gebunden. Der Render-Sub-App-Hook lief direkt nach Bevys `prepare_windows`.
Nach der Messung wurden Probe und temporäre Abhängigkeiten entfernt. Das
unveränderte Target wurde neu gebaut.

### Tatsächlich occluded und schwarz

Relevanter Lauf: `target/cold-start-z2kb05em`

Die Hauptthread-Probe meldete:

```text
focused=false visible=true miniaturized=false key=false
occlusion=NSWindowOcclusionState(8192) occlusion_visible=false
```

Die Probe enthielt zuvor einen Aktivierungs- und Front-Anforderungsversuch für
dieses eigene Fenster. Der unmittelbar danach gelesene Zustand blieb occluded.
Es gab in diesem Prozess keinen zweiten AppKit-Abtastpunkt. Unabhängig davon
blieben alle 139 Render-Abtastungen über 7.707 ms bei:

```text
swap_chain_texture=false swap_chain_view=false view_format=false
```

Die vier Captures waren RGB-null. Sie endeten 3.214, 3.887, 4.560 und
5.222 ms nach dem Warp. Ihr reflektierter Zustand war gleich.

Ein zusätzlicher Lauf mit unverändertem Fensterwunsch,
`target/cold-start-d8niwlgx`, zeigte ebenfalls 137 Renderframes ohne View und
vier schwarze Captures. Dessen erste AppKit-Abfrage war noch nicht
hauptthreadgebunden und wird daher nicht als AppKit-Nachweis verwendet.

### Tatsächlich sichtbar und grün

Relevanter Lauf: `target/cold-start-bjwe7bt3`

Dieser frische Prozess enthielt nur lesende Fenster- und Renderinstrumentierung.
Er änderte Fokus, Level, Position und Space des Fensters nicht. AppKit meldete:

```text
focused=true visible=true miniaturized=false key=true
occlusion=NSWindowOcclusionState(8194) occlusion_visible=true
```

Alle 184 Render-Abtastungen über 9.431 ms meldeten:

```text
swap_chain_texture=true swap_chain_view=true view_format=true
```

Die vier Captures endeten 3.448, 4.541, 5.637 und 6.651 ms nach dem Warp.
Keiner war schwarz. Jeder enthielt 2.763.510 RGB-Bytes ungleich null,
alle vier PNGs waren bytegleich, und der reflektierte Zustand blieb gleich.

Das ist eine A/B-Beobachtung anhand des tatsächlich gemessenen Fensterzustands,
keine erfolgreiche kontrollierte Sichtbarkeitsintervention. Der sichtbare
Zustand trat erst in einem späteren frischen Prozess auf.

### Fokus- und Frontversuche

`focused: true` allein machte das Fenster nicht nachweislich sichtbar:

- `target/cold-start-55atml2a`: drei frische Prozesse, 18/18 schwarze Captures.
- `target/cold-start-x63x19db`: AppKit meldete weiterhin `key=false` und
  `occlusion_visible=false`; 138 Render-Abtastungen hatten keine View, 4/4
  Captures waren schwarz.

Begrenzte Versuche nur am eigenen Fenster mit `NSApplication`-Aktivierung,
`makeKeyAndOrderFront`, `orderFrontRegardless`, Always-on-top und
Move-to-active-space erreichten in den betreffenden Prozessen ebenfalls keinen
gemessenen sichtbaren Zustand. Die Artefakte sind:

- `target/cold-start-z2kb05em`
- `target/cold-start-rtccoh2k`
- `target/cold-start-pd8h_kk4`
- `target/cold-start-opoqnoun`

Diese Versuche änderten mehrere Fenstereigenschaften und eignen sich nicht als
saubere Einzelvariablen. Sie zeigen vor allem, dass `focused: true` oder ein
Frontwunsch nicht mit tatsächlicher AppKit-Sichtbarkeit gleichgesetzt werden
darf.

## Quellenmechanismus

Die Quellen von wgpu 29.0.4 und Bevy 0.19.1 erklären den stillen Nullausgang:

- `wgpu-hal-29.0.4/src/metal/surface.rs` gibt `SurfaceError::Occluded` zurück,
  wenn Metal kein Drawable liefert und der AppKit-Occlusion-State nicht
  `Visible` enthält.
- `bevy_render-0.19.1/src/view/window/mod.rs::prepare_windows` behandelt
  `CurrentSurfaceTexture::Occluded` mit einem leeren Zweig. Deshalb erscheint
  keine Renderer-Warnung und es entsteht keine Swapchain-View.
- `bevy_render-0.19.1/src/view/window/screenshot.rs::submit_screenshot_commands`
  überspringt das Kopieren, wenn `swap_chain_texture_view` fehlt.
- `collect_screenshots` liest den bereits vorbereiteten Buffer trotzdem. Der
  nicht beschriebene Mappingbereich ist in wgpu-core nullinitialisiert. So kann
  der Capture als erfolgreich abgeschlossen werden und ein vollständig
  schwarzes PNG liefern.

Das passt exakt zum Live-Lauf: occluded, keine View, keine Renderer-Warnung,
erfolgreiches Capture-Outcome und RGB-null.

## Bewertung der Hypothesen

### 1. Occlusion oder fehlende tatsächliche Fenstersichtbarkeit

Für die aktuell reproduzierten Schwarzbilder bestätigt. Der schwarze und der
grüne Live-Lauf unterscheiden sich entlang der gesamten gemessenen Kette:
AppKit-Occlusion-State, Swapchain-View und PNG-Inhalt.

Für die historischen sporadischen Läufe bleibt die Kausalität offen, weil deren
AppKit- und Renderzustand nicht aufgezeichnet wurde.

### 2. Asynchrone Pipeline-Bereitschaft nach einem Cold Start

Nicht bestätigt. Unter der schwarzen Bedingung wurden auch Folgecaptures nach
mehreren Sekunden nicht grün. Unter der sichtbaren Bedingung war die
Swapchain-View ab der ersten Render-Abtastung vorhanden und bereits der erste
Capture grün.

Die Messung isoliert Shader- oder Renderpipeline-Kompilation nicht unabhängig
von der Fenstersichtbarkeit. Sie kann daher nicht ausschließen, dass ein anderer
Fehler zusätzlich eine Pipeline-Race-Bedingung enthält.

### 3. `Ready` bedeutet Renderbereitschaft

Widerlegt. `Ready` aus `PostStartup` kann vorliegen, obwohl über viele
Renderframes keine Swapchain-View verfügbar ist. `Ready` belegt in diesem
Vertrag die Session-Bereitschaft, nicht ein erfolgreich präsentiertes
Fensterbild.

### 4. Der erste explizite Tick oder die Kameraaktivierung fehlt

Für diese Läufe widerlegt. Der Warp wurde mit genau einem ausgeführten Tick
bestätigt, die 3D-Kamera war danach aktiv, und das berechnete Ziel blieb
1280 × 720. Trotzdem waren Captures bei fehlender Swapchain-View schwarz.

### 5. Deckel- oder externer-Bildschirm-Ursache

Nicht untersucht und nicht bewiesen. Die Messung erfasst nur den tatsächlichen
Occlusion-State des Testfensters. Sie ordnet ihn weder dem Deckel noch einem
bestimmten Bildschirm zu.

## Grenzen und ausgeschlossene Läufe

- Es gelang nicht, innerhalb desselben eingefrorenen Prozesses kontrolliert von
  `occlusion_visible=false` zu `true` zu wechseln. Daher gibt es keinen
  Nachweis, dass bloßes Warten oder ein bestimmter Fokusaufruf den Zustand
  heilt.
- `target/cold-start-ht6804un` ist ausgeschlossen. Der erste Stand des
  Messprogramms las die Komponentenform falsch und brach vor Warp und Capture
  ab.
- `target/cold-start-pqn58xf7` ist ausgeschlossen. Eine frühe AppKit-Probe war
  nicht an den Hauptthread gebunden und löste absichtlich sichtbar einen Panic
  aus, bevor ein gültiger Capture-Lauf entstand.
- Die Diagnose hat keine globalen Caches geleert, keine Display- oder
  Sicherheitseinstellung geändert und keine fremde Session oder Anwendung
  beendet.
- Die Probe weist einen aktuellen Mechanismus nach, aber keine historische
  Alleinursache. Für eine historische Zuordnung müsste ein künftiger schwarzer
  Lauf AppKit-Occlusion, das Ergebnis von `get_current_texture` und das
  tatsächliche Ausführen des Screenshot-Copy im selben Capture korrelieren.

## Aufräumzustand

Alle von dieser Diagnose gestarteten Server, Sessions und gerenderten
Anwendungen sind beendet. Die temporäre Rust-/AppKit-Instrumentierung und ihre
Cargo-Abhängigkeiten sind entfernt. Das ursprüngliche `blend_modes`-Target wurde
danach neu gebaut; sein Binary enthält keine `[DEBUG-cold-start-*]`-Marker.
Logs, PNGs und JSONL-Messdaten bleiben wie verlangt unter `target/cold-start-*`.
Es wurden keine Commits oder Pushes erzeugt.
