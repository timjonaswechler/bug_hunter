// Rust-ähnliche Interface-Skizze, kein kompilierbarer Quelltext.
// Verhalten: target.md. Umsetzung: implementation-plan.md.
// Fehlende Körper und ausgelassene Enum-Varianten sind Skizzen, keine leeren Implementierungen.
// Ziel ist fensterlose Bildausgabe mit virtuellen Eingaben. Die unten vorhandenen
// Command-Formen bleiben Migrationsbasis, nicht Nachweis einer Headless-Implementation.
// Offen: Bildziel-Konfiguration/Metadaten, Renderbereitschaft, relative Blickbewegung
// und die versionierte Ablösung der bisherigen *_window_unavailable-Fehler.

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

    // Nur die von woodpecker definierten konkreten Command-Typen implementieren dieses Trait.
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
            // MoveTo/MoveBy beziehen sich im Ziel auf den logischen Bildzielraum.
            // MoveBy bleibt begrenzte Pointerverschiebung. Relative Blickbewegung
            // ohne Cursorgrenzen braucht noch eine ausdrücklich festgelegte Command-Form.
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
        // Nimmt das primäre Bildziel auf, nicht den Inhalt eines OS-Fensters.
        // Die sichere Dateiablage und Request-Korrelation bleiben erhalten.
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
        pub woodpecker_version: String,
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

    // Synchroner, nicht klonbarer Handle. Der private Koordinator arbeitet ohne Empfangsaufrufe weiter.
    pub struct Session {}

    impl Session {
        pub fn start(config: Config) -> Result<Self, Error>;

        pub fn capabilities(&self) -> &Capabilities;

        // Typgelöste Ausführung für den Server bleibt intern bei session.
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

// Modulverantwortungen; Paketierung und genaue äußere Signaturen folgen dem Umsetzungsplan.
mod server {
    // Verzeichnis, Session-Handles, Activity und clientunabhängige Report-Orchestrierung.
    // Verwaltung: Erstellen, Liste, Detail und Serverende. Erstellen bestätigt nur die Annahme.
    // Der Server ergänzt den Artefaktpfad vor dem internen Session-Start.
    struct CreateSession {
        launch: crate::session::launch::Config,
        tick: crate::command::tick::Config,
        report: crate::report::Config,
    }

    // Serverseitig vergebene Identität, getrennt von RequestId und Activity-Cursor.
    struct SessionId {}

    // Lebenszyklus des Verzeichniseintrags, nicht Recording-/Replay-Zustand.
    enum SessionState {
        Starting,
        Ready,
        Stopping,
        Ended,
        Failed,
    }
}

mod client {
    // HTTP-Verwaltung ohne Bindung; eigener WebSocket-Zugang fest an eine Session gebunden.
    // Verbindungszugriff und Antwortzuordnung, ohne Spielregeln oder Terminaldarstellung.
    // Client ist hier der gebundene Zugang für REPL und Script.
    // Seine Methodensignaturen folgen dem Activity-Vertrag in target.md.
    pub struct Client {}

    // Fehlerform wird mit dem äußeren Codec konkretisiert, nicht aus Session-Interna kopiert.
    pub enum Error {}
}

mod cli {
    // Argumente, Config-Dateizugriff, Terminaldarstellung und Ablaufwahl.

    mod repl {
        // Menschliche Eingaben werden gemeinsame Commands; Ergebnisse kommen über client.
        pub enum Exit {
            Quit,
            InputClosed,
            Interrupted,
        }

        pub fn run(client: &mut crate::client::Client) -> Result<Exit, Error>;

        pub enum Error {
            Terminal { message: String },
            Client(crate::client::Error),
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

        // Verwendet die bestehende Client-Bindung; Abschlussbarrieren siehe target.md.
        pub fn run(
            client: &mut crate::client::Client,
            script: &Script,
        ) -> Result<Outcome, Error>;

        pub enum Error {
            InvalidScript {
                command_index: Option<usize>,
                message: String,
            },
            Client {
                command_index: usize,
                error: crate::client::Error,
            },
        }
    }
}
