//! Virtual keyboard tokens, held-key state, and the Bevy event bridge.
//!
//! Wire tokens are stable and layout-independent. Validation checks token support; event creation
//! additionally requires exactly one primary window and a valid press/release state transition.

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key as BevyKey, KeyCode, KeyboardInput},
    },
    prelude::*,
    window::{PrimaryWindow, Window},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use std::{collections::BTreeSet, fmt};

macro_rules! define_keys {
    ($( $variant:ident => ($wire:literal, $code:ident, $logical:expr) ),+ $(,)?) => {
        /// A stable, layout-independent key name supported by Virtual Input.
        ///
        /// Deserialization preserves unsupported tokens as [`Key::Unknown`], allowing validation
        /// to return [`Error::InvalidKey`] instead of rejecting JSON structurally.
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum Key {
            $(
                #[doc = concat!("Stable Virtual Input wire token `", $wire, "`.")]
                $variant,
            )+
            /// Preserves an unknown wire value so validation can return a typed error.
            Unknown(String),
        }

        impl Key {
            /// Returns the stable wire token, including an unsupported token in [`Key::Unknown`].
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $wire, )+
                    Self::Unknown(value) => value,
                }
            }

            fn from_name(value: String) -> Self {
                match value.as_str() {
                    $( $wire => Self::$variant, )+
                    _ => Self::Unknown(value),
                }
            }

            fn resolve(&self) -> Result<(KeyCode, BevyKey), Error> {
                match self {
                    $( Self::$variant => Ok((KeyCode::$code, $logical)), )+
                    Self::Unknown(value) => Err(Error::InvalidKey(value.clone())),
                }
            }
        }
    };
}

define_keys! {
    A => ("a", KeyA, BevyKey::Character("a".into())),
    B => ("b", KeyB, BevyKey::Character("b".into())),
    C => ("c", KeyC, BevyKey::Character("c".into())),
    D => ("d", KeyD, BevyKey::Character("d".into())),
    E => ("e", KeyE, BevyKey::Character("e".into())),
    F => ("f", KeyF, BevyKey::Character("f".into())),
    G => ("g", KeyG, BevyKey::Character("g".into())),
    H => ("h", KeyH, BevyKey::Character("h".into())),
    I => ("i", KeyI, BevyKey::Character("i".into())),
    J => ("j", KeyJ, BevyKey::Character("j".into())),
    K => ("k", KeyK, BevyKey::Character("k".into())),
    L => ("l", KeyL, BevyKey::Character("l".into())),
    M => ("m", KeyM, BevyKey::Character("m".into())),
    N => ("n", KeyN, BevyKey::Character("n".into())),
    O => ("o", KeyO, BevyKey::Character("o".into())),
    P => ("p", KeyP, BevyKey::Character("p".into())),
    Q => ("q", KeyQ, BevyKey::Character("q".into())),
    R => ("r", KeyR, BevyKey::Character("r".into())),
    S => ("s", KeyS, BevyKey::Character("s".into())),
    T => ("t", KeyT, BevyKey::Character("t".into())),
    U => ("u", KeyU, BevyKey::Character("u".into())),
    V => ("v", KeyV, BevyKey::Character("v".into())),
    W => ("w", KeyW, BevyKey::Character("w".into())),
    X => ("x", KeyX, BevyKey::Character("x".into())),
    Y => ("y", KeyY, BevyKey::Character("y".into())),
    Z => ("z", KeyZ, BevyKey::Character("z".into())),
    Digit0 => ("digit_0", Digit0, BevyKey::Character("0".into())),
    Digit1 => ("digit_1", Digit1, BevyKey::Character("1".into())),
    Digit2 => ("digit_2", Digit2, BevyKey::Character("2".into())),
    Digit3 => ("digit_3", Digit3, BevyKey::Character("3".into())),
    Digit4 => ("digit_4", Digit4, BevyKey::Character("4".into())),
    Digit5 => ("digit_5", Digit5, BevyKey::Character("5".into())),
    Digit6 => ("digit_6", Digit6, BevyKey::Character("6".into())),
    Digit7 => ("digit_7", Digit7, BevyKey::Character("7".into())),
    Digit8 => ("digit_8", Digit8, BevyKey::Character("8".into())),
    Digit9 => ("digit_9", Digit9, BevyKey::Character("9".into())),
    Backquote => ("backquote", Backquote, BevyKey::Character("`".into())),
    Backslash => ("backslash", Backslash, BevyKey::Character("\\".into())),
    BracketLeft => ("bracket_left", BracketLeft, BevyKey::Character("[".into())),
    BracketRight => ("bracket_right", BracketRight, BevyKey::Character("]".into())),
    Comma => ("comma", Comma, BevyKey::Character(",".into())),
    Equal => ("equal", Equal, BevyKey::Character("=".into())),
    Minus => ("minus", Minus, BevyKey::Character("-".into())),
    Period => ("period", Period, BevyKey::Character(".".into())),
    Quote => ("quote", Quote, BevyKey::Character("'".into())),
    Semicolon => ("semicolon", Semicolon, BevyKey::Character(";".into())),
    Slash => ("slash", Slash, BevyKey::Character("/".into())),
    AltLeft => ("alt_left", AltLeft, BevyKey::Alt),
    AltRight => ("alt_right", AltRight, BevyKey::Alt),
    ControlLeft => ("control_left", ControlLeft, BevyKey::Control),
    ControlRight => ("control_right", ControlRight, BevyKey::Control),
    ShiftLeft => ("shift_left", ShiftLeft, BevyKey::Shift),
    ShiftRight => ("shift_right", ShiftRight, BevyKey::Shift),
    SuperLeft => ("super_left", SuperLeft, BevyKey::Super),
    SuperRight => ("super_right", SuperRight, BevyKey::Super),
    Backspace => ("backspace", Backspace, BevyKey::Backspace),
    CapsLock => ("caps_lock", CapsLock, BevyKey::CapsLock),
    ContextMenu => ("context_menu", ContextMenu, BevyKey::ContextMenu),
    Enter => ("enter", Enter, BevyKey::Enter),
    Space => ("space", Space, BevyKey::Space),
    Tab => ("tab", Tab, BevyKey::Tab),
    Delete => ("delete", Delete, BevyKey::Delete),
    End => ("end", End, BevyKey::End),
    Home => ("home", Home, BevyKey::Home),
    Insert => ("insert", Insert, BevyKey::Insert),
    PageDown => ("page_down", PageDown, BevyKey::PageDown),
    PageUp => ("page_up", PageUp, BevyKey::PageUp),
    ArrowDown => ("arrow_down", ArrowDown, BevyKey::ArrowDown),
    ArrowLeft => ("arrow_left", ArrowLeft, BevyKey::ArrowLeft),
    ArrowRight => ("arrow_right", ArrowRight, BevyKey::ArrowRight),
    ArrowUp => ("arrow_up", ArrowUp, BevyKey::ArrowUp),
    Escape => ("escape", Escape, BevyKey::Escape),
    F1 => ("f1", F1, BevyKey::F1),
    F2 => ("f2", F2, BevyKey::F2),
    F3 => ("f3", F3, BevyKey::F3),
    F4 => ("f4", F4, BevyKey::F4),
    F5 => ("f5", F5, BevyKey::F5),
    F6 => ("f6", F6, BevyKey::F6),
    F7 => ("f7", F7, BevyKey::F7),
    F8 => ("f8", F8, BevyKey::F8),
    F9 => ("f9", F9, BevyKey::F9),
    F10 => ("f10", F10, BevyKey::F10),
    F11 => ("f11", F11, BevyKey::F11),
    F12 => ("f12", F12, BevyKey::F12),
}

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::from_name(String::deserialize(deserializer)?))
    }
}

/// Virtual keyboard transitions accepted by a Controlled Session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Presses a supported key that is not already held.
    Press {
        /// Stable key token to press.
        key: Key,
    },
    /// Releases a supported key that is currently held.
    Release {
        /// Stable key token to release.
        key: Key,
    },
}

impl Command {
    /// Checks only that the key token is supported.
    ///
    /// Window availability and duplicate state transitions are checked by [`keyboard_event`].
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Press { key } | Self::Release { key } => key.resolve().map(|_| ()),
        }
    }
}

/// Session-local held-key state.
#[derive(Clone, Debug, Default, Resource)]
pub struct State {
    pressed: BTreeSet<Key>,
}

impl State {
    /// Returns whether `key` is currently held by Virtual Input in this session.
    pub fn is_pressed(&self, key: &Key) -> bool {
        self.pressed.contains(key)
    }

    /// Returns `{ "pressed": [...] }` with stable wire tokens in deterministic [`Key`] enum
    /// order (the declaration order), not lexicographic wire-token order.
    pub fn observation(&self) -> serde_json::Value {
        json!({
            "pressed": self.pressed.iter().map(Key::as_str).collect::<Vec<_>>(),
        })
    }
}

/// Keyboard validation or event-construction failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The deserialized wire token is not supported.
    InvalidKey(String),
    /// No entity is both a Bevy window and primary window.
    NoPrimaryWindow,
    /// More than one primary window is available.
    AmbiguousPrimaryWindow,
    /// A press was requested for an already-held key.
    KeyAlreadyPressed(Key),
    /// A release was requested for a key that is not held.
    KeyNotPressed(Key),
}

impl Error {
    /// Returns the stable protocol error code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidKey(_) => "invalid_key",
            Self::NoPrimaryWindow | Self::AmbiguousPrimaryWindow => "keyboard_window_unavailable",
            Self::KeyAlreadyPressed(_) => "key_already_pressed",
            Self::KeyNotPressed(_) => "key_not_pressed",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey(key) => write!(formatter, "unsupported keyboard key {key:?}"),
            Self::NoPrimaryWindow => formatter.write_str("no primary window is available"),
            Self::AmbiguousPrimaryWindow => {
                formatter.write_str("more than one primary window is available")
            }
            Self::KeyAlreadyPressed(key) => {
                write!(
                    formatter,
                    "keyboard key {:?} is already pressed",
                    key.as_str()
                )
            }
            Self::KeyNotPressed(key) => {
                write!(formatter, "keyboard key {:?} is not pressed", key.as_str())
            }
        }
    }
}

impl std::error::Error for Error {}

/// Applies a virtual key transition and constructs the corresponding Bevy [`KeyboardInput`].
///
/// A successful call updates session-local [`State`], but does not itself dispatch the returned
/// event or guarantee that application systems consume it. Press before release:
///
/// ```
/// use automation_control::keyboard::{Command, Key};
/// let commands = [Command::Press { key: Key::A }, Command::Release { key: Key::A }];
/// assert!(commands.iter().all(|command| command.validate().is_ok()));
/// ```
pub fn keyboard_event(
    state: &mut State,
    world: &World,
    command: &Command,
) -> Result<KeyboardInput, Error> {
    command.validate()?;
    let window = primary_window(world)?;
    let (key, button_state) = match command {
        Command::Press { key } => {
            if !state.pressed.insert(key.clone()) {
                return Err(Error::KeyAlreadyPressed(key.clone()));
            }
            (key, ButtonState::Pressed)
        }
        Command::Release { key } => {
            if !state.pressed.remove(key) {
                return Err(Error::KeyNotPressed(key.clone()));
            }
            (key, ButtonState::Released)
        }
    };
    let (key_code, logical_key) = key.resolve()?;
    Ok(KeyboardInput {
        key_code,
        logical_key,
        state: button_state,
        text: None,
        repeat: false,
        window,
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
    fn wire_keys_roundtrip_and_unknown_keys_validate_as_typed_errors() {
        let command = Command::Press { key: Key::A };
        assert_eq!(
            serde_json::to_value(&command).unwrap(),
            json!({"type": "press", "key": "a"})
        );
        let unknown: Command = serde_json::from_value(json!({
            "type": "press",
            "key": "hyperdrive"
        }))
        .unwrap();
        assert_eq!(
            unknown.validate().unwrap_err(),
            Error::InvalidKey("hyperdrive".into())
        );
    }

    #[test]
    fn press_and_release_are_distinct_typed_state_transitions() {
        let mut world = World::new();
        world.spawn((Window::default(), PrimaryWindow));
        let mut state = State::default();
        let press = Command::Press { key: Key::Escape };
        let release = Command::Release { key: Key::Escape };

        assert!(
            keyboard_event(&mut state, &world, &press)
                .unwrap()
                .state
                .is_pressed()
        );
        assert!(state.is_pressed(&Key::Escape));
        assert_eq!(
            keyboard_event(&mut state, &world, &press).unwrap_err(),
            Error::KeyAlreadyPressed(Key::Escape)
        );
        assert_eq!(
            keyboard_event(&mut state, &world, &release).unwrap().state,
            ButtonState::Released
        );
        assert!(!state.is_pressed(&Key::Escape));
        assert_eq!(
            keyboard_event(&mut state, &world, &release).unwrap_err(),
            Error::KeyNotPressed(Key::Escape)
        );
    }
}
