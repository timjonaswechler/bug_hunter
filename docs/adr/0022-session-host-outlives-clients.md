# Ein persistenter Session-Host überlebt seine Clients

## Status

Teilweise ersetzt durch [ADR-0023](0023-local-server-manages-multiple-sessions.md).
Die Ein-Session-pro-Server-Regel und die MCP-Präferenz sind nicht mehr das Ziel.
Client-unabhängige Session-Lebensdauer, Session Protocol v3 und die fachlichen Regeln im
Session-Modul bleiben gültig. Der folgende Text dokumentiert die ursprüngliche Entscheidung;
für Multi-Session-Verwaltung und den aktuellen CLI-Umfang gilt ADR-0023.

## Kontext

Die bisherige Host-Planung bindet Agent, REPL oder Script unmittelbar an eine laufende
`session::Session`. Insbesondere nimmt der Agent eine langlebige JSONL-Verbindung über
stdin/stdout an. Diese Form koppelt Prozessbesitz, Command-Eingabe und Beobachtung an den Client, der
den Host gestartet hat.

Ein Sprachmodell verarbeitet neu geschriebene stdout-Bytes nicht selbstständig. Erst ein
abgeschlossener Tool-Aufruf macht die Ausgabe zum Eingang eines neuen Modellschritts. Ein Mensch kann
außerdem nicht zuverlässig die Konsole eines Agent-Tool-Aufrufs übernehmen, und das Ende dieses
Aufrufs darf die Controlled Session nicht beenden.

Die Produktidee ist von ThePrimeagens Video
["am I becoming a real Game Dev?"](https://www.youtube.com/watch?v=tYQyh1tjSFc) inspiriert. Das
Video zeigt eine JSON-steuerbare Spielinstanz, die über eine FIFO von ihrem aufrufenden Terminal
entkoppelt, hinter einen Server gestellt und von einem Agent-Client bedient wird. Für `bug_hunter`
ist die entscheidende Regel dieselbe: Eine langlebige Instanz besitzt das Spiel; austauschbare
Clients beobachten und steuern sie.

## Entscheidung

1. Für eine Controlled Session läuft zunächst genau ein persistenter lokaler Session-Host. Dieser
   Prozess besitzt den einzigen `session::Session`-Wert, den Controlled-Application-Prozess und den
   vollständigen Session-Lebenszyklus.
2. Agent, REPL und Script sind Clients dieses Hosts. Sie besitzen weder `Session` noch die
   Prozess-Pipes und können sich verbinden oder trennen, ohne die Session dadurch zu beenden.
3. Das bestehende Session Protocol v3 bleibt ausschließlich die interne Verbindung zwischen
   `session::Session` und der Controlled Bevy Application. Clients greifen nicht direkt auf dieses
   Protokoll oder seine stdin/stdout-Pipes zu.
4. Der Session-Host bietet ein versioniertes lokales Client-Protokoll. Eine gemeinsame private
   Client-Bibliothek ist die Seam für REPL, Script und die Agent-Fassade. Der Agent erhält
   begrenzte Tool-Aufrufe, vorzugsweise über MCP, statt die dauerhafte Aufmerksamkeit eines
   Sprachmodells vorauszusetzen.
5. Der Host übernimmt angenommene Commands, terminale Outcomes und Session-Events in einen
   wiederaufnehmbaren Activity-Stream. Jeder Eintrag besitzt neben einer möglichen Request-ID einen
   monotonen Transport-Cursor. Aufbewahrungsgrenze, Gap-Verhalten und genaue JSON-Form werden
   gesondert festgelegt.
6. Mehrere Clients dürfen gleichzeitig verbunden sein und Commands senden. Der Session-Host führt
   sie in einer eindeutigen Empfangsreihenfolge dem bestehenden Session-Koordinator zu; die Session
   vergibt weiterhin die globalen Request-IDs und erzwingt ihre Command-, Replay-, Recording- und
   Shutdown-Regeln.
7. Ein Client-Abbruch, EOF oder das Ende eines Tool-Aufrufs sendet keinen Stop- oder
   Shutdown-Command. Laufende Arbeit und aktive Recordings bleiben Teil der Session; spätere Clients
   können ihre Outcomes und Events über den Activity-Stream beobachten.
8. Agent, REPL und Script dürfen ausdrücklich `command::Command::Shutdown` einreichen. Der
   serverseitige gemeinsame Adapter routet diese Variante zu `Session::shutdown`, statt sie über das
   allgemeine `Session::send` auszuführen. Dadurch bleiben Vorbedingungen, reservierte Request-ID und
   Prozessabschluss im Session-Modul.
9. Direkte Rust-Nutzer verwenden weiterhin konkrete Requests über `Session::send` und rufen
   `Session::shutdown` selbst auf. Das öffentliche Session-Interface erhält keinen dynamischen
   Client- oder Serververtrag.
10. Prozessstart und Discovery, lokaler Transport und Zugriffsschutz, Activity-Stream sowie
    Client- und Shutdown-Abläufe werden als HS1 bis HS4 abgeschlossen, bevor die
    transportabhängigen Agent- und Host-Interfaces festgelegt werden.

## Folgen

- Die Session-Lebensdauer ist unabhängig von Bash-, MCP-, REPL- und Script-Aufrufen.
- Menschen und Agenten benötigen keinen Zugriff auf dieselbe Konsole. Sie verbinden sich über das
  lokale Client-Protokoll mit derselben Session.
- Ein Agent kann `send`, `poll` und `wait` als begrenzte Tool-Aufrufe verwenden; jede Rückgabe kann
  einen neuen Modellschritt auslösen.
- Mehrere Beobachter und Command-Sender sind möglich. Fachliche Konflikte werden nicht in jedem
  Client dupliziert, sondern durch die bestehende Session-Annahme entschieden.
- Der serverseitige Activity-Stream muss Ergebnisse über Client-Trennungen hinweg begrenzt
  aufbewahren und Lücken sichtbar machen.
- Die bisherige langlebige stdin/stdout-Agent-Verbindung, automatischer Shutdown nach einem
  Controller-Ablauf und direkte Übergabe von `&mut Session` an Host-Controller sind nicht mehr das
  Zielmodell.
- Die Payload-Entscheidungen aus ADR-0020 und ADR-0021 bleiben Kandidaten, ihre äußere
  Transportform und Reihenfolge werden jedoch gegen den gemeinsamen Client-Stream neu geprüft.

## Prüfung

- Prozess-Fixtures trennen den startenden Client und belegen, dass Session-Host und Controlled
  Application weiterlaufen.
- REPL-, Script- und MCP-Fixtures verbinden sich unabhängig mit derselben Session und beobachten
  vorhandene sowie neue Activity-Einträge.
- Mehrere Clients senden Commands; deren Annahmereihenfolge, globale Request-IDs und Outcomes bleiben
  eindeutig.
- Ein Client trennt sich mit ausstehenden Commands und ein späterer Client liest deren Outcomes ab
  seinem Cursor.
- Client-Trennung sendet weder Stop noch Shutdown. Ein ausdrücklicher Shutdown durch jeden
  Client-Typ verwendet ausschließlich `Session::shutdown`.
- Direkte Rust-API-Tests belegen, dass das öffentliche `Session`-Interface keinen Server- oder
  dynamischen Client-Vertrag exportiert.
