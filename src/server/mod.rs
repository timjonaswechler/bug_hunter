//! Loopback-only multi-session server for trusted local clients.
mod activity;
mod entry;
pub mod protocol;
mod transport;

use crate::session::Error;
use protocol::{Create, Detail, Lifecycle};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant},
};

pub struct Config {
    pub address: SocketAddr,
    pub artifact_dir: PathBuf,
    pub shutdown_timeout: Duration,
    pub activity_bytes: usize,
}

pub struct Server {
    listener: tokio::net::TcpListener,
    inner: Arc<Inner>,
}
struct Inner {
    directory: Mutex<Directory>,
    root: PathBuf,
    timeout: Duration,
    activity_bytes: usize,
}
#[derive(Default)]
struct Directory {
    entries: BTreeMap<String, Arc<entry::Entry>>,
    deadline: Option<Instant>,
    forced: bool,
}

impl Server {
    pub async fn bind(config: Config) -> Result<Self, Error> {
        if !config.address.ip().is_loopback()
            || config.artifact_dir.as_os_str().is_empty()
            || config.shutdown_timeout.is_zero()
            || Instant::now()
                .checked_add(config.shutdown_timeout)
                .is_none()
            || config.activity_bytes == 0
        {
            return Err(Error::new(
                "invalid_config",
                "loopback, artifact root, positive deadline and activity limit required",
            ));
        }
        std::fs::create_dir_all(&config.artifact_dir).map_err(Error::io)?;
        let root = config.artifact_dir.canonicalize().map_err(Error::io)?;
        let listener = tokio::net::TcpListener::bind(config.address)
            .await
            .map_err(Error::io)?;
        Ok(Self {
            listener,
            inner: Arc::new(Inner {
                directory: Mutex::new(Directory::default()),
                root,
                timeout: config.shutdown_timeout,
                activity_bytes: config.activity_bytes,
            }),
        })
    }
    pub fn address(&self) -> Result<SocketAddr, Error> {
        self.listener.local_addr().map_err(Error::io)
    }
    pub async fn run(self) -> Result<(), Error> {
        let app = transport::router(self.inner.clone());
        let server = axum::serve(self.listener, app).into_future();
        tokio::pin!(server);
        let mut check = tokio::time::interval(Duration::from_millis(5));
        let mut interrupted = false;
        #[cfg(unix)]
        let mut interrupt =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .map_err(Error::io)?;
        #[cfg(unix)]
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .map_err(Error::io)?;
        loop {
            tokio::select! {
                result = &mut server => {
                    self.inner.stop(true);
                    while self.inner.finished().is_none() {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                    return Err(result.err().map(Error::io).unwrap_or_else(||
                        Error::new("server_closed", "listener ended unexpectedly")));
                }
                _ = check.tick() => {
                    if let Some(result) = self.inner.finished() { return result; }
                }
                _ = interrupt.recv() => {
                    self.inner.stop(interrupted);
                    interrupted = true;
                }
                _ = terminate.recv() => { self.inner.stop(false); }
            }
        }
    }
}

impl Inner {
    fn create(&self, create: Create) -> Result<Detail, Error> {
        self.create_using(create, random_id)
    }
    fn create_using(
        &self,
        create: Create,
        mut candidate: impl FnMut() -> Result<String, Error>,
    ) -> Result<Detail, Error> {
        create.validate()?;
        let mut directory = self.directory.lock().unwrap();
        if directory.deadline.is_some() {
            return Err(Error::new("server_stopping", "server is stopping"));
        }
        let id = loop {
            let id = candidate()?;
            if !directory.entries.contains_key(&id)
                && std::fs::symlink_metadata(self.root.join(&id)).is_err()
            {
                break id;
            }
        };
        let entry = entry::Entry::new(id.clone(), self.root.join(&id), self.activity_bytes);
        let detail = entry.detail();
        directory.entries.insert(id, entry.clone());
        entry::start(entry, create);
        Ok(detail)
    }
    fn select(&self, selector: &str) -> Result<Arc<entry::Entry>, Error> {
        if !(8..=32).contains(&selector.len()) || !selector.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::new(
                "invalid_session_id",
                "expected 8 to 32 hexadecimal characters",
            ));
        }
        let selector = selector.to_ascii_lowercase();
        let directory = self.directory.lock().unwrap();
        let mut matches = directory
            .entries
            .iter()
            .filter(|(id, _)| id.starts_with(&selector));
        let entry = matches
            .next()
            .map(|(_, e)| e.clone())
            .ok_or_else(|| Error::new("session_not_found", &selector))?;
        if matches.next().is_some() {
            return Err(Error::new(
                "ambiguous_session_id",
                "prefix matches multiple sessions",
            ));
        }
        Ok(entry)
    }
    fn list(&self) -> Vec<Detail> {
        let mut entries: Vec<_> = self
            .directory
            .lock()
            .unwrap()
            .entries
            .values()
            .map(|e| e.detail())
            .collect();
        entries.sort_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
        entries
    }
    fn stop(&self, force: bool) {
        let mut directory = self.directory.lock().unwrap();
        directory
            .deadline
            .get_or_insert_with(|| Instant::now() + self.timeout);
        directory.forced |= force;
        for entry in directory.entries.values() {
            entry.stop();
            if directory.forced {
                entry.abort();
            }
        }
    }
    fn finished(&self) -> Option<Result<(), Error>> {
        let mut directory = self.directory.lock().unwrap();
        let deadline = directory.deadline?;
        if Instant::now() >= deadline {
            directory.forced = true;
            for entry in directory.entries.values() {
                entry.abort();
            }
        }
        if directory
            .entries
            .values()
            .any(|e| !e.done.load(Ordering::Acquire))
        {
            return None;
        }
        if directory.forced
            || directory
                .entries
                .values()
                .any(|e| e.detail().state == Lifecycle::Failed)
        {
            Some(Err(Error::new(
                "shutdown_incomplete",
                "at least one session failed or required forced cleanup",
            )))
        } else {
            Some(Ok(()))
        }
    }
}

fn random_id() -> Result<String, Error> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|e| Error::new("randomness_unavailable", e))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{report, session::launch};

    #[tokio::test]
    async fn rejects_non_loopback_before_creating_artifacts() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/server-tests")
            .join(random_id().unwrap());
        let result = Server::bind(Config {
            address: "0.0.0.0:0".parse().unwrap(),
            artifact_dir: root.clone(),
            shutdown_timeout: Duration::from_secs(30),
            activity_bytes: 4096,
        })
        .await;
        assert!(matches!(result, Err(Error::InvalidConfig { .. })));
        assert!(!root.exists());
    }

    struct Fixture(Arc<Inner>);

    impl std::ops::Deref for Fixture {
        type Target = Inner;
        fn deref(&self) -> &Inner {
            &self.0
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.0.stop(true);
            let deadline = Instant::now() + Duration::from_secs(10);
            while self.0.finished().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    fn inner() -> Fixture {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/server-tests")
            .join(random_id().unwrap());
        std::fs::create_dir_all(&root).unwrap();
        Fixture(Arc::new(Inner {
            directory: Mutex::new(Directory::default()),
            root,
            timeout: Duration::from_millis(200),
            activity_bytes: 4096,
        }))
    }
    fn create(mode: &str) -> Create {
        Create {
            launch: launch::Config {
                manifest_path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/process/Cargo.toml"),
                package: "process_fixture".into(),
                target: launch::Target::Binary {
                    name: "process_fixture".into(),
                },
                features: vec![],
                arguments: vec![mode.into()],
            },
            tick: Default::default(),
            report: report::Config {
                tracing_errors: false,
                output: "reports".into(),
                provider: report::provider::Config::Local,
            },
        }
    }
    fn wait(mut check: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(30);
        while !check() {
            assert!(Instant::now() < end, "server condition timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn intrinsic_rejection_collisions_and_environment_failures() {
        let inner = inner();
        let mut invalid = create("normal");
        invalid.launch.package.clear();
        assert!(inner.create(invalid).is_err());
        assert!(inner.list().is_empty());
        let occupied = "1".repeat(32);
        std::fs::create_dir(inner.root.join(&occupied)).unwrap();
        let mut candidates = [occupied.clone(), "2".repeat(32)].into_iter();
        let mut missing = create("normal");
        missing.launch.manifest_path = inner.root.join("missing.toml");
        let accepted = inner
            .create_using(missing, || Ok(candidates.next().unwrap()))
            .unwrap();
        assert_eq!(accepted.id, "2".repeat(32));
        assert_eq!(accepted.state, Lifecycle::Starting);
        let entry = inner.select("22222222").unwrap();
        wait(|| entry.done.load(Ordering::Acquire));
        assert_eq!(entry.detail().state, Lifecycle::Failed);
        assert!(inner.root.join(occupied).is_dir());
        let before = entry.detail();
        entry.stop();
        assert_eq!(entry.detail().state, before.state);
        assert!(inner.select("222").is_err());
        assert!(inner.select("33333333").is_err());
    }
    #[test]
    fn stopping_a_running_start_is_regular_and_cleans_up() {
        let inner = inner();
        let detail = inner.create(create("delayed")).unwrap();
        let entry = inner.select(&detail.id).unwrap();
        wait(|| detail.artifact_dir.join("pid").is_file());
        let pid: i32 = std::fs::read_to_string(detail.artifact_dir.join("pid"))
            .unwrap()
            .parse()
            .unwrap();
        entry.stop();
        assert_eq!(entry.detail().state, Lifecycle::Stopping);
        wait(|| entry.done.load(Ordering::Acquire));
        assert_eq!(entry.detail().state, Lifecycle::Ended);
        assert_ne!(unsafe { libc::kill(pid, 0) }, 0);
    }
    #[test]
    fn a_single_deadline_forces_all_hanging_sessions_independently() {
        let inner = inner();
        let entries: Vec<_> = (0..3)
            .map(|_| {
                let detail = inner.create(create("hang_shutdown")).unwrap();
                inner.select(&detail.id).unwrap()
            })
            .collect();
        wait(|| {
            let details: Vec<_> = entries.iter().map(|e| e.detail()).collect();
            assert!(
                details.iter().all(|d| d.state != Lifecycle::Failed),
                "start failed: {details:?}"
            );
            details.iter().all(|d| d.state == Lifecycle::Ready)
        });
        let start = Instant::now();
        inner.stop(false);
        let deadline = inner.directory.lock().unwrap().deadline;
        inner.stop(false);
        assert_eq!(inner.directory.lock().unwrap().deadline, deadline);
        let mut result = None;
        wait(|| {
            result = inner.finished();
            result.is_some()
        });
        assert!(result.unwrap().is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        for entry in entries {
            assert_eq!(entry.detail().state, Lifecycle::Failed);
            let pid: i32 = std::fs::read_to_string(entry.detail().artifact_dir.join("pid"))
                .unwrap()
                .parse()
                .unwrap();
            assert_ne!(unsafe { libc::kill(pid, 0) }, 0);
        }
    }
}
