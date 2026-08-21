use automation_control::{
    Command, Handle, Response, RunMode,
    driver::{
        DriverError, LaunchSpec, LaunchTargetKind, Session, SessionOptions, wait::FrameLimit,
    },
    keyboard::{Command as KeyboardCommand, Key},
    observation::{Projection, Request as ObservationRequest, Selector},
    pointer::{Button, Command as PointerCommand},
    screenshot::Command as ScreenshotCommand,
    time::Command as TimeCommand,
};
use serde_json::Value;
use std::sync::Mutex;

static SESSION_TEST: Mutex<()> = Mutex::new(());

fn spawn_logical_state() -> Result<Session, Box<dyn std::error::Error>> {
    let launch = LaunchSpec {
        package: "bevy_test_apps".into(),
        kind: LaunchTargetKind::Binary,
        target: "logical_state".into(),
        features: vec!["automation".into()],
        arguments: vec![],
    };
    let mut command = launch.command();
    for variable in ["DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET"] {
        command.env_remove(variable);
    }
    Ok(Session::spawn_command(command, SessionOptions::new())?)
}

#[test]
fn logical_state_starts_without_a_display_or_screenshot_capability() {
    let _guard = session_test_guard();
    let mut session = spawn_logical_state().unwrap();
    let ready = session.ready().unwrap();

    assert_eq!(ready.mode, RunMode::Logical);
    assert_eq!(ready.controls, ["pointer", "keyboard", "text", "time"]);
    assert!(!ready.controls.iter().any(|control| control == "screenshot"));
    let error = session
        .request(Command::Screenshot(ScreenshotCommand::new("logical.png")))
        .unwrap_err();
    match error {
        DriverError::RequestFailed(response) => assert_eq!(
            response.error.unwrap().code,
            "screenshot_capability_unavailable"
        ),
        other => panic!("unexpected screenshot error: {other}"),
    }

    session.shutdown().unwrap();
}

#[test]
fn logical_state_schedules_advance_only_through_the_controlled_clock() {
    let _guard = session_test_guard();
    let mut session = spawn_logical_state().unwrap();
    session.ready().unwrap();
    let handle = target_handle(&mut session, "logical-state");

    let initial = session_observation(&mut session, handle);
    assert_eq!(initial["updates"], 0);
    assert_eq!(initial["fixed_updates"], 0);
    assert_eq!(initial["timer_finishes"], 0);
    assert_eq!(session_observation(&mut session, handle), initial);

    advance(&mut session, TimeCommand::advance(2, 25_000_000));
    let advanced = session_observation(&mut session, handle);
    assert_eq!(advanced["updates"], 2);
    assert_eq!(advanced["fixed_updates"], 5);
    assert_eq!(advanced["timer_finishes"], 1);
    assert_eq!(session_observation(&mut session, handle), advanced);

    session.shutdown().unwrap();
}

#[test]
fn virtual_pointer_and_keyboard_change_state_through_application_systems() {
    let _guard = session_test_guard();
    let mut session = spawn_logical_state().unwrap();
    session.ready().unwrap();
    advance(&mut session, TimeCommand::advance(1, 10_000_000));

    let button = target_summary(&mut session, "logical-button");
    assert_eq!(button["bounds"]["x"], 220.0);
    assert_eq!(button["bounds"]["y"], 130.0);
    assert_eq!(button["bounds"]["width"], 200.0);
    assert_eq!(button["bounds"]["height"], 100.0);
    session
        .request(Command::Pointer(PointerCommand::Move {
            surface: None,
            position: [320.0, 180.0],
        }))
        .unwrap();
    advance(&mut session, TimeCommand::advance(1, 10_000_000));
    session
        .request(Command::Pointer(PointerCommand::Press {
            button: Button::Primary,
        }))
        .unwrap();
    advance(&mut session, TimeCommand::advance(1, 10_000_000));
    session
        .request(Command::Pointer(PointerCommand::Release {
            button: Button::Primary,
        }))
        .unwrap();
    advance(&mut session, TimeCommand::advance(1, 10_000_000));

    session
        .request(Command::Keyboard(KeyboardCommand::Press { key: Key::A }))
        .unwrap();
    advance(&mut session, TimeCommand::advance(2, 10_000_000));
    let handle = target_handle(&mut session, "logical-state");
    let pressed = session_observation(&mut session, handle);
    assert_eq!(pressed["pointer_presses"], 1);
    assert_eq!(pressed["key_a_held"], true);
    assert_eq!(pressed["key_a_presses"], 1);
    assert_eq!(pressed["key_a_releases"], 0);

    session
        .request(Command::Keyboard(KeyboardCommand::Release { key: Key::A }))
        .unwrap();
    advance(&mut session, TimeCommand::advance(1, 10_000_000));
    let released = session_observation(&mut session, handle);
    assert_eq!(released["key_a_held"], false);
    assert_eq!(released["key_a_presses"], 1);
    assert_eq!(released["key_a_releases"], 1);

    session.shutdown().unwrap();
}

#[test]
fn host_waits_are_bounded_observe_and_advance_loops() {
    let _guard = session_test_guard();
    let mut session = spawn_logical_state().unwrap();
    session.ready().unwrap();
    let targets = ObservationRequest::new(Selector::Targets, Projection::Summary);

    let ready = session
        .wait_for_observation(
            targets.clone(),
            FrameLimit::new(2, 10_000_000).unwrap(),
            |result| {
                result["items"].as_array().is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item["name"] == "logical-button" && !item["bounds"].is_null())
                })
            },
        )
        .unwrap();
    assert!(
        ready.result.unwrap()["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["name"] == "logical-button")
    );

    let error = session
        .wait_for_observation(targets, FrameLimit::new(2, 10_000_000).unwrap(), |_| false)
        .unwrap_err();
    match error {
        DriverError::WaitLimitReached {
            frame_limit,
            last_observation,
        } => {
            assert_eq!(frame_limit, 2);
            assert!(last_observation["items"].is_array());
        }
        other => panic!("unexpected wait error: {other}"),
    }

    session.shutdown().unwrap();
}

#[test]
fn identical_action_sequences_produce_identical_observations() {
    let _guard = session_test_guard();
    assert_eq!(deterministic_observation(), deterministic_observation());
}

fn deterministic_observation() -> Value {
    let mut session = spawn_logical_state().unwrap();
    session.ready().unwrap();
    let handle = target_handle(&mut session, "logical-state");
    session
        .request(Command::Keyboard(KeyboardCommand::Press { key: Key::A }))
        .unwrap();
    advance(&mut session, TimeCommand::advance(2, 25_000_000));
    session
        .request(Command::Keyboard(KeyboardCommand::Release { key: Key::A }))
        .unwrap();
    advance(&mut session, TimeCommand::advance(2, 25_000_000));
    let observation = session_observation(&mut session, handle);
    session.shutdown().unwrap();
    observation
}

fn session_test_guard() -> std::sync::MutexGuard<'static, ()> {
    SESSION_TEST
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn advance(session: &mut Session, command: TimeCommand) {
    session.request(Command::Time(command)).unwrap();
}

fn target_summary(session: &mut Session, name: &str) -> Value {
    let response = session
        .request(Command::Observe(ObservationRequest::new(
            Selector::Targets,
            Projection::Summary,
        )))
        .unwrap();
    observation_items(response)
        .into_iter()
        .find(|item| item["name"] == name)
        .unwrap()
}

fn observation_items(response: Response) -> Vec<Value> {
    response
        .result
        .and_then(|result| result.get("items").and_then(Value::as_array).cloned())
        .expect("observation response should contain items")
}

fn target_handle(session: &mut Session, name: &str) -> Handle {
    serde_json::from_value(target_summary(session, name)["entity"].clone()).unwrap()
}

fn session_observation(session: &mut Session, handle: Handle) -> Value {
    const STATE_PATH: &str = "logical_state::SessionObservation";
    let response = session
        .request(Command::Observe(ObservationRequest::new(
            Selector::Entity(handle),
            Projection::Components {
                type_paths: vec![STATE_PATH.into()],
            },
        )))
        .unwrap();
    let item = observation_items(response)
        .into_iter()
        .next()
        .expect("state observation should contain one item");
    let component = &item["components"][STATE_PATH];
    assert_eq!(component["status"], "available");
    component["value"].clone()
}
