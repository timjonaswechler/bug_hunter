#[path = "capture_scene_visibility_jsonl.rs"]
mod capture_scene_visibility_jsonl;

use capture_scene_visibility_jsonl::DiagnosticJsonl;
use std::sync::{Arc, Barrier};

static BEVY: DiagnosticJsonl = DiagnosticJsonl::new("bevy_render", "VISIBILITY_LOG_DIR");
static ADAPTER: DiagnosticJsonl =
    DiagnosticJsonl::new("woodpecker_adapter", "VISIBILITY_LOG_DIR");

fn main() {
    let token = std::env::args().nth(1).expect("token");
    let threads: usize = std::env::args().nth(2).expect("threads").parse().unwrap();
    let entries: usize = std::env::args().nth(3).expect("entries").parse().unwrap();
    let payload_size: usize = std::env::args().nth(4).expect("payload size").parse().unwrap();
    assert!(BEVY.enabled() && ADAPTER.enabled());

    let barrier = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|thread| {
            let barrier = barrier.clone();
            let token = token.clone();
            std::thread::spawn(move || {
                let (source, writer) = if thread % 2 == 0 {
                    ("bevy_render", &BEVY)
                } else {
                    ("woodpecker_adapter", &ADAPTER)
                };
                let payload = "x".repeat(payload_size);
                barrier.wait();
                for entry in 0..entries {
                    let event_id = format!("{source}-{token}-{thread}-{entry}");
                    let row = format!(
                        "{{\"source\":\"{source}\",\"event_id\":\"{event_id}\",\"payload\":\"{payload}\"}}"
                    );
                    writer.write_json(&row).expect("diagnostic write failed");
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}
