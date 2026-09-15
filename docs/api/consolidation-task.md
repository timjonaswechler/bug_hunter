# Aufgabe: Zielplanung konsolidieren und Umsetzung vorbereiten

## Verbindliches Ziel

Das Ziel von `bug_hunter` ist, ein Spiel durch unser System kontrollieren, beobachten und
nach Fehlern untersuchen zu können. Recording, Replay und Reports unterstützen reproduzierbare
Untersuchungen. Menschen und Agenten sollen diesen Ablauf nutzen können.

Dieses Ziel steht fest. Die Aufgabe besteht darin, die vorhandenen Entscheidungen zu seiner
Umsetzung widerspruchsfrei zusammenzuführen und den Einstieg in die Neuimplementierung in Rust
vorzubereiten. Der lokale Multi-Session-Server und der zunächst über die CLI nutzbare Client-Zugang
sind Mittel dafür. Die Konsolidierung eröffnet keine neue Zielfindung.

Das Ergebnis soll weniger Dinge enthalten, die gleichzeitig gepflegt und im Blick behalten
werden müssen. Es soll die Umsetzung ermöglichen, nicht eine weitere umfangreiche Planungsrunde.

## Arbeitsgrenze

Diese Aufgabe konsolidiert die Planung und bereitet die Umsetzung vor. Sie beginnt noch nicht
mit der Neuimplementierung. Subagenten werden nur nach ausdrücklicher Zustimmung eingesetzt.
Gleichzeitig vorgenommene Nutzeränderungen werden gelesen und erhalten.

Die hier ausdrücklich vereinbarte Dokumentkonsolidierung ersetzt für diese Aufgabe die bisherige
Pflege derselben Entscheidungen in mehreren Planungsdateien. Bestehende Dokumente sind zunächst
Quellen zur Prüfung, nicht automatisch die gewünschte künftige Dokumentstruktur.

## Leitlinien für den aktiven Kontext

- Beschreibe ausschließlich aktuell gültige Entscheidungen.
- Entferne zurückgenommene Entscheidungen und Erläuterungen ihrer Ablösung aus der aktiven
  Zielbeschreibung.
- Entferne vollständig gestrichene Konzepte einschließlich ihrer Negativnennungen.
- Formuliere gewünschtes Verhalten direkt, ohne Vergleiche mit früheren Entwürfen.
- Behalte notwendige Grenzen gültiger Verträge bei. Eine feste Session-Bindung ist beispielsweise
  eine relevante Verhaltensregel, keine historische Negativnennung.
- Nutze historische Dokumente und Git bei Bedarf zur Klärung. Übernimm ihre Inhalte nicht
  ungeprüft in das aktuelle Zielbild.
- Erfinde beim Konsolidieren keine neuen Anforderungen.
- Ausdrückliche Nutzerentscheidungen haben Vorrang vor älteren Entwürfen und
  Implementierungsannahmen. Die bestehende Implementierung ist technische Referenz, keine
  automatische Zielanforderung.

## 1. Entscheidungen auf Widersprüche und Dopplungen prüfen

Lies `goal.rs`, `migration.md`, `open-questions.md`, die relevanten ADRs und bei Bedarf die
zugrunde liegenden Nutzerentscheidungen gemeinsam. Nutze `current.md` und den vorhandenen Code
gezielt zur Klärung technischer Zusammenhänge.

- Identifiziere doppelte, überholte und widersprüchliche Aussagen.
- Unterscheide echte Entscheidungskonflikte von veralteten Textstellen.
- Führe übereinstimmende Aussagen zusammen.
- Ordne jede fachliche Regel genau einem verantwortlichen Modul zu.
- Lege für jede Entscheidung eine maßgebliche Dokumentationsstelle fest.
- Prüfe besonders die Übergänge zwischen Session, Server, Client, CLI und Report-Verarbeitung.
- Prüfe nach der Überarbeitung den Gesamtstand erneut. Ein sauberer Diff allein belegt keine
  widerspruchsfreie Architektur.

Der feste Projektzweck ist dabei der Bezugspunkt. Zu konsolidieren sind die bereits vereinbarten
Mittel, Verantwortlichkeiten und Verhaltensregeln, nicht der Zweck selbst.

## 2. Offene Punkte einordnen und reduzieren

Ordne jeden bisher offenen Punkt einer Kategorie zu:

| Kategorie | Vorgehen |
|---|---|
| Bereits entschieden oder eindeutig aus dem Ziel ableitbar | Im Zielstand festhalten und aus der Fragenliste entfernen |
| Reversibles Implementierungsdetail | Während der Umsetzung anhand bestehender Muster entscheiden |
| Für den ersten Durchstich nicht erforderlich | Aus dessen Arbeitsumfang herausnehmen; weiterhin gewünschte spätere Arbeit getrennt und knapp führen |
| Echte unentschiedene Produkt- oder Architekturfrage | Mit konkreten Auswirkungen und einer Empfehlung vorlegen |

Frage nicht erneut nach bestätigten Regeln. Erzeuge keine neuen Fragen allein deshalb, weil
weitere Details denkbar sind. Fehlende technische Recherche ist zunächst Recherchearbeit,
keine Produktfrage an den Nutzer.

## 3. Ein gemeinsames Zieldokument erstellen

Extrahiere aus den gültigen Inhalten von `migration.md`, `open-questions.md` und den relevanten
Entscheidungen ein neues, gemeinsames Zieldokument. Kopiere die bisherigen Dateien nicht einfach
zusammen. Das neue Dokument beschreibt ausschließlich das gewünschte System.

- Beschlossene Regeln haben dort eine maßgebliche Stelle.
- `goal.rs` zeigt die passende Interface-Skizze und Modulverantwortlichkeiten.
- Kommentare in `goal.rs` erklären nur das zum Verständnis der Schnittstelle nötige Verhalten.
  Sie wiederholen nicht vollständig das Zieldokument.
- Tatsächlich offene Entscheidungen stehen getrennt von beschlossenen Regeln an einer Stelle.
- Der konkrete Umsetzungsplan steht ebenfalls an einer Stelle.
- Die bisherigen Planungsdateien werden nicht parallel als weitere Quellen derselben Regeln
  weitergeführt.

Prüfe den eigenständigen Nutzen aller betroffenen Dokumente:

- Übernimm aus `current.md` nur Informationen, die für den Rewrite tatsächlich benötigt werden.
  Eine dauerhaft gepflegte Gesamtbeschreibung des Altbestands ist kein Selbstzweck.
- Behalte `README.md` nur, wenn eigenständiger, beständiger Inhalt übrig bleibt. Sie erhält weder
  eine Fortschrittsübersicht noch einen laufend zu aktualisierenden nächsten Arbeitsschritt.
- Prüfe, welche ADRs für das aktuelle Ziel maßgeblich sind. Historische ADRs werden nicht
  nebenbei zu konkurrierenden aktuellen Spezifikationen.
- Entferne abgelöste Dateien nach Übernahme ihrer weiterhin benötigten Inhalte und passe
  Referenzen an. Git bewahrt die Historie.
- Erzeuge keine zusätzlichen Dokumente oder Übersichten, die andere Planungsstände nacherzählen.

## 4. Einen ersten funktionsfähigen Durchstich festlegen

Definiere einen begrenzten, tatsächlich ausführbaren Ablauf:

1. Server starten.
2. Session erstellen.
3. Client an die Session binden.
4. Einen repräsentativen Spiel-Command ausführen.
5. Dessen Ergebnis beobachten.
6. Geordnet beenden.

Lege fest:

- welche Module und Schnittstellen dafür notwendig sind,
- welche vorhandenen Komponenten wiederverwendet werden können,
- welche Tests und welcher reale Ablauf den Erfolg nachweisen,
- welche weiteren gewünschten Funktionen darauf aufbauen.

Der Durchstich ist ein Implementierungsschritt innerhalb des Gesamtziels. Er ersetzt dieses
nicht durch ein dauerhaft reduziertes Produkt. Lege keine Infrastruktur nur für möglicherweise
später benötigte Funktionen an.

## 5. Nur echte Konflikte zum Nutzer zurückbringen

Bearbeite alle aus dem Kontext lösbaren Punkte selbstständig.

Lege ausschließlich Konflikte vor, bei denen plausible Varianten zu unterschiedlichem
Nutzerverhalten oder erheblich anderer Architektur führen. Beschreibe jeweils:

- die konkrete Entscheidung,
- die Auswirkungen der Varianten,
- eine begründete Empfehlung.

Bündele zusammengehörige Entscheidungen. Vermeide einen neuen Zyklus aus vielen einzelnen
Bestätigungsfragen. Bleibt eine echte Entscheidung offen, markiere sie als solche, statt eine
Vermutung als beschlossene Regel einzutragen.

## Abschlusskriterien

Am Ende liegen vor:

- ein kompaktes, verbindliches Zieldokument,
- eine dazu passende Zielskizze,
- eine bereinigte Dokumentstruktur mit klaren Zuständigkeiten und gültigen Referenzen,
- eine kurze Liste echter Restentscheidungen,
- ein konkreter Plan für den ersten funktionsfähigen Durchstich.

Prüfe den konsolidierten Stand auf widersprüchliche Regeln, doppelte Zuständigkeiten,
veraltete Referenzen und unbeabsichtigt übernommene Altanforderungen. Prüfe den Durchstich
gegen das Gesamtziel und benenne tatsächliche Hindernisse für seine Implementierung.

Der Abschlussbericht nennt den erreichten Stand, verbleibende Hindernisse und den nächsten
Implementierungsschritt. Er erzählt nicht erneut die verworfenen Entwürfe.

Dieses Dokument ist ein einmaliger Arbeitsauftrag, keine zusätzliche dauerhaft zu pflegende
Statusübersicht oder Quelle fachlicher Regeln.
