# Ziel von woodpecker

`woodpecker` kontrolliert und beobachtet ein Spiel, damit Menschen und Agenten nach Fehlern
suchen können. Recording, Replay und Reports unterstützen reproduzierbare Untersuchungen.
Ein lokaler Server hält mehrere unabhängige Spiel-Sessions bereit; die CLI macht Verwaltung,
Steuerung und Beobachtung zugänglich.

Dieses Dokument ist die maßgebliche Beschreibung des gewünschten Systems.
[goal.rs](goal.rs) skizziert die Interfaces, ohne kompilierbaren Code vorzugeben.
Der [Umsetzungsplan](implementation-plan.md) steht getrennt davon.
Die nummerierten ADRs dokumentieren Entscheidungsherkunft, nicht
einen zusätzlich zu pflegenden Zielstand.

## Headless-Betrieb

Der Agent muss eine Spiel-Session ohne natives Fenster und ohne Displayserver
steuern können: Bild anfordern, virtuelle Tastatur- oder Mauseingaben vormerken,
explizite Ticks ausführen und das Ergebnis erneut beobachten. Ein geeignetes
Grafik-Backend bleibt erforderlich; Headless bedeutet nicht Rendering ohne
Grafikgerät oder Treiber.

Eine gerenderte Session besitzt ein eindeutiges primäres **Bildziel**, unabhängig
von einer Fensteroberfläche. Seine Pixelgröße, logische Größe, Skalierung und
Kamerazuordnung müssen für Capture, UI-Layout und Pointer-Picking übereinstimmen
und für den Client ermittelbar sein. Absolute Eingaben und Bildausgabe beziehen
sich auf denselben Koordinatenraum. Ein fehlendes Betriebssystemfenster ist kein
Eingabe- oder Capture-Fehler.

Rendering, GPU-Readback und Command-Verarbeitung dürfen ohne Simulationsfortschritt
weiterlaufen. Nur Warps führen Spiel- und Eingabesysteme aus. Weder Bildaufnahme
noch Renderbereitschaft dürfen versteckte Ticks, Fokusänderungen oder einen
physischen Cursor benötigen.

Eine optionale Fenstervorschau ist nicht Voraussetzung und noch nicht beschlossen.
Unterstützung beliebiger unveränderter Fensteranwendungen ist nicht automatisch
zugesagt. Die Spielanwendung integriert den Renderer und ihre Kameras ausdrücklich;
`woodpecker` bleibt ohne Renderer für reine Zustandsuntersuchungen nutzbar.

Dies ist das Ziel, nicht der heutige Implementierungsstand. Der
[Durchstich](slice.md) verwendet noch Fensteradapter. Die konkrete Konfiguration
des Bildziels, Bereitschaftskriterien, relative Blicksteuerung und die Migration
der bisherigen fensterbezogenen Fehlerformen sind Aufgaben des
[Headless-Plans](implementation-plan.md#headless-durchstich).

## Verantwortlichkeiten

| Modul | Besitzt |
| --- | --- |
| `handle` | Sessionlokale Entity-Identität aus `index: u32` und `generation: u32` |
| `command` | Fachliche Command- und Output-Typen; je Command seine Wert-, Zustands- und Erfolgsregeln |
| `session` | Eine Spielinstanz, Start, Prozessgruppe, Pipes, Koordinator, Annahme, Request-Korrelation, History, Recording- und Replay-Ausführung, Events und Ressourcenabschluss |
| `session::launch` | Cargo-Launch-Konfiguration, deren Validierung und Auflösung |
| `session::protocol` | Gemeinsamer Codec für qualifizierte Command-Namen, Argumente und Outputs sowie das interne Spielprotokoll v3 |
| `session::Plugin` | Bevy-Integration, Dispatch und kontrollierte Ausführung der Anwendungsschedules |
| `report` | Fehlererkennung, Marker-Framing, Report-Konstruktion, Titel, Signatur, Kontextauswahl, Darstellung und Provider |
| `server` | Session-Verzeichnis, IDs, Client-Routing, Activity-Aufbewahrung, Loopback-Bindung, Serverende und clientunabhängiger Report-Ablauf |
| `client` | Verbindungszugriff, Nutzung des gemeinsamen Codecs und Zuordnung von Antworten für Verwaltung und gebundene Session-Verbindungen |
| `cli` | Argumente, Konfigurationsdateien, Terminaldarstellung und Auswahl des Bedienablaufs |
| `cli::repl` | Menschliche Command-Eingabe und laufende Ergebnisdarstellung |
| `cli::script` | Versionierte Command-Liste, vollständiges Parsing und Zuordnung der Ergebnisse zu Listeneinträgen |

Die fachliche Validierung bleibt beim jeweiligen Owner. Ein Parser prüft die Darstellung,
der Server routet, die Session führt aus. Zusätzliche Nutzer desselben Vertrags verwenden
denselben Codec. Der getrennt versionierte äußere Client-Vertrag bekommt eine gemeinsame
Implementierungsstelle, keine Kopien in Client und Server.

Direkte Rust-Nutzung von `session::Session`, `session::Plugin`, Commands und Reports funktioniert
ohne Server-/CLI-Abhängigkeiten. Die Platzierung eines Moduls bestimmt nicht automatisch
seine öffentlichen Exports oder seine Paketierung.

## Server und Zugriff

### Start und Konfiguration

Der Server startet im Vordergrund mit leerem Session-Verzeichnis. Seine Logs erscheinen im
startenden Terminal. Jede Spiel-Session wird anschließend ausdrücklich erzeugt.
Ein Spielstartfehler beendet den Server nicht.

Der Server erhält serverweite Konfiguration:

- eine ausschließlich über Loopback erreichbare Adresse,
- einen nicht leeren Artefakt-Basisordner als Pflichtangabe,
- eine positive, endliche Shutdown-Frist, standardmäßig 30 Sekunden.

Relative Basisordner werden einmal gegen das Arbeitsverzeichnis beim Serverstart aufgelöst.
Fehler beim Anlegen oder Auflösen verhindern die Bereitschaft. Ungültige Werte werden mit
Startfehler abgelehnt, nicht still korrigiert.

Bereit ist der Server nach abgeschlossener Initialisierung, sobald er Verwaltungsaufrufe
annehmen kann. Erst dann zeigt die CLI die tatsächlich verwendete Adresse. Eine belegte
Adresse führt zum Startfehler; der Server weicht nicht auf eine andere aus.
Clients erhalten die Adresse ausdrücklich. Ein nicht erreichbarer Server ergibt einen
Verbindungsfehler.

Serverkonfiguration enthält keine Spiel-Launch-Defaults. Jede Erstellung erhält ihre eigene
vollständige Launch-, Tick- und Report-Konfiguration. Wiederholtes Lesen derselben Datei
erzeugt unabhängige Laufzeitwerte. Andere Servereinstellungen benötigen einen Neustart,
andere Session-Einstellungen eine neue Session. Fachliche Commands wie `SetPace` verändern
weiterhin den dafür vorgesehenen Laufzeitzustand.

### Session-Erstellung und Artefakte

Der Server vergibt 128 zufällige Bits als genau 32 kleine Hex-Zeichen ohne Bindestriche.
Clients bestimmen die ID nicht. Die Auswahl ist gegenüber parallelen Erstellungen eindeutig.
Eine Kollision mit dem Verzeichnis oder einem bereits vorhandenen ID-Verzeichnis führt vor
Rückgabe der ID zur Neuwahl.

Die Annahmegrenze trennt intrinsische Gültigkeit von Ausführbarkeit:

| Vor Annahme, bei Fehler kein Eintrag | Nach Annahme in `Starting`, bei Fehler `Failed` unter derselben ID |
| --- | --- |
| Anfrage dekodieren; Pflichtwerte, Wertebereiche und Kombinationen durch ihre Owner prüfen; ID auswählen | Cargo und Manifest auflösen; benötigte Dateien und Programme öffnen; Artefaktverzeichnis anlegen; Spiel starten und Handshake ausführen |

Ein fehlender erforderlicher Manifestwert ist somit eine unmittelbare Ablehnung. Eine
angegebene, aber fehlende oder unlesbare Manifestdatei ist ein Fehler der angelegten Session.
Die Erstellungsantwort bestätigt mit der vollständigen ID den Verzeichniseintrag,
nicht die Spielbereitschaft.

Jede Server-Session erhält `<absoluter Basisordner>/<vollständige Session-ID>/`.
Der Server setzt diesen Pfad in die interne Session-Konfiguration ein; die
Erstellungsanfrage enthält keinen frei wählbaren Session-Artefaktpfad.
Die Anlage erfolgt erst nach Annahme. Scheitert sie, bleibt die bereits zurückgegebene ID
unverändert. Auch ein inzwischen von außen angelegtes Ziel darf nicht übernommen werden.
Artefakte bleiben nach Session-Ende erhalten. Laufende Schreibarbeit einschließlich Reports
bleibt ihrer Session zugeordnet und wird vor Freigabe der dafür benötigten Ressourcen beendet
oder im geregelten Serverabbruch ausdrücklich als unvollständig behandelt.

### Identität, Bindung und Verwaltungsdaten

Maschinenantworten enthalten volle IDs. Menschenlesbare Anzeigen verwenden mindestens acht
Zeichen und verlängern kollidierende Präfixe bis zur Eindeutigkeit. Eingaben umfassen
8 bis 32 Hex-Zeichen. Der Server löst Präfixe gegen alle Verzeichniseinträge auf, auch gegen
beendete Sessions. Genau ein Treffer ist nötig; unbekannte und mehrdeutige Auswahl sind Fehler.

HTTP dient der Verwaltung, WebSocket der fest an eine Session gebundenen Steuerung und
Beobachtung. Beide verwenden dieselbe ausschließlich lokale Serveradresse.
Lokalen Clients wird vertraut; es gibt keine Schlüssel-Authentisierung, keine
Zugangsdaten-Konfiguration und keinen automatisch erzeugten Ersatzschlüssel.
Jeder lokale Prozess, der den Port erreicht, darf die API verwenden.
Remote-Betrieb wird nicht unterstützt. Browser-Origin-Anfragen bleiben ausgeschlossen.
Die Session-ID wählt eine Session aus und ist kein Berechtigungsnachweis.

Erstellen, Auflisten und Einzelabfrage benötigen keine Session-Bindung. Nach Erstellen öffnet
der Client mit der erhaltenen ID eine eigene Session-Verbindung. Diese Bindung gilt für die
gesamte Verbindung und wird nicht erneut aus dem Präfix bestimmt. Commands enthalten keine
erneute Session-ID. Eine andere Session benötigt eine neue Verbindung.

Mehrere Clients dürfen dieselbe Session steuern. Der Server ordnet eingehende Commands je
Session eindeutig und übergibt sie deren Ausführungsweg. Verschiedene Sessions teilen
keinen Command-, Recording- oder Replay-Zustand. Ein Client-Ende sendet weder Stop noch
Shutdown; angenommene Arbeit läuft weiter.

Die Liste enthält alle Einträge mit vollständiger ID, Lebenszykluszustand und Erstellungszeit,
älteste zuerst. Die Einzelabfrage ergänzt den zugewiesenen Artefaktpfad und bei `Failed`
einen stabilen Fehlercode mit verständlicher Meldung. Beide Antworten sind Momentaufnahmen
und geben nicht automatisch die vollständige Launch-Konfiguration aus.

### Sichtbarer Lebenszyklus

| Zustand | Bedeutung |
| --- | --- |
| `Starting` | Eintrag vorhanden; Vorbereitung oder Spielstart läuft |
| `Ready` | Start und Handshake abgeschlossen; Commands dürfen trotzdem noch laufen |
| `Stopping` | Herunterfahren läuft; Beobachtung bleibt möglich, neue Spielarbeit nicht |
| `Ended` | Regulär beendet |
| `Failed` | Vorbereitung, Start, Betrieb oder Abschluss fehlgeschlagen; mit Code und Meldung |

Eine Verbindung ist in jedem dieser Zustände möglich. Commands mit Bereitschaftsvoraussetzung
werden außerhalb von `Ready` abgelehnt, nicht für später vorgemerkt.
Ein einzelner Command-Fehler macht die Session nicht automatisch `Failed`.

Ein angenommener Shutdown wechselt nach `Stopping`. Ein vor Annahme blockierter
Einzel-Session-Shutdown verändert den Lebenszyklus nicht. Sauberer Abschluss ergibt `Ended`,
Abschlussfehler oder erzwungener Abbruch ergeben `Failed`. Beim Verwaltungs-Stopp gilt dies auch für
`Starting`; ein absichtlich beendeter Start ist bei erfolgreicher Bereinigung regulär beendet.
Bereits terminale Einträge bleiben unverändert.

`Ended` und `Failed` bleiben mit ihrem Grund bis zum Serverende im Verzeichnis. Das hält
keine Spielprozesse am Leben und verlängert nicht die Activity-Aufbewahrung.
Das Verzeichnis wird nach einem Serverneustart nicht wiederhergestellt.

### Activity und Report-Ablauf

Der Server übernimmt Command-Annahmen, terminale Outcomes, Session-Events und Report-Ergebnisse
in Activity je Session. Alle an dieselbe Session gebundenen Clients sehen denselben Strom,
einschließlich der Commands anderer Clients. Cursor, Session-ID und Request-ID haben
unterschiedliche Bedeutungen.

Ein Cursor enthält eine prüfbare Session-Zuordnung und eine monotone Position. Ein fremder
Cursor ist ein Fehler. Ein zu alter Cursor meldet eine Lücke und den ältesten verfügbaren
Einstieg; der Server setzt ihn nicht stillschweigend fort.

Eine konfigurierbare Byte-Grenze je Session begrenzt die Activity-Aufbewahrung. Ein einzelner
zu großer Eintrag wird nicht gekürzt, sondern erzeugt eine erkennbare Lücke.
`poll` liefert sofort verfügbare Einträge und einen Fortsetzungscursor. `wait` wartet begrenzt
auf Activity oder Fristablauf. Eine erkannte Lücke beendet das Warten auf verlorene Ergebnisse
mit ausdrücklich unbekanntem Ausgang.

Eine nach Verbindungsverlust nicht bestätigte Einreichung bleibt ungewiss und wird nicht
automatisch erneut gesendet.

Der gebundene Client kann zusätzlich eine aktuelle Momentaufnahme mit Lebenszyklus,
offenen serverseitigen Commands und Activity-Cursor abfragen. Diese Abfrage nimmt keinen
Spiel-Command an. Offene Commands bleiben unabhängig von der begrenzten Activity erhalten;
Momentaufnahme und Cursor werden unter derselben Sperre gelesen. Die Momentaufnahme
rekonstruiert keine verlorenen Outcomes.

Eine Command-Annahme erscheint vor ihrem terminalen Ergebnis mit der von der Session
vergebenen Request-ID und dem qualifizierten Command-Namen. Terminale Ergebnisse sind
`completed`, `rejected` oder `failed`. Session-Events gehören keinem Command.
Die fachlichen Event-Payloads bilden die Session-Varianten unmittelbar ab; optionale
Failure-Daten erscheinen als `null`. `Ended` schließt den Session-Eventstrom, nicht die gesamte
Activity. Bereits angestoßene Report-Veröffentlichungen können danach ihr Ergebnis liefern;
das Session-Endevent bestätigt deshalb keine vollständige Report-Ausgabe.

Ohne Session-Annahme entsteht keine fiktive Request-ID. Das gilt insbesondere vor Ready,
nach Session-Ende, bei vom Server abgewiesener neuer Arbeit während Server-Shutdown und
bei blockiertem Einzel-Session-Shutdown. Fachliche Ablehnungen normal angenommener
Commands erscheinen dagegen nach Pending mit deren ID.

Der Server nimmt Session-Events unabhängig von Clients entgegen. Bei `Failure` erzeugt er
unmittelbar den Report-Snapshot und stößt danach `report::submit` an. Er behält Report und
Provider-Outcome oder Submit-Fehler gemeinsam. Ein langsamer Provider oder Client darf
weder den Empfang der Session-Events noch das Leeren der Spiel-Pipes blockieren.

### Verwaltungs-Stopp einer Session

Die Docker-artige Session-Verwaltung bietet `session stop <id>` als ausdrücklichen
Verwaltungsauftrag an. Er stoppt genau die gewählte Session, unabhängig davon, ob sie
noch startet oder bereits läuft. Server und andere Sessions bleiben aktiv.

Der Server setzt eine gewählte Session in `Starting` oder `Ready` auf `Stopping` und nimmt
für sie keine neue Spielarbeit mehr an. Er beendet einen laufenden Startvorgang oder stoppt Replay und andere ausdrücklich
stoppbare Arbeit über deren fachliche Operationen. Bereits gesendete, nicht stoppbare Commands
dürfen abschließen. Danach beendet er aktive Recordings sauber und führt den Session-Shutdown
aus. Prozessbereinigung bleibt bei `session`, Ablaufkoordination bei `server`.

Der Spiel-Command `shutdown` behält seine Vorbedingungen. Der Verwaltungs-Stopp stellt diese
Vorbedingungen durch die ausdrücklich angeforderten Stop- und Abschlussoperationen her;
er umgeht sie nicht.

### Serverende

Serverende ist eine ausdrückliche Verwaltungsoperation, getrennt vom Shutdown einer
einzelnen Session. Der Server:

1. nimmt keine neuen Sessions oder Spiel-Commands mehr an,
2. führt den beschriebenen Verwaltungs-Stopp für alle noch nicht terminalen Sessions aus,
3. beendet nach Abschluss der Sessions und Report-Arbeit den Server.

Alle Sessions machen dabei unabhängig voneinander Fortschritt. Ein Fehler in einer Session
hält die übrigen nicht auf. Jede nicht sauber heruntergefahrene Session führt insgesamt
zum Fehlerausgang.

Die gemeinsame Frist läuft ab Annahme des Serverendes, nicht erneut pro Session oder Report.
Bis zum Verbindungsende bleiben Outcomes, Events und Report-Ergebnisse innerhalb der
Activity-Aufbewahrung abrufbar. Der Server wartet nicht auf vollständiges Abholen durch Clients.
Auch Reports echter Spielfehler während des Herunterfahrens verwenden dieselbe Frist.

Bei Fristablauf beendet die Session-Prozessverwaltung verbliebene Spielprozesse erzwungen und
bereinigt Ressourcen. Der Server meldet einen unvollständigen Abschluss mit Fehlerstatus.
Vollständige Outcomes, Recording-Footer und abgeschlossene Reports sind dann nicht garantiert.
Ein unterbrochener Provider-Aufruf kann extern bereits gewirkt haben; der Ausgang darf als
unbekannt erscheinen, niemals als behauptete erfolgreiche Übertragung oder Rücknahme.

Erstes Ctrl+C und SIGTERM lösen denselben geordneten Ablauf aus. Zweites Ctrl+C währenddessen
erzwingt die Bereinigung sofort. SIGKILL und Serverabsturz garantieren keinen geordneten Abschluss.
Absichtlich beendete Spielprozesse oder Startvorgänge sind keine Spielfehler-Reports.

## Direkte Session und Bevy-Integration

### Start und Pfade

`session::Config` enthält `launch`, `artifact_dir`, `tick` und `report`.
`launch::Config` nennt verpflichtend Manifest, nicht leeres Package sowie genau ein benanntes
Binary oder Example. `features` aktiviert Cargo-Features, `arguments` wird hinter `--`
unverändert an die Anwendung übergeben.

`session::launch` löst einen relativen Manifestpfad gegen das Arbeitsverzeichnis des
aufrufenden Prozesses auf, verlangt eine reguläre Datei und kanonisiert sie. Ihr Elternordner
ist der unveränderliche `project_dir`. `cargo metadata --format-version 1 --no-deps` und
`cargo run` erhalten `--manifest-path`; `cargo run` arbeitet in `project_dir`.
Das ausgewählte Package und seine Version müssen eindeutig auflösbar sein.
Ein nicht reguläres Manifest ergibt beim direkten Einstieg `InvalidConfig`, eine
fehlgeschlagene Cargo-Auflösung `Launch`. Beim Server bestimmt die Erstellungsgrenze,
ob ein Fehler die Anfrage ablehnt oder unter der angenommenen ID sichtbar wird.

Direkte Rust-Nutzer wählen ihren Artefakt-Root selbst. Ein relativer Wert bezieht sich auf
`project_dir`; absolute Roots und Roots außerhalb des Projekts sind erlaubt.
Ein leerer Root oder ein Root, der selbst ein Symlink ist, ergibt `InvalidConfig`.
Die Session legt ihn bei Bedarf an und kanonisiert ihn. Anlage-/Kanonisierungsfehler ergeben `Io`.
Alle einzelnen Artefakte bleiben unter diesem gemeinsamen Root.

Die Session übergibt den kanonischen Root intern als `WOODPECKER_ARTIFACT_DIR` und setzt
`RUST_BACKTRACE=1` für den Kindprozess. Das Plugin liest den Root vor Ready.
Ein fehlender oder ungültiger Root verhindert Ready.

`Session::start` gibt erst nach erfolgreichem Handshake und erforderlicher Layer-Bestätigung
einen nutzbaren Handle zurück. Start besitzt keinen eingebauten Timeout. Fehler vor Ready
räumen gestartete Ressourcen auf. Das Serverinterface verwendet denselben Startweg intern
abbrechbar, ohne den synchronen öffentlichen Einstieg zu verändern.

### Fortschritt, Annahme und Ergebnisse

`Session` ist ein synchroner, nicht klonbarer Handle ohne öffentlichen Async-Runtime-Vertrag.
Ein privater Koordinator besitzt Kindprozess, Pipes und veränderlichen Session-Zustand.
Eigene Leser leeren stdout und stderr ab Prozessstart. Auf der Spielseite liest ein privater
Leser stdin; nur der Bevy-Event-Loop greift auf den World zu. Ein Writer serialisiert
vollständige stdout-Nachrichten. Lang laufende Bevy-Arbeit erhält begrenzte Arbeitsbudgets,
zwischen denen neue Commands verarbeitet werden.

`command::Request` ist versiegelt und verbindet jeden konkreten Request mit einem Output-Typ.
`command::Command` ist die typgelöschte Darstellung für Parsing, History, Recording und interne
Ausführung. Die öffentliche `send`-Methode verwendet konkrete Requests.
Der interne dynamische Session-Adapter verwendet denselben Koordinator und routet `Shutdown`
ausschließlich zu `Session::shutdown`.

`send` wartet nur auf Annahme und liefert `Pending<C::Output>`. Eine beendete oder technisch
nicht erreichbare Session sowie erschöpfte normale IDs verhindern Annahme, Pending und
History-Eintrag. Commandspezifische Zustands- und Wertfehler werden dagegen nach Annahme
terminal, einschließlich Konflikten mit Replay oder Recording.

Die Session vergibt opake `u64`-Request-IDs ab 1, monoton und ohne Wiederverwendung.
Interne Replay-Commands verwenden denselben Nummernraum. `u64::MAX` ist für den einmaligen
Wire-Shutdown reserviert. Erschöpfung ergibt `RequestIdExhausted`, ohne laufende Commands
abzubrechen. Verschiedene Sessions dürfen denselben Zahlenwert verwenden.

Commands starten in Annahmereihenfolge, ihre Ergebnisse dürfen ungeordnet eintreffen.
Ein Pending gehört genau einer Session und einem Command. `try_receive` fragt ausschließlich
verfügbare interne Nachrichten ab; `receive` wartet nur auf dieses Pending. Ein terminales
Ergebnis wird genau einmal entnommen. Erneute Entnahme oder Verwendung an einer anderen
Session ergibt `InvalidPending`; `receive` konsumiert das Pending.

Pending-Drop beendet nur das Interesse am Ergebnis. Der Command läuft weiter, bleibt bis zum
terminalen Ausgang shutdown-relevant und schließt History und Recording ab.
Angenommene Arbeit einschließlich Replay macht auch ohne Empfangsaufrufe Fortschritt.

### History, Events und Fehler

Die History hält die letzten 50 angenommenen Commands in Annahmereihenfolge. Jeder beginnt
als `Unanswered`; ein terminales Outcome ergänzt ihn an derselben Position.
Aufgenommen werden Wire-Commands, Recording-/Replay-Steuerung, interne Replay-Commands und
angenommener Shutdown. Vor Annahme gescheiterte Aufrufe, Events und Reports gehören nicht hinein.
Die aktive Command-Tabelle bleibt davon getrennt; aus der History verdrängte Commands
bleiben empfangbar und für Recording und Shutdown relevant.

History-Outcomes sind `Completed { output }`, `Rejected { code, message }`,
`ProtocolFailed { code, message }`, `IoFailed { message }` und `Unanswered`.
Sie enthalten keine Request-IDs. Die öffentliche History-Darstellung erscheint im Report-Kontext.

Die Session puffert 256 normale Events und reserviert einen zusätzlichen Platz für `Ended`.
`try_receive_event` fragt nicht blockierend ab, `receive_event` wartet. Die Varianten sind:

- `ProtocolError { code, message }` für nicht einem offenen Command zuordenbare Protokollfehler,
- `RecordingFailed { path, message }`,
- `Failure { failure }`,
- `ObservationError { code, message }`, bei beschädigten Markern `invalid_report_marker`,
- `Ended { reason }`.

Endgründe sind `ProcessExit { status }`, `TransportClosed { channel }`,
`TransportFailed { channel, message }` und `EventQueueOverflow { capacity, dropped_events }`.
Kanäle sind stdin, stdout und stderr. Auch ein unerwarteter Exit-Status 0 ist ein Prozessende.
EOF nach bereits beobachtetem Prozessende gehört zum abschließenden Drain.

Bei einem unerwarteten Ende verarbeitet die Session letzte Responses und Marker aus den Pipes,
schließt Recording nach Möglichkeit ab und reiht `Ended` nach den übrigen Events genau einmal
ein. Bereits terminale Ergebnisse bleiben einmal abrufbar, offene Commands liefern `Ended`
und bleiben in der History `Unanswered`. Der laufende Replay-Start erhält stattdessen seinen
technischen `Blocked`-Abschluss. Nach Entnahme von `Ended` ist weiterer Event-Empfang beendet.

Bei Event-Überlauf stoppt Annahme, die Session beendet und sammelt die Prozessgruppe ein und
zählt verlorene Events. Auch ein nicht mehr speicherbares `RecordingFailed` zählt zum Verlust.
Der reservierte terminale Platz bleibt erhalten. Ein eigener Abbruch ersetzt den primären
Transport- oder Überlaufgrund nicht und erzeugt keinen zusätzlichen Prozessfehler-Report.
Bei Verlust des Koordinators liefern fallible Operationen `Ended`; ein Event ist dann
mangels Produzent nicht garantiert.

| Fehlergruppe | Wirkung |
| --- | --- |
| `InvalidConfig` | Ungültige Startkonfiguration, kein Session-Handle |
| `Launch` | Start oder Cargo-Auflösung erreicht keinen gültigen Handshake; Ressourcen bereinigen |
| `Io` | Lokale Datei-/Betriebssystemoperation des Aufrufs fehlgeschlagen |
| `InvalidPending` | Nur dieser Empfangsaufruf scheitert |
| `RequestIdExhausted` | Keine neue normale Annahme; bestehende Arbeit bleibt |
| `ShutdownBlocked` | Kein Shutdown angenommen; Zustand unverändert |
| `Rejected` | Nur der angenommene Command fachlich abgelehnt |
| `Protocol` | Zugeordneter Aufruf ohne gültiges Protokollergebnis; beim Handshake Startfehler |
| `Ended` | Prozess, benötigter Transport oder Koordinator terminal |

Ein Datei-I/O-Fehler beendet nicht automatisch die Session. Ein I/O-Fehler auf einer benötigten
Pipe nach Ready ist dagegen terminal, auch wenn ein bestimmter Command ihn entdeckt.

### Einzel-Session-Shutdown und Drop

`Session::shutdown(&mut self)` beginnt nur ohne offene Commands und mit Recorder in `Idle`.
`shutdown_commands_pending` hat Vorrang vor `shutdown_recording_active`. Eine Blockierung
sendet nichts, verbraucht keine ID und erzeugt keinen History-Eintrag. Nicht abgeholte Events,
History und erschöpfte normale IDs blockieren nicht. Arbeit muss vorher ausdrücklich
abgeschlossen oder gestoppt werden.

Nach Annahme verwendet Shutdown einmalig `u64::MAX` und nimmt keine neue Arbeit an.
Nur eine gültige `completed`-Response mit `output: null` und anschließend erfolgreicher
Prozessstatus ergeben Erfolg. Ablehnung ergibt `Rejected`, ungültige zugeordnete Antwort
`Protocol`, selbstständiges Prozess-/Transportende oder erfolgloser Exit `Ended`.
Jeder angenommene Ausgang ist terminal und räumt Ressourcen auf; nur eine Blockierung
vor Annahme lässt einen erneuten Versuch zu.

Erfolgreicher Shutdown erzeugt kein `Ended`-Event. Eine Shutdown-Ablehnung oder ein
zugeordneter Protokollfehler mit anschließend eigenem Abbruch erzeugt ebenfalls kein
zusätzliches Endevent. Selbstständige Prozess-/Transportenden verwenden die Event-Regeln.
Nach erfolgreichem Shutdown liefern fallible Session-Operationen `Ended`; unveränderliche
Capabilities und Pending-Metadaten bleiben lesbar. Der direkte Shutdown hat keinen Timeout.

Session-Drop fordert harten Ressourcenabschluss an. Der Koordinator schließt Pipes, beendet
die Prozessgruppe und sammelt den Kindprozess ein. Drop wartet nicht auf Command-Ergebnisse
und garantiert weder Recording-Footer noch Report-Abschluss. Es erzeugt keine Events
und keine Reports.

## Spiel-Commands

### Explizite Simulation und Input

Nur `tick.warp.start` führt Ticks aus. Ein Tick führt die von der Anwendung konfigurierten
Simulations-Schedules einmal in ihrer vorhandenen Reihenfolge aus und erhöht nach Erfolg
den Tick-Zähler um eins. Die Anwendung besitzt Zeitkonfiguration und Bedeutung eines Ticks;
`woodpecker` überschreibt keine simulierte Tick-Dauer.

Input-Erfolg bedeutet Annahme, Prüfung und Vormerkung für den nächsten ausgeführten Tick,
nicht bereits erfolgte Verarbeitung durch Anwendungssysteme. Mehrere Inputs vor einem Tick
werden gemeinsam bereitgestellt. Input-Outputs sind `()`, auf dem Wire `null`.

Kontrollierte Sessions verwenden ausschließlich virtuelle Eingabegeräte. Jede Session
besitzt ihren eigenen Pointer-, Button- und Tastaturzustand. Commands bewegen weder den
Betriebssystem-Cursor noch benötigen sie Betriebssystem-Fokus. Native Maus-, Touch-,
Tastatur- und IME-Eingaben gelangen nicht in die Bevy-Eingabeverarbeitung der Simulation.
Auch nativer Fokusverlust darf gehaltene virtuelle Tasten nicht freigeben.
Mehrere Clients derselben Session teilen sich deren Geräte; mehrere Pointer innerhalb
derselben Session und Mischbetrieb mit nativer Eingabe sind nicht vorgesehen.
Ohne Session-Plugin bleibt die native Bedienung unverändert.

Der virtuelle Pointer verwendet Bevys Picking-Eingabe mit eigener Pointer-Identität.
Seine Position steht am zugehörigen `PointerLocation`, nicht am Betriebssystemfenster.
Bevys UI-`Interaction` muss ebenfalls diesen Pointer verwenden. UI-Layout und
Picking erhalten Größe und Kamera vom Bildziel, nicht von einem nativen Fenster.
Direkte Betriebssystemabfragen und `RawWinitWindowEvent` sind keine kontrollierten
Eingabekanäle. Falls eine Anwendung zusätzlich native Fenster besitzt, dürfen
deren Fokus und Sichtbarkeit die virtuelle Eingabe nicht bestimmen.
Textfokus bezeichnet ausschließlich den Fokus innerhalb des jeweiligen Bevy World.

Keyboard verwendet eigene stabile, layoutunabhängige Key-Tokens mit fester interner Abbildung
auf physisches `KeyCode` und logisches Bevy-`Key`.
Pointer verwendet das Bildziel. `MoveTo` benutzt logische Pixel vom linken oberen Rand,
`MoveBy` ein Delta zur bekannten Position. Das Ziel muss `0 <= x < width` und
`0 <= y < height` erfüllen; Ablehnung verändert die Position nicht.
Relative Mausbewegung zur Blicksteuerung muss auch ohne Cursorposition und
Cursor-Capture des Betriebssystems möglich sein. Sie ist von der begrenzten
Pointerverschiebung `MoveBy` zu unterscheiden. Ihr Command-Format und ihre
Bevy-Zustellung werden im Headless-Durchstich festgelegt; die bestehende
`MoveBy`-Semantik darf nicht stillschweigend umgedeutet werden.
Buttons sind `left`, `right`, `middle` und wirken an der aktuellen Position.
Scroll übergibt horizontales und vertikales Delta samt Vorzeichen in Bevy-Zeileneinheiten;
`[0,0]` ist erlaubt. Text geht an den eindeutigen lebenden, editierbaren Bevy-Fokus
und umfasst höchstens 16.384 UTF-8-Bytes.

Text bindet sich bei der Annahme an die fokussierte `EditableText`-Entity.
Ein späterer Fokuswechsel leitet angenommenen Text nicht in ein anderes Feld um.
Erst beim nächsten Tick erhält dieses Feld den vorgemerkten Edit; Bevys normale
Selektions-, Zeichenfilter- und Längenregeln bestimmen dessen Verarbeitung.
Ist das Feld dann nicht mehr vorhanden oder editierbar, wird nichts an ein anderes
Feld zugestellt. Der bestätigte Command-Erfolg bleibt eine Vormerkungsbestätigung.
Leerer Text ist zulässig, er benötigt dieselben UI-Fokusprüfungen.
Keyboard und Text benötigen kein natives Fenster.

### Tick-Warp

`Start { ticks, pace }` verlangt positive `u64`-Ticks. Die optionale Pace überschreibt
nur diesen Warp; sonst gilt `tick::Config::pace`. Pace ist `AsFastAsPossible` oder
`TicksPerSecond { target }` mit endlichem `target > 0`. Sie steuert ausschließlich reale
Geschwindigkeit, nicht simulierte Zeit, und verspricht keine erreichbare Leistung.

`SetPace` setzt den Laufzeitstandard und gilt sofort für verbleibende Ticks eines aktiven Warps.
`Stop` beendet den Warp, ohne laufenden Warp ist es ein erfolgreicher No-op.
Ein zweiter Start während eines Warps wird abgelehnt.

Start bleibt bis zum Ende ausstehend und liefert `requested_ticks`, `executed_ticks` sowie
`completed` oder `stopped`. SetPace bestätigt die übernommene Pace; Stop liefert `was_running`.
Auch ein maximal schneller Warp bleibt zwischen begrenzten Arbeitsabschnitten steuerbar.

### Inspect

Inspect liest den aktuellen Bevy World ohne Mutation, Tick oder Zeitfortschritt.
Entities einschließlich Children und Resources sind getrennte Quellen.
Interne Resource-Entities gehören nicht in Entity-Ergebnisse.

Entity-Queries besitzen optional einen sessionlokalen Handle, `with`/`without`-Componentfilter
und eine Projektion: Summary, Component-Namen, Component-Werte mit `All` oder `Listed`,
oder Hierarchie mit `depth: u8`. Resource-Queries wählen `All` oder einen vollständigen
Type Path und liefern `Metadata` oder `Value`.

Alle vom Aufrufer gelieferten Type Paths müssen vor dem World-Zugriff exakt über die
TypeRegistry auflösbar sein. Sonst wird die ganze Query mit `unknown_type_path` abgelehnt.
Ein nicht existierender expliziter Handle ergibt `entity_not_found`.
Eine Suche ohne Treffer liefert erfolgreich `items: []`.

Metadatenprojektionen serialisieren keinen Wert und liefern keinen Lesbarkeitsstatus.
Angeforderte Werte sind `Readable { value }` oder `Unavailable { status }`:
`Missing`, `NotRegistered`, `NotReflectable`, `NotSerializable`.
`NotRegistered` betrifft entdeckte Werte mit fehlender benötigter Registrierung,
nicht unbekannte Type-Path-Eingaben.

`Listed` liefert jeden registrierten angeforderten Component in Eingabereihenfolge,
bei Abwesenheit `Missing`. `All` liefert vorhandene Components.
Resource-`All` liefert vorhandene, für reflektierten Resource-Zugriff registrierte Resources.
Ein explizit gewählter registrierter, fehlender Resource-Typ liefert bei `Value` einen
`Missing`-Eintrag, bei `Metadata` keinen Eintrag.

Die Query liefert alle passenden Items gemeinsam und ungekürzt. Sortierung:

- Entities nach `index`, dann `generation`,
- Resources nach `type_path`,
- Component-Metadaten und `All` nach `type_path`, dann `name`, ohne Type Path über `name`,
- `Listed` in Eingabereihenfolge,
- Hierarchie-Kinder in Bevy-`Children`-Reihenfolge.

Lesbare Werte folgen Bevys `TypedReflectSerializer` als `serde_json::Value`.
Ein privater Reflect-Processor behandelt die folgenden Fälle einheitlich:

- Direkt reflektierte `f32`/`f64` müssen endlich sein. Andernfalls wird der gesamte betroffene
  Component-/Resource-Wert `NotSerializable`. Ein vom opaken Custom Serializer erzeugtes
  `null` bleibt dessen Ausgabe.
- Maps bleiben JSON-Objekte. Unterstützte skalare Schlüssel werden Strings;
  nicht darstellbare Schlüssel machen den umgebenden Wert `NotSerializable`.
- Sets bleiben Arrays, sortiert nach den Bytes des kompakten JSON jedes Elements mit
  rekursiv alphabetisch geordneten Objektschlüsseln. Das gilt auch für verschachtelte Sets.
  Ein fehlerhaftes Element macht den umgebenden Wert `NotSerializable`.
- Asset-Handles mit Pfad oder UUID folgen `HandleSerializeProcessor`. Flüchtige Handles
  werden `{"Ephemeral":{"id":"<16 kleine Hex-Zeichen>"}}` aus `AssetIndex::to_bits()`.
  Das Token gilt nur innerhalb der Session. Untyped Handles enthalten zusätzlich Asset-Type-Path
  und Referenz wie `TypedHandleReference`. Fehlende nötige Registrierung ergibt
  `NotSerializable`.

Ein privater Inspect-Adapter darf geeignete öffentliche read-only Built-in-Handler aus
`bevy_remote` nutzen. Er validiert vorher die strikten Type Paths und ergänzt benötigte
Unavailable-Diagnosen, statt von Bevy übersprungene Werte zu verlieren.
Er installiert keine Remote-Plugins oder erreichbaren Mutationshandler.
BRP-Typen und IDs bleiben intern. Hierarchie und nicht abgedeckte Regeln bleiben im Adapter.
Die direkte `bevy_remote`-Abhängigkeit verwendet dessen HTTP-Default-Feature nicht.
Fixtures gegen Bevy 0.19.1 sichern den Vertrag bei Upgrades.

### Screenshot

`Capture { path }` nimmt das primäre Bildziel einschließlich seiner zugeordneten
Kamera- und UI-Ausgaben auf. Die Response wartet auf Renderdurchlauf,
asynchronen GPU-Readback und erfolgreiches PNG-Schreiben.
Simulationsticks und simulierte Zeit bleiben unverändert.
Ohne vollständige Unterstützung oder eindeutiges Bildziel wird der Command abgelehnt.
Erfolg erfordert die Zuordnung der gerenderten Ausgabe, GPU-Kopie und Bilddaten
zum Request. Ein vorbereiteter Buffer oder ein erfolgreiches Mapping allein
genügt nicht. Eine tatsächlich schwarze Szene bleibt gültig; Pixelwerte sind
keine allgemeine Bereitschaftsprüfung.

Der Adapter benötigt begrenztes Warten auf seine Render-/Readback-Voraussetzungen
mit ausdrücklichem Fehlerabschluss, ohne Simulationsticks. Die beobachtbaren
Bereitschaftskriterien für Assets, Pipelines und Kameraausgabe müssen im
Durchstich festgelegt und getestet werden. Eine pauschale Anzahl Startframes
oder wiederholte Captures bis zum ersten passenden Bild sind kein Zielverhalten.
Session-`Ready` bestätigt den Handshake, nicht automatisch die Bereitschaft
jedes späteren Bildes.

Der konkrete Pfad ist normalisiert, nicht leer, UTF-8, relativ zum Artefakt-Root und endet auf
`.png`. Er verwendet `/`, keine leeren Komponenten, `.`/`..` oder Backslashes und darf
auch über Symlinks nicht aus dem Root ausbrechen. Der Aufrufer liefert den fertigen Dateinamen.
Eine vorhandene Datei wird ersetzt. Erfolg enthält relativen `path`, `width`, `height`
und `overwritten`.

## Recording und Replay

### Recording-Ausführung

`recording.start { path }` und `recording.stop` werden vom Session-Koordinator ausgeführt.
Sie begrenzen einen Abschnitt der laufenden Session, ohne Spielzustand oder Zeit zu ändern.
Aufgenommen werden die tatsächlich ausgeführten Input-, Tick-, Inspect- und Screenshot-Commands
samt Outcomes, einschließlich interner Replay-Plan-Commands. Recording-/Replay-Steuerung,
Shutdown, Events und Reports gehören nicht in die Datei.

Der Recorder besitzt `Idle`, `Starting`, `Active`, `Stopping`.
Start ist nur in `Idle`, Stop nur in `Active` zulässig, jeweils ohne früher angenommene
offene Commands. Die Übergänge halten später angenommene Ausführungen als Barriere zurück.
Nach angelegter Datei und geschriebenem Header wird Start mit `Started { path }` beantwortet;
danach gehören freigegebene Commands zum Recording. Ein fehlgeschlagener Start gibt sie ohne
Recording frei. Stop schreibt Footer, leert, synchronisiert und schließt die Datei, bevor
`Stopped { path, recorded_commands }` antwortet und wartende Commands ohne Recording weiterlaufen.

Stabile Ablehnungen sind `recording_already_active`, `recording_not_active`,
`recording_transition_in_progress`, `recording_commands_pending`, `invalid_recording_path`
und `recording_path_exists`. Sie folgen auf Annahme; Start-/Stop-Dateifehler ergeben `Io`.
Fertige Einträge werden bis zu früheren noch offenen Outcomes gepuffert und dann in
Annahmereihenfolge geschrieben.

Ein Schreibfehler während `Active` beendet nur die Aufnahme, belässt die unvollständige Datei,
kehrt nach `Idle` zurück und erzeugt `RecordingFailed`. Das Spiel-Command-Outcome bleibt
unverändert. Scheitert bereits der Header, versucht der Recorder die neue Datei zu entfernen.
Bei unerwartetem Session-Ende versucht er offene aufgenommene Commands als `unanswered`
und einen `session_ended`-Footer zu schreiben. Ein Fehler dabei erscheint vor `Ended`.

### Recording-Dateivertrag

Recording und Replay verwenden normalisierte relative UTF-8-Pfade auf `.jsonl` unter dem
Artefakt-Root: `/`, nicht leer, keine leeren Komponenten, `.`/`..`, Backslashes oder Symlinks.
Der Recorder legt fehlende Eltern an und die endgültige Datei mit `create_new`; er überschreibt
nichts. Der Replay-Loader öffnet relativ zum geöffneten Root, ohne Symlinks zu folgen,
und verlangt eine reguläre Datei. Eine vorgezogene Prüfung allein reicht wegen Pfadaustausch
zwischen Prüfung und Öffnen nicht.

Eine Datei enthält genau einen Abschnitt in eigenständig versioniertem JSONL:

```jsonl
{"type":"recording_started","format_version":1}
{"type":"command","command":"input.keyboard.press","arguments":{"key":"space"},"outcome":{"status":"completed","output":null}}
{"type":"recording_ended","outcome":"stopped","recorded_commands":1}
```

Header zuerst, null oder mehr Command-Objekte, genau ein Footer zuletzt.
Jede Zeile ist ein JSON-Objekt; Leerzeilen, zusätzliche Daten, unbekannte oder doppelte Felder
und unbekannte `type`-Werte sind ungültig. Ein abschließendes LF ist optional.
`recorded_commands` muss exakt stimmen. Eine Datei ohne vollständigen Footer ist ungültig.
Commands und Outcomes werden ungekürzt ohne Request-IDs gespeichert und können vertraulich sein.

Outcome-Formen:

```json
{"status":"completed","output":null}
{"status":"rejected","error":{"code":"entity_not_found","message":"..."}}
{"status":"protocol_failed","error":{"code":"invalid_response","message":"..."}}
{"status":"io_failed","error":{"message":"..."}}
{"status":"unanswered"}
```

`completed.output` muss zur konkreten Command-Variante passen.
`unanswered` ist nur mit Footer-Outcome `session_ended` erlaubt; dieser darf auch terminale
Command-Outcomes enthalten. Der Adapter akzeptiert ausschließlich aufnehmbare Commands
und validiert ihre Argumente.

Der Loader prüft zuerst den Header. Ein ungültiger Header oder syntaktisch ungültige Version
ergibt `invalid_recording`; eine syntaktisch gültige, nicht unterstützte Version ergibt
`unsupported_recording_version`, ohne spätere Zeilen als Version 1 zu interpretieren.
Ungültige Pfade, Symlinks und nicht reguläre Ziele ergeben `invalid_recording_path`.
Fehlende, nicht lesbare oder beim Lesen fehlerhafte Dateien ergeben `Io`.
Die Meldung nennt Pfad und soweit bekannt Zeile; diese Ladefehler erzeugen kein Session-Event.

### Replay-Ausführung

Replay führt eine unterstützte Recording in der aktuellen Session aus. Der Aufrufer
verantwortet den Ausgangszustand. Die Aufzeichnung ist ein Command-Verlauf, kein World-Snapshot.
Der Abschluss bewertet die Verarbeitung des Plans, nicht die Gleichheit mit früheren Outputs.

Ein Start reserviert in `Idle` die Ausführung als `Preparing`. Eine private Ladeaufgabe
validiert die gesamte Datei und erstellt den vollständigen typisierten Plan vor dem ersten
Spiel-Command. Persistierte Versionstypen bleiben im Ladeadapter. Erfolg wechselt nach `Running`,
ein Ladefehler nach `Idle`. Weitere Starts werden in allen aktiven Zuständen mit
`replay_already_running` abgelehnt. Von außen ist während `Preparing`, `Running` und `Stopping`
nur Replay-Stop fachlich zulässig; sonst gilt `replay_in_progress`.

Ein erfolgreich beantworteter aufgezeichneter Warp liefert seine effektiven `executed_ticks`.
Der Loader prüft passende `requested_ticks`, `executed_ticks <= requested_ticks` und bei
`Completed` Gleichheit. Null effektive Ticks entfallen; der zugehörige aufgezeichnete Stop
wird nicht erneut ausgeführt. Ohne erfolgreiches aufgezeichnetes Warp-Outcome bleibt der
ursprüngliche Warp im Plan. Unmittelbar benachbarte Warps mit gleicher effektiver Pace dürfen
ohne zwischenliegenden Command oder Pace-Wechsel addiert werden, sofern `u64` nicht überläuft.
Andere aufgezeichnete Outcomes werden nur validiert, nicht als Erwartungen verwendet.

Zwischen Warps darf Replay mehrere Nicht-Tick-Commands in Dateireihenfolge einreichen.
Vor dem nächsten Warp müssen alle vorherigen Outcomes terminal sein; ein Warp muss
abschließen, bevor der Plan weiterläuft. `Completed` und fachliches `Rejected` erfüllen
die Barriere. Technische Fehler des neuen Laufs blockieren ihn.

Start antwortet nach vollständiger Verarbeitung mit `Completed`, nach kontrolliertem Stop
mit `Stopped` oder bei technischer Blockierung mit `Blocked { code, message }`.
Blockierungscodes sind:

| Code | Ursache |
| --- | --- |
| `command_protocol_failed` | Keine gültige Response eines Plan-Commands |
| `session_io_failed` | I/O verhindert geordnete weitere Ausführung |
| `session_ended` | Session oder benötigter Transport beendet |
| `request_id_exhausted` | Interner Plan-Command kann nicht angenommen werden |
| `stop_failed` | Aktiver Warp bei fortbestehender Session nicht kontrolliert stoppbar |

Stop gibt sofort keine weiteren Plan-Commands frei und stoppt einen aktiven Replay-Warp
intern mit Warp-Stop. Bereits gesendete, nicht stoppbare Commands laufen aus.
Erst dann erhalten Start `Stopped` und wartende Stops `was_running: true`.
Weitere Stops in `Stopping` warten auf denselben Abschluss; in `Idle` liefern sie sofort
`was_running: false`. Technische Blockierung hat Vorrang; wartende Stops erhalten dann
den auslösenden Session-Fehler. Bereits ausgeführte Wirkungen bleiben bestehen.

Stop in `Preparing` lässt die Ladeaufgabe ihren Abbruch bestätigen, bevor Replay wieder
`Idle` wird. Trifft ein Ladefehler vorher beim Koordinator ein, beendet dieser den Start
und der folgende Stop ist ein No-op. Laden und Stop erzeugen keine Ticks.

## Reporting

### Erkennung

`report::Observer` deutet geordnete stderr-Bytes und unerwarteten Prozessstatus, die ihm
`session` liefert. Die Session entscheidet, ob sie selbst eine Beendigung veranlasst hat.
Der Observer besitzt Zeilenpuffer und versioniertes Chunk-Framing mit Event-ID, Chunk-Index,
Chunk-Anzahl, Base64-JSON und Prüfsumme. Jeder begrenzte Chunk wird in einem Schreibaufruf
ausgegeben. Gültige Marker werden aus der menschlichen stderr-Ausgabe entfernt; alle anderen
Bytes bleiben unverändert. Beschädigte oder bei EOF unvollständige Marker erzeugen
`invalid_report_marker`.

Das Plugin verkettet den vorhandenen prozessweiten Panic-Hook. Der Hook schreibt synchron
einen Marker und erfasst `Backtrace::force_capture` im betroffenen Thread. Payload, Location,
Erfassungsstatus und rohe Display-Ausgabe bleiben getrennt. Ein unbekannter `panic_any`-Payload
kann ohne Meldung erscheinen. Die Anwendung darf den Hook danach nicht ersetzen.
Panics in Systemen, Tasks und Workern sind mit `unwind` und `abort` beobachtbar;
genaue oder buildübergreifend stabile Backtrace-Frames sind nicht garantiert.

`report.tracing_errors` ist standardmäßig aus. Bei Aktivierung registriert die Anwendung
`session::tracing_error_layer` über `LogPlugin::custom_layer`, bei eigenen Layern ausdrücklich
zusammengesetzt. Nur dispatchte Error-Level-Events werden vor dem Formatter erfasst.
Die Einstellung verändert keine Filter. Das Plugin sendet vor Ready einen internen
Layer-Statusmarker; bei aktivierter Beobachtung benötigt Start dessen positive Bestätigung,
unabhängig von der beobachteten Reihenfolge zu stdout-Ready.

Jeder vollständige Panic-Marker und jeder aktivierte Tracing-Marker liefert ein Failure.
Ein unerwartetes Prozessende liefert nach dem Pipe-Drain `ProcessExit`, sofern kein Panic
derselben Session erkannt wurde. Auch Status 0 ist unerwartet, wenn die Session das Ende
nicht absichtlich ausgelöst hat. Ein beschädigter Panic-Marker belegt keinen Panic.
Vor erfolgreichem Ready bleiben Fehler Startdiagnose, weil der Report-Kontext noch nicht
vollständig vorliegt. Ein behandeltes `Err` ohne beobachtbaren Fehlerauslöser erzeugt keinen Report.

### Konstruktion und Kontext

`Report::create(failure, &session)` ist unfehlbar und erzeugt Titel, Signatur und Kontext
gemeinsam. Die Felder von Report, Failure und Signature sind privat, mit lesenden Zugriffen.
Failure-Konstruktoren sind Panic mit optionaler Meldung, Tracing-Error und Prozessende.
Prozessende verwendet die feste Meldung `process exited unexpectedly`; der Status bleibt Diagnose.

Der Context kopiert beim Create-Aufruf die History sowie die beim Session-Start aufgenommenen
Metadaten:

- Application: Package, Cargo-Package-Version, Binary/Example, Features, Argumente und optionale
  Git-Revision aus dem Repository des ausgewählten Package-Manifests,
- Git: `HEAD` und Dirty einschließlich vorgemerkter, geänderter und nicht ignorierter
  unversionierter Dateien; ohne Git oder HEAD keine Revision,
- Version des laufenden `woodpecker`-Builds, bestätigte Protokollversion und Capabilities,
  Tick-Konfiguration vom Start, OS und Architektur,
- optionale getrimmte Cargo-/Rustc-Versionsausgaben aus demselben Startarbeitsordner,
- die begrenzten korrelierten History-Einträge.

Optionale Zusatzabfragen verhindern weder Session-Start noch Report. Spätere Outcomes,
Datei-, Branch- oder Toolchain-Änderungen verändern einen erzeugten Report nicht.
Absolute Projekt-/Manifest-/Artefaktpfade, Umgebung, Repository-URL, Branch, Diffs,
Session-/Request-IDs, Provider-Konfiguration und vollständige Cargo-Metadaten gehören nicht
als Kontextfelder hinein. Unveränderte Argumente, Commands und Outputs können solche
vertraulichen Inhalte dennoch selbst enthalten. Reports sind vertrauliche Artefakte.

### Titel, Markdown und Fehlersignatur

Der Titel vereinheitlicht CRLF/CR zu LF, entfernt vollständige ANSI-Sequenzen und verwendet
die erste nach äußerem ASCII-Trim nicht leere Meldungszeile. Interner Whitespace, Großschreibung
und Unicode bleiben erhalten. Über 120 Unicode-Skalarwerten wird nach 117 mit `...` gekürzt.
Ohne verwendbare Zeile gilt `panic`, `tracing error` oder `process exited unexpectedly`.
Die ursprüngliche Meldung bleibt unverändert.

`to_markdown` liefert die gemeinsame Darstellung für Datei und Issue-Body:
Titel als H1, vollständiger Signatur-Marker, Failure, Application, Environment, Commands.
Failure enthält vorhandene Ursprungsdiagnosen; fehlende Angaben sind ausdrücklich nicht verfügbar.
Meldung und Backtrace stehen in Text-Codeblöcken, strukturierte Daten in eingerücktem JSON.
Jeder Fence hat mindestens drei Backticks und ist länger als jede Backtick-Folge der Nutzdaten.
Der Text verwendet LF und endet mit genau einem LF. Er enthält keinen Erzeugungszeitpunkt
oder Provider-Zustand und kürzt Nutzdaten nicht.

Die Signatur lautet `v1:sha256:<64 kleine Hex-Zeichen>`. Ihre zwei geordneten Felder sind
`kind` mit `panic`, `tracing_error` oder `process_exit` und `message`.
Andere Diagnosen beeinflussen die Identität nicht. Für die Signatur wird eine Meldungskopie
in dieser Reihenfolge normalisiert:

1. CRLF und CR zu LF; vollständige ANSI-Sequenzen per Parser entfernen.
2. Absoluten Projektwurzelpfad in beiden Trenner-Schreibweisen durch `<project>` ersetzen.
3. Eigenständige ASCII-Tokens `0[xX][0-9a-fA-F]{8,16}` durch `<address>` ersetzen.
   Angrenzende ASCII-Buchstaben, Ziffern oder `_` verhindern den Treffer.
4. Gültige eigenständige ISO-Daten `YYYY-MM-DD`, Zeiten `HH:MM:SS` mit optionalem
   Sekundenbruchteil und Zeitzone sowie Kombinationen mit `T` oder Leerzeichen durch
   `<timestamp>` ersetzen. Kalenderdatum, Zeit und numerischer UTC-Offset müssen gültig sein;
   längste kombinierte Form zuerst. Dieselben ASCII-Token-Grenzen gelten.
5. Äußeren ASCII-Whitespace entfernen; übrige Zahlen, Unicode, Schreibweise und inneren
   Whitespace erhalten.

Fehlende Panic-Meldung verwendet den leeren String. Tracing verwendet das konventionelle
`message`-Feld, sonst übrige Felder in Deklarationsreihenfolge als `name=value`, getrennt
durch `, `; ohne Felder den Metadatenname. Primitive Werte verwenden kanonische
Marker-Darstellung, andere die beobachtete `record_debug`-Darstellung.

Der Hash-Preimage ist `bug_hunter.signature` in ASCII mit Nullbyte, dann Version als
Big-Endian-`u32`, Feldanzahl als Big-Endian-`u32`, dann `kind` und `message`.
Je Feld folgen UTF-8-Namenslänge als Big-Endian-`u32`, Name, UTF-8-Wertlänge als
Big-Endian-`u64`, Wert. SHA-256 liefert den Digest.

| kind | Normalisierte message | Digest |
| --- | --- | --- |
| `panic` | `stellar catalog invariant violated` | `e539c36fcbeb8357ba855f705c08274f29e82d866288dffaa1406607eba9302c` |
| `panic` | leer | `f7ad1a8211ca86c794de8c16b456c16006393c974b89e5aa11908ff1f35336a6` |
| `tracing_error` | `stellar catalog invariant violated` | `86a70a6857c6f193019c24bf0d04b01bde9f127e28b5c45c8f021de0c9165e46` |
| `process_exit` | `process exited unexpectedly` | `7ca80a848b0913a8eb5ab37f815efe34cd2c4bcdc46ad2fd1870c08902c3f240` |

Provider vergleichen die volle Signatur einschließlich Version und Algorithmus.
Eine Änderung von Normalisierung oder Feldbelegung benötigt eine neue Version;
bestehende Reports werden nicht rückwirkend neu berechnet.

Die Umbenennung in `woodpecker` ändert weder den v1-Hash-Namensraum
`bug_hunter.signature` noch den Marker `bug_hunter-signature`. Diese Bezeichner
gehören zum versionierten Dateivertrag, nicht zum aktuellen Produktnamen.

### Provider

`report::Config` wählt genau `Local` oder `Github` und einen nicht leeren relativen
`output: PathBuf` aus normalen Komponenten unter dem Artefakt-Root.
Absolute Pfade, `.` und `..` sind ungültige Konfiguration.
`submit(&report, &session)` verwendet ausschließlich diese beim Start wirksame Auswahl.
Die Wahl von Github ist die ausdrückliche Veröffentlichungsentscheidung.
Die Anwendung veröffentlicht damit ohne zusätzliche Rückfrage oder Freigabe pro Report.
`Local` veröffentlicht nichts auf GitHub.

Local schreibt den vollständigen Markdown-Text nach
`<output>/v1-sha256-<digest>.md`. Referenzen sind `PathBuf` relativ zum Artefakt-Root.
Vorhandene Pfadkomponenten und Zieldateien dürfen keine Symlinks sein.
Eine neue temporäre Datei im Zielverzeichnis wird nach vollständigem Schreiben ohne
Überschreiben am endgültigen Pfad eingesetzt. Eine vorhandene Datei mit demselben
Signatur-Marker ergibt `Existing`, mit fehlendem oder anderem Marker `Conflict`.
Bei paralleler Erstellung liest der unterlegene Aufruf die endgültige Datei erneut.

Github hat keine eigenen Konfigurationsfelder. Es führt `gh` in `project_dir` aus;
Git-Kontext und von `gh` unterstützte Umgebung bestimmen Repository, Host und Anmeldung.
Der Body wird nicht interaktiv über stdin übergeben, der Titel unverändert.
Die Duplikatsuche umfasst alle paginierten offenen und geschlossenen Issues und vergleicht
exakt eine eigene Markerzeile:

```markdown
<!-- bug_hunter-signature: v1:sha256:<digest> -->
```

Ein Treffer ergibt `Existing` mit Nummer und URL, ohne Änderung am Issue.
Sonst wird ein Issue erstellt und die Referenz zurückgegeben. Erfolgreiche GitHub-Aufrufe
erzeugen keine lokale Datei. Gleichzeitige Veröffentlichungen können Duplikate erzeugen;
GitHub bietet keine atomare Eindeutigkeit für Body-Marker.

Jeder Remote-Fehler führt zum selben lokalen Speicherweg. Erfolg des Rückfalls ergibt
`Fallback { reference: FileReference, provider_error }`; er ist kein verdeckter lokaler Erfolg.
Scheitert auch Local, enthält `FallbackFailed` beide Ursachen.

`report::Error` umfasst `Local(local::Error)` und `FallbackFailed`.
Local unterscheidet `InvalidPath { path }`, `Conflict { path }` und
`Filesystem { operation: Read|Write, path, message }`.
Github unterscheidet `Unavailable`, `CommandFailed`, `InvalidResponse`, jeweils mit
`Search|Publish` und lesbarer Meldung. Anmeldung oder Netzwerkfehler werden nicht aus
stderr in erfundene Kategorien zerlegt. Alle öffentlichen Fehler implementieren
`Display` und `std::error::Error`; freie Meldungstexte sind kein stabiler Steuervertrag.

## Gemeinsame Command-Kodierung und Spielprotokoll

### Nachrichten

`session::protocol` besitzt UTF-8-JSONL über stdin/stdout zwischen Session und Spiel.
stderr bleibt Diagnose-/Markerkanal. Erste Protokollnachricht ist genau:

```json
{"status":"ready","version":3,"capabilities":{"screenshot":true}}
```

Version muss 3 sein, Screenshot immer Boolean. Fehlende, typfalsche oder zusätzliche
Ready-Felder ergeben `invalid_ready`, andere Version `unsupported_protocol_version`.
Ein weiterer Ready nach erfolgreichem Start ergibt nicht fatal `unexpected_ready`
und ändert den Capability-Snapshot nicht.

Input, Tick und Inspect gehören fest zur Version. `screenshot` bestätigt vollständige
optionale Installation samt Renderer, nicht Erfolg jedes konkreten Aufrufs.
Die Anwendung bestimmt ihre Bevy-Zusammensetzung; das Plugin installiert keinen Renderer.

```json
{"request_id":17,"command":"input.keyboard.press","arguments":{"key":"a"}}
{"request_id":17,"command":"input.keyboard.press","status":"completed","output":null}
{"request_id":18,"command":"input.keyboard.press","status":"rejected","error":{"code":"key_already_pressed","message":"..."}}
{"request_id":null,"status":"protocol_error","error":{"code":"malformed_request","message":"..."}}
```

Ein qualifizierter Name und ein immer vorhandenes `arguments`-Objekt beschreiben den Command.
Wire-Requests verwenden dieselbe ID wie die Session-Annahme. Responses enthalten ID und
denselben Namen sowie ausschließlich Erfolgs- oder Fehlernutzlast. Falscher Name oder nicht
als erwarteter Output dekodierbare Antwort schließt genau das Pending mit `Protocol`.

Lesbare unbenutzte ID mit ungültigem Command ergibt eine korrelierte Ablehnung.
Nicht lesbare ID oder eine bereits ausstehende ID ergibt `protocol_error`, mit gelesener ID
oder `null`. Die Spielseite verarbeitet danach weitere Zeilen.
Bekannte offene ID im Protokollfehler schließt diesen Command; fehlende, unbekannte oder
bereits beantwortete ID erzeugt ein Session-Event und lässt andere Commands offen.

### Command-Formen

| Qualifizierter Name | arguments | Erfolgreicher output |
| --- | --- | --- |
| `input.keyboard.press`, `input.keyboard.release` | `{"key":"<token>"}` | `null` |
| `input.pointer.press`, `input.pointer.release` | `{"button":"left"}`, alternativ `right` oder `middle` | `null` |
| `input.pointer.move_to` | `{"position":[x,y]}` | `null` |
| `input.pointer.move_by`, `input.pointer.scroll` | `{"delta":[x,y]}` | `null` |
| `input.text.input` | `{"text":"..."}` | `null` |
| `tick.warp.start` | `{"ticks":n}` mit optionalem `pace` | `{"requested_ticks":n,"executed_ticks":m,"outcome":"completed"}`, alternativ `stopped` |
| `tick.warp.set_pace` | `{"pace":...}` | `{"pace":...}` |
| `tick.warp.stop` | `{}` | `{"was_running":true}`, alternativ `false` |
| `inspect.query` | nachfolgendes Query-Schema | `{"items":[...]}` |
| `screenshot.capture` | `{"path":"screenshots/current.png"}` | `{"path":"...","width":w,"height":h,"overwritten":true}`, alternativ `false` |
| `shutdown` | `{}` | `null`, geschrieben und geflusht vor Prozessende |

Die Tabelle beschreibt Schemas, nicht durchweg wörtliche JSON-Werte.
Pace wird `{"kind":"as_fast_as_possible"}` oder
`{"kind":"ticks_per_second","target":60.0}`.
Recording und Replay nutzen dieselbe qualifizierte Command-Form, werden aber ausschließlich
in der Session ausgeführt und sind keine zusätzlichen Spiel-Wire-Commands.

Inspect-Argumente:

```json
{"source":"entities","entity":null,"with":[],"without":[],"projection":{"kind":"summary"}}
{"source":"resources","selector":{"kind":"type","type_path":"game::GameState"},"projection":{"kind":"value"}}
```

Entity-Projektionen sind `summary`, `component_names`,
`{"kind":"components","selection":{"kind":"all"}}`,
`{"kind":"components","selection":{"kind":"listed","type_paths":[...]}}` und
`{"kind":"hierarchy","depth":3}`. Resources verwenden `metadata`/`value`,
Selektoren `all`/`type`. Handles sind `{"index":7,"generation":1}`.
Optionale Inspect-Felder sind ausdrücklich `null`, Listen immer Arrays.
Unbekannte oder zur anderen Source gehörende Felder werden abgelehnt.

Verschachtelte Enum-Kinds verwenden `snake_case`. Beispiele für Entity-Summary und
Resource-Value:

```json
{"items":[{"kind":"entity","entity":{"index":7,"generation":1},"result":{"kind":"summary","name":"Player","component_count":8}}]}
{"items":[{"kind":"resource","result":{"kind":"value","type_path":"game::GameState","value":{"status":"readable","value":{"score":42}}}}]}
```

Items verwenden `kind: "entity"` mit `entity` und `result`, beziehungsweise
`kind: "resource"` mit `result`. Resultate tragen das `kind` ihrer Projektion.
Component-Metadaten enthalten `name` und optionalen `type_path`.
Hierarchieknoten enthalten Handle, optionalen Namen und Children.
Value-Outputs sind ausschließlich
`{"status":"readable","value":...}` oder
`{"status":"unavailable","reason":"missing|not_registered|not_reflectable|not_serializable"}`.

### Fachliche Ablehnungscodes des Spielprotokolls

`invalid_arguments` gilt bei fehlenden, strukturell falschen oder zusätzlichen Argumentfeldern.
Zahlenpaare müssen endlich sein. Fehlercodes unterscheiden notwendige Reaktionen eines
Aufrufers; technische Details bleiben in `message`.

| Bereich | Weitere stabile Codes |
| --- | --- |
| Keyboard | `invalid_key`, `keyboard_window_unavailable`, `key_already_pressed`, `key_not_pressed` |
| Pointer-Bewegung | `pointer_window_unavailable`, `pointer_location_unavailable`, `pointer_position_out_of_bounds` |
| Pointer-Buttons | `invalid_pointer_button`, `pointer_location_unavailable`, `pointer_button_already_pressed`, `pointer_button_not_pressed` |
| Pointer-Scroll | `pointer_window_unavailable`, `pointer_location_unavailable` |
| Text | `text_too_large`, `text_window_unavailable`, `text_focus_unavailable` |
| Warp-Start | `invalid_tick_count`, `invalid_pace`, `warp_already_running` |
| Warp-SetPace | `invalid_pace` |
| Inspect | `unknown_type_path`, `entity_not_found` |
| Screenshot | `invalid_screenshot_path`, `screenshot_window_unavailable`, `screenshot_unavailable`, `screenshot_failed` |

Die oben aufgeführten `*_window_unavailable`-Codes beschreiben noch die
vorhandenen Fensteradapter. Vor der Headless-Integration sind ihre Ablösung
für fehlende Bildziele, betroffene Clients und die Protokollversionierung
festzulegen. Sie dürfen im Zielbetrieb nicht wegen eines fehlenden nativen
Fensters zurückgegeben oder ohne dokumentierte Migration umgedeutet werden.
Die folgenden Regeln beschreiben den bis dahin geschützten Fensterpfad:
Fensterfehler umfassen fehlende oder nicht eindeutige benötigte Fenster.
`screenshot_window_unavailable` umfasst auch eine im Capture-Frame nicht verfügbare
Fenster-Renderoberfläche. Ein nicht gefüllter Readback darf nicht als Erfolg gespeichert werden.
`screenshot_failed` fasst Readback-, PNG- und Schreibfehler zusammen.
Warp-Stop und Wire-Shutdown haben außer `invalid_arguments` keinen fachlichen Ablehnungscode.
Shutdown-Vorbedingungen werden vor dem Wire geprüft.

## CLI, REPL und Script

Die CLI ermöglicht Serverstart/-ende, Session-Erstellung, Liste und Detail, feste
Session-Auswahl, Command-Einreichung und Beobachtung. Ihr privater Config-Parser verarbeitet
versionierten TOML-Inhalt ohne Dateizugriff. Dateilesen liegt im aufrufenden CLI-Ablauf.
Unbekannte Felder und nicht unterstützte Versionen werden abgelehnt; Persistenzversionen
werden nicht in Laufzeitmodelle übernommen. Fachliche Config-Prüfung bleibt bei den Ownern.
CLI-Einstiege geben Ergebnisse zurück; die einbettende `main` gibt den ExitCode zurück.

Die Verwaltungs-CLI folgt grundsätzlich der Docker-artigen Form
`woodpecker <ressource> <aktion> [optionen] [id]`. Die Session-Verwaltung verwendet:

```text
woodpecker session create …
woodpecker session ls
woodpecker session inspect <id>
woodpecker session stop <id>
```

Diese Syntax organisiert Verwaltungsoperationen. Ihr Verhalten folgt dem Session-Vertrag:
`create` legt eine Session an und stößt ihren Spielstart an; `inspect` liest den
Verzeichniseintrag. Spielzustand wird dagegen über den Spiel-Command `inspect.query` gelesen.
Die Docker-Anlehnung legt keine zusätzlichen Operationen oder Lebenszyklusregeln fest.
Konkrete Optionen und Ausgabeformate werden bei der CLI-Implementation ausgearbeitet.

Agenten verwenden einzelne maschinenlesbare CLI-Aufrufe zum Erstellen oder Auswählen einer
Spiel-Session, zum Einreichen von Commands und zum Abfragen oder begrenzten Abwarten ihrer
Ergebnisse. Der Nutzer startet Codex oder eine andere Agent-Laufzeit selbst.
Modellzugang, Gespräch und Werkzeugschleife gehören dieser externen Laufzeit.
Die Agent-Sitzung ist von der Spiel-Session getrennt. Die menschliche REPL und der Agent
verwenden denselben Client-Vertrag und können dieselbe Spiel-Session steuern.
Jeder abgeschlossene CLI-Aufruf liefert dem Agenten Daten für seine nächste Entscheidung;
er muss keinen dauerhaft offenen interaktiven REPL-Prompt bedienen.

Nicht dekodierbare Eingaben erreichen die Session nicht und verbrauchen weder Request-ID
noch History-/Recording-Eintrag. Maschinenlesbare Command-Eingabefehler unterscheiden
`invalid_json` einschließlich ungültigem UTF-8 von `invalid_command` für syntaktisch gültige,
aber strukturell unzulässige Commands. Fachliche Fehler dekodierter Commands folgen
der normalen Session-Annahme.

Die REPL bleibt während ausstehender Commands ansprechbar. Sie zeigt Request-ID und Namen
als Pending, später das korrelierte Ergebnis. `help`, `pending`, `quit` sind lokale Befehle;
`pending` zeigt serverseitig noch offene Commands. `quit`, Ctrl+C und EOF trennen nur
diesen Client. `shutdown` fordert ausdrücklich den Einzel-Session-Shutdown an.
Während Replay lässt die REPL als Spielsteuerung nur Replay-Stop zu.
Parse-, Command- und nicht fatale Protokollfehler werden angezeigt, ohne die REPL zu beenden.
Unerwartetes Session-Ende und Terminal-I/O-Fehler beenden ihren Lauf mit Fehler.

Festgelegte Eingaben:

```text
command <command-json>
tick warp <ticks> [pace max|<ticks-per-second>]
tick warp pace max|<ticks-per-second>
tick warp stop
recording start <path>
recording stop
replay start <path>
replay stop
inspect query <arguments-json>
inspect entities
inspect entity <index>:<generation>
inspect resources
inspect resource <type-path>
```

`command` übernimmt den Rest der Zeile als gemeinsames Command-/Arguments-Objekt.
Damit sind auch Commands ohne eigene REPL-Kurzform erreichbar, etwa Input und Screenshot.
Die REPL startet ihre Beobachtung am Cursor der ersten Momentaufnahme und zeigt deren offene
Commands. Historische, bereits abgeschlossene Activity wird beim Einstieg nicht erneut
ausgegeben. Nach Verbindungsverlust verwendet sie dagegen den letzten Beobachtungscursor.

Die vollständige Inspect-Form übernimmt den gesamten Rest der Zeile als gemeinsames
Argumentobjekt. Die vier Kurzformen erzeugen Entity-Summary ohne Filter, Summary eines Handles,
Resource-Metadaten aller Resources beziehungsweise den Wert eines Typs.
Handle-Operanden sind dezimale `u32`, getrennt durch genau einen Doppelpunkt.
Der Type Path ist der nicht leere, außen getrimmte Rest der Zeile.
`help inspect` zeigt Kurzformen und kopierbare Beispiele aller Projektionen.

Ein Script ist ein versioniertes JSON-Dokument mit `version: 1` und `commands`-Array
aus gemeinsamen Command-/Arguments-Objekten. Session-Auswahl steht außerhalb der Datei.
`Script::parse` erhält bereits gelesenen Inhalt und prüft das vollständige Dokument vor
Ausführung; `Script::new` validiert programmatisch erzeugte Listen ebenso.
Die Liste enthält ausschließlich Commands. Ticks und Stops sind ausdrücklich.

Der Script-Client reicht in Dateireihenfolge ein und ordnet Ergebnisse über Session-Request-IDs
den ursprünglichen Array-Positionen zu. Normale Commands werden ohne allgemeine Ergebnisbarriere
eingereicht. Vor Recording-Start, Recording-Stop und Shutdown wartet er alle vorherigen
Script-Ergebnisse ab. Shutdown ist nur als letzter Listeneintrag zulässig; die vollständige
Script-Validierung prüft diese Position vor Ausführung.
Die Session prüft weiterhin ihre Vorbedingungen, auch gegenüber Arbeit anderer Clients.
Ein Inspect nach einem Warp-Start wartet somit nicht automatisch auf dessen Abschluss.
Wer den Zustand nach Warp-Abschluss benötigt, wartet dessen Ergebnis in einem getrennten
Aufruf ab, bevor er Inspect einreicht.

Alle Outcomes werden eingesammelt.
Nur ausschließlich `completed` ergibt `Passed`; sonst enthält `Failed` Command-Index,
optionale vergebene ID, Command und Ursache. Fehler oder Client-Ende brechen angenommene
Commands nicht versteckt ab.

Die ausführbare CLI-Form ist `woodpecker --address <adresse> session script <id> --file <datei>`.
Ihre Zusammenfassung behält auch die vollständigen erfolgreichen Outputs; Erfolgs- und
Fehlerlisten sind nach dem nullbasierten ursprünglichen Command-Index sortiert. Fachliche
Ablehnungen verhindern nicht die Einreichung späterer Commands. Verbindungsfehler,
Activity-Lücken und beschädigte Korrelation beenden dagegen die weitere Einreichung.
Bereits bekannte Ergebnisse bleiben erhalten. Offene Ergebnisse sind `unknown`, noch nicht
eingereichte Commands `not_submitted`; eine unbestätigte Einreichung wird nie wiederholt.
Vorvalidierung prüft Format, gemeinsame Command-Struktur und Shutdown-Position,
nicht die fachlichen Laufzeitvorbedingungen der Session.
