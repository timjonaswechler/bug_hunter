# Reihenfolge der offenen API-Entscheidungen

Diese Liste ordnet die offenen Stellen aus [`README.md`](README.md),
[`migration.md`](migration.md) und [`goal.rs`](goal.rs) nach ihren Abhängigkeiten.
Ein Punkt wird erst bearbeitet, wenn alle unter **Benötigt** genannten Punkte abgeschlossen sind.
So werden Entscheidungen zuerst im zuständigen Modul getroffen und danach in die größeren
Session- und Host-Interfaces übernommen.

Die Reihenfolge unterscheidet drei Arten von Arbeit:

- **Recherche** klärt eine technische Tatsache. Sie trifft noch keine Produktentscheidung.
- **Entscheidung** legt eine von außen sichtbare Regel oder Form fest.
- **Abschluss** prüft ein größeres Interface gegen alle darunter getroffenen Entscheidungen.

## Empfohlene Bearbeitungsreihenfolge

Diese lineare Reihenfolge ist der Arbeitsplan. Jeder Punkt wird abgeschlossen und in `goal.rs` und
`migration.md` festgehalten, bevor der nächste Punkt beginnt.

1. [x] **I1:** Verhalten unbekannter und mehrdeutiger Type Paths festlegen.
2. [x] **I2:** Inspect auf read-only Queries begrenzen und `Set` entfernen.
3. [x] **I3:** Allgemeine Asset-Inspection aus dem Zielinterface entfernen.
4. [ ] **I4:** Kanonische JSON-Abbildung reflektierter Werte festlegen.
5. [ ] **I5:** `command::inspect` einschließlich seiner JSON-Form und Wire-Fixtures abschließen.
6. [ ] **T1:** Den Ersatz von ADR-0003 für Tick-Warp festhalten.
7. [ ] **P1:** Die vollständige v3-Command-Abbildung abschließen.
8. [ ] **R1:** Beobachtbare Panics in Bevy-Systemen, Tasks und Worker-Threads untersuchen.
9. [ ] **R2:** Backtrace-Aktivierung, Erfassung und stabile Teile untersuchen.
10. [ ] **R3:** Die zuverlässige Erkennung von `tracing`-Errors untersuchen.
11. [ ] **R4:** Berechnung, Normalisierung und Speicherung der Fehlersignatur festlegen.
12. [ ] **R5:** Inhalt von `report::Context` und Ermittlung der Anwendungsversion festlegen.
13. [ ] **R6:** Konfiguration und Operationen des GitHub-Providers festlegen.
14. [ ] **R7:** Die öffentlichen Fehlergruppen von `report::Error` festlegen.
15. [ ] **R8:** Das gesamte Reporting-Interface abschließen.
16. [ ] **RP1:** Darstellung und Veröffentlichung von Replay-Abweichungen festlegen.
17. [ ] **RP2:** Das Replay-Interface abschließen.
18. [ ] **SR1:** Die Seam zwischen Session-Beobachtung und Reporting festlegen.
19. [ ] **S1:** Alle verbleibenden Session-Fragen als konkrete `OPEN`-Punkte erfassen.
20. [ ] **S2:** `session::Session` und seine direkten Module abschließen.
21. [ ] **H1:** Die REPL-Syntax für komplexe Inspect-Queries festlegen.
22. [ ] **H2:** Das Agent-Verhalten bei ungültigen JSONL-Eingaben festlegen.
23. [ ] **H3:** Die Ausgabe von Session-Events und unerwartetem Session-Ende im Agent-Modus
    festlegen.
24. [ ] **H4:** EOF und kontrollierten Abschluss des Agent-Modus festlegen.
25. [ ] **H5:** Die Capability-Anforderungen der einzelnen Host-Abläufe festlegen.
26. [ ] **H6:** Alle verbleibenden Host-Fragen als konkrete `OPEN`-Punkte erfassen.
27. [ ] **H7:** `host::run` und die öffentliche Host-Fassade abschließen.

Die Detailabschnitte unten beschreiben für jeden Punkt Owner, Voraussetzungen und erwartetes
Ergebnis.

## Stufe 1: unabhängige Fragen innerhalb eines Moduls

Diese Punkte können unabhängig voneinander bearbeitet werden. Sie sollen keine Session- oder
Host-Regeln vorwegnehmen.

### I1: Verhalten unbekannter und mehrdeutiger Type Paths

- **Art:** Entscheidung
- **Owner:** `command::inspect::query`
- **Benötigt:** nichts

Für jedes Query-Feld ist festzulegen, ob ein unbekannter oder mehrdeutiger Type Path den Command
ablehnt oder ein leeres beziehungsweise statusbehaftetes Ergebnis erzeugt. Dazu gehören mindestens:

- `entity.with` und `entity.without`,
- `component::Selection::Listed`,
- `resource::Selector::Type`.

Das Ergebnis muss in `goal.rs` und im Abschnitt "Inspect" von `migration.md` stehen.

**Entschieden:** Jeder vom Aufrufer gelieferte Type Path muss sich vor der Ausführung exakt über
Bevys `TypeRegistry` auflösen lassen. Andernfalls wird der gesamte Query-Command ohne Teilergebnis
abgelehnt. Short Paths und Aliase werden nicht eigens aufgelöst; `Unavailable::NotRegistered`
ersetzt diese Ablehnung nicht.

### I2: Read-only-Grenze von Inspect

- **Art:** Entscheidung
- **Owner:** `command::inspect`
- **Benötigt:** nichts

**Entschieden:** Inspect dient ausschließlich der Prüfung des aktuellen Spielzustands. Es besitzt
nur Queries und mutiert den World nicht. Ein `Set`-Command und alle Mutations- oder
Benachrichtigungsgarantien entfallen.

### I3: Allgemeine Asset-Inspection

- **Art:** Entscheidung
- **Owner:** `command::inspect`
- **Benötigt:** nichts

**Entschieden:** Das Zielinterface besitzt keine allgemeine Asset-Query, keinen Asset-Schlüssel und
keine Abbildung von Bevy-Asset-IDs. Sichtbare Assetwirkung wird per Screenshot geprüft; fachlich
relevanter Ladezustand soll über Components oder Resources der Anwendung beobachtbar sein. Eine
enge Asset-Status-Abfrage wird erst bei einem konkreten, dadurch nicht abgedeckten Anwendungsfall
neu geplant.

### I4: kanonische JSON-Abbildung reflektierter Werte

- **Art:** Entscheidung
- **Owner:** `command::inspect::value`
- **Benötigt:** nichts

Alle Value-Queries brauchen dieselbe Abbildung für Structs, Tupel, Enums, Maps, Sets, Handles und
numerische Sonderwerte. Festzulegen sind sowohl die JSON-Form als auch Ablehnungs- und
`Unavailable`-Fälle. Diese Entscheidung betrifft nur reflektierte Inspect-Werte. Sie legt nicht die
gesamte Protokollhülle fest.

### T1: Ersatz von ADR-0003 für Tick-Warp

- **Art:** Entscheidung dokumentieren
- **Owner:** `command::tick`
- **Benötigt:** nichts

Die bereits entworfenen Tick-Warp-Regeln widersprechen ADR-0003. Vor der Implementation muss eine
neue ADR festhalten, welche Regeln ersetzt werden und welche, etwa das Verbot versteckter Ticks,
bestehen bleiben.

### R1: beobachtbare Panics in den unterstützten Ausführungsorten

- **Art:** Recherche
- **Owner:** `report`
- **Benötigt:** nichts

Zu untersuchen sind Panics in Bevy-Systemen, asynchronen Tasks und Worker-Threads sowie die
unterstützten Panic-Strategien. Das Ergebnis muss benennen, welche Meldung und welcher Prozessstatus
für den Debug Host zuverlässig beobachtbar sind.

### R2: Backtrace-Aktivierung, Erfassung und Stabilität

- **Art:** Recherche
- **Owner:** `report`
- **Benötigt:** nichts

Zu untersuchen ist, wie der Debug Host beim Session-Start einen Backtrace anfordert, welche Daten er
zuverlässig erhält und welche Teile zwischen Builds stabil normalisiert werden können.

### R3: Erkennung von `tracing`-Errors

- **Art:** Recherche
- **Owner:** `report`
- **Benötigt:** nichts

Zu untersuchen sind die von Bevy verwendete `tracing`-Ausgabe, benutzerdefinierte Formatter,
ANSI-Ausgabe und mehrzeilige Events. Das Ergebnis muss eine belastbare Grenze zwischen einem
Error-Level-Event und beliebigem Text auf `stderr` liefern.

## Stufe 2: kleine Interfaces aus den lokalen Antworten abschließen

### I5: `command::inspect` abschließen

- **Art:** Abschluss
- **Owner:** `command::inspect`
- **Benötigt:** I1, I2, I3, I4

Die vier Inspect-Entscheidungen werden gemeinsam gegen Query, Output und Fehlerverhalten geprüft.
Danach werden die genaue JSON-Form beider Query-Kategorien und ihre Wire-Fixtures festgelegt.
Erst dann gilt `command::inspect` für die Protokollplanung als geschlossen.

### R4: Fehlersignatur festlegen

- **Art:** Entscheidung
- **Owner:** `report::Signature`
- **Benötigt:** R1, R2, R3

Festzulegen sind Berechnung, Normalisierung und persistierte Darstellung der Signatur für Panic,
Prozessende und aktivierte `tracing`-Errors. Flüchtige Adressen, absolute Projektpräfixe, Session,
Controller und Command-Verlauf dürfen die Identität nicht verändern.

### R5: Inhalt von `report::Context` und Anwendungsversion

- **Art:** Entscheidung
- **Owner:** `report::Context`
- **Benötigt:** nichts

Festzulegen sind die Anwendungs- und Session-Angaben für Diagnose und Reproduktion sowie die Quelle
der Anwendungsversion. Dabei ist zu prüfen, welche Daten `session::Session` besitzen muss, damit
`Report::create` den Context ohne ein zweites öffentliches Session-Context-Modell erzeugen kann.
Falls dafür neue Session-Daten nötig sind, werden sie hier benannt, aber erst unter S2 in das
Session-Interface übernommen.

### R6: GitHub-Provider-Interface

- **Art:** Entscheidung
- **Owner:** `report::provider::github`
- **Benötigt:** R4, R5

Festzulegen sind Repository- und Veröffentlichungseinstellungen sowie das Interface für
Duplikatsuche, lokalen Entwurf und Veröffentlichung. Die Duplikatsuche verwendet die unter R4
festgelegte Signatur.

### R7: öffentliche Fehler von `report`

- **Art:** Entscheidung
- **Owner:** `report::Error`
- **Benötigt:** R6

Die Provider- und Persistenzfehler werden erst nach den Provider-Operationen festgelegt. Obwohl
`migration.md` diese Arbeit bei der Implementation verortet, ist `report::Error` Teil des
öffentlichen Ziel-Interfaces. Seine Fehlergruppen müssen deshalb vor der Implementation feststehen.
Betriebssystemspezifische Details dürfen intern bleiben.

### R8: Reporting-Interface abschließen

- **Art:** Abschluss
- **Owner:** `report`
- **Benötigt:** R4, R5, R6, R7

`Report`, `Failure`, `Signature`, `Context`, Provider-Konfiguration, Provider-Outcome und
`report::Error` werden zusammen geprüft. Dabei muss auch feststehen, welche Daten die Session nur
beobachtet und an welcher Seam `report` daraus einen fachlichen Fehler erkennt.

## Stufe 3: Verträge, die mehrere kleine Module verbinden

### P1: vollständige v3-Command-Abbildung abschließen

- **Art:** Abschluss
- **Owner:** `session::protocol`
- **Benötigt:** I5, T1

Für alle Wire-Commands werden qualifizierter Name, `arguments`, erfolgreicher Output, Ablehnung und
stabile Fehlercodes festgelegt. Dazu gehören Ready, Input, Tick, Inspect, Screenshot und Shutdown.
Danach können Agent, Script und REPL dieselbe Command-Form verwenden, ohne eine zweite
Command-Sprache zu erfinden.

Recording und Replay bleiben hostseitige Commands. Ihre Command-Einträge verwenden dieselbe
äußere Form, gehören aber nicht zum Wire-Vertrag der Controlled Session.

### RP1: Darstellung und Veröffentlichung von Replay-Abweichungen

- **Art:** Entscheidung
- **Owner:** `command::replay`
- **Benötigt:** R8

Festzulegen ist, wie erkannte Abweichungen nach einem Replay zugänglich werden. Zu entscheiden sind
mindestens:

- ob sie Teil von `replay::Completion`, ein getrenntes Ergebnis oder ein Report sind,
- welche Daten pro Abweichung sichtbar werden,
- ob und wann eine Abweichung veröffentlicht wird,
- wie mehrere Abweichungen eines weiterlaufenden Replays dargestellt werden.

Die Entscheidung darf die bereits beschlossene Regel nicht ändern, dass Abweichungen das Replay
nicht stoppen und `completed` nicht in `blocked` umdeuten.

### RP2: Replay-Interface abschließen

- **Art:** Abschluss
- **Owner:** `command::replay`
- **Benötigt:** RP1

`Start`, `Stop`, `Completion`, `Outcome` und die Abweichungsdarstellung werden gemeinsam geprüft.
Danach darf die Session-Ausführung die endgültigen Replay-Outputs und Fehler übernehmen.

### SR1: Integration von Session-Beobachtung und Reporting festlegen

- **Art:** Entscheidung
- **Owner:** Seam zwischen `session` und `report`
- **Benötigt:** R8

Die Dokumente legen fest, dass `Session` stderr und Prozessstatus beobachtet, diese Daten aber nicht
fachlich als Panic, `tracing`-Error oder Prozessfehler interpretiert. Gleichzeitig zeigt `goal.rs`
noch keinen Weg, über den `report` diese Beobachtungen erhält. Vor Abschluss des Session-Interfaces
ist daher festzulegen:

- welche Beobachtungsdaten `Session` bereitstellt,
- wer `Report::create` und `report::submit` auslöst,
- wie die Vorrangregel Panic vor Prozessende angewendet wird,
- wie ein erzeugter oder fehlgeschlagener Report für den Host sichtbar wird.

Diese Entscheidung darf kein zweites Modul einführen, das dieselben Prozess- oder History-Daten wie
`Session` besitzt.

## Stufe 4: das Session-Interface schließen

### S1: verbleibende Session-Fragen ausdrücklich erfassen

- **Art:** Bestandsaufnahme
- **Owner:** `session`
- **Benötigt:** P1, RP2, SR1, R5

`README.md` nennt die übrige Session-Struktur als offen und `migration.md` bezeichnet das
öffentliche Session-Interface nur als grob festgelegt. Bevor weitere Typen angenommen werden, werden
alle noch fehlenden Entscheidungen als konkrete `OPEN`-Punkte in `goal.rs` eingetragen. Die
Bestandsaufnahme prüft insbesondere:

- Start und Ready-Handshake,
- `Pending`, `send`, `try_receive` und `receive`,
- hostseitige Recording- und Replay-Commands,
- History und sessionweite Events,
- Report-Auslösung aus SR1,
- Shutdown, Drop und unerwartetes Ende,
- die aus R5 benötigten Session-Daten.

Dieser Schritt trifft noch keine neuen Entscheidungen. Er verhindert, dass die pauschale offene
"Session-Struktur" stillschweigend als abgeschlossen behandelt wird.

### S2: `session::Session` und seine direkten Module abschließen

- **Art:** Abschluss
- **Owner:** `session`
- **Benötigt:** S1 und alle dort erfassten Entscheidungen

Abzuschließen sind `session::Config`, `session::launch`, `Capabilities`, `RequestId`, `Pending`,
`history`, `Event`, `Session` und `session::Error`. Jede Signatur in `goal.rs` wird gegen die Regeln
in `migration.md` geprüft. Danach dürfen Host-Module nur noch dieses Interface verwenden und keine
eigenen Session-Regeln ergänzen.

Die interne Datenstruktur für laufende Command-Arbeit im Bevy-Event-Loop wird erst bei der
Implementation entworfen. Sie ist kein offener Teil des öffentlichen Ziel-Interfaces, solange sie
die beschlossenen Fairness-, Reihenfolge- und World-Zugriffsregeln erfüllt.

## Stufe 5: Controller auf dem fertigen Session-Interface abschließen

### H1: komplexe Inspect-Eingaben der REPL

- **Art:** Entscheidung
- **Owner:** `host::repl`
- **Benötigt:** I5, P1, S2

Die Textsyntax für komplexe Inspect-Queries wird auf die fertige gemeinsame Command-Form
abgebildet. Sie darf keine zweite fachliche Query-Darstellung erzeugen.

### H2: ungültige Agent-Eingaben

- **Art:** Entscheidung
- **Owner:** `host::agent`
- **Benötigt:** P1, S2

Festzulegen ist das Verhalten für ungültige JSONL-Zeilen und syntaktisch gültige Einträge, die nicht
als Command dekodiert werden können. Zu bestimmen sind Ausgabeform, Fortsetzung nach dem Fehler und
die Frage, ob für nie angenommene Eingaben eine Request-ID existiert.

### H3: Session-Events im Agent-Modus

- **Art:** Entscheidung
- **Owner:** `host::agent`
- **Benötigt:** S2

Festzulegen ist, wie nicht fatale Session-Events und ein unerwartetes Session-Ende in der
maschinenlesbaren Ausgabe erscheinen. Die Darstellung muss von terminalen Command-Outcomes
unterscheidbar sein.

### H4: EOF und kontrollierter Agent-Abschluss

- **Art:** Entscheidung
- **Owner:** `host::agent`
- **Benötigt:** H2, H3

Festzulegen ist das Verhalten bei EOF mit ausstehenden Commands, laufendem Replay oder aktivem
Recording. Der Agent darf keine versteckten Stop-Commands senden und kein Recording stillschweigend
beenden. Danach kann das `host::agent::Exit`- und Fehler-Interface abgeschlossen werden.

### H5: konkrete Capability-Anforderungen der Host-Abläufe

- **Art:** Entscheidung
- **Owner:** privates `host::run`-Modul
- **Benötigt:** H1, H4 und das bereits skizzierte `host::script`

Für REPL, Agent und Script ist festzulegen, wie der Host ihre benötigten Capabilities vor dem Start
des Ablaufs bestimmt. Die Darstellung bleibt privat, muss aber aus den tatsächlich verwendeten
Commands ableitbar sein und darf keine zweite Capability-Definition neben
`session::Capabilities` schaffen.

## Stufe 6: Host als oberste Schicht abschließen

### H6: verbleibende Host-Fragen ausdrücklich erfassen

- **Art:** Bestandsaufnahme
- **Owner:** `host`
- **Benötigt:** H5

`README.md` nennt die übrige Host-Struktur pauschal als offen. Wie bei S1 werden zuerst konkrete
`OPEN`-Punkte erfasst, statt fehlende Details anzunehmen. Geprüft werden:

- das private versionierte TOML-Format,
- die Auswahl von REPL, Agent, Script und reinem Report-Aufruf,
- CLI-Syntax und erlaubter Artifact-Override,
- Dateizugriffe für Config, Script, Recording, Replay und Reports,
- Fehlerdarstellung und Exit-Code-Zuordnung,
- Capability-Prüfung,
- sauberer Shutdown nach jedem Ablauf.

### H7: `host::run` und die Host-Fassade abschließen

- **Art:** Abschluss
- **Owner:** `host`
- **Benötigt:** H6 und alle dort erfassten Entscheidungen

Zum Schluss wird der gesamte Ablauf von `host::run(config_source)` geprüft. Mit dem Feature `host`
bleibt `host::run` der einzige öffentliche Export. Ohne das Feature existiert `host` nicht. Private
CLI-, Config-, Controller- und Orchestrierungstypen dürfen nicht nach außen gelangen.

## Kurzform der Kausalkette

```text
I1 + I2 + I3 + I4 -> I5 -> P1 ----------------------+
T1 -----------------------> P1                      |
R1 + R2 + R3 -> R4 --+                              |
R5 ------------------+-> R6 -> R7 -> R8 -> RP1 -> RP2
                                  |                  |
                                  +-> SR1 -----------+
R5 -------------------------------------------------+-> S1 -> S2
                                                         |
I5 + P1 + S2 -> H1                                      |
P1 + S2 -> H2 -> H4                                     |
S2 -------> H3 -> H4                                    |
H1 + H4 -> H5 -> H6 -> H7 <-----------------------------+
```

## Nicht als vorgelagerte API-Entscheidung behandeln

Folgende Punkte bleiben bewusst bei der Implementation und blockieren die Reihenfolge nicht:

- die konkrete Datenstruktur für mehrere laufende Commands im Bevy-Event-Loop,
- private In-Memory-Ein-/Ausgabeadapter für Tests,
- private Betriebssystem- und Prozessfehlerdetails innerhalb der festgelegten Fehlergruppen,
- private Hilfsfunktionen, genaue Validierungsreihenfolgen und interne Scheduler-Budgets.

Wenn einer dieser Punkte doch eine öffentliche Signatur, einen Fehlerfall oder eine garantierte
Reihenfolge verändert, wird er vor der Implementation als neuer `OPEN`-Punkt an der zuständigen
Stelle aufgenommen.
