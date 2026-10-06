# Bevy test applications

This is a separate Cargo package, not a workspace member. Run commands from the
repository root with `--manifest-path bevy_test_apps/Cargo.toml`.

## Headless session acceptance

`counter` is the headless woodpecker integration. It uses `session::Plugin`,
an application-owned simulation schedule and a reflected counter resource.

```sh
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin counter
```

Start it through the CLI using `tests/fixtures/counter.toml`; see the
[walkthrough](../docs/api/usage.md#ausführen). The raw game binary expects the
session's internal launch environment.

## Rendered session acceptance

`context_menu` also uses `session::Plugin` when built with `slice`. Without that
feature it remains a native application. The acceptance test finds named entities
through general Inspect, reads layout coordinates, queues pointer input and checks
that application state changes only after an explicit Warp.
The woodpecker dependency enables `ui` so both picking observers and legacy `Interaction`
use the virtual pointer. The controlled window starts without requesting focus.
The test runs two concurrent sessions with independent pointer/button states.
It also presses, holds and releases `a` independently in both sessions and checks
the reflected key counters. Keyboard events do not insert text.
The test then focuses each text field with its virtual pointer and submits different
Unicode strings through `input.text.input`. The reflected text is sampled after
Bevy applies text edits, so a single explicit tick shows the resulting value.
The dependency also enables `screenshot`. The test captures the closed and open
menu, validates PNG chunks and pixel data, and checks that capture changes neither
the reflected tick counter nor simulated time. It covers overwrite, concurrent
requests, symlink escape rejection and separate session artifact roots.
The first session is also recorded. The test validates its JSONL header, footer,
command count, input and screenshot entries, and exclusions for recording controls
and commands from the second session.
The extended test also replaces an open menu, selects an item and reopens/closes
it, checking ordered child layout and every despawned subtree handle. Button and
text-field layout remain stable. The complete graphical acceptance passed,
including recording/replay and two sessions; see the completion plan for evidence.
Logs, full CLI responses, recordings and PNGs remain under `target/ui-*`, including
on failure.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin context_menu
python3 -m tests.acceptance.rendered.context_menu
# Retain the screenshots for visual inspection:
python3 -m tests.acceptance.rendered.context_menu --capture-dir target/ui-captures
```

This test opens a real window and requires a desktop session.
The CLI launch configuration is `tests/fixtures/context_menu.toml`.

## Controlled time and input acceptance

`logical_state` now installs `session::Plugin` with `slice`. The application chooses
20 ms simulation ticks, a 10 ms fixed step and a repeating 40 ms timer. Without
`slice` it still uses native input and automatic time.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin logical_state
python3 -m tests.acceptance.rendered.logical_state
```

This opens a real window. The test inspects the existing `SessionObservation`
component through the CLI, checks Update/FixedUpdate/timer counts and queued
keyboard press/hold/release, and proves that Inspect and real waiting do not advance
the scene. Bevy's first Time update has zero delta; subsequent ticks use 20 ms even
under a wall-clock pace limit. The extended test also queues a pointer press at
the inspected button center, checks a single press across held ticks, and captures
five PNGs, including pending press/release states. A green button on black is a
technical pixel fixture; capture must leave the entire observation unchanged.
The extended graphical acceptance passed; commands, results and artifact paths
are recorded in the completion plan.
Logs, PNGs and CLI evidence remain in `target/logical-state-*`, including on failure. The [coverage matrix](../docs/api/implementation-plan.md#abdeckungsmatrix)
records the remaining scenarios.

## Game menu acceptance

With `slice`, `game_menu` installs `session::Plugin` and chooses 100 ms simulation
ticks. Native time and input remain unchanged without the feature.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin game_menu
python3 -m tests.acceptance.rendered.game_menu
```

This desktop test uses the CLI to navigate from splash through display and sound
settings into the game and back after its timer expires. It reads actual button
positions through Inspect and sends virtual pointer input without native focus.
It verifies explicit tick boundaries, persistent settings, screen hierarchies and
`entity_not_found` for despawned screen/button handles. Input helpers do not tick.
Logs and full CLI responses remain under `target/game-menu-*`, even on failure.
The extended test adds `s`, `Escape` and `n` shortcuts with explicit input and
state-transition ticks, dead screen/button handles, and technical PNG checks for
splash, menu, settings and game screens, including pending keyboard input.
Quit is tested as an unexpected successful process exit while a second session
remains controllable. The full extended acceptance passed after the coordinator's
bounded EOF clarification fix: all nine PNGs, natural exit 0, process-exit failure,
local report and post-Quit session isolation were verified. The server's final
`shutdown_incomplete` is expected because the Quit session failed. The completion
plan retains evidence for both the initial failing run and the successful run.

## UI drag-and-drop acceptance

With `slice`, `ui_drag_drop` installs `session::Plugin` and chooses 20 ms simulation
ticks. Without it, native input and automatic time remain unchanged.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin ui_drag_drop
python3 -m tests.acceptance.rendered.ui_drag_drop
```

This desktop test reads actual tile positions and sizes through Inspect. Virtual
pointer commands drag Amber onto Blue, then over empty background. Explicit ticks
drive each input step. Assertions cover the intermediate position, active tile,
ordered drag phases, swapped or unchanged occupancy, all final tile positions and
restored transform, outline and z-index. Idle ticks must not repeat drag events.
No native focus or physical input is needed. The extended test also removes Amber
during an active drag using the slice-only, tick-bound `D` scene trigger. It checks
dead tile/child handles, hierarchy, release without an invalid swap, and a subsequent
Blue-to-Green drag. Eight technical PNG checks cover known scene colors and frozen
state/geometry, including a pending despawn key. The complete extended graphical
acceptance passed; commands, results and artifact paths are recorded in the
completion plan. Evidence remains under `target/ui-drag-drop-*`, including on failure.

## Mesh-picking acceptance

With `slice`, `mesh_picking` uses 20 ms ticks and activates its 3D camera on the
first explicit tick, after which Bevy can render initialized light clusters.
The native application still starts with an active camera and automatic time.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin mesh_picking
python3 -m tests.acceptance.rendered.mesh_picking
```

The test derives pointer positions from inspected camera and mesh geometry.
It reads actual render dimensions and scale rather than assuming the requested
window size. It checks hover, press, release and out on all three meshes, a
horizontal cube drag and exact timed rotation. The extended test also covers a
vertical cube drag, diagonal sphere/cylinder drags, held-pointer stillness, release
and renewed hover. Full quaternion checks include the normal timed rotation of
all meshes and forbid drag increments on unselected objects. It now contains
19 screenshots over 48 explicit ticks; the complete extended graphical acceptance
passed. Commands, results and artifact paths are recorded in the completion plan.
Local pixel checks distinguish neutral, cyan and yellow material states; other
objects must not inherit the selected object's material. Three headless oracle
tests run with `python3 -m unittest discover -s tests -p 'test_mesh_rotation.py'`.

Evidence remains under `target/mesh-picking-*`. Use an available desktop.
The [controlled visibility tests](../docs/api/diagnostics/screenshot-evidence.md)
confirmed a skipped screenshot copy under full window occlusion on macOS/Metal.
Visible, partially covered and restored-window acceptance passed. The adapter
rejects captures without a window surface instead of writing an unfilled buffer.
This does not establish the cause of every historical black PNG or a lid-related
cause.

## Combined investigation fixture

`logical_state --investigation` (only with `slice`) adds an opt-in reflected
`Investigation` resource and emits one tracing error on the first explicit tick
after a B press. Holding B does not repeat the failure. Normal `logical_state`
behavior is unchanged. `tests/fixtures/investigation.toml` enables automatic local
reports; `python3 -m tests.acceptance.rendered.investigation` exercises recording, queued input,
Inspect/Capture, reconnects, reports, management stop and replay in a fresh session.
The experimental monitor selection and delayed window creation were removed on
user request. Windows use their original automatic placement; the 640×360 size
and fixture pixel assertions remain unchanged.

Build `woodpecker` with `cli` and `logical_state` with `slice` first. This is a
**graphical** acceptance requiring fresh approval, two sequential visible windows
and stable window geometry. Set up the desired working display before starting.
Confirm visibility using the printed `visible-1.json` / `visible-2.json` path;
leave windows on their initial display. No native focus or placement manipulation.
Before the
first recorded tick the window may still be black; green pixels are checked only
after the explicit layout warm-up. Results remain under `target/investigation-*/`;
headless fixture tests are not pixel acceptance. The complete graphical run passed
in `target/investigation-ijz15dy0/`: two sessions, five ticks each, six matching
640×360 captures, reproduced failure/signature, closed recordings and unchanged
reports after management stop. All test processes exited. See the
[combined acceptance plan](../docs/api/implementation-plan.md#3-kombinierten-untersuchungsablauf-abnehmen).

## Blend-mode acceptance

With `slice`, `blend_modes` uses 20 ms ticks and activates its 3D camera on the
first explicit tick. Native input and time remain unchanged without the feature.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin blend_modes
python3 -m tests.acceptance.rendered.blend_modes
```

Keep window dimensions, scale and display assignment stable during acceptance.
Dynamic resize/DPI/display changes are deferred from the current scope. The
experimental resize mode and presentation adapter were removed; findings remain
in [the diagnostic record](../docs/api/diagnostics/window-schedules.md).

Two real sessions verify held arrow keys, camera orbit, alpha limits, separate
HDR/unlit/color key presses, stable material identities and two reproducible color
sequences. The first session executes 189 ticks, the second 16.
Six screenshots check all five blend modes at alpha endpoints, restoration,
lit/unlit rendering, rendering with HDR enabled and a color change.
The screenshot extension also checks both active cameras targeting the
primary window (3D order 0, UI order 1 without clearing), inspected UI bounds and
white glyphs in controls/status overlays. Alpha changes must update the status
mask while controls remain stable; restoration must restore both masks. Existing
3D color checks use the same PNGs. The full extended acceptance passed with a
visible, stationary window; evidence and two prior failed runs are recorded in
the completion plan. This does not establish resize/scale-change correctness.
The test reads actual render dimensions and reuses the mesh test's PNG and
quaternion helpers. Evidence remains under `target/blend-modes-*`.

The image checks are not a complete blend-equation or HDR-range test.
Premultiplied receives the same non-premultiplied RGB values as the other modes
in this fixture, so its color remains visible at alpha zero.
All seven scenes now have CLI acceptance; combined lifecycle and load scenarios
remain open, with per-scene limits listed in the coverage matrix.
Two full blend-mode runs passed, but a later rebuild run returned an all-black
first screenshot. The subsequent diagnosis found a skipped GPU copy when the
window surface is unavailable. The adapter now rejects that condition with
`screenshot_window_unavailable` instead of writing the unfilled readback.
Strict blend, mesh and context-menu image tests still fail if their windows lose
the render surface. Their assertions remain unchanged. Fully covered windows are
not guaranteed to be capturable and do not block completion of the current scope.
Pixel checks validate capture for these known fixtures, not a general product
feature for comparing screenshots against reference images.

## Alien Cake Addict: controlled preparation

The provided example and three bundled GLB models were preserved in commit
`b085470` before adaptation. `alien_cake_addict` is now an explicit Cargo target.
It needs **no `slice` feature**. Without the session launch environment it runs
normally with `DefaultPlugins`. When `WOODPECKER_ARTIFACT_DIR` is present it also
installs `woodpecker::session::Plugin`. The bridge currently requires that environment;
installing it unconditionally would not support a normal native launch.

The original gameplay functions, RNG selection, float cooldown, restart behavior,
system ordering, camera and window defaults are retained. No observation system,
fixed timestep or controlled-only game logic. Only Reflection derives/registrations
expose the existing `Game`, `State<GameState>` and `BonusSpawnTimer` resources.
Private Rust values are not automatically inspectable without Reflection metadata.
The asset path is anchored to this package's `assets/` for direct executable launches.
The original RNG APIs use `chacha20` + `rand` 0.10; a dependency alias keeps the
other examples on their existing `rand` 0.9 APIs.

Safe build/headless checks (do not open a window):

```sh
cargo build --features cli
cargo build --manifest-path bevy_test_apps/Cargo.toml --no-default-features --bin alien_cake_addict
cargo test --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets
```

After **fresh GUI approval**, use the normal server/session workflow:

```sh
# Terminal 1, repository root:
target/debug/woodpecker --address 127.0.0.1:4100 server start --artifact-dir target/alien-cake
# Terminal 2: this create command opens the game window.
target/debug/woodpecker --address 127.0.0.1:4100 session create --config tests/fixtures/alien_cake_addict.toml
# Copy the returned full session ID, and wait until session inspect reports Ready.
ID='paste-the-full-session-id-here'
target/debug/woodpecker --address 127.0.0.1:4100 session inspect "$ID"
target/debug/woodpecker --address 127.0.0.1:4100 session repl "$ID"
```

Use the normal `input.keyboard.press` / `release` commands with `arrow_up`,
`arrow_down`, `arrow_left`, `arrow_right` and `space`, then explicit `tick.warp.start`
commands. The plugin controls when the application's schedules execute; it preserves
the application's normal time policy. A tick is **not** guaranteed to be 100 ms and
three ticks do not promise a movement. Use pacing when exercising time-based movement.
The original cooldown and bonus timers still decide the effects. Inspect alone does
not advance gameplay. Native RNG behavior and the original `GITHUB_ACTIONS` seed
condition are unchanged; the session does not force a seed.

Inspect the registered **resource** with this command payload, using `session
submit` + correlated Activity polling or the normal Script/REPL interface:

```json
{"command":"inspect.query","arguments":{"source":"resources","selector":{"kind":"type","type_path":"alien_cake_addict::Game"},"projection":{"kind":"value"}}}
```

`Game` contains the existing board, player/bonus, score, cakes eaten and camera
focus fields. No extra counters, seed field or entity naming was added. Read the
state/timer through their registered resource types and standard Bevy components
through general Inspect. This is metadata, not a new gameplay state model.

Stop explicitly when done:

```sh
target/debug/woodpecker --address 127.0.0.1:4100 session stop "$ID"
target/debug/woodpecker --address 127.0.0.1:4100 server stop
```

The previous invasive adaptation, custom observation, movement script and tests
for changed gameplay were withdrawn. Original gameplay bodies are compared against
`b085470`; build, existing tests and Clippy are checked separately. These are not a
real session/asset-loading or pixel acceptance. Separate approved GUI evidence is
preserved in `target/render-bootstrap-gui-retry-4/` (rendered board/character) and
`target/alien-score-instrumented-1/` (fresh source-blind player reached live score
31 with 17 cakes). The parent independently confirmed the frozen Game, no later
ticks, no source attempts, four captures and clean shutdown. The winning PNG
shows `Sugar Rush: 31`. This score acceptance used temporary SDK diagnostics,
which were archived and removed afterwards. Historical intermittent window loss
was not reproduced or causally fixed; the previous seven-scene acceptance remains
separate. Cake-model pixel rendering is not inferred merely from the score.

## Native application fixtures

The other existing fixtures still use `slice` as a legacy native/controlled switch;
Alien Cake selects this at runtime instead. Their systems, semantic state,
reflection registrations and unit tests are retained for later Input, Inspect
and Screenshot acceptance. They do not expose the removed v2 integration.
There is no `automation` feature or automation marker.

| Binary | Purpose |
| --- | --- |
| `context_menu` | Pointer actions, context menus, keyboard and editable text |
| `blend_modes` | Deterministic colors, material state, camera and keyboard interaction |
| `mesh_picking` | Mesh hover, press, release, drag and reflected transforms |
| `ui_drag_drop` | Valid and invalid drops, layout and drag lifecycle |
| `game_menu` | Menu navigation, state-scoped entities, settings and timers |
| `logical_state` | Update/FixedUpdate, held keys, pointer presses and reflected timer state |
| `alien_cake_addict` | Original gameplay and virtual-keyboard restart; minimal plugin binding, rendered board and source-blind score 31 verified |

For example:

```sh
cargo run --manifest-path bevy_test_apps/Cargo.toml --bin context_menu
cargo test --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets
```

`composition::rendered` installs a normal rendered application.
`composition::logical` remains a data-only UI composition used by tests, without
a native window or renderer. It does not install woodpecker or advance a controlled session.

Give entities stable `Name` values and keep semantic state in application-owned,
registered reflected components. Do not reintroduce marker requirements.
The [completion plan](../docs/api/implementation-plan.md#abdeckungsmatrix)
records the remaining control and image acceptance cases.

## Source and license

These applications adapt Bevy examples with application-owned state, deterministic
test data and descriptive entity names. The original adaptations came from Star Sim.

- [`context_menu`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/usage/context_menu.rs)
- [`blend_modes`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/3d/blend_modes.rs)
- [`mesh_picking`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/picking/mesh_picking.rs)
- [`ui_drag_drop`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/ui/ui_drag_and_drop.rs)
- [`game_menu`](https://github.com/bevyengine/bevy/blob/v0.19.0/examples/showcase/game_menu.rs)

Bevy distributes these examples under the MIT License or Apache License 2.0,
as recorded in its [repository license files](https://github.com/bevyengine/bevy/tree/v0.19.1#license).
