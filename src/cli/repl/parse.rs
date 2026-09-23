use crate::{
    command::{Command, Empty, inspect, recording, replay, tick::warp},
    handle::Handle,
};

#[derive(Debug)]
pub(super) enum Input {
    Empty,
    Help,
    InspectHelp,
    Pending,
    Quit,
    Command(Command),
}

fn operand(text: &str) -> Result<&str, String> {
    let text = text.trim();
    if text.is_empty() {
        Err("missing operand".into())
    } else {
        Ok(text)
    }
}
fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err("expected an unsigned decimal integer".into());
    }
    text.parse().map_err(|_| "integer out of range".into())
}
fn pace(text: &str) -> Result<warp::Pace, String> {
    if text == "max" {
        return Ok(warp::Pace::AsFastAsPossible);
    }
    let target = text
        .parse()
        .map_err(|_| "expected max or positive ticks-per-second")?;
    let pace = warp::Pace::TicksPerSecond { target };
    if !pace.is_valid() {
        return Err("pace must be finite and positive".into());
    }
    Ok(pace)
}

pub(super) fn parse(line: &str) -> Result<Input, String> {
    let line = line.trim();
    // Preserve the full JSON argument and Type Path, including internal spaces.
    let (first, rest) = split(line);
    if first == "command" {
        return serde_json::from_str::<Command>(operand(rest)?)
            .map(Input::Command)
            .map_err(|e| format!("invalid command: {e}"));
    }
    let (second, rest) = split(rest);
    if first == "inspect" {
        let command = match second {
            "query" => serde_json::from_str::<inspect::Command>(operand(rest)?)
                .map_err(|e| format!("invalid inspect arguments: {e}"))?,
            "entities" if rest.is_empty() => entities(None),
            "entity" => {
                let (index, generation) = operand(rest)?
                    .split_once(':')
                    .ok_or("expected <index>:<generation>")?;
                entities(Some(Handle::new(number(index)?, number(generation)?)))
            }
            "resources" if rest.is_empty() => inspect::Command::Resources {
                selector: inspect::Selector::All {},
                projection: inspect::Projection::Metadata {},
            },
            "resource" => inspect::Command::Resources {
                selector: inspect::Selector::Type {
                    type_path: operand(rest)?.into(),
                },
                projection: inspect::Projection::Value {},
            },
            _ => return Err("unknown inspect form; use help inspect".into()),
        };
        return Ok(Input::Command(command.into()));
    }
    if matches!(first, "recording" | "replay") && second == "start" {
        let path = operand(rest)?.into();
        return Ok(Input::Command(if first == "recording" {
            recording::Start { path }.into()
        } else {
            replay::Start { path }.into()
        }));
    }
    let words: Vec<_> = line.split_whitespace().collect();
    Ok(match words.as_slice() {
        [] => Input::Empty,
        ["help"] => Input::Help,
        ["help", "inspect"] => Input::InspectHelp,
        ["pending"] => Input::Pending,
        ["quit"] => Input::Quit,
        ["shutdown"] => Input::Command(Command::Shutdown(Empty {})),
        ["recording", "stop"] => Input::Command(recording::Stop {}.into()),
        ["replay", "stop"] => Input::Command(replay::Stop {}.into()),
        ["tick", "warp", "stop"] => Input::Command(warp::Stop {}.into()),
        ["tick", "warp", "pace", value] => {
            Input::Command(warp::SetPace { pace: pace(value)? }.into())
        }
        ["tick", "warp", ticks] => Input::Command(
            warp::Start {
                ticks: number(ticks)?,
                pace: None,
            }
            .into(),
        ),
        ["tick", "warp", ticks, "pace", value] => Input::Command(
            warp::Start {
                ticks: number(ticks)?,
                pace: Some(pace(value)?),
            }
            .into(),
        ),
        _ => return Err("unknown input; use help".into()),
    })
}
fn split(text: &str) -> (&str, &str) {
    let (word, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    (word, rest.trim_start())
}
fn entities(entity: Option<Handle>) -> inspect::Command {
    inspect::Command::Entities {
        entity,
        with: vec![],
        without: vec![],
        projection: inspect::entity::Projection::Summary {},
    }
}

pub(super) const HELP: &str = "\
help | help inspect | pending | quit
command <command-json>
tick warp <ticks> [pace max|<ticks-per-second>]
tick warp pace max|<ticks-per-second>
tick warp stop
recording start <path> | recording stop
replay start <path> | replay stop
inspect query <arguments-json>
inspect entities | inspect entity <index>:<generation>
inspect resources | inspect resource <type-path>
shutdown
quit, Ctrl+C and EOF disconnect only this client. shutdown explicitly ends the session.";

pub(super) const INSPECT_HELP: &str = r#"inspect entities
inspect entity 1:0
inspect resources
inspect resource my_game::State
inspect query {"source":"entities","entity":null,"with":[],"without":[],"projection":{"kind":"summary"}}
inspect query {"source":"entities","entity":{"index":1,"generation":0},"with":[],"without":[],"projection":{"kind":"component_names"}}
inspect query {"source":"entities","entity":{"index":1,"generation":0},"with":[],"without":[],"projection":{"kind":"components","selection":{"kind":"listed","type_paths":["my_game::Position"]}}}
inspect query {"source":"entities","entity":{"index":1,"generation":0},"with":[],"without":[],"projection":{"kind":"hierarchy","depth":2}}
inspect query {"source":"resources","selector":{"kind":"all"},"projection":{"kind":"metadata"}}
inspect query {"source":"resources","selector":{"kind":"type","type_path":"my_game::State"},"projection":{"kind":"value"}}"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn command(line: &str) -> Command {
        let Input::Command(command) = parse(line).unwrap() else {
            panic!("{line}")
        };
        command
    }
    #[test]
    fn inspect_help_examples_all_decode_as_shared_commands() {
        for line in INSPECT_HELP.lines() {
            let command = command(line);
            let encoded = serde_json::to_string(&command).unwrap();
            assert_eq!(serde_json::from_str::<Command>(&encoded).unwrap(), command);
        }
    }
    #[test]
    fn short_forms_preserve_full_arguments_and_handle_bits() {
        assert_eq!(command("inspect entities"), entities(None).into());
        assert_eq!(
            command("inspect entity 4294967295:0002"),
            entities(Some(Handle::new(u32::MAX, 2))).into()
        );
        assert_eq!(
            command(" inspect resource  game::Map<String, 日本語>  "),
            inspect::Command::Resources {
                selector: inspect::Selector::Type {
                    type_path: "game::Map<String, 日本語>".into()
                },
                projection: inspect::Projection::Value {},
            }
            .into()
        );
        assert_eq!(
            command("recording start recordings/my run.jsonl"),
            recording::Start {
                path: "recordings/my run.jsonl".into()
            }
            .into()
        );
        assert_eq!(
            command("tick warp 18446744073709551615 pace 2.5"),
            warp::Start {
                ticks: u64::MAX,
                pace: Some(warp::Pace::TicksPerSecond { target: 2.5 })
            }
            .into()
        );
    }
    #[test]
    fn bad_inputs_never_become_commands() {
        for line in [
            "inspect entity +1:0",
            "inspect entity 1:2:3",
            "inspect entity 1:-1",
            "inspect entity 4294967296:0",
            "inspect entity 1: 2",
            "inspect entity 1:２",
            "inspect resource ",
            "inspect entities extra",
            "inspect query {}",
            "inspect query {",
            "tick warp -1",
            "tick warp 18446744073709551616",
            "tick warp pace NaN",
            "tick warp pace inf",
            "tick warp pace 0",
            "tick warp 3 extra",
            "recording start",
            "quit extra",
            "shutdown extra",
        ] {
            assert!(parse(line).is_err(), "{line}");
        }
    }
    #[test]
    fn all_control_forms_and_local_inputs() {
        for (line, name) in [
            ("tick warp 0", "tick.warp.start"),
            ("tick warp pace max", "tick.warp.set_pace"),
            ("tick warp stop", "tick.warp.stop"),
            ("recording stop", "recording.stop"),
            ("replay start plans/test.jsonl", "replay.start"),
            ("replay stop", "replay.stop"),
            ("shutdown", "shutdown"),
        ] {
            assert_eq!(command(line).name(), name);
        }
        assert!(matches!(parse("help"), Ok(Input::Help)));
        assert!(matches!(parse("help inspect"), Ok(Input::InspectHelp)));
        assert!(matches!(parse("pending"), Ok(Input::Pending)));
        assert!(matches!(parse("quit"), Ok(Input::Quit)));
        assert!(matches!(parse(" \t"), Ok(Input::Empty)));
    }

    #[test]
    fn generic_form_uses_the_shared_codec_for_commands_without_short_forms() {
        let line =
            r#"command {"command":"screenshot.capture","arguments":{"path":"shots/view.png"}}"#;
        assert_eq!(command(line).name(), "screenshot.capture");
        assert!(
            parse(r#"command {"command":"screenshot.capture","arguments":{"unknown":true}}"#)
                .is_err()
        );
        assert!(parse("command {").is_err());
    }
}
