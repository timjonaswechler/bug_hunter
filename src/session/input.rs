pub(super) mod keyboard;
pub(super) mod pointer;
pub(super) mod text;

use bevy::{
    input::{
        gestures::{DoubleTapGesture, PanGesture, PinchGesture, RotationGesture},
        keyboard::{KeyboardFocusLost, KeyboardInput},
        mouse::{MouseButtonInput, MouseMotion, MouseWheel},
        touch::TouchInput,
    },
    picking::{input::PointerInputSettings, pointer::PointerInput},
    prelude::*,
    window::{CursorEntered, CursorLeft, CursorMoved, Ime, WindowEvent},
};

use super::window::primary as primary_window;

#[cfg(feature = "ui")]
mod ui;

pub(super) fn install(app: &mut App) {
    if !app.is_plugin_added::<bevy::input::InputPlugin>() {
        app.add_plugins(bevy::input::InputPlugin);
    }
    keyboard::install(app);
    if !app.is_plugin_added::<bevy::picking::PickingPlugin>() {
        app.add_plugins(bevy::picking::PickingPlugin);
    }
    app.add_message::<WindowEvent>()
        .add_message::<CursorMoved>()
        .add_message::<PointerInput>()
        .insert_resource(PointerInputSettings {
            is_mouse_enabled: false,
            is_touch_enabled: false,
        });
    app.world_mut()
        .spawn((Name::new("woodpecker-pointer"), pointer::ID));
    #[cfg(feature = "ui")]
    ui::install(app);
}

/// Keep platform window events separate from messages consumed during simulation.
/// This preserves resize/close notifications and their message cursors while dropping
/// native input, including during long idle periods without message-update schedules.
#[derive(Default)]
pub(super) struct Gate {
    window_events: Messages<WindowEvent>,
}

impl Gate {
    pub(super) fn capture(&mut self, world: &mut World) {
        fn clear<M: Message>(world: &mut World) {
            if let Some(mut messages) = world.get_resource_mut::<Messages<M>>() {
                messages.clear();
            }
        }
        clear::<CursorMoved>(world);
        clear::<CursorEntered>(world);
        clear::<CursorLeft>(world);
        clear::<MouseButtonInput>(world);
        clear::<MouseMotion>(world);
        clear::<MouseWheel>(world);
        clear::<KeyboardInput>(world);
        clear::<KeyboardFocusLost>(world);
        clear::<Ime>(world);
        clear::<TouchInput>(world);
        clear::<PinchGesture>(world);
        clear::<RotationGesture>(world);
        clear::<DoubleTapGesture>(world);
        clear::<PanGesture>(world);
        clear::<PointerInput>(world);
        for event in world.resource_mut::<Messages<WindowEvent>>().drain() {
            match event {
                WindowEvent::CursorEntered(_)
                | WindowEvent::CursorLeft(_)
                | WindowEvent::CursorMoved(_)
                | WindowEvent::MouseButtonInput(_)
                | WindowEvent::MouseMotion(_)
                | WindowEvent::MouseWheel(_)
                | WindowEvent::KeyboardInput(_)
                | WindowEvent::KeyboardFocusLost(_)
                | WindowEvent::Ime(_)
                | WindowEvent::TouchInput(_)
                | WindowEvent::PinchGesture(_)
                | WindowEvent::RotationGesture(_)
                | WindowEvent::DoubleTapGesture(_)
                | WindowEvent::PanGesture(_) => {}
                event => {
                    self.window_events.write(event);
                }
            }
        }
    }

    /// Swap in before a tick and back out afterwards. Only the sanitized buffer
    /// participates in simulation message aging; Winit writes into the other buffer.
    pub(super) fn swap_window_events(&mut self, world: &mut World) {
        std::mem::swap(
            &mut *world.resource_mut::<Messages<WindowEvent>>(),
            &mut self.window_events,
        );
    }
}
