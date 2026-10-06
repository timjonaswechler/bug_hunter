//! Main-thread AppKit sampler for the Bevy v0.20.0-rc.1 fixture.

use crate::FixtureCamera;
use bevy::{
    prelude::*,
    window::{PrimaryWindow, RawHandleWrapper},
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::NSView;
use raw_window_handle::RawWindowHandle;
use std::{
    fs::OpenOptions,
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static SAMPLE_ID: AtomicU64 = AtomicU64::new(0);

pub fn install(app: &mut App) {
    let marker = MainThreadMarker::new().expect("Bevy App must start on the macOS main thread");
    app.insert_non_send(marker)
        .add_systems(bevy::app::Main, sample.after(bevy::app::Main::run_main));
}

fn sample(
    _main_thread: NonSend<MainThreadMarker>,
    window: Single<(&Window, &RawHandleWrapper), With<PrimaryWindow>>,
    camera: Single<(&Camera, &Transform), With<FixtureCamera>>,
) {
    let (bevy_window, raw) = window.into_inner();
    let RawWindowHandle::AppKit(handle) = raw.get_window_handle() else {
        return;
    };
    // RawHandleWrapper retains the winit window. MainThreadMarker and NonSend
    // prove that this dereference and every AppKit call run on the main thread.
    let view: Retained<NSView> =
        unsafe { Retained::retain(handle.ns_view.as_ptr().cast()) }.expect("valid NSView");
    let Some(native) = view.window() else {
        return;
    };
    let frame = native.frame();
    let content = native.contentRectForFrameRect(frame);
    let occlusion = native.occlusionState().bits();
    let visible_bit = objc2_app_kit::NSWindowOcclusionState::Visible.bits();
    let (camera_component, camera_transform) = camera.into_inner();
    let sample_id = SAMPLE_ID.fetch_add(1, Ordering::Relaxed) + 1;
    let unix_time_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let fields = format!(
        "{{\"sample_id\":{sample_id},\"unix_time_ns\":{unix_time_ns},\"main_thread\":true,\
         \"visible\":{},\"key\":{},\"miniaturized\":{},\"occlusion\":{},\
         \"occlusion_visible\":{},\"native_x\":{},\"native_y\":{},\
         \"native_width\":{},\"native_height\":{},\"content_x\":{},\
         \"content_y\":{},\"content_width\":{},\"content_height\":{},\
         \"bevy_physical_width\":{},\"bevy_physical_height\":{},\
         \"bevy_scale_factor\":{},\"camera_active\":{},\
         \"camera_target_size\":\"{:?}\",\"camera_viewport\":\"{:?}\",\
         \"camera_translation\":{:?},\"camera_rotation\":{:?},\
         \"fixture\":\"blend-modes-static-v1:alpha=.9:camera=0,2.5,10:spheres=5:floor=49\"}}",
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
        camera_component.is_active,
        camera_component.physical_target_size(),
        camera_component.viewport,
        camera_transform.translation.to_array(),
        camera_transform.rotation.to_array(),
    );
    append_log(&fields);
    write_latest(&fields);
}

fn append_log(fields: &str) {
    let Some(path) = std::env::var_os("WOODPECKER_CAPTURE_CAUSALITY_RC_NATIVE_LOG") else {
        return;
    };
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(
        file,
        "{{\"source\":\"appkit_main\",\"pid\":{},\"fields\":{fields}}}",
        std::process::id(),
    );
}

fn write_latest(fields: &str) {
    let Some(path) = std::env::var_os("WOODPECKER_CAPTURE_CAUSALITY_RC_NATIVE_LATEST") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    if std::fs::write(&temporary, fields).is_ok() {
        let _ = std::fs::rename(temporary, path);
    }
}
