use crate::echo_handle::EchoHandle;
use pnet::packet::{
    icmp::{
        echo_reply::EchoReplyPacket,
        echo_request::MutableEchoRequestPacket,
        IcmpCode,
        IcmpPacket,
        IcmpTypes,
    },
    ipv4::Ipv4Packet,
    Packet,
};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::{
    io::ErrorKind,
    mem::MaybeUninit,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetError {
    #[error("Unexpected error occured: {0}")]
    UnexpetedError(String),

    #[error("Socket2 API error: {0}")]
    Socket2Error(String),

    #[error("Failed to create socket: {0}")]
    SocketCreationError(String),

    #[error("Failed to bind socket: {0}")]
    SocketBindError(String),

    #[error("Send request error")]
    IcpmSendError(String),

    #[error("Respond reply error")]
    IcmpReplyError(String),

    #[error("Ping request error")]
    PingError(String),

    #[error("Received wrong sequence number: {0}")]
    WrongSequenceNumber(u16),
}

type Result<T, E = NetError> = std::result::Result<T, E>;

pub struct PingNet;

pub type RecvRespond = (Ipv4Addr, u16);

const ECHO_IPV4_PACKET_BUFFER_SIZE: usize =
    EchoReplyPacket::minimum_packet_size() + Ipv4Packet::minimum_packet_size();

impl PingNet {
    pub fn create_socket() -> Result<Arc<Socket>, NetError> {
        let socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))
            .map_err(|e| NetError::SocketCreationError(e.to_string()))?;
        let ipv4_unspecified: SockAddr =
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 80).into();
        socket.bind(&ipv4_unspecified).map_err(|e| NetError::SocketBindError(e.to_string()))?;
        Ok(Arc::new(socket))
    }

    pub fn send_ping_request(socket: Arc<Socket>, connection_handle: EchoHandle) -> Result<()> {
        let addr = &connection_handle.destination;
        let seq_num = connection_handle.seq;
        let identifier = connection_handle.identifier;

        log::debug!("send_ping_request: seq_num: {} - started", seq_num);

        let mut buf = [0u8; MutableEchoRequestPacket::minimum_packet_size()];
        let packet = PingNet::create_icmp_request_packet(&mut buf, seq_num, identifier);
        socket
            .send_to(packet.packet(), addr)
            .map_err(|e| NetError::IcpmSendError(e.to_string()))?;

        log::debug!("send_ping_request: seq_num: {} - finished", seq_num);

        Ok(())
    }

    pub async fn recv_ping_respond(socket: Arc<Socket>) -> Option<RecvRespond> {
        log::debug!("recv_ping_respond - started");

        let mut buf: [MaybeUninit<u8>; ECHO_IPV4_PACKET_BUFFER_SIZE] =
            [MaybeUninit::<u8>::uninit(); ECHO_IPV4_PACKET_BUFFER_SIZE];
        let (size, _sock_addr) = PingNet::read_socket(&socket, &mut buf).await;

        log::debug!(
            "recv_ping_respond: size: {}, _sock_addr: {:?}",
            size,
            _sock_addr.as_socket_ipv4()
        );

        if size >= ECHO_IPV4_PACKET_BUFFER_SIZE {
            let temp: Vec<u8> = buf.iter_mut().map(|b| unsafe { b.assume_init() }).collect();
            let o: [u8; ECHO_IPV4_PACKET_BUFFER_SIZE] = match temp.try_into() {
                Ok(arr) => arr,
                Err(_e) => {
                    log::warn!("recv_ping_respond: Vec<u8>.try_into failed, size: {}, ECHO_IPV4_PACKET_BUFFER_SIZE: {} - finished with None", size, ECHO_IPV4_PACKET_BUFFER_SIZE);

                    // TODO: NetError::RecvBufferInitFailed
                    return None;
                }
            };

            let ipv4_packet = Ipv4Packet::new(&o)?;

            if ipv4_packet.get_next_level_protocol()
                == pnet::packet::ip::IpNextHeaderProtocols::Icmp
            {
                let icmp_packet = EchoReplyPacket::new(ipv4_packet.payload())?;

                let icmp_type = icmp_packet.get_icmp_type();
                let source = ipv4_packet.get_source();
                let destination = ipv4_packet.get_destination();

                if icmp_type == IcmpTypes::EchoReply
                    || (icmp_type == IcmpTypes::EchoRequest
                        && source == Ipv4Addr::new(127, 0, 0, 1)
                        && destination == Ipv4Addr::new(127, 0, 0, 1))
                {
                    let ipv4_addr_recv = ipv4_packet.get_source();
                    let seq_num_recv = icmp_packet.get_sequence_number();
                    let idetifier = icmp_packet.get_identifier();

                    log::debug!(
                        "recv_ping_respond: ip: {}, seq_num_recv: {}, idetifier: {}",
                        ipv4_addr_recv,
                        seq_num_recv,
                        idetifier
                    );

                    return Some((ipv4_addr_recv, seq_num_recv));
                } else {
                    log::warn!(
                        "recv_ping_respond: Unexpected ICMP type: {:?}",
                        icmp_packet.get_icmp_type()
                    );
                }
            }
        }

        log::warn!(
            "recv_ping_respond: size={}, ECHO_IPV4_PACKET_BUFFER_SIZE={}, finished with None",
            size,
            ECHO_IPV4_PACKET_BUFFER_SIZE
        );

        // TODO: NetError::RecvNotEchoReply
        None
    }

    ///
    /// Pre-defined templates
    ///

    /// Creates ICMP request
    pub fn create_icmp_request_packet(
        buf: &mut [u8; MutableEchoRequestPacket::minimum_packet_size()],
        seq: u16,
        identifier: u16,
    ) -> MutableEchoRequestPacket {
        let mut packet = MutableEchoRequestPacket::new(buf).unwrap();

        packet.set_icmp_type(IcmpTypes::EchoRequest);
        packet.set_icmp_code(IcmpCode(0));
        packet.set_sequence_number(seq);
        packet.set_identifier(identifier);

        let checksum = pnet::packet::icmp::checksum(&IcmpPacket::new(packet.packet()).unwrap());
        packet.set_checksum(checksum);

        packet
    }

    /// Reads incoming bytes
    pub async fn read_socket(sock: &Arc<Socket>, buf: &mut [MaybeUninit<u8>]) -> (usize, SockAddr) {
        loop {
            match sock.recv_from(buf) {
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    } else {
                        panic!("Something went wrong while reading the socket");
                    }
                }
                Ok(res) => return res,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::echo_handle::EchoStatus;
    use tokio::time::Instant;

    #[tokio::test]
    async fn test_send_recv_icpm_echo() {
        let destination_ipv4 = Ipv4Addr::new(127, 0, 0, 1);
        let seq = 1;
        let identifier = 1;
        let earlier = Instant::now();
        let now = Instant::now();
        let status = EchoStatus::Pending;

        let connection_handle =
            EchoHandle::new(destination_ipv4, seq, identifier, earlier, now, status);

        let socket = PingNet::create_socket().expect("create_socket failed");

        let send_result = PingNet::send_ping_request(Arc::clone(&socket), connection_handle)
            .expect("send_ping_request failed");
        assert_eq!(send_result, ());

        let recv_result = PingNet::recv_ping_respond(Arc::clone(&socket))
            .await
            .expect("recv_ping_respond failed");
        assert_eq!(recv_result, (destination_ipv4, seq));
    }
}
