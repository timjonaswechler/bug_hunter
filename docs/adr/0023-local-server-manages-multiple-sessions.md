# Ein lokaler Server verwaltet mehrere unabhängige Sessions

## Status

Angenommen. Ersetzt die Ein-Session-pro-Server-Regel und die MCP-Präferenz aus
[ADR-0022](0022-session-host-outlives-clients.md). Dessen Regeln zur unabhängigen
Client-Lebensdauer und zur fachlichen Zuständigkeit von `session` bleiben bestehen.

Die Modulzuordnung wird durch [ADR-0024](0024-separate-server-client-and-cli.md) ersetzt:
`server`, `client` und `cli` lösen die `host`-Sammelstruktur auf. Die folgenden
`host::…`-Pfade dokumentieren die damalige Zuordnung; die Multi-Session-Regeln bleiben gültig.

## Kontext

Das [Inspirationsvideo](../api/research/inspiration-video-session-host.md) zeigt getrennte
Server- und Agent-Client-Aufrufe sowie die Steuerung mehrerer Spielinstanzen.
Die genaue Implementation ist nicht zugänglich und kein Vertrag für `bug_hunter`.
Für unser Ziel soll ein lokaler Server die Sessions gemeinsam verwalten, statt für jede
Session einen eigenen Hostprozess und zusätzlich eine serverübergreifende Discovery zu benötigen.

Die erste Bedienung erfolgt vollständig über die CLI. Weboberflächen und andere zusätzliche UIs
kommen später; MCP wird aus der aktuellen Planung entfernt.

## Entscheidung

1. Ein persistenter lokaler `host::server` verwaltet mehrere unabhängige `session::Session`-Werte.
   Er besitzt das Session-Verzeichnis, die Zuordnung von Session-Identität zu Session-Handle und
   die Lebensdauer dieser Handles. Ein Client besitzt keine Session.
2. Jede Session behält ihre fachlichen Regeln. Ihr privater Koordinator besitzt ihren
   Spielprozess, dessen Pipes und ihren veränderlichen Ausführungszustand. Commands, Pending,
   Recording, Replay, History und Shutdown werden nicht in der Serververwaltung dupliziert.
3. Session-Verwaltung und Commands innerhalb einer Session sind getrennte Operationen.
   Erzeugen, Auflisten und Auswählen sowie die Abfrage des Session-Zustands gehören zum
   Serververtrag. Spiel-Commands adressieren eine bestimmte Session.
4. Der bestehende Typ `session::RequestId` bleibt sessionlokal.
   Serverweite Korrelation muss zusätzlich die Session identifizieren.
   Die Session-ID ist weder eine Command-Request-ID noch ein Activity-Cursor.
5. `command::Command::Shutdown` wird weiterhin ausschließlich zu `Session::shutdown` der
   adressierten Session geroutet. Er beendet nicht den Server oder andere Sessions.
   Serverende ist eine eigene Operation; ihr Verhalten gegenüber aktiven Sessions bleibt offen.
6. Client-Trennung, EOF und das Ende eines Agent-Aufrufs senden weder Stop noch Shutdown.
   Mehrere Clients können dieselbe Session steuern; unterschiedliche Sessions teilen keine
   fachlichen Command-, Replay- oder Recording-Zustände.
7. Die CLI soll Server- und Session-Verwaltung sowie Command-Einreichung und Beobachtung abdecken.
   REPL, Script und der noch zu planende Agent-Zugang verwenden die gemeinsame private
   `host::client`-Seam. Es gibt im aktuellen Ziel kein `host::mcp`, keinen MCP-Vertrag und
   keine Weboberfläche.
8. Das interne Session Protocol v3 und das öffentliche typisierte Rust-Session-Interface bleiben
   unverändert. Weder Server-Discovery noch Session-IDs werden deshalb in den internen
   Spieltransport oder in direkte Rust-Session-Aufrufe aufgenommen.

## Noch offen

- HS1: Serverstart und Discovery, Session-Erzeugung und -Identität, Zustandsabfrage,
  fehlgeschlagener Start, Aufbewahrung beendeter Sessions und Verhalten beim Serverende.
- HS2: Client-Transport, Zugriffsschutz, Routing und Client-Fehler.
- HS3: Session-Zuordnung der Activity, Cursor-Geltungsbereich, Aufbewahrung und Gap-Verhalten.
- HS4: Annahmereihenfolge je Session, ungewisse Einreichung bei Verbindungsabbruch und Shutdown.
- H4: Agent-Bedienung, Modellwahl und Entscheidung über eine eigene oder eingebundene Agent-Laufzeit.

Die vorgeschlagene zufällige Session-ID mit hexadezimaler Kurzform ist noch kein abgeschlossener
Formatvertrag. Ebenso sind HTTP, die Flags des Videos und dessen Build-Ablauf nicht übernommen.

## Prüfung

- Über die CLI zwei Sessions erzeugen, auflisten und gezielt ansprechen, ohne zusätzliche UI.
- Gleiche Request-ID-Werte in unterschiedlichen Sessions eindeutig zuordnen.
- Recording oder Replay in Session A verändert die fachliche Annahme in Session B nicht.
- Shutdown oder unerwartetes Spielprozessende von Session A beendet weder Session B noch den Server.
- Einen Client mit ausstehender Arbeit trennen und deren Ergebnis später wieder beobachten.
- Client-, Server- und Session-Fehler unterscheiden; keine MCP-Abhängigkeit im Ziel.
- Direkte Rust-Nutzer betreiben weiterhin eine Session ohne lokalen Server.
