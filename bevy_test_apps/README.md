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
[walkthrough](../docs/api/slice.md#ausführen). The raw game binary expects the
session's internal launch environment.

## Rendered session acceptance

`context_menu` also uses `session::Plugin` when built with `slice`. Without that
feature it remains a native application. The acceptance test finds named entities
through general Inspect, reads layout coordinates, queues pointer input and checks
that application state changes only after an explicit Warp.
`slice` enables `woodpecker/ui` so both picking observers and legacy `Interaction`
use the virtual pointer. The controlled window starts without requesting focus.
The test runs two concurrent sessions with independent pointer/button states.
It also presses, holds and releases `a` independently in both sessions and checks
the reflected key counters. Keyboard events do not insert text.
The test then focuses each text field with its virtual pointer and submits different
Unicode strings through `input.text.input`. The reflected text is sampled after
Bevy applies text edits, so a single explicit tick shows the resulting value.
`slice` also enables `woodpecker/screenshot`. The test captures the closed and open
menu, validates PNG chunks and pixel data, and checks that capture changes neither
the reflected tick counter nor simulated time. It covers overwrite, concurrent
requests, symlink escape rejection and separate session artifact roots.
The first session is also recorded. The test validates its JSONL header, footer,
command count, input and screenshot entries, and exclusions for recording controls
and commands from the second session.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin context_menu
python3 tests/ui.py
# Retain the screenshots for visual inspection:
python3 tests/ui.py --capture-dir target/ui-captures
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
python3 tests/logical_state.py
```

This opens a real window. The test inspects the existing `SessionObservation`
component through the CLI, checks Update/FixedUpdate/timer counts and queued
keyboard press/hold/release, and proves that Inspect and real waiting do not advance
the scene. Bevy's first Time update has zero delta; subsequent ticks use 20 ms even
under a wall-clock pace limit. It does not yet test this scene's pointer observer
or screenshots. Logs and CLI evidence remain in `target/logical-state-*`, including
on failure. The [coverage matrix](../docs/api/next-steps.md#abdeckungsmatrix-für-block-6)
records the remaining scenarios.

## Game menu acceptance

With `slice`, `game_menu` installs `session::Plugin` and chooses 100 ms simulation
ticks. Native time and input remain unchanged without the feature.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin game_menu
python3 tests/game_menu.py
```

This desktop test uses the CLI to navigate from splash through display and sound
settings into the game and back after its timer expires. It reads actual button
positions through Inspect and sends virtual pointer input without native focus.
It verifies explicit tick boundaries, persistent settings, screen hierarchies and
`entity_not_found` for despawned screen/button handles. Input helpers do not tick.
Logs and full CLI responses remain under `target/game-menu-*`, even on failure.
Screenshots, keyboard shortcuts and the Quit button are not covered here.
The next scene acceptance is `ui_drag_drop`.

## Native application fixtures

The binaries without `slice` use native Bevy input. Their systems, semantic state,
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
The [migration plan](../docs/api/implementation-plan.md#migration-und-bereinigung)
records which v2 controller assertions still need to be ported.

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
