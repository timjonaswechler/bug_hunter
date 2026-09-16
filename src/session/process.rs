//! Owns the process group. Readers drain both pipes independently of command consumers.
use super::Error;
use std::{
    io::{BufRead, Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread::JoinHandle,
};

pub(super) enum Event {
    Line(String),
    Diagnostic(Vec<u8>),
    Closed(&'static str),
    Failed(&'static str, String),
}

pub(super) struct Process {
    pub child: Child,
    pub events: mpsc::Receiver<Event>,
    pub writer: Option<mpsc::Sender<String>>,
    workers: Vec<JoinHandle<()>>,
    terminated: bool,
}

impl Process {
    pub fn spawn(mut command: Command) -> Result<Self, Error> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(not(unix))]
        return Err(Error::new(
            "launch",
            "this experimental process supervisor currently requires Unix",
        ));
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::new("launch", e))?;
        let stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let (tx, events) = mpsc::channel();
        let out = tx.clone();
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if out.send(Event::Line(line)).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = out.send(Event::Failed("stdout", e.to_string()));
                        return;
                    }
                }
            }
            let _ = out.send(Event::Closed("stdout"));
        });
        let err = tx.clone();
        let diagnostic = std::thread::spawn(move || {
            let mut buffer = [0; 8192];
            loop {
                match stderr.read(&mut buffer) {
                    Ok(0) => {
                        let _ = err.send(Event::Closed("stderr"));
                        return;
                    }
                    Ok(n) => {
                        if err.send(Event::Diagnostic(buffer[..n].to_vec())).is_err() {
                            return;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        let _ = err.send(Event::Failed("stderr", e.to_string()));
                        return;
                    }
                }
            }
        });
        let (writer, lines) = mpsc::channel::<String>();
        let input = std::thread::spawn(move || {
            for line in lines {
                if let Err(e) = writeln!(stdin, "{line}").and_then(|_| stdin.flush()) {
                    let _ = tx.send(Event::Failed("stdin", e.to_string()));
                    break;
                }
            }
        });
        Ok(Self {
            child,
            events,
            writer: Some(writer),
            workers: vec![reader, diagnostic, input],
            terminated: false,
        })
    }

    pub fn terminate(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        #[cfg(unix)]
        // SAFETY: the child is launched in its own process group, with pgid equal to its pid.
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let _ = self.child.kill();
        self.writer.take();
        let _ = self.child.wait();
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.terminate();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
