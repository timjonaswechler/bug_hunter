use bevy::{prelude::*, window::PrimaryWindow};

pub(super) fn primary(world: &World) -> Option<(Entity, &Window)> {
    let mut windows = world.iter_entities().filter_map(|entity| {
        entity
            .contains::<PrimaryWindow>()
            .then(|| entity.get::<Window>().map(|window| (entity.id(), window)))
            .flatten()
    });
    let first = windows.next()?;
    windows.next().is_none().then_some(first)
}
