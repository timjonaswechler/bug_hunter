//! These rules belong to the persisted v1 signature contract.
use super::{Failure, Origin};

pub(super) fn kind(failure: &Failure) -> &'static str {
    match failure.origin() {
        Origin::Panic { .. } => "panic",
        Origin::TracingError { .. } => "tracing_error",
        Origin::ProcessExit { .. } => "process_exit",
    }
}

pub(super) fn lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(super) fn title(failure: &Failure) -> String {
    let clean = clean(failure.message().unwrap_or_default());
    let line = clean
        .split('\n')
        .map(|line| line.trim_matches(|c: char| c.is_ascii_whitespace()))
        .find(|line| !line.is_empty())
        .unwrap_or(match failure.origin() {
            Origin::Panic { .. } => "panic",
            Origin::TracingError { .. } => "tracing error",
            Origin::ProcessExit { .. } => "process exited unexpectedly",
        });
    if line.chars().count() > 120 {
        line.chars().take(117).chain("...".chars()).collect()
    } else {
        line.into()
    }
}

fn clean(text: &str) -> String {
    let text = lf(text);
    let mut result = String::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        if let Some(length) = ansi(&text[index..]) {
            index += length;
        } else {
            let c = text[index..].chars().next().unwrap();
            result.push(c);
            index += c.len_utf8();
        }
    }
    result
}

/// Complete ECMA-48 sequences, including control strings and non-color CSI.
/// Incomplete or malformed sequences remain diagnostic text.
fn ansi(text: &str) -> Option<usize> {
    let first = text.chars().next()?;
    let (start, command) = if first == '\x1b' {
        (2, *text.as_bytes().get(1)? as char)
    } else if ('\u{80}'..='\u{9f}').contains(&first) {
        (first.len_utf8(), first)
    } else {
        return None;
    };
    match command {
        '[' | '\u{9b}' => {
            let bytes = text.as_bytes();
            let mut i = start;
            while bytes.get(i).is_some_and(|b| (0x30..=0x3f).contains(b)) {
                i += 1;
            }
            while bytes.get(i).is_some_and(|b| (0x20..=0x2f).contains(b)) {
                i += 1;
            }
            bytes
                .get(i)
                .filter(|b| (0x40..=0x7e).contains(*b))
                .map(|_| i + 1)
        }
        ']' | 'P' | 'X' | '^' | '_' | '\u{9d}' | '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => {
            let osc = matches!(command, ']' | '\u{9d}');
            for (offset, c) in text[start..].char_indices() {
                if c == '\u{9c}' || (osc && c == '\x07') {
                    return Some(start + offset + c.len_utf8());
                }
                if c == '\x1b' && text[start + offset..].starts_with("\x1b\\") {
                    return Some(start + offset + 2);
                }
            }
            None
        }
        _ if first == '\x1b' => {
            let bytes = text.as_bytes();
            let mut i = 1;
            while bytes.get(i).is_some_and(|b| (0x20..=0x2f).contains(b)) {
                i += 1;
            }
            bytes
                .get(i)
                .filter(|b| (0x30..=0x7e).contains(*b))
                .map(|_| i + 1)
        }
        _ => Some(start),
    }
}

pub(super) fn message(text: &str, project: &std::path::Path) -> String {
    let mut text = clean(text);
    if let Some(root) = project.to_str().filter(|root| !root.is_empty()) {
        let slash = root.replace('\\', "/");
        let backslash = root.replace('/', "\\");
        text = text
            .replace(&slash, "<project>")
            .replace(&backslash, "<project>");
    }
    let text = replace_tokens(&text, address, "<address>");
    replace_tokens(&text, timestamp, "<timestamp>")
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .into()
}

fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn replace_tokens(text: &str, parse: fn(&[u8]) -> Option<usize>, replacement: &str) -> String {
    let bytes = text.as_bytes();
    let mut result = String::with_capacity(text.len());
    let mut copied = 0;
    let mut i = 0;
    while i < bytes.len() {
        if (i == 0 || !word(bytes[i - 1]))
            && let Some(length) = parse(&bytes[i..])
            && bytes.get(i + length).is_none_or(|b| !word(*b))
        {
            result.push_str(&text[copied..i]);
            result.push_str(replacement);
            i += length;
            copied = i;
        } else {
            i += 1;
        }
    }
    result.push_str(&text[copied..]);
    result
}

fn address(bytes: &[u8]) -> Option<usize> {
    if bytes.first() != Some(&b'0') || !matches!(bytes.get(1), Some(b'x' | b'X')) {
        return None;
    }
    let digits = bytes[2..]
        .iter()
        .take_while(|b| b.is_ascii_hexdigit())
        .count();
    (8..=16).contains(&digits).then_some(2 + digits)
}

fn number(bytes: &[u8], start: usize, length: usize) -> Option<u32> {
    bytes
        .get(start..start + length)?
        .iter()
        .try_fold(0, |n, b| {
            b.is_ascii_digit().then(|| n * 10 + u32::from(b - b'0'))
        })
}

fn date(bytes: &[u8]) -> Option<usize> {
    if bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return None;
    }
    let year = number(bytes, 0, 4)?;
    let month = number(bytes, 5, 2)?;
    let day = number(bytes, 8, 2)?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    (day >= 1 && day <= days).then_some(10)
}

fn time(bytes: &[u8]) -> Option<usize> {
    if bytes.get(2) != Some(&b':')
        || bytes.get(5) != Some(&b':')
        || number(bytes, 0, 2)? > 23
        || number(bytes, 3, 2)? > 59
        || number(bytes, 6, 2)? > 59
    {
        return None;
    }
    let mut end = 8;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let digits = bytes[end..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits == 0 {
            return None;
        }
        end += digits;
    }
    match bytes.get(end) {
        Some(b'Z') => end += 1,
        Some(b'+' | b'-') => {
            let hours = number(bytes, end + 1, 2)?;
            let colon = bytes.get(end + 3) == Some(&b':');
            let minutes = number(bytes, end + 3 + usize::from(colon), 2)?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            end += 5 + usize::from(colon);
        }
        _ => {}
    }
    Some(end)
}

fn timestamp(bytes: &[u8]) -> Option<usize> {
    if let Some(end) = date(bytes) {
        if matches!(bytes.get(end), Some(b'T' | b' '))
            && let Some(length) = time(&bytes[end + 1..])
        {
            return Some(end + 1 + length);
        }
        return Some(end);
    }
    time(bytes)
}
