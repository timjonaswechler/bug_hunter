# API-Entscheidungen

Dieses Dokument ist der Index für angenommene Entscheidungen, die die `bug_hunter`-API betreffen.
Die Entscheidung selbst steht in einer ADR-Datei. Ungeklärte Ideen in `goal.rs` werden hier nicht
als beschlossene Architektur geführt.

## Angenommene Entscheidungen

| Entscheidung | Status | Bedeutung |
| --- | --- | --- |
| [ADR-0003: explizite Zeitsteuerung](../../../../docs/adr/0003-explicit-timing-no-hidden-advance-no-wait.md) | angenommen | Zeit vergeht nur durch `step`; virtuelle Eingabe wird bis zum nächsten `step` gesammelt. |
| [ADR-0004: Session Protocol v3](../../../../docs/adr/0004-session-protocol-v3-request-correlation.md) | angenommen | Responses werden über rein transportlokale IDs korreliert; der zentrale Codec verwendet qualifizierte Command-Namen und meldet nicht fatale Protokollfehler. |
