# Session-Pfade werden am Cargo-Manifest verankert

Historischer Entscheidungsstand. Für den Rewrite gilt [target.md](../api/target.md).
Die aktuelle Zuordnung unterscheidet direkte Rust-Roots von serverseitig erzeugten
ID-Unterverzeichnissen.

## Status

Angenommen.

Die unten erwähnte `host::run`-Fassade entfällt nach
[ADR-0024](0024-separate-server-client-and-cli.md). Die Pfadregeln gelten unverändert für
direkte Rust-Session-Nutzung und für Sessions, die der Server erzeugt.

## Kontext

Eine Controlled Session benötigt einen festen Ausgangspunkt für `cargo metadata`, `cargo run`, Git
und `gh`. `launch::Config` nannte bisher nur Package, Target, Features und
Anwendungsargumente. Das aktuelle Arbeitsverzeichnis des Host-Prozesses wäre damit eine
stillschweigende zweite Launch-Konfiguration.

Screenshot, Recording und Report benötigen außerdem dasselbe Artifact-Verzeichnis. Der aktuelle
Code unterscheidet noch Host- und Child-Roots. Diese Trennung lässt offen, wo ein Artifact eines
Commands oder Reports liegt und welche Grenze für relative Pfade gilt.

Ein Ordner allein bezeichnet ein Cargo-Projekt nicht eindeutig. Cargo besitzt mit
`--manifest-path` bereits eine eindeutige Eingabe für Package- und Workspace-Manifeste.

## Entscheidung

1. `session::launch::Config` erhält das Pflichtfeld `manifest_path: PathBuf`.
2. `Session::start` löst einen relativen Manifestpfad einmalig gegen das aktuelle Arbeitsverzeichnis
   des aufrufenden Prozesses auf. Der Pfad muss auf eine vorhandene reguläre Datei zeigen und wird
   kanonisiert. Sein Elternordner ist der unveränderliche `project_dir` der Session.
3. `cargo metadata` und `cargo run` erhalten den kanonischen Pfad über `--manifest-path`.
   `cargo run` läuft mit `project_dir` als Arbeitsverzeichnis. Das weiterhin verpflichtende
   `launch::Config::package` wählt das Package ausdrücklich aus.
4. Cargo-Metadaten liefern den tatsächlichen Manifestpfad des ausgewählten Packages. Git-Revision
   und Dirty-Status stammen wie bisher beschlossen aus dem Repository, das dieses Package-Manifest
   enthält.
5. Der GitHub-Provider führt `gh` mit `project_dir` als Arbeitsverzeichnis aus. Er übergibt weiterhin
   kein `--repo`.
6. `session::Config::artifact_dir` bezeichnet das einzige Artifact-Root der Session. Screenshot,
   Recording und Report verwenden keine getrennten Host- und Child-Roots.
7. Ein relativer Artifact-Pfad wird gegen `project_dir` aufgelöst. Ein absoluter Pfad und ein Root
   außerhalb des Projekts bleiben erlaubt. Der konfigurierte Root darf nicht leer und nicht selbst
   ein Symlink sein.
8. `Session::start` legt einen fehlenden Artifact-Root einschließlich seiner Elternverzeichnisse an,
   kanonisiert ihn und speichert den absoluten Pfad unveränderlich. Relative Pfade einzelner
   Artifacts dürfen diesen Root nicht verlassen.
9. `Session::start` übergibt den kanonischen Artifact-Root über die reservierte interne
   Umgebungsvariable `BUG_HUNTER_ARTIFACT_DIR` an den Kindprozess. Ein vorhandener Wert wird für den
   Kindprozess überschrieben. `session::Plugin` liest den Wert einmal vor dem Ready-Handshake.
10. Die Umgebungsvariable ist kein öffentliches Konfigurationsinterface, keine Capability und kein
    Feld des v3-Protokolls. Der absolute Projekt-, Manifest- und Artifact-Pfad bleibt aus
    `report::Context` ausgeschlossen.
11. Ein leerer Pfad, ein nicht reguläres Manifest oder ein Artifact-Root, der selbst ein Symlink ist,
    ergibt `session::Error::InvalidConfig`. Fehler beim Anlegen oder Kanonisieren des Artifact-Roots
    ergeben `session::Error::Io`. Kann Cargo Workspace, Package oder Version nicht auflösen, ergibt
    der Start `session::Error::Launch`.
12. Kann die Controlled Session den übergebenen Artifact-Root nicht lesen, erreicht sie den
    Ready-Handshake nicht. Der Host erhält `session::Error::Launch` und räumt den Kindprozess auf.

## Folgen

- Ein Wechsel des globalen Arbeitsverzeichnisses verändert eine laufende Session nicht.
- Ein Host kann ein Workspace- oder Package-Manifest angeben. Cargo-Metadaten bleiben die Quelle für
  das tatsächlich ausgewählte Package.
- Relative Artifact-Pfade verhalten sich bei direkter Verwendung von `Session` und über
  `host::run` gleich.
- Das Artifact-Root darf ausdrücklich außerhalb des Cargo-Projekts liegen. Erst die Pfade einzelner
  Artifacts bilden die eingeschränkte Sicherheitsgrenze.
- Der private Host-Config-Parser muss einen Manifestpfad liefern. Seine konkrete TOML- und
  CLI-Darstellung wird mit dem Host-Interface festgelegt.
- Die bisherigen getrennten Host- und Child-Artifact-Roots sowie die öffentliche Konfiguration der
  Übergabe entfallen.

## Prüfung

- Workspace-Fixtures starten über ein Workspace- und ein Package-Manifest und wählen das
  konfigurierte Package eindeutig aus.
- Eine Fixture wechselt nach `Session::start` das Arbeitsverzeichnis und prüft, dass Cargo-, Git-,
  GitHub- und Artifact-Pfade unverändert bleiben.
- Pfad-Fixtures prüfen relative und absolute Manifest- und Artifact-Pfade, ein fehlendes Manifest,
  ein Verzeichnis statt eines Manifests, einen fehlenden Artifact-Root und einen Symlink als Root.
- Eine Fixture verwendet ein Artifact-Root außerhalb des Projekts und prüft Screenshot, Recording
  und lokalen Report unter demselben Root.
- Eine Kindprozess-Fixture prüft, dass `Session::start` einen vorhandenen
  `BUG_HUNTER_ARTIFACT_DIR`-Wert überschreibt und `session::Plugin` den kanonischen Wert erhält.
- Report-Fixtures prüfen weiterhin, dass absolute Projekt-, Manifest- und Artifact-Pfade nicht im
  Report-Kontext erscheinen.
