# bug_hunter — API-Ist-Stand und Verantwortlichkeiten

> Einstieg: [`Dokumentindex`](README.md), [`Zielskizze`](goal.rs),
> [`Migrationsplan`](migration.md) und [`Entscheidungsindex`](decisions.md).
>
> Dieses Dokument bildet den aktuellen Arbeitsbaum von `crates/bug_hunter` ab — einschließlich der
> noch nicht abgeschlossenen, großen Umstellung. Es ist kein Refactor-Plan und behauptet nicht, dass
> eine geplante Änderung bereits umgesetzt ist.
>
> Quelle: `src/**/*.rs` im Arbeitsbaum. Sichtbarkeiten und private Symbole werden dokumentiert,
> soweit sie fachlich oder für Ownership/Abhängigkeiten relevant sind. Testmodule und reine
> Test-Hilfsfunktionen sind nicht Teil der Produktions-API; ihre Seiteneffekte werden bei Bedarf
> in den Modulnotizen erwähnt. Type Aliases werden bewusst nur als Legacy-/Duplikat-Hinweis
> aufgeführt, nicht als eigener fachlicher Baustein.

## Arbeitsregeln

- **Ist-Stand zuerst:** Die folgenden Einträge beschreiben den Code, wie er jetzt vorliegt. Die
  uncommitted Änderungen gehören ausdrücklich zu diesem Ist-Stand.
- **Keine Zielplanung:** Zielnamen, Verschiebungen und Löschkandidaten gehören in
  [`goal.rs`](goal.rs), [`migration.md`](migration.md) oder eine ADR, nicht in dieses Dokument.
- **Ein Owner:** Ein fachlicher Datensatz, eine Konfiguration und eine Verantwortung werden nur
  einmal besessen. Andere Module verwenden Referenzen oder den gemeinsamen Typ.
- **Sichtbarkeit:** `pub` ist crate-öffentlich, `pub(crate)` intern im Crate, private Einträge
  sind nur im jeweiligen Modul sichtbar. Re-Exports werden zusätzlich als API-Kante vermerkt.
- **Wire / Guest / Host:** `Wire` bezeichnet serialisierte Protokoll- oder Recording-Daten,
  `Guest` den Controlled-Session-Prozess mit Bevy World, `Host` die `feature = "host"`-Seite.
- Mehrere Nutzer derselben Information erzeugen eine gemeinsame Seam, wenn sie dieselbe Invariante oder dasselbe Format benötigen. Einmalige Konverter, Adapter und fachliche Sonderlogik bleiben am engsten zugehörigen Modul. Gemeinsame Kernlogik wird zentralisiert; domänenspezifische Fehler und Seiteneffekte bleiben lokal.

## Modulaufgaben und Owner (Ist-Stand)

| Modul | Zentrale Aufgabe | Besitzt | Verwendet / Seiteneffekte |
| --- | --- | --- | --- |
| `lib.rs` | Öffentliche Crate-Fassade und Kompatibilitäts-Re-Exports | Top-Level-Namen, Protokollversion, Artifact-Root-Umgebungsvariable | verbindet fast alle Domänen; liest die Umgebung bei `artifact_root_path` |
| `file_ref` | Dateireferenz-Modul im aktuellen Arbeitsbaum | `Reference`, Pfad-Validierung und `Set` | kein eigenes I/O; Teile sind DTOs und JSON-Helfer |
| `client` | Bevy-Plugin, das Transport, Request-Verarbeitung und kontrollierte Frames verbindet | `AutomationControlPlugin`, Transport-Abstraktionen sowie Guest-internen Zustand | Bevy World/Schedules; liest stdin und schreibt JSONL über `Output` |
| `command` | Gemeinsames Modell und Validieren von Guest-Steuerbefehlen | `Command`, Request-Envelope, Input-Zustände und kontrollierte Zeit | `observe`, `screenshot`, Bevy; serialisiert/deserialisiert Wire-Formen |
| `command::input` | Virtuelle Pointer-, Keyboard- und Texteingabe samt Beobachtungszustand | jeweilige Commands, States und fachliche Fehler | Bevy World; erzeugt Bevy Input-/IME-Ereignisse, verändert Guest-Zustand |
| `failure` | Persistierbares Format für einen einzelnen Fehlerbericht | `Detail`, `Report`, Fehlerart und Version | JSON-Dateien und Dateisystem über explizite Pfade |
| `handle` | Stabiler, serialisierbarer Verweis auf Bevy-Entities | Index/Generation und Auflösungsfehler | Bevy `World`; prüft Lebensdauer und Generation, besitzt keine Entity |
| `host` | Host-seitige CLI, Controller-Orchestrierung, Diagnose, Replay und Reports | Host-Konfiguration und CLI-/Workflow-Zustände; re-exportiert mehrere Session-Typen | `feature = "host"`, Prozesse, stdin/stdout, Dateien und externe GitHub-CLI |
| `observe` | Selektieren und serialisieren von Guest-Zustand | Beobachtungs-Request, Selector, Projection und Grenzen | Bevy World, Reflection und `serde_json`; liest, verändert den World nicht |
| `protocol` | Stabiler JSONL-Vertrag zwischen Host und Controlled Session | Wire-Request/Command, Ready, Response und Fehler | delegiert Validierung an Domänen; serialisiert/deserialisiert JSONL |
| `report` | Einlesen und Zusammenführen der Lauf-Artefakte für Provider | kanonisches `Report`-Modell und enthaltene Sessions/Fehler | Dateisystem, Recording-/Replay-JSON; erzeugt selbst keine Diagnose |
| `screenshot` | Validieren, Auslösen und Persistieren von PNG-Screenshots | Screenshot-Command, Capture-Fehler und Guest-Capture-Service | Bevy RenderApp, cap-std und Dateisystem unter einem Root |
| `session` | Laufzeit eines Controlled-Session-Prozesses sowie Recording/Replay | Context, Launch-Spec, Driver, Controller, Diagnostics, Recording und Replay | startet/stoppt Kindprozesse, JSONL-Kommunikation, Dateien und Bevy |
| `target.rs` | Markierung von Entities als beobachtbare Automation Targets | `AutomationTarget`-Component | Bevy Reflection; wird von `observe::world` gelesen |

## `file_ref/mod.rs`

- `pub mod path`
- `pub mod set`
- `pub use set::Set`
- `pub struct Reference`
  - Felder:
    - `pub kind: String,`
    - `pub path: String,`
    - `pub mime_type: String,`
    - `pub width: Option<u32>,`
    - `pub height: Option<u32>,`
- `pub fn from_result(result: &serde_json::Value) -> Option<Self>`

- `pub fn matches_value(&self, value: &serde_json::Value) -> bool`

## `file_ref/path.rs`

- `pub enum Error`
  - Varianten:
    - `Empty`
    - `Absolute`
    - `Backslash`
    - `EmptyComponent`
    - `DotComponent`
    - `NotUtf8`
- `pub fn validate(path: &Path) -> Result<PathBuf, Error>`
- `pub fn validate_with_extension(path: &Path, expected_ext: &str) -> Result<PathBuf, String>`

## `file_ref/set.rs`

- `pub struct Set`
  - Felder:
    - `pub recent_log: PathBuf,`
    - `pub failure_report: PathBuf,`

- `pub fn new(recent_log: PathBuf, failure_report: PathBuf) -> Self`


## `client/mod.rs`

- `mod plugin`
- `pub mod transport`
- `pub use plugin::{AutomationControlPlugin, InputFactory}`
- `pub use transport::{Input, JsonLinesInput, Output, StdoutOutput}`

## `client/plugin.rs`

- `pub struct AutomationControlPlugin`
  - Felder: `output: Arc<dyn Output>,; input_factory: Arc<dyn Fn() -> JsonLinesInput + Send + Sync>,`
- `pub trait InputFactory`
- `pub fn rendered_stdio() -> Self`
- `pub fn logical_stdio() -> Self`
- `pub fn with_io(input: impl InputFactory, output: Arc<dyn Output>) -> Self`
- `pub(super) struct Input;`
- `pub(super) struct Frames;`
- `pub(super) struct Output;`

## `client/transport.rs`

- `pub enum Input`
  - Varianten: `Line(String) | Eof | Error(String)`
- `pub struct JsonLinesInput`
  - Felder: `receiver: Mutex<Receiver<Input>>,`
- `pub fn stdin(capacity: usize) -> Self`
- `pub fn from_receiver(receiver: Receiver<Input>) -> Self`
- `pub fn try_recv(&self) -> Result<Input, TryRecvError>`
- `pub trait Output`
- `pub struct StdoutOutput;`

## `command/input/bundle.rs`

- `pub struct Bundle`
  - Felder:
    - `pub pointer: pointer::State,`
    - `pub keyboard: keyboard::State,`
    - `pub text: text::State,`
    - `pub clock: Clock,`
- `pub fn observation(&self, world: &World) -> serde_json::Value`

## `command/input/keyboard.rs`

- `pub enum Key`
  - Varianten:
    - `$( | $variant | )+`
    - `Unknown(String)`
- `pub fn as_str(&self) -> &str`
- `pub enum Command`
  - Varianten:
    - `Press { |key: Key| }`
    - `Release { |key: Key| }`
- `pub fn validate(&self) -> Result<(), Error>`
- `pub struct State`
  - Felder:
    - `pressed: BTreeSet<Key>,`
- `pub fn is_pressed(&self, key: &Key) -> bool`
- `pub fn observation(&self) -> serde_json::Value`
- `pub enum Error`
  - Varianten:
    - `InvalidKey(String)`
    - `NoPrimaryWindow`
    - `AmbiguousPrimaryWindow`
    - `KeyAlreadyPressed(Key)`
    - `KeyNotPressed(Key)`
- `pub fn code(&self) -> &'static str`

## `command/input/mod.rs`

- `pub mod bundle`
- `pub mod keyboard`
- `pub mod pointer`
- `pub mod text`

## `command/input/pointer.rs`

- `pub(crate) struct Helper;`
- `pub enum Button`
  - Varianten:
    - `Primary`
    - `Secondary`
    - `Middle`
- `pub type PointerButton = Button`
- `pub type PointerCommand = Command`
- `pub type PointerState = State`
- `pub enum Command`
  - Varianten:
    - `Move { surface: Option<Handle>, position: [f32; 2] }`
    - `Press { button: Button }`
    - `Release { button: Button }`
    - `Scroll { delta: [f32; 2] }`
- `pub fn validate(&self) -> Result<(), PointerError>`
- `pub struct State`
  - Felder:
    - `pub position: Option<[f32; 2]>`
    - `pub surface: Option<Handle>`
    - `pub pressed: BTreeSet<Button>`
    - `pub scroll_delta: [f32; 2]`
- `pub fn is_pressed(&self, button: Button) -> bool`
- `pub fn observation(&self) -> serde_json::Value`
- `pub enum PointerError`
  - Varianten:
    - `NonFinitePosition`
    - `NoPrimarySurface`
    - `AmbiguousPrimarySurface`
    - `SurfaceNotLive(Handle)`
    - `SurfaceNotWindow(Handle)`
    - `NoLocation`
    - `ButtonAlreadyPressed(Button)`
    - `ButtonNotPressed(Button)`
- `pub struct Surface`
  - Felder:
    - `pub handle: Handle`
    - `pub target: NormalizedRenderTarget`
- `pub fn resolve_surface(world: &World, requested: Option<Handle>) -> Result<Surface, PointerError>`
- `pub fn ensure_mouse_pointer(mut commands: Commands, pointers: Query<&PointerId>)`
- `pub(crate) fn spawn_helper(mut commands: Commands)`

## `command/input/text.rs`

- `pub const MAX_BYTES: usize`
- `pub struct Command`
  - Felder:
    - `pub text: String`
- `pub fn new(text: impl Into<String>) -> Self`
- `pub fn validate(&self) -> Result<(), Error>`
- `pub struct State`
  - Felder:
    - `last_target: Option<Handle>`
    - `last_text: Option<String>`
    - `commits: u64`
- `pub fn observation(&self, world: &World) -> serde_json::Value`
- `pub enum Error`
  - Varianten:
    - `TooLarge(usize)`
    - `NoPrimaryWindow`
    - `AmbiguousPrimaryWindow`
    - `NoFocusState`
    - `NoFocusedEntity`
    - `FocusNotLive(Handle)`
    - `FocusNotEditable(Handle)`
- `pub fn code(&self) -> &'static str`
- `pub fn text_event(state: &mut State, world: &World, command: &Command) -> Result<Ime, Error>`

## `command/mod.rs`

- `pub mod input`
- `pub mod tick`
- `pub const PROTOCOL_VERSION: u32`
- `pub enum Command`
  - Varianten:
    - `Observe(observe::Request)`
    - `Pointer(pointer::Command)`
    - `Keyboard(keyboard::Command)`
    - `Text(text::Command)`
    - `Time(tick::Command)`
    - `Screenshot(screenshot::Command)`
    - `Shutdown`
- `pub fn validate(&self) -> Result<(), String>`
- `pub struct Request`
  - Felder:
    - `pub sequence: u64`
    - `pub command: Command`
- `pub fn new(sequence: u64, command: Command) -> Self`
- `pub fn validate(&self) -> Result<(), String>`
- `pub fn decode_request(line: &str) -> Result<Request, String>`

## `command/tick.rs`

- `pub const MAX_FRAMES: u64`
- `pub const MAX_STEP_NANOSECONDS: u64`
- `pub enum Command`
  - Varianten:
    - `Step { |frames: u64| step_nanoseconds: u64 | }`
- `pub fn step(frames: u64, step_nanoseconds: u64) -> Self`
- `pub fn validate(&self) -> Result<(), Error>`
- `pub(crate) fn into_step(self) -> Step`
- `pub(crate) struct Step`
  - Felder:
    - `pub frames: u64`
    - `pub step: Duration`
- `pub enum Error`
  - Varianten:
    - `InvalidFrames`
    - `TooManyFrames(u64)`
    - `InvalidStep`
    - `StepTooLarge(u64)`
- `pub fn code(self) -> &'static str`
- `pub struct Clock`
  - Felder:
    - `frame_index: u64`
    - `elapsed: Duration`
    - `last_step_nanoseconds: Option<u64>`
- `pub fn frame_index(&self) -> u64`
- `pub fn elapsed(&self) -> Duration`
- `pub fn last_step_nanoseconds(&self) -> Option<u64>`
- `pub(crate) fn complete_frame(&mut self, step: Duration)`
- `pub fn observation(&self) -> Value`

## `failure/kind.rs`

- `pub enum Kind`
  - Varianten:
    - `Panic`
    - `Error`
    - `CliError`
    - `Other`
- `pub fn as_str(&self) -> &str`

## `failure/mod.rs`

- `pub mod kind`
- `pub struct Detail`
  - Felder:
    - `pub kind: String`
    - `pub message: String`
- `pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self`
- `pub struct Report`
  - Felder:
    - `pub version: u32`
    - `pub detail: Detail`
    - `pub cli_error: Option<String>`
    - `pub record_path: Option<PathBuf>`
- `pub const VERSION: u32`
- `pub fn new(detail: Detail, cli_error: Option<String>, record_path: Option<PathBuf>) -> Self`
- `pub fn validate(&self) -> Result<(), String>`
- `pub fn write_to(&self, path: impl AsRef<Path>) -> Result<(), String>`
- `pub fn load(path: impl AsRef<Path>) -> Result<Self, String>`

## `handle/mod.rs`

- `pub struct Handle`
  - Felder:
    - `pub index: u32`
    - `pub generation: u32`
- `pub fn new(index: u32, generation: u32) -> Self`
- `pub fn from_entity(entity: Entity) -> Self`
- `pub fn entity(self) -> Option<Entity>`
- `pub fn resolve(self, world: &World) -> Result<Entity, HandleError>`
- `pub enum HandleError`
  - Varianten:
    - `InvalidBits(Handle)`
    - `NotLive(Handle)`

## `host/command_line.rs`

- `pub enum CommandLine`
  - Varianten:
    - `Run(RunOptions)`
    - `Report(ReportOptions)`
- `pub struct RunOptions`
  - Felder:
    - `pub config_path: Option<PathBuf>`
    - `pub scenario: String`
    - `pub artifact_dir: Option<PathBuf>`
    - `pub record: Option<PathBuf>`
- `pub struct ReportOptions`
  - Felder:
    - `pub config_path: Option<PathBuf>`
    - `pub artifact_dir: PathBuf`
    - `pub create: bool`
- `pub const USAGE: &str`
- `pub struct CommandLineError(String);`

## `host/config.rs`

- `pub const CONFIG_VERSION: u32`
- `pub struct Config`
  - Felder:
    - `pub version: u32`
    - `pub profile_id: String`
    - `pub tool: ToolConfig`
    - `pub application: LaunchSpec`
    - `pub session: SessionConfig`
    - `pub report: ReportConfig`
- `pub struct ToolConfig`
  - Felder:
    - `pub name: String`
    - `pub about: String`
    - `pub default_artifact_dir: PathBuf`
- `pub type ApplicationConfig = LaunchSpec`
- `pub struct SessionConfig`
  - Felder:
    - `pub id: String`
    - `pub surface_width: u32`
    - `pub surface_height: u32`
    - `pub frame_nanoseconds: u64`
    - `pub startup_frames: u64`
- `pub struct ReportConfig`
  - Felder:
    - `pub generated_by: Option<String>`
- `pub struct ScreenConfig`
  - Felder:
    - `pub target: String`
    - `pub component: String`
    - `pub value_pointer: String`
    - `pub result_field: String`
- `pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError>`
- `pub fn parse(contents: &str) -> Result<Self, ConfigError>`
- `pub fn validate(&self) -> Result<(), String>`
- `pub enum ConfigError`
  - Varianten:
    - `Read { | path: PathBuf | error: std::io::Error | }`
    - `Parse { |path: PathBuf | error: String | }`
    - `Invalid(String)`

## `host/mod.rs`

- `pub mod github`
- `pub type ReportError = String`
- `pub mod report`

## `host/repl.rs`

- `pub(crate) const HELP: &str`
- `pub(crate) enum Command`
  - Varianten:
    - `Click(String)`
    - `Action(Action)`
    - `Observe(Observation)`
    - `Pause`
    - `Resume`
    - `Step(u64)`
    - `Recording(RecordingCommand)`
    - `Status`
    - `Help`
    - `Quit`
- `pub(crate) enum RecordingCommand`
  - Varianten:
    - `Start(Option<PathBuf>)`
    - `Stop`
- `pub(crate) fn from_line(line: &str) -> Result<Self, String>`
- `pub(crate) fn write_status(output: &mut impl Write, status: &Status) -> io::Result<()>`

## `host/runner.rs`

- `pub fn run_embedded(profile_source: &'static str) -> !`

## `host/script.rs`

- `pub(crate) const INVALID_SCRIPT_EXIT: i32`
- `pub(crate) const ACTION_EXIT: i32`
- `pub(crate) const EXPECTATION_EXIT: i32`
- `pub(crate) struct Error`
  - Felder:
    - `kind: ErrorKind`
    - `script: PathBuf`
    - `step: Option<usize>`
    - `message: String`
    - `expected: Option<Value>`
    - `actual: Option<Value>`
    - `last_observation: Option<(String, Value)>`
- `pub(crate) fn exit_code(&self) -> i32`
- `pub(crate) struct Summary`
  - Felder:
    - `pub(crate) completed: usize`
    - `pub(crate) skipped: usize`

## `lib.rs`

- `pub mod artifact`
- `pub mod client`
- `pub mod command`
- `pub mod failure`
- `pub mod handle`
- `pub mod host`
- `pub mod observe`
- `pub mod protocol`
- `pub mod report`
- `pub mod screenshot`
- `pub mod session`
- `pub mod target`
- `pub const bug_hunter_ARTIFACT_DIR: &str`
- `pub fn artifact_root_path(default: impl Into<std::path::PathBuf>) -> std::path::PathBuf`

## `observe/mod.rs`

- `pub mod world`
- `pub const DEFAULT_LIMIT: u32`
- `pub const MAX_LIMIT: u32`
- `pub const MAX_HIERARCHY_DEPTH: u8`
- `pub const MAX_COMPONENT_BYTES: usize`
- `pub struct Request`
  - Felder:
    - `pub selector: Selector`
    - `pub projection: Projection`
    - `pub limit: u32`
    - `pub cursor: Option<u32>`
- `pub fn new(selector: Selector, projection: Projection) -> Self`
- `pub fn validate(&self) -> Result<(), Error>`
- `pub enum Selector`
  - Varianten:
    - `Targets`
    - `Ui`
    - `Pointers`
    - `Entity(Handle)`
    - `VirtualInput`
    - `Clock`
- `pub enum Projection`
  - Varianten:
    - `Summary`
    - `ComponentNames`
    - `Components { type_paths: Vec<String> }`
    - `Hierarchy { depth: u8 }`
- `pub enum Error`
  - Varianten:
    - `InvalidLimit(u32)`
    - `InvalidCursor`
    - `InvalidDepth(u8)`
    - `InvalidComponentPath(String)`
    - `UnsupportedVirtualInputProjection`
    - `UnsupportedClockProjection`
    - `UnknownEntity(crate::handle::Handle)`

## `observe/world/hierarchy.rs`

- `pub(crate) fn hierarchy(world: &World, entity: Entity, depth: u8) -> Result<Value, Error>`

## `observe/world/mod.rs`

- `pub mod hierarchy`
- `pub mod projection`
- `pub fn observe_world(world: &World, request: &Request) -> Result<Value, Error>`

## `observe/world/projection.rs`

- `pub(crate) fn component_names(world: &World, entity: Entity) -> Vec<String>`

## `protocol.rs`

- `pub const PROTOCOL_VERSION: u32`
- `pub struct Request`
  - Felder:
    - `pub sequence: u64`
    - `pub command: Command`
- `pub enum Command`
  - Varianten:
    - `Observe(ObservationRequest)`
    - `Pointer(PointerCommand)`
    - `Keyboard(KeyboardCommand)`
    - `Text(TextCommand)`
    - `Time(TimeCommand)`
    - `Screenshot(ScreenshotCommand)`
    - `Shutdown`
- `pub struct Ready`
  - Felder:
    - `pub kind: String`
    - `pub version: u32`
    - `pub controls: Vec<String>`
    - `pub observation_scopes: Vec<String>`
- `pub fn new() -> Self`
- `pub(crate) fn with_screenshot(mut self) -> Self`
- `pub struct Response`
  - Felder:
    - `pub sequence: u64`
    - `pub status: ResponseStatus`
    - `pub result: Option<Value>`
    - `pub error: Option<ProtocolError>`
- `pub enum ResponseStatus`
  - Varianten:
    - `Completed`
    - `Error`
- `pub struct ProtocolError`
  - Felder:
    - `pub code: String`
    - `pub message: String`
- `pub fn completed(sequence: u64, result: Value) -> Self`
- `pub fn error(sequence: u64, code: impl Into<String>, message: impl Into<String>) -> Self`
- `pub enum DecodeError`
  - Varianten:
    - `Malformed(String)`
    - `UnsupportedVersion(u32)`
    - `InvalidSequence`
    - `InvalidArguments(String)`
- `pub fn decode_request(line: &str) -> Result<Request, Response>`

## `report/mod.rs`

- `pub mod provider`
- `pub struct Report`
  - Felder:
    - `pub artifact_dir: PathBuf`
    - `pub sessions: Vec<Session>`
    - `pub replay: Option<Value>`
    - `pub failure: Option<Failure>`
    - `pub recent_log: String`
    - `pub session_tail: String`
- `pub struct Session`
  - Felder:
    - `pub recording: PathBuf`
    - `pub configuration: Value`
    - `pub controllers: Vec<String>`
    - `pub result: &'static str`
    - `pub errors: Vec<RecordedError>`
    - `pub artifacts: Vec<Value>`
- `pub struct RecordedError`
  - Felder:
    - `pub kind: String`
    - `pub message: String`
- `pub struct Failure`
  - Felder:
    - `pub kind: String`
    - `pub message: String`
    - `pub cli_error: Option<String>`
- `pub fn load(artifact_dir: impl AsRef<Path>) -> Result<Self, String>`
- `pub fn json(&self) -> Result<String, String>`

## `report/provider/console.rs`

- `pub fn to_console(report: &Report) -> String`

## `report/provider/github.rs`

- `pub struct Draft`
  - Felder:
    - `pub title: String`
    - `pub body: String`
- `pub fn from_report(report: &crate::report::Report, footer: Option<&str>) -> Self`
- `pub fn write_to(&self, artifact_dir: impl AsRef<Path>) -> Result<PathBuf, io::Error>`
- `pub struct Publisher`
  - Felder:
    - `title: String`
    - `draft_path: PathBuf`
- `pub fn draft(self) -> Outcome`
- `pub fn publish(self) -> Result<Outcome, Error>`
- `pub type Report = Publisher`
- `pub enum Outcome`
  - Varianten:
    - `Drafted { title: String, path: PathBuf }`
    - `Existing { title: String, issue: Issue }`
    - `Created { title: String, url: String }`
- `pub struct Issue`
  - Felder:
    - `number: u64`
    - `title: String`
    - `url: String`
- `pub enum Error`
  - Varianten:
    - `Io { operation: &'static str, path: PathBuf, error: io::Error }`
    - `Start { operation: &'static str, error: io::Error }`
    - `Failed { operation: &'static str, stderr: String }`
    - `InvalidResponse(serde_json::Error)`

## `report/provider/json.rs`

- `pub fn to_json(report: &Report) -> Result<String, String>`

## `report/provider/mod.rs`

- `pub mod console`
- `pub mod github`
- `pub mod json`

## `screenshot/capture.rs`

- `pub(crate) fn open_artifact_root(root: &Path) -> Result<Dir, Error>`
- `pub(crate) fn prepare_target(root: &Path, relative: &Path) -> Result<PathBuf, Error>`
- `pub(crate) fn reject_symlink(path: &Path) -> Result<(), Error>`
- `pub(crate) fn ensure_below_root(root: &Path, path: &Path) -> Result<(), Error>`
- `pub(crate) fn write_png(directory: &Dir, relative: &Path, image: Image) -> Result<(), Error>`
- `pub(crate) fn verify_png(directory: &Dir, relative: &Path) -> Result<(), Error>`
- `pub(crate) fn path_to_wire(path: &Path) -> Result<String, Error>`
- `pub(crate) fn finish(service: &super::plugin::Service, relative_path: &Path, image: Image) -> Result<Value, Error>`

## `screenshot/mod.rs`

- `pub mod capture`
- `pub mod plugin`
- `pub const CONTROL_NAME: &str`
- `pub const MIME_TYPE: &str`
- `pub struct Command`
  - Felder:
    - `pub path: String`
- `pub type Request = Command`
- `pub fn new(path: impl Into<String>) -> Self`
- `pub fn reference(&self, width: u32, height: u32) -> crate::artifact::Reference`
- `pub fn validate(&self) -> Result<(), Error>`
- `pub enum Error`
  - Varianten:
    - `CapabilityUnavailable`
    - `EmptyPath`
    - `AbsolutePath`
    - `PathTraversal`
    - `InvalidPath(String)`
    - `SymlinkEscape(PathBuf)`
    - `OutsideArtifactRoot(PathBuf)`
    - `Io(String)`
    - `Capture(String)`
- `pub fn code(&self) -> &'static str`
- `pub(crate) fn validate_relative_path(path: &Path) -> Result<PathBuf, Error>`

## `screenshot/plugin.rs`

- `pub struct Plugin`
  - Felder:
    - `artifact_root: Option<PathBuf>`
- `pub fn with_artifact_root(path: impl Into<PathBuf>) -> Self`
- `pub(crate) struct Service`
  - Felder:
    - `pub(crate) artifact_root: PathBuf`
    - `pub(crate) directory: Dir`
- `pub(crate) struct Capture`
  - Felder:
    - `pub entity: Entity`
- `pub(crate) fn is_available(world: &World) -> bool`
- `pub(crate) fn start(world: &mut World, command: &Command) -> Result<Capture, Error>`

## `session/context.rs`

- `pub struct Context`
  - Felder:
    - `pub session_id: String`
    - `pub protocol_version: u32`
    - `pub configuration: Value`
- `pub fn new(session_id: impl Into<String>, configuration: Value) -> Self`
- `pub fn validate(&self) -> Result<(), String>`

## `session/controller.rs`

- `pub(crate) struct SurfaceSize`
  - Felder:
    - `width: f32`
    - `height: f32`
- `pub(crate) fn new(width: u32, height: u32) -> Self`
- `pub(crate) enum Button`
  - Varianten:
    - `Left`
    - `Right`
    - `Middle`
- `pub(crate) fn from_name(value: &str) -> Result<Self, String>`
- `pub(crate) enum PointerAction`
  - Varianten:
    - `Move { x: f32, y: f32 }`
    - `Press(Button)`
    - `Release(Button)`
    - `Click(Button)`
    - `Scroll { x: f32, y: f32 }`
- `pub(crate) enum KeyboardAction`
  - Varianten:
    - `Press(String)`
    - `Release(String)`
- `pub(crate) enum Action`
  - Varianten:
    - `Pointer(PointerAction)`
    - `Keyboard(KeyboardAction)`
    - `Text(String)`
- `pub(crate) enum Observation`
  - Varianten:
    - `Targets`
    - `Ui`
    - `Pointers`
    - `VirtualInput`
    - `Clock`
- `pub(crate) fn from_name(value: &str) -> Option<Self>`
- `pub(crate) fn as_str(self) -> &'static str`
- `pub(crate) struct Status`
  - Felder:
    - `pub(crate) instance: String`
    - `pub(crate) paused: bool`
    - `pub(crate) last_action: String`
- `pub(crate) enum ControllerError`
  - Varianten:
    - `Launch(String)`
    - `Communication(String)`
    - `Child(String)`
    - `Request { code: String, message: String }`
    - `Invalid(String)`
    - `Shutdown`
- `pub(crate) fn is_fatal(&self) -> bool`
- `pub(crate) struct ControllerSession`
  - Felder:
    - `driver: Option<DriverSession>`
    - `profile: Config`
    - `surface: SurfaceSize`
    - `instance: String`
    - `paused: bool`
    - `last_action: String`
- `pub(crate) fn perform(&mut self, action: Action) -> Result<Value, ControllerError>`
- `pub(crate) fn activate_target(&mut self, target: &str) -> Result<(), ControllerError>`
- `pub(crate) fn observe(&mut self, observation: Observation) -> Result<Value, ControllerError>`
- `pub(crate) fn pause(&mut self) -> Result<(), ControllerError>`
- `pub(crate) fn resume(&mut self) -> Result<(), ControllerError>`
- `pub(crate) fn stop_recording(&mut self) -> Result<PathBuf, ControllerError>`
- `pub(crate) fn capture_invalid_command(&mut self)`
- `pub(crate) fn capture_script_error(&mut self, kind: &str)`
- `pub(crate) fn capture_operation_error(&mut self, error: &ControllerError)`
- `pub(crate) fn step(&mut self, frames: u64) -> Result<(), ControllerError>`
- `pub(crate) fn status(&mut self) -> Result<Status, ControllerError>`
- `pub(crate) fn ensure_running(&mut self) -> Result<(), ControllerError>`
- `pub(crate) fn shutdown(&mut self) -> Result<(), ControllerError>`

## `session/diagnostics.rs`

- `pub const DEFAULT_RECENT_LOG_CAPACITY: usize`
- `pub const FAILURE_REPORT_VERSION: u32`
- `pub struct FailureHeadline`
  - Felder:
    - `pub kind: &'static str`
    - `pub message: String`
- `pub struct FailureReport`
  - Felder:
    - `pub version: u32`
    - `pub kind: String`
    - `pub message: String`
    - `pub cli_error: Option<String>`
    - `pub record_path: Option<PathBuf>`
- `pub fn load(path: impl AsRef<Path>) -> Result<Self, DiagnosticsError>`
- `pub fn validate(&self) -> Result<(), String>`
- `pub fn write_to(&self, path: impl AsRef<Path>) -> Result<(), DiagnosticsError>`
- `pub struct DiagnosticArtifacts`
  - Felder:
    - `pub recent_log: PathBuf`
    - `pub failure_report: PathBuf`
- `pub enum DiagnosticsError`
  - Varianten:
    - `Io { operation: &'static str, path: PathBuf, error: io::Error }`
    - `Json { operation: &'static str, path: PathBuf, error: String }`
    - `Invalid { path: PathBuf, message: String }`
- `pub struct RecentLogs(Arc<Mutex<RecentLogState>>);`
- `pub fn with_capacity(capacity: usize) -> Self`
- `pub fn push(&self, line: String)`
- `pub fn snapshot(&self) -> Vec<String>`
- `pub fn failure(&self) -> Option<FailureHeadline>`

## `session/driver.rs`

- `pub enum DriverError`
  - Varianten:
    - `Launch(String)`
    - `Io(String)`
    - `Protocol(String)`
    - `Child(String)`
    - `RequestFailed(Response)`
- `pub struct SessionOptions`
  - Felder:
    - `pub record: Option<PathBuf>`
    - `pub recent_logs: RecentLogs`
    - `pub artifact_dir: Option<PathBuf>`
    - `pub session_artifact_dir: Option<PathBuf>`
    - `pub recording: recording::Options`
- `pub fn new() -> Self`
- `pub fn with_record(mut self, path: Option<PathBuf>) -> Self`
- `pub fn with_recent_logs(mut self, recent_logs: RecentLogs) -> Self`
- `pub fn with_artifact_dir(mut self, path: impl Into<PathBuf>) -> Self`
- `pub fn with_session_artifact_dir(mut self, path: impl Into<PathBuf>) -> Self`
- `pub fn with_controller(mut self, controller: Controller) -> Self`
- `pub struct Session`
  - Felder:
    - `child: Child`
    - `stdin: Option<ChildStdin>`
    - `receiver: mpsc::Receiver<Result<Value, String>>`
    - `recording: recording::State`
    - `stderr_thread: Option<thread::JoinHandle<()>>`
    - `next_sequence: u64`
    - `clean_shutdown: bool`
- `pub fn spawn(spec: &LaunchSpec, options: SessionOptions) -> Result<Self, DriverError>`
- `pub fn ready(&mut self) -> Result<Ready, DriverError>`
- `pub fn request(&mut self, command: Command) -> Result<Response, DriverError>`
- `pub fn configure_recording(&mut self, configuration: Value) -> Result<(), DriverError>`
- `pub fn start_recording(&mut self, path: Option<PathBuf>) -> Result<PathBuf, DriverError>`
- `pub fn stop_recording(&mut self) -> Result<PathBuf, DriverError>`
- `pub fn active_recording_path(&self) -> Option<&Path>`
- `pub fn capture_controller_action(&mut self, action: Value) -> Result<(), DriverError>`
- `pub fn ensure_running(&mut self) -> Result<(), DriverError>`
- `pub fn shutdown(mut self) -> Result<(), DriverError>`

## `session/launch.rs`

- `pub struct Spec`
  - Felder:
    - `pub package: String`
    - `pub kind: Kind`
    - `pub target: String`
    - `pub features: Vec<String>`
    - `pub arguments: Vec<String>`
- `pub enum Kind`
  - Varianten:
    - `Binary`
    - `Example`
- `pub fn command(&self) -> std::process::Command`

## `session/mod.rs`

- `pub mod context`
- `pub mod controller`
- `pub mod diagnostics`
- `pub mod driver`
- `pub mod launch`
- `pub mod recording`
- `pub mod replay`

## `session/recording/mod.rs`

- `pub mod sanitize`
- `pub mod state`
- `pub mod writer`
- `pub const FORMAT_VERSION: u32`
- `pub struct Entry`
  - Felder:
    - `pub version: u32`
    - `pub sequence: u64`
    - `pub event: Event`
- `pub struct Controller`
  - Felder:
    - `pub origin: String`
- `pub fn new(origin: impl Into<String>) -> Self`
- `pub enum Event`
  - Varianten:
    - `SessionStarted { context: Context }`
    - `ControllerAction { controller: Controller, action: Value }`
    - `GameResponse { request_sequence: u64, status: ResponseStatus, result: Option<Value>, error: Option<failure::Detail> }`
    - `Observation { request_sequence: u64, request: observe::Request, result: Value }`
    - `Error { detail: failure::Detail }`
    - `Artifact { request_sequence: u64, artifact: artifact::Reference }`
    - `RecordingStopped`
    - `SessionEnded { outcome: Outcome }`
- `pub enum Outcome`
  - Varianten:
    - `Completed`
    - `Aborted`
- `pub struct Recording`
  - Felder:
    - `pub entries: Vec<Entry>`
- `pub enum Error`
  - Varianten:
    - `Io(String)`
    - `Json(String)`
    - `Invalid(String)`
    - `UnsupportedVersion(u64)`
- `pub fn parse_path(path: impl AsRef<std::path::Path>) -> Result<Self, Error>`
- `pub fn parse_reader(reader: impl BufRead) -> Result<Self, Error>`
- `pub fn validate(&self) -> Result<(), Error>`

## `session/recording/sanitize.rs`

- `pub const MAX_DEPTH: usize`
- `pub const MAX_COLLECTION_ITEMS: usize`
- `pub const MAX_STRING_BYTES: usize`
- `pub const MAX_ENTRY_BYTES: usize`
- `pub const REDACTED: &str`
- `pub fn sanitize_event(event: Event) -> Event`
- `pub fn compact_event(event: &Event, original_bytes: usize) -> Event`

## `session/recording/state.rs`

- `pub struct Options`
  - Felder:
    - `pub context: SessionContext`
    - `pub context_explicit: bool`
    - `pub controller: Controller`
- `pub struct State`
  - Felder:
    - `pub writer: Option<Writer>`
    - `pub context: SessionContext`
    - `pub context_explicit: bool`
    - `pub context_written: bool`
    - `pub ready: bool`
    - `pub artifact_root: PathBuf`
    - `pub controller: Controller`
    - `pub host_sequence: u64`

## `session/recording/writer.rs`

- `pub enum Error`
  - Varianten:
    - `Io(String)`
    - `AlreadyExists(PathBuf)`
    - `Json(String)`
    - `InvalidPath(String)`
- `pub struct Writer`
  - Felder:
    - `file: cap_std::fs::File`
    - `path: PathBuf`
- `pub fn create(artifact_root: &Path, requested: Option<&Path>) -> Result<Self, Error>`
- `pub fn path(&self) -> &Path`
- `pub fn write(&mut self, entry: &Entry) -> Result<(), Error>`
- `pub fn path_below_artifact_root(artifact_root: impl AsRef<Path>, requested: impl AsRef<Path>) -> Result<PathBuf, Error>`
- `pub fn command_value(command: &crate::Command) -> Result<Value, Error>`

## `session/replay/mod.rs`

- `pub mod runner`
- `pub struct ValidationError`
  - Felder:
    - `pub message: String`
- `pub struct RecordedAction`
  - Felder:
    - `pub sequence: u64`
    - `pub action: &'a Value`
    - `pub expected: &'a [Entry]`
- `pub fn validate(recording: &Recording) -> Result<Vec<RecordedAction<'_>>, ValidationError>`
- `pub fn artifact_matches(expected: &ArtifactReference, actual: &Value) -> bool`
- `pub fn expected_value(entries: &[Entry]) -> Value`
- `pub fn response_value(response: &Response) -> Value`

## `session/replay/runner.rs`

- `pub const REPLAY_EXIT: i32`
- `pub(crate) enum Outcome`
  - Varianten:
    - `Passed`
    - `Failed`
- `pub struct ResultArtifact`
  - Felder:
    - `version: u32`
    - `source: PathBuf`
    - `controller: &'static str`
    - `configuration: Value`
    - `artifact_root: PathBuf`
    - `outcome: Outcome`
    - `actions: usize`
    - `error: Option<String>`
    - `artifacts: Vec<ArtifactReference>`
- `pub struct Error`
  - Felder:
    - `message: String`
    - `sequence: Option<u64>`
    - `action: Option<Value>`
    - `expected: Option<Value>`
    - `actual: Option<Value>`
    - `artifacts: Vec<ArtifactReference>`
- `pub fn invalid(message: impl Into<String>) -> Self`
- `pub fn mismatch(sequence: u64, action: &Value, message: impl Into<String>, expected: Value, actual: Value, artifacts: Vec<ArtifactReference>) -> Self`
- `pub fn exit_code(&self) -> i32`
- `pub struct Summary`
  - Felder:
    - `pub actions: usize`
- `pub trait ReplayDriver`
- `pub fn execute(driver: &mut impl ReplayDriver, actions: &[RecordedAction<'_>]) -> Result<Vec<ArtifactReference>, Error>`
- `pub fn result_artifact(source: &Path, configuration: &Value, artifact_root: PathBuf, actions: usize, outcome_is_passed: bool, error: Option<String>, artifacts: Vec<ArtifactReference>) -> ResultArtifact`
- `pub fn write_result(artifact_dir: &Path, result: &ResultArtifact) -> Result<(), String>`
- `pub fn persist_early_failure(source: &Path, artifact_dir: &Path, session_artifact_dir: &Path, configuration: &Value, actions: usize, error: Error) -> Error`
- `pub fn finalize_and_write(source: &Path, configuration: &Value, session_artifact_dir: PathBuf, actions_len: usize, result: &Result<Vec<ArtifactReference>, Error>, artifact_dir: &Path) -> Result<Summary, Error>`

## `target.rs`

- `pub struct AutomationTarget;`
