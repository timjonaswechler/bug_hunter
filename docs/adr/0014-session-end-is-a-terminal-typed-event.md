# Unerwartetes Session-Ende ist ein typisiertes terminales Event

Historischer Entscheidungsstand. Für den Rewrite gilt [target.md](../api/target.md).

## Status

Angenommen.

## Kontext

Das bisherige `session::Event::Ended` erklärt nicht, warum eine Session endete. Der Host muss für
seine menschliche und maschinenlesbare Ausgabe zwischen einem Prozessende, einer geschlossenen Pipe,
einem I/O-Fehler und eigener Überlast unterscheiden können.

Der private Session-Koordinator läuft unabhängig vom Polling des Controllers. Eine unbegrenzte
Event-Queue könnte deshalb bei vielen Tracing- oder Protokollfehlern unbegrenzt wachsen. Würde der
Koordinator stattdessen an einer vollen Queue blockieren, könnte er die Prozess-Pipes nicht mehr
leeren und den Kindprozess dadurch ebenfalls blockieren.

Ein erfolgreicher `Session::shutdown` besitzt bereits ein direktes Ergebnis. Ein zusätzliches
`Ended`-Event für denselben kontrollierten Vorgang würde zwei konkurrierende Abschlussmeldungen
erzeugen.

## Entscheidung

1. `session::Event::Ended` enthält einen typisierten `session::EndReason`.
2. `EndReason::ProcessExit { status }` bezeichnet jedes Prozessende außerhalb eines bestätigten
   Shutdowns. Auch Exit-Status 0 ist in diesem Fall unerwartet.
3. `EndReason::TransportClosed { channel }` bezeichnet ein sauberes EOF eines benötigten
   Transportkanals, solange der Prozess noch laufen sollte.
4. `EndReason::TransportFailed { channel, message }` bezeichnet einen konkreten I/O-Fehler auf
   stdin, stdout oder stderr. `TransportChannel` unterscheidet diese drei Kanäle.
5. Ein stdout- oder stderr-EOF nach einem bereits beobachteten Prozessende gehört zum abschließenden
   Drain und erzeugt keinen eigenen Transport-Endgrund.
6. Schließt ein benötigter Kanal unerwartet, beendet der Koordinator die Session, beendet die
   Prozessgruppe und sammelt den Kindprozess ein. Die durch diesen Host-Abbruch verursachte
   Prozessbeendigung ersetzt den ursprünglichen Transportgrund nicht und erzeugt keinen
   `ProcessExit`-Report.
7. Bei einem unerwarteten Prozessende merkt der Koordinator zuerst den Status, leert stdout und
   stderr bis EOF, verarbeitet letzte Responses und Reporting-Marker und reiht erst danach `Ended`
   ein.
8. Bereits gespeicherte Events und beim abschließenden Drain erkannte Events bleiben in
   FIFO-Reihenfolge. `Ended` wird genau einmal eingereiht und ist immer das letzte Session-Event.
9. Bereits terminal vorliegende Command-Ergebnisse bleiben nach dem Session-Ende über ihr `Pending`
   abrufbar. Noch nicht terminale Commands bleiben `Unanswered` und liefern `session::Error::Ended`.
10. Nach Entnahme des terminalen Events liefern weitere Event-Empfangsoperationen
    `session::Error::Ended`. Andere Session-Operationen dürfen diesen Fehler bereits vor der Entnahme
    des Events liefern.
11. Ein erfolgreicher `Session::shutdown` und `Session::drop` erzeugen kein `Ended`-Event. Nach
    erfolgreichem Shutdown liefern alle weiteren Session-Operationen einschließlich eines zweiten
    Shutdowns `session::Error::Ended`.
12. Die Queue fasst 256 normale Events. Für das terminale `Ended` bleibt ein zusätzlicher Platz
    reserviert.
13. Passt ein normales Event nicht mehr in die Queue, nimmt der Koordinator keine neuen Commands an,
    beendet die Prozessgruppe und zählt alle nicht mehr speicherbaren Events. Nach den bereits
    gespeicherten Events folgt
    `EndReason::EventQueueOverflow { capacity: 256, dropped_events }`.
14. Die durch einen Event-Überlauf ausgelöste Prozessbeendigung erzeugt keinen `ProcessExit`-Report.
    Der Überlauf bleibt der primäre Endgrund und macht den Verlust mindestens eines Events
    ausdrücklich sichtbar.
15. Die Event-Kapazität ist vorerst nicht konfigurierbar.

## Folgen

- Der Host kann ein unerwartetes Ende ohne das Parsen freier Fehlermeldungen darstellen.
- `Event::Ended` beschreibt den Lebenszyklus; ein davor ausgegebenes `Event::Failure` beschreibt
  unabhängig davon den möglichen Report.
- Ein Panic mit anschließendem Prozessende erzeugt zuerst `Failure::panic` und danach ein
  `ProcessExit`-Endevent. Ohne Panic kann derselbe Prozessstatus zusätzlich
  `Failure::process_exit` auslösen.
- Lang laufende Sessions besitzen eine feste Obergrenze für nicht abgeholte Events.
- Ein Controller, der Events nicht abholt, verliert bei Überlauf die Session statt unbemerkt Speicher
  aufzubauen oder den Kindprozess an vollen Pipes festzusetzen.
- Die genaue Darstellung der Endgründe im Agent- und REPL-Modus bleibt Aufgabe des Host-Interfaces.

## Prüfung

- Prozess-Fixtures prüfen unerwartete Exit-Statuswerte 0 und ungleich 0.
- Transport-Fixtures schließen stdin, stdout und stderr vorzeitig und unterscheiden EOF von einem
  konkreten I/O-Fehler.
- Eine Panic-Fixture prüft `Failure` vor `Ended`; eine Prozessfehler-Fixture prüft
  `Failure::process_exit` vor demselben terminalen Event.
- Bereits eingetroffene Outputs bleiben nach dem Prozessende einmal abrufbar. Offene Commands liefern
  `Error::Ended` und bleiben in der History `Unanswered`.
- Erfolgreicher Shutdown und Drop erzeugen kein `Ended`-Event.
- Eine Fixture füllt 256 Events, erzeugt weitere Events und prüft Anzahl, Reihenfolge,
  `dropped_events` und den reservierten terminalen Platz.
- Nach Entnahme von `Ended` liefern beide Event-Empfangsmethoden `Error::Ended`.
