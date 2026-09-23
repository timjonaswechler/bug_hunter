# Backtrace-Erfassung mit stabilem Rust

## Fragestellung

R2 prüft die Backtrace-Erfassung für native Controlled Sessions. Der Debug Host startet die
Anwendung über `cargo run`. `session::Plugin` beobachtet Panics über den in R1 festgelegten
prozessweiten Hook.

Zu klären sind drei Punkte:

1. Wie fordert der Debug Host den Backtrace vor dem Prozessstart an?
2. Welche Daten kann der Panic-Hook verlässlich an den Host übermitteln?
3. Welche Backtrace-Daten bleiben zwischen Builds stabil genug für eine Fehlersignatur?

## Quellen

- [`std::backtrace`](https://doc.rust-lang.org/std/backtrace/index.html) beschreibt
  `Backtrace::capture`, `Backtrace::force_capture`, die Umgebungsvariablen und die Grenzen der
  Genauigkeit. Adressen, Symbole, Dateinamen und Zeilenangaben sind nur bestmögliche Ergebnisse.
- [`Backtrace`](https://doc.rust-lang.org/std/backtrace/struct.Backtrace.html) stellt auf stabilem
  Rust den Erfassungsstatus sowie `Display` und `Debug` bereit. Der Zugriff auf einzelne Frames über
  `Backtrace::frames` ist weiterhin instabil.
- [`PanicHookInfo`](https://doc.rust-lang.org/std/panic/struct.PanicHookInfo.html) liefert Payload und
  Panic-Stelle getrennt vom Backtrace. `location` gibt einen optionalen Dateinamen sowie Zeile und
  Spalte zurück.
- Das
  [Rust-Buch zu Panic-Backtraces](https://doc.rust-lang.org/book/ch09-01-unrecoverable-errors-with-panic.html)
  beschreibt `RUST_BACKTRACE=1` als gekürzte und `RUST_BACKTRACE=full` als ausführliche Ausgabe.
  Die genaue Ausgabe hängt von Betriebssystem und Rust-Version ab.
- Die
  [Cargo-Profile](https://doc.rust-lang.org/cargo/reference/profiles.html#debug) bestimmen über
  `debug`, ob ein Binary die für Dateinamen und Zeilen nötigen Debug-Informationen enthält. Das
  `dev`-Profil enthält sie standardmäßig. Das `release`-Profil enthält sie standardmäßig nicht.

## Verifiziertes Verhalten

Kleine Kindprozess-Fixtures wurden mit stabilem Rust 1.97.1 auf `aarch64-apple-darwin` ausgeführt.
Der Panic-Hook erfasste jeweils einen Backtrace mit `Backtrace::capture` und einen mit
`Backtrace::force_capture`. Die Fixtures deckten `panic = "unwind"` und `panic = "abort"` ab.

| Startumgebung | `Backtrace::capture` | `Backtrace::force_capture` | vorheriger Standard-Hook |
| --- | --- | --- | --- |
| Variable fehlt | `Disabled` | `Captured` | Meldung ohne Backtrace |
| `RUST_BACKTRACE=0` | `Disabled` | `Captured` | Meldung ohne Backtrace |
| `RUST_BACKTRACE=1` | `Captured` | `Captured` | gekürzter Backtrace |
| `RUST_BACKTRACE=full` | `Captured` | `Captured` | ausführlicher Backtrace mit Adressen und weiteren Runtime-Frames |

Bei `panic = "abort"` konnte der Hook den mit `force_capture` angeforderten Backtrace noch
formatieren und ausgeben, bevor der Prozess abbrach. Das beweist keine Unterstützung auf jeder
Plattform. `BacktraceStatus::Unsupported` bleibt ein erlaubtes Ergebnis.

Eine zweite Fixture verglich optimierte Builds ohne Debug-Informationen, mit
`line-tables-only` und mit vollständigen Debug-Informationen:

- `PanicHookInfo::location` enthielt in allen drei Builds Datei, Zeile und Spalte.
- Ohne Debug-Informationen enthielt der Backtrace in dieser Fixture noch Funktionssymbole, aber
  keine Quellstellen für die Anwendungsframes. Andere Toolchains dürfen auch die Symbole weglassen.
- Mit `line-tables-only` waren Quellstellen vorhanden.
- Optimierung, Inlining und fehlende Unwind-Informationen dürfen Frames entfernen oder verändern.

`RUST_LIB_BACKTRACE` kann `Backtrace::capture` unabhängig von `RUST_BACKTRACE` deaktivieren. Außerdem
merkt sich die Standardbibliothek die Umgebungsentscheidung bei der ersten Erfassung. Eine Änderung
der Variablen innerhalb des bereits laufenden Prozesses ist deshalb keine belastbare Aktivierung.

## Festlegung für den Session-Start

Der Debug Host setzt für den gestarteten Cargo-Prozess immer `RUST_BACKTRACE=1`. Cargo vererbt den
Wert an die Controlled Session. Der Host übernimmt weder einen fehlenden, `0` gesetzten noch einen
auf `full` gesetzten Wert aus seiner eigenen Umgebung.

Diese feste Einstellung hat zwei Gründe:

- Der verkettete vorherige Hook kann eine übliche, gekürzte Backtrace-Ausgabe für Menschen
  schreiben.
- Wiederholte Sessions erhalten dieselbe Einstellung. `full` würde vor allem Adressen und
  Runtime-Frames ergänzen, die für den Report nicht stabil sind.

Das Feld gehört nicht in `session::launch::Config`. Es ist Teil der internen
Prozessbeobachtung und keine frei wählbare Anwendungseinstellung.

## Festlegung für den Panic-Marker

`session::Plugin` ruft im Panic-Hook `Backtrace::force_capture` auf dem panikenden Thread auf. Damit
hängt der maschinenlesbare Panic-Marker weder von `RUST_LIB_BACKTRACE` noch vom Verhalten des
vorherigen Hooks ab.

Der Marker übermittelt:

- den `BacktraceStatus`,
- bei `Captured` die unveränderte `Display`-Ausgabe als String,
- bei `Disabled` oder `Unsupported` keinen erfundenen Backtrace.

Payload und `PanicHookInfo::location` bleiben getrennte Markerfelder. Der Host gewinnt sie nicht
durch das Parsen des formatierten Backtraces. Der vorherige Hook läuft nach dem Marker weiter. Seine
stderr-Ausgabe dient Menschen und wird nicht als Quelle des Report-Backtraces geparst.

Der formatierte Backtrace ist mehrzeilig und kann groß werden.
[ADR-0009](../../adr/0009-session-transports-report-observations.md) legt deshalb ein
Marker-Framing fest, das Zeilenumbrüche erhält und gleichzeitige Panics nicht vermischt. R2 legt
dieses Transportformat nicht fest.

## Stabilität zwischen Builds

Die Standardbibliothek garantiert für keinen einzelnen Backtrace-Frame genaue oder
buildübergreifend stabile Werte. Auf stabilem Rust fehlt außerdem ein strukturiertes Frame-Interface.
Die `Display`-Ausgabe zu parsen würde ein nicht zugesichertes Textformat zum internen Protokoll
machen.

| Bestandteil | Eignung für Diagnose | Eignung für eine buildübergreifende Signatur |
| --- | --- | --- |
| vollständige rohe `Display`-Ausgabe | ja | nein |
| Frame-Nummer | nur zur Lesbarkeit innerhalb einer Ausgabe | nein |
| Instruktionsadresse | gelegentlich zur lokalen Diagnose | nein |
| Rust- und Crate-Hash im Symbol | gelegentlich zur lokalen Diagnose | nein |
| Funktionssymbol | ja, wenn vorhanden | nicht garantiert |
| absolute Quelldatei | ja, wenn vorhanden | nein |
| zum Projekt relative Quelldatei eines Backtrace-Frames | ja, wenn ableitbar | nein in Signaturversion 1 |
| Zeile und Spalte eines Frames | ja, wenn vorhanden | ändern sich bei Quelltextänderungen |
| Frames aus Rust, Bevy und Task-Executoren | selten für die Fehleridentität nötig | nein |

Die getrennte Panic-Stelle aus `PanicHookInfo::location` ist belastbarer als ein geparster
Backtrace-Frame. R4 verwendet die Stelle als Diagnose, schließt sie aber aus der Signatur aus.

R4 schließt Backtrace-Frames aus, weil der rohe Backtrace keine garantierten stabilen Teile besitzt.
Der Report bewahrt ihn trotzdem unverändert als Diagnose auf.

## Geprüfte Alternative

Die externe Crate [`backtrace`](https://docs.rs/backtrace/latest/backtrace/) bietet strukturierten
Zugriff auf Frames und Symbole. Auch sie arbeitet nur bestmöglich und nennt fehlende
Unwind-Informationen, fehlende Debug-Informationen und nicht unterstützte Plattformen als Grenzen.

Für das festgelegte Report-Feld `Option<String>` bringt diese zusätzliche Abhängigkeit derzeit
keinen verlässlicheren buildübergreifenden Wert. Die Planung verwendet deshalb zunächst
`std::backtrace`. Die Signatur nimmt keine einzelnen Frames auf. Das Parsen der
`std::backtrace`-Textausgabe ist kein akzeptabler Ersatz für ein strukturiertes Interface.

## Benötigte Implementations-Fixtures

Die Implementation übernimmt folgende Fälle als Kindprozess-Fixtures:

- Session-Start überschreibt geerbte Werte `0` und `full` mit `RUST_BACKTRACE=1`,
- ein Panic mit geerbtem `RUST_LIB_BACKTRACE=0` enthält wegen `force_capture` trotzdem einen
  Backtrace, sofern die Plattform ihn unterstützt,
- `unwind` und `abort` schreiben Marker, Status und Backtrace vor dem jeweiligen Prozessausgang,
- `BacktraceStatus::Unsupported` erzeugt einen Panic ohne Backtrace,
- fehlende Debug-Informationen verhindern weder Panic-Erkennung noch Report-Erzeugung,
- der vorherige Hook läuft weiter, seine formatierte Ausgabe wird aber nicht als Marker geparst,
- ein mehrzeiliger Backtrace bleibt genau einem Panic-Marker zugeordnet,
- zwei nahezu gleichzeitige Panics erzeugen zwei getrennt dekodierbare Marker.
