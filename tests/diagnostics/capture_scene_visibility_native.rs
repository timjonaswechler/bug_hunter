//! Main-thread AppKit sampler for disposable scene-visibility binaries.

use bevy::{
    camera::Camera,
    prelude::*,
    window::{PrimaryWindow, RawHandleWrapper},
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::NSView;
use raw_window_handle::RawWindowHandle;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

mod jsonl;

use jsonl::DiagnosticJsonl;

static SAMPLE_ID: AtomicU64 = AtomicU64::new(0);
static LOG: DiagnosticJsonl = DiagnosticJsonl::new(
    "appkit_main",
    "WOODPECKER_CAPTURE_SCENE_VISIBILITY_NATIVE_LOG_DIR",
);

pub fn install(app: &mut App) {
    let marker = MainThreadMarker::new().expect("Bevy App must start on the macOS main thread");
    app.insert_non_send(marker)
        .add_systems(bevy::app::Main, sample.after(bevy::app::Main::run_main));
}

fn sample(
    _main_thread: NonSend<MainThreadMarker>,
    window: Single<(&Window, &RawHandleWrapper), With<PrimaryWindow>>,
    cameras: Query<(Entity, &Camera, &Transform)>,
    mut last_front_request: Local<String>,
) {
    let (bevy_window, raw) = window.into_inner();
    let RawWindowHandle::AppKit(handle) = raw.get_window_handle() else {
        return;
    };
    // RawHandleWrapper retains the winit window. MainThreadMarker and NonSend
    // prove that the AppKit calls happen on the process main thread.
    let view: Retained<NSView> =
        unsafe { Retained::retain(handle.ns_view.as_ptr().cast()) }.expect("valid NSView");
    let Some(native) = view.window() else {
        return;
    };

    if let Some(root) = std::env::var_os("WOODPECKER_ARTIFACT_DIR") {
        let request = std::fs::read_to_string(std::path::PathBuf::from(root).join("visibility-front"))
            .unwrap_or_default();
        if !request.is_empty() && request != *last_front_request {
            native.orderFrontRegardless();
            *last_front_request = request;
        }
    }

    let frame = native.frame();
    let content = native.contentRectForFrameRect(frame);
    let occlusion = native.occlusionState().bits();
    let visible_bit = objc2_app_kit::NSWindowOcclusionState::Visible.bits();
    let camera_rows = cameras
        .iter()
        .map(|(entity, camera, transform)| {
            format!(
                "{{\"entity\":\"{entity}\",\"active\":{},\"physical_target_size\":\"{:?}\",\"viewport\":\"{:?}\",\"translation\":{:?},\"rotation\":{:?}}}",
                camera.is_active,
                camera.physical_target_size(),
                camera.viewport,
                transform.translation.to_array(),
                transform.rotation.to_array(),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let artifact_dir = std::env::var("WOODPECKER_ARTIFACT_DIR").unwrap_or_default();
    let session_id = std::path::Path::new(&artifact_dir)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let unix_time_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    log(&format!(
        "\"event\":\"appkit_sample\",\"sample_id\":{},\"unix_time_ns\":{unix_time_ns},\
         \"pid\":{},\"session_id\":{:?},\"artifact_dir\":{:?},\"main_thread\":true,\
         \"visible\":{},\"key\":{},\"miniaturized\":{},\"occlusion\":{},\
         \"occlusion_visible\":{},\"native_x\":{},\"native_y\":{},\
         \"native_width\":{},\"native_height\":{},\"content_x\":{},\
         \"content_y\":{},\"content_width\":{},\"content_height\":{},\
         \"bevy_physical_width\":{},\"bevy_physical_height\":{},\
         \"bevy_scale_factor\":{},\"cameras\":[{}]",
        SAMPLE_ID.fetch_add(1, Ordering::Relaxed) + 1,
        std::process::id(),
        session_id,
        artifact_dir,
        native.isVisible(),
        native.isKeyWindow(),
        native.isMiniaturized(),
        occlusion,
        occlusion & visible_bit != 0,
        frame.origin.x,
        frame.origin.y,
        frame.size.width,
        frame.size.height,
        content.origin.x,
        content.origin.y,
        content.size.width,
        content.size.height,
        bevy_window.physical_width(),
        bevy_window.physical_height(),
        bevy_window.scale_factor(),
        camera_rows,
    ));
}

fn log(fields: &str) {
    if !LOG.enabled() {
        return;
    }
    let row = format!("{{\"source\":\"appkit_main\",{fields}}}");
    LOG.write_json(&row)
        .expect("failed to write appkit_main visibility diagnostic");
}
