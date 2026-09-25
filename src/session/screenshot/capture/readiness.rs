//! Exact final-output decision for the locked Bevy 0.19.1 image paths.
use super::selection::ImagePipeline;
use bevy::{
    camera::{CameraOutputMode, CompositingSpace, ImageRenderTarget},
    core_pipeline::blit::BlitPipelineKey,
    render::render_resource::{BlendState, CachedPipelineState, Pipeline, TextureFormat},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PipelineStatus {
    Missing,
    Waiting,
    Failed,
    Ready,
}

pub(super) fn pipeline_status(state: Option<&CachedPipelineState>) -> PipelineStatus {
    match state {
        None => PipelineStatus::Missing,
        Some(CachedPipelineState::Queued | CachedPipelineState::Creating(_)) => {
            PipelineStatus::Waiting
        }
        Some(CachedPipelineState::Err(_)) => PipelineStatus::Failed,
        Some(CachedPipelineState::Ok(Pipeline::RenderPipeline(_))) => PipelineStatus::Ready,
        Some(CachedPipelineState::Ok(Pipeline::ComputePipeline(_))) => PipelineStatus::Failed,
    }
}

pub(super) fn additional_pipeline_status(state: &CachedPipelineState) -> PipelineStatus {
    match state {
        CachedPipelineState::Ok(_) => PipelineStatus::Ready,
        state => pipeline_status(Some(state)),
    }
}

// Mirrors prepare_view_upscaling_pipelines in locked Bevy 0.19.1. Its
// ViewUpscalingPipeline fields are private, so resolving this key through the
// same SpecializedRenderPipelines resource returns the memoized final ID.
pub(super) fn upscaling_key(
    mode: &CameraOutputMode,
    sorted_camera_index: usize,
    target_format: Option<TextureFormat>,
    source_space: Option<CompositingSpace>,
) -> Option<BlitPipelineKey> {
    let CameraOutputMode::Write { blend_state, .. } = mode else {
        return None;
    };
    Some(BlitPipelineKey {
        target_format: target_format?,
        blend_state: blend_state
            .or_else(|| (sorted_camera_index > 0).then_some(BlendState::ALPHA_BLENDING)),
        samples: 1,
        source_space,
    })
}

pub(super) struct OutputView {
    pub target: Option<ImageRenderTarget>,
    pub pipeline: Option<ImagePipeline>,
    pub has_3d_depth_texture: bool,
    pub opaque_3d_items: Option<bool>,
    pub has_upscaling_pipeline: bool,
    // Attachment selection alone is never readiness.
    pub attachment_selected: bool,
    pub key: Option<BlitPipelineKey>,
}

pub(super) fn output_ready(
    target: &ImageRenderTarget,
    pipeline: ImagePipeline,
    views: impl IntoIterator<Item = OutputView>,
    resolve: impl FnOnce(BlitPipelineKey) -> PipelineStatus,
    additional_pipelines: impl IntoIterator<Item = PipelineStatus>,
) -> bool {
    let mut matching = views
        .into_iter()
        .filter(|view| view.target.as_ref() == Some(target));
    let Some(view) = matching.next() else {
        return false;
    };
    if matching.next().is_some()
        || view.pipeline != Some(pipeline)
        || !view.has_upscaling_pipeline
        || !view.attachment_selected
        || (pipeline == ImagePipeline::ThreeD
            && (!view.has_3d_depth_texture || view.opaque_3d_items != Some(true)))
    {
        return false;
    }
    let Some(key) = view.key else { return false };
    resolve(key) == PipelineStatus::Ready
        && additional_pipelines
            .into_iter()
            .all(|status| status == PipelineStatus::Ready)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::ClearColorConfig, prelude::Handle, shader::ShaderCacheError, tasks::TaskPool,
    };

    fn key() -> BlitPipelineKey {
        upscaling_key(
            &CameraOutputMode::default(),
            0,
            Some(TextureFormat::Rgba8UnormSrgb),
            None,
        )
        .unwrap()
    }

    fn target(scale_factor: f32) -> ImageRenderTarget {
        ImageRenderTarget {
            handle: Handle::default(),
            scale_factor,
        }
    }

    fn view(pipeline: ImagePipeline) -> OutputView {
        OutputView {
            target: Some(target(1.5)),
            pipeline: Some(pipeline),
            has_3d_depth_texture: pipeline == ImagePipeline::ThreeD,
            opaque_3d_items: (pipeline == ImagePipeline::ThreeD).then_some(true),
            has_upscaling_pipeline: true,
            attachment_selected: true,
            key: Some(key()),
        }
    }

    fn ready(pipeline: ImagePipeline, views: impl IntoIterator<Item = OutputView>) -> bool {
        output_ready(
            &target(1.5),
            pipeline,
            views,
            |_| PipelineStatus::Ready,
            [PipelineStatus::Ready],
        )
    }

    #[test]
    fn arbitrary_ready_pipelines_cannot_replace_the_final_upscaling_pipeline() {
        for final_status in [
            PipelineStatus::Missing,
            PipelineStatus::Waiting,
            PipelineStatus::Failed,
        ] {
            assert!(!output_ready(
                &target(1.5),
                ImagePipeline::TwoD,
                [view(ImagePipeline::TwoD)],
                |_| final_status,
                [PipelineStatus::Ready]
            ));
        }
        assert!(!output_ready(
            &target(1.5),
            ImagePipeline::TwoD,
            [view(ImagePipeline::TwoD)],
            |_| PipelineStatus::Ready,
            [PipelineStatus::Failed]
        ));
        assert!(ready(ImagePipeline::TwoD, [view(ImagePipeline::TwoD)]));
    }

    #[test]
    fn real_cached_failure_states_feed_the_final_pipeline_decision() {
        let pool = TaskPool::new();
        let states = [
            CachedPipelineState::Queued,
            CachedPipelineState::Creating(pool.spawn(std::future::pending())),
            CachedPipelineState::Err(ShaderCacheError::CreateShaderModule(
                "permanent failure".into(),
            )),
        ];
        for state in &states {
            assert_ne!(additional_pipeline_status(state), PipelineStatus::Ready);
            assert!(!output_ready(
                &target(1.5),
                ImagePipeline::TwoD,
                [view(ImagePipeline::TwoD)],
                |_| pipeline_status(Some(state)),
                []
            ));
        }
    }

    #[test]
    fn target_component_attachment_and_write_output_are_all_required() {
        let forbidden = |_| panic!("incomplete output resolved a pipeline");
        assert!(!output_ready(
            &target(1.5),
            ImagePipeline::TwoD,
            [],
            forbidden,
            []
        ));
        let mut missing = view(ImagePipeline::TwoD);
        missing.has_upscaling_pipeline = false;
        assert!(!output_ready(
            &target(1.5),
            ImagePipeline::TwoD,
            [missing],
            forbidden,
            []
        ));
        let mut unselected = view(ImagePipeline::TwoD);
        unselected.attachment_selected = false;
        assert!(!output_ready(
            &target(1.5),
            ImagePipeline::TwoD,
            [unselected],
            forbidden,
            []
        ));
        let mut skipped = view(ImagePipeline::TwoD);
        skipped.key = upscaling_key(
            &CameraOutputMode::Skip,
            0,
            Some(TextureFormat::Rgba8UnormSrgb),
            None,
        );
        assert!(!output_ready(
            &target(1.5),
            ImagePipeline::TwoD,
            [skipped],
            forbidden,
            []
        ));
    }

    #[test]
    fn perspective_3d_requires_its_graph_depth_and_opaque_mesh_phase() {
        let forbidden = |_| panic!("incomplete 3D output resolved a pipeline");
        let mut wrong_pipeline = view(ImagePipeline::TwoD);
        wrong_pipeline.has_3d_depth_texture = true;
        wrong_pipeline.opaque_3d_items = Some(true);
        assert!(!output_ready(
            &target(1.5),
            ImagePipeline::ThreeD,
            [wrong_pipeline],
            forbidden,
            []
        ));
        let mut missing_depth = view(ImagePipeline::ThreeD);
        missing_depth.has_3d_depth_texture = false;
        assert!(!output_ready(
            &target(1.5),
            ImagePipeline::ThreeD,
            [missing_depth],
            forbidden,
            []
        ));
        let mut empty_phase = view(ImagePipeline::ThreeD);
        empty_phase.opaque_3d_items = Some(false);
        assert!(!output_ready(
            &target(1.5),
            ImagePipeline::ThreeD,
            [empty_phase],
            forbidden,
            []
        ));
        assert!(ready(ImagePipeline::ThreeD, [view(ImagePipeline::ThreeD)]));
    }

    #[test]
    fn missing_opaque_3d_phase_resource_does_not_block_2d_but_fails_3d_closed() {
        let mut two_d = view(ImagePipeline::TwoD);
        two_d.opaque_3d_items = None;
        assert!(ready(ImagePipeline::TwoD, [two_d]));

        let mut three_d = view(ImagePipeline::ThreeD);
        three_d.opaque_3d_items = None;
        let forbidden = |_| panic!("missing 3D phase resource resolved a pipeline");
        assert!(!output_ready(
            &target(1.5),
            ImagePipeline::ThreeD,
            [three_d],
            forbidden,
            []
        ));
    }

    #[test]
    fn same_image_handle_at_another_scale_is_not_the_capture_target() {
        let forbidden = |_| panic!("different normalized target resolved a pipeline");
        assert!(!output_ready(
            &target(1.0),
            ImagePipeline::TwoD,
            [view(ImagePipeline::TwoD)],
            forbidden,
            []
        ));
        assert!(ready(ImagePipeline::TwoD, [view(ImagePipeline::TwoD)]));
    }

    #[test]
    fn target_is_unique_and_specialization_key_matches_bevy() {
        assert!(!ready(
            ImagePipeline::TwoD,
            [view(ImagePipeline::TwoD), view(ImagePipeline::TwoD)]
        ));
        let explicit = upscaling_key(
            &CameraOutputMode::Write {
                blend_state: Some(BlendState::REPLACE),
                clear_color: ClearColorConfig::None,
            },
            1,
            Some(TextureFormat::Rgba8UnormSrgb),
            Some(CompositingSpace::Srgb),
        )
        .unwrap();
        assert_eq!(explicit.blend_state, Some(BlendState::REPLACE));
        assert_eq!(explicit.samples, 1);
        assert_eq!(explicit.source_space, Some(CompositingSpace::Srgb));
    }
}
