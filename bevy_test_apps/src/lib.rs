/// Dimensions of the data-only surface used for Logical Mode UI layout and pointer targeting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalSurface {
    width: u32,
    height: u32,
}

impl LogicalSurface {
    pub fn new(width: u32, height: u32) -> Self {
        assert!(width > 0, "logical surface width must be greater than zero");
        assert!(
            height > 0,
            "logical surface height must be greater than zero"
        );
        Self { width, height }
    }
}

pub mod composition {
    use super::LogicalSurface;
    use bevy::{
        asset::AssetPlugin,
        camera::CameraPlugin,
        image::{ImagePlugin, TextureAtlasPlugin},
        mesh::MeshPlugin,
        picking::{InteractionPlugin, PickingPlugin},
        prelude::*,
        text::TextPlugin,
        transform::TransformPlugin,
        ui::UiPlugin,
        ui_widgets::EditableTextInputPlugin,
        window::{PrimaryWindow, WindowResolution},
    };

    /// Adds a rendered Player Run or, with `automation`, a rendered Controlled Session.
    pub fn rendered(app: &mut App, window: Window) -> &mut App {
        #[cfg(feature = "automation")]
        {
            app.add_plugins(
                DefaultPlugins
                    .set(WindowPlugin {
                        primary_window: Some(window),
                        ..default()
                    })
                    .build()
                    .disable::<bevy::input::InputPlugin>()
                    .disable::<bevy::gilrs::GilrsPlugin>(),
            )
            .add_plugins((
                automation_control::AutomationControlPlugin::rendered_stdio(),
                automation_control::screenshot::Plugin::default(),
            ))
        }

        #[cfg(not(feature = "automation"))]
        {
            app.add_plugins(DefaultPlugins.set(WindowPlugin {
                primary_window: Some(window),
                ..default()
            }))
        }
    }

    /// Adds a renderer-free Logical Mode composition with one fixed, data-only UI surface.
    pub fn logical(app: &mut App, surface: LogicalSurface) -> &mut App {
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins((
                TransformPlugin,
                ImagePlugin::default(),
                TextureAtlasPlugin,
                MeshPlugin,
                CameraPlugin,
                TextPlugin,
                PickingPlugin,
                InteractionPlugin,
                UiPlugin,
                EditableTextInputPlugin,
            ));

        #[cfg(feature = "automation")]
        app.add_plugins(automation_control::AutomationControlPlugin::logical_stdio());

        app.world_mut().spawn((
            Name::new("logical-surface"),
            PrimaryWindow,
            Window {
                resolution: WindowResolution::new(surface.width, surface.height)
                    .with_scale_factor_override(1.0),
                resizable: false,
                ..default()
            },
        ));
        app
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        input::InputPlugin,
        prelude::*,
        render::{RenderApp, RenderPlugin},
        window::{PrimaryWindow, WindowPlugin},
    };

    #[test]
    fn logical_composition_has_a_fixed_surface_without_native_or_render_plugins() {
        let mut app = App::new();
        composition::logical(&mut app, LogicalSurface::new(640, 360));

        assert!(!app.is_plugin_added::<InputPlugin>());
        assert!(!app.is_plugin_added::<bevy::gilrs::GilrsPlugin>());
        assert!(!app.is_plugin_added::<bevy::winit::WinitPlugin>());
        assert!(!app.is_plugin_added::<bevy::picking::input::PointerInputPlugin>());
        assert!(!app.is_plugin_added::<WindowPlugin>());
        assert!(!app.is_plugin_added::<RenderPlugin>());
        assert!(app.is_plugin_added::<bevy::picking::PickingPlugin>());
        assert!(app.is_plugin_added::<bevy::ui_widgets::EditableTextInputPlugin>());
        assert!(app.get_sub_app(RenderApp).is_none());

        let mut windows = app
            .world_mut()
            .query_filtered::<&Window, With<PrimaryWindow>>();
        let window = windows.single(app.world()).unwrap();
        assert_eq!(window.resolution.physical_width(), 640);
        assert_eq!(window.resolution.physical_height(), 360);
        assert_eq!(window.resolution.scale_factor(), 1.0);
        assert!(!window.resizable);
    }
}
