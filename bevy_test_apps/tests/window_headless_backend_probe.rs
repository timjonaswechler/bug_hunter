//! CPU-only feasibility probe for preserving a real Bevy window render target while preparing
//! a Render-World output attachment override. This test deliberately installs no Winit or render
//! plugins and never runs the Render schedule.

use bevy::{
    app::{App, Update},
    asset::{AssetEvent, Assets, Handle, RenderAssetUsages},
    camera::{
        Camera, Camera2d, Camera3d, NormalizedRenderTarget, OrthographicProjection,
        PerspectiveProjection, Projection, RenderTarget, Viewport,
    },
    ecs::schedule::IntoScheduleConfigs,
    image::Image,
    input::{
        ButtonInput, ButtonState,
        keyboard::{Key, KeyboardFocusLost, KeyboardInput, keyboard_input_system},
        mouse::{MouseButton, MouseButtonInput, mouse_button_input_system},
    },
    math::{UVec2, Vec2},
    picking::{
        backend::ray::RayMap,
        pointer::{Location, PointerId, PointerLocation},
    },
    prelude::{GlobalTransform, Transform},
    render::{
        Render, RenderSystems,
        camera::{ExtractedCamera, camera_system},
        gpu_readback::Readback,
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        texture::{GpuImage, ManualTextureViews, OutputColorAttachment},
        view::{
            ExtractedView, ViewTargetAttachments, prepare_view_attachments, prepare_view_targets,
        },
    },
    window::{
        PrimaryWindow, RawHandleWrapper, Window, WindowCreated, WindowRef, WindowResized,
        WindowResolution, WindowScaleFactorChanged,
    },
};

/// Maps already-normalized camera targets to internal output image assets. Multiple cameras with
/// the same target therefore retain one shared stack; independent targets remain independent.
#[derive(bevy::prelude::Resource)]
struct ProbeOutputImages(Vec<(NormalizedRenderTarget, Handle<Image>)>);

/// A compile-checked sketch of the public Render-World seam. It preserves the extracted camera's
/// normalized Window target and replaces only that target's final output attachment.
fn prepare_probe_window_outputs(
    cameras: bevy::prelude::Query<(&ExtractedCamera, &ExtractedView)>,
    gpu_images: bevy::prelude::Res<RenderAssets<GpuImage>>,
    outputs: bevy::prelude::Res<ProbeOutputImages>,
    mut attachments: bevy::prelude::ResMut<ViewTargetAttachments>,
) {
    for (camera, _view) in &cameras {
        let Some(target @ NormalizedRenderTarget::Window(_)) = camera.target.as_ref() else {
            continue;
        };
        let Some(size) = camera.physical_target_size else {
            continue;
        };
        let Some((_, image_handle)) = outputs.0.iter().find(|(candidate, _)| candidate == target)
        else {
            continue;
        };
        let Some(gpu_image) = gpu_images.get(image_handle) else {
            continue;
        };
        let texture_size = gpu_image.texture_descriptor.size;
        if size.x == 0
            || size.y == 0
            || texture_size.width != size.x
            || texture_size.height != size.y
            || attachments.contains_key(target)
        {
            continue;
        }

        attachments.insert(
            target.clone(),
            OutputColorAttachment::new(gpu_image.texture_view.clone(), gpu_image.view_format()),
        );
    }
}

fn register_public_render_world_hook(app: &mut App, outputs: ProbeOutputImages) {
    app.init_resource::<ViewTargetAttachments>()
        .insert_resource(outputs)
        .add_systems(
            Render,
            prepare_probe_window_outputs
                .after(prepare_assets::<GpuImage>)
                .after(prepare_view_attachments)
                .before(prepare_view_targets)
                .in_set(RenderSystems::PrepareViews),
        );
}

fn configure_cpu_camera_and_input_app() -> App {
    let mut app = App::new();

    app.add_message::<WindowResized>()
        .add_message::<WindowCreated>()
        .add_message::<WindowScaleFactorChanged>()
        .add_message::<AssetEvent<Image>>()
        .add_message::<KeyboardInput>()
        .add_message::<KeyboardFocusLost>()
        .add_message::<MouseButtonInput>()
        .init_resource::<Assets<Image>>()
        .init_resource::<ManualTextureViews>()
        .init_resource::<RayMap>()
        .init_resource::<ButtonInput<bevy::input::keyboard::KeyCode>>()
        .init_resource::<ButtonInput<Key>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_systems(
            Update,
            (
                camera_system,
                RayMap::repopulate.after(camera_system),
                keyboard_input_system,
                mouse_button_input_system,
            ),
        );

    app
}

fn assert_window_target(target: &RenderTarget) {
    assert!(matches!(target, RenderTarget::Window(WindowRef::Primary)));
}

#[test]
fn real_window_entity_drives_camera_picking_and_input_without_os_handles() {
    let mut app = configure_cpu_camera_and_input_app();

    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(800, 450).with_scale_factor_override(2.0),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();

    // The entity is a real Main-World Window/PrimaryWindow, but no Winit/raw-handle path exists.
    assert!(app.world().get::<Window>(window).is_some());
    assert!(app.world().get::<PrimaryWindow>(window).is_some());
    assert!(app.world().get::<RawHandleWrapper>(window).is_none());

    let camera_3d_transform = Transform::from_xyz(0.0, 0.0, 5.0);
    let camera_3d = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera {
                viewport: Some(Viewport {
                    physical_position: UVec2::new(100, 50),
                    physical_size: UVec2::new(400, 200),
                    ..Default::default()
                }),
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            Projection::Perspective(PerspectiveProjection {
                fov: 1.1,
                ..Default::default()
            }),
            camera_3d_transform,
            GlobalTransform::from(camera_3d_transform),
        ))
        .id();

    let camera_2d_transform = Transform::from_xyz(0.0, 0.0, 10.0);
    let camera_2d = app
        .world_mut()
        .spawn((
            Camera2d,
            Camera {
                order: 1,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            Projection::Orthographic(OrthographicProjection::default_2d()),
            camera_2d_transform,
            GlobalTransform::from(camera_2d_transform),
        ))
        .id();

    let normalized_target = RenderTarget::Window(WindowRef::Primary)
        .normalize(Some(window))
        .expect("the real PrimaryWindow entity must normalize the target");
    app.world_mut().spawn((
        PointerId::Mouse,
        PointerLocation::new(Location {
            target: normalized_target.clone(),
            // Logical center of the 3D camera's physical viewport at scale factor 2.
            position: Vec2::new(150.0, 75.0),
        }),
    ));

    app.update();

    let camera = app.world().get::<Camera>(camera_3d).unwrap();
    assert_eq!(camera.physical_target_size(), Some(UVec2::new(800, 450)));
    assert_eq!(camera.target_scaling_factor(), Some(2.0));
    assert_eq!(camera.physical_viewport_size(), Some(UVec2::new(400, 200)));
    assert_eq!(
        camera.logical_viewport_size(),
        Some(Vec2::new(200.0, 100.0))
    );
    assert_eq!(
        camera.logical_viewport_rect().unwrap().min,
        Vec2::new(50.0, 25.0)
    );
    assert_eq!(
        camera.logical_viewport_rect().unwrap().max,
        Vec2::new(250.0, 125.0)
    );
    assert_window_target(app.world().get::<RenderTarget>(camera_3d).unwrap());
    assert_window_target(app.world().get::<RenderTarget>(camera_2d).unwrap());
    assert_eq!(
        *app.world().get::<Transform>(camera_3d).unwrap(),
        camera_3d_transform
    );
    match app.world().get::<Projection>(camera_3d).unwrap() {
        Projection::Perspective(projection) => {
            assert_eq!(projection.fov, 1.1);
            assert_eq!(projection.aspect_ratio, 2.0);
        }
        _ => panic!("3D camera lost its perspective projection"),
    }

    // Bevy's own public RayMap path maps one pointer to both cameras in the same target stack.
    let rays = app.world().resource::<RayMap>();
    let ray_3d = rays
        .map
        .get(&bevy::picking::backend::ray::RayId::new(
            camera_3d,
            PointerId::Mouse,
        ))
        .expect("3D camera ray");
    assert!(ray_3d.direction.as_vec3().z < 0.0);
    assert!(
        rays.map
            .get(&bevy::picking::backend::ray::RayId::new(
                camera_2d,
                PointerId::Mouse,
            ))
            .is_some()
    );
    assert_eq!(rays.map.len(), 2);

    // Real Bevy input messages retain the Window entity identity without an OS event loop.
    app.world_mut().write_message(KeyboardInput {
        key_code: bevy::input::keyboard::KeyCode::KeyD,
        logical_key: Key::Character("d".into()),
        state: ButtonState::Pressed,
        text: Some("d".into()),
        repeat: false,
        window,
    });
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window,
    });
    app.update();
    assert!(
        app.world()
            .resource::<ButtonInput<bevy::input::keyboard::KeyCode>>()
            .pressed(bevy::input::keyboard::KeyCode::KeyD)
    );
    assert!(
        app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left)
    );

    // A synthetic backend can update the real Window and send the same public resize message.
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set_physical_resolution(1000, 500);
    app.world_mut().write_message(WindowResized {
        window,
        width: 500.0,
        height: 250.0,
    });
    app.update();

    let camera = app.world().get::<Camera>(camera_3d).unwrap();
    assert_eq!(camera.physical_target_size(), Some(UVec2::new(1000, 500)));
    assert_eq!(camera.target_scaling_factor(), Some(2.0));
    assert_window_target(app.world().get::<RenderTarget>(camera_3d).unwrap());
    assert_eq!(
        *app.world().get::<Transform>(camera_3d).unwrap(),
        camera_3d_transform
    );
    match app.world().get::<Projection>(camera_3d).unwrap() {
        Projection::Perspective(projection) => {
            assert_eq!(projection.fov, 1.1);
            // The explicit viewport is unchanged, so its aspect remains unchanged too.
            assert_eq!(projection.aspect_ratio, 2.0);
        }
        _ => panic!("3D camera lost its perspective projection after resize"),
    }
}

#[test]
fn public_render_world_attachment_and_readback_chain_registers_without_gpu_code() {
    let mut app = App::new();
    app.init_schedule(Render).init_resource::<Assets<Image>>();

    let mut image = Image::new_fill(
        Extent3d {
            width: 800,
            height: 450,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC;
    let image_handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let target = RenderTarget::Window(WindowRef::Entity(bevy::prelude::Entity::PLACEHOLDER))
        .normalize(None)
        .unwrap();

    register_public_render_world_hook(
        &mut app,
        ProbeOutputImages(vec![(target, image_handle.clone())]),
    );
    let request = app.world_mut().spawn(Readback::texture(image_handle)).id();

    assert!(app.world().contains_resource::<ViewTargetAttachments>());
    assert!(app.world().contains_resource::<ProbeOutputImages>());
    assert!(matches!(
        app.world().get::<Readback>(request),
        Some(Readback::Texture(_))
    ));
    // Deliberately no app.update() or Render schedule run: this is API/scheduling evidence only.
}
