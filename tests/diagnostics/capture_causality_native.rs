//! Main-thread AppKit sampler for the disposable capture-causality binary.

use bevy::{
    camera::Camera,
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
    cameras: Query<&Camera>,
) {
    let (bevy_window, raw) = window.into_inner();
    let RawWindowHandle::AppKit(handle) = raw.get_window_handle() else {
        return;
    };
    // RawHandleWrapper retains the winit window. MainThreadMarker and NonSend
    // prove this dereference happens on the process main thread.
    let view: Retained<NSView> =
        unsafe { Retained::retain(handle.ns_view.as_ptr().cast()) }.expect("valid NSView");
    let Some(native) = view.window() else {
        return;
    };
    let frame = native.frame();
    let occlusion = native.occlusionState().bits();
    let visible_bit = objc2_app_kit::NSWindowOcclusionState::Visible.bits();
    let camera_rows = cameras
        .iter()
        .map(|camera| {
            format!(
                "{{\"active\":{},\"physical_target_size\":\"{:?}\",\"viewport\":\"{:?}\"}}",
                camera.is_active,
                camera.physical_target_size(),
                camera.viewport,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    log(&format!(
        "\"event\":\"appkit_sample\",\"sample_id\":{},\"main_thread\":true,\
         \"visible\":{},\"key\":{},\"miniaturized\":{},\"occlusion\":{},\
         \"occlusion_visible\":{},\"native_x\":{},\"native_y\":{},\
         \"native_width\":{},\"native_height\":{},\"bevy_physical_width\":{},\
         \"bevy_physical_height\":{},\"bevy_scale_factor\":{},\"cameras\":[{}]",
        SAMPLE_ID.fetch_add(1, Ordering::Relaxed) + 1,
        native.isVisible(),
        native.isKeyWindow(),
        native.isMiniaturized(),
        occlusion,
        occlusion & visible_bit != 0,
        frame.origin.x,
        frame.origin.y,
        frame.size.width,
        frame.size.height,
        bevy_window.physical_width(),
        bevy_window.physical_height(),
        bevy_window.scale_factor(),
        camera_rows,
    ));
}

fn log(fields: &str) {
    let Some(path) = std::env::var_os("WOODPECKER_CAPTURE_CAUSALITY_NATIVE_LOG") else {
        return;
    };
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(
        file,
        "{{\"source\":\"appkit_main\",\"fields\":{{{fields}}}}}"
    );
}
