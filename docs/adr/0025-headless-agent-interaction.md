---
status: accepted
---

# Agenten steuern das Spiel ohne natives Fenster

Das Ziel ist eine Session, in der ein Agent Bilder anfordert und mit virtueller
Tastatur und Maus durch Spielwelt und UI navigiert. Native Fenster,
Betriebssystemfokus und Bildschirmpräsentation dürfen dafür keine Voraussetzung
sein. Das Bildziel ist deshalb unabhängig von einer Fensteroberfläche;
Simulation bleibt ausschließlich an explizite Warps gebunden.

Die untersuchte Alternative war, den bestehenden Fenster-Screenshotpfad gegen
Verdeckung abzusichern und zu reparieren. Der Guard verhindert dort falsche
Erfolge, schafft aber keinen Betrieb ohne Fenster oder Displayserver. Diese
Reparatur wird nicht zur Voraussetzung für das Headless-Ziel gemacht.

Offscreen-Rendering beseitigt die Fensterabhängigkeit, nicht automatisch
asynchrone Asset- oder Pipelinebereitschaft. Der konkrete Render-/Readback-Adapter,
sein Bereitschaftsnachweis und die Protokollmigration werden erst nach einem
kleinen Durchstich festgelegt. Ein Bevy-Fork ist nicht beschlossen.

Die bestehenden Fensteradapter bleiben bis zu ihrem nachgewiesenen Ersatz
geschützt. Eine optionale Fenstervorschau und unveränderte Unterstützung beliebiger
Fensteranwendungen waren in dieser ursprünglichen Entscheidung noch keine
zugesagten Funktionen.

## Spätere Präzisierung

Die nachfolgende CPU-Untersuchung zeigte für Bevy 0.19.1 eine öffentliche
zentrale Anschlussstelle, die Windowidentität und unveränderte Spielkameras
bewahren kann. Das aktuelle Ziel verlangt deshalb ein nahezu unverändertes Spiel
ohne Markerpflicht, Parallelkamera, manuelle 2D-/3D-Auswahl oder eigene
Pickinglogik. Das ändert nicht die hier entschiedene Fensterlosigkeit und
explizite Tickkontrolle; es grenzt die damals offene Integration enger ein.

Der gültige Vertrag steht in [target.md](../api/target.md#headless-betrieb), der
IST-/Migrationsstand in
[headless-integration.md](../api/headless-integration.md). Historische enge
Imageziel-Demos belegen nur ihre jeweilige Fixture und sind keine konkurrierende
Zielarchitektur.
