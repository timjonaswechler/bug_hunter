# Dokumentation

- [target.md](target.md) beschreibt das gewünschte Verhalten und seine Modulverantwortlichkeiten.
- [goal.rs](goal.rs) zeigt die dazugehörige Rust-ähnliche Interface-Skizze.
- [implementation-plan.md](implementation-plan.md) besitzt den Abschlussplan,
  die Abdeckungsmatrix und noch offene Entscheidungen.
- [usage.md](usage.md) beschreibt Bedienung, Builds und ausführbare Tests.
- [next-steps.md](next-steps.md) ist der Einstieg für eine neue Session: Arbeitsstand,
  unmittelbarer nächster Schritt, letzter Prüfstand und Arbeitsregeln.
- Bei Screenshotproblemen: [Fehler und Guard](diagnostics/black-screenshots.md),
  [Nachweise und Reproduktion](diagnostics/screenshot-evidence.md),
  [datierter Upstream-Stand](diagnostics/bevy-upstream-screenshot-status.md).
  [Resize und Kameraaktualisierung](diagnostics/window-schedules.md) sind ein getrennter Befund.

Eine Regel hat eine maßgebliche Stelle. Interface-Kommentare erklären nur, was zum Verständnis
der Signatur nötig ist. Die Skizze muss nicht kompilieren.

Der vorhandene Code ist technische Referenz, keine automatische Zielanforderung.
[Research](research/) liefert technische Nachweise;
[ADRs](../adr/README.md) bewahren Entscheidungsherkunft. Beide werden bei Bedarf gelesen,
nicht als parallel zu pflegende Zielbeschreibung verwendet.

Neue Produktentscheidungen benötigen ausdrückliche Zustimmung. Aus bestehenden Regeln
ableitbare Schritte und reversible Implementierungsdetails werden ohne erneute
Bestätigungsschleife bearbeitet. Bestehende Nutzeränderungen bleiben erhalten.
Subagenten werden nur nach ausdrücklicher Zustimmung eingesetzt.
