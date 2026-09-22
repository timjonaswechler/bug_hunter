//! Standalone Bevy v0.20.0-rc.1 capture fixture.
//!
//! This deliberately does not depend on woodpecker. The project-wide Bevy
//! 0.19 adapter is outside the release-tag experiment's dependency graph.

mod capture_causality_rc_native;

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    window::{PrimaryWindow, WindowPosition, WindowResolution},
};
use serde_json::json;
use std::{
    io::BufRead,
    path::PathBuf,
    sync::{Mutex, mpsc},
};

const FIXTURE_FINGERPRINT: &str =
    "blend-modes-static-v1:alpha=.9:camera=0,2.5,10:spheres=5:floor=49";
const CAPTURE_COUNT: usize = 4;

#[derive(Component)]
pub(crate) struct FixtureCamera;

#[derive(Resource)]
struct InputCommands(Mutex<mpsc::Receiver<String>>);

#[derive(Resource, Default)]
struct CaptureSequence(usize);

fn main() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Bevy v0.20.0-rc.1 capture causality".into(),
            resolution: WindowResolution::new(1280, 720).with_scale_factor_override(1.0),
            position: WindowPosition::At(IVec2::new(120, 120)),
            resizable: false,
            focused: false,
            ..default()
        }),
        ..default()
    }))
    .insert_resource(InputCommands(Mutex::new(rx)))
    .init_resource::<CaptureSequence>()
    .add_systems(Startup, (setup, announce_ready).chain())
    .add_systems(Update, process_commands);
    capture_causality_rc_native::install(&mut app);
    app.run();
}

fn announce_ready(window: Single<&Window, With<PrimaryWindow>>) {
    println!(
        "{}",
        json!({
            "event": "app_ready",
            "pid": std::process::id(),
            "fixture": FIXTURE_FINGERPRINT,
            "physical_width": window.physical_width(),
            "physical_height": window.physical_height(),
        })
    );
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let base_color = Color::srgba(0.9, 0.2, 0.3, 0.9);
    let sphere_mesh = meshes.add(Sphere::new(0.9).mesh().ico(7).unwrap());
    for (name, x, alpha_mode) in [
        ("opaque", -4.0, AlphaMode::Opaque),
        ("blend", -2.0, AlphaMode::Blend),
        ("premultiplied", 0.0, AlphaMode::Premultiplied),
        ("add", 2.0, AlphaMode::Add),
        ("multiply", 4.0, AlphaMode::Multiply),
    ] {
        let material = materials.add(StandardMaterial {
            base_color,
            alpha_mode,
            ..default()
        });
        commands.spawn((
            Name::new(format!("sphere-{name}")),
            Mesh3d(sphere_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(x, 0.0, 0.0),
        ));
    }

    let black_material = materials.add(Color::BLACK);
    let white_material = materials.add(Color::WHITE);
    let plane_mesh = meshes.add(Plane3d::default().mesh().size(2.0, 2.0));
    for x in -3..4 {
        for z in -3..4 {
            commands.spawn((
                Name::new(format!("floor-{x}-{z}")),
                Mesh3d(plane_mesh.clone()),
                MeshMaterial3d(if (x + z) % 2 == 0 {
                    black_material.clone()
                } else {
                    white_material.clone()
                }),
                Transform::from_xyz(x as f32 * 2.0, -1.0, z as f32 * 2.0),
            ));
        }
    }

    commands.spawn((
        Name::new("key-light"),
        PointLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    commands.spawn((
        Name::new("camera"),
        FixtureCamera,
        Camera3d::default(),
        Transform::from_translation(Vec3::new(0.0, 2.5, 10.0)).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn process_commands(
    mut commands: Commands,
    input: Res<InputCommands>,
    mut sequence: ResMut<CaptureSequence>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<&Transform, With<FixtureCamera>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(receiver) = input.0.try_lock() else {
        return;
    };
    while let Ok(line) = receiver.try_recv() {
        let mut words = line.split_whitespace();
        match (words.next(), words.next(), words.next()) {
            (Some("capture"), Some(label), None) => {
                assert!(
                    sequence.0 < CAPTURE_COUNT,
                    "more than four captures requested"
                );
                sequence.0 += 1;
                let ordinal = sequence.0;
                let output_dir = PathBuf::from(
                    std::env::var_os("WOODPECKER_CAPTURE_CAUSALITY_RC_PNG_DIR")
                        .expect("PNG output directory"),
                );
                let path = output_dir.join(format!("{ordinal:02}-{label}.png"));
                let event_label = label.to_owned();
                let event_path = path.clone();
                let mut screenshot = commands.spawn(Screenshot::primary_window());
                let entity = screenshot.id();
                screenshot.observe(move |captured: On<ScreenshotCaptured>| {
                    let rgb = captured
                        .image
                        .clone()
                        .try_into_dynamic()
                        .expect("screenshot image must decode")
                        .to_rgb8();
                    std::fs::create_dir_all(event_path.parent().unwrap()).unwrap();
                    rgb.save(&event_path).unwrap();
                    let nonzero_rgb_bytes =
                        rgb.as_raw().iter().filter(|value| **value != 0).count();
                    println!(
                        "{}",
                        json!({
                            "event": "capture_complete",
                            "ordinal": ordinal,
                            "label": event_label,
                            "screenshot_entity": format!("{}", captured.entity),
                            "path": event_path,
                            "width": rgb.width(),
                            "height": rgb.height(),
                            "nonzero_rgb_bytes": nonzero_rgb_bytes,
                            "fixture": FIXTURE_FINGERPRINT,
                        })
                    );
                });
                println!(
                    "{}",
                    json!({
                        "event": "capture_spawned",
                        "ordinal": ordinal,
                        "label": label,
                        "screenshot_entity": format!("{entity}"),
                        "physical_width": window.physical_width(),
                        "physical_height": window.physical_height(),
                        "scale_factor": window.scale_factor(),
                        "camera_translation": camera.translation.to_array(),
                        "camera_rotation": camera.rotation.to_array(),
                        "fixture": FIXTURE_FINGERPRINT,
                    })
                );
            }
            (Some("quit"), None, None) => {
                assert_eq!(sequence.0, CAPTURE_COUNT, "quit before four captures");
                exit.write(AppExit::Success);
            }
            _ => panic!("unknown command: {line}"),
        }
    }
}
