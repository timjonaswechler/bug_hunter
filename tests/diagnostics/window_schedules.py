#!/usr/bin/env python3
"""Static headless probe for the window/schedule/screenshot source seam."""

from __future__ import annotations

import os
from pathlib import Path
import tomllib


ROOT = Path(__file__).resolve().parents[2]
CARGO_HOME = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))


def locked_version(name: str) -> str:
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    versions = {package["version"] for package in lock["package"] if package["name"] == name}
    assert len(versions) == 1, f"expected one locked {name} version, got {sorted(versions)}"
    return versions.pop()


def crate_source(name: str) -> Path:
    version = locked_version(name)
    matches = list((CARGO_HOME / "registry" / "src").glob(f"*/{name}-{version}"))
    assert len(matches) == 1, f"expected installed source for {name} {version}, got {matches}"
    return matches[0]


def ordered(text: str, *fragments: str) -> None:
    offset = 0
    for fragment in fragments:
        found = text.find(fragment, offset)
        assert found >= 0, f"missing source fragment after byte {offset}: {fragment!r}"
        offset = found + len(fragment)


plugin = (ROOT / "src/session/plugin.rs").read_text()
ordered(
    plugin,
    "std::mem::replace(&mut order.labels, vec![Control.intern()])",
    "for label in &bridge.schedules",
    "world.try_run_schedule(*label)",
    "if let Some(receiver) = world.get_resource::<bevy::time::TimeReceiver>()",
)

camera = (crate_source("bevy_render") / "src/camera.rs").read_text()
assert ".add_systems(\n                PostUpdate,\n                camera_system" in camera
ordered(
    camera,
    "pub fn camera_system(",
    "window_resized_reader.read()",
    "window_scale_factor_changed_reader",
    "camera.computed.target_info = Some(new_computed_target_info)",
)

window = (crate_source("bevy_render") / "src/view/window/mod.rs").read_text()
ordered(
    window,
    "pub fn prepare_windows(",
    "match surface.get_current_texture()",
    "wgpu::CurrentSurfaceTexture::Occluded => {}",
)

screenshot = (crate_source("bevy_render") / "src/view/window/screenshot.rs").read_text()
submit = screenshot[screenshot.index("pub(crate) fn submit_screenshot_commands") :]
submit = submit[: submit.index("fn render_screenshot(")]
ordered(
    submit,
    "let Some(swap_chain_texture_view) = window.swap_chain_texture_view.as_ref() else",
    "render_screenshot(",
)
render = screenshot[screenshot.index("fn render_screenshot(") :]
render = render[: render.index("pub(crate) fn collect_screenshots")]
assert "encoder.copy_texture_to_buffer(" in render

metal = (crate_source("wgpu-hal") / "src/metal/surface.rs").read_text()
ordered(
    metal,
    "let occlusion_state: usize",
    "if !is_visible",
    "return Err(crate::SurfaceError::Occluded)",
)

print(
    "confirmed source seam: controlled idle skips camera PostUpdate; "
    "Metal occlusion can remove the swapchain view; screenshot copy then skips"
)
