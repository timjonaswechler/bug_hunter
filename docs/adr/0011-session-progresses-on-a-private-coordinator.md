# Session-Fortschritt läuft auf einem privaten Koordinator

## Status

Angenommen.

## Kontext

`Session::send` liefert ein `Pending`, während Wire-Commands und hostseitige Commands weiterlaufen.
Insbesondere muss ein Replay nach seiner Annahme den Plan ausführen, auch wenn der Aufrufer sein
Ergebnis noch nicht abfragt. Ein kooperatives Modell, das nur in `try_receive`, `receive` oder
`receive_event` arbeitet, würde den Fortschritt vom Polling-Verhalten des Controllers abhängig
machen.

stdout und stderr müssen unabhängig vom Controller geleert werden. Andernfalls kann ein
Provider-Aufruf oder eine andere langsame Host-Operation den Kindprozess an einer vollen Pipe
blockieren. Gleichzeitig darf kein Hintergrundthread den Bevy-`World` unkontrolliert verändern.

Die öffentliche Session-API ist synchron. Der Umbau soll keinen öffentlichen Async-Runtime-Vertrag
und keine Abhängigkeit von einem bestimmten Executor einführen.

## Entscheidung

1. `Session::start` startet einen privaten Session-Koordinator und wartet auf dessen vollständiges
   Startergebnis. Erst nach Ready-Handshake und den erforderlichen Startprüfungen gibt es eine
   nutzbare `Session`.
2. Nach erfolgreichem Start besitzt allein der Koordinator den Kindprozess, seine Pipes und den
   veränderlichen Session-Zustand. `Session` ist der synchrone Handle zu diesem Koordinator und ist
   nicht klonbar.
3. `Session::send` übergibt einen Command an den Koordinator. Nach seiner Annahme läuft der Command
   unabhängig davon weiter, ob der Aufrufer `try_receive`, `receive` oder eine Event-Methode
   aufruft.
4. Ein fallengelassenes `Pending` hält den Command nicht an. Hostseitige Arbeit wie Replay macht
   ebenfalls ohne Polling durch den Aufrufer Fortschritt.
5. Eigene Leser leeren stdout und stderr ab dem Prozessstart fortlaufend. Der Koordinator verarbeitet
   ihre Nachrichten, korreliert Responses, führt hostseitige Commands aus, pflegt Recording und
   History und füllt die Ergebnis- und Event-Queues.
6. Ein langsamer Aufruf von `report::submit` im gemeinsamen Host-Ablauf hält den Koordinator und das
   Leeren der Prozess-Pipes nicht an.
7. `Session::try_receive` und `Session::try_receive_event` prüfen ausschließlich bereits verfügbare
   interne Nachrichten. Sie führen keine blockierenden Datei-, Prozess- oder Netzwerkoperationen
   aus und kehren sofort zurück.
8. `Session::receive` und `Session::receive_event` warten auf dem internen Nachrichtenkanal. Der
   Koordinator verarbeitet währenddessen andere Responses, Events und hostseitige Arbeit weiter.
9. Der private Koordinator, seine Threads und Kanäle sind kein öffentliches Transport- oder
   Async-Interface. Die API setzt weder Tokio noch einen anderen Executor voraus.
10. Auf der Controlled-Session-Seite liest eine private Komponente stdin, ohne den Bevy-Event-Loop zu
    blockieren. Sie übergibt dekodierte Requests an die Bevy-Seite. Ausschließlich Systeme im
    Bevy-Event-Loop greifen auf den `World` zu.
11. Lang laufende Bevy-Arbeit erhält pro Event-Loop-Durchlauf ein begrenztes Arbeitsbudget. Ein
    einzelner interner stdout-Writer serialisiert Ready, Responses und Protokollfehler als vollständige
    JSONL-Nachrichten.
12. `Session::drop` fordert den Koordinator zum Abbruch auf. Der Koordinator schließt die Pipes,
    beendet die Prozessgruppe und sammelt den Kindprozess ein. Drop führt weiterhin keinen
    fachlichen Shutdown aus und garantiert weder Command-Outcomes noch Recording- oder
    Report-Abschluss.

## Folgen

- Ein angenommenes Replay kann vollständig ablaufen, während der Aufrufer sein `Pending` nur hält.
- Controller müssen keine versteckte Pump-Schleife implementieren.
- `try_receive` und `try_receive_event` behalten eine klare nicht blockierende Bedeutung.
- Der Kindprozess kann während einer langsamen Host-Operation weiter Ausgaben erzeugen, ohne an
  stdout oder stderr zu blockieren.
- Die konkrete Threadzahl, Kanalbibliothek und interne Wake-up-Strategie bleiben
  Implementierungsdetails.
- Grenzen für nicht abgeholte Ergebnisse, History und Events werden getrennt festgelegt. Diese
  Entscheidung erlaubt keine unbegrenzten Queues.

## Prüfung

- Eine Fixture sendet einen Command, pollt mehrere Event-Loop-Durchläufe lang nicht und erhält danach
  das bereits entstandene Ergebnis.
- Eine Replay-Fixture hält das Start-`Pending` ohne weitere Session-Aufrufe und prüft, dass der Plan
  vollständig ausgeführt wird.
- Eine Drop-Fixture verwirft ein `Pending` und prüft, dass der Command weiterläuft und die History
  terminal ergänzt wird.
- Eine Kindprozess-Fixture schreibt mehr als eine Pipe-Kapazität auf stdout und stderr, während der
  Host anderweitig blockiert, und beendet sich ohne Deadlock.
- `try_receive` und `try_receive_event` werden ohne vorhandene Nachricht aufgerufen und kehren ohne
  Datei-, Prozess- oder Netzwerkzugriff zurück.
- Bevy-Fixtures senden weitere Commands während eines laufenden Warp und prüfen, dass ausschließlich
  der Bevy-Event-Loop den World-Zustand verändert.
- Eine Parallelitäts-Fixture erzeugt überlappende Responses und prüft, dass der einzelne Writer
  vollständige, nicht vermischte JSONL-Zeilen ausgibt.
