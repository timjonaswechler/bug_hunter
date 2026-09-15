# Zielplanung

- [target.md](target.md) beschreibt das gewünschte Verhalten und seine Modulverantwortlichkeiten.
- [goal.rs](goal.rs) zeigt die dazugehörige Rust-ähnliche Interface-Skizze.
- [implementation-plan.md](implementation-plan.md) legt Baufolge, Wiederverwendung und Abnahme fest.

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
