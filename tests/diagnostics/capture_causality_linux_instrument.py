"""Instrument disposable Bevy 0.19.1 and woodpecker copies for Linux.

The base instrumentation is shared with the corrected scene-visibility probe.
All edits are exact and apply only to copies below target/.
"""

from pathlib import Path

from diagnostics.capture_scene_visibility_instrument import (
    instrument_adapter as instrument_adapter_base,
    instrument_bevy as instrument_bevy_base,
    replace,
)


LOG_ENV = "WOODPECKER_CAPTURE_CAUSALITY_LINUX_LOG_DIR"
RAW_ENV = "WOODPECKER_CAPTURE_CAUSALITY_LINUX_RAW_DIR"


def instrument_bevy(root: Path) -> None:
    instrument_bevy_base(root)
    source = root / "src"
    diagnostic = source / "capture_causality.rs"
    text = diagnostic.read_text()
    text = text.replace("WOODPECKER_CAPTURE_SCENE_VISIBILITY_LOG_DIR", LOG_ENV)
    text = text.replace("WOODPECKER_CAPTURE_SCENE_VISIBILITY_RAW_DIR", RAW_ENV)
    diagnostic.write_text(text)
    replace(
        diagnostic,
        '''    let row = format!(
        "{{\\"source\\":\\"bevy_render\\",\\"pid\\":{},\\"session_id\\":{:?},\\"artifact_dir\\":{:?},{fields}}}",
        std::process::id(),
        session_id,
        artifact_dir,
    );
''',
        '''    let placement_id = std::fs::read_to_string(
        std::path::Path::new(&artifact_dir).join("linux-capture-placement-id"),
    )
    .unwrap_or_default();
    let row = format!(
        "{{\\"source\\":\\"bevy_render\\",\\"pid\\":{},\\"session_id\\":{:?},\\"artifact_dir\\":{:?},\\"placement_id\\":{:?},{fields}}}",
        std::process::id(),
        session_id,
        artifact_dir,
        placement_id.trim(),
    );
''',
    )

    screenshot = source / "view/window/screenshot.rs"
    replace(
        screenshot,
        '''                    "\\"event\\":\\"screenshot_prepared\\",\\"frame_id\\":{},\\"capture_id\\":{},\\"buffer_id\\":{},\\"capture_texture_id\\":{},\\"screenshot_entity\\":\\"{}\\",\\"window\\":\\"{}\\",\\"width\\":{},\\"height\\":{},\\"format\\":\\"{:?}\\",\\"cameras\\":[{}]",
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
''',
        '''                    "\\"event\\":\\"screenshot_prepared\\",\\"frame_id\\":{},\\"capture_id\\":{},\\"buffer_id\\":{},\\"capture_texture_id\\":{},\\"screenshot_entity\\":\\"{}\\",\\"window\\":\\"{}\\",\\"width\\":{},\\"height\\":{},\\"bytes_per_row\\":{},\\"format\\":\\"{:?}\\",\\"cameras\\":[{}]",
                    state.frame_id,
                    state.capture_id,
                    state.buffer_id,
                    state.texture_id,
                    entity,
                    window,
                    size.width,
                    size.height,
                    gpu_readback::align_byte_size(
                        size.width * view_format.pixel_size().unwrap_or(0) as u32,
                    ),
                    view_format,
                    camera_rows,
''',
    )
    replace(
        screenshot,
        '''            let channels = pixel_size.max(1);
            let color_channels = channels.min(3);
            let nonzero_rgb_bytes = result
                .chunks(channels)
                .map(|pixel| pixel[..color_channels].iter().filter(|value| **value != 0).count())
                .sum::<usize>();
            let mut fnv1a64 = 0xcbf29ce484222325_u64;
''',
        '''            let channels = pixel_size.max(1);
            let color_channels = channels.min(3);
            let nonzero_all_bytes = result.iter().filter(|value| **value != 0).count();
            let nonzero_rgb_bytes = result
                .chunks(channels)
                .map(|pixel| pixel[..color_channels].iter().filter(|value| **value != 0).count())
                .sum::<usize>();
            let alpha = if channels >= 4 {
                result.chunks(channels).map(|pixel| pixel[3]).collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let nonzero_alpha_bytes = alpha.iter().filter(|value| **value != 0).count();
            let alpha_min = alpha.iter().copied().min().unwrap_or(0);
            let alpha_max = alpha.iter().copied().max().unwrap_or(0);
            let mut fnv1a64 = 0xcbf29ce484222325_u64;
''',
    )
    replace(
        screenshot,
        '''            if let Some(raw_dir) = crate::capture_causality::raw_dir() {
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
''',
        '''            if let Some(raw_dir) = crate::capture_causality::raw_dir() {
                std::fs::create_dir_all(&raw_dir)
                    .expect("failed to create Linux capture raw-readback directory");
                std::fs::write(
                    raw_dir.join(format!(
                        "capture-{capture_id}-buffer-{buffer_id}-{width}x{height}-{texture_format:?}.raw"
                    )),
                    &result,
                )
                .expect("failed to write Linux capture raw readback");
            }
            crate::capture_causality::log(&format!(
                "\\"event\\":\\"image_returned\\",\\"frame_id\\":{frame_id},\\"capture_id\\":{capture_id},\\"buffer_id\\":{buffer_id},\\"capture_texture_id\\":{texture_id},\\"screenshot_entity\\":\\"{entity}\\",\\"copy_encoded\\":{copy_encoded},\\"width\\":{width},\\"height\\":{height},\\"format\\":\\"{texture_format:?}\\",\\"byte_length\\":{},\\"nonzero_all_bytes\\":{nonzero_all_bytes},\\"nonzero_rgb_bytes\\":{nonzero_rgb_bytes},\\"nonzero_alpha_bytes\\":{nonzero_alpha_bytes},\\"alpha_min\\":{alpha_min},\\"alpha_max\\":{alpha_max},\\"fnv1a64\\":\\"{fnv1a64:016x}\\"",
                result.len(),
            ));
''',
    )

    renderer = source / "renderer/mod.rs"
    replace(
        renderer,
        '''    let adapter_info = adapter.get_info();
    info!("{:?}", adapter_info);
''',
        '''    let adapter_info = adapter.get_info();
    info!("{:?}", adapter_info);
    crate::capture_causality::log(&format!(
        "\\"event\\":\\"adapter_selected\\",\\"name\\":{:?},\\"vendor\\":{},\\"device\\":{},\\"device_type\\":\\"{:?}\\",\\"driver\\":{:?},\\"driver_info\\":{:?},\\"backend\\":\\"{:?}\\",\\"pci_bus_id\\":{:?}",
        adapter_info.name,
        adapter_info.vendor,
        adapter_info.device,
        adapter_info.device_type,
        adapter_info.driver,
        adapter_info.driver_info,
        adapter_info.backend,
        adapter_info.device_pci_bus_id,
    ));
''',
    )


def instrument_adapter(root: Path) -> None:
    instrument_adapter_base(root)
    replace(
        root / "src/lib.rs",
        "mod capture_scene_visibility_jsonl;\n",
        "#[allow(dead_code)]\nmod capture_scene_visibility_jsonl;\n",
    )
    for path in (
        root / "src/capture_scene_visibility_jsonl.rs",
        root / "src/session/screenshot/capture.rs",
    ):
        text = path.read_text().replace(
            "WOODPECKER_CAPTURE_SCENE_VISIBILITY_LOG_DIR",
            LOG_ENV,
        )
        path.write_text(text)
    capture = root / "src/session/screenshot/capture.rs"
    replace(
        capture,
        '''    let row = serde_json::json!({
        "source": "woodpecker_adapter",
        "pid": std::process::id(),
        "session_id": session_id,
        "artifact_dir": artifact_dir,
        "fields": value,
    });
''',
        '''    let placement_id = std::fs::read_to_string(
        Path::new(&artifact_dir).join("linux-capture-placement-id"),
    )
    .unwrap_or_default();
    let row = serde_json::json!({
        "source": "woodpecker_adapter",
        "pid": std::process::id(),
        "session_id": session_id,
        "artifact_dir": artifact_dir,
        "placement_id": placement_id.trim(),
        "fields": value,
    });
''',
    )


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("bevy_render_copy", type=Path)
    parser.add_argument("woodpecker_copy", type=Path)
    args = parser.parse_args()
    instrument_bevy(args.bevy_render_copy.resolve())
    instrument_adapter(args.woodpecker_copy.resolve())
