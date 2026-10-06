# Panic-Beobachtbarkeit in Bevy 0.19.1

## Fragestellung

R1 prüft, welche Panics ein nativer Debug Host bei einer als Kindprozess gestarteten Controlled
Session beobachten kann. Untersucht wurden Panics in Bevy-Systemen, Bevy-Tasks und gewöhnlichen
Rust-Worker-Threads mit `panic = "unwind"` und `panic = "abort"`.

Web- und Wasm-Ausführungen gehören nicht zu dieser Prüfung. Das geplante Session-Modell startet
einen nativen Kindprozess und beobachtet dessen getrennte stdout-, stderr- und Prozesskanäle.

## Quellen

- Rust ruft den [prozessweiten Panic-Hook](https://doc.rust-lang.org/std/panic/fn.set_hook.html)
  vor der Panic-Runtime auf. Das gilt für `unwind` und `abort`. Der Standard-Hook schreibt eine
  menschenlesbare Meldung auf stderr.
- [`catch_unwind`](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html) fängt nur
  Unwind-Panics. Ein installierter Hook läuft trotzdem vor dem Fangen.
- Bevys
  [`PanicHandlerPlugin`](https://docs.rs/bevy_app/0.19.1/src/bevy_app/panic_handler.rs.html#40-62)
  verwaltet ebenfalls den globalen Hook. Auf nativen Plattformen ist sein Verhalten
  featureabhängig. Es ist kein Ereigniskanal zum Debug Host.
- Der
  [Multi-Threaded-Schedule-Executor](https://docs.rs/bevy_ecs/0.19.1/src/bevy_ecs/schedule/executor/multi_threaded.rs.html)
  fängt einen System-Panic auf dem ausführenden Thread und setzt das Unwinding nach dem Abschluss
  der Schedule-Verwaltung fort.
- Ein
  [Bevy-TaskPool](https://docs.rs/bevy_tasks/0.19.1/src/bevy_tasks/task_pool.rs.html)
  fängt Task-Panics, damit seine Worker weiterlaufen. Ein Scoped Task gibt den Panic an den Scope
  zurück. Ein normaler Task gibt ihn beim Await weiter.

## Verifiziertes Verhalten

Kleine Prozess-Fixtures wurden mit Bevy 0.19.1 auf `aarch64-apple-darwin` ausgeführt. Die
Exit-Statuswerte sind plattformspezifisch. Entscheidend ist, ob der Prozess endet oder weiterläuft.

| Ausführungsort | `panic = "unwind"` | Beobachtung |
| --- | --- | --- |
| Bevy-System im Multi-Threaded Executor | Prozess endet, im Test mit Status 101 | Der Hook läuft auf dem TaskPool-Thread. Bevy setzt den Panic anschließend auf dem Schedule-Aufrufer fort. |
| Gewarteter Bevy-Task | Prozess endet, im Test mit Status 101 | Der Hook läuft beim ersten Panic. Das Await setzt den gespeicherten Panic fort. |
| Scoped Bevy-Task | Prozess endet, im Test mit Status 101 | Der Scope sammelt den Panic ein und setzt ihn beim Verlassen des Scopes fort. |
| Detached Bevy-Task | Prozess läuft weiter und endet im Test mit Status 0 | Der Hook läuft, aber der TaskPool fängt den Panic und setzt seine Ausführung fort. |
| Nicht gejointer `std::thread` | Nur der Worker endet, der Prozess endet im Test mit Status 0 | Der Hook läuft. Ohne Join oder ausdrückliches Weiterreichen erreicht der Panic den Hauptthread nicht. |

Mit `panic = "abort"` liefen dieselben Hooks. Ein Panic im Bevy-System, im detached Task und im
gewöhnlichen Worker brach danach jeweils den gesamten Prozess ab. Der Testhost meldete Status 134
und `SIGABRT`. Andere Betriebssysteme dürfen dafür einen anderen Prozessstatus liefern.

## Was der Host ohne eigenen Marker erkennen kann

Der Prozessstatus reicht nur für Panics, die den Prozess beenden. Er erkennt keinen detached
Task-Panic und keinen Panic eines nicht gejointen Worker-Threads unter `panic = "unwind"`.

Das Parsen der normalen stderr-Ausgabe schließt diese Lücke nicht zuverlässig. Die Meldung ist für
Menschen formatiert und darf sich mit Rust-Version, Hook und Anwendungskonfiguration ändern. Eine
Anwendung kann den globalen Hook ersetzen oder ganz ohne Ausgabe arbeiten. Dann kann ein
Unwind-Panic vollständig gefangen werden, während der Host weder einen fehlgeschlagenen
Prozessstatus noch eine Panic-Meldung sieht.

## Folgerung für die Zielarchitektur

Eine verlässliche Panic-Erkennung an allen drei untersuchten Ausführungsorten benötigt einen Hook
in der Controlled Session:

1. `session::Plugin` installiert einen prozessweiten Panic-Hook und bewahrt den vorherigen Hook auf.
2. Der Hook schreibt vor dem Aufruf des vorherigen Hooks synchron einen maschinenlesbaren
   Panic-Marker auf stderr.
3. Der Marker enthält mindestens die bestmöglich lesbare Payload und die vom Hook gelieferte
   Code-Stelle. Ein unbekannter `panic_any`-Payload bleibt als Panic ohne erfundene Meldung sichtbar.
4. Der Hook darf bei einem Schreibfehler nicht selbst paniken. Für `panic = "abort"` darf er die
   Ausgabe nicht erst an einen Hintergrundthread übergeben, weil der Prozess unmittelbar danach
   endet.
5. Der Debug Host behandelt den Marker auch dann als Panic, wenn der Prozess später mit Status 0
   endet. Ein fehlgeschlagener Prozessstatus ohne Marker bleibt ein `ProcessExit`.

Der vorherige Hook bleibt verkettet, damit die Anwendung ihre gewohnte Diagnoseausgabe behält.
Ersetzt Anwendungscode den Hook nach der Installation erneut, geht die Garantie verloren. Die
unterstützte Zusammensetzung darf den von `session::Plugin` installierten Hook deshalb nicht
nachträglich ersetzen.

R1 legt noch weder das genaue Markerformat noch die Backtrace-Erfassung fest. R2 prüft den
Backtrace. [ADR-0009](../../adr/0009-session-transports-report-observations.md) legt fest, wie
Session und Report die beobachteten Marker austauschen.

## Benötigte Implementations-Fixtures

Die Implementation übernimmt die untersuchten Fälle als Kindprozess-Fixtures:

- System-Panic mit `unwind`,
- gewarteter, scoped und detached Bevy-Task mit `unwind`,
- nicht gejointer Rust-Worker mit `unwind`,
- mindestens ein System- und ein Hintergrund-Panic mit `abort`,
- ein eigener vorheriger Hook, der nach dem Marker weiterhin aufgerufen wird,
- ein Panic-Marker gefolgt von Prozessstatus 0,
- ein fehlgeschlagener Prozessstatus ohne Panic-Marker.
