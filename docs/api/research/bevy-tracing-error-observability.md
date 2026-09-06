# Beobachtbare `tracing`-Errors in Bevy 0.19.1

## Fragestellung

R3 prüft, wie der Debug Host ein Error-Level-Event von beliebigem Text auf stderr unterscheidet.
Untersucht wurden Bevys `LogPlugin`, eigene Formatter, ANSI-Ausgabe, mehrzeilige Events und Events
aus der `log`-Fassade.

Die Prüfung gilt für native Controlled Sessions. Die Anwendung verwendet Bevys globalen
`tracing`-Subscriber. Web-, Wasm-, Android- und iOS-Ausgaben gehören nicht zu dieser Recherche.

## Quellen

- [`bevy_log`](https://docs.rs/bevy_log/0.19.1/src/bevy_log/lib.rs.html#7-17) exportiert die
  Logging-Makros aus `tracing`. `LogPlugin` gehört zu `DefaultPlugins`.
- [`LogPlugin::custom_layer`](https://docs.rs/bevy_log/0.19.1/src/bevy_log/lib.rs.html#226-249)
  fügt einen eigenen `tracing_subscriber::Layer` vor dem Formatter ein. `fmt_layer` kann den
  Standardformatter dagegen vollständig ersetzen.
- Die
  [native Subscriber-Zusammensetzung](https://docs.rs/bevy_log/0.19.1/src/bevy_log/lib.rs.html#309-389)
  verbindet den Custom Layer, den `EnvFilter` und den Formatter. Der Standardformatter schreibt auf
  stderr. `LogTracer` leitet Events aus der `log`-Fassade an `tracing` weiter.
- Ein globaler
  [`tracing`-Subscriber](https://docs.rs/tracing/0.1.41/tracing/subscriber/fn.set_global_default.html)
  kann nur einmal gesetzt werden. Ein später hinzugefügtes Bevy-Plugin kann einen bestehenden
  Subscriber nicht um einen weiteren Layer ergänzen.
- Ein [`tracing::Event`](https://docs.rs/tracing/0.1.41/tracing/struct.Event.html) besitzt Metadaten
  und strukturierte Felder. Ein Event muss kein Textfeld `message` enthalten.
- [`Metadata`](https://docs.rs/tracing-core/0.1.33/tracing_core/struct.Metadata.html) liefert Level,
  Target und Namen. Quelldatei, Zeile und Modulpfad sind optional.
- Ein [`Visit`](https://docs.rs/tracing-core/0.1.33/tracing_core/field/trait.Visit.html) liest die
  Feldwerte während `Layer::on_event`. Nicht eigens unterstützte Werttypen sind nur über ihre
  `Debug`-Darstellung verfügbar.
- [`tracing_log::NormalizeEvent`](https://docs.rs/tracing-log/0.2.0/tracing_log/trait.NormalizeEvent.html)
  stellt für weitergeleitete `log`-Events das ursprüngliche Target und die ursprüngliche Quellstelle
  wieder her.

## Warum stderr-Parsing nicht trägt

Eine Prozess-Fixture verwendete Bevy 0.19.1 und gab dasselbe Error-Event mit Standardformatter,
einem einfachen Formatter und einem ANSI-Formatter aus. Die Meldung enthielt einen Zeilenumbruch und
strukturierte Felder. Zusätzlich schrieb die Fixture einen ähnlich aussehenden Text direkt auf
stderr.

| Ausgabe | Beobachtetes Textverhalten |
| --- | --- |
| Bevy-Standardformatter | Zeitstempel, Level und Target stehen nur auf der ersten Zeile. Die zweite Meldungszeile sieht wie beliebiger stderr-Text aus. |
| eigener einfacher Formatter | Zeitstempel und Target fehlten. Der Formatter schrieb in der Fixture auf stdout statt stderr. |
| eigener ANSI-Formatter | Level und Feldnamen enthielten ANSI-Steuersequenzen. Die Meldung blieb mehrzeilig. |
| direktes `eprintln!` | Der Text konnte Präfix, Level und Target des Formatters nachbilden, ohne ein `tracing`-Event zu sein. |

Ein Parser kann deshalb weder den Anfang und das Ende eines Events noch dessen Level verlässlich aus
dem Text zurückgewinnen. ANSI-Bereinigung löst nur die Steuersequenzen. Sie beweist nicht, dass der
Text von `tracing` stammt. Ein eigener Formatter kann außerdem Felder, Target und Level weglassen
oder die Ausgabe vollständig unterdrücken.

Die heutige Suche nach Zeilen, die mit `ERROR` beginnen, darf nicht in die Zielarchitektur
übernommen werden. Beliebiger stderr-Text löst keinen Tracing-Report aus.

## Belastbare Ereignisgrenze

Die Erkennung erfolgt in einem eigenen `tracing_subscriber::Layer`. `Layer::on_event` erhält genau
ein Event vor dessen Textformatierung. Der Layer verarbeitet nur Events mit
`metadata.level() == Level::ERROR`.

Damit gelten folgende Regeln:

- Ein mehrzeiliges `message`-Feld bleibt ein einzelnes Event.
- Vom Formatter ergänzte ANSI-Sequenzen, Zeitstempel und Präfixe erreichen den Layer nicht.
- Ein direktes `eprintln!` erreicht den Layer ebenfalls nicht.
- Ein `error_span!` ohne Error-Event ist nur ein Span und löst keinen Report aus.
- Ein durch Filterung oder Compile-Time-Filter deaktiviertes Event wird nicht dispatcht und ist
  nicht beobachtbar. `report.tracing_errors` verändert die Filter der Anwendung nicht.

Die letzte Grenze ist beabsichtigt. Reporting beobachtet ausgeführte Events. Es schaltet keine von
der Anwendung deaktivierte Diagnose frei und verändert damit auch nicht deren Laufzeitverhalten.

## Zusammensetzung mit Bevys `LogPlugin`

Ein Layer lässt sich nicht nachträglich in den bereits global installierten Subscriber einfügen.
`session::Plugin` allein kann die Erfassung deshalb nicht zuverlässig aktivieren, wenn
`DefaultPlugins` den `LogPlugin` schon aufgebaut hat.

Das Ziel stellt die Funktion `session::tracing_error_layer` bereit. Die Anwendung trägt sie als
`LogPlugin::custom_layer` ein und fügt `session::Plugin` wie bisher als Bevy-Plugin hinzu:

```rust
App::new()
    .add_plugins(DefaultPlugins.set(LogPlugin {
        custom_layer: bug_hunter::session::tracing_error_layer,
        ..Default::default()
    }))
    .add_plugins(bug_hunter::session::Plugin::default());
```

Ein eigener `fmt_layer` bleibt damit kompatibel, weil Bevy Custom Layer und Formatter getrennt
zusammensetzt. Verwendet die Anwendung schon einen anderen `custom_layer`, muss ihre gemeinsame
Layer-Funktion beide Layer zurückgeben. Deaktiviert sie Bevys `LogPlugin` zugunsten eines eigenen
Subscribers, muss sie den Error-Layer selbst in diesen Subscriber aufnehmen. Das automatische
Nachrüsten eines fremden globalen Subscribers ist nicht möglich.

Diese Zusammensetzung ist nur nötig, wenn `report.tracing_errors` verwendet werden soll. Panic- und
Prozessfehler bleiben davon unabhängig. Die Anwendung benötigt weiterhin keine reportspezifischen
Events, Fehlercodes oder Meldeaufrufe.

Der Error-Layer bestätigt seine Registrierung durch einen internen Startmarker. Wenn
`report.tracing_errors = true` gesetzt ist, akzeptiert der Debug Host die Session erst, nachdem er
diesen Marker erhalten hat. Das genaue Markerformat und die Zuordnung zu `Session::start` stehen in
[ADR-0009](../../adr/0009-session-transports-report-observations.md).

## Erfasste Event-Daten

Der Layer übermittelt vor der Formatter-Ausgabe:

- das Level,
- normalisiertes Target, Namen und optionale Quellstelle,
- alle Event-Felder in ihrer vom `Visit` beobachteten Reihenfolge,
- den Wert des konventionellen Feldes `message` getrennt von den übrigen Feldern, sofern es
  vorhanden ist.

Für ein weitergeleitetes `log::error!` verwendet der Layer `NormalizeEvent`. Ohne diese
Normalisierung wäre das Metadaten-Target immer `log`; das ursprüngliche Target stünde nur im Feld
`log.target`.

Primitive Feldwerte bleiben typisiert, solange `Visit` dafür eine Methode anbietet. Andere Werte
erhält der Layer nur über `record_debug`. R3 verspricht keine stabile Serialisierung beliebiger
benutzerdefinierter `Debug`-Werte.

Besitzt ein Error-Event kein `message`-Feld, bleibt es trotzdem ein beobachteter Error. Der Marker
bewahrt seine strukturierten Felder. R4 verwendet ihre kanonische Zusammenfassung als
Report-Meldung und damit als Signatur-Eingabe. Bei einem vorhandenen `message`-Feld bleiben
zusätzliche strukturierte Werte aus der Signatur ausgeschlossen.

Literal im Event enthaltene ANSI-Steuersequenzen sind Nutzdaten und bleiben erhalten. Nur vom
Formatter ergänzte Sequenzen fehlen erwartungsgemäß. Der Report darf die ursprünglichen Felddaten
nicht anhand der formatierten Konsolenausgabe ersetzen.

## Verifiziertes Verhalten

Die Fixture bestätigte:

- Standard-, eigener und ANSI-Formatter änderten die vom Error-Layer erfassten Metadaten und Felder
  nicht.
- Ein `message`-Feld mit Zeilenumbruch kam als ein Feldwert bei genau einem `on_event`-Aufruf an.
- Ein Event ohne `message` blieb mit seinen strukturierten Feldern sichtbar.
- `log::error!` erreichte den Layer über Bevys `LogTracer`. Das ursprüngliche Target war als
  `log.target` vorhanden.
- Ein direkt geschriebenes, täuschend ähnlich formatiertes `ERROR` auf stderr erreichte den Layer
  nicht.
- Ein per `RUST_LOG` deaktiviertes Target erreichte weder den Formatter noch den Error-Layer.

## Benötigte Implementations-Fixtures

Die Implementation übernimmt folgende Fälle:

- `tracing::error!` mit und ohne `message`,
- ein mehrzeiliges `message`-Feld,
- strukturierte String-, Zahlen-, Bool-, Error- und Debug-Felder,
- abweichende Formatter mit und ohne ANSI,
- ein Formatter ohne Textausgabe,
- ein direktes `eprintln!`, das eine Error-Zeile nachbildet,
- ein durch Laufzeitfilter deaktiviertes Error-Event,
- ein durch Compile-Time-Filter deaktiviertes Error-Event,
- ein über Bevys `LogTracer` weitergeleitetes `log::error!`,
- ein eigener Subscriber mit ausdrücklich eingebautem Error-Layer,
- fehlende Layer-Registrierung bei aktiviertem `report.tracing_errors`,
- zwei gleichzeitige Error-Events mit getrennt dekodierbaren Markern.
