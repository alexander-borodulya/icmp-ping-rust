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

#[derive(Debug, Clone)]
pub struct ConnectionHandle {
    pub destination_ipv4: Ipv4Addr,
    pub destination: SockAddr,
    pub seq: u16,
    pub identifier: u16,
    pub earlier: Instant,
    pub now: Instant,
    pub elapsed_micros: u128,
    pub status: EchoStatus,
}

impl ConnectionHandle {
    pub fn new(destination_ipv4: Ipv4Addr, seq: u16, identifier: u16, earlier: Instant, now: Instant, status: EchoStatus) -> Self {
        let socket_addr_v4 = SocketAddrV4::new(destination_ipv4, 0);
        let destination = SockAddr::from(socket_addr_v4);
        let elapsed_micros = now.duration_since(earlier).as_micros();
        Self { destination_ipv4, destination, seq, identifier, earlier, now, elapsed_micros, status }
    }

    pub fn to_received(self, now: Instant) -> Self {
        Self::new(self.destination_ipv4, self.seq, self.identifier, self.earlier, now, EchoStatus::Received)
    }
}

impl Display for ConnectionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{} - {}", self.destination_ipv4, self.seq, self.elapsed_micros, self.status)
    }
}
