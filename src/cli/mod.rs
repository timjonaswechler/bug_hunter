//! Machine-oriented CLI, interactive REPL and prevalidated scripts.
mod config;
mod repl;
pub mod script;
use crate::{
    client::Management,
    command::Command,
    server::{
        self,
        protocol::{Cursor, Result as Reply},
    },
};
use clap::{Parser, Subcommand};
use std::{io::Write, net::SocketAddr, path::PathBuf, time::Duration};
type Error = Box<dyn std::error::Error>;

#[derive(Parser)]
#[command(name = "woodpecker", about = "Experimental v3 headless vertical slice")]
struct Arguments {
    #[arg(long)]
    address: SocketAddr,
    #[command(subcommand)]
    resource: Resource,
}
#[derive(Subcommand)]
enum Resource {
    Server {
        #[command(subcommand)]
        action: Server,
    },
    Session {
        #[command(subcommand)]
        action: Session,
    },
}
#[derive(Subcommand)]
enum Server {
    Start {
        #[arg(long)]
        artifact_dir: PathBuf,
        #[arg(long, default_value = "30")]
        shutdown_seconds: f64,
        #[arg(long, default_value = "4194304")]
        activity_bytes: usize,
    },
    Stop,
}
#[derive(Subcommand)]
enum Session {
    Script {
        id: String,
        #[arg(long)]
        file: PathBuf,
    },
    Repl {
        id: String,
    },
    Create {
        #[arg(long)]
        config: PathBuf,
    },
    Ls,
    Inspect {
        id: String,
    },
    Stop {
        id: String,
    },
    Submit {
        id: String,
        #[arg(long)]
        command: String,
    },
    Poll {
        id: String,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, default_value = "0")]
        wait_ms: u64,
    },
}

pub fn run() -> Result<(), Error> {
    let args = Arguments::parse();
    if let Resource::Server {
        action:
            Server::Start {
                artifact_dir,
                shutdown_seconds,
                activity_bytes,
            },
    } = args.resource
    {
        if !shutdown_seconds.is_finite() || shutdown_seconds <= 0.0 {
            return Err("shutdown seconds must be finite and positive".into());
        }
        let timeout = Duration::try_from_secs_f64(shutdown_seconds)?;
        return tokio::runtime::Runtime::new()?.block_on(async {
            let server = server::Server::bind(server::Config {
                address: args.address, artifact_dir, shutdown_timeout: timeout, activity_bytes,
            }).await?;
            print(&serde_json::json!({"state":"Ready","address":server.address()?.to_string(),"experimental":true}))?;
            server.run().await?;
            Ok(())
        });
    }
    let management = Management::new(args.address)?;
    let value = match args.resource {
        Resource::Server {
            action: Server::Stop,
        } => management.request("POST", "/stop", None)?,
        Resource::Session { action } => match action {
            Session::Script { id, file } => {
                let script = match script::Script::parse(&std::fs::read_to_string(file)?) {
                    Ok(script) => script,
                    Err(error) => {
                        print(&error)?;
                        return Err(error.into());
                    }
                };
                let outcome = script::run_cli(args.address, id, script)?;
                print(&outcome)?;
                return if outcome.passed() {
                    Ok(())
                } else {
                    Err("script failed; see structured outcome".into())
                };
            }
            Session::Repl { id } => return repl::run(args.address, id),
            Session::Create { config: path } => {
                let create = config::parse(&std::fs::read_to_string(path)?)?;
                management.request("POST", "/sessions", Some(&serde_json::to_value(create)?))?
            }
            Session::Ls => management.request("GET", "/sessions", None)?,
            Session::Inspect { id } => {
                management.request("GET", &format!("/sessions/{id}"), None)?
            }
            Session::Stop { id } => {
                management.request("POST", &format!("/sessions/{id}/stop"), None)?
            }
            Session::Submit { id, command } => {
                let _: serde_json::Value =
                    serde_json::from_str(&command).map_err(|e| format!("invalid_json: {e}"))?;
                let command: Command =
                    serde_json::from_str(&command).map_err(|e| format!("invalid_command: {e}"))?;
                let reply = management.bind(&id)?.submit(command)?;
                print(&reply)?;
                return match reply {
                    Reply::Error { code, message } => Err(format!("{code}: {message}").into()),
                    _ => Ok(()),
                };
            }
            Session::Poll {
                id,
                cursor,
                wait_ms,
            } => {
                let cursor = cursor
                    .map(|s| serde_json::from_str::<Cursor>(&s))
                    .transpose()?;
                let reply = management.bind(&id)?.poll(cursor, wait_ms)?;
                print(&reply)?;
                return match reply {
                    Reply::Error { code, message } => Err(format!("{code}: {message}").into()),
                    Reply::Gap { .. } => Err("activity gap: missing outcomes are unknown".into()),
                    _ => Ok(()),
                };
            }
        },
        _ => unreachable!(),
    };
    print(&value)
}
fn print(value: &impl serde::Serialize) -> Result<(), Error> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, value)?;
    writeln!(stdout)?;
    stdout.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_operations_require_no_credentials() {
        let args = Arguments::try_parse_from([
            "woodpecker",
            "--address",
            "127.0.0.1:4100",
            "session",
            "ls",
        ])
        .unwrap();
        assert!(matches!(
            args.resource,
            Resource::Session {
                action: Session::Ls
            }
        ));
    }
}
