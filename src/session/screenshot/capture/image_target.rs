//! Explicit fixed image target selection and capture-frame verification.
use super::{Service, readiness, selection, verification::Verification};
use bevy::{
    camera::{CameraOutputMode, ImageRenderTarget, Projection, RenderTarget},
    core_pipeline::{blit::BlitPipeline, core_3d::Opaque3d, upscaling::ViewUpscalingPipeline},
    prelude::*,
    render::{
        Extract, ExtractSchedule, RenderApp,
        camera::ExtractedCamera,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{PipelineCache, SpecializedRenderPipelines},
        renderer::{RenderGraph, RenderGraphSystems},
        view::{ExtractedView, ViewDepthTexture, ViewTarget},
    },
};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Selected {
    pub target: ImageRenderTarget,
    pub pipeline: selection::ImagePipeline,
    camera: Entity,
    projection: Option<PerspectiveState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PerspectiveState {
    fov: u32,
    aspect_ratio: u32,
    near: u32,
    far: u32,
    near_clip_plane: [u32; 4],
}

impl PerspectiveState {
    fn from_projection(projection: &bevy::camera::PerspectiveProjection) -> Self {
        Self {
            fov: projection.fov.to_bits(),
            aspect_ratio: projection.aspect_ratio.to_bits(),
            near: projection.near.to_bits(),
            far: projection.far.to_bits(),
            near_clip_plane: projection.near_clip_plane.to_array().map(f32::to_bits),
        }
    }
}

#[derive(Clone)]
pub(super) struct Request {
    pub selected: Selected,
    pub entity: Option<Entity>,
    pub verification: Arc<Verification>,
}

#[derive(Resource, Default)]
struct RenderRequest(Option<Request>);

pub(super) fn install(app: &mut App) {
    app.sub_app_mut(RenderApp)
        .init_resource::<RenderRequest>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(
            RenderGraph,
            verify_fixed_image_output.in_set(RenderGraphSystems::Finish),
        );
}

pub(super) fn configured(world: &World) -> bool {
    world.iter_entities().any(|entity| {
        #[cfg(feature = "headless-2d")]
        if entity.contains::<crate::session::HeadlessCaptureCamera2d>() {
            return true;
        }
        #[cfg(feature = "headless-3d")]
        if entity.contains::<crate::session::HeadlessCaptureCamera3d>() {
            return true;
        }
        false
    })
}

pub(super) fn target(world: &World) -> Result<Selected, super::Diagnostic> {
    if world
        .iter_entities()
        .any(|entity| entity.contains::<Window>())
    {
        return Err(super::Diagnostic::new(
            "screenshot_target_unavailable",
            "headless image capture does not support Window entities",
        ));
    }
    let image_cameras: Vec<_> = world
        .iter_entities()
        .filter_map(|entity| {
            let camera = entity.get::<Camera>()?;
            let RenderTarget::Image(target) = entity.get::<RenderTarget>()? else {
                return None;
            };
            Some((entity, camera, target))
        })
        .collect();
    let [(entity, camera, target)] = image_cameras.as_slice() else {
        return Err(super::Diagnostic::new(
            "screenshot_target_unavailable",
            "expected one explicitly selected image-target camera",
        ));
    };
    let image = world
        .get_resource::<Assets<Image>>()
        .and_then(|images| images.get(&target.handle));
    let dimensions_match = image.is_some_and(|image| {
        camera.computed.target_info.as_ref().is_some_and(|info| {
            image.texture_descriptor.size.width > 0
                && image.texture_descriptor.size.height > 0
                && info.physical_size
                    == UVec2::new(
                        image.texture_descriptor.size.width,
                        image.texture_descriptor.size.height,
                    )
                && info.scale_factor.to_bits() == target.scale_factor.to_bits()
        })
    });
    let common = (
        camera.is_active,
        camera.viewport.is_none(),
        matches!(camera.output_mode, CameraOutputMode::Write { .. }),
        target.scale_factor.is_finite() && target.scale_factor > 0.0,
        dimensions_match,
    );

    #[cfg(feature = "headless-2d")]
    let marked_2d = entity.contains::<crate::session::HeadlessCaptureCamera2d>();
    #[cfg(not(feature = "headless-2d"))]
    let marked_2d = false;
    if selection::fixed_2d_camera_supported(
        marked_2d,
        entity.contains::<Camera2d>() && !entity.contains::<Camera3d>(),
        common.0,
        common.1,
        common.2,
        common.3,
        common.4,
    ) {
        return Ok(Selected {
            target: (*target).clone(),
            pipeline: selection::ImagePipeline::TwoD,
            camera: entity.id(),
            projection: None,
        });
    }

    #[cfg(feature = "headless-3d")]
    let marked_3d = entity.contains::<crate::session::HeadlessCaptureCamera3d>();
    #[cfg(not(feature = "headless-3d"))]
    let marked_3d = false;
    if let Some(Projection::Perspective(projection)) = entity.get::<Projection>() {
        let expected_aspect = camera
            .physical_target_size()
            .map(|size| size.x as f32 / size.y as f32);
        let projection_valid = projection.fov.is_finite()
            && projection.fov > 0.0
            && projection.fov < std::f32::consts::PI
            && projection.aspect_ratio.is_finite()
            && expected_aspect
                .is_some_and(|aspect| (projection.aspect_ratio - aspect).abs() < 0.001)
            && projection.near.is_finite()
            && projection.near > 0.0
            && projection.far.is_finite()
            && projection.far > projection.near
            && projection.near_clip_plane.is_finite();
        if selection::fixed_3d_camera_supported(selection::Fixed3dCamera {
            marked: marked_3d,
            camera_3d: entity.contains::<Camera3d>() && !entity.contains::<Camera2d>(),
            perspective: true,
            active: common.0,
            full_viewport: common.1,
            writes_output: common.2,
            valid_scale: common.3,
            dimensions_match: common.4,
            valid_projection: projection_valid,
        }) {
            return Ok(Selected {
                target: (*target).clone(),
                pipeline: selection::ImagePipeline::ThreeD,
                camera: entity.id(),
                projection: Some(PerspectiveState::from_projection(projection)),
            });
        }
    }

    Err(super::Diagnostic::new(
        "screenshot_target_unavailable",
        "selected image target requires one initialized active full-image writing camera of an enabled fixed 2D or perspective 3D kind",
    ))
}

fn extract(service: Extract<Res<Service>>, mut request: ResMut<RenderRequest>) {
    request.0 = service
        .active
        .as_ref()
        .and_then(super::Active::image_request);
}

fn verify_fixed_image_output(world: &mut World) {
    let Some(request) = world.resource::<RenderRequest>().0.clone() else {
        return;
    };
    let raw_views: Vec<_> = world
        .query::<(
            &ExtractedCamera,
            &ExtractedView,
            &ViewTarget,
            Has<ViewUpscalingPipeline>,
            Has<Camera2d>,
            Has<Camera3d>,
            Has<ViewDepthTexture>,
        )>()
        .iter(world)
        .map(
            |(camera, extracted_view, view, has_upscaling, camera_2d, camera_3d, depth)| {
                let pipeline = match (camera_2d, camera_3d) {
                    (true, false) => Some(selection::ImagePipeline::TwoD),
                    (false, true) => Some(selection::ImagePipeline::ThreeD),
                    _ => None,
                };
                (
                    extracted_view.retained_view_entity,
                    readiness::OutputView {
                        target: match &camera.target {
                            Some(bevy::camera::NormalizedRenderTarget::Image(target)) => {
                                Some(target.clone())
                            }
                            _ => None,
                        },
                        pipeline,
                        has_3d_depth_texture: depth,
                        opaque_3d_items: None,
                        has_upscaling_pipeline: has_upscaling,
                        attachment_selected: view.needs_present(),
                        key: readiness::upscaling_key(
                            &camera.output_mode,
                            camera.sorted_camera_index_for_target,
                            view.out_texture_view_format(),
                            view.compositing_space,
                        ),
                    },
                )
            },
        )
        .collect();
    let opaque_phases = world.get_resource::<ViewBinnedRenderPhases<Opaque3d>>();
    let views: Vec<_> = raw_views
        .into_iter()
        .map(|(retained, mut view)| {
            if view.pipeline == Some(selection::ImagePipeline::ThreeD) {
                view.opaque_3d_items = opaque_phases
                    .and_then(|phases| phases.get(&retained))
                    .map(|phase| !phase.is_empty());
            }
            view
        })
        .collect();
    let additional: Vec<_> = world
        .resource::<PipelineCache>()
        .pipelines()
        .map(|pipeline| readiness::additional_pipeline_status(&pipeline.state))
        .collect();
    let ready = world.resource_scope(
        |world, mut pipelines: Mut<SpecializedRenderPipelines<BlitPipeline>>| {
            let cache = world.resource::<PipelineCache>();
            readiness::output_ready(
                &request.selected.target,
                request.selected.pipeline,
                views,
                |key| {
                    let id = pipelines.specialize(cache, world.resource::<BlitPipeline>(), key);
                    readiness::pipeline_status(Some(cache.get_render_pipeline_state(id)))
                },
                additional,
            )
        },
    );
    request.verification.record_frame(request.entity, ready);
}
