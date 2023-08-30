use std::{net::{Ipv4Addr, SocketAddr, IpAddr}, sync::Arc, mem::MaybeUninit, io::ErrorKind, time::Duration};
use crate::connection_handle::ConnectionHandle;
use pnet::packet::{icmp::{echo_request::MutableEchoRequestPacket, echo_reply::EchoReplyPacket, IcmpTypes, IcmpCode, IcmpPacket}, Packet, ipv4::Ipv4Packet};
use socket2::{Socket, SockAddr, Domain, Type, Protocol};
use thiserror::Error;
use tokio::{time::Instant, task::JoinHandle};

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
pub type ConnectionHandleResult = Result<ConnectionHandle, NetError>;
pub type ConnectionJoinHandle = JoinHandle<Result<ConnectionHandle, NetError>>;

pub type RecvRespond = (Ipv4Addr, u16);

impl PingNet {
    pub fn create_socket() -> Result<Arc<Socket>, NetError> {
        let sock_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0).into();
        let socket = Socket::new(
            Domain::IPV4,
            Type::DGRAM,
            Some(Protocol::ICMPV4),
        ).map_err(|e| NetError::SocketCreationError(e.to_string()))?;
        socket.bind(&sock_addr).map_err(|e| NetError::SocketBindError(e.to_string()))?;
        // let r = socket.listen(128).unwrap();
        Ok(Arc::new(socket))
    }
    
    pub fn spawn_icmp_echo(socket: Arc<Socket>, destination: SockAddr, seq_num: u16, identifier: u16) -> ConnectionJoinHandle {
        let echo_jh = tokio::task::spawn(async move {
            let socket = Arc::clone(&socket);
            log::debug!("\tspawn_icmp_echo {} - started...", seq_num);
            let earlier = Instant::now();
            let (destination, seq_num_recv) = PingNet::icmp_echo(socket, &destination, seq_num, identifier).await?;
            log::debug!("\tspawn_icmp_echo: seq_num: {:?}, seq_num_recv: {:?} - finished...", seq_num, seq_num_recv);
            Ok(ConnectionHandle::new(destination, seq_num_recv, identifier, earlier, Instant::now()))
        });
        echo_jh
    }

    /// Join request and response into a single functiont
    pub async fn icmp_echo(socket: Arc<Socket>, destination: &SockAddr, seq_num: u16, identifier: u16) -> Result<RecvRespond, NetError> {

        let _bytes_sent = PingNet::send_ping_request(Arc::clone(&socket), destination, seq_num, identifier)?;

        // #[cfg(debug_assertions)]
        #[cfg(feature = "ENABLE_EMULATE_PAYLOAD")]
        crate::utils::emulate_payload_delay(seq_num).await;

        let recv_result = PingNet::recv_ping_respond(socket).await;
        recv_result.ok_or(NetError::IcmpReplyError("Failed to receive ICMP response".to_string()))
    }
    
    pub fn send_ping_request(socket: Arc<Socket>, destination: &SockAddr, seq_num: u16, identifier: u16) -> Result<usize, NetError> {
        log::trace!("\t\tsend_ping_request: {:?}, {:?} - started", destination.as_socket_ipv4(), seq_num);
        let mut buf = [0u8; MutableEchoRequestPacket::minimum_packet_size()];
        let packet = PingNet::create_icmp_request_packet(&mut buf, seq_num, identifier);
        let r = Ok(socket.send_to(&packet.packet(), destination).map_err(|e| NetError::IcpmSendError(e.to_string()))?);
        log::trace!("\t\tsend_ping_request: {:?}, {:?} - finished: r: {:?}", destination.as_socket_ipv4(), seq_num, r);
        r
    }
    
    pub fn send_ping_request_2(socket: Arc<Socket>, connection_handle: ConnectionHandle) -> Result<ConnectionHandle> {
        let addr = connection_handle.get_sock_addr();
        let seq_num = connection_handle.seq;
        let identifier = connection_handle.identifier;

        log::trace!("send_ping_request: seq_num: {}: [ {} ] - started", seq_num, connection_handle);

        let mut buf = [0u8; MutableEchoRequestPacket::minimum_packet_size()];
        let packet = PingNet::create_icmp_request_packet(&mut buf, seq_num, identifier);
        socket.send_to(&packet.packet(), &addr).map_err(|e| NetError::IcpmSendError(e.to_string()))?;
        let now = Instant::now();

        let connection_handle = connection_handle.to_sent(now);

        log::trace!("send_ping_request: seq_num: {}: [ {} ] - finished", seq_num, connection_handle);

        Ok(connection_handle)
    }

    pub async fn recv_ping_respond(socket: Arc<Socket>) -> Option<RecvRespond> {
        log::debug!("recv_ping_respond - started");

        const IPV4_PACKET_BUFFER_SIZE: usize = EchoReplyPacket::minimum_packet_size() + Ipv4Packet::minimum_packet_size();

        let mut buf: [MaybeUninit<u8>; IPV4_PACKET_BUFFER_SIZE] = [MaybeUninit::<u8>::uninit(); IPV4_PACKET_BUFFER_SIZE];
        let (size, _sock_addr) = PingNet::read_socket(&socket, &mut buf).await;

        if size > Ipv4Packet::minimum_packet_size() {
            let temp: Vec<u8> = buf.iter_mut().map(|b| unsafe { b.assume_init() }).collect();
            let o: [u8; IPV4_PACKET_BUFFER_SIZE] = match temp.try_into() {
                Ok(arr) => arr,
                Err(_e) => {
                    log::warn!("recv_ping_respond: Vec<u8>.try_into failed, size: {}, IPV4_PACKET_BUFFER_SIZE: {} - finished with None", size, IPV4_PACKET_BUFFER_SIZE);
                    return None // TODO: NetError::RecvBufferInitFailed
                }
            };
            
            let ipv4_packet = Ipv4Packet::new(&o)?;
            if ipv4_packet.get_next_level_protocol() == pnet::packet::ip::IpNextHeaderProtocols::Icmp {
                let icmp_packet = EchoReplyPacket::new(ipv4_packet.payload())?;
                if icmp_packet.get_icmp_type() == IcmpTypes::EchoReply {
                    let ipv4_addr_recv = ipv4_packet.get_source();
                    let seq_num_recv = icmp_packet.get_sequence_number();
                    let idetifier = icmp_packet.get_identifier();
                    log::info!("recv_ping_respond: ip: {}, seq_num_recv: {}, id: {}", ipv4_addr_recv, seq_num_recv, idetifier);
                    return Some((ipv4_addr_recv, seq_num_recv));
                }
            }
        }
        log::warn!("recv_ping_respond: size={}, Ipv4Packet::minimum_packet_size()={}, finished with None", size, Ipv4Packet::minimum_packet_size());
        None // TODO: NetError::RecvNotEchoReply
    
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
