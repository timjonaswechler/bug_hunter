# Der konfigurierte Provider wird direkt verwendet und fällt lokal zurück

Historischer Entscheidungsstand. Für den Rewrite gilt [target.md](../api/target.md).

## Status

Angenommen.

## Kontext

Reports enthalten unveränderte Anwendungsargumente, Commands und Outcomes. Die Provider-Auswahl
muss deshalb eindeutig bestimmen, ob ein Report lokal bleibt oder an einen Remote-Provider gesendet
wird.

Ein vorgeschalteter lokaler Entwurf würde auch bei einer erfolgreichen Remote-Veröffentlichung eine
zweite Report-Datei erzeugen. Umgekehrt darf ein fehlendes Netzwerk, Werkzeug oder Login den Report
nicht vollständig verlieren lassen.

GitHub CLI besitzt bereits Regeln zur Auflösung von Repository, Host und Anmeldung. Ein zweites
Konfigurationsmodell in `bug_hunter` könnte diesen Werten widersprechen.

## Entscheidung

1. `report::submit` verwendet direkt den Provider aus der beim Session-Start wirksamen
   `provider::Config`. Der Aufruf nimmt keine zweite Report- oder Provider-Konfiguration entgegen.
2. Der lokale Provider schreibt ausschließlich einen Markdown-Report.
3. Der GitHub-Provider sucht über `gh` nach einem bestehenden Issue und erstellt ohne Treffer direkt
   ein neues Issue. Bei einem erfolgreichen GitHub-Ablauf schreibt er keine lokale Markdown-Datei.
4. Jeder Fehler eines Remote-Providers löst den lokalen Markdown-Rückfall aus. Es erfolgt kein
   Wechsel zu einem anderen Remote-Provider.
5. Ein erfolgreicher Rückfall ergibt `provider::Outcome::Fallback`. Das Outcome enthält den lokalen
   Pfad und den Remote-Fehler. Der fehlgeschlagene Remote-Versuch wird nicht als lokaler Erfolg
   verborgen.
6. Schlägt auch das lokale Schreiben fehl, gibt `report::submit` einen Fehler mit Remote- und
   Persistenzursache zurück. R7 legt dessen öffentliche Form fest.
7. `github::Config` besitzt keine Felder. Der Provider führt `gh` im Projektordner der Controlled
   Session aus und übergibt kein `--repo`. Repository, Host und Anmeldung werden nativ durch `gh`
   aufgelöst.
8. Der GitHub-Body enthält die vollständige R4-Signatur in einem unsichtbaren Marker. Die Suche
   vergleicht nur diesen Marker und umfasst offene sowie geschlossene Issues.
9. Ein vorhandenes Issue wird nicht verändert, kommentiert oder erneut geöffnet. Das Outcome
   verweist mit Nummer und URL darauf.
10. Der Provider übernimmt `Report::title` ohne GitHub-spezifischen Präfix. Labels, Assignees,
    Milestones und eigene Repository-, Host- oder Authentifizierungseinstellungen gehören nicht zu
    diesem Umbau.

## Folgen

- Die Provider-Konfiguration ist zugleich die ausdrückliche Entscheidung zwischen lokaler
  Speicherung und Remote-Veröffentlichung.
- Ein GitHub-Erfolg erzeugt genau eine Remote-Referenz. Ein Remote-Fehler erzeugt nach Möglichkeit
  genau eine lokale Referenz und bleibt im Outcome sichtbar.
- Bestehende geschlossene Issues verhindern ebenso wie offene Issues ein neues Duplikat.
- Die native `gh`-Konfiguration kann über lokalen Git-Kontext und die von `gh` unterstützte Umgebung
  wirken, ohne Teil der `bug_hunter`-API zu werden.
- GitHub garantiert keine atomare Eindeutigkeit für Body-Marker. Gleichzeitige Veröffentlichungen
  können trotz vorheriger Suche Duplikate erzeugen.
- Spätere Provider-Einstellungen benötigen eine eigene API-Entscheidung und eigene Tests.

## Prüfung

- Fixtures prüfen lokale Erstellung sowie GitHub-Erstellung ohne vorherige lokale Datei.
- Prozess-Fixtures prüfen fehlendes `gh`, fehlende Anmeldung, nicht auflösbares Repository,
  Netzwerkfehler, fehlgeschlagene Suche und fehlgeschlagene Issue-Erstellung.
- Jeder Remote-Fehler prüft den lokalen Rückfall, den erhaltenen Remote-Fehler und den lokalen Pfad.
- Ein zusätzlicher Schreibfehler prüft die gemeinsame Rückgabe beider Ursachen.
- GitHub-Fixtures prüfen exakte Marker in offenen und geschlossenen Issues sowie abweichende Titel
  und veränderte übrige Body-Inhalte.
- Command-Fixtures prüfen den Projektordner, das fehlende `--repo`, stdin als Body-Quelle und das
  Fehlen eigener Labels, Assignees, Milestones und Titelpräfixe.
