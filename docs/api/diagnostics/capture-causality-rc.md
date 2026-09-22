# Capture-Kausalität mit Bevy v0.20.0-rc.1

Stand: Der freigegebene macOS-Lauf mit vier festen Captures ist abgeschlossen.
Der Fehler ist im festen Release Candidate reproduziert. Der Lauf verwendete
weder Bevy `main` noch eine Migration des Projekts.

## Ergebnis

Unter macOS mit Metal liefert Bevy `v0.20.0-rc.1` bei vollständiger nativer
Verdeckung weiterhin einen erfolgreichen, vollständig nullwertigen Readback aus
einem Buffer, in den keine Screenshot-Kopie geschrieben wurde.

Die Kette im verdeckten Capture lautet:

```text
NSWindow sichtbar, nicht minimiert, Visible-Bit nicht gesetzt
-> CurrentSurfaceTexture::Occluded
-> Screenshot-Textur und Readback-Buffer vorbereitet
-> kein swap_chain_texture_view
-> copy_texture_to_buffer übersprungen
-> Queue ohne diese Kopie eingereicht
-> Mapping desselben Buffers erfolgreich
-> 3.686.400 rohe Nullbytes
-> schwarzes PNG im eigenständigen Bevy-Repro
```

Nach dem Entfernen des eigenen Deckfensters kehren Surface-Akquise, Kopie und
das bytegleiche Bild ohne Änderung der Szene zurück. Teilverdeckung löst den
Fehler nicht aus.

Die Evidenz liegt unter:

```text
target/capture-causality-rc-run-7grh6a8p/
```

`result.json` enthält den maschinenlesbaren Befund
`reproduced_skip_copy_zero_readback`.

## Fester Release-Stand

Der Vorbereitungsschritt prüfte den öffentlichen Tag per `git ls-remote` und
verlangte genau diese Zuordnung:

| Feld | Wert |
| --- | --- |
| Bevy-Tag | `v0.20.0-rc.1` |
| Tag-Commit | `1b1f3ec1bec87386d7c19bda7d7870d4e18235c5` |
| Bevy-Paket | `0.20.0-rc.1` |
| lokal instrumentiertes `bevy_render` | `0.20.0-rc.1` |
| `wgpu` | `30.0.1` |
| `wgpu-core` | `30.0.1` |
| `wgpu-hal` | `30.0.1` |
| `wgpu-types` | `30.0.1` |
| temporäre Lockfile-SHA-256 | `2dfaa9468736dec75e2ddf326320830a5deba08bb41e13a0b92da98a589400c4` |

Das separate Projekt, seine Lockfile, die kopierte und instrumentierte
`bevy_render`-Quelle sowie der Cargo-Build liegen unter
`target/capture-causality-rc/`. Cargo verwendete die normale
`crates.io`-Registry. Es gab keine Registryquelländerung.

## Abweichung vom Projektadapter

Der normale woodpecker-Adapter hängt an Bevy 0.19. Eine Einbindung in den
Release Candidate hätte eine Projektmigration verlangt. Der Versuch verwendet
deshalb ein eigenständiges Bevy-Minimalprojekt und portiert woodpecker nicht.

Die statische Fixture übernimmt den visuellen Kern der vorhandenen
Blend-Modes-Szene:

- fünf Kugeln mit `Opaque`, `Blend`, `Premultiplied`, `Add` und `Multiply`;
- das schwarz-weiße Bodenraster mit 49 Feldern;
- denselben Punktlicht- und Kamerastand;
- Alpha 0,9 und eine feste, nichtschwarze Szene.

Die Diagnose-App enthält keine Bedienlogik und keine UI-Beschriftungen der
0.19-Test-App. Ihre Fixture-Kennung lautet
`blend-modes-static-v1:alpha=.9:camera=0,2.5,10:spheres=5:floor=49`.
Kamera, Fenstergröße, Skalierung und diese Kennung waren bei allen vier
Captures gleich.

Der woodpecker-Surface-Guard war in diesem eigenständigen Repro nicht
verlinkt. Deshalb schreibt Capture 3 das schwarze PNG. Das ist eine
Scopeabweichung und kein Verhalten des normalen Adapters. Der normale Guard,
die Root-Dependencies und die Root-Lockfile wurden nicht geändert. Dieser Lauf
macht keine neue Aussage darüber, wie der Guard unter einer Bevy-0.20-Migration
reagieren würde.

## Werkzeuge und feste Folge

Neue RC-spezifische Werkzeuge:

- `tests/diagnostics/capture_causality_rc.py`
- `tests/diagnostics/capture_causality_rc_instrument.py`
- `tests/diagnostics/capture_causality_rc_app.rs`
- `tests/diagnostics/capture_causality_rc_native.rs`
- `tests/diagnostics/capture_causality_rc_deck.swift`

Der Python-Treiber kopiert die RC-Quelle nach `target/`, instrumentiert nur die
Kopie und baut das eigenständige Projekt. Die App liest vier explizite
Capture-Kommandos. Es gibt keine Capture-Wiederholung bis zu einem passenden
Bild. Die Szene besitzt keine zeitabhängige Steuerung und wird zwischen den
Captures nicht verändert.

Das AppKit-System läuft im Prozess-Hauptthread. Vor jedem Capture verlangte der
Treiber acht aufeinanderfolgende native Samples mit identischer Fenster-,
Kamera- und Fixture-Messung. Die Samples deckten mindestens 100 ms ab. Die drei
Zustandswechsel nach dem ersten Capture hatten stabile Messspannen von
116.968 ms, 116.848 ms und 112.459 ms. Der lange erste Zeitraum enthielt den
initialen Renderaufbau und endete ebenfalls in acht identischen Samples.

Das Deck ist ein eigenes borderless AppKit-Fenster. Es verändert weder fremde
Fenster noch Systemeinstellungen. Für Teil- und Vollverdeckung wird seine
Geometrie aus dem zuvor gemessenen Zielrechteck berechnet und anschließend aus
dem tatsächlichen `NSWindow.frame` des Decks ausgewertet.

Die instrumentierte Bevy-Kopie vergibt monotone `frame_id`, `capture_id`,
`buffer_id` und `capture_texture_id`. Dieselben IDs stehen an Vorbereitung,
Copy oder Skip, Submit, Map und CPU-Auswertung. Jede Surface-Akquise enthält das
zuletzt auf dem Hauptthread atomar veröffentlichte AppKit-Sample.

## Geometrie und native Zustände

Das Ziel-`NSWindow` blieb bei `(120, 600, 1280, 752)`. Sein Inhaltsrechteck und
die Bevy-Auflösung blieben 1280 mal 720 bei Skalierungsfaktor 1. Das Fenster war
in allen vier Zuständen sichtbar, nicht minimiert und nicht fokussiert.

| Zustand | Deckrechteck | Schnittfläche mit Ziel | AppKit-Befund am Ziel |
| --- | --- | ---: | --- |
| sichtbar, nicht fokussiert | `(3434, 1406, 4, 4)` | 0 | `visible=true`, `key=false`, `miniaturized=false`, Occlusion `8194`, Visible-Bit gesetzt |
| teilweise verdeckt | `(760, 592, 648, 768)` | 481.280 von 962.560 Punkten, genau 50 Prozent | gleicher sichtbarer Befund, Visible-Bit gesetzt |
| vollständig verdeckt | `(112, 592, 1296, 768)` | 962.560 von 962.560 Punkten | `visible=true`, `key=false`, `miniaturized=false`, Occlusion `8192`, Visible-Bit nicht gesetzt |
| wieder sichtbar | `(3434, 1406, 4, 4)` | 0 | sichtbarer Ausgangsbefund wiederhergestellt |

Die Teilverdeckung wird damit durch die beiden gemessenen Fensterrechtecke
belegt. Sie wird nicht aus `occlusionVisible` abgeleitet. Das Vollfenster des
Decks überdeckte die gesamte gemessene Zielfläche mit acht Punkten Rand.

## Capture-Ketten

| Zustand | Frame, Entity, IDs | Acquire und Copy | Readback und PNG |
| --- | --- | --- | --- |
| sichtbar | Frame 11, Entity `369v0`, Capture/Buffer/Textur `1/1/1` | `success`; Copy aufgezeichnet; Queue mit Copy eingereicht | Map erfolgreich; 3.686.400 nichtnullwertige Rohbytes; 2.764.800 nichtnullwertige RGB-Bytes; Alpha 255; PNG-SHA-256 `c820d153d70a683faa9b491342727106adeed422ed35cf0de815d58c0445af16` |
| teilweise verdeckt | Frame 59, Entity `371v0`, IDs `2/2/2` | `success`; Copy aufgezeichnet; Queue mit Copy eingereicht | Rohdaten und PNG bytegleich zum sichtbaren Ausgangsbild |
| vollständig verdeckt | Frame 118, Entity `373v0`, IDs `3/3/3` | `occluded`; `copy_skipped` mit `swap_chain_view_missing`; Queue ohne Copy eingereicht | Map erfolgreich; 3.686.400 Nullbytes; RGB0 und Alpha0; Roh-SHA-256 `0c660f2bd3eff3150dd0040789abe2291613b9af319df870203d4f77a4913a5f`; schwarzes PNG-SHA-256 `7f0fcfad3151bf17c4ba0faa396039ee753bfd086f998f751361bedcb6a3e4b4` |
| wieder sichtbar | Frame 172, Entity `375v0`, IDs `4/4/4` | `success`; Copy aufgezeichnet; Queue mit Copy eingereicht | Rohdaten und PNG bytegleich zum Ausgangsbild |

Bei den vier Captures blieben Kameraübersetzung `[0, 2.5, 10]`,
Kamerarotation, aktiver Kamerastatus, Zielgröße 1280 mal 720, fehlender
Viewport, Surface-Größe und Fenstergröße gleich. `size_changed` und
`present_mode_changed` waren jeweils `false`.

Die bekannte Fixture ist in den drei nicht vollständig verdeckten Zuständen
bytegleich und nichtschwarz. Die Diagnose lehnt schwarze Szenen nicht
heuristisch als Fehler ab. Hier ordnen Surface-Status, fehlende Copy, identische
IDs und der reversible A/B/C/A-Vergleich die Nullbytes dem unbeschriebenen
Buffer zu, nicht einem legitim schwarzen Szeneninhalt.

`app.stderr.log` nennt als Adapter `Apple M1 Pro` mit Backend `Metal`. Es enthält
keinen Geräteverlust und keinen wgpu-Validierungsfehler. Die Warnung beim Ende
betrifft ein bereits zerstörtes, winit unbekanntes Fensterereignis nach dem
geordneten App-Exit und nicht die Capture-Kette.

## Befund und Grenze

Der Occlusionfehler ist in Bevy `v0.20.0-rc.1` reproduziert. Vollständige native
Verdeckung führt im gemessenen Frame zu `CurrentSurfaceTexture::Occluded`.
Bevy bereitet dennoch Screenshot-Textur und Readback-Buffer vor, überspringt die
Kopie wegen des fehlenden Swapchain-Views, reicht die Queue ein und mappt den
unbeschriebenen Buffer erfolgreich als Nullbytes. Teilverdeckung behält einen
verwendbaren Surface-View. Die Wiederherstellung ist ohne Szenenänderung
bytegleich.

Der Lauf beweist diesen Pfad auf macOS 26.6.2, Apple M1 Pro und Metal. Er belegt
keine Betriebssystemunabhängigkeit, keine Aussage zu anderen Backends und keine
allgemeine Ursache für jedes schwarze Screenshotbild.

Die angeforderte Datei
`docs/api/diagnostics/bevy-upstream-screenshot-status.md` war in diesem
Worktree nicht materialisiert. Der Versuch las und verwendete
`capture-causality-live.md`, `capture-causality-criteria.md` sowie die
vorhandenen `capture_causality_*`-Werkzeuge. Es wurde kein fremder Checkout
geöffnet.

## Befehle und Prüfungen

Headless-Prüfungen vor der GUI-Freigabe:

```sh
python3 -m py_compile \
  tests/diagnostics/capture_causality_rc.py \
  tests/diagnostics/capture_causality_rc_instrument.py
xcrun swiftc -parse tests/diagnostics/capture_causality_rc_deck.swift
python3 tests/diagnostics/capture_causality_rc.py --prepare-only
```

Der einzige freigegebene grafische Lauf:

```sh
python3 tests/diagnostics/capture_causality_rc.py
```

Der Lauf bestand alle Zustands-, Geometrie-, ID-, Copy-, Submit-, Map-,
Pixel-, Dimensions- und Wiederherstellungsassertionen. Es gab keinen zweiten
grafischen Versuch und keine automatische Wiederholung eines Captures.

## Cleanup

Der Lauf beendete beide eigenen Prozesse:

| Prozess | PID | Exitcode | läuft noch |
| --- | ---: | ---: | --- |
| Bevy-Repro | 51952 | 0 | nein |
| AppKit-Deck | 52155 | 0 | nein |

Der Cleanupnachweis liegt in
`target/capture-causality-rc-run-7grh6a8p/cleanup.json`. Das Deckfenster ist
geschlossen. Build- und Evidenzdateien bleiben zur Prüfung unter `target/`.
Nach ihrer Sicherung können ausschließlich die RC-Artefakte so entfernt werden:

```sh
rm -rf target/capture-causality-rc \
  target/capture-causality-rc-run-7grh6a8p
```
