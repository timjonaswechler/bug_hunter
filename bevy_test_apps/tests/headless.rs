//! Isolated integration probe, not a replacement for the session adapters.
//! Run explicitly with `cargo test --manifest-path bevy_test_apps/Cargo.toml
//! --test headless -- --ignored --nocapture`.
use bevy::{
    app::{MainScheduleOrder, PluginsState},
    asset::{AssetEvent, AssetId, handle_internal_asset_events},
    camera::{ImageRenderTarget, RenderTarget},
    core_pipeline::{blit::BlitPipeline, core_2d::Transparent2d, upscaling::ViewUpscalingPipeline},
    input::mouse::MouseMotion,
    picking::{
        PickingSystems,
        backend::ray::{RayId, RayMap},
        pointer::{
            Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
        },
    },
    prelude::*,
    render::{
        Extract, ExtractSchedule, RenderApp,
        camera::ExtractedCamera,
        render_asset::RenderAssets,
        render_phase::ViewSortedRenderPhases,
        render_resource::{
            BufferDescriptor, BufferUsages, CommandEncoderDescriptor, MapMode, PipelineCache,
            SpecializedRenderPipelines, TexelCopyBufferInfo, TexelCopyBufferLayout, TextureFormat,
            TextureUsages,
        },
        renderer::{RenderAdapterInfo, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue},
        texture::GpuImage,
        view::ViewTarget,
    },
    shader::Shader,
    time::TimeUpdateStrategy,
    ui_render::TransparentUi,
    window::ExitCondition,
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[path = "headless/readiness.rs"]
mod readiness;

const WIDTH: u32 = 321; // Exercise padded GPU rows.
const HEIGHT: u32 = 181;
const SCALE: f32 = 1.5;
const POINTER: PointerId = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(25));

#[derive(Resource, Default, Clone, Debug, PartialEq)]
struct Simulation {
    ticks: u64,
    pre_updates: u64,
    post_updates: u64,
    fixed_updates: u64,
    movement: f32,
    angle: f32,
    world_clicks: u32,
    ui_clicks: u32,
}

#[derive(Resource, Clone)]
struct Scene {
    image: Handle<Image>,
    camera: Entity,
    object: Entity,
    button: Entity,
}

#[derive(Component)]
struct GameCamera;

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = images.add(image);
    commands.spawn(POINTER);
    let camera = commands
        .spawn((
            Camera2d,
            Msaa::Off,
            RenderTarget::Image(ImageRenderTarget {
                handle: image.clone(),
                scale_factor: SCALE,
            }),
            GameCamera,
        ))
        .id();
    let object = commands
        .spawn((
            Name::new("world-object"),
            Sprite::from_color(Color::srgb(1., 0., 0.), Vec2::splat(40.)),
            Pickable::default(),
            Transform::default(),
        ))
        .observe(|_: On<Pointer<Press>>, mut state: ResMut<Simulation>| {
            state.world_clicks += 1;
        })
        .id();
    let button = commands
        .spawn((
            Name::new("overlay-button"),
            Node {
                position_type: PositionType::Absolute,
                left: px(8.),
                top: px(8.),
                width: px(48.),
                height: px(24.),
                ..default()
            },
            BackgroundColor(Color::srgb(0., 1., 0.)),
            UiTargetCamera(camera),
        ))
        .observe(|_: On<Pointer<Press>>, mut state: ResMut<Simulation>| {
            state.ui_clicks += 1;
        })
        .id();
    commands.insert_resource(Scene {
        image,
        camera,
        object,
        button,
    });
}

fn simulate(
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: MessageReader<MouseMotion>,
    mut state: ResMut<Simulation>,
    mut camera: Single<&mut Transform, With<GameCamera>>,
) {
    state.ticks += 1;
    if keys.pressed(KeyCode::KeyD) {
        state.movement += 2.;
        camera.translation.x += 2.;
    }
    for event in motion.read() {
        state.angle += event.delta.x * 0.001;
        camera.rotation = Quat::from_rotation_z(state.angle);
    }
}

// Bevy 0.19.1's Location::is_in_viewport requires a PrimaryWindow even for
// Image targets. Use the documented RayMap extension instead of a fake Window.
fn image_rays(
    scene: Res<Scene>,
    cameras: Query<(&Camera, &RenderTarget, &GlobalTransform)>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    mut rays: ResMut<RayMap>,
) {
    let (camera, target, transform) = cameras.get(scene.camera).unwrap();
    if !camera.is_active {
        return;
    }
    for (id, pointer) in &pointers {
        let Some(location) = &pointer.location else {
            continue;
        };
        if target.normalize(None).as_ref() == Some(&location.target)
            && camera
                .logical_viewport_rect()
                .is_some_and(|rect| rect.contains(location.position))
            && let Ok(ray) = camera.viewport_to_world(transform, location.position)
        {
            rays.map.insert(RayId::new(scene.camera, *id), ray);
        }
    }
}

// Each request owns its completion channel. Only the copy system can claim it.
// A prepared or mapped buffer without this request's copy never produces a result.
type CaptureResult = Result<Vec<u8>, String>;

#[derive(Clone)]
struct Capture {
    id: u64,
    image: Handle<Image>,
    expect_content: bool,
    result: Arc<Mutex<Option<mpsc::Sender<CaptureResult>>>>,
}

#[derive(Resource, Default, Clone)]
struct Request(Option<Capture>);

fn extract(request: Extract<Res<Request>>, mut output: ResMut<Request>) {
    *output = request.clone();
}

fn copy_after_render(world: &mut World) {
    let Some(request) = world.resource::<Request>().0.clone() else {
        return;
    };
    if request.result.lock().unwrap().is_none() {
        return;
    }
    let views: Vec<_> = world
        .query::<(&ExtractedCamera, &ViewTarget, Has<ViewUpscalingPipeline>)>()
        .iter(world)
        .map(
            |(camera, view, has_upscaling_pipeline)| readiness::OutputView {
                image: match &camera.target {
                    Some(bevy::camera::NormalizedRenderTarget::Image(image)) => {
                        Some(image.handle.id())
                    }
                    _ => None,
                },
                has_upscaling_pipeline,
                attachment_selected: view.needs_present(),
                key: readiness::upscaling_key(
                    &camera.output_mode,
                    camera.sorted_camera_index_for_target,
                    view.out_texture_view_format(),
                    view.compositing_space,
                ),
            },
        )
        .collect();
    let final_output_ready = world.resource_scope(
        |world, mut pipelines: Mut<SpecializedRenderPipelines<BlitPipeline>>| {
            let cache = world.resource::<PipelineCache>();
            readiness::output_ready(request.image.id(), views, |key| {
                // Same resource and key as Bevy's Prepare system, not a new cache
                // or the global waiting set. A newly queued entry is not Ready.
                let id = pipelines.specialize(cache, world.resource::<BlitPipeline>(), key);
                readiness::pipeline_status(Some(cache.get_render_pipeline_state(id)))
            })
        },
    );
    if !final_output_ready {
        return;
    }
    let cache = world.resource::<PipelineCache>();
    let sprites = world.resource::<ViewSortedRenderPhases<Transparent2d>>();
    let ui = world.resource::<ViewSortedRenderPhases<TransparentUi>>();
    let sprite_items: Vec<_> = sprites
        .values()
        .flat_map(|phase| phase.items.values())
        .collect();
    let ui_items: Vec<_> = ui.values().flat_map(|phase| phase.items.values()).collect();
    // This probe owns exactly one camera, one sprite, one solid UI node and no
    // dynamically loaded scene assets. Do not reuse these counts as a general gate.
    if request.expect_content && (sprite_items.is_empty() || ui_items.is_empty()) {
        return;
    }
    if sprite_items
        .iter()
        .any(|item| cache.get_render_pipeline(item.pipeline).is_none())
        || ui_items
            .iter()
            .any(|item| cache.get_render_pipeline(item.pipeline).is_none())
        || cache.waiting_pipelines().next().is_some()
    {
        return;
    }
    let images = world.resource::<RenderAssets<GpuImage>>();
    let Some(image) = images.get(&request.image) else {
        return;
    };
    let device = world.resource::<RenderDevice>();
    let stride = RenderDevice::align_copy_bytes_per_row(WIDTH as usize * 4);
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("headless-probe-readback"),
        size: stride as u64 * HEIGHT as u64,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        image.texture.as_image_copy(),
        TexelCopyBufferInfo {
            buffer: &buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride as u32),
                rows_per_image: None,
            },
        },
        image.texture_descriptor.size,
    );
    world.resource::<RenderQueue>().submit([encoder.finish()]);
    let sender = request.result.lock().unwrap().take().unwrap();
    let mapped = buffer.clone();
    buffer.slice(..).map_async(MapMode::Read, move |result| {
        let result = result
            .map_err(|error| format!("capture {}: {error}", request.id))
            .map(|()| {
                let view = mapped.slice(..).get_mapped_range();
                let pixels = view
                    .chunks_exact(stride)
                    .flat_map(|row| row[..WIDTH as usize * 4].iter().copied())
                    .collect();
                drop(view);
                mapped.unmap();
                pixels
            });
        let _ = sender.send(result);
    });
}

struct Probe {
    app: App,
    schedules: Vec<bevy::ecs::schedule::InternedScheduleLabel>,
    next_request: u64,
    shaders: HashSet<AssetId<Shader>>,
    inputs: Vec<Input>,
}

enum Input {
    Key(bool),
    Look(Vec2),
    Pointer(Vec2, PointerAction),
}

impl Probe {
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(
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
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )))
        .init_resource::<Simulation>()
        .init_resource::<Request>()
        .add_systems(Startup, setup)
        .add_systems(
            PreUpdate,
            image_rays
                .after(RayMap::repopulate)
                .in_set(PickingSystems::ProcessInput),
        )
        .add_systems(PreUpdate, |mut state: ResMut<Simulation>| {
            state.pre_updates += 1
        })
        .add_systems(PostUpdate, |mut state: ResMut<Simulation>| {
            state.post_updates += 1
        })
        .add_systems(FixedUpdate, |mut state: ResMut<Simulation>| {
            state.fixed_updates += 1
        })
        .add_systems(Update, simulate);
        app.sub_app_mut(RenderApp)
            .init_resource::<Request>()
            .add_systems(ExtractSchedule, extract)
            .add_systems(
                RenderGraph,
                copy_after_render.in_set(RenderGraphSystems::Finish),
            );
        let deadline = Instant::now() + Duration::from_secs(30);
        while app.plugins_state() != PluginsState::Ready {
            assert!(
                Instant::now() < deadline,
                "renderer initialization timed out"
            );
            bevy::tasks::tick_global_task_pools_on_main_thread();
            std::thread::yield_now();
        }
        app.finish();
        app.cleanup();
        let adapter = app.world().resource::<RenderAdapterInfo>();
        eprintln!("headless adapter: {} / {:?}", adapter.name, adapter.backend);
        let schedules =
            std::mem::take(&mut app.world_mut().resource_mut::<MainScheduleOrder>().labels);
        // Startup only. No First/PreUpdate/Update/PostUpdate or simulation time.
        app.update();
        assert_eq!(app.world().resource::<Simulation>().ticks, 0);
        assert_eq!(
            app.world_mut().query::<&Window>().iter(app.world()).count(),
            0
        );
        assert!(!app.is_plugin_added::<bevy::winit::WinitPlugin>());
        Self {
            app,
            schedules,
            next_request: 1,
            shaders: HashSet::new(),
            inputs: Vec::new(),
        }
    }

    fn snapshot(&self) -> (Simulation, Duration, Transform, Transform) {
        let world = self.app.world();
        let scene = world.resource::<Scene>();
        (
            world.resource::<Simulation>().clone(),
            world.resource::<Time<Virtual>>().elapsed(),
            *world.get::<Transform>(scene.camera).unwrap(),
            *world.get::<Transform>(scene.object).unwrap(),
        )
    }

    fn pump(&mut self) {
        // Narrow probe-only asset maintenance: the fixed scene loads no assets
        // after its initialization tick except built-in shaders. Publish each
        // loaded shader once. This does not support hot reload or general assets
        // and must NOT be copied into the production session adapter.
        handle_internal_asset_events(self.app.world_mut());
        let added: Vec<_> = self
            .app
            .world()
            .resource::<Assets<Shader>>()
            .ids()
            .filter(|id| self.shaders.insert(*id))
            .collect();
        for id in added {
            self.app
                .world_mut()
                .write_message(AssetEvent::<Shader>::Added { id });
        }
        if let Some(receiver) = self.app.world().get_resource::<bevy::time::TimeReceiver>() {
            for _ in receiver.0.try_iter() {}
        }
        self.app.update();
        std::thread::sleep(Duration::from_millis(1));
    }

    fn warp(&mut self) {
        let world = self.app.world_mut();
        let scene = world.resource::<Scene>().clone();
        for input in self.inputs.drain(..) {
            match input {
                Input::Key(down) => {
                    world.write_message(bevy::input::keyboard::KeyboardInput {
                        key_code: KeyCode::KeyD,
                        logical_key: bevy::input::keyboard::Key::Character("d".into()),
                        state: if down {
                            bevy::input::ButtonState::Pressed
                        } else {
                            bevy::input::ButtonState::Released
                        },
                        text: None,
                        repeat: false,
                        // A live input-source entity, NOT a Window component or OS handle.
                        window: scene.camera,
                    });
                }
                Input::Look(delta) => {
                    world.write_message(MouseMotion { delta });
                }
                Input::Pointer(position, action) => {
                    world.write_message(PointerInput::new(
                        POINTER,
                        Location {
                            target: RenderTarget::Image(ImageRenderTarget {
                                handle: scene.image.clone(),
                                scale_factor: SCALE,
                            })
                            .normalize(None)
                            .unwrap(),
                            position,
                        },
                        action,
                    ));
                }
            }
        }
        for label in &self.schedules {
            world.run_schedule(*label);
        }
    }

    fn capture(&mut self, content: bool, timeout: Duration) -> Option<Vec<u8>> {
        let before = self.snapshot();
        let (sender, receiver) = mpsc::channel();
        let request = Capture {
            id: self.next_request,
            image: self.app.world().resource::<Scene>().image.clone(),
            expect_content: content,
            result: Arc::new(Mutex::new(Some(sender))),
        };
        self.next_request += 1;
        self.app.world_mut().resource_mut::<Request>().0 = Some(request);
        let deadline = Instant::now() + timeout;
        let result = loop {
            self.pump();
            assert_eq!(
                self.snapshot(),
                before,
                "capture changed simulation, time or transforms"
            );
            match receiver.try_recv() {
                Ok(result) => break Some(result.expect("GPU mapping failed")),
                Err(mpsc::TryRecvError::Disconnected) => panic!("lost capture completion"),
                Err(mpsc::TryRecvError::Empty) if Instant::now() < deadline => {}
                Err(mpsc::TryRecvError::Empty) => break None,
            }
        };
        self.app.world_mut().resource_mut::<Request>().0 = None;
        result
    }

    fn pointer(&mut self, position: Vec2, action: PointerAction) {
        self.inputs.push(Input::Pointer(position, action));
    }
}

fn pixel(bytes: &[u8], x: u32, y: u32) -> &[u8] {
    let offset = ((y * WIDTH + x) * 4) as usize;
    &bytes[offset..offset + 3]
}

#[test]
#[ignore = "requires a GPU backend, but no window or display server"]
fn image_capture_and_virtual_input_without_a_window() {
    let mut probe = Probe::new();
    // The uninitialized camera must not be mistaken for a finished black image.
    assert!(probe.capture(true, Duration::from_millis(100)).is_none());
    assert_eq!(probe.snapshot().0.ticks, 0);
    probe.warp(); // Explicit initialization of camera geometry, layout and visibility.
    let image = probe
        .capture(true, Duration::from_secs(30))
        .expect("scene never became render-ready");
    assert_eq!(image.len(), (WIDTH * HEIGHT * 4) as usize);
    assert_eq!(pixel(&image, WIDTH / 2, HEIGHT / 2), [255, 0, 0]);
    assert_eq!(pixel(&image, 20, 20), [0, 255, 0]);
    assert_eq!(pixel(&image, WIDTH - 1, HEIGHT - 1), [0, 0, 0]);
    assert_eq!(probe.capture(true, Duration::from_secs(30)).unwrap(), image);

    // Queue motion without any cursor position. Even a capture must leave it queued.
    let before = probe.snapshot();
    probe.inputs.push(Input::Key(true));
    probe.inputs.push(Input::Look(Vec2::new(500., 0.)));
    assert_eq!(probe.capture(true, Duration::from_secs(30)).unwrap(), image);
    assert_eq!(probe.snapshot(), before);
    probe.warp();
    assert_eq!(probe.snapshot().0.movement, 2.);
    assert_eq!(probe.snapshot().0.angle, 0.5);
    probe.warp();
    assert_eq!(probe.snapshot().0.movement, 4.);
    assert_eq!(
        probe.snapshot().0.angle,
        0.5,
        "relative motion repeated without new input"
    );
    probe.inputs.push(Input::Key(false));
    probe.warp();
    assert_eq!(probe.snapshot().0.movement, 4.);
    let moved = probe.capture(true, Duration::from_secs(30)).unwrap();
    assert_ne!(moved, image);
    assert_eq!(
        pixel(&moved, 20, 20),
        [0, 255, 0],
        "UI moved with the camera"
    );
    let camera_entity = probe.app.world().resource::<Scene>().camera;
    probe
        .app
        .world_mut()
        .get_mut::<Camera>(camera_entity)
        .unwrap()
        .is_active = false;
    assert!(
        probe.capture(false, Duration::from_millis(100)).is_none(),
        "inactive camera accepted an old image"
    );
    probe
        .app
        .world_mut()
        .get_mut::<Camera>(camera_entity)
        .unwrap()
        .is_active = true;

    // Derive world picking coordinates from the real camera, not a synthetic window.
    let scene = probe.app.world().resource::<Scene>().clone();
    let world = probe.app.world();
    let camera = world.get::<Camera>(scene.camera).unwrap();
    assert_eq!(
        camera.physical_target_size(),
        Some(UVec2::new(WIDTH, HEIGHT))
    );
    assert_eq!(
        camera.logical_target_size(),
        Some(Vec2::new(WIDTH as f32 / SCALE, HEIGHT as f32 / SCALE))
    );
    let position = camera
        .world_to_viewport(
            world.get::<GlobalTransform>(scene.camera).unwrap(),
            Vec3::ZERO,
        )
        .unwrap();
    probe.pointer(position, PointerAction::Move { delta: Vec2::ZERO });
    probe.warp();
    probe.pointer(position, PointerAction::Press(PointerButton::Primary));
    assert_eq!(probe.snapshot().0.world_clicks, 0);
    probe.warp();
    assert_eq!(probe.snapshot().0.world_clicks, 1);
    probe.pointer(position, PointerAction::Release(PointerButton::Primary));
    probe.warp();

    let ui_position = probe
        .app
        .world()
        .get::<UiGlobalTransform>(scene.button)
        .unwrap()
        .translation
        / SCALE;
    probe.pointer(
        ui_position,
        PointerAction::Move {
            delta: ui_position - position,
        },
    );
    probe.warp();
    probe.pointer(ui_position, PointerAction::Press(PointerButton::Primary));
    let frozen = probe.snapshot();
    probe.capture(true, Duration::from_secs(30)).unwrap();
    assert_eq!(probe.snapshot(), frozen);
    probe.warp();
    assert_eq!(probe.snapshot().0.ui_clicks, 1);
    assert_eq!(probe.snapshot().0.world_clicks, 1);

    // A rendered black scene is valid; readiness never depends on nonzero pixels.
    *probe
        .app
        .world_mut()
        .get_mut::<Visibility>(scene.object)
        .unwrap() = Visibility::Hidden;
    *probe
        .app
        .world_mut()
        .get_mut::<Visibility>(scene.button)
        .unwrap() = Visibility::Hidden;
    probe.warp();
    let black = probe.capture(false, Duration::from_secs(30)).unwrap();
    assert!(black.chunks_exact(4).all(|rgba| rgba[..3] == [0, 0, 0]));
    let state = probe.snapshot().0;
    assert_eq!(state.pre_updates, state.ticks);
    assert_eq!(state.post_updates, state.ticks);
    assert!(state.fixed_updates > 0);
    assert_eq!(
        probe.snapshot().1,
        Duration::from_millis((state.ticks - 1) * 20)
    );
    assert_eq!(
        probe
            .app
            .world_mut()
            .query::<&Window>()
            .iter(probe.app.world())
            .count(),
        0
    );
    // Diagnostic artifacts only. Session PNG publication must still go through
    // screenshot::destination after the protocol/integration decisions are settled.
    let artifacts =
        std::env::temp_dir().join(format!("woodpecker-headless-probe-{}", std::process::id()));
    std::fs::create_dir(&artifacts).unwrap();
    for (name, bytes) in [
        ("scene.png", image),
        ("moved.png", moved),
        ("black.png", black),
    ] {
        Image::new(
            bevy::render::render_resource::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            bytes,
            TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::MAIN_WORLD,
        )
        .try_into_dynamic()
        .unwrap()
        .to_rgb8()
        .save(artifacts.join(name))
        .unwrap();
    }
    eprintln!(
        "headless probe passed: {}x{}, scale {}, {} explicit ticks; capture unchanged; artifacts {}",
        WIDTH,
        HEIGHT,
        SCALE,
        probe.snapshot().0.ticks,
        artifacts.display()
    );
}
