//! Controller smoke test for the rendered Controlled Session.
use automation_control::{
    Command, Handle,
    driver::{LaunchSpec, LaunchTargetKind, Session, SessionOptions},
    keyboard::{Command as KeyboardCommand, Key},
    observation::{Projection, Request as ObservationRequest, Selector},
    pointer::{Button, Command as PointerCommand},
    screenshot::Command as ScreenshotCommand,
    text::Command as TextCommand,
    time::Command as TimeCommand,
};
use serde_json::Value;
use std::{
    f32::consts::TAU,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Options {
    duration: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Bounds {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run(parse_options(std::env::args().skip(1))?)
}

fn run(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    let launch = LaunchSpec {
        package: "bevy_test_apps".into(),
        kind: LaunchTargetKind::Binary,
        target: "context_menu".into(),
        features: vec!["automation".into()],
        arguments: vec![],
    };
    let artifact_root = temporary_artifact_root();
    fs::create_dir_all(&artifact_root)?;
    let mut session = Session::spawn(
        &launch,
        SessionOptions::new().with_artifact_dir(&artifact_root),
    )?;
    let ready = session.ready()?;
    assert_eq!(ready.version, 2);
    assert_eq!(ready.mode, automation_control::RunMode::Rendered);
    assert_eq!(
        ready.controls,
        ["pointer", "keyboard", "text", "time", "screenshot"]
    );
    assert!(ready.observation_scopes.contains(&"virtual_input".into()));
    assert!(ready.observation_scopes.contains(&"clock".into()));
    advance(&mut session)?;

    let targets = observe(&mut session, Selector::Targets)?;
    let button = find_named(&targets, "button")?;
    let background = find_named(&targets, "background")?;
    let background_handle: Handle = serde_json::from_value(background["entity"].clone())?;
    let text_input = find_named(&targets, "text-input")?;
    let text_input_handle: Handle = serde_json::from_value(text_input["entity"].clone())?;
    let text_input_center = center(&text_input["bounds"])?;
    let background_bounds = Bounds::from_value(&background["bounds"])?;
    let button_center = center(&button["bounds"])?;

    click(&mut session, button_center)?;

    let menu = observe(&mut session, Selector::Targets)?;
    let item = find_named(&menu, "item-fuchsia")?;
    let item_center = center(&item["bounds"])?;
    click(&mut session, item_center)?;
    pointer(&mut session, PointerCommand::Scroll { delta: [0.0, -2.0] })?;

    session.request(Command::Keyboard(KeyboardCommand::Press { key: Key::A }))?;
    advance(&mut session)?;
    let held = observe_session_state(&mut session, background_handle)?;
    assert_eq!(held["key_a_held"], true);
    assert_eq!(held["key_a_presses"], 1);

    let input = observe_virtual_input(&mut session)?;
    assert_eq!(
        input["pointer"]["scroll_delta"],
        serde_json::json!([0.0, -2.0])
    );
    assert_eq!(input["keyboard"]["pressed"], serde_json::json!(["a"]));

    session.request(Command::Keyboard(KeyboardCommand::Release { key: Key::A }))?;
    advance(&mut session)?;
    click(&mut session, text_input_center)?;
    session.request(Command::Text(TextCommand::new("controlled text")))?;
    advance(&mut session)?;
    advance(&mut session)?;

    let input = observe_virtual_input(&mut session)?;
    assert_eq!(
        input["text"]["focused"],
        serde_json::json!(text_input_handle)
    );
    assert_eq!(input["text"]["last_text"], "controlled text");

    let state = session.request(Command::Observe(ObservationRequest::new(
        Selector::Entity(background_handle),
        Projection::Components {
            type_paths: vec!["context_menu::SessionState".into()],
        },
    )))?;
    let value =
        &state.result.as_ref().unwrap()["items"][0]["components"]["context_menu::SessionState"];
    assert_eq!(value["status"], "available");
    assert_eq!(value["value"]["menu_open"], false);
    assert_eq!(value["value"]["selected_item"], "fuchsia");
    assert_eq!(value["value"]["key_a_held"], false);
    assert_eq!(value["value"]["key_a_releases"], 1);
    assert_eq!(value["value"]["text"], "controlled text");

    let first = capture_screenshot(&mut session, &artifact_root, "captures/first.png")?;
    let second = capture_screenshot(&mut session, &artifact_root, "captures/second.png")?;
    assert_eq!(first.dimensions(), (640, 360));
    assert_eq!(second.dimensions(), first.dimensions());
    assert_fuchsia_background(&first)?;
    assert_fuchsia_background(&second)?;

    move_pointer_in_circles(&mut session, background_bounds, options.duration)?;
    session.shutdown()?;
    fs::remove_dir_all(artifact_root).ok();
    Ok(())
}

fn temporary_artifact_root() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "automation-control-rendered-screenshot-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn capture_screenshot(
    session: &mut Session,
    artifact_root: &Path,
    relative_path: &str,
) -> Result<image::RgbImage, Box<dyn std::error::Error>> {
    let response = session.request(Command::Screenshot(ScreenshotCommand::new(relative_path)))?;
    let artifact = &response
        .result
        .as_ref()
        .ok_or("screenshot result missing")?["artifact"];
    assert_eq!(artifact["type"], "screenshot");
    assert_eq!(artifact["path"], relative_path);
    assert_eq!(artifact["mime_type"], "image/png");
    assert_eq!(artifact["width"], 640);
    assert_eq!(artifact["height"], 360);

    let path = artifact_root.join(relative_path);
    let bytes = fs::read(&path)?;
    assert!(bytes.len() > 8);
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    Ok(image::open(path)?.to_rgb8())
}

fn assert_fuchsia_background(image: &image::RgbImage) -> Result<(), Box<dyn std::error::Error>> {
    let pixel = image.get_pixel(5, 5).0;
    if pixel[0] < 180 || pixel[1] > 100 || pixel[2] < 180 {
        return Err(format!("expected fuchsia background, got RGB {pixel:?}").into());
    }
    Ok(())
}

fn parse_options<I, S>(args: I) -> Result<Options, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter();
    let mut duration = None;
    while let Some(argument) = args.next() {
        match argument.as_ref() {
            "--circle-seconds" => {
                if duration.is_some() {
                    return Err("--circle-seconds may only be supplied once".into());
                }
                let seconds = args
                    .next()
                    .ok_or("--circle-seconds requires a positive integer")?
                    .as_ref()
                    .parse::<u64>()
                    .map_err(|_| "--circle-seconds requires a positive integer")?;
                if seconds == 0 {
                    return Err("--circle-seconds requires a positive integer".into());
                }
                duration = Some(Duration::from_secs(seconds));
            }
            unknown => return Err(format!("unknown argument {unknown:?}")),
        }
    }
    Ok(Options {
        duration: duration.unwrap_or_default(),
    })
}

impl Bounds {
    fn from_value(value: &Value) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            x: value["x"].as_f64().ok_or("bounds.x missing")? as f32,
            y: value["y"].as_f64().ok_or("bounds.y missing")? as f32,
            width: value["width"].as_f64().ok_or("bounds.width missing")? as f32,
            height: value["height"].as_f64().ok_or("bounds.height missing")? as f32,
        })
    }
}

fn position_on_circle(bounds: Bounds, elapsed: Duration) -> [f32; 2] {
    let center = [
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    ];
    let radius = bounds.width.min(bounds.height) * 0.3;
    let angle = elapsed.as_secs_f32() * TAU;
    [
        center[0] + radius * angle.cos(),
        center[1] + radius * angle.sin(),
    ]
}

fn move_pointer_in_circles(
    session: &mut Session,
    bounds: Bounds,
    duration: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    if duration.is_zero() {
        return Ok(());
    }
    eprintln!(
        "moving the virtual pointer in circles for {} seconds",
        duration.as_secs()
    );
    let started = Instant::now();
    while started.elapsed() < duration {
        pointer(
            session,
            PointerCommand::Move {
                surface: None,
                position: position_on_circle(bounds, started.elapsed()),
            },
        )?;
        thread::sleep(Duration::from_millis(16));
    }
    eprintln!("virtual pointer circle complete");
    Ok(())
}

fn observe(
    session: &mut Session,
    selector: Selector,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let response = session.request(Command::Observe(ObservationRequest::new(
        selector,
        Projection::Summary,
    )))?;
    Ok(response
        .result
        .and_then(|value| value["items"].as_array().cloned())
        .ok_or("observation did not return items")?)
}

fn pointer(
    session: &mut Session,
    command: PointerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    session.request(Command::Pointer(command))?;
    advance(session)
}

fn advance(session: &mut Session) -> Result<(), Box<dyn std::error::Error>> {
    session.request(Command::Time(TimeCommand::advance(1, 16_666_667)))?;
    Ok(())
}

fn click(session: &mut Session, position: [f32; 2]) -> Result<(), Box<dyn std::error::Error>> {
    pointer(
        session,
        PointerCommand::Move {
            surface: None,
            position,
        },
    )?;
    pointer(
        session,
        PointerCommand::Press {
            button: Button::Primary,
        },
    )?;
    pointer(
        session,
        PointerCommand::Release {
            button: Button::Primary,
        },
    )
}

fn observe_virtual_input(session: &mut Session) -> Result<Value, Box<dyn std::error::Error>> {
    let response = session.request(Command::Observe(ObservationRequest::new(
        Selector::VirtualInput,
        Projection::Summary,
    )))?;
    response
        .result
        .and_then(|value| {
            value["items"]
                .as_array()
                .and_then(|items| items.first())
                .cloned()
        })
        .ok_or_else(|| "virtual input observation did not return an item".into())
}

fn observe_session_state(
    session: &mut Session,
    background: Handle,
) -> Result<Value, Box<dyn std::error::Error>> {
    let response = session.request(Command::Observe(ObservationRequest::new(
        Selector::Entity(background),
        Projection::Components {
            type_paths: vec!["context_menu::SessionState".into()],
        },
    )))?;
    Ok(response.result.ok_or("state observation has no result")?["items"][0]["components"]
        ["context_menu::SessionState"]["value"]
        .clone())
}

fn find_named<'a>(items: &'a [Value], name: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    items
        .iter()
        .find(|item| item["name"] == name)
        .ok_or_else(|| format!("target {name:?} not found in {items:?}").into())
}

fn center(bounds: &Value) -> Result<[f32; 2], Box<dyn std::error::Error>> {
    let x = bounds["x"].as_f64().ok_or("bounds.x missing")? as f32;
    let y = bounds["y"].as_f64().ok_or("bounds.y missing")? as f32;
    let width = bounds["width"].as_f64().ok_or("bounds.width missing")? as f32;
    let height = bounds["height"].as_f64().ok_or("bounds.height missing")? as f32;
    Ok([x + width / 2.0, y + height / 2.0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a display and render adapter"]
    fn rendered_session_writes_a_semantically_stable_screenshot() {
        run(Options::default()).unwrap();
    }

    #[test]
    fn circle_seconds_enables_timed_pointer_motion() {
        let options = parse_options(["--circle-seconds", "60"]).unwrap();
        assert_eq!(options.duration, Duration::from_secs(60));
    }

    #[test]
    fn pointer_completes_one_circle_per_second_inside_the_bounds() {
        let bounds = Bounds {
            x: 10.0,
            y: 20.0,
            width: 200.0,
            height: 100.0,
        };
        assert_eq!(position_on_circle(bounds, Duration::ZERO), [140.0, 70.0]);
        let quarter = position_on_circle(bounds, Duration::from_millis(250));
        assert!((quarter[0] - 110.0).abs() < 0.001);
        assert!((quarter[1] - 100.0).abs() < 0.001);
    }
}
