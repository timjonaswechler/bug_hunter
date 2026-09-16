# Bevy test applications

This is a separate Cargo package, not a workspace member. Run commands from the
repository root with `--manifest-path bevy_test_apps/Cargo.toml`.

## Headless session acceptance

`counter` is the supported woodpecker integration. It uses `session::Plugin`,
an application-owned simulation schedule and a reflected counter resource.

```sh
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin counter
```

Start it through the CLI using `tests/fixtures/counter.toml`; see the
[walkthrough](../docs/api/slice.md#ausführen). The raw game binary expects the
session's internal launch environment.

## Native application fixtures

The other binaries use native Bevy input. Their systems, semantic state,
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
