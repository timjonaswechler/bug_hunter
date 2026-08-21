//! Repeatedly measures the gap between protocol readiness and the first visible rendered frame.
use automation_control::{
    Command,
    driver::{LaunchSpec, LaunchTargetKind, Session, SessionOptions},
    observation::{Projection, Request as ObservationRequest, Selector},
    screenshot::Command as ScreenshotCommand,
    time::Command as TimeCommand,
};
use serde::Serialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitCode},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const STEP_NANOSECONDS: u64 = 16_666_667;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FrameKind {
    Black,
    ClearOnly,
    Scene,
}

#[derive(Debug, Serialize)]
struct ImageStats {
    kind: FrameKind,
    minimum_channel: u8,
    maximum_channel: u8,
    nonzero_pixels: u64,
    mean_channel: f64,
}

impl ImageStats {
    fn contains_scene_content(&self) -> bool {
        self.kind == FrameKind::Scene
    }
}

#[derive(Debug, Serialize)]
struct RunReport {
    run: usize,
    ready_milliseconds: u128,
    ready_capture_milliseconds: Option<u128>,
    ready_capture: Option<ImageStats>,
    scene_bounds_ready: bool,
    first_visible_attempt: Option<usize>,
    first_visible_milliseconds: Option<u128>,
    post_frame_captures: Vec<ImageStats>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("render readiness stress failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let runs = environment_usize("RENDER_STRESS_RUNS", 20)?;
    let attempts = environment_usize("RENDER_STRESS_ATTEMPTS", 1)?;
    let delay_milliseconds = environment_u64("RENDER_STRESS_DELAY_MS", 0)?;
    let frames = environment_u64("RENDER_STRESS_FRAMES", 2)?;
    let capture_ready = environment_bool("RENDER_STRESS_CAPTURE_READY", false)?;
    let artifact_root = env::var_os("RENDER_STRESS_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(temporary_artifact_root);
    fs::create_dir_all(&artifact_root)?;

    println!(
        "render readiness stress: runs={runs} attempts={attempts} delay_ms={delay_milliseconds} frames={frames} capture_ready={capture_ready} artifacts={}",
        artifact_root.display()
    );

    let mut reports = Vec::with_capacity(runs);
    let mut failures = 0;
    for run_index in 0..runs {
        let report = run_session(
            run_index,
            attempts,
            delay_milliseconds,
            frames,
            capture_ready,
            &artifact_root,
        )?;
        let status = if report.first_visible_attempt.is_some() {
            "scene_visible"
        } else {
            failures += 1;
            "scene_missing"
        };
        println!(
            "run={} ready_ms={} ready_capture_ms={:?} ready_kind={:?} bounds_ready={} last_capture_kind={:?} first_visible_attempt={:?} first_visible_ms={:?} status={status}",
            report.run,
            report.ready_milliseconds,
            report.ready_capture_milliseconds,
            report.ready_capture.as_ref().map(|capture| capture.kind),
            report.scene_bounds_ready,
            report
                .post_frame_captures
                .last()
                .map(|capture| capture.kind),
            report.first_visible_attempt,
            report.first_visible_milliseconds,
        );
        reports.push(report);
    }

    let report_path = artifact_root.join("report.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&reports)?)?;
    println!(
        "render readiness stress complete: failures={failures}/{runs} report={}",
        report_path.display()
    );
    if failures > 0 {
        Err(format!("{failures} of {runs} sessions never produced scene content").into())
    } else {
        Ok(())
    }
}

fn run_session(
    run_index: usize,
    attempts: usize,
    delay_milliseconds: u64,
    frames: u64,
    capture_ready: bool,
    artifact_root: &Path,
) -> Result<RunReport, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let options = SessionOptions::new().with_artifact_dir(artifact_root);
    let mut session = match env::var_os("RENDER_STRESS_APP") {
        Some(application) => Session::spawn_command(ProcessCommand::new(application), options)?,
        None => {
            let launch = LaunchSpec {
                package: "bevy_test_apps".into(),
                kind: LaunchTargetKind::Binary,
                target: "ui_drag_drop".into(),
                features: vec!["automation".into()],
                arguments: vec![],
            };
            Session::spawn(&launch, options)?
        }
    };
    let ready = session.ready()?;
    if ready.mode != automation_control::RunMode::Rendered {
        return Err(format!("run {run_index} started in {:?} mode", ready.mode).into());
    }
    if !ready.controls.contains(&"screenshot".into()) {
        return Err(format!("run {run_index} did not advertise screenshot capture").into());
    }
    let ready_milliseconds = started.elapsed().as_millis();
    let ready_capture = capture_ready
        .then(|| {
            capture(
                &mut session,
                artifact_root,
                &format!("run-{run_index:03}/ready.png"),
            )
        })
        .transpose()?;
    let ready_capture_milliseconds = ready_capture
        .as_ref()
        .map(|_| started.elapsed().as_millis());

    for _ in 0..frames {
        session.request(Command::Time(TimeCommand::advance(1, STEP_NANOSECONDS)))?;
    }
    let scene_bounds_ready = observe_scene_bounds(&mut session)?;
    let mut post_frame_captures = Vec::new();
    let mut first_visible_attempt = None;
    let mut first_visible_milliseconds = None;
    for attempt in 0..attempts {
        let stats = capture(
            &mut session,
            artifact_root,
            &format!("run-{run_index:03}/post-frame-{attempt:03}.png"),
        )?;
        let visible = stats.contains_scene_content();
        post_frame_captures.push(stats);
        if visible {
            first_visible_attempt = Some(attempt);
            first_visible_milliseconds = Some(started.elapsed().as_millis());
            break;
        }
        std::thread::sleep(Duration::from_millis(delay_milliseconds));
    }
    session.shutdown()?;

    Ok(RunReport {
        run: run_index,
        ready_milliseconds,
        ready_capture_milliseconds,
        ready_capture,
        scene_bounds_ready,
        first_visible_attempt,
        first_visible_milliseconds,
        post_frame_captures,
    })
}

fn observe_scene_bounds(session: &mut Session) -> Result<bool, Box<dyn std::error::Error>> {
    let response = session.request(Command::Observe(ObservationRequest::new(
        Selector::Targets,
        Projection::Summary,
    )))?;
    let items = response
        .result
        .and_then(|value| value["items"].as_array().cloned())
        .ok_or("target observation did not return items")?;
    Ok(items.iter().any(|item| {
        item["name"] == "tile-amber"
            && item["bounds"]["width"]
                .as_f64()
                .is_some_and(|width| width > 0.0)
            && item["bounds"]["height"]
                .as_f64()
                .is_some_and(|height| height > 0.0)
    }))
}

fn capture(
    session: &mut Session,
    artifact_root: &Path,
    relative_path: &str,
) -> Result<ImageStats, Box<dyn std::error::Error>> {
    let response = session.request(Command::Screenshot(ScreenshotCommand::new(relative_path)))?;
    let artifact = &response.result.ok_or("screenshot result missing")?["artifact"];
    if artifact["path"] != relative_path {
        return Err(format!("unexpected screenshot artifact: {artifact}").into());
    }
    let image = image::open(artifact_root.join(relative_path))?.to_rgb8();
    let mut minimum_channel = u8::MAX;
    let mut maximum_channel = u8::MIN;
    let mut nonzero_pixels = 0_u64;
    let mut channel_sum = 0_u64;
    for pixel in image.pixels() {
        let channels = pixel.0;
        if channels != [0, 0, 0] {
            nonzero_pixels += 1;
        }
        for channel in channels {
            minimum_channel = minimum_channel.min(channel);
            maximum_channel = maximum_channel.max(channel);
            channel_sum += u64::from(channel);
        }
    }
    let channel_count = u64::from(image.width()) * u64::from(image.height()) * 3;
    let kind = if maximum_channel == 0 {
        FrameKind::Black
    } else if maximum_channel <= 100 {
        FrameKind::ClearOnly
    } else {
        FrameKind::Scene
    };
    Ok(ImageStats {
        kind,
        minimum_channel,
        maximum_channel,
        nonzero_pixels,
        mean_channel: channel_sum as f64 / channel_count as f64,
    })
}

fn environment_usize(name: &str, default: usize) -> Result<usize, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) => {
            let parsed = value.parse::<usize>()?;
            if parsed == 0 {
                Err(format!("{name} must be greater than zero").into())
            } else {
                Ok(parsed)
            }
        }
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}

fn environment_bool(name: &str, default: bool) -> Result<bool, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) => match value.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(format!("{name} must be true, false, 1, or 0").into()),
        },
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}

fn environment_u64(name: &str, default: u64) -> Result<u64, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) => Ok(value.parse::<u64>()?),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}

fn temporary_artifact_root() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    env::temp_dir().join(format!(
        "automation-control-render-readiness-stress-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}
