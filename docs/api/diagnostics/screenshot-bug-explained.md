# Warum Bevy bei einem verdeckten Fenster schwarze Screenshots liefern kann

Diese Erklärung bezieht sich auf den hier untersuchten Bevy-0.19.1-Code.
Der [kontrollierte Live-Versuch](capture-causality-live.md#beweiskräftiger-lauf)
belegt den Fehlerpfad. Er beweist nicht rückwirkend die Ursache jedes früheren
schwarzen Bildes.

## Der Fehler in einem Satz

Bevy überspringt bei fehlender Fensteroberfläche die Kopie für den Screenshot,
liest den vorbereiteten, unbeschriebenen Buffer danach aber trotzdem aus.
Dieser liefert Nullen und damit ein schwarzes Bild.

## Zwei Aufgaben, die getrennt funktionieren müssten

Für eine Aufnahme braucht Bevy diese Schritte:

```text
Szene in eine GPU-Textur rendern
    ↓
Bild aus dieser Textur in einen auslesbaren Buffer kopieren
    ↓
Buffer auslesen und daraus ein Image erstellen
```

Zusätzlich soll das Bild im Fenster erscheinen. Dafür stellt das Betriebssystem
eine Fenstertextur bereit, die Swapchain-Textur. Ein View beschreibt den Zugriff
auf diese Textur.

Ist das Fenster vollständig verdeckt, liefert Metal im gemessenen Fall
`Occluded` statt einer Fenstertextur. Das ist zunächst kein Screenshotfehler.
Die Aufnahme besitzt eine eigene Textur und müsste nicht auf dem Bildschirm
angezeigt werden, um ausgelesen werden zu können.

## Den Fehler im Bevy-Code verfolgen

Die folgenden Funktionen stehen in
`bevy_render-0.19.1/src/view/window/screenshot.rs`. Exakte Quellenstellen und
die ergänzenden wgpu-Nachweise enthält die
[Quellenübersicht](capture-causality-criteria.md#was-die-quellen-tatsächlich-sichern).

### 1. `prepare_screenshots` bereitet die Aufnahme vor

Bevy erzeugt eine eigene Screenshot-Textur und einen Buffer zum späteren
Auslesen. Es trägt die Textur in `ViewTargetAttachments` ein, damit die
Kameraausgabe dort landet. Der Buffer enthält zu diesem Zeitpunkt noch keine
kopierten Bilddaten.

### 2. `submit_screenshot_commands` überspringt zu viel

Im Zweig `NormalizedRenderTarget::Window` werden Format und View geprüft.
Der entscheidende Ausschnitt, um den Aufruf verkürzt:

```rust
let Some(texture_format) = window.swap_chain_texture_view_format else {
    continue;
};
let Some(swap_chain_texture_view) = window.swap_chain_texture_view.as_ref() else {
    continue;
};
// Erst danach wird render_screenshot(...) aufgerufen.
```

Im kontrollierten Verdeckungsfall greift das zweite `continue`. Das Format kann
noch aus einem früheren sichtbaren Frame vorhanden sein; der aktuelle View fehlt.
`render_screenshot` wird deshalb nicht aufgerufen.

### 3. `render_screenshot` verbindet Kopie und Anzeige

Diese Funktion zeichnet zuerst `encoder.copy_texture_to_buffer(...)` auf.
Danach folgt der optionale Renderpass zur Anzeige im Fenster.

Die vorangestellte View-Prüfung verhindert also beide Aufgaben. Die Kopie von
der Screenshot-Textur in den Buffer braucht den Fenster-View jedoch nicht.
Das ist die fehlerhafte Kopplung.

### 4. `collect_screenshots` liest trotzdem aus

Diese Funktion mappt alle vorbereiteten Screenshot-Buffer mit `map_async`.
Sie prüft nicht, ob für den jeweiligen Buffer eine Bildkopie aufgezeichnet wurde.

Ein unbeschriebener Mappingbereich liefert nullinitialisierte Bytes.
RGB-Werte von null bedeuten Schwarz. Das Mapping kann erfolgreich sein,
obwohl kein gerendertes Bild in den Buffer kopiert wurde.
Bevy liefert daraus ein `Image`; der bisherige woodpecker-Dateipfad konnte
daraus ein formal gültiges schwarzes PNG erzeugen.

## Was der Versuch gezeigt hat

| Zustand im selben Prozess | Bildkopie | Ergebnis |
| --- | --- | --- |
| Sichtbar, ohne Fokus | ausgeführt | korrektes Bild |
| Vollständig verdeckt | wegen fehlendem View übersprungen | derselbe vorbereitete Buffer liefert ausschließlich Nullbytes |
| Wieder sichtbar, ohne Fokus | ausgeführt | Rohdaten und PNG bytegleich zum ersten Capture |

Zwischen den Aufnahmen lief kein weiterer Simulationstick. Szene, Kamera,
Transform, Größen und Viewports blieben unverändert. Die Logs ordnen Vorbereitung,
Copy beziehungsweise Skip, Submit und Mapping über Capture-/Buffer-IDs zu.
Messwerte und Grenzen stehen im [Versuchsbericht](capture-causality-live.md).

Fehlender Fokus allein ist damit nicht die Ursache dieses Wechsels. Auch eine
geänderte Kameragröße oder noch nicht renderbereite Szene erklärt diesen
kontrollierten Ablauf nicht.

## Was bereits geschützt ist und was noch nicht

Der woodpecker-Guard erkennt den fehlenden View im Capture-Frame und lehnt die
Aufnahme mit `screenshot_window_unavailable` ab. Im verdeckten Fall des Versuchs
entstand deshalb keine PNG-Datei. Die Diagnose maß den rohen Bevy-Readback
separat vor dieser Absicherung.

Der Guard verhindert einen falschen Erfolg. Er ermöglicht noch keine erfolgreiche
Aufnahme verdeckter Fenster.

Vor einer Änderung an Bevy oder einem eigenen Capture-Adapter werden die
tatsächlich veröffentlichten Bevy-0.20-Prereleases, der Entwicklungszweig sowie
offene und geschlossene Upstream-Issues und PRs geprüft. Ein ähnliches schwarzes
Bild in einem Issue ist noch kein Beleg für denselben Mechanismus.
