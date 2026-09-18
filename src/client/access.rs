use crate::{
    command::Command,
    server::protocol::{self, Cursor, Operation, Request, Response, Result as Reply},
};
use std::net::{SocketAddr, TcpStream};
use tungstenite::{Message, WebSocket, client::IntoClientRequest};

#[derive(Debug)]
pub enum Error {
    Connection(String),
    /// The server may have accepted a command. Never retry automatically.
    SubmissionUnknown(String),
    Protocol(String),
    Remote {
        code: String,
        message: String,
    },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

pub struct Management {
    address: SocketAddr,
}
impl Management {
    pub fn new(address: SocketAddr) -> Result<Self, Error> {
        if !address.ip().is_loopback() {
            return Err(Error::Connection("loopback address required".into()));
        }
        Ok(Self { address })
    }
    pub fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, Error> {
        let url = format!("http://{}{}{path}", self.address, protocol::PREFIX);
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .new_agent();
        let response = match method {
            "GET" => agent.get(&url).call(),
            "POST" => agent
                .post(&url)
                .send_json(body.unwrap_or(&serde_json::Value::Null)),
            _ => return Err(Error::Protocol("unsupported HTTP method".into())),
        }
        .map_err(|e| Error::Connection(e.to_string()))?;
        let value: serde_json::Value = response
            .into_body()
            .read_json()
            .map_err(|e| Error::Protocol(e.to_string()))?;
        if value["version"] != protocol::VERSION {
            return Err(Error::Protocol("unsupported outer version".into()));
        }
        if value.get("error").is_some() {
            return Err(Error::Remote {
                code: value["error"]["code"]
                    .as_str()
                    .unwrap_or("invalid_response")
                    .into(),
                message: value["error"]["message"]
                    .as_str()
                    .unwrap_or("invalid response")
                    .into(),
            });
        }
        value
            .get("data")
            .cloned()
            .ok_or_else(|| Error::Protocol("missing data".into()))
    }
    pub fn bind(&self, selector: &str) -> Result<Client, Error> {
        self.bind_with(selector, |_| Ok(()))
    }
    #[cfg(feature = "cli")]
    pub(crate) fn bind_interruptible(
        &self,
        selector: &str,
        interrupt: &Interrupt,
    ) -> Result<Client, Error> {
        self.bind_with(selector, |stream| interrupt.attach(stream))
    }
    fn bind_with(
        &self,
        selector: &str,
        attach: impl FnOnce(&TcpStream) -> Result<(), Error>,
    ) -> Result<Client, Error> {
        if !(8..=32).contains(&selector.len()) || !selector.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Protocol("expected 8 to 32 hex digits".into()));
        }
        let request = format!(
            "ws://{}{}/sessions/{selector}/connect",
            self.address,
            protocol::PREFIX
        )
        .into_client_request()
        .map_err(|e| Error::Protocol(e.to_string()))?;
        let stream =
            TcpStream::connect_timeout(&self.address, std::time::Duration::from_millis(500))
                .map_err(|e| Error::Connection(e.to_string()))?;
        attach(&stream)?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(35)))
            .map_err(|e| Error::Connection(e.to_string()))?;
        stream
            .set_write_timeout(Some(std::time::Duration::from_secs(35)))
            .map_err(|e| Error::Connection(e.to_string()))?;
        let (socket, _) = tungstenite::client(request, stream).map_err(|e| match e {
            tungstenite::HandshakeError::Failure(tungstenite::Error::Http(response)) => {
                Error::Remote {
                    code: "binding_rejected".into(),
                    message: format!("server rejected session binding: {}", response.status()),
                }
            }
            e => Error::Connection(e.to_string()),
        })?;
        Ok(Client { socket, call: 0 })
    }
}

/// Wakes blocking socket IO when the owning interactive client exits.
#[cfg(feature = "cli")]
#[derive(Default)]
pub(crate) struct Interrupt(std::sync::Mutex<(bool, Option<TcpStream>)>);
#[cfg(feature = "cli")]
impl Interrupt {
    fn attach(&self, stream: &TcpStream) -> Result<(), Error> {
        let mut state = self.0.lock().unwrap();
        if state.0 {
            return Err(Error::Connection("client closed".into()));
        }
        state.1 = Some(
            stream
                .try_clone()
                .map_err(|e| Error::Connection(e.to_string()))?,
        );
        Ok(())
    }
    pub fn cancel(&self) {
        let mut state = self.0.lock().unwrap();
        state.0 = true;
        if let Some(stream) = state.1.take() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }
}

pub struct Client {
    socket: WebSocket<TcpStream>,
    call: u64,
}
impl Client {
    pub fn submit(&mut self, command: Command) -> Result<Reply, Error> {
        self.exchange(Operation::Submit { command })
    }
    pub fn poll(&mut self, cursor: Option<Cursor>, wait_ms: u64) -> Result<Reply, Error> {
        self.exchange(Operation::Poll { cursor, wait_ms })
    }
    pub fn snapshot(&mut self) -> Result<protocol::Snapshot, Error> {
        match self.exchange(Operation::Snapshot {})? {
            Reply::Snapshot { snapshot } => Ok(snapshot),
            Reply::Error { code, message } => Err(Error::Remote { code, message }),
            _ => unreachable!("exchange validates snapshot responses"),
        }
    }
    fn exchange(&mut self, operation: Operation) -> Result<Reply, Error> {
        self.call = self
            .call
            .checked_add(1)
            .ok_or_else(|| Error::Protocol("client call counter exhausted".into()))?;
        let expected = match &operation {
            Operation::Submit { command } => Some(command.name()),
            Operation::Poll { .. } | Operation::Snapshot {} => None,
        };
        let snapshot = matches!(operation, Operation::Snapshot {});
        let submitting = expected.is_some();
        let transport = |e: tungstenite::Error| {
            if submitting {
                Error::SubmissionUnknown(e.to_string())
            } else {
                Error::Connection(e.to_string())
            }
        };
        let request = Request {
            version: protocol::VERSION,
            call: self.call,
            operation,
        };
        self.socket
            .send(Message::Text(
                serde_json::to_string(&request).unwrap().into(),
            ))
            .map_err(transport)?;
        loop {
            match self.socket.read().map_err(transport)? {
                Message::Text(text) => {
                    let response: Response = serde_json::from_str(&text).map_err(|e| {
                        if submitting {
                            Error::SubmissionUnknown(e.to_string())
                        } else {
                            Error::Protocol(e.to_string())
                        }
                    })?;
                    if response.version != protocol::VERSION || response.call != self.call {
                        return Err(if submitting {
                            Error::SubmissionUnknown("uncorrelated response".into())
                        } else {
                            Error::Protocol("uncorrelated response".into())
                        });
                    }
                    let valid = match (&response.result, expected) {
                        (Reply::Error { .. }, _) => true,
                        (
                            Reply::Pending {
                                request_id,
                                command,
                            },
                            Some(expected),
                        ) => {
                            *request_id > 0
                                && command == expected
                                && (*request_id == u64::MAX) == (expected == "shutdown")
                        }
                        (Reply::Snapshot { .. }, None) => snapshot,
                        (Reply::Activity { .. } | Reply::Gap { .. }, None) => !snapshot,
                        _ => false,
                    };
                    if !valid {
                        return Err(if submitting {
                            Error::SubmissionUnknown("response does not match submission".into())
                        } else {
                            Error::Protocol("response does not match poll".into())
                        });
                    }
                    return Ok(response.result);
                }
                Message::Close(_) => {
                    return Err(if submitting {
                        Error::SubmissionUnknown("connection closed".into())
                    } else {
                        Error::Connection("connection closed".into())
                    });
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_only_needs_a_loopback_address() {
        assert!(Management::new("127.0.0.1:4100".parse().unwrap()).is_ok());
        assert!(Management::new("[::1]:4100".parse().unwrap()).is_ok());
        assert!(Management::new("192.0.2.1:4100".parse().unwrap()).is_err());
    }

    #[test]
    fn disconnect_or_invalid_confirmation_leaves_submission_unknown() {
        for reply in [
            None,
            Some(
                r#"{"version":1,"call":1,"result":{"kind":"pending","request_id":0,"command":"tick.warp.stop"}}"#,
            ),
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let peer = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut socket = tungstenite::accept(stream).unwrap();
                let request = socket.read().unwrap();
                assert!(matches!(request, Message::Text(_)));
                if let Some(reply) = reply {
                    socket.send(Message::Text(reply.into())).unwrap();
                }
                socket.close(None).unwrap();
            });
            let stream = TcpStream::connect(address).unwrap();
            let (socket, _) = tungstenite::client(format!("ws://{address}/"), stream).unwrap();
            let mut client = Client { socket, call: 0 };
            assert!(matches!(
                client.submit(crate::command::tick::warp::Stop {}.into()),
                Err(Error::SubmissionUnknown(_))
            ));
            assert_eq!(client.call, 1);
            peer.join().unwrap();
        }
    }
}
