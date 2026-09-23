# Bevy Remote Protocol als Controlled-Session-Protokoll

Historische technische Bewertung. Der gültige Vertrag steht in [target.md](../target.md).
Nicht mehr vorhandene Quellpfade werden als damalige Referenzen bezeichnet; sie sind keine
Beschreibung der aktuellen Dateistruktur.

> **Nachgelagerte Projektentscheidung:** `command::inspect` wurde nach dieser Bewertung vollständig
> auf read-only Entity- und Resource-Queries begrenzt. Alle im Bericht geprüften v3-`Set`- und
> Asset-Anforderungen sind damit überholt; die Bewertung der BRP-Query- und
> Session-Protokollgrenzen bleibt gültig.

## Entscheidungsrahmen

Dieser Bericht prüft **Variante 3** als „möglichst unverändertes Stock-BRP als vollständiges
Controlled-Session-Protokoll“. Zur Einordnung werden die beiden realistischen Alternativen
mitgeführt:

- **Variante 1 / A:** eigenes v3-JSONL-Protokoll bleibt der Session-Vertrag; BRP wird höchstens
  als interner, explizit aufgerufener Inspect-Backend verwendet.
- **Variante 2 / B:** eigener v3-Vertrag mit einer BRP-/JSON-RPC-Dispatcher- oder Mailbox-Schicht
  und ausgewählten Built-ins beziehungsweise Custom-Methoden.
- **Variante 3 / C:** Stock-BRP-Semantik und -Transport werden zum Session-Vertrag.

Die Bezeichnung „Variante 3“ ist damit bewusst von einer späteren Produktentscheidung getrennt:
Es handelt sich um eine Architekturprüfung gegen die bereits beschriebenen v3-Anforderungen,
nicht um eine Freigabe zur Implementation.

**Quellbaseline:** Bevy `v0.19.1`, Commit
[`b56fc29d3016e641754765244b5ba3f9cc504671`](https://github.com/bevyengine/bevy/commit/b56fc29d3016e641754765244b5ba3f9cc504671).
Alle BRP-Aussagen beziehen sich auf diesen Stand; spätere `main`-Änderungen sind kein Nachweis
für v0.19.1.

## Kurzfazit

**Variante 3 ist als vollständiger v3-Session-Vertrag nicht kompatibel.** Stock-BRP liefert eine
brauchbare, reflektionsbasierte ECS-Remote-Schnittstelle, aber keinen kontrollierten Session-Lauf:
es fehlen der explizite Tick-/Warp-Vertrag, hostvergebene Session-IDs, ein Ready-/Capability-
Handshake, terminale Antworten für lang laufende Befehle, die getrennten Input- und Screenshot-
Semantiken, Assets sowie die geforderten stabilen Status- und Fehlerformen. Zusätzlich aktiviert
das Default-Interface weitreichende Weltmutation und besitzt keine Authentisierung.

**Bewertungsempfehlung, keine Produktentscheidung:** Variante 1/A hält den bestehenden v3-Vertrag
am zuverlässigsten intakt. BRP kann dort optional und ohne `RemotePlugin` als technische Hilfe für
bestimmte read-only-Reflection-/Query-Pfade dienen. Variante 2/B ist nur als begrenzter Prototyp
vertretbar, wenn ein eigener Adapter weiterhin Session-Zustand, Scheduling, Korrelation,
Allowlist und terminale Outcomes besitzt. Variante 3/C sollte allenfalls als ausdrücklich
separater, lokal gebundener Entwicklungs-Inspector betrachtet werden, nicht als Controlled Session.

---

## 1. Bestätigte Fakten aus Bevy 0.19.1

### 1.1 BRP ist JSON-RPC-Datenmodell, aber kein fertiger stdio-/JSONL-Transport

- `RemotePlugin` richtet die BRP-Mailbox und die Handler ein, **startet aber keinen Transport**.
  Der Crate-Text verlangt für Remote-Verbindungen ein zweites Plugin.
  [`bevy_remote/src/lib.rs`, Zeilen 1–9 und 566–572](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L1-L9)
- Der mitgelieferte Transport ist `RemoteHttpPlugin`. Er bindet standardmäßig an
  `127.0.0.1:15702`; mit aktivem Render-Feature existiert zusätzlich ein Render-Port
  `15703`. HTTP ist im Crate als eigener Modulpfad implementiert.
  [`http.rs`, Zeilen 1–17 und 103–161](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/http.rs#L1-L17)
- Eine stdio-, Pipe- oder JSONL-Implementierung ist in der geprüften `bevy_remote`-Quelle nicht
  vorhanden. Ein stdio-Adapter wäre daher neue, projektspezifische Transportlogik. Das lokale
  Projekt besitzt diese Framing-Schicht bereits: `JsonLinesInput` liest nicht blockierend aus
  einem begrenzten Kanal, `StdoutOutput` schreibt und flusht genau eine JSON-Zeile.
  [`src/client/transport.rs:1-110`](../../../src/client/transport.rs#L1-L110)

**Folgerung:** BRP kann den bestehenden Prozesskanal nicht ohne Adapter ersetzen. Ein Adapter ist
nicht nur eine andere Socket-Adresse: Er muss JSONL-Framing, stdout-Exklusivität, EOF, Fehler,
Korrelation und Lebensdauer von Streams definieren.

### 1.2 JSON-RPC-Request-ID und Entity-ID haben die falsche Identitätssemantik

- BRP beschreibt `id` als beliebige JSON-Daten, die vom Client kommen und nur zurückkopiert werden.
  Es gibt keine hostseitige Vergabe, keine Prüfung auf Eindeutigkeit und keinen Session-Handle.
  [`lib.rs`, Zeilen 28–39 und 1039–1053](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L28-L39)
- `BrpRequest.id` ist im Rust-Typ optional. Die Deserialisierung akzeptiert fehlende IDs; die
  Response serialisiert eine vorhandene ID, lässt sie bei `None` aber weg.
  [`lib.rs`, Zeilen 1039–1156 und 1159–1190](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L1039-L1190)
- Die Entity-ID wird als Bevy-`Entity` transportiert; die BRP-Dokumentation zeigt dafür eine
  einzelne JSON-Zahl wie `4294967298`. `bug_hunter` verwendet dagegen den verlustfreien,
  sessionlokalen Objekt-Handle `{ "index": u32, "generation": u32 }` und prüft ihn gegen den
  aktuellen World.
  [`lib.rs`, Zeilen 13–25](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L13-L25),
  damalige Quellreferenz `src/handle/mod.rs:1-106`
- Der v3-Entwurf macht `RequestId` ausdrücklich ausschließlich von `Session` vergeben und trennt
  ihn vom Controller. Die Response nennt zusätzlich den qualifizierten Command-Namen.
  [aktueller Vertrag für Annahme und Korrelation](../target.md#fortschritt-annahme-und-ergebnisse)

**Severity: hoch.** Stock-BRP als Wire-Vertrag würde entweder Client-IDs und numerische Bevy-
Entity-IDs übernehmen oder einen Adapter benötigen, der beide Identitätssysteme separat abbildet.
Die zweite Lösung ist faktisch ein eigener v3-Vertrag über BRP und nicht Variante 3.

### 1.3 Mailbox, Scheduling und Nebenläufigkeit

- `RemotePlugin` erzeugt in `PreStartup` eine begrenzte `async_channel`-Mailbox mit
  `CHANNEL_SIZE = 16`. `BrpMessage` enthält nur Methode, Parameter und einen Antwortsender;
  Request-ID, Client- oder Verbindungsidentität sind dort nicht enthalten.
  [`lib.rs`, Zeilen 564 und 1442–1475](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L564-L564)
  [`lib.rs`, Zeilen 1442–1475](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L1442-L1475)
- `process_remote_requests` leert die Mailbox pro Ausführung vollständig, führt Instant-Handler
  nacheinander mit exklusivem `&mut World` aus und sendet das Ergebnis auf dem jeweiligen Kanal.
  Bei einer unbekannten Methode wird ein Fehler gesendet und die Funktion beendet sich an dieser
  Stelle; die übrige Mailbox wird auf den nächsten Lauf verschoben.
  [`lib.rs`, Zeilen 1477–1523](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L1477-L1523)
- Für eine HTTP-Anfrage wartet der Transport bei einem Instant-Request auf genau einen
  Kanalwert. Für eine Watch-Anfrage wird ein Kanal mit Kapazität 8 als SSE-Stream verwendet.
  Batches werden vollständig aus dem HTTP-Body gelesen und in Eingangsreihenfolge verarbeitet;
  Streaming ist innerhalb eines Batches verboten.
  [`http.rs`, Zeilen 302–430](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/http.rs#L302-L430)
- Ongoing-Watches werden bei jedem `RemoteLast`-Lauf nacheinander geprüft. `Some(value)` wird mit
  `try_send` gesendet; bei vollem oder geschlossenem Kanal wird der Sender geschlossen und der
  Watch anschließend bereinigt. `None` ist nur „in diesem Poll keine Antwort“, kein terminaler
  Abschluss.
  [`lib.rs`, Zeilen 1525–1569](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L1525-L1569)
- `RemoteWatchingRequests` speichert `(BrpMessage, Handler)` ohne serververgebene Watch-ID. Die
  Kernstruktur kann daher einen Watch nicht über eine unabhängige ID gezielt abbrechen. Beim
  Beenden hängt die Bereinigung am Schließen des Antwortkanals.
  [`lib.rs`, Zeilen 995–997 und 1559–1569](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L995-L997)
- `world.observe+watch` legt Event-Observer und Buffer unter dem Schlüssel
  `event` beziehungsweise `event@entity` ab. Zwei Requests mit denselben Parametern teilen daher
  den Buffer; ein Poll kann die Daten des anderen Polls drainieren. Der Buffer und der Observer
  werden von dieser Methode nicht an einen einzelnen Session-Request gebunden.
  [`builtin_methods.rs`, Zeilen 1556–1665](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L1556-L1665)

**Folgerung:** BRP unterstützt zwar mehrere gleichzeitig eingereichte Instant-Requests und
transportseitig konkurrierende HTTP-Verbindungen, garantiert aber weder die v3-Annahme-/Pending-
Semantik noch faire Interleaving-Budgets. Eine große Query oder ein langer Instant-Handler hält die
serielle Weltverarbeitung auf. Watch ist ein fortlaufender Stream, nicht die v3-Abstraktion
„eine Start-Response bleibt pending und endet genau einmal“.

### 1.4 `RemoteLast` ist ein normaler Schedule, kein neutraler Dispatcher

- `RemotePlugin::build` registriert `RemoteLast` und fügt ihn mit
  `MainScheduleOrder::insert_after(Last, RemoteLast)` in die Hauptreihenfolge ein.
  [`lib.rs`, Zeilen 805–854](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L805-L854)
- Mit Render-Unterstützung wird ein zweiter `RemoteLast` nach `Render` in der Render-SubApp
  registriert. Das ist eine eigene Welt und kann auf natürlichen Renderläufen Requests
  verarbeiten.
  [`lib.rs`, Zeilen 856–905](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L856-L905)
- Bevy führt den Default-Main-Schedule als `First`, `PreUpdate`, State-Transition,
  `RunFixedMainLoop`, `Update`, `SpawnScene`, `PostUpdate`, `Last`; `RunFixedMainLoop` kann
  abhängig von verstrichener Zeit null bis viele `FixedMain`-Durchläufe ausführen. Rendering läuft
  separat.
  [`bevy_app/main_schedule.rs`, Zeilen 13–45](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_app/src/main_schedule.rs#L13-L45)
- `AutomationControlPlugin` nimmt die vorhandene Main-Reihenfolge auf und ersetzt sie durch
  `Input`, `Frames`, `Output`; die ursprüngliche Reihenfolge wird in
  `SimulationScheduleOrder` für explizite kontrollierte Frames behalten.
  [`src/client/plugin.rs:122-220`](../../../src/client/plugin.rs#L122-L220)
  [`src/client/plugin.rs:620-700`](../../../src/client/plugin.rs#L620-L700)
- Die aktuelle Frame-Schleife setzt für jeden expliziten Step eine manuelle Duration, führt die
  gespeicherten Schedules aus und zählt erst danach die Clock. Der v3-Entwurf verlangt dagegen
  Warp-Steuerung ohne versteckte Ticks und will die Zeitbedeutung bei der Anwendung belassen.
  [`src/client/plugin.rs:580-650`](../../../src/client/plugin.rs#L580-L650),
  damalige Quellreferenz `src/command/tick.rs:1-116`,
  [aktueller Tick-Warp-Vertrag](../target.md#tick-warp)

**Severity: hoch.** Wird BRP vor der Schedule-Umsortierung installiert, kann `RemoteLast` in die
übernommene Simulationsreihenfolge geraten und mit jedem expliziten Frame ausgeführt werden. Wird
es danach installiert, ist die angenommene `Last`-Position nicht mehr selbstverständlich. Wird es
nur aus der Reihenfolge entfernt, werden BRP-Anfragen überhaupt nicht verarbeitet, solange kein
anderer Adapter `RemoteLast` ausdrücklich ausführt. Diese drei Fälle müssen getrennt getestet
werden; Stock-BRP selbst besitzt keine Controlled-Session-Schedulegrenze.

### 1.5 Stock-Built-ins sind ein breites Welt-Mutationsinterface

Der Default von `RemotePlugin` registriert für Main und bei Render-Unterstützung für Render alle
aufgelisteten Built-ins:

- `world.query`, `world.get_components`, `world.list_components`,
  `world.get_components+watch` und `world.list_components+watch`,
- `world.spawn_entity`, `world.despawn_entity`, `world.insert_components`,
  `world.remove_components`, `world.reparent_entities`,
- `world.mutate_components`,
- `world.get_resources`, `world.list_resources`, `world.insert_resources`,
  `world.remove_resources`, `world.mutate_resources`,
- `world.trigger_event`, `world.write_message`, `world.observe+watch`,
- `registry.schema`, `schedule.list`, `schedule.graph` und `rpc.discover`.

Die Registrierung ist im Quelltext vollständig sichtbar; einen fertigen Default-Read-Only-Modus
bietet `RemotePlugin::default()` nicht.
[`lib.rs`, Zeilen 671–788](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L671-L788)

`RemotePlugin::empty()` ist zwar intern vorhanden, aber privat (Zeilen 579–587). Die öffentliche
`RemoteMethods`-Resource kann Methoden einfügen und lesen, stellt jedoch keinen offiziellen
„Plugin ohne Built-ins“-Konstruktor bereit (Zeilen 961–992). Ein Allowlist-Adapter muss deshalb
entweder die Plugin-Implementierung forken/erweitern oder die registrierten System-IDs und die
Resource in einer bewusst getesteten Reihenfolge ersetzen.

**Severity: hoch.** Ein versehentlich aktivierter BRP-Endpunkt erlaubt nicht nur Inspect, sondern
Entity-Erzeugung/-Löschung, Component-/Resource-Insert/Remove, Feldmutation, Reparenting,
Event-Trigger und Message-Schreiben. Die Default-Adresse ist zwar Loopback, aber das ist keine
Authentisierung. `with_address`, Header und insbesondere CORS-Header ändern keine Autorisierung.
[`http.rs`, Zeilen 103–211](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/http.rs#L103-L211)

### 1.6 Query- und Reflection-Semantik decken das v3-Inspect nicht vollständig ab

`world.query` verlangt vollständige Type Paths und baut einen `QueryBuilder<FilteredEntityRef>`
über die ECS-Welt. Die konkrete Semantik bei `strict: false` ist ausdrücklich:

- unbekannte erforderliche `components` oder `filter.with`: leere Ergebnismenge,
- unbekannte optionale Komponenten oder `filter.without`: ignorieren,
- unbekannte `has`: als nicht vorhanden (`false`),
- mit `strict: true`: Fehler bei unbekannten beziehungsweise nicht verwendeten Komponenten.

[`builtin_methods.rs`, Zeilen 856–943](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L856-L943)

Für das Ergebnis werden nicht serialisierbare Components in `serialize_components` nicht als
belegtes `Unavailable`-Element ausgegeben, sondern übersprungen und gewarnt. Für `get_components`
existiert bei `strict: false` eine getrennte `errors`-Map; das ist nicht die v3-Form
`Readable`/`Unavailable { status }` pro Element.
[`builtin_methods.rs`, Zeilen 1020–1054 und 760–828](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L1020-L1054)

Die v3-Ziel-Queries sind anders geschnitten:

- Entities, Resources und Assets sind drei getrennte Kategorien;
- interne Resource-Entities sollen nicht als Entity-Ergebnisse erscheinen;
- Query-Ergebnisse werden vollständig ohne Cursor/Pagination geliefert;
- jede nicht lesbare Value bleibt als typisierter Status sichtbar;
- `Set` besitzt getrennte Whole-Value-/Reflect-Path-Semantik und liest den neuen Wert zurück.

Diese Liste beschreibt den damaligen Prüfgegenstand.
Der aktuelle [Inspect-Vertrag](../target.md#inspect) steht im konsolidierten Ziel.

**Unbelegte, prototyppflichtige Einzelheit:** Der BRP-Quellcode legt bei `world.query` keinen
expliziten Ausschluss interner Resource-Entities an. Er verwendet den allgemeinen
`FilteredEntityRef`-Builder; ob aktuelle World-Interna praktisch erscheinen, muss mit einer
Minimal-App geprüft werden. Schon die fehlende explizite Exclusion-Regel genügt, um Stock-BRP
nicht ungeprüft als v3-Query-Vertrag zu übernehmen.

### 1.7 Resources, Assets und Mutation

- `world.get_resources` löst ein reflektiertes Resource-Type-Path auf, sucht den zugehörigen
  Resource-Entity-Eintrag und gibt bei fehlender Resource einen BRP-Fehler zurück. Es gibt keine
  v3-Statuswerte für `Missing`, `NotRegistered`, `NotReflectable` und `NotSerializable` pro
  Ergebnis.
  [`builtin_methods.rs`, Zeilen 617–659 und 2006–2022](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L617-L659)
- `world.list_resources` listet Registry-Einträge mit `ReflectResource`, nicht ausschließlich
  aktuell vorhandene Ressourcen.
  [`builtin_methods.rs`, Zeilen 1413–1431](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L1413-L1431)
- Die geprüfte BRP-Quelle enthält keine Asset-Kategorie, keinen Asset-Schlüssel und keine
  Asset-Pfade. Die `bevy_asset`-Feature-Abhängigkeit ist vorhanden, ersetzt aber keine
  `Assets<T>`-Query-/Set-Methoden. Die v3-Anforderungen an Live-Assets, opake sessionlokale
  Schlüssel und direkte Asset-Mutation werden dadurch nicht erfüllt.
  [`builtin_methods.rs`, Methodenliste Zeilen 44–111](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L44-L111)
  [`bevy_remote/Cargo.toml`](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_remote/Cargo.toml)
- `world.mutate_components` und `world.mutate_resources` mutieren genau einen Reflect-Pfad,
  geben aber nur `null` zurück. Es gibt keine vollständige v3-Validierungs-/Readback-Garantie für
  den neuen JSON-Wert; Component/Resource-Whole-Value, Assets und Set-Target-Regeln fehlen.
  [`builtin_methods.rs`, Zeilen 1164–1281](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L1164-L1281)
- `world.insert_components`, `world.insert_resources`, `world.spawn_entity`, Remove, Despawn und
  Reparent verändern die Struktur, obwohl v3 `Set` ausschließlich bestehende Werte verändern
  soll. `reparent_entities` prüft die Liste in einer Schleife; ein Self-Reparent nach zuvor
  verarbeiteten Einträgen kann deshalb nicht als atomare Gesamtvalidierung behandelt werden.
  [`builtin_methods.rs`, Zeilen 1057–1075, 1116–1162 und 1283–1375](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L1057-L1075)

Die BRP-Serializer-Route (`ReflectSerializer`, `TypedReflectDeserializer`) ist technisch nützlich,
aber ihre Registrierung und JSON-Form sind ein Bevy-Reflection-Vertrag. Type Paths werden direkt
mit `TypeRegistry::get_with_type_path` aufgelöst; eine eigene, explizite Policy für mehrdeutige
Pfade ist im Built-in-Code nicht sichtbar. Das lokale I1 bleibt daher für den v3-Vertrag offen,
auch wenn BRP für unbekannte Query-Pfade eine `strict`-Policy besitzt.
[`builtin_methods.rs`, Zeilen 1817–1855 und 1906–2004](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L1817-L1855)
Damals referenzierter Quellpfad: `src/observe/world/projection.rs:1-90`.

### 1.8 Input, Tick-Warp und Screenshot

**Input**

`world.write_message` kann jede registrierte reflektierte Message schreiben. Das offizielle BRP-
Beispiel benutzt diesen Weg für `WindowEvent::CursorMoved` sowie Mouse-Press/Release.
[`builtin_methods.rs`, Zeilen 1518–1554](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L1518-L1554)
[`examples/remote/integration_test.rs`](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/examples/remote/integration_test.rs)

Das ist nicht die v3-Virtual-Input-Semantik: Es gibt keine stabilen, layoutunabhängigen Keyboard-
Tokens, keine getrennten `MoveTo`-/`MoveBy`-Übergänge, keine sessionlokale Pointer-State-
Validierung und keine IME-Grenze von 16 KiB. Die lokalen Adapter validieren Zustandsübergänge und
queue'n Events erst für den nächsten kontrollierten Frame.
Damals referenzierte Quellpfade: `src/command/input/keyboard.rs:1-220`,
`src/command/input/pointer.rs:1-300`, `src/command/input/text.rs:1-180`.
[`src/client/plugin.rs:267-530`](../../../src/client/plugin.rs#L267-L530)

**Tick-Warp**

BRP stellt keine Tick- oder Warp-Methode bereit. `schedule.list` und `schedule.graph` sind
Inspection der Schedules, keine Ausführung eines v3-Ticks.
[`builtin_methods.rs`, Zeilen 1721–1799](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L1721-L1799)

Ein Custom-Instant-Handler läuft mit exklusivem `&mut World` synchron innerhalb von
`RemoteLast`. Ein Handler kann nicht als Stock-BRP-Operation mehrere kontrollierte Frames ausführen
und gleichzeitig einen anderen Request wie `Stop` bearbeiten. Ein Custom-Watching-Handler kann
zwar bei späteren Polls `None` oder `Some` liefern, besitzt aber im v0.19.1-Kern keinen
terminalen One-Shot-/Close-Vertrag. Für Start/SetPace/Stop wären daher zusätzlich eigene Warp-
State, ein eigener kontrollierter Scheduler, Korrelation über Parameter und eine Adapterregel
zum Schließen des Streams nötig.

**Screenshot**

BRP besitzt keine dedizierte Screenshot-/Artifact-Methode. Das offizielle Integration-Beispiel
spawnt eine `Screenshot`-Entity und beobachtet `ScreenshotCaptured`; damit wird ein Image-Event
über einen Watch-Stream sichtbar, aber weder ein sandboxed PNG-Pfad noch `overwritten`,
PNG-Verifikation oder ein v3-Output garantiert.
[`examples/remote/integration_test.rs`](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/examples/remote/integration_test.rs)

Die lokale Screenshot-Integration koppelt dagegen asynchronen GPU-Readback, Artifact-Root,
Traversal-/Symlink-Prüfung und PNG-Schreiben an den ausstehenden Session-Request.
Damals referenzierte Quellpfade: `src/screenshot/plugin.rs:1-122`,
`src/screenshot/capture.rs:1-150`.

**Severity: hoch.** Ohne eigene Session-Schicht lassen sich Tick-Warp und Screenshot nicht in den
v3-Vertrag einpassen. Eine BRP-Watch-Response ist kein belastbarer Ersatz für „pending bis
terminal, genau eine Response“.

### 1.9 Fehler, Discovery, Sicherheit und Versionierung

- BRP-Fehler besitzen numerische `i16`-Codes wie `-32602`, `-23401` und `-23501`, eine kurze
  Nachricht und optionales `data`. v3 verlangt stabile fachliche String-Codes sowie die
  Unterscheidung von `Completed`, `Rejected` und `ProtocolFailed`.
  [`lib.rs`, Zeilen 1302–1428](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs#L1302-L1428)
  [aktueller Spielprotokoll-Vertrag](../target.md#gemeinsame-command-kodierung-und-spielprotokoll)
- `rpc.discover` stellt ein OpenRPC-Dokument mit Methoden und `openrpc: "1.3.2"` bereit; es ist
  kein v3-Ready-Handshake und meldet weder Session-Fähigkeiten wie „Screenshot wartet auf PNG“
  noch Tick-/Recording-/Replay-Zustand.
  [`builtin_methods.rs`, Zeilen 1078–1114](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/builtin_methods.rs#L1078-L1114)
- `registry.schema` kann eine breite Registry-/Schema-Ansicht liefern; `world.query` und
  Resource-Methoden sind nicht auf `AutomationTarget` begrenzt. Der v3-Entwurf entfernt diesen
  Marker und definiert stattdessen gezielte Inspect-Kategorien, aber Stock-BRP besitzt dafür keine
  v3-Allowlist.
- HTTP bindet standardmäßig nur an Loopback, bietet aber konfigurierbare Adresse und Header.
  Im geprüften Transport sind weder Authentisierung noch TLS vorgesehen.
  [`http.rs`, Zeilen 103–211](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/http.rs#L103-L211)
- Methodennamen, vollständige Type Paths und Reflection-Schemata sind an den Bevy-/Anwendungsbuild
  gekoppelt. Ein BRP-Handshake kündigt keine eigene BRP-Version an; `rpc.discover` allein ersetzt
  keine v3-Versions- und Capability-Verhandlung.

**Severity: hoch für einen erreichbaren Produktions-/Testprozess.** Variante 3 wäre nur als
explizit opt-in, lokal abgeschotteter Entwicklungsendpunkt vertretbar; selbst dann sollte die
Default-Mutationsliste nicht als Controlled Session exponiert werden.

### 1.10 Feature- und Dependency-Auswirkung

- `bevy_remote` `0.19.1` aktiviert standardmäßig `http`, `bevy_asset` und `bevy_render`.
  `http` zieht unter anderem `async-io`, `hyper`, `smol-hyper`, `http-body-util` und
  `bevy_tasks/async-io` hinzu.
  [`bevy_remote/Cargo.toml`](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_remote/Cargo.toml)
- Die Bevy-Feature `bevy_remote` aktiviert zusätzlich `serialize`; im nativen
  `bevy_internal`-Dependencypfad wird `bevy_remote` mit Default-Features geführt. Das ist für ein
  Projekt mit `default-features = false` relevant.
  [`bevy_internal/Cargo.toml`](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_internal/Cargo.toml)
- `bug_hunter` verwendet derzeit Bevy `0.19.0`, `default-features = false` und führt `bevy_remote`
  noch nicht als Abhängigkeit.
  [`Cargo.toml:1-18`](../../../Cargo.toml#L1-L18)

**Folgerung:** Für Variante 1/A wäre eine optionale direkte `bevy_remote`-Abhängigkeit mit
`default-features = false` und einer festgelegten Bevy-0.19.1-Lock-/Versionsbaseline zu prüfen.
Das vermeidet den HTTP-Server, beseitigt aber weder API-Kopplung noch die fachlichen Adapter-
aufgaben. Eine tatsächliche Feature-/Compile-Messung bleibt Prototypfrage.

---

## 2. Abgleich mit den v3-Invarianten

| v3-Anforderung | Stock-BRP 0.19.1 | Bewertung |
|---|---|---|
| Eigenes Ready mit Version 3 und Capabilities | `rpc.discover`, kein Session-Ready | **nicht erfüllt** |
| Request-ID von Session/Host, Controller liefert keine ID | Client-`id`, optional und beliebig | **nicht erfüllt** |
| Mehrere Pending-Requests und Out-of-order terminale Outcomes | Instant-Kanal und parallele HTTP-Verbindungen, aber kein v3-Pending/Outcome-Modell; Batch seriell | **nur transportseitig teilweise** |
| Ein expliziter Simulationstick, keine versteckte Uhr | keine Tick-Methode; `RemoteLast` regulärer Schedule; Render-Remote separat | **nicht erfüllt** |
| Warp-Start pending bis Completed/Stopped, SetPace/Stop währenddessen | kein Tick-Warp; Watch ohne terminalen One-Shot-/Close-Vertrag | **nicht erfüllt** |
| Input erst im nächsten kontrollierten Tick, stabile Tokens und Zustandsvalidierung | generisches `write_message` | **nicht erfüllt** |
| Entity-Query ohne interne Resource-Entities, keine Cursor/Pagination, Unavailable-Status | allgemeiner QueryBuilder, strict/skip/errors; Ressource als ECS-Sonderpfad | **nicht erfüllt** |
| Resources als eigene Kategorie und nur vorhandene All-Ergebnisse | Registry-Listing und einzelner Resource-Entity-Zugriff | **nicht erfüllt** |
| Assets mit opakem sessionlokalem Schlüssel und Set | keine Asset-Built-ins | **nicht erfüllt** |
| Set nur auf bestehende Component/Resource/Asset, atomare Vorprüfung, Readback | Insert/Remove/Spawn zusätzlich; Mutate nur Pfad und `null` | **nicht erfüllt** |
| Screenshot als PNG-Artefakt mit Sandbox, Overwrite, Größe und pending Readback | Screenshot-Entity + Event-Watch als Beispiel | **nicht erfüllt** |
| Stabile fachliche String-Fehler und ProtocolFailed | numerische BRP-Codes und generische Fehler | **nicht erfüllt** |
| Default-Allowlist und keine ungeschützte Weltmutation | breites Default-Mutationsinterface, keine Auth | **nicht erfüllt** |
| JSONL über stdin/stdout | nur HTTP plus SSE für Watch | **nicht erfüllt** |
| Session Recording/Replay/Shutdown über denselben v3-Ausführungsweg | keine Session-Recording-/Replay-Verben; generisches AppExit per Message möglich | **nicht erfüllt** |

Die Matrix beschreibt den damaligen Prüfgegenstand. Die maßgebliche aktuelle Zielbeschreibung
steht in [target.md](../target.md), die Interface-Skizze in [goal.rs](../goal.rs).

---

## 3. Variantenvergleich

### Variante 1 / A — eigenes v3 plus BRP intern für Inspect

**Form:** Der eigene JSONL-Vertrag bleibt alleiniger Prozess-/Session-Vertrag. Kein
`RemoteHttpPlugin` und möglichst kein `RemotePlugin`; ausgewählte öffentliche BRP-Handler wie
`process_remote_query_request`, `process_remote_get_components_request` oder der Resource-Handler
werden an einer expliziten Inspect-Grenze direkt als Bevy-System aufgerufen.

**Stärken**

- Ready, hostvergebene IDs, Pending-Liste, Out-of-order-Zuordnung und terminale Outcomes bleiben
  im bestehenden Owner `Session`.
- Kein Netzwerkendpunkt, keine BRP-Default-Allowlist und kein zusätzliches `RemoteLast`-Scheduling,
  wenn nur die öffentlichen Instant-Handler direkt verwendet werden.
- Die kontrollierten Input-, Tick-, Screenshot-, Recording- und Replay-Semantiken bleiben eigene
  Domänenregeln.
- BRP-Reflection/Serializer kann bei read-only ECS-Queries Arbeit sparen.

**Kosten und Grenzen**

- BRP-Handler liefern weiterhin Bevy-Entity-Zahlen, `errors`/Skip-Semantik und keine v3-Status-
  formen; ein Wrapper muss Handles, Kategorien und Statuswerte übersetzen.
- Assets, v3-Set, Screenshot und Tick-Warp werden dadurch nicht gelöst.
- Direkte Verwendung der `bevy_remote::builtin_methods` koppelt an Bevy-Interna und muss auf die
  geprüfte Version gepinnt sowie mit Fixtures getestet werden.

**Bewertung:** technisch am ehesten kompatibel; BRP ist hier ein optionaler Backend-Baustein,
nicht der Session-Vertrag.

### Variante 2 / B — BRP-/JSON-RPC-Dispatcher mit selektierten Methoden

**Form:** Ein eigener Adapter nutzt `BrpSender`/`BrpMessage`, registriert nur eigene oder
freigegebene Methoden, übersetzt die v3-Commands und stellt einen stdio-Transport bereit.

**Stärken**

- Gemeinsame Handlerform (`In<Option<Value>> -> BrpResult`) und Reflection können für ausgewählte
  Instant-Inspect-Methoden genutzt werden.
- Ein späterer BRP-Inspector könnte begrenzte Interoperabilität erhalten.
- Eine eigene Allowlist kann die gefährlichen Default-Methoden theoretisch ausschließen.

**Kosten und Blocker**

- `RemotePlugin::empty()` ist privat; Custom-only benötigt Fork/Upstream-Änderung oder eine
  fragile Resource-/System-ID-Filterung.
- Der Adapter muss die begrenzte Mailbox, Backpressure, stdout-Synchronisierung, Handlerfehler,
  JSONL-Framing und die fehlende Request-ID im `BrpMessage` selbst lösen.
- Für Warp/Screenshot ist ein eigener Pending-/State-/Close-Mechanismus außerhalb der Stock-Watch-
  Semantik nötig.
- `RemoteLast` muss bewusst aus der Simulation entfernt und an eine definierte Control-Phase
  gesetzt werden; sonst drohen falsche oder doppelte Verarbeitung.
- B bleibt fachlich ein eigener v3-Dispatcher mit BRP als Implementierungsdetail. Stock-BRP-
  Kompatibilität darf dabei nicht als v3-Semantik ausgegeben werden.

**Bewertung:** als begrenztes Experiment möglich; als „BRP statt v3“ nicht belastbar.

### Variante 3 / C — möglichst unverändertes Stock-BRP

**Form:** `RemotePlugin::default()` plus `RemoteHttpPlugin` beziehungsweise ein möglichst dünner
stdio-Transport; Clients sprechen JSON-RPC mit Stock-Built-ins.

**Stärken**

- Schnellster Weg zu generischem World-Inspect und Reflection.
- Offizielle HTTP-/Client-/Integration-Beispiele und OpenRPC-Discovery vorhanden.
- Für einen unkritischen lokalen Entwickler-Inspector kann die breite Methodeabdeckung nützlich sein.

**Entscheidende Schwächen**

- Die Tabelle in Abschnitt 2 weist die zentralen v3-Kriterien als nicht erfüllt aus.
- Die Stock-Weltmutation ist weiter als `Set` und besitzt keine v3-Allowlist.
- Tick-Warp, kontrollierte Input-Reihenfolge und Screenshot-Artefakt fehlen vollständig.
- IDs, Fehler, Ready, JSONL, Asset-Kategorie und terminale Langläuferantworten sind inkompatibel.
- HTTP-Loopback ist keine Sicherheitsgrenze für lokale untrusted Prozesse; mit konfigurierbarer
  Adresse wird das Risiko größer.

**Bewertung für Variante 3:** **No-Go als vollständiges Controlled-Session-Protokoll.** Eine
separate, explizit opt-in geschützte BRP-Debugschnittstelle ist eine andere Funktion und darf nicht
als Erfüllung von v3 gezählt werden.

---

## 4. Blocker getrennt von offenen Prototypfragen

### 4.1 Blocker für Variante 3 als v3-Vertrag

1. **B1 / hoch — Session- und ID-Vertrag:** BRP verwendet optionale clientvergebene JSON-IDs und
   Bevy-Entity-Zahlen; v3 verlangt Session-/Host-ID und `{index,generation}`-Handles.
2. **B2 / hoch — kontrollierte Zeit:** Kein Stock-Tick/Warp; `RemoteLast` ist ein regulärer
   Schedule. Eine Instant-Methode kann keinen abbrechbaren, nebenläufig steuerbaren Warp tragen.
3. **B3 / hoch — terminale Langläuferantwort:** Watching ist ein fortlaufender `Option`-Stream
   ohne v0.19.1-One-Shot-/Close-/Watch-ID-Vertrag.
4. **B4 / hoch — semantische Abdeckung:** Keine Assets, kein v3-Inspect-Statusmodell, kein
   Screenshot-Artefaktvertrag und keine v3-Virtual-Input-Commands.
5. **B5 / hoch — Mutation/Sicherheit:** Default-BRP erlaubt weitreichende Spawn-/Remove-/Resource-
   und Event-/Message-Mutation; keine eingebaute Authentisierung oder fachliche Allowlist.
6. **B6 / hoch — Wire/Fehler:** HTTP/SSE und numerische BRP-Fehler sind nicht JSONL/v3-
   `Rejected`/`ProtocolFailed`/stabile String-Codes.
7. **B7 / mittel bis hoch — Scheduling-/Render-Isolation:** Main- und Render-`RemoteLast` können
   in falschen Laufgrenzen und Welten Requests verarbeiten.

### 4.2 Offene Fragen für einen begrenzten Prototyp

Diese Punkte sind keine Argumente, Variante 3 bereits zu akzeptieren; sie bestimmen nur, ob A oder
B technisch sinnvoll eingegrenzt werden kann:

- **P1:** Enthält `process_remote_query_request` in der konkreten App interne Resource-Entities?
  Ein Test muss das gegen die v3-Exclusion-Regel stellen.
- **P2:** Welche exakte TypeRegistry-Semantik gilt bei mehrfachen/mehrdeutigen Type Paths? Für I1
  braucht jede Query-Position eine explizite Entscheidung.
- **P3:** Welche reflected Werte serialisieren mit derselben kanonischen Form für Query und Set,
  und welche bleiben `NotSerializable`?
- **P4:** Kann ein stdio-Adapter die Response-Kanäle zuverlässig als ID-Map führen, ohne dass
  `BrpMessage`-Handler Request-Identität benötigen? Dazu gehören parallele Requests, EOF,
  Backpressure und atomare stdout-Ausgabe.
- **P5:** Lässt sich `RemoteLast` in einer Minimal-App einmal pro gewünschter Control-Phase
  ausführen, ohne `RunFixedMainLoop` oder Render-Remote unbeabsichtigt einzubeziehen?
- **P6:** Wie wird eine eigene Watching-Methode nach dem ersten terminalen Ergebnis geschlossen,
  und wie wird `Stop` bei laufendem Warp garantiert noch angenommen?
- **P7:** Welche Minimal-Featurekombination kompiliert `bevy_remote` ohne HTTP und ohne unnötige
  Render-/Asset-Features gegen die festgelegte Bevy-Baseline?
- **P8:** Welche BRP-Read-Handler liefern genug Rohdaten, um die v3-Wrapper ohne Verlust von
  `Unavailable`-Status, Entity-Handle oder Ressourcen-/Asset-Trennung zu bauen?

---

## 5. Vorgeschlagener Minimalprototyp

Der Prototyp soll die Architekturfrage falsifizieren, nicht bereits die Produktentscheidung
vorwegnehmen. Er ändert den v3-Vertrag nicht und verwendet keine öffentliche BRP-Netzwerk-
freigabe.

1. **Feature-/Build-Seam:** Bevy exakt auf `0.19.1`/den Baseline-Commit pinnen und eine optionale
   direkte `bevy_remote`-Abhängigkeit mit `default-features = false` untersuchen. `cargo tree
   -e features` und ein Minimalbuild dokumentieren.
2. **Direct-Handler-Spike (A):** In einer expliziten Inspect-Phase nur `world.query`,
   `world.get_components` und `world.get_resources` direkt aufrufen. Handle-Übersetzung,
   Resource-Entity-Ausschluss, Type-Path-Policy, `Unavailable`-Mapping und JSON-Fixtures gegen den
   v3-Entwurf prüfen. Kein `RemotePlugin`, kein HTTP.
3. **Schedule-Spike (B):** Separate Minimal-App mit `RemotePlugin` und künstlicher Mailbox;
   Plugin-Reihenfolge vor/nach `AutomationControlPlugin`, Anzahl der `RemoteLast`-Läufe und
   `Clock`-/Anwendungs-Tick-Zähler instrumentieren. Erwartung: kein Request darf ohne explizite
   Session-Grenze einen Simulations-Tick erzeugen.
4. **Pending-/Warp-Spike:** Einen Custom-Watch mit Token, Start-State, begrenztem Framebudget,
   `SetPace`, `Stop` und finalem Ergebnis versuchen. Testkriterium ist genau eine terminale
   Response bei gleichzeitig weiter angenommenen Stop-/Pace-Requests. Ein Fehlschlag bestätigt,
   dass BRP-Watch nur als internes Detail, nicht als v3-Pending-Modell taugt.
5. **Mailbox-/Concurrency-Spike:** Mindestens 17 Requests bei pausierter Welt, zwei identische
   Watch-Parameter, unbekannte Methode zwischen gültigen Requests und parallele Sender testen.
   Kanalfüllung, Reihenfolge, Fairness, Cleanup und mögliche Shared-Buffer-Verluste protokollieren.
6. **Inspect-/Set-Spike:** vorhandene und fehlende Resources, interne Resource-Entities,
   nicht registrierte/nicht reflektierbare/nicht serialisierbare Components, Path-Fehler,
   Change-Detection und Asset-IDs prüfen. BRP-Ergebnisse nur als Rohvergleich, nicht als
   v3-Ausgabe werten.
7. **Security-Spike:** `rpc.discover` des Defaults als Negativtest erfassen; Spawn, Resource-
   Insert/Remove, Event-Trigger und Message-Schreiben müssen bei einem kontrollierten Adapter
   nicht erreichbar sein. Ein positives Ergebnis der Default-Methoden ist ein Blocker, kein
   Akzeptanznachweis.
8. **Screenshot-Spike:** bestehende Screenshot-Seam mit BRP-Event-Beispiel vergleichen: Render-
   Readback, kein Simulations-Tick, Artifact-Root, vorhandene Datei, PNG-Verifikation, Breite/
   Höhe und terminale Response. Der BRP-Image-Stream genügt nur, wenn ein separater Adapter all
   diese Regeln nachweisbar übernimmt.

---

## 6. Akzeptanztests für die Architekturprüfung

Die folgenden Tests sind Prüfaufträge für den Prototyp; sie wurden in dieser Recherche nicht
implementiert oder ausgeführt.

| ID | Test | Akzeptanzbedingung |
|---|---|---|
| AT-01 | Ready/ID | v3-Ready meldet Version/Capabilities; Controller sendet keine ID; Session vergibt jede ID eindeutig. |
| AT-02 | Mehrere Pending | Zwei oder mehr Commands können gleichzeitig ausstehen; Outcomes werden unabhängig von Eingangs-/Abschlussreihenfolge zugeordnet. |
| AT-03 | Warp-Steuerung | `Start` bleibt pending; `SetPace` und `Stop` werden währenddessen angenommen; Abschluss genau einmal mit `completed` oder `stopped`. |
| AT-04 | No hidden tick | Observe, Set, Input-Annahme, Mailbox-Poll und Screenshot verändern den Simulations-Tick nur nach der explizit erlaubten Grenze. |
| AT-05 | Schedule order | Alle Anwendungsschedules laufen exakt in bestehender Reihenfolge; `RemoteLast` ist weder ungewollter Simulationstick noch doppelte Control-Phase. |
| AT-06 | Input | Unbekannte/stale Handles, ungültige Zustandsübergänge, nichtfinite Pointerwerte und >16-KiB-Text werden fachlich abgelehnt; gültiger Input wird erst im nächsten Tick konsumiert. |
| AT-07 | Inspect entities | Children sind enthalten; interne Resource-Entities nicht; Handles roundtrippen Index und Generation ohne JSON-Zahlverlust. |
| AT-08 | Inspect statuses | Ein nicht lesbarer einzelner Wert erzeugt `Unavailable` mit stabilem Status; die übrige Query bleibt erfolgreich. |
| AT-09 | Set atomicity | Vor jeder Änderung sind Existenz, Registry, Reflect-Pfad, Deserialisierung und Serialisierbarkeit geprüft; bei Fehler bleibt der alte Wert unverändert; Erfolg liefert Readback. |
| AT-10 | Assets | Live-Assets können mit sessionlokalem opakem Schlüssel gelesen und gesetzt werden; fehlende Typen/Keys haben die beschlossene I1/I3-Semantik. |
| AT-11 | Screenshot | Readback/PNG-Abschluss bleibt pending, zählt keinen Simulationstick, hält Sandbox/Overwrite-Regeln ein und liefert Pfad, Dimensionen und `overwritten`. |
| AT-12 | Allowlist | Nicht freigegebene BRP-Methoden sind nicht discoverbar oder aufrufbar; insbesondere Spawn, Despawn, Insert/Remove, Event-Trigger und Message-Write. |
| AT-13 | Backpressure/Cancel | Eine volle Mailbox oder ein geschlossener Watch-Kanal führt zu einem definierten Session-Outcome und nicht zu einem Deadlock oder stillen Verlust. |
| AT-14 | Feature baseline | Die gewählte direkte/Dispatcher-Variante kompiliert reproduzierbar mit der festgelegten Bevy-0.19.1-Featurekombination; HTTP wird nur bei ausdrücklicher Entscheidung eingebunden. |

**Erwartetes Ergebnis:** Stock-BRP/Variante 3 kann AT-01 bis AT-14 nicht ohne eine zusätzliche
Session- und Adapter-Schicht erfüllen. Besteht der Direct-Handler-Spike nur die read-only-Teile,
ist das ein Argument für A, nicht für Stock-BRP als vollständigen Vertrag.

---

## 7. Quellen und Quellenbewertung

### Beibehaltene Primärquellen

- [Bevy `bevy_remote/src/lib.rs`, v0.19.1](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_remote/src/lib.rs) — BRP-Dokumentation, Request-/Response-/ID-Typen, Mailbox, Handlerausführung, `RemoteLast`, Watch-Lebensdauer; Commit-Ansicht mit Ranges: [b56…/lib.rs](https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_remote/src/lib.rs).
- [Bevy `builtin_methods.rs`, v0.19.1](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_remote/src/builtin_methods.rs) — vollständige Built-in-Liste, Query-/Resource-/Mutation-/Watch-/Schema-/Schedule-Semantik.
- [Bevy `http.rs`, v0.19.1](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_remote/src/http.rs) — alleiniger mitgelieferter HTTP-Transport, Batches, SSE, Kanalgrößen und Loopback-Defaults.
- [Bevy `bevy_remote/Cargo.toml`, v0.19.1](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_remote/Cargo.toml) — Feature- und Dependency-Auswirkungen.
- [Bevy `bevy_internal/Cargo.toml`, v0.19.1](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_internal/Cargo.toml) — Aggregatfeature `bevy_remote` und native Default-Feature-Verknüpfung.
- [Bevy `bevy_app/main_schedule.rs`, v0.19.1](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_app/src/main_schedule.rs) — Main-/Render-Schedule-Reihenfolge.
- [Bevy Remote Client-Beispiel, v0.19.1](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/examples/remote/client.rs) — Query, Schema-/Entity-ID-Nutzung und Message-Beispiel.
- [Bevy Remote Integration-Test, v0.19.1](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/examples/remote/integration_test.rs) — Screenshot-Entity, Event-Watch und WindowEvent-Input als praktische Referenz.
- Lokale Quellen der Untersuchung: `goal.rs`, die damalige Migrationsplanung,
  [`src/client/plugin.rs`](../../../src/client/plugin.rs),
  [`src/client/transport.rs`](../../../src/client/transport.rs),
  [`src/protocol.rs`](../../../src/protocol.rs) sowie die damaligen Pfade
  `src/handle/mod.rs`, `src/observe/world/mod.rs`, `src/observe/world/projection.rs`,
  `src/screenshot/plugin.rs` und `src/screenshot/capture.rs`.

### Bewusst nicht als Baseline verwendet

- Bevy `main`/latest-Dokumentation: kann nach v0.19.1 geänderte Watch-ID-, Cancellation- oder
  Transportsemantik enthalten.
- [PR #16407 „Add BRP watch_id and bevy/unwatch“](https://github.com/bevyengine/bevy/pull/16407):
  nützlicher Hinweis auf eine spätere Designlücke, aber kein Beweis für den geprüften
  v0.19.1-Stand und daher nicht für die positive Bewertung herangezogen.
- Allgemeine Blog-/SEO-Erklärungen zu BRP: gegenüber den getaggten Quelltexten redundant und ohne
  zusätzliche Primärbelege.

---

## 8. Lücken und Rest-Risiken

- Es wurde kein neuer Compile-, Laufzeit-, Netzwerk- oder Performance-Test ausgeführt; die
  Bewertung ist Quellcode- und Vertragsanalyse.
- Die konkrete Frage, ob `world.query` Resource-Entities sichtbar macht, sowie die tatsächliche
  TypeRegistry-Behandlung mehrdeutiger Pfade bleiben Minimal-App-Tests.
- Eine direkte Nutzung öffentlicher BRP-Built-ins kann Reflection-/JSON-Arbeit reduzieren, bindet
  aber an Bevy `0.19.1` und darf bei einem Bevy-Upgrade nicht ungeprüft weiterlaufen.
- Große Query-Antworten, Reflected Images und Event-Buffers besitzen im Stock-BRP keine an den
  v3-Artifact-/Session-Vertrag angepassten Größenbudgets.
- Ein Adapter, der Watch-Kanäle beim ersten `Some` selbst schließt, wäre eine neue Semantik und
  muss gegen Stop, Drop, EOF und Cleanup getestet werden; sie ist nicht Stock-BRP.
- Die Feature-/Compile-Kosten der empfohlenen Direct-Handler-Variante sind noch nicht gemessen.
- Die bestehende lokale Implementierung ist selbst im Umbau zu v3; insbesondere das derzeitige
  v2-Wire in `src/protocol.rs` und die aktuelle synchrone Host-Anfrage sind keine fertige v3-
  Implementationsfreigabe. Die Architekturprüfung schützt nur die bereits festgelegten
  Invarianten.

## Ergebnis

Variante 3/C ist für einen vollständigen Controlled-Session-Vertrag abzulehnen. Die belastbare
technische Richtung ist: v3 bleibt eigener Session-/JSONL-Vertrag; BRP darf höchstens als eng
begrenzte, intern aufgerufene Reflection-/Inspect-Hilfe dienen. Eine spätere Entscheidung für B
muss die oben genannten Blocker mit einem Prototyp nachweisen und darf Stock-BRP-Default-
Semantik nicht stillschweigend als v3 ausgeben.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Der Bericht trennt bestätigte Fakten, Folgerungen, Blocker und Prototypfragen und nennt konkrete lokale Pfade/Ranges sowie Severity: insbesondere src/client/plugin.rs:122-220 und :580-650 (Schedule/Clock), src/client/transport.rs:1-110 (JSONL), src/handle/mod.rs:1-106 (Handles), src/observe/world/projection.rs:1-90 (Status-/Reflectiongrenzen), src/screenshot/plugin.rs:1-122 und src/screenshot/capture.rs:1-150 (Artifact-Lifecycle) sowie goal.rs/migration.md (v3-Vertrag). Die BRP-Nachweise verlinken Bevy v0.19.1/Commit b56fc29d3016e641754765244b5ba3f9cc504671 mit exakten Quellbereichen; Blocker B1-B7 sind nach Severity ausgewiesen."
    }
  ],
  "changedFiles": [
    "crates/bug_hunter/docs/api/research/bevy-remote-as-session-protocol.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [],
  "validationOutput": [
    "Nur der vorgeschriebene deutsche Forschungs-/Entscheidungsbericht wurde geschrieben; keine andere Datei wurde editiert.",
    "Der Bericht endet mit diesem strukturierten acceptance-report und enthält die geforderten review-findings und residual-risks."
  ],
  "residualRisks": [
    "hoch: Variante 3 verletzt Session-ID-, Tick-Warp-, terminale Pending-, Screenshot-, Asset-, Fehler- und Allowlist-Anforderungen.",
    "hoch: RemotePlugin fügt RemoteLast in Main- und bei Render-Unterstützung in Render-Schedule ein; die Integration mit kontrollierten Schedules ist ohne Prototyp nicht sicher.",
    "mittel: konkrete Sichtbarkeit interner Resource-Entities und mehrdeutiger Type Paths ist noch nicht zur Laufzeit getestet.",
    "mittel: Feature-/Compile-Kosten einer direkten bevy_remote-Integration und die Stabilität der öffentlichen Built-in-Handler-API sind ungeprüft.",
    "mittel: BRP-Backpressure, Shared-Watch-Buffer und fehlende One-Shot-Cancellation benötigen gezielte Laufzeittests."
  ],
  "noStagedFiles": true,
  "diffSummary": "Neuer deutscher Architekturbericht zur Eignung von Bevy Remote Protocol 0.19.1 als vollständigem Controlled-Session-Protokoll; keine Quellcodeänderungen und keine Änderungen an anderen Dateien.",
  "reviewFindings": [
    "blocker: Variante 3/C als vollständiger v3-Vertrag — Stock-BRP besitzt keine kontrollierte Tick-/Warp-Semantik und keine terminale One-Shot-Response für Langläufer.",
    "blocker: Variante 3/C — clientvergebene optionale JSON-RPC-IDs und numerische Bevy-Entity-IDs widersprechen hostvergebenen RequestId- und sessionlokalen Handle-Regeln.",
    "high: Bevy bevy_remote/src/lib.rs:671-788 — RemotePlugin::default registriert ein breites Mutationsinterface; RemotePlugin::empty ist in :579-587 privat und eine Allowlist ist nicht standardmäßig vorhanden.",
    "high: Bevy bevy_remote/src/lib.rs:805-905 — RemoteLast wird nach Last beziehungsweise nach Render eingeordnet und kann mit Controlled-Schedule- und Render-Grenzen kollidieren.",
    "high: Bevy builtin_methods.rs:44-111 und :856-1799 — keine dedizierte Asset-Kategorie, kein v3-Status-/Readback-Set, kein Tick-/Screenshot-Artefakt-Command.",
    "medium: Bevy lib.rs:1525-1569 und builtin_methods.rs:1556-1665 — Watch ist pollender Stream ohne Stock-One-Shot-Close; gleiche Event-/Entity-Parameter teilen Event-Buffer."
  ],
  "manualNotes": "Die Empfehlung ist eine evidenzbasierte Architekturprüfung, keine Produktentscheidung und keine Implementationsfreigabe. Variante 3 ist nur als getrennte, ausdrücklich opt-in lokale Debugschnittstelle denkbar; für die Controlled Session bleibt ein eigener v3-Vertrag erforderlich."
}
```