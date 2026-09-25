# Bevy test applications

This is a separate Cargo package, not a workspace member. Run commands from the
repository root with `--manifest-path bevy_test_apps/Cargo.toml`.

The authoritative target and file-by-file migration plan is
[`docs/api/headless-integration.md`](../docs/api/headless-integration.md). The
fixed 2D/3D image-target fixtures below are retained historical/fallback evidence,
not the target application contract. The planned central backend is not yet
implemented; no command in this file constitutes permission for a GPU run.

## Headless session acceptance

`counter` is the headless woodpecker integration. It uses `session::Plugin`,
an application-owned simulation schedule and a reflected counter resource.

```sh
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin counter
```

Start it through the CLI using `tests/fixtures/counter.toml`; see the
[walkthrough](../docs/api/slice.md#ausführen). The raw game binary expects the
session's internal launch environment.

## Historical headless `game_menu` acceptance

With `slice`, `game_menu --headless` keeps the existing menu/state/button code
but replaces the normal window with one fixed 800×600 marked `Camera2d` image
target, disables Winit and uses a controlled 20 ms tick. The matching session
config is `tests/fixtures/game_menu_headless.toml`. This is the current narrow
fixture, not the transparent target architecture.

One authorized screenshot-reading run completed the bounded
Splash → Main → Settings → Display → High → Back → Back → New Game flow. It
used 16 warps / 65 ticks, 8 captures and 6 press/release click pairs; every click
coordinate came from the current PNG. Final state was `game_state=Game`,
`menu_state=Disabled`, and `display_quality=High`. Evidence is retained under
`target/headless-game-menu-agent.RiUArg`. This establishes only that fixed
marker/image-target menu path; it does not establish general UI, camera-stack,
PBR or readback parity. Further GPU runs require separate approval.

## Fixed headless rendered session acceptance (legacy fixture)

`headless_session` is the narrow production capture fixture. It enables
`woodpecker/headless-2d` and explicitly marks its supported
`HeadlessCaptureCamera2d`; ordinary window applications keep window capture even
when they have additional image cameras. The fixture owns one immutable 321×181
`Image` target at scale 1.5, one writing orthographic camera, a red procedural
sprite, a green UI overlay and a black background. Winit is
disabled, `WindowPlugin` creates no primary window, and a startup assertion
rejects any `Window` entity. One explicit initialization warp runs one tick;
capture must not change the reflected tick/time/transform snapshot.

Build only (no GPU or process is started):

```sh
CARGO_TARGET_DIR=/Users/tim-jonaswechler/GitHub-Projekte/bug_hunter/target/asset-maintenance-build \
cargo build --offline --locked --manifest-path bevy_test_apps/Cargo.toml \
  --features slice --bin headless_session
CARGO_TARGET_DIR=/Users/tim-jonaswechler/GitHub-Projekte/bug_hunter/target/asset-maintenance-build \
cargo build --offline --locked --features cli --bin woodpecker
```

The CLI → Session → PNG acceptance is GPU-dependent and explicitly opt-in. It
must not be run as part of a normal test or build. After separate approval, run
exactly once from the repository root:

```sh
CARGO_TARGET_DIR=/Users/tim-jonaswechler/GitHub-Projekte/bug_hunter/target/asset-maintenance-build \
WOODPECKER_CLI=/Users/tim-jonaswechler/GitHub-Projekte/bug_hunter/target/asset-maintenance-build/debug/woodpecker \
WOODPECKER_RUN_GPU=1 python3 tests/headless_session.py
```

The script issues exactly one one-tick warp and one capture, validates PNG CRCs,
filters, dimensions and the three known colors, compares reflected state before
and after capture, then stops session and server. Before starting the server it
prints a unique persistent `target/headless-session-*` evidence directory; PNGs,
server log and session artifacts remain there on success, assertion failure or
timeout, and the success JSON repeats that path. Capture has the production
30-second deadline; script readiness/CLI/cleanup deadlines are 60/40/15 seconds.
One separately approved run was executed on Apple M1 Pro / Metal. It performed
one one-tick warp and one capture, preserved the reflected state, and produced a
valid 321×181 PNG, but the PNG was entirely black instead of containing the red
sprite and green UI. The test exited 1, so graphical acceptance remains open.
Evidence is retained in `target/headless-session-damdmro7` and
`target/headless-session-acceptance.vsw5Gj`; no retry was run.

The fixture deliberately assumes all asset types are registered before the
session plugin cleanup, no late application assets after the initialization
warp and no hot reload. Warm-up readiness is not reused: the exact final blit
pipeline and selected output are checked again for the correlated capture
entity's frame. It does not claim support for arbitrary plugins/readers, 3D,
multiple cameras, partial viewports or projection changes. Existing evidence
cannot distinguish asset transfer, visibility/extraction/phase population, and
the screenshot source. No black-pixel/content gate, hidden warp, or speculative
asset-maintenance fix was added; valid black captures remain valid.

One separately approved diagnostic rerun used
`WOODPECKER_HEADLESS_CAPTURE_DIAGNOSTICS=1` and reproduced the all-black PNG
without a retry. The correlated frame had the target/default GPU images, one
visible and extracted sprite, one ready sprite phase item and one sprite batch;
all 43 cached pipelines were ready. The camera output nevertheless remained the
persistent image target. The camera's `ImageRenderTarget` uses scale 1.5, while
`Screenshot::image(handle)` reconstructs it with scale 1.0; Bevy includes that
scale in the normalized attachment key, so the camera never writes the temporary
screenshot attachment. Evidence: `target/headless-session-9kvwo2gy` and
`target/headless-session-diagnostic-run.fj5KB8`.

The production capture seam now retains the complete `ImageRenderTarget`
through selection, queueing, readiness and screenshot spawning, and creates
`Screenshot(RenderTarget::Image(target.clone()))`. GPU-free regressions prove
that scale 1.5 survives the production start/poll path, that a queued request is
rejected if the camera changes to scale 1.0, and that readiness does not match
the same handle at scale 1.0. Red/green evidence is in
`target/headless-target-identity-red.7X2YYQ`,
`target/headless-target-identity-green.dP2Nav`, and
`target/headless-target-identity-final.VqS5RW`.

One explicitly approved post-fix GPU acceptance run passed with exit code 0.
It performed exactly one one-tick warp and one capture, preserved `SceneState`,
and produced the required 321×181 image: the center pixel is red, `(20, 20)` is
green, and the lower-right pixel is black. Offline decoding counted 8,100 red,
2,592 green, and 47,409 black pixels with no other colors; PNG SHA-256 is
`fc78aac3468dbeaa2ce61054bf4f5dbdb68e9d7d14ef24b9b72d8dfd19cb4069`.
Run logs are in `target/headless-session-post-fix-acceptance.93dP6x`; session
artifacts are in `target/headless-session-m15ac2ki`. No diagnostic instrumentation
was enabled and no processes remained. The total GPU acceptance-run count is
three: the two pre-fix runs exited 1 and this post-fix run exited 0.

The earlier UI extraction zeroes remain diagnostically inconclusive because
`ExtractedUiNodes` was inspected after prepare cleared it and the game-view key
was used instead of the UI-view key. The actual green UI pixel now accepts only
this fixed 2D CLI/session path; it does not establish general headless, 3D, or
input support.

The narrow keyboard slice is now accepted on the same fixed 2D CLI/session
path. With `headless-2d`, existing keyboard press/release commands accept exactly
one marked `Camera2d` image target when no `Window` exists. They queue an
internal virtual keyboard edge without inventing a native window entity or
`WindowEvent`; `ButtonInput<KeyCode>` and logical-key state change only in the
next explicit warp. Existing primary-window behavior is unchanged.

One explicitly approved run of `tests/headless_session_keyboard.py` passed with
exit code 0. It executed exactly three warps with 1 + 10 + 1 ticks and exactly
three captures. Press and release themselves did not advance state. While `D`
was held, the sprite moved 40 logical / 60 physical pixels to the right; the
post-release tick added no movement. All captures preserved `SceneState`, time,
and recorded camera/sprite transforms. Each 321×181 image retained 8,100 red,
2,592 green, and 47,409 black pixels; the green UI bounds stayed identical. The
moved and released PNGs are byte-identical by SHA-256
`9d63ceef7ff7b556a3324340327e810eeb265300a8b118e05affdd916589f5af`.
Evidence: `target/headless-session-keyboard-acceptance.vIKnDP` and
`target/headless-session-keyboard-u7dadzkr`. No diagnostics ran and no processes
remained. The total Session GPU acceptance-run count is four, with exit codes
1, 1, 0, 0. Any further GPU run requires separate approval.

## Fixed procedural 3D session capture (accepted legacy fixture)

`headless_session_3d` is a separate narrow fixture using the same CLI → Session
→ Capture → PNG service. It enables `woodpecker/headless-3d` and marks exactly
one fixed-perspective `Camera3d` on an immutable 321×181 image target at scale
1.5. Two procedural opaque unlit Cuboids overlap in depth; a green UI overlay
and black background provide independent pixel regions. There is no Winit,
`Window`, input, external asset, loader, hot reload, lighting, or shadow scope.

Build and CPU projection-test only (no renderer is started):

```sh
CARGO_TARGET_DIR=/Users/tim-jonaswechler/GitHub-Projekte/bug_hunter/target/asset-maintenance-build \
cargo test --offline --locked --manifest-path bevy_test_apps/Cargo.toml \
  --features headless-3d-session --bin headless_session_3d
CARGO_TARGET_DIR=/Users/tim-jonaswechler/GitHub-Projekte/bug_hunter/target/asset-maintenance-build \
cargo build --offline --locked --manifest-path bevy_test_apps/Cargo.toml \
  --features headless-3d-session --bin headless_session_3d
```

The one authorized opt-in acceptance reached `Ready`, then failed before its
initial `SceneState` response because Metal rejected a zero-width `clustering
dummy texture`. It performed 0 warps, 0 ticks, and 0 captures. The CPU regression
reproduces the underlying lifecycle with Bevy's actual camera and light systems:
`PostStartup` initializes the target, but the Session withholds `PostUpdate`, so
an immediately active `Camera3d` exposed default zero `Clusters` to extraction.
The fixture now starts that camera inactive and activates it in `First` during
the existing one-tick initialization warp; the same test observes nonzero cluster
dimensions afterward.

One separately authorized post-fix run used the unchanged bounded command:

```sh
CARGO_TARGET_DIR=/Users/tim-jonaswechler/GitHub-Projekte/bug_hunter/target/asset-maintenance-build \
WOODPECKER_CLI=/Users/tim-jonaswechler/GitHub-Projekte/bug_hunter/target/asset-maintenance-build/debug/woodpecker \
WOODPECKER_RUN_GPU_3D=1 python3 tests/headless_session_3d.py
```

The script has 60-second Ready, 40-second CLI/capture, and 15-second shutdown
wait bounds. It performs exactly one one-tick initialization warp and one
capture, stores server/session/command/PNG evidence under a unique persistent
`target/headless-session-3d-*` directory even on failure, and does not retry.
That single post-fix run passed on macOS 27.0 with Apple M1 Pro / Metal. Pre-warp
Ready and both fixture/state inspect calls completed before exactly one warp with
one tick and one capture. The 321×181 PNG at scale 1.5 passed the red/blue depth,
green UI, and black-background assertions; capture left tick, virtual time,
camera, and both cuboid poses unchanged. Offline decoding counted 3,906 red,
6,887 blue, 2,592 green, and 44,716 black pixels with no other colors. PNG
SHA-256 is `c02eb84193deef2bccfc193a5f2ad8b4eb9d23eb2e96a4281909ea8d3c64cfff`.
Evidence: `target/headless-session-3d-ip_yzmvp` and
`target/headless-session-3d-post-init-fix-acceptance.bvZ0Uj`. No retry or extra
GPU launch occurred, and no owned process remained. The total Session GPU
invocation count is six, with exit codes 1, 1, 0, 0, 1, 0. This accepts only the
fixed procedural 3D fixture on this macOS/Metal platform, not general 3D,
arbitrary assets, or Linux.

The existing v3 keyboard press/release path now also accepts exactly one suitable
`HeadlessCaptureCamera3d` perspective image target when `headless-3d` is enabled.
It uses the same internal virtual message and tick-bound physical/logical
`ButtonInput` updates as the accepted 2D path; it emits no fake `Window`, raw
`KeyboardInput`, or new wire command. Missing, inactive, orthographic,
ambiguous, or mixed marked 2D/3D targets fail closed. Feature-isolated CPU tests
cover 2D-only, 3D-only, both opt-ins, duplicate transitions, held/released edges,
and the existing window path.

In `headless_session_3d`, held `D` moves only the red front cuboid by 0.08 world
units per game tick along X. The camera activation guard, projection, blue back
cuboid, UI, and static no-key capture remain unchanged. CPU projection checks
place the ten-tick position at X≈1.2 and establish robust interior regions where
the moved red cuboid appears and blue depth is newly exposed.

The separately authorized GPU acceptance ran exactly once and passed with exit
0 on macOS 27.0 / Apple M1 Pro / Metal. It issued exactly three warps totaling
1 + 10 + 1 = 12 ticks and exactly three captures. Press and release acceptance
did not advance the game. Ten held ticks moved the red front cuboid from X≈0.4
to X≈1.2 at the recorded f32 step 0.08; the final released tick did not move it.
Camera, projection, back cuboid, and each capture's complete before/after state
remained identical.

All initial-red, initial-blue, moved-red, newly exposed moved-blue, and green-UI
interior regions were 100%. The moved and released 321×181 images were
byte-identical with SHA-256
`f8aed17e236ffc588aea91ffd2100b33040182210240c82dbfe02f513316f84f`;
the unchanged initial image retained SHA-256
`c02eb84193deef2bccfc193a5f2ad8b4eb9d23eb2e96a4281909ea8d3c64cfff`.
Evidence: `target/headless-session-3d-keyboard-vhmx4ac3` and
`target/headless-session-3d-keyboard-acceptance.fKOX2J`. Cleanup completed with
no owned process remaining and there was no retry. The total Session GPU
invocation count is now seven, with exit codes 1, 1, 0, 0, 1, 0, 0.

## CPU-only central-backend feasibility probe

`tests/window_headless_backend_probe.rs` validates public Bevy 0.19.1 seams
without Winit, OS windows, adapter, `RenderDevice`, server, session, app process
or screenshot. It preserves real Main-World `Window` identity and unchanged
Camera2d/Camera3d targets, projections, viewports and transforms; exercises
Bevy's `camera_system`, `RayMap` and `ButtonInput`; and compile-/registration-
checks a Render-World hook from `ViewTargetAttachments` to an internal
`GpuImage` and `Readback::texture`.

```sh
cargo test --offline --locked --manifest-path bevy_test_apps/Cargo.toml \
  --test window_headless_backend_probe
```

The test does **not** execute the render schedule and proves no GPU draw,
readback, pixel, PBR, shadow, text, stack, HDR/MSAA or tonemapping parity. See the
[authoritative integration plan](../docs/api/headless-integration.md#öffentliche-bevy-anschlussstelle-cpu-belegt).

## Isolated headless rendering probe (legacy fixture)

`tests/headless.rs` exercises a small procedural 2D scene against Bevy 0.19.1,
without Winit or any `Window` entity. It is GPU-dependent and ignored in ordinary
test runs:

```sh
cargo test --manifest-path bevy_test_apps/Cargo.toml --test headless -- --ignored --nocapture
```

The probe checks a 321×181 image at scale 1.5, UI overlay, padded readback rows,
per-request GPU copies, camera readiness deadlines and a valid black scene.
Capture must preserve simulation counters, virtual time and transforms.
Ten explicit ticks initialize the scene, move and rotate its camera, and deliver
virtual world/UI pointer presses. Pointer positions come from real camera and UI
geometry. The test prints its GPU adapter and a temporary directory containing
`scene.png`, `moved.png` and `black.png` for inspection.

The pre-review probe passed on Apple M1 Pro / Metal, and one separately approved
Metal run later passed the corrected final-pipeline gate. `needs_present()` is
an attachment selection flag, not proof of a final draw. Perspective FoV, partial viewports and
zoom/projection changes remain open graphics acceptance cases, not results of
this orthographic full-image probe. Linux without a display server is not yet
verified. This is not CLI/session acceptance or a production adapter. Its narrow shader
maintenance and Image-specific RayMap extension are historical fixture details,
not the central target design. Existing rendered tests, production code and the
window screenshot guard remain unchanged.

Non-graphics checks for the decision used by the copy path:

```sh
cargo test --offline --locked --manifest-path bevy_test_apps/Cargo.toml --test headless
cargo clippy --offline --locked --manifest-path bevy_test_apps/Cargo.toml --test headless -- -D warnings
```

The first command runs five readiness tests and skips the GPU test. A new GPU run
with `--ignored` requires separate approval.

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
on failure. The [headless integration plan](../docs/api/headless-integration.md#noch-offene-nachweise)
records the remaining headless scenarios.

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

## UI drag-and-drop acceptance

With `slice`, `ui_drag_drop` installs `session::Plugin` and chooses 20 ms simulation
ticks. Without it, native input and automatic time remain unchanged.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin ui_drag_drop
python3 tests/ui_drag_drop.py
```

This desktop test reads actual tile positions and sizes through Inspect. Virtual
pointer commands drag Amber onto Blue, then over empty background. Explicit ticks
drive each input step. Assertions cover the intermediate position, active tile,
ordered drag phases, swapped or unchanged occupancy, all final tile positions and
restored transform, outline and z-index. Idle ticks must not repeat drag events.
No native focus or physical input is needed. Evidence remains under
`target/ui-drag-drop-*`, including on failure. This does not test screenshots.

## Mesh-picking acceptance

`mesh_picking` is also the intended unchanged real-scene reference for the later
central window/headless GPU comparison. That comparison has not been authorized
or run; the commands below describe only the existing window acceptance.

With `slice`, `mesh_picking` uses 20 ms ticks and activates its 3D camera on the
first explicit tick, after which Bevy can render initialized light clusters.
The native application still starts with an active camera and automatic time.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin mesh_picking
python3 tests/mesh_picking.py
```

The test derives pointer positions from inspected camera and mesh geometry.
It reads actual render dimensions and scale rather than assuming the requested
window size. It checks hover, press, release and out on all three meshes, a
horizontal cube drag, exact timed rotation and 13 screenshots over 24 explicit
ticks. Local pixel checks distinguish neutral, cyan and yellow material states;
other objects must not inherit the selected object's material.

Evidence remains under `target/mesh-picking-*`. Use an available desktop.
The [recorded visibility findings](../docs/api/diagnostics/rendering-findings.md)
confirmed a skipped screenshot copy under full window occlusion on macOS/Metal.
Visible, partially covered and restored-window acceptance passed. The adapter
rejects captures without a window surface instead of writing an unfilled buffer.
This does not establish the cause of every historical black PNG or a lid-related
cause.

## Blend-mode acceptance

With `slice`, `blend_modes` uses 20 ms ticks and activates its 3D camera on the
first explicit tick. Native input and time remain unchanged without the feature.

```sh
cargo build --features cli --bin woodpecker
cargo build --manifest-path bevy_test_apps/Cargo.toml --features slice --bin blend_modes
python3 tests/blend_modes.py
```

Two real sessions verify held arrow keys, camera orbit, alpha limits, separate
HDR/unlit/color key presses, stable material identities and two reproducible color
sequences. The first session executes 189 ticks, the second 16.
Six screenshots check all five blend modes at alpha endpoints, restoration,
lit/unlit rendering, rendering with HDR enabled and a color change.
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
the render surface. Their assertions remain unchanged. Reliable capture of fully
occluded windows remains open; this guard prevents false success, not occlusion.

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
