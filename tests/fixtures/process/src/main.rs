use serde_json::{Value, json};
use std::{
    io::{BufRead, Write},
    time::Duration,
};

fn emit(value: Value) {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &value).unwrap();
    writeln!(stdout).unwrap();
    stdout.flush().unwrap();
}
fn completed(id: u64, command: &str, output: Value) {
    emit(json!({"request_id":id,"command":command,"status":"completed","output":output}));
}
fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let root = std::path::PathBuf::from(std::env::var_os("WOODPECKER_ARTIFACT_DIR").unwrap());
    std::fs::write(root.join("pid"), std::process::id().to_string()).unwrap();
    if mode == "delayed" {
        std::thread::sleep(Duration::from_secs(60));
    }
    if mode == "wrong_version" {
        emit(json!({"status":"ready","version":2,"capabilities":{"screenshot":false}}));
        std::thread::sleep(Duration::from_secs(60));
    }
    if mode == "full_pipes" {
        std::io::stderr()
            .write_all(&vec![b'x'; 1024 * 1024])
            .unwrap();
    }
    emit(json!({"status":"ready","version":3,"capabilities":{"screenshot":false}}));
    if mode == "overflow" {
        for _ in 0..400 {
            emit(
                json!({"status":"protocol_error","request_id":null,"error":{"code":"fixture","message":"unknown"}}),
            );
        }
    }
    let mut active = None;
    for line in std::io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let id = request["request_id"].as_u64().unwrap();
        let command = request["command"].as_str().unwrap();
        if mode == "unsolicited" {
            emit(
                json!({"status":"protocol_error","request_id":987654,"error":{"code":"fixture","message":"unknown"}}),
            );
            emit(json!({"status":"ready","version":3,"capabilities":{"screenshot":true}}));
        }
        if mode == "exit" {
            let child = std::process::Command::new("sleep")
                .arg("60")
                .spawn()
                .unwrap();
            std::fs::write(root.join("descendant"), child.id().to_string()).unwrap();
            completed(id, command, json!({"was_running":false}));
            return;
        }
        if mode == "corrupt" && command != "shutdown" {
            completed(id, command, json!({"was_running":"invalid"}));
            continue;
        }
        if mode == "known_error" && command != "shutdown" {
            emit(
                json!({"status":"protocol_error","request_id":id,"error":{"code":"fixture","message":"known"}}),
            );
            continue;
        }
        if mode == "wrong_name" && command != "shutdown" {
            completed(id, "inspect.query", json!({"items":[]}));
            continue;
        }
        match command {
            "tick.warp.start" => {
                let ticks = request["arguments"]["ticks"].as_u64().unwrap();
                if ticks == 1 {
                    completed(
                        id,
                        command,
                        json!({"requested_ticks":1,"executed_ticks":1,"outcome":"completed"}),
                    );
                } else {
                    active = Some((id, ticks));
                }
            }
            "tick.warp.stop" => {
                completed(id, command, json!({"was_running":active.is_some()}));
                if let Some((id, requested)) = active.take() {
                    completed(
                        id,
                        "tick.warp.start",
                        json!({"requested_ticks":requested,"executed_ticks":0,"outcome":"stopped"}),
                    );
                }
            }
            "inspect.query" => completed(id, command, json!({"items":[]})),
            "shutdown" if mode == "hang_shutdown" => std::thread::sleep(Duration::from_secs(60)),
            "shutdown" => {
                completed(id, command, Value::Null);
                return;
            }
            _ => panic!("unsupported fixture command"),
        }
    }
}
