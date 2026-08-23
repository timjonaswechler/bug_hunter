# `bug_hunter`

`bug_hunter` is inspired by ThePrimeagen's video about he's become a game developer ([Youtube](https://www.youtube.com/watch?v=tYQyh1tjSFc)).
He had build a system that lets AI Agents control a game.

I thought why only AI Agents, why not humans and scripts as well?

This repo is a try to bring this idea to life.
A user can start a host app that opens the possiblities to control the game. 

> Showcase gif or short video

## Integration into your project

1. Add the code base as submodule

```bash
git submodule add https://github.com/timjonaswechler/bug_hunter crates/bug_hunter
```

2. Add to your project root `Cargo.toml`

```toml
[workspace]
members = [
    "crates/bug_hunter",
    "crates/bug_hunter/bevy_test_apps",
]    
```

3. Build a host app for your project

```bash
mkdir apps/bug_hunter_host
cd apps/bug_hunter_host
cargo init
```

4. Add the host app to bug_hunter_host `Cargo.toml`

```toml
[dependencies]
bug_hunter = { path = "../../crates/bug_hunter", features = ["host"] }

[dev-dependencies]
image = { version = "0.25", default-features = false, features = ["png"] }
serde_json = "1.0"
```

5. Change the `main.rs` of your host app to use the bug hunter host

```rust
fn main() {
    bug_hunter::host::run_embedded(include_str!("../automation.toml"));
}
```

6. Create the `automation.toml` file

```toml
version = 1
profile_id = "bug_hunter_host-v1"

[tool]
name = "bug_hunter_host"
about = "Start and control one isolated Star Sim session"
default_artifact_dir = "artifacts/bug_hunter_host"

[application]
package = "app"
kind = "binary"
target = "app"
features = ["automation-control"]
arguments = []
mode_argument = "--controlled-mode"

[session]
id = "alpha"

surface_width = 640
surface_height = 360
frame_nanoseconds = 16666667
startup_frames = 1

[report]
generated_by = "bug_hunter_host report"

[screen]
target = "session.status"
component = "app::menu::SessionObservation"
value_pointer = "/active_screen"
result_field = "active_screen"

```




## Roadmap

- [ ] Test/add the functionality script, agent and human can control the game together. 
  The host app is started with a script. After that is done the session waits for any input from a human or agent.

- [ ] Add other issue reporting services
  - [ ] local (fallback)
  - [ ] gitlab
  - [ ] discord
  - [ ] ...

- [ ] 


The public command groups are `Observe`, `Pointer`, `Keyboard`, `Text`, `Time`, `Screenshot`, and `Shutdown`. Pointer
actions are `Move`, `Press`, `Release`, and `Scroll`; there is no wire-level click. Keyboard press
and release are separate requests. `keyboard::Key` defines the stable wire names for letters,
digits, punctuation, modifiers, navigation keys, and F1 through F12. Text commands accept at most
16 KiB of UTF-8 and commit the whole string to the focused Bevy `EditableText`. A missing,
stale, or non-editable focus returns `text_focus_unavailable`.

`time.step` runs between 1 and 10,000 complete application frames. `step_nanoseconds` must be an
integer between 1 and 1,000,000,000. Invalid frame counts and steps return
`invalid_time_frames`, `time_frames_too_large`, `invalid_time_step`, or `time_step_too_large`
without changing the clock. Controlled Sessions do not run Bevy simulation schedules while no
`step` command is pending. Input commands queue session-local Virtual Input until the next `step`, while
Bevy application systems consume all queued transitions atomically on that controlled frame.

A rendered composition opts into screenshots with `bug_hunter::screenshot::Plugin`. The
plugin does not install a renderer, and `ready.controls` includes `screenshot` only when the
composition already has Bevy's renderer and screenshot capture. Logical Mode and renderer-free
compositions return `screenshot_capability_unavailable`.

`ready` is a control-plane signal. It reports that the protocol accepts commands and that the
advertised services exist. It does not mean that Bevy has completed UI layout, render extraction,
or its first presented frame. Controllers that need an image must `step` separate controlled
frames and poll an application-specific visible condition with explicit `observe` + `step` loops, such as an expected tile color.
Sending several frames in one `time.step` request steps simulation schedules in one outer
application update, so it is not a substitute for separate `step` calls while the first rendered scene
is being prepared.

Screenshot paths must be normalized, relative `.png` paths beneath the host-provided session
artifact root. Absolute paths, `.` or `..` components, and symbolic links are rejected. A completed
response contains the artifact type, session-relative path, MIME type, width, and height. The
response is written only after GPU readback, PNG encoding, and a successful read-back check. Fixed
scene dimensions and semantic image content are testable. Identical PNG bytes across operating
systems and GPUs are not guaranteed.

## Embed the plugin

```rust
use bug_hunter::AutomationControlPlugin;

app.add_plugins(AutomationControlPlugin::logical_stdio());
// A rendered composition uses `rendered_stdio()` instead.
```

Tests and embedding applications provide the protocol mode and transport adapters explicitly:

```rust
use bug_hunter::RunMode;

app.add_plugins(AutomationControlPlugin::with_io(
    RunMode::Rendered,
    input,
    output,
));
```

`RunMode` is protocol metadata. It does not select an application composition or install or remove a
renderer; the embedding application remains responsible for composing the matching session.

The rendered `bevy_test_apps` composition disables `InputPlugin` and `GilrsPlugin`. The control
plugin also disables Bevy's native mouse and touch picking producers, writes `PointerInput` into
Bevy's picking pipeline, and clears the OS cursor stored on Bevy windows. After the first Virtual
Pointer move, Rendered Mode displays a red circular position helper above the UI. The helper is
ignored by picking and is not installed in Logical Mode. Because UI, focus, and picking systems
still require low-level Bevy input message channels and state resources when
`InputPlugin` is absent, the plugin registers them empty as
compatibility prerequisites. Registration neither creates native input producers nor opens an OS
input connection; the composition's native producers remain disabled. The plugin clears native
window, keyboard, focus, mouse, touch, scroll, gamepad, and IME message buffers before focused-input
dispatch.

Virtual keyboard commands write `KeyboardInput`, update Bevy's `ButtonInput<KeyCode>` and
`ButtonInput<Key>` resources, and use Bevy's focused-input dispatch. Virtual text commands write
`Ime::Commit`, which `EditableTextInputPlugin` routes to the focused `EditableText`. The plugin
processes one request per event-loop update. Input responses acknowledge the queued session-local
transition. A `step` response is written only after every requested frame, including application
observers and text-edit systems, has completed. `stdout` contains JSONL only. Diagnostics use
`stderr`.

## Observation

`observation::observe_world` computes each answer from the current World. Selectors cover target
markers, UI entities, pointer entities, and one entity handle. Projections include summaries,
component names, selected reflected component values, and bounded hierarchies. Results are sorted
by session-local `{index,generation}` handles and paged with a hard limit and stateless cursor.
Reflection is read-only. Components that are absent, unregistered, not reflectable, not serializable,
or too large receive an explicit status.

The `clock` selector with the `summary` projection reports the completed frame index, elapsed
controlled nanoseconds, and the last step in nanoseconds. Repeated observations do not change it.

```json
{"sequence":7,"command":{"type":"observe","selector":{"type":"clock"},"projection":{"type":"summary"},"limit":1}}
```

The `virtual_input` selector with the `summary` projection reports the session's pointer position,
pressed pointer buttons, last scroll delta, held keyboard keys, current Bevy input focus, and last
text commit. These resources belong to one Controlled Session and are never shared with another
session.

```json
{"sequence":8,"command":{"type":"observe","selector":{"type":"virtual_input"},"projection":{"type":"summary"},"limit":1}}
```

`AutomationTarget` is an empty marker. It has no persistent semantic ID, role, label, or action
list. Handles are valid only for the current Bevy World.

## Debug Host

Enable the optional Debug Host support for a host application. The former `driver` feature and
`bug_hunter::driver` module remain aliases for compatibility.

```toml
bug_hunter = { path = "../../crates/bug_hunter", features = ["host"] }
```

```rust
let mut session = Session::spawn(&launch, SessionOptions::new())?;
let ready = session.ready()?;
session.request(Command::Shutdown)?;
session.shutdown()?;
```

`host::Session` owns protocol and recording sequences, child-process lifecycle, JSON serialization,
response matching and stderr streaming. `LaunchSpec` can start Cargo binaries or examples.
Diagnostics and report helpers remain available to host tools. `host::run_embedded` adds the generic
REPL, Session Script, Session Replay, and artifact-report workflows from a validated TOML profile.
Application-specific launch and observation details belong in that embedded profile rather than in
the host implementation.

## Session Recording

Enable recording at launch with an artifact-root-relative path:

```rust
use std::path::PathBuf;
use bug_hunter::driver::{SessionOptions, recording::Controller};

let options = SessionOptions::new()
    .with_artifact_dir("artifacts/session")
    .with_record(Some(PathBuf::from("recordings/run.jsonl")))
    .with_recording_context(
        "alpha",
        bug_hunter::RunMode::Logical,
        serde_json::json!({"surface": [640, 360]}),
    )
    .with_controller(Controller::new("repl"));
```

`with_recording_context` lets a host include its complete configuration and checks its mode against
`ready`. Without explicit context, the driver waits for `ready` and fills the mode and protocol before
writing `SessionStarted`. `Session::start_recording(None)` allocates a collision-free path below the
artifact root. `Session::stop_recording()` flushes and closes the current segment. Starting another segment keeps
the same strictly increasing host sequence and writes the complete open-session context first.
Requested paths must be relative `.jsonl` paths without `.` or `..` components, and existing files
or symbolic-link escapes are rejected.

The JSONL format has its own `driver::recording::FORMAT_VERSION`. Every line contains `version`,
`sequence`, and a typed event. Events distinguish session context, source-neutral Controller
actions with Controller-origin metadata, game responses, observations, errors, artifact references,
recording stops, and session completion or abort. The recorder never copies child stderr. It redacts
sensitive fields and messages and bounds strings, collections, nesting, and serialized entry size so
credentials and raw model prompts cannot enter the file. `recording::Recording::parse_reader` and
`parse_path` validate the version and strictly increasing
sequences, returning an unsupported-version error for newer formats.

Timing is explicit: there is no `wait` primitive. Controllers poll with `observe` + `step` loops. Batching is supported — multiple Virtual Inputs may be queued before one `step` and are consumed atomically in the next frame:

```rust
session.request(Command::Pointer(PointerCommand::Move { surface: None, position: [10.0, 20.0] }))?;
session.request(Command::Keyboard(KeyboardCommand::Press { key: Key::A }))?;
session.request(Command::Text(TextCommand::new("hi")))?;
session.request(Command::Time(TimeCommand::step(1, 16_666_667)))?; // all three visible in this frame
```

Polling example:

```rust
for _ in 0..120 {
    let obs = session.request(Command::Observe(observation.clone()))?;
    if obs.result["items"].as_array().is_some_and(|items| !items.is_empty()) { break; }
    session.request(Command::Time(TimeCommand::step(1, 16_666_667)))?;
}
```

## Bevy application smoke tests

The display-free `logical_state` binary runs in Logical Mode with a fixed data-only surface. It exercises Virtual Pointer and Keyboard input, controlled timers, `Update`, `FixedUpdate`, reflected state, deterministic observations, bounded host waits, and typed screenshot rejection through a real child process:

```bash
cargo test -p bug_hunter --features driver --test logical_state -- --test-threads=1
```

The `bevy_test_apps` package also contains the rendered `context_menu` binary. Without features it runs as a Player
Run. With `automation`, it disables `InputPlugin` and the native gamepad producer, enables the
Controlled Session plugin, marks the background, buttons, menu items, and editable text target, and
exposes reflected pointer, keyboard, and text state. Package structure, run commands, target naming,
reflection conventions, and the adapted Bevy example's provenance are documented in
[`bevy_test_apps/README.md`](bevy_test_apps/README.md).

```bash
cargo run -p bug_hunter --example bevy_controller --features driver
```

The default smoke exits as soon as its assertions pass. To keep the Controlled Session open while
moving the virtual pointer in one circle per second, provide a duration:

```bash
cargo run -p bug_hunter --example bevy_controller --features driver -- --circle-seconds 60
```

A display and render adapter are required for that smoke test. Display-free protocol, observation,
and driver tests do not need them.

## Testing

```bash
cargo test -p bug_hunter
cargo test -p bug_hunter --features driver
```

The rendered readiness stress test launches fresh `ui_drag_drop` sessions and distinguishes black
textures, Bevy's clear-only frame, and screenshots containing scene content. Build the controlled
binary once so repeated launches do not include Cargo startup time:

```bash
cargo build -p bevy_test_apps --bin ui_drag_drop --features automation
RENDER_STRESS_APP=target/debug/ui_drag_drop \
  cargo run -p bug_hunter --example render_readiness_stress --features driver
```

Use `RENDER_STRESS_RUNS`, `RENDER_STRESS_FRAMES`, `RENDER_STRESS_ATTEMPTS`, and
`RENDER_STRESS_DELAY_MS` to change the run. `RENDER_STRESS_CAPTURE_READY=true` also records the
frame available immediately after `ready`.

Replay, camera operations, screenshot comparison, video capture, persistent semantic IDs, and
model adapters are follow-up work outside this slice.
