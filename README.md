# `woodpecker`

Control and inspect a Bevy application through an explicit local session.
A loopback-only server owns multiple independent game processes; CLI clients can
disconnect without stopping accepted work.

Local clients are trusted. No access key or environment setup is required.
Remote operation is not supported.

## Current implementation

The experimental v3 slice supports tick warps, pace changes, stop, reflected
resource and entity inspection, virtual pointer, keyboard and text input, PNG screenshots,
session management, JSONL recording, replay, reporting, REPL, scripts and shutdown.
Combined lifecycle/load acceptance remains unfinished. Fully covered windows are
not guaranteed to be capturable; an unavailable render surface produces an explicit error.
Actual black images are valid. Screenshot comparison is not a product feature.
The v2 API and its `host`/`driver` features have been removed.

Controlled sessions ignore native mouse, touch, keyboard and IME input. Their pointers
do not move the OS cursor or require window focus. Applications using Bevy UI must
enable `woodpecker/ui`, which also enables focused text input. The context-menu fixture
enables it through `slice`.

- [Target contract](docs/api/target.md)
- [Completion plan and coverage](docs/api/implementation-plan.md)
- [Handoff and remaining tasks](docs/api/next-steps.md)
- [CLI usage and acceptance tests](docs/api/usage.md)
- [Bevy test applications](bevy_test_apps/README.md)

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

The [walkthrough](docs/api/usage.md#ausführen) continues with Warp, Inspect and shutdown.
The server and CLI currently require Unix process groups.

## Rust integration and features

The library exposes `command`, `handle`, `session` and report configuration.
Use `session::Plugin` in the game and `session::Session` for direct process control.
The default library has no HTTP/CLI dependencies and does not install or require a
renderer. It uses Bevy Render types to guard an application-installed RenderApp:
extraction and rendering begin after the first completed explicit tick, then
continue while simulation is paused. Startup never runs a hidden warm-up tick.

| Feature | Entry points |
| --- | --- |
| `ui` | Virtual legacy UI interaction and focused text input |
| `screenshot` | GPU readback and sandboxed PNG output in rendered applications |
| `server` | Local session management and HTTP/WebSocket serving |
| `client` | Network management and fixed-session access |
| `cli` | The `woodpecker` executable |

## Tests

See [tests/README.md](tests/README.md) for groups, build prerequisites and coverage
retention decisions. Rendered acceptance requires separate GUI approval.

```sh
cargo test --all-features --all-targets
cargo test --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets
python3 -m tests.acceptance.headless.session_lifecycle
python3 -m tests.acceptance.rendered.context_menu
python3 -m tests.acceptance.headless.server_shutdown
```

The process tests launch real children. macOS Gatekeeper can delay or reject
locally built executables; see the [environment note](docs/api/usage.md#noch-nicht-enthalten).

## Origin

Inspired by [ThePrimeagen's game-development video](https://www.youtube.com/watch?v=tYQyh1tjSFc).
The existing [GitHub repository URL](https://github.com/timjonaswechler/bug_hunter)
and versioned report signature identifiers remain unchanged until the coordinated rename.
