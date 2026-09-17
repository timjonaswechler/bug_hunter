use super::protocol::{self, Diagnostic, Message};
use crate::command::{Command, tick::warp};
use bevy::{
    app::{AppExit, MainScheduleOrder},
    ecs::schedule::{InternedScheduleLabel, ScheduleLabel},
    prelude::*,
};
use std::{
    io::{BufRead, Write},
    sync::{Mutex, mpsc},
    time::Instant,
};

/// Experimental v3 bridge. Install after the application's simulation plugins.
/// It preserves the configured schedule order and time policy; only a warp runs that order.
/// Native mouse, keyboard, touch and IME input is discarded in controlled sessions.
/// Enable the `ui` feature for applications using Bevy UI's `Interaction` components.
#[derive(Default)]
pub struct Plugin;

#[derive(Resource)]
struct Bridge {
    input: Mutex<mpsc::Receiver<String>>,
    output: mpsc::Sender<(Message, Option<mpsc::Sender<()>>)>,
    schedules: Vec<InternedScheduleLabel>,
    pace: warp::Pace,
    active: Option<Active>,
    ticks: u64,
    pointer: super::input::pointer::State,
    keyboard: super::input::keyboard::State,
    text: super::input::text::State,
    input_gate: super::input::Gate,
}

struct Active {
    id: u64,
    requested: u64,
    executed: u64,
    pace: warp::Pace,
    last_tick: Option<Instant>,
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct Control;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        let root = std::env::var_os("WOODPECKER_ARTIFACT_DIR")
            .expect("session requires WOODPECKER_ARTIFACT_DIR");
        assert!(
            std::path::Path::new(&root).is_dir(),
            "invalid artifact directory"
        );
        let pace = std::env::var("WOODPECKER_TICK_PACE")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let (input, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::stdin().lock().lines() {
                match line {
                    Ok(line) => {
                        if input.send(line).is_err() {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        });
        let (output, messages) = mpsc::channel::<(Message, Option<mpsc::Sender<()>>)>();
        std::thread::spawn(move || {
            let mut stdout = std::io::stdout().lock();
            for (message, flushed) in messages {
                if serde_json::to_writer(&mut stdout, &message).is_err()
                    || stdout.write_all(b"\n").is_err()
                    || stdout.flush().is_err()
                {
                    break;
                }
                if let Some(flushed) = flushed {
                    let _ = flushed.send(());
                }
            }
        });
        install(app, rx, output, pace);
    }

    fn finish(&self, app: &mut App) {
        // Startup schedules precede the first control iteration, so Ready is sent there.
        app.add_systems(PostStartup, ready);
    }
}

fn install(
    app: &mut App,
    input: mpsc::Receiver<String>,
    output: mpsc::Sender<(Message, Option<mpsc::Sender<()>>)>,
    pace: warp::Pace,
) {
    super::input::install(app);
    let schedules = {
        let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
        std::mem::replace(&mut order.labels, vec![Control.intern()])
    };
    app.insert_resource(Bridge {
        input: Mutex::new(input),
        output,
        schedules,
        pace,
        active: None,
        ticks: 0,
        pointer: Default::default(),
        keyboard: Default::default(),
        text: Default::default(),
        input_gate: Default::default(),
    })
    .add_systems(Control, run);
}

fn ready(bridge: Res<Bridge>) {
    let _ = bridge.output.send((
        Message::Ready {
            version: protocol::VERSION,
            capabilities: protocol::Capabilities { screenshot: false },
        },
        None,
    ));
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut bridge: Mut<Bridge>| {
        bridge.input_gate.capture(world);
        // Bound both command intake and tick execution so neither can starve the other.
        for _ in 0..64 {
            let line = bridge.input.lock().unwrap().try_recv();
            match line {
                Ok(line) => {
                    let decoded = protocol::decode(&line);
                    let response = match decoded {
                        Ok((id, command)) if bridge.active.as_ref().is_some_and(|a| a.id == id) => {
                            Some(Message::ProtocolError {
                                request_id: Some(id),
                                error: Diagnostic::new("duplicate_request_id", command.name()),
                            })
                        }
                        Ok((id, command)) => dispatch(world, &mut bridge, id, command),
                        Err(message) => Some(message),
                    };
                    if let Some(response) = response {
                        let _ = bridge.output.send((response, None));
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    world.write_message(AppExit::Success);
                    return;
                }
            }
        }
        let budget = Instant::now();
        for _ in 0..64 {
            let Some(active) = bridge.active.as_ref() else {
                break;
            };
            if let warp::Pace::TicksPerSecond { target } = active.pace
                && active
                    .last_tick
                    .is_some_and(|t| t.elapsed().as_secs_f64() < 1.0 / target as f64)
            {
                break;
            }
            bridge.input_gate.capture(world);
            bridge.input_gate.swap_window_events(world);
            bridge.pointer.flush(world);
            bridge.keyboard.flush(world);
            bridge.text.flush(world);
            for label in &bridge.schedules {
                let _ = world.try_run_schedule(*label);
            }
            bridge.input_gate.swap_window_events(world);
            bridge.ticks = bridge.ticks.checked_add(1).expect("tick counter exhausted");
            let active = bridge.active.as_mut().unwrap();
            active.executed += 1;
            active.last_tick = Some(Instant::now());
            if active.executed == active.requested {
                finish(&mut bridge, warp::Outcome::Completed);
                break;
            }
            if budget.elapsed().as_millis() >= 2 {
                break;
            }
        }
    });
    // Rendering continues between warps. Its bounded clock channel must be drained
    // even when the simulation's time_system does not run. Never update Time here.
    if let Some(receiver) = world.get_resource::<bevy::time::TimeReceiver>() {
        for _ in receiver.0.try_iter() {}
    }
}

fn finish(bridge: &mut Bridge, outcome: warp::Outcome) {
    if let Some(active) = bridge.active.take() {
        let _ = bridge.output.send((
            protocol::completed(
                active.id,
                "tick.warp.start",
                warp::Completion {
                    requested_ticks: active.requested,
                    executed_ticks: active.executed,
                    outcome,
                },
            ),
            None,
        ));
    }
}

fn dispatch(world: &mut World, bridge: &mut Bridge, id: u64, command: Command) -> Option<Message> {
    let name = command.name();
    let reject = |code, message: &str| {
        Some(Message::Rejected {
            request_id: id,
            command: name.into(),
            error: Diagnostic::new(code, message),
        })
    };
    let input_response = |result| {
        Some(match result {
            Ok(()) => protocol::completed(id, name, ()),
            Err(error) => Message::Rejected {
                request_id: id,
                command: name.into(),
                error,
            },
        })
    };
    let value = match command {
        Command::TextInput(input) => {
            return input_response(bridge.text.input(world, input));
        }
        Command::KeyboardPress(input) => {
            return input_response(bridge.keyboard.key(world, &input.key, true));
        }
        Command::KeyboardRelease(input) => {
            return input_response(bridge.keyboard.key(world, &input.key, false));
        }
        Command::PointerMoveTo(input) => {
            return input_response(bridge.pointer.move_to(world, input.position));
        }
        Command::PointerMoveBy(input) => {
            return input_response(bridge.pointer.move_by(world, input.delta));
        }
        Command::PointerPress(input) => {
            return input_response(bridge.pointer.button(world, &input.button, true));
        }
        Command::PointerRelease(input) => {
            return input_response(bridge.pointer.button(world, &input.button, false));
        }
        Command::PointerScroll(input) => {
            return input_response(bridge.pointer.scroll(world, input.delta));
        }
        Command::Start(start) => {
            if start.ticks == 0 {
                return reject("invalid_tick_count", "ticks must be positive");
            }
            let pace = start.pace.unwrap_or_else(|| bridge.pace.clone());
            if !pace.is_valid() {
                return reject("invalid_pace", "pace must be finite and positive");
            }
            if bridge.active.is_some() {
                return reject("warp_already_running", "warp is active");
            }
            bridge.active = Some(Active {
                id,
                requested: start.ticks,
                executed: 0,
                pace,
                last_tick: None,
            });
            return None;
        }
        Command::SetPace(change) => {
            if !change.pace.is_valid() {
                return reject("invalid_pace", "pace must be finite and positive");
            }
            bridge.pace = change.pace.clone();
            if let Some(active) = &mut bridge.active {
                active.pace = change.pace.clone();
                active.last_tick = None;
            }
            serde_json::json!({"pace": change.pace})
        }
        Command::Stop(_) => {
            let was_running = bridge.active.is_some();
            finish(bridge, warp::Outcome::Stopped);
            serde_json::json!({"was_running": was_running})
        }
        Command::Inspect(query) => match super::inspect::query(world, query) {
            Ok(output) => serde_json::to_value(output).unwrap(),
            Err(error) => {
                return Some(Message::Rejected {
                    request_id: id,
                    command: name.into(),
                    error,
                });
            }
        },
        Command::Shutdown(_) => {
            // All output uses one writer; shutdown waits for its flush acknowledgement.
            let response = protocol::completed(id, name, ());
            let (tx, rx) = mpsc::channel();
            let _ = bridge.output.send((response, Some(tx)));
            let _ = rx.recv();
            world.write_message(AppExit::Success);
            return None;
        }
    };
    Some(protocol::completed(id, name, value))
}

#[cfg(test)]
mod input_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[derive(Resource, Reflect, Default)]
    #[reflect(Resource)]
    struct Counter {
        ticks: u64,
    }

    #[test]
    fn pointer_commands_wait_for_ticks_and_preserve_button_lifetimes() {
        use crate::command::input::pointer;
        use bevy::{
            input::mouse::MouseButtonInput,
            window::{CursorMoved, PrimaryWindow, WindowEvent},
        };
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let (tx, input) = mpsc::channel();
        let (output, rx) = mpsc::channel();
        install(&mut app, input, output, warp::Pace::AsFastAsPossible);
        let send = |id, command: Command| tx.send(protocol::encode(id, &command)).unwrap();
        send(
            1,
            pointer::MoveTo {
                position: [12., 17.],
            }
            .into(),
        );
        send(
            2,
            pointer::Press {
                button: "left".into(),
            }
            .into(),
        );
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(rx.try_iter().count(), 2);
        assert!(
            app.world()
                .get::<Window>(window)
                .unwrap()
                .cursor_position()
                .is_none()
        );
        assert!(
            !app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        assert!(app.world().resource::<Messages<WindowEvent>>().is_empty());
        assert!(
            app.world()
                .resource::<Messages<MouseButtonInput>>()
                .is_empty()
        );
        let warp = |id| {
            send(
                id,
                warp::Start {
                    ticks: 1,
                    pace: None,
                }
                .into(),
            )
        };
        warp(3);
        app.update();
        assert!(
            app.world()
                .resource::<ButtonInput<MouseButton>>()
                .just_pressed(MouseButton::Left)
        );
        assert_eq!(
            app.world().get::<Window>(window).unwrap().cursor_position(),
            None
        );
        assert_eq!(app.world().resource::<Messages<CursorMoved>>().len(), 0);
        assert_eq!(app.world().resource::<Messages<WindowEvent>>().len(), 0);
        warp(4);
        app.update();
        let buttons = app.world().resource::<ButtonInput<MouseButton>>();
        assert!(buttons.pressed(MouseButton::Left));
        assert!(!buttons.just_pressed(MouseButton::Left));
        send(
            5,
            pointer::Release {
                button: "left".into(),
            }
            .into(),
        );
        app.update();
        assert!(
            app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        warp(6);
        app.update();
        assert!(
            app.world()
                .resource::<ButtonInput<MouseButton>>()
                .just_released(MouseButton::Left)
        );
        assert!(
            !app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
    }

    #[test]
    fn idle_control_drains_render_clock_without_advancing_time() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let (sender, receiver) = bevy::time::create_time_channels();
        app.insert_resource(receiver);
        let (_tx, input) = mpsc::channel();
        let (output, _rx) = mpsc::channel();
        install(&mut app, input, output, warp::Pace::AsFastAsPossible);
        for _ in 0..10 {
            sender
                .0
                .try_send(bevy::platform::time::Instant::now())
                .unwrap();
            app.update();
            assert_eq!(
                app.world().resource::<Time<Real>>().elapsed(),
                Duration::ZERO
            );
        }
    }

    #[test]
    fn entity_inspect_uses_the_command_bridge_without_ticks_or_time_progress() {
        use crate::command::inspect::{self, component, entity};
        #[derive(Component, Reflect)]
        #[reflect(Component)]
        struct State(u32);

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .register_type::<State>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                17,
            )))
            .add_systems(Update, |mut states: Query<&mut State>| {
                for mut state in &mut states {
                    state.0 += 1;
                }
            });
        let entity = app.world_mut().spawn((Name::new("button"), State(0))).id();
        let (tx, input) = mpsc::channel();
        let (output, rx) = mpsc::channel();
        install(&mut app, input, output, warp::Pace::AsFastAsPossible);
        let request = inspect::Command::Entities {
            entity: Some(entity.into()),
            with: vec![],
            without: vec![],
            projection: entity::Projection::Components {
                selection: component::Selection::Listed {
                    type_paths: vec![State::type_path().into()],
                },
            },
        };
        for id in 1..=2 {
            let before = app.world().resource::<Time<Real>>().elapsed();
            let change_tick = app
                .world()
                .entity(entity)
                .get_change_ticks::<State>()
                .unwrap()
                .changed;
            tx.send(protocol::encode(id, &request.clone().into()))
                .unwrap();
            app.update();
            let Message::Completed {
                request_id,
                command,
                output,
            } = rx.try_recv().unwrap().0
            else {
                panic!("inspect must complete");
            };
            assert_eq!(request_id, id);
            assert_eq!(command, "inspect.query");
            assert_eq!(
                output["items"][0]["result"]["components"][0]["value"]["value"],
                id - 1
            );
            assert_eq!(app.world().resource::<Time<Real>>().elapsed(), before);
            assert_eq!(app.world().get::<State>(entity).unwrap().0, (id - 1) as u32);
            // The control schedule itself can advance the world's change tick, but
            // Inspect never marks the application component as changed.
            assert_eq!(
                app.world()
                    .entity(entity)
                    .get_change_ticks::<State>()
                    .unwrap()
                    .changed,
                change_tick
            );
            if id == 1 {
                tx.send(protocol::encode(
                    3,
                    &warp::Start {
                        ticks: 1,
                        pace: None,
                    }
                    .into(),
                ))
                .unwrap();
                app.update();
                assert!(matches!(
                    rx.try_recv().unwrap().0,
                    Message::Completed { request_id: 3, .. }
                ));
            }
        }
    }

    #[test]
    fn only_warps_run_configured_schedules_and_controls_remain_reachable() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                17,
            )))
            .init_resource::<Counter>()
            .register_type::<Counter>()
            .add_systems(Update, |mut counter: ResMut<Counter>| counter.ticks += 1);
        let (tx, input) = mpsc::channel();
        let (output, rx) = mpsc::channel();
        install(&mut app, input, output, warp::Pace::AsFastAsPossible);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(app.world().resource::<Counter>().ticks, 0);
        let query = crate::command::inspect::Command::Resources {
            selector: crate::command::inspect::Selector::All {},
            projection: crate::command::inspect::Projection::Value {},
        };
        tx.send(protocol::encode(1, &query.into())).unwrap();
        app.update();
        assert!(matches!(
            rx.try_recv().unwrap().0,
            Message::Completed { request_id: 1, .. }
        ));
        assert_eq!(app.world().resource::<Counter>().ticks, 0);

        tx.send(protocol::encode(
            2,
            &warp::Start {
                ticks: 3,
                pace: None,
            }
            .into(),
        ))
        .unwrap();
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<Counter>().ticks, 3);
        assert!(
            matches!(rx.try_recv().unwrap().0, Message::Completed { request_id: 2, output, .. }
            if output["executed_ticks"] == 3)
        );
        assert_eq!(
            app.world().resource::<Time<Real>>().delta(),
            Duration::from_millis(17)
        );
        let time = app.world().resource::<Time<Real>>().elapsed();
        app.update();
        assert_eq!(app.world().resource::<Time<Real>>().elapsed(), time);

        tx.send(protocol::encode(
            3,
            &warp::Start {
                ticks: u64::MAX,
                pace: None,
            }
            .into(),
        ))
        .unwrap();
        app.update();
        assert!(app.world().resource::<Counter>().ticks <= 67);
        tx.send(protocol::encode(
            4,
            &warp::SetPace {
                pace: warp::Pace::TicksPerSecond { target: 0.1 },
            }
            .into(),
        ))
        .unwrap();
        tx.send(protocol::encode(5, &warp::Stop {}.into())).unwrap();
        app.update();
        let messages: Vec<_> = rx.try_iter().map(|(message, _)| message).collect();
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::Completed { request_id: 4, .. }))
        );
        assert!(messages.iter().any(
            |m| matches!(m, Message::Completed { request_id: 3, output, .. }
            if output["outcome"] == "stopped")
        ));
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::Completed { request_id: 5, .. }))
        );
        let ticks = app.world().resource::<Counter>().ticks;
        app.update();
        assert_eq!(app.world().resource::<Counter>().ticks, ticks);
    }
}
