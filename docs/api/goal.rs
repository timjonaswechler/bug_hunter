mod handle {
    pub struct Handle {
        pub index: u32,
        pub generation: u32,
    }
}

mod command {
    pub trait Request: Into<Command> {
        type Output;
    }

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
                    surface: Option<Handle>,
                    motion: [f32; 2],
                },
                MoveBy {
                    surface: Option<Handle>,
                    motion: [f32; 2],
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
                target: Option<Handle>,

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
        pub enum Command {
            Start { path: String },
            Stop,
        }

        pub enum Output {
            Finished(Completion),

            Stopped { was_running: bool },
        }

        pub struct Completion {
            pub recorded_commands: u64,
            pub attempted_commands: u64,
            pub outcome: Outcome,
            // OPEN: Festlegen, wie Replay-Abweichungen nach Planung des Reporting-Interfaces
            // zugänglich gemacht werden.
        }

        pub enum Outcome {
            Completed,

            Stopped,

            Blocked,
        }

        impl Request for Command {
            type Output = Output;
        }
    }
}

mod report {
    pub struct Report {
        pub title: String,
        pub failure: Failure,
        pub signature: Signature,
        pub context: Context,
    }

    pub struct Failure {
        pub origin: Origin,
        pub message: String,
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
        },
    }

    pub struct Signature {
        pub value: String,
    }

    pub struct Location {
        pub file: String,
        pub line: u32,
        pub column: Option<u32>,
    }

    pub struct Context {
        // OPEN: Die für Diagnose und Reproduktion notwendigen Anwendungs- und
        // Session-Angaben festlegen. Dieser Typ gehört ausschließlich zum Report und wird
        // bei dessen Erzeugung aus der laufenden Session zusammengestellt.
        pub commands: Vec<crate::session::history::Entry>,
    }

    pub struct Config {
        pub tracing_errors: bool,
        pub output: String,
        pub provider: provider::Config,
    }

    impl Report {
        pub fn create(failure: Failure, session: &crate::session::Session) -> Self;
    }

    pub fn submit(
        report: &Report,
        config: &Config,
        session_artifact_dir: &str,
    ) -> Result<provider::Outcome, Error>;

    pub enum Error {
        // OPEN: Die genaue Liste der Provider- und Persistenzfehler mit der Implementation planen.
    }

    pub mod provider {
        pub enum Config {
            Local(local::Config),
            Github(github::Config),
        }

        pub enum Reference {
            File { path: String },
            Issue { identifier: String, url: String },
        }

        pub enum Outcome {
            Created { reference: Reference },
            Existing { reference: Reference },
        }

        pub mod local {
            pub struct Config {}
        }

        pub mod github {
            pub struct Config {
                // OPEN: Notwendige Repository- und Veröffentlichungseinstellungen festlegen.
            }
        }
    }

    // OPEN: Normalisierung und persistierte Darstellung der Fehlersignatur festlegen. Flüchtige
    // Speicheradressen und absolute Projektpräfixe dürfen dieselbe Assertion zwischen Ausführungen
    // nicht zu unterschiedlichen Signaturen machen.
}

mod session {
    pub struct Plugin {}

    impl Default for Plugin {}
    impl bevy::app::Plugin for Plugin {}

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
    pub struct RequestId(u64);

    pub mod launch {
        pub struct Config {
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

    pub struct Pending<Output> {}

    impl<Output> Pending<Output> {
        pub fn request_id(&self) -> &RequestId;
        pub fn command(&self) -> &crate::command::Command;
    }

    pub enum Event {
        ProtocolError { code: String, message: String },
        Ended,
    }

    pub struct Session {}

    impl Session {
        pub fn start(config: Config) -> Result<Self, Error>;

        pub fn capabilities(&self) -> &Capabilities;

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

        Rejected { code: String, message: String },
        Protocol { code: String, message: String },

        Ended,
    }
}

#[cfg(feature = "host")]
pub mod host {
    // Einziger öffentlicher Host-Einstieg. Die Implementation liest intern eine versionierte
    // TOML-Konfiguration. Config-, CLI- und Controller-Typen bleiben privat.
    pub fn run(config_source: &str) -> std::process::ExitCode;

    mod repl {
        pub enum Exit {
            Quit,
            InputClosed,
            Interrupted,
        }

        pub fn run(session: &mut crate::session::Session) -> Result<Exit, Error>;

        pub enum Error {
            Terminal { message: String },
            Session(crate::session::Error),
            ActiveRecordingOnInputClose,
        }
    }

    mod agent {
        pub enum Exit {
            InputClosed,
        }

        // Liest dieselben JSON-Command-Einträge, die auch ein Script enthält. Der Agent liefert
        // keine Request-ID. Session vergibt sie und die JSONL-Ausgabe meldet für jeden gültigen
        // Eintrag zuerst pending und später genau ein terminales Outcome. Jede Meldung enthält
        // Request-ID und qualifizierten Command-Namen.
        pub fn run(
            session: &mut crate::session::Session,
            input: impl std::io::BufRead,
            output: impl std::io::Write,
        ) -> Result<Exit, Error>;

        pub enum Error {
            Input { message: String },
            Output { message: String },
            Session(crate::session::Error),
        }

        // OPEN: Kontrollierten Abschluss, EOF bei noch laufenden Commands und Session-Events
        // zusammen mit dem übrigen Agent-Interface festlegen.
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

        // Reicht alle Commands in Dateireihenfolge an Session weiter, ohne auf terminale Outcomes
        // zu warten. Danach werden alle noch ausstehenden Outcomes ungeordnet eingesammelt.
        // Session-Start, Shutdown, Dateizugriff, Darstellung und Exit-Code bleiben beim Host.
        pub fn run(
            session: &mut crate::session::Session,
            script: &Script,
        ) -> Result<Outcome, Error>;

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
