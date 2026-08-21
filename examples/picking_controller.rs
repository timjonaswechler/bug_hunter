//! Controller smoke test for the rendered mesh-picking Controlled Session.
use automation_control::{
    Command, Handle,
    driver::{LaunchSpec, LaunchTargetKind, Session, SessionOptions},
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

const TRANSFORM_PATH: &str = "bevy_transform::components::transform::Transform";
const PICKABLE_PATH: &str = "bevy_picking::Pickable";
const PICKING_INTERACTION_PATH: &str = "bevy_picking::hover::PickingInteraction";
const STATE_PATH: &str = "mesh_picking::MeshInteractionState";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run()
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let launch = LaunchSpec {
        package: "bevy_test_apps".into(),
        kind: LaunchTargetKind::Binary,
        target: "mesh_picking".into(),
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
    advance(&mut session, 1)?;

    let targets = observe(&mut session, Selector::Targets, Projection::Summary)?;
    assert_eq!(targets.len(), 3);
    find_named(&targets, "left-sphere")?;
    find_named(&targets, "right-cylinder")?;
    let cube = find_named(&targets, "center-cube")?;
    let cube_handle: Handle = serde_json::from_value(cube["entity"].clone())?;
    let initial = observe_mesh(&mut session, cube_handle)?;
    let initial_transform = component_value(&initial, TRANSFORM_PATH)?.clone();
    assert_eq!(
        component_value(&initial, STATE_PATH)?["last_interaction"],
        "Idle"
    );
    assert_eq!(
        component_value(&initial, PICKABLE_PATH)?["is_hoverable"],
        true
    );
    assert_eq!(component_value(&initial, PICKING_INTERACTION_PATH)?, "None");

    let unchanged = observe_mesh(&mut session, cube_handle)?;
    assert_eq!(
        component_value(&unchanged, TRANSFORM_PATH)?,
        &initial_transform
    );
    pointer(
        &mut session,
        PointerCommand::Move {
            surface: None,
            position: [320.0, 180.0],
        },
    )?;
    let hovered = observe_mesh(&mut session, cube_handle)?;
    assert_eq!(
        component_value(&hovered, STATE_PATH)?["last_interaction"],
        "Hover"
    );
    assert_eq!(
        component_value(&hovered, PICKING_INTERACTION_PATH)?,
        "Hovered"
    );
    assert_eq!(observe_frame(&mut session)?, 2);
    let initial_image = capture_screenshot(&mut session, &artifact_root, "mesh/initial.png")?;
    assert_center_is_cyan(&initial_image)?;

    advance(&mut session, 12)?;
    let rotated = observe_mesh(&mut session, cube_handle)?;
    let rotated_transform = component_value(&rotated, TRANSFORM_PATH)?;
    assert_ne!(rotated_transform, &initial_transform);
    assert_expected_rotation(rotated_transform, 14)?;
    assert_eq!(observe_frame(&mut session)?, 14);
    let rotated_image = capture_screenshot(&mut session, &artifact_root, "mesh/rotated.png")?;
    assert_center_is_cyan(&rotated_image)?;
    assert_ne!(initial_image, rotated_image);

    pointer(
        &mut session,
        PointerCommand::Press {
            button: Button::Primary,
        },
    )?;
    let pressed = observe_mesh(&mut session, cube_handle)?;
    assert_eq!(
        component_value(&pressed, STATE_PATH)?["last_interaction"],
        "Press"
    );
    let transform_before_drag = component_value(&pressed, TRANSFORM_PATH)?.clone();

    pointer(
        &mut session,
        PointerCommand::Move {
            surface: None,
            position: [340.0, 190.0],
        },
    )?;
    let dragged = observe_mesh(&mut session, cube_handle)?;
    let drag_state = component_value(&dragged, STATE_PATH)?;
    assert_eq!(drag_state["last_interaction"], "Drag");
    assert_eq!(drag_state["drag_events"], 1);
    let dragged_transform = component_value(&dragged, TRANSFORM_PATH)?;
    assert_ne!(dragged_transform, &transform_before_drag);
    assert_drag_tilt(dragged_transform)?;
    assert_eq!(observe_frame(&mut session)?, 16);

    let dragged_image = capture_screenshot(&mut session, &artifact_root, "mesh/dragged.png")?;
    assert_ne!(rotated_image, dragged_image);
    assert_center_is_yellow(&dragged_image)?;

    pointer(
        &mut session,
        PointerCommand::Release {
            button: Button::Primary,
        },
    )?;
    let released = observe_mesh(&mut session, cube_handle)?;
    assert_eq!(
        component_value(&released, STATE_PATH)?["last_interaction"],
        "Release"
    );

    session.shutdown()?;
    fs::remove_dir_all(artifact_root).ok();
    Ok(())
}

fn temporary_artifact_root() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "automation-control-mesh-picking-{}-{}",
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
    let artifact = &response.result.ok_or("screenshot result missing")?["artifact"];
    assert_eq!(artifact["path"], relative_path);
    assert_eq!(artifact["mime_type"], "image/png");
    assert_eq!(artifact["width"], 640);
    assert_eq!(artifact["height"], 360);
    Ok(image::open(artifact_root.join(relative_path))?.to_rgb8())
}

fn assert_expected_rotation(
    transform: &Value,
    completed_frames: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let rotation = transform["rotation"]
        .as_array()
        .ok_or("Transform.rotation is not an array")?;
    let actual_y = rotation[1].as_f64().ok_or("Transform.rotation.y missing")? as f32;
    let actual_w = rotation[3].as_f64().ok_or("Transform.rotation.w missing")? as f32;
    let step_seconds = 16_666_667_f32 / 1_000_000_000.0;
    let half_angle = completed_frames as f32 * step_seconds / 4.0;
    if (actual_y - half_angle.sin()).abs() > 0.000_01
        || (actual_w - half_angle.cos()).abs() > 0.000_01
    {
        return Err(format!("unexpected controlled rotation: {rotation:?}").into());
    }
    Ok(())
}

fn assert_drag_tilt(transform: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let rotation = transform["rotation"]
        .as_array()
        .ok_or("Transform.rotation is not an array")?;
    let x = rotation[0].as_f64().ok_or("Transform.rotation.x missing")?;
    if x.abs() < 0.05 {
        return Err(format!("drag did not add the expected x-axis tilt: {rotation:?}").into());
    }
    Ok(())
}

fn assert_center_is_cyan(image: &image::RgbImage) -> Result<(), Box<dyn std::error::Error>> {
    let pixel = image.get_pixel(image.width() / 2, image.height() / 2).0;
    if pixel[1] < 30
        || pixel[2] < 30
        || u16::from(pixel[1].min(pixel[2])) <= u16::from(pixel[0]) + 20
    {
        return Err(
            format!("expected the hovered center mesh to be cyan, got RGB {pixel:?}").into(),
        );
    }
    Ok(())
}

fn assert_center_is_yellow(image: &image::RgbImage) -> Result<(), Box<dyn std::error::Error>> {
    let pixel = image.get_pixel(image.width() / 2, image.height() / 2).0;
    if pixel[0] < 30
        || pixel[1] < 30
        || u16::from(pixel[2]) * 2 >= u16::from(pixel[0].min(pixel[1]))
    {
        return Err(
            format!("expected the pressed center mesh to be yellow, got RGB {pixel:?}").into(),
        );
    }
    Ok(())
}

fn pointer(
    session: &mut Session,
    command: PointerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    session.request(Command::Pointer(command))?;
    advance(session, 1)
}

fn advance(session: &mut Session, frames: u64) -> Result<(), Box<dyn std::error::Error>> {
    session.request(Command::Time(TimeCommand::advance(frames, 16_666_667)))?;
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

fn observe_frame(session: &mut Session) -> Result<u64, Box<dyn std::error::Error>> {
    let item = observe(session, Selector::Clock, Projection::Summary)?
        .into_iter()
        .next()
        .ok_or("clock observation did not return an item")?;
    item["frame_index"]
        .as_u64()
        .ok_or_else(|| "clock observation has no frame index".into())
}

fn observe_mesh(session: &mut Session, mesh: Handle) -> Result<Value, Box<dyn std::error::Error>> {
    observe(
        session,
        Selector::Entity(mesh),
        Projection::Components {
            type_paths: vec![
                TRANSFORM_PATH.into(),
                PICKABLE_PATH.into(),
                PICKING_INTERACTION_PATH.into(),
                STATE_PATH.into(),
            ],
        },
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "mesh observation did not return an item".into())
}

fn component_value<'a>(
    item: &'a Value,
    type_path: &str,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let component = &item["components"][type_path];
    if component["status"] != "available" {
        return Err(format!("component {type_path:?} is unavailable: {component}").into());
    }
    Ok(&component["value"])
}

fn find_named<'a>(items: &'a [Value], name: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    items
        .iter()
        .find(|item| item["name"] == name)
        .ok_or_else(|| format!("target {name:?} not found in {items:?}").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a display and render adapter"]
    fn rendered_session_controls_mesh_picking_through_public_commands() {
        run().unwrap();
    }
}
