use super::{Error, Outcome, Script, execution};
use crate::client::{Interrupt, Management};
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// CLI owns the connection, cancellation and worker lifetime; the library runner
/// remains usable without signals or a Tokio runtime.
pub(crate) fn run(
    address: SocketAddr,
    selector: String,
    script: Script,
) -> Result<Outcome, super::super::Error> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let cancel = Arc::new(AtomicBool::new(false));
            let interrupt = Arc::new(Interrupt::default());
            let worker_cancel = cancel.clone();
            let worker_interrupt = interrupt.clone();
            let (sender, mut receiver) = tokio::sync::oneshot::channel();
            let worker = std::thread::Builder::new()
                .name("script-client".into())
                .spawn(move || {
                    let result = Management::new(address)
                        .and_then(|management| {
                            management.bind_interruptible(&selector, &worker_interrupt)
                        })
                        .map_err(|e| Error::Client {
                            message: e.to_string(),
                        })
                        .and_then(|mut client| {
                            execution::run(&mut client, &script, &worker_cancel)
                        });
                    let _ = sender.send(result);
                })?;
            let result = tokio::select! {
                result = &mut receiver => result,
                signal = tokio::signal::ctrl_c() => {
                    cancel.store(true, Ordering::Release);
                    interrupt.cancel();
                    // Even signal setup failure must close and join the worker.
                    let result = receiver.await;
                    if let Err(error) = signal {
                        let _ = worker.join();
                        return Err(error.into());
                    }
                    result
                }
            };
            worker.join().map_err(|_| "script client worker panicked")?;
            Ok(result.map_err(|_| "script client worker disappeared")??)
        })
}
