use bevy::input::keyboard::{Key as LogicalKey, KeyCode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Press {
    pub key: Key,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub key: Key,
}

crate::command::request!(Press, KeyboardPress, ());
crate::command::request!(Release, KeyboardRelease, ());

macro_rules! keys {
    ($($variant:ident => ($token:literal, $code:ident, $logical:expr)),+ $(,)?) => {
        /// Layout-independent physical keys with fixed logical mappings.
        /// Keyboard events carry no text; text entry is a separate command.
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum Key {
            $(#[doc = concat!("Wire token `", $token, "`.")]
              $variant,)+
            /// Unsupported tokens are retained so the game can reject them as `invalid_key`.
            Unknown(String),
        }

        impl Key {
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $token,)+
                    Self::Unknown(token) => token,
                }
            }

            pub(crate) fn resolve(&self) -> Option<(KeyCode, LogicalKey)> {
                // Resolve by wire token as well for manually constructed Unknown values.
                match self.as_str() {
                    $($token => Some((KeyCode::$code, $logical)),)+
                    _ => None,
                }
            }
        }

        impl<'de> Deserialize<'de> for Key {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let token = String::deserialize(deserializer)?;
                Ok(match token.as_str() {
                    $($token => Self::$variant,)+
                    _ => Self::Unknown(token),
                })
            }
        }

        #[cfg(test)]
        #[test]
        fn all_tokens_roundtrip_with_distinct_physical_codes() {
            let keys = [$(Key::$variant,)+];
            let mut codes = std::collections::HashSet::new();
            for key in keys {
                let json = serde_json::to_string(&key).unwrap();
                assert_eq!(serde_json::from_str::<Key>(&json).unwrap(), key);
                assert!(codes.insert(key.resolve().unwrap().0));
            }
            assert_eq!(Key::A.resolve(), Some((KeyCode::KeyA, LogicalKey::Character("a".into()))));
            assert_eq!(Key::ShiftLeft.resolve().unwrap().1, LogicalKey::Shift);
            assert_eq!(Key::ShiftRight.resolve().unwrap().1, LogicalKey::Shift);
            let invalid: Key = serde_json::from_str("\"KeyA\"").unwrap();
            assert!(invalid.resolve().is_none());
            assert_eq!(serde_json::to_string(&invalid).unwrap(), "\"KeyA\"");
        }
    };
}

impl Serialize for Key {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

keys! {
    A => ("a", KeyA, LogicalKey::Character("a".into())),
    B => ("b", KeyB, LogicalKey::Character("b".into())),
    C => ("c", KeyC, LogicalKey::Character("c".into())),
    D => ("d", KeyD, LogicalKey::Character("d".into())),
    E => ("e", KeyE, LogicalKey::Character("e".into())),
    F => ("f", KeyF, LogicalKey::Character("f".into())),
    G => ("g", KeyG, LogicalKey::Character("g".into())),
    H => ("h", KeyH, LogicalKey::Character("h".into())),
    I => ("i", KeyI, LogicalKey::Character("i".into())),
    J => ("j", KeyJ, LogicalKey::Character("j".into())),
    K => ("k", KeyK, LogicalKey::Character("k".into())),
    L => ("l", KeyL, LogicalKey::Character("l".into())),
    M => ("m", KeyM, LogicalKey::Character("m".into())),
    N => ("n", KeyN, LogicalKey::Character("n".into())),
    O => ("o", KeyO, LogicalKey::Character("o".into())),
    P => ("p", KeyP, LogicalKey::Character("p".into())),
    Q => ("q", KeyQ, LogicalKey::Character("q".into())),
    R => ("r", KeyR, LogicalKey::Character("r".into())),
    S => ("s", KeyS, LogicalKey::Character("s".into())),
    T => ("t", KeyT, LogicalKey::Character("t".into())),
    U => ("u", KeyU, LogicalKey::Character("u".into())),
    V => ("v", KeyV, LogicalKey::Character("v".into())),
    W => ("w", KeyW, LogicalKey::Character("w".into())),
    X => ("x", KeyX, LogicalKey::Character("x".into())),
    Y => ("y", KeyY, LogicalKey::Character("y".into())),
    Z => ("z", KeyZ, LogicalKey::Character("z".into())),
    Digit0 => ("digit_0", Digit0, LogicalKey::Character("0".into())),
    Digit1 => ("digit_1", Digit1, LogicalKey::Character("1".into())),
    Digit2 => ("digit_2", Digit2, LogicalKey::Character("2".into())),
    Digit3 => ("digit_3", Digit3, LogicalKey::Character("3".into())),
    Digit4 => ("digit_4", Digit4, LogicalKey::Character("4".into())),
    Digit5 => ("digit_5", Digit5, LogicalKey::Character("5".into())),
    Digit6 => ("digit_6", Digit6, LogicalKey::Character("6".into())),
    Digit7 => ("digit_7", Digit7, LogicalKey::Character("7".into())),
    Digit8 => ("digit_8", Digit8, LogicalKey::Character("8".into())),
    Digit9 => ("digit_9", Digit9, LogicalKey::Character("9".into())),
    Backquote => ("backquote", Backquote, LogicalKey::Character("`".into())),
    Backslash => ("backslash", Backslash, LogicalKey::Character("\\".into())),
    BracketLeft => ("bracket_left", BracketLeft, LogicalKey::Character("[".into())),
    BracketRight => ("bracket_right", BracketRight, LogicalKey::Character("]".into())),
    Comma => ("comma", Comma, LogicalKey::Character(",".into())),
    Equal => ("equal", Equal, LogicalKey::Character("=".into())),
    Minus => ("minus", Minus, LogicalKey::Character("-".into())),
    Period => ("period", Period, LogicalKey::Character(".".into())),
    Quote => ("quote", Quote, LogicalKey::Character("'".into())),
    Semicolon => ("semicolon", Semicolon, LogicalKey::Character(";".into())),
    Slash => ("slash", Slash, LogicalKey::Character("/".into())),
    AltLeft => ("alt_left", AltLeft, LogicalKey::Alt),
    AltRight => ("alt_right", AltRight, LogicalKey::Alt),
    ControlLeft => ("control_left", ControlLeft, LogicalKey::Control),
    ControlRight => ("control_right", ControlRight, LogicalKey::Control),
    ShiftLeft => ("shift_left", ShiftLeft, LogicalKey::Shift),
    ShiftRight => ("shift_right", ShiftRight, LogicalKey::Shift),
    SuperLeft => ("super_left", SuperLeft, LogicalKey::Super),
    SuperRight => ("super_right", SuperRight, LogicalKey::Super),
    Backspace => ("backspace", Backspace, LogicalKey::Backspace),
    CapsLock => ("caps_lock", CapsLock, LogicalKey::CapsLock),
    ContextMenu => ("context_menu", ContextMenu, LogicalKey::ContextMenu),
    Enter => ("enter", Enter, LogicalKey::Enter),
    Space => ("space", Space, LogicalKey::Space),
    Tab => ("tab", Tab, LogicalKey::Tab),
    Delete => ("delete", Delete, LogicalKey::Delete),
    End => ("end", End, LogicalKey::End),
    Home => ("home", Home, LogicalKey::Home),
    Insert => ("insert", Insert, LogicalKey::Insert),
    PageDown => ("page_down", PageDown, LogicalKey::PageDown),
    PageUp => ("page_up", PageUp, LogicalKey::PageUp),
    ArrowDown => ("arrow_down", ArrowDown, LogicalKey::ArrowDown),
    ArrowLeft => ("arrow_left", ArrowLeft, LogicalKey::ArrowLeft),
    ArrowRight => ("arrow_right", ArrowRight, LogicalKey::ArrowRight),
    ArrowUp => ("arrow_up", ArrowUp, LogicalKey::ArrowUp),
    Escape => ("escape", Escape, LogicalKey::Escape),
    F1 => ("f1", F1, LogicalKey::F1),
    F2 => ("f2", F2, LogicalKey::F2),
    F3 => ("f3", F3, LogicalKey::F3),
    F4 => ("f4", F4, LogicalKey::F4),
    F5 => ("f5", F5, LogicalKey::F5),
    F6 => ("f6", F6, LogicalKey::F6),
    F7 => ("f7", F7, LogicalKey::F7),
    F8 => ("f8", F8, LogicalKey::F8),
    F9 => ("f9", F9, LogicalKey::F9),
    F10 => ("f10", F10, LogicalKey::F10),
    F11 => ("f11", F11, LogicalKey::F11),
    F12 => ("f12", F12, LogicalKey::F12),
}
