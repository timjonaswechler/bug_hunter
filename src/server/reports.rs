//! One owned provider worker per session. No client participates in job ownership.
use super::entry::Entry;
use crate::report::{self, Report};
use std::sync::{Arc, atomic::Ordering, mpsc};

pub(super) struct Reports {
    entry: Arc<Entry>,
    sender: Option<mpsc::Sender<Report>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Reports {
    pub fn new(entry: Arc<Entry>, destination: Arc<report::Destination>) -> Self {
        let (sender, jobs) = mpsc::channel::<Report>();
        let owner = entry.clone();
        let worker = std::thread::spawn(move || {
            for report in jobs {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    destination.run(&report, &owner.report_cancel)
                }))
                .unwrap_or_else(|_| report::work::Result::Interrupted {
                    message: "report worker panicked; external outcome may be unknown".into(),
                });
                if matches!(result, report::work::Result::Interrupted { .. }) {
                    owner.report_incomplete.store(true, Ordering::Release);
                }
                owner.push(serde_json::json!({"kind":"report","report":report,"result":result}));
            }
        });
        Self {
            entry,
            sender: Some(sender),
            worker: Some(worker),
        }
    }

    pub fn submit(&self, report: Report) {
        if let Err(error) = self.sender.as_ref().unwrap().send(report) {
            self.entry.report_incomplete.store(true, Ordering::Release);
            self.entry.push(serde_json::json!({
                "kind":"report", "report":error.0,
                "result":{"status":"interrupted","message":"report worker unavailable; outcome unknown"}
            }));
        }
    }

    pub fn finish(mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Reports {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            // Unwinding the session owner must not detach provider work.
            self.entry.report_cancel.store(true, Ordering::Release);
            self.sender.take();
            let _ = worker.join();
        }
    }
}
