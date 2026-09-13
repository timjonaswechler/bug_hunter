# API-Entscheidungen

Dieses Dokument ist der Index für angenommene und ersetzte Entscheidungen, die die `bug_hunter`-API
betreffen. Die Entscheidung selbst steht in einer ADR-Datei. Ungeklärte Ideen in `goal.rs` werden
hier nicht als beschlossene Architektur geführt.

## Entscheidungen

| Entscheidung | Status | Bedeutung |
| --- | --- | --- |
| [ADR-0003: explizite Zeitsteuerung](../adr/0003-explicit-timing-no-hidden-advance-no-wait.md) | ersetzt durch ADR-0005 | Historische Step-Regel; keine versteckten Ticks, gesammelter Virtual Input und kein eingebautes Wait bleiben gültig. |
| [ADR-0004: Session Protocol v3](../adr/0004-session-protocol-v3-request-correlation.md) | angenommen | Responses werden über rein transportlokale IDs korreliert; der zentrale Codec verwendet qualifizierte Command-Namen und meldet nicht fatale Protokollfehler. |
| [ADR-0005: Tick-Warp](../adr/0005-tick-warp-preserves-explicit-simulation-control.md) | angenommen | Tick-Warp ersetzt Step und die hostdefinierte Tick-Dauer, ohne versteckte Ticks oder eingebaute Wait-Schleifen wieder einzuführen. |
| [ADR-0006: Identität aus Fehlerart und bereinigter Meldung](../adr/0006-versioned-failure-signatures.md) | angenommen | Eine versionierte SHA-256-Signatur kodiert ausschließlich Fehlerart und bereinigte Meldung; zusätzliche Diagnosedaten verändern die Duplikatidentität nicht. |
| [ADR-0007: Report-Kontext als Session-Momentaufnahme](../adr/0007-report-context-is-a-session-snapshot.md) | angenommen | Reports übernehmen eine feste Auswahl der beim Session-Start wirksamen Anwendungs-, Quell-, Toolchain- und Verlaufsdaten direkt aus der Session. |
| [ADR-0008: direkter Provider mit lokalem Rückfall](../adr/0008-configured-provider-with-local-fallback.md) | angenommen | Der konfigurierte Provider wird direkt verwendet; jeder Remote-Fehler sichert den Report lokal und bleibt zusammen mit dem lokalen Pfad sichtbar. |
| [ADR-0009: Session transportiert Report-Beobachtungen](../adr/0009-session-transports-report-observations.md) | angenommen | Session leert und transportiert stderr; `report` deutet interne Marker, und der gemeinsame Host-Ablauf erzeugt und veröffentlicht Reports. |
| [ADR-0010: Session-Pfade am Cargo-Manifest](../adr/0010-manifest-rooted-session-paths.md) | angenommen | Ein Pflicht-Manifest bestimmt den Projektordner; Screenshot, Recording und Report verwenden ein gemeinsames, kanonisches Artifact-Root. |
| [ADR-0011: Session-Fortschritt auf privatem Koordinator](../adr/0011-session-progresses-on-a-private-coordinator.md) | angenommen | Ein interner Koordinator treibt Commands und Host-Arbeit unabhängig vom Polling des synchronen, nicht klonbaren Session-Handles an. |
| [ADR-0012: Command-Annahme vor Pending-Ergebnis](../adr/0012-command-acceptance-precedes-pending-results.md) | angenommen | Nur eine nicht verfügbare Session oder erschöpfte IDs verhindern die Annahme; commandspezifische Ergebnisse schließen ein nicht klonbares `Pending` ab. |
| [ADR-0013: begrenztes History-Fenster](../adr/0013-session-history-is-a-bounded-report-window.md) | angenommen | Die Session hält für Reports die letzten 50 angenommenen Commands; vollständige dauerhafte Abläufe bleiben Aufgabe des Recordings. |
| [ADR-0014: typisiertes terminales Session-Event](../adr/0014-session-end-is-a-terminal-typed-event.md) | angenommen | Unerwartete Prozess-, Transport- und Überlastungsenden werden typisiert und nach allen übrigen Events genau einmal ausgegeben. |
| [ADR-0015: geordnete versionierte JSONL-Recordings](../adr/0015-recordings-are-ordered-versioned-jsonl-sections.md) | angenommen | Ein Recording schreibt qualifizierte Commands und terminale Outcomes in Annahmereihenfolge; sichere Start- und Stop-Barrieren begrenzen den Abschnitt. |
| [ADR-0016: vollständige Replay-Validierung](../adr/0016-replay-validates-recordings-before-execution.md) | angenommen | Ein privater Versionsadapter validiert ein Recording vollständig und baut den Replay-Plan, bevor der erste Command ausgeführt wird. |
| [ADR-0017: lokaler oder terminaler Session-Fehler](../adr/0017-session-errors-have-local-or-terminal-scope.md) | angenommen | Command- und Bedienfehler bleiben lokal; Prozess- und Transportverlust beenden die Session, und Shutdown beginnt nur an einer ruhenden Grenze. |
| [ADR-0018: typisierte und versiegelte Requests](../adr/0018-public-requests-are-typed-and-sealed.md) | angenommen | Konkrete Requests behalten ihren Output-Typ; der Session-Host verwendet den privaten typgelöschten Weg einschließlich der Route zu `Session::shutdown`. |
| [ADR-0019: JSON für komplexe REPL-Inspect-Queries](../adr/0019-repl-uses-json-for-complex-inspect-queries.md) | angenommen | Vier feste Kurzformen decken häufige Abfragen ab; alle komplexen Inspect-Eingaben verwenden direkt das gemeinsame JSON-Argumentobjekt. |
| [ADR-0020: lokale Agent-Eingabefehler](../adr/0020-invalid-agent-input-stays-local.md) | teilweise wieder geöffnet durch ADR-0022 | Die zwei Fehlerklassen bleiben; ihre äußere Form wird im gemeinsamen Client-Protokoll erneut geprüft. |
| [ADR-0021: getrennte Agent-Session-Events](../adr/0021-agent-serializes-session-events-separately.md) | teilweise wieder geöffnet durch ADR-0022 | Die Event-Payloads bleiben; H3 ergänzt sie um den wiederaufnehmbaren Activity-Stream. |
| [ADR-0022: persistenter Session-Host](../adr/0022-session-host-outlives-clients.md) | angenommen | Ein lokaler Host besitzt eine Session unabhängig von seinen Clients; REPL, Script und MCP-Agent verwenden einen gemeinsamen Command- und Activity-Stream. |
