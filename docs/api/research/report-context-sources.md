# Datenquellen für den Report-Kontext

## Fragestellung

R5 muss festlegen, welche Anwendungs- und Session-Daten `Report::create` aus einer laufenden
`session::Session` übernimmt. Der Context soll einen Fehler einordnen und eine Reproduktion
unterstützen. Er ist weder ein vollständiger System-Snapshot noch ein zweites öffentliches
Session-Modell.

## Festgelegte Context-Daten

| Daten | Quelle | Regel |
| --- | --- | --- |
| Package, Target, Features und Anwendungsargumente | validierte `session::launch::Config` | Die Session hält die beim Start wirksame Konfiguration unverändert fest. |
| Anwendungsversion | `package.version` aus `cargo metadata --format-version 1 --no-deps` | Die Session wählt dasselbe Package, das `launch.package` bezeichnet. Eine nicht ermittelbare Version verhindert bereits den Session-Start. |
| Git-Revision und Dirty-Status | Git-Repository des ausgewählten Package-Manifests | Optional. Die Revision ist `HEAD`. Dirty umfasst vorgemerkte, geänderte und nicht ignorierte unversionierte Dateien. Fehlt Git oder `HEAD`, bleibt die Angabe aus. |
| `bug_hunter`-Version | Package-Version der laufenden Host-Implementierung | Die Version stammt aus dem Build des Hosts, nicht aus dem gestarteten Anwendungspackage. |
| Protokollversion und Capabilities | erfolgreicher `Ready`-Handshake | Der Context übernimmt die von der Anwendung bestätigten Werte. |
| Standard-Pace für Tick-Warps | wirksame `command::tick::Config` | Sie ist nötig, um einen Warp ohne eigene Pace einzuordnen. |
| Betriebssystem und Architektur | Build-Ziel des laufenden Hosts | Es genügen die stabilen Werte von `std::env::consts::OS` und `ARCH`. |
| Cargo- und Rustc-Version | beim Session-Start verwendete Toolchain | Die Session speichert getrimmte Versionsausgaben. Kann eine Zusatzabfrage nicht ausgeführt werden, bleibt der jeweilige Wert aus und der Report entsteht trotzdem. |
| Command-Historie | korrelierte Session-Historie | `Report::create` kopiert höchstens die letzten 50 gesendeten Commands samt Outcome in Sendereihenfolge. |

Cargo dokumentiert `cargo metadata` als maschinenlesbare Quelle für Package-Struktur und
Package-Version. Das Ausgabeformat ist versioniert, wenn der Aufruf `--format-version` ausdrücklich
setzt. `--no-deps` reicht hier aus, weil R5 nur das ausgewählte Anwendungspackage und nicht seinen
Abhängigkeitsgraphen benötigt:

- [Cargo: `cargo metadata`](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html)
- [Cargo: Nutzung durch externe Werkzeuge](https://doc.rust-lang.org/cargo/reference/external-tools.html#information-about-package-structure)

`env!("CARGO_PKG_VERSION")` innerhalb der Bibliothek ist keine Quelle für die Anwendungsversion. Der
Wert bezeichnet dort die Version von `bug_hunter`. Ein Git-Tag ist ebenfalls keine verlässliche
Anwendungsversion, weil ein Checkout keinen Tag besitzen muss und Tags nicht dem
`package.version`-Vertrag von Cargo entsprechen.

## Festgelegte öffentliche Form

```rust
pub struct Context {
    pub application: Application,
    pub bug_hunter_version: String,
    pub protocol_version: u32,
    pub capabilities: session::Capabilities,
    pub tick: command::tick::Config,
    pub platform: Platform,
    pub toolchain: Toolchain,
    pub commands: Vec<session::history::Entry>,
}

pub struct Application {
    pub package: String,
    pub version: String,
    pub target: session::launch::Target,
    pub features: Vec<String>,
    pub arguments: Vec<String>,
    pub source: Option<SourceRevision>,
}

pub struct SourceRevision {
    pub commit: String,
    pub dirty: bool,
}

pub struct Platform {
    pub os: String,
    pub arch: String,
}

pub struct Toolchain {
    pub cargo: Option<String>,
    pub rustc: Option<String>,
}
```

`Application` besitzt eine reportspezifische, persistierbare Momentaufnahme. Sie verwendet
`launch::Target`, übernimmt aber nicht `launch::Config` als Ganzes. Dadurch fügt eine spätere
Launch-Option nicht unbemerkt ein neues Report-Feld hinzu.

Ein eigener öffentlicher `session::Context` oder `report::SessionContext` ist nicht nötig.
`session::Session` hält intern die wirksame Konfiguration, die aufgelöste Package-Version, optionale
Quellinformationen, Toolchain-Daten, Handshake-Daten und die Command-Historie. `Report::create`
erstellt daraus einmalig `report::Context`.

## Nicht aufgenommene Daten

Die Entscheidung nimmt folgende Daten nicht in den Context auf:

- absolute Projekt-, Manifest- und Artifact-Pfade,
- Umgebungsvariablen,
- Repository-URL, Branchname und Quelltext-Diff,
- Session-, Controller- und Wire-Request-IDs,
- Provider- und Report-Konfiguration,
- vollständige Cargo-Metadaten und den Abhängigkeitsgraphen,
- einen Bevy-World-Snapshot.

Absolute Pfade und Umgebungsvariablen sind maschinengebunden und können Zugangsdaten enthalten. Ein
Branchname identifiziert keinen Quellstand. Der Commit und der Dirty-Status liefern die nützliche
Information, ohne einen lokalen Diff in den Report zu kopieren.

## Vertrauliche Diagnosewerte

Anwendungsargumente, Texteingaben und Inspect-Outcomes können vertrauliche Werte enthalten. Eine
allgemeine automatische Bereinigung wäre unvollständig. Sie könnte Zugangsdaten übersehen und zugleich
Werte entfernen, die den Fehler reproduzierbar machen.

Die festgelegte Form übernimmt diese Werte deshalb unverändert und behandelt jeden Report als
vertrauliches Artefakt. Eine Veröffentlichung bei GitHub muss unter R6 ausdrücklich bestätigt werden.
Ein standardmäßig bereinigter Context wurde verworfen. Er hätte einen eigenen Report-Datentyp für
Commands und Outcomes, vollständige Bereinigungsregeln und eine Opt-in-Einstellung für vollständige
Diagnosewerte benötigt, ohne unbekannte Zugangsdaten verlässlich erkennen zu können.

## Benötigter interner Session-Zustand

S2 muss die folgenden Daten im privaten Zustand von `session::Session` vorsehen:

1. die validierte und wirksame `launch::Config` sowie `tick::Config`,
2. die durch Cargo aufgelöste Package-Version,
3. die optionale Git-Revision samt Dirty-Status,
4. die `bug_hunter`-, Cargo- und Rustc-Version sowie OS und Architektur,
5. die vom Handshake bestätigte Protokollversion und Capabilities,
6. die korrelierte Command-Historie ohne persistierte Wire-Request-IDs.

Die Metadaten werden beim Session-Start aufgenommen. Ein späterer Wechsel von Branch, Toolchain oder
Konfiguration verändert den Context der bereits laufenden Session nicht.
