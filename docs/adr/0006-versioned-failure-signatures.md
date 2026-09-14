# Fehlerart und bereinigte Meldung bestimmen die Duplikatidentität

## Status

Angenommen.

## Kontext

Lokale und Remote-Provider sollen denselben wiederholt beobachteten Fehler wiederfinden. Der
Issue-Body enthält Diagnosewerte wie Code-Stelle, Prozessstatus, Toolchain und Command-Verlauf.
Diese Werte können sich zwischen zwei Beobachtungen desselben Fehlers ändern.

Auch die Fehlermeldung kann technische Schwankungen enthalten. Dazu gehören ANSI-Sequenzen,
absolute Projektpfade, Speicheradressen und Zeitangaben. Eine pauschale Entfernung aller Zahlen
würde dagegen fachlich verschiedene Meldungen wie `HTTP 404` und `HTTP 500` zusammenführen.

## Entscheidung

1. Die fachliche Identität besteht ausschließlich aus der Fehlerart und der bereinigten
   Fehlermeldung.
2. Code-Stelle, Tracing-Target, Prozessstatus, Backtrace, Session, Controller, Command-Verlauf und
   Report-Kontext bleiben Diagnose und gehören nicht zur Signatur.
3. Die Meldungsbereinigung vereinheitlicht Zeilenenden, entfernt ANSI-Steuersequenzen und äußeren
   ASCII-Leerraum und ersetzt absolute Projektpfade, Speicheradressen sowie klar erkennbare
   ISO-Datums- und Uhrzeitangaben.
4. Andere Zahlen, Großschreibung, Unicode und interner Leerraum bleiben erhalten. Unix-Zeitstempel
   und beliebige Zahlenfolgen werden nicht anhand ihrer Größe geraten.
5. `report::Signature::value` besitzt die Form `v1:sha256:<digest>`. Der Digest besteht aus genau 64
   kleingeschriebenen Hex-Zeichen.
6. SHA-256 verarbeitet eine versionierte, längenpräfixierte Binärdarstellung der beiden Felder
   `kind` und `message`.
7. Ein unerwartetes Prozessende verwendet die feste Meldung `process exited unexpectedly`.
   Der beobachtete Prozessstatus bleibt ausschließlich Diagnose.
8. Fehlt einem Panic wegen eines unbekannten `panic_any`-Payloads eine lesbare Meldung, verwendet das
   Signaturfeld `message` den leeren String.
9. Provider speichern und vergleichen die vollständige Signatur einschließlich Version und
   Algorithmus. Eine neue Identitäts- oder Bereinigungsregel benötigt eine neue Signaturversion und
   ändert bestehende Reports nicht rückwirkend.

Die genaue Binärdarstellung, Feldreihenfolge und die Golden Vectors stehen im Abschnitt
"Fehlersignatur" von `docs/api/migration.md`.

## Folgen

- Gleichartige Fehler an verschiedenen Code-Stellen erhalten dieselbe Signatur, wenn Art und
  bereinigte Meldung übereinstimmen.
- Unterschiedliche Tracing-Targets, Prozessstatus, Backtraces, Thread-IDs und Command-Verläufe
  verändern die Signatur nicht.
- Ein Panic und ein Prozessende mit derselben Meldung bleiben wegen ihrer Fehlerart verschieden.
- Unerwartete Prozessenden ohne erkannten Panic oder Tracing-Error erhalten dieselbe Signatur.
- Ein unbekannter Panic-Payload und eine ausdrücklich leere Panic-Meldung erhalten dieselbe
  Signatur. Beide besitzen keine unterscheidbare Meldung.
- Ein Tracing-Event ohne `message` verwendet eine kanonische Zusammenfassung seiner beobachteten
  Felder als Meldung. Änderungen an dieser Meldung können seine Signatur verändern.
- Klar erkennbare Zeitangaben verändern die Signatur nicht. Andere Zahlen bleiben Teil der
  Identität.
- SHA-256 benötigt eine kleine zusätzliche Implementationsabhängigkeit. Rusts `DefaultHasher` ist
  kein Ersatz, weil Algorithmus und Seed kein persistierter Vertrag sind.

## Prüfung

- Golden Vectors fixieren Binärdarstellung und Digest.
- Geänderte Speicheradressen, ANSI-Ausgabe, Projektwurzelpfade und erkennbare Zeitangaben behalten
  dieselbe Signatur.
- Eine andere bereinigte Meldung oder Fehlerart ändert die Signatur.
- Code-Stelle, Tracing-Target, Prozessstatus, Backtrace, Session, Controller und Command-Verlauf
  ändern sie nicht.
- Ein unbekannter Panic-Payload verwendet den Golden Vector für einen leeren `message`-Wert.
- Verschiedene Prozessstatuswerte erzeugen für ein unerwartetes Prozessende dieselbe Signatur.
- Zahlen wie HTTP-Statuscodes bleiben erhalten und können Signaturen unterscheiden.
