# Inspirationsvideo: Session-Host und Agent-Client

## Quelle und Stand

Quelle: ThePrimeagen, [am I becoming a real Game Dev?](https://www.youtube.com/watch?v=tYQyh1tjSFc).

Die automatisch erzeugten englischen Untertitel wurden mit dem lokal installierten `yt-dlp`
abgerufen und vollständig gelesen. Zusätzlich wurden Einzelbilder des Abschnitts
02:45–03:50 geprüft. Die Startbefehle unten sind visuell bestätigt; der Quellcode
der gezeigten Anwendung wurde nicht geprüft.

Diese Notiz unterscheidet gesprochene Erklärungen, sichtbare Befehle und noch unbekannte
Implementierungsdetails. Sie trifft keine neue Rewrite-Entscheidung.

## Im gesprochenen Inhalt belegt

- [02:08–02:46](https://www.youtube.com/watch?v=tYQyh1tjSFc&t=128s):
  Das Spiel bietet JSON-Ausgaben und JSON-Steuerung. Genannt werden ein Ready-Signal,
  Abfragen des UI-Baums und der Spielstatistik sowie Mausbewegungen und Maustasten.
- [02:49–03:32](https://www.youtube.com/watch?v=tYQyh1tjSFc&t=169s):
  Er beschreibt ein separates Projekt, den Spielstart über FIFO und einen Server darum.
  Dieser Aufbau soll mehrere Spielinstanzen ermöglichen; anschließend spricht er von zehn
  laufenden Instanzen. Wie deren Lebenszyklus implementiert ist, bleibt unbestätigt.
- [03:32–04:21](https://www.youtube.com/watch?v=tYQyh1tjSFc&t=212s):
  Er startet ausdrücklich einen Client mit einer Grok-Modellwahl und einer ID.
  Laut seiner Erklärung ist die ID nicht erforderlich. Der Ablauf startet das Spiel,
  und das Modell bedient dessen UI. Es erhält grundlegende Spielinformationen und
  eine Persona, die hohen Schaden bevorzugt.
- [05:26–05:54](https://www.youtube.com/watch?v=tYQyh1tjSFc&t=326s):
  Er baut das Spiel ausdrücklich separat und verweist auf das zu testende Binary.
  Automatisches Bauen lehnt er für diesen Ablauf ab, damit Test-Build und weitere
  Entwicklungsänderungen getrennt bleiben.
- [06:10–07:24](https://www.youtube.com/watch?v=tYQyh1tjSFc&t=370s):
  Nach einer absichtlich eingebauten Assertion wird der Stacktrace verwendet, um in
  Linear nach einem vorhandenen Issue zu suchen und gegebenenfalls ein Issue anzulegen.
  MCP erwähnt er während dieses Linear-Ablaufs bei etwa 06:46. Diese Erwähnung belegt
  nicht, dass die Spielsteuerung ebenfalls MCP verwendet.
- [07:40–08:37](https://www.youtube.com/watch?v=tYQyh1tjSFc&t=460s):
  Das Ziel sind wiederholte Spieldurchläufe und deduplizierte Fehlerberichte.
  Automatisches Beobachten von Linear mit anschließender PR-Erstellung beschreibt er
  ausdrücklich als noch nicht eingerichtet.

## Visuell bestätigte Startbefehle

Bei ungefähr [03:29](https://www.youtube.com/watch?v=tYQyh1tjSFc&t=209s) zeigt das Terminal:

```sh
bun run server.ts un2.json | tee out
```

Darauf folgt:

```text
mordoria server listening on http://127.0.0.1:3000
```

Bei ungefähr [03:34](https://www.youtube.com/watch?v=tYQyh1tjSFc&t=214s) zeigt es:

```sh
bun run index.ts http://127.0.0.1:3000 --model=grok-4.5-fast-xhigh --id=4000
```

Der Shell-Prompt nennt das Verzeichnis `phaser`. Das allein identifiziert weder ein
öffentliches Repository noch eine verwendete Game-Engine.

Damit sind zwei getrennte Einstiege sichtbar: ein unter Bun ausgeführtes
TypeScript-Serverprogramm und ein unter Bun ausgeführtes Clientprogramm. Der Client
erhält die lokale HTTP-Adresse und die Modellwahl. Die genaue Bedeutung von `--id`
und der Inhalt von `un2.json` sind aus diesen Bildern nicht ersichtlich.

Vor dem Neustart des Servers zeigen dessen Logs unter anderem `GET /mode` und
`GET /end` mit einem `id`-Parameter. HTTP-Zugriffe sind damit sichtbar; daraus folgt
noch kein vollständiger Endpunktvertrag und keine belegte Shutdown-Semantik.

Der Startbefehl ist kein Aufruf einer erkennbaren allgemeinen Grok-CLI. Er startet
das projektspezifische `index.ts`. Zusammen mit der gesprochenen Erklärung spricht
das für einen Agent-Client mit Modellanbindung. Welches SDK oder welche Agent-Laufzeit
dahinterliegt, lässt sich ohne Quellcode nicht bestimmen.

## Noch zu verifizieren

- Der Inhalt der Serverkonfiguration und ein zugehöriges öffentliches Repository.
- Die genaue Bedeutung von `--id` und der Ablauf zum Erzeugen einer Spielinstanz.
- Ob Client-Ende die zugeordnete Spielinstanz beendet oder sie weiterlaufen lässt.
- Wie der Client Modellaufrufe, Spiel-Commands und Ergebnisse verbindet.
- Der vollständige HTTP-Vertrag und die tatsächlichen MCP-Einsatzstellen.

## Lokale Werkzeuge

`yt-dlp` kann die Untertitel dieses Videos erfolgreich abrufen.
Zusätzlich ist `pi-web-access` als globale Pi-Erweiterung installiert; deren Dokumentation
beschreibt YouTube-Analyse und Einzelbildextraktion. Die Erweiterung wurde für diese
Untersuchung bisher nicht ausgeführt.

Die zunächst defekte Homebrew-Installation von `ffmpeg` wurde mit Zustimmung des Nutzers
einschließlich benötigter Abhängigkeiten erneuert. `ffmpeg -version` liefert jetzt
Version 9.0.1. Der Download des Videoabschnitts mit `yt-dlp` und die anschließende
Einzelbildextraktion mit `ffmpeg` waren erfolgreich.
