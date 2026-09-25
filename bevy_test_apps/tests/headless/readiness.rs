//! Readiness decision used by copy_after_render. No GPU is needed to test it.
use bevy::{
    asset::AssetId,
    camera::{CameraOutputMode, CompositingSpace},
    core_pipeline::blit::BlitPipelineKey,
    prelude::Image,
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

// Mirrors prepare_view_upscaling_pipelines in locked Bevy 0.19.1. That component's
// pipeline ID and key are private. Resolving THIS key through the SAME
// SpecializedRenderPipelines<BlitPipeline> resource returns its memoized ID.
// The probe does not mutate camera/output settings between Prepare and Finish.
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
    pub image: Option<AssetId<Image>>,
    pub has_upscaling_pipeline: bool,
    // needs_present only means the attachment getter was called, NOT a draw.
    pub attachment_selected: bool,
    pub key: Option<BlitPipelineKey>,
}

pub(super) fn output_ready(
    image: AssetId<Image>,
    views: impl IntoIterator<Item = OutputView>,
    resolve: impl FnOnce(BlitPipelineKey) -> PipelineStatus,
) -> bool {
    let mut matching = views.into_iter().filter(|view| view.image == Some(image));
    let Some(view) = matching.next() else {
        return false;
    };
    // The fixed probe has one output camera. Do not silently approve a subset
    // if another camera is later added to the same target.
    if matching.next().is_some() || !view.has_upscaling_pipeline || !view.attachment_selected {
        return false;
    }
    let Some(key) = view.key else { return false };
    resolve(key) == PipelineStatus::Ready
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{camera::ClearColorConfig, shader::ShaderCacheError, tasks::TaskPool};

    fn key() -> BlitPipelineKey {
        upscaling_key(
            &CameraOutputMode::default(),
            0,
            Some(TextureFormat::Rgba8UnormSrgb),
            None,
        )
        .unwrap()
    }

    fn view() -> OutputView {
        OutputView {
            image: Some(AssetId::default()),
            has_upscaling_pipeline: true,
            attachment_selected: true,
            key: Some(key()),
        }
    }

    #[test]
    fn attachment_selection_and_empty_waiting_queue_cannot_approve_a_bad_final_pipeline() {
        // These enter the same decision used immediately before the GPU copy.
        // No waiting-queue input exists: even an empty queue cannot override Err.
        for status in [
            PipelineStatus::Missing,
            PipelineStatus::Waiting,
            PipelineStatus::Failed,
        ] {
            assert!(
                !output_ready(AssetId::default(), [view()], |_| status),
                "{status:?}"
            );
        }
        // Successful lookup is simulated; this is not a GPU/draw proof.
        assert!(output_ready(AssetId::default(), [view()], |_| {
            PipelineStatus::Ready
        }));
    }

    #[test]
    fn real_cached_failure_states_feed_the_copy_decision() {
        let pool = TaskPool::new();
        let states = [
            CachedPipelineState::Queued,
            CachedPipelineState::Creating(pool.spawn(std::future::pending())),
            CachedPipelineState::Err(ShaderCacheError::CreateShaderModule(
                "permanent failure".into(),
            )),
        ];
        for state in &states {
            assert!(!output_ready(AssetId::default(), [view()], |_| {
                pipeline_status(Some(state))
            }));
        }
        assert!(!output_ready(AssetId::default(), [view()], |_| {
            pipeline_status(None)
        }));
    }

    #[test]
    fn missing_component_attachment_view_or_output_never_reaches_pipeline_lookup() {
        let forbidden_lookup = |_| panic!("incomplete output must not resolve a pipeline");
        assert!(!output_ready(AssetId::default(), [], forbidden_lookup));
        let mut missing = view();
        missing.has_upscaling_pipeline = false;
        assert!(!output_ready(
            AssetId::default(),
            [missing],
            forbidden_lookup
        ));
        let mut unselected = view();
        unselected.attachment_selected = false;
        assert!(!output_ready(
            AssetId::default(),
            [unselected],
            forbidden_lookup
        ));
        let mut skipped = view();
        skipped.key = upscaling_key(
            &CameraOutputMode::Skip,
            0,
            Some(TextureFormat::Rgba8UnormSrgb),
            None,
        );
        assert!(!output_ready(
            AssetId::default(),
            [skipped],
            forbidden_lookup
        ));
        let mut no_format = view();
        no_format.key = upscaling_key(&CameraOutputMode::default(), 0, None, None);
        assert!(!output_ready(
            AssetId::default(),
            [no_format],
            forbidden_lookup
        ));
    }

    #[test]
    fn another_target_or_ambiguous_camera_cannot_supply_readiness() {
        let mut other = view();
        other.image = None;
        assert!(!output_ready(AssetId::default(), [other], |_| panic!(
            "wrong target"
        )));
        assert!(!output_ready(
            AssetId::default(),
            [view(), view()],
            |_| panic!("ambiguous target")
        ));
        let mut other = view();
        other.image = Some(AssetId::Uuid {
            uuid: bevy::asset::uuid::Uuid::from_u128(42),
        });
        other.key.as_mut().unwrap().target_format = TextureFormat::Bgra8UnormSrgb;
        assert!(!output_ready(
            AssetId::default(),
            [other, view()],
            |resolved| {
                assert!(resolved == key(), "resolved the other target's pipeline");
                PipelineStatus::Failed
            }
        ));
    }

    #[test]
    fn specialization_key_matches_bevy_output_format_blending_samples_and_color_space() {
        let first = key();
        assert_eq!(first.target_format, TextureFormat::Rgba8UnormSrgb);
        assert_eq!(first.blend_state, None);
        assert_eq!(first.samples, 1);
        assert_eq!(first.source_space, None);
        let later = upscaling_key(
            &CameraOutputMode::default(),
            1,
            Some(TextureFormat::Bgra8UnormSrgb),
            Some(CompositingSpace::Srgb),
        )
        .unwrap();
        assert_eq!(later.blend_state, Some(BlendState::ALPHA_BLENDING));
        assert_eq!(later.target_format, TextureFormat::Bgra8UnormSrgb);
        assert_eq!(later.source_space, Some(CompositingSpace::Srgb));
        let explicit = upscaling_key(
            &CameraOutputMode::Write {
                blend_state: Some(BlendState::REPLACE),
                clear_color: ClearColorConfig::None,
            },
            1,
            Some(TextureFormat::Rgba8UnormSrgb),
            None,
        )
        .unwrap();
        assert_eq!(explicit.blend_state, Some(BlendState::REPLACE));
        let mut output = view();
        output.key = Some(later);
        assert!(output_ready(AssetId::default(), [output], |resolved| {
            assert!(resolved == later);
            PipelineStatus::Ready
        }));
    }
}
