# `woodpecker`

Control and inspect a Bevy application through an explicit local session.
A loopback-only server owns multiple independent game processes; CLI clients can
disconnect without stopping accepted work.

Local clients are trusted. No access key or environment setup is required.
Remote operation is not supported.

## Current implementation

The experimental v3 slice supports tick warps, pace changes, stop, reflected
resource and entity inspection, virtual pointer, keyboard and text input, PNG screenshots,
session management, JSONL recording/replay, reporting, REPL, scripts and shutdown.
The v2 API and its `host`/`driver` features have been removed.

The target is [headless agent interaction](docs/api/target.md#headless-betrieb):
run a nearly unchanged Bevy game without a native window or display server while
preserving its Window identity, cameras, projections, viewports, stacks,
transforms, UI and normal Bevy picking. Woodpecker should centrally provide the
output attachment, virtual input source and explicit tick control; it should not
require a marked capture camera, a manual 2D/3D choice, an application-owned
parallel image camera or scene-specific raycasting.

That transparent backend is **not implemented yet**. The current opt-in
`headless-2d` and `headless-3d` marker/image-target paths are narrow fallback
fixtures with successful bounded GPU evidence. The `game_menu` image-target flow
also completed its authorized Medium → High → Back → Play acceptance. These runs
validate the existing fixtures only, not general PBR, shadow, text, camera-stack,
HDR/MSAA/tonemapping or readback parity. A CPU-only Bevy 0.19.1 probe has
identified a feasible public attachment/readback seam, but did not run the
render schedule or GPU.

Controlled sessions ignore native mouse, touch, keyboard and IME input. Their
pointers do not move the OS cursor or require window focus. Applications using
Bevy UI must currently enable `woodpecker/ui`, which also enables Bevy UI picking
and focused text input.

- [Authoritative headless integration and file-by-file migration plan](docs/api/headless-integration.md)
- [Target contract](docs/api/target.md)
- [Immediate next step](docs/api/next-steps.md)
- [Broader implementation plan](docs/api/implementation-plan.md)
- [Runnable CLI walkthrough and acceptance tests](docs/api/slice.md)
- [Bevy test applications and fixture status](bevy_test_apps/README.md)

## Build and run

From this repository's root:

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin counter
target/debug/woodpecker --address 127.0.0.1:4100 \
  server start --artifact-dir target/session-artifacts
```

In another terminal:

```sh
target/debug/woodpecker --address 127.0.0.1:4100 \
  session create --config tests/fixtures/counter.toml
target/debug/woodpecker --address 127.0.0.1:4100 session ls
```

The [walkthrough](docs/api/slice.md#ausführen) continues with Warp, Inspect and shutdown.
The server and CLI currently require Unix process groups.

## Rust integration and features

The library exposes `command`, `handle`, `session` and report configuration.
Use `session::Plugin` in the game and `session::Session` for direct process control.
The default library has no HTTP/CLI dependencies and does not require a renderer.

| Feature | Entry points |
| --- | --- |
| `ui` | Virtual legacy UI interaction and focused text input |
| `screenshot` | GPU readback and sandboxed PNG output in rendered applications |
| `headless-2d` | Current legacy fixture: explicit single full-image `Camera2d` target |
| `headless-3d` | Current legacy fixture: explicit single fixed-perspective `Camera3d` target |
| `server` | Local session management and HTTP/WebSocket serving |
| `client` | Network management and fixed-session access |
| `cli` | The `woodpecker` executable |

## Tests

```sh
cargo test --all-features --all-targets
cargo test --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets
python3 tests/slice.py
python3 tests/ui.py
python3 tests/shutdown.py
```

The process tests launch real children. macOS Gatekeeper can delay or reject
locally built executables; see the [environment note](docs/api/slice.md#noch-nicht-enthalten).

## Origin

Inspired by [ThePrimeagen's game-development video](https://www.youtube.com/watch?v=tYQyh1tjSFc).
The existing [GitHub repository URL](https://github.com/timjonaswechler/bug_hunter)
and versioned report signature identifiers remain unchanged until the coordinated rename.
