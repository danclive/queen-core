use std::collections::HashMap;
use std::io::ErrorKind::{WouldBlock, ConnectionAborted};
use std::cell::Cell;
use std::sync::mpsc::TryRecvError;
use std::error::Error;

use queen_io::*;
use queen_io::channel::{self, Receiver, Sender};
use queen_io::tcp::TcpListener;
use queen_io::tcp::TcpStream;

use wire_protocol::Message;

use super::connection::Connection;

const SOCKET: Token = Token(0);
const CHANNEL: Token = Token(1);

pub struct Service {
    poll: Poll,
    events: Events,
    conns: HashMap<Token, Connection>,
    token_counter: Cell<usize>,
    rx_in: Receiver<ServiceMessage>,
    tx_out: Sender<ServiceMessage>,
    socket: Option<TcpListener>,
    run: bool
}

#[derive(Debug)]
pub enum ServiceMessage {
    Message(usize, Message),
    Command(Command)
}

#[derive(Debug)]
pub enum Command {
    Listen {
        id: usize,
        addr: String
    },
    LinkTo {
        id: usize,
        addr: String
    },
    Shoutdown,
    CloseConn {
        id: usize
    },
    ListenReply {
        id: usize,
        addr: String,
        success: bool,
        msg: String
    },
    LinkToReply {
        id: usize,
        token: usize,
        addr: String,
        success: bool,
        msg: String
    }
}

impl Service {
    pub fn new() -> io::Result<(Service, Sender<ServiceMessage>, Receiver<ServiceMessage>)> {
        let (tx_in, rx_in) = channel::channel()?;
        let (tx_out, rx_out) = channel::channel()?;

        let service = Service {
            poll: Poll::new()?,
            events: Events::with_capacity(256),
            conns: HashMap::with_capacity(128),
            token_counter: Cell::new(8),
            rx_in: rx_in,
            tx_out: tx_out,
            socket: None,
            run: true

        };

        service.poll.register(&service.rx_in, CHANNEL, Ready::readable(), PollOpt::edge() | PollOpt::oneshot())?;

        Ok((service, tx_in, rx_out))
    }

    pub fn with_channel(tx_out: Sender<ServiceMessage>, rx_in: Receiver<ServiceMessage>) -> io::Result<Service> {
        let service = Service {
            poll: Poll::new()?,
            events: Events::with_capacity(256),
            conns: HashMap::with_capacity(128),
            token_counter: Cell::new(8),
            rx_in: rx_in,
            tx_out: tx_out,
            socket: None,
            run: true

        };

        service.poll.register(&service.rx_in, CHANNEL, Ready::readable(), PollOpt::edge() | PollOpt::oneshot())?;

        Ok(service)
    }

    fn next_token(&self) -> Token {
        let next_token = self.token_counter.get() + 1;

        Token(self.token_counter.replace(next_token))
    }

    fn channel_process(&mut self) -> io::Result<()> {
        loop {
            let msg = match self.rx_in.try_recv() {
                Ok(msg) => msg,
                Err(err) => {
                    if let TryRecvError::Empty = err {
                        break;
                    }

                    return Err(io::Error::new(ConnectionAborted, err).into())
                }
            };

            match msg {
                ServiceMessage::Message(id, message) => {
                    if let Some(conn) = self.conns.get_mut(&Token(id)) {
                        conn.recv_message(&self.poll, message)?;
                    }
                }
                ServiceMessage::Command(command) => {
                    self.service_command(command)?;
                }
            }
        }

        self.poll.reregister(&self.rx_in, CHANNEL, Ready::readable(), PollOpt::edge() | PollOpt::oneshot())?;

        Ok(())
    }

    fn service_command(&mut self, command: Command) -> io::Result<()> {
        match command {
            Command::Listen { id, addr } => {
                let socket = match TcpListener::bind(addr.clone()) {
                    Ok(socket) => socket,
                    Err(err) => {
                        let _= self.tx_out.send(
                            ServiceMessage::Command(
                                Command::ListenReply {
                                    id: id,
                                    addr: addr,
                                    success: false,
                                    msg: err.description().to_owned()
                                }
                            )
                        );

                        return Ok(())
                    }
                };

                self.poll.register(&socket, SOCKET, Ready::readable(), PollOpt::edge() | PollOpt::oneshot())?;

                self.socket = Some(socket);

                let _ = self.tx_out.send(
                    ServiceMessage::Command(
                        Command::ListenReply {
                            id: id,
                            addr: addr,
                            success: true,
                            msg: String::default()
                        }
                    )
                );
            }
            Command::LinkTo { id, addr } => {
                let socket = match TcpStream::connect(addr.clone()) {
                    Ok(socket) => socket,
                    Err(err) => {
                        let _= self.tx_out.send(
                            ServiceMessage::Command(
                                Command::LinkToReply {
                                    id: id,
                                    token: 0,
                                    addr: addr,
                                    success: false,
                                    msg: err.description().to_owned()
                                }
                            )
                        );

                        return Ok(())
                    }
                };

                let token = self.next_token();

                let conn = Connection::new(socket, token)?;
                conn.register_insterest(&self.poll);

                self.conns.insert(token, conn);

                let _ = self.tx_out.send(
                    ServiceMessage::Command(
                        Command::LinkToReply {
                            id: id,
                            token: token.into(),
                            addr: addr,
                            success: true,
                            msg: String::default()
                        }
                    )
                );
            }
            Command::Shoutdown => {
                self.run = false
            }
            Command::CloseConn { id } => {
                self.remove_connent(Token(id));
            }
            _ => {
                unimplemented!()
            }
        }

        Ok(())
    }

    fn connect_process(&mut self, event: Event, token: Token) -> io::Result<()> {
        if event.readiness().is_hup() || event.readiness().is_error() {
            self.remove_connent(token);

            let _ = self.tx_out.send(
                ServiceMessage::Command(
                    Command::CloseConn { id: token.into() }
                )
            );

            return Ok(())
        }

        let mut close = false;

        if event.readiness().is_readable() {
            if let Some(conn) = self.conns.get_mut(&token) {
                close = conn.reader(&self.poll, &self.tx_out).is_err();
            }
        }

        if event.readiness().is_writable() {
            if let Some(conn) = self.conns.get_mut(&token) {
                close = conn.writer(&self.poll).is_err();
            }
        }

        if close {
            self.remove_connent(token);

            let _ = self.tx_out.send(
                ServiceMessage::Command(
                    Command::CloseConn { id: token.into() }
                )
            );
        }

        Ok(())
    }

    fn remove_connent(&mut self, token: Token) {
        if let Some(conn) = self.conns.remove(&token) {
            conn.deregister(&self.poll).unwrap();
        }
    }

    fn dispatch(&mut self, event: Event) -> io::Result<()> {
        match event.token() {
            SOCKET => {
                match self.socket {
                    Some(ref socket) => {
                        loop {
                            let socket = match socket.accept().map(|s| s.0) {
                                Ok(socket) => socket,
                                Err(err) => {
                                    if let WouldBlock = err.kind() {
                                        break;
                                    } else {
                                        return Err(err)
                                    }
                                }
                            };

                            let token = self.next_token();

                            let conn = Connection::new(socket, token)?;
                            conn.register_insterest(&self.poll);

                            self.conns.insert(
                                token,
                                conn
                            );
                        }

                        self.poll.reregister(socket, SOCKET, Ready::readable(), PollOpt::edge() | PollOpt::oneshot())?;
                    }
                    None => ()
                }

                Ok(())
            }
            CHANNEL => self.channel_process(),
            token => self.connect_process(event, token)
        }
    }

    fn run_once(&mut self) -> io::Result<()> {
        let size = self.poll.poll(&mut self.events, None)?;

        for i in 0..size {
            let event = self.events.get(i).unwrap();
            self.dispatch(event)?;
        }

        Ok(())
    }

    pub fn run(&mut self) -> io::Result<()> {
        while self.run {
            self.run_once()?;
        }

        Ok(())
    }
}
