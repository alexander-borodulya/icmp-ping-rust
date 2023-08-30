use std::{net::{Ipv4Addr, SocketAddrV4}, fmt::Display};

use socket2::SockAddr;
use tokio::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EchoStatus {
    Pending,
    Sent,
    Received,
    Elapsed,
    Failed,
}

impl Display for EchoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EchoStatus::Pending => write!(f, "Pending"),
            EchoStatus::Sent => write!(f, "Sent"),
            EchoStatus::Received => write!(f, "Received"),
            EchoStatus::Elapsed => write!(f, "Elapsed"),
            EchoStatus::Failed => write!(f, "Failed"),
        }
    }
}



#[derive(Debug, Clone, Copy)]
pub struct ConnectionHandle {
    pub destination: Ipv4Addr,
    pub seq: u16,
    pub identifier: u16,
    pub earlier: Instant,
    pub now: Instant,
    pub elapsed_micros: u128,
    pub status: EchoStatus,
}

impl ConnectionHandle {
    pub fn new(destination: Ipv4Addr, seq: u16, identifier: u16, earlier: Instant, now: Instant) -> Self {
        let elapsed_micros = now.duration_since(earlier).as_micros();
        Self { destination, seq, identifier, earlier, now, elapsed_micros, status: EchoStatus::Pending }
    }

    pub fn new_sent(destination: Ipv4Addr, seq: u16, identifier: u16, earlier: Instant, now: Instant) -> Self {
        let elapsed_micros = now.duration_since(earlier).as_micros();
        Self { destination, seq, identifier, earlier, now, elapsed_micros, status: EchoStatus::Sent }
    }

    pub fn new_received(destination: Ipv4Addr, seq: u16, identifier: u16, earlier: Instant, now: Instant) -> Self {
        let elapsed_micros = now.duration_since(earlier).as_micros();
        Self { destination, seq, identifier, earlier, now, elapsed_micros, status: EchoStatus::Received }
    }

    pub fn to_sent(self, now: Instant) -> Self {
        ConnectionHandle::new_sent(self.destination, self.seq, self.identifier, self.earlier, now)
    }

    pub fn to_received(self, now: Instant) -> Self {
        ConnectionHandle::new_received(self.destination, self.seq, self.identifier, self.earlier, now)
    }

    pub fn get_sock_addr(&self) -> SockAddr {
        let socket_addr_v4 = SocketAddrV4::new(self.destination, 0);
        SockAddr::from(socket_addr_v4)
    }

}

impl Display for ConnectionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{} - {}", self.destination, self.seq, self.elapsed_micros, self.status)
    }
}
