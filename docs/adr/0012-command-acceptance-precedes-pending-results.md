# Command-Annahme geht dem Pending-Ergebnis voraus

## Status

Angenommen.

## Kontext

`Session::send` nimmt Wire- und hostseitige Commands über dasselbe Interface an. Controller müssen
für jeden angenommenen Command zuerst dessen Request-ID und später genau ein terminales Ergebnis
erhalten. Würden commandspezifische Zustandsprüfungen bereits direkt aus `send` fehlschlagen, müsste
der Agent manche gültigen Eingaben ohne vorherige `pending`-Meldung ausgeben.

Ein fallengelassenes `Pending` darf einen bereits gestarteten Command nicht abbrechen. Der
Koordinator muss dessen Response weiterhin korrelieren, History und Recording abschließen und den
Command beim Shutdown als ausstehend behandeln.

Request-IDs sind transportlokale Korrelation und keine fachliche Reihenfolge. Trotzdem benötigt der
endliche `u64`-Nummernraum eine Regel, die einen sauberen Shutdown nicht unmöglich macht.

## Entscheidung

1. `Session::send` trennt die Annahme eines Commands von seinem terminalen Ergebnis. Es wartet
   höchstens auf die interne Annahmebestätigung des Session-Koordinators und nie auf die
   Command-Ausführung.
2. Nur eine beendete oder technisch nicht mehr erreichbare Session sowie ein erschöpfter
   Request-ID-Bereich verhindern die Annahme. In diesen Fällen entstehen weder `Pending` noch
   Request-ID oder History-Eintrag.
3. Commandspezifische Zustands- und Argumentfehler schließen einen angenommenen Command später
   terminal ab. Dazu gehören insbesondere `replay_in_progress`, `replay_already_running`, ein
   bereits aktives Recording und ein ungültiger Recording-Pfad.
4. Jeder angenommene Command erhält deshalb zuerst ein `Pending`. Erfolg, `Rejected`, `Protocol` und
   ein commandbezogener `Io`-Fehler sind dessen mögliche terminale Ergebnisse.
5. `RequestId` ist ein opaker `u64` mit `as_u64` und `Display`. Der Typ ist kopierbar,
   vergleichbar, hashbar und sortierbar.
6. Die Vergabe beginnt bei 1, steigt innerhalb einer Session monoton und verwendet keinen Wert
   erneut. Interne Replay-Commands verwenden denselben Nummernraum. Sichtbare IDs dürfen dadurch
   Lücken besitzen, und ihr Zahlenwert bestimmt keine fachliche Ausführungs- oder
   Response-Reihenfolge.
7. Verschiedene Sessions dürfen denselben Zahlenwert vergeben. Ein `Pending` enthält deshalb neben
   der Request-ID eine private Session-Identität.
8. `u64::MAX` bleibt für den abschließenden Wire-Shutdown reserviert. Sind die normalen IDs
   verbraucht, gibt `send` `session::Error::RequestIdExhausted` zurück. Bereits angenommene Commands
   können weiterhin beendet und die Session kann sauber heruntergefahren werden.
9. `Pending` ist nicht klonbar und repräsentiert genau einen angenommenen Command. Es gibt seine
   Request-ID als kopierbaren Wert und den vollständigen Command lesend frei.
10. `try_receive` liefert `Ok(None)`, solange der Command nicht terminal ist. Ein erfolgreicher
    Output oder ein terminaler Fehler kann genau einmal entnommen werden. Ein weiterer Zugriff und
    die Verwendung mit einer anderen Session ergeben `InvalidPending`.
11. `receive` konsumiert das `Pending`. Das Typsystem verhindert danach einen zweiten Zugriff.
12. Das Fallenlassen eines nicht terminalen `Pending` verwirft nur das Interesse des Aufrufers. Der
    Command läuft weiter und zählt bis zu seinem terminalen Ergebnis als ausstehend. History und ein
    aktives Recording werden weiterhin abgeschlossen.
13. Hat der Koordinator einen Command ohne verbliebenes `Pending` abgeschlossen, darf er dessen
    eigentlichen Rückgabewert nach dem Abschluss von History und Recording verwerfen.
14. `Session::shutdown` bleibt außerhalb des allgemeinen `send`-Ablaufs. Es prüft seine
    Vorbedingungen synchron und verwendet bei Annahme die reservierte Shutdown-ID.

## Folgen

- Der Agent kann für jeden syntaktisch gültigen und von der Session angenommenen Command zuerst
  `pending` und danach genau ein terminales Ergebnis ausgeben.
- Commands während eines exklusiven Replays erhalten eine Request-ID und enden mit
  `replay_in_progress`, statt bereits im `send`-Aufruf zu verschwinden.
- Eine fallengelassene Ergebnisreferenz ist kein versteckter Cancel- oder Detach-Command.
- `RequestIdExhausted` beendet die Session nicht, verhindert aber weitere normale Commands.
- Die genaue Wirkung von technischen Session-Fehlern wird getrennt festgelegt.

## Prüfung

- Fixtures unterscheiden Fehler vor der Annahme von terminalen Command-Ablehnungen.
- Agent-Fixtures prüfen `pending` vor `completed`, `rejected` und `failed`, auch bei einem Command
  während eines exklusiven Replays.
- Zwei Sessions vergeben dieselben numerischen IDs; ein `Pending` der einen Session wird von der
  anderen mit `InvalidPending` abgelehnt.
- Ein terminal ausgelesenes `Pending` ergibt beim zweiten Zugriff `InvalidPending`.
- Eine Fixture verwirft ein nicht terminales `Pending`, prüft den weiteren Command-Fortschritt und
  den später abgeschlossenen History- und Recording-Eintrag.
- Eine künstlich am Ende des Nummernraums gestartete Session reserviert `u64::MAX` für Shutdown,
  lehnt einen weiteren normalen Command mit `RequestIdExhausted` ab und fährt sauber herunter.
