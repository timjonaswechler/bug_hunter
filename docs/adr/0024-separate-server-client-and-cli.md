# Server, Client-Zugriff und CLI sind getrennte Module

## Status

Angenommen. Ersetzt die `host`-Sammelstruktur und deren gemeinsame `host::run`-Fassade
aus der bisherigen Zielplanung. Die fachlichen Entscheidungen aus
[ADR-0023](0023-local-server-manages-multiple-sessions.md) bleiben gültig; dessen
`host::…`-Modulpfade werden durch die hier beschriebene Zuordnung ersetzt.

## Kontext

Mit mehreren Sessions und unabhängigen Clients fasst `host` nicht mehr einen einzelnen
Start-und-Controller-Ablauf zusammen. Es enthält zugleich den Besitzer der Sessions,
den Zugriff auf diesen Besitzer und konkrete Terminalbedienung. Eine bloße Umbenennung
würde diese unterschiedlichen Verantwortungen nicht trennen.

## Entscheidung

- `server` verwaltet Session-Verzeichnis und Session-Handles und stellt den Client-Vertrag bereit.
  Es kennt weder REPL-Syntax noch Script-Dateiformate oder Modellwahl.
- `client` kapselt die Verbindung zum Server, Protokollkodierung und Antwortzuordnung für
  Session-Verwaltung, Commands und Activity. Es besitzt weder Terminaldarstellung noch Spielregeln.
- `cli` verarbeitet Argumente, stellt Ergebnisse im Terminal dar und wählt den auszuführenden
  Ablauf, etwa Serverstart, Session-Erzeugen oder REPL. REPL und Script werden als
  `cli::repl` und `cli::script` eingeordnet. Die CLI besitzt keine Session-Regeln.
- `session` bleibt von diesen Modulen unabhängig und führt genau eine Session aus.
  Kindprozess, Pending, Recording, Replay und History behalten ihren bisherigen Owner.
- Der bisherige gemeinsame Report-Ablauf wird `server` zugeordnet, nicht `cli`.
  Der Server verarbeitet Session-Events und ruft `Report::create` und `report::submit` auf,
  auch wenn kein Client verbunden ist. Report-Erstellung und Provider-Regeln bleiben bei `report`.
- Der Agent-Zugang wird erst in H4 eingeordnet. Ein externer Agent, der die CLI benutzt,
  verlangt nicht zwingend ein eigenes Modul; eine eigene oder eingebundene Agent-Laufzeit
  ist weiterhin nicht beschlossen.

## Abgrenzung und Folgen

Die Entscheidung legt Modulverantwortungen fest, nicht Crates, ausführbare Programme,
öffentliche Rust-Exports oder Feature-Namen. H7 entscheidet diese Punkte gesondert.
Die bisherige Kopplung "Feature `host` schaltet genau Modul `host` frei" ist damit wieder offen.
Direkte Rust-Session-Nutzung und `session::Plugin` dürfen weiterhin keine Server- oder
CLI-Abhängigkeit voraussetzen; der alte Feature-Alias `driver` bleibt gestrichen.

Der heutige Bevy-seitige Code unter `client` wird weiterhin nach `session` überführt.
Das neue Zielmodul `client` bezeichnet ausschließlich den Zugriff auf den lokalen Server.
Es ist kein Re-Export des bisherigen Bevy-Clients.

Ein gemeinsamer Protokollvertrag verlangt einen Owner für Wire-Typen und Versionierung.
Wo dieser Vertrag liegt, wird in HS2 entschieden; die Modultrennung rechtfertigt keine
getrennten Client- und Serverkopien desselben Codecs.

## Prüfung

- `goal.rs` enthält `server`, `client` und `cli` als getrennte Module, ohne `host`-Wrapper
  oder vorweggenommenen gemeinsamen `run`-Export.
- CLI, REPL und Script verwenden den Client-Zugriff, statt Session-Regeln zu kopieren.
- Session-Verarbeitung und Report-Auslösung laufen ohne CLI-Verbindung weiter.
- Der Server besitzt weder Terminalparser noch Modellanbindung; `client` rendert keine Ausgaben.
- Nach H7 prüfen Builds und Exporttests den beschlossenen Feature- und Crate-Zuschnitt.
