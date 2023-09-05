use socket2::SockAddr;
use std::{
    fmt::Display,
    net::{Ipv4Addr, SocketAddrV4},
};
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
pub struct EchoHandle {
    pub destination_ipv4: Ipv4Addr,
    pub destination: SockAddr,
    pub seq: u16,
    pub identifier: u16,
    pub earlier: Instant,
    pub now: Instant,
    pub elapsed_micros: u128,
    pub status: EchoStatus,
}

impl EchoHandle {
    pub fn new(
        destination_ipv4: Ipv4Addr,
        seq: u16,
        identifier: u16,
        earlier: Instant,
        now: Instant,
        status: EchoStatus,
    ) -> Self {
        let socket_addr_v4 = SocketAddrV4::new(destination_ipv4, 0);
        let destination = SockAddr::from(socket_addr_v4);
        let elapsed_micros = now.duration_since(earlier).as_micros();
        Self {
            destination_ipv4,
            destination,
            seq,
            identifier,
            earlier,
            now,
            elapsed_micros,
            status,
        }
    }

    pub fn to_received(self, now: Instant) -> Self {
        Self::new(
            self.destination_ipv4,
            self.seq,
            self.identifier,
            self.earlier,
            now,
            EchoStatus::Received,
        )
    }

    pub fn to_output_format(&self) -> String {
        format!("{},{},{}", self.destination_ipv4, self.seq, {
            match self.status {
                EchoStatus::Pending | EchoStatus::Sent | EchoStatus::Failed => {
                    format!("{} - {}", self.elapsed_micros, self.status)
                }
                EchoStatus::Received => self.elapsed_micros.to_string(),
                EchoStatus::Elapsed => "timeout".to_string(),
            }
        })
    }
}

impl Display for EchoHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{},{},{} - {}",
            self.destination_ipv4, self.seq, self.elapsed_micros, self.status
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_to_output_format() {
        let handle = EchoHandle::new(
            Ipv4Addr::new(127, 0, 0, 1),
            1,
            1,
            Instant::now(),
            Instant::now(),
            EchoStatus::Pending,
        );
        assert_eq!(handle.to_output_format(), "127.0.0.1,1,0 - Pending");

        let handle = EchoHandle::new(
            Ipv4Addr::new(127, 0, 0, 1),
            2,
            1,
            Instant::now(),
            Instant::now(),
            EchoStatus::Sent,
        );
        assert_eq!(handle.to_output_format(), "127.0.0.1,2,0 - Sent");

        let handle = EchoHandle::new(
            Ipv4Addr::new(127, 0, 0, 1),
            3,
            1,
            Instant::now(),
            Instant::now()
                .checked_add(Duration::from_micros(650))
                .expect("Failed to add 650 micros"),
            EchoStatus::Received,
        );
        assert_eq!(handle.to_output_format(), "127.0.0.1,3,650");

        let handle = EchoHandle::new(
            Ipv4Addr::new(127, 0, 0, 1),
            4,
            1,
            Instant::now(),
            Instant::now(), // TODO: For now, validate status only, without testing elapsed time.
            EchoStatus::Elapsed,
        );
        assert_eq!(handle.to_output_format(), "127.0.0.1,4,timeout");

        let handle = EchoHandle::new(
            Ipv4Addr::new(127, 0, 0, 1),
            5,
            1,
            Instant::now(),
            Instant::now(),
            EchoStatus::Failed,
        );
        assert_eq!(handle.to_output_format(), "127.0.0.1,5,0 - Failed");
    }
}
