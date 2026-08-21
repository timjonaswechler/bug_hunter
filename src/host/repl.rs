use super::controller::{
    Action, Button, ControllerError, ControllerSession, KeyboardAction, Observation, PointerAction,
    Status,
};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread,
    time::Duration,
};

pub(crate) const HELP: &str = "\
commands:
  click TARGET
  pointer move X Y                 normalized X/Y in [0, 1)
  pointer press|release|click BUTTON
  scroll DX DY
  key KEY press|release
  text TEXT
  observe targets|ui|pointers|input|clock
  pause | resume | step FRAMES
  record start [PATH] | record stop
  status | help | quit
buttons: left, right, middle
keys use case-insensitive names such as Escape, A, ArrowLeft";

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Command {
    Click(String),
    Action(Action),
    Observe(Observation),
    Pause,
    Resume,
    Step(u64),
    Recording(RecordingCommand),
    Status,
    Help,
    Quit,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RecordingCommand {
    Start(Option<PathBuf>),
    Stop,
}

impl Command {
    pub(crate) fn from_line(line: &str) -> Result<Self, String> {
        let line = line.trim();
        if line.is_empty() {
            return Err("enter a command; use help to list commands".into());
        }
        if let Some(text) = line.strip_prefix("text") {
            let Some(text) = text.strip_prefix(char::is_whitespace) else {
                return Err("usage: text TEXT".into());
            };
            let text = text.trim_start();
            if text.is_empty() {
                return Err("text requires a non-empty value".into());
            }
            return Ok(Command::Action(Action::Text(text.into())));
        }

        let words = line.split_whitespace().collect::<Vec<_>>();
        match words.as_slice() {
        ["help"] => Ok(Command::Help),
        ["status"] => Ok(Command::Status),
        ["quit"] => Ok(Command::Quit),
        ["pause"] => Ok(Command::Pause),
        ["resume"] => Ok(Command::Resume),
        ["step", frames] => frames
            .parse::<u64>()
            .map(Command::Step)
            .map_err(|_| "step FRAMES requires a positive integer".into()),
        ["record", "start"] => Ok(Command::Recording(RecordingCommand::Start(None))),
        ["record", "start", path] => Ok(Command::Recording(RecordingCommand::Start(Some(
            PathBuf::from(path),
        )))),
        ["record", "stop"] => Ok(Command::Recording(RecordingCommand::Stop)),
        ["click", target] => Ok(Command::Click((*target).into())),
        ["pointer", "move", x, y] => Ok(Command::Action(Action::Pointer(PointerAction::Move {
            x: number(x, "pointer X")?,
            y: number(y, "pointer Y")?,
        }))),
        ["pointer", "press", value] => Ok(Command::Action(Action::Pointer(PointerAction::Press(
            Button::from_name(value)?,
        )))),
        ["pointer", "release", value] => Ok(Command::Action(Action::Pointer(
            PointerAction::Release(Button::from_name(value)?),
        ))),
        ["pointer", "click", value] => Ok(Command::Action(Action::Pointer(PointerAction::Click(
            Button::from_name(value)?,
        )))),
        ["scroll", x, y] => Ok(Command::Action(Action::Pointer(PointerAction::Scroll {
            x: number(x, "scroll DX")?,
            y: number(y, "scroll DY")?,
        }))),
        ["key", key, state] if state.eq_ignore_ascii_case("press") => Ok(Command::Action(
            Action::Keyboard(KeyboardAction::Press((*key).into())),
        )),
        ["key", key, state] if state.eq_ignore_ascii_case("release") => Ok(Command::Action(
            Action::Keyboard(KeyboardAction::Release((*key).into())),
        )),
        ["observe", scope] => Observation::from_name(scope)
            .map(Command::Observe)
            .ok_or_else(|| format!(
                "unsupported observation {scope:?}; expected targets, ui, pointers, input, or clock"
            )),
        ["pointer", ..] => {
            Err("usage: pointer move X Y | pointer press|release|click left|right|middle".into())
        }
        ["key", ..] => Err("usage: key KEY press|release".into()),
        ["observe", ..] => Err("usage: observe targets|ui|pointers|input|clock".into()),
        ["step", ..] => Err("usage: step FRAMES".into()),
        ["record", ..] => Err("usage: record start [PATH] | record stop".into()),
        ["click", ..] => Err("usage: click TARGET".into()),
        _ => Err(format!(
            "unknown command {line:?}; use help to list commands"
        )),
    }
    }
}

pub(crate) fn run(
    mut session: ControllerSession,
    interrupted: Arc<AtomicBool>,
) -> Result<(), ControllerError> {
    let input = stdin_events();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let loop_result = run_loop(&mut session, input, &mut output, interrupted);
    match loop_result {
        Ok(()) => session.shutdown(),
        Err(error) => Err(error),
    }
}

fn run_loop(
    session: &mut ControllerSession,
    input: Receiver<InputEvent>,
    output: &mut impl Write,
    interrupted: Arc<AtomicBool>,
) -> Result<(), ControllerError> {
    write_status(output, &session.status()?).map_err(output_error)?;
    write!(output, "\n> ")
        .and_then(|_| output.flush())
        .map_err(output_error)?;

    loop {
        if interrupted.load(Ordering::SeqCst) {
            writeln!(output).map_err(output_error)?;
            return Ok(());
        }
        match input.recv_timeout(Duration::from_millis(100)) {
            Ok(InputEvent::Line(line)) => {
                let command = match Command::from_line(&line) {
                    Ok(command) => command,
                    Err(error) => {
                        session.capture_invalid_command();
                        writeln!(output, "error: {error}").map_err(output_error)?;
                        write!(output, "> ")
                            .and_then(|_| output.flush())
                            .map_err(output_error)?;
                        continue;
                    }
                };
                if matches!(command, Command::Quit) {
                    return Ok(());
                }
                match execute(session, command, output) {
                    Ok(()) => {}
                    Err(error) if error.is_fatal() => {
                        session.capture_operation_error(&error);
                        return Err(error);
                    }
                    Err(error) => {
                        session.capture_operation_error(&error);
                        writeln!(output, "error: {error}").map_err(output_error)?;
                    }
                }
                write!(output, "\n> ")
                    .and_then(|_| output.flush())
                    .map_err(output_error)?;
            }
            Ok(InputEvent::Eof) => {
                writeln!(output).map_err(output_error)?;
                return Ok(());
            }
            Ok(InputEvent::Error(message)) => {
                return Err(ControllerError::Communication(format!(
                    "could not read terminal input: {message}"
                )));
            }
            Err(RecvTimeoutError::Timeout) => session.ensure_running()?,
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

fn execute(
    session: &mut ControllerSession,
    command: Command,
    output: &mut impl Write,
) -> Result<(), ControllerError> {
    match command {
        Command::Click(target) => session.activate_target(&target)?,
        Command::Action(action) => {
            session.perform(action)?;
        }
        Command::Observe(observation) => {
            let value = session.observe(observation)?;
            writeln!(
                output,
                "observation {}:\n{}",
                observation.as_str(),
                serde_json::to_string_pretty(&value).map_err(|error| {
                    ControllerError::Communication(format!("could not format observation: {error}"))
                })?
            )
            .map_err(output_error)?;
        }
        Command::Pause => session.pause()?,
        Command::Resume => session.resume()?,
        Command::Step(frames) => session.step(frames)?,
        Command::Recording(RecordingCommand::Start(path)) => {
            let path = session.start_recording(path)?;
            writeln!(output, "recording started: {}", path.display()).map_err(output_error)?;
        }
        Command::Recording(RecordingCommand::Stop) => {
            let path = session.stop_recording()?;
            writeln!(output, "recording stopped: {}", path.display()).map_err(output_error)?;
        }
        Command::Status => {}
        Command::Help => {
            writeln!(output, "{HELP}").map_err(output_error)?;
        }
        Command::Quit => unreachable!("quit is handled by the REPL loop"),
    }
    write_status(output, &session.status()?).map_err(output_error)
}

pub(crate) fn write_status(output: &mut impl Write, status: &Status) -> io::Result<()> {
    writeln!(
        output,
        "instance={} mode={} screen={} paused={}",
        status.instance, status.mode, status.active_screen, status.paused
    )?;
    writeln!(output, "last action: {}", status.last_action)
}

fn number(value: &str, name: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .map_err(|_| format!("{name} must be a number, got {value:?}"))
}

fn output_error(error: io::Error) -> ControllerError {
    ControllerError::Communication(format!("could not write terminal output: {error}"))
}

enum InputEvent {
    Line(String),
    Eof,
    Error(String),
}

fn stdin_events() -> Receiver<InputEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        loop {
            let mut line = String::new();
            match input.read_line(&mut line) {
                Ok(0) => {
                    let _ = sender.send(InputEvent::Eof);
                    return;
                }
                Ok(_) => {
                    while line.ends_with(['\n', '\r']) {
                        line.pop();
                    }
                    if sender.send(InputEvent::Line(line)).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = sender.send(InputEvent::Error(error.to_string()));
                    return;
                }
            }
        }
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::super::controller::Mode;
    use super::*;

    #[test]
    fn parses_the_documented_line_commands() {
        assert_eq!(
            Command::from_line("click menu.tab.museum").unwrap(),
            Command::Click("menu.tab.museum".into())
        );
        assert_eq!(
            Command::from_line("pointer move 0.5 0.3").unwrap(),
            Command::Action(Action::Pointer(PointerAction::Move { x: 0.5, y: 0.3 }))
        );
        assert_eq!(
            Command::from_line("pointer press right").unwrap(),
            Command::Action(Action::Pointer(PointerAction::Press(Button::Right)))
        );
        assert_eq!(
            Command::from_line("pointer release middle").unwrap(),
            Command::Action(Action::Pointer(PointerAction::Release(Button::Middle)))
        );
        assert_eq!(
            Command::from_line("pointer click left").unwrap(),
            Command::Action(Action::Pointer(PointerAction::Click(Button::Left)))
        );
        assert_eq!(
            Command::from_line("scroll 1 -2").unwrap(),
            Command::Action(Action::Pointer(PointerAction::Scroll { x: 1.0, y: -2.0 }))
        );
        assert_eq!(
            Command::from_line("key Escape press").unwrap(),
            Command::Action(Action::Keyboard(KeyboardAction::Press("Escape".into())))
        );
        assert_eq!(
            Command::from_line("key Escape release").unwrap(),
            Command::Action(Action::Keyboard(KeyboardAction::Release("Escape".into())))
        );
        assert_eq!(
            Command::from_line("text hello museum visitors").unwrap(),
            Command::Action(Action::Text("hello museum visitors".into()))
        );
        assert_eq!(
            Command::from_line("observe input").unwrap(),
            Command::Observe(Observation::VirtualInput)
        );
        assert_eq!(Command::from_line("pause").unwrap(), Command::Pause);
        assert_eq!(Command::from_line("resume").unwrap(), Command::Resume);
        assert_eq!(Command::from_line("step 3").unwrap(), Command::Step(3));
        assert_eq!(
            Command::from_line("record start records/session.jsonl").unwrap(),
            Command::Recording(RecordingCommand::Start(Some(PathBuf::from(
                "records/session.jsonl"
            ))))
        );
        assert_eq!(
            Command::from_line("record start").unwrap(),
            Command::Recording(RecordingCommand::Start(None))
        );
        assert_eq!(
            Command::from_line("record stop").unwrap(),
            Command::Recording(RecordingCommand::Stop)
        );
        assert_eq!(Command::from_line("status").unwrap(), Command::Status);
        assert_eq!(Command::from_line("help").unwrap(), Command::Help);
        assert_eq!(Command::from_line("quit").unwrap(), Command::Quit);
    }

    #[test]
    fn command_errors_include_the_expected_syntax() {
        assert_eq!(
            Command::from_line("pointer move left down").unwrap_err(),
            "pointer X must be a number, got \"left\""
        );
        assert!(
            Command::from_line("pointer press fourth")
                .unwrap_err()
                .contains("left, right, or middle")
        );
        assert_eq!(
            Command::from_line("key Escape").unwrap_err(),
            "usage: key KEY press|release"
        );
        assert_eq!(
            Command::from_line("record start one two").unwrap_err(),
            "usage: record start [PATH] | record stop"
        );
        assert!(
            Command::from_line("observe world")
                .unwrap_err()
                .contains("targets, ui, pointers")
        );
        assert!(Command::from_line("").unwrap_err().contains("use help"));
    }

    #[test]
    fn status_is_stable_and_human_readable() {
        let status = Status {
            instance: "alpha".into(),
            mode: Mode::Rendered,
            active_screen: "museum".into(),
            paused: false,
            last_action: "click menu.tab.museum".into(),
        };
        let mut output = Vec::new();
        write_status(&mut output, &status).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "instance=alpha mode=rendered screen=museum paused=false\nlast action: click menu.tab.museum\n"
        );
    }
}
