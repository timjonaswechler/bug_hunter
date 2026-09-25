//! Fixed procedural 3D fixture for CLI -> Session -> Capture acceptance.
//!
//! Scope is deliberately narrow: one immutable 321x181 image target at scale
//! 1.5, one fixed perspective camera, two opaque unlit cuboids, and one UI
//! overlay. The existing virtual keyboard path can move only the front cuboid
//! along world X. There is no window, pointer input, external asset, hot reload,
//! viewport, camera motion, or target/projection mutation.
use bevy::{
    app::ScheduleRunnerPlugin,
    camera::{ImageRenderTarget, PerspectiveProjection, Projection, RenderTarget},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
    render::render_resource::TextureFormat,
    time::TimeUpdateStrategy,
    window::ExitCondition,
};
use std::time::Duration;

const WIDTH: u32 = 321;
const HEIGHT: u32 = 181;
const SCALE: f32 = 1.5;
const FOV: f32 = std::f32::consts::FRAC_PI_4;
const NEAR: f32 = 0.1;
const FAR: f32 = 100.0;
const CAMERA_POSITION: Vec3 = Vec3::new(0.0, 0.0, 8.0);
const BACK_POSITION: Vec3 = Vec3::new(-0.4, 0.0, 0.0);
const FRONT_POSITION: Vec3 = Vec3::new(0.4, 0.0, 2.0);
const BACK_SIZE: f32 = 3.0;
const FRONT_SIZE: f32 = 1.5;
const FRONT_STEP_PER_TICK: f32 = 0.08;

#[derive(Resource, Reflect, Default, Debug, PartialEq)]
#[reflect(Resource)]
struct SceneState {
    ticks: u64,
    virtual_millis: u64,
    camera_translation: [f32; 3],
    camera_rotation: [f32; 4],
    camera_vertical_fov_radians: f32,
    camera_aspect_ratio: f32,
    camera_near: f32,
    camera_far: f32,
    target_physical_size: [u32; 2],
    target_scale_factor: f32,
    back_translation: [f32; 3],
    front_translation: [f32; 3],
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
struct Headless3dInfo {
    physical_width: u32,
    physical_height: u32,
    scale_factor: f32,
    vertical_fov_radians: f32,
    near: f32,
    far: f32,
    front_step_per_tick: f32,
}

#[derive(Component)]
struct GameCamera;

#[derive(Component)]
struct BackCuboid;

#[derive(Component)]
struct FrontCuboid;

type CameraComponents<'a> = (&'a Transform, &'a Projection, &'a Camera, &'a RenderTarget);
type BackCuboidFilter = (With<BackCuboid>, Without<GameCamera>, Without<FrontCuboid>);
type FrontCuboidFilter = (With<FrontCuboid>, Without<GameCamera>, Without<BackCuboid>);

fn unlit(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    }
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let image = images.add(Image::new_target_texture(
        WIDTH,
        HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let camera = commands
        .spawn((
            Name::new("headless-3d-camera"),
            Camera3d::default(),
            Camera {
                is_active: false,
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: FOV,
                aspect_ratio: WIDTH as f32 / HEIGHT as f32,
                near: NEAR,
                far: FAR,
                ..default()
            }),
            Transform::from_translation(CAMERA_POSITION).looking_at(Vec3::ZERO, Vec3::Y),
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            RenderTarget::Image(ImageRenderTarget {
                handle: image,
                scale_factor: SCALE,
            }),
            GameCamera,
            woodpecker::session::HeadlessCaptureCamera3d,
        ))
        .id();

    commands.spawn((
        Name::new("blue-back-cuboid"),
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::splat(BACK_SIZE)))),
        MeshMaterial3d(materials.add(unlit(Color::srgb(0.0, 0.0, 1.0)))),
        Transform::from_translation(BACK_POSITION),
        BackCuboid,
    ));
    commands.spawn((
        Name::new("red-front-cuboid"),
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::splat(FRONT_SIZE)))),
        MeshMaterial3d(materials.add(unlit(Color::srgb(1.0, 0.0, 0.0)))),
        Transform::from_translation(FRONT_POSITION),
        FrontCuboid,
    ));
    commands.spawn((
        Name::new("green-ui-overlay"),
        Node {
            position_type: PositionType::Absolute,
            left: px(8.0),
            top: px(8.0),
            width: px(48.0),
            height: px(24.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.0, 1.0, 0.0)),
        UiTargetCamera(camera),
    ));
}

fn move_front_cuboid(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut front: Single<&mut Transform, With<FrontCuboid>>,
) {
    if keyboard.pressed(KeyCode::KeyD) {
        front.translation.x += FRONT_STEP_PER_TICK;
    }
}

fn observe_scene(
    time: Res<Time<Virtual>>,
    camera_state: Single<CameraComponents<'_>, With<GameCamera>>,
    back: Single<&Transform, BackCuboidFilter>,
    front: Single<&Transform, FrontCuboidFilter>,
    mut state: ResMut<SceneState>,
) {
    state.ticks += 1;
    state.virtual_millis = time.elapsed().as_millis() as u64;
    let (camera_transform, camera_projection, camera, render_target) = *camera_state;
    state.camera_translation = camera_transform.translation.to_array();
    state.camera_rotation = camera_transform.rotation.to_array();
    let Projection::Perspective(projection) = camera_projection else {
        unreachable!("fixed 3D fixture uses a perspective projection")
    };
    state.camera_vertical_fov_radians = projection.fov;
    state.camera_aspect_ratio = projection.aspect_ratio;
    state.camera_near = projection.near;
    state.camera_far = projection.far;
    let target_info = camera
        .computed
        .target_info
        .as_ref()
        .expect("camera geometry initialized by the explicit tick");
    state.target_physical_size = target_info.physical_size.to_array();
    let RenderTarget::Image(render_target) = render_target else {
        unreachable!("fixed 3D fixture uses an image target")
    };
    state.target_scale_factor = render_target.scale_factor;
    state.back_translation = back.translation.to_array();
    state.front_translation = front.translation.to_array();
}

fn assert_no_window(world: &mut World) {
    assert_eq!(world.query::<&Window>().iter(world).count(), 0);
}

fn activate_game_camera(mut camera: Single<&mut Camera, With<GameCamera>>) {
    camera.is_active = true;
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<bevy::audio::AudioPlugin>()
                .disable::<bevy::gilrs::GilrsPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(1)))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )))
        .init_resource::<SceneState>()
        .insert_resource(Headless3dInfo {
            physical_width: WIDTH,
            physical_height: HEIGHT,
            scale_factor: SCALE,
            vertical_fov_radians: FOV,
            near: NEAR,
            far: FAR,
            front_step_per_tick: FRONT_STEP_PER_TICK,
        })
        .register_type::<SceneState>()
        .register_type::<Headless3dInfo>()
        .add_systems(Startup, setup)
        .add_systems(PostStartup, assert_no_window)
        // PostStartup initializes the image-target geometry while the camera is
        // inactive. First activates it inside the one explicit initialization
        // tick, before PostUpdate computes nonzero 3D cluster dimensions.
        .add_systems(First, activate_game_camera.run_if(run_once))
        .add_systems(Update, move_front_cuboid)
        .add_systems(Last, observe_scene)
        .add_plugins(woodpecker::session::Plugin)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        app::MainScheduleOrder,
        asset::{AssetApp, AssetPlugin},
        camera::{CameraPlugin, CameraProjection, CameraUpdateSystems},
        image::ImagePlugin,
        light::{
            LightPlugin,
            cluster::{Clusters, GlobalClusterGpuSettings, GlobalClusterSettings},
        },
        render::{camera::camera_system, texture::ManualTextureViews},
        transform::TransformPlugin,
    };

    #[test]
    fn camera_waits_for_first_simulation_tick_before_clustered_rendering() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            },
            AssetPlugin::default(),
            ImagePlugin::default(),
            bevy::mesh::MeshPlugin,
            CameraPlugin,
            LightPlugin,
        ))
        .init_asset::<StandardMaterial>()
        .init_asset::<bevy::gizmos::GizmoAsset>()
        .init_resource::<ManualTextureViews>()
        .insert_resource(GlobalClusterSettings {
            supports_storage_buffers: true,
            clustered_decals_are_usable: false,
            gpu_clustering: Some(GlobalClusterGpuSettings {
                initial_z_slice_list_capacity: 4096,
                initial_index_list_capacity: 65536,
            }),
            max_uniform_buffer_clusterable_objects: 204,
            view_cluster_bindings_max_indices: 65536,
        })
        .add_systems(Startup, setup)
        .add_systems(PostStartup, camera_system.in_set(CameraUpdateSystems))
        .add_systems(
            PostUpdate,
            camera_system
                .in_set(CameraUpdateSystems)
                .before(bevy::asset::AssetEventSystems),
        )
        .add_systems(First, activate_game_camera.run_if(run_once));

        assert!(
            app.world()
                .get_resource::<bevy::render::renderer::RenderDevice>()
                .is_none()
        );
        let simulation_schedules = {
            let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
            std::mem::take(&mut order.labels)
        };
        app.update();

        let world = app.world_mut();
        assert_eq!(world.query::<&Window>().iter(world).count(), 0);
        let mut camera_query = world.query_filtered::<(&Camera, &Clusters), With<GameCamera>>();
        let (camera, clusters) = camera_query.single(world).unwrap();
        assert_eq!(
            camera.physical_target_size(),
            Some(UVec2::new(WIDTH, HEIGHT))
        );
        assert_eq!(clusters.dimensions, UVec3::ZERO);
        assert!(
            !camera.is_active,
            "an active pre-warp camera exposes zero cluster dimensions to extraction"
        );

        for schedule in simulation_schedules {
            let _ = world.try_run_schedule(schedule);
        }

        let (camera, clusters) = camera_query.single(world).unwrap();
        assert!(camera.is_active);
        assert!(clusters.dimensions.cmpgt(UVec3::ZERO).all());
    }

    #[test]
    fn held_d_moves_only_the_front_cuboid_once_per_game_tick() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, move_front_cuboid);
        let front = app
            .world_mut()
            .spawn((Transform::from_translation(FRONT_POSITION), FrontCuboid))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Transform>(front).unwrap().translation,
            FRONT_POSITION
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyD);
        for _ in 0..10 {
            app.update();
        }
        let moved_x = app.world().get::<Transform>(front).unwrap().translation.x;
        assert!((moved_x - 1.2).abs() <= f32::EPSILON * 8.0);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyD);
        app.update();
        assert_eq!(
            app.world().get::<Transform>(front).unwrap().translation.x,
            moved_x
        );
    }

    fn viewport_point(world: Vec3) -> Vec2 {
        let projection = PerspectiveProjection {
            fov: FOV,
            aspect_ratio: WIDTH as f32 / HEIGHT as f32,
            near: NEAR,
            far: FAR,
            ..default()
        };
        let view_from_world = Transform::from_translation(CAMERA_POSITION)
            .looking_at(Vec3::ZERO, Vec3::Y)
            .to_matrix()
            .inverse();
        let clip = projection.get_clip_from_view() * view_from_world * world.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        Vec2::new(
            (ndc.x + 1.0) * WIDTH as f32 * 0.5,
            (1.0 - ndc.y) * HEIGHT as f32 * 0.5,
        )
    }

    #[test]
    fn fixed_projection_places_front_cuboid_over_part_of_back_cuboid() {
        let back_front_z = BACK_POSITION.z + BACK_SIZE * 0.5;
        let front_front_z = FRONT_POSITION.z + FRONT_SIZE * 0.5;
        let back_min = viewport_point(Vec3::new(
            BACK_POSITION.x - BACK_SIZE * 0.5,
            BACK_POSITION.y + BACK_SIZE * 0.5,
            back_front_z,
        ));
        let back_max = viewport_point(Vec3::new(
            BACK_POSITION.x + BACK_SIZE * 0.5,
            BACK_POSITION.y - BACK_SIZE * 0.5,
            back_front_z,
        ));
        let front_min = viewport_point(Vec3::new(
            FRONT_POSITION.x - FRONT_SIZE * 0.5,
            FRONT_POSITION.y + FRONT_SIZE * 0.5,
            front_front_z,
        ));
        let front_max = viewport_point(Vec3::new(
            FRONT_POSITION.x + FRONT_SIZE * 0.5,
            FRONT_POSITION.y - FRONT_SIZE * 0.5,
            front_front_z,
        ));

        assert!(back_min.x < 110.0 && back_max.x > 190.0);
        assert!(front_min.x < 155.0 && front_max.x > 195.0);
        let overlap = Vec2::new(170.0, 90.0);
        assert!(overlap.cmpge(back_min).all() && overlap.cmple(back_max).all());
        assert!(overlap.cmpge(front_min).all() && overlap.cmple(front_max).all());
        assert!(CAMERA_POSITION.z - front_front_z < CAMERA_POSITION.z - back_front_z);
        assert!(CAMERA_POSITION.z - front_front_z > NEAR);
        assert!(CAMERA_POSITION.z - back_front_z < FAR);

        let moved_front = FRONT_POSITION + Vec3::X * FRONT_STEP_PER_TICK * 10.0;
        let moved_min = viewport_point(Vec3::new(
            moved_front.x - FRONT_SIZE * 0.5,
            moved_front.y + FRONT_SIZE * 0.5,
            front_front_z,
        ));
        let moved_max = viewport_point(Vec3::new(
            moved_front.x + FRONT_SIZE * 0.5,
            moved_front.y - FRONT_SIZE * 0.5,
            front_front_z,
        ));
        let newly_exposed_blue = Vec2::new(160.0, 90.0);
        assert!(newly_exposed_blue.cmpge(back_min).all());
        assert!(newly_exposed_blue.cmple(back_max).all());
        assert!(
            !(newly_exposed_blue.cmpge(moved_min).all()
                && newly_exposed_blue.cmple(moved_max).all())
        );
        let moved_red = Vec2::new(215.0, 90.0);
        assert!(moved_red.cmpge(moved_min).all() && moved_red.cmple(moved_max).all());
        assert!(moved_min.x > 175.0 && moved_max.x < 245.0);
    }
}
