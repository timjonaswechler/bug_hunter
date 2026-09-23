mod network;
#[cfg(test)]
mod network_tests;
mod parse;
mod terminal;

use crate::server::protocol::Result as Reply;
use network::{Event, Network, Request};
use std::{collections::BTreeSet, net::SocketAddr, sync::mpsc, time::Duration};

pub(super) fn run(address: SocketAddr, selector: String) -> Result<(), super::Error> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let mut terminal = terminal::Terminal::new()?;
            let network = Network::start(address, selector)?;
            let mut shown = BTreeSet::new();
            terminal.print(
                "Connecting. help lists commands; quit disconnects without stopping the session.",
            )?;
            let interrupt = tokio::signal::ctrl_c();
            tokio::pin!(interrupt);
            let mut tick = tokio::time::interval(Duration::from_millis(10));
            loop {
                tokio::select! {
                    result = &mut interrupt => { result?; return Ok(()); }
                    _ = tick.tick() => {}
                }
                // Render at most one response before checking input. A busy peer
                // must not monopolize the terminal with a full output queue.
                match network.events.try_recv() {
                    Ok(Event::End(result)) => return result.map_err(Into::into),
                    Ok(event) => display(event, &mut shown, &mut terminal)?,
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err("REPL network worker ended unexpectedly".into());
                    }
                }
                if let Some(input) = terminal.read()? {
                    let line = match input {
                        terminal::Input::Closed | terminal::Input::Interrupted => return Ok(()),
                        terminal::Input::Invalid(error) => {
                            terminal.print(&error)?;
                            continue;
                        }
                        terminal::Input::Line(line) => line,
                    };
                    let request = match parse::parse(&line) {
                        Ok(parse::Input::Empty) => continue,
                        Ok(parse::Input::Quit) => return Ok(()),
                        Ok(parse::Input::Help) => {
                            terminal.print(parse::HELP)?;
                            continue;
                        }
                        Ok(parse::Input::InspectHelp) => {
                            terminal.print(parse::INSPECT_HELP)?;
                            continue;
                        }
                        Ok(parse::Input::Pending) => Request::Pending,
                        Ok(parse::Input::Command(command)) => Request::Submit(command),
                        Err(error) => {
                            terminal.print(&format!("input error: {error}"))?;
                            continue;
                        }
                    };
                    match network.requests.try_send(request) {
                        Ok(()) => {}
                        Err(mpsc::TrySendError::Full(_)) => {
                            terminal.print("not sent: client input queue is full")?
                        }
                        Err(mpsc::TrySendError::Disconnected(_)) => {
                            return Err("REPL network worker unavailable".into());
                        }
                    }
                }
            }
        })
}

fn display(
    event: Event,
    shown: &mut BTreeSet<u64>,
    terminal: &mut terminal::Terminal,
) -> std::io::Result<()> {
    match event {
        Event::Connected(snapshot) => {
            terminal.print(&format!(
                "connected {} {:?}",
                snapshot.cursor.session_id, snapshot.state
            ))?;
            for pending in snapshot.pending {
                if shown.insert(pending.request_id) {
                    terminal.print(&format!(
                        "[{}] {} pending",
                        pending.request_id, pending.command
                    ))?;
                }
            }
        }
        Event::Pending(snapshot) => {
            terminal.print(&format!("server pending: {}", snapshot.pending.len()))?;
            for pending in snapshot.pending {
                terminal.print(&format!(
                    "[{}] {} pending",
                    pending.request_id, pending.command
                ))?;
            }
        }
        Event::Notice(message) => terminal.print(&message)?,
        Event::Reply(Reply::Pending {
            request_id,
            command,
        }) => {
            if shown.insert(request_id) {
                terminal.print(&format!("[{request_id}] {command} pending"))?;
            }
        }
        Event::Reply(Reply::Activity { entries, .. }) => {
            for entry in entries {
                let event = entry.event;
                if let Some(id) = event["request_id"].as_u64() {
                    if event["kind"] == "pending" {
                        if !shown.insert(id) {
                            continue;
                        }
                        terminal.print(&format!(
                            "[{id}] {} pending",
                            event["command"].as_str().unwrap_or("?")
                        ))?;
                        continue;
                    }
                    shown.remove(&id);
                }
                terminal.print(&event.to_string())?;
            }
        }
        Event::Reply(Reply::Gap { message, .. }) => {
            terminal.print(&format!("Activity gap: {message}; missing outcomes are unknown. Resuming at oldest retained entry."))?;
            shown.clear();
        }
        Event::Reply(reply) => terminal.print(&serde_json::to_string(&reply)?)?,
        Event::End(_) => unreachable!(),
    }
    Ok(())
}
