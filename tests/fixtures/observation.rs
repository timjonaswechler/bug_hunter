//! A real Bevy process exercising the public observation integration.
use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*};
use std::{io::Write, time::Duration};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "handled".into());
    let abort = mode == "abort";
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let root = std::env::var_os("WOODPECKER_ARTIFACT_DIR").unwrap();
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(std::path::Path::new(&root).join("prior-hook"))
        {
            let _ = writeln!(file, "called");
        }
        if abort {
            #[cfg(unix)]
            unsafe {
                let limit = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                libc::setrlimit(libc::RLIMIT_CORE, &limit);
            }
            std::process::abort();
        }
        previous(info);
    }));
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(2))));
    if mode != "missing_layer" {
        app.add_plugins(LogPlugin {
            custom_layer: woodpecker::session::tracing_error_layer,
            filter: if mode == "filtered" { "off" } else { "info" }.into(),
            ..default()
        });
    }
    if mode == "pre_ready" {
        app.add_systems(Startup, || panic!("panic before Ready"));
    }
    let mut first = true;
    app.add_systems(Update, move || {
        if !first {
            return;
        }
        first = false;
        match mode.as_str() {
            "panic" | "abort" => panic!("fixture panic\nsecond line"),
            "caught" => {
                let _ = std::panic::catch_unwind(|| panic!("caught panic"));
            }
            "unknown" => {
                let _ = std::panic::catch_unwind(|| std::panic::panic_any(42u32));
            }
            "worker" => {
                let _ = std::thread::spawn(|| panic!("worker panic")).join();
            }
            "task" => {
                bevy::tasks::AsyncComputeTaskPool::get()
                    .spawn(async {
                        panic!("task panic");
                    })
                    .detach();
            }
            "trace" | "filtered" => {
                tracing::error!(target: "fixture", answer = 42, "observed trace")
            }
            "fields" => {
                tracing::error!(target: "fixture", count = 3u64, valid = true, text = "Grüße")
            }
            "corrupt_marker" | "incomplete_marker" => {
                eprint!("\x1eWOODPECKER_REPORT:{{invalid");
                if mode == "corrupt_marker" {
                    eprintln!();
                }
                std::process::exit(0);
            }
            "exit_zero" => std::process::exit(0),
            _ => eprintln!("ERROR this is a handled Err, not a tracing event"),
        }
    });
    app.add_plugins(woodpecker::session::Plugin);
    app.run();
}
