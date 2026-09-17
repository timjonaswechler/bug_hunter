use super::{Diagnostic, Message, destination};
use crate::{command::screenshot::Output, session::protocol};
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
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

const READBACK_TIMEOUT: Duration = Duration::from_secs(30);
const COMMAND: &str = "screenshot.capture";

struct Job {
    id: u64,
    path: String,
    window: Entity,
}

struct Active {
    job: Job,
    entity: Entity,
    since: Instant,
    writing: bool,
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

pub(super) fn start(world: &mut World, id: u64, path: String) -> Result<(), Diagnostic> {
    let service = world.get_resource::<Service>().ok_or_else(|| {
        Diagnostic::new(
            "screenshot_unavailable",
            "renderer and screenshot service are not installed",
        )
    })?;
    let window = window(world)?;
    destination::prepare(&service.root, &path)?;
    world
        .resource_mut::<Service>()
        .queue
        .push_back(Job { id, path, window });
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
        for (entity, image) in images {
            if let Some(active) = service
                .active
                .as_mut()
                .filter(|a| a.entity == entity && !a.writing)
            {
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
        if service
            .active
            .as_ref()
            .is_some_and(|a| !a.writing && a.since.elapsed() >= READBACK_TIMEOUT)
        {
            let active = service.active.take().unwrap();
            world.despawn(active.entity);
            responses.push(response(
                active.job.id,
                Err(destination::failed("GPU readback timed out")),
            ));
        }
        if service.active.is_none() {
            while let Some(job) = service.queue.pop_front() {
                if window(world).ok() != Some(job.window) {
                    responses.push(response(
                        job.id,
                        Err(Diagnostic::new(
                            "screenshot_window_unavailable",
                            "primary window changed before capture",
                        )),
                    ));
                    continue;
                }
                // Bevy drops concurrent screenshots for the same render target. Serialize them.
                let entity = world.spawn(Screenshot::window(job.window)).id();
                service.active = Some(Active {
                    job,
                    entity,
                    since: Instant::now(),
                    writing: false,
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
                    window,
                },
                entity,
                since: Instant::now(),
                writing: false,
            }),
            sender,
            receiver: Mutex::new(receiver),
        });
        (world, entity, images)
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
