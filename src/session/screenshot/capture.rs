use super::{Diagnostic, Message, destination};
use crate::{command::screenshot::Output, session::protocol};
#[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
use bevy::camera::RenderTarget;
use bevy::{
    ecs::schedule::ScheduleCleanupPolicy,
    prelude::*,
    render::{
        RenderApp,
        renderer::RenderDevice,
        view::screenshot::{
            Captured, CapturedScreenshots, Screenshot, ScreenshotCaptured, trigger_screenshots,
        },
    },
    window::RawHandleWrapper,
};
use cap_std::fs::Dir;
use std::{
    collections::VecDeque,
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex, OnceLock, mpsc},
    time::{Duration, Instant},
};

#[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
mod image_target;
#[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
mod readiness;
#[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
mod selection;
mod surface;
#[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
mod verification;

const READBACK_TIMEOUT: Duration = Duration::from_secs(30);
const COMMAND: &str = "screenshot.capture";

#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Window(Entity),
    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
    Image(image_target::Selected),
}

struct Job {
    id: u64,
    path: String,
    target: Target,
}

enum Verification {
    Window(Arc<OnceLock<bool>>),
    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
    Image(Arc<verification::Verification>),
}

struct Active {
    job: Job,
    entity: Option<Entity>,
    since: Instant,
    writing: bool,
    verification: Verification,
}

impl Active {
    fn window_surface(&self) -> Option<(Entity, Arc<OnceLock<bool>>)> {
        match (&self.job.target, &self.verification) {
            (Target::Window(window), Verification::Window(surface)) => {
                Some((*window, surface.clone()))
            }
            #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
            _ => None,
        }
    }

    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
    fn image_request(&self) -> Option<image_target::Request> {
        match (&self.job.target, &self.verification) {
            (Target::Image(selected), Verification::Image(verification)) => {
                Some(image_target::Request {
                    selected: selected.clone(),
                    entity: self.entity,
                    verification: verification.clone(),
                })
            }
            _ => None,
        }
    }
}

#[derive(Resource)]
pub(super) struct Service {
    root: Arc<Dir>,
    queue: VecDeque<Job>,
    active: Option<Active>,
    sender: mpsc::Sender<Result<Output, Diagnostic>>,
    receiver: Mutex<mpsc::Receiver<Result<Output, Diagnostic>>>,
}

impl Service {
    pub(super) fn pending(&self, id: u64) -> bool {
        self.active.as_ref().is_some_and(|a| a.job.id == id)
            || self.queue.iter().any(|job| job.id == id)
    }
}

pub(super) fn install(app: &mut App, root: &Path) {
    if app.get_sub_app(RenderApp).is_none()
        || !app.world().contains_resource::<RenderDevice>()
        || !app.world().contains_resource::<CapturedScreenshots>()
    {
        return;
    }
    let root = match Dir::open_ambient_dir(root, cap_std::ambient_authority()) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("woodpecker screenshot unavailable: {error}");
            return;
        }
    };
    // Readback delivery must progress without running Update or any simulation system.
    app.world_mut().schedule_scope(Update, |world, schedule| {
        schedule
            .remove_systems_in_set(
                trigger_screenshots,
                world,
                ScheduleCleanupPolicy::RemoveSystemsOnly,
            )
            .expect("remove tick-bound screenshot delivery");
    });
    let (sender, receiver) = mpsc::channel();
    app.insert_resource(Service {
        root: Arc::new(root),
        queue: VecDeque::new(),
        active: None,
        sender,
        receiver: Mutex::new(receiver),
    });
    surface::install(app);
    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
    image_target::install(app);
}

fn window(world: &World) -> Result<Entity, Diagnostic> {
    super::super::window::primary(world)
        .filter(|(id, window)| {
            world.get::<RawHandleWrapper>(*id).is_some()
                && window.physical_width() > 0
                && window.physical_height() > 0
        })
        .map(|(id, _)| id)
        .ok_or_else(|| {
            Diagnostic::new(
                "screenshot_window_unavailable",
                "expected one primary rendered window",
            )
        })
}

fn target(world: &World) -> Result<Target, Diagnostic> {
    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
    {
        let window = window(world);
        match selection::choose(
            window.as_ref().ok().copied(),
            image_target::configured(world),
        ) {
            selection::Choice::Window(window) => Ok(Target::Window(window)),
            selection::Choice::Image => image_target::target(world).map(Target::Image),
            selection::Choice::Unavailable => window.map(Target::Window),
        }
    }
    #[cfg(not(any(feature = "headless-2d", feature = "headless-3d")))]
    {
        window(world).map(Target::Window)
    }
}

pub(super) fn start(world: &mut World, id: u64, path: String) -> Result<(), Diagnostic> {
    let service = world.get_resource::<Service>().ok_or_else(|| {
        Diagnostic::new(
            "screenshot_unavailable",
            "renderer and screenshot service are not installed",
        )
    })?;
    let target = target(world)?;
    destination::prepare(&service.root, &path)?;
    world
        .resource_mut::<Service>()
        .queue
        .push_back(Job { id, path, target });
    Ok(())
}

fn response(id: u64, result: Result<Output, Diagnostic>) -> Message {
    match result {
        Ok(output) => protocol::completed(id, COMMAND, output),
        Err(error) => Message::Rejected {
            request_id: id,
            command: COMMAND.into(),
            error,
        },
    }
}

fn encode(root: &Dir, path: String, image: Image) -> Result<Output, Diagnostic> {
    let image = image
        .try_into_dynamic()
        .map_err(destination::failed)?
        .to_rgb8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err(destination::failed("empty screenshot image"));
    }
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(destination::failed)?;
    let overwritten = destination::write(root, &path, bytes.get_ref())?;
    Ok(Output {
        path,
        width,
        height,
        overwritten,
    })
}

pub(super) fn poll(world: &mut World) -> Vec<Message> {
    if !world.contains_resource::<Service>() {
        return Vec::new();
    }
    let images: Vec<_> = world
        .resource::<CapturedScreenshots>()
        .0
        .lock()
        .unwrap()
        .try_iter()
        .collect();
    world.resource_scope(|world, mut service: Mut<Service>| {
        let mut responses = Vec::new();
        if service.active.as_ref().is_some_and(|active| {
            !active.writing
                && matches!(
                    &active.verification,
                    Verification::Window(surface) if surface.get() == Some(&false)
                )
        }) {
            let active = service.active.take().unwrap();
            if let Some(entity) = active.entity {
                world.despawn(entity);
            }
            responses.push(response(
                active.job.id,
                Err(Diagnostic::new(
                    "screenshot_window_unavailable",
                    "primary window has no render surface for this capture frame",
                )),
            ));
        }
        #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
        if service.active.as_ref().is_some_and(|active| {
            let (Some(entity), Verification::Image(verification)) =
                (active.entity, &active.verification)
            else {
                return false;
            };
            verification.capture_result(entity) == Some(false)
        }) {
            let active = service.active.take().unwrap();
            if let Some(entity) = active.entity {
                world.despawn(entity);
            }
            responses.push(response(
                active.job.id,
                Err(Diagnostic::new(
                    "screenshot_failed",
                    "capture frame output was not verified",
                )),
            ));
        }
        for (entity, image) in images {
            if let Some(active) = service
                .active
                .as_mut()
                .filter(|active| active.entity == Some(entity) && !active.writing)
            {
                let verified = match &active.verification {
                    Verification::Window(surface) => surface.get() == Some(&true),
                    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
                    Verification::Image(verification) => {
                        verification.capture_result(entity) == Some(true)
                    }
                };
                if !verified {
                    responses.push(response(
                        active.job.id,
                        Err(Diagnostic::new(
                            "screenshot_failed",
                            "capture frame output was not verified",
                        )),
                    ));
                    world.despawn(entity);
                    service.active = None;
                    continue;
                }
                active.writing = true;
                let path = active.job.path.clone();
                let root = service.root.clone();
                let sender = service.sender.clone();
                world.despawn(entity);
                // Encoding and disk IO must not block Control, command intake, or rendering.
                std::thread::spawn(move || {
                    let _ = sender.send(encode(&root, path, image));
                });
            } else if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.insert(Captured);
                world.trigger(ScreenshotCaptured { entity, image });
            }
        }
        let completion = service.receiver.lock().unwrap().try_recv().ok();
        if let Some(result) = completion {
            let active = service.active.take().expect("one active writer");
            responses.push(response(active.job.id, result));
        }
        #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
        if let Some(active) = service.active.as_mut()
            && !active.writing
            && active.entity.is_none()
            && matches!(
                &active.verification,
                Verification::Image(verification) if verification.warmup_ready()
            )
        {
            let Target::Image(selected) = &active.job.target else {
                unreachable!("image verification belongs to an image target")
            };
            let entity = world
                .spawn(Screenshot(RenderTarget::Image(selected.target.clone())))
                .id();
            let Verification::Image(verification) = &active.verification else {
                unreachable!("image target has image verification")
            };
            verification.begin_capture(entity);
            active.entity = Some(entity);
        }
        if service
            .active
            .as_ref()
            .is_some_and(|active| !active.writing && active.since.elapsed() >= READBACK_TIMEOUT)
        {
            let active = service.active.take().unwrap();
            if let Some(entity) = active.entity {
                world.despawn(entity);
            }
            responses.push(response(
                active.job.id,
                Err(destination::failed("GPU readback timed out")),
            ));
        }
        if service.active.is_none() {
            while let Some(job) = service.queue.pop_front() {
                if target(world).ok().as_ref() != Some(&job.target) {
                    let (code, message) = match job.target {
                        Target::Window(_) => (
                            "screenshot_window_unavailable",
                            "primary window changed before capture",
                        ),
                        #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
                        Target::Image(_) => (
                            "screenshot_target_unavailable",
                            "image target changed before capture",
                        ),
                    };
                    responses.push(response(job.id, Err(Diagnostic::new(code, message))));
                    continue;
                }
                // Bevy drops concurrent screenshots for the same render target. Serialize them.
                let (entity, verification) = match &job.target {
                    Target::Window(window) => (
                        Some(world.spawn(Screenshot::window(*window)).id()),
                        Verification::Window(Arc::default()),
                    ),
                    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
                    Target::Image(_) => (
                        None,
                        Verification::Image(Arc::new(verification::Verification::default())),
                    ),
                };
                service.active = Some(Active {
                    job,
                    entity,
                    since: Instant::now(),
                    writing: false,
                    verification,
                });
                break;
            }
        }
        responses
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };
    use destination::tests::Sandbox;

    fn image() -> Image {
        Image::new_fill(
            Extent3d {
                width: 2,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[12, 34, 56, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD,
        )
    }

    fn fixture(root: Dir) -> (World, Entity, mpsc::Sender<(Entity, Image)>) {
        let mut world = World::new();
        world.insert_resource(Time::<Virtual>::default());
        let entity = world.spawn_empty().id();
        let window = world.spawn_empty().id();
        let (sender, receiver) = mpsc::channel();
        let (images, captured) = mpsc::channel();
        world.insert_resource(CapturedScreenshots(Arc::new(Mutex::new(captured))));
        world.insert_resource(Service {
            root: Arc::new(root),
            queue: VecDeque::new(),
            active: Some(Active {
                job: Job {
                    id: 17,
                    path: "images/a.png".into(),
                    target: Target::Window(window),
                },
                entity: Some(entity),
                since: Instant::now(),
                writing: false,
                verification: Verification::Window(Arc::default()),
            }),
            sender,
            receiver: Mutex::new(receiver),
        });
        (world, entity, images)
    }

    fn window(active: &Active) -> Entity {
        match active.job.target {
            Target::Window(window) => window,
            #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
            Target::Image(_) => panic!("window fixture changed target"),
        }
    }

    fn surface(active: &Active) -> &OnceLock<bool> {
        match &active.verification {
            Verification::Window(surface) => surface,
            #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
            Verification::Image(_) => panic!("window fixture changed verification"),
        }
    }

    #[test]
    fn prepared_readback_without_render_surface_is_rejected_not_encoded() {
        let sandbox = Sandbox::new();
        let root = sandbox.root("root");
        let reader = root.try_clone().unwrap();
        let (mut world, entity, images) = fixture(root);
        // Reproduce the render-world condition that makes Bevy skip its Copy:
        // a prepared request but no acquired window view. The adapter still
        // receives an Image, as it does from Bevy's zero-initialized buffer.
        let active = world.resource::<Service>().active.as_ref().unwrap();
        let mut render = World::new();
        render.insert_resource(super::surface::Frame::new(
            window(active),
            match &active.verification {
                Verification::Window(surface) => surface.clone(),
                #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
                Verification::Image(_) => unreachable!(),
            },
        ));
        render.insert_resource(bevy::render::view::window::ExtractedWindows::default());
        render.run_system_cached(super::surface::check).unwrap();
        let mut black = image();
        black.data.as_mut().unwrap().fill(0);
        images.send((entity, black)).unwrap();
        let responses = poll(&mut world);
        assert!(
            matches!(&responses[..], [Message::Rejected { request_id: 17, error, .. }]
            if error.code == "screenshot_window_unavailable"),
            "{responses:?}"
        );
        assert!(!reader.exists("images/a.png"));
        assert!(!world.resource::<Service>().pending(17));
        assert!(world.get_entity(entity).is_err());
        images.send((entity, image())).unwrap();
        assert!(
            poll(&mut world).is_empty(),
            "late readback revived a rejected capture"
        );
    }

    #[test]
    fn verified_black_image_is_valid_even_if_a_later_frame_loses_its_surface() {
        let sandbox = Sandbox::new();
        let root = sandbox.root("root");
        let reader = root.try_clone().unwrap();
        let (mut world, entity, images) = fixture(root);
        let active = world.resource::<Service>().active.as_ref().unwrap();
        surface(active).set(true).unwrap();
        let mut render = World::new();
        render.insert_resource(super::surface::Frame::new(
            window(active),
            match &active.verification {
                Verification::Window(surface) => surface.clone(),
                #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
                Verification::Image(_) => unreachable!(),
            },
        ));
        render.insert_resource(bevy::render::view::window::ExtractedWindows::default());
        render.run_system_cached(super::surface::check).unwrap();
        assert_eq!(surface(active).get(), Some(&true));
        let mut black = image();
        black.data.as_mut().unwrap().fill(0);
        images.send((entity, black)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let response = loop {
            if let Some(response) = poll(&mut world).into_iter().next() {
                break response;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert!(matches!(
            response,
            Message::Completed { request_id: 17, .. }
        ));
        let png = image::load_from_memory(&reader.read("images/a.png").unwrap()).unwrap();
        assert_eq!(png.to_rgb8().into_raw(), vec![0; 6]);
    }

    #[test]
    fn unavailable_surface_preserves_existing_file_without_waiting_for_readback() {
        let sandbox = Sandbox::new();
        let root = sandbox.root("root");
        root.create_dir("images").unwrap();
        root.write("images/a.png", b"existing file").unwrap();
        let reader = root.try_clone().unwrap();
        let (mut world, entity, images) = fixture(root);
        let active = world.resource::<Service>().active.as_ref().unwrap();
        surface(active).set(false).unwrap();
        // A later available frame cannot approve an earlier uncopied buffer.
        assert!(surface(active).set(true).is_err());
        assert!(
            matches!(&poll(&mut world)[..], [Message::Rejected { error, .. }]
            if error.code == "screenshot_window_unavailable")
        );
        images.send((entity, image())).unwrap();
        assert!(poll(&mut world).is_empty());
        assert_eq!(reader.read("images/a.png").unwrap(), b"existing file");
    }

    #[test]
    fn unverified_readback_fails_closed() {
        let sandbox = Sandbox::new();
        let root = sandbox.root("root");
        let reader = root.try_clone().unwrap();
        let (mut world, entity, images) = fixture(root);
        images.send((entity, image())).unwrap();
        assert!(
            matches!(&poll(&mut world)[..], [Message::Rejected { error, .. }]
            if error.code == "screenshot_failed")
        );
        assert!(!reader.exists("images/a.png"));
    }

    #[test]
    fn response_waits_for_readback_and_decodable_png_without_advancing_time() {
        let sandbox = Sandbox::new();
        let root = sandbox.root("root");
        let reader = root.try_clone().unwrap();
        let (mut world, entity, images) = fixture(root);
        assert!(poll(&mut world).is_empty());
        assert!(!reader.exists("images/a.png"));
        assert!(world.resource::<Service>().pending(17));
        let elapsed = world.resource::<Time<Virtual>>().elapsed();
        surface(world.resource::<Service>().active.as_ref().unwrap())
            .set(true)
            .unwrap();
        images.send((entity, image())).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let response = loop {
            let responses = poll(&mut world);
            if let Some(response) = responses.into_iter().next() {
                break response;
            }
            assert!(Instant::now() < deadline, "writer did not finish");
            std::thread::yield_now();
        };
        assert!(
            matches!(response, Message::Completed { request_id: 17, ref output, .. }
            if output == &serde_json::json!({"path":"images/a.png","width":2,"height":1,"overwritten":false}))
        );
        let png = image::load_from_memory(&reader.read("images/a.png").unwrap()).unwrap();
        assert_eq!(png.to_rgb8().into_raw(), vec![12, 34, 56, 12, 34, 56]);
        assert_eq!(world.resource::<Time<Virtual>>().elapsed(), elapsed);
        assert!(world.get_entity(entity).is_err());
        assert!(!world.resource::<Service>().pending(17));
        assert!(poll(&mut world).is_empty());
    }

    #[test]
    fn readback_timeout_cleans_up_and_ignores_late_images() {
        let sandbox = Sandbox::new();
        let (mut world, entity, images) = fixture(sandbox.root("root"));
        world
            .resource_mut::<Service>()
            .active
            .as_mut()
            .unwrap()
            .since = Instant::now() - READBACK_TIMEOUT;
        assert!(
            matches!(&poll(&mut world)[0], Message::Rejected { request_id: 17, error, .. }
            if error.code == "screenshot_failed")
        );
        assert!(world.get_entity(entity).is_err());
        images.send((entity, image())).unwrap();
        assert!(poll(&mut world).is_empty());
    }

    #[test]
    fn png_and_destination_errors_do_not_claim_success() {
        let sandbox = Sandbox::new();
        let root = sandbox.root("root");
        let mut unsupported = image();
        unsupported.texture_descriptor.format = TextureFormat::Rg32Uint;
        assert_eq!(
            encode(&root, "unsupported.png".into(), unsupported)
                .unwrap_err()
                .code,
            "screenshot_failed"
        );
        assert!(!root.exists("unsupported.png"));
        root.create_dir("directory.png").unwrap();
        assert_eq!(
            encode(&root, "directory.png".into(), image())
                .unwrap_err()
                .code,
            "screenshot_failed"
        );
    }

    #[cfg(feature = "headless-2d")]
    #[test]
    fn rendered_window_remains_primary_when_an_image_target_also_exists() {
        use bevy::camera::{ImageRenderTarget, RenderTarget};

        let mut world = World::new();
        world.spawn((
            Camera2d,
            crate::session::HeadlessCaptureCamera2d,
            RenderTarget::Image(ImageRenderTarget {
                handle: Handle::default(),
                scale_factor: 1.0,
            }),
        ));
        assert!(image_target::configured(&world));
        // `Some(window)` is the successful result of the unchanged rendered
        // primary-window seam; image configuration must not be evaluated first.
        let window = Entity::from_bits(41);
        assert_eq!(
            selection::choose(Some(window), image_target::configured(&world)),
            selection::Choice::Window(window)
        );
    }

    #[cfg(feature = "headless-2d")]
    #[test]
    fn unmarked_image_camera_is_not_accepted_as_the_fixed_2d_target() {
        use bevy::camera::{ImageRenderTarget, RenderTarget, RenderTargetInfo};

        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::new_target_texture(
            321,
            181,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        world.insert_resource(images);
        world.spawn((
            Camera {
                computed: bevy::camera::ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(321, 181),
                        scale_factor: 1.5,
                    }),
                    ..default()
                },
                ..default()
            },
            RenderTarget::Image(ImageRenderTarget {
                handle: image,
                scale_factor: 1.5,
            }),
        ));
        assert_eq!(
            image_target::target(&world).unwrap_err().code,
            "screenshot_target_unavailable"
        );
    }

    #[cfg(feature = "headless-2d")]
    #[test]
    fn image_target_requires_one_initialized_full_image_writing_camera() {
        use bevy::camera::{ImageRenderTarget, RenderTarget, RenderTargetInfo};

        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::new_target_texture(
            321,
            181,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        world.insert_resource(images);
        let camera = world
            .spawn((
                Camera::default(),
                Camera2d,
                crate::session::HeadlessCaptureCamera2d,
                RenderTarget::Image(ImageRenderTarget {
                    handle: image.clone(),
                    scale_factor: 1.5,
                }),
            ))
            .id();
        assert_eq!(
            target(&world).unwrap_err().code,
            "screenshot_target_unavailable"
        );
        world
            .get_mut::<Camera>(camera)
            .unwrap()
            .computed
            .target_info = Some(RenderTargetInfo {
            physical_size: UVec2::new(321, 181),
            scale_factor: 1.5,
        });
        let Target::Image(selected) = target(&world).unwrap() else {
            panic!("initialized image camera must select its image target")
        };
        assert_eq!(selected.target.handle, image.clone());
        assert_eq!(selected.target.scale_factor, 1.5);
        assert_eq!(selected.pipeline, selection::ImagePipeline::TwoD);
        world.spawn((
            Camera {
                computed: bevy::camera::ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(321, 181),
                        scale_factor: 1.5,
                    }),
                    ..default()
                },
                ..default()
            },
            RenderTarget::Image(ImageRenderTarget {
                handle: image,
                scale_factor: 1.5,
            }),
        ));
        assert_eq!(
            target(&world).unwrap_err().code,
            "screenshot_target_unavailable"
        );
    }

    #[cfg(feature = "headless-3d")]
    fn fixed_3d_world() -> (World, Entity) {
        use bevy::camera::{
            ImageRenderTarget, PerspectiveProjection, Projection, RenderTargetInfo,
        };

        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::new_target_texture(
            321,
            181,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        world.insert_resource(images);
        let camera = world
            .spawn((
                Camera {
                    computed: bevy::camera::ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(321, 181),
                            scale_factor: 1.5,
                        }),
                        ..default()
                    },
                    ..default()
                },
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection {
                    aspect_ratio: 321.0 / 181.0,
                    near: 0.1,
                    far: 100.0,
                    ..default()
                }),
                crate::session::HeadlessCaptureCamera3d,
                RenderTarget::Image(ImageRenderTarget {
                    handle: image,
                    scale_factor: 1.5,
                }),
            ))
            .id();
        (world, camera)
    }

    #[cfg(feature = "headless-3d")]
    #[test]
    fn fixed_3d_target_is_explicit_perspective_and_preserves_full_identity() {
        use bevy::camera::{OrthographicProjection, PerspectiveProjection, Projection};

        let (mut world, camera) = fixed_3d_world();
        let selected = image_target::target(&world).unwrap();
        assert_eq!(selected.pipeline, selection::ImagePipeline::ThreeD);
        assert_eq!(selected.target.scale_factor, 1.5);

        world
            .entity_mut(camera)
            .remove::<crate::session::HeadlessCaptureCamera3d>();
        assert_eq!(
            image_target::target(&world).unwrap_err().code,
            "screenshot_target_unavailable"
        );
        world
            .entity_mut(camera)
            .insert(crate::session::HeadlessCaptureCamera3d);
        world.entity_mut(camera).insert(Projection::Orthographic(
            OrthographicProjection::default_3d(),
        ));
        assert_eq!(
            image_target::target(&world).unwrap_err().code,
            "screenshot_target_unavailable"
        );
        world.entity_mut(camera).insert((
            Projection::Perspective(PerspectiveProjection {
                aspect_ratio: 321.0 / 181.0,
                ..default()
            }),
            Camera2d,
        ));
        assert_eq!(
            image_target::target(&world).unwrap_err().code,
            "screenshot_target_unavailable"
        );
    }

    #[cfg(feature = "headless-3d")]
    #[test]
    fn fixed_3d_rejects_ambiguous_image_cameras_and_window_still_wins() {
        use bevy::camera::{ImageRenderTarget, RenderTarget};

        let (mut world, _) = fixed_3d_world();
        assert!(image_target::configured(&world));
        let window = Entity::from_bits(72);
        assert_eq!(
            selection::choose(Some(window), image_target::configured(&world)),
            selection::Choice::Window(window)
        );
        world.spawn((
            Camera::default(),
            RenderTarget::Image(ImageRenderTarget {
                handle: Handle::default(),
                scale_factor: 1.5,
            }),
        ));
        assert_eq!(
            image_target::target(&world).unwrap_err().code,
            "screenshot_target_unavailable"
        );
    }

    #[cfg(feature = "headless-3d")]
    #[test]
    fn queued_3d_capture_rejects_projection_change_before_activation() {
        use bevy::camera::Projection;

        let sandbox = Sandbox::new();
        let (mut world, camera) = fixed_3d_world();
        let (sender, receiver) = mpsc::channel();
        let (_images, captured) = mpsc::channel();
        world.insert_resource(CapturedScreenshots(Arc::new(Mutex::new(captured))));
        world.insert_resource(Service {
            root: Arc::new(sandbox.root("root")),
            queue: VecDeque::new(),
            active: None,
            sender,
            receiver: Mutex::new(receiver),
        });
        start(&mut world, 31, "fixed-3d.png".into()).unwrap();
        let mut projection = world.get_mut::<Projection>(camera).unwrap();
        let Projection::Perspective(projection) = projection.as_mut() else {
            panic!("fixture uses perspective")
        };
        projection.fov *= 0.75;

        let responses = poll(&mut world);
        assert!(
            matches!(&responses[..], [Message::Rejected { request_id: 31, error, .. }]
                if error.code == "screenshot_target_unavailable"),
            "{responses:?}"
        );
    }

    #[cfg(feature = "headless-2d")]
    fn image_capture_fixture(root: Dir) -> (World, Entity, image_target::Selected) {
        use bevy::camera::{ImageRenderTarget, RenderTarget, RenderTargetInfo};

        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::new_target_texture(
            321,
            181,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        world.insert_resource(images);
        let target = ImageRenderTarget {
            handle: image,
            scale_factor: 1.5,
        };
        let camera = world
            .spawn((
                Camera {
                    computed: bevy::camera::ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(321, 181),
                            scale_factor: 1.5,
                        }),
                        ..default()
                    },
                    ..default()
                },
                Camera2d,
                crate::session::HeadlessCaptureCamera2d,
                RenderTarget::Image(target.clone()),
            ))
            .id();
        let (sender, receiver) = mpsc::channel();
        let (_images, captured) = mpsc::channel();
        world.insert_resource(CapturedScreenshots(Arc::new(Mutex::new(captured))));
        world.insert_resource(Service {
            root: Arc::new(root),
            queue: VecDeque::new(),
            active: None,
            sender,
            receiver: Mutex::new(receiver),
        });
        let selected = image_target::target(&world).unwrap();
        (world, camera, selected)
    }

    #[cfg(feature = "headless-2d")]
    #[test]
    fn image_capture_spawns_screenshot_with_the_selected_camera_target() {
        use bevy::camera::RenderTarget;

        let sandbox = Sandbox::new();
        let (mut world, _, camera_target) = image_capture_fixture(sandbox.root("root"));
        start(&mut world, 23, "fixed.png".into()).unwrap();
        assert!(poll(&mut world).is_empty());
        let active = world.resource::<Service>().active.as_ref().unwrap();
        assert_eq!(active.job.target, Target::Image(camera_target.clone()));
        let Verification::Image(verification) = &active.verification else {
            panic!("image target must use image verification")
        };
        verification.record_frame(None, true);
        assert!(poll(&mut world).is_empty());
        let entity = world
            .resource::<Service>()
            .active
            .as_ref()
            .unwrap()
            .entity
            .unwrap();
        let screenshot = world.get::<Screenshot>(entity).unwrap();
        assert_eq!(
            screenshot.0.normalize(None),
            RenderTarget::Image(camera_target.target).normalize(None),
            "queued capture must preserve the selected camera target identity"
        );
    }

    #[cfg(feature = "headless-2d")]
    #[test]
    fn queued_image_capture_rejects_a_scale_change_before_activation() {
        use bevy::camera::{ImageRenderTarget, RenderTarget};

        let sandbox = Sandbox::new();
        let (mut world, camera, camera_target) = image_capture_fixture(sandbox.root("root"));
        start(&mut world, 24, "fixed.png".into()).unwrap();
        *world.get_mut::<RenderTarget>(camera).unwrap() = RenderTarget::Image(ImageRenderTarget {
            handle: camera_target.target.handle,
            scale_factor: 1.0,
        });

        let responses = poll(&mut world);
        assert!(
            matches!(&responses[..], [Message::Rejected { request_id: 24, error, .. }]
                if error.code == "screenshot_target_unavailable"),
            "{responses:?}"
        );
        assert!(world.resource::<Service>().active.is_none());
    }

    #[test]
    fn missing_ambiguous_or_unrendered_windows_are_rejected() {
        let sandbox = Sandbox::new();
        let (mut world, _, _) = fixture(sandbox.root("root"));
        assert_eq!(
            start(&mut world, 18, "a.png".into()).unwrap_err().code,
            "screenshot_window_unavailable"
        );
        world.spawn((Window::default(), bevy::window::PrimaryWindow));
        assert_eq!(
            start(&mut world, 18, "a.png".into()).unwrap_err().code,
            "screenshot_window_unavailable"
        );
        world.spawn((Window::default(), bevy::window::PrimaryWindow));
        assert_eq!(
            start(&mut world, 18, "a.png".into()).unwrap_err().code,
            "screenshot_window_unavailable"
        );
        assert!(!world.resource::<Service>().pending(18));
    }
}
