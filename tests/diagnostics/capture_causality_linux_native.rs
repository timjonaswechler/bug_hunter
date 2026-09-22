//! X11 main-thread sampler for the disposable Linux capture-causality binary.

use bevy::{
    camera::Camera,
    prelude::*,
    window::{PrimaryWindow, RawHandleWrapper},
};
use raw_window_handle::RawWindowHandle;
use std::{
    ffi::{c_char, c_int, c_long, c_ulong, c_void},
    ptr,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

mod jsonl;

use jsonl::DiagnosticJsonl;

type Display = c_void;
type XWindow = c_ulong;

const VISIBILITY_CHANGE_MASK: c_long = 1 << 16;
const IS_UNMAPPED: c_int = 0;
const IS_UNVIEWABLE: c_int = 1;
const IS_VIEWABLE: c_int = 2;
const VISIBILITY_UNOBSCURED: c_int = 0;
const VISIBILITY_PARTIALLY_OBSCURED: c_int = 1;
const VISIBILITY_FULLY_OBSCURED: c_int = 2;

#[repr(C)]
struct XWindowAttributes {
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border_width: c_int,
    depth: c_int,
    visual: *mut c_void,
    root: XWindow,
    class: c_int,
    bit_gravity: c_int,
    win_gravity: c_int,
    backing_store: c_int,
    backing_planes: c_ulong,
    backing_pixel: c_ulong,
    save_under: c_int,
    colormap: c_ulong,
    map_installed: c_int,
    map_state: c_int,
    all_event_masks: c_long,
    your_event_mask: c_long,
    do_not_propagate_mask: c_long,
    override_redirect: c_int,
    screen: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XVisibilityEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: XWindow,
    state: c_int,
}

#[repr(C)]
union XEvent {
    type_: c_int,
    visibility: XVisibilityEvent,
    pad: [c_long; 24],
}

#[link(name = "X11")]
unsafe extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut Display;
    fn XCloseDisplay(display: *mut Display) -> c_int;
    fn XDefaultRootWindow(display: *mut Display) -> XWindow;
    fn XGetWindowAttributes(
        display: *mut Display,
        window: XWindow,
        attributes: *mut XWindowAttributes,
    ) -> c_int;
    fn XTranslateCoordinates(
        display: *mut Display,
        source: XWindow,
        destination: XWindow,
        source_x: c_int,
        source_y: c_int,
        destination_x: *mut c_int,
        destination_y: *mut c_int,
        child: *mut XWindow,
    ) -> c_int;
    fn XGetInputFocus(display: *mut Display, focus: *mut XWindow, revert_to: *mut c_int) -> c_int;
    fn XSelectInput(display: *mut Display, window: XWindow, event_mask: c_long) -> c_int;
    fn XCheckWindowEvent(
        display: *mut Display,
        window: XWindow,
        event_mask: c_long,
        event: *mut XEvent,
    ) -> c_int;
    fn XFlush(display: *mut Display) -> c_int;
}

struct X11State {
    display: *mut Display,
    selected_window: XWindow,
    visibility: Option<c_int>,
}

impl X11State {
    fn open() -> Self {
        let display = unsafe { XOpenDisplay(ptr::null()) };
        assert!(
            !display.is_null(),
            "failed to open DISPLAY for Linux diagnostic"
        );
        Self {
            display,
            selected_window: 0,
            visibility: None,
        }
    }

    fn select(&mut self, window: XWindow) {
        if self.selected_window == window {
            return;
        }
        assert_ne!(window, 0, "X11 target window must be nonzero");
        unsafe {
            XSelectInput(self.display, window, VISIBILITY_CHANGE_MASK);
            XFlush(self.display);
        }
        self.selected_window = window;
        self.visibility = None;
    }

    fn drain_visibility(&mut self) {
        loop {
            let mut event = XEvent { pad: [0; 24] };
            let found = unsafe {
                XCheckWindowEvent(
                    self.display,
                    self.selected_window,
                    VISIBILITY_CHANGE_MASK,
                    &mut event,
                )
            };
            if found == 0 {
                break;
            }
            self.visibility = Some(unsafe { event.visibility.state });
        }
    }
}

impl Drop for X11State {
    fn drop(&mut self) {
        if !self.display.is_null() {
            unsafe {
                XCloseDisplay(self.display);
            }
        }
    }
}

static SAMPLE_ID: AtomicU64 = AtomicU64::new(0);
static LOG: DiagnosticJsonl =
    DiagnosticJsonl::new("x11_main", "WOODPECKER_CAPTURE_CAUSALITY_LINUX_LOG_DIR");

pub fn install(app: &mut App) {
    assert!(
        LOG.enabled(),
        "Linux capture diagnostic log directory is not configured"
    );
    app.insert_non_send(X11State::open())
        .add_systems(bevy::app::Main, sample.after(bevy::app::Main::run_main));
}

fn sample(
    mut x11: NonSendMut<X11State>,
    window: Single<(&Window, &RawHandleWrapper), With<PrimaryWindow>>,
    cameras: Query<(Entity, &Camera, &Transform)>,
) {
    let (bevy_window, raw) = window.into_inner();
    let xid = match raw.get_window_handle() {
        RawWindowHandle::Xlib(handle) => handle.window,
        RawWindowHandle::Xcb(handle) => c_ulong::from(handle.window.get()),
        _ => panic!("Linux X11 diagnostic did not receive an X11 window handle"),
    };
    x11.select(xid);
    x11.drain_visibility();

    let mut attributes: XWindowAttributes = unsafe { std::mem::zeroed() };
    assert_ne!(
        unsafe { XGetWindowAttributes(x11.display, xid, &mut attributes) },
        0,
        "XGetWindowAttributes failed for owned target window",
    );
    let root = unsafe { XDefaultRootWindow(x11.display) };
    let mut root_x = 0;
    let mut root_y = 0;
    let mut child = 0;
    assert_ne!(
        unsafe {
            XTranslateCoordinates(
                x11.display,
                xid,
                root,
                0,
                0,
                &mut root_x,
                &mut root_y,
                &mut child,
            )
        },
        0,
        "XTranslateCoordinates failed for owned target window",
    );
    let mut focused_window = 0;
    let mut revert_to = 0;
    unsafe {
        XGetInputFocus(x11.display, &mut focused_window, &mut revert_to);
    }

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
    let placement_id = std::fs::read_to_string(
        std::path::Path::new(&artifact_dir).join("linux-capture-placement-id"),
    )
    .unwrap_or_default();
    let unix_time_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let map_state = match attributes.map_state {
        IS_UNMAPPED => "unmapped",
        IS_UNVIEWABLE => "unviewable",
        IS_VIEWABLE => "viewable",
        _ => "unknown",
    };
    let visibility = match x11.visibility {
        Some(VISIBILITY_UNOBSCURED) => "\"unobscured\"",
        Some(VISIBILITY_PARTIALLY_OBSCURED) => "\"partially_obscured\"",
        Some(VISIBILITY_FULLY_OBSCURED) => "\"fully_obscured\"",
        Some(_) => "\"unknown\"",
        None => "null",
    };
    let row = format!(
        "{{\"source\":\"x11_main\",\"event\":\"x11_sample\",\"sample_id\":{},\"unix_time_ns\":{unix_time_ns},\"pid\":{},\"session_id\":{:?},\"artifact_dir\":{:?},\"placement_id\":{:?},\"main_thread\":true,\"x11_window\":{xid},\"bevy_visible\":{},\"map_state\":\"{map_state}\",\"focused\":{},\"focused_window\":{focused_window},\"visibility_state\":{visibility},\"native_x\":{root_x},\"native_y\":{root_y},\"native_width\":{},\"native_height\":{},\"border_width\":{},\"bevy_physical_width\":{},\"bevy_physical_height\":{},\"bevy_scale_factor\":{},\"cameras\":[{camera_rows}]}}",
        SAMPLE_ID.fetch_add(1, Ordering::Relaxed) + 1,
        std::process::id(),
        session_id,
        artifact_dir,
        placement_id.trim(),
        bevy_window.visible,
        focused_window == xid,
        attributes.width,
        attributes.height,
        attributes.border_width,
        bevy_window.physical_width(),
        bevy_window.physical_height(),
        bevy_window.scale_factor(),
    );
    LOG.write_json(&row)
        .expect("failed to write x11_main Linux capture diagnostic");
}
