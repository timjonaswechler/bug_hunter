"""Apply the temporary Bevy 0.19.1 capture-causality instrumentation.

The caller must pass a disposable copy of bevy_render. Registry sources are
never changed. Replacements are deliberately exact so a Bevy source change
fails preparation rather than producing a subtly different measurement.
"""

from pathlib import Path


DIAGNOSTIC_MODULE = r'''
use std::{
    fs::OpenOptions,
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
};

static FRAME_ID: AtomicU64 = AtomicU64::new(0);
static CAPTURE_ID: AtomicU64 = AtomicU64::new(0);
static BUFFER_ID: AtomicU64 = AtomicU64::new(0);
static TEXTURE_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn enabled() -> bool {
    std::env::var_os("WOODPECKER_CAPTURE_CAUSALITY_LOG").is_some()
}

pub(crate) fn next_frame() -> u64 {
    FRAME_ID.fetch_add(1, Ordering::Relaxed) + 1
}

pub(crate) fn frame() -> u64 {
    FRAME_ID.load(Ordering::Relaxed)
}

pub(crate) fn next_capture_ids() -> (u64, u64, u64) {
    (
        CAPTURE_ID.fetch_add(1, Ordering::Relaxed) + 1,
        BUFFER_ID.fetch_add(1, Ordering::Relaxed) + 1,
        TEXTURE_ID.fetch_add(1, Ordering::Relaxed) + 1,
    )
}

pub(crate) fn log(fields: &str) {
    let Some(path) = std::env::var_os("WOODPECKER_CAPTURE_CAUSALITY_LOG") else {
        return;
    };
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{{\"source\":\"bevy_render\",{fields}}}");
}

pub(crate) fn raw_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("WOODPECKER_CAPTURE_CAUSALITY_RAW_DIR").map(Into::into)
}
'''.lstrip()


def replace(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one source block, found {count}")
    path.write_text(text.replace(old, new))


def instrument(root: Path) -> None:
    source = root / "src"
    (source / "capture_causality.rs").write_text(DIAGNOSTIC_MODULE)
    replace(
        source / "lib.rs",
        "extern crate alloc;\n",
        "extern crate alloc;\n\nmod capture_causality;\n",
    )

    window = source / "view/window/mod.rs"
    replace(
        window,
        ") {\n    for window in windows.windows.values_mut() {\n"
        "        // Skip acquiring a swap-chain texture for windows that no camera\n",
        ") {\n    let diagnostic_frame = crate::capture_causality::next_frame();\n"
        "    for window in windows.windows.values_mut() {\n"
        "        // Skip acquiring a swap-chain texture for windows that no camera\n",
    )
    replace(
        window,
        """        if !is_camera_target && !window.needs_initial_present {
            continue;
        }

        let window_surfaces = window_surfaces.deref_mut();
        let Some(surface_data) = window_surfaces.surfaces.get(&window.entity) else {
            continue;
        };
""",
        """        let write_camera_count = sorted_cameras
            .0
            .iter()
            .filter(|camera| {
                matches!(
                    &camera.target,
                    Some(bevy_camera::NormalizedRenderTarget::Window(target))
                        if target.entity() == window.entity
                ) && matches!(camera.output_mode, bevy_camera::CameraOutputMode::Write { .. })
            })
            .count();
        if !is_camera_target && !window.needs_initial_present {
            crate::capture_causality::log(&format!(
                "\\"event\\":\\"surface_acquire\\",\\"frame_id\\":{diagnostic_frame},\\"window\\":\\"{}\\",\\"outcome\\":\\"skipped_no_camera_target\\",\\"is_camera_target\\":false,\\"write_camera_count\\":{write_camera_count},\\"needs_initial_present\\":false,\\"surface_exists\\":{},\\"physical_width\\":{},\\"physical_height\\":{}",
                window.entity,
                window_surfaces.surfaces.contains_key(&window.entity),
                window.physical_width,
                window.physical_height,
            ));
            continue;
        }

        let window_surfaces = window_surfaces.deref_mut();
        let Some(surface_data) = window_surfaces.surfaces.get(&window.entity) else {
            crate::capture_causality::log(&format!(
                "\\"event\\":\\"surface_acquire\\",\\"frame_id\\":{diagnostic_frame},\\"window\\":\\"{}\\",\\"outcome\\":\\"skipped_surface_missing\\",\\"is_camera_target\\":{is_camera_target},\\"write_camera_count\\":{write_camera_count},\\"needs_initial_present\\":{},\\"surface_exists\\":false,\\"physical_width\\":{},\\"physical_height\\":{}",
                window.entity,
                window.needs_initial_present,
                window.physical_width,
                window.physical_height,
            ));
            continue;
        };
""",
    )
    replace(
        window,
        """        let surface = &surface_data.surface;
        match surface.get_current_texture() {
""",
        """        let surface = &surface_data.surface;
        let acquisition = surface.get_current_texture();
        let acquisition_name = match &acquisition {
            wgpu::CurrentSurfaceTexture::Success(_) => "success",
            wgpu::CurrentSurfaceTexture::Suboptimal(_) => "suboptimal",
            wgpu::CurrentSurfaceTexture::Timeout => "timeout",
            wgpu::CurrentSurfaceTexture::Outdated => "outdated",
            wgpu::CurrentSurfaceTexture::Lost => "lost",
            wgpu::CurrentSurfaceTexture::Occluded => "occluded",
            wgpu::CurrentSurfaceTexture::Validation => "validation",
        };
        crate::capture_causality::log(&format!(
            "\\"event\\":\\"surface_acquire\\",\\"frame_id\\":{diagnostic_frame},\\"window\\":\\"{}\\",\\"outcome\\":\\"{acquisition_name}\\",\\"is_camera_target\\":{is_camera_target},\\"write_camera_count\\":{write_camera_count},\\"needs_initial_present\\":{},\\"surface_exists\\":true,\\"physical_width\\":{},\\"physical_height\\":{},\\"surface_width\\":{},\\"surface_height\\":{},\\"size_changed\\":{},\\"present_mode_changed\\":{}",
            window.entity,
            window.needs_initial_present,
            window.physical_width,
            window.physical_height,
            surface_data.configuration.width,
            surface_data.configuration.height,
            window.size_changed,
            window.present_mode_changed,
        ));
        match acquisition {
""",
    )

    screenshot = source / "view/window/screenshot.rs"
    replace(
        screenshot,
        """    sync::{
        mpsc::{Receiver, Sender},
        Mutex,
""",
        """    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender},
        Mutex,
""",
    )
    replace(
        screenshot,
        """    pub pipeline_id: CachedRenderPipelineId,
    pub size: Extent3d,
}
""",
        """    pub pipeline_id: CachedRenderPipelineId,
    pub size: Extent3d,
    pub frame_id: u64,
    pub capture_id: u64,
    pub buffer_id: u64,
    pub texture_id: u64,
    pub copy_encoded: AtomicBool,
}
""",
    )
    replace(
        screenshot,
        """    manual_texture_views: Res<ManualTextureViews>,
    mut view_target_attachments: ResMut<ViewTargetAttachments>,
) {
""",
        """    manual_texture_views: Res<ManualTextureViews>,
    mut view_target_attachments: ResMut<ViewTargetAttachments>,
    cameras: Query<(Entity, &crate::camera::ExtractedCamera)>,
) {
""",
    )
    replace(
        screenshot,
        """                let size = Extent3d {
                    width: surface_data.configuration.width,
                    height: surface_data.configuration.height,
                    ..default()
                };
                let (texture_view, state) = prepare_screenshot_state(
                    size,
                    view_format,
                    &render_device,
                    &screenshot_pipeline,
                    &pipeline_cache,
                    &mut pipelines,
                );
                prepared.insert(*entity, state);
""",
        """                let size = Extent3d {
                    width: surface_data.configuration.width,
                    height: surface_data.configuration.height,
                    ..default()
                };
                let (texture_view, state) = prepare_screenshot_state(
                    size,
                    view_format,
                    &render_device,
                    &screenshot_pipeline,
                    &pipeline_cache,
                    &mut pipelines,
                );
                let camera_rows = cameras
                    .iter()
                    .filter(|(_, camera)| {
                        matches!(
                            &camera.target,
                            Some(NormalizedRenderTarget::Window(target))
                                if target.entity() == window
                        )
                    })
                    .map(|(camera_entity, camera)| {
                        format!(
                            "{{\\"entity\\":\\"{camera_entity}\\",\\"physical_target_size\\":\\"{:?}\\",\\"physical_viewport_size\\":\\"{:?}\\",\\"viewport\\":\\"{:?}\\",\\"output_mode\\":\\"{:?}\\",\\"clear_color\\":\\"{:?}\\"}}",
                            camera.physical_target_size,
                            camera.physical_viewport_size,
                            camera.viewport,
                            camera.output_mode,
                            camera.clear_color,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                crate::capture_causality::log(&format!(
                    "\\"event\\":\\"screenshot_prepared\\",\\"frame_id\\":{},\\"capture_id\\":{},\\"buffer_id\\":{},\\"capture_texture_id\\":{},\\"screenshot_entity\\":\\"{}\\",\\"window\\":\\"{}\\",\\"width\\":{},\\"height\\":{},\\"format\\":\\"{:?}\\",\\"cameras\\":[{}]",
                    state.frame_id,
                    state.capture_id,
                    state.buffer_id,
                    state.texture_id,
                    entity,
                    window,
                    size.width,
                    size.height,
                    view_format,
                    camera_rows,
                ));
                prepared.insert(*entity, state);
""",
    )
    replace(
        screenshot,
        """) -> (TextureView, ScreenshotPreparedState) {
    let texture = render_device.create_texture(&wgpu::TextureDescriptor {
""",
        """) -> (TextureView, ScreenshotPreparedState) {
    let frame_id = crate::capture_causality::frame();
    let (capture_id, buffer_id, texture_id) =
        crate::capture_causality::next_capture_ids();
    let texture = render_device.create_texture(&wgpu::TextureDescriptor {
""",
    )
    replace(
        screenshot,
        """            pipeline_id,
            size,
        },
""",
        """            pipeline_id,
            size,
            frame_id,
            capture_id,
            buffer_id,
            texture_id,
            copy_encoded: AtomicBool::new(false),
        },
""",
    )
    replace(
        screenshot,
        """                let Some(window) = windows.get(&window) else {
                    continue;
                };
""",
        """                let Some(window) = windows.get(&window) else {
                    log_skipped_copy(prepared, entity, "extracted_window_missing");
                    continue;
                };
""",
    )
    replace(
        screenshot,
        """                let Some(texture_format) = window.swap_chain_texture_view_format else {
                    continue;
                };
""",
        """                let Some(texture_format) = window.swap_chain_texture_view_format else {
                    log_skipped_copy(prepared, entity, "view_format_missing");
                    continue;
                };
""",
    )
    replace(
        screenshot,
        """                let Some(swap_chain_texture_view) = window.swap_chain_texture_view.as_ref() else {
                    continue;
                };
""",
        """                let Some(swap_chain_texture_view) = window.swap_chain_texture_view.as_ref() else {
                    log_skipped_copy(prepared, entity, "swap_chain_view_missing");
                    continue;
                };
""",
    )
    replace(
        screenshot,
        """fn render_screenshot(
""",
        """fn log_skipped_copy(
    prepared: &RenderScreenshotsPrepared,
    entity: &Entity,
    reason: &str,
) {
    if let Some(state) = prepared.get(entity) {
        crate::capture_causality::log(&format!(
            "\\"event\\":\\"copy_skipped\\",\\"frame_id\\":{},\\"capture_id\\":{},\\"buffer_id\\":{},\\"capture_texture_id\\":{},\\"screenshot_entity\\":\\"{}\\",\\"reason\\":\\"{reason}\\"",
            state.frame_id, state.capture_id, state.buffer_id, state.texture_id, entity,
        ));
    }
}

fn render_screenshot(
""",
    )
    replace(
        screenshot,
        """            extent,
        );

        if let Some(pipeline) = pipelines.get_render_pipeline(prepared_state.pipeline_id) {
""",
        """            extent,
        );
        prepared_state.copy_encoded.store(true, Ordering::Release);
        crate::capture_causality::log(&format!(
            "\\"event\\":\\"copy_texture_to_buffer_encoded\\",\\"frame_id\\":{},\\"capture_id\\":{},\\"buffer_id\\":{},\\"capture_texture_id\\":{},\\"screenshot_entity\\":\\"{}\\",\\"width\\":{width},\\"height\\":{height},\\"format\\":\\"{texture_format:?}\\"",
            prepared_state.frame_id,
            prepared_state.capture_id,
            prepared_state.buffer_id,
            prepared_state.texture_id,
            entity,
        ));

        if let Some(pipeline) = pipelines.get_render_pipeline(prepared_state.pipeline_id) {
""",
    )
    replace(
        screenshot,
        """pub(crate) fn collect_screenshots(world: &mut World) {
""",
        """pub(crate) fn diagnostic_queue_submitted(world: &World) {
    if !crate::capture_causality::enabled() {
        return;
    }
    for (entity, state) in world.resource::<RenderScreenshotsPrepared>().iter() {
        crate::capture_causality::log(&format!(
            "\\"event\\":\\"queue_submitted\\",\\"frame_id\\":{},\\"capture_id\\":{},\\"buffer_id\\":{},\\"capture_texture_id\\":{},\\"screenshot_entity\\":\\"{}\\",\\"copy_encoded\\":{}",
            state.frame_id,
            state.capture_id,
            state.buffer_id,
            state.texture_id,
            entity,
            state.copy_encoded.load(Ordering::Acquire),
        ));
    }
}

pub(crate) fn collect_screenshots(world: &mut World) {
""",
    )
    replace(
        screenshot,
        """        let buffer = prepared.buffer.clone();

        let finish = async move {
""",
        """        let buffer = prepared.buffer.clone();
        let frame_id = prepared.frame_id;
        let capture_id = prepared.capture_id;
        let buffer_id = prepared.buffer_id;
        let texture_id = prepared.texture_id;
        let copy_encoded = prepared.copy_encoded.load(Ordering::Acquire);
        crate::capture_causality::log(&format!(
            "\\"event\\":\\"map_requested\\",\\"frame_id\\":{frame_id},\\"capture_id\\":{capture_id},\\"buffer_id\\":{buffer_id},\\"capture_texture_id\\":{texture_id},\\"screenshot_entity\\":\\"{entity}\\",\\"copy_encoded\\":{copy_encoded}"
        ));

        let finish = async move {
""",
    )
    replace(
        screenshot,
        """            rx.recv().await.unwrap();
            let data = buffer_slice.get_mapped_range();
""",
        """            rx.recv().await.unwrap();
            crate::capture_causality::log(&format!(
                "\\"event\\":\\"map_success\\",\\"frame_id\\":{frame_id},\\"capture_id\\":{capture_id},\\"buffer_id\\":{buffer_id},\\"capture_texture_id\\":{texture_id},\\"screenshot_entity\\":\\"{entity}\\",\\"copy_encoded\\":{copy_encoded}"
            ));
            let data = buffer_slice.get_mapped_range();
""",
    )
    replace(
        screenshot,
        """            if let Err(e) = sender.send((
                entity,
                Image::new(
""",
        """            let channels = pixel_size.max(1);
            let color_channels = channels.min(3);
            let nonzero_rgb_bytes = result
                .chunks(channels)
                .map(|pixel| pixel[..color_channels].iter().filter(|value| **value != 0).count())
                .sum::<usize>();
            let mut fnv1a64 = 0xcbf29ce484222325_u64;
            for byte in &result {
                fnv1a64 ^= u64::from(*byte);
                fnv1a64 = fnv1a64.wrapping_mul(0x100000001b3);
            }
            if let Some(raw_dir) = crate::capture_causality::raw_dir() {
                let _ = std::fs::create_dir_all(&raw_dir);
                let _ = std::fs::write(
                    raw_dir.join(format!(
                        "capture-{capture_id}-buffer-{buffer_id}-{width}x{height}-{texture_format:?}.raw"
                    )),
                    &result,
                );
            }
            crate::capture_causality::log(&format!(
                "\\"event\\":\\"image_returned\\",\\"frame_id\\":{frame_id},\\"capture_id\\":{capture_id},\\"buffer_id\\":{buffer_id},\\"capture_texture_id\\":{texture_id},\\"screenshot_entity\\":\\"{entity}\\",\\"copy_encoded\\":{copy_encoded},\\"width\\":{width},\\"height\\":{height},\\"format\\":\\"{texture_format:?}\\",\\"byte_length\\":{},\\"nonzero_rgb_bytes\\":{nonzero_rgb_bytes},\\"fnv1a64\\":\\"{fnv1a64:016x}\\"",
                result.len(),
            ));
            if let Err(e) = sender.send((
                entity,
                Image::new(
""",
    )

    renderer = source / "renderer/mod.rs"
    replace(
        renderer,
        """        render_queue.submit([encoder.finish()]);
    }
""",
        """        render_queue.submit([encoder.finish()]);
        crate::view::screenshot::diagnostic_queue_submitted(world);
    }
""",
    )


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("bevy_render_copy", type=Path)
    args = parser.parse_args()
    instrument(args.bevy_render_copy.resolve())
