mod handle {
    pub struct Handle {
        pub index: u32,
        pub generation: u32,
    }
}

mod command {
    mod private {
        pub trait Sealed {}
    }

    // Nur die von bug_hunter definierten konkreten Command-Typen implementieren dieses Trait.
    pub trait Request: private::Sealed + Into<Command> {
        type Output;
    }

    // Gemeinsame typgelöschte Darstellung für Parsing, History, Recording und interne Ausführung.
    // Dieser Typ implementiert nicht Request. Shutdown ist nur über Session::shutdown ausführbar.
    pub enum Command {
        Input(input::Command),
        Tick(tick::Command),
        Inspect(inspect::Command),
        Screenshot(screenshot::Command),

        Recording(recording::Command),

        Replay(replay::Command),

        Shutdown,
    }

    pub mod input {
        pub enum Command {
            Keyboard(keyboard::Command),
            Pointer(pointer::Command),
            Text(text::Command),
        }

        mod keyboard {
            enum Command {
                Press { key: Key },
                Release { key: Key },
            }

            pub enum Key {}

            impl Request for Command {
                type Output = ();
            }

            macro_rules! define_keys {}
        }

        mod pointer {
            enum Command {
                Press {
                    button: PointerButton,
                },
                Release {
                    button: PointerButton,
                },
                MoveTo {
                    position: [f32; 2],
                },
                MoveBy {
                    delta: [f32; 2],
                },
                Scroll {
                    delta: [f32; 2],
                },
            }

            enum PointerButton {
                Left,
                Right,
                Middle,
            }

            impl Request for Command {
                type Output = ();
            }
        }

        mod text {
            enum Command {
                Input { text: String },
            }

            impl Command {
                pub fn new(text: impl Into<String>) -> Self;
            }

            impl Request for Command {
                type Output = ();
            }
        }
    }

    pub mod tick {
        pub struct Config {
            pub pace: warp::Pace,
        }

        pub enum Command {
            Warp(warp::Command),
        }

        pub mod warp {
            pub enum Pace {
                AsFastAsPossible,
                TicksPerSecond { target: f32 },
            }

            pub enum Command {
                Start {
                    ticks: u64,
                    pace: Option<Pace>,
                },
                SetPace {
                    pace: Pace,
                },
                Stop,
            }

            pub enum Output {
                Finished(Completion),
                PaceChanged { pace: Pace },
                Stopped { was_running: bool },
            }

            pub struct Completion {
                pub requested_ticks: u64,
                pub executed_ticks: u64,
                pub outcome: Outcome,
            }

            pub enum Outcome {
                Completed,
                Stopped,
            }

            impl crate::command::Request for Command {
                type Output = Output;
            }
        }
    }

    pub mod inspect {
        pub use query::{Command, Output};

        pub mod value {
            pub enum Output {
                Readable { value: serde_json::Value },
                Unavailable { status: Status },
            }

            pub enum Status {
                Missing,
                NotRegistered,
                NotReflectable,
                NotSerializable,
            }

            pub enum Projection {
                Metadata,
                Value,
            }
        }

        pub mod query {
            // Jeder vom Aufrufer gelieferte Type Path muss sich vor der Abfrage exakt über Bevys
            // TypeRegistry auflösen lassen. Andernfalls wird der gesamte Command ohne Teilergebnis
            // abgelehnt.
            pub struct Command {
                pub source: Source,
            }

            pub enum Source {
                Entities(entity::Query),
                Resources(resource::Query),
            }

            pub struct Output {
                pub items: Vec<Item>,
            }

            impl crate::command::Request for Command {
                type Output = Output;
            }

            pub enum Item {
                Entity(entity::Output),
                Resource(resource::Output),
            }

            pub mod component {
                pub enum Selection {
                    All,
                    Listed { type_paths: Vec<String> },
                }

                pub struct Metadata {
                    pub name: String,
                    pub type_path: Option<String>,
                }

                pub struct Output {
                    pub component: Metadata,
                    pub value: super::super::value::Output,
                }
            }

            pub mod entity {
                pub struct Query {
                    pub entity: Option<Handle>,
                    pub with: Vec<String>,
                    pub without: Vec<String>,
                    pub projection: Projection,
                }

                pub enum Projection {
                    Summary,
                    ComponentNames,
                    Components {
                        selection: super::component::Selection,
                    },
                    Hierarchy {
                        depth: u8,
                    },
                }

                pub struct Output {
                    pub entity: Handle,
                    pub result: Result,
                }

                pub enum Result {
                    Summary {
                        name: Option<String>,
                        component_count: u32,
                    },
                    ComponentNames {
                        components: Vec<super::component::Metadata>,
                    },
                    Components {
                        components: Vec<super::component::Output>,
                    },
                    Hierarchy {
                        root: hierarchy::Node,
                    },
                }

                pub mod hierarchy {
                    pub struct Node {
                        pub entity: Handle,
                        pub name: Option<String>,
                        pub children: Vec<Node>,
                    }
                }
            }

            pub mod resource {
                pub struct Query {
                    pub selector: Selector,
                    pub projection: super::super::value::Projection,
                }

                pub enum Selector {
                    All,
                    Type { type_path: String },
                }

                pub enum Output {
                    Metadata {
                        type_path: String,
                    },
                    Value {
                        type_path: String,
                        value: super::super::value::Output,
                    },
                }
            }
        }
    }

    pub mod screenshot {
        pub enum Command {
            Capture {
                path: String,
            },
        }

        pub struct Output {
            pub path: String,
            pub width: u32,
            pub height: u32,

            pub overwritten: bool,
        }

        impl Request for Command {
            type Output = Output;
        }
    }

    pub mod recording {
        pub enum Command {
            Start { path: String },
            Stop,
        }

        pub enum Output {
            Started {
                path: String,
            },
            Stopped {
                path: String,
                recorded_commands: u64,
            },
        }

        impl Request for Command {
            type Output = Output;
        }
    }

    pub mod replay {
        // Start lädt die Recording vollständig und bildet daraus einen internen Replay-Plan.
        // Gestoppte Warps werden auf ihre tatsächlich ausgeführten Ticks verkürzt; unmittelbar
        // aufeinanderfolgende kompatible Warps dürfen zusammengefasst werden.
        // Während Preparing, Running und Stopping ist von außen nur Stop zulässig.
        pub enum Command {
            Start { path: String },
            Stop,
        }

        pub enum Output {
            // Antwort auf Start, nachdem der Plan vollständig beendet wurde.
            Finished(Completion),

            // Antwort auf Stop, nachdem alle bereits gesendeten Commands terminal sind.
            Stopped { was_running: bool },
        }

        pub struct Completion {
            pub outcome: Outcome,
        }

        pub enum Outcome {
            Completed,

            Stopped,

            // Stabile Codes: command_protocol_failed, session_io_failed, session_ended,
            // request_id_exhausted, stop_failed.
            Blocked { code: String, message: String },
        }

        impl Request for Command {
            type Output = Output;
        }
    }
}

mod report {
    pub struct Report {
        title: String,
        failure: Failure,
        signature: Signature,
        context: Context,
    }

    pub struct Failure {
        origin: Origin,
        message: Option<String>,
    }

    pub enum Origin {
        Panic {
            location: Option<Location>,
            backtrace: Option<String>,
        },
        ProcessExit {
            status: String,
        },
        TracingError {
            target: Option<String>,
            location: Option<Location>,
        },
    }

    pub struct Signature {
        value: String,
    }

    pub struct Location {
        pub file: String,
        pub line: u32,
        pub column: Option<u32>,
    }

    pub struct Context {
        pub application: Application,
        pub bug_hunter_version: String,
        pub protocol_version: u32,
        pub capabilities: crate::session::Capabilities,
        pub tick: crate::command::tick::Config,
        pub platform: Platform,
        pub toolchain: Toolchain,
        pub commands: Vec<crate::session::history::Entry>,
    }

    pub struct Application {
        pub package: String,
        pub version: String,
        pub target: crate::session::launch::Target,
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

    pub struct Config {
        pub tracing_errors: bool,
        pub output: std::path::PathBuf,
        pub provider: provider::Config,
    }

    impl Report {
        pub fn create(failure: Failure, session: &crate::session::Session) -> Self;

        pub fn title(&self) -> &str;

        pub fn failure(&self) -> &Failure;

        pub fn signature(&self) -> &Signature;

        pub fn context(&self) -> &Context;

        pub fn to_markdown(&self) -> String;
    }

    impl Failure {
        pub fn panic(
            message: Option<String>,
            location: Option<Location>,
            backtrace: Option<String>,
        ) -> Self;

        pub fn process_exit(status: String) -> Self;

        pub fn tracing_error(
            message: String,
            target: Option<String>,
            location: Option<Location>,
        ) -> Self;

        pub fn origin(&self) -> &Origin;

        pub fn message(&self) -> Option<&str>;
    }

    impl Signature {
        pub fn as_str(&self) -> &str;
    }

    pub fn submit(
        report: &Report,
        session: &crate::session::Session,
    ) -> Result<provider::Outcome, Error>;

    pub enum Error {
        Local(provider::local::Error),
        FallbackFailed {
            provider: provider::Error,
            local: provider::local::Error,
        },
    }

    pub mod provider {
        pub enum Error {
            Github(github::Error),
        }

        pub enum Config {
            Local(local::Config),
            Github(github::Config),
        }

        pub struct FileReference {
            pub path: std::path::PathBuf,
        }

        pub enum Reference {
            File(FileReference),
            Issue { identifier: String, url: String },
        }

        pub enum Outcome {
            Created { reference: Reference },
            Existing { reference: Reference },
            Fallback {
                reference: FileReference,
                provider_error: Error,
            },
        }

        pub mod local {
            pub struct Config {}

            pub enum Error {
                InvalidPath {
                    path: std::path::PathBuf,
                },
                Conflict {
                    path: std::path::PathBuf,
                },
                Filesystem {
                    operation: Operation,
                    path: std::path::PathBuf,
                    message: String,
                },
            }

            pub enum Operation {
                Read,
                Write,
            }
        }

        pub mod github {
            pub struct Config {}

            pub enum Error {
                Unavailable {
                    operation: Operation,
                    message: String,
                },
                CommandFailed {
                    operation: Operation,
                    message: String,
                },
                InvalidResponse {
                    operation: Operation,
                    message: String,
                },
            }

            pub enum Operation {
                Search,
                Publish,
            }
        }
    }
}

mod session {
    pub struct Plugin {}

    impl Default for Plugin {}
    impl bevy::app::Plugin for Plugin {}

    pub fn tracing_error_layer(app: &mut bevy::app::App) -> Option<bevy::log::BoxedLayer>;

    pub struct Capabilities {
        pub screenshot: bool,
    }

    pub struct Config {
        pub launch: launch::Config,
        pub artifact_dir: std::path::PathBuf,
        pub tick: crate::command::tick::Config,
        pub report: crate::report::Config,
    }

    // Wird ausschließlich von Session vergeben und niemals vom Controller gewählt.
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct RequestId(u64);

    impl RequestId {
        pub fn as_u64(self) -> u64;
    }

    impl std::fmt::Display for RequestId {}

    pub mod launch {
        pub struct Config {
            pub manifest_path: std::path::PathBuf,
            pub package: String,
            pub target: Target,
            pub features: Vec<String>,
            pub arguments: Vec<String>,
        }

        pub enum Target {
            Binary { name: String },
            Example { name: String },
        }
    }

    pub mod history {
        pub struct Entry {
            pub command: crate::command::Command,
            pub outcome: Outcome,
        }

        pub enum Outcome {
            Completed { output: serde_json::Value },
            Rejected { code: String, message: String },

            ProtocolFailed { code: String, message: String },

            IoFailed { message: String },

            Unanswered,
        }
    }

    pub mod protocol {
        pub const VERSION: u32 = 3;

        pub struct Ready {
            pub version: u32,
            pub capabilities: crate::session::Capabilities,
        }

        pub struct Request {
            pub request_id: crate::session::RequestId,
            pub command: Command,
        }

        pub enum Command {
            Input(crate::command::input::Command),
            Tick(crate::command::tick::Command),
            Inspect(crate::command::inspect::Command),
            Screenshot(crate::command::screenshot::Command),
            Shutdown,
        }

        pub enum Message {
            Ready(Ready),
            Response(Response),

            ProtocolError(ProtocolError),
        }

        pub struct Response {
            pub request_id: crate::session::RequestId,
            // Vollständig qualifizierter Name des beantworteten Commands.
            pub command: String,
            pub outcome: Outcome,
        }

        pub enum Outcome {
            Completed { output: serde_json::Value },
            Rejected { error: Diagnostic },
        }

        pub struct ProtocolError {
            pub request_id: Option<crate::session::RequestId>,
            pub error: Diagnostic,
        }

        pub struct Diagnostic {
            pub code: String,
            pub message: String,
        }
    }

    // Nicht klonbar und genau einer Session sowie einem angenommenen Command zugeordnet.
    pub struct Pending<Output> {}

    impl<Output> Pending<Output> {
        pub fn request_id(&self) -> RequestId;
        pub fn command(&self) -> &crate::command::Command;
    }

    pub enum Event {
        ProtocolError { code: String, message: String },

        RecordingFailed {
            path: String,
            message: String,
        },

        Failure {
            failure: crate::report::Failure,
        },

        ObservationError {
            // Stabiler Code für ungültige oder bei EOF unvollständige Marker:
            // invalid_report_marker.
            code: String,
            message: String,
        },

        Ended {
            reason: EndReason,
        },
    }

    pub enum EndReason {
        ProcessExit {
            status: String,
        },

        TransportClosed {
            channel: TransportChannel,
        },

        TransportFailed {
            channel: TransportChannel,
            message: String,
        },

        EventQueueOverflow {
            capacity: u32,
            dropped_events: u64,
        },
    }

    pub enum TransportChannel {
        Stdin,
        Stdout,
        Stderr,
    }

    // Synchroner, nicht klonbarer Handle zu einem privaten Session-Koordinator. Angenommene
    // Commands und hostseitige Arbeit laufen unabhängig von receive-Aufrufen weiter.
    pub struct Session {}

    impl Session {
        pub fn start(config: Config) -> Result<Self, Error>;

        pub fn capabilities(&self) -> &Capabilities;

        // Akzeptiert nur die versiegelten konkreten Request-Typen. Die dynamische Verarbeitung von
        // command::Command für die eingebauten Host-Abläufe bleibt crate-intern bei session.
        pub fn send<C>(&mut self, command: C) -> Result<Pending<C::Output>, Error>
        where
            C: crate::command::Request;

        pub fn try_receive<Output>(
            &mut self,
            pending: &mut Pending<Output>,
        ) -> Result<Option<Output>, Error>;

        pub fn receive<Output>(&mut self, pending: Pending<Output>) -> Result<Output, Error>;

        pub fn try_receive_event(&mut self) -> Result<Option<Event>, Error>;
        pub fn receive_event(&mut self) -> Result<Event, Error>;

        pub fn shutdown(&mut self) -> Result<(), Error>;
    }

    pub enum Error {
        InvalidConfig { message: String },
        Launch { message: String },
        Io { message: String },
        InvalidPending { message: String },
        RequestIdExhausted,
        ShutdownBlocked { code: String, message: String },

        Rejected { code: String, message: String },
        Protocol { code: String, message: String },

        Ended,
    }
}

// OPEN(H7): Crates, ausführbare Programme, öffentliche Exports und Feature-Zuschnitt festlegen.
// Die folgende Modulaufteilung entscheidet nur über Verantwortungen.
mod server {
    // Besitzt das Verzeichnis und die Handles mehrerer unabhängiger Sessions.
    // Startet ohne Session; jede Session wird anschließend ausdrücklich erstellt.
    // Serverconfig enthält nur serverweite Werte, keine impliziten Spiel-Launch-Defaults.
    // Session-Erzeugen erhält eine eigene vollständige Config, ohne Vererbung vom Server.
    // Erste Version ohne Live-Neukonfiguration: Serverneustart bzw. neue Session erforderlich.
    // Server leitet Artefaktverzeichnis als <Basisordner>/<vollständige Session-ID>/ ab.
    // Beispiel: worktrees/<32-stellige ID>/; Kurzformen werden nicht als Pfadnamen verwendet.
    // Basisordner aus Serverconfig, beim Start relativ zum Arbeitsverzeichnis absolut auflösen.
    // Basisordner ist Pflicht und nicht leer; Anlage-/Auflösungsfehler verhindern Serverbereitschaft.
    // Shutdown-Frist positiv und endlich, Standard 30 s; kein unbegrenzter Wert.
    // Ungültige Serverconfig: Startfehler statt stillschweigender Korrektur.
    // Vorhandene ID-Verzeichnisse nicht wiederverwenden; neue ID wählen vor ihrer Rückgabe.
    // Anlage während Starting nach Annahme; Anlagefehler -> Failed unter der vergebenen ID.
    // Keine automatische Löschung, kein Session-Artefakt-Override im Serverbetrieb.
    // Freigabe erst nach aller Schreibarbeit inklusive Reports; vorhandene Dateien bleiben erhalten.
    // Erste Version: Vordergrundbetrieb, kein eingebauter Hintergrundstart.
    // CLI erhält die Serveradresse ausdrücklich; keine Discovery und kein impliziter Serverstart.
    // Bereit nach Initialisierung und Annahmebereitschaft für Verwaltungsaufrufe, ohne Spielinstanz.
    // Dann zeigt die CLI die tatsächliche Adresse; Startfehler beenden den Prozess mit Fehlerstatus.
    // Kein stillschweigendes Ausweichen auf eine andere Adresse.
    // Jede Session besitzt weiterhin ihre fachliche Ausführung und ihren Spielprozess.
    // Leert Session-Events und führt den gemeinsamen Report-Ablauf unabhängig von Clients aus.
    // Server vergibt im Session-Verzeichnis eindeutige IDs; Clients geben keine eigene ID vor.
    // 128 zufällige Bits als 32 kleine Hex-Zeichen ohne Bindestriche; bei Kollision neu erzeugen.
    // Maschinenantworten enthalten volle IDs; Anzeige ab 8 Zeichen bis zur Eindeutigkeit verlängern.
    // Volle ID ist die Identität; Kurzform ist nur ein Präfix, keine eigene ID.
    // Eingaben benötigen 8 bis 32 Hex-Zeichen; Eindeutigkeit wird zusätzlich geprüft.
    // Server löst Präfixe über alle Einträge inklusive Ended/Failed auf.
    // Genau ein Treffer bindet an die volle ID; kein/mehrere Treffer werden abgelehnt.
    // Spätere neue Einträge verändern eine bestehende Bindung nicht.
    // Annahme und Verzeichniseintrag liefern die ID, ohne auf Spielbereitschaft zu warten.
    // Startfortschritt und Spielstartfehler bleiben über diese ID der Session zugeordnet.
    // Unlesbare Anfrage oder fachlich ungültige Config: Ablehnung ohne Session-Eintrag.
    // Vorbereitung und Spielstart folgen der Annahme; ihre Fehler gehören zur Session-ID.
    // Ended/Failed bleiben bis Serverende im Verzeichnis, ohne automatische zeitliche Löschung.
    // Das hält keine Spielprozesse am Leben und garantiert keine Activity-/Log-Aufbewahrung.
    // Vor Annahme: Dekodierung, Pflichtwerte, Wertebereiche/Kombinationen und ID-Auswahl.
    // Danach: Cargo-/Manifest-Auflösung, Dateien/Programme öffnen, Verzeichnisanlage und Start.
    // Umgebungsfehler ergeben Failed unter der ID; fachliche Validierung bleibt beim bisherigen Owner.
    // Erstellen bestätigt Anlage mit vollständiger ID, nicht Spielbereitschaft.
    // Liste: alle Einträge mit voller ID, Zustand und Erstellungszeit, älteste zuerst.
    // Detail: zusätzlich zugewiesener Artefaktpfad und bei Failed Fehlercode/Meldung.
    // Liste/Detail sind Momentaufnahmen, keine automatische Ausgabe der Launch-Konfiguration.
    // OPEN(HS1, HS2): Konkrete Signaturen, Zeitformat und Antwort-Hüllen.
    // Ausdrückliches Serverende fährt alle Sessions herunter und beendet danach den Server.
    // Dabei keine neuen Sessions/Spiel-Commands; stoppbare Arbeit stoppen, übrige abschließen.
    // Danach Recording sauber beenden und Session-Shutdown ausführen, kein unmittelbarer harter Abbruch.
    // Konfigurierbare Frist für gesamten Server-Shutdown; danach verbliebene Spielprozesse
    // erzwungen beenden und Ressourcen aufräumen. Erzwingung wird als Fehler gemeldet.
    // Standard 30 s ab Annahme des Serverendes, über Serverkonfiguration anpassbar.
    // Sessions machen beim Herunterfahren unabhängig voneinander Fortschritt.
    // Erstes Ctrl+C oder SIGTERM startet denselben Shutdown; zweites Ctrl+C erzwingt Abbruch.
    // SIGKILL und Serverabsturz garantieren keinen geordneten Abschluss.
    // Serverende beendet auch Sessions in Starting; sie werden nicht vom Shutdown ausgenommen.
    // Fehler einer Session blockieren andere nicht; nach Bereinigung insgesamt Fehler melden.
    // Verarbeitet Outcomes, Events und Reports bis zum Verbindungsende weiter, ohne Abholpflicht.
    // Report-Arbeit einschließlich Reports echter Spielfehler nutzt dieselbe Shutdown-Frist.
    // Sessions und Reports fertig: früher enden; absichtliches Beenden ist kein Spielbug.
    // Abbruch meldet unvollständigen Abschluss; externer Provider-Ausgang kann unbekannt sein.
    // HTTP-Verwaltung und gebundene WebSocket-Session-Verbindungen an derselben Loopback-Adresse.
    // Gemeinsamer Zugriffsschlüssel schützt beide Zugänge; Session-ID ist keine Berechtigung.
    // OPEN(HS2, HS3): Schlüsselbereitstellung/-prüfung, Routing, Activity und Cursor.

    // Sichtbarer Session-Lebenszyklus; Recording, Replay und Commands sind davon getrennt.
    // Ein einzelner Command-Fehler bedeutet nicht automatisch Failed.
    // Failed erhält stabilen maschinenlesbaren Fehlercode und verständliche Meldung.
    // Starting/Ready -> Stopping bei Shutdown-Beginn; Ended/Failed bleiben unverändert.
    // Stopping bleibt beobachtbar, nimmt aber keine neue Spielarbeit an.
    // Sauberer Abschluss -> Ended, auch bei absichtlich beendetem Start.
    // Shutdown-Fehler oder erzwungener Abbruch -> Failed nach Ressourcenbereinigung.
    enum SessionState {
        Starting, // Angelegt; Vorbereitung oder Spielstart läuft.
        Ready,    // Spielstart und Verbindung abgeschlossen; bedeutet nicht untätig.
        Stopping, // Geordnetes Herunterfahren läuft.
        Ended,    // Regulär beendet.
        Failed,   // Vorbereitung, Spielstart oder weiterer Session-Betrieb fehlgeschlagen.
    }
}

mod client {
    // Gemeinsame Seam für Verwaltungsaufrufe, Commands und Activity.
    // Kapselt Verbindung, Protokollkodierung und Antwortzuordnung, ohne Terminaldarstellung.
    // Serververwaltung: Auflisten und Erstellen ohne Session-Bindung.
    // Session-Verbindung: Steuerung und Beobachtung genau einer Session.
    // Keine Umwandlung einer Verwaltungs- in eine Session-Verbindung; nach Erstellen
    // wird mit der erhaltenen ID eine eigene Session-Verbindung geöffnet.
    // HTTP für Verwaltung, WebSocket für gebundene Session-Verbindungen.
    // OPEN(HS2, H7): Konkrete Interfaces der beiden Zugänge.
    // Commands adressieren eine Session; ausstehende Arbeit und Activity bleiben serverseitig.
    // Client verbindet sich und bindet sich zur Steuerung an genau eine Session gleichzeitig.
    // Commands nutzen diese Bindung, ohne erneute Session-ID pro Command.
    // Mehrere Clients dürfen dieselbe Session steuern; keine exklusive Bindung.
    // Bindung bleibt für die Verbindung fest; andere Session benötigt eine neue Verbindung.
    // Trennung beendet weder die Session noch ihre laufende Arbeit.
    // Verbindung zu jedem vorhandenen Eintrag, auch Stopping, Ended oder Failed.
    // Verbindung verspricht keine Steuerbarkeit; Beobachtung richtet sich nach Aufbewahrung.
    // Commands mit Bereitschaftsvoraussetzung werden außerhalb Ready abgelehnt, nicht vorgemerkt.
    // OPEN(HS1, HS4): Einzel-Session-Abbruch während Starting; genaue Command-Ablehnungsabbildung.
    // OPEN(HS1, HS2): Anmeldung und Fehler beim Verbindungsaufbau.
    // Platzhalter für den gebundenen Session-Zugang, den REPL und Script verwenden.
    pub struct Client {}
}

// OPEN(H4): Agent-Zugang und dessen Modulplatzierung festlegen. Ein CLI-nutzender Agent braucht
// nicht zwingend ein eigenes Modul; eine eigene oder eingebundene Laufzeit ist noch nicht beschlossen.

mod cli {
    // Argumente, Terminaldarstellung und Auswahl des Ablaufs; keine Session-Regeln.
    // CLI zuerst; Weboberflächen und weitere UIs sind nicht Teil des aktuellen Umfangs.
    // OPEN(H7): Einstiegspunkte festlegen; die bisherige host::run-Fassade entfällt.

    mod repl {
        // Komplexe Inspect-Queries verwenden `inspect query <arguments-json>` und damit direkt den
        // gemeinsamen Inspect-Codec. Vier feste Kurzformen erzeugen lediglich häufige Commands;
        // die REPL besitzt kein eigenes Query-Modell und keine vollständige Inspect-Flag-Sprache.
        // Nutzt die Session-Bindung des Clients statt einer Session-ID pro Command.
        pub enum Exit {
            Quit,
            InputClosed,
            Interrupted,
        }

        pub fn run(client: &mut crate::client::Client) -> Result<Exit, Error>;

        pub enum Error {
            Terminal { message: String },
            Session(crate::session::Error),
        }
    }

    mod script {
        pub struct Script {
            commands: Vec<crate::command::Command>,
        }

        impl Script {
            // Liest ein versioniertes JSON-Dokument und validiert alle Command-Einträge vor der
            // Ausführung. Die persistierte Formatversion wird nicht Teil des Laufzeitmodells.
            pub fn parse(source: &str) -> Result<Self, Error>;

            pub fn new(commands: Vec<crate::command::Command>) -> Result<Self, Error>;

            pub fn commands(&self) -> &[crate::command::Command];
        }

        pub enum Outcome {
            Passed,
            Failed {
                failures: Vec<Failure>,
            },
        }

        pub struct Failure {
            pub command_index: usize,
            pub request_id: Option<crate::session::RequestId>,
            pub command: crate::command::Command,
            pub reason: Reason,
        }

        pub enum Reason {
            Rejected {
                code: String,
                message: String,
            },
            Failed {
                message: String,
            },
        }

        // OPEN(HS1, HS2): Die ausgewählte Session kommt vom Aufrufer, nicht aus der Script-Datei.
        // OPEN(HS3, HS4): Einreichungs- und Wartefolge gegen Activity-Stream und Shutdown prüfen.
        // Ein ausdrücklicher Shutdown wird vom Server zu Session::shutdown geroutet; das Script
        // besitzt die Session nicht.
        pub fn run(
            client: &mut crate::client::Client,
            script: &Script,
        ) -> Result<Outcome, Error>;

        // OPEN(HS2, HS3): Client- und Cursorfehler ergänzen, ohne Session-Interna offenzulegen.
        pub enum Error {
            InvalidScript {
                command_index: Option<usize>,
                message: String,
            },
            Session {
                command_index: usize,
                error: crate::session::Error,
            },
        }
    }
}
