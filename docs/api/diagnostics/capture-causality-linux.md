# Capture-Kausalität auf Linux/X11 mit Bevy 0.19.1

Stand: Der freigegebene Linux-Lauf wurde am 22. September 2026 genau einmal
mit dem angekündigten Befehl gestartet. Er wurde nach dem ersten, sichtbar und
nicht fokussiert ausgeführten Capture angehalten, weil bereits dieses bekannte
Ausgangsbild vollständig nullwertig war. Teilverdeckung, Vollverdeckung und
Wiederherstellung wurden gemäß der Stopregel nicht mehr ausgeführt. Es gab
keine Wiederholung und keinen Backendwechsel.

## Ergebnis und Einordnung

Der Lauf beobachtete einen **anderen Fehlerpfad** als den auf macOS belegten
Skip-Copy-/Nullbuffer-Pfad. Der Wrapper schrieb beim sofortigen Assertion-Abbruch
den konservativen Fallback `inconclusive_state_control_or_infrastructure` in
`result.json`; die nachfolgende Auswertung der vollständigen ersten ID-Kette
präzisiert den technischen Befund als „anderer Fehlerpfad beobachtet“:

```text
X11-Fenster sichtbar, viewable, unobscured und nicht fokussiert
-> Vulkan-Surface-Akquise success
-> Screenshot-Textur und Readback-Buffer vorbereitet
-> copy_texture_to_buffer aufgezeichnet
-> Queue mit Copy eingereicht
-> Mapping desselben Buffers erfolgreich
-> 3.686.400 rohe Nullbytes
-> Guard akzeptiert wegen vorhandener Surface
-> formal gültiges schwarzes PNG
```

Damit ist der macOS-Pfad in diesem Lauf **nicht reproduziert**: Die Copy wurde
nicht übersprungen, und die Surface-Akquise war erfolgreich. Der
Verdeckungsvergleich selbst ist nicht entscheidbar, weil seine bekannte
nichtschwarze Ausgangsbedingung schon im ersten Zustand fehlschlug und der Lauf
deshalb vor den drei weiteren Zuständen endete. Das ist keine fehlende
Zustandskontrolle des ersten Zustands; dessen X11-Zustand wurde stabil bestätigt.
Es ist auch kein Nachweis dafür, wie sich dieses oder andere Linux-Systeme bei
Vollverdeckung verhalten.

Der aktive woodpecker-Guard verhielt sich entsprechend seinem Vertrag: Er
verhindert einen Erfolg nach fehlendem Window-View, verwendet aber keine
Schwarzpixelheuristik. Da View, Format und Copy vorhanden waren, akzeptierte er
das von Bevy gelieferte schwarze Image. Dieser Befund ist daher von der auf
macOS gemessenen Guard-Ablehnung nach `copy_skipped` zu unterscheiden.

## System und Versionen

| Feld | Wert |
| --- | --- |
| Distribution | Linux Mint 22.3 (Zena) |
| Kernel | `7.0.0-31-generic`, x86_64 |
| Desktop | Cinnamon 6.6.9 |
| Window-Manager/Compositor | Muffin 6.6.3 |
| Sitzung | X11, `DISPLAY=:0` |
| GPU | Intel HD Graphics 520, PCI `0000:00:02.0` |
| Kernel-Treiber | `i915` |
| Mesa | 25.2.8 (`25.2.8-0ubuntu0.24.04.2`) |
| tatsächlich gewählter wgpu-Adapter | `Intel(R) HD Graphics 520 (SKL GT2)`, `IntegratedGpu` |
| tatsächlich gewähltes Backend | Vulkan |
| wgpu-Treiberangabe | `Intel open-source Mesa driver` |
| Rust | `rustc 1.95.0 (59807616e 2026-04-14)` |
| Cargo | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |
| Bevy | `bevy 0.19.1`, `bevy_render 0.19.1` |
| wgpu | `wgpu`, `wgpu-core`, `wgpu-hal`, `wgpu-types` jeweils 29.0.4 |

Der ausgewählte Adapter war die Intel-GPU und nicht das ebenfalls von
`vulkaninfo` angebotene llvmpipe. Der Befund ist deshalb kein
Software-Rendering-Befund.

Die vollständige Vorabumgebung steht in
`target/capture-causality-linux/environment-preflight.json`. Die strukturierte
Laufzeitentscheidung steht als `adapter_selected` in
`events/bevy_render-325749.jsonl` und wird durch `server.stderr.log` bestätigt.

## Werkzeuge und Schutz des Arbeitsbaums

Neu angelegt wurden:

- `tests/diagnostics/capture_causality_linux.py`
- `tests/diagnostics/capture_causality_linux_deck.py`
- `tests/diagnostics/capture_causality_linux_instrument.py`
- `tests/diagnostics/capture_causality_linux_native.rs`
- `tests/diagnostics/capture_causality_linux_support.py`
- `tests/diagnostics/capture_causality_linux_support_test.py`

Der Vorbereitungsschritt kopierte woodpecker, `bevy_test_apps` und
`bevy_render 0.19.1` nach `target/capture-causality-linux/`. Nur diese Kopien
wurden instrumentiert. Root-Manifeste, Root-Lockfiles, Produktionscode und die
Cargo-Registryquelle blieben unverändert. Der SHA-256 der ursprünglichen
`screenshot.rs` blieb
`f49e4ca4f0929fc51a5c55fc002294c6009ae7a4c1c07dc6ef6c8ff737df0c5b`.

Die Diagnose schreibt getrennte JSONL-Shards je Quelle und PID:

```text
events/bevy_render-325749.jsonl
events/woodpecker_adapter-325749.jsonl
events/x11_main-325749.jsonl
events/deck_x11-326524.jsonl
events/controller-325675.jsonl
```

Jede Rust-Quelle serialisiert die vollständige Zeile einschließlich Newline
unter einem prozesslokalen Mutex; Controller und Deck flushen vollständige
Zeilen. Schreibfehler sind fatal und sichtbar. Die Korrelation verwendet
Session-, Placement-, Request-, Screenshot-Entity-, Frame-, Capture-, Textur-
und Buffer-IDs, nicht Dateireihenfolge oder zeitliche Nähe.

## Befehle und Headless-Vorbereitung

Vor dem Grafiklauf wurden unter anderem ausgeführt:

```sh
python3 tests/diagnostics/capture_causality_linux.py --prepare-only
RUSTUP_TOOLCHAIN=1.95.0 cargo +1.95.0 test --all-features -- --test-threads=1
cargo +1.95.0 test --no-default-features -- --test-threads=1
```

Abschließend bestanden 136 Root-Unit-/CLI-Tests, 7 Beobachtungstests und 13
Session-Prozesstests mit allen Features. Ohne Default-Features bestanden 92
Root-Tests sowie erneut 7 Beobachtungs- und 13 Session-Prozesstests. Der
headless Librarytest der kopierten Bevy-Test-App, vier neue
Ergebnisklassifikationstests und sieben bestehende JSONL-/Mehrschreibertests
bestanden ebenfalls.

Der einzige freigegebene Grafikbefehl war:

```sh
python3 tests/diagnostics/capture_causality_linux.py
```

Es wurde kein zweiter Grafiklauf gestartet.

## Tatsächlicher Fenster- und Simulationszustand

Die angekündigte Folge war sichtbar ohne Fokus, teilweise verdeckt,
vollständig verdeckt und wieder sichtbar. Tatsächlich erreicht wurde nur der
erste Zustand:

| Messung | Wert |
| --- | --- |
| Ziel-XID | `85983236` |
| Zielrechteck | `(86, 114, 1280, 720)` |
| Zielzustand | `map_state=viewable`, `bevy_visible=true` |
| X11-Visibility | `unobscured` |
| Fokus | Ziel nicht fokussiert; Fokus auf eigenem Deck-XID `88080385` |
| Deckrechteck | `(1914, 1074, 4, 4)` |
| Schnittfläche | 0 von 921.600 Pixeln |
| Stabilitätsnachweis | 8 identische Samples über 652.820.878 ns |
| Bevy-Größe/Skalierung | 1280×720, Faktor 1 |

Beide Kameras waren aktiv, hatten eine physische Zielgröße von 1280×720 und
keinen Viewport. Die 3D-Kamera stand bei `[0, 2.5, 10]`; ihre Rotation blieb
unverändert. Drei Inspect-Snapshots vor Platzierung, vor Capture und nach
Capture waren für `SceneState`, `Camera` und `Transform` identisch. Der Zustand
enthielt Alpha 0,9, HDR aus, Unlit aus, Kamerawinkel 0 und den erwarteten Seed
`0x5eedb1e5`.

Es gab genau einen expliziten `tick.warp.start {"ticks":1}` vor dem Capture und
keinen weiteren Simulationstick. Nach dem fehlerhaften Ausgangsbild wurde kein
zweiter Capture angefordert.

## Acquire-, Copy- und Readback-Kette

Der einzige Request hatte Request-ID 6 und Screenshot-Entity `450v0`. Seine
IDs waren Frame/Capture/Textur/Buffer `22/1/1/1`.

| Schranke | Messwert |
| --- | --- |
| Surface-Akquise | `success`; Surface, physisches Fenster und Capture jeweils 1280×720 |
| Größenänderung | `size_changed=false`, `present_mode_changed=false` |
| Vorbereitung | `Bgra8UnormSrgb`, `bytes_per_row=5120`, zwei schreibende Kameras |
| Copy | `copy_texture_to_buffer_encoded`, Textur 1 → Buffer 1 |
| Queue | `queue_submitted`, `copy_encoded=true` |
| Mapping | `map_requested`, anschließend `map_success`, jeweils Buffer 1 |
| CPU-Rohdaten | 3.686.400 Bytes; alle null; RGB-nonzero 0; Alpha min/max 0/0 |
| Adapter | Guard akzeptierte Request 6 als `completed`, 1280×720 |

Die 127 während der Prozesslaufzeit protokollierten Surface-Akquisen bestanden
aus 126 `success` und einer `suboptimal`; es gab kein `occluded`. Für den
Capture-Frame selbst gilt `success`. Es gab kein `copy_skipped`, keinen
Device-Loss und keinen wgpu-Validierungsfehler.

Die Kette belegt eine tatsächlich eingereichte Copy und das spätere erfolgreiche
Mapping desselben Buffers. Sie widerlegt damit für diesen Capture die spezielle
Erklärung „Copy wegen fehlendem Swapchain-View übersprungen“. Sie bestimmt noch
nicht, warum die erfolgreich kopierte Capture-Textur vollständig nullwertig
war; eine weitere Diagnose wäre ein neuer, gesondert freizugebender Schritt.

## Bilddaten

| Artefakt | Ergebnis |
| --- | --- |
| Rohreadback | 3.686.400 Nullbytes, SHA-256 `0c660f2bd3eff3150dd0040789abe2291613b9af319df870203d4f77a4913a5f` |
| PNG | 1280×720, 15.057 Bytes, SHA-256 `7f0fcfad3151bf17c4ba0faa396039ee753bfd086f998f751361bedcb6a3e4b4` |
| dekodiertes PNG | genau ein RGB-Wert; 921.600 dunkle Pixel; keine hellen oder rot-dominanten Pixel |

Das Bild verfehlte damit die vorab festgelegte Ausgangsbedingung der bekannten
Blend-Szene. Ein Vergleich mit Teil-, Voll- oder Wiederverdeckung existiert
nicht. Insbesondere gibt es keinen Nachweis eines korrekten Bildes trotz
Verdeckung und keinen Nachweis eines durch Verdeckung ausgelösten
Surface-Ausfalls.

## Artefakte

Der Lauf liegt vollständig unter:

```text
target/capture-causality-linux-run-xl2md8er/
```

Wichtige Dateien:

- `result.json`: maschinenlesbarer Abbruch und einzige Capture-Kette;
- `events/*.jsonl`: getrennte Ereignisquellen;
- `raw-readbacks/capture-1-buffer-1-1280x720-Bgra8UnormSrgb.raw`;
- `artifacts/3530681c9cf0c0eb93e75dfa00649450/screenshots/01-visible-unfocused.png`;
- `server.stderr.log`: Adapter- und Bevy-Laufzeitangaben;
- `cleanup.json`: Prozessbereinigung.

Die Artefakte wurden nach dem unerwarteten Ergebnis nicht entfernt oder durch
einen weiteren Lauf ersetzt.

## Grenzen

- Die vierteilige Plattformmessung wurde nicht abgeschlossen. Der Lauf sagt
  nichts darüber aus, ob Vollverdeckung auf diesem System eine Surface-Akquise
  verhindert.
- Der Nullbuffer entstand trotz erfolgreicher Copy und ist deshalb nicht der
  bekannte macOS-Skip-Copy-Pfad.
- Aus einem einzelnen Intel/Vulkan/X11-System folgt keine Aussage über Linux
  allgemein, Wayland, andere Compositoren, GPUs oder Backends.
- Es wurde weder ein Fix noch eine Architekturänderung, Projektmigration,
  Szenenmatrix oder Bevy-0.20-Prüfung vorgenommen.

## Cleanup und Grafikslot

Alle eigenen Prozesse wurden beendet:

| Prozess | PID | Exitcode | läuft noch |
| --- | ---: | ---: | --- |
| Bevy-Test-App | 325749 | über Session-Stopp beendet | nein |
| X11-Deck | 326524 | 0 | nein |
| woodpecker-Server | 325688 | 0 | nein |

Das Deckfenster wurde unmappt und zerstört. Die abschließende Prozessprüfung
fand keinen laufenden Prozess aus diesem Versuch. Der Grafikslot ist
freigegeben: `GUI_SLOT_RELEASED`.
