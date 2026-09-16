# `woodpecker`

Control and inspect a Bevy application through an explicit local session.
A loopback-only server owns multiple independent game processes; CLI clients can
disconnect without stopping accepted work.

Local clients are trusted. No access key or environment setup is required.
Remote operation is not supported.

## Current implementation

The experimental v3 slice supports tick warps, pace changes, stop, reflected
Resource-Inspect, session management and shutdown. Input, Entity-Inspect,
screenshots, recording, replay, reporting, REPL and scripts remain planned work.
The v2 API and its `host`/`driver` features have been removed.

- [Target contract](docs/api/target.md)
- [Implementation and migration status](docs/api/implementation-plan.md#migration-und-bereinigung)
- [Handoff and remaining tasks](docs/api/next-steps.md)
- [Runnable CLI walkthrough and acceptance tests](docs/api/slice.md)
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

The [walkthrough](docs/api/slice.md#ausführen) continues with Warp, Inspect and shutdown.
The server and CLI currently require Unix process groups.

## Rust integration and features

The library exposes `command`, `handle`, `session` and report configuration.
Use `session::Plugin` in the game and `session::Session` for direct process control.
The default library has no HTTP/CLI dependencies and does not require a renderer.

| Feature | Entry points |
| --- | --- |
| `server` | Local session management and HTTP/WebSocket serving |
| `client` | Network management and fixed-session access |
| `cli` | The `woodpecker` executable |

## Tests

```sh
cargo test --all-features --all-targets
cargo test --manifest-path bevy_test_apps/Cargo.toml --all-features --all-targets
python3 tests/slice.py
python3 tests/shutdown.py
```

The process tests launch real children. macOS Gatekeeper can delay or reject
locally built executables; see the [environment note](docs/api/slice.md#noch-nicht-enthalten).

## Origin

Inspired by [ThePrimeagen's game-development video](https://www.youtube.com/watch?v=tYQyh1tjSFc).
The existing [GitHub repository URL](https://github.com/timjonaswechler/bug_hunter)
and versioned report signature identifiers remain unchanged until the coordinated rename.
