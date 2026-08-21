//! Bevy integration for controlled schedules, Virtual Input isolation, and protocol transport.
//!
//! The plugin runs controlled schedules only for explicit time advances, queues Virtual Input for
//! controlled frames, and discards native keyboard, pointer, text, and scroll input. [`RunMode`] is
//! handshake metadata, not a request to install or remove a renderer or window. Screenshot
//! capability remains conditional on the embedding composition.

use crate::{
    keyboard::{self as virtual_keyboard, Command as KeyboardCommand, State as KeyboardState},
    observation::{self, Request as ObservationRequest},
    pointer::{self, Command as PointerCommand, State as PointerState},
    protocol::{self, Command, Response, RunMode},
    screenshot::{self, Command as ScreenshotCommand},
    text::{self as virtual_text, Command as TextCommand, State as TextState},
    time::{Clock, Command as TimeCommand},
    transport::{Input, JsonLinesInput, Output},
};
use bevy::{
    app::{AppExit, MainScheduleOrder},
    ecs::{
        schedule::{InternedScheduleLabel, ScheduleLabel},
        system::SystemParam,
    },
    input::{
        ButtonInput, InputSystems,
        gamepad::GamepadButtonChangedEvent,
        keyboard::{Key, KeyCode, KeyboardFocusLost, KeyboardInput, keyboard_input_system},
        mouse::{
            AccumulatedMouseMotion, AccumulatedMouseScroll, MouseButton, MouseButtonInput,
            MouseMotion, MouseWheel,
        },
        touch::{TouchInput, Touches},
    },
    input_focus::{InputDispatchPlugin, InputFocusPlugin},
    picking::{PickingPlugin, input::PointerInputSettings, pointer::PointerInput},
    prelude::*,
    time::{Real, TimePlugin, TimeReceiver, TimeUpdateStrategy, Virtual},
    window::{Ime, WindowEvent},
};
use serde_json::json;
use std::sync::{Arc, Mutex};

const INPUT_CAPACITY: usize = 64;

/// Installs the Controlled Session protocol and schedule bridge in a Bevy application.
///
/// [`Default`] is equivalent to [`Self::rendered_stdio`]. The embedding application remains
/// responsible for selecting plugins, windows, rendering, and [`crate::screenshot::Plugin`].
#[derive(Clone)]
pub struct AutomationControlPlugin {
    mode: RunMode,
    output: Arc<dyn Output>,
    input_factory: Arc<dyn Fn() -> JsonLinesInput + Send + Sync>,
}

impl std::fmt::Debug for AutomationControlPlugin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AutomationControlPlugin")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl Default for AutomationControlPlugin {
    fn default() -> Self {
        Self::rendered_stdio()
    }
}

/// Converts an input source into the factory used when the Bevy plugin is built.
///
/// The factory is invoked once for one plugin installation. Passing a [`JsonLinesInput`] uses a
/// one-shot factory and panics if invoked more than once; a closure may define other behavior.
pub trait InputFactory: Send + Sync + 'static {
    /// Produces the one-session input factory.
    fn factory(self) -> Arc<dyn Fn() -> JsonLinesInput + Send + Sync>;
}

impl InputFactory for JsonLinesInput {
    fn factory(self) -> Arc<dyn Fn() -> JsonLinesInput + Send + Sync> {
        let input = Mutex::new(Some(self));
        Arc::new(move || {
            input
                .lock()
                .unwrap()
                .take()
                .expect("controlled input factory called once")
        })
    }
}

impl<F> InputFactory for F
where
    F: Fn() -> JsonLinesInput + Send + Sync + 'static,
{
    fn factory(self) -> Arc<dyn Fn() -> JsonLinesInput + Send + Sync> {
        Arc::new(self)
    }
}

impl AutomationControlPlugin {
    /// Configures Logical Mode metadata and stdin/stdout JSONL transport.
    ///
    /// This does not itself create an application composition or remove a renderer.
    pub fn logical_stdio() -> Self {
        Self::stdio(RunMode::Logical)
    }

    /// Configures Rendered Mode metadata and stdin/stdout JSONL transport.
    ///
    /// This does not install rendering, a window, or screenshot support.
    pub fn rendered_stdio() -> Self {
        Self::stdio(RunMode::Rendered)
    }

    fn stdio(mode: RunMode) -> Self {
        Self {
            mode,
            output: Arc::new(crate::transport::StdoutOutput),
            input_factory: Arc::new(|| JsonLinesInput::stdin(INPUT_CAPACITY)),
        }
    }

    /// Configures explicit mode metadata and custom transport adapters.
    ///
    /// The input factory supplies the [`JsonLinesInput`]. The [`Output`] receives the startup
    /// [`crate::Ready`] and subsequent [`Response`] values. `mode` is metadata and does not select
    /// the embedding Bevy composition.
    pub fn with_io(mode: RunMode, input: impl InputFactory, output: Arc<dyn Output>) -> Self {
        Self {
            mode,
            output,
            input_factory: input.factory(),
        }
    }
}

impl Plugin for AutomationControlPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TimePlugin>() {
            app.add_plugins(TimePlugin);
        }
        if !app.is_plugin_added::<PickingPlugin>() {
            app.add_plugins(PickingPlugin);
        }
        if !app.is_plugin_added::<InputFocusPlugin>() {
            app.add_plugins(InputFocusPlugin);
        }
        if !app.is_plugin_added::<InputDispatchPlugin>() {
            app.add_plugins(InputDispatchPlugin);
        }
        // The picking core remains active, but all native mouse and touch producers are disabled.
        // The Controlled Session supplies PointerInput messages itself.
        app.insert_resource(PointerInputSettings {
            is_mouse_enabled: false,
            is_touch_enabled: false,
        });
        register_native_input_channels(app);
        app.insert_resource(Transport {
            input: (self.input_factory)(),
            output: Arc::clone(&self.output),
        });
        app.insert_resource(Configuration { mode: self.mode });
        // Controlled Sessions use explicit manual durations. Disconnect Bevy's render-clock channel
        // so nested controlled frames neither consume wall-clock timestamps nor trigger missing-time
        // warnings when several simulation frames run before the next render frame.
        app.world_mut().remove_resource::<TimeReceiver>();
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .update_with_duration(std::time::Duration::ZERO);
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(std::time::Duration::from_nanos(
                crate::time::MAX_STEP_NANOSECONDS,
            ));
        app.init_resource::<PointerState>()
            .init_resource::<KeyboardState>()
            .init_resource::<TextState>()
            .init_resource::<Clock>()
            .init_resource::<QueuedVirtualInput>()
            .init_resource::<PendingRequests>()
            .init_resource::<ExpectedSequence>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::ZERO,
            ))
            .add_systems(Startup, emit_ready)
            .add_systems(PostStartup, pointer::ensure_mouse_pointer)
            .add_systems(PostStartup, pointer::spawn_helper.run_if(rendered_mode))
            .add_systems(First, pointer::update_helper.run_if(rendered_mode))
            .add_systems(
                control_schedule::Input,
                (
                    discard_native_focused_input,
                    bevy::render::view::window::screenshot::trigger_screenshots.run_if(
                        resource_exists::<
                            bevy::render::view::window::screenshot::CapturedScreenshots,
                        >,
                    ),
                    collect_screenshot,
                    receive_request,
                    dispatch_request,
                )
                    .chain(),
            )
            .add_systems(control_schedule::Frames, run_controlled_frames)
            .add_systems(PreUpdate, keyboard_input_system.in_set(InputSystems))
            .add_systems(control_schedule::Output, complete_request);
        let simulation_schedules = {
            let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
            let simulation_schedules = std::mem::take(&mut order.labels);
            order.labels = vec![
                control_schedule::Input.intern(),
                control_schedule::Frames.intern(),
                control_schedule::Output.intern(),
            ];
            simulation_schedules
        };
        app.insert_resource(SimulationScheduleOrder(simulation_schedules));
    }
}

mod control_schedule {
    use super::ScheduleLabel;

    #[derive(Clone, Debug, PartialEq, Eq, Hash, ScheduleLabel)]
    pub(super) struct Input;

    #[derive(Clone, Debug, PartialEq, Eq, Hash, ScheduleLabel)]
    pub(super) struct Frames;

    #[derive(Clone, Debug, PartialEq, Eq, Hash, ScheduleLabel)]
    pub(super) struct Output;
}

#[derive(Resource)]
struct SimulationScheduleOrder(Vec<InternedScheduleLabel>);

#[derive(Resource)]
struct Transport {
    input: JsonLinesInput,
    output: Arc<dyn Output>,
}

#[derive(Clone, Copy, Resource)]
struct Configuration {
    mode: RunMode,
}

fn rendered_mode(configuration: Res<Configuration>) -> bool {
    configuration.mode == RunMode::Rendered
}

#[derive(Default, Resource)]
struct PendingRequests(Vec<PendingRequest>);

#[derive(Default, Resource)]
struct QueuedVirtualInput {
    pointers: Vec<PointerInput>,
    keyboards: Vec<KeyboardInput>,
    text: Vec<Ime>,
}

enum PendingRequest {
    Observe {
        sequence: u64,
        request: ObservationRequest,
    },
    Pointer {
        sequence: u64,
        command: PointerCommand,
    },
    Keyboard {
        sequence: u64,
        command: KeyboardCommand,
    },
    Text {
        sequence: u64,
        command: TextCommand,
    },
    Time {
        sequence: u64,
        command: TimeCommand,
    },
    Screenshot {
        sequence: u64,
        command: ScreenshotCommand,
        capture: Option<screenshot::Capture>,
    },
    Response(Response),
    Shutdown {
        sequence: u64,
    },
}

#[derive(Resource)]
struct ExpectedSequence(u64);

impl Default for ExpectedSequence {
    fn default() -> Self {
        Self(1)
    }
}

fn register_native_input_channels(app: &mut App) {
    // Focus, UI widgets, and the picking backend still declare these low-level channels and state
    // resources. Registering them is compatibility plumbing only: it installs no native producer
    // or OS connection, and the controlled composition can keep InputPlugin disabled.
    app.add_message::<KeyboardInput>()
        .add_message::<KeyboardFocusLost>()
        .add_message::<Ime>()
        .add_message::<WindowEvent>()
        .add_message::<GamepadButtonChangedEvent>()
        .add_message::<MouseButtonInput>()
        .add_message::<MouseMotion>()
        .add_message::<MouseWheel>()
        .add_message::<TouchInput>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<Key>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<AccumulatedMouseScroll>()
        .init_resource::<Touches>();
}

#[derive(SystemParam)]
struct NativeInputBuffers<'w, 's> {
    keyboard: ResMut<'w, Messages<KeyboardInput>>,
    focus_lost: ResMut<'w, Messages<KeyboardFocusLost>>,
    ime: ResMut<'w, Messages<Ime>>,
    window_events: ResMut<'w, Messages<WindowEvent>>,
    mouse_buttons: ResMut<'w, Messages<MouseButtonInput>>,
    mouse_motion: ResMut<'w, Messages<MouseMotion>>,
    scroll: ResMut<'w, Messages<MouseWheel>>,
    touch: ResMut<'w, Messages<TouchInput>>,
    gamepad: ResMut<'w, Messages<GamepadButtonChangedEvent>>,
    windows: Query<'w, 's, &'static mut Window>,
}

fn discard_native_focused_input(mut input: NativeInputBuffers) {
    input.keyboard.clear();
    input.focus_lost.clear();
    input.ime.clear();
    input.window_events.clear();
    input.mouse_buttons.clear();
    input.mouse_motion.clear();
    input.scroll.clear();
    input.touch.clear();
    input.gamepad.clear();
    for mut window in &mut input.windows {
        if window.physical_cursor_position().is_some() {
            window.set_cursor_position(None);
        }
    }
}

fn emit_ready(world: &World) {
    let configuration = world.resource::<Configuration>();
    let mut ready = protocol::Ready::new(configuration.mode);
    if configuration.mode == RunMode::Rendered && screenshot::is_available(world) {
        ready = ready.with_screenshot();
    }
    if let Err(error) = world.resource::<Transport>().output.ready(&ready) {
        eprintln!("automation-control ready failed: {error}");
    }
}

fn collect_screenshot(world: &mut World) {
    let pending = world
        .resource::<PendingRequests>()
        .0
        .last()
        .and_then(|pending| match pending {
            PendingRequest::Screenshot {
                sequence,
                capture: Some(capture),
                ..
            } => Some((*sequence, capture.clone())),
            _ => None,
        });
    let Some((sequence, capture)) = pending else {
        return;
    };
    let Some(result) = screenshot::take_completion(world, &capture) else {
        return;
    };

    let response = match result {
        Ok(result) => Response::completed(sequence, result),
        Err(error) => Response::error(sequence, error.code(), error.to_string()),
    };
    world.despawn(capture.entity);
    world.resource_mut::<PendingRequests>().0.pop();
    world
        .resource_mut::<PendingRequests>()
        .0
        .push(PendingRequest::Response(response));
}

/// Reads at most one line per update. This keeps request processing ordered and prevents a burst
/// of pointer transitions from becoming one synthetic click in one Bevy update.
fn receive_request(world: &mut World) {
    if !world.resource::<PendingRequests>().0.is_empty() {
        return;
    }
    let input = world.resource::<Transport>().input.try_recv();
    match input {
        Ok(Input::Line(line)) => match protocol::decode_request(&line) {
            Ok(request) => {
                let sequence = request.sequence;
                if let Err(response) = accept_sequence(world, sequence) {
                    write_response(world, response);
                    return;
                }
                let pending = match request.command {
                    Command::Observe(request) => PendingRequest::Observe { sequence, request },
                    Command::Pointer(command) => PendingRequest::Pointer { sequence, command },
                    Command::Keyboard(command) => PendingRequest::Keyboard { sequence, command },
                    Command::Text(command) => PendingRequest::Text { sequence, command },
                    Command::Time(command) => PendingRequest::Time { sequence, command },
                    Command::Screenshot(command) => PendingRequest::Screenshot {
                        sequence,
                        command,
                        capture: None,
                    },
                    Command::Shutdown => PendingRequest::Shutdown { sequence },
                };
                world.resource_mut::<PendingRequests>().0.push(pending);
            }
            Err(response) => {
                if response.sequence > 0
                    && let Err(response) = accept_sequence(world, response.sequence)
                {
                    write_response(world, response);
                    return;
                }
                write_response(world, response);
            }
        },
        Ok(Input::Eof) => {
            world.write_message(AppExit::Success);
        }
        Ok(Input::Error(message)) => {
            write_response(world, Response::error(0, "input_error", message));
            world.write_message(AppExit::error());
        }
        Err(std::sync::mpsc::TryRecvError::Empty | std::sync::mpsc::TryRecvError::Disconnected) => {
        }
    }
}

fn accept_sequence(world: &mut World, sequence: u64) -> Result<(), Response> {
    let expected = world.resource::<ExpectedSequence>().0;
    if sequence != expected {
        return Err(Response::error(
            sequence,
            "unexpected_sequence",
            format!("expected sequence {expected}, got {sequence}"),
        ));
    }
    world.resource_mut::<ExpectedSequence>().0 = expected.saturating_add(1);
    Ok(())
}

fn dispatch_request(world: &mut World) {
    let Some(pending) = world.resource_mut::<PendingRequests>().0.pop() else {
        return;
    };
    match pending {
        PendingRequest::Observe { sequence, request } => {
            world
                .resource_mut::<PendingRequests>()
                .0
                .push(PendingRequest::Observe { sequence, request });
        }
        PendingRequest::Pointer { sequence, command } => {
            let result = {
                let mut state = world.remove_resource::<PointerState>().unwrap_or_default();
                let result = pointer::pointer_event(&mut state, world, &command);
                world.insert_resource(state);
                result
            };
            match result {
                Ok(event) => {
                    world
                        .resource_mut::<QueuedVirtualInput>()
                        .pointers
                        .push(event);
                    world
                        .resource_mut::<PendingRequests>()
                        .0
                        .push(PendingRequest::Pointer { sequence, command });
                }
                Err(error) => {
                    world
                        .resource_mut::<PendingRequests>()
                        .0
                        .push(PendingRequest::Response(Response::error(
                            sequence,
                            "pointer_failed",
                            error.to_string(),
                        )))
                }
            }
        }
        PendingRequest::Keyboard { sequence, command } => {
            let result = {
                let mut state = world.remove_resource::<KeyboardState>().unwrap_or_default();
                let result = virtual_keyboard::keyboard_event(&mut state, world, &command);
                world.insert_resource(state);
                result
            };
            match result {
                Ok(event) => {
                    world
                        .resource_mut::<QueuedVirtualInput>()
                        .keyboards
                        .push(event);
                    world
                        .resource_mut::<PendingRequests>()
                        .0
                        .push(PendingRequest::Keyboard { sequence, command });
                }
                Err(error) => {
                    world
                        .resource_mut::<PendingRequests>()
                        .0
                        .push(PendingRequest::Response(Response::error(
                            sequence,
                            error.code(),
                            error.to_string(),
                        )))
                }
            }
        }
        PendingRequest::Text { sequence, command } => {
            let result = {
                let mut state = world.remove_resource::<TextState>().unwrap_or_default();
                let result = virtual_text::text_event(&mut state, world, &command);
                world.insert_resource(state);
                result
            };
            match result {
                Ok(event) => {
                    world.resource_mut::<QueuedVirtualInput>().text.push(event);
                    world
                        .resource_mut::<PendingRequests>()
                        .0
                        .push(PendingRequest::Text { sequence, command });
                }
                Err(error) => {
                    world
                        .resource_mut::<PendingRequests>()
                        .0
                        .push(PendingRequest::Response(Response::error(
                            sequence,
                            error.code(),
                            error.to_string(),
                        )))
                }
            }
        }
        PendingRequest::Time { sequence, command } => {
            world
                .resource_mut::<PendingRequests>()
                .0
                .push(PendingRequest::Time { sequence, command });
        }
        PendingRequest::Screenshot {
            sequence,
            command,
            capture,
        } => {
            if world.resource::<Configuration>().mode != RunMode::Rendered {
                let error = screenshot::Error::CapabilityUnavailable;
                world
                    .resource_mut::<PendingRequests>()
                    .0
                    .push(PendingRequest::Response(Response::error(
                        sequence,
                        error.code(),
                        error.to_string(),
                    )));
                return;
            }
            if capture.is_some() {
                world
                    .resource_mut::<PendingRequests>()
                    .0
                    .push(PendingRequest::Screenshot {
                        sequence,
                        command,
                        capture,
                    });
                return;
            }
            match screenshot::start(world, &command) {
                Ok(capture) => {
                    world
                        .resource_mut::<PendingRequests>()
                        .0
                        .push(PendingRequest::Screenshot {
                            sequence,
                            command,
                            capture: Some(capture),
                        })
                }
                Err(error) => {
                    world
                        .resource_mut::<PendingRequests>()
                        .0
                        .push(PendingRequest::Response(Response::error(
                            sequence,
                            error.code(),
                            error.to_string(),
                        )))
                }
            }
        }
        PendingRequest::Response(response) => {
            world
                .resource_mut::<PendingRequests>()
                .0
                .push(PendingRequest::Response(response));
        }
        PendingRequest::Shutdown { sequence } => {
            world
                .resource_mut::<PendingRequests>()
                .0
                .push(PendingRequest::Shutdown { sequence });
        }
    }
}

fn run_controlled_frames(world: &mut World) {
    let Some(advance) = world
        .resource::<PendingRequests>()
        .0
        .last()
        .and_then(|pending| match pending {
            PendingRequest::Time { command, .. } => Some(command.into_advance()),
            _ => None,
        })
    else {
        return;
    };
    let schedules = world.resource::<SimulationScheduleOrder>().0.clone();
    for _ in 0..advance.frames {
        *world.resource_mut::<TimeUpdateStrategy>() =
            TimeUpdateStrategy::ManualDuration(advance.step);
        for &schedule in &schedules {
            let _ = world.try_run_schedule(schedule);
            if (*schedule).eq(&First) {
                flush_virtual_input(world);
            }
        }
        world.resource_mut::<Clock>().complete_frame(advance.step);
    }
    *world.resource_mut::<TimeUpdateStrategy>() =
        TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO);
}

fn flush_virtual_input(world: &mut World) {
    let mut queued = world
        .remove_resource::<QueuedVirtualInput>()
        .unwrap_or_default();
    for event in queued.pointers.drain(..) {
        world.write_message(event);
    }
    for event in queued.keyboards.drain(..) {
        world.write_message(event);
    }
    for event in queued.text.drain(..) {
        world.write_message(event);
    }
    world.insert_resource(queued);
}

fn complete_request(world: &mut World) {
    let Some(pending) = world.resource_mut::<PendingRequests>().0.pop() else {
        return;
    };
    match pending {
        PendingRequest::Observe { sequence, request } => {
            let response = match observation::observe_world(world, &request) {
                Ok(result) => Response::completed(sequence, result),
                Err(error) => Response::error(sequence, "observation_failed", error.to_string()),
            };
            write_response(world, response);
        }
        PendingRequest::Pointer { sequence, .. } => {
            write_response(
                world,
                Response::completed(
                    sequence,
                    json!({"pointer": world.resource::<PointerState>().observation()}),
                ),
            );
        }
        PendingRequest::Keyboard { sequence, .. } => {
            write_response(
                world,
                Response::completed(
                    sequence,
                    json!({"keyboard": world.resource::<KeyboardState>().observation()}),
                ),
            );
        }
        PendingRequest::Text { sequence, .. } => {
            write_response(
                world,
                Response::completed(
                    sequence,
                    json!({"text": world.resource::<TextState>().observation(world)}),
                ),
            );
        }
        PendingRequest::Time { sequence, .. } => {
            write_response(
                world,
                Response::completed(
                    sequence,
                    json!({"clock": world.resource::<Clock>().observation()}),
                ),
            );
        }
        pending @ PendingRequest::Screenshot { .. } => {
            world.resource_mut::<PendingRequests>().0.push(pending);
        }
        PendingRequest::Response(response) => write_response(world, response),
        PendingRequest::Shutdown { sequence } => {
            write_response(world, Response::completed(sequence, json!({})));
            world.write_message(AppExit::Success);
        }
    }
}

fn write_response(world: &World, response: Response) {
    if let Err(error) = world.resource::<Transport>().output.response(&response) {
        eprintln!("automation-control response failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Input;
    use bevy::{
        input::ButtonState,
        input_focus::{FocusedInput, InputFocus},
        picking::{
            DefaultPickingPlugins, Pickable, PickingSystems,
            backend::{HitData, PointerHits},
            pointer::{PointerId, PointerLocation},
        },
        window::{CursorMoved, PrimaryWindow, Window, WindowEvent},
    };
    use std::sync::{Mutex, mpsc};

    #[derive(Default)]
    struct MemoryOutput {
        ready: Mutex<Vec<protocol::Ready>>,
        responses: Mutex<Vec<Response>>,
    }

    impl Output for MemoryOutput {
        fn ready(&self, value: &protocol::Ready) -> std::io::Result<()> {
            self.ready.lock().unwrap().push(value.clone());
            Ok(())
        }

        fn response(&self, value: &Response) -> std::io::Result<()> {
            self.responses.lock().unwrap().push(value.clone());
            Ok(())
        }
    }

    fn controlled_app() -> (App, mpsc::SyncSender<Input>, Arc<MemoryOutput>, Entity) {
        controlled_app_in_mode(RunMode::Logical)
    }

    fn controlled_app_in_mode(
        mode: RunMode,
    ) -> (App, mpsc::SyncSender<Input>, Arc<MemoryOutput>, Entity) {
        let (sender, receiver) = mpsc::sync_channel(8);
        let receiver = Mutex::new(Some(receiver));
        let output = Arc::new(MemoryOutput::default());
        let output_trait: Arc<dyn Output> = output.clone();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<WindowEvent>()
            .add_plugins(DefaultPickingPlugins)
            .add_plugins(AutomationControlPlugin::with_io(
                mode,
                move || JsonLinesInput::from_receiver(receiver.lock().unwrap().take().unwrap()),
                output_trait,
            ));
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        (app, sender, output, window)
    }

    #[test]
    fn plugin_disconnects_render_time_from_manually_controlled_time() {
        let (_input_sender, input_receiver) = mpsc::sync_channel(1);
        let input_receiver = Mutex::new(Some(input_receiver));
        let output: Arc<dyn Output> = Arc::new(MemoryOutput::default());
        let (_render_sender, render_receiver) = bevy::time::create_time_channels();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(render_receiver)
            .add_plugins(AutomationControlPlugin::with_io(
                RunMode::Rendered,
                move || {
                    JsonLinesInput::from_receiver(input_receiver.lock().unwrap().take().unwrap())
                },
                output,
            ));

        assert!(!app.world().contains_resource::<TimeReceiver>());
    }

    #[test]
    fn with_io_preserves_the_explicit_mode_in_ready_metadata() {
        for mode in [RunMode::Logical, RunMode::Rendered] {
            let (mut app, _sender, output, _window) = controlled_app_in_mode(mode);
            app.update();
            assert_eq!(output.ready.lock().unwrap()[0].mode, mode);
        }
    }

    #[test]
    fn plugin_uses_no_caller_capability_list_and_emits_v2_ready() {
        let (mut app, sender, output, _window) = controlled_app();
        app.update();
        assert_eq!(output.ready.lock().unwrap()[0].version, 2);
        assert_eq!(
            output.ready.lock().unwrap()[0].controls,
            ["pointer", "keyboard", "text", "time"]
        );
        sender
            .send(Input::Line(
                r#"{"sequence":1,"command":{"type":"shutdown"}}"#.into(),
            ))
            .unwrap();
        app.update();
        assert_eq!(output.responses.lock().unwrap()[0].sequence, 1);
    }

    #[test]
    fn logical_mode_rejects_screenshots_without_initializing_a_renderer() {
        let (mut app, sender, output, _window) = controlled_app();
        app.update();
        assert!(app.get_sub_app(bevy::render::RenderApp).is_none());
        assert!(
            !output.ready.lock().unwrap()[0]
                .controls
                .contains(&"screenshot".into())
        );

        sender
            .send(Input::Line(
                r#"{"sequence":1,"command":{"type":"screenshot","path":"capture.png"}}"#.into(),
            ))
            .unwrap();
        app.update();
        let responses = output.responses.lock().unwrap();
        assert_eq!(
            responses[0].error.as_ref().map(|error| error.code.as_str()),
            Some("screenshot_capability_unavailable")
        );
        assert!(app.get_sub_app(bevy::render::RenderApp).is_none());
    }

    #[test]
    fn out_of_order_sequences_are_rejected_without_hanging_the_session() {
        let (mut app, sender, output, _window) = controlled_app();
        app.update();
        sender
            .send(Input::Line(
                r#"{"sequence":2,"command":{"type":"shutdown"}}"#.into(),
            ))
            .unwrap();
        app.update();
        let responses = output.responses.lock().unwrap();
        assert_eq!(responses[0].sequence, 2);
        assert_eq!(
            responses[0].error.as_ref().map(|error| error.code.as_str()),
            Some("unexpected_sequence")
        );
        assert!(app.should_exit().is_none());
    }

    #[test]
    fn a_typed_command_error_consumes_its_sequence() {
        let (mut app, sender, output, _window) = controlled_app();
        app.update();
        sender
            .send(Input::Line(
                r#"{"sequence":1,"command":{"type":"observe","selector":{"type":"targets"},"projection":{"type":"summary"},"limit":0}}"#.into(),
            ))
            .unwrap();
        app.update();
        sender
            .send(Input::Line(
                r#"{"sequence":2,"command":{"type":"shutdown"}}"#.into(),
            ))
            .unwrap();
        app.update();
        let responses = output.responses.lock().unwrap();
        assert_eq!(responses[0].sequence, 1);
        assert_eq!(
            responses[0].error.as_ref().map(|error| error.code.as_str()),
            Some("invalid_arguments")
        );
        assert_eq!(responses[1].sequence, 2);
        assert_eq!(responses[1].status, protocol::ResponseStatus::Completed);
    }

    #[test]
    fn malformed_commands_consume_their_envelope_sequence() {
        let (mut app, sender, output, _window) = controlled_app();
        app.update();
        sender
            .send(Input::Line(
                r#"{"sequence":1,"command":{"type":"pointer","action":{"type":"move","surface":null,"position":[null,2.0]}}}"#.into(),
            ))
            .unwrap();
        app.update();
        sender
            .send(Input::Line(
                r#"{"sequence":2,"command":{"type":"shutdown"}}"#.into(),
            ))
            .unwrap();
        app.update();

        let responses = output.responses.lock().unwrap();
        assert_eq!(responses[0].sequence, 1);
        assert_eq!(
            responses[0].error.as_ref().map(|error| error.code.as_str()),
            Some("malformed_request")
        );
        assert_eq!(responses[1].sequence, 2);
        assert_eq!(responses[1].status, protocol::ResponseStatus::Completed);
    }

    #[test]
    fn native_mouse_and_touch_input_are_disabled_but_virtual_move_updates_the_pointer() {
        let (mut app, sender, output, window) = controlled_app();
        app.init_resource::<PressCount>();
        let camera = app.world_mut().spawn_empty().id();
        let target = app
            .world_mut()
            .spawn(Pickable::default())
            .observe(|_: On<Pointer<Press>>, mut count: ResMut<PressCount>| {
                count.0 += 1;
            })
            .id();
        app.insert_resource(FakeHit { camera, target });
        app.add_systems(PreUpdate, write_fake_hits.in_set(PickingSystems::Backend));
        app.update();
        app.world_mut()
            .entity_mut(window)
            .get_mut::<Window>()
            .unwrap()
            .set_cursor_position(Some(Vec2::new(400.0, 300.0)));
        app.world_mut()
            .write_message(WindowEvent::CursorMoved(CursorMoved {
                window,
                position: Vec2::new(400.0, 300.0),
                delta: None,
            }));
        app.world_mut()
            .write_message(WindowEvent::MouseButtonInput(MouseButtonInput {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                window,
            }));
        let native_touch = TouchInput {
            phase: bevy::input::touch::TouchPhase::Started,
            position: Vec2::new(400.0, 300.0),
            window,
            force: None,
            id: 7,
        };
        app.world_mut().write_message(native_touch);
        app.world_mut()
            .write_message(WindowEvent::TouchInput(native_touch));
        app.update();
        let native_location = app
            .world_mut()
            .query::<&PointerLocation>()
            .single(app.world())
            .unwrap();
        assert!(native_location.location().is_none());
        assert!(
            app.world()
                .get::<Window>(window)
                .unwrap()
                .cursor_position()
                .is_none()
        );
        assert_eq!(app.world().resource::<PressCount>().0, 0);
        assert!(app.world().resource::<Touches>().iter().next().is_none());
        assert!(
            app.world_mut()
                .query::<&PointerId>()
                .iter(app.world())
                .all(PointerId::is_mouse)
        );

        sender
            .send(Input::Line(
                r#"{"sequence":1,"command":{"type":"pointer","action":{"type":"move","surface":null,"position":[20.0,30.0]}}}"#.into(),
            ))
            .unwrap();
        app.update();
        send_command(
            &mut app,
            &sender,
            2,
            r#"{"type":"time","action":{"type":"advance","frames":1,"step_nanoseconds":16666667}}"#,
        );
        let virtual_location = app
            .world_mut()
            .query::<&PointerLocation>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            virtual_location
                .location()
                .map(|location| location.position),
            Some(Vec2::new(20.0, 30.0))
        );
        assert_eq!(output.responses.lock().unwrap()[0].sequence, 1);
    }

    #[derive(Resource)]
    struct FakeHit {
        camera: Entity,
        target: Entity,
    }

    #[derive(Resource, Default)]
    struct PressCount(u32);

    #[derive(Resource, Default)]
    struct KeyboardCapture {
        presses: u32,
        releases: u32,
    }

    #[derive(Resource, Default)]
    struct TextCapture(Vec<String>);

    #[derive(Resource, Default)]
    struct WindowCapture(u32);

    #[derive(Resource, Default, Debug, Eq, PartialEq)]
    struct FrameCapture {
        updates: u32,
        fixed_updates: u32,
        held_updates: u32,
    }

    fn capture_update(keys: Res<ButtonInput<KeyCode>>, mut capture: ResMut<FrameCapture>) {
        capture.updates += 1;
        if keys.pressed(KeyCode::KeyA) {
            capture.held_updates += 1;
        }
    }

    fn capture_fixed_update(mut capture: ResMut<FrameCapture>) {
        capture.fixed_updates += 1;
    }

    fn send_command(app: &mut App, sender: &mpsc::SyncSender<Input>, sequence: u64, command: &str) {
        sender
            .send(Input::Line(format!(
                r#"{{"sequence":{sequence},"command":{command}}}"#
            )))
            .unwrap();
        app.update();
    }

    #[test]
    fn simulation_and_virtual_time_advance_only_for_time_commands() {
        let (mut app, sender, output, _window) = controlled_app();
        app.init_resource::<FrameCapture>()
            .add_systems(Update, capture_update)
            .add_systems(FixedUpdate, capture_fixed_update);
        app.update();

        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<FrameCapture>().updates, 0);
        assert_eq!(app.world().resource::<Clock>().frame_index(), 0);
        assert_eq!(
            app.world().resource::<Time<Virtual>>().elapsed(),
            std::time::Duration::ZERO
        );

        send_command(
            &mut app,
            &sender,
            1,
            r#"{"type":"time","action":{"type":"advance","frames":1,"step_nanoseconds":25000000}}"#,
        );

        assert_eq!(app.world().resource::<FrameCapture>().updates, 1);
        assert_eq!(app.world().resource::<Clock>().frame_index(), 1);
        assert_eq!(
            app.world().resource::<Time<Virtual>>().elapsed(),
            std::time::Duration::from_millis(25)
        );
        assert_eq!(
            output.responses.lock().unwrap()[0].result.as_ref().unwrap()["clock"]["frame_index"],
            1
        );
    }

    #[test]
    fn maximum_step_is_not_clamped_by_bevy_virtual_time() {
        let (mut app, sender, _output, _window) = controlled_app();
        app.update();
        send_command(
            &mut app,
            &sender,
            1,
            r#"{"type":"time","action":{"type":"advance","frames":1,"step_nanoseconds":1000000000}}"#,
        );

        assert_eq!(
            app.world().resource::<Time<Virtual>>().elapsed(),
            std::time::Duration::from_secs(1)
        );
        assert_eq!(
            app.world().resource::<Time<Virtual>>().delta(),
            std::time::Duration::from_secs(1)
        );
    }

    #[test]
    fn rendered_redraw_updates_do_not_run_simulation_schedules() {
        let (mut app, sender, output, _window) = controlled_app_in_mode(RunMode::Rendered);
        app.init_resource::<FrameCapture>()
            .add_systems(Update, capture_update);
        app.update();
        for _ in 0..4 {
            app.update();
        }

        assert_eq!(output.ready.lock().unwrap()[0].mode, RunMode::Rendered);
        assert_eq!(app.world().resource::<FrameCapture>().updates, 0);
        assert_eq!(app.world().resource::<Clock>().frame_index(), 0);
        assert_eq!(
            app.world().resource::<Time<Virtual>>().elapsed(),
            std::time::Duration::ZERO
        );
        assert!(
            !output.ready.lock().unwrap()[0]
                .controls
                .contains(&"screenshot".into())
        );
        sender
            .send(Input::Line(
                r#"{"sequence":1,"command":{"type":"screenshot","path":"capture.png"}}"#.into(),
            ))
            .unwrap();
        app.update();
        assert_eq!(
            output.responses.lock().unwrap()[0]
                .error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("screenshot_capability_unavailable")
        );
    }

    #[test]
    fn invalid_time_commands_do_not_partially_advance_the_clock() {
        let (mut app, sender, output, _window) = controlled_app();
        app.init_resource::<FrameCapture>()
            .add_systems(Update, capture_update);
        app.update();
        for (sequence, action) in [
            (1, r#"{"type":"advance","frames":0,"step_nanoseconds":1}"#),
            (
                2,
                r#"{"type":"advance","frames":10001,"step_nanoseconds":1}"#,
            ),
            (3, r#"{"type":"advance","frames":1,"step_nanoseconds":0}"#),
            (
                4,
                r#"{"type":"advance","frames":1,"step_nanoseconds":1000000001}"#,
            ),
        ] {
            send_command(
                &mut app,
                &sender,
                sequence,
                &format!(r#"{{"type":"time","action":{action}}}"#),
            );
        }

        assert_eq!(app.world().resource::<FrameCapture>().updates, 0);
        assert_eq!(app.world().resource::<Clock>().frame_index(), 0);
        assert_eq!(
            app.world().resource::<Clock>().elapsed(),
            std::time::Duration::ZERO
        );
        assert_eq!(
            output
                .responses
                .lock()
                .unwrap()
                .iter()
                .map(|response| response.error.as_ref().unwrap().code.as_str())
                .collect::<Vec<_>>(),
            [
                "invalid_time_frames",
                "time_frames_too_large",
                "invalid_time_step",
                "time_step_too_large",
            ]
        );
    }

    #[test]
    fn controlled_clocks_are_isolated_between_sessions() {
        let (mut first, first_sender, _first_output, _first_window) = controlled_app();
        let (mut second, _second_sender, _second_output, _second_window) = controlled_app();
        first.update();
        second.update();
        send_command(
            &mut first,
            &first_sender,
            1,
            r#"{"type":"time","action":{"type":"advance","frames":2,"step_nanoseconds":500}}"#,
        );

        assert_eq!(first.world().resource::<Clock>().frame_index(), 2);
        assert_eq!(
            first.world().resource::<Clock>().elapsed(),
            std::time::Duration::from_nanos(1_000)
        );
        assert_eq!(second.world().resource::<Clock>().frame_index(), 0);
        assert_eq!(
            second.world().resource::<Clock>().elapsed(),
            std::time::Duration::ZERO
        );
    }

    #[test]
    fn batched_and_single_frame_advances_have_the_same_result() {
        fn run(frame_batches: &[u32]) -> (FrameCapture, Clock) {
            let (mut app, sender, _output, _window) = controlled_app();
            app.init_resource::<FrameCapture>()
                .add_systems(Update, capture_update)
                .add_systems(FixedUpdate, capture_fixed_update);
            app.world_mut()
                .resource_mut::<Time<Fixed>>()
                .set_timestep(std::time::Duration::from_millis(10));
            app.update();
            for (index, frames) in frame_batches.iter().enumerate() {
                send_command(
                    &mut app,
                    &sender,
                    index as u64 + 1,
                    &format!(
                        r#"{{"type":"time","action":{{"type":"advance","frames":{frames},"step_nanoseconds":25000000}}}}"#
                    ),
                );
            }
            let capture = app.world_mut().remove_resource::<FrameCapture>().unwrap();
            let clock = app.world_mut().remove_resource::<Clock>().unwrap();
            (capture, clock)
        }

        let (batched_capture, batched_clock) = run(&[4]);
        let (single_capture, single_clock) = run(&[1, 1, 1, 1]);
        assert_eq!(batched_capture, single_capture);
        assert_eq!(batched_capture.updates, 4);
        assert_eq!(batched_capture.fixed_updates, 10);
        assert_eq!(batched_clock.frame_index(), single_clock.frame_index());
        assert_eq!(batched_clock.elapsed(), single_clock.elapsed());
    }

    #[test]
    fn held_virtual_keys_are_consumed_on_each_controlled_frame() {
        let (mut app, sender, _output, _window) = controlled_app();
        app.init_resource::<FrameCapture>()
            .add_systems(Update, capture_update);
        app.update();

        send_command(
            &mut app,
            &sender,
            1,
            r#"{"type":"keyboard","action":{"type":"press","key":"a"}}"#,
        );
        assert_eq!(app.world().resource::<FrameCapture>().updates, 0);
        send_command(
            &mut app,
            &sender,
            2,
            r#"{"type":"time","action":{"type":"advance","frames":3,"step_nanoseconds":16666667}}"#,
        );
        assert_eq!(app.world().resource::<FrameCapture>().held_updates, 3);

        send_command(
            &mut app,
            &sender,
            3,
            r#"{"type":"keyboard","action":{"type":"release","key":"a"}}"#,
        );
        send_command(
            &mut app,
            &sender,
            4,
            r#"{"type":"time","action":{"type":"advance","frames":1,"step_nanoseconds":16666667}}"#,
        );
        assert_eq!(app.world().resource::<FrameCapture>().updates, 4);
        assert_eq!(app.world().resource::<FrameCapture>().held_updates, 3);
    }

    fn capture_window(mut input: MessageReader<WindowEvent>, mut capture: ResMut<WindowCapture>) {
        capture.0 += input.read().count() as u32;
    }

    fn capture_text(mut input: MessageReader<Ime>, mut capture: ResMut<TextCapture>) {
        for message in input.read() {
            if let Ime::Commit { value, .. } = message {
                capture.0.push(value.clone());
            }
        }
    }

    fn write_fake_hits(
        hit: Res<FakeHit>,
        pointers: Query<(&PointerId, &PointerLocation)>,
        mut hits: MessageWriter<PointerHits>,
    ) {
        for (pointer, location) in &pointers {
            if location.location().is_some() {
                hits.write(PointerHits::new(
                    *pointer,
                    vec![(hit.target, HitData::new(hit.camera, 0.0, None, None))],
                    0.0,
                ));
            }
        }
    }

    #[test]
    fn native_keyboard_and_text_are_ignored_while_virtual_input_uses_bevy_paths() {
        let (mut app, sender, output, window) = controlled_app();
        app.init_resource::<KeyboardCapture>()
            .init_resource::<TextCapture>()
            .init_resource::<WindowCapture>()
            .add_systems(Update, (capture_text, capture_window));
        let target = app
            .world_mut()
            .spawn(bevy::text::EditableText::default())
            .observe(
                |event: On<FocusedInput<KeyboardInput>>, mut capture: ResMut<KeyboardCapture>| {
                    match event.input.state {
                        ButtonState::Pressed => capture.presses += 1,
                        ButtonState::Released => capture.releases += 1,
                    }
                },
            )
            .id();
        app.update();
        app.world_mut()
            .insert_resource(InputFocus::from_entity(target));

        let native_key = KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: Key::Character("a".into()),
            state: ButtonState::Pressed,
            text: Some("a".into()),
            repeat: false,
            window,
        };
        app.world_mut().write_message(native_key.clone());
        app.world_mut()
            .write_message(WindowEvent::KeyboardInput(native_key));
        app.world_mut().write_message(Ime::Commit {
            window,
            value: "native".into(),
        });
        app.update();
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyA)
        );
        assert_eq!(app.world().resource::<KeyboardCapture>().presses, 0);
        assert!(app.world().resource::<TextCapture>().0.is_empty());
        assert_eq!(app.world().resource::<WindowCapture>().0, 0);

        send_command(
            &mut app,
            &sender,
            1,
            r#"{"type":"keyboard","action":{"type":"press","key":"a"}}"#,
        );
        send_command(
            &mut app,
            &sender,
            2,
            r#"{"type":"time","action":{"type":"advance","frames":1,"step_nanoseconds":16666667}}"#,
        );
        assert!(
            app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyA)
        );
        app.world_mut().write_message(KeyboardFocusLost);
        app.world_mut()
            .write_message(WindowEvent::KeyboardFocusLost(KeyboardFocusLost));
        app.update();
        assert!(
            app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyA)
        );
        send_command(
            &mut app,
            &sender,
            3,
            r#"{"type":"keyboard","action":{"type":"press","key":"a"}}"#,
        );
        send_command(
            &mut app,
            &sender,
            4,
            r#"{"type":"keyboard","action":{"type":"release","key":"a"}}"#,
        );
        send_command(
            &mut app,
            &sender,
            5,
            r#"{"type":"time","action":{"type":"advance","frames":1,"step_nanoseconds":16666667}}"#,
        );
        send_command(&mut app, &sender, 6, r#"{"type":"text","text":"virtual"}"#);
        send_command(
            &mut app,
            &sender,
            7,
            r#"{"type":"time","action":{"type":"advance","frames":1,"step_nanoseconds":16666667}}"#,
        );
        send_command(
            &mut app,
            &sender,
            8,
            r#"{"type":"observe","selector":{"type":"virtual_input"},"projection":{"type":"summary"},"limit":1}"#,
        );

        let capture = app.world().resource::<KeyboardCapture>();
        assert_eq!(capture.presses, 1);
        assert_eq!(capture.releases, 1);
        assert_eq!(app.world().resource::<TextCapture>().0, ["virtual"]);
        let responses = output.responses.lock().unwrap();
        assert_eq!(
            responses[2].error.as_ref().map(|error| error.code.as_str()),
            Some("key_already_pressed")
        );
        let input = &responses[7].result.as_ref().unwrap()["items"][0];
        assert_eq!(input["keyboard"]["pressed"], json!([]));
        assert_eq!(input["text"]["focused"], json!(crate::Handle::from(target)));
        assert_eq!(input["text"]["last_text"], "virtual");
    }

    #[test]
    fn virtual_input_state_is_isolated_per_controlled_session() {
        let (mut first, first_sender, _first_output, _first_window) = controlled_app();
        let (mut second, _second_sender, _second_output, _second_window) = controlled_app();
        first.update();
        second.update();
        let first_focus = first
            .world_mut()
            .spawn(bevy::text::EditableText::default())
            .id();
        let second_focus = second
            .world_mut()
            .spawn(bevy::text::EditableText::default())
            .id();
        first
            .world_mut()
            .insert_resource(InputFocus::from_entity(first_focus));
        second
            .world_mut()
            .insert_resource(InputFocus::from_entity(second_focus));

        for (sequence, command) in [
            (
                1,
                r#"{"type":"pointer","action":{"type":"move","surface":null,"position":[10.0,20.0]}}"#,
            ),
            (
                2,
                r#"{"type":"pointer","action":{"type":"scroll","delta":[0.0,-2.0]}}"#,
            ),
            (
                3,
                r#"{"type":"keyboard","action":{"type":"press","key":"a"}}"#,
            ),
            (4, r#"{"type":"text","text":"first only"}"#),
        ] {
            first_sender
                .send(Input::Line(format!(
                    r#"{{"sequence":{sequence},"command":{command}}}"#
                )))
                .unwrap();
            first.update();
            second.update();
        }

        assert_eq!(
            first.world().resource::<PointerState>().position,
            Some([10.0, 20.0])
        );
        assert_eq!(
            first.world().resource::<PointerState>().scroll_delta,
            [0.0, -2.0]
        );
        assert_eq!(second.world().resource::<PointerState>().position, None);
        assert_eq!(
            second.world().resource::<PointerState>().scroll_delta,
            [0.0, 0.0]
        );
        assert!(
            first
                .world()
                .resource::<KeyboardState>()
                .is_pressed(&crate::keyboard::Key::A)
        );
        assert!(
            !second
                .world()
                .resource::<KeyboardState>()
                .is_pressed(&crate::keyboard::Key::A)
        );
        assert_eq!(
            first
                .world()
                .resource::<TextState>()
                .observation(first.world())["last_text"],
            "first only"
        );
        assert_eq!(
            second
                .world()
                .resource::<TextState>()
                .observation(second.world())["last_text"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn virtual_move_press_and_release_traverse_picking_and_trigger_an_observer() {
        let (mut app, sender, output, _window) = controlled_app();
        app.init_resource::<PressCount>();
        let camera = app.world_mut().spawn_empty().id();
        let target = app
            .world_mut()
            .spawn(Pickable::default())
            .observe(|_: On<Pointer<Press>>, mut count: ResMut<PressCount>| {
                count.0 += 1;
            })
            .id();
        app.insert_resource(FakeHit { camera, target });
        app.add_systems(PreUpdate, write_fake_hits.in_set(PickingSystems::Backend));
        app.update();

        for (sequence, action) in [
            (
                1,
                r#"{"type":"move","surface":null,"position":[20.0,30.0]}"#,
            ),
            (3, r#"{"type":"press","button":"primary"}"#),
            (5, r#"{"type":"release","button":"primary"}"#),
        ] {
            send_command(
                &mut app,
                &sender,
                sequence,
                &format!(r#"{{"type":"pointer","action":{action}}}"#),
            );
            send_command(
                &mut app,
                &sender,
                sequence + 1,
                r#"{"type":"time","action":{"type":"advance","frames":1,"step_nanoseconds":16666667}}"#,
            );
        }

        assert_eq!(app.world().resource::<PressCount>().0, 1);
        assert_eq!(
            output
                .responses
                .lock()
                .unwrap()
                .iter()
                .map(|response| response.sequence)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6]
        );
    }
}
