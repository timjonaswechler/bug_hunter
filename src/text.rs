//! Focused text Virtual Input through Bevy IME commit events.
//!
//! Commands are UTF-8-size validated. Event construction additionally requires one primary window,
//! a live focused entity, and Bevy [`EditableText`] on that entity.

use crate::entity::Handle;
use bevy::{
    input_focus::InputFocus,
    prelude::*,
    text::EditableText,
    window::{Ime, PrimaryWindow, Window},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt;

/// Maximum UTF-8 payload accepted by one text command.
pub const MAX_BYTES: usize = 16 * 1024;

/// A request to commit text to the UI entity that currently owns Bevy input focus.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    /// UTF-8 text requested for commit.
    pub text: String,
}

impl Command {
    /// Creates a text-commit request.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    /// Checks only the [`MAX_BYTES`] UTF-8 byte limit.
    ///
    /// Window and focus requirements are checked by [`text_event`].
    pub fn validate(&self) -> Result<(), Error> {
        if self.text.len() > MAX_BYTES {
            return Err(Error::TooLarge(self.text.len()));
        }
        Ok(())
    }
}

/// Session-local record of the last virtual text commit.
#[derive(Clone, Debug, Default, Resource)]
pub struct State {
    last_target: Option<Handle>,
    last_text: Option<String>,
    commits: u64,
}

impl State {
    /// Returns focused handle, last requested target/text, and successful bridge-call count.
    pub fn observation(&self, world: &World) -> serde_json::Value {
        let focused = world
            .get_resource::<InputFocus>()
            .and_then(InputFocus::get)
            .filter(|entity| world.get_entity(*entity).is_ok())
            .map(Handle::from);
        json!({
            "focused": focused,
            "last_target": self.last_target,
            "last_text": self.last_text,
            "commits": self.commits,
        })
    }
}

/// Text validation or event-construction failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// Payload exceeds [`MAX_BYTES`]; contains the actual UTF-8 byte length.
    TooLarge(usize),
    /// No primary window exists.
    NoPrimaryWindow,
    /// More than one primary window exists.
    AmbiguousPrimaryWindow,
    /// Bevy's input-focus resource is not installed.
    NoFocusState,
    /// No entity currently owns input focus.
    NoFocusedEntity,
    /// The focused handle no longer resolves.
    FocusNotLive(Handle),
    /// The focused entity does not contain [`EditableText`].
    FocusNotEditable(Handle),
}

impl Error {
    /// Returns the stable protocol error code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::TooLarge(_) => "text_too_large",
            Self::NoPrimaryWindow | Self::AmbiguousPrimaryWindow => "text_window_unavailable",
            Self::NoFocusState
            | Self::NoFocusedEntity
            | Self::FocusNotLive(_)
            | Self::FocusNotEditable(_) => "text_focus_unavailable",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge(length) => write!(
                formatter,
                "text payload must be at most {MAX_BYTES} UTF-8 bytes, got {length}"
            ),
            Self::NoPrimaryWindow => formatter.write_str("no primary window is available"),
            Self::AmbiguousPrimaryWindow => {
                formatter.write_str("more than one primary window is available")
            }
            Self::NoFocusState => formatter.write_str("Bevy input focus is not installed"),
            Self::NoFocusedEntity => formatter.write_str("no entity owns Bevy input focus"),
            Self::FocusNotLive(handle) => {
                write!(formatter, "focused entity handle {handle} is not live")
            }
            Self::FocusNotEditable(handle) => {
                write!(
                    formatter,
                    "focused entity handle {handle} is not editable text"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

/// Records a valid focused commit and constructs Bevy's [`Ime::Commit`] event.
///
/// State is updated before the returned event is dispatched; success therefore records bridge
/// construction, not proof that application systems consumed or applied the text.
pub fn text_event(state: &mut State, world: &World, command: &Command) -> Result<Ime, Error> {
    command.validate()?;
    let window = primary_window(world)?;
    let focused = world
        .get_resource::<InputFocus>()
        .ok_or(Error::NoFocusState)?
        .get()
        .ok_or(Error::NoFocusedEntity)?;
    let focused_ref = world
        .get_entity(focused)
        .map_err(|_| Error::FocusNotLive(Handle::from(focused)))?;
    if !focused_ref.contains::<EditableText>() {
        return Err(Error::FocusNotEditable(Handle::from(focused)));
    }
    state.last_target = Some(Handle::from(focused));
    state.last_text = Some(command.text.clone());
    state.commits = state.commits.saturating_add(1);
    Ok(Ime::Commit {
        window,
        value: command.text.clone(),
    })
}

fn primary_window(world: &World) -> Result<Entity, Error> {
    let mut windows = world
        .iter_entities()
        .filter(|entity| entity.contains::<PrimaryWindow>() && entity.contains::<Window>())
        .map(|entity| entity.id());
    let Some(window) = windows.next() else {
        return Err(Error::NoPrimaryWindow);
    };
    if windows.next().is_some() {
        return Err(Error::AmbiguousPrimaryWindow);
    }
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_size_and_focus_failures_are_typed() {
        let oversized = Command::new("x".repeat(MAX_BYTES + 1));
        assert_eq!(oversized.validate(), Err(Error::TooLarge(MAX_BYTES + 1)));

        let mut world = World::new();
        world.spawn((Window::default(), PrimaryWindow));
        let mut state = State::default();
        assert_eq!(
            text_event(&mut state, &world, &Command::new("hello")),
            Err(Error::NoFocusState)
        );

        let target = world.spawn_empty().id();
        world.insert_resource(InputFocus::from_entity(target));
        assert_eq!(
            text_event(&mut state, &world, &Command::new("hello")),
            Err(Error::FocusNotEditable(Handle::from(target)))
        );
    }

    #[test]
    fn text_commit_records_the_current_session_focus() {
        let mut world = World::new();
        let window = world.spawn((Window::default(), PrimaryWindow)).id();
        let target = world.spawn(EditableText::default()).id();
        world.insert_resource(InputFocus::from_entity(target));
        let mut state = State::default();

        assert_eq!(
            text_event(&mut state, &world, &Command::new("hello")).unwrap(),
            Ime::Commit {
                window,
                value: "hello".into()
            }
        );
        let observation = state.observation(&world);
        assert_eq!(observation["focused"], json!(Handle::from(target)));
        assert_eq!(observation["last_text"], "hello");
        assert_eq!(observation["commits"], 1);
    }
}
