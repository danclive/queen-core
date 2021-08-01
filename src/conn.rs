use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

use queen::{Wire, Port, Socket};
use queen::error::{Result, Error, RecvError, SendError};
use queen::net::{NsonCodec, CryptoOptions};
use queen::nson::{Message, MessageId};

pub struct Conn {
    connector: Box<dyn Connector>,
    wire: Option<Wire<Message>>
}

pub trait Connector: Send + 'static {
    fn connect(&self) -> Result<Wire<Message>>;
}

impl Conn {
    pub fn new(connector: impl Connector) -> Self {
        let conn = Conn {
            connector: Box::new(connector),
            wire: None
        };

        conn
    }

    pub fn connected(&self) -> bool {
        !self.wire.is_none()
    }

    pub fn connect(&mut self) -> Result<()> {
        log::debug!("conn::connect");
        let wire = self.connector.connect()?;

        self.wire = Some(wire);

        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.wire = None
    }

    pub fn fd(&mut self) -> Result<RawFd> {
        if !self.connected() {
            return Err(Error::Disconnected("conn::fd".to_string()))
        }

        let fd = self.wire.as_ref().unwrap().as_raw_fd();

        Ok(fd)
    }

    pub fn send(&mut self, message: Message) -> Result<()> {
        if !self.connected() {
            return Err(Error::Disconnected("conn::send".to_string()))
        }

        match self.wire.as_ref().unwrap().send(message) {
            Ok(_) => (),
            Err(err) => {
                match err {
                    SendError::Disconnected(_) => {
                        self.wire = None
                    }
                    SendError::Full(_) => {
                        return Err(Error::Full("wire.send".to_string()))
                    }
                }
            }
        }

        Ok(())
    }

    pub fn recv(&mut self) -> Result<Option<Message>> {
        if !self.connected() {
            return Err(Error::Disconnected("conn::recv".to_string()))
        }

        match self.wire.as_ref().unwrap().recv() {
            Ok(message) => {
                return Ok(Some(message))
            }
            Err(err) => {
                if matches!(err, RecvError::Disconnected) {
                    self.wire = None
                }
            }
        }

        Ok(None)
    }

    pub fn wait(&mut self, timeout: Option<Duration>) -> Result<Option<Message>> {
        if !self.connected() {
            return Err(Error::Disconnected("conn::wait".to_string()))
        }

        match self.wire.as_ref().unwrap().wait(timeout) {
            Ok(message) => {
                return Ok(Some(message))
            }
            Err(err) => {
                if matches!(err, RecvError::Disconnected) {
                    self.wire = None
                }
            }
        }

        Ok(None)
    }
}

pub struct PortConnector {
    pub port: Port<NsonCodec>,
    pub addr: SocketAddr,
    pub slot_id: MessageId,
    pub root: bool,
    pub attr: Message,
    pub crypto_options: Option<CryptoOptions>,
}

impl Connector for PortConnector {
    fn connect(&self) -> Result<Wire<Message>> {
        self.port.connect(
            self.addr.clone(),
            self.slot_id,
            self.root,
            self.attr.clone(),
            self.crypto_options.clone(),
            None
        )
    }
}

pub struct SocketConnector {
    pub socket: Socket,
    pub slot_id: MessageId,
    pub root: bool,
    pub attr: Message,
}

impl Connector for SocketConnector {
    fn connect(&self) -> Result<Wire<Message>> {
        self.socket.connect(
            self.slot_id,
            self.root,
            self.attr.clone(),
            None,
            None
        )
    }
}
