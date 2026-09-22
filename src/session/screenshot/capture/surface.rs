//! Pin the window-view prerequisite to the frame that extracts this capture.
//!
//! Bevy 0.19 skips the window screenshot copy when its swapchain view or format
//! is absent, but still maps the prepared buffer. A later frame's availability
//! must not validate that earlier, unfilled readback.
use super::Service;
use bevy::{
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems, view::window::ExtractedWindows,
    },
};
use std::sync::{Arc, OnceLock};

#[derive(Resource, Default)]
pub(super) struct Frame(Option<(Entity, Arc<OnceLock<bool>>)>);

impl Frame {
    pub(super) fn new(window: Entity, result: Arc<OnceLock<bool>>) -> Self {
        Self(Some((window, result)))
    }
}

pub(super) fn install(app: &mut App) {
    app.sub_app_mut(RenderApp)
        .init_resource::<Frame>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(
            Render,
            check
                .after(RenderSystems::Prepare)
                .before(RenderSystems::Render),
        );
}

fn extract(service: Extract<Res<Service>>, mut frame: ResMut<Frame>) {
    *frame = service
        .active
        .as_ref()
        .filter(|active| active.surface.get().is_none())
        .map(|active| Frame::new(active.job.window, active.surface.clone()))
        .unwrap_or_default();
}

pub(super) fn check(frame: Res<Frame>, windows: Option<Res<ExtractedWindows>>) {
    let Some((window, result)) = &frame.0 else {
        return;
    };
    let available = windows
        .as_ref()
        .and_then(|windows| windows.get(window))
        .is_some_and(|window| {
            window.swap_chain_texture_view.is_some()
                && window.swap_chain_texture_view_format.is_some()
        });
    // This is a per-request, first-frame result, not a mutable global flag.
    let _ = result.set(available);
}
