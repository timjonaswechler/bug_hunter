# Öffentliche Requests bleiben typisiert und versiegelt

## Status

Angenommen.

## Kontext

`Session::send` verbindet einen Command über `command::Request::Output` mit seinem konkreten
Rückgabetyp. Ein öffentlich implementierbares `Request`-Trait würde fremden Typen erlauben, einem
vorhandenen Command einen unzutreffenden Output-Typ zuzuordnen. Der fest versionierte
Command-Vertrag besitzt zugleich keinen Erweiterungspunkt für anwendungsdefinierte Commands.

Agent, REPL und Script arbeiten dagegen mit der typgelöschten `command::Command`-Darstellung. Diese
Darstellung umfasst Commands mit unterschiedlichen Outputs und den ausschließlich über
`Session::shutdown` erlaubten Shutdown. Sie kann deshalb nicht selbst denselben öffentlichen
`Request`-Vertrag erfüllen.

Ein öffentliches zweites Sende- oder Empfangsinterface nur für die eingebauten Host-Abläufe würde
die Regeln für Annahme, Request-IDs, Pending-Ergebnisse und Shutdown nach außen duplizieren.

## Entscheidung

1. `command::Request` ist ein öffentlich nutzbares, aber versiegeltes Trait. Nur die von
   `bug_hunter` definierten Command-Typen können es implementieren.
2. Jeder konkrete Request-Typ bestimmt genau einen Output-Typ. Das öffentliche
   `Session::send<C: Request>` und die typisierten `Pending<C::Output>` bleiben erhalten.
3. `command::Command` ist die gemeinsame typgelöschte Darstellung für Parsing, History, Recording
   und interne Ausführung. Der Typ implementiert nicht `Request`.
4. `command::Command::Shutdown` ist kein über `Session::send` ausführbarer Request. An der
   öffentlichen Session-Grenze beginnt Shutdown ausschließlich über `Session::shutdown`, damit
   Vorbedingungen und reservierte Request-ID nicht umgangen werden können.
5. `session` besitzt einen crate-internen Adapter, der eine bereits validierte `command::Command`
   über denselben Koordinator annimmt und ihr Ergebnis typgelöscht bereitstellt. Der persistente
   Session-Host verwendet diesen Adapter, ohne Annahme-, Korrelations- oder Fehlerregeln nachzubauen.
6. Für `command::Command::Shutdown` routet der Adapter ausdrücklich zu `Session::shutdown`, statt die
   Variante über das allgemeine `Session::send` auszuführen. Dadurch dürfen REPL, Script und
   MCP-Agent Shutdown über denselben Client-Command-Weg anfordern, ohne Vorbedingungen oder
   reservierte Request-ID selbst zu verwalten.
7. Der Koordinator darf den eingebauten Host-Abläufen crate-intern eine Aktivitätsbenachrichtigung
   anbieten. Dadurch können sie mehrere typgelöschte Ergebnisse und Session-Events verarbeiten,
   ohne eng zu pollen.
8. Das öffentliche Session-Interface erhält kein zweites dynamisches `send`, kein `receive_any` und
   keinen öffentlichen Aktivitätskanal. Direkte Rust-Nutzer verwenden konkrete Requests und deren
   `Pending`-Werte.

## Folgen

- Eine externe Crate kann keinen falschen Output-Typ für einen vorhandenen Command behaupten.
- Direkte Rust-Nutzer erhalten weiterhin typisierte Ergebnisse.
- Der Session-Host kann heterogene Client-Commands einschließlich einer ausdrücklichen
  Shutdown-Anforderung gemeinsam verwalten, ohne eine zweite öffentliche Ausführungsregel
  einzuführen.
- Neue Commands benötigen eine Implementation des versiegelten Traits innerhalb von `bug_hunter`
  und eine Ergänzung des privaten dynamischen Adapters.
- Shutdown kann nicht durch die allgemeine Command-Annahme an seinen Vorbedingungen vorbei gesendet
  werden.

## Prüfung

- Ein Compile-Fail-Test belegt, dass eine externe Crate `command::Request` nicht implementieren
  kann.
- API-Tests prüfen die konkreten Output-Typen aller öffentlichen Requests.
- Agent-, REPL- und Script-Fixtures reichen heterogene Commands über den privaten Adapter ein und
  erhalten deren Outcomes ungeordnet.
- Ein API-Test belegt, dass `command::Command` nicht an das öffentliche `Session::send` übergeben
  werden kann.
- Shutdown-Fixtures belegen, dass der interne Adapter eine Client-Anforderung ausschließlich zu
  `Session::shutdown` routet und nur dieser Aufruf die reservierte Request-ID verwendet.
