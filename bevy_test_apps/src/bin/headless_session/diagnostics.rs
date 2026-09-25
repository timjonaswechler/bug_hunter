//! Opt-in, one-shot evidence for the fixed headless session capture.
use bevy::{
    asset::{AssetEvent, AssetId},
    camera::{NormalizedRenderTarget, RenderTarget},
    core_pipeline::core_2d::Transparent2d,
    ecs::system::SystemParam,
    image::TRANSPARENT_IMAGE_HANDLE,
    prelude::*,
    render::{
        Extract, ExtractSchedule, RenderApp,
        camera::ExtractedCamera,
        render_asset::RenderAssets,
        render_phase::ViewSortedRenderPhases,
        render_resource::{CachedPipelineState, PipelineCache},
        renderer::{RenderGraph, RenderGraphSystems},
        texture::GpuImage,
        view::{
            ExtractedView, ViewTarget, screenshot::Screenshot, visibility::RenderVisibleEntities,
        },
    },
    shader::Shader,
    sprite_render::{ExtractedSprites, SpriteBatches},
    ui_render::{ExtractedUiNodes, TransparentUi, UiBatch},
};

const ENABLED: &str = "WOODPECKER_HEADLESS_CAPTURE_DIAGNOSTICS";

pub(super) struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os(ENABLED).as_deref() != Some(std::ffi::OsStr::new("1")) {
            return;
        }
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            warn!(
                target: "headless_session_diagnostic",
                "[headless-session-diagnostic] RenderApp unavailable"
            );
            return;
        };
        render_app
            .init_resource::<DiagnosticRequest>()
            .add_systems(ExtractSchedule, extract_request)
            .add_systems(
                RenderGraph,
                capture_frame_snapshot.in_set(RenderGraphSystems::Finish),
            );
    }
}

#[derive(Clone, Debug)]
struct MainSnapshot {
    entity: Entity,
    image: AssetId<Image>,
    image_assets: usize,
    shader_assets: usize,
    image_events: usize,
    shader_events: usize,
    target_image_in_main: bool,
    default_image_in_main: bool,
    transparent_image_in_main: bool,
}

#[derive(Resource, Default)]
struct DiagnosticRequest {
    pending: Option<MainSnapshot>,
    logged: Option<Entity>,
}

impl DiagnosticRequest {
    fn record(&mut self, snapshot: MainSnapshot) -> bool {
        if self.logged == Some(snapshot.entity)
            || self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.entity == snapshot.entity)
        {
            return false;
        }
        self.pending = Some(snapshot);
        true
    }

    fn take(&mut self) -> Option<MainSnapshot> {
        let snapshot = self.pending.take()?;
        self.logged = Some(snapshot.entity);
        Some(snapshot)
    }
}

fn extract_request(
    screenshots: Extract<Query<(Entity, &Screenshot)>>,
    images: Extract<Res<Assets<Image>>>,
    shaders: Extract<Res<Assets<Shader>>>,
    image_events: Extract<Res<Messages<AssetEvent<Image>>>>,
    shader_events: Extract<Res<Messages<AssetEvent<Shader>>>>,
    mut diagnostic: ResMut<DiagnosticRequest>,
) {
    let mut requests = screenshots.iter();
    let Some((entity, screenshot)) = requests.next() else {
        return;
    };
    let RenderTarget::Image(target) = &screenshot.0 else {
        return;
    };
    let snapshot = MainSnapshot {
        entity,
        image: target.handle.id(),
        image_assets: images.len(),
        shader_assets: shaders.len(),
        image_events: image_events.len(),
        shader_events: shader_events.len(),
        target_image_in_main: images.contains(&target.handle),
        default_image_in_main: images.contains(&Handle::<Image>::default()),
        transparent_image_in_main: images.contains(&TRANSPARENT_IMAGE_HANDLE),
    };
    if !diagnostic.record(snapshot.clone()) {
        return;
    }
    info!(
        target: "headless_session_diagnostic",
        request = ?snapshot.entity,
        image = ?snapshot.image,
        screenshot_requests = 1 + requests.count(),
        image_assets = snapshot.image_assets,
        shader_assets = snapshot.shader_assets,
        image_events = snapshot.image_events,
        shader_events = snapshot.shader_events,
        target_image_in_main = snapshot.target_image_in_main,
        default_image_in_main = snapshot.default_image_in_main,
        transparent_image_in_main = snapshot.transparent_image_in_main,
        "[headless-session-diagnostic] capture_request"
    );
}

#[derive(Default)]
struct PipelineCounts {
    ready: usize,
    waiting: usize,
    failed: usize,
}

impl PipelineCounts {
    fn add(&mut self, state: &CachedPipelineState) {
        match state {
            CachedPipelineState::Ok(_) => self.ready += 1,
            CachedPipelineState::Queued | CachedPipelineState::Creating(_) => self.waiting += 1,
            CachedPipelineState::Err(_) => self.failed += 1,
        }
    }
}

#[derive(SystemParam)]
struct Evidence<'w, 's> {
    views: Query<
        'w,
        's,
        (
            Entity,
            &'static ExtractedCamera,
            &'static ExtractedView,
            &'static ViewTarget,
            &'static RenderVisibleEntities,
        ),
    >,
    sprites: Res<'w, ExtractedSprites>,
    sprite_phases: Res<'w, ViewSortedRenderPhases<Transparent2d>>,
    sprite_batches: Res<'w, SpriteBatches>,
    ui_nodes: Res<'w, ExtractedUiNodes>,
    ui_phases: Res<'w, ViewSortedRenderPhases<TransparentUi>>,
    ui_batches: Query<'w, 's, &'static UiBatch>,
    gpu_images: Res<'w, RenderAssets<GpuImage>>,
    pipelines: Res<'w, PipelineCache>,
}

fn capture_frame_snapshot(mut diagnostic: ResMut<DiagnosticRequest>, evidence: Evidence) {
    let Some(snapshot) = diagnostic.take() else {
        return;
    };
    let matching_views: Vec<_> = evidence
        .views
        .iter()
        .filter(|(_, camera, _, _, _)| {
            matches!(
                &camera.target,
                Some(NormalizedRenderTarget::Image(target)) if target.handle.id() == snapshot.image
            )
        })
        .collect();

    let mut global_pipelines = PipelineCounts::default();
    for pipeline in evidence.pipelines.pipelines() {
        global_pipelines.add(&pipeline.state);
    }

    let Some((view_entity, camera, view, target, visible)) = matching_views.first().copied() else {
        info!(
            target: "headless_session_diagnostic",
            request = ?snapshot.entity,
            image = ?snapshot.image,
            matching_views = matching_views.len(),
            extracted_sprites = evidence.sprites.sprites.len(),
            extracted_ui_nodes = evidence.ui_nodes.uinodes.len(),
            pipelines_ready = global_pipelines.ready,
            pipelines_waiting = global_pipelines.waiting,
            pipelines_failed = global_pipelines.failed,
            "[headless-session-diagnostic] capture_frame_no_target_view"
        );
        return;
    };

    let sprite_items = evidence
        .sprite_phases
        .get(&view.retained_view_entity)
        .map(|phase| phase.items.values().collect::<Vec<_>>())
        .unwrap_or_default();
    let ui_items = evidence
        .ui_phases
        .get(&view.retained_view_entity)
        .map(|phase| phase.items.values().collect::<Vec<_>>())
        .unwrap_or_default();
    let sprite_pipeline_ready = sprite_items
        .iter()
        .filter(|item| {
            evidence
                .pipelines
                .get_render_pipeline(item.pipeline)
                .is_some()
        })
        .count();
    let ui_pipeline_ready = ui_items
        .iter()
        .filter(|item| {
            evidence
                .pipelines
                .get_render_pipeline(item.pipeline)
                .is_some()
        })
        .count();
    let visible_sprites = visible
        .get::<Sprite>()
        .map(|entities| entities.iter_visible().count())
        .unwrap_or(0);
    let target_ui_nodes = evidence
        .ui_nodes
        .uinodes
        .iter()
        .filter(|node| node.extracted_camera_entity == view_entity)
        .count();
    let nonempty_ui_batches = evidence
        .ui_batches
        .iter()
        .filter(|batch| !batch.range.is_empty())
        .count();
    let target_gpu = evidence.gpu_images.get(snapshot.image);
    let default_gpu = evidence.gpu_images.get(Handle::<Image>::default().id());
    let transparent_gpu = evidence.gpu_images.get(TRANSPARENT_IMAGE_HANDLE.id());
    let output_is_persistent_target = target_gpu.and_then(|image| {
        target
            .out_texture()
            .map(|output| output.id() == image.texture_view.id())
    });
    let main_is_output = target
        .out_texture()
        .map(|output| output.id() == target.main_texture_view().id());

    info!(
        target: "headless_session_diagnostic",
        request = ?snapshot.entity,
        image = ?snapshot.image,
        matching_views = matching_views.len(),
        view = ?view_entity,
        physical_target_size = ?camera.physical_target_size,
        target_gpu_image = target_gpu.is_some(),
        default_gpu_image = default_gpu.is_some(),
        transparent_gpu_image = transparent_gpu.is_some(),
        visible_sprites,
        extracted_sprites = evidence.sprites.sprites.len(),
        sprite_phase_items = sprite_items.len(),
        sprite_phase_pipelines_ready = sprite_pipeline_ready,
        sprite_batches = evidence.sprite_batches.len(),
        extracted_ui_nodes = evidence.ui_nodes.uinodes.len(),
        target_ui_nodes,
        ui_phase_items = ui_items.len(),
        ui_phase_pipelines_ready = ui_pipeline_ready,
        ui_batches = evidence.ui_batches.iter().count(),
        nonempty_ui_batches,
        output_attachment_selected = target.needs_present(),
        output_is_persistent_target = ?output_is_persistent_target,
        main_is_output = ?main_is_output,
        pipelines_ready = global_pipelines.ready,
        pipelines_waiting = global_pipelines.waiting,
        pipelines_failed = global_pipelines.failed,
        "[headless-session-diagnostic] capture_frame"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(entity: Entity) -> MainSnapshot {
        MainSnapshot {
            entity,
            image: AssetId::default(),
            image_assets: 1,
            shader_assets: 2,
            image_events: 3,
            shader_events: 4,
            target_image_in_main: true,
            default_image_in_main: true,
            transparent_image_in_main: true,
        }
    }

    #[test]
    fn each_capture_entity_gets_at_most_one_bounded_snapshot() {
        let first = Entity::from_bits(7);
        let second = Entity::from_bits(8);
        let mut request = DiagnosticRequest::default();
        assert!(request.record(snapshot(first)));
        assert!(!request.record(snapshot(first)));
        assert_eq!(request.take().unwrap().entity, first);
        assert!(request.take().is_none());
        assert!(!request.record(snapshot(first)));
        assert!(request.record(snapshot(second)));
        assert_eq!(request.take().unwrap().entity, second);
    }
}
