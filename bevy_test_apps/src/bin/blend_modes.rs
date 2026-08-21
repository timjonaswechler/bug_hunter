//! Adapted from Bevy's `examples/3d/blend_modes.rs`; see the package README for provenance.

use bevy::{camera::Hdr, color::palettes::css::ORANGE, prelude::*, window::WindowResolution};
use bevy_test_apps::composition;
use rand::{Rng, SeedableRng, rngs::StdRng};

#[cfg(feature = "automation")]
use automation_control::AutomationTarget;

const COLOR_SEED: u64 = 0x5eed_b1e5;
const INITIAL_ALPHA: f32 = 0.9;
const CAMERA_POSITION: Vec3 = Vec3::new(0.0, 2.5, 10.0);
const SPHERE_COUNT: usize = 5;

fn main() {
    let mut app = App::new();
    composition::rendered(
        &mut app,
        Window {
            title: "Controlled blend modes test".into(),
            resolution: WindowResolution::new(1280, 720).with_scale_factor_override(1.0),
            resizable: false,
            ..default()
        },
    );

    app.insert_resource(ClearColor(Color::srgb(0.03, 0.03, 0.05)))
        .register_type::<Transform>()
        .register_type::<ObservedMaterialHandle>()
        .register_type::<SceneState>()
        .add_systems(Startup, setup)
        .add_systems(Update, (control_scene, position_labels).chain())
        .run();
}

#[derive(Component)]
struct ControllableMaterial {
    color_slot: usize,
    unlit: bool,
}

/// Reflected session-local identity for a sphere's material handle.
#[derive(Component, Reflect)]
#[reflect(Component)]
struct ObservedMaterialHandle {
    asset_id: String,
}

#[derive(Component)]
struct CameraTarget;

#[derive(Component)]
struct ProjectedLabel {
    entity: Entity,
}

#[derive(Component)]
struct StatusDisplay;

/// Reflected semantic state for the blend-modes Controlled Session.
#[derive(Clone, Component, Debug, Reflect)]
#[reflect(Component)]
struct SceneState {
    alpha: f32,
    hdr: bool,
    unlit: bool,
    camera_angle: f32,
    seed: u64,
    color_changes: u32,
    colors: Vec<[f32; 4]>,
}

impl Default for SceneState {
    fn default() -> Self {
        let initial = [0.9, 0.2, 0.3, INITIAL_ALPHA];
        Self {
            alpha: INITIAL_ALPHA,
            hdr: false,
            unlit: false,
            camera_angle: 0.0,
            seed: COLOR_SEED,
            color_changes: 0,
            colors: vec![initial; SPHERE_COUNT],
        }
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let base_color = Color::srgba(0.9, 0.2, 0.3, INITIAL_ALPHA);
    let sphere_mesh = meshes.add(Sphere::new(0.9).mesh().ico(7).unwrap());
    let modes = [
        ("opaque", -4.0, AlphaMode::Opaque),
        ("blend", -2.0, AlphaMode::Blend),
        ("premultiplied", 0.0, AlphaMode::Premultiplied),
        ("add", 2.0, AlphaMode::Add),
        ("multiply", 4.0, AlphaMode::Multiply),
    ];
    let mut spheres = Vec::with_capacity(SPHERE_COUNT);

    for (slot, (name, x, alpha_mode)) in modes.into_iter().enumerate() {
        let material = materials.add(StandardMaterial {
            base_color,
            alpha_mode,
            ..default()
        });
        let entity = commands
            .spawn((
                Name::new(format!("sphere-{name}")),
                Mesh3d(sphere_mesh.clone()),
                MeshMaterial3d(material.clone()),
                ObservedMaterialHandle {
                    asset_id: format!("{:?}", material.id()),
                },
                Transform::from_xyz(x, 0.0, 0.0),
                ControllableMaterial {
                    color_slot: slot,
                    unlit: true,
                },
                #[cfg(feature = "automation")]
                AutomationTarget,
            ))
            .id();
        spheres.push((entity, name));
    }

    let black_material = materials.add(Color::BLACK);
    let white_material = materials.add(Color::WHITE);
    let plane_mesh = meshes.add(Plane3d::default().mesh().size(2.0, 2.0));
    let mut color_slot = SPHERE_COUNT;
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
                ControllableMaterial {
                    color_slot,
                    unlit: false,
                },
            ));
            color_slot += 1;
        }
    }

    commands.spawn((
        Name::new("ui-camera"),
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));
    commands.spawn((
        Name::new("key-light"),
        PointLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    commands.spawn((
        Name::new("camera"),
        CameraTarget,
        SceneState::default(),
        Camera3d::default(),
        Transform::from_translation(CAMERA_POSITION).looking_at(Vec3::ZERO, Vec3::Y),
        #[cfg(feature = "automation")]
        AutomationTarget,
        #[cfg(target_arch = "wasm32")]
        Msaa::Off,
    ));

    let text_style = TextFont::default();
    let label_text_style = (text_style.clone(), TextColor(ORANGE.into()));
    commands.spawn((
        Name::new("controls"),
        Text::new("Up / Down - Increase / Decrease Alpha\nLeft / Right - Rotate Camera\nH - Toggle HDR\nSpacebar - Toggle Unlit\nC - Change Colors"),
        text_style.clone(),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
    ));
    commands.spawn((
        Name::new("scene-display"),
        Text::default(),
        text_style,
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            right: px(12),
            ..default()
        },
        StatusDisplay,
    ));

    for ((entity, name), lines) in spheres.into_iter().zip([4, 3, 2, 1, 0]) {
        let stem = "│\n".repeat(lines);
        commands.spawn((
            Name::new(format!("label-{name}")),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            ProjectedLabel { entity },
            children![(
                Text::new(format!("┌─ {}\n{stem}", title(name))),
                label_text_style.clone(),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::ZERO,
                    ..default()
                },
                TextLayout::default().with_no_wrap(),
            )],
        ));
    }
}

fn title(name: &str) -> &str {
    match name {
        "opaque" => "Opaque",
        "blend" => "Blend",
        "premultiplied" => "Premultiplied",
        "add" => "Add",
        "multiply" => "Multiply",
        _ => name,
    }
}

fn control_scene(
    mut materials: ResMut<Assets<StandardMaterial>>,
    controllable: Query<(&MeshMaterial3d<StandardMaterial>, &ControllableMaterial)>,
    camera: Single<(Entity, &mut Transform, Has<Hdr>, &mut SceneState), With<CameraTarget>>,
    mut display: Single<&mut Text, With<StatusDisplay>>,
    mut clear_color: ResMut<ClearColor>,
    time: Res<Time>,
    input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    let (camera_entity, mut camera_transform, has_hdr, mut state) = camera.into_inner();
    if input.pressed(KeyCode::ArrowUp) {
        state.alpha = (state.alpha + time.delta_secs()).min(1.0);
    } else if input.pressed(KeyCode::ArrowDown) {
        state.alpha = (state.alpha - time.delta_secs()).max(0.0);
    }

    if input.just_pressed(KeyCode::Space) {
        state.unlit = !state.unlit;
    }
    if input.just_pressed(KeyCode::KeyH) {
        state.hdr = !has_hdr;
        if has_hdr {
            commands.entity(camera_entity).remove::<Hdr>();
        } else {
            commands.entity(camera_entity).insert(Hdr);
        }
    } else {
        state.hdr = has_hdr;
    }
    if input.just_pressed(KeyCode::KeyC) {
        state.color_changes = state.color_changes.saturating_add(1);
    }

    for (material_handle, controls) in &controllable {
        let Some(mut material) = materials.get_mut(material_handle) else {
            continue;
        };
        if state.color_changes > 0 {
            let color = deterministic_color(
                state.seed,
                state.color_changes,
                controls.color_slot,
                state.alpha,
            );
            material.base_color = Color::srgba(color[0], color[1], color[2], color[3]);
            if controls.color_slot < state.colors.len() {
                state.colors[controls.color_slot] = color;
            }
        } else {
            material.base_color.set_alpha(state.alpha);
            if controls.color_slot < state.colors.len() {
                state.colors[controls.color_slot][3] = state.alpha;
            }
        }
        if controls.unlit {
            material.unlit = state.unlit;
        }
    }

    let rotation = if input.pressed(KeyCode::ArrowLeft) {
        time.delta_secs()
    } else if input.pressed(KeyCode::ArrowRight) {
        -time.delta_secs()
    } else {
        0.0
    };
    state.camera_angle += rotation;
    camera_transform.rotate_around(Vec3::ZERO, Quat::from_rotation_y(rotation));
    clear_color.0 = Color::srgb(
        0.03 + state.camera_angle.sin().abs() * 0.12,
        0.03 + (1.0 - state.alpha) * 0.12,
        if state.hdr { 0.12 } else { 0.05 },
    );
    display.0 = format!(
        "  HDR: {}\nUnlit: {}\nAlpha: {:.2}\nCamera angle: {:.2}\nSeed: {}\nColor changes: {}",
        if state.hdr { "ON " } else { "OFF" },
        if state.unlit { "ON " } else { "OFF" },
        state.alpha,
        state.camera_angle,
        state.seed,
        state.color_changes,
    );
}

fn deterministic_color(seed: u64, change: u32, slot: usize, alpha: f32) -> [f32; 4] {
    let stream = seed
        ^ u64::from(change).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (slot as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let mut rng = StdRng::seed_from_u64(stream);
    [rng.random(), rng.random(), rng.random(), alpha]
}

fn position_labels(
    camera: Single<(&Camera, &GlobalTransform), With<CameraTarget>>,
    mut labels: Query<(&mut Node, &ProjectedLabel)>,
    labeled: Query<&GlobalTransform>,
) {
    let (camera, camera_transform) = camera.into_inner();
    for (mut node, label) in &mut labels {
        let Ok(transform) = labeled.get(label.entity) else {
            continue;
        };
        let Ok(viewport_position) =
            camera.world_to_viewport(camera_transform, transform.translation() + Vec3::Y)
        else {
            continue;
        };
        node.top = px(viewport_position.y);
        node.left = px(viewport_position.x);
    }
}

#[cfg(test)]
mod blend_modes_tests {
    use super::*;
    use std::time::Duration;

    fn controls_app(seed: u64) -> (App, Entity, Handle<StandardMaterial>) {
        let mut app = App::new();
        app.init_resource::<Assets<StandardMaterial>>()
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(Time::<()>::default())
            .insert_resource(ClearColor::default())
            .add_systems(Update, control_scene);
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgba(0.9, 0.2, 0.3, INITIAL_ALPHA),
                alpha_mode: AlphaMode::Blend,
                ..default()
            });
        app.world_mut().spawn((
            MeshMaterial3d(material.clone()),
            ControllableMaterial {
                color_slot: 0,
                unlit: true,
            },
        ));
        let mut state = SceneState::default();
        state.seed = seed;
        let camera = app
            .world_mut()
            .spawn((
                CameraTarget,
                Camera3d::default(),
                Transform::from_translation(CAMERA_POSITION).looking_at(Vec3::ZERO, Vec3::Y),
                state,
            ))
            .id();
        app.world_mut().spawn((Text::default(), StatusDisplay));
        (app, camera, material)
    }

    fn advance(app: &mut App, seconds: f32) {
        app.world_mut()
            .resource_mut::<Time<()>>()
            .advance_by(Duration::from_secs_f32(seconds));
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    #[test]
    fn held_arrows_change_camera_and_alpha_only_during_advanced_frames() {
        let (mut app, camera, _) = controls_app(COLOR_SEED);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowLeft);
        advance(&mut app, 0.25);
        advance(&mut app, 0.25);
        assert_eq!(
            app.world().get::<SceneState>(camera).unwrap().camera_angle,
            0.5
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ArrowLeft);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowDown);
        advance(&mut app, 0.2);
        let state = app.world().get::<SceneState>(camera).unwrap();
        assert_eq!(state.camera_angle, 0.5);
        assert!((state.alpha - 0.7).abs() < 0.0001);
    }

    #[test]
    fn mode_keys_toggle_hdr_and_unlit_on_separate_press_sequences() {
        let (mut app, camera, _) = controls_app(COLOR_SEED);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyH);
        advance(&mut app, 1.0 / 60.0);
        assert!(app.world().get::<SceneState>(camera).unwrap().hdr);
        assert!(app.world().entity(camera).contains::<Hdr>());

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyH);
        advance(&mut app, 1.0 / 60.0);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        advance(&mut app, 1.0 / 60.0);
        assert!(app.world().get::<SceneState>(camera).unwrap().unlit);
    }

    #[test]
    fn equal_seeds_and_color_key_sequences_produce_equal_observable_colors() {
        let (mut first, first_camera, first_material) = controls_app(42);
        let (mut second, second_camera, second_material) = controls_app(42);
        let (mut different_seed, different_camera, _) = controls_app(43);
        for app in [&mut first, &mut second, &mut different_seed] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyC);
            advance(app, 1.0 / 60.0);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyC);
            advance(app, 1.0 / 60.0);
        }

        let first_state = first.world().get::<SceneState>(first_camera).unwrap();
        let second_state = second.world().get::<SceneState>(second_camera).unwrap();
        assert_eq!(first_state.colors, second_state.colors);
        assert_ne!(
            &first_state.colors,
            &different_seed
                .world()
                .get::<SceneState>(different_camera)
                .unwrap()
                .colors
        );
        assert_ne!(first_state.colors[0], [0.9, 0.2, 0.3, INITIAL_ALPHA]);
        let first_color = first
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&first_material)
            .unwrap()
            .base_color;
        let second_color = second
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&second_material)
            .unwrap()
            .base_color;
        assert_eq!(first_color, second_color);
    }
}
