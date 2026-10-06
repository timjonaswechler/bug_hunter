# Ungültige Agent-Eingaben bleiben lokal

Historischer Entscheidungsstand. Für den Rewrite gilt [target.md](../api/target.md).
Die damalige äußere JSONL-Hülle ist kein beschlossener Client-Vertrag.

## Status

Teilweise wieder geöffnet durch ADR-0022.

## Kontext

Die ursprünglich geplante `host::agent`-Fassade liest JSONL-Einträge ohne vom Aufrufer vergebene
Request-ID und reicht daraus dekodierte `command::Command`-Werte an die Session weiter. Die Session
vergibt eine ID erst bei der Annahme eines Commands. Eine syntaktisch oder strukturell ungültige
Eingabe erreicht diese Grenze nicht.

Der Agent muss solche Eingaben maschinenlesbar melden und danach weitere Zeilen verarbeiten können.
Die Meldung darf zugleich keine Session-Korrelation vortäuschen und nicht mit einem terminalen
Outcome eines angenommenen Commands verwechselt werden.

ADR-0022 ersetzt die Annahme einer langlebigen Agent-Verbindung über stdin/stdout durch einen
persistenten Session-Host und begrenzte Agent-Tool-Aufrufe. Die beiden Fehlerklassen und die
Session-Grenze bleiben beschlossen. Ihre äußere Form, Zeilenzählung und Fortsetzungsregel werden
gegen das gemeinsame Client-Protokoll neu geprüft.

## Entscheidung

1. `host::agent` liest jede JSONL-Zeile als Bytes. Eine vollständig gelesene Zeile mit ungültigem
   UTF-8 bleibt dadurch ein lokaler Inhaltsfehler und kein technischer Lesefehler.
2. Ungültiges JSON einschließlich einer leeren Zeile erzeugt `input_error` mit dem stabilen Code
   `invalid_json`.
3. Ein syntaktisch gültiger JSON-Wert, der nicht als erlaubter Agent-Command dekodiert werden kann,
   erzeugt `input_error` mit dem stabilen Code `invalid_command`. Dazu gehören ein Nicht-Objekt,
   fehlende oder unbekannte Felder, ein unbekannter Command-Name und strukturell falsche Argumente.
   `shutdown` ist dagegen ein erlaubter Client-Command und wird vom serverseitigen Adapter zu
   `Session::shutdown` geroutet.
4. Die genauere Ursache steht ausschließlich in `error.message`. Es gibt keine weiteren stabilen
   Eingabefehlercodes.
5. Eine Eingabefehlermeldung besitzt die Form:

   ```json
   {"input_line":4,"request_id":null,"status":"input_error","error":{"code":"invalid_json","message":"expected value at column 18"}}
   ```

   `input_line` ist die bei eins beginnende Nummer der physischen Eingabezeile und zählt auch leere
   und ungültige Zeilen. Der Agent gibt den ursprünglichen Zeileninhalt nicht erneut aus.
6. Ein Eingabefehler wird nicht an die Session gesendet. Er erzeugt weder Request-ID,
   `pending`-Meldung, History-Eintrag noch Recording-Eintrag und verbraucht keine Session-ID.
7. Nach jeder vollständig gelesenen ungültigen Zeile verarbeitet der Agent die nächste Zeile.
   Bereits angenommene Commands laufen unverändert weiter.
8. Sobald die Eingabe als erlaubter `command::Command` dekodiert wurde, übernimmt die Session.
   Fachliche Fehler wie ungültige Command-Werte, unbekannte Type Paths oder nicht vorhandene
   Entities erhalten daher eine Request-ID und enden mit einer normalen `rejected`-Meldung.
9. Ein `std::io::Error` beim Lesen beendet `run` mit `agent::Error::Input`, weil keine verlässliche
   Grenze zur nächsten Zeile besteht. Ein Fehler beim Schreiben der Ausgabe beendet `run` mit
   `agent::Error::Output`. Reguläres EOF ist kein Lesefehler; sein Verhalten wird getrennt
   festgelegt.

## Folgen

- Jeder angenommene Command und nur ein angenommener Command besitzt eine Session-Request-ID.
- Ein maschineller Aufrufer kann fehlerhafte Eingaben anhand ihrer physischen Zeilennummer
  lokalisieren.
- Einzelne fehlerhafte Einträge unterbrechen weder den Eingabestrom noch laufende Commands.
- Der Agent benötigt keine eigene fachliche Command-Validierung. Er ordnet Fehler des gemeinsamen
  Decoders lediglich einem der beiden lokalen Codes zu.
- Session-Events und EOF benötigen weiterhin eigene Regeln.

## Prüfung

- Fixtures decken kaputtes JSON, ungültiges UTF-8, eine leere Zeile und einen JSON-Nicht-Objektwert
  ab.
- Weitere Fixtures decken fehlende und unbekannte Felder, einen unbekannten Command-Namen sowie
  strukturell falsche Argumente ab. Eine Shutdown-Fixture belegt die Weitergabe als erlaubten
  Client-Command.
- Eine ungültige Zeile zwischen zwei gültigen Commands sowie mehrere ungültige Zeilen hintereinander
  belegen die Fortsetzung und korrekte `input_line`-Werte.
- Fixtures belegen, dass ein Eingabefehler keine Request-ID verbraucht und keinen History- oder
  Recording-Eintrag erzeugt.
- Fachlich ungültige, aber dekodierbare Commands erhalten dagegen eine Request-ID und ein
  korreliertes `rejected`.
- Injizierte Lese- und Schreibfehler belegen den jeweils terminalen `agent::Error`.
