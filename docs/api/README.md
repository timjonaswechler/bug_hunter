# Zielplanung

- [target.md](target.md) beschreibt das gewünschte Verhalten und seine Modulverantwortlichkeiten.
- [goal.rs](goal.rs) zeigt die dazugehörige Rust-ähnliche Interface-Skizze.
- [headless-integration.md](headless-integration.md) ist der maßgebliche IST/ZIEL-Stand
  und dateigenaue Migrationsplan für den Headless-Umbau.
- [next-steps.md](next-steps.md) nennt nur den unmittelbar nächsten Schritt und die
  Bedingungen für GPU-Parität und späteren Rückbau.
- [implementation-plan.md](implementation-plan.md) ordnet Headless in die übrige Baufolge ein.
- [slice.md](slice.md) beschreibt den ausführbaren experimentellen Fensterdurchstich und seine Tests.

Eine Regel hat eine maßgebliche Stelle. Interface-Kommentare erklären nur, was zum Verständnis
der Signatur nötig ist. Die Skizze muss nicht kompilieren.

Aktuelles Produktziel ist ein
[transparenter Headless-Betrieb](target.md#headless-betrieb): nahezu unveränderte
Windowdaten, Spielkameras, Projektionen, Viewports, Stacks und Bevy-Picking bei
zentraler Ausgabe-, Eingabe- und Ticksteuerung. Das ist noch nicht der Stand des
ausführbaren Durchstichs. [ADR-0025](../adr/0025-headless-agent-interaction.md)
dokumentiert die Herkunft der früheren engen Integrationsentscheidung; die
aktuelle Präzisierung und ihre Evidenz stehen in
[headless-integration.md](headless-integration.md). Die abgeschlossene
Fensterdiagnose ist auf
[relevante Rendering-Befunde](diagnostics/rendering-findings.md) reduziert.

Der vorhandene Code ist technische Referenz, keine automatische Zielanforderung.
[Research](research/) liefert technische Nachweise;
[ADRs](../adr/README.md) bewahren Entscheidungsherkunft. Beide werden bei Bedarf gelesen,
nicht als parallel zu pflegende Zielbeschreibung verwendet.

Neue Produktentscheidungen benötigen ausdrückliche Zustimmung. Aus bestehenden Regeln
ableitbare Schritte und reversible Implementierungsdetails werden ohne erneute
Bestätigungsschleife bearbeitet. Bestehende Nutzeränderungen bleiben erhalten.
Subagenten werden nur nach ausdrücklicher Zustimmung eingesetzt.
