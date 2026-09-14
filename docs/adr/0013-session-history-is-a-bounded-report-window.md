# Session-History ist ein begrenztes Report-Fenster

## Status

Angenommen.

## Kontext

Reports benötigen die zuletzt angenommenen Commands samt korrelierten Outcomes. Eine vollständige
dauerhafte Aufzeichnung ist dagegen Aufgabe des ausdrücklich gestarteten Recordings. Würde die
Session für Reports alle Commands einer beliebig langen Sitzung im Speicher halten, wäre ihr
Speicherverbrauch ohne fachlichen Nutzen unbegrenzt.

Der Koordinator muss laufende Commands unabhängig von der Report-History bis zu ihrem terminalen
Ergebnis verfolgen. Ein `Pending` kann fallengelassen werden, und ein alter laufender Command kann
aus dem Report-Fenster fallen, ohne dass seine Response, sein Recording-Eintrag oder seine
Shutdown-Relevanz verloren gehen darf.

Das bisherige History-Outcome kann bekannte commandbezogene I/O-Fehler nicht darstellen.
`Unanswered` wäre dafür falsch, weil ein terminales Ergebnis vorliegt.

## Entscheidung

1. Die Session besitzt einen Ringpuffer mit den letzten 50 angenommenen Commands in
   Annahmereihenfolge. Diese Grenze ist zugleich die bereits festgelegte Höchstgrenze für
   `report::Context::commands`.
2. Die aktive Command-Tabelle und die Report-History bleiben getrennte interne Strukturen. Die
   aktive Tabelle hält jeden nicht terminalen Command bis zur Korrelation, auch wenn sein
   History-Eintrag bereits aus dem Ring gefallen ist.
3. Jeder angenommene `command::Command` erhält sofort einen History-Eintrag mit
   `Outcome::Unanswered`. Das gilt für Wire-Commands, Recording Start und Stop, Replay Start und
   Stop, intern vom Replay ausgeführte Plan-Commands und den angenommenen Shutdown-Command.
4. Ein vor der Annahme fehlgeschlagener `send`-Aufruf, Session-Events, Reports und interne
   Koordinatornachrichten erhalten keinen History-Eintrag.
5. Ein terminales Ergebnis ersetzt `Unanswered` an der vorhandenen Position. Die Reihenfolge
   eintreffender Responses sortiert die History nicht um.
6. `history::Outcome` unterscheidet `Completed`, `Rejected`, `ProtocolFailed`, `IoFailed` und
   `Unanswered`. `IoFailed` enthält die menschenlesbare Meldung eines bekannten commandbezogenen
   I/O-Fehlers.
7. `Unanswered` bedeutet, dass beim Snapshot kein terminales Command-Ergebnis vorliegt. Das umfasst
   einen noch laufenden Command und einen Command, dessen Ergebnis durch das Session-Ende
   ausbleibt. Die History unterscheidet diese beiden Fälle nicht.
8. Endet die Session, markiert der Koordinator alle noch im Ring vorhandenen nicht terminalen
   Einträge als `Unanswered`. Commands außerhalb des Rings bleiben für den Report ausgeschlossen.
9. `Report::create` kopiert den aktuellen Ring beim Aufruf. Der Snapshot entsteht nicht bereits beim
   Schreiben eines Panic- oder Tracing-Markers. Der gemeinsame Host-Ablauf ruft `Report::create`
   unmittelbar nach dem Empfang von `Event::Failure` und vor `report::submit` auf.
10. `Session` erhält keinen öffentlichen History-Accessor. `history::Entry` und
    `history::Outcome` bleiben öffentlich, weil `report::Context::commands` sie freigibt.
11. Recording und History erhalten terminale Ergebnisse am selben gemeinsamen
    Command-Ausführungspunkt, besitzen aber keinen gemeinsamen Speicher. Recording bleibt die
    vollständige persistierte Aufzeichnung des ausdrücklich gewählten Abschnitts.

## Folgen

- Der Speicher für den Report-Verlauf bleibt auf 50 Commands begrenzt.
- Eine beliebige Zahl gleichzeitig laufender Commands benötigt weiterhin je einen Eintrag in der
  aktiven Command-Tabelle. Diese notwendige Korrelation wird nicht als Report-History ausgegeben.
- Ein langsamer älterer Command kann aus dem Report-Fenster fallen und trotzdem korrekt beendet,
  empfangen und aufgezeichnet werden.
- Ein Report kann laufende Commands als `Unanswered` zeigen. Spätere Outcomes verändern den bereits
  erzeugten Report nicht.
- Ein commandbezogener Datei- oder Flush-Fehler wird nicht als fehlende Antwort ausgegeben.
- Wer eine vollständige Sitzung benötigt, muss Recording verwenden.

## Prüfung

- History-Fixtures prüfen null, genau 50 und mehr als 50 angenommene Commands.
- Vertauscht eintreffende Responses verändern die Annahmereihenfolge nicht.
- Ein aus dem Ring gefallener laufender Command bleibt empfangbar und schließt ein aktives Recording
  ab.
- Ein fallengelassenes `Pending` führt weiterhin zur terminalen Ergänzung seines History-Eintrags.
- Fixtures prüfen `Completed`, `Rejected`, `ProtocolFailed`, `IoFailed` und `Unanswered`.
- Ein vor der Annahme fehlgeschlagener Command, ein Session-Event und ein Report-Aufruf erscheinen
  nicht in der History.
- Ein Report-Snapshot bleibt unverändert, wenn Commands danach enden oder weitere Commands
  angenommen werden.
- Ein Compile-Fail- oder API-Test belegt, dass `Session` keinen öffentlichen History-Accessor
  besitzt.
