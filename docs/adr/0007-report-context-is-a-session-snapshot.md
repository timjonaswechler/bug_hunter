# Report-Kontext ist eine unveränderliche Session-Momentaufnahme

## Status

Angenommen.

## Kontext

Ein Fehlerbericht braucht genug Angaben, um die gestartete Anwendung und die Bedingungen der
Controlled Session einzuordnen. Ein zweites öffentliches `session::Context`-Modell würde denselben
Zustand neben `session::Session` führen und könnte von der laufenden Session abweichen.

Die Anwendungsversion darf nicht aus `env!("CARGO_PKG_VERSION")` innerhalb von `bug_hunter`
stammen. Dieser Wert bezeichnet die Version von `bug_hunter`, nicht die Version des gestarteten
Cargo-Packages. Ein Git-Tag ist ebenfalls keine verlässliche Package-Version.

Anwendungsargumente, Texteingaben und Inspect-Outcomes können vertrauliche Daten enthalten. Eine
allgemeine automatische Bereinigung kann unbekannte Zugangsdaten übersehen und zugleich Daten
entfernen, die für eine Reproduktion nötig sind.

## Entscheidung

1. `Report::create` erzeugt `report::Context` direkt aus der laufenden `session::Session`. Es gibt
   keinen öffentlichen `session::Context` und keinen vom Aufrufer vorbereiteten Report-Kontext.
2. Der Context enthält eine `report::Application` mit Package, Cargo-Package-Version, Target,
   Features, Anwendungsargumenten und optionaler Git-Revision samt Dirty-Status.
3. Die Anwendungsversion ist `package.version` des durch `session::launch::Config::package`
   ausgewählten Packages. Die Session liest sie beim Start aus dem versionierten Ausgabeformat von
   `cargo metadata`.
4. Git ergänzt optional `HEAD` und den Dirty-Status des Repositorys, das das ausgewählte
   Package-Manifest enthält. Git ersetzt nicht die Cargo-Package-Version. Repository-URL, Branch und
   Quelltext-Diff gehören nicht zum Context.
5. Der Context enthält außerdem die `bug_hunter`-Version, die bestätigte Protokollversion und
   Capabilities, die wirksame Tick-Konfiguration, OS und Architektur sowie optionale Cargo- und
   Rustc-Versionsausgaben.
6. Der Context enthält höchstens die letzten 50 gesendeten Commands samt korreliertem Outcome in
   Sendereihenfolge. Wire-Request-IDs werden nicht übernommen. Ein ausstehender Command bleibt als
   `Unanswered` sichtbar.
7. Die Session nimmt unveränderliche Metadaten beim Start auf. Änderungen an Branch, Toolchain oder
   Konfigurationsobjekten verändern den Context einer laufenden Session nicht.
8. Anwendungsargumente, Commands und Outcomes werden unverändert übernommen. Reports gelten als
   vertrauliche Artefakte. Ein externer Provider darf einen Report nur nach ausdrücklicher
   Veröffentlichungsentscheidung senden.
9. Absolute Projekt-, Manifest- und Artifact-Pfade, Umgebungsvariablen, Session- und Controller-IDs,
   Provider-Konfiguration, vollständige Cargo-Metadaten und Bevy-World-Snapshots werden nicht
   aufgenommen.
10. Eine fehlende Git- oder Toolchain-Zusatzangabe verhindert weder Session noch Report. Kann Cargo
    die Package-Version nicht auflösen, scheitert bereits der Session-Start.

## Folgen

- Der Context beschreibt die beim Start wirksamen Bedingungen und keine später neu gelesene
  Konfiguration.
- Cargo-Package-Version und Git-Revision erfüllen verschiedene Aufgaben. Die Package-Version ist die
  Anwendungsangabe, der Commit ordnet den Quellstand ein.
- Ein Dirty-Status weist auf nicht durch den Commit beschriebene Änderungen hin, kopiert diese
  Änderungen aber nicht in den Report.
- OS, Architektur und Toolchain helfen bei plattform- oder compilerabhängigen Fehlern, ohne absolute
  lokale Pfade offenzulegen.
- Reports können weiterhin vertrauliche Werte enthalten. Lokale Dateien und Entwürfe müssen
  entsprechend behandelt werden. R6 muss eine versehentliche GitHub-Veröffentlichung ausschließen.
- `session::Session` benötigt privaten Zustand für die aufgelösten Metadaten und die korrelierte
  Historie, aber kein zweites öffentliches Context-Modell.

## Prüfung

- Workspace-Fixtures prüfen direkte und geerbte Cargo-Package-Versionen sowie die Auswahl des
  konfigurierten Packages und Targets.
- Git-Fixtures prüfen saubere und geänderte Worktrees, unversionierte Dateien, fehlendes Git und ein
  Repository ohne `HEAD`.
- Context-Fixtures prüfen die Grenze von 50 Commands, Sendereihenfolge, korrelierte Outcomes,
  `Unanswered` und das Fehlen von Wire-Request-IDs.
- Fixtures prüfen die Übernahme von Anwendungsargumenten und Diagnosewerten sowie den Ausschluss von
  Umgebung, absoluten Pfaden, Provider-Konfiguration und Quelltext-Diffs.
- Fehlschlagende optionale Git-, Cargo-Versions- und Rustc-Versionsabfragen verhindern die
  Report-Erzeugung nicht.
