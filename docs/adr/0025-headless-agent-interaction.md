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
Fensteranwendungen sind keine mit dieser Entscheidung zugesagten Funktionen.
Der gültige Vertrag steht in [target.md](../api/target.md#headless-betrieb).
