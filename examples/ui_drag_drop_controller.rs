//! Controller test for the rendered UI drag-and-drop Controlled Session.
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
    time::Duration,
};

const NODE_PATH: &str = "bevy_ui::ui_node::Node";
const TRANSFORM_PATH: &str = "bevy_ui::ui_transform::UiTransform";
const Z_INDEX_PATH: &str = "bevy_ui::ui_node::GlobalZIndex";
const OUTLINE_PATH: &str = "bevy_ui::ui_node::Outline";
const STATE_PATH: &str = "ui_drag_drop::SceneState";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run()
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let launch = LaunchSpec {
        package: "bevy_test_apps".into(),
        kind: LaunchTargetKind::Binary,
        target: "ui_drag_drop".into(),
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
    assert!(ready.controls.contains(&"pointer".into()));
    assert!(ready.controls.contains(&"time".into()));
    assert!(ready.controls.contains(&"screenshot".into()));
    for _ in 0..30 {
        advance(&mut session, 1)?;
    }

    let targets = observe(&mut session, Selector::Targets, Projection::Summary)?;
    let grid = target(&targets, "drag-grid")?;
    let amber = target(&targets, "tile-amber")?;
    let blue = target(&targets, "tile-blue")?;
    let green = target(&targets, "tile-green")?;
    target(&targets, "tile-rose")?;
    let grid_handle = handle(grid)?;
    let amber_handle = handle(amber)?;
    let blue_handle = handle(blue)?;
    let green_handle = handle(green)?;
    let amber_center = center(amber)?;
    let blue_center = center(blue)?;
    let green_center = center(green)?;

    let initial_state = scene_state(&mut session, grid_handle)?;
    assert_eq!(
        initial_state["occupancy"],
        serde_json::json!(["Amber", "Blue", "Green", "Rose"])
    );
    assert_eq!(initial_state["active_tile"], Value::Null);
    let initial_amber = tile_components(&mut session, amber_handle)?;
    let initial_blue = tile_components(&mut session, blue_handle)?;

    move_pointer(&mut session, amber_center)?;
    wait_for_pixel(
        &mut session,
        &artifact_root,
        "ui-drag-drop/warmup",
        amber_center,
        is_amber,
    )?;
    let hovered_state = scene_state(&mut session, grid_handle)?;
    assert!(hovered_state["hover_events"].as_u64().unwrap_or_default() >= 1);
    let initial_image = capture(&mut session, &artifact_root, "ui-drag-drop/initial.png")?;
    assert_color(&initial_image, amber_center, "amber", is_amber)?;
    assert_color(&initial_image, blue_center, "blue", is_blue)?;

    pointer(
        &mut session,
        PointerCommand::Press {
            button: Button::Primary,
        },
    )?;
    let midpoint = [
        (amber_center[0] + blue_center[0]) / 2.0,
        (amber_center[1] + blue_center[1]) / 2.0,
    ];
    move_pointer(&mut session, midpoint)?;
    let dragging_state = scene_state(&mut session, grid_handle)?;
    assert_eq!(dragging_state["active_tile"], "Amber");
    assert_eq!(dragging_state["drag_start_events"], 1);
    assert!(dragging_state["drag_events"].as_u64().unwrap_or_default() >= 1);
    assert_eq!(
        dragging_state["drag_sequence"].as_array().unwrap()[0],
        "DragStart"
    );
    let dragging_amber = tile_components(&mut session, amber_handle)?;
    assert_ne!(
        component(&dragging_amber, TRANSFORM_PATH)?,
        component(&initial_amber, TRANSFORM_PATH)?
    );
    assert_ne!(
        component(&dragging_amber, Z_INDEX_PATH)?,
        component(&initial_amber, Z_INDEX_PATH)?
    );
    assert_ne!(
        component(&dragging_amber, OUTLINE_PATH)?,
        component(&initial_amber, OUTLINE_PATH)?
    );
    wait_for_pixel(
        &mut session,
        &artifact_root,
        "ui-drag-drop/dragging-warmup",
        midpoint,
        is_amber,
    )?;
    let dragging_image = capture(&mut session, &artifact_root, "ui-drag-drop/dragging.png")?;
    assert_color(&dragging_image, midpoint, "dragged amber", is_amber)?;

    move_pointer(&mut session, blue_center)?;
    pointer(
        &mut session,
        PointerCommand::Release {
            button: Button::Primary,
        },
    )?;
    let dropped_state = scene_state(&mut session, grid_handle)?;
    assert_eq!(dropped_state["active_tile"], Value::Null);
    assert_eq!(
        dropped_state["occupancy"],
        serde_json::json!(["Blue", "Amber", "Green", "Rose"])
    );
    assert_eq!(dropped_state["drag_start_events"], 1);
    assert!(dropped_state["drag_events"].as_u64().unwrap_or_default() >= 2);
    assert_eq!(dropped_state["drag_drop_events"], 1);
    assert_eq!(dropped_state["drag_end_events"], 1);
    assert_drag_sequence(&dropped_state["drag_sequence"])?;

    let dropped_amber = tile_components(&mut session, amber_handle)?;
    let dropped_blue = tile_components(&mut session, blue_handle)?;
    assert_ne!(
        component(&dropped_amber, NODE_PATH)?,
        component(&initial_amber, NODE_PATH)?
    );
    assert_ne!(
        component(&dropped_blue, NODE_PATH)?,
        component(&initial_blue, NODE_PATH)?
    );
    assert_eq!(
        component(&dropped_amber, TRANSFORM_PATH)?,
        component(&initial_amber, TRANSFORM_PATH)?
    );
    assert_eq!(
        component(&dropped_amber, Z_INDEX_PATH)?,
        component(&initial_amber, Z_INDEX_PATH)?
    );
    assert_eq!(
        component(&dropped_amber, OUTLINE_PATH)?,
        component(&initial_amber, OUTLINE_PATH)?
    );
    wait_for_pixel(
        &mut session,
        &artifact_root,
        "ui-drag-drop/dropped-warmup",
        amber_center,
        is_blue,
    )?;
    let dropped_image = capture(&mut session, &artifact_root, "ui-drag-drop/dropped.png")?;
    assert_color(
        &dropped_image,
        amber_center,
        "blue in the source slot",
        is_blue,
    )?;
    assert_color(
        &dropped_image,
        blue_center,
        "amber in the destination slot",
        is_amber,
    )?;

    let occupancy_before_invalid = dropped_state["occupancy"].clone();
    let green_before_invalid = tile_components(&mut session, green_handle)?;
    let drop_count_before_invalid = dropped_state["drag_drop_events"].clone();
    move_pointer(&mut session, green_center)?;
    pointer(
        &mut session,
        PointerCommand::Press {
            button: Button::Primary,
        },
    )?;
    move_pointer(&mut session, [green_center[0], green_center[1] - 60.0])?;
    move_pointer(&mut session, [40.0, 40.0])?;
    pointer(
        &mut session,
        PointerCommand::Release {
            button: Button::Primary,
        },
    )?;

    let invalid_state = scene_state(&mut session, grid_handle)?;
    assert_eq!(invalid_state["active_tile"], Value::Null);
    assert_eq!(invalid_state["occupancy"], occupancy_before_invalid);
    assert_eq!(invalid_state["drag_drop_events"], drop_count_before_invalid);
    assert_eq!(invalid_state["drag_start_events"], 2);
    assert_eq!(invalid_state["drag_end_events"], 2);
    let green_after_invalid = tile_components(&mut session, green_handle)?;
    for path in [NODE_PATH, TRANSFORM_PATH, Z_INDEX_PATH, OUTLINE_PATH] {
        assert_eq!(
            component(&green_after_invalid, path)?,
            component(&green_before_invalid, path)?
        );
    }

    session.shutdown()?;
    fs::remove_dir_all(artifact_root).ok();
    Ok(())
}

fn assert_drag_sequence(sequence: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let events = sequence.as_array().ok_or("drag_sequence is not an array")?;
    if events.len() < 4
        || events.first() != Some(&Value::String("DragStart".into()))
        || events[1..events.len() - 2]
            .iter()
            .any(|event| event != "Drag")
        || events[events.len() - 2] != "DragDrop"
        || events.last() != Some(&Value::String("DragEnd".into()))
    {
        return Err(format!("unexpected Bevy drag lifecycle: {events:?}").into());
    }
    Ok(())
}

fn temporary_artifact_root() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "automation-control-ui-drag-drop-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn pointer(
    session: &mut Session,
    command: PointerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    session.request(Command::Pointer(command))?;
    advance(session, 1)
}

fn move_pointer(
    session: &mut Session,
    position: [f32; 2],
) -> Result<(), Box<dyn std::error::Error>> {
    pointer(
        session,
        PointerCommand::Move {
            surface: None,
            position,
        },
    )
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

fn scene_state(session: &mut Session, grid: Handle) -> Result<Value, Box<dyn std::error::Error>> {
    let item = observe(
        session,
        Selector::Entity(grid),
        Projection::Components {
            type_paths: vec![STATE_PATH.into()],
        },
    )?
    .remove(0);
    Ok(component(&item, STATE_PATH)?.clone())
}

fn tile_components(
    session: &mut Session,
    tile: Handle,
) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(observe(
        session,
        Selector::Entity(tile),
        Projection::Components {
            type_paths: vec![
                NODE_PATH.into(),
                TRANSFORM_PATH.into(),
                Z_INDEX_PATH.into(),
                OUTLINE_PATH.into(),
            ],
        },
    )?
    .remove(0))
}

fn component<'a>(
    item: &'a Value,
    type_path: &str,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let component = &item["components"][type_path];
    if component["status"] != "available" {
        return Err(format!("component {type_path:?} is unavailable: {component}").into());
    }
    Ok(&component["value"])
}

fn target<'a>(items: &'a [Value], name: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    items
        .iter()
        .find(|item| item["name"] == name)
        .ok_or_else(|| format!("target {name:?} not found in {items:?}").into())
}

fn handle(item: &Value) -> Result<Handle, Box<dyn std::error::Error>> {
    Ok(serde_json::from_value(item["entity"].clone())?)
}

fn center(item: &Value) -> Result<[f32; 2], Box<dyn std::error::Error>> {
    let bounds = &item["bounds"];
    Ok([
        (bounds["x"].as_f64().ok_or("bounds.x missing")?
            + bounds["width"].as_f64().ok_or("bounds.width missing")? / 2.0) as f32,
        (bounds["y"].as_f64().ok_or("bounds.y missing")?
            + bounds["height"].as_f64().ok_or("bounds.height missing")? / 2.0) as f32,
    ])
}

fn capture(
    session: &mut Session,
    root: &Path,
    path: &str,
) -> Result<image::RgbImage, Box<dyn std::error::Error>> {
    let response = session.request(Command::Screenshot(ScreenshotCommand::new(path)))?;
    let artifact = &response.result.ok_or("screenshot result missing")?["artifact"];
    assert_eq!(artifact["path"], path);
    assert_eq!(artifact["width"], 640);
    assert_eq!(artifact["height"], 480);
    Ok(image::open(root.join(path))?.to_rgb8())
}

fn wait_for_pixel(
    session: &mut Session,
    root: &Path,
    stem: &str,
    position: [f32; 2],
    expected: fn([u8; 3]) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    for attempt in 0..40 {
        advance(session, 1)?;
        let image = capture(session, root, &format!("{stem}-{attempt}.png"))?;
        if expected(pixel(&image, position)) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(format!("expected scene color did not render at {position:?}").into())
}

fn assert_color(
    image: &image::RgbImage,
    position: [f32; 2],
    name: &str,
    expected: fn([u8; 3]) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = pixel(image, position);
    if !expected(actual) {
        return Err(format!("expected {name} at {position:?}, got RGB {actual:?}").into());
    }
    Ok(())
}

fn pixel(image: &image::RgbImage, position: [f32; 2]) -> [u8; 3] {
    image
        .get_pixel((position[0] - 30.0) as u32, (position[1] - 30.0) as u32)
        .0
}

fn is_amber([red, green, blue]: [u8; 3]) -> bool {
    red > 180 && green > 100 && blue < 180 && red > green
}

fn is_blue([red, green, blue]: [u8; 3]) -> bool {
    blue > 160 && blue > red && blue > green
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a display and render adapter"]
    fn rendered_session_drives_complete_ui_drag_gestures() {
        run().unwrap();
    }
}
