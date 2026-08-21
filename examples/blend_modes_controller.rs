//! Controller smoke test for the rendered blend-modes Controlled Session.
use automation_control::{
    Command, Handle,
    driver::{LaunchSpec, LaunchTargetKind, Session, SessionOptions},
    keyboard::{Command as KeyboardCommand, Key},
    observation::{Projection, Request as ObservationRequest, Selector},
    screenshot::Command as ScreenshotCommand,
    time::Command as TimeCommand,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

const STATE_PATH: &str = "blend_modes::SceneState";
const MATERIAL_HANDLE_PATH: &str = "blend_modes::ObservedMaterialHandle";
const TRANSFORM_PATH: &str = "bevy_transform::components::transform::Transform";
const STEP_NANOSECONDS: u64 = 100_000_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run()
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let launch = LaunchSpec {
        package: "bevy_test_apps".into(),
        kind: LaunchTargetKind::Binary,
        target: "blend_modes".into(),
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
    assert_eq!(ready.mode, automation_control::RunMode::Rendered);
    assert!(ready.controls.contains(&"keyboard".into()));
    assert!(ready.controls.contains(&"time".into()));
    assert!(ready.controls.contains(&"screenshot".into()));
    for _ in 0..30 {
        advance(&mut session, 1)?;
    }
    wait_for_rendered_scene(&mut session, &artifact_root)?;
    let targets = observe(&mut session, Selector::Targets, Projection::Summary)?;
    for name in [
        "camera",
        "sphere-opaque",
        "sphere-blend",
        "sphere-premultiplied",
        "sphere-add",
        "sphere-multiply",
    ] {
        find_named(&targets, name)?;
    }
    let camera = handle(find_named(&targets, "camera")?)?;
    let blend = handle(find_named(&targets, "sphere-blend")?)?;
    let component_names = observe(
        &mut session,
        Selector::Entity(blend),
        Projection::ComponentNames,
    )?;
    assert!(
        component_names[0]["components"]
            .as_array()
            .is_some_and(|components| components.iter().any(|path| path
                .as_str()
                .is_some_and(|path| path.contains("MeshMaterial3d")))),
        "blend sphere has no MeshMaterial3d component"
    );
    let blend_components = observe(
        &mut session,
        Selector::Entity(blend),
        Projection::Components {
            type_paths: vec![MATERIAL_HANDLE_PATH.into()],
        },
    )?;
    assert_eq!(
        blend_components[0]["components"][MATERIAL_HANDLE_PATH]["status"],
        "available"
    );
    assert!(
        blend_components[0]["components"][MATERIAL_HANDLE_PATH]["value"]["asset_id"]
            .as_str()
            .is_some_and(|asset_id| !asset_id.is_empty())
    );
    let initial = scene_state(&mut session, camera)?;
    let camera_components = observe(
        &mut session,
        Selector::Entity(camera),
        Projection::Components {
            type_paths: vec![TRANSFORM_PATH.into(), STATE_PATH.into()],
        },
    )?;
    assert_eq!(
        camera_components[0]["components"][TRANSFORM_PATH]["status"],
        "available"
    );
    assert_eq!(
        camera_components[0]["components"][STATE_PATH]["status"],
        "available"
    );
    let initial_transform = camera_components[0]["components"][TRANSFORM_PATH]["value"].clone();
    let before = capture(&mut session, &artifact_root, "blend-modes/before.png")?;

    press(&mut session, Key::ArrowLeft)?;
    assert_eq!(
        scene_state(&mut session, camera)?["camera_angle"],
        initial["camera_angle"]
    );
    advance(&mut session, 3)?;
    release(&mut session, Key::ArrowLeft)?;
    let after_camera_state = scene_state(&mut session, camera)?;
    assert!(after_camera_state["camera_angle"].as_f64().unwrap() > 0.0);
    let rotated_transform = observe(
        &mut session,
        Selector::Entity(camera),
        Projection::Components {
            type_paths: vec![TRANSFORM_PATH.into()],
        },
    )?[0]["components"][TRANSFORM_PATH]["value"]
        .clone();
    assert_ne!(rotated_transform, initial_transform);
    advance(&mut session, 2)?;
    assert_eq!(
        scene_state(&mut session, camera)?["camera_angle"],
        after_camera_state["camera_angle"]
    );
    let after_camera = capture_changed(
        &mut session,
        &artifact_root,
        "after-camera",
        &before,
        "camera rotation",
    )?;

    press(&mut session, Key::ArrowDown)?;
    assert_eq!(
        scene_state(&mut session, camera)?["alpha"],
        after_camera_state["alpha"]
    );
    advance(&mut session, 4)?;
    release(&mut session, Key::ArrowDown)?;
    let after_alpha_state = scene_state(&mut session, camera)?;
    let expected_alpha = initial["alpha"].as_f64().unwrap() - 0.4;
    assert!((after_alpha_state["alpha"].as_f64().unwrap() - expected_alpha).abs() < 0.0001);
    let after_alpha = capture_changed(
        &mut session,
        &artifact_root,
        "after-alpha",
        &after_camera,
        "alpha change",
    )?;

    tap(&mut session, Key::H)?;
    tap(&mut session, Key::Space)?;
    tap(&mut session, Key::C)?;
    let after_modes_state = scene_state(&mut session, camera)?;
    assert_eq!(after_modes_state["hdr"], true);
    assert_eq!(after_modes_state["unlit"], true);
    assert_eq!(after_modes_state["seed"], 0x5eed_b1e5_u64);
    assert_eq!(after_modes_state["color_changes"], 1);
    capture_changed(
        &mut session,
        &artifact_root,
        "after-modes",
        &after_alpha,
        "mode and color change",
    )?;

    session.shutdown()?;
    fs::remove_dir_all(artifact_root).ok();
    Ok(())
}

fn wait_for_rendered_scene(
    session: &mut Session,
    root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    for attempt in 0..100 {
        let image = capture(session, root, &format!("blend-modes/warmup-{attempt}.png"))?;
        if image.get_pixel(0, 0).0.iter().any(|channel| *channel > 2) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("blend-modes scene did not render during warmup".into())
}

fn temporary_artifact_root() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "automation-control-blend-modes-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn press(session: &mut Session, key: Key) -> Result<(), Box<dyn std::error::Error>> {
    session.request(Command::Keyboard(KeyboardCommand::Press { key }))?;
    Ok(())
}

fn release(session: &mut Session, key: Key) -> Result<(), Box<dyn std::error::Error>> {
    session.request(Command::Keyboard(KeyboardCommand::Release { key }))?;
    Ok(())
}

fn tap(session: &mut Session, key: Key) -> Result<(), Box<dyn std::error::Error>> {
    press(session, key.clone())?;
    advance(session, 1)?;
    release(session, key)?;
    advance(session, 1)
}

fn advance(session: &mut Session, frames: u64) -> Result<(), Box<dyn std::error::Error>> {
    session.request(Command::Time(TimeCommand::advance(
        frames,
        STEP_NANOSECONDS,
    )))?;
    Ok(())
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

fn scene_state(session: &mut Session, camera: Handle) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(observe(
        session,
        Selector::Entity(camera),
        Projection::Components {
            type_paths: vec![STATE_PATH.into()],
        },
    )?[0]["components"][STATE_PATH]["value"]
        .clone())
}

fn find_named<'a>(items: &'a [Value], name: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    items
        .iter()
        .find(|item| item["name"] == name)
        .ok_or_else(|| format!("target {name:?} not found in {items:?}").into())
}

fn handle(item: &Value) -> Result<Handle, Box<dyn std::error::Error>> {
    Ok(serde_json::from_value(item["entity"].clone())?)
}

fn capture(
    session: &mut Session,
    root: &Path,
    path: &str,
) -> Result<image::RgbImage, Box<dyn std::error::Error>> {
    let response = session.request(Command::Screenshot(ScreenshotCommand::new(path)))?;
    let artifact = &response.result.ok_or("screenshot result missing")?["artifact"];
    assert_eq!(artifact["path"], path);
    assert_eq!(artifact["width"], 1280);
    assert_eq!(artifact["height"], 720);
    let image = image::open(root.join(path))?.to_rgb8();
    assert_eq!(image.width(), 1280);
    assert_eq!(image.height(), 720);
    Ok(image)
}

fn capture_changed(
    session: &mut Session,
    root: &Path,
    stem: &str,
    previous: &image::RgbImage,
    change: &str,
) -> Result<image::RgbImage, Box<dyn std::error::Error>> {
    for attempt in 0..30 {
        let image = capture(session, root, &format!("blend-modes/{stem}-{attempt}.png"))?;
        let changed = previous
            .pixels()
            .zip(image.pixels())
            .filter(|(left, right)| left != right)
            .count();
        if changed >= 1_000 {
            return Ok(image);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(format!("{change} was not visible after repeated redraws").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a display and render adapter"]
    fn rendered_blend_modes_session_observes_controls_and_screenshots() {
        run().unwrap();
    }
}
