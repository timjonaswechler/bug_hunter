//! Adapt Bevy's legacy Interaction system without exposing the virtual cursor to Winit.
use bevy::{
    camera::NormalizedRenderTarget,
    ecs::schedule::ScheduleCleanupPolicy,
    input::InputSystems,
    picking::pointer::{PointerId, PointerLocation},
    prelude::*,
    ui::{UiPlugin, UiSystems, ui_focus_system},
};

pub(super) fn install(app: &mut App) {
    if app.is_plugin_added::<UiPlugin>() {
        // The original system reads the physical Window cursor. Replace just that
        // system's view, leaving the rest of the UI and native window backend intact.
        app.world_mut()
            .schedule_scope(PreUpdate, |world, schedule| {
                schedule
                    .remove_systems_in_set(
                        ui_focus_system,
                        world,
                        ScheduleCleanupPolicy::RemoveSystemsOnly,
                    )
                    .expect("replace native UI focus system");
            });
        app.add_systems(
            PreUpdate,
            focus
                .in_set(UiSystems::Focus)
                .after(InputSystems)
                .after(bevy::picking::PickingSystems::ProcessInput),
        );
    }
}

fn focus(world: &mut World) {
    let location = world
        .query::<(&PointerId, &PointerLocation)>()
        .iter(world)
        .find(|(id, _)| **id == super::pointer::ID)
        .and_then(|(_, pointer)| pointer.location.clone());
    let mut positions = Vec::new();
    for (entity, mut window) in world.query::<(Entity, &mut Window)>().iter_mut(world) {
        positions.push((entity, window.internal));
        let virtual_position = location.as_ref().and_then(|location| {
            matches!(location.target, NormalizedRenderTarget::Window(target) if target.entity() == entity)
                .then_some(location.position)
        });
        window
            .bypass_change_detection()
            .set_cursor_position(virtual_position);
    }
    let result = world.run_system_cached(ui_focus_system);
    // Restore even on a system-parameter error, before any backend system can run.
    for (entity, internal) in positions {
        if let Some(mut window) = world.get_mut::<Window>(entity) {
            window.bypass_change_detection().internal = internal;
        }
    }
    result.expect("virtual UI focus system");
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{input::touch::Touches, window::WindowRef};

    #[test]
    fn ui_focus_restores_native_cursor_state_exactly() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Touches>()
            .init_resource::<bevy::ui::UiStack>();
        let mut window = Window::default();
        // Preserve full physical precision, including positions outside the window.
        window.set_physical_cursor_position(Some(bevy::math::DVec2::new(9000.123456789, -2.)));
        let before = window.internal;
        let entity = app.world_mut().spawn(window).id();
        app.world_mut().spawn((
            super::super::pointer::ID,
            PointerLocation::new(bevy::picking::pointer::Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Entity(entity).normalize(None).unwrap(),
                ),
                position: Vec2::new(12., 17.),
            }),
        ));
        let changed = app
            .world()
            .entity(entity)
            .get_change_ticks::<Window>()
            .unwrap()
            .changed;
        focus(app.world_mut());
        assert_eq!(app.world().get::<Window>(entity).unwrap().internal, before);
        assert_eq!(
            app.world()
                .entity(entity)
                .get_change_ticks::<Window>()
                .unwrap()
                .changed,
            changed
        );
    }
}
