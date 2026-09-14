# REPL verwendet JSON für komplexe Inspect-Queries

## Status

Angenommen.

## Kontext

`command::inspect::query` besitzt eine vollständige gemeinsame JSON-Form für Entity- und
Resource-Queries. Agent und Script verwenden diese Form bereits. Die REPL benötigt zusätzlich eine
menschliche Textsyntax, ohne Filter, Selektoren und Projektionen als zweite fachliche
Query-Darstellung nachzubauen.

Eine vollständige REPL-Sprache aus Unterbefehlen und Flags wäre für einfache Eingaben bequem, müsste
aber jede heutige und künftige Inspect-Variante neben dem gemeinsamen Command-Codec erneut
modellieren. Ausschließlich rohe JSON-Argumente wären eindeutig, aber für häufige Übersichtsabfragen
unnötig umständlich.

## Entscheidung

1. Die vollständige REPL-Form für Inspect lautet `inspect query <arguments-json>`.
   `<arguments-json>` ist genau das `arguments`-Objekt von `inspect.query`; es enthält weder den
   qualifizierten Command-Namen noch eine Request-ID.
2. Der JSON-Text umfasst den gesamten Rest der Eingabezeile und muss ein Objekt sein. Die REPL
   dekodiert ihn mit demselben Inspect-Arguments-Codec wie Agent, Script und Wire-Protokoll.
   Pflichtfelder, `null`, leere Listen, Enum-Kinds und die Ablehnung unbekannter Felder folgen deshalb
   ausschließlich dem gemeinsamen Vertrag.
3. Die REPL bietet genau vier Inspect-Kurzformen:
   - `inspect entities` fragt die Summary aller Entities ohne Komponentenfilter ab.
   - `inspect entity <index>:<generation>` fragt die Summary dieses Entity-Handles ab.
   - `inspect resources` fragt die Metadaten aller Resources ab.
   - `inspect resource <type-path>` fragt den Wert der bezeichneten Resource ab.
4. Jede Kurzform erzeugt direkt denselben `command::inspect::query::Command` wie die entsprechende
   JSON-Form. Die REPL definiert dafür keinen eigenen Query-Typ und keinen allgemeinen Flag-Parser.
   Alle weiteren Filter, Selektionen und Projektionen verwenden `inspect query`.
5. Fehlerhafte JSON-Syntax, ein anderer JSON-Wert als ein Objekt, strukturell ungültige Argumente und
   fehlerhafte Kurzform-Operanden sind lokale REPL-Parsefehler. Sie werden nicht an die Session
   gesendet und erhalten keine Request-ID.
6. Fachliche Fehler eines erfolgreich dekodierten Commands, etwa ein unbekannter Type Path oder ein
   nicht mehr vorhandenes Entity, durchlaufen die normale Session-Annahme. Sie erhalten eine
   Request-ID und enden mit einer Command-Ablehnung.
7. `help inspect` zeigt die vier Kurzformen und kopierbare JSON-Beispiele für alle Entity- und
   Resource-Projektionen.

## Folgen

- Komplexe REPL-Eingaben können den vollständigen Inspect-Vertrag ausdrücken, ohne ihn ein zweites
  Mal zu modellieren.
- Änderungen der gemeinsamen JSON-Form müssen nicht zusätzlich in einer vollständigen REPL-DSL
  nachgezogen werden.
- Häufige Übersichts- und Einzelabfragen bleiben kurz.
- Nutzer benötigen für kombinierte Filter, Komponentenauswahl und Hierarchieabfragen JSON.
- Ein lokaler Syntaxfehler ist eindeutig von einer durch die Session korrelierten fachlichen
  Ablehnung unterscheidbar.

## Prüfung

- Parser-Fixtures bilden jede Kurzform auf exakt denselben `command::Command`-Wert wie ihre
  ausgeschriebene JSON-Form ab.
- Inspect-Fixtures reichen jede Projektion mindestens einmal über `inspect query` ein.
- Ungültiges JSON, Nicht-Objekte, fehlende oder unbekannte Felder und ungültige Handles erzeugen
  lokale Parsefehler ohne Request-ID.
- Unbekannte Type Paths und nicht vorhandene, syntaktisch gültige Handles erzeugen nach der Annahme
  eine korrelierte Command-Ablehnung.
- Ein Help-Snapshot enthält die Kurzformen und die vollständigen JSON-Beispiele.
