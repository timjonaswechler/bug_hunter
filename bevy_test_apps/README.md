# Bevy test applications

This package holds small Bevy applications used to exercise `automation_control`. Each application is an explicit binary, so another application only needs a new `src/bin/*.rs` file and `[[bin]]` entry.

Rendered binaries use `composition::rendered` with a configured window. Without features, it builds a Player Run with Bevy's native input plugins. With `--features automation`, it builds a Rendered Mode Controlled Session with `InputPlugin` and `GilrsPlugin` disabled, screenshot support enabled, and `automation_control` supplying Virtual Input over protocol v2.

With `--features automation`, `logical_state` uses `composition::logical`. Without the feature it uses the rendered composition as a native Player Run, like the other binaries. The logical composition installs no Winit, `WindowPlugin`, `RenderPlugin`, `InputPlugin`, Gilrs, or native pointer producer. It creates one data-only `Window` component with fixed dimensions so Bevy UI layout and Virtual Pointer coordinates share a session-local surface. No operating-system window backs that entity.

Controlled compositions still register the empty low-level Bevy input message channels and state resources required by UI, focus, and picking systems. These are compatibility prerequisites, not native input producers or operating-system connections. The control plugin clears native message buffers before focused-input dispatch.

## Context menu

Run the application with native mouse and keyboard input:

```bash
cargo run -p bevy_test_apps --bin context_menu
```

The controller launches the automation composition and checks pointer buttons, scrolling, keyboard state, focused text entry, reflected state, and screenshots through the public `driver::Session` interface:

```bash
cargo run -p automation_control --example bevy_controller --features driver
```

## Blend modes

Run the adapted 3D blend-modes scene as a Player Run:

```bash
cargo run -p bevy_test_apps --bin blend_modes
```

Add `--features automation` to run it as a Controlled Session. The camera and five blend-mode spheres have stable names and `AutomationTarget` markers. The camera's reflected `blend_modes::SceneState` reports alpha, HDR, unlit mode, camera angle, color seed, color-change count, and the five sphere colors. Each sphere exposes its session-local material asset ID through the reflected `blend_modes::ObservedMaterialHandle` component, alongside Bevy's `MeshMaterial3d<StandardMaterial>` component.

The scene uses seed `0x5eedb1e5`. Each `C` press derives colors from that seed, the color-change count, and the object's stable color slot. Identical seeds and key sequences therefore produce identical observed colors.

The rendered controller checks held arrow keys, controlled time, separate mode-key presses, reflected state, camera and material components, and screenshots at fixed controlled frames:

```bash
cargo run -p automation_control --example blend_modes_controller --features driver
```

## Mesh picking

Run the 3D picking scene with native pointer input:

```bash
cargo run -p bevy_test_apps --bin mesh_picking
```

The rendered controller drives the same binary through Virtual Input and controlled time. It checks mesh hover, press, release, drag, reflected transforms and interaction state, deterministic rotation, and screenshots:

```bash
cargo run -p automation_control --example picking_controller --features driver
```

## UI drag and drop

Run the adapted UI grid with native pointer input:

```bash
cargo run -p bevy_test_apps --bin ui_drag_drop
```

The rendered controller derives tile centers from observed UI bounds, then builds complete drags from pointer moves, a press, controlled frames, and a release. It checks the Bevy `DragStart`, `Drag`, `DragDrop`, and `DragEnd` lifecycle, reflected layout and presentation components, valid and invalid drops, and screenshots of the initial, dragging, and dropped states:

```bash
cargo run -p automation_control --example ui_drag_drop_controller --features driver
```

## Game menu

Run the adapted multi-screen menu as a Player Run:

```bash
cargo run -p bevy_test_apps --bin game_menu
```

With `--features automation`, the persistent `game-menu-state` target reflects the active game state, menu state, display quality, volume, and both timer positions. Screen roots, navigation buttons, and setting buttons have stable names and session-local target handles.

The rendered controller navigates with Virtual Pointer and Virtual Keyboard input, verifies state-scoped despawning and stale-handle rejection, and captures the main menu, settings, display settings, and game screen:

```bash
cargo run -p automation_control --example game_menu_controller --features driver
```

## Logical state

`logical_state` covers UI layout, Virtual Pointer presses, held Virtual Keyboard input, timers, `Update`, `FixedUpdate`, and reflected state without a display server or render adapter:

```bash
cargo run -p bevy_test_apps --bin logical_state --features automation
```

The driver integration test removes display environment variables and controls the child through the public session API:

```bash
cargo test -p automation_control --features driver --test logical_state -- --test-threads=1
```

## Application conventions

- Put application systems, components, and semantic test state in the binary that owns them. Shared code belongs in `composition::rendered` or `composition::logical` only when every app in that mode needs it.
- Give observable entities stable, descriptive `Name` values. Names must not depend on spawn order or an entity handle. Use lowercase kebab-case where a name has multiple words.
- Add `AutomationTarget` only to entities a Controller must find or operate. Keep the marker behind `cfg(feature = "automation")`.
- Store assertions that span several UI systems in a small application-owned component. Derive `Reflect`, add `#[reflect(Component)]`, and register the type with `App::register_type`.

## Source and license

`context_menu` adapts Bevy 0.19.1's [`examples/usage/context_menu.rs`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/usage/context_menu.rs). The text-input field, semantic session state, stable names, and Controlled Session integration are Star Sim changes.

`blend_modes` adapts Bevy 0.19.1's [`examples/3d/blend_modes.rs`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/3d/blend_modes.rs). Stable targets, reflected state, deterministic colors, fixed window dimensions, and Controlled Session integration are Star Sim changes.

`mesh_picking` adapts Bevy 0.19.1's [`examples/picking/mesh_picking.rs`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/picking/mesh_picking.rs). Star Sim reduces the scene to three stable targets and adds Controlled Session input, deterministic rotation checks, reflected interaction state, and screenshot assertions.

`ui_drag_drop` adapts Bevy 0.19.1's [`examples/ui/ui_drag_and_drop.rs`](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/ui/ui_drag_and_drop.rs). Star Sim uses a compact grid with stable targets and adds reflected lifecycle state, Controlled Session gestures, invalid-drop checks, and semantic screenshot assertions.

`game_menu` adapts Bevy 0.19's [`examples/showcase/game_menu.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/examples/showcase/game_menu.rs). Star Sim removes external image assets, adds stable target names, reflects menu state and timer progress, supports keyboard navigation, and supplies a Controlled Session controller.

Bevy distributes these examples under either the MIT License or Apache License 2.0, as recorded in Bevy's [repository license files](https://github.com/bevyengine/bevy/tree/v0.19.1#license).
