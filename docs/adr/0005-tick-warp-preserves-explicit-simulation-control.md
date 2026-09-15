# Tick-Warp erhält die explizite Simulationssteuerung

Historischer Entscheidungsstand. Für den Rewrite gilt [target.md](../api/target.md).

## Status

Angenommen.

## Kontext

ADR-0003 führte `Step { frames, step_nanoseconds }` als einzigen Zeit-Command ein. Dadurch
verschwanden versteckte Simulationsschritte aus Controller-Aktionen, Wartefunktionen und Virtual
Input. Der Host bestimmte dabei weiterhin die simulierte Dauer jedes Steps.

Das Ziel-Interface ersetzt Step durch einen begrenzten Tick-Warp. Die Anwendung soll selbst
bestimmen, was ein Simulationstick für ihre Schedules und ihre simulierte Zeit bedeutet.
`bug_hunter` kontrolliert nur, wann und wie schnell in realer Zeit diese Ticks ausgeführt werden.

## Entscheidung

1. Ticks bleiben explizit. Nur `command::tick` führt Simulationsticks aus. Input, Inspect,
   Screenshot, Recording, Replay und der normale Bevy-Event-Loop führen keine versteckten
   Simulationsticks aus.
2. `warp::Command::Start { ticks, pace }` ersetzt
   `Step { frames, step_nanoseconds }`. `ticks` ist größer als null und hat neben dem Wertebereich
   von `u64` kein zusätzliches fachliches Maximum. Ein Warp mit einem Tick übernimmt den
   Anwendungsfall eines einzelnen Steps.
3. Ein Tick führt die von der Anwendung konfigurierten Simulations-Schedules genau einmal in ihrer
   bestehenden Reihenfolge aus. Die Anwendung besitzt die Bedeutung des Ticks und ihre
   Zeitkonfiguration. `bug_hunter` übergibt keine simulierte Tick-Dauer und überschreibt diese
   Konfiguration nicht.
4. `Pace::AsFastAsPossible` und `Pace::TicksPerSecond { target }` steuern ausschließlich die reale
   Ausführungsgeschwindigkeit. Die Pace verändert weder die simulierte Zeit noch die Zahl der
   angeforderten Ticks.
5. Die Response von `Start` bleibt bis zum natürlichen Abschluss oder einem `Stop` ausstehend.
   Währenddessen verarbeitet die Session `SetPace`, `Stop` und andere Commands. Ein zweiter `Start`
   wird abgelehnt.
6. Auch `AsFastAsPossible` erhält pro Event-Loop-Durchlauf ein begrenztes Arbeitsbudget. Danach
   verarbeitet die Session erneut eingegangene Commands. Die Budgetgröße ist kein öffentlicher
   Konfigurations- oder Protokollwert.
7. Virtual Input bleibt bis zum nächsten ausgeführten Tick gesammelt. Mehrere Input-Commands vor
   diesem Tick werden gemeinsam für ihn bereitgestellt. Bei einem Warp mit mehreren Ticks betrifft
   das den ersten ausgeführten Tick.
8. Eingebaute Wait-Funktionen bleiben entfernt. Ein Controller darf Inspect und explizite
   Tick-Commands kombinieren, aber `bug_hunter` führt keine verborgene Polling-Schleife mit eigenen
   Ticks ein.

## Verhältnis zu ADR-0003

Diese ADR ersetzt folgende Teile von ADR-0003:

- `step` als einzigen öffentlichen Zeit-Command,
- `Step { frames, step_nanoseconds }`,
- die vom Host vorgegebene simulierte Dauer eines Steps,
- `frame_nanoseconds` und `startup_frames` als Steuerung des Ziel-Interfaces,
- die direkte Zuordnung eines Clock-Zustands zu einzelnen Step-Aufrufen.

Folgende Regeln von ADR-0003 bleiben bestehen:

- keine versteckten Ticks,
- eine aus dem Command-Verlauf erkennbare Tick-Ausführung,
- gesammelter Virtual Input bis zum nächsten Tick,
- kein eingebautes `wait`,
- keine Dummy-Eingabe zum Ausführen eines Ticks.

Diese ADR ersetzt ADR-0003.

## Folgen

- Die Anwendung behält ihre eigene Tick- und Zeitsemantik. `bug_hunter` koppelt sie nicht an eine
  Host-Konfiguration.
- Derselbe begrenzte Warp führt unabhängig von seiner Pace dieselbe Anzahl Ticks aus. Gleicher
  fachlicher Zustand setzt weiterhin einen gleichen Ausgangszustand und deterministische
  Anwendungslogik voraus.
- Große Warps bleiben abbrechbar, und die Session bleibt während ihrer Ausführung ansprechbar.
- Wire-Format, Recordings, Replay, REPL, Beispiele und Tests müssen von Step auf Tick-Warp
  umgestellt werden.
- Der Wechsel ist ein Breaking Change. Alte Step-Commands und Aufzeichnungen benötigen einen
  versionsbewussten Adapter oder werden als nicht unterstützte Formatversion abgelehnt.

## Prüfung

- Nur `Warp::Start` führt Ticks aus.
- Input, Inspect, Screenshot, Recording und Replay führen keinen Tick aus.
- `Start { ticks: 1, .. }` führt genau einen Tick aus.
- `Start` meldet angeforderte und tatsächlich ausgeführte Ticks.
- `SetPace` und `Stop` werden während eines laufenden Warp verarbeitet.
- Ein zweiter `Start` wird abgelehnt.
- Virtual Input wird beim nächsten Tick und nicht schon bei seiner Annahme bereitgestellt.
- Unterschiedliche Pace-Werte verändern bei gleichem Ausgangszustand nicht die Zahl ausgeführter
  Ticks.
