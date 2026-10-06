use crate::{command::input::text::Input, session::protocol::Diagnostic};
use bevy::prelude::*;
#[cfg(feature = "ui")]
use bevy::{
    input_focus::InputFocus,
    text::{EditableText, TextEdit},
};

const MAX_BYTES: usize = 16_384;

#[derive(Default)]
pub(crate) struct State {
    #[cfg(feature = "ui")]
    pending: Vec<(Entity, String)>,
}

impl State {
    pub(crate) fn input(&mut self, world: &World, input: Input) -> Result<(), Diagnostic> {
        if input.text.len() > MAX_BYTES {
            return Err(Diagnostic::new(
                "text_too_large",
                "text exceeds 16384 UTF-8 bytes",
            ));
        }
        super::primary_window(world).ok_or_else(|| {
            Diagnostic::new(
                "text_window_unavailable",
                "expected exactly one primary window",
            )
        })?;
        #[cfg(feature = "ui")]
        {
            let entity = world
                .get_resource::<InputFocus>()
                .and_then(InputFocus::get)
                .filter(|entity| world.get::<EditableText>(*entity).is_some())
                .ok_or_else(|| {
                    Diagnostic::new(
                        "text_focus_unavailable",
                        "expected a live focused EditableText entity",
                    )
                })?;
            // Pin the validated target. Another queued input may change focus during
            // the tick; it must not redirect this text into a different field.
            self.pending.push((entity, input.text));
            Ok(())
        }
        #[cfg(not(feature = "ui"))]
        Err(Diagnostic::new(
            "text_focus_unavailable",
            "text input requires woodpecker/ui",
        ))
    }

    pub(crate) fn flush(&mut self, _world: &mut World) {
        #[cfg(feature = "ui")]
        for (entity, text) in self.pending.drain(..) {
            if let Some(mut editable) = _world.get_mut::<EditableText>(entity) {
                // Queue the same edit as Bevy's IME commit handler, without touching
                // the OS IME or bypassing Bevy's text filtering and selection logic.
                editable.queue_edit(TextEdit::ImeCommit { value: text.into() });
            }
        }
    }
}

#[cfg(test)]
mod tests;
