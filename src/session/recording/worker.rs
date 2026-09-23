use super::super::Error;
use super::file::Recording;
use cap_std::fs::Dir;
use serde_json::Value;
use std::sync::mpsc;

pub(super) enum Operation {
    Start(String),
    Append(Vec<Value>),
    Stop,
    End(Vec<Value>),
}

pub(super) enum Reply {
    Started(Result<(), Error>),
    Stopped(Result<u64, Error>),
    Failed { path: String, message: String },
    Ended,
}

pub(super) fn spawn(root: Dir) -> (mpsc::Sender<Operation>, mpsc::Receiver<Reply>) {
    let (sender, operations) = mpsc::channel();
    let (replies, receiver) = mpsc::channel();
    std::thread::spawn(move || run(root, operations, replies, Recording::start));
    (sender, receiver)
}

pub(super) fn run<W: super::file::Sink>(
    root: Dir,
    operations: mpsc::Receiver<Operation>,
    replies: mpsc::Sender<Reply>,
    mut start: impl FnMut(&Dir, String) -> Result<Recording<W>, Error>,
) {
    let mut active: Option<Recording<W>> = None;
    for operation in operations {
        let reply = match operation {
            Operation::Start(path) => Reply::Started(start(&root, path).map(|recording| {
                active = Some(recording);
            })),
            Operation::Append(entries) => {
                let Some(recording) = &mut active else {
                    continue;
                };
                if let Err(error) = recording.append(entries) {
                    let path = recording.path.clone();
                    active = None;
                    Reply::Failed {
                        path,
                        message: error.message().into(),
                    }
                } else {
                    continue;
                }
            }
            Operation::Stop => Reply::Stopped(match active.take() {
                Some(recording) => recording.finish("stopped"),
                None => Err(Error::io("recording failed before stop")),
            }),
            Operation::End(entries) => {
                if let Some(mut recording) = active.take() {
                    let path = recording.path.clone();
                    if let Err(error) = recording
                        .append(entries)
                        .and_then(|_| recording.finish("session_ended"))
                    {
                        let _ = replies.send(Reply::Failed {
                            path,
                            message: error.message().into(),
                        });
                    }
                }
                let _ = replies.send(Reply::Ended);
                return;
            }
        };
        if replies.send(reply).is_err() {
            return;
        }
    }
}
