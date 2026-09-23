//! Terminal-only line editing. Piped input is read without a permanently blocked stdin thread.
use crossterm::{
    cursor::MoveToColumn,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType},
};
use std::{
    io::{self, IsTerminal, Write},
    time::Duration,
};
use unicode_width::UnicodeWidthChar;

pub(super) enum Input {
    Line(String),
    Closed,
    Interrupted,
    Invalid(String),
}
pub(super) struct Terminal {
    tty: bool,
    line: String,
    position: usize,
    bytes: Vec<u8>,
    eof: bool,
}
impl Terminal {
    pub fn new() -> io::Result<Self> {
        let tty = io::stdin().is_terminal() && io::stdout().is_terminal();
        if tty {
            terminal::enable_raw_mode()?;
        }
        Ok(Self {
            tty,
            line: String::new(),
            position: 0,
            bytes: vec![],
            eof: false,
        })
    }
    pub fn print(&mut self, text: &str) -> io::Result<()> {
        let mut out = io::stdout().lock();
        if self.tty {
            execute!(out, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        }
        // Data from the game is not allowed to issue terminal control sequences.
        for line in text.lines() {
            let mut safe = String::with_capacity(line.len());
            for ch in line.chars() {
                if ch.is_control() {
                    safe.extend(ch.escape_default());
                } else {
                    safe.push(ch);
                }
            }
            write!(out, "{safe}{}", if self.tty { "\r\n" } else { "\n" })?;
        }
        out.flush()?;
        drop(out);
        self.redraw()
    }
    pub fn redraw(&self) -> io::Result<()> {
        if !self.tty {
            return Ok(());
        }
        let width = terminal::size()?.0.saturating_sub(3) as usize;
        let mut start = self.position;
        let mut used = 0;
        for (index, ch) in self.line[..self.position].char_indices().rev() {
            let cells = ch.width().unwrap_or(0);
            if used + cells > width {
                break;
            }
            used += cells;
            start = index;
        }
        let mut visible = self.line[start..self.position].to_owned();
        let mut total = used;
        for ch in self.line[self.position..].chars() {
            let cells = ch.width().unwrap_or(0);
            if total + cells > width {
                break;
            }
            visible.push(ch);
            total += cells;
        }
        let mut out = io::stdout().lock();
        execute!(out, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        write!(out, "> {visible}")?;
        execute!(out, MoveToColumn((used + 2) as u16))?;
        out.flush()
    }
    pub fn read(&mut self) -> io::Result<Option<Input>> {
        if !self.tty {
            return self.read_pipe();
        }
        if !event::poll(Duration::ZERO)? {
            return Ok(None);
        }
        let Event::Key(key) = event::read()? else {
            self.redraw()?;
            return Ok(None);
        };
        if key.kind == KeyEventKind::Release {
            return Ok(None);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => return Ok(Some(Input::Interrupted)),
                KeyCode::Char('d') if self.line.is_empty() => return Ok(Some(Input::Closed)),
                KeyCode::Char('u') => {
                    self.line.clear();
                    self.position = 0;
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Enter => {
                    let line = std::mem::take(&mut self.line);
                    self.position = 0;
                    self.print(&format!("> {line}"))?;
                    return Ok(Some(Input::Line(line)));
                }
                KeyCode::Char(ch) if !ch.is_control() => {
                    self.line.insert(self.position, ch);
                    self.position += ch.len_utf8();
                }
                KeyCode::Backspace if self.position > 0 => {
                    let index = self.line[..self.position]
                        .char_indices()
                        .next_back()
                        .unwrap()
                        .0;
                    self.line.drain(index..self.position);
                    self.position = index;
                }
                KeyCode::Delete if self.position < self.line.len() => {
                    self.line.remove(self.position);
                }
                KeyCode::Left if self.position > 0 => {
                    self.position = self.line[..self.position]
                        .char_indices()
                        .next_back()
                        .unwrap()
                        .0;
                }
                KeyCode::Right if self.position < self.line.len() => {
                    self.position += self.line[self.position..]
                        .chars()
                        .next()
                        .unwrap()
                        .len_utf8();
                }
                KeyCode::Home => self.position = 0,
                KeyCode::End => self.position = self.line.len(),
                _ => {}
            }
        }
        self.redraw()?;
        Ok(None)
    }
    fn read_pipe(&mut self) -> io::Result<Option<Input>> {
        if let Some(end) = self.bytes.iter().position(|b| *b == b'\n') {
            let bytes: Vec<_> = self.bytes.drain(..=end).collect();
            return Ok(Some(decode(bytes)));
        }
        if self.eof {
            return Ok(Some(if self.bytes.is_empty() {
                Input::Closed
            } else {
                decode(std::mem::take(&mut self.bytes))
            }));
        }
        let mut fd = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        };
        // poll/read borrow valid stack buffers. No other REPL code reads stdin.
        let ready = unsafe { libc::poll(&mut fd, 1, 0) };
        if ready < 0 {
            return Err(io::Error::last_os_error());
        }
        if ready == 0 {
            return Ok(None);
        }
        let mut bytes = [0u8; 4096];
        let count =
            unsafe { libc::read(libc::STDIN_FILENO, bytes.as_mut_ptr().cast(), bytes.len()) };
        if count < 0 {
            return Err(io::Error::last_os_error());
        }
        self.eof = count == 0;
        self.bytes.extend_from_slice(&bytes[..count as usize]);
        Ok(None)
    }
}
fn decode(bytes: Vec<u8>) -> Input {
    match String::from_utf8(bytes) {
        Ok(line) => Input::Line(line),
        Err(e) => Input::Invalid(format!("invalid UTF-8 input: {e}")),
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        if self.tty {
            let _ = execute!(io::stdout(), MoveToColumn(0), Clear(ClearType::CurrentLine));
            let _ = terminal::disable_raw_mode();
        }
    }
}
