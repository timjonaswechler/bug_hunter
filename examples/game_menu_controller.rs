//! Controller smoke test for the rendered game-menu Controlled Session.
use automation_control::{
    Command, Handle,
    driver::{DriverError, LaunchSpec, LaunchTargetKind, Session, SessionOptions},
    keyboard::{Command as KeyboardCommand, Key},
    observation::{Projection, Request as ObservationRequest, Selector},
    pointer::{Button, Command as PointerCommand},
    screenshot::Command as ScreenshotCommand,
    time::Command as TimeCommand,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const STATE_PATH: &str = "game_menu::SessionObservation";
const STEP_NANOSECONDS: u64 = 100_000_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run()
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let launch = LaunchSpec {
        package: "bevy_test_apps".into(),
        kind: LaunchTargetKind::Binary,
        target: "game_menu".into(),
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
    for control in ["pointer", "keyboard", "time", "screenshot"] {
        assert!(ready.controls.iter().any(|candidate| candidate == control));
    }

    advance(&mut session, 9)?;
    let targets = targets(&mut session)?;
    let state_handle = handle(named_item(&targets, "game-menu-state")?)?;
    let splash = state(&mut session, state_handle)?;
    assert_eq!(splash["game_state"], "Splash");
    check_close(splash["splash_elapsed_seconds"].as_f64(), 0.9)?;
    assert_eq!(state(&mut session, state_handle)?, splash);

    advance(&mut session, 1)?;
    assert_eq!(state(&mut session, state_handle)?["game_state"], "Splash");
    advance(&mut session, 2)?;
    expect_state(&mut session, state_handle, "Menu", "Main")?;
    capture(&mut session, &artifact_root, "game-menu/main.png")?;

    let old_settings = target_handle(&mut session, "settings-button")?;
    click(&mut session, "settings-button")?;
    expect_state(&mut session, state_handle, "Menu", "Settings")?;
    reject_stale(&mut session, old_settings)?;
    capture(&mut session, &artifact_root, "game-menu/settings.png")?;

    click(&mut session, "display-settings-button")?;
    expect_state(&mut session, state_handle, "Menu", "SettingsDisplay")?;
    capture(
        &mut session,
        &artifact_root,
        "game-menu/display-settings.png",
    )?;
    click(&mut session, "quality-high-button")?;
    assert_eq!(
        state(&mut session, state_handle)?["display_quality"],
        "High"
    );

    tap_key(&mut session, Key::Escape)?;
    expect_state(&mut session, state_handle, "Menu", "Settings")?;
    click(&mut session, "sound-settings-button")?;
    click(&mut session, "volume-3-button")?;
    let configured = state(&mut session, state_handle)?;
    assert_eq!(configured["menu_state"], "SettingsSound");
    assert_eq!(configured["display_quality"], "High");
    assert_eq!(configured["volume"], 3);
    click(&mut session, "sound-back-button")?;
    click(&mut session, "settings-back-button")?;
    expect_state(&mut session, state_handle, "Menu", "Main")?;

    let old_play = target_handle(&mut session, "new-game-button")?;
    click(&mut session, "new-game-button")?;
    expect_state(&mut session, state_handle, "Game", "Disabled")?;
    reject_stale(&mut session, old_play)?;
    let game = state(&mut session, state_handle)?;
    assert_eq!(game["display_quality"], "High");
    assert_eq!(game["volume"], 3);
    let elapsed = game["game_elapsed_seconds"].clone();
    assert_eq!(
        state(&mut session, state_handle)?["game_elapsed_seconds"],
        elapsed
    );
    let game_screen = target_handle(&mut session, "game-screen")?;
    capture(&mut session, &artifact_root, "game-menu/game.png")?;

    advance(&mut session, 40)?;
    assert_eq!(state(&mut session, state_handle)?["game_state"], "Game");
    let before_observations = state(&mut session, state_handle)?["game_elapsed_seconds"].clone();
    assert_eq!(
        state(&mut session, state_handle)?["game_elapsed_seconds"],
        before_observations
    );
    advance(&mut session, 12)?;
    expect_state(&mut session, state_handle, "Menu", "Main")?;
    reject_stale(&mut session, game_screen)?;
    let new_play = target_handle(&mut session, "new-game-button")?;
    assert_ne!(old_play, new_play);

    session.shutdown()?;
    fs::remove_dir_all(artifact_root).ok();
    Ok(())
}

fn temporary_artifact_root() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "automation-control-game-menu-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn advance(session: &mut Session, frames: u64) -> Result<(), Box<dyn std::error::Error>> {
    session.request(Command::Time(TimeCommand::advance(
        frames,
        STEP_NANOSECONDS,
    )))?;
    Ok(())
}

fn tap_key(session: &mut Session, key: Key) -> Result<(), Box<dyn std::error::Error>> {
    session.request(Command::Keyboard(KeyboardCommand::Press {
        key: key.clone(),
    }))?;
    advance(session, 1)?;
    session.request(Command::Keyboard(KeyboardCommand::Release { key }))?;
    advance(session, 1)
}

fn click(session: &mut Session, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let target = targets(session)?;
    let item = named_item(&target, name)?;
    let target_handle = handle(item)?;
    let bounds = &item["bounds"];
    let x = bounds["x"].as_f64().ok_or("button x bound missing")?
        + bounds["width"].as_f64().ok_or("button width missing")? / 2.0;
    let y = bounds["y"].as_f64().ok_or("button y bound missing")?
        + bounds["height"].as_f64().ok_or("button height missing")? / 2.0;
    session.request(Command::Pointer(PointerCommand::Move {
        surface: None,
        position: [x as f32, y as f32],
    }))?;
    advance(session, 1)?;
    let pointers = observe(session, Selector::Pointers, Projection::Summary)?;
    let interactions = pointers
        .first()
        .and_then(|pointer| pointer["interactions"].as_array())
        .ok_or("pointer interactions missing")?;
    let target_value = serde_json::to_value(target_handle)?;
    if !interactions.contains(&target_value) {
        return Err(format!("pointer did not hit {name:?}: {pointers:?}").into());
    }
    session.request(Command::Pointer(PointerCommand::Press {
        button: Button::Primary,
    }))?;
    advance(session, 1)?;
    session.request(Command::Pointer(PointerCommand::Release {
        button: Button::Primary,
    }))?;
    advance(session, 1)?;
    advance(session, 1)
}

fn expect_state(
    session: &mut Session,
    state_handle: Handle,
    game: &str,
    menu: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let value = state(session, state_handle)?;
    assert_eq!(value["game_state"], game);
    assert_eq!(value["menu_state"], menu);
    Ok(())
}

fn state(session: &mut Session, state_handle: Handle) -> Result<Value, Box<dyn std::error::Error>> {
    let item = observe(
        session,
        Selector::Entity(state_handle),
        Projection::Components {
            type_paths: vec![STATE_PATH.into()],
        },
    )?
    .into_iter()
    .next()
    .ok_or("state observation returned no item")?;
    let component = &item["components"][STATE_PATH];
    if component["status"] != "available" {
        return Err(format!("game menu state is unavailable: {component}").into());
    }
    Ok(component["value"].clone())
}

fn targets(session: &mut Session) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    observe(session, Selector::Targets, Projection::Summary)
}

fn observe(
    session: &mut Session,
    selector: Selector,
    projection: Projection,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let response = session.request(Command::Observe(ObservationRequest::new(
        selector, projection,
    )))?;
    response
        .result
        .and_then(|value| value["items"].as_array().cloned())
        .ok_or_else(|| "observation did not return items".into())
}

fn named_item<'a>(items: &'a [Value], name: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    items
        .iter()
        .find(|item| item["name"] == name)
        .ok_or_else(|| format!("target {name:?} not found in {items:?}").into())
}

fn handle(item: &Value) -> Result<Handle, Box<dyn std::error::Error>> {
    Ok(serde_json::from_value(item["entity"].clone())?)
}

fn target_handle(session: &mut Session, name: &str) -> Result<Handle, Box<dyn std::error::Error>> {
    handle(named_item(&targets(session)?, name)?)
}

fn reject_stale(session: &mut Session, stale: Handle) -> Result<(), Box<dyn std::error::Error>> {
    let result = session.request(Command::Observe(ObservationRequest::new(
        Selector::Entity(stale),
        Projection::Summary,
    )));
    match result {
        Err(DriverError::RequestFailed(response)) => {
            let error = response.error.ok_or("stale response has no typed error")?;
            assert_eq!(error.code, "observation_failed");
            assert!(error.message.contains("is not live"));
            Ok(())
        }
        Err(error) => Err(format!("unexpected stale-handle error: {error}").into()),
        Ok(response) => Err(format!("stale handle resolved unexpectedly: {response:?}").into()),
    }
}

fn capture(
    session: &mut Session,
    root: &Path,
    path: &str,
) -> Result<image::RgbImage, Box<dyn std::error::Error>> {
    let response = session.request(Command::Screenshot(ScreenshotCommand::new(path)))?;
    let artifact = &response.result.ok_or("screenshot result missing")?["artifact"];
    assert_eq!(artifact["path"], path);
    assert_eq!(artifact["width"], 800);
    assert_eq!(artifact["height"], 600);
    Ok(image::open(root.join(path))?.to_rgb8())
}

fn check_close(actual: Option<f64>, expected: f64) -> Result<(), Box<dyn std::error::Error>> {
    let actual = actual.ok_or("reflected timer value is not a number")?;
    if (actual - expected).abs() > 0.001 {
        return Err(format!("expected {expected}, got {actual}").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a display and render adapter"]
    fn rendered_session_navigates_game_menu_through_public_commands() {
        run().unwrap();
    }
}
